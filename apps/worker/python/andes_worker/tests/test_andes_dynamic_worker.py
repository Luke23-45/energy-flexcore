# P8.16 — The ANDES dynamic-worker test suite: pure-logic parts that RUN
#         WITHOUT ANDES installed, plus the named import-gate test.
#
# ══════════════════════════════════════════════════════════════════════════
# HONESTY HEADER
#
# 1. WRITTEN, NEVER RUN. pytest was never installed in this loop; no test
#    below has ever executed. The pure-logic tests target stdlib-only
#    worker functions and are expected to pass at the first human run; the
#    ANDES-path tests assert the HONESTY GATE (the named ImportError) and
#    are guarded so they skip loudly on a machine where ANDES actually IS
#    installed (there, the guarded path tests the UNVERIFIED run instead —
#    that execution belongs to the human build/test pass, never to this
#    suite's claims).
# 2. Run instruction (the recorded procedure):
#    `PYTHONPATH=apps/worker/python/andes_worker python -m pytest
#    apps/worker/python/andes_worker/tests -v` from the repository root.
# 3. What this suite pins (ledger P8.16 acceptance, the parts provable
#    without ANDES): fail-closed request validation (refusals before any
#    ANDES import), the proposal-only boundary in code (a forbidden key
#    anywhere in the result tree refuses emission), the result envelope's
#    fixed posture (proposal_only, authority none, mode lock, boundary
#    statements, §9.2 provenance field names), and the fail-closed
#    composition edge (malformed stdin → exit 2, nothing on stdout).
#    NOTHING here proves anything about ANDES behaviour.
# ══════════════════════════════════════════════════════════════════════════

from __future__ import annotations

import io
import json
import os
import sys

import pytest

sys.path.insert(
    0,
    os.path.join(os.path.dirname(os.path.abspath(__file__)), os.pardir),
)

import andes_dynamic_worker as worker  # noqa: E402 — path set above


def _valid_request(**overrides) -> dict:
    request = {
        "protocol": worker.PROTOCOL_ID,
        "study_type": "eigenvalue",
        "case_path": "cases/kundur_two_area/RECORD-CASE-FILE-PATH",
        "case_version": "recorded-at-first-run",
        "models": ["GENROU", "TGOV1", "SEXS"],
        "event": {"kind": "line_trip", "at_seconds": 1.0},
        "provenance": {
            "input_snapshot_id": "snapshot-RECORD",
            "request_id": "req-0001",
        },
    }
    request.update(overrides)
    return request


def _valid_time_domain_request() -> dict:
    return {
        "protocol": worker.PROTOCOL_ID,
        "study_type": "time_domain",
        "case_path": "cases/kundur_two_area/RECORD-CASE-FILE-PATH",
        "case_version": "recorded-at-first-run",
        "models": ["GENROU", "TGOV1", "SEXS", "PVD1", "ESD1"],
        "simulation": {
            "solver": "trapezoidal-RECORD-AT-RUN",
            "step_seconds": 0.01,
            "end_time_seconds": 10.0,
        },
        "monitored": [
            {"signal": "RECORD-SIGNAL-AT-RUN", "unit": "Hz"},
            {"signal": "RECORD-SIGNAL-AT-RUN", "unit": "pu"},
        ],
        "evaluation": {
            "baseline": 50.0,
            "event_time_s": 1.0,
            "tolerance": 0.1,
            "quantity": "frequency_hz",
        },
        "provenance": {
            "input_snapshot_id": "snapshot-RECORD",
            "request_id": "req-0002",
        },
    }


