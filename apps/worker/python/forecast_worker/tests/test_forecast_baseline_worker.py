# P15.02 — pytest suite for the forecast baseline worker (WRITTEN-UNRUN:
#          no pytest run was executed in this loop; the human build/test
#          gate owns the run. Every expectation below is hand-computed
#          from the rule sets — the same numbers as the Rust tests in
#          crates/flexcore-application/src/forecast_baselines.rs, pinning
#          the two-language mirror. Nothing is fabricated.)
#
# Run instruction (human gate):
#   pytest apps/worker/python/forecast_worker/tests/

import json
import subprocess
import sys
from pathlib import Path

WORKER = Path(__file__).resolve().parent.parent / "forecast_baseline_worker.py"

import importlib.util  # noqa: E402 — the worker path is computed above

_spec = importlib.util.spec_from_file_location("forecast_baseline_worker", WORKER)
worker = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(worker)


# ---------------------------------------------------------------------------
# The baseline rule sets (mirrors of the Rust rule sets)
# ---------------------------------------------------------------------------


def test_persistence_carries_the_last_value_forward():
    # HAND CHECK: history [-0.4, -0.5, -0.6] MW, horizon 3.
    assert worker.persistence_forecast([-0.4, -0.5, -0.6], 3) == [-0.6, -0.6, -0.6]


def test_persistence_preserves_the_callers_sign_convention():
    assert worker.persistence_forecast([0.0, 1.5], 1) == [1.5]


def test_persistence_refuses_empty_and_non_finite_history():
    for history in ([], [0.5, float("nan")], [0.5, float("inf")]):
        try:
            worker.persistence_forecast(history, 1)
        except (worker.WorkerRequestError, worker.ForecastComputationError):
            pass
        else:
            raise AssertionError("non-finite or empty history must be refused")


def test_seasonal_naive_returns_the_value_one_season_earlier():
    # HAND CHECK: history of 5, season 3, horizon 3 -> indices 2, 3, 4.
    assert worker.seasonal_naive_forecast([-0.0, -0.1, -0.2, -0.3, -0.4], 3, 3) == [
        -0.2,
        -0.3,
        -0.4,
    ]


def test_seasonal_naive_wraps_within_the_last_season():
    # HAND CHECK: history of 4, season 2, horizon 4.
    assert worker.seasonal_naive_forecast([10.0, 20.0, 30.0, 40.0], 2, 4) == [
        30.0,
        40.0,
        30.0,
        40.0,
    ]


def test_seasonal_naive_refuses_a_short_history():
    try:
        worker.seasonal_naive_forecast([1.0, 2.0], 3, 1)
    except worker.ForecastComputationError as error:
        assert "season_length + 1" in str(error)
    else:
        raise AssertionError("a short history must be refused")


def test_degree_day_fit_recovers_exact_coefficients():
    # HAND CHECK (the orthogonal-regressor construction shared with the
    # Rust test): intercept 1.0, hdd 2.0, cdd 3.0 — exact.
    orthogonal = [
        {"load_mw": 1.0, "heating_degree_days": 0.0, "cooling_degree_days": 0.0},
        {"load_mw": 3.0, "heating_degree_days": 1.0, "cooling_degree_days": 0.0},
        {"load_mw": 1.0, "heating_degree_days": 0.0, "cooling_degree_days": 0.0},
        {"load_mw": 1.0, "heating_degree_days": 0.0, "cooling_degree_days": 0.0},
        {"load_mw": 4.0, "heating_degree_days": 0.0, "cooling_degree_days": 1.0},
    ]
    coefficients = worker.fit_degree_day_regression(orthogonal)
    for got, want in zip(coefficients, (1.0, 2.0, 3.0)):
        assert abs(got - want) < 1e-9


def test_degree_day_predict_is_linear():
    coefficients = (1.0, 2.0, 3.0)
    assert worker.degree_day_predict(coefficients, [2.0, 0.0], [1.0, 0.0]) == [8.0, 1.0]


def test_degree_day_refuses_singular_design_and_negative_degree_days():
    constant = [
        {"load_mw": 1.0, "heating_degree_days": 0.0, "cooling_degree_days": 0.0},
        {"load_mw": 1.0, "heating_degree_days": 0.0, "cooling_degree_days": 0.0},
        {"load_mw": 1.0, "heating_degree_days": 0.0, "cooling_degree_days": 0.0},
    ]
    for samples in (
        constant,
        [
            {"load_mw": 1.0, "heating_degree_days": -1.0, "cooling_degree_days": 0.0},
            {"load_mw": 2.0, "heating_degree_days": 0.0, "cooling_degree_days": 0.0},
            {"load_mw": 3.0, "heating_degree_days": 0.0, "cooling_degree_days": 0.0},
        ],
    ):
        try:
            worker.fit_degree_day_regression(samples)
        except worker.ForecastComputationError:
            pass
        else:
            raise AssertionError("a singular or negative input must be refused")


# ---------------------------------------------------------------------------
# The request gate and the permission vocabulary (P15.01)
# ---------------------------------------------------------------------------


def _valid_request(**overrides):
    request = {
        "protocol": worker.PROTOCOL_ID,
        "mode": "baseline_forecast",
        "baseline": worker.BASELINE_PERSISTENCE,
        "permissions_declared": ["telemetry.read", "forecast.publish"],
        "scope": {"scope_kind": "site", "scope_id": "site-north"},
        "horizon": {"steps": 3, "interval_millis": 900000},
        "history": [-0.4, -0.5, -0.6],
        "season_length": None,
        "degree_day": None,
        "provenance": {"input_snapshot_id": "snapshot-RECORD", "request_id": "req-0001"},
    }
    request.update(overrides)
    return request


