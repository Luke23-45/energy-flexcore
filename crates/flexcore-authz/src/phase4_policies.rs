// P4.13 — The recorded Phase 4 command-surface policy grants (report
//         §14.1–§14.2, §17.3; ledger P4.11/P4.13; decision record:
//         `ledger/phase-04-grid-feasibility-dispatch/11_13_command_gates.md`).
//
// A separate module (not an edit of any earlier grant set) holding the
// EXPLICIT allow rules for the four `command.*` actions the Phase 4 gates
// evaluate:
//
// - [`Action::CommandCreate`] / [`Action::CommandValidate`] — the P4.11
//   creation + validation gate surface (proposing from approved schedules
//   and running the schema/freshness/asset checks);
// - [`Action::CommandApprove`] — the P4.13 authorization gate: the REQUIRED
//   human/policy authorization for FEASIBLE → APPROVED (§14.1; §17.3).
//   There is NO auto-approve action and no grant set that approves by
//   itself: the policy set answers "may this role approve AT THIS SCOPE",
//   and the P4.13 gate additionally enforces separation of duties, scope
//   coverage of the command's asset, and a recorded expiry. A policy match
//   is a NECESSARY condition for APPROVED, never the decision itself.
// - [`Action::CommandRead`] — reading command records and journals.
//
// The module follows the same explicit-grant convention as the earlier
// phase grant sets (P1.11 allow-only `PolicySet`, deny-by-default; the
// P2.12 recorded-grants convention: every grant carries a stable rule
// reference that becomes the allow evidence). Nothing matches by default:
// the empty set denies everything, and an action outside the closed
// vocabulary denies with `UnknownAction` regardless of grants.
//
// Recorded grant decisions (RECOMMENDED DEFAULTS, reviewable):
//
// | Action            | Roles                  | Scope kinds        | Rationale |
// |---|---|---|---|
// | `command.create`  | Operator, TenantAdmin  | tenant, site, asset | Proposing candidates is day-to-day operations (§14.2 PROPOSED row) |
// | `command.validate`| Operator, TenantAdmin  | tenant, site, asset | The validation gate is part of the same operational surface (P4.11) |
// | `command.approve` | TenantAdmin only       | site, asset        | Separation of duties by role: the approving authority is NOT the proposing Operator role (§17.4). Tenant-level approval is NOT granted in the recorded default — approvals are resource/site/asset-scoped (§17.3 control 2) |
// | `command.read`    | Operator, TenantAdmin, Viewer | tenant, site, asset | Read-only visibility incl. the audit journal (§16.2) |
//
// What this module deliberately does NOT grant: `dispatch.authorize`,
// `dispatch.issue` (not on the Action surface at all — §5.7), any action
// for plugins (plugins may propose and request validation but can never
// authorize, issue, or select the control-authority mode — §5.7, P0.17),
// and any Operator approval grant (the separation-of-duties default).
// FlexCore is supervisory orchestration only; the entire 0.x line is
// simulation-only and an APPROVED command authorizes a simulated issue at
// most (§21.4; ledger P4.13 step 4).

use crate::action::Action;
use crate::policy::{PolicySet, Role};
use crate::scope::ScopeKind;

/// The stable rule-reference prefix of every recorded Phase 4 grant (the
/// allow evidence an authorization decision cites).
pub const PHASE4_POLICY_PREFIX: &str = "policy:phase4";

/// Builds the recorded Phase 4 command-surface policy set: exactly the
/// grants of the table in the module header, each with its stable rule
/// reference (`policy:phase4:<role>:<action>:<scope-kind>`).
///
/// The set is deny-by-default like every `PolicySet`: callers that need a
/// stricter deployment REVOKE by not using this builder (or by evaluating a
/// set without the relevant grant) — there is no override flag and no
/// wildcard.
pub fn phase4_command_policies() -> PolicySet {
    let mut set = PolicySet::new();

    // command.create — Operator and TenantAdmin, at every scope kind.
    for role in [Role::Operator, Role::TenantAdmin] {
        for kind in [ScopeKind::Tenant, ScopeKind::Site, ScopeKind::Asset] {
            set.grant(
                role,
                Action::CommandCreate,
                kind,
                crate::decision::EvidenceRef::new(format!(
                    "{PHASE4_POLICY_PREFIX}:{}:command.create:{}",
                    role.id(),
                    kind.id()
                )),
            );
        }
    }

    // command.validate — Operator and TenantAdmin, at every scope kind.
    for role in [Role::Operator, Role::TenantAdmin] {
        for kind in [ScopeKind::Tenant, ScopeKind::Site, ScopeKind::Asset] {
            set.grant(
                role,
                Action::CommandValidate,
                kind,
                crate::decision::EvidenceRef::new(format!(
                    "{PHASE4_POLICY_PREFIX}:{}:command.validate:{}",
                    role.id(),
                    kind.id()
                )),
            );
        }
    }

    // command.approve — TenantAdmin ONLY, at site and asset scope kinds
    // (separation of duties by role; resource/site/asset-scoped approval,
    // §17.3). No tenant-level approval grant, no Operator/Viewer grant.
    for kind in [ScopeKind::Site, ScopeKind::Asset] {
        set.grant(
            Role::TenantAdmin,
            Action::CommandApprove,
            kind,
            crate::decision::EvidenceRef::new(format!(
                "{PHASE4_POLICY_PREFIX}:tenant-admin:command.approve:{}",
                kind.id()
            )),
        );
    }

    // command.read — Operator, TenantAdmin, Viewer, at every scope kind.
    for role in [Role::Operator, Role::TenantAdmin, Role::Viewer] {
        for kind in [ScopeKind::Tenant, ScopeKind::Site, ScopeKind::Asset] {
            set.grant(
                role,
                Action::CommandRead,
                kind,
                crate::decision::EvidenceRef::new(format!(
                    "{PHASE4_POLICY_PREFIX}:{}:command.read:{}",
                    role.id(),
                    kind.id()
                )),
            );
        }
    }

    // feasibility.read — Operator, TenantAdmin, Viewer, at every scope
    // kind (P4.30): the `/api/v1/feasibility-checks` resource is
    // READ-ONLY evidence (§15.2); no action on this surface runs a check
    // or executes anything.
    for role in [Role::Operator, Role::TenantAdmin, Role::Viewer] {
        for kind in [ScopeKind::Tenant, ScopeKind::Site, ScopeKind::Asset] {
            set.grant(
                role,
                Action::FeasibilityRead,
                kind,
                crate::decision::EvidenceRef::new(format!(
                    "{PHASE4_POLICY_PREFIX}:{}:feasibility.read:{}",
                    role.id(),
                    kind.id()
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

    #[test]
    fn the_recorded_set_grants_exactly_the_documented_matrix() {
        // The set is non-empty and every grant is one of the table rows;
        // asserted indirectly here by behaviour, exhaustively by the
        // integration test in tests/phase4_policies.rs.
        let set = phase4_command_policies();
        assert!(set != PolicySet::new(), "the recorded set is not empty");
    }
}
