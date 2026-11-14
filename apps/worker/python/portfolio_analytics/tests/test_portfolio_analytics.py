# P8.22 — The portfolio-analytics test suite: pure-logic parts that RUN
#         WITHOUT any framework installed.
#
# ══════════════════════════════════════════════════════════════════════════
# HONESTY HEADER
#
# 1. WRITTEN, NEVER RUN. pytest was never installed in this loop; no test
#    below has ever executed. Expected values are HAND-COMPUTED
#    derivations with the arithmetic stated in each test (never observed
#    outputs). Run instruction:
#    `PYTHONPATH=apps/worker/python/portfolio_analytics python -m pytest
#    apps/worker/python/portfolio_analytics/tests -v`; the in-module
#    self-tests run WITHOUT pytest:
#    `python apps/worker/python/portfolio_analytics/analytics.py`.
# 2. What this suite pins (ledger P8.22 steps 1–4): the flexibility
#    analytics (2.0 MW x 0.5 h = 1.0 MWh; shortfall = max(0, available -
#    delivered) MWh with over-delivery SEPARABLE, never netted), the
#    portfolio aggregate (available 5.0, delivered 4.75, shortfall 0.25,
#    ratio 4.75/5.0 = 0.95), the uncertainty-band calibration coverage
#    (2/3 inclusive of the band boundary), the baseline-accounting
#    comparison (differences exactly 0.0 within the pre-registered
#    tolerance; a beyond-tolerance difference REPORTED as a discrepancy,
#    never absorbed), the DIVISOR_ZERO refusal (no ratio over an empty
#    denominator), the canonical-units discipline (SOC a fraction in
#    [0, 1]; direction declared, never encoded in a magnitude; telemetry
#    quality states carried), and the REPORTING-ONLY boundary
#    (dispatch-shaped keys refused at any depth; no dispatch/rank/score-
#    shaped callable exists in the module namespace — the ledger's named
#    risk "an analytic that silently becomes a dispatch input" is
#    structurally absent).
# 3. NOTHING here proves any performance or correctness property of a
#    real portfolio: the suite pins analytic DEFINITIONS over constructed
#    inputs. Every measured cell in the P8.22 record
#    (`ledger/phase-08-advanced-coordination/22_portfolio_analytics.md`)
#    is EMPTY until the human/lab gate fills it.
# ══════════════════════════════════════════════════════════════════════════

from __future__ import annotations

import os
import sys

import pytest

sys.path.insert(
    0,
    os.path.join(os.path.dirname(os.path.abspath(__file__)), os.pardir),
)

from analytics import (  # noqa: E402 — path set above
    CALIBRATION_SCOPE_STATEMENT,
    PORTFOLIO_ANALYTICS_SCHEMA,
    AnalyticsInputError,
    aggregate_delivery,
    assert_reporting_only,
    compare_to_baseline_accounting,
    event_energy,
    uncertainty_calibration,
    validate_flexibility_event,
)
import analytics as analytics_module  # noqa: E402 — path set above


def _event(
    event_id: str,
    available_mw: float,
    delivered_mw: float,
    interval_hours: float,
    **overrides,
) -> dict:
    base = dict(
        event_id=event_id,
        interval_hours=interval_hours,
        available_mw=available_mw,
        delivered_mw=delivered_mw,
        direction="export",
        quality_state="GOOD",
    )
    base.update(overrides)
    return base


