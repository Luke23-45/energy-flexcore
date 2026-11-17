// P1.17 — Minimal Prometheus-compatible metrics registry.
//
// P1.20 adds the per-request recorder middleware (`track_metrics`) and the
// request correlation log: every served request emits ONE JSON log line
// carrying a locally generated trace id, the method, the route pattern, the
// status, the tenant id, and the actor subject. Rules:
// - The trace id is NOT a UUID: it is 32 hex characters derived from a
//   std-library hash of (wall-clock nanoseconds, per-process counter) —
//   good enough as a per-process correlation handle with no external
//   dependency; collisions across processes are not the goal.
// - The `Authorization` header is NEVER logged; identity is resolved for
//   correlation only, and a failure simply leaves tenant/actor absent.
// - No other header or body content is ever logged.
//
// P2.21 — telemetry metric families. Added to the SAME registry (no second
// metrics surface): the six families from ledger P2.21 step 1 render below
// the Phase 1 request/error families, always (zero values included) so a
// scrape exposes the full inventory even on an idle system. Cardinality is
// decided ONCE (recorded in
// `ledger/phase-02-asset-telemetry/21_22_observability_security.md`):
// - ingestion delay is a per-BATCH summary (`_sum`/`_count`); the per-asset
//   breakdown goes to the structured logs, never to labels;
// - staleness carries per-asset series ONLY while the asset is currently
//   stale (the series disappears when the asset reports fresh again);
// - quality counts are per-STATE totals (all six canonical states render,
//   zero included); per-asset quality counts go to logs;
// - quarantine counts carry a closed reason-class label only — the free-text
//   quarantine reason never becomes a label.
//
// P11.01 — golden-signal SLI families. Added to the SAME registry (no
// second metrics surface), exactly as named in the Phase 11 record
// `docs/phases-11-20/phase-11-operational-robustness/records/
// 01_p11_01_metric_taxonomy.md` (the taxonomy is named FIRST; this code
// implements only what that record names):
// - `flexcore_api_request_duration_milliseconds` — control-plane-api
//   latency SLI; histogram over the FIXED registered bucket set, NO
//   labels (per-route breakdown travels in the correlation log line);
// - `flexcore_api_in_flight_requests` — control-plane-api saturation;
//   no labels;
// - `flexcore_command_accept_duration_milliseconds` — command-lifecycle
//   accept-latency SLI; same fixed bucket set, no labels;
// - `flexcore_command_state_transitions_total{from,to}` — bounded by the
//   frozen 14-state / 27-edge lifecycle; only legal edges are counted,
//   and label values come from `CommandState::name()` only (no span,
//   event, or metric invents a state);
// - `flexcore_commands_stuck` was named in the record but NOT added in
//   wave 1 — wave 2 (P11.20) implements it (see the P11.20 section below).
//
// P11.07 — the correlation log line carries the structured-log schema v1
// attributes (`schema_version`, `tenant_id`, `command_id`, `trace_id`,
// `severity_number`); see the log-schema record for the field contract.
// The severity mapping helper lives at the bottom of this file.
//
// P13.09 — webhook dead-letter depth gauge (Phase 13 wave 1, Agent B).
// `flexcore_webhook_dead_letters` is added to the SAME registry (no second
// metrics surface), as the phase-13 plan §2.3 delivery contract requires
// ("its depth is a metric with a registered alert threshold; nothing is
// silently dropped"). A gauge, NO labels (the recorded cardinality
// discipline): per-delivery reasons stay in the `webhook_dead_letters`
// bookkeeping rows (migration 0020). The delivery service and its scanner
// live in `webhook_service.rs`. WRITTEN-UNRUN; see the phase-13 record
// docs/phases-11-20/phase-13-enterprise-identity-integration/records/
// 21_p13_09_webhook_service.md.
//
// P11.20 — stuck-state detection (Phase 11 wave 2, Agent B). The
// `flexcore_commands_stuck` gauge family is added to the SAME registry
// (no second metrics surface), EXACTLY as named in the P11.01 taxonomy
// record §2 family 5: a gauge, NO labels. It counts the commands parked
// in a NON-terminal frozen lifecycle state beyond the registered stuck
// threshold (FAILED and EXPIRED included — they exit only via FALLBACK;
// the terminal/non-terminal vocabulary is the domain's
// `flexcore_domain::command_state`, never redefined here). The scanner
// that drives the gauge also lives in this file (the reader port + the
// Postgres implementation + the pure counting function + the hosting
// loop); per-state stuck breakdowns go to LOGS, never to labels (the
// recorded cardinality decision). The P11.03 alert rule and the P11.09
// dashboard panel bind the family name verbatim and required ZERO edits
// here (wave-1 handoff honored). WRITTEN-UNRUN;
// records/51_p11_20_stuck_state.md.

use std::collections::BTreeMap;
use std::hash::Hasher as _;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::auth::IdentityResolver as _;

// P11.01: the transition-family label values and the legality oracle come
// from the frozen lifecycle vocabulary — no state name is duplicated here.
// P11.20: the terminal/non-terminal classification (`is_terminal`) comes
// from the SAME frozen vocabulary — the stuck-state domain is the domain's,
// never a second definition.
use flexcore_domain::command_state::{is_terminal, CommandState};
// P11.20 — the stuck-state scanner reads through the storage/repository
// ports (app-layer use; the dependency rule is unaffected — this module
// lives in the app crate, and the domain imports above stay pure).
use flexcore_domain::ids::TenantId;
use flexcore_domain::tenant_context::{AuthenticatedClaims, TenantContext, VerifiedClaims};
use flexcore_ports::repository::TenantRepository;
use flexcore_storage::command_store::{CommandListFilter, CommandRecordStore};

/// The frozen telemetry event types (the §15.3 vocabulary) counted by the
/// [`MetricsRegistry`] telemetry-events family.
pub const TELEMETRY_EVENT_TYPES: [&str; 3] = [
    "TelemetryReceived",
    "TelemetryQualityChanged",
    "AssetAvailabilityChanged",
];

/// The six canonical telemetry quality states (P0.08, frozen vocabulary,
/// exactly one per value) — the quality metric's closed label set.
pub const QUALITY_STATES: [&str; 6] = ["GOOD", "SUSPECT", "BAD", "MISSING", "ESTIMATED", "STALE"];

/// The FIXED registered latency bucket set (milliseconds) for the P11.01
/// SLI histograms (record 01, family 1/3): registered before any run that
/// could fill them (X-8 pre-registration). Changing this set is a recorded
/// amendment to the taxonomy record.
pub const SLI_LATENCY_BUCKETS_MILLIS: [u64; 12] = [
    1, 5, 10, 25, 50, 100, 250, 500, 1_000, 2_500, 5_000, 10_000,
];

/// P11.20 — the REGISTERED stuck-state threshold (X-8 pre-registration:
/// written down before any run that could measure against it), in
/// milliseconds. A command parked in a non-terminal frozen lifecycle state
/// for strictly longer than this duration counts as stuck.
///
/// **RECOMMENDATION (24 h) pending operator tuning** — the value is a
/// registered default, not an approved operating number: the three
/// authorization/validation/feasibility gates are human/policy steps whose
/// legitimate dwell time is an operator answer, and no observed command
/// duration distribution exists (the 0–10 base is WRITTEN-UNRUN). Changing
/// this value is a recorded amendment to the P11.20 decision record.
pub const COMMANDS_STUCK_THRESHOLD_MILLIS: i64 = 86_400_000;

/// P11.20 — the registered scan interval for the stuck-state scanner.
/// **RECOMMENDATION** pending operator tuning: the gauge is scraped by
/// Prometheus, so the scan cadence only bounds detection latency; one
/// minute is far below the 24 h threshold and cheap at the current
/// deployment scale. Registered before any run (X-8).
pub const STUCK_STATE_SCAN_INTERVAL: Duration = Duration::from_secs(60);

/// P11.20 — the keyset page size for the scanner's per-state command
/// listing (the port applies a limit to EVERY query — no unbounded query;
/// the routes use the same keyset discipline, routes_phase4).
const STUCK_SCAN_PAGE_SIZE: u32 = 200;

/// The P11.07 structured-log schema version string carried by every
/// correlation line (see record 07 for the field contract).
pub const LOG_SCHEMA_VERSION: &str = "flexcore.log.v1";

