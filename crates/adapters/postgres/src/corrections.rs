// P2.31 — Versioned telemetry corrections and as-of (point-in-time) reads
//         (foundations §6.4; ledger P2.31 steps 1–4).
//
// The rule this module implements, verbatim in substance: a correction is a
// NEW versioned row, never an in-place update of the original value; a past
// decision must always remain explainable from stored data.
//
// Storage shape (migration 0012; decisions D-C1..D-C4 in
// `ledger/phase-02-asset-telemetry/25_31_exit_portfolio_corrections.md`):
//
// - A correction is a NEW ROW in the SAME `telemetry` table, carrying the
//   superseded record's storage identity legs (asset_id, event_time,
//   source_sequence, quantity), the corrected value/quality, and the
//   correction metadata (`correction_reason`, `corrected_by`, `corrected_at`,
//   `correction_of` → the original row's `telemetry_id`), distinguished by
//   `telemetry_version` (1 = original, >= 2 = corrections). The extended PK
//   `telemetry_pkey` keeps the P2.05 D-P3 semantics (partition key included)
//   and is the unique constraint that permits multiple versions of one
//   identity. The table is append-only for every runtime role (D-P6 grants:
//   INSERT/SELECT only) — no UPDATE/DELETE path exists.
// - `corrected_at` is the correction's own timestamp (when the correction
//   became known); it is the as-of visibility key for versions >= 2.
//   Version 1 is always visible.
//
// As-of semantics (ledger step 3; decision D-C4): a query naming an as-of
// time returns, per storage identity, the LATEST version whose correction
// timestamp is <= the as-of time (the original version is always visible);
// the default (no as-of) returns the latest version. Implemented as an
// anti-join against any strictly-newer version visible at the as-of
// instant — served by the extended PK index prefix (no new index needed).
//
// Late-arrival rule (ledger step 2, consistent with P2.09; decision D-C3):
// classified by [`classify_late_arrival`] — an exact duplicate is
// acknowledged (P2.09, unchanged); a late record for an interval with no
// value record yet is the FIRST record and is stored through the normal
// ingest path; a late record for an already-covered interval becomes a
// versioned correction through [`PgUnitOfWork::insert_telemetry_correction`]
// (reason + actor mandatory). WIRING, recorded honestly: the worker
// ingestion pipeline keeps the P2.09 behaviour (a conflicting duplicate
// arriving unannounced is quarantined — it must never silently become a
// correction). The correction path is an explicit, authorized entry point;
// `apps/worker` composition is outside this task's file ownership, so the
// pipeline-side call of the classifier is the recorded follow-up wiring
// (see the decision document, D-C3).
//
// The current-state view (P2.18, `telemetry.rs`
// `latest_telemetry_per_asset`) reads the latest VERSION only via the
// `telemetry_version DESC` tiebreak recorded there.

use std::sync::atomic::{AtomicU64, Ordering};

use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::quality::TelemetryQuality;
use flexcore_contracts::time_model::{EventTime, IngestTime};
use flexcore_contracts::units::ReactiveSignConvention;
use flexcore_domain::ids::AssetId;
use flexcore_domain::tenant_context::TenantContext;
use flexcore_ports::backend_error::BackendError;
use sqlx::postgres::PgRow;
use sqlx::Row;

use crate::enum_map::{quality_from_text, quality_text};
use crate::error::{codes, map_backend};
use crate::telemetry::{
    clock_quality_from_text, decode_stored_telemetry, is_telemetry_pkey_violation, q_sign_text,
    validate_record, HistoryCursor, NormalizedTelemetryRecord, Quantity, StoredTelemetry,
    TelemetryError, TelemetryValue, TELEMETRY_COLUMNS,
};
use crate::unit_of_work::{tx_ref, PgUnitOfWork};
use crate::uuid_map::uuid_text;

/// Savepoint names must be unique per transaction lifetime (the recorded
/// `telemetry.rs` rule; a second process-wide counter, same reasoning).
static CORRECTION_SAVEPOINT_COUNTER: AtomicU64 = AtomicU64::new(0);

// --- The late-arrival rule (pure, ledger step 2) ------------------------------