class TestRequestValidation:
    def test_valid_eigenvalue_request_accepted(self) -> None:
        validated = worker.validate_request(_valid_request())
        assert validated["study_type"] == "eigenvalue"
        assert validated["models"] == ["GENROU", "TGOV1", "SEXS"]
        assert validated["input_snapshot_id"] == "snapshot-RECORD"

    def test_valid_time_domain_request_accepted(self) -> None:
        validated = worker.validate_request(_valid_time_domain_request())
        assert validated["study_type"] == "time_domain"
        assert validated["evaluation"]["baseline"] == 50.0
        assert validated["monitored"][0]["unit"] == "Hz"

    def test_wrong_protocol_refused(self) -> None:
        with pytest.raises(worker.WorkerRequestError):
            worker.validate_request(_valid_request(protocol="flexcore-andes-worker/2"))

    def test_unknown_study_type_refused(self) -> None:
        with pytest.raises(worker.WorkerRequestError):
            worker.validate_request(_valid_request(study_type="ac_opf"))

    def test_missing_case_path_refused(self) -> None:
        with pytest.raises(worker.WorkerRequestError):
            worker.validate_request(_valid_request(case_path=""))

    def test_empty_models_refused(self) -> None:
        with pytest.raises(worker.WorkerRequestError):
            worker.validate_request(_valid_request(models=[]))

    def test_non_canonical_unit_refused(self) -> None:
        request = _valid_time_domain_request()
        request["monitored"] = [{"signal": "x", "unit": "GW"}]
        with pytest.raises(worker.WorkerRequestError):
            worker.validate_request(request)

    def test_time_domain_requires_simulation_block(self) -> None:
        request = _valid_time_domain_request()
        del request["simulation"]
        with pytest.raises(worker.WorkerRequestError):
            worker.validate_request(request)

    def test_non_positive_step_refused(self) -> None:
        request = _valid_time_domain_request()
        request["simulation"]["step_seconds"] = 0.0
        with pytest.raises(worker.WorkerRequestError):
            worker.validate_request(request)

    def test_non_monotone_monitor_list_refused_shape(self) -> None:
        # A monitored entry that is not an object is a refusal.
        request = _valid_time_domain_request()
        request["monitored"] = ["frequency"]
        with pytest.raises(worker.WorkerRequestError):
            worker.validate_request(request)

    def test_missing_provenance_refused(self) -> None:
        request = _valid_request()
        del request["provenance"]
        with pytest.raises(worker.WorkerRequestError):
            worker.validate_request(request)

    def test_missing_input_snapshot_refused(self) -> None:
        request = _valid_request()
        request["provenance"] = {"request_id": "req-0003"}
        with pytest.raises(worker.WorkerRequestError):
            worker.validate_request(request)

    def test_negative_tolerance_refused(self) -> None:
        request = _valid_time_domain_request()
        request["evaluation"]["tolerance"] = -0.1
        with pytest.raises(worker.WorkerRequestError):
            worker.validate_request(request)


class TestProposalOnlyBoundary:
    def test_clean_result_passes(self) -> None:
        validated = worker.validate_request(_valid_request())
        result = worker.build_result(
            validated=validated,
            termination_status="completed",
            outputs={"traces": None, "eigenvalues": [{"real": -0.5, "imag": 8.0}],
                     "steady_state": None},
            metrics=None,
            diagnostics=[],
            andes_version=None,
            execution_ms=0,
        )
        worker.assert_proposal_only(result)  # must not raise

    def test_forbidden_key_refused_anywhere_in_tree(self) -> None:
        validated = worker.validate_request(_valid_request())
        result = worker.build_result(
            validated=validated,
            termination_status="completed",
            outputs={"traces": None, "eigenvalues": None, "steady_state": None},
            metrics=None,
            diagnostics=[],
            andes_version=None,
            execution_ms=0,
        )
        smuggled = dict(result)
        smuggled["outputs"] = dict(result["outputs"])
        smuggled["outputs"]["authorization_token"] = "bearer-REDACTED"
        with pytest.raises(RuntimeError):
            worker.assert_proposal_only(smuggled)

    def test_nested_forbidden_key_refused(self) -> None:
        deep = {"a": {"b": {"command_id": "CMD-1"}}}
        with pytest.raises(RuntimeError):
            worker.assert_proposal_only(deep)

    def test_list_payloads_scanned(self) -> None:
        smuggled = [{"ok": True}, {"dispatch": {"target": "device-1"}}]
        with pytest.raises(RuntimeError):
            worker.assert_proposal_only(smuggled)


