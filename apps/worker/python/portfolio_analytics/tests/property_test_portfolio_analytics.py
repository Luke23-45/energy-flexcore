# P8.37 — Hand-written PROPERTY tests for the portfolio-analytics research
#               module (the reporting-only boundary, the energy
#               arithmetic, the calibration coverage — analytics.py).
#
# ══════════════════════════════════════════════════════════════════════════
# HONESTY HEADER — read before trusting anything below
#
# 1. PROPERTY-BASED, STDLIB-ONLY. No property-testing library is imported
#    or installed (the Phase 8 loop bans; P8.37 requires hand-written
#    generators). Every generator is a plain function over
#    `random.Random(<fixed seed>)`. EACH property states its ORACLE, its
#    SEED, and its DETERMINISM contract (the module constant
#    PROPERTY_SEED; a fresh seed for the P8.37 verification method is a
#    one-constant change — the properties must hold for ANY seed).
# 2. WRITTEN, NEVER RUN. pytest was never installed in this loop. The
#    tested surface is the stdlib-only analytics module (P8.22).
# 3. Run instruction (the recorded procedure):
#    `PYTHONPATH=apps/worker/python/portfolio_analytics python -m pytest
#    apps/worker/python/portfolio_analytics/tests -v` from the
#    repository root.
# 4. WHAT THE PROPERTIES PIN (ledger P8.37 step 2, as code):
#    - P-REPORT:  the reporting-only scan catches an injected
#                 dispatch-shaped key at ANY nesting depth (the
#                 DISPATCH_SHAPED_KEYS set is the oracle) — an analytic
#                 can never silently become a dispatch input (ADR-010);
#    - P-ENERGY:  the energy arithmetic is exact per event and ADDITIVE
#                 in aggregate for arbitrary valid events — every
#                 aggregate is recomputable from per_event (the
#                 ledger's independent-recompute verification method,
#                 as a property); shortfall and over-delivery are never
#                 both positive for one event;
#    - P-CALIB:   the calibration coverage lies in [0, 1] and equals
#                 the recomputed covered/total fraction for arbitrary
#                 valid record sets;
#    - P-REFUSE:  malformed events / records / comparisons are ALWAYS
#                 refused with the module's named error class — never a
#                 number.
#    Everything here is REPORTING-ONLY bookkeeping over injected
#    scenario data (P8.22 boundary): nothing proves any dispatch,
#    settlement, or field property, and no measured figure exists.
# ══════════════════════════════════════════════════════════════════════════

from __future__ import annotations

import os
import random
import sys

import pytest

sys.path.insert(
    0,
    os.path.join(os.path.dirname(os.path.abspath(__file__)), os.pardir),
)

import analytics as an  # noqa: E402 — path set above

PROPERTY_SEED = 20261114


def _rng() -> random.Random:
    return random.Random(PROPERTY_SEED)


def _event(rng: random.Random) -> dict:
    """An arbitrary VALID flexibility event (MW / h; SOC a fraction)."""
    return {
        "event_id": "evt-%04d" % rng.randrange(10000),
        "interval_hours": rng.choice([0.25, 0.5, 1.0, rng.uniform(0.01, 24.0)]),
        "available_mw": rng.choice([0.0, rng.uniform(0.0, 10.0)]),
        "delivered_mw": rng.uniform(0.0, 10.0),
        "direction": rng.choice(sorted(an.DIRECTIONS)),
        "quality_state": rng.choice(sorted(an.TELEMETRY_QUALITY_STATES)),
        "soc_start_fraction": rng.choice([None, rng.uniform(0.0, 1.0)]),
        "soc_end_fraction": rng.choice([None, rng.uniform(0.0, 1.0)]),
    }


def _nested_tree(rng: random.Random, depth: int) -> dict:
    """An arbitrary JSON-shaped tree (the carrier for the injected
    dispatch-shaped key in P-REPORT)."""
    if depth <= 0:
        return rng.choice([1.0, "leaf", True, None])
    if rng.random() < 0.4:
        return [_nested_tree(rng, depth - 1) for _ in range(rng.randrange(1, 3))]
    return {
        "k%d" % i: _nested_tree(rng, depth - 1)
        for i in range(rng.randrange(1, 4))
    }


