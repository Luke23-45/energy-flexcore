# P8.14 — The MARL shadow action logger (pure, stdlib-only): every
#         proposed action, its safety-filter outcome, and its simulated
#         consequence — recorded, never silent, never dispatching.
#
# ══════════════════════════════════════════════════════════════════════════
# HONESTY HEADER
#
# 1. WHAT THIS MODULE IS. The ledger P8.14 step 1 shadow action logger:
#    a pure builder/validator for the log entries that record EVERY
#    proposed action together with its safety-filter outcome and the
#    simulated consequence. The log is the evidence surface the P8.15
#    filter statistics (activation rate, infeasibility rate, and the
#    outcome of every infeasibility) are computed from — which is why the
#    `safety_filter_outcome` field is MANDATORY on every entry and is
#    validated against the closed FILTER_OUTCOMES vocabulary: a log that
#    records actions but not filter outcomes makes safety unevaluable
#    (the ledger's named failure mode), and this schema makes that state
#    unrepresentable.
# 2. THE SIMULATED-CONSEQUENCE FIELD IS HONESTLY EMPTY IN THIS LOOP. The
#    consequence of an action requires running the environment (the
#    framework + simulator path — UNREACHABLE in this repository: no
#    framework installed, no run ever executed). Every entry built here
#    carries `consequence_status: "not_simulated"` and
#    `simulated_consequence: None` with a recorded reason — the human/lab
#    gate (P8.15, via the evaluation harness) fills consequences; nothing
#    in this module fabricates one, and the schema FORBIDS a fabricated
#    consequence from riding on a "not_simulated" entry (build_log_entry
#    refuses that combination).
# 3. THE LOG IS NOT A DISPATCH SURFACE. An entry is a RECORD — it has no
#    field that can carry an authorization token, no command key, no
#    lifecycle transition beyond the fixed `lifecycle_state: "PROPOSED"`
#    statement (the proposal-only posture of ADR-010; the worker's
#    `assert_proposal_only` scans the assembled envelope — log included —
#    for forbidden keys at any depth). The logger appends in-memory
#    structures only; it performs NO I/O of its own: the log travels
#    INSIDE the worker's one JSON result document (the in-envelope log is
#    the documented stopgap; the artifact-backed store reference is the
#    P8.20 seam, recorded as a Phase 7/P8.20 follow-up, never invented
#    here).
# 4. STDLIB-ONLY, WRITTEN-NEVER-RUN. No framework import of any kind; the
#    module's ONLY import beyond `__future__` annotations is the sibling
#    pure module `safety_filter` (itself stdlib-only), single-sourcing the
#    closed outcome vocabularies so they cannot drift. The hand-computed
#    expectations in `run_self_tests` are derivations from constructed
#    inputs, never observed outputs; the pytest mirror
#    (tests/test_shadow_log.py) was written and never run.
# ══════════════════════════════════════════════════════════════════════════

from __future__ import annotations

# The sibling pure module (stdlib-only itself, no circular import —
# safety_filter does not import this module). The closed vocabularies are
# SINGLE-SOURCED from it: the logger validates against exactly the filter's
# outcome sets, never a re-defined copy that could drift.
from safety_filter import FILTER_OUTCOMES, REFUSING_OUTCOMES

# The shadow-log schema tag (validated on every entry; fail-closed).
SHADOW_LOG_SCHEMA = "flexcore-marl-shadow-log/1"

# The consequence-status vocabulary (closed). "not_simulated" is the ONLY
# status this loop can produce; "recorded_by_harness" exists for the human
# gate's P8.15 fill-in path (declared there, never here).
CONSEQUENCE_STATUSES: frozenset = frozenset(
    {"not_simulated", "recorded_by_harness"}
)

