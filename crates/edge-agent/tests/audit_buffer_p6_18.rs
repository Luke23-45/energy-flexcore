// P6.18 — Local audit buffer and offline behaviour: integration tests
//         (ledger P6.18; test file written, NOT run — the loop bans in
//         `ledger/phase-06-edge-protocol/ENTRY_NOTE.md` hold: nothing
//         built, no tests executed. The human build/test pass runs
//         `cargo test -p flexcore-edge-agent --test audit_buffer_p6_18`).
//
// Coverage (the ledger P6.18 step-3 test set):
//
// 1. **Central outage during a command flow** — with the uplink DOWN, the
//    runner enters degraded mode and the P6.14 gate refuses arriving
//    commands; every generated event (gate state, command rejection,
//    degraded transition, poll failures) lands DURABLY in the audit
//    buffer, in order.
// 2. **Edge restart with a non-empty audit buffer** — the records survive
//    the restart and upload IN ORDER after reconnect (sequences persist
//    across the restart; committed records do not redeliver).
// 3. **Audit-buffer overflow** — explicit reported loss: hard audited
//    counter, latched health flag, reconciliation identity
//    (appended == committed + pending + dropped) — never silent, never an
//    in-place overwrite of an un-uploaded record.
// 4. **In-order upload guarantee** — drain always takes the lowest
//    sequence first; an upload-commit that would skip an un-uploaded
//    predecessor is REFUSED (fail-closed, nothing removed).
// 5. **Event-count reconciliation across the outage window** — the
//    ledger's 30-minute outage scenario is expressed here as a BOUNDED
//    SYNTHETIC window (30 one-minute ticks on a fixed clock, ~30 audited
//    events). THE REAL 30-MINUTE LAB RUN IS A HUMAN GATE INPUT AND IS
//    RECORDED AS UNRUN (decision doc: `ledger/phase-06-edge-protocol/
//    18_21_audit_offline_isolation.md` — no lab, no fault injector, and
//    no cargo execution exist in this loop).
//
// The buffer's own unit tests (journal round-trip, out-of-order commit
// refusal, overflow counters, AuditSink mapping) live in
// `audit_buffer.rs`.

use std::sync::Arc;
use std::time::Duration;

use flexcore_edge_agent::audit_buffer::{
    AuditUploadError, BufferedAuditKind, BufferedAuditRecord, FileAuditBuffer,
};
use flexcore_edge_agent::command_path::{
    CommandPathGateInputs, CommandPathRuntime, CommandReceiver, EdgeCommand, EnablementKey,
};
use flexcore_edge_agent::policy::{AdapterPolicy, OperatingWindow, RateLimit, SitePolicyConfig, SitePolicyEngine};
use flexcore_edge_agent::ports::{
    AuditSink, EdgeAuditEvent, EdgeAuditKind, EdgeError, FailClosedEmsSource, FailClosedUplink,
};
use flexcore_edge_agent::runner::{EdgeRunner, RunnerParams};
use flexcore_ports::clock::Clock;
use flexcore_ports::fakes::FixedClock;

const BASE_MILLIS: i64 = 1_700_000_000_000;
const OUTAGE_WINDOW_MILLIS: i64 = 1_800_000; // 30 minutes, the ledger scenario
const TICK_MILLIS: i64 = 60_000; // one-minute supervision ticks

/// A std-only unique scratch directory per call.
fn scratch_dir(label: &str) -> std::path::PathBuf {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "flexcore-edge-p618-it-{}-{}-{}",
        std::process::id(),
        label,
        COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&dir).expect("scratch dir creates");
    dir
}

fn cleanup(dir: &std::path::Path) {
    let _ = std::fs::remove_dir_all(dir);
}

fn clock_at(millis: i64) -> Arc<dyn Clock + Send + Sync> {
    Arc::new(FixedClock::at_unix_millis(millis))
}

fn key() -> EnablementKey {
    EnablementKey::new(
        "not-a-real-key-this-gate-is-default-off".to_string(),
        "deployment:unused-key".to_string(),
    )
    .expect("valid key")
}

