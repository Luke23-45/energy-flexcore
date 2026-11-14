# P7.17 — Request handling: envelope validation, correlation-id echo,
#         determinism enforcement, and the concrete-worker hook.
#
# ══════════════════════════════════════════════════════════════════════════
# HONESTY HEADER
#
# 1. STDLIB-ONLY, genuinely importable/runnable today. This module never
#    imports grpcio; the server seam (server.py) is the only grpcio
#    touchpoint in the template.
# 2. Isolation rules enforced HERE (the template README records the full
#    set): the handler never imports control-plane code, never holds
#    state across requests beyond its declared caches, and never writes
#    to platform tables — a worker's output is a PROPOSAL (ADR-010), and
#    §9.2 provenance travels with it.
# 3. Determinism (P7.42): in deterministic mode the handler stamps the
#    response's determinism block from the REQUEST (seed, request_id,
#    idempotency key) and refuses to invent wall-clock values. The
#    optional `now_ms` hook is advisory metadata (execution_ms), never a
#    result field.
# ══════════════════════════════════════════════════════════════════════════

from __future__ import annotations

from typing import Optional, Protocol

from . import errors

# The mandatory envelope fields of an Execute request (wire shape mirrors
# the proto `ExecuteRequest` after decoding).
REQUIRED_REQUEST_FIELDS: frozenset[str] = frozenset(
    {"contract_version", "request_id", "correlation_id", "payload_type"}
)


class WorkerLogic(Protocol):
    """The concrete worker's computation hook.

    A concrete worker implements `evaluate` and NOTHING else of the
    envelope: validation, correlation echo, and determinism stamping are
    the template's job. `evaluate` receives the request dict (already
    validated) and returns the result payload (bytes) plus a provenance
    dict (the §9.2 fields; see `ResultProvenance` in the proto).
    """

    def evaluate(
        self, request: dict[str, object]
    ) -> tuple[bytes, dict[str, object]]:
        """Compute the result for a validated request."""
        ...


def validate_envelope(request: dict[str, object]) -> None:
    """Validate the Execute envelope; raise `WorkerError` on refusal.

    Refusals happen BEFORE any computation: version skew, a missing
    mandatory field, or an empty correlation id is a structured refusal
    (errors.py), never a best-effort parse.
    """
    if not isinstance(request, dict):  # pragma: no cover - defensive
        raise errors.invalid_request("request envelope must be a mapping")
    for field in sorted(REQUIRED_REQUEST_FIELDS):
        value = request.get(field)
        if value is None or (isinstance(value, str) and not value):
            raise errors.invalid_request(
                f"missing mandatory request field {field!r}", field=field
            )
    version = request["contract_version"]
    if not isinstance(version, str):
        raise errors.invalid_request(
            "contract_version must be a string", field="contract_version"
        )
    received_major = _major_of(version)
    expected_major = _major_of(errors.CONTRACT_VERSION)
    if received_major != expected_major:
        # Major-version skew is an explicit refusal; minor/patch drift
        # within a major is tolerated by agreement (proto header note).
        raise errors.unsupported_contract_version(version)
    request_id = request["request_id"]
    if not isinstance(request_id, str) or not request_id.strip():
        raise errors.invalid_request(
            "request_id must be a non-empty string", field="request_id"
        )
    payload_type = request["payload_type"]
    if not isinstance(payload_type, str) or not payload_type.strip():
        raise errors.invalid_request(
            "payload_type must be a non-empty string", field="payload_type"
        )


def _major_of(version: str) -> str:
    """The compatibility class of a version tag (`flexcore.worker.vN`).

    Takes only the leading major segment (`flexcore.worker.v1.1` -> "1")
    so minor/patch drift within a major stays tolerated.
    """
    parts = version.rsplit(".v", 1)
    if len(parts) != 2 or not parts[1]:
        return version  # unparseable tags are handled by the caller
    return parts[1].split(".", 1)[0]


def check_health(machine: object, detail: str = "") -> dict[str, object]:
    """Answer a Check call from the health state machine (thin alias of
    `health.readiness_response`, kept here so the server seam has one
    handler-shaped entrypoint for both rpcs)."""
    from . import health  # noqa: PLC0415 — local import keeps module order clear.

    assert isinstance(machine, health.HealthStateMachine)
    return health.readiness_response(machine, errors.CONTRACT_VERSION, detail)


def handle_execute(
    request: dict[str, object],
    logic: WorkerLogic,
    *,
    deterministic: bool = True,
    accepts_work: bool = True,
    now_ms: Optional[int] = None,
) -> dict[str, object]:
    """Handle one Execute request end to end (the template's core).

    Steps, in order:
      1. Refuse when the worker is not accepting work (DRAINING/STOPPING
         via `accepts_work=False`) — structured UNAVAILABLE.
      2. Validate the envelope (refusals before any computation).
      3. Give the payload to the concrete `WorkerLogic.evaluate`.
      4. Stamp the §9.2-shaped response: correlation/request echo, the
         determinism block (from the REQUEST, never invented), and the
         provenance the logic reported.

    In deterministic mode, `now_ms` (wall clock) is recorded only as
    advisory `execution_ms` provenance metadata — never as a result
    field, and excluded from P7.42 comparison (P7.42 risk note).
    """
    if not accepts_work:
        raise errors.unavailable("worker is draining or stopping")

    validate_envelope(request)

    deterministic_requested = bool(request.get("deterministic", deterministic))
    seed = request.get("seed")
    seed_supported = isinstance(seed, int)

    payload, provenance = logic.evaluate(request)

    execution_ms = 0
    if now_ms is not None:
        # Advisory only; wall-clock never enters the result payload.
        execution_ms = max(0, int(now_ms))

    return {
        "contract_version": errors.CONTRACT_VERSION,
        "correlation_id": request["correlation_id"],
        "request_id": request["request_id"],
        "status": "EXECUTE_STATUS_OK",
        "payload": payload,
        "provenance": provenance,
        "determinism": {
            "seed": seed if seed is not None else 0,
            "request_id": request["request_id"],
            "idempotency_key": request.get("idempotency_key", ""),
            "deterministic": bool(deterministic and deterministic_requested),
            "seed_supported": seed_supported,
        },
        "error": None,
    }


def handle_execute_error(
    request_id: str,
    correlation_id: str,
    error: errors.WorkerError,
    *,
    execution_started: bool = False,
) -> dict[str, object]:
    """Build the error-shaped Execute response for a `WorkerError`.

    The status is REFUSED when the error is a refusal code (nothing ever
    executed), FAILED when execution started and failed in-band, and
    CANCELLED for cancellation — mirroring the proto `ExecuteStatus`
    vocabulary.
    """
    if error.code == "WORKER_ERROR_CODE_CANCELLED":
        status = "EXECUTE_STATUS_CANCELLED"
    elif error.is_refusal() and not execution_started:
        status = "EXECUTE_STATUS_REFUSED"
    else:
        status = "EXECUTE_STATUS_FAILED"
    return {
        "contract_version": errors.CONTRACT_VERSION,
        "correlation_id": correlation_id,
        "request_id": request_id,
        "status": status,
        "payload": b"",
        "provenance": {
            "worker_version": "",
            "model_version": "",
            "solver_version": "",
            "input_snapshot_id": "",
            "termination_status": "failed",
            "diagnostics": [error.message],
            "execution_ms": 0,
        },
        "determinism": None,
        "error": error.as_detail(),
    }
