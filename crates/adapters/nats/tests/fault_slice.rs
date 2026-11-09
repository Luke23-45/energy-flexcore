// P1.24 — Fault-injection test slice (broker): duplicated delivery,
//         reordered publish, broker outage, poison message (ledger P1.24;
//         report §19.2 test layer 6, §6.3–§6.4, §21.2; P1.15/P1.16).
//
// GATING (mirrors crates/adapters/nats/tests/integration.rs): every broker
// test is gated on `FLEXCORE_TEST_NATS_URL` and SKIPS CLEANLY (returns
// without asserting) when the variable is unset or empty. No test in this
// file needs PostgreSQL — the consumer handlers use the adapter's
// `InMemoryInboxStore` test double, exactly like the P1.15/P1.16
// integration tests; the durable inbox variant is the postgres adapter's.
//
// Each test FIRST asserts the injected fault condition actually held
// (ledger P1.24 risk note: "fault tests that pass because the fault was
// never actually injected"), then asserts the recovery/guard property.
//
// Threats covered (ledger P0.17 §5 = report §17.4):
//   - Row 1 "Forged/replayed command" — replay/duplicate half: the same
//     idempotency key (Nats-Msg-Id) produces ONE side effect (row 6 too).
//   - Row 6 "Broker duplication/replay" — duplicate delivery, reordered
//     publication, inbox dedup by idempotency key, outbox/inbox pattern.
//   - Row 10 "Central outage" — Phase 1 slice: broker outage leaves the
//     outbox pending and drains with zero loss on recovery (the site-local
//     EMS half is post-Phase-1, §17.4 row 10 control).
//
// Cases (ledger P1.24 steps 2–4):
//   (a) duplicated delivery  -> single side effect;
//   (b) reordered publish per asset stream -> consumer handles without
//       corrupting state; inbox dedup asserted by idempotency key;
//   (c) broker outage -> outbox entries stay pending with attempt counts;
//       drain after recovery with zero loss;
//   (d) poison message -> FLEXCORE_DLQ after max_deliver, quarantine
//       counted in ProcessReport, never silently dropped.
//
// Recorded limits (mirroring the P1.15/P1.16 file): a full server-kill
// outage is the P1.25 Compose stack's job; here the outage is proxied by
// publishing to a stream that does not exist (the same failure path: the
// broker rejects the publish, the entry stays pending). FixedClock note:
// the public fake constructor is `FixedClock::at_unix_millis`.
//
// Recorded gap (P1.24 step 1, dropped messages): "kill the consumer
// mid-batch" redelivery is covered for the outbox publisher half by the
// existing P1.15 crash-mid-batch test and for the consumer half by the
// P1.16 crash-between-commit-and-ack test (both in
// crates/adapters/nats/tests/integration.rs); a hard process-kill variant
// needs the Compose stack (P1.25) and is listed as not-yet-covered in the
// coverage note (ledger/phase-01-rust-foundation/23_24_test_coverage_note.md).

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use flexcore_nats_adapter::consumer::{
    ConsumerConfig, DeliveredMessage, HandlerError, IdempotentConsumer, ProcessOutcome,
};
use flexcore_nats_adapter::inbox::{InMemoryInboxStore, InboxStore, RecordOutcome};
use flexcore_nats_adapter::publisher::{OutboxPublisher, PublisherConfig};
use flexcore_nats_adapter::topology::{
    dlq_subject_for, provision_durable_pull_consumer, provision_phase_1_streams,
    provision_stream, ConsumerPolicy, DeploymentLimits, STREAM_AUDIT, STREAM_DISPATCH,
    STREAM_REGISTRY, STREAMS,
};
use flexcore_ports::clock::Clock;
use flexcore_ports::event_outbox::OutboxWrite;
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
/// step-1 composition root).
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