/// A permissive site policy for the target adapter (never consulted while
/// the gate is disabled; composed because a receiver cannot be constructed
/// without an independent policy gate — fail-closed by composition).
fn permissive_policy() -> Arc<SitePolicyEngine> {
    Arc::new(
        SitePolicyEngine::new(SitePolicyConfig {
            format_version: 1,
            adapters: vec![AdapterPolicy {
                adapter_id: "simulated-test".to_string(),
                enabled: true,
                rate_limit: RateLimit {
                    max_commands: 100,
                    window_millis: 60_000,
                },
                operating_window: OperatingWindow {
                    start_millis_of_day: 0,
                    end_millis_of_day: 86_400_000,
                },
            }],
        })
        .expect("valid policy"),
    )
}

fn issued_command(id: &str) -> EdgeCommand {
    EdgeCommand::new(id.to_string(), "ess-01".to_string(), "ISSUED".to_string(), 1)
        .expect("valid command")
}

// --- 1. Central outage during a command flow --------------------------------------

#[test]
fn central_outage_during_a_command_flow_events_accumulate_in_order() {
    let dir = scratch_dir("outage-command-flow");
    let journal = dir.join("audit.journal");
    let buffer = Arc::new(FileAuditBuffer::open(&journal, 1_000, clock_at(BASE_MILLIS)).expect("opens"));

    // The command receiver over the DURABLE audit buffer, with the gate at
    // its DEFAULT (disabled): an ISSUED command arriving at the edge is
    // rejected — not queued, not forwarded — and the rejection is audited.
    let runtime = CommandPathRuntime::evaluate(&CommandPathGateInputs::default(), &key(), BASE_MILLIS);
    let mut receiver = CommandReceiver::new(
        runtime,
        permissive_policy(),
        Arc::clone(&buffer) as Arc<dyn AuditSink>,
        clock_at(BASE_MILLIS + 1_000),
    );
    let refusal = receiver
        .receive(&issued_command("cmd-outage-1"), "simulated-test")
        .expect_err("the default build refuses commands");
    assert_eq!(refusal.code(), "command_path_disabled");

    // The supervision loop with the channel DOWN (FailClosedUplink): the
    // poll fails (site EMS unreachable in this synthetic window) and the
    // runner enters degraded buffering mode — both audited durably.
    let mut runner = EdgeRunner::new(
        RunnerParams::from_config(Duration::from_millis(60_000), 10, 1_000),
        Box::new(FailClosedEmsSource),
        Box::new(FailClosedUplink),
        Box::new(flexcore_edge_agent::ports::InMemoryLocalBuffer::new(1_000)),
        Arc::clone(&buffer) as Arc<dyn AuditSink>,
        clock_at(BASE_MILLIS + 2_000),
    );
    let report = runner.tick();
    assert!(report.poll_failed);
    assert!(report.forward_failed);
    assert!(runner.is_degraded(), "channel down = degraded mode");

    // EVERYTHING generated during the outage window is durably in the
    // buffer, in order, with no gaps.
    let pending = buffer.drain(1_000);
    assert!(pending.len() >= 4, "gate state + rejection + degraded entry + poll failure: {}", pending.len());
    let names: Vec<&str> = pending.iter().map(|r| r.kind.name()).collect();
    assert!(
        names.iter().any(|n| *n == "EDGE_COMMAND_REJECTED"),
        "the command refusal is durably audited: {names:?}"
    );
    assert!(
        names.iter().any(|n| *n == "EDGE_DEGRADED_BUFFERING_ENTERED"),
        "the degraded entry is durably audited: {names:?}"
    );
    assert!(
        names.iter().any(|n| *n == "EDGE_SOURCE_POLL_FAILED"),
        "the poll failure is durably audited: {names:?}"
    );
    // In order: strictly increasing sequences, record order == append order.
    for pair in pending.windows(2) {
        assert!(
            pair[0].sequence < pair[1].sequence,
            "sequences strictly increase in append order"
        );
    }
    let rejection = pending
        .iter()
        .find(|r| r.kind == BufferedAuditKind::CommandRejected)
        .expect("the rejection record");
    assert!(
        rejection.detail.contains("command_denial") && rejection.detail.contains("code=command_path_disabled"),
        "the P4-pattern denial record rides in the detail: {}",
        rejection.detail
    );
    assert_eq!(rejection.recorded_at.unix_millis(), (BASE_MILLIS + 1_000) as i128);
    cleanup(&dir);
}

