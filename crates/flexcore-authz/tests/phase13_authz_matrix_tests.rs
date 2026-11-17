// P13.19 — the authz-matrix EXTENSION tests (wave 2; WRITTEN-UNRUN —
// never compiled or executed; execution is the human build/test gate,
// conventions §1 item 3).
//
// Extends the P1.23 security suite and record 05's in-crate tests at
// integration level, over the PUBLIC `PolicySet::evaluate` entry (the
// P1.23 provenance note applies verbatim: contexts are built through the
// domain's single public claims path; a bypass would be a compile error).
//
// What is under test (record 05 §3's table, behaviorally):
// - SiteApprover (the P13.05/LR-049 approval-duty role): allows
//   `command.approve` ONLY at site/asset scope within its tenant; denies
//   at tenant scope; denies the whole administrative surface; denies
//   cross-tenant scopes.
// - apikey.* (the P13.08 surface): grants ONLY to PlatformAdmin and
//   TenantAdmin, ONLY at tenant scope; denies Operator/Viewer/
//   SiteApprover; denies at site/asset scope kinds.
// - The never-granted permission set (`dispatch.authorize`,
//   `dispatch.issue`, `secret.read`, `network.any`) stays STRUCTURALLY
//   unreachable: the ids do not parse (deny UnknownAction) — asserted
//   through the public string-evaluation entry for every principal class.
//
// NOTHING here grants, executes, or authorizes anything in any
// environment: this is a written-unrun test file over pure policy
// evaluation. The frozen command lifecycle, the frozen 26-event
// vocabulary, and the never-granted permission set are untouched by this
// file (it reads nothing but the closed enums and the policy sets).
use flexcore_authz::action::Action;
use flexcore_authz::decision::AuthorizationDecision;
use flexcore_authz::phase13_policies::{PHASE13_POLICY_PREFIX, phase13_policies};
use flexcore_authz::policy::{PolicySet, Role};
use flexcore_authz::scope::{AssetScope, Scope, ScopeKind, SiteScope, TenantScope};
use flexcore_contracts::ids::FlexUuid;
use flexcore_domain::ids::{AssetId, SiteId, TenantId};
use flexcore_domain::tenant_context::{
    AuthenticatedClaims, TenantContext, VerifiedClaims,
};

const UUID_TENANT_A: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f8a01";
const UUID_TENANT_B: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f8a02";
const UUID_SITE: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f8a03";
const UUID_ASSET: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f8a04";

fn uuid(text: &str) -> FlexUuid {
    FlexUuid::parse(text).expect("valid test uuid")
}

fn tenant_a() -> TenantId {
    TenantId::new(uuid(UUID_TENANT_A))
}

fn tenant_b() -> TenantId {
    TenantId::new(uuid(UUID_TENANT_B))
}

/// The single public claims path (P0.16 rule 5) — the same construction
/// the API middleware uses.
fn context_for(tenant: TenantId, subject: &str) -> TenantContext {
    TenantContext::from_authenticated_claims(AuthenticatedClaims::from_verified(
        VerifiedClaims {
            tenant_id: tenant,
            subject: subject.to_string(),
        },
    ))
}

fn evaluate(
    policies: &PolicySet,
    role: Role,
    action: Action,
    scope: Scope,
    ctx: &TenantContext,
) -> AuthorizationDecision {
    policies.evaluate(role, action, Some(&scope), Some(ctx))
}

fn assert_denied(decision: &AuthorizationDecision, what: &str) {
    assert!(
        !decision.is_allowed(),
        "expected a DENY for {what}; got an allow — the recorded matrix broke"
    );
    assert!(
        decision.deny_reason().is_some(),
        "every denial carries a reason (§10.1) — {what} denied without one"
    );
}

fn assert_allowed(decision: &AuthorizationDecision, what: &str) {
    assert!(
        decision.is_allowed(),
        "expected an ALLOW for {what}; got: {:?}",
        decision.deny_reason()
    );
}

// --- SiteApprover: the LR-049 separation (record 05 §3 row 1) --------------------

#[test]
fn site_approver_approves_at_site_scope_within_the_tenant() {
    let policies = phase13_policies();
    let ctx = context_for(tenant_a(), "user:approver-1");
    let decision = evaluate(
        &policies,
        Role::SiteApprover,
        Action::CommandApprove,
        Scope::Site(SiteScope::new(tenant_a(), SiteId::new(uuid(UUID_SITE)))),
        &ctx,
    );
    assert_allowed(&decision, "site-approver command.approve at own-tenant site scope");
    // The allow evidence cites the recorded phase-13 rule reference.
    if let AuthorizationDecision::Allow(_, evidence) = &decision {
        assert!(
            evidence.as_str().starts_with(PHASE13_POLICY_PREFIX),
            "the allow cites a phase-13 rule reference (auditable per §17.3); got: {}",
            evidence.as_str()
        );
    }
}

