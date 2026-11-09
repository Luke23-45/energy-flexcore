// P2.06 — The telemetry repository on the Postgres adapter: insert (single +
// batch), query by asset + time range, latest-value-per-asset, and the
// telemetry-quarantine write/list path — all tenant-scoped via the unit of
// work's transaction and `TenantContext`.
//
// Storage shape (P2.05 `05_telemetry_storage_design.md`, D-P1..D-P7): one
// partitioned `telemetry` table (monthly RANGE partitions on `event_time`),
// PK `(asset_id, event_time, source_sequence, quantity)` (D-P3 — includes
// the partition key), composite DESC indexes, RLS with `FORCE` and the
// P1.12 §5.1 tenant policy, and the `telemetry_quarantine` evidence table
// (P2.07 §3).
//
// Record type (recorded alignment obligation): this module carries a
// **crate-local** `NormalizedTelemetryRecord` mirroring the P2.04 §3 field
// set, not the `flexcore_ports::telemetry_source::NormalizedTelemetry` type
// — the P2.11 port contract is being implemented concurrently and its type
// name/shape may shift; the adapter must not gate on it. The mapping is
// 1:1 (recorded once, P2.04 §3 field numbers):
//
//   ports `NormalizedTelemetry`          → `NormalizedTelemetryRecord`
//   tenant_id / asset_id                 → tenant_id / asset_id (fields 2–3)
//   quantity: TelemetryQuantity          → quantity: Quantity (field 4; the
//                                          crate-local `Quantity` enum maps
//                                          1:1 onto the frozen P2.04 §4
//                                          keys and the ports enum)
//   value: Option<QuantityValue>         → value: Option<TelemetryValue>
//                                          (field 11–13; the unit and the
//                                          reactive convention are carried
//                                          explicitly: `unit` is derived
//                                          from the quantity per P2.04 §4,
//                                          and a reactive record must carry
//                                          `q_sign_convention`, field 15)
//   event_time / ingest_time             → event_time: EventTime /
//                                          ingest_time: IngestTime (fields
//                                          5–6; distinct contract types)
//   source_sequence: u64                 → source_sequence: i64 (field 7;
//                                          `bigint` at storage, P2.04 §10)
//   source_id                            → source_id (field 8)
//   quality                              → quality: TelemetryQuality (9)
//   source_quality_code                  → source_quality_code (10)
//   clock_quality                        → clock_quality (16)
//
// P2.11 alignment obligation: whoever lands the P2.11 ingest path maps
// `NormalizedTelemetry` onto this record (a pure field-for-field
// conversion) or replaces this record with the port type. No field of the
// P2.04 §3 set is renamed or dropped here, so the conversion is mechanical.
//
// Duplicate handling (P2.06 step 3, P2.09 §5 of the storage design): the
// insert is a plain INSERT — no `ON CONFLICT`. A PK collision is classified
// inside a SAVEPOINT so the surrounding unit-of-work transaction stays
// usable:
//
//   - exact duplicate (every value-bearing and identity field of the
//     colliding row is identical) → `ExactDuplicateAcknowledged`, no second
//     row (P2.09: acknowledged, not stored twice);
//   - conflicting duplicate (same identity, different content) →
//     `TelemetryError::ConflictingDuplicate` — never silently overwritten;
//     the CALLER (P2.10 pipeline) quarantines it via
//     [`quarantine_telemetry`] with a reason.
//
// An event_time outside every existing partition is a PostgreSQL routing
// error (SQLSTATE 23514 raised with no constraint name — recorded
// assumption, verified in the partition tests); the repository maps it to
// `TelemetryError::OutOfRangeEventTime` so the pipeline quarantines it
// (P2.05 §7 item 2; partition pre-creation is the P2.20 job).
//
// No retention value, period, or TTL rule exists anywhere in this module
// (D-P5; P1.12 D-S3).

use std::sync::atomic::{AtomicU64, Ordering};

use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::quality::TelemetryQuality;
use flexcore_contracts::time_model::{ClockQuality, EventTime, IngestTime};
use flexcore_contracts::units::ReactiveSignConvention;
use flexcore_domain::ids::{AssetId, TenantId};
use flexcore_domain::tenant_context::TenantContext;
use flexcore_ports::backend_error::BackendError;
use sqlx::postgres::PgRow;
use sqlx::Row;

use crate::enum_map::{quality_from_text, quality_text};
use crate::error::{codes, map_backend};
use crate::unit_of_work::{tx_ref, PgUnitOfWork, Tx};
use crate::uuid_map::{uuid_from_text, uuid_text};

/// Savepoint names must be unique per transaction lifetime; a process-wide
/// counter is enough (names only need to differ within one transaction, and
/// a transaction cannot outlive the process).
static SAVEPOINT_COUNTER: AtomicU64 = AtomicU64::new(0);

// --- Vocabulary mappings (crate-local, frozen P2.04 §4 keys) ------------------

/// The frozen Phase 2 quantity vocabulary (P2.04 §4; extension is an
/// explicit migration, never a silent widening — the database CHECK and
/// this enum are both frozen to the same nine keys). Mirrors
/// `flexcore_ports::telemetry_source::TelemetryQuantity` 1:1 (see the
/// module-header mapping table).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Quantity {
    ActivePower,
    ReactivePower,
    Soc,
    Energy,
    Availability,
    OperatingMode,
    Health,
    ConnectionState,
    BaselineReference,
}

impl Quantity {
    /// The `telemetry.quantity` column value.
    pub fn db_text(self) -> &'static str {
        match self {
            Quantity::ActivePower => "active_power",
            Quantity::ReactivePower => "reactive_power",
            Quantity::Soc => "soc",
            Quantity::Energy => "energy",
            Quantity::Availability => "availability",
            Quantity::OperatingMode => "operating_mode",
            Quantity::Health => "health",
            Quantity::ConnectionState => "connection_state",
            Quantity::BaselineReference => "baseline_reference",
        }
    }

    /// The column value as the vocabulary. An unknown key is a decode
    /// failure (the CHECK constraint makes one impossible from rows written
    /// by this schema's own writers).
    pub fn from_db_text(raw: &str) -> Result<Self, BackendError> {
        match raw {
            "active_power" => Ok(Quantity::ActivePower),
            "reactive_power" => Ok(Quantity::ReactivePower),
            "soc" => Ok(Quantity::Soc),
            "energy" => Ok(Quantity::Energy),
            "availability" => Ok(Quantity::Availability),
            "operating_mode" => Ok(Quantity::OperatingMode),
            "health" => Ok(Quantity::Health),
            "connection_state" => Ok(Quantity::ConnectionState),
            "baseline_reference" => Ok(Quantity::BaselineReference),
            _ => Err(crate::error::decode_failure("telemetry.quantity")),
        }
    }

    /// The canonical unit of the quantity (P2.04 §4; P0.07 §1). The `unit`
    /// column is derived from the quantity at insert — a record never
    /// carries a unit that disagrees with its quantity.
    pub fn canonical_unit(self) -> &'static str {
        match self {
            Quantity::ActivePower | Quantity::BaselineReference => "MW",
            Quantity::ReactivePower => "MVAr",
            Quantity::Energy => "MWh",
            Quantity::Soc
            | Quantity::Availability
            | Quantity::OperatingMode
            | Quantity::Health
            | Quantity::ConnectionState => "1",
        }
    }

    /// The value kind the quantity's row must carry (P2.04 §4).
    fn expects_numeric(self) -> bool {
        matches!(
            self,
            Quantity::ActivePower
                | Quantity::ReactivePower
                | Quantity::Soc
                | Quantity::Energy
                | Quantity::BaselineReference
        )
    }

    fn expects_bool(self) -> bool {
        matches!(self, Quantity::Availability | Quantity::ConnectionState)
    }
}

/// The value of one record, one of the P2.04 §3 value kinds (numeric /
/// state text / boolean). The reactive sign convention is NOT folded into
/// the value: it travels as its own record field (`q_sign_convention`,
/// field 15), exactly as it travels as its own column.
#[derive(Debug, Clone, PartialEq)]
pub enum TelemetryValue {
    /// A numeric value in the record's canonical unit (finite — non-finite
    /// values are rejected before any SQL runs).
    Numeric(f64),
    /// A source-declared text state carried verbatim, non-empty (field 12).
    Text(String),
    /// A boolean state value (field 13).
    Flag(bool),
}

// --- Errors and outcomes ------------------------------------------------------

