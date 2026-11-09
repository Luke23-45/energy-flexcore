# P4.10 — Decision note: the command state machine implementation

- **Date:** 2026-11-07.
- **Subtask:** P4.10 (`ledger_phase_4.md`). Report trace: §14.1, §14.2,
  §6.1 (DDD), §8.1 (`flexcore-domain`); the P0.11 freeze
  (`ledger/phase-00-product-freeze/11_command_state_machine.md`) is the
  NORMATIVE ORACLE for the transition table.
- **Deliverables:** `crates/flexcore-domain/src/command_lifecycle.rs` (the
  lifecycle engine), `crates/flexcore-domain/src/lib.rs` (additive decl),
  `crates/flexcore-storage/src/command_store.rs` (NEW — the persistence
  contract), `crates/flexcore-storage/src/lib.rs` (additive decl),
  `crates/flexcore-domain/tests/command_lifecycle.rs` (NEW),
  `crates/flexcore-storage/tests/command_store.rs` (NEW),
  `crates/adapters/postgres/migrations/0016_commands_p4_10.{up,down}.sql`
  (NEW), `crates/adapters/postgres/src/command_store.rs` (NEW),
  `crates/adapters/postgres/src/lib.rs` (additive decl),
  `crates/adapters/postgres/tests/command_store.rs` (NEW, DB-gated),
  this note.
- **Reused, never restated:** the P0.11 frozen 27-edge transition table and
  the 14-state vocabulary (`flexcore_domain::command_state` — the single
  source of truth; this batch adds NO edge); the P1.06/P1.08
  `DispatchCommand` §14.3 record type; the P3.22 revision-chain supersede
  discipline and `is_revision_superseded` surface; the P1.12 §5.1 RLS
  conventions and the P3.21/P3.22 unit-of-work persistence pattern.
- **Not built here (recorded boundaries):** the §12.5 network feasibility
  pipeline and its gate are P4.12's; the full §14.3 field-level command
  validators are P4.14's; the expiry sweep/job is P4.15's; the per-asset/site
  fallback POLICIES are P4.17's (the FALLBACK state, transition, and evidence
  reference are implemented here); the exhaustive 14×14 transition-matrix
  test is P4.33's.

## 1. Frozen-edge conformance statement

The engine implements exactly the 14 states and 27 state-to-state edges of
the P0.11 freeze (report §14.1): the 8 forward states PROPOSED, VALIDATED,
FEASIBLE, APPROVED, ISSUED, ACKNOWLEDGED, TRACKING, COMPLETED, plus
REJECTED, FAILED, EXPIRED, CANCELLED, SUPERSEDED, FALLBACK. No edge was
added, removed, or re-annotated; the engine never re-encodes an edge — every
guard decision goes through `command_state::try_transition`, so the table
remains the single source of truth. Terminal states (COMPLETED, REJECTED,
CANCELLED, SUPERSEDED, FALLBACK) have no outgoing transitions. A missing or
extra edge anywhere is a BUG against the oracle, never an extension. The
migration encodes the same 27 edges in a SQL function as database DEFENCE
IN DEPTH ONLY; a divergence between the two encodings is likewise a bug.

## 2. Recorded decisions

