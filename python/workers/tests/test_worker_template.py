# P7.17 — The worker template test suite: pure-logic parts that RUN
#         WITHOUT grpcio, plus import-guarded server-seam tests.
#
# ══════════════════════════════════════════════════════════════════════════
# HONESTY HEADER
#
# 1. WRITTEN, NEVER RUN. pytest was never installed in this loop; no test
#    below has ever executed. The pure-logic tests are written against
#    stdlib-only template modules and are expected to pass at the first
#    human run; the server-seam tests are import-guarded
#    (`pytest.importorskip("grpc")`) so the suite skips — not fails — on
#    a machine without grpcio.
# 2. Run instruction (the recorded procedure, mirroring the P5.15
#    pattern): scratch venv, `pip install pytest` (version recorded then)
#    plus the requirements.txt pins once their UNPINNED-REVERIFY entries
#    are replaced with verified versions, then
#    `python -m pytest python/workers/tests/ -v` from the repository root
#    with `python/workers/` on `PYTHONPATH` (the package has no build
#    step; it is imported in place).
# 3. Coverage map (what each test pins, ledger P7.17 acceptance):
#    config parsing (fail-closed), health transitions (closed table),
#    error mapping (code vocabulary + status mapping + proto
#    name-alignment), correlation echo (mandatory on every response),
#    determinism flag (seed honoured, wall-clock excluded), envelope
#    validation (refusals before execution), shutdown sequence
#    (idempotent, grace cap), and the example echo worker (deterministic
#    digest, refusal of unknown payload types).
# ══════════════════════════════════════════════════════════════════════════

from __future__ import annotations

import hashlib
import re

import pytest

from worker_template import errors
from worker_template import example_echo_worker
from worker_template import handler
from worker_template import server
from worker_template.config import (
    DEFAULT_BIND_ADDR,
    DEFAULT_MAX_INFLIGHT,
    ConfigError,
    from_mapping,
)
from worker_template.health import (
    IllegalHealthTransition,
    HealthStateMachine,
    readiness_response,
)


# ---------------------------------------------------------------------------
# config.py — fail-closed parsing
# ---------------------------------------------------------------------------


class TestConfig:
    def test_missing_worker_id_fails_closed(self) -> None:
        with pytest.raises(ConfigError, match="FLEXCORE_WORKER_ID"):
            from_mapping({})

    def test_defaults_apply_when_keys_absent(self) -> None:
        config = from_mapping({"FLEXCORE_WORKER_ID": "worker-a"})
        assert config.worker_id == "worker-a"
        assert config.bind_addr == DEFAULT_BIND_ADDR  # DEFAULT: loopback
        assert config.max_inflight == DEFAULT_MAX_INFLIGHT  # DEFAULT: 4
        assert config.deterministic is True  # DEFAULT: deterministic ON
        assert config.drain_grace_ms == 30_000  # DEFAULT: 30 s grace
        assert config.log_level == "info"

    def test_invalid_bind_host_is_refused(self) -> None:
        with pytest.raises(ConfigError, match="not an allowed bind host"):
            from_mapping(
                {
                    "FLEXCORE_WORKER_ID": "worker-a",
                    "FLEXCORE_WORKER_BIND_ADDR": "example.com:50051",
                }
            )

    def test_invalid_port_range_refused(self) -> None:
        with pytest.raises(ConfigError, match="port out of range"):
            from_mapping(
                {
                    "FLEXCORE_WORKER_ID": "worker-a",
                    "FLEXCORE_WORKER_BIND_ADDR": "127.0.0.1:70000",
                }
            )

    def test_invalid_inflight_refused_not_defaulted(self) -> None:
        # Fail-closed: an INVALID value never silently becomes the default.
        with pytest.raises(ConfigError, match="expected a positive integer"):
            from_mapping(
                {
                    "FLEXCORE_WORKER_ID": "worker-a",
                    "FLEXCORE_WORKER_MAX_INFLIGHT": "not-a-number",
                }
            )

    def test_deterministic_flag_parsing(self) -> None:
        on = from_mapping(
            {"FLEXCORE_WORKER_ID": "w", "FLEXCORE_WORKER_DETERMINISTIC": "true"}
        )
        off = from_mapping(
            {"FLEXCORE_WORKER_ID": "w", "FLEXCORE_WORKER_DETERMINISTIC": "0"}
        )
        bad = {"FLEXCORE_WORKER_ID": "w", "FLEXCORE_WORKER_DETERMINISTIC": "maybe"}
        assert on.deterministic is True
        assert off.deterministic is False
        with pytest.raises(ConfigError, match="expected a boolean"):
            from_mapping(bad)


# ---------------------------------------------------------------------------
# health.py — the closed transition table
# ---------------------------------------------------------------------------