/// The typed errors of the telemetry repository (P2.06 step 3). Storage
/// failures that are not one of the named classes surface as
/// `Backend` with an opaque code (crate error discipline — no raw storage
/// text ever leaves this module).
#[derive(Debug)]
pub enum TelemetryError {
    /// The record's tenant disagrees with the unit of work's
    /// `TenantContext` — refused before any SQL runs (P1.07 obligation).
    TenantMismatch,
    /// The colliding row has the same identity but different content —
    /// a conflicting duplicate (P2.09). Never overwritten; the caller
    /// quarantines the record with a reason.
    ConflictingDuplicate,
    /// The record's `event_time` falls outside every existing monthly
    /// partition — the P2.05 §7 item 2 out-of-range path. The caller
    /// quarantines and counts it; partition pre-creation is P2.20.
    OutOfRangeEventTime,
    /// Any other storage failure, opaque (stable code + fixed class
    /// message).
    Backend(BackendError),
}

impl std::fmt::Display for TelemetryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TelemetryError::TenantMismatch => {
                write!(f, "the telemetry record belongs to a different tenant than the caller's context")
            }
            TelemetryError::ConflictingDuplicate => {
                write!(f, "a telemetry record with the same identity but different content already exists (conflicting duplicate)")
            }
            TelemetryError::OutOfRangeEventTime => {
                write!(f, "the record's event_time falls outside every existing telemetry partition")
            }
            TelemetryError::Backend(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for TelemetryError {}

fn backend_rejected(message: &'static str) -> TelemetryError {
    TelemetryError::Backend(BackendError::new(codes::REJECTED.to_string(), message.to_string()))
}

/// The outcome of one accepted insert.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TelemetryInsertOutcome {
    /// Stored as a new row.
    Stored,
    /// An exact duplicate was acknowledged (P2.09): the colliding row is
    /// byte-identical in every value-bearing field, so no second row was
    /// written.
    ExactDuplicateAcknowledged,
}

/// Per-record result of a batch insert.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TelemetryBatchOutcome {
    Stored,
    ExactDuplicateAcknowledged,
    /// Conflicting duplicate — the caller quarantines this record.
    ConflictingDuplicate,
    /// `event_time` outside every partition — the caller quarantines and
    /// counts it (P2.05 §7 item 2).
    OutOfRangeEventTime,
}

/// One batch member's index (into the caller's slice) and its outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TelemetryBatchItem {
    pub index: usize,
    pub outcome: TelemetryBatchOutcome,
}

// --- The crate-local canonical record -----------------------------------------

/// The canonical telemetry record (P2.04 §3 field set) this repository
/// persists. Crate-local on purpose — see the module header for the 1:1
/// mapping to `flexcore_ports::telemetry_source::NormalizedTelemetry` and
/// the recorded alignment obligation.
#[derive(Debug, Clone, PartialEq)]
pub struct NormalizedTelemetryRecord {
    /// The asset's tenant (field 2; P0.16 rule 1). Proven consistent with
    /// `asset_id` by the composite FK, and re-checked against the caller's
    /// `TenantContext` before any SQL runs.
    pub tenant_id: TenantId,
    /// The measured asset (field 3).
    pub asset_id: AssetId,
    /// The quantity this record carries (field 4; D-T2 — one quantity per
    /// record).
    pub quantity: Quantity,
    /// The canonical value, or `None` exactly for a `MISSING` gap marker
    /// (fields 11–13; D-T6: derived records).
    pub value: Option<TelemetryValue>,
    /// When the measurement occurred (field 5; P0.09).
    pub event_time: EventTime,
    /// When FlexCore received it (field 6; P0.09 — distinct from
    /// `event_time`, re-checked `ingest_time >= event_time` here).
    pub ingest_time: IngestTime,
    /// Source-assigned ordering identifier (field 7; `bigint` at storage,
    /// P2.04 §10).
    pub source_sequence: i64,
    /// Source identity (field 8). The `flexcore:` namespace is reserved for
    /// platform-derived records (D-T6); the namespace rule itself is
    /// enforced at the record-construction boundary (P2.11), not here.
    pub source_id: String,
    /// Exactly one of the six canonical quality states (field 9; P0.08).
    pub quality: TelemetryQuality,
    /// Original richer source state as diagnostic metadata (field 10).
    pub source_quality_code: Option<String>,
    /// The declared reactive sign convention (field 15; D-T4) — present
    /// exactly when `quantity == Quantity::ReactivePower`, re-checked here.
    pub q_sign_convention: Option<ReactiveSignConvention>,
    /// Recorded synchronisation status (field 16; D-T5 — explicit
    /// `UNKNOWN`, never assumed).
    pub clock_quality: ClockQuality,
}

/// A stored telemetry row as this repository reads it back (the same field
/// set plus the surrogate `telemetry_id`, P2.04 D-T3, and the proven
/// `tenant_id`).
#[derive(Debug, Clone, PartialEq)]
pub struct StoredTelemetry {
    pub telemetry_id: FlexUuid,
    pub tenant_id: TenantId,
    pub asset_id: AssetId,
    pub quantity: Quantity,
    pub value: Option<TelemetryValue>,
    pub event_time: EventTime,
    pub ingest_time: IngestTime,
    pub source_sequence: i64,
    pub source_id: String,
    pub quality: TelemetryQuality,
    pub source_quality_code: Option<String>,
    pub q_sign_convention: Option<ReactiveSignConvention>,
    pub clock_quality: ClockQuality,
}

// --- Quarantine (P2.07 §3) ------------------------------------------------------

/// A quarantine entry to record. The caller (P2.10 pipeline stage) supplies
/// the stage, the rule that rejected the record, and the raw payload as
/// evidence; the tenant is always the authenticated source context's tenant.
#[derive(Debug, Clone, PartialEq)]
pub struct TelemetryQuarantineEntry {
    pub tenant_id: TenantId,
    /// If resolvable at the failed stage.
    pub asset_id: Option<AssetId>,
    /// If present on the rejected record.
    pub event_time: Option<EventTime>,
    pub source_id: Option<String>,
    pub source_sequence: Option<i64>,
    /// The pipeline stage that rejected the record.
    pub stage: String,
    /// The rule that rejected it (human-safe; never raw backend text).
    pub reason: String,
    /// The stage evidence (JSONB).
    pub raw_payload: serde_json::Value,
    /// Retry attempt count (0 on first quarantine).
    pub attempt_count: i32,
}

/// A stored quarantine row.
#[derive(Debug, Clone, PartialEq)]
pub struct StoredQuarantineEntry {
    pub quarantine_id: FlexUuid,
    pub entry: TelemetryQuarantineEntry,
    pub quarantined_at: EventTime,
}

// --- SQL -----------------------------------------------------------------------

/// P2.31: shared with `corrections.rs` (the as-of read variants select the
/// same column set).
pub(crate) const TELEMETRY_COLUMNS: &str = "SELECT telemetry_id::text AS telemetry_id, \
     tenant_id::text AS tenant_id, asset_id::text AS asset_id, quantity, \
     event_time, ingest_time, source_sequence, source_id, quality::text AS quality, \
     source_quality_code, value_num, value_text, value_bool, unit, \
     q_sign_convention::text AS q_sign_convention, clock_quality::text AS clock_quality \
     FROM telemetry";

const QUARANTINE_COLUMNS: &str = "SELECT quarantine_id::text AS quarantine_id, \
     tenant_id::text AS tenant_id, asset_id::text AS asset_id, event_time, \
     source_id, source_sequence, stage, reason, raw_payload, quarantined_at, \
     attempt_count FROM telemetry_quarantine";

// --- Row decoding ---------------------------------------------------------------
//
// Self-contained decode helpers (the unit_of_work helpers return the
// repository port error, which this module's typed error does not carry):
// a failed decode is a `Backend` failure with the stable decode code.

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

fn col_i64(row: &PgRow, column: &'static str) -> Result<i64, TelemetryError> {
    row.try_get::<i64, _>(column).map_err(|_| decode_failure(column))
}

fn col_opt_i64(row: &PgRow, column: &'static str) -> Result<Option<i64>, TelemetryError> {
    row.try_get::<Option<i64>, _>(column).map_err(|_| decode_failure(column))
}

fn col_uuid(row: &PgRow, column: &'static str) -> Result<FlexUuid, TelemetryError> {
    let raw = col_str(row, column)?;
    uuid_from_text(&raw).map_err(|_| decode_failure(column))
}

