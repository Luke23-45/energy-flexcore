// P5.25 — Live-update API contract tests (§15.1, §21.3; ledger P5.25
//         acceptance): the `GET /api/v1/live` SSE stream over the
//         Test-variant backend.
//
// Composition rule (the P1.17/P4.30 test pattern — no network, no
// database): the app state is built with the flexcore-ports fakes plus the
// in-crate Phase 3/4 record-store fakes, a fixed identity resolver, and
// the cumulative Phase 4 policy set (whose `telemetry.read` grants are the
// interim live-stream gate — see the recorded drift on
// `auth::authorize_live_stream`). Events reach the stream through the
// in-process `LiveEventBus` — the exact seam the composition root's NATS
// forwarder feeds in production — so the tests exercise the real decode →
// tenant-filter → SSE path, not a mock.
//
// Recorded assertions covered here (the ledger P5.25 acceptance list):
// - **Authz**: unauthenticated is 401; an empty policy set denies with 403
//   (deny-by-default); the viewer role can subscribe (tenant-scoped, all
//   roles).
// - **SSE framing**: a 200 subscription carries the `text/event-stream`
//   content type.
// - **Tenant scoping with no cross-tenant leakage**: a tenant's stream
//   carries exactly that tenant's events and never another tenant's
//   (scoping at consumption from the decoded payload, D-J2); a payload
//   that does not decode into the frozen §15.3 vocabulary is never
//   forwarded to ANY stream (fail closed).
// - **Correlation (§21.3)**: forwarded events carry the §15.3
//   correlation_id — the same id the audit record carries — plus the
//   tenant/site/asset scope.
// - **At-least-once honesty**: the envelope carries the duplicate/reorder
//   statement; a lagged stream emits RESYNC_REQUIRED (no silent
//   gap-filling) and keeps streaming.
// - **No secrets in the stream (P5.25 step 5)**: extra fields on a payload
//   (here: a planted `authorization` bearer value) never reach the stream
//   — the envelope is extracted from the frozen event schema, never a
//   pass-through of the raw payload.
// - **OpenAPI**: the spec registers `/api/v1/live` with GET only.
//
// UNRUN — the human executes this file with the first `cargo test` (the
// Phase 5 loop bans builds and test runs; recorded in the decision record
// `ledger/phase-05-operator-console-simulator/01_25_scaffold_live.md`).

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::response::Response;
use axum::Router;
use flexcore_authz::policy::{PolicySet, Role};
use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::quality::TelemetryQuality;
use flexcore_contracts::time_model::EventTime;
use flexcore_control_plane::auth::{AuthError, Caller, IdentityResolver, phase4_default_policies};
use flexcore_control_plane::probes::Readiness;
use flexcore_control_plane::routes::stage1_router;
use flexcore_control_plane::routes_live::LiveEventBus;
use flexcore_control_plane::state::AppState;
use flexcore_domain::ids::{AssetId, SiteId, TenantId};
use flexcore_domain::tenant_context::{AuthenticatedClaims, TenantContext, VerifiedClaims};
use flexcore_events::{DomainEvent, TelemetryQualityChangedPayload, TelemetryReceivedPayload};
use flexcore_ports::fakes::{
    CountingIdGenerator, FakeAssetRepository, FakeEventOutbox, FakePortfolioRepository,
    FakeSiteRepository, FakeTenantRepository, FakeWriteJournal, FixedClock,
};
use serde_json::{Value, json};
use tower::ServiceExt;

const TENANT_A_SEED: u128 = 1;
const TENANT_B_SEED: u128 = 2;
const CORRELATION_SEED: u128 = 42;
const CORRELATION_B_SEED: u128 = 44;

fn flex_uuid(seed: u128) -> FlexUuid {
    FlexUuid::from_bytes(seed.to_be_bytes())
}

fn tenant_id(seed: u128) -> TenantId {
    TenantId::new(flex_uuid(seed))
}

fn tenant_str(seed: u128) -> String {
    flex_uuid(seed).to_string()
}

const TELEMETRY_SUBJECT: &str = "flexcore.v1.telemetry.telemetry_received";
const QUALITY_SUBJECT: &str = "flexcore.v1.telemetry.telemetry_quality_changed";

// ---------------------------------------------------------------------------
// Identity + composition (the P4.30 test pattern, extended with the live
// bus: the tests publish into the SAME bus instance the handlers subscribe)
// ---------------------------------------------------------------------------

struct FixedIdentityResolver {
    callers: Vec<(String, Caller)>,
}

