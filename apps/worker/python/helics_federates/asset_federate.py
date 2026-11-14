# P5.16 — The ASSET/END-USE federate: the federation's bridge to the
#         EXISTING Phase 2 asset simulator, via the simulator's own
#         canonical replay records.
#
# ══════════════════════════════════════════════════════════════════════════
# HONESTY HEADER — read `federate_common.py` first.
#
# WHAT THIS FEDERATE IS:
#   At each HELICS-granted time step it publishes the asset measurements
#   the Phase 2 simulator produced — read from the canonical replay
#   records file (`options.input_records_path`, the `ReplayRecord` JSON
#   array `flexcore-sim-runner run --out` emits) — and it records the
#   setpoints it receives from the control federate. It IS the
#   "asset/end-use federate" of the report's three-federate minimum
#   (P5.16 step 1).
#
# WHAT THIS FEDERATE IS NOT:
#   - It contains NO physics. The behaviour profiles stay in the Phase 2
#     simulator (`crates/flexcore-simulator`); this script only replays
#     the records the simulator already produced on the scenario clock
#     and maps event_time_unix_ms → federate time. HELICS coordinates the
#     exchange; it does not replace the physics (§12.4, §19.1).
#   - It is NOT a live dispatch path. Setpoints received here are
#     RECORDED for the run record; issuing commands to real devices does
#     not exist in the 0.x line (simulation-only, §21.4).
#
# DATA MAPPING (recorded assumptions):
#   - A record qualifies for a topic when its `quantity` equals the
#     quantity the topic declares (topic form
#     `flexcore/asset/<asset_key>/<quantity>`) and its `asset_id` is the
#     stand-in UUID for that asset key — the records file carries UUIDs,
#     so the federate matches by the TOPIC's declared asset key only for
#     topics whose declared key appears in the records' `asset_id`-derived
#     key map (`options.asset_key_map`, an object asset_key → asset UUID;
#     a topic whose key is absent from the map is a config error,
#     fail-closed).
#   - Only `quality` == "GOOD" or "SUSPECT" records are published (the
#     same usable-sample rule the platform applies everywhere); a BAD/
#     MISSING/ESTIMATED/STALE sample is skipped and COUNTED
#     (skipped_publications), never coerced (the P5.14 tracking-quality
#     discipline).
#   - A step with no qualifying sample for a topic publishes NOTHING for
#     it that step (absence, never a fabricated value — the `drop`-fault
#     discipline) and counts the skip.
#
# Every helics call: UNVERIFIED (table in `federate_common.py`).
# ══════════════════════════════════════════════════════════════════════════

from __future__ import annotations

import argparse
import json

from federate_common import (
    emit_report,
    fail,
    federate_report,
    helics_version,
    import_helics,
    load_config,
    section_for,
)

USABLE_QUALITIES = ("GOOD", "SUSPECT")


def load_records(path: str) -> list:
    try:
        with open(path, "r", encoding="utf-8") as handle:
            records = json.load(handle)
    except OSError as error:
        fail("the input records file could not be read: %s" % error)
        return []
    except json.JSONDecodeError as error:
        fail("the input records file is not a valid JSON document: %s" % error)
        return []
    if not isinstance(records, list):
        fail("the input records file is not a JSON array of records")
    return records


