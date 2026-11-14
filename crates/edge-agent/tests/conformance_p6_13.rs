// P6.13 — Adapter read/telemetry conformance: the §19.2(6) fault-injection
//         set, driven through the protocol-agnostic harness (ledger P6.13;
//         test file written, NOT run — the loop bans in
//         `ledger/phase-06-edge-protocol/ENTRY_NOTE.md` hold; the human
//         build/test pass runs these).
//
// Coverage (each case must reject-or-flag per the harness contract — never
// crash, never fabricate continuity):
//
// 1. The FULL §19.2(6) set against the TEST-ONLY `SimulatedEmsSource`
//    fixture (in-process, no network): valid, malformed, truncated,
//    out-of-order, duplicate delivery, clock skew (future and past), stale
//    data — plus the strict-validation `unmapped_point` case (unknown
//    fields are RECORDED, never silently accepted).
// 2. Named diagnostics: every rejection carries a non-empty, named cause
//    (unknown quality code, absent quality, non-finite value, bad
//    timestamp, ingest-before-event) — never a silent partial application.
// 3. Duplicate delivery at the INGEST level: a same-tick re-poll is an
//    idempotent, counted skip (the P4.24 edge dedup window, cited) — one
//    stored value, never a double count.
// 4. No-panic property: every faulted reading flows through the SAME total
//    ACL path; the harness turns unreachable-adapter and invalid-fixture
//    conditions into named reports, never panics on untrusted input.
//
// Reference vectors: NONE RUN (none obtainable in this loop; the OpenEMS
// suite is P6.19's scope) — recorded as "reference vectors: NONE RUN
// (none obtainable in this loop); UNRUN" in the decision doc. The
// read-path latency/throughput measurement template (EMPTY cells, no
// fabricated figures) is likewise in the decision doc.

use std::sync::Arc;

use flexcore_contracts::quality::TelemetryQuality;
use flexcore_contracts::time_model::IngestTime;
use flexcore_contracts::units::ReactiveSignConvention;
use flexcore_edge_agent::adapters::acl::{
    AntiCorruptionLayer, AclError, NativeSign, NativeTimestamp, NativeUnit, PointMapping,
};
use flexcore_edge_agent::adapters::port::{EmsNativeReading, PointKey};
use flexcore_edge_agent::adapters::simulated::{
    SimulatedEmsSource, SimulatedFixture, SimulatedPointFixture,
};
use flexcore_edge_agent::conformance::{
    CaseOutcome, ConformanceCase, ConformanceHarness, ConformanceLimits,
};
use flexcore_edge_agent::ingest::AdapterIngestSource;
use flexcore_ports::fakes::FixedClock;
use flexcore_ports::telemetry_source::TelemetryQuantity;

const BASE_MILLIS: i64 = 1_700_000_000_000;
/// The edge receipt clock: 5 s AFTER the scenario's tick-0 event_time
/// (distinct timestamps, P0.09).
const INGEST_MILLIS: i64 = BASE_MILLIS + 5_000;

/// The deterministic TEST-ONLY scenario fixture: one active-power point in
/// kW (GOOD at tick 0), one declared-unmapped diagnostic point (the
/// "unknown field" of the strict-validation case).
fn fixture() -> SimulatedFixture {
    SimulatedFixture {
        points: vec![
            SimulatedPointFixture {
                point: PointKey::new("ess0", "ActivePower"),
                native_unit: NativeUnit::Kw,
                values_per_tick: vec![1_500.0],
                quality_per_tick: vec!["GOOD".to_string()],
            },
            SimulatedPointFixture {
                point: PointKey::new("ess0", "InternalDebugTemp"),
                native_unit: NativeUnit::Kw,
                values_per_tick: vec![0.0],
                quality_per_tick: vec!["GOOD".to_string()],
            },
        ],
        start_millis: BASE_MILLIS,
        step_millis: 1_000,
        reactive_source_convention: ReactiveSignConvention::InjectionPositive,
        cadence_millis: 5_000,
    }
}