// --- 2. Edge restart with a non-empty audit buffer ---------------------------------

#[test]
fn edge_restart_with_a_non_empty_audit_buffer_uploads_in_order_after_reconnect() {
    let dir = scratch_dir("restart");
    let journal = dir.join("audit.journal");

    // Phase 1 — the outage window: events accumulate; the edge "crashes"
    // before ANY upload is confirmed (nothing committed).
    {
        let buffer = FileAuditBuffer::open(&journal, 1_000, clock_at(BASE_MILLIS)).expect("opens");
        for n in 1..=5u64 {
            buffer
                .record_event(
                    BufferedAuditKind::DegradedBufferingEntered,
                    "edge:pilot-site-A",
                    "uplink",
                    format!("outage event {n}"),
                )
                .expect("records");
        }
        assert_eq!(buffer.pending_len(), 5);
        // Drop WITHOUT commit_uploaded: the records must survive on disk.
    }

    // Phase 2 — reconnect (restart): the buffer replays the journal; the
    // records upload IN ORDER (drain front-first, persisted sequences).
    let reconnect_clock = clock_at(BASE_MILLIS + OUTAGE_WINDOW_MILLIS);
    let buffer = FileAuditBuffer::open(&journal, 1_000, Arc::clone(&reconnect_clock)).expect("reopens");
    assert_eq!(buffer.pending_len(), 5, "the records survive the restart");
    let batch = buffer.drain(5);
    assert_eq!(
        batch.iter().map(|r| r.sequence).collect::<Vec<_>>(),
        vec![1, 2, 3, 4, 5],
        "in-order upload: lowest sequence first, across the restart"
    );
    // The upload is CONFIRMED → commit: the journal compacts to empty.
    let committed = buffer.commit_uploaded(&batch).expect("commits");
    assert_eq!(committed, 5);
    assert_eq!(buffer.pending_len(), 0);

    // Phase 3 — a later restart holds nothing (no redelivery of
    // confirmed uploads).
    drop(buffer);
    let final_state = FileAuditBuffer::open(&journal, 1_000, reconnect_clock).expect("reopens");
    assert_eq!(final_state.pending_len(), 0, "committed records do not redeliver");
    cleanup(&dir);
}

// --- 3. Audit-buffer overflow: explicit reported loss --------------------------------