def test_the_never_granted_set_is_the_verbatim_kernel_mirror():
    assert worker.NEVER_GRANTED_PERMISSIONS == frozenset(
        {"dispatch.authorize", "dispatch.issue", "secret.read", "network.any"}
    )


def test_the_producer_vocabulary_has_no_dispatch_member():
    assert worker.PRODUCER_PERMISSIONS == frozenset(
        {"telemetry.read", "forecast.publish"}
    )


def test_requests_with_never_granted_permissions_are_refused_first():
    for permission in ("dispatch.authorize", "dispatch.issue", "secret.read", "network.any"):
        try:
            worker.validate_request(_valid_request(permissions_declared=[permission]))
        except worker.WorkerRequestError as error:
            assert "NEVER granted" in str(error)
        else:
            raise AssertionError("%s must be refused" % permission)


def test_requests_with_out_of_vocabulary_permissions_are_refused():
    try:
        worker.validate_request(_valid_request(permissions_declared=["schedule.propose"]))
    except worker.WorkerRequestError as error:
        assert "outside the forecast-producer vocabulary" in str(error)
    else:
        raise AssertionError("schedule.propose is not a producer permission")


def test_unknown_baselines_are_refused_with_no_default():
    try:
        worker.validate_request(_valid_request(baseline="forecast-baseline/ml-dream"))
    except worker.WorkerRequestError as error:
        assert "no default baseline" in str(error)
    else:
        raise AssertionError("an unknown baseline must be refused")


def test_requests_without_provenance_are_refused_before_computation():
    request = _valid_request()
    del request["provenance"]
    try:
        worker.validate_request(request)
    except worker.WorkerRequestError as error:
        assert "provenance" in str(error)
    else:
        raise AssertionError("a provenance-less request must be refused")


# ---------------------------------------------------------------------------
# The proposal-only boundary (ADR-010 extension, enforced in code)
# ---------------------------------------------------------------------------


def test_proposal_only_refuses_a_nested_forbidden_key():
    deep = {"outputs": {"ranking": [{"candidate_id": "c1", "approved": True}]}}
    try:
        worker.assert_proposal_only(deep)
    except RuntimeError as error:
        assert "proposal-only boundary violation" in str(error)
    else:
        raise AssertionError("a nested forbidden key must refuse emission")


def test_proposal_only_accepts_a_clean_tree():
    worker.assert_proposal_only({"outputs": {"values": [1.0, 2.0]}})


# ---------------------------------------------------------------------------
# The envelope posture (fixed, provenance-carrying, no skill claim)
# ---------------------------------------------------------------------------


def test_full_request_completes_with_the_fixed_posture():
    result = worker.run_forecast(_valid_request())
    assert result["ok"] is True
    assert result["proposal_only"] is True
    assert result["authority"] == "none"
    assert result["control_authority_mode_locked"] == worker.MODE_LOCKED == "simulation-shadow"
    assert result["forecast"]["baseline_version"] == "persistence.v1"
    assert result["forecast"]["outputs"]["values"] == [-0.6, -0.6, -0.6]
    assert result["honesty"]["model_registry_state"] == "UNTRAINED"
    assert "forecast-eval-set-v1" in result["honesty"]["eval_set_ref"]
    assert "EMPTY" in result["honesty"]["eval_set_status"]
    assert "NO-SKILL-UNTIL-EVALUATED" in result["honesty"]["no_skill_note"]
    worker.assert_proposal_only(result)


def test_an_in_band_refusal_is_a_failed_envelope_never_a_crash():
    result = worker.run_forecast(_valid_request(history=[]))
    assert result["ok"] is False
    assert result["provenance"]["termination_status"] == "failed"
    assert result["forecast"]["outputs"] is None
    assert result["provenance"]["diagnostics"], "the refusal carries diagnostics"
    worker.assert_proposal_only(result)


def test_the_boundary_statements_carry_the_adr_010_extension():
    result = worker.run_forecast(_valid_request())
    statements = " ".join(result["boundary_statements"])
    assert "never a command" in statements
    assert "structural, not conventional" in statements
    assert "every evaluation cell is EMPTY" in statements
    assert "UNTRAINED" in statements
    assert "telemetry.read + forecast.publish" in statements


def test_the_guarded_numpy_import_raises_the_named_requirement():
    try:
        worker.import_numpy()
    except ImportError as error:
        assert worker.NUMPY_DEPLOYMENT_REQUIREMENT in str(error)
        assert "DECISION-POINT" in str(error)
    else:
        # numpy IS installed in this environment: the guarded import
        # returning is not a failure of the gate (the gate exists so a
        # missing framework fails NAMED); record nothing.
        pass


# ---------------------------------------------------------------------------
# The transport contract: one JSON in, one JSON out; malformed -> nothing
# on stdout, exit 2 (subprocess-level; the human gate runs it the same way)
# ---------------------------------------------------------------------------


def test_main_emits_one_envelope_for_a_valid_request():
    proc = subprocess.run(
        [sys.executable, str(WORKER)],
        input=json.dumps(_valid_request()),
        capture_output=True,
        text=True,
    )
    assert proc.returncode == 0
    lines = [line for line in proc.stdout.splitlines() if line.strip()]
    assert len(lines) == 1, "exactly one JSON document on stdout"
    result = json.loads(lines[0])
    assert result["protocol"] == worker.PROTOCOL_ID


def test_main_emits_nothing_on_stdout_for_malformed_input():
    proc = subprocess.run(
        [sys.executable, str(WORKER)],
        input="{not json",
        capture_output=True,
        text=True,
    )
    assert proc.returncode == 2
    assert proc.stdout == "", "malformed input -> NOTHING on stdout"
    assert proc.stderr.strip() != "", "the reason goes to stderr"
