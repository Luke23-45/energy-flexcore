// FIM wave 8B (Agent B) — the API-KEY route surface (P13.08 / FIM-F08.02):
// HTTP exposure of the EXISTING `crate::api_keys::ApiKeyService` — the
// wiring wave's named remaining step ("NO ROUTE serves it yet",
// `state.rs` / `main.rs::fim_compose`).
//
// **Implemented under FIM wave 8B, WRITTEN-UNRUN.** This module has never
// been compiled or executed: no key has ever been issued, rotated,
// authenticated, or revoked over HTTP. The first human build/test is the
// gate (program conventions §1). LIVE CALLER INTEROP (a real integration
// authenticating with an issued key) IS UNPROVEN — **HUMAN LAB GATE**.
//
// THE GATE (documented per the wave contract): this surface mounts ONLY
// when the API-key service is composed in the state — and the wiring wave
// composes it ONLY when the `FLEXCORE_API_KEYS_ENABLED` configuration
// section is present (optional
// `FLEXCORE_API_KEYS_ROTATION_GRACE_SECONDS`, default the registered 24 h
// recommendation; `main.rs::fim_compose`). Absent the gate the service
// stays `None` and `routes.rs::fim_router` mounts NO routes at all — the
// paths do not exist (not a 404 fallback), exactly the recorded SCIM
// posture.
//
// THE ENDPOINTS (record 08 §2's semantics consumed by citation — the
// one-time secret display, the digest-at-rest rule, the rotation overlap,
// the immediate revocation are the SERVICE's, unchanged):
//
// | Endpoint | Method | Authentication | Authorization |
// |---|---|---|---|
// | `/api/v1/api-keys` | POST | the standard caller path (`CallerExtractor` → the state's `IdentityResolver`; 401 when absent) | the EXISTING `apikey.create` action at tenant scope |
// | `/api/v1/api-keys/{key_id}/rotate` | POST | the standard caller path | the EXISTING `apikey.rotate` action at tenant scope |
// | `/api/v1/api-keys/{key_id}/revoke` | POST | the standard caller path | the EXISTING `apikey.revoke` action at tenant scope |
// | `/api/v1/api-keys/self` | GET | the presented `fk_` key literal IS the credential (bearer — the module's recorded wire form, `api_keys.rs` header point 2) | none beyond the key authentication itself (an integration's "who am I") |
//
// The four actions are the RECORDED `apikey.*` grant set of
// `flexcore_authz::phase13_policies` (PlatformAdmin/TenantAdmin at tenant
// scope). The SHIPPED policy composition (`phase5_default_policies`)
// grants NONE of them (FRV1-R2-009 composes phase13_policies deliberately
// NOT), so the admin routes deny for every role under the shipped defaults
// — deny-by-default preserved; a deployment that composes the recorded
// phase-13 grant set operates them. No new action is defined; the surface
// never widens the closed `Action` vocabulary.
//
// NEVER-GRANTED SET (pinned by the routes exactly as by the service): a
// scope string that is not on the closed `Action` surface — which
// structurally includes `dispatch.authorize`, `dispatch.issue`,
// `secret.read`, `network.any` — is refused at issuance/rotation by the
// SERVICE (`InvalidScope`) and the route maps that to a 422. Nothing is
// written for a refused issuance.
//
// ERROR MAPPING (the EXISTING `crate::error::ApiError` vocabulary only):
// record/scope refusals → 422 `validation_rejected`; unknown key on a
// tenant-scoped admin path → 404 `not_found` (no existence leak); rotation
// of a revoked key → 409 `conflict` (the request is valid but conflicts
// with the resource's state); every authentication refusal on the `self`
// endpoint → 401 `unauthenticated` (ONE bare shape — malformed, unknown,
// revoked, and superseded-past-grace keys are indistinguishable; no
// oracle); store failures → the opaque 500 `infrastructure_failure`. No
// error ever echoes a presented key.
//
// SECRET POSTURE (the wave contract's hardest requirement): the RAW key is
// returned ONCE — in the issue response and in each rotate response — and
// NEVER in any other response. The response shape makes that explicit and
// impossible to log: (1) the field is named `secret_literal` inside an
// explicit `secret_handling: "shown-once"` marker, so a consumer cannot
// mistake it for a repeatable value; (2) the response carries
// `Cache-Control: no-store` / `Pragma: no-cache` (the RFC 6749 §5.1
// credential-response posture, consumed by citation); (3) the DTO carrying
// the literal deliberately does NOT implement `Debug` (nothing can
// log-render it — the service-side pins in `api_keys.rs` tests assert the
// same for stored records); (4) the P1.20 middleware logs no
// request/response bodies (metrics.rs recorded rule); (5) every subsequent
// response carries the 8-character FINGERPRINT only (the record 08 §4
// display rule), never the literal.
//
// BOUNDED BODIES: the composition's `RequestBodyLimitLayer` (1 MiB) bounds
// every request, and the handlers add their own structural bounds (a
// 64-entry scope list, bounded scope strings) — an over-bounded request is
// a 422, never a silent truncation.

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use utoipa::ToSchema;

