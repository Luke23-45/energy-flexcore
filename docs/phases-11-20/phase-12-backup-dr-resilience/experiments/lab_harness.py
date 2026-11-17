# P12.07 — Shared harness for the four defining chaos experiments
#         (C-PG-1, C-NATS-1, C-WORKER-1, C-POOL-1). Pure stdlib.
#
# ══════════════════════════════════════════════════════════════════════════
# HONESTY HEADER — read before trusting anything below
#
# 1. THIS HARNESS WAS WRITTEN, NEVER EXECUTED. No experiment below has ever
#    run; no fault has ever been injected into any system; every evidence
#    cell in the P12.07 record (records/21_p12_07_chaos_program.md) is
#    EMPTY/UNRUN. Execution is a HUMAN GATE on a lab/isolated deployment
#    that does not exist yet (plan §1; conventions §5).
# 2. WHAT THIS IS: the common mechanics the four experiment scripts share —
#    (a) the pre-registration block every script must print before it may
#    run (steady-state hypothesis, blast radius, abort criteria — the
#    [S-81]/[S-82] discipline: hypothesis FIRST, fault second); (b) the
#    probe/parse helpers that read `/ready` and `/metrics` from the lab
#    deployment (both are written code: `apps/worker/src/probes.rs` and the
#    control-plane probe/metrics modules — the P11.21 per-service table);
#    (c) the transcription of the FROZEN 14-state / 27-edge command
#    lifecycle used as the C-PG-1 oracle; (d) the evidence recorder whose
#    timing cells are filled ONLY from a run's own clocks.
# 3. WHAT THIS IS NOT: no subprocess is started by importing this module;
#    no network connection is opened; no file is written; no fault is
#    modeled here (the pure-data fault vocabulary lives in
#    `apps/worker/python/drill/fault_injection.py`, P8.24 + P12.09). The
#    experiment scripts call the lab-control commands (e.g. `docker
#    compose stop postgres`) ONLY when the human gate passes `--execute`
#    with an explicit lab profile.
# 4. THE ORACLE COPY RULE: the 14-state / 27-edge table below is
#    TRANSCRIBED verbatim from
#    `crates/flexcore-domain/src/command_state.rs` (the `FROZEN_EDGES`
#    test table, P0.11 freeze). It is a COPY for lab-side checking, never
#    a second authority: if the Rust table changes, this transcription
#    must be re-diffed and re-cited, and a mismatch is a FINDING, not an
#    improvisation.
# 5. UNITS AND NAMING: metric-family names are the P11.01-taxonomy names
#    as rendered by written code (`flexcore_http_requests_total`,
#    `flexcore_commands_stuck`,
#    `flexcore_command_consumer_duplicate_deliveries_total`,
#    `flexcore_command_state_transitions_total{from,to}`); state names are
#    the frozen SCREAMING_SNAKE vocabulary. No name is invented here.
# 6. Run instruction (human gate ONLY; never run in the writing loop):
#      python -I docs/phases-11-20/phase-12-backup-dr-resilience/experiments/<experiment>.py --help
#    then, against the ISOLATED lab deployment, with `--execute`.
# ══════════════════════════════════════════════════════════════════════════

from __future__ import annotations

import json
import time
import urllib.request
import urllib.error

# ---------------------------------------------------------------------------
# The frozen command lifecycle (C-PG-1 oracle data) — transcribed verbatim
# from `crates/flexcore-domain/src/command_state.rs` (P0.11 freeze).
# ---------------------------------------------------------------------------

STATES_14: tuple = (
    "PROPOSED", "VALIDATED", "FEASIBLE", "APPROVED", "ISSUED",
    "ACKNOWLEDGED", "TRACKING", "COMPLETED", "REJECTED", "FAILED",
    "EXPIRED", "CANCELLED", "SUPERSEDED", "FALLBACK",
)

TERMINAL_5: tuple = ("COMPLETED", "REJECTED", "CANCELLED", "SUPERSEDED", "FALLBACK")