#[test]
fn audit_buffer_overflow_is_explicitly_reported_never_silent() {
    let dir = scratch_dir("overflow");
    let journal = dir.join("audit.journal");
    let buffer = Arc::new(FileAuditBuffer::open(&journal, 3, clock_at(BASE_MILLIS)).expect("opens"));
    let sink: Arc<dyn AuditSink> = Arc::clone(&buffer);

    // Five events into a capacity-3 buffer: two OLDEST records are dropped
    // under the audited drop-of-oldest policy.
    for n in 1..=5u64 {
        sink.record(EdgeAuditEvent {
            recorded_at: flexcore_contracts::time_model::IngestTime::from_unix_millis(
                BASE_MILLIS + n as i64 * 1_000,
            )
            .expect("valid"),
            kind: EdgeAuditKind::SourcePollFailed,
            detail: format!("overflow drill event {n}"),
        })
        .expect("records");
    }

    assert_eq!(buffer.pending_len(), 3);
    assert_eq!(buffer.dropped_oldest(), 2, "the loss is COUNTED");
    assert!(buffer.has_overflowed(), "the health flag LATCHES");
    // Reconciliation identity: appended == pending + dropped (nothing
    // committed yet) — the loss is explicitly accounted, never silent.
    assert_eq!(
        buffer.appended_total(),
        buffer.pending_len() as u64 + buffer.dropped_oldest(),
    );
    // What survived is the NEWEST, in order (never an in-place overwrite).
    let pending = buffer.drain(10);
    assert_eq!(
        pending.iter().map(|r| r.sequence).collect::<Vec<_>>(),
        vec![3, 4, 5],
        "the OLDEST (1, 2) were dropped and REPORTED, order preserved"
    );
    // The drop persists across a restart: the reopened journal holds the
    // pending set only. (The audited counters are per-open — the recorded
    // limitation — so the REOPENED instance reports no new drop; the loss
    // itself was counted and logged at the moment it happened, and the
    // flagged health state lives in the operator-visible composition.)
    drop(buffer);
    let reopened = FileAuditBuffer::open(&journal, 3, clock_at(BASE_MILLIS)).expect("reopens");
    assert_eq!(reopened.pending_len(), 3, "the surviving set persists");
    assert_eq!(reopened.dropped_oldest(), 0, "no NEW drop at open (3 pending <= capacity 3)");
    cleanup(&dir);
}

// --- 4. In-order upload guarantee -----------------------------------------------------

#[test]
fn upload_is_in_order_only_and_never_skips_an_un_uploaded_predecessor() {
    let dir = scratch_dir("in-order");
    let journal = dir.join("audit.journal");
    let buffer = FileAuditBuffer::open(&journal, 1_000, clock_at(BASE_MILLIS)).expect("opens");
    for n in 1..=6u64 {
        buffer
            .record_event(
                BufferedAuditKind::ForwardBatch,
                "edge:pilot-site-A",
                "uplink",
                format!("batch marker {n}"),
            )
            .expect("records");
    }

    // Batch A (seq 1–2) uploads and commits.
    let batch_a = buffer.drain(2);
    buffer.commit_uploaded(&batch_a).expect("commits");

    // Batch B (seq 3–4) FAILS to upload: prepend back at the front.
    let batch_b = buffer.drain(2);
    assert_eq!(batch_b.iter().map(|r| r.sequence).collect::<Vec<_>>(), vec![3, 4]);
    buffer.prepend(batch_b);

    // The uploader CANNOT jump ahead to seq 5–6 while 3–4 are un-uploaded:
    // an out-of-order commit is refused and removes NOTHING.
    let batch_c = buffer.drain(2);
    assert_eq!(batch_c.iter().map(|r| r.sequence).collect::<Vec<_>>(), vec![5, 6]);
    buffer.prepend(batch_c);
    let jump = vec![BufferedAuditRecord {
        sequence: 5,
        recorded_at: flexcore_contracts::time_model::IngestTime::from_unix_millis(BASE_MILLIS)
            .expect("valid"),
        kind: BufferedAuditKind::ForwardBatch,
        actor: "edge:pilot-site-A".to_string(),
        object: "uplink".to_string(),
        detail: "attempting to skip the un-uploaded 3–4".to_string(),
    }];
    let error = buffer.commit_uploaded(&jump).expect_err("in-order only");
    assert!(matches!(error, AuditUploadError::OutOfOrderCommit { .. }));
    assert_eq!(buffer.pending_len(), 4, "seq 3–6 all retained, in order");

    // Retry batch B: it now uploads, commits, and C follows IN ORDER.
    let retry = buffer.drain(2);
    assert_eq!(retry.iter().map(|r| r.sequence).collect::<Vec<_>>(), vec![3, 4]);
    buffer.commit_uploaded(&retry).expect("commits");
    let tail = buffer.drain(10);
    assert_eq!(
        tail.iter().map(|r| r.sequence).collect::<Vec<_>>(),
        vec![5, 6],
        "no record is uploaded before its predecessors"
    );
    buffer.commit_uploaded(&tail).expect("commits");
    assert_eq!(buffer.pending_len(), 0);
    cleanup(&dir);
}

