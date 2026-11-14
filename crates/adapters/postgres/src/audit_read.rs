// P4.36 — PostgreSQL implementation of the audit-events READ contract
//         (report §15.2, §16.2, §21.3; ledger P4.36; decision record:
//         `ledger/phase-04-grid-feasibility-dispatch/36_audit_read_api.md`).
//
// Implements `flexcore_storage::audit_read::AuditEventReader` for
// [`PgUnitOfWork`] on the exact P4.30 `feasibility_check.rs` pattern. The
// `audit_records` table (migration 0008) is append-only: flexcore_app holds
// INSERT + SELECT only and the before-update/delete trigger refuses row
// mutation unconditionally (P1.12 §4.7 layers 1–2). This module is
// SELECT-only by construction — there is no write statement here, and none
// may be added (writes stay on the P1.10 `AuditSink`, P1.13 `audit.rs`).
//
// Tenant scoping is the exact feasibility-check discipline: the
// transaction-scoped `flexcore.tenant_id` GUC feeds the `tenant_isolation`
// RLS policy (ENABLE + FORCE, migration 0008), every statement filters by
// the context's tenant explicitly, and a foreign-tenant (or platform-level
// NULL-tenant) record reads as `None` — existence never leaks (P0.16 rule
// 5, §16.2). Every filter value is a bound parameter and the limit is
// ALWAYS applied (no unbounded query).
//
// VERIFY-BUILD (field-visibility precedent): this module accesses the
// private `tx`/`runtime` fields of `PgUnitOfWork` from a sibling module —
// exactly the pattern the existing `command_store.rs`, `telemetry.rs`,
// `feasibility_check.rs`, and `schedule_store.rs` modules use for their
// `impl PgUnitOfWork` blocks.

use crate::error::map_backend;
use crate::unit_of_work::{tx_ref, PgUnitOfWork};
use crate::uuid_map::{uuid_from_text, uuid_text};
use flexcore_domain::ids::TenantId;
use flexcore_storage::audit_read::{
    AuditEventFilter, AuditEventReadError, AuditEventReader, AuditEventRow,
};
use serde_json::Value;
use time::OffsetDateTime;

/// The column projection of one audit record (uuid columns selected back
/// as `::text` — the recorded FlexUuid mapping; the jsonb reference
/// columns are decoded as JSON values in [`decode_audit_event_row`]).
const AUDIT_EVENT_SELECT: &str = "SELECT audit_id::text AS audit_id, \
     tenant_id::text AS tenant_id, actor, action, object_type, object_id, \
     before_ref, after_ref, decision, evidence, event_time, ingest_time, \
     correlation_id, corrected_audit_id::text AS corrected_audit_id \
     FROM audit_records";

/// Converts one stored timestamptz to UTC unix millis (the contract-level
/// time representation; the driver decodes UTC).
fn millis_from_timestamp(value: OffsetDateTime) -> Result<i64, AuditEventReadError> {
    i64::try_from(value.unix_timestamp_nanos().div_euclid(1_000_000)).map_err(|_| {
        AuditEventReadError::Backend {
            code: "E_STORE_DECODE".to_string(),
            message: "stored timestamp is out of the millisecond range".to_string(),
        }
    })
}

/// Builds a UTC timestamp from a UTC unix-millis filter bound (the inverse
/// of [`millis_from_timestamp`]).
fn timestamp_from_millis(
    millis: i64,
    field: &'static str,
) -> Result<OffsetDateTime, AuditEventReadError> {
    OffsetDateTime::from_unix_timestamp_nanos((millis as i128) * 1_000_000).map_err(|_| {
        AuditEventReadError::InvalidFilter { field }
    })
}

/// One jsonb reference column as its pointer string: a JSON string
/// round-trips exactly; any other jsonb shape renders as its JSON text
/// (rows written by this adapter always store the JSON-string form — the
/// recorded P1.12 §4.7 reference discipline, same decoding as the
/// repository `col_json_ref` helper).
fn json_ref_text(value: Option<Value>) -> Option<String> {
    value.map(|value| match value {
        Value::String(text) => text,
        other => serde_json::to_string(&other).unwrap_or_else(|_| "{}".to_string()),
    })
}

fn decode_failure(field: &'static str) -> AuditEventReadError {
    AuditEventReadError::Backend {
        code: "E_STORE_DECODE".to_string(),
        message: format!("stored value in column {field} could not be decoded"),
    }
}

