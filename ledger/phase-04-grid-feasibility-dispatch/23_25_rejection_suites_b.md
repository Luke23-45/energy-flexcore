# P4.23 + P4.24 + P4.25 — Decision note: deterministic rejection suites for
# unauthorized, duplicate, and expired commands (§23 Stage 4 exit gate, part B)

- **Date:** 2026-11-10.
- **Subtasks:** P4.23, P4.24, P4.25 (`ledger_phase_4.md`). Report trace:
  §23 Stage 4 exit gate, §14.1–§14.3, §14.4, §15.3–§15.5, §16.2,
  §17.3–§17.5 (invariants 4, 8), §19.2.
- **Deliverable files:** `crates/flexcore-application/tests/rejection_unauthorized.rs`
  (P4.23), `crates/flexcore-application/tests/rejection_duplicate.rs` (P4.24),
  `crates/flexcore-application/tests/rejection_expired.rs` (P4.25), this note.
  No source file was touched (test-only suites over EXISTING public APIs;
  `crates/flexcore-application/src/*` and `lib.rs` untouched — file
  ownership held).
- **Build/test status (honest):** NOTHING was built, run, or installed. No
  `cargo build/check/test/clippy/fmt`, no test execution, no installs — the
  loop bans were held for the whole batch. The three suites are WRITTEN,
  never executed. Status of all three subtasks: **implemented, awaiting
  human build/test** — never "tested"/"verified"/"passed".

## 1. Implementation model (same pattern as the established suites)

Test-only suites over existing public APIs with in-test fakes, matching the
precedents (`tests/command_gates.rs`, `tests/command_record.rs`,
`tests/command_expiry.rs`, `tests/fallback_policy.rs`,
`tests/dispatch_simulator.rs`): `FakeEventOutbox` (`.len()`/`.writes()`),
`CountingIdGenerator`, `StateChangeReceipt::new(entity, op, Option<TenantId>)`,
tenant contexts through the single claims path (P0.16 rule 5),
`CommandGateStore`/`CommandGateView`, `replay_full_lifecycle`,
`command_state::{CommandState, try_transition}` (the P0.11 frozen oracle),
`SimulationKeyring`, and controlled clocks everywhere (never wall-clock;
every faked seam fails closed by construction).

## 2. Pre-registered fixture expectation tables (UNRUN)

Every expectation below is PRE-REGISTERED and was NOT observed by execution.
The suites encode the tables as code (`pre_registered_*` helpers) that the
tests assert against verbatim.

### P4.23 — unauthorized (`tests/rejection_unauthorized.rs`)

| Fixture | Pre-registered outcome (named authorization error) | Audit/denial record asserted |
|---|---|---|
| Approval by an actor WITHOUT the required role (least-privileged roles: `viewer`, `operator`, `platform-admin`, unknown role) | `authorization_denied:no matching rule for role <role>` → FEASIBLE → REJECTED at the authorization gate | Journal row: gate `Authorization`, evidence `authz-denial:command/<id>@<t>`, denying actor + reason; one `DispatchRejected` |
| Approval by the PROPOSER where separation of duties is configured (recorded default: ENFORCED) | `self-approval prohibited (separation of duties, §17.4)` → REJECTED | Same denial-record shape, denying actor = the proposer |
| Cross-tenant approval attempt | Blocked: `NotFound` — "does not exist in this tenant scope" (existence never leaks, P0.16 rule 5); no journal row, no event for the foreign caller | The named refusal is the caller-visible record; its AuditSink append is the P4.28/P4.30 wiring (open item, §4) |
| Approval with an EXPIRED authorization scope (boundary `expiry == now` and strictly past — end-exclusive §10.3) | `authorization_expired_before_decision` → REJECTED | Same denial-record shape |
| Command issue attempted without the APPROVED state (from FEASIBLE) | Blocked with `WrongState` at BOTH enforcement points: the P4.12 issue hook and the P4.16 adapter (`operation "issue"`); zero delivered envelopes | The named `WrongState` refusal; no journal row; no event |
| CONTROL: the correct role (`tenant-admin`, the only recorded `command.approve` grant, asset/site scope) | APPROVED — the fixtures fail on least-privileged roles, not on a broken harness; approval alone never issues | The P4.13 `AuthorizationDecisionRecord` (actor, role, scope, limits, expiry, applied separation-of-duties, covered evidence) |

