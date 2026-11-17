// P15.15 — PostgreSQL implementation of the interconnection-record store
//         (UL 1741 SB COCs, IEEE 1547.1-2020 test reports, per DEVICE
//         MODEL; migration 0021).
//
// Implements `flexcore_storage::interconnection_store::InterconnectionRecordStore`
// for [`PgUnitOfWork`], the same unit-of-work pattern as the P3.28
// envelope store: every statement runs inside the caller's one native
// transaction.
//
// TENANCY (recorded — different from the tenant-owned stores):
// interconnection records are per DEVICE MODEL — certification is a
// property of the equipment model, not of any tenant's instance of it.
// The table is a platform-global equipment catalog: NO tenant_id column,
// NO tenant RLS (migration 0021; the decision is recorded in the storage
// trait header and the decision record). Reads are exact-linkage lookups
// on the declared device-model string — a near-miss lists NOTHING and
// never matches approximately (the domain statement mirrored).
//
// Immutability: INSERT and SELECT only (the migration grants flexcore_app
// exactly those); a (device_model, document_ref) collision maps to
// [`InterconnectionStoreError::Conflict`] — records are immutable and a
// new document version is a NEW record.
//
// Payload rule: `certified_functions` and `ride_through_categories` are
// jsonb arrays of the P14.13 catalog names; this adapter owns the JSON
// binding and therefore validates on insert that the payload parses and
// that EVERY entry names a closed-vocabulary function/category
// (`flexcore_domain::ieee1547_settings`) — an unknown name is refused,
// never stored (fail-closed, the P0.08 discipline mirrored).
//
// The platform LISTS certified equipment; it never claims certification
// for itself [S-21][S-169][S-170] — this module stores REFERENCES to
// certification documents, nothing more.

use std::collections::HashSet;

use crate::error::{is_unique_violation, map_backend};
use crate::unit_of_work::{tx_ref, PgUnitOfWork};
use flexcore_domain::ieee1547_settings::{RideThroughCategory, SettingFunctionId};
use flexcore_storage::interconnection_store::{
    InterconnectionRecordReceipt, InterconnectionRecordRow, InterconnectionRecordStore,
    InterconnectionStoreError,
};
use serde_json::Value;

/// The column projection (uuid columns selected back as `::text` — the
/// recorded FlexUuid mapping).
const INTERCONNECTION_SELECT: &str = "SELECT record_id::text AS record_id, \
     device_model, document_kind, document_ref, document_version, issuer, category, \
     certified_functions, ride_through_categories, settings_profile_ref, \
     ingested_at_unix_ms, ingestion_source \
     FROM interconnection_records";

/// The closed vocabulary of P14.13 catalog function names (the canonical
/// snake_case spellings; the enum is the single home, this set is built
/// from it so a catalog change flows through).
fn certified_function_names() -> HashSet<&'static str> {
    SettingFunctionId::ALL.iter().map(|f| f.name()).collect()
}

/// The closed vocabulary of P14.13 ride-through category names.
fn ride_through_category_names() -> HashSet<&'static str> {
    RideThroughCategory::ALL.iter().map(|c| c.name()).collect()
}

/// Validates one jsonb-array payload field: it must parse as a JSON array
/// of strings, and every string must name a closed-vocabulary entry. An
/// unknown name is refused, never stored.
fn validate_catalog_json(payload: &str, allowed: &HashSet<&'static str>, field: &'static str) -> Result<(), InterconnectionStoreError> {
    let value: Value = serde_json::from_str(payload).map_err(|_| {
        InterconnectionStoreError::InvalidPayloadJson { field }
    })?;
    let entries = value.as_array().ok_or({
        InterconnectionStoreError::InvalidPayloadJson { field }
    })?;
    for entry in entries {
        let name = entry.as_str().ok_or({
            InterconnectionStoreError::InvalidPayloadJson { field }
        })?;
        if !allowed.contains(name) {
            return Err(InterconnectionStoreError::InvalidPayloadJson { field });
        }
    }
    Ok(())
}

