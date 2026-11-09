// P2.10 — The telemetry ingestion service: the P2.07 pipeline (receive →
// validate → normalize → quality-assign → dedup → store → publish) as a
// worker task, with quarantine and backpressure (ledger P2.10).
//
// # Recorded decision: worker task, not a dedicated binary
//
// Ledger P2.10 step 1 fixes the default: the ingestion service is a worker
// task in `apps/worker`, reusing the P1.19 hosting (shutdown watch,
// bounded drain, probe server) instead of a second process. This module is
// that task.
//
// # The pipeline (P2.07 §1, stages 1–7)
//
// | # | Stage | Where | Evidence |
// |---|---|---|---|
// | 1 | Receive (+ source authentication) | [`IngestionTask::run`] / [`authenticate_source`] | The source identity comes from the `TelemetrySource` port's provider declaration; the Phase 2 authentication is a RECORDED STUB (see [`SOURCE_AUTH_STATUS`]) — health + optional trusted-source-id check only. A rejected frame is quarantined (stage `1-receive`) and counted, never silently dropped |
// | 2 | Schema validation | the port's `decode` | Decode failure → quarantine (stage `2-validate`) + count; never a partial record (P2.11 step 3) |
// | 3 | Normalize (unit/sign) | the port's `decode` | The port emits canonical records (P2.04 §3, canonical units P0.07); the mapping onto the storage record is 1:1 in [`UowTelemetryIngestStore`] |
// | 4 | Quality assignment | [`refine_quality`] | Calls the P2.08 pure rules (`flexcore_domain::quality_rules::assign_quality`); the pipeline can only ever HOLD or LOWER a provider-assigned state, never upgrade it (the non-silencing rule, P0.08) |
// | 5 | Duplicate / out-of-order handling | [`TelemetryDeduplicator`] (P2.09) | Exact duplicate → acknowledged, one row; conflicting → quarantined; late older → stored, state not regressed |
// | 6 | Store | [`UowTelemetryIngestStore`] over the P2.06 repository | Inside ONE unit-of-work transaction with the quarantine writes and the outbox appends (all-or-nothing) |
// | 7 | Publish | [`publish_events`] | `TelemetryReceived` + `TelemetryQualityChanged` (previous+new, P2.16 step 2) + `AssetAvailabilityChanged` (P2.16 step 3) via the OUTBOX port only — never a direct broker publish (report §6.3) |
//
// # Backpressure (P2.07 §5)
//
// A bounded channel (default capacity **10,000** frames per ingestion
// worker instance, configuration-overridable via
// `FLEXCORE_INGEST_CHANNEL_CAPACITY`) sits between the receive side
// ([`IngestionQueue::offer`], called by the source bridge) and the
// pipeline loop. At the limit the frame is SHED with a retryable error and
// the shed metric increments — the source retries with backoff. Unbounded
// buffering and silent dropping are both prohibited.
//
// # Source wiring (P2.10 step 5, recorded; composition landed P2.25/P2.31)
//
// The service consumes ONLY the `TelemetrySource` port; no simulator code
// is imported anywhere in this crate. The simulator source (P2.14) now
// wires in at composition: `main.rs` `compose_ingestion` constructs the
// port object and the subscription query when `FLEXCORE_SIMULATOR_SCENARIO`
// is configured (scenario path, optional seed/horizon overrides; the
// tenant and the scenario-key → registry-ID mapping resolved from the
// registry per P2.25 step 2) and spawns the pull-side source bridge over
// `IngestionQueue`. With no scenario configured the worker runs without
// the ingestion task, recorded honestly in the startup log. This module
// itself remains source-agnostic — the port-only rule is unchanged
// (decision note: `ledger/phase-02-asset-telemetry/24_worker_source_wiring.md`).
//
// # Event granularity — DECIDED ONCE (P2.16 step 1)
//
// `TelemetryReceived` is emitted per STORED record (not per batch) with a
// deterministic idempotency key (redelivery-safe through the outbox's key
// uniqueness). Rationale: the frozen P1.09 `TelemetryReceivedPayload`
// carries a single (tenant, site, asset, event_time) — a batch-level event
// cannot name multiple assets without changing the frozen payload shape;
// and Phase 2 telemetry arrives at the bounded scenario cadence
// (P2.13/P2.14), so the high-frequency event-volume risk the ledger names
// does not arise in Phase 2. Recorded in the flexcore-events README; a
// future batch emission would be a schema-version change, never a silent
// reshaping.
//
// # Quality and availability events (P2.16 steps 2–3)
//
// `TelemetryQualityChanged` carries BOTH the previous and the new state:
// the additive `previous_quality` payload field (serde default for wire
// compatibility, schema version stays 1 — the P2.16 revision) is set from
// the dedup report's previous-state snapshot.
//
// `AssetAvailabilityChanged` is emitted when the availability rule
// (`flexcore_application::availability`, DECIDED ONCE — latest quality +
// availability value + source connectivity; STALE/MISSING leave the
// previous availability unchanged, BAD is never trusted) computes a
// CHANGED availability for a stored record that moved the current state
// forward. The pre-batch availability record is read in the same unit of
// work BEFORE the dedup stage inserts (the post-insert read would see the
// incoming row); the frozen payload carries scope + time only (no
// availability value — recorded assumption, flexcore-events README).
//
// All three events are appended to the OUTBOX inside the SAME unit of work
// as the store stage (stage 6 + 7, all-or-nothing — report §6.3; never a
// direct broker publish).
//
// # Sync-port hosting
//
// Same mechanism as `runner`: the sync ports (unit-of-work repository,
// inbox store) block on private runtimes, so every port-touching section
// of the loop runs under `tokio::task::block_in_place` (multi-thread
// runtime required — same contract as the P1.19 loops).
//
// # Observability instrumentation (P2.21)
//
// The pipeline records the telemetry metric families on the
// [`IngestionMetrics`] handle it already owns (same names as the
// control-plane registry's P2.21 families — one naming space, no second
// vocabulary). Cardinality is decided ONCE (recorded in
// `ledger/phase-02-asset-telemetry/21_22_observability_security.md`):
// ingestion delay is a per-batch summary over STORED records (per-asset
// breakdown in the per-record log lines, never labels); quality counts are
// per-STATE totals (all six canonical states); quarantine counts carry a
// closed reason-class label; staleness is a bounded observation counter —
// the per-asset staleness series lives in the control-plane registry and
// the per-asset signal travels as the log line here (P2.21 step 3: a
// metric + log, never a hard-coded alerting threshold — the freshness
// limit is deployment configuration, [`IngestionConfig`]).

use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use flexcore_application::availability::{
    availability, availability_changed, AvailabilitySignalInput, AvailabilityState,
};
use flexcore_application::error::ApplicationError;
use flexcore_application::payload_encoding::EventPayloadEncoder;
use flexcore_application::telemetry_dedup::{
    CurrentStateTransition, DedupError, DedupReport, RecordDisposition, TelemetryDeduplicator,
};
use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::quality::TelemetryQuality;
use flexcore_contracts::time_model::EventTime;
use flexcore_domain::ids::AssetId;
use flexcore_domain::quality_rules::{
    assign_quality, staleness, CheckResult, FreshnessLimit, QualityInput,
};
use flexcore_events::envelope::{EventEnvelope, SUPPORTED_SCHEMA_VERSION};
use flexcore_events::events::{
    AssetAvailabilityChangedPayload, DomainEvent, TelemetryQualityChangedPayload,
    TelemetryReceivedPayload,
};
use flexcore_events::subject::subject_for;
use flexcore_ports::clock::Clock;
use flexcore_ports::event_outbox::{EventOutbox, EventOutboxError, OutboxWrite};
use flexcore_ports::id_generator::IdGenerator;
use flexcore_ports::repository::AssetRepository;
use flexcore_ports::state_change::{StateChangeOperation, StateChangeReceipt};
use flexcore_ports::telemetry_ingest::{
    IngestInboxError, IngestionInbox, TelemetryIngestError, TelemetryIngestItem,
    TelemetryIngestOutcome, TelemetryIngestStore, TelemetryLatestRecord, TelemetryQuarantineWrite,
};
use flexcore_ports::telemetry_read::quantity_key;
use flexcore_ports::telemetry_source::{
    NormalizedTelemetry, NormalizedTelemetryParts, QuantityValue, TelemetryQuantity,
    TelemetrySource, TelemetrySourceError, TelemetrySourceHealth, TelemetrySubscription,
};
use flexcore_postgres_adapter::telemetry::{
    Quantity as PgQuantity, StoredTelemetry, TelemetryBatchOutcome,
    TelemetryError as PgTelemetryError, TelemetryQuarantineEntry, TelemetryValue as PgTelemetryValue,
};
use flexcore_postgres_adapter::{FlexCoreStore, PgUnitOfWork};
use tokio::sync::{mpsc, watch};

/// The inbox consumer name of the ingestion pipeline (the delivery keys
/// are checked/recorded under it; P2.09 step 4).
pub const INGESTION_CONSUMER: &str = "telemetry-ingestion-v1";

/// The RECORDED Phase 2 source-authentication status (P2.10 step 6: record
/// the stub status honestly). Real cryptographic source authentication
/// arrives with real adapters in a later phase.
pub const SOURCE_AUTH_STATUS: &str = "STUB: Phase 2 authenticates the simulator source by \
     provider declaration and (optional) trusted-source-id configuration only — no \
     cryptographic source authentication exists yet; real source authentication is a later \
     phase (recorded per ledger P2.10 step 6)";

/// The telemetry state-change entity name for the outbox receipts (the
/// P2.06 table; outside the Stage 1 `entities` vocabulary on purpose).
const TELEMETRY_ENTITY: &str = "telemetry";

// --- Configuration ------------------------------------------------------------

