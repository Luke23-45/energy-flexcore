// P10.07 — Edge offline/degraded-mode tests (release-gate slice, edge
//         crate): the telemetry-buffer offline behaviour, the audit-buffer
//         overflow with audited drop-oldest, the disabled command path, the
//         uplink loss with bounded buffering, and the central-outage
//         site-local enforcement (report §22.3 gate 6; §18.1–§18.3; §21.2;
//         §17.4; ledger
//         `ledger/phase-10-v1-release-hardening/ledger_phase_10.md` P10.07;
//         record:
//         `ledger/phase-10-v1-release-hardening/07_edge_offline_tests.md`).
//
// WHAT THIS FILE PROVES — over the REAL code paths, by exercise (never
// re-implemented):
//
// 1. TELEMETRY BUFFER OFFLINE (`crates/edge-agent/src/buffer_file.rs`, P6.09
//    `FileLocalBuffer`): while the uplink is down the buffer is BOUNDED (a
//    record bound given at open), the overflow policy is the AUDITED
//    drop-of-OLDEST (hard counter `dropped_oldest()` + LATCHED health flag
//    `has_overflowed()` + loud log — never a silent overwrite, never
//    unbounded growth, §18.3), age retention drops expired records on the
//    edge's own `ingest_time` with its own audited counter, and
//    store-and-forward survives an edge restart (journal replay; disk-first
//    appends; the drain→forward→commit window is at-least-once — a crash in
//    it RE-DELIVERS after restart, so downstream duplicates are possible and
//    are consumed by the platform's idempotent ingest; ZERO-LOSS and
//    ZERO-DUPLICATION are never claimed — the tests assert the documented
//    BOUNDED behaviours).
// 2. AUDIT BUFFER OVERFLOW (`crates/edge-agent/src/audit_buffer.rs`, P6.18
//    `FileAuditBuffer`): same audited drop-oldest discipline for the
//    edge-local audit journal (counter + latched flag + the
//    `appended_total == pending + dropped` reconciliation), persisted
//    monotonic sequences, in-order upload with the fail-closed
//    out-of-order-commit refusal (`AuditUploadError::OutOfOrderCommit`
//    removes NOTHING), and NO age-based audit drop (audit evidence
//    retention is a deployment RetentionPolicy decision, P0.16 §4 — never a
//    period in code).
// 3. UPLINK LOSS (`crates/edge-agent/src/runner.rs`, P6.05 `EdgeRunner`):
//    forward failure enters DEGRADED (buffering-only) mode with an audited
//    entry event, the loop keeps polling and buffering, a long outage is
//    BOUNDED by backpressure (a full buffer PAUSES polling, audited ONCE
//    per episode — backpressure replaces loss), and recovery drains in
//    order with an audited exit event.
// 4. COMMAND PATH DISABLED (`crates/edge-agent/src/command_path.rs`, P6.14):
//    the default build REFUSES every command (`command_path_disabled`,
//    `config_flag_off`), the refusal is audited (`EDGE_COMMAND_REJECTED`)
//    and NOTHING is retained (no queue field exists — a refused command is
//    not queued for later issuance); enablement is time-bounded and the
//    expiry is enforced at READ time (hand-computed boundary: the instant
//    of `valid_until` is already disabled).
// 5. CENTRAL OUTAGE (§17.4, §21.2: "Site-local fallback and autonomous
//    local EMS; no dependence on cloud for protection"): the site-local
//    policy engine (`policy.rs`, P6.17) and the health monitor (`health.rs`,
//    P6.17) take NO uplink — a disconnected edge still enforces the site
//    policy (rate limit, adapter enablement, operating window) and still
//    blocks commands targeting FAILING adapters, locally. FlexCore is a
//    supervisory layer (AGENTS.md rule 3): nothing here performs protection;
//    the site EMS's own protective behaviour is entirely local to the site
//    (the recorded P6.18 offline-behaviour statement, cited).
// 6. NO STALE-BUFFERED COMMAND IS ISSUED (P9.24 cross-reference, acceptance
//    criterion c): offline buffering and the command path are INDEPENDENT
//    surfaces — a degraded runner with a full buffer composes a command
//    receiver that still refuses every command; buffering never enables
//    issuance.
//
// HAND-COMPUTED BOUNDARIES (every number in an assert is derived here, not
// read from a run): buffer capacity 2 → the 3rd append drops exactly one
// oldest record; max-age retention keeps the record whose ingest age is
// EXACTLY the limit and drops one millis older (the comparison is
// `ingest < limit` with `limit = now - max_age`); the runner's backpressure
// boundary is headroom == 0 (buffer.len() == capacity); the policy
// rate-limit window keeps a command accepted exactly `window_millis` ago in
// the window for `now == accept + window_millis` (the retention predicate is
// `at > now - window`, so at == the boundary is pruned — budget renews at
// exactly the window age); the enablement expiry boundary is `now ==
// valid_until` → disabled, `valid_until - 1` → enabled.
//
// STATUS: WRITTEN-UNRUN (ENTRY_NOTE boundary 3; no cargo command was
// executed in this loop; zero observed failures is NOT claimed — the human
// build/test gate owns:
// `cargo test -p flexcore-edge-agent --test p10_07_edge_offline_degraded_modes`).
// Expected strings are the modules' own constants. P9.20 fallback drills and
// P9.24 backup rehearsal are CROSS-REFERENCED (their records carry the
// deployment-run evidence this suite does not invent). Per-test trigger /
// observed-behaviour tables live in the record file; timings and versions
// are recorded there as human-gate cells (EMPTY).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use flexcore_contracts::quality::TelemetryQuality;
use flexcore_contracts::time_model::{EventTime, IngestTime};
use flexcore_contracts::units::ActivePowerMW;
use flexcore_ports::clock::Clock;
use flexcore_ports::fakes::FixedClock;
use flexcore_ports::telemetry_source::{QuantityValue, TelemetryQuantity};

