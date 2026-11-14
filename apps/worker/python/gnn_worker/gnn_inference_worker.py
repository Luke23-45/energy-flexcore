# P8.11 — The GNN warm-start/screening inference worker: an isolated,
#         proposal-only, DECISION-POINT-gated Class C research worker.
#
# ══════════════════════════════════════════════════════════════════════════
# HONESTY HEADER — read before trusting anything below
#
# 1. THIS WORKER WAS WRITTEN, NEVER EXECUTED. The GNN framework stack is a
#    recorded DECISION-POINT (P8.03 §4; D4 OPEN): NO torch version and NO
#    PyG version is selected, pinned, installed, or verified in this
#    repository's history. `import torch` / `import torch_geometric` are
#    guarded INSIDE functions with the NAMED ImportError constants below;
#    the pure-logic core of this file imports, runs, and self-tests with
#    NOTHING installed — which is also the loop's honesty gate: without
#    the (never-selected) framework, NO inference output can ever exist.
#    Every inference result in this repository is therefore an EMPTY cell.
# 2. WHAT THE GNN IS FOR (P8.10 design; §13.1): two roles, never conflated
#    — (a) warm-start candidate generation for the EXACT solver (the GNN
#    changes the solver's starting point; the solver still solves, and a
#    warm start affects SPEED of an exact solve, never its optimality);
#    (b) screening: ranking constraints/contingencies with scores and
#    uncertainty (surrogate estimates, never decisions). HiGHS is
#    LP/QP/MIP — never SOCP; Clarabel is convex conic — never MISOCP;
#    solver classes are never conflated (AGENTS.md rule 4).
# 3. CONTROL BOUNDARY (absolute; ADR-010): this worker has NO field-write
#    path and NO device channel — its only output is ONE JSON document on
#    stdout. It never issues, approves, or carries a command; every output
#    is a PROPOSAL; `assert_proposal_only()` scans the result tree for
#    authorization/command-shaped keys and refuses emission. Mode-locked
#    to simulation/shadow (a constant; no request field can change it).
# 4. PROVENANCE (ledger P8.11 step 3; §9.2 + P8.04): every output carries
#    model version, training-data reference (the P8.04 result key
#    `training_data_ref`; the full training-data provenance record is
#    P8.35's — this worker carries the REFERENCE, never the record),
#    input snapshot ID, and topology scope (the P8.04 result key
#    `topology_scope`). The result payload is checked against the CLOSED
#    P8.04 module key set for `flexcore.research.gnn_warm_start.v1`.
# 5. SCOPE DISCIPLINE (ledger P8.11 step 4): a request whose topology_id
#    is outside the model manifest's trained topology set is FLAGGED
#    in-band and its inference outputs stay NULL — an out-of-scope input
#    is never silently answered. A missing model artifact fails LOUDLY
#    (named `ModelArtifactError`, exit 2, nothing on stdout). The request
#    may declare an inference timeout within the recorded maximum; its
#    runtime enforcement is a VERIFY-BUILD item (UNVERIFIED).
# 6. TRANSPORT CONTRACT (mirrors `../andes_worker/andes_dynamic_worker.py`
#    and `../grid_worker.py`): ONE JSON document on stdin (the
#    `flexcore-gnn-worker/1` request), ONE JSON document on stdout (the
#    result envelope). Malformed request → NOTHING on stdout, reason on
#    stderr, exit 2. An in-band failure (out-of-scope topology, refused
#    study) is a RESULT envelope with `termination_status: "failed"` —
#    never a crash and never a fabricated figure. NO accuracy, latency,
#    speedup, or ranking-quality figure exists anywhere in this file:
#    every measured cell belongs to the human/lab gate (P8.12; ENTRY_NOTE
#    boundary 4).
# 7. CONTRACT SEAM (recorded, not invented): this worker reuses the Phase
#    7 worker template's §9.2 provenance field names verbatim and the
#    P8.04 closed result vocabulary, but binds no generated
#    `flexcore.worker.v1` gRPC servicer (buf generation remains the
#    activation step; the transport is the JSON-document discipline). The
#    gaps are recorded in `ledger/phase-08-advanced-coordination/
#    11_gnn_inference_worker.md` §8 as Phase 7 follow-ups — never fixed by
#    inventing a parallel contract.
# ══════════════════════════════════════════════════════════════════════════

from __future__ import annotations

import json
import math
import os
import sys

# The worker-transport protocol tag (the mirror of `flexcore-andes-worker/1`
# for the GNN track; validated on every request, fail-closed).
PROTOCOL_ID = "flexcore-gnn-worker/1"

WORKER_VERSION = "flexcore-gnn-worker/0.1.0"

# Control-authority mode this worker is LOCKED to (§21.4). A constant, not
# a setting: no request field can change it, no plugin can select a mode.
MODE_LOCKED = "simulation-shadow"

# The two GNN roles (P8.10 design §3 — never conflated into one headline).
GNN_MODES: frozenset = frozenset({"warm_start", "screening"})

# The P8.04 GNN payload type (the closed research vocabulary). BOTH roles
# ride this one payload type — P8.04 defines exactly one GNN payload type;
# the role is distinguished by `result_kind`. Adding a separate screening
# payload type would be an ADDITIVE P8.04 change (a Phase 7 follow-up),
# never a parallel contract invented here.
PAYLOAD_TYPE = "flexcore.research.gnn_warm_start.v1"

# The result kind each role produces (the closed §13.4 vocabulary in the
# P8.04 template — NAME-ALIGNED with `flexcore-research` validation.rs).
GNN_RESULT_KIND_BY_MODE: dict = {
    "warm_start": "candidate_setpoint_vector",
    "screening": "ranking",
}

# The closed §13.4 result-kind vocabulary (mirrored from the P8.04 template;
# the two MUST move together — the recorded seam).
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

# The CLOSED P8.04 module result-key set for the GNN payload type (mirrored
# verbatim from `python/workers/worker_template/research.py`). The GNN
# result payload's keys MUST stay inside this set; a key outside it is a
# structured refusal, never tolerated.
GNN_RESULT_FIELDS: frozenset = frozenset(
    {"result_kind", "training_data_ref", "topology_scope", "uncertainty"}
)

# The exact-solver family a warm start targets (closed). HiGHS is LP/QP/MIP
# — NEVER SOCP (AGENTS.md rule 4); Clarabel (convex conic) is a different
# solver class and is deliberately NOT a warm-start target declared here,
# because conflating the classes is the named failure mode this repo bans.
TARGET_SOLVERS: frozenset = frozenset({"highs_lp_qp_mip"})

# The §10.2 canonical unit vocabulary a declared node feature may carry.
# Power features are MW/MVAr/MVA/MWh; SOC is a FRACTION in [0,1] at the API
# boundary (unit-free by kind); voltage is per-unit (pu, solver
# representation); status is binary. `kV`/`A`/`percent` are accepted by the
# shared vocabulary but only meaningful for the kinds that declare them.
ALLOWED_UNITS: frozenset = frozenset(
    {"MW", "MVAr", "MVA", "MWh", "kV", "A", "pu", "percent"}
)

# The closed node-feature kinds (per P8.10 §5 data contract).
FEATURE_KINDS: frozenset = frozenset({"power", "soc", "voltage", "status"})

