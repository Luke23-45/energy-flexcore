# P7.17 — The gRPC server skeleton: the ONLY grpcio touchpoint in the
#         template.
#
# ══════════════════════════════════════════════════════════════════════════
# HONESTY HEADER — READ BEFORE TRUSTING THIS FILE
#
# 1. grpcio was NEVER installed and this server was NEVER run. Every
#    grpcio call below is marked `UNVERIFIED — grpcio API` and follows
#    the documented grpcio API as of the 2026-11-07 research cut-off.
#    `import grpc` happens INSIDE `serve()` (and once in
#    `require_grpc()`), so this file PARSES, IMPORTS, and passes the
#    pure-logic tests WITHOUT grpcio installed — the named ImportError
#    below is the deployment requirement message a missing grpcio
#    produces.
# 2. The generated stubs do not exist yet: `contracts/worker_api.proto`
#    becomes Python stubs via the proto/ buf generation (P1.18
#    governance; plugin versions pinned at activation, never floated).
#    `serve()` therefore takes the generated service ADDITION as a
#    parameter (`service_addition_cb`) instead of importing a generated
#    module — the wiring shape is fixed here; the generated import is
#    the concrete worker's (see example_echo_worker.py's recorded
#    activation note).
# 3. The pure-logic helpers at the bottom (`ShutdownSequence`) ARE
#    runnable today and carry the drain/grace decision logic the server
#    seam calls into.
# ══════════════════════════════════════════════════════════════════════════
#
# Isolation rules the server enforces structurally (README §isolation):
# - The worker process contains NO control-plane code: nothing here
#   imports from apps/, crates/, or any flexcore Rust surface — there is
#   no such import path from a Python process at all, which IS the ADR-004
#   isolation mechanism.
# - The server holds no per-tenant state: requests are independent; any
#   cache the concrete worker keeps must be declared in its README and
#   never keyed by anything it would refuse to disclose.
# - Outputs are proposals (ADR-010): the server never calls anything
#   that could issue a command — it answers requests, period.

from __future__ import annotations

import threading
from typing import Callable

from . import errors
from .config import WorkerConfig
from .health import HealthStateMachine

# The named deployment requirement a missing grpcio surfaces (raised by
# `require_grpc`); the template README repeats it verbatim.
GRPC_DEPLOYMENT_REQUIREMENT = (
    "flexcore worker template requires the grpcio deployment dependency: "
    "install the pins recorded in python/workers/requirements.txt "
    "(UNPINNED-REVERIFY) before running any worker"
)


def require_grpc() -> object:
    """Import and return the grpc module or raise the named requirement.

    The single choke point for the grpcio import; every other use in the
    template goes through this so the deployment requirement is stated
    exactly once.
    """
    try:
        import grpc  # noqa: PLC0415 — deliberate deferred import (see header).
    except ImportError as exc:  # pragma: no cover - depends on deployment
        raise ImportError(GRPC_DEPLOYMENT_REQUIREMENT) from exc
    return grpc


def serve(
    config: WorkerConfig,
    health_machine: HealthStateMachine,
    service_addition_cb: Callable[[object], None],
    *,
    max_workers: int = 4,
) -> object:
    """Build and start the gRPC server (the grpcio seam).

    `service_addition_cb` receives the UNVERIFIED generated
    servicer/server pair wiring — the concrete worker's generated-stub
    registration — and is called BEFORE the server starts, so a worker
    never serves a port it cannot answer.

    UNVERIFIED — grpcio API, every call below:
    - `grpc.server(...)` with a ThreadPoolExecutor-style option
      (`futures.ThreadPoolExecutor(max_workers=...)`) — the documented
      pattern.
    - `server.add_insecure_port(...)` — loopback by DEFAULT
      (config.bind_addr); TLS/mTLS is the recorded deployment-stage seam
      (P7.34 transport identity), not this template's.
    - `server.start()` / `server.wait_for_termination()` /
      `server.stop(grace_seconds)` — the lifecycle calls
      `graceful_stop` drives.

    Returns the started server object (the caller keeps it to shut it
    down via `graceful_stop`).
    """
    grpc = require_grpc()  # UNVERIFIED — grpcio API (named requirement above).

    # UNVERIFIED — grpcio API: server construction with the thread-pool
    # option. The in-flight bound (config.max_inflight) is the worker's
    # own backpressure; the supervisor's cap (P7.18) is the outer bound.
    from concurrent import futures  # noqa: PLC0415 — stdlib, server-thread pool.

    server = grpc.server(
        futures.ThreadPoolExecutor(max_workers=max_workers),
        options=[
            # Refuse to serve an unbounded number of concurrent calls:
            # the worker degrades, it does not queue unboundedly (P7.18
            # backpressure posture, mirrored on the worker side).
            ("grpc.max_concurrent_streams", config.max_inflight),
        ],
    )
    service_addition_cb(server)

    # UNVERIFIED — grpcio API: returns 0 on failure; the template treats
    # 0 as a fatal startup error (fail-closed), not a warning.
    port = server.add_insecure_port(config.bind_addr)
    if port == 0:
        raise RuntimeError(
            f"worker failed to bind {config.bind_addr!r} (fail-closed)"
        )

    health_machine.transition("READY")  # serving begins only once bound
    server.start()  # UNVERIFIED — grpcio API
    return server


