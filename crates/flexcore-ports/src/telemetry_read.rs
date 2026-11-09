// P2.12/P2.17/P2.18 — The `TelemetryReader` port: the read side of the
//         canonical telemetry store (query, current state, quarantine).
//
// Additive port (recorded scope decision): the P2.06 storage adapter is
// being implemented in parallel and must NOT be coupled to this surface
// through its existing transaction-bound repository traits, so the read
// side is its own trait here. The Postgres implementation of
// `TelemetryReader` (on `PgUnitOfWork`, inside the request's transaction so
// RLS applies) is the recorded P2.06-adjacent gap; until it lands the only
// implementation is the in-crate test fake.
//
// Contract, precisely:
//
// - **Tenant scoping is structural** (P0.16 rules 1 and 5): every method
//   takes the domain `TenantContext` and reads only rows whose tenant
//   matches the context. A foreign-tenant row never exists at this
//   boundary (no existence leak), exactly like the repository ports.
// - [`TelemetryReadQuery`] names one asset, a `from`/`until` `event_time`
//   range, an optional quantity filter (`None` = the full frozen Phase 2
//   quantity vocabulary — a read-side wildcard is recorded here; the
//   source-side [`crate::telemetry_source::TelemetryQuery`] deliberately
//   has none), an optional quality filter, a page limit, and an optional
//   keyset cursor.
// - **Cursor pagination (DECIDED ONCE, P2.12)**: keyset over the record
//   sort key `(event_time, asset_id, quantity)` — the same key
//   [`sort_key`] renders. The cursor is an opaque versioned string; a
//   page returns the records strictly after the cursor in sort order. No
//   offset pagination exists anywhere: offsets are unstable on a
//   time-ordered, append-heavy table and a full scan waiting to happen
//   (the P2.12 named risk). The API layer enforces the limits (range,
//   page size) and rejects an unparsable cursor with 422.
// - [`latest_state`] implements the P2.18 current-state rule: for each
//   quantity of the asset, the record with the latest `event_time` (NOT
//   the latest `ingest_time`; ties break on the higher
//   `source_sequence`). A quantity with no record is simply absent —
//   "MISSING" is a state the API layer derives from absence, never a
//   zero value.
// - [`list_quarantine`] reads the P2.07 quarantine: the evidence trail of
//   rejected records (raw payload, failure reason, source identity,
//   timestamps, attempt count), tenant-scoped.
//
// Sync trait (house rule, crate README): no futures, no runtime types.
//
// Ingestion writes do NOT come through this port: the ingestion pipeline
// (P2.10) owns the write side. This is the read model only.

use crate::backend_error::BackendError;
use crate::telemetry_source::{NormalizedTelemetry, TelemetryQuantity};
use flexcore_contracts::time_model::EventTime;
use flexcore_domain::ids::{AssetId, TenantId};
use flexcore_domain::tenant_context::TenantContext;

// --- Quantity key helpers ---------------------------------------------------

/// The canonical lowercase snake_case id of a frozen Phase 2 quantity
/// (P2.04 §4 vocabulary). Used on the API surface (query filters), in the
/// cursor sort key, and in audit records. One spelling for every caller.
pub fn quantity_key(quantity: TelemetryQuantity) -> &'static str {
    match quantity {
        TelemetryQuantity::ActivePower => "active_power",
        TelemetryQuantity::ReactivePower => "reactive_power",
        TelemetryQuantity::Soc => "soc",
        TelemetryQuantity::Energy => "energy",
        TelemetryQuantity::Availability => "availability",
        TelemetryQuantity::OperatingMode => "operating_mode",
        TelemetryQuantity::Health => "health",
        TelemetryQuantity::ConnectionState => "connection_state",
        TelemetryQuantity::BaselineReference => "baseline_reference",
    }
}

