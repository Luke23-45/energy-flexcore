// FIM-F11.05 — SoH estimator and measured-counter warranty headroom pinned
// test suite (integration, from outside the crate).
//
// Every expected value in this file was computed OFFLINE with `python -I`
// (exact arithmetic; the pinned SoH fractions and counter values are
// dyadic). The tests are PINNED EXPECTATIONS, never runs: no build and no
// test execution has happened under the FIM program — the first human build
// gates everything.
//
// Pinned here (the FIM-F11.05 list):
// - the weakest-provenance merge on the SoH estimator (Measured then
//   Configured -> Configured; an UNKNOWN input is excluded and recorded);
// - the estimate's staleness bound (fresh at age == max_age, stale beyond —
//   the P2.08 convention);
// - BMS-override disagreement recording: the reported value stays
//   authoritative and the divergence beyond tolerance becomes DATA;
// - headroom monotonicity: more measured throughput leaves no more
//   headroom, and an exceeded allowance is a record, never a penalty;
// - the P15-E-3 out-of-scope chemistry refusal, unchanged;
// - the recorded FIM-F11 statements.

use flexcore_contracts::capability::Provenance;

use flexcore_domain::bess_degradation::{
    DegradationChemistry, MonotonicCounter, SohObservation,
};
use flexcore_domain::bess_soh::{
    estimate_soh, evaluate_soh_estimate_freshness, SohAuthority, SohEstimationConfig,
    SohFreshness, SohLedgerCounters, SohStalenessConfig, DEFAULT_SOH_DISAGREEMENT_TOLERANCE_FRACTION,
    ESTIMATED_RETENTION_HEADROOM_STATEMENT, MEASURED_COUNTER_HEADROOM_STATEMENT,
    SOH_AUTHORITY_RULE_STATEMENT, SOH_DISAGREEMENT_STATEMENT, SOH_ESTIMATION_DEFERRAL_STATEMENT,
};
use flexcore_domain::bess_warranty::{
    compute_measured_counter_headroom, counter_headroom, estimated_capacity_retention_headroom,
    CapacityRetentionPoint, HeadroomStatus, WarrantyTerms,
};

const DAY: i64 = 24 * 3_600_000;

fn observation(soh: f64, at_ms: i64, provenance: Provenance) -> SohObservation {
    SohObservation::new(soh, at_ms, provenance, "bms-7".to_string()).unwrap()
}

fn measured_counter(value: f64, provenance: Provenance) -> MonotonicCounter {
    let mut counter = MonotonicCounter::new();
    counter.record(value, provenance).unwrap();
    counter
}

fn terms() -> WarrantyTerms {
    WarrantyTerms::new(
        "warranty-fim-f11-001".to_string(),
        0,
        10 * 365 * 24 * 3_600_000 + 6 * 3_600_000,
        vec![CapacityRetentionPoint {
            year_index: 1,
            min_retention_fraction: 0.96,
        }],
        Some(6_000.0),
        Some(60_000.0),
        None,
        None,
        "declared warranty citation".to_string(),
        Provenance::Configured,
    )
    .unwrap()
}

// --- the weakest-provenance merge on the estimator ---------------------------

#[test]
fn the_estimator_merges_the_weakest_credited_provenance() {
    let staleness = SohStalenessConfig::new(10_000).unwrap();
    let config = SohEstimationConfig::default();
    let observations = [
        observation(0.96, 1_000, Provenance::Measured),
        observation(0.94, 2_000, Provenance::Configured),
    ];
    let outcome = estimate_soh(&observations, None, 5_000, &staleness, &config).unwrap();
    let estimate = outcome.estimate.expect("credited inputs exist");
    // The equal-weight mean over the credited inputs: (0.96 + 0.94) / 2.
    assert!((estimate.soh_fraction - 0.95).abs() < 1e-12);
    // Weakest observed wins: Measured then Configured -> Configured
    // (the P3.02 rule, reused — unknown narrows, never widens).
    assert_eq!(estimate.provenance, Provenance::Configured);
    assert_eq!(estimate.input_observations, 2);
    assert_eq!(estimate.newest_input_event_time_unix_ms, 2_000);
}

