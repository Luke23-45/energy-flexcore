// P4.30 — Phase 4 API contract tests (§15.2, §16.1, §21.3; ledger P4.30
//         step 3): the dispatch-command and feasibility-check READ
//         resources over the Test-variant backend.
//
// Composition rule (no network, no database — the P1.17 test pattern): the
// app state is built with the flexcore-ports fakes plus the in-crate Phase
// 4 record-store fakes (`phase4_fakes`), a fixed identity resolver, and
// the cumulative Phase 4 policy set. Records are seeded DIRECTLY into the
// fakes (there is NO POST route on either resource — §15.2), and requests
// go through the real router via `tower::ServiceExt::oneshot`.
//
// Recorded assertions covered here (the ledger P4.30 acceptance list):
// - **OpenAPI-shape validation**: every 200 response body is validated
//   against the GENERATED OpenAPI document (components.schemas — the spec
//   is generated from the route annotations, never hand-written).
// - **Least-privilege actions**: the resources authorize `command.read`
//   and `feasibility.read` (deny-by-default; an empty policy set denies
//   with 403; unauthenticated is 401).
// - **Tenant scoping with no existence leak**: another tenant's records
//   are absent from listings and a by-id read of a foreign-tenant record
//   is 404.
// - **Correlation fields (§21.3)**: the served records carry the tenant/
//   portfolio/site/asset scope, the schedule + command identities, the
//   model id/version/hash, the actor identity, and the correlation id.
// - **READ-ONLY surface (§15.2)**: the generated OpenAPI registers the two
//   resources with GET operations ONLY, live POST probes are refused, and
//   the no-execute + simulation-only statements travel in the responses.
// - **Keyset listing discipline**: default/maximum page size, malformed
//   cursor and filter values are 422 (never a silent "first page").

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use flexcore_authz::action::Action as A;
use flexcore_authz::policy::{PolicySet, Role};
use flexcore_contracts::ids::FlexUuid;
use flexcore_control_plane::auth::{AuthError, Caller, IdentityResolver, phase4_default_policies};
use flexcore_control_plane::dto_phase4::{
    DISPATCH_SIMULATION_ONLY_STATEMENT, NO_EXECUTE_ENDPOINT_STATEMENT,
};
use flexcore_control_plane::phase4_fakes::{
    FakeCommandRecordStore, FakeFeasibilityCheckStore,
};
use flexcore_control_plane::probes::Readiness;
use flexcore_control_plane::routes::stage1_router;
use flexcore_control_plane::state::AppState;
use flexcore_domain::command_lifecycle::{Actor, CommandTransition, EvidenceReference, Gate};
use flexcore_domain::command_state::CommandState;
use flexcore_domain::ids::{CommandId, TenantId};
use flexcore_domain::tenant_context::{AuthenticatedClaims, TenantContext, VerifiedClaims};
use flexcore_storage::command_store::{
    CommandListFilter, CommandRecordRow, CommandRecordStore, CommandTransitionRecord,
};
use flexcore_storage::feasibility_check::{FeasibilityCheckRow, FeasibilityCheckStore};
use flexcore_ports::fakes::{
    CountingIdGenerator, FakeAssetRepository, FakeEventOutbox, FakePortfolioRepository,
    FakeSiteRepository, FakeTenantRepository, FakeWriteJournal, FixedClock,
};
use serde_json::{Value, json};
use tower::ServiceExt;

const TENANT_A_SEED: u128 = 1;
const TENANT_B_SEED: u128 = 2;

fn tenant_id(seed: u128) -> TenantId {
    TenantId::new(FlexUuid::from_bytes(seed.to_be_bytes()))
}

// Fixed canonical UUIDs (canonical UUIDv7-shaped text; byte-ordered so the
// keyset page order is predictable: command 1 < command 2).
const COMMAND_1: &str = "0192a5e0-0000-7000-8000-000000000001";
const COMMAND_2: &str = "0192a5e0-0000-7000-8000-000000000002";
const COMMAND_B: &str = "0192a5e0-0000-7000-8000-0000000000b1";
const PORTFOLIO_A: &str = "0192a5e0-0000-7000-8000-0000000000a1";
const SITE_A: &str = "0192a5e0-0000-7000-8000-0000000000a2";
const ASSET_A: &str = "0192a5e0-0000-7000-8000-0000000000a3";
const SCHEDULE_A: &str = "0192a5e0-0000-7000-8000-0000000000a4";
const PORTFOLIO_B: &str = "0192a5e0-0000-7000-8000-0000000000b2";
const SITE_B: &str = "0192a5e0-0000-7000-8000-0000000000b3";
const ASSET_B: &str = "0192a5e0-0000-7000-8000-0000000000b4";
const SCHEDULE_B: &str = "0192a5e0-0000-7000-8000-0000000000b5";

// ---------------------------------------------------------------------------
// Identity + composition (the P1.17 test pattern, extended with the Phase 4
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

