// P1.10 — TransactionalWrite: the "state change + outbox record in ONE
//         transaction" rule expressed as a type the adapter must honour.
//
// Part of the flexcore-storage P1.10 deliverable (report §6.3 step 1, §15.5;
// P0.14/P1.09 outbox record; ledger P1.12 §4.6 outbox table design; ledger
// P1.13 step 4 atomicity test). The transactional outbox (report §6.3) has
// four steps: (1) write the state change AND an outbox record in one
// PostgreSQL transaction; (2) publish the outbox record; (3) mark delivered
// only after broker acknowledgement; (4) idempotent consumers. Step 1 is the
// dual-write hole this module addresses: a state change persisted without its
// outbox record loses the event; an outbox record without its state change
// announces something that never happened.
//
// What this module can and cannot do:
//
// - It CAN make the partial write unrepresentable at construction:
//   [`TransactionalWrite`] has private fields and exactly one constructor,
//   which requires both the [`StateChangeDescriptor`] and the
//   [`flexcore_events::outbox::OutboxRecord`]. There is no way to build a
//   `TransactionalWrite` carrying only one part.
// - It CANNOT enforce atomicity. A type cannot force the adapter to run two
//   writes inside one native transaction. That obligation is stated in the
//   documentation on the type itself (an adapter implementer reading the
//   type sees it), and the mechanical enforcement is the P1.13 PostgreSQL
//   adapter's atomicity test — ledger P1.13 step 4: "Implement the outbox
//   write: state change + outbox insert in one transaction
//   (`sqlx::Transaction`), with a test proving atomicity (state without
//   outbox, and outbox without state, are both impossible in the test's
//   failure injection)" and the matching acceptance criterion "Outbox
//   atomicity test passes (injected failure leaves neither state nor outbox,
//   or both)". Cite P1.13 when reviewing; this crate is only half of the
//   evidence.

use flexcore_domain::ids::TenantId;
use flexcore_events::outbox::OutboxRecord;
use serde::{Deserialize, Serialize};

/// Errors returned when building a [`StateChangeDescriptor`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateChangeError {
    /// The entity name was empty or all whitespace; a state-change
    /// descriptor must name the entity (table or aggregate) it writes.
    EmptyEntity,
}

impl std::fmt::Display for StateChangeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StateChangeError::EmptyEntity => {
                write!(f, "the state-change entity name must be non-empty")
            }
        }
    }
}

impl std::error::Error for StateChangeError {}

/// The kind of state change a descriptor names. Current-state tables are
/// updated in place (report §6.5 via P0.16 §5); deletes are legitimate for
/// working data, never for the append-oriented audit history (that is the
/// flexcore-audit port's rule, not a storage-operation question).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StateChangeOperation {
    /// A new row / new aggregate state is inserted.
    Insert,
    /// An existing row / aggregate state is updated in place.
    Update,
    /// An existing row is deleted (working data only; never the audit
    /// history — see the flexcore-audit append-only rule).
    Delete,
}

/// Describes the state-change half of a transactional write: which entity
/// (Stage 1 table or aggregate) is written, with which operation, and for
/// which tenant.
///
/// The tenant scope uses the typed [`TenantId`] (P0.16 rule 1: `tenant_id`
/// is part of every tenant-owned record and query scope). `None` is
/// legitimate only for the genuinely tenant-less writes — the `tenants`
/// table itself and the outbox working queue (P1.12 §2.1) — never as a way
/// to omit scoping on a tenant-owned table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateChangeDescriptor {
    entity: String,
    operation: StateChangeOperation,
    tenant_scope: Option<TenantId>,
}

impl StateChangeDescriptor {
    /// Builds a descriptor, rejecting an empty entity name.
    pub fn new(
        entity: String,
        operation: StateChangeOperation,
        tenant_scope: Option<TenantId>,
    ) -> Result<Self, StateChangeError> {
        if entity.trim().is_empty() {
            return Err(StateChangeError::EmptyEntity);
        }
        Ok(Self {
            entity,
            operation,
            tenant_scope,
        })
    }

    /// The entity being written (e.g. `"assets"`, `"sites"`).
    pub fn entity(&self) -> &str {
        &self.entity
    }

    /// The operation applied to the entity.
    pub fn operation(&self) -> StateChangeOperation {
        self.operation
    }

    /// The tenant scope of the write, if it is tenant-owned.
    pub fn tenant_scope(&self) -> Option<TenantId> {
        self.tenant_scope
    }
}

/// One transactional write: a state change and its outbox record, which the
/// adapter MUST persist atomically — in ONE native transaction (report §6.3
/// step 1, §15.5).
///
/// ## Adapter obligation (binding, not advisory)
///
/// A persistence adapter that accepts a `TransactionalWrite` MUST:
///
/// 1. execute the state change and insert the outbox record inside **one**
///    native transaction, driven through a single
///    [`crate::unit_of_work::UnitOfWork`] boundary;
/// 2. never split the two writes across transactions, never write the
///    outbox record "later" from a background task, and never swallow one
///    part while committing the other;
/// 3. roll the whole transaction back on any failure, so an injected or
///    real failure leaves **neither** the state change **nor** the outbox
///    record — or, on success, both.
///
/// ## Why a type alone is not the enforcement
///
/// Atomicity is a property of the adapter's execution, not of this value;
/// the type only removes the unrepresentable partial value at construction
/// (private fields, one constructor requiring both parts). The mechanical
/// enforcement is the P1.13 PostgreSQL adapter atomicity test (ledger P1.13
/// step 4 and acceptance criterion: an injected failure leaves neither state
/// nor outbox, or both). Adapters targeting other stores must provide the
/// equivalent test; reviewers should reject an adapter that accepts this
/// type without one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransactionalWrite {
    state_change: StateChangeDescriptor,
    outbox: OutboxRecord,
}

