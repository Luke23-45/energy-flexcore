// P1.11 — The `Action` vocabulary: the Phase 1 authorization surface.
//
// Ledger P1.11 step 5 (deny-by-default): "an unknown action ... denies."
//
// The surface is a **closed enum** covering exactly the Phase 1 registry
// actions (P0.10/P0.15 vocabulary; the P1.17 API surface). Each action has a
// canonical lowercase dotted id. `Action::parse` returns `None` for
// anything else, and the policy layer turns that into
// [`crate::decision::DenyReason::UnknownAction`] — an untyped caller (an
// API path segment today, a capability string in a later phase) gets a
// reasoned deny, never a parse-error panic and never a match-all.
//
// Extending the surface means adding a variant, its `id` arm, its `parse`
// arm — and an explicit rule grant in the `PolicySet`. Nothing matches by
// default.
//
// Deliberate exclusion: the §5.7 plugin capability names `dispatch.authorize`
// and `dispatch.issue` do NOT exist on this surface and must never enter it.
// Final dispatch authorization is the Phase 4 protected kernel;
// `dispatch.authorize` is never granted to general plugins (report §5.7;
// P0.17; P0.19). See the crate-root boundary statement.
//
// Phase 4 refinement (P4.13): the APPLICATION-surface command actions
// (`command.create` / `command.validate` / `command.approve` / `command.read`)
// ARE on this surface — they authorize human/policy actors at the P4.11
// creation + validation gates and the P4.13 authorization gate, evaluated
// against the recorded grants in `crate::phase4_policies`. They are human-
// approval-surface actions, not the §5.7 plugin capabilities: no action on
// this surface issues a command (ISSUED is P4.16's signed, sequenced step),
// and no action skips the FEASIBLE precondition of §14.1. There is no
// auto-approve action and none may be added.
//
// Modelling note (recorded): tenant creation is a platform-level action,
// but evaluation stays uniform — it is evaluated against an explicit
// `TenantScope` whose tenant must equal the acting principal's context
// tenant. No role, including `PlatformAdmin`, bypasses the tenant match;
// breadth comes from rule grants, never from a bypass.