#[test]
fn an_unknown_provenance_observation_is_excluded_and_recorded_never_credited() {
    let staleness = SohStalenessConfig::new(10_000).unwrap();
    let config = SohEstimationConfig::default();
    let observations = [
        observation(0.96, 1_000, Provenance::Measured),
        observation(0.10, 2_000, Provenance::Unknown),
    ];
    let outcome = estimate_soh(&observations, None, 5_000, &staleness, &config).unwrap();
    assert_eq!(outcome.unknown_provenance_inputs_excluded, 1);
    let estimate = outcome.estimate.expect("one credited input");
    // The UNKNOWN input never entered the arithmetic (0.10 excluded).
    assert!((estimate.soh_fraction - 0.96).abs() < 1e-12);
    assert_eq!(estimate.provenance, Provenance::Measured);
    // Unknown-only input sets credit nothing and invent nothing.
    let only_unknown = [observation(0.96, 1_000, Provenance::Unknown)];
    let outcome = estimate_soh(&only_unknown, None, 5_000, &staleness, &config).unwrap();
    assert!(matches!(outcome.authority, SohAuthority::NotCreditable { .. }));
    assert!(outcome.estimate.is_none());
}

#[test]
fn the_estimate_staleness_bound_follows_its_newest_input() {
    let staleness = SohStalenessConfig::new(1_000).unwrap();
    let config = SohEstimationConfig::default();
    let observations = [
        observation(0.96, 1_000, Provenance::Measured),
        observation(0.94, 2_000, Provenance::Measured),
    ];
    let outcome = estimate_soh(&observations, None, 5_000, &staleness, &config).unwrap();
    let estimate = outcome.estimate.unwrap();
    // age == max_age exactly is NOT stale (the P2.08 convention).
    assert_eq!(
        evaluate_soh_estimate_freshness(&estimate, 3_000, &staleness),
        SohFreshness::Fresh
    );
    assert!(matches!(
        evaluate_soh_estimate_freshness(&estimate, 3_001, &staleness),
        SohFreshness::Stale { .. }
    ));
}

// --- the BMS-override disagreement recording -----------------------------------

#[test]
fn the_reported_soh_stays_authoritative_and_the_disagreement_is_recorded() {
    // Pinned: reported 0.90 (freshest), older 0.96, estimate = 0.93,
    // delta = 0.03 > the 0.01 default tolerance -> a disagreement RECORD.
    let staleness = SohStalenessConfig::new(10_000).unwrap();
    let config = SohEstimationConfig::default();
    assert!(
        (DEFAULT_SOH_DISAGREEMENT_TOLERANCE_FRACTION - 0.01).abs() < 1e-12,
        "the recorded default tolerance is pinned"
    );
    let observations = [
        observation(0.96, 1_000, Provenance::Measured),
        observation(0.90, 2_000, Provenance::Measured),
    ];
    let outcome = estimate_soh(&observations, None, 5_000, &staleness, &config).unwrap();
    match &outcome.authority {
        SohAuthority::Reported {
            soh_fraction,
            provenance,
            event_time_unix_ms,
            source,
        } => {
            assert!((soh_fraction - 0.90).abs() < 1e-12, "the reported value is operative");
            assert_eq!(*provenance, Provenance::Measured);
            assert_eq!(*event_time_unix_ms, 2_000);
            assert_eq!(source, "bms-7");
        }
        other => panic!("expected the reported authority, got {other:?}"),
    }
    let record = outcome.disagreement.expect("the divergence becomes DATA");
    assert!((record.reported_soh_fraction - 0.90).abs() < 1e-12);
    assert!((record.estimated_soh_fraction - 0.93).abs() < 1e-12);
    assert!((record.delta_fraction - 0.03).abs() < 1e-12);
    assert!((record.tolerance_fraction - 0.01).abs() < 1e-12);
    assert_eq!(record.recorded_at_unix_ms, 5_000);
    assert!(record.note.contains("remains the authority"));
}

#[test]
fn agreement_within_tolerance_records_no_disagreement() {
    let staleness = SohStalenessConfig::new(10_000).unwrap();
    let config = SohEstimationConfig::default();
    let observations = [
        observation(0.94, 1_000, Provenance::Measured),
        observation(0.945, 2_000, Provenance::Measured),
    ];
    let outcome = estimate_soh(&observations, None, 5_000, &staleness, &config).unwrap();
    // Estimate 0.9425 vs the freshest reported 0.945: delta 0.0025 within
    // the 0.01 tolerance -> no disagreement record exists.
    assert!(outcome.disagreement.is_none());
}

