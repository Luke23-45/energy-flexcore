// P1.23 — Security test suite (authz slice): deny-by-default evaluation of
//         the `PolicySet`, derived from the §17.4 threat table (ledger
//         P1.23; report §19.2 test layer 7, §17.4, §16.2; P0.17).
//
// Threats covered (ledger P0.17 §5 = report §17.4):
//   - Row 4 "Cross-tenant leakage" — a scope naming another tenant than the
//     context denies, with no role bypass (tested for PlatformAdmin
//     explicitly).
//   - Row 7 "Unauthorized operator action" — unknown actions, missing
//     scopes, missing contexts, and unmatched (role, action, scope-kind)
//     combinations all deny; the empty policy set denies everything.
//
// Assertions per case (ledger P1.23 acceptance: "security tests that only
// cover the happy path" are the recorded risk):
//   - every denial carries a `DenyReason` and a non-empty, stable
//     `EvidenceRef` (auditable per §17.3 control 12 / P0.16 rule 6);
//   - `is_allowed()` is `false` on every denial;
//   - allow paths exist only where an explicit rule was granted.
//
// Provenance note: the `TenantContext` values below are built through the
// domain's single public claims path
// (`VerifiedClaims` -> `AuthenticatedClaims::from_verified` ->
// `TenantContext::from_authenticated_claims`), the same path the API
// middleware uses (P0.16 rule 5). This test exercises the PUBLIC
// `PolicySet::evaluate` entry with real contexts — unlike the in-crate
// tests, which use the private `decide` core because they were written
// before the claims constructor existed. If a bypass (raw string, bare ID,
// struct literal, fabricated claims, default) were attempted here, it would
// be a compile error, not a runtime check — see the negative-test comment
// in `flexcore-domain/src/tenant_context.rs` and in this crate's
// `src/policy.rs`.

use flexcore_authz::action::Action;
use flexcore_authz::decision::{AllowReason, DenyReason};
use flexcore_authz::policy::{PolicySet, Role};
use flexcore_authz::scope::{AssetScope, Scope, ScopeKind, SiteScope, TenantScope};
use flexcore_contracts::ids::FlexUuid;
use flexcore_domain::ids::{AssetId, SiteId, TenantId};
use flexcore_domain::tenant_context::{
    AuthenticatedClaims, TenantContext, VerifiedClaims,
};

const UUID_TENANT_A: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a01";
const UUID_TENANT_B: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a02";
const UUID_SITE: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a03";
const UUID_ASSET: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a04";

fn uuid(text: &str) -> FlexUuid {
    FlexUuid::parse(text).expect("valid test uuid")
}

fn tenant_a() -> TenantId {
    TenantId::new(uuid(UUID_TENANT_A))
}

fn tenant_b() -> TenantId {
    TenantId::new(uuid(UUID_TENANT_B))
}

/// Builds the tenant context through the ONLY public construction path
/// (P0.16 rule 5). In a real deployment the middleware (P1.17) attests the
/// OIDC token was verified before building `VerifiedClaims`; here the test
/// fixes the claims exactly like the port fakes do.
fn context_for(tenant: TenantId) -> TenantContext {
    TenantContext::from_authenticated_claims(AuthenticatedClaims::from_verified(VerifiedClaims {
        tenant_id: tenant,
        subject: "security-test:principal".to_string(),
    }))
}

fn asset_scope(tenant: TenantId) -> Scope {
    Scope::Asset(AssetScope::new(tenant, AssetId::new(uuid(UUID_ASSET))))
}

fn site_scope(tenant: TenantId) -> Scope {
    Scope::Site(SiteScope::new(tenant, SiteId::new(uuid(UUID_SITE))))
}

fn tenant_scope(tenant: TenantId) -> Scope {
    Scope::Tenant(TenantScope::new(tenant))
}

