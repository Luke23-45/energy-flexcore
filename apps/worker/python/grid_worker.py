# P5.15 — The grid numerical worker entrypoint: grid-api IR → pandapower
#         AC power flow → the grid-api result payload (one JSON document on
#         stdin, one JSON document on stdout).
#
# ══════════════════════════════════════════════════════════════════════════
# HONESTY HEADER (read before trusting anything below)
#
# 1. THIS WORKER WAS WRITTEN, NEVER EXECUTED. The Phase 5 loop bans
#    install/build/run; pandapower 3.5.6 (the recorded pin,
#    `ledger/phase-04-grid-feasibility-dispatch` §3, D-P4.01-1) was NEVER
#    installed and this script was NEVER run — not against real pandapower,
#    not against a stub. Every pandapower call below follows the provider's
#    DOCUMENTED API as of the 2026-11-07 research cut-off and is
#    **UNVERIFIED** until the human's build/test pass executes it against
#    the pinned version. Nothing in this file is evidence that any power
#    flow ever ran.
# 2. SOLVER-CLASS DISCIPLINE (absolute; AGENTS.md rule 4): what this worker
#    computes — when it is finally executed — is an **AC power-flow
#    evaluation of specified injections and controls** (pandapower
#    `runpp`, a Newton–Raphson steady-state power flow). It is **NOT an
#    AC-OPF**: no objective, no optimization, no dispatch recommendation
#    comes out of this worker, and the terms "AC power flow" and "AC-OPF"
#    are never interchanged. The gate on the Rust side never optimizes
#    (§12.5).
# 3. SCOPE (declared, §12.4/§12.5): balanced positive-sequence AC power
#    flow on a radial distribution model. Unbalanced three-phase,
#    protection, dynamics, sub-cycle behaviour, and steady-state stability
#    assessment are NOT modelled here. A converged, within-limits result
#    is a simulation statement about the declared model and limits — never
#    a field-safety claim, and never transient-stability evidence (a
#    steady-state tool is not a stability tool, §19.1).
# 4. TRANSPORT CONTRACT (mirrors
#    `crates/flexcore-grid-api/src/worker_backend.rs`): ONE JSON document
#    on stdin (the `flexcore-grid-worker/1` request), ONE JSON document on
#    stdout (the `flexcore-grid-worker/1` response envelope whose payload
#    is the `BackendPowerFlowOutcome` serde shape). A malformed request is
#    answered with a NON-ZERO exit and NOTHING on stdout (the Rust side
#    fail-closes: BackendError::Unavailable). A failure DURING an attempted
#    evaluation is answered IN-BAND (`converged: false` + `reported_error`)
#    — a non-convergence or evaluation error is a RESULT, never a crash
#    and never retried (NO_RETRY_POLICY, D-P4.05-2).
# ══════════════════════════════════════════════════════════════════════════
#
# Mapping assumptions (each one recorded, none verified against the pinned
# provider — VERIFY-BUILD items):
#
# | IR field                | pandapower mapping (documented-API form)        | Assumption recorded |
# |-------------------------|--------------------------------------------------|---------------------|
# | bus `nominal_kv`        | `create_bus(vn_kv=...)`                          | direct |
# | slack bus / ext grid    | `create_ext_grid(vm_pu, va_degree)`              | direct |
# | branch r/x ohm          | `create_line_from_parameters(r_ohm_per_km, x_ohm_per_km, length_km=1.0)` | unit-length representation: the IR carries absolute ohms, pandapower lines carry per-km × length; 1 km of per-km = the absolute value |
# | branch `rated_current_a`| `max_i_ka = rated_current_a / 1000`              | A → kA conversion |
# | trafo `rated_mva`       | `create_transformer_from_parameters(sn_mva=...)` | direct |
# | trafo `vk_percent`      | `vk_percent`                                     | direct |
# | trafo `vk_r_percent`    | `vkr_percent`                                    | the IR's real-part spelling maps to pandapower's `vkr_percent` |
# | (no IR field)           | `pfe_kw=0.0`, `i0_percent=0.0`                   | the IR carries no no-load data; zero no-load losses is a recorded MODEL simplification, listed in the outcome warnings — never silently assumed |
# | injection p >= 0 (into network) | `create_sgen(p_mw=p, q_mvar=q)`          | positive sgen = generation |
# | injection p < 0 (consumption)   | `create_load(p_mw=-p, q_mvar=-q)`        | positive load = consumption; the q sign flips so the payload's injection-positive reactive convention is preserved |
#
# Result mapping (row completeness is a CONTRACT — one row per model
# element, matched by identifier; a missing result row is reported in-band,
# and the Rust adapter independently re-checks every row):
#
# | Outcome field            | Source (documented API) |
# |--------------------------|-------------------------|
# | `converged`              | `net.converged` after `pp.runpp` |
# | bus `vm_pu`              | `net.res_bus.vm_pu` per created bus |
# | branch `i_from_ka`/`i_to_ka` | `net.res_line.i_from_ka` / `i_to_ka` |
# | branch `loading_percent` | `net.res_line.loading_percent` |
# | trafo `loading_percent`  | `net.res_trafo.loading_percent` |
# | trafo `i_hv_ka`/`i_lv_ka`| `net.res_trafo.i_hv_ka` / `i_lv_ka` |
# | `iterations`             | `net._ppc["iterations"]` when present (private surface — read defensively, `None` when absent; the outcome field is optional) |
# | `provider_version_reported` | `pandapower.__version__` |
#
# Everything this worker computes is simulation-only (the 0.x line): no
# field read, no field write, no control authority, no safety or
# certification implication.