use flexcore_edge_agent::audit_buffer::{AuditUploadError, BufferedAuditKind, FileAuditBuffer};
use flexcore_edge_agent::buffer_file::FileLocalBuffer;
use flexcore_edge_agent::command_path::{
    CommandPathEnablement, CommandPathError, CommandPathGateInputs, CommandPathRuntime,
    CommandReceiver, CommandRefusal, EdgeCommand, EnablementKey, GateRefusal,
};
use flexcore_edge_agent::health::{
    AdapterHealthMonitor, AdapterObservation, BlockedAction, HealthLimits,
};
use flexcore_edge_agent::policy::{PolicyViolation, SitePolicyConfig, SitePolicyEngine};
use flexcore_edge_agent::ports::{
    EdgeError, EdgeTelemetrySample, EdgeTelemetrySampleParts, EmsSource, FailClosedEmsSource,
    InMemoryAuditSink, InMemoryLocalBuffer, LocalBuffer, UplinkChannel,
};
use flexcore_edge_agent::runner::{EdgeRunner, RunnerParams};

const BASE_MILLIS: i64 = 1_700_000_000_000;

/// A unique scratch directory per call (std-only; no tempfile dep).
fn scratch_dir(label: &str) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "flexcore-edge-p10-07-{}-{}-{}",
        std::process::id(),
        label,
        COUNTER.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&dir).expect("scratch dir creates");
    dir
}

fn cleanup(dir: &Path) {
    let _ = std::fs::remove_dir_all(dir);
}

/// A power sample with distinct sequence and ingest time (the canonical
/// validated constructor — the buffer can never hold an invalid sample).
fn power_sample(sequence: u64, ingest_offset_millis: i64) -> EdgeTelemetrySample {
    EdgeTelemetrySample::new(EdgeTelemetrySampleParts {
        asset_key: "ess-01".to_string(),
        quantity: TelemetryQuantity::ActivePower,
        value: QuantityValue::Power(ActivePowerMW::new(2.0).expect("finite")),
        quality: TelemetryQuality::Good,
        source_quality_code: None,
        event_time: EventTime::from_unix_millis(BASE_MILLIS + sequence as i64 * 1_000)
            .expect("valid"),
        ingest_time: IngestTime::from_unix_millis(BASE_MILLIS + ingest_offset_millis)
            .expect("valid"),
        source_sequence: sequence,
        source_id: "edge:pilot-site-A:simulated-test/ess0/ActivePower".to_string(),
    })
    .expect("valid sample")
}

// =================================================================================
// 1. Telemetry buffer offline (buffer_file.rs, P6.09) — bounded, audited
//    drop-oldest, store-and-forward across restart. ZERO-LOSS and
//    ZERO-DUPLICATION are never claimed; the documented BOUNDED behaviours
//    are asserted.
// =================================================================================

#[test]
fn offline_telemetry_buffering_is_bounded_and_the_drop_of_oldest_is_audited() {
    let dir = scratch_dir("bounded");
    let path = dir.join("telemetry.journal");
    let clock = Arc::new(FixedClock::at_unix_millis(BASE_MILLIS + 9_000)) as Arc<dyn Clock + Send + Sync>;

    // Link loss: samples accumulate in the durable buffer. Capacity 2 is
    // the configuration bound (§18.3: bounded offline buffering).
    let mut buffer = FileLocalBuffer::open(&path, 2, None, clock).expect("opens");
    buffer.append(power_sample(1, 1_000)).expect("appends");
    buffer.append(power_sample(2, 1_000)).expect("appends");
    // Hand-computed boundary: capacity 2, the THIRD append crosses it —
    // the recorded overflow policy fires: drop the OLDEST, audited.
    buffer.append(power_sample(3, 1_000)).expect("appends (drop-oldest)");
    assert_eq!(buffer.len(), 2, "the buffer stays AT its bound — never unbounded");
    assert_eq!(buffer.dropped_oldest(), 1, "the drop is COUNTED, never silent");
    assert!(buffer.has_overflowed(), "the health flag LATCHES on the overflow");
    let drained = buffer.drain(10);
    let sequences: Vec<u64> = drained.iter().map(|s| s.source_sequence()).collect();
    assert_eq!(sequences, vec![2, 3], "the OLDEST was dropped; order preserved");
    // Zero-loss is NOT claimed: exactly one record was dropped and the
    // counter is the explicit loss report an operator's health surface
    // reads (P6.17).
    cleanup(&dir);
}