/// The ingestion tuning. Every value is deployment configuration; the
/// defaults record the P2.07 §5 decisions.
#[derive(Debug, Clone)]
pub struct IngestionConfig {
    /// The bounded channel capacity between receive and the pipeline loop
    /// (P2.07 §5 default: 10,000 buffered frames per ingestion worker
    /// instance).
    pub channel_capacity: usize,
    /// When set, only a provider declaring this exact source identity is
    /// accepted (the Phase 2 stub authentication's configuration knob).
    pub trusted_source_id: Option<String>,
    /// Bounded retries for transient store failures per frame, with
    /// exponential backoff between attempts. After the budget is exhausted
    /// the frame is counted (`frames_failed`) and logged at error level —
    /// never silently dropped; the source's next emission redelivers.
    pub store_max_attempts: u32,
    pub store_retry_base: Duration,
    pub store_retry_cap: Duration,
    /// The pipeline's freshness limit in milliseconds for the P2.21
    /// staleness signal (P2.08: per-use-case configuration, NEVER a
    /// hard-coded constant). When set, every decoded record's age
    /// (`now - event_time`, the P2.08 staleness rule) is evaluated and a
    /// stale record increments the staleness metric and emits the
    /// staleness log line; the stored quality state is NOT changed here
    /// (staleness is a view-time signal, P2.08). `None` = the pipeline
    /// performs no staleness evaluation.
    pub stale_freshness_limit_millis: Option<i64>,
}

impl Default for IngestionConfig {
    fn default() -> Self {
        Self {
            channel_capacity: 10_000,
            trusted_source_id: None,
            store_max_attempts: 5,
            store_retry_base: Duration::from_millis(200),
            store_retry_cap: Duration::from_secs(5),
            stale_freshness_limit_millis: None,
        }
    }
}

// --- Metrics ------------------------------------------------------------------

/// The canonical quality-state labels, in frozen P0.08 vocabulary order.
/// Shared by the per-state counters and the Prometheus rendering.
pub const QUALITY_STATE_LABELS: [&str; 6] =
    ["GOOD", "SUSPECT", "BAD", "MISSING", "ESTIMATED", "STALE"];

/// The frozen telemetry event types (§15.3) counted by the outbox-events
/// family (same names as the control-plane registry's family).
pub const TELEMETRY_EVENT_LABELS: [&str; 3] = [
    "TelemetryReceived",
    "TelemetryQualityChanged",
    "AssetAvailabilityChanged",
];

/// The closed quarantine reason classes (the P2.21 cardinality decision:
/// the free-text quarantine reason NEVER becomes a metric label — the
/// classification maps the rejection's stage and rule onto one of these).
pub const QUARANTINE_REASON_CLASSES: [&str; 5] = [
    "source_rejected",
    "decode_failed",
    "conflicting_duplicate",
    "out_of_range_event_time",
    "store_rejected",
];

/// The quality enum's index into [`QUALITY_STATE_LABELS`].
fn quality_state_index(q: TelemetryQuality) -> usize {
    match q {
        TelemetryQuality::Good => 0,
        TelemetryQuality::Suspect => 1,
        TelemetryQuality::Bad => 2,
        TelemetryQuality::Missing => 3,
        TelemetryQuality::Estimated => 4,
        TelemetryQuality::Stale => 5,
    }
}

/// Classifies a quarantine rejection onto the closed reason class
/// vocabulary (module-header cardinality decision). The stage names are
/// the pipeline's own; the out-of-range rule text is the storage adapter's
/// frozen rejection reason.
fn quarantine_reason_class(stage: &str, reason: &str) -> &'static str {
    match stage {
        "1-receive" => "source_rejected",
        "2-validate" => "decode_failed",
        "5-duplicate-handling" => {
            if reason.contains("outside every existing telemetry partition") {
                "out_of_range_event_time"
            } else {
                "conflicting_duplicate"
            }
        }
        _ => "store_rejected",
    }
}

/// The ingestion counters (the P2.21 metric surface reads these; the
/// quarantine count is the P2.07 §3 requirement).
#[derive(Default)]
pub struct IngestionMetrics {
    pub frames_received: AtomicU64,
    pub records_stored: AtomicU64,
    pub duplicates_acknowledged: AtomicU64,
    pub redeliveries_skipped: AtomicU64,
    pub records_quarantined: AtomicU64,
    pub frames_shed: AtomicU64,
    pub frames_rejected_auth: AtomicU64,
    pub quality_changed_events: AtomicU64,
    pub availability_changed_events: AtomicU64,
    pub received_events: AtomicU64,
    pub store_retries: AtomicU64,
    pub frames_failed: AtomicU64,
    // --- P2.21 telemetry observability ------------------------------------
    /// Per-batch ingestion-delay summary over STORED records
    /// (`ingest_time - event_time`, ms; negative deltas clamp to 0).
    pub ingestion_delay_ms_sum: AtomicU64,
    pub ingestion_delay_ms_count: AtomicU64,
    /// Ingestion bounded-channel length at the last observation (gauge).
    pub queue_depth: AtomicU64,
    /// Records whose age exceeded the configured freshness limit (the
    /// P2.21 staleness signal; thresholds are deployment configuration).
    pub stale_observations: AtomicU64,
    /// Stored records per canonical quality state (per-state totals — the
    /// recorded cardinality decision; per-asset counts go to logs).
    pub quality_states: [AtomicU64; 6],
    /// Quarantined records by closed reason class.
    pub quarantine_by_reason: Mutex<BTreeMap<String, AtomicU64>>,
}

/// A plain snapshot of the counters (test/ops inspection).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct IngestionMetricsSnapshot {
    pub frames_received: u64,
    pub records_stored: u64,
    pub duplicates_acknowledged: u64,
    pub redeliveries_skipped: u64,
    pub records_quarantined: u64,
    pub frames_shed: u64,
    pub frames_rejected_auth: u64,
    pub quality_changed_events: u64,
    pub availability_changed_events: u64,
    pub received_events: u64,
    pub store_retries: u64,
    pub frames_failed: u64,
    /// Per-batch ingestion-delay summary (sum, count) in milliseconds.
    pub ingestion_delay_ms_sum: u64,
    pub ingestion_delay_ms_count: u64,
    pub queue_depth: u64,
    pub stale_observations: u64,
    /// Stored records per canonical quality state (frozen order).
    pub quality_states: [u64; 6],
}

impl IngestionMetrics {
    /// A consistent-enough snapshot (individual counters may lag a read by
    /// one increment; the counts are monotonic). The per-reason quarantine
    /// map is read separately via [`IngestionMetrics::quarantine_reasons`].
    pub fn snapshot(&self) -> IngestionMetricsSnapshot {
        IngestionMetricsSnapshot {
            frames_received: self.frames_received.load(Ordering::SeqCst),
            records_stored: self.records_stored.load(Ordering::SeqCst),
            duplicates_acknowledged: self.duplicates_acknowledged.load(Ordering::SeqCst),
            redeliveries_skipped: self.redeliveries_skipped.load(Ordering::SeqCst),
            records_quarantined: self.records_quarantined.load(Ordering::SeqCst),
            frames_shed: self.frames_shed.load(Ordering::SeqCst),
            frames_rejected_auth: self.frames_rejected_auth.load(Ordering::SeqCst),
            quality_changed_events: self.quality_changed_events.load(Ordering::SeqCst),
            availability_changed_events: self.availability_changed_events.load(Ordering::SeqCst),
            received_events: self.received_events.load(Ordering::SeqCst),
            store_retries: self.store_retries.load(Ordering::SeqCst),
            frames_failed: self.frames_failed.load(Ordering::SeqCst),
            ingestion_delay_ms_sum: self.ingestion_delay_ms_sum.load(Ordering::SeqCst),
            ingestion_delay_ms_count: self.ingestion_delay_ms_count.load(Ordering::SeqCst),
            queue_depth: self.queue_depth.load(Ordering::SeqCst),
            stale_observations: self.stale_observations.load(Ordering::SeqCst),
            quality_states: std::array::from_fn(|i| self.quality_states[i].load(Ordering::SeqCst)),
        }
    }

    /// The quarantine counters per closed reason class.
    pub fn quarantine_reasons(&self) -> BTreeMap<String, u64> {
        self.quarantine_by_reason
            .lock()
            .expect("metrics mutex poisoned")
            .iter()
            .map(|(k, v)| (k.clone(), v.0.load(Ordering::SeqCst)))
            .collect()
    }

    /// Counts one stored record under its canonical quality state.
    pub fn record_quality_state(&self, q: TelemetryQuality) {
        self.quality_states[quality_state_index(q)].fetch_add(1, Ordering::SeqCst);
    }

    /// Counts one quarantined record under its closed reason class.
    pub fn record_quarantine_reason(&self, reason_class: &str) {
        let mut map = self
            .quarantine_by_reason
            .lock()
            .expect("metrics mutex poisoned");
        let entry = map
            .entry(reason_class.to_string())
            .or_insert_with(|| AtomicU64::new(0));
        entry.0.fetch_add(1, Ordering::SeqCst);
    }

    /// Records the ingestion delay of one stored record
    /// (`ingest_time - event_time`, ms; a negative delta — the source clock
    /// ahead of the event clock — clamps to 0; clock skew is a separate
    /// signal, P2.08/P2.14).
    pub fn record_ingestion_delay_millis(&self, delay_millis: i128) {
        let delay = delay_millis.clamp(0, u64::MAX as i128) as u64;
        self.ingestion_delay_ms_sum.fetch_add(delay, Ordering::SeqCst);
        self.ingestion_delay_ms_count.fetch_add(1, Ordering::SeqCst);
    }

    /// Sets the queue-depth gauge (the bounded channel's current length).
    pub fn set_queue_depth(&self, depth: usize) {
        self.queue_depth.store(depth as u64, Ordering::SeqCst);
    }

