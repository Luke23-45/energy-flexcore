// P5.31 — API contract tests for the `/api/v1/alarms` READ resource
//         (§20.1 item 1, §16.2; ledger P5.31): the derived-on-read alarm
//         derivation over the Test-variant backend.
//
// Composition rule (no network, no database — the P1.17 test pattern):
// the app state is built with the flexcore-ports fakes plus the in-crate
// Phase 4 record-store fakes, the telemetry read fake, a fixed identity
// resolver, and the cumulative Phase 5 policy set. Source records are
// seeded DIRECTLY into the fakes (there is NO alarm write route — §15.2
// discipline), and requests go through the real router via
// `tower::ServiceExt::oneshot`.
//
// UNRUN — written, never executed in this loop (the Phase 5 entry-note
// bans: nothing built, nothing run). The human executes `cargo test
// -p flexcore-control-plane --test api_alarms` with the first build.
//
// Recorded assertions covered here:
// - **Derivation contract**: a FAILED command, a LIMIT_VIOLATED check, and
//   a BAD telemetry observation each yield exactly their recorded alarm
//   code, severity, deterministic id, event-time `raised_at`, and source
//   reference; GOOD data yields NO alarm (no fabricated records).
// - **Honesty fields**: every response carries the alarm-model statement
//   verbatim, the model version, the derivation instant, the consulted
//   source counts, and the ABSENT sources by name (no invented sources).
// - **Telemetry-source honesty**: with the telemetry port wired the
//   telemetry alarms derive; with it `None` (the recorded Postgres wiring
//   gap) the source is named unavailable and NO telemetry alarm appears.
// - **Least-privilege**: `alarm.read` denies with 403 on an empty policy
//   set; unauthenticated is 401; every role in the Phase 5 default set may
//   read (viewer included).
// - **Tenant scoping with no existence leak**: another tenant's records
//   never produce alarms on this tenant's derivation.
// - **Filter discipline**: malformed severity/code/uuid filters are 422;
//   an inverted time window is 422; `active_only=false` is refused with
//   the honest derivation-contract message.
// - **READ-ONLY surface**: no alarm mutation route exists in the OpenAPI
//   document (GET /api/v1/alarms only) and a live POST probe is refused.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use flexcore_authz::policy::{PolicySet, Role};
use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::quality::TelemetryQuality;
use flexcore_contracts::time_model::{ClockQuality, EventTime, IngestTime};
use flexcore_control_plane::auth::{AuthError, Caller, IdentityResolver, phase5_default_policies};
use flexcore_control_plane::phase4_fakes::{FakeCommandRecordStore, FakeFeasibilityCheckStore};
use flexcore_control_plane::probes::Readiness;
use flexcore_control_plane::routes::stage1_router;
use flexcore_control_plane::state::AppState;
use flexcore_domain::entities::{Asset, AssetType, Portfolio, Site, Tenant};
use flexcore_domain::ids::{AssetId, PortfolioId, SiteId, TenantId};
use flexcore_domain::tenant_context::{AuthenticatedClaims, TenantContext, VerifiedClaims};
use flexcore_ports::fakes::{
    FakeAssetRepository, FakePortfolioRepository, FakeSiteRepository, FakeTelemetryReader,
    FakeTenantRepository, FixedClock,
};
use flexcore_ports::telemetry_source::{
    NormalizedTelemetry, NormalizedTelemetryParts, QuantityValue, TelemetryQuantity,
};
use flexcore_storage::command_store::{CommandRecordRow, CommandRecordStore, CommandTransitionRecord};
use flexcore_storage::feasibility_check::{FeasibilityCheckRow, FeasibilityCheckStore};
use serde_json::{Value, json};
use tower::ServiceExt;

const TENANT_A_SEED: u128 = 1;
const TENANT_B_SEED: u128 = 2;

fn tenant_id(seed: u128) -> TenantId {
    TenantId::new(FlexUuid::from_bytes(seed.to_be_bytes()))
}

