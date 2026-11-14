// P6.06 — The control-plane enrollment endpoint: `POST /api/v1/edge/enroll`
//         (report §18.1, §18.3, §17.3, §16.2; ledger P6.06; decision
//         record `06_07_identity_channel.md`). ADDITIVE — no existing
//         route is modified.
//
// PATH PROVENANCE (recorded, per the additive rule of P4.36/P5.31):
// `/api/v1/edge/enroll` is NOT one of the 16 frozen §15.2 candidate
// resource paths (`flexcore-contracts::api_paths::REST_RESOURCE_PATHS`).
// The frozen list governs dispatch-state mutation (no route outside it may
// mutate dispatch state — and this route touches none). Phase 4/5 already
// added non-frozen additive resources the same way (`/api/v1/audit-events`
// P4.36, `/api/v1/alarms` P5.31, `/api/v1/live` P5.25). This route follows
// the identical additive rule: versioned base path, single narrow POST,
// no execute/pass-through semantics, documented here and in the decision
// record.
//
// AUTH MODEL (the enrollment boundary): the credential is the SINGLE-USE
// provisioning token bound at deployment time to exactly one approved
// (tenant, site) scope — NOT a user bearer token and NOT a role-gated
// caller. The resolver is the [`ProvisioningTokenValidator`] seam; the
// shipped default is [`DenyAllEnrollmentTokens`] (no token validation
// service exists in 0.x — fail closed; a real validator, typically
// config-backed, is composed per deployment). Without a composed store
// AND validator the route fails closed (401 / infrastructure), never
// enrolls "to be helpful".
//
// FAIL-CLOSED MATRIX (the ledger P6.06 negative set):
// | Attempt | Result |
// |---|---|
// | missing/invalid provisioning token | 401 |
// | binding (tenant/site) ≠ token's approved scope | 403 |
// | site not approved in the tenant | 403 (+ REJECTED journal event) |
// | duplicate identity (existing ACTIVE term) | 409 (+ REJECTED journal event) |
// | re-enrollment of a REVOKED identity | 403 (+ REJECTED journal event) |
// | re-enrollment after EXPIRY | allowed as ROTATION (new term, seq+1, ROTATED event) |
// | no enrollment store composed | infrastructure error (honest, never a fake success) |
//
// The registry write + journal event persist per the migration 0018
// append-only design; NO key or certificate material is accepted, stored,
// or returned anywhere (§17.3). No PKI is implemented — the CA and
// OCSP/CRL are DEPLOYMENT REQUIREMENTS (decision record §3).

use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::extract::{Query, State};
use axum::http::header::AUTHORIZATION;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use flexcore_authz::action::Action;
use flexcore_contracts::ids::FlexUuid;
use flexcore_domain::ids::TenantId;
use flexcore_domain::tenant_context::{AuthenticatedClaims, TenantContext, VerifiedClaims};
use flexcore_storage::edge_enrollment::{
    EdgeDeviceIdentityRow, EdgeEnrollmentEventRow, EdgeEnrollmentStore, EdgeEnrollmentStoreError,
};

use crate::auth::{authorize_tenant_scoped, CallerExtractor};
use crate::dto::DataEnvelope;
use crate::dto_edge::{
    CommandPathStateDto, DataFreshnessDto, DecommissionRequestBody, DecommissionResponseDto,
    EdgeConnectivityReportDto, EdgeStatusDto, EdgeStatusParams, EnrollRequestBody,
    EnrollResponseDto,
};
use crate::error::ApiError;
use crate::state::AppState;

/// The honest record note carried verbatim in every 201 response.
const RECORD_NOTE: &str = "certificate issuance and OCSP/CRL revocation checking are \
                           DEPLOYMENT REQUIREMENTS: no CA is implemented in the FlexCore 0.x \
                           line, so no certificate is issued by this seam and no revocation \
                           status is asserted (decision record: \
                           ledger/phase-06-edge-protocol/06_07_identity_channel.md)";

// --- The token validator seam -------------------------------------------------

/// The approved (tenant, site) scope a validated provisioning token is
/// bound to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovedEnrollmentScope {
    pub tenant_id: FlexUuid,
    pub site_id: FlexUuid,
}

/// Errors while resolving the enrollment credential.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnrollmentAuthError {
    /// No `Authorization` header / no bearer token.
    MissingCredentials,
    /// A token was present but is not a valid provisioning token.
    InvalidCredentials,
}

/// Resolves the request's `Authorization` bearer value to the approved
/// enrollment scope. The seam the deployment composes its real (config-
/// or service-backed) single-use token service into; the shipped default
/// denies everything.
pub trait ProvisioningTokenValidator: Send + Sync {
    fn validate(&self, bearer: Option<&str>) -> Result<ApprovedEnrollmentScope, EnrollmentAuthError>;
}

/// The shipped validator: denies every request. Without a composed token
/// validation service there is NO enrollment — an unauthenticated edge is
/// never assigned a scope (the P1.17 `DenyAllResolver` rule applied at the
/// enrollment boundary).
pub struct DenyAllEnrollmentTokens;

impl ProvisioningTokenValidator for DenyAllEnrollmentTokens {
    fn validate(&self, _bearer: Option<&str>) -> Result<ApprovedEnrollmentScope, EnrollmentAuthError> {
        Err(EnrollmentAuthError::MissingCredentials)
    }
}

/// TEST/DEV SUPPORT — a static token→scope table (the `DevAuthResolver`
/// pattern: documented, dev-only, never a production credential; the
/// production validator is a deployment composition). Tokens are consumed
/// ONCE (single-use semantics demonstrated at the seam the deployment's
/// real service must implement).
#[derive(Default)]
pub struct StaticEnrollmentTokens {
    tokens: Mutex<Vec<(String, ApprovedEnrollmentScope)>>,
}

impl StaticEnrollmentTokens {
    pub fn new(tokens: Vec<(String, ApprovedEnrollmentScope)>) -> Self {
        Self {
            tokens: Mutex::new(tokens),
        }
    }
}

impl ProvisioningTokenValidator for StaticEnrollmentTokens {
    fn validate(&self, bearer: Option<&str>) -> Result<ApprovedEnrollmentScope, EnrollmentAuthError> {
        let raw = bearer.ok_or(EnrollmentAuthError::MissingCredentials)?;
        let token = raw
            .strip_prefix("Bearer ")
            .ok_or(EnrollmentAuthError::InvalidCredentials)?;
        let mut tokens = self.tokens.lock().expect("poisoned token table");
        let position = tokens
            .iter()
            .position(|(candidate, _)| candidate == token)
            .ok_or(EnrollmentAuthError::InvalidCredentials)?;
        let (_, scope) = tokens.remove(position); // single-use: consumed
        Ok(scope)
    }
}

// --- The Test-backend fake store ------------------------------------------------

/// TEST SUPPORT — an in-memory [`EdgeEnrollmentStore`] (the
/// `phase4_fakes` pattern, local to this module). Tenant-scoped by the
/// context; enforces the one-ACTIVE-term rule and the seq semantics the
/// real store's partial unique index enforces.
#[derive(Default)]
pub struct FakeEdgeEnrollmentStore {
    inner: Mutex<FakeInner>,
}

#[derive(Default)]
struct FakeInner {
    devices: Vec<EdgeDeviceIdentityRow>,
    events: Vec<EdgeEnrollmentEventRow>,
    approved_sites: Vec<(flexcore_domain::ids::TenantId, String)>,
}

impl FakeEdgeEnrollmentStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Seeds an approved site for a tenant (the real store derives approval
    /// from the `sites` table; tests seed the equivalent here).
    pub fn approve_site(&self, tenant: flexcore_domain::ids::TenantId, site_id: String) {
        self.inner
            .lock()
            .expect("poisoned")
            .approved_sites
            .push((tenant, site_id));
    }

    /// All recorded journal events (test assertions).
    pub fn events(&self) -> Vec<EdgeEnrollmentEventRow> {
        self.inner.lock().expect("poisoned").events.clone()
    }
}

impl EdgeEnrollmentStore for FakeEdgeEnrollmentStore {
    fn enroll_device(&self, ctx: &TenantContext, row: &EdgeDeviceIdentityRow) -> Result<(), EdgeEnrollmentStoreError> {
        let mut inner = self.inner.lock().expect("poisoned");
        if row.enrollment_seq == 0 {
            return Err(EdgeEnrollmentStoreError::InvalidRecord {
                field: "enrollment_seq",
            });
        }
        let duplicate_active = inner.devices.iter().any(|d| {
            d.tenant_id == ctx.tenant_id()
                && d.device_id == row.device_id
                && d.status == "ACTIVE"
        });
        if duplicate_active {
            return Err(EdgeEnrollmentStoreError::Conflict {
                device_id: row.device_id.clone(),
            });
        }
        let duplicate_seq = inner.devices.iter().any(|d| {
            d.tenant_id == ctx.tenant_id()
                && d.device_id == row.device_id
                && d.enrollment_seq == row.enrollment_seq
        });
        if duplicate_seq {
            return Err(EdgeEnrollmentStoreError::Conflict {
                device_id: row.device_id.clone(),
            });
        }
        inner.devices.push(row.clone());
        Ok(())
    }

    fn find_device(
        &self,
        ctx: &TenantContext,
        device_id: &str,
    ) -> Result<Option<EdgeDeviceIdentityRow>, EdgeEnrollmentStoreError> {
        let inner = self.inner.lock().expect("poisoned");
        Ok(inner
            .devices
            .iter()
            .filter(|d| d.tenant_id == ctx.tenant_id() && d.device_id == device_id)
            .max_by_key(|d| d.enrollment_seq)
            .cloned())
    }

    fn site_is_approved(
        &self,
        ctx: &TenantContext,
        site_id: &str,
    ) -> Result<bool, EdgeEnrollmentStoreError> {
        // Approval = explicitly seeded (the real store derives it from the
        // tenant's `sites` table) OR already bound by an existing registry
        // row. An unknown site is NOT approved — the honest default.
        let inner = self.inner.lock().expect("poisoned");
        Ok(inner.approved_sites.iter().any(|(tenant, site)| {
            *tenant == ctx.tenant_id() && site == site_id
        }) || inner.devices.iter().any(|d| {
            d.tenant_id == ctx.tenant_id() && d.site_id == site_id
        }))
    }

    fn append_event(
        &self,
        ctx: &TenantContext,
        event: &EdgeEnrollmentEventRow,
    ) -> Result<(), EdgeEnrollmentStoreError> {
        let mut inner = self.inner.lock().expect("poisoned");
        inner.events.push(EdgeEnrollmentEventRow {
            tenant_id: ctx.tenant_id(),
            ..event.clone()
        });
        Ok(())
    }

    fn list_events(
        &self,
        ctx: &TenantContext,
        device_id: &str,
        limit: u32,
    ) -> Result<Vec<EdgeEnrollmentEventRow>, EdgeEnrollmentStoreError> {
        let inner = self.inner.lock().expect("poisoned");
        Ok(inner
            .events
            .iter()
            .filter(|e| e.tenant_id == ctx.tenant_id() && e.device_id == device_id)
            .take(limit as usize)
            .cloned()
            .collect())
    }
}

// --- P6.21 — server-side edge-session scoping (defence in depth) ---------------
//
// The rule (§16.2, ledger P6.21): every edge-session request is scoped by
// the SERVER to the edge's enrolled identity — the tenant + site binding
// comes from the ENROLLMENT STORE (the authoritative copy), never from any
// request-carried field. This resolver is the single hardening point the
// edge-session surfaces compose (v0.4.0's only server-facing edge endpoint
// is the enrollment route above; telemetry/command session endpoints are
// later work and MUST resolve their scope through this helper, not through
// a request field). It is fail-closed: an identity that does not resolve to
// an ACTIVE term in the caller's tenant scope has NO scope — never a
// guessed, defaulted, or widened one. RLS stays the deeper control
// (migration 0018: ENABLE + FORCE row level security on the registry and
// journal; the §16.2/[R25] warning that the application role must not own
// tenant tables or bypass RLS is recorded in the decision document and
// tracked as a human-gate input).

/// The server-derived scope of an edge session: the binding as recorded in
/// the enrollment store — tenant + site + term — with no request-supplied
/// component anywhere in the shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EdgeSessionScope {
    /// The tenant the identity is enrolled under (from the store row).
    pub tenant_id: TenantId,
    /// The device identity that was resolved.
    pub device_id: String,
    /// The ONE approved site the identity is bound to (from the store row;
    /// §16.2: one edge, one tenant, one approved site).
    pub site_id: String,
    /// The ACTIVE identity term this scope was resolved from.
    pub enrollment_seq: u64,
}

/// Named refusals of the session-scope resolution. Fail-closed: a refusal
/// means the caller gets NO scope (never a fallback scope).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EdgeSessionScopeError {
    /// The device does not exist in the caller's tenant scope — including
    /// a device enrolled under ANOTHER tenant (existence never leaks across
    /// the tenant boundary, P0.16 rule 5).
    UnknownIdentity { device_id: String },
    /// The identity exists but its CURRENT (highest-sequence) term is not
    /// ACTIVE (REVOKED / EXPIRED): the session has no scope.
    IdentityNotActive { device_id: String, status: String },
}

impl std::fmt::Display for EdgeSessionScopeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EdgeSessionScopeError::UnknownIdentity { device_id } => write!(
                f,
                "device {device_id} does not resolve to an identity in this tenant scope"
            ),
            EdgeSessionScopeError::IdentityNotActive { device_id, status } => write!(
                f,
                "device {device_id} has no ACTIVE identity term (current status: {status}); \
                 the session has no scope"
            ),
        }
    }
}

impl std::error::Error for EdgeSessionScopeError {}

/// Resolves the edge-session scope for `device_id` FROM THE STORE, under
/// the caller's (server-side, authenticated) tenant context. There is
/// deliberately NO parameter by which a request could supply, override, or
/// widen any part of the scope — the tenant comes from the context and the
/// site comes from the store row.
pub fn resolve_edge_session_scope(
    store: &dyn EdgeEnrollmentStore,
    ctx: &TenantContext,
    device_id: &str,
) -> Result<EdgeSessionScope, EdgeSessionScopeError> {
    let unknown = || EdgeSessionScopeError::UnknownIdentity {
        device_id: device_id.to_string(),
    };
    // The tenant-scoped lookup (RLS + the explicit tenant filter): a
    // device enrolled under ANOTHER tenant reads as None here — the
    // cross-tenant identity is INVISIBLE, which is the isolation itself.
    let device = store.find_device(ctx, device_id).map_err(|_| unknown())?;
    let device = device.ok_or_else(unknown)?;
    if device.status != "ACTIVE" {
        return Err(EdgeSessionScopeError::IdentityNotActive {
            device_id: device.device_id,
            status: device.status,
        });
    }
    Ok(EdgeSessionScope {
        tenant_id: device.tenant_id,
        device_id: device.device_id,
        site_id: device.site_id,
        enrollment_seq: device.enrollment_seq,
    })
}