/// The observed storage state for one late-arriving record, as the P2.09
/// duplicate classification reports it. Pure input — no I/O, unit-testable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LateArrivalFacts {
    /// The arriving record is byte-identical to the latest stored version of
    /// its identity (the P2.09 exact-duplicate outcome).
    pub exact_duplicate_of_current: bool,
    /// The record's (asset, event_time, quantity) interval already holds a
    /// VALUE-bearing record — a measured value, not only a derived MISSING
    /// gap marker (P2.04 D-T6: markers are not values).
    pub interval_covered_by_value_record: bool,
}

/// The decision the late-arrival rule prescribes (P2.31 step 2, consistent
/// with P2.09 — decided once, recorded in the decision document D-C6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LateArrivalDecision {
    /// P2.09 exact duplicate: acknowledge, store nothing.
    AcknowledgeExactDuplicate,
    /// The interval had no value record (nothing, or only a MISSING gap
    /// marker): the late record is the FIRST value record — store it through
    /// the normal ingest path. The gap marker remains stored history; the
    /// late value never overwrites or removes it.
    StoreAsFirstValueRecord,
    /// The interval already carries a value record and the arrival differs
    /// from it: store the arrival as a VERSIONED CORRECTION through the
    /// explicit correction path (reason + actor mandatory) — never an
    /// in-place overwrite, never a silent replacement of the value a past
    /// decision used.
    RecordAsVersionedCorrection,
}

/// The late-arrival rule (P2.31 step 2). Pure: the caller supplies the
/// observed facts, the rule returns the decision. The exact-duplicate check
/// dominates (an identical re-emission is acknowledged regardless of
/// coverage); otherwise coverage decides between first-record and
/// correction.
pub fn classify_late_arrival(facts: LateArrivalFacts) -> LateArrivalDecision {
    if facts.exact_duplicate_of_current {
        LateArrivalDecision::AcknowledgeExactDuplicate
    } else if !facts.interval_covered_by_value_record {
        LateArrivalDecision::StoreAsFirstValueRecord
    } else {
        LateArrivalDecision::RecordAsVersionedCorrection
    }
}

// --- The correction record (ledger step 1) --------------------------------------

/// The storage identity of the record a correction supersedes: exactly the
/// P2.04 §8 / P2.09 identity plus the per-quantity row grain — the storage
/// PK legs (P2.05 D-P3).
#[derive(Debug, Clone, PartialEq)]
pub struct CorrectionTarget {
    pub asset_id: AssetId,
    pub event_time: EventTime,
    pub source_sequence: i64,
    pub quantity: Quantity,
}

/// One correction to store (ledger step 1): the corrected value, the
/// original record's identity, the correction reason, the actor/source, the
/// correction's own timestamp, and the moment FlexCore received it. The
/// original row is never touched.
#[derive(Debug, Clone, PartialEq)]
pub struct TelemetryCorrection {
    /// Which stored record this correction supersedes.
    pub target: CorrectionTarget,
    /// The corrected value (`None` exactly with a `MISSING` corrected
    /// quality — a retraction that marks the original value a gap; the same
    /// value-totality rules as any telemetry row apply).
    pub value: Option<TelemetryValue>,
    /// The corrected quality state (one of the six canonical states).
    pub quality: TelemetryQuality,
    /// Corrected diagnostic source code (metadata; may stay as it was).
    pub source_quality_code: Option<String>,
    /// The declared reactive sign convention — present exactly when the
    /// corrected quantity is reactive_power (P2.04 D-T4, re-checked).
    pub q_sign_convention: Option<ReactiveSignConvention>,
    /// Why the value is corrected (mandatory, non-blank).
    pub reason: String,
    /// Who/what authored the correction (mandatory, non-blank).
    pub corrected_by: String,
    /// The correction's own timestamp — when the correction became known
    /// (foundations §6.4). The as-of visibility key for versions >= 2; must
    /// not precede the corrected_at of the version it supersedes.
    pub corrected_at: IngestTime,
    /// When FlexCore received the correction (P0.09; distinct from
    /// `corrected_at`).
    pub ingest_time: IngestTime,
}

/// The outcome of one accepted correction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CorrectionOutcome {
    /// Stored as a new version (the version it received).
    Stored { version: i32 },
    /// No stored record exists for the target identity — per the recorded
    /// late-arrival rule the caller stores the record through the NORMAL
    /// ingest path instead (it is the first record for that interval, not a
    /// correction; see [`classify_late_arrival`]).
    OriginalMissing,
}