// Fixed canonical UUIDs for the seeded source records.
const PORTFOLIO_A: &str = "0192a5e0-0000-7000-8000-0000000000a1";
const SITE_A: &str = "0192a5e0-0000-7000-8000-0000000000a2";
const ASSET_A: &str = "0192a5e0-0000-7000-8000-0000000000a3";
const COMMAND_A: &str = "0192a5e0-0000-7000-8000-000000000001";
const COMMAND_B: &str = "0192a5e0-0000-7000-8000-000000000002";
const CHECK_A: &str = "0192a5e0-0000-7000-8000-00000000000c1";
const COMMAND_B_TENANT: &str = "0192a5e0-0000-7000-8000-0000000000b1";

// ---------------------------------------------------------------------------
// Identity + composition (the api_phase4.rs pattern, plus the telemetry
// reader and the registry fakes the alarm derivation consults)
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

#[allow(clippy::type_complexity)]
fn composed_alarm_router(
    policies: PolicySet,
    telemetry: Option<Arc<FakeTelemetryReader>>,
) -> (
    Router,
    Arc<FakeCommandRecordStore>,
    Arc<FakeFeasibilityCheckStore>,
    Arc<FakeAssetRepository>,
) {
    let command_records = Arc::new(FakeCommandRecordStore::new());
    let feasibility_checks = Arc::new(FakeFeasibilityCheckStore::new());
    let journal = flexcore_ports::fakes::FakeWriteJournal::new();
    let assets = Arc::new(FakeAssetRepository::new(journal.clone()));
    let resolver = FixedIdentityResolver {
        callers: vec![
            (
                "admin-a".to_string(),
                caller_for(TENANT_A_SEED, "user:admin-a", Role::TenantAdmin),
            ),
            (
                "viewer-a".to_string(),
                caller_for(TENANT_A_SEED, "user:viewer", Role::Viewer),
            ),
            (
                "admin-b".to_string(),
                caller_for(TENANT_B_SEED, "user:admin-b", Role::TenantAdmin),
            ),
        ],
    };
    let state = AppState::for_tests_with_phase4_audit(
        Arc::new(FakeTenantRepository::new(journal.clone())),
        Arc::new(FakePortfolioRepository::new(journal.clone())),
        Arc::new(FakeSiteRepository::new(journal.clone())),
        assets.clone(),
        Arc::new(flexcore_ports::fakes::FakeEventOutbox::new(journal.clone())),
        Arc::new(flexcore_ports::fakes::CountingIdGenerator::new()),
        Arc::new(FixedClock::at_unix_millis(1_700_000_010_000)),
        policies,
        Arc::new(resolver),
        passing_readiness(),
        telemetry.map(|t| t as Arc<dyn flexcore_ports::telemetry_read::TelemetryReader>),
        300_000,
        Arc::new(flexcore_control_plane::phase3_fakes::FakeScheduleRecordStore::new()),
        Arc::new(flexcore_control_plane::phase3_fakes::FakeScheduleRevisionStore::new()),
        Arc::new(flexcore_control_plane::phase3_fakes::FakeEnvelopeRecordStore::new()),
        command_records.clone(),
        feasibility_checks.clone(),
        Arc::new(flexcore_control_plane::phase4_fakes::FakeAuditEventReader::new()),
    );
    (stage1_router(state), command_records, feasibility_checks, assets)
}

// ---------------------------------------------------------------------------
// HTTP helpers
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
        serde_json::from_slice(&bytes).expect("the API always emits JSON")
    };
    (status, body)
}

fn get(uri: &str, token: Option<&str>) -> Request<Body> {
    let mut builder = Request::builder().method("GET").uri(uri);
    if let Some(token) = token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    builder.body(Body::empty()).expect("request builds")
}

fn post(uri: &str, token: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("authorization", format!("Bearer {token}"))
        .body(Body::empty())
        .expect("request builds")
}

// ---------------------------------------------------------------------------
// Source-record seeding (directly into the fakes; there is NO alarm write
// route and no command-write route on this surface)
// ---------------------------------------------------------------------------

const EVENT_MS: i64 = 1_700_000_000_000;

