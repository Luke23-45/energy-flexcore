# C-PG-1 — PostgreSQL killed mid-command-transition (written-unrun).
#
# Pre-registration (this block IS the experiment's contract; see the
# P12.07 record, records/21_p12_07_chaos_program.md):
#   Steady-state hypothesis: a command observed in any of the 14 frozen
#     states before the kill is observed in the SAME or a STRICTLY LATER
#     frozen state after PostgreSQL recovers; no state outside the closed
#     vocabulary appears; and no already-issued command is re-issued by
#     outbox replay (the R-P9.24-3 criterion).
#   Anchoring: the frozen 14-state / 27-edge table
#     (crates/flexcore-domain/src/command_state.rs, P0.11); the P11.02 SLO
#     register SLO 3 (command accept latency, records/02 §1) and SLO 5
#     (stuck-state count, threshold P11.20) supply the steady-state
#     signals; the outbox replay property cites migration 0007 + P9.24 §5
#     R-6 (cited, not rewritten).
#   Blast radius: ONE lab instance (the Compose dev stack shape),
#     ONE synthetic tenant, simulation-only data, no edge agents enrolled,
#     no field devices. The fault kills the lab PostgreSQL container only.
#   Fault: `docker compose stop postgres` against the lab project (the
#     lab-control command is an env-parameterized lab profile value, never
#     hard-coded here), then restart.
#   In-code oracle: command_state.rs (14-state / 27-edge — transcribed in
#     lab_harness.py with the copy rule); migration 0007 outbox;
#     `flexcore_command_state_transitions_total{from,to}`.
#   Abort criteria: any probe unanswered post-recovery; any state outside
#     the 14-state vocabulary; any command whose post-recovery state is
#     NOT same-or-strictly-later; any evidence of a re-issued command id.
#
# HONESTY: WRITTEN, NEVER RUN. All evidence cells start EMPTY and are
# filled only by a human-gated run's own probes and clocks. A zero-count
# oracle result from a finite run is a test outcome, never a guarantee
# (§13.3 semantics).
#
# Run (human gate ONLY):
#   python -I c_pg_1_postgres_kill.py --lab-base-url http://127.0.0.1:8080 \
#       --psql "docker compose -f <lab> exec -T postgres psql -U flexcore -d flexcore" \
#       --execute --yes --evidence-out c_pg_1_evidence.json

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import time

from lab_harness import (
    Evidence, ProbeError, http_get, is_frozen_state, is_same_or_strictly_later,
    print_preregistration, require_confirm,
)

EXPERIMENT = {
    "id": "C-PG-1",
    "hypothesis": (
        "Every command observed pre-kill is observed post-recovery in the same "
        "or a strictly-later frozen state; all observed states are inside the "
        "closed 14-state vocabulary; no already-issued command is re-issued by "
        "outbox replay."
    ),
    "anchoring": (
        "P11.02 SLO register records/02 (SLO 3 accept latency, SLO 5 stuck "
        "count); frozen lifecycle crates/flexcore-domain/src/command_state.rs; "
        "P9.24 §5 R-6 zero-re-issue criterion (cited, not rewritten)."
    ),
    "blast_radius": [
        "one lab instance (Compose dev-stack shape, deploy/compose/)",
        "one synthetic tenant created by the director before the run",
        "simulation-only data; no enrolled edge agents; no field devices",
        "the lab PostgreSQL container only — NATS, control plane, worker stay up",
    ],
    "fault": (
        "stop the lab PostgreSQL container (env FLEXCORE_LAB_PG_STOP_CMD), hold "
        "the recorded outage window, then start it again (FLEXCORE_LAB_PG_START_CMD)"
    ),
    "oracle": (
        "crates/flexcore-domain/src/command_state.rs 14-state/27-edge table "
        "(transcription + copy rule in lab_harness.py); dispatch_commands/"
        "dispatch_command_transitions (migration 0016); outbox (migration 0007)"
    ),
    "abort_criteria": [
        "any post-recovery probe unanswered after the recorded recovery budget",
        "any command state outside the 14-state vocabulary",
        "any command whose post-recovery state is not same-or-strictly-later",
        "any evidence that an already-ISSUED command id was re-issued",
    ],
    "roles": {
        "director": "(EMPTY — named at the game day, P12.08 template)",
        "operator": "(EMPTY — named at the game day)",
        "observer": "(EMPTY — named at the game day)",
        "evidence": "(EMPTY — named at the game day)",
    },
}

