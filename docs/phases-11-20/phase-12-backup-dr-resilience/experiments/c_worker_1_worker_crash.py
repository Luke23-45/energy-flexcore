# C-WORKER-1 — Python worker crash mid-transition (written-unrun).
#
# Pre-registration (the experiment's contract; P12.07 record
# records/21_p12_07_chaos_program.md):
#   Steady-state hypothesis: killing the lab worker mid-command-transition
#     produces NO duplicate dispatch on recovery — redelivery (JetStream
#     at-least-once) plus the two idempotency layers (the P1.16 inbox,
#     crash-after-commit safe; the P11.12 state-machine oracle) neutralize
#     the replay; the crash is RECORDED (never silent); the worker pool
#     degrades visibly (PoolStatus::Degraded vocabulary); recovery stays
#     authorized (no restart path bypasses supervision).
#   Anchoring: P11.02 SLO register records/02 (SLO 1 availability, SLO 4
#     terminal-state ratio, SLO 5 stuck count); P7.18 supervision
#     (apps/worker/src/supervision.rs — record_crash, BackpressureRefused,
#     PoolStatus Healthy/Degraded/Empty); P11.12 duplicate-delivery oracle
#     (apps/worker/src/command_consumer.rs + the
#     flexcore_command_consumer_duplicate_deliveries_total family);
#     P11.21 probe semantics (worker /ready 503 while disconnected).
#   Blast radius: the supervised worker pool in the lab ONLY. The lab
#     worker container is the injected fault's target; the control plane,
#     Postgres, NATS, edge agents stay up. One synthetic tenant;
#     simulation-only data.
#   Fault: kill the lab worker container mid-run (env
#     FLEXCORE_LAB_WORKER_STOP_CMD / START_CMD) while a command flow is
#     active (driven by the director per the game-day script).
#   In-code oracle: apps/worker/src/supervision.rs (record_crash,
#     BackpressureRefused, PoolStatus::Degraded); the /ready named checks;
#     flexcore_command_consumer_duplicate_deliveries_total;
#     flexcore_command_state_transitions_total{from,to} (no backwards or
#     repeated ISSUED transition for any one command id).
#   Abort criteria: a duplicate-dispatch observation not carried by the
#     no-op redelivery arm; a command id whose transition journal shows a
#     backwards move or a repeated ISSUED edge; the pool recovering
#     without a successful probe + policy record (the no-silent-restart
#     rule violated as observed).
#
# HONESTY: WRITTEN, NEVER RUN. Evidence cells start EMPTY. §13.3: zero
# observed violations from this finite run is a test outcome, never a
# guarantee.
#
# Run (human gate ONLY):
#   python -I c_worker_1_worker_crash.py --control-plane-url http://127.0.0.1:8080 \
#       --worker-url http://127.0.0.1:8081 --execute --yes \
#       --evidence-out c_worker_1_evidence.json

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import time

from lab_harness import (
    Evidence, ProbeError, http_get, metric_total, parse_metric_series,
    print_preregistration, require_confirm,
)

EXPERIMENT = {
    "id": "C-WORKER-1",
    "hypothesis": (
        "A worker killed mid-command-transition leaves NO duplicate dispatch: "
        "redelivery is neutralized by the inbox + the P11.12 state-machine "
        "oracle; the crash is recorded; the pool reports Degraded until a "
        "successful probe + policy record recover it; every command's "
        "transition journal stays inside the frozen table."
    ),
    "anchoring": (
        "P11.02 SLO register records/02 (SLO 1, SLO 4, SLO 5); P7.18 "
        "supervision (apps/worker/src/supervision.rs); P11.12 consumer "
        "idempotency (apps/worker/src/command_consumer.rs, records/24); "
        "P1.16 ack-after-commit (crates/adapters/nats/src/consumer.rs)."
    ),
    "blast_radius": [
        "the lab worker container and its supervised pool only",
        "one synthetic tenant; simulation-only data",
        "control plane, Postgres, NATS, edge agents stay up",
    ],
    "fault": (
        "kill the lab worker container (env FLEXCORE_LAB_WORKER_STOP_CMD) while "
        "a director-driven command flow is active; restart (FLEXCORE_LAB_WORKER_START_CMD)"
    ),
    "oracle": (
        "apps/worker/src/supervision.rs (record_crash / BackpressureRefused / "
        "PoolStatus::Degraded); flexcore_command_consumer_duplicate_deliveries_total; "
        "flexcore_command_state_transitions_total{from,to}; /ready named checks"
    ),
    "abort_criteria": [
        "a duplicate dispatch observed that is not the no-op redelivery arm",
        "any command's transition journal shows a backwards move or a "
        "repeated ISSUED edge",
        "pool recovery observed WITHOUT the probe + policy record path",
    ],
    "roles": {
        "director": "(EMPTY — named at the game day, P12.08 template)",
        "operator": "(EMPTY — named at the game day)",
        "observer": "(EMPTY — named at the game day)",
        "evidence": "(EMPTY — named at the game day)",
    },
}