impl IdentityResolver for FixedIdentityResolver {
    fn resolve(&self, bearer: Option<&str>) -> Result<Caller, AuthError> {
        let raw = bearer.ok_or(AuthError::MissingCredentials)?;
        let token = raw
            .strip_prefix("Bearer ")
            .ok_or(AuthError::InvalidCredentials)?;
        self.callers
            .iter()
            .find(|(t, _)| t == token)
            .map(|(_, c)| c.clone())
            .ok_or(AuthError::InvalidCredentials)
    }
}

fn caller_for(seed: u128, subject: &str, role: Role) -> Caller {
    let claims = AuthenticatedClaims::from_verified(VerifiedClaims {
        tenant_id: tenant_id(seed),
        subject: subject.to_string(),
    });
    Caller {
        ctx: TenantContext::from_authenticated_claims(claims),
        role,
    }
}

fn passing_readiness() -> Readiness {
    let check: flexcore_control_plane::probes::HealthCheck = Arc::new(|| {
        Box::pin(async { Ok::<(), String>(()) })
            as std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), String>> + Send>>
    });
    Readiness::new(vec![("test-dependency".to_string(), check)])
}

/// The test-owned bus handle (`Arc<LiveEventBus>`, the exact type
/// `AppState::live` holds). `close(&self)` is interior-mutability, so the
/// test publishes, closes, and reads the drained streams deterministically.
type TestBus = Arc<LiveEventBus>;

/// Builds a router whose state carries a TEST-OWNED live bus: the state's
/// auto-created bus is replaced (functional struct update — all fields are
/// public) with the test's Arc, so publish/close through the test handle
/// reach the SSE handlers exactly as the composition root's NATS forwarder
/// does in production. This is the real decode → tenant-filter → SSE path.
fn stream_fixture_with_policies(policies: PolicySet) -> (TestBus, Router) {
    let bus: TestBus = Arc::new(LiveEventBus::new());
    let journal = FakeWriteJournal::new();
    let outbox = Arc::new(FakeEventOutbox::new(journal.clone()));
    let resolver = FixedIdentityResolver {
        callers: vec![
            (
                "caller-a".to_string(),
                caller_for(TENANT_A_SEED, "user:admin-a", Role::TenantAdmin),
            ),
            (
                "caller-b".to_string(),
                caller_for(TENANT_B_SEED, "user:admin-b", Role::TenantAdmin),
            ),
            (
                "viewer-a".to_string(),
                caller_for(TENANT_A_SEED, "user:viewer", Role::Viewer),
            ),
        ],
    };
    let state = AppState::for_tests_with_phase4(
        Arc::new(FakeTenantRepository::new(journal.clone())),
        Arc::new(FakePortfolioRepository::new(journal.clone())),
        Arc::new(FakeSiteRepository::new(journal.clone())),
        Arc::new(FakeAssetRepository::new(journal.clone())),
        outbox,
        Arc::new(CountingIdGenerator::new()),
        Arc::new(FixedClock::at_unix_millis(1_700_000_000_000)),
        policies,
        Arc::new(resolver),
        passing_readiness(),
        None,
        300_000,
        Arc::new(flexcore_control_plane::phase3_fakes::FakeScheduleRecordStore::new()),
        Arc::new(flexcore_control_plane::phase3_fakes::FakeScheduleRevisionStore::new()),
        Arc::new(flexcore_control_plane::phase3_fakes::FakeEnvelopeRecordStore::new()),
        Arc::new(flexcore_control_plane::phase4_fakes::FakeCommandRecordStore::new()),
        Arc::new(flexcore_control_plane::phase4_fakes::FakeFeasibilityCheckStore::new()),
    );
    let state = AppState {
        live: bus.clone(),
        ..state
    };
    (bus, stage1_router(state))
}

/// The default fixture: the cumulative Phase 4 policy set (the interim
/// live-stream grants).
fn stream_fixture() -> (TestBus, Router) {
    stream_fixture_with_policies(phase4_default_policies())
}

/// Builds a frozen `TelemetryReceived` payload as it travels on the wire.
fn event_payload(seed: u128, tenant_seed: u128) -> Vec<u8> {
    let payload = TelemetryReceivedPayload {
        correlation_id: flex_uuid(seed),
        tenant_id: tenant_id(tenant_seed),
        site_id: SiteId::new(flex_uuid(0xa1)),
        asset_id: AssetId::new(flex_uuid(0xa2)),
        event_time: EventTime::from_unix_millis(1_700_000_000_000)
            .expect("valid timestamp"),
    };
    serde_json::to_vec(&DomainEvent::TelemetryReceived(payload)).expect("serializes")
}

