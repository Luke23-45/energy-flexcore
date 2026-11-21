// FIM-F11 (Wave 5) — Rainflow cycle counting per ASTM E1049-85 §5.4.4 (the
// three-point method) over SOC fraction series (pure domain computation, no
// I/O).
//
// WHAT THIS MODULE IS: the closing of the P15.07 RECORDED SEAM
// (`crate::bess_degradation::RAINFLOW_EXTRACTION_SEAM`): an SOC time series is
// turned into (DOD, mean-SOC) cycles — full cycles and residual half cycles,
// each with range and mean — and accumulated into a DOD × mean-SOC histogram
// in the exact shape of the P15.07 `DodMeanSocHistogram` (same edge
// validation, same bin resolution, reused — not reimplemented).
//
// IMPLEMENTED-UNDER-FIM-F11, WRITTEN-UNRUN: this module was written under the
// Final Implementation Milestone (Wave 5, feature F-11) and has never been
// compiled or executed. The first human build gates it; the pinned vectors in
// `tests/bess_fim_rainflow.rs` are python-computed expectations, never runs.
//
// THE METHOD, EXACTLY AS PINNED (ASTM E1049-85 §5.4.4, three-point rule):
//
// 1. **Reversal (turning-point) extraction**: the first and the last samples
//    of the series are turning points (the reversal is undefined at the ends
//    and they are treated as reversals, per the standard's reversal
//    definition); constant repeats are dropped; a sample between two
//    same-sign increments (colinear) is dropped; a sample is a turning point
//    exactly when the increment sign flips across it.
// 2. **Three-point rule** over a stack of turning points: with X the range
//    of the two most recent points and Y the range of the two before them —
//    X < Y: read the next point; X ≥ Y with Y containing the stack's first
//    point: count Y as one HALF cycle and discard the stack's first point;
//    X ≥ Y otherwise: count Y as one FULL cycle and discard the peak and
//    valley of Y. Cycle range = |a − b|, cycle mean = (a + b)/2.
// 3. **Residue**: after the last point, every remaining consecutive pair in
//    the stack is counted as one HALF cycle (range + mean).
//
// DECLARED CONVENTIONS (each decided once, recorded here — not physics):
//
// 1. **The two-sample case is a recorded DEVIATION from common reference
//    implementations**: reference implementations of the standard drop the
//    final turning point for a series of exactly two distinct samples and
//    would count nothing. This module counts the single movement as one HALF
//    cycle — the standard's reversal definition treats the first and last
//    points as reversals, and the residue rule then counts the remaining
//    pair. Recorded so a counted discharge episode is never silently lost.
// 2. **Constant/degenerate inputs are REFUSED with named errors** (chosen
//    over the alternative): an empty series, a single sample, and a constant
//    series carry no cycle information — a silent zero would be
//    indistinguishable from "no data" (P3.02 posture). Non-finite samples
//    are refused by index; SOC samples outside [0, 1] are refused by the
//    SOC entry point (the canonical-units freeze, §10.2 — SOC is a fraction
//    in [0, 1] at the API boundary).
// 3. **Half cycles are stored as HALF-CYCLE EQUIVALENTS** (a full cycle = 2,
//    a half cycle = 1) so the standard's half-cycle information is preserved
//    exactly in integer counts; `cycles_per_bin` converts back to cycles.
//    The P15.07 `DodMeanSocHistogram` continues to store whole counted
//    cycles only (its recorded semantics are unchanged); this module adds
//    the half-cycle-faithful accumulator alongside it. No lossy conversion
//    into the whole-cycle histogram is provided.
// 4. **DOD of a cycle is its range** over the SOC fraction series (the depth
//    of the cycle, a fraction in (0, 1]); **mean SOC is (a + b)/2**. The
//    extraction arithmetic itself is dimensionless — the E1049 worked
//    example is pinned on raw load units — and the SOC entry point applies
//    the units validation.
//
// PROVENANCE (P3.02, reused — never reimplemented): the caller DECLARES the
// provenance of the input series (typically `Measured` for telemetry); the
// extraction is a deterministic declared transform that adds no new
// knowledge class, so the histogram's provenance is the declared input
// provenance, merged weakest-wins across batches (unknown narrows, never
// widens).
//
// ADR-010 (conventions §6.3): a rainflow histogram is INPUT DATA and a
// RECORD. It never authorizes or produces a dispatch command.
//
// UNITS (canonical freeze untouched): SOC fractions in [0, 1], DOD a
// fraction in (0, 1]; no energies appear here — the MWh-equivalent cost
// mapping lives in `crate::bess_cost_curve`.
//
// Control status: pure domain computation for the simulation-only 0.x line.
// No I/O, no solver coupling, no device-write path, no dispatch reach.

