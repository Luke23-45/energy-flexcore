# P8.39 — The co-simulation cross-check test suite: the hand-computed
#         checker cases as pytest tests (WRITTEN, NEVER RUN — see header).
#
# ══════════════════════════════════════════════════════════════════════════
# HONESTY HEADER
#
# 1. WRITTEN, NEVER RUN. pytest was never installed in this loop; no test
#    below has ever executed. The expected values are HAND COMPUTATIONS
#    from the constructed inputs (the arithmetic is stated in each test's
#    comment and in the module's in-module `run_self_tests()`), never
#    observed outputs of the code under test.
# 2. Run instruction (the recorded procedure): a scratch venv, pytest
#    installed per the recorded UNPINNED-REVERIFY discipline, then
#    `PYTHONPATH=apps/worker/python/andes_worker python -m pytest
#    apps/worker/python/andes_worker/tests -v` from the repository root.
#    The sys.path insertion below makes the suite robust to a direct
#    `pytest apps/worker/python/andes_worker/tests` invocation as well.
# 3. These tests pin the P8.39 cross-checker DEFINITIONS (the recorded
#    comparison rules and refusal discipline), not simulator behaviour:
#    no ANDES, no HELICS, no pandapower, no I/O, no network — the module
#    is pure by construction and the suite must stay able to run without
#    any of them installed.
# 4. The test arithmetic pins the DEFINITIONS; it is NOT a cross-check
#    measurement and fills no EMPTY cell in the decision record
#    (39_cosim_crosschecks.md).
# ══════════════════════════════════════════════════════════════════════════

from __future__ import annotations

import os
import sys

import pytest

sys.path.insert(
    0,
    os.path.join(os.path.dirname(os.path.abspath(__file__)), os.pardir),
)

import cosim_crosschecks as cc  # noqa: E402 — path set above


# Shared constructed inputs (identical to the in-module self-tests so the
# two suites pin the same definitions).

SHARED_TIMES = [0.0, 3600.0, 7200.0]  # s; a shared 1-hour-step time base
SCHEDULE = [2.0, 3.0, -1.0]  # MW; P > 0 = export
DYNAMIC_ENDPOINTS = [2.0, 3.05, -1.0]  # MW

BUSES = ["bus_1", "bus_2", "bus_3"]
V_PP = [1.000, 0.980, 1.020]  # pu (pandapower PF)
V_AN = [1.000, 0.981, 1.020]  # pu (ANDES steady-state PF)


def _endpoint_agreement(values_b=DYNAMIC_ENDPOINTS, times_b=None, **overrides):
    kwargs = dict(
        tolerance=0.1,
        label_a="quasi-static schedule endpoints",
        label_b="ANDES TDS end-of-window endpoints",
        class_a="steady_state",
        class_b="dynamic",
        quantity="p_mw",
        sign_convention_a="export_positive",
        sign_convention_b="export_positive",
    )
    kwargs.update(overrides)
    return cc.endpoint_agreement(
        SHARED_TIMES, SCHEDULE, times_b if times_b is not None else SHARED_TIMES,
        values_b, **kwargs
    )


