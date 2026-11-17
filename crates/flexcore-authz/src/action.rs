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
    /// Read a persisted §12.5 feasibility-check evidence record (the
    /// `/api/v1/feasibility-checks` read resource; P4.30 — read-only; no
    /// execute or check-running action exists on this surface, §15.2).
    FeasibilityRead,
    /// Read audit evidence records (the `/api/v1/audit-events` read
    /// resource; P4.36 — read-only; there is NO update, delete, or
    /// export/dump action on this surface: the trail is append-only,
    /// §15.2, §16.2; writes stay the P1.10 `AuditSink`'s append alone).
    AuditEventRead,

    // --- Phase 5 addition (P5.31; closed-enum extension rule: a variant,
    // its `id` arm, its `parse` arm, and explicit policy grants — nothing
    // matches by default; evaluated at tenant scope, no role bypasses the
    // tenant match; the recorded Phase 5 grants live in the control plane's
    // `phase5_default_policies`) ----------------------------------------------
    /// Read the derived alarm records (the `/api/v1/alarms` read resource;
    /// P5.31 — READ-only; alarms are derived-on-read operator-attention
    /// records, tenant-scoped like every record they derive from; there is
    /// NO alarm acknowledge, create, or delete action on this surface).
    AlarmRead,

    // --- Phase 13 additions (P13.08/P13.05; closed-enum extension rule: a
    // variant, its `id` arm, its `parse` arm, and explicit policy grants —
    // nothing matches by default; the recorded Phase 13 grants live in
    // `crate::phase13_policies`. Evaluated at TENANT scope only: API keys
    // are tenant-scoped credentials, and no tenant-level approval grant is
    // implied by any of these — `command.approve`'s recorded grant set is
    // untouched) --------------------------------------------------------------
    /// Create (issue) a per-tenant API key (P13.08). Tenant scope.
    ApiKeyCreate,
    /// Rotate (re-issue) a per-tenant API key (P13.08: the old secret is
    /// superseded with a recorded overlap window). Tenant scope.
    ApiKeyRotate,
    /// Revoke a per-tenant API key — immediate (P13.08). Tenant scope.
    ApiKeyRevoke,
    /// Read API-key metadata — key id, scopes, state, rotation and
    /// last-used evidence; NEVER the secret (P13.08: the secret is shown
    /// once at issuance/rotation and is not stored retrievable). Tenant
    /// scope.
    ApiKeyRead,

    // --- Phase 16 additions (P16.19/P16.22; closed-enum extension rule: a
    // variant, its `id` arm, its `parse` arm, and explicit policy grants —
    // nothing matches by default; the recorded Phase 16 market grants live
    // in `crate::phase16_market_policies`. Evaluated at TENANT scope only:
    // market participation is a per-tenant, jurisdiction-named posture, and
    // the `market.*` set authorizes RECORD and GRANT-ADMINISTRATION work —
    // it NEVER touches dispatch. The P14.16 command-relay-only rule is
    // consumed by citation (see `phase16_market_policies`): a market
    // adapter reaches dispatch ONLY through the command lifecycle, so no
    // `market.dispatch.*` action exists on this surface and none may be
    // added. The per-market bid grant is the P16.06 `MarketBidGrant` RECORD
    // (flexcore-application::bid_composition), checked by the composer —
    // these actions authorize grant ADMINISTRATION, they never replace
    // that record-level check) ---------------------------------------------
    /// Read ingested market price records (P16.07 record layer). Tenant
    /// scope. A price record is ingested data — never a command (ADR-010).
    MarketPriceRead,
    /// Compose an aggregation-level bid RECORD from asset envelopes (the
    /// P16.06 composition service). Tenant scope. Composing is a RECORD
    /// action — no bid is submitted and none can be (`NO_BID_SUBMITTED_
    /// STATEMENT`); submission would additionally require a jurisdiction
    /// adapter (ADR-011) and a named counterparty (D1, OPEN). The
    /// composer separately requires operator-approved mode plus an ACTIVE
    /// `MarketBidGrant` (P16.06).
    MarketBidCompose,
    /// Issue an explicit per-market bid grant — a NEW `MarketBidGrant`
    /// record (P16.06's grant model). Tenant scope. Grant administration
    /// is an administrative action (PlatformAdmin/TenantAdmin by the
    /// recorded default grants).
    MarketGrantIssue,
    /// Revoke an explicit per-market bid grant (record the REVOKED state
    /// — the supersede model; the old grant row is retained). Tenant
    /// scope.
    MarketGrantRevoke,
    /// Read per-market grant records (state, validity, market-structure
    /// coverage). Tenant scope.
    MarketGrantRead,
    /// Read ingested settlement statement records and reconciliation
    /// reports (P16.09/P16.12 record layer). Tenant scope. Reading
    /// settlement evidence never adjusts, settles, or writes back — the
    /// engine's only output is dispute evidence.
    MarketSettlementRead,
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
            Action::FeasibilityRead => "feasibility.read",
            Action::AuditEventRead => "audit.event.read",
            Action::AlarmRead => "alarm.read",
            Action::ApiKeyCreate => "apikey.create",
            Action::ApiKeyRotate => "apikey.rotate",
            Action::ApiKeyRevoke => "apikey.revoke",
            Action::ApiKeyRead => "apikey.read",
            Action::MarketPriceRead => "market.price.read",
            Action::MarketBidCompose => "market.bid.compose",
            Action::MarketGrantIssue => "market.grant.issue",
            Action::MarketGrantRevoke => "market.grant.revoke",
            Action::MarketGrantRead => "market.grant.read",
            Action::MarketSettlementRead => "market.settlement.read",
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
            "feasibility.read" => Some(Action::FeasibilityRead),
            "audit.event.read" => Some(Action::AuditEventRead),
            "alarm.read" => Some(Action::AlarmRead),
            "apikey.create" => Some(Action::ApiKeyCreate),
            "apikey.rotate" => Some(Action::ApiKeyRotate),
            "apikey.revoke" => Some(Action::ApiKeyRevoke),
            "apikey.read" => Some(Action::ApiKeyRead),
            "market.price.read" => Some(Action::MarketPriceRead),
            "market.bid.compose" => Some(Action::MarketBidCompose),
            "market.grant.issue" => Some(Action::MarketGrantIssue),
            "market.grant.revoke" => Some(Action::MarketGrantRevoke),
            "market.grant.read" => Some(Action::MarketGrantRead),
            "market.settlement.read" => Some(Action::MarketSettlementRead),
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
            // P4.30 addition (the /feasibility-checks read resource).
            Action::FeasibilityRead,
            // P4.36 addition (the /audit-events read resource).
            Action::AuditEventRead,
            // P5.31 addition (the /alarms read resource).
            Action::AlarmRead,
            // P13.08 additions (the API-key administration surface; tenant
            // scope only; the secret is never readable).
            Action::ApiKeyCreate,
            Action::ApiKeyRotate,
            Action::ApiKeyRevoke,
            Action::ApiKeyRead,
            // P16.19 additions (the market surface; tenant scope only; the
            // record + grant-administration actions — no dispatch, no
            // submission).
            Action::MarketPriceRead,
            Action::MarketBidCompose,
            Action::MarketGrantIssue,
            Action::MarketGrantRevoke,
            Action::MarketGrantRead,
            Action::MarketSettlementRead,
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
        // P5.31: the alarm surface is READ-only — no write/acknowledge
        // action exists or parses.
        assert_eq!(Action::parse("alarm.write"), None);
        assert_eq!(Action::parse("alarm.acknowledge"), None);
        assert_eq!(Action::parse(""), None);
        // P13.08: the API-key surface has no secret-read action (the
        // never-granted `secret.read` name stays off the surface — the
        // apikey.* set administers keys, it never reads key material) and
        // no approval action (approvals are `command.approve`'s alone).
        assert_eq!(Action::parse("apikey.secret"), None);
        assert_eq!(Action::parse("apikey.approve"), None);
        assert_eq!(Action::parse("apikey.issue"), None);
        // The §5.7 capability names that must never enter this surface parse
        // to nothing, so they can only ever produce Deny(UnknownAction).
        assert_eq!(Action::parse("command.issue"), None);
        assert_eq!(Action::parse("command.auto_approve"), None);
        assert_eq!(Action::parse("commands.approve"), None);
        // P16.19: the market surface is RECORD + GRANT-ADMINISTRATION only.
        // There is no bid-submission action (no bid is submitted — none can
        // be), no market dispatch action (the P14.16 command-relay-only
        // rule: a market adapter reaches dispatch ONLY through the command
        // lifecycle), and no market approval action (approvals are
        // `command.approve`'s alone).
        assert_eq!(Action::parse("market.bid.submit"), None);
        assert_eq!(Action::parse("market.dispatch.relay"), None);
        assert_eq!(Action::parse("market.dispatch.authorize"), None);
        assert_eq!(Action::parse("market.bid.approve"), None);
        assert_eq!(Action::parse("market.settlement.write"), None);
        assert_eq!(Action::parse("market.settlement.adjust"), None);
    }
}