use flexcore_contracts::capability::Provenance;

use crate::bess_degradation::{bin_index, validate_edges, DegradationLedgerError};

// --- Recorded statements -----------------------------------------------------

/// The recorded method statement (visible in records and exports, not a
/// comment): the E1049 three-point method, its conventions, and its
/// written-unrun status.
pub const RAINFLOW_METHOD_STATEMENT: &str =
    "Rainflow counting per ASTM E1049-85 section 5.4.4 (three-point method): \
turning-point extraction (first and last samples are turning points; \
constant repeats and colinear samples dropped), the three-point rule for \
full and half cycles, and the residue rule (remaining pairs counted as half \
cycles). Implemented under FIM-F11, WRITTEN-UNRUN: never compiled, never \
executed; the first human build gates it. The two-sample case counts one \
half cycle (recorded deviation from reference implementations, recorded so \
a counted discharge is never silently lost). A rainflow histogram is INPUT \
DATA and a RECORD; it never authorizes or produces a dispatch command \
(ADR-010).";

// --- Errors ------------------------------------------------------------------

/// Errors of rainflow counting and histogram accumulation. Each variant
/// names the offending field/value — never a bare "invalid input" (P3.04
/// error-catalogue style).
#[derive(Debug, Clone, PartialEq)]
pub enum RainflowError {
    /// The series was empty: there are no samples to count.
    EmptySeries,
    /// The series carried fewer than two distinct samples (constant or
    /// single-sample): no cycle information exists and nothing is invented.
    DegenerateSeries {
        /// The number of samples offered.
        samples: usize,
    },
    /// A sample was NaN or infinite.
    NonFiniteSample {
        /// The sample index (0-based).
        index: usize,
        /// The offending value.
        value: f64,
    },
    /// A SOC sample was outside the canonical fraction range [0, 1]
    /// (the units freeze, §10.2 — applied by the SOC entry point).
    SocOutOfRange {
        /// The sample index (0-based).
        index: usize,
        /// The offending value.
        value: f64,
    },
    /// A histogram error from the reused P15.07 edge/bin machinery.
    Histogram(DegradationLedgerError),
}

impl std::fmt::Display for RainflowError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RainflowError::EmptySeries => write!(f, "the SOC series is empty; there is nothing to count"),
            RainflowError::DegenerateSeries { samples } => write!(
                f,
                "the series carries {samples} sample(s) with fewer than two distinct \
                 values: no cycle information exists and none is invented (P3.02)"
            ),
            RainflowError::NonFiniteSample { index, value } => {
                write!(f, "sample[{index}] must be finite (found {value})")
            }
            RainflowError::SocOutOfRange { index, value } => write!(
                f,
                "sample[{index}] must be an SOC fraction in [0,1] (found {value})"
            ),
            RainflowError::Histogram(error) => write!(f, "rainflow histogram error: {error}"),
        }
    }
}

impl std::error::Error for RainflowError {}

// --- Cycles ------------------------------------------------------------------

/// One rainflow-counted cycle: its depth of discharge (the range, a fraction
/// in (0, 1] for SOC series), its mean value, and whether the three-point
/// rule counted it as a FULL cycle (count 1.0) or a HALF cycle (count 0.5).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RainflowCycle {
    /// The cycle depth (the range |a − b|; a fraction in (0, 1] for SOC
    /// series).
    pub dod: f64,
    /// The cycle mean ((a + b)/2; a fraction in [0, 1] for SOC series).
    pub mean_soc: f64,
    /// True when the three-point rule counted a FULL cycle; false for a
    /// residual HALF cycle.
    pub full: bool,
}

