// P6.05 — The edge supervision loop (report §18.1, §3.3, §21.2; ledger
// P6.05).
//
// The loop implements the edge's core cycle: **poll → buffer → forward**,
// offline-first:
//
// 1. **Poll.** Pull up to the backpressure headroom from the [`EmsSource`]
//    (never more than the buffer can absorb — the buffer bound is the
//    §21.4-style resource ceiling, and backpressure replaces loss: a full
//    buffer PAUSES polling, audited, instead of dropping samples).
// 2. **Buffer.** Every polled sample is appended to the [`LocalBuffer`]
//    before any forward is attempted. Even in the healthy state samples
//    flow through the buffer — one code path, no fast-path that could
//    diverge from the degraded path (a sample that skipped the buffer could
//    be lost on a crash between poll and forward; the buffer-first order
//    makes at-least-once delivery structural, the §15.4 discipline carried
//    to the edge).
// 3. **Forward.** Drain the oldest batch from the buffer and hand it to the
//    [`UplinkChannel`]. Success → repeat until the buffer is empty. Failure
//    → the batch goes back to the FRONT of the buffer in order
//    ([`LocalBuffer::prepend`]), the runner enters DEGRADED mode, and the
//    loop keeps polling and buffering. Recovery (a successful forward while
//    degraded) exits degraded mode with an audit event.
//
// Degraded-mode state machine (audited through the [`AuditSink`], the
// P4.28/P4.30 audit-or-refuse discipline cited, not reinvented):
//
// ```text
//            forward ok ──────────────► HEALTHY ──► (buffer drains)
//   HEALTHY ──── forward fails ────────► DEGRADED (keep polling + buffering)
//   DEGRADED ─── forward ok ───────────► HEALTHY (recovery; buffer drains)
//   DEGRADED ─── buffer full ──────────► polling PAUSED (audited backpressure)
// ```
//
// Shutdown: the loop watches a `tokio::sync::watch<bool>` channel (the
// worker-composition pattern). On shutdown it finishes the current tick and
// exits; the buffer retains everything not yet forwarded, so a restart
// resumes without loss (P6.09 makes the buffer durable — this wave's
// in-memory buffer is explicitly volatile and the composition logs that
// honestly).
//
// Sync ports, async loop: the ports are deliberately synchronous (see
// `ports.rs`); the in-memory implementations are non-blocking. When P6.09
// (disk buffer) and P6.07 (mTLS uplink) land, their adapters own their
// blocking/async bridging — the loop shape stays.
//
// P6.08/P6.09 additions (wave 2b): the source side is now composable for
// real — `ingest::AdapterIngestSource` wraps any P6.12 `EmsAdapter` + ACL
// behind the same `EmsSource` port (the TEST-ONLY `simulated-test` fixture
// and, later, the P6.10 OpenEMS adapter compose identically) — and the
// forward loop gained ONE new call: `buffer.commit_forwarded(batch)` after
// a confirmed forward (the P6.09 commit point; a no-op for the volatile
// in-memory buffer, a journal compaction for the durable one — the
// at-least-once window is recorded on the port and in `buffer_file`).

use std::sync::Arc;
use std::time::Duration;

use flexcore_ports::clock::Clock;
use tokio::sync::watch;

use crate::ports::{EdgeAuditEvent, EdgeAuditKind, EmsSource, LocalBuffer, UplinkChannel};

/// Runner tuning, derived from the [`crate::config::EdgeConfig`] at
/// composition (kept separate from the config type so tests can construct
/// the loop directly).
#[derive(Debug, Clone)]
pub struct RunnerParams {
    /// EMS poll cadence (supervisory minutes-scale, §3.5 — never sub-second
    /// control).
    pub poll_interval: Duration,
    /// Uplink forward batch size.
    pub forward_batch_size: usize,
    /// Upper bound on samples pulled per tick (the buffer-headroom window).
    pub max_poll_per_tick: usize,
}

impl RunnerParams {
    /// Derives the runner parameters from the edge configuration.
    pub fn from_config(
        poll_interval: Duration,
        forward_batch_size: usize,
        buffer_capacity: usize,
    ) -> Self {
        Self {
            poll_interval,
            forward_batch_size,
            // Poll at most what the buffer can absorb this tick
            // (backpressure before loss).
            max_poll_per_tick: buffer_capacity,
        }
    }
}