class TestHealth:
    def test_starts_starting_not_ready(self) -> None:
        machine = HealthStateMachine()
        assert machine.state == "STARTING"
        assert machine.is_ready() is False
        assert machine.accepts_work() is False

    def test_starting_to_ready_to_draining_to_stopping(self) -> None:
        machine = HealthStateMachine()
        assert machine.transition("READY") == "READY"
        assert machine.is_ready() is True
        machine.transition("DRAINING")
        assert machine.accepts_work() is False
        machine.transition("STOPPING")
        assert machine.state == "STOPPING"

    def test_draining_cannot_recover_to_ready(self) -> None:
        # Recorded rule: once draining, the worker is on its way out.
        machine = HealthStateMachine()
        machine.transition("READY")
        machine.transition("DRAINING")
        with pytest.raises(IllegalHealthTransition):
            machine.transition("READY")

    def test_unknown_state_refused(self) -> None:
        machine = HealthStateMachine()
        with pytest.raises(IllegalHealthTransition, match="unknown health state"):
            machine.transition("STOPPED")

    def test_starting_to_stopping_allowed(self) -> None:
        machine = HealthStateMachine()
        machine.transition("STOPPING")
        assert machine.state == "STOPPING"

    def test_readiness_response_shape(self) -> None:
        machine = HealthStateMachine()
        machine.transition("READY")
        response = readiness_response(machine, errors.CONTRACT_VERSION)
        assert response["state"] == "READY"
        assert response["ready"] is True
        assert response["contract_version"] == "flexcore.worker.v1"


# ---------------------------------------------------------------------------
# errors.py — the structured error vocabulary
# ---------------------------------------------------------------------------


class TestErrors:
    def test_unknown_code_rejected_at_construction(self) -> None:
        with pytest.raises(ValueError, match="unknown worker error code"):
            errors.WorkerError("WORKER_ERROR_CODE_NOT_A_REAL_CODE", "x")

    def test_refusal_vs_inband_classification(self) -> None:
        refusal = errors.unsupported_payload_type("unknown/type")
        assert refusal.is_refusal() is True
        inband = errors.internal_error("solver exploded")
        assert inband.is_refusal() is False
        assert inband.retryable is True

    def test_grpc_status_mapping(self) -> None:
        assert errors.grpc_status_name_for(
            "WORKER_ERROR_CODE_INVALID_REQUEST"
        ) == "INVALID_ARGUMENT"
        assert errors.grpc_status_name_for(
            "WORKER_ERROR_CODE_UNAVAILABLE"
        ) == "UNAVAILABLE"
        assert errors.grpc_status_name_for(
            "WORKER_ERROR_CODE_CONFIG_INVALID"
        ) == "FAILED_PRECONDITION"
        assert errors.grpc_status_name_for(
            "WORKER_ERROR_CODE_CANCELLED"
        ) == "CANCELLED"

    def test_error_codes_align_with_proto_enum_names(self) -> None:
        # The proto `WorkerErrorCode` enum (contracts/worker_api.proto)
        # is the wire definition of record; the Python vocabulary MUST
        # carry the same names. Parsed from the proto source so the pin
        # breaks loudly if either side drifts.
        import pathlib

        proto_path = (
            pathlib.Path(__file__).resolve().parent.parent
            / "contracts"
            / "worker_api.proto"
        )
        text = proto_path.read_text(encoding="utf-8")
        proto_names = set(
            re.findall(r"^\s*(WORKER_ERROR_CODE_[A-Z_]+)\s*=\s*\d+;", text, re.M)
        )
        assert proto_names == set(errors.WORKER_ERROR_CODES)

    def test_map_error_to_grpc_status_skips_without_grpcio(self) -> None:
        # The grpcio seam raises the named deployment requirement when
        # grpcio is absent — the error names the requirement, it is not
        # a bare ImportError leak.
        grpc = pytest.importorskip("grpc", reason="grpcio not installed here")
        status = errors.map_error_to_grpc_status(errors.cancelled("test"))
        assert status is grpc.StatusCode.CANCELLED  # UNVERIFIED — grpcio API


# ---------------------------------------------------------------------------
# handler.py — envelope validation, correlation echo, determinism
# ---------------------------------------------------------------------------


def _valid_request(**overrides: object) -> dict[str, object]:
    request: dict[str, object] = {
        "contract_version": "flexcore.worker.v1",
        "request_id": "req-0001",
        "correlation_id": "corr-9f2c",
        "payload_type": "flexcore.example.echo_request.v1",
        "payload": b"hello",
        "seed": 42,
        "idempotency_key": "idem-0001",
        "input_snapshot_id": "snap-1",
    }
    request.update(overrides)
    return request


