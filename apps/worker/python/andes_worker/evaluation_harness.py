# P8.17 — Dynamic-response evaluation harness: PURE metrics computed on
#         INJECTED time-series data (no simulator, no I/O, no ANDES).
#
# ══════════════════════════════════════════════════════════════════════════
# HONESTY HEADER — read before trusting anything below
#
# 1. WHAT THIS MODULE IS: pure functions that compute dynamic-response
#    metrics — frequency nadir/zenith, maximum ROCOF, damping ratio from a
#    decaying-oscillation window, and settling time — from time-series
#    data handed to them by the caller. Every function is STDLIB-ONLY
#    (math only), performs NO I/O, imports NO simulator, and can be
#    imported and tested today without ANDES (or anything else) installed.
#    The traces these metrics will eventually consume come from the ANDES
#    dynamic worker (`andes_dynamic_worker.py`) — the ANDES run itself is
#    a HUMAN/LAB GATE input (P8.17; ENTRY_NOTE boundary 4: every measured
#    cell is EMPTY; never fabricate a figure).
# 2. WHAT THIS MODULE IS NOT: it is not a stability assessment, not a
#    protection-grade measurement chain, and not evidence about the real
#    grid. A metric computed on a trace is a statement ABOUT THAT TRACE of
#    A DECLARED SIMULATION MODEL (§19.1) — the model scope travels with
#    every result in the worker's response envelope, never with this
#    module (which sees only numbers).
# 3. METRIC DEFINITIONS (recorded, each pinned by a hand-computed self-test
#    in `run_self_tests()` below):
#    - nadir:     min(v) over samples with t >= event_time; depth =
#                 baseline − nadir. Pre-event samples are EXCLUDED.
#    - zenith:    max(v) over the same post-event window; height =
#                 zenith − baseline.
#    - max ROCOF: max |Δv/Δt| over INTERIOR samples via central
#                 differences (v[i+1] − v[i−1]) / (t[i+1] − t[i−1]).
#                 First/last samples have no central difference and are
#                 excluded by construction. A breakpoint between segments
#                 makes the central difference at the adjacent sample an
#                 AVERAGE of the two slopes (documented artifact — see the
#                 two-segment self-test).
#    - damping:   logarithmic decrement from the FIRST TWO successive
#                 local-peak SAMPLES of the detrended (baseline-subtracted)
#                 window: δ = ln(p1/p2); ζ = δ / sqrt(4π² + δ²). Peak
#                 SAMPLES (strictly greater than both neighbours) are an
#                 estimator of the true continuous peaks: with sampling
#                 aligned so samples land exactly on true peaks, the
#                 estimate is exact (the self-test constructs exactly that
#                 case and states the arithmetic).
#    - settling:  the first post-event sample time T such that EVERY
#                 sample from T to the end of the series satisfies
#                 |v − baseline| <= tolerance. "Stays until the end" is
#                 part of the definition: a signal that re-exits the band
#                 has NOT settled at its first entry (the self-test pins
#                 this with a re-excitation case). Settling time is
#                 reported relative to event_time; "never left the band"
#                 is reported as 0.0; "still out of band at the end of the
#                 series" is reported as NOT SETTLED (never extrapolated).
# 4. UNITS: frequency quantities are Hz, ROCOF is Hz/s, voltage is per-unit
#    (pu, solver representation), time is seconds, power is MW/MVAr (the
#    §10.2 canonical units). Sign convention for P (MW) is NOT used here —
#    these functions are unit-agnostic on the value axis; the CALLER (the
#    worker's response envelope) carries the unit and sign declaration.
# 5. WRITTEN, NEVER EXECUTED. No function in this module has ever run in
#    this loop (the loop bans execution). The self-tests below carry HAND-
#    COMPUTED expected values — each expected figure is derived in a
#    comment from the constructed input, never read back from the code
#    under test. The first execution belongs to the human build/test pass.
# ══════════════════════════════════════════════════════════════════════════

from __future__ import annotations

import math

# The quantities the composed evaluator accepts (unit-declared axes).
QUANTITIES: tuple[str, ...] = ("frequency_hz", "voltage_pu")


class MetricInputError(ValueError):
    """A metric's injected inputs are invalid (fail-closed).

    Raised BEFORE any metric value is produced: a malformed series can
    never yield a number, because a number computed from malformed input
    would be indistinguishable from a real measurement.
    """