# Whether a feature kind REQUIRES a declared unit (and which). A soc or
# status feature carries no unit; a voltage feature must declare "pu"; a
# power feature must declare one of the MW-family units.
KIND_UNIT_RULE: dict = {
    "power": ("required", ALLOWED_UNITS - {"pu", "kV", "A", "percent"}),
    "soc": ("forbidden", frozenset()),
    "voltage": ("required", frozenset({"pu"})),
    "status": ("forbidden", frozenset()),
}

# The recorded inference-timeout maximum (ledger P8.11 step 4 names the
# timeout as a failure mode to test). The value is a recorded LIMIT, not a
# performance figure; the runtime enforcement mechanism is a VERIFY-BUILD
# item (UNVERIFIED) — the pure core only validates the DECLARATION.
MAX_INFERENCE_TIMEOUT_S = 60.0

# The framework pin status, carried verbatim wherever the stack is named
# (P8.03 §4 DECISION-POINT; nothing selected, installed, or verified).
GNN_PIN_STATUS = (
    "DECISION-POINT OPEN — no torch/PyG pair is selected, pinned, "
    "installed, or verified (P8.03 section 4; RESEARCH.md R8-C3 skew: "
    "PyG 2.8.0 supports PyTorch only up to 2.12 while PyTorch 2.15 is "
    "current); the P8.10 design record section 6 carries the "
    "RECOMMENDATION, never a decision"
)

# The named deployment requirements a missing framework surfaces (raised
# by the guarded importers; `11_gnn_inference_worker.md` §6 repeats them).
TORCH_DEPLOYMENT_REQUIREMENT = (
    "flexcore GNN inference worker requires the research ML framework: "
    "torch is NOT INSTALLED and the torch/PyG pair is a recorded "
    "DECISION-POINT (P8.03 section 4, D4 OPEN) — no version is selected, "
    "pinned, or verified in this repository. At the human build/test pass, "
    "pin the verified-compatible pair recorded in "
    "ledger/phase-08-advanced-coordination/10_gnn_warmstart.md section 6 "
    "into an ISOLATED research environment (never a system environment, "
    "never the platform runtime) after the P8.36 at-pin-time advisory "
    "check, and update the pin status"
)

PYG_DEPLOYMENT_REQUIREMENT = (
    "flexcore GNN inference worker requires torch_geometric (PyG): NOT "
    "INSTALLED and the torch/PyG pair is a recorded DECISION-POINT (P8.03 "
    "section 4, D4 OPEN; R8-C3 version skew — a PyG release supporting "
    "the selected torch version must be re-checked at pin time per "
    "RESEARCH.md section 7 task 2). Pin only a verified-compatible pair "
    "into the isolated research environment; never install into the "
    "platform runtime"
)

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
    "A warm start changes only the exact solver's starting point: the "
    "solver (HiGHS LP/QP/MIP for the declared class) still solves the "
    "problem to its own termination criteria — the GNN does not replace "
    "the solver, and a warm start affects SPEED of an exact solve, never "
    "its optimality (report 13.1).",
    "A screening score is a surrogate estimate with uncertainty, never a "
    "decision: a screened-out constraint may still be active (the miss "
    "cost is a pre-registered P8.12 metric), and no screening output "
    "removes a constraint from any solve (report 13.1).",
    "Warm-start and screening results are never conflated into one "
    "headline number: a warm-start win is not a screening win (P8.02 M2; "
    "report 13.1).",
    "This worker's outputs are research PROPOSALS only (ADR-010): no "
    "authorization token exists in this response type and nothing here "
    "can be issued to a device.",
    "This worker is mode-locked to simulation/shadow; it cannot select "
    "or raise a control-authority mode (report 21.4).",
    "HiGHS is LP/QP/MIP, never SOCP; Clarabel is convex conic, never "
    "MISOCP; solver classes are never conflated (AGENTS.md rule 4).",
    "The torch/PyG framework pair is a recorded DECISION-POINT (P8.03 "
    "section 4): nothing is installed, pinned, or verified in this "
    "repository, so no inference output exists here — every measured "
    "cell is EMPTY until the human/lab gate (P8.12).",
)

UNVERIFIED_NOTE = (
    "UNVERIFIED-API worker: no torch/PyG pair is installed or pinned "
    "(DECISION-POINT, P8.03 section 4); every torch/PyG call below "
    "follows the documented PyG message-passing API shape as of the "
    "2026-11-07 research cut-off and is UNVERIFIED. This worker was "
    "never executed. First execution belongs to the human build/test "
    "pass, after the pair is pinned per the P8.10 design record."
)


class WorkerRequestError(ValueError):
    """A malformed inference request — fail-closed BEFORE any computation."""


class ModelArtifactError(RuntimeError):
    """The declared model artifact is missing or unreadable — fails LOUDLY.

    Named per ledger P8.11 step 4 ("model file missing ... must fail
    loudly"): never absorbed, never answered with a default model, never
    converted into a silent degraded result.
    """


# ---------------------------------------------------------------------------
# Guarded framework imports — the single torch/PyG choke points, and the
# loop's honesty gate: without the (never-installed) framework, no
# inference output can ever exist.
# ---------------------------------------------------------------------------


def import_torch():
    """Imports `torch` or raises the NAMED DECISION-POINT requirement.

    The import lives INSIDE this function so the rest of the module
    parses, imports, and self-tests with nothing installed.
    """
    try:
        import torch  # noqa: PLC0415 — deliberate deferred import (see header).
    except ImportError as error:
        raise ImportError(TORCH_DEPLOYMENT_REQUIREMENT) from error
    return torch


def import_torch_geometric():
    """Imports `torch_geometric` or raises the NAMED DECISION-POINT
    requirement (R8-C3: the PyG release must match the selected torch)."""
    try:
        import torch_geometric  # noqa: PLC0415 — deliberate deferred import.

        try:
            import torch_geometric.utils  # noqa: F401,PLC0415 — surface probe.
        except ImportError as inner:
            raise ImportError(PYG_DEPLOYMENT_REQUIREMENT) from inner
    except ImportError as error:
        raise ImportError(PYG_DEPLOYMENT_REQUIREMENT) from error
    return torch_geometric


def framework_version_reported(module) -> str | None:
    """The installed framework version, read defensively (UNVERIFIED
    surface; must never crash the result)."""
    try:
        version = getattr(module, "__version__", None)
        return None if version is None else str(version)
    except Exception:  # noqa: BLE001 — the probe must never crash the worker
        return None


# ---------------------------------------------------------------------------
# Pure request validation (no torch, no PyG, no I/O beyond the given dict)
# ---------------------------------------------------------------------------


def _require_nonempty_str(container: dict, field: str, where: str) -> str:
    value = container.get(field)
    if not isinstance(value, str) or not value.strip():
        raise WorkerRequestError("%s.%s must be a non-empty string" % (where, field))
    return value


def _require_finite_number(container: dict, field: str, where: str) -> float:
    value = container.get(field)
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise WorkerRequestError("%s.%s must be a finite number" % (where, field))
    if not math.isfinite(value):
        raise WorkerRequestError("%s.%s must be finite" % (where, field))
    return float(value)