/// Builds a frozen `TelemetryQualityChanged` payload (the P2.16 shape with
/// both quality states).
fn quality_event_payload(tenant_seed: u128) -> Vec<u8> {
    let payload = TelemetryQualityChangedPayload {
        correlation_id: flex_uuid(CORRELATION_B_SEED),
        tenant_id: tenant_id(tenant_seed),
        site_id: SiteId::new(flex_uuid(0xa1)),
        asset_id: AssetId::new(flex_uuid(0xa2)),
        quality: TelemetryQuality::Stale,
        previous_quality: Some(TelemetryQuality::Good),
        event_time: EventTime::from_unix_millis(1_700_000_000_000)
            .expect("valid timestamp"),
    };
    serde_json::to_vec(&DomainEvent::TelemetryQualityChanged(payload)).expect("serializes")
}

// ---------------------------------------------------------------------------
// HTTP helpers (the P4.30 pattern + the SSE reader)
// ---------------------------------------------------------------------------

async fn call(router: &Router, request: Request<Body>) -> (StatusCode, Value) {
    let response = router
        .clone()
        .oneshot(request)
        .await
        .expect("oneshot is infallible for this router");
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("body reads cleanly");
    let body = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).expect("error responses are JSON")
    };
    (status, body)
}

fn get(uri: &str, token: Option<&str>) -> Request<Body> {
    let mut builder = Request::builder().method("GET").uri(uri);
    if let Some(t) = token {
        builder = builder.header("authorization", format!("Bearer {t}"));
    }
    builder.body(Body::empty()).expect("valid GET request")
}

/// Opens one SSE subscription and returns the raw response (headers are
/// readable immediately; the BODY must only be read after the bus closed —
/// `read_stream` below).
async fn open_stream(router: &Router, token: &str) -> Response {
    router
        .clone()
        .oneshot(get("/api/v1/live", Some(token)))
        .await
        .expect("oneshot is infallible for this router")
}

/// Reads a TERMINATED SSE response as (status, content-type, body text).
/// The stream must have been ended (the bus closed) so the body read
/// completes; tokio broadcast delivers the buffered events before the
/// end-of-stream, so nothing published before the close is lost.
async fn read_stream(response: Response) -> (StatusCode, String, String) {
    let status = response.status();
    let content_type = response
        .headers()
        .get(axum::http::header::CONTENT_TYPE)
        .map(|v| v.to_str().expect("ascii header").to_string())
        .unwrap_or_default();
    let bytes = axum::body::to_bytes(response.into_body(), 16 * 1024 * 1024)
        .await
        .expect("the terminated SSE body reads cleanly");
    (
        status,
        content_type,
        String::from_utf8_lossy(&bytes).to_string(),
    )
}

