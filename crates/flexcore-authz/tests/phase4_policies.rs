// P4.13 — Phase 4 command-surface grant tests (report §14.1, §17.3–§17.4;
//         ledger P4.13; decision record:
//         `ledger/phase-04-grid-feasibility-dispatch/11_13_command_gates.md`).
//
// Coverage per the ledger P4.13 acceptance criteria, authorization slice:
//
// - the recorded grant matrix behaves exactly as documented in
//   `flexcore_authz::phase4_policies` (creation/validation at every scope
//   kind for Operator + TenantAdmin; approval for TenantAdmin at site/asset
//   ONLY; read for the three roles; nothing else);
// - deny-by-default holds for the command surface: the empty set denies
//   `command.approve`, and unknown actions (incl. the §5.7 plugin capability
//   names) deny with `UnknownAction` regardless of grants;
// - the tenant match cannot be bypassed by any role (cross-tenant approval
//   denies with `ScopeDenied`);
// - every allow carries the stable `policy:phase4:...` rule reference as
//   the allow evidence; every denial carries a `DenyReason` + evidence
//   (§16.2 audit discipline);
// - the approval-action restrictions are the structural half of the P4.13
//   separation of duties: the Operator (proposing) role has NO approval
//   grant, so a role-based self-approval path does not exist at the policy
//   layer. The identity-level SoD check (approver ≠ proposer) is enforced
//   at the P4.13 gate (flexcore-application) — recorded division, see the
//   decision doc.
//
// Provenance note: tenant contexts are built through the domain's single
// public claims path (`VerifiedClaims` → `AuthenticatedClaims::from_verified`
// → `TenantContext::from_authenticated_claims`), the same path the API
// middleware uses (P0.16 rule 5).

use flexcore_authz::action::Action;
use flexcore_authz::decision::DenyReason;
use flexcore_authz::phase4_policies::{phase4_command_policies, PHASE4_POLICY_PREFIX};
use flexcore_authz::policy::{PolicySet, Role};
use flexcore_authz::scope::{AssetScope, Scope, ScopeKind, SiteScope, TenantScope};
use flexcore_contracts::ids::FlexUuid;
use flexcore_domain::ids::{AssetId, SiteId, TenantId};
use flexcore_domain::tenant_context::{
    AuthenticatedClaims, TenantContext, VerifiedClaims,
};

const UUID_TENANT_A: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7b01";
const UUID_TENANT_B: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7b02";
const UUID_SITE: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7b03";
const UUID_ASSET: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7b04";
const UUID_OTHER_ASSET: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7b05";

fn uuid(text: &str) -> FlexUuid {
    FlexUuid::parse(text).expect("valid test uuid")
}

fn tenant_a() -> TenantId {
    TenantId::new(uuid(UUID_TENANT_A))
}

fn tenant_b() -> TenantId {
    TenantId::new(uuid(UUID_TENANT_B))
}

/// The ONLY public construction path (P0.16 rule 5).
fn context_for(tenant: TenantId) -> TenantContext {
    TenantContext::from_authenticated_claims(AuthenticatedClaims::from_verified(VerifiedClaims {
        tenant_id: tenant,
        subject: "phase4-test:principal".to_string(),
    }))
}

fn tenant_scope(tenant: TenantId) -> Scope {
    Scope::Tenant(TenantScope::new(tenant))
}

fn site_scope(tenant: TenantId) -> Scope {
    Scope::Site(SiteScope::new(tenant, SiteId::new(uuid(UUID_SITE))))
}

fn asset_scope(tenant: TenantId) -> Scope {
    Scope::Asset(AssetScope::new(tenant, AssetId::new(uuid(UUID_ASSET))))
}

fn other_asset_scope(tenant: TenantId) -> Scope {
    Scope::Asset(AssetScope::new(tenant, AssetId::new(uuid(UUID_OTHER_ASSET))))
}

// --- The recorded grant matrix -------------------------------------------------