fn command_row(command_id: &str, tenant: TenantId, site: &str, asset: &str) -> CommandRecordRow {
    CommandRecordRow::new(
        tenant,
        command_id.to_string(),
        PORTFOLIO_A.to_string(),
        site.to_string(),
        asset.to_string(),
        "0192a5e0-0000-7000-8000-0000000000a4".to_string(),
        1,
        EVENT_MS + 60_000,
        None,
        r#"{"command":"fixture"}"#.to_string(),
    )
    .expect("valid fixture command row")
}

fn transition(command_id: &str, tenant: TenantId, from: &str, to: &str, at: i64) -> CommandTransitionRecord {
    CommandTransitionRecord {
        tenant_id: tenant,
        command_id: command_id.to_string(),
        from_state: from.to_string(),
        to_state: to.to_string(),
        actor: "test-gate".to_string(),
        at_unix_millis: at,
        gate: Some("feasibility".to_string()),
        reason: Some("fixture reason".to_string()),
        evidence_reference: Some(format!("check:{CHECK_A}")),
    }
}

fn feasibility_row(check_id: &str, tenant: TenantId, outcome: &str) -> FeasibilityCheckRow {
    FeasibilityCheckRow::new(
        tenant,
        check_id.to_string(),
        Some(COMMAND_A.to_string()),
        PORTFOLIO_A.to_string(),
        SITE_A.to_string(),
        ASSET_A.to_string(),
        "0192a5e0-0000-7000-8000-0000000000a4".to_string(),
        1,
        Some("model-a".to_string()),
        Some("v1".to_string()),
        Some("hash-a".to_string()),
        outcome.to_string(),
        if outcome == "LIMIT_VIOLATED" {
            Some("transformer_rating".to_string())
        } else {
            None
        },
        if outcome == "FAIL_CLOSED" {
            Some("adapter unavailable".to_string())
        } else {
            None
        },
        None,
        None,
        "test-gate".to_string(),
        "corr-1".to_string(),
        EVENT_MS + 5_000,
    )
    .expect("valid fixture feasibility row")
}

/// Registers one real tenant → portfolio → site → asset chain in the
/// registry fakes (the alarm derivation reads assets through the same
/// repository ports the routes use).
fn seed_registry(ctx: &TenantContext, assets: &FakeAssetRepository) {
    let created = EventTime::from_unix_millis(EVENT_MS).expect("valid fixture time");
    let tenant = Tenant::try_new(
        tenant_id(TENANT_A_SEED),
        "tenant-a".to_string(),
        None,
        "idp://tenant-a".to_string(),
        "default".to_string(),
        created,
    )
    .expect("valid fixture tenant");
    let portfolio = Portfolio::try_new(
        &tenant,
        PortfolioId::new(FlexUuid::parse(PORTFOLIO_A).expect("valid uuid")),
        "portfolio-a".to_string(),
        None,
        "operator-a".to_string(),
        "SYNTHETIC_REGION".to_string(),
        "fixture aggregation".to_string(),
        created,
    )
    .expect("valid fixture portfolio");
    let site = Site::try_new(
        &portfolio,
        SiteId::new(FlexUuid::parse(SITE_A).expect("valid uuid")),
        "site-a".to_string(),
        None,
        "lab".to_string(),
        flexcore_contracts::time_model::SiteTimezone::new("UTC".to_string())
            .expect("valid fixture timezone"),
        "pcc-a".to_string(),
        "grid-model-a".to_string(),
        "sim:edge-a".to_string(),
        created,
    )
    .expect("valid fixture site");
    let asset = Asset::try_new(
        &site,
        AssetId::new(FlexUuid::parse(ASSET_A).expect("valid uuid")),
        "asset-a".to_string(),
        None,
        AssetType::Bess,
        "vendor-a".to_string(),
        flexcore_contracts::units::ActivePowerMW::new(2.0).expect("valid MW"),
        Some(flexcore_contracts::units::EnergyMWh::new(4.0).expect("valid MWh")),
        "3".to_string(),
        "bus-a".to_string(),
        "bess-default".to_string(),
        created,
    )
    .expect("valid fixture asset");
    use flexcore_ports::repository::AssetRepository as _;
    assets
        .insert(ctx, &asset)
        .expect("asset seeds into the composed registry fake");
}

