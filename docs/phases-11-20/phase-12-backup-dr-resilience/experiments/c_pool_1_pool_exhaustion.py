# C-POOL-1 — Connection-pool exhaustion at the API edge (written-unrun).
#
# Pre-registration (the experiment's contract; P12.07 record
# records/21_p12_07_chaos_program.md):
#   Steady-state hypothesis: when the connection pool (and the API-edge
#     limiter budget) is exhausted, callers receive backpressure — 429
#     with Retry-After and the stable `rate_limited` envelope (the P11.14
#     contract) — NEVER hangs, and nothing queues unboundedly.
#   Anchoring: P11.02 SLO register records/02 (SLO 1 availability, SLO 2
#     API latency ≤ 1000 ms RECOMMENDATION); the P11.14 limiter seam
#     (apps/control-plane/src/rate_limit.rs — 429/Retry-After contract,
#     per-tenant in-flight caps, `flexcore_http_requests_total{status="429"}`
#     counting); the P12.06 PgBouncer transaction-mode pooling design
#     (Agent A's record, records/06_p12_06_pgbouncer_introduction.md —
#     reconciled from the expected-path sibling citation, plan §2.1).
#   Blast radius: the API edge and the pool of ONE lab instance only;
#     one synthetic tenant drives the load; the limiter is in-process
#     per-instance (the P11.14 §4 scale-out honesty note) so the
#     experiment assumes the single-instance lab shape.
#   Fault: concurrent load over the governed command-submission POSTs
#     (routes::COMMAND_SUBMISSION_PATHS — the candidate-submission class)
#     beyond the registered class budget, WITHOUT any container fault.
#     The pool-exhaustion leg (if the P12.06 design lands a pool cap the
#     lab exercises) adds the pooled-connection ceiling via the lab
#     profile, never improvised here.
#   In-code oracle: apps/control-plane/src/rate_limit.rs (429 +
#     Retry-After ≥ 1 s + `rate_limited` envelope;
#     flexcore_http_requests_total{status="429"}); /ready stays 200
#     (probes are NEVER rate-limited — the P11.14 contract row).
#   Abort criteria: any caller observed HANGING beyond the registered
#     probe timeout instead of receiving 429; /health or /ready or
#     /metrics rate-limited (the never-limited contract violated as
#     observed); 5xx storms replacing the 429 contract.
#
# HONESTY: WRITTEN, NEVER RUN. Evidence cells start EMPTY. The script
# opens NO sockets itself at write time; the load leg runs only under the
# human gate. §13.3 semantics apply to every zero-observation.
#
# Run (human gate ONLY):
#   python -I c_pool_1_pool_exhaustion.py --control-plane-url http://127.0.0.1:8080 \
#       --concurrency 64 --duration-seconds 30 --execute --yes \
#       --evidence-out c_pool_1_evidence.json

from __future__ import annotations

import argparse
import json
import sys
import threading
import time
import urllib.error
import urllib.request

from lab_harness import (
    Evidence, http_get, metric_total, print_preregistration, require_confirm,
)

EXPERIMENT = {
    "id": "C-POOL-1",
    "hypothesis": (
        "Under load beyond the registered class budget, callers receive 429 "
        "with Retry-After and the `rate_limited` envelope (the P11.14 "
        "contract) within the registered request timeout — never hangs; the "
        "probe endpoints (/health, /ready, /metrics) are never rate-limited; "
        "no unbounded queueing is observed."
    ),
    "anchoring": (
        "P11.02 SLO register records/02 (SLO 1, SLO 2 — RECOMMENDATION "
        "thresholds); P11.14 limiter (apps/control-plane/src/rate_limit.rs, "
        "records/26); P12.06 pooling design (Agent A, "
        "records/06_p12_06_pgbouncer_introduction.md — sibling citation "
        "reconciled, plan §2.1)."
    ),
    "blast_radius": [
        "the API edge + pool of ONE lab instance (the P11.14 in-process, "
        "single-instance limiter shape)",
        "one synthetic tenant; the governed candidate-submission POSTs only",
        "no container fault; load only",
    ],
    "fault": (
        "concurrent POSTs over the governed command-submission paths beyond "
        "the registered operator-class budget for the registered duration"
    ),
    "oracle": (
        "apps/control-plane/src/rate_limit.rs 429/Retry-After/rate_limited "
        "contract; flexcore_http_requests_total{status=\"429\"}; probe "
        "endpoints stay 200 (P11.14 never-limited row)"
    ),
    "abort_criteria": [
        "any caller hangs beyond the registered per-request timeout",
        "/health, /ready, or /metrics answers 429 (the never-limited row)",
        "5xx responses replace the 429 contract at the edge",
    ],
    "roles": {
        "director": "(EMPTY — named at the game day, P12.08 template)",
        "operator": "(EMPTY — named at the game day)",
        "observer": "(EMPTY — named at the game day)",
        "evidence": "(EMPTY — named at the game day)",
    },
}