/// One stored version of a record's identity, as
/// [`PgUnitOfWork::query_telemetry_versions`] reads it back: the original
/// (version 1) stays queryable unchanged after any correction — the
/// ledger's "original intact + queryable" acceptance.
#[derive(Debug, Clone, PartialEq)]
pub struct StoredTelemetryVersion {
    pub telemetry_id: FlexUuid,
    pub version: i32,
    pub value: Option<TelemetryValue>,
    pub quality: TelemetryQuality,
    pub source_quality_code: Option<String>,
    pub source_id: String,
    /// The canonical unit (P0.07): `MW`, `MVAr`, `MWh`, or `1`.
    pub unit: String,
    /// When this version was received (P0.09).
    pub ingest_time: IngestTime,
    /// The correction timestamp; `None` exactly on version 1 (the original
    /// is not a correction).
    pub corrected_at: Option<IngestTime>,
    /// The correction reason; `None` exactly on version 1.
    pub correction_reason: Option<String>,
    /// The correction actor/source; `None` exactly on version 1.
    pub corrected_by: Option<String>,
}

// --- Errors and local helpers ---------------------------------------------------

fn backend_rejected(message: &'static str) -> TelemetryError {
    TelemetryError::Backend(BackendError::new(codes::REJECTED.to_string(), message.to_string()))
}

fn decode_failure(column: &'static str) -> TelemetryError {
    TelemetryError::Backend(crate::error::decode_failure(column))
}

fn col_str(row: &PgRow, column: &'static str) -> Result<String, TelemetryError> {
    row.try_get::<String, _>(column).map_err(|_| decode_failure(column))
}

fn col_opt_str(row: &PgRow, column: &'static str) -> Result<Option<String>, TelemetryError> {
    row.try_get::<Option<String>, _>(column).map_err(|_| decode_failure(column))
}

fn col_f64_opt(row: &PgRow, column: &'static str) -> Result<Option<f64>, TelemetryError> {
    row.try_get::<Option<f64>, _>(column).map_err(|_| decode_failure(column))
}

fn col_bool_opt(row: &PgRow, column: &'static str) -> Result<Option<bool>, TelemetryError> {
    row.try_get::<Option<bool>, _>(column).map_err(|_| decode_failure(column))
}

fn col_i32(row: &PgRow, column: &'static str) -> Result<i32, TelemetryError> {
    row.try_get::<i32, _>(column).map_err(|_| decode_failure(column))
}

fn col_ingest_time(row: &PgRow, column: &'static str) -> Result<IngestTime, TelemetryError> {
    let raw: time::OffsetDateTime = row.try_get(column).map_err(|_| decode_failure(column))?;
    IngestTime::new(raw).map_err(|_| decode_failure(column))
}

fn col_opt_ingest_time(row: &PgRow, column: &'static str) -> Result<Option<IngestTime>, TelemetryError> {
    let raw: Option<time::OffsetDateTime> = row
        .try_get(column)
        .map_err(|_| decode_failure(column))?;
    raw.map(IngestTime::new)
        .transpose()
        .map_err(|_| decode_failure(column))
}

/// Decodes one stored version row (the version-metadata column set of the
/// `query_telemetry_versions` SELECT).
fn decode_stored_version(row: &PgRow) -> Result<StoredTelemetryVersion, TelemetryError> {
    let quality = quality_from_text(&col_str(row, "quality")?).map_err(TelemetryError::Backend)?;
    Ok(StoredTelemetryVersion {
        telemetry_id: FlexUuid::parse(&col_str(row, "telemetry_id")?)
            .map_err(|_| decode_failure("telemetry.telemetry_id"))?,
        version: col_i32(row, "telemetry_version")?,
        value: if let Some(num) = col_f64_opt(row, "value_num")? {
            Some(TelemetryValue::Numeric(num))
        } else if let Some(text) = col_opt_str(row, "value_text")? {
            Some(TelemetryValue::Text(text))
        } else if let Some(flag) = col_bool_opt(row, "value_bool")? {
            Some(TelemetryValue::Flag(flag))
        } else {
            None
        },
        quality,
        source_quality_code: col_opt_str(row, "source_quality_code")?,
        source_id: col_str(row, "source_id")?,
        unit: col_str(row, "unit")?,
        ingest_time: col_ingest_time(row, "ingest_time")?,
        corrected_at: col_opt_ingest_time(row, "corrected_at")?,
        correction_reason: col_opt_str(row, "correction_reason")?,
        corrected_by: col_opt_str(row, "corrected_by")?,
    })
}

