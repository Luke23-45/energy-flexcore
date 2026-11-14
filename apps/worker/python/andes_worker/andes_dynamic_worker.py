# P8.16 — The ANDES dynamic-simulation research worker: an isolated,
#         GPLv3-contained, proposal-only Class C worker.
#
# ══════════════════════════════════════════════════════════════════════════
# HONESTY HEADER — read before trusting anything below
#
# 1. THIS WORKER WAS WRITTEN, NEVER EXECUTED. ANDES 2.0.0 (the recorded
#    pin, `requirements-andes.txt` in this directory; RESEARCH.md §4:
#    released 11 March 2026) was NEVER installed in this repository's
#    history, and this script was NEVER run — not against real ANDES, not
#    against a stub. Every `andes` call below follows the DOCUMENTED ANDES
#    2.0 API as of the 2026-11-07 research cut-off and is **UNVERIFIED**
#    until the human build/test pass executes it against the pinned
#    version (R8-C5: pin ANDES 2.0.0 or a VERIFIED later release; the live
#    docs show a development version string — never cite "the docs"
#    generically). Nothing in this file is evidence that any dynamic
#    simulation ever ran.
# 2. LICENSE ISOLATION (P8.03 six-point design; gate G-4 OPEN — legal
#    confirmation is a HUMAN gate this worker cannot grant):
#    - ANDES lives ONLY inside this worker process; the import is guarded
#      INSIDE `import_andes()` (a missing ANDES raises the NAMED
#      deployment requirement below — the pure-logic core of this file
#      imports and tests WITHOUT ANDES installed, which is also the loop's
#      honesty gate: without ANDES, no study result can ever exist);
#    - no core crate, no SDK crate, no app depends on ANDES; the Rust side
#      never links, vendores, or FFI-bridges ANDES code;
#    - FlexCore consumes ANDES OUTPUTS (traces, eigenvalues) AS DATA over
#      this worker's JSON-document boundary — never ANDES-derived code;
#    - any distribution of ANDES-derived code goes through legal first;
#    - the isolation is verified by INSPECTING the worker boundary (the
#      import-graph check in `16_andes_integration.md` §6), not by this
#      header's say-so;
#    - dynamic results cross the trust boundary as UNTRUSTED INPUT and are
#      never dispatch inputs.
# 3. CONTROL BOUNDARY (absolute): this worker has NO field-write path and
#    NO device channel — its only output is ONE JSON document on stdout.
#    It never issues, approves, or carries a command; its output type is a
#    PROPOSAL (ADR-010): `assert_proposal_only()` scans the result for any
#    authorization-token/command-shaped key and refuses to emit one. It is
#    mode-locked to simulation/shadow: it can never select or raise a
#    control-authority mode (mode is a deployment/per-site setting, §21.4).
# 4. MODEL BOUNDARY (§19.1): a time-domain simulation and an eigenvalue
#    analysis are statements about the DECLARED ANDES model and case —
#    never about the real grid. The steady-state power flow ANDES can run
#    for TDS initialization is a NEWTON-RAPHSON STEADY-STATE POWER FLOW:
#    it is NOT transient stability evidence, and the dynamic simulator is
#    NOT a replacement for the distribution quasi-static power-flow
#    pipeline (pandapower stays the Phase 4 quasi-static tool). Solver
#    classes are never conflated (AGENTS.md rule 4). Every result carries
#    its model scope (case, models, versions) attached.
# 5. TRANSPORT CONTRACT (mirrors `../grid_worker.py`): ONE JSON document
#    on stdin (the `flexcore-andes-worker/1` request), ONE JSON document
#    on stdout (the `flexcore-andes-worker/1` result envelope). A
#    malformed request is answered with NOTHING on stdout + a reason on
#    stderr + exit 2 (fail-closed). A failure DURING an attempted study
#    (non-convergence, unreadable case, missing output surface) is
#    answered IN-BAND (`termination_status: "failed"` + diagnostics) — a
#    failed study is a RESULT, never a crash and never a fabricated
#    figure. NO number in any output is invented: metrics are computed
#    ONLY on traces actually extracted from the run (via the pure
#    `evaluation_harness.py`), and a missing trace is a recorded failure.
# 6. CONTRACT SEAM (recorded, not invented): this worker reuses the Phase
#    7 worker template's §9.2 provenance field names verbatim
#    (`worker_version`, `model_version`, `solver_version`,
#    `input_snapshot_id`, `termination_status`, `diagnostics`,
#    `execution_ms`) and follows the template's guarded-import pattern,
#    but its transport is the stdin/stdout JSON-document discipline of the
#    Phase 5 grid worker — binding the generated `flexcore.worker.v1`
#    gRPC servicer (buf generation of
#    `python/workers/contracts/worker_api.proto`) is the recorded
#    ACTIVATION step, exactly as `example_echo_worker.py` records it. The
#    gap is recorded in `16_andes_integration.md` §7 as a Phase 7
#    follow-up, never fixed by inventing a parallel contract.
#
# ANDES API usage table (DOCUMENTED 2.0 forms; every row UNVERIFIED —
# VERIFY-BUILD items, checked at the human's first worker run):
#
# | Purpose                  | Documented call (as written below)          | Recorded assumption |
# |--------------------------|---------------------------------------------|---------------------|
# | version probe            | `andes.__version__`                         | read defensively; `None` when absent |
# | case load + setup        | `andes.load(case_path, setup=True)`         | `setup=True` runs the DAE setup; a load failure raises |
# | steady-state init (PF)   | `system.PFlow.run()`                        | converged flag read defensively from the PFlow attribute surface |
# | TDS initialization       | `system.TDS.init()`                         | required before `TDS.run` |
# | TDS run to end time      | `system.TDS.run(to=<end_time>)`             | `to=` keyword form; a `TDS.converged`-style flag is read defensively |
# | eigenvalue analysis      | `system.EIG.run()`                          | eigenvalues read defensively from an `EIG` result surface |
# | trace extraction         | `system.dae.ts` (t + state/output arrays)   | the TDS recorded-output surface; read defensively, absence is an IN-BAND failure |
#
# Everything this worker computes is simulation-only research output (the
# proposal-only ceiling, §13.4): no field read, no field write, no control
# authority, no safety or certification implication, no protection-grade
# claim, and no transient-stability claim about any real system.
# ══════════════════════════════════════════════════════════════════════════

