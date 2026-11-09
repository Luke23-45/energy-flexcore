// P1.09 — Transactional outbox record (report §6.3, §6.4, §15.5; P0.14 §4).
//
// Part of the flexcore-events P1.09 deliverable. The outbox record is the
// type the application writes **in the same PostgreSQL transaction** as the
// state change (report §6.3 step 1; the write happens via the storage layer
// and the P1.13 adapter, never here — this crate imports no database or
// broker). The publisher (P1.15) reads unsent entries, publishes to NATS
// JetStream, and marks the entry delivered **only after broker
// acknowledgement**; consumers must be idempotent because durable messaging
// is normally at-least-once, not a magical exactly-once execution
// guarantee [R6].
//
// Rules encoded here:
//
// - The record carries the **serialized payload** (produced by the caller —
//   this crate imports no JSON library at runtime; serde_json is a
//   dev-dependency only), the **destination subject**, and the
//   **idempotency key**.
// - The idempotency key is **unique** in the outbox store (enforced by the
//   P1.12 schema's unique constraint); the record type carries it
//   mandatorily via the validated [`IdempotencyKey`] type.
// - Delivery state is exactly one of Pending/Delivered/Failed; the attempt
//   count increments per publish attempt; `updated_at` never precedes
//   `created_at`; timestamps require the UTC offset at construction
//   (contracts time rule, P0.09).

use crate::envelope::IdempotencyKey;
use crate::subject::Subject;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

/// Delivery state of an outbox record (report §6.3 steps 2–3).
///
/// `Pending` — not yet published, or published without acknowledgement
/// (republish on the next drain; the broker deduplicates by idempotency key
/// within the duplicate window, and consumers deduplicate durably).
/// `Delivered` — published and acknowledged by the broker.
/// `Failed` — exceeded the publisher's retry policy; routed for
/// investigation, never silently dropped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DeliveryState {
    /// Not yet published, or awaiting broker acknowledgement.
    Pending,
    /// Published; broker acknowledgement received.
    Delivered,
    /// Publication failed after the configured retries.
    Failed,
}

/// Errors returned by outbox-record construction and transitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutboxError {
    /// A timestamp was supplied with a non-UTC offset (contracts time rule,
    /// P0.09: UTC required, never silently converted).
    NonUtcTimestamp,
    /// The update timestamp precedes the creation timestamp.
    UpdatedBeforeCreated,
    /// The serialized payload was empty.
    EmptyPayload,
}

impl std::fmt::Display for OutboxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OutboxError::NonUtcTimestamp => {
                write!(f, "outbox timestamps must carry the UTC offset (zero offset)")
            }
            OutboxError::UpdatedBeforeCreated => {
                write!(f, "updated_at must not precede created_at")
            }
            OutboxError::EmptyPayload => {
                write!(f, "the serialized event payload must not be empty")
            }
        }
    }
}

impl std::error::Error for OutboxError {}

fn require_utc(t: OffsetDateTime) -> Result<OffsetDateTime, OutboxError> {
    if t.offset().is_utc() {
        Ok(t)
    } else {
        Err(OutboxError::NonUtcTimestamp)
    }
}

/// One transactional-outbox record (report §6.3 four-step sequence; §15.5).
///
/// Written in the same PostgreSQL transaction as the state change (step 1);
/// published to NATS JetStream (step 2); marked delivered after broker
/// acknowledgement (step 3). Idempotency-key uniqueness is enforced by the
/// outbox store's unique constraint (P1.12 design), which makes republishing
/// safe after a crash between publish and mark (step 4's consumer
/// idempotency completes the chain).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "OutboxWire")]
pub struct OutboxRecord {
    idempotency_key: IdempotencyKey,
    destination_subject: Subject,
    payload: String,
    delivery_state: DeliveryState,
    attempt_count: u32,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
}

impl OutboxRecord {
    /// Creates a new outbox record in the `Pending` state with zero
    /// attempts. The payload is the **serialized** event (JSON text
    /// produced by the caller via serde_json); the destination subject
    /// comes from the subject scheme (P1.14 decides the final scheme).
    pub fn new(
        idempotency_key: IdempotencyKey,
        destination_subject: Subject,
        payload: String,
        created_at: OffsetDateTime,
    ) -> Result<Self, OutboxError> {
        require_utc(created_at)?;
        if payload.is_empty() {
            return Err(OutboxError::EmptyPayload);
        }
        Ok(Self {
            idempotency_key,
            destination_subject,
            payload,
            delivery_state: DeliveryState::Pending,
            attempt_count: 0,
            created_at,
            updated_at: created_at,
        })
    }