/// An action on the Phase 1 authorization surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Action {
    /// Create a tenant (platform-level registry action).
    TenantCreate,
    /// Read tenant registry data (the caller's own tenant only; the API
    /// never enumerates other tenants — coordinator addition 2026-11-07,
    /// required by the P1.17 GET routes on `/api/v1/tenants`).
    TenantRead,
    /// Create a portfolio within the acting principal's tenant.
    PortfolioCreate,
    /// Create a site within the acting principal's tenant.
    SiteCreate,
    /// Create an asset within the acting principal's tenant.
    AssetCreate,
    /// Read an asset.
    AssetRead,
    /// Read a portfolio.
    PortfolioRead,
    /// Read a site.
    SiteRead,

    // --- Phase 2 additions (P2.12/P2.17; closed-enum extension rule: a
    // variant, its `id` arm, its `parse` arm, and explicit policy grants —
    // nothing matches by default) -------------------------------------------
    /// Delete a tenant (P2.02 RESTRICT rule enforced by the caller).
    TenantDelete,
    /// Delete a portfolio (P2.02 RESTRICT rule enforced by the caller).
    PortfolioDelete,
    /// Delete a site (P2.02 RESTRICT rule enforced by the caller).
    SiteDelete,
    /// Delete an asset (a registry leaf; telemetry history is untouched).
    AssetDelete,
    /// Update an asset's capability-bearing metadata (P2.03 use case).
    AssetUpdate,
    /// Deactivate an asset (terminal lifecycle transition, P2.03).
    AssetDeactivate,
    /// Read stored telemetry (time-range query, current state; P2.12/P2.18).
    TelemetryRead,
    /// Read the ingestion quarantine (P2.07; admin role by grant design).
    QuarantineRead,

    // --- Phase 3 additions (P3.28; closed-enum extension rule: a variant,
    // its `id` arm, its `parse` arm, and explicit policy grants — nothing
    // matches by default; evaluated at tenant scope, no role bypasses the
    // tenant match) ----------------------------------------------------------
    /// Read persisted portfolio flexibility-envelope records
    /// (`GET /api/v1/flexibility/envelopes`).
    FlexibilityEnvelopeRead,
    /// Create (register) a persisted portfolio flexibility-envelope record
    /// with its `FlexibilityEnvelopeUpdated` outbox events
    /// (`POST /api/v1/flexibility/envelopes`).
    FlexibilityEnvelopeCreate,
    /// Read persisted schedule revisions and chains
    /// (`GET /api/v1/schedules`).
    ScheduleRead,
    /// Propose a candidate schedule revision with its `ScheduleProposed`
    /// and (asset/time-coupled validation) `ScheduleValidated` outbox
    /// events (`POST /api/v1/schedules`). NEVER an approval, dispatch, or
    /// execute action — no such actions exist in Phase 3.
    SchedulePropose,
    /// Read persisted §12.6 optimization-run (schedule-result) records
    /// (`GET /api/v1/optimization-runs`).
    OptimizationRunRead,
    /// Create (register) a §12.6 optimization-run record with its
    /// `ScheduleProposed` outbox event
    /// (`POST /api/v1/optimization-runs`).
    OptimizationRunCreate,

    // --- Phase 4 additions (P4.13; closed-enum extension rule: a variant,
    // its `id` arm, its `parse` arm, and explicit policy grants — nothing
    // matches by default; evaluated at tenant/site/asset scope, no role
    // bypasses the tenant match; the recorded Phase 4 grants live in
    // `crate::phase4_policies`) ----------------------------------------------
    /// Create a PROPOSED dispatch command from an approved schedule
    /// revision (the P4.11 creation gate).
    CommandCreate,
    /// Run the validation gate on a PROPOSED command (the P4.11 validation
    /// gate). NEVER a feasibility or authorization action — the three gates
    /// are distinct (§14.1).
    CommandValidate,
    /// Grant the human/policy authorization for a FEASIBLE command (the
    /// P4.13 authorization gate). This is the ONLY approval action on the
    /// surface; there is no auto-approve action and no action that skips
    /// the FEASIBLE precondition. `dispatch.authorize` / `dispatch.issue`
    /// remain excluded (§5.7: never granted to general plugins; the Phase 4
    /// protected kernel; see the module header).
    CommandApprove,
    /// Read a dispatch command record and its transition journal.
    CommandRead,
}

