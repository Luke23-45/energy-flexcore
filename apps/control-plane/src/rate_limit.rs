// P11.14 — Per-tenant token-bucket rate limiting + in-flight command caps
// (Phase 11 wave 1, Agent B; ledger P11.14; [S-85] the API-edge rate-limit
// practice; gap analysis §5 Tier 0 row 6).
//
// # Principal classes BEFORE thresholds (the P11.14 ordering rule)
//
// The plan requires the principal classes to be DEFINED before any
// threshold, so a bucket sized for interactive operators can never starve
// the fleet uplink that shares the same edge (plan §5 risk 4). Three
// classes, closed set:
//
// | Class | Definition (subject prefix) | Why it exists |
// |---|---|---|
// | [`PrincipalClass::Operator`] | everything else (interactive operator / console / CLI) | the default human budget |
// | [`PrincipalClass::EdgeAgentUplink`] | subject starts with `edge-agent:` or `edge:` | an EXPLICIT class so fleet uplink is never starved by interactive-operator budgets |
// | [`PrincipalClass::Plugin`] | subject starts with `plugin:` | the bounded third-party class |
//
// The subject-prefix classification is an ASSUMPTION pending the real
// identity integration (no production OIDC exists in 0.x — the recorded
// P1.17 status; the dev-auth resolver's subject format is the only
// observed shape). The classification function is the single place to
// amend when real subjects land.
//
// # The contract
//
// - A request that exceeds its class bucket gets **429** with a
//   `Retry-After` header (whole seconds, ceil of the refill wait, minimum
//   1) and the stable error envelope (`rate_limited`) — the [S-85]
//   contract shape.
// - Per-tenant IN-FLIGHT CAPS apply to the command-submission POST class
//   ([`crate::routes::COMMAND_SUBMISSION_PATHS`]): over the cap is 429
//   with `Retry-After: 1` (a poll-style retry; the cap releases as the
//   requests complete).
// - `/health`, `/ready`, and `/metrics` are NEVER rate limited (probes
//   and scraping must not depend on request budgets — recorded).
// - Unauthenticated requests pass through untouched (the route handler's
//   401 stays authoritative; budgets are per principal, and an
//   unauthenticated request has none).
//
// # Thresholds are registered values, not tuned claims (X-8)
//
// Every number below is a REGISTERED RECOMMENDATION written down BEFORE
// any run that could fill it, pending operator sign-off on real data. No
// capacity, sufficiency, or performance claim attaches to any of them.
//
// # Scale-out honesty
//
// This limiter is IN-PROCESS and per-instance. With more than one control
// plane instance the effective budget multiplies per instance (each
// process keeps its own buckets and in-flight counters). The Redis-backed
// shared limiter is RECORDED as the multi-instance path in the P11.14
// decision record — deliberately NOT built here (no dependency, no
// deployment change in this phase). Single-instance is the recorded
// deployment shape until that record is acted on.
//
// Hand-rolled with `std::time::Instant` and mutexes only — no rate-limit
// dependency is added (the wave-1 dependency-contention rule).

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;

use crate::error::ErrorEnvelope;
use crate::state::AppState;

/// The principal classes (closed set — see the module header).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PrincipalClass {
    /// Interactive operator / console / CLI budget (the default class).
    Operator,
    /// The edge-agent fleet uplink — an EXPLICIT class so fleet uplink is
    /// never starved by interactive-operator budgets.
    EdgeAgentUplink,
    /// The bounded third-party plugin class.
    Plugin,
}