/// The mapping table under test: the active-power point is mapped with a
/// DECLARED sign; the diagnostic point is in the DECLARED-unmapped
/// inventory (recorded as unknown, never silently accepted).
fn acl() -> AntiCorruptionLayer {
    AntiCorruptionLayer::new(
        vec![PointMapping {
            point: PointKey::new("ess0", "ActivePower"),
            asset_key: "ess-01".to_string(),
            quantity: TelemetryQuantity::ActivePower,
            native_unit: NativeUnit::Kw,
            native_sign: Some(NativeSign::ExportPositive),
            reactive_source_convention: None,
        }],
        "edge:pilot-site-A:simulated-test",
        vec![PointKey::new("ess0", "InternalDebugTemp")],
    )
    .expect("valid mapping table")
}

fn harness() -> ConformanceHarness {
    ConformanceHarness::new(
        acl(),
        INGEST_MILLIS,
        ConformanceLimits {
            max_clock_skew_past_millis: 3_600_000,
        },
    )
    .expect("valid limits")
}

#[test]
fn the_full_1926_fault_set_satisfies_the_contract_in_process() {
    let harness = harness();
    let mut adapter = SimulatedEmsSource::new(fixture());
    let report =
        harness.run_fault_injection_set(&mut adapter, &PointKey::new("ess0", "ActivePower"));
    assert!(
        report.is_clean(),
        "the §19.2(6) set must reject-or-flag per case, never crash or fabricate:\n{}",
        report.render()
    );
    // All nine cases ran, in the fixed deterministic order.
    assert_eq!(report.records.len(), 9);
    let names: Vec<&str> = report.records.iter().map(|r| r.case.name()).collect();
    assert_eq!(
        names,
        vec![
            "valid",
            "malformed",
            "truncated",
            "out_of_order",
            "duplicate_delivery",
            "clock_skew_future",
            "clock_skew_past",
            "stale_data",
            "unmapped_point",
        ]
    );
}

#[test]
fn malformed_readings_are_rejected_with_named_diagnostics_never_partially_applied() {
    let harness = harness();
    let mut adapter = SimulatedEmsSource::new(fixture());
    let template = adapter
        .read(&PointKey::new("ess0", "ActivePower"))
        .expect("the template reads");

    // Unknown quality code: a NAMED rejection (no default quality exists).
    let record = harness.run_case(ConformanceCase::Malformed, &template);
    match &record.outcome {
        CaseOutcome::Rejected { diagnostic } => {
            assert!(
                diagnostic.contains("WEIRD_VENDOR_FLAG"),
                "the diagnostic names the offending code: {diagnostic}"
            );
        }
        other => panic!("malformed must be rejected, got {other:?}"),
    }

    // A non-finite raw value: a NAMED rejection, before any conversion.
    let mut non_finite = template.clone();
    non_finite.raw_value = f64::NAN;
    let ingest = IngestTime::from_unix_millis(INGEST_MILLIS).expect("valid");
    assert_eq!(
        harness.acl().map_reading(&non_finite, ingest),
        Err(AclError::NonFiniteValue)
    );

    // The reading is rejected WHOLE: nothing partial exists to apply. The
    // proof at the ingest level: an ACL rejection produces NO sample and is
    // counted (exercised further by the sibling point still flowing in the
    // P6.08 suite; here the whole-reading rule is the named error above).
}

