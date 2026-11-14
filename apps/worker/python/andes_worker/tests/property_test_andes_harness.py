# P8.37 — Hand-written PROPERTY tests for the dynamic evaluation harness
#               (the pure P8.17 metric definitions over INJECTED traces —
#               evaluation_harness.py of the andes_worker).
#
# ══════════════════════════════════════════════════════════════════════════
# HONESTY HEADER — read before trusting anything below
#
# 1. PROPERTY-BASED, STDLIB-ONLY, NO ANDES. No property-testing library
#    and no ANDES is imported or installed (the Phase 8 loop bans; P8.37
#    requires hand-written generators). Every generator is a plain
#    function over `random.Random(<fixed seed>)`. EACH property states
#    its ORACLE, its SEED, and its DETERMINISM contract (the module
#    constant PROPERTY_SEED; a fresh seed for the P8.37 verification
#    method is a one-constant change — the properties must hold for ANY
#    seed).
# 2. WRITTEN, NEVER RUN. pytest was never installed in this loop. The
#    tested surface is the stdlib-only metric module; no dynamic
#    simulation ever ran, and no measured figure exists (the P8.17
#    measured cells are EMPTY — ENTRY_NOTE boundary 4).
# 3. Run instruction (the recorded procedure):
#    `PYTHONPATH=apps/worker/python/andes_worker python -m pytest
#    apps/worker/python/andes_worker/tests -v` from the repository root.
# 4. WHAT THE PROPERTIES PIN (ledger P8.37 step 2, as code):
#    - P-REFUSE:  a malformed series (length mismatch, < 2 samples,
#                 non-finite value, non-increasing time) is ALWAYS
#                 refused (`MetricInputError`) — a number is never
#                 produced from malformed input;
#    - P-EXTREME: the nadir/zenith equal the independently recomputed
#                 min/max of the post-event window, and the reported
#                 times are the FIRST times achieving them (the min/max
#                 definitions are the oracle);
#    - P-ROCOF:   on a generated piecewise-linear signal the central-
#                 difference estimate equals the true interior slope
#                 where the signal is locally linear (the closed form is
#                 the oracle), and the reported extremum is the max-|slope|
#                 sample;
#    - P-SETTLE:  a settling verdict is consistent with the recorded
#                 definition recomputed from the series (the first band
#                 entry that HOLDS to the end; a signal out of band at
#                 the end is NOT settled, never extrapolated);
#    - P-DAMP:    for a decaying sampled oscillation whose peaks are
#                 sampled exactly, the damping ratio equals the
#                 logarithmic-decrement closed form recomputed in the
#                 test; a growing envelope is REFUSED, never measured.
#    Every metric is a statement about the INJECTED TRACE of a declared
#    simulation model — never about the real grid (report 19.1; a
#    steady-state tool is not a stability tool, and a simulated trace is
#    not a field measurement). Nothing here is dynamic-stability
#    evidence.
# ══════════════════════════════════════════════════════════════════════════

from __future__ import annotations

import math
import os
import random
import sys

import pytest

sys.path.insert(
    0,
    os.path.join(os.path.dirname(os.path.abspath(__file__)), os.pardir),
)

import evaluation_harness as mh  # noqa: E402 — path set above

PROPERTY_SEED = 20261114


def _rng() -> random.Random:
    return random.Random(PROPERTY_SEED)


def _series(rng: random.Random, size: int | None = None) -> tuple:
    """An arbitrary VALID series: strictly increasing finite times,
    finite values (a jagged random walk — nothing smooth assumed)."""
    size = size if size is not None else rng.randrange(2, 60)
    times = []
    t = rng.uniform(0.0, 10.0)
    for _ in range(size):
        times.append(t)
        t += abs(rng.uniform(0.001, 1.0))
    values = [rng.uniform(-2.0, 2.0) for _ in range(size)]
    return times, values


def _corrupt_series(rng: random.Random, times: list, values: list) -> tuple:
    """Applies ONE random MALFORMING corruption to a valid series (the
    P-REFUSE catalogue: length mismatch, single sample, non-finite
    value, repeated or decreasing time, non-numeric cell)."""
    corruption = rng.choice(
        ("length_mismatch", "single_sample", "nonfinite_value",
         "repeated_time", "decreasing_time", "non_numeric")
    )
    t = list(times)
    v = list(values)
    if corruption == "length_mismatch":
        v.pop() if len(v) > len(t) - 1 else t.pop()
        return t, v
    if corruption == "single_sample":
        return t[:1], v[:1]
    if corruption == "nonfinite_value":
        v[rng.randrange(len(v))] = rng.choice([float("nan"), float("inf")])
        return t, v
    if corruption == "non_numeric":
        v[rng.randrange(len(v))] = rng.choice(["x", True, None])
        return t, v
    index = rng.randrange(1, len(t))
    if corruption == "repeated_time":
        t[index] = t[index - 1]
    else:
        t[index] = t[index - 1] - abs(rng.uniform(0.001, 1.0))
    return t, v


# ---------------------------------------------------------------------------
# P-REFUSE — malformed series never yield a number
# ---------------------------------------------------------------------------