/// The as-of visibility predicate (D-C4): row `telemetry` is visible iff NO
/// strictly-newer version of the same storage identity is visible at the
/// as-of instant. `$SUPERSEDED` is replaced with the builder's as-of
/// parameter index (always bound — `NULL` means "no as-of", which lets every
/// newer version supersede). A version >= 2 superseder always carries
/// `corrected_at` (the 0012 CHECK), so the comparison is total.
const NOT_SUPERSEDED_AT_AS_OF: &str = "AND NOT EXISTS ( \
         SELECT 1 FROM telemetry sup \
         WHERE sup.asset_id = telemetry.asset_id \
           AND sup.event_time = telemetry.event_time \
           AND sup.source_sequence = telemetry.source_sequence \
           AND sup.quantity = telemetry.quantity \
           AND sup.telemetry_version > telemetry.telemetry_version \
           AND ($SUPERSEDED::timestamptz IS NULL OR sup.corrected_at <= $SUPERSEDED))";

// --- The repository on the unit of work -----------------------------------------

impl PgUnitOfWork {
    /// Stores one correction as a NEW versioned row (P2.31 steps 1–2), inside
    /// THIS unit of work's transaction. The procedure, in order:
    ///
    /// 1. The original row (version 1) of the target identity is read. If it
    ///    does not exist, [`CorrectionOutcome::OriginalMissing`] is returned —
    ///    the caller stores the record through the normal ingest path (the
    ///    late-arrival rule: an uncovered interval gets a first record, not a
    ///    correction).
    /// 2. The latest version's metadata is read; the new version is
    ///    `latest + 1`, and `corrected_at` must not precede the superseded
    ///    version's `corrected_at` (the as-of ordering must stay monotone per
    ///    identity, otherwise "latest version ≤ as-of" becomes ambiguous).
    /// 3. The corrected content is re-validated through the SAME storage
    ///    rules as any ingested record ([`validate_record`]: tenant match,
    ///    time ordering, value-kind pairing, SOC range, finiteness).
    /// 4. The new row is inserted inside a savepoint. A PK collision here
    ///    means a concurrent correction created a newer version first — a
    ///    rejected retry, never an overwrite.
    ///
    /// The original row (and every intermediate version) remains queryable
    /// unchanged ([`PgUnitOfWork::query_telemetry_versions`]).
    pub fn insert_telemetry_correction(
        &self,
        ctx: &TenantContext,
        correction: &TelemetryCorrection,
    ) -> Result<CorrectionOutcome, TelemetryError> {
        if correction.reason.trim().is_empty() || correction.corrected_by.trim().is_empty() {
            return Err(backend_rejected(
                "a correction must carry a non-blank reason and actor (P2.31 step 1)",
            ));
        }
        let is_reactive = correction.target.quantity == Quantity::ReactivePower;
        if is_reactive != correction.q_sign_convention.is_some() {
            return Err(backend_rejected(
                "a reactive_power correction must carry its declared q_sign_convention, and no \
                 other quantity may carry one (P2.04 D-T4)",
            ));
        }
        let tenant_text = uuid_text(ctx.tenant_id().uuid());
        let asset_text = uuid_text(correction.target.asset_id.uuid());
        let quantity_text = correction.target.quantity.db_text();
        let event_time = correction.target.event_time.timestamp();
        let sequence = correction.target.source_sequence;
        let unit_text = correction.target.quantity.canonical_unit();
        let (value_num, value_text, value_bool) = match &correction.value {
            Some(TelemetryValue::Numeric(n)) => (Some(*n), None, None),
            Some(TelemetryValue::Text(text)) => (None, Some(text.clone()), None),
            Some(TelemetryValue::Flag(flag)) => (None, None, Some(*flag)),
            None => (None, None, None),
        };
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard).await.map_err(TelemetryError::Backend)?;

