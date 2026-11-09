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

use std::collections::BTreeMap;
use std::hash::Hasher as _;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use crate::auth::IdentityResolver as _;

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
        out
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
    let response = next.run(request).await;
    let status = response.status().as_u16();
    state
        .metrics
        .record_request(&method, &route, status, status >= 400);
    tracing::info!(
        trace_id = next_trace_id().as_str(),
        method = %method,
        route = %route,
        status = status,
        duration_ms = started.elapsed().as_millis() as u64,
        tenant = tenant.as_deref().unwrap_or(""),
        actor = actor.as_deref().unwrap_or(""),
        "request served"
    );
    response
}

