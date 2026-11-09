// P1.07 — Repository ports, one per Stage 1 aggregate (P1.07 step 2).
//
// Recorded decision (crate README §"repository-trait shape"): one port per
// aggregate — [`TenantRepository`], [`PortfolioRepository`],
// [`SiteRepository`], [`AssetRepository`] — rather than a single
// mega-repository. Each aggregate has its own lifecycle and its own
// tenant-scoping rule; separate traits keep the `TenantContext` obligation
// precise and let adapters implement only what a Stage 1 slice needs.
//
// Rules encoded here:
//
// - **Every method that touches tenant-owned data takes the domain
//   `TenantContext`** (P0.16 rules 1 and 5). There is no method for
//   tenant-owned data without one, and no raw-`TenantId` query path: the
//   context is the only tenant-scoping token the ports accept. The one
//   exception is the [`TenantRepository`] itself — the tenant registry root
//   is not tenant-owned data (it *is* the tenant; P1.12 §4.1 carries no
//   `tenant_id` and no per-tenant row filter), so its methods carry no
//   context; access control for platform-level tenant administration is an
//   application/API-layer authorization concern (P1.11), and this exception
//   is recorded in the crate README.
// - **Cross-tenant reads do not leak existence.** For read methods the
//   adapter must return "not found" for a row that exists but belongs to a
//   different tenant — existence inside another tenant's scope is never
//   revealed. Writes that would cross the scope are refused with
//   [`RepositoryError::TenantMismatch`].
// - **Write methods return a [`StateChangeReceipt`]** so the caller pairs
//   the state change with exactly one outbox record (report §6.3 step 1;
//   see `crate::state_change`).
// - **CRUD + tenant-scoped queries**: `insert` / `update` / `delete` are
//   the write surface, `find_by_id` the point lookup, and
//   `list_for_tenant` the tenant-scoped query. Stage 1 use cases use only
//   insert/find/list; update and delete exist so the port is a complete
//   CRUD contract for the adapter (P1.13) and for later subtasks — no use
//   case in this phase calls them.
//
// Adapter obligations (P1.13): scope every query to the context's tenant,
// refuse cross-tenant writes with `TenantMismatch`, report storage failures
// only as [`RepositoryError::Backend`] with an opaque code + message (never
// raw storage text), and execute the write inside the caller's unit of work
// (the composition root opens one unit of work per use-case invocation; see
// the `EventOutbox` port docs for the binding rule).

use crate::backend_error::BackendError;
use crate::state_change::StateChangeReceipt;
use flexcore_domain::entities::{Asset, Portfolio, Site, Tenant};
use flexcore_domain::ids::{AssetId, PortfolioId, SiteId, TenantId};
use flexcore_domain::tenant_context::TenantContext;

/// Errors returned by the repository ports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepositoryError {
    /// The entity does not exist within the caller's tenant scope. A row
    /// that exists but belongs to a different tenant is deliberately
    /// indistinguishable from a missing one — existence is never leaked
    /// across a tenant boundary (P0.16 rule 5; cross-tenant leakage is a
    /// named §17.4 threat).
    NotFound {
        /// Which entity kind was not found (a [`crate::state_change::entities`]
        /// name).
        entity: &'static str,
    },
    /// The insert/update collided with an existing record on a uniqueness
    /// rule (primary identifier, tenant-scoped natural key). The record was
    /// NOT written.
    Conflict {
        /// Which entity kind collided.
        entity: &'static str,
        /// The uniqueness rule that collided (adapter-supplied, human-safe).
        reason: String,
    },
    /// An explicit entity belonging to a different tenant was supplied to a
    /// write method. The write was refused. Unlike the `NotFound` case the
    /// mismatch is reported directly: the caller already holds the foreign
    /// entity, so there is no existence left to hide.
    TenantMismatch {
        /// Which entity kind was refused.
        entity: &'static str,
    },
    /// The storage backend failed. The record state is the adapter's
    /// transactional obligation (all-or-nothing inside the caller's unit of
    /// work). `code` and `message` are opaque and human-safe — never raw
    /// storage text (see [`BackendError`]).
    Backend(BackendError),
}

impl std::fmt::Display for RepositoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RepositoryError::NotFound { entity } => {
                write!(f, "{entity} not found in the caller's tenant scope")
            }
            RepositoryError::Conflict { entity, reason } => {
                write!(f, "{entity} conflicts with an existing record: {reason}")
            }
            RepositoryError::TenantMismatch { entity } => {
                write!(f, "refused a cross-tenant write on {entity}")
            }
            RepositoryError::Backend(e) => write!(f, "storage backend failure: {e}"),
        }
    }
}

impl std::error::Error for RepositoryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            RepositoryError::Backend(e) => Some(e),
            _ => None,
        }
    }
}

/// The repository port for the **Tenant** aggregate (P0.10 §1 row 1).
///
/// The one repository whose methods carry no `TenantContext`: the tenant
/// registry root is not tenant-owned data — it *is* the tenant (P1.12 §4.1:
/// no `tenant_id` column, no per-tenant row filter to express in Phase 1).
/// Platform-level tenant administration is gated by application
/// authorization (P1.11), not by a context parameter; this exception is
/// recorded in the crate README and applies to this trait only.
pub trait TenantRepository {
    /// Inserts a new tenant. Conflicts if the primary identifier or the
    /// identity-provider reference uniqueness rule (recorded assumption:
    /// one tenant per identity-provider reference) already has a row.
    fn insert(&self, tenant: &Tenant) -> Result<StateChangeReceipt, RepositoryError>;