| ID | Decision | Where |
|---|---|---|
| D-P4.10-1 | **Frozen-edge conformance.** The P0.11 table is the single source of truth; the domain engine, the storage constructors, and the migration's `dispatch_commands_legal_edge` function all validate through or mirror it exactly (27 edges, 14 states, 5 terminal states with no outgoing edges, `[*] → PROPOSED` as the only entry). Illegal transitions are rejected with a named error (`LifecycleError::NotPermitted` / `CommandStoreError::IllegalTransition`), never silently ignored, never auto-corrected. | `command_state.rs` (oracle); `command_lifecycle.rs` (`commit_transition`); storage `command_store.rs` (`CommandTransitionRecord::from_transition`); migration 0016 (`dispatch_commands_legal_edge`, `dispatch_commands_guard_update`) |
| D-P4.10-2 | **Write model.** A command row is INSERTed exactly once, in `PROPOSED` (a BEFORE INSERT trigger rejects any other initial state); the ONLY permitted update of a command row is a state transition — the runtime role carries a COLUMN-LEVEL UPDATE grant on exactly the two transition columns (`state`, `superseded_by_command_id`), and a BEFORE UPDATE trigger rejects any content change, any non-frozen edge, and any `SUPERSEDED` movement without the backward link. Content columns are immutable; a re-proposal is a NEW command. The journal (`dispatch_command_transitions`) is INSERT-only: one immutable row per transition (from, to, actor, timestamp, reason, evidence reference, gate). The one-active-transition invariant per command is enforced in SQL: the adapter's compare-and-swap (`UPDATE ... WHERE state = <from>`; zero rows affected = named `TransitionConflict`) plus the trigger's one-state-change-per-UPDATE rule. | storage `command_store.rs`; migration 0016 (triggers, grants, CHECKs); adapter `command_store.rs` (`record_command_transition`) |
| D-P4.10-3 | **Supersede discipline (mirrors P3.22).** A newer command supersedes an older one before completion; both records are RETAINED for audit; the links are bidirectional — the replacement declares the forward link (`supersedes_command_id`, §14.3 field 16) at creation, and the SUPERSEDED transition sets the backward link (`superseded_by_command_id`, required exactly in the SUPERSEDED state, never self-referential — CHECKs in migration 0016). The backward link value is carried in the journal row's evidence reference as `command:<uuid>` (the format the domain engine records in `record_superseded_by`) and derived + UUID-validated by the adapter. SUPERSEDED and CANCELLED are different states and are never merged (§14.2; P0.11 §3.1): CANCELLED is the explicit operator/policy act (APPROVED/ISSUED only, actor + reason required); SUPERSEDED is replacement-driven (VALIDATED/FEASIBLE/APPROVED/ISSUED/ACKNOWLEDGED/TRACKING). They produce structurally distinct journal rows. | `command_lifecycle.rs` (`record_superseded_by`, `record_cancellation`); storage `command_store.rs` (`superseded_by_link`, `COMMAND_STATE_SUPERSEDED`); migration 0016 (supersede CHECKs/FKs) |
| D-P4.10-4 | **Gate separation is structural, not conventional.** Validation, feasibility, and authorization are three DISTINCT transition functions, each applicable ONLY from its own state (`record_validation` from PROPOSED; `record_feasibility` from VALIDATED; `record_authorization` from FEASIBLE — a wrong-state call is a named `WrongState` error). APPROVED without FEASIBLE is impossible by construction: FEASIBLE is reachable only through `record_feasibility` with a passed outcome, and APPROVED only from FEASIBLE. Each gate carries its OWN evidence reference (validation evidence ≠ feasibility evidence ≠ authorization decision); every gate pass and every gate rejection names its gate in the journal (the `gate` column, CHECK-enforced to be present exactly for → VALIDATED / FEASIBLE / APPROVED and → REJECTED). REJECTED carries WHICH gate rejected (`rejected_by`) and the failing check / denial reason. Acknowledgement is recorded as application-layer ACCEPTANCE, never physical delivery (§14.1). | `command_lifecycle.rs` (`Gate`, `record_validation`, `record_feasibility`, `record_authorization`); storage `command_store.rs` (`COMMAND_STATES_REQUIRING_GATE`); migration 0016 (`dispatch_command_transitions_gate_rule`) |
| D-P4.10-5 | **P4.12 pre-wiring, no feasibility gate.** Commands reference schedule ID + revision (P3.22 identity scheme; composite FK into `schedule_revisions` makes the referenced revision exist). A transition on a command whose source schedule revision is SUPERSEDED — or unknown in the tenant scope — is rejected at the gate-check level via `CommandRecordStore::ensure_schedule_revision_usable`, which calls the P3.22 `is_revision_superseded` surface (`Some(true)` rejected as superseded; `Some(false)` usable; `None` rejected as unknown — unknown is never "usable"). The §12.5 network feasibility pipeline itself is NOT built here. | storage `command_store.rs` (`ensure_schedule_revision_usable`, `ScheduleRevisionSuperseded`/`ScheduleRevisionUnknown`); adapter `command_store.rs` (impl over the P3.22 surface); P3.22 `PHASE4_GATE_NOTE` |
| D-P4.10-6 | **Expiry is STATE SUPPORT only.** The four EXPIRED edges (frozen transitions 10, 15, 20, 24), the validity comparison against a controlled clock (`CommandLifecycle::validity_has_lapsed`, §10.3 end-exclusive: lapsed once `now >= valid_until`), and the milestone-not-reached record (P4.15 step 1: BEFORE-ISSUE / BEFORE-ACKNOWLEDGEMENT / BEFORE-TRACKING-COMPLETION / BEFORE-COMPLETION, matched to the current state) are implemented here. Expiry never fires early (`ValidityNotLapsed`), and the sweep/job/timer is P4.15's. Timestamps on EVERY transition come from a caller-supplied controlled clock (`LifecycleClock`), never wall-clock, and a timestamp moving backwards against the journal is a named error (`TimestampRegression`, §17.4 clock-drift discipline). FAILED / EXPIRED exit only via FALLBACK (frozen edges 26–27) with a policy reference; the policies themselves are P4.17's; FALLBACK is terminal. | `command_lifecycle.rs` (`LifecycleClock`, `validity_has_lapsed`, `record_expiry`, `ExpiryMilestone`, `record_fallback`); storage `command_store.rs` (`COMMAND_STATES_REQUIRING_REASON`) |

## 3. Reason / evidence / gate reporting rules (decided once)

The journal reporting rules are total over the 14 states, enforced by the
storage constructors and re-encoded as CHECKs in migration 0016:

- **Evidence REQUIRED** on entry to VALIDATED, FEASIBLE, APPROVED, ISSUED,
  ACKNOWLEDGED, TRACKING, COMPLETED, SUPERSEDED, FALLBACK (§6.4: each stage
  records evidence; §14.3 field 16; §14.4).