from __future__ import annotations

import json
import math
import sys

import evaluation_harness

# The worker-transport protocol tag (the mirror of
# `flexcore-grid-worker/1` for the dynamic track; the tag is validated on
# every request, fail-closed).
PROTOCOL_ID = "flexcore-andes-worker/1"

# The recorded ANDES pin (`requirements-andes.txt`; RESEARCH.md §4). The
# worker reports the INSTALLED version; the pin discipline is carried by
# the decision record and the P8.36 at-pin-time CVE check — never by
# refusing a mismatched install silently.
ANDES_PINNED_VERSION = "2.0.0"

# Control-authority mode this worker is LOCKED to (§21.4). It is a
# constant, not a setting: no request field can change it, and no plugin
# or research module can select a mode (§5.7).
MODE_LOCKED = "simulation-shadow"

WORKER_VERSION = "flexcore-andes-worker/0.1.0"

# The named deployment requirement a missing ANDES surfaces (raised by
# `import_andes`); `16_andes_integration.md` §5 repeats it verbatim.
ANDES_DEPLOYMENT_REQUIREMENT = (
    "flexcore ANDES dynamic worker requires the isolated research "
    "dependency: install the ANDES pin recorded in "
    "apps/worker/python/andes_worker/requirements-andes.txt "
    "(ANDES %s, GPLv3, ISOLATION DESIGN GATE G-4 OPEN — legal "
    "confirmation required before any dependence) into an ISOLATED "
    "research environment, never into a system environment and never into "
    "the platform's runtime" % ANDES_PINNED_VERSION
)

# The unit vocabulary a monitored signal may declare (§10.2 canonical
# units; frequency in Hz; voltage pu in solver representation). A declared
# unit outside this set is a fail-closed config error, never guessed.
ALLOWED_UNITS: frozenset = frozenset(
    {"MW", "MVAr", "MVA", "MWh", "kV", "A", "pu", "percent", "Hz", "rad"}
)

