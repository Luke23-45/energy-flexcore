// P1.15 — The transactional-outbox publisher: drain the outbox through the
// sync `EventOutbox` port, publish to JetStream with `Nats-Msg-Id` =
// idempotency key, mark delivered ONLY after the broker publish
// acknowledgement (report §6.3 steps 2–3; P1.14 §2; ledger P1.15).
//
// The §6.3 rules implemented here, and where they bite:
//
// 1. **Read pending entries via the port** ([`EventOutbox::pending`], in
//    insertion order, batch-limited). The port is sync; see the
//    sync-port/async-adapter resolution in the crate README and
//    src/lib.rs — the calls are short database round-trips between async
//    broker operations.
// 2. **Publish with `Nats-Msg-Id` = the entry's idempotency key on EVERY
//    publish** (P1.15 acceptance: code inspection + test). Broker-side
//    deduplication then collapses re-publications after a crash between
//    publish and mark — within the duplicate window only (P1.14 §2). The
//    window is a courtesy; the durable guarantee is this outbox plus the
//    consumer inbox (P1.16).
// 3. **Mark delivered only after the broker publish ack.** The ack is the
//    awaited `PublishAckFuture` (`PublishAck`), not the send. A crash
//    before `mark_delivered` leaves the entry pending; the re-publication
//    is deduplicated by key (tested in tests/).
// 4. **On failure, leave the entry pending** with an incremented attempt
//    count; after `max_attempts` failed attempts, `mark_failed` routes the
//    entry for investigation — it stays in the outbox store and is never
//    silently dropped (P1.14 §4.1 / port obligation 4).
// 5. **Retry with backoff, no loss.** `run` loops until the shutdown watch
//    channel fires: broker unavailable → every publish fails → entries
//    remain pending with growing attempt counts; when the broker returns,
//    the pending entries publish again. No entry is ever removed from the
//    store on the publish path — loss is impossible here by construction,
//    which is what the broker-restart evidence in tests/ and (fully) the
//    P1.25 Compose stack exercises.
//
// Recorded limit (P1.15 step 5, and a ledger exit item): the attempt
// backoff here is per drain cycle. The `PendingOutboxEntry` port view does
// not carry `updated_at`, so per-entry delay scheduling cannot be expressed
// through the port; the cycle-level backoff in [`OutboxPublisher::run`] is
// the honest approximation. A per-entry backoff needs a port extension
// (`updated_at` on the pending view) and is deliberately not improvised.

use std::time::Duration;

// The message-id header constant: async-nats 0.50.0 names it
// `NATS_MESSAGE_ID` (the NATS server's wire header is `Nats-Msg-Id`; the
// client constant is the canonical name for it).
use async_nats::header::{HeaderMap, NATS_MESSAGE_ID};
// Paths per the async-nats 0.50.0 docs (VERIFY-BUILD: `PublishError` is a
// type alias under `jetstream::context`; `PublishAck` lives under
// `jetstream::publish`; the jetstream root does not re-export them).
use async_nats::jetstream::context::PublishError;
use async_nats::jetstream::publish::PublishAck;
use async_nats::jetstream::Context;
use flexcore_ports::event_outbox::{EventOutbox, EventOutboxError, PendingOutboxEntry};
use tokio::sync::watch;

/// Publisher tuning. Every value here is configuration, not semantics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublisherConfig {
    /// How many pending entries one [`OutboxPublisher::drain_once`] reads
    /// and attempts (port `pending` batch limit).
    pub batch_size: u32,
    /// Failed publish attempts after which an entry is routed to `Failed`
    /// (it stays in the outbox store for investigation; it is never
    /// deleted).
    pub max_attempts: u32,
    /// Base delay of the exponential backoff between drain cycles after a
    /// cycle with publish failures.
    pub backoff_base: Duration,
    /// Upper bound of the backoff between drain cycles.
    pub backoff_cap: Duration,
    /// Idle delay when the outbox has no pending entries.
    pub idle_interval: Duration,
}

impl Default for PublisherConfig {
    fn default() -> Self {
        Self {
            batch_size: 100,
            max_attempts: 10,
            backoff_base: Duration::from_millis(250),
            backoff_cap: Duration::from_secs(30),
            idle_interval: Duration::from_millis(200),
        }
    }
}

/// What one drain cycle did. `broker_deduplicated` is the count of
/// publications the broker acknowledged as duplicates of an earlier
/// `Nats-Msg-Id` within the duplicate window — the observable evidence for
/// the crash-between-publish-and-mark deduplication test.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DrainReport {
    /// Entries read from the outbox this cycle.
    pub drained: usize,
    /// Publications the broker acknowledged as fresh.
    pub published: usize,
    /// Publications the broker acknowledged as duplicates (dedup window).
    pub broker_deduplicated: usize,
    /// Entries marked delivered (only after an acknowledgement, fresh or
    /// deduplicated).
    pub marked_delivered: usize,
    /// Publish failures that recorded an attempt and stayed pending.
    pub attempt_failures: usize,
    /// Entries moved to `Failed` after exhausting `max_attempts`.
    pub marked_failed: usize,
}

