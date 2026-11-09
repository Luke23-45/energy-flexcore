// P1.19 — Worker integration test: outbox → publisher → JetStream →
// idempotent consumer → inbox dedup (end-to-end over the same code paths
// the worker hosts).
//
// Gated on BOTH `FLEXCORE_TEST_DATABASE_URL` (PostgreSQL service, P1.13
// convention) and `FLEXCORE_TEST_NATS_URL` (NATS 2.15.x service, P1.15
// convention); skips cleanly when either is unset (CI provides both).
//
// What is asserted (ledger P1.15/P1.16/P1.19 acceptance overlap):
// 1. An outbox entry created through the store's unit of work (state
//    change + outbox row in one transaction, report §6.3 step 1) is
//    published by the P1.15 publisher and marked delivered ONLY after the
//    broker acknowledgement.
// 2. A re-publication of the same `Nats-Msg-Id` outside the duplicate
//    window is delivered again and the P1.16 consumer skips it via the
//    durable inbox — two deliveries, ONE side effect.
//
// Requires a multi-thread tokio runtime: the loops drive the sync ports
// under `block_in_place` (see `runner`'s module header).

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use flexcore_contracts::time_model::EventTime;
use flexcore_nats_adapter::consumer::{ConsumerConfig, IdempotentConsumer, ProcessOutcome};
use flexcore_nats_adapter::inbox::InboxStore;
use flexcore_nats_adapter::publisher::OutboxPublisher;
use flexcore_nats_adapter::topology::{
    provision_durable_pull_consumer, provision_phase_1_streams, ConsumerPolicy,
    DeploymentLimits, STREAM_REGISTRY,
};
use flexcore_ports::event_outbox::{EventOutbox, OutboxWrite};
use flexcore_ports::state_change::{StateChangeOperation, StateChangeReceipt};
use flexcore_postgres_adapter::FlexCoreStore;
use flexcore_worker::inbox::PgInboxStore;
use flexcore_worker::runner::{inbox_recording_handler, publisher_config};

const CONSUMER_DURABLE: &str = "registry-worker-v1";

fn env_url(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|url| url.trim().to_string())
        .filter(|url| !url.is_empty())
}

fn skipped(db: Option<String>, nats: Option<String>) -> bool {
    if db.is_none() || nats.is_none() {
        println!(
            "skipped worker_integration: FLEXCORE_TEST_DATABASE_URL and \
             FLEXCORE_TEST_NATS_URL must both be set (CI provides both services)"
        );
        return true;
    }
    false
}

