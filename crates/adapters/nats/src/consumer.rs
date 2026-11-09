// P1.16 — The idempotent consumer helper (report §6.3 step 4, §6.4
// guardrail; P1.14 §5; ledger P1.16).
//
// The flow implemented here, per P1.16 step 2, in order:
//
//   1. **Check inbox** — if `(consumer_name, idempotency_key)` is already
//      recorded, the message is acknowledged WITHOUT re-executing the side
//      effect ([`ProcessOutcome::SkippedDuplicate`]). This is the safe
//      path for every redelivery: crash between commit and ack (P1.16
//      acceptance: "acknowledge-after-commit ordering is verified by
//      test"), duplicate delivery, replay.
//   2. **Process** — the caller-supplied handler applies the side effect
//      and records the inbox entry **inside its own one storage
//      transaction** (the binding obligation, stated on
//      [`InboxStore::record`] and in the crate README; the adapter cannot
//      name a transaction, the handler owns it).
//   3. **Ack only after the handler returned** — i.e. after its transaction
//      committed. A crash before the ack is safe: the redelivery hits the
//      inbox check in step 1.
//
// Poison path (P1.16 step 4, P1.14 §5.2): when a handler fails on a
// delivery whose delivery count has reached the consumer's `max_deliver`,
// the original message plus failure metadata is published to
// `flexcore.dlq.v1.<area>.<event_slug>` (captured by `FLEXCORE_DLQ`), the
// original delivery is terminated (`+TERM`), and the quarantine is logged
// at ERROR level and counted in [`ProcessReport`] (the P1.20 metric
// hookup). **A poison message is never silently dropped.** A message
// without an idempotency key cannot be made idempotent at all — it is
// quarantined on first delivery. If the server's own `max_deliver`
// terminates a delivery before this helper sees the final attempt (possible
// when helper and consumer provisioning disagree), the `$JS.EVENT.ADVISORY.
// CONSUMER.MAX_DELIVERIES.>` advisory stream is the secondary signal
// (P1.14 §5.2 item 4) — monitoring it is an operator/worker duty, not
// silently assumed here.
//
// # Transport seam (no `futures-util` in this crate)
//
// Pulls use the documented JetStream pull protocol over the core
// request/response surface: a request on
// `$JS.API.CONSUMER.MSG.NEXT.<stream>.<consumer>` with payload
// `{"batch":1,"expires":<nanos>}`; the reply is the delivered message (or
// the server's pull-expiry control reply, which async-nats surfaces as a
// `RequestError`). Acks (`+ACK` / `+TERM` / `-NAK`) are published to the
// delivered message's reply subject. Delivery counts come from the
// `$JS.ACK.<stream>.<consumer>.<delivered>.<sseq>.<dseq>.<ts>.<pending>`
// reply-subject token layout. Every protocol constant carries a
// VERIFY-BUILD note (a build-time verification instruction, not a deferred
// task). This keeps the crate free of `futures-util`/`tokio-stream`
// (`StreamExt`), which the P1.15 dependency ban forbids.
//
// # Sync handler contract (recorded decision)
//
// The handler is a **sync** `FnMut`, matching the house port rule (ports
// are sync; the adapter bridges asynchrony). A handler that needs async I/O
// bridges internally (block-in-place, or a channel hand-off) — the same
// recorded resolution as the publisher's sync `EventOutbox` port
// (src/lib.rs decision 1). The handler receives `&dyn InboxStore` so the
// inbox record can join its transaction; see the single-home decision for
// `InboxStore` in src/inbox.rs.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_nats::client::RequestErrorKind;
use async_nats::jetstream::Context;
use async_nats::{client::Client, header::HeaderMap, header::NATS_MESSAGE_ID};
use tokio::sync::watch;

use crate::inbox::InboxStore;
use crate::topology::{dlq_subject_for, DLQ_SUBJECT_PREFIX};