#[test]
fn ledger_counters_travel_as_recorded_context_and_never_enter_the_arithmetic() {
    let staleness = SohStalenessConfig::new(10_000).unwrap();
    let config = SohEstimationConfig::default();
    let observations = [observation(0.96, 1_000, Provenance::Measured)];
    let counters = SohLedgerCounters {
        full_equivalent_cycles: 1_234.5,
        full_equivalent_cycles_provenance: Provenance::Measured,
        cumulative_throughput_mwh: 24_690.0,
        cumulative_throughput_mwh_provenance: Provenance::Measured,
    };
    let outcome =
        estimate_soh(&observations, Some(counters), 5_000, &staleness, &config).unwrap();
    let recorded = outcome.ledger_counters_recorded.expect("context travels");
    assert!((recorded.full_equivalent_cycles - 1_234.5).abs() < 1e-9);
    assert!((recorded.cumulative_throughput_mwh - 24_690.0).abs() < 1e-9);
    // No aging law exists: the counters change nothing in the estimate.
    let estimate = outcome.estimate.unwrap();
    assert!((estimate.soh_fraction - 0.96).abs() < 1e-12);
}

// --- headroom monotonicity from measured counters ------------------------------

#[test]
fn headroom_falls_monotonically_as_measured_counters_consume_the_allowance() {
    // FIM-F11.04 acceptance: headroom computed from measured counters falls
    // as throughput consumes the allowance; an exceeded allowance is a
    // record, never a penalty.
    let mut consumed = MonotonicCounter::new();
    consumed.record(20_000.0, Provenance::Measured).unwrap();
    let first = counter_headroom(60_000.0, &consumed, "MWh");
    match &first {
        HeadroomStatus::Within { remaining, unit } => {
            assert_eq!(*unit, "MWh");
            assert!((remaining - 40_000.0).abs() < 1e-9);
        }
        other => panic!("expected Within(MWh), got {other:?}"),
    }
    consumed.record(20_000.0, Provenance::Measured).unwrap();
    let second = counter_headroom(60_000.0, &consumed, "MWh");
    match &second {
        HeadroomStatus::Within { remaining, .. } => {
            assert!((remaining - 20_000.0).abs() < 1e-9);
            assert!(remaining < 40_000.0, "headroom never grows with consumption");
        }
        other => panic!("expected Within(MWh), got {other:?}"),
    }
    consumed.record(30_000.0, Provenance::Measured).unwrap();
    let third = counter_headroom(60_000.0, &consumed, "MWh");
    match &third {
        HeadroomStatus::Exceeded { overage, unit } => {
            assert_eq!(*unit, "MWh");
            assert!((overage - 10_000.0).abs() < 1e-9);
        }
        other => panic!("expected Exceeded(MWh), got {other:?}"),
    }
}

#[test]
fn the_measured_counter_report_records_not_configured_and_the_unknown_refusal() {
    let cycles = measured_counter(1_000.0, Provenance::Measured);
    let throughput = measured_counter(20_000.0, Provenance::Measured);
    let report =
        compute_measured_counter_headroom(Some(6_000.0), Some(60_000.0), &cycles, &throughput)
            .unwrap();
    assert!(report.counters_creditable);
    assert!(matches!(report.cycles, HeadroomStatus::Within { .. }));
    assert!(matches!(report.throughput, HeadroomStatus::Within { .. }));
    // A dimension without a declared limit is NotConfigured.
    let partial =
        compute_measured_counter_headroom(None, Some(60_000.0), &cycles, &throughput).unwrap();
    assert_eq!(partial.cycles, HeadroomStatus::NotConfigured);
    // UNKNOWN-provenance counters credit nothing (P3.02).
    let unknown = measured_counter(1_000.0, Provenance::Unknown);
    let report = compute_measured_counter_headroom(
        Some(6_000.0),
        Some(60_000.0),
        &unknown,
        &throughput,
    )
    .unwrap();
    assert!(matches!(report.cycles, HeadroomStatus::NotCreditable { .. }));
    assert!(!report.counters_creditable);
    assert!(report.statement.contains("falls monotonically"));
    assert!(MEASURED_COUNTER_HEADROOM_STATEMENT.contains("never a penalty, a claim, or a dispatch input"));
}