impl TransactionalWrite {
    /// Builds a transactional write from both mandatory parts.
    ///
    /// This is the only constructor: the fields are private, so a
    /// `TransactionalWrite` cannot exist without a state change **and** an
    /// outbox record. The outbox record is the real
    /// [`flexcore_events::outbox::OutboxRecord`] (idempotency key,
    /// destination subject, serialized payload), built by the caller per the
    /// P1.09 rules.
    pub fn new(state_change: StateChangeDescriptor, outbox: OutboxRecord) -> Self {
        Self {
            state_change,
            outbox,
        }
    }

    /// The state-change half (what the adapter must write).
    pub fn state_change(&self) -> &StateChangeDescriptor {
        &self.state_change
    }

    /// The outbox half (what the adapter must insert into the outbox store
    /// in the same transaction).
    pub fn outbox(&self) -> &OutboxRecord {
        &self.outbox
    }

    /// Consumes the write and yields both parts, for adapters that write
    /// them positionally inside one transaction. Both parts are yielded
    /// together — the P1.13 atomicity test covers the adapter code that
    /// destructures this pair.
    pub fn into_parts(self) -> (StateChangeDescriptor, OutboxRecord) {
        (self.state_change, self.outbox)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flexcore_contracts::ids::FlexUuid;
    use flexcore_events::envelope::IdempotencyKey;
    use flexcore_events::outbox::DeliveryState;
    use flexcore_events::subject::Subject;
    use time::OffsetDateTime;

    fn utc_millis(millis: i64) -> OffsetDateTime {
        OffsetDateTime::from_unix_timestamp_nanos(i128::from(millis) * 1_000_000)
            .expect("valid UTC timestamp")
    }

    fn sample_tenant() -> TenantId {
        let mut bytes = [0u8; 16];
        bytes[15] = 1;
        TenantId::new(FlexUuid::from_bytes(bytes))
    }

    fn sample_outbox_record() -> OutboxRecord {
        OutboxRecord::new(
            IdempotencyKey::new("asset-registered:seq-7".to_string()).expect("valid key"),
            Subject::new("flexcore.v1.registry.asset_registered".to_string())
                .expect("valid subject"),
            "{\"serialized\": true}".to_string(),
            utc_millis(1_000),
        )
        .expect("valid outbox record")
    }

    #[test]
    fn construction_requires_both_parts_and_yields_them_back() {
        // Private fields mean struct-literal syntax is unavailable outside
        // this module and there is no one-part constructor: `new` with both
        // parts is the only way in. This test exercises that constructor and
        // asserts both parts survive.
        let descriptor =
            StateChangeDescriptor::new("assets".to_string(), StateChangeOperation::Insert, None)
                .unwrap();
        let record = sample_outbox_record();
        let write = TransactionalWrite::new(descriptor.clone(), record.clone());

        assert_eq!(write.state_change(), &descriptor);
        assert_eq!(write.outbox(), &record);
        assert_eq!(write.state_change().entity(), "assets");
        assert_eq!(
            write.state_change().operation(),
            StateChangeOperation::Insert
        );
        assert_eq!(write.outbox().delivery_state(), DeliveryState::Pending);
    }

    #[test]
    fn into_parts_yields_both_parts_together() {
        let descriptor = StateChangeDescriptor::new(
            "portfolios".to_string(),
            StateChangeOperation::Update,
            Some(sample_tenant()),
        )
        .unwrap();
        let record = sample_outbox_record();
        let (entity, outbox) = TransactionalWrite::new(descriptor, record.clone()).into_parts();
        assert_eq!(entity.entity(), "portfolios");
        assert!(entity.tenant_scope().is_some());
        assert_eq!(outbox, record);
    }

    #[test]
    fn descriptor_rejects_an_empty_entity_name() {
        assert_eq!(
            StateChangeDescriptor::new(String::new(), StateChangeOperation::Insert, None),
            Err(StateChangeError::EmptyEntity)
        );
        assert_eq!(
            StateChangeDescriptor::new("   ".to_string(), StateChangeOperation::Insert, None),
            Err(StateChangeError::EmptyEntity)
        );
    }

    #[test]
    fn descriptor_carries_the_tenant_scope_when_tenant_owned() {
        let tenant = sample_tenant();
        let descriptor = StateChangeDescriptor::new(
            "sites".to_string(),
            StateChangeOperation::Insert,
            Some(tenant),
        )
        .unwrap();
        assert_eq!(descriptor.tenant_scope(), Some(tenant));
        // Tenant-less writes (the tenants root table, the outbox queue).
        let descriptor =
            StateChangeDescriptor::new("tenants".to_string(), StateChangeOperation::Insert, None)
                .unwrap();
        assert_eq!(descriptor.tenant_scope(), None);
    }

    #[test]
    fn operation_serde_uses_the_frozen_names() {
        for op in [
            StateChangeOperation::Insert,
            StateChangeOperation::Update,
            StateChangeOperation::Delete,
        ] {
            let back: StateChangeOperation =
                serde_json::from_value(serde_json::to_value(&op).unwrap()).unwrap();
            assert_eq!(back, op);
        }
        assert!(
            serde_json::from_value::<StateChangeOperation>(serde_json::json!("UPSERT")).is_err(),
            "no invented operation may deserialize"
        );
    }
}