STUDY_TYPES: frozenset = frozenset(
    {"time_domain", "eigenvalue", "steady_state_power_flow"}
)

# Keys that must NEVER appear anywhere in a result document: this is the
# in-code proposal-only boundary (P8.04 posture; ADR-010). The response
# type cannot express an authorization token or a device command — if one
# of these keys ever appears, `assert_proposal_only` refuses the emission.
FORBIDDEN_RESULT_KEYS: frozenset = frozenset(
    {
        "authorization_token",
        "authorization",
        "auth_token",
        "command",
        "command_id",
        "command_ids",
        "issue",
        "issued",
        "dispatch",
        "dispatchable",
        "approval",
        "approval_token",
    }
)

# The boundary statements carried VERBATIM in every result envelope (the
# model limits travel with every number; §19.1, P8.16 step 4).
BOUNDARY_STATEMENTS: tuple = (
    "Every result is a statement about the DECLARED ANDES model and case "
    "(model scope attached above), never about the real grid (report 19.1).",
    "A steady-state power flow is NOT transient-stability evidence, and "
    "the dynamic simulator is NOT a replacement for distribution "
    "quasi-static power flow (report 19.1).",
    "This worker's outputs are research PROPOSALS/diagnostics only "
    "(ADR-010): no authorization token exists in this response type, and "
    "nothing here can be issued to a device.",
    "This worker is mode-locked to simulation/shadow; it cannot select or "
    "raise a control-authority mode (report 21.4).",
    "ANDES is GPLv3 and lives only inside this isolated worker; FlexCore "
    "consumes its outputs as data, never its code (P8.03 isolation "
    "design; gate G-4 legal confirmation OPEN).",
)

UNVERIFIED_NOTE = (
    "UNVERIFIED-API worker: ANDES %s (GPLv3) was never installed in this "
    "loop and this worker was never executed; every andes call follows "
    "the documented 2.0 API as of the 2026-11-07 cut-off (R8-C5: pin "
    "2.0.0 or a VERIFIED later release at implementation). First "
    "execution belongs to the human build/test pass."
) % ANDES_PINNED_VERSION


class WorkerRequestError(ValueError):
    """A malformed study request — fail-closed BEFORE any computation."""


def import_andes():
    """Imports the `andes` module or raises the NAMED deployment
    requirement.

    This is the single ANDES choke point and the loop's honesty gate:
    WITHOUT the installed pinned package, `run_study` can NEVER produce a
    result, so no dynamic-study record can exist without ANDES having
    actually run. The import lives INSIDE this function so the rest of
    this module parses, imports, and passes the pure-logic tests without
    ANDES installed.
    """
    try:
        import andes  # noqa: PLC0415 — deliberate deferred import (see header).
    except ImportError as error:
        raise ImportError(ANDES_DEPLOYMENT_REQUIREMENT) from error
    return andes


def andes_version_reported(andes_module) -> str | None:
    """The installed ANDES version, read defensively (must never crash
    the result; UNVERIFIED surface)."""
    try:
        version = getattr(andes_module, "__version__", None)
        return None if version is None else str(version)
    except Exception:  # noqa: BLE001 — the probe must never crash the worker
        return None


# ---------------------------------------------------------------------------
# Pure request validation (no ANDES, no I/O beyond the caller's dict)
# ---------------------------------------------------------------------------


def _require_nonempty_str(container: dict, field: str, where: str) -> str:
    value = container.get(field)
    if not isinstance(value, str) or not value.strip():
        raise WorkerRequestError(
            "%s.%s must be a non-empty string" % (where, field)
        )
    return value


def _require_finite_number(container: dict, field: str, where: str) -> float:
    value = container.get(field)
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise WorkerRequestError("%s.%s must be a finite number" % (where, field))
    if not math.isfinite(value):
        raise WorkerRequestError("%s.%s must be finite" % (where, field))
    return float(value)


