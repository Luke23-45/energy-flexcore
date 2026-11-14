# P8.14 — The MARL simulation/shadow worker: an isolated, proposal-only,
#         NEVER-live-dispatch Class C research worker (mirrors the
#         andes_worker / gnn_inference_worker patterns).
#
# ══════════════════════════════════════════════════════════════════════════
# HONESTY HEADER — read before trusting anything below
#
# 1. THIS WORKER WAS WRITTEN, NEVER EXECUTED. The MARL framework stack is
#    a recorded DECISION-POINT (P8.03 §2: Ray/RLlib UNPINNED-REVERIFY — no
#    version verified from a primary source as of the 2026-11-07 cut-off,
#    RESEARCH.md §7 task 1; Gymnasium 1.3.0 recorded, license UNVERIFIED).
#    NOTHING is selected, pinned, installed, or verified in this
#    repository's history. `import ray` / `import rllib` /
#    `import gymnasium` are guarded INSIDE functions with the NAMED
#    ImportError constants below; the pure-logic core of this file (and
#    of `safety_filter.py` / `shadow_log.py`) imports, runs, and
#    self-tests with NOTHING installed — which is also the loop's honesty
#    gate: without the (never-selected) framework, NO policy inference
#    output can ever exist. Every training/inference result in this
#    repository is an EMPTY cell: NO reward curve, convergence statistic,
#    win-rate, or violation frequency exists anywhere in this directory,
#    and none may be fabricated (ENTRY_NOTE boundary 4).
# 2. THE BOUNDARY (PROHIBITED, not deferred — stated verbatim, carried in
#    every result envelope): "No live device commands from MARL — a
#    research MARL worker has no code path to the dispatch gateway; this
#    is prohibited, not deferred" (02_research_scope_freeze.md X-2;
#    ENTRY_NOTE boundary 1: "no research module has any field authority;
#    a MARL worker issuing live device commands is PROHIBITED, not
#    deferred"). IN CODE, not only in prose:
#    - the ONLY output channel is ONE JSON document on stdout; there is
#      no transport import of any kind (the pytest suite parses this
#      file's AST and pins the import set);
#    - no function whose name is dispatch/issue/authorize/approve/command-
#      shaped EXISTS in this module (pinned by an introspection test);
#    - the result TYPE cannot carry an authorization token or a device
#      command: `assert_proposal_only()` scans the whole result tree and
#      refuses emission of any forbidden key at any depth (ADR-010);
#    - an action whose permission is outside the closed ceiling is
#      refused with the named outcome `refused_permission_ceiling`
#      (safety_filter.py — no clamping turns a forbidden permission into
#      an allowed one);
#    - mode-locked to simulation/shadow by a CONSTANT: no request field
#      can select or raise a control-authority mode (§21.4, §5.7).
# 3. WHAT THE WORKER DOES (ledger P8.14 steps 1–4): shadow inference and
#    a deterministic filter probe, each producing a PROPOSAL into the
#    P8.05 validation-harness vocabulary (result_kind
#    candidate_setpoint_vector inside the closed P8.04 MARL payload
#    `flexcore.research.marl_shadow.v1`: result_kind, policy_version,
#    training_seed, environment_version, safety_filter_outcome), with a
#    shadow action logger (shadow_log.py) recording every proposed action,
#    its safety-filter outcome, and its simulated consequence — honestly
#    EMPTY (`consequence_status: "not_simulated"`) in this loop, filled by
#    the human/lab gate via P8.15.
#    - mode `filter_probe`: the request DECLARES a candidate action
#      (replayed from a recording or produced by a rule baseline); the
#      worker runs ONLY the pure safety filter + logging + envelope —
#      no framework, runnable today. This is the P8.15 harness's
#      deterministic filter oracle, never a policy output.
#    - mode `shadow_inference`: the action comes FROM the policy
#      (guarded framework import; the request may NOT inject an action —
#      a caller-injected action would bypass the policy, and is refused).
# 4. TRAINING IS UNREACHABLE HERE. The training loop is guarded behind
#    the framework import (`run_training`, the ONLY rllib/gymnasium
#    training surface) and CANNOT be reached in this repository: the
#    guarded import raises the NAMED DECISION-POINT requirement first.
#    Training results are EMPTY cells — never a reward curve, never a
#    convergence stat, never a win-rate.
# 5. THE SAFETY FILTER IS A SUBSET, NOT THE CBF LAYER (P8.13 §3): the
#    implemented filter (safety_filter.py) projects against the CLOSED
#    permission ceiling and declared P/ramp limits, and defines the
#    infeasible-filter outcome (fallback attempted, PROVABLY infeasible,
#    recorded failure — never an unfiltered pass-through). It does NOT
#    cover voltage/branch-current/SOC-dynamic constraints; the
#    control-barrier-function derivation is RECORDED AS NOT YET DERIVED
#    (13_marl_design.md §3) and TRAINING IS BLOCKED until a
#    power-systems reviewer signs it. No safety or certification claim is
#    made or implied anywhere in this directory.
# 6. TRANSPORT CONTRACT (mirrors the andes/gnn workers): ONE JSON
#    document on stdin (the `flexcore-marl-worker/1` request), ONE JSON
#    document on stdout (the result envelope). A malformed request is
#    answered with NOTHING on stdout + a reason on stderr + exit 2
#    (fail-closed). A refusal INSIDE a well-formed request (permission
#    outside the ceiling, non-finite proposal, infeasible filter) is
#    answered IN-BAND (termination_status "failed" + the named outcome +
#    the full filter report) — a refusal is a RESULT, never a crash and
#    never a silent pass-through.
# 7. CONTRACT SEAM (recorded, not invented): the worker reuses the Phase
#    7 template's §9.2 provenance field names verbatim and the CLOSED
#    P8.04 MARL result-key set; it binds no generated
#    `flexcore.worker.v1` gRPC servicer (buf generation remains the
#    activation step). The gaps (no field for the filtered setpoints
#    themselves; manifest/context-in-request stopgaps pending P8.20
#    artifact store / P8.32 signing / P8.35 provenance record) are
#    recorded in `ledger/phase-08-advanced-coordination/14_marl_worker.md`
#    §8 as Phase 7 follow-ups — never fixed by inventing a parallel
#    contract.
# ══════════════════════════════════════════════════════════════════════════

from __future__ import annotations

import ast
import inspect
import json
import os
import sys

import safety_filter
import shadow_log

# The worker-transport protocol tag (the mirror of `flexcore-andes-worker/1`
# and `flexcore-gnn-worker/1` for the MARL track; validated on every
# request, fail-closed).
PROTOCOL_ID = "flexcore-marl-worker/1"

WORKER_VERSION = "flexcore-marl-worker/0.1.0"

# Control-authority mode this worker is LOCKED to (§21.4; §5.7). A
# constant, not a setting: no request field can change it, no plugin can
# select a mode, and no research module can raise it.
MODE_LOCKED = "simulation-shadow"