/// The JetStream pull API subject prefix (`$JS.API.CONSUMER.MSG.NEXT.
/// <stream>.<consumer>`) — the documented pull protocol endpoint.
const MSG_NEXT_PREFIX: &str = "$JS.API.CONSUMER.MSG.NEXT";

/// The `+ACK` acknowledgement payload (JetStream ack protocol).
const ACK_PAYLOAD: &[u8] = b"+ACK";
/// The `+TERM` termination payload: the message is never redelivered.
const TERM_PAYLOAD: &[u8] = b"+TERM";

/// A message delivered by JetStream, reduced to what the idempotent flow
/// needs. `delivery_count` is parsed from the ack-reply subject; `None`
/// means it could not be parsed (the max-deliver comparison then never
/// triggers and the advisory stream is the remaining safety net).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliveredMessage {
    /// The subject the message was published to.
    pub subject: String,
    /// The `Nats-Msg-Id` header value (the idempotency key, P1.15). `None`
    /// if absent — such a message is quarantined, never processed.
    pub idempotency_key: Option<String>,
    /// The message payload (the serialized event, verbatim).
    pub payload: Vec<u8>,
    /// The delivery count (1-based, from the ack-reply subject layout).
    pub delivery_count: Option<i64>,
    /// The ack-reply subject (`None` for control replies).
    pub reply: Option<String>,
}

/// Why the handler did not commit its transaction. Any handler error is
/// retryable by redelivery until `max_deliver` is exhausted; the string is
/// the handler's own opaque diagnostic (it never reaches an API surface
/// from here).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandlerError(pub String);

impl std::fmt::Display for HandlerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "handler failed: {}", self.0)
    }
}

impl std::error::Error for HandlerError {}

/// Consumer helper failures. Message contents are opaque diagnostics for
/// operator logs — this is an infrastructure adapter, not an API surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConsumerError {
    /// A broker interaction failed (pull, ack, DLQ publish).
    Broker(String),
    /// The inbox store failed.
    Inbox(String),
}

impl std::fmt::Display for ConsumerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConsumerError::Broker(m) => write!(f, "nats broker failure: {m}"),
            ConsumerError::Inbox(m) => write!(f, "inbox store failure: {m}"),
        }
    }
}

impl std::error::Error for ConsumerError {}

/// What one processed delivery (or pull) did. These are also the counts in
/// [`ProcessReport`] — the P1.20 observability hookup reads them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ProcessOutcome {
    /// The pull window expired with no message available.
    #[default]
    Idle,
    /// The handler committed; the message was acknowledged after commit.
    Processed,
    /// The inbox already recorded the key; acknowledged without
    /// re-executing the side effect (redelivery / duplicate / replay).
    SkippedDuplicate,
    /// The handler failed below `max_deliver`; the message was NAKed for
    /// prompt redelivery.
    RetryScheduled,
    /// The message was quarantined to `FLEXCORE_DLQ` and terminated —
    /// logged at ERROR and counted, never silently dropped.
    Quarantined,
}

/// Counters accumulated by [`IdempotentConsumer::run`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ProcessReport {
    pub idle_polls: u64,
    pub processed: u64,
    pub skipped_duplicates: u64,
    pub retries_scheduled: u64,
    pub quarantined: u64,
    pub errors: u64,
}

/// Consumer helper configuration. `max_deliver` MUST equal the provisioned
/// consumer's `max_deliver` ([`crate::topology::ConsumerPolicy`]): the
/// helper quarantines on the final attempt the server would have allowed;
/// a mismatch means either the server terminates first (advisory stream is
/// the net) or this helper terminates early.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsumerConfig {
    /// The stream to consume from (a [`crate::topology::STREAMS`] name).
    pub stream: String,
    /// The durable consumer name (`<area>-<purpose>-v1`, P1.14 §5.1) — also
    /// the inbox consumer identity.
    pub durable_name: String,
    /// Redelivery budget; must match the provisioned consumer.
    pub max_deliver: i64,
    /// The pull request window (server-side `expires`). Keep well below the
    /// client request timeout so the server's expiry reply wins the race.
    pub pull_expires: Duration,
    /// NAK delay asked for on retry (server may clamp).
    pub retry_delay: Duration,
    /// Idle sleep after an empty pull window in [`IdempotentConsumer::run`].
    pub idle_interval: Duration,
    /// Base delay between cycles after a broker/inbox error.
    pub error_backoff: Duration,
}

