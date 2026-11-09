// P2.09 — Duplicate and out-of-order handling (sequence, idempotency).
//
// The dedup rules of ledger P2.09 (report trace §6.3, §15.4, §6.4), on top
// of the storage decisions D-P3 (P2.05 §5: the row-grain PK
// `(asset_id, event_time, source_sequence, quantity)` and the sample-level
// dedup identity `(asset_id, event_time, source_sequence)`):
//
// 1. **Identity** ([`RecordIdentity`]): a telemetry sample is identified by
//    `(asset_id, event_time, source_sequence)`; the per-record idempotency
//    key ([`delivery_key`]) qualifies it with the quantity (the row grain)
//    and the source identity. Decided once here; the storage adapter
//    enforces the same tuple with its primary key.
// 2. **Duplicate handling**: an **exact duplicate** (same identity, every
//    value-bearing field identical) is acknowledged and not stored twice —
//    the classification itself is delegated to the store adapter's PK
//    collision path (`ExactDuplicateAcknowledged`); a **conflicting
//    duplicate** (same identity, different content) is quarantined with a
//    reason, never silently overwritten and never silently accepted. The
//    pure classification rule ([`duplicate_classification`]) is kept here
//    as the one spelling of the boundary.
// 3. **Out-of-order handling**: a late record with an OLDER `event_time` is
//    stored (valid history); the current state derives from the latest
//    `event_time`, never the latest arrival. The pure rule
//    ([`current_state_transition`]) decides per record whether it updates
//    the current state: a late NEWER `event_time` updates it; an older (or
//    equal) one never regresses it. The SQL read side (latest-per-asset
//    `DISTINCT ON ... ORDER BY event_time DESC`) implements the same rule
//    durably; this function is the write-side decision the pipeline
//    records as evidence.
// 4. **Consumer-side idempotency** (the P1.16 inbox pattern): a
//    redelivered record whose delivery key is already recorded is skipped,
//    never re-processed ([`TelemetryDeduplicator::process_batch`] checks
//    the inbox per record; the CALLER records the deliveries after its
//    batch commit — see the function's contract note).
//
// [`TelemetryDeduplicator::process_batch`] is the stage-5 orchestration the
// P2.10 ingestion pipeline calls: pre-check the inbox → store the batch
// (the adapter classifies duplicates) → quarantine conflicting and
// out-of-range records with a reason → report per-record dispositions,
// current-state transitions, and quality changes (the
// `TelemetryQualityChanged` trigger, P2.08's `quality_state_changed`).
//
// Pure where possible; the only I/O is through the
// `flexcore_ports::telemetry_ingest` ports. Sync (house rule). This crate
// depends on no serialization library, so the quarantine evidence payload
// is a structured, serializer-free rendering
// ([`record_evidence`]).

use std::collections::HashMap;

use flexcore_contracts::quality::TelemetryQuality;
use flexcore_contracts::time_model::EventTime;
use flexcore_domain::ids::AssetId;
use flexcore_domain::quality_rules::quality_state_changed;
use flexcore_domain::tenant_context::TenantContext;
use flexcore_ports::telemetry_ingest::{
    IngestionInbox, TelemetryIngestError, TelemetryIngestItem, TelemetryIngestOutcome,
    TelemetryIngestStore, TelemetryLatestRecord, TelemetryQuarantineWrite,
};
use flexcore_ports::telemetry_read::quantity_key;
use flexcore_ports::telemetry_source::{NormalizedTelemetry, TelemetryQuantity};

// --- Identity (P2.09 step 1, decided once) -----------------------------------

/// The identity of a telemetry sample for deduplication (P2.09 step 1;
/// D-P3): `(asset_id, event_time, source_sequence)`. The per-ROW grain
/// adds the quantity (one row = one quantity of one sample); the per-record
/// idempotency key ([`delivery_key`]) carries both plus the source id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RecordIdentity {
    /// The measured asset.
    pub asset_id: AssetId,
    /// When the measurement occurred.
    pub event_time: EventTime,
    /// The source-assigned ordering identifier.
    pub source_sequence: u64,
}

impl RecordIdentity {
    /// The identity of a canonical record.
    pub fn of(record: &NormalizedTelemetry) -> Self {
        Self {
            asset_id: record.asset_id(),
            event_time: record.event_time(),
            source_sequence: record.source_sequence(),
        }
    }
}

/// The per-record consumer-side idempotency key (P2.09 step 4): the dedup
/// identity plus the row-grain quantity and the source identity. The same
/// record redelivered by the source produces the same key; different
/// records never do (the key is the inbox's uniqueness input).
pub fn delivery_key(record: &NormalizedTelemetry) -> String {
    format!(
        "telemetry-ingest:{}:{}:{}:{}:{}",
        record.source_id(),
        record.asset_id().uuid().to_hyphenated(),
        record.event_time().unix_millis(),
        record.source_sequence(),
        quantity_key(record.quantity()),
    )
}