/// Parses a canonical quantity id. Unknown ids are `None` and must surface
/// as validation rejections at the API layer, never as a silent wildcard.
pub fn parse_quantity_key(id: &str) -> Option<TelemetryQuantity> {
    match id {
        "active_power" => Some(TelemetryQuantity::ActivePower),
        "reactive_power" => Some(TelemetryQuantity::ReactivePower),
        "soc" => Some(TelemetryQuantity::Soc),
        "energy" => Some(TelemetryQuantity::Energy),
        "availability" => Some(TelemetryQuantity::Availability),
        "operating_mode" => Some(TelemetryQuantity::OperatingMode),
        "health" => Some(TelemetryQuantity::Health),
        "connection_state" => Some(TelemetryQuantity::ConnectionState),
        "baseline_reference" => Some(TelemetryQuantity::BaselineReference),
        _ => None,
    }
}

/// The canonical unit id of a quantity's value (P2.04 §5; `1` is the
/// dimensionless unit). Recorded here so every read-side response carries
/// the unit explicitly instead of letting consumers infer it.
pub fn quantity_unit(quantity: TelemetryQuantity) -> &'static str {
    match quantity {
        TelemetryQuantity::ActivePower
        | TelemetryQuantity::BaselineReference => "MW",
        TelemetryQuantity::ReactivePower => "MVAr",
        TelemetryQuantity::Soc
        | TelemetryQuantity::Availability
        | TelemetryQuantity::OperatingMode
        | TelemetryQuantity::Health
        | TelemetryQuantity::ConnectionState => "1",
        TelemetryQuantity::Energy => "MWh",
    }
}

/// The record sort key `(event_time, asset_id, quantity)` as one
/// lexicographically comparable string: a zero-padded millisecond
/// timestamp, the asset UUID, and the quantity id. This is BOTH the query
/// sort order and the cursor payload (DECIDED ONCE above).
///
/// Recorded assumption: `event_time` is non-negative epoch millis (the
/// platform never stores pre-1970 measurements), so zero-padding preserves
/// numeric order lexicographically.
pub fn sort_key(record: &NormalizedTelemetry) -> String {
    format!(
        "v1:{:0>15}:{}:{}",
        record.event_time().unix_millis(),
        record.asset_id().uuid().to_hyphenated(),
        quantity_key(record.quantity()),
    )
}

// --- Cursor -----------------------------------------------------------------

/// An opaque keyset pagination cursor (DECIDED ONCE, see module header).
/// Constructed from the sort key of the last record of a page; parsed
/// strictly — anything that does not carry the version prefix and a
/// well-formed key is `None` and must be rejected (422), never silently
/// treated as "first page".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TelemetryCursor {
    key: String,
}

impl TelemetryCursor {
    /// Builds the cursor for the record with the given sort key.
    pub fn from_key(key: String) -> Self {
        Self { key }
    }

    /// Builds the cursor from a record (the last record of a page).
    pub fn for_record(record: &NormalizedTelemetry) -> Self {
        Self {
            key: sort_key(record),
        }
    }

    /// Parses an opaque cursor value. `None` for anything malformed.
    pub fn parse(raw: &str) -> Option<Self> {
        let rest = raw.strip_prefix("v1:")?;
        let mut parts = rest.splitn(3, ':');
        let millis = parts.next()?;
        let asset = parts.next()?;
        let quantity = parts.next()?;
        if millis.len() != 15 || !millis.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        flexcore_contracts::ids::FlexUuid::parse(asset).ok()?;
        parse_quantity_key(quantity)?;
        Some(Self {
            key: raw.to_string(),
        })
    }

    /// The raw sort key this cursor carries (strictly-after comparison).
    pub fn key(&self) -> &str {
        &self.key
    }
}

// --- The query --------------------------------------------------------------

/// Errors returned by [`TelemetryReader`] query methods.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TelemetryReadError {
    /// The time range is inverted (`from` after `until`).
    InvertedTimeRange,
    /// The page limit was zero — a page names at least one record.
    EmptyPageLimit,
    /// The read backend failed. Code and message are opaque and
    /// human-safe, never raw storage text (see [`BackendError`]).
    Backend(BackendError),
}

