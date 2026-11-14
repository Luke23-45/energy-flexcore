# P5.16 — The GRID federate: the federation's bridge to the EXISTING
#         Phase 4 pandapower pipeline, via the UNMODIFIED P5.15 worker
#         protocol.
#
# ══════════════════════════════════════════════════════════════════════════
# HONESTY HEADER — read `federate_common.py` first.
#
# WHAT THIS FEDERATE IS:
#   At each HELICS-granted time step it collects the injection
#   publications (per asset: P in MW, Q in MVAr — injection-positive,
#   P > 0 export), translates them into ONE `flexcore-grid-worker/1`
#   request (the EXACT `WorkerRequest` shape of
#   `crates/flexcore-grid-api/src/worker_backend.rs`), spawns the
#   UNMODIFIED P5.15 worker (`options.grid_worker_script`) as a
#   subprocess, and publishes the outcome's bus voltages (pu) and branch
#   loading percentages back into the federation.
#
# WHAT THIS FEDERATE IS NOT:
#   - It contains NO power-flow logic. The AC power flow stays in the
#     Phase 4 pipeline (pandapower `runpp` inside the worker); this
#     script only re-uses the worker transport, one JSON document each
#     way, exactly as the Rust `WorkerProcessBackend` does. HELICS
#     coordinates the exchange; it does not replace the physics
#     (§12.4, §19.1). AC power flow ≠ AC-OPF (AGENTS.md rule 4).
#   - A completed co-simulation run is not a power-system validation by
#     itself (§19.1), and a steady-state power-flow result is never
#     transient-stability evidence.
#
# FAIL-CLOSED (mirrors the Rust `WorkerProcessBackend` discipline):
#   - worker exit non-zero with no output, worker timeout (killed),
#     unparseable output, wrong protocol tag, missing fields, or
#     non-finite numbers ⇒ THIS FEDERATE exits non-zero with nothing
#     meaningful on stdout. There is NO path from a worker failure to a
#     published value; the orchestrator records the federate as
#     fail-closed.
#   - A worker IN-BAND failure (`converged: false` +
#     `reported_error`) is a RESULT: the federate records it, publishes
#     NOTHING that step (absence, never a fabricated voltage), counts
#     the skip, and continues — the same in-band discipline the P4.08
#     gate applies.
#
# MAPPING ASSUMPTIONS (recorded, UNVERIFIED):
#   - `options.model_ir_path` holds the `VersionedNetworkModel` serde
#     shape (the worker's request `model` document) as a JSON file.
#   - `options.bus_map` maps the injection topics' asset keys to bus_ids
#     (asset_key → bus_id). An injection topic without a bus-map entry is
#     a fail-closed config error BEFORE any federation time advances.
#   - Publication topics carry the bus/branch ids verbatim:
#     `flexcore/grid/bus/<bus_id>/vm_pu` and
#     `flexcore/grid/branch/<branch_id>/loading_percent`; a publication
#     topic not of that form is a fail-closed config error.
#
# Every helics call: UNVERIFIED (table in `federate_common.py`).
# ══════════════════════════════════════════════════════════════════════════

from __future__ import annotations

import argparse
import json
import subprocess
import sys

from federate_common import (
    emit_report,
    fail,
    federate_report,
    helics_version,
    import_helics,
    load_config,
    section_for,
)

GRID_WORKER_PROTOCOL_ID = "flexcore-grid-worker/1"
GRID_WORKER_REQUEST = "run_power_flow"


def load_json_file(path: str, what: str):
    try:
        with open(path, "r", encoding="utf-8") as handle:
            return json.load(handle)
    except OSError as error:
        fail("%s could not be read: %s" % (what, error))
        return None  # unreachable
    except json.JSONDecodeError as error:
        fail("%s is not a valid JSON document: %s" % (what, error))
        return None  # unreachable