// --- The route -----------------------------------------------------------------

/// Builds the enrollment router (merged into the stage-1 router). POST
/// only on the enrollment surface — a narrow provisioning write, not a
/// registry API. P6.25/P6.32 (ADDITIVE): the per-edge status read
/// (`GET /api/v1/edge/status`) and the operator decommission
/// (`POST /api/v1/edge/decommission`) join the same additive rule —
/// versioned base paths, no dispatch-state mutation, no execute/pass-through
/// semantics anywhere.
pub fn edge_router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/edge/enroll", post(enroll_edge_device))
        .route("/api/v1/edge/status", get(get_edge_status))
        .route("/api/v1/edge/decommission", post(decommission_edge_device))
}

fn now_unix_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn map_store_error(error: EdgeEnrollmentStoreError) -> ApiError {
    match error {
        EdgeEnrollmentStoreError::InvalidRecord { field } => ApiError::validation(format!(
            "the enrollment record was rejected: '{field}' is missing or malformed"
        )),
        EdgeEnrollmentStoreError::Conflict { device_id } => ApiError::conflict(
            "EDGE_IDENTITY_EXISTS",
            format!(
                "device '{device_id}' is already enrolled and ACTIVE; no two edges share an \
                 identity (ledger P6.06)"
            ),
        ),
        EdgeEnrollmentStoreError::NotFound { .. } | EdgeEnrollmentStoreError::Backend { .. } => {
            ApiError::infrastructure()
        }
    }
}

/// Records the REJECTED journal event (the §17.3 audit of every refused
/// enrollment). Best-effort at the HTTP boundary: a journal failure is
/// logged loudly (tracing::error!) and never masked by a fake success —
/// the refusal response stands either way.
fn record_rejection(
    store: &dyn EdgeEnrollmentStore,
    ctx: &TenantContext,
    device_id: &str,
    reason: &str,
    correlation_id: &str,
) {
    let event = EdgeEnrollmentEventRow::new(
        ctx.tenant_id(),
        next_event_id(),
        device_id.to_string(),
        "REJECTED".to_string(),
        format!("edge:enrollment:{device_id}"),
        Some(reason.to_string()),
        correlation_id.to_string(),
        now_unix_millis(),
    );
    if let Err(error) = store.append_event(ctx, &event) {
        tracing::error!(
            %error,
            device_id,
            "EDGE ENROLLMENT REJECTION AUDIT FAILED — the refusal is NOT journaled"
        );
    }
}

/// Enrolls one edge device (the P6.06 control-plane seam). See the module
/// header for the path provenance, the auth model, and the fail-closed
/// matrix.
#[utoipa::path(
    post,
    path = "/api/v1/edge/enroll",
    request_body = EnrollRequestBody,
    responses(
        (status = 201, description = "Device identity issued (RECORD only — no certificate is issued; certificate issuance and OCSP/CRL are deployment requirements)", body = DataEnvelope<EnrollResponseDto>),
        (status = 401, description = "Missing or invalid provisioning token (no validation service composed → always 401)"),
        (status = 403, description = "The binding is outside the token's approved scope, the site is not approved in the tenant, or the identity is REVOKED"),
        (status = 409, description = "Duplicate identity: an ACTIVE term already exists (no two edges share an identity)"),
        (status = 422, description = "Malformed device id / tenant / site, or an expiry term before the issuance instant", body = crate::error::ErrorEnvelope),
    )
)]
async fn enroll_edge_device(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<EnrollRequestBody>,
) -> Result<(StatusCode, Json<DataEnvelope<EnrollResponseDto>>), ApiError> {
    // 1. The enrollment credential: the single-use provisioning token.
    let bearer = headers
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    let scope = state
        .enrollment_tokens
        .validate(bearer.as_deref())
        .map_err(|error| match error {
            EnrollmentAuthError::MissingCredentials => ApiError::unauthenticated(),
            EnrollmentAuthError::InvalidCredentials => ApiError::unauthenticated(),
        })?;

    // 2. Validate the request shape (fail fast, 422).
    if body.device_id.trim().is_empty() {
        return Err(ApiError::validation(
            "'device_id' is mandatory and must be non-empty".to_string(),
        ));
    }
    let tenant_uuid = FlexUuid::parse(&body.tenant_id)
        .map_err(|_| ApiError::validation(format!("'tenant_id' is not a valid UUID: '{}'", body.tenant_id)))?;
    let site_uuid = FlexUuid::parse(&body.site_id)
        .map_err(|_| ApiError::validation(format!("'site_id' is not a valid UUID: '{}'", body.site_id)))?;

    // 3. The binding must MATCH the token's approved scope (§16.2: an edge
    //    cannot enroll for a tenant/site it is not approved for).
    if tenant_uuid != scope.tenant_id || site_uuid != scope.site_id {
        return Err(ApiError::forbidden(
            "the requested tenant/site binding does not match the provisioning token's \
             approved scope (§16.2)",
        ));
    }

    // 4. The persistence store must be composed (fail closed otherwise).
    let store = state
        .edge_enrollment
        .as_ref()
        .ok_or_else(ApiError::infrastructure)?
        .clone();

    let ctx = TenantContext::from_authenticated_claims(AuthenticatedClaims::from_verified(
        VerifiedClaims {
            tenant_id: TenantId::new(scope.tenant_id),
            subject: format!("edge:enrollment:{}", body.device_id.trim()),
        },
    ));
    let correlation_id = headers
        .get("x-flexcore-correlation-id")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("edge-enroll:{}", body.device_id.trim()));
    let enrolled_at = now_unix_millis();
    if let Some(expires_at) = body.expires_at_unix_millis {
        if expires_at <= enrolled_at {
            return Err(ApiError::validation(
                "'expires_at_unix_millis' must be after the issuance instant".to_string(),
            ));
        }
    }

    // 5. The site must be approved in the tenant (§16.2).
    let approved = store
        .site_is_approved(&ctx, &site_uuid.to_hyphenated().to_string())
        .map_err(map_store_error)?;
    if !approved {
        record_rejection(
            store.as_ref(),
            &ctx,
            body.device_id.trim(),
            "site not approved in the tenant scope",
            &correlation_id,
        );
        return Err(ApiError::forbidden(
            "the requested site is not an approved site of this tenant (§16.2)",
        ));
    }

    // 6. The identity state decides: fresh enroll, rotation, or refusal.
    let device_id = body.device_id.trim().to_string();
    let existing = store
        .find_device(&ctx, &device_id)
        .map_err(map_store_error)?;
    let (seq, event_type) = match existing.as_ref().map(|d| d.status.as_str()) {
        None => (1u64, "ENROLLED"),
        Some("REVOKED") => {
            record_rejection(
                store.as_ref(),
                &ctx,
                &device_id,
                "revoked identity: re-enrollment is refused",
                &correlation_id,
            );
            return Err(ApiError::forbidden(
                "this device identity is REVOKED; re-enrollment is refused (ledger P6.06)",
            ));
        }
        Some("ACTIVE") => {
            record_rejection(
                store.as_ref(),
                &ctx,
                &device_id,
                "duplicate identity: an ACTIVE term already exists",
                &correlation_id,
            );
            return Err(ApiError::conflict(
                "EDGE_IDENTITY_EXISTS",
                format!(
                    "device '{device_id}' is already enrolled and ACTIVE; no two edges share \
                     an identity (ledger P6.06)"
                ),
            ));
        }
        // EXPIRED (or any non-ACTIVE/REVOKED status): rotation.
        Some(_) => {
            let seq = existing
                .as_ref()
                .map(|d| d.enrollment_seq + 1)
                .unwrap_or(1);
            (seq, "ROTATED")
        }
    };

    let row = EdgeDeviceIdentityRow::new(
        ctx.tenant_id(),
        device_id.clone(),
        site_uuid.to_hyphenated().to_string(),
        seq,
        "ACTIVE".to_string(),
        body.approved_asset_scope.clone(),
        enrolled_at,
        body.expires_at_unix_millis,
    )
    .map_err(map_store_error)?;
    store.enroll_device(&ctx, &row).map_err(map_store_error)?;
    let event = EdgeEnrollmentEventRow::new(
        ctx.tenant_id(),
        next_event_id(),
        device_id.clone(),
        event_type.to_string(),
        format!("edge:enrollment:{device_id}"),
        None,
        correlation_id.clone(),
        enrolled_at,
    )
    .map_err(map_store_error)?;
    store.append_event(&ctx, &event).map_err(map_store_error)?;

    Ok((
        StatusCode::CREATED,
        Json(DataEnvelope {
            data: EnrollResponseDto {
                device_id,
                tenant_id: body.tenant_id,
                site_id: body.site_id,
                enrollment_seq: seq,
                status: "ACTIVE".to_string(),
                enrolled_at_unix_millis: enrolled_at,
                expires_at_unix_millis: body.expires_at_unix_millis,
                certificate_issued: false,
                record_note: RECORD_NOTE.to_string(),
            },
        }),
    ))
}