/// A fixed-bucket latency histogram over milliseconds (P11.01). Counts are
/// CUMULATIVE per bucket (Prometheus histogram exposition); a value is
/// observed into every bucket whose upper bound it fits. No labels exist
/// at this type level — the recorded cardinality decision (no route /
/// tenant / method labels) is enforced by construction.
#[derive(Debug)]
struct DurationHistogram {
    cumulative: Vec<u64>,
    sum_millis: u64,
    count: u64,
}

impl DurationHistogram {
    fn new() -> Self {
        Self {
            cumulative: vec![0; SLI_LATENCY_BUCKETS_MILLIS.len()],
            sum_millis: 0,
            count: 0,
        }
    }

    /// Observes one latency sample (milliseconds). Sub-millisecond
    /// truncation is not applied: callers pass integer milliseconds.
    fn observe(&mut self, millis: u64) {
        for (index, bound) in SLI_LATENCY_BUCKETS_MILLIS.iter().enumerate() {
            if millis <= *bound {
                self.cumulative[index] = self.cumulative[index].saturating_add(1);
            }
        }
        self.sum_millis = self.sum_millis.saturating_add(millis);
        self.count = self.count.saturating_add(1);
    }
}

/// Global, label-keyed counters rendered in Prometheus text format.
#[derive(Debug)]
pub struct MetricsRegistry {
    requests_total: Mutex<BTreeMap<String, AtomicCount>>,
    errors_total: AtomicU64,
    // --- P2.21 telemetry families -----------------------------------------
    /// Outbox telemetry events by frozen event type.
    telemetry_events_total: Mutex<BTreeMap<String, AtomicCount>>,
    /// Total quarantined records.
    quarantine_total: AtomicU64,
    /// Quarantined records by closed reason class.
    quarantine_by_reason_total: Mutex<BTreeMap<String, AtomicCount>>,
    /// Ingestion bounded-channel length (last observation).
    queue_depth: AtomicU64,
    /// Stored records per canonical quality state (fixed six-slot array;
    /// a state that never occurred still renders with 0).
    quality_states: [AtomicU64; 6],
    /// Staleness observations per asset. A series exists ONLY while the
    /// asset's latest evaluation is stale (the entry is removed when the
    /// asset reports fresh again) — the bounded per-asset decision.
    asset_stale_total: Mutex<BTreeMap<String, u64>>,
    /// Per-batch ingestion-delay summary (`ingest_time - event_time`, ms).
    ingestion_delay: Mutex<IngestionDelaySummary>,
    // --- P11.01 golden-signal SLI families (record 01) --------------------
    /// control-plane-api latency SLI: fixed-bucket request-duration
    /// histogram, no labels (recorded cardinality decision).
    api_request_duration: Mutex<DurationHistogram>,
    /// control-plane-api saturation: current in-flight request count
    /// inside the `track_metrics` middleware, no labels.
    api_in_flight_requests: AtomicU64,
    /// command-lifecycle accept-latency SLI: fixed-bucket histogram, no
    /// labels (recorded on the command-submission path; the API exists
    /// here, route-side wiring is Agent B's handoff note — record 01 §5).
    command_accept_duration: Mutex<DurationHistogram>,
    /// command-lifecycle gate transitions over the frozen 27-edge set;
    /// only legal `(from, to)` pairs are counted. Label values come from
    /// `CommandState::name()` only.
    command_state_transitions: Mutex<BTreeMap<(String, String), u64>>,
    /// P11.20 stuck-state gauge (NO labels — the recorded cardinality
    /// decision): the number of commands parked in a non-terminal frozen
    /// lifecycle state beyond the registered threshold
    /// ([`COMMANDS_STUCK_THRESHOLD_MILLIS`]). Driven only by the
    /// stuck-state scanner ([`MetricsRegistry::set_commands_stuck`]); on a
    /// failed scan the last value is KEPT (a scanner outage never reads as
    /// "no stuck commands").
    commands_stuck: AtomicU64,
    /// P13.09 webhook dead-letter depth gauge (NO labels): the number of
    /// UNRESOLVED webhook deliveries that exhausted their bounded retry
    /// schedule (plan §2.3: "its depth is a metric with a registered alert
    /// threshold; nothing is silently dropped"). Driven only by the
    /// webhook delivery scanner ([`MetricsRegistry::set_webhook_dead_letters`],
    /// reading the delivery bookkeeping store — migration 0020); on a
    /// failed read the last value is KEPT (a scanner outage never reads as
    /// "no dead letters"). The composition wiring and the recommended
    /// alert threshold are recorded in the phase-13 record
    /// `records/21_p13_09_webhook_service.md`. WRITTEN-UNRUN (Phase 13
    /// wave 1, Agent B).
    webhook_dead_letters: AtomicU64,
}

/// The running per-batch ingestion-delay summary.
#[derive(Debug, Default)]
struct IngestionDelaySummary {
    sum_millis: u64,
    count: u64,
}

#[derive(Debug)]
struct AtomicCount(AtomicU64);

impl Default for MetricsRegistry {
    fn default() -> Self {
        Self {
            requests_total: Mutex::new(BTreeMap::new()),
            errors_total: AtomicU64::new(0),
            telemetry_events_total: Mutex::new(BTreeMap::new()),
            quarantine_total: AtomicU64::new(0),
            quarantine_by_reason_total: Mutex::new(BTreeMap::new()),
            queue_depth: AtomicU64::new(0),
            quality_states: std::array::from_fn(|_| AtomicU64::new(0)),
            asset_stale_total: Mutex::new(BTreeMap::new()),
            ingestion_delay: Mutex::new(IngestionDelaySummary::default()),
            api_request_duration: Mutex::new(DurationHistogram::new()),
            api_in_flight_requests: AtomicU64::new(0),
            command_accept_duration: Mutex::new(DurationHistogram::new()),
            command_state_transitions: Mutex::new(BTreeMap::new()),
            commands_stuck: AtomicU64::new(0),
            webhook_dead_letters: AtomicU64::new(0),
        }
    }
}

