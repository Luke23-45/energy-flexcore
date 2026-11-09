// P1.07 — The transactional-outbox port (report §6.3 steps 1–3, §15.5).
//
// The transactional outbox has four steps (report §6.3): (1) write the
// state change AND an outbox record in one storage transaction; (2) publish
// the outbox record to the broker; (3) mark it delivered only after broker
// acknowledgement; (4) make consumers idempotent. This port is the
// application's handle on steps 1–3.
//
// ## Why the port defines its own `OutboxWrite` instead of naming the
// storage/event types
//
// The dependency rule (`tools/check_dependency_rule.py`) allows
// flexcore-ports only the contracts and domain edges; the storage
// `TransactionalWrite` and the event `OutboxRecord` are one edge away. The
// port therefore carries exactly the parts of those types it can name, and
// the adapter maps them 1:1 (P1.13; the mapping is recorded in the crate
// README):
//
//   `OutboxWrite.state_change`  ->  storage state-change descriptor
//   (entity, operation, tenant scope — from the repository receipt)
//   `OutboxWrite.idempotency_key` / `destination_subject` / `payload` /
//   `created_at`  ->  event-crate outbox record fields (payload stored
//   verbatim; the serialized event is produced by the composition root).
//
// The structural half of the rule is preserved: `OutboxWrite::new`
// **consumes** the `StateChangeReceipt` a repository write returned, so an
// outbox record cannot be assembled without the state change having been
// written first.
//
// ## The "caller's unit of work" binding (adapter obligation, binding)
//
// A port value cannot name a transaction, so the binding is by convention,
// stated here as an obligation: the composition root opens **one** unit of
// work (the storage crate's `UnitOfWork`) per use-case invocation and
// obtains unit-of-work-scoped port instances from the adapter; every port
// call inside the invocation — repository writes and outbox appends alike —
// executes inside that one native transaction, all-or-nothing. The
// mechanical enforcement is the P1.13 adapter's atomicity test ("an
// injected failure leaves neither state nor outbox, or both"); the
// in-crate fakes provide the ordering evidence for the application tests
// via the shared write journal (P1.08 acceptance).
//
// ## Publisher side (steps 2–3)
//
// `pending` yields the not-yet-delivered entries in insertion order for the
// outbox publisher (P1.15); `record_attempt` counts a publish attempt;
// `mark_delivered` is called **only after broker acknowledgement**;
// `mark_failed` routes the entry for investigation — a failed entry is
// never silently dropped. Entries are identified by their idempotency key,
// which is unique in the outbox store (P1.12 §4 unique constraint), so
// republishing after a crash between publish and mark is safe.

use crate::backend_error::BackendError;
use crate::state_change::StateChangeReceipt;
use flexcore_contracts::time_model::EventTime;

/// Errors returned when assembling an [`OutboxWrite`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutboxWriteError {
    /// The idempotency key was empty; every outbox record carries a
    /// mandatory, non-empty key (P1.09 envelope rule).
    EmptyIdempotencyKey,
    /// The destination subject was empty.
    EmptyDestinationSubject,
    /// The serialized event payload was empty (P1.09 record rule).
    EmptyPayload,
}

impl std::fmt::Display for OutboxWriteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OutboxWriteError::EmptyIdempotencyKey => {
                write!(f, "the outbox idempotency key is mandatory and must be non-empty")
            }
            OutboxWriteError::EmptyDestinationSubject => {
                write!(f, "the outbox destination subject must be non-empty")
            }
            OutboxWriteError::EmptyPayload => {
                write!(f, "the serialized event payload must not be empty")
            }
        }
    }
}

impl std::error::Error for OutboxWriteError {}

/// One complete transactional write at the port boundary: the state-change
/// half (consumed from the repository receipt) plus the outbox-record half
/// (key, destination subject, serialized payload, creation time). The
/// adapter MUST persist both halves atomically — see the module header for
/// the unit-of-work binding obligation.
///
/// This is the port-side counterpart of the storage `TransactionalWrite`;
/// the mapping is 1:1 (module header).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutboxWrite {
    state_change: StateChangeReceipt,
    idempotency_key: String,
    destination_subject: String,
    payload: String,
    created_at: EventTime,
}

impl OutboxWrite {
    /// Assembles the write from both mandatory halves.
    ///
    /// This is the only constructor: the state-change half comes from the
    /// `StateChangeReceipt` the repository write returned, so an outbox
    /// record cannot exist without its state change (report §6.3 step 1).
    /// The `payload` is the **serialized** event produced by the caller
    /// (the composition root owns the wire rendering; the event schema is
    /// the event crate's, P1.09).
    pub fn new(
        state_change: StateChangeReceipt,
        idempotency_key: String,
        destination_subject: String,
        payload: String,
        created_at: EventTime,
    ) -> Result<Self, OutboxWriteError> {
        if idempotency_key.trim().is_empty() {
            return Err(OutboxWriteError::EmptyIdempotencyKey);
        }
        if destination_subject.trim().is_empty() {
            return Err(OutboxWriteError::EmptyDestinationSubject);
        }
        if payload.is_empty() {
            return Err(OutboxWriteError::EmptyPayload);
        }
        Ok(Self {
            state_change,
            idempotency_key,
            destination_subject,
            payload,
            created_at,
        })
    }

