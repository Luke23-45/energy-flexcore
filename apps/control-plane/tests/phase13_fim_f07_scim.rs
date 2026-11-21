// FIM-F07.05 — the pinned SCIM suite for the FIM-F07 wave (Agent A;
// WRITTEN-UNRUN — never compiled or executed; execution is the human
// build/test gate, conventions §1 item 3).
//
// What this file pins (docs/releases/implementation_ledger/ledger_F07.md,
// FIM-F07.02/FIM-F07.05):
//
// - the SCIM create/deactivate ROUND TRIP including the group→role
//   mapping (over the real router via `tower::ServiceExt::oneshot` — NO
//   sockets, NO network: the request/response exchange is entirely
//   in-memory);
// - the SCIM 2.0 ListResponse shape AS DATA (the record-pinned
//   `schemas`/`totalResults`/`Resources`; the record does not pin the
//   URN spellings or startIndex/itemsPerPage — those are
//   UNVERIFIED-against-RFC-7644, asserted here only as the shape THIS
//   implementation serves);
// - the minimal filter (`userName Eq "value"`) + pagination handling
//   (and the named 400 for anything beyond the minimal grammar);
// - the DEPROVISIONING ENFORCEMENT POINT: a deactivated user's token
//   REJECTS at the RESOLVER (`ScimAwareIdentityResolver`) while the
//   underlying token is still valid (record 03 §2/§4 — resolver-level
//   enforcement, never UI-side);
// - the single group→role vocabulary: the SCIM mapping resolves through
//   `crate::auth::parse_role` — the SAME parser the login path consumes
//   (`tests/phase13_scim_role_mapping_tests.rs` pins the vocabulary; this
//   file pins that the SCIM store feeds it).
//
// HONESTY (FIM-F07.06): implemented-under-FIM-F07; NO SCIM object has
// ever been provisioned; RFC 7644 conformance against a real IdP push
// client is a HUMAN LAB GATE (P13.20); the Postgres-backed store is the
// recorded `UnwiredScimStoreBackend` deployment seam (the in-memory store
// is the only shipped backend — `scim_store.rs` honesty header).

use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use flexcore_authz::policy::Role;
use flexcore_contracts::ids::FlexUuid;
use flexcore_control_plane::auth::{AuthError, Caller, IdentityResolver};
use flexcore_control_plane::scim::{scim_router, ScimAwareIdentityResolver, ScimService};
use flexcore_control_plane::scim_store::{
    InMemoryScimStore, NewScimGroup, NewScimUser, ScimStore,
};
use flexcore_domain::ids::TenantId;
use flexcore_domain::tenant_context::{AuthenticatedClaims, TenantContext, VerifiedClaims};
use serde_json::{json, Value};
use tower::ServiceExt;

const TENANT_A_UUID: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f8a01";
const TENANT_B_UUID: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f8a02";

fn tenant_a() -> TenantId {
    TenantId::new(FlexUuid::parse(TENANT_A_UUID).expect("valid test uuid"))
}

fn tenant_b() -> TenantId {
    TenantId::new(FlexUuid::parse(TENANT_B_UUID).expect("valid test uuid"))
}

/// The shared fixture: one in-memory store, the SCIM service for tenant A,
/// and the router (unmounted — the composition-root wiring is the
/// orchestrator's; the tests drive the router directly, which is exactly
/// what a mounted deployment would serve).
struct Fixture {
    store: Arc<dyn ScimStore>,
    router: Router,
}

fn fixture() -> Fixture {
    let store: Arc<dyn ScimStore> = Arc::new(InMemoryScimStore::default());
    let counter = Arc::new(AtomicI64::new(0));
    let clock = Arc::new(AtomicI64::new(1_000));
    let counter_for_service = Arc::clone(&counter);
    let clock_for_service = Arc::clone(&clock);
    let service = Arc::new(ScimService::new(
        Arc::clone(&store),
        tenant_a(),
        Box::new(move || format!("scim-{}", counter_for_service.fetch_add(1, Ordering::SeqCst))),
        Box::new(move || clock_for_service.load(Ordering::SeqCst)),
    ));
    Fixture {
        store,
        router: scim_router(service),
    }
}

