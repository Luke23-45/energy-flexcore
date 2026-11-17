// P16.01 — PostgreSQL implementation of the market-records store
//         (programs, contracts, enrollments, market events, deliveries,
//         settlements, retention configuration; migration 0022).
//
// Implements `flexcore_storage::market_records_store::MarketRecordsStore`
// for [`PgUnitOfWork`], the same unit-of-work pattern as the P3.28
// envelope store and the P15.15 interconnection store: every statement
// runs inside the caller's one native transaction.
//
// AUDIT-CHAIN LINKAGE (the P11 chain is consumed, never redefined): the
// caller pairs every insert with its P11 `audit_records` append THROUGH
// THE P11 SINK in the same unit of work; the row's `audit_record_id`
// names that appended record. This module performs NO chain computation
// of its own — no chain columns exist on any market-records table, and
// the keyed-HMAC chain treatment, the NULL-while-unkeyed semantics, and
// the LR-203 production-crypto requirement live on `audit_records`
// (migration 0019) exactly as P11.15 recorded them.
//
// Immutability: INSERT + SELECT only on all six record tables (the
// migration grants carry exactly that); a (record_id, tenant_id)
// collision maps to [`MarketRecordsStoreError::Conflict`] — a corrected
// record is a NEW superseding row. The retention-configuration table is
// the only mutable surface (upsert).
//
// Payload rule: the JSON payload columns (`program_config`, `terms`,
// `qualification_evidence`, `event_payload`, `statement_payload`) are
// jsonb; this adapter owns the JSON binding and therefore validates on
// insert that each payload parses and carries the shape the storage row
// declares (object for the config/terms/payload columns, array for the
// qualification-evidence list) — a wrong shape is refused, never stored
// (fail-closed, the P0.08 discipline mirrored).
//
// Money discipline: `amount_text` is validated against the decimal-text
// shape and `currency_code` against the three-letter shape by the row
// constructor; this module performs NO numeric conversion of any money
// value and NO arithmetic — the amount is bound as TEXT exactly as
// transcribed.
//
// Tenant scoping: every statement runs under the caller's
// transaction-scoped `flexcore.tenant_id` (the P1.12 §5.1 RLS
// conventions); the read methods filter explicitly by tenant (and by
// program/event where the read is scoped to one).
//
// Honesty: this module implements RECORD STORAGE. No market is
// connected, no registration is filed, no settlement statement is
// received, and no qualification is claimed anywhere (ADR-011;
// conventions §7.3). Every table starts EMPTY.

use crate::error::{is_unique_violation, map_backend};
use crate::unit_of_work::{tx_ref, PgUnitOfWork};
use flexcore_storage::market_records_store::{
    is_currency_code_shape, is_decimal_text, MarketContractRow, MarketDeliveryRow,
    MarketEnrollmentRow, MarketEventRow, MarketProgramRow, MarketRecordReceipt,
    MarketRecordsStore, MarketRecordsStoreError, MarketRetentionConfigRow, MarketSettlementRow,
};
use serde_json::Value;

/// Validates that a payload field parses as JSON and carries the shape
/// the storage row declares: `want_object` selects object vs array. A
/// wrong shape is refused, never stored.
fn validate_payload_json(payload: &str, want_object: bool, field: &'static str) -> Result<(), MarketRecordsStoreError> {
    let value: Value = serde_json::from_str(payload).map_err(|_| {
        MarketRecordsStoreError::InvalidPayloadJson { field }
    })?;
    let shape_ok = if want_object {
        value.is_object()
    } else {
        value.is_array()
    };
    if shape_ok {
        Ok(())
    } else {
        Err(MarketRecordsStoreError::InvalidPayloadJson { field })
    }
}

/// Validates the identity fields the storage crate cannot parse (no UUID
/// dependency there): every identifier must be canonical UUID text.
fn validate_uuid_field(value: &str, field: &'static str) -> Result<(), MarketRecordsStoreError> {
    crate::uuid_map::uuid_from_text(value)
        .map(|_| ())
        .map_err(|_| MarketRecordsStoreError::InvalidRecord { field })
}

/// Validates the settlement money fields beyond the row constructor:
/// the decimal-text shape and the currency shape are re-checked here so
/// a storage row that bypasses the constructor still cannot carry a
/// non-decimal amount (defence in depth; no numeric conversion exists).
fn validate_money_fields(
    amount_text: &str,
    currency_code: &str,
) -> Result<(), MarketRecordsStoreError> {
    if !is_decimal_text(amount_text) {
        return Err(MarketRecordsStoreError::InvalidRecord { field: "amount_text" });
    }
    if !is_currency_code_shape(currency_code) {
        return Err(MarketRecordsStoreError::InvalidRecord {
            field: "currency_code",
        });
    }
    Ok(())
}

fn backend_error(err: sqlx::Error) -> MarketRecordsStoreError {
    let backend = map_backend(err);
    MarketRecordsStoreError::Backend {
        code: backend.code().to_string(),
        message: backend.message().to_string(),
    }
}

/// Maps an insert-side sqlx failure onto the storage error. A uniqueness
/// violation is the immutability conflict (the (record_id, tenant_id)
/// PRIMARY KEY; the colliding identity is the caller-supplied value —
/// the adapter never reads engine text, per the no-raw-storage-text
/// rule); everything else is the opaque backend class.
fn insert_conflict_or_backend(
    err: sqlx::Error,
    record_id: &str,
    tenant_id: &str,
) -> MarketRecordsStoreError {
    if is_unique_violation(&err) {
        return MarketRecordsStoreError::Conflict {
            record_id: record_id.to_string(),
            tenant_id: tenant_id.to_string(),
        };
    }
    backend_error(err)
}