use flexcore_authz::action::Action;

use crate::api_keys::ApiKeyError;
use crate::auth::{authorize_tenant_scoped, CallerExtractor};
use crate::dto::DataEnvelope;
use crate::error::ApiError;

/// The issuance endpoint (record 08 §2: keys are tenant-scoped credentials
/// issued by tenant/platform administrators).
pub const API_KEYS_PATH: &str = "/api/v1/api-keys";
/// The rotation endpoint (record 08 §2: rotate = a new secret for the SAME
/// key id; the old secret is superseded with a grace window).
pub const API_KEY_ROTATE_PATH: &str = "/api/v1/api-keys/{key_id}/rotate";
/// The immediate-revocation endpoint (record 08 §2: "no works-until-
/// refresh window for a revoked key").
pub const API_KEY_REVOKE_PATH: &str = "/api/v1/api-keys/{key_id}/revoke";
/// The self-authentication endpoint: the presented `fk_` literal is the
/// credential; the response is the resolved principal.
pub const API_KEY_SELF_PATH: &str = "/api/v1/api-keys/self";

/// The structural bound on a submitted scope list (a route-level bound on
/// top of the service's closed-surface validation).
pub const MAX_SCOPES_PER_REQUEST: usize = 64;
/// The structural bound on one scope string.
pub const MAX_SCOPE_LENGTH: usize = 64;

/// The explicit one-time marker carried by every response that contains a
/// raw key literal (`secret_handling`). A consumer cannot mistake the
/// field for a repeatable value, and a log-scanner can grep exactly this
/// marker.
pub const SECRET_HANDLING_SHOWN_ONCE: &str = "shown-once";

/// The revoked-key state value the revoke response renders (the closed
/// `KEY_STATES` vocabulary of `api_keys.rs`).
pub const API_KEY_REVOKED_STATE: &str = "REVOKED";

// ---------------------------------------------------------------------------
// Request/response bodies
// ---------------------------------------------------------------------------

/// The issuance request: the requested scope subset, each entry a
/// canonical action id on the closed surface.
#[derive(Debug, serde::Deserialize, ToSchema)]
pub struct IssueApiKeyBody {
    pub scopes: Vec<String>,
}

/// The issue/rotate response. Carries the RAW key literal — shown ONCE,
/// never stored retrievable (record 08 §6.4: a lost key is rotated, not
/// recovered). Deliberately NOT `Debug` (nothing can log-render this
/// type); the response carries the no-store cache headers.
#[derive(serde::Serialize, ToSchema)]
pub struct IssuedApiKeyDto {
    /// The public key id (`fkid_...`; safe to keep, log, audit).
    pub key_id: String,
    /// The RAW secret literal — present in THIS response ONLY. Store it
    /// now; it cannot be recovered.
    pub secret_literal: String,
    /// The explicit one-time marker ([`SECRET_HANDLING_SHOWN_ONCE`],
    /// rendered at construction).
    pub secret_handling: String,
    /// The display fingerprint (the first 8 characters of the secret's
    /// payload) — the form every subsequent rendering carries.
    pub fingerprint: String,
    /// The key's scope subset, as canonical action ids.
    pub scopes: Vec<String>,
}

/// The authenticated principal (the `self` endpoint's response): the
/// STORE-resolved tenant and scope subset. No credential material — the
/// raw key appears in the request's bearer header only, never in any
/// response.
#[derive(Debug, serde::Serialize, ToSchema)]
pub struct ApiKeyPrincipalDto {
    pub key_id: String,
    pub tenant_id: String,
    /// The granted actions, as canonical action ids (the closed surface —
    /// the never-granted names cannot appear here).
    pub scopes: Vec<String>,
}