// --- Duplicate classification (pure rule) -------------------------------------

/// The classification of a same-identity collision (P2.09 step 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DuplicateClassification {
    /// Same identity, every value-bearing field identical: acknowledge,
    /// never store twice.
    ExactDuplicate,
    /// Same identity, different content: quarantine with a reason, never
    /// overwrite, never silently accept.
    ConflictingDuplicate,
}

/// The pure duplicate-classification rule: a collision is an exact
/// duplicate iff the two records are equal in every identity and
/// value-bearing field (the port record type has no surrogate handle, so
/// whole-value equality IS the rule). The storage adapter implements the
/// same classification against its stored row; this function is the one
/// spelled boundary both the fakes and the tests use.
pub fn duplicate_classification(
    stored: &NormalizedTelemetry,
    incoming: &NormalizedTelemetry,
) -> DuplicateClassification {
    if stored == incoming {
        DuplicateClassification::ExactDuplicate
    } else {
        DuplicateClassification::ConflictingDuplicate
    }
}

// --- Out-of-order / current-state rule (pure rule) -----------------------------

/// Whether an incoming record updates the current state of its (asset,
/// quantity) (P2.09 step 3; the P2.07 §4 ordering guarantee).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CurrentStateTransition {
    /// The record's `event_time` is newer than the current state's (or the
    /// state does not exist yet): the current state MOVES to this record.
    UpdatesState,
    /// The record's `event_time` is older than (or equal to) the current
    /// state's: the record is valid history and is stored, but the current
    /// state is NOT regressed.
    RetainsState,
}

/// The pure out-of-order rule: the current state derives from the latest
/// `event_time`, never the latest arrival.
///
/// - No current state yet → the record establishes it.
/// - Newer `event_time` than the current state → updates it (a late
///   NEWER record moves the state forward).
/// - Older or equal `event_time` → retains the state (a late OLDER record
///   never regresses it; an equal `event_time` with the same identity is a
///   duplicate handled by the dedup rule above, and a different record at
///   the same `event_time` never rewinds the frontier — ties on the read
///   side break on the higher `source_sequence`).
pub fn current_state_transition(
    current_latest: Option<EventTime>,
    incoming: EventTime,
) -> CurrentStateTransition {
    match current_latest {
        None => CurrentStateTransition::UpdatesState,
        Some(latest) if incoming > latest => CurrentStateTransition::UpdatesState,
        Some(_) => CurrentStateTransition::RetainsState,
    }
}

// --- The batch disposition ------------------------------------------------------

/// What happened to one record of a processed batch (per-record evidence;
/// the no-silent-skip rule — every input record gets exactly one
/// disposition).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordDisposition {
    /// Stored as a new row, with its current-state effect and (when there
    /// was a previous state) whether the canonical quality state changed.
    Stored {
        /// Whether the record moved the current state forward.
        transition: CurrentStateTransition,
        /// The previous current-state quality, when a state existed.
        previous_quality: Option<TelemetryQuality>,
        /// Whether the quality state changed vs the previous state
        /// (the `TelemetryQualityChanged` trigger; `false` for a first
        /// value — nothing changed FROM anything).
        quality_changed: bool,
    },
    /// Exact duplicate acknowledged: same identity and content; not stored
    /// twice.
    ExactDuplicateAcknowledged,
    /// Conflicting duplicate: same identity, different content. The stored
    /// value is unchanged; the record was quarantined with the reason.
    ConflictingDuplicateQuarantined {
        /// The recorded quarantine reason.
        reason: String,
    },
    /// `event_time` outside every existing partition. The record was
    /// quarantined with the reason (P2.05 §7 item 2).
    OutOfRangeEventTimeQuarantined {
        /// The recorded quarantine reason.
        reason: String,
    },
    /// A redelivered record whose delivery key was already recorded: NOT
    /// re-processed (the P1.16 inbox pattern).
    RedeliverySkipped,
}

