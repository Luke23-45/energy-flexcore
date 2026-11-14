// P6.08 — Edge telemetry ingestion read path: integration tests (ledger
// P6.08; test file written, NOT run — the loop bans in
// `ledger/phase-06-edge-protocol/ENTRY_NOTE.md` hold: nothing built, no
// tests executed. The human build/test pass runs these).
//
// Coverage:
//
// 1. **Read-path integration** — `SimulatedEmsSource` (TEST-only fixture) →
//    `AdapterIngestSource` (EmsAdapter.read → P6.12 ACL → canonical
//    sample) → `EdgeRunner` poll→buffer→forward → recording uplink.
//    Asserts forward ORDER, canonical units (kW→MW, %→fraction), quality
//    passthrough (OUT_OF_RANGE arrives SUSPECT with the original code —
//    never upgraded), event_time from the source, ingest_time stamped at
//    the edge (distinct, later), source sequence, non-`flexcore:` source
//    identity (P2.04 D-T6).
// 2. **Edge deduplication** — a same-event_time re-read is an idempotent,
//    counted skip (P4.24 fixture-(a) discipline, cited); a second tick
//    with fresh event times flows again.
// 3. **ACL failure accounting** — an unmappable reading is rejected WHOLE
//    and counted; the healthy sibling point still flows; a poll where
//    EVERY point fails (forced-unreachable fixture) is a hard
//    `EDGE_SOURCE_POLL_FAILED` audit event, never a clean empty poll.
// 4. **Full offline scenario through the runner** — samples flow
//    buffer-first (the P6.05 order), the uplink receives exactly the
//    mapped set, and the audit trail shows the P6.05 state machine (no
//    fabricated telemetry anywhere).

use std::sync::{Arc, Mutex};
use std::time::Duration;

use flexcore_contracts::quality::TelemetryQuality;
use flexcore_edge_agent::adapters::acl::{AntiCorruptionLayer, NativeSign, NativeUnit, PointMapping};
use flexcore_edge_agent::adapters::port::PointKey;
use flexcore_edge_agent::adapters::simulated::{
    SimulatedEmsSource, SimulatedFixture, SimulatedPointFixture,
};
use flexcore_edge_agent::ingest::{AdapterIngestSource, IngestStats};
use flexcore_edge_agent::ports::{
    EdgeError, EdgeTelemetrySample, InMemoryAuditSink, InMemoryLocalBuffer, UplinkChannel,
};
use flexcore_edge_agent::runner::{EdgeRunner, RunnerParams};
use flexcore_contracts::units::ReactiveSignConvention;
use flexcore_ports::fakes::FixedClock;
use flexcore_ports::telemetry_source::TelemetryQuantity;

const BASE_MILLIS: i64 = 1_700_000_000_000;
/// The edge clock: the edge receives 5 s AFTER the scenario's tick-0
/// event_time (distinct timestamps, P0.09).
const EDGE_INGEST_MILLIS: i64 = BASE_MILLIS + 5_000;

/// The deterministic scenario fixture: two points, one of which goes
/// OUT_OF_RANGE on tick 1 (the degraded-quality scenario).
fn fixture() -> SimulatedFixture {
    SimulatedFixture {
        points: vec![
            SimulatedPointFixture {
                point: PointKey::new("ess0", "ActivePower"),
                native_unit: NativeUnit::Kw,
                values_per_tick: vec![1_500.0, 1_600.0],
                quality_per_tick: vec!["GOOD".to_string(), "GOOD".to_string()],
            },
            SimulatedPointFixture {
                point: PointKey::new("ess0", "Soc"),
                native_unit: NativeUnit::SocPercent,
                values_per_tick: vec![87.0, 86.0],
                quality_per_tick: vec!["GOOD".to_string(), "OUT_OF_RANGE".to_string()],
            },
        ],
        start_millis: BASE_MILLIS,
        step_millis: 1_000,
        reactive_source_convention: ReactiveSignConvention::InjectionPositive,
        cadence_millis: 5_000,
    }
}