impl std::fmt::Display for TelemetryReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TelemetryReadError::InvertedTimeRange => {
                write!(f, "the query time range is inverted (from after until)")
            }
            TelemetryReadError::EmptyPageLimit => {
                write!(f, "the page limit must be at least one record")
            }
            TelemetryReadError::Backend(e) => write!(f, "telemetry read backend failure: {e}"),
        }
    }
}

impl std::error::Error for TelemetryReadError {}

/// What a consumer wants from [`TelemetryReader::query`]: one asset, an
/// `event_time` range, optional quantity/quality filters, a page limit,
/// and an optional keyset cursor. Built only through
/// [`TelemetryReadQuery::new`], which rejects an inverted range and a zero
/// limit; every other limit (maximum range, maximum page size) is the API
/// layer's recorded contract and is enforced there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TelemetryReadQuery {
    asset: AssetId,
    from: EventTime,
    until: EventTime,
    quantities: Option<Vec<TelemetryQuantity>>,
    quality: Option<flexcore_contracts::quality::TelemetryQuality>,
    limit: u32,
    cursor: Option<TelemetryCursor>,
}

impl TelemetryReadQuery {
    /// Builds a query. `quantities == None` selects the full frozen
    /// quantity vocabulary (recorded read-side wildcard); `quality ==
    /// None` selects every quality state. Rejects an inverted time range
    /// and a zero page limit.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        asset: AssetId,
        from: EventTime,
        until: EventTime,
        quantities: Option<Vec<TelemetryQuantity>>,
        quality: Option<flexcore_contracts::quality::TelemetryQuality>,
        limit: u32,
        cursor: Option<TelemetryCursor>,
    ) -> Result<Self, TelemetryReadError> {
        if from > until {
            return Err(TelemetryReadError::InvertedTimeRange);
        }
        if limit == 0 {
            return Err(TelemetryReadError::EmptyPageLimit);
        }
        Ok(Self {
            asset,
            from,
            until,
            quantities,
            quality,
            limit,
            cursor,
        })
    }

    /// The queried asset.
    pub fn asset(&self) -> AssetId {
        self.asset
    }

    /// Inclusive range start over `event_time`.
    pub fn from(&self) -> EventTime {
        self.from
    }

    /// Inclusive range end over `event_time`.
    pub fn until(&self) -> EventTime {
        self.until
    }

    /// The quantity filter; `None` = all frozen quantities.
    pub fn quantities(&self) -> Option<&[TelemetryQuantity]> {
        self.quantities.as_deref()
    }

    /// The quality filter; `None` = every canonical state.
    pub fn quality(&self) -> Option<flexcore_contracts::quality::TelemetryQuality> {
        self.quality
    }

    /// The maximum number of records this page may return.
    pub fn limit(&self) -> u32 {
        self.limit
    }

    /// The keyset cursor, if the caller continues from a previous page.
    pub fn cursor(&self) -> Option<&TelemetryCursor> {
        self.cursor.as_ref()
    }
}

/// One page of telemetry query results, in ascending sort-key order.
#[derive(Debug, Clone, PartialEq)]
pub struct TelemetryPage {
    records: Vec<NormalizedTelemetry>,
    next_cursor: Option<TelemetryCursor>,
}

impl TelemetryPage {
    /// Builds a page. Intended for adapter implementations.
    pub fn new(records: Vec<NormalizedTelemetry>, next_cursor: Option<TelemetryCursor>) -> Self {
        Self {
            records,
            next_cursor,
        }
    }

    /// The records of this page (ascending sort-key order).
    pub fn records(&self) -> &[NormalizedTelemetry] {
        &self.records
    }

