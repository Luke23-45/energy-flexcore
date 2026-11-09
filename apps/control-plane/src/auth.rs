// P1.17 — Authentication and authorization at the route layer.
//
// Authentication status (recorded, ledger P1.17 step 3): Phase 1 ships NO
// production OIDC/OAuth2 integration. The only identity resolver is
// `DevAuthResolver`, compiled ONLY behind the `dev-auth` feature
// (test/dev-profile-gated; never a default feature; CI fails if it appears
// in default features). Without the feature, `DenyAllResolver` denies every
// request — no hard-coded tenant, no anonymous fallback.
//
// Authorization: every route evaluates the caller against the authz
// `PolicySet` with an EXPLICIT action, scope, and tenant context
// (deny-by-default; P0.16 rule 5, P0.17 §17.3). The decision (with its
// reason and evidence) is what would be audited; a denial maps to 403.
use axum::http::request::Parts;
use flexcore_authz::action::Action;
use flexcore_authz::decision::AuthorizationDecision;
use flexcore_authz::policy::{PolicySet, Role};
use flexcore_authz::scope::{Scope, TenantScope};
use flexcore_contracts::ids::FlexUuid;
use flexcore_domain::ids::TenantId;
use flexcore_domain::tenant_context::{AuthenticatedClaims, TenantContext, VerifiedClaims};

/// A caller resolved from verified authentication: tenant context + role.
#[derive(Debug, Clone)]
pub struct Caller {
    pub ctx: TenantContext,
    pub role: Role,
}

/// Errors while resolving identity from a request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthError {
    /// No `Authorization` header / no bearer token.
    MissingCredentials,
    /// A token was present but could not be resolved to a verified identity.
    InvalidCredentials,
}

/// Resolves a request's `Authorization` bearer value to a [`Caller`].
///
/// Production wiring (P1.17): the OIDC integration lands in a later phase;
/// the only shipped resolver is the feature-gated dev resolver below. The
/// trait exists so the composition root and tests stay honest about the
/// single call site that turns tokens into claims.
pub trait IdentityResolver: Send + Sync {
    fn resolve(&self, bearer: Option<&str>) -> Result<Caller, AuthError>;
}

/// Denies every request. Compiled when `dev-auth` is OFF (release builds):
/// without a real identity provider there is NO authentication, and an
/// unauthenticated caller is never assigned a context (P0.16 rule 5).
pub struct DenyAllResolver;

impl IdentityResolver for DenyAllResolver {
    fn resolve(&self, _bearer: Option<&str>) -> Result<Caller, AuthError> {
        Err(AuthError::MissingCredentials)
    }
}

/// Dev/test identity resolver, compiled ONLY with `--features dev-auth`.
///
/// Token format (documented, dev-only, never a production credential):
/// `Bearer dev tenant=<uuid> subject=<subject> role=<platform-admin|tenant-admin|operator|viewer>`
///
/// This resolver constructs `VerifiedClaims` — the reviewed attestation
/// point that the "verification" happened (here: the token format matched).
/// It exists so the full auth path is exercisable in dev and CI; it is not
/// compiled into release builds.
#[cfg(feature = "dev-auth")]
pub struct DevAuthResolver;

#[cfg(feature = "dev-auth")]
impl IdentityResolver for DevAuthResolver {
    fn resolve(&self, bearer: Option<&str>) -> Result<Caller, AuthError> {
        let raw = bearer.ok_or(AuthError::MissingCredentials)?;
        let rest = raw.strip_prefix("Bearer ").ok_or(AuthError::InvalidCredentials)?;
        let mut tenant: Option<FlexUuid> = None;
        let mut subject: Option<String> = None;
        let mut role: Option<Role> = None;
        for part in rest.split_whitespace() {
            if let Some(v) = part.strip_prefix("tenant=") {
                tenant = Some(FlexUuid::parse(v).map_err(|_| AuthError::InvalidCredentials)?);
            } else if let Some(v) = part.strip_prefix("subject=") {
                subject = Some(v.to_string());
            } else if let Some(v) = part.strip_prefix("role=") {
                role = Some(parse_role(v).ok_or(AuthError::InvalidCredentials)?);
            }
        }
        let tenant = tenant.ok_or(AuthError::InvalidCredentials)?;
        let subject = subject.ok_or(AuthError::InvalidCredentials)?;
        let role = role.ok_or(AuthError::InvalidCredentials)?;
        let claims = AuthenticatedClaims::from_verified(VerifiedClaims {
            tenant_id: TenantId::new(tenant),
            subject,
        });
        Ok(Caller {
            ctx: TenantContext::from_authenticated_claims(claims),
            role,
        })
    }
}