impl RainflowCycle {
    /// The cycle's contribution to a half-cycle-equivalent count (a full
    /// cycle = 2, a half cycle = 1 — the recorded convention).
    pub fn half_cycle_equivalents(&self) -> u64 {
        if self.full {
            2
        } else {
            1
        }
    }

    /// The cycle's contribution to a cycle count (1.0 full, 0.5 half).
    pub fn count(&self) -> f64 {
        if self.full {
            1.0
        } else {
            0.5
        }
    }
}

/// Validates every sample as finite; returns the named error by index
/// otherwise.
fn validate_finite_samples(series: &[f64]) -> Result<(), RainflowError> {
    for (index, value) in series.iter().enumerate() {
        if !value.is_finite() {
            return Err(RainflowError::NonFiniteSample { index, value: *value });
        }
    }
    Ok(())
}

/// Extracts the turning points (reversals) of the series per the recorded
/// convention: the first and last samples are turning points; constant
/// repeats are dropped; colinear samples are dropped; a sign flip of the
/// increment marks a turning point. Consecutive turning points are always
/// distinct, so every extracted range is strictly positive.
fn turning_points(series: &[f64]) -> Vec<f64> {
    let mut points = vec![series[0]];
    if series.len() == 1 {
        return points;
    }
    let mut previous_delta = series[1] - series[0];
    for index in 2..series.len() {
        if series[index] == series[index - 1] {
            // Constant repeat: no information, dropped (recorded convention).
            continue;
        }
        let delta = series[index] - series[index - 1];
        if previous_delta * delta < 0.0 {
            points.push(series[index - 1]);
        }
        previous_delta = delta;
    }
    // The last sample is a turning point; push it only if it is distinct
    // from the last pushed point (a constant tail adds no movement).
    let last = series[series.len() - 1];
    if last != points[points.len() - 1] {
        points.push(last);
    }
    points
}

/// The ASTM E1049-85 §5.4.4 three-point rule and residue rule over
/// pre-extracted turning points.
fn count_turning_points(points: &[f64]) -> Vec<RainflowCycle> {
    let mut stack: Vec<f64> = Vec::with_capacity(points.len());
    let mut cycles = Vec::new();
    for &point in points {
        stack.push(point);
        while stack.len() >= 3 {
            let x1 = stack[stack.len() - 3];
            let x2 = stack[stack.len() - 2];
            let x3 = stack[stack.len() - 1];
            let x_range = (x3 - x2).abs();
            let y_range = (x2 - x1).abs();
            if x_range < y_range {
                // Read the next point.
                break;
            } else if stack.len() == 3 {
                // Y contains the stack's first point: count it as one HALF
                // cycle and discard the first point.
                cycles.push(RainflowCycle {
                    dod: (stack[0] - stack[1]).abs(),
                    mean_soc: 0.5 * (stack[0] + stack[1]),
                    full: false,
                });
                stack.remove(0);
            } else {
                // Count Y as one FULL cycle and discard the peak and valley
                // of Y.
                cycles.push(RainflowCycle {
                    dod: (x1 - x2).abs(),
                    mean_soc: 0.5 * (x1 + x2),
                    full: true,
                });
                let last = stack.pop().expect("stack length checked >= 3");
                stack.pop();
                stack.pop();
                stack.push(last);
            }
        }
    }
    // Residue: every remaining consecutive pair is one HALF cycle.
    while stack.len() > 1 {
        cycles.push(RainflowCycle {
            dod: (stack[0] - stack[1]).abs(),
            mean_soc: 0.5 * (stack[0] + stack[1]),
            full: false,
        });
        stack.remove(0);
    }
    cycles
}

/// The dimensionless E1049 three-point core over a raw f64 series (the
/// pinned ASTM worked example runs on raw load units). Refuses empty,
/// single-sample, and constant series with named errors; refuses non-finite
/// samples by index. The SOC entry point ([`count_soc_series`]) applies the
/// canonical-units validation on top of this core.
pub fn extract_cycles(series: &[f64]) -> Result<Vec<RainflowCycle>, RainflowError> {
    validate_finite_samples(series)?;
    if series.is_empty() {
        return Err(RainflowError::EmptySeries);
    }
    let points = turning_points(series);
    if points.len() < 2 {
        return Err(RainflowError::DegenerateSeries { samples: series.len() });
    }
    Ok(count_turning_points(&points))
}