fn telemetry_observation(
    tenant: TenantId,
    asset: AssetId,
    quantity: TelemetryQuantity,
    quality: TelemetryQuality,
    value: Option<QuantityValue>,
    event_ms: i64,
) -> NormalizedTelemetry {
    NormalizedTelemetry::new(NormalizedTelemetryParts {
        tenant_id: tenant,
        asset_id: asset,
        quantity,
        value,
        event_time: EventTime::from_unix_millis(event_ms).expect("valid test event time"),
        ingest_time: IngestTime::from_unix_millis(event_ms).expect("valid test ingest time"),
        source_sequence: 1,
        source_id: "sim:lab-1".to_string(),
        quality,
        source_quality_code: None,
        clock_quality: ClockQuality::Synchronized,
    })
    .expect("canonical test record")
}

// ---------------------------------------------------------------------------
// The tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a_failed_command_derives_a_dispatch_failure_alarm_with_its_source_evidence() {
    let (router, commands, _checks, _assets) = composed_alarm_router(phase5_default_policies(), None);
    let ctx = TenantContext::from_authenticated_claims(AuthenticatedClaims::from_verified(
        VerifiedClaims {
            tenant_id: tenant_id(TENANT_A_SEED),
            subject: "seed".to_string(),
        },
    ));
    commands
        .insert_command_record(&ctx, &command_row(COMMAND_A, tenant_id(TENANT_A_SEED), SITE_A, ASSET_A))
        .expect("command seeds");
    commands
        .record_command_transition(
            &ctx,
            &transition(COMMAND_A, tenant_id(TENANT_A_SEED), "PROPOSED", "FAILED", EVENT_MS + 10_000),
        )
        .expect("transition seeds");

    let (status, body) = call(&router, &get("/api/v1/alarms", Some("admin-a"))).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let alarms = body["data"].as_array().expect("alarm array");
    assert_eq!(alarms.len(), 1, "exactly the FAILED command's alarm: {body}");
    let alarm = &alarms[0];
    assert_eq!(alarm["code"], json!("DISPATCH_FAILURE"));
    assert_eq!(alarm["severity"], json!("CRITICAL"));
    assert_eq!(alarm["state"], json!("ACTIVE"));
    assert_eq!(
        alarm["alarm_id"],
        json!(format!("flexcore:alarm:DISPATCH_FAILURE:{COMMAND_A}"))
    );
    // raised_at = the EVENT time of the transition into FAILED (never the
    // derivation instant).
    assert_eq!(alarm["raised_at_unix_millis"], json!(EVENT_MS + 10_000));
    assert_eq!(alarm["source_kind"], json!("DISPATCH_COMMAND"));
    assert_eq!(
        alarm["source_reference"],
        json!(format!("{COMMAND_A}:check:{CHECK_A}"))
    );
    // The honesty fields travel in the response (verbatim statement, model
    // version, consulted/absent sources).
    assert!(body["derivation_statement"]
        .as_str()
        .expect("statement present")
        .contains("NOT protection signals"));
    assert_eq!(body["model_version"], json!("ALARM-MODEL-0.1.0 (P5.31)"));
    let sources = &body["sources"];
    assert_eq!(sources["dispatch_commands_consulted"], json!(1));
    assert!(sources["absent_sources"]
        .as_array()
        .expect("absent sources named")
        .iter()
        .any(|s| s.as_str().expect("text").contains("edge/adapter health")));
    // cleared_at is never served on the derived-on-read path.
    assert!(alarm.get("cleared_at").is_none() || alarm["cleared_at"].is_null());
    // Single bounded derivation: no cursor, by contract.
    assert!(body["next_cursor"].is_null());
}

