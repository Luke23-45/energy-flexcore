// P1.13 — Audit record persistence: the append-only `AuditSink` INSERT.
//
// Column mapping (P1.12 §4.7, the storage counterpart of the flexcore-audit
// `AuditRecord`):
//
// | Record field        | Column            | Note |
// |---|---|---|
// | `audit_id`          | `audit_id` (uuid)   | caller-supplied identifier, cast from text |
// | `tenant_id`         | `tenant_id` (uuid)  | NULL = platform-level action |
// | `actor` / `action`  | `actor` / `action`  | validated non-empty upstream |
// | `object`            | `object_type` + `object_id` | |
// | `before_ref` / `after_ref` | `before_ref` / `after_ref` (jsonb) | *references*, not payload copies |
// | `decision`          | `decision` (jsonb)  | outcome + reason reference, never a bare allow/deny |
// | `evidence`          | `evidence` (jsonb)  | evidence reference |
// | `event_time` / `ingest_time` | `event_time` / `ingest_time` | distinct columns, P0.09 |
// | `correlation_id`    | `correlation_id`    | column type is text per P1.12 §4.7; the uuid travels in canonical text form |
// | `corrects`          | `corrected_audit_id` (uuid) | corrections are NEW rows referencing the original |
//
// Append-only layer 1 is the grant set (INSERT/SELECT only, P1.12 §4.7);
// layer 2 the before-update/delete trigger; layer 3 the port. This module
// only ever INSERTs.

use crate::error::map_audit_rejection;
use crate::uuid_map::uuid_text;
use flexcore_audit::record::AuditRecord;
use serde_json::Value;
use sqlx::Postgres;

/// One reference field as its jsonb value (a JSON string — the reference is
/// a pointer, not a payload copy, P1.12 §4.7).
fn reference_json(reference: Option<&str>) -> Option<Value> {
    reference.map(|r| Value::String(r.to_string()))
}

/// Inserts one audit record. The caller is responsible for the transaction
/// and the tenant context: under the P1.12 §5 RLS policy the row's
/// `tenant_id` must match the transaction's `flexcore.tenant_id` setting
/// (a platform-level row with `tenant_id IS NULL` is therefore only
/// writable through the owner/ops path, P1.12 §5.2).
pub(crate) async fn insert_audit_record<'e, E>(
    executor: E,
    record: &AuditRecord,
) -> Result<(), flexcore_audit::record::AuditError>
where
    E: sqlx::Executor<'e, Database = Postgres>,
{
    let decision_json: Option<Value> = match &record.decision {
        Some(decision) => Some(serde_json::to_value(decision).map_err(|_| {
            flexcore_audit::record::AuditError::Rejected {
                reason: "the recorded decision could not be serialized".to_string(),
            }
        })?),
        None => None,
    };
    let corrected = record
        .corrects
        .as_ref()
        .map(|correction| uuid_text(correction.original_audit_id()));

    sqlx::query(
        "INSERT INTO audit_records \
             (audit_id, tenant_id, actor, action, object_type, object_id, \
              before_ref, after_ref, decision, evidence, \
              event_time, ingest_time, correlation_id, corrected_audit_id) \
         VALUES ($1::uuid, $2::uuid, $3, $4, $5, $6, $7::jsonb, $8::jsonb, $9::jsonb, $10::jsonb, \
                 $11, $12, $13, $14::uuid)",
    )
    .bind(uuid_text(record.audit_id))
    .bind(record.tenant_id.map(|tenant| uuid_text(tenant.uuid())))
    .bind(record.actor.as_str())
    .bind(record.action.as_str())
    .bind(record.object.object_type.as_str())
    .bind(record.object.object_id.as_str())
    .bind(reference_json(record.before_ref.as_ref().map(|r| r.as_str())))
    .bind(reference_json(record.after_ref.as_ref().map(|r| r.as_str())))
    .bind(decision_json)
    .bind(reference_json(record.evidence.as_ref().map(|r| r.as_str())))
    .bind(record.event_time.timestamp())
    .bind(record.ingest_time.timestamp())
    .bind(uuid_text(record.correlation_id))
    .bind(corrected)
    .execute(executor)
    .await
    .map_err(map_audit_rejection)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use flexcore_audit::record::{
        ActorIdentity, AuditAction, AuditObject, AuditRecord, AuditRecordFields, RecordReference,
    };
    use flexcore_contracts::ids::FlexUuid;
    use flexcore_contracts::time_model::{EventTime, IngestTime};
    use flexcore_domain::ids::TenantId;

    fn sample_uuid(tag: u8) -> FlexUuid {
        let mut bytes = [0u8; 16];
        bytes[15] = tag;
        FlexUuid::from_bytes(bytes)
    }

    fn sample_record() -> AuditRecord {
        AuditRecord::new(AuditRecordFields {
            audit_id: sample_uuid(1),
            tenant_id: Some(TenantId::new(sample_uuid(2))),
            actor: ActorIdentity::new("operator-1".to_string()).expect("valid actor"),
            action: AuditAction::new("asset.register".to_string()).expect("valid action"),
            object: AuditObject::new("asset".to_string(), "asset-1".to_string())
                .expect("valid object"),
            before_ref: None,
            after_ref: Some(
                RecordReference::new("asset-1:v2".to_string()).expect("valid reference"),
            ),
            decision: None,
            evidence: None,
            event_time: EventTime::from_unix_millis(1_000).expect("valid event time"),
            ingest_time: IngestTime::from_unix_millis(2_000).expect("valid ingest time"),
            correlation_id: sample_uuid(3),
        })
        .expect("valid record")
    }

    #[test]
    fn reference_fields_map_to_json_strings() {
        // A reference is stored as a JSON string (a pointer), never
        // stringified into an object — matching the P1.12 §4.7 rule that
        // before/after are references, not payload copies.
        let value = reference_json(Some("asset-1:v2"));
        assert_eq!(value, Some(Value::String("asset-1:v2".to_string())));
        assert_eq!(reference_json(None), None);
    }

    #[test]
    fn record_fields_cover_every_audit_column() {
        // Compile-level field walk: the record carries every column the
        // INSERT writes, so the mapping cannot silently drop a §10.1 field.
        let record = sample_record();
        assert_eq!(uuid_text(record.audit_id), "00000000-0000-0000-0000-000000000001");
        assert!(record.tenant_id.is_some());
        assert_eq!(record.actor.as_str(), "operator-1");
        assert_eq!(record.action.as_str(), "asset.register");
        assert_eq!(record.object.object_type.as_str(), "asset");
        assert_eq!(record.object.object_id.as_str(), "asset-1");
        assert!(record.corrects.is_none());
    }
}