/// Counts an SOC fraction series (fractions in [0, 1] at the API boundary —
/// the canonical-units freeze, §10.2) into rainflow cycles: full cycles and
/// residual half cycles, each with DOD (the range) and mean SOC. This is the
/// recorded closure of the P15.07 `RAINFLOW_EXTRACTION_SEAM`
/// ([`crate::bess_degradation::RAINFLOW_EXTRACTION_SEAM`]).
pub fn count_soc_series(series: &[f64]) -> Result<Vec<RainflowCycle>, RainflowError> {
    for (index, value) in series.iter().enumerate() {
        if !(0.0..=1.0).contains(value) {
            return Err(RainflowError::SocOutOfRange { index, value: *value });
        }
    }
    extract_cycles(series)
}

// --- The DOD x mean-SOC histogram (half-cycle-faithful) ------------------------

/// The rainflow-fed DOD × mean-SOC histogram (FIM-F11): the accumulator for
/// EXTRACTED cycles, in the exact shape of the P15.07
/// `DodMeanSocHistogram` (same edge validation, same lower-edge-inclusive /
/// upper-edge-exclusive bin resolution — both reused, not reimplemented).
///
/// The recorded difference to the P15.07 type: this accumulator stores FULL
/// and HALF cycle counts in separate planes, so the standard's half-cycle
/// information is preserved exactly. The `cycles_per_bin` accessor exposes
/// per-bin cycle counts as f64 (half-cycle-equivalents / 2.0) for the
/// cost-curve mapping in `crate::bess_cost_curve`. No lossy conversion into
/// the whole-cycle P15.07 histogram is provided.
#[derive(Debug, Clone, PartialEq)]
pub struct RainflowDodMeanSocHistogram {
    dod_bin_edges: Vec<f64>,
    mean_soc_bin_edges: Vec<f64>,
    full_cycles: Vec<u64>,
    half_cycles: Vec<u64>,
    provenance: Provenance,
    batches: u64,
}

impl RainflowDodMeanSocHistogram {
    /// Constructs a histogram from declared, strictly ascending bin edges in
    /// [0, 1] (both tables start at 0.0 and end at 1.0 — the same coverage
    /// rule the P15.07 histogram enforces, reused via the same validator).
    pub fn new(
        dod_bin_edges: Vec<f64>,
        mean_soc_bin_edges: Vec<f64>,
    ) -> Result<Self, RainflowError> {
        validate_edges(
            &dod_bin_edges,
            "dod_bin_edges",
            true,
            "the first DOD edge must be 0.0 (a zero-depth cycle is not a cycle)",
        )
        .map_err(RainflowError::Histogram)?;
        validate_edges(
            &mean_soc_bin_edges,
            "mean_soc_bin_edges",
            true,
            "the first mean-SOC edge must be 0.0 so every value in [0,1] resolves to exactly one bin",
        )
        .map_err(RainflowError::Histogram)?;
        let dod_bins = dod_bin_edges.len() - 1;
        let soc_bins = mean_soc_bin_edges.len() - 1;
        Ok(Self {
            dod_bin_edges,
            mean_soc_bin_edges,
            full_cycles: vec![0u64; dod_bins * soc_bins],
            half_cycles: vec![0u64; dod_bins * soc_bins],
            provenance: Provenance::Unknown,
            batches: 0,
        })
    }

    /// Records a batch of extracted cycles with the series' DECLARED
    /// provenance. The provenance merges weakest-wins across batches
    /// (unknown narrows, never widens; the first batch sets it).
    pub fn record_cycles(
        &mut self,
        cycles: &[RainflowCycle],
        provenance: Provenance,
    ) -> Result<(), RainflowError> {
        for cycle in cycles {
            self.record_cycle(*cycle, provenance)?;
        }
        Ok(())
    }

