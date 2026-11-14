# P8.04 — Research worker contracts: result, provenance, and the
#         proposal-only boundary, as an ADDITIVE module of the Phase 7
#         worker template.
#
# ══════════════════════════════════════════════════════════════════════════
# HONESTY HEADER
#
# 1. STDLIB-ONLY, genuinely importable today: json is the only new import
#    and it is the standard library. NO new external dependency is added —
#    grpcio stays UNPINNED-REVERIFY (`python/workers/requirements.txt`,
#    unchanged) and NOTHING was installed in this loop. This module never
#    imports grpcio at module level; the ONLY grpcio touchpoint in the
#    template remains `server.require_grpc` (imports `grpc` INSIDE the
#    function with the named ImportError `GRPC_DEPLOYMENT_REQUIREMENT`),
#    and `require_worker_grpc` below deliberately REUSES that choke point
#    instead of defining a second seam.
# 2. NO PARALLEL CONTRACT (ENTRY_NOTE boundary 3): research workers REUSE
#    the Phase 7 contract `contracts/worker_api.proto`
#    (`FlexcoreWorkerService` Check/Execute/Cancel/Shutdown, package
#    `flexcore.worker.v1`) and the template envelope unchanged. This
#    module defines only the per-`payload_type` RESULT agreement the proto
#    delegates to the payload schema ("the payload type discriminator —
#    the agreement between caller and worker about the payload's schema").
#    A contract gap discovered here is recorded as a PHASE 7 FOLLOW-UP in
#    `ledger/phase-08-advanced-coordination/04_worker_contracts.md`, never
#    fixed by inventing a parallel contract.
# 3. WRITTEN-UNRUN. The test suite `tests/test_research_contracts.py` was
#    written in the same loop and NEVER executed (pytest never installed;
#    loop bans). The pure-logic functions are expected to pass at the
#    first human run.
# ══════════════════════════════════════════════════════════════════════════
#
# Design rules (recorded — the P8.04 ledger steps, in code):
#
# - THE RESULT CONTRACT (step 1): every research result carries the §9.2
#   provenance base (ResultProvenance of the proto, mirrored by
#   `validate_research_provenance`) plus a CLOSED per-module result-key
#   set (`MODULE_RESULT_FIELDS`): ADMM residuals/iteration counts; GNN
#   warm-start provenance (model version via provenance, training-data
#   ref, topology scope); MARL policy version, training seed,
#   environment version, safety-filter outcome. The key sets are CLOSED:
#   a key outside the set is a structured refusal, never a silent pass.
# - THE PROPOSAL-ONLY BOUNDARY IS ENFORCED IN CODE, NOT ONLY IN DOCS
#   (step 2): `build_research_result` whitelists every key; a result key
#   outside the closed module set cannot exist; and
#   `assert_proposal_only` recursively refuses any
#   authorization/authority-shaped key ANYWHERE in a result (defense in
#   depth for results assembled outside the builder). There is NO field,
#   key, or type in this module that can carry an authorization token —
#   the boundary is structural, matching
#   `crates/flexcore-research/src/validation.rs` (the §13.4 vocabulary
#   and the harness's no-researcher-verdict rule; the name alignment with
#   `ResearchResultKind` is the recorded seam).
# - THE VALIDATION HOOK (step 3): `proposal_candidate_record` wraps an
#   accepted result as the PROPOSED-stage candidate record that enters
#   the protected validation → feasibility → authorization path
#   (§13.1, §14.1). It grants no authority: the record's `authority`
#   field is a fixed statement, not a token. Physical re-validation is
#   the independent harness's job (flexcore-research::validation) — this
#   module never validates physics and never trusts the researcher's own
#   claim (there is no field a researcher verdict could ride in on).
# - `buf lint` / `buf breaking` (step 4): NOT RUN in this loop (no buf
#   toolchain executed; loop bans). Recorded as a Phase 7 follow-up in
#   the decision record — the proto registration into the `proto/` buf
#   module is the activation step, unchanged from P7.17.

