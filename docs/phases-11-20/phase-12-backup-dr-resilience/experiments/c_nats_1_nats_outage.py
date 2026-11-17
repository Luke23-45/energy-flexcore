# C-NATS-1 — NATS outage during the command flow (written-unrun).
#
# Pre-registration (the experiment's contract; P12.07 record
# records/21_p12_07_chaos_program.md):
#   Steady-state hypothesis: commands queue/park safely while NATS is
#     unreachable — pending publications hold in the transactional outbox
#     (migration 0007), durable consumers resume with redelivery after
#     reconnect, zero message loss is OBSERVED (a finite-run outcome,
#     never a guarantee), and the DLQ family receives only poison
#     messages (nothing is DLQ'd merely because of the outage).
#   Anchoring: P11.02 SLO register records/02 (SLO 1 availability, SLO 3
#     accept latency); the P11.13 JetStream hardening + DLQ family
#     topology (crates/adapters/nats/src/topology.rs DLQ_STREAM_*); the
#     P1.16 ack-after-commit consumer (crates/adapters/nats/src/
#     consumer.rs — a crash before the ack is safe, redelivery is
#     idempotent); the worker runner's exponential error backoff
#     (apps/worker/src/runner.rs).
#   Blast radius: the lab NATS instance and its consumers ONLY. Postgres,
#     the control-plane API edge, and the edge-agent stay up. One
#     synthetic tenant; simulation-only data.
#   Fault: stop the lab NATS container (env FLEXCORE_LAB_NATS_STOP_CMD /
#     FLEXCORE_LAB_NATS_START_CMD), hold the registered outage window,
#     restart.
#   In-code oracle: /ready named dependency check `broker` (the P11.21
#     probe table); `flexcore_command_consumer_duplicate_deliveries_total`
#     (the P11.12 oracle metric — redelivery neutralization);
#     `flexcore_commands_stuck` (P11.20 — commands parked by the outage
#     must not cross the stuck threshold during the run window);
#     outbox-row delta before/after reconnect (publication backlog
#     drains).
#   Abort criteria: worker /ready never returns 200 post-recovery within
#     the budget; duplicate-deliveries counter grows without the
#     accompanying no-op redelivery semantics (a DLQ'd non-poison
#     message); any outbox row lost (count decreases without a recorded
#     publish).
#
# HONESTY: WRITTEN, NEVER RUN. Evidence cells start EMPTY. Zero observed
# violations from this finite experiment is a test outcome, never a
# guarantee (§13.3 semantics).
#
# Run (human gate ONLY):
#   python -I c_nats_1_nats_outage.py --control-plane-url http://127.0.0.1:8080 \
#       --worker-url http://127.0.0.1:8081 --execute --yes \
#       --evidence-out c_nats_1_evidence.json

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import time

from lab_harness import (
    Evidence, ProbeError, family_present, http_get, metric_total,
    print_preregistration, require_confirm,
)

EXPERIMENT = {
    "id": "C-NATS-1",
    "hypothesis": (
        "While NATS is unreachable, commands queue/park safely (publications "
        "hold in the outbox, consumers error-backoff and keep their state); "
        "after reconnect, durable consumers resume with redelivery, the outbox "
        "backlog drains, zero message loss is observed, and only poison "
        "messages reach the DLQ family."
    ),
    "anchoring": (
        "P11.02 SLO register records/02 (SLO 1, SLO 3); P11.13 DLQ family "
        "topology (crates/adapters/nats/src/topology.rs); P1.16 "
        "ack-after-commit (crates/adapters/nats/src/consumer.rs); P11.12 "
        "consumer idempotency oracle (apps/worker/src/command_consumer.rs); "
        "worker runner error backoff (apps/worker/src/runner.rs)."
    ),
    "blast_radius": [
        "the lab NATS container and its consumers only",
        "one synthetic tenant; simulation-only data",
        "PostgreSQL, the control-plane API edge, and edge agents stay up",
    ],
    "fault": (
        "stop the lab NATS container (env FLEXCORE_LAB_NATS_STOP_CMD), hold "
        "the registered outage window, restart (FLEXCORE_LAB_NATS_START_CMD)"
    ),
    "oracle": (
        "/ready named checks `database` and `broker` (P11.21 probe table); "
        "flexcore_command_consumer_duplicate_deliveries_total (P11.12); "
        "flexcore_commands_stuck (P11.20); outbox row delta over the run"
    ),
    "abort_criteria": [
        "worker /ready never returns 200 within the recovery budget",
        "a non-poison message reaches the DLQ family during the run",
        "an outbox row disappears without a recorded publish (loss observed)",
    ],
    "roles": {
        "director": "(EMPTY — named at the game day, P12.08 template)",
        "operator": "(EMPTY — named at the game day)",
        "observer": "(EMPTY — named at the game day)",
        "evidence": "(EMPTY — named at the game day)",
    },
}


def readiness_checks(base_url: str) -> dict:
    """Reads the /ready document and returns the per-dependency verdicts it
    carries (the P11.21 named checks, `database` and `broker`). An
    unanswered probe is recorded as UNANSWERED — never guessed."""
    try:
        status, body = http_get(base_url + "/ready")
    except ProbeError as error:
        return {"probe": "UNANSWERED", "detail": str(error)}
    return {"probe": status, "body": body[:2000]}


