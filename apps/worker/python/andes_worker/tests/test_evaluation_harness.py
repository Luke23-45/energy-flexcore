# P8.17 — The evaluation-harness test suite: the hand-computed metric
#         cases as pytest tests (WRITTEN, NEVER RUN — see header).
#
# ══════════════════════════════════════════════════════════════════════════
# HONESTY HEADER
#
# 1. WRITTEN, NEVER RUN. pytest was never installed in this loop; no test
#    below has ever executed. The expected values are HAND COMPUTATIONS
#    from the constructed inputs (the arithmetic is stated in each test's
#    comment and in the harness's in-module `run_self_tests()`), never
#    observed outputs of the code under test.
# 2. Run instruction (the recorded procedure): a scratch venv, pytest
#    installed per the recorded UNPINNED-REVERIFY discipline, then
#    `PYTHONPATH=apps/worker/python/andes_worker python -m pytest
#    apps/worker/python/andes_worker/tests -v` from the repository root.
#    The modules are imported in place (no build step). The sys.path
#    insertion below makes the suite robust to a direct
#    `pytest apps/worker/python/andes_worker/tests` invocation as well.
# 3. These tests pin the P8.17 metric DEFINITIONS (the recorded formulas),
#    not simulator behaviour: no ANDES, no I/O, no network — the harness
#    is pure by construction and the suite must stay able to run without
#    ANDES installed.
# ══════════════════════════════════════════════════════════════════════════

from __future__ import annotations

import math
import os
import sys

import pytest

sys.path.insert(
    0,
    os.path.join(os.path.dirname(os.path.abspath(__file__)), os.pardir),
)

import evaluation_harness as eh  # noqa: E402 — path set above


# Shared constructed series (identical to the in-module self-tests so the
# two suites pin the same definitions).

NADIR_TIMES = [0.0, 0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5, 4.0, 4.5, 5.0]
NADIR_VALUES = [48.9, 50.0, 50.0, 50.0, 50.0, 49.4, 49.2, 49.35, 49.6, 49.9, 50.0]

VOLT_TIMES = [0.0, 0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5, 4.0, 4.5, 5.0]
VOLT_VALUES = [1.2, 1.0, 1.0, 1.0, 1.0, 1.05, 1.06, 1.02, 1.0, 1.0, 1.0]


def _decay_series(re_excite: bool):
    ts = [round(0.25 * k, 10) for k in range(0, 25)]
    vs = []
    for t in ts:
        if t < 1.0:
            vs.append(0.0)
        elif t <= 5.0:
            vs.append(1.0 - 0.25 * (t - 1.0))
        else:
            vs.append(0.0)
    if re_excite:
        for i, t in enumerate(ts):
            if t == 5.5:
                vs[i] = 0.5
    return ts, vs


class TestExtremes:
    def test_frequency_nadir_excludes_pre_event(self) -> None:
        # HAND: post-event minimum 49.2 Hz at t=3.0; depth 50.0-49.2 = 0.8 Hz;
        # the pre-event 48.9 Hz dip must not win.
        nadir = eh.frequency_nadir(NADIR_TIMES, NADIR_VALUES, 50.0, 2.0)
        assert nadir["nadir_hz"] == pytest.approx(49.2)
        assert nadir["nadir_depth_hz"] == pytest.approx(0.8)
        assert nadir["nadir_time_s"] == 3.0

    def test_voltage_zenith_excludes_pre_event(self) -> None:
        # HAND: post-event maximum 1.06 pu at t=3.0; height 1.06-1.00 = 0.06 pu.
        volt = eh.voltage_extremes(VOLT_TIMES, VOLT_VALUES, 1.0, 2.0)
        assert volt["v_max_pu"] == pytest.approx(1.06)
        assert volt["v_max_height_pu"] == pytest.approx(0.06)
        assert volt["v_min_pu"] == pytest.approx(1.0)

    def test_event_boundary_sample_included(self) -> None:
        # HAND: the sample exactly at event_time (t=2.0, v=50.0) is
        # post-event (>=); the pre-event sample t=1.5 is not.
        result = eh.excursion_extremes([1.5, 2.0, 2.5], [9.0, 5.0, 7.0], 0.0, 2.0)
        assert result["nadir_value"] == 5.0
        assert result["zenith_value"] == 7.0
        assert result["post_event_samples"] == 2