- **Evidence ALLOWED (not required) on REJECTED** — the failing-check
  evidence, per ledger P4.11 ("REJECTED with the failing check and
  evidence"). The domain engine always records it on gate rejections;
  forbidden on every other state.
- **Reason REQUIRED** on REJECTED (the failing check / denial reason, §14.3),
  FAILED (the frozen-table trigger), EXPIRED (the milestone not reached),
  CANCELLED (the operator/policy reason), FALLBACK (the policy that fired).
- **Gate named exactly** on → VALIDATED / FEASIBLE / APPROVED and → REJECTED
  (§14.1: the three gates are distinct); never on any other transition.
- **Failure triggers are state-matched**: a NACK/timeout/transport failure
  fails only an ISSUED command, "no expected response" only an
  ACKNOWLEDGED one, a tracking deviation/timeout only a TRACKING one
  (`FailureReason::allowed_from`).

## 4. Rejection-point statement

The freeze's three rejection points are all implemented and
gate-attributed: PROPOSED → REJECTED (malformed / unsupported — validation
gate), VALIDATED → REJECTED (capability / network failure — feasibility
gate), FEASIBLE → REJECTED (authorization denied — authorization gate).
No other state reaches REJECTED. The §14.3 rejection triggers (stale,
duplicated-with-conflicting-payload, out-of-sequence, outside-capability,
missing-evidence) are P4.14's validators; this batch provides the state
machinery they reject through.

## 5. Test coverage map (ledger P4.10 acceptance criteria)

| Acceptance criterion | Where satisfied |
|---|---|
| All §14.1 transitions exist and work | `flexcore-domain/tests/command_lifecycle.rs` (every legal edge exercised); postgres `tests/command_store.rs` (the full forward path persisted step by step) |
| Tested illegal transitions rejected with a named error; terminal states have no outgoing transitions | `flexcore-domain/tests/command_lifecycle.rs`; postgres `tests/command_store.rs` (`illegal_and_racing_transitions_are_refused_and_leave_no_change`) |
| No state outside §14.2 exists | The closed vocabulary is pinned in `command_state.rs` (14 states) and mirrored by `COMMAND_STATES` (a test pins the mirror in both directions) |
| Actor + timestamp on every transition, controlled clock | `LifecycleClock`; `FixedClock` in the domain tests; timestamp-regression test |
| Gate separation structural; APPROVED-without-FEASIBLE impossible | `record_validation`/`record_feasibility`/`record_authorization` state guards; `record_authorization` unreachable except from FEASIBLE (guard test in the domain suite) |
| REJECTED carries which gate | `rejected_by`; journal `gate` column; gate-rejection persistence test |
| SUPERSEDED bidirectional, retained, distinct from CANCELLED | Domain suite + postgres `supersede_links_both_directions_and_stays_distinct_from_cancellation` |
| Schedule-revision gate check (P4.12 pre-wiring) | postgres `the_schedule_revision_gate_check_rejects_superseded_and_unknown` |
| Expiry with controlled clock | Domain suite (`record_expiry` early/late/milestone cases) |
| Journal completeness (from, to, actor, timestamp, reason, evidence) | Postgres `the_full_forward_lifecycle_journalls_every_transition` |
| RLS / cross-tenant | Postgres `a_foreign_tenant_command_reads_as_none_and_leaks_nothing` (DB-gated on `FLEXCORE_TEST_DATABASE_URL`; skips cleanly when unset) |
| Content immutability + append-only journal at the database | Postgres `command_content_and_the_journal_are_immutable_at_the_database` |

The exhaustive 14×14 matrix (legal AND illegal, generated from the table)
is P4.33's and is deliberately not duplicated here.

## 6. VERIFY-BUILD record

- **VERIFY-BUILD (field-visibility precedent):** the new
  `crates/adapters/postgres/src/command_store.rs` accesses the private
  `tx`/`runtime` fields of `PgUnitOfWork` from a sibling module — the same
  pattern as `telemetry.rs`, `corrections.rs`, and `schedule_store.rs`. If
  rustc rejects that access (E0616), the fix is `pub(crate)` on the two
  field declarations in `unit_of_work.rs` (outside this subtask's
  ownership).
- **Not run (per task constraints):** `cargo build`/`check`/`test`/`clippy`
  were NOT executed for this batch; all Rust and SQL above is
  reviewed-by-construction against the existing patterns
  (`schedule_store.rs`, migration 0014) and carries the VERIFY-BUILD note
  where an unverifiable API detail remained.

## 7. Control status

The command lifecycle is a protected platform invariant owned by the
platform kernel (§0.2, §5.7): plugins may propose and request validation
but can never authorize, issue, or select the control-authority mode. The
entire 0.x line is simulation-only (§21.4): nothing in this batch performs
a device write, and an acknowledgement recorded through it is acceptance,
never physical delivery (§14.1). FlexCore performs no primary frequency
response, synthetic inertia, relay protection, inverter inner-loop control,
or sub-cycle control (§14.5).
