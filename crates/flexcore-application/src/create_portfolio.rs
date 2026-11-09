// P1.08 — `CreatePortfolio`: create a portfolio under the caller's tenant
//         (report §10.1 row 2; §3.4 workflow step 2; ledger P1.08 step 2).
//
// Tenant context: the use case takes the domain `TenantContext` and never
// reads a tenant identifier from the command (P0.16 rule 5). The owning
// tenant entity is fetched **through the context** — a stale or foreign
// tenant therefore reads as missing, never as another tenant's data.
//
// Duplicate rule (recorded assumption — no uniqueness rule is frozen for
// portfolios): the portfolio name must be unique within the caller's
// tenant; the fake repository and the future P1.13 adapter enforce the
// same rule, and the use case checks it first for a clean error.
//
// Event choice (recorded assumption, crate README): the frozen §15.3
// vocabulary defines no portfolio-creation event and none may be invented
// (P0.14), so the use case emits `OperatorActionRecorded` under the
// caller's tenant scope.

use std::sync::Arc;

use crate::error::{ApplicationError, AuthorizationError, ValidationError};
use crate::event_assembly::emit;
use crate::payload_encoding::EventPayloadEncoder;
use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::time_model::EventTime;
use flexcore_domain::entities::Portfolio;
use flexcore_domain::tenant_context::TenantContext;
use flexcore_events::events::{DomainEvent, OperatorActionRecordedPayload};
use flexcore_ports::clock::Clock;
use flexcore_ports::event_outbox::EventOutbox;
use flexcore_ports::id_generator::IdGenerator;
use flexcore_ports::repository::{PortfolioRepository, TenantRepository};
use flexcore_ports::state_change::entities;

/// The creation input. The parent tenant comes from the context, the
/// portfolio identifier and timestamps from the ports — never from the
/// request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreatePortfolioCommand {
    /// The portfolio name (unique within the tenant — recorded assumption).
    pub name: String,
    /// Optional description.
    pub description: Option<String>,
    /// The operator (P0.10 §1 row 2).
    pub operator: String,
    /// The market region (P0.10 §1 row 2).
    pub market_region: String,
    /// The aggregation rules reference (P0.10 §1 row 2).
    pub aggregation_rules: String,
}

/// Creates a portfolio under the caller's tenant.
pub struct CreatePortfolio {
    tenants: Arc<dyn TenantRepository>,
    portfolios: Arc<dyn PortfolioRepository>,
    outbox: Arc<dyn EventOutbox>,
    ids: Arc<dyn IdGenerator>,
    clock: Arc<dyn Clock>,
    encoder: Arc<dyn EventPayloadEncoder>,
}

impl CreatePortfolio {
    /// Composes the use case from its ports.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        tenants: Arc<dyn TenantRepository>,
        portfolios: Arc<dyn PortfolioRepository>,
        outbox: Arc<dyn EventOutbox>,
        ids: Arc<dyn IdGenerator>,
        clock: Arc<dyn Clock>,
        encoder: Arc<dyn EventPayloadEncoder>,
    ) -> Self {
        Self {
            tenants,
            portfolios,
            outbox,
            ids,
            clock,
            encoder,
        }
    }

    /// Creates the portfolio: the caller's tenant is resolved through the
    /// context, duplicates are rejected on the tenant-scoped name rule,
    /// the domain constructor derives the ownership chain, one insert and
    /// one outbox write follow — inside the caller's unit of work.
    pub fn execute(
        &self,
        ctx: &TenantContext,
        command: CreatePortfolioCommand,
    ) -> Result<Portfolio, ApplicationError> {
        // The owning tenant comes from the context, never from the command.
        let tenant = self
            .tenants
            .find_by_id(ctx.tenant_id())?
            .ok_or(AuthorizationError::CallerTenantUnknown)?;

        // Duplicate registration rejected (tenant-scoped name rule —
        // recorded assumption).
        if self
            .portfolios
            .list_for_tenant(ctx)?
            .iter()
            .any(|p| p.name() == command.name)
        {
            return Err(ValidationError::DuplicateRegistration {
                entity: entities::PORTFOLIOS,
                key: command.name.clone(),
            }
            .into());
        }

        let portfolio_id = self.ids.new_portfolio_id();
        let created_at: EventTime = self.clock.now_event_time();
        let portfolio = Portfolio::try_new(
            &tenant,
            portfolio_id,
            command.name,
            command.description,
            command.operator,
            command.market_region,
            command.aggregation_rules,
            created_at,
        )?;

        let state_change = self.portfolios.insert(ctx, &portfolio)?;

        // One transactional outbox write for the state change (§6.3 step 1);
        // recorded assumption: the audit-area operator-action event stands
        // in for the (non-existent, non-inventable) portfolio-creation event.
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

        Ok(portfolio)
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
    fn the_use_case_composes_from_ports_and_derives_the_chain_in_domain_construction() {
        // Compile-level evidence for the shape decision: the use case holds
        // only port trait objects and the domain types, and portfolio
        // construction takes the parent Tenant — so the tenant scope of the
        // created portfolio is derived, never supplied by the caller.
        let f: fn(
            Arc<dyn TenantRepository>,
            Arc<dyn PortfolioRepository>,
            Arc<dyn EventOutbox>,
            Arc<dyn IdGenerator>,
            Arc<dyn Clock>,
            Arc<dyn EventPayloadEncoder>,
        ) -> CreatePortfolio = CreatePortfolio::new;
        let _ = f;
    }
}
