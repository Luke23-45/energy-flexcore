// P3.28 — Phase 3 API contract tests (§15.1–§15.3, §15.5; ledger P3.28
//         step 4): the flexibility-envelope, schedule, and optimization-run
//         resources over the Test-variant backend.
//
// Composition rule (no network, no database — the P1.17 test pattern): the
// app state is built with the flexcore-ports fakes plus the in-crate Phase 3
// record-store fakes (`phase3_fakes`), a fixed identity resolver, and the
// cumulative Phase 3 policy set. Requests go through the real router via
// `tower::ServiceExt::oneshot`.
//
// Recorded assertions covered here (the ledger P3.28 acceptance list):
// - **OpenAPI-shape validation**: every 201/200 response body is validated
//   against the GENERATED OpenAPI document (components.schemas — the spec
//   is generated from the route annotations, never hand-written): every
//   returned field must be a schema property and every required property
//   must be returned.
// - **Outbox rows carry the schema version**: the serialized envelope in
//   every outbox row parses and carries `schema_version == 1`
//   (SUPPORTED_SCHEMA_VERSION) and the frozen event name.
// - **State + outbox atomic pairing**: each write's journal shows the
//   state-change receipt followed by the paired outbox append (the Test
//   backend has no transaction — see the `phase3_fakes` module header; the
//   database-gated atomicity evidence is the adapter's). A forced publish
//   failure fails CLOSED (no success response, no outbox row).
// - **Cross-tenant access is denied with no existence leak**: another
//   tenant's records read as missing (404), for writes and for the
//   by-id schedule-chain read.
// - **ScheduleValidated is tested as asset/time-coupled validation ONLY**:
//   the emitted frozen payload carries NO scope field (recorded gap — the
//   frozen shape cannot carry the caveat), the record fields carry the
//   scope statement, a FAILed validation stores NOTHING and emits NO
//   event, and the emitted-validated event is bound to a PASS revision.
// - **No execute-command operation exists** (§15.2): a structural test
//   over the generated OpenAPI paths (plus live probes of the forbidden
//   paths) asserts the surface never grows an execute/dispatch/approve
//   endpoint.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use flexcore_authz::policy::Role;
use flexcore_contracts::ids::FlexUuid;
use flexcore_control_plane::auth::{AuthError, Caller, IdentityResolver, phase3_default_policies};
use flexcore_control_plane::dto_phase3::{PHASE3_DEFAULT_PAGE_SIZE, PHASE3_MAX_PAGE_SIZE};
use flexcore_control_plane::phase3_fakes::{
    FakeEnvelopeRecordStore, FakeScheduleRecordStore, FakeScheduleRevisionStore,
};
use flexcore_control_plane::probes::Readiness;
use flexcore_control_plane::routes::stage1_router;
use flexcore_control_plane::routes_phase3::schedule_validated_scope_statement;
use flexcore_control_plane::state::AppState;
use flexcore_domain::ids::TenantId;
use flexcore_domain::tenant_context::{AuthenticatedClaims, TenantContext, VerifiedClaims};
use flexcore_events::envelope::SUPPORTED_SCHEMA_VERSION;
use flexcore_ports::fakes::{
    CountingIdGenerator, FakeEventOutbox, FakeJournalOp, FakePortfolioRepository,
    FakeSiteRepository, FakeTenantRepository, FakeWriteJournal, FixedClock,
};
use flexcore_ports::state_change::StateChangeOperation;
use serde_json::{Value, json};
use tower::ServiceExt;

const TENANT_A_SEED: u128 = 1;
const TENANT_B_SEED: u128 = 2;

fn tenant_id(seed: u128) -> TenantId {
    TenantId::new(FlexUuid::from_bytes(seed.to_be_bytes()))
}

/// Fixed canonical UUIDs for the caller-supplied §12.6 candidate identities
/// (byte-ordered ascending, so the keyset page order is predictable).
const CANDIDATE_1: &str = "0192a5e0-0000-7000-8000-0000000000c1";
const CANDIDATE_2: &str = "0192a5e0-0000-7000-8000-0000000000c2";
const CANDIDATE_3: &str = "0192a5e0-0000-7000-8000-0000000000c3";
/// A candidate id that references no registered optimization-run record.
const CANDIDATE_UNKNOWN: &str = "0192a5e0-0000-7000-8000-0000000000ff";
/// The directly seeded ownership-chain ids of the failing-outbox
/// composition (canonical UUID text; see
/// `composed_router_with_failing_outbox`).
const FIXTURE_PORTFOLIO_ID: &str = "0192a5e0-0000-7000-8000-0000000000a1";
const FIXTURE_SITE_ID: &str = "0192a5e0-0000-7000-8000-0000000000a2";

// ---------------------------------------------------------------------------
// Identity + composition (the P1.17 test pattern, extended with the Phase 3
// record-store fakes)
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

/// The full Phase 3 Test composition: router + shared outbox + journal +
/// the three Phase 3 record-store fakes (for storage inspection).
fn composed_phase3_router() -> (
    Router,
    Arc<FakeEventOutbox>,
    FakeWriteJournal,
    Arc<FakeScheduleRecordStore>,
    Arc<FakeScheduleRevisionStore>,
    Arc<FakeEnvelopeRecordStore>,
) {
    let journal = FakeWriteJournal::new();
    let outbox = Arc::new(FakeEventOutbox::new(journal.clone()));
    let schedule_records = Arc::new(FakeScheduleRecordStore::new());
    let schedule_revisions = Arc::new(FakeScheduleRevisionStore::new());
    let envelope_records = Arc::new(FakeEnvelopeRecordStore::new());
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
                "operator".to_string(),
                caller_for(TENANT_A_SEED, "user:operator", Role::Operator),
            ),
            (
                "viewer".to_string(),
                caller_for(TENANT_A_SEED, "user:viewer", Role::Viewer),
            ),
        ],
    };
    let state = AppState::for_tests_with_phase3(
        Arc::new(FakeTenantRepository::new(journal.clone())),
        Arc::new(FakePortfolioRepository::new(journal.clone())),
        Arc::new(FakeSiteRepository::new(journal.clone())),
        // The Phase 3 surface never touches assets; the fake is wired for
        // completeness of the bundle.
        Arc::new(flexcore_ports::fakes::FakeAssetRepository::new(journal.clone())),
        outbox.clone(),
        Arc::new(CountingIdGenerator::new()),
        Arc::new(FixedClock::at_unix_millis(1_700_000_000_000)),
        phase3_default_policies(),
        Arc::new(resolver),
        passing_readiness(),
        None,
        300_000,
        schedule_records.clone(),
        schedule_revisions.clone(),
        envelope_records.clone(),
    );
    (
        stage1_router(state),
        outbox,
        journal,
        schedule_records,
        schedule_revisions,
        envelope_records,
    )
}