            // 1. The original (version 1) must exist and belong to the caller.
            let original = sqlx::query(
                "SELECT telemetry_id::text AS telemetry_id, tenant_id::text AS tenant_id, \
                        source_id, clock_quality::text AS clock_quality \
                 FROM telemetry \
                 WHERE asset_id = $1::uuid AND event_time = $2 \
                   AND source_sequence = $3 AND quantity = $4 AND telemetry_version = 1",
            )
            .bind(&asset_text)
            .bind(event_time)
            .bind(sequence)
            .bind(quantity_text)
            .fetch_optional(&mut *tx)
            .await
            .map_err(map_backend)
            .map_err(TelemetryError::Backend)?;
            let original = match original {
                Some(row) => row,
                None => return Ok(CorrectionOutcome::OriginalMissing),
            };
            let original_tenant = col_str(&original, "tenant_id")?;
            if original_tenant != tenant_text {
                return Err(TelemetryError::TenantMismatch);
            }
            let original_id = col_str(&original, "telemetry_id")?;
            let source_id = col_str(&original, "source_id")?;
            let clock_quality_raw = col_str(&original, "clock_quality")?;

            // 2. The latest version and its corrected_at (as-of ordering).
            let latest = sqlx::query(
                "SELECT telemetry_version, corrected_at \
                 FROM telemetry \
                 WHERE asset_id = $1::uuid AND event_time = $2 \
                   AND source_sequence = $3 AND quantity = $4 \
                 ORDER BY telemetry_version DESC LIMIT 1",
            )
            .bind(&asset_text)
            .bind(event_time)
            .bind(sequence)
            .bind(quantity_text)
            .fetch_one(&mut *tx)
            .await
            .map_err(map_backend)
            .map_err(TelemetryError::Backend)?;
            let latest_version = col_i32(&latest, "telemetry_version")?;
            let latest_corrected_at: Option<time::OffsetDateTime> = latest
                .try_get("corrected_at")
                .map_err(|_| decode_failure("telemetry.corrected_at"))?;
            if let Some(prev) = latest_corrected_at {
                if correction.corrected_at.timestamp() < prev {
                    return Err(backend_rejected(
                        "a correction's corrected_at must not precede the corrected_at of the \
                         version it supersedes (the per-identity as-of ordering must stay \
                         monotone)",
                    ));
                }
            }
            let new_version = latest_version + 1;

            // 3. The corrected content passes the identical record rules.
            let clock_quality = clock_quality_from_text(&clock_quality_raw)
                .map_err(TelemetryError::Backend)?;
            let record = NormalizedTelemetryRecord {
                tenant_id: ctx.tenant_id(),
                asset_id: correction.target.asset_id,
                quantity: correction.target.quantity,
                value: correction.value.clone(),
                event_time: correction.target.event_time,
                ingest_time: correction.ingest_time,
                source_sequence: correction.target.source_sequence,
                source_id: source_id.clone(),
                quality: correction.quality,
                source_quality_code: correction.source_quality_code.clone(),
                q_sign_convention: correction.q_sign_convention,
                clock_quality,
            };
            validate_record(ctx, &record)?;