// --- the estimate-based retention record ----------------------------------------

#[test]
fn the_estimate_based_retention_record_is_advisory_and_pinned() {
    // Evaluated at day 30 (inside warranty year 1): the year-1 point (0.96)
    // applies; the estimate 0.94 gives margin -0.02 — a recorded
    // below-schedule value, never a claim.
    let staleness = SohStalenessConfig::default();
    let config = SohEstimationConfig::default();
    let observations = [
        observation(0.96, DAY, Provenance::Measured),
        observation(0.92, 2 * DAY, Provenance::Measured),
    ];
    let outcome = estimate_soh(&observations, None, 2 * DAY, &staleness, &config).unwrap();
    let estimate = outcome.estimate.unwrap();
    assert!((estimate.soh_fraction - 0.94).abs() < 1e-12);
    let record = estimated_capacity_retention_headroom(
        &terms(),
        &estimate,
        30 * DAY,
        &SohStalenessConfig::default(),
    )
    .unwrap();
    assert!(record.estimate_is_advisory, "the estimate never overrides a report");
    assert_eq!(record.estimate_provenance, Provenance::Measured);
    assert_eq!(record.detail.year_index, 1);
    assert!((record.detail.scheduled_minimum_fraction - 0.96).abs() < 1e-12);
    assert!((record.detail.observed_soh_fraction - 0.94).abs() < 1e-12);
    assert!((record.detail.margin_fraction - (0.94 - 0.96)).abs() < 1e-12);
    assert!(!record.detail.beyond_schedule_coverage);
    assert!(ESTIMATED_RETENTION_HEADROOM_STATEMENT.contains("never overrides"));
}

#[test]
fn the_estimate_based_retention_record_refuses_a_stale_estimate() {
    // The estimate's newest input is 30 days old at evaluation time: stale
    // beyond the 7-day default bound -> refused, never invented.
    let staleness = SohStalenessConfig::default();
    let config = SohEstimationConfig::default();
    let observations = [observation(0.94, DAY, Provenance::Measured)];
    let outcome = estimate_soh(&observations, None, DAY, &staleness, &config).unwrap();
    let estimate = outcome.estimate.unwrap();
    assert!(matches!(
        estimated_capacity_retention_headroom(
            &terms(),
            &estimate,
            30 * DAY,
            &SohStalenessConfig::default(),
        ),
        Err(flexcore_domain::bess_warranty::WarrantyError::EstimateNotCreditable { .. })
    ));
}

// --- the unchanged P15-E-3 refusal and the recorded statements -------------------

#[test]
fn the_out_of_scope_chemistry_refusal_is_unchanged() {
    // The recorded limitation is unchanged: no degradation parameter is
    // credited for an out-of-scope chemistry anywhere downstream.
    let chemistry = DegradationChemistry::OutOfScope { name: "LTO".to_string() };
    assert!(!chemistry.is_in_scope());
    assert_eq!(chemistry.label(), "OUT_OF_SCOPE(LTO)");
}

#[test]
fn the_recorded_fim_f11_statements_carry_the_honesty_lines() {
    // The updated deferral statement records the supersession honestly:
    // implemented, written-unrun, calibration a HUMAN LAB GATE.
    assert!(SOH_ESTIMATION_DEFERRAL_STATEMENT.contains("DEFERRED"));
    assert!(SOH_ESTIMATION_DEFERRAL_STATEMENT.contains("WRITTEN-UNRUN"));
    assert!(SOH_ESTIMATION_DEFERRAL_STATEMENT.contains("HUMAN LAB GATE"));
    assert!(SOH_ESTIMATION_DEFERRAL_STATEMENT.contains("no aging law"));
    assert!(SOH_AUTHORITY_RULE_STATEMENT.contains("never replaces it"));
    assert!(SOH_AUTHORITY_RULE_STATEMENT.contains("never authorizes or produces a dispatch command"));
    assert!(SOH_DISAGREEMENT_STATEMENT.contains("remains the authority"));
}