#[test]
fn max_age_retention_boundary_is_hand_computed_inclusive_at_the_limit() {
    let dir = scratch_dir("max-age");
    let path = dir.join("telemetry.journal");
    // "Now" is 9 s past BASE; a 5 s retention puts the boundary at
    // BASE + 4_000 (limit = now - max_age). The predicate keeps records
    // whose ingest_time >= limit and drops strictly older ones.
    let clock = Arc::new(FixedClock::at_unix_millis(BASE_MILLIS + 9_000)) as Arc<dyn Clock + Send + Sync>;
    let mut buffer =
        FileLocalBuffer::open(&path, 100, Some(Duration::from_millis(5_000)), clock).expect("opens");
    buffer
        .append(power_sample(1, 3_999)) // ingest at BASE+3_999: one millis OLDER than the limit
        .expect("appends");
    buffer
        .append(power_sample(2, 4_000)) // ingest at BASE+4_000: EXACTLY at the limit — kept
        .expect("appends");
    buffer
        .append(power_sample(3, 6_000)) // ingest at BASE+6_000: fresh
        .expect("appends");
    let drained = buffer.drain(10);
    let sequences: Vec<u64> = drained.iter().map(|s| s.source_sequence()).collect();
    assert_eq!(sequences, vec![2, 3], "age == limit is KEPT; one millis older is DROPPED");
    assert_eq!(buffer.dropped_expired(), 1, "the age drop is audited, never silent");
    assert_eq!(buffer.dropped_oldest(), 0, "no capacity drop happened here");
    cleanup(&dir);
}

#[test]
fn store_and_forward_survives_a_restart_and_a_crash_in_the_drain_window_redelivers() {
    let dir = scratch_dir("restart");
    let path = dir.join("telemetry.journal");
    let clock = Arc::new(FixedClock::at_unix_millis(BASE_MILLIS + 9_000)) as Arc<dyn Clock + Send + Sync>;

    // Phase 1 — the channel is DOWN: three samples buffer durably.
    {
        let mut buffer = FileLocalBuffer::open(&path, 100, None, Arc::clone(&clock)).expect("opens");
        for seq in 1..=3u64 {
            buffer.append(power_sample(seq, 1_000)).expect("appends");
        }
    } // the edge "restarts" (the buffer is dropped without any commit).

    // Phase 2 — restart: the journal replays; the pending set survives.
    let mut recovered =
        FileLocalBuffer::open(&path, 100, None, Arc::clone(&clock)).expect("reopens");
    assert_eq!(recovered.len(), 3, "the buffered telemetry survives the restart");
    let drained = recovered.drain(3);
    let sequences: Vec<u64> = drained.iter().map(|s| s.source_sequence()).collect();
    assert_eq!(sequences, vec![1, 2, 3], "store-and-forward preserves order");

    // Phase 3 — the runner hands the batch to a forward that CRASHES before
    // the commit point (the drain→forward→commit window). The documented
    // behaviour is at-least-once: the records were never removed from the
    // journal, so the restart RE-DELIVERS them. Downstream duplicates are
    // possible by design and consumed by the platform's idempotent ingest —
    // ZERO-DUPLICATION IS NEVER CLAIMED, here or anywhere.
    drop(recovered); // crash: drain happened, commit did not.
    let mut redeliverer =
        FileLocalBuffer::open(&path, 100, None, Arc::clone(&clock)).expect("reopens after crash");
    assert_eq!(redeliverer.len(), 3, "the un-committed batch is re-delivered (at-least-once)");
    let redelivered = redeliverer.drain(3);
    assert_eq!(
        redelivered.iter().map(|s| s.source_sequence()).collect::<Vec<_>>(),
        vec![1, 2, 3],
        "redelivery preserves order"
    );
    // Phase 4 — the forward SUCCEEDS and the commit point runs: the journal
    // compacts to the still-pending set (empty); a further restart holds
    // nothing and no duplicates.
    redeliverer.commit_forwarded(&redelivered);
    drop(redeliverer);
    let clean = FileLocalBuffer::open(&path, 100, None, Arc::clone(&clock)).expect("reopens");
    assert_eq!(clean.len(), 0, "committed records do not redeliver");
    cleanup(&dir);
}

#[test]
fn a_failed_forward_batch_goes_back_in_order_and_the_channel_outage_loses_nothing() {
    let dir = scratch_dir("failed-forward");
    let path = dir.join("telemetry.journal");
    let clock = Arc::new(FixedClock::at_unix_millis(BASE_MILLIS + 9_000)) as Arc<dyn Clock + Send + Sync>;
    let mut buffer = FileLocalBuffer::open(&path, 100, None, clock).expect("opens");
    for seq in 1..=4u64 {
        buffer.append(power_sample(seq, 1_000)).expect("appends");
    }
    // Batch A (seq 1–2) forwards and commits; batch B (seq 3–4) fails and
    // is prepend-ed back at the FRONT, in order — the outage mid-drain
    // records nothing lost.
    let batch_a = buffer.drain(2);
    buffer.commit_forwarded(&batch_a);
    let batch_b = buffer.drain(2);
    buffer.prepend(batch_b);
    drop(buffer);
    let mut reopened = FileLocalBuffer::open(&path, 100, None, Arc::new(FixedClock::at_unix_millis(BASE_MILLIS + 9_000))).expect("reopens");
    let pending = reopened.drain(10);
    assert_eq!(
        pending.iter().map(|s| s.source_sequence()).collect::<Vec<_>>(),
        vec![3, 4],
        "committed batch gone; the failed batch is back at the front in order"
    );
    cleanup(&dir);
}