def _validate_series(times: list, values: list) -> None:
    """Fail-closed validation shared by every metric.

    Requires: equal, non-empty lengths; at least two samples; every time
    and value a finite (non-boolean) real number; times STRICTLY
    increasing (a repeated timestamp cannot be ordered, and a metric on
    unordered time is not a metric).
    """
    if not isinstance(times, (list, tuple)) or not isinstance(values, (list, tuple)):
        raise MetricInputError("times and values must be sequences of numbers")
    if len(times) != len(values):
        raise MetricInputError(
            "times and values must have equal length (%d vs %d)"
            % (len(times), len(values))
        )
    if len(times) < 2:
        raise MetricInputError("a metric needs at least two samples")
    previous_time = None
    for index, (t, v) in enumerate(zip(times, values)):
        if isinstance(t, bool) or not isinstance(t, (int, float)) or not math.isfinite(t):
            raise MetricInputError("times[%d] is not a finite real number" % index)
        if isinstance(v, bool) or not isinstance(v, (int, float)) or not math.isfinite(v):
            raise MetricInputError("values[%d] is not a finite real number" % index)
        if previous_time is not None and t <= previous_time:
            raise MetricInputError(
                "times must be strictly increasing (times[%d]=%r follows %r)"
                % (index, t, previous_time)
            )
        previous_time = t


def _validate_scalar(name: str, value) -> None:
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(value):
        raise MetricInputError("%s must be a finite real number" % name)


def _post_event_samples(times: list, values: list, event_time: float) -> list:
    post = [(t, v) for t, v in zip(times, values) if t >= event_time]
    if not post:
        raise MetricInputError(
            "no samples at or after event_time=%r (an excursion metric is "
            "defined only on the post-event window)" % event_time
        )
    return post


# ---------------------------------------------------------------------------
# Extremes (nadir / zenith) over the post-event window
# ---------------------------------------------------------------------------


def excursion_extremes(
    times: list, values: list, baseline: float, event_time: float
) -> dict:
    """Minimum and maximum over the post-event window, relative to baseline.

    Pure. Returns unit-agnostic keys; the thin wrappers below attach the
    declared units (Hz / pu) to the same computation.
    """
    _validate_series(times, values)
    _validate_scalar("baseline", baseline)
    _validate_scalar("event_time", event_time)
    post = _post_event_samples(times, values, event_time)
    nadir_value = min(v for _t, v in post)
    nadir_time = min(t for t, v in post if v == nadir_value)
    zenith_value = max(v for _t, v in post)
    zenith_time = min(t for t, v in post if v == zenith_value)
    return {
        "nadir_value": nadir_value,
        "nadir_depth": baseline - nadir_value,
        "nadir_time": nadir_time,
        "zenith_value": zenith_value,
        "zenith_height": zenith_value - baseline,
        "zenith_time": zenith_time,
        "baseline": baseline,
        "event_time": event_time,
        "post_event_samples": len(post),
    }


def frequency_nadir(
    times: list, values: list, baseline_hz: float, event_time_s: float
) -> dict:
    """Frequency nadir (Hz) and depth (Hz) over the post-event window.

    A frequency nadir is a property OF THE INJECTED TRACE of a declared
    simulation model — never a statement about the real grid's frequency
    stability (§19.1; a steady-state tool is not a stability tool, and a
    simulated trace is not a field measurement).
    """
    result = excursion_extremes(times, values, baseline_hz, event_time_s)
    return {
        "nadir_hz": result["nadir_value"],
        "nadir_depth_hz": result["nadir_depth"],
        "nadir_time_s": result["nadir_time"],
        "baseline_hz": result["baseline"],
        "event_time_s": result["event_time"],
        "post_event_samples": result["post_event_samples"],
    }


def voltage_extremes(
    times: list, values: list, baseline_pu: float, event_time_s: float
) -> dict:
    """Voltage nadir/zenith (pu, solver representation) post-event."""
    result = excursion_extremes(times, values, baseline_pu, event_time_s)
    return {
        "v_min_pu": result["nadir_value"],
        "v_min_depth_pu": result["nadir_depth"],
        "v_min_time_s": result["nadir_time"],
        "v_max_pu": result["zenith_value"],
        "v_max_height_pu": result["zenith_height"],
        "v_max_time_s": result["zenith_time"],
        "baseline_pu": result["baseline"],
        "event_time_s": result["event_time"],
        "post_event_samples": result["post_event_samples"],
    }


