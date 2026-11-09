// P1.07 — The ID-generation port (P1.07 step 2; contracts `ids` rule).
//
// The contracts crate provides the identifier **representation** only
// (`FlexUuid` over `[u8; 16]`; parse/format/nil): "ID *generation* (random
// v4 or time-ordered v7) requires entropy and a version policy; that is an
// application-layer concern behind the id-generation port defined in
// P1.07." This is that port. The domain typed identifiers wrap `FlexUuid`
// (no bare-UUID confusion); the port hands out both the raw form and the
// typed wrappers so call sites never unwrap and re-wrap by hand.
//
// Version policy (recorded): v4 versus v7 is the adapter's decision, made
// once per deployment and recorded with the storage design (P1.12 §2 —
// the store's identifiers should match the generator's version policy).
// The port deliberately exposes nothing but "a new identifier": no
// entropy, no clock, no policy lives here.

use flexcore_contracts::ids::FlexUuid;
use flexcore_domain::ids::{AssetId, PortfolioId, SiteId, TenantId};

/// The identifier generator the application layer uses for every new
/// entity and event correlation identifier. Test determinism: the in-crate
/// fake ([`crate::fakes::CountingIdGenerator`]) yields a deterministic,
/// strictly increasing sequence and counts every generation.
pub trait IdGenerator {
    /// A fresh identifier. Implementations must never return the nil UUID
    /// and must never repeat an identifier within a deployment's lifetime.
    fn new_uuid(&self) -> FlexUuid;

    /// A fresh typed tenant identifier.
    fn new_tenant_id(&self) -> TenantId {
        TenantId::new(self.new_uuid())
    }

    /// A fresh typed portfolio identifier.
    fn new_portfolio_id(&self) -> PortfolioId {
        PortfolioId::new(self.new_uuid())
    }

    /// A fresh typed site identifier.
    fn new_site_id(&self) -> SiteId {
        SiteId::new(self.new_uuid())
    }

    /// A fresh typed asset identifier.
    fn new_asset_id(&self) -> AssetId {
        AssetId::new(self.new_uuid())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal inline implementation proving the default typed-ID helpers
    /// work over any `new_uuid` and never produce nil identifiers.
    struct SequenceGenerator {
        next: std::cell::Cell<u128>,
    }

    impl IdGenerator for SequenceGenerator {
        fn new_uuid(&self) -> FlexUuid {
            let n = self.next.get();
            self.next.set(n + 1);
            FlexUuid::from_bytes(n.to_be_bytes())
        }
    }

    #[test]
    fn typed_helpers_wrap_the_generated_uuid() {
        let gen = SequenceGenerator { next: std::cell::Cell::new(7) };
        let id = gen.new_tenant_id();
        assert_eq!(id.uuid(), FlexUuid::from_bytes(7u128.to_be_bytes()));
        assert_ne!(id.uuid(), FlexUuid::NIL);
    }

    #[test]
    fn typed_helpers_are_distinct_types_over_distinct_values() {
        let gen = SequenceGenerator { next: std::cell::Cell::new(1) };
        let t = gen.new_tenant_id();
        let p = gen.new_portfolio_id();
        let s = gen.new_site_id();
        let a = gen.new_asset_id();
        // Each call advances the underlying sequence, so the identifiers
        // differ, and the wrapper types never unify.
        assert_ne!(t.uuid(), p.uuid());
        assert_ne!(p.uuid(), s.uuid());
        assert_ne!(s.uuid(), a.uuid());
    }
}