/// A policy set with exactly one grant: TenantAdmin may read assets at
/// asset scope. Everything else in this crate is denied by default.
fn granted_set() -> PolicySet {
    let mut set = PolicySet::new();
    set.grant(
        Role::TenantAdmin,
        Action::AssetRead,
        ScopeKind::Asset,
        flexcore_authz::decision::EvidenceRef::new("policy:tenant-admin:asset.read:asset"),
    );
    set
}

// --- Unknown action id denies (§17.4 row 7) ---------------------------------

#[test]
fn unknown_action_id_denies_with_evidence() {
    let ctx = context_for(tenant_a());
    // `dispatch.authorize` is deliberately NOT on the Phase 1 surface
    // (Phase 4 protected kernel; §5.7). An untyped caller asking for it
    // denies before scope, context, or rules are considered.
    let decision = PolicySet::new().evaluate_action_id(
        Role::PlatformAdmin,
        "dispatch.authorize",
        Some(&asset_scope(tenant_a())),
        Some(&ctx),
    );
    assert!(!decision.is_allowed(), "unknown action must deny");
    assert_eq!(decision.deny_reason(), Some(DenyReason::UnknownAction));
    assert_eq!(decision.evidence().as_str(), "authz:deny:unknown-action");
}

#[test]
fn unknown_action_id_denies_even_with_a_granted_role_and_matching_rule() {
    // Deny-by-default for anything the surface does not know: even a role
    // with a real grant cannot smuggle an unlisted action id through.
    let ctx = context_for(tenant_a());
    let mut set = granted_set();
    set.grant(
        Role::TenantAdmin,
        Action::AssetRead,
        ScopeKind::Asset,
        flexcore_authz::decision::EvidenceRef::new("policy:x"),
    );
    for unknown in [
        "dispatch.authorize",
        "dispatch.issue",
        "secret.read",
        "network.any",
        "asset.delete",
        // Near-misses: wrong case, wrong separator, empty.
        "Asset.Read",
        "asset_read",
        "",
    ] {
        let decision = set.evaluate_action_id(Role::TenantAdmin, unknown, Some(&asset_scope(tenant_a())), Some(&ctx));
        assert_eq!(
            decision.deny_reason(),
            Some(DenyReason::UnknownAction),
            "action id {unknown:?} must deny"
        );
        assert!(!decision.is_allowed());
        assert!(!decision.evidence().as_str().is_empty());
    }
}

// --- Missing scope denies (no default scope exists) --------------------------

#[test]
fn missing_scope_denies_even_for_a_granted_role_and_action() {
    let ctx = context_for(tenant_a());
    // Known action, granted role, valid context — but no scope: denied.
    // There is no default scope anywhere in this crate.
    let decision = granted_set().evaluate(Role::TenantAdmin, Action::AssetRead, None, Some(&ctx));
    assert!(!decision.is_allowed(), "a scopeless request must deny");
    assert_eq!(decision.deny_reason(), Some(DenyReason::UnknownScope));
    assert_eq!(decision.evidence().as_str(), "authz:deny:unknown-scope");
}

#[test]
fn missing_scope_denies_for_every_role() {
    let ctx = context_for(tenant_a());
    for role in [Role::PlatformAdmin, Role::TenantAdmin, Role::Operator, Role::Viewer] {
        let decision = granted_set().evaluate(role, Action::AssetRead, None, Some(&ctx));
        assert_eq!(decision.deny_reason(), Some(DenyReason::UnknownScope), "{role:?}");
        assert!(!decision.is_allowed());
    }
}

// --- Missing context denies ---------------------------------------------------

#[test]
fn missing_context_denies_with_evidence() {
    // Explicit scope, granted role and action, but no tenant context:
    // denied — evaluation runs against the claims-derived domain context
    // (P0.16 rule 5); without one there is nothing to match tenancy against.
    let decision = granted_set().evaluate(Role::TenantAdmin, Action::AssetRead, Some(&asset_scope(tenant_a())), None);
    assert!(!decision.is_allowed(), "a contextless request must deny");
    assert_eq!(decision.deny_reason(), Some(DenyReason::MissingContext));
    assert_eq!(decision.evidence().as_str(), "authz:deny:missing-context");
}