#[tokio::test]
async fn good_data_derives_no_alarm_and_the_empty_response_names_the_absent_sources() {
    let (router, commands, checks, _assets) = composed_alarm_router(phase5_default_policies(), None);
    let ctx = TenantContext::from_authenticated_claims(AuthenticatedClaims::from_verified(
        VerifiedClaims {
            tenant_id: tenant_id(TENANT_A_SEED),
            subject: "seed".to_string(),
        },
    ));
    // A COMPLETED command and a PASSED check are NOT alarm sources.
    commands
        .insert_command_record(&ctx, &command_row(COMMAND_A, tenant_id(TENANT_A_SEED), SITE_A, ASSET_A))
        .expect("command seeds");
    commands
        .record_command_transition(
            &ctx,
            &transition(COMMAND_A, tenant_id(TENANT_A_SEED), "PROPOSED", "COMPLETED", EVENT_MS + 10_000),
        )
        .expect("transition seeds");
    checks
        .insert_feasibility_check(&ctx, &feasibility_row(CHECK_A, tenant_id(TENANT_A_SEED), "PASSED"))
        .expect("check seeds");

    let (status, body) = call(&router, &get("/api/v1/alarms", Some("admin-a"))).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"].as_array().expect("array").len(), 0);
    assert_eq!(body["sources"]["dispatch_commands_consulted"], json!(1));
    assert_eq!(body["sources"]["feasibility_checks_consulted"], json!(1));
    // The telemetry source is named unavailable (no port wired in this
    // composition) — never silently omitted, never fabricated.
    assert!(body["sources"]["telemetry_current_state"]
        .as_str()
        .expect("source status present")
        .starts_with("unavailable"));
}

#[tokio::test]
async fn feasibility_evidence_derives_the_recorded_verdict_alarms() {
    let (router, _commands, checks, _assets) = composed_alarm_router(phase5_default_policies(), None);
    let ctx = TenantContext::from_authenticated_claims(AuthenticatedClaims::from_verified(
        VerifiedClaims {
            tenant_id: tenant_id(TENANT_A_SEED),
            subject: "seed".to_string(),
        },
    ));
    checks
        .insert_feasibility_check(
            &ctx,
            &feasibility_row(CHECK_A, tenant_id(TENANT_A_SEED), "LIMIT_VIOLATED"),
        )
        .expect("check seeds");

    let (status, body) = call(&router, &get("/api/v1/alarms?code=FEASIBILITY_LIMIT_VIOLATED", Some("viewer-a"))).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let alarms = body["data"].as_array().expect("array");
    assert_eq!(alarms.len(), 1);
    assert_eq!(alarms[0]["code"], json!("FEASIBILITY_LIMIT_VIOLATED"));
    assert_eq!(alarms[0]["severity"], json!("INFO"));
    assert_eq!(
        alarms[0]["alarm_id"],
        json!(format!("flexcore:alarm:FEASIBILITY_LIMIT_VIOLATED:{CHECK_A}"))
    );
    assert_eq!(alarms[0]["source_reference"], json!(CHECK_A));
    assert_eq!(alarms[0]["raised_at_unix_millis"], json!(EVENT_MS + 5_000));
}