from __future__ import annotations

import json
import sys

# The protocol tag MUST match `worker_backend.rs` WORKER_PROTOCOL_ID.
PROTOCOL_ID = "flexcore-grid-worker/1"
REQUEST_RUN_POWER_FLOW = "run_power_flow"

# The recorded provider pin (D-P4.01-1). The worker reports its INSTALLED
# version; the Rust adapter cross-checks it against the pin — the worker
# itself does not refuse a mismatch (the pin decision lives with the
# adapter), but it surfaces the installed version always.
PINNED_PANDAPOWER_VERSION = "3.5.6"


def _fail(message: str) -> None:
    """Answers a malformed request: nothing on stdout, the reason on
    stderr, non-zero exit. The Rust transport maps this to a fail-closed
    `BackendError::Unavailable` (never a best-effort parse of a partial
    document)."""
    sys.stderr.write("flexcore-grid-worker: " + message + "\n")
    sys.exit(2)


def _finite(value: object) -> bool:
    return isinstance(value, (int, float)) and float(value) == float(value) and float(value) not in (
        float("inf"),
        float("-inf"),
    )


def build_network(model: dict):
    """Translates the provider-neutral model IR into the pinned provider's
    model, in the DOCUMENTED API form (UNVERIFIED — see the honesty
    header). Returns `(net, bus_index, branch_index, trafo_index)` where
    each index maps the IR element id to the provider element index, so
    result rows are matched by identity, never by table order."""
    import pandapower as pp  # deferred import: a missing provider is a worker-level failure answered on stderr, not a crash inside a half-built network

    net = pp.create_empty_network(name=model.get("model_ref", {}).get("model_id", "flexcore-grid"),
                                  sn_mva=model.get("base_mva", 1.0))

    bus_index: dict[str, int] = {}
    for bus in model.get("buses", []):
        bus_index[bus["bus_id"]] = pp.create_bus(
            net,
            vn_kv=float(bus["nominal_kv"]),
            name=bus["bus_id"],
        )

    # The slack: one ext grid per IR external grid (the IR's slack carrier).
    for external in model.get("external_grids", []):
        pp.create_ext_grid(
            net,
            bus=bus_index[external["bus_id"]],
            vm_pu=float(external["vm_pu"]),
            va_degree=float(external.get("va_degree", 0.0)),
        )

    branch_index: dict[str, int] = {}
    for branch in model.get("branches", []):
        # Unit-length representation (recorded assumption above): absolute
        # ohms mapped as per-km × 1 km.
        branch_index[branch["branch_id"]] = pp.create_line_from_parameters(
            net,
            from_bus=bus_index[branch["from_bus"]],
            to_bus=bus_index[branch["to_bus"]],
            length_km=1.0,
            r_ohm_per_km=float(branch["r_ohm"]),
            x_ohm_per_km=float(branch["x_ohm"]),
            c_nf_per_km=0.0,
            max_i_ka=float(branch["rated_current_a"]) / 1000.0,
            name=branch["branch_id"],
        )

    trafo_index: dict[str, int] = {}
    warnings: list[str] = []
    for trafo in model.get("transformers", []):
        hv_kv = float(next(b["nominal_kv"] for b in model["buses"] if b["bus_id"] == trafo["hv_bus"]))
        lv_kv = float(next(b["nominal_kv"] for b in model["buses"] if b["bus_id"] == trafo["lv_bus"]))
        # No-load data is absent from the IR; zeros are a recorded model
        # simplification (assumption table above) — carried into warnings
        # so no result can lose the statement.
        warnings.append(
            "transformer '%s': no-load parameters (pfe_kw, i0_percent) are absent from the "
            "grid-model IR and were mapped as ZERO — a recorded model simplification, not "
            "provider behaviour" % trafo["transformer_id"]
        )
        trafo_index[trafo["transformer_id"]] = pp.create_transformer_from_parameters(
            net,
            hv_bus=bus_index[trafo["hv_bus"]],
            lv_bus=bus_index[trafo["lv_bus"]],
            sn_mva=float(trafo["rated_mva"]),
            vn_hv_kv=hv_kv,
            vn_lv_kv=lv_kv,
            vk_percent=float(trafo["vk_percent"]),
            vkr_percent=float(trafo["vk_r_percent"]),
            pfe_kw=0.0,
            i0_percent=0.0,
            name=trafo["transformer_id"],
        )

    return net, bus_index, branch_index, trafo_index, warnings