    /// Renders the telemetry metric families in Prometheus text format,
    /// using the SAME metric names as the control-plane registry's P2.21
    /// families (one naming space; the worker's future /metrics surface
    /// renders these, the control plane's `/metrics` renders its own
    /// registry — the ledger P2.21 acceptance scrapes the control plane).
    pub fn render_prometheus(&self) -> String {
        let mut out = String::new();
        out.push_str("# HELP flexcore_telemetry_events_total Telemetry domain events appended to the outbox, by frozen event type.\n");
        out.push_str("# TYPE flexcore_telemetry_events_total counter\n");
        let event_values = [
            self.received_events.load(Ordering::SeqCst),
            self.quality_changed_events.load(Ordering::SeqCst),
            self.availability_changed_events.load(Ordering::SeqCst),
        ];
        for (label, value) in TELEMETRY_EVENT_LABELS.iter().zip(event_values) {
            out.push_str(&format!(
                "flexcore_telemetry_events_total{{event=\"{label}\"}} {value}\n"
            ));
        }
        let quarantined = self.records_quarantined.load(Ordering::SeqCst);
        out.push_str("# HELP flexcore_telemetry_quarantine_total Total telemetry records quarantined by the ingestion pipeline.\n");
        out.push_str("# TYPE flexcore_telemetry_quarantine_total counter\n");
        out.push_str(&format!("flexcore_telemetry_quarantine_total {quarantined}\n"));
        out.push_str("# HELP flexcore_telemetry_quarantine_by_reason_total Quarantined telemetry records by closed reason class (the free-text reason is never a label).\n");
        out.push_str("# TYPE flexcore_telemetry_quarantine_by_reason_total counter\n");
        let reasons = self.quarantine_reasons();
        for reason_class in QUARANTINE_REASON_CLASSES {
            let value = reasons.get(reason_class).copied().unwrap_or(0);
            out.push_str(&format!(
                "flexcore_telemetry_quarantine_by_reason_total{{reason_class=\"{reason_class}\"}} {value}\n"
            ));
        }
        let depth = self.queue_depth.load(Ordering::SeqCst);
        out.push_str("# HELP flexcore_telemetry_queue_depth Ingestion bounded-channel length at the last observation.\n");
        out.push_str("# TYPE flexcore_telemetry_queue_depth gauge\n");
        out.push_str(&format!("flexcore_telemetry_queue_depth {depth}\n"));
        out.push_str("# HELP flexcore_telemetry_quality_states_total Stored telemetry records by canonical quality state (per-state totals; per-asset counts go to logs).\n");
        out.push_str("# TYPE flexcore_telemetry_quality_states_total counter\n");
        for (index, state) in QUALITY_STATE_LABELS.iter().enumerate() {
            let v = self.quality_states[index].load(Ordering::SeqCst);
            out.push_str(&format!(
                "flexcore_telemetry_quality_states_total{{quality=\"{state}\"}} {v}\n"
            ));
        }
        let stale = self.stale_observations.load(Ordering::SeqCst);
        out.push_str("# HELP flexcore_telemetry_stale_observations_total Records whose age exceeded the deployment freshness limit (per-asset staleness series live in the control-plane registry; the per-asset signal is the log line).\n");
        out.push_str("# TYPE flexcore_telemetry_stale_observations_total counter\n");
        out.push_str(&format!("flexcore_telemetry_stale_observations_total {stale}\n"));
        let sum = self.ingestion_delay_ms_sum.load(Ordering::SeqCst);
        let count = self.ingestion_delay_ms_count.load(Ordering::SeqCst);
        out.push_str("# HELP flexcore_telemetry_ingestion_delay_milliseconds Per-batch summary of (ingest_time - event_time) in milliseconds over stored records; per-asset breakdown goes to logs.\n");
        out.push_str("# TYPE flexcore_telemetry_ingestion_delay_milliseconds summary\n");
        out.push_str(&format!("flexcore_telemetry_ingestion_delay_milliseconds_sum {sum}\n"));
        out.push_str(&format!(
            "flexcore_telemetry_ingestion_delay_milliseconds_count {count}\n"
        ));
        out
    }
}

// --- Stage 1: source authentication (recorded Phase 2 STUB) ---------------------

/// Why the receive stage rejected a frame's source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceAuthRejection {
    pub reason: String,
}

/// The stage-1 source authentication (Phase 2 RECORDED STUB — see
/// [`SOURCE_AUTH_STATUS`]). Two checks, both over the port contract only:
///
/// 1. the provider must not report itself `Unavailable`;
/// 2. when `trusted_source_id` is configured, the provider's declared
///    source identity must match it exactly.
///
/// Anything else accepts the frame's source — the honest stub boundary.
/// A rejected source is quarantined (stage `1-receive`) and counted; it is
/// never silently dropped.
pub fn authenticate_source(
    source: &dyn TelemetrySource,
    trusted_source_id: Option<&str>,
) -> Result<(), SourceAuthRejection> {
    if let TelemetrySourceHealth::Unavailable { reason } = source.health() {
        return Err(SourceAuthRejection {
            reason: format!("the telemetry source reports itself unavailable: {reason}"),
        });
    }
    if let Some(trusted) = trusted_source_id {
        if source.source_id() != trusted {
            return Err(SourceAuthRejection {
                reason: format!(
                    "the source identity '{}' is not the configured trusted source '{trusted}'",
                    source.source_id()
                ),
            });
        }
    }
    Ok(())
}

// --- Stage 4: quality assignment (calls the P2.08 rules) ------------------------

/// The severity rank of the states the quality assigner can produce
/// (P2.08: GOOD < SUSPECT < BAD). The other three states are produced by
/// other pipeline producers (staleness/gap detectors, explicit derivation)
/// and never by [`assign_quality`] — the producer separation, P2.08.
fn severity(q: TelemetryQuality) -> Option<u8> {
    match q {
        TelemetryQuality::Good => Some(0),
        TelemetryQuality::Suspect => Some(1),
        TelemetryQuality::Bad => Some(2),
        _ => None,
    }
}

/// The more severe of two quality states. The pipeline holds or lowers a
/// provider-assigned state, never upgrades it: a source-reported
/// degradation must survive the rule stage (the non-silencing rule).
fn worse_of(a: TelemetryQuality, b: TelemetryQuality) -> TelemetryQuality {
    match (severity(a), severity(b)) {
        (Some(x), Some(y)) if x >= y => a,
        (Some(_), Some(_)) => b,
        (Some(_), None) => a,
        (None, Some(_)) => b,
        (None, None) => a,
    }
}

/// Stage 4: quality assignment for one canonical record (P2.07 stage 4 —
/// the rules are the P2.08 pure functions; this is the pipeline's input
/// assembly and the hold-or-lower combination).
///
/// Recorded stage inputs (Phase 2 ingestion):
/// - `schema_valid = true` — the port's decode guarantees a complete
///   canonical record (a shape failure never reaches this stage);
/// - `source_reported_invalid` — the provider's own quality mapping said
///   BAD (hard rule 3 of P2.08: source-reported invalid → BAD);
/// - `soc_raw_fraction = None` — the typed `StateOfCharge` enforces [0,1]
///   at construction (the raw pre-construction path does not exist here);
/// - range/consistency checks `NotApplicable` — Phase 2 ingestion runs no
///   rated-limit configuration; wiring the asset's rated limits in is a
///   recorded follow-up (P2.16/P2.22 scope).
pub fn refine_quality(record: &NormalizedTelemetry) -> NormalizedTelemetry {
    let decision = assign_quality(&QualityInput {
        schema_valid: true,
        source_reported_invalid: record.quality() == TelemetryQuality::Bad,
        soc_raw_fraction: None,
        range_check: CheckResult::NotApplicable,
        consistency_check: CheckResult::NotApplicable,
    });
    let effective = worse_of(record.quality(), decision.quality);
    if effective == record.quality() {
        return record.clone();
    }
    // Rebuild the record with the lowered state; the original source
    // quality code travels as diagnostic metadata (P0.08 §3).
    let parts = NormalizedTelemetryParts {
        tenant_id: record.tenant_id(),
        asset_id: record.asset_id(),
        quantity: record.quantity(),
        value: record.value().cloned(),
        event_time: record.event_time(),
        ingest_time: record.ingest_time(),
        source_sequence: record.source_sequence(),
        source_id: record.source_id().to_string(),
        quality: effective,
        source_quality_code: record.source_quality_code().map(str::to_string),
        clock_quality: record.clock_quality(),
    };
    NormalizedTelemetry::new(parts).expect("a lowered quality state cannot invalidate a \
         complete canonical record (GOOD/SUSPECT/BAD are all measured states)")
}

// --- Stage 2/3 evidence: quarantine ---------------------------------------------

/// The worker's db handle alias for readability.
type Db = FlexCoreStore;

/// One frame offered by the source bridge: the opaque raw payload plus the
/// receive timestamp.
#[derive(Debug)]
pub struct IngestFrame {
    pub payload: Vec<u8>,
}

/// The shed error of [`offer_bounded`] — RETRYABLE by contract: the source
/// backs off and re-offers (P2.07 §5; never unbounded buffering, never a
/// silent drop).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameShed;

/// The bounded backpressure boundary (P2.07 §5): a `try_send` against the
/// bounded channel; a full channel sheds the frame with [`FrameShed`] and
/// increments the shed metric.
fn offer_bounded(
    tx: &mpsc::Sender<IngestFrame>,
    metrics: &IngestionMetrics,
    payload: Vec<u8>,
) -> Result<(), FrameShed> {
    match tx.try_send(IngestFrame { payload }) {
        Ok(()) => Ok(()),
        Err(mpsc::error::TrySendError::Full(_)) => {
            metrics.frames_shed.fetch_add(1, Ordering::SeqCst);
            Err(FrameShed)
        }
        Err(mpsc::error::TrySendError::Closed(_)) => {
            // The pipeline task is gone: shedding is the honest report
            // (retrying into a closed channel would be a silent drop).
            metrics.frames_shed.fetch_add(1, Ordering::SeqCst);
            Err(FrameShed)
        }
    }
}

/// The receive-side handle the source bridge holds (P2.14 wires this). A
/// clone shares the bounded channel and the metrics.
#[derive(Clone)]
pub struct IngestionQueue {
    tx: mpsc::Sender<IngestFrame>,
    metrics: Arc<IngestionMetrics>,
}

impl IngestionQueue {
    /// Offers one frame to the ingestion pipeline. `Err(FrameShed)` at the
    /// bounded-channel limit (or after shutdown closed the channel):
    /// retryable, counted, never buffered without bound.
    pub fn offer(&self, payload: Vec<u8>) -> Result<(), FrameShed> {
        offer_bounded(&self.tx, &self.metrics, payload)
    }

    /// Whether the pipeline side has closed the channel (the ingestion task
    /// ended). A source bridge checks this before retrying a shed offer:
    /// re-offering into a closed channel would be a silent loss — retryable
    /// at the channel's limit, never past the channel's lifetime.
    pub fn is_closed(&self) -> bool {
        self.tx.is_closed()
    }
}

// --- The unit-of-work ingest store (the port's 1:1 storage mapping) -------------

/// Maps the [`TelemetryIngestStore`] port onto the P2.06 telemetry
/// repository of one open unit of work. Every call executes inside that
/// unit of work's native transaction (the port's unit-of-work binding
/// obligation); the record and quarantine mappings are the 1:1 conversions
/// the adapter's telemetry module header records.
pub struct UowTelemetryIngestStore<'a> {
    unit: &'a PgUnitOfWork,
}