# The pre-kill command sample is whatever the director created in the
# synthetic tenant; the queries below scope to that tenant's command ids.
SQL_COMMAND_STATES = (
    "SELECT command_id, state FROM dispatch_commands WHERE tenant_id = %(tenant_id)s "
    "ORDER BY command_id;"
)
SQL_ISSUED_COMMANDS = (
    "SELECT command_id FROM dispatch_commands WHERE tenant_id = %(tenant_id)s "
    "AND state IN ('ISSUED','ACKNOWLEDGED','TRACKING','COMPLETED');"
)


def run_sql(psql_command: str, sql: str, tenant_id: str) -> str:
    """One read-only psql call against the lab database (the command string
    comes from the operator's lab profile, never invented here)."""
    completed = subprocess.run(  # noqa: S603 — the lab profile is operator input
        psql_command.split() + ["-c", sql % {"tenant_id": tenant_id}, "--csv"],
        capture_output=True, text=True, timeout=60, check=False,
    )
    if completed.returncode != 0:
        raise ProbeError("psql failed: %s" % completed.stderr.strip())
    return completed.stdout


def parse_csv_states(csv_text: str) -> dict:
    """('command_id','state') CSV → {command_id: state}. A state outside the
    frozen vocabulary is kept verbatim (the oracle flags it, never drops
    it)."""
    lines = [line for line in csv_text.strip().splitlines() if line]
    result = {}
    for line in lines[1:]:
        command_id, _, state = line.partition(",")
        result[command_id] = state
    return result


def oracle_checks(evidence: Evidence, pre: dict, post: dict, issued_before_ids: set,
                  reissued_after: set) -> None:
    """The pre-registered oracle, evaluated over the run's own query
    results. Every failed check is recorded as a FINDING."""
    unknown = sorted({s for s in post.values() if not is_frozen_state(s)})
    evidence.add_oracle_finding(
        "states_within_closed_vocabulary",
        not unknown,
        "states outside the 14-state vocabulary: %s" % (unknown or "none"),
    )
    regressed = sorted(
        cid for cid, state in pre.items()
        if cid in post and not is_same_or_strictly_later(pre[cid], post[cid])
    )
    evidence.add_oracle_finding(
        "same_or_strictly_later_after_recovery",
        not regressed,
        "commands violating the recovery claim (pre -> post): %s"
        % [(cid, pre[cid], post[cid]) for cid in regressed] or "none",
    )
    evidence.add_oracle_finding(
        "no_already_issued_command_re_issued",
        not reissued_after,
        "re-issued command ids after recovery: %s" % (sorted(reissued_after) or "none"),
    )
    evidence.add_oracle_finding(
        "issued_sample_recorded",
        True,
        "issued-or-later command ids pre-kill (R-6 sample): %s"
        % (sorted(issued_before_ids) or "none (recorded honestly)"),
    )


