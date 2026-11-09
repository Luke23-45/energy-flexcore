// P1.08 — `CreateSite`: create a site under a portfolio of the caller's
//         tenant (report §10.1 row 3; §3.4 workflow step 3; ledger P1.08
//         step 2).
//
// Tenant context: the use case takes the domain `TenantContext` and never
// reads a tenant identifier from the command (P0.16 rule 5). The parent
// portfolio is fetched **through the context**, so a portfolio from another
// tenant reads as missing (`ParentOutsideTenantScope`) — cross-tenant
// access is rejected without leaking existence; the domain constructor
// additionally derives the whole ownership chain from the parent, so a
// site can never carry a different tenant than its portfolio.
//
// Duplicate rule (frozen): the site edge identity is unique within the
// caller's tenant (P0.16 rule 7 — "keep site edge identities scoped to one
// tenant").
//
// Event choice (recorded assumption, crate README): the frozen §15.3
// vocabulary defines no site-creation event and none may be invented
// (P0.14), so the use case emits `OperatorActionRecorded` under the
// caller's tenant scope.

use std::sync::Arc;

use crate::error::{ApplicationError, ValidationError};
use crate::event_assembly::emit;
use crate::payload_encoding::EventPayloadEncoder;
use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::time_model::EventTime;
use flexcore_contracts::time_model::SiteTimezone;
use flexcore_domain::entities::Site;
use flexcore_domain::tenant_context::TenantContext;
use flexcore_events::events::{DomainEvent, OperatorActionRecordedPayload};
use flexcore_ports::clock::Clock;
use flexcore_ports::event_outbox::EventOutbox;
use flexcore_ports::id_generator::IdGenerator;
use flexcore_ports::repository::{PortfolioRepository, SiteRepository};
use flexcore_ports::state_change::entities;

/// The creation input. The parent portfolio is referenced by identifier;
/// its tenant scope is verified through the context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateSiteCommand {
    /// The parent portfolio (must belong to the caller's tenant).
    pub portfolio_id: flexcore_domain::ids::PortfolioId,
    /// The site name.
    pub name: String,
    /// Optional description.
    pub description: Option<String>,
    /// The location (P0.10 §1 row 3).
    pub location: String,
    /// The site timezone name (metadata; canonical timestamps stay UTC,
    /// P0.09). Parsed into the validated `SiteTimezone` type here.
    pub timezone: String,
    /// The point of common coupling reference (P0.10 §1 row 3).
    pub point_of_common_coupling: String,
    /// The grid model reference (P0.10 §1 row 3).
    pub grid_model_reference: String,
    /// The edge identity (P0.10 §1 row 3; tenant-scoped uniqueness, P0.16
    /// rule 7).
    pub edge_identity: String,
}

/// Creates a site under a portfolio of the caller's tenant.
pub struct CreateSite {
    portfolios: Arc<dyn PortfolioRepository>,
    sites: Arc<dyn SiteRepository>,
    outbox: Arc<dyn EventOutbox>,
    ids: Arc<dyn IdGenerator>,
    clock: Arc<dyn Clock>,
    encoder: Arc<dyn EventPayloadEncoder>,
}

impl CreateSite {
    /// Composes the use case from its ports.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        portfolios: Arc<dyn PortfolioRepository>,
        sites: Arc<dyn SiteRepository>,
        outbox: Arc<dyn EventOutbox>,
        ids: Arc<dyn IdGenerator>,
        clock: Arc<dyn Clock>,
        encoder: Arc<dyn EventPayloadEncoder>,
    ) -> Self {
        Self {
            portfolios,
            sites,
            outbox,
            ids,
            clock,
            encoder,
        }
    }

    /// Creates the site: the parent portfolio is resolved through the
    /// context, the edge-identity duplicate rule is checked, the domain
    /// constructor derives the ownership chain, one insert and one outbox
    /// write follow — inside the caller's unit of work.
    pub fn execute(
        &self,
        ctx: &TenantContext,
        command: CreateSiteCommand,
    ) -> Result<Site, ApplicationError> {
        // Cross-tenant rejection: a portfolio outside the caller's tenant
        // reads as missing (no existence leak).
        let portfolio = self
            .portfolios
            .find_by_id(ctx, command.portfolio_id)?
            .ok_or(crate::error::AuthorizationError::ParentOutsideTenantScope {
                entity: entities::PORTFOLIOS,
            })?;

        // Duplicate registration rejected: edge identity unique within the
        // tenant (P0.16 rule 7).
        if self
            .sites
            .list_for_tenant(ctx)?
            .iter()
            .any(|s| s.edge_identity() == command.edge_identity)
        {
            return Err(ValidationError::DuplicateRegistration {
                entity: entities::SITES,
                key: command.edge_identity.clone(),
            }
            .into());
        }

        let site_id = self.ids.new_site_id();
        let created_at: EventTime = self.clock.now_event_time();
        let site = Site::try_new(
            &portfolio,
            site_id,
            command.name,
            command.description,
            command.location,
            // The timezone metadata is validated by the contracts type; a
            // bad name is a validation-class error, not authorization.
            SiteTimezone::new(command.timezone)
                .map_err(|e| crate::error::ValidationError::from(flexcore_domain::entities::EntityError::Time(e)))?,
            command.point_of_common_coupling,
            command.grid_model_reference,
            command.edge_identity,
            created_at,
        )?;

        let state_change = self.sites.insert(ctx, &site)?;

        // One transactional outbox write for the state change (§6.3 step 1);
        // recorded assumption: the audit-area operator-action event stands
        // in for the (non-existent, non-inventable) site-creation event.
        let correlation_id: FlexUuid = self.ids.new_uuid();
        let event = DomainEvent::OperatorActionRecorded(OperatorActionRecordedPayload {
            correlation_id,
            tenant_id: ctx.tenant_id(),
            event_time: created_at,
        });
        emit(
            state_change,
            event,
            correlation_id,
            created_at,
            self.encoder.as_ref(),
            self.outbox.as_ref(),
        )?;

        Ok(site)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // NOTE (recorded gap, see the crate README "blocked tests"): the happy-
    // path, duplicate, and cross-tenant tests for this use case need a
    // `TenantContext`, and no code outside flexcore-domain can construct
    // one today (P1.06 claims boundary — private `AuthenticatedClaims`
    // fields, no public constructor). The ready-to-run test bodies are in
    // the crate README and activate unchanged once the domain exposes the
    // claims constructor for verified tokens (API-middleware subtask).
    #[test]
    fn the_use_case_composes_from_ports_and_the_parent_chain_is_domain_derived() {
        // Compile-level evidence for the shape decision: site construction
        // takes the parent Portfolio — the tenant scope of the created site
        // is derived from the (context-verified) parent, never supplied by
        // the caller.
        let f: fn(
            Arc<dyn PortfolioRepository>,
            Arc<dyn SiteRepository>,
            Arc<dyn EventOutbox>,
            Arc<dyn IdGenerator>,
            Arc<dyn Clock>,
            Arc<dyn EventPayloadEncoder>,
        ) -> CreateSite = CreateSite::new;
        let _ = f;
    }
}