class TestEndpointAgreement:
    def test_within_pre_registered_tolerance(self) -> None:
        # HAND: deltas = [|2.0-2.0|, |3.05-3.0|, |-1.0-(-1.0)|]
        #            = [0.0, 0.05, 0.0] MW → max 0.05 MW at t=3600.0 s,
        # within the pre-registered 0.1 MW tolerance.
        record = _endpoint_agreement()
        assert record["comparison"]["within_tolerance"] is True
        assert record["comparison"]["max_abs_delta"] == pytest.approx(0.05)
        assert record["comparison"]["max_delta_time_s"] == 3600.0
        assert record["comparison"]["per_sample_abs_deltas"] == [0.0, 0.05, 0.0]
        assert record["comparison"]["shared_sample_count"] == 3

    def test_beyond_tolerance_reported_not_absorbed(self) -> None:
        # HAND: dynamic endpoints [2.0, 3.2, -1.0] → deltas
        # [0.0, 0.2, 0.0] → max 0.2 MW > 0.1 MW → disagreement is
        # reported; the tolerance is never widened to fit.
        record = _endpoint_agreement(values_b=[2.0, 3.2, -1.0])
        assert record["comparison"]["within_tolerance"] is False
        assert record["comparison"]["max_abs_delta"] == pytest.approx(0.2)

    def test_mismatched_time_base_refused_never_interpolated(self) -> None:
        # HAND: the third timestamps differ (7200 vs 10800 s) — comparison
        # refused; interpolation would fabricate samples neither model
        # produced.
        with pytest.raises(cc.CrossCheckError, match="TIME_BASE_MISMATCH"):
            _endpoint_agreement(times_b=[0.0, 3600.0, 10800.0])

    def test_length_mismatch_refused(self) -> None:
        # HAND: 2 vs 3 samples → TIME_BASE_MISMATCH (never padded).
        with pytest.raises(cc.CrossCheckError, match="TIME_BASE_MISMATCH"):
            cc.endpoint_agreement(
                [0.0, 3600.0], [2.0, 3.0], SHARED_TIMES, DYNAMIC_ENDPOINTS,
                tolerance=0.1,
                label_a="a", label_b="b",
                class_a="steady_state", class_b="dynamic",
                quantity="p_mw",
                sign_convention_a="export_positive",
                sign_convention_b="export_positive",
            )

    def test_no_verdict_without_pre_registered_tolerance(self) -> None:
        # X-8: a None tolerance is refused — the comparison does not exist
        # without a pre-registered tolerance, and none is defaulted.
        with pytest.raises(
            cc.CrossCheckError, match="THRESHOLD_NOT_PRE_REGISTERED"
        ):
            _endpoint_agreement(tolerance=None)

    def test_sign_convention_mismatch_refused(self) -> None:
        # §10.2: the sign convention is DECLARED; a comparison across
        # differently-declared conventions is refused (matching
        # conventions — the shared helper — are accepted throughout).
        with pytest.raises(cc.CrossCheckError, match="SIGN_CONVENTION_MISMATCH"):
            _endpoint_agreement(sign_convention_b="load_positive")

    def test_unknown_quantity_refused(self) -> None:
        # Units are declared, never assumed (§10.2).
        with pytest.raises(cc.CrossCheckError, match="QUANTITY_UNKNOWN"):
            _endpoint_agreement(quantity="horsepower")

    def test_unknown_model_class_refused(self) -> None:
        # A class outside the closed vocabulary makes the X-5 separation
        # unenforceable — refused.
        with pytest.raises(cc.CrossCheckError, match="MODEL_CLASS_UNKNOWN"):
            _endpoint_agreement(class_a="transient")

    def test_nonfinite_value_refused(self) -> None:
        # A NaN is refused before any delta is computed.
        with pytest.raises(cc.CrossCheckError, match="series_a"):
            cc.endpoint_agreement(
                SHARED_TIMES, [2.0, float("nan"), -1.0], SHARED_TIMES,
                DYNAMIC_ENDPOINTS,
                tolerance=0.1,
                label_a="a", label_b="b",
                class_a="steady_state", class_b="dynamic",
                quantity="p_mw",
                sign_convention_a="export_positive",
                sign_convention_b="export_positive",
            )