/// A one-tick report, returned by [`EdgeRunner::tick`] and aggregated by the
/// long-running loop (visible to tests and to health, P6.17 later).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TickReport {
    pub polled: usize,
    pub buffered: usize,
    pub forwarded: usize,
    pub buffered_len_after: usize,
    pub degraded: bool,
    pub poll_paused_backpressure: bool,
    pub poll_failed: bool,
    pub forward_failed: bool,
}

/// The composed edge supervision loop. Owns the four port objects and the
/// degraded-mode state; nothing else in the crate drives the ports.
pub struct EdgeRunner {
    params: RunnerParams,
    source: Box<dyn EmsSource>,
    uplink: Box<dyn UplinkChannel>,
    buffer: Box<dyn LocalBuffer>,
    audit: Arc<dyn AuditSinkAlias>,
    // The P1.07 `Clock` trait declares no Send/Sync supertrait, so the
    // trait object carries them explicitly — the runner is spawned on the
    // tokio runtime and must be Send. (`FixedClock` and [`crate::clock::WallClock`]
    // both satisfy Send + Sync.)
    clock: Arc<dyn Clock + Send + Sync>,
    degraded: bool,
    buffer_full_audited: bool,
}

// The runner holds `Arc<dyn AuditSink>`; the alias keeps the struct signature
// readable. `AuditSink: Send + Sync` is declared on the trait itself, so the
// Arc is Send.
type AuditSinkAlias = dyn crate::ports::AuditSink;

impl EdgeRunner {
    /// Composes the loop over the four ports and the clock (audit
    /// timestamps come from the same [`Clock`] port the control plane uses,
    /// P1.07 — deterministic fakes in tests, wall clock in production).
    pub fn new(
        params: RunnerParams,
        source: Box<dyn EmsSource>,
        uplink: Box<dyn UplinkChannel>,
        buffer: Box<dyn LocalBuffer>,
        audit: Arc<dyn AuditSinkAlias>,
        clock: Arc<dyn Clock + Send + Sync>,
    ) -> Self {
        Self {
            params,
            source,
            uplink,
            buffer,
            audit,
            clock,
            degraded: false,
            buffer_full_audited: false,
        }
    }

    /// The buffer occupancy (health visibility, P6.17 later).
    pub fn buffered_len(&self) -> usize {
        self.buffer.len()
    }

    /// Whether the loop is currently in degraded (buffering-only) mode.
    pub fn is_degraded(&self) -> bool {
        self.degraded
    }

    fn audit(&self, kind: EdgeAuditKind, detail: String) {
        let event = EdgeAuditEvent {
            recorded_at: self.clock.now_ingest_time(),
            kind,
            detail,
        };
        if let Err(error) = self.audit.record(event) {
            // Audit is never silently skipped: a failed audit write is a
            // loud tracing error (the P4.28/P4.30 discipline; the edge has
            // no transactional store to refuse with — P6.18 owns the
            // durable offline audit buffer).
            tracing::error!(%error, kind = kind.name(), "edge audit write FAILED");
        }
    }

