<!-- P10.03 — Command-state transition test suite — full matrix (decision record). REAL test files WRITTEN-UNRUN over the actual crate seams (ENTRY_NOTE boundary 3): zero observed failures are NOT claimed from an unrun suite; the run is the human build/test gate. Release-candidate identifier PENDING P10.18. -->
# P10.03 — Command-State Transition Test Suite — Full Matrix

Date: 2026-11-14. Subtask: P10.03 (`ledger_phase_10.md` §P10.03).
Owner role: Platform engineering (with QA/test). Report trace: §22.3 release
gate 2 ("all command-state transitions are tested"); §14.1–§14.3.
Inputs: P10.01 scope freeze (`01_release_scope_freeze.md`); the §14.1 state
machine; the P0.11 frozen table
(`ledger/phase-00-product-freeze/11_command_state_machine.md`) as the test
oracle; P9.19 shadow dry-run evidence
(`ledger/phase-09-pilot-qualification/19_command_dry_run.md`) —
cross-referenced, NOT redone; Phase 9 exit record P9.30 §2.
Deliverable: `crates/flexcore-domain/tests/command_transition_release_matrix.rs`
(additive; no `src` logic touched; no other crate touched).

## 1. Honest status — the plain statement

**The suite is WRITTEN, never executed.** The loop bans (no builds, no test
runs, no installs) were held; per ENTRY_NOTE.md boundary 3 this is
"implemented, awaiting human build/test". **Zero observed failures are NOT
claimed** — an unrun suite has no observed failures and no observed passes;
the run is the human gate. Every expected outcome in the suite is
hand-computed from the P0.11 freeze table, so the run, when it happens, is a
comparison against an independent oracle — not a self-check.

The suite also runs against a release candidate that **does not exist as a
built artifact yet** (ENTRY_NOTE boundary 1): the candidate-identifier cell
below is `<PENDING P10.18>` and any candidate change re-opens this subtask
(ledger step 5, P10.01 §6.1 drift rule).

| Field | Value |
|---|---|
| Release-candidate identifier | `<PENDING P10.18 — no candidate build exists>` |
| Suite result | `<EMPTY — WRITTEN-UNRUN; the human run fills this>` |
| P9.19 cross-reference | shadow dry-run TEMPLATE, UNRUN (`19_command_dry_run.md`) — cited, not redone |

## 2. Terminology finding — FROZEN_TRANSITIONS and CRASH_TRANSITIONS are PLUGIN-lifecycle identifiers

The subtask hand-off refers to "the FROZEN_TRANSITIONS table" in
`crates/flexcore-domain/src/command_lifecycle.rs` and to "CRASH_TRANSITIONS".
Recording the actual code facts, so no reviewer chases a wrong table:

- **`FROZEN_TRANSITIONS` and `CRASH_TRANSITIONS` are the PLUGIN lifecycle
  tables** (report §5.6): `crates/flexcore-kernel/src/lifecycle.rs`
  (`FROZEN_TRANSITIONS`, 12 edges; `CRASH_TRANSITIONS`, 5 added `→ Failed`
  edges so a plugin crash in any non-terminal state is representable — the
  recorded DEFAULT decision). They are exercised by
  `crates/flexcore-kernel/tests/lifecycle_p7_07.rs` (WRITTEN-UNRUN) and are
  re-verified for the release by P10.08 (plugin permission-boundary tests),
  which owns that seam. This suite's crate (flexcore-domain) cannot depend on
  flexcore-kernel — the dependency rule forbids it
  (`tools/check_dependency_rule.py`) — so the plugin tables are OUT of this
  file's reach by design, not by omission.
- **The command lifecycle's frozen table is the P0.11 27-edge table**
  implemented in `crates/flexcore-domain/src/command_state.rs`
  (`try_transition`, states, terminal set) and driven by
  `crates/flexcore-domain/src/command_lifecycle.rs` (the engine whose guard
  decisions all delegate to the table). That table — cited verbatim from
  report §14.1 with its guard text — is what this suite tests, per the
  ledger's trace (§22.3 gate 2; §14.1–§14.3).