/// The full Phase 4 Test composition: router + shared outbox + journal +
/// the two Phase 4 record-store fakes (for direct seeding + inspection).
#[allow(clippy::type_complexity)]
fn composed_phase4_router_with_policies(
    policies: PolicySet,
) -> (
    Router,
    Arc<FakeEventOutbox>,
    FakeWriteJournal,
    Arc<FakeCommandRecordStore>,
    Arc<FakeFeasibilityCheckStore>,
) {
    let journal = FakeWriteJournal::new();
    let outbox = Arc::new(FakeEventOutbox::new(journal.clone()));
    let command_records = Arc::new(FakeCommandRecordStore::new());
    let feasibility_checks = Arc::new(FakeFeasibilityCheckStore::new());
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
        outbox.clone(),
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
        command_records.clone(),
        feasibility_checks.clone(),
    );
    (
        stage1_router(state),
        outbox,
        journal,
        command_records,
        feasibility_checks,
    )
}

fn composed_phase4_router() -> (
    Router,
    Arc<FakeEventOutbox>,
    FakeWriteJournal,
    Arc<FakeCommandRecordStore>,
    Arc<FakeFeasibilityCheckStore>,
) {
    composed_phase4_router_with_policies(phase4_default_policies())
}

// ---------------------------------------------------------------------------
// HTTP helpers (the P3.28 test pattern)
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

/// Fetches the generated OpenAPI document (the spec endpoint is
/// unauthenticated by design: it is a contract artifact, not tenant data).
async fn openapi_spec(router: &Router) -> Value {
    let (status, body) = call(router, get("/api/v1/openapi.json", None)).await;
    assert_eq!(status, StatusCode::OK, "the OpenAPI spec is reachable");
    body
}

// ---------------------------------------------------------------------------
// OpenAPI-shape validation (the api_phase3 helpers, reused verbatim)
// ---------------------------------------------------------------------------

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

fn ref_name(reference: &str) -> &str {
    reference
        .rsplit('/')
        .next()
        .expect("a $ref always has a name segment")
}

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

fn validate_envelope_response(spec: &Value, item: &str, body: &Value, at: &str) {
    let (name, schema) = schema(spec, &format!("DataEnvelope_{item}"));
    let data_ref = schema["properties"]["data"]["$ref"]
        .as_str()
        .unwrap_or_else(|| panic!("{at}: {name} does not reference the {item} item schema"));
    validate_against_schema(spec, ref_name(data_ref), &body["data"], at);
}

fn validate_page_response(spec: &Value, page: &str, item: &str, body: &Value, at: &str) {
    let (_, schema) = schema(spec, page);
    let items_ref = schema["properties"]["data"]["items"]["$ref"]
        .as_str()
        .unwrap_or_else(|| panic!("{at}: {page} does not reference the {item} item schema"));
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
}

// ---------------------------------------------------------------------------
// Seeding (directly into the fakes — there is NO POST route on either
// resource, §15.2)
// ---------------------------------------------------------------------------

fn ctx_for(seed: u128) -> TenantContext {
    TenantContext::from_authenticated_claims(AuthenticatedClaims::from_verified(
        VerifiedClaims {
            tenant_id: tenant_id(seed),
            subject: "p4.30-api-test".to_string(),
        },
    ))
}

/// Seeds one command record for a tenant scope, optionally walking it to
/// FEASIBLE (a VALIDATED + FEASIBLE journal pair through the storage
/// constructors, which enforce the frozen-edge + evidence rules).
fn seed_command(
    store: &FakeCommandRecordStore,
    tenant_seed: u128,
    command_id: &str,
    portfolio: &str,
    site: &str,
    asset: &str,
    schedule: &str,
    walk_to_feasible: bool,
) {
    let ctx = ctx_for(tenant_seed);
    let row = CommandRecordRow::new(
        tenant_id(tenant_seed),
        command_id.to_string(),
        portfolio.to_string(),
        site.to_string(),
        asset.to_string(),
        schedule.to_string(),
        1,
        1_700_000_200_000,
        None,
        "{\"target_p_mw\":1.5,\"units\":\"MW\",\"sign\":\"export_positive\"}".to_string(),
    )
    .expect("the command row builds");
    CommandRecordStore::insert_command_record(store, &ctx, &row)
        .expect("the command inserts");
    if walk_to_feasible {
        let command = CommandId::new(FlexUuid::parse(command_id).expect("canonical"));
        for (index, (from, to, gate, evidence)) in [
            (
                CommandState::Proposed,
                CommandState::Validated,
                Some(Gate::Validation),
                "evidence:validation-v1",
            ),
            (
                CommandState::Validated,
                CommandState::Feasible,
                Some(Gate::Feasibility),
                "feasibility:attempt/attempt-1:model/feeder-a:hash/fnv1a64:deadbeef",
            ),
        ]
        .into_iter()
        .enumerate()
        {
            let transition = CommandTransition {
                command_id: command,
                from,
                to,
                actor: Actor::new("svc:validator").expect("actor builds"),
                at_unix_millis: 1_700_000_100_000 + index as i64 * 1_000,
                gate,
                reason: None,
                evidence: Some(
                    EvidenceReference::new(evidence).expect("evidence builds"),
                ),
            };
            let record = CommandTransitionRecord::from_transition(tenant_id(tenant_seed), &transition)
                .expect("the transition record builds (the edge is in the frozen table)");
            CommandRecordStore::record_command_transition(store, &ctx, &record)
                .expect("the transition commits");
        }
    }
}

