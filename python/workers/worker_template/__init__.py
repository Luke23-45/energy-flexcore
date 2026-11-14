# P7.17 — The FlexCore Class C worker template package.
#
# The GENERIC template every Python numerical/ML worker starts from
# (report §5.4 Class C; ADR-004: Python is isolated to workers, never a
# backend; ADR-008: worker contracts are Protobuf/gRPC). The template is
# consistent with — not a duplicate of — the P5.15 pandapower grid
# worker (`apps/worker/python/grid_worker.py`): that worker is the
# concrete precedent; this package is the shape any NEW worker begins
# from (config, health, structured errors, correlation echo,
# determinism, graceful shutdown) with its domain logic as the only
# custom part.
#
# Module map:
# - `config`   — env-driven `WorkerConfig`, fail-closed defaults.
# - `health`   — the STARTING/READY/DRAINING/STOPPING state machine.
# - `errors`   — the structured error vocabulary (mirrors the proto
#                `WorkerErrorCode`; single grpcio seam for status
#                mapping).
# - `handler`  — envelope validation, correlation-id echo, determinism
#                stamping, and the `WorkerLogic` hook the concrete
#                worker implements.
# - `server`   — the gRPC server skeleton (the ONLY grpcio touchpoint;
#                imports inside functions with a named deployment
#                requirement) + the pure `ShutdownSequence`.
#
# Honest status: WRITTEN-UNRUN. grpcio was never installed; the server
# seam is UNVERIFIED. Everything except `server.serve` /
# `server.graceful_stop` / `errors.map_error_to_grpc_status` is
# stdlib-only and runnable today (see `tests/test_worker_template.py`).

from .config import (  # noqa: F401 — template's public surface.
    CONTRACT_VERSION,
    ConfigError,
    WorkerConfig,
    from_env,
    from_mapping,
)
from .errors import WorkerError  # noqa: F401
from .health import (  # noqa: F401
    HEALTH_STATES,
    HealthStateMachine,
    IllegalHealthTransition,
)
from .handler import WorkerLogic, handle_execute, validate_envelope  # noqa: F401

__all__ = [
    "CONTRACT_VERSION",
    "ConfigError",
    "HEALTH_STATES",
    "WorkerConfig",
    "WorkerError",
    "WorkerLogic",
    "HealthStateMachine",
    "IllegalHealthTransition",
    "handle_execute",
    "validate_envelope",
    "from_env",
    "from_mapping",
]
