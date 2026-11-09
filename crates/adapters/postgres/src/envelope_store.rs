// P3.28 — PostgreSQL implementation of the flexibility-envelope record
//         persistence contract (report §11.6, §16.1, §16.2; ledger P3.28
//         step 3; migration 0015).
//
// Implements `flexcore_storage::envelope_store::EnvelopeRecordStore` for
// [`PgUnitOfWork`], the same unit-of-work pattern as the P3.21/P3.22
// schedule stores: every statement runs inside the caller's one native
// transaction, so the envelope-record insert and the paired
// `FlexibilityEnvelopeUpdated` outbox rows (one per covered site, appended
// through the unit of work's `EventOutbox` implementation) are atomic —
// state + outbox, or neither (§15.5).
//
// Tenant scoping (P1.12 §5.1 conventions): the unit of work's
// transaction-scoped `flexcore.tenant_id` GUC feeds the `tenant_isolation`
// RLS policy on `flexibility_envelopes` (FORCEd, migration 0015); on top
// of RLS every statement filters by the context's tenant explicitly, and a
// foreign-tenant record reads as `None` / never lists — existence never
// leaks across a tenant boundary (P0.16 rule 5).
//
// Immutability: INSERT and SELECT only (the migration grants
// `flexcore_app` exactly those); a (tenant, portfolio, version) collision
// maps to [`EnvelopeStoreError::Conflict`] — envelope versions are
// immutable and a recomputation is a NEW version.
//
// Payload rule: the row's `envelope_payload_json` and `covered_site_ids`
// (a JSON array of canonical UUID texts) are validated to parse here (the
// storage crate cannot — no serialization dependency), then bound as
// `jsonb`. The site ids are stored as jsonb rather than `uuid[]` so the
// composition root's JSON rendering travels verbatim; the individual ids
// are validated as canonical UUID texts before the insert.
//
// VERIFY-BUILD (field-visibility precedent): this module accesses the
// private `tx`/`runtime` fields of `PgUnitOfWork` from a sibling module —
// exactly the pattern the existing `telemetry.rs`, `corrections.rs`, and
// `schedule_store.rs` modules use for their `impl PgUnitOfWork` blocks. If
// rustc rejects that access (E0616), the single-line fix is `pub(crate)`
// on the two field declarations in `unit_of_work.rs`, which repairs every
// module at once; that file was outside this subtask's ownership.

use crate::error::{is_unique_violation, map_backend};
use crate::unit_of_work::{tx_ref, PgUnitOfWork};
use crate::uuid_map::{uuid_from_text, uuid_text};
use flexcore_domain::tenant_context::TenantContext;
use flexcore_storage::envelope_store::{
    EnvelopeListFilter, EnvelopeRecordReceipt, EnvelopeRecordRow, EnvelopeRecordStore,
    EnvelopeStoreError,
};
use serde_json::Value;

/// The column projection (uuid columns selected back as `::text` — the
/// recorded FlexUuid mapping).
const ENVELOPE_SELECT: &str = "SELECT envelope_id::text AS envelope_id, \
     tenant_id::text AS tenant_id, portfolio_id::text AS portfolio_id, \
     envelope_version, horizon_start_millis, horizon_end_millis, \
     interval_millis, interval_count, covered_site_ids, envelope_payload \
     FROM flexibility_envelopes";