class TestVectorAgreement:
    def test_within_pre_registered_tolerance(self) -> None:
        # HAND: deltas = [0.000, 0.001, 0.000] pu → max 0.001 pu at
        # bus_2, within the pre-registered 0.005 pu tolerance.
        record = cc.vector_agreement(
            BUSES, V_PP, BUSES, V_AN,
            tolerance=0.005,
            label_a="pandapower PF bus voltages",
            label_b="ANDES steady-state PF bus voltages",
            class_a="steady_state", class_b="steady_state",
            quantity="voltage_pu",
            sign_convention_a="nominal_base",
            sign_convention_b="nominal_base",
        )
        assert record["comparison"]["within_tolerance"] is True
        assert record["comparison"]["max_abs_delta"] == pytest.approx(0.001)
        assert record["comparison"]["max_delta_element"] == "bus_2"
        assert record["comparison"]["per_element_abs_deltas"] == [0.0, 0.001, 0.0]

    def test_beyond_tolerance_recorded_for_explanation(self) -> None:
        # HAND: ANDES [1.000, 0.990, 1.020] → deltas [0.000, 0.010, 0.000]
        # → max 0.01 pu > 0.005 pu → beyond; recorded AND to be explained
        # (model conventions), never absorbed by widening (X-8).
        record = cc.vector_agreement(
            BUSES, V_PP, BUSES, [1.000, 0.990, 1.020],
            tolerance=0.005,
            label_a="pandapower PF bus voltages",
            label_b="ANDES steady-state PF bus voltages",
            class_a="steady_state", class_b="steady_state",
            quantity="voltage_pu",
            sign_convention_a="nominal_base",
            sign_convention_b="nominal_base",
        )
        assert record["comparison"]["within_tolerance"] is False
        assert record["comparison"]["max_abs_delta"] == pytest.approx(0.01)

    def test_reordered_index_base_refused_never_silently_aligned(self) -> None:
        # A reordered index list is refused — a silent alignment could
        # compare a voltage against the wrong bus.
        with pytest.raises(cc.CrossCheckError, match="INDEX_BASE_MISMATCH"):
            cc.vector_agreement(
                BUSES, V_PP, ["bus_1", "bus_3", "bus_2"], V_AN,
                tolerance=0.005,
                label_a="a", label_b="b",
                class_a="steady_state", class_b="steady_state",
                quantity="voltage_pu",
                sign_convention_a="nominal_base",
                sign_convention_b="nominal_base",
            )

    def test_negative_tolerance_refused(self) -> None:
        with pytest.raises(cc.CrossCheckError, match="TOLERANCE_INVALID"):
            cc.vector_agreement(
                BUSES, V_PP, BUSES, V_AN,
                tolerance=-0.005,
                label_a="a", label_b="b",
                class_a="steady_state", class_b="steady_state",
                quantity="voltage_pu",
                sign_convention_a="nominal_base",
                sign_convention_b="nominal_base",
            )

    def test_flows_compared_in_canonical_units(self) -> None:
        # HAND: branch flows (MW) pp = [10.0, -5.0]; ANDES = [10.0, -5.0]
        # → deltas [0.0, 0.0] → max 0.0 within any tolerance; the same
        # index/quantity discipline as the voltage case.
        record = cc.vector_agreement(
            ["branch_1", "branch_2"], [10.0, -5.0],
            ["branch_1", "branch_2"], [10.0, -5.0],
            tolerance=0.01,
            label_a="pandapower PF branch flows",
            label_b="ANDES steady-state PF branch flows",
            class_a="steady_state", class_b="steady_state",
            quantity="p_mw",
            sign_convention_a="export_positive",
            sign_convention_b="export_positive",
        )
        assert record["comparison"]["within_tolerance"] is True
        assert record["comparison"]["max_abs_delta"] == pytest.approx(0.0)


class TestFederateTimeStepCheck:
    def test_consistent_declared_steps(self) -> None:
        report = cc.federate_time_step_check([
            {"name": "flexcore_asset", "declared_time_step_seconds": 1.0},
            {"name": "flexcore_grid", "declared_time_step_seconds": 1.0},
            {"name": "flexcore_research", "declared_time_step_seconds": 1.0},
        ])
        assert report["time_step_consistent"] is True
        assert report["mismatch_detected"] is False

    def test_mismatch_detected_and_named(self) -> None:
        # flexcore_grid declares 2.0 s against the others' 1.0 s: detected,
        # named, and the interaction claim refused — never silently
        # absorbed.
        report = cc.federate_time_step_check([
            {"name": "flexcore_asset", "declared_time_step_seconds": 1.0},
            {"name": "flexcore_grid", "declared_time_step_seconds": 2.0},
            {"name": "flexcore_research", "declared_time_step_seconds": 1.0},
        ])
        assert report["mismatch_detected"] is True
        assert report["time_step_consistent"] is False
        assert report["mismatching_federates"] == ["flexcore_grid"]
        assert report["verdict"].startswith("INTERACTION_CLAIM_REFUSED")

    def test_single_federate_refused(self) -> None:
        # A one-federate 'federation' has nothing to agree with.
        with pytest.raises(
            cc.CrossCheckError, match="FEDERATION_NEEDS_TWO_FEDERATES"
        ):
            cc.federate_time_step_check(
                [{"name": "flexcore_asset", "declared_time_step_seconds": 1.0}]
            )

    def test_invalid_time_step_refused(self) -> None:
        with pytest.raises(cc.CrossCheckError, match="TIME_STEP_INVALID"):
            cc.federate_time_step_check([
                {"name": "flexcore_asset", "declared_time_step_seconds": 1.0},
                {"name": "flexcore_grid", "declared_time_step_seconds": 0.0},
            ])