- The command-domain fail-closed analogue of the plugin `CRASH_TRANSITIONS`
  decision IS asserted in the suite: every non-terminal command state has at
  least one frozen exit, and every terminal state has none
  (`every_non_terminal_state_has_a_frozen_exit_fail_closed`), plus the
  command-domain "crash" vocabulary is the frozen failure-trigger set
  (`FailureReason`, 6 triggers) and expiry-milestone set (`ExpiryMilestone`,
  4 milestones), each pinned textually.

## 3. What the suite covers (matrix arithmetic, hand-computed)

### 3.1 Table-level matrix — 196 ordered pairs, all tested

The state set is closed and pinned (14 states, names asserted). The suite
transcribes the P0.11 freeze table as DATA (27 edges, §14.1 guard text
verbatim) and derives every expected outcome from that transcription — never
from the implementation:

- **27 legal edges exercised positively** — each succeeds and lands in
  exactly the frozen target (`table_level_matrix_all_196_pairs_have_hand_
  computed_outcomes`, `table_level_no_edge_exists_outside_the_transcribed_
  freeze`).
- **169 illegal pairs exercised negatively** — 14 × 14 = 196, minus 27 legal
  = 169; every one refused with the NAMED error
  (`TransitionError::NotPermitted` carrying both states, whose Display names
  both) — including all 70 terminal-state cells, all 14 self-transition
  cells, and the gate-skipping shortcuts.
- Counts asserted, not assumed: 27 / 169 / 196.

### 3.2 Engine-level full matrix — 374 (state, operation, lapse-mode) cells