impl MarketRecordsStore for PgUnitOfWork {
    fn insert_market_program(
        &self,
        row: &MarketProgramRow,
    ) -> Result<MarketRecordReceipt, MarketRecordsStoreError> {
        validate_uuid_field(&row.program_id, "program_id")?;
        validate_uuid_field(&row.tenant_id, "tenant_id")?;
        validate_uuid_field(&row.audit_record_id, "audit_record_id")?;
        validate_payload_json(&row.program_config_json, true, "program_config")?;
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard)
                .await
                .map_err(|err| MarketRecordsStoreError::Backend {
                    code: err.code().to_string(),
                    message: err.message().to_string(),
                })?;
            let config: Value = serde_json::from_str(&row.program_config_json).map_err(|_| {
                MarketRecordsStoreError::InvalidPayloadJson { field: "program_config" }
            })?;
            let result = sqlx::query(
                "INSERT INTO market_programs \
                     (program_id, tenant_id, program_name, jurisdiction, state, \
                      program_config, supersedes_program_id, audit_record_id, \
                      recorded_at_unix_ms, recorded_by) \
                 VALUES ($1::uuid, $2::uuid, $3, $4, $5, $6, $7::uuid, $8::uuid, $9, $10)",
            )
            .bind(&row.program_id)
            .bind(&row.tenant_id)
            .bind(&row.program_name)
            .bind(&row.jurisdiction)
            .bind(&row.state)
            .bind(config)
            .bind(row.supersedes_program_id.as_deref())
            .bind(&row.audit_record_id)
            .bind(row.recorded_at_unix_ms)
            .bind(&row.recorded_by)
            .execute(&mut *tx)
            .await
            .map_err(|err| {
                insert_conflict_or_backend(err, &row.program_id, &row.tenant_id)
            })?;
            if result.rows_affected() != 1 {
                return Err(MarketRecordsStoreError::Backend {
                    code: "E_STORE_REJECTED".to_string(),
                    message: "the market-program insert affected no rows".to_string(),
                });
            }
            Ok(MarketRecordReceipt::new(
                row.program_id.clone(),
                row.tenant_id.clone(),
                "PROGRAM".to_string(),
            ))
        })
    }

    fn market_program_rows_for_tenant(
        &self,
        tenant_id: &str,
    ) -> Result<Vec<MarketProgramRow>, MarketRecordsStoreError> {
        validate_uuid_field(tenant_id, "tenant_id")?;
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard)
                .await
                .map_err(|err| MarketRecordsStoreError::Backend {
                    code: err.code().to_string(),
                    message: err.message().to_string(),
                })?;
            let rows = sqlx::query(
                "SELECT program_id::text AS program_id, tenant_id::text AS tenant_id, \
                        program_name, jurisdiction, state, program_config, \
                        supersedes_program_id::text AS supersedes_program_id, \
                        audit_record_id::text AS audit_record_id, \
                        recorded_at_unix_ms, recorded_by \
                 FROM market_programs WHERE tenant_id = $1::uuid ORDER BY program_id ASC",
            )
            .bind(tenant_id)
            .fetch_all(&mut *tx)
            .await
            .map_err(backend_error)?;
            rows.iter().map(decode_market_program_row).collect()
        })
    }

    fn insert_market_contract(
        &self,
        row: &MarketContractRow,
    ) -> Result<MarketRecordReceipt, MarketRecordsStoreError> {
        validate_uuid_field(&row.contract_id, "contract_id")?;
        validate_uuid_field(&row.tenant_id, "tenant_id")?;
        validate_uuid_field(&row.program_id, "program_id")?;
        validate_uuid_field(&row.audit_record_id, "audit_record_id")?;
        validate_payload_json(&row.terms_json, true, "terms")?;
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard)
                .await
                .map_err(|err| MarketRecordsStoreError::Backend {
                    code: err.code().to_string(),
                    message: err.message().to_string(),
                })?;
            let terms: Value = serde_json::from_str(&row.terms_json).map_err(|_| {
                MarketRecordsStoreError::InvalidPayloadJson { field: "terms" }
            })?;
            let result = sqlx::query(
                "INSERT INTO market_contracts \
                     (contract_id, tenant_id, program_id, contract_ref, state, terms, \
                      supersedes_contract_id, audit_record_id, recorded_at_unix_ms, recorded_by) \
                 VALUES ($1::uuid, $2::uuid, $3::uuid, $4, $5, $6, $7::uuid, $8::uuid, $9, $10)",
            )
            .bind(&row.contract_id)
            .bind(&row.tenant_id)
            .bind(&row.program_id)
            .bind(&row.contract_ref)
            .bind(&row.state)
            .bind(terms)
            .bind(row.supersedes_contract_id.as_deref())
            .bind(&row.audit_record_id)
            .bind(row.recorded_at_unix_ms)
            .bind(&row.recorded_by)
            .execute(&mut *tx)
            .await
            .map_err(|err| {
                insert_conflict_or_backend(err, &row.contract_id, &row.tenant_id)
            })?;
            if result.rows_affected() != 1 {
                return Err(MarketRecordsStoreError::Backend {
                    code: "E_STORE_REJECTED".to_string(),
                    message: "the market-contract insert affected no rows".to_string(),
                });
            }
            Ok(MarketRecordReceipt::new(
                row.contract_id.clone(),
                row.tenant_id.clone(),
                "CONTRACT".to_string(),
            ))
        })
    }

    fn market_contract_rows_for_program(
        &self,
        tenant_id: &str,
        program_id: &str,
    ) -> Result<Vec<MarketContractRow>, MarketRecordsStoreError> {
        validate_uuid_field(tenant_id, "tenant_id")?;
        validate_uuid_field(program_id, "program_id")?;
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard)
                .await
                .map_err(|err| MarketRecordsStoreError::Backend {
                    code: err.code().to_string(),
                    message: err.message().to_string(),
                })?;
            let rows = sqlx::query(
                "SELECT contract_id::text AS contract_id, tenant_id::text AS tenant_id, \
                        program_id::text AS program_id, contract_ref, state, terms, \
                        supersedes_contract_id::text AS supersedes_contract_id, \
                        audit_record_id::text AS audit_record_id, \
                        recorded_at_unix_ms, recorded_by \
                 FROM market_contracts WHERE tenant_id = $1::uuid AND program_id = $2::uuid \
                 ORDER BY contract_id ASC",
            )
            .bind(tenant_id)
            .bind(program_id)
            .fetch_all(&mut *tx)
            .await
            .map_err(backend_error)?;
            rows.iter().map(decode_market_contract_row).collect()
        })
    }

    fn insert_market_enrollment(
        &self,
        row: &MarketEnrollmentRow,
    ) -> Result<MarketRecordReceipt, MarketRecordsStoreError> {
        validate_uuid_field(&row.enrollment_id, "enrollment_id")?;
        validate_uuid_field(&row.tenant_id, "tenant_id")?;
        validate_uuid_field(&row.program_id, "program_id")?;
        validate_uuid_field(&row.asset_id, "asset_id")?;
        validate_uuid_field(&row.audit_record_id, "audit_record_id")?;
        validate_payload_json(
            &row.qualification_evidence_json,
            false,
            "qualification_evidence",
        )?;
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard)
                .await
                .map_err(|err| MarketRecordsStoreError::Backend {
                    code: err.code().to_string(),
                    message: err.message().to_string(),
                })?;
            let evidence: Value =
                serde_json::from_str(&row.qualification_evidence_json).map_err(|_| {
                    MarketRecordsStoreError::InvalidPayloadJson {
                        field: "qualification_evidence",
                    }
                })?;
            let result = sqlx::query(
                "INSERT INTO market_enrollments \
                     (enrollment_id, tenant_id, program_id, asset_id, state, \
                      window_start_millis, window_end_millis, qualification_evidence, \
                      supersedes_enrollment_id, audit_record_id, \
                      recorded_at_unix_ms, recorded_by) \
                 VALUES ($1::uuid, $2::uuid, $3::uuid, $4::uuid, $5, $6, $7, $8, \
                         $9::uuid, $10::uuid, $11, $12)",
            )
            .bind(&row.enrollment_id)
            .bind(&row.tenant_id)
            .bind(&row.program_id)
            .bind(&row.asset_id)
            .bind(&row.state)
            .bind(row.window_start_millis)
            .bind(row.window_end_millis)
            .bind(evidence)
            .bind(row.supersedes_enrollment_id.as_deref())
            .bind(&row.audit_record_id)
            .bind(row.recorded_at_unix_ms)
            .bind(&row.recorded_by)
            .execute(&mut *tx)
            .await
            .map_err(|err| {
                insert_conflict_or_backend(err, &row.enrollment_id, &row.tenant_id)
            })?;
            if result.rows_affected() != 1 {
                return Err(MarketRecordsStoreError::Backend {
                    code: "E_STORE_REJECTED".to_string(),
                    message: "the market-enrollment insert affected no rows".to_string(),
                });
            }
            Ok(MarketRecordReceipt::new(
                row.enrollment_id.clone(),
                row.tenant_id.clone(),
                "ENROLLMENT".to_string(),
            ))
        })
    }

    fn market_enrollment_rows_for_program(
        &self,
        tenant_id: &str,
        program_id: &str,
    ) -> Result<Vec<MarketEnrollmentRow>, MarketRecordsStoreError> {
        validate_uuid_field(tenant_id, "tenant_id")?;
        validate_uuid_field(program_id, "program_id")?;
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard)
                .await
                .map_err(|err| MarketRecordsStoreError::Backend {
                    code: err.code().to_string(),
                    message: err.message().to_string(),
                })?;
            let rows = sqlx::query(
                "SELECT enrollment_id::text AS enrollment_id, tenant_id::text AS tenant_id, \
                        program_id::text AS program_id, asset_id::text AS asset_id, state, \
                        window_start_millis, window_end_millis, qualification_evidence, \
                        supersedes_enrollment_id::text AS supersedes_enrollment_id, \
                        audit_record_id::text AS audit_record_id, \
                        recorded_at_unix_ms, recorded_by \
                 FROM market_enrollments WHERE tenant_id = $1::uuid AND program_id = $2::uuid \
                 ORDER BY enrollment_id ASC",
            )
            .bind(tenant_id)
            .bind(program_id)
            .fetch_all(&mut *tx)
            .await
            .map_err(backend_error)?;
            rows.iter().map(decode_market_enrollment_row).collect()
        })
    }

    fn insert_market_event(
        &self,
        row: &MarketEventRow,
    ) -> Result<MarketRecordReceipt, MarketRecordsStoreError> {
        validate_uuid_field(&row.event_id, "event_id")?;
        validate_uuid_field(&row.tenant_id, "tenant_id")?;
        validate_uuid_field(&row.program_id, "program_id")?;
        validate_uuid_field(&row.audit_record_id, "audit_record_id")?;
        validate_payload_json(&row.event_payload_json, true, "event_payload")?;
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard)
                .await
                .map_err(|err| MarketRecordsStoreError::Backend {
                    code: err.code().to_string(),
                    message: err.message().to_string(),
                })?;
            let payload: Value = serde_json::from_str(&row.event_payload_json).map_err(|_| {
                MarketRecordsStoreError::InvalidPayloadJson { field: "event_payload" }
            })?;
            let result = sqlx::query(
                "INSERT INTO market_events \
                     (event_id, tenant_id, program_id, external_ref, event_kind, \
                      start_millis, end_millis, event_payload, audit_record_id, \
                      recorded_at_unix_ms, recorded_by) \
                 VALUES ($1::uuid, $2::uuid, $3::uuid, $4, $5, $6, $7, $8, $9::uuid, $10, $11)",
            )
            .bind(&row.event_id)
            .bind(&row.tenant_id)
            .bind(&row.program_id)
            .bind(&row.external_ref)
            .bind(&row.event_kind)
            .bind(row.start_millis)
            .bind(row.end_millis)
            .bind(payload)
            .bind(&row.audit_record_id)
            .bind(row.recorded_at_unix_ms)
            .bind(&row.recorded_by)
            .execute(&mut *tx)
            .await
            .map_err(|err| insert_conflict_or_backend(err, &row.event_id, &row.tenant_id))?;
            if result.rows_affected() != 1 {
                return Err(MarketRecordsStoreError::Backend {
                    code: "E_STORE_REJECTED".to_string(),
                    message: "the market-event insert affected no rows".to_string(),
                });
            }
            Ok(MarketRecordReceipt::new(
                row.event_id.clone(),
                row.tenant_id.clone(),
                "MARKET_EVENT".to_string(),
            ))
        })
    }

    fn market_event_rows_for_program(
        &self,
        tenant_id: &str,
        program_id: &str,
    ) -> Result<Vec<MarketEventRow>, MarketRecordsStoreError> {
        validate_uuid_field(tenant_id, "tenant_id")?;
        validate_uuid_field(program_id, "program_id")?;
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard)
                .await
                .map_err(|err| MarketRecordsStoreError::Backend {
                    code: err.code().to_string(),
                    message: err.message().to_string(),
                })?;
            let rows = sqlx::query(
                "SELECT event_id::text AS event_id, tenant_id::text AS tenant_id, \
                        program_id::text AS program_id, external_ref, event_kind, \
                        start_millis, end_millis, event_payload, \
                        audit_record_id::text AS audit_record_id, \
                        recorded_at_unix_ms, recorded_by \
                 FROM market_events WHERE tenant_id = $1::uuid AND program_id = $2::uuid \
                 ORDER BY event_id ASC",
            )
            .bind(tenant_id)
            .bind(program_id)
            .fetch_all(&mut *tx)
            .await
            .map_err(backend_error)?;
            rows.iter().map(decode_market_event_row).collect()
        })
    }

    fn insert_market_delivery(
        &self,
        row: &MarketDeliveryRow,
    ) -> Result<MarketRecordReceipt, MarketRecordsStoreError> {
        validate_uuid_field(&row.delivery_id, "delivery_id")?;
        validate_uuid_field(&row.tenant_id, "tenant_id")?;
        validate_uuid_field(&row.program_id, "program_id")?;
        validate_uuid_field(&row.event_id, "event_id")?;
        validate_uuid_field(&row.asset_id, "asset_id")?;
        validate_uuid_field(&row.audit_record_id, "audit_record_id")?;
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard)
                .await
                .map_err(|err| MarketRecordsStoreError::Backend {
                    code: err.code().to_string(),
                    message: err.message().to_string(),
                })?;
            let result = sqlx::query(
                "INSERT INTO market_deliveries \
                     (delivery_id, tenant_id, program_id, event_id, asset_id, \
                      interval_start_millis, interval_end_millis, delivered_energy_mwh, \
                      measured_quality, audit_record_id, recorded_at_unix_ms, recorded_by) \
                 VALUES ($1::uuid, $2::uuid, $3::uuid, $4::uuid, $5::uuid, $6, $7, $8, $9, \
                         $10::uuid, $11, $12)",
            )
            .bind(&row.delivery_id)
            .bind(&row.tenant_id)
            .bind(&row.program_id)
            .bind(&row.event_id)
            .bind(&row.asset_id)
            .bind(row.interval_start_millis)
            .bind(row.interval_end_millis)
            .bind(row.delivered_energy_mwh)
            .bind(&row.measured_quality)
            .bind(&row.audit_record_id)
            .bind(row.recorded_at_unix_ms)
            .bind(&row.recorded_by)
            .execute(&mut *tx)
            .await
            .map_err(|err| {
                insert_conflict_or_backend(err, &row.delivery_id, &row.tenant_id)
            })?;
            if result.rows_affected() != 1 {
                return Err(MarketRecordsStoreError::Backend {
                    code: "E_STORE_REJECTED".to_string(),
                    message: "the market-delivery insert affected no rows".to_string(),
                });
            }
            Ok(MarketRecordReceipt::new(
                row.delivery_id.clone(),
                row.tenant_id.clone(),
                "DELIVERY".to_string(),
            ))
        })
    }

    fn market_delivery_rows_for_event(
        &self,
        tenant_id: &str,
        event_id: &str,
    ) -> Result<Vec<MarketDeliveryRow>, MarketRecordsStoreError> {
        validate_uuid_field(tenant_id, "tenant_id")?;
        validate_uuid_field(event_id, "event_id")?;
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard)
                .await
                .map_err(|err| MarketRecordsStoreError::Backend {
                    code: err.code().to_string(),
                    message: err.message().to_string(),
                })?;
            let rows = sqlx::query(
                "SELECT delivery_id::text AS delivery_id, tenant_id::text AS tenant_id, \
                        program_id::text AS program_id, event_id::text AS event_id, \
                        asset_id::text AS asset_id, interval_start_millis, \
                        interval_end_millis, delivered_energy_mwh, measured_quality, \
                        audit_record_id::text AS audit_record_id, \
                        recorded_at_unix_ms, recorded_by \
                 FROM market_deliveries WHERE tenant_id = $1::uuid AND event_id = $2::uuid \
                 ORDER BY asset_id ASC, interval_start_millis ASC",
            )
            .bind(tenant_id)
            .bind(event_id)
            .fetch_all(&mut *tx)
            .await
            .map_err(backend_error)?;
            rows.iter().map(decode_market_delivery_row).collect()
        })
    }

    fn insert_market_settlement(
        &self,
        row: &MarketSettlementRow,
    ) -> Result<MarketRecordReceipt, MarketRecordsStoreError> {
        validate_uuid_field(&row.settlement_id, "settlement_id")?;
        validate_uuid_field(&row.tenant_id, "tenant_id")?;
        validate_uuid_field(&row.program_id, "program_id")?;
        validate_uuid_field(&row.audit_record_id, "audit_record_id")?;
        validate_money_fields(&row.amount_text, &row.currency_code)?;
        validate_payload_json(&row.statement_payload_json, true, "statement_payload")?;
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard)
                .await
                .map_err(|err| MarketRecordsStoreError::Backend {
                    code: err.code().to_string(),
                    message: err.message().to_string(),
                })?;
            let payload: Value =
                serde_json::from_str(&row.statement_payload_json).map_err(|_| {
                    MarketRecordsStoreError::InvalidPayloadJson {
                        field: "statement_payload",
                    }
                })?;
            let result = sqlx::query(
                "INSERT INTO market_settlements \
                     (settlement_id, tenant_id, program_id, statement_ref, source_format, \
                      period_start_millis, period_end_millis, line_item_kind, amount_text, \
                      currency_code, statement_payload, supersedes_settlement_id, \
                      audit_record_id, recorded_at_unix_ms, recorded_by) \
                 VALUES ($1::uuid, $2::uuid, $3::uuid, $4, $5, $6, $7, $8, $9, $10, $11, \
                         $12::uuid, $13::uuid, $14, $15)",
            )
            .bind(&row.settlement_id)
            .bind(&row.tenant_id)
            .bind(&row.program_id)
            .bind(&row.statement_ref)
            .bind(&row.source_format)
            .bind(row.period_start_millis)
            .bind(row.period_end_millis)
            .bind(&row.line_item_kind)
            // The amount is bound AS TEXT exactly as transcribed — no
            // numeric conversion exists anywhere on this path.
            .bind(&row.amount_text)
            .bind(&row.currency_code)
            .bind(payload)
            .bind(row.supersedes_settlement_id.as_deref())
            .bind(&row.audit_record_id)
            .bind(row.recorded_at_unix_ms)
            .bind(&row.recorded_by)
            .execute(&mut *tx)
            .await
            .map_err(|err| {
                insert_conflict_or_backend(err, &row.settlement_id, &row.tenant_id)
            })?;
            if result.rows_affected() != 1 {
                return Err(MarketRecordsStoreError::Backend {
                    code: "E_STORE_REJECTED".to_string(),
                    message: "the market-settlement insert affected no rows".to_string(),
                });
            }
            Ok(MarketRecordReceipt::new(
                row.settlement_id.clone(),
                row.tenant_id.clone(),
                "SETTLEMENT".to_string(),
            ))
        })
    }

    fn market_settlement_rows_for_program(
        &self,
        tenant_id: &str,
        program_id: &str,
    ) -> Result<Vec<MarketSettlementRow>, MarketRecordsStoreError> {
        validate_uuid_field(tenant_id, "tenant_id")?;
        validate_uuid_field(program_id, "program_id")?;
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard)
                .await
                .map_err(|err| MarketRecordsStoreError::Backend {
                    code: err.code().to_string(),
                    message: err.message().to_string(),
                })?;
            let rows = sqlx::query(
                "SELECT settlement_id::text AS settlement_id, tenant_id::text AS tenant_id, \
                        program_id::text AS program_id, statement_ref, source_format, \
                        period_start_millis, period_end_millis, line_item_kind, amount_text, \
                        currency_code, statement_payload, \
                        supersedes_settlement_id::text AS supersedes_settlement_id, \
                        audit_record_id::text AS audit_record_id, \
                        recorded_at_unix_ms, recorded_by \
                 FROM market_settlements WHERE tenant_id = $1::uuid AND program_id = $2::uuid \
                 ORDER BY settlement_id ASC",
            )
            .bind(tenant_id)
            .bind(program_id)
            .fetch_all(&mut *tx)
            .await
            .map_err(backend_error)?;
            rows.iter().map(decode_market_settlement_row).collect()
        })
    }

    fn upsert_market_retention_config(
        &self,
        row: &MarketRetentionConfigRow,
    ) -> Result<MarketRecordReceipt, MarketRecordsStoreError> {
        validate_uuid_field(&row.tenant_id, "tenant_id")?;
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard)
                .await
                .map_err(|err| MarketRecordsStoreError::Backend {
                    code: err.code().to_string(),
                    message: err.message().to_string(),
                })?;
            let result = sqlx::query(
                "INSERT INTO market_record_retention_config \
                     (tenant_id, record_class, retention_state, period_days, basis, \
                      set_by, set_at_unix_ms) \
                 VALUES ($1::uuid, $2, $3, $4, $5, $6, $7) \
                 ON CONFLICT (tenant_id, record_class) DO UPDATE SET \
                     retention_state = EXCLUDED.retention_state, \
                     period_days = EXCLUDED.period_days, \
                     basis = EXCLUDED.basis, \
                     set_by = EXCLUDED.set_by, \
                     set_at_unix_ms = EXCLUDED.set_at_unix_ms",
            )
            .bind(&row.tenant_id)
            .bind(&row.record_class)
            .bind(&row.retention_state)
            .bind(row.period_days)
            .bind(row.basis.as_deref())
            .bind(&row.set_by)
            .bind(row.set_at_unix_ms)
            .execute(&mut *tx)
            .await
            .map_err(backend_error)?;
            if result.rows_affected() == 0 {
                return Err(MarketRecordsStoreError::Backend {
                    code: "E_STORE_REJECTED".to_string(),
                    message: "the retention-configuration upsert affected no rows"
                        .to_string(),
                });
            }
            Ok(MarketRecordReceipt::new(
                row.record_class.clone(),
                row.tenant_id.clone(),
                "RETENTION_CONFIG".to_string(),
            ))
        })
    }

    fn market_retention_config_rows_for_tenant(
        &self,
        tenant_id: &str,
    ) -> Result<Vec<MarketRetentionConfigRow>, MarketRecordsStoreError> {
        validate_uuid_field(tenant_id, "tenant_id")?;
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard)
                .await
                .map_err(|err| MarketRecordsStoreError::Backend {
                    code: err.code().to_string(),
                    message: err.message().to_string(),
                })?;
            let rows = sqlx::query(
                "SELECT tenant_id::text AS tenant_id, record_class, retention_state, \
                        period_days, basis, set_by, set_at_unix_ms \
                 FROM market_record_retention_config WHERE tenant_id = $1::uuid \
                 ORDER BY record_class ASC",
            )
            .bind(tenant_id)
            .fetch_all(&mut *tx)
            .await
            .map_err(backend_error)?;
            rows.iter().map(decode_market_retention_config_row).collect()
        })
    }
}