impl MetricsRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Records one served request with its outcome labels.
    pub fn record_request(&self, method: &str, route: &str, status: u16, error: bool) {
        let key = format!("method=\"{method}\",route=\"{route}\",status=\"{status}\"");
        let mut map = self.requests_total.lock().expect("metrics mutex poisoned");
        let entry = map
            .entry(key)
            .or_insert_with(|| AtomicCount(AtomicU64::new(0)));
        entry.0.fetch_add(1, Ordering::Relaxed);
        if error {
            self.errors_total.fetch_add(1, Ordering::Relaxed);
        }
    }

    // -----------------------------------------------------------------------
    // P2.21 — telemetry metric recording (see the module header for the
    // recorded cardinality decisions)
    // -----------------------------------------------------------------------

    /// Counts one telemetry domain event appended to the outbox, by frozen
    /// event type (the §15.3 names — anything else is not counted; the
    /// vocabulary is closed).
    pub fn record_telemetry_event(&self, event_type: &str) {
        if !TELEMETRY_EVENT_TYPES.contains(&event_type) {
            return;
        }
        let mut map = self
            .telemetry_events_total
            .lock()
            .expect("metrics mutex poisoned");
        let entry = map
            .entry(event_type.to_string())
            .or_insert_with(|| AtomicCount(AtomicU64::new(0)));
        entry.0.fetch_add(1, Ordering::Relaxed);
    }

    /// Counts one quarantined record under its closed reason class. The
    /// free-text quarantine reason NEVER becomes a label (recorded
    /// cardinality decision).
    pub fn record_quarantine(&self, reason_class: &str) {
        self.quarantine_total.fetch_add(1, Ordering::Relaxed);
        let mut map = self
            .quarantine_by_reason_total
            .lock()
            .expect("metrics mutex poisoned");
        let entry = map
            .entry(reason_class.to_string())
            .or_insert_with(|| AtomicCount(AtomicU64::new(0)));
        entry.0.fetch_add(1, Ordering::Relaxed);
    }

    /// Sets the ingestion bounded-channel depth gauge (the current backlog).
    pub fn set_queue_depth(&self, depth: u64) {
        self.queue_depth.store(depth, Ordering::Relaxed);
    }

    /// Counts one stored telemetry record under its canonical quality state.
    /// Only the six canonical states (P0.08) are counted — a seventh value is
    /// a vocabulary violation and is ignored here rather than inventing a
    /// series. Per-state totals only; per-asset counts go to logs.
    pub fn record_quality_state(&self, state: &str) {
        let Some(index) = QUALITY_STATES.iter().position(|s| *s == state) else {
            return;
        };
        self.quality_states[index].fetch_add(1, Ordering::Relaxed);
    }

    /// Records the ingestion delay of one stored record
    /// (`ingest_time - event_time`, milliseconds). A negative delta (the
    /// source clock ahead of the event clock) clamps to 0 — the summary is
    /// about delay, not clock skew. Per-batch: callers aggregate one value
    /// per stored record; the per-asset breakdown goes to the logs.
    pub fn record_ingestion_delay_millis(&self, delay_millis: i128) {
        let delay = delay_millis.clamp(0, u64::MAX as i128) as u64;
        let mut summary = self.ingestion_delay.lock().expect("metrics mutex poisoned");
        summary.sum_millis = summary.sum_millis.saturating_add(delay);
        summary.count = summary.count.saturating_add(1);
    }

    // -----------------------------------------------------------------------
    // P11.01 — golden-signal SLI recording (see the module header and
    // records/01_p11_01_metric_taxonomy.md for the recorded decisions)
    // -----------------------------------------------------------------------

    /// Observes one served request's duration into the control-plane-api
    /// latency SLI histogram (no labels — recorded decision).
    pub fn record_api_request_duration_millis(&self, millis: u64) {
        let mut histogram = self
            .api_request_duration
            .lock()
            .expect("metrics mutex poisoned");
        histogram.observe(millis);
    }

    /// Marks one request as entered the middleware (in-flight count up).
    /// Paired with [`MetricsRegistry::leave_request`] on the response path.
    pub fn enter_request(&self) {
        self.api_in_flight_requests.fetch_add(1, Ordering::Relaxed);
    }

    /// Marks one request as served (in-flight count down; saturating —
    /// a lopsided call can never drive the gauge negative).
    pub fn leave_request(&self) {
        self.api_in_flight_requests.fetch_sub(1, Ordering::Relaxed);
    }

    /// Observes one command-accept duration into the command-lifecycle
    /// accept-latency SLI histogram (no labels — recorded decision). The
    /// command-submission route (Agent B's files) calls this with the
    /// elapsed milliseconds from request arrival to the committed
    /// PROPOSED record (record 01 §5 handoff note).
    pub fn record_command_accept_duration_millis(&self, millis: u64) {
        let mut histogram = self
            .command_accept_duration
            .lock()
            .expect("metrics mutex poisoned");
        histogram.observe(millis);
    }

    /// Counts one committed command-lifecycle gate transition. BOTH label
    /// values must be names from the frozen `CommandState` vocabulary AND
    /// the edge `(from, to)` must be one of the 27 legal frozen edges —
    /// anything else is a vocabulary violation and is IGNORED here rather
    /// than inventing a series (deny-by-default, same as the P2.21
    /// quality-state recorder).
    pub fn record_command_state_transition(&self, from: &str, to: &str) {
        let Some(from_state) = CommandState::ALL.iter().copied().find(|s| s.name() == from) else {
            return;
        };
        let Some(to_state) = CommandState::ALL.iter().copied().find(|s| s.name() == to) else {
            return;
        };
        // The frozen-edge oracle: an illegal edge is never counted (no
        // metric invents a state or an edge).
        if flexcore_domain::command_state::try_transition(from_state, to_state).is_err() {
            return;
        }
        let mut map = self
            .command_state_transitions
            .lock()
            .expect("metrics mutex poisoned");
        let entry = map
            .entry((from.to_string(), to.to_string()))
            .or_insert(0);
        *entry = entry.saturating_add(1);
    }

    /// P11.20 — sets the stuck-state gauge to the latest scan result. The
    /// ONLY writer is the stuck-state scanner; a failed scan does not call
    /// this, so the gauge keeps its previous value.
    pub fn set_commands_stuck(&self, count: u64) {
        self.commands_stuck.store(count, Ordering::Relaxed);
    }

    /// P13.09 — sets the webhook dead-letter depth gauge to the latest
    /// scan result. The ONLY writer is the webhook delivery scanner
    /// (reading the delivery bookkeeping store's dead-letter count —
    /// migration 0020); a failed read does not call this, so the gauge
    /// keeps its previous value (a scanner outage never reads as "no dead
    /// letters"). The recommended alert threshold is registered in
    /// `webhook_service.rs::DEAD_LETTER_ALERT_THRESHOLD_RECOMMENDED` and
    /// the P13.09 decision record; the threshold itself is an operator
    /// decision on the monitoring side. WRITTEN-UNRUN.
    pub fn set_webhook_dead_letters(&self, count: u64) {
        self.webhook_dead_letters.store(count, Ordering::Relaxed);
    }

    /// Records the staleness evaluation of one asset's latest value. When
    /// stale, the per-asset series is created/incremented; when fresh, the
    /// per-asset series is REMOVED (the bounded per-asset decision — a
    /// series exists only while the asset is currently stale). Returns the
    /// current staleness observation count for the asset (0 when fresh).
    pub fn record_staleness(&self, asset_id: &str, stale: bool) -> u64 {
        let mut map = self
            .asset_stale_total
            .lock()
            .expect("metrics mutex poisoned");
        if stale {
            let count = map.entry(asset_id.to_string()).or_insert(0);
            *count = count.saturating_add(1);
            *count
        } else {
            map.remove(asset_id);
            0
        }
    }

    /// Renders the `/metrics` body (Prometheus text exposition format).
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str("# HELP flexcore_http_requests_total Total HTTP requests.\n");
        out.push_str("# TYPE flexcore_http_requests_total counter\n");
        if let Ok(map) = self.requests_total.lock() {
            for (labels, count) in map.iter() {
                let v = count.0.load(Ordering::Relaxed);
                out.push_str(&format!("flexcore_http_requests_total{{{labels}}} {v}\n"));
            }
        }
        let errors = self.errors_total.load(Ordering::Relaxed);
        out.push_str("# HELP flexcore_http_errors_total Total HTTP 5xx/4xx error responses.\n");
        out.push_str("# TYPE flexcore_http_errors_total counter\n");
        out.push_str(&format!("flexcore_http_errors_total {errors}\n"));

        // --- P2.21 telemetry families (always rendered, zero included) ----
        out.push_str("# HELP flexcore_telemetry_events_total Telemetry domain events appended to the outbox, by frozen event type.\n");
        out.push_str("# TYPE flexcore_telemetry_events_total counter\n");
        if let Ok(map) = self.telemetry_events_total.lock() {
            for event_type in TELEMETRY_EVENT_TYPES {
                let v = map
                    .get(event_type)
                    .map(|c| c.0.load(Ordering::Relaxed))
                    .unwrap_or(0);
                out.push_str(&format!(
                    "flexcore_telemetry_events_total{{event=\"{event_type}\"}} {v}\n"
                ));
            }
        }
        let quarantined = self.quarantine_total.load(Ordering::Relaxed);
        out.push_str("# HELP flexcore_telemetry_quarantine_total Total telemetry records quarantined by the ingestion pipeline.\n");
        out.push_str("# TYPE flexcore_telemetry_quarantine_total counter\n");
        out.push_str(&format!("flexcore_telemetry_quarantine_total {quarantined}\n"));
        out.push_str("# HELP flexcore_telemetry_quarantine_by_reason_total Quarantined telemetry records by closed reason class (the free-text reason is never a label).\n");
        out.push_str("# TYPE flexcore_telemetry_quarantine_by_reason_total counter\n");
        if let Ok(map) = self.quarantine_by_reason_total.lock() {
            for (reason_class, count) in map.iter() {
                let v = count.0.load(Ordering::Relaxed);
                out.push_str(&format!(
                    "flexcore_telemetry_quarantine_by_reason_total{{reason_class=\"{reason_class}\"}} {v}\n"
                ));
            }
        }
        let depth = self.queue_depth.load(Ordering::Relaxed);
        out.push_str("# HELP flexcore_telemetry_queue_depth Ingestion bounded-channel length at the last observation.\n");
        out.push_str("# TYPE flexcore_telemetry_queue_depth gauge\n");
        out.push_str(&format!("flexcore_telemetry_queue_depth {depth}\n"));
        out.push_str("# HELP flexcore_telemetry_quality_states_total Stored telemetry records by canonical quality state (per-state totals; per-asset counts go to logs).\n");
        out.push_str("# TYPE flexcore_telemetry_quality_states_total counter\n");
        for (index, state) in QUALITY_STATES.iter().enumerate() {
            let v = self.quality_states[index].load(Ordering::Relaxed);
            out.push_str(&format!(
                "flexcore_telemetry_quality_states_total{{quality=\"{state}\"}} {v}\n"
            ));
        }
        if let Ok(map) = self.asset_stale_total.lock() {
            let stale_assets = map.len() as u64;
            out.push_str("# HELP flexcore_telemetry_stale_assets Assets currently evaluated stale by the ingestion freshness evaluation.\n");
            out.push_str("# TYPE flexcore_telemetry_stale_assets gauge\n");
            out.push_str(&format!("flexcore_telemetry_stale_assets {stale_assets}\n"));
            out.push_str("# HELP flexcore_telemetry_asset_stale_total Staleness observations per asset; a series exists ONLY while the asset is currently stale (bounded cardinality).\n");
            out.push_str("# TYPE flexcore_telemetry_asset_stale_total counter\n");
            for (asset_id, count) in map.iter() {
                out.push_str(&format!(
                    "flexcore_telemetry_asset_stale_total{{asset_id=\"{asset_id}\"}} {count}\n"
                ));
            }
        }
        if let Ok(summary) = self.ingestion_delay.lock() {
            out.push_str("# HELP flexcore_telemetry_ingestion_delay_milliseconds Per-batch summary of (ingest_time - event_time) in milliseconds over stored records; per-asset breakdown goes to logs.\n");
            out.push_str("# TYPE flexcore_telemetry_ingestion_delay_milliseconds summary\n");
            out.push_str(&format!(
                "flexcore_telemetry_ingestion_delay_milliseconds_sum {}\n",
                summary.sum_millis
            ));
            out.push_str(&format!(
                "flexcore_telemetry_ingestion_delay_milliseconds_count {}\n",
                summary.count
            ));
        }

        // --- P11.01 golden-signal SLI families (always rendered, zero
        // included; record 01 is the naming authority) -------------------
        if let Ok(histogram) = self.api_request_duration.lock() {
            render_duration_histogram(
                &mut out,
                "flexcore_api_request_duration_milliseconds",
                "control-plane-api latency SLI: served request duration (fixed registered buckets; no labels by recorded decision).",
                &histogram,
            );
        }
        let in_flight = self.api_in_flight_requests.load(Ordering::Relaxed);
        out.push_str("# HELP flexcore_api_in_flight_requests control-plane-api saturation: requests currently inside the middleware (no labels by recorded decision).\n");
        out.push_str("# TYPE flexcore_api_in_flight_requests gauge\n");
        out.push_str(&format!("flexcore_api_in_flight_requests {in_flight}\n"));
        if let Ok(histogram) = self.command_accept_duration.lock() {
            render_duration_histogram(
                &mut out,
                "flexcore_command_accept_duration_milliseconds",
                "command-lifecycle accept-latency SLI: request arrival to committed PROPOSED record (fixed registered buckets; no labels by recorded decision).",
                &histogram,
            );
        }
        out.push_str("# HELP flexcore_command_state_transitions_total Committed command-lifecycle gate transitions over the frozen 14-state/27-edge lifecycle (from,to is bounded by the frozen vocabulary; terminal-state ratio is derived in queries, not stored).\n");
        out.push_str("# TYPE flexcore_command_state_transitions_total counter\n");
        if let Ok(map) = self.command_state_transitions.lock() {
            for ((from, to), count) in map.iter() {
                out.push_str(&format!(
                    "flexcore_command_state_transitions_total{{from=\"{from}\",to=\"{to}\"}} {count}\n"
                ));
            }
        }
        // --- P11.20 stuck-state gauge (always rendered, zero included;
        // NO labels — the P11.01 taxonomy name, record 01 §2 family 5) ---
        let stuck = self.commands_stuck.load(Ordering::Relaxed);
        out.push_str("# HELP flexcore_commands_stuck Commands parked in a non-terminal frozen lifecycle state beyond the registered stuck threshold (FAILED and EXPIRED included — they exit only via FALLBACK; no labels by recorded decision, per-state breakdowns go to logs).\n");
        out.push_str("# TYPE flexcore_commands_stuck gauge\n");
        out.push_str(&format!("flexcore_commands_stuck {stuck}\n"));
        // --- P13.09 webhook dead-letter depth gauge (always rendered,
        // zero included; NO labels — the recorded cardinality decision;
        // the plan §2.3 contract: "its depth is a metric with a registered
        // alert threshold; nothing is silently dropped") ---
        let dead_letters = self.webhook_dead_letters.load(Ordering::Relaxed);
        out.push_str("# HELP flexcore_webhook_dead_letters Unresolved webhook deliveries that exhausted their bounded retry schedule and landed in the dead-letter table (migration 0020); no labels by recorded decision, per-delivery reasons stay in the bookkeeping rows.\n");
        out.push_str("# TYPE flexcore_webhook_dead_letters gauge\n");
        out.push_str(&format!("flexcore_webhook_dead_letters {dead_letters}\n"));
        out
    }
}

