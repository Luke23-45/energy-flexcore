# P8.18 — The FLEXCORE/RESEARCH-CONTROLLER federate: the research-only
#         federation leg (proposals in, measurements out — nothing else).
#
# ══════════════════════════════════════════════════════════════════════════
# HONESTY HEADER — read `federate_common.py` first.
#
# WHAT THIS FEDERATE IS:
#   A RESEARCH-ONLY value federate. At each HELICS-granted time step it
#   publishes the PROPOSAL active-power values of a pre-computed research-
#   proposals fixture (`options.proposals_path` — a JSON object asset_key
#   → list of {from_seconds, p_mw} intervals, the SAME fixture discipline
#   the Phase 5 control federate uses for setpoints) and records the
#   measurements it receives. It is the research leg added to the P5.16
#   three-federate minimum for the multi-domain interaction track (§19.1):
#   a research proposal's DELIVERY to the simulated grid — including any
#   communication-domain effect a federation models — is observed here as
#   data.
#
# WHAT THIS FEDERATE IS NOT:
#   - It contains NO research intelligence and NO physics: the proposals
#     come from a PRE-COMPUTED fixture (an ADMM/DMPC/GNN/MARL module's
#     recorded output, or a placeholder the operator pastes), exactly as
#     the control federate's setpoints come from a schedule fixture. There
#     is no optimizer, no MARL policy, and no power flow behind this
#     script. HELICS coordinates the exchange; it is TRANSPORT, not the
#     physics model (§12.4, §19.1).
#   - It is NOT a dispatch path and has NO FIELD-WRITE PATH. Proposal
#     values published on `flexcore/research/...` topics are federation
#     VALUES for co-simulation research: they are not commands, carry no
#     command lifecycle state, enter no validation→feasibility→
#     authorization gate, and can reach NO device — this script's only
#     output channel is the HELICS publication + the exit report on
#     stdout. A research proposal enters the protected path (if ever) as
#     a PROPOSAL through the platform's gates, never through this
#     federate (ADR-010, §13.4).
#   - Control authority: MODE-LOCKED to simulation/shadow (§21.4). The
#     mode is a recorded constant below, not a setting: no request, no
#     plugin, and no research module can select or raise a
#     control-authority mode (§5.7).
#
# MAPPING ASSUMPTIONS (recorded, UNVERIFIED — same discipline as
# `control_federate.py`):
#   - `options.proposals_path` holds the proposals fixture (asset_key →
#     [{from_seconds, p_mw}, ...]); at federate time t the ACTIVE interval
#     is the last one with from_seconds <= t; a key with no interval
#     covering t publishes NOTHING (absence, never a fabricated proposal)
#     and counts the skip.
#   - Publication topics carry the form
#     `flexcore/research/<asset_key>/proposal_p_mw` — a DISTINCT topic
#     family from the control federate's `flexcore/control/.../setpoint_p_mw`
#     so a research proposal can never be mistaken for (or silently
#     substituted into) a control setpoint stream. Subscription topics
#     carry the ids verbatim (asset/grid measurement forms); any topic not
#     of a recognized form is a fail-closed config error.
#
# CONTRACT SEAM (recorded, not invented — a Phase 7 follow-up in
# `18_helics_cosim.md` §7): the P5.16 Rust orchestrator
# (`apps/sim-runner/src/helics_orchestration.rs`) validates the federation
# config against THREE roles (asset/grid/control) and its canonical
# publication families; a `research_controller` role is NOT among them, so
# running this federate under that orchestrator requires an ADDITIVE role
# variant there. This script therefore validates its own config section
# (fail-closed, reusing `federate_common.py`) and no sim-runner seam was
# added in this step: the ledger's P8.18 deliverable names the federation
# script + scenario run record + boundary statement, not an orchestrator
# change, and a contract gap is recorded, never papered over.
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

# The research proposal publication form (topic prefix and suffix).
RESEARCH_TOPIC_PREFIX = "flexcore/research/"
RESEARCH_TOPIC_SUFFIX = "/proposal_p_mw"

# The measurement topic families this federate may subscribe to (the
# existing federates' publication forms; anything else is a config error).
MEASUREMENT_TOPIC_PREFIXES = ("flexcore/asset/", "flexcore/grid/")

# MODE LOCK (§21.4): a constant, never a setting. The exit report carries
# it so no run record can lose the federation's control posture.
MODE_LOCKED = "simulation-shadow"

RESEARCH_NOTE = (
    "P8.18 research-only federate: publishes PRE-COMPUTED research "
    "PROPOSALS (ADR-010) on flexcore/research topics and records "
    "received measurements; NO field-write path exists in this script, "
    "no command lifecycle state is touched, and proposals enter no "
    "authorization gate here. HELICS is TRANSPORT: it synchronizes "
    "federates and carries values; it is NOT the power-system physics "
    "model (report 12.4, 19.1)."
)