def topic_declares(topic: str, prefix: str) -> tuple[str, str] | None:
    """`flexcore/asset/<key>/<quantity>` → (key, quantity), or None when
    the topic is not of the asset form."""
    if not topic.startswith(prefix):
        return None
    remainder = topic[len(prefix):]
    parts = remainder.rsplit("/", 1)
    if len(parts) != 2 or not parts[0] or not parts[1]:
        return None
    return parts[0], parts[1]


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Asset/end-use HELICS federate for the FlexCore Phase 5 co-simulation (P5.16)."
    )
    parser.add_argument("--federation-config", required=True)
    parser.add_argument("--federate", required=True)
    args = parser.parse_args()

    config = load_config(args.federation_config)
    section = section_for(config, args.federate)
    if section.get("role") != "asset":
        fail("federate %r is not declared with role 'asset'" % args.federate)
    options = section.get("options", {})
    records_path = options.get("input_records_path")
    if not isinstance(records_path, str) or not records_path:
        fail("the asset federate requires options.input_records_path (the Phase 2 simulator's records)")
    key_map = options.get("asset_key_map")
    if not isinstance(key_map, dict) or not key_map:
        fail("the asset federate requires options.asset_key_map (asset_key → asset UUID)")

    records = load_records(records_path)

    h = import_helics()

    federate = h.helicsCreateValueFederate(args.federate)
    broker_address = config.get("broker", {}).get("address", "")
    h.helicsFederateSetCoreInitString(federate, "--broker=%s" % broker_address)

    period = config.get("time_step_seconds")
    stop_time = config.get("stop_time_seconds")
    h.helicsFederateSetTimeProperty(federate, h.HELICS_PROPERTY_TIME_PERIOD, period)

    # Publications: one per configured publication topic, matched to the
    # records feed by (asset key, quantity). A topic whose declaration
    # does not resolve into the records feed is a fail-closed config
    # error BEFORE any federation time advances.
    publications: list = []
    for endpoint in section.get("publications", []):
        declared = topic_declares(endpoint["topic"], "flexcore/asset/")
        if declared is None:
            fail("publication topic %r is not of the flexcore/asset/<key>/<quantity> form" % endpoint["topic"])
        asset_key, quantity = declared
        if asset_key not in key_map:
            fail("publication topic %r names an asset key absent from options.asset_key_map" % endpoint["topic"])
        publications.append(
            (endpoint["topic"], asset_key, quantity, h.helicsFederateRegisterPublication(
                federate, endpoint["topic"], h.HELICS_DATA_TYPE_DOUBLE, endpoint["unit"]))
        )

    # Subscriptions: the control federate's setpoints (recorded, never a
    # live dispatch path).
    subscriptions: list = []
    for endpoint in section.get("subscriptions", []):
        subscriptions.append(
            (endpoint["topic"], h.helicsFederateRegisterSubscription(federate, endpoint["topic"], endpoint["unit"]))
        )

    h.helicsFederateEnterExecutingMode(federate)

    granted = 0.0
    steps = 0
    published_values = 0
    received_values = 0
    skipped_publications = 0
    window_end_ms = None

    # The scenario start instant is the earliest record's event time; a
    # federate step t covers the half-open window [start + (t-1)*step,
    # start + t*step) — event times are mapped, never regenerated.
    event_times = [r["event_time_unix_ms"] for r in records if isinstance(r, dict) and "event_time_unix_ms" in r]
    if not event_times:
        fail("the records feed carries no event_time_unix_ms values")
    start_ms = min(event_times)
    step_ms = int(round(period * 1000.0))

    while granted < stop_time - 1e-9:
        granted = h.helicsFederateRequestTime(federate, granted + period)
        steps += 1
        window_end_ms = start_ms + steps * step_ms

        # Publish: the newest usable record for each (key, quantity)
        # within this step's window.
        for topic, asset_key, quantity, pub in publications:
            candidate = None
            for record in records:
                if (
                    record.get("asset_id") == key_map[asset_key]
                    and record.get("quantity") == quantity
                    and start_ms + (steps - 1) * step_ms <= record.get("event_time_unix_ms", -1) < window_end_ms
                    and record.get("quality") in USABLE_QUALITIES
                ):
                    candidate = record  # records arrive in sequence order; the last match in-window is the newest
            if candidate is None:
                skipped_publications += 1
                continue
            h.helicsPublicationPublishDouble(pub, float(candidate["value"]))
            published_values += 1

        # Receive: record the setpoints offered this step (counted; the
        # accepted/rejected decision stays on the platform side, §14.1).
        for _topic, sub in subscriptions:
            h.helicsInputGetDouble(sub)
            received_values += 1

    h.helicsFederateDisconnect(federate)
    try:
        h.helicsFederateFree(federate)
    except Exception:  # noqa: BLE001 — teardown best-effort
        pass
    try:
        h.helicsCloseLibrary()
    except Exception:  # noqa: BLE001
        pass

    emit_report(
        federate_report(
            federation=config.get("federation_name", ""),
            federate=args.federate,
            role="asset",
            granted_steps=steps,
            final_granted_time_seconds=granted,
            published_values=published_values,
            received_values=received_values,
            skipped_publications=skipped_publications,
            helics_version_reported=helics_version(h),
        )
    )


if __name__ == "__main__":
    main()
