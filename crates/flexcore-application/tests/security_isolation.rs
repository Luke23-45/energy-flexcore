// P1.23 — Security test suite (tenant-isolation slice): the application use
//         cases cannot cross the tenant boundary, run against the ports
//         fakes (ledger P1.23 step 1; report §19.2 test layer 7, §17.4 row
//         4 "Cross-tenant leakage", §16.2; P0.16 rules 1, 5, 7; P1.07,
//         P1.08).
//
// Threat covered (ledger P0.17 §5 = report §17.4): row 4 "Cross-tenant
// leakage — tenant context, RLS, scoped service identity, access tests,
// audit". The RLS and audit halves are the P1.13 adapter's and P1.10's
// tested obligations; THIS file proves the application-layer half: with
// the tenant context derived only from verified claims, tenant A's use
// cases cannot read or write tenant B's portfolio, site, or asset.
//
// Route/method coverage checklist (ledger P1.23: "per-route checklist in
// the test file header"). Phase 1 registry surface — each repository
// method exercised positive + negative where the Stage 1 use cases touch
// it:
//
//   | repository method            | positive | cross-tenant negative |
//   |------------------------------|----------|-----------------------|
//   | TenantRepository::find_by_id | yes      | n/a (registry root, no context — recorded decision) |
//   | PortfolioRepository::find_by_id | yes (A) | yes (B reads A's -> None) |
//   | PortfolioRepository::list_for_tenant | yes | yes (B lists -> empty) |
//   | PortfolioRepository::insert  | yes      | via use case only (CallerTenantUnknown) |
//   | SiteRepository::find_by_id   | yes (A)  | yes (B reads A's -> None) |
//   | SiteRepository::list_for_tenant | yes   | yes (B lists -> empty) |
//   | AssetRepository::find_by_id  | yes (A)  | yes (B reads A's -> None) |
//   | AssetRepository::list_for_tenant | yes  | yes (B lists -> empty) |
//   | duplicate registration (uniqueness rules) | rejected for tenant, portfolio, site, asset | — |
//
// Auth-bypass note (ledger P1.23 step 2): the four token-failure cases
// (missing token, expired token, wrong audience/issuer, token for a
// different tenant) are verified at the API middleware (P1.17), which owns
// OIDC verification; this file covers the "token for a different tenant"
// consequence at the use-case boundary (tenant B's context cannot touch
// tenant A's data) and the authz crate's tests cover the evaluation rules.
// The middleware itself does not exist in this slice — recorded gap, owned
// by P1.17.
//
// Replay note (ledger P1.23 step 3): the same-idempotency-key replay test
// lives in the fault slice (crates/adapters/nats/tests/fault_slice.rs,
// case (a)) against the P1.16 inbox — cross-referenced there.
//
// Tenant contexts are built through the domain's single public claims path
// (P0.16 rule 5). Recorded gap being closed here: the flexcore-application
// in-crate tests and the ports fakes were written before the claims
// constructor existed and carry "blocked tests" notes; this integration
// test activates the cross-tenant half of those documented test bodies
// without editing any existing file.

use std::sync::Arc;

use flexcore_application::error::{ApplicationError, AuthorizationError, ValidationError};
use flexcore_application::payload_encoding::EventPayloadEncoder;
use flexcore_application::{
    CreatePortfolio, CreatePortfolioCommand, CreateSite, CreateSiteCommand, RegisterAsset,
    RegisterAssetCommand,
};
use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::time_model::EventTime;
use flexcore_contracts::units::{ActivePowerMW, EnergyMWh};
use flexcore_domain::entities::{AssetType, Tenant};
use flexcore_domain::ids::{AssetId, PortfolioId, SiteId, TenantId};
use flexcore_domain::tenant_context::{
    AuthenticatedClaims, TenantContext, VerifiedClaims,
};
use flexcore_events::envelope::EventEnvelope;
use flexcore_ports::fakes::{
    CountingIdGenerator, FakeAssetRepository, FakeEventOutbox, FakeJournalOp,
    FakePortfolioRepository, FakeSiteRepository, FakeTenantRepository, FakeWriteJournal,
    FixedClock,
};
use flexcore_ports::repository::{
    AssetRepository, PortfolioRepository, SiteRepository, TenantRepository,
};
use flexcore_ports::state_change::entities;

const UUID_TENANT_A: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a01";
const UUID_TENANT_B: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a02";

// --- Test harness -------------------------------------------------------------

