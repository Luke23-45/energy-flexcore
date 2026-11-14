// P10.04 — Authorization and tenant-isolation test suite (control-plane
//         slice): the `DenyAllResolver` deny-by-default proof and the
//         cross-tenant denial matrix over the CUMULATIVE default policy
//         sets (report §22.3 gate 3; §16.2; §17.3–§17.4; §16.3-adjacent
//         identity rule "tenant context is derived from authenticated
//         claims, never from untrusted request-body fields"; ledger
//         P10.04 steps 1, 2, 4; record
//         `ledger/phase-10-v1-release-hardening/04_authz_tenant_tests.md`).
//
// WHAT THIS FILE PROVES:
//
// 1. THE DENY-ALL PROOF. `DenyAllResolver` (the ONLY identity resolver
//    compiled when the `dev-auth` feature is OFF — i.e. release builds)
//    refuses EVERY request with `MissingCredentials`, with or without a
//    bearer token: without a real identity provider there is NO
//    authentication, and an unauthenticated caller is never assigned a
//    context (P0.16 rule 5; no hard-coded tenant, no anonymous fallback).
//    A denial maps to 401 at the route layer (`caller_from_parts`), by
//    citation — the HTTP runtime itself is exercised by the existing
//    `api_*` suites, not duplicated here.
// 2. THE ROLE VOCABULARY IS CLOSED. `parse_role` accepts exactly the four
//    recorded roles; a forged role string is `None` — there is no way to
//    inject a role that does not exist on the surface.
// 3. THE CUMULATIVE DEFAULT-POLICY GRANT TABLE IS EXACT (in-tenant). Over
//    the full closed matrix — 4 roles × 29 actions × 3 scope kinds (348
//    combinations) — `phase5_default_policies()` (the cumulative superset
//    of the Phase 1→5 default sets, built by `auth.rs`) allows EXACTLY the
//    recorded grants and denies everything else with `NoMatchingRule`.
//    The expected table below is transcribed from the doc comments of
//    `apps/control-plane/src/auth.rs` (phase1–phase5 builders).
// 4. THE CROSS-TENANT DENIAL MATRIX over the same 348 combinations: scope
//    tenant B vs context tenant A → `ScopeDenied` for EVERY cell, zero
//    allows — including `telemetry.read`, `command.read`, and every other
//    read grant (§16.2: cross-tenant read, write, and command attempts are
//    denied; P0.16 rule 6: audited via the decision evidence).
// 5. ROLE BOUNDARIES CALLED OUT: `command.approve` is granted NOWHERE in
//    the REST default sets (the gate actions are evaluated through the
//    recorded `flexcore_authz::phase4_policies` grants at the GATES — the
//    REST write-path absence is the recorded P4.30 decision);
//    `quarantine.read` is PlatformAdmin/TenantAdmin only; `tenant.delete`
//    is PlatformAdmin only.
//
// STATUS: WRITTEN-UNRUN (ENTRY_NOTE boundary 3; no cargo command was
// executed; the human build/test gate owns:
// `cargo test -p flexcore-control-plane --test p10_04_deny_all_and_default_policies`).
// The expected allow table is a transcription of the recorded grant
// builders — verified cell-by-cell against `auth.rs` at authoring time.

use flexcore_authz::action::Action;
use flexcore_authz::decision::DenyReason;
use flexcore_authz::policy::{PolicySet, Role};
use flexcore_authz::scope::{AssetScope, Scope, ScopeKind, SiteScope, TenantScope};
use flexcore_control_plane::auth::{
    parse_role, phase1_default_policies, phase2_default_policies, phase3_default_policies,
    phase4_default_policies, phase5_default_policies, AuthError, DenyAllResolver, IdentityResolver,
};
use flexcore_contracts::ids::FlexUuid;
use flexcore_domain::ids::{AssetId, SiteId, TenantId};
use flexcore_domain::tenant_context::{
    AuthenticatedClaims, TenantContext, VerifiedClaims,
};

// --- Fixtures -----------------------------------------------------------------------

const UUID_TENANT_A: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f9a01";
const UUID_TENANT_B: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f9a02";
const UUID_SITE: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f9a03";
const UUID_ASSET: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f9a04";

fn uuid(text: &str) -> FlexUuid {
    FlexUuid::parse(text).expect("valid test uuid")
}

fn tenant_a() -> TenantId {
    TenantId::new(uuid(UUID_TENANT_A))
}

fn tenant_b() -> TenantId {
    TenantId::new(uuid(UUID_TENANT_B))
}

