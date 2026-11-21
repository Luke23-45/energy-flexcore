// FIM-F07.02 (Wave 4, Agent A; WRITTEN-UNRUN) — the SCIM 2.0 HTTP surface:
//         a ROUTER-AGNOSTIC module. `scim_router` builds the endpoint set
//         as an `axum::Router` that the composition root NESTS (no
//         `main.rs`/`routes*.rs` change was made by this subtask — the
//         orchestrator composes the router; this module owns no state of
//         the application `AppState`).
//
// HONESTY HEADER (FIM-F07.06, stated in full):
//
// - Implemented-under-FIM-F07. WRITTEN-UNRUN: this surface has never been
//   compiled or served; NO SCIM request has ever been processed, no SCIM
//   object has ever been provisioned. Execution is the HUMAN BUILD/TEST
//   GATE; RFC 7644 conformance against a real IdP push client is a HUMAN
//   LAB GATE (P13.20).
// - SCIM schema details as DATA: the record 03 shape (the list response
//   `schemas`/`totalResults`/`Resources`, the user lifecycle) is pinned
//   below. Every SCIM detail the record does NOT pin is marked
//   UNVERIFIED-against-RFC-7644 at its use site — the schema URN
//   constants, the error-body shape, the ServiceProviderConfig fields,
//   the PATCH grammar subset, and the pagination semantics beyond the
//   record's pin are UNVERIFIED. Nothing here claims conformance
//   (conventions §7 item 3).
// - AUTHORIZATION IS NOT IN THIS MODULE: `scim_router` is tenant-PINNED
//   by construction — every handler operates on exactly ONE tenant (the
//   SCIM connection's tenant, record 01 §4), supplied at composition.
//   The composed deployment MUST mount the router behind tenant-scoped
//   authentication/authorization middleware (the orchestrator's wiring);
//   mounted bare, the surface would be unauthenticated — the wiring is a
//   deployment act, and the fail-closed default is that NOTHING composes
//   it in this repo.
//
// Endpoints (the task-pinned surface):
//
// - `GET  /scim/v2/Users`                list (pagination + the minimal
//                                        `userName Eq "..."` filter)
// - `POST /scim/v2/Users`                create (201; the record 03 §2
//                                        lifecycle; optional initial group
//                                        memberships per the record's
//                                        create-with-`groups` shape)
// - `GET  /scim/v2/Users/{id}`           read
// - `PUT  /scim/v2/Users/{id}`           full replace
// - `PATCH /scim/v2/Users/{id}`          MINIMAL patch: `replace` on
//                                        `active` only (anything else is a
//                                        named 400 — the full RFC 7644
//                                        PatchOp grammar is
//                                        UNVERIFIED-against-RFC-7644 and
//                                        deliberately not built)
// - `DELETE /scim/v2/Users/{id}`         DEACTIVATE (record 03 §2: DELETE
//                                        or `active=false` is the
//                                        deprovisioning path — the record
//                                        is the authority; the RFC-level
//                                        hard-delete semantics are
//                                        UNVERIFIED-against-RFC-7644)
// - `GET  /scim/v2/Groups`               list (read-only over HTTP; group
//                                        creation/mapping is store-level
//                                        in this batch)
// - `GET  /scim/v2/ServiceProviderConfig` the minimal discovery resource
//
// The group→role mapping consumes the SAME closed vocabulary as the login
// path (`crate::auth::parse_role`, the single home — record 03 §3; the
// written-unrun `tests/phase13_scim_role_mapping_tests.rs` pins the
// vocabulary): mapping validation happens at the store boundary
// (`scim_store.rs`), never in the HTTP layer.
//
// Deprovisioning enforcement (record 03 §2/§4): a deactivated user's
// token REJECTS at the RESOLVER regardless of token validity. The
// enforcement point below is [`ScimAwareIdentityResolver`] — a resolver
// wrapper the composed deployment opts into (composition, not code
// change; `DenyAllResolver` remains the shipped default).

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use flexcore_domain::ids::TenantId;