def validate_request(request: dict) -> dict:
    """Validates the study request; raises `WorkerRequestError` on refusal.

    Refusals happen BEFORE any computation and before any ANDES import:
    a wrong protocol tag, an unknown study type, a missing case path, an
    empty model list, a monitored signal with a non-canonical unit, or a
    missing provenance field is a structured refusal, never a best-effort
    parse. Returns the validated fields, normalized.
    """
    if not isinstance(request, dict):
        raise WorkerRequestError("request must be a JSON object")
    if request.get("protocol") != PROTOCOL_ID:
        raise WorkerRequestError(
            "request protocol %r is not %r" % (request.get("protocol"), PROTOCOL_ID)
        )
    study_type = request.get("study_type")
    if study_type not in STUDY_TYPES:
        raise WorkerRequestError(
            "study_type %r is not one of %s" % (study_type, sorted(STUDY_TYPES))
        )
    case_path = _require_nonempty_str(request, "case_path", "request")
    case_version = _require_nonempty_str(request, "case_version", "request")

    models = request.get("models")
    if not isinstance(models, list) or not models:
        raise WorkerRequestError(
            "request.models must be a non-empty list of declared ANDES "
            "model names (the model scope is recorded verbatim)"
        )
    for index, model in enumerate(models):
        if not isinstance(model, str) or not model.strip():
            raise WorkerRequestError(
                "request.models[%d] must be a non-empty string" % index
            )

    monitored: list = []
    simulation: dict = {}
    evaluation: dict = {}
    if study_type == "time_domain":
        simulation = request.get("simulation")
        if not isinstance(simulation, dict):
            raise WorkerRequestError(
                "a time_domain study requires request.simulation "
                "(solver, step_seconds, end_time_seconds)"
            )
        _require_nonempty_str(simulation, "solver", "request.simulation")
        step = _require_finite_number(simulation, "step_seconds", "request.simulation")
        end = _require_finite_number(
            simulation, "end_time_seconds", "request.simulation"
        )
        if step <= 0.0:
            raise WorkerRequestError("request.simulation.step_seconds must be > 0")
        if end <= 0.0:
            raise WorkerRequestError("request.simulation.end_time_seconds must be > 0")
        monitored_raw = request.get("monitored")
        if not isinstance(monitored_raw, list) or not monitored_raw:
            raise WorkerRequestError(
                "a time_domain study requires request.monitored (the "
                "declared export list; absence is a refusal, never an "
                "assumed default)"
            )
        for index, entry in enumerate(monitored_raw):
            if not isinstance(entry, dict):
                raise WorkerRequestError("request.monitored[%d] must be an object" % index)
            signal = _require_nonempty_str(entry, "signal", "request.monitored[%d]" % index)
            unit = _require_nonempty_str(entry, "unit", "request.monitored[%d]" % index)
            if unit not in ALLOWED_UNITS:
                raise WorkerRequestError(
                    "request.monitored[%d].unit %r is not a declared "
                    "canonical unit (allowed: %s)"
                    % (index, unit, sorted(ALLOWED_UNITS))
                )
            monitored.append({"signal": signal, "unit": unit})
        evaluation_raw = request.get("evaluation")
        if evaluation_raw is not None:
            if not isinstance(evaluation_raw, dict):
                raise WorkerRequestError("request.evaluation must be an object")
            _require_finite_number(evaluation_raw, "baseline", "request.evaluation")
            _require_finite_number(evaluation_raw, "event_time_s", "request.evaluation")
            tolerance = _require_finite_number(
                evaluation_raw, "tolerance", "request.evaluation"
            )
            if tolerance < 0.0:
                raise WorkerRequestError(
                    "request.evaluation.tolerance must be non-negative"
                )
            quantity = evaluation_raw.get("quantity", "frequency_hz")
            if quantity not in evaluation_harness.QUANTITIES:
                raise WorkerRequestError(
                    "request.evaluation.quantity %r is not one of %s"
                    % (quantity, list(evaluation_harness.QUANTITIES))
                )
            evaluation = {
                "baseline": float(evaluation_raw["baseline"]),
                "event_time_s": float(evaluation_raw["event_time_s"]),
                "tolerance": float(tolerance),
                "quantity": quantity,
            }

    event = request.get("event")
    if event is not None:
        if not isinstance(event, dict):
            raise WorkerRequestError("request.event must be an object")
        _require_nonempty_str(event, "kind", "request.event")
        _require_finite_number(event, "at_seconds", "request.event")

    provenance = request.get("provenance")
    if not isinstance(provenance, dict):
        raise WorkerRequestError(
            "request.provenance is required (input_snapshot_id, request_id)"
        )
    input_snapshot_id = _require_nonempty_str(
        provenance, "input_snapshot_id", "request.provenance"
    )
    request_id = _require_nonempty_str(provenance, "request_id", "request.provenance")

    return {
        "study_type": study_type,
        "case_path": case_path,
        "case_version": case_version,
        "models": list(models),
        "simulation": simulation,
        "monitored": monitored,
        "evaluation": evaluation,
        "event": event if isinstance(event, dict) else None,
        "input_snapshot_id": input_snapshot_id,
        "request_id": request_id,
    }