class TestEventEnergy:
    def test_hand_computed_single_event(self) -> None:
        # HAND CHECK: 2.0 MW x 0.5 h = 1.0 MWh; 1.5 MW x 0.5 h = 0.75 MWh;
        # shortfall 1.0 - 0.75 = 0.25 MWh.
        record = event_energy(_event("ev-1", 2.0, 1.5, 0.5))
        assert record["schema"] == PORTFOLIO_ANALYTICS_SCHEMA
        assert record["available_mwh"] == pytest.approx(1.0)
        assert record["delivered_mwh"] == pytest.approx(0.75)
        assert record["shortfall_mwh"] == pytest.approx(0.25)
        assert record["over_delivery_mwh"] == pytest.approx(0.0)
        assert record["quality_state"] == "GOOD"
        assert record["direction"] == "export"
        assert record["usage"].startswith("derived reporting only")

    def test_over_delivery_separable(self) -> None:
        # HAND CHECK: 2.0 vs 2.5 MW over 1.0 h -> gap -0.5 MWh -> shortfall
        # 0.0, over-delivery 0.5 (never netted into the shortfall).
        record = event_energy(_event("ev-2", 2.0, 2.5, 1.0))
        assert record["shortfall_mwh"] == pytest.approx(0.0)
        assert record["over_delivery_mwh"] == pytest.approx(0.5)

    def test_quality_state_carried(self) -> None:
        record = event_energy(
            _event("ev-3", 1.0, 1.0, 1.0, quality_state="STALE")
        )
        assert record["quality_state"] == "STALE"

    def test_negative_magnitude_refused(self) -> None:
        with pytest.raises(AnalyticsInputError, match="EVENT_INVALID"):
            event_energy(_event("ev-4", -2.0, 1.0, 1.0))

    def test_nonpositive_interval_refused(self) -> None:
        with pytest.raises(AnalyticsInputError, match="EVENT_INVALID"):
            event_energy(_event("ev-5", 1.0, 1.0, 0.0))

    def test_quality_state_closed_vocabulary(self) -> None:
        with pytest.raises(AnalyticsInputError, match="EVENT_INVALID"):
            event_energy(_event("ev-6", 1.0, 1.0, 1.0, quality_state="fine"))

    def test_direction_must_be_declared(self) -> None:
        with pytest.raises(AnalyticsInputError, match="EVENT_INVALID"):
            event_energy(_event("ev-7", 1.0, 1.0, 1.0, direction="up"))

    def test_soc_fraction_in_unit_interval(self) -> None:
        normalized = validate_flexibility_event(
            _event("ev-8", 1.0, 1.0, 1.0, soc_start_fraction=0.25,
                   soc_end_fraction=0.75)
        )
        assert normalized["soc_start_fraction"] == pytest.approx(0.25)
        assert normalized["soc_end_fraction"] == pytest.approx(0.75)
        with pytest.raises(AnalyticsInputError, match="EVENT_INVALID"):
            validate_flexibility_event(
                _event("ev-9", 1.0, 1.0, 1.0, soc_start_fraction=1.5)
            )

    def test_empty_event_id_refused(self) -> None:
        with pytest.raises(AnalyticsInputError, match="EVENT_INVALID"):
            validate_flexibility_event(_event("  ", 1.0, 1.0, 1.0))


class TestAggregateDelivery:
    def test_hand_computed_aggregate(self) -> None:
        # HAND CHECK: ev-1 (1.0 / 0.75 MWh) + ev-3 (4.0 / 4.0 MWh) ->
        # available 5.0, delivered 4.75, shortfall 0.25; ratio
        # 4.75 / 5.0 = 0.95.
        aggregate = aggregate_delivery(
            [_event("ev-1", 2.0, 1.5, 0.5), _event("ev-3", 4.0, 4.0, 1.0)]
        )
        assert aggregate["event_count"] == 2
        assert aggregate["available_total_mwh"] == pytest.approx(5.0)
        assert aggregate["delivered_total_mwh"] == pytest.approx(4.75)
        assert aggregate["shortfall_total_mwh"] == pytest.approx(0.25)
        assert aggregate["delivery_ratio"] == pytest.approx(0.95)
        assert aggregate["quality_state_mix"] == {"GOOD": 2}

    def test_per_event_records_travel_for_recompute(self) -> None:
        events = [_event("ev-1", 2.0, 1.5, 0.5), _event("ev-3", 4.0, 4.0, 1.0)]
        aggregate = aggregate_delivery(events)
        assert [r["event_id"] for r in aggregate["per_event"]] == ["ev-1", "ev-3"]
        # The independent-recompute path: totals equal the sum over the
        # carried per-event records.
        assert aggregate["available_total_mwh"] == pytest.approx(
            sum(r["available_mwh"] for r in aggregate["per_event"])
        )
        assert aggregate["delivered_total_mwh"] == pytest.approx(
            sum(r["delivered_mwh"] for r in aggregate["per_event"])
        )
        assert aggregate["shortfall_total_mwh"] == pytest.approx(
            sum(r["shortfall_mwh"] for r in aggregate["per_event"])
        )

    def test_zero_available_total_is_divisor_zero(self) -> None:
        with pytest.raises(AnalyticsInputError, match="DIVISOR_ZERO"):
            aggregate_delivery([_event("ev-4", 0.0, 0.0, 1.0)])

    def test_empty_event_list_refused(self) -> None:
        with pytest.raises(AnalyticsInputError, match="EVENT_INVALID"):
            aggregate_delivery([])


