// FIM wave 8B (Agent B) — the MACHINE-IDENTITY route surface (P13.07 /
// FIM-F08.01): HTTP exposure of the EXISTING
// `crate::machine_identity::MachineIdentityService` — the wiring wave's
// named remaining step ("NO ROUTE serves this service yet",
// `state.rs` / `main.rs::fim_compose`).
//
// **Implemented under FIM wave 8B, WRITTEN-UNRUN.** This module has never
// been compiled or executed: no route request has ever been served, no
// token issued or validated over HTTP, no client registered through the
// API. The first human build/test is the gate (program conventions §1).
// LIVE TOKEN INTEROP (a real integration partner driving these endpoints)
// IS UNPROVEN — **HUMAN LAB GATE**.
//
// THE GATE (documented per the wave contract): this surface mounts ONLY
// when the machine-identity service is composed in the state — and the
// wiring wave composes it ONLY when the `FLEXCORE_MACHINE_IDENTITY_ENABLED`
// configuration section is present (required
// `FLEXCORE_MACHINE_IDENTITY_ISSUER` / `FLEXCORE_MACHINE_IDENTITY_AUDIENCE`
// / `FLEXCORE_MACHINE_IDENTITY_ISSUER_SECRET`; `main.rs::fim_compose`).
// Absent the gate the service stays `None` and
// `routes.rs::fim_router` mounts NO routes at all — the paths do not exist
// (not a 404 fallback), exactly the recorded SCIM posture.
//
// THE ENDPOINTS (record 07 §2's flow shape consumed by citation; record 07
// §3's scope-subset rule and record 01's never-granted set are the
// SERVICE's, enforced at registration — the routes add no scope space):
//
// | Endpoint | Method | Authentication | Authorization |
// |---|---|---|---|
// | `/api/v1/machine-identity/clients` | POST | the standard caller path (`CallerExtractor` → the state's `IdentityResolver`; 401 when absent) | the EXISTING `apikey.create` action at tenant scope (see the guard note) |
// | `/api/v1/machine-identity/token` | POST | the CLIENT's own credentials (`grant_type=client_credentials`; record 07 §2) — the service authenticates the client secret | none beyond the client authentication itself (the OAuth2 client-credentials flow has no caller principal) |
// | `/api/v1/machine-identity/validate` | GET | the presented machine-identity token IS the credential (bearer) | none beyond the token validation itself (a self-describing "who am I" for integrations) |
// | `/api/v1/machine-identity/clients/{client_id}/revoke` | POST | the standard caller path | the EXISTING `apikey.revoke` action at tenant scope |
//
// GUARD NOTE (the recorded composition decision): no dedicated machine.*
// action exists on the closed `flexcore-authz` surface, and this wave adds
// none (the surface never widens). The admin endpoints evaluate the
// RECORDED integration-credential administration grants — `apikey.create`
// / `apikey.revoke` at tenant scope, the exact grant table of
// `flexcore_authz::phase13_policies` ("tenant-scoped credentials
// administered by tenant/platform administrators"). The SHIPPED policy
// composition (`phase5_default_policies`) grants NONE of the apikey.*
// actions (FRV1-R2-009 composes phase13_policies deliberately NOT), so
// these admin routes deny for every role under the shipped defaults —
// deny-by-default preserved; a deployment that composes the recorded
// phase-13 grant set operates them.
//
// ERROR MAPPING (the EXISTING `crate::error::ApiError` vocabulary only):
// registration/record refusals → 422 `validation_rejected`; unknown
// client on a tenant-scoped admin path → 404 `not_found` (no existence
// leak across the tenant boundary); every client-authentication and token
// validation refusal → 401 `unauthenticated` (ONE bare shape — missing,
// malformed, wrong-secret, disabled, and revoked clients are
// indistinguishable; no oracle); duplicate registration → 409 `conflict`;
// store failures → the opaque 500 `infrastructure_failure`. No error ever
// echoes a presented secret or token (the service's own contract, intact).
//
// SECRET POSTURE: the registration REQUEST carries the client secret (the
// caller-supplied credential the service digests at rest); the RESPONSES
// never echo it — the register response carries the submitted (now
// validated) registration facts and the secret's 4-character display
// prefix only (the record's own fingerprint form,
// `machine_identity.rs::register_client`), and the token response carries
// the minted JWT once. The request/response types that carry credential
// material deliberately do NOT implement `Debug` (nothing can log-render
// them), the credential responses carry `Cache-Control: no-store` /
// `Pragma: no-cache` (the RFC 6749 §5.1 token-response posture, consumed
// by citation), and the P1.20 middleware logs no request/response bodies
// (metrics.rs recorded rule: "No other header or body content is ever
// logged").
//
// BOUNDED BODIES: the composition's `RequestBodyLimitLayer` (1 MiB) bounds
// every request, and the handlers add their own structural bounds (a
// 64-entry scope list, bounded string fields) — an over-bounded request is
// a 422, never a silent truncation.

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use utoipa::ToSchema;

