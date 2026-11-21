// FIM-F11.05 — Rainflow pinned test suite (integration, from outside the
// crate).
//
// Every expected value in this file was computed OFFLINE with `python -I`
// against a reference implementation of the ASTM E1049-85 §5.4.4 three-point
// method that mirrors `flexcore_domain::rainflow` line for line, and the
// worked-example vectors were cross-checked against the standard's own
// worked example (reproduced by the reference `rainflow` package as its
// TEST_CASE_1/TEST_CASE_2 vectors). The tests are PINNED EXPECTATIONS, never
// runs: no build and no test execution has happened under the FIM program —
// the first human build gates everything.
//
// Pinned here:
// - the ASTM E1049 worked example (9 points, 7 counted cycles) and the
//   23-point vector (13 counted cycles), exact in f64;
// - the pinned SOC-fraction histogram for a series with known cycles
//   (half-cycle-faithful counts, provenance);
// - the degenerate-input refusals (empty, constant, non-finite, out-of-range
//   SOC) and the recorded two-sample deviation;
// - the closed-seam and method statements.

use flexcore_contracts::capability::Provenance;

use flexcore_domain::bess_degradation::RAINFLOW_EXTRACTION_SEAM;
use flexcore_domain::rainflow::{
    count_soc_series, extract_cycles, RainflowCycle, RainflowDodMeanSocHistogram, RainflowError,
    RAINFLOW_METHOD_STATEMENT,
};

fn cycles_of(series: &[f64]) -> Vec<(f64, f64, bool)> {
    extract_cycles(series)
        .unwrap()
        .into_iter()
        .map(|cycle: RainflowCycle| (cycle.dod, cycle.mean_soc, cycle.full))
        .collect()
}

// --- the ASTM E1049-85 worked example -----------------------------------

#[test]
fn the_astm_e1049_worked_example_pins_seven_cycles() {
    // The standard's own worked example (load units; the dimensionless core
    // is pinned on it). Expected (range, mean, full?) in emission order:
    // (3, -0.5, 0.5), (4, -1, 0.5), (4, 1, 1.0), (8, 1, 0.5), (9, 0.5, 0.5),
    // (8, 0, 0.5), (6, 1, 0.5) — computed offline and cross-checked against
    // the standard's published result.
    let series = [-2.0, 1.0, -3.0, 5.0, -1.0, 3.0, -4.0, 4.0, -2.0];
    assert_eq!(
        cycles_of(&series),
        vec![
            (3.0, -0.5, false),
            (4.0, -1.0, false),
            (4.0, 1.0, true),
            (8.0, 1.0, false),
            (9.0, 0.5, false),
            (8.0, 0.0, false),
            (6.0, 1.0, false),
        ]
    );
}

#[test]
fn the_twenty_three_point_vector_pins_thirteen_cycles() {
    // The second pinned E1049 vector (a 23-point series): 13 counted cycles
    // in emission order, exact in f64.
    let series = [
        -1.5, 1.0, -3.0, 10.0, -1.0, 3.0, -8.0, 4.0, -2.0, 6.0, -1.0, -4.0, -8.0, 2.0, 1.0,
        -5.0, 0.0, 2.5, -4.0, 1.0, 0.0, 2.0, -0.5,
    ];
    assert_eq!(
        cycles_of(&series),
        vec![
            (2.5, -0.25, false),
            (4.0, -1.0, false),
            (4.0, 1.0, true),
            (13.0, 3.5, false),
            (6.0, 1.0, true),
            (14.0, -1.0, true),
            (7.0, -1.5, true),
            (1.0, 0.5, true),
            (18.0, 1.0, false),
            (10.5, -2.75, false),
            (6.5, -0.75, false),
            (6.0, -1.0, false),
            (2.5, 0.75, false),
        ]
    );
}

// --- the pinned SOC histogram ---------------------------------------------

/// The pinned SOC series: 0.5 → 0.25 → 0.75 → 0.25 → 0.75 → 0.5 (exact
/// dyadic fractions). Offline computation: five HALF cycles —
/// (DOD 0.25, mean 0.375), (DOD 0.5, mean 0.5) ×3, (DOD 0.25, mean 0.625).
const PINNED_SOC_SERIES: [f64; 6] = [0.5, 0.25, 0.75, 0.25, 0.75, 0.5];