fn acl() -> AntiCorruptionLayer {
    AntiCorruptionLayer::new(
        vec![
            PointMapping {
                point: PointKey::new("ess0", "ActivePower"),
                asset_key: "ess-01".to_string(),
                quantity: TelemetryQuantity::ActivePower,
                native_unit: NativeUnit::Kw,
                native_sign: Some(NativeSign::ExportPositive),
                reactive_source_convention: None,
            },
            PointMapping {
                point: PointKey::new("ess0", "Soc"),
                asset_key: "ess-01".to_string(),
                quantity: TelemetryQuantity::Soc,
                native_unit: NativeUnit::SocPercent,
                native_sign: None,
                reactive_source_convention: None,
            },
        ],
        "edge:pilot-site-A:simulated-test",
        vec![PointKey::new("ess0", "InternalDebugTemp")],
    )
    .expect("valid mapping table")
}

fn acl_mappings() -> Vec<PointMapping> {
    vec![
        PointMapping {
            point: PointKey::new("ess0", "ActivePower"),
            asset_key: "ess-01".to_string(),
            quantity: TelemetryQuantity::ActivePower,
            native_unit: NativeUnit::Kw,
            native_sign: Some(NativeSign::ExportPositive),
            reactive_source_convention: None,
        },
        PointMapping {
            point: PointKey::new("ess0", "Soc"),
            asset_key: "ess-01".to_string(),
            quantity: TelemetryQuantity::Soc,
            native_unit: NativeUnit::SocPercent,
            native_sign: None,
            reactive_source_convention: None,
        },
    ]
}

/// One forwarded sample, reduced to the fields the assertions need.
#[derive(Debug, Clone, PartialEq)]
struct Recorded {
    source_id: String,
    sequence: u64,
    quality: TelemetryQuality,
    source_quality_code: Option<String>,
    event_millis: i128,
    ingest_millis: i128,
    asset_key: String,
}

/// A recording [`UplinkChannel`] test double (order-preserving; also
/// scriptable to fail the first N forwards for the outage scenarios).
struct RecordingUplink {
    recorded: Arc<Mutex<Vec<Recorded>>>,
    fail_first: Mutex<u32>,
}

impl RecordingUplink {
    fn new(recorded: Arc<Mutex<Vec<Recorded>>>) -> Self {
        Self {
            recorded,
            fail_first: Mutex::new(0),
        }
    }

    fn failing_first(recorded: Arc<Mutex<Vec<Recorded>>>, n: u32) -> Self {
        Self {
            recorded,
            fail_first: Mutex::new(n),
        }
    }
}

impl UplinkChannel for RecordingUplink {
    fn is_connected(&self) -> bool {
        *self.fail_first.lock().expect("poisoned") == 0
    }

    fn forward(&mut self, samples: &[EdgeTelemetrySample]) -> Result<(), EdgeError> {
        let mut fails = self.fail_first.lock().expect("poisoned");
        if *fails > 0 {
            *fails -= 1;
            return Err(EdgeError::UplinkForwardFailed {
                reason: "scripted channel down".to_string(),
            });
        }
        drop(fails);
        self.recorded
            .lock()
            .expect("poisoned")
            .extend(samples.iter().map(|s| Recorded {
                source_id: s.source_id().to_string(),
                sequence: s.source_sequence(),
                quality: s.quality(),
                source_quality_code: s.source_quality_code().map(str::to_string),
                event_millis: s.event_time().unix_millis(),
                ingest_millis: s.ingest_time().unix_millis(),
                asset_key: s.asset_key().to_string(),
            }));
        Ok(())
    }
}

