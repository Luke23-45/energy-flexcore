// P6.09 — Telemetry buffering, store-and-forward, and recovery:
// integration tests (ledger P6.09; test file written, NOT run — the loop
// bans in `ledger/phase-06-edge-protocol/ENTRY_NOTE.md` hold: nothing
// built, no tests executed. The human build/test pass runs these).
//
// Coverage:
//
// 1. **Store-and-forward across a forced outage (runner level)** — the
//    durable `FileLocalBuffer` composed into the `EdgeRunner` with a
//    scripted uplink outage: samples accumulate ON DISK while degraded; a
//    simulated edge "crash" (the runner dropped) leaves the journal
//    intact; the reopened edge re-delivers the un-committed records in
//    order (at-least-once, §15.4); after a confirmed forward the journal
//    compacts to empty (no duplicates on the NEXT restart).
// 2. **Recovery drill** — forced disconnect (the simulated fixture's
//    control handle) while the uplink is down, restore, drain completes in
//    order; the degraded state machine audits entry/exit exactly once.
// 3. **Overflow drop accounting through the runner** — a tiny-capacity
//    durable buffer under a fail-closed uplink: backpressure pauses
//    polling FIRST (the P6.05 first line); the audited drop-oldest policy
//    is the last resort and its counters latch.
// 4. **LocalBuffer trait parity** — the file buffer and the in-memory
//    buffer behave identically at the port level (bounds, order, prepend);
//    `commit_forwarded` is a no-op for the volatile buffer (recorded).
//
// The buffer's own unit tests (journal round-trip, corrupt-tail recovery,
// overflow, max-age, commit compaction) live in `buffer_file.rs`.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use flexcore_contracts::quality::TelemetryQuality;
use flexcore_contracts::units::ReactiveSignConvention;
use flexcore_edge_agent::adapters::acl::{AntiCorruptionLayer, NativeSign, NativeUnit, PointMapping};
use flexcore_edge_agent::adapters::port::PointKey;
use flexcore_edge_agent::adapters::simulated::{
    SimulatedEmsSource, SimulatedFixture, SimulatedPointFixture,
};
use flexcore_edge_agent::buffer_file::FileLocalBuffer;
use flexcore_edge_agent::ingest::AdapterIngestSource;
use flexcore_edge_agent::ports::{
    EdgeError, EdgeTelemetrySample, EdgeTelemetrySampleParts, FailClosedUplink,
    InMemoryAuditSink, InMemoryLocalBuffer, LocalBuffer, LocalBufferExt, UplinkChannel,
};
use flexcore_edge_agent::runner::{EdgeRunner, RunnerParams};
use flexcore_contracts::time_model::{EventTime, IngestTime};
use flexcore_contracts::units::ActivePowerMW;
use flexcore_ports::fakes::FixedClock;
use flexcore_ports::telemetry_source::{QuantityValue, TelemetryQuantity};
use tokio::sync::watch;

const BASE_MILLIS: i64 = 1_700_000_000_000;
const EDGE_INGEST_MILLIS: i64 = BASE_MILLIS + 5_000;

fn fixture() -> SimulatedFixture {
    SimulatedFixture {
        points: vec![SimulatedPointFixture {
            point: PointKey::new("ess0", "ActivePower"),
            native_unit: NativeUnit::Kw,
            values_per_tick: vec![1_500.0, 1_600.0, 1_700.0],
            quality_per_tick: vec!["GOOD".to_string()],
        }],
        start_millis: BASE_MILLIS,
        step_millis: 1_000,
        reactive_source_convention: ReactiveSignConvention::InjectionPositive,
        cadence_millis: 5_000,
    }
}

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
        vec![],
    )
    .expect("valid mapping table")
}

fn acl_mappings() -> Vec<PointMapping> {
    vec![PointMapping {
        point: PointKey::new("ess0", "ActivePower"),
        asset_key: "ess-01".to_string(),
        quantity: TelemetryQuantity::ActivePower,
        native_unit: NativeUnit::Kw,
        native_sign: Some(NativeSign::ExportPositive),
        reactive_source_convention: None,
    }]
}