    /// The cursor to continue from, when more records may follow.
    pub fn next_cursor(&self) -> Option<&TelemetryCursor> {
        self.next_cursor.as_ref()
    }
}

// --- The quarantine record --------------------------------------------------

/// One quarantined telemetry record (P2.07 §3): the evidence trail of a
/// record that failed the ingestion pipeline. Never silently dropped, never
/// silently stored as `GOOD` — this is where the rejected payloads live,
/// with the reason they were rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuarantinedTelemetry {
    tenant_id: TenantId,
    /// The raw payload exactly as received (diagnostic evidence).
    raw_payload: String,
    /// Why the record was rejected (the pipeline stage's recorded evidence).
    failure_reason: String,
    /// The source identity the record claimed (may be untrustworthy — that
    /// is part of what the pipeline records).
    source_id: String,
    /// The asset the record claimed, when one could be parsed.
    asset_id: Option<AssetId>,
    /// The event time the record claimed, when one could be parsed.
    event_time: Option<EventTime>,
    /// When the record was quarantined.
    quarantined_at: flexcore_contracts::time_model::IngestTime,
    /// How many reprocessing attempts were made so far.
    attempt_count: u32,
}

impl QuarantinedTelemetry {
    /// Builds a quarantine entry. Intended for the ingestion adapter.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        tenant_id: TenantId,
        raw_payload: String,
        failure_reason: String,
        source_id: String,
        asset_id: Option<AssetId>,
        event_time: Option<EventTime>,
        quarantined_at: flexcore_contracts::time_model::IngestTime,
        attempt_count: u32,
    ) -> Self {
        Self {
            tenant_id,
            raw_payload,
            failure_reason,
            source_id,
            asset_id,
            event_time,
            quarantined_at,
            attempt_count,
        }
    }

    /// The owning tenant.
    pub fn tenant_id(&self) -> TenantId {
        self.tenant_id
    }

    /// The raw payload as received.
    pub fn raw_payload(&self) -> &str {
        &self.raw_payload
    }

    /// The recorded failure reason.
    pub fn failure_reason(&self) -> &str {
        &self.failure_reason
    }

    /// The claimed source identity.
    pub fn source_id(&self) -> &str {
        &self.source_id
    }

    /// The claimed asset, when parseable.
    pub fn asset_id(&self) -> Option<AssetId> {
        self.asset_id
    }

    /// The claimed event time, when parseable.
    pub fn event_time(&self) -> Option<EventTime> {
        self.event_time
    }

    /// When the record was quarantined.
    pub fn quarantined_at(&self) -> flexcore_contracts::time_model::IngestTime {
        self.quarantined_at
    }

    /// How many reprocessing attempts were made so far.
    pub fn attempt_count(&self) -> u32 {
        self.attempt_count
    }
}

// --- The port ---------------------------------------------------------------

/// The read side of the canonical telemetry store (P2.12 query, P2.18
/// current state, P2.07 quarantine read). Tenant-scoped by construction;
/// foreign-tenant rows do not exist at this boundary.
pub trait TelemetryReader: Send + Sync {
    /// Runs a time-range query (keyset-paginated, ascending sort-key order).
    fn query(
        &self,
        ctx: &TenantContext,
        query: TelemetryReadQuery,
    ) -> Result<TelemetryPage, TelemetryReadError>;

    /// The P2.18 current state: for each quantity of the asset, the record
    /// with the latest `event_time` (ties: higher `source_sequence`).
    /// Quantities with no record are absent — the API layer derives
    /// MISSING from absence, never a zero value.
    fn latest_state(
        &self,
        ctx: &TenantContext,
        asset_id: AssetId,
    ) -> Result<Vec<NormalizedTelemetry>, TelemetryReadError>;

    /// The P2.07 quarantine, tenant-scoped, in quarantine order.
    fn list_quarantine(
        &self,
        ctx: &TenantContext,
    ) -> Result<Vec<QuarantinedTelemetry>, TelemetryReadError>;
}
