// P1.10 — UnitOfWork: a plain, trait-agnostic transactional-boundary type.
//
// Part of the flexcore-storage P1.10 deliverable (report §6.3, §16.1; ledger
// P1.10 deliverable A). This type is deliberately **not** a port and **not**
// a transaction: it carries no async, no SQL, no connection, and no generic
// over a backend handle. It is a vocabulary type that describes the
// begin/commit/rollback semantics of one transactional boundary, so that the
// application layer and the persistence adapter (P1.13) share one name for
// the boundary and one set of rules for it.
//
// The compile-level state machine: `commit` and `rollback` **consume** the
// `UnitOfWork`, so a unit of work can be finalized exactly once — commit
// after commit, commit after rollback, and rollback after rollback are
// compile errors, which is the strongest form of this state machine
// expressible in plain types. What a plain type cannot express is the
// adapter-side obligations below; those are enforced by the adapter's tests
// (P1.13), not by this file.
//
// Adapter contract (every persistence adapter MUST honour this; recorded per
// the P1.10 deliverable):
//
// 1. `UnitOfWork::begin` corresponds to opening the native transaction
//    (for the Stage 1 PostgreSQL adapter: `sqlx::Transaction`, P1.13).
// 2. Every write belonging to the flow — including every
//    `TransactionalWrite` (state change + outbox record) — executes inside
//    that one native transaction, never across several.
// 3. `commit` corresponds to COMMIT of the native transaction; `rollback`
//    corresponds to ROLLBACK. Exactly one of the two happens.
// 4. Dropping an **unfinalized** `UnitOfWork` (neither `commit` nor
//    `rollback` called) MUST roll back the native transaction. A plain type
//    cannot implement a drop handler for a backend it does not know, so the
//    adapter implements this — the P1.13 atomicity test (injected failure
//    leaves neither state nor outbox, or both) is the enforcement.

/// How a finalized unit of work ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum UnitOfWorkOutcome {
    /// The adapter committed the native transaction.
    Committed,
    /// The adapter rolled the native transaction back.
    RolledBack,
}

/// A unit of work after it has been finalized (committed or rolled back).
///
/// Returned by [`UnitOfWork::commit`] and [`UnitOfWork::rollback`]; keeping
/// the label makes the boundary visible in logs and diagnostics without the
/// finalized value being usable for further writes (there is no operation on
/// it that could reopen the transaction).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinalizedUnitOfWork {
    label: Option<String>,
    outcome: UnitOfWorkOutcome,
}

impl FinalizedUnitOfWork {
    /// The diagnostic label, if one was given at `begin`.
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    /// How the unit of work ended.
    pub fn outcome(&self) -> UnitOfWorkOutcome {
        self.outcome
    }

    /// `true` if the unit of work committed.
    pub fn is_committed(&self) -> bool {
        self.outcome == UnitOfWorkOutcome::Committed
    }
}

/// One transactional boundary: begin, exactly one of commit/rollback.
///
/// Construct with [`UnitOfWork::begin`] or [`UnitOfWork::begin_labelled`].
/// The type is a descriptor of the boundary (see the module header for the
/// adapter contract); it holds no backend handle, so it can be created
/// anywhere and driven by the adapter that owns the native transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitOfWork {
    label: Option<String>,
}

impl UnitOfWork {
    /// Begins a unit of work (adapter: open the native transaction).
    pub fn begin() -> Self {
        Self { label: None }
    }

    /// Begins a unit of work with a diagnostic label for logs and tracing.
    pub fn begin_labelled(label: impl Into<String>) -> Self {
        Self {
            label: Some(label.into()),
        }
    }

    /// The diagnostic label, if one was given.
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    /// Commits the unit of work (adapter: COMMIT the native transaction).
    ///
    /// Consumes `self`: a committed unit of work cannot be committed again
    /// or rolled back — that is a compile error, not a runtime check.
    pub fn commit(self) -> FinalizedUnitOfWork {
        FinalizedUnitOfWork {
            label: self.label,
            outcome: UnitOfWorkOutcome::Committed,
        }
    }

    /// Rolls the unit of work back (adapter: ROLLBACK the native
    /// transaction). Consumes `self` for the same reason as [`commit`].
    ///
    /// [`commit`]: UnitOfWork::commit
    pub fn rollback(self) -> FinalizedUnitOfWork {
        FinalizedUnitOfWork {
            label: self.label,
            outcome: UnitOfWorkOutcome::RolledBack,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commit_produces_a_committed_finalized_unit() {
        let unit = UnitOfWork::begin();
        let finalized = unit.commit();
        assert_eq!(finalized.outcome(), UnitOfWorkOutcome::Committed);
        assert!(finalized.is_committed());
        assert_eq!(finalized.label(), None);
    }

    #[test]
    fn rollback_produces_a_rolled_back_finalized_unit() {
        let unit = UnitOfWork::begin();
        let finalized = unit.rollback();
        assert_eq!(finalized.outcome(), UnitOfWorkOutcome::RolledBack);
        assert!(!finalized.is_committed());
    }

    #[test]
    fn the_label_travels_to_the_finalized_unit() {
        let unit = UnitOfWork::begin_labelled("register-asset");
        assert_eq!(unit.label(), Some("register-asset"));
        let finalized = unit.commit();
        assert_eq!(finalized.label(), Some("register-asset"));
    }

    #[test]
    fn begin_unlabelled_has_no_label() {
        let unit = UnitOfWork::begin();
        assert_eq!(unit.label(), None);
    }

    #[test]
    fn a_finalized_unit_cannot_be_reused_for_writes() {
        // The API surface of FinalizedUnitOfWork must not offer any way to
        // reopen or re-finalize a transaction: exactly the three inspection
        // accessors exist, and no commit/rollback/begin method.
        let finalized = UnitOfWork::begin().commit();
        assert!(finalized.is_committed());
        // Compile-level evidence: `finalized.commit()` / `finalized.rollback()`
        // do not compile — there is no such method. Nothing further to assert
        // at runtime; the type system is the test.
    }
}