/// Parses the dev role token into the closed [`Role`] enum.
pub fn parse_role(id: &str) -> Option<Role> {
    match id {
        "platform-admin" => Some(Role::PlatformAdmin),
        "tenant-admin" => Some(Role::TenantAdmin),
        "operator" => Some(Role::Operator),
        "viewer" => Some(Role::Viewer),
        _ => None,
    }
}

/// Extracts a [`Caller`] from request parts via the configured resolver.
/// Used by every handler through [`crate::state::AppState`].
pub fn caller_from_parts(
    resolver: &dyn IdentityResolver,
    parts: &Parts,
) -> Result<Caller, crate::error::ApiError> {
    let bearer = parts
        .headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());
    resolver
        .resolve(bearer.as_deref())
        .map_err(|e| match e {
            AuthError::MissingCredentials => crate::error::ApiError::unauthenticated(),
            AuthError::InvalidCredentials => crate::error::ApiError::unauthenticated(),
        })
}

/// Evaluates the policy for an action with a tenant-level scope (the Phase 1
/// default: registry writes and tenant-scoped lists). Returns the caller on
/// allow; a reasoned 403 on deny.
pub fn authorize_tenant_scoped(
    policies: &PolicySet,
    caller: &Caller,
    action: Action,
) -> Result<(), crate::error::ApiError> {
    let scope = Scope::Tenant(TenantScope::new(caller.ctx.tenant_id()));
    evaluate(policies, caller, action, Some(scope))
}

/// Evaluates the policy for an action with an already-built scope.
pub fn evaluate(
    policies: &PolicySet,
    caller: &Caller,
    action: Action,
    scope: Option<Scope>,
) -> Result<(), crate::error::ApiError> {
    let decision: AuthorizationDecision =
        policies.evaluate(caller.role, action, scope.as_ref(), Some(&caller.ctx));
    if decision.is_allowed() {
        Ok(())
    } else {
        Err(crate::error::ApiError::forbidden(&format!(
            "action {} denied ({})",
            action.id(),
            decision
                .deny_reason()
                .map(|r| format!("{r:?}"))
                .unwrap_or_else(|| "unspecified".to_string())
        )))
    }
}

/// The Axum extractor that resolves the caller from the request via the
/// state's `IdentityResolver`. Denies with 401 when authentication is
/// absent or invalid (P0.16 rule 5: a context is never fabricated).
pub struct CallerExtractor(pub Caller);

impl axum::extract::FromRequestParts<crate::state::AppState> for CallerExtractor {
    type Rejection = crate::error::ApiError;

    async fn from_request_parts(
        parts: &mut axum::extract::request::Parts,
        state: &crate::state::AppState,
    ) -> Result<Self, Self::Rejection> {
        caller_from_parts(state.identity.as_ref(), parts).map(CallerExtractor)
    }
}

