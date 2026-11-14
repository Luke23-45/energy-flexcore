# P5.16 — The HELICS broker stand-alone script: creates the federation's
#         broker IN-PROCESS (pyhelics), supervises it until the federates
#         disconnect, and emits the broker exit report.
#
# ══════════════════════════════════════════════════════════════════════════
# HONESTY HEADER — read `federate_common.py` first:
#
# 1. WRITTEN, NEVER EXECUTED; helics==3.7.0 NEVER installed; every helics
#    call UNVERIFIED (the table in `federate_common.py` covers this script's
#    rows: helicsCreateBroker, helicsBrokerIsConnected, teardown).
# 2. The broker is COORDINATION ONLY. It carries no values and knows no
#    physics; HELICS coordinates federates, it does not replace their
#    physics models (§12.4, §19.1).
# 3. FAIL-CLOSED: no helics, malformed config, or broker-level error ⇒
#    nothing on stdout + stderr reason + exit 2 (no false exit report).
# 4. Supervision-loop ASSUMPTIONS (recorded, UNVERIFIED):
#    - the broker is created for exactly `broker.expected_federates`
#      federates (the `-f` init flag — documented broker init form);
#    - `helicsBrokerIsConnected` returns True while the federation lives
#      and False once every federate has disconnected (the loop exits on
#      either that transition or the overall timeout, whichever first);
#    - a SIGTERM/SIGINT triggers the fail-closed teardown (no half-alive
#      broker left behind).
# ══════════════════════════════════════════════════════════════════════════

from __future__ import annotations

import argparse
import signal
import time

from federate_common import (
    PINNED_HELICS_VERSION,
    REPORT_KIND_BROKER,
    emit_report,
    fail,
    helics_version,
    import_helics,
    load_config,
)

# The overall supervision bound (wall clock — the broker is REAL
# infrastructure, not a scenario-clock component; its timeout is
# deployment configuration, defaulting to the orchestrator's bound).
DEFAULT_SUPERVISION_TIMEOUT_SECONDS = 120.0


def main() -> None:
    parser = argparse.ArgumentParser(
        description="HELICS broker for the FlexCore Phase 5 co-simulation federation (P5.16)."
    )
    parser.add_argument("--federation-config", required=True)
    parser.add_argument(
        "--timeout-seconds",
        type=float,
        default=DEFAULT_SUPERVISION_TIMEOUT_SECONDS,
        help="overall supervision bound (fail-closed on expiry)",
    )
    args = parser.parse_args()

    config = load_config(args.federation_config)
    broker_spec = config.get("broker", {})
    expected_federates = broker_spec.get("expected_federates")
    if not isinstance(expected_federates, int) or expected_federates < 1:
        fail("the config's broker.expected_federates is missing or invalid")

    h = import_helics()

    # SIGTERM/SIGINT → fail-closed teardown (no report, non-zero exit).
    def _terminate(signum, _frame):  # noqa: ANN001 — signal-handler signature
        try:
            h.helicsBrokerDisconnect(broker_holder[0])
        except Exception:  # noqa: BLE001, S110 — teardown best-effort, exit stays non-zero
            pass
        fail("the broker received signal %d and tore down fail-closed" % signum)

    signal.signal(signal.SIGTERM, _terminate)
    signal.signal(signal.SIGINT, _terminate)

    broker_holder: list = []
    # Documented broker init: `-f <count>` declares the expected federate
    # count; the address comes from the orchestrator's deployment config
    # via the broker's own CLI/environment in a real deployment. The
    # in-process broker binds the address recorded in the config (the
    # `--broker_address` init form is an ASSUMPTION — UNVERIFIED).
    broker_holder.append(
        h.helicsCreateBroker(
            "zmq",
            "",
            "-f %d --broker_address=%s --loglevel=warning"
            % (expected_federates, broker_spec.get("address", "")),
        )
    )
    if not h.helicsBrokerIsConnected(broker_holder[0]):
        fail("the broker could not connect (address %r)" % broker_spec.get("address"))

    # Supervision loop: exit when the federation completes (every
    # federate disconnected) or the timeout expires — whichever first.
    deadline = time.monotonic() + args.timeout_seconds
    while True:
        if not h.helicsBrokerIsConnected(broker_holder[0]):
            break
        if time.monotonic() >= deadline:
            h.helicsBrokerDisconnect(broker_holder[0])
            fail(
                "the broker supervision timeout (%.1fs) expired before the federation "
                "completed — fail-closed, never a partial-federation pass"
                % args.timeout_seconds
            )
        time.sleep(0.05)

    version = helics_version(h)
    h.helicsBrokerDisconnect(broker_holder[0])
    try:
        h.helicsBrokerFree(broker_holder[0])
    except Exception:  # noqa: BLE001 — teardown best-effort; the report is still emitted
        pass
    try:
        h.helicsCloseLibrary()
    except Exception:  # noqa: BLE001
        pass

    emit_report(
        {
            "protocol": "flexcore-helics-federate/1",
            "kind": REPORT_KIND_BROKER,
            "federation": config.get("federation_name", ""),
            "federate": "broker",
            "role": None,
            "granted_steps": None,
            "final_granted_time_seconds": None,
            "published_values": None,
            "received_values": None,
            "skipped_publications": None,
            "ok": True,
            "status_text": (
                "HELICS broker for federation %r: coordination only — it carries no values "
                "and knows no physics (report 12.4, 19.1); the broker's helics API usage is "
                "UNVERIFIED until the human build/test pass (pin %s)."
                % (config.get("federation_name", ""), PINNED_HELICS_VERSION)
            ),
            "helics_version_reported": version,
        }
    )


if __name__ == "__main__":
    main()