            // 4. The versioned insert, savepoint-guarded (the surrounding
            // unit-of-work transaction stays usable on a classified failure).
            let savepoint = format!(
                "telemetry_correction_{}",
                CORRECTION_SAVEPOINT_COUNTER.fetch_add(1, Ordering::SeqCst)
            );
            sqlx::query(&format!("SAVEPOINT {savepoint}"))
                .execute(&mut *tx)
                .await
                .map_err(map_backend)
                .map_err(TelemetryError::Backend)?;
            let insert = sqlx::query(
                "INSERT INTO telemetry \
                     (tenant_id, asset_id, quantity, event_time, ingest_time, source_sequence, \
                      source_id, quality, source_quality_code, value_num, value_text, \
                      value_bool, unit, q_sign_convention, clock_quality, telemetry_version, \
                      correction_of, correction_reason, corrected_by, corrected_at) \
                 VALUES ($1::uuid, $2::uuid, $3, $4, $5, $6, $7, $8::telemetry_quality, $9, \
                         $10, $11, $12, $13, $14::reactive_sign_convention, $15::clock_quality, \
                         $16, $17::uuid, $18, $19, $20)",
            )
            .bind(&tenant_text)
            .bind(&asset_text)
            .bind(quantity_text)
            .bind(event_time)
            .bind(correction.ingest_time.timestamp())
            .bind(sequence)
            .bind(&source_id)
            .bind(quality_text(correction.quality))
            .bind(&correction.source_quality_code)
            .bind(value_num)
            .bind(value_text)
            .bind(value_bool)
            .bind(unit_text)
            .bind(correction.q_sign_convention.map(q_sign_text))
            .bind(&clock_quality_raw)
            .bind(new_version)
            .bind(&original_id)
            .bind(&correction.reason)
            .bind(&correction.corrected_by)
            .bind(correction.corrected_at.timestamp())
            .execute(&mut *tx)
            .await;
            match insert {
                Ok(_) => {
                    sqlx::query(&format!("RELEASE SAVEPOINT {savepoint}"))
                        .execute(&mut *tx)
                        .await
                        .map_err(map_backend)
                        .map_err(TelemetryError::Backend)?;
                    Ok(CorrectionOutcome::Stored { version: new_version })
                }
                Err(err) => {
                    sqlx::query(&format!("ROLLBACK TO SAVEPOINT {savepoint}"))
                        .execute(&mut *tx)
                        .await
                        .map_err(map_backend)
                        .map_err(TelemetryError::Backend)?;
                    let classified = if is_telemetry_pkey_violation(&err) {
                        // A concurrent correction created a newer version of
                        // the same identity first: the caller retries against
                        // the latest version. Never an overwrite.
                        Some(backend_rejected(
                            "a concurrent correction created a newer version of the same record; \
                             retry against the latest version",
                        ))
                    } else {
                        None
                    };
                    sqlx::query(&format!("RELEASE SAVEPOINT {savepoint}"))
                        .execute(&mut *tx)
                        .await
                        .map_err(map_backend)
                        .map_err(TelemetryError::Backend)?;
                    match classified {
                        Some(rejection) => Err(rejection),
                        None => Err(TelemetryError::Backend(map_backend(err))),
                    }
                }
            }
        })
    }

    /// Every stored version of one record's identity, ascending by version
    /// (P2.31 acceptance: the original row remains queryable after a
    /// correction — version 1 is always present in the result of a
    /// corrected record). Tenant-scoped through the unit of work's RLS GUC
    /// plus the explicit tenant filter (defence in depth, P1.13 pattern).
    pub fn query_telemetry_versions(
        &self,
        ctx: &TenantContext,
        target: &CorrectionTarget,
    ) -> Result<Vec<StoredTelemetryVersion>, TelemetryError> {
        let tenant_text = uuid_text(ctx.tenant_id().uuid());
        let asset_text = uuid_text(target.asset_id.uuid());
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard).await.map_err(TelemetryError::Backend)?;
            let rows = sqlx::query(
                "SELECT telemetry_id::text AS telemetry_id, telemetry_version, source_id, \
                        quality::text AS quality, source_quality_code, value_num, value_text, \
                        value_bool, unit, ingest_time, corrected_at, correction_reason, \
                        corrected_by \
                 FROM telemetry \
                 WHERE tenant_id = $1::uuid AND asset_id = $2::uuid AND event_time = $3 \
                   AND source_sequence = $4 AND quantity = $5 \
                 ORDER BY telemetry_version ASC",
            )
            .bind(&tenant_text)
            .bind(&asset_text)
            .bind(target.event_time.timestamp())
            .bind(target.source_sequence)
            .bind(target.quantity.db_text())
            .fetch_all(&mut *tx)
            .await
            .map_err(map_backend)
            .map_err(TelemetryError::Backend)?;
            rows.iter().map(decode_stored_version).collect()
        })
    }

    /// The as-of aware multi-asset range read (P2.31 step 3): per storage
    /// identity, the latest version whose correction timestamp is <= `as_of`;
    /// `as_of = None` returns the latest version (the default). The original
    /// version is always visible — an as-of view never un-sees the fact that
    /// a record existed; it decides WHICH version was current. Ordered
    /// descending by event_time (the P2.06 `query_telemetry` ordering);
    /// bounded by `limit` (1.., the caller's page-size rule).
    #[allow(clippy::too_many_arguments)]
    pub fn query_telemetry_as_of(
        &self,
        ctx: &TenantContext,
        assets: &[AssetId],
        from: EventTime,
        until: EventTime,
        quantities: Option<&[Quantity]>,
        as_of: Option<IngestTime>,
        limit: u32,
    ) -> Result<Vec<StoredTelemetry>, TelemetryError> {
        if from.timestamp() > until.timestamp() {
            return Err(backend_rejected(
                "the query time range is inverted (from after until)",
            ));
        }
        if assets.is_empty() {
            return Err(backend_rejected("a telemetry query must name at least one asset"));
        }
        if let Some(quantities) = quantities {
            if quantities.is_empty() {
                return Err(backend_rejected(
                    "a telemetry query must name at least one quantity (no implicit wildcard)",
                ));
            }
        }
        if limit == 0 {
            return Err(backend_rejected("a page must request at least one record"));
        }
        let asset_texts: Vec<String> = assets
            .iter()
            .map(|asset| uuid_text(asset.uuid()))
            .collect();
        let tenant_text = uuid_text(ctx.tenant_id().uuid());
        let quantity_texts: Option<Vec<&'static str>> =
            quantities.map(|set| set.iter().map(|q| q.db_text()).collect());
        // Parameter indices: 1 tenant, 2 assets, 3 from, 4 until, 5 as_of
        // (always bound, possibly NULL); optional quantities and LIMIT
        // follow.
        let mut next_param = 6usize;
        let mut sql = format!(
            "{TELEMETRY_COLUMNS} WHERE tenant_id = $1::uuid \
             AND asset_id = ANY($2::uuid[]) \
             AND event_time >= $3 AND event_time <= $4 {}",
            NOT_SUPERSEDED_AT_AS_OF.replace("$SUPERSEDED", "$5")
        );
        if quantity_texts.is_some() {
            sql.push_str(&format!(" AND quantity = ANY(${next_param}::text[])"));
            next_param += 1;
        }
        sql.push_str(" ORDER BY event_time DESC, asset_id, quantity, source_sequence LIMIT $");
        sql.push_str(&next_param.to_string());

        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard).await.map_err(TelemetryError::Backend)?;
            let mut query = sqlx::query(&sql)
                .bind(&tenant_text)
                .bind(&asset_texts)
                .bind(from.timestamp())
                .bind(until.timestamp())
                .bind(as_of.map(|t| t.timestamp()));
            if let Some(texts) = &quantity_texts {
                query = query.bind(texts);
            }
            query = query.bind(i64::from(limit));
            let rows = query
                .fetch_all(&mut *tx)
                .await
                .map_err(map_backend)
                .map_err(TelemetryError::Backend)?;
            rows.iter().map(decode_stored_telemetry).collect()
        })
    }

    /// The as-of aware historical raw query (P2.31 step 3 on the P2.19
    /// route): exactly the D-H1 keyset-paginated history query with the
    /// version-visibility predicate applied — per storage identity, the
    /// latest version visible at `as_of` (`None` = latest version), so a
    /// page never shows a superseded value next to its correction. The
    /// cursor, ordering, limits, and comparison fields are the P2.19 ones
    /// unchanged (D-H1/D-H3; `ingest_time` stays metadata).
    #[allow(clippy::too_many_arguments)]
    pub fn query_telemetry_history_as_of(
        &self,
        ctx: &TenantContext,
        asset: AssetId,
        from: EventTime,
        until: EventTime,
        quantities: Option<&[Quantity]>,
        quality: Option<TelemetryQuality>,
        limit: u32,
        cursor: Option<HistoryCursor>,
        as_of: Option<IngestTime>,
    ) -> Result<(Vec<StoredTelemetry>, Option<HistoryCursor>), TelemetryError> {
        if from.timestamp() > until.timestamp() {
            return Err(backend_rejected(
                "the query time range is inverted (from after until)",
            ));
        }
        if limit == 0 {
            return Err(backend_rejected(
                "a page must request at least one record",
            ));
        }
        if let Some(quantities) = quantities {
            if quantities.is_empty() {
                return Err(backend_rejected(
                    "a telemetry query must name at least one quantity (no implicit wildcard)",
                ));
            }
        }
        let tenant_text = uuid_text(ctx.tenant_id().uuid());
        let asset_text = uuid_text(asset.uuid());
        let quantity_texts: Option<Vec<&'static str>> =
            quantities.map(|set| set.iter().map(|q| q.db_text()).collect());
        let quality_text = quality.map(quality_text);

        // Parameter indices assigned as clauses are appended (the recorded
        // `query_telemetry_history` pattern): 1 tenant, 2 asset, 3 from,
        // 4 until, 5 as_of (always bound); optional quantities, quality,
        // cursor legs, and LIMIT follow.
        let mut next_param = 6usize;
        let mut sql = format!(
            "{TELEMETRY_COLUMNS} WHERE tenant_id = $1::uuid \
             AND asset_id = $2::uuid \
             AND event_time >= $3 AND event_time <= $4 {}",
            NOT_SUPERSEDED_AT_AS_OF.replace("$SUPERSEDED", "$5")
        );
        if quantity_texts.is_some() {
            sql.push_str(&format!(" AND quantity = ANY(${next_param}::text[])"));
            next_param += 1;
        }
        if quality_text.is_some() {
            sql.push_str(&format!(" AND quality::text = ${next_param}"));
            next_param += 1;
        }
        if cursor.is_some() {
            let (p, q, r) = (next_param, next_param + 1, next_param + 2);
            next_param += 3;
            // Row-wise strictly-after keyset comparison over the PK's
            // varying legs (D-H1), identical to the P2.19 query.
            sql.push_str(&format!(
                " AND (event_time, source_sequence, quantity) > \
                 (${p}::timestamptz, ${q}::bigint, ${r}::text)"
            ));
        }
        sql.push_str(" ORDER BY event_time ASC, source_sequence ASC, quantity ASC LIMIT $");
        sql.push_str(&next_param.to_string());

        let fetch = i64::from(limit) + 1; // one extra row detects a next page
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard).await.map_err(TelemetryError::Backend)?;
            let mut query = sqlx::query(&sql)
                .bind(&tenant_text)
                .bind(&asset_text)
                .bind(from.timestamp())
                .bind(until.timestamp())
                .bind(as_of.map(|t| t.timestamp()));
            if let Some(texts) = &quantity_texts {
                query = query.bind(texts);
            }
            if let Some(text) = &quality_text {
                query = query.bind(text);
            }
            if let Some(cursor) = &cursor {
                let (millis, sequence, quantity) = cursor.legs();
                let cursor_time = time::OffsetDateTime::from_unix_timestamp_millis(millis)
                    .map_err(|_| decode_failure("telemetry.cursor"))?;
                query = query
                    .bind(cursor_time)
                    .bind(sequence)
                    .bind(quantity.db_text());
            }
            query = query.bind(fetch);
            let rows = query
                .fetch_all(&mut *tx)
                .await
                .map_err(map_backend)
                .map_err(TelemetryError::Backend)?;
            let has_more = rows.len() as u64 > u64::from(limit);
            let take = rows.len().min(limit as usize);
            let mut records = Vec::with_capacity(take);
            for row in &rows[..take] {
                records.push(decode_stored_telemetry(row)?);
            }
            let next_cursor = if has_more {
                records.last().map(|r| HistoryCursor::for_record(r))
            } else {
                None
            };
            Ok((records, next_cursor))
        })
    }
}