#[test]
fn truncated_readings_are_rejected_with_named_diagnostics() {
    let harness = harness();
    let mut adapter = SimulatedEmsSource::new(fixture());
    let template = adapter
        .read(&PointKey::new("ess0", "ActivePower"))
        .expect("the template reads");

    // Truncation shape 1: the frame lost its quality field → the ACL
    // refuses (there is no default quality).
    let record = harness.run_case(ConformanceCase::Truncated, &template);
    match &record.outcome {
        CaseOutcome::Rejected { diagnostic } => {
            assert!(
                diagnostic.to_lowercase().contains("quality"),
                "the diagnostic names the missing quality: {diagnostic}"
            );
        }
        other => panic!("truncated must be rejected, got {other:?}"),
    }

    // Truncation shape 2: the frame lost the tail of its RFC 3339 timestamp
    // → the ACL refuses the whole reading (never guesses a time).
    let mut cut = template.clone();
    cut.event_time = NativeTimestamp::UtcRfc3339("2026-11-12T10:00:".to_string());
    let ingest = IngestTime::from_unix_millis(INGEST_MILLIS).expect("valid");
    let error = harness
        .acl()
        .map_reading(&cut, ingest)
        .expect_err("a truncated timestamp is refused");
    assert!(
        matches!(error, AclError::NonUtcNativeTimestamp { .. }),
        "the truncation is a named timestamp refusal: {error}"
    );
}

#[test]
fn out_of_order_delivery_is_flagged_and_never_reordered_or_renumbered() {
    let harness = harness();
    let mut adapter = SimulatedEmsSource::new(fixture());
    let template = adapter
        .read(&PointKey::new("ess0", "ActivePower"))
        .expect("the template reads");
    assert_eq!(template.source_sequence, 1, "tick 0 carries sequence 1");
    let record = harness.run_case(ConformanceCase::OutOfOrder, &template);
    match &record.outcome {
        CaseOutcome::Flagged { diagnostic, .. } => {
            assert!(
                diagnostic.contains("sequence regression"),
                "the flag names the regression: {diagnostic}"
            );
        }
        other => panic!("out-of-order must be flagged, got {other:?}"),
    }
}

#[test]
fn duplicate_delivery_is_an_idempotent_counted_skip_at_the_ingest_level() {
    // The P4.24 edge dedup window (cited), exercised through the REAL read
    // path: the same deterministic tick re-polled yields the SAME identity
    // → skipped, counted, never double-counted.
    let sim = SimulatedEmsSource::new(fixture());
    let mut source = AdapterIngestSource::new(
        Box::new(sim),
        acl(),
        vec![PointMapping {
            point: PointKey::new("ess0", "ActivePower"),
            asset_key: "ess-01".to_string(),
            quantity: TelemetryQuantity::ActivePower,
            native_unit: NativeUnit::Kw,
            native_sign: Some(NativeSign::ExportPositive),
            reactive_source_convention: None,
        }],
        Arc::new(FixedClock::at_unix_millis(INGEST_MILLIS)),
        64,
    )
    .expect("valid composition");

    let first = source.poll(10).expect("first poll");
    assert_eq!(first.len(), 1, "the mapped point emits");
    let second = source.poll(10).expect("second poll (same tick)");
    assert!(
        second.is_empty(),
        "the exact re-delivery is deduplicated, not re-emitted"
    );
    let stats = source.stats();
    assert_eq!(stats.duplicates_skipped, 1, "the skip is counted");
    assert_eq!(stats.samples_emitted, 1, "the stored count does not grow");
}

#[test]
fn clock_skew_future_is_rejected_and_clock_skew_past_is_bounded() {
    let harness = harness();
    let mut adapter = SimulatedEmsSource::new(fixture());
    let template = adapter
        .read(&PointKey::new("ess0", "ActivePower"))
        .expect("the template reads");

    // Future event_time: rejected (ingest >= event, P0.09) — never a
    // silent clock "fix".
    let record = harness.run_case(ConformanceCase::ClockSkewFuture, &template);
    match &record.outcome {
        CaseOutcome::Rejected { diagnostic } => {
            assert!(
                diagnostic.contains("ingest_time must not precede event_time")
                    || diagnostic.contains("TimeOrdering")
                    || diagnostic.contains("rejected"),
                "the diagnostic names the ordering refusal: {diagnostic}"
            );
        }
        other => panic!("future skew must be rejected, got {other:?}"),
    }

    // Past event_time WITHIN the bound: accepted, recorded (contract
    // sanity — the bounded-past class is the Valid-class sibling of the
    // flagged case asserted below through the full set).
    let record = harness.run_case(ConformanceCase::Valid, &template);
    assert_eq!(record.outcome.class(), "accepted");

    // Past event_time BEYOND the bound: flagged (aged data must not pass
    // unremarked) — the full set asserts this via the harness contract.
    let full = harness.run_fault_injection_set(&mut adapter, &PointKey::new("ess0", "ActivePower"));
    let past = full
        .records
        .iter()
        .find(|r| r.case == ConformanceCase::ClockSkewPast)
        .expect("past-skew case present");
    match &past.outcome {
        CaseOutcome::Flagged { diagnostic, .. } => {
            assert!(
                diagnostic.contains("trails the edge receipt clock"),
                "the flag names the skew: {diagnostic}"
            );
        }
        other => panic!("aged data beyond the bound must be flagged, got {other:?}"),
    }
}