// --- 5. Event-count reconciliation across the (synthetic) 30-minute window ---------

#[test]
fn outage_window_event_count_reconciles_before_and_after_reconnect() {
    let dir = scratch_dir("reconciliation");
    let journal = dir.join("audit.journal");

    // The ledger's 30-minute central outage, expressed as a BOUNDED
    // SYNTHETIC window: 30 one-minute ticks on a fixed clock, each tick
    // failing its poll (audited) while the buffer stays within capacity.
    // THE REAL 30-MINUTE LAB RUN IS A HUMAN GATE INPUT — RECORDED AS UNRUN
    // (decision doc; no lab exists in this loop, and cargo is banned).
    let clock = Arc::new(FixedClock::at_unix_millis(BASE_MILLIS));
    let buffer = Arc::new(FileAuditBuffer::open(&journal, 1_000, Arc::clone(&clock) as Arc<dyn Clock + Send + Sync>).expect("opens"));
    let mut runner = EdgeRunner::new(
        RunnerParams::from_config(Duration::from_millis(60_000), 10, 1_000),
        Box::new(FailClosedEmsSource),
        Box::new(FailClosedUplink),
        Box::new(flexcore_edge_agent::ports::InMemoryLocalBuffer::new(1_000)),
        Arc::clone(&buffer) as Arc<dyn AuditSink>,
        Arc::clone(&clock) as Arc<dyn Clock + Send + Sync>,
    );
    let mut synthetic_ticks = 0u64;
    for _ in 0..(OUTAGE_WINDOW_MILLIS / TICK_MILLIS) {
        let _ = runner.tick();
        synthetic_ticks += 1;
        clock.advance_millis(TICK_MILLIS);
    }
    assert_eq!(synthetic_ticks, 30, "the synthetic window covers the 30-minute ledger scenario");

    // BEFORE reconnect: every tick produced exactly one audited poll
    // failure (plus one degraded-entry event); nothing was lost within
    // capacity, nothing was dropped.
    let expected_events = synthetic_ticks + 1; // 30 poll failures + 1 degraded entry
    assert!(buffer.dropped_oldest() == 0, "no loss within capacity");
    assert!(!buffer.has_overflowed());
    assert_eq!(buffer.appended_total(), expected_events);

    // Reconnect: drain ALL pending records and reconcile the count.
    let pending = buffer.drain(1_000);
    assert_eq!(
        pending.len() as u64,
        expected_events,
        "event-count reconciliation: every event generated during the outage window is uploaded on reconnect"
    );
    // In order with strictly increasing sequences; the FIRST tick audits
    // the poll failure and then the degraded entry (the runner's poll
    // phase runs before the forward phase), so:
    //   seq 1  = SOURCE_POLL_FAILED   at BASE
    //   seq 2  = DEGRADED_BUFFERING_ENTERED at BASE
    //   seq 3..=31 = one SOURCE_POLL_FAILED per later tick, 60 s apart.
    assert_eq!(
        pending.iter().map(|r| r.sequence).collect::<Vec<_>>(),
        (1..=expected_events).collect::<Vec<_>>(),
        "sequences are dense and ordered across the whole outage window"
    );
    assert_eq!(pending[0].kind, BufferedAuditKind::SourcePollFailed);
    assert_eq!(pending[1].kind, BufferedAuditKind::DegradedBufferingEntered);
    // The timestamps advance with the synthetic window (never re-stamped).
    assert_eq!(pending[0].recorded_at.unix_millis(), BASE_MILLIS as i128);
    assert_eq!(
        pending[2].recorded_at.unix_millis(),
        (BASE_MILLIS + TICK_MILLIS) as i128
    );
    assert_eq!(
        pending.last().expect("non-empty").recorded_at.unix_millis(),
        (BASE_MILLIS + (OUTAGE_WINDOW_MILLIS - TICK_MILLIS)) as i128,
        "the last tick ran one tick-interval before the window closed"
    );

    // The upload is confirmed → commit → a restart holds nothing.
    buffer.commit_uploaded(&pending).expect("commits");
    drop(buffer);
    let after = FileAuditBuffer::open(&journal, 1_000, Arc::clone(&clock) as Arc<dyn Clock + Send + Sync>).expect("reopens");
    assert_eq!(after.pending_len(), 0, "reconciled: zero un-uploaded records remain");
    cleanup(&dir);
}