# The 27 frozen edges, in table order (transcription; see the copy rule above).
FROZEN_EDGES_27: tuple = (
    ("PROPOSED", "VALIDATED"),        # 1
    ("PROPOSED", "REJECTED"),         # 2
    ("VALIDATED", "FEASIBLE"),        # 3
    ("VALIDATED", "REJECTED"),        # 4
    ("VALIDATED", "SUPERSEDED"),      # 5
    ("FEASIBLE", "APPROVED"),         # 6
    ("FEASIBLE", "REJECTED"),         # 7
    ("FEASIBLE", "SUPERSEDED"),       # 8
    ("APPROVED", "ISSUED"),           # 9
    ("APPROVED", "EXPIRED"),          # 10
    ("APPROVED", "SUPERSEDED"),       # 11
    ("APPROVED", "CANCELLED"),        # 12
    ("ISSUED", "ACKNOWLEDGED"),       # 13
    ("ISSUED", "FAILED"),             # 14
    ("ISSUED", "EXPIRED"),            # 15
    ("ISSUED", "SUPERSEDED"),         # 16
    ("ISSUED", "CANCELLED"),          # 17
    ("ACKNOWLEDGED", "TRACKING"),     # 18
    ("ACKNOWLEDGED", "FAILED"),       # 19
    ("ACKNOWLEDGED", "EXPIRED"),      # 20
    ("ACKNOWLEDGED", "SUPERSEDED"),   # 21
    ("TRACKING", "COMPLETED"),        # 22
    ("TRACKING", "FAILED"),           # 23
    ("TRACKING", "EXPIRED"),          # 24
    ("TRACKING", "SUPERSEDED"),       # 25
    ("FAILED", "FALLBACK"),           # 26
    ("EXPIRED", "FALLBACK"),          # 27
)


def is_frozen_state(name: str) -> bool:
    """A state text outside the closed 14-state vocabulary is an oracle
    violation (C-PG-1 check 1), never a data quirk."""
    return name in STATES_14


def is_frozen_edge(src: str, dst: str) -> bool:
    """Same-state is legal recovery (no change); otherwise the pair must be
    one of the 27 frozen edges. Terminal states have no outgoing edges —
    enforced by their absence from the table."""
    if src == dst:
        return True
    return (src, dst) in FROZEN_EDGES_27


def is_same_or_strictly_later(pre: str, post: str) -> bool:
    """The C-PG-1 steady-state recovery claim: a command observed in state
    `pre` before the fault is observed in the SAME or a strictly-later
    frozen state after recovery. 'Later' = reachable from `pre` through
    one or more frozen edges (a forward closure over the 27-edge table;
    terminal states close on themselves)."""
    if pre == post:
        return True
    if pre not in STATES_14 or post not in STATES_14:
        return False
    reachable = set()
    frontier = {pre}
    while frontier:
        current = frontier.pop()
        for (src, dst) in FROZEN_EDGES_27:
            if src == current and dst not in reachable:
                reachable.add(dst)
                frontier.add(dst)
    return post in reachable


# ---------------------------------------------------------------------------
# Lab probes — both endpoints are written code (P11.21 per-service probe
# table, `apps/worker/src/probes.rs` and `apps/control-plane/src/probes.rs`).
# ---------------------------------------------------------------------------

class ProbeError(RuntimeError):
    """A probe could not be answered — recorded as UNANSWERED, never
    guessed."""


def http_get(url: str, timeout_s: float = 5.0) -> tuple:
    """One GET against the lab deployment. Returns (status_code, body_text);
    a connection failure raises ProbeError (the caller records it — an
    unanswered probe is a finding of the run, never a fabricated pass)."""
    request = urllib.request.Request(url, method="GET")
    try:
        with urllib.request.urlopen(request, timeout=timeout_s) as response:
            return response.status, response.read().decode("utf-8", errors="replace")
    except urllib.error.HTTPError as error:
        return error.code, error.read().decode("utf-8", errors="replace")
    except OSError as error:
        raise ProbeError("%s: %s" % (type(error).__name__, error)) from error


def parse_metric_series(exposition: str, metric_name: str) -> list:
    """Minimal Prometheus text-format reader for ONE family name: returns a
    list of (labels_dict, float_value) rows for exact-name matches (suffix
    families like `_bucket`/`_sum`/`_count` are NOT matched — the caller
    asks for them by their own name). Unparseable lines are skipped
    silently ONLY if they do not start with the requested name; a line
    that starts with the name but fails to parse raises (the oracle must
    not read a half-line as a value)."""
    rows = []
    for line in exposition.splitlines():
        if not line or line.startswith("#"):
            continue
        if not line.startswith(metric_name):
            continue
        if len(line) > len(metric_name) and line[len(metric_name)] not in (" ", "{"):
            continue  # a longer family name sharing the prefix
        text, _, value_text = line.rpartition(" ")
        labels: dict = {}
        if text.endswith("}"):
            inner = text[len(metric_name) + 1:-1]
            for field in inner.split(","):
                if not field:
                    continue
                key, _, value = field.partition("=")
                labels[key] = value.strip('"')
        rows.append((labels, float(value_text)))
    return rows