impl PrincipalClass {
    /// The stable class name (used in the log line and the bucket key).
    pub const fn as_str(self) -> &'static str {
        match self {
            PrincipalClass::Operator => "operator",
            PrincipalClass::EdgeAgentUplink => "edge_agent_uplink",
            PrincipalClass::Plugin => "plugin",
        }
    }

    /// The registered token-bucket refill rate for the class (requests
    /// per second). REGISTERED RECOMMENDATION (X-8), pending operator
    /// sign-off on real data.
    pub const fn refill_per_second(self) -> f64 {
        match self {
            PrincipalClass::Operator => 10.0,
            PrincipalClass::EdgeAgentUplink => 50.0,
            PrincipalClass::Plugin => 2.0,
        }
    }

    /// The registered burst capacity for the class (tokens). REGISTERED
    /// RECOMMENDATION (X-8), pending operator sign-off.
    pub const fn burst(self) -> f64 {
        match self {
            PrincipalClass::Operator => 40.0,
            PrincipalClass::EdgeAgentUplink => 200.0,
            PrincipalClass::Plugin => 10.0,
        }
    }

    /// The registered per-tenant in-flight cap for the command-submission
    /// POST class. REGISTERED RECOMMENDATION (X-8), pending operator
    /// sign-off. The edge-agent cap is the LARGEST by design: a fleet of
    /// agents legitimately holds more concurrent submissions than a
    /// human operator session.
    pub const fn max_in_flight_command_submissions(self) -> u64 {
        match self {
            PrincipalClass::Operator => 8,
            PrincipalClass::EdgeAgentUplink => 64,
            PrincipalClass::Plugin => 2,
        }
    }
}

/// Classifies a caller subject into its principal class. The SINGLE
/// classification point (amend here when real subject formats land — the
/// recorded ASSUMPTION in the module header).
pub fn classify_principal(subject: &str) -> PrincipalClass {
    if subject.starts_with("edge-agent:") || subject.starts_with("edge:") {
        PrincipalClass::EdgeAgentUplink
    } else if subject.starts_with("plugin:") {
        PrincipalClass::Plugin
    } else {
        PrincipalClass::Operator
    }
}

/// One token bucket: refill rate + burst capacity, per key (the P11.02
/// glossary definition).
#[derive(Debug)]
struct TokenBucket {
    tokens: f64,
    last_refill: Instant,
    class: PrincipalClass,
}

impl TokenBucket {
    fn new(class: PrincipalClass) -> Self {
        Self {
            tokens: class.burst(),
            last_refill: Instant::now(),
            class,
        }
    }

    /// Refills elapsed tokens (capped at the burst) and consumes one.
    /// Returns `Ok(())` on success or `Err(retry_after_seconds)` — the
    /// ceil of the wait for one token, minimum 1 (the 429 contract).
    fn try_consume(&mut self) -> Result<(), u64> {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        self.tokens = (self.tokens + elapsed * self.class.refill_per_second())
            .min(self.class.burst());
        self.last_refill = now;
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            Ok(())
        } else {
            let deficit = 1.0 - self.tokens;
            let seconds = (deficit / self.class.refill_per_second()).ceil() as u64;
            Err(seconds.max(1))
        }
    }
}

/// The bounded bucket map: a bucket is created per (class, tenant) on
/// first sight and evicted oldest-first when the registered cap is hit —
/// never unbounded growth.
#[derive(Debug, Default)]
pub struct RateLimitState {
    buckets: Mutex<BTreeMap<(String, String), TokenBucket>>,
    in_flight: Mutex<BTreeMap<String, u64>>,
    rejected_total: AtomicU64,
    in_flight_rejected_total: AtomicU64,
}

/// The registered bucket-map cap (entries, per process). RECOMMENDATION
/// (X-8): with per-tenant-per-class buckets, this bounds the map for any
/// realistic tenant population; eviction is oldest-refill, never
/// unbounded.
const MAX_TRACKED_BUCKETS: usize = 10_000;

impl RateLimitState {
    fn try_consume(&self, class: PrincipalClass, tenant: &str) -> Result<(), u64> {
        let mut buckets = self.buckets.lock().expect("rate-limit mutex poisoned");
        let key = (class.as_str().to_string(), tenant.to_string());
        // FRV1-R2-006 (final review): the eviction check runs BEFORE the
        // insert, so the map never transiently holds MAX+1 entries and the
        // eviction scan can never drop the bucket the caller is about to
        // use. A key that already has a bucket inserts nothing and needs
        // no eviction. Post-insert the map holds at most
        // MAX_TRACKED_BUCKETS entries — the stated bound, not one past it.
        if !buckets.contains_key(&key) && buckets.len() >= MAX_TRACKED_BUCKETS {
            // Bounded map: evict the least-recently-refilled bucket. A
            // linear scan is acceptable at the registered cap.
            if let Some(oldest) = buckets
                .iter()
                .min_by_key(|(_, b)| b.last_refill)
                .map(|(k, _)| k.clone())
            {
                buckets.remove(&oldest);
            }
        }
        let bucket = buckets.entry(key).or_insert_with(|| TokenBucket::new(class));
        bucket.try_consume()
    }

