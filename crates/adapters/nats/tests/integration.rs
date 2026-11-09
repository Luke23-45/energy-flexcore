// P1.15 / P1.16 — Integration tests against a real NATS server 2.15.x
// (ledger P1.15 step 4, P1.16 step 5; P1.14 §5 consumer/DLQ policies).
//
// GATING (recorded decision, src/lib.rs decision 5): every test in this
// file is gated on `FLEXCORE_TEST_NATS_URL` and SKIPS CLEANLY (returns
// without asserting) when the variable is unset or empty. The durable
// inbox-store variants additionally need PostgreSQL (`FLEXCORE_TEST_
// DATABASE_URL`); they live with the postgres adapter, which owns the
// inbox table (single-home decision in src/inbox.rs) — deliberately NOT
// simulated here.
//
// Semantics under test (ledger acceptance):
//
// - publish → consume round trip (P1.15);
// - `Nats-Msg-Id` = idempotency key on every publish, so a re-publication
//   after a crash between publish and mark is broker-deduplicated within
//   the duplicate window (P1.15: "marked delivered only after ack" — the
//   kill-between-publish-and-mark test);
// - crash mid-batch → entries stay pending, nothing lost, no duplicate
//   side effect after recovery ("restart" is proxied by a fresh publisher
//   against the same broker; a full server-kill test is the P1.25 Compose
//   stack's job — recorded limit);
// - broker unavailability → entries stay pending with attempt counts, and
//   publish once the target stream exists (no loss, no premature marking);
// - duplicate delivery / redelivery → exactly one side effect (P1.16);
// - ack-after-commit: crash between commit and ack → redelivery is
//   acknowledged without re-execution (P1.16);
// - poison message → `FLEXCORE_DLQ` after `max_deliver`, with metadata and
//   an error log — never a silent drop (P1.16 step 4, P1.14 §5.2).
//
// Test isolation: subjects and durable names carry a per-run suffix, so
// re-runs against a persistent dev server see neither old messages (the
// consumer filters on its exact subject) nor old dedup entries (unique
// `Nats-Msg-Id`s).

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use flexcore_nats_adapter::consumer::{
    ConsumerConfig, DeliveredMessage, HandlerError, IdempotentConsumer, ProcessOutcome,
};
use flexcore_nats_adapter::inbox::{InMemoryInboxStore, InboxStore, RecordOutcome};
use flexcore_nats_adapter::publisher::{OutboxPublisher, PublisherConfig};
use flexcore_nats_adapter::topology::{
    provision_durable_pull_consumer, provision_phase_1_streams, ConsumerPolicy,
    DeploymentLimits, STREAM_AUDIT, STREAM_DISPATCH, STREAM_REGISTRY,
};
use flexcore_ports::clock::Clock;
use flexcore_ports::event_outbox::{EventOutbox, EventOutboxError, OutboxWrite};
use flexcore_ports::fakes::{FakeEventOutbox, FakeWriteJournal, FixedClock};
use flexcore_ports::state_change::{StateChangeOperation, StateChangeReceipt};

/// The test gate: `None` means "skip cleanly".
fn nats_url() -> Option<String> {
    match std::env::var("FLEXCORE_TEST_NATS_URL") {
        Ok(url) if !url.trim().is_empty() => Some(url),
        _ => {
            eprintln!(
                "skipping: FLEXCORE_TEST_NATS_URL is not set (start a NATS 2.15.x \
                 server and export FLEXCORE_TEST_NATS_URL=nats://127.0.0.1:4222)"
            );
            None
        }
    }
}

/// P1.14 §4.1: limits are deployment configuration; these are test values.
fn limits() -> DeploymentLimits {
    DeploymentLimits {
        duplicate_window: Duration::from_secs(120),
        max_age: None,
        max_messages: None,
        max_bytes: None,
    }
}

fn run_suffix() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or_default()
}