// --- Row decoding ----------------------------------------------------------------

/// Re-validates through the row constructor: a stored row that does not
/// satisfy the storage-level completeness rules cannot be returned (the
/// fail-closed decode discipline of the envelope/interconnection stores).
fn decode_market_program_row(
    row: &sqlx::postgres::PgRow,
) -> Result<MarketProgramRow, MarketRecordsStoreError> {
    let text = |column: &'static str| -> Result<String, MarketRecordsStoreError> {
        row.try_get::<String, _>(column).map_err(|_| {
            MarketRecordsStoreError::Backend {
                code: "E_STORE_DECODE".to_string(),
                message: format!("stored value in column {column} could not be decoded"),
            }
        })
    };
    let int = |column: &'static str| -> Result<i64, MarketRecordsStoreError> {
        row.try_get::<i64, _>(column).map_err(|_| {
            MarketRecordsStoreError::Backend {
                code: "E_STORE_DECODE".to_string(),
                message: format!("stored value in column {column} could not be decoded"),
            }
        })
    };
    let config: Value = row
        .try_get::<Value, _>("program_config")
        .map_err(|_| MarketRecordsStoreError::Backend {
            code: "E_STORE_DECODE".to_string(),
            message: "stored value in column program_config could not be decoded".to_string(),
        })?;
    MarketProgramRow::new(
        text("program_id")?,
        text("tenant_id")?,
        text("program_name")?,
        text("jurisdiction")?,
        text("state")?,
        serde_json::to_string(&config).map_err(|_| MarketRecordsStoreError::Backend {
            code: "E_STORE_DECODE".to_string(),
            message: "the stored program_config payload could not be rendered".to_string(),
        })?,
        row.try_get::<Option<String>, _>("supersedes_program_id")
            .map_err(|_| MarketRecordsStoreError::Backend {
                code: "E_STORE_DECODE".to_string(),
                message: "stored value in column supersedes_program_id could not be decoded"
                    .to_string(),
            })?,
        text("audit_record_id")?,
        int("recorded_at_unix_ms")?,
        text("recorded_by")?,
    )
}