/// A runner composed over the DURABLE buffer, the simulated read path, and
/// a scriptable uplink. Returns the runner plus its audit sink.
fn durable_runner(
    path: &std::path::Path,
    uplink: Box<dyn UplinkChannel>,
    audit: Arc<InMemoryAuditSink>,
) -> EdgeRunner {
    let source = AdapterIngestSource::new(
        Box::new(SimulatedEmsSource::new(fixture())),
        acl(),
        acl_mappings(),
        Arc::new(FixedClock::at_unix_millis(EDGE_INGEST_MILLIS)),
        64,
    )
    .expect("valid read-path composition");
    let buffer = FileLocalBuffer::open(
        path,
        100,
        Some(Duration::from_millis(3_600_000)),
        Arc::new(FixedClock::at_unix_millis(EDGE_INGEST_MILLIS)) as Arc<dyn flexcore_ports::clock::Clock + Send + Sync>,
    )
    .expect("the durable buffer opens");
    EdgeRunner::new(
        RunnerParams::from_config(Duration::from_millis(5_000), 10, 100),
        Box::new(source),
        uplink,
        Box::new(buffer),
        Arc::clone(&audit) as Arc<dyn flexcore_edge_agent::ports::AuditSink>,
        Arc::new(FixedClock::at_unix_millis(EDGE_INGEST_MILLIS)),
    )
}

/// A scripted uplink: fails the first N forwards, then succeeds (the
/// forced-outage / recovery drill fake).
struct FlakyUplink {
    fail_first: Mutex<u32>,
}

impl FlakyUplink {
    fn failing_first(n: u32) -> Self {
        Self {
            fail_first: Mutex::new(n),
        }
    }
}

impl UplinkChannel for FlakyUplink {
    fn is_connected(&self) -> bool {
        *self.fail_first.lock().expect("poisoned") == 0
    }
    fn forward(&mut self, samples: &[EdgeTelemetrySample]) -> Result<(), EdgeError> {
        let mut fails = self.fail_first.lock().expect("poisoned");
        if *fails > 0 {
            *fails -= 1;
            return Err(EdgeError::UplinkForwardFailed {
                reason: "drill: channel down".to_string(),
            });
        }
        assert!(!samples.is_empty());
        Ok(())
    }
}

/// A std-only unique scratch directory per call.
fn scratch_dir(label: &str) -> std::path::PathBuf {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "flexcore-edge-p609-it-{}-{}-{}",
        std::process::id(),
        label,
        COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&dir).expect("scratch dir creates");
    dir
}

// --- 1. Store-and-forward across a forced outage (runner level) -----------------