# ---------------------------------------------------------------------------
# Maximum ROCOF (central differences, interior samples)
# ---------------------------------------------------------------------------


def max_rocof(times: list, values: list) -> dict:
    """Max |dv/dt| over interior samples via central differences.

    The estimator at sample i is (v[i+1] − v[i−1]) / (t[i+1] − t[i−1]).
    Non-uniform sampling is honoured (the actual time span is used), but
    the estimator is exact only where the signal is locally linear — a
    slope breakpoint makes the adjacent sample's estimate an average of
    the two slopes (documented artifact, pinned by the two-segment
    self-test).
    """
    _validate_series(times, values)
    best_signed = None
    best_time = None
    for i in range(1, len(times) - 1):
        span = times[i + 1] - times[i - 1]
        slope = (values[i + 1] - values[i - 1]) / span
        if not math.isfinite(slope):
            raise MetricInputError("central difference at index %d is not finite" % i)
        if best_signed is None or abs(slope) > abs(best_signed):
            best_signed = slope
            best_time = times[i]
    if best_signed is None:
        raise MetricInputError(
            "a max-ROCOF estimate needs at least THREE samples: a central "
            "difference requires both neighbours, so the first and last "
            "samples never contribute"
        )
    return {
        "max_abs_rocof": abs(best_signed),
        "max_signed_rocof": best_signed,
        "rocof_time": best_time,
        "method": "central_difference_interior_samples",
        "interior_samples": len(times) - 2,
    }


# ---------------------------------------------------------------------------
# Damping ratio (logarithmic decrement, successive peak samples)
# ---------------------------------------------------------------------------


def damping_ratio_from_window(
    times: list, values: list, baseline: float = 0.0
) -> dict:
    """Damping ratio from a decaying-oscillation window (log decrement).

    Method: subtract `baseline`; detect local-peak SAMPLES (strictly
    greater than both neighbours — the first and last samples of the
    window can never be peaks); take the FIRST TWO successive peaks;
    δ = ln(p1/p2); ζ = δ / sqrt(4π² + δ²).

    Constraints (fail-closed): at least two peak samples; both peak
    excursions strictly positive (a log decrement is defined on a decaying
    positive envelope; a growing or non-positive oscillation is refused,
    not silently measured). Peak samples estimate the true continuous
    peaks — exact only when samples land on the true peaks (the self-test
    constructs that case and states the arithmetic).
    """
    _validate_series(times, values)
    _validate_scalar("baseline", baseline)
    detrended = [v - baseline for v in values]
    peaks: list = []
    for i in range(1, len(times) - 1):
        if detrended[i] > detrended[i - 1] and detrended[i] > detrended[i + 1]:
            peaks.append((times[i], detrended[i]))
    if len(peaks) < 2:
        raise MetricInputError(
            "fewer than two successive peak samples in the window (%d found); "
            "a log-decrement damping ratio is not computable" % len(peaks)
        )
    (t1, p1), (t2, p2) = peaks[0], peaks[1]
    if p1 <= 0.0 or p2 <= 0.0:
        raise MetricInputError(
            "log-decrement damping needs two positive peak excursions "
            "(got %r and %r after baseline subtraction)" % (p1, p2)
        )
    if p2 >= p1:
        raise MetricInputError(
            "the peak envelope is GROWING (p2=%r >= p1=%r): this window is "
            "not a decaying oscillation, so a damping ratio is not "
            "computable here; a growing oscillation is recorded as an "
            "instability observation by the caller, never measured as a "
            "damping ratio" % (p1, p2)
        )
    period = t2 - t1
    if period <= 0.0:
        raise MetricInputError("peak times must be strictly ordered")
    delta = math.log(p1 / p2)
    zeta = delta / math.sqrt(4.0 * math.pi * math.pi + delta * delta)
    return {
        "damping_ratio": zeta,
        "log_decrement": delta,
        "peak1_time": t1,
        "peak1_value": p1,
        "peak2_time": t2,
        "peak2_value": p2,
        "oscillation_period_s": period,
        "peak_count": len(peaks),
        "method": "logarithmic_decrement_successive_peak_samples",
    }