use crate::auth::{AuthError, Caller, IdentityResolver};
use crate::scim_store::{
    NewScimUser, ScimListQuery, ScimStore, ScimStoreError, ScimUser, ScimUserPatch,
};

// --- The SCIM schema constants (UNVERIFIED-against-RFC-7644 as strings) ----------

// The URN spellings below are the RFC 7643/7644 core-schema identifiers as
// commonly published. The P13.03 record pins the response SHAPE (schemas,
// totalResults, Resources) as data, not these strings; the strings are
// therefore UNVERIFIED-against-RFC-7644 in this repo (no primary-source
// verification was performed in this wave — honesty rule 1).
pub const SCIM_USER_SCHEMA_URN: &str = "urn:ietf:params:scim:schemas:core:2.0:User";
pub const SCIM_GROUP_SCHEMA_URN: &str = "urn:ietf:params:scim:schemas:core:2.0:Group";
pub const SCIM_LIST_RESPONSE_URN: &str = "urn:ietf:params:scim:api:messages:2.0:ListResponse";
pub const SCIM_PATCH_OP_URN: &str = "urn:ietf:params:scim:api:messages:2.0:PatchOp";
pub const SCIM_ERROR_URN: &str = "urn:ietf:params:scim:api:messages:2.0:Error";
pub const SCIM_SPC_SCHEMA_URN: &str =
    "urn:ietf:params:scim:schemas:core:2.0:ServiceProviderConfig";

/// The Users collection path (the router mounts it relative to the nest
/// point).
pub const SCIM_USERS_PATH: &str = "/scim/v2/Users";
/// The user item path pattern.
pub const SCIM_USER_PATH: &str = "/scim/v2/Users/{id}";
/// The Groups collection path (GET only — the task-pinned surface).
pub const SCIM_GROUPS_PATH: &str = "/scim/v2/Groups";
/// The ServiceProviderConfig path.
pub const SCIM_SPC_PATH: &str = "/scim/v2/ServiceProviderConfig";

// --- The service state -------------------------------------------------------------

/// The SCIM service state: the tenant-scoped store, the ONE tenant the
/// connection serves (record 01 §4), the id generator, and the controlled
/// clock. Every handler operates on exactly this tenant — there is no
/// request path that can widen it (the tenant is not taken from the
/// request; the header's authorization note covers the mounting contract).
pub struct ScimService {
    store: Arc<dyn ScimStore>,
    tenant: TenantId,
    ids: Box<dyn Fn() -> String + Send + Sync>,
    now_millis: Box<dyn Fn() -> i64 + Send + Sync>,
}

impl ScimService {
    /// Builds the service. The store is injected (the in-memory
    /// implementation for tests and single-process deployments; the
    /// Postgres-backed store is the recorded `UnwiredScimStoreBackend`
    /// deployment seam — `scim_store.rs` honesty header).
    pub fn new(
        store: Arc<dyn ScimStore>,
        tenant: TenantId,
        ids: Box<dyn Fn() -> String + Send + Sync>,
        now_millis: Box<dyn Fn() -> i64 + Send + Sync>,
    ) -> Self {
        Self {
            store,
            tenant,
            ids,
            now_millis,
        }
    }

    /// The tenant every handler of this service operates on (recorded for
    /// audit/observability composition).
    pub fn tenant(&self) -> TenantId {
        self.tenant
    }
}

/// FIM-F07.02 — the router-agnostic SCIM 2.0 router. The orchestrator
/// nests it (e.g. `Router::new().nest("/scim", scim_router(...))` — the
/// paths below are absolute from the nest point). NO composition exists
/// in this repo (the fail-closed default: the surface is simply not
/// mounted).
pub fn scim_router(service: Arc<ScimService>) -> Router {
    Router::new()
        .route(
            SCIM_USERS_PATH,
            get(list_users_handler).post(create_user_handler),
        )
        .route(
            SCIM_USER_PATH,
            get(get_user_handler)
                .put(put_user_handler)
                .patch(patch_user_handler)
                .delete(delete_user_handler),
        )
        .route(SCIM_GROUPS_PATH, get(list_groups_handler))
        .route(SCIM_SPC_PATH, get(service_provider_config_handler))
        .with_state(service)
}