/// Builds the Phase 1 default policy set.
///
/// Deny-by-default with EXPLICIT grants (no role bypasses the tenant
/// match): PlatformAdmin gets the full registry surface; TenantAdmin gets
/// tenant-scoped registry writes and reads; Operator/Viewer get reads.
/// Every grant carries a stable rule reference used as allow evidence.
pub fn phase1_default_policies() -> flexcore_authz::policy::PolicySet {
    use flexcore_authz::action::Action as A;
    use flexcore_authz::decision::EvidenceRef;
    use flexcore_authz::policy::{PolicySet, Role as R};
    use flexcore_authz::scope::ScopeKind as K;

    let mut p = PolicySet::new();
    let mut grant = |role: R, action: A, kind: K, id: &str, p: &mut PolicySet| {
        p.grant(role, action, kind, EvidenceRef::new(id.to_string()));
    };
    for (action, kind) in [
        (A::TenantCreate, K::Tenant),
        (A::TenantRead, K::Tenant),
        (A::PortfolioCreate, K::Tenant),
        (A::SiteCreate, K::Tenant),
        (A::AssetCreate, K::Tenant),
        (A::PortfolioRead, K::Tenant),
        (A::SiteRead, K::Tenant),
        (A::AssetRead, K::Tenant),
                (A::SiteRead, K::Site),
        (A::AssetRead, K::Asset),
    ] {
        grant(
            R::PlatformAdmin,
            action,
            kind,
            "p1-default/platform-admin",
            &mut p,
        );
    }
    for (action, kind) in [
        (A::PortfolioCreate, K::Tenant),
        (A::SiteCreate, K::Tenant),
        (A::AssetCreate, K::Tenant),
        (A::PortfolioRead, K::Tenant),
        (A::SiteRead, K::Tenant),
        (A::AssetRead, K::Tenant),
                (A::SiteRead, K::Site),
        (A::AssetRead, K::Asset),
    ] {
        grant(R::TenantAdmin, action, kind, "p1-default/tenant-admin", &mut p);
    }
    for (action, kind) in [
        (A::PortfolioRead, K::Tenant),
        (A::SiteRead, K::Tenant),
        (A::AssetRead, K::Tenant),
                (A::SiteRead, K::Site),
        (A::AssetRead, K::Asset),
    ] {
        grant(R::Operator, action, kind, "p1-default/operator", &mut p);
        grant(R::Viewer, action, kind, "p1-default/viewer", &mut p);
    }
    p
}

/// Builds the Phase 2 default policy set (P2.12): the Phase 1 grants plus
/// the Phase 2 action surface, all still deny-by-default with EXPLICIT
/// grants and no role bypassing the tenant match.
///
/// Recorded grants (rule references `p2-default/<role>`):
///
/// - **Tenant deletion** (`tenant.delete`): PlatformAdmin only — deleting
///   the tenant registry root is a platform-level action.
/// - **Registry deletes** (`portfolio.delete`, `site.delete`,
///   `asset.delete`), **asset updates** (`asset.update`) and
///   **deactivation** (`asset.deactivate`): PlatformAdmin and TenantAdmin.
///   All evaluated at tenant scope (the API layer enforces the P2.02
///   RESTRICT rule before the delete is issued, so scope granularity stays
///   tenant-level in Phase 2 — recorded).
/// - **Telemetry reads** (`telemetry.read`): every role — telemetry is
///   tenant-scoped data every authenticated role may read.
/// - **Quarantine reads** (`quarantine.read`): PlatformAdmin and
///   TenantAdmin only (the P2.07 decision: quarantine is an admin
///   surface; a raw rejected payload is diagnostic evidence, not
///   operational data).
pub fn phase2_default_policies() -> flexcore_authz::policy::PolicySet {
    use flexcore_authz::action::Action as A;
    use flexcore_authz::decision::EvidenceRef;
    use flexcore_authz::policy::{Role as R};
    use flexcore_authz::scope::ScopeKind as K;

    let mut p = phase1_default_policies();
    let mut grant = |role: R, action: A, kind: K, id: &str, p: &mut PolicySet| {
        p.grant(role, action, kind, EvidenceRef::new(id.to_string()));
    };

    for (action, kind) in [
        (A::TenantDelete, K::Tenant),
        (A::PortfolioDelete, K::Tenant),
        (A::SiteDelete, K::Tenant),
        (A::AssetDelete, K::Tenant),
        (A::AssetUpdate, K::Tenant),
        (A::AssetDeactivate, K::Tenant),
        (A::TelemetryRead, K::Tenant),
        (A::QuarantineRead, K::Tenant),
    ] {
        grant(
            R::PlatformAdmin,
            action,
            kind,
            "p2-default/platform-admin",
            &mut p,
        );
    }
    for (action, kind) in [
        (A::PortfolioDelete, K::Tenant),
        (A::SiteDelete, K::Tenant),
        (A::AssetDelete, K::Tenant),
        (A::AssetUpdate, K::Tenant),
        (A::AssetDeactivate, K::Tenant),
        (A::TelemetryRead, K::Tenant),
        (A::QuarantineRead, K::Tenant),
    ] {
        grant(R::TenantAdmin, action, kind, "p2-default/tenant-admin", &mut p);
    }
    for (action, kind) in [(A::TelemetryRead, K::Tenant)] {
        grant(R::Operator, action, kind, "p2-default/operator", &mut p);
        grant(R::Viewer, action, kind, "p2-default/viewer", &mut p);
    }
    p
}