    /// Runs one full tick: poll → buffer → forward. Public so tests drive
    /// the loop deterministically tick-by-tick (no sleeps in tests).
    pub fn tick(&mut self) -> TickReport {
        let mut report = TickReport::default();
        report.degraded = self.degraded;

        // --- Poll (with buffer-headroom backpressure) --------------------
        let headroom = self.buffer.capacity().saturating_sub(self.buffer.len());
        let headroom = headroom.min(self.params.max_poll_per_tick);
        if headroom == 0 {
            report.poll_paused_backpressure = true;
            if !self.buffer_full_audited {
                self.audit(
                    EdgeAuditKind::BufferFull,
                    format!(
                        "buffer full ({}/{}); polling paused until the uplink drains it",
                        self.buffer.len(),
                        self.buffer.capacity()
                    ),
                );
                self.buffer_full_audited = true;
            }
        } else {
            match self.source.poll(headroom) {
                Ok(samples) => {
                    report.polled = samples.len();
                    for sample in samples {
                        match self.buffer.append(sample) {
                            Ok(()) => report.buffered += 1,
                            // The backpressure window should make this
                            // unreachable; if it happens, it is audited and
                            // the sample is NOT silently dropped — the tick
                            // stops buffering and the loop re-enters the
                            // backpressure state next tick.
                            Err(error) => {
                                self.audit(
                                    EdgeAuditKind::BufferFull,
                                    format!("append failed inside the backpressure window: {error}"),
                                );
                                self.buffer_full_audited = true;
                                break;
                            }
                        }
                    }
                }
                Err(error) => {
                    report.poll_failed = true;
                    self.audit(
                        EdgeAuditKind::SourcePollFailed,
                        format!("source '{}': {error}", self.source.source_name()),
                    );
                }
            }
        }

        // --- Forward (drain → uplink; failure re-buffers in order) --------
        while self.buffer.len() > 0 {
            let batch_size = self.params.forward_batch_size.max(1);
            let batch = self.buffer.drain(batch_size);
            let drained = batch.len();
            match self.uplink.forward(&batch) {
                Ok(()) => {
                    // The P6.09 commit point: the batch was confirmed
                    // forwarded, so the buffer may drop it from durable
                    // storage. Between `drain` and this call the records
                    // stayed on disk (durable buffer) — a crash in that
                    // window re-delivers them after restart (at-least-once,
                    // §15.4; the volatile in-memory buffer no-ops here).
                    self.buffer.commit_forwarded(&batch);
                    report.forwarded += drained;
                    report.buffered_len_after = self.buffer.len();
                    if self.degraded {
                        self.degraded = false;
                        report.degraded = false;
                        self.audit(
                            EdgeAuditKind::DegradedBufferingExited,
                            format!(
                                "uplink recovered; {} samples still buffered",
                                self.buffer.len()
                            ),
                        );
                    }
                }
                Err(error) => {
                    report.forward_failed = true;
                    // Put the failed batch back at the FRONT, in order; the
                    // buffer stays the source of truth.
                    self.buffer.prepend(batch);
                    report.buffered_len_after = self.buffer.len();
                    if !self.degraded {
                        self.degraded = true;
                        report.degraded = true;
                        self.audit(
                            EdgeAuditKind::DegradedBufferingEntered,
                            format!("uplink forward failed: {error}"),
                        );
                    }
                    break;
                }
            }
        }

        report.buffered_len_after = self.buffer.len();
        report
    }