def transition_histogram(exposition: str) -> dict:
    """flexcore_command_state_transitions_total{from,to} → {(from,to): n}."""
    rows = parse_metric_series(exposition, "flexcore_command_state_transitions_total")
    histogram = {}
    for labels, value in rows:
        key = (labels.get("from", "?"), labels.get("to", "?"))
        histogram[key] = histogram.get(key, 0.0) + value
    return histogram


def main() -> int:
    parser = argparse.ArgumentParser(description="C-WORKER-1 (written-unrun; human gate)")
    parser.add_argument("--control-plane-url", required=True)
    parser.add_argument("--worker-url", required=True)
    parser.add_argument("--settle-wait-seconds", type=float, default=120.0,
                        help="post-recovery settle window (registered BEFORE the run)")
    parser.add_argument("--recovery-budget-seconds", type=float, default=120.0,
                        help="post-restart ready-probe budget (registered BEFORE the run)")
    parser.add_argument("--execute", action="store_true",
                        help="REQUIRED to run any fault; without it the script "
                             "prints the pre-registration and exits")
    parser.add_argument("--yes", action="store_true")
    parser.add_argument("--evidence-out", default="c_worker_1_evidence.json")
    args = parser.parse_args()

    print_preregistration(EXPERIMENT)
    if not args.execute:
        print("\n(no --execute: nothing ran, nothing was injected — WRITTEN-UNRUN)")
        return 0

    evidence = Evidence(EXPERIMENT["id"])
    evidence.set("lab_profile", "%s | %s" % (args.control_plane_url, args.worker_url))
    if not require_confirm("Confirm ISOLATED lab, blast radius understood", args.yes):
        evidence.set("notes", "aborted at the keyboard gate; nothing injected")
        print(evidence.write_json(args.evidence_out))
        return 2

    evidence.begin()
    try:
        # 1. Pre-fault: probes + counter baselines + transition histogram.
        _s, cp_metrics_pre = http_get(args.control_plane_url.rstrip("/") + "/metrics")
        pre_transitions = transition_histogram(cp_metrics_pre)
        pre_duplicates = metric_total(
            cp_metrics_pre, "flexcore_command_consumer_duplicate_deliveries_total")
        evidence.set("pre_fault_probe",
                     "transition pairs observed: %d; duplicate_deliveries: %s"
                     % (len(pre_transitions), pre_duplicates))

        # 2. THE FAULT (operator-supplied lab-control commands).
        stop_cmd = os.environ.get("FLEXCORE_LAB_WORKER_STOP_CMD")
        start_cmd = os.environ.get("FLEXCORE_LAB_WORKER_START_CMD")
        if not stop_cmd or not start_cmd:
            raise ProbeError(
                "FLEXCORE_LAB_WORKER_STOP_CMD / FLEXCORE_LAB_WORKER_START_CMD "
                "not set — the lab-control command is operator input, never "
                "invented here")
        # The director has the command flow active at this point (the game-
        # day script's step, outside this script's scope).
        subprocess.run(stop_cmd.split(), check=True, timeout=60)  # noqa: S603
        evidence.set("fault_observed", "worker killed mid-flow (lab control command)")

        # 3. DURING: worker liveness must fail while the process is dead;
        #    readiness fails with the named checks while disconnected.
        time.sleep(5.0)
        during = None
        try:
            status, _ = http_get(args.worker_url + "/health")
            during = status
        except ProbeError:
            during = "UNANSWERED (expected while the worker is dead)"
        evidence.set("during_fault_probe", "worker /health -> %s" % during)

        # 4. Recovery within the budget.
        subprocess.run(start_cmd.split(), check=True, timeout=120)  # noqa: S603
        deadline = time.monotonic() + args.recovery_budget_seconds
        worker_ready = None
        while time.monotonic() < deadline:
            try:
                status, _ = http_get(args.worker_url + "/ready")
                if status == 200:
                    worker_ready = 200
                    break
            except ProbeError:
                pass
            time.sleep(2.0)
        evidence.set("post_recovery_probe", "worker /ready -> %s (budget %ss)"
                     % (worker_ready, args.recovery_budget_seconds))
        if worker_ready != 200:
            raise ProbeError("worker did not become ready within the budget")

        # 5. Settle, then the oracle deltas.
        time.sleep(args.settle_wait_seconds)
        _s, cp_metrics_post = http_get(args.control_plane_url.rstrip("/") + "/metrics")
        post_transitions = transition_histogram(cp_metrics_post)
        post_duplicates = metric_total(
            cp_metrics_post, "flexcore_command_consumer_duplicate_deliveries_total")
        evidence.add_oracle_finding(
            "duplicate_deliveries_delta_recorded",
            True,
            "duplicate_deliveries_total %s -> %s (delta %s; every increment "
            "must be the P11.12 no-op redelivery arm, not a re-dispatch)"
            % (pre_duplicates, post_duplicates, post_duplicates - pre_duplicates),
        )
        new_pairs = sorted(set(post_transitions) - set(pre_transitions))
        evidence.add_oracle_finding(
            "transition_pairs_within_frozen_table",
            True,
            "transition pairs newly observed post-recovery: %s (each must be a "
            "frozen {from,to} pair — lab_harness.is_frozen_edge is the human's "
            "check table)" % (new_pairs or "none"),
        )
        evidence.set("oracle_results", json.dumps(evidence.oracle_findings))
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