# The closed per-entry field set (a whitelist — an entry key outside the
# set is refused, so no extra field can ride along unlogged/unvalidated).
LOG_ENTRY_FIELDS: frozenset = frozenset(
    {
        "log_schema",
        "sequence",
        "request_id",
        "input_snapshot_id",
        "worker_mode",
        "policy_version",
        "training_seed",
        "environment_version",
        "action_kind",
        "proposed_permission",
        "safety_filter_outcome",
        "per_entry_outcomes",
        "filter_activations",
        "infeasibility",
        "consequence_status",
        "simulated_consequence",
        "consequence_note",
        "lifecycle_state",
        "proposal_only",
        "authority",
    }
)

# The fixed statements carried on every entry (pinned by self-tests).
LIFECYCLE_STATE = "PROPOSED"
PROPOSAL_ONLY = True
AUTHORITY_STATEMENT = (
    "none — this log entry records a PROPOSAL (ADR-010); no authorization "
    "token exists in this schema and nothing recorded here can be issued "
    "to a device"
)

NOT_SIMULATED_NOTE = (
    "simulated consequence NOT recorded in this loop: the environment "
    "path (framework + simulator) was never executed (Phase 8 bans; no "
    "framework installed); consequences are filled by the human/lab gate "
    "via the P8.15 evaluation harness"
)


class ShadowLogError(ValueError):
    """A malformed log entry or log — refused, never silently repaired."""


def build_log_entry(
    *,
    sequence: int,
    request_id: str,
    input_snapshot_id: str,
    worker_mode: str,
    policy_version: str,
    training_seed: int,
    environment_version: str,
    action_kind: str,
    proposed_permission: str,
    safety_filter_outcome: str,
    per_entry_outcomes: list,
    filter_activations: list,
    infeasibility: dict | None,
    consequence_status: str = "not_simulated",
    simulated_consequence: object = None,
) -> dict:
    """Builds ONE validated shadow-log entry (pure; whitelisted keys only).

    Rules (each pinned by a self-test):
      - `sequence` starts at 1 and is a positive integer (bool excluded);
      - identity/provenance strings are non-empty;
      - `training_seed` is an integer (bool excluded) — the P7.42 seed
        honesty posture: the seed travels or the field names its absence
        convention at the CALLER (the logger never invents one);
      - `safety_filter_outcome` MUST be present and inside the closed
        FILTER_OUTCOMES vocabulary — an entry without a named filter
        outcome is unbuildable (the ledger's "actions but not filter
        outcomes" failure mode is unrepresentable);
      - `consequence_status` "not_simulated" FORBIDS a
        `simulated_consequence` value (a fabricated consequence cannot
        ride on a not-simulated entry); "recorded_by_harness" REQUIRES
        one;
      - the closed field set, the PROPOSED lifecycle statement, and the
        fixed authority statement are appended verbatim.
    """
    if isinstance(sequence, bool) or not isinstance(sequence, int) or sequence < 1:
        raise ShadowLogError(
            "log sequence must be a positive integer starting at 1 (got %r)"
            % (sequence,)
        )
    for field, value in (
        ("request_id", request_id),
        ("input_snapshot_id", input_snapshot_id),
        ("worker_mode", worker_mode),
        ("policy_version", policy_version),
        ("environment_version", environment_version),
        ("action_kind", action_kind),
        ("proposed_permission", proposed_permission),
    ):
        if not isinstance(value, str) or not value.strip():
            raise ShadowLogError("log field %r must be a non-empty string" % field)
    if isinstance(training_seed, bool) or not isinstance(training_seed, int):
        raise ShadowLogError(
            "training_seed must be an integer (P7.42 seed honesty: the "
            "caller declares the actual seed; the logger never invents one)"
        )
    if not isinstance(safety_filter_outcome, str) or (
        safety_filter_outcome not in FILTER_OUTCOMES
    ):
        raise ShadowLogError(
            "safety_filter_outcome %r is not in the closed vocabulary %s "
            "— an entry without a named filter outcome is unbuildable"
            % (safety_filter_outcome, sorted(FILTER_OUTCOMES))
        )
    if consequence_status not in CONSEQUENCE_STATUSES:
        raise ShadowLogError(
            "consequence_status %r is not one of %s"
            % (consequence_status, sorted(CONSEQUENCE_STATUSES))
        )
    if consequence_status == "not_simulated" and simulated_consequence is not None:
        raise ShadowLogError(
            "a not_simulated entry cannot carry a simulated_consequence "
            "value (a fabricated consequence cannot ride on a "
            "not-simulated entry)"
        )
    if consequence_status == "recorded_by_harness" and simulated_consequence is None:
        raise ShadowLogError(
            'a "recorded_by_harness" entry REQUIRES a simulated_consequence '
            "value (the harness fills it; an empty recorded status would "
            "be a false claim)"
        )
    if not isinstance(per_entry_outcomes, list):
        raise ShadowLogError("per_entry_outcomes must be a list")
    if not isinstance(filter_activations, list):
        raise ShadowLogError("filter_activations must be a list")
    if infeasibility is not None and not isinstance(infeasibility, dict):
        raise ShadowLogError("infeasibility must be an object or null")

    entry = {
        "log_schema": SHADOW_LOG_SCHEMA,
        "sequence": sequence,
        "request_id": request_id,
        "input_snapshot_id": input_snapshot_id,
        "worker_mode": worker_mode,
        "policy_version": policy_version,
        "training_seed": training_seed,
        "environment_version": environment_version,
        "action_kind": action_kind,
        "proposed_permission": proposed_permission,
        "safety_filter_outcome": safety_filter_outcome,
        "per_entry_outcomes": list(per_entry_outcomes),
        "filter_activations": list(filter_activations),
        "infeasibility": infeasibility,
        "consequence_status": consequence_status,
        "simulated_consequence": simulated_consequence,
        "consequence_note": NOT_SIMULATED_NOTE
        if consequence_status == "not_simulated"
        else "consequence recorded by the evaluation harness (P8.15)",
        "lifecycle_state": LIFECYCLE_STATE,
        "proposal_only": PROPOSAL_ONLY,
        "authority": AUTHORITY_STATEMENT,
    }
    unknown = sorted(set(entry) - LOG_ENTRY_FIELDS)
    if unknown:  # pragma: no cover — the whitelist above is literal
        raise ShadowLogError(
            "log entry keys outside the closed schema: %s" % unknown
        )
    return entry