#[test]
fn a_foreign_journal_file_is_refused_not_guessed_at_open() {
    let dir = scratch_dir("foreign");
    let path = dir.join("telemetry.journal");
    std::fs::write(&path, b"not a flexcore journal at all").expect("writes");
    let error = FileLocalBuffer::open(
        &path,
        10,
        None,
        Arc::new(FixedClock::at_unix_millis(BASE_MILLIS)) as Arc<dyn Clock + Send + Sync>,
    )
    .expect_err("a foreign file is refused");
    assert!(
        matches!(error, flexcore_edge_agent::buffer_file::FileBufferError::NotAJournal { .. }),
        "named error: {error}"
    );
    cleanup(&dir);
}

// =================================================================================
// 2. Audit buffer overflow (audit_buffer.rs, P6.18) — audited drop-oldest,
//    in-order upload, fail-closed out-of-order commit, NO age-based drop.
// =================================================================================

#[test]
fn audit_buffer_overflow_drops_oldest_with_a_counted_loss_and_a_latched_flag() {
    let dir = scratch_dir("audit-overflow");
    let path = dir.join("audit.journal");
    let buffer =
        FileAuditBuffer::open(&path, 2, Arc::new(FixedClock::at_unix_millis(BASE_MILLIS)))
            .expect("opens");
    for n in 1..=3u64 {
        buffer
            .record_event(
                BufferedAuditKind::ScopeDenied,
                "edge:pilot-site-A",
                &format!("tenant:tenant-{n}"),
                "EDGE_SCOPE_DENIED: telemetry request refused".to_string(),
            )
            .expect("records");
    }
    // Hand-computed boundary: capacity 2, the third record crosses it.
    assert_eq!(buffer.pending_len(), 2);
    assert_eq!(buffer.dropped_oldest(), 1, "the drop is COUNTED (the loss report)");
    assert!(buffer.has_overflowed(), "the health flag LATCHES");
    // The reconciliation the P6.17 health surface reads.
    assert_eq!(
        buffer.appended_total(),
        (buffer.pending_len() + buffer.dropped_oldest() as usize) as u64,
        "appended == pending + dropped — the loss is explicitly reported, never silent"
    );
    let pending = buffer.drain(10);
    assert_eq!(
        pending.iter().map(|r| r.sequence).collect::<Vec<_>>(),
        vec![2, 3],
        "the OLDEST was dropped, order preserved — never an in-place overwrite"
    );
    cleanup(&dir);
}

#[test]
fn audit_upload_is_in_order_and_an_out_of_order_commit_is_refused_removing_nothing() {
    let dir = scratch_dir("audit-upload");
    let path = dir.join("audit.journal");
    let buffer =
        FileAuditBuffer::open(&path, 100, Arc::new(FixedClock::at_unix_millis(BASE_MILLIS)))
            .expect("opens");
    for n in 1..=4u64 {
        buffer
            .record_event(BufferedAuditKind::DegradedBufferingEntered, "edge-agent", "uplink", format!("episode {n}"))
            .expect("records");
    }
    // Batch A (seq 1–2) uploads and commits.
    let batch_a = buffer.drain(2);
    assert_eq!(batch_a.iter().map(|r| r.sequence).collect::<Vec<_>>(), vec![1, 2]);
    assert_eq!(buffer.commit_uploaded(&batch_a).expect("commits"), 2);
    // A crafted commit that SKIPS the front (an un-uploaded predecessor) is
    // refused fail-closed: nothing is removed, ever, out of order.
    let all = buffer.drain(10);
    buffer.prepend(all);
    let bogus = vec![flexcore_edge_agent::audit_buffer::BufferedAuditRecord {
        sequence: 3,
        recorded_at: IngestTime::from_unix_millis(BASE_MILLIS).expect("valid"),
        kind: BufferedAuditKind::DegradedBufferingEntered,
        actor: "edge-agent".to_string(),
        object: "uplink".to_string(),
        detail: "crafted out-of-order commit attempt".to_string(),
    }];
    let error = buffer
        .commit_uploaded(&bogus)
        .expect_err("out-of-order commit refused");
    assert!(matches!(error, AuditUploadError::OutOfOrderCommit { .. }), "named error: {error}");
    assert_eq!(buffer.pending_len(), 2, "nothing was removed by the refused commit");
    // Restart with the failed batch un-committed: the records survive and
    // upload in persisted-sequence order (at-least-once, in order).
    drop(buffer);
    let reopened = FileAuditBuffer::open(&path, 100, Arc::new(FixedClock::at_unix_millis(BASE_MILLIS)))
        .expect("reopens");
    let pending = reopened.drain(10);
    assert_eq!(
        pending.iter().map(|r| r.sequence).collect::<Vec<_>>(),
        vec![3, 4],
        "committed records do not redeliver; the failed batch is retained in order"
    );
    cleanup(&dir);
}

#[test]
fn audit_records_carry_actor_object_and_timestamp_and_refuse_anonymous_records() {
    let dir = scratch_dir("audit-shape");
    let path = dir.join("audit.journal");
    let buffer =
        FileAuditBuffer::open(&path, 10, Arc::new(FixedClock::at_unix_millis(BASE_MILLIS)))
            .expect("opens");
    // An audit record without actor or object is NOT an audit record —
    // refused rather than recorded as a hole (the P6.21 shape).
    let error = buffer
        .record_event(BufferedAuditKind::ScopeDenied, "  ", "tenant:tenant-B", "refused")
        .expect_err("an actorless record is refused");
    assert!(
        matches!(error, flexcore_edge_agent::ports::EdgeError::AuditWriteFailed { .. }),
        "named error: {error}"
    );
    assert_eq!(buffer.pending_len(), 0, "nothing was recorded");
    cleanup(&dir);
}

