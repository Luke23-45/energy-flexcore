// P2.03 — `DeactivateAsset`: deactivate an asset of the caller's tenant with
//         a mandatory reason, emitting the frozen §15.3
//         `AssetAvailabilityChanged` event (ledger P2.03 step 2–3).
//
// Tenant context: the use case takes the domain `TenantContext` and never
// reads a tenant identifier from the command (P0.16 rule 5). The asset is
// fetched **through the context**, so an asset from another tenant reads as
// missing (`ParentOutsideTenantScope`) — cross-tenant access is rejected
// without leaking existence.
//
// Semantics (P2.02): deactivation is a registry lifecycle fact, distinct
// from the telemetry-derived `available` flag on `AssetState`. The reason is
// mandatory, validated, and **persisted on the asset record itself**
// (`Asset::deactivate` stores reason + timestamp), so the "why" survives
// with the state it explains. Deactivation is terminal in Phase 2: a second
// deactivation surfaces the domain's `AlreadyDeactivated` error as a
// validation-class rejection.
//
// Event choice (frozen vocabulary): `AssetAvailabilityChanged` is the §15.3
// event for this action, emitted via the transactional outbox exactly like
// every other registry state change (§6.3 step 1) — never a direct NATS
// publish from this use case.
//
// AUDIT GAP (recorded, P2.03): the ledger step also names an audit-record
// write via the `AuditSink` port for this use case. That port is single-homed
// in `flexcore-audit` by the recorded P1.10 decision (it is explicitly NOT a
// repository trait, so `flexcore-ports` does not define it), and the
// dependency rule (`tools/check_dependency_rule.py`) does not allow a
// `flexcore-application → flexcore-audit` edge without an ADR-granted
// exception in `ALLOWED_EXTRA_EDGES`. Both changes are outside this
// subtask's edit scope, so the direct append is NOT implemented here. The
// audit evidence that DOES exist: the reason and deactivation timestamp are
// persisted on the asset record (repository `Update` state change, visible in
// the write journal) and the `AssetAvailabilityChanged` outbox record carries
// the fact to every consumer. Recorded resolution path: either grant the
// application → flexcore-audit edge via ADR (adding the `AuditSink`
// parameter to this use case's constructor), or append the audit record at
// the composition root, where the unit of work that implements `AuditSink`
// (P1.13 `PgUnitOfWork`) is already open.

use std::sync::Arc;

use crate::error::{ApplicationError, AuthorizationError};
use crate::event_assembly::emit;
use crate::payload_encoding::EventPayloadEncoder;
use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::time_model::EventTime;
use flexcore_domain::entities::Asset;
use flexcore_domain::ids::AssetId;
use flexcore_domain::tenant_context::TenantContext;
use flexcore_events::events::{AssetAvailabilityChangedPayload, DomainEvent};
use flexcore_ports::clock::Clock;
use flexcore_ports::event_outbox::EventOutbox;
use flexcore_ports::id_generator::IdGenerator;
use flexcore_ports::repository::AssetRepository;
use flexcore_ports::state_change::entities;

/// The deactivation input. The asset is referenced by identifier; its tenant
/// scope is verified through the context. The reason is mandatory (P2.02:
/// the durable record of why the asset left coordination scope).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeactivateAssetCommand {
    /// The asset to deactivate (must belong to the caller's tenant).
    pub asset_id: AssetId,
    /// Why the asset is being deactivated (validated non-empty by the
    /// domain).
    pub reason: String,
}

/// Deactivates an asset within the caller's tenant.
pub struct DeactivateAsset {
    assets: Arc<dyn AssetRepository>,
    outbox: Arc<dyn EventOutbox>,
    ids: Arc<dyn IdGenerator>,
    clock: Arc<dyn Clock>,
    encoder: Arc<dyn EventPayloadEncoder>,
}

impl DeactivateAsset {
    /// Composes the use case from its ports.
    pub fn new(
        assets: Arc<dyn AssetRepository>,
        outbox: Arc<dyn EventOutbox>,
        ids: Arc<dyn IdGenerator>,
        clock: Arc<dyn Clock>,
        encoder: Arc<dyn EventPayloadEncoder>,
    ) -> Self {
        Self {
            assets,
            outbox,
            ids,
            clock,
            encoder,
        }
    }