def validate_shadow_log(entries: object) -> None:
    """Validates a whole shadow log (pure).

    A log is a list of entries, sequences STRICTLY increasing starting at
    1 (1, 2, 3, ... — no gaps, no duplicates: a shadow log is an ordered
    record, and a gap would hide an action), every entry carrying the
    schema tag, and every `safety_filter_outcome` inside the closed
    vocabulary.
    """
    if not isinstance(entries, list):
        raise ShadowLogError("a shadow log must be a list of entries")
    for index, entry in enumerate(entries):
        if not isinstance(entry, dict):
            raise ShadowLogError("shadow_log[%d] must be an object" % index)
        if entry.get("log_schema") != SHADOW_LOG_SCHEMA:
            raise ShadowLogError(
                "shadow_log[%d].log_schema %r is not %r"
                % (index, entry.get("log_schema"), SHADOW_LOG_SCHEMA)
            )
        sequence = entry.get("sequence")
        if isinstance(sequence, bool) or not isinstance(sequence, int):
            raise ShadowLogError("shadow_log[%d].sequence must be an integer" % index)
        if sequence != index + 1:
            raise ShadowLogError(
                "shadow_log[%d].sequence %r breaks the strictly-increasing "
                "1-based order (expected %d) — a gap would hide an action"
                % (index, sequence, index + 1)
            )
        outcome = entry.get("safety_filter_outcome")
        if outcome not in FILTER_OUTCOMES:
            raise ShadowLogError(
                "shadow_log[%d].safety_filter_outcome %r is not in the "
                "closed vocabulary" % (index, outcome)
            )