fn col_event_time(row: &PgRow, column: &'static str) -> Result<EventTime, TelemetryError> {
    let raw: time::OffsetDateTime = row.try_get(column).map_err(|_| decode_failure(column))?;
    EventTime::new(raw).map_err(|_| decode_failure(column))
}

fn col_ingest_time(row: &PgRow, column: &'static str) -> Result<IngestTime, TelemetryError> {
    let raw: time::OffsetDateTime = row.try_get(column).map_err(|_| decode_failure(column))?;
    IngestTime::new(raw).map_err(|_| decode_failure(column))
}

fn col_opt_event_time(row: &PgRow, column: &'static str) -> Result<Option<EventTime>, TelemetryError> {
    let raw: Option<time::OffsetDateTime> = row
        .try_get(column)
        .map_err(|_| decode_failure(column))?;
    raw.map(EventTime::new)
        .transpose()
        .map_err(|_| decode_failure(column))
}

/// P2.31: shared with `corrections.rs` (the correction insert binds the
/// same enum spellings).
pub(crate) fn clock_quality_text(clock_quality: ClockQuality) -> &'static str {
    match clock_quality {
        ClockQuality::Synchronized => "SYNCHRONIZED",
        ClockQuality::Unsynchronized => "UNSYNCHRONIZED",
        ClockQuality::Unknown => "UNKNOWN",
    }
}

/// P2.31: shared with `corrections.rs` (the correction insert re-validates
/// the corrected record through the shared record shape).
pub(crate) fn clock_quality_from_text(raw: &str) -> Result<ClockQuality, BackendError> {
    match raw {
        "SYNCHRONIZED" => Ok(ClockQuality::Synchronized),
        "UNSYNCHRONIZED" => Ok(ClockQuality::Unsynchronized),
        "UNKNOWN" => Ok(ClockQuality::Unknown),
        _ => Err(crate::error::decode_failure("telemetry.clock_quality")),
    }
}

/// P2.31: shared with `corrections.rs` (the correction insert binds the
/// same enum spellings).
pub(crate) fn q_sign_text(convention: ReactiveSignConvention) -> &'static str {
    match convention {
        ReactiveSignConvention::InjectionPositive => "INJECTION_POSITIVE",
        ReactiveSignConvention::ConsumptionPositive => "CONSUMPTION_POSITIVE",
    }
}

fn q_sign_from_text(raw: &str) -> Result<ReactiveSignConvention, BackendError> {
    match raw {
        "INJECTION_POSITIVE" => Ok(ReactiveSignConvention::InjectionPositive),
        "CONSUMPTION_POSITIVE" => Ok(ReactiveSignConvention::ConsumptionPositive),
        _ => Err(crate::error::decode_failure("telemetry.q_sign_convention")),
    }
}

/// P2.31: shared with `corrections.rs` (the as-of read variants decode the
/// same row shape).
pub(crate) fn decode_stored_telemetry(row: &PgRow) -> Result<StoredTelemetry, TelemetryError> {
    let tenant_id = TenantId::new(col_uuid(row, "tenant_id")?);
    let asset_id = AssetId::new(col_uuid(row, "asset_id")?);
    let quantity = Quantity::from_db_text(&col_str(row, "quantity")?)
        .map_err(TelemetryError::Backend)?;
    let value = if let Some(num) = col_f64_opt(row, "value_num")? {
        Some(TelemetryValue::Numeric(num))
    } else if let Some(text) = col_opt_str(row, "value_text")? {
        Some(TelemetryValue::Text(text))
    } else if let Some(flag) = col_bool_opt(row, "value_bool")? {
        Some(TelemetryValue::Flag(flag))
    } else {
        None
    };
    let quality = quality_from_text(&col_str(row, "quality")?)
        .map_err(TelemetryError::Backend)?;
    let q_sign_convention = match col_opt_str(row, "q_sign_convention")? {
        Some(raw) => Some(q_sign_from_text(&raw).map_err(TelemetryError::Backend)?),
        None => None,
    };
    let clock_quality =
        clock_quality_from_text(&col_str(row, "clock_quality")?)
            .map_err(TelemetryError::Backend)?;
    Ok(StoredTelemetry {
        telemetry_id: col_uuid(row, "telemetry_id")?,
        tenant_id,
        asset_id,
        quantity,
        value,
        event_time: col_event_time(row, "event_time")?,
        ingest_time: col_ingest_time(row, "ingest_time")?,
        source_sequence: col_i64(row, "source_sequence")?,
        source_id: col_str(row, "source_id")?,
        quality,
        source_quality_code: col_opt_str(row, "source_quality_code")?,
        q_sign_convention,
        clock_quality,
    })
}

// --- Validation before SQL (repository-boundary re-checks) ----------------------

/// Validates the record against the caller's tenant context and the
/// storage-level rules the database would also enforce (checked first so a
/// rejected record costs no round trip): tenant match, time ordering
/// (P1.05 semantics re-checked at the repository boundary, P2.05 §3), value
/// totality, quantity/value-kind pairing, unit derivation, reactive
/// convention presence, finiteness, and the `bigint` range of the source
/// sequence.
/// P2.31: shared with `corrections.rs` — a corrected record is re-validated
/// against the identical storage rules before its version row is written.
pub(crate) fn validate_record(
    ctx: &TenantContext,
    record: &NormalizedTelemetryRecord,
) -> Result<(), TelemetryError> {
    if record.tenant_id != ctx.tenant_id() {
        return Err(TelemetryError::TenantMismatch);
    }
    if record.ingest_time.timestamp() < record.event_time.timestamp() {
        return Err(backend_rejected(
            "the record's ingest_time precedes its event_time (P0.09 time ordering)",
        ));
    }
    if record.source_sequence < 0 {
        return Err(backend_rejected(
            "the record's source_sequence is negative (the storage column is a non-negative bigint)",
        ));
    }
    if record.source_id.trim().is_empty() {
        return Err(backend_rejected(
            "the record's source_id is empty (source identity is mandatory, P2.04 §8)",
        ));
    }
    // Reactive convention presence (D-T4): NOT NULL exactly when the
    // quantity is reactive_power.
    let is_reactive = record.quantity == Quantity::ReactivePower;
    if is_reactive != record.q_sign_convention.is_some() {
        return Err(backend_rejected(
            "a reactive_power record must carry its declared q_sign_convention, and no other quantity may carry one (P2.04 D-T4)",
        ));
    }
    // Value totality + quantity/value-kind pairing (P2.04 §3).
    match (&record.value, record.quality) {
        (Some(_), TelemetryQuality::Missing) => {
            return Err(backend_rejected(
                "a MISSING record is a gap marker and must not carry a value",
            ));
        }
        (None, TelemetryQuality::Missing) => {}
        (None, _) => {
            return Err(backend_rejected(
                "a measured record must carry a value (the partial-record rule)",
            ));
        }
        (Some(value), _) => {
            let kind_ok = match value {
                TelemetryValue::Numeric(n) => {
                    if !n.is_finite() {
                        return Err(backend_rejected(
                            "a numeric value must be finite (NaN/infinity rejected, P1.05 UnitsError::NotFinite)",
                        ));
                    }
                    record.quantity.expects_numeric()
                }
                TelemetryValue::Text(text) => {
                    if text.trim().is_empty() {
                        return Err(backend_rejected(
                            "a text state value must be non-empty",
                        ));
                    }
                    !record.quantity.expects_numeric() && !record.quantity.expects_bool()
                }
                TelemetryValue::Flag(_) => record.quantity.expects_bool(),
            };
            if !kind_ok {
                return Err(backend_rejected(
                    "the record's value kind does not match its quantity (P2.04 §3-§4 pairing)",
                ));
            }
            // SOC fraction range (P0.07 §4; D-S5 forward constraint). The
            // value kind was already checked above, so only numeric values
            // reach this branch for soc.
            if record.quantity == Quantity::Soc {
                if let TelemetryValue::Numeric(n) = value {
                    if !(0.0..=1.0).contains(n) {
                        return Err(backend_rejected(
                            "a soc value must be a fraction in [0,1] (P0.07 §4)",
                        ));
                    }
                }
            }
        }
    }
    Ok(())
}

/// The INSERT bind values for a validated record.
struct InsertBinds {
    tenant_text: String,
    asset_text: String,
    quantity_text: &'static str,
    unit_text: &'static str,
    value_num: Option<f64>,
    value_text: Option<String>,
    value_bool: Option<bool>,
    q_sign_text: Option<&'static str>,
}

