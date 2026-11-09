//! Neryva FlexCore — non-port persistence abstractions (P1.10).
//!
//! This crate holds the persistence abstractions that are **not ports**: the
//! repository-abstraction traits live in `flexcore-ports` (decided in P1.07
//! and resolved by P1.10 step 1 — one home, recorded), so **no repository
//! trait is defined here**. The P1.30 boundary audit checks that the two
//! crates never duplicate a trait or type. (P3.21 recorded exception: the
//! `ScheduleRecordStore` record-store contract lives here by the P3.21
//! subtask's file ownership — see the [`schedule_store`] module header for
//! the deviation statement and the relocation path. It is a record store,
//! not a Stage 1 aggregate repository.)
//!
//! | Module | Content |
//! |---|---|
//! | [`unit_of_work`] | [`unit_of_work::UnitOfWork`] — a plain, trait-agnostic transactional-boundary type describing begin/commit/rollback semantics (no async, no SQL) |
//! | [`outbox_write`] | [`outbox_write::TransactionalWrite`] — the "state change + outbox record in ONE transaction" rule (report §6.3 step 1, §15.5) expressed as a type the adapter must honour |
//! | [`retention`] | [`retention::RetentionPolicy`] — per-data-class retention **configuration** that defaults to unset everywhere (P0.16 §4: no universal retention period) |
//! | [`schedule_store`] | P3.21 — [`schedule_store::ScheduleRecordStore`] + row/receipt/error types: the §12.6 schedule-result record persistence contract (append-oriented, immutable records; adapter obligations on the trait; a recorded home deviation from the single-home rule, see the module header). P3.23 appended additively: [`schedule_store::ScheduleComparisonStore`] + row/receipt/error types (the schedule-comparison record persistence contract) |
//! | [`schedule_revision`] | P3.22 — [`schedule_revision::ScheduleRevisionChainStore`] + row/record/receipt/error types: the schedule identity/revision chain and the §14.2 SUPERSEDED transition (append-only revisions, bidirectional supersede links, the `is_revision_superseded` check the Phase 4 P4.12 gate will call; the recorded identity scheme is D-P3.22-1) |
//! | [`command_store`] | P4.10 — [`command_store::CommandRecordStore`] + row/record/receipt/error types: the protected command-lifecycle persistence contract (append-only command records + immutable state-change journal, frozen-edge validation through the P0.11 domain table, bidirectional supersede links, the `ensure_schedule_revision_usable` P4.12 pre-wiring check; same recorded home deviation as `schedule_store`) |
//! | [`envelope_store`] | P3.28 — [`envelope_store::EnvelopeRecordStore`] + row/receipt/error types: the portfolio flexibility-envelope record persistence contract (immutable, append-oriented; same recorded home deviation as `schedule_store`, same adapter obligations) |
//!
//! # The atomicity rule this crate can only describe
//!
//! A type cannot enforce transactional atomicity.
//! [`outbox_write::TransactionalWrite`] makes a partial write
//! **unrepresentable at construction** (it cannot exist without both
//! parts), and the module documents the adapter obligation; the actual
//! enforcement is the P1.13 PostgreSQL adapter's atomicity test (ledger
//! P1.13: "outbox atomicity test passes — injected failure leaves neither
//! state nor outbox, or both"). Reviewers should read the two together.
//!
//! # Scope and control status
//!
//! Persistence-abstraction types for the design baseline only. The entire
//! 0.x line is simulation-only; nothing here performs a write, opens a
//! connection, or touches a database — the adapters (P1.13) own every
//! implementation detail, under the dependency rule enforced by
//! `tools/check_dependency_rule.py` (no sqlx, no tokio in core crates).

// P3.28 — the portfolio flexibility-envelope record persistence contract
// (`EnvelopeRecordStore` trait + row/receipt/error types). Same recorded
// home deviation as `schedule_store` (D-P3.21-4); immutable, append-
// oriented records; the adapter obligations are stated on the trait.
// P4.10 — the protected command-lifecycle persistence contract
// (`CommandRecordStore` trait + row/record/receipt/error types). Same
// recorded home deviation as `schedule_store` (D-P3.21-4); append-only
// command records + an immutable state-change journal; the frozen §14.1
// edge table (P0.11) is the single source of truth, validated through the
// domain's `try_transition`, never re-encoded here.
pub mod command_store;
pub mod envelope_store;
pub mod outbox_write;
pub mod retention;
// P3.21 — the schedule-record persistence contract (`ScheduleRecordStore`
// trait + row/receipt/error types). A recorded home deviation from the
// P1.10 single-home rule (repository traits in flexcore-ports): the ports
// crate was frozen for the P3.21 batch; see the module header (D-P3.21-4)
// and the phase decision record for the relocation path.
pub mod schedule_store;
// P3.22 — the schedule identity/revision chain and the §14.2 SUPERSEDED
// transition (`ScheduleRevisionChainStore` trait + row/record/receipt/error
// types; append-only revisions, bidirectional supersede links). The
// comparison-record storage (P3.23) is appended to `schedule_store`
// additively.
pub mod schedule_revision;
pub mod unit_of_work;