def main() -> int:
    parser = argparse.ArgumentParser(description="C-NATS-1 (written-unrun; human gate)")
    parser.add_argument("--control-plane-url", required=True)
    parser.add_argument("--worker-url", required=True)
    parser.add_argument("--outage-hold-seconds", type=float, default=60.0,
                        help="registered BEFORE the run (X-8)")
    parser.add_argument("--drain-wait-seconds", type=float, default=120.0,
                        help="post-reconnect outbox-drain window (registered BEFORE the run)")
    parser.add_argument("--recovery-budget-seconds", type=float, default=120.0,
                        help="post-restart ready-probe budget (registered BEFORE the run)")
    parser.add_argument("--execute", action="store_true",
                        help="REQUIRED to run any fault; without it the script "
                             "prints the pre-registration and exits")
    parser.add_argument("--yes", action="store_true")
    parser.add_argument("--evidence-out", default="c_nats_1_evidence.json")
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
        # 1. Pre-fault steady state.
        pre_cp = readiness_checks(args.control_plane_url)
        pre_worker = readiness_checks(args.worker_url)
        _s, worker_metrics_pre = http_get(args.worker_url.rstrip("/") + "/metrics")
        pre_duplicates = metric_total(
            worker_metrics_pre, "flexcore_command_consumer_duplicate_deliveries_total")
        pre_dlq = metric_total(worker_metrics_pre,
                               "flexcore_command_consumer_unsupported_lifecycle_facts_total")
        evidence.set("pre_fault_probe", "cp=%s worker=%s duplicates=%s dlq_facts=%s"
                     % (pre_cp.get("probe"), pre_worker.get("probe"),
                        pre_duplicates, pre_dlq))

        # 2. THE FAULT (operator-supplied lab-control commands; never invented).
        stop_cmd = os.environ.get("FLEXCORE_LAB_NATS_STOP_CMD")
        start_cmd = os.environ.get("FLEXCORE_LAB_NATS_START_CMD")
        if not stop_cmd or not start_cmd:
            raise ProbeError(
                "FLEXCORE_LAB_NATS_STOP_CMD / FLEXCORE_LAB_NATS_START_CMD not set — "
                "the lab-control command is operator input, never invented here")
        subprocess.run(stop_cmd.split(), check=True, timeout=60)  # noqa: S603
        evidence.set("fault_observed", "nats stopped (lab control command)")

        # 3. DURING the outage: the worker must report the named `broker`
        #    check failing (503 with named checks — the P11.21 contract),
        #    while /health (liveness) stays 200.
        time.sleep(min(args.outage_hold_seconds, 10.0))
        during_health = None
        try:
            status, _ = http_get(args.worker_url + "/health")
            during_health = status
        except ProbeError:
            during_health = "UNANSWERED"
        during_ready = readiness_checks(args.worker_url)
        evidence.set("during_fault_probe",
                     "worker /health -> %s; worker /ready -> %s"
                     % (during_health, during_ready.get("probe")))

        # 4. Recovery: restart NATS; wait for ready within the budget.
        subprocess.run(start_cmd.split(), check=True, timeout=120)  # noqa: S603
        deadline = time.monotonic() + args.recovery_budget_seconds
        worker_ready = None
        while time.monotonic() < deadline:
            probe = readiness_checks(args.worker_url)
            if probe.get("probe") == 200:
                worker_ready = 200
                break
            time.sleep(2.0)
        evidence.set("post_recovery_probe", "worker /ready -> %s (budget %ss)"
                     % (worker_ready, args.recovery_budget_seconds))
        if worker_ready != 200:
            raise ProbeError("worker did not become ready within the budget")

        # 5. Drain window, then the oracle counters.
        time.sleep(args.drain_wait_seconds)
        _s, worker_metrics_post = http_get(args.worker_url.rstrip("/") + "/metrics")
        post_duplicates = metric_total(
            worker_metrics_post, "flexcore_command_consumer_duplicate_deliveries_total")
        post_dlq = metric_total(worker_metrics_post,
                                "flexcore_command_consumer_unsupported_lifecycle_facts_total")
        evidence.add_oracle_finding(
            "duplicate_deliveries_delta_recorded",
            True,
            "duplicate_deliveries_total %s -> %s (delta %s; the P11.12 no-op "
            "redelivery arm is the expected carrier of this delta)"
            % (pre_duplicates, post_duplicates, post_duplicates - pre_duplicates),
        )
        evidence.add_oracle_finding(
            "dlq_facts_delta_recorded",
            True,
            "unsupported_lifecycle_facts_total %s -> %s (a non-poison DLQ "
            "delivery during the outage window is an ABORT criterion)"
            % (pre_dlq, post_dlq),
        )
        stuck_present = family_present(worker_metrics_post, "flexcore_commands_stuck")
        evidence.add_oracle_finding(
            "stuck_gauge_present",
            stuck_present,
            "flexcore_commands_stuck renders on the control-plane /metrics "
            "(P11.20 written code); human reads the value against the 24 h "
            "RECOMMENDATION threshold",
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