impl<'a> UowTelemetryIngestStore<'a> {
    /// Borrows the open unit of work.
    pub fn new(unit: &'a PgUnitOfWork) -> Self {
        Self { unit }
    }
}

/// Maps a port quantity onto the frozen storage vocabulary (1:1).
fn pg_quantity(quantity: TelemetryQuantity) -> PgQuantity {
    match quantity {
        TelemetryQuantity::ActivePower => PgQuantity::ActivePower,
        TelemetryQuantity::ReactivePower => PgQuantity::ReactivePower,
        TelemetryQuantity::Soc => PgQuantity::Soc,
        TelemetryQuantity::Energy => PgQuantity::Energy,
        TelemetryQuantity::Availability => PgQuantity::Availability,
        TelemetryQuantity::OperatingMode => PgQuantity::OperatingMode,
        TelemetryQuantity::Health => PgQuantity::Health,
        TelemetryQuantity::ConnectionState => PgQuantity::ConnectionState,
        TelemetryQuantity::BaselineReference => PgQuantity::BaselineReference,
    }
}

/// Maps the canonical port record onto the storage record (P2.04 §3 field
/// set, 1:1 — see the adapter telemetry module header's mapping table).
fn pg_record(record: &NormalizedTelemetry) -> Result<flexcore_postgres_adapter::telemetry::NormalizedTelemetryRecord, TelemetryIngestError> {
    let source_sequence = i64::try_from(record.source_sequence()).map_err(|_| {
        TelemetryIngestError::Backend(flexcore_ports::backend_error::BackendError::new(
            "REJECTED".to_string(),
            "the record's source_sequence exceeds the storage bigint range".to_string(),
        ))
    })?;
    let (value, q_sign_convention) = match record.value() {
        Some(QuantityValue::Power(p)) => (Some(PgTelemetryValue::Numeric(p.value())), None),
        Some(QuantityValue::Reactive { power, convention }) => {
            (Some(PgTelemetryValue::Numeric(power.value())), Some(*convention))
        }
        Some(QuantityValue::Fraction(f)) => (Some(PgTelemetryValue::Numeric(f.value())), None),
        Some(QuantityValue::Energy(e)) => (Some(PgTelemetryValue::Numeric(e.value())), None),
        Some(QuantityValue::Flag(b)) => (Some(PgTelemetryValue::Flag(*b)), None),
        Some(QuantityValue::State(text)) => (Some(PgTelemetryValue::Text(text.clone())), None),
        // A MISSING gap marker (derived namespace only) carries no value.
        None => (None, None),
    };
    Ok(flexcore_postgres_adapter::telemetry::NormalizedTelemetryRecord {
        tenant_id: record.tenant_id(),
        asset_id: record.asset_id(),
        quantity: pg_quantity(record.quantity()),
        value,
        event_time: record.event_time(),
        ingest_time: record.ingest_time(),
        source_sequence,
        source_id: record.source_id().to_string(),
        quality: record.quality(),
        source_quality_code: record.source_quality_code().map(str::to_string),
        q_sign_convention,
        clock_quality: record.clock_quality(),
    })
}

fn map_telemetry_error(error: PgTelemetryError) -> TelemetryIngestError {
    match error {
        PgTelemetryError::TenantMismatch => TelemetryIngestError::TenantMismatch,
        PgTelemetryError::Backend(backend) => TelemetryIngestError::Backend(backend),
        // Conflicting-duplicate and out-of-range never surface as whole-call
        // errors from the batch API (they are per-item outcomes).
        other => TelemetryIngestError::Backend(flexcore_ports::backend_error::BackendError::new(
            "TELEMETRY_STORE_FAILED".to_string(),
            other.to_string(),
        )),
    }
}

impl<'a> TelemetryIngestStore for UowTelemetryIngestStore<'a> {
    fn insert_batch(
        &self,
        ctx: &flexcore_domain::tenant_context::TenantContext,
        records: &[NormalizedTelemetry],
    ) -> Result<Vec<TelemetryIngestItem>, TelemetryIngestError> {
        let batch: Vec<flexcore_postgres_adapter::telemetry::NormalizedTelemetryRecord> = records
            .iter()
            .map(pg_record)
            .collect::<Result<Vec<_>, _>>()?;
        let items = self
            .unit
            .insert_telemetry_batch(ctx, &batch)
            .map_err(map_telemetry_error)?;
        Ok(items
            .into_iter()
            .map(|item| TelemetryIngestItem {
                index: item.index,
                outcome: match item.outcome {
                    TelemetryBatchOutcome::Stored => TelemetryIngestOutcome::Stored,
                    TelemetryBatchOutcome::ExactDuplicateAcknowledged => {
                        TelemetryIngestOutcome::ExactDuplicateAcknowledged
                    }
                    TelemetryBatchOutcome::ConflictingDuplicate => {
                        TelemetryIngestOutcome::ConflictingDuplicate
                    }
                    TelemetryBatchOutcome::OutOfRangeEventTime => {
                        TelemetryIngestOutcome::OutOfRangeEventTime
                    }
                },
            })
            .collect())
    }

    fn quarantine(
        &self,
        ctx: &flexcore_domain::tenant_context::TenantContext,
        entry: &TelemetryQuarantineWrite,
    ) -> Result<(), TelemetryIngestError> {
        // The evidence payload travels as JSON where it parses; otherwise
        // verbatim as a JSON string (the column is JSONB).
        let raw_payload: serde_json::Value =
            serde_json::from_str(&entry.raw_payload).unwrap_or_else(|_| {
                serde_json::Value::String(entry.raw_payload.clone())
            });
        let pg_entry = TelemetryQuarantineEntry {
            tenant_id: entry.tenant_id,
            asset_id: entry.asset_id,
            event_time: entry.event_time,
            source_id: entry.source_id.clone(),
            source_sequence: entry.source_sequence,
            stage: entry.stage.clone(),
            reason: entry.reason.clone(),
            raw_payload,
            attempt_count: entry.attempt_count,
        };
        self.unit
            .quarantine_telemetry(ctx, &pg_entry)
            .map_err(map_telemetry_error)
    }

    fn latest_record(
        &self,
        ctx: &flexcore_domain::tenant_context::TenantContext,
        asset_id: AssetId,
        quantity: TelemetryQuantity,
    ) -> Result<Option<TelemetryLatestRecord>, TelemetryIngestError> {
        let rows = self
            .unit
            .latest_telemetry_per_asset(ctx, &[asset_id], Some(&[pg_quantity(quantity)]))
            .map_err(map_telemetry_error)?;
        Ok(rows.into_iter().next().map(|row| TelemetryLatestRecord {
            event_time: row.event_time,
            quality: row.quality,
        }))
    }
}

// --- The durable inbox over the worker's PgInboxStore -----------------------------

/// Maps the [`IngestionInbox`] port onto the worker's durable
/// [`flexcore_worker::inbox::PgInboxStore`] (the P1.16 inbox table; same
/// database as the store). The check/record split of the port maps 1:1
/// onto the store's `is_recorded`/`record`.
#[derive(Clone)]
pub struct PgIngestionInbox {
    inner: crate::inbox::PgInboxStore,
}

impl PgIngestionInbox {
    /// Wraps the worker's inbox store.
    pub fn new(inner: crate::inbox::PgInboxStore) -> Self {
        Self { inner }
    }
}

impl IngestionInbox for PgIngestionInbox {
    fn is_recorded(&self, consumer: &str, idempotency_key: &str) -> Result<bool, IngestInboxError> {
        use flexcore_nats_adapter::inbox::InboxStore;
        self.inner
            .is_recorded(consumer, idempotency_key)
            .map_err(|_| {
                IngestInboxError::Backend(flexcore_ports::backend_error::BackendError::new(
                    "INGEST_INBOX_FAILED".to_string(),
                    "the ingestion inbox check failed".to_string(),
                ))
            })
    }

    fn record(
        &self,
        consumer: &str,
        idempotency_key: &str,
    ) -> Result<flexcore_ports::telemetry_ingest::IngestDeliveryOutcome, IngestInboxError> {
        use flexcore_nats_adapter::inbox::{InboxStore, RecordOutcome};
        match self.inner.record(consumer, idempotency_key) {
            Ok(RecordOutcome::Inserted) => {
                Ok(flexcore_ports::telemetry_ingest::IngestDeliveryOutcome::FirstDelivery)
            }
            Ok(RecordOutcome::AlreadyRecorded) => {
                Ok(flexcore_ports::telemetry_ingest::IngestDeliveryOutcome::AlreadyRecorded)
            }
            Err(_) => Err(IngestInboxError::Backend(
                flexcore_ports::backend_error::BackendError::new(
                    "INGEST_INBOX_FAILED".to_string(),
                    "the ingestion inbox record failed".to_string(),
                ),
            )),
        }
    }
}

// --- Stage 7: publication (OUTBOX only) -------------------------------------------

/// Errors of one batch's stage 6+7 execution (the caller retries with
/// backoff; nothing was committed — the unit of work rolls back on drop).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IngestPipelineError {
    Store(TelemetryIngestError),
    Outbox(EventOutboxError),
    Encode(ApplicationError),
    Commit(String),
}

impl std::fmt::Display for IngestPipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IngestPipelineError::Store(e) => write!(f, "telemetry store failed: {e}"),
            IngestPipelineError::Outbox(e) => write!(f, "outbox append failed: {e}"),
            IngestPipelineError::Encode(_) => write!(f, "event payload encoding failed"),
            IngestPipelineError::Commit(reason) => write!(f, "unit-of-work commit failed: {reason}"),
        }
    }
}

impl std::error::Error for IngestPipelineError {}

/// Appends one outbox write, treating an idempotency-key collision as
/// "already published" (a redelivered batch re-runs stage 7 with the same
/// deterministic keys; the outbox's key uniqueness deduplicates — report
/// §6.3, P1.12 §4).
fn append_idempotent(unit: &PgUnitOfWork, write: OutboxWrite) -> Result<(), IngestPipelineError> {
    match unit.append(write) {
        Ok(()) => Ok(()),
        Err(EventOutboxError::DuplicateKey { .. }) => Ok(()),
        Err(other) => Err(IngestPipelineError::Outbox(other)),
    }
}