async fn serve(router: &Router, method: &str, uri: &str, body: Option<Value>) -> (StatusCode, Value) {
    let builder = Request::builder().method(method).uri(uri);
    let request = match body {
        Some(payload) => builder
            .header("content-type", "application/json")
            .body(Body::from(payload.to_string()))
            .expect("the request builds"),
        None => builder.body(Body::empty()).expect("the request builds"),
    };
    let response = router.clone().oneshot(request).await.expect("oneshot is infallible");
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024)
        .await
        .expect("the response body reads");
    let parsed = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).expect("the response body is JSON")
    };
    (status, parsed)
}

fn group(
    store: &dyn ScimStore,
    id: &str,
    display_name: &str,
    role: Option<&str>,
) -> String {
    store
        .create_group(
            &tenant_a(),
            NewScimGroup {
                id: id.to_string(),
                display_name: display_name.to_string(),
                external_id: None,
                member_user_ids: vec![],
                mapped_role_id: role.map(|r| r.to_string()),
            },
            1_000,
        )
        .expect("the group creates");
    id.to_string()
}

// --- The create/deactivate round trip (the F-07 acceptance criterion 4) ----

#[tokio::test]
async fn a_scim_create_deactivate_round_trip_round_trips_the_user_and_its_group_mapping() {
    let fx = fixture();
    let group_id = group(
        fx.store.as_ref(),
        "g-admins",
        "tenant administrators",
        Some("tenant-admin"),
    );

    // CREATE: POST /scim/v2/Users with the initial group membership (the
    // record 03 §2 create-with-`groups` lifecycle shape).
    let (status, created) = serve(
        &fx.router,
        "POST",
        "/scim/v2/Users",
        Some(json!({
            "schemas": ["urn:ietf:params:scim:schemas:core:2.0:User"],
            "userName": "alice",
            "displayName": "Alice Example",
            "active": true,
            "groups": [{"value": group_id}],
        })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "the create answers 201: {created}");
    assert_eq!(created["schemas"][0], "urn:ietf:params:scim:schemas:core:2.0:User");
    assert_eq!(created["userName"], "alice");
    assert_eq!(created["active"], true);
    assert_eq!(created["groups"][0]["value"], "g-admins");
    let user_id = created["id"].as_str().expect("the SCIM id is served").to_string();
    assert!(!user_id.is_empty(), "the id generator assigned a stable SCIM id");

    // The MAPPING resolved through the closed vocabulary (the store feeds
    // `parse_role` — the single home, record 03 §3).
    assert_eq!(
        fx.store
            .resolve_role_for_user(&tenant_a(), &user_id)
            .expect("the resolution reads"),
        Some(Role::TenantAdmin),
        "the created user's group mapping resolves to the role"
    );

    // DEACTIVATE: DELETE /scim/v2/Users/{id} is the record 03 §2
    // deprovisioning path (the record stays; `active` flips).
    let (status, body) = serve(
        &fx.router,
        "DELETE",
        &format!("/scim/v2/Users/{user_id}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "the deactivate answers 204: {body}");

    // The record survives (lifecycle evidence) with active=false, and the
    // mapping still resolves — the ENFORCEMENT is the resolver's active
    // check (`ScimAwareIdentityResolver` below), not a store-side role
    // removal.
    let (status, fetched) = serve(
        &fx.router,
        "GET",
        &format!("/scim/v2/Users/{user_id}"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(fetched["active"], false, "the deactivation round-trips");
    assert_eq!(fetched["userName"], "alice");
    assert_eq!(
        fx.store
            .resolve_role_for_user(&tenant_a(), &user_id)
            .expect("the resolution reads"),
        Some(Role::TenantAdmin),
        "the mapping survives the deactivation"
    );
}

#[tokio::test]
async fn deactivated_user_serves_active_false_and_the_mapping_survives() {
    let fx = fixture();
    let group_id = group(fx.store.as_ref(), "g-ops", "operators", Some("operator"));
    let (status, created) = serve(
        &fx.router,
        "POST",
        "/scim/v2/Users",
        Some(json!({"userName": "bob", "groups": [{"value": group_id}]})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let user_id = created["id"].as_str().unwrap().to_string();

    let (status, body) = serve(&fx.router, "DELETE", &format!("/scim/v2/Users/{user_id}"), None).await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    let (status, fetched) = serve(&fx.router, "GET", &format!("/scim/v2/Users/{user_id}"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(fetched["active"], false, "the deactivation round-trips");
    assert_eq!(fetched["userName"], "bob", "the record survives the deactivation");
    // The mapping itself is unchanged — the ENFORCEMENT is the resolver's
    // active check, not a store-side role removal.
    assert_eq!(
        fx.store
            .resolve_role_for_user(&tenant_a(), &user_id)
            .expect("the resolution reads"),
        Some(Role::Operator)
    );
}

// --- The ListResponse shape as data -----------------------------------------

#[tokio::test]
async fn the_users_list_serves_the_record_pinned_response_shape() {
    let fx = fixture();
    group(fx.store.as_ref(), "g1", "everyone", None);
    for name in ["alice", "bob", "carol"] {
        let (status, _) = serve(
            &fx.router,
            "POST",
            "/scim/v2/Users",
            Some(json!({"userName": name, "groups": [{"value": "g1"}]})),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
    }
    let (status, list) = serve(&fx.router, "GET", "/scim/v2/Users", None).await;
    assert_eq!(status, StatusCode::OK);
    // The record-pinned shape as DATA: schemas, totalResults, Resources.
    assert!(list["schemas"]
        .as_array()
        .expect("schemas is an array")
        .iter()
        .any(|s| s == "urn:ietf:params:scim:api:messages:2.0:ListResponse"));
    assert_eq!(list["totalResults"], 3);
    let resources = list["Resources"].as_array().expect("Resources is an array");
    assert_eq!(resources.len(), 3);
    for resource in resources {
        assert_eq!(resource["schemas"][0], "urn:ietf:params:scim:schemas:core:2.0:User");
        assert!(resource["id"].is_string());
        assert!(resource["userName"].is_string());
        assert!(resource["active"].is_boolean());
    }
}

#[tokio::test]
async fn the_minimal_filter_and_pagination_are_handled_and_beyond_them_refused() {
    let fx = fixture();
    for name in ["alice", "bob", "alice-2"] {
        let (status, _) = serve(
            &fx.router,
            "POST",
            "/scim/v2/Users",
            Some(json!({"userName": name})),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
    }
    // The minimal filter: userName Eq "alice" (URL-encoded).
    let (status, filtered) = serve(
        &fx.router,
        "GET",
        "/scim/v2/Users?filter=userName%20Eq%20%22alice%22",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(filtered["totalResults"], 1, "only the exact match: {filtered}");
    assert_eq!(filtered["Resources"][0]["userName"], "alice");

    // Pagination: startIndex=2, count=1 over the unfiltered list.
    let (status, page) = serve(
        &fx.router,
        "GET",
        "/scim/v2/Users?startIndex=2&count=1",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(page["totalResults"], 3, "totalResults counts the filter, not the page");
    assert_eq!(page["Resources"].as_array().unwrap().len(), 1);
    assert_eq!(page["Resources"][0]["userName"], "bob", "the id-sorted page 2 of size 1");

    // Anything beyond the minimal grammar is a NAMED 400 (never a silent
    // pass-all).
    for unsupported in [
        "/scim/v2/Users?filter=userName%20Ne%20%22alice%22",
        "/scim/v2/Users?filter=displayName%20Eq%20%22alice%22",
        "/scim/v2/Users?filter=userName%20Eq%20alice",
    ] {
        let (status, err) = serve(&fx.router, "GET", unsupported, None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{unsupported}: {err}");
        assert!(
            err["detail"].is_string(),
            "the refusal carries a named detail: {err}"
        );
    }
}

// --- PATCH (minimal) and the error mappings ---------------------------------

#[tokio::test]
async fn patch_replaces_active_and_anything_else_is_a_named_400() {
    let fx = fixture();
    let (status, created) = serve(
        &fx.router,
        "POST",
        "/scim/v2/Users",
        Some(json!({"userName": "carol"})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let user_id = created["id"].as_str().unwrap().to_string();

    // The supported subset: replace active=false (the record 03 §2
    // deactivation path over PATCH).
    let (status, patched) = serve(
        &fx.router,
        "PATCH",
        &format!("/scim/v2/Users/{user_id}"),
        Some(json!({
            "schemas": ["urn:ietf:params:scim:api:messages:2.0:PatchOp"],
            "Operations": [{"op": "replace", "path": "active", "value": false}],
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{patched}");
    assert_eq!(patched["active"], false);

    // Unsupported grammar: a named 400, and the record is unchanged.
    let (status, err) = serve(
        &fx.router,
        "PATCH",
        &format!("/scim/v2/Users/{user_id}"),
        Some(json!({
            "Operations": [{"op": "replace", "path": "userName", "value": "carol-2"}],
        })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(err["detail"].is_string());
    let (status, unchanged) = serve(&fx.router, "GET", &format!("/scim/v2/Users/{user_id}"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(unchanged["userName"], "carol", "the refused patch did not apply");
}

#[tokio::test]
async fn duplicate_user_name_answers_409_and_unknown_group_404() {
    let fx = fixture();
    let (status, _) = serve(
        &fx.router,
        "POST",
        "/scim/v2/Users",
        Some(json!({"userName": "dave"})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    // Duplicate (case-insensitive uniqueness, the stricter direction).
    let (status, err) = serve(
        &fx.router,
        "POST",
        "/scim/v2/Users",
        Some(json!({"userName": "DAVE"})),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{err}");
    // Unknown group reference at create: 404 (a record of the tenant
    // vocabulary the store validates).
    let (status, err) = serve(
        &fx.router,
        "POST",
        "/scim/v2/Users",
        Some(json!({"userName": "erin", "groups": [{"value": "no-such-group"}]})),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{err}");
}

// --- Groups + ServiceProviderConfig ------------------------------------------

#[tokio::test]
async fn the_groups_list_serves_the_mapped_role_ids_and_spc_serves_the_minimal_resource() {
    let fx = fixture();
    group(
        fx.store.as_ref(),
        "g-approvers",
        "site approvers",
        Some("site-approver"),
    );
    group(fx.store.as_ref(), "g-open", "open group", None);
    let (status, groups) = serve(&fx.router, "GET", "/scim/v2/Groups", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(groups["totalResults"], 2);
    let resources = groups["Resources"].as_array().unwrap();
    assert_eq!(resources[0]["displayName"], "open group");
    assert_eq!(resources[1]["displayName"], "site approvers");
    assert_eq!(resources[1]["flexcoreRoleId"], "site-approver");
    assert!(
        resources[0].get("flexcoreRoleId").is_none(),
        "an unmapped group serves no role id (it contributes nothing)"
    );

    // The minimal ServiceProviderConfig.
    let (status, spc) = serve(&fx.router, "GET", "/scim/v2/ServiceProviderConfig", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        spc["schemas"][0],
        "urn:ietf:params:scim:schemas:core:2.0:ServiceProviderConfig"
    );
    assert!(spc["meta"].is_object());
}

// --- The deprovisioning enforcement point (record 03 §2/§4) -------------------

/// The inner-identity stand-in: accepts exactly one bearer token and
/// resolves it through the single public claims path (the composed
/// deployment wraps the P13.02 `OidcResolver`; no IdP exists, so the
/// inner path is a local fixture).
struct StaticInner {
    tenant: TenantId,
    subject: &'static str,
    token: &'static str,
    role: Role,
}

impl IdentityResolver for StaticInner {
    fn resolve(&self, bearer: Option<&str>) -> Result<Caller, AuthError> {
        let raw = bearer.ok_or(AuthError::MissingCredentials)?;
        if raw.strip_prefix("Bearer ").unwrap_or("") != self.token {
            return Err(AuthError::InvalidCredentials);
        }
        let claims = AuthenticatedClaims::from_verified(VerifiedClaims {
            tenant_id: self.tenant,
            subject: self.subject.to_string(),
        });
        Ok(Caller {
            ctx: TenantContext::from_authenticated_claims(claims),
            role: self.role,
        })
    }
}

#[test]
fn a_deactivated_user_denies_at_the_resolver_while_the_token_is_still_valid() {
    let store: Arc<dyn ScimStore> = Arc::new(InMemoryScimStore::default());
    group(store.as_ref(), "g-admins", "tenant administrators", Some("tenant-admin"));
    store
        .create_user(
            &tenant_a(),
            NewScimUser {
                id: "u-alice".to_string(),
                user_name: "alice".to_string(),
                display_name: None,
                external_id: None,
                active: true,
                group_ids: vec!["g-admins".to_string()],
            },
            1_000,
        )
        .expect("alice provisions");
    // The inner path resolves alice's VALID token to PlatformAdmin (the
    // inner role is REPLACED, never unioned, by the SCIM mapping).
    let resolver = ScimAwareIdentityResolver::new(
        Box::new(StaticInner {
            tenant: tenant_a(),
            subject: "alice",
            token: "tok-alice",
            role: Role::PlatformAdmin,
        }),
        Arc::clone(&store),
        tenant_a(),
    );
    let caller = resolver
        .resolve(Some("Bearer tok-alice"))
        .expect("an active provisioned user resolves");
    assert_eq!(caller.role, Role::TenantAdmin, "the SCIM mapping is the authority");
    assert_eq!(caller.ctx.subject(), "alice");

    // DEACTIVATE (the record 03 §2/§4 deprovisioning path). The token is
    // UNCHANGED and still valid for the inner resolver — the RESOLVER gate
    // is what now denies (within the recorded 5-minute SLA: here, on the
    // very next authentication).
    store
        .deactivate_user(&tenant_a(), "u-alice", 2_000)
        .expect("the deactivation books");
    assert_eq!(
        resolver
            .resolve(Some("Bearer tok-alice"))
            .expect_err("the resolver must deny this request"),
        AuthError::InvalidCredentials,
        "a deactivated user's token rejects at the resolver regardless of token validity"
    );

    // An UNPROVISIONED subject denies too (composing this resolver opts
    // the deployment into SCIM-managed identity — the fail-closed shape).
    let unprovisioned = ScimAwareIdentityResolver::new(
        Box::new(StaticInner {
            tenant: tenant_a(),
            subject: "mallory",
            token: "tok-mallory",
            role: Role::PlatformAdmin,
        }),
        Arc::clone(&store),
        tenant_a(),
    );
    assert_eq!(
        unprovisioned
            .resolve(Some("Bearer tok-mallory"))
            .expect_err("the resolver must deny this request"),
        AuthError::InvalidCredentials
    );

    // A provisioned user in NO mapped group denies (never defaults to
    // Viewer — the P13.02 rule through the SCIM path).
    store
        .create_user(
            &tenant_a(),
            NewScimUser {
                id: "u-nomap".to_string(),
                user_name: "nomap".to_string(),
                display_name: None,
                external_id: None,
                active: true,
                group_ids: vec![],
            },
            1_100,
        )
        .expect("nomap provisions");
    let nomap = ScimAwareIdentityResolver::new(
        Box::new(StaticInner {
            tenant: tenant_a(),
            subject: "nomap",
            token: "tok-nomap",
            role: Role::PlatformAdmin,
        }),
        Arc::clone(&store),
        tenant_a(),
    );
    assert_eq!(
        nomap
            .resolve(Some("Bearer tok-nomap"))
            .expect_err("the resolver must deny this request"),
        AuthError::InvalidCredentials
    );

    // The tenant boundary: a user of ANOTHER tenant is invisible to this
    // connection's enforcement (deny, never resolve).
    store
        .create_user(
            &tenant_b(),
            NewScimUser {
                id: "u-b".to_string(),
                user_name: "other-tenant-user".to_string(),
                display_name: None,
                external_id: None,
                active: true,
                group_ids: vec![],
            },
            1_100,
        )
        .expect("the tenant-B user provisions");
    let cross = ScimAwareIdentityResolver::new(
        Box::new(StaticInner {
            tenant: tenant_b(),
            subject: "other-tenant-user",
            token: "tok-b",
            role: Role::PlatformAdmin,
        }),
        Arc::clone(&store),
        tenant_a(),
    );
    assert_eq!(
        cross
            .resolve(Some("Bearer tok-b"))
            .expect_err("the resolver must deny this request"),
        AuthError::InvalidCredentials,
        "the enforcement lookup is tenant-scoped by construction"
    );
}