# ---------------------------------------------------------------------------
# Settling time (first entry into a band that HOLDS to the end)
# ---------------------------------------------------------------------------


def settling_time(
    times: list, values: list, baseline: float, event_time: float, tolerance: float
) -> dict:
    """First post-event band entry that holds to the end of the series.

    Definition (recorded): let T be the earliest post-event sample time
    such that |v − baseline| <= tolerance for EVERY sample from T to the
    last sample. Settling time is T − event_time. Outcomes:
      - a signal that never leaves the band settles at 0.0 (relative);
      - a signal still out of band at the series' end is NOT SETTLED —
        never extrapolated, never reported as a number;
      - a re-exit after entry moves the settled time past the re-entry
        (pinned by the re-excitation self-test).
    """
    _validate_series(times, values)
    _validate_scalar("baseline", baseline)
    _validate_scalar("event_time", event_time)
    _validate_scalar("tolerance", tolerance)
    if tolerance < 0.0:
        raise MetricInputError("tolerance must be non-negative")
    post_indices = [i for i, t in enumerate(times) if t >= event_time]
    if not post_indices:
        raise MetricInputError(
            "no samples at or after event_time=%r" % event_time
        )
    last_out_index = None
    for i in post_indices:
        if abs(values[i] - baseline) > tolerance:
            last_out_index = i
    if last_out_index is None:
        return {
            "settled": True,
            "settling_time_s": 0.0,
            "settled_abs_time_s": times[post_indices[0]],
            "last_out_of_band_time_s": None,
            "tolerance": tolerance,
            "baseline": baseline,
            "event_time_s": event_time,
            "note": "never left the tolerance band after the event",
        }
    if last_out_index == len(times) - 1:
        return {
            "settled": False,
            "settling_time_s": None,
            "settled_abs_time_s": None,
            "last_out_of_band_time_s": times[last_out_index],
            "tolerance": tolerance,
            "baseline": baseline,
            "event_time_s": event_time,
            "note": "still out of band at the end of the series; "
                    "not settled (never extrapolated)",
        }
    settled_abs = times[last_out_index + 1]
    return {
        "settled": True,
        "settling_time_s": settled_abs - event_time,
        "settled_abs_time_s": settled_abs,
        "last_out_of_band_time_s": times[last_out_index],
        "tolerance": tolerance,
        "baseline": baseline,
        "event_time_s": event_time,
        "note": "first band entry that holds to the end of the series",
    }


# ---------------------------------------------------------------------------
# Composed per-trace evaluation
# ---------------------------------------------------------------------------


def evaluate_response(
    times: list,
    values: list,
    *,
    baseline: float,
    event_time: float,
    tolerance: float,
    quantity: str,
) -> dict:
    """All applicable metrics for one injected trace, in one document.

    Damping is OPTIONAL by nature: a trace without a decaying oscillation
    yields `damping_ratio: None` with the failure reason recorded in
    `diagnostics` — an inapplicable metric is recorded as such, never
    silently omitted and never forced.
    """
    if quantity not in QUANTITIES:
        raise MetricInputError(
            "quantity %r is not one of %s" % (quantity, list(QUANTITIES))
        )
    diagnostics: list = []
    extremes = excursion_extremes(times, values, baseline, event_time)
    rocof = max_rocof(times, values)
    try:
        damping = damping_ratio_from_window(times, values, baseline=baseline)
    except MetricInputError as error:
        damping = None
        diagnostics.append("damping_ratio not computable: %s" % error)
    settling = settling_time(times, values, baseline, event_time, tolerance)
    return {
        "quantity": quantity,
        "extremes": extremes,
        "max_rocof": rocof,
        "damping_ratio": damping,
        "settling": settling,
        "diagnostics": diagnostics,
        "provenance": {
            "module": "evaluation_harness",
            "metric_set": "P8.17 pure dynamic-response metrics",
            "input_samples": len(times),
            "note": "metrics computed on INJECTED data; every figure is a "
                    "statement about the injected trace of a declared "
                    "simulation model, never about the real grid (19.1)",
        },
    }