/// Resolves the site of a stored record's asset through the Stage 1 asset
/// repository (the `TelemetryReceived` payload carries a `site_id`; the
/// asset's existence is guaranteed by the telemetry composite FK).
fn site_of(unit: &PgUnitOfWork, ctx: &flexcore_domain::tenant_context::TenantContext, asset_id: AssetId) -> Option<flexcore_domain::ids::SiteId> {
    match AssetRepository::find_by_id(unit, ctx, asset_id) {
        Ok(Some(asset)) => Some(asset.site_id()),
        Ok(None) => None,
        Err(_) => None,
    }
}

/// The deterministic idempotency key of one stored record's events (the
/// outbox deduplicates a redelivered batch by key uniqueness).
fn event_key(event: &str, record: &NormalizedTelemetry) -> String {
    format!(
        "{event}:{}:{}:{}:{}",
        record.asset_id().uuid().to_hyphenated(),
        record.event_time().unix_millis(),
        record.source_sequence(),
        quantity_key(record.quantity()),
    )
}

/// The boolean value of a canonical record, when the quantity's value kind
/// is boolean (the `availability` signal, P2.04 §4).
fn record_flag(record: &NormalizedTelemetry) -> Option<bool> {
    match record.value() {
        Some(QuantityValue::Flag(b)) => Some(*b),
        _ => None,
    }
}

/// The boolean value of a stored row, when its value kind is boolean.
fn stored_flag(value: &Option<PgTelemetryValue>) -> Option<bool> {
    match value {
        Some(PgTelemetryValue::Flag(b)) => Some(*b),
        _ => None,
    }
}

/// Reads the PRE-batch `availability` records for the batch's distinct
/// assets (P2.16 step 3 input). Must run BEFORE the dedup stage inserts:
/// inside the same unit of work a post-insert read would see the incoming
/// rows and the previous availability would be unknowable. Assets without
/// an availability record map to `None` (no signal → Unknown).
fn pre_batch_availability(
    unit: &PgUnitOfWork,
    ctx: &flexcore_domain::tenant_context::TenantContext,
    records: &[NormalizedTelemetry],
) -> Result<HashMap<AssetId, Option<StoredTelemetry>>, IngestPipelineError> {
    let mut assets: Vec<AssetId> = Vec::new();
    for record in records {
        if !assets.contains(&record.asset_id()) {
            assets.push(record.asset_id());
        }
    }
    let mut map: HashMap<AssetId, Option<StoredTelemetry>> =
        assets.iter().map(|a| (*a, None)).collect();
    if assets.is_empty() {
        return Ok(map);
    }
    let rows = unit
        .latest_telemetry_per_asset(ctx, &assets, Some(&[PgQuantity::Availability]))
        .map_err(map_telemetry_error)
        .map_err(IngestPipelineError::Store)?;
    for row in rows {
        map.insert(row.asset_id, Some(row));
    }
    Ok(map)
}

/// Stage 7: publishes `TelemetryReceived` (per stored record — the
/// recorded P2.16 granularity decision), `TelemetryQualityChanged`
/// (previous+new, P2.16 step 2), and `AssetAvailabilityChanged` (P2.16
/// step 3) via the OUTBOX port — inside the same unit of work as the store
/// stage (report §6.3 step 1; never a direct broker publish). Returns the
/// number of events appended (freshly, not deduplicated).
#[allow(clippy::too_many_arguments)]
fn publish_events(
    unit: &PgUnitOfWork,
    ctx: &flexcore_domain::tenant_context::TenantContext,
    records: &[NormalizedTelemetry],
    report: &DedupReport,
    pre_batch_availability: &HashMap<AssetId, Option<StoredTelemetry>>,
    now: EventTime,
    ids: &dyn IdGenerator,
    encoder: &dyn EventPayloadEncoder,
    metrics: &IngestionMetrics,
) -> Result<usize, IngestPipelineError> {
    let mut appended = 0;
    for (index, disposition) in report.dispositions.iter().enumerate() {
        let record = &records[index];
        let (emit_received, previous_quality, updates_state) = match disposition {
            RecordDisposition::Stored {
                transition,
                quality_changed: _,
                previous_quality,
                ..
            } => (
                true,
                *previous_quality,
                *transition == CurrentStateTransition::UpdatesState,
            ),
            _ => (false, None, false),
        };
        if !emit_received {
            continue;
        }
        let site_id = match site_of(unit, ctx, record.asset_id()) {
            Some(site) => site,
            None => {
                // The composite FK makes this unreachable for a stored row;
                // log loudly rather than skipping silently.
                tracing::error!(
                    asset = %record.asset_id().uuid().to_hyphenated(),
                    "a stored telemetry record's asset vanished from the registry; \
                     no event was published for it"
                );
                continue;
            }
        };
        // TelemetryReceived (deterministic key; per-record granularity —
        // the recorded P2.16 decision, see the module header).
        let received = DomainEvent::TelemetryReceived(TelemetryReceivedPayload {
            correlation_id: ids.new_uuid(),
            tenant_id: ctx.tenant_id(),
            site_id,
            asset_id: record.asset_id(),
            event_time: record.event_time(),
        });
        appended += emit_event(
            unit,
            &received,
            &event_key("telemetry_received", record),
            now,
            encoder,
            ctx,
        )?;
        metrics.received_events.fetch_add(1, Ordering::SeqCst);

        // P2.21 per-record correlation log (the P1.20 rule): tenant_id,
        // site_id, asset_id, source_id, event_time, quality. This is the
        // per-asset breakdown the metric families deliberately do NOT carry
        // (recorded cardinality decisions — module header). No credential
        // or payload content ever enters these fields: identity ids,
        // vocabulary states, and timestamps only (the no-secrets rule).
        tracing::info!(
            tenant_id = %ctx.tenant_id().uuid().to_hyphenated(),
            site_id = %site_id.uuid().to_hyphenated(),
            asset_id = %record.asset_id().uuid().to_hyphenated(),
            source_id = %record.source_id(),
            event_time = record.event_time().unix_millis(),
            quality = ?record.quality(),
            quantity = %quantity_key(record.quantity()),
            "telemetry record stored"
        );

        // TelemetryQualityChanged on quality-state change (P2.08 rule);
        // the payload carries BOTH the previous and the new state (the
        // P2.16 step-2 payload revision).
        if let Some(previous) = previous_quality {
            let changed = flexcore_domain::quality_rules::quality_state_changed(
                previous,
                record.quality(),
            );
            if changed {
                tracing::info!(
                    asset = %record.asset_id().uuid().to_hyphenated(),
                    quantity = %quantity_key(record.quantity()),
                    previous = ?previous,
                    new = ?record.quality(),
                    "telemetry quality state changed (previous and new states both carried \
                     by the event payload — the P2.16 step-2 revision)"
                );
                let event = DomainEvent::TelemetryQualityChanged(TelemetryQualityChangedPayload {
                    correlation_id: ids.new_uuid(),
                    tenant_id: ctx.tenant_id(),
                    site_id,
                    asset_id: record.asset_id(),
                    quality: record.quality(),
                    previous_quality: Some(previous),
                    event_time: record.event_time(),
                });
                appended += emit_event(
                    unit,
                    &event,
                    &event_key("telemetry_quality_changed", record),
                    now,
                    encoder,
                    ctx,
                )?;
                metrics.quality_changed_events.fetch_add(1, Ordering::SeqCst);
            }
        }

        // AssetAvailabilityChanged on a computed availability change
        // (P2.16 step 3): the availability rule is the recorded pure rule
        // (`flexcore_application::availability`), driven by the latest
        // availability record's quality + value + the source connectivity
        // signal. Stage 1 passed for this frame, so the source IS
        // reporting at every emission point. An `Unchanged` assessment
        // resolves to the previous state — it can never produce an event
        // by itself.
        if updates_state && record.quantity() == TelemetryQuantity::Availability {
            // The pre-batch assessment: from the latest availability row
            // BEFORE this batch (the connectivity signal cannot be known
            // retroactively; it is taken as reporting for the past read —
            // recorded). `Unchanged` there resolves to Unknown: without a
            // tracked history we honestly do not know the older state.
            let previous_state = match pre_batch_availability.get(&record.asset_id()) {
                Some(Some(row)) => {
                    let assessment = availability(&AvailabilitySignalInput {
                        latest_quality: Some(row.quality),
                        latest_value: stored_flag(&row.value),
                        source_reporting: true,
                    });
                    match assessment.state {
                        AvailabilityState::Unchanged => AvailabilityState::Unknown,
                        other => other,
                    }
                }
                _ => AvailabilityState::Unknown,
            };
            let new_assessment = availability(&AvailabilitySignalInput {
                latest_quality: Some(record.quality()),
                latest_value: record_flag(record),
                source_reporting: true,
            });
            let new_state = match new_assessment.state {
                AvailabilityState::Unchanged => previous_state,
                other => other,
            };
            if availability_changed(previous_state, new_state) {
                tracing::info!(
                    asset = %record.asset_id().uuid().to_hyphenated(),
                    previous = ?previous_state,
                    new = ?new_state,
                    reason = new_assessment.reason,
                    "asset availability changed (the frozen payload carries scope + time; \
                     the concrete state is re-derivable from the availability telemetry — \
                     recorded assumption, flexcore-events README)"
                );
                let event =
                    DomainEvent::AssetAvailabilityChanged(AssetAvailabilityChangedPayload {
                        correlation_id: ids.new_uuid(),
                        tenant_id: ctx.tenant_id(),
                        site_id,
                        asset_id: record.asset_id(),
                        event_time: record.event_time(),
                    });
                appended += emit_event(
                    unit,
                    &event,
                    &event_key("asset_availability_changed", record),
                    now,
                    encoder,
                    ctx,
                )?;
                metrics
                    .availability_changed_events
                    .fetch_add(1, Ordering::SeqCst);
            }
        }
    }
    Ok(appended)
}

