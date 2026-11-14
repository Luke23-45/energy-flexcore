# P5.16 — The FLEXCORE/CONTROL federate: the federation's control-side
#         leg (the FlexCore/control federate of P5.16 step 1).
#
# ══════════════════════════════════════════════════════════════════════════
# HONESTY HEADER — read `federate_common.py` first.
#
# WHAT THIS FEDERATE IS:
#   At each HELICS-granted time step it publishes the ACTIVE-POWER
#   SETPOINTS of the pinned schedule fixture (`options.setpoints_path` —
#   a JSON object asset_key → list of {from_seconds, p_mw} intervals,
#   the same fixture discipline the scenario-run composition uses) and
#   records the measurements it receives from the asset and grid
#   federates. It is the "FlexCore/control federate" of the
#   three-federate minimum (P5.16 step 1).
#
# WHAT THIS FEDERATE IS NOT:
#   - It contains NO optimization and NO physics. The setpoints come
#     from a PRE-COMPUTED schedule fixture, exactly as the scenario-run
#     composition keeps the re-solve at the optimizer-port seam
#     (D-P5.14 context in `14_15_adapters.md`); there is no optimizer
#     behind this script. HELICS coordinates the exchange; it does not
#     replace the physics (§12.4, §19.1).
#   - It is NOT a dispatch path. Setpoints published here are federation
#     VALUES for co-simulation — they are not commands, they carry no
#     command lifecycle state, and nothing is issued to any device
#     (simulation-only, §21.4; the command lifecycle and its gates live
#     in the platform, untouched by this federate).
#   - Control authority: this federate exists only inside a simulated
#     federation. It can never select or raise a control-authority mode
#     (§21.4, §5.7 — mode is a deployment/per-site setting).
#
# MAPPING ASSUMPTIONS (recorded, UNVERIFIED):
#   - `options.setpoints_path` holds the setpoint fixture (asset_key →
#     [{from_seconds, p_mw}, ...]); at federate time t the ACTIVE
#     interval is the last one with from_seconds <= t; a key with no
#     interval covering t publishes NOTHING (absence, never a fabricated
#     setpoint) and counts the skip.
#   - Subscription topics carry the ids verbatim
#     (`flexcore/asset/<asset_key>/p_mw` measurements;
#     `flexcore/grid/bus/<bus_id>/vm_pu` voltages); any topic not of a
#     recognized form is a fail-closed config error.
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


def load_setpoints(path: str) -> dict:
    try:
        with open(path, "r", encoding="utf-8") as handle:
            fixture = json.load(handle)
    except OSError as error:
        fail("the setpoint fixture could not be read: %s" % error)
        return {}
    except json.JSONDecodeError as error:
        fail("the setpoint fixture is not a valid JSON document: %s" % error)
        return {}
    if not isinstance(fixture, dict) or not fixture:
        fail("the setpoint fixture is not a non-empty JSON object (asset_key → intervals)")
    for key, intervals in fixture.items():
        if not isinstance(intervals, list) or not intervals:
            fail("the setpoint fixture's entry for %r is not a non-empty interval list" % key)
        for interval in intervals:
            if not isinstance(interval, dict) or "from_seconds" not in interval or "p_mw" not in interval:
                fail("the setpoint fixture's entry for %r has an interval missing from_seconds/p_mw" % key)
    return fixture


def active_setpoint(intervals: list, t: float) -> float | None:
    """The last interval with from_seconds <= t (the fixture's flat
    discipline), or None when none covers t."""
    active = None
    for interval in intervals:
        if interval["from_seconds"] <= t:
            active = interval["p_mw"]
    return active


def main() -> None:
    parser = argparse.ArgumentParser(
        description="FlexCore/control HELICS federate for the Phase 5 co-simulation (P5.16)."
    )
    parser.add_argument("--federation-config", required=True)
    parser.add_argument("--federate", required=True)
    args = parser.parse_args()

    config = load_config(args.federation_config)
    section = section_for(config, args.federate)
    if section.get("role") != "control":
        fail("federate %r is not declared with role 'control'" % args.federate)
    options = section.get("options", {})
    setpoints_path = options.get("setpoints_path")
    if not isinstance(setpoints_path, str) or not setpoints_path:
        fail("the control federate requires options.setpoints_path (the pinned schedule fixture)")

    setpoints = load_setpoints(setpoints_path)

    h = import_helics()

    federate = h.helicsCreateValueFederate(args.federate)
    broker_address = config.get("broker", {}).get("address", "")
    h.helicsFederateSetCoreInitString(federate, "--broker=%s" % broker_address)

    period = config.get("time_step_seconds")
    stop_time = config.get("stop_time_seconds")
    h.helicsFederateSetTimeProperty(federate, h.HELICS_PROPERTY_TIME_PERIOD, period)

    # Publications: setpoints of the form
    # `flexcore/control/<asset_key>/setpoint_p_mw` (MW, injection-
    # positive). Each must name an asset key the fixture covers.
    publications: list = []
    for endpoint in section.get("publications", []):
        topic = endpoint["topic"]
        if not topic.startswith("flexcore/control/") or not topic.endswith("/setpoint_p_mw"):
            fail("publication topic %r is not a setpoint_p_mw publication form" % topic)
        asset_key = topic[len("flexcore/control/"):-len("/setpoint_p_mw")]
        if asset_key not in setpoints:
            fail("publication topic %r names an asset key absent from the setpoint fixture" % topic)
        publications.append(
            (asset_key, h.helicsFederateRegisterPublication(
                federate, topic, h.HELICS_DATA_TYPE_DOUBLE, endpoint["unit"]))
        )

    # Subscriptions: measurements (asset P, bus voltages), recorded —
    # they are federation VALUES, never a control input to an optimizer
    # (there is no optimizer here).
    subscriptions: list = []
    for endpoint in section.get("subscriptions", []):
        topic = endpoint["topic"]
        if not (topic.startswith("flexcore/asset/") or topic.startswith("flexcore/grid/")):
            fail("subscription topic %r is neither an asset nor a grid publication form" % topic)
        subscriptions.append(
            (topic, h.helicsFederateRegisterSubscription(federate, topic, endpoint["unit"]))
        )

    h.helicsFederateEnterExecutingMode(federate)

    granted = 0.0
    steps = 0
    published_values = 0
    received_values = 0
    skipped_publications = 0

    while granted < stop_time - 1e-9:
        granted = h.helicsFederateRequestTime(federate, granted + period)
        steps += 1

        for asset_key, pub in publications:
            value = active_setpoint(setpoints[asset_key], granted)
            if value is None:
                skipped_publications += 1
                continue
            h.helicsPublicationPublishDouble(pub, float(value))
            published_values += 1

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
            role="control",
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