impl EnvelopeRecordStore for PgUnitOfWork {
    /// Inserts one envelope record inside THIS unit of work's transaction
    /// (adapter obligation 1: the same native transaction must carry the
    /// paired `FlexibilityEnvelopeUpdated` outbox rows). The
    /// (tenant, portfolio, version) uniqueness maps to
    /// [`EnvelopeStoreError::Conflict`] (records are immutable).
    fn insert_envelope_record(
        &self,
        ctx: &TenantContext,
        row: &EnvelopeRecordRow,
    ) -> Result<EnvelopeRecordReceipt, EnvelopeStoreError> {
        // The payload must be valid JSON before it reaches the database —
        // the storage crate treats it as opaque text, this adapter owns the
        // jsonb binding and therefore the validation.
        let payload: Value = serde_json::from_str(&row.envelope_payload_json)
            .map_err(|_| EnvelopeStoreError::InvalidRecord {
                field: "envelope_payload_json",
            })?;
        for (field, value) in [("envelope_id", &row.envelope_id), ("portfolio_id", &row.portfolio_id)] {
            uuid_from_text(value).map_err(|_| EnvelopeStoreError::InvalidRecord { field })?;
        }
        for site in &row.covered_site_ids {
            uuid_from_text(site)
                .map_err(|_| EnvelopeStoreError::InvalidRecord { field: "covered_site_ids" })?;
        }
        let interval_count_i32 = i32::try_from(row.interval_count).map_err(|_| {
            EnvelopeStoreError::InvalidRecord {
                field: "interval_count",
            }
        })?;
        let covered_sites: Value = serde_json::to_value(&row.covered_site_ids)
            .map_err(|_| EnvelopeStoreError::InvalidRecord {
                field: "covered_site_ids",
            })?;
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard)
                .await
                .map_err(|err| EnvelopeStoreError::Backend {
                    code: err.code().to_string(),
                    message: err.message().to_string(),
                })?;
            let result = sqlx::query(
                "INSERT INTO flexibility_envelopes \
                     (envelope_id, tenant_id, portfolio_id, envelope_version, \
                      horizon_start_millis, horizon_end_millis, interval_millis, \
                      interval_count, covered_site_ids, envelope_payload) \
                 VALUES ($1::uuid, $2::uuid, $3::uuid, $4, $5, $6, $7, $8, $9, $10)",
            )
            .bind(&row.envelope_id)
            .bind(uuid_text(ctx.tenant_id().uuid()))
            .bind(&row.portfolio_id)
            .bind(&row.envelope_version)
            .bind(row.horizon_start_millis)
            .bind(row.horizon_end_millis)
            .bind(row.interval_millis)
            .bind(interval_count_i32)
            .bind(covered_sites)
            .bind(payload)
            .execute(&mut *tx)
            .await
            .map_err(|err| map_envelope_store_error(err, &row.portfolio_id, &row.envelope_version))?;
            if result.rows_affected() != 1 {
                return Err(EnvelopeStoreError::Backend {
                    code: "E_STORE_REJECTED".to_string(),
                    message: "the flexibility-envelope insert affected no rows".to_string(),
                });
            }
            Ok(EnvelopeRecordReceipt::new(
                row.envelope_id.clone(),
                ctx.tenant_id(),
            ))
        })
    }

    /// Looks a record up by envelope identity within the caller's tenant
    /// scope (RLS GUC + explicit tenant filter; a foreign-tenant record
    /// reads as `None`).
    fn find_envelope_record(
        &self,
        ctx: &TenantContext,
        envelope_id: &str,
    ) -> Result<Option<EnvelopeRecordRow>, EnvelopeStoreError> {
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard)
                .await
                .map_err(|err| EnvelopeStoreError::Backend {
                    code: err.code().to_string(),
                    message: err.message().to_string(),
                })?;
            let row = sqlx::query(&format!(
                "{ENVELOPE_SELECT} WHERE envelope_id = $1::uuid AND tenant_id = $2::uuid"
            ))
            .bind(envelope_id)
            .bind(uuid_text(ctx.tenant_id().uuid()))
            .fetch_optional(&mut *tx)
            .await
            .map_err(map_envelope_backend)?;
            match row {
                None => Ok(None),
                Some(row) => decode_envelope_record_row(&row).map(Some),
            }
        })
    }

    /// Lists records tenant-scoped: keyset over `envelope_id` ascending,
    /// bounded by `limit`, filtered by the optional portfolio filter and
    /// the optional cursor position. No offset pagination anywhere.
    fn list_envelope_records(
        &self,
        ctx: &TenantContext,
        filter: &EnvelopeListFilter,
        limit: usize,
    ) -> Result<Vec<EnvelopeRecordRow>, EnvelopeStoreError> {
        if let Some(portfolio) = filter.portfolio_id.as_deref() {
            uuid_from_text(portfolio)
                .map_err(|_| EnvelopeStoreError::InvalidRecord { field: "portfolio_id" })?;
        }
        if let Some(after) = filter.after_envelope_id.as_deref() {
            uuid_from_text(after)
                .map_err(|_| EnvelopeStoreError::InvalidRecord { field: "after_envelope_id" })?;
        }
        let limit_i64 = i64::try_from(limit).map_err(|_| EnvelopeStoreError::Backend {
            code: "E_STORE_LIMIT".to_string(),
            message: "the page limit is out of range".to_string(),
        })?;
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard)
                .await
                .map_err(|err| EnvelopeStoreError::Backend {
                    code: err.code().to_string(),
                    message: err.message().to_string(),
                })?;
            // One parameterized shape: the filters are always bound (None
            // = no constraint), so the statement text is constant.
            let rows = sqlx::query(
                &format!(
                    "{ENVELOPE_SELECT} \
                     WHERE tenant_id = $1::uuid \
                       AND ($2::uuid IS NULL OR portfolio_id = $2::uuid) \
                       AND ($3::uuid IS NULL OR envelope_id > $3::uuid) \
                     ORDER BY envelope_id ASC \
                     LIMIT $4"
                ),
            )
            .bind(uuid_text(ctx.tenant_id().uuid()))
            .bind(filter.portfolio_id.as_deref())
            .bind(filter.after_envelope_id.as_deref())
            .bind(limit_i64)
            .fetch_all(&mut *tx)
            .await
            .map_err(map_envelope_backend)?;
            rows.iter().map(decode_envelope_record_row).collect()
        })
    }
}