/// Assembles and appends ONE transactional outbox write for an event (the
/// `event_assembly` pattern, worker-side: the state change is the telemetry
/// insert that already happened in this unit of work).
fn emit_event(
    unit: &PgUnitOfWork,
    event: &DomainEvent,
    idempotency_key: &str,
    now: EventTime,
    encoder: &dyn EventPayloadEncoder,
    ctx: &flexcore_domain::tenant_context::TenantContext,
) -> Result<usize, IngestPipelineError> {
    let envelope = EventEnvelope::new(SUPPORTED_SCHEMA_VERSION, idempotency_key, event.clone())
        .map_err(|e| {
            IngestPipelineError::Encode(ApplicationError::Infrastructure(
                flexcore_application::error::InfrastructureError::new(
                    "event_envelope_failed".to_string(),
                    e.to_string(),
                ),
            ))
        })?;
    let payload = encoder.encode(&envelope)?;
    let subject = subject_for(&envelope.event).to_string();
    let receipt = StateChangeReceipt::new(
        TELEMETRY_ENTITY.to_string(),
        StateChangeOperation::Insert,
        Some(ctx.tenant_id()),
    );
    let write = OutboxWrite::new(receipt, idempotency_key.to_string(), subject, payload, now)
        .map_err(|e| {
            IngestPipelineError::Encode(ApplicationError::Infrastructure(
                flexcore_application::error::InfrastructureError::new(
                    "outbox_write_failed".to_string(),
                    e.to_string(),
                ),
            ))
        })?;
    append_idempotent(unit, write)
}

// --- The ingestion task -------------------------------------------------------------

/// Why the task ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IngestionRunOutcome {
    /// The channel closed (the source bridge dropped its queue): drained.
    ChannelClosed,
    /// Shutdown fired: new offers stopped, buffered frames were drained.
    ShutdownDrained,
    /// Stage 1 rejected the source at startup.
    SourceAuthFailed { reason: String },
    /// The source rejected the subscription query.
    SubscribeFailed { reason: String },
    /// The task was already running (the receiver was taken).
    AlreadyRunning,
}

/// The task's final report.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct IngestionRunReport {
    pub frames_processed: u64,
    pub frames_drained_at_shutdown: u64,
    pub outcome: Option<IngestionRunOutcome>,
}

/// The ingestion service task (P2.10). Construct with [`IngestionTask::new`];
/// run with [`IngestionTask::run`] (a worker background task, P1.19
/// hosting); hand [`IngestionTask::queue`] to the source bridge.
pub struct IngestionTask {
    source: Arc<dyn TelemetrySource>,
    db: Db,
    inbox: PgIngestionInbox,
    encoder: Arc<dyn EventPayloadEncoder>,
    clock: Arc<dyn Clock>,
    ids: Arc<dyn IdGenerator>,
    config: IngestionConfig,
    metrics: Arc<IngestionMetrics>,
    queue_tx: mpsc::Sender<IngestFrame>,
    queue_rx: Mutex<Option<mpsc::Receiver<IngestFrame>>>,
}

impl IngestionTask {
    /// Composes the task over its ports. `db` is the shared FlexCoreStore
    /// (the worker's pool); `inbox` the durable ingestion inbox; `encoder`
    /// the event payload renderer; `clock`/`ids` the platform clock and
    /// the ID generator (the store's uuidv7 policy).
    pub fn new(
        source: Arc<dyn TelemetrySource>,
        db: Db,
        inbox: PgIngestionInbox,
        encoder: Arc<dyn EventPayloadEncoder>,
        clock: Arc<dyn Clock>,
        ids: Arc<dyn IdGenerator>,
        config: IngestionConfig,
        metrics: Arc<IngestionMetrics>,
    ) -> Self {
        let (queue_tx, queue_rx) = mpsc::channel(config.channel_capacity.max(1));
        Self {
            source,
            db,
            inbox,
            encoder,
            clock,
            ids,
            config,
            metrics,
            queue_tx,
            queue_rx: Mutex::new(Some(queue_rx)),
        }
    }

    /// The receive-side handle for the source bridge (bounded; sheds at
    /// the limit).
    pub fn queue(&self) -> IngestionQueue {
        IngestionQueue {
            tx: self.queue_tx.clone(),
            metrics: Arc::clone(&self.metrics),
        }
    }

    /// The metrics handle.
    pub fn metrics(&self) -> Arc<IngestionMetrics> {
        Arc::clone(&self.metrics)
    }

    /// The ingestion loop until shutdown (P1.19 hosting pattern). Stage 1
    /// runs once at start; stages 2–7 run per frame. On shutdown: stop
    /// accepting (the queue sheds), drain the buffered frames, exit.
    pub async fn run(
        &self,
        query: flexcore_ports::telemetry_source::TelemetryQuery,
        mut shutdown: watch::Receiver<bool>,
    ) -> IngestionRunReport {
        let mut report = IngestionRunReport::default();

        // Stage 1 (once per task): source authentication (recorded STUB).
        if let Err(rejection) =
            authenticate_source(self.source.as_ref(), self.config.trusted_source_id.as_deref())
        {
            tracing::warn!(
                reason = %rejection.reason,
                auth_status = SOURCE_AUTH_STATUS,
                "ingestion source rejected at the receive stage"
            );
            self.metrics.frames_rejected_auth.fetch_add(1, Ordering::SeqCst);
            report.outcome = Some(IngestionRunOutcome::SourceAuthFailed {
                reason: rejection.reason,
            });
            return report;
        }
        tracing::info!(
            source = %self.source.source_id(),
            auth_status = SOURCE_AUTH_STATUS,
            "ingestion source accepted (Phase 2 stub authentication)"
        );

        let subscription = match self.source.subscribe(&query) {
            Ok(subscription) => subscription,
            Err(error) => {
                tracing::warn!(error = %error, "the telemetry source rejected the subscription");
                report.outcome = Some(IngestionRunOutcome::SubscribeFailed {
                    reason: error.to_string(),
                });
                return report;
            }
        };

        // The pipeline's tenant scope: the subscription query's tenant
        // (the query is constructed only through the domain TenantContext,
        // P2.11 — the structural tenant boundary).
        let ctx = flexcore_domain::tenant_context::TenantContext::from_authenticated_claims(
            flexcore_domain::tenant_context::AuthenticatedClaims::from_verified(
                flexcore_domain::tenant_context::VerifiedClaims {
                    tenant_id: query.tenant_id(),
                    subject: "worker:telemetry-ingestion".to_string(),
                },
            ),
        );

        let mut rx = match self.queue_rx.lock().expect("poisoned queue mutex").take() {
            Some(rx) => rx,
            None => {
                report.outcome = Some(IngestionRunOutcome::AlreadyRunning);
                return report;
            }
        };

        loop {
            if *shutdown.borrow() {
                let drained = self.drain(&ctx, &subscription, &mut rx).await;
                report.frames_drained_at_shutdown = drained;
                report.outcome = Some(IngestionRunOutcome::ShutdownDrained);
                return report;
            }
            tokio::select! {
                biased;
                frame = rx.recv() => {
                    match frame {
                        Some(frame) => {
                            // P2.21: the queue-depth gauge reads the bounded
                            // channel's current length (backlog after this
                            // frame's dequeue).
                            self.metrics.set_queue_depth(rx.len());
                            report.frames_processed += 1;
                            self.process_frame(&ctx, &subscription, frame).await;
                        }
                        None => {
                            // The bridge dropped its queue handle: drain is
                            // complete by construction, exit cleanly.
                            report.outcome = Some(IngestionRunOutcome::ChannelClosed);
                            return report;
                        }
                    }
                }
                changed = shutdown.changed() => {
                    let _ = changed;
                    let drained = self.drain(&ctx, &subscription, &mut rx).await;
                    report.frames_drained_at_shutdown = drained;
                    report.outcome = Some(IngestionRunOutcome::ShutdownDrained);
                    return report;
                }
            }
        }
    }

    /// Drains the buffered frames after shutdown (stop accepting → drain →
    /// exit; the graceful-shutdown contract).
    async fn drain(
        &self,
        ctx: &flexcore_domain::tenant_context::TenantContext,
        subscription: &TelemetrySubscription,
        rx: &mut mpsc::Receiver<IngestFrame>,
    ) -> u64 {
        let mut drained = 0u64;
        loop {
            self.metrics.set_queue_depth(rx.len());
            match rx.try_recv() {
                Ok(frame) => {
                    drained += 1;
                    self.process_frame(ctx, subscription, frame).await;
                }
                Err(_) => return drained,
            }
        }
    }

    /// Runs stages 1–7 for one received frame. A frame that exhausts its
    /// bounded store-retry budget is counted (`frames_failed`) and logged
    /// at error level — never silently dropped; the source's next emission
    /// redelivers it.
    ///
    /// P2.21 tracing: the frame's span carries the P1.20 correlation fields
    /// (tenant_id, source_id); the per-record log lines inside stage 7 add
    /// site_id, asset_id, event_time, and quality.
    async fn process_frame(
        &self,
        ctx: &flexcore_domain::tenant_context::TenantContext,
        subscription: &TelemetrySubscription,
        frame: IngestFrame,
    ) {
        let _frame = tracing::info_span!(
            "telemetry_ingest_frame",
            tenant_id = %ctx.tenant_id().uuid().to_hyphenated(),
            source_id = %self.source.source_id(),
        )
        .entered();
        self.metrics.frames_received.fetch_add(1, Ordering::SeqCst);

        // Stage 1 (per frame): the source's health can change mid-stream.
        if let Err(rejection) =
            authenticate_source(self.source.as_ref(), self.config.trusted_source_id.as_deref())
        {
            self.metrics.frames_rejected_auth.fetch_add(1, Ordering::SeqCst);
            self.quarantine_raw(
                ctx,
                &frame.payload,
                "1-receive",
                &rejection.reason,
            );
            return;
        }

        // Stages 2+3: decode = schema validation + normalization to
        // canonical records (the port's contract; failure is Err, never a
        // partial record).
        let records = match self.source.decode(subscription, &frame.payload) {
            Ok(records) => records,
            Err(error) => {
                let reason = match &error {
                    TelemetrySourceError::DecodeFailed { reason } => reason.clone(),
                    other => other.to_string(),
                };
                self.quarantine_raw(ctx, &frame.payload, "2-validate", &reason);
                return;
            }
        };

        // Stage 4: quality assignment (the P2.08 rules; hold-or-lower).
        let records: Vec<NormalizedTelemetry> =
            records.into_iter().map(|r| refine_quality(&r)).collect();

        // Stages 5–7 with the bounded retry budget (retryable transient
        // store failures; exponential backoff, capped).
        let mut attempt: u32 = 0;
        loop {
            attempt += 1;
            match self.process_batch(ctx, &records) {
                Ok(batch) => {
                    if batch.stored > 0 {
                        tracing::info!(
                            tenant_id = %ctx.tenant_id().uuid().to_hyphenated(),
                            source_id = %self.source.source_id(),
                            stored = batch.stored,
                            duplicates = batch.exact_duplicates_acknowledged,
                            quarantined = batch.conflicting_quarantined + batch.out_of_range_quarantined,
                            "ingestion batch committed"
                        );
                    }
                    return;
                }
                Err(error) => {
                    self.metrics.store_retries.fetch_add(1, Ordering::SeqCst);
                    if attempt >= self.config.store_max_attempts {
                        self.metrics.frames_failed.fetch_add(1, Ordering::SeqCst);
                        tracing::error!(
                            attempts = attempt,
                            error = %error,
                            "the ingestion frame exhausted its store-retry budget; it was NOT \
                             stored and NOT quarantined (the store is failing) — counted and \
                             logged, the source's next emission redelivers"
                        );
                        return;
                    }
                    let delay = backoff_delay(
                        self.config.store_retry_base,
                        self.config.store_retry_cap,
                        attempt,
                    );
                    tracing::warn!(
                        attempt,
                        error = %error,
                        backoff_ms = delay.as_millis() as u64,
                        "transient ingestion failure; retrying with backoff"
                    );
                    tokio::time::sleep(delay).await;
                }
            }
        }
    }