    /// Deactivates the asset: the asset is resolved through the context, the
    /// domain applies the terminal lifecycle transition with the reason,
    /// one update and one outbox write follow — inside the caller's unit of
    /// work.
    pub fn execute(
        &self,
        ctx: &TenantContext,
        command: DeactivateAssetCommand,
    ) -> Result<Asset, ApplicationError> {
        // Cross-tenant rejection: an asset outside the caller's tenant reads
        // as missing (no existence leak).
        let asset = self
            .assets
            .find_by_id(ctx, command.asset_id)?
            .ok_or(AuthorizationError::ParentOutsideTenantScope {
                entity: entities::ASSETS,
            })?;

        // Domain lifecycle transition: terminal, reason-validated, immutable
        // value in / new value out.
        let deactivated = asset.deactivate(command.reason, self.clock.now_event_time())?;

        let state_change = self.assets.update(ctx, &deactivated)?;

        // One transactional outbox write for the state change (§6.3 step 1),
        // carrying the frozen `AssetAvailabilityChanged` event (§15.3).
        let correlation_id: FlexUuid = self.ids.new_uuid();
        let event_time: EventTime = self.clock.now_event_time();
        let event = DomainEvent::AssetAvailabilityChanged(AssetAvailabilityChangedPayload {
            correlation_id,
            tenant_id: ctx.tenant_id(),
            site_id: deactivated.site_id(),
            asset_id: deactivated.asset_id(),
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

        Ok(deactivated)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flexcore_contracts::ids::FlexUuid;
    use flexcore_contracts::units::ActivePowerMW;
    use flexcore_domain::entities::{AssetType, Portfolio, Site, Tenant};
    use flexcore_domain::ids::{PortfolioId, SiteId, TenantId};
    use flexcore_domain::tenant_context::{AuthenticatedClaims, VerifiedClaims};
    use flexcore_events::envelope::EventEnvelope;
    use flexcore_ports::fakes::{
        CountingIdGenerator, FakeAssetRepository, FakeEventOutbox, FakeJournalOp,
        FakeWriteJournal, FixedClock,
    };
    use flexcore_ports::state_change::StateChangeOperation;

    const UUID_TENANT_A: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a11";
    const UUID_TENANT_B: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a12";
    const UUID_PORTFOLIO: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a13";
    const UUID_SITE: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a14";
    const UUID_ASSET: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a15";

    /// Deterministic payload stub: only the wire rendering is injected
    /// (`payload_encoding` module header).
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
        TenantContext::from_authenticated_claims(AuthenticatedClaims::from_verified(
            VerifiedClaims {
                tenant_id: TenantId::new(uuid(uuid_text)),
                subject: "user:operator-7".to_string(),
            },
        ))
    }

    fn registered_asset() -> Asset {
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
        Asset::try_new(
            &site,
            AssetId::new(uuid(UUID_ASSET)),
            "BESS 1".to_string(),
            None,
            AssetType::Bess,
            "vendor-x/model-1".to_string(),
            ActivePowerMW::new(2.0).unwrap(),
            None,
            "three-phase".to_string(),
            "bus-17".to_string(),
            "capability:bess-v1".to_string(),
            EventTime::from_unix_millis(4_000).unwrap(),
        )
        .unwrap()
    }

    struct Harness {
        use_case: DeactivateAsset,
        assets: Arc<dyn AssetRepository>,
        outbox: Arc<dyn EventOutbox>,
        journal: FakeWriteJournal,
        ctx: TenantContext,
    }

    fn harness() -> Harness {
        let journal = FakeWriteJournal::new();
        let assets: Arc<dyn AssetRepository> =
            Arc::new(FakeAssetRepository::new(journal.clone()));
        let outbox: Arc<dyn EventOutbox> = Arc::new(FakeEventOutbox::new(journal.clone()));
        let use_case = DeactivateAsset::new(
            assets.clone(),
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

    #[test]
    fn happy_path_deactivates_records_the_reason_and_emits_asset_availability_changed() {
        let h = harness();
        let asset = registered_asset();
        h.assets
            .insert(&h.ctx, &asset)
            .expect("asset registered for the test");

        let deactivated = h
            .use_case
            .execute(
                &h.ctx,
                DeactivateAssetCommand {
                    asset_id: asset.asset_id(),
                    reason: "planned maintenance".to_string(),
                },
            )
            .expect("deactivation succeeds");
        assert_eq!(
            deactivated.lifecycle_status(),
            flexcore_domain::entities::AssetLifecycleStatus::Deactivated
        );
        assert_eq!(deactivated.deactivation_reason(), Some("planned maintenance"));
        assert_eq!(deactivated.deactivated_at().unwrap().unix_millis(), 10_000);

        // Stored state follows.
        let stored = h
            .assets
            .find_by_id(&h.ctx, asset.asset_id())
            .expect("find succeeds")
            .expect("row exists");
        assert_eq!(
            stored.lifecycle_status(),
            flexcore_domain::entities::AssetLifecycleStatus::Deactivated
        );

        // The outbox holds exactly one record, keyed as the frozen event.
        let pending = h.outbox.pending(10).expect("outbox read succeeds");
        assert_eq!(pending.len(), 1, "exactly one outbox record");
        assert!(
            pending[0]
                .idempotency_key()
                .starts_with("asset_availability_changed:"),
            "the outbox record carries the frozen AssetAvailabilityChanged key: {}",
            pending[0].idempotency_key()
        );

        // One state change, one outbox append for the frozen event, in that
        // order, before the commit marker (§6.3 step 1 evidence).
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
                        "asset_availability_changed:{}",
                        FlexUuid::from_bytes(1u128.to_be_bytes())
                    ),
                    entity: "assets".to_string(),
                    tenant_scope: Some(TenantId::new(uuid(UUID_TENANT_A))),
                },
                FakeJournalOp::Commit,
            ]
        );
    }