/// A composition whose outbox ALWAYS fails (`FailingEventOutbox`): the
/// forced-publish-failure injection. The tenant-A ownership chain is seeded
/// DIRECTLY through the repository fakes (their `insert` port methods emit
/// no events), because every route — including the fixture-setup routes —
/// pairs its writes with an outbox append that must succeed to return 201.
fn composed_router_with_failing_outbox() -> Router {
    use flexcore_contracts::time_model::{EventTime, SiteTimezone};
    use flexcore_domain::entities::{Portfolio, Site, Tenant};
    use flexcore_domain::ids::{PortfolioId, SiteId};
    use flexcore_ports::repository::{PortfolioRepository as _, SiteRepository as _, TenantRepository as _};

    // Fixed chain ids (canonical UUID text; the routes parse them).
    const PORTFOLIO: &str = FIXTURE_PORTFOLIO_ID;
    const SITE: &str = FIXTURE_SITE_ID;

    let journal = FakeWriteJournal::new();
    let tenants = Arc::new(FakeTenantRepository::new(journal.clone()));
    let portfolios = Arc::new(FakePortfolioRepository::new(journal.clone()));
    let sites = Arc::new(FakeSiteRepository::new(journal.clone()));
    let event_time = EventTime::from_unix_millis(1_700_000_000_000)
        .expect("the fixture event time is valid");
    let tenant = Tenant::try_new(
        tenant_id(TENANT_A_SEED),
        "Alpha Grid Co".to_string(),
        None,
        "idp:alpha".to_string(),
        "profile:default".to_string(),
        event_time,
    )
    .expect("the fixture tenant builds");
    tenants.insert(&tenant).expect("the fixture tenant inserts");
    let portfolio = Portfolio::try_new(
        &tenant,
        PortfolioId::new(FlexUuid::parse(PORTFOLIO).expect("canonical")),
        "Alpha Portfolio".to_string(),
        None,
        "ops-a".to_string(),
        "EU-DE".to_string(),
        "rules:default".to_string(),
        event_time,
    )
    .expect("the fixture portfolio builds");
    let ctx = TenantContext::from_authenticated_claims(AuthenticatedClaims::from_verified(
        VerifiedClaims {
            tenant_id: tenant_id(TENANT_A_SEED),
            subject: "user:admin-a".to_string(),
        },
    ));
    portfolios.insert(&ctx, &portfolio).expect("the fixture portfolio inserts");
    let site = Site::try_new(
        &portfolio,
        SiteId::new(FlexUuid::parse(SITE).expect("canonical")),
        "Alpha Site".to_string(),
        None,
        "52.52N,13.40E".to_string(),
        SiteTimezone::new("UTC".to_string()).expect("UTC is a valid site timezone"),
        "poc-1".to_string(),
        "grid-model-alpha-v1".to_string(),
        "edge:alpha-site:1".to_string(),
        event_time,
    )
    .expect("the fixture site builds");
    sites.insert(&ctx, &site).expect("the fixture site inserts");

    let resolver = FixedIdentityResolver {
        callers: vec![(
            "caller-a".to_string(),
            caller_for(TENANT_A_SEED, "user:admin-a", Role::TenantAdmin),
        )],
    };
    let state = AppState::for_tests_with_phase3(
        tenants,
        portfolios,
        sites,
        Arc::new(flexcore_ports::fakes::FakeAssetRepository::new(journal.clone())),
        Arc::new(flexcore_control_plane::phase3_fakes::FailingEventOutbox),
        Arc::new(CountingIdGenerator::new()),
        Arc::new(FixedClock::at_unix_millis(1_700_000_000_000)),
        phase3_default_policies(),
        Arc::new(resolver),
        passing_readiness(),
        None,
        300_000,
        Arc::new(FakeScheduleRecordStore::new()),
        Arc::new(FakeScheduleRevisionStore::new()),
        Arc::new(FakeEnvelopeRecordStore::new()),
    );
    stage1_router(state)
}

// ---------------------------------------------------------------------------
// Request helpers
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
    if let Some(t) = token {
        builder = builder.header("authorization", format!("Bearer {t}"));
    }
    builder.body(Body::empty()).expect("valid GET request")
}

fn post_json(uri: &str, token: Option<&str>, body: Value) -> Request<Body> {
    let mut builder = Request::builder().method("POST").uri(uri);
    if let Some(t) = token {
        builder = builder.header("authorization", format!("Bearer {t}"));
    }
    builder
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .expect("valid POST request")
}

// ---------------------------------------------------------------------------
// Fixture: tenant A -> portfolio -> site (the ownership chain the Phase 3
// records reference; the routes verify it in-tenant per request)
// ---------------------------------------------------------------------------

/// The ids of the tenant-A ownership chain created through the real routes
/// (the counting generator yields tenant=...0001, portfolio=...0002,
/// site=...0003; the tests read the ids from the responses, never hard-code
/// them).
struct Chain {
    portfolio_id: String,
    site_id: String,
    site_grid_model_reference: String,
}