# The two worker modes (closed): `filter_probe` is the pure, framework-free
# deterministic filter oracle (a DECLARED action is probed through the
# safety filter); `shadow_inference` is the policy path (UNREACHABLE here —
# framework DECISION-POINT). The two are never conflated: a filter_probe
# result is NEVER a policy output, and a shadow_inference result is never
# produced from a caller-injected action.
WORKER_MODES: frozenset = frozenset({"filter_probe", "shadow_inference"})

# The P8.04 MARL payload type (the closed research vocabulary).
PAYLOAD_TYPE = "flexcore.research.marl_shadow.v1"

# The result kind a completed proposal carries (closed §13.4 vocabulary;
# NAME-ALIGNED with `crates/flexcore-research/src/validation.rs::
# ResearchResultKind` — the recorded seam, the two MUST move together).
MARL_RESULT_KIND = "candidate_setpoint_vector"

# The closed §13.4 result-kind vocabulary (mirrored from the P8.04 template).
RESULT_KINDS: frozenset = frozenset(
    {
        "proposed_schedule",
        "candidate_setpoint_vector",
        "ranking",
        "forecast_distribution",
        "uncertainty_estimate",
        "diagnostics",
    }
)

# The CLOSED P8.04 module result-key set for the MARL payload type
# (mirrored verbatim from `python/workers/worker_template/research.py`):
# policy_version + training_seed + environment_version +
# safety_filter_outcome (+ result_kind). The result payload's keys MUST
# stay inside this set; a key outside it is a structured refusal.
MARL_RESULT_FIELDS: frozenset = frozenset(
    {
        "result_kind",
        "policy_version",
        "training_seed",
        "environment_version",
        "safety_filter_outcome",
    }
)

# The framework pin status, carried verbatim wherever the stack is named
# (P8.03 §2; RESEARCH.md §3/§7 task 1; nothing selected/installed/verified).
MARL_PIN_STATUS = (
    "DECISION-POINT OPEN — no Ray/RLlib/Gymnasium version is selected, "
    "pinned, installed, or verified (P8.03 section 2: Ray/RLlib "
    "UNPINNED-REVERIFY, no primary-source version as of the 2026-11-07 "
    "cut-off, RESEARCH.md section 7 task 1; Gymnasium 1.3.0 recorded, "
    "license UNVERIFIED); the P8.13 design record section 5 carries the "
    "RECOMMENDATION (RLlib NEW API stack, PyTorch leg), never a decision"
)

# The named deployment requirements a missing framework surfaces (raised
# by the guarded importers; `14_marl_worker.md` §6 repeats them verbatim).
RAY_RLLIB_DEPLOYMENT_REQUIREMENT = (
    "flexcore MARL worker requires the research RL framework: ray/rllib "
    "is NOT INSTALLED and the RL framework choice is a recorded "
    "DECISION-POINT (P8.03 section 2: Ray/RLlib UNPINNED-REVERIFY — no "
    "version verified from a primary source as of the 2026-11-07 cut-off, "
    "RESEARCH.md section 7 task 1; D4 OPEN). At the human build/test pass, "
    "pin the framework per the P8.13 design record section 5 (RECOMMENDED: "
    "RLlib NEW API stack on the PyTorch leg) into an ISOLATED research "
    "environment (never a system environment, never the platform runtime) "
    "after the P8.36 at-pin-time advisory check, and update the pin status"
)

GYMNASIUM_DEPLOYMENT_REQUIREMENT = (
    "flexcore MARL worker requires gymnasium (the environment API): NOT "
    "INSTALLED; Gymnasium 1.3.0 is RECORDED (RESEARCH.md section 3) but "
    "its license is UNVERIFIED until confirmed from the project's primary "
    "source at pin time. Pin only a verified-compatible version into the "
    "isolated research environment; never install into the platform runtime"
)


class WorkerRequestError(ValueError):
    """A malformed worker request — fail-closed BEFORE any computation."""


class PolicyArtifactError(RuntimeError):
    """The declared policy artifact is missing or unreadable — fails
    LOUDLY (named, like the GNN worker's ModelArtifactError): never
    absorbed, never answered with a default policy, never converted into
    a silent degraded result. In this repository the path can NEVER pass
    (no policy was ever trained), so the loud failure is the only
    reachable outcome on the shadow_inference path."""


# Keys that must NEVER appear anywhere in a result document (the in-code
# proposal-only boundary; P8.04 posture; ADR-010).
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

# The boundary statements carried VERBATIM in every result envelope.
BOUNDARY_STATEMENTS: tuple = (
    "No live device commands from MARL: a research MARL worker has no "
    "code path to the dispatch gateway; this is PROHIBITED, not deferred "
    "(02_research_scope_freeze.md X-2; ENTRY_NOTE boundary 1).",
    "This worker's outputs are research PROPOSALS only (ADR-010): the "
    "result type carries no authorization token and no device command, "
    "and every proposal enters the same validation -> feasibility -> "
    "authorization gates as any other candidate, gaining no authority "
    "from having been proposed (report 13.4).",
    "This worker is mode-locked to simulation/shadow; it cannot select "
    "or raise a control-authority mode (report 21.4; report 5.7).",
    "The implemented safety filter is a STATIC limit/permission projection "
    "(permission ceiling + P/ramp limits) and is NOT the "
    "control-barrier-function layer: the CBF derivation is recorded as "
    "NOT YET DERIVED (13_marl_design.md section 3) and TRAINING IS "
    "BLOCKED until a power-systems reviewer signs it. No safety or "
    "certification claim is made or implied.",
    "Filter infeasibility is never silent: the fallback is attempted, "
    "provably infeasible, and RECORDED (named outcome "
    "infeasible_no_safe_projection) — never an unfiltered action passed "
    "through (report 13.3; ledger P8.13 step 3).",
    "Zero observed violations from a finite test set is a test outcome, "
    "not a universal safety guarantee (report 13.3; "
    "02_research_scope_freeze.md X-4).",
    "The environment references crates/flexcore-simulator BY CITATION: "
    "this worker imports no crate, no Rust code, and no simulator — the "
    "environment is the P8.13 design's data contract over scenario "
    "artifacts and telemetry.read-class inputs.",
    "The RL framework is a recorded DECISION-POINT (P8.03 section 2): "
    "nothing is installed, pinned, or verified, so NO training or policy "
    "inference result exists here — every measured cell is EMPTY until "
    "the human/lab gate (P8.15).",
)

UNVERIFIED_NOTE = (
    "UNVERIFIED-API worker: no ray/rllib/gymnasium version is installed "
    "or pinned (DECISION-POINT, P8.03 section 2); every framework call "
    "below follows the documented RLlib NEW API stack / Gymnasium 1.x API "
    "shape as of the 2026-11-07 research cut-off and is UNVERIFIED. This "
    "worker was never executed. First execution belongs to the human "
    "build/test pass, after the framework is pinned per the P8.13 design "
    "record. Training results are EMPTY cells — never fabricated."
)


# ---------------------------------------------------------------------------
# Guarded framework imports — the single ray/gymnasium choke points, and
# the loop's honesty gate: without the (never-installed) framework, no
# policy output and no training run can ever exist.
# ---------------------------------------------------------------------------