def validate_graph(node_count: object, edges: object) -> list:
    """Graph-shape checks (pure): node count, edge endpoints in range,
    self-loops refused.

    Rules (recorded, hand-tested below): `node_count` a positive integer;
    every edge a 2-sequence of integer endpoints with 0 <= e < node_count;
    self-loops (src == dst) are REFUSED (a device connected to itself is
    not a power-system edge; a silently tolerated self-loop would corrupt
    message passing). Parallel edges are ALLOWED (real feeders have
    parallel lines) and carried as given.
    """
    if isinstance(node_count, bool) or not isinstance(node_count, int):
        raise WorkerRequestError("request.topology.node_count must be an integer")
    if node_count < 1:
        raise WorkerRequestError("request.topology.node_count must be >= 1")
    if not isinstance(edges, list):
        raise WorkerRequestError("request.topology.edges must be a list of [src, dst]")
    normalized = []
    for index, edge in enumerate(edges):
        if not isinstance(edge, (list, tuple)) or len(edge) != 2:
            raise WorkerRequestError(
                "request.topology.edges[%d] must be a [src, dst] pair" % index
            )
        src, dst = edge
        for name, endpoint in (("src", src), ("dst", dst)):
            if isinstance(endpoint, bool) or not isinstance(endpoint, int):
                raise WorkerRequestError(
                    "request.topology.edges[%d].%s must be an integer index"
                    % (index, name)
                )
            if endpoint < 0 or endpoint >= node_count:
                raise WorkerRequestError(
                    "request.topology.edges[%d].%s=%d is outside the node "
                    "index range [0, %d)" % (index, name, endpoint, node_count)
                )
        if src == dst:
            raise WorkerRequestError(
                "request.topology.edges[%d] is a self-loop (%d -> %d); "
                "self-loops are refused" % (index, src, dst)
            )
        normalized.append([int(src), int(dst)])
    return normalized


def validate_feature_declarations(declarations: object) -> list:
    """Validates the per-node feature declarations (closed kinds; unit
    rules per kind; §10.2 canonical units)."""
    if not isinstance(declarations, list) or not declarations:
        raise WorkerRequestError(
            "request.feature_declarations must be a non-empty list (the "
            "feature contract is declared, never assumed)"
        )
    validated = []
    for index, decl in enumerate(declarations):
        if not isinstance(decl, dict):
            raise WorkerRequestError(
                "request.feature_declarations[%d] must be an object" % index
            )
        name = _require_nonempty_str(
            decl, "name", "request.feature_declarations[%d]" % index
        )
        kind = decl.get("kind")
        if kind not in FEATURE_KINDS:
            raise WorkerRequestError(
                "request.feature_declarations[%d].kind %r is not one of %s"
                % (index, kind, sorted(FEATURE_KINDS))
            )
        unit = decl.get("unit")
        rule, allowed = KIND_UNIT_RULE[kind]
        if rule == "required":
            if not isinstance(unit, str) or unit not in allowed:
                raise WorkerRequestError(
                    "request.feature_declarations[%d] (%s, kind %r) must "
                    "declare unit in %s" % (index, name, kind, sorted(allowed))
                )
        elif rule == "forbidden" and unit is not None:
            raise WorkerRequestError(
                "request.feature_declarations[%d] (%s, kind %r) carries no "
                "unit (kind rule)" % (index, name, kind)
            )
        validated.append({"name": name, "kind": kind, "unit": unit})
    return validated


def validate_features(features: object, node_count: int, declarations: list) -> None:
    """Feature-vector shape and range checks (pure).

    `features` must be a list of exactly `node_count` rows, each row of
    exactly len(declarations) finite numbers, with per-kind ranges: soc in
    [0, 1] (SOC is a fraction at the API boundary), status in {0, 1},
    voltage > 0 (a non-positive per-unit voltage is not a state), power
    finite of any sign (P > 0 = export at the point of connection is a
    caller-side sign declaration, carried by the feature NAME, not
    enforced here).
    """
    if not isinstance(features, list):
        raise WorkerRequestError("request.features must be a list of node rows")
    if len(features) != node_count:
        raise WorkerRequestError(
            "request.features has %d rows for node_count %d (shape mismatch)"
            % (len(features), node_count)
        )
    width = len(declarations)
    for row_index, row in enumerate(features):
        if not isinstance(row, (list, tuple)) or len(row) != width:
            raise WorkerRequestError(
                "request.features[%d] must have exactly %d values (one per "
                "declaration)" % (row_index, width)
            )
        for col_index, value in enumerate(row):
            if isinstance(value, bool) or not isinstance(value, (int, float)):
                raise WorkerRequestError(
                    "request.features[%d][%d] must be a finite number"
                    % (row_index, col_index)
                )
            if not math.isfinite(value):
                raise WorkerRequestError(
                    "request.features[%d][%d] must be finite"
                    % (row_index, col_index)
                )
            kind = declarations[col_index]["kind"]
            if kind == "soc" and not (0.0 <= float(value) <= 1.0):
                raise WorkerRequestError(
                    "request.features[%d][%d] is a SOC fraction and must be "
                    "in [0, 1] (got %r)" % (row_index, col_index, value)
                )
            if kind == "status" and float(value) not in (0.0, 1.0):
                raise WorkerRequestError(
                    "request.features[%d][%d] is a status feature and must "
                    "be 0 or 1 (got %r)" % (row_index, col_index, value)
                )
            if kind == "voltage" and float(value) <= 0.0:
                raise WorkerRequestError(
                    "request.features[%d][%d] is a per-unit voltage and "
                    "must be > 0 (got %r)" % (row_index, col_index, value)
                )


def evaluate_topology_scope(
    topology_id: str, trained_topology_ids: list, declared_scope: str
) -> dict:
    """The scope check (pure): is the requested topology inside the model
    manifest's trained topology set?

    Returns {"in_scope": bool, "reason": str, ...}; NEVER raises for an
    out-of-scope answer — flagging is the contract (the caller decides the
    in-band envelope). `topology_id` membership is EXACT string equality
    against the manifest's trained list; no fuzzy or prefix matching is
    permitted (a near-miss must flag, not pass).
    """
    if topology_id in trained_topology_ids:
        return {
            "in_scope": True,
            "topology_id": topology_id,
            "declared_scope": declared_scope,
            "reason": "topology_id is in the manifest's trained_topology_ids",
        }
    return {
        "in_scope": False,
        "topology_id": topology_id,
        "declared_scope": declared_scope,
        "reason": (
            "topology_id %r is NOT in the manifest's trained_topology_ids "
            "(%d trained topologies declared); inference is refused — an "
            "out-of-scope topology is flagged, never silently answered"
            % (topology_id, len(trained_topology_ids))
        ),
    }


def check_model_artifact_path(artifact_path: object) -> str:
    """The pure part of the missing-model failure mode.

    Validates that the manifest's model artifact resolves to an existing
    file path and raises the NAMED `ModelArtifactError` when it does not.
    In this repository the path can NEVER pass (nothing was ever
    downloaded/installed/trained), so the loud failure is the only
    reachable outcome.
    """
    if not isinstance(artifact_path, str) or not artifact_path.strip():
        raise ModelArtifactError(
            "model artifact ref is missing or empty: a GNN inference "
            "request without a declared, versioned model artifact fails "
            "LOUDLY (ledger P8.11 step 4); artifact store refs are the "
            "P8.20/P8.32 seam"
        )
    if artifact_path.startswith("object-store:"):
        # An object-store ref (P8.20) is resolved by the worker runtime at
        # activation time; resolution is a VERIFY-BUILD item (UNVERIFIED).
        return artifact_path
    if not os.path.isfile(artifact_path):
        raise ModelArtifactError(
            "model artifact file not found: %r — a missing model file "
            "fails loudly (ledger P8.11 step 4); there is no default "
            "model and no degraded-mode inference" % artifact_path
        )
    return artifact_path


