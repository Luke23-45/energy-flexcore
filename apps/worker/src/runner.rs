// P1.19 — The hosted background loops: the outbox publisher drain loop,
// the idempotent consumer pull loop, and the Phase 1 consumer handler.
//
// # Hosting (recorded decision, nats adapter lib.rs decisions 1 and 3)
//
// The adapter ships the library types ([`OutboxPublisher`],
// [`IdempotentConsumer`]); the worker ships the runtime wiring. The
// adapters call the SYNC ports (`EventOutbox`, `InboxStore`) between async
// broker operations, and the postgres implementations block on private
// runtimes — legal only from a blocking context. This module therefore
// wraps every adapter step in `tokio::task::block_in_place` on the
// multi-thread runtime (`drive`): the loop itself stays an async task (so
// shutdown watches and sleeps work normally) while each port-touching step
// runs on a blocking-capable thread. This is the mechanism the recorded
// decision names ("host the drain loop on a blocking thread with a channel
// bridge"), without the bridge: the per-step granularity keeps blocking
// slots short.
//
// # Backoff and loss-freedom
//
// Failure handling mirrors the adapters' `run` loops: per-entry publish
// failures are already recorded on the entry by the publisher (attempt
// count, eventual `Failed` state — never dropped); a failed drain cycle or
// consumer error sleeps with exponential backoff and retries. No failure
// path here can lose an outbox entry or a message: the outbox store and
// the unacknowledged delivery are the durability.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use flexcore_nats_adapter::consumer::{
    ConsumerConfig, DeliveredMessage, HandlerError, IdempotentConsumer, ProcessOutcome,
    ProcessReport,
};
use flexcore_nats_adapter::inbox::{InboxStore, RecordOutcome};
use flexcore_nats_adapter::publisher::{OutboxPublisher, PublisherConfig};
use flexcore_ports::event_outbox::EventOutbox;
use tokio::sync::watch;

/// Runs a future on the worker runtime from a blocking-capable context.
/// See the module header: this is how the sync ports inside the adapter
/// futures are legal.
async fn drive<F: std::future::Future>(future: F) -> F::Output {
    tokio::task::block_in_place(|| tokio::runtime::Handle::current().block_on(future))
}

/// Sleeps `duration`, or returns early when shutdown fires. `Ok(())` =
/// keep going; `Err(())` = shut down.
async fn sleep_or_shutdown(
    duration: Duration,
    shutdown: &mut watch::Receiver<bool>,
) -> Result<(), ()> {
    tokio::select! {
        () = tokio::time::sleep(duration) => Ok(()),
        changed = shutdown.changed() => {
            let _ = changed;
            Err(())
        }
    }
}

/// Exponential backoff between cycles after failures:
/// `base * 2^(failures - 1)`, clamped to `[base, cap]`.
fn backoff(base: Duration, cap: Duration, failures: u64) -> Duration {
    let exponent = failures.saturating_sub(1).min(16);
    let factor = 1u64.checked_shl(exponent as u32).unwrap_or(u64::MAX);
    base.checked_mul(factor)
        .unwrap_or(cap)
        .min(cap)
        .max(base.min(cap))
}

/// Builds the publisher tuning from the worker config: the poll interval
/// is the idle sleep; the retry budget and backoff bounds stay at the
/// adapter defaults (they are delivery-behaviour values, not deployment
/// tuning — changing them belongs to the adapter's recorded policy).
pub fn publisher_config(poll_interval: Duration) -> PublisherConfig {
    PublisherConfig {
        idle_interval: poll_interval,
        ..Default::default()
    }
}

/// The outbox publisher loop until shutdown. One drain cycle per
/// iteration; idle and backoff sleeps are shutdown-aware. Cancel-safe by
/// construction: a crash or shutdown mid-cycle leaves unmarked entries
/// pending and idempotent by key (report §6.3; P1.15).
pub async fn run_outbox_publisher(
    jetstream: async_nats::jetstream::Context,
    outbox: Arc<dyn EventOutbox>,
    config: PublisherConfig,
    mut shutdown: watch::Receiver<bool>,
) {
    let publisher = OutboxPublisher::new(&jetstream, outbox.as_ref(), config.clone());
    loop {
        if *shutdown.borrow() {
            return;
        }
        let report = drive(publisher.drain_once()).await;
        match report {
            Ok(report) => {
                if report.drained > 0 {
                    tracing::info!(
                        drained = report.drained,
                        published = report.published,
                        broker_deduplicated = report.broker_deduplicated,
                        marked_delivered = report.marked_delivered,
                        attempt_failures = report.attempt_failures,
                        marked_failed = report.marked_failed,
                        "outbox drain cycle completed"
                    );
                }
                if report.attempt_failures > 0 {
                    let delay = backoff(
                        config.backoff_base,
                        config.backoff_cap,
                        report.attempt_failures as u64,
                    );
                    if sleep_or_shutdown(delay, &mut shutdown).await.is_err() {
                        return;
                    }
                } else if report.drained == 0
                    && sleep_or_shutdown(config.idle_interval, &mut shutdown)
                        .await
                        .is_err()
                {
                    return;
                }
            }
            Err(store_error) => {
                tracing::error!(
                    error = %store_error,
                    "outbox store failure; retrying after backoff (no entry is lost: \
                     the store owns the durability)"
                );
                if sleep_or_shutdown(config.backoff_base, &mut shutdown)
                    .await
                    .is_err()
                {
                    return;
                }
            }
        }
    }
}