def import_ray():
    """Imports `ray` (RLlib lives inside Ray) or raises the NAMED
    DECISION-POINT requirement. The import lives INSIDE this function so
    the rest of the module parses, imports, and self-tests with nothing
    installed."""
    try:
        import ray  # noqa: PLC0415 — deliberate deferred import (see header).
    except ImportError as error:
        raise ImportError(RAY_RLLIB_DEPLOYMENT_REQUIREMENT) from error
    return ray


def import_gymnasium():
    """Imports `gymnasium` or raises the NAMED deployment requirement."""
    try:
        import gymnasium  # noqa: PLC0415 — deliberate deferred import.
    except ImportError as error:
        raise ImportError(GYMNASIUM_DEPLOYMENT_REQUIREMENT) from error
    return gymnasium


def framework_version_reported(module) -> str | None:
    """The installed framework version, read defensively (UNVERIFIED
    surface; must never crash the result)."""
    try:
        version = getattr(module, "__version__", None)
        return None if version is None else str(version)
    except Exception:  # noqa: BLE001 — the probe must never crash the worker
        return None


# ---------------------------------------------------------------------------
# Pure request validation (no framework, no I/O beyond the given dict)
# ---------------------------------------------------------------------------


def _require_nonempty_str(container: dict, field: str, where: str) -> str:
    value = container.get(field)
    if not isinstance(value, str) or not value.strip():
        raise WorkerRequestError("%s.%s must be a non-empty string" % (where, field))
    return value


def _require_int_seed(container: dict, field: str, where: str) -> int:
    value = container.get(field)
    if isinstance(value, bool) or not isinstance(value, int):
        raise WorkerRequestError(
            "%s.%s must be an integer (P7.42 seed honesty: the caller "
            "declares the actual training seed, or the recorded "
            "no-training-provenance convention 0 for a policy-free "
            "action; the worker never invents one)" % (where, field)
        )
    return int(value)


def validate_policy_context(policy_context: object) -> dict:
    """The P8.04 payload provenance the caller must declare: policy_version
    (non-empty), training_seed (integer), environment_version (non-empty).
    Echoed on every output and into the shadow log — never invented."""
    if not isinstance(policy_context, dict):
        raise WorkerRequestError(
            "request.policy_context is required (policy_version, "
            "training_seed, environment_version — the P8.04 MARL payload "
            "vocabulary)"
        )
    policy_version = _require_nonempty_str(
        policy_context, "policy_version", "request.policy_context"
    )
    training_seed = _require_int_seed(
        policy_context, "training_seed", "request.policy_context"
    )
    environment_version = _require_nonempty_str(
        policy_context, "environment_version", "request.policy_context"
    )
    return {
        "policy_version": policy_version,
        "training_seed": training_seed,
        "environment_version": environment_version,
    }


def validate_request(request: dict) -> dict:
    """Validates the worker request; raises `WorkerRequestError` on
    refusal. Refusals happen BEFORE any framework import and before any
    filter computation. Returns the validated fields, normalized."""
    if not isinstance(request, dict):
        raise WorkerRequestError("request must be a JSON object")
    if request.get("protocol") != PROTOCOL_ID:
        raise WorkerRequestError(
            "request protocol %r is not %r" % (request.get("protocol"), PROTOCOL_ID)
        )
    mode = request.get("mode")
    if mode not in WORKER_MODES:
        raise WorkerRequestError(
            "request.mode %r is not one of %s (filter_probe is the pure "
            "filter oracle; shadow_inference is the policy path — the "
            "two are never conflated)" % (mode, sorted(WORKER_MODES))
        )
    policy_context = validate_policy_context(request.get("policy_context"))

    action = None
    policy_manifest = {}
    if mode == "filter_probe":
        if request.get("action") is None:
            raise WorkerRequestError(
                "a filter_probe request requires request.action (the "
                "DECLARED candidate action to probe — from a recording or "
                "a rule baseline, never a policy output)"
            )
        # Full structural validation now: malformed input is a refusal,
        # not an outcome (safety_filter.FilterInputError is normalized
        # into WorkerRequestError here so the composition edge stays
        # fail-closed with one refusal type).
        try:
            action = safety_filter.validate_action(request.get("action"))
        except safety_filter.FilterInputError as error:
            raise WorkerRequestError(str(error)) from error
    else:  # shadow_inference
        if request.get("action") is not None:
            raise WorkerRequestError(
                "a shadow_inference request must NOT carry request.action: "
                "the action comes FROM the policy, and a caller-injected "
                "action would bypass it (refused — this is the P8.14 "
                "boundary, not a convenience)"
            )
        manifest = request.get("policy_manifest")
        if not isinstance(manifest, dict):
            raise WorkerRequestError(
                "a shadow_inference request requires request.policy_manifest "
                "(policy_version, policy_artifact_ref, training_data_ref, "
                "observation_scope)"
            )
        policy_manifest = {
            "policy_version": _require_nonempty_str(
                manifest, "policy_version", "request.policy_manifest"
            ),
            "policy_artifact_ref": _require_nonempty_str(
                manifest, "policy_artifact_ref", "request.policy_manifest"
            ),
            "training_data_ref": _require_nonempty_str(
                manifest, "training_data_ref", "request.policy_manifest"
            ),
            "observation_scope": _require_nonempty_str(
                manifest, "observation_scope", "request.policy_manifest"
            ),
        }

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
        "mode": mode,
        "policy_context": policy_context,
        "action": action,
        "policy_manifest": policy_manifest,
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
    an authorization token or a device command, and this is the enforced
    proof, not prose. The shadow log travels INSIDE the envelope and is
    scanned with everything else.
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


def enforce_marl_result_schema(result: dict) -> None:
    """The CLOSED P8.04 result-key set for the MARL payload type (pure).

    Mirrors `research.enforce_result_schema` for payload type
    `flexcore.research.marl_shadow.v1`: every key inside
    `MARL_RESULT_FIELDS`; `result_kind` in the closed §13.4 vocabulary;
    `policy_version`/`environment_version` non-empty; `training_seed` an
    integer (bool excluded); `safety_filter_outcome` inside the closed
    filter-outcome vocabulary (the LOAD-BEARING key: a result without a
    named filter outcome is unrepresentable). Raises `WorkerRequestError`
    (a contract refusal, safe to wire).
    """
    if not isinstance(result, dict):
        raise WorkerRequestError("a MARL result payload must be a mapping")
    unknown = sorted(set(result) - MARL_RESULT_FIELDS)
    if unknown:
        raise WorkerRequestError(
            "result keys outside the closed %s schema: %s" % (PAYLOAD_TYPE, unknown)
        )
    kind = result.get("result_kind")
    if not isinstance(kind, str) or kind not in RESULT_KINDS:
        raise WorkerRequestError(
            "result_kind %r is not in the closed §13.4 vocabulary" % (kind,)
        )
    for field in ("policy_version", "environment_version"):
        value = result.get(field)
        if not isinstance(value, str) or not value.strip():
            raise WorkerRequestError(
                "result.%s must be a non-empty string (the P8.04 MARL "
                "payload vocabulary)" % field
            )
    seed = result.get("training_seed")
    if isinstance(seed, bool) or not isinstance(seed, int):
        raise WorkerRequestError(
            "result.training_seed must be an integer (P7.42 seed honesty)"
        )
    outcome = result.get("safety_filter_outcome")
    if not isinstance(outcome, str) or outcome not in safety_filter.FILTER_OUTCOMES:
        raise WorkerRequestError(
            "result.safety_filter_outcome %r is not in the closed "
            "vocabulary %s (the load-bearing key: a result without a "
            "named filter outcome is unrepresentable)"
            % (outcome, sorted(safety_filter.FILTER_OUTCOMES))
        )