from __future__ import annotations

import json
from typing import Optional, Protocol

from . import errors, handler

# The contract version REUSED from the Phase 7 template (single source:
# config.py/errors.py define it; this module re-exports the same value —
# a research worker speaks exactly the template contract, never a fork).
RESEARCH_CONTRACT_VERSION: str = errors.CONTRACT_VERSION

# ---------------------------------------------------------------------------
# Closed vocabularies (P8.04 step 1)
# ---------------------------------------------------------------------------

# The research payload types (the per-module `payload_type` agreements).
# CLOSED: an unknown type is refused (UNSUPPORTED_PAYLOAD_TYPE), never
# guessed. Names follow the proto's own example form
# ("flexcore.research.forecast_request.v1", worker_api.proto field 3 note).
RESEARCH_PAYLOAD_TYPES: tuple[str, ...] = (
    "flexcore.research.admm_schedule.v1",
    "flexcore.research.gnn_warm_start.v1",
    "flexcore.research.marl_shadow.v1",
)

# The closed §13.4 result-kind vocabulary — NAME-ALIGNED with
# `crates/flexcore-research/src/validation.rs::ResearchResultKind` (the
# recorded seam: the Rust enum is the definition of record; these strings
# are its `as_str` forms, and the two MUST move together).
RESEARCH_RESULT_KINDS: tuple[str, ...] = (
    "proposed_schedule",
    "candidate_setpoint_vector",
    "ranking",
    "forecast_distribution",
    "uncertainty_estimate",
    "diagnostics",
)

# The per-module result-key sets (CLOSED). `result_kind` is mandatory in
# every set. These are the P8.04 step-1 module-specific extensions of the
# §9.2 base; the provenance base itself is checked by
# `validate_research_provenance` below and is NOT duplicated here.
MODULE_RESULT_FIELDS: dict[str, frozenset[str]] = {
    "flexcore.research.admm_schedule.v1": frozenset(
        {
            "result_kind",
            "primal_residual",
            "dual_residual",
            "iteration_count",
        }
    ),
    "flexcore.research.gnn_warm_start.v1": frozenset(
        {
            "result_kind",
            "training_data_ref",
            "topology_scope",
            "uncertainty",
        }
    ),
    "flexcore.research.marl_shadow.v1": frozenset(
        {
            "result_kind",
            "policy_version",
            "training_seed",
            "environment_version",
            "safety_filter_outcome",
        }
    ),
}

# The §9.2 provenance base every research result carries (the proto
# `ResultProvenance` fields + the P8.32 model-artifact seam), mirrored
# from `flexcore-research::validation::check_provenance` stage 3: the
# SAME fields the Rust harness requires, so a proposal refused on one
# side would be refused on the other. `termination_status` has the
# proto's own rule: mandatory, and "ok" alone is not sufficient.
REQUIRED_PROVENANCE_FIELDS: tuple[str, ...] = (
    "worker_version",
    "model_version",
    "solver_version",
    "input_snapshot_id",
    "termination_status",
    "model_artifact_ref",
)

# Keys that must NEVER appear anywhere in a research result (any depth).
# The whitelist (`MODULE_RESULT_FIELDS`) already makes these
# unexpressible through the builder; this closed set is the defense in
# depth for results assembled by other paths. Every entry is an
# authorization/authority-shaped key (ADR-010; §13.4: a research output
# is never an authorization artifact).
FORBIDDEN_RESULT_KEYS: frozenset[str] = frozenset(
    {
        "authorization",
        "authorization_token",
        "auth_token",
        "access_token",
        "refresh_token",
        "id_token",
        "approval",
        "approved",
        "authorized",
        "command_id",
        "dispatch",
        "dispatch_command",
        "issued_command",
        "grant",
        "capability_grant",
    }
)