    /// Records one publish attempt: increments the attempt count and moves
    /// `updated_at` forward. Leaves the delivery state unchanged — marking
    /// delivered happens only after broker acknowledgement.
    pub fn record_attempt(self, at: OffsetDateTime) -> Result<Self, OutboxError> {
        let mut updated = self;
        require_utc(at)?;
        updated.bump_updated_at(at)?;
        updated.attempt_count += 1;
        Ok(updated)
    }

    /// Marks the record delivered after broker acknowledgement (report §6.3
    /// step 3).
    pub fn mark_delivered(self, at: OffsetDateTime) -> Result<Self, OutboxError> {
        let mut updated = self;
        require_utc(at)?;
        updated.bump_updated_at(at)?;
        updated.delivery_state = DeliveryState::Delivered;
        Ok(updated)
    }

    /// Marks the record failed after exhausting the publisher's retry
    /// policy. The entry stays in the outbox for investigation — it is
    /// never silently dropped.
    pub fn mark_failed(self, at: OffsetDateTime) -> Result<Self, OutboxError> {
        let mut updated = self;
        require_utc(at)?;
        updated.bump_updated_at(at)?;
        updated.delivery_state = DeliveryState::Failed;
        Ok(updated)
    }

    fn bump_updated_at(&mut self, at: OffsetDateTime) -> Result<(), OutboxError> {
        if at < self.created_at {
            return Err(OutboxError::UpdatedBeforeCreated);
        }
        self.updated_at = at;
        Ok(())
    }

    /// The mandatory idempotency key (unique in the outbox store).
    pub fn idempotency_key(&self) -> &IdempotencyKey {
        &self.idempotency_key
    }

    /// The destination subject for publication.
    pub fn destination_subject(&self) -> &Subject {
        &self.destination_subject
    }

    /// The serialized event payload.
    pub fn payload(&self) -> &str {
        &self.payload
    }

    /// The current delivery state.
    pub fn delivery_state(&self) -> DeliveryState {
        self.delivery_state
    }

    /// The number of publish attempts so far.
    pub fn attempt_count(&self) -> u32 {
        self.attempt_count
    }

    /// Creation timestamp (UTC).
    pub fn created_at(&self) -> OffsetDateTime {
        self.created_at
    }

    /// Last-update timestamp (UTC).
    pub fn updated_at(&self) -> OffsetDateTime {
        self.updated_at
    }
}

/// The deserialization target for [`OutboxRecord`]; every invariant is
/// re-checked in the `TryFrom` conversion, so a deserialized record always
/// satisfies the construction rules.
#[derive(Deserialize)]
struct OutboxWire {
    idempotency_key: IdempotencyKey,
    destination_subject: Subject,
    payload: String,
    delivery_state: DeliveryState,
    attempt_count: u32,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
}

impl TryFrom<OutboxWire> for OutboxRecord {
    type Error = OutboxError;