    /// Records one extracted cycle (full or half) into its (DOD, mean-SOC)
    /// bin. Values are re-validated here (named errors) — a hand-constructed
    /// cycle never enters the accumulator unvalidated.
    pub fn record_cycle(
        &mut self,
        cycle: RainflowCycle,
        provenance: Provenance,
    ) -> Result<(), RainflowError> {
        if !cycle.dod.is_finite() {
            return Err(RainflowError::Histogram(DegradationLedgerError::NonFiniteValue {
                field: "dod",
                value: cycle.dod,
            }));
        }
        if cycle.dod <= 0.0 || cycle.dod > 1.0 {
            return Err(RainflowError::Histogram(DegradationLedgerError::ValueOutOfRange {
                field: "dod",
                value: cycle.dod,
                lower: f64::EPSILON,
                upper: 1.0,
            }));
        }
        if !cycle.mean_soc.is_finite() {
            return Err(RainflowError::Histogram(
                DegradationLedgerError::NonFiniteValue {
                    field: "mean_soc",
                    value: cycle.mean_soc,
                },
            ));
        }
        if !(0.0..=1.0).contains(&cycle.mean_soc) {
            return Err(RainflowError::Histogram(DegradationLedgerError::ValueOutOfRange {
                field: "mean_soc",
                value: cycle.mean_soc,
                lower: 0.0,
                upper: 1.0,
            }));
        }
        let dod_bin = bin_index(&self.dod_bin_edges, cycle.dod);
        let soc_bin = bin_index(&self.mean_soc_bin_edges, cycle.mean_soc);
        let soc_bins = self.mean_soc_bin_edges.len() - 1;
        let index = dod_bin * soc_bins + soc_bin;
        if cycle.full {
            self.full_cycles[index] += 1;
        } else {
            self.half_cycles[index] += 1;
        }
        // As in the P15.07 accumulators: a fresh histogram's UNKNOWN means
        // "no batches yet"; the first batch sets the provenance, later
        // batches merge weakest-wins.
        if self.batches == 0 {
            self.provenance = provenance;
        } else {
            self.provenance =
                crate::bess_degradation::weaker_provenance(self.provenance, provenance);
        }
        self.batches += 1;
        Ok(())
    }

    /// The declared DOD bin edges.
    pub fn dod_bin_edges(&self) -> &[f64] {
        &self.dod_bin_edges
    }

    /// The declared mean-SOC bin edges.
    pub fn mean_soc_bin_edges(&self) -> &[f64] {
        &self.mean_soc_bin_edges
    }

    /// The FULL cycle counts, row-major: `dod_bin * mean_soc_bins + soc_bin`.
    pub fn full_cycles(&self) -> &[u64] {
        &self.full_cycles
    }

    /// The HALF cycle counts, row-major (same indexing).
    pub fn half_cycles(&self) -> &[u64] {
        &self.half_cycles
    }

    /// The half-cycle-equivalent counts per bin (full = 2, half = 1),
    /// row-major — the recorded convention that keeps the standard's
    /// half-cycle information exact in integer counts.
    pub fn half_cycle_equivalents(&self) -> Vec<u64> {
        self.full_cycles
            .iter()
            .zip(self.half_cycles.iter())
            .map(|(&full, &half)| full * 2 + half)
            .collect()
    }

    /// The per-bin cycle counts as f64 (half-cycle-equivalents / 2.0),
    /// row-major — the input shape the cost-curve mapping consumes.
    pub fn cycles_per_bin(&self) -> Vec<f64> {
        self.half_cycle_equivalents()
            .into_iter()
            .map(|equivalents| equivalents as f64 / 2.0)
            .collect()
    }

    /// The merged provenance (the weakest declared batch provenance).
    pub fn provenance(&self) -> Provenance {
        self.provenance
    }

    /// The number of recorded batches.
    pub fn batches(&self) -> u64 {
        self.batches
    }

    /// The total cycle count over all bins (full cycles + half cycles / 2).
    pub fn total_cycles(&self) -> f64 {
        self.full_cycles.iter().sum::<u64>() as f64
            + self.half_cycles.iter().sum::<u64>() as f64 / 2.0
    }
}