/// The serialized envelope the postgres adapter classifies the outbox
/// columns from (`event.event_type` + top-level `schema_version`, P1.13).
fn probe_payload() -> String {
    serde_json::json!({
        "schema_version": "1",
        "event": {
            "event_type": "WORKER_PROBE_EVENT",
            "correlation_id": "00000000-0000-0000-0000-000000000000"
        }
    })
    .to_string()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn outbox_publish_and_idempotent_consume_end_to_end() {
    let db_url = env_url("FLEXCORE_TEST_DATABASE_URL");
    let nats_url = env_url("FLEXCORE_TEST_NATS_URL");
    if skipped(db_url.clone(), nats_url.clone()) {
        return;
    }
    let db_url = db_url.expect("checked");
    let nats_url = nats_url.expect("checked");

    // --- Stores -----------------------------------------------------------

    let store = FlexCoreStore::connect(&db_url).expect("store connects");
    store.run_migrations().expect("migrations apply");
    let inbox = PgInboxStore::new(store.pool().clone()).expect("inbox store starts");
    inbox.ensure_table().expect("inbox table exists");

    // --- Broker and topology ----------------------------------------------

    let client = async_nats::connect(&nats_url).await.expect("nats connects");
    let jetstream = async_nats::jetstream::new(client);

    // A deliberately tiny duplicate window: the test's second publish of
    // the same Nats-Msg-Id must pass the broker dedup and be delivered,
    // so the INBOX (not the window) is what collapses it.
    let limits = DeploymentLimits {
        duplicate_window: Duration::from_millis(10),
        max_age: None,
        max_messages: None,
        max_bytes: None,
    };
    provision_phase_1_streams(&jetstream, &limits)
        .await
        .expect("streams provision");
    let policy = ConsumerPolicy {
        durable_name: CONSUMER_DURABLE.to_string(),
        stream: STREAM_REGISTRY.to_string(),
        filter_subject: "flexcore.v1.registry.>".to_string(),
        ack_wait: Duration::from_secs(30),
        max_deliver: 5,
        max_ack_pending: 100,
    };
    provision_durable_pull_consumer(&jetstream, &policy)
        .await
        .expect("durable consumer provisions");

    // --- Outbox entry via the store (one transaction, §6.3 step 1) --------

    let key = format!("worker-it-{}", std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos());
    let subject = "flexcore.v1.registry.worker_probe_event";
    let created_at =
        EventTime::from_unix_millis(1_732_000_000_000).expect("fixed timestamp");
    let uow = store.begin(None).expect("unit of work opens");
    uow.append(
        OutboxWrite::new(
            // Intended for repository implementations and test fakes —
            // this is the test-fake use.
            StateChangeReceipt::new("assets".to_string(), StateChangeOperation::Insert, None),
            key.clone(),
            subject.to_string(),
            probe_payload(),
            created_at,
        )
        .expect("outbox write assembles"),
    )
    .expect("outbox row appends inside the unit of work");
    uow.commit().expect("state change + outbox row commit");

    // The entry is pending before the publisher runs.
    assert_eq!(
        store.pending(10).expect("pending reads").len(),
        1,
        "the outbox entry is pending before publication"
    );

    // --- Publisher: drain once, mark delivered only after ack --------------

    let publisher = OutboxPublisher::new(&jetstream, &store, publisher_config(Duration::from_millis(50)));
    let report = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(publisher.drain_once())
    })
    .expect("drain cycle succeeds");
    assert_eq!(report.drained, 1);
    assert_eq!(report.published, 1, "the broker acknowledged a fresh publish");
    assert_eq!(report.marked_delivered, 1, "delivered is marked after the ack");
    assert!(
        store.pending(10).expect("pending reads").is_empty(),
        "the entry left pending only if unacknowledged — it was acknowledged"
    );

    // --- Consumer: first delivery processed, side effect applied ----------

    let side_effects = Arc::new(AtomicUsize::new(0));
    let consumer = IdempotentConsumer::new(
        &jetstream,
        &inbox,
        ConsumerConfig {
            stream: STREAM_REGISTRY.to_string(),
            durable_name: CONSUMER_DURABLE.to_string(),
            max_deliver: policy.max_deliver,
            pull_expires: Duration::from_millis(500),
            idle_interval: Duration::from_millis(50),
            ..Default::default()
        },
    );
    let mut handler = inbox_recording_handler(
        CONSUMER_DURABLE.to_string(),
        Arc::clone(&side_effects),
    );

    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    let mut processed = false;
    while !processed && tokio::time::Instant::now() < deadline {
        match tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(consumer.process_next(&mut handler))
        }) {
            Ok(ProcessOutcome::Processed) => processed = true,
            Ok(_) => {}
            Err(_) => { /* retried on the next pull; the delivery is retained */ }
        }
    }
    assert!(processed, "the outbox-published event was consumed");
    assert_eq!(side_effects.load(Ordering::SeqCst), 1);
    assert!(
        inbox
            .is_recorded(CONSUMER_DURABLE, &key)
            .expect("inbox reads"),
        "the durable inbox holds the processed key"
    );

    // --- Duplicate delivery → one side effect ------------------------------

    // Re-publish the same Nats-Msg-Id (simulating the crash-between-
    // publish-and-mark re-publication; the duplicate window has passed).
    // The broker redelivers; the inbox check — not the window — dedupes.
    let duplicate = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            let mut headers = async_nats::header::HeaderMap::new();
            headers.insert(
                async_nats::header::NATS_MESSAGE_ID,
                key.clone(),
            );
            let ack = jetstream
                .publish_with_headers(subject, headers, probe_payload().into_bytes().into())
                .await
                .expect("duplicate publish sent");
            ack.await.expect("duplicate publish acknowledged");
        })
    });
    let _ = duplicate;

    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    let mut duplicate_seen = false;
    while !duplicate_seen && tokio::time::Instant::now() < deadline {
        match tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(consumer.process_next(&mut handler))
        }) {
            Ok(ProcessOutcome::SkippedDuplicate) => duplicate_seen = true,
            Ok(_) => {}
            Err(_) => {}
        }
    }
    assert!(
        duplicate_seen,
        "the re-publication was delivered and skipped via the inbox"
    );
    assert_eq!(
        side_effects.load(Ordering::SeqCst),
        1,
        "two deliveries produced exactly ONE side effect"
    );

    store.pool().close();
}