// ---------------------------------------------------------------------------
// P11.20 — stuck-state detection (scanner, reader port, Postgres reader,
// pure counting). WRITTEN-UNRUN: the reader drives sync adapter ports that
// have never executed; the first human build is the verification gate.
// records/51_p11_20_stuck_state.md.
// ---------------------------------------------------------------------------

/// One command's lifecycle position as the scanner reads it: the command
/// identity, its stored state text (validated against the frozen 14-state
/// vocabulary before use), and the time the command ENTERED its current
/// state (UTC unix millis).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandStateSnapshot {
    /// The command ID (canonical UUID text).
    pub command_id: String,
    /// The stored state text — expected to be one of the 14 frozen
    /// `CommandState::name()` values; anything else is counted as
    /// unjudgeable, never guessed.
    pub state: String,
    /// When the command entered its current state (UTC unix millis).
    /// `None` = the store could not derive it (a store anomaly or a
    /// non-UUIDv7 identifier on a never-transitioned command) — the
    /// command is reported unjudgeable, never silently counted either way.
    pub state_entered_at_millis: Option<i64>,
}

/// The read port the stuck-state scanner consumes. Sync methods (the house
/// rule): the Postgres implementation blocks on the adapter's private
/// runtime, so the scanner calls it under `tokio::task::block_in_place`
/// (the recorded sync-port hosting rule — the same shape as the worker's
/// P11.12 gateway). Defined HERE (app layer) so the domain and the storage
/// crate stay untouched; the pattern is the worker's `CommandStateGateway`.
pub trait CommandStateReader: Send + Sync {
    /// Every registered tenant's id (platform-level listing; the
    /// tenant-scoping rule does not apply to the tenant list itself — the
    /// `TenantRepository::list_all` port contract).
    fn tenant_ids(&self) -> Result<Vec<TenantId>, String>;

    /// All of one tenant's commands currently parked in a NON-terminal
    /// frozen lifecycle state, with their state-entry times. Terminal-state
    /// commands are excluded BY THE READER (they can never leave their
    /// state, so they cannot become stuck); the pure counter still re-checks
    /// with the domain's `is_terminal` (defence in depth, one vocabulary).
    fn parked_commands(&self, ctx: &TenantContext) -> Result<Vec<CommandStateSnapshot>, String>;
}

/// The Postgres implementation over the real store: one unit of work per
/// tenant scan (read-only — dropped without commit, which rolls the native
/// transaction back; the adapter contract, P1.10 rule 4). The scan lists,
/// PER NON-TERMINAL STATE, the tenant's currently-parked commands (keyset
/// pagination, bounded pages): terminal rows accumulate forever for audit,
/// so an unfiltered listing would be unbounded over time, while each
/// per-state filter is bounded by what is actually parked. The state list
/// is DERIVED from the frozen vocabulary (`CommandState::ALL` filtered by
/// `is_terminal`) — no second state list exists.
pub struct PgCommandStateReader {
    store: Arc<flexcore_postgres_adapter::FlexCoreStore>,
}

impl PgCommandStateReader {
    /// Builds the reader over the store (the unit-of-work factory).
    pub fn new(store: Arc<flexcore_postgres_adapter::FlexCoreStore>) -> Self {
        Self { store }
    }
}

impl CommandStateReader for PgCommandStateReader {
    fn tenant_ids(&self) -> Result<Vec<TenantId>, String> {
        let unit = self
            .store
            .begin(None)
            .map_err(|e| format!("stuck-scan tenant unit of work failed: {e}"))?;
        let tenants = TenantRepository::list_all(&unit)
            .map_err(|e| format!("stuck-scan tenant listing failed: {e}"))?;
        // Read-only: drop without commit (the adapter contract).
        drop(unit);
        Ok(tenants.iter().map(|tenant| tenant.tenant_id()).collect())
    }