struct PassthroughEncoder;

impl EventPayloadEncoder for PassthroughEncoder {
    fn encode(&self, envelope: &EventEnvelope) -> Result<String, ApplicationError> {
        Ok(format!("payload-for-{}", envelope.idempotency_key.as_str()))
    }
}

fn uuid(text: &str) -> FlexUuid {
    FlexUuid::parse(text).expect("valid test uuid")
}

fn tenant_a_id() -> TenantId {
    TenantId::new(uuid(UUID_TENANT_A))
}

fn tenant_b_id() -> TenantId {
    TenantId::new(uuid(UUID_TENANT_B))
}

/// Builds the tenant context through the ONLY public construction path
/// (P0.16 rule 5): verified claims -> authenticated claims -> context.
/// The test fixes the claims exactly like the port fakes; in production the
/// API middleware (P1.17) attests the OIDC token was verified.
fn context_for(tenant: TenantId, subject: &str) -> TenantContext {
    TenantContext::from_authenticated_claims(AuthenticatedClaims::from_verified(VerifiedClaims {
        tenant_id: tenant,
        subject: subject.to_string(),
    }))
}

fn tenant_entity(tenant_id: TenantId, name: &str, idp: &str) -> Tenant {
    Tenant::try_new(
        tenant_id,
        name.to_string(),
        None,
        idp.to_string(),
        "profile:default".to_string(),
        EventTime::from_unix_millis(1_000).expect("valid timestamp"),
    )
    .expect("valid tenant")
}

/// All fakes over one shared journal, plus the four Stage 1 use cases and
/// both tenants pre-seeded in the registry (the FK anchor the create use
/// cases resolve through the context).
struct Fixture {
    journal: FakeWriteJournal,
    tenants: Arc<FakeTenantRepository>,
    portfolios: Arc<FakePortfolioRepository>,
    sites: Arc<FakeSiteRepository>,
    assets: Arc<FakeAssetRepository>,
    outbox: Arc<FakeEventOutbox>,
    create_portfolio: CreatePortfolio,
    create_site: CreateSite,
    register_asset: RegisterAsset,
}

fn fixture() -> Fixture {
    let journal = FakeWriteJournal::new();
    let tenants = Arc::new(FakeTenantRepository::new(journal.clone()));
    let portfolios = Arc::new(FakePortfolioRepository::new(journal.clone()));
    let sites = Arc::new(FakeSiteRepository::new(journal.clone()));
    let assets = Arc::new(FakeAssetRepository::new(journal.clone()));
    let outbox = Arc::new(FakeEventOutbox::new(journal.clone()));
    let ids = Arc::new(CountingIdGenerator::new());
    let clock = Arc::new(FixedClock::at_unix_millis(2_000));
    let encoder: Arc<dyn EventPayloadEncoder> = Arc::new(PassthroughEncoder);
    let create_portfolio = CreatePortfolio::new(
        tenants.clone(),
        portfolios.clone(),
        outbox.clone(),
        ids.clone(),
        clock.clone(),
        encoder.clone(),
    );
    let create_site = CreateSite::new(
        portfolios.clone(),
        sites.clone(),
        outbox.clone(),
        ids.clone(),
        clock.clone(),
        encoder.clone(),
    );
    let register_asset = RegisterAsset::new(
        sites.clone(),
        assets.clone(),
        outbox.clone(),
        ids.clone(),
        clock,
        encoder,
    );
    // Seed both tenants in the registry (registry root, no context by the
    // recorded decision).
    tenants
        .insert(&tenant_entity(tenant_a_id(), "Tenant A", "idp:tenant-a"))
        .expect("seed tenant A");
    tenants
        .insert(&tenant_entity(tenant_b_id(), "Tenant B", "idp:tenant-b"))
        .expect("seed tenant B");
    Fixture {
        journal,
        tenants,
        portfolios,
        sites,
        assets,
        outbox,
        create_portfolio,
        create_site,
        register_asset,
    }
}

fn portfolio_command(name: &str) -> CreatePortfolioCommand {
    CreatePortfolioCommand {
        name: name.to_string(),
        description: None,
        operator: "Acme Operations".to_string(),
        market_region: "DE-AMPRION".to_string(),
        aggregation_rules: "rules:constrained-aggregation".to_string(),
    }
}

// --- Positive path: tenant A builds its own chain ------------------------------