fn insert_binds(record: &NormalizedTelemetryRecord) -> InsertBinds {
    let (value_num, value_text, value_bool) = match &record.value {
        Some(TelemetryValue::Numeric(n)) => (Some(*n), None, None),
        Some(TelemetryValue::Text(text)) => (None, Some(text.clone()), None),
        Some(TelemetryValue::Flag(flag)) => (None, None, Some(*flag)),
        None => (None, None, None),
    };
    InsertBinds {
        tenant_text: uuid_text(record.tenant_id.uuid()),
        asset_text: uuid_text(record.asset_id.uuid()),
        quantity_text: record.quantity.db_text(),
        // The unit is derived from the quantity, never carried from the
        // caller — it cannot disagree (P2.04 §4).
        unit_text: record.quantity.canonical_unit(),
        value_num,
        value_text,
        value_bool,
        q_sign_text: record
            .q_sign_convention
            .map(q_sign_text),
    }
}

/// Whether a sqlx error is the PostgreSQL partition-routing failure ("no
/// partition of relation ... found for row"). RECORDED ASSUMPTION (verified
/// by the P2.06 partition test on PostgreSQL 18): the server raises
/// SQLSTATE 23514 with NO constraint name for this case, while every
/// genuine CHECK violation of this schema carries its constraint name —
/// classification is by (code, constraint) pair, never by message text.
fn is_no_partition_error(err: &sqlx::Error) -> bool {
    match err {
        sqlx::Error::Database(db) => {
            db.code().as_deref() == Some("23514") && db.constraint().is_none()
        }
        _ => false,
    }
}

/// Whether a sqlx error is a violation of the telemetry primary key (the
/// duplicate signal; constraint classification, never text matching).
/// P2.31: shared with `corrections.rs` (a concurrent correction of the same
/// identity collides on the same PK).
pub(crate) fn is_telemetry_pkey_violation(err: &sqlx::Error) -> bool {
    matches!(
        err,
        sqlx::Error::Database(db)
            if matches!(db.kind(), sqlx::error::DatabaseErrorKind::UniqueViolation)
                && db.constraint() == Some("telemetry_pkey")
    )
}

/// One INSERT attempt inside the savepoint.
async fn insert_attempt(tx: &mut Tx, binds: &InsertBinds, record: &NormalizedTelemetryRecord) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO telemetry \
             (tenant_id, asset_id, quantity, event_time, ingest_time, source_sequence, \
              source_id, quality, source_quality_code, value_num, value_text, value_bool, \
              unit, q_sign_convention, clock_quality) \
         VALUES ($1::uuid, $2::uuid, $3, $4, $5, $6, $7, $8::telemetry_quality, $9, \
                 $10, $11, $12, $13, $14::reactive_sign_convention, $15::clock_quality)",
    )
    .bind(&binds.tenant_text)
    .bind(&binds.asset_text)
    .bind(binds.quantity_text)
    .bind(record.event_time.timestamp())
    .bind(record.ingest_time.timestamp())
    .bind(record.source_sequence)
    .bind(&record.source_id)
    .bind(quality_text(record.quality))
    .bind(&record.source_quality_code)
    .bind(binds.value_num)
    .bind(binds.value_text.clone())
    .bind(binds.value_bool)
    .bind(binds.unit_text)
    .bind(binds.q_sign_text)
    .bind(clock_quality_text(record.clock_quality))
    .execute(&mut *tx)
    .await
    .map(|_| ())
}

/// Reads the colliding row back and compares it field-by-field with the
/// rejected insert. Exact duplicate iff every value-bearing and identity
/// field is identical (the surrogate telemetry_id and the uuidv7 default
/// are the only fields allowed to differ — they are not identity, P2.04
/// D-T3 / P2.05 §5 item 4).
async fn colliding_row_is_exact_duplicate(
    tx: &mut Tx,
    binds: &InsertBinds,
    record: &NormalizedTelemetryRecord,
) -> Result<bool, TelemetryError> {
    let row = sqlx::query(
        "SELECT tenant_id::text AS tenant_id, ingest_time, source_id, \
                quality::text AS quality, source_quality_code, value_num, \
                value_text, value_bool, unit, \
                q_sign_convention::text AS q_sign_convention, \
                clock_quality::text AS clock_quality \
         FROM telemetry \
         WHERE asset_id = $1::uuid AND event_time = $2 \
           AND source_sequence = $3 AND quantity = $4",
    )
    .bind(&binds.asset_text)
    .bind(record.event_time.timestamp())
    .bind(record.source_sequence)
    .bind(binds.quantity_text)
    .fetch_optional(&mut *tx)
    .await
    .map_err(map_backend)
    .map_err(TelemetryError::Backend)?;
    let row = match row {
        Some(row) => row,
        // The colliding row vanished between the failed insert and the
        // read (a concurrent transaction's rows are invisible until
        // commit, so within this transaction the row must exist). No row
        // means a concurrent writer committed and was removed in between;
        // treat as non-duplicate so the caller quarantines the ambiguous
        // record instead of acknowledging it (fail closed).
        None => return Ok(false),
    };
    let stored_tenant = row
        .try_get::<String, _>("tenant_id")
        .map_err(|_| TelemetryError::Backend(crate::error::decode_failure("telemetry.tenant_id")))?;
    if stored_tenant != binds.tenant_text {
        return Ok(false);
    }
    let stored_ingest: time::OffsetDateTime = row
        .try_get("ingest_time")
        .map_err(|_| TelemetryError::Backend(crate::error::decode_failure("telemetry.ingest_time")))?;
    if stored_ingest != record.ingest_time.timestamp() {
        return Ok(false);
    }
    let stored_source_id = row
        .try_get::<String, _>("source_id")
        .map_err(|_| TelemetryError::Backend(crate::error::decode_failure("telemetry.source_id")))?;
    if stored_source_id != record.source_id {
        return Ok(false);
    }
    let stored_quality = row
        .try_get::<String, _>("quality")
        .map_err(|_| TelemetryError::Backend(crate::error::decode_failure("telemetry.quality")))?;
    if stored_quality != quality_text(record.quality) {
        return Ok(false);
    }
    let stored_source_quality_code = row
        .try_get::<Option<String>, _>("source_quality_code")
        .map_err(|_| TelemetryError::Backend(crate::error::decode_failure("telemetry.source_quality_code")))?;
    if stored_source_quality_code != record.source_quality_code {
        return Ok(false);
    }
    let stored_num = row
        .try_get::<Option<f64>, _>("value_num")
        .map_err(|_| TelemetryError::Backend(crate::error::decode_failure("telemetry.value_num")))?;
    let stored_text = row
        .try_get::<Option<String>, _>("value_text")
        .map_err(|_| TelemetryError::Backend(crate::error::decode_failure("telemetry.value_text")))?;
    let stored_bool = row
        .try_get::<Option<bool>, _>("value_bool")
        .map_err(|_| TelemetryError::Backend(crate::error::decode_failure("telemetry.value_bool")))?;
    let (num, text, boolean) = match &record.value {
        Some(TelemetryValue::Numeric(n)) => (Some(*n), None, None),
        Some(TelemetryValue::Text(text)) => (None, Some(text.clone()), None),
        Some(TelemetryValue::Flag(flag)) => (None, None, Some(*flag)),
        None => (None, None, None),
    };
    if stored_num != num || stored_text != text || stored_bool != boolean {
        return Ok(false);
    }
    let stored_unit = row
        .try_get::<String, _>("unit")
        .map_err(|_| TelemetryError::Backend(crate::error::decode_failure("telemetry.unit")))?;
    if stored_unit != binds.unit_text {
        return Ok(false);
    }
    let stored_q_sign = row
        .try_get::<Option<String>, _>("q_sign_convention")
        .map_err(|_| TelemetryError::Backend(crate::error::decode_failure("telemetry.q_sign_convention")))?;
    if stored_q_sign != binds.q_sign_text.map(str::to_string) {
        return Ok(false);
    }
    let stored_clock = row
        .try_get::<String, _>("clock_quality")
        .map_err(|_| TelemetryError::Backend(crate::error::decode_failure("telemetry.clock_quality")))?;
    if stored_clock != clock_quality_text(record.clock_quality) {
        return Ok(false);
    }
    Ok(true)
}