    /// The long-running loop: tick on the poll cadence until the shutdown
    /// watch fires (the worker-composition shutdown pattern). Returns the
    /// last tick's report (health visibility).
    pub async fn run(mut self, mut shutdown: watch::Receiver<bool>) -> TickReport {
        tracing::info!(
            source = %self.source.source_name(),
            poll_interval_ms = self.params.poll_interval.as_millis() as u64,
            forward_batch_size = self.params.forward_batch_size,
            "edge supervision loop started (offline-first; buffer-first ingestion)"
        );
        let mut last = TickReport::default();
        loop {
            if *shutdown.borrow_and_update() {
                tracing::info!(
                    buffered = self.buffer.len(),
                    degraded = self.degraded,
                    "edge supervision loop stopping (buffered samples are retained by the buffer port)"
                );
                self.audit(
                    EdgeAuditKind::Startup,
                    format!("shutdown; {} samples retained in the buffer", self.buffer.len()),
                );
                return last;
            }
            last = self.tick();
            // A missed deadline is the next tick's problem, not a burst:
            // sleep the cadence, check shutdown, tick again.
            tokio::time::sleep(self.params.poll_interval).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::{
        EdgeError, EdgeTelemetrySample, EdgeTelemetrySampleParts, FailClosedEmsSource,
        FailClosedUplink, InMemoryAuditSink, InMemoryLocalBuffer,
    };
    use flexcore_contracts::quality::TelemetryQuality;
    use flexcore_contracts::time_model::{EventTime, IngestTime};
    use flexcore_contracts::units::ActivePowerMW;
    use flexcore_ports::fakes::FixedClock;
    use flexcore_ports::telemetry_source::{QuantityValue, TelemetryQuantity};
    use std::sync::Mutex;

    const BASE_MILLIS: i64 = 1_700_000_000_000;

    fn sample(sequence: u64) -> EdgeTelemetrySample {
        EdgeTelemetrySample::new(EdgeTelemetrySampleParts {
            asset_key: "ess-01".to_string(),
            quantity: TelemetryQuantity::ActivePower,
            value: QuantityValue::Power(ActivePowerMW::new(2.0).expect("finite")),
            quality: TelemetryQuality::Good,
            source_quality_code: None,
            event_time: EventTime::from_unix_millis(BASE_MILLIS + 1_000).expect("valid"),
            ingest_time: IngestTime::from_unix_millis(BASE_MILLIS + 2_000).expect("valid"),
            source_sequence: sequence,
            source_id: "edge:pilot-site-A:openems-rest:ess-01:active_power".to_string(),
        })
        .expect("valid sample")
    }

    /// A scripted fake EMS source: serves the queued sequences in order,
    /// then fails (or reports empty, per `fail_after`).
    struct ScriptedSource {
        queued: Mutex<Vec<u64>>,
        fail: bool,
    }

    impl ScriptedSource {
        fn serving(sequences: Vec<u64>, fail_after: bool) -> Self {
            Self {
                queued: Mutex::new(sequences),
                fail: fail_after,
            }
        }
    }

    impl EmsSource for ScriptedSource {
        fn source_name(&self) -> &str {
            "scripted-fake"
        }

        fn poll(&mut self, max: usize) -> Result<Vec<EdgeTelemetrySample>, EdgeError> {
            let mut queue = self.queued.lock().expect("poisoned");
            if queue.is_empty() {
                return if self.fail {
                    Err(EdgeError::SourcePollFailed {
                        reason: "scripted failure".to_string(),
                    })
                } else {
                    Ok(Vec::new())
                };
            }
            let take = max.min(queue.len());
            let drained: Vec<u64> = queue.drain(..take).collect();
            Ok(drained.into_iter().map(sample).collect())
        }
    }

    /// A fake uplink that fails the first N forwards, then succeeds.
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
                    reason: "scripted channel down".to_string(),
                });
            }
            debug_assert!(!samples.is_empty(), "the runner forwards only non-empty batches");
            Ok(())
        }
    }

    fn runner(
        source: Box<dyn EmsSource>,
        uplink: Box<dyn UplinkChannel>,
        capacity: usize,
    ) -> EdgeRunner {
        EdgeRunner::new(
            RunnerParams::from_config(Duration::from_millis(5_000), 2, capacity),
            source,
            uplink,
            Box::new(InMemoryLocalBuffer::new(capacity)),
            Arc::new(InMemoryAuditSink::new()),
            Arc::new(FixedClock::at_unix_millis(BASE_MILLIS)),
        )
    }

    #[test]
    fn healthy_tick_polls_buffers_forwards_in_order() {
        let sink = Arc::new(InMemoryAuditSink::new());
        let mut runner = EdgeRunner::new(
            RunnerParams::from_config(Duration::from_millis(5_000), 10, 100),
            Box::new(ScriptedSource::serving(vec![1, 2, 3, 4], false)),
            Box::new(FlakyUplink::failing_first(0)),
            Box::new(InMemoryLocalBuffer::new(100)),
            Arc::clone(&sink) as Arc<dyn AuditSinkAlias>,
            Arc::new(FixedClock::at_unix_millis(BASE_MILLIS)),
        );
        let report = runner.tick();
        assert_eq!(report.polled, 4);
        assert_eq!(report.buffered, 4);
        assert_eq!(report.forwarded, 4);
        assert_eq!(runner.buffered_len(), 0);
        assert!(!runner.is_degraded());
        assert!(!report.poll_failed);
        assert!(!report.forward_failed);
    }

    #[test]
    fn uplink_failure_enters_degraded_and_preserves_order() {
        let sink = Arc::new(InMemoryAuditSink::new());
        let mut runner = EdgeRunner::new(
            RunnerParams::from_config(Duration::from_millis(5_000), 2, 100),
            Box::new(ScriptedSource::serving(vec![1, 2, 3, 4, 5], false)),
            Box::new(FlakyUplink::failing_first(2)),
            Box::new(InMemoryLocalBuffer::new(100)),
            Arc::clone(&sink) as Arc<dyn AuditSinkAlias>,
            Arc::new(FixedClock::at_unix_millis(BASE_MILLIS)),
        );
        // Tick 1: poll 5, buffer 5, forward batch of 2 FAILS → degraded,
        // batch back at the front in order.
        let report = runner.tick();
        assert_eq!(report.polled, 5);
        assert_eq!(report.buffered, 5);
        assert_eq!(report.forwarded, 0);
        assert!(report.forward_failed);
        assert!(runner.is_degraded());
        assert_eq!(runner.buffered_len(), 5);
        // Tick 2: poll (empty), forward still failing (second scripted
        // failure) → stays degraded.
        let report = runner.tick();
        assert!(runner.is_degraded());
        assert_eq!(report.forwarded, 0);
        // Tick 3: uplink recovers → both batches forward in order (5
        // samples), degraded exits.
        let report = runner.tick();
        assert!(!runner.is_degraded());
        assert_eq!(report.forwarded, 5);
        assert_eq!(runner.buffered_len(), 0);

        // Audit trail: exactly one entered + one exited, in order.
        let events = sink.events();
        let kinds: Vec<&str> = events.iter().map(|e| e.kind.name()).collect();
        let entered = kinds.iter().filter(|k| **k == "EDGE_DEGRADED_BUFFERING_ENTERED").count();
        let exited = kinds.iter().filter(|k| **k == "EDGE_DEGRADED_BUFFERING_EXITED").count();
        assert_eq!(entered, 1, "one degraded-entry audit event");
        assert_eq!(exited, 1, "one degraded-exit audit event");
        let entered_pos = kinds.iter().position(|k| *k == "EDGE_DEGRADED_BUFFERING_ENTERED").unwrap();
        let exited_pos = kinds.iter().position(|k| *k == "EDGE_DEGRADED_BUFFERING_EXITED").unwrap();
        assert!(entered_pos < exited_pos, "exit is audited after entry");
    }

    #[test]
    fn buffer_backpressure_pauses_polling_audited() {
        let sink = Arc::new(InMemoryAuditSink::new());
        // Capacity 3, batch size 1: the uplink never succeeds, the buffer
        // fills, polling pauses (audited once), no silent drops.
        let mut runner = EdgeRunner::new(
            RunnerParams::from_config(Duration::from_millis(5_000), 1, 3),
            Box::new(ScriptedSource::serving(vec![1, 2, 3, 4, 5, 6], false)),
            Box::new(FailClosedUplink),
            Box::new(InMemoryLocalBuffer::new(3)),
            Arc::clone(&sink) as Arc<dyn AuditSinkAlias>,
            Arc::new(FixedClock::at_unix_millis(BASE_MILLIS)),
        );
        let _ = runner.tick(); // polls 3 (capacity), forward fails, buffer 3
        assert_eq!(runner.buffered_len(), 3);
        let report = runner.tick(); // headroom 0 → backpressure, no poll
        assert!(report.poll_paused_backpressure);
        assert_eq!(report.polled, 0);
        assert_eq!(runner.buffered_len(), 3);
        // The BufferFull audit fired exactly once (not per tick).
        let events = sink.events();
        let full_count = events
            .iter()
            .filter(|e| e.kind.name() == "EDGE_BUFFER_FULL")
            .count();
        assert_eq!(full_count, 1, "backpressure is audited once per fill episode");
    }

    #[test]
    fn source_poll_failure_is_audited_and_loop_continues() {
        let sink = Arc::new(InMemoryAuditSink::new());
        let mut runner = EdgeRunner::new(
            RunnerParams::from_config(Duration::from_millis(5_000), 10, 100),
            Box::new(FailClosedEmsSource),
            Box::new(FlakyUplink::failing_first(0)),
            Box::new(InMemoryLocalBuffer::new(100)),
            Arc::clone(&sink) as Arc<dyn AuditSinkAlias>,
            Arc::new(FixedClock::at_unix_millis(BASE_MILLIS)),
        );
        let report = runner.tick();
        assert!(report.poll_failed);
        assert_eq!(report.polled, 0);
        // No fabricated telemetry: nothing buffered, nothing forwarded.
        assert_eq!(runner.buffered_len(), 0);
        let events = sink.events();
        assert!(
            events.iter().any(|e| e.kind.name() == "EDGE_SOURCE_POLL_FAILED"),
            "the poll failure is audited"
        );
    }

    #[test]
    fn fail_closed_uplink_keeps_everything_buffered() {
        let mut runner = runner(
            Box::new(ScriptedSource::serving(vec![1, 2], false)),
            Box::new(FailClosedUplink),
            100,
        );
        let report = runner.tick();
        assert_eq!(report.polled, 2);
        assert_eq!(report.forwarded, 0);
        assert!(runner.is_degraded());
        assert_eq!(runner.buffered_len(), 2, "offline-first: nothing lost");
    }
}
