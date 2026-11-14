# P4.26 + P4.33 — Decision note: the combined/adversarial rejection suite and the exhaustive command-transition matrix (§23 Stage 4, §22.3)

- **Date:** 2026-11-10.
- **Subtasks:** P4.26, P4.33 (`ledger_phase_4.md`). Report trace: §23 Stage 4
  exit gate, §19.2 layer 6 (fault injection), §17.4, §22.3 (release gate),
  §14.1–§14.2.
- **Deliverable files:** `crates/flexcore-application/tests/rejection_combined.rs`
  (P4.26), `crates/flexcore-domain/tests/transition_matrix.rs` (P4.33), this
  note, and the ledger checkbox ticks. No source file was touched (test-only
  suites over EXISTING public APIs; `crates/*/src/*` and both `lib.rs` files
  untouched — file ownership held).
- **Build/test status (honest):** NOTHING was built, run, or installed. No
  `cargo build/check/test/clippy/fmt`, no test execution, no installs — the
  loop bans were held for the whole batch. Both suites are WRITTEN, never
  executed. Status of both subtasks: **implemented, awaiting human
  build/test** — never "tested"/"verified"/"passed". In particular the
  determinism requirement is recorded as in-suite assertions (each fixture
  runs twice with identical inputs; outcomes asserted identical), not as two
  actual suite runs.

## 1. Implementation model (same pattern as the established suites)

Test-only suites over existing public APIs with in-test fakes, matching the
P4.20–P4.25 precedents (`rejection_invalid/stale/unauthorized/duplicate/
expired`): `FakeEventOutbox` (`.len()`/`.writes()`; the outbox port's
idempotency-key uniqueness), `CountingIdGenerator`,
`StateChangeReceipt::new(entity, op, Option<TenantId>)`, tenant contexts
through the single claims path (P0.16 rule 5), `CommandGateStore` /
`CommandGateView` with compare-and-swap journalling, `replay_full_lifecycle`
(use), `command_state::{CommandState, try_transition, TransitionError}`
(the P0.11 frozen oracle), `SimulationKeyring`, `check_issue_refusals` (the
P4.16 pure refusal-order check), the P4.15 `CommandExpiryService` /
`apply_expiry`, controlled clocks everywhere (never wall-clock), and faked
seams that fail closed by construction. All names/signatures were matched
against the real modules end-to-end before writing (no hallucinated APIs).

## 2. P4.26 — combined and adversarial fixtures (UNRUN, pre-registered)

Every row's expectation is PRE-REGISTERED and was NOT observed by execution;
the suite encodes the pre-registered reasons as `pre_registered_rejection()`
and asserts against them verbatim. Per the ledger rule: the rejection must
name at least one applicable failing check; where two faults apply, the
recorded reason is ONE OF them — never a pass.