/// Publisher failures that are fatal to a drain cycle. Per-entry publish
/// failures are NOT fatal — they are recorded on the entry (attempt count,
/// backoff, eventual `mark_failed`) and surfaced in the [`DrainReport`].
/// Only an outbox-store failure aborts the cycle.
#[derive(Debug)]
pub enum PublisherError {
    /// The outbox port failed (opaque backend error inside; never raw
    /// storage or broker text at any API boundary above this crate).
    OutboxStore(EventOutboxError),
}

impl std::fmt::Display for PublisherError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PublisherError::OutboxStore(e) => write!(f, "outbox store failure: {e}"),
        }
    }
}

impl std::error::Error for PublisherError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            PublisherError::OutboxStore(e) => Some(e),
        }
    }
}

impl From<EventOutboxError> for PublisherError {
    fn from(e: EventOutboxError) -> Self {
        PublisherError::OutboxStore(e)
    }
}

/// Exponential backoff for a drain cycle that saw `failures` publish
/// failures: `base * 2^(failures - 1)`, clamped to `[base, cap]`. Pure so
/// it is unit-testable without a broker.
fn cycle_backoff(base: Duration, cap: Duration, failures: usize) -> Duration {
    let exponent = failures.saturating_sub(1).min(16);
    let factor = 1u32.checked_shl(exponent as u32).unwrap_or(u32::MAX);
    base.checked_mul(factor)
        .unwrap_or(cap)
        .min(cap)
        .max(base.min(cap))
}

/// The outbox publisher (P1.15). Borrows the JetStream context and the
/// outbox port; the composition root owns both and drives [`run`] (or calls
/// [`drain_once`] directly) — see the recorded hosting decision in
/// src/lib.rs (apps/worker, P1.19, hosts this type as a background task).
pub struct OutboxPublisher<'a> {
    jetstream: &'a Context,
    outbox: &'a dyn EventOutbox,
    config: PublisherConfig,
}

impl<'a> OutboxPublisher<'a> {
    pub fn new(
        jetstream: &'a Context,
        outbox: &'a dyn EventOutbox,
        config: PublisherConfig,
    ) -> Self {
        Self {
            jetstream,
            outbox,
            config,
        }
    }

    /// One drain cycle: read pending entries, publish each with
    /// `Nats-Msg-Id` = idempotency key, mark delivered per entry **only
    /// after its broker ack**. A crash anywhere inside this function leaves
    /// unmarked entries pending and idempotent by key — re-publication is
    /// deduplicated by the broker within the duplicate window and by the
    /// consumer inbox durably (P1.16).
    pub async fn drain_once(&self) -> Result<DrainReport, PublisherError> {
        // Sync port call (recorded resolution): short database round-trip.
        let entries = self.outbox.pending(self.config.batch_size)?;
        let mut report = DrainReport {
            drained: entries.len(),
            ..Default::default()
        };
        for entry in entries {
            match self.publish_entry(&entry).await {
                Ok(ack) => {
                    if ack.duplicate {
                        // The broker deduplicated a re-publication within
                        // the duplicate window. The entry is still marked
                        // delivered — the publish is on the stream, exactly
                        // once per window; consumers stay idempotent
                        // regardless (P1.16).
                        report.broker_deduplicated += 1;
                        tracing::debug!(
                            idempotency_key = entry.idempotency_key(),
                            subject = entry.destination_subject(),
                            "outbox re-publication deduplicated by the broker duplicate window"
                        );
                    } else {
                        report.published += 1;
                    }
                    // Report §6.3 step 3 — strictly after the ack.
                    self.outbox.mark_delivered(entry.idempotency_key())?;
                    report.marked_delivered += 1;
                }
                Err(publish_error) => {
                    // Leave pending: record the attempt; the entry is
                    // retried on a later cycle. After the configured
                    // attempt budget, route to Failed for investigation —
                    // never silently dropped.
                    self.outbox.record_attempt(entry.idempotency_key())?;
                    report.attempt_failures += 1;
                    tracing::warn!(
                        idempotency_key = entry.idempotency_key(),
                        subject = entry.destination_subject(),
                        attempt = entry.attempt_count() + 1,
                        error = %publish_error,
                        "outbox publish failed; entry stays pending for retry"
                    );
                    if entry.attempt_count() + 1 >= self.config.max_attempts {
                        self.outbox.mark_failed(entry.idempotency_key())?;
                        report.marked_failed += 1;
                        tracing::error!(
                            idempotency_key = entry.idempotency_key(),
                            subject = entry.destination_subject(),
                            "outbox entry exhausted the publisher retry budget and is marked \
                             FAILED for investigation; it is retained in the outbox store"
                        );
                    }
                }
            }
        }
        Ok(report)
    }