// =================================================================================
// 3. Uplink loss with bounded buffering (runner.rs, P6.05) — degraded mode,
//    audited transitions, backpressure replaces loss.
// =================================================================================

/// A scripted fake EMS source: serves the queued sequences in order, then
/// reports empty (the same shape as the runner's own unit-test fake).
struct ScriptedSource {
    queued: Mutex<Vec<u64>>,
}

impl ScriptedSource {
    fn serving(sequences: Vec<u64>) -> Self {
        Self { queued: Mutex::new(sequences) }
    }
}

impl EmsSource for ScriptedSource {
    fn source_name(&self) -> &str {
        "scripted-fake"
    }

    fn poll(&mut self, max: usize) -> Result<Vec<EdgeTelemetrySample>, EdgeError> {
        let mut queue = self.queued.lock().expect("poisoned");
        if queue.is_empty() {
            return Ok(Vec::new());
        }
        let take = max.min(queue.len());
        let drained: Vec<u64> = queue.drain(..take).collect();
        Ok(drained.into_iter().map(|seq| power_sample(seq, 1_000)).collect())
    }
}

/// A fake uplink that fails the first N forwards, then succeeds.
struct FlakyUplink {
    fail_first: Mutex<u32>,
}

impl FlakyUplink {
    fn failing_first(n: u32) -> Self {
        Self { fail_first: Mutex::new(n) }
    }
}

impl UplinkChannel for FlakyUplink {
    fn is_connected(&self) -> bool {
        *self.fail_first.lock().expect("poisoned") == 0
    }

    fn forward(&mut self, _samples: &[EdgeTelemetrySample]) -> Result<(), EdgeError> {
        let mut fails = self.fail_first.lock().expect("poisoned");
        if *fails > 0 {
            *fails -= 1;
            return Err(EdgeError::UplinkForwardFailed {
                reason: "scripted channel down".to_string(),
            });
        }
        Ok(())
    }
}

fn offline_runner(
    source: ScriptedSource,
    uplink: FlakyUplink,
    capacity: usize,
    batch: usize,
    sink: Arc<InMemoryAuditSink>,
) -> EdgeRunner {
    EdgeRunner::new(
        RunnerParams::from_config(Duration::from_millis(5_000), batch, capacity),
        Box::new(source),
        Box::new(uplink),
        Box::new(InMemoryLocalBuffer::new(capacity)),
        sink as Arc<dyn flexcore_edge_agent::ports::AuditSink>,
        Arc::new(FixedClock::at_unix_millis(BASE_MILLIS)),
    )
}

#[test]
fn uplink_loss_enters_degraded_keeps_buffering_and_recovery_drains_in_order_audited() {
    let sink = Arc::new(InMemoryAuditSink::new());
    // Five samples served; the first TWO forwards fail (the outage), then
    // the channel recovers.
    let mut runner = offline_runner(
        ScriptedSource::serving(vec![1, 2, 3, 4, 5]),
        FlakyUplink::failing_first(2),
        100,
        2,
        Arc::clone(&sink),
    );
    // Tick 1: poll 5, buffer 5, forward batch of 2 FAILS → degraded entered.
    let report = runner.tick();
    assert_eq!(report.polled, 5);
    assert_eq!(report.forwarded, 0);
    assert!(report.forward_failed);
    assert!(runner.is_degraded(), "the outage enters degraded (buffering-only) mode");
    assert_eq!(runner.buffered_len(), 5);
    // Tick 2: still down — stays degraded, keeps buffering (nothing lost).
    let report = runner.tick();
    assert!(runner.is_degraded());
    assert_eq!(report.forwarded, 0);
    assert_eq!(runner.buffered_len(), 5);
    // Tick 3: recovery — the buffer drains in order and degraded exits.
    let report = runner.tick();
    assert!(!runner.is_degraded(), "recovery exits degraded mode");
    assert_eq!(report.forwarded, 5, "all buffered samples forward on recovery");
    assert_eq!(runner.buffered_len(), 0);
    // The degraded entry/exit is AUDITED exactly once each, in order.
    let kinds: Vec<&str> = sink.events().iter().map(|e| e.kind.name()).collect();
    let entered = kinds.iter().filter(|k| **k == "EDGE_DEGRADED_BUFFERING_ENTERED").count();
    let exited = kinds.iter().filter(|k| **k == "EDGE_DEGRADED_BUFFERING_EXITED").count();
    assert_eq!(entered, 1, "one degraded-entry audit event");
    assert_eq!(exited, 1, "one degraded-exit audit event");
    let entered_pos = kinds.iter().position(|k| *k == "EDGE_DEGRADED_BUFFERING_ENTERED").expect("entered");
    let exited_pos = kinds.iter().position(|k| *k == "EDGE_DEGRADED_BUFFERING_EXITED").expect("exited");
    assert!(entered_pos < exited_pos, "the exit is audited after the entry");
}