fn decode_market_contract_row(
    row: &sqlx::postgres::PgRow,
) -> Result<MarketContractRow, MarketRecordsStoreError> {
    let text = |column: &'static str| -> Result<String, MarketRecordsStoreError> {
        row.try_get::<String, _>(column).map_err(|_| {
            MarketRecordsStoreError::Backend {
                code: "E_STORE_DECODE".to_string(),
                message: format!("stored value in column {column} could not be decoded"),
            }
        })
    };
    let int = |column: &'static str| -> Result<i64, MarketRecordsStoreError> {
        row.try_get::<i64, _>(column).map_err(|_| {
            MarketRecordsStoreError::Backend {
                code: "E_STORE_DECODE".to_string(),
                message: format!("stored value in column {column} could not be decoded"),
            }
        })
    };
    let terms: Value = row
        .try_get::<Value, _>("terms")
        .map_err(|_| MarketRecordsStoreError::Backend {
            code: "E_STORE_DECODE".to_string(),
            message: "stored value in column terms could not be decoded".to_string(),
        })?;
    MarketContractRow::new(
        text("contract_id")?,
        text("tenant_id")?,
        text("program_id")?,
        text("contract_ref")?,
        text("state")?,
        serde_json::to_string(&terms).map_err(|_| MarketRecordsStoreError::Backend {
            code: "E_STORE_DECODE".to_string(),
            message: "the stored terms payload could not be rendered".to_string(),
        })?,
        row.try_get::<Option<String>, _>("supersedes_contract_id")
            .map_err(|_| MarketRecordsStoreError::Backend {
                code: "E_STORE_DECODE".to_string(),
                message: "stored value in column supersedes_contract_id could not be decoded"
                    .to_string(),
            })?,
        text("audit_record_id")?,
        int("recorded_at_unix_ms")?,
        text("recorded_by")?,
    )
}