/// Builds the Phase 3 default policy set (P3.28): the Phase 2 grants plus
/// the Phase 3 action surface (flexibility envelopes, schedules,
/// optimization runs), all still deny-by-default with EXPLICIT grants and
/// no role bypassing the tenant match. Evaluated at tenant scope (the
/// Phase 3 records are tenant-owned; the routes enforce the ownership
/// chain — portfolio/site existence and the revision chain — before any
/// write, so scope granularity stays tenant-level in Phase 3, recorded).
///
/// Recorded grants (rule references `p3-default/<role>`; the closure is
/// the `phase2_default_policies` set extended — the phase policy set is
/// cumulative by construction):
///
/// - **Envelope create** (`flexibility.envelope.create`): PlatformAdmin
///   and TenantAdmin — registering an envelope record (and emitting its
///   `FlexibilityEnvelopeUpdated` events) is a tenant-administration /
///   scheduling-pipeline surface, not an operator hand action in Phase 3.
/// - **Schedule propose** (`schedule.propose`): PlatformAdmin,
///   TenantAdmin, AND Operator — proposing a CANDIDATE schedule revision
///   is the operator scheduling action (simulation-only in 0.x; the event
///   is a proposal, never an approval, and NO approval/dispatch/execute
///   action exists on the surface).
/// - **Optimization-run create** (`optimization_run.create`):
///   PlatformAdmin and TenantAdmin — the record is produced by the
///   optimization pipeline (§12.6 persist-before-use), not typed by an
///   operator.
/// - **Reads** (`flexibility.envelope.read`, `schedule.read`,
///   `optimization_run.read`): every role — tenant-scoped records every
///   authenticated role may read (cross-tenant reads are 404 by the
///   route/store scoping regardless).
/// - Deliberately ABSENT: no envelope/schedule/run delete, update, or
///   approve action; no dispatch or execute action anywhere (§15.2; the
///   records are immutable, and validation/feasibility/authorization are
///   Phase 4 gates).
pub fn phase3_default_policies() -> flexcore_authz::policy::PolicySet {
    use flexcore_authz::action::Action as A;
    use flexcore_authz::decision::EvidenceRef;
    use flexcore_authz::policy::Role as R;
    use flexcore_authz::scope::ScopeKind as K;

    let mut p = phase2_default_policies();
    let mut grant = |role: R, action: A, kind: K, id: &str, p: &mut PolicySet| {
        p.grant(role, action, kind, EvidenceRef::new(id.to_string()));
    };

    let admin_surface = [
        (A::FlexibilityEnvelopeCreate, K::Tenant),
        (A::FlexibilityEnvelopeRead, K::Tenant),
        (A::SchedulePropose, K::Tenant),
        (A::ScheduleRead, K::Tenant),
        (A::OptimizationRunCreate, K::Tenant),
        (A::OptimizationRunRead, K::Tenant),
    ];
    for (action, kind) in admin_surface {
        grant(
            R::PlatformAdmin,
            action,
            kind,
            "p3-default/platform-admin",
            &mut p,
        );
        grant(R::TenantAdmin, action, kind, "p3-default/tenant-admin", &mut p);
    }
    // Operator: proposes candidate schedules and reads everything on the
    // Phase 3 surface; creates no envelope or optimization-run record.
    for (action, kind) in [
        (A::SchedulePropose, K::Tenant),
        (A::FlexibilityEnvelopeRead, K::Tenant),
        (A::ScheduleRead, K::Tenant),
        (A::OptimizationRunRead, K::Tenant),
    ] {
        grant(R::Operator, action, kind, "p3-default/operator", &mut p);
    }
    // Viewer: reads only.
    for (action, kind) in [
        (A::FlexibilityEnvelopeRead, K::Tenant),
        (A::ScheduleRead, K::Tenant),
        (A::OptimizationRunRead, K::Tenant),
    ] {
        grant(R::Viewer, action, kind, "p3-default/viewer", &mut p);
    }
    p
}
