// P1.13 — Outbox row mapping: envelope classification and outbox SQL.
//
// Recorded mappings (crate README):
// - `event_type` / `schema_version` columns: the port's `OutboxWrite`
//   carries only the serialized envelope payload (P1.09), so the adapter
//   classifies the two columns structurally from the payload JSON —
//   `schema_version` from the envelope field, `event_type` from the
//   internally-tagged event discriminant (`"event_type"`, SCREAMING_SNAKE,
//   per the flexcore-events serialization rule). A payload that does not
//   name both is refused (`E_OUTBOX_PAYLOAD`): the P1.12 columns are NOT
//   NULL and are never guessed.
// - `payload` is stored verbatim as jsonb (the serialized envelope).
// - `created_at` comes from the port's `EventTime`; `updated_at` starts
//   equal; the wrapped event's own `event_time` stays inside the payload
//   (P1.12 §2.2: the P0.09 distinction is never silently lost).

use crate::error::{codes, decode_failure, map_backend, map_outbox_error};
use flexcore_contracts::time_model::EventTime;
use flexcore_ports::backend_error::BackendError;
use flexcore_ports::event_outbox::{EventOutboxError, PendingOutboxEntry};
use serde_json::Value;
use sqlx::Postgres;
use time::OffsetDateTime;

/// Derives `(event_type, schema_version)` from the serialized envelope
/// payload (see the module header for the recorded mapping).
pub(crate) fn outbox_classifiers(payload: &str) -> Result<(String, String), BackendError> {
    let value: Value = serde_json::from_str(payload).map_err(|_| {
        BackendError::new(
            codes::OUTBOX_PAYLOAD.to_string(),
            "the outbox payload is not valid JSON (the envelope is serialized by the caller per P1.09)".to_string(),
        )
    })?;
    let event_type = value
        .get("event")
        .and_then(|event| event.get("event_type"))
        .and_then(Value::as_str)
        .ok_or_else(|| {
            BackendError::new(
                codes::OUTBOX_PAYLOAD.to_string(),
                "the outbox payload does not name its event type; the envelope must carry the internally tagged event".to_string(),
            )
        })?
        .to_string();
    let schema_version = match value.get("schema_version") {
        Some(Value::Number(n)) => n.to_string(),
        Some(Value::String(s)) => s.clone(),
        Some(other) => other.to_string(),
        None => {
            return Err(BackendError::new(
                codes::OUTBOX_PAYLOAD.to_string(),
                "the outbox payload does not carry a schema version; a breaking change requires a new version, never a silent default".to_string(),
            ))
        }
    };
    Ok((event_type, schema_version))
}

/// Parses the payload into the jsonb value stored verbatim.
pub(crate) fn outbox_payload_json(payload: &str) -> Result<Value, EventOutboxError> {
    serde_json::from_str(payload).map_err(|_| {
        EventOutboxError::Backend(BackendError::new(
            codes::OUTBOX_PAYLOAD.to_string(),
            "the outbox payload is not valid JSON (the envelope is serialized by the caller per P1.09)".to_string(),
        ))
    })
}

/// Inserts one outbox row inside the caller's transaction. A uniqueness
/// violation on `idempotency_key` maps to the durable
/// [`EventOutboxError::DuplicateKey`] (P1.12 §4.6 unique constraint).
pub(crate) async fn insert_outbox_row<'e, E>(
    executor: E,
    idempotency_key: &str,
    event_type: &str,
    schema_version: &str,
    payload: Value,
    destination_subject: &str,
    created_at: OffsetDateTime,
) -> Result<(), EventOutboxError>
where
    E: sqlx::Executor<'e, Database = Postgres>,
{
    sqlx::query(
        "INSERT INTO outbox \
             (idempotency_key, event_type, schema_version, payload, destination_subject, \
              delivery_state, attempt_count, created_at, updated_at) \
         VALUES ($1, $2, $3, $4, $5, 'pending', 0, $6, $6)",
    )
    .bind(idempotency_key)
    .bind(event_type)
    .bind(schema_version)
    .bind(payload)
    .bind(destination_subject)
    .bind(created_at)
    .execute(executor)
    .await
    .map_err(|err| map_outbox_error(err, idempotency_key))?;
    Ok(())
}

/// Reads up to `limit` pending entries in insertion order (created_at,
/// outbox_id — uuidv7 keeps outbox_id locality-ordered, D-S1; ordering is
/// never by id alone, P1.12 §2.3).
pub(crate) async fn pending_entries<'e, E>(
    executor: E,
    limit: u32,
) -> Result<Vec<PendingOutboxEntry>, EventOutboxError>
where
    E: sqlx::Executor<'e, Database = Postgres>,
{
    let rows = sqlx::query(
        "SELECT idempotency_key, destination_subject, payload::text AS payload, \
                attempt_count, created_at \
         FROM outbox \
         WHERE delivery_state = 'pending' \
         ORDER BY created_at, outbox_id \
         LIMIT $1",
    )
    .bind(i64::from(limit))
    .fetch_all(executor)
    .await
    .map_err(map_backend)
    .map_err(EventOutboxError::Backend)?;

    let mut entries = Vec::with_capacity(rows.len());
    for row in &rows {
        let idempotency_key: String = row
            .try_get("idempotency_key")
            .map_err(|_| EventOutboxError::Backend(decode_failure("outbox.idempotency_key")))?;
        let destination_subject: String = row
            .try_get("destination_subject")
            .map_err(|_| EventOutboxError::Backend(decode_failure("outbox.destination_subject")))?;
        let payload: String = row
            .try_get("payload")
            .map_err(|_| EventOutboxError::Backend(decode_failure("outbox.payload")))?;
        // attempt_count carries a CHECK (attempt_count >= 0); the
        // database cannot hand back a negative value.
        let attempts: i32 = row
            .try_get("attempt_count")
            .map_err(|_| EventOutboxError::Backend(decode_failure("outbox.attempt_count")))?;
        let created: OffsetDateTime = row
            .try_get("created_at")
            .map_err(|_| EventOutboxError::Backend(decode_failure("outbox.created_at")))?;
        let created_at = EventTime::new(created)
            .map_err(|_| EventOutboxError::Backend(decode_failure("outbox.created_at")))?;
        entries.push(PendingOutboxEntry::new(
            idempotency_key,
            destination_subject,
            payload,
            u32::try_from(i64::from(attempts)).unwrap_or(0),
            created_at,
        ));
    }
    Ok(entries)
}