    fn try_from(wire: OutboxWire) -> Result<Self, Self::Error> {
        let record = Self::new(
            wire.idempotency_key,
            wire.destination_subject,
            wire.payload,
            wire.created_at,
        )?;
        require_utc(wire.updated_at)?;
        if wire.updated_at < record.created_at {
            return Err(OutboxError::UpdatedBeforeCreated);
        }
        Ok(Self {
            idempotency_key: record.idempotency_key,
            destination_subject: record.destination_subject,
            payload: record.payload,
            delivery_state: wire.delivery_state,
            attempt_count: wire.attempt_count,
            created_at: record.created_at,
            updated_at: wire.updated_at,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::envelope::EventEnvelope;
    use crate::events::OperatorActionRecordedPayload;
    use crate::subject::subject_for;
    use flexcore_contracts::ids::FlexUuid;
    use flexcore_contracts::time_model::EventTime;
    use flexcore_domain::ids::TenantId;

    fn utc_millis(millis: i64) -> OffsetDateTime {
        OffsetDateTime::from_unix_timestamp_nanos(i128::from(millis) * 1_000_000)
            .expect("valid UTC timestamp")
    }

    fn sample_envelope() -> EventEnvelope {
        EventEnvelope::new(
            crate::envelope::SUPPORTED_SCHEMA_VERSION,
            "operator-action-1",
            DomainEvent::OperatorActionRecorded(OperatorActionRecordedPayload {
                correlation_id: FlexUuid::nil(),
                tenant_id: TenantId::new(FlexUuid::nil()),
                event_time: EventTime::from_unix_millis(1_700_000_000_000)
                    .expect("valid UTC timestamp"),
            }),
        )
        .unwrap()
    }

    use crate::events::DomainEvent;

    fn sample_record() -> OutboxRecord {
        let envelope = sample_envelope();
        OutboxRecord::new(
            envelope.idempotency_key.clone(),
            subject_for(&envelope.event),
            "{\"serialized\": true}".to_string(),
            utc_millis(1_000),
        )
        .unwrap()
    }

    #[test]
    fn new_record_starts_pending_with_zero_attempts() {
        let record = sample_record();
        assert_eq!(record.delivery_state(), DeliveryState::Pending);
        assert_eq!(record.attempt_count(), 0);
        assert_eq!(record.created_at(), record.updated_at());
        assert_eq!(record.idempotency_key().as_str(), "operator-action-1");
        assert_eq!(record.payload(), "{\"serialized\": true}");
    }

    #[test]
    fn record_attempts_increment_and_marking_follows_the_ack_rule() {
        let record = sample_record().record_attempt(utc_millis(2_000)).unwrap();
        assert_eq!(record.attempt_count(), 1);
        assert_eq!(record.delivery_state(), DeliveryState::Pending);

        // Delivered only after broker acknowledgement, by explicit mark.
        let delivered = record.mark_delivered(utc_millis(3_000)).unwrap();
        assert_eq!(delivered.delivery_state(), DeliveryState::Delivered);
        assert_eq!(delivered.attempt_count(), 1);
        assert!(delivered.updated_at() > delivered.created_at());
    }

    #[test]
    fn failed_records_stay_in_the_outbox_state_machine() {
        let record = sample_record().record_attempt(utc_millis(2_000)).unwrap();
        let failed = record.mark_failed(utc_millis(3_000)).unwrap();
        assert_eq!(failed.delivery_state(), DeliveryState::Failed);
        // A failed record can be retried by the publisher (back to attempts,
        // not deletion): record another attempt.
        let retried = failed.record_attempt(utc_millis(4_000)).unwrap();
        assert_eq!(retried.attempt_count(), 2);
    }

    #[test]
    fn construction_rejects_bad_input() {
        let envelope = sample_envelope();
        assert_eq!(
            OutboxRecord::new(
                envelope.idempotency_key.clone(),
                subject_for(&envelope.event),
                String::new(),
                utc_millis(1_000)
            ),
            Err(OutboxError::EmptyPayload)
        );
        assert_eq!(
            OutboxRecord::new(
                envelope.idempotency_key.clone(),
                subject_for(&envelope.event),
                "x".to_string(),
                utc_millis(1_000).replace_offset(time::UtcOffset::from_hms(-5, 0, 0).unwrap())
            ),
            Err(OutboxError::NonUtcTimestamp)
        );
        let record = sample_record();
        assert_eq!(
            record.record_attempt(utc_millis(500)),
            Err(OutboxError::UpdatedBeforeCreated)
        );
    }

    #[test]
    fn record_round_trips_through_serde_json() {
        let record = sample_record()
            .record_attempt(utc_millis(2_000))
            .unwrap()
            .mark_delivered(utc_millis(3_000))
            .unwrap();
        let json = serde_json::to_string(&record).unwrap();
        let back: OutboxRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(back, record);
        assert_eq!(back.delivery_state(), DeliveryState::Delivered);
        assert_eq!(back.attempt_count(), 1);
    }

    #[test]
    fn deserialization_revalidates_every_invariant() {
        let record = sample_record();
        let json = serde_json::to_value(&record).unwrap();

        // Non-UTC created_at never deserializes.
        let mut bad = json.clone();
        bad["created_at"] = serde_json::json!("2026-01-01T12:00:00+02:00");
        assert!(serde_json::from_value::<OutboxRecord>(bad).is_err());

        // updated_at before created_at never deserializes.
        let mut bad = json;
        bad["created_at"] = serde_json::json!("1970-01-01T00:01:00Z");
        bad["updated_at"] = serde_json::json!("1970-01-01T00:00:30Z");
        assert!(serde_json::from_value::<OutboxRecord>(bad).is_err());
    }

    #[test]
    fn delivery_state_serde_uses_the_frozen_names() {
        for state in [
            DeliveryState::Pending,
            DeliveryState::Delivered,
            DeliveryState::Failed,
        ] {
            let back: DeliveryState =
                serde_json::from_value(serde_json::to_value(&state).unwrap()).unwrap();
            assert_eq!(back, state);
        }
        assert!(serde_json::from_value::<DeliveryState>(serde_json::json!("UNKNOWN")).is_err());
    }
}