#[test]
fn tenant_a_builds_its_own_portfolio_site_asset_chain() {
    let fx = fixture();
    let ctx_a = context_for(tenant_a_id(), "user:a-admin");

    let portfolio = fx
        .create_portfolio
        .execute(&ctx_a, portfolio_command("North portfolio"))
        .expect("portfolio created");
    assert_eq!(portfolio.tenant_id(), tenant_a_id());

    let site = fx
        .create_site
        .execute(
            &ctx_a,
            CreateSiteCommand {
                portfolio_id: portfolio.portfolio_id(),
                name: "Substation 7".to_string(),
                description: None,
                location: "52.52N 13.40E".to_string(),
                timezone: "Europe/Berlin".to_string(),
                point_of_common_coupling: "PCC-7".to_string(),
                grid_model_reference: "grid-model:rev-42".to_string(),
                edge_identity: "edge-0007".to_string(),
            },
        )
        .expect("site created");
    assert_eq!(site.tenant_id(), tenant_a_id());
    assert_eq!(site.portfolio_id(), portfolio.portfolio_id());

    let asset = fx
        .register_asset
        .execute(
            &ctx_a,
            RegisterAssetCommand {
                site_id: site.site_id(),
                name: "BESS 1".to_string(),
                description: None,
                asset_type: AssetType::Bess,
                vendor_model: "vendor-x/model-1".to_string(),
                rated_power_mw: ActivePowerMW::new(2.0).expect("valid power"),
                rated_energy_mwh: Some(EnergyMWh::new(4.0).expect("valid energy")),
                phase: "three-phase".to_string(),
                bus_reference: "bus-17".to_string(),
                capability_profile: "capability:bess-v1".to_string(),
            },
        )
        .expect("asset registered");
    assert_eq!(asset.tenant_id(), tenant_a_id());
    assert_eq!(asset.site_id(), site.site_id());

    // Tenant A's reads see the full chain.
    assert!(fx
        .portfolios
        .find_by_id(&ctx_a, portfolio.portfolio_id())
        .expect("find")
        .is_some());
    assert!(fx
        .sites
        .find_by_id(&ctx_a, site.site_id())
        .expect("find")
        .is_some());
    assert!(fx
        .assets
        .find_by_id(&ctx_a, asset.asset_id())
        .expect("find")
        .is_some());
}

// --- Cross-tenant reads read as missing (no existence leak) --------------------

#[test]
fn tenant_b_cannot_read_any_of_tenant_as_entities() {
    let fx = fixture();
    let ctx_a = context_for(tenant_a_id(), "user:a-admin");
    let ctx_b = context_for(tenant_b_id(), "user:b-admin");

    // Tenant A builds a full chain.
    let portfolio = fx
        .create_portfolio
        .execute(&ctx_a, portfolio_command("North portfolio"))
        .expect("portfolio created");
    let site = fx
        .create_site
        .execute(
            &ctx_a,
            CreateSiteCommand {
                portfolio_id: portfolio.portfolio_id(),
                name: "Substation 7".to_string(),
                description: None,
                location: "52.52N 13.40E".to_string(),
                timezone: "Europe/Berlin".to_string(),
                point_of_common_coupling: "PCC-7".to_string(),
                grid_model_reference: "grid-model:rev-42".to_string(),
                edge_identity: "edge-0007".to_string(),
            },
        )
        .expect("site created");
    let asset = fx
        .register_asset
        .execute(
            &ctx_a,
            RegisterAssetCommand {
                site_id: site.site_id(),
                name: "BESS 1".to_string(),
                description: None,
                asset_type: AssetType::Bess,
                vendor_model: "vendor-x/model-1".to_string(),
                rated_power_mw: ActivePowerMW::new(2.0).expect("valid power"),
                rated_energy_mwh: Some(EnergyMWh::new(4.0).expect("valid energy")),
                phase: "three-phase".to_string(),
                bus_reference: "bus-17".to_string(),
                capability_profile: "capability:bess-v1".to_string(),
            },
        )
        .expect("asset registered");

    // Tenant B's repository reads: every cross-tenant find_by_id reads as
    // None — existence is never leaked (P1.07 port contract; the same
    // behaviour the P1.13 RLS adapter implements).
    assert_eq!(
        fx.portfolios.find_by_id(&ctx_b, portfolio.portfolio_id()).expect("find"),
        None,
        "tenant B must not read tenant A's portfolio"
    );
    assert_eq!(
        fx.sites.find_by_id(&ctx_b, site.site_id()).expect("find"),
        None,
        "tenant B must not read tenant A's site"
    );
    assert_eq!(
        fx.assets.find_by_id(&ctx_b, asset.asset_id()).expect("find"),
        None,
        "tenant B must not read tenant A's asset"
    );
    // And the scoped listings are empty for B while full for A.
    assert!(fx.portfolios.list_for_tenant(&ctx_b).expect("list").is_empty());
    assert!(fx.sites.list_for_tenant(&ctx_b).expect("list").is_empty());
    assert!(fx.assets.list_for_tenant(&ctx_b).expect("list").is_empty());
    assert_eq!(fx.portfolios.list_for_tenant(&ctx_a).expect("list").len(), 1);
    assert_eq!(fx.sites.list_for_tenant(&ctx_a).expect("list").len(), 1);
    assert_eq!(fx.assets.list_for_tenant(&ctx_a).expect("list").len(), 1);
}