// --- 6. Command rejections during an outage survive the restart ----------------------

#[test]
fn command_rejections_during_an_outage_are_durably_audited_across_restart() {
    let dir = scratch_dir("rejections-restart");
    let journal = dir.join("audit.journal");

    // Phase 1 — disconnected (the channel is down): three commands arrive
    // on the response side in three separate sessions; the default gate
    // refuses each, audited.
    {
        let buffer = Arc::new(FileAuditBuffer::open(&journal, 1_000, clock_at(BASE_MILLIS)).expect("opens"));
        let runtime = CommandPathRuntime::evaluate(&CommandPathGateInputs::default(), &key(), BASE_MILLIS);
        let mut receiver = CommandReceiver::new(
            runtime,
            permissive_policy(),
            Arc::clone(&buffer) as Arc<dyn AuditSink>,
            clock_at(BASE_MILLIS + 1_000),
        );
        for n in 1..=3u64 {
            let result = receiver.receive(
                &issued_command(&format!("cmd-{n}")),
                "simulated-test",
            );
            assert!(result.is_err(), "no command is admitted while the gate is off");
        }
        // The edge never queues refused commands: the buffer holds audit
        // records only, and the refusal count is exact (3 rejections + 1
        // composition-time gate-state event).
        let rejections = buffer
            .drain(1_000)
            .into_iter()
            .filter(|r| r.kind == BufferedAuditKind::CommandRejected)
            .count();
        assert_eq!(rejections, 3, "every refusal is audited, nothing is queued");
    }

    // Phase 2 — restart after reconnect: the rejection records SURVIVE and
    // upload in order with actor/object/timestamp intact.
    let buffer = FileAuditBuffer::open(&journal, 1_000, clock_at(BASE_MILLIS + OUTAGE_WINDOW_MILLIS)).expect("reopens");
    let pending = buffer.drain(1_000);
    let rejection_records: Vec<&BufferedAuditRecord> = pending
        .iter()
        .filter(|r| r.kind == BufferedAuditKind::CommandRejected)
        .collect();
    assert_eq!(rejection_records.len(), 3, "the rejection evidence survives the restart");
    for (index, record) in rejection_records.iter().enumerate() {
        assert!(
            record.detail.contains("command_denial")
                && record.detail.contains(&format!("cmd-{}", index + 1))
                && record.detail.contains("at="),
            "the P4-pattern denial record carries the command identity and timestamp: {}",
            record.detail
        );
        assert_eq!(record.actor, "edge-agent");
    }
    let committed = buffer.commit_uploaded(&pending).expect("commits");
    assert_eq!(committed, pending.len());
    cleanup(&dir);
}

// --- 7. The runner's degraded-mode evidence composes into the durable buffer ---------

#[test]
fn a_failed_audit_write_is_a_named_error_never_a_silent_skip() {
    // An audit record without actor/object is refused (record_event
    // contract) — the audit-or-refuse discipline: the caller learns the
    // record was NOT recorded, never silently continues.
    let dir = scratch_dir("fail-loud");
    let journal = dir.join("audit.journal");
    let buffer = FileAuditBuffer::open(&journal, 10, clock_at(BASE_MILLIS)).expect("opens");
    let error = buffer
        .record_event(BufferedAuditKind::ScopeDenied, "", "object", "no actor")
        .expect_err("refused");
    assert!(matches!(error, EdgeError::AuditWriteFailed { .. }));
    cleanup(&dir);
}