    /// The state-change half (what the adapter must write in the same
    /// transaction).
    pub fn state_change(&self) -> &StateChangeReceipt {
        &self.state_change
    }

    /// The idempotency key (unique in the outbox store; travels as the
    /// broker's deduplication identifier, P1.15).
    pub fn idempotency_key(&self) -> &str {
        &self.idempotency_key
    }

    /// The destination subject for publication.
    pub fn destination_subject(&self) -> &str {
        &self.destination_subject
    }

    /// The serialized event payload (stored verbatim).
    pub fn payload(&self) -> &str {
        &self.payload
    }

    /// When the outbox record was created (UTC; contracts time type).
    pub fn created_at(&self) -> EventTime {
        self.created_at
    }
}

/// One pending outbox entry as the publisher sees it (report §6.3 steps
/// 2–3). A read-model view: the data was validated when the write was
/// appended, so this type carries the fields the publisher needs without
/// re-validating them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingOutboxEntry {
    idempotency_key: String,
    destination_subject: String,
    payload: String,
    attempt_count: u32,
    created_at: EventTime,
}

impl PendingOutboxEntry {
    /// Builds the entry view. Intended for adapter implementations reading
    /// the outbox store.
    pub fn new(
        idempotency_key: String,
        destination_subject: String,
        payload: String,
        attempt_count: u32,
        created_at: EventTime,
    ) -> Self {
        Self {
            idempotency_key,
            destination_subject,
            payload,
            attempt_count,
            created_at,
        }
    }

    /// The idempotency key (unique; the publisher deduplicates on it).
    pub fn idempotency_key(&self) -> &str {
        &self.idempotency_key
    }

    /// The destination subject to publish to.
    pub fn destination_subject(&self) -> &str {
        &self.destination_subject
    }

    /// The serialized event payload to publish.
    pub fn payload(&self) -> &str {
        &self.payload
    }

    /// How many publish attempts were recorded so far.
    pub fn attempt_count(&self) -> u32 {
        self.attempt_count
    }

    /// When the record was created (UTC).
    pub fn created_at(&self) -> EventTime {
        self.created_at
    }
}

/// Errors returned by the [`EventOutbox`] port.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventOutboxError {
    /// An entry with the same idempotency key already exists. The record
    /// was NOT appended (the P1.12 unique constraint is the durable form of
    /// this rule).
    DuplicateKey {
        /// The colliding key.
        idempotency_key: String,
    },
    /// No entry exists for the supplied idempotency key.
    UnknownKey {
        /// The key that matched nothing.
        idempotency_key: String,
    },
    /// The outbox backend failed. The transactional state is the adapter's
    /// obligation (all-or-nothing inside the caller's unit of work). The
    /// code and message are opaque — never raw storage or broker text.
    Backend(BackendError),
}

impl std::fmt::Display for EventOutboxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EventOutboxError::DuplicateKey { idempotency_key } => {
                write!(f, "an outbox entry with key {idempotency_key} already exists")
            }
            EventOutboxError::UnknownKey { idempotency_key } => {
                write!(f, "no outbox entry matches key {idempotency_key}")
            }
            EventOutboxError::Backend(e) => write!(f, "outbox backend failure: {e}"),
        }
    }
}

impl std::error::Error for EventOutboxError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            EventOutboxError::Backend(e) => Some(e),
            _ => None,
        }
    }
}

/// The transactional-outbox port (report §6.3, §15.5; P1.12 §4).
///
/// Adapter obligations (binding; enforced by the P1.13 atomicity test):
///
/// 1. Execute the state change named by [`OutboxWrite::state_change`] and
///    insert the outbox record inside **one** native transaction, within
///    the caller's unit of work (module header). Never split the halves,
///    never write the outbox record "later" from a background task.
/// 2. Roll the whole transaction back on any failure: an injected or real
///    failure leaves **neither** the state change **nor** the outbox
///    record — or, on success, both.
/// 3. Enforce idempotency-key uniqueness (the P1.12 unique constraint is
///    the durable form; the port reports [`EventOutboxError::DuplicateKey`]).
/// 4. `pending` yields entries whose delivery is not yet acknowledged, in
///    insertion order; `mark_delivered` is written only after broker
///    acknowledgement (P1.15); `mark_failed` keeps the entry in the store
///    for investigation — a failed entry is never silently dropped.
pub trait EventOutbox {
    /// Appends one complete transactional write (state change + outbox
    /// record) inside the caller's unit of work.
    fn append(&self, write: OutboxWrite) -> Result<(), EventOutboxError>;

    /// Reads up to `limit` pending (not yet acknowledged) entries in
    /// insertion order, for the outbox publisher.
    fn pending(&self, limit: u32) -> Result<Vec<PendingOutboxEntry>, EventOutboxError>;

    /// Records one publish attempt for the entry with the given key:
    /// increments the attempt count. The delivery state is unchanged.
    fn record_attempt(&self, idempotency_key: &str) -> Result<(), EventOutboxError>;

    /// Marks the entry delivered — **only after broker acknowledgement**
    /// (report §6.3 step 3).
    fn mark_delivered(&self, idempotency_key: &str) -> Result<(), EventOutboxError>;

    /// Marks the entry failed after the publisher's retry policy was
    /// exhausted. The entry stays in the outbox store for investigation.
    fn mark_failed(&self, idempotency_key: &str) -> Result<(), EventOutboxError>;
}