fn decode_market_enrollment_row(
    row: &sqlx::postgres::PgRow,
) -> Result<MarketEnrollmentRow, MarketRecordsStoreError> {
    let text = |column: &'static str| -> Result<String, MarketRecordsStoreError> {
        row.try_get::<String, _>(column).map_err(|_| {
            MarketRecordsStoreError::Backend {
                code: "E_STORE_DECODE".to_string(),
                message: format!("stored value in column {column} could not be decoded"),
            }
        })
    };
    let int = |column: &'static str| -> Result<i64, MarketRecordsStoreError> {
        row.try_get::<i64, _>(column).map_err(|_| {
            MarketRecordsStoreError::Backend {
                code: "E_STORE_DECODE".to_string(),
                message: format!("stored value in column {column} could not be decoded"),
            }
        })
    };
    let opt_int = |column: &'static str| -> Result<Option<i64>, MarketRecordsStoreError> {
        row.try_get::<Option<i64>, _>(column).map_err(|_| {
            MarketRecordsStoreError::Backend {
                code: "E_STORE_DECODE".to_string(),
                message: format!("stored value in column {column} could not be decoded"),
            }
        })
    };
    let evidence: Value = row
        .try_get::<Value, _>("qualification_evidence")
        .map_err(|_| MarketRecordsStoreError::Backend {
            code: "E_STORE_DECODE".to_string(),
            message: "stored value in column qualification_evidence could not be decoded"
                .to_string(),
        })?;
    MarketEnrollmentRow::new(
        text("enrollment_id")?,
        text("tenant_id")?,
        text("program_id")?,
        text("asset_id")?,
        text("state")?,
        opt_int("window_start_millis")?,
        opt_int("window_end_millis")?,
        serde_json::to_string(&evidence).map_err(|_| MarketRecordsStoreError::Backend {
            code: "E_STORE_DECODE".to_string(),
            message: "the stored qualification_evidence payload could not be rendered"
                .to_string(),
        })?,
        row.try_get::<Option<String>, _>("supersedes_enrollment_id")
            .map_err(|_| MarketRecordsStoreError::Backend {
                code: "E_STORE_DECODE".to_string(),
                message: "stored value in column supersedes_enrollment_id could not be decoded"
                    .to_string(),
            })?,
        text("audit_record_id")?,
        int("recorded_at_unix_ms")?,
        text("recorded_by")?,
    )
}