impl Default for ConsumerConfig {
    fn default() -> Self {
        Self {
            stream: String::new(),
            durable_name: String::new(),
            max_deliver: 5,
            pull_expires: Duration::from_secs(2),
            retry_delay: Duration::from_millis(500),
            idle_interval: Duration::from_millis(200),
            error_backoff: Duration::from_secs(1),
        }
    }
}

impl ConsumerConfig {
    /// Config for a provisioned [`crate::topology::ConsumerPolicy`],
    /// carrying the policy's `max_deliver` (the must-match rule above).
    pub fn from_policy(policy: &crate::topology::ConsumerPolicy) -> Self {
        Self {
            stream: policy.stream.clone(),
            durable_name: policy.durable_name.clone(),
            max_deliver: policy.max_deliver,
            ..Default::default()
        }
    }
}

// VERIFY-BUILD: two async-nats 0.50.0 behaviours this module relies on,
// both checked against the 0.50.0 docs and to be re-confirmed mechanically
// at the first `cargo build`:
//   1. `Client::request` converts a server error-status reply (e.g. the
//      pull request's 408 expiry control message) into
//      `Err(RequestError { kind })`, with 408 mapped to
//      `RequestErrorKind::TimedOut`. If the mapping differs, adjust the
//      `TimedOut` arm in `pull_one` — the payload stays unacknowledged and
//      is redelivered, so a mis-mapping degrades to slower retries, never
//      to loss.
//   2. The ack-reply subject layout
//      `$JS.ACK.<stream>.<consumer>.<delivered>.<sseq>.<dseq>.<ts>.
//      <pending>` puts the 1-based delivery count in token 4 (0-based).
//      If the layout changed, fix `parse_delivery_count`.

/// The idempotent consumer helper (P1.16). Clones the JetStream context's
/// client for core-level pulls and acks; borrows the inbox store (the
/// durable implementation is the postgres adapter's — single-home decision
/// in src/inbox.rs). The composition root provisions the durable consumer
/// (P1.14 §5.1) and hosts [`run`](Self::run) as a background task
/// (apps/worker, P1.19).
pub struct IdempotentConsumer<'a> {
    jetstream: Context,
    client: Client,
    inbox: &'a dyn InboxStore,
    config: ConsumerConfig,
}

impl<'a> IdempotentConsumer<'a> {
    pub fn new(
        jetstream: &Context,
        inbox: &'a dyn InboxStore,
        config: ConsumerConfig,
    ) -> Self {
        Self {
            jetstream: jetstream.clone(),
            client: jetstream.client(),
            inbox,
            config,
        }
    }