    fn parked_commands(&self, ctx: &TenantContext) -> Result<Vec<CommandStateSnapshot>, String> {
        let unit = self
            .store
            .begin(Some(ctx))
            .map_err(|e| format!("stuck-scan command unit of work failed: {e}"))?;
        let mut out = Vec::new();
        // The nine non-terminal states of the frozen 14-state vocabulary,
        // derived — never re-listed by hand.
        for state in CommandState::ALL.iter().copied().filter(|s| !is_terminal(*s)) {
            let state_name = state.name();
            let mut cursor: Option<String> = None;
            loop {
                let filter = CommandListFilter {
                    state: Some(state_name.to_string()),
                    after_command_id: cursor.clone(),
                };
                let page = CommandRecordStore::list_commands(&unit, ctx, &filter, STUCK_SCAN_PAGE_SIZE)
                    .map_err(|e| format!("stuck-scan listing of {state_name} commands failed: {e}"))?;
                let page_full = page.len() == STUCK_SCAN_PAGE_SIZE as usize;
                for record in page {
                    let entered = state_entry_time(&unit, ctx, &record)?;
                    out.push(CommandStateSnapshot {
                        command_id: record.row.command_id.clone(),
                        state: record.state.clone(),
                        state_entered_at_millis: entered,
                    });
                    cursor = Some(record.row.command_id);
                }
                if !page_full {
                    break;
                }
            }
        }
        // Read-only: drop without commit (the adapter contract).
        drop(unit);
        Ok(out)
    }
}

/// Derives when a stored command entered its CURRENT state:
///
/// 1. The transition journal is ordered by sequence, and the last entry is
///    the transition INTO the current state — its `at_unix_millis` is the
///    state-entry time (the journal is INSERT-only and every committed
///    transition writes exactly one row, P4.10).
/// 2. An EMPTY journal means the command never transitioned — it is still
///    in its initial PROPOSED state (no frozen edge re-enters PROPOSED, so
///    a non-PROPOSED state with an empty journal is a store anomaly). Its
///    state-entry time is its creation time: the D-S1 identifier policy
///    makes every command ID a UUIDv7, whose top 48 bits ARE the unix
///    creation milliseconds ([`uuidv7_unix_millis`]). A non-UUIDv7 id (or
///    the anomaly) yields `None` — the command is reported unjudgeable,
///    never guessed.
fn state_entry_time(
    unit: &flexcore_postgres_adapter::PgUnitOfWork,
    ctx: &TenantContext,
    record: &flexcore_storage::command_store::CommandRecord,
) -> Result<Option<i64>, String> {
    let journal = CommandRecordStore::transition_journal(unit, ctx, &record.row.command_id)
        .map_err(|e| {
            format!(
                "stuck-scan journal read for command {} failed: {e}",
                record.row.command_id
            )
        })?;
    if let Some(last) = journal.last() {
        // Consistency check: the last journal entry must have ENTERED the
        // stored state. A mismatch is a store anomaly — unjudgeable, never
        // a guessed timestamp.
        if last.to_state != record.state {
            tracing::warn!(
                command_id = %record.row.command_id,
                stored_state = %record.state,
                journal_to_state = %last.to_state,
                "stuck-state scan: stored state and journal disagree; the command is \
                 not judgeable for stuck-state purposes"
            );
            return Ok(None);
        }
        return Ok(Some(last.at_unix_millis));
    }
    // Empty journal: the command must be in its initial PROPOSED state.
    if record.state != CommandState::Proposed.name() {
        tracing::warn!(
            command_id = %record.row.command_id,
            stored_state = %record.state,
            "stuck-state scan: non-PROPOSED command with an empty transition journal; \
             the command is not judgeable for stuck-state purposes"
        );
        return Ok(None);
    }
    Ok(uuidv7_unix_millis(&record.row.command_id))
}

/// Extracts the unix-millisecond creation timestamp from a canonical UUIDv7
/// text (D-S1: every command ID is a PostgreSQL 18 `uuidv7()`): the first
/// 12 hex digits are the 48-bit `unix_ts_ms` field, and the 13th hex digit
/// carries the version nibble, which must be 7. Anything else (malformed
/// text, another UUID version) is `None` — never a guessed timestamp.
///
/// ASSUMPTION (recorded): the D-S1 policy guarantees UUIDv7 command IDs;
/// the version-bit check fails closed if a deployment composes a different
/// generator.
pub fn uuidv7_unix_millis(text: &str) -> Option<i64> {
    let bytes = text.as_bytes();
    // Canonical UUID text: 36 chars, hyphens at 8/13/18/23.
    if bytes.len() != 36
        || bytes[8] != b'-'
        || bytes[13] != b'-'
        || bytes[18] != b'-'
        || bytes[23] != b'-'
    {
        return None;
    }
    let hex: String = text.chars().filter(|c| *c != '-').collect();
    // Exactly 32 ASCII hex digits — the fail-closed guard that also makes
    // the byte-indexed slicing below panic-free on any input.
    if hex.len() != 32 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let version = u8::from_str_radix(&hex[12..13], 16).ok()?;
    if version != 7 {
        return None;
    }
    i64::from_str_radix(&hex[0..12], 16).ok()
}

/// The result of one stuck-state scan: the gauge value (the total) plus the
/// per-state breakdown that goes to LOGS, never to labels (the recorded
/// cardinality decision), plus the count of commands the store could not
/// judge.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct StuckScanOutcome {
    /// Commands parked beyond the threshold — the gauge value.
    pub stuck_total: u64,
    /// The per-state breakdown (state name → stuck count). Logs only.
    pub stuck_by_state: BTreeMap<String, u64>,
    /// Commands whose state text is outside the frozen vocabulary or whose
    /// state-entry time is not derivable — surfaced by the scanner's logs,
    /// never counted as stuck and never silently dropped.
    pub unjudgeable: u64,
}

impl StuckScanOutcome {
    fn merge(&mut self, other: StuckScanOutcome) {
        self.stuck_total = self.stuck_total.saturating_add(other.stuck_total);
        for (state, count) in other.stuck_by_state {
            *self.stuck_by_state.entry(state).or_insert(0) += count;
        }
        self.unjudgeable = self.unjudgeable.saturating_add(other.unjudgeable);
    }
}

/// The PURE stuck-state classifier (unit-tested, no I/O): over the given
/// snapshots, counts every command whose state is non-terminal in the
/// frozen vocabulary AND whose park duration (`now_millis − state entry`)
/// is STRICTLY GREATER than the threshold. A command exactly AT the
/// threshold is not yet beyond it. A future-dated entry time (a source
/// clock ahead of the scanner's) clamps to age zero — never negative, so
/// never spuriously stuck. Terminal states never count (the frozen table
/// gives them no outgoing edge, so they cannot become stuck — the domain's
/// `is_terminal` is the authority). Unknown state text and missing entry
/// times count as unjudgeable.
pub fn count_stuck_commands(
    now_millis: i64,
    threshold_millis: i64,
    commands: &[CommandStateSnapshot],
) -> StuckScanOutcome {
    let mut outcome = StuckScanOutcome::default();
    for command in commands {
        let Some(state) = CommandState::ALL.iter().copied().find(|s| s.name() == command.state)
        else {
            outcome.unjudgeable += 1;
            continue;
        };
        // Defence in depth: the reader already excludes terminal states;
        // the classifier re-checks against the SAME domain function.
        if is_terminal(state) {
            continue;
        }
        let Some(entered) = command.state_entered_at_millis else {
            outcome.unjudgeable += 1;
            continue;
        };
        let age = now_millis.saturating_sub(entered).max(0);
        if age > threshold_millis {
            outcome.stuck_total += 1;
            *outcome
                .stuck_by_state
                .entry(command.state.clone())
                .or_insert(0) += 1;
        }
    }
    outcome
}

/// One full scan over every tenant (sync reader; caller hosts it under
/// `block_in_place`). Pure over the reader's results apart from the reader
/// itself, so tests inject a fake.
fn scan_all_tenants(
    reader: &dyn CommandStateReader,
    now_millis: i64,
    threshold_millis: i64,
) -> Result<StuckScanOutcome, String> {
    let mut outcome = StuckScanOutcome::default();
    for tenant_id in reader.tenant_ids()? {
        // The scanner's tenant context: constructed from fixed verified
        // claims exactly once per tenant (the single greppable path, P0.16)
        // — a read-only service identity, never an operator's.
        let ctx = TenantContext::from_authenticated_claims(AuthenticatedClaims::from_verified(
            VerifiedClaims {
                tenant_id,
                subject: "control-plane:stuck-scanner".to_string(),
            },
        ));
        let snapshots = reader.parked_commands(&ctx)?;
        outcome.merge(count_stuck_commands(now_millis, threshold_millis, &snapshots));
    }
    Ok(outcome)
}