/// Seeds one feasibility-check evidence record for a tenant scope.
#[allow(clippy::too_many_arguments)]
fn seed_check(
    store: &FakeFeasibilityCheckStore,
    tenant_seed: u128,
    check_id: &str,
    command_id: Option<&str>,
    portfolio: &str,
    site: &str,
    asset: &str,
    schedule: &str,
    outcome: &str,
    checked_at: i64,
) {
    let ctx = ctx_for(tenant_seed);
    let row = FeasibilityCheckRow::new(
        tenant_id(tenant_seed),
        check_id.to_string(),
        command_id.map(|id| id.to_string()),
        portfolio.to_string(),
        site.to_string(),
        asset.to_string(),
        schedule.to_string(),
        1,
        Some("feeder-a".to_string()),
        Some("grid-model-v1".to_string()),
        Some("fnv1a64:deadbeef".to_string()),
        outcome.to_string(),
        None,
        None,
        Some("margins within tolerance".to_string()),
        Some("balanced radial feeder".to_string()),
        "svc:grid-gate".to_string(),
        format!("trace-{check_id}"),
        checked_at,
    )
    .expect("the check row builds");
    FeasibilityCheckStore::insert_feasibility_check(store, &ctx, &row)
        .expect("the check inserts");
}

// ---------------------------------------------------------------------------
// Tests — authorization (least-privilege actions, 401/403/404)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn unauthenticated_requests_are_401() {
    let (router, _outbox, _journal, _commands, _checks) = composed_phase4_router();
    let uris = [
        "/api/v1/dispatch-commands".to_string(),
        format!("/api/v1/dispatch-commands/{COMMAND_1}"),
        "/api/v1/feasibility-checks".to_string(),
        "/api/v1/feasibility-checks/attempt-1".to_string(),
    ];
    for uri in &uris {
        let (status, _) = call(&router, get(uri, None)).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "no bearer at {uri}");
    }
}

#[tokio::test]
async fn a_deny_by_default_policy_set_is_403() {
    // The empty PolicySet denies everything (deny-by-default; the P1.11
    // rule): the Phase 4 read actions are granted explicitly, never by
    // default.
    let (router, _outbox, _journal, commands, checks) =
        composed_phase4_router_with_policies(PolicySet::new());
    seed_command(&commands, TENANT_A_SEED, COMMAND_1, PORTFOLIO_A, SITE_A, ASSET_A, SCHEDULE_A, false);
    seed_check(&checks, TENANT_A_SEED, "attempt-1", Some(COMMAND_1), PORTFOLIO_A, SITE_A, ASSET_A, SCHEDULE_A, "PASSED", 1_700_000_150_000);
    for uri in [
        "/api/v1/dispatch-commands",
        "/api/v1/feasibility-checks",
    ] {
        let (status, body) = call(&router, get(uri, Some("caller-a"))).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "no grant at {uri}");
        assert!(
            body["error"]["message"]
                .as_str()
                .unwrap_or_default()
                .contains("denied"),
            "the denial names the action"
        );
    }
}

#[tokio::test]
async fn the_read_actions_are_the_least_privilege_names() {
    // The resources authorize the two recorded Phase 4 read actions by
    // id — command.read and feasibility.read (the dispatch surface has NO
    // write action on this API; the gate actions live at the gates).
    assert_eq!(A::CommandRead.id(), "command.read");
    assert_eq!(A::FeasibilityRead.id(), "feasibility.read");
    // And the §5.7 plugin capability names remain off the surface.
    assert_eq!(A::parse("dispatch.authorize"), None);
    assert_eq!(A::parse("dispatch.issue"), None);
    assert_eq!(A::parse("command.execute"), None);
}

// ---------------------------------------------------------------------------
// Tests — dispatch-command reads
// ---------------------------------------------------------------------------