Aggregate exit checks in-suite: every fixture's outcome equals the
pre-registered table; every in-scope denial has its journal denial record +
`DispatchRejected`; NO fixture reaches ISSUED (journal check + the P0.11
frozen-edge oracle: no REJECTED/FEASIBLE/EXPIRED → ISSUED edge exists);
fixtures run twice with identical outcomes.

### P4.24 — duplicate (`tests/rejection_duplicate.rs`)

| Fixture | Pre-registered outcome | One-effective-command evidence |
|---|---|---|
| (a) EXACT REPLAY (same idempotency key, same payload) | Idempotent replay — deduplicated to the single original record; never a rejection | Store records: 1 before and 1 after the replay; end-to-end: exactly 1 delivered envelope, exactly 1 `DispatchIssued`, 1 command per key (the adapter refuses the re-issue of the ISSUED command by state) |
| (b) Same idempotency key, CONFLICTING payload | REJECTED — the P4.14 `duplicated_conflicting_payload` verdict + the store seam refuses a second record under the key | Records: 1 after the conflicting attempt; the refusal is the named `Conflict` |
| (c) Same asset/channel, OUT-OF-SEQUENCE version (equal and lower) | REJECTED — the P4.14 `out_of_sequence` verdict + the store sequence seam refuses with the named `out_of_sequence` code; only a strictly greater sequence records; the FIRST command of a channel never fails | `last_recorded` advances only on strictly greater sequences |
| (d) REDELIVERED event carrying an already-processed command | Idempotent consumer — no duplicate side effect: the replayed ISSUED transition is refused by the store compare-and-swap, the redelivered outbox write under the existing `dispatch_issued:<command uuid>` key is refused (`DuplicateKey`), the re-issue is refused by state | Exactly 1 command per key, 1 `DispatchIssued`, 1 envelope |

Plus the DERIVATION test: the idempotency key must cover the FULL payload +
sequence. The suite pins the property over the real §14.3
`CommandRecordFields` shape — a reference derivation is fingerprinted over
EVERY field, determinism is asserted across calls, and every single-field
perturbation (including `sequence`) changes the key; the derivation then
feeds `duplicate_conflict_rejection` to classify exactly the conflicting
case. All four fixtures run twice in-suite with identical outcomes.

### P4.25 — expired (`tests/rejection_expired.rs`)

| Fixture (stage) | Pre-registered state + milestone | Post-expiry assertions | Fallback per P4.17–P4.18 |
|---|---|---|---|
| APPROVED expiring before issue | APPROVED → EXPIRED; milestone `BeforeIssue`; the journal reason IS the milestone text; expiry time recorded (controlled clock) | The adapter refuses the issue BEFORE the sweep (`ExpiredCommandRefused`); after expiry no ISSUED/ACKNOWLEDGED/COMPLETED row; adapter issue/poll → `WrongState(Expired)`; `try_transition` oracle: EXPIRED's ONLY outgoing edge is → FALLBACK; exactly one `DispatchExpired` | Named OnExpiry policy → `Activated` (EXPIRED → FALLBACK, frozen edge 27; `DispatchFallbackActivated`; audit-recorded) |
| ISSUED expiring before acknowledgement | ISSUED → EXPIRED; milestone `BeforeAcknowledgement` | Poll from ISSUED → `WrongState` (nothing acknowledges it as fresh); sweep expiry; same no-progress + oracle checks; exactly one `DispatchExpired` | Named OnExpiry policy → `Activated` |
| ACKNOWLEDGED expiring before tracking completes | ACKNOWLEDGED → EXPIRED at the poll (check-at-transition path beside the sweep); milestone `BeforeTrackingCompletion` | No TRACKING row recorded (no telemetry); same no-progress + oracle checks; exactly one `DispatchExpired` | CONTROL: empty registry → the explicit default entry `InterventionRequired` — NO transition, decision audit-recorded |
| TRACKING expiring before completion | TRACKING → EXPIRED at the poll; milestone `BeforeCompletion` | The criterion was never satisfied (no COMPLETED row); same no-progress + oracle checks; exactly one `DispatchExpired` | Named OnExpiry policy → `Activated` |
| (guard) Expiry never fires early; boundary end-exclusive | `ValidityNotLapsed` 1 ms before the bound; expiry allowed exactly at `now == valid_until` (§10.3) | — | — |
| (guard) Fallback fires ONLY where the policy requires it | A FAILED-only policy evaluated on EXPIRED → `TriggerNotConfigured`: no transition, no event, decision audit-recorded | EXPIRED untouched | P0.11 edge-27 discipline |

## 3. Recorded decisions