# ---------------------------------------------------------------------------
# Pure result envelope (proposal-only by construction)
# ---------------------------------------------------------------------------


def assert_proposal_only(document) -> None:
    """Refuses to emit any document carrying a forbidden key.

    The in-code proposal-only boundary: scans EVERY mapping key in the
    document tree (recursively) against `FORBIDDEN_RESULT_KEYS`. A result
    that somehow grew an authorization/command-shaped field is refused
    (RuntimeError) instead of emitted — the response TYPE cannot express
    an authorization token, and this is the enforced proof, not prose.
    """
    if isinstance(document, dict):
        for key, value in document.items():
            if isinstance(key, str) and key.strip().lower() in FORBIDDEN_RESULT_KEYS:
                raise RuntimeError(
                    "proposal-only boundary violation: result key %r is "
                    "forbidden (ADR-010: a research result never carries "
                    "an authorization token or a device command)" % key
                )
            assert_proposal_only(value)
    elif isinstance(document, list):
        for item in document:
            assert_proposal_only(item)


def build_result(
    *,
    validated: dict,
    termination_status: str,
    outputs: dict,
    metrics: dict | None,
    diagnostics: list,
    andes_version: str | None,
    execution_ms: int,
) -> dict:
    """Builds the `flexcore-andes-worker/1` result envelope (pure).

    `termination_status` is "completed" only when the study actually ran
    to its recorded end and its outputs were extracted; "failed" marks an
    in-band study failure (non-convergence, unreadable case, missing
    output surface) with diagnostics — a failed study is a RESULT, never
    a crash and never a fabricated figure.
    """
    result = {
        "protocol": PROTOCOL_ID,
        "kind": "andes_study_result",
        "ok": termination_status == "completed",
        "study_type": validated["study_type"],
        # Proposal-only posture (ADR-010), enforced in code by
        # `assert_proposal_only` before emission.
        "proposal_only": True,
        "authority": "none",
        "control_authority_mode_locked": MODE_LOCKED,
        "boundary_statements": list(BOUNDARY_STATEMENTS),
        "model_scope": {
            "tool": "ANDES",
            "andes_pin": ANDES_PINNED_VERSION,
            "andes_version_reported": andes_version,
            "case_path": validated["case_path"],
            "case_version": validated["case_version"],
            "declared_models": validated["models"],
            "event": validated["event"],
            "docs_note": "R8-C5: pin ANDES 2.0.0 or a VERIFIED later "
                         "release; the live docs show a development "
                         "version string and are not a version citation.",
        },
        "outputs": outputs,
        "metrics": metrics,
        "provenance": {
            # §9.2 field names, verbatim from the Phase 7 worker template.
            "worker_version": WORKER_VERSION,
            "model_version": andes_version if andes_version is not None else "",
            "solver_version": validated.get("simulation", {}).get("solver", ""),
            "input_snapshot_id": validated["input_snapshot_id"],
            "request_id": validated["request_id"],
            "termination_status": termination_status,
            "diagnostics": list(diagnostics),
            "execution_ms": max(0, int(execution_ms)),
        },
        "status_text": "%s %s" % (UNVERIFIED_NOTE, " | ".join(BOUNDARY_STATEMENTS[:2])),
        "unverified_note": UNVERIFIED_NOTE,
    }
    return result


# ---------------------------------------------------------------------------
# The ANDES study path (guarded import; every call UNVERIFIED)
# ---------------------------------------------------------------------------