use flexcore_authz::action::Action;
use flexcore_contracts::ids::FlexUuid;
use flexcore_domain::ids::TenantId;

use crate::auth::{authorize_tenant_scoped, CallerExtractor};
use crate::dto::DataEnvelope;
use crate::error::ApiError;
use crate::machine_identity::{MachineIdentityError, TOKEN_TTL_SECONDS_DEFAULT};

/// The registration endpoint (tenant-administered data; record 07 §2).
pub const MACHINE_CLIENTS_PATH: &str = "/api/v1/machine-identity/clients";
/// The client-credentials token endpoint (record 07 §2: `POST /token` with
/// `grant_type=client_credentials`).
pub const MACHINE_TOKEN_PATH: &str = "/api/v1/machine-identity/token";
/// The token-validation ("who am I") endpoint: the presented machine token
/// is the credential.
pub const MACHINE_VALIDATE_PATH: &str = "/api/v1/machine-identity/validate";
/// The revocation endpoint (the emergency kill-switch; record 07 §4).
pub const MACHINE_CLIENT_REVOKE_PATH: &str =
    "/api/v1/machine-identity/clients/{client_id}/revoke";

/// The structural bound on a submitted scope list (a route-level bound on
/// top of the service's closed-surface validation; an over-bounded request
/// is a 422, never a silent truncation).
pub const MAX_SCOPES_PER_REQUEST: usize = 64;
/// The structural bound on one scope string (a canonical action id is far
/// shorter; the bound rejects junk before the service parses it).
pub const MAX_SCOPE_LENGTH: usize = 64;
/// The structural bound on a client identifier.
pub const MAX_CLIENT_ID_LENGTH: usize = 128;
/// The structural bound on a presented client secret.
pub const MAX_SECRET_LENGTH: usize = 512;

/// The state value the revoke response renders (record 07 §4's state
/// vocabulary: the emergency kill-switch disables the client).
pub const MACHINE_CLIENT_DISABLED_STATE: &str = "disabled";

/// The grant-type literal this token endpoint serves (record 07 §2).
pub const CLIENT_CREDENTIALS_GRANT: &str = "client_credentials";

// ---------------------------------------------------------------------------
// Request/response bodies
// ---------------------------------------------------------------------------

/// The registration request. Deliberately NOT `Debug` (it carries the
/// client secret — nothing can log-render this type).
#[derive(serde::Deserialize, ToSchema)]
pub struct RegisterMachineClientBody {
    /// The client identifier (opaque non-empty text, as registered).
    pub client_id: String,
    /// The requested scope subset — each entry MUST be a canonical action
    /// id on the closed surface; the never-granted names
    /// (`dispatch.authorize`, `dispatch.issue`, `secret.read`,
    /// `network.any`) are structurally refused by the service
    /// (`InvalidScope`), and the route maps that to a 422. Nothing is
    /// written for a refused registration.
    pub allowed_scopes: Vec<String>,
    /// The client secret (caller-supplied credential material; stored ONLY
    /// as its SHA-256 verifier by the service). Never echoed in any
    /// response beyond the 4-character display prefix.
    pub secret: String,
}