    #[test]
    fn deactivation_requires_a_reason_and_is_terminal() {
        let h = harness();
        let asset = registered_asset();
        h.assets
            .insert(&h.ctx, &asset)
            .expect("asset registered for the test");

        // Empty reason: validation-class rejection, no write, no event.
        let err = h
            .use_case
            .execute(
                &h.ctx,
                DeactivateAssetCommand {
                    asset_id: asset.asset_id(),
                    reason: "   ".to_string(),
                },
            )
            .unwrap_err();
        assert!(
            matches!(err, ApplicationError::Validation(_)),
            "an empty reason is a validation-class rejection: {err}"
        );
        assert!(h.journal.ops().is_empty());

        // Happy path first, then the terminal guard.
        let _ = h
            .use_case
            .execute(
                &h.ctx,
                DeactivateAssetCommand {
                    asset_id: asset.asset_id(),
                    reason: "retired".to_string(),
                },
            )
            .expect("first deactivation succeeds");
        h.journal.record_commit();
        let ops_after_first = h.journal.ops().len();

        let err = h
            .use_case
            .execute(
                &h.ctx,
                DeactivateAssetCommand {
                    asset_id: asset.asset_id(),
                    reason: "again".to_string(),
                },
            )
            .unwrap_err();
        assert!(
            matches!(err, ApplicationError::Validation(_)),
            "deactivation is terminal: {err}"
        );
        assert_eq!(
            h.journal.ops().len(),
            ops_after_first,
            "no state change and no outbox write after the terminal rejection"
        );
    }

    #[test]
    fn a_cross_tenant_asset_reads_as_missing_without_leaking_existence() {
        let h = harness();
        let asset = registered_asset();
        h.assets
            .insert(&h.ctx, &asset)
            .expect("asset registered under tenant A");

        let other_ctx = tenant_ctx(UUID_TENANT_B);
        let err = h
            .use_case
            .execute(
                &other_ctx,
                DeactivateAssetCommand {
                    asset_id: asset.asset_id(),
                    reason: "maintenance".to_string(),
                },
            )
            .unwrap_err();
        assert_eq!(
            err,
            ApplicationError::Authorization(AuthorizationError::ParentOutsideTenantScope {
                entity: "assets"
            })
        );
        assert!(h.journal.ops().is_empty());
    }

    #[test]
    fn an_unknown_asset_id_is_rejected_within_the_tenant_scope() {
        let h = harness();
        let err = h
            .use_case
            .execute(
                &h.ctx,
                DeactivateAssetCommand {
                    asset_id: AssetId::new(uuid(UUID_ASSET)),
                    reason: "maintenance".to_string(),
                },
            )
            .unwrap_err();
        assert_eq!(
            err,
            ApplicationError::Authorization(AuthorizationError::ParentOutsideTenantScope {
                entity: "assets"
            })
        );
        assert!(h.journal.ops().is_empty());
    }
}
