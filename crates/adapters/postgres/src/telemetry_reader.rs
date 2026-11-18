// P2.12 — The `TelemetryReader` port on the PostgreSQL adapter: the read
//         side of the canonical telemetry store (keyset-paginated query,
//         P2.18 current state, P2.07 quarantine read), tenant-scoped by
//         construction. Simulation-only (AGENTS.md §3 rule 5): this is a
//         read of stored simulation telemetry; it grants no control
//         authority and carries no protection-grade functionality.
//
// DESIGN DECISION (recorded once): the reader is POOL-HELD via
// `FlexCoreStore`, NOT bound to the request's `PgUnitOfWork`.
//
// - `PgUnitOfWork` is `!Sync` (its native transaction is not `Sync`), so
//   no reader holding one can be stored as `Arc<dyn TelemetryReader>` in
//   `AppState`; and `AppState` holds the reader as a long-lived `Arc`.
// - Each trait call therefore opens its OWN unit of work via
//   `FlexCoreStore::begin(Some(ctx))` — the same transaction discipline
//   as `FlexCoreStore::begin` (the RLS `flexcore.tenant_id` GUC is set
//   transaction-locally before the first statement) and the same
//   inherent repository methods — and commits it after the read. A
//   failed commit is a backend failure; on any error the unit of work
//   is dropped, which rolls the native transaction back (P1.10 adapter
//   contract rule 4).
// - Consequence, recorded honestly: the request's unit of work (e.g. the
//   asset-existence check the query route runs) and the reader's unit of
//   work are SEPARATE transactions. The read is tenant-scoped in its own
//   transaction either way (RLS GUC + the explicit `tenant_id` filter
//   in every repository query, the P1.13 defence-in-depth pattern), but
//   it does not share a snapshot with the request transaction.
//
// Cursor (P2.12, DECIDED ONCE — mirrors the in-crate fake's semantics
// exactly): the port cursor is the `sort_key` string, millisecond-
// granular by contract. Rows sharing one sort key (same millisecond
// `event_time` + same quantity for one asset, differing only in
// `source_sequence` or sub-millisecond time) sit at one sort-key
// position; a page boundary inside such a group can omit the group's
// later rows from later pages. The precise-pagination path is the
// history route, whose `HistoryCursor` carries `source_sequence`.
//
// Version visibility (P2.31 D-C4): the port query reads the LATEST
// version per storage identity (`NOT_SUPERSEDED` with a NULL as-of
// binding — see `PgUnitOfWork::query_telemetry_port`); the current-
// state read needs no such predicate because its `DISTINCT ON` already
// carries the `telemetry_version DESC` tiebreak.
//
// INDEX ANALYSIS (no migration — the existing indexes cover this
// query): migration 0010's `(tenant_id, asset_id, event_time DESC)`
// serves the tenant + asset + range predicate with a backward scan
// for ascending `event_time`, and `(asset_id, quantity, event_time
// DESC)` serves the quantity-filtered shape. No new index is added;
// EXPLAIN verification is deferred to the human build round.
//
// Error discipline (P1.07/P1.08): every failure surfaces as an opaque
// `TelemetryReadError::Backend` — stable code + fixed class message,
// never raw storage text. A stored row that cannot be rebuilt into a
// canonical `NormalizedTelemetry` (bad pairing, non-finite or
// out-of-range value, missing reactive convention, unparseable
// sequence) is a decode failure, never a coerced record.

use flexcore_contracts::time_model::IngestTime;
use flexcore_contracts::units::{ActivePowerMW, EnergyMWh, ReactivePowerMVAr, StateOfCharge};
use flexcore_domain::ids::AssetId;
use flexcore_domain::tenant_context::TenantContext;
use flexcore_ports::telemetry_read::{
    parse_quantity_key, QuarantinedTelemetry, TelemetryCursor, TelemetryPage, TelemetryReadError,
    TelemetryReadQuery, TelemetryReader,
};
use flexcore_ports::telemetry_source::{
    NormalizedTelemetry, NormalizedTelemetryParts, QuantityValue, TelemetryQuantity,
};

use crate::error::decode_failure;
use crate::store::FlexCoreStore;
use crate::telemetry::{
    Quantity, StoredQuarantineEntry, StoredTelemetry, TelemetryError, TelemetryValue,
};

/// The PostgreSQL `TelemetryReader` (P2.12): pool-held on the store; each
/// trait call opens its own tenant-scoped unit of work (see the module
/// header for the design decision).
pub struct PgTelemetryReader {
    store: FlexCoreStore,
}

impl PgTelemetryReader {
    /// Builds the reader over the store's pool and runtime.
    pub fn new(store: FlexCoreStore) -> Self {
        Self { store }
    }
}