#[tokio::test]
async fn dispatch_command_list_is_tenant_scoped_newest_first_and_schema_valid() {
    let (router, _outbox, _journal, commands, _checks) = composed_phase4_router();
    seed_command(&commands, TENANT_A_SEED, COMMAND_1, PORTFOLIO_A, SITE_A, ASSET_A, SCHEDULE_A, false);
    seed_command(&commands, TENANT_A_SEED, COMMAND_2, PORTFOLIO_A, SITE_A, ASSET_A, SCHEDULE_A, true);
    seed_command(&commands, TENANT_B_SEED, COMMAND_B, PORTFOLIO_B, SITE_B, ASSET_B, SCHEDULE_B, false);

    let spec = openapi_spec(&router).await;
    let (status, body) = call(&router, get("/api/v1/dispatch-commands", Some("caller-a"))).await;
    assert_eq!(status, StatusCode::OK);
    let ids: Vec<&str> = body["data"]
        .as_array()
        .expect("page data array")
        .iter()
        .map(|c| c["command_id"].as_str().expect("command id"))
        .collect();
    assert_eq!(
        ids,
        vec![COMMAND_2, COMMAND_1],
        "only the caller's tenant's commands, newest first (keyset over UUIDv7 ids)"
    );
    validate_page_response(&spec, "DispatchCommandPageEnvelope", "DispatchCommandDto", &body, "list");

    // The correlation (§21.3) and the recorded statements travel.
    let first = &body["data"][0];
    assert_eq!(first["tenant_id"], tenant_id(TENANT_A_SEED).uuid().to_hyphenated().to_string());
    assert_eq!(first["portfolio_id"], PORTFOLIO_A);
    assert_eq!(first["site_id"], SITE_A);
    assert_eq!(first["asset_id"], ASSET_A);
    assert_eq!(first["schedule_id"], SCHEDULE_A);
    assert_eq!(first["state"], "FEASIBLE");
    assert_eq!(
        first["no_execute_endpoint_statement"].as_str().expect("no-execute statement"),
        NO_EXECUTE_ENDPOINT_STATEMENT
    );
    assert_eq!(
        first["simulation_only_statement"].as_str().expect("simulation-only statement"),
        DISPATCH_SIMULATION_ONLY_STATEMENT
    );

    // Caller B sees only its own command — cross-tenant records do not
    // leak into listings.
    let (status, body_b) = call(&router, get("/api/v1/dispatch-commands", Some("caller-b"))).await;
    assert_eq!(status, StatusCode::OK);
    let ids_b: Vec<&str> = body_b["data"]
        .as_array()
        .expect("page data array")
        .iter()
        .map(|c| c["command_id"].as_str().expect("command id"))
        .collect();
    assert_eq!(ids_b, vec![COMMAND_B]);
}

#[tokio::test]
async fn dispatch_command_detail_carries_the_full_journal_and_cross_tenant_is_404() {
    let (router, _outbox, _journal, commands, _checks) = composed_phase4_router();
    seed_command(&commands, TENANT_A_SEED, COMMAND_1, PORTFOLIO_A, SITE_A, ASSET_A, SCHEDULE_A, true);

    let spec = openapi_spec(&router).await;
    let uri = format!("/api/v1/dispatch-commands/{COMMAND_1}");
    let (status, body) = call(&router, get(&uri, Some("viewer-a"))).await;
    assert_eq!(status, StatusCode::OK, "a Viewer may read (command.read)");
    validate_envelope_response(&spec, "DispatchCommandDetailDto", &body, "detail");
    let journal = body["data"]["journal"]
        .as_array()
        .expect("the journal array");
    assert_eq!(journal.len(), 2, "every journalled transition is served");
    assert_eq!(journal[0]["from_state"], "PROPOSED");
    assert_eq!(journal[0]["to_state"], "VALIDATED");
    assert_eq!(journal[0]["gate"], "validation");
    assert_eq!(journal[0]["evidence_reference"], "evidence:validation-v1");
    assert_eq!(journal[1]["to_state"], "FEASIBLE");
    assert_eq!(journal[1]["gate"], "feasibility");
    assert!(
        journal[1]["evidence_reference"]
            .as_str()
            .expect("feasibility evidence")
            .starts_with("feasibility:attempt/attempt-1:model/feeder-a:hash/"),
        "the feasibility evidence binds THE MODEL HASH (§17.5 invariant 3)"
    );
    assert_eq!(body["data"]["command"]["state"], "FEASIBLE");

    // Cross-tenant by-id read: 404, no existence leak (P0.16 rule 5).
    let (status, _) = call(&router, get(&uri, Some("caller-b"))).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn malformed_state_filter_cursor_and_limit_are_422() {
    let (router, _outbox, _journal, _commands, _checks) = composed_phase4_router();
    for uri in [
        "/api/v1/dispatch-commands?state=SUPERCEDED",
        "/api/v1/dispatch-commands?cursor=not-a-uuid",
        "/api/v1/dispatch-commands?limit=1001",
        "/api/v1/dispatch-commands?limit=0",
        // The feasibility-check filters are UUID-validated too.
        "/api/v1/feasibility-checks?command_id=not-a-uuid",
        "/api/v1/feasibility-checks?limit=1001",
    ] {
        let (status, _) = call(&router, get(uri, Some("caller-a"))).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "malformed {uri}");
    }
}

// ---------------------------------------------------------------------------
// Tests — feasibility-check reads
// ---------------------------------------------------------------------------