def run_worker_once(program: list, request: dict) -> dict:
    """ONE JSON document on the worker's stdin, ONE on its stdout — the
    exact P5.15 transport contract, with the same fail-closed mapping the
    Rust `WorkerProcessBackend` applies (spawn failure, non-zero exit
    with no output, malformed output, wrong protocol tag)."""
    try:
        completed = subprocess.run(
            program,
            input=json.dumps(request),
            capture_output=True,
            text=True,
            timeout=30.0,
            check=False,
        )
    except subprocess.TimeoutExpired:
        fail("the grid worker did not answer within the bounded wait; killed — fail-closed")
        return {}  # unreachable
    except OSError as error:
        fail("the grid worker process could not be spawned: %s" % error)
        return {}  # unreachable
    text = completed.stdout
    if not text.strip():
        fail(
            "the grid worker exited %d and produced no result document%s"
            % (
                completed.returncode,
                "" if not completed.stderr.strip() else " (worker stderr: %s)" % completed.stderr.strip(),
            )
        )
        return {}  # unreachable
    try:
        response = json.loads(text)
    except json.JSONDecodeError as error:
        fail("the grid worker response is not a contract-shaped JSON document: %s" % error)
        return {}  # unreachable
    if response.get("protocol") != GRID_WORKER_PROTOCOL_ID:
        fail("the grid worker response carries protocol %r, not %r" % (response.get("protocol"), GRID_WORKER_PROTOCOL_ID))
    return response


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Grid HELICS federate for the FlexCore Phase 5 co-simulation (P5.16)."
    )
    parser.add_argument("--federation-config", required=True)
    parser.add_argument("--federate", required=True)
    args = parser.parse_args()

    config = load_config(args.federation_config)
    section = section_for(config, args.federate)
    if section.get("role") != "grid":
        fail("federate %r is not declared with role 'grid'" % args.federate)
    options = section.get("options", {})
    worker_script = options.get("grid_worker_script")
    if not isinstance(worker_script, str) or not worker_script:
        fail("the grid federate requires options.grid_worker_script (the UNMODIFIED P5.15 worker)")
    model_ir_path = options.get("model_ir_path")
    if not isinstance(model_ir_path, str) or not model_ir_path:
        fail("the grid federate requires options.model_ir_path (the VersionedNetworkModel JSON)")
    bus_map = options.get("bus_map")
    if not isinstance(bus_map, dict) or not bus_map:
        fail("the grid federate requires options.bus_map (asset_key → bus_id)")
    interpreter = options.get("grid_worker_interpreter", sys.executable or "python")

    model_ir = load_json_file(model_ir_path, "the model IR document")

    h = import_helics()

    federate = h.helicsCreateValueFederate(args.federate)
    broker_address = config.get("broker", {}).get("address", "")
    h.helicsFederateSetCoreInitString(federate, "--broker=%s" % broker_address)

    period = config.get("time_step_seconds")
    stop_time = config.get("stop_time_seconds")
    h.helicsFederateSetTimeProperty(federate, h.HELICS_PROPERTY_TIME_PERIOD, period)

    # Subscriptions: per-asset injections of the form
    # `flexcore/asset/<asset_key>/p_mw` (and q_mvar), each mapped to its
    # bus through options.bus_map.
    injections: list = []
    for endpoint in section.get("subscriptions", []):
        topic = endpoint["topic"]
        if not topic.startswith("flexcore/asset/"):
            fail("subscription topic %r is not an injection publication (flexcore/asset/...)" % topic)
        remainder = topic[len("flexcore/asset/"):]
        key_parts = remainder.rsplit("/", 1)
        if len(key_parts) != 2:
            fail("subscription topic %r is not of the <asset_key>/<quantity> form" % topic)
        asset_key, quantity = key_parts
        if asset_key not in bus_map:
            fail("subscription topic %r names an asset key absent from options.bus_map" % topic)
        injections.append(
            {
                "topic": topic,
                "asset_key": asset_key,
                "quantity": quantity,
                "bus_id": bus_map[asset_key],
                "sub": h.helicsFederateRegisterSubscription(federate, topic, endpoint["unit"]),
            }
        )
    p_injections = [entry for entry in injections if entry["quantity"] == "p_mw"]
    if not p_injections:
        fail("the grid federate must subscribe at least one p_mw injection publication")

    # Publications: bus voltages + branch loadings, ids verbatim in the
    # topic; validated against the model IR BEFORE any time advances.
    bus_ids = {bus["bus_id"] for bus in model_ir.get("buses", [])}
    branch_ids = {branch["branch_id"] for branch in model_ir.get("branches", [])}
    publications: list = []
    for endpoint in section.get("publications", []):
        topic = endpoint["topic"]
        if topic.startswith("flexcore/grid/bus/") and topic.endswith("/vm_pu"):
            element_id = topic[len("flexcore/grid/bus/"):-len("/vm_pu")]
            if element_id not in bus_ids:
                fail("publication topic %r names a bus absent from the model IR" % topic)
        elif topic.startswith("flexcore/grid/branch/") and topic.endswith("/loading_percent"):
            element_id = topic[len("flexcore/grid/branch/"):-len("/loading_percent")]
            if element_id not in branch_ids:
                fail("publication topic %r names a branch absent from the model IR" % topic)
        else:
            fail("publication topic %r is neither a bus vm_pu nor a branch loading_percent form" % topic)
        publications.append(
            (topic, element_id, h.helicsFederateRegisterPublication(
                federate, topic, h.HELICS_DATA_TYPE_DOUBLE, endpoint["unit"]))
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

        # Read the current injections (injection-positive: P > 0 export).
        injection_rows = []
        for entry in injections:
            value = h.helicsInputGetDouble(entry["sub"])
            received_values += 1
            injection_rows.append(
                {
                    "injection_id": "%s@%s" % (entry["asset_key"], entry["bus_id"]),
                    "bus_id": entry["bus_id"],
                    "p_injection_mw": value if entry["quantity"] == "p_mw" else 0.0,
                    "q_injection_mvar": value if entry["quantity"] == "q_mvar" else 0.0,
                }
            )

        # Collapse to ONE row per bus (P and Q of the same asset pair up;
        # two assets on one bus would already be separate config rows).
        by_bus: dict = {}
        for row in injection_rows:
            acc = by_bus.setdefault(
                row["bus_id"], {"injection_id": row["injection_id"], "bus_id": row["bus_id"],
                                "p_injection_mw": 0.0, "q_injection_mvar": 0.0}
            )
            acc["p_injection_mw"] += row["p_injection_mw"]
            acc["q_injection_mvar"] += row["q_injection_mvar"]

        request = {
            "protocol": GRID_WORKER_PROTOCOL_ID,
            "request": GRID_WORKER_REQUEST,
            "model": model_ir,
            "injections": list(by_bus.values()),
            "options": {"evaluation_label": "helics-grid-federate-step-%d" % steps, "report_provider_version": True},
        }
        response = run_worker_once([interpreter, worker_script], request)

        if not response.get("converged", False):
            # In-band RESULT (§12.5): record, publish nothing this step,
            # never fabricate a voltage, never retry (NO_RETRY_POLICY).
            skipped_publications += len(publications)
            continue

        res_bus = {row["bus_id"]: row["vm_pu"] for row in response.get("bus_results", [])}
        res_branch = {row["branch_id"]: row["loading_percent"] for row in response.get("branch_results", [])}
        for topic, element_id, pub in publications:
            if topic.startswith("flexcore/grid/bus/"):
                value = res_bus.get(element_id)
            else:
                value = res_branch.get(element_id)
            if value is None:
                # Row completeness is a contract (one row per model
                # element); a missing row is fail-closed, never skipped.
                fail("the grid worker's result carries no row for %r (contract violation)" % element_id)
                return  # unreachable
            h.helicsPublicationPublishDouble(pub, float(value))
            published_values += 1

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
            role="grid",
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