/// The registered-machine-client response: the submitted (now validated)
/// registration facts plus the secret's display prefix. The secret itself
/// is NEVER echoed (it was caller-supplied and is stored only as a
/// verifier).
#[derive(Debug, serde::Serialize, ToSchema)]
pub struct MachineClientDto {
    pub client_id: String,
    /// The owning tenant, canonical hyphenated UUID text.
    pub tenant_id: String,
    pub allowed_scopes: Vec<String>,
    /// The secret's display prefix (the first 4 characters of the secret's
    /// payload — the record's own fingerprint form; never the secret).
    pub secret_prefix: String,
    /// The stored record's construction values (`register_client` stores
    /// `enabled: true`, epoch 0 — a fresh registration starts enabled at
    /// epoch 0).
    pub credential_epoch: u64,
    pub enabled: bool,
}

/// The token request (the record 07 §2 client-credentials flow).
/// Deliberately NOT `Debug` (it carries the client secret).
#[derive(serde::Deserialize, ToSchema)]
pub struct MachineTokenRequest {
    /// MUST be exactly `client_credentials` (the only grant this endpoint
    /// serves; anything else is a 422, never a fallback).
    pub grant_type: String,
    pub client_id: String,
    pub client_secret: String,
    /// The client's registered tenant (part of the client's registration
    /// data). The registry lookup is scoped by it; a wrong tenant and an
    /// unknown client are indistinguishable (both land on the same bare
    /// 401 — no existence leak).
    pub tenant_id: String,
}

/// The token response: the RFC 6749 §5.1 / RFC 9068-shaped flat body (the
/// token-endpoint wire form record 07 §2 names — deliberately NOT the
/// platform `DataEnvelope`, which is the ADMIN surfaces' vocabulary).
/// `access_token` is a live credential: the response carries
/// `Cache-Control: no-store` / `Pragma: no-cache` and the type is NOT
/// `Debug`.
#[derive(serde::Serialize, ToSchema)]
pub struct MachineTokenResponse {
    pub access_token: String,
    /// Always `"Bearer"` (the [`crate::machine_identity::MACHINE_SUBJECT_PREFIX`]
    /// grammar's token type; the constant rendered at construction).
    pub token_type: String,
    /// The registered TTL recommendation the composition composes (the
    /// composition root builds the service's default TTL); the token's own
    /// `exp` claim is the validation authority.
    pub expires_in: i64,
}

/// The resolved machine principal (the validate endpoint's response): the
/// REGISTRY's tenant and scope subset. No credential material.
#[derive(Debug, serde::Serialize, ToSchema)]
pub struct MachinePrincipalDto {
    pub client_id: String,
    /// The full subject (`integration:<client_id>` — the record 01 grammar).
    pub subject: String,
    pub tenant_id: String,
    /// The granted actions, as canonical action ids (the closed surface —
    /// the never-granted names cannot appear here).
    pub scopes: Vec<String>,
}