/// Decodes one `audit_records` row into the storage row type,
/// re-validating through the row constructor (fail closed on a malformed
/// row — the same discipline as the feasibility-check store).
fn decode_audit_event_row(
    row: &sqlx::postgres::PgRow,
) -> Result<AuditEventRow, AuditEventReadError> {
    let try_text = |column: &'static str| -> Result<String, AuditEventReadError> {
        row.try_get::<String, _>(column).map_err(|_| decode_failure(column))
    };
    let try_opt_text = |column: &'static str| -> Result<Option<String>, AuditEventReadError> {
        row.try_get::<Option<String>, _>(column)
            .map_err(|_| decode_failure(column))
    };
    let try_opt_json = |column: &'static str| -> Result<Option<Value>, AuditEventReadError> {
        row.try_get::<Option<Value>, _>(column)
            .map_err(|_| decode_failure(column))
    };
    let try_millis =
        |column: &'static str| -> Result<i64, AuditEventReadError> {
            let raw: OffsetDateTime =
                row.try_get(column).map_err(|_| decode_failure(column))?;
            millis_from_timestamp(raw)
        };
    // The decision jsonb decodes into its two halves: outcome + reason
    // reference (a decision is outcome + reason, never a bare allow/deny
    // — P1.11; the row constructor enforces the pairing).
    let decision = try_opt_json("decision")?;
    let decision_outcome = decision
        .as_ref()
        .and_then(|value| value.get("outcome"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let decision_reason_reference = decision
        .as_ref()
        .and_then(|value| value.get("reason_reference"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let tenant_id: Option<TenantId> = match try_opt_text("tenant_id")? {
        None => None,
        Some(text) => {
            let uuid = uuid_from_text(&text).map_err(|_| decode_failure("tenant_id"))?;
            Some(TenantId::new(flexcore_contracts::ids::FlexUuid::from_bytes(
                uuid.to_bytes(),
            )))
        }
    };
    AuditEventRow::new(
        try_text("audit_id")?,
        tenant_id,
        try_text("actor")?,
        try_text("action")?,
        try_text("object_type")?,
        try_text("object_id")?,
        json_ref_text(try_opt_json("before_ref")?),
        json_ref_text(try_opt_json("after_ref")?),
        decision_outcome,
        decision_reason_reference,
        json_ref_text(try_opt_json("evidence")?),
        try_millis("event_time")?,
        try_millis("ingest_time")?,
        try_text("correlation_id")?,
        try_opt_text("corrected_audit_id")?,
    )
    .map_err(|_| AuditEventReadError::Backend {
        code: "E_STORE_DECODE".to_string(),
        message: "the stored audit record failed the storage-level rules".to_string(),
    })
}

/// Validates one canonical-UUID text field, mapping a malformed value to
/// the named read error (the adapter fails closed before the database sees
/// a value it could only reject with raw engine text).
fn require_uuid_text(value: &str, field: &'static str) -> Result<(), AuditEventReadError> {
    uuid_from_text(value)
        .map(|_| ())
        .map_err(|_| AuditEventReadError::InvalidFilter { field })
}

/// Maps a finalized-unit-of-work failure (the `tx_ref` error class) onto
/// the opaque read error.
fn map_unit_of_work_error(err: flexcore_ports::backend_error::BackendError) -> AuditEventReadError {
    AuditEventReadError::Backend {
        code: err.code().to_string(),
        message: err.message().to_string(),
    }
}

/// Maps a statement failure (the `sqlx::Error` class) onto the opaque read
/// error — never raw storage text (the P4.30 discipline).
fn map_sql_error(err: sqlx::Error) -> AuditEventReadError {
    let backend = map_backend(err);
    AuditEventReadError::Backend {
        code: backend.code().to_string(),
        message: backend.message().to_string(),
    }
}

impl AuditEventReader for PgUnitOfWork {
    fn find_audit_event(
        &self,
        ctx: &flexcore_domain::tenant_context::TenantContext,
        audit_id: &str,
    ) -> Result<Option<AuditEventRow>, AuditEventReadError> {
        if audit_id.trim().is_empty() {
            return Err(AuditEventReadError::InvalidFilter { field: "audit_id" });
        }
        require_uuid_text(audit_id.trim(), "audit_id")?;
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx =
                tx_ref(&mut guard).await.map_err(map_unit_of_work_error)?;
            let row = sqlx::query(&format!(
                "{AUDIT_EVENT_SELECT} WHERE audit_id = $1::uuid AND tenant_id = $2::uuid"
            ))
            .bind(audit_id.trim())
            .bind(uuid_text(ctx.tenant_id().uuid()))
            .fetch_optional(&mut *tx)
            .await
            .map_err(map_sql_error)?;
            match row {
                // A foreign-tenant (or platform-level NULL-tenant) record
                // reads as None — no existence leak (P0.16 rule 5; RLS
                // already filtered it).
                None => Ok(None),
                Some(row) => decode_audit_event_row(&row).map(Some),
            }
        })
    }

    fn list_audit_events(
        &self,
        ctx: &flexcore_domain::tenant_context::TenantContext,
        filter: &AuditEventFilter,
        limit: u32,
    ) -> Result<Vec<AuditEventRow>, AuditEventReadError> {
        for (field, value) in [
            ("action", filter.action.as_deref()),
            ("actor", filter.actor.as_deref()),
            ("object_type", filter.object_type.as_deref()),
            ("object_id", filter.object_id.as_deref()),
        ] {
            if let Some(value) = value {
                if value.trim().is_empty() {
                    return Err(AuditEventReadError::InvalidFilter { field });
                }
            }
        }
        if let (Some(from), Some(to)) =
            (filter.from_event_unix_millis, filter.to_event_unix_millis)
        {
            if from > to {
                return Err(AuditEventReadError::InvalidFilter {
                    field: "from_event_unix_millis",
                });
            }
        }
        if let Some(after) = filter.after_audit_id.as_deref() {
            require_uuid_text(after.trim(), "after_audit_id")?;
        }
        let from = filter
            .from_event_unix_millis
            .map(|millis| timestamp_from_millis(millis, "from_event_unix_millis"))
            .transpose()?;
        let to = filter
            .to_event_unix_millis
            .map(|millis| timestamp_from_millis(millis, "to_event_unix_millis"))
            .transpose()?;
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx =
                tx_ref(&mut guard).await.map_err(map_unit_of_work_error)?;
            // Ordered by (event_time, audit_id) — the trait's documented
            // listing order — with the explicit tenant filter on top of
            // RLS (P0.16 rule 5, §16.2). Every filter value is a bound
            // parameter; the limit is ALWAYS applied (no unbounded query).
            // The time bounds are INCLUSIVE on both ends (the recorded
            // P4.36 DEFAULT decision).
            //
            // Placeholder numbering: `params` grows in the exact order the
            // placeholders appear in the statement, so every predicate
            // names its own bind — no positional collisions (each `let n`
            // is taken BEFORE the next push).
            #[derive(Debug)]
            enum Bind {
                Text(String),
                Time(OffsetDateTime),
                Limit(i64),
            }
            let mut params: Vec<Bind> = vec![Bind::Text(uuid_text(ctx.tenant_id().uuid()))];
            let mut statement = format!("{AUDIT_EVENT_SELECT} WHERE tenant_id = $1::uuid");
            if let Some(action) = filter.action.as_deref() {
                let n = params.len() + 1;
                params.push(Bind::Text(action.to_string()));
                statement.push_str(&format!(" AND action = ${n}"));
            }
            if let Some(actor) = filter.actor.as_deref() {
                let n = params.len() + 1;
                params.push(Bind::Text(actor.to_string()));
                statement.push_str(&format!(" AND actor = ${n}"));
            }
            if let Some(object_type) = filter.object_type.as_deref() {
                let n = params.len() + 1;
                params.push(Bind::Text(object_type.to_string()));
                statement.push_str(&format!(" AND object_type = ${n}"));
            }
            if let Some(object_id) = filter.object_id.as_deref() {
                let n = params.len() + 1;
                params.push(Bind::Text(object_id.to_string()));
                statement.push_str(&format!(" AND object_id = ${n}"));
            }
            if let Some(from) = from {
                let n = params.len() + 1;
                params.push(Bind::Time(from));
                statement.push_str(&format!(" AND event_time >= ${n}"));
            }
            if let Some(to) = to {
                let n = params.len() + 1;
                params.push(Bind::Time(to));
                statement.push_str(&format!(" AND event_time <= ${n}"));
            }
            if let Some(after) = filter.after_audit_id.as_deref() {
                let n = params.len() + 1;
                params.push(Bind::Text(after.trim().to_string()));
                statement.push_str(&format!(
                    " AND (event_time, audit_id) > \
                     ((SELECT event_time FROM audit_records \
                       WHERE audit_id = ${n}::uuid AND tenant_id = $1::uuid), ${n}::uuid)"
                ));
            }
            let n = params.len() + 1;
            params.push(Bind::Limit(i64::from(limit)));
            statement
                .push_str(&format!(" ORDER BY event_time ASC, audit_id ASC LIMIT ${n}"));
            let mut query = sqlx::query(&statement);
            for bind in params {
                query = match bind {
                    Bind::Text(value) => query.bind(value),
                    Bind::Time(value) => query.bind(value),
                    Bind::Limit(value) => query.bind(value),
                };
            }
            let rows = query
                .fetch_all(&mut *tx)
                .await
                .map_err(map_sql_error)?;
            rows.iter().map(decode_audit_event_row).collect()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The composition root holds ports as trait objects; object safety is
    // part of the contract (the same evidence pattern as the adapter's
    // `every_implemented_port_is_object_safe`). Compile-level evidence that
    // PgUnitOfWork carries the audit-events reader.
    #[test]
    fn the_audit_event_reader_is_object_safe_for_the_unit_of_work() {
        fn takes_dyn_reader(_: &dyn AuditEventReader) {}
        let _ = takes_dyn_reader as fn(&dyn AuditEventReader);
    }

    #[test]
    fn the_reader_source_contains_no_write_statement() {
        // The P4.36 read port is SELECT-only: the adapter module never
        // issues a write statement against audit_records (writes stay on
        // the P1.13 AuditSink module). The forbidden statement heads are
        // assembled with format! so this test does not itself contain the
        // forbidden source strings (the same evidence pattern as the
        // P1.10 sink source test).
        let source = include_str!("audit_read.rs");
        for (head, rest) in [
            ("INSERT", " INTO"),
            ("UPDATE", " audit_records"),
            ("DELETE", " FROM"),
        ] {
            let statement = format!("{head}{rest}");
            assert!(
                !source.contains(&statement),
                "the audit-events reader must never issue a write statement ({statement})"
            );
        }
    }
}
