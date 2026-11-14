# P7.17 — The example concrete worker: a minimal, DETERMINISTIC echo
#         worker built on the template (stdlib-only, runnable today).
#
# ══════════════════════════════════════════════════════════════════════════
# HONESTY HEADER
#
# 1. The ECHO LOGIC below is stdlib-only and genuinely runnable today
#    (`python -c` or the template tests) — it computes a deterministic
#    result (payload echo + SHA-256 digest) with zero dependencies.
# 2. The `main()` entrypoint is the only part that needs grpcio: it
#    calls `server.serve`, which raises the named deployment requirement
#    when grpcio is absent. In this loop it was NEVER executed.
# 3. Activation note (recorded): wiring the generated
#    `flexcore.worker.v1` servicer (buf generation of
#    `contracts/worker_api.proto`) into `serve`'s
#    `service_addition_cb` is the FIRST human build step — the cb shape
#    is fixed in server.py; the generated import happens HERE, in the
#    concrete worker, never in the template.
# 4. Scope: this worker demonstrates the template. It computes nothing
#    numerical, holds no caches, and has no authority — its output is a
#    proposal by construction (ADR-010).
# ══════════════════════════════════════════════════════════════════════════

from __future__ import annotations

import hashlib
import sys

from worker_template import errors
from worker_template.config import ConfigError, from_env
from worker_template.handler import handle_execute
from worker_template.health import HealthStateMachine
from worker_template.server import (
    ShutdownSequence,
    graceful_stop,
    make_signal_hook,
    serve,
)

# The payload types this example accepts (one entry, deliberately).
ACCEPTED_PAYLOAD_TYPES: frozenset[str] = frozenset(
    {"flexcore.example.echo_request.v1"}
)


class EchoWorkerLogic:
    """The example worker's computation: deterministic payload echo.

    `evaluate` returns `(result_payload, provenance)`. The result is a
    small deterministic JSON-ish document (built without importing json
    to stay a pure byte transformation): the echo of the input payload
    plus its SHA-256 digest — the same input ALWAYS produces the same
    output bytes, which is exactly the property P7.42's replay compares.
    """

    def evaluate(
        self, request: dict[str, object]
    ) -> tuple[bytes, dict[str, object]]:
        payload_type = request["payload_type"]
        if payload_type not in ACCEPTED_PAYLOAD_TYPES:
            raise errors.unsupported_payload_type(str(payload_type))
        raw = request.get("payload", b"")
        if not isinstance(raw, bytes):
            raise errors.invalid_request(
                "payload must be bytes", field="payload"
            )
        digest = hashlib.sha256(raw).hexdigest()
        result = (
            b'{"echo_digest":"' + digest.encode("ascii") + b'"}'
        )
        provenance = {
            "worker_version": "flexcore-worker-template-example/0.1.0",
            "model_version": "",  # no domain model: an echo has none
            "solver_version": "",  # no solver: never claim one (AGENTS.md rule 4)
            "input_snapshot_id": request.get("input_snapshot_id", ""),
            "termination_status": "completed",
            "diagnostics": [
                "example echo worker: deterministic payload digest; "
                "no domain computation, no authority, proposal-only"
            ],
            # execution_ms is stamped by the handler from the advisory
            # clock; the logic itself never reads a wall clock.
            "execution_ms": 0,
        }
        return result, provenance


def main() -> int:
    """The composition edge: config -> health -> server -> signals.

    Fails closed: any `ConfigError` exits non-zero BEFORE any port is
    bound; a missing grpcio raises the named deployment requirement.
    """
    try:
        config = from_env()
    except ConfigError as exc:
        print(f"worker config invalid, refusing to start: {exc}", file=sys.stderr)
        return 2

    machine = HealthStateMachine()
    logic = EchoWorkerLogic()
    sequence = ShutdownSequence(machine)

    def service_addition_cb(server: object) -> None:
        # ACTIVATION NOTE (recorded): the generated servicer registers
        # here once buf generation has produced the stubs, e.g.:
        #   generated.add_FlexcoreWorkerServicer_to_service(
        #       ExampleWorkerServicer(logic, machine, sequence), server)
        # UNVERIFIED — grpcio API; the generated module does not exist
        # in this loop. Until then the server would start with no rpc
        # implementation, which is refused by construction: this cb
        # deliberately does nothing and the example is NOT served.
        del server

    server = serve(config, machine, service_addition_cb)
    hook = make_signal_hook(sequence)
    try:
        import signal

        signal.signal(signal.SIGTERM, lambda *_: hook())
        signal.signal(signal.SIGINT, lambda *_: hook())
    except (ValueError, OSError):  # pragma: no cover - non-main thread
        pass  # hook unavailable: the supervisor's Shutdown rpc remains

    # Block until termination: the SIGTERM/SIGINT hook drives the
    # drain+stop sequence; this line is the wait, not the shutdown.
    # UNVERIFIED — grpcio API (`wait_for_termination`); KeyboardInterrupt
    # covers the SIGINT path when the hook is unavailable.
    try:
        server.wait_for_termination()  # type: ignore[attr-defined]
    except KeyboardInterrupt:  # pragma: no cover - interactive path
        pass
    sequence.request_shutdown(config.drain_grace_ms)
    graceful_stop(server, machine, config.drain_grace_ms / 1000.0)
    return 0


if __name__ == "__main__":  # pragma: no cover - composition edge
    raise SystemExit(main())


# The pure handle_execute path, exported for tests without a server:
__all__ = ["ACCEPTED_PAYLOAD_TYPES", "EchoWorkerLogic", "handle_execute"]