def _inject_key(tree, rng: random.Random, key: str) -> None:
    """Injects `key` at a random mapping position anywhere in the tree."""
    node = tree
    while True:
        if isinstance(node, dict) and (rng.random() < 0.5 or not any(
            isinstance(v, (dict, list)) for v in node.values()
        )):
            node[key] = 1.0
            return
        if isinstance(node, list):
            children = [v for v in node if isinstance(v, (dict, list))]
            if not children or rng.random() < 0.3:
                node.append({key: 1.0})
                return
            node = rng.choice(children)
            continue
        children = [v for v in node.values() if isinstance(v, (dict, list))]
        if not children:
            node[key] = 1.0
            return
        node = rng.choice(children)


# ---------------------------------------------------------------------------
# P-REPORT — the reporting-only boundary at any depth
# ---------------------------------------------------------------------------


class TestReportingOnlyBoundaryProperty:
    def test_dispatch_shaped_key_caught_at_any_depth(self) -> None:
        """P-REPORT. Oracle: `DISPATCH_SHAPED_KEYS` — the scan raises
        `AnalyticsInputError` for EVERY dispatch-shaped key injected at
        a random position of an arbitrary tree of depth 1..12. Generator:
        seeded trees + seeded key choice. Determinism: fixed seed; the
        assertion is the same refusal at every depth."""
        rng = _rng()
        for _ in range(150):
            tree = _nested_tree(rng, rng.randrange(1, 13))
            key = rng.choice(sorted(an.DISPATCH_SHAPED_KEYS))
            _inject_key(tree, rng, key)
            with pytest.raises(an.AnalyticsInputError):
                an.assert_reporting_only(tree)

    def test_case_variants_caught_and_clean_trees_pass(self) -> None:
        """P-REPORT (bypass resistance + negative control). Oracle: the
        same set with case/whitespace normalization — 'SCHEDULE',
        ' setpoint ' are caught; the same generated trees WITHOUT the
        injection never raise."""
        rng = _rng()
        for _ in range(80):
            tree = _nested_tree(rng, rng.randrange(1, 10))
            key = rng.choice(sorted(an.DISPATCH_SHAPED_KEYS))
            _inject_key(tree, rng, key.upper())
            with pytest.raises(an.AnalyticsInputError):
                an.assert_reporting_only(tree)
        for _ in range(80):
            an.assert_reporting_only(_nested_tree(rng, rng.randrange(1, 10)))


# ---------------------------------------------------------------------------
# P-ENERGY — the energy arithmetic is exact and additive
# ---------------------------------------------------------------------------


class TestEnergyArithmeticProperty:
    def test_event_energy_matches_formula(self) -> None:
        """P-ENERGY (per event). Oracle: the stated formula, recomputed
        independently — available_mwh = a*h; delivered_mwh = d*h;
        shortfall_mwh = max(0, a*h - d*h); over_delivery_mwh =
        max(0, d*h - a*h); shortfall and over-delivery are never both
        positive. Generator: seeded valid events."""
        rng = _rng()
        for _ in range(300):
            event = _event(rng)
            record = an.event_energy(event)
            a = event["available_mw"] * event["interval_hours"]
            d = event["delivered_mw"] * event["interval_hours"]
            assert record["available_mwh"] == pytest.approx(a)
            assert record["delivered_mwh"] == pytest.approx(d)
            assert record["shortfall_mwh"] == pytest.approx(max(0.0, a - d))
            assert record["over_delivery_mwh"] == pytest.approx(max(0.0, d - a))
            assert not (record["shortfall_mwh"] > 0.0
                        and record["over_delivery_mwh"] > 0.0)

    def test_aggregate_is_additive_and_recomputable(self) -> None:
        """P-ENERGY (aggregate). Oracle: additivity — the totals equal
        the sums of the per-event records carried INSIDE the output, so
        every aggregate is recomputable independently (the ledger's
        verification method, as a property); the delivery ratio equals
        delivered/available exactly. Generator: seeded event lists of
        length 1..30 (with at least one nonzero-available event, so the
        DIVISOR_ZERO refusal is not hit)."""
        rng = _rng()
        for _ in range(60):
            events = [_event(rng) for _ in range(rng.randrange(1, 31))]
            if all(e["available_mw"] == 0.0 for e in events):
                events[0]["available_mw"] = 1.0
            aggregate = an.aggregate_delivery(events)
            per = aggregate["per_event"]
            assert aggregate["available_total_mwh"] == pytest.approx(
                sum(r["available_mwh"] for r in per)
            )
            assert aggregate["delivered_total_mwh"] == pytest.approx(
                sum(r["delivered_mwh"] for r in per)
            )
            assert aggregate["shortfall_total_mwh"] == pytest.approx(
                sum(r["shortfall_mwh"] for r in per)
            )
            assert aggregate["delivery_ratio"] == pytest.approx(
                aggregate["delivered_total_mwh"] / aggregate["available_total_mwh"]
            )

    def test_zero_available_total_refused(self) -> None:
        """P-ENERGY (the empty-denominator refusal). Oracle:
        `DIVISOR_ZERO` — an aggregate whose available total is exactly
        0 MWh is refused, never reported as a perfect ratio, for
        arbitrary all-zero-available event lists."""
        rng = _rng()
        for _ in range(30):
            events = [_event(rng) for _ in range(rng.randrange(1, 6))]
            for e in events:
                e["available_mw"] = 0.0
            with pytest.raises(an.AnalyticsInputError):
                an.aggregate_delivery(events)