/// The P11.20 scanner loop until shutdown: scan every tenant, set the
/// `flexcore_commands_stuck` gauge, log the per-state breakdown (logs,
/// never labels), sleep, repeat. The scan runs under
/// `tokio::task::block_in_place` because the sync reader blocks on the
/// Postgres adapter's private runtime (which panics inside a plain
/// `block_on` from an async context — the recorded hosting rule); the
/// hosting runtime must therefore be multi-threaded (the composition
/// roots' runtimes are). On a scan ERROR the gauge keeps its PREVIOUS
/// value — a scan outage never reads as "no stuck commands" — and the
/// error is logged. WRITTEN-UNRUN.
pub async fn run_stuck_state_scanner(
    reader: Arc<dyn CommandStateReader>,
    metrics: Arc<MetricsRegistry>,
    interval: Duration,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
) {
    loop {
        if *shutdown.borrow() {
            return;
        }
        let now_millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
            .unwrap_or(0);
        // The reader's port calls block on the adapter's private runtime —
        // the blocking section runs off the async executor's worker thread
        // (block_in_place), never a plain block_on inside the runtime.
        let scan = tokio::task::block_in_place(|| {
            scan_all_tenants(
                reader.as_ref(),
                now_millis,
                COMMANDS_STUCK_THRESHOLD_MILLIS,
            )
        });
        match scan {
            Ok(outcome) => {
                metrics.set_commands_stuck(outcome.stuck_total);
                if !outcome.stuck_by_state.is_empty() {
                    // Per-state breakdown: logs, never labels (the recorded
                    // cardinality decision, record 01 family 5).
                    tracing::warn!(
                        stuck_total = outcome.stuck_total,
                        by_state = ?outcome.stuck_by_state,
                        "stuck-state scan: commands parked in non-terminal states beyond \
                         the registered threshold"
                    );
                } else {
                    tracing::info!(
                        stuck_total = 0,
                        "stuck-state scan: no commands parked beyond the registered threshold"
                    );
                }
                if outcome.unjudgeable > 0 {
                    tracing::warn!(
                        unjudgeable = outcome.unjudgeable,
                        "stuck-state scan: some commands could not be judged (state text \
                         outside the frozen vocabulary, a store/journal disagreement, or a \
                         non-UUIDv7 identifier); they are excluded from the gauge"
                    );
                }
            }
            Err(error) => {
                // The gauge keeps its previous value on a failed scan.
                tracing::error!(
                    error = %error,
                    "stuck-state scan failed; flexcore_commands_stuck keeps its previous value"
                );
            }
        }
        // Sleep, or return early when shutdown fires.
        tokio::select! {
            () = tokio::time::sleep(interval) => {}
            changed = shutdown.changed() => {
                let _ = changed;
                return;
            }
        }
    }
}

/// Renders one fixed-bucket latency histogram in the Prometheus text
/// exposition format (P11.01): cumulative `_bucket{le}` series over the
/// registered bucket set (no labels — the recorded cardinality decision),
/// then `_bucket{le="+Inf"}`, `_sum`, and `_count`. The `+Inf` bucket
/// equals the total observation count by construction.
fn render_duration_histogram(out: &mut String, family: &str, help: &str, histogram: &DurationHistogram) {
    out.push_str(&format!("# HELP {family} {help}\n"));
    out.push_str(&format!("# TYPE {family} histogram\n"));
    for (index, bound) in SLI_LATENCY_BUCKETS_MILLIS.iter().enumerate() {
        let v = histogram.cumulative[index];
        out.push_str(&format!("{family}_bucket{{le=\"{bound}\"}} {v}\n"));
    }
    out.push_str(&format!("{family}_bucket{{le=\"+Inf\"}} {}\n", histogram.count));
    out.push_str(&format!("{family}_sum {}\n", histogram.sum_millis));
    out.push_str(&format!("{family}_count {}\n", histogram.count));
}