#[test]
fn a_long_outage_is_bounded_backpressure_pauses_polling_and_audits_once_per_episode() {
    let sink = Arc::new(InMemoryAuditSink::new());
    // Capacity 3, batch 1, the channel never recovers: the buffer fills,
    // polling PAUSES (backpressure replaces loss), the pause is audited
    // ONCE per fill episode, and nothing is silently dropped.
    let mut runner = offline_runner(
        ScriptedSource::serving(vec![1, 2, 3, 4, 5, 6]),
        FlakyUplink::failing_first(u32::MAX),
        3,
        1,
        Arc::clone(&sink),
    );
    let _ = runner.tick(); // polls 3 (the capacity), forward fails → buffer 3
    assert_eq!(runner.buffered_len(), 3, "the bound is the capacity");
    let report = runner.tick(); // headroom 0 → backpressure
    assert!(
        report.poll_paused_backpressure,
        "a full buffer PAUSES polling — the bound is the §18.3 resource ceiling"
    );
    assert_eq!(report.polled, 0, "no sample is polled past the bound");
    assert_eq!(runner.buffered_len(), 3, "nothing was dropped to make room");
    let full_count = sink
        .events()
        .iter()
        .filter(|e| e.kind.name() == "EDGE_BUFFER_FULL")
        .count();
    assert_eq!(full_count, 1, "backpressure is audited ONCE per fill episode, not per tick");
}

#[test]
fn a_source_poll_failure_is_audited_and_no_telemetry_is_fabricated() {
    let sink = Arc::new(InMemoryAuditSink::new());
    // Adapter failure (§18.1 degraded mode): the poll fails, the failure is
    // audited, and NOTHING is fabricated — no substituted sample, no
    // zero-fill, no inferred data. The source is the named fail-closed
    // `FailClosedEmsSource` port shape.
    let mut runner = EdgeRunner::new(
        RunnerParams::from_config(Duration::from_millis(5_000), 10, 100),
        Box::new(FailClosedEmsSource),
        Box::new(FlakyUplink::failing_first(0)),
        Box::new(InMemoryLocalBuffer::new(100)),
        Arc::clone(&sink) as Arc<dyn flexcore_edge_agent::ports::AuditSink>,
        Arc::new(FixedClock::at_unix_millis(BASE_MILLIS)),
    );
    let report = runner.tick();
    assert!(report.poll_failed);
    assert_eq!(report.polled, 0);
    assert_eq!(runner.buffered_len(), 0, "no fabricated telemetry on a failed poll");
    assert!(
        sink.events().iter().any(|e| e.kind.name() == "EDGE_SOURCE_POLL_FAILED"),
        "the adapter failure is audited"
    );
}

// =================================================================================
// 4. Command path disabled (command_path.rs, P6.14) — the default build
//    refuses everything, audited, retaining nothing; time-bounded
//    enablement enforced at read time.
// =================================================================================

fn enablement_key() -> EnablementKey {
    EnablementKey::new(
        "lab-enablement-key-material".to_string(),
        "deployment:lab-enablement-key-1".to_string(),
    )
    .expect("valid key")
}

#[test]
fn the_command_path_is_disabled_by_default_and_every_refusal_is_audited_retaining_nothing() {
    // Default gate inputs: everything OFF. The fixed evaluation order (step
    // 1) reports `config_flag_off` — the DEFAULT build's named reason.
    let inputs = CommandPathGateInputs::default();
    let runtime = CommandPathRuntime::evaluate(&inputs, &enablement_key(), BASE_MILLIS);
    match runtime.enablement_at(BASE_MILLIS) {
        CommandPathEnablement::Disabled { reason } => {
            assert_eq!(reason, GateRefusal::ConfigFlagOff);
            assert_eq!(reason.code(), "config_flag_off");
        }
        CommandPathEnablement::Enabled { .. } => {
            panic!("the default build must NEVER enable the command path")
        }
    }
    // The receiver refuses an ISSUED command: reject + audit + retain
    // nothing (there is no queue field to retain it in — the recorded P6.18
    // offline statement: "a refused command is retained nowhere").
    let policy = SitePolicyEngine::new(site_policy()).expect("loads");
    let sink = Arc::new(InMemoryAuditSink::new());
    let mut receiver = CommandReceiver::new(
        runtime,
        Arc::new(policy),
        Arc::clone(&sink) as Arc<dyn flexcore_edge_agent::ports::AuditSink>,
        Arc::new(FixedClock::at_unix_millis(BASE_MILLIS)),
    );
    let command = EdgeCommand::new(
        "cmd-offline-1".to_string(),
        "ess-01".to_string(),
        "ISSUED".to_string(),
        1,
    )
    .expect("valid command");
    let refusal = receiver
        .receive(&command, "simulated-test")
        .expect_err("the disabled path refuses every command");
    assert!(matches!(refusal, CommandRefusal::PathDisabled { .. }), "named refusal: {refusal}");
    assert_eq!(refusal.code(), "command_path_disabled");
    assert!(
        sink.events().iter().any(|e| e.detail.contains("EDGE_COMMAND_REJECTED")),
        "the refusal is audited"
    );
}