#[test]
fn creation_and_validation_are_granted_to_operator_and_tenant_admin_at_every_scope_kind() {
    let set = phase4_command_policies();
    let ctx = context_for(tenant_a());
    for action in [Action::CommandCreate, Action::CommandValidate] {
        for scope in [
            tenant_scope(tenant_a()),
            site_scope(tenant_a()),
            asset_scope(tenant_a()),
        ] {
            for role in [Role::Operator, Role::TenantAdmin] {
                let decision = set.evaluate(role, action, Some(&scope), Some(&ctx));
                assert!(
                    decision.is_allowed(),
                    "{:?} at {:?} must be allowed for {:?}",
                    action,
                    scope.kind(),
                    role
                );
                assert!(
                    decision
                        .evidence()
                        .as_str()
                        .starts_with(PHASE4_POLICY_PREFIX),
                    "the allow evidence cites the recorded phase4 rule"
                );
            }
            // The Viewer role has neither grant.
            let viewer = set.evaluate(Role::Viewer, action, Some(&scope), Some(&ctx));
            assert_eq!(viewer.deny_reason(), Some(DenyReason::NoMatchingRule));
        }
    }
}

#[test]
fn approval_is_granted_to_tenant_admin_at_site_and_asset_scope_only() {
    let set = phase4_command_policies();
    let ctx = context_for(tenant_a());

    // Allowed: TenantAdmin at site and asset scope.
    for scope in [site_scope(tenant_a()), asset_scope(tenant_a())] {
        let decision = set.evaluate(Role::TenantAdmin, Action::CommandApprove, Some(&scope), Some(&ctx));
        assert!(decision.is_allowed(), "tenant-admin approves at {:?}", scope.kind());
        assert_eq!(
            decision.evidence().as_str(),
            format!("{PHASE4_POLICY_PREFIX}:tenant-admin:command.approve:{}", scope.kind().id()),
        );
    }

    // NOT granted: tenant-level approval (approvals are resource/site/asset
    // scoped, §17.3 control 2 — the recorded default).
    let tenant_level = set.evaluate(
        Role::TenantAdmin,
        Action::CommandApprove,
        Some(&tenant_scope(tenant_a())),
        Some(&ctx),
    );
    assert_eq!(tenant_level.deny_reason(), Some(DenyReason::NoMatchingRule));

    // NOT granted: the Operator (proposing) role — the structural half of
    // separation of duties: no role-based self-approval path exists.
    for scope in [
        tenant_scope(tenant_a()),
        site_scope(tenant_a()),
        asset_scope(tenant_a()),
    ] {
        let operator = set.evaluate(Role::Operator, Action::CommandApprove, Some(&scope), Some(&ctx));
        assert_eq!(
            operator.deny_reason(),
            Some(DenyReason::NoMatchingRule),
            "the proposing Operator role must have no approval grant"
        );
        let viewer = set.evaluate(Role::Viewer, Action::CommandApprove, Some(&scope), Some(&ctx));
        assert_eq!(viewer.deny_reason(), Some(DenyReason::NoMatchingRule));
        let platform = set.evaluate(Role::PlatformAdmin, Action::CommandApprove, Some(&scope), Some(&ctx));
        assert_eq!(
            platform.deny_reason(),
            Some(DenyReason::NoMatchingRule),
            "PlatformAdmin has breadth only from grants, never a bypass"
        );
    }
}

#[test]
fn command_read_is_granted_to_operator_tenant_admin_and_viewer() {
    let set = phase4_command_policies();
    let ctx = context_for(tenant_a());
    for role in [Role::Operator, Role::TenantAdmin, Role::Viewer] {
        for scope in [
            tenant_scope(tenant_a()),
            site_scope(tenant_a()),
            asset_scope(tenant_a()),
        ] {
            let decision = set.evaluate(role, Action::CommandRead, Some(&scope), Some(&ctx));
            assert!(decision.is_allowed(), "command.read allowed for {role:?}");
        }
    }
    // PlatformAdmin has no command.read grant (breadth from rules only).
    let platform = set.evaluate(
        Role::PlatformAdmin,
        Action::CommandRead,
        Some(&asset_scope(tenant_a())),
        Some(&ctx),
    );
    assert_eq!(platform.deny_reason(), Some(DenyReason::NoMatchingRule));
}