async fn create_chain(router: &Router) -> Chain {
    let (status, _) = call(
        router,
        post_json(
            "/api/v1/tenants",
            Some("caller-a"),
            json!({
                "name": "Alpha Grid Co",
                "identity_provider_reference": "idp:alpha",
                "policy_profile": "profile:default"
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "tenant A setup");
    let (status, body) = call(
        router,
        post_json(
            "/api/v1/portfolios",
            Some("caller-a"),
            json!({
                "name": "Alpha Portfolio",
                "operator": "ops-a",
                "market_region": "EU-DE",
                "aggregation_rules": "rules:default"
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "portfolio setup");
    let portfolio_id = body["data"]["id"].as_str().expect("portfolio id").to_string();
    let (status, body) = call(
        router,
        post_json(
            "/api/v1/sites",
            Some("caller-a"),
            json!({
                "portfolio_id": portfolio_id,
                "name": "Alpha Site",
                "location": "52.52N,13.40E",
                "timezone": "UTC",
                "point_of_common_coupling": "poc-1",
                "grid_model_reference": "grid-model-alpha-v1",
                "edge_identity": "edge:alpha-site:1"
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "site setup");
    Chain {
        portfolio_id: portfolio_id.clone(),
        site_id: body["data"]["id"].as_str().expect("site id").to_string(),
        site_grid_model_reference: body["data"]["grid_model_reference"]
            .as_str()
            .expect("site grid model reference")
            .to_string(),
    }
}

// ---------------------------------------------------------------------------
// Body builders
// ---------------------------------------------------------------------------

fn optimization_run_body(chain: &Chain, candidate_id: &str) -> Value {
    json!({
        "portfolio_id": chain.portfolio_id,
        "site_id": chain.site_id,
        "candidate_id": candidate_id,
        "status": "optimal",
        "problem_class": "lp",
        "solver_name": "highs",
        "solver_version": "1.15.1",
        "provider_id": "tier1-python-worker",
        "provider_version": "0.1.0",
        "objective_value": 12.5,
        "objective_units": "currency-units",
        "input_snapshot_id": "schedule_input_snapshot:env-v1:v1:synthetic:fnv1a64:0",
        "grid_model_version": "grid-model-alpha-v1",
        "record_payload_json": "{\"status\":\"optimal\",\"diagnostics\":[]}"
    })
}

fn envelope_body(chain: &Chain, version: &str, site_ids: Vec<String>) -> Value {
    json!({
        "portfolio_id": chain.portfolio_id,
        "envelope_version": version,
        "horizon_start_millis": 1_700_000_000_000_i64,
        "horizon_end_millis": 1_700_008_640_000_i64,
        "interval_millis": 900_000_i64,
        "interval_count": 96,
        "covered_site_ids": site_ids,
        "envelope_payload_json": "{\"intervals\":[]}"
    })
}

fn schedule_body(chain: &Chain, candidate_id: &str) -> Value {
    json!({
        "portfolio_id": chain.portfolio_id,
        "site_id": chain.site_id,
        "revision": 1,
        "horizon_start_millis": 1_700_000_000_000_i64,
        "horizon_end_millis": 1_700_008_640_000_i64,
        "roll_trigger": "initial_solve",
        "candidate_id": candidate_id,
        "asset_time_coupled_validation_passed": true,
        "schedule_payload_json": "{\"setpoints\":[]}"
    })
}

/// Registers one optimization-run record (the persist-before-use
/// prerequisite of every schedule revision POST).
async fn register_run(router: &Router, chain: &Chain, candidate_id: &str) {
    let (status, _) = call(
        router,
        post_json(
            "/api/v1/optimization-runs",
            Some("caller-a"),
            optimization_run_body(chain, candidate_id),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "run registration setup");
}

/// Fetches the generated OpenAPI document (the spec endpoint is
/// unauthenticated by design: it is a contract artifact, not tenant data).
async fn openapi_spec(router: &Router) -> Value {
    let (status, body) = call(router, get("/api/v1/openapi.json", None)).await;
    assert_eq!(status, StatusCode::OK, "the OpenAPI spec is reachable");
    body
}

// ---------------------------------------------------------------------------
// OpenAPI-shape validation (response bodies validate against the GENERATED
// schemas; ledger P3.28 acceptance "API responses validate against OpenAPI")
// ---------------------------------------------------------------------------

/// Resolves one component schema by name; generic names resolve by the
/// utoipa `Base_Item` convention (VERIFY-BUILD: utoipa 5 renders
/// `DataEnvelope<T>` as `DataEnvelope_<T>` — the lookup below also falls
/// back to a prefix match so a renderer naming change fails as a clear
/// panic, not a silent pass).
fn schema<'a>(spec: &'a Value, name: &str) -> (&'a str, &'a Value) {
    let components = &spec["components"]["schemas"];
    let components = components
        .as_object()
        .expect("the generated spec carries components.schemas");
    if let Some(pair) = components.get_key_value(name) {
        return (pair.0.as_str(), pair.1);
    }
    components
        .iter()
        .find(|(k, _)| k.starts_with(&format!("{name}_")))
        .map(|(k, v)| (k.as_str(), v))
        .unwrap_or_else(|| {
            let available: Vec<&String> = components.keys().collect();
            panic!("schema {name} is missing from the generated OpenAPI document; available: {available:?}")
        })
}

/// Resolves a `#/components/schemas/<name>` reference to its component
/// name.
fn ref_name(reference: &str) -> &str {
    reference
        .rsplit('/')
        .next()
        .expect("a $ref always has a name segment")
}

/// Validates one response value against a component schema: every returned
/// field must be a declared property (no shape drift), every required
/// property must be returned, and nested `$ref` properties recurse.
fn validate_against_schema(spec: &Value, schema_name: &str, value: &Value, at: &str) {
    let (name, schema) = schema(spec, schema_name);
    let object = value
        .as_object()
        .unwrap_or_else(|| panic!("{at}: the response is not the JSON object {name} requires"));
    let properties = schema["properties"]
        .as_object()
        .unwrap_or_else(|| panic!("{at}: schema {name} declares no properties"));
    for key in object.keys() {
        assert!(
            properties.contains_key(key),
            "{at}: response field '{key}' is NOT a property of the OpenAPI schema {name}"
        );
    }
    if let Some(required) = schema["required"].as_array() {
        for r in required {
            let r = r.as_str().expect("required entries are strings");
            assert!(
                object.contains_key(r),
                "{at}: required OpenAPI property '{r}' of {name} is missing from the response"
            );
        }
    }
    for (key, property) in properties {
        let Some(field) = object.get(key) else {
            continue;
        };
        if let Some(nested) = property["$ref"].as_str() {
            validate_against_schema(spec, ref_name(nested), field, &format!("{at}.{key}"));
        }
        if let Some(items) = property["items"]["$ref"].as_str() {
            let elements = field
                .as_array()
                .unwrap_or_else(|| panic!("{at}.{key}: the array field is not an array"));
            for (index, element) in elements.iter().enumerate() {
                validate_against_schema(
                    spec,
                    ref_name(items),
                    element,
                    &format!("{at}.{key}[{index}]"),
                );
            }
        }
    }
}

/// Validates a `DataEnvelope<T>` response body against the generated
/// `DataEnvelope_<T>` schema.
fn validate_envelope_response(spec: &Value, item: &str, body: &Value, at: &str) {
    let (name, schema) = schema(spec, &format!("DataEnvelope_{item}"));
    let data_ref = schema["properties"]["data"]["$ref"]
        .as_str()
        .unwrap_or_else(|| panic!("{at}: {name} does not reference the {item} item schema"));
    validate_against_schema(spec, ref_name(data_ref), &body["data"], at);
}

/// Validates a page response (`{"data": [T...], "next_cursor"}`) against
/// the generated page schema.
fn validate_page_response(spec: &Value, page: &str, item: &str, body: &Value, at: &str) {
    let (name, schema) = schema(spec, page);
    let items_ref = schema["properties"]["data"]["items"]["$ref"]
        .as_str()
        .unwrap_or_else(|| panic!("{at}: {name} does not reference the {item} item schema"));
    let elements = body["data"]
        .as_array()
        .unwrap_or_else(|| panic!("{at}: the page data is not an array"));
    for (index, element) in elements.iter().enumerate() {
        validate_against_schema(
            spec,
            ref_name(items_ref),
            element,
            &format!("{at}.data[{index}]"),
        );
    }
    let _ = name;
}

// ---------------------------------------------------------------------------
// Outbox inspection helpers (the §15.5 evidence)
// ---------------------------------------------------------------------------

/// Renders the frozen PascalCase event name as the wire tag
/// (`ScheduleProposed` -> `SCHEDULE_PROPOSED`; the DomainEvent enum is
/// internally tagged with `event_type`, SCREAMING_SNAKE_CASE — the frozen
/// flexcore-events wire shape).
fn screaming_snake(name: &str) -> String {
    let mut out = String::with_capacity(name.len() + 8);
    for (index, c) in name.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if index > 0 {
                out.push('_');
            }
            out.push(c.to_ascii_uppercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// Parses one serialized outbox payload and asserts it is a supported
/// (schema_version 1) envelope carrying the expected frozen event name.
fn assert_outbox_payload(write: &flexcore_ports::event_outbox::OutboxWrite, event: &str) {
    let payload: Value = serde_json::from_str(write.payload())
        .expect("the outbox payload is the serialized event envelope");
    assert_eq!(
        payload["schema_version"].as_u64(),
        Some(SUPPORTED_SCHEMA_VERSION as u64),
        "the outbox row carries the envelope schema version"
    );
    // The typed event is the envelope's event field; the internally tagged
    // wire form carries the SCREAMING_SNAKE event_type tag next to the
    // payload fields.
    assert_eq!(
        payload["event"]["event_type"].as_str(),
        Some(screaming_snake(event).as_str()),
        "the outbox row carries the frozen event name"
    );
}

/// The outbox rows paired with ONE Phase 3 record entity. The fixture
/// setup (tenant/portfolio/site creation through the registry routes) also
/// emits its own paired outbox rows — the Phase 3 assertions filter by the
/// record entity carried on each write's state-change receipt.
fn phase3_writes(
    outbox: &FakeEventOutbox,
    entity: &str,
) -> Vec<flexcore_ports::event_outbox::OutboxWrite> {
    outbox
        .writes()
        .into_iter()
        .filter(|w| w.state_change().entity() == entity)
        .collect()
}

/// Asserts the journal shows one state-change receipt followed by exactly
/// one paired outbox append for `entity` (the §15.5 pairing in the Test
/// backend's journalled ordering).
fn assert_state_then_outbox(journal_ops: &[FakeJournalOp], entity: &str) {
    let mut found_state = false;
    for op in journal_ops {
        match op {
            FakeJournalOp::StateChange { entity: e, .. } if e == entity => {
                found_state = true;
            }
            FakeJournalOp::OutboxAppend { entity: e, .. } if e == entity => {
                assert!(
                    found_state,
                    "the outbox append for {entity} must follow its state-change receipt"
                );
                return;
            }
            _ => {}
        }
    }
    panic!("no state-change + outbox-append pairing found for {entity}");
}

// ---------------------------------------------------------------------------
// Tests — optimization runs (§12.6 persist-before-use + ScheduleProposed)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn optimization_run_post_is_201_schema_valid_and_pairs_state_with_outbox() {
    let (router, outbox, journal, records, _revisions, _envelopes) = composed_phase3_router();
    let chain = create_chain(&router).await;
    let (status, body) = call(
        &router,
        post_json(
            "/api/v1/optimization-runs",
            Some("caller-a"),
            optimization_run_body(&chain, CANDIDATE_1),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(body["data"]["candidate_id"], CANDIDATE_1);
    assert_eq!(body["data"]["status"], "optimal");
    // The record is a CANDIDATE: the lifecycle statement travels in the
    // response; `status` is the SOLVER outcome only.
    assert_eq!(body["data"]["candidate_status"], "candidate");
    assert!(body["data"]["candidate_status_statement"]
        .as_str()
        .expect("candidate status statement")
        .contains("CANDIDATE"));
    // The grid-model traceability is the SITE's own Phase 2 reference, not
    // caller-invented (the P3.21 GRID_MODEL_PROVENANCE rule).
    assert_eq!(
        body["data"]["grid_model_site_reference"],
        chain.site_grid_model_reference.as_str()
    );
    assert_eq!(body["data"]["grid_model_site_id"], chain.site_id.as_str());

    // Response validates against the generated OpenAPI schema.
    let spec = openapi_spec(&router).await;
    validate_envelope_response(&spec, "OptimizationRunDto", &body, "POST /optimization-runs");

    // The §15.5 pairing: one state-change receipt + one ScheduleProposed
    // outbox append, state first, in the SAME unit of work.
    assert_state_then_outbox(&journal.ops(), "schedule_results");
    let writes = phase3_writes(&outbox, "schedule_results");
    assert_eq!(writes.len(), 1, "one POST = exactly one outbox row");
    assert_eq!(
        writes[0].idempotency_key(),
        format!("schedule_proposed:{}:{}:{CANDIDATE_1}", chain.portfolio_id, chain.site_id),
        "the deterministic P3.21 idempotency key"
    );
    assert_eq!(
        writes[0].destination_subject(),
        "flexcore.v1.scheduling.schedule_proposed"
    );
    assert_outbox_payload(&writes[0], "ScheduleProposed");
    // The row records the key as the pairing proof (§15.5).
    let stored = records.rows();
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].schedule_proposed_key, writes[0].idempotency_key());
    assert_eq!(stored[0].tenant_id, tenant_id(TENANT_A_SEED));
}

#[tokio::test]
async fn duplicate_optimization_run_candidate_is_409() {
    let (router, _outbox, _journal, _records, _revisions, _envelopes) = composed_phase3_router();
    let chain = create_chain(&router).await;
    register_run(&router, &chain, CANDIDATE_1).await;
    let (status, body) = call(
        &router,
        post_json(
            "/api/v1/optimization-runs",
            Some("caller-a"),
            optimization_run_body(&chain, CANDIDATE_1),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["error"]["code"], "candidate_exists");
}

#[tokio::test]
async fn optimization_run_with_unknown_solver_status_is_422() {
    let (router, _outbox, _journal, records, _revisions, _envelopes) = composed_phase3_router();
    let chain = create_chain(&router).await;
    let mut body = optimization_run_body(&chain, CANDIDATE_1);
    body["status"] = json!("unbounded");
    let (status, _) = call(&router, post_json("/api/v1/optimization-runs", Some("caller-a"), body)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    // Fail-before-write: a rejected record leaves NOTHING stored.
    assert!(
        records.rows().is_empty(),
        "a validation-rejected POST stores nothing"
    );
}

// ---------------------------------------------------------------------------
// Tests — flexibility envelopes (one FlexibilityEnvelopeUpdated per site)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn envelope_post_is_201_and_emits_one_event_per_covered_site() {
    let (router, outbox, journal, _records, _revisions, envelopes) = composed_phase3_router();
    let chain = create_chain(&router).await;
    // Two covered sites: register a second site in the same portfolio.
    let (status, body) = call(
        &router,
        post_json(
            "/api/v1/sites",
            Some("caller-a"),
            json!({
                "portfolio_id": chain.portfolio_id,
                "name": "Alpha Site 2",
                "location": "52.52N,13.41E",
                "timezone": "UTC",
                "point_of_common_coupling": "poc-2",
                "grid_model_reference": "grid-model-alpha-v1",
                "edge_identity": "edge:alpha-site:2"
            }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "second site setup");
    let site_b = body["data"]["id"].as_str().expect("second site id").to_string();
    let (status, envelope) = call(
        &router,
        post_json(
            "/api/v1/flexibility/envelopes",
            Some("caller-a"),
            envelope_body(&chain, "env-v1", vec![chain.site_id.clone(), site_b]),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(envelope["data"]["envelope_version"], "env-v1");
    assert_eq!(envelope["data"]["covered_site_ids"].as_array().expect("sites").len(), 2);
    // The recorded frozen-payload gap note travels in the response.
    assert!(envelope["data"]["event_payload_gap_note"]
        .as_str()
        .expect("gap note")
        .contains("FlexibilityEnvelopeUpdatedPayload"));

    let spec = openapi_spec(&router).await;
    validate_envelope_response(&spec, "EnvelopeRecordDto", &envelope, "POST /flexibility/envelopes");

    // One event per covered site, SAME unit of work, deterministic keys.
    assert_state_then_outbox(&journal.ops(), "flexibility_envelopes");
    let writes = phase3_writes(&outbox, "flexibility_envelopes");
    assert_eq!(writes.len(), 2, "one FlexibilityEnvelopeUpdated per site");
    for (index, write) in writes.iter().enumerate() {
        assert_eq!(
            write.destination_subject(),
            "flexcore.v1.flexibility.flexibility_envelope_updated"
        );
        assert_outbox_payload(write, "FlexibilityEnvelopeUpdated");
        let expected_key = format!(
            "flexibility_envelope_updated:{}:{}:env-v1",
            chain.portfolio_id,
            // The publications preserve the covered-site list order.
            if index == 0 { &chain.site_id } else { &site_b }
        );
        assert_eq!(write.idempotency_key(), expected_key);
    }
    assert_eq!(envelopes.rows().len(), 1, "the envelope record is stored");
}

#[tokio::test]
async fn duplicate_envelope_version_is_409() {
    let (router, _outbox, _journal, _records, _revisions, _envelopes) = composed_phase3_router();
    let chain = create_chain(&router).await;
    let (status, _) = call(
        &router,
        post_json(
            "/api/v1/flexibility/envelopes",
            Some("caller-a"),
            envelope_body(&chain, "env-v1", vec![chain.site_id.clone()]),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let (status, body) = call(
        &router,
        post_json(
            "/api/v1/flexibility/envelopes",
            Some("caller-a"),
            envelope_body(&chain, "env-v1", vec![chain.site_id.clone()]),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["error"]["code"], "duplicate_envelope_version");
}

#[tokio::test]
async fn envelope_without_covered_sites_is_422() {
    let (router, outbox, _journal, _records, _revisions, _envelopes) = composed_phase3_router();
    let chain = create_chain(&router).await;
    let mut body = envelope_body(&chain, "env-v1", Vec::new());
    body["covered_site_ids"] = json!([]);
    let (status, _) = call(&router, post_json("/api/v1/flexibility/envelopes", Some("caller-a"), body)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(
        phase3_writes(&outbox, "flexibility_envelopes").is_empty(),
        "a rejected envelope emits no events"
    );
}

// ---------------------------------------------------------------------------
// Tests — schedules (CANDIDATE revisions, ScheduleProposed + ScheduleValidated)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn schedule_post_is_201_candidate_and_emits_proposed_plus_validated() {
    let (router, outbox, journal, _records, revisions, _envelopes) = composed_phase3_router();
    let chain = create_chain(&router).await;
    register_run(&router, &chain, CANDIDATE_1).await;
    let (status, body) = call(
        &router,
        post_json(
            "/api/v1/schedules",
            Some("caller-a"),
            schedule_body(&chain, CANDIDATE_1),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    // The lifecycle statement: every POSTed schedule is a CANDIDATE; the
    // validation scope is ASSET/TIME-COUPLED ONLY, with the recorded caveat
    // verbatim in the response (NOT network feasibility — Phase 4).
    assert_eq!(body["data"]["candidate_status"], "candidate");
    assert_eq!(body["data"]["validation_scope"], "asset_time_coupled");
    assert_eq!(
        body["data"]["validation_scope_statement"],
        schedule_validated_scope_statement()
    );
    assert_eq!(body["data"]["revision"], 1);
    assert_eq!(body["data"]["state"], "active");

    let spec = openapi_spec(&router).await;
    validate_envelope_response(&spec, "ScheduleRevisionDto", &body, "POST /schedules");
    let schedule_id = body["data"]["schedule_id"].as_str().expect("schedule id").to_string();

    // Two events, one unit of work: ScheduleProposed + ScheduleValidated.
    assert_state_then_outbox(&journal.ops(), "schedule_revisions");
    let writes = phase3_writes(&outbox, "schedule_revisions");
    assert_eq!(writes.len(), 2, "one revision insert = two outbox rows");
    let keys: Vec<&str> = writes.iter().map(|w| w.idempotency_key()).collect();
    assert!(keys.contains(&format!("schedule_proposed:{}:{}:{CANDIDATE_1}", chain.portfolio_id, chain.site_id).as_str()));
    assert!(keys.contains(&format!("schedule_validated:{}:{}:{CANDIDATE_1}", chain.portfolio_id, chain.site_id).as_str()));
    for write in &writes {
        assert_eq!(write.destination_subject().starts_with("flexcore.v1.scheduling."), true);
        assert_outbox_payload(write, if write.idempotency_key().starts_with("schedule_proposed:") { "ScheduleProposed" } else { "ScheduleValidated" });
    }

    // The revision is stored, active, and carries no supersede link.
    let stored = revisions.rows();
    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].row.schedule_id, schedule_id);
    assert_eq!(stored[0].row.revision, 1);
    assert_eq!(stored[0].state, "active");
    assert!(stored[0].superseded_by_revision.is_none());
}

#[tokio::test]
async fn schedule_validated_payload_cannot_carry_the_scope_caveat_recorded_gap() {
    // The frozen ScheduleValidatedPayload (flexcore-events, frozen for this
    // subtask) carries exactly correlation/tenant/portfolio/site/asset +
    // event_time — NO validation-scope field. The caveat therefore travels
    // in the record fields and docs (recorded gap, dto_phase3 module
    // header); this test pins the frozen shape so the gap stays visible.
    let (router, outbox, _journal, _records, _revisions, _envelopes) = composed_phase3_router();
    let chain = create_chain(&router).await;
    register_run(&router, &chain, CANDIDATE_1).await;
    let (status, _) = call(
        &router,
        post_json("/api/v1/schedules", Some("caller-a"), schedule_body(&chain, CANDIDATE_1)),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let validated = phase3_writes(&outbox, "schedule_revisions")
        .into_iter()
        .find(|w| w.idempotency_key().starts_with("schedule_validated:"))
        .expect("the ScheduleValidated outbox row");
    let payload: Value = serde_json::from_str(validated.payload()).expect("serialized envelope");
    // The internally tagged wire form: the payload fields are siblings of
    // the event_type tag inside the envelope's event object.
    let event_payload = &payload["event"];
    let mut keys: Vec<&str> = event_payload
        .as_object()
        .expect("the frozen payload is an object")
        .keys()
        .collect();
    keys.sort();
    assert_eq!(
        keys,
        vec![
            "asset_id",
            "correlation_id",
            "event_time",
            "event_type",
            "portfolio_id",
            "site_id",
            "tenant_id"
        ],
        "the frozen ScheduleValidated payload has NO validation-scope field (the recorded gap)"
    );
    assert!(
        !keys.contains(&"validation_scope"),
        "no scope field exists in the frozen wire shape"
    );
}

#[tokio::test]
async fn schedule_with_failed_asset_time_coupled_validation_is_422_and_stores_nothing() {
    let (router, outbox, _journal, _records, revisions, _envelopes) = composed_phase3_router();
    let chain = create_chain(&router).await;
    register_run(&router, &chain, CANDIDATE_1).await;
    let mut body = schedule_body(&chain, CANDIDATE_1);
    body["asset_time_coupled_validation_passed"] = json!(false);
    let (status, error) = call(&router, post_json("/api/v1/schedules", Some("caller-a"), body)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    // A FAILED Phase 3 validation never becomes a stored candidate and no
    // ScheduleValidated event is emitted for it: NOTHING stored, NO event.
    assert!(
        revisions.rows().is_empty(),
        "a failed-validation schedule stores no revision"
    );
    assert!(
        phase3_writes(&outbox, "schedule_revisions").is_empty(),
        "a failed-validation schedule emits no events"
    );
    assert!(error["error"]["message"]
        .as_str()
        .expect("error message")
        .contains("asset/time-coupled"));
}

#[tokio::test]
async fn schedule_referencing_an_unregistered_candidate_is_422() {
    let (router, outbox, _journal, _records, revisions, _envelopes) = composed_phase3_router();
    let chain = create_chain(&router).await;
    // No optimization-run record for CANDIDATE_UNKNOWN: the revision and
    // the run are two views of ONE solve — POST /optimization-runs first.
    let (status, error) = call(
        &router,
        post_json(
            "/api/v1/schedules",
            Some("caller-a"),
            schedule_body(&chain, CANDIDATE_UNKNOWN),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(
        error["error"]["message"].as_str().expect("msg").contains("optimization-runs"),
        "the rejection names the remediation"
    );
    assert!(revisions.rows().is_empty());
    assert!(phase3_writes(&outbox, "schedule_revisions").is_empty());
}

#[tokio::test]
async fn schedule_chain_revision_two_supersedes_one_in_one_transaction() {
    let (router, outbox, _journal, _records, revisions, _envelopes) = composed_phase3_router();
    let chain = create_chain(&router).await;
    register_run(&router, &chain, CANDIDATE_1).await;
    register_run(&router, &chain, CANDIDATE_2).await;
    let (status, first) = call(
        &router,
        post_json("/api/v1/schedules", Some("caller-a"), schedule_body(&chain, CANDIDATE_1)),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let schedule_id = first["data"]["schedule_id"].as_str().expect("schedule id").to_string();

    // Revision 2 continues the chain: supersedes exactly revision 1.
    let mut second = schedule_body(&chain, CANDIDATE_2);
    second["schedule_id"] = json!(schedule_id);
    second["revision"] = json!(2);
    second["roll_trigger"] = json!("horizon_advance");
    second["supersedes_candidate_id"] = json!(CANDIDATE_1);
    let (status, body) = call(&router, post_json("/api/v1/schedules", Some("caller-a"), second)).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(body["data"]["supersedes_revision"], 1);
    assert_eq!(body["data"]["supersedes_candidate_id"], CANDIDATE_1);
    assert_eq!(body["data"]["state"], "active");

    // The full chain read: the superseded revision is retained for audit
    // (§14.2) with the forward link, the current revision is active.
    let (status, page) = call(
        &router,
        get(&format!("/api/v1/schedules?schedule_id={schedule_id}"), Some("caller-a")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let chain_records = page["data"].as_array().expect("chain");
    assert_eq!(chain_records.len(), 2);
    assert_eq!(chain_records[0]["state"], "superseded");
    assert_eq!(chain_records[0]["superseded_by_revision"], 2);
    assert_eq!(chain_records[1]["state"], "active");

    let spec = openapi_spec(&router).await;
    validate_page_response(
        &spec,
        "ScheduleRevisionPageEnvelope",
        "ScheduleRevisionDto",
        &page,
        "GET /schedules?schedule_id",
    );
    assert_eq!(revisions.rows().len(), 2, "both revisions are stored");

    // Two revision inserts = four events (ScheduleProposed + ScheduleValidated
    // per revision), all bound to their state change.
    assert_eq!(phase3_writes(&outbox, "schedule_revisions").len(), 4);
}

#[tokio::test]
async fn duplicate_schedule_revision_is_409() {
    let (router, _outbox, _journal, _records, _revisions, _envelopes) = composed_phase3_router();
    let chain = create_chain(&router).await;
    register_run(&router, &chain, CANDIDATE_1).await;
    let (status, first) = call(
        &router,
        post_json("/api/v1/schedules", Some("caller-a"), schedule_body(&chain, CANDIDATE_1)),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let schedule_id = first["data"]["schedule_id"].as_str().expect("schedule id");
    let mut duplicate = schedule_body(&chain, CANDIDATE_1);
    duplicate["schedule_id"] = json!(schedule_id);
    let (status, body) = call(
        &router,
        post_json("/api/v1/schedules", Some("caller-a"), duplicate),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["error"]["code"], "schedule_revision_exists");
}

#[tokio::test]
async fn unknown_roll_trigger_is_422() {
    let (router, _outbox, _journal, _records, _revisions, _envelopes) = composed_phase3_router();
    let chain = create_chain(&router).await;
    register_run(&router, &chain, CANDIDATE_1).await;
    let mut body = schedule_body(&chain, CANDIDATE_1);
    body["roll_trigger"] = json!("spontaneous");
    let (status, _) = call(&router, post_json("/api/v1/schedules", Some("caller-a"), body)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
}

// ---------------------------------------------------------------------------
// Tests — cross-tenant isolation (404, no existence leak)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn cross_tenant_writes_and_reads_are_404_with_no_leak() {
    let (router, outbox, _journal, records, revisions, _envelopes) = composed_phase3_router();
    let chain = create_chain(&router).await;
    register_run(&router, &chain, CANDIDATE_1).await;

    // Tenant B references tenant A's portfolio in an envelope POST: 404.
    let (status, body) = call(
        &router,
        post_json(
            "/api/v1/flexibility/envelopes",
            Some("caller-b"),
            envelope_body(&chain, "env-v1", vec![chain.site_id.clone()]),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], "not_found");

    // Tenant B references tenant A's portfolio in an optimization-run POST:
    // 404, and NOTHING is stored in B's (or A's) view.
    let (status, _) = call(
        &router,
        post_json(
            "/api/v1/optimization-runs",
            Some("caller-b"),
            optimization_run_body(&chain, CANDIDATE_2),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(
        records.rows().iter().all(|r| r.tenant_id == tenant_id(TENANT_A_SEED)),
        "no record is created outside the owning tenant"
    );

    // Tenant B reads tenant A's schedule chain: 404 (no existence leak).
    let (status, first) = call(
        &router,
        post_json("/api/v1/schedules", Some("caller-a"), schedule_body(&chain, CANDIDATE_1)),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let schedule_id = first["data"]["schedule_id"].as_str().expect("schedule id");
    let (status, body) = call(
        &router,
        get(&format!("/api/v1/schedules?schedule_id={schedule_id}"), Some("caller-b")),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"]["code"], "not_found");

    // Tenant B's own listings never contain tenant A's records.
    for uri in [
        "/api/v1/schedules",
        "/api/v1/optimization-runs",
        "/api/v1/flexibility/envelopes",
    ] {
        let (status, page) = call(&router, get(uri, Some("caller-b"))).await;
        assert_eq!(status, StatusCode::OK);
        assert!(
            page["data"].as_array().expect("page data").is_empty(),
            "another tenant's records never appear in a listing"
        );
    }
    assert_eq!(revisions.rows().len(), 1, "the chain belongs to tenant A only");
    let phase3_entities = ["flexibility_envelopes", "schedule_results", "schedule_revisions"];
    for entity in phase3_entities {
        for write in phase3_writes(&outbox, entity) {
            assert!(
                write.idempotency_key().contains(&chain.portfolio_id),
                "every emitted {entity} event belongs to tenant A's records"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Tests — authorization (deny-by-default, the P3.28 grant surface)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn viewer_cannot_propose_but_can_read() {
    let (router, _outbox, _journal, _records, _revisions, _envelopes) = composed_phase3_router();
    let chain = create_chain(&router).await;
    register_run(&router, &chain, CANDIDATE_1).await;
    let (status, body) = call(
        &router,
        post_json("/api/v1/schedules", Some("viewer"), schedule_body(&chain, CANDIDATE_1)),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["error"]["code"], "forbidden");

    let (status, _) = call(&router, get("/api/v1/schedules", Some("viewer"))).await;
    assert_eq!(status, StatusCode::OK, "reads are granted to every role");
}

#[tokio::test]
async fn operator_cannot_create_envelopes_or_runs_but_can_propose_schedules() {
    let (router, _outbox, _journal, _records, _revisions, _envelopes) = composed_phase3_router();
    let chain = create_chain(&router).await;
    register_run(&router, &chain, CANDIDATE_1).await;

    let (status, _) = call(
        &router,
        post_json(
            "/api/v1/flexibility/envelopes",
            Some("operator"),
            envelope_body(&chain, "env-v1", vec![chain.site_id.clone()]),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, _) = call(
        &router,
        post_json(
            "/api/v1/optimization-runs",
            Some("operator"),
            optimization_run_body(&chain, CANDIDATE_1),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (status, _) = call(
        &router,
        post_json("/api/v1/schedules", Some("operator"), schedule_body(&chain, CANDIDATE_1)),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "schedule propose is the operator action");
}

#[tokio::test]
async fn unauthenticated_phase3_requests_are_401() {
    let (router, _outbox, _journal, _records, _revisions, _envelopes) = composed_phase3_router();
    for (method_uri, body) in [
        ("/api/v1/schedules", None),
        ("/api/v1/optimization-runs", None),
        ("/api/v1/flexibility/envelopes", None),
    ] {
        let (status, _) = match body {
            None => call(&router, get(method_uri, None)).await,
            Some(b) => call(&router, post_json(method_uri, None, b)).await,
        };
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }
}

// ---------------------------------------------------------------------------
// Tests — keyset pagination + recorded limits
// ---------------------------------------------------------------------------

#[tokio::test]
async fn optimization_run_listing_is_keyset_paginated_with_recorded_limits() {
    let (router, _outbox, _journal, _records, _revisions, _envelopes) = composed_phase3_router();
    let chain = create_chain(&router).await;
    for candidate in [CANDIDATE_1, CANDIDATE_2, CANDIDATE_3] {
        register_run(&router, &chain, candidate).await;
    }

    // Recorded limits: over the maximum and below 1 are 422.
    let (status, _) = call(
        &router,
        get(&format!("/api/v1/optimization-runs?limit={}", PHASE3_MAX_PAGE_SIZE + 1), Some("caller-a")),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (status, _) = call(&router, get("/api/v1/optimization-runs?limit=0", Some("caller-a"))).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    // Keyset walk: page 1 (limit 2) -> cursor -> page 2 (1 row, no cursor).
    let (status, page1) = call(
        &router,
        get("/api/v1/optimization-runs?limit=2", Some("caller-a")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let page1_ids: Vec<&str> = page1["data"]
        .as_array()
        .expect("page data")
        .iter()
        .map(|r| r["candidate_id"].as_str().expect("candidate id"))
        .collect();
    assert_eq!(page1_ids, vec![CANDIDATE_1, CANDIDATE_2], "ascending keyset order");
    let cursor = page1["next_cursor"].as_str().expect("next_cursor on a full page");
    let (status, page2) = call(
        &router,
        get(&format!("/api/v1/optimization-runs?limit=2&cursor={cursor}"), Some("caller-a")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let page2_ids: Vec<&str> = page2["data"]
        .as_array()
        .expect("page data")
        .iter()
        .map(|r| r["candidate_id"].as_str().expect("candidate id"))
        .collect();
    assert_eq!(page2_ids, vec![CANDIDATE_3]);
    assert!(
        page2["next_cursor"].is_null(),
        "the last page carries no next_cursor"
    );

    // A malformed cursor is a 422, never a silent "first page".
    let (status, _) = call(
        &router,
        get("/api/v1/optimization-runs?cursor=not-a-cursor", Some("caller-a")),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

    // The closed §12.6 status vocabulary: a seventh value is 422.
    let (status, _) = call(
        &router,
        get("/api/v1/optimization-runs?status=unbounded", Some("caller-a")),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let (status, filtered) = call(
        &router,
        get("/api/v1/optimization-runs?status=optimal", Some("caller-a")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(filtered["data"].as_array().expect("data").len(), 3);

    // The page response validates against the generated OpenAPI schema.
    let spec = openapi_spec(&router).await;
    validate_page_response(
        &spec,
        "OptimizationRunPageEnvelope",
        "OptimizationRunDto",
        &page1,
        "GET /optimization-runs",
    );
    // The defaults are the recorded constants.
    assert_eq!(PHASE3_DEFAULT_PAGE_SIZE, 200);
    assert_eq!(PHASE3_MAX_PAGE_SIZE, 1000);
}

#[tokio::test]
async fn envelope_and_schedule_listings_are_keyset_paginated() {
    let (router, _outbox, _journal, _records, _revisions, _envelopes) = composed_phase3_router();
    let chain = create_chain(&router).await;
    register_run(&router, &chain, CANDIDATE_1).await;
    register_run(&router, &chain, CANDIDATE_2).await;
    call(
        &router,
        post_json(
            "/api/v1/flexibility/envelopes",
            Some("caller-a"),
            envelope_body(&chain, "env-v1", vec![chain.site_id.clone()]),
        ),
    )
    .await;
    call(
        &router,
        post_json("/api/v1/schedules", Some("caller-a"), schedule_body(&chain, CANDIDATE_1)),
    )
    .await;
    call(
        &router,
        post_json("/api/v1/schedules", Some("caller-a"), schedule_body(&chain, CANDIDATE_2)),
    )
    .await;

    // Envelope listing: one record, keyset shape, schema-valid.
    let (status, page) = call(&router, get("/api/v1/flexibility/envelopes", Some("caller-a"))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(page["data"].as_array().expect("data").len(), 1);
    let spec = openapi_spec(&router).await;
    validate_page_response(
        &spec,
        "EnvelopePageEnvelope",
        "EnvelopeRecordDto",
        &page,
        "GET /flexibility/envelopes",
    );

    // Schedule listing: two chains (two single-revision chains), keyset
    // cursor continues after the first.
    let (status, schedules) = call(&router, get("/api/v1/schedules?limit=1", Some("caller-a"))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(schedules["data"].as_array().expect("data").len(), 1);
    let cursor = schedules["next_cursor"].as_str().expect("full page carries a cursor");
    assert!(
        cursor.contains(':'),
        "the schedule cursor is the composite <schedule_id>:<revision>"
    );
    let (status, rest) = call(
        &router,
        get(&format!("/api/v1/schedules?limit=1&cursor={cursor}"), Some("caller-a")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(rest["data"].as_array().expect("data").len(), 1);
    assert!(rest["next_cursor"].is_null());
}

// ---------------------------------------------------------------------------
// Tests — forced publish failure (fail closed) and the structural
// no-execute-command guarantee
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a_forced_publish_failure_fails_closed() {
    // The outbox append ALWAYS fails: the request must fail CLOSED — no 201,
    // no outbox row, an opaque infrastructure error (never a success
    // response with the events silently missing, which is exactly the
    // dual-write hole §15.5 forbids).
    //
    // RECORDED SCOPE (the phase3_fakes module header): the Test backend has
    // no transaction to roll back, so the state row written BEFORE the
    // failing append is not undone here — the database-gated atomicity
    // evidence (state + outbox in ONE native transaction, commit-or-
    // rollback) is the Postgres adapter's and `run_in_uow`'s commit
    // discipline (state.rs). This test pins the fail-closed CONTRACT; the
    // atomicity itself is asserted at the adapter layer.
    let router = composed_router_with_failing_outbox();
    let (status, body) = call(
        &router,
        post_json(
            "/api/v1/flexibility/envelopes",
            Some("caller-a"),
            envelope_body(
                &Chain {
                    portfolio_id: FIXTURE_PORTFOLIO_ID.to_string(),
                    site_id: FIXTURE_SITE_ID.to_string(),
                    site_grid_model_reference: "grid-model-alpha-v1".to_string(),
                },
                "env-v1",
                vec![FIXTURE_SITE_ID.to_string()],
            ),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(body["error"]["code"], "infrastructure_failure");
    assert!(
        body["error"]["message"].as_str().expect("opaque message").is_empty()
            || !body["error"]["message"].as_str().expect("opaque").contains("sql"),
        "no storage text leaks into the error envelope"
    );
}

#[tokio::test]
async fn the_surface_has_no_execute_command_operation() {
    // §15.2: NO "execute arbitrary command" endpoint. Two structural checks:
    // (1) the GENERATED OpenAPI path set (generated from the route
    // annotations, so a new route appears here) contains no
    // command/execute/dispatch/approve operation; (2) the forbidden paths
    // answer 404 live.
    let (router, _outbox, _journal, _records, _revisions, _envelopes) = composed_phase3_router();
    let spec = openapi_spec(&router).await;
    let paths = spec["paths"]
        .as_object()
        .expect("the generated spec carries paths");
    assert!(
        paths.contains_key("/api/v1/flexibility/envelopes")
            && paths.contains_key("/api/v1/schedules")
            && paths.contains_key("/api/v1/optimization-runs"),
        "the three Phase 3 resources are registered"
    );
    for path in paths.keys() {
        let lowered = path.to_lowercase();
        for banned in ["command", "execute", "dispatch", "approve", "approval"] {
            assert!(
                !lowered.contains(banned),
                "the API surface must never expose a {banned} operation (found {path})"
            );
        }
        // The Phase 3 resources expose exactly get + post — no PUT/PATCH/
        // DELETE lifecycle mutation and no action sub-resource.
        if matches!(
            path.as_str(),
            "/api/v1/flexibility/envelopes" | "/api/v1/schedules" | "/api/v1/optimization-runs"
        ) {
            let operations = paths[path]
                .as_object()
                .expect("each path carries operation objects");
            let mut ops: Vec<&String> = operations.keys().collect();
            ops.sort();
            assert_eq!(ops.len(), 2, "exactly two operations on {path}");
            assert!(
                operations.contains_key("get") && operations.contains_key("post"),
                "the Phase 3 resources are get+post only"
            );
        }
    }
    // Live probes: the forbidden routes do not exist.
    for (method_uri, token) in [
        ("/api/v1/commands", "caller-a"),
        ("/api/v1/commands/0192a5e0-0000-7000-8000-0000000000c1/execute", "caller-a"),
        ("/api/v1/dispatch", "caller-a"),
        ("/api/v1/schedules/execute", "caller-a"),
    ] {
        let (status, _) = call(
            &router,
            post_json(method_uri, Some(token), json!({"anything": true})),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND, "no execute route at {method_uri}");
    }
    // The frozen event names this surface emits are exactly the three
    // §15.3 Phase 3 events — no Dispatch* event is ever assembled here.
    assert_eq!(
        flexcore_control_plane::routes_phase3::emitted_event_names(),
        ["FlexibilityEnvelopeUpdated", "ScheduleProposed", "ScheduleValidated"]
    );
}

#[tokio::test]
async fn outbox_rows_always_carry_the_supported_schema_version() {
    // Every emitted outbox row's serialized envelope parses as a
    // schema_version 1 envelope (P1.09: an unknown version never parses —
    // the parse itself is the schema check).
    let (router, outbox, _journal, _records, _revisions, _envelopes) = composed_phase3_router();
    let chain = create_chain(&router).await;
    register_run(&router, &chain, CANDIDATE_1).await;
    call(
        &router,
        post_json(
            "/api/v1/flexibility/envelopes",
            Some("caller-a"),
            envelope_body(&chain, "env-v1", vec![chain.site_id.clone()]),
        ),
    )
    .await;
    call(
        &router,
        post_json("/api/v1/schedules", Some("caller-a"), schedule_body(&chain, CANDIDATE_1)),
    )
    .await;
    let mut writes = Vec::new();
    for entity in ["flexibility_envelopes", "schedule_results", "schedule_revisions"] {
        writes.extend(phase3_writes(&outbox, entity));
    }
    assert_eq!(writes.len(), 4, "1 run + 1 envelope(1 site) + 1 schedule = 4 rows");
    for write in &writes {
        let payload: Value = serde_json::from_str(write.payload())
            .expect("every outbox payload is the serialized event envelope");
        assert_eq!(payload["schema_version"], SUPPORTED_SCHEMA_VERSION);
        assert_eq!(payload["idempotency_key"], write.idempotency_key());
        // The receipt pairing: the write is bound to the state change of the
        // record it accompanies (the §15.5 rule, carried on the write).
        assert_eq!(
            write.state_change().operation(),
            StateChangeOperation::Insert
        );
        assert!(write.created_at().unix_millis() > 0, "the event time is carried");
    }
}