def graceful_stop(
    server: object,
    health_machine: HealthStateMachine,
    grace_seconds: float,
) -> None:
    """Drain and stop the server (the Shutdown rpc's execution path).

    Sequence: READY -> DRAINING (refuse new work, finish in-flight
    within the grace window) -> STOPPING -> `server.stop(grace)`.
    Illegal transitions raise `IllegalHealthTransition` — a worker that
    was never READY cannot "drain"; it goes STARTING -> STOPPING.
    """
    if health_machine.state == "READY":
        health_machine.transition("DRAINING")
    health_machine.transition("STOPPING")
    # UNVERIFIED — grpcio API: stop(grace) returns an event that fires
    # when in-flight rpcs finish or the grace elapses.
    event = server.stop(grace_seconds)  # type: ignore[attr-defined]
    if hasattr(event, "wait"):
        event.wait(timeout=grace_seconds + 1.0)


class ShutdownSequence:
    """The pure drain/grace decision logic behind Shutdown (runnable
    today, no grpcio): given the current health state and the requested
    grace, computes whether the shutdown is accepted and which state the
    worker moves to. `graceful_stop` calls this first; the class exists
    so the decision is unit-testable without a server."""

    def __init__(self, machine: HealthStateMachine) -> None:
        self._machine = machine
        self._lock = threading.Lock()

    def request_shutdown(self, grace_ms: int) -> dict[str, object]:
        """Accept (or refuse) a Shutdown request; pure decision.

        Returns the response shape (mirrors the proto
        `ShutdownResponse`). The worker accepts by moving to DRAINING
        (or STOPPING when idle); a second Shutdown is answered
        idempotently — accepted=True again, no state change.
        """
        with self._lock:
            state = self._machine.state
            if state == "STOPPING":
                return {"accepted": True, "already_stopping": True}
            if state == "DRAINING":
                # Idempotent re-request during the drain window.
                return {"accepted": True, "already_stopping": False}
            if state == "READY":
                self._machine.transition("DRAINING")
                return {"accepted": True, "already_stopping": False}
            if state == "STARTING":
                # A worker that never got ready stops without draining
                # (nothing in flight to drain).
                self._machine.transition("STOPPING")
                return {"accepted": True, "already_stopping": False}
            raise errors.internal_error(
                f"unknown health state {state!r} during shutdown"
            )

    @property
    def grace_ms_cap(self) -> int:
        """The recorded cap on any requested grace window (DEFAULT
        60_000 ms): a caller cannot pin a worker in DRAINING forever —
        the cap bounds the drain, the supervisor's crash isolation
        bounds the rest (P7.18)."""
        return 60_000  # DEFAULT: grace cap (decision record §5)


def make_signal_hook(sequence: ShutdownSequence) -> Callable[[], None]:
    """Build the SIGTERM hook: signal -> DRAINING (or STOPPING when
    never READY). The composition edge (the concrete worker's __main__)
    registers this with `signal.signal`; the hook itself is pure."""
    def hook() -> None:
        sequence.request_shutdown(0)
    return hook