#[tokio::test]
async fn the_wired_telemetry_port_derives_telemetry_quality_alarms() {
    let telemetry = Arc::new(FakeTelemetryReader::new());
    let (router, _commands, _checks, assets) =
        composed_alarm_router(phase5_default_policies(), Some(telemetry.clone()));
    let ctx = TenantContext::from_authenticated_claims(AuthenticatedClaims::from_verified(
        VerifiedClaims {
            tenant_id: tenant_id(TENANT_A_SEED),
            subject: "seed".to_string(),
        },
    ));
    // The telemetry fan-out consults only REGISTERED assets: the chain is
    // seeded into the composed state's own asset repository.
    seed_registry(&ctx, &assets);
    // A BAD active_power observation qualifies; a GOOD soc observation does
    // not (no fabricated alarms from usable data).
    telemetry.seed(vec![
        telemetry_observation(
            tenant_id(TENANT_A_SEED),
            AssetId::new(FlexUuid::parse(ASSET_A).expect("valid uuid")),
            TelemetryQuantity::ActivePower,
            TelemetryQuality::Bad,
            Some(QuantityValue::Power(
                flexcore_contracts::units::ActivePowerMW::new(0.0).expect("valid MW"),
            )),
            EVENT_MS,
        ),
        telemetry_observation(
            tenant_id(TENANT_A_SEED),
            AssetId::new(FlexUuid::parse(ASSET_A).expect("valid uuid")),
            TelemetryQuantity::Soc,
            TelemetryQuality::Good,
            Some(QuantityValue::Fraction(
                flexcore_contracts::units::StateOfCharge::new(0.5).expect("valid SOC"),
            )),
            EVENT_MS,
        ),
    ]);
    let (status, body) = call(&router, &get("/api/v1/alarms", Some("admin-a"))).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let alarms = body["data"].as_array().expect("array");
    assert_eq!(alarms.len(), 1, "exactly the BAD observation's alarm: {body}");
    assert_eq!(alarms[0]["code"], json!("TELEMETRY_LOSS"));
    assert_eq!(alarms[0]["severity"], json!("WARNING"));
    assert_eq!(alarms[0]["asset_id"], json!(ASSET_A));
    assert_eq!(alarms[0]["raised_at_unix_millis"], json!(EVENT_MS));
    assert_eq!(alarms[0]["source_kind"], json!("TELEMETRY_CURRENT_STATE"));
    assert!(alarms[0]["source_reference"]
        .as_str()
        .expect("reference present")
        .contains("active_power"));
    assert_eq!(body["sources"]["telemetry_assets_consulted"], json!(1));
    assert_eq!(
        body["sources"]["telemetry_current_state"],
        json!("available")
    );
}