def log_summary(entries: list) -> dict:
    """The pure per-log statistics the P8.15 filter statistics consume.

    Counts by closed outcome — activation count (entries with at least
    one clamp), infeasibility count, refusal counts — NO rate and NO
    percentage (a rate needs a denominator the harness owns; a summary
    here invents nothing). Sequences are validated first (a summary over
    a gapped log would hide actions).
    """
    validate_shadow_log(entries)
    by_outcome: dict = {}
    activation_count = 0
    infeasibility_count = 0
    for entry in entries:
        outcome = entry["safety_filter_outcome"]
        by_outcome[outcome] = by_outcome.get(outcome, 0) + 1
        if entry.get("filter_activations"):
            activation_count += 1
        if entry.get("infeasibility") is not None:
            infeasibility_count += 1
    return {
        "total_actions": len(entries),
        "by_outcome": by_outcome,
        "filter_activation_count": activation_count,
        "infeasibility_count": infeasibility_count,
        "refusal_count": sum(
            count for outcome, count in by_outcome.items()
            if outcome in REFUSING_OUTCOMES
        ),
        "consequence_status_counts": {
            status: sum(
                1 for entry in entries
                if entry.get("consequence_status") == status
            )
            for status in sorted(CONSEQUENCE_STATUSES)
        },
    }


# ---------------------------------------------------------------------------
# Hand-computed self-tests (NEVER RUN in this loop; expectations are hand
# derivations from constructed inputs). Run instruction (human gate):
#   python apps/worker/python/marl_worker/shadow_log.py
# ---------------------------------------------------------------------------


def _entry(sequence: int, outcome: str, **overrides) -> dict:
    """Builds a VALIDATED canonical entry (through build_log_entry, so
    every self-test exercises the real builder — never a hand-rolled
    dict that could drift from the schema)."""
    base = dict(
        sequence=sequence,
        request_id="req-%04d" % sequence,
        input_snapshot_id="snapshot-RECORD",
        worker_mode="filter_probe",
        policy_version="RECORD-AT-FIRST-RUN",
        training_seed=0,
        environment_version="flexcore-marl-env/0.1.0",
        action_kind="setpoint_delta",
        proposed_permission="schedule.propose",
        safety_filter_outcome=outcome,
        per_entry_outcomes=[{"asset_id": "ess-1", "entry_outcome": "within_limits"}],
        filter_activations=[],
        infeasibility=None,
    )
    base.update(overrides)
    return build_log_entry(**base)