/// Inserts one validated record inside the caller's transaction, with the
/// savepoint-guarded duplicate classification. The unit of work's tenant
/// GUC is already set (store.begin(Some(&ctx))); the record's own tenant
/// was re-checked against the context before any SQL ran.
async fn insert_one(
    tx: &mut Tx,
    record: &NormalizedTelemetryRecord,
) -> Result<TelemetryInsertOutcome, TelemetryError> {
    let binds = insert_binds(record);
    let savepoint = format!("telemetry_insert_{}", SAVEPOINT_COUNTER.fetch_add(1, Ordering::SeqCst));
    sqlx::query(&format!("SAVEPOINT {savepoint}"))
        .execute(&mut *tx)
        .await
        .map_err(map_backend)
        .map_err(TelemetryError::Backend)?;
    match insert_attempt(tx, &binds, record).await {
        Ok(()) => {
            sqlx::query(&format!("RELEASE SAVEPOINT {savepoint}"))
                .execute(&mut *tx)
                .await
                .map_err(map_backend)
                .map_err(TelemetryError::Backend)?;
            Ok(TelemetryInsertOutcome::Stored)
        }
        Err(err) => {
            // Roll the failed statement back so the surrounding
            // transaction stays usable regardless of the classification.
            sqlx::query(&format!("ROLLBACK TO SAVEPOINT {savepoint}"))
                .execute(&mut *tx)
                .await
                .map_err(map_backend)
                .map_err(TelemetryError::Backend)?;
            if is_telemetry_pkey_violation(&err) {
                let exact = colliding_row_is_exact_duplicate(tx, &binds, record).await?;
                sqlx::query(&format!("RELEASE SAVEPOINT {savepoint}"))
                    .execute(&mut *tx)
                    .await
                    .map_err(map_backend)
                    .map_err(TelemetryError::Backend)?;
                if exact {
                    Ok(TelemetryInsertOutcome::ExactDuplicateAcknowledged)
                } else {
                    Err(TelemetryError::ConflictingDuplicate)
                }
            } else if is_no_partition_error(&err) {
                sqlx::query(&format!("RELEASE SAVEPOINT {savepoint}"))
                    .execute(&mut *tx)
                    .await
                    .map_err(map_backend)
                    .map_err(TelemetryError::Backend)?;
                Err(TelemetryError::OutOfRangeEventTime)
            } else {
                sqlx::query(&format!("RELEASE SAVEPOINT {savepoint}"))
                    .execute(&mut *tx)
                    .await
                    .map_err(map_backend)
                    .map_err(TelemetryError::Backend)?;
                Err(TelemetryError::Backend(map_backend(err)))
            }
        }
    }
}

// --- The repository on the unit of work ------------------------------------------