/// Appends one complete outbox write (the test stands in for the §6.3
/// step-1 composition root; the atomicity of that write is the P1.13
/// adapter's tested obligation, not this test's subject).
fn append_entry(outbox: &FakeEventOutbox, key: &str, subject: &str, payload: &str) {
    let clock = FixedClock::at_unix_millis(0);
    let receipt = StateChangeReceipt::new(
        flexcore_ports::state_change::entities::ASSETS.to_string(),
        StateChangeOperation::Insert,
        None,
    );
    let write = OutboxWrite::new(
        receipt,
        key.to_string(),
        subject.to_string(),
        payload.to_string(),
        clock.now_event_time(),
    )
    .expect("test outbox write is well-formed");
    outbox.append(write).expect("unique key in a fresh outbox");
}

/// A `FakeEventOutbox` wrapper whose `mark_delivered` can be made to fail —
/// the "publisher killed between publish and mark" injection point.
struct CrashyMarkOutbox {
    inner: FakeEventOutbox,
    fail_marks: AtomicBool,
}

impl CrashyMarkOutbox {
    fn new() -> Self {
        Self {
            inner: FakeEventOutbox::new(FakeWriteJournal::new()),
            fail_marks: AtomicBool::new(false),
        }
    }
}

impl EventOutbox for CrashyMarkOutbox {
    fn append(&self, write: OutboxWrite) -> Result<(), EventOutboxError> {
        self.inner.append(write)
    }
    fn pending(
        &self,
        limit: u32,
    ) -> Result<Vec<flexcore_ports::event_outbox::PendingOutboxEntry>, EventOutboxError> {
        self.inner.pending(limit)
    }
    fn record_attempt(&self, idempotency_key: &str) -> Result<(), EventOutboxError> {
        self.inner.record_attempt(idempotency_key)
    }
    fn mark_delivered(&self, idempotency_key: &str) -> Result<(), EventOutboxError> {
        if self.fail_marks.load(Ordering::SeqCst) {
            // The simulated crash: the broker ack was received but the
            // process died before the delivered flag was written.
            return Err(EventOutboxError::Backend(
                flexcore_ports::backend_error::BackendError::new(
                    "SIMULATED_CRASH".to_string(),
                    "test crash between broker publish and mark_delivered".to_string(),
                ),
            ));
        }
        self.inner.mark_delivered(idempotency_key)
    }
    fn mark_failed(&self, idempotency_key: &str) -> Result<(), EventOutboxError> {
        self.inner.mark_failed(idempotency_key)
    }
}