impl InterconnectionRecordStore for PgUnitOfWork {
    /// Inserts one interconnection record (immutable; INSERT only). A
    /// (device_model, document_ref) collision maps to
    /// [`InterconnectionStoreError::Conflict`].
    fn insert_interconnection_record(
        &self,
        row: &InterconnectionRecordRow,
    ) -> Result<InterconnectionRecordReceipt, InterconnectionStoreError> {
        // The identity and payload validation the storage crate cannot
        // do (no serialization dependency there): record_id must be a
        // canonical UUID text; the catalog payloads must be arrays of
        // closed-vocabulary names.
        crate::uuid_map::uuid_from_text(&row.record_id).map_err(|_| {
            InterconnectionStoreError::InvalidRecord { field: "record_id" }
        })?;
        validate_catalog_json(
            &row.certified_functions_json,
            &certified_function_names(),
            "certified_functions",
        )?;
        validate_catalog_json(
            &row.ride_through_categories_json,
            &ride_through_category_names(),
            "ride_through_categories",
        )?;
        let functions: Value = serde_json::from_str(&row.certified_functions_json)
            .map_err(|_| InterconnectionStoreError::InvalidPayloadJson {
                field: "certified_functions",
            })?;
        let categories: Value = serde_json::from_str(&row.ride_through_categories_json)
            .map_err(|_| InterconnectionStoreError::InvalidPayloadJson {
                field: "ride_through_categories",
            })?;
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard)
                .await
                .map_err(|err| InterconnectionStoreError::Backend {
                    code: err.code().to_string(),
                    message: err.message().to_string(),
                })?;
            let result = sqlx::query(
                "INSERT INTO interconnection_records \
                     (record_id, device_model, document_kind, document_ref, \
                      document_version, issuer, category, certified_functions, \
                      ride_through_categories, settings_profile_ref, \
                      ingested_at_unix_ms, ingestion_source) \
                 VALUES ($1::uuid, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)",
            )
            .bind(&row.record_id)
            .bind(&row.device_model)
            .bind(&row.document_kind)
            .bind(&row.document_ref)
            .bind(&row.document_version)
            .bind(&row.issuer)
            .bind(&row.category)
            .bind(functions)
            .bind(categories)
            .bind(&row.settings_profile_ref)
            .bind(row.ingested_at_unix_ms)
            .bind(&row.ingestion_source)
            .execute(&mut *tx)
            .await
            .map_err(|err| map_interconnection_store_error(err, &row.device_model, &row.document_ref))?;
            if result.rows_affected() != 1 {
                return Err(InterconnectionStoreError::Backend {
                    code: "E_STORE_REJECTED".to_string(),
                    message: "the interconnection-record insert affected no rows"
                        .to_string(),
                });
            }
            Ok(InterconnectionRecordReceipt::new(
                row.record_id.clone(),
                row.device_model.clone(),
            ))
        })
    }

    /// Every record for ONE device model, in ascending record_id order
    /// (the exact-linkage read; no fuzzy match).
    fn records_for_device_model(
        &self,
        device_model: &str,
    ) -> Result<Vec<InterconnectionRecordRow>, InterconnectionStoreError> {
        if device_model.trim().is_empty() {
            return Err(InterconnectionStoreError::InvalidRecord {
                field: "device_model",
            });
        }
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard)
                .await
                .map_err(|err| InterconnectionStoreError::Backend {
                    code: err.code().to_string(),
                    message: err.message().to_string(),
                })?;
            let rows = sqlx::query(&format!(
                "{INTERCONNECTION_SELECT} WHERE device_model = $1 ORDER BY record_id ASC"
            ))
            .bind(device_model)
            .fetch_all(&mut *tx)
            .await
            .map_err(map_interconnection_backend)?;
            rows.iter().map(decode_interconnection_record_row).collect()
        })
    }
}