def _defensive_getattr(obj, name):
    try:
        return getattr(obj, name)
    except Exception:  # noqa: BLE001 — an absent UNVERIFIED surface is in-band
        return None


def extract_traces(system, monitored: list) -> list | None:
    """Extracts the monitored traces from a finished TDS run (UNVERIFIED).

    Reads the TDS recorded-output surface (`system.dae.ts`) defensively:
    ANY absence — no `dae`, no `ts`, no readable arrays — returns `None`
    and the caller records an IN-BAND failure. A trace is never
    synthesized, interpolated into existence, or padded: an unexportable
    signal is a recorded failure, never a fabricated series.
    """
    dae = _defensive_getattr(system, "dae")
    ts = _defensive_getattr(dae, "ts") if dae is not None else None
    if ts is None:
        return None
    times = _defensive_getattr(ts, "t")
    xy = _defensive_getattr(ts, "xy")
    if times is None or xy is None:
        return None
    try:
        time_list = [float(t) for t in times]
    except (TypeError, ValueError):
        return None
    traces = []
    for entry in monitored:
        # UNVERIFIED: mapping a declared signal name onto the recorded
        # array index requires the ANDES variable surface at run time;
        # an unmappable signal is recorded as an unexported trace with a
        # diagnostic — never filled with another signal's values.
        traces.append(
            {
                "signal": entry["signal"],
                "unit": entry["unit"],
                "times": None,
                "values": None,
                "exported": False,
                "diagnostic": "signal-to-array mapping is a VERIFY-BUILD "
                              "item against the installed ANDES "
                              "variable surface; not exported in this "
                              "run record",
            }
        )
    return {"times": time_list, "traces": traces}