def check_policy_artifact_path(artifact_path: object) -> str:
    """The pure part of the missing-policy failure mode (mirrors the GNN
    worker): validates that the manifest's policy artifact resolves to an
    existing file path and raises the NAMED `PolicyArtifactError` when it
    does not. In this repository the path can NEVER pass (no policy was
    ever trained), so the loud failure is the only reachable outcome.
    """
    if not isinstance(artifact_path, str) or not artifact_path.strip():
        raise PolicyArtifactError(
            "policy artifact ref is missing or empty: a shadow-inference "
            "request without a declared, versioned policy artifact fails "
            "LOUDLY (ledger P8.14); artifact store refs are the P8.20 seam"
        )
    if artifact_path.startswith("object-store:"):
        # An object-store ref (P8.20) is resolved by the worker runtime at
        # activation time; resolution is a VERIFY-BUILD item (UNVERIFIED).
        return artifact_path
    if not os.path.isfile(artifact_path):
        raise PolicyArtifactError(
            "policy artifact file not found: %r — a missing policy file "
            "fails loudly (ledger P8.14); there is no default policy and "
            "no degraded-mode inference" % artifact_path
        )
    return artifact_path


def build_result(
    *,
    validated: dict,
    termination_status: str,
    filter_outcome: str | None,
    outputs: dict | None,
    diagnostics: list,
    ray_version: str | None,
    gymnasium_version: str | None,
    execution_ms: int,
) -> dict:
    """Builds the `flexcore-marl-worker/1` result envelope (pure).

    `termination_status` is "completed" only when the filter ACCEPTED the
    action (passed or clamped — the proposal is emitted into the P8.05
    validation-harness vocabulary) or, on the shadow_inference path, the
    policy actually produced and filtered an action (UNREACHABLE here).
    "failed" marks an in-band refusal (permission outside the ceiling,
    non-finite proposal, infeasible filter) WITH the named outcome and the
    full filter report — a refusal is a RESULT, never a crash, never a
    silent pass-through, never a fabricated figure.
    """
    envelope = {
        "protocol": PROTOCOL_ID,
        "kind": "marl_shadow_result",
        "ok": termination_status == "completed",
        "mode": validated["mode"],
        "payload_type": PAYLOAD_TYPE,
        # Proposal-only posture (ADR-010), enforced in code by
        # `assert_proposal_only` before emission.
        "proposal_only": True,
        "authority": "none",
        "control_authority_mode_locked": MODE_LOCKED,
        "boundary_statements": list(BOUNDARY_STATEMENTS),
        "policy_scope": {
            "worker_mode": validated["mode"],
            "payload_type": PAYLOAD_TYPE,
            "result_kind": MARL_RESULT_KIND,
            "policy_version": validated["policy_context"]["policy_version"],
            "training_seed": validated["policy_context"]["training_seed"],
            "environment_version": validated["policy_context"][
                "environment_version"
            ],
            "framework_pin_status": MARL_PIN_STATUS,
            "ray_version_reported": ray_version,
            "gymnasium_version_reported": gymnasium_version,
            "training_status": (
                "EMPTY — no training run exists in this repository; no "
                "reward curve, convergence statistic, or win-rate is "
                "recorded anywhere (ENTRY_NOTE boundary 4)"
            ),
        },
        "filter_outcome": filter_outcome,
        "outputs": outputs,
        "provenance": {
            # §9.2 field names, verbatim from the Phase 7 worker template.
            "worker_version": WORKER_VERSION,
            "model_version": validated["policy_context"]["policy_version"],
            "solver_version": "none-research-worker",
            "input_snapshot_id": validated["input_snapshot_id"],
            "request_id": validated["request_id"],
            "termination_status": termination_status,
            "diagnostics": list(diagnostics),
            "execution_ms": max(0, int(execution_ms)),
        },
        "status_text": "%s | %s"
        % (UNVERIFIED_NOTE, BOUNDARY_STATEMENTS[0]),
        "unverified_note": UNVERIFIED_NOTE,
    }
    return envelope


def _log_entry_for(validated: dict, report: dict, sequence: int) -> dict:
    """Builds the shadow-log entry for ONE filtered action (pure; the
    ledger P8.14 step-1 logger: action + filter outcome + consequence)."""
    return shadow_log.build_log_entry(
        sequence=sequence,
        request_id=validated["request_id"],
        input_snapshot_id=validated["input_snapshot_id"],
        worker_mode=validated["mode"],
        policy_version=validated["policy_context"]["policy_version"],
        training_seed=validated["policy_context"]["training_seed"],
        environment_version=validated["policy_context"]["environment_version"],
        action_kind=report["action_kind"],
        proposed_permission=report["proposed_permission"],
        safety_filter_outcome=report["action_outcome"],
        per_entry_outcomes=[
            {
                "asset_id": entry["asset_id"],
                "entry_outcome": entry["entry_outcome"],
                "p_current_mw": entry["p_current_mw"],
                "p_proposed_mw": entry["p_proposed_mw"],
                "p_filtered_mw": entry["p_filtered_mw"],
            }
            for entry in report["entries"]
        ],
        filter_activations=list(report["filter_activations"]),
        infeasibility=report["infeasibility"],
    )


# ---------------------------------------------------------------------------
# The worker paths
# ---------------------------------------------------------------------------