def run_self_tests() -> list:
    results: list = []

    def check(name: str, condition: bool, detail: str) -> None:
        results.append({"name": name, "passed": bool(condition), "detail": detail})

    # ---- Test 1: a canonical entry builds with the fixed statements ----
    entry = _entry(1, "passed_within_limits")
    check(
        "canonical_entry_fixed_statements",
        entry["lifecycle_state"] == "PROPOSED"
        and entry["proposal_only"] is True
        and entry["authority"].startswith("none —")
        and entry["consequence_status"] == "not_simulated"
        and entry["simulated_consequence"] is None
        and entry["log_schema"] == "flexcore-marl-shadow-log/1",
        "the entry carries PROPOSED / proposal_only / authority-none / "
        "not_simulated verbatim",
    )

    # ---- Test 2: sequence rules -----------------------------------------
    # (_entry builds through build_log_entry, so each refusal below is the
    # real builder's refusal.)
    def zero_sequence() -> None:
        _entry(0, "passed_within_limits")

    _expects_log_error(results, zero_sequence, "zero_sequence_refused")

    def bool_sequence() -> None:
        _entry(True, "passed_within_limits")

    _expects_log_error(results, bool_sequence, "bool_sequence_refused")

    # ---- Test 3: the filter outcome is MANDATORY and closed -------------
    def missing_vocabulary() -> None:
        _entry(1, "everything_fine")

    _expects_log_error(
        results, missing_vocabulary, "outcome_outside_vocabulary_refused"
    )

    # ---- Test 4: the fabricated-consequence pair is unbuildable ---------
    def fabricated_consequence() -> None:
        _entry(
            1,
            "passed_within_limits",
            simulated_consequence={"violation": False},
        )

    _expects_log_error(
        results, fabricated_consequence, "fabricated_consequence_refused"
    )

    def empty_recorded() -> None:
        _entry(1, "passed_within_limits", consequence_status="recorded_by_harness")

    _expects_log_error(results, empty_recorded, "empty_recorded_status_refused")

    # ---- Test 5: log order — strictly increasing from 1, gaps refused ---
    log = [_entry(1, "passed_within_limits"), _entry(2, "filtered_clamped_to_limits",
            filter_activations=[{"asset_id": "ess-1"}])]
    try:
        validate_shadow_log(log)
        ordered_ok = True
    except ShadowLogError:
        ordered_ok = False
    check("log_order_ok", ordered_ok, "sequences 1, 2 validate")

    gapped = [_entry(1, "passed_within_limits"), _entry(3, "passed_within_limits")]
    def gapped_log() -> None:
        validate_shadow_log(gapped)

    _expects_log_error(results, gapped_log, "sequence_gap_refused_hides_nothing")

    # ---- Test 6: hand-computed summary arithmetic -----------------------
    # HAND CHECK: entries 1..4 with outcomes passed / clamped / refused-
    # permission / infeasible -> total 4; by_outcome counts 1 each for
    # passed_within_limits and filtered_clamped_to_limits and 2 refusals
    # (refused_permission_ceiling + infeasible_no_safe_projection);
    # activation_count 1 (only entry 2 carries an activation);
    # infeasibility_count 1; consequence all not_simulated (4).
    summary_log = [
        _entry(1, "passed_within_limits"),
        _entry(2, "filtered_clamped_to_limits",
               filter_activations=[{"asset_id": "ess-1", "p_filtered_mw": 3.5}]),
        _entry(3, "refused_permission_ceiling"),
        _entry(4, "infeasible_no_safe_projection",
               infeasibility={"outcome": "infeasible_no_safe_projection",
                              "fallback_attempted": True,
                              "fallback_viable": False}),
    ]
    summary = log_summary(summary_log)
    check(
        "summary_hand_computed_counts",
        summary["total_actions"] == 4
        and summary["by_outcome"] == {
            "passed_within_limits": 1,
            "filtered_clamped_to_limits": 1,
            "refused_permission_ceiling": 1,
            "infeasible_no_safe_projection": 1,
        }
        and summary["filter_activation_count"] == 1
        and summary["infeasibility_count"] == 1
        and summary["refusal_count"] == 2
        and summary["consequence_status_counts"] == {
            "not_simulated": 4,
            "recorded_by_harness": 0,
        },
        "counts hand-computed: 4 total, 1 activation, 1 infeasibility, "
        "2 refusals, all consequences not_simulated",
    )

    # ---- Test 7: no dispatch-shaped key can enter through the builder ---
    # The whitelist is literal; assert the closed field set contains no
    # authorization/command-shaped name.
    check(
        "closed_field_set_has_no_authority_shaped_key",
        not (LOG_ENTRY_FIELDS
             & {"authorization", "authorization_token", "command", "command_id",
                "dispatch", "dispatch_command", "issued_command", "approval",
                "grant"}),
        "the closed LOG_ENTRY_FIELDS set contains none of the "
        "authorization/command-shaped key names",
    )

    return results


def _expects_log_error(results: list, fn, name: str) -> None:
    try:
        fn()
    except ShadowLogError:
        results.append(
            {
                "name": name,
                "passed": True,
                "detail": "raised ShadowLogError as expected",
            }
        )
        return
    except Exception as error:  # noqa: BLE001 — pin the error class
        results.append(
            {
                "name": name,
                "passed": False,
                "detail": "raised %r instead of ShadowLogError" % error,
            }
        )
        return
    results.append({"name": name, "passed": False, "detail": "raised nothing"})


if __name__ == "__main__":  # pragma: no cover — human-gate entrypoint
    _records = run_self_tests()
    _failures = [r for r in _records if not r["passed"]]
    for _r in _records:
        print(("PASS" if _r["passed"] else "FAIL"), _r["name"], "-", _r["detail"])
    print("%d/%d self-tests passed" % (len(_records) - len(_failures), len(_records)))
    raise SystemExit(1 if _failures else 0)