def rank_candidates(scored: list) -> list:
    """Sorts scored screening candidates by score, DESCENDING, with a
    deterministic tie-break (candidate_id ascending) — pure, hand-tested.

    `scored` is a list of {"candidate_id": str, "score": float,
    "uncertainty": float}. Scores are surrogate estimates; NO range is
    imposed on a score (a normalized [0,1] score would be a model design
    choice recorded at training time, not a worker rule), but every score
    and uncertainty must be finite, and uncertainty must be >= 0. A
    screening ranking without per-entry uncertainty is refused (§13.1:
    surrogate estimates WITH uncertainty).
    """
    if not isinstance(scored, list) or not scored:
        raise WorkerRequestError(
            "a screening ranking needs a non-empty candidate list"
        )
    seen_ids: set = set()
    for index, entry in enumerate(scored):
        if not isinstance(entry, dict):
            raise WorkerRequestError("scored[%d] must be an object" % index)
        candidate_id = _require_nonempty_str(entry, "candidate_id", "scored[%d]" % index)
        if candidate_id in seen_ids:
            raise WorkerRequestError(
                "duplicate candidate_id %r in the screening list" % candidate_id
            )
        seen_ids.add(candidate_id)
        score = entry.get("score")
        uncertainty = entry.get("uncertainty")
        for name, value in (("score", score), ("uncertainty", uncertainty)):
            if isinstance(value, bool) or not isinstance(value, (int, float)):
                raise WorkerRequestError(
                    "scored[%d].%s must be a finite number" % (index, name)
                )
            if not math.isfinite(value):
                raise WorkerRequestError(
                    "scored[%d].%s must be finite" % (index, name)
                )
        if uncertainty < 0:
            raise WorkerRequestError(
                "scored[%d].uncertainty must be >= 0" % index
            )
    return sorted(
        scored,
        key=lambda entry: (-float(entry["score"]), str(entry["candidate_id"])),
    )


def validate_request(request: dict) -> dict:
    """Validates the inference request; raises `WorkerRequestError` on
    refusal. Refusals happen BEFORE any framework import and before any
    artifact check. Returns the validated fields, normalized."""
    if not isinstance(request, dict):
        raise WorkerRequestError("request must be a JSON object")
    if request.get("protocol") != PROTOCOL_ID:
        raise WorkerRequestError(
            "request protocol %r is not %r" % (request.get("protocol"), PROTOCOL_ID)
        )
    mode = request.get("mode")
    if mode not in GNN_MODES:
        raise WorkerRequestError(
            "request.mode %r is not one of %s (the two GNN roles are "
            "separate and never conflated)" % (mode, sorted(GNN_MODES))
        )
    target_solver = request.get("target_solver")
    if target_solver not in TARGET_SOLVERS:
        raise WorkerRequestError(
            "request.target_solver %r is not one of %s (HiGHS is LP/QP/MIP; "
            "solver classes are never conflated)" % (target_solver, sorted(TARGET_SOLVERS))
        )

    manifest = request.get("model_manifest")
    if not isinstance(manifest, dict):
        raise WorkerRequestError(
            "request.model_manifest is required (model_version, "
            "model_artifact_ref, training_data_ref, topology_scope, "
            "trained_topology_ids)"
        )
    model_version = _require_nonempty_str(manifest, "model_version", "request.model_manifest")
    model_artifact_ref = _require_nonempty_str(
        manifest, "model_artifact_ref", "request.model_manifest"
    )
    training_data_ref = _require_nonempty_str(
        manifest, "training_data_ref", "request.model_manifest"
    )
    topology_scope = _require_nonempty_str(
        manifest, "topology_scope", "request.model_manifest"
    )
    trained_topology_ids = manifest.get("trained_topology_ids")
    if (
        not isinstance(trained_topology_ids, list)
        or not trained_topology_ids
        or not all(isinstance(t, str) and t.strip() for t in trained_topology_ids)
    ):
        raise WorkerRequestError(
            "request.model_manifest.trained_topology_ids must be a "
            "non-empty list of non-empty topology identifiers (the "
            "trained scope is declared, never assumed)"
        )

    topology = request.get("topology")
    if not isinstance(topology, dict):
        raise WorkerRequestError(
            "request.topology is required (topology_id, node_count, edges)"
        )
    topology_id = _require_nonempty_str(topology, "topology_id", "request.topology")
    node_count = topology.get("node_count")
    edges = validate_graph(node_count, topology.get("edges"))

    declarations = validate_feature_declarations(request.get("feature_declarations"))
    validate_features(request.get("features"), int(node_count), declarations)

    screening: dict = {}
    if mode == "screening":
        screening_raw = request.get("screening")
        if not isinstance(screening_raw, dict):
            raise WorkerRequestError(
                "a screening request requires request.screening "
                "(candidates: the constraints/contingencies to rank)"
            )
        candidates = screening_raw.get("candidates")
        if not isinstance(candidates, list) or not candidates:
            raise WorkerRequestError(
                "request.screening.candidates must be a non-empty list of "
                "{candidate_id, node_index} entries"
            )
        seen: set = set()
        normalized_candidates = []
        for index, entry in enumerate(candidates):
            if not isinstance(entry, dict):
                raise WorkerRequestError(
                    "request.screening.candidates[%d] must be an object" % index
                )
            candidate_id = _require_nonempty_str(
                entry, "candidate_id", "request.screening.candidates[%d]" % index
            )
            if candidate_id in seen:
                raise WorkerRequestError(
                    "request.screening.candidates[%d]: duplicate "
                    "candidate_id %r" % (index, candidate_id)
                )
            seen.add(candidate_id)
            node_index = entry.get("node_index")
            if (
                isinstance(node_index, bool)
                or not isinstance(node_index, int)
                or node_index < 0
                or node_index >= int(node_count)
            ):
                raise WorkerRequestError(
                    "request.screening.candidates[%d].node_index must be a "
                    "node index in [0, %d)" % (index, int(node_count))
                )
            normalized_candidates.append(
                {"candidate_id": candidate_id, "node_index": int(node_index)}
            )
        screening = {"candidates": normalized_candidates}

    timeout_seconds = None
    if request.get("timeout_seconds") is not None:
        timeout_seconds = _require_finite_number(request, "timeout_seconds", "request")
        if timeout_seconds <= 0.0:
            raise WorkerRequestError("request.timeout_seconds must be > 0")
        if timeout_seconds > MAX_INFERENCE_TIMEOUT_S:
            raise WorkerRequestError(
                "request.timeout_seconds %r exceeds the recorded maximum "
                "%r (the runtime enforcement of the timeout is a "
                "VERIFY-BUILD item; the declaration is validated here)"
                % (timeout_seconds, MAX_INFERENCE_TIMEOUT_S)
            )

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
        "target_solver": target_solver,
        "model_manifest": {
            "model_version": model_version,
            "model_artifact_ref": model_artifact_ref,
            "training_data_ref": training_data_ref,
            "topology_scope": topology_scope,
            "trained_topology_ids": list(trained_topology_ids),
        },
        "topology": {
            "topology_id": topology_id,
            "node_count": int(node_count),
            "edges": edges,
        },
        "feature_declarations": declarations,
        "features": request.get("features"),
        "screening": screening,
        "timeout_seconds": timeout_seconds,
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