def load_proposals(path: str) -> dict:
    """Loads and validates the proposals fixture (the control federate's
    setpoint-fixture discipline, applied to proposals). Fail-closed."""
    try:
        with open(path, "r", encoding="utf-8") as handle:
            fixture = json.load(handle)
    except OSError as error:
        fail("the proposals fixture could not be read: %s" % error)
        return {}
    except json.JSONDecodeError as error:
        fail("the proposals fixture is not a valid JSON document: %s" % error)
        return {}
    if not isinstance(fixture, dict) or not fixture:
        fail("the proposals fixture is not a non-empty JSON object (asset_key → intervals)")
    for key, intervals in fixture.items():
        if not isinstance(intervals, list) or not intervals:
            fail("the proposals fixture's entry for %r is not a non-empty interval list" % key)
        for interval in intervals:
            if not isinstance(interval, dict) or "from_seconds" not in interval or "p_mw" not in interval:
                fail("the proposals fixture's entry for %r has an interval missing from_seconds/p_mw" % key)
    return fixture


def active_proposal(intervals: list, t: float) -> float | None:
    """The last interval with from_seconds <= t (the fixture's flat
    discipline), or None when none covers t (absence, never a fabricated
    proposal)."""
    active = None
    for interval in intervals:
        if interval["from_seconds"] <= t:
            active = interval["p_mw"]
    return active


def main() -> None:
    parser = argparse.ArgumentParser(
        description=(
            "FlexCore/research-controller HELICS federate for the Phase 8 "
            "multi-domain research federation (P8.18). Research-only: "
            "proposals out, measurements in, no field write."
        )
    )
    parser.add_argument("--federation-config", required=True)
    parser.add_argument("--federate", required=True)
    args = parser.parse_args()

    config = load_config(args.federation_config)
    section = section_for(config, args.federate)
    if section.get("role") != "research_controller":
        fail("federate %r is not declared with role 'research_controller'" % args.federate)
    options = section.get("options", {})
    proposals_path = options.get("proposals_path")
    if not isinstance(proposals_path, str) or not proposals_path:
        fail("the research-controller federate requires options.proposals_path (the pre-computed proposals fixture)")

    proposals = load_proposals(proposals_path)

    h = import_helics()

    federate = h.helicsCreateValueFederate(args.federate)
    broker_address = config.get("broker", {}).get("address", "")
    h.helicsFederateSetCoreInitString(federate, "--broker=%s" % broker_address)

    period = config.get("time_step_seconds")
    stop_time = config.get("stop_time_seconds")
    h.helicsFederateSetTimeProperty(federate, h.HELICS_PROPERTY_TIME_PERIOD, period)

    # Publications: research PROPOSALS of the form
    # `flexcore/research/<asset_key>/proposal_p_mw` (MW, injection-
    # positive, P > 0 = export at the point of connection, §10.2). Each
    # must name an asset key the fixture covers, and MUST NOT reuse the
    # control federate's setpoint family.
    publications: list = []
    for endpoint in section.get("publications", []):
        topic = endpoint["topic"]
        if not topic.startswith(RESEARCH_TOPIC_PREFIX) or not topic.endswith(RESEARCH_TOPIC_SUFFIX):
            fail("publication topic %r is not a research proposal_p_mw publication form" % topic)
        asset_key = topic[len(RESEARCH_TOPIC_PREFIX):-len(RESEARCH_TOPIC_SUFFIX)]
        if not asset_key:
            fail("publication topic %r has an empty asset key" % topic)
        if asset_key not in proposals:
            fail("publication topic %r names an asset key absent from the proposals fixture" % topic)
        publications.append(
            (asset_key, h.helicsFederateRegisterPublication(
                federate, topic, h.HELICS_DATA_TYPE_DOUBLE, endpoint["unit"]))
        )

    # Subscriptions: measurements (asset P/Q, bus voltages, branch
    # loadings), recorded as federation VALUES for the research record —
    # they are never a dispatch or authorization input (there is no
    # dispatch or authorization here).
    subscriptions: list = []
    for endpoint in section.get("subscriptions", []):
        topic = endpoint["topic"]
        if not any(topic.startswith(prefix) for prefix in MEASUREMENT_TOPIC_PREFIXES):
            fail("subscription topic %r is neither an asset nor a grid measurement form" % topic)
        subscriptions.append(
            (topic, h.helicsFederateRegisterSubscription(federate, topic, endpoint["unit"]))
        )

    h.helicsFederateEnterExecutingMode(federate)

    granted = 0.0
    steps = 0
    published_proposals = 0
    received_values = 0
    skipped_proposals = 0

    while granted < stop_time - 1e-9:
        granted = h.helicsFederateRequestTime(federate, granted + period)
        steps += 1

        for asset_key, pub in publications:
            value = active_proposal(proposals[asset_key], granted)
            if value is None:
                skipped_proposals += 1
                continue
            h.helicsPublicationPublishDouble(pub, float(value))
            published_proposals += 1

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

    report = federate_report(
        federation=config.get("federation_name", ""),
        federate=args.federate,
        role="research_controller",
        granted_steps=steps,
        final_granted_time_seconds=granted,
        published_values=published_proposals,
        received_values=received_values,
        skipped_publications=skipped_proposals,
        helics_version_reported=helics_version(h),
    )
    # Additive research-posture fields (the shared report builder's shape
    # is untouched; the research posture travels with every run record).
    report["published_proposals"] = published_proposals
    report["skipped_proposals"] = skipped_proposals
    report["control_authority_mode_locked"] = MODE_LOCKED
    report["field_write_path"] = "absent"
    report["proposal_only"] = True
    report["research_note"] = RESEARCH_NOTE
    emit_report(report)


if __name__ == "__main__":
    main()