/// The revocation response (the record's public fields; no credential
/// material).
#[derive(Debug, serde::Serialize, ToSchema)]
pub struct ApiKeyRevokedDto {
    pub key_id: String,
    /// The post-revocation state (`REVOKED` — immediate for both the
    /// current and any superseded secret; the constant rendered at
    /// construction, [`API_KEY_REVOKED_STATE`]).
    pub state: String,
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

/// Builds the API-key router over the composed state. Mounted by
/// `routes.rs::fim_router` ONLY when `state.api_keys` is composed (which
/// happens only behind the `FLEXCORE_API_KEYS_ENABLED` gate); absent
/// composition this function is never called and the paths do not exist.
/// The router state is the `AppState` because the admin endpoints
/// authenticate through the SAME `CallerExtractor` / policy evaluation
/// every other route uses — no parallel auth plumbing.
pub fn api_key_router(state: crate::state::AppState) -> Router {
    Router::new()
        .route(API_KEYS_PATH, post(issue_api_key))
        .route(API_KEY_ROTATE_PATH, post(rotate_api_key))
        .route(API_KEY_REVOKE_PATH, post(revoke_api_key))
        .route(API_KEY_SELF_PATH, get(authenticate_api_key))
        .with_state(state)
}

/// The composed service (defensive: `fim_router` mounts this router only
/// when the service is composed; a directly-constructed miscomposition
/// fails closed instead of serving anything).
fn composed_service(
    state: &crate::state::AppState,
) -> Result<std::sync::Arc<crate::api_keys::ApiKeyService>, ApiError> {
    state.api_keys.clone().ok_or_else(ApiError::infrastructure)
}

/// The shared route-level structural bounds (see the module header).
fn bounded_scopes(scopes: &[String]) -> Result<(), ApiError> {
    if scopes.len() > MAX_SCOPES_PER_REQUEST {
        return Err(ApiError::validation(format!(
            "scopes exceeds the bound of {MAX_SCOPES_PER_REQUEST} entries"
        )));
    }
    for scope in scopes {
        if scope.len() > MAX_SCOPE_LENGTH {
            return Err(ApiError::validation(format!(
                "a scope string exceeds the bound of {MAX_SCOPE_LENGTH} characters"
            )));
        }
    }
    Ok(())
}

/// The shared error mapping (the EXISTING `ApiError` vocabulary; the named
/// [`ApiKeyError`] class never reaches the client — only its HTTP class
/// does; no variant echoes a presented key).
///
/// `admin_path` distinguishes the two contexts the SAME service refusal
/// can arise in: on a tenant-scoped ADMIN path an unknown key is a 404 (no
/// existence leak), while on the `self` authentication path every refusal
/// is ONE bare 401 (no oracle about which refusal shape failed).
fn map_api_key_error(error: ApiKeyError, admin_path: bool) -> ApiError {
    match error {
        ApiKeyError::InvalidRecord { .. } | ApiKeyError::InvalidScope { .. } => {
            ApiError::validation(error.to_string())
        }
        ApiKeyError::MalformedKey | ApiKeyError::SupersededPastGrace => {
            ApiError::unauthenticated()
        }
        ApiKeyError::UnknownKey => {
            if admin_path {
                ApiError::not_found()
            } else {
                ApiError::unauthenticated()
            }
        }
        ApiKeyError::KeyRevoked => {
            if admin_path {
                // The request is well-formed but conflicts with the
                // resource's current state (the ApiError::conflict rule).
                ApiError::conflict(
                    "api_key_revoked",
                    "the API key is revoked; a revoked key cannot be rotated (immediate \
                     revocation, record 08 §2)"
                        .to_string(),
                )
            } else {
                ApiError::unauthenticated()
            }
        }
        // Not reachable from these routes (the routes never evaluate an
        // API-key principal against the policy set); the defensive mapping
        // denies.
        ApiKeyError::ScopeNotGranted { .. } => ApiError::forbidden(&error.to_string()),
        ApiKeyError::Backend { code, message } => {
            // The named duplicate/missing codes map onto the conflict /
            // not-found classes with CONSTRUCTED messages (never the
            // backend text); everything else stays opaque (the P1.17
            // rule: no backend text is ever leaked).
            let _ = message;
            match code.as_str() {
                "E_KEY_EXISTS" => ApiError::conflict(
                    "api_key_exists",
                    "an API key with this id is already registered; issuance is explicit, \
                     never an upsert"
                        .to_string(),
                ),
                "E_KEY_MISSING" => ApiError::not_found(),
                _ => ApiError::infrastructure(),
            }
        }
    }
}

/// The bearer value of the `Authorization` header (trimmed, non-empty);
/// `None` for an absent, malformed, or empty credential.
fn bearer_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|raw| raw.strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(|token| token.to_string())
}

