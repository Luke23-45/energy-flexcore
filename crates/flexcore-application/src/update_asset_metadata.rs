// P2.03 — `UpdateAssetMetadata`: update an asset's capability-bearing
//         metadata (vendor/model, rated power/energy, capability-profile
//         reference) and emit the frozen §15.3 `AssetCapabilityUpdated`
//         event (ledger P2.03 step 2–3).
//
// Tenant context: the use case takes the domain `TenantContext` and never
// reads a tenant identifier from the command (P0.16 rule 5). The asset and
// its parent site are fetched **through the context**, so an asset (or site)
// from another tenant reads as missing (`ParentOutsideTenantScope`) —
// cross-tenant access is rejected without leaking existence.
//
// What may change (P2.02): vendor/model, rated power/energy, and the
// capability-profile **reference** — the profile's contents are Phase 3
// (P0.10 §5); Phase 2 stores and re-points the reference only. The repoints
// are revalidated by the domain (`Asset::update_capability_metadata`):
// positive rated magnitudes in canonical MW/MWh (P0.07), non-empty
// references. Identity, ownership chain, lifecycle, and creation time are
// not touchable through this use case — an asset can never be moved between
// sites or tenants by a metadata update.
//
// Event choice (frozen vocabulary): `AssetCapabilityUpdated` is the §15.3
// event for this action, emitted via the transactional outbox exactly like
// every other registry state change (§6.3 step 1) — never a direct NATS
// publish from this use case.

use std::sync::Arc;

use crate::error::{ApplicationError, AuthorizationError};
use crate::event_assembly::emit;
use crate::payload_encoding::EventPayloadEncoder;
use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::time_model::EventTime;
use flexcore_contracts::units::{ActivePowerMW, EnergyMWh};
use flexcore_domain::entities::Asset;
use flexcore_domain::ids::AssetId;
use flexcore_domain::tenant_context::TenantContext;
use flexcore_events::events::{AssetCapabilityUpdatedPayload, DomainEvent};
use flexcore_ports::clock::Clock;
use flexcore_ports::event_outbox::EventOutbox;
use flexcore_ports::id_generator::IdGenerator;
use flexcore_ports::repository::{AssetRepository, SiteRepository};
use flexcore_ports::state_change::entities;

/// The metadata-update input. The asset is referenced by identifier; its
/// tenant scope is verified through the context.
#[derive(Debug, Clone, PartialEq)]
pub struct UpdateAssetMetadataCommand {
    /// The asset to update (must belong to the caller's tenant).
    pub asset_id: AssetId,
    /// The new vendor/model string (P0.10 §1 row 4).
    pub vendor_model: String,
    /// The new rated power in MW (strictly positive by type construction).
    pub rated_power_mw: ActivePowerMW,
    /// The new rated energy in MWh, when applicable (storage assets).
    pub rated_energy_mwh: Option<EnergyMWh>,
    /// The new capability-profile **reference** (P0.10 §1 row 4; contents
    /// are Phase 3, P0.10 §5).
    pub capability_profile: String,
}

/// Updates an asset's capability-bearing metadata within the caller's
/// tenant.
pub struct UpdateAssetMetadata {
    assets: Arc<dyn AssetRepository>,
    sites: Arc<dyn SiteRepository>,
    outbox: Arc<dyn EventOutbox>,
    ids: Arc<dyn IdGenerator>,
    clock: Arc<dyn Clock>,
    encoder: Arc<dyn EventPayloadEncoder>,
}