#[test]
fn site_approver_approves_at_asset_scope_within_the_tenant() {
    let policies = phase13_policies();
    let ctx = context_for(tenant_a(), "user:approver-1");
    let decision = evaluate(
        &policies,
        Role::SiteApprover,
        Action::CommandApprove,
        Scope::Asset(AssetScope::new(
            tenant_a(),
            AssetId::new(uuid(UUID_ASSET)),
        )),
        &ctx,
    );
    assert_allowed(&decision, "site-approver command.approve at own-tenant asset scope");
}

#[test]
fn site_approver_has_no_tenant_level_approval_grant() {
    let policies = phase13_policies();
    let ctx = context_for(tenant_a(), "user:approver-1");
    assert_denied(
        &evaluate(
            &policies,
            Role::SiteApprover,
            Action::CommandApprove,
            Scope::Tenant(TenantScope::new(tenant_a())),
            &ctx,
        ),
        "site-approver command.approve at TENANT scope (the Phase 4 rule stands: no tenant-level approval grant for anyone)",
    );
}

#[test]
fn site_approver_denies_cross_tenant_scopes() {
    let policies = phase13_policies();
    let ctx = context_for(tenant_a(), "user:approver-1");
    assert_denied(
        &evaluate(
            &policies,
            Role::SiteApprover,
            Action::CommandApprove,
            Scope::Site(SiteScope::new(tenant_b(), SiteId::new(uuid(UUID_SITE)))),
            &ctx,
        ),
        "site-approver command.approve against another tenant's site",
    );
    assert_denied(
        &evaluate(
            &policies,
            Role::SiteApprover,
            Action::CommandApprove,
            Scope::Asset(AssetScope::new(
                tenant_b(),
                AssetId::new(uuid(UUID_ASSET)),
            )),
            &ctx,
        ),
        "site-approver command.approve against another tenant's asset",
    );
}

#[test]
fn site_approver_holds_no_administrative_surface_grant() {
    // The record 05 §3 sweep: the administrative surface denies for the
    // approval-duty role — the LR-049 separation is the point.
    let policies = phase13_policies();
    let ctx = context_for(tenant_a(), "user:approver-1");
    let administrative = [
        (Action::TenantCreate, ScopeKind::Tenant),
        (Action::TenantDelete, ScopeKind::Tenant),
        (Action::PortfolioCreate, ScopeKind::Tenant),
        (Action::PortfolioDelete, ScopeKind::Tenant),
        (Action::SiteCreate, ScopeKind::Tenant),
        (Action::SiteDelete, ScopeKind::Tenant),
        (Action::AssetCreate, ScopeKind::Tenant),
        (Action::AssetDelete, ScopeKind::Tenant),
        (Action::AssetUpdate, ScopeKind::Tenant),
        (Action::AssetDeactivate, ScopeKind::Tenant),
        (Action::ApiKeyCreate, ScopeKind::Tenant),
        (Action::ApiKeyRotate, ScopeKind::Tenant),
        (Action::ApiKeyRevoke, ScopeKind::Tenant),
        (Action::ApiKeyRead, ScopeKind::Tenant),
        (Action::CommandCreate, ScopeKind::Tenant),
    ];
    for (action, kind) in administrative {
        let scope = match kind {
            ScopeKind::Tenant => Scope::Tenant(TenantScope::new(tenant_a())),
            ScopeKind::Site => Scope::Site(SiteScope::new(tenant_a(), SiteId::new(uuid(UUID_SITE)))),
            ScopeKind::Asset => {
                Scope::Asset(AssetScope::new(tenant_a(), AssetId::new(uuid(UUID_ASSET))))
            }
        };
        assert_denied(
            &evaluate(&policies, Role::SiteApprover, action, scope, &ctx),
            &format!("site-approver {} at {kind:?} scope", action.id()),
        );
    }
}

// --- apikey.*: the P13.08 admin surface (record 05 §3 rows 2–5) ------------------

#[test]
fn apikey_actions_grant_only_the_two_admin_roles_at_tenant_scope() {
    let policies = phase13_policies();
    let ctx = context_for(tenant_a(), "user:admin-1");
    for action in [
        Action::ApiKeyCreate,
        Action::ApiKeyRotate,
        Action::ApiKeyRevoke,
        Action::ApiKeyRead,
    ] {
        assert_allowed(
            &evaluate(
                &policies,
                Role::PlatformAdmin,
                action,
                Scope::Tenant(TenantScope::new(tenant_a())),
                &ctx,
            ),
            &format!("platform-admin {} at tenant scope", action.id()),
        );
        assert_allowed(
            &evaluate(
                &policies,
                Role::TenantAdmin,
                action,
                Scope::Tenant(TenantScope::new(tenant_a())),
                &ctx,
            ),
            &format!("tenant-admin {} at tenant scope", action.id()),
        );
    }
}