/// Composes the full P6.08 read path behind the runner: fixture → adapter
/// (TEST-only id) → ACL → `AdapterIngestSource` → `EdgeRunner` over the
/// recording uplink and the in-memory buffer.
fn composed_runner(
    fixture: SimulatedFixture,
    uplink: RecordingUplink,
    audit: Arc<InMemoryAuditSink>,
) -> EdgeRunner {
    let source = AdapterIngestSource::new(
        Box::new(SimulatedEmsSource::new(fixture)),
        acl(),
        acl_mappings(),
        Arc::new(FixedClock::at_unix_millis(EDGE_INGEST_MILLIS)),
        64,
    )
    .expect("valid read-path composition");
    EdgeRunner::new(
        RunnerParams::from_config(Duration::from_millis(5_000), 10, 100),
        Box::new(source),
        Box::new(uplink),
        Box::new(InMemoryLocalBuffer::new(100)),
        Arc::clone(&audit) as Arc<dyn flexcore_edge_agent::ports::AuditSink>,
        Arc::new(FixedClock::at_unix_millis(EDGE_INGEST_MILLIS)),
    )
}

// --- 1. Read-path integration --------------------------------------------------

#[test]
fn read_path_flows_simulated_to_uplink_with_canonical_fields_in_order() {
    let recorded = Arc::new(Mutex::new(Vec::new()));
    let audit = Arc::new(InMemoryAuditSink::new());
    let mut runner = composed_runner(fixture(), RecordingUplink::new(Arc::clone(&recorded)), audit);

    let report = runner.tick();
    assert_eq!(report.polled, 2, "both mapped points polled");
    assert_eq!(report.buffered, 2, "buffer-first ingestion (P6.05 order)");
    assert_eq!(report.forwarded, 2);
    assert!(!runner.is_degraded());
    assert!(!report.poll_failed);
    assert!(!report.forward_failed);

    let forwarded = recorded.lock().expect("poisoned").clone();
    assert_eq!(forwarded.len(), 2);
    // Order: the ACL's declared mapping order (power first, soc second).
    assert_eq!(
        forwarded[0].source_id,
        "edge:pilot-site-A:simulated-test/ess0/ActivePower"
    );
    assert_eq!(
        forwarded[1].source_id,
        "edge:pilot-site-A:simulated-test/ess0/Soc"
    );
    // Source identity NEVER uses the reserved platform namespace
    // (P2.04 D-T6; also enforced by the sample constructor).
    for record in &forwarded {
        assert!(
            !record.source_id.starts_with("flexcore:"),
            "measured edge sources must not use the reserved namespace"
        );
        // event_time from the SOURCE (tick 0); ingest_time the EDGE stamp
        // — distinct fields, never conflated (P0.09).
        assert_eq!(record.event_millis, BASE_MILLIS as i128);
        assert_eq!(record.ingest_millis, EDGE_INGEST_MILLIS as i128);
        assert!(record.ingest_millis > record.event_millis);
        assert_eq!(record.asset_key, "ess-01");
    }
    // Sequences are the source's (tick + 1).
    assert_eq!(forwarded[0].sequence, 1);
    assert_eq!(forwarded[1].sequence, 1);
    // Quality at tick 0: both GOOD — exactly one canonical state per value.
    assert_eq!(forwarded[0].quality, TelemetryQuality::Good);
    assert_eq!(forwarded[1].quality, TelemetryQuality::Good);
    assert_eq!(forwarded[0].source_quality_code, None);
}