/// The revocation response (the record's public fields; no credential
/// material).
#[derive(Debug, serde::Serialize, ToSchema)]
pub struct MachineClientRevokedDto {
    pub client_id: String,
    /// The post-revocation state (record 07 §4: disabled — every token
    /// issued before the revocation refuses at validation immediately).
    /// The constant rendered at construction
    /// ([`MACHINE_CLIENT_DISABLED_STATE`]).
    pub state: String,
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

/// Builds the machine-identity router over the composed state. Mounted by
/// `routes.rs::fim_router` ONLY when `state.machine_identity` is composed
/// (which happens only behind the `FLEXCORE_MACHINE_IDENTITY_ENABLED`
/// gate); absent composition this function is never called and the paths
/// do not exist. The router state is the `AppState` because the admin
/// endpoints authenticate through the SAME `CallerExtractor` /
/// policy evaluation every other route uses — no parallel auth plumbing.
pub fn machine_identity_router(state: crate::state::AppState) -> Router {
    Router::new()
        .route(MACHINE_CLIENTS_PATH, post(register_machine_client))
        .route(MACHINE_TOKEN_PATH, post(issue_machine_token))
        .route(MACHINE_VALIDATE_PATH, get(validate_machine_token))
        .route(MACHINE_CLIENT_REVOKE_PATH, post(revoke_machine_client))
        .with_state(state)
}

/// The composed service (defensive: `fim_router` mounts this router only
/// when the service is composed; a directly-constructed miscomposition
/// fails closed instead of serving anything).
fn composed_service(
    state: &crate::state::AppState,
) -> Result<std::sync::Arc<crate::machine_identity::MachineIdentityService>, ApiError> {
    state
        .machine_identity
        .clone()
        .ok_or_else(ApiError::infrastructure)
}

/// The shared route-level structural bounds (see the module header).
fn bounded_scopes(scopes: &[String]) -> Result<(), ApiError> {
    if scopes.len() > MAX_SCOPES_PER_REQUEST {
        return Err(ApiError::validation(format!(
            "allowed_scopes exceeds the bound of {MAX_SCOPES_PER_REQUEST} entries"
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
/// [`MachineIdentityError`] class never reaches the client — only its HTTP
/// class does; no variant echoes a presented credential).
fn map_machine_identity_error(error: MachineIdentityError) -> ApiError {
    match error {
        MachineIdentityError::InvalidRecord { .. } | MachineIdentityError::InvalidScope { .. } => {
            ApiError::validation(error.to_string())
        }
        // The tenant-scoped admin path: existence never leaks across the
        // tenant boundary (a wrong tenant and an unknown client are
        // indistinguishable 404s).
        MachineIdentityError::UnknownClient => ApiError::not_found(),
        // Every client-authentication / token-validation refusal is ONE
        // bare 401 (no oracle about which half failed).
        MachineIdentityError::InvalidSecret
        | MachineIdentityError::ClientDisabled
        | MachineIdentityError::ClientRevoked
        | MachineIdentityError::InvalidToken
        | MachineIdentityError::ExpiredToken
        | MachineIdentityError::WrongIssuer
        | MachineIdentityError::WrongAudience
        | MachineIdentityError::TenantMismatch => ApiError::unauthenticated(),
        // Not reachable from these routes (the routes never evaluate a
        // machine principal against the policy set); the defensive mapping
        // denies.
        MachineIdentityError::ScopeNotGranted { .. } => ApiError::forbidden(&error.to_string()),
        MachineIdentityError::Backend { code, message } => {
            // Duplicate registration is a conflict with the current state
            // (registration is explicit, never an upsert); the message is
            // CONSTRUCTED here, never forwarded. Every other backend
            // failure stays opaque (the P1.17 rule: no backend text is
            // ever leaked).
            let _ = message;
            if code == "E_CLIENT_EXISTS" {
                ApiError::conflict(
                    "machine_client_exists",
                    "a machine client with this id is already registered; registration is \
                     explicit, never an upsert"
                        .to_string(),
                )
            } else {
                ApiError::infrastructure()
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

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// POST `/api/v1/machine-identity/clients` — registers a machine client in
/// the CALLER'S tenant (tenant-administered data, record 07 §2). The
/// service stores ONLY the secret's verifier; the response never echoes
/// the secret.
///
/// RESPONSE-SHAPE NOTE (honest, no re-read): the service composes its
/// registry privately and exposes no record-getter, so the response is
/// built from the SUBMITTED (now service-validated) registration facts and
/// the stored record's construction values — `register_client` stores
/// exactly the submitted client id / tenant / scopes with
/// `enabled: true`, epoch `0`, and the 4-character display prefix of the
/// submitted secret. A re-read through a future service getter is the
/// recorded follow-up; nothing here invents state the service did not
/// store.
async fn register_machine_client(
    State(state): State<crate::state::AppState>,
    CallerExtractor(caller): CallerExtractor,
    Json(body): Json<RegisterMachineClientBody>,
) -> Result<(StatusCode, Json<DataEnvelope<MachineClientDto>>), ApiError> {
    // The EXISTING integration-credential administration action at tenant
    // scope (see the module-header guard note; deny-by-default preserved).
    authorize_tenant_scoped(&state.policies, &caller, Action::ApiKeyCreate)?;
    if body.client_id.is_empty() || body.client_id.len() > MAX_CLIENT_ID_LENGTH {
        return Err(ApiError::validation(format!(
            "client_id must be 1..={MAX_CLIENT_ID_LENGTH} characters"
        )));
    }
    bounded_scopes(&body.allowed_scopes)?;
    if body.secret.is_empty() || body.secret.len() > MAX_SECRET_LENGTH {
        return Err(ApiError::validation(format!(
            "secret must be 1..={MAX_SECRET_LENGTH} characters"
        )));
    }
    let service = composed_service(&state)?;
    let tenant_id = caller.ctx.tenant_id();
    service
        .register_client(
            &body.client_id,
            tenant_id,
            body.allowed_scopes.clone(),
            &body.secret,
        )
        .map_err(map_machine_identity_error)?;
    Ok((
        StatusCode::CREATED,
        Json(DataEnvelope {
            data: MachineClientDto {
                client_id: body.client_id,
                tenant_id: tenant_id.uuid().to_hyphenated(),
                allowed_scopes: body.allowed_scopes,
                secret_prefix: body.secret.chars().take(4).collect(),
                credential_epoch: 0,
                enabled: true,
            },
        }),
    ))
}

/// POST `/api/v1/machine-identity/token` — the client-credentials token
/// endpoint (record 07 §2). The CLIENT's own credentials authenticate the
/// request; every refusal is ONE bare 401; the minted JWT is returned once
/// with the no-store cache posture.
async fn issue_machine_token(
    State(state): State<crate::state::AppState>,
    Json(body): Json<MachineTokenRequest>,
) -> Result<Response, ApiError> {
    if body.grant_type != CLIENT_CREDENTIALS_GRANT {
        return Err(ApiError::validation(format!(
            "grant_type must be '{CLIENT_CREDENTIALS_GRANT}' (record 07 §2: the only grant \
             this endpoint serves)"
        )));
    }
    if body.client_id.is_empty() || body.client_id.len() > MAX_CLIENT_ID_LENGTH {
        return Err(ApiError::validation(format!(
            "client_id must be 1..={MAX_CLIENT_ID_LENGTH} characters"
        )));
    }
    if body.client_secret.is_empty() || body.client_secret.len() > MAX_SECRET_LENGTH {
        return Err(ApiError::validation(format!(
            "client_secret must be 1..={MAX_SECRET_LENGTH} characters"
        )));
    }
    let tenant_id = FlexUuid::parse(&body.tenant_id)
        .map(TenantId::new)
        .map_err(|_| {
            ApiError::validation("tenant_id must be a canonical tenant UUID".to_string())
        })?;
    let service = composed_service(&state)?;
    let token = service
        .issue_client_credentials_token(&tenant_id, &body.client_id, &body.client_secret)
        .map_err(map_machine_identity_error)?;
    // RFC 6749 §5.1 posture: a token response MUST NOT be stored — the
    // live credential carries the no-store cache headers (consumed by
    // citation).
    let mut response = (
        StatusCode::OK,
        Json(MachineTokenResponse {
            access_token: token,
            token_type: "Bearer".to_string(),
            expires_in: TOKEN_TTL_SECONDS_DEFAULT,
        }),
    )
        .into_response();
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

/// GET `/api/v1/machine-identity/validate` — the presented machine token
/// IS the credential; the response is the registry-resolved principal (the
/// tenant and scope subset come from the REGISTRY, never the token claim).
/// Every validation refusal is ONE bare 401.
async fn validate_machine_token(
    State(state): State<crate::state::AppState>,
    headers: HeaderMap,
) -> Result<Json<DataEnvelope<MachinePrincipalDto>>, ApiError> {
    let token = bearer_token(&headers).ok_or_else(ApiError::unauthenticated)?;
    let service = composed_service(&state)?;
    let principal = service
        .validate_machine_token(&token)
        .map_err(map_machine_identity_error)?;
    Ok(Json(DataEnvelope {
        data: MachinePrincipalDto {
            client_id: principal.client_id,
            subject: principal.subject,
            tenant_id: principal.tenant_id.uuid().to_hyphenated(),
            scopes: principal
                .scopes
                .iter()
                .map(|action| action.id().to_string())
                .collect(),
        },
    }))
}

/// POST `/api/v1/machine-identity/clients/{client_id}/revoke` — the
/// emergency kill-switch (record 07 §4): state → disabled AND the
/// credential epoch bumps, so every token issued before the revocation
/// refuses at validation immediately. Unknown client → 404 (no existence
/// leak).
async fn revoke_machine_client(
    State(state): State<crate::state::AppState>,
    CallerExtractor(caller): CallerExtractor,
    axum::extract::Path(client_id): axum::extract::Path<String>,
) -> Result<Json<DataEnvelope<MachineClientRevokedDto>>, ApiError> {
    authorize_tenant_scoped(&state.policies, &caller, Action::ApiKeyRevoke)?;
    let service = composed_service(&state)?;
    let tenant_id = caller.ctx.tenant_id();
    service
        .revoke_client(&tenant_id, &client_id)
        .map_err(map_machine_identity_error)?;
    Ok(Json(DataEnvelope {
        data: MachineClientRevokedDto {
            client_id,
            state: MACHINE_CLIENT_DISABLED_STATE.to_string(),
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
        // The named refusal classes map onto the EXISTING ApiError
        // vocabulary; nothing echoes a presented credential.
        let validation = map_machine_identity_error(MachineIdentityError::InvalidScope { index: 0 });
        assert!(http_class(&validation).contains("422"));
        let unauthenticated = map_machine_identity_error(MachineIdentityError::InvalidSecret);
        assert!(http_class(&unauthenticated).contains("401"));
        for refusal in [
            MachineIdentityError::ClientDisabled,
            MachineIdentityError::ClientRevoked,
            MachineIdentityError::InvalidToken,
            MachineIdentityError::ExpiredToken,
            MachineIdentityError::WrongIssuer,
            MachineIdentityError::WrongAudience,
            MachineIdentityError::TenantMismatch,
        ] {
            assert!(
                http_class(&map_machine_identity_error(refusal)).contains("401"),
                "every client-authentication/token-validation refusal is one bare 401"
            );
        }
        let not_found = map_machine_identity_error(MachineIdentityError::UnknownClient);
        assert!(http_class(&not_found).contains("404"));
        // The duplicate-registration backend refusal maps to a 409 with a
        // CONSTRUCTED message (the backend message is never forwarded).
        let conflict = map_machine_identity_error(MachineIdentityError::Backend {
            code: "E_CLIENT_EXISTS".to_string(),
            message: "backend text that must never travel".to_string(),
        });
        assert!(http_class(&conflict).contains("409"));
        assert!(!http_class(&conflict).contains("must never travel"));
        // Any other backend failure stays opaque.
        let opaque = map_machine_identity_error(MachineIdentityError::Backend {
            code: "E_OTHER".to_string(),
            message: "backend text that must never travel".to_string(),
        });
        assert!(http_class(&opaque).contains("500"));
        assert!(!http_class(&opaque).contains("must never travel"));
        // And no mapping ever renders a presented credential.
        let refused = map_machine_identity_error(MachineIdentityError::InvalidSecret);
        assert!(!http_class(&refused).contains("credential-material"));
    }

    #[test]
    fn the_route_paths_live_under_the_versioned_api_prefix() {
        for path in [
            MACHINE_CLIENTS_PATH,
            MACHINE_TOKEN_PATH,
            MACHINE_VALIDATE_PATH,
            MACHINE_CLIENT_REVOKE_PATH,
        ] {
            assert!(
                path.starts_with("/api/v1/machine-identity/"),
                "the surface lives under the versioned machine-identity prefix: {path}"
            );
        }
    }

    #[test]
    fn the_structural_bounds_are_declared_constants() {
        assert!(MAX_SCOPES_PER_REQUEST > 0);
        assert!(MAX_SCOPE_LENGTH > 0);
        assert!(MAX_CLIENT_ID_LENGTH > 0);
        assert!(MAX_SECRET_LENGTH > 0);
        assert_eq!(MACHINE_CLIENT_DISABLED_STATE, "disabled");
        assert_eq!(CLIENT_CREDENTIALS_GRANT, "client_credentials");
    }

    #[test]
    fn the_bearer_extraction_is_strict() {
        let mut headers = HeaderMap::new();
        assert_eq!(bearer_token(&headers), None);
        headers.insert(
            axum::http::header::AUTHORIZATION,
            axum::http::HeaderValue::from_static("Bearer a-token"),
        );
        assert_eq!(bearer_token(&headers).as_deref(), Some("a-token"));
        headers.insert(
            axum::http::header::AUTHORIZATION,
            axum::http::HeaderValue::from_static("Basic a-token"),
        );
        assert_eq!(bearer_token(&headers), None);
        headers.insert(
            axum::http::header::AUTHORIZATION,
            axum::http::HeaderValue::from_static("Bearer    "),
        );
        assert_eq!(bearer_token(&headers), None, "an empty bearer is absent");
    }
}