/// A handler that records the inbox entry under the consumer's durable
/// identity, counts side effects, and records the arrival order of the
/// idempotency keys (the ordering evidence for the reorder case).
fn ordering_handler(
    consumer_name: &str,
    effects: Arc<AtomicUsize>,
    arrival_order: Arc<Mutex<Vec<String>>>,
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
        arrival_order
            .lock()
            .expect("test mutex")
            .push(key.to_string());
        effects.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

/// Polls `pull_one` until a message arrives or the deadline passes.
async fn pull_until(
    consumer: &IdempotentConsumer<'_>,
    deadline_secs: u64,
) -> Option<DeliveredMessage> {
    let deadline = std::time::Instant::now() + Duration::from_secs(deadline_secs);
    loop {
        match consumer.pull_one().await.expect("pull") {
            Some(message) => return Some(message),
            None => {
                if std::time::Instant::now() > deadline {
                    return None;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
    }
}

// --- (a) Duplicate delivery -> single side effect (§17.4 rows 1/6) -------------

/// The fault: the same idempotency key is published TWICE (a replayed or
/// duplicated publication — the exact shape of §17.4 row 6). FIRST the test
/// asserts the duplicate actually occurred on the broker (the second
/// publish is acknowledged as a duplicate within the window), THEN that the
/// consumer executed the side effect exactly once — and that a re-delivered
/// message is skipped by the inbox (idempotency key dedup).
#[tokio::test]
async fn duplicated_publication_and_redelivery_yield_one_side_effect() {
    let Some(url) = nats_url() else { return };
    let client = async_nats::connect(&url).await.expect("connect to NATS");
    let js = async_nats::jetstream::new(client);
    provision_phase_1_streams(&js, &limits()).await.expect("provision streams");

    let suffix = run_suffix();
    let subject = format!("flexcore.v1.registry.dup_{suffix}");
    let key = format!("dup-key-{suffix}");

    // Two outboxes carrying the SAME idempotency key, subject, and payload
    // — the replayed/duplicated publication.
    let first = FakeEventOutbox::new(FakeWriteJournal::new());
    let second = FakeEventOutbox::new(FakeWriteJournal::new());
    append_entry(&first, &key, &subject, r#"{"event":"AssetRegistered"}"#);
    append_entry(&second, &key, &subject, r#"{"event":"AssetRegistered"}"#);

    // First publication: fresh.
    let report = OutboxPublisher::new(&js, &first, PublisherConfig::default())
        .drain_once()
        .await
        .expect("first drain");
    assert_eq!(report.published, 1, "the first publication is fresh");
    assert_eq!(report.broker_deduplicated, 0);

    // FAULT ASSERTION: the second publication of the same key is
    // acknowledged by the broker as a duplicate — the duplicate delivery
    // actually happened.
    let report = OutboxPublisher::new(&js, &second, PublisherConfig::default())
        .drain_once()
        .await
        .expect("duplicate drain");
    assert_eq!(
        report.broker_deduplicated, 1,
        "the fault must be real: the second publish is broker-deduplicated by Nats-Msg-Id"
    );
    assert_eq!(report.published, 0);

    // GUARD: exactly one message is on the stream, and the consumer
    // executes the side effect exactly once.
    let durable = format!("registry-dup-{suffix}");
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
    let arrival = Arc::new(Mutex::new(Vec::new()));
    let mut handler = ordering_handler(&durable, effects.clone(), arrival.clone());

    let message = pull_until(&consumer, 10)
        .await
        .expect("exactly one delivered message for the duplicated key");
    assert_eq!(message.idempotency_key.as_deref(), Some(key.as_str()));
    let outcome = consumer
        .process_delivered(message.clone(), &mut handler)
        .await
        .expect("process first delivery");
    assert!(matches!(outcome, ProcessOutcome::Processed));

    // The redelivery of the same message (what a duplicate/replay looks
    // like to the consumer) is skipped by the inbox — no second effect.
    let outcome = consumer
        .process_delivered(message, &mut handler)
        .await
        .expect("process redelivery");
    assert!(
        matches!(outcome, ProcessOutcome::SkippedDuplicate),
        "the redelivery must be skipped by the inbox dedup"
    );
    assert_eq!(effects.load(Ordering::SeqCst), 1, "exactly ONE side effect");
    assert_eq!(
        arrival.lock().expect("test mutex").len(),
        1,
        "the side effect ran for exactly one idempotency key"
    );
    assert_eq!(inbox.len(), 1, "one inbox record, keyed by the idempotency key");
}

// --- (b) Reordered publish per asset stream -> no state corruption -------------

/// The fault: two events for the same asset stream are published in
/// REVERSE order — sequence 2 is published (and acknowledged) BEFORE
/// sequence 1, so the stream holds them out of order and the consumer
/// receives the newer event first. FIRST the test asserts the reorder
/// actually happened (arrival order is [seq-2, seq-1]), THEN that the
/// consumer handled both without corrupting state: two distinct
/// idempotency keys, two inbox records, two side effects, and re-presenting
/// the out-of-order older event changes nothing (inbox dedup by key).
#[tokio::test]
async fn reordered_publish_per_asset_stream_is_handled_without_state_corruption() {
    let Some(url) = nats_url() else { return };
    let client = async_nats::connect(&url).await.expect("connect to NATS");
    let js = async_nats::jetstream::new(client);
    provision_phase_1_streams(&js, &limits()).await.expect("provision streams");

    let suffix = run_suffix();
    // One subject = one asset event stream (per-asset stream ordering per
    // P1.14); the payload carries the source sequence the consumer would
    // order by.
    let subject = format!("flexcore.v1.registry.reorder_{suffix}");
    let key_seq1 = format!("reorder-1-{suffix}");
    let key_seq2 = format!("reorder-2-{suffix}");

    // FAULT INJECTION: publish sequence 2 FIRST, await its ack, then
    // sequence 1. The outbox drains in insertion order, so seq 2 goes into
    // its outbox first.
    let outbox_newer = FakeEventOutbox::new(FakeWriteJournal::new());
    let outbox_older = FakeEventOutbox::new(FakeWriteJournal::new());
    append_entry(
        &outbox_newer,
        &key_seq2,
        &subject,
        r#"{"event":"AssetRegistered","source_sequence":2}"#,
    );
    append_entry(
        &outbox_older,
        &key_seq1,
        &subject,
        r#"{"event":"AssetRegistered","source_sequence":1}"#,
    );
    let report = OutboxPublisher::new(&js, &outbox_newer, PublisherConfig::default())
        .drain_once()
        .await
        .expect("drain newer first");
    assert_eq!(report.published, 1, "seq 2 published BEFORE seq 1");
    let report = OutboxPublisher::new(&js, &outbox_older, PublisherConfig::default())
        .drain_once()
        .await
        .expect("drain older second");
    assert_eq!(report.published, 1);

    // GUARD: the consumer processes both deliveries in the (corrupted)
    // arrival order without losing or duplicating state.
    let durable = format!("registry-reorder-{suffix}");
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
    let arrival = Arc::new(Mutex::new(Vec::new()));
    let mut handler = ordering_handler(&durable, effects.clone(), arrival.clone());

    // FAULT ASSERTION: the first delivery really is the NEWER event (seq 2
    // before seq 1) — the reorder actually reached the consumer.
    let first = pull_until(&consumer, 10).await.expect("first delivery");
    assert_eq!(
        first.idempotency_key.as_deref(),
        Some(key_seq2.as_str()),
        "the reorder must be real: the newer event arrives first"
    );
    let outcome = consumer
        .process_delivered(first, &mut handler)
        .await
        .expect("process newer-first");
    assert!(matches!(outcome, ProcessOutcome::Processed));

    let second = pull_until(&consumer, 10).await.expect("second delivery");
    assert_eq!(second.idempotency_key.as_deref(), Some(key_seq1.as_str()));
    let outcome = consumer
        .process_delivered(second.clone(), &mut handler)
        .await
        .expect("process older-second");
    assert!(matches!(outcome, ProcessOutcome::Processed));

    // No state corruption: both keys recorded exactly once, in arrival
    // order (newer first — the consumer did NOT silently reorder or drop).
    {
        let arrival = arrival.lock().expect("test mutex");
        assert_eq!(arrival.len(), 2, "both events produced their side effect");
        assert_eq!(arrival[0], key_seq2, "arrival order preserved (reordered)");
        assert_eq!(arrival[1], key_seq1);
    }
    assert_eq!(effects.load(Ordering::SeqCst), 2);
    assert_eq!(inbox.len(), 2, "one inbox record per distinct idempotency key");

    // Inbox dedup by idempotency key: re-presenting the older event
    // (replay after the reorder) is acknowledged without a third effect.
    let outcome = consumer
        .process_delivered(second, &mut handler)
        .await
        .expect("re-present older event");
    assert!(matches!(outcome, ProcessOutcome::SkippedDuplicate));
    assert_eq!(effects.load(Ordering::SeqCst), 2, "no state corruption: still 2 effects");
    assert_eq!(inbox.len(), 2);
}

// --- (c) Broker outage -> pending with attempts, drain after recovery ----------

/// The fault: the broker cannot accept the publication (the target stream
/// does not exist — the same failure path a broker outage takes: the
/// publish is not acknowledged). FIRST the test asserts the outage held
/// (attempt recorded, nothing marked delivered), THEN that recovery drains
/// the backlog with ZERO loss.
#[tokio::test]
async fn broker_outage_keeps_outbox_pending_and_drains_after_recovery_with_zero_loss() {
    let Some(url) = nats_url() else { return };
    let client = async_nats::connect(&url).await.expect("connect to NATS");
    let js = async_nats::jetstream::new(client);
    // Deliberately NO provision_phase_1_streams here: the dispatch stream
    // does not exist yet — that IS the outage proxy.

    let suffix = run_suffix();
    let subject_a = format!("flexcore.v1.dispatch.outage_a_{suffix}");
    let subject_b = format!("flexcore.v1.dispatch.outage_b_{suffix}");
    let key_a = format!("outage-a-{suffix}");
    let key_b = format!("outage-b-{suffix}");

    let outbox = FakeEventOutbox::new(FakeWriteJournal::new());
    append_entry(&outbox, &key_a, &subject_a, r#"{"n":1}"#);
    append_entry(&outbox, &key_b, &subject_b, r#"{"n":2}"#);

    // During the outage: every publish fails.
    let publisher = OutboxPublisher::new(&js, &outbox, PublisherConfig::default());
    let report = publisher.drain_once().await.expect("drain during outage");
    // FAULT ASSERTION: the outage actually held — both publishes failed.
    assert_eq!(
        report.attempt_failures, 2,
        "the fault must be real: both publishes failed while the stream was absent"
    );
    assert_eq!(report.marked_delivered, 0, "nothing marked without a broker ack");
    assert_eq!(report.published, 0);

    // The entries stay pending WITH attempt counts (no loss, no silent drop).
    let pending = outbox.pending(10).expect("pending read");
    assert_eq!(pending.len(), 2, "both entries stay pending through the outage");
    for entry in &pending {
        assert_eq!(entry.attempt_count(), 1, "each failure recorded one attempt");
    }
    let keys: Vec<_> = pending.iter().map(|e| e.idempotency_key().to_string()).collect();
    assert!(keys.contains(&key_a) && keys.contains(&key_b));

    // RECOVERY: the stream appears ("the broker came back").
    let definition = STREAMS
        .iter()
        .find(|d| d.name == STREAM_DISPATCH)
        .expect("dispatch stream definition exists");
    provision_stream(&js, definition, &limits())
        .await
        .expect("provision the stream = recovery");

    let report = publisher.drain_once().await.expect("drain after recovery");
    assert_eq!(
        report.published + report.broker_deduplicated, 2,
        "the whole backlog publishes after recovery"
    );
    assert_eq!(report.marked_delivered, 2, "marked only after the ack");
    assert_eq!(
        outbox.pending(10).expect("pending").len(),
        0,
        "ZERO loss: the backlog is fully drained"
    );

    // The consumer sees both events, each exactly once.
    let inbox = InMemoryInboxStore::new();
    let effects = Arc::new(AtomicUsize::new(0));
    for (tag, filter, key) in [
        ("a", subject_a.clone(), key_a.clone()),
        ("b", subject_b.clone(), key_b.clone()),
    ] {
        let durable = format!("dispatch-outage-{tag}-{suffix}");
        let policy = ConsumerPolicy {
            durable_name: durable.clone(),
            stream: STREAM_DISPATCH.to_string(),
            filter_subject: filter,
            ack_wait: Duration::from_secs(5),
            max_deliver: 3,
            max_ack_pending: 100,
        };
        provision_durable_pull_consumer(&js, &policy).await.expect("provision");
        let consumer = IdempotentConsumer::new(&js, &inbox, ConsumerConfig::from_policy(&policy));
        let arrival = Arc::new(Mutex::new(Vec::new()));
        let mut handler = ordering_handler(&durable, effects.clone(), arrival);
        let message = pull_until(&consumer, 10).await.expect("delivered message");
        assert_eq!(message.idempotency_key.as_deref(), Some(key.as_str()));
        assert!(matches!(
            consumer.process_delivered(message, &mut handler).await.expect("process"),
            ProcessOutcome::Processed
        ));
    }
    assert_eq!(effects.load(Ordering::SeqCst), 2, "zero loss end to end");
}

// --- (d) Poison message -> DLQ after max_deliver, quarantine counted ----------

/// The fault: a handler that always fails (a poison message). FIRST the
/// test asserts the retry path actually engaged (attempt 1 below
/// max_deliver is NAKed for redelivery with an incremented delivery
/// count), THEN that the final attempt quarantines the message to
/// FLEXCORE_DLQ with failure metadata, terminates the original (no
/// infinite redelivery), and COUNTS the quarantine in `ProcessReport`
/// (driven through the public `run` loop — the P1.20 metric hookup).
#[tokio::test]
async fn poison_message_reaches_the_dlq_after_max_deliver_with_a_counted_quarantine() {
    let Some(url) = nats_url() else { return };
    let client = async_nats::connect(&url).await.expect("connect to NATS");
    let js = async_nats::jetstream::new(client);
    provision_phase_1_streams(&js, &limits()).await.expect("provision streams");

    let suffix = run_suffix();
    let subject = format!("flexcore.v1.audit.poison_{suffix}");
    let key = format!("poison-{suffix}");

    let outbox = FakeEventOutbox::new(FakeWriteJournal::new());
    append_entry(&outbox, &key, &subject, r#"{"event":"OperatorActionRecorded"}"#);
    let pub_report = OutboxPublisher::new(&js, &outbox, PublisherConfig::default())
        .drain_once()
        .await
        .expect("drain");
    assert_eq!(pub_report.published, 1, "the poison message is on the stream");

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
        idle_interval: Duration::from_millis(100),
        error_backoff: Duration::from_millis(100),
        ..ConsumerConfig::from_policy(&policy)
    };
    let consumer = IdempotentConsumer::new(&js, &inbox, config.clone());

    let mut poison_handler =
        |_message: &DeliveredMessage, _inbox: &dyn InboxStore| -> Result<(), HandlerError> {
            Err(HandlerError("poison: always fails".to_string()))
        };

    // FAULT ASSERTION part 1: attempt 1 fails below max_deliver and is
    // NAKed for redelivery — the retry path engaged.
    let first = pull_until(&consumer, 10).await.expect("first delivery");
    assert!(first.delivery_count.unwrap_or(0) >= 1);
    let outcome = consumer
        .process_delivered(first, &mut poison_handler)
        .await
        .expect("process attempt 1");
    assert!(
        matches!(outcome, ProcessOutcome::RetryScheduled),
        "the fault must be real: attempt 1 is NAKed for redelivery"
    );

    // Drive the production `run` loop (a second helper instance over the
    // same durable identity — the instance is a cheap context clone) until
    // the poison message is quarantined, then stop it. The loop runs on
    // THIS task (the consumer borrows the stack-local inbox, so it cannot
    // be moved into a spawned task); a tiny spawned timer fires the
    // shutdown watch after enough cycles for the NAK redelivery +
    // quarantine to happen.
    let run_consumer = IdempotentConsumer::new(&js, &inbox, config.clone());
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    tokio::spawn({
        let shutdown_tx = shutdown_tx.clone();
        async move {
            tokio::time::sleep(Duration::from_secs(3)).await;
            let _ = shutdown_tx.send(true);
        }
    });
    let mut run_handler =
        |_message: &DeliveredMessage, _inbox: &dyn InboxStore| -> Result<(), HandlerError> {
            Err(HandlerError("poison: always fails".to_string()))
        };
    // The loop: redelivery (attempt 2 == max_deliver) -> quarantine; then
    // the timer shuts it down.
    let run_report = run_consumer.run(&mut run_handler, shutdown_rx).await;

    // GUARD: the quarantine happened and was COUNTED (never silent).
    assert!(
        run_report.quarantined >= 1,
        "the poison path must count the quarantine: {run_report:?}"
    );
    assert_eq!(
        run_report.processed, 0,
        "a poison message never executes its side effect"
    );
    drop(shutdown_tx);

    // The DLQ copy exists with the failure metadata.
    let dlq_subject = dlq_subject_for(&subject).expect("conforming test subject");
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
    let metadata = String::from_utf8(decode_base64(data_b64)).expect("JSON metadata");
    assert!(metadata.contains(&durable), "metadata names the consumer");
    assert!(metadata.contains(&subject), "metadata names the original subject");
    assert!(metadata.contains(&key), "metadata carries the idempotency key");
    assert!(
        metadata.contains("poison: always fails"),
        "metadata carries the last handler error"
    );

    // The terminated original is NOT redelivered: no infinite poison loop.
    let after = pull_until(&consumer, 2).await;
    assert!(
        after.is_none(),
        "a terminated poison message must not redeliver"
    );
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