fn context_for(tenant: TenantId) -> TenantContext {
    // The ONLY public construction path (P0.16 rule 5): verified claims →
    // authenticated claims → context. Tenant context is NEVER taken from a
    // request-body field in these tests — the release-gate criterion (c).
    TenantContext::from_authenticated_claims(AuthenticatedClaims::from_verified(VerifiedClaims {
        tenant_id: tenant,
        subject: "p10-04:control-plane:principal".to_string(),
    }))
}

fn scope_of(kind: ScopeKind, tenant: TenantId) -> Scope {
    match kind {
        ScopeKind::Tenant => Scope::Tenant(TenantScope::new(tenant)),
        ScopeKind::Site => Scope::Site(SiteScope::new(tenant, SiteId::new(uuid(UUID_SITE)))),
        ScopeKind::Asset => Scope::Asset(AssetScope::new(tenant, AssetId::new(uuid(UUID_ASSET)))),
    }
}

const ALL_ACTIONS: [Action; 29] = [
    Action::TenantCreate,
    Action::TenantRead,
    Action::PortfolioCreate,
    Action::SiteCreate,
    Action::AssetCreate,
    Action::AssetRead,
    Action::PortfolioRead,
    Action::SiteRead,
    Action::TenantDelete,
    Action::PortfolioDelete,
    Action::SiteDelete,
    Action::AssetDelete,
    Action::AssetUpdate,
    Action::AssetDeactivate,
    Action::TelemetryRead,
    Action::QuarantineRead,
    Action::FlexibilityEnvelopeRead,
    Action::FlexibilityEnvelopeCreate,
    Action::ScheduleRead,
    Action::SchedulePropose,
    Action::OptimizationRunRead,
    Action::OptimizationRunCreate,
    Action::CommandCreate,
    Action::CommandValidate,
    Action::CommandApprove,
    Action::CommandRead,
    Action::FeasibilityRead,
    Action::AuditEventRead,
    Action::AlarmRead,
];

const ALL_ROLES: [Role; 4] = [
    Role::PlatformAdmin,
    Role::TenantAdmin,
    Role::Operator,
    Role::Viewer,
];

const ALL_SCOPE_KINDS: [ScopeKind; 3] = [ScopeKind::Tenant, ScopeKind::Site, ScopeKind::Asset];