#[test]
fn missing_context_denies_even_for_platform_admin() {
    // Breadth comes from rules, never from a bypass: PlatformAdmin without
    // a context denies exactly like everyone else.
    let mut set = PolicySet::new();
    set.grant(
        Role::PlatformAdmin,
        Action::TenantRead,
        ScopeKind::Tenant,
        flexcore_authz::decision::EvidenceRef::new("policy:platform-admin:tenant.read:tenant"),
    );
    let decision = set.evaluate(Role::PlatformAdmin, Action::TenantRead, Some(&tenant_scope(tenant_a())), None);
    assert_eq!(decision.deny_reason(), Some(DenyReason::MissingContext));
    assert!(!decision.is_allowed());
}

// --- Cross-tenant scope denies, no role bypass (§17.4 row 4) ------------------

#[test]
fn cross_tenant_scope_denies_even_for_platform_admin() {
    // The scope names tenant B while the (verified-claims) context names
    // tenant A: denied even though an identical in-tenant request would
    // allow. PlatformAdmin — the broadest role — gets NO tenant-match
    // bypass (§17.4 cross-tenant-leakage row; P0.16 rule 6).
    let ctx_admin_of_a = context_for(tenant_a());
    let mut set = PolicySet::new();
    set.grant(
        Role::PlatformAdmin,
        Action::SiteRead,
        ScopeKind::Site,
        flexcore_authz::decision::EvidenceRef::new("policy:platform-admin:site.read:site"),
    );
    // Sanity: the same grant allows the in-tenant request.
    let in_tenant = set.evaluate(
        Role::PlatformAdmin,
        Action::SiteRead,
        Some(&site_scope(tenant_a())),
        Some(&ctx_admin_of_a),
    );
    assert!(in_tenant.is_allowed(), "in-tenant request must allow");
    assert_eq!(in_tenant.allow_reason(), Some(AllowReason::PolicyMatch));

    let cross = set.evaluate(
        Role::PlatformAdmin,
        Action::SiteRead,
        Some(&site_scope(tenant_b())),
        Some(&ctx_admin_of_a),
    );
    assert!(!cross.is_allowed(), "cross-tenant scope must deny");
    assert_eq!(cross.deny_reason(), Some(DenyReason::ScopeDenied));
    assert_eq!(cross.evidence().as_str(), "authz:deny:scope-denied");
}

#[test]
fn cross_tenant_scope_denies_at_every_scope_level() {
    let ctx = context_for(tenant_a());
    for (name, scope) in [
        ("tenant", tenant_scope(tenant_b())),
        ("site", site_scope(tenant_b())),
        ("asset", asset_scope(tenant_b())),
    ] {
        let decision = granted_set().evaluate(Role::TenantAdmin, Action::AssetRead, Some(&scope), Some(&ctx));
        assert_eq!(decision.deny_reason(), Some(DenyReason::ScopeDenied), "{name} scope");
        assert!(!decision.is_allowed());
    }
}

#[test]
fn cross_tenant_denial_holds_for_every_role_with_a_grant() {
    // No role bypasses the tenant match: grant each role the same action,
    // then present a foreign-tenant scope — each denies.
    for role in [Role::PlatformAdmin, Role::TenantAdmin, Role::Operator, Role::Viewer] {
        let ctx = context_for(tenant_a());
        let mut set = PolicySet::new();
        set.grant(
            role,
            Action::AssetRead,
            ScopeKind::Asset,
            flexcore_authz::decision::EvidenceRef::new("policy:per-role"),
        );
        let decision = set.evaluate(
            role,
            Action::AssetRead,
            Some(&asset_scope(tenant_b())),
            Some(&ctx),
        );
        assert_eq!(decision.deny_reason(), Some(DenyReason::ScopeDenied), "{role:?}");
        assert!(!decision.is_allowed());
    }
}