class TestRocof:
    def test_linear_ramp_exact(self) -> None:
        # HAND: v = 50 + 0.5t, dt = 0.25 → every interior central
        # difference is (0.5(t+h) - 0.5(t-h))/(2h) = 0.5 exactly.
        t = [0.0, 0.25, 0.5, 0.75, 1.0, 1.25, 1.5]
        v = [50.0 + 0.5 * x for x in t]
        rocof = eh.max_rocof(t, v)
        assert rocof["max_abs_rocof"] == pytest.approx(0.5)
        assert rocof["interior_samples"] == 5

    def test_two_segments_breakpoint_artifact(self) -> None:
        # HAND: slope 0.5 up to t=1.0, slope 2.0 after. Interior central
        # differences: t=1.25 → (51.5-50.375)/0.75 = 1.5 (the averaging
        # artifact); t=1.5 → (52.0-51.0)/0.5 = 2.0; t=1.75 → 2.0;
        # t=1.0 → (51.0-50.375)/0.5 = 1.25. Max = 2.0, first attained at
        # t=1.5.
        t = [0.0, 0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0]
        v = [50.0 + 0.5 * x if x <= 1.0 else 50.5 + 2.0 * (x - 1.0) for x in t]
        rocof = eh.max_rocof(t, v)
        assert rocof["max_abs_rocof"] == pytest.approx(2.0)
        assert rocof["rocof_time"] == 1.5

    def test_first_and_last_samples_excluded(self) -> None:
        # HAND: two samples only → zero interior samples → the metric has
        # nothing to estimate over and must fail closed, not invent one.
        with pytest.raises(eh.MetricInputError):
            eh.max_rocof([0.0, 1.0], [1.0, 2.0])


class TestDamping:
    def test_analytic_decaying_sinusoid(self) -> None:
        # x(t) = 100·e^(-t)·cos(2πt), t = 0.0..4.0 step 0.1. True peaks at
        # integer t with envelope 100·e^(-k); the continuous maxima near
        # t≈0.975/1.975 fall BETWEEN samples, so the detected peak SAMPLES
        # are t = 1, 2, 3 with values 100·e^(-1) = 36.78794412 and
        # 100·e^(-2) = 13.53352832.
        # HAND: delta = ln(e^1) = 1.0; zeta = 1/sqrt(4π²+1)
        #       = 1/sqrt(40.478417604357) = 1/6.362262 = 0.157177 (6 dp).
        t = [round(0.1 * k, 10) for k in range(0, 41)]
        v = [100.0 * math.exp(-x) * math.cos(2.0 * math.pi * x) for x in t]
        damp = eh.damping_ratio_from_window(t, v, baseline=0.0)
        assert damp["log_decrement"] == pytest.approx(1.0, abs=1e-9)
        assert damp["damping_ratio"] == pytest.approx(
            1.0 / math.sqrt(4.0 * math.pi * math.pi + 1.0)
        )
        assert 0.1571 <= damp["damping_ratio"] <= 0.1573
        assert damp["peak1_time"] == 1.0
        assert damp["peak2_time"] == 2.0
        assert damp["peak1_value"] == pytest.approx(36.78794412, abs=1e-6)
        assert damp["peak2_value"] == pytest.approx(13.53352832, abs=1e-6)
        assert damp["oscillation_period_s"] == pytest.approx(1.0)

    def test_baseline_subtraction(self) -> None:
        # HAND: same oscillation riding on a 50.0 Hz offset; after
        # baseline subtraction the peaks are identical, so the damping
        # ratio must be identical.
        t = [round(0.1 * k, 10) for k in range(0, 41)]
        v = [50.0 + 100.0 * math.exp(-x) * math.cos(2.0 * math.pi * x) for x in t]
        damp = eh.damping_ratio_from_window(t, v, baseline=50.0)
        assert damp["damping_ratio"] == pytest.approx(
            1.0 / math.sqrt(4.0 * math.pi * math.pi + 1.0)
        )

    def test_monotone_series_refused(self) -> None:
        # No interior peak samples → the log decrement is undefined.
        with pytest.raises(eh.MetricInputError):
            eh.damping_ratio_from_window([0.0, 1.0, 2.0], [1.0, 2.0, 3.0], 0.0)

    def test_growing_oscillation_refused(self) -> None:
        # x(t) = e^(+t)·cos(2πt): peak SAMPLES at t=1,2,3 have values
        # e^1 = 2.718... and e^2 = 7.389... — the second excursion is
        # LARGER (p2 > p1, growing envelope). A log decrement on a growing
        # oscillation would yield a negative delta and an unphysical
        # damping ratio; the function must refuse, not report a number.
        t = [round(0.1 * k, 10) for k in range(0, 41)]
        v = [math.exp(x) * math.cos(2.0 * math.pi * x) for x in t]
        with pytest.raises(eh.MetricInputError):
            eh.damping_ratio_from_window(t, v, baseline=0.0)