class TestGrantedTimeCheck:
    def test_synchronized_no_dropout(self) -> None:
        # HAND: both federates complete to 3600.0 s = scenario end.
        report = cc.granted_time_check(
            [
                {"name": "flexcore_asset", "final_granted_time_s": 3600.0,
                 "granted_step_count": 3600},
                {"name": "flexcore_grid", "final_granted_time_s": 3600.0,
                 "granted_step_count": 3600},
            ],
            scenario_end_time_s=3600.0,
        )
        assert report["synchronization_consistent"] is True
        assert report["all_federates_completed"] is True
        assert report["dropouts"] == []

    def test_dropout_detected_and_named(self) -> None:
        # HAND: flexcore_research stops at 1800.0 s of a 3600.0 s
        # scenario — DETECTED and NAMED, never treated as "no change".
        report = cc.granted_time_check(
            [
                {"name": "flexcore_asset", "final_granted_time_s": 3600.0,
                 "granted_step_count": 3600},
                {"name": "flexcore_grid", "final_granted_time_s": 3600.0,
                 "granted_step_count": 3600},
                {"name": "flexcore_research", "final_granted_time_s": 1800.0,
                 "granted_step_count": 1800},
            ],
            scenario_end_time_s=3600.0,
        )
        assert report["all_federates_completed"] is False
        assert report["synchronization_consistent"] is False
        assert len(report["dropouts"]) == 1
        assert report["dropouts"][0]["federate"] == "flexcore_research"
        assert report["verdict"].startswith("INTERACTION_CLAIM_REFUSED")

    def test_lagging_federate_flagged_without_scenario_end(self) -> None:
        # Without a configured end, a federate behind the latest one is
        # still flagged (recorded, never treated as "no change").
        report = cc.granted_time_check(
            [
                {"name": "flexcore_asset", "final_granted_time_s": 3600.0,
                 "granted_step_count": 3600},
                {"name": "flexcore_grid", "final_granted_time_s": 1800.0,
                 "granted_step_count": 1800},
            ]
        )
        assert report["synchronization_consistent"] is False
        assert report["dropouts"][0]["federate"] == "flexcore_grid"
        assert report["dropouts"][0]["latest_federate_final_time_s"] == 3600.0

    def test_duplicate_federate_refused(self) -> None:
        # A duplicated record would double-count a federate.
        with pytest.raises(cc.CrossCheckError, match="FEDERATE_NAME_DUPLICATE"):
            cc.granted_time_check(
                [
                    {"name": "flexcore_grid",
                     "final_granted_time_s": 3600.0, "granted_step_count": 3600},
                    {"name": "flexcore_grid",
                     "final_granted_time_s": 3600.0, "granted_step_count": 3600},
                ]
            )

    def test_zero_step_count_refused(self) -> None:
        # A zero-step report is invalid: a real dropout is recorded in the
        # scenario record, never smuggled in as a "valid" report.
        with pytest.raises(cc.CrossCheckError, match="STEP_COUNT_INVALID"):
            cc.granted_time_check(
                [
                    {"name": "flexcore_asset",
                     "final_granted_time_s": 3600.0, "granted_step_count": 3600},
                    {"name": "flexcore_grid",
                     "final_granted_time_s": 0.0, "granted_step_count": 0},
                ],
                scenario_end_time_s=3600.0,
            )