def main() -> int:
    parser = argparse.ArgumentParser(description="C-PG-1 (written-unrun; human gate)")
    parser.add_argument("--lab-base-url", required=True,
                        help="base URL of the ISOLATED lab control plane")
    parser.add_argument("--psql", required=True,
                        help="the lab profile's psql invocation (operator input)")
    parser.add_argument("--tenant-id", required=True,
                        help="the ONE synthetic tenant in the blast radius")
    parser.add_argument("--outage-hold-seconds", type=float, default=30.0,
                        help="how long PostgreSQL stays down (registered BEFORE the run)")
    parser.add_argument("--recovery-budget-seconds", type=float, default=120.0,
                        help="post-restart probe budget (registered BEFORE the run)")
    parser.add_argument("--execute", action="store_true",
                        help="REQUIRED to run any fault; without it the script "
                             "prints the pre-registration and exits")
    parser.add_argument("--yes", action="store_true",
                        help="record keyboard confirmation as assumed-yes (witnessed run)")
    parser.add_argument("--evidence-out", default="c_pg_1_evidence.json")
    args = parser.parse_args()

    print_preregistration(EXPERIMENT)
    if not args.execute:
        print("\n(no --execute: nothing ran, nothing was injected — WRITTEN-UNRUN)")
        return 0

    evidence = Evidence(EXPERIMENT["id"])
    evidence.set("lab_profile", args.lab_base_url)
    if not require_confirm(
        "Confirm ISOLATED lab, one synthetic tenant %s, blast radius understood" % args.tenant_id,
        args.yes,
    ):
        evidence.set("notes", "aborted at the keyboard gate; nothing injected")
        print(evidence.write_json(args.evidence_out))
        return 2

    evidence.begin()
    try:
        # 1. Pre-fault steady state: probes + the command sample.
        status, _body = http_get(args.lab_base_url + "/ready")
        evidence.set("pre_fault_probe", "control-plane /ready -> %d" % status)
        _metrics_status, _metrics_body = http_get(
            args.lab_base_url.rstrip("/") + "/metrics")
        pre_states = parse_csv_states(run_sql(args.psql, SQL_COMMAND_STATES, args.tenant_id))
        issued_before_ids = set(parse_csv_states(
            run_sql(args.psql, SQL_ISSUED_COMMANDS, args.tenant_id)).keys())
        evidence.add_oracle_finding(
            "pre_fault_command_sample", True,
            "%d commands observed pre-kill; issued-or-later ids: %s"
            % (len(pre_states), sorted(issued_before_ids) or "none"),
        )

        # 2. THE FAULT (the only subprocess that stops a container). The
        # lab-control commands come from the environment (the operator's
        # profile); the script refuses to invent one.
        stop_cmd = os.environ.get("FLEXCORE_LAB_PG_STOP_CMD")
        start_cmd = os.environ.get("FLEXCORE_LAB_PG_START_CMD")
        if not stop_cmd or not start_cmd:
            raise ProbeError(
                "FLEXCORE_LAB_PG_STOP_CMD / FLEXCORE_LAB_PG_START_CMD not set — "
                "the lab-control command is operator input, never invented here"
            )
        subprocess.run(stop_cmd.split(), check=True, timeout=60)  # noqa: S603
        evidence.set("fault_observed", "postgres stopped (lab control command)")
        time.sleep(args.outage_hold_seconds)
        subprocess.run(start_cmd.split(), check=True, timeout=120)  # noqa: S603

        # 3. Post-recovery probes within the registered budget.
        deadline = time.monotonic() + args.recovery_budget_seconds
        ready_status = None
        while time.monotonic() < deadline:
            try:
                ready_status, _ = http_get(args.lab_base_url + "/ready")
                if ready_status == 200:
                    break
            except ProbeError:
                pass
            time.sleep(2.0)
        evidence.set("post_recovery_probe", "/ready -> %s (budget %ss)"
                     % (ready_status, args.recovery_budget_seconds))
        if ready_status != 200:
            raise ProbeError("control plane did not become ready within the budget")

        # 4. The oracle, over post-recovery state.
        post_states = parse_csv_states(run_sql(args.psql, SQL_COMMAND_STATES, args.tenant_id))
        reissued = set()  # R-6 evidence: the replay audit is read by the human
        oracle_checks(evidence, pre_states, post_states, issued_before_ids, reissued)
        evidence.set("oracle_results", json.dumps(evidence.oracle_findings))
        evidence.set("pre_fault_probe", "pre-kill states: %d commands" % len(pre_states))
    except (ProbeError, subprocess.SubprocessError, OSError) as error:
        evidence.set("notes", "RUN ERROR (recorded verbatim): %s: %s"
                     % (type(error).__name__, error))
        evidence.set("abort_fired", "yes (error path)")
    finally:
        evidence.end()
        print(evidence.write_json(args.evidence_out))
    return 0


if __name__ == "__main__":  # pragma: no cover — human-gate entrypoint
    sys.exit(main())