fn decode_market_event_row(
    row: &sqlx::postgres::PgRow,
) -> Result<MarketEventRow, MarketRecordsStoreError> {
    let text = |column: &'static str| -> Result<String, MarketRecordsStoreError> {
        row.try_get::<String, _>(column).map_err(|_| {
            MarketRecordsStoreError::Backend {
                code: "E_STORE_DECODE".to_string(),
                message: format!("stored value in column {column} could not be decoded"),
            }
        })
    };
    let int = |column: &'static str| -> Result<i64, MarketRecordsStoreError> {
        row.try_get::<i64, _>(column).map_err(|_| {
            MarketRecordsStoreError::Backend {
                code: "E_STORE_DECODE".to_string(),
                message: format!("stored value in column {column} could not be decoded"),
            }
        })
    };
    let payload: Value = row
        .try_get::<Value, _>("event_payload")
        .map_err(|_| MarketRecordsStoreError::Backend {
            code: "E_STORE_DECODE".to_string(),
            message: "stored value in column event_payload could not be decoded".to_string(),
        })?;
    MarketEventRow::new(
        text("event_id")?,
        text("tenant_id")?,
        text("program_id")?,
        text("external_ref")?,
        text("event_kind")?,
        int("start_millis")?,
        int("end_millis")?,
        serde_json::to_string(&payload).map_err(|_| MarketRecordsStoreError::Backend {
            code: "E_STORE_DECODE".to_string(),
            message: "the stored event_payload could not be rendered".to_string(),
        })?,
        text("audit_record_id")?,
        int("recorded_at_unix_ms")?,
        text("recorded_by")?,
    )
}