class TestStructuralSeparation:
    def test_well_formed_record_passes(self) -> None:
        # HAND: the endpoint record has two declared-class blocks and a
        # deltas-only comparison block → passes the in-code X-5 walk.
        record = _endpoint_agreement()
        cc.assert_structural_separation(record)  # must not raise
        assert record["series_blocks"][0]["model_class"] == "steady_state"
        assert record["series_blocks"][1]["model_class"] == "dynamic"
        assert "values" not in record["comparison"]

    def test_merged_series_key_refused(self) -> None:
        # X-5 in code: a merged/averaged/blended series is
        # unrepresentable.
        with pytest.raises(cc.CrossCheckError, match="MERGED_SERIES_REFUSED"):
            cc.assert_structural_separation(
                {"pair_id": "CC-4", "merged_series": [1.0, 2.0]}
            )

    def test_values_in_comparison_block_refused(self) -> None:
        # The comparison block carries DELTAS ONLY; a value there would be
        # a joint series across model classes.
        with pytest.raises(
            cc.CrossCheckError, match="COMPARISON_CARRIES_VALUES_REFUSED"
        ):
            cc.assert_structural_separation(
                {"pair_id": "CC-4", "comparison": {"values": [1.0, 2.0]}}
            )

    def test_undeclared_model_class_refused(self) -> None:
        with pytest.raises(
            cc.CrossCheckError, match="CLASS_DECLARATION_REQUIRED"
        ):
            cc.assert_structural_separation(
                {"pair_id": "CC-4",
                 "series_blocks": [{"label": "x", "values": [1.0]}]}
            )


class TestRecordAssembly:
    def test_proposal_only_boundary_enforced(self) -> None:
        # ADR-010 in code: an authorization-shaped key anywhere in the
        # record is refused — the record type cannot express a dispatch
        # decision.
        record = _endpoint_agreement()
        with pytest.raises(cc.CrossCheckError, match="PROPOSAL_ONLY_REFUSED"):
            cc.build_crosscheck_record(
                "CC-4", [record], provenance={"authorization_token": "x"}
            )

    def test_record_carries_boundary_statements_verbatim(self) -> None:
        record = cc.build_crosscheck_record("CC-4", [_endpoint_agreement()])
        assert record["protocol"] == cc.PROTOCOL_ID
        assert record["result_class"] == "agreement_evidence_only"
        assert record["boundary_statements"] == list(cc.BOUNDARY_STATEMENTS)
        assert "AGREEMENT EVIDENCE ON THE OVERLAPPING DOMAIN" in (
            record["boundary_statements"][0]
        )

    def test_empty_checks_refused(self) -> None:
        with pytest.raises(cc.CrossCheckError, match="checks"):
            cc.build_crosscheck_record("CC-4", [])


class TestHonestyPins:
    def test_written_unrun_note_present(self) -> None:
        # The module's own honesty note must say WRITTEN-UNRUN: nothing
        # here is evidence that any co-simulation or power flow ran.
        assert "WRITTEN-UNRUN" in cc.WRITTEN_UNRUN_NOTE

    def test_boundary_statements_carry_the_disciplines(self) -> None:
        joined = "\n".join(cc.BOUNDARY_STATEMENTS)
        assert "AGREEMENT EVIDENCE ON THE OVERLAPPING DOMAIN" in joined
        assert "never averaged, merged, or blended" in joined  # X-5
        assert "NOT transient-stability evidence" in joined    # X-5 (X-1 family)
        assert "TRANSPORT, not physics" in joined              # HELICS discipline
        assert "PRE-REGISTERED before any run" in joined       # X-8

    def test_closed_vocabularies(self) -> None:
        assert cc.MODEL_CLASSES == ("steady_state", "dynamic")
        assert set(cc.ALLOWED_QUANTITIES) == {"voltage_pu", "p_mw", "q_mvar"}
        assert "merged_series" in cc.FORBIDDEN_MERGE_KEYS
        assert "values" in cc.FORBIDDEN_COMPARISON_KEYS
        assert "authorization_token" in cc.FORBIDDEN_RESULT_KEYS