/// Derives a process-unique journal event id (uuid-shaped text): the wall
/// instant in the high bytes and a process counter in the low bytes —
/// uniqueness within the tenant comes from the counter + instant
/// composition (the real PK (event_id, tenant_id) refuses duplicates
/// regardless).
fn next_event_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(1);
    let mut bytes = [0u8; 16];
    bytes[..8].copy_from_slice(&now_unix_millis().to_be_bytes());
    bytes[8..16].copy_from_slice(&COUNTER.fetch_add(1, Ordering::SeqCst).to_be_bytes());
    FlexUuid::from_bytes(u128::from_be_bytes(bytes))
        .to_hyphenated()
        .to_string()
}

// --- P6.25 — the per-edge status read (`GET /api/v1/edge/status`) ---------
//
// The console surfacing seam (report §20.1/§20.2, ledger P6.25; decision
// record `25_31_32_console_clock_decommission.md` §1): per-edge status —
// connectivity (last-seen), certificate expiry, adapter health (the P6.17
// HealthSnapshot JSON rendered VERBATIM), buffer depth, data freshness,
// and the P6.14 command-path state (disabled / test-env enabled WITH
// expiry).
//
// AUTH (recorded drift, the `authorize_live_stream` P5.25 pattern): the
// ledger's action id for this surface would be a new closed-enum variant
// (`edge.status.read`), but the `Action` vocabulary is a CLOSED enum owned
// by `crates/flexcore-authz` (outside this wave's file ownership). The
// route therefore evaluates `telemetry.read` — already tenant-scoped and
// granted to ALL roles by the phase policy sets — as the interim
// deny-by-default gate; the single swap point is this call site. Tenant
// scoping is server-side: the tenant comes from the caller's verified
// claims, the site from the registry row (the P6.21
// no-request-supplied-scope rule) — a cross-tenant device_id is 404 by the
// store scoping, never a leaked existence.
//
// DATA SOURCE (the honest seam): the control plane holds NO edge state of
// its own (the edge is outbound-only, §3.3) — the operational facts
// (last-seen, certificate expiry, adapter-health snapshot, buffer depth,
// freshness, command-path state) arrive through the
// [`EdgeStatusSource`] seam, which the deployment's edge-status forwarder
// composes. The shipped default is [`NoEdgeStatusReported`] — EVERY
// operational field renders as an explicit gap (`null` + the note), never
// a plausible "connected"/"fresh" value (the P6.25 risk row). Registry
// facts (identity status, term sequence, expiry) come from the enrollment
// store (migration 0018). This surface is POLLING (a plain GET; no live
// SSE requirement — the recorded decision).

/// The closed command-path state values the DTO renders (the P6.14 gate
/// states the console displays: disabled, or test-env enabled WITH expiry).
pub const COMMAND_PATH_STATE_DISABLED: &str = "DISABLED";
/// The test-env-enabled state (with a time-bounded enablement).
pub const COMMAND_PATH_STATE_TEST_ENV_ENABLED: &str = "TEST_ENV_ENABLED";
/// No reporter composed / an out-of-vocabulary report — the gap, named
/// (never silently rendered as DISABLED, which would look safe).
pub const COMMAND_PATH_STATE_NOT_REPORTED: &str = "NOT_REPORTED";

/// The honest surface note (verbatim in every response).
pub const EDGE_STATUS_NOTE: &str = "polling surface (plain GET; no live SSE on this resource) \
                                    — operational fields are supplied by the composed \
                                    EdgeStatusSource; where none is composed they render as \
                                    explicit gaps, never plausible healthy values. The \
                                    console can never enable the command path; enablement \
                                    takes effect ONLY through the P6.14 procedure.";

/// The operational status report of ONE edge, as the edge-status forwarder
/// supplies it (plain data; every field optional — absence is an honest
/// gap, never a default).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EdgeStatusReport {
    /// When the edge was last seen by the channel/health surface.
    pub last_seen_unix_millis: Option<i64>,
    /// The device certificate's expiry (unix millis), when known.
    pub certificate_expiry_unix_millis: Option<i64>,
    /// The P6.17 `HealthSnapshot.json()` rendering, VERBATIM (the console
    /// renders it as-is, closed vocabulary, never prettified).
    pub adapter_health_snapshot_json: Option<String>,
    /// Buffered-telemetry depth (records).
    pub buffer_depth: Option<u64>,
    /// Buffered-telemetry capacity (records).
    pub buffer_capacity: Option<u64>,
    /// The event_time of the last telemetry the edge produced.
    pub last_event_unix_millis: Option<i64>,
    /// The reported P6.14 command-path state (`DISABLED` /
    /// `TEST_ENV_ENABLED`).
    pub command_path_state: Option<String>,
    /// The enablement's expiry (unix millis), when TEST_ENV_ENABLED.
    pub command_path_enablement_valid_until_unix_millis: Option<i64>,
}

/// Errors of the status-source seam (named; mapped to infrastructure).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EdgeStatusSourceError {
    /// The composed reporter failed (code + message opaque/human-safe).
    Backend { reason: String },
}

impl std::fmt::Display for EdgeStatusSourceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EdgeStatusSourceError::Backend { reason } => {
                write!(f, "the edge-status reporter failed: {reason}")
            }
        }
    }
}

impl std::error::Error for EdgeStatusSourceError {}

/// The edge-status source seam (P6.25): the deployment composes its
/// edge-status forwarder (whatever transport carries the edge's health/
/// status reports inbound); the shipped default reports NOTHING — every
/// field renders as an explicit gap (fail-closed honesty, never a fake
/// "all healthy").
pub trait EdgeStatusSource: Send + Sync {
    /// The status report for `device_id` under the caller's (server-side)
    /// tenant context; `Ok(None)` = no report exists (the gap is named,
    /// never guessed).
    fn status_for(
        &self,
        ctx: &TenantContext,
        device_id: &str,
    ) -> Result<Option<EdgeStatusReport>, EdgeStatusSourceError>;
}

