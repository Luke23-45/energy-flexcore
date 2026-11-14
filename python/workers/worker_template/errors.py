# P7.17 — Structured worker errors: the error vocabulary that travels
#         over the wire (mirrors `contracts/worker_api.proto`
#         `WorkerErrorCode` / `WorkerErrorDetail`).
#
# ══════════════════════════════════════════════════════════════════════════
# HONESTY HEADER (read before trusting anything below)
#
# 1. This module is STDLIB-ONLY and genuinely importable today. The only
#    third-party surface — mapping a `WorkerError` onto a gRPC status —
#    lives in `map_error_to_grpc_status`, which imports `grpc` INSIDE the
#    function (so this file parses, imports, and passes the pure-logic
#    tests without grpcio installed) and is marked UNVERIFIED there:
#    grpcio has never been installed or executed in this loop.
# 2. The `WorkerErrorCode` values below MUST stay name-aligned with the
#    `WorkerErrorCode` enum in `contracts/worker_api.proto` — the
#    template's tests pin the alignment; the proto is the wire definition
#    of record.
# ══════════════════════════════════════════════════════════════════════════
#
# Design rules (recorded):
# - Errors are STRUCTURED, never bare strings: code + message + retryable
#   + field, so the supervisor (P7.18) can decide retry/backoff/fail from
#   the code alone.
# - Messages are safe for logs by construction: the constructors never
#   accept a payload body, a config dump, or any secret value — callers
#   pass identifiers and short reasons only.
# - A refused request (version skew, unknown payload type, malformed
#   envelope) never executed anything, and says so via `refused` codes.

from __future__ import annotations

from typing import Optional

# The stable error-code vocabulary (name-aligned with the
# `WorkerErrorCode` proto enum; the proto enum is the wire definition of
# record). Values here are the wire names, not integers: the concrete
# gRPC status mapping is deferred to the grpcio seam (UNVERIFIED).
WORKER_ERROR_CODES: tuple[str, ...] = (
    "WORKER_ERROR_CODE_NONE",
    "WORKER_ERROR_CODE_INVALID_REQUEST",
    "WORKER_ERROR_CODE_UNSUPPORTED_CONTRACT_VERSION",
    "WORKER_ERROR_CODE_UNSUPPORTED_PAYLOAD_TYPE",
    "WORKER_ERROR_CODE_UNAVAILABLE",
    "WORKER_ERROR_CODE_INTERNAL",
    "WORKER_ERROR_CODE_CANCELLED",
    "WORKER_ERROR_CODE_CONFIG_INVALID",
)

# The codes that mean "the request never executed" — safe to retry under
# the supervisor's idempotency rules (P7.18), because there is no prior
# execution whose side effect could duplicate.
REFUSAL_CODES: frozenset[str] = frozenset(
    {
        "WORKER_ERROR_CODE_INVALID_REQUEST",
        "WORKER_ERROR_CODE_UNSUPPORTED_CONTRACT_VERSION",
        "WORKER_ERROR_CODE_UNSUPPORTED_PAYLOAD_TYPE",
        "WORKER_ERROR_CODE_UNAVAILABLE",
        "WORKER_ERROR_CODE_CONFIG_INVALID",
    }
)

# The contract version this template implements and validates against.
CONTRACT_VERSION: str = "flexcore.worker.v1"


class WorkerError(Exception):
    """A structured worker error.

    One of these becomes the `WorkerErrorDetail` of an ExecuteResponse
    whose status is not OK. `code` is a name from `WORKER_ERROR_CODES`
    (never an ad-hoc string); `message` is short, log-safe prose.
    """

    def __init__(
        self,
        code: str,
        message: str,
        *,
        retryable: bool = False,
        field: Optional[str] = None,
    ) -> None:
        if code not in WORKER_ERROR_CODES:
            raise ValueError(f"unknown worker error code: {code!r}")
        super().__init__(message)
        self.code = code
        self.message = message
        self.retryable = retryable
        self.field = field

    def is_refusal(self) -> bool:
        """True when the request was refused BEFORE any execution."""
        return self.code in REFUSAL_CODES

    def as_detail(self) -> dict[str, object]:
        """The wire shape (mirrors the proto `WorkerErrorDetail`)."""
        return {
            "code": self.code,
            "message": self.message,
            "retryable": self.retryable,
            "field": self.field or "",
        }