/// The Phase 1 consumer handler: the side effect IS the inbox record.
///
/// P1.19 scope note: no business consumer exists yet in Phase 1 — the
/// worker runs the full P1.16 idempotent flow (pull → inbox check →
/// process → commit → ack) with the durable inbox record as the one
/// committed side effect. When real consumers arrive they supply their own
/// handler that applies the state change AND calls `record` inside its one
/// transaction; this handler keeps the same contract (its single-statement
/// transaction is the record itself — see `inbox`'s module header).
///
/// `side_effects` counts committed inserts — the duplicate-delivery
/// evidence the integration test asserts on.
pub fn inbox_recording_handler(
    durable_name: String,
    side_effects: Arc<AtomicUsize>,
) -> impl FnMut(&DeliveredMessage, &dyn InboxStore) -> Result<(), HandlerError> {
    move |message: &DeliveredMessage, inbox: &dyn InboxStore| {
        let Some(key) = message.idempotency_key.as_deref() else {
            // The consumer helper quarantines keyless messages before the
            // handler runs; this arm is a defensive restatement.
            return Err(HandlerError(
                "delivered message carries no idempotency key".to_string(),
            ));
        };
        match inbox.record(&durable_name, key) {
            Ok(RecordOutcome::Inserted) => {
                side_effects.fetch_add(1, Ordering::SeqCst);
                tracing::debug!(
                    consumer = %durable_name,
                    "inbox record committed (side effect applied)"
                );
                Ok(())
            }
            Ok(RecordOutcome::AlreadyRecorded) => {
                // Concurrent duplicate won the race; per the trait
                // obligation the side effect is NOT executed again.
                Ok(())
            }
            Err(error) => Err(HandlerError(error.to_string())),
        }
    }
}

/// The idempotent consumer loop until shutdown: pull → process → ack,
/// with idle and error backoff. Returns the accumulated
/// [`ProcessReport`]. Cancel-safety comes from the consumer helper: a
/// shutdown mid-message leaves it unacknowledged → JetStream redelivers →
/// the inbox deduplicates.
pub async fn run_idempotent_consumer(
    jetstream: async_nats::jetstream::Context,
    inbox: Arc<dyn InboxStore>,
    config: ConsumerConfig,
    side_effects: Arc<AtomicUsize>,
    mut shutdown: watch::Receiver<bool>,
) -> ProcessReport {
    let mut handler = inbox_recording_handler(config.durable_name.clone(), side_effects);
    let consumer = IdempotentConsumer::new(&jetstream, inbox.as_ref(), config.clone());
    let mut report = ProcessReport::default();
    loop {
        if *shutdown.borrow() {
            return report;
        }
        let outcome = drive(consumer.process_next(&mut handler)).await;
        match outcome {
            Ok(ProcessOutcome::Idle) => {
                report.idle_polls += 1;
                if sleep_or_shutdown(config.idle_interval, &mut shutdown)
                    .await
                    .is_err()
                {
                    return report;
                }
            }
            Ok(outcome) => {
                match outcome {
                    ProcessOutcome::Processed => report.processed += 1,
                    ProcessOutcome::SkippedDuplicate => report.skipped_duplicates += 1,
                    ProcessOutcome::RetryScheduled => report.retries_scheduled += 1,
                    ProcessOutcome::Quarantined => report.quarantined += 1,
                    ProcessOutcome::Idle => {}
                }
                tracing::debug!(outcome = ?outcome, "consumer step completed");
            }
            Err(error) => {
                report.errors += 1;
                tracing::error!(
                    consumer = %config.durable_name,
                    error = %error,
                    "consumer cycle failed; the message stays unacknowledged \
                     and will be redelivered by JetStream (at-least-once)"
                );
                if sleep_or_shutdown(config.error_backoff, &mut shutdown)
                    .await
                    .is_err()
                {
                    return report;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_grows_exponentially_and_caps() {
        let base = Duration::from_millis(100);
        let cap = Duration::from_millis(10_000);
        assert_eq!(backoff(base, cap, 0), base);
        assert_eq!(backoff(base, cap, 1), base);
        assert_eq!(backoff(base, cap, 2), Duration::from_millis(200));
        assert_eq!(backoff(base, cap, 8), cap);
        assert_eq!(backoff(base, cap, u64::MAX), cap);
    }

    #[test]
    fn publisher_config_carries_the_poll_interval() {
        let config = publisher_config(Duration::from_millis(500));
        assert_eq!(config.idle_interval, Duration::from_millis(500));
        // Retry policy stays at the adapter defaults.
        assert_eq!(config.max_attempts, PublisherConfig::default().max_attempts);
    }
}