class TestSettling:
    def test_plain_decay(self) -> None:
        # HAND: band ±0.05; decay crosses 0.05 at t = 1 + 4·0.95 = 4.8;
        # samples straddle it: x(4.75) = 0.0625 (out), x(5.0) = 0.0 (in).
        # Settled from 5.0 s → 4.0 s after the event at 1.0 s.
        ts, vs = _decay_series(re_excite=False)
        st = eh.settling_time(ts, vs, 0.0, 1.0, 0.05)
        assert st["settled"] is True
        assert st["settling_time_s"] == pytest.approx(4.0)
        assert st["settled_abs_time_s"] == 5.0
        assert st["last_out_of_band_time_s"] == 4.75

    def test_re_excitation_holds_to_end(self) -> None:
        # HAND: bump to 0.5 at t=5.5 restores out-of-band; settled from
        # 5.75 s → 4.75 s after the event. A naive first-entry answer
        # (4.0 s) is wrong and must not be produced.
        ts, vs = _decay_series(re_excite=True)
        st = eh.settling_time(ts, vs, 0.0, 1.0, 0.05)
        assert st["settled"] is True
        assert st["settling_time_s"] == pytest.approx(4.75)
        assert st["last_out_of_band_time_s"] == 5.5

    def test_never_left_band(self) -> None:
        st = eh.settling_time([0.0, 1.0, 2.0, 3.0], [0.0, 0.0, 0.0, 0.0], 0.0, 1.0, 0.05)
        assert st["settled"] is True
        assert st["settling_time_s"] == 0.0

    def test_never_settled_no_extrapolation(self) -> None:
        st = eh.settling_time([0.0, 1.0, 2.0, 3.0], [0.0, 1.0, 0.5, 0.2], 0.0, 1.0, 0.05)
        assert st["settled"] is False
        assert st["settling_time_s"] is None

    def test_band_boundary_is_inclusive(self) -> None:
        # HAND: |x - baseline| <= tolerance counts as IN band: at t=2.0
        # the deviation is exactly 0.1 → in band; t=1.0 is 0.5 → out.
        st = eh.settling_time(
            [0.0, 1.0, 2.0, 3.0], [0.0, 0.5, 0.1, 0.1], 0.0, 1.0, 0.1
        )
        assert st["settled"] is True
        assert st["settled_abs_time_s"] == 2.0


class TestValidation:
    @pytest.mark.parametrize(
        "times, values",
        [
            ([0.0, 1.0], [1.0]),  # mismatched lengths
            ([0.0], [1.0]),  # single sample
            ([0.0, 1.0, 0.5], [1.0, 2.0, 3.0]),  # non-monotone times
            ([0.0, 1.0, 1.0], [1.0, 2.0, 3.0]),  # repeated timestamp
            ([0.0, float("nan"), 2.0], [1.0, 2.0, 3.0]),  # NaN time
            ([0.0, 1.0, 2.0], [1.0, float("inf"), 3.0]),  # inf value
        ],
    )
    def test_invalid_series_fail_closed(self, times, values) -> None:
        with pytest.raises(eh.MetricInputError):
            eh.excursion_extremes(times, values, 0.0, 0.0)

    def test_no_post_event_samples(self) -> None:
        with pytest.raises(eh.MetricInputError):
            eh.excursion_extremes([0.0, 1.0], [1.0, 1.0], 1.0, 5.0)

    def test_unknown_quantity_refused(self) -> None:
        with pytest.raises(eh.MetricInputError):
            eh.evaluate_response(
                [0.0, 1.0, 2.0],
                [1.0, 2.0, 3.0],
                baseline=0.0,
                event_time=0.0,
                tolerance=0.1,
                quantity="reactive_power_MVAr",  # not a declared quantity
            )


class TestComposition:
    def test_composed_document(self) -> None:
        # HAND (see evaluation_harness.run_self_tests, Test 10, for the
        # full arithmetic): max |ROCOF| = 1.1 Hz/s (the pre-event step at
        # t=0.5 is an interior sample and counts); settled 2.5 s after the
        # event; damping honestly refused with one diagnostic.
        result = eh.evaluate_response(
            NADIR_TIMES,
            NADIR_VALUES,
            baseline=50.0,
            event_time=2.0,
            tolerance=0.1,
            quantity="frequency_hz",
        )
        assert result["quantity"] == "frequency_hz"
        assert result["extremes"]["nadir_value"] == pytest.approx(49.2)
        assert result["max_rocof"]["max_abs_rocof"] == pytest.approx(1.1)
        assert result["max_rocof"]["rocof_time"] == 0.5
        assert result["damping_ratio"] is None
        assert len(result["diagnostics"]) == 1
        assert result["settling"]["settled"] is True
        assert result["settling"]["settling_time_s"] == pytest.approx(2.5)
        assert result["provenance"]["input_samples"] == len(NADIR_TIMES)

    def test_composed_with_damping_present(self) -> None:
        # HAND: an oscillating trace yields a damping ratio and NO
        # diagnostics.
        t = [round(0.1 * k, 10) for k in range(0, 41)]
        v = [100.0 * math.exp(-x) * math.cos(2.0 * math.pi * x) for x in t]
        result = eh.evaluate_response(
            t, v, baseline=0.0, event_time=0.0, tolerance=0.05, quantity="voltage_pu"
        )
        assert result["damping_ratio"] is not None
        assert result["diagnostics"] == []