/// The P11.07 SeverityNumber mapping for `tracing` levels onto the OTel
/// log-severity scale ([S-62] data model: TRACE 1–4, DEBUG 5–8, INFO
/// 9–12, WARN 13–16, ERROR 17–20, FATAL 21–24). The base value of each
/// band is used (`tracing` has no per-band sub-levels); `tracing`'s
/// `trace!` maps to 1, `debug!` to 5, `info!` to 9, `warn!` to 13,
/// `error!` to 17. Unknown level strings map to 0 (`UNSPECIFIED`) rather
/// than guessing a band. The same numeric mapping is defined for the
/// Python emitter (structlog/stdlib level names) in the log-schema record.
pub fn severity_number_for_level(level: &str) -> u8 {
    match level {
        "TRACE" => 1,
        "DEBUG" => 5,
        "INFO" => 9,
        "WARN" => 13,
        "ERROR" => 17,
        "FATAL" => 21,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The P2.21 metric-inventory check: every telemetry family renders,
    /// including the six canonical quality states at zero.
    #[test]
    fn render_exposes_all_six_telemetry_families_and_quality_states() {
        let registry = MetricsRegistry::new();
        let body = registry.render();
        for family in [
            "flexcore_telemetry_events_total",
            "flexcore_telemetry_quarantine_total",
            "flexcore_telemetry_queue_depth",
            "flexcore_telemetry_quality_states_total",
            "flexcore_telemetry_stale_assets",
            "flexcore_telemetry_ingestion_delay_milliseconds",
        ] {
            assert!(body.contains(family), "missing metric family {family}");
        }
        for state in QUALITY_STATES {
            assert!(
                body.contains(&format!("quality=\"{state}\"")),
                "quality state {state} must be observable"
            );
        }
    }

    /// The recording API drives the rendered values; unknown vocabulary is
    /// never counted (deny-by-default extends to metric labels).
    #[test]
    fn telemetry_recording_updates_the_families_with_closed_vocabularies() {
        let registry = MetricsRegistry::new();
        registry.record_telemetry_event("TelemetryReceived");
        registry.record_telemetry_event("TelemetryReceived");
        registry.record_telemetry_event("NotAFrozenEvent");
        registry.record_quarantine("decode_failed");
        registry.record_quality_state("STALE");
        registry.record_quality_state("PARTIALLY_VALID");
        registry.set_queue_depth(7);
        registry.record_ingestion_delay_millis(-5);
        registry.record_ingestion_delay_millis(120);
        let stale = registry.record_staleness("asset-a", true);
        registry.record_staleness("asset-b", true);
        registry.record_staleness("asset-b", false);
        assert_eq!(stale, 1);

        let body = registry.render();
        assert!(body.contains(
            "flexcore_telemetry_events_total{event=\"TelemetryReceived\"} 2"
        ));
        assert!(!body.contains("NotAFrozenEvent"));
        assert!(body.contains("flexcore_telemetry_quarantine_total 1"));
        assert!(body.contains("reason_class=\"decode_failed\""));
        assert!(body.contains("quality=\"STALE\"} 1"));
        assert!(body.contains("flexcore_telemetry_queue_depth 7"));
        // The negative delay clamped to 0: sum = 0 + 120, count = 2.
        assert!(body.contains("flexcore_telemetry_ingestion_delay_milliseconds_sum 120"));
        assert!(body.contains("flexcore_telemetry_ingestion_delay_milliseconds_count 2"));
        // Only the currently-stale asset keeps a series.
        assert!(body.contains("asset_id=\"asset-a\""));
        assert!(!body.contains("asset_id=\"asset-b\""));
        assert!(body.contains("flexcore_telemetry_stale_assets 1"));
    }

    /// P11.01 — the golden-signal SLI families render always (zero
    /// included), and the latency histograms use the fixed registered
    /// bucket set with cumulative `_bucket` series plus `_sum`/`_count`.
    #[test]
    fn sli_families_render_with_the_registered_bucket_set() {
        let registry = MetricsRegistry::new();
        let body = registry.render();
        for family in [
            "flexcore_api_request_duration_milliseconds",
            "flexcore_api_in_flight_requests",
            "flexcore_command_accept_duration_milliseconds",
            "flexcore_command_state_transitions_total",
        ] {
            assert!(body.contains(family), "missing SLI family {family}");
        }
        // Every registered bucket renders even at zero observations.
        for bound in SLI_LATENCY_BUCKETS_MILLIS {
            assert!(
                body.contains(&format!(
                    "flexcore_api_request_duration_milliseconds_bucket{{le=\"{bound}\"}} 0"
                )),
                "bucket le=\"{bound}\" must render at zero"
            );
        }
        assert!(body.contains(
            "flexcore_api_request_duration_milliseconds_bucket{le=\"+Inf\"} 0"
        ));
        assert!(body.contains("flexcore_api_in_flight_requests 0"));
    }

    /// P11.01 — the latency histograms observe into every fitting bucket,
    /// and the in-flight gauge saturates rather than going negative.
    #[test]
    fn sli_histograms_observe_cumulatively_and_the_gauge_saturates() {
        let registry = MetricsRegistry::new();
        registry.record_api_request_duration_millis(60);
        registry.record_api_request_duration_millis(1_500);
        registry.record_command_accept_duration_millis(9);
        let body = registry.render();
        // 60 ms lands in le=100 and above; 1_500 ms lands in le=2500 and
        // above. Cumulative bucket semantics: le=100 counts 1, le=2500
        // counts 2, le=10000 counts 2, +Inf counts 2.
        assert!(body.contains(
            "flexcore_api_request_duration_milliseconds_bucket{le=\"50\"} 0"
        ));
        assert!(body.contains(
            "flexcore_api_request_duration_milliseconds_bucket{le=\"100\"} 1"
        ));
        assert!(body.contains(
            "flexcore_api_request_duration_milliseconds_bucket{le=\"2500\"} 2"
        ));
        assert!(body.contains(
            "flexcore_api_request_duration_milliseconds_bucket{le=\"+Inf\"} 2"
        ));
        assert!(body.contains("flexcore_api_request_duration_milliseconds_sum 1560"));
        assert!(body.contains("flexcore_api_request_duration_milliseconds_count 2"));
        assert!(body.contains(
            "flexcore_command_accept_duration_milliseconds_bucket{le=\"10\"} 1"
        ));
        assert!(body.contains("flexcore_command_accept_duration_milliseconds_count 1"));
        registry.enter_request();
        registry.enter_request();
        registry.leave_request();
        assert!(registry.render().contains("flexcore_api_in_flight_requests 1"));
        registry.leave_request();
        registry.leave_request();
        // A lopsided leave saturates at zero, never negative.
        assert!(registry.render().contains("flexcore_api_in_flight_requests 0"));
    }

    /// P11.01 — the transition counter counts ONLY the legal frozen edges
    /// (the 27-edge lifecycle is the closed vocabulary); label values come
    /// from `CommandState::name()` and an illegal edge or a foreign state
    /// name is ignored, never rendered as a series.
    #[test]
    fn command_transitions_count_only_legal_frozen_edges() {
        let registry = MetricsRegistry::new();
        registry.record_command_state_transition("PROPOSED", "VALIDATED");
        registry.record_command_state_transition("PROPOSED", "VALIDATED");
        registry.record_command_state_transition("VALIDATED", "FEASIBLE");
        // Illegal edge: never counted, never rendered.
        registry.record_command_state_transition("COMPLETED", "ISSUED");
        // Foreign state names: never counted, never rendered.
        registry.record_command_state_transition("OPEN", "CLOSED");
        let body = registry.render();
        assert!(body.contains(
            "flexcore_command_state_transitions_total{from=\"PROPOSED\",to=\"VALIDATED\"} 2"
        ));
        assert!(body.contains(
            "flexcore_command_state_transitions_total{from=\"VALIDATED\",to=\"FEASIBLE\"} 1"
        ));
        assert!(!body.contains("COMPLETED"));
        assert!(!body.contains("OPEN"));
    }

    /// P11.07 — the SeverityNumber mapping uses the OTel band bases and
    /// maps unknown level strings to UNSPECIFIED (0), never a guessed band.
    #[test]
    fn severity_number_mapping_covers_the_tracing_levels() {
        assert_eq!(severity_number_for_level("TRACE"), 1);
        assert_eq!(severity_number_for_level("DEBUG"), 5);
        assert_eq!(severity_number_for_level("INFO"), 9);
        assert_eq!(severity_number_for_level("WARN"), 13);
        assert_eq!(severity_number_for_level("ERROR"), 17);
        assert_eq!(severity_number_for_level("FATAL"), 21);
        assert_eq!(severity_number_for_level("not-a-level"), 0);
        assert_eq!(severity_number_for_level(""), 0);
    }

    // --- P11.20 — stuck-state detection ------------------------------------

    /// P11.20 — the stuck gauge renders ALWAYS (zero included), as a gauge,
    /// with NO labels, under exactly the P11.01 taxonomy name; the setter
    /// drives the rendered value.
    #[test]
    fn stuck_gauge_renders_label_free_under_the_taxonomy_name() {
        let registry = MetricsRegistry::new();
        let body = registry.render();
        assert!(
            body.contains("# TYPE flexcore_commands_stuck gauge"),
            "flexcore_commands_stuck must render as a gauge at zero"
        );
        assert!(
            body.contains("flexcore_commands_stuck 0\n"),
            "the zero value must render (always-render discipline)"
        );
        assert!(
            !body.contains("flexcore_commands_stuck{"),
            "the family carries NO labels (record 01 §2 family 5)"
        );
        registry.set_commands_stuck(4);
        assert!(registry.render().contains("flexcore_commands_stuck 4\n"));
    }

    /// P11.20 — the pure classifier: non-terminal commands beyond the
    /// threshold count (FAILED and EXPIRED included); a command exactly AT
    /// the threshold is not yet beyond it; terminal states never count;
    /// unknown state text and missing entry times are unjudgeable; a
    /// future-dated entry time clamps to age zero.
    #[test]
    fn stuck_classifier_counts_only_beyond_threshold_non_terminal_states() {
        let now = 1_700_000_100_000i64;
        let threshold = 86_400_000i64;
        let snapshots = vec![
            // FAILED parked 25 h: beyond the threshold — counts.
            CommandStateSnapshot {
                command_id: "c-failed".to_string(),
                state: "FAILED".to_string(),
                state_entered_at_millis: Some(now - 25 * 3_600_000),
            },
            // EXPIRED parked 25 h: counts (FAILED/EXPIRED exit only via
            // FALLBACK — non-terminal).
            CommandStateSnapshot {
                command_id: "c-expired".to_string(),
                state: "EXPIRED".to_string(),
                state_entered_at_millis: Some(now - 25 * 3_600_000),
            },
            // PROPOSED parked exactly AT the threshold: not beyond it.
            CommandStateSnapshot {
                command_id: "c-exact".to_string(),
                state: "PROPOSED".to_string(),
                state_entered_at_millis: Some(now - threshold),
            },
            // TRACKING parked 1 h: fresh.
            CommandStateSnapshot {
                command_id: "c-fresh".to_string(),
                state: "TRACKING".to_string(),
                state_entered_at_millis: Some(now - 3_600_000),
            },
            // COMPLETED parked a year: terminal — never counts.
            CommandStateSnapshot {
                command_id: "c-terminal".to_string(),
                state: "COMPLETED".to_string(),
                state_entered_at_millis: Some(now - 365 * 86_400_000),
            },
            // Unknown state text: unjudgeable.
            CommandStateSnapshot {
                command_id: "c-unknown".to_string(),
                state: "PARKED_FOREVER".to_string(),
                state_entered_at_millis: Some(now - 86_400_000),
            },
            // Missing entry time: unjudgeable.
            CommandStateSnapshot {
                command_id: "c-unjudged".to_string(),
                state: "ISSUED".to_string(),
                state_entered_at_millis: None,
            },
            // Future-dated entry: age clamps to zero — not stuck.
            CommandStateSnapshot {
                command_id: "c-future".to_string(),
                state: "APPROVED".to_string(),
                state_entered_at_millis: Some(now + 86_400_000),
            },
        ];
        let outcome = count_stuck_commands(now, threshold, &snapshots);
        assert_eq!(outcome.stuck_total, 2);
        assert_eq!(outcome.stuck_by_state.get("FAILED"), Some(&1));
        assert_eq!(outcome.stuck_by_state.get("EXPIRED"), Some(&1));
        assert_eq!(outcome.stuck_by_state.len(), 2);
        assert_eq!(outcome.unjudgeable, 2);
    }

    /// P11.20 — the classifier's terminal-state exclusion uses the domain's
    /// frozen vocabulary: every one of the five terminal states is skipped
    /// regardless of age, and every non-terminal state can count.
    #[test]
    fn stuck_classifier_uses_the_frozen_terminal_vocabulary() {
        let now = 1_000_000_000_000i64;
        let ancient = Some(now - 100 * 86_400_000);
        for state in CommandState::ALL {
            let outcome = count_stuck_commands(
                now,
                86_400_000,
                &[CommandStateSnapshot {
                    command_id: "c".to_string(),
                    state: state.name().to_string(),
                    state_entered_at_millis: ancient,
                }],
            );
            if is_terminal(state) {
                assert_eq!(
                    outcome.stuck_total, 0,
                    "terminal state {} must never count",
                    state.name()
                );
            } else {
                assert_eq!(
                    outcome.stuck_total, 1,
                    "non-terminal state {} must count when ancient",
                    state.name()
                );
            }
        }
    }

    /// P11.20 — the UUIDv7 creation-time extraction: a canonical UUIDv7
    /// (D-S1) yields its 48-bit unix_ts_ms; wrong version, malformed text,
    /// and non-canonical shapes fail closed to `None`.
    #[test]
    fn uuidv7_extraction_reads_the_timestamp_field_and_fails_closed() {
        // unix_ms 1_700_000_000_000 = 0x18BCFE568E00 in the top 48 bits;
        // version nibble 7, variant bits set.
        let uuid = "18bcfe56-8e00-7000-8000-000000000000";
        assert_eq!(uuidv7_unix_millis(uuid), Some(1_700_000_000_000));
        // Uppercase hex is still canonical UUID text.
        assert_eq!(
            uuidv7_unix_millis("18BCFE56-8E00-7000-8000-000000000000"),
            Some(1_700_000_000_000)
        );
        // Version 4: fail closed (not a D-S1 identifier).
        assert_eq!(uuidv7_unix_millis("18bcfe56-8e00-4000-8000-000000000000"), None);
        // Wrong length / missing hyphens: fail closed.
        assert_eq!(uuidv7_unix_millis("18bcfe568e0070008000000000000000"), None);
        assert_eq!(uuidv7_unix_millis("not-a-uuid"), None);
        assert_eq!(uuidv7_unix_millis(""), None);
    }

    /// P11.20 — the full scan over an injected reader: per-tenant results
    /// merge, the reader's terminal exclusion is re-checked, and errors
    /// surface as named errors (the gauge-keeping behavior lives in the
    /// hosting loop, which is never run here).
    #[test]
    fn scan_all_tenants_merges_per_tenant_outcomes() {
        struct FakeReader {
            tenants: Vec<TenantId>,
            calls: std::sync::Mutex<usize>,
        }
        let tenant_a = {
            let mut bytes = [0u8; 16];
            bytes[15] = 1;
            TenantId::new(flexcore_contracts::ids::FlexUuid::from_bytes(bytes))
        };
        let tenant_b = {
            let mut bytes = [0u8; 16];
            bytes[15] = 2;
            TenantId::new(flexcore_contracts::ids::FlexUuid::from_bytes(bytes))
        };
        let reader = FakeReader {
            tenants: vec![tenant_a, tenant_b],
            calls: std::sync::Mutex::new(0),
        };
        impl CommandStateReader for FakeReader {
            fn tenant_ids(&self) -> Result<Vec<TenantId>, String> {
                Ok(self.tenants.clone())
            }
            fn parked_commands(
                &self,
                _ctx: &TenantContext,
            ) -> Result<Vec<CommandStateSnapshot>, String> {
                *self.calls.lock().unwrap() += 1;
                let call = *self.calls.lock().unwrap();
                Ok(match call {
                    1 => vec![
                        CommandStateSnapshot {
                            command_id: "a-stuck".to_string(),
                            state: "FAILED".to_string(),
                            state_entered_at_millis: Some(0),
                        },
                        // Terminal state from the reader: re-checked and
                        // skipped by the classifier (defence in depth).
                        CommandStateSnapshot {
                            command_id: "a-terminal".to_string(),
                            state: "FALLBACK".to_string(),
                            state_entered_at_millis: Some(0),
                        },
                    ],
                    _ => vec![CommandStateSnapshot {
                        command_id: "b-stuck".to_string(),
                        state: "EXPIRED".to_string(),
                        state_entered_at_millis: Some(0),
                    }],
                })
            }
        }
        let outcome = scan_all_tenants(&reader, 86_400_001, 86_400_000).expect("scan succeeds");
        assert_eq!(outcome.stuck_total, 2);
        assert_eq!(outcome.stuck_by_state.get("FAILED"), Some(&1));
        assert_eq!(outcome.stuck_by_state.get("EXPIRED"), Some(&1));
        assert_eq!(outcome.unjudgeable, 0);
        assert_eq!(*reader.calls.lock().unwrap(), 2);
    }
}

// ---------------------------------------------------------------------------
// P1.20 — trace-id generation (documented decision)
// ---------------------------------------------------------------------------

/// Per-process request counter feeding the trace-id hash.
static TRACE_COUNTER: AtomicU64 = AtomicU64::new(0);

/// A 32-hex-character, locally generated correlation id: a std-library
/// hash over (wall-clock nanoseconds, per-process counter). Recorded
/// decision: this is deliberately NOT a UUID — the std hash keeps the
/// dependency set minimal, and the id's only job is to correlate the one
/// JSON log line per request with the error responses' opaque "see server
/// logs with the trace id" pointer. It carries no cross-process guarantee.
pub fn next_trace_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let counter = TRACE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut first = std::collections::hash_map::DefaultHasher::new();
    first.write_u128(nanos as u128);
    first.write_u64(counter);
    let a = first.finish();
    // Widen to 64 more bits so the id is 32 hex chars, not 16.
    let mut second = std::collections::hash_map::DefaultHasher::new();
    second.write_u64(a);
    second.write_u64(!a);
    let b = second.finish();
    format!("{a:016x}{b:016x}")
}

