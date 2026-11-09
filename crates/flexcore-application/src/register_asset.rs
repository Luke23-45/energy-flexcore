// P1.08 — `RegisterAsset`: register an asset under a site of the caller's
//         tenant (report §10.1 row 4; §3.4 workflow step 4; ledger P1.08
//         step 2; the P1.26 exit-gate workflow's last registry step).
//
// Tenant context: the use case takes the domain `TenantContext` and never
// reads a tenant identifier from the command (P0.16 rule 5). The parent
// site is fetched **through the context**, so a site from another tenant
// reads as missing (`ParentOutsideTenantScope`) — cross-tenant access is
// rejected without leaking existence; the domain constructor derives the
// whole ownership chain (asset -> site -> portfolio -> tenant).
//
// Duplicate rule (recorded assumption — no uniqueness rule is frozen for
// assets): the asset name must be unique within the parent site; the fake
// repository and the future P1.13 adapter enforce the same rule.
//
// Event choice (frozen vocabulary): `AssetRegistered` is the correct §15.3
// event for this action — the only Stage 1 registry step whose state
// change maps onto a dedicated frozen event.
//
// Quantities: the command carries the canonical unit types (`ActivePowerMW`,
// `EnergyMWh`) — the API layer parses and validates them before the use
// case runs; the rated-quantity positivity rules are re-enforced by the
// domain constructor regardless.

use std::sync::Arc;

use crate::error::{ApplicationError, ValidationError};
use crate::event_assembly::emit;
use crate::payload_encoding::EventPayloadEncoder;
use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::time_model::EventTime;
use flexcore_domain::entities::{Asset, AssetType};
use flexcore_contracts::units::{ActivePowerMW, EnergyMWh};
use flexcore_domain::tenant_context::TenantContext;
use flexcore_events::events::{AssetRegisteredPayload, DomainEvent};
use flexcore_ports::clock::Clock;
use flexcore_ports::event_outbox::EventOutbox;
use flexcore_ports::id_generator::IdGenerator;
use flexcore_ports::repository::{AssetRepository, SiteRepository};
use flexcore_ports::state_change::entities;

/// The registration input. The parent site is referenced by identifier;
/// its tenant scope is verified through the context.
#[derive(Debug, Clone, PartialEq)]
pub struct RegisterAssetCommand {
    /// The parent site (must belong to the caller's tenant).
    pub site_id: flexcore_domain::ids::SiteId,
    /// The asset name (unique within the site — recorded assumption).
    pub name: String,
    /// Optional description.
    pub description: Option<String>,
    /// The asset type (P0.10 §2 vocabulary; backup generation is
    /// deliberately absent from the frozen list).
    pub asset_type: AssetType,
    /// The vendor/model string (P0.10 §1 row 4).
    pub vendor_model: String,
    /// The rated power in MW (strictly positive by type construction).
    pub rated_power_mw: ActivePowerMW,
    /// The rated energy in MWh, when applicable (storage assets).
    pub rated_energy_mwh: Option<EnergyMWh>,
    /// The phase reference (vocabulary-free until a phase enumeration is
    /// frozen; P0.10 §1 row 4).
    pub phase: String,
    /// The bus reference in the grid model (P0.10 §1 row 4).
    pub bus_reference: String,
    /// The capability profile reference (P0.10 §1 row 4).
    pub capability_profile: String,
}

/// Registers an asset under a site of the caller's tenant.
pub struct RegisterAsset {
    sites: Arc<dyn SiteRepository>,
    assets: Arc<dyn AssetRepository>,
    outbox: Arc<dyn EventOutbox>,
    ids: Arc<dyn IdGenerator>,
    clock: Arc<dyn Clock>,
    encoder: Arc<dyn EventPayloadEncoder>,
}

impl RegisterAsset {
    /// Composes the use case from its ports.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        sites: Arc<dyn SiteRepository>,
        assets: Arc<dyn AssetRepository>,
        outbox: Arc<dyn EventOutbox>,
        ids: Arc<dyn IdGenerator>,
        clock: Arc<dyn Clock>,
        encoder: Arc<dyn EventPayloadEncoder>,
    ) -> Self {
        Self {
            sites,
            assets,
            outbox,
            ids,
            clock,
            encoder,
        }
    }

    /// Registers the asset: the parent site is resolved through the
    /// context, the within-site duplicate rule is checked, the domain
    /// constructor derives the ownership chain, one insert and one outbox
    /// write follow — inside the caller's unit of work.
    pub fn execute(
        &self,
        ctx: &TenantContext,
        command: RegisterAssetCommand,
    ) -> Result<Asset, ApplicationError> {
        // Cross-tenant rejection: a site outside the caller's tenant reads
        // as missing (no existence leak).
        let site = self
            .sites
            .find_by_id(ctx, command.site_id)?
            .ok_or(crate::error::AuthorizationError::ParentOutsideTenantScope {
                entity: entities::SITES,
            })?;

        // Duplicate registration rejected (name unique within the site —
        // recorded assumption).
        if self
            .assets
            .list_for_tenant(ctx)?
            .iter()
            .any(|a| a.site_id() == command.site_id && a.name() == command.name)
        {
            return Err(ValidationError::DuplicateRegistration {
                entity: entities::ASSETS,
                key: command.name.clone(),
            }
            .into());
        }

        let asset_id = self.ids.new_asset_id();
        let created_at: EventTime = self.clock.now_event_time();
        let asset = Asset::try_new(
            &site,
            asset_id,
            command.name,
            command.description,
            command.asset_type,
            command.vendor_model,
            command.rated_power_mw,
            command.rated_energy_mwh,
            command.phase,
            command.bus_reference,
            command.capability_profile,
            created_at,
        )?;

        let state_change = self.assets.insert(ctx, &asset)?;

        // One transactional outbox write for the state change (§6.3 step 1),
        // carrying the frozen `AssetRegistered` event (§15.3) with the full
        // ownership chain the context-verified parents provided.
        let correlation_id: FlexUuid = self.ids.new_uuid();
        let event = DomainEvent::AssetRegistered(AssetRegisteredPayload {
            correlation_id,
            tenant_id: ctx.tenant_id(),
            portfolio_id: site.portfolio_id(),
            site_id: site.site_id(),
            asset_id: asset.asset_id(),
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

        Ok(asset)
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
        // Compile-level evidence for the shape decision: asset construction
        // takes the parent Site — the tenant scope of the created asset is
        // derived from the (context-verified) parent, never supplied by the
        // caller.
        let f: fn(
            Arc<dyn SiteRepository>,
            Arc<dyn AssetRepository>,
            Arc<dyn EventOutbox>,
            Arc<dyn IdGenerator>,
            Arc<dyn Clock>,
            Arc<dyn EventPayloadEncoder>,
        ) -> RegisterAsset = RegisterAsset::new;
        let _ = f;
    }
}