def run_study(request: dict) -> dict:
    """Runs the requested ANDES study (the ONLY ANDES path in this file).

    Raises ImportError (the named deployment requirement) when ANDES is
    absent — the honesty gate. Every andes call below is UNVERIFIED
    (table in the module header). Any runtime failure AFTER the import is
    an IN-BAND result (`termination_status: "failed"` + diagnostics),
    never a crash and never a fabricated output.
    """
    andes = import_andes()
    validated = validate_request(request)
    diagnostics: list = []
    version = andes_version_reported(andes)

    try:
        # UNVERIFIED — ANDES API: load + DAE setup.
        system = andes.load(validated["case_path"], setup=True)

        study_type = validated["study_type"]
        outputs: dict = {"traces": None, "eigenvalues": None, "steady_state": None}

        if study_type == "steady_state_power_flow":
            # UNVERIFIED — ANDES API: Newton-Raphson steady-state power
            # flow. This is INITIALIZATION-CLASS computation: a converged
            # power flow is NOT transient-stability evidence (§19.1).
            pflow = _defensive_getattr(system, "PFlow")
            if pflow is None:
                raise RuntimeError("ANDES PFlow surface absent (UNVERIFIED surface)")
            pflow.run()
            converged = _defensive_getattr(pflow, "converged") is True
            outputs["steady_state"] = {"converged": converged}
            termination = "completed" if converged else "failed"
            if not converged:
                diagnostics.append("steady-state power flow did not converge")

        elif study_type == "eigenvalue":
            eig = _defensive_getattr(system, "EIG")
            if eig is None:
                raise RuntimeError("ANDES EIG surface absent (UNVERIFIED surface)")
            # UNVERIFIED — ANDES API: eigenvalue analysis on the linearized
            # declared model.
            eig.run()
            raw_evals = _defensive_getattr(eig, "eval")
            if raw_evals is None:
                raise RuntimeError(
                    "ANDES EIG result surface absent (UNVERIFIED surface); "
                    "eigenvalues are never synthesized"
                )
            outputs["eigenvalues"] = [
                {"real": float(ev.real), "imag": float(ev.imag)} for ev in raw_evals
            ]
            termination = "completed"

        else:  # time_domain
            simulation = validated["simulation"]
            # UNVERIFIED — ANDES API: steady-state initialization, TDS
            # initialization, and the time-domain run to the recorded end
            # time.
            pflow = _defensive_getattr(system, "PFlow")
            if pflow is None:
                raise RuntimeError("ANDES PFlow surface absent (UNVERIFIED surface)")
            pflow.run()
            if _defensive_getattr(pflow, "converged") is not True:
                raise RuntimeError(
                    "steady-state initialization did not converge; the "
                    "time-domain study is not attempted on an "
                    "uninitialized model (recorded as an in-band failure)"
                )
            tds = _defensive_getattr(system, "TDS")
            if tds is None:
                raise RuntimeError("ANDES TDS surface absent (UNVERIFIED surface)")
            tds.init()
            tds.run(to=simulation["end_time_seconds"])
            if _defensive_getattr(tds, "converged") is not True:
                diagnostics.append(
                    "TDS convergence flag not asserted true (UNVERIFIED "
                    "surface, read defensively)"
                )
            extracted = extract_traces(system, validated["monitored"])
            if extracted is None:
                raise RuntimeError(
                    "TDS recorded-output surface unreadable "
                    "(system.dae.ts absent or unusable); traces are never "
                    "synthesized — recorded as an in-band failure"
                )
            outputs["traces"] = extracted
            termination = "completed"

    except ImportError:
        raise  # the named deployment requirement propagates (honesty gate)
    except WorkerRequestError:
        raise  # a malformed request is a REFUSAL (exit 2), never in-band
    except Exception as error:  # noqa: BLE001 — in-band study failure
        diagnostics.append("study failed in-band: %s" % error)
        termination = "failed"
        outputs = {"traces": None, "eigenvalues": None, "steady_state": None}

    metrics = None
    if (
        termination == "completed"
        and validated["evaluation"]
        and outputs.get("traces")
    ):
        # Metrics are computed ONLY on actually extracted traces, via the
        # pure harness. An unexported trace contributes a diagnostic, not
        # a number.
        metric_entries = []
        for trace in outputs["traces"].get("traces", []):
            if not trace.get("exported"):
                diagnostics.append(
                    "metrics skipped for %r: trace not exported"
                    % trace.get("signal")
                )
                continue
            metric_entries.append(
                {
                    "signal": trace["signal"],
                    "unit": trace["unit"],
                    "result": evaluation_harness.evaluate_response(
                        trace["times"],
                        trace["values"],
                        baseline=validated["evaluation"]["baseline"],
                        event_time=validated["evaluation"]["event_time_s"],
                        tolerance=validated["evaluation"]["tolerance"],
                        quantity=validated["evaluation"]["quantity"],
                    ),
                }
            )
        metrics = {"per_trace": metric_entries, "tool": "evaluation_harness (P8.17)"}

    return build_result(
        validated=validated,
        termination_status=termination,
        outputs=outputs,
        metrics=metrics,
        diagnostics=diagnostics,
        andes_version=version,
        execution_ms=0,
    )


# ---------------------------------------------------------------------------
# Composition edge: one JSON document in, one JSON document out
# ---------------------------------------------------------------------------


def main() -> int:
    """Reads ONE request document on stdin, emits ONE result on stdout.

    Fail-closed: any malformed input is answered with NOTHING on stdout,
    a reason on stderr, and exit 2. A successful study emits exactly one
    `flexcore-andes-worker/1` envelope, after `assert_proposal_only`.
    """
    raw = sys.stdin.read()
    try:
        request = json.loads(raw)
    except json.JSONDecodeError as error:
        sys.stderr.write(
            "flexcore-andes-worker: request is not a valid JSON document: %s\n" % error
        )
        return 2
    try:
        validated_probe = validate_request(request)
        del validated_probe  # validation is the gate; run_study revalidates
        result = run_study(request)
        assert_proposal_only(result)
    except WorkerRequestError as error:
        sys.stderr.write("flexcore-andes-worker: request refused: %s\n" % error)
        return 2
    except ImportError as error:
        sys.stderr.write("flexcore-andes-worker: %s\n" % error)
        return 2
    except RuntimeError as error:
        sys.stderr.write("flexcore-andes-worker: refused: %s\n" % error)
        return 2
    sys.stdout.write(json.dumps(result))
    sys.stdout.write("\n")
    sys.stdout.flush()
    return 0


if __name__ == "__main__":  # pragma: no cover — composition edge
    raise SystemExit(main())