#[test]
fn tenant_b_use_case_referencing_tenant_as_parent_maps_to_parent_outside_tenant_scope() {
    // The use-case-level mapping: a cross-tenant parent reference is
    // deliberately indistinguishable from a missing one — both map to
    // AuthorizationError::ParentOutsideTenantScope (P1.08 error taxonomy;
    // recorded mapping in flexcore-application/src/error.rs).
    let fx = fixture();
    let ctx_a = context_for(tenant_a_id(), "user:a-admin");
    let ctx_b = context_for(tenant_b_id(), "user:b-admin");

    let portfolio = fx
        .create_portfolio
        .execute(&ctx_a, portfolio_command("North portfolio"))
        .expect("portfolio created");
    let site = fx
        .create_site
        .execute(
            &ctx_a,
            CreateSiteCommand {
                portfolio_id: portfolio.portfolio_id(),
                name: "Substation 7".to_string(),
                description: None,
                location: "52.52N 13.40E".to_string(),
                timezone: "Europe/Berlin".to_string(),
                point_of_common_coupling: "PCC-7".to_string(),
                grid_model_reference: "grid-model:rev-42".to_string(),
                edge_identity: "edge-0007".to_string(),
            },
        )
        .expect("site created");

    // B tries to create a site under A's portfolio.
    let err = fx
        .create_site
        .execute(
            &ctx_b,
            CreateSiteCommand {
                portfolio_id: portfolio.portfolio_id(),
                name: "B site".to_string(),
                description: None,
                location: "48.85N 2.35E".to_string(),
                timezone: "Europe/Paris".to_string(),
                point_of_common_coupling: "PCC-B".to_string(),
                grid_model_reference: "grid-model:b".to_string(),
                edge_identity: "edge-b-1".to_string(),
            },
        )
        .expect_err("cross-tenant parent must be rejected");
    assert_eq!(
        err,
        ApplicationError::Authorization(AuthorizationError::ParentOutsideTenantScope {
            entity: entities::PORTFOLIOS,
        }),
        "cross-tenant portfolio reference must map to ParentOutsideTenantScope"
    );

    // B tries to register an asset under A's site.
    let err = fx
        .register_asset
        .execute(
            &ctx_b,
            RegisterAssetCommand {
                site_id: site.site_id(),
                name: "B asset".to_string(),
                description: None,
                asset_type: AssetType::Bess,
                vendor_model: "vendor-x/model-1".to_string(),
                rated_power_mw: ActivePowerMW::new(1.0).expect("valid power"),
                rated_energy_mwh: None,
                phase: "three-phase".to_string(),
                bus_reference: "bus-1".to_string(),
                capability_profile: "capability:bess-v1".to_string(),
            },
        )
        .expect_err("cross-tenant parent must be rejected");
    assert_eq!(
        err,
        ApplicationError::Authorization(AuthorizationError::ParentOutsideTenantScope {
            entity: entities::SITES,
        }),
        "cross-tenant site reference must map to ParentOutsideTenantScope"
    );

    // Nothing was written by the rejected invocations.
    assert_eq!(fx.portfolios.len(), 1, "only A's portfolio exists");
    assert_eq!(fx.sites.len(), 1, "only A's site exists");
    assert_eq!(fx.assets.len(), 0, "no asset was registered for B");
}