def run_filter_probe(request: dict) -> dict:
    """The PURE path: probe a DECLARED candidate action through the safety
    filter, log it, and emit the refusal-or-proposal envelope.

    No framework is touched: this path is runnable today with nothing
    installed and is the P8.15 evaluation harness's deterministic filter
    oracle. A refused action is an IN-BAND result (termination "failed")
    with the named outcome — never a crash, never a silent pass-through.
    """
    validated = validate_request(request)
    diagnostics: list = []
    report = safety_filter.filter_action(validated["action"])
    outcome = report["action_outcome"]
    if outcome not in safety_filter.FILTER_OUTCOMES:  # pragma: no cover
        raise RuntimeError(
            "filter produced an outcome outside the closed vocabulary: %r"
            % (outcome,)
        )

    entry = _log_entry_for(validated, report, sequence=1)
    log = [entry]
    summary = shadow_log.log_summary(log)

    if outcome in safety_filter.REFUSING_OUTCOMES:
        diagnostics.append(
            "action refused in-band: %s%s"
            % (
                outcome,
                " (fallback attempted, not viable)"
                if outcome == "infeasible_no_safe_projection"
                else "",
            )
        )
        outputs = {
            "filter_report": report,
            "proposal": None,
            "result_payload": None,
            "shadow_log": log,
            "shadow_log_summary": summary,
            "shadow_log_reference": (
                "in-envelope:sequence-1..1 (documented stopgap pending "
                "the P8.20 artifact store)"
            ),
        }
        return build_result(
            validated=validated,
            termination_status="failed",
            filter_outcome=outcome,
            outputs=outputs,
            diagnostics=diagnostics,
            ray_version=None,
            gymnasium_version=None,
            execution_ms=0,
        )

    # Accepted (passed_within_limits / filtered_clamped_to_limits): the
    # FILTERED action is the emitted proposal — a candidate_setpoint_vector
    # into the P8.05 validation-harness vocabulary, carrying the closed
    # P8.04 payload. The filtered setpoints travel in `outputs.proposal`
    # (the closed P8.04 key set has no field for them — the recorded
    # Phase 7 follow-up; the payload carries the contract keys).
    payload = {
        "result_kind": MARL_RESULT_KIND,
        "policy_version": validated["policy_context"]["policy_version"],
        "training_seed": validated["policy_context"]["training_seed"],
        "environment_version": validated["policy_context"]["environment_version"],
        "safety_filter_outcome": outcome,
    }
    enforce_marl_result_schema(payload)
    outputs = {
        "filter_report": report,
        "proposal": {
            "action_kind": report["action_kind"],
            "proposed_permission": report["proposed_permission"],
            "setpoints": [
                {
                    "asset_id": entry["asset_id"],
                    "p_current_mw": entry["p_current_mw"],
                    "p_proposed_mw": entry["p_proposed_mw"],
                    "p_filtered_mw": entry["p_filtered_mw"],
                }
                for entry in report["entries"]
            ],
            "proposal_statement": (
                "a candidate_setpoint_vector PROPOSAL (ADR-010) into the "
                "P8.05 validation harness: it enters at lifecycle state "
                "PROPOSED through the same validation -> feasibility -> "
                "authorization gates as any other candidate and gains no "
                "authority from having been proposed; it is never issued "
                "to a device by this worker"
            ),
        },
        "result_payload": payload,
        "shadow_log": log,
        "shadow_log_summary": summary,
        "shadow_log_reference": (
            "in-envelope:sequence-1..1 (documented stopgap pending the "
            "P8.20 artifact store)"
        ),
    }
    return build_result(
        validated=validated,
        termination_status="completed",
        filter_outcome=outcome,
        outputs=outputs,
        diagnostics=diagnostics,
        ray_version=None,
        gymnasium_version=None,
        execution_ms=0,
    )


def run_training(policy_manifest: dict) -> dict:
    """The training path — UNREACHABLE in this repository (the loop's
    honesty gate).

    The ledger P8.14 step 1 names training as an OFFLINE step; training
    is guarded behind the framework import and CANNOT run here: the
    guarded import raises the NAMED DECISION-POINT requirement before any
    training code is reached. NO reward curve, convergence statistic, or
    win-rate exists or may exist (EMPTY cells; ENTRY_NOTE boundary 4).
    The signature is kept minimal on purpose: a real training entrypoint
    (dataset ref per P8.35, hyperparameters pre-registered per P8.02 §5 /
    X-8, seeds per P8.38) belongs to the human gate's recorded decision —
    never invented here.
    """
    # The guarded imports ARE the gate: without ray/rllib and gymnasium
    # (this repository's permanent state until the human pin), nothing
    # below is reachable.
    ray = import_ray()
    gymnasium = import_gymnasium()
    del ray, gymnasium  # UNVERIFIED — real training code is the human gate's
    raise RuntimeError(
        "training is not implemented in this loop: the framework "
        "DECISION-POINT is OPEN (P8.03 section 2), the CBF safety-layer "
        "derivation is NOT YET SIGNED (13_marl_design.md section 3 blocks "
        "training), and the training-data provenance record (P8.35) does "
        "not exist. Training results are EMPTY cells — never fabricated."
    )


def run_shadow_inference(request: dict) -> dict:
    """The policy path (the ONLY ray/gymnasium path in this file).

    Order of gates:
      1. pure request validation (refusals before anything else; a
         caller-injected action is refused here — the action must come
         FROM the policy);
      2. the guarded ray import (named DECISION-POINT requirement) — the
         honesty gate dominates: with no framework installed (this
         repository's permanent state until the human pin), NO further
         gate is reachable;
      3. the guarded gymnasium import (named requirement);
      4. the policy-artifact check (missing file fails LOUDLY, named
         PolicyArtifactError);
      5. the UNVERIFIED inference steps (artifact load, policy forward,
         action extraction) — unreachable in this repository;
      6. the SAME pure safety filter (safety_filter.filter_action) — the
         policy's raw action NEVER bypasses the filter, and the same
         refusal/acceptance envelope as run_filter_probe is produced.
    """
    validated = validate_request(request)
    # Gate 2/3: guarded framework imports (named requirements).
    ray = import_ray()
    gymnasium = import_gymnasium()
    ray_version = framework_version_reported(ray)
    gymnasium_version = framework_version_reported(gymnasium)

    # Gate 4: missing policy artifact fails loudly (named error) —
    # checked AFTER the framework gate, so the DECISION-POINT requirement
    # is the first honest refusal on this machine.
    check_policy_artifact_path(validated["policy_manifest"]["policy_artifact_ref"])

    # Gates 5–6 are UNREACHABLE in this repository. The with-framework
    # implementation belongs to the human build/test pass: load the
    # signed policy artifact (P8.32 seam), run the policy forward in
    # inference mode over the declared observation (telemetry.read-class
    # inputs only), extract the action, then run the SAME pure filter.
    # Marked UNVERIFIED; never stubbed with a fabricated action.
    raise PolicyArtifactError(
        "shadow inference is UNREACHABLE in this repository: the with-"
        "framework implementation (artifact load, policy forward, action "
        "extraction) is the human build/test pass's VERIFY-BUILD work "
        "after the framework DECISION-POINT is resolved; this worker "
        "never fabricates an action"
    )


def run_worker(request: dict) -> dict:
    """Dispatches ONE request to its path (the mode is the only switch;
    the two modes are never conflated)."""
    validated_probe = validate_request(request)
    if validated_probe["mode"] == "filter_probe":
        return run_filter_probe(request)
    return run_shadow_inference(request)


# ---------------------------------------------------------------------------
# Composition edge: one JSON document in, one JSON document out
# ---------------------------------------------------------------------------


def main() -> int:
    """Reads ONE request document on stdin, emits ONE result on stdout.

    Fail-closed: any malformed input is answered with NOTHING on stdout,
    a reason on stderr, and exit 2. An accepted filter probe emits exactly
    one `flexcore-marl-worker/1` envelope, after `assert_proposal_only`.
    """
    raw = sys.stdin.read()
    try:
        request = json.loads(raw)
    except json.JSONDecodeError as error:
        sys.stderr.write(
            "flexcore-marl-worker: request is not a valid JSON document: %s\n"
            % error
        )
        return 2
    try:
        validation_probe = validate_request(request)
        del validation_probe  # validation is the gate; run_worker revalidates
        result = run_worker(request)
        assert_proposal_only(result)
    except WorkerRequestError as error:
        sys.stderr.write("flexcore-marl-worker: request refused: %s\n" % error)
        return 2
    except ImportError as error:
        sys.stderr.write("flexcore-marl-worker: %s\n" % error)
        return 2
    except PolicyArtifactError as error:
        sys.stderr.write("flexcore-marl-worker: %s\n" % error)
        return 2
    except RuntimeError as error:
        sys.stderr.write("flexcore-marl-worker: refused: %s\n" % error)
        return 2
    sys.stdout.write(json.dumps(result))
    sys.stdout.write("\n")
    sys.stdout.flush()
    return 0