// --- The wire DTOs ------------------------------------------------------------------

/// The SCIM meta block (minimal: resourceType + location; the created/
/// lastModified dateTime fields are UNVERIFIED-against-RFC-7644 and
/// deliberately omitted).
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ScimMeta {
    #[serde(rename = "resourceType")]
    pub resource_type: &'static str,
    pub location: String,
}

/// The user resource (the record-pinned fields as data; the schemas/
/// attribute spellings beyond that are UNVERIFIED-against-RFC-7644).
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ScimUserResource {
    pub schemas: Vec<&'static str>,
    pub id: String,
    pub user_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_id: Option<String>,
    pub active: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<ScimGroupRef>,
    pub meta: ScimMeta,
}

/// The group reference inside a user resource (the `value`/`display`
/// shape; UNVERIFIED-against-RFC-7644 beyond the record's pin).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ScimGroupRef {
    /// The group id.
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display: Option<String>,
}

/// The group resource (the record-pinned fields; `members` as value
/// references).
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ScimGroupResource {
    pub schemas: Vec<&'static str>,
    pub id: String,
    pub display_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_id: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub members: Vec<ScimGroupRef>,
    /// The mapped role id, when the group is mapped onto the closed
    /// vocabulary (a FlexCore-specific extension attribute — marked as
    /// such: UNVERIFIED-against-RFC-7644; the mapping is tenant data,
    /// record 03 §3).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flexcore_role_id: Option<String>,
    pub meta: ScimMeta,
}

fn user_resource(user: &ScimUser, group_displays: &[(String, String)]) -> ScimUserResource {
    ScimUserResource {
        schemas: vec![SCIM_USER_SCHEMA_URN],
        id: user.id.clone(),
        user_name: user.user_name.clone(),
        display_name: user.display_name.clone(),
        external_id: user.external_id.clone(),
        active: user.active,
        groups: user
            .group_ids
            .iter()
            .map(|id| ScimGroupRef {
                value: id.clone(),
                display: group_displays
                    .iter()
                    .find(|(gid, _)| gid == id)
                    .map(|(_, display)| display.clone()),
            })
            .collect(),
        meta: ScimMeta {
            resource_type: "User",
            location: format!("{SCIM_USERS_PATH}/{id}", id = user.id),
        },
    }
}

// --- Error mapping ------------------------------------------------------------------

/// Maps a named store error to the SCIM status + detail. The status
/// choices (404 unknown resource, 409 duplicate userName, 422 invalid
/// mapping value) are this repo's minimal handling — UNVERIFIED-against-
/// RFC-7644 beyond the record's pin.
fn map_store_error(err: &ScimStoreError) -> (StatusCode, String) {
    match err {
        ScimStoreError::UserNotFound { .. } | ScimStoreError::GroupNotFound { .. } => {
            (StatusCode::NOT_FOUND, err.to_string())
        }
        ScimStoreError::DuplicateUserName { .. } => (StatusCode::CONFLICT, err.to_string()),
        ScimStoreError::UserNotInTenant { .. } | ScimStoreError::UnknownRoleId { .. } => {
            (StatusCode::UNPROCESSABLE_ENTITY, err.to_string())
        }
        ScimStoreError::StorePoisoned { .. } => {
            (StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
        }
    }
}

/// The SCIM error body (the `schemas`/`status`/`detail` shape as DATA;
/// UNVERIFIED-against-RFC-7644 as a wire detail).
fn scim_error(status: StatusCode, detail: String) -> (StatusCode, Json<Value>) {
    (
        status,
        Json(json!({
            "schemas": [SCIM_ERROR_URN],
            "status": status.as_u16().to_string(),
            "detail": detail,
        })),
    )
}

// --- Request bodies -------------------------------------------------------------------

/// The create-user body (the record 03 §2 shape: `userName` required;
/// the optional initial `groups` memberships ride the create per the
/// record's create-with-`groups` lifecycle — UNVERIFIED-against-RFC-7644;
/// role-affecting updates are the PATCH/PUT surfaces).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScimUserCreateRequest {
    pub user_name: String,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub external_id: Option<String>,
    #[serde(default = "default_true")]
    pub active: bool,
    #[serde(default)]
    pub groups: Vec<ScimGroupRef>,
}