/// A stored row that cannot be rebuilt into a canonical record is an
/// opaque decode failure (the stable decode code, naming the record —
/// never raw storage text).
fn decode_record() -> TelemetryReadError {
    TelemetryReadError::Backend(decode_failure("telemetry.record"))
}

/// Maps a repository error onto the port error. The port query path only
/// produces `Backend` failures — its validation rejections are
/// `Backend(REJECTED)` and every storage failure is `Backend`-mapped
/// inside the repository. The remaining `TelemetryError` variants belong
/// to the insert path and cannot surface here; they map to the same
/// opaque decode failure rather than inventing a new error class.
fn map_repository_error(err: TelemetryError) -> TelemetryReadError {
    match err {
        TelemetryError::Backend(backend) => TelemetryReadError::Backend(backend),
        _ => decode_record(),
    }
}

/// The port quantity for an adapter quantity. The two vocabularies are
/// the same frozen P2.04 §4 set, spelled identically, so the conversion
/// is total — a direct match, not a parse.
fn to_adapter_quantity(quantity: TelemetryQuantity) -> Quantity {
    match quantity {
        TelemetryQuantity::ActivePower => Quantity::ActivePower,
        TelemetryQuantity::ReactivePower => Quantity::ReactivePower,
        TelemetryQuantity::Soc => Quantity::Soc,
        TelemetryQuantity::Energy => Quantity::Energy,
        TelemetryQuantity::Availability => Quantity::Availability,
        TelemetryQuantity::OperatingMode => Quantity::OperatingMode,
        TelemetryQuantity::Health => Quantity::Health,
        TelemetryQuantity::ConnectionState => Quantity::ConnectionState,
        TelemetryQuantity::BaselineReference => Quantity::BaselineReference,
    }
}

/// Rebuilds one stored row into a canonical port record (P2.04 §3). The
/// value is built per quantity from the stored value kind; any pairing
/// the insert path could not have written — and any value the unit
/// constructors reject (non-finite, SOC outside [0,1]) — fails closed
/// as a decode failure. `None` is passed through untouched: it is valid
/// only with a `MISSING` gap marker, and `NormalizedTelemetry::new`
/// enforces that (as it enforces the quantity/value pairing, the
/// `flexcore:` source-namespace rule, and ingest-after-event ordering).
fn map_record(stored: &StoredTelemetry) -> Result<NormalizedTelemetry, TelemetryReadError> {
    let quantity =
        parse_quantity_key(stored.quantity.db_text()).ok_or_else(decode_record)?;
    let value = match (&stored.value, quantity) {
        (
            Some(TelemetryValue::Numeric(f)),
            TelemetryQuantity::ActivePower | TelemetryQuantity::BaselineReference,
        ) => Some(QuantityValue::Power(
            ActivePowerMW::new(*f).map_err(|_| decode_record())?,
        )),
        (Some(TelemetryValue::Numeric(f)), TelemetryQuantity::ReactivePower) => {
            Some(QuantityValue::Reactive {
                power: ReactivePowerMVAr::new(*f).map_err(|_| decode_record())?,
                convention: stored.q_sign_convention.ok_or_else(decode_record)?,
            })
        }
        (Some(TelemetryValue::Numeric(f)), TelemetryQuantity::Soc) => {
            Some(QuantityValue::Fraction(
                StateOfCharge::new(*f).map_err(|_| decode_record())?,
            ))
        }
        (Some(TelemetryValue::Numeric(f)), TelemetryQuantity::Energy) => {
            Some(QuantityValue::Energy(
                EnergyMWh::new(*f).map_err(|_| decode_record())?,
            ))
        }
        (
            Some(TelemetryValue::Text(text)),
            TelemetryQuantity::OperatingMode | TelemetryQuantity::Health,
        ) => Some(QuantityValue::State(text.clone())),
        (
            Some(TelemetryValue::Flag(flag)),
            TelemetryQuantity::Availability | TelemetryQuantity::ConnectionState,
        ) => Some(QuantityValue::Flag(*flag)),
        (None, _) => None,
        // Any other stored pairing cannot be a canonical record (the
        // insert path validates the same pairing before writing); fail
        // closed as a decode failure rather than coercing it.
        _ => return Err(decode_record()),
    };
    let source_sequence =
        u64::try_from(stored.source_sequence).map_err(|_| decode_record())?;
    NormalizedTelemetry::new(NormalizedTelemetryParts {
        tenant_id: stored.tenant_id,
        asset_id: stored.asset_id,
        quantity,
        value,
        event_time: stored.event_time,
        ingest_time: stored.ingest_time,
        source_sequence,
        source_id: stored.source_id.clone(),
        quality: stored.quality,
        source_quality_code: stored.source_quality_code.clone(),
        clock_quality: stored.clock_quality,
    })
    .map_err(|_| decode_record())
}