# ---------------------------------------------------------------------------
# In-module self-tests for the PURE-LOGIC core (hand-computed; NEVER RUN
# in this loop). Run instruction (human gate):
#   python apps/worker/python/marl_worker/marl_worker.py
# (the pytest suite mirrors these cases in tests/test_marl_worker.py).
# ---------------------------------------------------------------------------


def _limits(p_min: float, p_max: float, ramp: float) -> dict:
    return {
        "p_min_mw": p_min,
        "p_max_mw": p_max,
        "ramp_mw_per_interval": ramp,
    }


def _valid_request(**overrides) -> dict:
    """The canonical valid filter_probe request used by the self-tests
    (all figures are CONSTRUCTED test inputs, never measurements)."""
    request = {
        "protocol": PROTOCOL_ID,
        "mode": "filter_probe",
        "policy_context": {
            # training_seed 0 is the documented no-training-provenance
            # convention, declared by the CALLER (the worker never
            # invents a seed; see _require_int_seed).
            "policy_version": "RECORD-AT-FIRST-RUN",
            "training_seed": 0,
            "environment_version": "flexcore-marl-env/0.1.0",
        },
        "action": {
            "action_kind": "setpoint_delta",
            "proposed_permission": "schedule.propose",
            "assets": [
                {
                    "asset_id": "ess-1",
                    "p_current_mw": 2.0,
                    "p_proposed_mw": 7.0,
                    "soc_fraction": 0.5,
                    "limits": _limits(0.0, 5.0, 1.5),
                }
            ],
        },
        "provenance": {
            "input_snapshot_id": "snapshot-RECORD",
            "request_id": "req-0001",
        },
    }
    request.update(overrides)
    return request