| ID | Decision | Where |
|---|---|---|
| D-P4.23-1 | **The §16.2 denial record of record is the journal row + `DispatchRejected`.** Every in-scope authorization denial journals FEASIBLE → REJECTED with the `authz-denial:command/<id>@<t>` evidence reference, the denying actor, and the named reason; `DispatchRejected` is emitted. The frozen payloads carry identity + time only (the recorded payload gap, D-P4.15-4) — actor/reason travel in the journal, which the suite asserts verbatim. | `tests/rejection_unauthorized.rs` (`assert_denial_record`) |
| D-P4.23-2 | **The evaluator fake mirrors the RECORDED Phase 4 default grant matrix** (`flexcore-authz::phase4_policies::phase4_command_policies`: `command.approve` → TenantAdmin only, at site/asset kinds; deny-by-default). The real crate's evaluation is tested in `crates/flexcore-authz/tests/phase4_policies.rs`; this suite proves the GATE consumes a deny decision into a REJECTED, audit-recorded outcome. The tenant-level-approval denial (no tenant-kind grant) is a least-privilege fixture, not a new policy decision. **DEFAULT** (the grant matrix is the recorded recommended default, reviewable). | `tests/rejection_unauthorized.rs` (`Phase4MatrixAuthz`); `phase4_policies.rs` |
| D-P4.23-3 | **The cross-tenant approval attempt is refused as `NotFound`** (the tenant-scoped store seam; P0.16 rule 5 — a foreign and a missing command are indistinguishable). The named refusal text is the audit-ready denial; its `AuditSink` append is P4.28/P4.30 wiring (open item, §4) — recorded, not silently assumed. | `tests/rejection_unauthorized.rs` (cross-tenant fixture) |
| D-P4.24-1 | **The duplicate rules are enforced at THREE existing seams**, and the suite pins each: (1) the P4.14 pure verdicts (`duplicate_conflict_rejection`, `out_of_sequence_rejection` — the same-key-different-payload rejection and the strictly-monotonic sequence rule); (2) the store seam's insert/compare-and-swap contract (the fake states the contract the P4.30 store implementation must satisfy: dedup-to-original on identical payload, `Conflict` on a conflicting payload, named `out_of_sequence` refusal); (3) the outbox port's idempotency-key uniqueness (`EventOutboxError::DuplicateKey`, the P1.12 unique constraint — republish-after-crash safe, §15.5). **DEFAULT** (the seam contract is the suite's stated expectation pending P4.30). | `tests/rejection_duplicate.rs` |
| D-P4.24-2 | **The derivation property, not a production derivation function.** The §14.3 idempotency-key derivation is composition-root machinery; the ledger's risk ("key derived from mutable fields") is mitigated by a REQUIRED PROPERTY pinned in-suite over the real `CommandRecordFields` shape: the key covers the FULL payload + sequence, is deterministic, and every single-field perturbation changes it. The P4.30 implementation must satisfy the same property (re-verification, §4). The reference derivation in the suite uses only `std` (`DefaultHasher`). | `tests/rejection_duplicate.rs` (`derive_idempotency_key`) |
| D-P4.24-3 | **The NATS-inbox redelivery dedup is cross-referenced, not duplicated**: the durable consumer-side idempotency is the P1.16 fault slice (`crates/adapters/nats/tests/fault_slice.rs`, case (a)); this suite pins the application-seam half (CAS refusal + outbox `DuplicateKey` + adapter state refusal). | `tests/rejection_duplicate.rs` (fixture d) |
| D-P4.25-1 | **Expiry is tested at BOTH recorded paths**: the P4.15 sweep (`apply_expiry`/the pure `sweep_validity_windows` milestone mapping) AND the check-at-transition path (the adapter's tracking poll expires the command at the poll — the ledger's risk mitigation "expiry is checked at every transition attempt and by the sweep"). The domain `ValidityNotLapsed` guard and the §10.3 end-exclusive boundary are re-pinned. | `tests/rejection_expired.rs` (all fixtures + guard test) |
| D-P4.25-2 | **The fallback consequence fires only where policy requires it.** Named OnExpiry policies activate EXPIRED → FALLBACK through the frozen edge 27 (audit-recorded, `DispatchFallbackActivated`); an empty registry resolves to the explicit default entry (`InterventionRequired`, no transition); a FAILED-only policy on EXPIRED is `TriggerNotConfigured`. REUSED from P4.17–P4.18 unchanged — no fallback code was touched. | `tests/rejection_expired.rs`; `fallback_policy.rs` |
| D-P4.25-3 | **No-progress discipline is structural.** After every expiry: no ISSUED/ACKNOWLEDGED/COMPLETED journal row after the EXPIRED entry, adapter issue/poll refuse with `WrongState(Expired)`, and the P0.11 frozen-edge oracle pins that EXPIRED is NOT terminal with exactly ONE outgoing edge (→ FALLBACK). | `tests/rejection_expired.rs` (`assert_no_progress_after_expiry`) |

## 4. Dependency note and the recorded open item

The ledger lists P4.23's inputs as "P4.13, P4.28" and P4.24's inputs as
"P4.14, P4.30". P4.13 and P4.14 are DONE; **P4.28 (dispatch security
controls) and P4.30 (audit/events/persistence wiring) are NOT yet
implemented** (later waves). Handled honestly, per the instruction pattern
(the P3.29 F2 precedent):

- **OPEN ITEM (P4.31 re-verifies P4.23's wiring; P4.29 re-verifies P4.24's
  wiring):** the suites exercise the security/idempotency mechanisms at
  their EXISTING enforcement points — the P4.13 authorization gate, the
  tenant-scoped store seam, the P4.14 rejection-rule verdicts + issue
  re-validation, the P4.16 adapter's `SimulationKeyring`/sequence/state
  guards, the P1.09 outbox idempotency-key uniqueness, and the P0.11
  frozen-edge oracle — with in-test fakes. When P4.28/P4.30 land, their
  re-verification must confirm: (a) the real `flexcore-authz` policy set is
  wired into the gate's evaluator seam with the same recorded matrix;
  (b) cross-tenant/out-of-scope denials are appended to the audit trail
  through the `AuditSink`; (c) the production store implements the
  duplicate/sequence seam contract this suite states; (d) the production
  idempotency-key derivation satisfies the full-payload+sequence coverage
  property. Until then the suites' seam-contract fakes are the executable
  specification.
- **P4.25 has no dependency gap** (inputs P4.15, P4.16 — both done; the
  suite drives the P4.15 service and the P4.16 adapter as implemented).

## 5. File/test index

| File | Content |
|---|---|
| `crates/flexcore-application/tests/rejection_unauthorized.rs` | P4.23 suite: the pre-registered denial table; least-privileged-role denials (viewer/operator/platform-admin/unknown); tenant-kind-approval denial; the correct-role control; proposer self-approval (separation of duties); cross-tenant block (existence never leaks); expired authorization scope (boundary + past); issue without APPROVED at both enforcement points; per-fixture audit-denial record + no-ISSUED checks; frozen-edge oracle; two-run determinism |
| `crates/flexcore-application/tests/rejection_duplicate.rs` | P4.24 suite: the derivation property (full payload + sequence, deterministic, perturbation-sensitive, feeds the verdicts); fixture (a) exact replay (seam dedup + end-to-end no-second-issue); fixture (b) conflicting duplicate; fixture (c) out-of-sequence; fixture (d) redelivered event (CAS + outbox `DuplicateKey` + state refusal); one-effective-command-per-key checks; two-run determinism |
| `crates/flexcore-application/tests/rejection_expired.rs` | P4.25 suite: four stage fixtures (APPROVED/ISSUED/ACKNOWLEDGED/TRACKING → EXPIRED with the milestone recorded, one `DispatchExpired` each, no-progress assertions, frozen-edge oracle, fallback where policy requires + the default-entry and TriggerNotConfigured controls), the never-early/end-exclusive guard, controlled clocks throughout |

## 6. Honest limitations

- **Nothing was built, run, installed, or executed.** The loop bans held
  for the whole batch; every table row above is PRE-REGISTERED and UNRUN —
  the human build/test pass is the gate. Nothing in this note may be read
  as a test result.
- The evaluator fake (D-P4.23-2) and the duplicate/sequence store-seam
  contract (D-P4.24-1) are in-test fakes stating the expected seam
  contract; the production wiring is P4.28/P4.30's (open item, §4).
- The `flexcore-authz` and `flexcore-audit` crates are not dependencies of
  `flexcore-application` (the dependency rule), so the suites cannot drive
  the real `PolicySet` or append real audit rows; the recorded matrix is
  mirrored in-test and the real crate's tests are cited, not duplicated.
- The NATS-inbox consumer dedup (fixture d's transport half) lives in the
  P1.16 fault slice in `crates/adapters/nats` and is cross-referenced,
  not re-tested here (different crate; the dependency rule forbids naming
  it from `flexcore-application` tests).
- Research cut-off: no external sources were consulted; the 7 November
  2026 cut-off is unchanged.