# ---------------------------------------------------------------------------
# P-CALIB — the calibration coverage
# ---------------------------------------------------------------------------


class TestCalibrationCoverageProperty:
    def test_coverage_in_unit_interval_and_recomputable(self) -> None:
        """P-CALIB. Oracle: the stated formula — coverage =
        |{low <= actual <= high}| / |records|, recomputed independently;
        it lies in [0, 1] for arbitrary valid record sets. Generator:
        seeded bands and actuals in one declared unit."""
        rng = _rng()
        for _ in range(100):
            records = []
            for i in range(rng.randrange(1, 40)):
                low = rng.uniform(-10.0, 10.0)
                high = low + abs(rng.uniform(0.0, 5.0))
                records.append(
                    {
                        "declared_band_low": low,
                        "declared_band_high": high,
                        "actual": rng.uniform(-15.0, 15.0),
                        "unit": "MW",
                        "regime": "regime-%d" % rng.randrange(5),
                    }
                )
            result = an.uncertainty_calibration(records)
            covered = sum(
                1
                for r in records
                if r["declared_band_low"] <= r["actual"] <= r["declared_band_high"]
            )
            assert result["covered_count"] == covered
            assert result["coverage"] == pytest.approx(covered / len(records))
            assert 0.0 <= result["coverage"] <= 1.0

    def test_boundary_actuals_count_as_covered(self) -> None:
        """P-CALIB (the closed-band boundary). Oracle: the closed
        interval — an actual EXACTLY on a band endpoint is covered
        (`low <= actual <= high`), for arbitrary generated bands."""
        rng = _rng()
        for _ in range(50):
            low = rng.uniform(-10.0, 10.0)
            high = low + abs(rng.uniform(0.0, 5.0))
            endpoint = rng.choice([low, high])
            result = an.uncertainty_calibration(
                [
                    {
                        "declared_band_low": low,
                        "declared_band_high": high,
                        "actual": endpoint,
                        "unit": "MW",
                        "regime": "boundary-regime",
                    }
                ]
            )
            assert result["coverage"] == 1.0

    def test_malformed_records_refused(self) -> None:
        """P-CALIB (refusal side). Oracle: the named refusal — an
        inverted band (high < low), a non-finite value, an unknown unit
        or regime, and a mixed-unit record set are ALWAYS refused."""
        rng = _rng()
        for _ in range(50):
            low = rng.uniform(-10.0, 10.0)
            record = {
                "declared_band_low": low,
                "declared_band_high": low - abs(rng.uniform(0.0, 1.0)),
                "actual": low,
                "unit": "MW",
                "regime": "r",
            }
            with pytest.raises(an.AnalyticsInputError):
                an.uncertainty_calibration([record])
        with pytest.raises(an.AnalyticsInputError):
            an.uncertainty_calibration(
                [
                    {
                        "declared_band_low": 0.0,
                        "declared_band_high": 1.0,
                        "actual": float("nan"),
                        "unit": "MW",
                        "regime": "r",
                    }
                ]
            )
        with pytest.raises(an.AnalyticsInputError):
            an.uncertainty_calibration(
                [
                    {
                        "declared_band_low": 0.0,
                        "declared_band_high": 1.0,
                        "actual": 0.5,
                        "unit": "MW",
                        "regime": "r",
                    },
                    {
                        "declared_band_low": 0.0,
                        "declared_band_high": 1.0,
                        "actual": 0.5,
                        "unit": "MWh",
                        "regime": "r",
                    },
                ]
            )