def run_self_tests() -> list:
    """Runs every hand-computed self-test; returns one record per test.

    Each record: {"name", "passed", "detail"}. This function itself has
    NEVER been executed in this loop; the expectations are hand
    derivations from the constructed inputs, never observed outputs.
    """
    results: list = []

    def check(name: str, condition: bool, detail: str) -> None:
        results.append({"name": name, "passed": bool(condition), "detail": detail})

    # ---- Test 1: a clamped filter_probe completes end to end ------------
    # HAND CHECK: p_current 2.0, ramp 1.5, limits [0, 5] -> interval
    #   [0.5, 3.5]; proposed 7.0 clamps to 3.5 (= min(5.0, 2.0+1.5): the
    #   ramp binds). Envelope: completed, outcome
    #   filtered_clamped_to_limits, payload carries the closed P8.04 keys,
    #   log entry records the activation, consequence not_simulated.
    envelope = run_filter_probe(_valid_request())
    entry = envelope["outputs"]["shadow_log"][0]
    check(
        "filter_probe_clamped_completes",
        envelope["ok"] is True
        and envelope["provenance"]["termination_status"] == "completed"
        and envelope["filter_outcome"] == "filtered_clamped_to_limits"
        and envelope["outputs"]["proposal"]["setpoints"][0]["p_filtered_mw"] == 3.5
        and envelope["outputs"]["result_payload"]["safety_filter_outcome"]
        == "filtered_clamped_to_limits"
        and envelope["outputs"]["result_payload"]["result_kind"]
        == "candidate_setpoint_vector"
        and entry["safety_filter_outcome"] == "filtered_clamped_to_limits"
        and len(entry["filter_activations"]) == 1
        and entry["consequence_status"] == "not_simulated"
        and entry["simulated_consequence"] is None
        and entry["lifecycle_state"] == "PROPOSED",
        "proposed 7.0 -> filtered 3.5; completed envelope; payload keys "
        "closed; log records action + outcome + activation + "
        "not_simulated consequence",
    )
    try:
        enforce_marl_result_schema(envelope["outputs"]["result_payload"])
        payload_ok = True
    except WorkerRequestError:
        payload_ok = False
    check(
        "filter_probe_payload_matches_closed_p804_schema",
        payload_ok,
        "the emitted payload passes the closed P8.04 MARL schema check",
    )
    try:
        shadow_log.validate_shadow_log(envelope["outputs"]["shadow_log"])
        log_ok = True
    except shadow_log.ShadowLogError:
        log_ok = False
    check(
        "filter_probe_shadow_log_valid",
        log_ok and envelope["outputs"]["shadow_log_summary"]["total_actions"] == 1,
        "the shadow log validates (sequence 1) and the summary counts "
        "one action",
    )

    # ---- Test 2: permission outside the ceiling -> in-band refusal ------
    refused_request = _valid_request()
    refused_request["action"]["proposed_permission"] = "dispatch.issue"
    envelope = run_filter_probe(refused_request)
    check(
        "filter_probe_permission_refused_in_band",
        envelope["ok"] is False
        and envelope["provenance"]["termination_status"] == "failed"
        and envelope["filter_outcome"] == "refused_permission_ceiling"
        and envelope["outputs"]["proposal"] is None
        and envelope["outputs"]["result_payload"] is None
        and envelope["outputs"]["shadow_log"][0]["safety_filter_outcome"]
        == "refused_permission_ceiling"
        and envelope["outputs"]["shadow_log_summary"]["refusal_count"] == 1,
        "'dispatch.issue' -> in-band failed envelope, named outcome, "
        "NO proposal emitted, refusal logged",
    )

    # ---- Test 3: infeasible filter -> fallback attempted, recorded ------
    # HAND CHECK: p_current 3.0 outside [1.0, 2.0], ramp 0.5 -> interval
    #   [max(1.0, 2.5), min(2.0, 3.5)] = [2.5, 2.0] EMPTY -> infeasible;
    #   fallback (hold-current 3.0) provably outside [1.0, 2.0] -> not
    #   viable; infeasibility recorded on the log entry.
    infeasible_request = _valid_request()
    infeasible_request["action"]["assets"][0] = {
        "asset_id": "ess-1",
        "p_current_mw": 3.0,
        "p_proposed_mw": 3.0,
        "soc_fraction": None,
        "limits": _limits(1.0, 2.0, 0.5),
    }
    envelope = run_filter_probe(infeasible_request)
    log_entry = envelope["outputs"]["shadow_log"][0]
    check(
        "filter_probe_infeasible_recorded_never_silent",
        envelope["ok"] is False
        and envelope["filter_outcome"] == "infeasible_no_safe_projection"
        and envelope["outputs"]["proposal"] is None
        and log_entry["infeasibility"] is not None
        and log_entry["infeasibility"]["recorded"] is True
        and log_entry["infeasibility"]["outcome"] == "infeasible_no_safe_projection"
        and envelope["outputs"]["filter_report"]["fallback"]["attempted"] is True
        and envelope["outputs"]["filter_report"]["fallback"]["viable"] is False
        and envelope["outputs"]["shadow_log_summary"]["infeasibility_count"] == 1,
        "interval [2.5, 2.0] EMPTY -> named infeasible outcome; fallback "
        "attempted and recorded NOT viable; the infeasibility is on the "
        "log entry (never silent)",
    )

    # ---- Test 4: the envelope's fixed posture and verbatim boundaries ---
    envelope = run_filter_probe(_valid_request())
    prohibited_present = any(
        "PROHIBITED, not deferred" in statement
        for statement in envelope["boundary_statements"]
    )
    finite_test_set_present = any(
        "not a universal safety guarantee" in statement
        for statement in envelope["boundary_statements"]
    )
    cbf_present = any(
        "NOT YET DERIVED" in statement and "TRAINING IS BLOCKED" in statement
        for statement in envelope["boundary_statements"]
    )
    check(
        "envelope_fixed_posture_and_verbatim_boundaries",
        envelope["proposal_only"] is True
        and envelope["authority"] == "none"
        and envelope["control_authority_mode_locked"] == "simulation-shadow"
        and MODE_LOCKED == "simulation-shadow"
        and prohibited_present
        and finite_test_set_present
        and cbf_present
        and envelope["policy_scope"]["training_status"].startswith("EMPTY"),
        "the envelope carries proposal_only / authority none / mode lock "
        "and the PROHIBITED-not-deferred, finite-test-set, and "
        "CBF-not-yet-derived statements verbatim",
    )
    try:
        assert_proposal_only(envelope)
        envelope_clean = True
    except RuntimeError:
        envelope_clean = False
    check(
        "envelope_passes_proposal_only",
        envelope_clean,
        "the built envelope passes the proposal-only scan",
    )

    # ---- Test 5: proposal-only boundary scans the whole tree ------------
    deep = {"outputs": {"shadow_log": [{"sequence": 1, "dispatch": {"p": 1.0}}]}}
    _expects_runtime_error(
        results,
        lambda: assert_proposal_only(deep),
        "proposal_only_nested_forbidden_key",
        "a nested 'dispatch' key anywhere in the tree refuses emission",
    )

    # ---- Test 6: NO dispatch-shaped callable exists in this module ------
    # The ledger P8.14 step 4 boundary test (inspection side): the module
    # namespace must contain no callable whose name is dispatch/issue/
    # authorize/approve/command-shaped — there is no code path to name.
    offending = [
        name
        for name, value in vars(sys.modules[__name__]).items()
        if callable(value)
        and not name.startswith("_")
        and any(
            token in name.lower()
            for token in ("dispatch", "issue", "authorize", "approve", "command")
        )
    ]
    check(
        "no_dispatch_shaped_callable_in_module",
        offending == [],
        "introspected module namespace: %r — no dispatch/issue/"
        "authorize/approve/command-shaped callable exists" % (offending,),
    )

    # ---- Test 7: the composition edge imports NO transport --------------
    # The AST of THIS file must import only stdlib + the two sibling pure
    # modules; ray/gymnasium appear ONLY inside function bodies (guarded).
    source = inspect.getsource(sys.modules[__name__])
    tree = ast.parse(source)
    allowed_roots = {
        "__future__",
        "json",
        "os",
        "sys",
        "ast",
        "inspect",
        "safety_filter",
        "shadow_log",
    }
    top_level_roots: list = []
    guarded_roots: list = []
    for node in tree.body:
        if isinstance(node, ast.Import):
            top_level_roots.extend(alias.name.split(".")[0] for alias in node.names)
        elif isinstance(node, ast.ImportFrom) and node.module:
            top_level_roots.append(node.module.split(".")[0])
    for node in ast.walk(tree):
        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)):
            for inner in ast.walk(node):
                if isinstance(inner, ast.Import):
                    guarded_roots.extend(
                        alias.name.split(".")[0] for alias in inner.names
                    )
                elif isinstance(inner, ast.ImportFrom) and inner.module:
                    guarded_roots.append(inner.module.split(".")[0])
    check(
        "module_imports_stdlib_and_siblings_only",
        all(root in allowed_roots for root in top_level_roots)
        and set(guarded_roots) <= {"ray", "gymnasium"},
        "top-level imports %s are stdlib/siblings; guarded in-function "
        "imports %s are the framework gates only (no transport, no "
        "dispatch client, no simulator import exists)"
        % (sorted(set(top_level_roots)), sorted(set(guarded_roots))),
    )

    # ---- Test 8: closed result schema refusals --------------------------
    canonical = {
        "result_kind": "candidate_setpoint_vector",
        "policy_version": "RECORD-AT-FIRST-RUN",
        "training_seed": 0,
        "environment_version": "flexcore-marl-env/0.1.0",
        "safety_filter_outcome": "filtered_clamped_to_limits",
    }
    try:
        enforce_marl_result_schema(canonical)
        schema_ok = True
    except WorkerRequestError:
        schema_ok = False
    check(
        "marl_result_schema_canonical_passes",
        schema_ok,
        "the canonical P8.04 MARL payload passes the closed schema",
    )

    def unknown_key() -> None:
        smuggled = dict(canonical)
        smuggled["dispatch_vector"] = [1.0]
        enforce_marl_result_schema(smuggled)

    _expects_request_error(results, unknown_key, "marl_result_unknown_key_refused")

    def outcome_outside_vocabulary() -> None:
        wrong = dict(canonical)
        wrong["safety_filter_outcome"] = "mostly_fine"
        enforce_marl_result_schema(wrong)

    _expects_request_error(
        results, outcome_outside_vocabulary, "marl_result_outcome_outside_vocabulary"
    )

    def bool_seed() -> None:
        wrong = dict(canonical)
        wrong["training_seed"] = True
        enforce_marl_result_schema(wrong)

    _expects_request_error(results, bool_seed, "marl_result_bool_seed_refused")

    def missing_outcome() -> None:
        wrong = dict(canonical)
        del wrong["safety_filter_outcome"]
        enforce_marl_result_schema(wrong)

    _expects_request_error(
        results, missing_outcome, "marl_result_missing_filter_outcome_refused"
    )

    # ---- Test 9: request refusals (fail-closed before anything) ---------
    def wrong_protocol() -> None:
        validate_request(_valid_request(protocol="flexcore-marl-worker/0"))

    _expects_request_error(results, wrong_protocol, "wrong_protocol_refused")

    def action_in_shadow_mode() -> None:
        request = _valid_request(mode="shadow_inference")
        request["policy_manifest"] = {
            "policy_version": "RECORD-AT-FIRST-RUN",
            "policy_artifact_ref": "object-store:policies/RECORD-REF",
            "training_data_ref": "object-store:training-data/RECORD-REF",
            "observation_scope": "balanced-radial-feeder/RECORD-CLASS",
        }
        validate_request(request)

    _expects_request_error(
        results, action_in_shadow_mode, "caller_injected_action_in_shadow_mode_refused"
    )

    def missing_action_in_probe() -> None:
        request = _valid_request()
        del request["action"]
        validate_request(request)

    _expects_request_error(
        results, missing_action_in_probe, "missing_action_in_filter_probe_refused"
    )

    def malformed_limits_refused() -> None:
        request = _valid_request()
        request["action"]["assets"][0]["limits"] = _limits(5.0, 1.0, 1.0)
        validate_request(request)

    _expects_request_error(
        results, malformed_limits_refused, "inverted_limits_normalized_to_refusal"
    )

    def bad_permission_type() -> None:
        validate_policy_context({"policy_version": "p", "training_seed": 0})

    _expects_request_error(
        results, bad_permission_type, "missing_environment_version_refused"
    )

    # ---- Test 10: missing policy artifact fails LOUDLY -------------------
    # HAND CHECK: no policy artifact exists in this repository (nothing
    # was ever trained), so the named PolicyArtifactError is the only
    # reachable outcome for a filesystem path.
    _expects_policy_artifact_error(
        results,
        lambda: check_policy_artifact_path("models/marl/never-trained-policy"),
        "policy_artifact_missing_fails_loudly",
        "a nonexistent policy artifact raises the named PolicyArtifactError",
    )
    _expects_policy_artifact_error(
        results,
        lambda: check_policy_artifact_path(""),
        "policy_artifact_empty_ref_fails_loudly",
        "an empty artifact ref raises the named PolicyArtifactError",
    )

    # ---- Test 11: shadow_inference raises the NAMED requirement ---------
    # HAND CHECK: no framework is installed, so the guarded ray import
    # raises the named DECISION-POINT requirement before any artifact is
    # touched — the honesty gate: no framework, no policy output (every
    # policy/inference result is an EMPTY cell here).
    _expects_named_import_error(
        results,
        lambda: run_shadow_inference(_shadow_request()),
        "shadow_inference_without_framework_raises_named_requirement",
        "a shadow_inference request without ray raises the named "
        "RAY_RLLIB_DEPLOYMENT_REQUIREMENT (the honesty gate)",
    )

    # ---- Test 12: training is UNREACHABLE (the honesty gate dominates) --
    _expects_named_import_error(
        results,
        lambda: run_training({}),
        "training_unreachable_without_framework",
        "run_training raises the named RAY_RLLIB_DEPLOYMENT_REQUIREMENT "
        "before any training code — training results are EMPTY cells",
    )

    # ---- Test 13: the ceiling mirrors the sim-lab profile ----------------
    # The closed PERMISSION_CEILING must equal the profile's
    # "permission-ceiling" list (the profile JSON is the definition of
    # record; the two MUST move together).
    check(
        "permission_ceiling_matches_sim_lab_profile",
        safety_filter.PERMISSION_CEILING
        == frozenset({"telemetry.read", "asset.read", "schedule.propose"})
        and safety_filter.ACTION_PROPOSAL_PERMISSION == "schedule.propose",
        "the filter's ceiling is exactly the sim-lab profile set "
        "(telemetry.read, asset.read, schedule.propose); the pytest suite "
        "pins the equality against the JSON file itself",
    )

    # ---- Test 14: mode lock is a constant, not a setting ----------------
    tampered = _valid_request()
    tampered["control_authority_mode"] = "operator-approved"
    envelope = run_filter_probe(tampered)
    check(
        "mode_lock_not_request_selectable",
        envelope["control_authority_mode_locked"] == "simulation-shadow"
        and envelope["control_authority_mode_locked"] == MODE_LOCKED,
        "a request attempting to raise the control-authority mode is "
        "ignored: the lock is a module constant (report 21.4)",
    )

    return results