// ---------------------------------------------------------------------------
// P1.20 — the request recorder middleware
// ---------------------------------------------------------------------------

/// Records one request in the metrics registry and emits the correlation
/// JSON log line. Mounted outermost (before timeout/limit layers) so
/// timeout- and limit-generated responses are recorded too. The route label
/// uses the MATCHED path pattern (no raw path cardinality explosion).
pub async fn track_metrics(
    State(state): State<crate::state::AppState>,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let method = request.method().to_string();
    let route = request
        .extensions()
        .get::<axum::extract::MatchedPath>()
        .map(|p| p.as_str().to_string())
        .unwrap_or_else(|| request.uri().path().to_string());
    // Correlation identity: resolved for the log line only, best effort.
    // The Authorization header itself is never logged (P1.20 no-secrets rule).
    let bearer = request
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    let (tenant, actor) = {
        match state.identity.resolve(bearer.as_deref()).ok() {
            Some(caller) => (
                Some(caller.ctx.tenant_id().uuid().to_hyphenated()),
                Some(caller.ctx.subject().to_string()),
            ),
            None => (None, None),
        }
    };

    let started = Instant::now();
    // P11.01 saturation: the request is in flight from here to the
    // response hand-back (the gauge is maintained by the same middleware
    // that records the request/error families).
    state.metrics.enter_request();
    let response = next.run(request).await;
    state.metrics.leave_request();
    let status = response.status().as_u16();
    let duration_millis = started.elapsed().as_millis() as u64;
    state
        .metrics
        .record_request(&method, &route, status, status >= 400);
    state
        .metrics
        .record_api_request_duration_millis(duration_millis);
    // P11.07 — structured-log schema v1 correlation line. Field contract:
    // records/07_p11_07_log_schema_v1.md. `command_id` is the empty
    // string on this generic middleware line: the command-scoped emitters
    // (the command routes — Agent B's files) populate it from the
    // command's identity; the middleware never parses request bodies
    // (P1.20 no-content rule). `tenant` was renamed `tenant_id` per the
    // schema's canonical attribute names (record 07).
    tracing::info!(
        schema_version = LOG_SCHEMA_VERSION,
        severity_number = severity_number_for_level("INFO"),
        trace_id = next_trace_id().as_str(),
        command_id = "",
        method = %method,
        route = %route,
        status = status,
        duration_ms = duration_millis,
        tenant_id = tenant.as_deref().unwrap_or(""),
        actor = actor.as_deref().unwrap_or(""),
        "request served"
    );
    response
}

