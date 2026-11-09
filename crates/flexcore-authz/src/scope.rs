// P1.11 — Authorization scopes: tenant, site, asset — every decision is
//         evaluated against an explicit scope.
//
// Ledger P1.11 step 3: "Implement scope types: tenant scope, site scope,
// asset scope; a decision is always evaluated against an explicit scope."
//
// Two rules are carried structurally:
//
// - **No default scope.** There is no `Default`, no `From<TenantId>`, and no
//   "current scope" anywhere in this crate. `PolicySet::evaluate` takes the
//   scope as an explicit parameter, and a request that names no scope
//   denies with [`crate::decision::DenyReason::UnknownScope`].
// - **Every scope carries its tenant.** Evaluation enforces
//   `scope.tenant() == context.tenant()`; a mismatch is a cross-tenant
//   request and denies ([`crate::decision::DenyReason::ScopeDenied`],
//   audited per P0.16 rule 6).
//
// Trust boundary (recorded deliberately): this crate does **not** verify
// that a site or asset actually belongs to the tenant named in the scope —
// that ownership is domain state (P1.06 entities; P0.16 rule 1). Callers
// must resolve scopes from domain/registry data, never from an untrusted
// request field (P0.16 rule 5, the same rule that governs the tenant
// context itself). What this crate enforces is the principal-to-scope
// tenant match; what it assumes is a scope whose entity-to-tenant binding
// came from the domain.

use flexcore_domain::ids::{AssetId, SiteId, TenantId};

/// The scope of a request that operates on a **tenant** (or on the tenant
/// registry itself, e.g. portfolio listing within the acting principal's
/// tenant).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TenantScope {
    tenant: TenantId,
}

/// The scope of a request that operates on a **site**.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SiteScope {
    tenant: TenantId,
    site: SiteId,
}

/// The scope of a request that operates on an **asset**.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AssetScope {
    tenant: TenantId,
    asset: AssetId,
}

impl TenantScope {
    /// Names the tenant the request operates in.
    pub fn new(tenant: TenantId) -> Self {
        Self { tenant }
    }

    /// The tenant this scope belongs to.
    pub fn tenant(&self) -> TenantId {
        self.tenant
    }
}

impl SiteScope {
    /// Names the owning tenant and the site the request operates on.
    pub fn new(tenant: TenantId, site: SiteId) -> Self {
        Self { tenant, site }
    }

    /// The tenant this scope belongs to.
    pub fn tenant(&self) -> TenantId {
        self.tenant
    }

    /// The site this scope targets.
    pub fn site(&self) -> SiteId {
        self.site
    }
}

impl AssetScope {
    /// Names the owning tenant and the asset the request operates on.
    pub fn new(tenant: TenantId, asset: AssetId) -> Self {
        Self { tenant, asset }
    }

    /// The tenant this scope belongs to.
    pub fn tenant(&self) -> TenantId {
        self.tenant
    }

    /// The asset this scope targets.
    pub fn asset(&self) -> AssetId {
        self.asset
    }
}

/// The scope level of a request or of a policy rule.
///
/// Scope levels do **not** inherit: a rule granted at one level matches only
/// that level. A tenant-level grant does not authorize a site- or
/// asset-level request; each level needs its own explicit rule (deny-by-
/// default, no implicit hierarchy).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScopeKind {
    /// Tenant-level scope.
    Tenant,
    /// Site-level scope.
    Site,
    /// Asset-level scope.
    Asset,
}

impl ScopeKind {
    /// Stable lowercase id, suitable for audit records and rule references.
    pub fn id(self) -> &'static str {
        match self {
            ScopeKind::Tenant => "tenant",
            ScopeKind::Site => "site",
            ScopeKind::Asset => "asset",
        }
    }
}

/// The explicit scope a decision is evaluated against: exactly one of
/// tenant, site, or asset. There is no "unscoped" variant — a request with
/// no scope never reaches this type; it denies (`UnknownScope`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Scope {
    /// A tenant-level request.
    Tenant(TenantScope),
    /// A site-level request.
    Site(SiteScope),
    /// An asset-level request.
    Asset(AssetScope),
}