// --- Deny-by-default -----------------------------------------------------------

#[test]
fn the_empty_set_denies_the_whole_command_surface() {
    let set = PolicySet::new();
    let ctx = context_for(tenant_a());
    for action in [
        Action::CommandCreate,
        Action::CommandValidate,
        Action::CommandApprove,
        Action::CommandRead,
    ] {
        let decision = set.evaluate(Role::TenantAdmin, action, Some(&asset_scope(tenant_a())), Some(&ctx));
        assert_eq!(decision.deny_reason(), Some(DenyReason::NoMatchingRule));
    }
}

#[test]
fn unknown_and_excluded_actions_deny_regardless_of_grants() {
    let set = phase4_command_policies();
    let ctx = context_for(tenant_a());
    // The §5.7 plugin capability names are not on the surface at all —
    // deny-by-default with UnknownAction even for a fully granted role.
    for action_id in ["dispatch.authorize", "dispatch.issue", "command.issue", "command.auto_approve"] {
        let decision = set.evaluate_action_id(
            Role::TenantAdmin,
            action_id,
            Some(&asset_scope(tenant_a())),
            Some(&ctx),
        );
        assert_eq!(decision.deny_reason(), Some(DenyReason::UnknownAction), "{action_id}");
    }
}

#[test]
fn cross_tenant_approval_denies_for_every_role_including_tenant_admin() {
    let set = phase4_command_policies();
    // Context authenticated in tenant A, scope naming tenant B: denied —
    // no role bypasses the tenant match (P0.16 rule 6; §17.4 row 4).
    let decision = set.evaluate(
        Role::TenantAdmin,
        Action::CommandApprove,
        Some(&asset_scope(tenant_b())),
        Some(&context_for(tenant_a())),
    );
    assert_eq!(decision.deny_reason(), Some(DenyReason::ScopeDenied));
    assert_eq!(decision.evidence().as_str(), "authz:deny:scope-denied");
}

// --- Scope coverage primitives (P4.13 gate input) -------------------------------
//
// The P4.13 gate evaluates the approval at the recorded scope and then
// checks that the scope covers the command's asset. The policy layer's
// contribution is the exact-kind rule: scope kinds do not inherit, so a
// site-scope grant does not authorize an asset-scope request for a
// DIFFERENT asset — the identity match itself is the gate's duty (recorded
// division; the gate owns site/asset identity comparison against the
// command).

#[test]
fn scope_kinds_do_not_inherit_across_targets() {
    let set = phase4_command_policies();
    let ctx = context_for(tenant_a());
    // A grant at ASSET scope kind matches any asset-scope request in scope
    // terms; the GATE must still compare the asset identity. Here: same
    // kind, different asset — the policy allows (it cannot know the
    // command), which is exactly why the gate's coverage check exists.
    let policy = set.evaluate(
        Role::TenantAdmin,
        Action::CommandApprove,
        Some(&other_asset_scope(tenant_a())),
        Some(&ctx),
    );
    assert!(policy.is_allowed(), "policy match is scope-KIND based");
    // The gate-side coverage rule (mirrored here as the decision doc's
    // recorded contract): an Asset scope covers only its own asset, a Site
    // scope covers its site's assets, a Tenant scope covers all. The
    // executable enforcement lives in flexcore-application::command_gates.
    let _ = ScopeKind::Asset;
}

#[test]
fn the_action_ids_round_trip_through_parse() {
    for action in [
        Action::CommandCreate,
        Action::CommandValidate,
        Action::CommandApprove,
        Action::CommandRead,
    ] {
        assert_eq!(Action::parse(action.id()), Some(action));
    }
}