fn decode_market_delivery_row(
    row: &sqlx::postgres::PgRow,
) -> Result<MarketDeliveryRow, MarketRecordsStoreError> {
    let text = |column: &'static str| -> Result<String, MarketRecordsStoreError> {
        row.try_get::<String, _>(column).map_err(|_| {
            MarketRecordsStoreError::Backend {
                code: "E_STORE_DECODE".to_string(),
                message: format!("stored value in column {column} could not be decoded"),
            }
        })
    };
    let int = |column: &'static str| -> Result<i64, MarketRecordsStoreError> {
        row.try_get::<i64, _>(column).map_err(|_| {
            MarketRecordsStoreError::Backend {
                code: "E_STORE_DECODE".to_string(),
                message: format!("stored value in column {column} could not be decoded"),
            }
        })
    };
    let energy = row
        .try_get::<f64, _>("delivered_energy_mwh")
        .map_err(|_| MarketRecordsStoreError::Backend {
            code: "E_STORE_DECODE".to_string(),
            message: "stored value in column delivered_energy_mwh could not be decoded"
                .to_string(),
        })?;
    MarketDeliveryRow::new(
        text("delivery_id")?,
        text("tenant_id")?,
        text("program_id")?,
        text("event_id")?,
        text("asset_id")?,
        int("interval_start_millis")?,
        int("interval_end_millis")?,
        energy,
        text("measured_quality")?,
        text("audit_record_id")?,
        int("recorded_at_unix_ms")?,
        text("recorded_by")?,
    )
}