# The recorded proposal-only statement (travels on every candidate
# record; the repo's statement pattern — pinned by tests).
PROPOSAL_ONLY_STATEMENT: str = (
    "a research result is a PROPOSAL (ADR-010): it carries no authorization "
    "token and cannot carry one (the result key sets are closed and "
    "authorization-shaped keys are refused at any depth); it enters the "
    "protected validation -> feasibility -> authorization path as a "
    "candidate at lifecycle state PROPOSED and gains NO authority from "
    "having been proposed or validated (§13.4). No safety, certification, "
    "or field-authority claim is made."
)


class ProposalOnlyViolation(Exception):
    """A result attempted to carry an authorization/authority-shaped key.

    Raised by `assert_proposal_only` (and by the builder whitelist). The
    message names the offending KEY and its path only — never a value,
    never a payload body. This is a contract-design violation by the
    worker author, not a per-request transport error: it is not a
    `WorkerError` and must never be converted into a wire detail that
    could echo content.
    """


# ---------------------------------------------------------------------------
# The research worker hook (the template's WorkerLogic, typed for research)
# ---------------------------------------------------------------------------


class ResearchWorkerLogic(Protocol):
    """The concrete research worker's computation hook.

    Same shape as `handler.WorkerLogic`, with the research CONTRACT: the
    result is a structured dict (checked against the closed module key
    set and scanned for authorization-shaped keys BEFORE serialization —
    the boundary is enforced on the structured value, not on bytes).
    Optional attribute `requires_seed: bool` — a worker whose algorithm
    needs the request seed sets it True and a seedless request is
    refused (the P7.42 seed-honesty rule, mirrored).
    """

    def evaluate(
        self, request: dict[str, object]
    ) -> tuple[dict[str, object], dict[str, object]]:
        """Return `(result, provenance)` for a validated request."""
        ...


# ---------------------------------------------------------------------------
# Checks (refusals before execution; named, structured, log-safe)
# ---------------------------------------------------------------------------


def validate_payload_type(payload_type: object) -> str:
    """Refuse any payload type outside the closed research vocabulary."""
    if not isinstance(payload_type, str) or payload_type not in RESEARCH_PAYLOAD_TYPES:
        raise errors.unsupported_payload_type(str(payload_type))
    return payload_type


def enforce_result_schema(
    payload_type: str, result: dict[str, object]
) -> None:
    """Enforce the CLOSED module result-key set and the result kind.

    - every key must be inside `MODULE_RESULT_FIELDS[payload_type]`
      (whitelist — an unknown key is a structured refusal, never
      tolerated "for later");
    - `result_kind` must be present and inside `RESEARCH_RESULT_KINDS`.

    Raises `errors.WorkerError` (INVALID_REQUEST) on violation — these
    are contract refusals, safe to wire.
    """
    allowed = MODULE_RESULT_FIELDS.get(payload_type)
    if allowed is None:  # pragma: no cover - guarded by validate_payload_type
        raise errors.unsupported_payload_type(payload_type)
    unknown = sorted(set(result) - allowed)
    if unknown:
        raise errors.invalid_request(
            f"result keys outside the closed {payload_type} schema: {unknown}",
            field="result",
        )
    kind = result.get("result_kind")
    if not isinstance(kind, str) or kind not in RESEARCH_RESULT_KINDS:
        raise errors.invalid_request(
            f"result_kind {kind!r} is not in the closed §13.4 vocabulary",
            field="result_kind",
        )