/// The production-shaped test handler: records the inbox entry under the
/// consumer's durable identity (the same identity the helper checks — the
/// handler MUST use the consumer name the helper was configured with),
/// standing in for the handler's one transaction (the durable
/// single-transaction version is the postgres adapter's, recorded decision),
/// and then applies exactly one counted side effect.
fn counting_handler(
    consumer_name: &str,
    effects: Arc<AtomicUsize>,
    seen: Arc<Mutex<Vec<String>>>,
) -> impl FnMut(&DeliveredMessage, &dyn InboxStore) -> Result<(), HandlerError> {
    let consumer_name = consumer_name.to_string();
    move |message, inbox| {
        let key = message
            .idempotency_key
            .as_deref()
            .ok_or_else(|| HandlerError("no idempotency key".to_string()))?;
        match inbox.record(&consumer_name, key) {
            Ok(RecordOutcome::Inserted) => {}
            Ok(RecordOutcome::AlreadyRecorded) => return Ok(()),
            Err(e) => return Err(HandlerError(e.to_string())),
        }
        seen.lock().expect("test mutex").push(message.subject.clone());
        effects.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

/// Publish → consume round trip; then the same delivery again (duplicate /
/// redelivery) produces `SkippedDuplicate` and NO second side effect
/// (ledger P1.16: duplicate-delivery test).
#[tokio::test]
async fn publish_then_consume_and_duplicate_delivery_yields_one_side_effect() {
    let Some(url) = nats_url() else { return };
    let client = async_nats::connect(&url).await.expect("connect to NATS");
    let js = async_nats::jetstream::new(client);
    provision_phase_1_streams(&js, &limits()).await.expect("provision streams");

    let suffix = run_suffix();
    let subject = format!("flexcore.v1.registry.test_{suffix}");
    let key = format!("key-{suffix}");

    let outbox = FakeEventOutbox::new(FakeWriteJournal::new());
    append_entry(&outbox, &key, &subject, r#"{"event":"AssetRegistered"}"#);

    let publisher = OutboxPublisher::new(&js, &outbox, PublisherConfig::default());
    let report = publisher.drain_once().await.expect("drain");
    assert_eq!(report.drained, 1);
    assert_eq!(report.published, 1);
    assert_eq!(report.marked_delivered, 1, "delivered only after the ack");
    assert_eq!(report.attempt_failures, 0);

    let durable = format!("registry-roundtrip-{suffix}");
    let policy = ConsumerPolicy {
        durable_name: durable.clone(),
        stream: STREAM_REGISTRY.to_string(),
        filter_subject: subject.clone(),
        ack_wait: Duration::from_secs(5),
        max_deliver: 3,
        max_ack_pending: 100,
    };
    provision_durable_pull_consumer(&js, &policy).await.expect("provision consumer");

    let inbox = InMemoryInboxStore::new();
    let consumer = IdempotentConsumer::new(&js, &inbox, ConsumerConfig::from_policy(&policy));
    let effects = Arc::new(AtomicUsize::new(0));
    let seen = Arc::new(Mutex::new(Vec::new()));

    let mut handler = counting_handler(&durable, effects.clone(), seen.clone());

    let message = consumer
        .pull_one()
        .await
        .expect("pull")
        .expect("one delivered message");
    assert_eq!(message.subject, subject);
    assert_eq!(message.idempotency_key.as_deref(), Some(key.as_str()));

    let first = consumer
        .process_delivered(message.clone(), &mut handler)
        .await
        .expect("process");
    assert!(matches!(first, ProcessOutcome::Processed));
    assert_eq!(effects.load(Ordering::SeqCst), 1);

    // The same delivery again — what a redelivery, duplicate publish or
    // replay looks like to the consumer — must not re-execute.
    let second = consumer
        .process_delivered(message, &mut handler)
        .await
        .expect("process redelivery");
    assert!(matches!(second, ProcessOutcome::SkippedDuplicate));
    assert_eq!(effects.load(Ordering::SeqCst), 1, "exactly one side effect");
    assert_eq!(seen.lock().expect("test mutex").len(), 1);
}

/// P1.15 acceptance: an outbox entry is marked delivered only after the
/// broker ack — the test "kills" the publisher between publish and mark,
/// then verifies the re-publication is broker-deduplicated (the
/// `Nats-Msg-Id` = idempotency-key rule) and the consumer sees the event
/// exactly once (no loss, no duplicate side effect).
#[tokio::test]
async fn kill_between_publish_and_mark_republish_is_deduplicated() {
    let Some(url) = nats_url() else { return };
    let client = async_nats::connect(&url).await.expect("connect to NATS");
    let js = async_nats::jetstream::new(client);
    provision_phase_1_streams(&js, &limits()).await.expect("provision streams");

    let suffix = run_suffix();
    let subject = format!("flexcore.v1.registry.test_{suffix}");
    let key = format!("key-{suffix}");

    let outbox = Arc::new(CrashyMarkOutbox::new());
    append_entry(&outbox, &key, &subject, r#"{"event":"AssetRegistered"}"#);

    // First drain: publish succeeds on the broker, then the process "dies"
    // before mark_delivered — drain_once surfaces the store failure and the
    // entry stays pending.
    outbox.fail_marks.store(true, Ordering::SeqCst);
    let crashed_publisher = OutboxPublisher::new(&js, outbox.as_ref(), PublisherConfig::default());
    let result = crashed_publisher.drain_once().await;
    assert!(result.is_err(), "the simulated mark crash must surface");
    let pending: Vec<_> = outbox.pending(10).expect("pending read");
    assert_eq!(pending.len(), 1, "the entry stays pending after the crash");
    assert_eq!(pending[0].idempotency_key(), key);

    // "Restart": a fresh publisher (same outbox store), marks healthy.
    outbox.fail_marks.store(false, Ordering::SeqCst);
    let restarted_publisher = OutboxPublisher::new(&js, outbox.as_ref(), PublisherConfig::default());
    let report = restarted_publisher
        .drain_once()
        .await
        .expect("drain after restart");
    assert_eq!(
        report.broker_deduplicated, 1,
        "the re-publication is deduplicated by Nats-Msg-Id within the window"
    );
    assert_eq!(report.published, 0, "nothing new was published");
    assert_eq!(report.marked_delivered, 1, "marked after the (duplicate) ack");
    assert_eq!(outbox.pending(10).expect("pending").len(), 0, "no loss");

    // The consumer sees the event exactly once.
    let durable = format!("registry-crashmark-{suffix}");
    let policy = ConsumerPolicy {
        durable_name: durable.clone(),
        stream: STREAM_REGISTRY.to_string(),
        filter_subject: subject.clone(),
        ack_wait: Duration::from_secs(5),
        max_deliver: 3,
        max_ack_pending: 100,
    };
    provision_durable_pull_consumer(&js, &policy).await.expect("provision consumer");
    let inbox = InMemoryInboxStore::new();
    let consumer = IdempotentConsumer::new(&js, &inbox, ConsumerConfig::from_policy(&policy));
    let effects = Arc::new(AtomicUsize::new(0));
    let seen = Arc::new(Mutex::new(Vec::new()));
    let mut handler = counting_handler(&durable, effects.clone(), seen.clone());
    let outcome = consumer.process_next(&mut handler).await.expect("process");
    assert!(matches!(outcome, ProcessOutcome::Processed));
    assert_eq!(effects.load(Ordering::SeqCst), 1, "no duplicate side effect");
}

/// Crash mid-batch: two pending entries, the mark-crash fires on the first —
/// the whole batch is aborted and BOTH entries stay pending; after recovery
/// both publish (deduplicated / fresh as applicable) and the consumer sees
/// each exactly once. Broker-restart no-loss evidence (with the recorded
/// restart proxy).
#[tokio::test]
async fn crash_mid_batch_leaves_all_entries_pending_and_recovers_without_loss() {
    let Some(url) = nats_url() else { return };
    let client = async_nats::connect(&url).await.expect("connect to NATS");
    let js = async_nats::jetstream::new(client);
    provision_phase_1_streams(&js, &limits()).await.expect("provision streams");

    let suffix = run_suffix();
    let subject_a = format!("flexcore.v1.registry.test_batch_a_{suffix}");
    let subject_b = format!("flexcore.v1.registry.test_batch_b_{suffix}");

    let outbox = Arc::new(CrashyMarkOutbox::new());
    append_entry(&outbox, &format!("batch-a-{suffix}"), &subject_a, r#"{"n":1}"#);
    append_entry(&outbox, &format!("batch-b-{suffix}"), &subject_b, r#"{"n":2}"#);

    outbox.fail_marks.store(true, Ordering::SeqCst);
    let publisher = OutboxPublisher::new(&js, outbox.as_ref(), PublisherConfig::default());
    assert!(publisher.drain_once().await.is_err(), "mark crash aborts the batch");
    assert_eq!(
        outbox.pending(10).expect("pending").len(),
        2,
        "crash mid-batch: BOTH entries stay pending"
    );

    outbox.fail_marks.store(false, Ordering::SeqCst);
    let report = publisher.drain_once().await.expect("recovery drain");
    // Entry A was already on the broker (published before the crash) →
    // deduplicated; entry B is fresh.
    assert_eq!(report.broker_deduplicated, 1);
    assert_eq!(report.published, 1);
    assert_eq!(report.marked_delivered, 2);
    assert_eq!(outbox.pending(10).expect("pending").len(), 0);

    // Consumer: each subject exactly once.
    let inbox = InMemoryInboxStore::new();
    for (durable_tag, filter) in [("a", subject_a), ("b", subject_b)] {
        let durable = format!("registry-batch-{durable_tag}-{suffix}");
        let policy = ConsumerPolicy {
            durable_name: durable.clone(),
            stream: STREAM_REGISTRY.to_string(),
            filter_subject: filter,
            ack_wait: Duration::from_secs(5),
            max_deliver: 3,
            max_ack_pending: 100,
        };
        provision_durable_pull_consumer(&js, &policy).await.expect("provision");
        let consumer = IdempotentConsumer::new(&js, &inbox, ConsumerConfig::from_policy(&policy));
        let effects = Arc::new(AtomicUsize::new(0));
        let seen = Arc::new(Mutex::new(Vec::new()));
        let mut handler = counting_handler(&durable, effects.clone(), seen.clone());
        let outcome = consumer.process_next(&mut handler).await.expect("process");
        assert!(matches!(outcome, ProcessOutcome::Processed));
        assert_eq!(effects.load(Ordering::SeqCst), 1, "one side effect per event");
    }
}

/// Broker unavailability → publish failures leave entries pending with
/// attempt counts and NOTHING marked delivered; when the target stream
/// exists ("the broker came back"), the entries publish — no loss, no
/// premature marking. (A dead-socket drain is the Compose stack's
/// server-kill test; an unrouted subject exercises the same failure path
/// deterministically.)
#[tokio::test]
async fn broker_unavailable_entries_stay_pending_and_publish_after_recovery() {
    let Some(url) = nats_url() else { return };
    let client = async_nats::connect(&url).await.expect("connect to NATS");
    let js = async_nats::jetstream::new(client);

    let suffix = run_suffix();
    let subject = format!("flexcore.v1.dispatch.test_{suffix}");
    let key = format!("key-{suffix}");

    let outbox = FakeEventOutbox::new(FakeWriteJournal::new());
    append_entry(&outbox, &key, &subject, r#"{"event":"DispatchProposed"}"#);

    // No FLEXCORE_DISPATCH stream yet: every publish attempt fails.
    let publisher = OutboxPublisher::new(&js, &outbox, PublisherConfig::default());
    let report = publisher.drain_once().await.expect("drain with failures");
    assert_eq!(report.attempt_failures, 1);
    assert_eq!(report.marked_delivered, 0, "never marked without an ack");
    let pending = outbox.pending(10).expect("pending");
    assert_eq!(pending.len(), 1, "the entry stays pending");
    assert_eq!(pending[0].attempt_count(), 1, "the attempt is recorded");
    assert_eq!(pending[0].idempotency_key(), key);

    // "Recovery": the stream appears; the same pending entry publishes.
    let definition = flexcore_nats_adapter::topology::STREAMS
        .iter()
        .find(|d| d.name == STREAM_DISPATCH)
        .expect("dispatch stream definition");
    flexcore_nats_adapter::topology::provision_stream(&js, definition, &limits())
        .await
        .expect("provision dispatch stream");
    let report = publisher.drain_once().await.expect("drain after recovery");
    assert_eq!(report.published, 1);
    assert_eq!(report.marked_delivered, 1);
    assert_eq!(outbox.pending(10).expect("pending").len(), 0, "no loss");

    // Consumer sees it exactly once.
    let durable = format!("dispatch-recovery-{suffix}");
    let policy = ConsumerPolicy {
        durable_name: durable.clone(),
        stream: STREAM_DISPATCH.to_string(),
        filter_subject: subject.clone(),
        ack_wait: Duration::from_secs(5),
        max_deliver: 3,
        max_ack_pending: 100,
    };
    provision_durable_pull_consumer(&js, &policy).await.expect("provision");
    let inbox = InMemoryInboxStore::new();
    let consumer = IdempotentConsumer::new(&js, &inbox, ConsumerConfig::from_policy(&policy));
    let effects = Arc::new(AtomicUsize::new(0));
    let seen = Arc::new(Mutex::new(Vec::new()));
    let mut handler = counting_handler(&durable, effects.clone(), seen.clone());
    assert!(matches!(
        consumer.process_next(&mut handler).await.expect("process"),
        ProcessOutcome::Processed
    ));
    assert_eq!(effects.load(Ordering::SeqCst), 1);
}

/// P1.16 acceptance (ack-after-commit): the handler commits (inbox record
/// written), then the consumer "crashes" before acknowledging. The
/// redelivery after `ack_wait` is acknowledged WITHOUT re-executing the
/// side effect.
#[tokio::test]
async fn crash_between_commit_and_ack_redelivery_is_acknowledged_without_reexecution() {
    let Some(url) = nats_url() else { return };
    let client = async_nats::connect(&url).await.expect("connect to NATS");
    let js = async_nats::jetstream::new(client);
    provision_phase_1_streams(&js, &limits()).await.expect("provision streams");

    let suffix = run_suffix();
    let subject = format!("flexcore.v1.audit.test_{suffix}");
    let key = format!("key-{suffix}");

    let outbox = FakeEventOutbox::new(FakeWriteJournal::new());
    append_entry(&outbox, &key, &subject, r#"{"event":"OperatorActionRecorded"}"#);
    let publisher = OutboxPublisher::new(&js, &outbox, PublisherConfig::default());
    assert_eq!(publisher.drain_once().await.expect("drain").published, 1);

    let durable = format!("audit-commitack-{suffix}");
    let policy = ConsumerPolicy {
        durable_name: durable.clone(),
        stream: STREAM_AUDIT.to_string(),
        filter_subject: subject.clone(),
        // Short ack_wait so the unacked post-commit delivery is redelivered
        // quickly (the broker's redelivery timing, deliberately exercised).
        ack_wait: Duration::from_millis(500),
        max_deliver: 5,
        max_ack_pending: 100,
    };
    provision_durable_pull_consumer(&js, &policy).await.expect("provision");
    let inbox = InMemoryInboxStore::new();
    let consumer = IdempotentConsumer::new(&js, &inbox, ConsumerConfig::from_policy(&policy));
    let effects = Arc::new(AtomicUsize::new(0));
    let seen = Arc::new(Mutex::new(Vec::new()));

    // First delivery: the handler runs and COMMITS (the inbox record is
    // written) — and then the process crashes before any ack: the pulled
    // message is simply dropped unacknowledged.
    let mut handler = counting_handler(&durable, effects.clone(), seen.clone());
    let first = consumer.pull_one().await.expect("pull").expect("message");
    assert_eq!(first.idempotency_key.as_deref(), Some(key.as_str()));
    handler(&first, &inbox).expect("handler commit");
    assert_eq!(effects.load(Ordering::SeqCst), 1);
    drop(first); // the crash: no ack for a committed side effect

    // Redelivery (after ack_wait): the inbox check answers, the message is
    // acknowledged, and the side effect is NOT re-executed.
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    let redelivered = loop {
        match consumer.pull_one().await.expect("pull redelivery") {
            Some(m) => break m,
            None => {
                assert!(
                    std::time::Instant::now() < deadline,
                    "no redelivery within 10s of the ack_wait"
                );
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
    };
    assert_eq!(redelivered.idempotency_key.as_deref(), Some(key.as_str()));
    assert!(
        redelivered.delivery_count.unwrap_or(0) >= 2,
        "the redelivery must carry an incremented delivery count"
    );
    let outcome = consumer
        .process_delivered(redelivered, &mut handler)
        .await
        .expect("process redelivery");
    assert!(matches!(outcome, ProcessOutcome::SkippedDuplicate));
    assert_eq!(effects.load(Ordering::SeqCst), 1, "no duplicate side effect");
    assert_eq!(seen.lock().expect("test mutex").len(), 1);
}

/// P1.16 step 4 / P1.14 §5.2: a handler that keeps failing is NAKed below
/// `max_deliver`, and on the final attempt the original message plus
/// failure metadata lands on `FLEXCORE_DLQ` (via
/// `flexcore.dlq.v1.<area>.<event_slug>`), the delivery is terminated, and
/// the quarantine is alerted (the ERROR log line + the
/// `ProcessReport.quarantined` counter — the P1.20 metric hookup). Never a
/// silent drop.
#[tokio::test]
async fn poison_message_reaches_flexcore_dlq_after_max_deliver() {
    let Some(url) = nats_url() else { return };
    let client = async_nats::connect(&url).await.expect("connect to NATS");
    let js = async_nats::jetstream::new(client);
    provision_phase_1_streams(&js, &limits()).await.expect("provision streams");

    let suffix = run_suffix();
    let subject = format!("flexcore.v1.audit.test_poison_{suffix}");
    let key = format!("poison-{suffix}");

    let outbox = FakeEventOutbox::new(FakeWriteJournal::new());
    append_entry(&outbox, &key, &subject, r#"{"event":"OperatorActionRecorded"}"#);
    let publisher = OutboxPublisher::new(&js, &outbox, PublisherConfig::default());
    assert_eq!(publisher.drain_once().await.expect("drain").published, 1);

    let durable = format!("audit-poison-{suffix}");
    let policy = ConsumerPolicy {
        durable_name: durable.clone(),
        stream: STREAM_AUDIT.to_string(),
        filter_subject: subject.clone(),
        ack_wait: Duration::from_secs(5),
        max_deliver: 2,
        max_ack_pending: 100,
    };
    provision_durable_pull_consumer(&js, &policy).await.expect("provision");
    let inbox = InMemoryInboxStore::new();
    let config = ConsumerConfig {
        retry_delay: Duration::from_millis(100),
        ..ConsumerConfig::from_policy(&policy)
    };
    let consumer = IdempotentConsumer::new(&js, &inbox, config);

    // A handler that always fails (the poison).
    let mut poison_handler = |_message: &DeliveredMessage,
                              _inbox: &flexcore_nats_adapter::inbox::InboxStore|
     -> Result<(), HandlerError> { Err(HandlerError("poison: always fails".to_string())) };

    // Attempt 1 → below max_deliver → NAK, prompt redelivery.
    let first = consumer.pull_one().await.expect("pull").expect("message");
    let outcome = consumer
        .process_delivered(first, &mut poison_handler)
        .await
        .expect("process");
    assert!(matches!(outcome, ProcessOutcome::RetryScheduled));

    // Attempt 2 → delivery count reached max_deliver → DLQ + TERM.
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    let second = loop {
        match consumer.pull_one().await.expect("pull redelivery") {
            Some(m) => break m,
            None => {
                assert!(
                    std::time::Instant::now() < deadline,
                    "no NAK redelivery within 10s"
                );
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        }
    };
    assert!(second.delivery_count.unwrap_or(0) >= 2);
    let dlq_subject = flexcore_nats_adapter::topology::dlq_subject_for(&subject)
        .expect("conforming test subject");
    let outcome = consumer
        .process_delivered(second, &mut poison_handler)
        .await
        .expect("process final attempt");
    assert!(matches!(outcome, ProcessOutcome::Quarantined));

    // The quarantine copy is on FLEXCORE_DLQ with failure metadata.
    let reply: serde_json::Value = js
        .request(
            "STREAM.MSG.GET.FLEXCORE_DLQ",
            &serde_json::json!({ "last_by_subj": dlq_subject }),
        )
        .await
        .expect("DLQ copy must exist on the FLEXCORE_DLQ stream");
    let data_b64 = reply["message"]["data"]
        .as_str()
        .expect("the DLQ copy carries the metadata payload");
    let payload = String::from_utf8(decode_base64(data_b64)).expect("JSON metadata");
    assert!(payload.contains(&durable), "metadata names the consumer");
    assert!(payload.contains(&subject), "metadata names the original subject");
    assert!(payload.contains(&key), "metadata carries the idempotency key");
    assert!(payload.contains("poison: always fails"), "metadata carries the last error");

    // The terminated original is NOT redelivered again: the consumer goes
    // idle (nothing new within the window) instead of looping forever.
    tokio::time::sleep(Duration::from_millis(300)).await;
    let after = consumer.pull_one().await.expect("pull after term");
    assert!(after.is_none(), "a terminated poison message must not redeliver");
}

/// Minimal base64 (standard alphabet) decoder for the JetStream
/// `STREAM.MSG.GET` response — std-only, test use only.
fn decode_base64(input: &str) -> Vec<u8> {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    fn value(byte: u8) -> Option<u32> {
        TABLE.iter().position(|&t| t == byte).map(|p| p as u32)
    }
    let values: Vec<u32> = input
        .bytes()
        .filter(|b| !matches!(b, b'=' | b'\n' | b'\r'))
        .map(|b| value(b).expect("valid base64 input from JetStream"))
        .collect();
    let mut out = Vec::with_capacity(values.len() * 3 / 4 + 3);
    for chunk in values.chunks(4) {
        let word: u32 = chunk
            .iter()
            .enumerate()
            .map(|(i, &v)| v << (18 - 6 * i))
            .fold(0, |acc, v| acc | v);
        out.push((word >> 16) as u8);
        if chunk.len() > 2 {
            out.push((word >> 8) as u8);
        }
        if chunk.len() > 3 {
            out.push(word as u8);
        }
    }
    out
}