fn decode_market_settlement_row(
    row: &sqlx::postgres::PgRow,
) -> Result<MarketSettlementRow, MarketRecordsStoreError> {
    let text = |column: &'static str| -> Result<String, MarketRecordsStoreError> {
        row.try_get::<String, _>(column).map_err(|_| {
            MarketRecordsStoreError::Backend {
                code: "E_STORE_DECODE".to_string(),
                message: format!("stored value in column {column} could not be decoded"),
            }
        })
    };
    let int = |column: &'static str| -> Result<i64, MarketRecordsStoreError> {
        row.try_get::<i64, _>(column).map_err(|_| {
            MarketRecordsStoreError::Backend {
                code: "E_STORE_DECODE".to_string(),
                message: format!("stored value in column {column} could not be decoded"),
            }
        })
    };
    let payload: Value = row
        .try_get::<Value, _>("statement_payload")
        .map_err(|_| MarketRecordsStoreError::Backend {
            code: "E_STORE_DECODE".to_string(),
            message: "stored value in column statement_payload could not be decoded".to_string(),
        })?;
    let settlement_id = text("settlement_id")?;
    let tenant_id = text("tenant_id")?;
    // Defence in depth: the money fields re-validate through the shared
    // shape checks even on the read path (no numeric conversion exists).
    let amount_text = text("amount_text")?;
    let currency_code = text("currency_code")?;
    validate_money_fields(&amount_text, &currency_code)?;
    MarketSettlementRow::new(
        settlement_id,
        tenant_id,
        text("program_id")?,
        text("statement_ref")?,
        text("source_format")?,
        int("period_start_millis")?,
        int("period_end_millis")?,
        text("line_item_kind")?,
        amount_text,
        currency_code,
        serde_json::to_string(&payload).map_err(|_| MarketRecordsStoreError::Backend {
            code: "E_STORE_DECODE".to_string(),
            message: "the stored statement_payload could not be rendered".to_string(),
        })?,
        row.try_get::<Option<String>, _>("supersedes_settlement_id")
            .map_err(|_| MarketRecordsStoreError::Backend {
                code: "E_STORE_DECODE".to_string(),
                message: "stored value in column supersedes_settlement_id could not be decoded"
                    .to_string(),
            })?,
        text("audit_record_id")?,
        int("recorded_at_unix_ms")?,
        text("recorded_by")?,
    )
}

fn decode_market_retention_config_row(
    row: &sqlx::postgres::PgRow,
) -> Result<MarketRetentionConfigRow, MarketRecordsStoreError> {
    let text = |column: &'static str| -> Result<String, MarketRecordsStoreError> {
        row.try_get::<String, _>(column).map_err(|_| {
            MarketRecordsStoreError::Backend {
                code: "E_STORE_DECODE".to_string(),
                message: format!("stored value in column {column} could not be decoded"),
            }
        })
    };
    let opt_text = |column: &'static str| -> Result<Option<String>, MarketRecordsStoreError> {
        row.try_get::<Option<String>, _>(column).map_err(|_| {
            MarketRecordsStoreError::Backend {
                code: "E_STORE_DECODE".to_string(),
                message: format!("stored value in column {column} could not be decoded"),
            }
        })
    };
    let opt_int = |column: &'static str| -> Result<Option<i32>, MarketRecordsStoreError> {
        row.try_get::<Option<i32>, _>(column).map_err(|_| {
            MarketRecordsStoreError::Backend {
                code: "E_STORE_DECODE".to_string(),
                message: format!("stored value in column {column} could not be decoded"),
            }
        })
    };
    MarketRetentionConfigRow::new(
        text("tenant_id")?,
        text("record_class")?,
        text("retention_state")?,
        opt_int("period_days")?,
        opt_text("basis")?,
        text("set_by")?,
        row.try_get::<i64, _>("set_at_unix_ms").map_err(|_| {
            MarketRecordsStoreError::Backend {
                code: "E_STORE_DECODE".to_string(),
                message: "stored value in column set_at_unix_ms could not be decoded".to_string(),
            }
        })?,
    )
}