/// Rebuilds one stored quarantine row into a port quarantine record
/// (P2.07 §3): the raw payload as stored, the stage + reason as the
/// failure reason, and the claimed source identity / asset / event time
/// as parsed (or absent where the failed stage could not resolve them).
fn map_quarantine(
    stored: &StoredQuarantineEntry,
) -> Result<QuarantinedTelemetry, TelemetryReadError> {
    let entry = &stored.entry;
    let quarantined_at = IngestTime::new(stored.quarantined_at.timestamp()).map_err(|_| {
        TelemetryReadError::Backend(decode_failure("telemetry_quarantine.quarantined_at"))
    })?;
    let attempt_count = u32::try_from(entry.attempt_count).map_err(|_| {
        TelemetryReadError::Backend(decode_failure("telemetry_quarantine.attempt_count"))
    })?;
    Ok(QuarantinedTelemetry::new(
        entry.tenant_id,
        entry.raw_payload.to_string(),
        format!("{}: {}", entry.stage, entry.reason),
        entry.source_id.clone().unwrap_or_default(),
        entry.asset_id,
        entry.event_time,
        quarantined_at,
        attempt_count,
    ))
}

impl TelemetryReader for PgTelemetryReader {
    fn query(
        &self,
        ctx: &TenantContext,
        query: TelemetryReadQuery,
    ) -> Result<TelemetryPage, TelemetryReadError> {
        let adapter_quantities: Option<Vec<Quantity>> = query
            .quantities()
            .map(|quantities| {
                quantities
                    .iter()
                    .map(|quantity| to_adapter_quantity(*quantity))
                    .collect()
            });
        // One extra row is fetched (the repository's lookahead); the
        // reader takes the first `limit` rows and sets the next cursor
        // from the last of them iff the extra row exists.
        let limit = query.limit() as usize;
        let uow = self
            .store
            .begin(Some(ctx))
            .map_err(TelemetryReadError::Backend)?;
        let rows = uow
            .query_telemetry_port(
                ctx,
                query.asset(),
                query.from(),
                query.until(),
                adapter_quantities.as_deref(),
                query.quality(),
                query.limit(),
                query.cursor().cloned(),
            )
            .map_err(map_repository_error)?;
        let has_more = rows.len() > limit;
        let take = rows.len().min(limit);
        let mut records = Vec::with_capacity(take);
        for stored in &rows[..take] {
            records.push(map_record(stored)?);
        }
        let next_cursor = if has_more {
            records.last().map(TelemetryCursor::for_record)
        } else {
            None
        };
        uow.commit().map_err(TelemetryReadError::Backend)?;
        Ok(TelemetryPage::new(records, next_cursor))
    }

    fn latest_state(
        &self,
        ctx: &TenantContext,
        asset_id: AssetId,
    ) -> Result<Vec<NormalizedTelemetry>, TelemetryReadError> {
        let uow = self
            .store
            .begin(Some(ctx))
            .map_err(TelemetryReadError::Backend)?;
        // The P2.18 current-state rule, exactly as the repository
        // implements it: `DISTINCT ON (asset_id, quantity)`, latest
        // `event_time`, ties on higher `source_sequence`, final
        // `telemetry_version DESC` tiebreak so a superseded version can
        // never win.
        let rows = uow
            .latest_telemetry_per_asset(ctx, &[asset_id], None)
            .map_err(map_repository_error)?;
        let mut records = Vec::with_capacity(rows.len());
        for stored in &rows {
            records.push(map_record(stored)?);
        }
        uow.commit().map_err(TelemetryReadError::Backend)?;
        Ok(records)
    }

    fn list_quarantine(
        &self,
        ctx: &TenantContext,
    ) -> Result<Vec<QuarantinedTelemetry>, TelemetryReadError> {
        let uow = self
            .store
            .begin(Some(ctx))
            .map_err(TelemetryReadError::Backend)?;
        // The port defines no page bound for the quarantine read, so the
        // full tenant-scoped list is returned (u32::MAX as the
        // repository limit), newest first.
        let entries = uow
            .list_telemetry_quarantine(ctx, u32::MAX)
            .map_err(map_repository_error)?;
        let mut quarantined = Vec::with_capacity(entries.len());
        for stored in &entries {
            quarantined.push(map_quarantine(stored)?);
        }
        uow.commit().map_err(TelemetryReadError::Backend)?;
        Ok(quarantined)
    }
}