# The governed candidate-submission class (the closed constant, verbatim
# scope from P11.11/P11.14 — routes::COMMAND_SUBMISSION_PATHS).
GOVERNED_PATHS = ("/api/v1/schedules", "/api/v1/flexibility/envelopes",
                  "/api/v1/optimization-runs")


def one_post(base_url: str, path: str, timeout_s: float) -> dict:
    """One governed POST with the mandatory Idempotency-Key (the P11.11
    contract makes the key mandatory — a run without it would measure the
    422 layer, not the limiter). Returns the outcome dict for the run's
    tally; NO verdict is computed here."""
    body = json.dumps({"probe": "c-pool-1"}).encode("utf-8")
    request = urllib.request.Request(
        base_url.rstrip("/") + path, data=body, method="POST",
        headers={"Content-Type": "application/json",
                 "Idempotency-Key": "c-pool-1-%d" % time.monotonic_ns()})
    started = time.monotonic()
    try:
        with urllib.request.urlopen(request, timeout=timeout_s) as response:
            return {"path": path, "status": response.status,
                    "retry_after": response.headers.get("Retry-After"),
                    "seconds": time.monotonic() - started}
    except urllib.error.HTTPError as error:
        return {"path": path, "status": error.code,
                "retry_after": error.headers.get("Retry-After"),
                "seconds": time.monotonic() - started}
    except OSError as error:
        return {"path": path, "status": "TRANSPORT_ERROR", "retry_after": None,
                "seconds": time.monotonic() - started,
                "error": "%s: %s" % (type(error).__name__, error)}