impl UpdateAssetMetadata {
    /// Composes the use case from its ports.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        assets: Arc<dyn AssetRepository>,
        sites: Arc<dyn SiteRepository>,
        outbox: Arc<dyn EventOutbox>,
        ids: Arc<dyn IdGenerator>,
        clock: Arc<dyn Clock>,
        encoder: Arc<dyn EventPayloadEncoder>,
    ) -> Self {
        Self {
            assets,
            sites,
            outbox,
            ids,
            clock,
            encoder,
        }
    }

    /// Updates the metadata: the asset is resolved through the context, the
    /// parent site is resolved to fill the frozen event's ownership chain,
    /// the domain revalidates and produces the new state, one update and one
    /// outbox write follow — inside the caller's unit of work.
    pub fn execute(
        &self,
        ctx: &TenantContext,
        command: UpdateAssetMetadataCommand,
    ) -> Result<Asset, ApplicationError> {
        // Cross-tenant rejection: an asset outside the caller's tenant reads
        // as missing (no existence leak).
        let asset = self
            .assets
            .find_by_id(ctx, command.asset_id)?
            .ok_or(AuthorizationError::ParentOutsideTenantScope {
                entity: entities::ASSETS,
            })?;

        // The frozen `AssetCapabilityUpdated` payload carries the full
        // ownership chain, so the parent site is resolved through the same
        // context (it must exist within the tenant — the derived chain made
        // sure the asset was registered under it).
        let site = self
            .sites
            .find_by_id(ctx, asset.site_id())?
            .ok_or(AuthorizationError::ParentOutsideTenantScope {
                entity: entities::SITES,
            })?;

        // Domain revalidation + new state (identity/ownership/lifecycle are
        // preserved by the domain method).
        let updated = asset.update_capability_metadata(
            command.vendor_model,
            command.rated_power_mw,
            command.rated_energy_mwh,
            command.capability_profile,
        )?;

        let state_change = self.assets.update(ctx, &updated)?;

        // One transactional outbox write for the state change (§6.3 step 1),
        // carrying the frozen `AssetCapabilityUpdated` event (§15.3).
        let correlation_id: FlexUuid = self.ids.new_uuid();
        let event_time: EventTime = self.clock.now_event_time();
        let event = DomainEvent::AssetCapabilityUpdated(AssetCapabilityUpdatedPayload {
            correlation_id,
            tenant_id: ctx.tenant_id(),
            portfolio_id: site.portfolio_id(),
            site_id: site.site_id(),
            asset_id: updated.asset_id(),
            event_time,
        });
        emit(
            state_change,
            event,
            correlation_id,
            event_time,
            self.encoder.as_ref(),
            self.outbox.as_ref(),
        )?;

        Ok(updated)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flexcore_domain::entities::{Portfolio, Site, Tenant};
    use flexcore_domain::ids::{PortfolioId, SiteId, TenantId};
    use flexcore_domain::tenant_context::{AuthenticatedClaims, VerifiedClaims};
    use flexcore_events::envelope::EventEnvelope;
    use flexcore_ports::fakes::{
        CountingIdGenerator, FakeAssetRepository, FakeEventOutbox, FakeSiteRepository,
        FakeWriteJournal, FakeJournalOp, FixedClock,
    };
    use flexcore_ports::state_change::StateChangeOperation;

    const UUID_TENANT_A: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a01";
    const UUID_TENANT_B: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a02";
    const UUID_PORTFOLIO: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a03";
    const UUID_SITE: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a04";
    const UUID_ASSET: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a05";

    /// Deterministic payload stub: the use cases own the envelope and the
    /// key; only the wire rendering is injected (module header of
    /// `payload_encoding`).
    struct StubEncoder;

    impl EventPayloadEncoder for StubEncoder {
        fn encode(&self, _envelope: &EventEnvelope) -> Result<String, ApplicationError> {
            Ok("{\"stub\": true}".to_string())
        }
    }

    fn uuid(text: &str) -> FlexUuid {
        FlexUuid::parse(text).unwrap()
    }

    fn tenant_ctx(uuid_text: &str) -> TenantContext {
        // The documented external construction path: VerifiedClaims (public
        // fields) -> AuthenticatedClaims::from_verified -> TenantContext.
        TenantContext::from_authenticated_claims(AuthenticatedClaims::from_verified(
            VerifiedClaims {
                tenant_id: TenantId::new(uuid(uuid_text)),
                subject: "user:operator-7".to_string(),
            },
        ))
    }

    fn chain() -> (Tenant, Portfolio, Site) {
        let tenant = Tenant::try_new(
            TenantId::new(uuid(UUID_TENANT_A)),
            "Acme Energy".to_string(),
            None,
            "idp:acme".to_string(),
            "profile:default".to_string(),
            EventTime::from_unix_millis(1_000).unwrap(),
        )
        .unwrap();
        let portfolio = Portfolio::try_new(
            &tenant,
            PortfolioId::new(uuid(UUID_PORTFOLIO)),
            "North portfolio".to_string(),
            None,
            "Acme Operations".to_string(),
            "DE-AMPRION".to_string(),
            "rules:constrained-aggregation".to_string(),
            EventTime::from_unix_millis(2_000).unwrap(),
        )
        .unwrap();
        let site = Site::try_new(
            &portfolio,
            SiteId::new(uuid(UUID_SITE)),
            "Substation 7 site".to_string(),
            None,
            "52.52N 13.40E".to_string(),
            flexcore_contracts::time_model::SiteTimezone::new("Europe/Berlin".to_string())
                .unwrap(),
            "PCC-7".to_string(),
            "grid-model:rev-42".to_string(),
            "edge-0007".to_string(),
            EventTime::from_unix_millis(3_000).unwrap(),
        )
        .unwrap();
        (tenant, portfolio, site)
    }

    fn registered_asset(site: &Site) -> Asset {
        Asset::try_new(
            site,
            AssetId::new(uuid(UUID_ASSET)),
            "BESS 1".to_string(),
            None,
            flexcore_domain::entities::AssetType::Bess,
            "vendor-x/model-1".to_string(),
            ActivePowerMW::new(2.0).unwrap(),
            Some(EnergyMWh::new(4.0).unwrap()),
            "three-phase".to_string(),
            "bus-17".to_string(),
            "capability:bess-v1".to_string(),
            EventTime::from_unix_millis(4_000).unwrap(),
        )
        .unwrap()
    }

    struct Harness {
        use_case: UpdateAssetMetadata,
        assets: Arc<dyn AssetRepository>,
        outbox: Arc<dyn EventOutbox>,
        journal: FakeWriteJournal,
        ctx: TenantContext,
    }

    fn harness() -> Harness {
        let journal = FakeWriteJournal::new();
        let assets: Arc<dyn AssetRepository> =
            Arc::new(FakeAssetRepository::new(journal.clone()));
        let sites: Arc<dyn SiteRepository> = Arc::new(FakeSiteRepository::new(journal.clone()));
        let outbox: Arc<dyn EventOutbox> = Arc::new(FakeEventOutbox::new(journal.clone()));
        let use_case = UpdateAssetMetadata::new(
            assets.clone(),
            sites,
            outbox.clone(),
            Arc::new(CountingIdGenerator::new()),
            Arc::new(FixedClock::at_unix_millis(10_000)),
            Arc::new(StubEncoder),
        );
        Harness {
            use_case,
            assets,
            outbox,
            journal,
            ctx: tenant_ctx(UUID_TENANT_A),
        }
    }

    fn command(asset_id: AssetId) -> UpdateAssetMetadataCommand {
        UpdateAssetMetadataCommand {
            asset_id,
            vendor_model: "vendor-y/model-9".to_string(),
            rated_power_mw: ActivePowerMW::new(3.5).unwrap(),
            rated_energy_mwh: Some(EnergyMWh::new(7.0).unwrap()),
            capability_profile: "capability:bess-v2".to_string(),
        }
    }

    #[test]
    fn happy_path_updates_metadata_and_emits_asset_capability_updated_via_the_outbox() {
        let h = harness();
        let (_, _, site) = chain();
        let asset = registered_asset(&site);
        h.assets
            .insert(&h.ctx, &asset)
            .expect("asset registered for the test");

        let updated = h
            .use_case
            .execute(&h.ctx, command(asset.asset_id()))
            .expect("metadata update succeeds");
        assert_eq!(updated.vendor_model(), "vendor-y/model-9");
        assert_eq!(updated.rated_power_mw().value(), 3.5);
        assert_eq!(updated.capability_profile(), "capability:bess-v2");
        assert_eq!(updated.asset_id(), asset.asset_id());
        // Stored state follows.
        assert_eq!(
            h.assets
                .find_by_id(&h.ctx, asset.asset_id())
                .expect("find succeeds")
                .expect("row exists"),
            updated
        );

        // Exactly one outbox write, keyed as the frozen event.
        let pending = h.outbox.pending(10).expect("outbox read succeeds");
        assert_eq!(pending.len(), 1, "exactly one outbox record");
        assert!(
            pending[0]
                .idempotency_key()
                .starts_with("asset_capability_updated:"),
            "the outbox record carries the frozen AssetCapabilityUpdated key: {}",
            pending[0].idempotency_key()
        );
        h.journal.record_commit();
        assert_eq!(
            h.journal.ops(),
            vec![
                FakeJournalOp::StateChange {
                    entity: "assets".to_string(),
                    operation: StateChangeOperation::Update,
                    tenant_scope: Some(TenantId::new(uuid(UUID_TENANT_A))),
                },
                FakeJournalOp::OutboxAppend {
                    idempotency_key: format!(
                        "asset_capability_updated:{}",
                        FlexUuid::from_bytes(1u128.to_be_bytes())
                    ),
                    entity: "assets".to_string(),
                    tenant_scope: Some(TenantId::new(uuid(UUID_TENANT_A))),
                },
                FakeJournalOp::Commit,
            ],
            "state change and outbox append adjacent, before the commit marker"
        );
    }

    #[test]
    fn a_cross_tenant_asset_reads_as_missing_without_leaking_existence() {
        let h = harness();
        let (_, _, site) = chain();
        let asset = registered_asset(&site);
        h.assets
            .insert(&h.ctx, &asset)
            .expect("asset registered under tenant A");

        let other_ctx = tenant_ctx(UUID_TENANT_B);
        let err = h
            .use_case
            .execute(&other_ctx, command(asset.asset_id()))
            .unwrap_err();
        assert_eq!(
            err,
            ApplicationError::Authorization(AuthorizationError::ParentOutsideTenantScope {
                entity: "assets"
            })
        );
        // Nothing was written and nothing was emitted.
        assert!(h.journal.ops().is_empty());
    }

    #[test]
    fn an_unknown_asset_id_is_rejected_within_the_tenant_scope() {
        let h = harness();
        let missing = AssetId::new(uuid(UUID_ASSET));
        let err = h.use_case.execute(&h.ctx, command(missing)).unwrap_err();
        assert_eq!(
            err,
            ApplicationError::Authorization(AuthorizationError::ParentOutsideTenantScope {
                entity: "assets"
            })
        );
        assert!(h.journal.ops().is_empty());
    }

    #[test]
    fn the_domain_revalidation_rejects_an_invalid_rated_quantity_before_any_write() {
        let h = harness();
        let (_, _, site) = chain();
        let asset = registered_asset(&site);
        h.assets
            .insert(&h.ctx, &asset)
            .expect("asset registered for the test");

        let mut bad = command(asset.asset_id());
        bad.rated_power_mw = ActivePowerMW::new(0.0).unwrap();
        let err = h.use_case.execute(&h.ctx, bad).unwrap_err();
        assert!(
            matches!(err, ApplicationError::Validation(_)),
            "non-positive rated power is a validation-class rejection: {err}"
        );
        assert!(h.journal.ops().is_empty(), "no state change, no outbox write");
    }
}