class TestUncertaintyCalibration:
    def _records(self) -> list:
        # HAND CHECK: band [1.0, 2.0] MW; actuals 1.5 (in), 0.9 (out),
        # 2.0 (in — boundary inclusive) -> coverage 2/3.
        return [
            {
                "declared_band_low": 1.0,
                "declared_band_high": 2.0,
                "actual": 1.5,
                "unit": "MW",
                "regime": "RECORD-AT-FIRST-RUN",
            },
            {
                "declared_band_low": 1.0,
                "declared_band_high": 2.0,
                "actual": 0.9,
                "unit": "MW",
                "regime": "RECORD-AT-FIRST-RUN",
            },
            {
                "declared_band_low": 1.0,
                "declared_band_high": 2.0,
                "actual": 2.0,
                "unit": "MW",
                "regime": "RECORD-AT-FIRST-RUN",
            },
        ]

    def test_hand_computed_coverage(self) -> None:
        calibration = uncertainty_calibration(self._records())
        assert calibration["covered_count"] == 2
        assert calibration["record_count"] == 3
        assert calibration["coverage"] == pytest.approx(2 / 3)
        assert calibration["unit"] == "MW"
        assert calibration["calibration_scope"] == CALIBRATION_SCOPE_STATEMENT
        assert calibration["regimes_assessed"] == ["RECORD-AT-FIRST-RUN"]

    def test_inverted_band_refused(self) -> None:
        records = self._records()
        records[0]["declared_band_low"] = 2.0
        records[0]["declared_band_high"] = 1.0
        with pytest.raises(AnalyticsInputError, match="CALIBRATION_INVALID"):
            uncertainty_calibration(records)

    def test_mixed_units_refused(self) -> None:
        records = self._records()
        records[0]["unit"] = "MWh"
        with pytest.raises(AnalyticsInputError, match="CALIBRATION_INVALID"):
            uncertainty_calibration(records)

    def test_missing_regime_refused(self) -> None:
        records = self._records()
        del records[0]["regime"]
        with pytest.raises(AnalyticsInputError, match="CALIBRATION_INVALID"):
            uncertainty_calibration(records)

    def test_empty_records_refused(self) -> None:
        with pytest.raises(AnalyticsInputError, match="CALIBRATION_INVALID"):
            uncertainty_calibration([])