/// The expected in-tenant allow table of the CUMULATIVE
/// `phase5_default_policies()` set, transcribed verbatim from the
/// `phase1_default_policies` … `phase5_default_policies` builders in
/// `apps/control-plane/src/auth.rs`.
fn default_grants(role: Role, action: Action, kind: ScopeKind) -> bool {
    use Action as A;
    use ScopeKind::{Asset, Site, Tenant};
    match (role, action, kind) {
        // --- Phase 1 (p1-default) ---
        (Role::PlatformAdmin, A::TenantCreate, Tenant) => true,
        (Role::PlatformAdmin, A::TenantRead, Tenant) => true,
        (Role::PlatformAdmin, A::PortfolioCreate, Tenant) => true,
        (Role::PlatformAdmin, A::SiteCreate, Tenant) => true,
        (Role::PlatformAdmin, A::AssetCreate, Tenant) => true,
        (Role::PlatformAdmin, A::PortfolioRead, Tenant) => true,
        (Role::PlatformAdmin, A::SiteRead, Tenant | Site) => true,
        (Role::PlatformAdmin, A::AssetRead, Tenant | Asset) => true,
        (Role::TenantAdmin, A::PortfolioCreate, Tenant) => true,
        (Role::TenantAdmin, A::SiteCreate, Tenant) => true,
        (Role::TenantAdmin, A::AssetCreate, Tenant) => true,
        (Role::TenantAdmin, A::PortfolioRead, Tenant) => true,
        (Role::TenantAdmin, A::SiteRead, Tenant | Site) => true,
        (Role::TenantAdmin, A::AssetRead, Tenant | Asset) => true,
        (Role::Operator, A::PortfolioRead, Tenant) => true,
        (Role::Operator, A::SiteRead, Tenant | Site) => true,
        (Role::Operator, A::AssetRead, Tenant | Asset) => true,
        (Role::Viewer, A::PortfolioRead, Tenant) => true,
        (Role::Viewer, A::SiteRead, Tenant | Site) => true,
        (Role::Viewer, A::AssetRead, Tenant | Asset) => true,
        // --- Phase 2 (p2-default) ---
        (Role::PlatformAdmin, A::TenantDelete, Tenant) => true,
        (Role::PlatformAdmin, A::PortfolioDelete, Tenant) => true,
        (Role::PlatformAdmin, A::SiteDelete, Tenant) => true,
        (Role::PlatformAdmin, A::AssetDelete, Tenant) => true,
        (Role::PlatformAdmin, A::AssetUpdate, Tenant) => true,
        (Role::PlatformAdmin, A::AssetDeactivate, Tenant) => true,
        (Role::PlatformAdmin, A::TelemetryRead, Tenant) => true,
        (Role::PlatformAdmin, A::QuarantineRead, Tenant) => true,
        (Role::TenantAdmin, A::PortfolioDelete, Tenant) => true,
        (Role::TenantAdmin, A::SiteDelete, Tenant) => true,
        (Role::TenantAdmin, A::AssetDelete, Tenant) => true,
        (Role::TenantAdmin, A::AssetUpdate, Tenant) => true,
        (Role::TenantAdmin, A::AssetDeactivate, Tenant) => true,
        (Role::TenantAdmin, A::TelemetryRead, Tenant) => true,
        (Role::TenantAdmin, A::QuarantineRead, Tenant) => true,
        (Role::Operator, A::TelemetryRead, Tenant) => true,
        (Role::Viewer, A::TelemetryRead, Tenant) => true,
        // --- Phase 3 (p3-default) ---
        (Role::PlatformAdmin, A::FlexibilityEnvelopeCreate, Tenant) => true,
        (Role::PlatformAdmin, A::FlexibilityEnvelopeRead, Tenant) => true,
        (Role::PlatformAdmin, A::SchedulePropose, Tenant) => true,
        (Role::PlatformAdmin, A::ScheduleRead, Tenant) => true,
        (Role::PlatformAdmin, A::OptimizationRunCreate, Tenant) => true,
        (Role::PlatformAdmin, A::OptimizationRunRead, Tenant) => true,
        (Role::TenantAdmin, A::FlexibilityEnvelopeCreate, Tenant) => true,
        (Role::TenantAdmin, A::FlexibilityEnvelopeRead, Tenant) => true,
        (Role::TenantAdmin, A::SchedulePropose, Tenant) => true,
        (Role::TenantAdmin, A::ScheduleRead, Tenant) => true,
        (Role::TenantAdmin, A::OptimizationRunCreate, Tenant) => true,
        (Role::TenantAdmin, A::OptimizationRunRead, Tenant) => true,
        (Role::Operator, A::SchedulePropose, Tenant) => true,
        (Role::Operator, A::FlexibilityEnvelopeRead, Tenant) => true,
        (Role::Operator, A::ScheduleRead, Tenant) => true,
        (Role::Operator, A::OptimizationRunRead, Tenant) => true,
        (Role::Viewer, A::FlexibilityEnvelopeRead, Tenant) => true,
        (Role::Viewer, A::ScheduleRead, Tenant) => true,
        (Role::Viewer, A::OptimizationRunRead, Tenant) => true,
        // --- Phase 4 (p4-default: the READ surface) ---
        (Role::PlatformAdmin, A::CommandRead, Tenant) => true,
        (Role::PlatformAdmin, A::FeasibilityRead, Tenant) => true,
        (Role::PlatformAdmin, A::AuditEventRead, Tenant) => true,
        (Role::TenantAdmin, A::CommandRead, Tenant) => true,
        (Role::TenantAdmin, A::FeasibilityRead, Tenant) => true,
        (Role::TenantAdmin, A::AuditEventRead, Tenant) => true,
        (Role::Operator, A::CommandRead, Tenant) => true,
        (Role::Operator, A::FeasibilityRead, Tenant) => true,
        (Role::Operator, A::AuditEventRead, Tenant) => true,
        (Role::Viewer, A::CommandRead, Tenant) => true,
        (Role::Viewer, A::FeasibilityRead, Tenant) => true,
        (Role::Viewer, A::AuditEventRead, Tenant) => true,
        // --- Phase 5 (p5-default) ---
        (Role::PlatformAdmin, A::AlarmRead, Tenant) => true,
        (Role::TenantAdmin, A::AlarmRead, Tenant) => true,
        (Role::Operator, A::AlarmRead, Tenant) => true,
        (Role::Viewer, A::AlarmRead, Tenant) => true,
        _ => false,
    }
}

// --- 1. The deny-all proof ------------------------------------------------------------