/// Maps a sqlx failure onto the storage error. A uniqueness violation on
/// the insert path is the immutability conflict (the (tenant, portfolio,
/// version) unique constraint; the colliding identity is the caller-supplied
/// value — the adapter never reads engine text, per the no-raw-storage-text
/// rule); everything else is the opaque backend class.
fn map_envelope_store_error(
    err: sqlx::Error,
    portfolio_id: &str,
    envelope_version: &str,
) -> EnvelopeStoreError {
    if is_unique_violation(&err) {
        return EnvelopeStoreError::Conflict {
            portfolio_id: portfolio_id.to_string(),
            envelope_version: envelope_version.to_string(),
        };
    }
    let backend = map_backend(err);
    EnvelopeStoreError::Backend {
        code: backend.code().to_string(),
        message: backend.message().to_string(),
    }
}

/// Maps a read-side sqlx failure onto the opaque backend class.
fn map_envelope_backend(err: sqlx::Error) -> EnvelopeStoreError {
    let backend = map_backend(err);
    EnvelopeStoreError::Backend {
        code: backend.code().to_string(),
        message: backend.message().to_string(),
    }
}

/// Decodes one `flexibility_envelopes` row into the storage row type,
/// re-validating through the row constructor (fail closed on a malformed
/// row — the same discipline as the schedule-record store).
fn decode_envelope_record_row(
    row: &sqlx::postgres::PgRow,
) -> Result<EnvelopeRecordRow, EnvelopeStoreError> {
    let try_text = |column: &'static str| -> Result<String, EnvelopeStoreError> {
        row.try_get::<String, _>(column).map_err(|_| {
            EnvelopeStoreError::Backend {
                code: "E_STORE_DECODE".to_string(),
                message: format!("stored value in column {column} could not be decoded"),
            }
        })
    };
    let try_i64 = |column: &'static str| -> Result<i64, EnvelopeStoreError> {
        row.try_get::<i64, _>(column).map_err(|_| {
            EnvelopeStoreError::Backend {
                code: "E_STORE_DECODE".to_string(),
                message: format!("stored value in column {column} could not be decoded"),
            }
        })
    };
    let envelope_id = try_text("envelope_id")?;
    let tenant_text = try_text("tenant_id")?;
    let tenant_uuid = uuid_from_text(&tenant_text).map_err(|_| EnvelopeStoreError::Backend {
        code: "E_STORE_DECODE".to_string(),
        message: "stored value in column tenant_id could not be decoded".to_string(),
    })?;
    let portfolio_id = try_text("portfolio_id")?;
    let envelope_version = try_text("envelope_version")?;
    let horizon_start_millis = try_i64("horizon_start_millis")?;
    let horizon_end_millis = try_i64("horizon_end_millis")?;
    let interval_millis = try_i64("interval_millis")?;
    let interval_count_i32 = row
        .try_get::<i32, _>("interval_count")
        .map_err(|_| EnvelopeStoreError::Backend {
            code: "E_STORE_DECODE".to_string(),
            message: "stored value in column interval_count could not be decoded".to_string(),
        })?;
    let interval_count = u32::try_from(interval_count_i32).map_err(|_| {
        EnvelopeStoreError::Backend {
            code: "E_STORE_DECODE".to_string(),
            message: "stored value in column interval_count is out of range".to_string(),
        }
    })?;
    let covered_sites_value: Value = row
        .try_get::<Value, _>("covered_site_ids")
        .map_err(|_| EnvelopeStoreError::Backend {
            code: "E_STORE_DECODE".to_string(),
            message: "stored value in column covered_site_ids could not be decoded".to_string(),
        })?;
    let covered_site_ids = covered_sites_value
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|v| v.as_str().map(str::to_string).unwrap_or_default())
        .collect::<Vec<String>>();
    let payload: Value = row
        .try_get::<Value, _>("envelope_payload")
        .map_err(|_| EnvelopeStoreError::Backend {
            code: "E_STORE_DECODE".to_string(),
            message: "stored value in column envelope_payload could not be decoded".to_string(),
        })?;
    let envelope_payload_json = serde_json::to_string(&payload).map_err(|_| {
        EnvelopeStoreError::Backend {
            code: "E_STORE_DECODE".to_string(),
            message: "the stored envelope payload could not be rendered".to_string(),
        }
    })?;
    // Re-validate through the row constructor: a row that does not satisfy
    // the storage-level completeness rules cannot be returned (the adapter
    // fails closed on a malformed row rather than smuggling it through).
    EnvelopeRecordRow::new(
        flexcore_domain::ids::TenantId::new(flexcore_contracts::ids::FlexUuid::from_bytes(
            tenant_uuid.to_bytes(),
        )),
        portfolio_id,
        envelope_id,
        envelope_version,
        horizon_start_millis,
        horizon_end_millis,
        interval_millis,
        interval_count,
        covered_site_ids,
        envelope_payload_json,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // Composition-root evidence: the envelope store is object-safe so the
    // control-plane unit-of-work bundle can hold it as a trait object (the
    // same evidence pattern as the schedule stores).
    #[test]
    fn the_envelope_record_store_is_object_safe_for_the_unit_of_work() {
        fn takes_dyn_store(_: &dyn EnvelopeRecordStore) {}
        let _ = takes_dyn_store as fn(&dyn EnvelopeRecordStore);
    }
}