/// The shipped default: no edge-status reporter is composed, so NO
/// operational status exists — `Ok(None)` for every device.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoEdgeStatusReported;

impl EdgeStatusSource for NoEdgeStatusReported {
    fn status_for(
        &self,
        _ctx: &TenantContext,
        _device_id: &str,
    ) -> Result<Option<EdgeStatusReport>, EdgeStatusSourceError> {
        Ok(None)
    }
}

/// The closed command-path state rendering: a reported value inside the
/// closed set passes verbatim; anything else renders `NOT_REPORTED` (the
/// gap is named — an out-of-vocabulary value is never mapped onto a
/// plausible state). The composed note carries the raw value verbatim for
/// diagnostics.
fn command_path_state_dto(
    reported: Option<&str>,
    enablement_valid_until_unix_millis: Option<i64>,
) -> CommandPathStateDto {
    let (state, note) = match reported {
        Some(value @ (COMMAND_PATH_STATE_DISABLED | COMMAND_PATH_STATE_TEST_ENV_ENABLED)) => (
            value.to_string(),
            "the P6.14 gate state as reported; the console can NEVER enable the command \
             path — enablement takes effect ONLY through the signed per-site configuration \
             change procedure (P6.14). A proposed setpoint is never an executed command."
                .to_string(),
        ),
        other => (
            COMMAND_PATH_STATE_NOT_REPORTED.to_string(),
            match other {
                Some(raw) => format!(
                    "the edge-status reporter supplied an out-of-vocabulary command-path \
                     value ('{raw}'); the gap is named, never mapped onto a plausible state. \
                     The console can NEVER enable the command path (P6.14)."
                ),
                None => "no edge-status reporter is composed, so the command-path state is \
                         NOT REPORTED — this is an explicit gap, never a silent DISABLED. \
                         The console can NEVER enable the command path (P6.14)."
                    .to_string(),
            },
        ),
    };
    CommandPathStateDto {
        state,
        enablement_valid_until_unix_millis,
        note,
    }
}

/// Composes the [`EdgeStatusDto`] from the registry row and the (optional)
/// operational report. Registry facts are authoritative; every absent
/// operational fact renders as an explicit gap.
fn compose_edge_status(
    row: &EdgeDeviceIdentityRow,
    report: Option<&EdgeStatusReport>,
) -> EdgeStatusDto {
    let default_report = EdgeStatusReport::default();
    let report = report.unwrap_or(&default_report);
    let source_note = if report.last_seen_unix_millis.is_some() {
        "last-seen supplied by the composed edge-status reporter".to_string()
    } else {
        "no edge-status reporter composed (or the edge was never seen): last-seen is NOT \
         known — this is an explicit gap, never a plausible 'connected'"
            .to_string()
    };
    let freshness_note = match report.last_event_unix_millis {
        Some(_) => "last event time supplied by the composed edge-status reporter; \
                    staleness is judged against the edge's own freshness limit (P6.17)"
            .to_string(),
        None => "no data-freshness report exists (no reporter composed) — freshness is NOT \
                 known, never assumed fresh"
            .to_string(),
    };
    EdgeStatusDto {
        device_id: row.device_id.clone(),
        site_id: row.site_id.clone(),
        enrollment_seq: row.enrollment_seq,
        enrollment_status: row.status.clone(),
        identity_expires_at_unix_millis: row.expires_at_unix_millis,
        certificate_expiry_unix_millis: report.certificate_expiry_unix_millis,
        connectivity: EdgeConnectivityReportDto {
            last_seen_unix_millis: report.last_seen_unix_millis,
            source_note,
        },
        adapter_health_snapshot_json: report.adapter_health_snapshot_json.clone(),
        buffer_depth: report.buffer_depth,
        buffer_capacity: report.buffer_capacity,
        data_freshness: DataFreshnessDto {
            last_event_unix_millis: report.last_event_unix_millis,
            note: freshness_note,
        },
        command_path: command_path_state_dto(
            report.command_path_state.as_deref(),
            report.command_path_enablement_valid_until_unix_millis,
        ),
        status_note: EDGE_STATUS_NOTE.to_string(),
    }
}

/// The per-edge status read (P6.25). Auth: every authenticated role,
/// tenant-scoped (the recorded-drift gate above); the device must resolve
/// in the caller's tenant (cross-tenant = 404, existence never leaks).
#[utoipa::path(
    get,
    path = "/api/v1/edge/status",
    params(EdgeStatusParams),
    responses(
        (status = 200, description = "The per-edge status record (registry facts + the composed operational report; absent operational facts render as explicit gaps)", body = DataEnvelope<EdgeStatusDto>),
        (status = 401, description = "Unauthenticated caller"),
        (status = 403, description = "The role is not granted the interim status-read action"),
        (status = 404, description = "The device does not resolve in the caller's tenant scope (cross-tenant identities are invisible, §16.2)"),
        (status = 503, description = "No enrollment store composed, or the status reporter failed (infrastructure — honest, never a fake success)"),
    )
)]
async fn get_edge_status(
    State(state): State<AppState>,
    caller: CallerExtractor,
    Query(params): Query<EdgeStatusParams>,
) -> Result<Json<DataEnvelope<EdgeStatusDto>>, ApiError> {
    // AUTH: the recorded-drift interim gate (deny-by-default; the swap
    // point for the closed-enum `edge.status.read` action).
    authorize_tenant_scoped(&state.policies, &caller.0, Action::TelemetryRead)?;
    let store = state
        .edge_enrollment
        .as_ref()
        .ok_or_else(ApiError::infrastructure)?
        .clone();
    if params.device_id.trim().is_empty() {
        return Err(ApiError::validation(
            "'device_id' is mandatory and must be non-empty".to_string(),
        ));
    }
    let ctx = caller.0.ctx.clone();
    let row = store
        .find_device(&ctx, params.device_id.trim())
        .map_err(|_| ApiError::not_found())?
        .ok_or_else(ApiError::not_found)?;
    let report = state
        .edge_status
        .status_for(&ctx, params.device_id.trim())
        .map_err(|error| {
            tracing::error!(
                %error,
                device_id = %params.device_id,
                "the edge-status reporter FAILED — the status is not served as a fake success"
            );
            ApiError::infrastructure()
        })?;
    Ok(Json(DataEnvelope {
        data: compose_edge_status(&row, report.as_ref()),
    }))
}