fn pinned_histogram() -> RainflowDodMeanSocHistogram {
    RainflowDodMeanSocHistogram::new(
        vec![0.0, 0.25, 0.5, 0.75, 1.0],
        vec![0.0, 0.25, 0.5, 0.75, 1.0],
    )
    .unwrap()
}

#[test]
fn a_recorded_soc_series_yields_the_pinned_cycles() {
    let cycles = count_soc_series(&PINNED_SOC_SERIES).unwrap();
    assert_eq!(cycles.len(), 5, "five half cycles, no full cycle");
    for cycle in &cycles {
        assert!(!cycle.full, "every counted cycle here is a residual half");
        assert!(cycle.dod == 0.25 || cycle.dod == 0.5);
    }
    let dod_of = |index: usize| cycles[index].dod;
    let mean_of = |index: usize| cycles[index].mean_soc;
    assert_eq!((dod_of(0), mean_of(0)), (0.25, 0.375));
    assert_eq!((dod_of(1), mean_of(1)), (0.5, 0.5));
    assert_eq!((dod_of(2), mean_of(2)), (0.5, 0.5));
    assert_eq!((dod_of(3), mean_of(3)), (0.5, 0.5));
    assert_eq!((dod_of(4), mean_of(4)), (0.25, 0.625));
}

#[test]
fn a_recorded_soc_series_yields_the_pinned_histogram() {
    let mut histogram = pinned_histogram();
    let cycles = count_soc_series(&PINNED_SOC_SERIES).unwrap();
    histogram.record_cycles(&cycles, Provenance::Measured).unwrap();
    // Bin grid: DOD edges [0, .25, .5, .75, 1] x mean edges [0, .25, .5,
    // .75, 1]; DOD 0.25 -> bin 1, DOD 0.5 -> bin 2; mean 0.375 -> bin 1,
    // mean 0.5 -> bin 2, mean 0.625 -> bin 2 (lower edge inclusive, upper
    // edge exclusive; the last bin includes its upper edge).
    let soc_bins = 4;
    let half = histogram.half_cycles();
    assert_eq!(half[1 * soc_bins + 1], 1, "(DOD 0.25, mean 0.375)");
    assert_eq!(half[2 * soc_bins + 2], 3, "(DOD 0.5, mean 0.5) x3");
    assert_eq!(half[1 * soc_bins + 2], 1, "(DOD 0.25, mean 0.625)");
    assert!(half.iter().filter(|&&count| count > 0).count() == 3);
    assert!(histogram.full_cycles().iter().all(|&count| count == 0));
    // Half-cycle equivalents: full = 2, half = 1 — the recorded convention.
    assert_eq!(
        histogram.half_cycle_equivalents()[2 * soc_bins + 2],
        3,
        "three halves are three equivalents, exactly preserved"
    );
    assert!((histogram.total_cycles() - 2.5).abs() < 1e-12);
    assert_eq!(histogram.provenance(), Provenance::Measured);
    // The per-bin cycle counts the cost-curve mapping consumes:
    // 0.5, 1.5, 0.5 cycles in the three occupied bins.
    let per_bin = histogram.cycles_per_bin();
    assert!((per_bin[1 * soc_bins + 1] - 0.5).abs() < 1e-12);
    assert!((per_bin[2 * soc_bins + 2] - 1.5).abs() < 1e-12);
    assert!((per_bin[1 * soc_bins + 2] - 0.5).abs() < 1e-12);
}

#[test]
fn histogram_provenance_merges_weakest_across_batches() {
    let mut histogram = pinned_histogram();
    let cycles = count_soc_series(&PINNED_SOC_SERIES).unwrap();
    histogram
        .record_cycles(&cycles[..2], Provenance::Measured)
        .unwrap();
    histogram
        .record_cycles(&cycles[2..], Provenance::Configured)
        .unwrap();
    // Weakest observed wins: Measured then Configured -> Configured
    // (unknown narrows, never widens — the P3.02 rule, reused).
    assert_eq!(histogram.provenance(), Provenance::Configured);
    assert_eq!(histogram.batches(), 2);
    let mut histogram = pinned_histogram();
    histogram.record_cycles(&cycles[..2], Provenance::Measured).unwrap();
    histogram.record_cycles(&cycles[2..], Provenance::Unknown).unwrap();
    assert_eq!(histogram.provenance(), Provenance::Unknown);
}