/// The report of one processed batch: per-record dispositions plus the
/// aggregates the pipeline turns into metrics and events.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DedupReport {
    /// One disposition per input record, in input order.
    pub dispositions: Vec<RecordDisposition>,
    /// Records stored as new rows.
    pub stored: usize,
    /// Exact duplicates acknowledged.
    pub exact_duplicates_acknowledged: usize,
    /// Conflicting duplicates quarantined.
    pub conflicting_quarantined: usize,
    /// Out-of-range records quarantined.
    pub out_of_range_quarantined: usize,
    /// Redeliveries skipped via the inbox.
    pub redeliveries_skipped: usize,
    /// Stored records whose quality state changed vs the previous current
    /// state (the `TelemetryQualityChanged` triggers).
    pub quality_changes: usize,
    /// Stored records that moved the current state forward.
    pub state_updates: usize,
    /// The delivery keys of every processed (non-skipped) record, in input
    /// order. The caller records them in the inbox AFTER its batch commit
    /// (see [`TelemetryDeduplicator::process_batch`]).
    pub delivered_keys: Vec<String>,
}

/// Errors of the batch processing: a store or inbox failure aborts the
/// whole batch (the caller rolls its unit of work back — nothing was
/// committed, nothing is lost: the source redelivers).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DedupError {
    /// The store port failed.
    Store(TelemetryIngestError),
    /// The inbox port failed.
    Inbox(flexcore_ports::telemetry_ingest::IngestInboxError),
}

impl std::fmt::Display for DedupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DedupError::Store(e) => write!(f, "telemetry store failed: {e}"),
            DedupError::Inbox(e) => write!(f, "telemetry inbox failed: {e}"),
        }
    }
}

impl std::error::Error for DedupError {}

/// The recorded quarantine reason for a conflicting duplicate (P2.09
/// step 2: quarantined WITH A REASON, stored value unchanged).
pub const CONFLICTING_DUPLICATE_REASON: &str = "conflicting duplicate: a record with the same \
     identity (asset_id, event_time, source_sequence, quantity) but different content already \
     exists; the stored value was left unchanged";

/// The recorded quarantine reason for an out-of-range `event_time`
/// (P2.05 §7 item 2; partition pre-creation is the P2.20 job).
pub const OUT_OF_RANGE_REASON: &str = "the record's event_time falls outside every existing \
     telemetry partition; the record was not stored";

/// The stage-5 duplicate/out-of-order handler (P2.10 pipeline stage 5).
/// Stateless: construct once, call per batch.
#[derive(Debug, Clone)]
pub struct TelemetryDeduplicator {
    /// The inbox consumer name the delivery keys are checked against
    /// (per-pipeline, e.g. `telemetry-ingestion-v1`).
    consumer: String,
}

impl TelemetryDeduplicator {
    /// Builds the handler for one ingestion pipeline instance.
    pub fn new(consumer: String) -> Self {
        Self { consumer }
    }

    /// The inbox consumer name.
    pub fn consumer(&self) -> &str {
        &self.consumer
    }