def apply_injections(net, injections: list[dict]):
    """Materialises the translated injections. Positive payload values mean
    injection INTO the network (the `BackendInjection` contract)."""
    import pandapower as pp

    # The bus-name → element index lookup uses the element `name` fields the
    # build step recorded (documented attribute surface; the indices are
    # re-derived from the created elements' names, never assumed positional).
    name_to_bus = {name: idx for idx, name in net.bus["name"].items()}

    for injection in injections:
        bus = name_to_bus[injection["bus_id"]]
        p = float(injection["p_injection_mw"])
        q = float(injection["q_injection_mvar"])
        if p >= 0.0:
            # Injection: generation-type element (positive sgen = generation).
            pp.create_sgen(net, bus=bus, p_mw=p, q_mvar=q, name=injection["injection_id"])
        else:
            # Consumption: load-type element; the reactive sign flips so the
            # payload's injection-positive convention is preserved (positive
            # payload q = injection into the network = negative load q).
            pp.create_load(net, bus=bus, p_mw=-p, q_mvar=-q, name=injection["injection_id"])


def run_power_flow(request: dict) -> dict:
    """The one request kind: model + injections → AC power flow → outcome
    payload (the `BackendPowerFlowOutcome` serde shape, before the
    protocol-tag envelope is added)."""
    import pandapower as pp

    model = request["model"]
    injections = request.get("injections", [])
    options = request.get("options", {})
    report_version = bool(options.get("report_provider_version", True))

    for injection in injections:
        for field in ("p_injection_mw", "q_injection_mvar"):
            if not _finite(injection.get(field)):
                return {
                    "converged": False,
                    "reported_error": "injection field '%s' is not a finite number" % field,
                    "provider_version_reported": _provider_version(report_version),
                    "bus_results": [],
                    "branch_results": [],
                    "transformer_results": [],
                    "iterations": None,
                    "worker_status_text": "request rejected before any power flow (in-band, fail-closed)",
                    "warnings": [],
                }

    net, bus_index, branch_index, trafo_index, warnings = build_network(model)
    apply_injections(net, injections)

    # THE power-flow call: a Newton–Raphson balanced AC power flow of the
    # specified injections — NOT an AC-OPF (solver-class discipline; the
    # honesty header). Explicit arguments, no provider-side defaults relied
    # on silently (every deviation from the documented default is an
    # assumption carried in the header table).
    pp.runpp(net, algorithm="nr", calculate_voltage_angles=True)

    converged = bool(getattr(net, "converged", False))
    iterations = None
    try:
        raw_iterations = net._ppc.get("iterations") if hasattr(net, "_ppc") else None
        if raw_iterations is not None:
            iterations = int(raw_iterations)
    except (KeyError, TypeError, ValueError):
        iterations = None

    bus_results = []
    branch_results = []
    transformer_results = []
    missing_rows: list[str] = []

    res_bus = getattr(net, "res_bus", None)
    for bus_id, element in bus_index.items():
        try:
            vm_pu = float(res_bus.at[element, "vm_pu"])
        except (KeyError, AttributeError, ValueError):
            missing_rows.append("bus '%s'" % bus_id)
            continue
        bus_results.append({"bus_id": bus_id, "vm_pu": vm_pu})

    res_line = getattr(net, "res_line", None)
    for branch_id, element in branch_index.items():
        try:
            branch_results.append({
                "branch_id": branch_id,
                "i_from_ka": float(res_line.at[element, "i_from_ka"]),
                "i_to_ka": float(res_line.at[element, "i_to_ka"]),
                "loading_percent": float(res_line.at[element, "loading_percent"]),
            })
        except (KeyError, AttributeError, ValueError):
            missing_rows.append("branch '%s'" % branch_id)

    res_trafo = getattr(net, "res_trafo", None)
    for trafo_id, element in trafo_index.items():
        try:
            transformer_results.append({
                "transformer_id": trafo_id,
                "loading_percent": float(res_trafo.at[element, "loading_percent"]),
                "i_hv_ka": float(res_trafo.at[element, "i_hv_ka"]),
                "i_lv_ka": float(res_trafo.at[element, "i_lv_ka"]),
            })
        except (KeyError, AttributeError, ValueError):
            missing_rows.append("transformer '%s'" % trafo_id)

    reported_error = None
    if missing_rows:
        # Row completeness is a contract; a missing row is reported in-band
        # (the Rust adapter independently fail-closes on row mismatch too).
        reported_error = "result rows missing for: " + ", ".join(missing_rows)
    if not converged and reported_error is None:
        # Non-convergence is an in-band RESULT (§12.5) — never an exception.
        reported_error = "the AC power flow did not converge"

    return {
        "converged": converged,
        "reported_error": reported_error,
        "provider_version_reported": _provider_version(report_version),
        "bus_results": bus_results,
        "branch_results": branch_results,
        "transformer_results": transformer_results,
        "iterations": iterations,
        "worker_status_text": (
            "AC power flow (Newton–Raphson) of specified injections — NOT an AC-OPF; "
            "pandapower %s as pinned (D-P4.01-1); UNVERIFIED-API worker, first execution "
            "belongs to the human build/test pass" % _provider_version(report_version)
        ),
        "warnings": warnings,
    }