    /// Pulls at most one message from the durable consumer. `Ok(None)` when
    /// the pull window expired with nothing available.
    pub async fn pull_one(&self) -> Result<Option<DeliveredMessage>, ConsumerError> {
        let subject = format!(
            "{}.{}.{}",
            MSG_NEXT_PREFIX, self.config.stream, self.config.durable_name
        );
        let expires_nanos = self.config.pull_expires.as_nanos();
        let payload = format!(r#"{{"batch":1,"expires":{expires_nanos}}}"#);
        // The pull request itself is a core request; JetStream replies with
        // either the next message or the expiry control message.
        match self.client.request(subject, payload.into_bytes().into()).await {
            Ok(message) => Ok(Some(Self::to_delivered(message))),
            Err(error) => {
                // VERIFY-BUILD: `RequestError::kind()` is the documented
                // accessor; 408 expiry replies map to `TimedOut` (checked
                // against the 0.50.0 docs, re-confirm at first build).
                if matches!(error.kind(), RequestErrorKind::TimedOut) {
                    // Server pull window expired (or client timeout — the
                    // expires value stays below the client default): idle.
                    Ok(None)
                } else {
                    Err(ConsumerError::Broker(error.to_string()))
                }
            }
        }
    }

    /// Reduces a core message to [`DeliveredMessage`].
    fn to_delivered(message: async_nats::Message) -> DeliveredMessage {
        let idempotency_key = message
            .headers
            .as_ref()
            .and_then(|h| h.get(NATS_MESSAGE_ID))
            .map(|value| value.as_str().to_string());
        let reply = message.reply.as_ref().map(|s| s.to_string());
        let delivery_count = message
            .reply
            .as_ref()
            .and_then(|reply| parse_delivery_count(reply.as_str()));
        DeliveredMessage {
            subject: message.subject.to_string(),
            idempotency_key,
            payload: message.payload.to_vec(),
            delivery_count,
            reply,
        }
    }

    /// The P1.16 flow for one delivered message (see the module header).
    /// Public so integration tests and ops tooling can drive a message
    /// through the exact production flow (including the redelivery paths)
    /// without depending on broker timing.
    pub async fn process_delivered<H>(
        &self,
        message: DeliveredMessage,
        handler: &mut H,
    ) -> Result<ProcessOutcome, ConsumerError>
    where
        H: FnMut(&DeliveredMessage, &dyn InboxStore) -> Result<(), HandlerError>,
    {
        // A message without an idempotency key cannot be made idempotent;
        // quarantine on first sight (poison rule: never silently drop).
        let Some(idempotency_key) = message.idempotency_key.clone() else {
            return self.quarantine(&message, None, "no Nats-Msg-Id header").await;
        };

        // Step 1 — check inbox: a recorded key means the side effect
        // already committed; acknowledge without re-executing.
        match self.inbox.is_recorded(&self.config.durable_name, &idempotency_key) {
            Ok(true) => {
                self.ack(&message, ACK_PAYLOAD).await?;
                return Ok(ProcessOutcome::SkippedDuplicate);
            }
            Ok(false) => {}
            Err(e) => return Err(ConsumerError::Inbox(e.to_string())),
        }

        // Step 2 — process: the handler applies its state change and
        // records the inbox entry inside its one transaction. (The in-memory
        // double records on `record`; the durable store enforces the unique
        // constraint — a concurrent duplicate gets `AlreadyRecorded` and
        // must roll back inside the handler.)
        match handler(&message, self.inbox) {
            Ok(()) => {
                // The handler has committed its one transaction: state
                // change + inbox record (the binding obligation on
                // `InboxStore::record`). The helper deliberately does NOT
                // record again: an inbox row written outside the handler's
                // transaction would silently weaken the one-transaction
                // rule. A concurrent duplicate wins or loses on the
                // store's unique constraint inside the handler's own
                // transaction (handler rolls back on `AlreadyRecorded`).
                // Step 3 — acknowledge strictly after the handler returned
                // (i.e. after its commit). A crash before this ack is safe:
                // the redelivery hits the inbox check above.
                self.ack(&message, ACK_PAYLOAD).await?;
                Ok(ProcessOutcome::Processed)
            }
            Err(handler_error) => {
                let attempted = message.delivery_count.unwrap_or(0);
                if attempted >= self.config.max_deliver {
                    // Poison: final failed attempt → DLQ + TERM + alert.
                    self.quarantine(
                        &message,
                        Some(&idempotency_key),
                        &handler_error.to_string(),
                    )
                    .await
                } else {
                    // Retryable: NAK with a short delay; the message is
                    // redelivered and the inbox check governs idempotency.
                    self.nak(&message).await?;
                    Ok(ProcessOutcome::RetryScheduled)
                }
            }
        }
    }

    /// Publishes `+ACK` to the message's reply subject.
    async fn ack(&self, message: &DeliveredMessage, payload: &[u8]) -> Result<(), ConsumerError> {
        let Some(reply) = message.reply.clone() else {
            // No reply subject → nothing to ack; JetStream will redeliver
            // after AckWait. Surface loudly rather than pretend success.
            return Err(ConsumerError::Broker(format!(
                "message on {} has no ack reply subject",
                message.subject
            )));
        };
        self.client
            .publish(reply, payload.to_vec().into())
            .await
            .map_err(|e| ConsumerError::Broker(e.to_string()))
    }

    /// Publishes `-NAK` with the configured delay (prompt redelivery).
    async fn nak(&self, message: &DeliveredMessage) -> Result<(), ConsumerError> {
        let delay_nanos = self.config.retry_delay.as_nanos();
        let payload = format!(r#"-NAK {{"delay":{delay_nanos}}}"#).into_bytes();
        self.ack(message, &payload).await
    }

    /// The poison path: original message + failure metadata →
    /// `flexcore.dlq.v1.<area>.<event_slug>` (captured by `FLEXCORE_DLQ`),
    /// then `+TERM` the original delivery, then log at ERROR. Returns
    /// [`ProcessOutcome::Quarantined`].
    ///
    /// The DLQ copy carries the original `Nats-Msg-Id`, so the per-stream
    /// duplicate window collapses repeated quarantine attempts of the same
    /// message. If the original subject does not conform to the P1.14
    /// scheme (no derivable DLQ subject), the copy goes to the explicit
    /// catch-all `flexcore.dlq.v1.unrouted` with the original subject in
    /// the metadata — an error, never a silent drop.
    async fn quarantine(
        &self,
        message: &DeliveredMessage,
        idempotency_key: Option<&str>,
        last_error: &str,
    ) -> Result<ProcessOutcome, ConsumerError> {
        let quarantined_at_millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
            .unwrap_or_default();
        let metadata = serde_json::json!({
            "consumer": self.config.durable_name,
            "stream": self.config.stream,
            "original_subject": message.subject,
            "idempotency_key": idempotency_key,
            "delivery_count": message.delivery_count,
            "last_error": last_error,
            "quarantined_at_unix_millis": quarantined_at_millis,
            // The serialized event, verbatim (lossy UTF-8 — events are
            // JSON text; a non-UTF-8 payload is itself poison evidence).
            "payload": String::from_utf8_lossy(&message.payload),
        });
        let dlq_subject = dlq_subject_for(&message.subject)
            .unwrap_or_else(|| format!("{DLQ_SUBJECT_PREFIX}.unrouted"));
        if !message.subject.starts_with("flexcore.v1.") {
            tracing::error!(
                original_subject = %message.subject,
                "DLQ copy for a non-conforming subject routed to the catch-all"
            );
        }

        let mut headers = HeaderMap::new();
        if let Some(key) = idempotency_key {
            headers.insert(NATS_MESSAGE_ID, key.to_string());
        }
        self.jetstream
            .publish_with_headers(dlq_subject, headers, metadata.to_string().into_bytes().into())
            .await
            .map_err(|e| ConsumerError::Broker(format!("DLQ publish failed: {e}")))?
            .await
            .map_err(|e| {
                ConsumerError::Broker(format!("DLQ publish not acknowledged: {e}"))
            })?;

        // Terminate the original delivery only after the DLQ copy is
        // acknowledged — otherwise the poison message could be lost.
        self.ack(message, TERM_PAYLOAD).await?;

        // Alert: ERROR log now; the metric counter lands with P1.20
        // (ProcessReport.quarantined is the counter source).
        tracing::error!(
            consumer = %self.config.durable_name,
            stream = %self.config.stream,
            original_subject = %message.subject,
            idempotency_key = idempotency_key.unwrap_or("<none>"),
            delivery_count = message.delivery_count,
            last_error = %last_error,
            "POISON MESSAGE quarantined to FLEXCORE_DLQ (never silently dropped)"
        );
        Ok(ProcessOutcome::Quarantined)
    }

    /// One pull + process step (the unit [`run`] loops).
    pub async fn process_next<H>(
        &self,
        handler: &mut H,
    ) -> Result<ProcessOutcome, ConsumerError>
    where
        H: FnMut(&DeliveredMessage, &dyn InboxStore) -> Result<(), HandlerError>,
    {
        match self.pull_one().await? {
            None => Ok(ProcessOutcome::Idle),
            Some(message) => self.process_delivered(message, handler).await,
        }
    }

    /// The consumption loop until the shutdown watch fires (apps/worker,
    /// P1.19, owns the channel). Broker or inbox errors are logged and
    /// backed off — the message stays unacknowledged and JetStream
    /// redelivers, so no error path here can lose or duplicate a side
    /// effect. Quarantine events are surfaced in the returned report.
    pub async fn run<H>(
        &self,
        handler: &mut H,
        mut shutdown: watch::Receiver<bool>,
    ) -> ProcessReport
    where
        H: FnMut(&DeliveredMessage, &dyn InboxStore) -> Result<(), HandlerError>,
    {
        let mut report = ProcessReport::default();
        loop {
            if *shutdown.borrow() {
                return report;
            }
            match self.process_next(handler).await {
                Ok(ProcessOutcome::Idle) => {
                    report.idle_polls += 1;
                    if self
                        .sleep_or_shutdown(self.config.idle_interval, &mut shutdown)
                        .await
                        .is_some()
                    {
                        return report;
                    }
                }
                Ok(outcome) => match outcome {
                    ProcessOutcome::Processed => report.processed += 1,
                    ProcessOutcome::SkippedDuplicate => report.skipped_duplicates += 1,
                    ProcessOutcome::RetryScheduled => report.retries_scheduled += 1,
                    ProcessOutcome::Quarantined => report.quarantined += 1,
                    ProcessOutcome::Idle => unreachable!("Idle handled above"),
                },
                Err(error) => {
                    report.errors += 1;
                    tracing::error!(
                        consumer = %self.config.durable_name,
                        error = %error,
                        "consumer cycle failed; the message stays unacknowledged \
                         and will be redelivered by JetStream (at-least-once)"
                    );
                    if self
                        .sleep_or_shutdown(self.config.error_backoff, &mut shutdown)
                        .await
                        .is_some()
                    {
                        return report;
                    }
                }
            }
        }
    }

    async fn sleep_or_shutdown(
        &self,
        duration: Duration,
        shutdown: &mut watch::Receiver<bool>,
    ) -> Option<()> {
        tokio::select! {
            () = tokio::time::sleep(duration) => None,
            changed = shutdown.changed() => {
                let _ = changed;
                Some(())
            }
        }
    }
}

/// Parses the delivery count (1-based) from a `$JS.ACK.…` reply subject.
/// VERIFY-BUILD: token layout per the module-header note.
fn parse_delivery_count(reply: &str) -> Option<i64> {
    let tokens: Vec<&str> = reply.split('.').collect();
    if tokens.len() < 5 || tokens[0] != "$JS" || tokens[1] != "ACK" {
        return None;
    }
    tokens[4].parse::<i64>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delivery_count_parses_from_ack_reply_layout() {
        let reply = "$JS.ACK.FLEXCORE_REGISTRY.registry-test-v1.3.42.7.1730000000000.0";
        assert_eq!(parse_delivery_count(reply), Some(3));
        assert_eq!(parse_delivery_count("$JS.ACK.S.C.1.1.1.1.1"), Some(1));
        assert_eq!(parse_delivery_count("_INBOX.abcd"), None);
        assert_eq!(parse_delivery_count("$JS.ACK.S.C.notanumber.1.1.1.1"), None);
        assert_eq!(parse_delivery_count("$NOPE.ACK.S.C.1.1.1.1.1"), None);
    }
}