// --- P6.32 — the operator decommission (`POST /api/v1/edge/decommission`) --
//
// The enrollment counterpart (report §18.1/§17.3/§16.2, ledger P6.32;
// decision record §3): operator-initiated, authorized, with a confirmation
// record naming the edge, tenant, site, actor, and time.
//
// AUTH + STEP-UP (the recorded drift pattern): a new closed-enum action
// (`edge.decommission`) is outside this wave's file ownership, so the RBAC
// gate evaluates `asset.deactivate` at tenant scope (granted to
// PlatformAdmin and TenantAdmin ONLY by the phase policy sets — the
// operator-initiated, admin-gated surface). The STEP-UP is the
// confirmation binding: the payload MUST name the edge identity, tenant,
// site, actor, and time, and the named actor must MATCH the authenticated
// caller's subject and the named tenant must MATCH the caller's verified
// tenant — a mismatch is a refusal (never a proxy confirmation). The
// CONFIRMATION RECORD is persisted via the append-only enrollment journal
// (event type REVOKED; reason = the full confirmation text).
//
// REVOCATION (the 0018 append-only term model): revocation appends a NEW
// registry row at the next `enrollment_seq` with the closed status
// `REVOKED` (the registry's lifecycle is expressed as inserts, migration
// 0018) — entirely through the EXISTING `EdgeEnrollmentStore` trait, so no
// storage-crate change is required (the decision record notes the
// DECOMMISSIONED alias is a console/journal-level word; the registry
// vocabulary stays the closed ACTIVE/REVOKED/EXPIRED).
//
// FAIL-CLOSED MATRIX (extends the P6.06 matrix):
// | Attempt | Result |
// |---|---|
// | unauthenticated / not PlatformAdmin or TenantAdmin | 401 / 403 |
// | actor ≠ authenticated subject, or tenant ≠ verified tenant | 403 (+ REJECTED journal event) |
// | unknown device (or cross-tenant) | 404 (+ REJECTED journal event where the tenant matches) |
// | site ≠ the registry row's binding | 403 (+ REJECTED journal event) |
// | already REVOKED (re-decommission) | 409 (+ REJECTED journal event) |
// | no enrollment store composed | infrastructure error (honest) |
//
// AFTER revocation, ANY later session/enrollment attempt from the identity
// is rejected + audited by the EXISTING fail-closed matrix: the session
// scope resolves only for an ACTIVE term
// (`resolve_edge_session_scope`), and re-enrollment of a REVOKED identity
// is refused (the P6.06 route matrix above).

/// The honest decommission record note (verbatim in every 200 response).
pub const DECOMMISSION_RECORD_NOTE: &str = "the identity term is REVOKED (append-only registry, \
    migration 0018) and the confirmation record is persisted in the enrollment journal; the \
    edge-side wipe (credentials, buffered telemetry, audit buffer per the upload-then-wipe / \
    gap-event rule) is a separate operator-driven step on the edge host (the edge agent has no \
    inbound listener); OCSP/CRL revocation propagation remains a DEPLOYMENT REQUIREMENT \
    (decision record: ledger/phase-06-edge-protocol/25_31_32_console_clock_decommission.md)";

/// Builds the confirmation-record reason text (the persisted journal
/// record: edge identity, tenant, site, actor, time — all five named).
fn decommission_confirmation_reason(
    body: &DecommissionRequestBody,
    revoked_at_unix_millis: i64,
) -> String {
    format!(
        "DECOMMISSION_CONFIRMED: device='{}' tenant='{}' site='{}' actor='{}' \
         confirmed_at={} revoked_at={}",
        body.device_id.trim(),
        body.tenant_id.trim(),
        body.site_id.trim(),
        body.actor.trim(),
        body.confirmed_at_unix_millis,
        revoked_at_unix_millis,
    )
}

/// The decommission endpoint (P6.32). See the section header for the auth
/// model, the step-up binding, and the fail-closed matrix.
#[utoipa::path(
    post,
    path = "/api/v1/edge/decommission",
    request_body = DecommissionRequestBody,
    responses(
        (status = 200, description = "The identity term is REVOKED and the confirmation record is persisted in the enrollment journal", body = DataEnvelope<DecommissionResponseDto>),
        (status = 401, description = "Unauthenticated caller"),
        (status = 403, description = "The role is not granted the interim decommission action, the named actor does not match the authenticated subject, the named tenant does not match the verified tenant, or the named site does not match the registry binding"),
        (status = 404, description = "The device does not resolve in the caller's tenant scope"),
        (status = 409, description = "The identity is already REVOKED (re-decommission refused)"),
        (status = 422, description = "The confirmation payload is incomplete (identity/tenant/site/actor/time all mandatory)"),
        (status = 503, description = "No enrollment store composed (infrastructure — honest, never a fake success)"),
    )
)]
async fn decommission_edge_device(
    State(state): State<AppState>,
    caller: CallerExtractor,
    Json(body): Json<DecommissionRequestBody>,
) -> Result<Json<DataEnvelope<DecommissionResponseDto>>, ApiError> {
    // 1. RBAC (the recorded-drift interim gate: admins only).
    authorize_tenant_scoped(&state.policies, &caller.0, Action::AssetDeactivate)?;
    let store = state
        .edge_enrollment
        .as_ref()
        .ok_or_else(ApiError::infrastructure)?
        .clone();
    let ctx = caller.0.ctx.clone();
    let correlation_id = format!(
        "edge-decommission:{}:{}",
        body.device_id.trim(),
        body.confirmed_at_unix_millis
    );

    // 2. Step-up: the confirmation payload must name EVERYTHING, and the
    //    named actor/tenant must MATCH the authenticated caller (a
    //    mismatch is a refusal + audited, never a proxy confirmation).
    let validation_failure = validate_decommission_confirmation(&body, &caller.0, &ctx);
    if let Err(error) = validation_failure {
        record_rejection(
            store.as_ref(),
            &ctx,
            body.device_id.trim(),
            &error,
            &correlation_id,
        );
        return Err(ApiError::forbidden(&error));
    }

    // 3. The device must resolve in the caller's tenant (cross-tenant = 404).
    let device_id = body.device_id.trim().to_string();
    let row = store
        .find_device(&ctx, &device_id)
        .map_err(map_store_error)?
        .ok_or_else(ApiError::not_found)?;

    // 4. The named site must MATCH the registry binding (§16.2 — one edge,
    //    one approved site; a wrong site is a refusal).
    if row.site_id != body.site_id.trim() {
        let reason = format!(
            "the named site does not match the registry binding (registry site '{}')",
            row.site_id
        );
        record_rejection(store.as_ref(), &ctx, &device_id, &reason, &correlation_id);
        return Err(ApiError::forbidden(&reason));
    }

    // 5. Already REVOKED: refuse the re-decommission (fail-closed; the
    //    response would otherwise imply a new revocation happened).
    if row.status == "REVOKED" {
        let reason = "the identity term is already REVOKED; re-decommission is refused";
        record_rejection(store.as_ref(), &ctx, &device_id, reason, &correlation_id);
        return Err(ApiError::conflict(
            "EDGE_IDENTITY_REVOKED",
            format!(
                "device '{device_id}' is already REVOKED; re-decommission is refused \
                 (ledger P6.32)"
            ),
        ));
    }

    // 6. REVOKE: append the new term at the next sequence with the closed
    //    status REVOKED (the 0018 append-only lifecycle), then persist the
    //    CONFIRMATION RECORD in the journal (REVOKED event; reason = the
    //    full confirmation text).
    let revoked_at = now_unix_millis();
    let new_seq = row.enrollment_seq + 1;
    let revoked_row = EdgeDeviceIdentityRow::new(
        ctx.tenant_id(),
        device_id.clone(),
        row.site_id.clone(),
        new_seq,
        "REVOKED".to_string(),
        row.approved_asset_scope.clone(),
        revoked_at,
        row.expires_at_unix_millis,
    )
    .map_err(map_store_error)?;
    store.enroll_device(&ctx, &revoked_row).map_err(map_store_error)?;
    let confirmation_text = decommission_confirmation_reason(&body, revoked_at);
    let event = EdgeEnrollmentEventRow::new(
        ctx.tenant_id(),
        next_event_id(),
        device_id.clone(),
        "REVOKED".to_string(),
        format!("operator:{}", body.actor.trim()),
        Some(confirmation_text),
        correlation_id.clone(),
        revoked_at,
    )
    .map_err(map_store_error)?;
    store.append_event(&ctx, &event).map_err(map_store_error)?;

    Ok(Json(DataEnvelope {
        data: DecommissionResponseDto {
            device_id,
            tenant_id: body.tenant_id.trim().to_string(),
            site_id: row.site_id,
            enrollment_seq: new_seq,
            status: "REVOKED".to_string(),
            actor: body.actor.trim().to_string(),
            confirmed_at_unix_millis: body.confirmed_at_unix_millis,
            revoked_at_unix_millis: revoked_at,
            record_note: DECOMMISSION_RECORD_NOTE.to_string(),
        },
    }))
}