class TestBaselineAccountingComparison:
    def _aggregate(self) -> dict:
        # HAND CHECK (TestAggregateDelivery): available 5.0, delivered
        # 4.75, shortfall 0.25 MWh.
        return aggregate_delivery(
            [_event("ev-1", 2.0, 1.5, 0.5), _event("ev-3", 4.0, 4.0, 1.0)]
        )

    def _baseline(self, delivered: float, shortfall: float) -> dict:
        return {
            "schema": PORTFOLIO_ANALYTICS_SCHEMA,
            "available_total_mwh": 5.0,
            "delivered_total_mwh": delivered,
            "shortfall_total_mwh": shortfall,
        }

    def test_zero_discrepancy_within_tolerance(self) -> None:
        comparison = compare_to_baseline_accounting(
            analytics_totals=self._aggregate(),
            baseline_totals=self._baseline(4.75, 0.25),
            pre_registered_tolerance_mwh=1e-9,
        )
        assert comparison["all_within_tolerance"] is True
        assert comparison["discrepancy_fields"] == []
        assert comparison["comparisons"]["delivered_total_mwh"][
            "absolute_difference_mwh"
        ] == pytest.approx(0.0)

    def test_discrepancy_reported_not_absorbed(self) -> None:
        # HAND CHECK: |4.75 - 4.55| = 0.2 MWh > 0.1 MWh tolerance ->
        # reported as a discrepancy (never absorbed).
        comparison = compare_to_baseline_accounting(
            analytics_totals=self._aggregate(),
            baseline_totals=self._baseline(4.55, 0.45),
            pre_registered_tolerance_mwh=0.1,
        )
        assert comparison["all_within_tolerance"] is False
        assert sorted(comparison["discrepancy_fields"]) == [
            "delivered_total_mwh",
            "shortfall_total_mwh",
        ]
        assert comparison["comparisons"]["delivered_total_mwh"][
            "absolute_difference_mwh"
        ] == pytest.approx(0.2)

    def test_boundary_difference_is_within_tolerance(self) -> None:
        # HAND CHECK: |4.75 - 4.65| = 0.1 == tolerance 0.1 -> within
        # (the boundary IS within, per <=).
        comparison = compare_to_baseline_accounting(
            analytics_totals=self._aggregate(),
            baseline_totals=self._baseline(4.65, 0.35),
            pre_registered_tolerance_mwh=0.1,
        )
        assert comparison["all_within_tolerance"] is True

    def test_missing_tolerance_refused(self) -> None:
        with pytest.raises(
            AnalyticsInputError, match="TOLERANCE_NOT_PRE_REGISTERED"
        ):
            compare_to_baseline_accounting(
                analytics_totals=self._aggregate(),
                baseline_totals=self._baseline(4.75, 0.25),
                pre_registered_tolerance_mwh=None,
            )

    def test_negative_tolerance_refused(self) -> None:
        with pytest.raises(
            AnalyticsInputError, match="TOLERANCE_NOT_PRE_REGISTERED"
        ):
            compare_to_baseline_accounting(
                analytics_totals=self._aggregate(),
                baseline_totals=self._baseline(4.75, 0.25),
                pre_registered_tolerance_mwh=-0.1,
            )

    def test_non_aggregate_input_refused(self) -> None:
        with pytest.raises(
            AnalyticsInputError, match="BASELINE_COMPARE_INVALID"
        ):
            compare_to_baseline_accounting(
                analytics_totals={"delivered_total_mwh": 4.75},
                baseline_totals=self._baseline(4.75, 0.25),
                pre_registered_tolerance_mwh=0.1,
            )


class TestReportingOnlyBoundary:
    def test_dispatch_shaped_key_refused(self) -> None:
        with pytest.raises(
            AnalyticsInputError, match="REPORTING_ONLY_VIOLATION"
        ):
            assert_reporting_only(
                {"delivery_ratio": 0.95, "dispatch_rank": [1, 2, 3]}
            )

    def test_nested_dispatch_shaped_key_refused(self) -> None:
        with pytest.raises(
            AnalyticsInputError, match="REPORTING_ONLY_VIOLATION"
        ):
            assert_reporting_only({"a": [{"b": {"flexibility_score": 0.9}}]})

    def test_list_item_dispatch_shaped_key_refused(self) -> None:
        with pytest.raises(
            AnalyticsInputError, match="REPORTING_ONLY_VIOLATION"
        ):
            assert_reporting_only([{"setpoint_mw": 3.5}])

    def test_real_outputs_scan_clean(self) -> None:
        assert_reporting_only(self._aggregate_like_record())

    def _aggregate_like_record(self) -> dict:
        aggregate = aggregate_delivery(
            [_event("ev-1", 2.0, 1.5, 0.5), _event("ev-3", 4.0, 4.0, 1.0)]
        )
        comparison = compare_to_baseline_accounting(
            analytics_totals=aggregate,
            baseline_totals={
                "schema": PORTFOLIO_ANALYTICS_SCHEMA,
                "available_total_mwh": 5.0,
                "delivered_total_mwh": 4.75,
                "shortfall_total_mwh": 0.25,
            },
            pre_registered_tolerance_mwh=1e-9,
        )
        return {"aggregate": aggregate, "comparison": comparison}

    def test_no_dispatch_shaped_callable_in_module(self) -> None:
        forbidden = (
            "dispatch",
            "issue",
            "authorize",
            "approve",
            "command",
            "rank",
            "score",
            "schedule",
        )
        offenders = [
            name
            for name, obj in vars(analytics_module).items()
            if callable(obj)
            and not name.startswith("_")
            and any(token in name.lower() for token in forbidden)
        ]
        assert offenders == []