// --- Unknown role-to-action pairs deny (grants are exact) ---------------------

#[test]
fn unknown_role_to_action_pairs_deny() {
    let ctx = context_for(tenant_a());
    // Only (TenantAdmin, AssetRead, Asset) is granted. Every other
    // (role, action, scope-kind) combination denies with NoMatchingRule —
    // roles are flat, no hierarchy, no implication (P1.11).
    let mut set = PolicySet::new();
    set.grant(
        Role::TenantAdmin,
        Action::AssetRead,
        ScopeKind::Asset,
        flexcore_authz::decision::EvidenceRef::new("policy:tenant-admin:asset.read:asset"),
    );

    let cases: Vec<(Role, Action, Scope, &'static str)> = vec![
        // Wrong role, right action and kind.
        (Role::Viewer, Action::AssetRead, asset_scope(tenant_a()), "viewer not granted"),
        (Role::Operator, Action::AssetRead, asset_scope(tenant_a()), "operator not granted"),
        (Role::PlatformAdmin, Action::AssetRead, asset_scope(tenant_a()), "platform-admin not granted"),
        // Right role, wrong action.
        (Role::TenantAdmin, Action::SiteRead, site_scope(tenant_a()), "site.read not granted to tenant-admin"),
        (Role::TenantAdmin, Action::AssetCreate, asset_scope(tenant_a()), "asset.create not granted"),
        (Role::TenantAdmin, Action::PortfolioRead, tenant_scope(tenant_a()), "portfolio.read not granted"),
        // Right role and action, wrong scope level — scope levels do not
        // inherit.
        (Role::TenantAdmin, Action::AssetRead, site_scope(tenant_a()), "asset.read at site scope"),
        (Role::TenantAdmin, Action::AssetRead, tenant_scope(tenant_a()), "asset.read at tenant scope"),
        // Right role, right kind, different action/kind pairing.
        (Role::TenantAdmin, Action::TenantCreate, tenant_scope(tenant_a()), "tenant.create not granted"),
    ];
    for (role, action, scope, why) in cases {
        let decision = set.evaluate(role, action, Some(&scope), Some(&ctx));
        assert_eq!(
            decision.deny_reason(),
            Some(DenyReason::NoMatchingRule),
            "{why} must deny with NoMatchingRule"
        );
        assert!(!decision.is_allowed(), "{why} must not allow");
        assert_eq!(decision.evidence().as_str(), "authz:deny:no-matching-rule");
    }
}

#[test]
fn empty_policy_set_denies_every_well_formed_request() {
    // The Default set is empty: deny everything, even fully in-tenant,
    // well-formed requests for known actions with valid contexts.
    let ctx = context_for(tenant_a());
    for action in [
        Action::TenantCreate,
        Action::TenantRead,
        Action::PortfolioCreate,
        Action::PortfolioRead,
        Action::SiteCreate,
        Action::SiteRead,
        Action::AssetCreate,
        Action::AssetRead,
    ] {
        let scope = match action {
            Action::TenantCreate | Action::TenantRead | Action::PortfolioCreate | Action::PortfolioRead => {
                tenant_scope(tenant_a())
            }
            Action::SiteCreate | Action::SiteRead => site_scope(tenant_a()),
            Action::AssetCreate | Action::AssetRead => asset_scope(tenant_a()),
        };
        for role in [Role::PlatformAdmin, Role::TenantAdmin, Role::Operator, Role::Viewer] {
            let decision = PolicySet::new().evaluate(role, action, Some(&scope), Some(&ctx));
            assert_eq!(
                decision.deny_reason(),
                Some(DenyReason::NoMatchingRule),
                "empty set must deny {role:?} {action:?}"
            );
            assert!(!decision.is_allowed());
        }
    }
}

// --- Every denial carries evidence; is_allowed is false on all denials -------

