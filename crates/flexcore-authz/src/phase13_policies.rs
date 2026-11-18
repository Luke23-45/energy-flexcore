// P13.05 / P13.08 — The recorded Phase 13 policy grants (decision record:
//         phase-13 `records/05_p13_05_role_model.md`, `records/
//         08_p13_08_api_keys_webhook_secret_format.md`).
//
// A separate module (the phase4_policies convention: not an edit of any
// earlier grant set) holding the EXPLICIT allow rules Phase 13 adds to the
// closed vocabularies:
//
// - [`Action::CommandApprove`] for the NEW [`Role::SiteApprover`] at site
//   and asset scope (P13.05; the LR-049 direction): approval duty is
//   separated from administrative rights. The SiteApprover role has NO
//   grant anywhere else on the surface — no registry write, no delete, no
//   API-key administration, no configuration change. What this module
//   deliberately does NOT change: the recorded Phase 4 TenantAdmin
//   `command.approve` grants stand exactly as recorded in
//   `crate::phase4_policies` (same roles, same scope kinds, same rule
//   references); this module ADDS approvers, it never widens existing
//   grants.
// - The `apikey.*` action set (P13.08) for PlatformAdmin and TenantAdmin
//   at TENANT scope only: API keys are tenant-scoped credentials and are
//   administered by tenant/platform administrators.
//
// The module follows the same explicit-grant convention as every earlier
// phase grant set (allow-only `PolicySet`, deny-by-default; every grant
// carries a stable rule reference that becomes the allow evidence).
// Nothing matches by default; the empty set denies everything.
//
// The NEVER-GRANTED permission set is byte-identical after Phase 13:
// `dispatch.authorize`, `dispatch.issue`, `secret.read`, `network.any` —
// none exists on the `Action` surface, none is granted here, none is
// grantable by any principal (human, service account, API key, or machine
// identity). Adapters reach dispatch only through the command lifecycle
// (conventions §6.4).
//
// ABAC-lite note (recorded): "portfolio/site-scoped permissions" are the
// EXISTING typed scope machinery used at its finer levels — a grant is a
// (role, action, scope-kind) triple and evaluation matches the scope KIND
// exactly (scope levels do not inherit, `crate::scope`). Phase 13 adds no
// new scope kind and no rule-expression language: the refinement is that
// approvers and key administrators now have roles whose grants land at
// site/asset/tenant scope deliberately, not because the role forced a
// tenant-wide shape. Site-level rules for existing roles remain exactly
// what earlier phases recorded.

use crate::action::Action;
use crate::policy::{PolicySet, Role};
use crate::scope::ScopeKind;

/// The stable rule-reference prefix of every recorded Phase 13 grant (the
/// allow evidence an authorization decision cites).
pub const PHASE13_POLICY_PREFIX: &str = "policy:phase13";