    /// Quarantines a RAW frame (stages 1/2 reject before any canonical
    /// record exists): the evidence trail of the no-silent-skip rule.
    /// Quarantine-write failures are logged, never swallowed silently.
    fn quarantine_raw(
        &self,
        ctx: &flexcore_domain::tenant_context::TenantContext,
        payload: &[u8],
        stage: &str,
        reason: &str,
    ) {
        self.metrics.records_quarantined.fetch_add(1, Ordering::SeqCst);
        // P2.21: the quarantine metric carries the closed reason class only
        // (the free-text reason is never a label).
        self.metrics
            .record_quarantine_reason(quarantine_reason_class(stage, reason));
        let raw = String::from_utf8_lossy(payload).to_string();
        let entry = TelemetryQuarantineWrite {
            tenant_id: ctx.tenant_id(),
            asset_id: None,
            event_time: None,
            source_id: Some(self.source.source_id().to_string()),
            source_sequence: None,
            stage: stage.to_string(),
            reason: reason.to_string(),
            raw_payload: raw,
            attempt_count: 0,
        };
        let result = tokio::task::block_in_place(|| {
            let unit = match self.db.begin(Some(ctx)) {
                Ok(unit) => unit,
                Err(error) => return Err(error.to_string()),
            };
            let store = UowTelemetryIngestStore::new(&unit);
            store.quarantine(ctx, &entry).map_err(|e| e.to_string())?;
            unit.commit().map_err(|e| e.to_string())
        });
        if let Err(error) = result {
            tracing::error!(
                stage,
                error,
                "the quarantine write for a rejected frame failed; the rejection is still \
                 counted and logged (the evidence write is retried by the source's redelivery)"
            );
        } else {
            tracing::warn!(stage, reason, "an ingestion frame was quarantined");
        }
    }

    /// Stages 5–7 for one batch of canonical records, all-or-nothing: ONE
    /// unit of work covers the dedup stage's inserts + quarantine writes
    /// and the outbox appends; a failure rolls everything back.
    ///
    /// Public for the integration tests and future embeddings (the
    /// production caller is the run loop's stage sequence — the tests
    /// drive the SAME code path the loop uses).
    pub fn process_batch(
        &self,
        ctx: &flexcore_domain::tenant_context::TenantContext,
        records: &[NormalizedTelemetry],
    ) -> Result<DedupReport, IngestPipelineError> {
        // P2.21 staleness signal (P2.08 staleness rule reused — never
        // recomputed here): a record whose age exceeds the DEPLOYMENT
        // freshness limit increments the staleness metric and emits the
        // staleness log line. The stored quality state is NOT changed
        // (staleness is a view-time signal, P2.08); the threshold is
        // deployment configuration, never hard-coded (P2.21 step 3: a
        // metric + log, not an alerting rule). Evaluated here — the one
        // entry point shared by the run loop and direct batch callers.
        if let Some(limit_millis) = self.config.stale_freshness_limit_millis {
            if let Ok(limit) = FreshnessLimit::try_new(limit_millis) {
                let now = self.clock.now_event_time();
                for record in records {
                    if staleness(record.event_time(), now, limit).is_stale() {
                        self.metrics.stale_observations.fetch_add(1, Ordering::SeqCst);
                        tracing::warn!(
                            tenant_id = %ctx.tenant_id().uuid().to_hyphenated(),
                            asset_id = %record.asset_id().uuid().to_hyphenated(),
                            source_id = %record.source_id(),
                            event_time = record.event_time().unix_millis(),
                            quality = ?record.quality(),
                            freshness_limit_ms = limit.millis(),
                            "telemetry staleness signal: the record's age exceeds the \
                             deployment freshness limit (the threshold is deployment \
                             configuration; this is a metric + log signal, not an alerting \
                             rule — P2.21 step 3)"
                        );
                    }
                }
            }
        }
        tokio::task::block_in_place(|| {
            let unit = self
                .db
                .begin(Some(ctx))
                .map_err(|e| IngestPipelineError::Commit(e.to_string()))?;
            // P2.16 step 3 input: the PRE-batch availability records, read
            // in the SAME unit of work BEFORE the dedup stage inserts (a
            // post-insert read would see the incoming rows). One extra
            // latest-row read per frame; only taken when the batch could
            // carry an availability signal at all.
            let pre_batch_availability = pre_batch_availability(&unit, ctx, records)?;
            let outcome = (|| {
                let store = UowTelemetryIngestStore::new(&unit);
                let dedup = TelemetryDeduplicator::new(INGESTION_CONSUMER.to_string());
                // Stage 5 (+ stage 6 inserts and the rejected records'
                // quarantine writes inside the same transaction).
                let report = dedup
                    .process_batch(ctx, records, &store, &self.inbox)
                    .map_err(|error| match error {
                        DedupError::Store(store_error) => IngestPipelineError::Store(store_error),
                        DedupError::Inbox(inbox_error) => {
                            IngestPipelineError::Commit(inbox_error.to_string())
                        }
                    })?;
                // Stage 7: publish via the OUTBOX (same transaction).
                let now = self.clock.now_event_time();
                publish_events(
                    &unit,
                    ctx,
                    records,
                    &report,
                    &pre_batch_availability,
                    now,
                    self.ids.as_ref(),
                    self.encoder.as_ref(),
                    &self.metrics,
                )?;
                Ok(report)
            })();
            match outcome {
                Ok(report) => {
                    unit.commit()
                        .map_err(|e| IngestPipelineError::Commit(e.to_string()))?;
                    // Post-commit: record the deliveries in the inbox
                    // (P2.09 step 4 — AFTER the commit, so a rolled-back
                    // batch is re-processed and made harmless by the
                    // store-level dedup; see the port's contract note).
                    for key in &report.delivered_keys {
                        if let Err(error) = self.inbox.record(INGESTION_CONSUMER, key) {
                            tracing::warn!(
                                error = %error,
                                "the post-commit inbox record failed; a redelivery of this \
                                 batch will be deduplicated by the store instead"
                            );
                        }
                    }
                    // P2.21: outcome metrics over the COMMITTED batch —
                    // recorded on this one shared path (the run loop and
                    // direct batch callers behave identically).
                    self.metrics
                        .records_stored
                        .fetch_add(report.stored as u64, Ordering::SeqCst);
                    self.metrics
                        .duplicates_acknowledged
                        .fetch_add(report.exact_duplicates_acknowledged as u64, Ordering::SeqCst);
                    self.metrics
                        .redeliveries_skipped
                        .fetch_add(report.redeliveries_skipped as u64, Ordering::SeqCst);
                    self.metrics.records_quarantined.fetch_add(
                        (report.conflicting_quarantined + report.out_of_range_quarantined) as u64,
                        Ordering::SeqCst,
                    );
                    // The quarantine metric carries closed reason classes,
                    // never the free-text rejection reason.
                    for _ in 0..report.conflicting_quarantined {
                        self.metrics.record_quarantine_reason("conflicting_duplicate");
                    }
                    for _ in 0..report.out_of_range_quarantined {
                        self.metrics.record_quarantine_reason("out_of_range_event_time");
                    }
                    // The per-batch ingestion-delay summary and the
                    // per-state quality counts cover the STORED records
                    // (duplicates are not new values). The per-asset delay
                    // and quality breakdown travels in the per-record log
                    // lines emitted by stage 7 — never as metric labels.
                    for (record, disposition) in records.iter().zip(&report.dispositions) {
                        if matches!(disposition, RecordDisposition::Stored { .. }) {
                            let delay = record.ingest_time().unix_millis()
                                - record.event_time().unix_millis();
                            self.metrics.record_ingestion_delay_millis(delay);
                            self.metrics.record_quality_state(record.quality());
                        }
                    }
                    Ok(report)
                }
                Err(error) => {
                    // Drop rolls the native transaction back (P1.10 rule 4).
                    drop(unit);
                    Err(error)
                }
            }
        })
    }
}

/// Exponential backoff between store retries, clamped to `[base, cap]`
/// (the runner's backoff pattern).
fn backoff_delay(base: Duration, cap: Duration, failures: u32) -> Duration {
    let exponent = failures.saturating_sub(1).min(16);
    let factor = 1u64.checked_shl(exponent).unwrap_or(u64::MAX);
    base.checked_mul(factor)
        .unwrap_or(cap)
        .min(cap)
        .max(base.min(cap))
}

/// The worker-side event payload encoder (the composition root owns the
/// serialization library; the same rendering as the control plane's).
#[derive(Debug, Default, Clone, Copy)]
pub struct JsonEventPayloadEncoder;

impl EventPayloadEncoder for JsonEventPayloadEncoder {
    fn encode(&self, envelope: &EventEnvelope) -> Result<String, ApplicationError> {
        serde_json::to_string(envelope).map_err(|_| {
            ApplicationError::Infrastructure(
                flexcore_application::error::InfrastructureError::new(
                    "event_payload_encode_failed".to_string(),
                    "the event payload could not be rendered".to_string(),
                ),
            )
        })
    }
}