def metric_total(exposition: str, metric_name: str, label_filters: dict = None) -> float:
    """Sum of the matching series values (label_filters: exact-match
    constraints, e.g. {"status": "429"}). A family with zero matching
    rows reads as 0.0 — a zero observation, never a missing family claim:
    whether a family renders at all is checked separately by
    `family_present`."""
    rows = parse_metric_series(exposition, metric_name)
    total = 0.0
    for labels, value in rows:
        if label_filters and any(labels.get(k) != v for k, v in label_filters.items()):
            continue
        total += value
    return total


def family_present(exposition: str, metric_name: str) -> bool:
    return len(parse_metric_series(exposition, metric_name)) > 0


# ---------------------------------------------------------------------------
# Pre-registration and evidence (the [S-81]/[S-82] discipline)
# ---------------------------------------------------------------------------

def print_preregistration(experiment: dict) -> None:
    """Prints the pre-registration block VERBATIM before anything runs
    (hypothesis first, fault second — the [S-81] order). `experiment`
    carries: id, hypothesis, anchoring, blast_radius, fault, oracle,
    abort_criteria, roles. Nothing here executes anything."""
    print("=" * 74)
    print("EXPERIMENT PRE-REGISTRATION — %s" % experiment["id"])
    print("=" * 74)
    print("Steady-state hypothesis (verified during AND after the fault):")
    print("  " + experiment["hypothesis"])
    print("")
    print("Anchoring: " + experiment["anchoring"])
    print("Blast radius (bounded by design, never improvised):")
    for item in experiment["blast_radius"]:
        print("  - " + item)
    print("Injected fault: " + experiment["fault"])
    print("In-code oracle: " + experiment["oracle"])
    print("Abort criteria (ANY one aborts the run, evidence kept):")
    for item in experiment["abort_criteria"]:
        print("  - " + item)
    print("Roles (P12.08 template): director=%(director)s, operator=%(operator)s, "
          "observer=%(observer)s, evidence=%(evidence)s" % experiment["roles"])
    print("=" * 74)


class Evidence:
    """The run's evidence record. Every cell starts EMPTY (the string
    "(EMPTY — UNRUN)"); timing cells are filled ONLY from the run's own
    monotonic clocks at execution time. The recorder never computes a
    verdict — pass/fail judgement is the human gate's, against the
    pre-registered hypothesis."""

    EMPTY = "(EMPTY — UNRUN)"

    def __init__(self, experiment_id: str) -> None:
        self.experiment_id = experiment_id
        self.started_monotonic = None
        self.cells = {
            "experiment_id": experiment_id,
            "executed_by": self.EMPTY,
            "witnessed_by": self.EMPTY,
            "lab_profile": self.EMPTY,
            "pre_fault_probe": self.EMPTY,
            "fault_observed": self.EMPTY,
            "during_fault_probe": self.EMPTY,
            "post_recovery_probe": self.EMPTY,
            "oracle_results": self.EMPTY,
            "abort_fired": self.EMPTY,
            "wall_clock_start_unix_ms": self.EMPTY,
            "wall_clock_end_unix_ms": self.EMPTY,
            "notes": self.EMPTY,
        }
        self.oracle_findings: list = []

    def begin(self) -> None:
        self.started_monotonic = time.monotonic()
        self.cells["wall_clock_start_unix_ms"] = int(time.time() * 1000)

    def end(self) -> None:
        self.cells["wall_clock_end_unix_ms"] = int(time.time() * 1000)

    def set(self, key: str, value) -> None:
        if key not in self.cells:
            raise KeyError("unknown evidence cell %r" % (key,))
        self.cells[key] = value

    def add_oracle_finding(self, check: str, passed: bool, detail: str) -> None:
        """One oracle check outcome. `passed=False` is a FINDING of the run
        (kept verbatim), never hidden and never auto-retried."""
        self.oracle_findings.append(
            {"check": check, "passed": bool(passed), "detail": detail}
        )

    def write_json(self, path: str) -> str:
        payload = dict(self.cells)
        payload["oracle_findings"] = list(self.oracle_findings)
        with open(path, "w", encoding="utf-8") as handle:
            json.dump(payload, handle, indent=2, sort_keys=True)
            handle.write("\n")
        return path


def require_confirm(prompt: str, assume_yes: bool) -> bool:
    """The human gate's explicit confirmation at the keyboard. With
    `--yes` (a scripted, witnessed run) the confirmation is recorded as
    assumed-yes with the flag named in the evidence record."""
    if assume_yes:
        return True
    answer = input(prompt + " [type GO to proceed] ")
    return answer.strip() == "GO"