def _shadow_request() -> dict:
    """The canonical valid shadow_inference request (CONSTRUCTED test
    inputs; the artifact ref names a file that does not exist and can
    never exist in this repository)."""
    request = _valid_request(mode="shadow_inference")
    del request["action"]
    request["policy_manifest"] = {
        "policy_version": "RECORD-AT-FIRST-RUN",
        "policy_artifact_ref": "models/marl/never-trained-policy",
        "training_data_ref": "object-store:training-data/RECORD-REF",
        "observation_scope": "balanced-radial-feeder/RECORD-CLASS",
    }
    return request


def _expects_request_error(results: list, fn, name: str) -> None:
    """Self-test helper: `fn` must raise WorkerRequestError and nothing else."""
    try:
        fn()
    except WorkerRequestError:
        results.append(
            {
                "name": name,
                "passed": True,
                "detail": "raised WorkerRequestError as expected",
            }
        )
        return
    except Exception as error:  # noqa: BLE001 — pin the error class
        results.append(
            {
                "name": name,
                "passed": False,
                "detail": "raised %r instead of WorkerRequestError" % error,
            }
        )
        return
    results.append({"name": name, "passed": False, "detail": "raised nothing"})


def _expects_runtime_error(results: list, fn, name: str, detail: str) -> None:
    try:
        fn()
    except RuntimeError:
        results.append({"name": name, "passed": True, "detail": detail})
        return
    except Exception as error:  # noqa: BLE001 — pin the error class
        results.append(
            {
                "name": name,
                "passed": False,
                "detail": "%s; raised %r instead of RuntimeError" % (detail, error),
            }
        )
        return
    results.append({"name": name, "passed": False, "detail": "%s; raised nothing" % detail})


def _expects_policy_artifact_error(results: list, fn, name: str, detail: str) -> None:
    try:
        fn()
    except PolicyArtifactError:
        results.append({"name": name, "passed": True, "detail": detail})
        return
    except Exception as error:  # noqa: BLE001 — pin the error class
        results.append(
            {
                "name": name,
                "passed": False,
                "detail": "%s; raised %r instead of PolicyArtifactError"
                % (detail, error),
            }
        )
        return
    results.append({"name": name, "passed": False, "detail": "%s; raised nothing" % detail})


def _expects_named_import_error(results: list, fn, name: str, detail: str) -> None:
    try:
        fn()
    except ImportError as error:
        results.append(
            {
                "name": name,
                "passed": RAY_RLLIB_DEPLOYMENT_REQUIREMENT in str(error)
                and "DECISION-POINT" in str(error),
                "detail": "%s (message names the requirement: %s)"
                % (detail, RAY_RLLIB_DEPLOYMENT_REQUIREMENT in str(error)),
            }
        )
        return
    except Exception as error:  # noqa: BLE001 — pin the error class
        results.append(
            {
                "name": name,
                "passed": False,
                "detail": "%s; raised %r instead of ImportError" % (detail, error),
            }
        )
        return
    results.append({"name": name, "passed": False, "detail": "%s; raised nothing" % detail})


if __name__ == "__main__":  # pragma: no cover — human-gate entrypoint
    _records = run_self_tests()
    _failures = [r for r in _records if not r["passed"]]
    for _r in _records:
        print(("PASS" if _r["passed"] else "FAIL"), _r["name"], "-", _r["detail"])
    print("%d/%d self-tests passed" % (len(_records) - len(_failures), len(_records)))
    raise SystemExit(1 if _failures else 0)
