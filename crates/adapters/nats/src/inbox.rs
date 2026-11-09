// P1.16 — The inbox store port (consumer side) and an in-memory
// implementation.
//
// The inbox pattern (report §6.3 step 4, §6.4 guardrail; P1.16): every
// consumer with side effects keeps inbox records keyed by the message
// idempotency key, per consumer identity, with a UNIQUE constraint in the
// durable store. Redeliveries — which at-least-once delivery makes normal,
// not exceptional — are detected via the inbox and acknowledged without
// re-executing side effects. The broker duplicate window (P1.14 §2) is a
// courtesy; the inbox is the durable guarantee.
//
// # Recorded single-home decision
//
// The `InboxStore` trait lives HERE (the NATS adapter crate), not in
// flexcore-ports: (a) flexcore-ports is outside this subtask's edit scope;
// (b) the inbox is not a domain capability — it is consumer-side
// delivery-deduplication infrastructure, and its durable home is the
// PostgreSQL schema the postgres adapter owns (P1.12/P1.13; P1.16 step 1
// adds the inbox table with the unique constraint, consumed-at timestamp,
// and consumer identity). The durable implementation therefore lands in
// `crates/adapters/postgres` (flexcore-postgres-adapter) and implements
// exactly this trait. If a later phase promotes the trait to flexcore-ports
// (it is the same shape as the other ports), this definition must move
// verbatim and be deleted here — one home, one definition (P1.10 step 1
// single-home rule).
//
// # The transactional obligation (binding on handlers and the durable
// store)
//
// "Record inbox + state change in ONE transaction" (P1.16 step 2) cannot be
// expressed inside a sync trait method — a port value cannot name a
// transaction. The obligation is by convention, exactly like the
// flexcore-ports "caller's unit of work" binding (P1.07):
//
// - the HANDLER opens its own storage transaction, applies its state
//   change, calls [`InboxStore::record`] (or receives
//   [`RecordOutcome::AlreadyRecorded`] and rolls back), and commits;
// - the store's `record` must fail (or report `AlreadyRecorded`) on a
//   duplicate key so a concurrent duplicate delivery rolls back its
//   transaction instead of double-executing;
// - the consumer helper ([`crate::consumer`]) acknowledges only after the
//   handler returns — i.e. after the commit — so a crash between commit and
//   ack redelivers safely into the inbox check.
//
// The in-memory implementation below is a TEST/DEV double in the spirit of
// flexcore-ports::fakes: it must never be wired into a deployment. It
// honours the insert-if-absent semantics (`AlreadyRecorded`) but of course
// does not persist.

use std::collections::HashSet;
use std::sync::Mutex;

/// Outcome of a duplicate-safe inbox record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordOutcome {
    /// The record was inserted; the handler's transaction may commit.
    Inserted,
    /// The key was already recorded (by this delivery loop or a concurrent
    /// duplicate). The caller MUST treat this as "do not execute the side
    /// effect": in a transactional handler, roll back.
    AlreadyRecorded,
}

/// Inbox store failures. The durable implementation maps its backend error
/// into the opaque message here (never raw storage text at any API boundary
/// above this crate).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InboxStoreError {
    /// The backend failed; message is opaque.
    Backend(String),
}

impl std::fmt::Display for InboxStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InboxStoreError::Backend(message) => write!(f, "inbox store failure: {message}"),
        }
    }
}

impl std::error::Error for InboxStoreError {}

/// The inbox-store port (consumer side; recorded single-home decision in
/// the module header). Sync, like every port in this codebase — the
/// consumer helper calls it between async broker operations.
pub trait InboxStore: Send + Sync {
    /// Whether this consumer already recorded (and therefore already
    /// executed the side effect for) the message with this idempotency key.
    fn is_recorded(
        &self,
        consumer_name: &str,
        idempotency_key: &str,
    ) -> Result<bool, InboxStoreError>;

    /// Records processing of one message, duplicate-safely: exactly one
    /// caller gets [`RecordOutcome::Inserted`], every other caller for the
    /// same `(consumer_name, idempotency_key)` gets
    /// [`RecordOutcome::AlreadyRecorded`]. MUST be called inside the same
    /// storage transaction as the handler's state change (module header).
    /// The durable implementation stamps the consumed-at timestamp and the
    /// consumer identity (P1.16 step 1) — the trait carries the key only,
    /// because that is all the dedup decision needs.
    fn record(
        &self,
        consumer_name: &str,
        idempotency_key: &str,
    ) -> Result<RecordOutcome, InboxStoreError>;
}

/// In-memory [`InboxStore`] — TEST/DEV double, never a deployment wiring
/// (same status as flexcore-ports::fakes). Insert-if-absent semantics via a
/// mutex-guarded set; nothing persists.
#[derive(Default)]
pub struct InMemoryInboxStore {
    recorded: Mutex<HashSet<(String, String)>>,
}

impl InMemoryInboxStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// How many records are held (test inspection helper).
    pub fn len(&self) -> usize {
        self.recorded.lock().expect("poisoned mutex in test double").len()
    }

    /// Whether the store holds no records.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl InboxStore for InMemoryInboxStore {
    fn is_recorded(
        &self,
        consumer_name: &str,
        idempotency_key: &str,
    ) -> Result<bool, InboxStoreError> {
        let recorded = self.recorded.lock().expect("poisoned mutex in test double");
        Ok(recorded.contains(&(consumer_name.to_string(), idempotency_key.to_string())))
    }

    fn record(
        &self,
        consumer_name: &str,
        idempotency_key: &str,
    ) -> Result<RecordOutcome, InboxStoreError> {
        let mut recorded = self.recorded.lock().expect("poisoned mutex in test double");
        if recorded.insert((consumer_name.to_string(), idempotency_key.to_string())) {
            Ok(RecordOutcome::Inserted)
        } else {
            Ok(RecordOutcome::AlreadyRecorded)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_memory_inbox_is_duplicate_safe_per_consumer() {
        let inbox = InMemoryInboxStore::new();
        assert!(!inbox.is_recorded("audit-persist-v1", "key-1").unwrap());

        assert_eq!(
            inbox.record("audit-persist-v1", "key-1").unwrap(),
            RecordOutcome::Inserted
        );
        // Same consumer + key → AlreadyRecorded (the unique-constraint
        // semantic the durable store enforces).
        assert_eq!(
            inbox.record("audit-persist-v1", "key-1").unwrap(),
            RecordOutcome::AlreadyRecorded
        );
        assert!(inbox.is_recorded("audit-persist-v1", "key-1").unwrap());

        // A different consumer records the same key independently: inbox
        // records are per consumer identity.
        assert_eq!(
            inbox.record("registry-projection-v1", "key-1").unwrap(),
            RecordOutcome::Inserted
        );
        assert_eq!(inbox.len(), 2);
    }
}