#[test]
fn deny_all_resolver_refuses_every_request_without_a_context() {
    // Release builds (dev-auth OFF): there is NO authentication. A missing
    // token AND a supplied token are both refused with the same named
    // error — no anonymous fallback, no hard-coded tenant (P0.16 rule 5).
    let resolver = DenyAllResolver;
    // (`Caller` is not `PartialEq` by design — an identity is not an
    // equation — so the refusals are asserted by pattern, not equality.)
    assert!(matches!(
        resolver.resolve(None),
        Err(AuthError::MissingCredentials)
    ));
    assert!(matches!(
        resolver.resolve(Some(
            "Bearer dev tenant=0192a5e0-0000-7e2f-9a1b-3c4d5e6f9a01 subject=x role=platform-admin"
        )),
        Err(AuthError::MissingCredentials)
    ));
    assert!(matches!(
        resolver.resolve(Some("Bearer whatever")),
        Err(AuthError::MissingCredentials)
    ));
    // The trait-object shape the composition root holds.
    fn takes_dyn(_: &dyn IdentityResolver) {}
    takes_dyn(&resolver);
}

#[test]
fn the_role_vocabulary_is_closed() {
    // Exactly the four recorded roles parse; a forged role string is None
    // — a role cannot be injected that does not exist on the surface.
    assert_eq!(parse_role("platform-admin"), Some(Role::PlatformAdmin));
    assert_eq!(parse_role("tenant-admin"), Some(Role::TenantAdmin));
    assert_eq!(parse_role("operator"), Some(Role::Operator));
    assert_eq!(parse_role("viewer"), Some(Role::Viewer));
    for forged in ["superadmin", "PLATFORM-ADMIN", "", "root", "tenant_admin"] {
        assert_eq!(parse_role(forged), None, "{forged:?} must not parse");
    }
}

// --- 2. The cumulative default-policy grant table is exact (348 cells) -----------------

#[test]
fn the_cumulative_default_set_allows_exactly_the_recorded_table() {
    let set = phase5_default_policies();
    let ctx = context_for(tenant_a());
    let mut allows = 0usize;
    for role in ALL_ROLES {
        for action in ALL_ACTIONS {
            for kind in ALL_SCOPE_KINDS {
                let decision =
                    set.evaluate(role, action, Some(&scope_of(kind, tenant_a())), Some(&ctx));
                let expected = default_grants(role, action, kind);
                assert_eq!(
                    decision.is_allowed(),
                    expected,
                    "cumulative default table mismatch: role={} action={} kind={}",
                    role.id(),
                    action.id(),
                    kind.id(),
                );
                if expected {
                    allows += 1;
                } else {
                    assert_eq!(
                        decision.deny_reason(),
                        Some(DenyReason::NoMatchingRule),
                        "role={} action={} kind={}",
                        role.id(),
                        action.id(),
                        kind.id(),
                    );
                }
            }
        }
    }
    // The recorded total, counted cell-by-cell from the transcription:
    // PlatformAdmin 27 (p1: 9, p2: 8, p3: 6, p4: 3, p5: 1),
    // TenantAdmin 25 (p1: 8, p2: 7, p3: 6, p4: 3, p5: 1),
    // Operator 14 (p1: 5, p2: 1, p3: 4, p4: 3, p5: 1),
    // Viewer 13 (p1: 5, p2: 1, p3: 3, p4: 3, p5: 1).
    assert_eq!(
        allows,
        27 + 25 + 14 + 13,
        "the cumulative table grants exactly 79 cells"
    );
}

#[test]
fn the_phase_sets_are_cumulative_strict_extensions() {
    // Each phase set extends the previous one (the recorded construction):
    // every grant of phase N is a grant of phase N+1 — the default sets
    // never silently revoke.
    let sets = [
        phase1_default_policies(),
        phase2_default_policies(),
        phase3_default_policies(),
        phase4_default_policies(),
        phase5_default_policies(),
    ];
    let ctx = context_for(tenant_a());
    for window in sets.windows(2) {
        let (earlier, later) = (&window[0], &window[1]);
        for role in ALL_ROLES {
            for action in ALL_ACTIONS {
                for kind in ALL_SCOPE_KINDS {
                    if earlier
                        .evaluate(role, action, Some(&scope_of(kind, tenant_a())), Some(&ctx))
                        .is_allowed()
                    {
                        assert!(
                            later
                                .evaluate(role, action, Some(&scope_of(kind, tenant_a())), Some(&ctx))
                                .is_allowed(),
                            "phase set regressed: role={} action={} kind={}",
                            role.id(),
                            action.id(),
                            kind.id(),
                        );
                    }
                }
            }
        }
    }
}