/// Builds the recorded Phase 13 policy set: exactly the grants of the
/// table in the module header, each with its stable rule reference
/// (`policy:phase13:<role>:<action>:<scope-kind>`).
///
/// Recorded grant decisions (RECOMMENDED DEFAULTS, reviewable):
///
/// | Action | Roles | Scope kinds | Rationale |
/// |---|---|---|---|
/// | `command.approve` | SiteApprover | site, asset | The LR-049 direction: a dedicated approval-duty role scoped to the resources it may approve — never a tenant-wide administrative authority. No tenant-level grant (the Phase 4 rule stands) |
/// | `apikey.create` / `apikey.rotate` / `apikey.revoke` / `apikey.read` | PlatformAdmin, TenantAdmin | tenant | API keys are tenant-scoped credentials administered by tenant/platform administrators (P13.08) |
///
/// What this module deliberately does NOT grant: anything to SiteApprover
/// beyond `command.approve` at site/asset scope; `apikey.*` to Operator,
/// Viewer, or SiteApprover; any tenant-level `command.approve` grant (the
/// Phase 4 recorded default has none and this module adds none); and the
/// four never-granted names (structurally impossible — not on the surface).
pub fn phase13_policies() -> PolicySet {
    let mut set = PolicySet::new();

    // command.approve — SiteApprover ONLY (the new role), at site and
    // asset scope kinds. No tenant-level grant; no grant for any other
    // role (the Phase 4 TenantAdmin grants live in phase4_policies and are
    // untouched).
    for kind in [ScopeKind::Site, ScopeKind::Asset] {
        set.grant(
            Role::SiteApprover,
            Action::CommandApprove,
            kind,
            crate::decision::EvidenceRef::new(format!(
                "{PHASE13_POLICY_PREFIX}:site-approver:command.approve:{}",
                kind.id()
            )),
        );
    }

    // apikey.* — PlatformAdmin and TenantAdmin, at tenant scope only.
    for action in [
        Action::ApiKeyCreate,
        Action::ApiKeyRotate,
        Action::ApiKeyRevoke,
        Action::ApiKeyRead,
    ] {
        for role in [Role::PlatformAdmin, Role::TenantAdmin] {
            set.grant(
                role,
                action,
                ScopeKind::Tenant,
                crate::decision::EvidenceRef::new(format!(
                    "{PHASE13_POLICY_PREFIX}:{}:{}:tenant",
                    role.id(),
                    action.id()
                )),
            );
        }
    }

    set
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decision::DenyReason;
    use crate::policy::PolicySet;
    use crate::scope::{AssetScope, SiteScope};
    use flexcore_contracts::ids::FlexUuid;
    use flexcore_domain::ids::{AssetId, SiteId, TenantId};
    use flexcore_domain::tenant_context::{AuthenticatedClaims, TenantContext, VerifiedClaims};

    const UUID_TENANT: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f8c01";
    const UUID_SITE: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f8c02";
    const UUID_ASSET: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f8c03";

    fn tenant() -> TenantId {
        TenantId::new(FlexUuid::parse(UUID_TENANT).unwrap())
    }

    /// A test tenant context, built through the single public verified
    /// path (tenant_context.rs: tests construct `VerifiedClaims` directly);
    /// the subject is a fixed test identity, never a real principal.
    /// Passing `None` for the context would deny with `MissingContext`
    /// before any rule is reached (policy.rs step 3), so the grant tests
    /// below evaluate against this context.
    fn context() -> TenantContext {
        TenantContext::from_authenticated_claims(AuthenticatedClaims::from_verified(
            VerifiedClaims {
                tenant_id: tenant(),
                subject: "test".to_string(),
            },
        ))
    }

    #[test]
    fn site_approver_may_approve_at_site_and_asset_scope_only() {
        let set = phase13_policies();
        let context = context();
        let site = SiteScope::new(tenant(), SiteId::new(FlexUuid::parse(UUID_SITE).unwrap()));
        let asset =
            AssetScope::new(tenant(), AssetId::new(FlexUuid::parse(UUID_ASSET).unwrap()));
        let tenant_scope = crate::scope::TenantScope::new(tenant());

        assert!(set
            .evaluate(Role::SiteApprover, Action::CommandApprove, Some(&crate::scope::Scope::Site(site)), Some(&context))
            .is_allowed());
        assert!(set
            .evaluate(Role::SiteApprover, Action::CommandApprove, Some(&crate::scope::Scope::Asset(asset)), Some(&context))
            .is_allowed());
        // NO tenant-level approval grant (the Phase 4 rule stands).
        assert_eq!(
            set.evaluate(Role::SiteApprover, Action::CommandApprove, Some(&crate::scope::Scope::Tenant(tenant_scope)), Some(&context))
                .deny_reason(),
            Some(DenyReason::NoMatchingRule)
        );
    }

    #[test]
    fn site_approver_has_no_administrative_grant_anywhere() {
        // The LR-049 separation: the approval-duty role carries no
        // administrative right. A representative sweep of the
        // administrative surface — every one of these denies.
        let set = phase13_policies();
        let context = context();
        let tenant_scope = crate::scope::Scope::Tenant(crate::scope::TenantScope::new(tenant()));
        let admin_actions = [
            Action::TenantCreate,
            Action::TenantDelete,
            Action::PortfolioCreate,
            Action::SiteCreate,
            Action::AssetCreate,
            Action::AssetDelete,
            Action::AssetUpdate,
            Action::AssetDeactivate,
            Action::QuarantineRead,
            Action::ApiKeyCreate,
            Action::ApiKeyRotate,
            Action::ApiKeyRevoke,
            Action::ApiKeyRead,
            Action::CommandCreate,
            Action::CommandValidate,
        ];
        for action in admin_actions {
            assert_eq!(
                set.evaluate(Role::SiteApprover, action, Some(&tenant_scope), Some(&context))
                    .deny_reason(),
                Some(DenyReason::NoMatchingRule),
                "SiteApprover must have no grant for {}",
                action.id()
            );
        }
    }

    #[test]
    fn apikey_actions_grant_only_to_admin_roles_at_tenant_scope() {
        let set = phase13_policies();
        let context = context();
        let tenant_scope = crate::scope::Scope::Tenant(crate::scope::TenantScope::new(tenant()));
        let key_actions = [
            Action::ApiKeyCreate,
            Action::ApiKeyRotate,
            Action::ApiKeyRevoke,
            Action::ApiKeyRead,
        ];
        for action in key_actions {
            for role in [Role::PlatformAdmin, Role::TenantAdmin] {
                assert!(set.evaluate(role, action, Some(&tenant_scope), Some(&context)).is_allowed());
            }
            for role in [Role::Operator, Role::Viewer, Role::SiteApprover] {
                assert_eq!(
                    set.evaluate(role, action, Some(&tenant_scope), Some(&context))
                        .deny_reason(),
                    Some(DenyReason::NoMatchingRule),
                    "{role:?} must have no grant for {}",
                    action.id()
                );
            }
        }
    }

    #[test]
    fn the_never_granted_names_stay_off_the_surface() {
        // Byte-identical guarantee (conventions §6.4): the four names parse
        // to nothing and therefore can only ever deny with UnknownAction.
        for name in ["dispatch.authorize", "dispatch.issue", "secret.read", "network.any"] {
            assert_eq!(Action::parse(name), None, "{name} must stay unparseable");
        }
    }
}