def _provider_version(report: bool) -> str | None:
    if not report:
        return None
    try:
        import pandapower
        return str(pandapower.__version__)
    except Exception:  # noqa: BLE001 — the version probe must never crash the worker
        return None


def main() -> None:
    # ONE JSON document on stdin to EOF (the transport contract; the Rust
    # side writes the request and closes stdin).
    raw = sys.stdin.read()
    try:
        request = json.loads(raw)
    except json.JSONDecodeError as error:
        _fail("the request is not a valid JSON document: %s" % error)
        return  # unreachable; keeps type-checkers honest

    if request.get("protocol") != PROTOCOL_ID:
        _fail("request protocol %r is not %s" % (request.get("protocol"), PROTOCOL_ID))
    if request.get("request") != REQUEST_RUN_POWER_FLOW:
        _fail("request kind %r is not %r" % (request.get("request"), REQUEST_RUN_POWER_FLOW))
    if not isinstance(request.get("model"), dict):
        _fail("the request carries no model document")

    try:
        outcome = run_power_flow(request)
    except Exception as error:  # noqa: BLE001 — an evaluation failure is IN-BAND, never a crash
        outcome = {
            "converged": False,
            "reported_error": "the worker failed during the evaluation: %s: %s"
            % (type(error).__name__, error),
            "provider_version_reported": _provider_version(True),
            "bus_results": [],
            "branch_results": [],
            "transformer_results": [],
            "iterations": None,
            "worker_status_text": "in-band evaluation failure (fail-closed on the Rust side)",
            "warnings": [],
        }

    sys.stdout.write(json.dumps({"protocol": PROTOCOL_ID, **outcome}))
    sys.stdout.flush()


if __name__ == "__main__":
    main()