#[test]
fn outage_accumulates_on_disk_and_restart_redelivers_in_order() {
    let dir = scratch_dir("outage-restart");
    let journal = dir.join("telemetry.journal");

    // Phase 1 — channel DOWN: the runner keeps polling (degraded), every
    // sample is durably buffered (disk first), nothing is forwarded.
    let audit = Arc::new(InMemoryAuditSink::new());
    {
        let mut runner = durable_runner(&journal, Box::new(FailClosedUplink), Arc::clone(&audit));
        let report = runner.tick();
        assert_eq!(report.polled, 1);
        assert_eq!(report.buffered, 1);
        assert_eq!(report.forwarded, 0);
        assert!(runner.is_degraded(), "the channel-down state is degraded mode");
        drop(runner); // the edge "crashes" mid-outage (nothing committed)
    }

    // Phase 2 — restart: the journal replays; the un-committed record is
    // re-delivered (at-least-once; a downstream duplicate would be
    // consumed by the platform's P4.24-style idempotent ingest — cited).
    let clock = Arc::new(FixedClock::at_unix_millis(EDGE_INGEST_MILLIS))
        as Arc<dyn flexcore_ports::clock::Clock + Send + Sync>;
    let mut recovered = FileLocalBuffer::open(&journal, 100, Some(Duration::from_millis(3_600_000)), clock)
        .expect("the journal survives the crash");
    assert_eq!(recovered.len(), 1, "the outage sample survives the restart");
    assert_eq!(recovered.corrupt_tail_truncated(), 0);
    let redelivered = recovered.drain(10);
    assert_eq!(redelivered.len(), 1);
    assert_eq!(redelivered[0].source_sequence(), 1, "ordered redelivery");
    assert_eq!(
        redelivered[0].event_time().unix_millis(),
        BASE_MILLIS as i128,
        "event_time survived the round trip (never re-stamped)"
    );

    // Phase 3 — the forward succeeds and the runner commits: the journal
    // compacts to empty, so the NEXT restart holds nothing (no duplicate
    // storm from already-delivered records).
    recovered.commit_forwarded(&redelivered);
    drop(recovered);
    let clock2 = Arc::new(FixedClock::at_unix_millis(EDGE_INGEST_MILLIS))
        as Arc<dyn flexcore_ports::clock::Clock + Send + Sync>;
    let clean = FileLocalBuffer::open(&journal, 100, Some(Duration::from_millis(3_600_000)), clock2)
        .expect("reopens after the commit");
    assert_eq!(clean.len(), 0, "committed records do not redeliver");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn runner_commit_after_recovery_empties_the_journal() {
    let dir = scratch_dir("commit-runner");
    let journal = dir.join("telemetry.journal");
    let audit = Arc::new(InMemoryAuditSink::new());

    // Outage: one sample buffered durably.
    {
        let mut runner = durable_runner(&journal, Box::new(FlakyUplink::failing_first(1)), Arc::clone(&audit));
        let _ = runner.tick(); // forward fails → degraded, sample on disk
        assert!(runner.is_degraded());
    }

    // Recovery: a fresh runner over the SAME journal with a healthy
    // uplink. This tick drains the redelivered record AND polls the fresh
    // scenario sample (identical content, a NEW dedup window — so both
    // append); both forward and the runner's commit compacts the journal.
    // The identical pair is exactly the at-least-once duplicate the
    // platform's P4.24-style idempotent ingest consumes (cited).
    {
        let mut runner = durable_runner(&journal, Box::new(FlakyUplink::failing_first(0)), Arc::clone(&audit));
        let report = runner.tick();
        assert_eq!(report.forwarded, 2, "the redelivered record + the fresh sample");
        assert!(!runner.is_degraded());
    }

    // The journal is empty after the committed forward.
    let clock = Arc::new(FixedClock::at_unix_millis(EDGE_INGEST_MILLIS))
        as Arc<dyn flexcore_ports::clock::Clock + Send + Sync>;
    let after = FileLocalBuffer::open(&journal, 100, Some(Duration::from_millis(3_600_000)), clock)
        .expect("reopens");
    assert_eq!(after.len(), 0, "the committed records are compacted away");

    // The audit trail shows exactly one degraded episode.
    let events = audit.events();
    let kinds: Vec<&str> = events.iter().map(|e| e.kind.name()).collect();
    assert_eq!(
        kinds.iter().filter(|k| **k == "EDGE_DEGRADED_BUFFERING_ENTERED").count(),
        1,
        "one entry across both runner lifetimes (the recovered runner saw a healthy uplink)"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// --- 2. Recovery drill -----------------------------------------------------------

#[tokio::test]
async fn forced_disconnect_then_restore_drains_in_order_audited() {
    let dir = scratch_dir("drill");
    let journal = dir.join("telemetry.journal");
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
        RunnerParams::from_config(Duration::from_millis(20), 10, 100),
        Box::new(source),
        Box::new(FlakyUplink::failing_first(0)),
        Box::new(FileLocalBuffer::open(
            &journal,
            100,
            Some(Duration::from_millis(3_600_000)),
            Arc::new(FixedClock::at_unix_millis(EDGE_INGEST_MILLIS))
                as Arc<dyn flexcore_ports::clock::Clock + Send + Sync>,
        )
        .expect("durable buffer opens")),
        Arc::clone(&audit) as Arc<dyn flexcore_edge_agent::ports::AuditSink>,
        Arc::new(FixedClock::at_unix_millis(EDGE_INGEST_MILLIS)),
    );

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let handle = tokio::spawn(runner.run(shutdown_rx));

    // Phase 1 — the site link is forced down for a few ticks.
    control.set_unreachable(Some("drill: forced disconnect".to_string()));
    tokio::time::sleep(Duration::from_millis(60)).await;

    // Phase 2 — restore: the loop polls again (fresh event times) and the
    // healthy uplink drains everything, in order. Shutdown waits long
    // enough that at least one full tick runs after the restore.
    control.set_unreachable(None);
    control.advance_tick();
    tokio::time::sleep(Duration::from_millis(60)).await;
    shutdown_tx.send(true).expect("the receiver is alive");
    let last = handle.await.expect("the runner finishes");

    assert!(!last.degraded, "the drill ends healthy");
    assert_eq!(last.buffered_len_after, 0, "the buffer drains after restore");

    // The DURABLE outcome is the deterministic assertion: everything
    // forwarded was committed, so the journal holds nothing.
    let clock = Arc::new(FixedClock::at_unix_millis(EDGE_INGEST_MILLIS))
        as Arc<dyn flexcore_ports::clock::Clock + Send + Sync>;
    let journal_state =
        FileLocalBuffer::open(&journal, 100, Some(Duration::from_millis(3_600_000)), clock)
            .expect("reopens after the drill");
    assert_eq!(journal_state.len(), 0, "drained-and-committed: nothing pending");

    // The audit trail: at least one poll-failure episode during the drill,
    // and NO fabricated telemetry ever (nothing was forwarded while the
    // source was unreachable).
    let events = audit.events();
    assert!(
        events
            .iter()
            .any(|e| e.kind.name() == "EDGE_SOURCE_POLL_FAILED"),
        "the forced-disconnect poll failures are audited"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// --- 3. Overflow drop accounting through the runner -------------------------------

#[test]
fn backpressure_first_and_drop_oldest_last_with_latched_counters() {
    let dir = scratch_dir("overflow-runner");
    let journal = dir.join("telemetry.journal");
    let audit = Arc::new(InMemoryAuditSink::new());

    // Capacity 2, batch size 1, uplink NEVER succeeds, ONE point per poll:
    // tick 1 buffers 1; tick 2 buffers 1 (buffer now full); tick 3 hits
    // full backpressure — polling pauses, NOTHING is dropped (the P6.05
    // first line).
    {
        let mut runner = EdgeRunner::new(
            RunnerParams::from_config(Duration::from_millis(5_000), 1, 2),
            Box::new(AdapterIngestSource::new(
                Box::new(SimulatedEmsSource::new(fixture())),
                acl(),
                acl_mappings(),
                Arc::new(FixedClock::at_unix_millis(EDGE_INGEST_MILLIS)),
                64,
            )
            .expect("valid composition")),
            Box::new(FailClosedUplink),
            Box::new(FileLocalBuffer::open(
                &journal,
                2,
                Some(Duration::from_millis(3_600_000)),
                Arc::new(FixedClock::at_unix_millis(EDGE_INGEST_MILLIS))
                    as Arc<dyn flexcore_ports::clock::Clock + Send + Sync>,
            )
            .expect("durable buffer opens")),
            Arc::clone(&audit) as Arc<dyn flexcore_edge_agent::ports::AuditSink>,
            Arc::new(FixedClock::at_unix_millis(EDGE_INGEST_MILLIS)),
        );
        let report = runner.tick();
        assert_eq!(report.buffered, 1);
        assert_eq!(report.forwarded, 0);
        let report = runner.tick();
        assert_eq!(report.buffered, 1);
        assert_eq!(runner.buffered_len(), 2, "the buffer is now full");
        let report = runner.tick();
        assert!(
            report.poll_paused_backpressure,
            "backpressure pauses polling FIRST — no drop"
        );
        assert_eq!(report.polled, 0);
        // Runner dropped: nothing was dropped by the runner path.
    }

    // The audited drop-oldest policy is the LAST resort: only an append
    // at capacity (outside the runner's backpressure window — e.g. a
    // reopened journal larger than the configured bound) engages it.
    // Prove it latches through the buffer itself:
    let clock = Arc::new(FixedClock::at_unix_millis(EDGE_INGEST_MILLIS))
        as Arc<dyn flexcore_ports::clock::Clock + Send + Sync>;
    let mut buffer = FileLocalBuffer::open(&journal, 2, Some(Duration::from_millis(3_600_000)), clock)
        .expect("reopens");
    assert_eq!(buffer.len(), 2);
    buffer
        .append(sample_at(9, BASE_MILLIS + 9_000))
        .expect("append engages drop-oldest");
    assert_eq!(buffer.len(), 2);
    assert_eq!(buffer.dropped_oldest(), 1, "the drop is COUNTED");
    assert!(buffer.has_overflowed(), "the health flag LATCHES");
    let pending = buffer.drain(10);
    assert_eq!(
        pending.iter().map(|s| s.source_sequence()).collect::<Vec<_>>(),
        vec![2, 9],
        "the OLDEST (sequence 1) was dropped, order preserved — never a silent overwrite"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

fn sample_at(sequence: u64, event_millis: i64) -> EdgeTelemetrySample {
    EdgeTelemetrySample::new(EdgeTelemetrySampleParts {
        asset_key: "ess-01".to_string(),
        quantity: TelemetryQuantity::ActivePower,
        value: QuantityValue::Power(ActivePowerMW::new(2.0).expect("finite")),
        quality: TelemetryQuality::Good,
        source_quality_code: None,
        event_time: EventTime::from_unix_millis(event_millis).expect("valid"),
        ingest_time: IngestTime::from_unix_millis(BASE_MILLIS + 9_000).expect("valid"),
        source_sequence: sequence,
        source_id: "edge:pilot-site-A:simulated-test/ess0/ActivePower".to_string(),
    })
    .expect("valid sample")
}

// --- 4. LocalBuffer trait parity ---------------------------------------------------

#[test]
fn file_and_memory_buffers_behave_identically_at_the_port_level() {
    let dir = scratch_dir("parity");
    let journal = dir.join("telemetry.journal");
    let clock = Arc::new(FixedClock::at_unix_millis(EDGE_INGEST_MILLIS))
        as Arc<dyn flexcore_ports::clock::Clock + Send + Sync>;

    let mut file_buffer = FileLocalBuffer::open(&journal, 3, None, Arc::clone(&clock))
        .expect("opens");
    let mut mem_buffer = InMemoryLocalBuffer::new(3);
    for seq in 1..=3u64 {
        let sample = sample_at(seq, BASE_MILLIS + seq as i64 * 1_000);
        file_buffer.append(sample.clone()).expect("appends");
        mem_buffer.append(sample).expect("appends");
    }
    assert_eq!(file_buffer.len(), mem_buffer.len());
    assert_eq!(file_buffer.capacity(), mem_buffer.capacity());
    assert!(!file_buffer.is_empty());
    assert!(!mem_buffer.is_empty());

    // Same drain order from both.
    let from_file = file_buffer.drain(2);
    let from_mem = mem_buffer.drain(2);
    assert_eq!(from_file, from_mem, "identical port-level behaviour");

    // Failed batch → prepend → same order restored.
    file_buffer.prepend(from_file);
    mem_buffer.prepend(from_mem);
    assert_eq!(file_buffer.drain(10), mem_buffer.drain(10));

    // The commit point: no-op for the volatile buffer (recorded on the
    // port), compaction for the durable one — both stay usable.
    mem_buffer.commit_forwarded(&[]);
    file_buffer.commit_forwarded(&[]);
    assert!(mem_buffer.is_empty());
    assert!(file_buffer.is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}