    /// Processes one batch of quality-assigned canonical records through
    /// the dedup stage (P2.07 stage 5) and the store stage (stage 6 — the
    /// store's own collision classification IS the duplicate decision).
    ///
    /// Contract, precisely:
    ///
    /// 1. **Inbox pre-check** per record: a delivery key already recorded
    ///    → [`RecordDisposition::RedeliverySkipped`], never re-processed.
    /// 2. **Store the remaining records** in one
    ///    [`TelemetryIngestStore::insert_batch`] call; the adapter's PK
    ///    collision path classifies exact vs conflicting duplicates.
    /// 3. **Quarantine** every conflicting duplicate and every
    ///    out-of-range record, with the recorded reason constants — the
    ///    no-silent-skip rule (P2.07 §2).
    /// 4. **Report** per-record dispositions, current-state transitions
    ///    (the pure out-of-order rule against the pre-batch latest
    ///    snapshots, advanced in-batch as stored records move the
    ///    frontier), and quality changes.
    ///
    /// The caller MUST record `DedupReport::delivered_keys` in the inbox
    /// only AFTER its batch commit: recording inside the caller's
    /// transaction would either break the store's atomicity obligation or
    /// — worse — skip a rolled-back batch on redelivery. A delivery lost
    /// to a rollback is simply re-processed and made harmless by the
    /// store-level dedup (at-least-once, never exactly-once).
    pub fn process_batch<S: TelemetryIngestStore + ?Sized, I: IngestionInbox + ?Sized>(
        &self,
        ctx: &TenantContext,
        records: &[NormalizedTelemetry],
        store: &S,
        inbox: &I,
    ) -> Result<DedupReport, DedupError> {
        // 1. Inbox pre-check: skip redeliveries before any store call.
        let mut dispositions: Vec<Option<RecordDisposition>> = records
            .iter()
            .map(|record| {
                let key = delivery_key(record);
                if inbox
                    .is_recorded(&self.consumer, &key)
                    .map_err(DedupError::Inbox)?
                {
                    Some(RecordDisposition::RedeliverySkipped)
                } else {
                    None
                }
            })
            .collect();
        let processed: Vec<usize> = (0..records.len())
            .filter(|&i| dispositions[i].is_none())
            .collect();

        // 2. Pre-batch current-state snapshots for the distinct
        //    (asset, quantity) keys of the processed records (the pure
        //    out-of-order rule's input). Read once per key; advanced
        //    in-batch as stored records move the frontier.
        let mut snapshots: HashMap<(AssetId, TelemetryQuantity), Option<TelemetryLatestRecord>> =
            HashMap::new();
        for &i in &processed {
            let record = &records[i];
            let key = (record.asset_id(), record.quantity());
            if !snapshots.contains_key(&key) {
                let latest = store
                    .latest_record(ctx, record.asset_id(), record.quantity())
                    .map_err(DedupError::Store)?;
                snapshots.insert(key, latest);
            }
        }

        // 3. Store the batch (stage 6); the adapter classifies duplicates.
        let batch: Vec<NormalizedTelemetry> =
            processed.iter().map(|&i| records[i].clone()).collect();
        let items: Vec<TelemetryIngestItem> = if batch.is_empty() {
            Vec::new()
        } else {
            store.insert_batch(ctx, &batch).map_err(DedupError::Store)?
        };

        // 4. Dispositions + quarantine of the rejected (no-silent-skip).
        for (position, item) in items.iter().enumerate() {
            let index = processed[position];
            let record = &records[index];
            let disposition = match item.outcome {
                TelemetryIngestOutcome::Stored => {
                    let key = (record.asset_id(), record.quantity());
                    let previous = snapshots.get(&key).cloned().flatten();
                    let transition = current_state_transition(
                        previous.map(|p| p.event_time),
                        record.event_time(),
                    );
                    let quality_changed = match &previous {
                        Some(prev) => quality_state_changed(prev.quality, record.quality()),
                        None => false,
                    };
                    if transition == CurrentStateTransition::UpdatesState {
                        snapshots.insert(
                            key,
                            Some(TelemetryLatestRecord {
                                event_time: record.event_time(),
                                quality: record.quality(),
                            }),
                        );
                    }
                    RecordDisposition::Stored {
                        transition,
                        previous_quality: previous.map(|p| p.quality),
                        quality_changed,
                    }
                }
                TelemetryIngestOutcome::ExactDuplicateAcknowledged => {
                    RecordDisposition::ExactDuplicateAcknowledged
                }
                TelemetryIngestOutcome::ConflictingDuplicate => {
                    store
                        .quarantine(
                            ctx,
                            &quarantine_entry(record, CONFLICTING_DUPLICATE_REASON),
                        )
                        .map_err(DedupError::Store)?;
                    RecordDisposition::ConflictingDuplicateQuarantined {
                        reason: CONFLICTING_DUPLICATE_REASON.to_string(),
                    }
                }
                TelemetryIngestOutcome::OutOfRangeEventTime => {
                    store
                        .quarantine(ctx, &quarantine_entry(record, OUT_OF_RANGE_REASON))
                        .map_err(DedupError::Store)?;
                    RecordDisposition::OutOfRangeEventTimeQuarantined {
                        reason: OUT_OF_RANGE_REASON.to_string(),
                    }
                }
            };
            dispositions[index] = Some(disposition);
        }

        // 5. Assemble the report; every input record has a disposition.
        let mut report = DedupReport {
            dispositions: dispositions
                .into_iter()
                .map(|d| d.expect("every record receives a disposition"))
                .collect(),
            stored: 0,
            exact_duplicates_acknowledged: 0,
            conflicting_quarantined: 0,
            out_of_range_quarantined: 0,
            redeliveries_skipped: 0,
            quality_changes: 0,
            state_updates: 0,
            delivered_keys: Vec::new(),
        };
        for (index, disposition) in report.dispositions.iter().enumerate() {
            match disposition {
                RecordDisposition::Stored {
                    transition,
                    quality_changed,
                    ..
                } => {
                    report.stored += 1;
                    if *transition == CurrentStateTransition::UpdatesState {
                        report.state_updates += 1;
                    }
                    if *quality_changed {
                        report.quality_changes += 1;
                    }
                    report.delivered_keys.push(delivery_key(&records[index]));
                }
                RecordDisposition::ExactDuplicateAcknowledged => {
                    report.exact_duplicates_acknowledged += 1;
                    report.delivered_keys.push(delivery_key(&records[index]));
                }
                RecordDisposition::ConflictingDuplicateQuarantined { .. } => {
                    report.conflicting_quarantined += 1;
                    report.delivered_keys.push(delivery_key(&records[index]));
                }
                RecordDisposition::OutOfRangeEventTimeQuarantined { .. } => {
                    report.out_of_range_quarantined += 1;
                    report.delivered_keys.push(delivery_key(&records[index]));
                }
                RecordDisposition::RedeliverySkipped => {
                    report.redeliveries_skipped += 1;
                }
            }
        }
        Ok(report)
    }
}