| Fixture | Faults present (both real, proven in-suite) | Pre-registered outcome (UNRUN) | Where each fault is proven |
|---|---|---|---|
| F-CM-1 | stale + infeasible: the source envelope is stale by the REAL P3.13 max-age evaluation (age 61 000 ms > configured 60 000 ms) AND the feasibility runner is scripted to `LimitViolated { branch_thermal:line_2_3 }` | REJECTED at the VALIDATION gate with `envelope_stale:MaxAgeExceeded { age_millis: 61000, max_age_millis: 60000 }`; the runner is NEVER called (a counting seam proves the §14.2 first-failure order, not assumed); journal PROPOSED → REJECTED with the check as reason; `DispatchProposed` then `DispatchRejected`; never ISSUED | control half: with the staleness fault removed, the SAME scripted verdict rejects at the feasibility gate with `network_limit:branch_thermal:line_2_3` — the masked fault is real |
| F-CM-2 | duplicate + unauthorized: a command at FEASIBLE whose idempotency key is replayed with a CONFLICTING payload, while an approval is attempted by a role with NO grant (`viewer`) | The conflicting duplicate is refused at the store seam (`Conflict`) and the P4.14 pure verdict `duplicated_conflicting_payload` fires; the approval attempt is REJECTED with `authorization_denied:no matching rule for role viewer` → FEASIBLE → REJECTED, §16.2 denial record (journal row names the Authorization gate + `authz-denial:` evidence + one `DispatchRejected`); exactly one effective command per key; never ISSUED | the pure-verdict check and the store-seam refusal prove fault 1; the denial record proves fault 2 |
| F-CM-3 | expired + infeasible: a command already at APPROVED when its validity ends (clock advanced exactly to the end-exclusive boundary) AND the grid model behind the issue hook is not Active (`retired`) | The adapter refuses the issue with `ExpiredCommandRefused` (the ledger refusal order — unsigned → unsequenced → expired — puts the expired refusal first; the recorded reason IS one of the two applicable checks); the P4.15 sweep then expires it: APPROVED → EXPIRED, milestone `BeforeIssue`, the boundary instant recorded, exactly one `DispatchExpired`; never ISSUED | control half: the SAME inactive-model fault with the window open blocks the issue on its own (`ModelNotActive`, state named) |
| F-CM-4 | forged credential + stale (§17.4 forged-command threat): the issue is attempted with a FORGED P4.13 authorization decision record while the validity has lapsed | The combined attempt is refused `ExpiredCommandRefused` (first applicable check in the refusal order; the forged record never gets evaluated); the forged credential ALONE (fresh window) blocks with `AuthorizationRecordMismatch` — the mismatch names the journal reference; the command's state is unchanged; never ISSUED. The pure `check_issue_refusals` order is pinned: forged (non-empty) authenticity + lapsed window → EXPIRED; blank signature → UNSIGNED (at any clock) | both halves prove each fault individually blocking |
| F-CM-5 | replayed event + conflicting payload: a REPLAYED sequence number (incoming 10 vs last recorded 10 — strictly-monotonic rule) AND a conflicting payload under the same idempotency key | The PURE §14.3 verdict layer names BOTH failing checks (`duplicated_conflicting_payload` AND `out_of_sequence` — the honest multi-check surface, §3); the store seam refuses the insert (`Conflict` — the key conflict fires first at the seam); the replayed-EVENT half is refused at the outbox seam (`EventOutboxError::DuplicateKey` on the existing `dispatch_proposed:<uuid>` key, the P1.12 unique constraint, republish-after-crash safe §15.5); exactly one effective command; never ISSUED | the pure-verdict pair, the seam refusal, and the outbox refusal each asserted |

Aggregate exit checks in-suite: every fixture's outcome matches its
pre-registered family and its pre-registered final state (REJECTED /
REJECTED / EXPIRED / APPROVED-unchanged / FEASIBLE-unchanged); every
fixture runs TWICE with identical outcomes (the in-suite determinism
discipline); NO combined fixture reaches ISSUED — asserted per fixture on
the journal and on the delivered-envelope list, and structurally on the
P0.11 frozen-edge oracle (no REJECTED → ISSUED, FEASIBLE → ISSUED, or
EXPIRED → ISSUED edge exists: the block is a property of the frozen table,
not a convention).

### The multi-check mitigation, answered honestly (ledger P4.26 risk note)

The ledger mitigation asks: where the implementation evaluates multiple
checks, the rejection record should list ALL failing checks found, not just
the first. What the real surfaces do (read, not assumed):

- The **validation gate** (`CommandGates::run_validation_gate` →
  `validation_checks`) is explicitly **first-failure-wins** — the source
  states "The first failure wins (one failing check per rejection,
  deterministically)" and returns a single `failing_check: Option<String>`
  in §14.2 order. F-CM-1 pins this honestly: the staleness check is
  recorded, the infeasible verdict is present but not reached, and the
  counting runner proves it was never called.
- The **authorization gate** (`record_command_approval`) evaluates its four
  checks in order and records the FIRST denial reason (the separation-of-
  duties check short-circuits; the policy/scope/expiry checks keep the
  first-set reason).
- The **adapter issue path** checks its refusals in the fixed ledger order
  (unsigned → unsequenced → expired) and stops at the first; F-CM-3/F-CM-4
  pre-register the first applicable refusal and prove the second fault
  blocking on its own in a control half.