impl PgUnitOfWork {
    /// Inserts one canonical record inside THIS unit of work's transaction
    /// (P2.07 pipeline stage 6: the store stage runs inside the unit of
    /// work). Duplicate/out-of-range outcomes are classified via savepoints
    /// and leave the transaction usable; other storage failures surface as
    /// [`TelemetryError::Backend`].
    pub fn insert_telemetry(
        &self,
        ctx: &TenantContext,
        record: &NormalizedTelemetryRecord,
    ) -> Result<TelemetryInsertOutcome, TelemetryError> {
        validate_record(ctx, record)?;
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard)
                .await
                .map_err(TelemetryError::Backend)?;
            insert_one(tx, record).await
        })
    }

    /// Inserts a batch of records, one at a time, inside the same unit of
    /// work (P2.07 stage 6; the batch is the pipeline's granularity input).
    /// Per-record duplicates and out-of-range records are reported as
    /// [`TelemetryBatchItem`]s (the caller quarantines them); the first
    /// hard storage failure aborts the batch with `Err` — the caller rolls
    /// the unit of work back (no partial-visibility issue: the transaction
    /// is all-or-nothing on commit either way).
    pub fn insert_telemetry_batch(
        &self,
        ctx: &TenantContext,
        records: &[NormalizedTelemetryRecord],
    ) -> Result<Vec<TelemetryBatchItem>, TelemetryError> {
        for record in records {
            validate_record(ctx, record)?;
        }
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard).await.map_err(TelemetryError::Backend)?;
            let mut items = Vec::with_capacity(records.len());
            for (index, record) in records.iter().enumerate() {
                match insert_one(tx, record).await {
                    Ok(TelemetryInsertOutcome::Stored) => {
                        items.push(TelemetryBatchItem {
                            index,
                            outcome: TelemetryBatchOutcome::Stored,
                        });
                    }
                    Ok(TelemetryInsertOutcome::ExactDuplicateAcknowledged) => {
                        items.push(TelemetryBatchItem {
                            index,
                            outcome: TelemetryBatchOutcome::ExactDuplicateAcknowledged,
                        });
                    }
                    Err(TelemetryError::ConflictingDuplicate) => {
                        items.push(TelemetryBatchItem {
                            index,
                            outcome: TelemetryBatchOutcome::ConflictingDuplicate,
                        });
                    }
                    Err(TelemetryError::OutOfRangeEventTime) => {
                        items.push(TelemetryBatchItem {
                            index,
                            outcome: TelemetryBatchOutcome::OutOfRangeEventTime,
                        });
                    }
                    Err(other) => return Err(other),
                }
            }
            Ok(items)
        })
    }

    /// Time-bounded history for the named assets and quantities (P2.06
    /// step 2; the P2.11 `TelemetryQuery` shape: tenant-scoped asset set +
    /// `from`/`until` over `event_time` + named quantities — no implicit
    /// wildcard). `from <= until` is the caller's contract (the P2.11 query
    /// constructor enforces it); this repository keeps both bounds inclusive
    /// per that contract. Tenant scoping: the unit of work's RLS GUC plus
    /// the explicit `tenant_id` filter (defence in depth, P1.13 pattern).
    pub fn query_telemetry(
        &self,
        ctx: &TenantContext,
        assets: &[AssetId],
        from: EventTime,
        until: EventTime,
        quantities: Option<&[Quantity]>,
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
        let asset_texts: Vec<String> = assets
            .iter()
            .map(|asset| uuid_text(asset.uuid()))
            .collect();
        let tenant_text = uuid_text(ctx.tenant_id().uuid());
        let quantity_texts: Option<Vec<&'static str>> =
            quantities.map(|set| set.iter().map(|q| q.db_text()).collect());
        // The optional quantity filter is appended from the frozen enum's
        // own db_text values — never caller-supplied text.
        let sql = match &quantity_texts {
            Some(_) => format!(
                "{TELEMETRY_COLUMNS} WHERE tenant_id = $1::uuid \
                 AND asset_id = ANY($2::uuid[]) \
                 AND event_time >= $3 AND event_time <= $4 \
                 AND quantity = ANY($5::text[]) \
                 ORDER BY event_time DESC, asset_id, quantity, source_sequence"
            ),
            None => format!(
                "{TELEMETRY_COLUMNS} WHERE tenant_id = $1::uuid \
                 AND asset_id = ANY($2::uuid[]) \
                 AND event_time >= $3 AND event_time <= $4 \
                 ORDER BY event_time DESC, asset_id, quantity, source_sequence"
            ),
        };
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard).await.map_err(TelemetryError::Backend)?;
            let mut query = sqlx::query(&sql)
                .bind(&tenant_text)
                .bind(&asset_texts)
                .bind(from.timestamp())
                .bind(until.timestamp());
            if let Some(texts) = &quantity_texts {
                query = query.bind(texts);
            }
            let rows = query
                .fetch_all(&mut *tx)
                .await
                .map_err(map_backend)
                .map_err(TelemetryError::Backend)?;
            rows.iter().map(decode_stored_telemetry).collect()
        })
    }

    /// The latest row per (asset, quantity) for the named assets (P2.18
    /// current-state read rule: the latest `event_time`, never the latest
    /// arrival — P2.07 §4). Served by the composite DESC indexes (D-P4);
    /// no materialized current-state table exists in Phase 2.
    ///
    /// P2.31: the `telemetry_version DESC` final tiebreak guarantees the
    /// current-state view reads the LATEST VERSION of a corrected record —
    /// a superseded version of the same storage identity (same event_time,
    /// same source_sequence) can never win the `DISTINCT ON`. With no
    /// corrections present the ordering behaves exactly as before (every
    /// row is version 1).
    pub fn latest_telemetry_per_asset(
        &self,
        ctx: &TenantContext,
        assets: &[AssetId],
        quantities: Option<&[Quantity]>,
    ) -> Result<Vec<StoredTelemetry>, TelemetryError> {
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
        let asset_texts: Vec<String> = assets
            .iter()
            .map(|asset| uuid_text(asset.uuid()))
            .collect();
        let tenant_text = uuid_text(ctx.tenant_id().uuid());
        let quantity_texts: Option<Vec<&'static str>> =
            quantities.map(|set| set.iter().map(|q| q.db_text()).collect());
        let sql = match &quantity_texts {
            Some(_) => format!(
                "SELECT DISTINCT ON (asset_id, quantity) {} \
                 WHERE tenant_id = $1::uuid \
                   AND asset_id = ANY($2::uuid[]) \
                   AND quantity = ANY($3::text[]) \
                 ORDER BY asset_id, quantity, event_time DESC, source_sequence DESC, \
                 telemetry_version DESC",
                TELEMETRY_COLUMNS.trim_start_matches("SELECT ")
            ),
            None => format!(
                "SELECT DISTINCT ON (asset_id, quantity) {} \
                 WHERE tenant_id = $1::uuid \
                   AND asset_id = ANY($2::uuid[]) \
                 ORDER BY asset_id, quantity, event_time DESC, source_sequence DESC, \
                 telemetry_version DESC",
                TELEMETRY_COLUMNS.trim_start_matches("SELECT ")
            ),
        };
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard).await.map_err(TelemetryError::Backend)?;
            let mut query = sqlx::query(&sql)
                .bind(&tenant_text)
                .bind(&asset_texts);
            if let Some(texts) = &quantity_texts {
                query = query.bind(texts);
            }
            let rows = query
                .fetch_all(&mut *tx)
                .await
                .map_err(map_backend)
                .map_err(TelemetryError::Backend)?;
            rows.iter().map(decode_stored_telemetry).collect()
        })
    }

    /// Records one quarantined record (P2.07 §3): the evidence trail for a
    /// rejected record — never silently dropped, never stored as GOOD.
    pub fn quarantine_telemetry(
        &self,
        ctx: &TenantContext,
        entry: &TelemetryQuarantineEntry,
    ) -> Result<(), TelemetryError> {
        if entry.tenant_id != ctx.tenant_id() {
            return Err(TelemetryError::TenantMismatch);
        }
        if entry.stage.trim().is_empty() || entry.reason.trim().is_empty() {
            return Err(backend_rejected(
                "a quarantine entry must record its stage and the rule that rejected the record",
            ));
        }
        let tenant_text = uuid_text(entry.tenant_id.uuid());
        let asset_text = entry.asset_id.map(|asset| uuid_text(asset.uuid()));
        let event_time = entry.event_time.map(|time| time.timestamp());
        let payload = entry.raw_payload.clone();
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard).await.map_err(TelemetryError::Backend)?;
            sqlx::query(
                "INSERT INTO telemetry_quarantine \
                     (tenant_id, asset_id, event_time, source_id, source_sequence, \
                      stage, reason, raw_payload, attempt_count) \
                 VALUES ($1::uuid, $2::uuid, $3, $4, $5, $6, $7, $8, $9)",
            )
            .bind(&tenant_text)
            .bind(&asset_text)
            .bind(event_time)
            .bind(&entry.source_id)
            .bind(entry.source_sequence)
            .bind(&entry.stage)
            .bind(&entry.reason)
            .bind(payload)
            .bind(entry.attempt_count)
            .execute(&mut *tx)
            .await
            .map_err(map_backend)
            .map_err(TelemetryError::Backend)?;
            Ok(())
        })
    }

    /// Lists quarantine entries for the caller's tenant, newest first
    /// (P2.07 §3: queryable, tenant-scoped; the admin API path is P2.12).
    pub fn list_telemetry_quarantine(
        &self,
        ctx: &TenantContext,
        limit: u32,
    ) -> Result<Vec<StoredQuarantineEntry>, TelemetryError> {
        let tenant_text = uuid_text(ctx.tenant_id().uuid());
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard).await.map_err(TelemetryError::Backend)?;
            let rows = sqlx::query(&format!(
                "{QUARANTINE_COLUMNS} WHERE tenant_id = $1::uuid \
                 ORDER BY quarantined_at DESC, quarantine_id LIMIT $2"
            ))
            .bind(&tenant_text)
            .bind(i64::from(limit))
            .fetch_all(&mut *tx)
            .await
            .map_err(map_backend)
            .map_err(TelemetryError::Backend)?;
            let mut entries = Vec::with_capacity(rows.len());
            for row in &rows {
                let tenant_uuid = col_uuid(row, "tenant_id")?;
                // asset_id is nullable ("if resolvable at the failed stage",
                // P2.07 §3); there is deliberately no FK on it.
                let asset_id = col_opt_str(row, "asset_id")?
                    .map(|text| {
                        uuid_from_text(&text)
                            .map_err(|_| decode_failure("telemetry_quarantine.asset_id"))
                    })
                    .transpose()?
                    .map(AssetId::new);
                entries.push(StoredQuarantineEntry {
                    quarantine_id: col_uuid(row, "quarantine_id")?,
                    quarantined_at: col_event_time(row, "quarantined_at")?,
                    entry: TelemetryQuarantineEntry {
                        tenant_id: TenantId::new(tenant_uuid),
                        asset_id,
                        event_time: col_opt_event_time(row, "event_time")?,
                        source_id: col_opt_str(row, "source_id")?,
                        source_sequence: col_opt_i64(row, "source_sequence")?,
                        stage: col_str(row, "stage")?,
                        reason: col_str(row, "reason")?,
                        raw_payload: row
                            .try_get("raw_payload")
                            .map_err(|_| decode_failure("telemetry_quarantine.raw_payload"))?,
                        attempt_count: row
                            .try_get::<i32, _>("attempt_count")
                            .map_err(|_| decode_failure("telemetry_quarantine.attempt_count"))?,
                    },
                });
            }
            Ok(entries)
        })
    }
}

// ---------------------------------------------------------------------------
// P2.19 — The historical view: keyset-paginated raw query + SQL bucket
// aggregation (decisions D-H1/D-H2 in
// `ledger/phase-02-asset-telemetry/19_20_historical_retention.md`).
//
// Both queries are partition-pruning by construction: the `event_time`
// bounds are ALWAYS in the WHERE clause against the partition key (P2.05
// D-P2), index-aligned with the P2.05 composite indexes. Both order and
// filter by `event_time` ONLY — `ingest_time` is metadata (P0.09), never an
// ordering or filter key. Both run inside the caller's unit-of-work
// transaction (RLS `flexcore.tenant_id` applies) and additionally filter by
// the context's tenant (defence in depth, P1.13 pattern).
//
// NO ROUNDING anywhere (P0.07 stance, D-H2): aggregates are returned as
// PostgreSQL double precision verbatim; neither this SQL nor any layer above
// rounds, re-scales, or re-units a value. P > 0 = export is the sign
// convention of the stored values and aggregation never re-interprets it.
// ---------------------------------------------------------------------------

/// The allowed bucket widths for the P2.19 bucketed aggregation (D-H2: the
/// set is closed — `1m|5m|15m|1h|1d` — and extension is an explicit change
/// to the decision document and this parser, never a client-provided
/// interval). Pure: parseable and testable without a database.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HistoryBucketWidth {
    OneMinute,
    FiveMinutes,
    FifteenMinutes,
    OneHour,
    OneDay,
}

impl HistoryBucketWidth {
    /// The wire key (`bucket` query parameter).
    pub fn key(self) -> &'static str {
        match self {
            HistoryBucketWidth::OneMinute => "1m",
            HistoryBucketWidth::FiveMinutes => "5m",
            HistoryBucketWidth::FifteenMinutes => "15m",
            HistoryBucketWidth::OneHour => "1h",
            HistoryBucketWidth::OneDay => "1d",
        }
    }

    /// Parses a bucket-width key. Anything outside the closed set is `None`
    /// and must surface as a 422 at the API layer, never a default width.
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim() {
            "1m" => Some(HistoryBucketWidth::OneMinute),
            "5m" => Some(HistoryBucketWidth::FiveMinutes),
            "15m" => Some(HistoryBucketWidth::FifteenMinutes),
            "1h" => Some(HistoryBucketWidth::OneHour),
            "1d" => Some(HistoryBucketWidth::OneDay),
            _ => None,
        }
    }

    /// The width in milliseconds (the bucket-count limit is validated
    /// against this, D-H3).
    pub fn millis(self) -> i64 {
        match self {
            HistoryBucketWidth::OneMinute => 60_000,
            HistoryBucketWidth::FiveMinutes => 300_000,
            HistoryBucketWidth::FifteenMinutes => 900_000,
            HistoryBucketWidth::OneHour => 3_600_000,
            HistoryBucketWidth::OneDay => 86_400_000,
        }
    }

    /// The PostgreSQL interval literal (bound as a text parameter cast to
    /// `::interval` — never caller-supplied text).
    fn sql_interval(self) -> &'static str {
        match self {
            HistoryBucketWidth::OneMinute => "1 minute",
            HistoryBucketWidth::FiveMinutes => "5 minutes",
            HistoryBucketWidth::FifteenMinutes => "15 minutes",
            HistoryBucketWidth::OneHour => "1 hour",
            HistoryBucketWidth::OneDay => "1 day",
        }
    }
}

