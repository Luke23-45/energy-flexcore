// P1.07 — State-change vocabulary shared by the write ports.
//
// The transactional-outbox rule (report §6.3 step 1, §15.5; P0.14 §4) is:
// write the state change AND an outbox record in ONE transaction. The
// storage crate expresses that rule as its `TransactionalWrite` type (a
// state-change descriptor plus an outbox record); the event crate owns the
// outbox record type. This crate depends on neither — the dependency rule
// (`tools/check_dependency_rule.py`) allows flexcore-ports only the
// contracts and domain edges — so the ports define the minimal port-side
// vocabulary that carries the same information, and the adapter maps it 1:1
// onto the real types (P1.13; the mapping is recorded in the crate README):
//
//   flexcore_ports::state_change::StateChangeOperation
//       <-> flexcore_storage::outbox_write::StateChangeOperation
//   flexcore_ports::state_change::StateChangeReceipt
//       <-> (the descriptor half of) flexcore_storage::outbox_write::StateChangeDescriptor
//   flexcore_ports::event_outbox::OutboxWrite
//       <-> flexcore_storage::outbox_write::TransactionalWrite
//
// The structural property is preserved at the port boundary: a repository
// write returns a [`StateChangeReceipt`], and [`crate::event_outbox::OutboxWrite::new`]
// consumes it — an outbox record cannot even be assembled without the state
// change having been written first. What a value still cannot force is the
// adapter executing both halves inside one native transaction; that
// obligation is documented on the `EventOutbox` port and mechanically
// enforced by the P1.13 adapter's atomicity test.

use flexcore_domain::ids::TenantId;

/// The Stage 1 canonical entity (table) names the state-change descriptors
/// use, matching the P1.12 §3/§4 registry table names. Adapters map them
/// onto their own store's physical naming; the constants exist so ports,
/// fakes, and use cases share one spelling.
pub mod entities {
    /// The tenant registry root (P1.12 §4.1).
    pub const TENANTS: &str = "tenants";
    /// The portfolio registry (P1.12 §4.2).
    pub const PORTFOLIOS: &str = "portfolios";
    /// The site registry (P1.12 §4.3).
    pub const SITES: &str = "sites";
    /// The asset registry (P1.12 §4.4).
    pub const ASSETS: &str = "assets";
}

/// The kind of state change a write port performed. Mirrors the storage
/// crate's operation vocabulary 1:1 (adapter maps it, P1.13). Deletes are
/// legitimate for working data, never for the append-oriented audit
/// history — that rule belongs to the audit crate's append-only port, not
/// to this vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StateChangeOperation {
    /// A new record / aggregate state was inserted.
    Insert,
    /// An existing record / aggregate state was updated in place.
    Update,
    /// An existing record was deleted (working data only).
    Delete,
}

/// What a repository write recorded: which entity, with which operation,
/// and for which tenant scope. Returned by every write method on the
/// repository ports; the caller pairs it with exactly one outbox record via
/// [`crate::event_outbox::OutboxWrite::new`].
///
/// The tenant scope uses the typed [`TenantId`]. `None` is legitimate only
/// for the genuinely tenant-less writes — the tenant registry root itself —
/// never as a way to omit scoping on a tenant-owned entity (P0.16 rule 1;
/// the same rule the storage descriptor encodes).
///
/// `#[must_use]`: dropping a receipt silently is a design decision that
/// should be visible (`let _ = receipt;`) — every Stage 1 use case pairs
/// its receipt with exactly one outbox record.
#[must_use = "a state change must be paired with exactly one outbox record (report §6.3 step 1) or explicitly dropped"]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateChangeReceipt {
    entity: String,
    operation: StateChangeOperation,
    tenant_scope: Option<TenantId>,
}

impl StateChangeReceipt {
    /// Builds a receipt from the recorded state change. Intended for
    /// repository implementations (adapters and test fakes).
    pub fn new(entity: String, operation: StateChangeOperation, tenant_scope: Option<TenantId>) -> Self {
        Self {
            entity,
            operation,
            tenant_scope,
        }
    }

    /// The entity the state change was recorded against (one of the
    /// [`entities`] names for the Stage 1 registry).
    pub fn entity(&self) -> &str {
        &self.entity
    }

    /// The operation that was applied.
    pub fn operation(&self) -> StateChangeOperation {
        self.operation
    }

    /// The tenant scope of the write, if the entity is tenant-owned.
    pub fn tenant_scope(&self) -> Option<TenantId> {
        self.tenant_scope
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flexcore_contracts::ids::FlexUuid;

    fn tenant(tag: u8) -> TenantId {
        let mut bytes = [0u8; 16];
        bytes[15] = tag;
        TenantId::new(FlexUuid::from_bytes(bytes))
    }

    #[test]
    fn receipt_carries_entity_operation_and_scope() {
        let receipt = StateChangeReceipt::new(
            entities::ASSETS.to_string(),
            StateChangeOperation::Insert,
            Some(tenant(1)),
        );
        assert_eq!(receipt.entity(), "assets");
        assert_eq!(receipt.operation(), StateChangeOperation::Insert);
        assert_eq!(receipt.tenant_scope(), Some(tenant(1)));
    }

    #[test]
    fn tenant_less_writes_carry_no_scope() {
        // The tenant registry root is the one Stage 1 entity without a
        // tenant scope (P1.12 §4.1).
        let receipt =
            StateChangeReceipt::new(entities::TENANTS.to_string(), StateChangeOperation::Insert, None);
        assert_eq!(receipt.entity(), "tenants");
        assert_eq!(receipt.tenant_scope(), None);
    }

    #[test]
    fn entity_names_match_the_stage_1_registry_tables() {
        assert_eq!(entities::TENANTS, "tenants");
        assert_eq!(entities::PORTFOLIOS, "portfolios");
        assert_eq!(entities::SITES, "sites");
        assert_eq!(entities::ASSETS, "assets");
    }
}
