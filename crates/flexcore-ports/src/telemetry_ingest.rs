// P2.09/P2.10 — The telemetry INGEST store port: the write side of the
// ingestion pipeline (stages 5–6 of the P2.07 design: duplicate handling
// + store, with the quarantine evidence trail).
//
// Additive port (recorded scope decision, same pattern as
// [`crate::telemetry_read::TelemetryReader`]): the storage adapter owns the
// concrete write surface already (the P2.06 repository), so this trait is
// the port-shaped view the P2.09 dedup module and the P2.10 ingestion
// service program against. The composition root (the worker, P2.10) maps
// it 1:1 onto the storage adapter's unit-of-work repository — the mapping
// obligation is recorded in the adapter's telemetry module header.
//
// Contract, precisely:
//
// - **Identity and dedup (P2.09 step 1, P2.05 D-P3)**: a sample's dedup
//   identity is `(asset_id, event_time, source_sequence)`; the row grain
//   adds `quantity` (one row = one quantity of one sample). The storage
//   adapter enforces it with the primary key; [`TelemetryIngestStore::
//   insert_batch`] classifies every colliding record:
//   - **exact duplicate** (same identity, every value-bearing field
//     identical) → [`TelemetryIngestOutcome::ExactDuplicateAcknowledged`],
//     never a second row (P2.09: acknowledged, not re-stored);
//   - **conflicting duplicate** (same identity, different content) →
//     [`TelemetryIngestOutcome::ConflictingDuplicate`] — never silently
//     overwritten, never silently accepted; the CALLER quarantines it via
//     [`TelemetryIngestStore::quarantine`] with a reason.
// - **Out-of-range `event_time`** (P2.05 §7 item 2): a record outside
//     every existing partition surfaces as
//     [`TelemetryIngestOutcome::OutOfRangeEventTime`] — the caller
//     quarantines and counts it, never silently drops it and never
//     re-writes it as `GOOD`.
// - **Latest-by-event-time read**: [`TelemetryIngestStore::latest_record`]
//   returns the current-state snapshot for one (asset, quantity): latest
//   `event_time`, never latest arrival (P2.09 step 3; the SQL read rule is
//   `DISTINCT ON ... ORDER BY event_time DESC`).
// - **Consumer-side idempotency** ([`IngestionInbox`], P2.09 step 4): the
//   inbox pattern of P1.16, split into the check and the record half so
//   the caller can record a delivery only AFTER its batch committed (a
//   check-and-record inside the caller's transaction would either break
//   the store's atomicity or skip a rolled-back batch on redelivery).
//   The dedup rule: a delivery whose key is already recorded is skipped,
//   never re-processed; a delivery recorded and then lost to a rollback is
//   re-processed and made harmless by the store-level dedup above.
//
// Sync trait (house rule, crate README): no futures, no runtime types.
//
// Tenant scoping is structural (P0.16 rules 1 and 5): every method takes
// the domain `TenantContext`; a record whose tenant disagrees with the
// context is refused, never stored.

use crate::backend_error::BackendError;
use crate::telemetry_source::{NormalizedTelemetry, TelemetryQuantity};
use flexcore_contracts::quality::TelemetryQuality;
use flexcore_contracts::time_model::EventTime;
use flexcore_domain::ids::{AssetId, TenantId};
use flexcore_domain::tenant_context::TenantContext;

// --- Per-record outcomes -----------------------------------------------------

/// The outcome of one record inside a committed ingest batch (P2.09).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TelemetryIngestOutcome {
    /// Stored as a new row.
    Stored,
    /// Exact duplicate acknowledged (P2.09): same identity and identical
    /// content; no second row was written.
    ExactDuplicateAcknowledged,
    /// Conflicting duplicate (P2.09): same identity, different content.
    /// The stored value is UNCHANGED; the caller must quarantine the
    /// incoming record with a reason.
    ConflictingDuplicate,
    /// The record's `event_time` falls outside every existing partition
    /// (P2.05 §7 item 2). The caller must quarantine and count it.
    OutOfRangeEventTime,
}

/// One batch member's index (into the caller's slice) and its outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TelemetryIngestItem {
    pub index: usize,
    pub outcome: TelemetryIngestOutcome,
}

/// Errors returned by the [`TelemetryIngestStore`] port. Per-record
/// duplicate/routing outcomes travel as batch items; this error type is
/// for whole-call failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TelemetryIngestError {
    /// A record (or quarantine entry) belongs to a different tenant than
    /// the caller's context — refused before any storage call.
    TenantMismatch,
    /// The storage backend failed. Code and message are opaque and
    /// human-safe, never raw storage text (see [`BackendError`]).
    Backend(BackendError),
}

impl std::fmt::Display for TelemetryIngestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TelemetryIngestError::TenantMismatch => {
                write!(f, "the telemetry record belongs to a different tenant than the caller's context")
            }
            TelemetryIngestError::Backend(e) => write!(f, "telemetry ingest backend failure: {e}"),
        }
    }
}

impl std::error::Error for TelemetryIngestError {}

/// The current-state snapshot of one (asset, quantity): the record with
/// the latest `event_time` (never the latest arrival — P2.09 step 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TelemetryLatestRecord {
    /// The `event_time` of the latest record.
    pub event_time: EventTime,
    /// Its canonical quality state.
    pub quality: TelemetryQuality,
}