/// The opaque keyset pagination cursor of the historical query (D-H1):
/// keyset over the P2.05 PK's varying legs `(event_time, source_sequence,
/// quantity)`; the asset leg is fixed by the single-asset query contract.
/// Wire format `hist-v1:<event_time millis, zero-padded 15>:<source_sequence,
/// zero-padded 19>:<quantity id>`, parsed strictly — a malformed cursor is
/// `None` and must be rejected (422), never silently treated as "first
/// page". Recorded assumption (as the P2.12 port cursor): `event_time` is a
/// non-negative epoch-millisecond value, so zero-padding preserves numeric
/// order lexicographically; `source_sequence` is a non-negative `bigint`
/// (storage CHECK), padded to its full 19-digit range.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryCursor {
    event_time_millis: i64,
    source_sequence: i64,
    quantity: Quantity,
}

impl HistoryCursor {
    /// Builds the cursor for the last record of a page (the strictly-after
    /// key of the next page).
    pub fn for_record(record: &StoredTelemetry) -> Self {
        let millis = record.event_time.timestamp().unix_millis();
        Self {
            event_time_millis: i64::try_from(millis)
                .expect("event_time milliseconds fit i64 (the recorded non-negative epoch assumption)"),
            source_sequence: record.source_sequence,
            quantity: record.quantity,
        }
    }

    /// The opaque wire form.
    pub fn encode(&self) -> String {
        format!(
            "hist-v1:{:0>15}:{:0>19}:{}",
            self.event_time_millis,
            self.source_sequence,
            self.quantity.db_text()
        )
    }

    /// Parses an opaque cursor value. `None` for anything malformed.
    pub fn decode(raw: &str) -> Option<Self> {
        let rest = raw.strip_prefix("hist-v1:")?;
        let mut parts = rest.splitn(3, ':');
        let millis = parts.next()?;
        let sequence = parts.next()?;
        let quantity = parts.next()?;
        if millis.len() != 15 || !millis.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        if sequence.len() != 19 || !sequence.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        Some(Self {
            event_time_millis: millis.parse::<i64>().ok()?,
            source_sequence: sequence.parse::<i64>().ok()?,
            quantity: Quantity::from_db_text(quantity).ok()?,
        })
    }

    /// P2.31: the cursor's keyset legs, for the as-of history variant in
    /// `corrections.rs` (which rebuilds the same strict-after comparison;
    /// the fields stay private to this crate).
    pub(crate) fn legs(&self) -> (i64, i64, Quantity) {
        (self.event_time_millis, self.source_sequence, self.quantity)
    }
}

/// One occupied aggregation bucket (D-H2): the bucket's start instant
/// (`date_bin` with origin = the query's inclusive `from` bound), the number
/// of numeric rows in it, and the count/mean/min/max of `value_num`. A
/// `MISSING` gap marker has no value and contributes to no bucket — it is
/// counted by nothing, never counted as zero.
#[derive(Debug, Clone, PartialEq)]
pub struct TelemetryBucket {
    pub bucket_start: EventTime,
    pub sample_count: i64,
    pub mean: f64,
    pub min: f64,
    pub max: f64,
}