def assert_proposal_only(value: object, _path: str = "result") -> None:
    """Recursively refuse any authorization-shaped key at ANY depth.

    Defense in depth behind the builder whitelist: a result assembled by
    any other path (a module handing back its own nested structure)
    cannot smuggle an authorization token past this scan. Raises
    `ProposalOnlyViolation` naming the key and its path — never a value.
    """
    if isinstance(value, dict):
        for key, inner in value.items():
            key_text = key if isinstance(key, str) else repr(key)
            if key_text in FORBIDDEN_RESULT_KEYS:
                raise ProposalOnlyViolation(
                    f"authorization-shaped key {key_text!r} at {_path} is "
                    "forbidden in a research result (ADR-010: proposals only)"
                )
            assert_proposal_only(inner, f"{_path}.{key_text}")
    elif isinstance(value, (list, tuple)):
        for index, inner in enumerate(value):
            assert_proposal_only(inner, f"{_path}[{index}]")


def validate_research_provenance(provenance: dict[str, object]) -> None:
    """The §9.2 + P8.32 provenance base, mirrored from the Rust harness.

    The SAME required fields and the SAME "bare ok is insufficient" rule
    as `flexcore-research::validation::check_provenance` (the recorded
    alignment seam). The seed-honesty pairing is enforced at the request
    boundary (`requires_seed`); the determinism block itself is stamped
    by the template handler from the REQUEST, per the proto.
    """
    if not isinstance(provenance, dict):  # pragma: no cover - defensive
        raise errors.invalid_request(
            "provenance must be a mapping", field="provenance"
        )
    for field in REQUIRED_PROVENANCE_FIELDS:
        value = provenance.get(field)
        if not isinstance(value, str) or not value.strip():
            raise errors.invalid_request(
                f"provenance field {field!r} is missing or empty", field=field
            )
    if provenance["termination_status"].strip() == "ok":
        raise errors.invalid_request(
            'termination_status "ok" alone is not sufficient '
            "(worker_api.proto ResultProvenance note)",
            field="termination_status",
        )


def build_research_result(
    payload_type: str, result: dict[str, object]
) -> dict[str, object]:
    """The ONLY sanctioned constructor of a research result payload.

    Validates the closed schema, scans for authorization-shaped keys,
    and returns the result unchanged otherwise. A token has no path
    through this function: the whitelist refuses unknown keys and the
    scan refuses forbidden ones.
    """
    enforce_result_schema(payload_type, result)
    assert_proposal_only(result)
    return dict(result)


# ---------------------------------------------------------------------------
# The validation hook (P8.04 step 3)
# ---------------------------------------------------------------------------


def proposal_candidate_record(
    result: dict[str, object],
    provenance: dict[str, object],
    *,
    proposal_id: str,
    producer_plugin_id: str,
    producer_plugin_version: str,
    sequence: int,
) -> dict[str, object]:
    """Wrap an accepted result as the PROPOSED-stage candidate record.

    This is the validation hook: the record a research proposal travels
    as when it enters the protected validation → feasibility →
    authorization path (§13.1: a GNN output passes the same physical
    validation as any other candidate; §14.1: a proposal enters at
    PROPOSED and only through the protected path). The record is
    DATA-only; `authority` is a fixed statement, never a token. The key
    set below is CLOSED by construction.
    """
    if not isinstance(result, dict) or "result_kind" not in result:
        raise errors.invalid_request(
            "a candidate record wraps a schema-checked result",
            field="result",
        )
    assert_proposal_only(result)
    assert_proposal_only(provenance, "provenance")
    if not isinstance(sequence, int) or sequence < 1:
        raise errors.invalid_request(
            "proposal sequence starts at 1", field="sequence"
        )
    for field, value in (
        ("proposal_id", proposal_id),
        ("producer_plugin_id", producer_plugin_id),
        ("producer_plugin_version", producer_plugin_version),
    ):
        if not isinstance(value, str) or not value.strip():
            raise errors.invalid_request(
                f"{field} must be a non-empty string", field=field
            )
    return {
        "proposal_id": proposal_id,
        "producer_plugin_id": producer_plugin_id,
        "producer_plugin_version": producer_plugin_version,
        "sequence": sequence,
        "result_kind": result["result_kind"],
        "result": dict(result),
        "provenance": dict(provenance),
        "lifecycle_state": "PROPOSED",
        "authority": PROPOSAL_ONLY_STATEMENT,
    }