/// Records one publish attempt: attempt_count + 1, delivery state unchanged.
pub(crate) async fn record_attempt<'e, E>(
    executor: E,
    idempotency_key: &str,
) -> Result<(), EventOutboxError>
where
    E: sqlx::Executor<'e, Database = Postgres>,
{
    let result = sqlx::query(
        "UPDATE outbox SET attempt_count = attempt_count + 1, updated_at = now() \
         WHERE idempotency_key = $1",
    )
    .bind(idempotency_key)
    .execute(executor)
    .await
    .map_err(|err| map_outbox_error(err, idempotency_key))?;
    if result.rows_affected() == 0 {
        return Err(EventOutboxError::UnknownKey {
            idempotency_key: idempotency_key.to_string(),
        });
    }
    Ok(())
}

/// Marks the entry delivered — only after broker acknowledgement (report
/// §6.3 step 3); sets `delivered_at` once.
pub(crate) async fn mark_delivered<'e, E>(
    executor: E,
    idempotency_key: &str,
) -> Result<(), EventOutboxError>
where
    E: sqlx::Executor<'e, Database = Postgres>,
{
    let result = sqlx::query(
        "UPDATE outbox SET delivery_state = 'delivered', delivered_at = now(), updated_at = now() \
         WHERE idempotency_key = $1",
    )
    .bind(idempotency_key)
    .execute(executor)
    .await
    .map_err(|err| map_outbox_error(err, idempotency_key))?;
    if result.rows_affected() == 0 {
        return Err(EventOutboxError::UnknownKey {
            idempotency_key: idempotency_key.to_string(),
        });
    }
    Ok(())
}

/// Marks the entry failed after the retry policy was exhausted; the entry
/// stays in the store for investigation — never silently dropped.
pub(crate) async fn mark_failed<'e, E>(
    executor: E,
    idempotency_key: &str,
) -> Result<(), EventOutboxError>
where
    E: sqlx::Executor<'e, Database = Postgres>,
{
    let result = sqlx::query(
        "UPDATE outbox SET delivery_state = 'failed', updated_at = now() \
         WHERE idempotency_key = $1",
    )
    .bind(idempotency_key)
    .execute(executor)
    .await
    .map_err(|err| map_outbox_error(err, idempotency_key))?;
    if result.rows_affected() == 0 {
        return Err(EventOutboxError::UnknownKey {
            idempotency_key: idempotency_key.to_string(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const ENVELOPE: &str = r#"{
        "schema_version": 1,
        "idempotency_key": "tenant-a:asset-1:seq-7",
        "event": {
            "event_type": "ASSET_REGISTERED",
            "correlation_id": "00000000-0000-0000-0000-000000000000"
        }
    }"#;

    #[test]
    fn classifiers_extract_the_envelope_fields() {
        let (event_type, schema_version) =
            outbox_classifiers(ENVELOPE).expect("valid envelope payload");
        assert_eq!(event_type, "ASSET_REGISTERED");
        assert_eq!(schema_version, "1");
    }

    #[test]
    fn a_payload_without_the_event_tag_is_refused() {
        let err = outbox_classifiers(r#"{"schema_version": 1}"#).unwrap_err();
        assert_eq!(err.code(), codes::OUTBOX_PAYLOAD);

        let err = outbox_classifiers(
            r#"{"schema_version": 1, "event": {"name": "no-tag"}}"#,
        )
        .unwrap_err();
        assert_eq!(err.code(), codes::OUTBOX_PAYLOAD);
    }

    #[test]
    fn a_payload_without_a_schema_version_is_refused() {
        // No silent default: a payload without a version never classifies.
        let err = outbox_classifiers(r#"{"event": {"event_type": "ASSET_REGISTERED"}}"#)
            .unwrap_err();
        assert_eq!(err.code(), codes::OUTBOX_PAYLOAD);
    }

    #[test]
    fn a_non_json_payload_is_refused() {
        let err = outbox_classifiers("not json").unwrap_err();
        assert_eq!(err.code(), codes::OUTBOX_PAYLOAD);
        assert!(outbox_payload_json("not json").is_err());
    }

    #[test]
    fn the_store_level_append_guard_is_explicit() {
        // The publisher-side EventOutbox on FlexCoreStore refuses `append`:
        // the transactional write must execute inside the caller's unit of
        // work (P1.07 EventOutbox port obligation). The guard is a free
        // function so this DB-free test can pin the exact error contract.
        let guard = crate::store::outbox_append_requires_unit_of_work();
        match guard {
            EventOutboxError::Backend(backend) => {
                assert_eq!(backend.code(), codes::OUTBOX_REQUIRES_UOW);
            }
            other => panic!("expected the guard backend error, got {other:?}"),
        }
    }
}