#[tokio::test]
async fn feasibility_check_list_and_detail_are_schema_valid_and_tenant_scoped() {
    let (router, _outbox, _journal, _commands, checks) = composed_phase4_router();
    seed_check(&checks, TENANT_A_SEED, "attempt-1", Some(COMMAND_1), PORTFOLIO_A, SITE_A, ASSET_A, SCHEDULE_A, "PASSED", 1_700_000_150_000);
    seed_check(&checks, TENANT_A_SEED, "attempt-2", None, PORTFOLIO_A, SITE_A, ASSET_A, SCHEDULE_A, "LIMIT_VIOLATED", 1_700_000_160_000);
    seed_check(&checks, TENANT_B_SEED, "attempt-b1", Some(COMMAND_B), PORTFOLIO_B, SITE_B, ASSET_B, SCHEDULE_B, "PASSED", 1_700_000_150_000);

    let spec = openapi_spec(&router).await;
    let (status, body) = call(&router, get("/api/v1/feasibility-checks", Some("caller-a"))).await;
    assert_eq!(status, StatusCode::OK);
    let ids: Vec<&str> = body["data"]
        .as_array()
        .expect("page data array")
        .iter()
        .map(|c| c["check_id"].as_str().expect("check id"))
        .collect();
    assert_eq!(
        ids,
        vec!["attempt-1", "attempt-2"],
        "only the caller's tenant's checks, in completion order"
    );
    validate_page_response(&spec, "FeasibilityCheckPageEnvelope", "FeasibilityCheckDto", &body, "list");

    // The correlation (§21.3): scope + command + schedule + model binding
    // + actor + correlation id, and the no-execute statement.
    let first = &body["data"][0];
    assert_eq!(first["command_id"], COMMAND_1);
    assert_eq!(first["schedule_id"], SCHEDULE_A);
    assert_eq!(first["model_id"], "feeder-a");
    assert_eq!(first["model_content_hash"], "fnv1a64:deadbeef");
    assert_eq!(first["outcome"], "PASSED");
    assert_eq!(first["actor"], "svc:grid-gate");
    assert_eq!(first["correlation_id"], "trace-attempt-1");
    assert_eq!(
        first["no_execute_endpoint_statement"].as_str().expect("no-execute statement"),
        NO_EXECUTE_ENDPOINT_STATEMENT
    );

    // The command filter scopes the listing.
    let (status, filtered) = call(
        &router,
        get(&format!("/api/v1/feasibility-checks?command_id={COMMAND_1}"), Some("caller-a")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(filtered["data"].as_array().expect("array").len(), 1);

    // Detail: by id in-tenant; cross-tenant is 404 — no existence leak.
    let (status, detail) = call(&router, get("/api/v1/feasibility-checks/attempt-1", Some("caller-a"))).await;
    assert_eq!(status, StatusCode::OK);
    validate_envelope_response(&spec, "FeasibilityCheckDto", &detail, "detail");
    assert_eq!(detail["data"]["check_id"], "attempt-1");
    let (status, _) = call(&router, get("/api/v1/feasibility-checks/attempt-b1", Some("caller-a"))).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = call(&router, get("/api/v1/feasibility-checks/attempt-1", Some("caller-b"))).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ---------------------------------------------------------------------------
// Tests — the READ-ONLY surface (§15.2)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn the_phase4_resources_are_read_only_with_no_execute_operation() {
    // §15.2: NO "execute arbitrary command" endpoint. Three structural
    // checks: (1) the GENERATED OpenAPI registers exactly the two Phase 4
    // resources; (2) they carry GET operations ONLY (no POST/PUT/PATCH/
    // DELETE — the transitions are the application gates' alone); (3) the
    // live POST probes are refused (405 on a registered path, 404 off it).
    let (router, _outbox, _journal, _commands, _checks) = composed_phase4_router();
    let spec = openapi_spec(&router).await;
    let paths = spec["paths"]
        .as_object()
        .expect("the generated spec carries paths");
    assert!(
        paths.contains_key("/api/v1/dispatch-commands")
            && paths.contains_key("/api/v1/feasibility-checks"),
        "the two Phase 4 resources are registered"
    );
    for path in [
        "/api/v1/dispatch-commands",
        "/api/v1/feasibility-checks",
    ] {
        let operations = paths[path]
            .as_object()
            .expect("each path carries operation objects");
        assert!(
            operations.contains_key("get") && operations.len() == 1,
            "{path} is GET-only (found {operations:?})"
        );
    }
    // Live probes: POST on the resources is 405 (the route exists, the
    // method does not); POST to execute-shaped paths is 404 (no route).
    let (status, _) = call(
        &router,
        post_json("/api/v1/dispatch-commands", Some("caller-a"), json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED, "no POST on the resource");
    let (status, _) = call(
        &router,
        post_json(
            &format!("/api/v1/dispatch-commands/{COMMAND_1}/execute"),
            Some("caller-a"),
            json!({}),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "no execute route");
    let (status, _) = call(
        &router,
        post_json("/api/v1/feasibility-checks", Some("caller-a"), json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED, "no POST on the resource");
}

// ---------------------------------------------------------------------------
// P4.36 — audit-events read resource (additive: the tests above are the
//         P4.30 dispatch-command / feasibility-check contracts, untouched)
// ---------------------------------------------------------------------------

use flexcore_control_plane::dto_phase4::AUDIT_APPEND_ONLY_STATEMENT;
use flexcore_control_plane::phase4_fakes::FakeAuditEventReader;
use flexcore_storage::audit_read::{AuditEventRow, AUDIT_DECISION_ALLOWED, AUDIT_DECISION_DENIED};

// Fixed canonical audit-record ids (canonical UUIDv7-shaped text; byte-
// ordered so the (event_time, audit_id) page order is predictable).
const AUDIT_1: &str = "0192a5e1-0000-7000-8000-000000000001";
const AUDIT_2: &str = "0192a5e1-0000-7000-8000-000000000002";
const AUDIT_3: &str = "0192a5e1-0000-7000-8000-000000000003";
const AUDIT_B: &str = "0192a5e1-0000-7000-8000-0000000000b1";
const AUDIT_CORRECTION: &str = "0192a5e1-0000-7000-8000-0000000000c1";
const AUDIT_OBJECT: &str = "0192a5e0-0000-7000-8000-0000000000a5";

/// The full Phase 4 Test composition for the audit-events read resource
/// (P4.36): the same pattern as `composed_phase4_router_with_policies`,
/// with the audit-events fake handle returned for direct seeding (the
/// read port has NO write method — seeding is a fake-only test helper).
fn composed_audit_router_with_policies(
    policies: PolicySet,
) -> (Router, Arc<FakeAuditEventReader>) {
    let journal = FakeWriteJournal::new();
    let outbox = Arc::new(FakeEventOutbox::new(journal.clone()));
    let audit_events = Arc::new(FakeAuditEventReader::new());
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
    let state = AppState::for_tests_with_phase4_audit(
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
        Arc::new(FakeCommandRecordStore::new()),
        Arc::new(FakeFeasibilityCheckStore::new()),
        audit_events.clone(),
    );
    (stage1_router(state), audit_events)
}

fn composed_audit_router() -> (Router, Arc<FakeAuditEventReader>) {
    composed_audit_router_with_policies(phase4_default_policies())
}

/// Seeds one audit-event row directly into the fake (the port itself has
/// no write method — the append-only rule holds at the API layer; the
/// real write path is the P1.10 AuditSink, exercised in the P4.36
/// DB-gated adapter tests).
#[allow(clippy::too_many_arguments)]
fn seed_audit_event(
    store: &FakeAuditEventReader,
    tenant_seed: u128,
    audit_id: &str,
    actor: &str,
    action: &str,
    object_type: &str,
    object_id: &str,
    decision_outcome: Option<&str>,
    decision_reason: Option<&str>,
    evidence: Option<&str>,
    event_millis: i64,
) {
    let row = AuditEventRow::new(
        audit_id.to_string(),
        Some(tenant_id(tenant_seed)),
        actor.to_string(),
        action.to_string(),
        object_type.to_string(),
        object_id.to_string(),
        None,
        Some(format!("{object_type}:{object_id}:{action}")),
        decision_outcome.map(str::to_string),
        decision_reason.map(str::to_string),
        evidence.map(str::to_string),
        event_millis,
        event_millis + 500,
        format!("trace-{audit_id}"),
        None,
    )
    .expect("the audit row builds");
    store.seed(row);
}

#[tokio::test]
async fn audit_events_unauthenticated_is_401_and_deny_by_default_is_403() {
    // 401: no credentials on either audit route.
    let (router, _audit) = composed_audit_router();
    for uri in [
        "/api/v1/audit-events".to_string(),
        format!("/api/v1/audit-events/{AUDIT_1}"),
    ] {
        let (status, _) = call(&router, get(&uri, None)).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "no bearer at {uri}");
    }
    // 403: the empty policy set denies everything (deny-by-default) —
    // `audit.event.read` is granted explicitly, never by default.
    let (router, _audit) = composed_audit_router_with_policies(PolicySet::new());
    for uri in [
        "/api/v1/audit-events".to_string(),
        format!("/api/v1/audit-events/{AUDIT_1}"),
    ] {
        let (status, body) = call(&router, get(&uri, Some("caller-a"))).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "no grant at {uri}");
        assert!(
            body["error"]["message"]
                .as_str()
                .unwrap_or_default()
                .contains("denied"),
            "the denial names the action at {uri}"
        );
    }
}

#[tokio::test]
async fn the_audit_read_action_is_the_least_privilege_name_with_no_write_sibling() {
    // The resource authorizes exactly `audit.event.read` (P4.36): the
    // read grant exists, and NO write/delete/export action exists on the
    // surface at all — the trail is append-only (the P1.10 port rule,
    // applied at the API layer; §15.2, §16.2).
    assert_eq!(A::AuditEventRead.id(), "audit.event.read");
    assert_eq!(A::parse("audit.event.read"), Some(A::AuditEventRead));
    // The append-only sibling actions that must never exist.
    assert_eq!(A::parse("audit.event.write"), None);
    assert_eq!(A::parse("audit.event.delete"), None);
    assert_eq!(A::parse("audit.event.export"), None);
    assert_eq!(A::parse("audit.event.append"), None);
    // Near-misses parse to nothing (deny-by-default, P1.11).
    assert_eq!(A::parse("audit.events.read"), None);
    assert_eq!(A::parse("Audit.Event.Read"), None);
}

#[tokio::test]
async fn audit_event_list_and_detail_are_schema_valid_tenant_scoped_and_fully_correlated() {
    let (router, audit) = composed_audit_router();
    // Tenant A: an approval decision (with its reason + evidence
    // references), a plain registration, an off-linkage correction, and a
    // correction row WITH its linkage to the first record (a NEW record —
    // the original is never edited).
    seed_audit_event(
        &audit,
        TENANT_A_SEED,
        AUDIT_1,
        "user:operator-1",
        "command.approve",
        "command",
        AUDIT_OBJECT,
        Some(AUDIT_DECISION_ALLOWED),
        Some("approval-record:apr-1"),
        Some("evidence:approval/apr-1"),
        1_700_000_100_000,
    );
    seed_audit_event(
        &audit,
        TENANT_A_SEED,
        AUDIT_2,
        "svc:registry",
        "asset.register",
        "asset",
        "asset-1",
        None,
        None,
        None,
        1_700_000_050_000,
    );
    seed_audit_event(
        &audit,
        TENANT_A_SEED,
        AUDIT_3,
        "svc:audit-worker",
        "audit.correct",
        "audit_record",
        AUDIT_1,
        None,
        None,
        None,
        1_700_000_200_000,
    );
    audit.seed(
        AuditEventRow::new(
            AUDIT_CORRECTION.to_string(),
            Some(tenant_id(TENANT_A_SEED)),
            "svc:audit-worker".to_string(),
            "audit.correct".to_string(),
            "audit_record".to_string(),
            AUDIT_1.to_string(),
            None,
            None,
            None,
            None,
            None,
            1_700_000_300_000,
            1_700_000_300_500,
            format!("trace-{AUDIT_CORRECTION}"),
            Some(AUDIT_1.to_string()),
        )
        .expect("the correction row builds"),
    );
    // Tenant B: one record that must never leak.
    seed_audit_event(
        &audit,
        TENANT_B_SEED,
        AUDIT_B,
        "user:admin-b",
        "command.approve",
        "command",
        AUDIT_OBJECT,
        Some(AUDIT_DECISION_DENIED),
        Some("policy-denial:pol-1"),
        None,
        1_700_000_100_000,
    );

    let spec = openapi_spec(&router).await;
    let (status, body) = call(&router, get("/api/v1/audit-events", Some("caller-a"))).await;
    assert_eq!(status, StatusCode::OK);
    let ids: Vec<&str> = body["data"]
        .as_array()
        .expect("page data array")
        .iter()
        .map(|r| r["audit_id"].as_str().expect("audit id"))
        .collect();
    assert_eq!(
        ids,
        vec![AUDIT_2, AUDIT_1, AUDIT_3, AUDIT_CORRECTION],
        "only the caller's tenant's records, in (event_time, audit_id) order"
    );
    validate_page_response(&spec, "AuditEventPageEnvelope", "AuditEventDto", &body, "audit list");

    // The §21.3 correlation: actor (an identity reference), the audited
    // object pair, the decision (outcome + reason reference, never a bare
    // allow), the evidence reference, the distinct P0.09 timestamps, the
    // trace id, and the append-only statement.
    let approval = body["data"]
        .as_array()
        .expect("array")
        .iter()
        .find(|r| r["audit_id"] == AUDIT_1)
        .expect("the approval record is served")
        .clone();
    assert_eq!(
        approval["tenant_id"],
        tenant_id(TENANT_A_SEED).uuid().to_hyphenated().to_string()
    );
    assert_eq!(approval["actor"], "user:operator-1");
    assert_eq!(approval["action"], "command.approve");
    assert_eq!(approval["object_type"], "command");
    assert_eq!(approval["object_id"], AUDIT_OBJECT);
    assert_eq!(approval["decision_outcome"], AUDIT_DECISION_ALLOWED);
    assert_eq!(approval["decision_reason_reference"], "approval-record:apr-1");
    assert_eq!(approval["evidence"], "evidence:approval/apr-1");
    assert_eq!(approval["event_time_unix_millis"], 1_700_000_100_000);
    assert_eq!(approval["ingest_time_unix_millis"], 1_700_000_100_500);
    assert_eq!(approval["correlation_id"], format!("trace-{AUDIT_1}"));
    assert_eq!(
        approval["append_only_statement"].as_str().expect("statement"),
        AUDIT_APPEND_ONLY_STATEMENT
    );
    // No secrets, no credentials: the served fields are references and
    // identities only (§21.3) — structural check over the served record.
    let served_text = approval.to_string().to_lowercase();
    assert!(
        !served_text.contains("password")
            && !served_text.contains("secret")
            && !served_text.contains("token="),
        "no secret material in the served audit record"
    );

    // The correction linkage survives the read: a NEW record referencing
    // the original (P1.10 step 3).
    let correction = body["data"]
        .as_array()
        .expect("array")
        .iter()
        .find(|r| r["audit_id"] == AUDIT_CORRECTION)
        .expect("the correction record is served")
        .clone();
    assert_eq!(correction["corrected_audit_id"], AUDIT_1);

    // The action (event class) filter scopes the listing.
    let (status, filtered) = call(
        &router,
        get("/api/v1/audit-events?action=command.approve", Some("caller-a")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let filtered_ids: Vec<&str> = filtered["data"]
        .as_array()
        .expect("array")
        .iter()
        .map(|r| r["audit_id"].as_str().expect("audit id"))
        .collect();
    assert_eq!(filtered_ids, vec![AUDIT_1], "the class filter scopes the listing");

    // The INCLUSIVE time window (the recorded P4.36 DEFAULT decision).
    let (status, windowed) = call(
        &router,
        get(
            "/api/v1/audit-events?from_event_unix_millis=1700000050000&to_event_unix_millis=1700000100000",
            Some("caller-a"),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        windowed["data"].as_array().expect("array").len(),
        2,
        "the inclusive bounds hit both records"
    );

    // Detail: by id in-tenant (a Viewer may read — audit evidence is
    // tenant-scoped data every authenticated role may read); the
    // cross-tenant record is 404 — no existence leak (§16.2, P0.16 rule 5).
    let detail_uri = format!("/api/v1/audit-events/{AUDIT_1}");
    let (status, detail) = call(&router, get(&detail_uri, Some("viewer-a"))).await;
    assert_eq!(status, StatusCode::OK);
    validate_envelope_response(&spec, "AuditEventDto", &detail, "audit detail");
    assert_eq!(detail["data"]["audit_id"], AUDIT_1);
    let (status, _) = call(
        &router,
        get(&format!("/api/v1/audit-events/{AUDIT_B}"), Some("caller-a")),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "cross-tenant detail is 404, no leak");
    let (status, _) = call(&router, get(&detail_uri, Some("caller-b"))).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "the foreign read of an in-tenant record is 404 too");
    // Caller B's own listing carries only tenant B's record.
    let (status, body_b) = call(&router, get("/api/v1/audit-events", Some("caller-b"))).await;
    assert_eq!(status, StatusCode::OK);
    let ids_b: Vec<&str> = body_b["data"]
        .as_array()
        .expect("page data array")
        .iter()
        .map(|r| r["audit_id"].as_str().expect("audit id"))
        .collect();
    assert_eq!(ids_b, vec![AUDIT_B], "tenant B sees only its own record");
}

#[tokio::test]
async fn malformed_audit_filters_cursors_and_limits_are_422() {
    let (router, _audit) = composed_audit_router();
    for uri in [
        "/api/v1/audit-events?cursor=not-a-uuid",
        "/api/v1/audit-events?limit=1001",
        "/api/v1/audit-events?limit=0",
        "/api/v1/audit-events?action=",
        "/api/v1/audit-events?actor=%20%20",
        "/api/v1/audit-events?from_event_unix_millis=200&to_event_unix_millis=100",
    ] {
        let (status, _) = call(&router, get(uri, Some("caller-a"))).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "malformed {uri}");
    }
    // A malformed DETAIL reference is 404 (a resource reference, not a
    // filter — the P1.17 rule).
    let (status, _) = call(
        &router,
        get("/api/v1/audit-events/not-a-uuid", Some("caller-a")),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn the_audit_events_resource_is_get_only_with_no_export_or_dump_route() {
    // §15.2 + the P1.10 append-only rule applied at the API layer: the
    // resource is GET-only in the GENERATED OpenAPI, live write probes
    // are refused, and no export/dump route exists anywhere near it.
    let (router, _audit) = composed_audit_router();
    let spec = openapi_spec(&router).await;
    let paths = spec["paths"]
        .as_object()
        .expect("the generated spec carries paths");
    assert!(
        paths.contains_key("/api/v1/audit-events"),
        "the audit-events resource is registered in the generated spec"
    );
    for path in ["/api/v1/audit-events", "/api/v1/audit-events/{event_id}"] {
        let operations = paths[path]
            .as_object()
            .expect("each path carries operation objects");
        assert!(
            operations.contains_key("get") && operations.len() == 1,
            "{path} is GET-only (found {operations:?})"
        );
    }
    // Live probes: POST on the list and the detail path is 405 (the route
    // exists, the method does not); POST to export/dump-shaped paths is
    // 404 (no such route — there is no generic export/dump endpoint).
    for uri in [
        "/api/v1/audit-events".to_string(),
        format!("/api/v1/audit-events/{AUDIT_1}"),
    ] {
        let (status, _) = call(&router, post_json(&uri, Some("caller-a"), json!({}))).await;
        assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED, "no POST on {uri}");
    }
    for uri in [
        "/api/v1/audit-events/export".to_string(),
        "/api/v1/audit-events/dump".to_string(),
        format!("/api/v1/audit-events/{AUDIT_1}/export"),
    ] {
        let (status, _) = call(&router, post_json(&uri, Some("caller-a"), json!({}))).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "no export/dump route at {uri}");
    }
}