    /// The in-flight gate for one command-submission request: `Ok(())`
    /// when under the per-tenant cap (the counter is incremented),
    /// `Err(())` when over it. The caller MUST hold an [`InFlightGuard`]
    /// (built from the same `Arc`) until the request completes — the
    /// guard decrements on drop, including on the panic-unwind path.
    fn enter_in_flight(&self, class: PrincipalClass, tenant: &str) -> Result<(), ()> {
        let mut in_flight = self.in_flight.lock().expect("rate-limit mutex poisoned");
        let current = in_flight.entry(tenant.to_string()).or_insert(0);
        if *current >= class.max_in_flight_command_submissions() {
            return Err(());
        }
        *current += 1;
        Ok(())
    }

    /// The rejection counters, rendered on `/metrics` under their own
    /// families (FRV1-R2-006: the counters were incremented but rendered
    /// nowhere — the P11.14 record §5 handoff note is closed by the
    /// additive P11.01 taxonomy amendment, families 6 and 7; the 429
    /// responses remain counted in the existing
    /// `flexcore_http_requests_total{status="429"}` series by the
    /// outermost correlation middleware).
    pub fn rejection_counters(&self) -> (u64, u64) {
        (
            self.rejected_total.load(Ordering::Relaxed),
            self.in_flight_rejected_total.load(Ordering::Relaxed),
        )
    }

    /// Renders the two rejection-counter families in the Prometheus text
    /// exposition format (appended to the P1.20 registry body by the
    /// `/metrics` handler). Always renders, zero values included (the
    /// always-render discipline of the P1.20/P11.01 registries); no
    /// labels (the recorded cardinality decision — per-class breakdowns
    /// travel in the correlation log line, never as label axes).
    pub fn render_rejection_metrics(&self) -> String {
        let (rate_limited, in_flight_rejected) = self.rejection_counters();
        let mut out = String::new();
        out.push_str("# HELP flexcore_rate_limit_rejections_total Requests rejected with 429 by the P11.14 token-bucket limiter (probes and /metrics are never rate limited).\n");
        out.push_str("# TYPE flexcore_rate_limit_rejections_total counter\n");
        out.push_str(&format!("flexcore_rate_limit_rejections_total {rate_limited}\n"));
        out.push_str("# HELP flexcore_rate_limit_in_flight_rejections_total Command-submission requests rejected with 429 by the per-tenant in-flight cap (the cap releases as requests complete).\n");
        out.push_str("# TYPE flexcore_rate_limit_in_flight_rejections_total counter\n");
        out.push_str(&format!(
            "flexcore_rate_limit_in_flight_rejections_total {in_flight_rejected}\n"
        ));
        out
    }
}

/// Decrements the per-tenant in-flight counter on drop (also on the
/// panic-unwind path — a Drop guard, not an early-return decrement). The
/// guard holds an `Arc` clone of the state, so its lifetime never borrows
/// the request's state reference (no unsafe anywhere in this module).
#[derive(Debug)]
struct InFlightGuard {
    state: Arc<RateLimitState>,
    tenant: String,
}

impl InFlightGuard {
    /// Builds the guard for a tenant whose counter was incremented by
    /// [`RateLimitState::enter_in_flight`].
    fn new(state: Arc<RateLimitState>, tenant: &str) -> Self {
        Self {
            state,
            tenant: tenant.to_string(),
        }
    }
}

impl Drop for InFlightGuard {
    fn drop(&mut self) {
        let mut in_flight = self.state.in_flight.lock().expect("rate-limit mutex poisoned");
        if let Some(current) = in_flight.get_mut(&self.tenant) {
            *current = current.saturating_sub(1);
        }
    }
}

/// Renders the 429 response: `Retry-After` (whole seconds) plus the stable
/// error envelope (`rate_limited`). The [S-85] contract shape.
fn rate_limited_response(retry_after_seconds: u64) -> Response {
    (
        (
            StatusCode::TOO_MANY_REQUESTS,
            [("Retry-After", retry_after_seconds.to_string())],
        ),
        Json(ErrorEnvelope {
            error: crate::error::ErrorBody {
                code: "rate_limited".to_string(),
                message: "the request exceeded the tenant's rate budget; retry after the \
                          Retry-After interval"
                    .to_string(),
            },
        }),
    )
        .into_response()
}