The P4.33 suite owns the TABLE; this suite additionally owns the ENGINE
surface (`CommandLifecycle`, 12 public operations with their argument
domains: pass/fail gate outcomes, approve/deny authorization outcomes, 6
failure triggers, 4 expiry milestones, cancel, supersede, fallback, and the
4 forward operations). For EVERY state × operation, the expected outcome is
hand-computed in an expectation function whose `match` over `CommandState`
has **all 14 arms and no wildcard** — adding a state variant is a compile
error, so the suite is exhaustive over the lifecycle vocabulary by
construction (the ledger's exhaustiveness requirement, implemented).

| Cell class | Count | Hand-computed expectation |
|---|---|---|
| Lapsed-clock cells: 14 states × 23 operations | 322 | success into exactly the frozen target (journal +1 entry) or the exact named error with state and journal unchanged |
| Pre-lapse expiry cells: 13 states × 4 milestones (EXPIRED unreachable pre-lapse) | 52 | `ValidityNotLapsed { now, valid_until }` exactly |
| **Total** | **374** | asserted counts: 30 successes, 90 `WrongState`, 72 `NotPermitted`, 78 `InvalidFailureReason`, 52 `InvalidExpiryMilestone`, 52 `ValidityNotLapsed` |

The 30 engine success cells realize the 27 table edges (the three FAILED
edges 14/19/23 are realized by six annotated (state, trigger) combos —
Issued: NACK/acknowledgement-timeout/transport-failure; Acknowledged:
no-expected-response; Tracking: tracking-deviation/tracking-timeout). The
per-state success-target SETS are asserted equal to the frozen-table
successors (`engine_success_targets_equal_the_frozen_table_successors`) —
the engine accepts exactly the frozen edges, no more, no fewer.

### 3.3 The five §14.3 rejection classes

All five classes are exercised into REJECTED with the failing check named on
the journal entry and the rejecting gate recorded (`rejected_by`):
`stale_telemetry`, `duplicate_conflicting_payload`, `out_of_sequence`,
`outside_capability`, `missing_evidence` — each through the validation gate
(freshness, duplication, sequence, capability are validation-gate checks per
§14.2). The missing-evidence class additionally covers the STRUCTURAL
refusals with their exact named errors: empty evidence reference
(`EmptyField{evidence_reference}`), whitespace actor
(`EmptyField{actor}`), a failed gate outcome without a failing check
(`EmptyField{failing_check}`), a denial without its reason
(`MissingFailingCheck{authorization}`), an empty cancellation reason, and
self-supersede (`SelfSupersede`).

**Recorded boundary (honest):** the payload-level §14.3 command-record
fields (unique ID, validity window, sequence, idempotency key, evidence
references) live in `crates/flexcore-domain/src/dispatch_command.rs` (P1.06)
and its tests — this suite proves the LIFECYCLE routes every failing
outcome to REJECTED with the named check; it does not re-test payload
construction. Ledger step 4's §14.3 invariant verification is therefore a
cross-reference to the P1.06 suite, not a duplicate.

### 3.4 Expiry path — hand-computed

- The four EXPIRED edges (10, 15, 20, 24) fire on their annotated milestone
  from APPROVED / ISSUED / ACKNOWLEDGED / TRACKING, with the journal entry
  asserted field by field (from/to, timestamp == the lapsed instant, gate
  None, reason == the milestone's frozen text, evidence None), then each
  EXPIRED command falls back (`EXPIRED → FALLBACK`, edge 27, with the
  policy-reference evidence and the frozen reason text).
- Expiry requested before the window ends is refused by name
  (`ValidityNotLapsed` with the exact observed time and bound); the state
  and journal are unchanged. Boundary: `now == valid_until` IS lapsed
  (§10.3 end-exclusive window) — expiry at the exact boundary records.
- A wrong milestone is refused by name from every non-annotated state
  (48 combos here + the 4 EXPIRED-state combos in the engine matrix = the
  52 `InvalidExpiryMilestone` cells).
- `ExpiryMilestone::expected_for`-equivalent mapping is hand-transcribed in
  the suite (the implementation's version is private — the transcription is
  the anti-circular oracle).

### 3.5 Failure-trigger matrix — 84 cells, hand-computed

14 states × 6 triggers = 84; exactly the 6 annotated combos succeed into
FAILED with the trigger text as the journal reason; the other 78 are refused
with `InvalidFailureReason` naming the state and trigger. All 6 trigger
texts and all 4 milestone texts are pinned verbatim
(`the_failure_and_expiry_annotation_domains_are_exhaustive_and_pinned`).

### 3.6 Gate separation, SUPERSEDED vs CANCELLED, per-asset fleet semantics, clock discipline

- Every gate operation applied from a wrong state is refused with
  `WrongState` naming the operation (90 cells); APPROVED without FEASIBLE
  evidence is impossible by construction (FEASIBLE is reachable only through
  the passing feasibility gate).
- SUPERSEDED and CANCELLED are distinct states with distinct journal
  records (replacement-named evidence + `superseded_by` link vs operator
  reason + no link); both walks hand-computed.
- Partial fleet success is per asset (§14.3): two asset lifecycles on the
  same dispatch — one COMPLETED, one FAILED via NACK then FALLBACK — with
  per-asset journals and command IDs; no portfolio-level acknowledgement is
  inferred from the first responding device.
- Timestamps never regress (`TimestampRegression` refused by name, state and
  journal unchanged); equal timestamps allowed. The controlled clock is a
  fixed/advanceable test double — no wall-clock dependence (P4.15
  verification-method requirement).

## 4. What the suite does NOT cover (recorded, not silently assumed)

| Excluded | Why | Owner |
|---|---|---|
| Plugin-lifecycle `FROZEN_TRANSITIONS` / `CRASH_TRANSITIONS` (§5.6) | Different state machine; different crate; dependency rule forbids domain → kernel | P10.08 re-verifies the P7 suites (`crates/flexcore-kernel/tests/lifecycle_p7_07.rs`, `conformance_p7_21.rs`) |
| Payload-level §14.3 command-record invariants | Owned by the P1.06 `dispatch_command` suite | `crates/flexcore-domain/src/dispatch_command.rs` tests |
| Rejected-attempt AUDIT ROWS at the application/persistence layer | The domain refusal is in-memory and attributable; the audit wiring is application-level (P4.30) | Application/audit suites; P10.04 covers audited denials |
| Live shadow dry-run against a deployment | No deployment exists (P9.30); P9.19 is an UNRUN template | P9.19 execution (human gate, Stage 9 chain) |
| The expiry SWEEP/job/timer | State support only in the domain; the sweep is P4.15's | P4.15 suite / application wiring |

## 5. Ledger step 5 — suite result recorded with the candidate identifier

The suite result cell (§1) stays EMPTY until the human run completes against
the P10.18 candidate identifier. Any failure BLOCKS the release (P10.27)
until fixed and re-run. The independent re-run (ledger verification method)
uses the filed configuration: this crate, this test file, pinned fixtures —
and confirms the transition list matches §14.1 exactly (the transcription
vs report diff is part of the reviewer procedure below).

## 6. Filling commands (human, at execution time)

```sh
# Build and run the suite against the release candidate (human gate):
cargo test -p flexcore-domain --test command_transition_release_matrix
#   (also covered by `cargo test --workspace` in the P10.18 CI run)

# Mutation check (ledger P4.33 verification method, re-run for the release):
#   deliberately add a bogus transition to command_state.rs's match, confirm
#   the suite FAILS the cell comparison, revert. The suite demonstrates the
#   mechanism (a bogus edge is not in the transcribed freeze data, so the
#   per-cell comparison catches it); the actual add/confirm/revert is human.

# Anti-circularity spot-check: diff the suite's transcribed 27-edge table
# against report §14.1 (the guard text must be word-identical):
grep -n "frozen edge" crates/flexcore-domain/tests/command_transition_release_matrix.rs

# Repository verification after any edit:
python tools/verify_docs.py
python tools/validate_agent.py
python -I tools/check_dependency_rule.py
```

## 7. Reviewer-verification procedure

1. **Oracle independence:** confirm the suite's expected outcomes derive
   from the transcribed P0.11 table and the hand-computed trigger/milestone
   annotations, not from the implementation (§3.2 expectation function; §3.4
   transcription note). Confirm the transcription's guard text matches
   report §14.1 word-for-word.
2. **Exhaustiveness:** confirm the expectation function's `match` over
   `CommandState` has 14 arms and no wildcard; confirm the 374-cell /
   30-success / refusal-kind counts are asserted, and that the per-state
   success-target sets are asserted equal to the frozen successors.
3. **Matrix coverage:** confirm the table-level test iterates ALL × ALL and
   asserts 27/169/196; confirm the §14.3 classes, expiry path, trigger
   matrix, SUPERSEDED/CANCELLED distinction, per-asset fleet test, and clock
   tests exist as described (§3.3–§3.6).
4. **Additivity:** confirm no `src` file changed and no other crate's tests
   changed; the file compiles against the same public seams as the P4.10 /
   P4.33 suites (`flexcore-domain` + `flexcore-contracts` only).
5. **Honesty:** confirm §1 claims no run results, §2 records the
   FROZEN_TRANSITIONS/CRASH_TRANSITIONS terminology finding, and §4 lists
   the exclusions with owners.
6. **Status:** WRITTEN-UNRUN; the tick reads "implemented, awaiting human
   build/test against the release candidate."

## 8. Open items — traced to the carried registers

| Open item | Carried in |
|---|---|
| Candidate identifier `<PENDING P10.18>`; any candidate change re-opens this subtask | P10.18 (build); P10.01 §6.1 drift rule |
| P9.19 shadow dry-run UNRUN (both bands) — cross-referenced, not redone | `ledger/phase-09-pilot-qualification/19_command_dry_run.md` |
| No baseline run exists anywhere; the first human run IS the baseline | P9.41 finding, carried via P9.30 §2 |
| Rejected-attempt audit rows at the application layer | P4.30 follow-up; P10.04 (audited denials) |
| Invariant-9 composition map-gap (stale/missing model ≠ pass) | P9.35 / LR-152 (owner: application/plugin integration) — P10.06 owns the release-side check, not this suite |
| Plugin-lifecycle crash-path verification for the release | P10.08 (owns the kernel seam; P7 suites WRITTEN-UNRUN) |
| ScenarioRecord v3 non-synthetic refusal affecting replay of lifecycle scenarios | P9.38 — Phase 7 follow-up |

Build/test status (honest): **WRITTEN-UNRUN.** The tick on P10.03 reads
"implemented, awaiting human build/test."