/// Wraps a credential-bearing body (issue / rotate) with the no-store
/// cache posture (the RFC 6749 §5.1 credential-response posture, consumed
/// by citation — a raw key literal is never cacheable).
fn credential_response<T: serde::Serialize>(
    status: StatusCode,
    body: T,
) -> Result<Response, ApiError> {
    let mut response = (status, Json(body)).into_response();
    response.headers_mut().insert(
        axum::http::header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-store"),
    );
    response.headers_mut().insert(
        axum::http::header::PRAGMA,
        axum::http::HeaderValue::from_static("no-cache"),
    );
    Ok(response)
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// POST `/api/v1/api-keys` — issues a NEW key for the CALLER'S tenant. The
/// raw literal is in THIS response ONLY (once), with the explicit
/// `secret_handling` marker and the no-store posture.
async fn issue_api_key(
    State(state): State<crate::state::AppState>,
    CallerExtractor(caller): CallerExtractor,
    Json(body): Json<IssueApiKeyBody>,
) -> Result<Response, ApiError> {
    authorize_tenant_scoped(&state.policies, &caller, Action::ApiKeyCreate)?;
    bounded_scopes(&body.scopes)?;
    let service = composed_service(&state)?;
    let issued = service
        .issue_key(caller.ctx.tenant_id(), body.scopes)
        .map_err(|error| map_api_key_error(error, true))?;
    credential_response(
        StatusCode::CREATED,
        DataEnvelope {
            data: IssuedApiKeyDto {
                key_id: issued.key_id,
                secret_literal: issued.secret_literal,
                secret_handling: SECRET_HANDLING_SHOWN_ONCE.to_string(),
                fingerprint: issued.fingerprint,
                scopes: issued.scopes,
            },
        },
    )
}

/// POST `/api/v1/api-keys/{key_id}/rotate` — rotates a key: a NEW secret
/// for the SAME key id; the old secret keeps verifying until the
/// configured grace window closes (record 08 §2). The new literal is in
/// THIS response ONLY (once).
async fn rotate_api_key(
    State(state): State<crate::state::AppState>,
    CallerExtractor(caller): CallerExtractor,
    axum::extract::Path(key_id): axum::extract::Path<String>,
) -> Result<Response, ApiError> {
    authorize_tenant_scoped(&state.policies, &caller, Action::ApiKeyRotate)?;
    let service = composed_service(&state)?;
    let issued = service
        .rotate_key(&caller.ctx.tenant_id(), &key_id)
        .map_err(|error| map_api_key_error(error, true))?;
    credential_response(
        StatusCode::OK,
        DataEnvelope {
            data: IssuedApiKeyDto {
                key_id: issued.key_id,
                secret_literal: issued.secret_literal,
                secret_handling: SECRET_HANDLING_SHOWN_ONCE.to_string(),
                fingerprint: issued.fingerprint,
                scopes: issued.scopes,
            },
        },
    )
}

/// POST `/api/v1/api-keys/{key_id}/revoke` — revokes a key IMMEDIATELY
/// (record 08 §2: the next authentication attempt with either secret
/// refuses). Unknown key → 404 (no existence leak).
async fn revoke_api_key(
    State(state): State<crate::state::AppState>,
    CallerExtractor(caller): CallerExtractor,
    axum::extract::Path(key_id): axum::extract::Path<String>,
) -> Result<Json<DataEnvelope<ApiKeyRevokedDto>>, ApiError> {
    authorize_tenant_scoped(&state.policies, &caller, Action::ApiKeyRevoke)?;
    let service = composed_service(&state)?;
    service
        .revoke_key(&caller.ctx.tenant_id(), &key_id)
        .map_err(|error| map_api_key_error(error, true))?;
    Ok(Json(DataEnvelope {
        data: ApiKeyRevokedDto {
            key_id,
            state: API_KEY_REVOKED_STATE.to_string(),
        },
    }))
}

/// GET `/api/v1/api-keys/self` — the presented `fk_` literal IS the
/// credential (the module's recorded wire form). The response is the
/// resolved principal (the tenant comes from the STORE row, never from the
/// request); on SUCCESS the service records the last-used evidence on the
/// key row. Every refusal is ONE bare 401.
async fn authenticate_api_key(
    State(state): State<crate::state::AppState>,
    headers: HeaderMap,
    uri: Uri,
) -> Result<Json<DataEnvelope<ApiKeyPrincipalDto>>, ApiError> {
    let presented = bearer_token(&headers).ok_or_else(ApiError::unauthenticated)?;
    let service = composed_service(&state)?;
    // The subject path the caller presented (the evidence field record 08
    // §2 prescribes; recorded on success only).
    let principal = service
        .authenticate(&presented, uri.path())
        .map_err(|error| map_api_key_error(error, false))?;
    Ok(Json(DataEnvelope {
        data: ApiKeyPrincipalDto {
            key_id: principal.key_id,
            tenant_id: principal.tenant_id.uuid().to_hyphenated(),
            scopes: principal
                .scopes
                .iter()
                .map(|action| action.id().to_string())
                .collect(),
        },
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn http_class(error: &ApiError) -> String {
        error.to_string()
    }

    #[test]
    fn the_error_mapping_stays_in_the_existing_vocabulary() {
        let validation = map_api_key_error(ApiKeyError::InvalidScope { index: 0 }, true);
        assert!(http_class(&validation).contains("422"));
        // The self path: every refusal is one bare 401 (no oracle).
        for refusal in [
            ApiKeyError::MalformedKey,
            ApiKeyError::UnknownKey,
            ApiKeyError::KeyRevoked,
            ApiKeyError::SupersededPastGrace,
        ] {
            let mapped = map_api_key_error(refusal.clone(), false);
            assert!(
                http_class(&mapped).contains("401"),
                "every authentication refusal is one bare 401: {refusal:?}"
            );
        }
        // The admin path: unknown key is a 404 (no existence leak); a
        // revoked key on rotate is a 409 conflict.
        assert!(http_class(&map_api_key_error(ApiKeyError::UnknownKey, true)).contains("404"));
        let conflict = map_api_key_error(ApiKeyError::KeyRevoked, true);
        assert!(http_class(&conflict).contains("409"));
        assert!(http_class(&conflict).contains("revoked"));
        // Backend failures: the named codes map with CONSTRUCTED messages;
        // everything else stays opaque.
        let exists = map_api_key_error(
            ApiKeyError::Backend {
                code: "E_KEY_EXISTS".to_string(),
                message: "backend text that must never travel".to_string(),
            },
            true,
        );
        assert!(http_class(&exists).contains("409"));
        assert!(!http_class(&exists).contains("must never travel"));
        assert!(http_class(&map_api_key_error(
            ApiKeyError::Backend {
                code: "E_KEY_MISSING".to_string(),
                message: "backend text".to_string(),
            },
            true
        ))
        .contains("404"));
        let opaque = map_api_key_error(
            ApiKeyError::Backend {
                code: "E_OTHER".to_string(),
                message: "backend text that must never travel".to_string(),
            },
            true,
        );
        assert!(http_class(&opaque).contains("500"));
        assert!(!http_class(&opaque).contains("must never travel"));
        // No mapping ever renders a presented key.
        assert!(!http_class(&map_api_key_error(ApiKeyError::MalformedKey, false))
            .contains("fk_"));
    }

    #[test]
    fn the_route_paths_live_under_the_versioned_api_prefix() {
        for path in [
            API_KEYS_PATH,
            API_KEY_ROTATE_PATH,
            API_KEY_REVOKE_PATH,
            API_KEY_SELF_PATH,
        ] {
            assert!(
                path.starts_with("/api/v1/api-keys"),
                "the surface lives under the versioned api-keys prefix: {path}"
            );
        }
    }

    #[test]
    fn the_structural_bounds_and_markers_are_declared_constants() {
        assert!(MAX_SCOPES_PER_REQUEST > 0);
        assert!(MAX_SCOPE_LENGTH > 0);
        assert_eq!(SECRET_HANDLING_SHOWN_ONCE, "shown-once");
        assert_eq!(API_KEY_REVOKED_STATE, "REVOKED");
    }

    #[test]
    fn the_one_time_marker_names_the_once_only_semantics() {
        // The wave contract: "the response shape must make that explicit".
        // The marker is a stable grep-able constant on every
        // credential-bearing response, and the revoked state renders the
        // closed KEY_STATES vocabulary value verbatim.
        assert!(crate::api_keys::KEY_STATES.contains(&API_KEY_REVOKED_STATE));
    }

    #[test]
    fn the_bearer_extraction_is_strict() {
        let mut headers = HeaderMap::new();
        assert_eq!(bearer_token(&headers), None);
        headers.insert(
            axum::http::header::AUTHORIZATION,
            axum::http::HeaderValue::from_static("Bearer fk_abc"),
        );
        assert_eq!(bearer_token(&headers).as_deref(), Some("fk_abc"));
        headers.insert(
            axum::http::header::AUTHORIZATION,
            axum::http::HeaderValue::from_static("bearer fk_abc"),
        );
        assert_eq!(
            bearer_token(&headers),
            None,
            "the scheme is case-sensitive (the scim_bearer_guard pattern)"
        );
    }
}
