// P16.19 / P16.22 — The recorded Phase 16 market policy grants (decision
//         records: phase-16 `records/61_p16_19_market_adapter_permission_
//         model.md`, `records/64_p16_22_market_v2_vocabulary_and_market_
//         actions.md`).
//
// A separate module (the phase13_policies convention: not an edit of any
// earlier grant set) holding the EXPLICIT allow rules Phase 16 adds to the
// closed vocabularies for the `market.*` action set. The `market.*` set
// authorizes RECORD and GRANT-ADMINISTRATION work only:
//
// - `market.price.read` (the P16.07 record layer) and
//   `market.settlement.read` (the P16.09/P16.12 record layer) are READ
//   actions over ingested market records. A price record is ingested data —
//   never a command (ADR-010); a settlement read never adjusts, settles, or
//   writes back (the P16.09 no-silent-adjustment rule).
// - `market.bid.compose` (the P16.06 composition service) composes an
//   aggregation-level bid RECORD. It is a RECORD action: no bid is
//   submitted and none can be (`NO_BID_SUBMITTED_STATEMENT`,
//   flexcore-application::bid_composition). Separately from this RBAC
//   surface, the composer requires operator-approved control mode plus an
//   ACTIVE per-market `MarketBidGrant` record (P16.06; conventions §6.4) —
//   the authz grant is one necessary condition, never a sufficient one.
// - `market.grant.issue` / `market.grant.revoke` / `market.grant.read` (the
//   P16.06 grant model) administer the explicit per-market bid grants:
//   issuing, revoking (the supersede model — the old grant row is
//   retained), and reading them. Grant administration is administrative
//   work (PlatformAdmin/TenantAdmin by the recorded default grants).
//
// THE P14.16 COMMAND-RELAY-ONLY RULE, CONSUMED BY CITATION: commands reach
// devices only through the 14-state command lifecycle with all three gates
// distinct; an adapter may relay an already-APPROVED command and may ingest
// external control objects at PROPOSED — never authorize, never issue
// (`crates/flexcore-kernel/src/adapter_permissions.rs`; phase-14 decision
// record 64). A market adapter is no exception: there is NO
// `market.dispatch.*` action on the `Action` surface, none may be added,
// and every `market.*` grant below carries zero dispatch reach. A market
// adapter reaches dispatch only through the command lifecycle.
//
// THE NEVER-GRANTED REVIEW (conventions §6.4; review ONLY — the set stays
// byte-identical, quoted here verbatim from
// `crates/flexcore-kernel/src/manifest.rs`):
//
//     pub const NEVER_GRANTED_PERMISSIONS: [&str; 4] = [
//         "dispatch.authorize",
//         "dispatch.issue",
//         "secret.read",
//         "network.any",
//     ];
//
// Review verdict (recorded in decision record 61 §3): the Phase 16 market
// surface needs NONE of the never-granted ids. Every market capability the
// phase delivers is expressed either as a `market.*` action on THIS surface
// (records and grant administration) or as a kernel-side adapter binding
// (`AdapterBinding`, the P14.16 model) — never as a §5.7 permission id, and
// never by widening the set. The set is unchanged; the in-module test pins
// both the byte-identical set and the disjointness of the `market.*` ids
// from it.
//
// The module follows the same explicit-grant convention as every earlier
// phase grant set (allow-only `PolicySet`, deny-by-default; every grant
// carries a stable rule reference that becomes the allow evidence).
// Nothing matches by default; the empty set denies everything.

use crate::action::Action;
use crate::policy::{PolicySet, Role};
use crate::scope::ScopeKind;

/// The stable rule-reference prefix of every recorded Phase 16 market
/// grant (the allow evidence an authorization decision cites).
pub const PHASE16_MARKET_POLICY_PREFIX: &str = "policy:phase16-market";

