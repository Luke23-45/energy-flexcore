// P1.08 — Shared outbox-write assembly (private module).
//
// One helper, one rule (report §6.3 step 1, §15.5): every Stage 1 use case
// records its state change and its domain event as **one** transactional
// outbox write. The mechanics, in order:
//
// 1. the repository write returned a `StateChangeReceipt` (the state-change
//    half — the receipt's existence proves the write was issued first);
// 2. the use case assembled the typed `DomainEvent` (frozen §15.3
//    vocabulary — the correct event per action, never an invented one);
// 3. this helper builds the envelope (P1.09: schema version + mandatory
//    idempotency key, derived deterministically from the frozen event name
//    plus the correlation identifier), obtains the destination subject from
//    the events crate's conservative default scheme (ASSUMPTION pending
//    P1.14), has the payload rendered by the injected encoder, and hands
//    the complete `OutboxWrite` to the `EventOutbox` port.
//
// What this helper cannot do is commit: the unit-of-work boundary belongs
// to the composition root (one unit of work per use-case invocation; the
// adapter executes the write halves atomically inside its native
// transaction — the P1.13 atomicity obligation). The fakes journal proves
// the ordering in tests: state change, then outbox append, then the single
// commit marker.

use crate::error::{ApplicationError, ValidationError};
use crate::payload_encoding::EventPayloadEncoder;
use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::time_model::EventTime;
use flexcore_events::envelope::{SUPPORTED_SCHEMA_VERSION, EventEnvelope};
use flexcore_events::events::{DomainEvent, event_name};
use flexcore_events::subject::subject_for;
use flexcore_ports::event_outbox::{EventOutbox, OutboxWrite};
use flexcore_ports::state_change::StateChangeReceipt;

/// Renders a PascalCase frozen event name as lower snake_case
/// (`AssetRegistered` -> `asset_registered`) for the idempotency key.
/// (The events crate has the same helper privately; duplicating three
/// lines here beats exporting a private helper across a crate boundary.)
fn snake_case(name: &str) -> String {
    let mut out = String::with_capacity(name.len() + 8);
    for (i, c) in name.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// The deterministic idempotency key for one event instance:
/// `<snake-case frozen event name>:<correlation uuid>`. Unique per event
/// instance (the correlation identifier is generated per use-case
/// invocation); the outbox store enforces uniqueness durably (P1.12 §4).
pub(crate) fn idempotency_key(event: &DomainEvent, correlation_id: FlexUuid) -> String {
    format!("{}:{}", snake_case(event_name(event)), correlation_id)
}

/// Assembles and appends the one transactional outbox write for a state
/// change and its event (report §6.3 step 1). See the module header.
pub(crate) fn emit(
    state_change: StateChangeReceipt,
    event: DomainEvent,
    correlation_id: FlexUuid,
    created_at: EventTime,
    encoder: &dyn EventPayloadEncoder,
    outbox: &dyn EventOutbox,
) -> Result<(), ApplicationError> {
    let idempotency_key = idempotency_key(&event, correlation_id);
    let envelope =
        EventEnvelope::new(SUPPORTED_SCHEMA_VERSION, &idempotency_key, event)
            .map_err(ValidationError::from)?;
    let payload = encoder.encode(&envelope)?;
    let destination_subject = subject_for(&envelope.event).to_string();
    // `?` performs a single From step, so the two-step shape errors
    // (OutboxWriteError -> ValidationError -> ApplicationError) go through
    // map_err explicitly here.
    let write = OutboxWrite::new(
        state_change,
        idempotency_key,
        destination_subject,
        payload,
        created_at,
    )
    .map_err(ValidationError::from)?;
    outbox.append(write)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use flexcore_domain::ids::TenantId;
    use flexcore_events::events::OperatorActionRecordedPayload;

    fn sample_event(correlation: FlexUuid) -> DomainEvent {
        DomainEvent::OperatorActionRecorded(OperatorActionRecordedPayload {
            correlation_id: correlation,
            tenant_id: TenantId::new(correlation),
            event_time: EventTime::from_unix_millis(1_000).expect("valid timestamp"),
        })
    }

    #[test]
    fn the_idempotency_key_is_the_snake_case_event_name_plus_correlation() {
        let correlation = FlexUuid::from_bytes(7u128.to_be_bytes());
        let key = idempotency_key(&sample_event(correlation), correlation);
        assert_eq!(
            key,
            format!("operator_action_recorded:{}", correlation),
            "the key derives from the frozen event name and the correlation id"
        );
    }
}