- The **PURE §14.3 verdict layer** (`flexcore_application::command_record`)
  evaluates each rule as an independent function, so a caller CAN collect
  all failing checks — F-CM-5 does exactly that and asserts BOTH names.
  The pure layer is the surface where "list all failing checks found" is
  real today; the gate/journal layer records one reason per REJECTED row.

Changing the gate surfaces to list all failing checks would be an
enforcement-code change — out of scope for a test-only subtask (file
ownership). Recorded here as the honest answer; the human review can
decide whether a multi-reason journal record is wanted (it would touch
`GateResult`/`CommandTransition` shapes and every suite above).

## 3. P4.33 — the exhaustive transition matrix (UNRUN)

- **Method (anti-circular).** The 27 freeze edges are hand-transcribed FROM
  THE P0.11 FREEZE DOCUMENT (`ledger/phase-00-product-freeze/
  11_command_state_machine.md` §2), in table order with guard text verbatim,
  as a const array `FROZEN_TABLE_EDGES: [(CommandState, CommandState, &str);
  27]`. The expected outcome of every matrix cell is derived from that
  transcription — never from the implementation under test (the ledger's
  circularity mitigation). The implementation's own frozen table
  (`command_state.rs` `FROZEN_EDGES`) is `pub(super)` inside a `#[cfg(test)]`
  module, i.e. NOT exposed programmatically to integration tests — so the
  hand-written transcription is the required form per the subtask.
- **Matrix construction.** The driver iterates `CommandState::ALL` ×
  `CommandState::ALL` — all 196 ordered pairs — and classifies each cell
  legal/illegal by the transcription. Exhaustive by construction: the state
  set is closed and pinned (a vocabulary test asserts the 14 names and no
  duplicates, so the pair space cannot silently shrink below 196).
- **Counts asserted, not assumed:** 27 legal cells, 169 illegal cells,
  196 total, each ordered pair appearing exactly once, and each of the 27
  transcribed edges realized as exactly one legal cell.
- **Legal half:** every frozen edge succeeds and lands EXACTLY in the
  frozen target (`try_transition(from, to) == Ok(to)`) — the P4.10 legal
  set re-run as part of the matrix (not as a separate suite, per the
  ledger step 2).
- **Illegal half:** all 169 illegal pairs are rejected with the NAMED
  domain error — `Err(TransitionError::NotPermitted { from, to })` exactly,
  with the error's Display naming both states (the audit-ready text). The
  named ledger skips (PROPOSED → APPROVED, VALIDATED → ISSUED,
  FEASIBLE → ISSUED and further shortcuts) are asserted individually, all
  14 self-transitions are illegal, all 5 × 14 = 70 cells out of terminal
  states (COMPLETED, REJECTED, CANCELLED, SUPERSEDED, FALLBACK) are
  illegal, and FAILED/EXPIRED exit only via FALLBACK (frozen edges 26–27).
- **Totality check:** `no_edge_outside_the_frozen_table_exists` proves
  over the WHOLE pair space that the oracle's accepted set and the
  transcribed freeze set coincide — no edge exists that is not in the
  P0.11 table, no table edge is missing, and an accepted transition lands
  exactly in the requested target (the oracle never redirects). A
  per-state outgoing-edge test diffs the transcription against the freeze
  document's own §2.1 second-pass counts (2/3/3/4/5/4/4/1/1 = 27).
- **Mutation safety.** The ledger's verification method ("add a bogus
  transition, confirm failure, revert") requires a build+run and is
  PENDING the human pass. The suite proves mechanically
  (`a_bogus_added_transition_would_fail_this_matrix`) that a "convenience"
  edge (PROPOSED → ISSUED is the worked example) is not in the
  transcription, is refused by the oracle, and would be caught by the
  cell-by-cell comparison — so any future transition-table change that
  adds an untested transition fails the suite (the CI-wiring intent of
  ledger step 4; the suites are CI-runnable by construction through the
  `ci` workflow's `unit-tests` job `cargo test --workspace`, equivalently
  `cargo test -p flexcore-domain` — no CI file was modified, file
  ownership).