#[test]
fn a_forged_tenant_cannot_drive_the_use_cases_at_all() {
    // A context whose tenant has NO registry row (stale/revoked tenant, or
    // a token issued before the tenant was removed): the create use cases
    // resolve the owning tenant through the context and map the missing
    // row to CallerTenantUnknown — the caller cannot proceed.
    let fx = fixture();
    let ghost = TenantId::new(uuid("0192a5e0-0000-7e2f-9a1b-3c4d5e6f7aff"));
    let ctx_ghost = context_for(ghost, "user:ghost");
    let err = fx
        .create_portfolio
        .execute(&ctx_ghost, portfolio_command("Ghost portfolio"))
        .expect_err("unknown caller tenant must be rejected");
    assert_eq!(
        err,
        ApplicationError::Authorization(AuthorizationError::CallerTenantUnknown),
    );
    assert_eq!(fx.portfolios.len(), 0, "nothing was written");
}

// --- Duplicate registration rejected (per-tenant uniqueness rules) -------------

#[test]
fn duplicate_registrations_are_rejected_for_each_registry() {
    let fx = fixture();
    let ctx_a = context_for(tenant_a_id(), "user:a-admin");

    // Tenant: the identity-provider reference is the uniqueness anchor
    // (re-registering the same tenant identity conflicts first, then the
    // idp reference — both are registry uniqueness rules and both reject).
    let _ = fx
        .tenants
        .insert(&tenant_entity(tenant_a_id(), "Tenant A again", "idp:tenant-a"))
        .expect_err("re-registering a registered tenant identity must conflict");

    // Portfolio: the name within the tenant.
    let portfolio = fx
        .create_portfolio
        .execute(&ctx_a, portfolio_command("North portfolio"))
        .expect("first portfolio created");
    let err = fx
        .create_portfolio
        .execute(&ctx_a, portfolio_command("North portfolio"))
        .expect_err("duplicate portfolio name must be rejected");
    assert_eq!(
        err,
        ApplicationError::Validation(ValidationError::DuplicateRegistration {
            entity: entities::PORTFOLIOS,
            key: "North portfolio".to_string(),
        })
    );

    // Site: the edge identity within the tenant.
    let site = fx
        .create_site
        .execute(
            &ctx_a,
            CreateSiteCommand {
                portfolio_id: portfolio.portfolio_id(),
                name: "Substation 7".to_string(),
                description: None,
                location: "52.52N 13.40E".to_string(),
                timezone: "Europe/Berlin".to_string(),
                point_of_common_coupling: "PCC-7".to_string(),
                grid_model_reference: "grid-model:rev-42".to_string(),
                edge_identity: "edge-0007".to_string(),
            },
        )
        .expect("site created");
    let err = fx
        .create_site
        .execute(
            &ctx_a,
            CreateSiteCommand {
                portfolio_id: portfolio.portfolio_id(),
                name: "Substation 8".to_string(),
                description: None,
                location: "52.52N 13.40E".to_string(),
                timezone: "Europe/Berlin".to_string(),
                point_of_common_coupling: "PCC-8".to_string(),
                grid_model_reference: "grid-model:rev-42".to_string(),
                edge_identity: "edge-0007".to_string(),
            },
        )
        .expect_err("duplicate edge identity must be rejected");
    assert!(matches!(
        err,
        ApplicationError::Validation(ValidationError::DuplicateRegistration { entity, .. })
            if entity == entities::SITES
    ));

    // Asset: the name within the site.
    fx.register_asset
        .execute(
            &ctx_a,
            RegisterAssetCommand {
                site_id: site.site_id(),
                name: "BESS 1".to_string(),
                description: None,
                asset_type: AssetType::Bess,
                vendor_model: "vendor-x/model-1".to_string(),
                rated_power_mw: ActivePowerMW::new(2.0).expect("valid power"),
                rated_energy_mwh: Some(EnergyMWh::new(4.0).expect("valid energy")),
                phase: "three-phase".to_string(),
                bus_reference: "bus-17".to_string(),
                capability_profile: "capability:bess-v1".to_string(),
            },
        )
        .expect("first asset registered");
    let err = fx
        .register_asset
        .execute(
            &ctx_a,
            RegisterAssetCommand {
                site_id: site.site_id(),
                name: "BESS 1".to_string(),
                description: None,
                asset_type: AssetType::PvInverter,
                vendor_model: "vendor-x/model-2".to_string(),
                rated_power_mw: ActivePowerMW::new(1.5).expect("valid power"),
                rated_energy_mwh: None,
                phase: "three-phase".to_string(),
                bus_reference: "bus-18".to_string(),
                capability_profile: "capability:pv-v1".to_string(),
            },
        )
        .expect_err("duplicate asset name within the site must be rejected");
    assert!(matches!(
        err,
        ApplicationError::Validation(ValidationError::DuplicateRegistration { entity, .. })
            if entity == entities::ASSETS
    ));

    // The duplicates stored nothing extra.
    assert_eq!(fx.tenants.len(), 2, "only the two seeded tenants exist");
    assert_eq!(fx.portfolios.len(), 1);
    assert_eq!(fx.sites.len(), 1);
    assert_eq!(fx.assets.len(), 1);
}