async fn openapi_spec(router: &Router) -> Value {
    let (status, body) = call(router, get("/api/v1/openapi.json", None)).await;
    assert_eq!(status, StatusCode::OK, "the OpenAPI spec is reachable");
    body
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Unauthenticated: 401 before any stream exists.
#[tokio::test]
async fn unauthenticated_request_is_rejected() {
    let (router, _bus) = composed_router_with_state_bus();
    let (status, body) = call(&router, get("/api/v1/live", None)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"]["code"], "unauthenticated");
}

/// Deny-by-default: an empty policy set denies the well-formed, in-tenant
/// request with 403 (the interim `telemetry.read` gate is still an
/// explicit-grant surface — nothing matches by default).
#[tokio::test]
async fn empty_policy_set_denies_with_403() {
    let (router, _bus) = stream_fixture_with_policies(PolicySet::new());
    let (status, body) = call(&router, get("/api/v1/live", Some("caller-a"))).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["error"]["code"], "forbidden");
}

/// A 200 subscription carries the SSE content type; the viewer role (the
/// least-privileged role) can subscribe — tenant-scoped, all roles.
#[tokio::test]
async fn sse_content_type_and_viewer_role() {
    let (_bus, router) = stream_fixture();
    let response = open_stream(&router, "viewer-a").await;
    assert_eq!(response.status(), StatusCode::OK);
    let content_type = response
        .headers()
        .get(axum::http::header::CONTENT_TYPE)
        .expect("SSE declares its content type")
        .to_str()
        .expect("ascii")
        .to_string();
    assert!(
        content_type.starts_with("text/event-stream"),
        "expected text/event-stream, got {content_type}"
    );
}

/// Tenant scoping with no existence leak (the ledger P5.25 acceptance):
/// tenant A's stream carries exactly tenant A's events — with full §21.3
/// correlation and the at-least-once honesty statement — and NEVER tenant
/// B's events; tenant B's stream mirrors it.
#[tokio::test]
async fn stream_is_tenant_scoped_without_cross_tenant_leakage() {
    let (bus, router) = stream_fixture();
    let stream_a = open_stream(&router, "caller-a").await;
    let stream_b = open_stream(&router, "caller-b").await;
    // Publish one event per tenant, then close: both streams drain their
    // buffered events and end (deterministic termination).
    bus.publish(
        TELEMETRY_SUBJECT.to_string(),
        event_payload(CORRELATION_SEED, TENANT_A_SEED),
    );
    bus.publish(
        TELEMETRY_SUBJECT.to_string(),
        event_payload(CORRELATION_B_SEED, TENANT_B_SEED),
    );
    bus.close();

    let (status_a, _, body_a) = read_stream(stream_a).await;
    assert_eq!(status_a, StatusCode::OK);
    let (status_b, _, body_b) = read_stream(stream_b).await;
    assert_eq!(status_b, StatusCode::OK);

    // Tenant A sees exactly its own event, with the §21.3 correlation id
    // (the same id the audit record carries).
    assert!(body_a.contains("event: TELEMETRY_RECEIVED"), "{body_a}");
    assert!(body_a.contains(&format!(
        "\"correlation_id\":\"{}\"",
        flex_uuid(CORRELATION_SEED)
    )));
    assert!(body_a.contains(&format!("\"tenant_id\":\"{}\"", tenant_str(TENANT_A_SEED))));
    assert!(body_a.contains("\"delivery\":\"at-least-once\""));
    assert!(
        body_a.contains("arrival order as occurrence order"),
        "the ordering honesty statement travels with every event"
    );
    assert!(
        body_a.contains("\"event_time\":"),
        "the event time travels (distinct from the delivered_at anchor)"
    );
    assert!(body_a.contains("\"delivered_at\":"));
    // …and NEVER tenant B's event: no cross-tenant stream leakage.
    assert!(!body_a.contains(&tenant_str(TENANT_B_SEED)));
    assert!(!body_a.contains(&format!("\"correlation_id\":\"{}\"", flex_uuid(CORRELATION_B_SEED))));

    // Tenant B mirrors: its own event, never tenant A's.
    assert!(body_b.contains("event: TELEMETRY_RECEIVED"));
    assert!(body_b.contains(&format!(
        "\"correlation_id\":\"{}\"",
        flex_uuid(CORRELATION_B_SEED)
    )));
    assert!(!body_b.contains(&tenant_str(TENANT_A_SEED)));
}

/// A payload that does not decode into the frozen §15.3 vocabulary is
/// never forwarded to any stream (fail closed), and a decoded event's
/// envelope is extracted from the frozen schema — a planted secret field
/// on the payload never reaches the stream (P5.25 step 5).
#[tokio::test]
async fn undecodable_or_secret_bearing_payloads_never_reach_the_stream() {
    let (bus, router) = stream_fixture();
    let stream = open_stream(&router, "caller-a").await;
    // 1. Junk that is not JSON at all.
    bus.publish(TELEMETRY_SUBJECT.to_string(), b"{not json".to_vec());
    // 2. An invented event type (the closed vocabulary rejects it).
    bus.publish(
        TELEMETRY_SUBJECT.to_string(),
        br#"{"event_type":"NOT_A_REAL_EVENT","tenant_id":"00000000-0000-0000-0000-000000000001"}"#
            .to_vec(),
    );
    // 3. A decodable frozen event that carries a planted extra field (a
    //    secret-looking bearer value): the envelope is extracted from the
    //    frozen schema, so the extra field must not pass through.
    let mut payload = serde_json::from_slice::<Value>(&event_payload(
        CORRELATION_SEED,
        TENANT_A_SEED,
    ))
    .expect("a frozen event is JSON");
    payload["authorization"] = json!("Bearer supersecret-dev-value");
    bus.publish(
        TELEMETRY_SUBJECT.to_string(),
        serde_json::to_vec(&payload).expect("serializes"),
    );
    bus.close();

    let (status, _, body) = read_stream(stream).await;
    assert_eq!(status, StatusCode::OK);
    // The decodable event passed (with its correlation id intact)…
    assert!(body.contains(&format!(
        "\"correlation_id\":\"{}\"",
        flex_uuid(CORRELATION_SEED)
    )));
    // …the junk and the invented tag did not…
    assert!(!body.contains("NOT_A_REAL_EVENT"));
    // …and the planted secret never reached the stream.
    assert!(
        !body.contains("supersecret"),
        "no secret may travel in the stream: {body}"
    );
    assert!(!body.contains("authorization"));
}

/// The quality event decodes with its frozen names (the console maps these
/// tags to the mirrored TS types, §15.3).
#[tokio::test]
async fn quality_events_forward_with_frozen_names() {
    let (bus, router) = stream_fixture();
    let stream = open_stream(&router, "caller-a").await;
    bus.publish(
        QUALITY_SUBJECT.to_string(),
        quality_event_payload(TENANT_A_SEED),
    );
    bus.close();
    let (status, _, body) = read_stream(stream).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("event: TELEMETRY_QUALITY_CHANGED"), "{body}");
    assert!(body.contains("\"event_name\":\"TelemetryQualityChanged\""));
    assert!(body.contains("\"quality\":\"STALE\""));
    assert!(body.contains("\"previous_quality\":\"GOOD\""));
}