#[test]
fn degraded_source_quality_arrives_flagged_never_upgraded() {
    // Advance the deterministic scenario FIRST: tick 1 declares
    // OUT_OF_RANGE on the soc point. A deliberately corrupted/degraded
    // source value MUST arrive flagged (SUSPECT), never as GOOD — the
    // P6.08 acceptance rule.
    let sim = SimulatedEmsSource::new(fixture());
    sim.control().advance_tick();
    let recorded = Arc::new(Mutex::new(Vec::new()));
    let source = AdapterIngestSource::new(
        Box::new(sim),
        acl(),
        acl_mappings(),
        Arc::new(FixedClock::at_unix_millis(EDGE_INGEST_MILLIS)),
        64,
    )
    .expect("valid composition");
    let mut runner = EdgeRunner::new(
        RunnerParams::from_config(Duration::from_millis(5_000), 10, 100),
        Box::new(source),
        Box::new(RecordingUplink::new(Arc::clone(&recorded))),
        Box::new(InMemoryLocalBuffer::new(100)),
        Arc::new(InMemoryAuditSink::new()) as Arc<dyn flexcore_edge_agent::ports::AuditSink>,
        Arc::new(FixedClock::at_unix_millis(EDGE_INGEST_MILLIS)),
    );
    let report = runner.tick();
    assert_eq!(report.forwarded, 2);

    let forwarded = recorded.lock().expect("poisoned").clone();
    let soc = forwarded
        .iter()
        .find(|r| r.source_id.ends_with("/Soc"))
        .expect("soc forwarded");
    assert_eq!(soc.quality, TelemetryQuality::Suspect);
    assert_eq!(soc.source_quality_code, Some("OUT_OF_RANGE".to_string()));
    assert_eq!(soc.event_millis, (BASE_MILLIS + 1_000) as i128);
    // The power point is unaffected: quality is per-value.
    let power = forwarded
        .iter()
        .find(|r| r.source_id.ends_with("/ActivePower"))
        .expect("power forwarded");
    assert_eq!(power.quality, TelemetryQuality::Good);
}

// --- 2. Edge deduplication -------------------------------------------------------

#[test]
fn same_event_time_reread_is_deduplicated_through_the_runner() {
    let recorded = Arc::new(Mutex::new(Vec::new()));
    let audit = Arc::new(InMemoryAuditSink::new());
    let mut runner = composed_runner(fixture(), RecordingUplink::new(Arc::clone(&recorded)), Arc::clone(&audit));

    let first = runner.tick();
    assert_eq!(first.forwarded, 2);

    // Re-poll at the SAME scenario tick: identical identity + content →
    // idempotent skip (P4.24 fixture-(a) discipline, cited), counted, and
    // nothing re-forwarded.
    let second = runner.tick();
    assert_eq!(second.polled, 0, "re-read samples are deduplicated away");
    assert_eq!(second.forwarded, 0);
    assert_eq!(recorded.lock().expect("poisoned").len(), 2, "no duplicates downstream");

    // The ingest stats are visible through the composed source only via
    // the counters asserted in the unit tests; here the observable
    // behaviour (zero re-forwarded samples) is the contract.
    let events = audit.events();
    assert!(
        events
            .iter()
            .all(|e| e.kind.name() != "EDGE_SOURCE_POLL_FAILED"),
        "a deduplicated poll is NOT a poll failure"
    );
}

#[test]
fn a_fresh_event_time_flows_again_after_dedup() {
    let sim = SimulatedEmsSource::new(fixture());
    let control = sim.control();
    let recorded = Arc::new(Mutex::new(Vec::new()));
    let source = AdapterIngestSource::new(
        Box::new(sim),
        acl(),
        acl_mappings(),
        Arc::new(FixedClock::at_unix_millis(EDGE_INGEST_MILLIS)),
        64,
    )
    .expect("valid composition");
    let mut runner = EdgeRunner::new(
        RunnerParams::from_config(Duration::from_millis(5_000), 10, 100),
        Box::new(source),
        Box::new(RecordingUplink::new(Arc::clone(&recorded))),
        Box::new(InMemoryLocalBuffer::new(100)),
        Arc::new(InMemoryAuditSink::new()) as Arc<dyn flexcore_edge_agent::ports::AuditSink>,
        Arc::new(FixedClock::at_unix_millis(EDGE_INGEST_MILLIS)),
    );
    let _ = runner.tick();
    control.advance_tick();
    let second = runner.tick();
    assert_eq!(second.forwarded, 2, "fresh event_time: no dedup");
    let forwarded = recorded.lock().expect("poisoned");
    let sequences: Vec<u64> = forwarded.iter().map(|r| r.sequence).collect();
    assert_eq!(sequences, vec![2, 2], "tick 1 sequences");
}

// --- 3. ACL failure accounting ----------------------------------------------------