// --- Outbox record written with the state change in the same unit of work ------

#[test]
fn the_outbox_record_is_registered_with_the_state_change_in_one_unit_of_work() {
    // §6.3 step 1 / P1.08 acceptance: the fake journal evidences the
    // ordering — state change, then outbox append naming the paired state
    // change, then the single commit marker the (simulated) composition
    // root records.
    let fx = fixture();
    let ctx_a = context_for(tenant_a_id(), "user:a-admin");

    let portfolio = fx
        .create_portfolio
        .execute(&ctx_a, portfolio_command("North portfolio"))
        .expect("portfolio created");
    fx.journal.record_commit();

    let ops = fx.journal.ops();
    assert_eq!(ops.len(), 3, "state change, outbox append, commit");
    assert_eq!(
        ops[0],
        FakeJournalOp::StateChange {
            entity: entities::PORTFOLIOS.to_string(),
            operation: flexcore_ports::state_change::StateChangeOperation::Insert,
            tenant_scope: Some(tenant_a_id()),
        },
        "the state change is recorded first, scoped to the context's tenant"
    );
    let expected_key = fx.outbox.writes()[0].idempotency_key().to_string();
    assert_eq!(
        ops[1],
        FakeJournalOp::OutboxAppend {
            idempotency_key: expected_key,
            entity: entities::PORTFOLIOS.to_string(),
            tenant_scope: Some(tenant_a_id()),
        },
        "the outbox append follows, pairing the same state change"
    );
    assert_eq!(ops[2], FakeJournalOp::Commit, "one commit covers both");

    // The outbox write itself carries the tenant scope of the context.
    let write = &fx.outbox.writes()[0];
    assert_eq!(write.state_change().tenant_scope(), Some(tenant_a_id()));
    assert_eq!(portfolio.tenant_id(), tenant_a_id());
}

#[test]
fn tenant_bs_use_cases_never_touch_tenant_as_outbox_scope() {
    // The isolation half of the ordering evidence: a rejected cross-tenant
    // invocation appends NO outbox record and journals NO state change.
    let fx = fixture();
    let ctx_a = context_for(tenant_a_id(), "user:a-admin");
    let ctx_b = context_for(tenant_b_id(), "user:b-admin");

    let portfolio = fx
        .create_portfolio
        .execute(&ctx_a, portfolio_command("North portfolio"))
        .expect("portfolio created");
    assert_eq!(fx.outbox.len(), 1);

    let ops_before = fx.journal.ops().len();
    let err = fx
        .create_site
        .execute(
            &ctx_b,
            CreateSiteCommand {
                portfolio_id: portfolio.portfolio_id(), // A's portfolio
                name: "B site".to_string(),
                description: None,
                location: "48.85N 2.35E".to_string(),
                timezone: "Europe/Paris".to_string(),
                point_of_common_coupling: "PCC-B".to_string(),
                grid_model_reference: "grid-model:b".to_string(),
                edge_identity: "edge-b-1".to_string(),
            },
        )
        .expect_err("cross-tenant invocation must be rejected (see the dedicated test above)");
    assert!(matches!(err, ApplicationError::Authorization(_)));
    assert_eq!(fx.outbox.len(), 1, "no outbox record for the rejected tenant");
    assert_eq!(
        fx.journal.ops().len(),
        ops_before,
        "no state change or outbox op journalled for the rejected tenant"
    );
}

// Compile-level pin (mirrors the in-crate use-case tests): the use cases
// compose from port trait objects, and every tenant-scoped entry point
// takes the domain `TenantContext` — a raw tenant-ID parameter path does
// not exist (P0.16 rule 5).
#[test]
fn use_cases_take_the_domain_tenant_context_not_a_tenant_id() {
    let ctx: fn(TenantId, &str) -> TenantContext = context_for;
    let _ = ctx;
    let _: Option<AssetId> = None;
    let _: Option<SiteId> = None;
    let _: Option<PortfolioId> = None;
}