impl Scope {
    /// The tenant this scope belongs to — the value evaluation matches
    /// against the context's tenant.
    pub fn tenant(&self) -> TenantId {
        match self {
            Scope::Tenant(scope) => scope.tenant(),
            Scope::Site(scope) => scope.tenant(),
            Scope::Asset(scope) => scope.tenant(),
        }
    }

    /// The scope level of this scope.
    pub fn kind(&self) -> ScopeKind {
        match self {
            Scope::Tenant(_) => ScopeKind::Tenant,
            Scope::Site(_) => ScopeKind::Site,
            Scope::Asset(_) => ScopeKind::Asset,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flexcore_contracts::ids::FlexUuid;

    const UUID_A: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a8b";
    const UUID_B: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a8c";

    fn uuid(s: &str) -> FlexUuid {
        FlexUuid::parse(s).unwrap()
    }

    #[test]
    fn scopes_carry_their_tenant_and_target() {
        let tenant = TenantId::new(uuid(UUID_A));
        let site = SiteId::new(uuid(UUID_B));
        let asset = AssetId::new(uuid(UUID_B));

        let t = TenantScope::new(tenant);
        assert_eq!(t.tenant(), tenant);

        let s = SiteScope::new(tenant, site);
        assert_eq!(s.tenant(), tenant);
        assert_eq!(s.site(), site);

        let a = AssetScope::new(tenant, asset);
        assert_eq!(a.tenant(), tenant);
        assert_eq!(a.asset(), asset);
    }

    #[test]
    fn scope_exposes_kind_and_tenant_uniformly() {
        let tenant = TenantId::new(uuid(UUID_A));
        let site = SiteId::new(uuid(UUID_B));
        let asset = AssetId::new(uuid(UUID_B));

        assert_eq!(Scope::Tenant(TenantScope::new(tenant)).kind(), ScopeKind::Tenant);
        assert_eq!(Scope::Site(SiteScope::new(tenant, site)).kind(), ScopeKind::Site);
        assert_eq!(Scope::Asset(AssetScope::new(tenant, asset)).kind(), ScopeKind::Asset);

        assert_eq!(Scope::Tenant(TenantScope::new(tenant)).tenant(), tenant);
        assert_eq!(Scope::Site(SiteScope::new(tenant, site)).tenant(), tenant);
        assert_eq!(Scope::Asset(AssetScope::new(tenant, asset)).tenant(), tenant);
    }

    #[test]
    fn scope_kinds_are_distinct_and_have_stable_ids() {
        assert_ne!(ScopeKind::Tenant, ScopeKind::Site);
        assert_ne!(ScopeKind::Site, ScopeKind::Asset);
        assert_ne!(ScopeKind::Tenant, ScopeKind::Asset);
        assert_eq!(ScopeKind::Tenant.id(), "tenant");
        assert_eq!(ScopeKind::Site.id(), "site");
        assert_eq!(ScopeKind::Asset.id(), "asset");
    }

    #[test]
    fn scopes_of_different_levels_over_the_same_uuid_do_not_unify() {
        // The scope types are typed over the distinct domain ID newtypes, so
        // a SiteId and an AssetId wrapping the same UUID can never be
        // confused — same primitive-obsession guard as the domain crate.
        let tenant = TenantId::new(uuid(UUID_A));
        let same_uuid = uuid(UUID_B);
        let site_scope = SiteScope::new(tenant, SiteId::new(same_uuid));
        let asset_scope = AssetScope::new(tenant, AssetId::new(same_uuid));

        assert_eq!(Scope::Site(site_scope).kind(), ScopeKind::Site);
        assert_eq!(Scope::Asset(asset_scope).kind(), ScopeKind::Asset);
        assert_ne!(Scope::Site(site_scope), Scope::Asset(asset_scope));
    }
}