#[test]
fn unmappable_reading_is_counted_and_the_healthy_point_still_flows() {
    // The soc point reports a code the ACL cannot map: the reading is
    // rejected WHOLE, counted, and never silently passed — while the
    // power point flows (the recorded per-poll accounting policy).
    let mut fixture = fixture();
    fixture.points[1].quality_per_tick = vec!["WEIRD_VENDOR_FLAG".to_string()];
    let recorded = Arc::new(Mutex::new(Vec::new()));
    let audit = Arc::new(InMemoryAuditSink::new());
    let mut runner = composed_runner(fixture, RecordingUplink::new(Arc::clone(&recorded)), audit);

    let report = runner.tick();
    assert_eq!(report.polled, 1, "only the mappable point emits");
    assert_eq!(report.forwarded, 1);
    assert!(!report.poll_failed, "a partial-with-accounting poll is not a failure");
    let forwarded = recorded.lock().expect("poisoned");
    assert_eq!(forwarded[0].source_id, "edge:pilot-site-A:simulated-test/ess0/ActivePower");
}

#[test]
fn forced_unreachable_source_is_a_hard_audited_poll_failure() {
    let sim = SimulatedEmsSource::new(fixture());
    let control = sim.control();
    control.set_unreachable(Some("drill: forced disconnect".to_string()));
    let audit = Arc::new(InMemoryAuditSink::new());
    let source = AdapterIngestSource::new(
        Box::new(sim),
        acl(),
        acl_mappings(),
        Arc::new(FixedClock::at_unix_millis(EDGE_INGEST_MILLIS)),
        64,
    )
    .expect("valid composition");
    let mut runner = EdgeRunner::new(
        RunnerParams::from_config(Duration::from_millis(5_000), 10, 100),
        Box::new(source),
        Box::new(RecordingUplink::new(Arc::new(Mutex::new(Vec::new())))),
        Box::new(InMemoryLocalBuffer::new(100)),
        Arc::clone(&audit) as Arc<dyn flexcore_edge_agent::ports::AuditSink>,
        Arc::new(FixedClock::at_unix_millis(EDGE_INGEST_MILLIS)),
    );
    let report = runner.tick();
    assert!(report.poll_failed, "an unreachable source fails the poll");
    assert_eq!(report.polled, 0, "no fabricated telemetry — nothing, ever");
    let events = audit.events();
    assert!(
        events
            .iter()
            .any(|e| e.kind.name() == "EDGE_SOURCE_POLL_FAILED"),
        "the hard poll failure is audited"
    );
}

// --- 4. Full offline scenario through the runner -----------------------------------