#[test]
fn apikey_actions_deny_operator_viewer_and_site_approver() {
    let policies = phase13_policies();
    let ctx = context_for(tenant_a(), "user: anyone");
    for role in [Role::Operator, Role::Viewer, Role::SiteApprover] {
        for action in [
            Action::ApiKeyCreate,
            Action::ApiKeyRotate,
            Action::ApiKeyRevoke,
            Action::ApiKeyRead,
        ] {
            assert_denied(
                &evaluate(
                    &policies,
                    role,
                    action,
                    Scope::Tenant(TenantScope::new(tenant_a())),
                    &ctx,
                ),
                &format!("{:?} {} at tenant scope", role, action.id()),
            );
        }
    }
}

#[test]
fn apikey_actions_deny_at_site_and_asset_scope_kinds() {
    // The grants are tenant-scoped ONLY (record 05 §3); scope levels do
    // not inherit (scope.rs), so a site/asset-kind request denies even
    // for the granted roles.
    let policies = phase13_policies();
    let ctx = context_for(tenant_a(), "user:admin-1");
    for action in [Action::ApiKeyCreate, Action::ApiKeyRead] {
        assert_denied(
            &evaluate(
                &policies,
                Role::TenantAdmin,
                action,
                Scope::Site(SiteScope::new(tenant_a(), SiteId::new(uuid(UUID_SITE)))),
                &ctx,
            ),
            &format!("tenant-admin {} at SITE scope (grants are tenant-kind only)", action.id()),
        );
        assert_denied(
            &evaluate(
                &policies,
                Role::PlatformAdmin,
                action,
                Scope::Asset(AssetScope::new(
                    tenant_a(),
                    AssetId::new(uuid(UUID_ASSET)),
                )),
                &ctx,
            ),
            &format!("platform-admin {} at ASSET scope (grants are tenant-kind only)", action.id()),
        );
    }
}

#[test]
fn apikey_actions_deny_cross_tenant_at_tenant_scope() {
    let policies = phase13_policies();
    let ctx = context_for(tenant_a(), "user:admin-1");
    assert_denied(
        &evaluate(
            &policies,
            Role::TenantAdmin,
            Action::ApiKeyCreate,
            Scope::Tenant(TenantScope::new(tenant_b())),
            &ctx,
        ),
        "tenant-admin apikey.create against another tenant (the cross-tenant match denies)",
    );
}

// --- The never-granted permission set: structurally unreachable -------------------

#[test]
fn the_never_granted_names_still_do_not_parse() {
    // The four names are NOT on the closed Action surface; parse → None
    // (deny UnknownAction). Byte-identical guarantee, re-asserted here as
    // the record 05/action.rs tests do — through the PUBLIC parse entry.
    assert_eq!(Action::parse("dispatch.authorize"), None);
    assert_eq!(Action::parse("dispatch.issue"), None);
    assert_eq!(Action::parse("secret.read"), None);
    assert_eq!(Action::parse("network.any"), None);
}

#[test]
fn the_never_granted_names_deny_through_the_string_evaluation_entry_for_every_role() {
    // Even if a future refactor broke the parse guard, the public
    // string-evaluation entry (evaluate_action_id) denies for every
    // principal class — the belt AND the suspenders, asserted together.
    let policies = phase13_policies();
    let ctx = context_for(tenant_a(), "user:anyone");
    let scope = Scope::Tenant(TenantScope::new(tenant_a()));
    for role in [
        Role::PlatformAdmin,
        Role::TenantAdmin,
        Role::Operator,
        Role::Viewer,
        Role::SiteApprover,
    ] {
        for name in ["dispatch.authorize", "dispatch.issue", "secret.read", "network.any"] {
            let decision = policies.evaluate_action_id(role, name, Some(&scope), Some(&ctx));
            assert_denied(
                &decision,
                &format!("{:?} requesting '{name}' (never granted to any principal class)", role),
            );
        }
    }
}

// --- The single-vocabulary rule (records 02/03 consumed by citation) --------------

#[test]
fn the_role_id_vocabulary_is_the_same_set_the_resolver_paths_map() {
    // Records 02/03 map SCIM groups and token groups through the SAME
    // closed role ids (`parse_role` in apps/control-plane auth.rs and
    // `map_role_claims` in oidc.rs). The authz-side contract this file
    // pins is that every role id the policy sets grant for is one of the
    // five ids and that the ids are distinct — the vocabulary cannot
    // fork between provisioning and login without this failing.
    let ids: [&str; 5] = [
        Role::PlatformAdmin.id(),
        Role::TenantAdmin.id(),
        Role::Operator.id(),
        Role::Viewer.id(),
        Role::SiteApprover.id(),
    ];
    assert_eq!(
        ids,
        ["platform-admin", "tenant-admin", "operator", "viewer", "site-approver"],
        "the closed role-id vocabulary (the record 03 §3 single home)"
    );
    for (i, a) in ids.iter().enumerate() {
        for b in ids.iter().skip(i + 1) {
            assert_ne!(a, b, "role ids must be distinct");
        }
    }
}