// --- Quarantine write --------------------------------------------------------

/// One quarantine entry to RECORD (P2.07 §3): the evidence trail for a
/// rejected record — never silently dropped, never silently stored as
/// `GOOD`. The write-shaped sibling of the read-side
/// [`crate::telemetry_read::QuarantinedTelemetry`]: the ingestion pipeline
/// supplies stage, rule, and evidence; the store stamps the timestamp.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TelemetryQuarantineWrite {
    /// The authenticated source context's tenant (always present — a
    /// record rejected before tenant resolution is rejected at the
    /// receive stage, not quarantined).
    pub tenant_id: TenantId,
    /// The measured asset, if resolvable at the failed stage.
    pub asset_id: Option<AssetId>,
    /// The claimed event time, if present on the rejected record.
    pub event_time: Option<EventTime>,
    /// The source identity, if known.
    pub source_id: Option<String>,
    /// The source-assigned sequence, if present.
    pub source_sequence: Option<i64>,
    /// The pipeline stage that rejected the record (P2.07 stage number or
    /// name — the evidence trail names WHERE it failed).
    pub stage: String,
    /// The rule that rejected it (human-safe; never raw backend text).
    pub reason: String,
    /// The stage evidence (raw payload or a structured rendering of it).
    pub raw_payload: String,
    /// Retry attempt count (0 on first quarantine).
    pub attempt_count: i32,
}

// --- The port ----------------------------------------------------------------

/// The write side of the canonical telemetry store (P2.09 dedup + P2.07
/// stage 6 store + quarantine). The ingestion pipeline (P2.10) is the only
/// intended consumer; the read model is [`crate::telemetry_read::
/// TelemetryReader`].
///
/// Unit-of-work binding (adapter obligation, same convention as the
/// `EventOutbox` port): the composition root opens one unit of work per
/// batch; every port call inside the batch — inserts, quarantine writes —
/// executes inside that one native transaction, all-or-nothing.
pub trait TelemetryIngestStore: Send + Sync {
    /// Inserts a batch of canonical records. Per-record duplicate and
    /// partition-routing outcomes are reported as [`TelemetryIngestItem`]s
    /// (the caller quarantines them); a whole-call failure aborts the
    /// batch (the caller rolls the unit of work back — the transaction is
    /// all-or-nothing on commit either way).
    fn insert_batch(
        &self,
        ctx: &TenantContext,
        records: &[NormalizedTelemetry],
    ) -> Result<Vec<TelemetryIngestItem>, TelemetryIngestError>;

    /// Records one quarantine entry (P2.07 §3 evidence trail).
    fn quarantine(
        &self,
        ctx: &TenantContext,
        entry: &TelemetryQuarantineWrite,
    ) -> Result<(), TelemetryIngestError>;

    /// The current-state snapshot for one (asset, quantity): latest
    /// `event_time`, never latest arrival (P2.09 step 3). `None` = the
    /// asset has no record for that quantity yet.
    fn latest_record(
        &self,
        ctx: &TenantContext,
        asset_id: AssetId,
        quantity: TelemetryQuantity,
    ) -> Result<Option<TelemetryLatestRecord>, TelemetryIngestError>;
}

// --- Consumer-side idempotency (P2.09 step 4) --------------------------------

/// The outcome of recording one delivery.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IngestDeliveryOutcome {
    /// This caller's record won: the delivery was not recorded before.
    FirstDelivery,
    /// Another caller (or a concurrent redelivery) already recorded it:
    /// the side effect must NOT be executed again (P1.16 obligation).
    AlreadyRecorded,
}

/// Errors returned by the [`IngestionInbox`] port.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IngestInboxError {
    /// The inbox backend failed (opaque, human-safe).
    Backend(BackendError),
}

impl std::fmt::Display for IngestInboxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IngestInboxError::Backend(e) => write!(f, "ingest inbox backend failure: {e}"),
        }
    }
}

impl std::error::Error for IngestInboxError {}

/// The consumer-side idempotency inbox for the ingestion path (P2.09
/// step 4; the P1.16 inbox pattern, split into check and record halves).
///
/// The split is deliberate: the ingestion pipeline checks a delivery key
/// BEFORE processing (skip a redelivery) and records it only AFTER the
/// batch committed. A record inside the caller's store transaction would
/// be ideal and is the recorded promotion path (the durable inbox joins
/// the storage adapter's unit of work); until then, a record that never
/// reached commit is simply re-processed on redelivery and made harmless
/// by the store-level dedup ([`TelemetryIngestStore::insert_batch`]) and
/// the caller's deterministic event idempotency keys — at-least-once,
/// never exactly-once, never a skipped store write.
pub trait IngestionInbox: Send + Sync {
    /// Whether the delivery key is already recorded for this consumer.
    fn is_recorded(&self, consumer: &str, idempotency_key: &str) -> Result<bool, IngestInboxError>;

    /// Records one delivery. `AlreadyRecorded` means a concurrent
    /// duplicate won the race: the side effect must not execute again.
    fn record(
        &self,
        consumer: &str,
        idempotency_key: &str,
    ) -> Result<IngestDeliveryOutcome, IngestInboxError>;
}