#[test]
fn histogram_edges_reuse_the_p1507_validation() {
    // The same declared-edge rules the P15.07 histogram enforces, reused via
    // the same validator (not restated): start at 0.0, end at 1.0, strictly
    // ascending, at least two edges.
    assert!(matches!(
        pinned_histogram_rejects(vec![0.0, 0.5], vec![0.2, 1.0]),
        Err(RainflowError::Histogram(_))
    ));
    assert!(matches!(
        pinned_histogram_rejects(vec![0.0, 0.5, 0.9], vec![0.0, 1.0]),
        Err(RainflowError::Histogram(_))
    ));
    assert!(matches!(
        pinned_histogram_rejects(vec![0.0, 0.5, 0.5, 1.0], vec![0.0, 1.0]),
        Err(RainflowError::Histogram(_))
    ));
}

fn pinned_histogram_rejects(
    dod_edges: Vec<f64>,
    mean_edges: Vec<f64>,
) -> Result<RainflowDodMeanSocHistogram, RainflowError> {
    RainflowDodMeanSocHistogram::new(dod_edges, mean_edges)
}

// --- degenerate inputs are refused with named errors ------------------------

#[test]
fn degenerate_series_are_refused_with_named_errors() {
    assert_eq!(extract_cycles(&[]), Err(RainflowError::EmptySeries));
    assert_eq!(
        extract_cycles(&[0.5]),
        Err(RainflowError::DegenerateSeries { samples: 1 })
    );
    assert_eq!(
        count_soc_series(&[0.5, 0.5, 0.5, 0.5]),
        Err(RainflowError::DegenerateSeries { samples: 4 })
    );
    assert!(matches!(
        extract_cycles(&[0.5, f64::NAN, 0.25]),
        Err(RainflowError::NonFiniteSample { index: 1, .. })
    ));
    // SOC fractions outside [0, 1] are refused by the SOC entry point (the
    // canonical-units freeze), by index.
    assert!(matches!(
        count_soc_series(&[0.5, 1.5]),
        Err(RainflowError::SocOutOfRange { index: 1, .. })
    ));
    assert!(matches!(
        count_soc_series(&[-0.1, 0.5]),
        Err(RainflowError::SocOutOfRange { index: 0, .. })
    ));
}

#[test]
fn the_two_sample_case_counts_one_half_cycle_the_recorded_deviation() {
    // The recorded convention (see the module header): a series of exactly
    // two distinct samples counts the single movement as one HALF cycle —
    // a counted discharge episode is never silently lost.
    let cycles = count_soc_series(&[0.8, 0.3]).unwrap();
    assert_eq!(cycles.len(), 1);
    assert!((cycles[0].dod - 0.5).abs() < 1e-12);
    assert!((cycles[0].mean_soc - 0.55).abs() < 1e-12);
    assert!(!cycles[0].full);
    // A constant two-sample series still refuses: no movement, no cycle.
    assert_eq!(
        count_soc_series(&[0.5, 0.5]),
        Err(RainflowError::DegenerateSeries { samples: 2 })
    );
}

// --- the recorded statements -------------------------------------------------

#[test]
fn the_closed_seam_and_method_statements_are_recorded_honestly() {
    // The P15.07 seam is CLOSED: the extraction is implemented, written-
    // unrun; the histogram still accepts caller-counted cycles.
    assert!(RAINFLOW_EXTRACTION_SEAM.contains("CLOSED"));
    assert!(RAINFLOW_EXTRACTION_SEAM.contains("IS NOW IMPLEMENTED"));
    assert!(RAINFLOW_EXTRACTION_SEAM.contains("written-unrun"));
    assert!(RAINFLOW_EXTRACTION_SEAM.contains("E1049"));
    assert!(RAINFLOW_METHOD_STATEMENT.contains("E1049"));
    assert!(RAINFLOW_METHOD_STATEMENT.contains("WRITTEN-UNRUN"));
    assert!(RAINFLOW_METHOD_STATEMENT.contains("half cycle"));
    assert!(RAINFLOW_METHOD_STATEMENT.contains("never authorizes or produces a dispatch command"));
}