    /// Publishes one entry and awaits the broker acknowledgement.
    /// VERIFY-BUILD: (a) `HeaderMap::insert` takes `V: IntoHeaderValue`;
    /// in async-nats 0.50.0 `HeaderValue: From<String>` exists, so the
    /// `String` key converts (checked against the 0.50.0 docs); (b)
    /// `PublishAck.duplicate` is the documented deduplication flag on the
    /// awaited ack (checked against the 0.50.0 docs).
    async fn publish_entry(
        &self,
        entry: &PendingOutboxEntry,
    ) -> Result<async_nats::jetstream::PublishAck, PublishError> {
        let mut headers = HeaderMap::new();
        // P1.15 acceptance: Nats-Msg-Id is set from the idempotency key on
        // EVERY publish, without exception.
        headers.insert(NATS_MESSAGE_ID, entry.idempotency_key().to_string());
        let ack_future = self
            .jetstream
            .publish_with_headers(
                entry.destination_subject().as_str(),
                headers,
                entry.payload().as_bytes().to_vec(),
            )
            .await?;
        // The broker publish acknowledgement — the ONLY thing that may
        // trigger `mark_delivered`.
        ack_future.await
    }

    /// The drain loop until shutdown. Broker unavailable → every cycle
    /// records attempts and backs off (no lost entries, no premature
    /// marking); store failure → logged, backed off, retried; idle → sleeps
    /// the idle interval. Exits only on the shutdown signal (apps/worker,
    /// P1.19, owns the watch channel).
    pub async fn run(&self, mut shutdown: watch::Receiver<bool>) {
        loop {
            if *shutdown.borrow() {
                return;
            }
            match self.drain_once().await {
                Ok(report) if report.drained == 0 => {
                    if self.sleep_or_shutdown(self.config.idle_interval, &mut shutdown)
                        .await
                        .is_some()
                    {
                        return;
                    }
                }
                Ok(report) if report.attempt_failures > 0 => {
                    // Backoff before the next cycle; the retry budget is
                    // consumed per cycle (recorded limit, module header).
                    let backoff = cycle_backoff(
                        self.config.backoff_base,
                        self.config.backoff_cap,
                        report.attempt_failures,
                    );
                    tracing::debug!(?backoff, failures = report.attempt_failures, "backing off");
                    if self.sleep_or_shutdown(backoff, &mut shutdown).await.is_some() {
                        return;
                    }
                }
                Ok(_) => {}
                Err(store_error) => {
                    tracing::error!(
                        error = %store_error,
                        "outbox store failure; retrying after backoff (no entry is lost: \
                         the store owns the durability)"
                    );
                    if self
                        .sleep_or_shutdown(self.config.backoff_base, &mut shutdown)
                        .await
                        .is_some()
                    {
                        return;
                    }
                }
            }
        }
    }

    /// Sleeps, unless shutdown fires first. Returns `Some(())` when the
    /// shutdown flag is set or the sender dropped (caller should exit),
    /// `None` otherwise.
    async fn sleep_or_shutdown(
        &self,
        duration: Duration,
        shutdown: &mut watch::Receiver<bool>,
    ) -> Option<()> {
        tokio::select! {
            () = tokio::time::sleep(duration) => None,
            changed = shutdown.changed() => {
                // Ok: the flag was sent to; Err: the sender dropped, i.e.
                // the composition root is going away — also a shutdown.
                let _ = changed;
                Some(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cycle_backoff_grows_exponentially_and_caps() {
        let base = Duration::from_millis(100);
        let cap = Duration::from_millis(10_000);
        assert_eq!(cycle_backoff(base, cap, 0), base, "no failures → base");
        assert_eq!(cycle_backoff(base, cap, 1), base);
        assert_eq!(cycle_backoff(base, cap, 2), Duration::from_millis(200));
        assert_eq!(cycle_backoff(base, cap, 3), Duration::from_millis(400));
        assert_eq!(cycle_backoff(base, cap, 7), Duration::from_millis(6_400));
        assert_eq!(
            cycle_backoff(base, cap, 8),
            cap,
            "grows into the cap"
        );
        assert_eq!(
            cycle_backoff(base, cap, usize::MAX),
            cap,
            "extreme inputs clamp to the cap"
        );
        assert_eq!(
            cycle_backoff(base, cap, 40),
            cap,
            "exponent is clamped before shifting (no overflow)"
        );
    }
}