def enforce_gnn_result_schema(result: dict) -> None:
    """The CLOSED P8.04 result-key set for the GNN payload type (pure).

    Mirrors `research.enforce_result_schema` for payload type
    `flexcore.research.gnn_warm_start.v1`: every key inside
    `GNN_RESULT_FIELDS`; `result_kind` in the closed vocabulary;
    `training_data_ref` and `topology_scope` present and non-empty (the
    P8.11 provenance requirement, stricter than the P8.04 base because
    the ledger's step 3 makes them mandatory on every output). Raises
    `WorkerRequestError` (a contract refusal, safe to wire).
    """
    if not isinstance(result, dict):
        raise WorkerRequestError("a GNN result payload must be a mapping")
    unknown = sorted(set(result) - GNN_RESULT_FIELDS)
    if unknown:
        raise WorkerRequestError(
            "result keys outside the closed %s schema: %s" % (PAYLOAD_TYPE, unknown)
        )
    kind = result.get("result_kind")
    if not isinstance(kind, str) or kind not in RESULT_KINDS:
        raise WorkerRequestError(
            "result_kind %r is not in the closed §13.4 vocabulary" % (kind,)
        )
    for field in ("training_data_ref", "topology_scope"):
        value = result.get(field)
        if not isinstance(value, str) or not value.strip():
            raise WorkerRequestError(
                "result.%s must be a non-empty string (P8.11 provenance "
                "requirement: every output carries its training-data "
                "reference and topology scope)" % field
            )