- **State-change and audit-gap assertion (ledger step 3), honestly
  scoped:** `try_transition` is a PURE domain function — a rejected
  transition leaves no state change BY CONSTRUCTION (there is no store at
  the domain layer) and there is no audit gap: no transition row is
  produced, because the refusal is a returned error, not a recorded event.
  Rejected-attempt AUDIT ROWS (where an audit policy requires one) are
  application/persistence wiring — P4.30 (pending). The engine-level gate
  discipline (a gate applied from the wrong state is refused with the
  journal unchanged) is exercised by the existing P4.10 suite
  (`crates/flexcore-domain/tests/command_lifecycle.rs`); this suite owns
  the TABLE, not the engine.

## 4. Recorded decisions (all labels DEFAULT)

| ID | Decision | Where |
|---|---|---|
| D-P4.26-1 | **Fixture set and pre-registration form.** Five combined fixtures, each carrying two REAL faults; each expectation pre-registered as a named reason string asserted verbatim, with per-fixture control halves proving each fault blocking on its own (so a masked fault is never a phantom). All labels DEFAULT. | `tests/rejection_combined.rs` header + `pre_registered_rejection` |
| D-P4.26-2 | **First-failure-only is the recorded reality of the gate surfaces; the pure §14.3 verdict layer is the multi-check surface.** F-CM-1 pins the first-failure order with a counting runner; F-CM-5 asserts BOTH pure verdicts. Changing the journal to multi-reason records is an enforcement-code change — explicitly out of scope here and left to review. | §2 (the mitigation answer); `validation_checks` source comment |
| D-P4.26-3 | **Determinism is an in-suite double run.** Every fixture closure runs twice from clean state with identical inputs and the outcomes are asserted equal (`assert_eq!` on outcome tuples); the aggregate re-runs all five. No external second run was performed (nothing was run at all). | per-fixture closures + `the_combined_fixtures_are_deterministic_across_two_runs` |
| D-P4.26-4 | **The forged-credential surface of record is the P4.13 authorization-record cross-check; command signatures are not cryptographically verified in simulation.** D-P4.16-1's keyring signs at issue with a non-cryptographic reference; the §14.3 field-15 surfaces enforced today are non-empty authenticity (`Unsigned`), the pure refusal order, and the `AuthorizationRecordMismatch` cross-check. Real signature/tamper verification is P4.28's (open item, §5). Recorded, not silently assumed. | F-CM-4 + its test comments |
| D-P4.33-1 | **The matrix oracle is the freeze DOCUMENT transcription, not the implementation.** 27 edges transcribed verbatim (with guards) from `11_command_state_machine.md` §2; the implementation table is not programmatically exposed to integration tests (checked), so the hand-written transcription is the required anti-circular form. Counts 27/169/196 asserted. | `tests/transition_matrix.rs` (`FROZEN_TABLE_EDGES`, `full_matrix`) |
| D-P4.33-2 | **Exhaustive by construction over the closed pair space** — `CommandState::ALL` × `CommandState::ALL` with a pinned 14-name vocabulary and a no-duplicates check, so no cell can be skipped and none hand-picked. | `full_matrix` + `the_matrix_is_built_over_exactly_the_14_frozen_states` |
| D-P4.33-3 | **No-state-change and no-audit-gap are structural at the domain layer; rejected-attempt audit rows are P4.30 wiring.** `try_transition` is pure — a refusal is a returned named error, never a recorded transition. Recorded as the honest answer to ledger step 3's "record which". | §3 last bullet; suite header |
| D-P4.33-4 | **CI wiring is by construction, not by edit.** The matrix test lives in `flexcore-domain`'s integration tests and runs under the existing `cargo test --workspace` job; a transition-table change that adds an untested transition fails the cell comparison. No CI file was modified (file ownership). | §3 mutation-safety bullet |

## 5. Dependencies and the recorded open item

- P4.26's inputs (P4.20–P4.25) are all done; the suite builds directly on
  their recorded decisions (D-P4.07-1/D-P4.08-1 routing and policy shapes
  for the infeasible half via the `CommandFeasibilityRunner` seam; the
  D-P4.23-2 grant matrix mirror for the unauthorized half; the D-P4.24-1
  store-seam duplicate/sequence contract; the D-P4.25-1/P4.15 expiry paths).