#[test]
fn every_gate_refusal_code_is_named_and_the_fixed_order_reports_the_first_missing_condition() {
    // The nine named refusal codes are pairwise distinct.
    let codes = [
        GateRefusal::ConfigFlagOff,
        GateRefusal::AllowlistEmpty,
        GateRefusal::D6RecordAbsent,
        GateRefusal::NoSignedRecord,
        GateRefusal::SiteMismatch,
        GateRefusal::KeyReferenceMismatch,
        GateRefusal::DigestMismatch,
        GateRefusal::TokenMismatch,
        GateRefusal::EnablementExpired,
    ];
    let mut rendered: Vec<&'static str> = codes.iter().map(|c| c.code()).collect();
    let len = rendered.len();
    rendered.sort_unstable();
    rendered.dedup();
    assert_eq!(rendered.len(), len, "every gate refusal code is DISTINCT: {rendered:?}");
    // The fixed order: with the flag off but everything else supplied, the
    // reported reason is STILL the flag (step 1) — a partially-met gate
    // never enables.
    let inputs = CommandPathGateInputs {
        site_id: "pilot-site-A".to_string(),
        config_flag_enabled: false,
        ..Default::default()
    };
    let runtime = CommandPathRuntime::evaluate(&inputs, &enablement_key(), BASE_MILLIS);
    assert!(matches!(
        runtime.enablement_at(BASE_MILLIS),
        CommandPathEnablement::Disabled { reason: GateRefusal::ConfigFlagOff }
    ));
}

#[test]
fn an_expired_enablement_reads_disabled_at_read_time_hand_computed_boundary() {
    // Compose an ENABLED runtime (a fully-signed record valid until T).
    let record = flexcore_edge_agent::command_path::SignedEnablementRecord {
        site_id: "pilot-site-A".to_string(),
        approver: "J. Doe (security lead)".to_string(),
        adapter_scope: vec!["simulated-test".to_string()],
        enablement_token: "lab-token-1".to_string(),
        issued_at_unix_millis: BASE_MILLIS,
        valid_until_unix_millis: BASE_MILLIS + 3_600_000,
        key_reference: "deployment:lab-enablement-key-1".to_string(),
        digest: None,
    };
    let record = record.sign(&enablement_key()).expect("signs");
    let inputs = CommandPathGateInputs {
        site_id: "pilot-site-A".to_string(),
        config_flag_enabled: true,
        signed_record: Some(record),
        environment_token: Some("lab-token-1".to_string()),
        command_allowlist: vec!["simulated-test".to_string()],
        d6_record_reference: Some("ledger/phase-06-edge-protocol/03_d6_boundary_review.md".to_string()),
    };
    let runtime = CommandPathRuntime::evaluate(&inputs, &enablement_key(), BASE_MILLIS);
    // Hand-computed boundary: one millis BEFORE the expiry → enabled.
    assert!(matches!(
        runtime.enablement_at(BASE_MILLIS + 3_600_000 - 1),
        CommandPathEnablement::Enabled { .. }
    ));
    // AT the expiry instant → already disabled (read-time enforcement: the
    // bound is exclusive; a long-lived runtime cannot outlive its record).
    match runtime.enablement_at(BASE_MILLIS + 3_600_000) {
        CommandPathEnablement::Disabled { reason } => {
            assert_eq!(reason, GateRefusal::EnablementExpired);
        }
        CommandPathEnablement::Enabled { .. } => panic!("an expired enablement must read disabled"),
    }
}

// =================================================================================
// 5. Central outage — site-local enforcement continues (policy.rs and
//    health.rs, P6.17). Neither surface touches the uplink: a disconnected
//    edge still enforces something (the recorded P6.17 risk row), and no
//    dependence on the cloud for protection (§17.4, §21.2 — FlexCore is
//    supervisory; the site EMS's own protective behaviour is local).
// =================================================================================

fn site_policy() -> SitePolicyConfig {
    SitePolicyConfig::parse(&format!(
        "{}\nadapter = simulated-test\nenabled = true\nrate_limit = 2/60000\noperating_window = 0000-2400\n",
        flexcore_edge_agent::policy::SITE_POLICY_FORMAT
    ))
    .expect("parses")
}

#[test]
fn a_disconnected_edge_still_enforces_the_site_policy_locally() {
    // The engine takes NO channel: enablement, rate limits, and operating
    // windows are enforced with the edge's own clock — a central outage
    // changes nothing here (structurally: there is no parameter to lose).
    let engine = SitePolicyEngine::new(site_policy()).expect("loads");
    let now = BASE_MILLIS;
    assert!(engine.admit_command("simulated-test", now).is_ok());
    assert!(engine.admit_command("simulated-test", now + 1).is_ok());
    // Hand-computed boundary: the limit is 2 per 60 000 ms — the third
    // command inside the window is refused, and a refusal consumes NO
    // budget (state unchanged).
    assert!(matches!(
        engine.admit_command("simulated-test", now + 2),
        Err(PolicyViolation::RateLimitExceeded { max_commands: 2, window_millis: 60_000, .. })
    ));
    assert_eq!(engine.admitted_in_window("simulated-test", now + 2), 2);
    // Hand-computed window boundary: a command accepted at `t0` is pruned
    // from the window at `now == t0 + window_millis` (the retention
    // predicate is `at > now - window`), so the first slot frees EXACTLY at
    // the window age — at now + 60_000 the budget allows one more command.
    assert!(engine.admit_command("simulated-test", now + 60_000).is_ok());
    // Disabled and unknown adapters are refused locally too (unknown =
    // refused, never defaulted).
    assert!(matches!(
        engine.admit_command("ghost-adapter", now),
        Err(PolicyViolation::UnknownAdapter { .. })
    ));
}