// --- 3. The cross-tenant denial matrix over the default sets ---------------------------

#[test]
fn every_cross_tenant_attempt_on_the_default_set_is_scope_denied() {
    let set = phase5_default_policies();
    let ctx = context_for(tenant_a());
    let mut denies = 0usize;
    for role in ALL_ROLES {
        for action in ALL_ACTIONS {
            for kind in ALL_SCOPE_KINDS {
                let decision =
                    set.evaluate(role, action, Some(&scope_of(kind, tenant_b())), Some(&ctx));
                assert!(
                    !decision.is_allowed(),
                    "cross-tenant allow: role={} action={} kind={}",
                    role.id(),
                    action.id(),
                    kind.id(),
                );
                assert_eq!(decision.deny_reason(), Some(DenyReason::ScopeDenied));
                assert_eq!(decision.evidence().as_str(), "authz:deny:scope-denied");
                denies += 1;
            }
        }
    }
    assert_eq!(denies, 348, "the matrix is exhaustive: 348 refusals, zero allows");
}

// --- 4. Role boundaries called out -----------------------------------------------------

#[test]
fn command_approve_is_granted_nowhere_in_the_rest_default_sets() {
    // The recorded P4.30 decision: the REST default sets carry the READ
    // surface only; `command.approve` / `command.create` /
    // `command.validate` are evaluated at the GATES through the recorded
    // `flexcore_authz::phase4_policies` grants — no REST grant exists for
    // any role (there is no REST write path to the command surface at
    // all, §15.2).
    let set = phase5_default_policies();
    let ctx = context_for(tenant_a());
    for role in ALL_ROLES {
        for action in [
            Action::CommandApprove,
            Action::CommandCreate,
            Action::CommandValidate,
        ] {
            for kind in ALL_SCOPE_KINDS {
                let decision =
                    set.evaluate(role, action, Some(&scope_of(kind, tenant_a())), Some(&ctx));
                assert!(
                    !decision.is_allowed(),
                    "no REST grant: role={} action={}",
                    role.id(),
                    action.id()
                );
                assert_eq!(decision.deny_reason(), Some(DenyReason::NoMatchingRule));
            }
        }
    }
    // Cross-tenant: the same refusals hold with the ScopeDenied reason.
    let decision = set.evaluate(
        Role::TenantAdmin,
        Action::CommandApprove,
        Some(&scope_of(ScopeKind::Asset, tenant_b())),
        Some(&ctx),
    );
    assert_eq!(decision.deny_reason(), Some(DenyReason::ScopeDenied));
}

#[test]
fn quarantine_read_is_admin_only_and_tenant_delete_is_platform_admin_only() {
    let set = phase5_default_policies();
    let ctx = context_for(tenant_a());
    // Quarantine (P2.07 decision): PlatformAdmin and TenantAdmin only —
    // a raw rejected payload is diagnostic evidence, not operational data.
    for role in ALL_ROLES {
        let decision = set.evaluate(
            role,
            Action::QuarantineRead,
            Some(&scope_of(ScopeKind::Tenant, tenant_a())),
            Some(&ctx),
        );
        assert_eq!(
            decision.is_allowed(),
            matches!(role, Role::PlatformAdmin | Role::TenantAdmin),
            "quarantine role={}",
            role.id()
        );
    }
    // Tenant deletion is a platform-level action: PlatformAdmin only.
    for role in ALL_ROLES {
        let decision = set.evaluate(
            role,
            Action::TenantDelete,
            Some(&scope_of(ScopeKind::Tenant, tenant_a())),
            Some(&ctx),
        );
        assert_eq!(
            decision.is_allowed(),
            role == Role::PlatformAdmin,
            "tenant.delete role={}",
            role.id()
        );
    }
}

#[test]
fn the_empty_set_remains_the_floor_for_every_default_set() {
    // Deny-by-default is structural: `PolicySet::new()` (the Default)
    // denies everything — a deployment that needs stricter behaviour
    // revokes by not using the builder. No default scope exists anywhere.
    let empty = PolicySet::new();
    let ctx = context_for(tenant_a());
    for role in ALL_ROLES {
        for action in ALL_ACTIONS {
            let decision = empty.evaluate(
                role,
                action,
                Some(&scope_of(ScopeKind::Tenant, tenant_a())),
                Some(&ctx),
            );
            assert!(!decision.is_allowed());
            assert_eq!(decision.deny_reason(), Some(DenyReason::NoMatchingRule));
        }
    }
}