/// Builds the quarantine evidence entry for one rejected record (P2.07 §3:
/// stage + rule + payload + identity; the store stamps the timestamp, the
/// attempt count is 0 on the first quarantine — retry counting is the
/// reprocessing path's concern, not the rejecting pipeline's).
fn quarantine_entry(record: &NormalizedTelemetry, reason: &str) -> TelemetryQuarantineWrite {
    TelemetryQuarantineWrite {
        tenant_id: record.tenant_id(),
        asset_id: Some(record.asset_id()),
        event_time: Some(record.event_time()),
        source_id: Some(record.source_id().to_string()),
        source_sequence: i64::try_from(record.source_sequence()).ok(),
        stage: "5-duplicate-handling".to_string(),
        reason: reason.to_string(),
        raw_payload: record_evidence(record),
        attempt_count: 0,
    }
}

/// A structured, serializer-free evidence rendering of the record (the
/// application crate depends on no serialization library; the composition
/// root may enrich the evidence with the raw wire payload).
fn record_evidence(record: &NormalizedTelemetry) -> String {
    format!(
        "{{\"asset_id\":\"{}\",\"quantity\":\"{}\",\"event_time_millis\":{},\
         \"source_id\":\"{}\",\"source_sequence\":{},\"quality\":\"{:?}\"}}",
        record.asset_id().uuid().to_hyphenated(),
        quantity_key(record.quantity()),
        record.event_time().unix_millis(),
        record.source_id(),
        record.source_sequence(),
        record.quality(),
    )
}