def build_result(
    *,
    validated: dict,
    termination_status: str,
    outputs: dict | None,
    diagnostics: list,
    torch_version: str | None,
    pyg_version: str | None,
    execution_ms: int,
) -> dict:
    """Builds the `flexcore-gnn-worker/1` result envelope (pure).

    `termination_status` is "completed" only when inference actually ran
    (impossible in this repository — no framework exists) and its outputs
    were extracted; "failed" marks an in-band failure (out-of-scope
    topology, refused inference) with diagnostics. NO accuracy, latency,
    or speedup figure exists in any envelope: measured cells belong to
    the P8.12 human/lab gate and are EMPTY here.
    """
    scope_check = evaluate_topology_scope(
        validated["topology"]["topology_id"],
        validated["model_manifest"]["trained_topology_ids"],
        validated["model_manifest"]["topology_scope"],
    )
    result = {
        "protocol": PROTOCOL_ID,
        "kind": "gnn_inference_result",
        "ok": termination_status == "completed",
        "mode": validated["mode"],
        "payload_type": PAYLOAD_TYPE,
        # Proposal-only posture (ADR-010), enforced in code by
        # `assert_proposal_only` before emission.
        "proposal_only": True,
        "authority": "none",
        "control_authority_mode_locked": MODE_LOCKED,
        "boundary_statements": list(BOUNDARY_STATEMENTS),
        "model_scope": {
            "role": validated["mode"],
            "target_solver": validated["target_solver"],
            "model_version": validated["model_manifest"]["model_version"],
            "model_artifact_ref": validated["model_manifest"]["model_artifact_ref"],
            "training_data_ref": validated["model_manifest"]["training_data_ref"],
            "topology_scope": validated["model_manifest"]["topology_scope"],
            "trained_topology_ids": validated["model_manifest"]["trained_topology_ids"],
            "framework_pin_status": GNN_PIN_STATUS,
            "torch_version_reported": torch_version,
            "pyg_version_reported": pyg_version,
        },
        "topology_scope_check": scope_check,
        "outputs": outputs,
        "provenance": {
            # §9.2 field names, verbatim from the Phase 7 worker template.
            "worker_version": WORKER_VERSION,
            "model_version": validated["model_manifest"]["model_version"],
            "solver_version": validated["target_solver"],
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
    return result


# ---------------------------------------------------------------------------
# The inference path (guarded imports; every torch/PyG call UNVERIFIED and
# UNREACHABLE in this repository — no framework exists here)
# ---------------------------------------------------------------------------


def run_inference(request: dict) -> dict:
    """Runs one GNN inference request (the ONLY torch/PyG path in this file).

    Order of gates:
      1. pure request validation (refusals before anything else);
      2. the topology-scope check — an out-of-scope topology is converted
         into the IN-BAND flagged envelope (outputs NULL) BEFORE any
         framework import: an out-of-scope input is flagged, never
         silently answered, and never needs the model to say so;
      3. the guarded torch import (named DECISION-POINT requirement) —
         the honesty gate dominates: with no framework installed (this
         repository's permanent state until the human pin), NO further
         gate is reachable;
      4. the guarded PyG import (named DECISION-POINT requirement);
      5. the model-artifact check (missing file fails LOUDLY, named
         ModelArtifactError);
      6. the UNVERIFIED inference steps (artifact load, graph assembly,
         forward pass) — unreachable in this repository.

    Raises ImportError (the named deployment requirements),
    WorkerRequestError (malformed request), or ModelArtifactError (missing
    model file). An out-of-scope topology does NOT raise: it RETURNS the
    in-band flagged envelope (termination "failed", outputs None,
    `topology_scope_check.in_scope: false`) — a flagged refusal is a
    result, not a crash.
    """
    validated = validate_request(request)
    diagnostics: list = []

    # Gate 2: scope flag (pure — no framework needed to refuse).
    scope_check = evaluate_topology_scope(
        validated["topology"]["topology_id"],
        validated["model_manifest"]["trained_topology_ids"],
        validated["model_manifest"]["topology_scope"],
    )
    if not scope_check["in_scope"]:
        diagnostics.append("out-of-scope topology refused: %s" % scope_check["reason"])
        return build_result(
            validated=validated,
            termination_status="failed",
            outputs=None,
            diagnostics=diagnostics,
            torch_version=None,
            pyg_version=None,
            execution_ms=0,
        )

    # Gate 3/4: the guarded framework imports (named requirements) — the
    # honesty gate dominates; nothing after this point is reachable here.
    torch = import_torch()
    pyg = import_torch_geometric()
    torch_version = framework_version_reported(torch)
    pyg_version = framework_version_reported(pyg)

    # Gate 5: missing model artifact fails loudly (named error) — checked
    # AFTER the framework gate, so the DECISION-POINT requirement is the
    # first honest refusal on this machine.
    check_model_artifact_path(validated["model_manifest"]["model_artifact_ref"])

    try:
        # UNVERIFIED — torch API: artifact load. The artifact is the
        # versioned, signed model artifact (P8.20 store ref / P8.32
        # signing seam); a load failure is an in-band failure, never a
        # default model.
        loaded = torch.load(
            validated["model_manifest"]["model_artifact_ref"], map_location="cpu"
        )
        model = loaded["model"] if isinstance(loaded, dict) else loaded

        # UNVERIFIED — PyG API: assemble the graph tensors from the
        # validated (pure) structures. The pure core has already enforced
        # every shape/range rule; this step only converts types.
        edge_index = torch.tensor(
            [e for edge in validated["topology"]["edges"] for e in edge],
            dtype=torch.long,
        ).reshape(2, -1)
        node_features = torch.tensor(
            validated["features"], dtype=torch.float32
        )

        # UNVERIFIED — torch API: inference-mode forward pass, CPU-only
        # by default (GPU use is a recorded deployment setting, never a
        # worker choice).
        model.eval()
        with torch.inference_mode():
            raw_outputs = model(node_features, edge_index)

        # UNVERIFIED — the role-specific output contract. Both branches
        # carry per-entry uncertainty and stay inside the closed P8.04
        # key set (`enforce_gnn_result_schema`).
        if validated["mode"] == "screening":
            scored = []
            for index, candidate in enumerate(validated["screening"]["candidates"]):
                scored.append(
                    {
                        "candidate_id": candidate["candidate_id"],
                        "node_index": candidate["node_index"],
                        "score": float(raw_outputs[index][0]),
                        "uncertainty": float(raw_outputs[index][1]),
                    }
                )
            ranking = rank_candidates(scored)
            payload = {
                "result_kind": GNN_RESULT_KIND_BY_MODE["screening"],
                "training_data_ref": validated["model_manifest"]["training_data_ref"],
                "topology_scope": validated["model_manifest"]["topology_scope"],
                "uncertainty": True,
            }
            enforce_gnn_result_schema(payload)
            outputs = {"ranking": ranking, "result_payload": payload}
        else:  # warm_start
            warm_start_vector = [float(v) for v in raw_outputs]
            payload = {
                "result_kind": GNN_RESULT_KIND_BY_MODE["warm_start"],
                "training_data_ref": validated["model_manifest"]["training_data_ref"],
                "topology_scope": validated["model_manifest"]["topology_scope"],
                "uncertainty": True,
            }
            enforce_gnn_result_schema(payload)
            outputs = {
                "warm_start_vector": warm_start_vector,
                "warm_start_statement": (
                    "a starting point for the declared exact solver "
                    "(%s) — the solver still solves; this vector is "
                    "never a schedule and never a device command"
                    % validated["target_solver"]
                ),
                "result_payload": payload,
            }
        termination = "completed"

    except ImportError:
        raise  # the named deployment requirements propagate (honesty gate)
    except WorkerRequestError:
        raise  # a contract refusal propagates (exit 2), never in-band
    except Exception as error:  # noqa: BLE001 — in-band inference failure
        diagnostics.append("inference failed in-band: %s" % error)
        termination = "failed"
        outputs = None

    return build_result(
        validated=validated,
        termination_status=termination,
        outputs=outputs,
        diagnostics=diagnostics,
        torch_version=torch_version,
        pyg_version=pyg_version,
        execution_ms=0,
    )


# ---------------------------------------------------------------------------
# Composition edge: one JSON document in, one JSON document out
# ---------------------------------------------------------------------------


def main() -> int:
    """Reads ONE request document on stdin, emits ONE result on stdout.

    Fail-closed: any malformed input is answered with NOTHING on stdout,
    a reason on stderr, and exit 2. A successful inference emits exactly
    one `flexcore-gnn-worker/1` envelope, after `assert_proposal_only`.
    """
    raw = sys.stdin.read()
    try:
        request = json.loads(raw)
    except json.JSONDecodeError as error:
        sys.stderr.write(
            "flexcore-gnn-worker: request is not a valid JSON document: %s\n" % error
        )
        return 2
    try:
        validation_probe = validate_request(request)
        del validation_probe  # validation is the gate; run_inference revalidates
        result = run_inference(request)
        assert_proposal_only(result)
    except WorkerRequestError as error:
        sys.stderr.write("flexcore-gnn-worker: request refused: %s\n" % error)
        return 2
    except ImportError as error:
        sys.stderr.write("flexcore-gnn-worker: %s\n" % error)
        return 2
    except ModelArtifactError as error:
        sys.stderr.write("flexcore-gnn-worker: %s\n" % error)
        return 2
    except RuntimeError as error:
        sys.stderr.write("flexcore-gnn-worker: refused: %s\n" % error)
        return 2
    sys.stdout.write(json.dumps(result))
    sys.stdout.write("\n")
    sys.stdout.flush()
    return 0


# ---------------------------------------------------------------------------
# In-module self-tests for the PURE-LOGIC core (hand-computed; NEVER RUN
# in this loop). Run instruction (human gate):
#   python apps/worker/python/gnn_worker/gnn_inference_worker.py
# (the pytest suite mirrors these cases in tests/test_gnn_inference_worker.py).
# ---------------------------------------------------------------------------


def run_self_tests() -> list:
    """Runs every hand-computed self-test; returns one record per test.

    Each record: {"name", "passed", "detail"}. This function itself has
    NEVER been executed in this loop; the expectations are hand
    derivations from the constructed inputs, never observed outputs.
    """
    results: list = []

    def check(name: str, condition: bool, detail: str) -> None:
        results.append({"name": name, "passed": bool(condition), "detail": detail})

    # ---- Test 1: a valid 3-node graph passes adjacency validation ------
    # Nodes {0,1,2}, edges 0->1, 1->2, 2->0. All endpoints in [0, 3); no
    # self-loop. HAND CHECK: 0,1,2 < 3 and >= 0; src != dst on all three.
    ok_edges = validate_graph(3, [[0, 1], [1, 2], [2, 0]])
    check(
        "graph_three_node_valid",
        ok_edges == [[0, 1], [1, 2], [2, 0]] and len(ok_edges) == 3,
        "a 3-node triangle graph passes; edges normalized verbatim",
    )

    # ---- Test 2: edge endpoint out of range refused ---------------------
    # HAND CHECK: endpoint 3 with node_count 3 is outside [0, 3).
    def out_of_range() -> None:
        validate_graph(3, [[0, 3]])

    _expects_request_error(results, out_of_range, "graph_edge_endpoint_out_of_range")

    # ---- Test 3: self-loop refused --------------------------------------
    def self_loop() -> None:
        validate_graph(3, [[1, 1]])

    _expects_request_error(results, self_loop, "graph_self_loop_refused")

    # ---- Test 4: negative endpoint refused ------------------------------
    def negative_endpoint() -> None:
        validate_graph(3, [[-1, 2]])

    _expects_request_error(results, negative_endpoint, "graph_negative_endpoint_refused")

    # ---- Test 5: feature shape mismatch refused -------------------------
    # 3 nodes declared, 2 feature rows -> shape mismatch.
    def shape_mismatch_rows() -> None:
        decls = validate_feature_declarations(
            [{"name": "p", "kind": "power", "unit": "MW"}]
        )
        validate_features([[1.0], [2.0]], 3, decls)

    _expects_request_error(results, shape_mismatch_rows, "features_row_count_mismatch")

    # ---- Test 6: feature width mismatch refused -------------------------
    # Two declarations, each row carries one value -> width mismatch.
    def shape_mismatch_width() -> None:
        decls = validate_feature_declarations(
            [
                {"name": "p", "kind": "power", "unit": "MW"},
                {"name": "s", "kind": "soc", "unit": None},
            ]
        )
        validate_features([[1.0], [1.0], [1.0]], 3, decls)

    _expects_request_error(results, shape_mismatch_width, "features_width_mismatch")

    # ---- Test 7: SOC out of [0,1] refused; boundary values accepted -----
    # HAND CHECK: 0.0 and 1.0 are the closed-interval endpoints (accepted);
    # 1.5 and -0.1 are outside (refused). SOC is a fraction at the API
    # boundary.
    def soc_ok() -> None:
        decls = validate_feature_declarations(
            [{"name": "s", "kind": "soc", "unit": None}]
        )
        validate_features([[0.0], [1.0]], 2, decls)

    try:
        soc_ok()
        soc_boundary_pass = True
    except WorkerRequestError:
        soc_boundary_pass = False
    check(
        "soc_boundary_values_accepted",
        soc_boundary_pass,
        "SOC 0.0 and 1.0 (the closed-interval endpoints) accepted",
    )

    def soc_too_high() -> None:
        decls = validate_feature_declarations(
            [{"name": "s", "kind": "soc", "unit": None}]
        )
        validate_features([[1.5]], 1, decls)

    _expects_request_error(results, soc_too_high, "soc_above_one_refused")

    def soc_negative() -> None:
        decls = validate_feature_declarations(
            [{"name": "s", "kind": "soc", "unit": None}]
        )
        validate_features([[-0.1]], 1, decls)

    _expects_request_error(results, soc_negative, "soc_negative_refused")

    # ---- Test 8: status feature must be 0 or 1 --------------------------
    def status_two() -> None:
        decls = validate_feature_declarations(
            [{"name": "st", "kind": "status", "unit": None}]
        )
        validate_features([[2.0]], 1, decls)

    _expects_request_error(results, status_two, "status_non_binary_refused")

    # ---- Test 9: unit rules per kind (canonical units; closed kinds) ----
    def bad_kind() -> None:
        validate_feature_declarations([{"name": "x", "kind": "embedding", "unit": None}])

    _expects_request_error(results, bad_kind, "feature_kind_outside_closed_set")

    def bad_unit() -> None:
        validate_feature_declarations([{"name": "p", "kind": "power", "unit": "GW"}])

    _expects_request_error(results, bad_unit, "power_unit_not_canonical_refused")

    def voltage_needs_pu() -> None:
        validate_feature_declarations([{"name": "v", "kind": "voltage", "unit": "kV"}])

    _expects_request_error(results, voltage_needs_pu, "voltage_kind_requires_pu_unit")

    def soc_carries_no_unit() -> None:
        validate_feature_declarations([{"name": "s", "kind": "soc", "unit": "pu"}])

    _expects_request_error(results, soc_carries_no_unit, "soc_kind_forbids_unit")

    # ---- Test 10: per-unit voltage must be positive ---------------------
    def voltage_nonpositive() -> None:
        decls = validate_feature_declarations(
            [{"name": "v", "kind": "voltage", "unit": "pu"}]
        )
        validate_features([[0.0]], 1, decls)

    _expects_request_error(results, voltage_nonpositive, "voltage_nonpositive_refused")

    # ---- Test 11: closed result schema (P8.04 key set) ------------------
    # (a) the canonical warm-start payload passes;
    # (b) an unknown key is refused (closed whitelist);
    # (c) a missing training_data_ref is refused (P8.11 provenance rule);
    # (d) a result_kind outside the §13.4 vocabulary is refused.
    canonical = {
        "result_kind": "candidate_setpoint_vector",
        "training_data_ref": "object-store:training-data/RECORD-REF",
        "topology_scope": "balanced-radial-feeder/RECORD-CLASS",
        "uncertainty": True,
    }
    try:
        enforce_gnn_result_schema(canonical)
        schema_ok = True
    except WorkerRequestError:
        schema_ok = False
    check(
        "gnn_result_schema_canonical_passes",
        schema_ok,
        "the canonical P8.04 payload (result_kind/training_data_ref/"
        "topology_scope/uncertainty) passes the closed schema",
    )

    def unknown_key() -> None:
        smuggled = dict(canonical)
        smuggled["dispatch_vector"] = [1.0]
        enforce_gnn_result_schema(smuggled)

    _expects_request_error(results, unknown_key, "gnn_result_unknown_key_refused")

    def missing_ref() -> None:
        incomplete = dict(canonical)
        del incomplete["training_data_ref"]
        enforce_gnn_result_schema(incomplete)

    _expects_request_error(results, missing_ref, "gnn_result_missing_training_data_ref")

    def bad_kind_result() -> None:
        wrong = dict(canonical)
        wrong["result_kind"] = "dispatch_schedule"
        enforce_gnn_result_schema(wrong)

    _expects_request_error(results, bad_kind_result, "gnn_result_kind_outside_vocabulary")

    # ---- Test 12: proposal-only boundary scans the whole tree -----------
    deep = {"outputs": {"ranking": [{"candidate_id": "c1", "approved": True}]}}
    _expects_runtime_error(
        results,
        lambda: assert_proposal_only(deep),
        "proposal_only_nested_forbidden_key",
        "a nested 'approved' key anywhere in the tree refuses emission",
    )
    try:
        assert_proposal_only({"outputs": {"ranking": [{"candidate_id": "c1"}]}})
        clean_ok = True
    except RuntimeError:
        clean_ok = False
    check(
        "proposal_only_clean_tree_passes",
        clean_ok,
        "a clean result tree passes the proposal-only scan",
    )

    # ---- Test 13: topology scope flag (flagged, never silently answered)
    trained = ["feeder-A", "feeder-B", "feeder-C"]
    in_scope = evaluate_topology_scope("feeder-B", trained, "balanced-radial")
    out_scope = evaluate_topology_scope("feeder-Z", trained, "balanced-radial")
    check(
        "topology_scope_in_scope_passes",
        in_scope["in_scope"] is True,
        "feeder-B is in the trained set; in_scope True",
    )
    check(
        "topology_scope_out_of_scope_flagged",
        out_scope["in_scope"] is False
        and "NOT in the manifest" in out_scope["reason"],
        "feeder-Z is not in the trained set; in_scope False with a "
        "reason — the flag, not a silent answer",
    )

    # ---- Test 14: missing model artifact fails LOUDLY -------------------
    # HAND CHECK: no artifact exists in this repository (nothing was ever
    # downloaded or trained), so the named ModelArtifactError is the only
    # reachable outcome for a filesystem path.
    _expects_model_artifact_error(
        results,
        lambda: check_model_artifact_path("models/gnn/never-trained.pt"),
        "model_artifact_missing_fails_loudly",
        "a nonexistent model file raises the named ModelArtifactError",
    )
    _expects_model_artifact_error(
        results,
        lambda: check_model_artifact_path(""),
        "model_artifact_empty_ref_fails_loudly",
        "an empty artifact ref raises the named ModelArtifactError",
    )

    # ---- Test 15: ranking order (hand-computed, deterministic tie-break)
    # HAND ARITHMETIC: scores c1=0.9, c3=0.5, c2=0.1 sort descending to
    # [c1, c3, c2]. A tie (t2=0.5, t1=0.5) breaks by candidate_id
    # ascending: t1 before t2.
    ranking = rank_candidates(
        [
            {"candidate_id": "c1", "score": 0.9, "uncertainty": 0.05},
            {"candidate_id": "c2", "score": 0.1, "uncertainty": 0.30},
            {"candidate_id": "c3", "score": 0.5, "uncertainty": 0.12},
        ]
    )
    check(
        "ranking_descending_order",
        [entry["candidate_id"] for entry in ranking] == ["c1", "c3", "c2"],
        "expected order c1 (0.9), c3 (0.5), c2 (0.1) — descending by score",
    )
    tie = rank_candidates(
        [
            {"candidate_id": "t2", "score": 0.5, "uncertainty": 0.1},
            {"candidate_id": "t1", "score": 0.5, "uncertainty": 0.1},
        ]
    )
    check(
        "ranking_deterministic_tie_break",
        [entry["candidate_id"] for entry in tie] == ["t1", "t2"],
        "a 0.5/0.5 tie resolves by candidate_id ascending (t1 before t2); "
        "deterministic, replayable",
    )

    def missing_uncertainty() -> None:
        rank_candidates(
            [{"candidate_id": "c1", "score": 0.9, "uncertainty": -0.1}]
        )

    _expects_request_error(
        results, missing_uncertainty, "ranking_negative_uncertainty_refused"
    )

    def duplicate_candidate() -> None:
        rank_candidates(
            [
                {"candidate_id": "c1", "score": 0.9, "uncertainty": 0.1},
                {"candidate_id": "c1", "score": 0.4, "uncertainty": 0.1},
            ]
        )

    _expects_request_error(
        results, duplicate_candidate, "ranking_duplicate_candidate_id_refused"
    )

    # ---- Test 16: timeout declaration within the recorded maximum -------
    def timeout_over_max() -> None:
        request = _valid_request()
        request["timeout_seconds"] = MAX_INFERENCE_TIMEOUT_S + 1.0
        validate_request(request)

    _expects_request_error(results, timeout_over_max, "timeout_over_recorded_max_refused")

    def timeout_zero() -> None:
        request = _valid_request()
        request["timeout_seconds"] = 0.0
        validate_request(request)

    _expects_request_error(results, timeout_zero, "timeout_nonpositive_refused")

    # ---- Test 17: full valid request accepted; envelope posture fixed ---
    validated = validate_request(_valid_request())
    envelope = build_result(
        validated=validated,
        termination_status="failed",
        outputs=None,
        diagnostics=["out-of-scope topology refused: recorded reason"],
        torch_version=None,
        pyg_version=None,
        execution_ms=7,
    )
    check(
        "envelope_fixed_posture",
        envelope["proposal_only"] is True
        and envelope["authority"] == "none"
        and envelope["control_authority_mode_locked"] == MODE_LOCKED
        and MODE_LOCKED == "simulation-shadow"
        and envelope["ok"] is False
        and envelope["provenance"]["termination_status"] == "failed"
        and envelope["provenance"]["execution_ms"] == 7
        and envelope["topology_scope_check"]["in_scope"] is True,
        "the envelope carries proposal_only/authority none/mode lock, "
        "§9.2 provenance names, and the scope check verbatim",
    )
    check(
        "envelope_boundary_statements_carry_the_two_roles",
        any("warm start" in s and "never" in s for s in envelope["boundary_statements"])
        and any("screening score" in s for s in envelope["boundary_statements"])
        and any("HiGHS is LP/QP/MIP" in s for s in envelope["boundary_statements"])
        and any("DECISION-POINT" in s for s in envelope["boundary_statements"]),
        "the warm-start-is-speed-not-optimality statement, the screening "
        "surrogate statement, the solver-class discipline, and the "
        "framework DECISION-POINT travel verbatim in every envelope",
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

    # ---- Test 18: the inference path flags out-of-scope WITHOUT torch ---
    # HAND CHECK: the scope gate runs BEFORE any framework import, so the
    # flagged in-band envelope is producible with NOTHING installed — the
    # failure mode ledger P8.11 step 4 names is testable today.
    out_request = _valid_request()
    out_request["topology"]["topology_id"] = "feeder-UNSEEN"
    out_result = run_inference(out_request)
    check(
        "inference_out_of_scope_flagged_without_framework",
        out_result["ok"] is False
        and out_result["provenance"]["termination_status"] == "failed"
        and out_result["outputs"] is None
        and out_result["topology_scope_check"]["in_scope"] is False
        and out_result["topology_scope_check"]["topology_id"] == "feeder-UNSEEN",
        "an out-of-scope topology yields termination failed, NULL outputs "
        "and the scope flag — computed with no torch installed",
    )

    # ---- Test 19: in-scope + no framework raises the NAMED requirement --
    _expects_named_import_error(
        results,
        lambda: run_inference(_valid_request()),
        "inference_without_framework_raises_named_requirement",
        "an in-scope request without torch raises the named "
        "TORCH_DEPLOYMENT_REQUIREMENT (the honesty gate: no framework, "
        "no inference output — every inference result is EMPTY here)",
    )

    return results


def _expects_request_error(results: list, fn, name: str) -> None:
    """Self-test helper: `fn` must raise WorkerRequestError and nothing else."""
    try:
        fn()
    except WorkerRequestError:
        check = results.append
        check(
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


def _expects_model_artifact_error(results: list, fn, name: str, detail: str) -> None:
    try:
        fn()
    except ModelArtifactError:
        results.append({"name": name, "passed": True, "detail": detail})
        return
    except Exception as error:  # noqa: BLE001 — pin the error class
        results.append(
            {
                "name": name,
                "passed": False,
                "detail": "%s; raised %r instead of ModelArtifactError"
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
                "passed": TORCH_DEPLOYMENT_REQUIREMENT in str(error)
                and "DECISION-POINT" in str(error),
                "detail": "%s (message names the requirement: %s)"
                % (detail, TORCH_DEPLOYMENT_REQUIREMENT in str(error)),
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


def _valid_request() -> dict:
    """The canonical valid request used by the self-tests (all figures are
    CONSTRUCTED test inputs, never measurements)."""
    return {
        "protocol": PROTOCOL_ID,
        "mode": "warm_start",
        "target_solver": "highs_lp_qp_mip",
        "model_manifest": {
            "model_version": "RECORD-AT-FIRST-RUN",
            "model_artifact_ref": "models/gnn/never-trained.pt",
            "training_data_ref": "object-store:training-data/RECORD-REF",
            "topology_scope": "balanced-radial-feeder/RECORD-CLASS",
            "trained_topology_ids": ["feeder-A", "feeder-B", "feeder-C"],
        },
        "topology": {
            "topology_id": "feeder-A",
            "node_count": 3,
            "edges": [[0, 1], [1, 2]],
        },
        "feature_declarations": [
            {"name": "p_export_mw", "kind": "power", "unit": "MW"},
            {"name": "soc_fraction", "kind": "soc", "unit": None},
        ],
        "features": [[1.5, 0.5], [0.0, 0.25], [-2.0, 1.0]],
        "provenance": {
            "input_snapshot_id": "snapshot-RECORD",
            "request_id": "req-0001",
        },
    }


if __name__ == "__main__":  # pragma: no cover — human-gate entrypoint
    _records = run_self_tests()
    _failures = [r for r in _records if not r["passed"]]
    for _r in _records:
        print(("PASS" if _r["passed"] else "FAIL"), _r["name"], "-", _r["detail"])
    print("%d/%d self-tests passed" % (len(_records) - len(_failures), len(_records)))
    raise SystemExit(1 if _failures else 0)