def invalid_request(message: str, field: Optional[str] = None) -> WorkerError:
    """A malformed envelope (missing mandatory field, wrong shape)."""
    return WorkerError(
        "WORKER_ERROR_CODE_INVALID_REQUEST",
        message,
        retryable=False,
        field=field,
    )


def unsupported_contract_version(received: str) -> WorkerError:
    """Major contract-version skew — an explicit refusal, never guessed."""
    return WorkerError(
        "WORKER_ERROR_CODE_UNSUPPORTED_CONTRACT_VERSION",
        f"worker speaks {CONTRACT_VERSION!r}; request carried "
        f"{received!r}",
        retryable=False,
        field="contract_version",
    )


def unsupported_payload_type(payload_type: str) -> WorkerError:
    """The payload type is unknown to this worker — refuse, never guess."""
    return WorkerError(
        "WORKER_ERROR_CODE_UNSUPPORTED_PAYLOAD_TYPE",
        f"worker does not accept payload_type {payload_type!r}",
        retryable=False,
        field="payload_type",
    )


def unavailable(detail: str) -> WorkerError:
    """The worker is DRAINING/STOPPING and accepts no new work."""
    return WorkerError(
        "WORKER_ERROR_CODE_UNAVAILABLE",
        detail,
        retryable=True,
        field=None,
    )


def internal_error(detail: str) -> WorkerError:
    """In-band execution failure AFTER the request started running."""
    return WorkerError(
        "WORKER_ERROR_CODE_INTERNAL",
        detail,
        retryable=True,
        field=None,
    )


def cancelled(detail: str) -> WorkerError:
    """The request was cancelled (Cancel rpc or the deadline)."""
    return WorkerError(
        "WORKER_ERROR_CODE_CANCELLED",
        detail,
        retryable=False,
        field=None,
    )


def config_invalid(detail: str) -> WorkerError:
    """Configuration is invalid — the worker fails closed at startup."""
    return WorkerError(
        "WORKER_ERROR_CODE_CONFIG_INVALID",
        detail,
        retryable=False,
        field=None,
    )


def grpc_status_name_for(code: str) -> str:
    """The canonical gRPC status NAME for an error code (pure logic).

    Returns the grpc.StatusCode name (e.g. "INVALID_ARGUMENT"). This
    function is pure string logic so the mapping is testable without
    grpcio; `map_error_to_grpc_status` below converts the name to the
    real status object at the grpcio seam.

    Mapping decision (recorded, DEFAULT): refusal codes map to
    INVALID_ARGUMENT / FAILED_PRECONDITION by refusal kind, CANCELLED to
    CANCELLED, and in-band INTERNAL failures to INTERNAL — the worker's
    own failure semantics stay in the result payload (status
    EXECUTE_STATUS_FAILED), never collapsed into the transport status.
    """
    mapping: dict[str, str] = {
        "WORKER_ERROR_CODE_NONE": "OK",
        "WORKER_ERROR_CODE_INVALID_REQUEST": "INVALID_ARGUMENT",
        "WORKER_ERROR_CODE_UNSUPPORTED_CONTRACT_VERSION": "INVALID_ARGUMENT",
        "WORKER_ERROR_CODE_UNSUPPORTED_PAYLOAD_TYPE": "INVALID_ARGUMENT",
        "WORKER_ERROR_CODE_UNAVAILABLE": "UNAVAILABLE",
        "WORKER_ERROR_CODE_INTERNAL": "INTERNAL",
        "WORKER_ERROR_CODE_CANCELLED": "CANCELLED",
        "WORKER_ERROR_CODE_CONFIG_INVALID": "FAILED_PRECONDITION",
    }
    if code not in mapping:
        raise ValueError(f"unknown worker error code: {code!r}")
    return mapping[code]


def map_error_to_grpc_status(error: WorkerError) -> object:
    """Map a `WorkerError` onto a real grpc status (grpcio seam).

    UNVERIFIED — grpcio API. grpcio was NEVER installed or executed in
    this loop; the `grpc.StatusCode[value]` lookup and the abort() /
    context usage pattern follow the documented grpcio API as of the
    2026-11-07 research cut-off and must be verified by the first human
    build/test pass (the template README records the deployment
    requirement). Imports are INSIDE this function so the rest of this
    module parses and tests without grpcio.
    """
    import grpc  # noqa: PLC0415 — deliberate deferred import; see docstring.

    status_name = grpc_status_name_for(error.code)
    # UNVERIFIED — grpcio API: StatusCode enum indexed by name.
    return grpc.StatusCode[status_name]