#[test]
fn stale_data_is_flagged_with_canonical_stale_never_upgraded() {
    let harness = harness();
    let mut adapter = SimulatedEmsSource::new(fixture());
    let template = adapter
        .read(&PointKey::new("ess0", "ActivePower"))
        .expect("the template reads");
    let record = harness.run_case(ConformanceCase::StaleData, &template);
    match &record.outcome {
        CaseOutcome::Flagged {
            diagnostic,
            quality: Some(quality),
        } => {
            assert_eq!(*quality, TelemetryQuality::Stale);
            assert!(diagnostic.contains("STALE"));
        }
        other => panic!("stale data must be flagged with canonical STALE, got {other:?}"),
    }
}

#[test]
fn unknown_fields_are_recorded_not_silently_accepted() {
    let harness = harness();
    // The declared-unmapped inventory RECORDS the unknown point (the
    // "logged, not silently accepted" surface of P6.13 step 1).
    assert!(
        harness
            .acl()
            .is_declared_unmapped(&PointKey::new("ess0", "InternalDebugTemp")),
        "the unknown diagnostic point is recorded as unmapped"
    );
    // A reading for an unmapped point is a NAMED rejection, never a guess.
    let record = harness.run_case(
        ConformanceCase::UnmappedPoint,
        &EmsNativeReading {
            point: PointKey::new("ess0", "ActivePower"),
            native_unit: NativeUnit::Kw,
            raw_value: 0.0,
            native_quality_code: Some("GOOD".to_string()),
            event_time: NativeTimestamp::UnixMillis(BASE_MILLIS),
            source_sequence: 1,
        },
    );
    match &record.outcome {
        CaseOutcome::Rejected { diagnostic } => {
            assert!(
                diagnostic.contains("conformance-unmapped"),
                "the diagnostic names the unmapped point: {diagnostic}"
            );
        }
        other => panic!("an unmapped point must be rejected, got {other:?}"),
    }
}

#[test]
fn every_faulted_reading_flows_the_same_total_path_without_panicking() {
    // The no-panic property: every case (including a forced-unreachable
    // adapter) runs through the same total mapping path and produces a
    // report — no panic, no unwrap on untrusted input.
    let harness = harness();
    let mut adapter = SimulatedEmsSource::new(fixture());
    let template = adapter
        .read(&PointKey::new("ess0", "ActivePower"))
        .expect("the template reads");
    for case in [
        ConformanceCase::Valid,
        ConformanceCase::Malformed,
        ConformanceCase::Truncated,
        ConformanceCase::OutOfOrder,
        ConformanceCase::DuplicateDelivery,
        ConformanceCase::ClockSkewFuture,
        ConformanceCase::ClockSkewPast,
        ConformanceCase::StaleData,
        ConformanceCase::UnmappedPoint,
    ] {
        let _record = harness.run_case(case, &template);
    }
    // And an unreachable adapter degrades to a NAMED violating report.
    adapter
        .control()
        .set_unreachable(Some("fault drill: EMS link down".to_string()));
    let report =
        harness.run_fault_injection_set(&mut adapter, &PointKey::new("ess0", "ActivePower"));
    assert!(
        !report.is_clean(),
        "an unreachable adapter cannot satisfy the contract — the report names it"
    );
    assert!(report.render().contains("template point"));
}