impl PgUnitOfWork {
    /// One keyset-paginated page of the historical raw query (P2.19 step 1;
    /// D-H1). Single asset, `event_time` range REQUIRED (the caller/URL
    /// contract; re-checked here), optional quantity and quality filters
    /// (quality: exactly the six P0.08 states — the caller validates the
    /// vocabulary; this repository binds only enum text it produced), page
    /// limit, optional cursor. Returns the ascending page and the cursor to
    /// continue from (`None` = last page). The SQL is the D-H4
    /// representative query: event_time bounds against the partition key,
    /// ordered by the PK's varying legs.
    #[allow(clippy::too_many_arguments)]
    pub fn query_telemetry_history(
        &self,
        ctx: &TenantContext,
        asset: AssetId,
        from: EventTime,
        until: EventTime,
        quantities: Option<&[Quantity]>,
        quality: Option<TelemetryQuality>,
        limit: u32,
        cursor: Option<HistoryCursor>,
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

        // Parameter indices are assigned as clauses are appended (the
        // optional quantity/quality/cursor clauses shift the LIMIT index).
        let mut next_param = 5usize;
        let mut sql = format!(
            "{TELEMETRY_COLUMNS} WHERE tenant_id = $1::uuid \
             AND asset_id = $2::uuid \
             AND event_time >= $3 AND event_time <= $4"
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
            // varying legs (D-H1). Explicit casts so the parameter types
            // are fixed regardless of the planner's inference.
            sql.push_str(&format!(
                " AND (event_time, source_sequence, quantity) > \
                 (${p}::timestamptz, ${q}::bigint, ${r}::text)"
            ));
        }
        sql.push_str(" ORDER BY event_time ASC, source_sequence ASC, quantity ASC \
             LIMIT $");
        sql.push_str(&next_param.to_string());

        let fetch = i64::from(limit) + 1; // one extra row detects a next page
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard).await.map_err(TelemetryError::Backend)?;
            let mut query = sqlx::query(&sql)
                .bind(&tenant_text)
                .bind(&asset_text)
                .bind(from.timestamp())
                .bind(until.timestamp());
            if let Some(texts) = &quantity_texts {
                query = query.bind(texts);
            }
            if let Some(text) = &quality_text {
                query = query.bind(text);
            }
            if let Some(cursor) = &cursor {
                let cursor_time = time::OffsetDateTime::from_unix_timestamp_millis(
                    cursor.event_time_millis,
                )
                .map_err(|_| decode_failure("telemetry.cursor"))?;
                query = query
                    .bind(cursor_time)
                    .bind(cursor.source_sequence)
                    .bind(cursor.quantity.db_text());
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
                records.last().map(HistoryCursor::for_record)
            } else {
                None
            };
            Ok((records, next_cursor))
        })
    }

    /// The bucketed aggregation (P2.19 step 2; D-H2/D-H4): count, mean,
    /// min, and max of `value_num` per `date_bin` bucket, bucketed IN SQL,
    /// origin = the query's inclusive `from` bound (the first bucket starts
    /// exactly at `from`). Numeric rows only — a `MISSING` gap marker has
    /// no value and contributes to no bucket. The quantity must be a
    /// numeric-carrying one (mixing units across quantities, or aggregating
    /// boolean/text-state values, is refused here as well as at the API
    /// layer — P0.07).
    pub fn aggregate_telemetry_history(
        &self,
        ctx: &TenantContext,
        asset: AssetId,
        quantity: Quantity,
        from: EventTime,
        until: EventTime,
        quality: Option<TelemetryQuality>,
        bucket: HistoryBucketWidth,
    ) -> Result<Vec<TelemetryBucket>, TelemetryError> {
        if from.timestamp() > until.timestamp() {
            return Err(backend_rejected(
                "the query time range is inverted (from after until)",
            ));
        }
        if !quantity.expects_numeric() {
            return Err(backend_rejected(
                "bucketed aggregation is defined for numeric-carrying quantities only \
                 (P0.07: units are never mixed and state values have no numeric aggregate)",
            ));
        }
        let tenant_text = uuid_text(ctx.tenant_id().uuid());
        let asset_text = uuid_text(asset.uuid());
        let quality_text = quality.map(quality_text);
        // VERIFY-BUILD: parameterized `date_bin` stride (a text parameter
        // cast to ::interval) and parameterized origin are the
        // standard-documented form (PostgreSQL 14+); verified by the P2.19
        // aggregation tests on PostgreSQL 18.
        let mut sql = String::from(
            "SELECT date_bin($1::interval, event_time, $2::timestamptz) AS bucket_start, \
             count(*)::bigint AS sample_count, avg(value_num) AS mean_value, \
             min(value_num) AS min_value, max(value_num) AS max_value \
             FROM telemetry \
             WHERE tenant_id = $3::uuid AND asset_id = $4::uuid AND quantity = $5 \
               AND event_time >= $6 AND event_time <= $7 \
               AND value_num IS NOT NULL",
        );
        if quality_text.is_some() {
            sql.push_str(" AND quality::text = $8");
        }
        sql.push_str(" GROUP BY 1 ORDER BY 1");
        self.runtime.block_on(async {
            let mut guard = self.tx.lock().await;
            let tx = tx_ref(&mut guard).await.map_err(TelemetryError::Backend)?;
            let mut query = sqlx::query(&sql)
                .bind(bucket.sql_interval())
                .bind(from.timestamp())
                .bind(&tenant_text)
                .bind(&asset_text)
                .bind(quantity.db_text())
                .bind(from.timestamp())
                .bind(until.timestamp());
            if let Some(text) = &quality_text {
                query = query.bind(text);
            }
            let rows = query
                .fetch_all(&mut *tx)
                .await
                .map_err(map_backend)
                .map_err(TelemetryError::Backend)?;
            let mut buckets = Vec::with_capacity(rows.len());
            for row in &rows {
                let start: time::OffsetDateTime = row
                    .try_get("bucket_start")
                    .map_err(|_| decode_failure("telemetry.bucket_start"))?;
                buckets.push(TelemetryBucket {
                    bucket_start: EventTime::new(start)
                        .map_err(|_| decode_failure("telemetry.bucket_start"))?,
                    sample_count: row
                        .try_get::<i64, _>("sample_count")
                        .map_err(|_| decode_failure("telemetry.sample_count"))?,
                    mean: row
                        .try_get::<f64, _>("mean_value")
                        .map_err(|_| decode_failure("telemetry.mean_value"))?,
                    min: row
                        .try_get::<f64, _>("min_value")
                        .map_err(|_| decode_failure("telemetry.min_value"))?,
                    max: row
                        .try_get::<f64, _>("max_value")
                        .map_err(|_| decode_failure("telemetry.max_value"))?,
                });
            }
            Ok(buckets)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quantity_vocabulary_is_the_frozen_p2_04_set() {
        // The nine frozen keys, each with its canonical unit per P2.04 §4.
        let pairs: [(Quantity, &str, &str); 9] = [
            (Quantity::ActivePower, "active_power", "MW"),
            (Quantity::ReactivePower, "reactive_power", "MVAr"),
            (Quantity::Soc, "soc", "1"),
            (Quantity::Energy, "energy", "MWh"),
            (Quantity::Availability, "availability", "1"),
            (Quantity::OperatingMode, "operating_mode", "1"),
            (Quantity::Health, "health", "1"),
            (Quantity::ConnectionState, "connection_state", "1"),
            (Quantity::BaselineReference, "baseline_reference", "MW"),
        ];
        for (quantity, key, unit) in pairs {
            assert_eq!(quantity.db_text(), key);
            assert_eq!(quantity.canonical_unit(), unit);
            assert_eq!(Quantity::from_db_text(key).expect("known key"), quantity);
        }
        // The vocabulary is frozen: an unknown key is a decode failure, and
        // extension is an explicit migration (P2.04 §4).
        assert!(Quantity::from_db_text("apparent_power").is_err());
    }

    #[test]
    fn quantity_value_kinds_match_the_p2_04_mapping() {
        // Numeric: active_power, reactive_power, soc, energy,
        // baseline_reference. Boolean: availability, connection_state.
        // Text state: operating_mode, health.
        for numeric in [Quantity::ActivePower, Quantity::ReactivePower, Quantity::Soc, Quantity::Energy, Quantity::BaselineReference] {
            assert!(numeric.expects_numeric());
            assert!(!numeric.expects_bool());
        }
        for boolean in [Quantity::Availability, Quantity::ConnectionState] {
            assert!(!boolean.expects_numeric());
            assert!(boolean.expects_bool());
        }
        for text_state in [Quantity::OperatingMode, Quantity::Health] {
            assert!(!text_state.expects_numeric());
            assert!(!text_state.expects_bool());
        }
    }

    #[test]
    fn no_partition_classification_uses_code_and_constraint_only() {
        // Classification is structural: the SQLSTATE class plus the absence
        // of a constraint name — never message text. A genuine CHECK
        // violation carries its constraint name, so the pair distinguishes
        // them (recorded assumption, verified by the partition tests).
        // The non-database transport errors never classify as partition
        // routing failures.
        assert!(!is_no_partition_error(&sqlx::Error::PoolClosed));
    }

    #[test]
    fn history_bucket_width_set_is_closed_and_pure() {
        // The recorded D-H2 allowed set: 1m|5m|15m|1h|1d, each with its
        // millisecond width; anything else is None (422 at the API layer).
        let pairs = [
            ("1m", HistoryBucketWidth::OneMinute, 60_000),
            ("5m", HistoryBucketWidth::FiveMinutes, 300_000),
            ("15m", HistoryBucketWidth::FifteenMinutes, 900_000),
            ("1h", HistoryBucketWidth::OneHour, 3_600_000),
            ("1d", HistoryBucketWidth::OneDay, 86_400_000),
        ];
        for (key, width, millis) in pairs {
            assert_eq!(HistoryBucketWidth::parse(key), Some(width));
            assert_eq!(width.key(), key);
            assert_eq!(width.millis(), millis);
        }
        assert_eq!(HistoryBucketWidth::parse("2m"), None);
        assert_eq!(HistoryBucketWidth::parse("1w"), None);
        assert_eq!(HistoryBucketWidth::parse(""), None);
        assert_eq!(HistoryBucketWidth::parse("1M"), None);
    }

    #[test]
    fn history_cursor_round_trips_and_parses_strictly() {
        let record = StoredTelemetry {
            telemetry_id: FlexUuid::from_bytes([7u8; 16]),
            tenant_id: TenantId::new(FlexUuid::from_bytes([1u8; 16])),
            asset_id: AssetId::new(FlexUuid::from_bytes([2u8; 16])),
            quantity: Quantity::ActivePower,
            value: Some(TelemetryValue::Numeric(1.5)),
            event_time: EventTime::from_unix_millis(1_893_456_000_123).expect("valid"),
            ingest_time: IngestTime::from_unix_millis(1_893_456_001_123).expect("valid"),
            source_sequence: 42,
            source_id: "sim://test".to_string(),
            quality: TelemetryQuality::Good,
            source_quality_code: None,
            q_sign_convention: None,
            clock_quality: ClockQuality::Synchronized,
        };
        let cursor = HistoryCursor::for_record(&record);
        let encoded = cursor.encode();
        assert!(encoded.starts_with("hist-v1:"));
        let decoded = HistoryCursor::decode(&encoded).expect("round trip");
        assert_eq!(decoded, cursor);
        // Strict: missing/wrong-length legs, unknown quantity, non-numeric
        // legs, and a foreign version prefix are all malformed.
        assert_eq!(HistoryCursor::decode("hist-v1:1:2:active_power"), None);
        assert_eq!(
            HistoryCursor::decode("hist-v1:0000001893456000123:0000000000000000042:active_power"),
            None
        );
        assert_eq!(
            HistoryCursor::decode("hist-v1:000000189345600123:0000000000000000042:apparent_power"),
            None
        );
        assert_eq!(
            HistoryCursor::decode("hist-v1:000000189345600123:00000000000000000x42:active_power"),
            None
        );
        assert_eq!(HistoryCursor::decode("v1:000000189345600123:0000000000000000042:active_power"), None);
        assert_eq!(HistoryCursor::decode(""), None);
    }

    #[test]
    fn telemetry_error_display_is_human_safe() {
        let errors = [
            TelemetryError::TenantMismatch,
            TelemetryError::ConflictingDuplicate,
            TelemetryError::OutOfRangeEventTime,
            TelemetryError::Backend(BackendError::new(
                codes::UNAVAILABLE.to_string(),
                "the storage backend is unavailable".to_string(),
            )),
        ];
        for error in &errors {
            let rendered = error.to_string();
            assert!(!rendered.contains("postgres://"));
            assert!(!rendered.contains("SQLSTATE"));
        }
    }
}