class TestMalformedSeriesRefusalProperty:
    def test_every_corruption_refused_by_every_metric(self) -> None:
        """P-REFUSE. Oracle: `MetricInputError` — every corruption in
        the catalogue, applied to an arbitrary valid series, is refused
        by EVERY entry-point metric (`excursion_extremes`, `max_rocof`,
        `damping_ratio_from_window`, `settling_time`), for arbitrary
        scalar arguments. Generator: seeded series + seeded corruption +
        seeded scalars."""
        rng = _rng()
        metrics = (
            lambda t, v: mh.excursion_extremes(
                t, v, rng.uniform(-2.0, 2.0), t[0]
            ),
            lambda t, v: mh.max_rocof(t, v),
            lambda t, v: mh.damping_ratio_from_window(
                t, v, baseline=rng.uniform(-2.0, 2.0)
            ),
            lambda t, v: mh.settling_time(
                t, v, rng.uniform(-2.0, 2.0), t[0], abs(rng.uniform(0.0, 1.0))
            ),
        )
        for _ in range(200):
            times, values = _series(rng, rng.randrange(3, 40))
            bad_t, bad_v = _corrupt_series(rng, times, values)
            metric = rng.choice(metrics)
            with pytest.raises(mh.MetricInputError):
                metric(bad_t, bad_v)

    def test_negative_settling_tolerance_refused(self) -> None:
        """P-REFUSE (scalar level). Oracle: the refusal — a negative
        settling tolerance is a malformed input, for arbitrary series."""
        rng = _rng()
        for _ in range(50):
            times, values = _series(rng)
            with pytest.raises(mh.MetricInputError):
                mh.settling_time(
                    times, values, 1.0, times[0], -abs(rng.uniform(1e-9, 1.0))
                )


# ---------------------------------------------------------------------------
# P-EXTREME — nadir/zenith are the window's min/max
# ---------------------------------------------------------------------------


class TestExcursionExtremesProperty:
    def test_extremes_match_recomputed_min_max(self) -> None:
        """P-EXTREME. Oracle: the min/max definitions recomputed
        independently — nadir/zenith values equal the post-event
        window's min/max, the reported times are the FIRST times
        achieving them, and the depths are baseline-relative. Generator:
        seeded series + seeded baseline/event_time (event_time always
        within the series span so the post-event window is non-empty)."""
        rng = _rng()
        for _ in range(200):
            times, values = _series(rng)
            baseline = rng.uniform(-2.0, 2.0)
            event_time = rng.choice(times)
            result = mh.excursion_extremes(times, values, baseline, event_time)
            post = [(t, v) for t, v in zip(times, values) if t >= event_time]
            nadir = min(v for _t, v in post)
            zenith = max(v for _t, v in post)
            assert result["nadir_value"] == nadir
            assert result["zenith_value"] == zenith
            assert result["nadir_time"] == min(
                t for t, v in post if v == nadir
            )
            assert result["zenith_time"] == min(
                t for t, v in post if v == zenith
            )
            assert result["nadir_depth"] == baseline - nadir
            assert result["zenith_height"] == zenith - baseline
            assert result["post_event_samples"] == len(post)

    def test_no_post_event_samples_refused(self) -> None:
        """P-EXTREME (window guard). Oracle: the refusal — an event
        time AFTER the whole series leaves an empty post-event window,
        which is refused (an excursion metric is defined only on the
        post-event window), for arbitrary series."""
        rng = _rng()
        for _ in range(50):
            times, values = _series(rng)
            with pytest.raises(mh.MetricInputError):
                mh.excursion_extremes(
                    times, values, 1.0, times[-1] + abs(rng.uniform(1e-6, 10.0))
                )


# ---------------------------------------------------------------------------
# P-ROCOF — the central-difference estimator
# ---------------------------------------------------------------------------


class TestMaxRocofProperty:
    def test_locally_linear_segment_slopes_are_exact(self) -> None:
        """P-ROCOF. Oracle: the closed form — on a signal that is
        EXACTLY linear on interior segments, the central difference at
        every interior sample equals the true slope, so the reported max
        |ROCOF| equals the max true |segment slope| recomputed in the
        test. Generator: seeded piecewise-linear signals with seeded
        slopes and breakpoints. (The module's documented artifact — an
        estimate at a breakpoint averages the two slopes — is AVOIDED
        here by construction: the max-slope segment is given at least
        three samples, so the reported extremum is an exact interior
        estimate.)"""
        rng = _rng()
        for _ in range(150):
            slopes = [rng.choice([-1.0, 1.0]) * rng.uniform(0.1, 5.0)
                      for _ in range(rng.randrange(2, 6))]
            dt = 0.1
            times = []
            values = []
            v = rng.uniform(-2.0, 2.0)
            for slope in slopes:
                for _ in range(3):  # three samples per segment: the
                    # middle one is an exact interior estimate
                    times.append(round(len(times) * dt, 10))
                    values.append(v)
                    v = v + slope * dt
            result = mh.max_rocof(times, values)
            true_max = max(abs(s) for s in slopes)
            assert result["max_abs_rocof"] == pytest.approx(true_max)
            assert result["interior_samples"] == len(times) - 2

    def test_two_samples_refused(self) -> None:
        """P-ROCOF (arity guard). Oracle: the refusal — a central
        difference needs THREE samples; a two-sample series is refused,
        for arbitrary values."""
        rng = _rng()
        for _ in range(50):
            times, values = _series(rng, 2)
            with pytest.raises(mh.MetricInputError):
                mh.max_rocof(times, values)