#[tokio::test]
async fn the_derivation_never_leaks_another_tenant_s_records() {
    let (router, commands, checks, _assets) = composed_alarm_router(phase5_default_policies(), None);
    let ctx_a = TenantContext::from_authenticated_claims(AuthenticatedClaims::from_verified(
        VerifiedClaims {
            tenant_id: tenant_id(TENANT_A_SEED),
            subject: "seed".to_string(),
        },
    ));
    let ctx_b = TenantContext::from_authenticated_claims(AuthenticatedClaims::from_verified(
        VerifiedClaims {
            tenant_id: tenant_id(TENANT_B_SEED),
            subject: "seed".to_string(),
        },
    ));
    // Tenant B owns the failing evidence; tenant A's derivation must not see it.
    commands
        .insert_command_record(&ctx_b, &command_row(COMMAND_B_TENANT, tenant_id(TENANT_B_SEED), SITE_A, ASSET_A))
        .expect("command seeds");
    commands
        .record_command_transition(
            &ctx_b,
            &transition(COMMAND_B_TENANT, tenant_id(TENANT_B_SEED), "PROPOSED", "FAILED", EVENT_MS),
        )
        .expect("transition seeds");
    checks
        .insert_feasibility_check(&ctx_b, &feasibility_row(CHECK_A, tenant_id(TENANT_B_SEED), "FAIL_CLOSED"))
        .expect("check seeds");

    let (status, body) = call(&router, &get("/api/v1/alarms", Some("admin-a"))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"].as_array().expect("array").len(), 0);
    // The consulted counts reflect tenant A's (empty) pages, not tenant B's.
    assert_eq!(body["sources"]["dispatch_commands_consulted"], json!(0));
}

#[tokio::test]
async fn active_only_false_is_refused_with_the_derivation_contract_reason() {
    let (router, _commands, _checks, _assets) = composed_alarm_router(phase5_default_policies(), None);
    let (status, body) = call(
        &router,
        &get("/api/v1/alarms?active_only=false", Some("admin-a")),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(body["error"]["message"]
        .as_str()
        .expect("message present")
        .contains("no alarm persistence exists"));
}

#[tokio::test]
async fn malformed_filters_are_422_never_silent_match_all() {
    let (router, _commands, _checks, _assets) = composed_alarm_router(phase5_default_policies(), None);
    for uri in [
        "/api/v1/alarms?severity=EMERGENCY",
        "/api/v1/alarms?code=MARKET_SIGNAL_LOST",
        "/api/v1/alarms?asset_id=not-a-uuid",
        "/api/v1/alarms?from_event_unix_millis=200&to_event_unix_millis=100",
    ] {
        let (status, body) = call(&router, &get(uri, Some("admin-a"))).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{uri}: {body}");
    }
}

#[tokio::test]
async fn the_time_window_filters_the_raised_at_event_time() {
    let (router, commands, _checks, _assets) = composed_alarm_router(phase5_default_policies(), None);
    let ctx = TenantContext::from_authenticated_claims(AuthenticatedClaims::from_verified(
        VerifiedClaims {
            tenant_id: tenant_id(TENANT_A_SEED),
            subject: "seed".to_string(),
        },
    ));
    commands
        .insert_command_record(&ctx, &command_row(COMMAND_A, tenant_id(TENANT_A_SEED), SITE_A, ASSET_A))
        .expect("command seeds");
    commands
        .record_command_transition(
            &ctx,
            &transition(COMMAND_A, tenant_id(TENANT_A_SEED), "PROPOSED", "FAILED", EVENT_MS + 10_000),
        )
        .expect("transition seeds");

    // Window containing the transition time: served.
    let (status, body) = call(
        &router,
        &get(
            &format!("/api/v1/alarms?from_event_unix_millis={}&to_event_unix_millis={}", EVENT_MS, EVENT_MS + 20_000),
            Some("admin-a"),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"].as_array().expect("array").len(), 1);
    // Window before the transition time: empty (the EVENT time is the
    // filter key, not the derivation time).
    let (status, body) = call(
        &router,
        &get(
            &format!("/api/v1/alarms?to_event_unix_millis={EVENT_MS}"),
            Some("admin-a"),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["data"].as_array().expect("array").len(), 0);
}

#[tokio::test]
async fn least_privilege_unauthenticated_401_empty_policy_403_every_role_reads() {
    let (router, _commands, _checks, _assets) = composed_alarm_router(phase5_default_policies(), None);
    // No credentials: 401.
    let (status, _) = call(&router, &get("/api/v1/alarms", None)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    // Unknown token: 401 (never an anonymous fallback).
    let (status, _) = call(&router, &get("/api/v1/alarms", Some("nobody"))).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // Empty policy set: deny-by-default 403.
    let (denied, _commands2, _checks2, _assets2) = composed_alarm_router(PolicySet::new(), None);
    let (status, body) = call(&denied, &get("/api/v1/alarms", Some("admin-a"))).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert!(body["error"]["message"]
        .as_str()
        .expect("reason present")
        .contains("alarm.read"));

    // Viewer (the least-privileged role) reads fine under the Phase 5 set.
    let (status, _) = call(&router, &get("/api/v1/alarms", Some("viewer-a"))).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn the_alarm_surface_is_get_only_in_the_generated_openapi_and_at_runtime() {
    let (router, _commands, _checks, _assets) = composed_alarm_router(phase5_default_policies(), None);
    // The generated OpenAPI registers exactly one alarms path with GET.
    let (status, body) = call(&router, &get("/api/v1/openapi.json", Some("admin-a"))).await;
    assert_eq!(status, StatusCode::OK);
    let paths = body["paths"].as_object().expect("paths object");
    let alarm_paths: Vec<_> = paths
        .keys()
        .filter(|p| p.contains("alarm"))
        .cloned()
        .collect();
    assert_eq!(alarm_paths, vec!["/api/v1/alarms".to_string()]);
    let operations = paths["/api/v1/alarms"].as_object().expect("path item");
    assert!(operations.get("get").is_some(), "GET registered");
    assert!(
        operations.get("post").is_none()
            && operations.get("put").is_none()
            && operations.get("patch").is_none()
            && operations.get("delete").is_none(),
        "no mutation operation exists on the alarm surface"
    );

    // And a live POST probe is refused (the route does not exist).
    let (status, _) = call(&router, &post("/api/v1/alarms", "admin-a")).await;
    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
}