impl Action {
    /// The canonical lowercase dotted id of this action (the form used on
    /// the Phase 1 API surface and in audit records).
    pub fn id(self) -> &'static str {
        match self {
            Action::TenantCreate => "tenant.create",
            Action::TenantRead => "tenant.read",
            Action::PortfolioCreate => "portfolio.create",
            Action::SiteCreate => "site.create",
            Action::AssetCreate => "asset.create",
            Action::AssetRead => "asset.read",
            Action::PortfolioRead => "portfolio.read",
            Action::SiteRead => "site.read",
            Action::TenantDelete => "tenant.delete",
            Action::PortfolioDelete => "portfolio.delete",
            Action::SiteDelete => "site.delete",
            Action::AssetDelete => "asset.delete",
            Action::AssetUpdate => "asset.update",
            Action::AssetDeactivate => "asset.deactivate",
            Action::TelemetryRead => "telemetry.read",
            Action::QuarantineRead => "quarantine.read",
            Action::FlexibilityEnvelopeRead => "flexibility.envelope.read",
            Action::FlexibilityEnvelopeCreate => "flexibility.envelope.create",
            Action::ScheduleRead => "schedule.read",
            Action::SchedulePropose => "schedule.propose",
            Action::OptimizationRunRead => "optimization_run.read",
            Action::OptimizationRunCreate => "optimization_run.create",
            Action::CommandCreate => "command.create",
            Action::CommandValidate => "command.validate",
            Action::CommandApprove => "command.approve",
            Action::CommandRead => "command.read",
        }
    }

    /// Parses a canonical action id.
    ///
    /// Exact, case-sensitive match on the lowercase dotted form; anything
    /// else is `None` and must be denied (`UnknownAction`), never defaulted.
    pub fn parse(id: &str) -> Option<Self> {
        match id {
            "tenant.create" => Some(Action::TenantCreate),
            "tenant.read" => Some(Action::TenantRead),
            "portfolio.create" => Some(Action::PortfolioCreate),
            "site.create" => Some(Action::SiteCreate),
            "asset.create" => Some(Action::AssetCreate),
            "asset.read" => Some(Action::AssetRead),
            "portfolio.read" => Some(Action::PortfolioRead),
            "site.read" => Some(Action::SiteRead),
            "tenant.delete" => Some(Action::TenantDelete),
            "portfolio.delete" => Some(Action::PortfolioDelete),
            "site.delete" => Some(Action::SiteDelete),
            "asset.delete" => Some(Action::AssetDelete),
            "asset.update" => Some(Action::AssetUpdate),
            "asset.deactivate" => Some(Action::AssetDeactivate),
            "telemetry.read" => Some(Action::TelemetryRead),
            "quarantine.read" => Some(Action::QuarantineRead),
            "flexibility.envelope.read" => Some(Action::FlexibilityEnvelopeRead),
            "flexibility.envelope.create" => Some(Action::FlexibilityEnvelopeCreate),
            "schedule.read" => Some(Action::ScheduleRead),
            "schedule.propose" => Some(Action::SchedulePropose),
            "optimization_run.read" => Some(Action::OptimizationRunRead),
            "optimization_run.create" => Some(Action::OptimizationRunCreate),
            "command.create" => Some(Action::CommandCreate),
            "command.validate" => Some(Action::CommandValidate),
            "command.approve" => Some(Action::CommandApprove),
            "command.read" => Some(Action::CommandRead),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_canonical_and_parse_round_trips() {
        for action in [
            Action::TenantCreate,
            Action::PortfolioCreate,
            Action::SiteCreate,
            Action::AssetCreate,
            Action::AssetRead,
            Action::PortfolioRead,
            Action::SiteRead,
            // Phase 2 additions (P2.12).
            Action::TenantDelete,
            Action::PortfolioDelete,
            Action::SiteDelete,
            Action::AssetDelete,
            Action::AssetUpdate,
            Action::AssetDeactivate,
            Action::TelemetryRead,
            Action::QuarantineRead,
            // Phase 3 additions (P3.28).
            Action::FlexibilityEnvelopeRead,
            Action::FlexibilityEnvelopeCreate,
            Action::ScheduleRead,
            Action::SchedulePropose,
            Action::OptimizationRunRead,
            Action::OptimizationRunCreate,
            // Phase 4 additions (P4.13).
            Action::CommandCreate,
            Action::CommandValidate,
            Action::CommandApprove,
            Action::CommandRead,
        ] {
            assert_eq!(Action::parse(action.id()), Some(action));
        }
    }

    #[test]
    fn parse_rejects_unknown_actions_deny_by_default() {
        // Not on the surface at all.
        assert_eq!(Action::parse("dispatch.authorize"), None);
        assert_eq!(Action::parse("dispatch.issue"), None);
        assert_eq!(Action::parse("secret.read"), None);
        assert_eq!(Action::parse("network.any"), None);
        // Near-misses: wrong case, wrong separator, unknown verb/resource.
        assert_eq!(Action::parse("Asset.Read"), None);
        assert_eq!(Action::parse("asset_read"), None);
        assert_eq!(Action::parse("asset.del"), None);
        assert_eq!(Action::parse("asset.purge"), None);
        assert_eq!(Action::parse("telemetry.write"), None);
        assert_eq!(Action::parse(""), None);
        // The §5.7 capability names that must never enter this surface parse
        // to nothing, so they can only ever produce Deny(UnknownAction).
        assert_eq!(Action::parse("command.issue"), None);
        assert_eq!(Action::parse("command.auto_approve"), None);
        assert_eq!(Action::parse("commands.approve"), None);
    }
}