# ══════════════════════════════════════════════════════════════════════════
# IN-MODULE SELF-TESTS — HAND-COMPUTED expected values.
#
# WRITTEN, NEVER EXECUTED (loop bans). Every expected figure below is
# derived BY HAND from the constructed input, with the arithmetic stated
# in the comment; nothing is read back from the functions under test
# except where explicitly marked as the formula itself (ζ's closed form).
# The human build/test pass runs them via:
#   python apps/worker/python/andes_worker/evaluation_harness.py
# (prints one PASS/FAIL line per test, exit 0 only if all pass) or via
#   python -m pytest apps/worker/python/andes_worker/tests -v
# (the pytest suite mirrors these cases in test_evaluation_harness.py).
# ══════════════════════════════════════════════════════════════════════════


def run_self_tests() -> list:
    """Runs every hand-computed self-test; returns one record per test.

    Each record: {"name", "passed", "detail"}. This function itself has
    NEVER been executed in this loop; the expected values are hand
    derivations, not observed outputs.
    """
    results: list = []

    def check(name: str, condition: bool, detail: str) -> None:
        results.append({"name": name, "passed": bool(condition), "detail": detail})

    # ---- Test 1: frequency nadir --------------------------------------
    # Constructed series (Hz): a pre-event dip at t=0 (48.9) that MUST be
    # excluded, event at t=2.0, post-event minimum 49.2 at t=3.0.
    # HAND ARITHMETIC: post-event samples are the values at t=2.0..5.0;
    # their minimum is 49.2; depth = 50.0 − 49.2 = 0.8 Hz.
    nadir_times = [0.0, 0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5, 4.0, 4.5, 5.0]
    nadir_values = [48.9, 50.0, 50.0, 50.0, 50.0, 49.4, 49.2, 49.35, 49.6, 49.9, 50.0]
    nadir = frequency_nadir(nadir_times, nadir_values, 50.0, 2.0)
    check(
        "frequency_nadir_post_event_only",
        abs(nadir["nadir_hz"] - 49.2) < 1e-12
        and abs(nadir["nadir_depth_hz"] - 0.8) < 1e-12
        and nadir["nadir_time_s"] == 3.0,
        "expected nadir 49.2 Hz, depth 0.8 Hz (50.0-49.2), time 3.0 s; "
        "pre-event 48.9 Hz excluded",
    )

    # ---- Test 2: voltage zenith ---------------------------------------
    # Constructed series (pu): pre-event spike 1.2 (excluded), post-event
    # maximum 1.06 at t=3.0. HAND ARITHMETIC: height = 1.06 − 1.00 = 0.06 pu.
    volt_times = [0.0, 0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5, 4.0, 4.5, 5.0]
    volt_values = [1.2, 1.0, 1.0, 1.0, 1.0, 1.05, 1.06, 1.02, 1.0, 1.0, 1.0]
    volt = voltage_extremes(volt_times, volt_values, 1.0, 2.0)
    check(
        "voltage_zenith_post_event_only",
        abs(volt["v_max_pu"] - 1.06) < 1e-12
        and abs(volt["v_max_height_pu"] - 0.06) < 1e-12
        and volt["v_max_time_s"] == 3.0,
        "expected zenith 1.06 pu, height 0.06 pu (1.06-1.00), time 3.0 s",
    )

    # ---- Test 3: ROCOF on a linear ramp (exact slope) ------------------
    # v(t) = 50 + 0.5·t sampled at dt=0.25. HAND ARITHMETIC: the central
    # difference at any interior sample is
    #   (v(t+h) − v(t−h)) / (2h) = (0.5(t+h) − 0.5(t−h)) / (2h) = 0.5
    # exactly (the signal is globally linear), so max |ROCOF| = 0.5 Hz/s.
    ramp_t = [0.0, 0.25, 0.5, 0.75, 1.0, 1.25, 1.5]
    ramp_v = [50.0 + 0.5 * t for t in ramp_t]
    rocof = max_rocof(ramp_t, ramp_v)
    check(
        "max_rocof_linear_ramp_exact",
        abs(rocof["max_abs_rocof"] - 0.5) < 1e-12,
        "expected exactly 0.5 Hz/s on a 0.5 Hz/s ramp",
    )

    # ---- Test 4: ROCOF across a slope breakpoint (documented artifact) -
    # v(t) = 50 + 0.5·t for t <= 1.0; v(t) = 50.5 + 2.0·(t−1) for t > 1.0.
    # Samples at dt=0.25 from 0 to 2.0.
    # HAND ARITHMETIC per interior sample:
    #   t=1.25: (v(1.5)−v(0.75))/(1.5−0.75) = (51.5−50.375)/0.75 = 1.5
    #   t=1.50: (v(1.75)−v(1.25))/(0.5)   = (52.0−51.0)/0.5   = 2.0
    #   t=1.75: (v(2.0)−v(1.5))/(0.5)     = (52.5−51.5)/0.5   = 2.0
    #   t=1.00: (v(1.25)−v(0.75))/(0.5)   = (51.0−50.375)/0.5 = 1.25
    # The t=1.25 estimate (1.5) is the documented averaging artifact of a
    # central difference straddling the breakpoint. The maximum over the
    # interior is exactly 2.0 Hz/s.
    seg_t = [0.0, 0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0]
    seg_v = [50.0 + 0.5 * t if t <= 1.0 else 50.5 + 2.0 * (t - 1.0) for t in seg_t]
    rocof2 = max_rocof(seg_t, seg_v)
    check(
        "max_rocof_two_segments",
        abs(rocof2["max_abs_rocof"] - 2.0) < 1e-12 and rocof2["rocof_time"] == 1.5,
        "expected max |ROCOF| = 2.0 Hz/s (first attained at t=1.5 s); the "
        "breakpoint sample t=1.25 averages to 1.5 Hz/s by hand",
    )

    # ---- Test 5: damping ratio, analytically known decaying sinusoid ---
    # x(t) = 100·e^(−t)·cos(2π·t), sampled t = 0.0 .. 4.0 step 0.1.
    # TRUE peaks (cos = +1) sit at integer t with envelope 100·e^(−k):
    #   x(1) = 100·e^(−1) = 36.78794412
    #   x(2) = 100·e^(−2) = 13.53352832
    # Sampling check (by hand): the continuous maxima near t≈0.975/1.975
    # fall BETWEEN samples (0.9: 32.89, 1.0: 36.79, 1.1: 26.93 — so t=1.0
    # is the local-peak SAMPLE), and the trough region around t=0.5
    # (x(0.4)=-54.2, x(0.5)=-60.7, x(0.6)=-44.4) contains no positive
    # local-maximum sample. Detected peak SAMPLES: t = 1, 2, 3.
    # HAND ARITHMETIC:
    #   δ = ln(x(1)/x(2)) = ln(100·e^(−1) / 100·e^(−2)) = ln(e¹) = 1.0
    #   ζ = δ / sqrt(4π² + δ²) = 1 / sqrt(4π² + 1)
    #     = 1 / sqrt(39.478417604357 + 1) = 1 / sqrt(40.478417604357)
    #     = 1 / 6.362262 = 0.157177 (to 6 dp)
    damped_t = [round(0.1 * k, 10) for k in range(0, 41)]
    damped_v = [100.0 * math.exp(-t) * math.cos(2.0 * math.pi * t) for t in damped_t]
    damp = damping_ratio_from_window(damped_t, damped_v, baseline=0.0)
    zeta_closed_form = 1.0 / math.sqrt(4.0 * math.pi * math.pi + 1.0)
    check(
        "damping_ratio_analytic_decaying_sinusoid",
        abs(damp["log_decrement"] - 1.0) < 1e-9
        and abs(damp["damping_ratio"] - zeta_closed_form) < 1e-12
        and 0.1571 <= damp["damping_ratio"] <= 0.1573
        and damp["peak1_time"] == 1.0
        and damp["peak2_time"] == 2.0
        and abs(damp["peak1_value"] - 36.78794412) < 1e-6
        and abs(damp["peak2_value"] - 13.53352832) < 1e-6
        and abs(damp["oscillation_period_s"] - 1.0) < 1e-12,
        "expected delta = ln(100e^-1 / 100e^-2) = 1.0 exactly; "
        "zeta = 1/sqrt(4*pi^2+1) = 0.157177 (6 dp); peaks at t=1,2 with "
        "values 36.78794412 / 13.53352832; period 1.0 s",
    )

    # ---- Test 6: settling time, plain decay ----------------------------
    # x(t) = 0 for t < 1.0; x(1.0) = 1.0; x(t) = 1 − 0.25·(t−1) for
    # 1.0 < t <= 5.0; x(t) = 0 for t > 5.0. Samples at dt=0.25, 0..6.
    # HAND ARITHMETIC: band ±0.05 around baseline 0. The linear decay
    # crosses 0.05 at t = 1 + 4·0.95 = 4.8 s; the SAMPLES straddle it:
    #   x(4.75) = 1 − 0.25·3.75 = 0.0625 > 0.05  (out of band)
    #   x(5.00) = 1 − 0.25·4.00 = 0.0000 <= 0.05 (in band, and all later
    #   samples are 0.0)
    # Last out-of-band sample: 4.75 s. Settled from 5.0 s. Relative to the
    # event at 1.0 s: 5.0 − 1.0 = 4.0 s.
    def decay_series(re_excite: bool):
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
            # Bump at t=5.5 back out of band, restored from t=5.75 on.
            for i, t in enumerate(ts):
                if t == 5.5:
                    vs[i] = 0.5
        return ts, vs

    s_times, s_values = decay_series(re_excite=False)
    st = settling_time(s_times, s_values, 0.0, 1.0, 0.05)
    check(
        "settling_time_plain_decay",
        st["settled"] is True
        and abs(st["settling_time_s"] - 4.0) < 1e-12
        and st["settled_abs_time_s"] == 5.0
        and st["last_out_of_band_time_s"] == 4.75,
        "expected settled at 5.0 s (4.0 s after the event); last out-of-band "
        "sample 4.75 s (x=0.0625 > 0.05 by hand)",
    )

    # ---- Test 7: settling time, re-excitation pins "holds to the end" --
    # Same series with a bump to 0.5 at t=5.5, restored from t=5.75.
    # HAND ARITHMETIC: the FIRST band entry (5.0 s) does NOT hold; the last
    # out-of-band sample is 5.5 s; settled from 5.75 s → 5.75 − 1.0 = 4.75 s.
    # A naive "first entry" implementation would (wrongly) answer 4.0 s.
    r_times, r_values = decay_series(re_excite=True)
    st2 = settling_time(r_times, r_values, 0.0, 1.0, 0.05)
    check(
        "settling_time_re_excitation_holds_to_end",
        st2["settled"] is True
        and abs(st2["settling_time_s"] - 4.75) < 1e-12
        and st2["settled_abs_time_s"] == 5.75
        and st2["last_out_of_band_time_s"] == 5.5,
        "expected settled at 5.75 s (4.75 s after the event); the first "
        "entry at 5.0 s does NOT hold (re-exit at 5.5 s)",
    )

    # ---- Test 8: settling — never left the band / never settled --------
    # (a) constant 0.0 after the event: settles at 0.0 s relative.
    flat = settling_time([0.0, 1.0, 2.0, 3.0], [0.0, 0.0, 0.0, 0.0], 0.0, 1.0, 0.05)
    check(
        "settling_time_never_left_band",
        flat["settled"] is True and flat["settling_time_s"] == 0.0,
        "expected settled=True, 0.0 s (never left the band)",
    )
    # (b) still out of band at the end: NOT settled, no number fabricated.
    never = settling_time(
        [0.0, 1.0, 2.0, 3.0], [0.0, 1.0, 0.5, 0.2], 0.0, 1.0, 0.05
    )
    check(
        "settling_time_never_settled_no_extrapolation",
        never["settled"] is False and never["settling_time_s"] is None,
        "expected settled=False with a null settling time (still out of "
        "band at the series end)",
    )

    # ---- Test 9: fail-closed validation --------------------------------
    def expects_error(name: str, fn, detail: str) -> None:
        try:
            fn()
        except MetricInputError:
            check(name, True, detail)
            return
        except Exception as error:  # noqa: BLE001 — pin the error class
            check(name, False, "%s; raised %r instead of MetricInputError" % (detail, error))
            return
        check(name, False, "%s; raised nothing" % detail)

    expects_error(
        "validation_mismatched_lengths",
        lambda: max_rocof([0.0, 1.0], [1.0]),
        "length mismatch must raise MetricInputError",
    )
    expects_error(
        "validation_non_monotone_times",
        lambda: max_rocof([0.0, 1.0, 0.5], [1.0, 2.0, 3.0]),
        "non-increasing times must raise MetricInputError",
    )
    expects_error(
        "validation_non_finite_value",
        lambda: max_rocof([0.0, 1.0, 2.0], [1.0, float("nan"), 3.0]),
        "a NaN value must raise MetricInputError",
    )
    expects_error(
        "validation_single_sample",
        lambda: excursion_extremes([0.0], [1.0], 0.0, 0.0),
        "a one-sample series must raise MetricInputError",
    )
    expects_error(
        "validation_no_post_event_samples",
        lambda: excursion_extremes([0.0, 1.0], [1.0, 1.0], 1.0, 5.0),
        "an empty post-event window must raise MetricInputError",
    )
    expects_error(
        "damping_needs_two_peaks",
        lambda: damping_ratio_from_window([0.0, 1.0, 2.0], [1.0, 2.0, 3.0], 0.0),
        "a monotone ramp has no peak samples; damping must raise",
    )

    # ---- Test 10: composed evaluation, damping honestly inapplicable ----
    # Same injected series as Test 1 (baseline 50.0 Hz, event 2.0 s,
    # tolerance 0.1 Hz).
    # HAND ARITHMETIC for the composed figures:
    # - extremes: unchanged from Test 1 (nadir 49.2 Hz, depth 0.8 Hz).
    # - max |ROCOF| over ALL interior samples (the extremes' post-event
    #   exclusion does NOT apply to ROCOF): the interior central
    #   differences are
    #     t=0.5: (50.0-48.9)/1.0 = 1.1   <- the pre-event step, included
    #     t=1.0: (50.0-50.0)/1.0 = 0.0
    #     t=1.5: (50.0-50.0)/1.0 = 0.0
    #     t=2.0: (49.4-50.0)/1.0 = 0.6
    #     t=2.5: (49.2-50.0)/1.0 = 0.8
    #     t=3.0: (49.35-49.4)/1.0 = 0.05
    #     t=3.5: (49.6-49.2)/1.0 = 0.4
    #     t=4.0: (49.9-49.35)/1.0 = 0.55
    #     t=4.5: (50.0-49.6)/1.0 = 0.4
    #   → max |ROCOF| = 1.1 Hz/s at t=0.5 s.
    # - damping: the trace has no two positive local-peak samples (only a
    #   monotone dip and recovery) → recorded as None with one diagnostic,
    #   never forced.
    # - settling (band ±0.1 Hz): |49.4-50|=0.6, |49.2-50|=0.8,
    #   |49.35-50|=0.65, |49.6-50|=0.4 are all > 0.1 (last out-of-band
    #   sample t=4.0); |49.9-50|=0.1 <= 0.1 and |50.0-50|=0.0 hold to the
    #   end → settled from 4.5 s = 4.5 - 2.0 = 2.5 s after the event.
    composed = evaluate_response(
        nadir_times,
        nadir_values,
        baseline=50.0,
        event_time=2.0,
        tolerance=0.1,
        quantity="frequency_hz",
    )
    check(
        "evaluate_response_composition",
        composed["quantity"] == "frequency_hz"
        and abs(composed["extremes"]["nadir_value"] - 49.2) < 1e-12
        and abs(composed["max_rocof"]["max_abs_rocof"] - 1.1) < 1e-12
        and composed["max_rocof"]["rocof_time"] == 0.5
        and composed["damping_ratio"] is None
        and len(composed["diagnostics"]) == 1
        and composed["settling"]["settled"] is True
        and abs(composed["settling"]["settling_time_s"] - 2.5) < 1e-12
        and composed["settling"]["last_out_of_band_time_s"] == 4.0,
        "composed doc carries extremes + ROCOF + honest damping refusal + "
        "settling; hand figures: max |ROCOF| = 1.1 Hz/s (the pre-event "
        "step at t=0.5 s is an interior sample and counts), settled 2.5 s "
        "after the event (from 4.5 s, last out-of-band sample 4.0 s)",
    )
    return results


if __name__ == "__main__":  # pragma: no cover — human-gate entrypoint
    _records = run_self_tests()
    _failures = [r for r in _records if not r["passed"]]
    for _r in _records:
        print(("PASS" if _r["passed"] else "FAIL"), _r["name"], "-", _r["detail"])
    print("%d/%d self-tests passed" % (len(_records) - len(_failures), len(_records)))
    raise SystemExit(1 if _failures else 0)