class TestResultEnvelope:
    def test_fixed_posture(self) -> None:
        validated = worker.validate_request(_valid_request())
        result = worker.build_result(
            validated=validated,
            termination_status="completed",
            outputs={"traces": None, "eigenvalues": None, "steady_state": None},
            metrics=None,
            diagnostics=[],
            andes_version=None,
            execution_ms=0,
        )
        assert result["proposal_only"] is True
        assert result["authority"] == "none"
        assert result["control_authority_mode_locked"] == worker.MODE_LOCKED
        assert worker.MODE_LOCKED == "simulation-shadow"
        assert len(result["boundary_statements"]) >= 4
        assert result["ok"] is True

    def test_provenance_field_names_match_template(self) -> None:
        # §9.2 field names, verbatim from the Phase 7 worker template.
        validated = worker.validate_request(_valid_request())
        result = worker.build_result(
            validated=validated,
            termination_status="failed",
            outputs={"traces": None, "eigenvalues": None, "steady_state": None},
            metrics=None,
            diagnostics=["study failed in-band: recorded reason"],
            andes_version=None,
            execution_ms=5,
        )
        for field in (
            "worker_version",
            "model_version",
            "solver_version",
            "input_snapshot_id",
            "termination_status",
            "diagnostics",
            "execution_ms",
        ):
            assert field in result["provenance"]
        assert result["provenance"]["termination_status"] == "failed"
        assert result["ok"] is False
        assert result["provenance"]["execution_ms"] == 5

    def test_model_scope_carries_the_declaration(self) -> None:
        validated = worker.validate_request(_valid_request())
        result = worker.build_result(
            validated=validated,
            termination_status="completed",
            outputs={"traces": None, "eigenvalues": None, "steady_state": None},
            metrics=None,
            diagnostics=[],
            andes_version="2.0.0",
            execution_ms=0,
        )
        assert result["model_scope"]["tool"] == "ANDES"
        assert result["model_scope"]["andes_pin"] == worker.ANDES_PINNED_VERSION
        assert result["model_scope"]["andes_version_reported"] == "2.0.0"
        assert result["model_scope"]["case_path"] == validated["case_path"]
        assert result["model_scope"]["declared_models"] == validated["models"]


class TestAndesImportGate:
    def _andes_importable(self) -> bool:
        try:
            import andes  # noqa: F401,PLC0415 — probe only

            return True
        except ImportError:
            return False

    def test_missing_andes_raises_the_named_requirement(self) -> None:
        if self._andes_importable():
            pytest.skip(
                "ANDES is installed on this machine; the named-requirement "
                "test only applies to environments without it (the run-"
                "path tests belong to the human build/test pass)"
            )
        with pytest.raises(ImportError) as excinfo:
            worker.run_study(_valid_request())
        assert worker.ANDES_DEPLOYMENT_REQUIREMENT in str(excinfo.value)
        assert "G-4" in str(excinfo.value)

    def test_module_imports_without_andes(self) -> None:
        # The honesty gate itself: the pure-logic core is importable with
        # no ANDES present (the deferred import is the mechanism).
        if self._andes_importable():
            pytest.skip("ANDES installed; the gate is trivially satisfied")
        assert callable(worker.validate_request)
        assert callable(worker.build_result)
        assert callable(worker.assert_proposal_only)


class TestFailClosedMain:
    def _run_main(self, stdin_text: str, capsys) -> int:
        monkeypatch_stdin = io.StringIO(stdin_text)
        saved_stdin = sys.stdin
        saved_stdout = sys.stdout
        sys.stdin = monkeypatch_stdin
        sys.stdout = io.StringIO()
        try:
            return worker.main()
        finally:
            captured_stdout = sys.stdout.getvalue()
            sys.stdin = saved_stdin
            sys.stdout = saved_stdout
            capsys.readouterr()  # drain pytest's own capture
            self._last_stdout = captured_stdout

    def test_malformed_json_exit_2_no_stdout(self, capsys) -> None:
        code = self._run_main("{not json", capsys)
        assert code == 2
        assert self._last_stdout == ""

    def test_wrong_protocol_exit_2_no_stdout(self, capsys) -> None:
        request = _valid_request()
        request["protocol"] = "something-else/1"
        code = self._run_main(json.dumps(request), capsys)
        assert code == 2
        assert self._last_stdout == ""

    def test_missing_andes_exit_2_no_stdout(self, capsys) -> None:
        try:
            import andes  # noqa: F401,PLC0415 — probe only

            pytest.skip("ANDES installed; the named-requirement path is untestable here")
        except ImportError:
            pass
        request = _valid_request()
        code = self._run_main(json.dumps(request), capsys)
        assert code == 2
        assert self._last_stdout == ""