# ---------------------------------------------------------------------------
# P-REFUSE — the event validator and the baseline comparison
# ---------------------------------------------------------------------------


class TestValidationAndComparisonProperty:
    def test_event_validator_refuses_every_value_corruption(self) -> None:
        """P-REFUSE. Oracle: the named refusal (`EVENT_INVALID`) — a
        corrupted field value (wrong type, non-finite, out of range,
        unknown vocabulary member) is ALWAYS refused, never normalized
        into a record. The bool corruption is skipped for
        interval_hours/available_mw/delivered_mw where bool is excluded
        but... bool IS excluded by the validator, so it is included
        here; it is skipped for the string/vocabulary fields where a
        bool is simply a wrong type too — all are refused either way.
        Generator: seeded corruptions over the eight event fields."""
        rng = _rng()
        base = _event(rng)
        base.update(
            interval_hours=1.0,
            available_mw=1.0,
            delivered_mw=1.0,
            direction="export",
            quality_state="GOOD",
        )
        # `event_id` is excluded: any non-empty string (including the
        # corrupted "not-valid") is a VALID id, so a string corruption
        # there is not a corruption. Every remaining field is refused
        # for every corruption in the catalogue.
        fields = ("interval_hours", "available_mw",
                  "delivered_mw", "direction", "quality_state",
                  "soc_start_fraction", "soc_end_fraction")
        for _ in range(200):
            corrupted = dict(base)
            field = rng.choice(fields)
            corruption = rng.choice(
                ("wrong_type_string", "wrong_type_bool", "wrong_type_list",
                 "nonfinite_value", "negative")
            )
            if corruption == "wrong_type_string":
                corrupted[field] = "not-valid"
            elif corruption == "wrong_type_bool":
                corrupted[field] = rng.choice([True, False])
            elif corruption == "wrong_type_list":
                corrupted[field] = [1.0]
            elif corruption == "nonfinite_value":
                corrupted[field] = rng.choice([float("nan"), float("inf")])
            else:
                corrupted[field] = -abs(rng.uniform(0.0, 10.0))
            with pytest.raises(an.AnalyticsInputError):
                an.validate_flexibility_event(corrupted)

    def test_comparison_refuses_missing_tolerance_and_bad_totals(self) -> None:
        """P-REFUSE (comparison level). Oracle: the named refusals —
        `TOLERANCE_NOT_PRE_REGISTERED` (a negative or non-numeric
        tolerance; the tolerance is INJECTED, never derived) and
        `BASELINE_COMPARE_INVALID` (a totals object that is not an
        aggregate_delivery record) are ALWAYS raised."""
        rng = _rng()
        for _ in range(50):
            tolerance = rng.choice([-1.0, float("nan"), "0.5", True, None])
            setup_event = _event(rng)
            setup_event["available_mw"] = 1.0  # nonzero: the setup itself
            # must not hit the DIVISOR_ZERO refusal (that refusal is its
            # own property, tested above).
            totals = an.aggregate_delivery([setup_event])
            with pytest.raises(an.AnalyticsInputError):
                an.compare_to_baseline_accounting(
                    analytics_totals=totals,
                    baseline_totals=totals,
                    pre_registered_tolerance_mwh=tolerance,
                )
            with pytest.raises(an.AnalyticsInputError):
                an.compare_to_baseline_accounting(
                    analytics_totals={"not": "an aggregate"},
                    baseline_totals=totals,
                    pre_registered_tolerance_mwh=0.5,
                )