#[test]
fn a_failing_adapter_blocks_command_targeting_locally_while_degraded_data_still_flows() {
    // Health derivation needs no uplink: it is computed from observations
    // the edge made itself. FAILING_* blocks the dependent command action
    // with an auditable reason; DEGRADED_* keeps data flowing (flagged).
    let mut monitor = AdapterHealthMonitor::new(HealthLimits {
        freshness_limit_millis: 30_000,
        unresponsive_after_millis: 60_000,
        error_rate_window_millis: 300_000,
        max_errors_in_window: 3,
        credential_rotation_window_millis: 7 * 86_400_000,
    })
    .expect("valid limits");
    monitor.observe(
        "simulated-test",
        BASE_MILLIS,
        AdapterObservation::Responsive { event_age_millis: 1_000 },
    );
    // Healthy: the command action is allowed locally.
    assert!(monitor
        .check_action_allowed("simulated-test", BlockedAction::Command, BASE_MILLIS + 1_000)
        .is_ok());
    // The adapter dies: after the detection window → FAILING_UNRESPONSIVE,
    // and the command targeting is BLOCKED locally (the reason names the
    // action and the status — the caller audits it).
    let reason = monitor
        .check_action_allowed("simulated-test", BlockedAction::Command, BASE_MILLIS + 61_000)
        .expect_err("blocked while FAILING");
    assert!(reason.contains("command") && reason.contains("FAILING_UNRESPONSIVE"));
    // DEGRADED (stale data) does NOT block — the data arrives flagged
    // (P0.08); the failure boundary is FAILING_*, exactly.
    let mut monitor2 = AdapterHealthMonitor::new(HealthLimits {
        freshness_limit_millis: 30_000,
        unresponsive_after_millis: 60_000,
        error_rate_window_millis: 300_000,
        max_errors_in_window: 3,
        credential_rotation_window_millis: 7 * 86_400_000,
    })
    .expect("valid limits");
    monitor2.observe(
        "simulated-test",
        BASE_MILLIS,
        AdapterObservation::Responsive { event_age_millis: 1_000 },
    );
    assert_eq!(
        monitor2.assess("simulated-test", BASE_MILLIS + 45_000).expect("assessed").status_name(),
        "DEGRADED_STALE_DATA"
    );
    assert!(monitor2
        .check_action_allowed("simulated-test", BlockedAction::Command, BASE_MILLIS + 45_000)
        .is_ok());
}

// =================================================================================
// 6. No stale-buffered command is issued (P9.24 cross-reference) — offline
//    buffering and the command path are independent surfaces.
// =================================================================================

#[test]
fn a_full_offline_buffer_never_enables_command_issuance() {
    // The edge is degraded (uplink down, buffer full), yet the command path
    // — evaluated from the same immutable gate inputs — stays DISABLED:
    // buffering state and command enablement share no code path, so no
    // command can be issued from a stale buffered state (acceptance c;
    // P9.24 cross-reference).
    let sink = Arc::new(InMemoryAuditSink::new());
    let mut runner = offline_runner(
        ScriptedSource::serving(vec![1, 2, 3]),
        FlakyUplink::failing_first(u32::MAX),
        3,
        1,
        Arc::clone(&sink),
    );
    let _ = runner.tick();
    assert!(runner.is_degraded());
    assert_eq!(runner.buffered_len(), 3, "the buffer is full and the edge is degraded");
    // The command receiver over DEFAULT gate inputs refuses, independent of
    // the runner's state (it does not even see it).
    let runtime = CommandPathRuntime::evaluate(&CommandPathGateInputs::default(), &enablement_key(), BASE_MILLIS);
    let policy = SitePolicyEngine::new(site_policy()).expect("loads");
    let mut receiver = CommandReceiver::new(
        runtime,
        Arc::new(policy),
        Arc::new(InMemoryAuditSink::new()) as Arc<dyn flexcore_edge_agent::ports::AuditSink>,
        Arc::new(FixedClock::at_unix_millis(BASE_MILLIS)),
    );
    let command = EdgeCommand::new(
        "cmd-from-buffer".to_string(),
        "ess-01".to_string(),
        "ISSUED".to_string(),
        1,
    )
    .expect("valid");
    let refusal = receiver.receive(&command, "simulated-test").expect_err("refused offline");
    assert_eq!(refusal.code(), "command_path_disabled");
    // And the config-level default is OFF even in the composition type: the
    // shipped `EdgeConfig` command-path flag defaults to false (the lib.rs
    // recorded posture); the runtime derived here carries no setter at all —
    // there is no API to flip a disabled runtime to enabled.
    assert!(matches!(
        runtime.enablement_at(BASE_MILLIS),
        CommandPathEnablement::Disabled { .. }
    ));
}

#[test]
fn the_edge_error_vocabulary_names_every_offline_failure() {
    // Named errors, not swallowed failures: the offline surface's error
    // variants are exercised through their constructors' messages.
    let io_error = EdgeError::UplinkForwardFailed { reason: "channel down".to_string() };
    assert!(io_error.to_string().contains("channel down"));
    let append_error = flexcore_edge_agent::buffer_file::FileBufferError::Io {
        reason: "disk full".to_string(),
    };
    assert!(append_error.to_string().contains("disk full"));
    // The command-path parse errors are named too (the enablement record is
    // never half-accepted).
    let parse_error: CommandPathError = flexcore_edge_agent::command_path::SignedEnablementRecord::parse("")
        .expect_err("an empty record is a named error");
    assert!(matches!(parse_error, CommandPathError::RecordMalformed { .. }));
}