// --- Tests (ledger P2.09 acceptance criteria) ---------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use flexcore_contracts::ids::FlexUuid;
    use flexcore_contracts::time_model::{ClockQuality, IngestTime};
    use flexcore_contracts::units::{ActivePowerMW, EnergyMWh, StateOfCharge};
    use flexcore_domain::tenant_context::{AuthenticatedClaims, TenantContext, VerifiedClaims};
    use flexcore_ports::fakes::{FakeIngestionInbox, FakeTelemetryIngestStore};
    use flexcore_ports::telemetry_source::{NormalizedTelemetryParts, QuantityValue};

    const TENANT_UUID: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a9b";
    const ASSET_UUID: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a8c";

    fn tenant_id() -> flexcore_domain::ids::TenantId {
        flexcore_domain::ids::TenantId::new(FlexUuid::parse(TENANT_UUID).unwrap())
    }

    fn asset_id() -> AssetId {
        AssetId::new(FlexUuid::parse(ASSET_UUID).unwrap())
    }

    fn ctx() -> TenantContext {
        TenantContext::from_authenticated_claims(AuthenticatedClaims::from_verified(
            VerifiedClaims {
                tenant_id: tenant_id(),
                subject: "test:dedup".to_string(),
            },
        ))
    }

    fn event(millis: i64) -> EventTime {
        EventTime::from_unix_millis(millis).unwrap()
    }

    fn ingest(millis: i64) -> IngestTime {
        IngestTime::from_unix_millis(millis).unwrap()
    }

    fn record_parts(event_millis: i64, sequence: u64, value_mw: f64) -> NormalizedTelemetryParts {
        NormalizedTelemetryParts {
            tenant_id: tenant_id(),
            asset_id: asset_id(),
            quantity: flexcore_ports::telemetry_source::TelemetryQuantity::ActivePower,
            value: Some(QuantityValue::Power(ActivePowerMW::new(value_mw).unwrap())),
            event_time: event(event_millis),
            ingest_time: ingest(event_millis + 1),
            source_sequence: sequence,
            source_id: "sim://exit-gate".to_string(),
            quality: TelemetryQuality::Good,
            source_quality_code: None,
            clock_quality: ClockQuality::Synchronized,
        }
    }

    fn record(event_millis: i64, sequence: u64, value_mw: f64) -> NormalizedTelemetry {
        NormalizedTelemetry::new(record_parts(event_millis, sequence, value_mw)).unwrap()
    }

    fn soc_record(event_millis: i64, sequence: u64, fraction: f64) -> NormalizedTelemetry {
        let mut parts = record_parts(event_millis, sequence, 0.0);
        parts.quantity = flexcore_ports::telemetry_source::TelemetryQuantity::Soc;
        parts.value = Some(QuantityValue::Fraction(StateOfCharge::new(fraction).unwrap()));
        NormalizedTelemetry::new(parts).unwrap()
    }

    fn dedup() -> TelemetryDeduplicator {
        TelemetryDeduplicator::new("telemetry-ingestion-v1".to_string())
    }

    /// The caller-side half of the deduplicator's contract: record the
    /// batch's delivered keys in the inbox (post-commit in the pipeline).
    fn record_deliveries(d: &TelemetryDeduplicator, inbox: &FakeIngestionInbox, report: &DedupReport) {
        for key in &report.delivered_keys {
            inbox.record(d.consumer(), key).expect("inbox record");
        }
    }

    #[test]
    fn the_identity_is_asset_event_time_and_sequence() {
        let a = record(1_000, 1, 1.5);
        let identity = RecordIdentity::of(&a);
        assert_eq!(identity.asset_id, asset_id());
        assert_eq!(identity.event_time, event(1_000));
        assert_eq!(identity.source_sequence, 1);
        // The delivery key qualifies the identity with the row-grain
        // quantity and the source id (D-P3; P2.04 D-T2).
        let key = delivery_key(&a);
        assert!(key.contains("sim://exit-gate"));
        assert!(key.contains("active_power"));
        // A different quantity of the same sample gets a different key.
        assert_ne!(key, delivery_key(&soc_record(1_000, 1, 0.5)));
    }

    #[test]
    fn classification_is_exact_only_when_content_is_identical() {
        let a = record(1_000, 1, 1.5);
        let same = record(1_000, 1, 1.5);
        assert_eq!(
            duplicate_classification(&a, &same),
            DuplicateClassification::ExactDuplicate
        );
        // Same identity, different value -> conflicting.
        let other_value = record(1_000, 1, 2.5);
        assert_eq!(
            duplicate_classification(&a, &other_value),
            DuplicateClassification::ConflictingDuplicate
        );
        // Different identity -> not an exact duplicate either.
        let other_identity = record(2_000, 1, 1.5);
        assert_eq!(
            duplicate_classification(&a, &other_identity),
            DuplicateClassification::ConflictingDuplicate
        );
    }

    #[test]
    fn the_current_state_never_regresses_and_updates_on_newer() {
        // First value establishes the state.
        assert_eq!(
            current_state_transition(None, event(1_000)),
            CurrentStateTransition::UpdatesState
        );
        // A late NEWER event_time updates the state.
        assert_eq!(
            current_state_transition(Some(event(1_000)), event(2_000)),
            CurrentStateTransition::UpdatesState
        );
        // A late OLDER event_time is stored but never regresses the state.
        assert_eq!(
            current_state_transition(Some(event(2_000)), event(1_000)),
            CurrentStateTransition::RetainsState
        );
        // An equal event_time never rewinds the frontier either.
        assert_eq!(
            current_state_transition(Some(event(2_000)), event(2_000)),
            CurrentStateTransition::RetainsState
        );
    }

    /// Ledger P2.09 acceptance: "Exact-duplicate test: one row stored,
    /// second insert acknowledged without a second row."
    #[test]
    fn exact_duplicate_is_acknowledged_and_stored_once() {
        let store = FakeTelemetryIngestStore::new();
        let inbox = FakeIngestionInbox::new();
        let d = dedup();

        let first = d
            .process_batch(&ctx(), &[record(1_000, 1, 1.5)], &store, &inbox)
            .unwrap();
        assert_eq!(first.stored, 1);
        record_deliveries(&d, &inbox, &first);

        let second = d
            .process_batch(&ctx(), &[record(1_000, 1, 1.5)], &store, &inbox)
            .unwrap();
        assert_eq!(second.exact_duplicates_acknowledged, 1);
        assert_eq!(second.stored, 0);
        record_deliveries(&d, &inbox, &second);

        // ONE row, despite two identical deliveries.
        assert_eq!(store.stored().len(), 1);
    }

    /// Ledger P2.09 acceptance: "Conflicting-duplicate test: quarantined
    /// with a reason; stored value unchanged."
    #[test]
    fn conflicting_duplicate_is_quarantined_and_the_stored_value_is_unchanged() {
        let store = FakeTelemetryIngestStore::new();
        let inbox = FakeIngestionInbox::new();
        let d = dedup();

        d.process_batch(&ctx(), &[record(1_000, 1, 1.5)], &store, &inbox)
            .unwrap();
        let report = d
            .process_batch(&ctx(), &[record(1_000, 1, 9.9)], &store, &inbox)
            .unwrap();

        assert_eq!(report.conflicting_quarantined, 1);
        assert_eq!(report.stored, 0);
        // The stored value is UNCHANGED (never overwritten).
        let stored = &store.stored()[0];
        assert_eq!(
            stored.value(),
            Some(&QuantityValue::Power(ActivePowerMW::new(1.5).unwrap()))
        );
        // The quarantine carries the recorded reason and the full identity.
        let quarantined = store.quarantined();
        assert_eq!(quarantined.len(), 1);
        assert_eq!(quarantined[0].reason, CONFLICTING_DUPLICATE_REASON);
        assert_eq!(quarantined[0].asset_id, Some(asset_id()));
        assert_eq!(quarantined[0].source_sequence, Some(1));
        assert_eq!(quarantined[0].stage, "5-duplicate-handling");
    }

    /// Ledger P2.09 acceptance: "Out-of-order test: late older record
    /// stored; current state unchanged; late newer record updates state."
    #[test]
    fn late_older_records_are_stored_and_never_regress_the_current_state() {
        let store = FakeTelemetryIngestStore::new();
        let inbox = FakeIngestionInbox::new();
        let d = dedup();
        let active_power = flexcore_ports::telemetry_source::TelemetryQuantity::ActivePower;

        // Arrival 1: the CURRENT event_time (state frontier).
        let first = d
            .process_batch(&ctx(), &[record(2_000, 2, 2.0)], &store, &inbox)
            .unwrap();
        assert_eq!(first.state_updates, 1);
        record_deliveries(&d, &inbox, &first);

        // Arrival 2: a LATE OLDER record — stored as valid history, the
        // current state is not regressed.
        let late = d
            .process_batch(&ctx(), &[record(1_000, 1, 1.0)], &store, &inbox)
            .unwrap();
        assert_eq!(late.stored, 1);
        assert_eq!(
            late.dispositions[0],
            RecordDisposition::Stored {
                transition: CurrentStateTransition::RetainsState,
                previous_quality: Some(TelemetryQuality::Good),
                quality_changed: false,
            }
        );
        record_deliveries(&d, &inbox, &late);

        // Arrival 3: a late NEWER record — the state moves forward.
        let newer = d
            .process_batch(&ctx(), &[record(3_000, 3, 3.0)], &store, &inbox)
            .unwrap();
        assert_eq!(newer.state_updates, 1);
        record_deliveries(&d, &inbox, &newer);

        // Three rows stored (valid history), and the latest-record read
        // rule (latest event_time, never latest arrival) reports the
        // NEWER record as the current state.
        assert_eq!(store.stored().len(), 3);
        let latest = store
            .latest_record(&ctx(), asset_id(), active_power)
            .unwrap()
            .expect("a current state exists");
        assert_eq!(latest.event_time, event(3_000));
    }

    /// Ledger P2.09 acceptance: "Redelivery test: one row after
    /// redelivery" (the inbox pattern: the forced redelivery is detected
    /// and skipped).
    #[test]
    fn a_forced_redelivery_is_detected_and_not_re_stored() {
        let store = FakeTelemetryIngestStore::new();
        let inbox = FakeIngestionInbox::new();
        let d = dedup();

        let first = d
            .process_batch(&ctx(), &[record(1_000, 1, 1.5)], &store, &inbox)
            .unwrap();
        // The pipeline records deliveries after its commit (contract note).
        record_deliveries(&d, &inbox, &first);
        assert_eq!(store.stored().len(), 1);

        // Forced redelivery: the SAME record arrives again.
        let redelivered = d
            .process_batch(&ctx(), &[record(1_000, 1, 1.5)], &store, &inbox)
            .unwrap();
        assert_eq!(redelivered.redeliveries_skipped, 1);
        assert_eq!(redelivered.stored, 0);
        // Still ONE row.
        assert_eq!(store.stored().len(), 1);
    }

    /// A redelivery of a batch LOST to a rollback (no deliveries recorded)
    /// is made harmless by the store-level dedup: acknowledged, one row.
    #[test]
    fn a_delivery_lost_to_a_rollback_is_made_harmless_by_the_store_dedup() {
        let store = FakeTelemetryIngestStore::new();
        let inbox = FakeIngestionInbox::new();
        let d = dedup();

        // First batch processed but the commit failed: no deliveries
        // recorded (the caller records only after commit).
        d.process_batch(&ctx(), &[record(1_000, 1, 1.5)], &store, &inbox)
            .unwrap();

        // Redelivery: not skipped by the inbox, acknowledged by the store.
        let again = d
            .process_batch(&ctx(), &[record(1_000, 1, 1.5)], &store, &inbox)
            .unwrap();
        assert_eq!(again.redeliveries_skipped, 0);
        assert_eq!(again.exact_duplicates_acknowledged, 1);
        assert_eq!(store.stored().len(), 1);
    }

    /// A batch mixing quantities of one sample: both rows stored, each
    /// with its own delivery key (the row-grain rule).
    #[test]
    fn sibling_quantities_of_one_sample_are_independent_rows_and_keys() {
        let store = FakeTelemetryIngestStore::new();
        let inbox = FakeIngestionInbox::new();
        let d = dedup();

        let report = d
            .process_batch(
                &ctx(),
                &[record(1_000, 1, 1.5), soc_record(1_000, 1, 0.5)],
                &store,
                &inbox,
            )
            .unwrap();
        assert_eq!(report.stored, 2);
        assert_eq!(store.stored().len(), 2);
        assert_eq!(report.delivered_keys.len(), 2);
        assert_ne!(report.delivered_keys[0], report.delivered_keys[1]);
    }

    /// A quality change on a newer record is reported (the
    /// TelemetryQualityChanged trigger, P2.08's quality_state_changed).
    #[test]
    fn a_quality_state_change_on_a_stored_record_is_reported() {
        let store = FakeTelemetryIngestStore::new();
        let inbox = FakeIngestionInbox::new();
        let d = dedup();

        let first = d
            .process_batch(&ctx(), &[record(1_000, 1, 1.5)], &store, &inbox)
            .unwrap();
        assert!(!first.quality_changes, "a first value changed from nothing");
        record_deliveries(&d, &inbox, &first);

        let mut suspect = record_parts(2_000, 2, 9.9);
        suspect.quality = TelemetryQuality::Suspect;
        suspect.source_quality_code = Some("OUT_OF_RANGE".to_string());
        let report = d
            .process_batch(
                &ctx(),
                &[NormalizedTelemetry::new(suspect).unwrap()],
                &store,
                &inbox,
            )
            .unwrap();
        assert_eq!(report.quality_changes, 1);
        match &report.dispositions[0] {
            RecordDisposition::Stored {
                previous_quality,
                quality_changed,
                ..
            } => {
                assert_eq!(*previous_quality, Some(TelemetryQuality::Good));
                assert!(*quality_changed);
            }
            other => panic!("expected a Stored disposition, got {other:?}"),
        }
    }

    /// Records of a foreign tenant are refused by the store port (the
    /// structural tenant-scoping rule, P0.16 rule 1) — never stored.
    #[test]
    fn a_foreign_tenant_record_is_refused_not_stored() {
        let store = FakeTelemetryIngestStore::new();
        let inbox = FakeIngestionInbox::new();
        let d = dedup();

        let mut foreign = record_parts(1_000, 1, 1.5);
        foreign.tenant_id = flexcore_domain::ids::TenantId::new(FlexUuid::from_bytes([0xEE; 16]));
        let foreign = NormalizedTelemetry::new(foreign).unwrap();

        let error = d
            .process_batch(&ctx(), &[foreign], &store, &inbox)
            .unwrap_err();
        assert!(matches!(
            error,
            DedupError::Store(TelemetryIngestError::TenantMismatch)
        ));
        assert!(store.stored().is_empty());
    }

    /// Energy records exercise a second value kind through the same path
    /// (MWh canonical unit, P0.07 §1).
    #[test]
    fn energy_records_flow_through_the_same_dedup_path() {
        let store = FakeTelemetryIngestStore::new();
        let inbox = FakeIngestionInbox::new();
        let d = dedup();

        let mut parts = record_parts(1_000, 1, 0.0);
        parts.quantity = flexcore_ports::telemetry_source::TelemetryQuantity::Energy;
        parts.value = Some(QuantityValue::Energy(EnergyMWh::new(0.25).unwrap()));
        let energy = NormalizedTelemetry::new(parts).unwrap();

        let report = d.process_batch(&ctx(), &[energy], &store, &inbox).unwrap();
        assert_eq!(report.stored, 1);
        assert_eq!(store.stored().len(), 1);
    }

    /// Site ids are not part of the dedup identity (the asset is); two
    /// records of DIFFERENT assets never collide even with identical
    /// event_time and sequence.
    #[test]
    fn identical_identity_on_a_different_asset_is_not_a_duplicate() {
        let store = FakeTelemetryIngestStore::new();
        let inbox = FakeIngestionInbox::new();
        let d = dedup();

        let mut other_asset = record_parts(1_000, 1, 1.5);
        other_asset.asset_id = AssetId::new(FlexUuid::from_bytes([0xAB; 16]));
        let other_asset = NormalizedTelemetry::new(other_asset).unwrap();

        let report = d
            .process_batch(
                &ctx(),
                &[record(1_000, 1, 1.5), other_asset],
                &store,
                &inbox,
            )
            .unwrap();
        assert_eq!(report.stored, 2);
        assert_eq!(report.exact_duplicates_acknowledged, 0);
        assert_eq!(store.stored().len(), 2);
    }
}