fn default_true() -> bool {
    true
}

/// The full-replace body (PUT).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScimUserReplaceRequest {
    pub user_name: String,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub external_id: Option<String>,
    #[serde(default = "default_true")]
    pub active: bool,
    #[serde(default)]
    pub groups: Vec<ScimGroupRef>,
}

/// The MINIMAL patch body: only `replace` operations on `active` are
/// honored (the deprovisioning path `active=false`, record 03 §2).
/// Anything else is a named 400 — the full PatchOp grammar is
/// UNVERIFIED-against-RFC-7644 and deliberately not built.
#[derive(Debug, Clone, Deserialize)]
pub struct ScimPatchRequest {
    #[serde(default, rename = "schemas")]
    pub _schemas: Vec<String>,
    #[serde(default, rename = "Operations")]
    pub operations: Vec<ScimPatchOperation>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ScimPatchOperation {
    pub op: String,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub value: Value,
}

// --- Query parsing --------------------------------------------------------------------

/// The list query params (SCIM pagination + the ONE minimal filter).
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ScimListParams {
    #[serde(default)]
    pub start_index: Option<String>,
    #[serde(default)]
    pub count: Option<String>,
    #[serde(default)]
    pub filter: Option<String>,
}

/// Parses the params into the store query. The filter grammar is MINIMAL:
/// exactly `userName Eq "value"` (attribute and operator compared
/// case-insensitively, the value in double quotes). Anything else is a
/// named 400 — the full RFC 7644 filter grammar is
/// UNVERIFIED-against-RFC-7644 and deliberately not built.
fn parse_list_params(params: &ScimListParams) -> Result<ScimListQuery, String> {
    let start_index = match &params.start_index {
        Some(raw) => raw
            .trim()
            .parse::<usize>()
            .map_err(|_| format!("startIndex '{raw}' is not a non-negative integer"))?,
        None => 1,
    };
    let count = match &params.count {
        Some(raw) => raw
            .trim()
            .parse::<usize>()
            .map_err(|_| format!("count '{raw}' is not a non-negative integer"))?,
        None => 0,
    };
    let user_name_eq = match &params.filter {
        Some(raw) => {
            let filter = raw.trim();
            let lower = filter.to_ascii_lowercase();
            if let Some(rest) = lower.strip_prefix("username") {
                let rest = rest.trim_start();
                let operator_len = filter.len() - rest.len();
                let rest_of_raw = &filter[operator_len..];
                let rest = rest_of_raw.trim_start();
                if !rest.to_ascii_lowercase().starts_with("eq") {
                    return Err(format!(
                        "the filter '{filter}' is not supported: only 'userName Eq \"value\"' \
                         is implemented"
                    ));
                }
                let consumed = filter.len() - rest.len() + 2;
                let value_part = filter[consumed.min(filter.len())..].trim();
                let value = value_part
                    .strip_prefix('"')
                    .and_then(|v| v.strip_suffix('"'))
                    .ok_or_else(|| {
                        format!(
                            "the filter '{filter}' is not supported: the comparison value must \
                             be a double-quoted string"
                        )
                    })?;
                if value.is_empty() {
                    return Err(format!(
                        "the filter '{filter}' is not supported: the comparison value must not \
                         be empty"
                    ));
                }
                Some(value.to_string())
            } else {
                return Err(format!(
                    "the filter '{filter}' is not supported: only 'userName Eq \"value\"' is \
                     implemented"
                ));
            }
        }
        None => None,
    };
    Ok(ScimListQuery {
        start_index,
        count,
        user_name_eq,
    })
}

// --- Handlers ---------------------------------------------------------------------------

/// The group (id, displayName) pairs of the tenant — the display names the
/// user-resource group references render. A store failure degrades to
/// empty DISPLAY names only (cosmetic, `value` ids still resolve the
/// mapping); the store reads that carry authority go through the named
/// error mapping below, never through this helper.
fn group_displays(service: &ScimService) -> Vec<(String, String)> {
    service
        .store
        .list_groups(&service.tenant, &ScimListQuery::all())
        .map(|result| {
            result
                .resources
                .iter()
                .map(|g| (g.id.clone(), g.display_name.clone()))
                .collect()
        })
        .unwrap_or_default()
}

fn list_users_handler(
    State(service): State<Arc<ScimService>>,
    Query(params): Query<ScimListParams>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let query = parse_list_params(&params).map_err(|detail| {
        scim_error(StatusCode::BAD_REQUEST, detail)
    })?;
    let result = service
        .store
        .list_users(&service.tenant, &query)
        .map_err(|e| {
            let (status, detail) = map_store_error(&e);
            scim_error(status, detail)
        })?;
    let displays = group_displays(&service);
    let resources: Vec<ScimUserResource> = result
        .resources
        .iter()
        .map(|u| user_resource(u, &displays))
        .collect();
    // The record-pinned ListResponse shape as DATA (schemas, totalResults,
    // Resources); startIndex/itemsPerPage are UNVERIFIED-against-RFC-7644.
    Ok(Json(json!({
        "schemas": [SCIM_LIST_RESPONSE_URN],
        "totalResults": result.total_results,
        "startIndex": result.start_index,
        "itemsPerPage": result.items_per_page,
        "Resources": resources,
    })))
}

fn create_user_handler(
    State(service): State<Arc<ScimService>>,
    Json(body): Json<ScimUserCreateRequest>,
) -> Result<(StatusCode, Json<ScimUserResource>), (StatusCode, Json<Value>)> {
    if body.user_name.trim().is_empty() {
        return Err(scim_error(
            StatusCode::BAD_REQUEST,
            "userName is required and must not be blank".to_string(),
        ));
    }
    let now = (service.now_millis)();
    let created = service
        .store
        .create_user(
            &service.tenant,
            NewScimUser {
                id: (service.ids)(),
                user_name: body.user_name,
                display_name: body.display_name,
                external_id: body.external_id,
                active: body.active,
                group_ids: body.groups.iter().map(|g| g.value.clone()).collect(),
            },
            now,
        )
        .map_err(|e| {
            let (status, detail) = map_store_error(&e);
            scim_error(status, detail)
        })?;
    let displays = group_displays(&service);
    Ok((
        StatusCode::CREATED,
        Json(user_resource(&created, &displays)),
    ))
}

fn get_user_handler(
    State(service): State<Arc<ScimService>>,
    Path(id): Path<String>,
) -> Result<Json<ScimUserResource>, (StatusCode, Json<Value>)> {
    let user = service
        .store
        .get_user(&service.tenant, &id)
        .map_err(|e| {
            let (status, detail) = map_store_error(&e);
            scim_error(status, detail)
        })?
        .ok_or_else(|| {
            scim_error(
                StatusCode::NOT_FOUND,
                format!("no SCIM user '{id}' in this tenant"),
            )
        })?;
    let displays = group_displays(&service);
    Ok(Json(user_resource(&user, &displays)))
}

fn put_user_handler(
    State(service): State<Arc<ScimService>>,
    Path(id): Path<String>,
    Json(body): Json<ScimUserReplaceRequest>,
) -> Result<Json<ScimUserResource>, (StatusCode, Json<Value>)> {
    if body.user_name.trim().is_empty() {
        return Err(scim_error(
            StatusCode::BAD_REQUEST,
            "userName is required and must not be blank".to_string(),
        ));
    }
    let now = (service.now_millis)();
    let updated = service
        .store
        .update_user(
            &service.tenant,
            &id,
            ScimUserPatch {
                user_name: Some(body.user_name),
                display_name: Some(body.display_name),
                external_id: Some(body.external_id),
                active: Some(body.active),
                group_ids: Some(body.groups.iter().map(|g| g.value.clone()).collect()),
            },
            now,
        )
        .map_err(|e| {
            let (status, detail) = map_store_error(&e);
            scim_error(status, detail)
        })?;
    let displays = group_displays(&service);
    Ok(Json(user_resource(&updated, &displays)))
}

fn patch_user_handler(
    State(service): State<Arc<ScimService>>,
    Path(id): Path<String>,
    Json(body): Json<ScimPatchRequest>,
) -> Result<Json<ScimUserResource>, (StatusCode, Json<Value>)> {
    // The minimal PatchOp subset: replace on `active` only (the
    // deprovisioning path). Anything else is a named 400.
    let mut patch = ScimUserPatch::default();
    for operation in &body.operations {
        let is_replace = operation.op.eq_ignore_ascii_case("replace");
        let targets_active = operation
            .path
            .as_deref()
            .map(|p| p.eq_ignore_ascii_case("active"))
            .unwrap_or(false);
        if !(is_replace && targets_active) {
            return Err(scim_error(
                StatusCode::BAD_REQUEST,
                format!(
                    "the patch operation (op='{}', path='{}') is not supported: only \
                     'replace' on 'active' is implemented",
                    operation.op,
                    operation.path.as_deref().unwrap_or("")
                ),
            ));
        }
        let active = operation.value.as_bool().ok_or_else(|| {
            scim_error(
                StatusCode::BAD_REQUEST,
                "the replace value for 'active' must be a boolean".to_string(),
            )
        })?;
        patch.active = Some(active);
    }
    if patch.active.is_none() {
        return Err(scim_error(
            StatusCode::BAD_REQUEST,
            "the patch carries no supported operation: only 'replace' on 'active' is \
             implemented"
                .to_string(),
        ));
    }
    let now = (service.now_millis)();
    let updated = service
        .store
        .update_user(&service.tenant, &id, patch, now)
        .map_err(|e| {
            let (status, detail) = map_store_error(&e);
            scim_error(status, detail)
        })?;
    let displays = group_displays(&service);
    Ok(Json(user_resource(&updated, &displays)))
}

fn delete_user_handler(
    State(service): State<Arc<ScimService>>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<Value>)> {
    // Record 03 §2: DELETE or active=false is the DEACTIVATION path — the
    // record stays (lifecycle evidence), the resolver denies the user at
    // the next authentication. (The RFC-level hard-delete semantics are
    // UNVERIFIED-against-RFC-7644; the record is the authority here.)
    let now = (service.now_millis)();
    service
        .store
        .deactivate_user(&service.tenant, &id, now)
        .map_err(|e| {
            let (status, detail) = map_store_error(&e);
            scim_error(status, detail)
        })?;
    Ok(StatusCode::NO_CONTENT)
}

fn list_groups_handler(
    State(service): State<Arc<ScimService>>,
    Query(params): Query<ScimListParams>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    // No filter for groups (the task-pinned GET surface); pagination uses
    // the same minimal handling.
    let start_index = match &params.start_index {
        Some(raw) => raw.trim().parse::<usize>().map_err(|_| {
            scim_error(
                StatusCode::BAD_REQUEST,
                format!("startIndex '{raw}' is not a non-negative integer"),
            )
        })?,
        None => 1,
    };
    let count = match &params.count {
        Some(raw) => raw.trim().parse::<usize>().map_err(|_| {
            scim_error(
                StatusCode::BAD_REQUEST,
                format!("count '{raw}' is not a non-negative integer"),
            )
        })?,
        None => 0,
    };
    let result = service
        .store
        .list_groups(
            &service.tenant,
            &ScimListQuery {
                start_index,
                count,
                user_name_eq: None,
            },
        )
        .map_err(|e| {
            let (status, detail) = map_store_error(&e);
            scim_error(status, detail)
        })?;
    let resources: Vec<ScimGroupResource> = result
        .resources
        .iter()
        .map(|g| ScimGroupResource {
            schemas: vec![SCIM_GROUP_SCHEMA_URN],
            id: g.id.clone(),
            display_name: g.display_name.clone(),
            external_id: g.external_id.clone(),
            members: g
                .member_user_ids
                .iter()
                .map(|uid| ScimGroupRef {
                    value: uid.clone(),
                    display: None,
                })
                .collect(),
            flexcore_role_id: g.mapped_role_id.clone(),
            meta: ScimMeta {
                resource_type: "Group",
                location: format!("{SCIM_GROUPS_PATH}/{id}", id = g.id),
            },
        })
        .collect();
    Ok(Json(json!({
        "schemas": [SCIM_LIST_RESPONSE_URN],
        "totalResults": result.total_results,
        "startIndex": result.start_index,
        "itemsPerPage": result.items_per_page,
        "Resources": resources,
    })))
}

fn service_provider_config_handler() -> Json<Value> {
    // The MINIMAL ServiceProviderConfig (the task-pinned resource). The
    // field set and the shapes below are UNVERIFIED-against-RFC-7644 —
    // the record pins no ServiceProviderConfig detail; a conformance run
    // is the human lab gate (P13.20).
    Json(json!({
        "schemas": [SCIM_SPC_SCHEMA_URN],
        "patch": { "supported": true },
        "filter": { "supported": true },
        "meta": {
            "resourceType": "ServiceProviderConfig",
            "location": SCIM_SPC_PATH,
        },
    }))
}

// --- The deprovisioning enforcement point (record 03 §2/§4) ------------------------

/// The SCIM-aware identity resolver (FIM-F07.02): wraps the production
/// identity path (the P13.02 `OidcResolver` in a composed deployment) and
/// enforces the deprovisioning semantics AT THE RESOLVER — a deactivated
/// (or unprovisioned) user's token REJECTS regardless of token validity,
/// exactly record 03 §2 ("a deactivated user's token REJECTS at the
/// resolver (fail closed), regardless of token validity") and §4 (the
/// 5-minute SLA is a propagation-window RECOMMENDATION; the enforcement
/// point is this check).
///
/// Composition discipline (the fail-closed posture):
/// - `DenyAllResolver` REMAINS the shipped default; composing THIS
///   resolver is a deployment act (the wrapper around the opted-in
///   production path).
/// - Composing this resolver opts the deployment into SCIM-managed
///   identity: a subject WITHOUT a provisioned, active user record in the
///   connection's tenant DENIES (fail closed — a token cannot ride on an
///   identity the provisioning state does not vouch for). This is the
///   recorded FIM decision for the seam; the subject↔userName matching
///   key is `user_name == ctx.subject()` (the recorded convention; the
///   IdP-`sub`-vs-`userName` detail is UNVERIFIED-against-RFC-7644).
/// - The role is the SCIM group→role mapping's FIRST hit through the
///   closed vocabulary (`ScimStore::resolve_role_for_user` — the single
///   home, record 03 §3). A user in NO mapped group denies (never
///   defaults to Viewer — the P13.02 rule). The inner resolver's role is
///   REPLACED, never unioned.
pub struct ScimAwareIdentityResolver {
    inner: Box<dyn IdentityResolver>,
    store: Arc<dyn ScimStore>,
    /// The tenant of the SCIM connection (every lookup is tenant-scoped).
    tenant: TenantId,
}

impl ScimAwareIdentityResolver {
    /// Builds the enforcing resolver over the injected inner path and
    /// store.
    pub fn new(
        inner: Box<dyn IdentityResolver>,
        store: Arc<dyn ScimStore>,
        tenant: TenantId,
    ) -> Self {
        Self {
            inner,
            store,
            tenant,
        }
    }
}

impl IdentityResolver for ScimAwareIdentityResolver {
    fn resolve(&self, bearer: Option<&str>) -> Result<Caller, AuthError> {
        // 1. The inner identity path must accept the token first (a bad
        //    token denies with the inner error, before any SCIM state is
        //    consulted).
        let caller = self.inner.resolve(bearer)?;
        // 2. The subject must be a provisioned, ACTIVE user of the
        //    connection's tenant (the deprovisioning gate). A store
        //    failure denies — never degrades to "let it through".
        let user = self
            .store
            .get_user_by_user_name(&self.tenant, caller.ctx.subject())
            .map_err(|_| AuthError::InvalidCredentials)?
            .ok_or(AuthError::InvalidCredentials)?;
        if !user.active {
            return Err(AuthError::InvalidCredentials);
        }
        // 3. The role: the SCIM group→role mapping (first mappable group
        //    wins; no union; no Viewer default).
        let role = self
            .store
            .resolve_role_for_user(&self.tenant, &user.id)
            .map_err(|_| AuthError::InvalidCredentials)?
            .ok_or(AuthError::InvalidCredentials)?;
        Ok(Caller {
            ctx: caller.ctx,
            role,
        })
    }
}

// The InMemoryScimStore is NOT imported here (the store is public; the
// HTTP layer composes whatever `ScimStore` implementation the deployment
// supplies). The module's public surface is exactly the endpoint set + the
// deprovisioning enforcement resolver.

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicI64, Ordering};

    use crate::scim_store::InMemoryScimStore;

    const TENANT_A_UUID: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f8a01";

    fn tenant() -> TenantId {
        TenantId::new(flexcore_contracts::ids::FlexUuid::parse(TENANT_A_UUID).unwrap())
    }

    fn service() -> Arc<ScimService> {
        let counter = Arc::new(AtomicI64::new(0));
        let clock = Arc::new(AtomicI64::new(1_000));
        Arc::new(ScimService::new(
            Arc::new(InMemoryScimStore::default()),
            tenant(),
            Box::new(move || {
                format!("scim-{}", counter.fetch_add(1, Ordering::SeqCst))
            }),
            Box::new(move || clock.load(Ordering::SeqCst)),
        ))
    }

    #[test]
    fn the_filter_parser_accepts_only_the_recorded_minimal_shape() {
        // The accepted shape: userName Eq "<value>".
        assert_eq!(
            parse_list_params(&ScimListParams {
                start_index: None,
                count: None,
                filter: Some(r#"userName Eq "alice""#.to_string()),
            })
            .expect("the accepted filter parses")
            .user_name_eq,
            Some("alice".to_string())
        );
        // Case-insensitive attribute and operator spelling.
        assert!(parse_list_params(&ScimListParams {
            start_index: None,
            count: None,
            filter: Some(r#"USERNAME eq "bob""#.to_string()),
        })
        .is_ok());
        // Everything else is a named rejection (never a silent pass-all).
        for unsupported in [
            r#"userName Ne "alice""#,
            r#"displayName Eq "alice""#,
            r#"userName Eq alice"#,
            r#"userName Co "ali""#,
            "userName Eq",
        ] {
            assert!(
                parse_list_params(&ScimListParams {
                    start_index: None,
                    count: None,
                    filter: Some(unsupported.to_string()),
                })
                .is_err(),
                "the filter '{unsupported}' must be refused"
            );
        }
    }

    #[test]
    fn the_router_exposes_exactly_the_pinned_endpoint_set() {
        // Compile-level pin (the same discipline as the domain tests): the
        // router is built by `scim_router` from exactly the four pinned
        // paths; there is no other route on it. The assertion is the
        // construction itself — axum panics on a duplicate route, and the
        // paths are the module constants.
        let _router = scim_router(service());
        assert_eq!(SCIM_USERS_PATH, "/scim/v2/Users");
        assert_eq!(SCIM_USER_PATH, "/scim/v2/Users/{id}");
        assert_eq!(SCIM_GROUPS_PATH, "/scim/v2/Groups");
        assert_eq!(SCIM_SPC_PATH, "/scim/v2/ServiceProviderConfig");
    }
}