/// Maps a sqlx failure onto the storage error. A uniqueness violation on
/// the insert path is the immutability conflict (the (device_model,
/// document_ref) unique constraint; the colliding identity is the
/// caller-supplied value — the adapter never reads engine text, per the
/// no-raw-storage-text rule); everything else is the opaque backend class.
fn map_interconnection_store_error(err: sqlx::Error, device_model: &str, document_ref: &str) -> InterconnectionStoreError {
    if is_unique_violation(&err) {
        return InterconnectionStoreError::Conflict {
            device_model: device_model.to_string(),
            document_ref: document_ref.to_string(),
        };
    }
    let backend = map_backend(err);
    InterconnectionStoreError::Backend {
        code: backend.code().to_string(),
        message: backend.message().to_string(),
    }
}

/// Maps a read-side sqlx failure onto the opaque backend class.
fn map_interconnection_backend(err: sqlx::Error) -> InterconnectionStoreError {
    let backend = map_backend(err);
    InterconnectionStoreError::Backend {
        code: backend.code().to_string(),
        message: backend.message().to_string(),
    }
}

/// Decodes one `interconnection_records` row into the storage row type,
/// re-validating through the row constructor (fail closed on a malformed
/// row — the same discipline as the envelope-record store).
fn decode_interconnection_record_row(
    row: &sqlx::postgres::PgRow,
) -> Result<InterconnectionRecordRow, InterconnectionStoreError> {
    let try_text = |column: &'static str| -> Result<String, InterconnectionStoreError> {
        row.try_get::<String, _>(column).map_err(|_| {
            InterconnectionStoreError::Backend {
                code: "E_STORE_DECODE".to_string(),
                message: format!("stored value in column {column} could not be decoded"),
            }
        })
    };
    let try_i64 = |column: &'static str| -> Result<i64, InterconnectionStoreError> {
        row.try_get::<i64, _>(column).map_err(|_| {
            InterconnectionStoreError::Backend {
                code: "E_STORE_DECODE".to_string(),
                message: format!("stored value in column {column} could not be decoded"),
            }
        })
    };
    let record_id = try_text("record_id")?;
    let device_model = try_text("device_model")?;
    let document_kind = try_text("document_kind")?;
    let document_ref = try_text("document_ref")?;
    let document_version = try_text("document_version")?;
    let issuer = try_text("issuer")?;
    let category = try_text("category")?;
    let functions: Value = row
        .try_get::<Value, _>("certified_functions")
        .map_err(|_| InterconnectionStoreError::Backend {
            code: "E_STORE_DECODE".to_string(),
            message: "stored value in column certified_functions could not be decoded"
                .to_string(),
        })?;
    let categories: Value = row
        .try_get::<Value, _>("ride_through_categories")
        .map_err(|_| InterconnectionStoreError::Backend {
            code: "E_STORE_DECODE".to_string(),
            message:
                "stored value in column ride_through_categories could not be decoded"
                    .to_string(),
        })?;
    let certified_functions_json = serde_json::to_string(&functions).map_err(|_| {
        InterconnectionStoreError::Backend {
            code: "E_STORE_DECODE".to_string(),
            message: "the stored certified_functions payload could not be rendered"
                .to_string(),
        }
    })?;
    let ride_through_categories_json =
        serde_json::to_string(&categories).map_err(|_| {
            InterconnectionStoreError::Backend {
                code: "E_STORE_DECODE".to_string(),
                message:
                    "the stored ride_through_categories payload could not be rendered"
                        .to_string(),
            }
        })?;
    let settings_profile_ref = try_text("settings_profile_ref")?;
    let ingested_at_unix_ms = try_i64("ingested_at_unix_ms")?;
    let ingestion_source = try_text("ingestion_source")?;
    // Re-validate through the row constructor: a row that does not
    // satisfy the storage-level completeness rules cannot be returned.
    InterconnectionRecordRow::new(
        record_id,
        device_model,
        document_kind,
        document_ref,
        document_version,
        issuer,
        category,
        certified_functions_json,
        ride_through_categories_json,
        settings_profile_ref,
        ingested_at_unix_ms,
        ingestion_source,
    )
}