# ---------------------------------------------------------------------------
# The Execute path (reuses the template handler end to end)
# ---------------------------------------------------------------------------


class _BoundaryAdapter:
    """Adapts a `ResearchWorkerLogic` onto the template's `WorkerLogic`.

    Runs, IN ORDER, before any bytes are produced:
      1. the payload-type vocabulary refusal (before compute);
      2. the seed-honesty refusal (`requires_seed` with no request seed);
      3. the concrete worker's `evaluate`;
      4. the closed-schema check, the proposal-only scan, and the §9.2
         provenance check on the STRUCTURED result;
      5. deterministic serialization (sorted keys, compact separators —
         the same input produces the same bytes, the P7.42 replay
         property).

    Any refusal raises before serialization: a boundary-violating result
    is never serialized, never stamped, never wired.
    """

    def __init__(self, logic: ResearchWorkerLogic) -> None:
        self._logic = logic

    def evaluate(
        self, request: dict[str, object]
    ) -> tuple[bytes, dict[str, object]]:
        payload_type = validate_payload_type(request.get("payload_type"))
        if getattr(self._logic, "requires_seed", False):
            seed = request.get("seed")
            if not isinstance(seed, int) or isinstance(seed, bool):
                raise errors.invalid_request(
                    "worker requires_seed but the request carries no integer "
                    "seed (P7.42 honesty rule)",
                    field="seed",
                )
        result, provenance = self._logic.evaluate(request)
        if not isinstance(result, dict):
            raise errors.invalid_request(
                "a research result must be a mapping", field="result"
            )
        enforce_result_schema(payload_type, result)
        assert_proposal_only(result)
        validate_research_provenance(provenance)
        payload = json.dumps(
            result, sort_keys=True, separators=(",", ":")
        ).encode("utf-8")
        return payload, provenance


def handle_research_execute(
    request: dict[str, object],
    logic: ResearchWorkerLogic,
    *,
    deterministic: bool = True,
    accepts_work: bool = True,
    now_ms: Optional[int] = None,
) -> dict[str, object]:
    """Handle one research Execute request through the TEMPLATE handler.

    The envelope validation, correlation echo, draining refusal, and
    determinism stamping are the template's (`handler.handle_execute`);
    this wrapper adds ONLY the research contract enforcement
    (`_BoundaryAdapter`). Refusal errors propagate as `WorkerError` for
    the caller to shape via `handler.handle_execute_error` — the same
    error path every template worker uses.
    """
    return handler.handle_execute(
        request,
        _BoundaryAdapter(logic),  # type: ignore[arg-type] — structural Protocol.
        deterministic=deterministic,
        accepts_work=accepts_work,
        now_ms=now_ms,
    )


def require_worker_grpc() -> object:
    """The research composition edge's transport check.

    REUSES the template's single grpcio choke point
    (`server.require_grpc`, which imports `grpc` INSIDE the function and
    raises the named ImportError `GRPC_DEPLOYMENT_REQUIREMENT`).
    Deliberately defines NO second grpcio seam: the template README
    records that every grpcio use goes through that one import, and a
    research worker is just a concrete template worker.
    """
    from . import server  # noqa: PLC0415 — local import keeps module order clear.

    return server.require_grpc()


# The recorded contract statement (pinned by tests).
RESEARCH_CONTRACT_STATEMENT: str = (
    "research workers REUSE the Phase 7 contract worker_api.proto "
    "(FlexcoreWorkerService Check/Execute/Cancel/Shutdown, "
    "flexcore.worker.v1) and the worker template envelope; this module "
    "adds only the per-payload_type result agreement, the closed result "
    "vocabularies, the in-code proposal-only boundary, and the PROPOSED-"
    "stage candidate record. No parallel contract exists or may be "
    "invented (a gap is a Phase 7 follow-up)."
)