- **OPEN ITEM (P4.28 re-verifies):** the forged-signature half of P4.26
  exercises the enforcement surfaces that exist in the simulation stack
  (D-P4.26-4). When P4.28 lands, its re-verification must confirm that real
  signature verification (tampered payload fails, missing signature
  rejected) is wired at the adapter, and re-run this suite's F-CM-4
  expectations against it — the pre-registered table will need a row for
  the verified-signature surface, added by decision-record revision, not by
  silently changing the assertions.
- **OPEN ITEM (P4.27 evidence pack):** this suite is a Stage 4 input; its
  UNRUN status must be carried into the P4.27 pack honestly (expectations
  pre-registered, no runs performed in this batch).
- **OPEN ITEM (P4.30):** rejected-attempt audit rows for illegal
  transition attempts, where the audit policy requires one — the domain
  refusal is the named error; the persistence wiring is P4.30's.

## 6. File/test index

| File | Content |
|---|---|
| `crates/flexcore-application/tests/rejection_combined.rs` | P4.26 suite: the five combined fixtures with pre-registered reasons, per-fixture control halves, the counting feasibility runner (first-failure order proven), the §16.2 denial-record assertions, the pure refusal-order check for forged/blank authenticity, the pure-verdict multi-check surface, the store-seam and outbox-seam refusals, per-fixture no-ISSUED + frozen-edge-oracle checks, aggregate exit check, two-run determinism |
| `crates/flexcore-domain/tests/transition_matrix.rs` | P4.33 suite: the freeze-document transcription (27 edges with guards), the mechanical 196-cell matrix, the 27/169 count assertions, the legal-edge walk with exact frozen targets, the 169 named-error rejections, terminal-row and shortcut and self-transition cells, the totality check, the freeze §2.1 outgoing-count diff, the bogus-edge mutation demonstration, the vocabulary pin |
| `ledger/phase-04-grid-feasibility-dispatch/ledger_phase_4.md` | P4.26/P4.33 objective boxes ticked as "implemented, awaiting human build/test" with this record cited |
| `ledger/phase-04-grid-feasibility-dispatch/HANDOVER.md` | wave-status row updated |

## 7. Honest limitations

- **Nothing was built, run, installed, or executed.** Every expectation
  above is PRE-REGISTERED and UNRUN — the human build/test pass is the
  gate; nothing in this note may be read as a test result. The ledger's
  mutation check for P4.33 is pending that pass (the suite's mechanical
  demonstration is written, not executed).
- **First-failure-only journal records:** the gate surfaces record one
  failing check per rejection; only the pure verdict layer lists all
  checks. Recorded (D-P4.26-2), not worked around.
- **No cryptographic signature verification exists in the simulation
  stack** (D-P4.16-1); F-CM-4 pins the surfaces that DO exist and records
  the P4.28 open item (§5).
- **Rejected-attempt audit rows are P4.30 wiring** (D-P4.33-3); the domain
  refusal is a returned error, by design.
- **The determinism "two runs" are in-suite assertions**, not actual suite
  executions (same recorded form as the P4.20–P4.25 suites).
- **Research cut-off:** no external sources were consulted; the
  7 November 2026 cut-off is unchanged.

## 8. Review status and verification

- **Build/test sign-off: PENDING (human).** Nothing in this batch was
  compiled or executed (loop bans held); the ledger checkboxes are ticked
  as "implemented, awaiting human build/test" only.
- **Security review sign-off: PENDING** — should confirm the F-CM-2/F-CM-4
  adversarial coverage against §17.4 and the D-P4.26-4 signature-surface
  boundary.
- **QA review sign-off: PENDING** — should confirm the determinism form,
  the pre-registration discipline, and the matrix exhaustiveness argument
  (closed pair space + asserted counts).
- **AGENTS.md §6 tools:** run at wave close over the documentation changes
  (this note, the ledger, HANDOVER); they scope the report and the agent
  harness, not the crates.