/// Validates the step-up confirmation binding: every field non-empty, the
/// named tenant equals the caller's verified tenant, and the named actor
/// equals the caller's authenticated subject. Returns the refusal reason
/// on failure (the caller audited-records it).
fn validate_decommission_confirmation(
    body: &DecommissionRequestBody,
    caller: &crate::auth::Caller,
    ctx: &TenantContext,
) -> Result<(), String> {
    if body.device_id.trim().is_empty()
        || body.tenant_id.trim().is_empty()
        || body.site_id.trim().is_empty()
        || body.actor.trim().is_empty()
    {
        return Err(
            "the decommission confirmation is incomplete: device_id, tenant_id, site_id, \
             actor, and confirmed_at_unix_millis are ALL mandatory (a confirmation that \
             cannot name what it confirms is not a confirmation)"
                .to_string(),
        );
    }
    if body.confirmed_at_unix_millis <= 0 {
        return Err(
            "the decommission confirmation time is invalid (confirmed_at_unix_millis must \
             be a positive unix-millis instant)"
                .to_string(),
        );
    }
    let verified_tenant = ctx.tenant_id().to_string();
    if body.tenant_id.trim() != verified_tenant {
        return Err(format!(
            "the named tenant does not match the authenticated caller's verified tenant \
             ('{}') — a decommission is never issued on another tenant's behalf",
            verified_tenant
        ));
    }
    if body.actor.trim() != caller.ctx.subject() {
        return Err(format!(
            "the named actor ('{}') does not match the authenticated subject — the \
             step-up confirmation must be issued by the calling principal itself, never \
             on another actor's behalf",
            body.actor.trim()
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deny_all_tokens_is_the_shipped_fail_closed_default() {
        let validator = DenyAllEnrollmentTokens;
        assert!(matches!(
            validator.validate(Some("Bearer whatever")),
            Err(EnrollmentAuthError::MissingCredentials)
        ));
    }

    #[test]
    fn static_tokens_are_single_use_at_the_seam() {
        let scope = ApprovedEnrollmentScope {
            tenant_id: FlexUuid::from_bytes(1u128.to_be_bytes()),
            site_id: FlexUuid::from_bytes(2u128.to_be_bytes()),
        };
        let validator = StaticEnrollmentTokens::new(vec![("t-1".to_string(), scope)]);
        validator
            .validate(Some("Bearer t-1"))
            .expect("the first use validates");
        assert!(
            validator.validate(Some("Bearer t-1")).is_err(),
            "the SAME token must be refused on reuse (single-use)"
        );
    }

    // --- P6.21 — the session-scope resolver unit tests -------------------------

    fn tenant_ctx(seed: u128) -> TenantContext {
        TenantContext::from_authenticated_claims(AuthenticatedClaims::from_verified(
            VerifiedClaims {
                tenant_id: TenantId::new(FlexUuid::from_bytes(seed.to_be_bytes())),
                subject: "p6.21-unit-test".to_string(),
            },
        ))
    }

    fn seed_active(store: &FakeEdgeEnrollmentStore, tenant: u128, device: &str, site: &str) {
        store
            .enroll_device(
                &tenant_ctx(tenant),
                &EdgeDeviceIdentityRow::new(
                    TenantId::new(FlexUuid::from_bytes(tenant.to_be_bytes())),
                    device.to_string(),
                    site.to_string(),
                    1,
                    "ACTIVE".to_string(),
                    None,
                    1_700_000_000_000,
                    None,
                )
                .expect("the row builds"),
            )
            .expect("seeded");
    }

    #[test]
    fn the_session_scope_comes_from_the_store_never_from_a_request() {
        let store = FakeEdgeEnrollmentStore::new();
        seed_active(&store, 1, "edge-01", "site-a-uuid");
        // There is NO site/tenant parameter to influence: the scope is the
        // store row, verbatim.
        let scope = resolve_edge_session_scope(&store, &tenant_ctx(1), "edge-01")
            .expect("resolves");
        assert_eq!(scope.site_id, "site-a-uuid");
        assert_eq!(scope.enrollment_seq, 1);
    }

    #[test]
    fn a_cross_tenant_identity_is_invisible_to_the_scope_resolver() {
        let store = FakeEdgeEnrollmentStore::new();
        seed_active(&store, 2, "edge-b", "site-b-uuid");
        // Tenant A's context cannot resolve tenant B's device — the
        // malicious client's identity claim is irrelevant: the context is
        // server-side, and the store is tenant-scoped twice (RLS + filter).
        let error = resolve_edge_session_scope(&store, &tenant_ctx(1), "edge-b")
            .expect_err("a cross-tenant identity never resolves");
        assert!(matches!(error, EdgeSessionScopeError::UnknownIdentity { .. }));
    }

    #[test]
    fn a_non_active_identity_has_no_session_scope() {
        let store = FakeEdgeEnrollmentStore::new();
        store
            .enroll_device(
                &tenant_ctx(1),
                &EdgeDeviceIdentityRow::new(
                    TenantId::new(FlexUuid::from_bytes(1u128.to_be_bytes())),
                    "edge-revoked".to_string(),
                    "site-a-uuid".to_string(),
                    1,
                    "REVOKED".to_string(),
                    None,
                    1_700_000_000_000,
                    None,
                )
                .expect("the row builds"),
            )
            .expect("seeded");
        let error = resolve_edge_session_scope(&store, &tenant_ctx(1), "edge-revoked")
            .expect_err("a REVOKED identity has no scope");
        assert!(matches!(
            error,
            EdgeSessionScopeError::IdentityNotActive { status, .. } if status == "REVOKED"
        ));
    }

    // --- P6.25 — the status DTO composition (unit tests) --------------------

    fn status_row(seed_status: &str) -> EdgeDeviceIdentityRow {
        EdgeDeviceIdentityRow::new(
            TenantId::new(FlexUuid::from_bytes(1u128.to_be_bytes())),
            "edge-01".to_string(),
            "site-a-uuid".to_string(),
            2,
            seed_status.to_string(),
            None,
            1_700_000_000_000,
            Some(1_700_060_480_000),
        )
        .expect("the row builds")
    }

    #[test]
    fn the_command_path_state_renders_the_closed_vocabulary_and_names_gaps() {
        // DISABLED passes verbatim; TEST_ENV_ENABLED carries the expiry.
        let disabled = command_path_state_dto(Some(COMMAND_PATH_STATE_DISABLED), None);
        assert_eq!(disabled.state, "DISABLED");
        let enabled = command_path_state_dto(
            Some(COMMAND_PATH_STATE_TEST_ENV_ENABLED),
            Some(1_700_003_600_000),
        );
        assert_eq!(enabled.state, "TEST_ENV_ENABLED");
        assert_eq!(enabled.enablement_valid_until_unix_millis, Some(1_700_003_600_000));
        assert!(enabled.note.contains("NEVER enable"));
        // An out-of-vocabulary report renders NOT_REPORTED (the gap named,
        // the raw value carried in the note) — never a plausible state.
        let unknown = command_path_state_dto(Some("some-odd-value"), None);
        assert_eq!(unknown.state, "NOT_REPORTED");
        assert!(unknown.note.contains("some-odd-value"));
        // No report → NOT_REPORTED, explicitly never a silent DISABLED.
        let none = command_path_state_dto(None, None);
        assert_eq!(none.state, "NOT_REPORTED");
        assert!(none.note.contains("never a silent DISABLED"));
    }

    #[test]
    fn edge_status_renders_gaps_when_no_report_and_values_when_reported() {
        // No reporter composed: registry facts flow, operational fields are
        // explicit gaps (never plausible "connected"/"fresh"/"disabled").
        let gaps = compose_edge_status(&status_row("ACTIVE"), None);
        assert_eq!(gaps.device_id, "edge-01");
        assert_eq!(gaps.enrollment_status, "ACTIVE");
        assert_eq!(gaps.enrollment_seq, 2);
        assert_eq!(
            gaps.identity_expires_at_unix_millis,
            Some(1_700_060_480_000)
        );
        assert_eq!(gaps.connectivity.last_seen_unix_millis, None);
        assert!(gaps.connectivity.source_note.contains("explicit gap"));
        assert!(gaps.adapter_health_snapshot_json.is_none());
        assert_eq!(gaps.buffer_depth, None);
        assert!(gaps.data_freshness.note.contains("never assumed fresh"));
        assert_eq!(gaps.command_path.state, "NOT_REPORTED");
        // With a report: every supplied value passes through verbatim.
        let report = EdgeStatusReport {
            last_seen_unix_millis: Some(1_700_000_500_000),
            certificate_expiry_unix_millis: Some(1_700_086_400_000),
            adapter_health_snapshot_json: Some(
                "{\"adapters\":[{\"adapter_id\":\"simulated-test\",\"status\":\"HEALTHY\"}]}"
                    .to_string(),
            ),
            buffer_depth: Some(3),
            buffer_capacity: Some(16),
            last_event_unix_millis: Some(1_700_000_400_000),
            command_path_state: Some(COMMAND_PATH_STATE_TEST_ENV_ENABLED.to_string()),
            command_path_enablement_valid_until_unix_millis: Some(1_700_003_600_000),
        };
        let full = compose_edge_status(&status_row("ACTIVE"), Some(&report));
        assert_eq!(full.connectivity.last_seen_unix_millis, Some(1_700_000_500_000));
        assert_eq!(
            full.adapter_health_snapshot_json.as_deref(),
            Some("{\"adapters\":[{\"adapter_id\":\"simulated-test\",\"status\":\"HEALTHY\"}]}"),
            "the P6.17 snapshot passes through VERBATIM"
        );
        assert_eq!(full.buffer_depth, Some(3));
        assert_eq!(
            full.data_freshness.last_event_unix_millis,
            Some(1_700_000_400_000)
        );
        assert_eq!(full.command_path.state, "TEST_ENV_ENABLED");
    }

    // --- P6.32 — the decommission matrix (store level, real server logic) ----

    #[test]
    fn the_append_only_revocation_blocks_sessions_and_reenrollment() {
        // The decommission handler revokes by APPENDING a REVOKED row at
        // the next sequence (the 0018 append-only term model) — through
        // the existing EdgeEnrollmentStore trait, so this flow is the REAL
        // server logic, exercised on the fake store.
        let store = FakeEdgeEnrollmentStore::new();
        let ctx = tenant_ctx(1);
        store
            .enroll_device(
                &ctx,
                &EdgeDeviceIdentityRow::new(
                    ctx.tenant_id(),
                    "edge-01".to_string(),
                    "site-a-uuid".to_string(),
                    1,
                    "ACTIVE".to_string(),
                    None,
                    1_700_000_000_000,
                    None,
                )
                .expect("the row builds"),
            )
            .expect("enrolled");
        // Operator decommissions: the REVOKED row appends at seq 2.
        store
            .enroll_device(
                &ctx,
                &EdgeDeviceIdentityRow::new(
                    ctx.tenant_id(),
                    "edge-01".to_string(),
                    "site-a-uuid".to_string(),
                    2,
                    "REVOKED".to_string(),
                    None,
                    1_700_000_100_000,
                    None,
                )
                .expect("the row builds"),
            )
            .expect("revoked row appended");
        // ANY later session attempt from the identity has NO scope (the
        // real resolver, not a fake): current term is REVOKED.
        let error = resolve_edge_session_scope(&store, &ctx, "edge-01")
            .expect_err("a decommissioned identity has no session scope");
        assert!(matches!(
            error,
            EdgeSessionScopeError::IdentityNotActive { status, .. } if status == "REVOKED"
        ));
        // The re-enrollment matrix refuses: the CURRENT status (highest
        // sequence) is REVOKED — the enroll route's REVOKED branch keys on
        // exactly this lookup.
        let current = store
            .find_device(&ctx, "edge-01")
            .expect("the store reads")
            .expect("present");
        assert_eq!(current.enrollment_seq, 2);
        assert_eq!(current.status, "REVOKED");
    }

    #[test]
    fn the_decommission_step_up_binding_refuses_mismatches() {
        // The step-up confirmation must be issued by the calling principal
        // itself, for the caller's own tenant.
        let ctx = tenant_ctx(1);
        let caller = crate::auth::Caller {
            ctx: ctx.clone(),
            role: flexcore_authz::policy::Role::TenantAdmin,
        };
        let tenant_text = ctx.tenant_id().to_string();
        let ok = DecommissionRequestBody {
            device_id: "edge-01".to_string(),
            tenant_id: tenant_text.clone(),
            site_id: "site-a-uuid".to_string(),
            actor: caller.ctx.subject().to_string(),
            confirmed_at_unix_millis: 1_700_000_000_000,
            reason: None,
        };
        assert!(validate_decommission_confirmation(&ok, &caller, &ctx).is_ok());
        // A proxy confirmation (another actor named) is refused.
        let proxied = DecommissionRequestBody { actor: "someone-else".to_string(), ..ok.clone() };
        let error = validate_decommission_confirmation(&proxied, &caller, &ctx)
            .expect_err("the actor must match the authenticated subject");
        assert!(error.contains("does not match the authenticated subject"));
        // Another tenant's decommission is refused.
        let other_tenant = DecommissionRequestBody {
            tenant_id: "00000000-0000-0000-0000-000000000000".to_string(),
            ..ok.clone()
        };
        let error = validate_decommission_confirmation(&other_tenant, &caller, &ctx)
            .expect_err("a decommission is never issued on another tenant's behalf");
        assert!(error.contains("verified tenant"));
        // An incomplete confirmation is refused.
        let incomplete = DecommissionRequestBody { actor: "  ".to_string(), ..ok.clone() };
        let error = validate_decommission_confirmation(&incomplete, &caller, &ctx)
            .expect_err("a confirmation that cannot name what it confirms is refused");
        assert!(error.contains("incomplete"));
        // A non-positive confirmation time is refused.
        let backdated = DecommissionRequestBody {
            confirmed_at_unix_millis: 0,
            ..ok
        };
        let error = validate_decommission_confirmation(&backdated, &caller, &ctx)
            .expect_err("the confirmation time is mandatory");
        assert!(error.contains("confirmation time"));
    }
}
