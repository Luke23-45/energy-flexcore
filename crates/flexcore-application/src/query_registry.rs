// P2.03 — `QueryRegistry`: tenant-scoped registry reads (ledger P2.03
//         step 2, "query registry (tenant-scoped)").
//
// Shape: a thin read-side use case over the existing repository ports (P1.07)
// — no read-model, no caching, no extra port. Every method that touches
// tenant-owned data takes the domain `TenantContext` (P0.16 rules 1 and 5);
// the two tenant-registry-root methods are the recorded exception (the tenant
// root is not tenant-owned data — it *is* the tenant; see the
// `TenantRepository` port docs).
//
// Cross-tenant behaviour is the repository ports' contract, unchanged here:
// a foreign-tenant row reads as "missing" (`None`) — existence is never
// leaked across a tenant boundary (P0.16 rule 5; §17.4 threat). An unknown
// identifier inside the caller's tenant also reads as `None`; these are
// deliberately indistinguishable at this boundary (recorded port decision).
//
// Reads emit no events and perform no writes: a query is not a state change,
// so nothing is paired with an outbox record (§6.3 step 1 applies to writes
// only).

use std::sync::Arc;

use crate::error::ApplicationError;
use flexcore_domain::entities::{Asset, Portfolio, Site, Tenant};
use flexcore_domain::ids::{AssetId, PortfolioId, SiteId, TenantId};
use flexcore_domain::tenant_context::TenantContext;
use flexcore_ports::repository::{
    AssetRepository, PortfolioRepository, SiteRepository, TenantRepository,
};

/// Tenant-scoped registry reads (list/get for tenants, portfolios, sites,
/// and assets).
pub struct QueryRegistry {
    tenants: Arc<dyn TenantRepository>,
    portfolios: Arc<dyn PortfolioRepository>,
    sites: Arc<dyn SiteRepository>,
    assets: Arc<dyn AssetRepository>,
}

impl QueryRegistry {
    /// Composes the read use case from the repository ports.
    pub fn new(
        tenants: Arc<dyn TenantRepository>,
        portfolios: Arc<dyn PortfolioRepository>,
        sites: Arc<dyn SiteRepository>,
        assets: Arc<dyn AssetRepository>,
    ) -> Self {
        Self {
            tenants,
            portfolios,
            sites,
            assets,
        }
    }

    // --- Tenant registry root (no context — recorded exception) -------------

    /// Lists all tenants (platform-level query; see the port docs for the
    /// root-aggregate exception).
    pub fn list_tenants(&self) -> Result<Vec<Tenant>, ApplicationError> {
        Ok(self.tenants.list_all()?)
    }

    /// Looks a tenant up by identifier.
    pub fn get_tenant(&self, tenant_id: TenantId) -> Result<Option<Tenant>, ApplicationError> {
        Ok(self.tenants.find_by_id(tenant_id)?)
    }

    // --- Tenant-scoped reads -------------------------------------------------

    /// Lists the caller's tenant's portfolios.
    pub fn list_portfolios(
        &self,
        ctx: &TenantContext,
    ) -> Result<Vec<Portfolio>, ApplicationError> {
        Ok(self.portfolios.list_for_tenant(ctx)?)
    }

    /// Looks a portfolio up within the caller's tenant scope; a foreign or
    /// unknown identifier reads as `None`.
    pub fn get_portfolio(
        &self,
        ctx: &TenantContext,
        portfolio_id: PortfolioId,
    ) -> Result<Option<Portfolio>, ApplicationError> {
        Ok(self.portfolios.find_by_id(ctx, portfolio_id)?)
    }

    /// Lists the caller's tenant's sites.
    pub fn list_sites(&self, ctx: &TenantContext) -> Result<Vec<Site>, ApplicationError> {
        Ok(self.sites.list_for_tenant(ctx)?)
    }

    /// Looks a site up within the caller's tenant scope; a foreign or
    /// unknown identifier reads as `None`.
    pub fn get_site(
        &self,
        ctx: &TenantContext,
        site_id: SiteId,
    ) -> Result<Option<Site>, ApplicationError> {
        Ok(self.sites.find_by_id(ctx, site_id)?)
    }

    /// Lists the caller's tenant's assets.
    pub fn list_assets(&self, ctx: &TenantContext) -> Result<Vec<Asset>, ApplicationError> {
        Ok(self.assets.list_for_tenant(ctx)?)
    }