class TestHandler:
    def test_correlation_and_request_echo_are_mandatory(self) -> None:
        logic = example_echo_worker.EchoWorkerLogic()
        response = handler.handle_execute(_valid_request(), logic)
        assert response["correlation_id"] == "corr-9f2c"
        assert response["request_id"] == "req-0001"
        assert response["status"] == "EXECUTE_STATUS_OK"

    def test_missing_correlation_refused_before_execution(self) -> None:
        logic = example_echo_worker.EchoWorkerLogic()
        with pytest.raises(errors.WorkerError) as excinfo:
            handler.handle_execute(_valid_request(correlation_id=""), logic)
        assert excinfo.value.code == "WORKER_ERROR_CODE_INVALID_REQUEST"
        assert excinfo.value.field == "correlation_id"

    def test_version_skew_refused(self) -> None:
        logic = example_echo_worker.EchoWorkerLogic()
        with pytest.raises(errors.WorkerError) as excinfo:
            handler.handle_execute(
                _valid_request(contract_version="flexcore.worker.v2"), logic
            )
        assert (
            excinfo.value.code
            == "WORKER_ERROR_CODE_UNSUPPORTED_CONTRACT_VERSION"
        )

    def test_minor_version_within_major_accepted(self) -> None:
        logic = example_echo_worker.EchoWorkerLogic()
        response = handler.handle_execute(
            _valid_request(contract_version="flexcore.worker.v1.1"), logic
        )
        assert response["status"] == "EXECUTE_STATUS_OK"

    def test_draining_worker_refuses_with_unavailable(self) -> None:
        logic = example_echo_worker.EchoWorkerLogic()
        with pytest.raises(errors.WorkerError) as excinfo:
            handler.handle_execute(
                _valid_request(), logic, accepts_work=False
            )
        assert excinfo.value.code == "WORKER_ERROR_CODE_UNAVAILABLE"
        assert excinfo.value.retryable is True

    def test_error_response_status_selection(self) -> None:
        refused = handler.handle_execute_error(
            "req-1",
            "corr-1",
            errors.unsupported_payload_type("x"),
        )
        assert refused["status"] == "EXECUTE_STATUS_REFUSED"
        failed = handler.handle_execute_error(
            "req-1",
            "corr-1",
            errors.internal_error("boom"),
            execution_started=True,
        )
        assert failed["status"] == "EXECUTE_STATUS_FAILED"
        cancelled = handler.handle_execute_error(
            "req-1", "corr-1", errors.cancelled("deadline")
        )
        assert cancelled["status"] == "EXECUTE_STATUS_CANCELLED"


# ---------------------------------------------------------------------------
# determinism (P7.42 contract surface)
# ---------------------------------------------------------------------------


class TestDeterminism:
    def test_seed_and_request_id_stamp_the_response(self) -> None:
        logic = example_echo_worker.EchoWorkerLogic()
        response = handler.handle_execute(_valid_request(seed=7), logic)
        determinism = response["determinism"]
        assert determinism["seed"] == 7
        assert determinism["request_id"] == "req-0001"
        assert determinism["idempotency_key"] == "idem-0001"
        assert determinism["deterministic"] is True
        assert determinism["seed_supported"] is True

    def test_same_input_same_digest(self) -> None:
        logic = example_echo_worker.EchoWorkerLogic()
        first = handler.handle_execute(_valid_request(), logic)
        second = handler.handle_execute(_valid_request(), logic)
        assert first["payload"] == second["payload"]
        assert hashlib.sha256(b"hello").hexdigest().encode("ascii") in (
            first["payload"]
        )

    def test_wall_clock_never_enters_result_payload(self) -> None:
        logic = example_echo_worker.EchoWorkerLogic()
        # now_ms is advisory provenance metadata only.
        response = handler.handle_execute(_valid_request(), logic, now_ms=123456)
        assert response["provenance"]["execution_ms"] == 123456
        assert b"123456" not in response["payload"]


# ---------------------------------------------------------------------------
# server.py — the pure shutdown sequence (no grpcio)
# ---------------------------------------------------------------------------


class TestShutdownSequence:
    def test_ready_drains_then_stops(self) -> None:
        machine = HealthStateMachine()
        machine.transition("READY")
        sequence = server.ShutdownSequence(machine)
        outcome = sequence.request_shutdown(30_000)
        assert outcome["accepted"] is True
        assert outcome["already_stopping"] is False
        assert machine.state == "DRAINING"
        # Idempotent second request during the drain window.
        again = sequence.request_shutdown(30_000)
        assert again["accepted"] is True
        assert machine.state == "DRAINING"

    def test_starting_stops_without_draining(self) -> None:
        machine = HealthStateMachine()
        sequence = server.ShutdownSequence(machine)
        outcome = sequence.request_shutdown(0)
        assert outcome["accepted"] is True
        assert machine.state == "STOPPING"

    def test_grace_cap_is_recorded(self) -> None:
        machine = HealthStateMachine()
        machine.transition("READY")
        sequence = server.ShutdownSequence(machine)
        assert sequence.grace_ms_cap == 60_000  # DEFAULT: recorded cap


# ---------------------------------------------------------------------------
# server seam — import-guarded (skips without grpcio)
# ---------------------------------------------------------------------------


class TestServerSeam:
    def test_require_grpc_names_the_deployment_requirement(self) -> None:
        grpc = pytest.importorskip("grpc", reason="grpcio not installed here")
        assert server.require_grpc() is grpc

    def test_named_import_error_when_grpcio_missing(self) -> None:
        try:
            import grpc  # noqa: F401
        except ImportError:
            # grpcio absent here (the expected state in this loop): the
            # choke point must raise the NAMED requirement, not leak a
            # bare ImportError.
            with pytest.raises(ImportError, match="grpcio deployment dependency"):
                server.require_grpc()
        else:
            pytest.skip("grpcio present: the missing-dependency path is unreachable")
