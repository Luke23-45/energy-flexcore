// P1.05 — Typed tenant-scoped identifiers (added during the P1.05/P1.09
//         restructure; P1.06 will consume them for the §10.1 entities).
// P1.06 — `CommandId` and `ScheduleId` added for the §10.1 DispatchCommand
//         and Schedule entities (`dispatch_command.rs`); same wrapper rule.
//
// Each entity identifier wraps `flexcore_contracts::ids::FlexUuid` in a
// distinct newtype so a bare UUID can never be confused between entity kinds
// (primitive-obsession guard, report §8.1 / P1.05 risk note). There is no
// `From<FlexUuid>` auto-conversion back into the generic type — access goes
// through the explicit `uuid()` accessor — and no two wrapper types unify.
//
// These types carry no semantics beyond identity: tenant scoping, ownership,
// and lifecycle live in the domain entities (P1.06) and the tenant-context
// type (P0.16 / P1.11). No ID generation happens here: generation is behind
// the application-layer id-generation port (P1.07), per the contracts crate
// `ids` module rule.
//
// Serialization note: the domain crate deliberately takes no serde
// dependency (dependency rule, `tools/check_dependency_rule.py`; ledger
// P1.06). The event crate (flexcore-events, P1.09) serialises these IDs by
// delegating to `FlexUuid`'s serde impl through field-level helpers.

use flexcore_contracts::ids::FlexUuid;

macro_rules! typed_id {
    (
        $(#[$meta:meta])*
        $name:ident
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(FlexUuid);

        impl $name {
            /// Wraps the given `FlexUuid` in this typed identifier.
            pub const fn new(uuid: FlexUuid) -> Self {
                Self(uuid)
            }

            /// The underlying `FlexUuid`.
            pub const fn uuid(self) -> FlexUuid {
                self.0
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                // Canonical hyphenated form of the wrapped UUID.
                write!(f, "{}", self.0)
            }
        }
    };
}

typed_id! {
    /// Identifier of a **tenant** (P0.10/P0.16 vocabulary). Every
    /// tenant-owned entity carries a `tenant_id`.
    TenantId
}

typed_id! {
    /// Identifier of a **portfolio** (P0.10 vocabulary).
    PortfolioId
}

typed_id! {
    /// Identifier of a **site** (P0.10 vocabulary).
    SiteId
}

typed_id! {
    /// Identifier of an **asset** (P0.10 vocabulary).
    AssetId
}

typed_id! {
    /// Identifier of a **dispatch command** (P0.10 §1 DispatchCommand row;
    /// report §14.3 "unique command ID"). Added in P1.06.
    CommandId
}

typed_id! {
    /// Identifier of a **schedule** (P0.10 §1 Schedule row; the dispatch
    /// command carries the source schedule ID and revision, report §14.3).
    /// Added in P1.06.
    ScheduleId
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "0192a5e0-8c3d-7e2f-9a1b-3c4d5e6f7a8b";

    #[test]
    fn typed_ids_wrap_and_expose_the_uuid() {
        let uuid = FlexUuid::parse(SAMPLE).unwrap();
        let tenant = TenantId::new(uuid);
        assert_eq!(tenant.uuid(), uuid);
        assert_eq!(tenant.to_string(), SAMPLE);
    }

    #[test]
    fn typed_ids_are_distinct_types_over_the_same_uuid() {
        // The whole point of the wrappers: the same 128-bit value wrapped in
        // TenantId vs SiteId can never be interchanged — the types do not
        // unify, and no From/Into exists between them.
        let uuid = FlexUuid::parse(SAMPLE).unwrap();
        let tenant = TenantId::new(uuid);
        let site = SiteId::new(uuid);
        assert_ne!(
            std::any::type_name::<TenantId>(),
            std::any::type_name::<SiteId>(),
            "TenantId and SiteId must remain distinct types"
        );
        assert_eq!(tenant.uuid(), site.uuid());
    }

    #[test]
    fn typed_ids_delegate_equality_to_the_wrapped_uuid() {
        let a = FlexUuid::parse(SAMPLE).unwrap();
        let b = FlexUuid::nil();
        assert_eq!(TenantId::new(a), TenantId::new(a));
        assert_ne!(TenantId::new(a), TenantId::new(b));
        assert_ne!(PortfolioId::new(a), SiteId::new(a));
        assert_ne!(SiteId::new(a), AssetId::new(a));
    }
}