// --- A small self-check -----------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_late_arrival_rule_is_the_recorded_decision_table() {
        // Exact duplicate dominates: acknowledged, nothing stored (P2.09).
        assert_eq!(
            classify_late_arrival(LateArrivalFacts {
                exact_duplicate_of_current: true,
                interval_covered_by_value_record: true,
            }),
            LateArrivalDecision::AcknowledgeExactDuplicate
        );
        assert_eq!(
            classify_late_arrival(LateArrivalFacts {
                exact_duplicate_of_current: true,
                interval_covered_by_value_record: false,
            }),
            LateArrivalDecision::AcknowledgeExactDuplicate
        );
        // Uncovered interval (nothing or only a MISSING marker): the late
        // record is the FIRST value record — normal ingest path.
        assert_eq!(
            classify_late_arrival(LateArrivalFacts {
                exact_duplicate_of_current: false,
                interval_covered_by_value_record: false,
            }),
            LateArrivalDecision::StoreAsFirstValueRecord
        );
        // Already covered with a different value: a versioned correction,
        // never an overwrite (P2.31 step 2 / foundations §6.4).
        assert_eq!(
            classify_late_arrival(LateArrivalFacts {
                exact_duplicate_of_current: false,
                interval_covered_by_value_record: true,
            }),
            LateArrivalDecision::RecordAsVersionedCorrection
        );
    }

    #[test]
    fn the_visibility_predicate_names_the_bound_parameter_once_per_occurrence() {
        // The predicate is a constant; the as-of parameter it references must
        // be the documented one so the SQL builders bind it correctly.
        assert_eq!(NOT_SUPERSEDED_AT_AS_OF.matches("$SUPERSEDED").count(), 2);
        assert!(NOT_SUPERSEDED_AT_AS_OF.contains("sup.telemetry_version > telemetry.telemetry_version"));
    }
}