    /// Looks an asset up within the caller's tenant scope; a foreign or
    /// unknown identifier reads as `None`.
    pub fn get_asset(
        &self,
        ctx: &TenantContext,
        asset_id: AssetId,
    ) -> Result<Option<Asset>, ApplicationError> {
        Ok(self.assets.find_by_id(ctx, asset_id)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flexcore_contracts::ids::FlexUuid;
    use flexcore_contracts::time_model::EventTime;
    use flexcore_contracts::units::ActivePowerMW;
    use flexcore_domain::entities::AssetType;
    use flexcore_domain::ids::AssetId as AssetIdType;
    use flexcore_domain::tenant_context::{AuthenticatedClaims, VerifiedClaims};
    use flexcore_ports::fakes::{
        FakeAssetRepository, FakePortfolioRepository, FakeSiteRepository, FakeTenantRepository,
    };

    const UUID_TENANT_A: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a21";
    const UUID_TENANT_B: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a22";
    const UUID_PORTFOLIO: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a23";
    const UUID_SITE: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a24";
    const UUID_ASSET: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a25";

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

    fn tenant(uuid_text: &str, name: &str) -> Tenant {
        Tenant::try_new(
            TenantId::new(uuid(uuid_text)),
            name.to_string(),
            None,
            format!("idp:{name}"),
            "profile:default".to_string(),
            EventTime::from_unix_millis(1_000).unwrap(),
        )
        .unwrap()
    }

    struct Harness {
        registry: QueryRegistry,
        tenants: Arc<dyn TenantRepository>,
        portfolios: Arc<dyn PortfolioRepository>,
        sites: Arc<dyn SiteRepository>,
        assets: Arc<dyn AssetRepository>,
        ctx_a: TenantContext,
        ctx_b: TenantContext,
    }

    fn harness() -> Harness {
        let tenants: Arc<dyn TenantRepository> = Arc::new(FakeTenantRepository::default());
        let portfolios: Arc<dyn PortfolioRepository> =
            Arc::new(FakePortfolioRepository::default());
        let sites: Arc<dyn SiteRepository> = Arc::new(FakeSiteRepository::default());
        let assets: Arc<dyn AssetRepository> = Arc::new(FakeAssetRepository::default());
        let registry = QueryRegistry::new(
            tenants.clone(),
            portfolios.clone(),
            sites.clone(),
            assets.clone(),
        );
        Harness {
            registry,
            tenants,
            portfolios,
            sites,
            assets,
            ctx_a: tenant_ctx(UUID_TENANT_A),
            ctx_b: tenant_ctx(UUID_TENANT_B),
        }
    }

    #[test]
    fn tenant_reads_return_the_registered_chain_for_the_callers_tenant() {
        let h = harness();
        let t = tenant(UUID_TENANT_A, "Acme");
        h.tenants.insert(&t).expect("tenant registered");

        assert_eq!(
            h.registry.get_tenant(t.tenant_id()).expect("read succeeds"),
            Some(t.clone())
        );
        assert_eq!(
            h.registry
                .list_tenants()
                .expect("read succeeds")
                .len(),
            1
        );

        // An empty registry subtree lists as empty, and a missing identifier
        // reads as None (indistinguishable from a foreign one).
        assert!(h.registry.list_portfolios(&h.ctx_a).expect("read").is_empty());
        assert_eq!(
            h.registry
                .get_portfolio(&h.ctx_a, PortfolioId::new(uuid(UUID_PORTFOLIO)))
                .expect("read succeeds"),
            None
        );
        assert_eq!(
            h.registry
                .get_site(&h.ctx_a, SiteId::new(uuid(UUID_SITE)))
                .expect("read succeeds"),
            None
        );
        assert_eq!(
            h.registry
                .get_asset(&h.ctx_a, AssetIdType::new(uuid(UUID_ASSET)))
                .expect("read succeeds"),
            None
        );
    }

    #[test]
    fn populated_chain_reads_are_tenant_scoped_and_complete_within_scope() {
        let h = harness();
        let t = tenant(UUID_TENANT_A, "Acme");
        h.tenants.insert(&t).expect("tenant registered");
        let portfolio = Portfolio::try_new(
            &t,
            PortfolioId::new(uuid(UUID_PORTFOLIO)),
            "North portfolio".to_string(),
            None,
            "Acme Operations".to_string(),
            "DE-AMPRION".to_string(),
            "rules:constrained-aggregation".to_string(),
            EventTime::from_unix_millis(2_000).unwrap(),
        )
        .unwrap();
        h.portfolios.insert(&h.ctx_a, &portfolio).expect("inserted");
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
        h.sites.insert(&h.ctx_a, &site).expect("inserted");
        let asset = Asset::try_new(
            &site,
            AssetIdType::new(uuid(UUID_ASSET)),
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
        .unwrap();
        h.assets.insert(&h.ctx_a, &asset).expect("inserted");

        // Within scope: every level of the chain resolves.
        assert_eq!(
            h.registry
                .list_portfolios(&h.ctx_a)
                .expect("read succeeds")
                .len(),
            1
        );
        assert_eq!(
            h.registry
                .get_portfolio(&h.ctx_a, portfolio.portfolio_id())
                .expect("read succeeds"),
            Some(portfolio.clone())
        );
        assert_eq!(
            h.registry
                .get_site(&h.ctx_a, site.site_id())
                .expect("read succeeds"),
            Some(site.clone())
        );
        assert_eq!(
            h.registry
                .get_asset(&h.ctx_a, asset.asset_id())
                .expect("read succeeds"),
            Some(asset.clone())
        );
        assert_eq!(
            h.registry.list_assets(&h.ctx_a).expect("read succeeds"),
            vec![asset]
        );
    }

    #[test]
    fn cross_tenant_reads_read_as_missing_without_leaking_existence() {
        let h = harness();
        let t = tenant(UUID_TENANT_A, "Acme");
        h.tenants.insert(&t).expect("tenant registered");
        let portfolio = Portfolio::try_new(
            &t,
            PortfolioId::new(uuid(UUID_PORTFOLIO)),
            "North portfolio".to_string(),
            None,
            "Acme Operations".to_string(),
            "DE-AMPRION".to_string(),
            "rules:constrained-aggregation".to_string(),
            EventTime::from_unix_millis(2_000).unwrap(),
        )
        .unwrap();
        h.portfolios.insert(&h.ctx_a, &portfolio).expect("inserted");

        // Tenant B sees none of tenant A's registry, and tenant A's rows are
        // untouched.
        assert!(h.registry.list_portfolios(&h.ctx_b).expect("read").is_empty());
        assert_eq!(
            h.registry
                .get_portfolio(&h.ctx_b, portfolio.portfolio_id())
                .expect("read succeeds"),
            None
        );
        assert_eq!(
            h.registry
                .list_portfolios(&h.ctx_a)
                .expect("read succeeds")
                .len(),
            1
        );
    }
}