def main() -> int:
    parser = argparse.ArgumentParser(description="C-POOL-1 (written-unrun; human gate)")
    parser.add_argument("--control-plane-url", required=True)
    parser.add_argument("--concurrency", type=int, default=64,
                        help="registered BEFORE the run (X-8)")
    parser.add_argument("--duration-seconds", type=float, default=30.0,
                        help="registered BEFORE the run (X-8)")
    parser.add_argument("--request-timeout-seconds", type=float, default=10.0,
                        help="per-request timeout; a hang beyond it is an ABORT criterion")
    parser.add_argument("--execute", action="store_true",
                        help="REQUIRED to run the load; without it the script "
                             "prints the pre-registration and exits")
    parser.add_argument("--yes", action="store_true")
    parser.add_argument("--evidence-out", default="c_pool_1_evidence.json")
    args = parser.parse_args()

    print_preregistration(EXPERIMENT)
    if not args.execute:
        print("\n(no --execute: nothing ran — WRITTEN-UNRUN)")
        return 0

    evidence = Evidence(EXPERIMENT["id"])
    evidence.set("lab_profile", args.control_plane_url)
    if not require_confirm("Confirm ISOLATED lab, blast radius understood", args.yes):
        evidence.set("notes", "aborted at the keyboard gate; no load generated")
        print(evidence.write_json(args.evidence_out))
        return 2

    evidence.begin()
    try:
        # 1. Pre-fault: probe baseline + 429-series baseline.
        _s, metrics_pre = http_get(args.control_plane_url.rstrip("/") + "/metrics")
        pre_429 = metric_total(metrics_pre, "flexcore_http_requests_total",
                               {"status": "429"})
        evidence.set("pre_fault_probe", "429-series baseline: %s" % pre_429)

        # 2. THE LOAD (no container fault). Each worker thread stops at its
        #    first TRANSPORT_ERROR/hang observation and records it — a hang
        #    beyond the timeout IS the abort criterion, recorded verbatim.
        outcomes: list = []
        lock = threading.Lock()
        stop_flag = threading.Event()

        def load_worker(path: str) -> None:
            while not stop_flag.is_set():
                outcome = one_post(args.control_plane_url, path,
                                   args.request_timeout_seconds)
                with lock:
                    outcomes.append(outcome)
                if outcome["status"] == "TRANSPORT_ERROR":
                    stop_flag.set()
                    return

        threads = []
        deadline = time.monotonic() + args.duration_seconds
        for index in range(args.concurrency):
            thread = threading.Thread(
                target=load_worker,
                args=(GOVERNED_PATHS[index % len(GOVERNED_PATHS)],))
            thread.start()
            threads.append(thread)
        while time.monotonic() < deadline and not stop_flag.is_set():
            time.sleep(0.5)
        stop_flag.set()
        for thread in threads:
            thread.join(timeout=args.request_timeout_seconds + 5.0)
        evidence.set("fault_observed",
                     "load window done: %d requests attempted" % len(outcomes))

        # 3. Probe-endpoint oracle: probes are NEVER rate-limited.
        probe_verdicts = []
        for probe_path in ("/health", "/ready", "/metrics"):
            try:
                status, _ = http_get(args.control_plane_url.rstrip("/") + probe_path)
                probe_verdicts.append((probe_path, status))
            except Exception as error:  # noqa: BLE001 — recorded verbatim
                probe_verdicts.append((probe_path, "UNANSWERED: %s" % error))
        evidence.set("post_recovery_probe", "probes: %s" % probe_verdicts)

        # 4. The oracle tally (human judges against the contract).
        status_tally: dict = {}
        hangs = 0
        probe_429 = [p for p, s in probe_verdicts if s == 429]
        for outcome in outcomes:
            status_tally[str(outcome["status"])] = \
                status_tally.get(str(outcome["status"]), 0) + 1
            if outcome["status"] == "TRANSPORT_ERROR":
                hangs += 1
        retry_after_seen = sorted(
            {str(o["retry_after"]) for o in outcomes if o["retry_after"]})
        evidence.add_oracle_finding(
            "status_tally_recorded", True, json.dumps(status_tally, sort_keys=True))
        evidence.add_oracle_finding(
            "retry_after_values_seen", True,
            "Retry-After values observed: %s (contract: whole seconds, >= 1)"
            % (retry_after_seen or "none"))
        evidence.add_oracle_finding(
            "no_hangs_observed", hangs == 0,
            "transport errors / hangs beyond the timeout: %d" % hangs)
        evidence.add_oracle_finding(
            "probes_never_rate_limited", not probe_429,
            "probe endpoints answering 429: %s" % (probe_429 or "none"))
        _s, metrics_post = http_get(args.control_plane_url.rstrip("/") + "/metrics")
        post_429 = metric_total(metrics_post, "flexcore_http_requests_total",
                                {"status": "429"})
        evidence.add_oracle_finding(
            "four29_series_delta", True,
            "flexcore_http_requests_total{status=\"429\"} %s -> %s (delta %s)"
            % (pre_429, post_429, post_429 - pre_429))
        evidence.set("oracle_results", json.dumps(evidence.oracle_findings))
    except (ProbeError, OSError) as error:
        evidence.set("notes", "RUN ERROR (recorded verbatim): %s: %s"
                     % (type(error).__name__, error))
        evidence.set("abort_fired", "yes (error path)")
    finally:
        evidence.end()
        print(evidence.write_json(args.evidence_out))
    return 0


if __name__ == "__main__":  # pragma: no cover — human-gate entrypoint
    sys.exit(main())