/// Builds the recorded Phase 16 market policy set: exactly the grants of
/// the table in the module header, each with its stable rule reference
/// (`policy:phase16-market:<role>:<action>:<scope-kind>`).
///
/// Recorded grant decisions (RECOMMENDED DEFAULTS, reviewable):
///
/// | Action | Roles | Scope kinds | Rationale |
/// |---|---|---|---|
/// | `market.price.read` / `market.settlement.read` | PlatformAdmin, TenantAdmin, Operator, Viewer | tenant | Market records are operator-visible data; the read-only role sees them, and no read action carries any dispatch or settlement-adjustment reach |
/// | `market.bid.compose` | Operator, TenantAdmin | tenant | Bid composition is operational work done under operator-approved mode (the P16.06 composer enforces the mode + the per-market grant record); PlatformAdmin keeps administration, not composition duty; Viewer and SiteApprover get none |
/// | `market.grant.issue` / `market.grant.revoke` / `market.grant.read` | PlatformAdmin, TenantAdmin | tenant | Per-market grants are tenant-scoped authority records administered by tenant/platform administrators (the P13.08 apikey.* posture mirrored) |
///
/// What this module deliberately does NOT grant: anything to SiteApprover
/// (the P13 separation stands — an approval-duty role carries no market
/// authority); `market.bid.compose` to Viewer or PlatformAdmin; any
/// `market.grant.*` action to Operator; and the four never-granted names
/// (structurally impossible — not on the surface).
pub fn phase16_market_policies() -> PolicySet {
    let mut set = PolicySet::new();

    // market.price.read / market.settlement.read — the four data-visible
    // roles, at tenant scope only.
    for action in [Action::MarketPriceRead, Action::MarketSettlementRead] {
        for role in [
            Role::PlatformAdmin,
            Role::TenantAdmin,
            Role::Operator,
            Role::Viewer,
        ] {
            set.grant(
                role,
                action,
                ScopeKind::Tenant,
                crate::decision::EvidenceRef::new(format!(
                    "{PHASE16_MARKET_POLICY_PREFIX}:{}:{}:tenant",
                    role.id(),
                    action.id()
                )),
            );
        }
    }

    // market.bid.compose — Operator and TenantAdmin at tenant scope only.
    // The composer's own gates (operator-approved mode + ACTIVE
    // MarketBidGrant) still apply on every call.
    for role in [Role::Operator, Role::TenantAdmin] {
        set.grant(
            role,
            Action::MarketBidCompose,
            ScopeKind::Tenant,
            crate::decision::EvidenceRef::new(format!(
                "{PHASE16_MARKET_POLICY_PREFIX}:{}:market.bid.compose:tenant",
                role.id()
            )),
        );
    }

    // market.grant.* — PlatformAdmin and TenantAdmin, at tenant scope only
    // (grant administration is administrative authority).
    for action in [
        Action::MarketGrantIssue,
        Action::MarketGrantRevoke,
        Action::MarketGrantRead,
    ] {
        for role in [Role::PlatformAdmin, Role::TenantAdmin] {
            set.grant(
                role,
                action,
                ScopeKind::Tenant,
                crate::decision::EvidenceRef::new(format!(
                    "{PHASE16_MARKET_POLICY_PREFIX}:{}:{}:tenant",
                    role.id(),
                    action.id()
                )),
            );
        }
    }

    set
}