    /// Updates an existing tenant in place. `NotFound` if the identifier is
    /// unknown.
    fn update(&self, tenant: &Tenant) -> Result<StateChangeReceipt, RepositoryError>;

    /// Looks a tenant up by identifier.
    fn find_by_id(&self, tenant_id: TenantId) -> Result<Option<Tenant>, RepositoryError>;

    /// Looks a tenant up by its identity-provider reference (P0.10 §1 row 1;
    /// the uniqueness anchor for duplicate-tenant detection — recorded
    /// assumption, see the crate README).
    fn find_by_identity_provider_reference(
        &self,
        identity_provider_reference: &str,
    ) -> Result<Option<Tenant>, RepositoryError>;

    /// Lists all tenants (platform-level query; the tenant-scoping rule
    /// does not apply to the root itself).
    fn list_all(&self) -> Result<Vec<Tenant>, RepositoryError>;

    /// Deletes a tenant. Returns whether a row was deleted.
    fn delete(&self, tenant_id: TenantId) -> Result<bool, RepositoryError>;
}

/// The repository port for the **Portfolio** aggregate (P0.10 §1 row 2).
/// Every method takes the caller's `TenantContext` (P0.16 rules 1 and 5).
pub trait PortfolioRepository {
    /// Inserts a new portfolio. Refuses a portfolio whose tenant differs
    /// from the context (`TenantMismatch`); conflicts on the identifier or
    /// the tenant-scoped portfolio-name uniqueness rule (recorded
    /// assumption).
    fn insert(
        &self,
        ctx: &TenantContext,
        portfolio: &Portfolio,
    ) -> Result<StateChangeReceipt, RepositoryError>;

    /// Updates an existing portfolio in place, scoped to the context's
    /// tenant.
    fn update(
        &self,
        ctx: &TenantContext,
        portfolio: &Portfolio,
    ) -> Result<StateChangeReceipt, RepositoryError>;

    /// Looks a portfolio up by identifier within the caller's tenant scope.
    /// A portfolio from another tenant reads as `None` (no existence leak).
    fn find_by_id(
        &self,
        ctx: &TenantContext,
        portfolio_id: PortfolioId,
    ) -> Result<Option<Portfolio>, RepositoryError>;

    /// Lists the caller's tenant's portfolios (the tenant-scoped query).
    fn list_for_tenant(&self, ctx: &TenantContext) -> Result<Vec<Portfolio>, RepositoryError>;

    /// Deletes a portfolio within the caller's tenant scope. Returns
    /// whether a row was deleted; a foreign-tenant row reads as `false`.
    fn delete(
        &self,
        ctx: &TenantContext,
        portfolio_id: PortfolioId,
    ) -> Result<bool, RepositoryError>;
}

/// The repository port for the **Site** aggregate (P0.10 §1 row 3). Every
/// method takes the caller's `TenantContext` (P0.16 rules 1 and 5; rule 7
/// keeps site edge identities tenant-scoped).
pub trait SiteRepository {
    /// Inserts a new site. Refuses a site whose tenant differs from the
    /// context (`TenantMismatch`); conflicts on the identifier or the
    /// tenant-scoped edge-identity rule (P0.16 rule 7).
    fn insert(&self, ctx: &TenantContext, site: &Site) -> Result<StateChangeReceipt, RepositoryError>;

    /// Updates an existing site in place, scoped to the context's tenant.
    fn update(&self, ctx: &TenantContext, site: &Site) -> Result<StateChangeReceipt, RepositoryError>;

    /// Looks a site up by identifier within the caller's tenant scope. A
    /// site from another tenant reads as `None` (no existence leak).
    fn find_by_id(&self, ctx: &TenantContext, site_id: SiteId)
        -> Result<Option<Site>, RepositoryError>;

    /// Lists the caller's tenant's sites (the tenant-scoped query).
    fn list_for_tenant(&self, ctx: &TenantContext) -> Result<Vec<Site>, RepositoryError>;

    /// Deletes a site within the caller's tenant scope. Returns whether a
    /// row was deleted; a foreign-tenant row reads as `false`.
    fn delete(&self, ctx: &TenantContext, site_id: SiteId) -> Result<bool, RepositoryError>;
}

/// The repository port for the **Asset** aggregate (P0.10 §1 row 4). Every
/// method takes the caller's `TenantContext` (P0.16 rules 1 and 5).
pub trait AssetRepository {
    /// Inserts a new asset. Refuses an asset whose tenant differs from the
    /// context (`TenantMismatch`); conflicts on the identifier or the
    /// asset-name-within-site uniqueness rule (recorded assumption).
    fn insert(
        &self,
        ctx: &TenantContext,
        asset: &Asset,
    ) -> Result<StateChangeReceipt, RepositoryError>;

    /// Updates an existing asset in place, scoped to the context's tenant.
    fn update(
        &self,
        ctx: &TenantContext,
        asset: &Asset,
    ) -> Result<StateChangeReceipt, RepositoryError>;

    /// Looks an asset up by identifier within the caller's tenant scope. An
    /// asset from another tenant reads as `None` (no existence leak).
    fn find_by_id(
        &self,
        ctx: &TenantContext,
        asset_id: AssetId,
    ) -> Result<Option<Asset>, RepositoryError>;

    /// Lists the caller's tenant's assets (the tenant-scoped query).
    fn list_for_tenant(&self, ctx: &TenantContext) -> Result<Vec<Asset>, RepositoryError>;

    /// Deletes an asset within the caller's tenant scope. Returns whether a
    /// row was deleted; a foreign-tenant row reads as `false`.
    fn delete(&self, ctx: &TenantContext, asset_id: AssetId) -> Result<bool, RepositoryError>;
}