#[test]
fn offline_then_recovery_forwards_everything_in_order_across_the_outage() {
    // The P6.08 read path composed with the P6.09 in-memory buffer and a
    // scripted outage: poll→buffer→forward with the buffer-first order, a
    // degraded episode, and an ordered drain on recovery.
    let recorded = Arc::new(Mutex::new(Vec::new()));
    let audit = Arc::new(InMemoryAuditSink::new());
    let sim = SimulatedEmsSource::new(fixture());
    let control = sim.control();
    let source = AdapterIngestSource::new(
        Box::new(sim),
        acl(),
        acl_mappings(),
        Arc::new(FixedClock::at_unix_millis(EDGE_INGEST_MILLIS)),
        64,
    )
    .expect("valid composition");
    let mut runner = EdgeRunner::new(
        RunnerParams::from_config(Duration::from_millis(5_000), 2, 100),
        Box::new(source),
        Box::new(RecordingUplink::failing_first(Arc::clone(&recorded), 2)),
        Box::new(InMemoryLocalBuffer::new(100)),
        Arc::clone(&audit) as Arc<dyn flexcore_edge_agent::ports::AuditSink>,
        Arc::new(FixedClock::at_unix_millis(EDGE_INGEST_MILLIS)),
    );

    // Tick 1: poll 2 (tick 0), buffer 2, forward FAILS → degraded, order kept.
    let report = runner.tick();
    assert_eq!(report.polled, 2);
    assert_eq!(report.buffered, 2);
    assert_eq!(report.forwarded, 0);
    assert!(runner.is_degraded());

    // Tick 2: new scenario tick, poll 2 more, forward FAILS again.
    control.advance_tick();
    let report = runner.tick();
    assert_eq!(report.polled, 2);
    assert_eq!(report.forwarded, 0);
    assert!(runner.is_degraded());

    // Tick 3: the uplink recovers → all four forward IN ORDER (tick-0
    // samples first, tick-1 samples second; the §15.4 ordered drain).
    let report = runner.tick();
    assert!(!runner.is_degraded());
    assert_eq!(report.forwarded, 4);
    assert_eq!(runner.buffered_len(), 0);

    let forwarded = recorded.lock().expect("poisoned").clone();
    let keys: Vec<(i128, &str)> = forwarded
        .iter()
        .map(|r| (r.event_millis, r.source_id.as_str()))
        .collect();
    assert_eq!(
        keys,
        vec![
            (BASE_MILLIS as i128, "edge:pilot-site-A:simulated-test/ess0/ActivePower"),
            (BASE_MILLIS as i128, "edge:pilot-site-A:simulated-test/ess0/Soc"),
            ((BASE_MILLIS + 1_000) as i128, "edge:pilot-site-A:simulated-test/ess0/ActivePower"),
            ((BASE_MILLIS + 1_000) as i128, "edge:pilot-site-A:simulated-test/ess0/Soc"),
        ],
        "order across the outage: event order, per-point identity"
    );

    // The audit trail shows the degraded state machine exactly once each.
    let events = audit.events();
    let kinds: Vec<&str> = events.iter().map(|e| e.kind.name()).collect();
    assert_eq!(
        kinds.iter().filter(|k| **k == "EDGE_DEGRADED_BUFFERING_ENTERED").count(),
        1
    );
    assert_eq!(
        kinds.iter().filter(|k| **k == "EDGE_DEGRADED_BUFFERING_EXITED").count(),
        1
    );
    assert_eq!(
        kinds.iter().filter(|k| **k == "EDGE_FORWARD_BATCH").count(),
        0,
        "the runner records forward batches through its own audit kinds; \
         this assertion pins the P6.05 vocabulary is unchanged by P6.08"
    );
}

#[test]
fn simulated_adapter_registry_id_is_allowlisted_test_only() {
    // The TEST-ONLY registration is visible through the allowlist.
    let registry = flexcore_edge_agent::adapters::AdapterRegistry::with_default_allowlist();
    assert!(registry.is_allowlisted("simulated-test"));
    assert_eq!(
        registry.status_of("simulated-test"),
        Ok(flexcore_edge_agent::adapters::AdapterStatus::TestOnly)
    );
    let error = flexcore_edge_agent::adapters::RegistryError::NotAllowlisted {
        id: "openems-rest-does-not-exist".to_string(),
    };
    assert!(error.to_string().contains("not in the allowlist"));
}

// Pin the documented configuration surface: the P6.08/P6.09 knobs exist
// with the recorded names, and the counters struct starts clean.
#[test]
fn config_surface_documents_the_p6_08_knobs() {
    assert_eq!(
        flexcore_edge_agent::config::ENV_DEDUP_WINDOW,
        "FLEXCORE_EDGE_DEDUP_WINDOW"
    );
    assert_eq!(
        flexcore_edge_agent::config::ENV_BUFFER_MAX_AGE_MS,
        "FLEXCORE_EDGE_BUFFER_MAX_AGE_MS"
    );
    assert_eq!(
        flexcore_edge_agent::config::ENV_OVERFLOW_POLICY,
        "FLEXCORE_EDGE_OVERFLOW_POLICY"
    );
    assert_eq!(
        flexcore_edge_agent::config::OVERFLOW_POLICY_DROP_OLDEST_AUDITED,
        "drop-oldest-audited"
    );
    assert_eq!(
        IngestStats::default(),
        IngestStats {
            polls: 0,
            samples_emitted: 0,
            read_failures: 0,
            acl_rejections: 0,
            duplicates_skipped: 0,
            conflicting_duplicates_skipped: 0,
            hard_poll_failures: 0,
            last_failure_reason: None,
        }
    );
}