# ---------------------------------------------------------------------------
# P-SETTLE — the settling-time definition
# ---------------------------------------------------------------------------


class TestSettlingTimeProperty:
    def test_verdict_matches_recomputed_definition(self) -> None:
        """P-SETTLE. Oracle: the recorded definition recomputed
        independently — let L be the last post-event index with
        |v - baseline| > tolerance; the signal settles at
        times[L+1] - event_time when L exists and is not the final
        index; it NEVER settles when L is the final index (no
        extrapolation); it settles at 0.0 when no such index exists.
        Generator: seeded series, baselines, tolerances."""
        rng = _rng()
        for _ in range(200):
            times, values = _series(rng)
            baseline = rng.uniform(-2.0, 2.0)
            tolerance = abs(rng.uniform(0.0, 1.0))
            event_time = rng.choice(times)
            result = mh.settling_time(
                times, values, baseline, event_time, tolerance
            )
            post_indices = [i for i, t in enumerate(times) if t >= event_time]
            last_out = None
            for i in post_indices:
                if abs(values[i] - baseline) > tolerance:
                    last_out = i
            if last_out is None:
                assert result["settled"] is True
                assert result["settling_time_s"] == 0.0
            elif last_out == len(times) - 1:
                assert result["settled"] is False
                assert result["settling_time_s"] is None
            else:
                assert result["settled"] is True
                assert result["settled_abs_time_s"] == times[last_out + 1]
                assert result["settling_time_s"] == (
                    times[last_out + 1] - event_time
                )


# ---------------------------------------------------------------------------
# P-DAMP — the logarithmic-decrement damping ratio
# ---------------------------------------------------------------------------


class TestDampingRatioProperty:
    def _decaying_peak_sampled_series(self, rng: random.Random) -> tuple:
        """A sampled decaying oscillation whose samples land EXACTLY on
        the true peaks (the module's own exactness condition, met by
        construction): positive triangular peaks with geometrically
        decaying amplitude on a uniform time grid."""
        p1 = rng.uniform(0.5, 2.0)
        decay = rng.uniform(0.2, 0.9)
        p2 = p1 * decay
        period = rng.uniform(0.2, 2.0)
        half = period / 2.0
        times = []
        values = []
        t = 0.0
        peaks = [p1, p2]
        for i, peak in enumerate(peaks):
            # rise to the peak, fall from it (triangular, sampled
            # densely; the peak sample is exact)
            steps = rng.randrange(3, 8)
            for s in range(steps + 1):
                times.append(t)
                values.append(peak * s / steps)
                t += half / steps
            for s in range(1, steps + 1):
                times.append(t)
                values.append(peak * (1.0 - s / steps))
                t += half / steps
        return times, values, p1, p2, period

    def test_damping_matches_closed_form_when_peaks_are_sampled(self) -> None:
        """P-DAMP. Oracle: the closed form recomputed in the test —
        delta = ln(p1/p2); zeta = delta / sqrt(4 pi^2 + delta^2) — for a
        series whose first two successive peak samples are exactly p1
        and p2 by construction. Generator: seeded decaying triangular
        oscillations."""
        rng = _rng()
        for _ in range(100):
            times, values, p1, p2, period = (
                self._decaying_peak_sampled_series(rng)
            )
            result = mh.damping_ratio_from_window(times, values, baseline=0.0)
            delta = math.log(p1 / p2)
            assert result["log_decrement"] == pytest.approx(delta)
            assert result["damping_ratio"] == pytest.approx(
                delta / math.sqrt(4.0 * math.pi * math.pi + delta * delta)
            )
            assert result["peak1_value"] == pytest.approx(p1)
            assert result["peak2_value"] == pytest.approx(p2)
            assert 0.0 < result["damping_ratio"] < 1.0

    def test_growing_envelope_refused(self) -> None:
        """P-DAMP (the instability refusal). Oracle: the refusal — a
        GROWING peak envelope (p2 >= p1) is refused, never measured as
        a damping ratio (a growing oscillation is an instability
        observation for the caller), for arbitrary growing
        constructions."""
        rng = _rng()
        for _ in range(50):
            growth = rng.uniform(1.1, 3.0)
            times = []
            values = []
            t = 0.0
            for peak in (1.0, 1.0 * growth):
                steps = 4
                for s in range(steps + 1):
                    times.append(t)
                    values.append(peak * s / steps)
                    t += 0.1
                for s in range(1, steps + 1):
                    times.append(t)
                    values.append(peak * (1.0 - s / steps))
                    t += 0.1
            with pytest.raises(mh.MetricInputError):
                mh.damping_ratio_from_window(times, values, baseline=0.0)