/// The P11.14 middleware. Mounted OUTER to the idempotency middleware
/// (a 429 must never be cached as a stored response — the P11.11 store
/// rule also skips 429, and the two agree) and INNER to the correlation
/// middleware (so 429s are recorded in the request/error families).
pub async fn rate_limit_middleware(
    State(state): State<AppState>,
    request: Request,
    next: axum::middleware::Next,
) -> Response {
    let path = request.uri().path().to_string();

    // Probes and the metrics scrape are never rate limited (recorded).
    if path == "/health" || path == "/ready" || path == "/metrics" {
        return next.run(request).await;
    }

    // Identity: best-effort resolution exactly like the correlation
    // middleware. No verified caller → pass through (the handler 401s;
    // budgets are per principal).
    let bearer = request
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    let Some(caller) = state.identity.resolve(bearer.as_deref()).ok() else {
        return next.run(request).await;
    };
    let tenant = caller.ctx.tenant_id().uuid().to_hyphenated();
    let class = classify_principal(caller.ctx.subject());

    // Token bucket.
    if let Err(retry_after) = state.rate_limit.try_consume(class, &tenant) {
        state
            .rate_limit
            .rejected_total
            .fetch_add(1, Ordering::Relaxed);
        return rate_limited_response(retry_after);
    }

    // Per-tenant in-flight cap on the command-submission POST class.
    let is_command_submission = request.method() == "POST"
        && crate::routes::COMMAND_SUBMISSION_PATHS.contains(&path.as_str());
    if is_command_submission {
        let entered = state.rate_limit.enter_in_flight(class, &tenant);
        if entered.is_err() {
            state
                .rate_limit
                .in_flight_rejected_total
                .fetch_add(1, Ordering::Relaxed);
            // Poll-style retry: the cap releases as requests complete.
            return rate_limited_response(1);
        }
        // The guard holds an Arc clone: its lifetime never borrows the
        // request's state reference, and drop decrements on every path
        // including panic unwind.
        let guard = InFlightGuard::new(state.rate_limit.clone(), &tenant);
        let response = next.run(request).await;
        drop(guard);
        return response;
    }

    next.run(request).await
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The class set is closed and the edge-agent class is DISTINCT from
    /// the operator class by construction (the starvation-prevention
    /// rule).
    #[test]
    fn principal_classes_are_closed_and_distinct() {
        assert_ne!(
            classify_principal("edge-agent:device-1"),
            classify_principal("operator-alice")
        );
        assert_eq!(classify_principal("edge:device-1"), PrincipalClass::EdgeAgentUplink);
        assert_eq!(classify_principal("plugin:invoker-1"), PrincipalClass::Plugin);
        assert_eq!(classify_principal("user:alice"), PrincipalClass::Operator);
        assert_eq!(classify_principal(""), PrincipalClass::Operator);
    }

    /// The edge-agent class budget is strictly larger than the operator
    /// budget on BOTH axes (registered; the fleet uplink is never starved
    /// by the interactive budget).
    #[test]
    fn the_edge_agent_budget_is_strictly_larger_than_the_operator_budget() {
        assert!(PrincipalClass::EdgeAgentUplink.refill_per_second() > PrincipalClass::Operator.refill_per_second());
        assert!(PrincipalClass::EdgeAgentUplink.burst() > PrincipalClass::Operator.burst());
        assert!(
            PrincipalClass::EdgeAgentUplink.max_in_flight_command_submissions()
                > PrincipalClass::Operator.max_in_flight_command_submissions()
        );
    }

    /// A fresh bucket accepts up to its burst, then 429s with a
    /// Retry-After of at least one second.
    #[test]
    fn bucket_burst_then_reject_with_retry_after() {
        let class = PrincipalClass::Plugin;
        let state = RateLimitState::default();
        for _ in 0..class.burst() as usize {
            assert_eq!(state.try_consume(class, "tenant-a"), Ok(()));
        }
        let rejected = state.try_consume(class, "tenant-a");
        assert!(rejected.is_err(), "an exhausted bucket must reject");
        assert!(rejected.unwrap_err() >= 1, "Retry-After is at least one second");
    }

    /// Buckets are per (class, tenant): one tenant's exhaustion never
    /// touches another tenant's, and never another class's.
    #[test]
    fn buckets_are_per_class_and_per_tenant() {
        let state = RateLimitState::default();
        for _ in 0..PrincipalClass::Plugin.burst() as usize {
            let _ = state.try_consume(PrincipalClass::Plugin, "tenant-a");
        }
        assert_eq!(
            state.try_consume(PrincipalClass::Plugin, "tenant-a").is_err(),
            true,
            "tenant-a's plugin bucket is exhausted"
        );
        assert_eq!(
            state.try_consume(PrincipalClass::Plugin, "tenant-b"),
            Ok(()),
            "tenant-b's plugin bucket is untouched"
        );
        assert_eq!(
            state.try_consume(PrincipalClass::Operator, "tenant-a"),
            Ok(()),
            "tenant-a's operator bucket is untouched"
        );
    }

    /// The in-flight gate admits up to the registered cap and rejects
    /// over it; the guard decrements on drop.
    #[test]
    fn in_flight_gate_admits_to_the_cap_then_rejects_and_releases_on_drop() {
        let state = Arc::new(RateLimitState::default());
        let class = PrincipalClass::Plugin;
        let mut guards = Vec::new();
        for _ in 0..class.max_in_flight_command_submissions() {
            state
                .enter_in_flight(class, "tenant-a")
                .expect("under cap");
            guards.push(InFlightGuard::new(state.clone(), "tenant-a"));
        }
        assert!(
            state.enter_in_flight(class, "tenant-a").is_err(),
            "over the cap must reject"
        );
        drop(guards);
        assert!(
            state.enter_in_flight(class, "tenant-a").is_ok(),
            "released on drop"
        );
    }

    /// FRV1-R2-006 (final review): the rejection counters are rendered
    /// under their own families on `/metrics` — every 429 increments the
    /// matching counter, and the rendered body exposes both families with
    /// the incremented values (the always-render, zero-included
    /// discipline).
    #[test]
    fn rejection_counters_render_under_their_own_families() {
        let state = RateLimitState::default();
        let class = PrincipalClass::Plugin;
        for _ in 0..class.burst() as usize {
            let _ = state.try_consume(class, "tenant-a");
        }
        let _ = state.try_consume(class, "tenant-a"); // 429 (bucket empty)
        let _ = state.enter_in_flight(class, "tenant-a");
        let _ = state.enter_in_flight(class, "tenant-a"); // over the cap
        let body = state.render_rejection_metrics();
        assert!(
            body.contains("flexcore_rate_limit_rejections_total 1"),
            "the token-bucket rejection must render: {body}"
        );
        assert!(
            body.contains("flexcore_rate_limit_in_flight_rejections_total 1"),
            "the in-flight-cap rejection must render: {body}"
        );
        let fresh = RateLimitState::default();
        let body = fresh.render_rejection_metrics();
        assert!(body.contains("flexcore_rate_limit_rejections_total 0"));
        assert!(body.contains("flexcore_rate_limit_in_flight_rejections_total 0"));
    }

    /// FRV1-R2-006 (final review): the eviction check runs BEFORE the
    /// insert, so the bucket map never exceeds the registered cap — the
    /// just-inserted bucket survives its own insertion.
    #[test]
    fn the_bucket_map_stays_bounded_across_eviction() {
        let state = RateLimitState::default();
        // Drive the map past the cap with distinct keys; every key still
        // gets a usable bucket (Ok or a bounded-map rejection — never a
        // panic), and the map can never hold more than the cap.
        for i in 0..(MAX_TRACKED_BUCKETS + 50) {
            let _ = state.try_consume(PrincipalClass::Plugin, &format!("tenant-{i}"));
        }
        let buckets = state.buckets.lock().expect("rate-limit mutex poisoned");
        assert!(
            buckets.len() <= MAX_TRACKED_BUCKETS,
            "the map must never exceed the registered cap (held {})",
            buckets.len()
        );
        // The most recent key was inserted after the last eviction and is
        // still present.
        assert!(buckets.contains_key(&(
            PrincipalClass::Plugin.as_str().to_string(),
            format!("tenant-{}", MAX_TRACKED_BUCKETS + 49)
        )));
    }
}