/// A lagged stream emits RESYNC_REQUIRED — the at-least-once honesty rule:
/// no silent gap-filling; the client re-syncs explicitly from the REST
/// read APIs (§15.1, P5.25 step 2; the full ordering/dedup discipline is
/// P5.33 and deliberately NOT here).
#[tokio::test]
async fn a_lagged_stream_emits_resync_required_and_keeps_streaming() {
    let (bus, router) = stream_fixture();
    let stream = open_stream(&router, "caller-a").await;
    // Far more events than the per-subscriber buffer (1024): the receiver
    // lags and the stream must surface the skip instead of silently
    // passing over it.
    for index in 0..2048u128 {
        bus.publish(
            TELEMETRY_SUBJECT.to_string(),
            event_payload(1000 + index, TENANT_A_SEED),
        );
    }
    bus.close();
    let (status, _, body) = read_stream(stream).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body.contains("event: RESYNC_REQUIRED"),
        "a lag must surface, never silently pass over: {body}"
    );
    assert!(body.contains("re-sync from the REST read APIs"));
    // The stream continued: normal data events still flowed after the lag.
    assert!(body.contains("event: TELEMETRY_RECEIVED"));
}

/// The generated OpenAPI registers /api/v1/live with GET only (a READ
/// surface: no POST/PUT/PATCH/DELETE exists anywhere on it, §15.2).
#[tokio::test]
async fn openapi_registers_the_live_route_get_only() {
    let (router, _bus) = composed_router_with_state_bus();
    let spec = openapi_spec(&router).await;
    let live = &spec["paths"]["/api/v1/live"];
    assert!(
        live.is_object(),
        "the /api/v1/live path must be registered: {}",
        spec["paths"]
    );
    assert!(
        live["get"].is_object(),
        "GET /api/v1/live must be documented"
    );
    for method in ["post", "put", "patch", "delete"] {
        assert!(
            live[method].is_null(),
            "/api/v1/live must not offer {method} (READ-only surface)"
        );
    }
    assert!(
        spec["components"]["schemas"]["LiveEventEnvelope"].is_object(),
        "the SSE envelope schema is registered"
    );
    assert!(
        spec["components"]["schemas"]["LiveResyncNotice"].is_object(),
        "the resync-notice schema is registered"
    );
}

/// A router composed the plain way (the state's OWN bus, never published
/// to): the header/status-only tests use this.
fn composed_router_with_state_bus() -> Router {
    let journal = FakeWriteJournal::new();
    let outbox = Arc::new(FakeEventOutbox::new(journal.clone()));
    let resolver = FixedIdentityResolver {
        callers: vec![(
            "caller-a".to_string(),
            caller_for(TENANT_A_SEED, "user:admin-a", Role::TenantAdmin),
        )],
    };
    let state = AppState::for_tests_with_phase4(
        Arc::new(FakeTenantRepository::new(journal.clone())),
        Arc::new(FakePortfolioRepository::new(journal.clone())),
        Arc::new(FakeSiteRepository::new(journal.clone())),
        Arc::new(FakeAssetRepository::new(journal.clone())),
        outbox,
        Arc::new(CountingIdGenerator::new()),
        Arc::new(FixedClock::at_unix_millis(1_700_000_000_000)),
        phase4_default_policies(),
        Arc::new(resolver),
        passing_readiness(),
        None,
        300_000,
        Arc::new(flexcore_control_plane::phase3_fakes::FakeScheduleRecordStore::new()),
        Arc::new(flexcore_control_plane::phase3_fakes::FakeScheduleRevisionStore::new()),
        Arc::new(flexcore_control_plane::phase3_fakes::FakeEnvelopeRecordStore::new()),
        Arc::new(flexcore_control_plane::phase4_fakes::FakeCommandRecordStore::new()),
        Arc::new(flexcore_control_plane::phase4_fakes::FakeFeasibilityCheckStore::new()),
    );
    stage1_router(state)
}