#[test]
fn every_deny_reason_is_produced_with_stable_evidence_and_false_is_allowed() {
    // One decision per reachable DenyReason (ExplicitDeny is reserved
    // vocabulary — never produced by the Phase 1 allow-only evaluator — so
    // it is pinned by id, not by evaluation).
    let ctx = context_for(tenant_a());
    let decisions = vec![
        // UnknownAction (via the untyped entry).
        PolicySet::new().evaluate_action_id(Role::PlatformAdmin, "dispatch.authorize", Some(&asset_scope(tenant_a())), Some(&ctx)),
        // UnknownScope.
        granted_set().evaluate(Role::TenantAdmin, Action::AssetRead, None, Some(&ctx)),
        // MissingContext.
        granted_set().evaluate(Role::TenantAdmin, Action::AssetRead, Some(&asset_scope(tenant_a())), None),
        // ScopeDenied.
        granted_set().evaluate(Role::TenantAdmin, Action::AssetRead, Some(&asset_scope(tenant_b())), Some(&ctx)),
        // NoMatchingRule.
        granted_set().evaluate(Role::Viewer, Action::AssetRead, Some(&asset_scope(tenant_a())), Some(&ctx)),
    ];
    let expected = [
        (DenyReason::UnknownAction, "authz:deny:unknown-action"),
        (DenyReason::UnknownScope, "authz:deny:unknown-scope"),
        (DenyReason::MissingContext, "authz:deny:missing-context"),
        (DenyReason::ScopeDenied, "authz:deny:scope-denied"),
        (DenyReason::NoMatchingRule, "authz:deny:no-matching-rule"),
    ];
    for (decision, (reason, evidence)) in decisions.iter().zip(expected.iter()) {
        assert!(!decision.is_allowed(), "{reason:?} must not allow");
        assert_eq!(decision.deny_reason(), Some(*reason));
        assert_eq!(decision.allow_reason(), None, "a denial carries no allow reason");
        assert_eq!(decision.evidence().as_str(), *evidence, "{reason:?} evidence is stable for audit");
    }
    // Reserved vocabulary: stable id, never produced in Phase 1.
    assert_eq!(DenyReason::ExplicitDeny.id(), "explicit-deny");
}

#[test]
fn allow_paths_exist_only_where_explicitly_granted() {
    // The counterpart of the deny suite: the one granted triple allows,
    // with the rule's reference as evidence; everything around it denies.
    let ctx = context_for(tenant_a());
    let allow = granted_set().evaluate(
        Role::TenantAdmin,
        Action::AssetRead,
        Some(&asset_scope(tenant_a())),
        Some(&ctx),
    );
    assert!(allow.is_allowed());
    assert_eq!(allow.allow_reason(), Some(AllowReason::PolicyMatch));
    assert_eq!(allow.deny_reason(), None);
    assert_eq!(allow.evidence().as_str(), "policy:tenant-admin:asset.read:asset");
}

#[test]
fn a_second_tenant_context_cannot_read_the_first_tenants_asset() {
    // End-to-end shape of the isolation rule with two real contexts: the
    // same policy, the same asset id — only the verified claims differ, and
    // that alone flips allow to ScopeDenied.
    let ctx_a = context_for(tenant_a());
    let ctx_b = context_for(tenant_b());
    let as_a = granted_set().evaluate(
        Role::TenantAdmin,
        Action::AssetRead,
        Some(&asset_scope(tenant_a())),
        Some(&ctx_a),
    );
    let as_b = granted_set().evaluate(
        Role::TenantAdmin,
        Action::AssetRead,
        Some(&asset_scope(tenant_a())),
        Some(&ctx_b),
    );
    assert!(as_a.is_allowed(), "tenant A reads its own asset");
    assert!(!as_b.is_allowed(), "tenant B must not read tenant A's asset");
    assert_eq!(as_b.deny_reason(), Some(DenyReason::ScopeDenied));
    assert_eq!(as_b.evidence().as_str(), "authz:deny:scope-denied");
}