/// The worker's `IdGenerator` over the same database's `uuidv7()` (the
/// D-S1 version policy; the adapter's own generator is `pub(crate)`, so
/// the composition root implements the port here over the shared pool —
/// one private runtime, like the other worker-owned stores). Used for the
/// event correlation identifiers.
#[derive(Clone)]
pub struct UuidV7Generator {
    pool: sqlx::PgPool,
    runtime: Arc<tokio::runtime::Runtime>,
}

impl UuidV7Generator {
    /// Builds the generator over the shared pool.
    pub fn new(pool: sqlx::PgPool) -> Self {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("the uuidv7 generator's runtime could not start");
        Self {
            pool,
            runtime: Arc::new(runtime),
        }
    }
}

impl IdGenerator for UuidV7Generator {
    fn new_uuid(&self) -> FlexUuid {
        let pool = self.pool.clone();
        let text: String = self
            .runtime
            .block_on(async {
                sqlx::query_scalar::<_, String>("SELECT uuidv7()::text")
                    .fetch_one(&pool)
                    .await
            })
            .expect("uuidv7() generation requires a reachable database; FlexCore ID \
                     generation is fail-fast (D-S1)");
        FlexUuid::parse(&text).expect("uuidv7() always returns a canonical UUID text value")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flexcore_contracts::ids::FlexUuid;
    use flexcore_contracts::time_model::{ClockQuality, IngestTime};
    use flexcore_domain::ids::TenantId;
    use flexcore_ports::fakes::FakeTelemetrySource;
    use flexcore_ports::telemetry_source::{
        NormalizedTelemetryParts, ProviderDeclaration, QualityMappingDeclaration,
        SourceStateMapping,
    };

    const TENANT_UUID: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a9b";
    const ASSET_UUID: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a8c";

    fn tenant_id() -> TenantId {
        TenantId::new(FlexUuid::parse(TENANT_UUID).unwrap())
    }

    fn asset_id() -> AssetId {
        AssetId::new(FlexUuid::parse(ASSET_UUID).unwrap())
    }

    fn ctx() -> TenantContext {
        TenantContext::from_authenticated_claims(AuthenticatedClaims::from_verified(
            VerifiedClaims {
                tenant_id: tenant_id(),
                subject: "test:ingestion".to_string(),
            },
        ))
    }

    fn good_record() -> NormalizedTelemetry {
        NormalizedTelemetry::new(NormalizedTelemetryParts {
            tenant_id: tenant_id(),
            asset_id: asset_id(),
            quantity: TelemetryQuantity::ActivePower,
            value: Some(QuantityValue::Power(ActivePowerMW::new(1.5).unwrap())),
            event_time: EventTime::from_unix_millis(1_893_456_000_000).unwrap(),
            ingest_time: IngestTime::from_unix_millis(1_893_456_000_001).unwrap(),
            source_sequence: 1,
            source_id: "sim://exit-gate".to_string(),
            quality: TelemetryQuality::Good,
            source_quality_code: None,
            clock_quality: ClockQuality::Synchronized,
        })
        .unwrap()
    }

    fn fake_source(records: Vec<NormalizedTelemetry>) -> FakeTelemetrySource {
        let declaration = ProviderDeclaration::new(
            "sim://exit-gate".to_string(),
            Duration::from_secs(60),
            QualityMappingDeclaration::new(vec![SourceStateMapping {
                source_code: "GOOD".to_string(),
                canonical: TelemetryQuality::Good,
            }])
            .unwrap(),
            ClockQuality::Synchronized,
        )
        .unwrap();
        FakeTelemetrySource::new(declaration, records)
    }

    #[test]
    fn source_auth_stub_accepts_a_healthy_declared_source() {
        let source = fake_source(vec![good_record()]);
        assert!(authenticate_source(&source, None).is_ok());
        assert!(authenticate_source(&source, Some("sim://exit-gate")).is_ok());
        // The stub status is recorded honestly and states the limitation.
        assert!(SOURCE_AUTH_STATUS.contains("STUB"));
        assert!(SOURCE_AUTH_STATUS.contains("no cryptographic source authentication"));
    }

    #[test]
    fn source_auth_rejects_an_untrusted_identity_and_an_unavailable_source() {
        let source = fake_source(vec![good_record()]);
        let rejection = authenticate_source(&source, Some("sim://other")).unwrap_err();
        assert!(rejection.reason.contains("not the configured trusted source"));

        // An unavailable provider is rejected at the receive stage, before
        // any decode runs.
        let rejection = authenticate_source(&ProbeSource, None).unwrap_err();
        assert!(rejection.reason.contains("unavailable"));
    }

    /// A provider reporting itself unavailable (the health check must
    /// reject it without needing any decode).
    struct ProbeSource;
    impl TelemetrySource for ProbeSource {
        fn provider_declaration(&self) -> &ProviderDeclaration {
            unreachable!("the health check never reads the declaration")
        }
        fn subscribe(
            &self,
            _query: &flexcore_ports::telemetry_source::TelemetryQuery,
        ) -> Result<TelemetrySubscription, TelemetrySourceError> {
            unreachable!()
        }
        fn decode(
            &self,
            _subscription: &TelemetrySubscription,
            _payload: &[u8],
        ) -> Result<Vec<NormalizedTelemetry>, TelemetrySourceError> {
            unreachable!()
        }
        fn health(&self) -> TelemetrySourceHealth {
            TelemetrySourceHealth::Unavailable {
                reason: "starting".to_string(),
            }
        }
    }

    #[test]
    fn the_quality_stage_holds_or_lowers_and_never_upgrades() {
        // A provider GOOD stays GOOD (the rule passes).
        assert_eq!(refine_quality(&good_record()).quality(), TelemetryQuality::Good);

        // A provider SUSPECT is NOT upgraded to GOOD by the passing rule
        // (the non-silencing rule; hold-or-lower).
        let mut suspect = NormalizedTelemetry::new(NormalizedTelemetryParts {
            quality: TelemetryQuality::Suspect,
            source_quality_code: Some("OUT_OF_RANGE".to_string()),
            ..good_parts()
        })
        .unwrap();
        let refined = refine_quality(&suspect);
        assert_eq!(refined.quality(), TelemetryQuality::Suspect);
        assert_eq!(refined.source_quality_code(), Some("OUT_OF_RANGE"));

        // A provider BAD (source-reported invalid) stays BAD (P2.08 rule 3).
        suspect = NormalizedTelemetry::new(NormalizedTelemetryParts {
            quality: TelemetryQuality::Bad,
            source_quality_code: Some("SOURCE_INVALID".to_string()),
            ..good_parts()
        })
        .unwrap();
        assert_eq!(refine_quality(&suspect).quality(), TelemetryQuality::Bad);
    }

    fn good_parts() -> NormalizedTelemetryParts {
        NormalizedTelemetryParts {
            tenant_id: tenant_id(),
            asset_id: asset_id(),
            quantity: TelemetryQuantity::ActivePower,
            value: Some(QuantityValue::Power(ActivePowerMW::new(1.5).unwrap())),
            event_time: EventTime::from_unix_millis(1_893_456_000_000).unwrap(),
            ingest_time: IngestTime::from_unix_millis(1_893_456_000_001).unwrap(),
            source_sequence: 1,
            source_id: "sim://exit-gate".to_string(),
            quality: TelemetryQuality::Good,
            source_quality_code: None,
            clock_quality: ClockQuality::Synchronized,
        }
    }

    #[test]
    fn backpressure_sheds_at_the_bounded_limit_and_counts() {
        // A capacity-1 channel with no consumer: the first offer is
        // buffered, the second SHEDS (retryable) and the shed metric
        // increments. This is the P2.07 §5 at-limit behaviour.
        let metrics = IngestionMetrics::default();
        let (tx, mut rx) = mpsc::channel(1);
        assert!(offer_bounded(&tx, &metrics, b"frame-1".to_vec()).is_ok());
        assert_eq!(offer_bounded(&tx, &metrics, b"frame-2".to_vec()), Err(FrameShed));
        assert_eq!(metrics.snapshot().frames_shed, 1);
        // The first frame is still buffered (bounded, not lost).
        assert!(rx.try_recv().is_ok());
        // After draining, the channel accepts again.
        assert!(offer_bounded(&tx, &metrics, b"frame-3".to_vec()).is_ok());
        assert_eq!(metrics.snapshot().frames_shed, 1);
    }

    #[test]
    fn backoff_grows_exponentially_and_caps() {
        let base = Duration::from_millis(200);
        let cap = Duration::from_secs(5);
        assert_eq!(backoff_delay(base, cap, 1), base);
        assert_eq!(backoff_delay(base, cap, 2), Duration::from_millis(400));
        assert_eq!(backoff_delay(base, cap, 100), cap);
    }

    #[test]
    fn the_storage_record_mapping_carries_the_canonical_fields() {
        let record = good_record();
        let mapped = pg_record(&record).unwrap();
        assert_eq!(mapped.tenant_id, tenant_id());
        assert_eq!(mapped.asset_id, asset_id());
        assert_eq!(mapped.quantity, PgQuantity::ActivePower);
        assert_eq!(mapped.value, Some(PgTelemetryValue::Numeric(1.5)));
        assert_eq!(mapped.source_sequence, 1);
        assert_eq!(mapped.source_id, "sim://exit-gate");
        assert_eq!(mapped.quality, TelemetryQuality::Good);
        assert!(mapped.q_sign_convention.is_none());
        assert_eq!(mapped.clock_quality, ClockQuality::Synchronized);

        // A reactive record carries its declared convention (D-T4).
        let reactive = NormalizedTelemetry::new(NormalizedTelemetryParts {
            quantity: TelemetryQuantity::ReactivePower,
            value: Some(QuantityValue::Reactive {
                power: flexcore_contracts::units::ReactivePowerMVAr::new(0.5).unwrap(),
                convention: flexcore_contracts::units::ReactiveSignConvention::ConsumptionPositive,
            }),
            ..good_parts()
        })
        .unwrap();
        let mapped = pg_record(&reactive).unwrap();
        assert_eq!(mapped.quantity, PgQuantity::ReactivePower);
        assert_eq!(
            mapped.q_sign_convention,
            Some(flexcore_contracts::units::ReactiveSignConvention::ConsumptionPositive)
        );
    }

    /// The tenant context is derived from the subscription query's tenant
    /// (the structural boundary — a query cannot exist without a context).
    #[test]
    fn the_pipeline_tenant_comes_from_the_query_scope() {
        assert_eq!(ctx().tenant_id(), tenant_id());
    }
}