/// The never-granted review as DATA (decision record 61 §3): the review
/// verdict, quotable by records and the console. The set itself stays
/// byte-identical in
/// `flexcore_kernel::manifest::NEVER_GRANTED_PERMISSIONS` — this function
/// only states the Phase 16 review outcome (the authz crate cannot depend
/// on the kernel, so the four ids are restated HERE AS A TEST-LOCAL COPY
/// ONLY; the single home of the set remains the kernel manifest, and the
/// pinning test exists in both crates).
pub fn phase16_market_never_granted_review_statement() -> String {
    format!(
        "Phase 16 (P16.19) never-granted review: the frozen set \
         [\"dispatch.authorize\", \"dispatch.issue\", \"secret.read\", \
         \"network.any\"] is unchanged; the market.* action set carries \
         zero dispatch reach (no market.dispatch.* action exists), the \
         per-market grant is the P16.06 MarketBidGrant RECORD checked by \
         the composer, and market adapters reach dispatch only through the \
         command lifecycle (the P14.16 command-relay-only rule, consumed \
         by citation)."
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decision::DenyReason;
    use crate::scope::TenantScope;
    use flexcore_contracts::ids::FlexUuid;
    use flexcore_domain::ids::TenantId;

    const UUID_TENANT: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f8d01";

    fn tenant() -> TenantId {
        TenantId::new(FlexUuid::parse(UUID_TENANT).unwrap())
    }

    fn tenant_scope() -> crate::scope::Scope {
        crate::scope::Scope::Tenant(TenantScope::new(tenant()))
    }

    #[test]
    fn market_read_actions_are_granted_to_the_four_data_visible_roles() {
        let set = phase16_market_policies();
        for action in [Action::MarketPriceRead, Action::MarketSettlementRead] {
            for role in [
                Role::PlatformAdmin,
                Role::TenantAdmin,
                Role::Operator,
                Role::Viewer,
            ] {
                let decision = set.evaluate(role, action, Some(&tenant_scope()), None);
                assert!(
                    decision.is_allowed(),
                    "{role:?} must be able to read {} at tenant scope",
                    action.id()
                );
            }
        }
    }

    #[test]
    fn bid_compose_is_granted_only_to_operator_and_tenant_admin() {
        let set = phase16_market_policies();
        for role in [Role::Operator, Role::TenantAdmin] {
            assert!(set
                .evaluate(role, Action::MarketBidCompose, Some(&tenant_scope()), None)
                .is_allowed());
        }
        for role in [Role::PlatformAdmin, Role::Viewer, Role::SiteApprover] {
            assert_eq!(
                set.evaluate(role, Action::MarketBidCompose, Some(&tenant_scope()), None)
                    .deny_reason(),
                Some(DenyReason::NoMatchingRule),
                "{role:?} must have no market.bid.compose grant",
            );
        }
    }

    #[test]
    fn grant_administration_is_administrative_only() {
        let set = phase16_market_policies();
        let grant_actions = [
            Action::MarketGrantIssue,
            Action::MarketGrantRevoke,
            Action::MarketGrantRead,
        ];
        for action in grant_actions {
            for role in [Role::PlatformAdmin, Role::TenantAdmin] {
                assert!(set
                    .evaluate(role, action, Some(&tenant_scope()), None)
                    .is_allowed());
            }
            for role in [Role::Operator, Role::Viewer, Role::SiteApprover] {
                assert_eq!(
                    set.evaluate(role, action, Some(&tenant_scope()), None)
                        .deny_reason(),
                    Some(DenyReason::NoMatchingRule),
                    "{role:?} must have no grant for {}",
                    action.id()
                );
            }
        }
    }

    #[test]
    fn site_approver_has_no_market_grant_anywhere() {
        // The P13.05 separation carries forward: the approval-duty role has
        // no market authority of any kind.
        let set = phase16_market_policies();
        let market_actions = [
            Action::MarketPriceRead,
            Action::MarketBidCompose,
            Action::MarketGrantIssue,
            Action::MarketGrantRevoke,
            Action::MarketGrantRead,
            Action::MarketSettlementRead,
        ];
        for action in market_actions {
            assert_eq!(
                set.evaluate(Role::SiteApprover, action, Some(&tenant_scope()), None)
                    .deny_reason(),
                Some(DenyReason::NoMatchingRule),
                "SiteApprover must have no grant for {}",
                action.id()
            );
        }
    }

    #[test]
    fn the_never_granted_names_stay_off_the_surface() {
        // Byte-identical guarantee (conventions §6.4): the four names parse
        // to nothing and therefore can only ever deny with UnknownAction.
        for name in ["dispatch.authorize", "dispatch.issue", "secret.read", "network.any"] {
            assert_eq!(Action::parse(name), None, "{name} must stay unparseable");
        }
        // The market.* ids are disjoint from the never-granted set: no
        // market action name can smuggle a permission id through a ceiling.
        let market_ids = [
            Action::MarketPriceRead.id(),
            Action::MarketBidCompose.id(),
            Action::MarketGrantIssue.id(),
            Action::MarketGrantRevoke.id(),
            Action::MarketGrantRead.id(),
            Action::MarketSettlementRead.id(),
        ];
        for id in market_ids {
            assert!(
                !["dispatch.authorize", "dispatch.issue", "secret.read", "network.any"]
                    .contains(&id),
                "market action '{id}' must never collide with the never-granted set"
            );
        }
        // And no market dispatch action exists — the P14.16
        // command-relay-only rule consumed by citation.
        assert_eq!(Action::parse("market.dispatch.relay"), None);
        assert_eq!(Action::parse("market.dispatch.issue"), None);
        // The review statement quotes the frozen four and the relay rule.
        let statement = phase16_market_never_granted_review_statement();
        for id in ["dispatch.authorize", "dispatch.issue", "secret.read", "network.any"] {
            assert!(statement.contains(id), "statement must quote '{id}'");
        }
        assert!(statement.contains("command lifecycle"));
    }

    #[test]
    fn every_allow_decision_carries_the_stable_rule_reference() {
        let set = phase16_market_policies();
        let decision = set.evaluate(
            Role::TenantAdmin,
            Action::MarketBidCompose,
            Some(&tenant_scope()),
            None,
        );
        assert!(decision.is_allowed());
        assert_eq!(
            decision.evidence().as_str(),
            "policy:phase16-market:tenant-admin:market.bid.compose:tenant"
        );
    }
}
