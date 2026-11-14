# P4.14 + P4.15 + P4.16 — Decision note: command invariants, expiry /
# cancellation / supersede, simulation-only dispatch adapter

- **Date:** 2026-11-10.
- **Subtasks:** P4.14, P4.15, P4.16 (`ledger_phase_4.md`). Report trace:
  §14.1–§14.3, §15.3, §17.5, §19.1, §21.4, §23 Stage 4, §24 steps 6–8.
- **Deliverables:** `crates/flexcore-application/src/command_record.rs`,
  `crates/flexcore-application/src/command_expiry.rs`,
  `crates/flexcore-application/src/dispatch_simulator.rs`,
  `crates/flexcore-application/src/lib.rs` (additive decls + re-exports),
  `crates/flexcore-application/tests/command_record.rs`,
  `crates/flexcore-application/tests/command_expiry.rs`,
  `crates/flexcore-application/tests/dispatch_simulator.rs`, this note.
- **Build/test status (honest):** NOTHING was built and NO test was run.
  The loop bans (no installs, no builds, no test execution) were held for
  the whole batch; every test file is written, never executed. All three
  subtasks are "implemented, awaiting human build/test" — never
  "tested"/"verified".
- **Reused, never restated:** the P4.10 lifecycle engine and its frozen-edge
  oracle (`flexcore_domain::command_lifecycle`); the P4.11–P4.13 command
  gates and their store/seam surfaces (`command_gates.rs`, decision record
  `11_13_command_gates.md`); the P1.06 `DispatchCommand` §14.3 validators;
  the P1.08 transactional-outbox assembly; the P3.22 supersede discipline
  (mirrored by `crates/flexcore-storage/src/schedule_revision.rs`); the
  P4.03 model-version health.

## 1. Recorded decisions

### P4.14 — §14.3 command invariants

| ID | Decision | Where |
|---|---|---|
| D-P4.14-1 | **The §14.3 record VALUE is the P1.06 `DispatchCommand`, reused; the ADDED piece is the flat field-level record** (`CommandRecordFields`, one named field per §14.3 bullet) plus the recorded completeness table `S143_FIELD_GROUPS` (17 ledger bullets, in ledger order). The flat record is the shape the composition root renders into the persisted `command_payload_json`; the typed unit values stay on the `DispatchCommand` (no numbers are re-parsed into the flat record). | `command_record.rs` (`CommandRecordFields`, `S143_FIELD_GROUPS`) |
| D-P4.14-2 | **Two completeness validators, any missing field is a NAMED error.** `validate_creation` (before PROPOSED) and `validate_issue` (before APPROVED → ISSUED) walk every field group in §14.3 order; a missing/invalid field blocks the transition with its own `MissingField`/`InvalidField` error naming the phase and the field — never silent, never defaulted. The recorded "optional for now" failure mode is mitigated structurally: every field of the table has its own named error and its own test, so dropping a field breaks the completeness tests loudly. The issue validator additionally refuses the `authz:pending:` placeholder (D-P4.11-3 — a pending authorization is never a decision) and an already-expired authorization. | `command_record.rs` (`validate_creation`, `validate_issue`, `PENDING_AUTHORIZATION_PREFIX`); tests/command_record.rs (28 per-field corruption cases + the issue-phase cases) |
| D-P4.14-3 | **The five §14.3 final-paragraph rejection rules are VERDICTS, not transitions.** `stale_rejection` (end-exclusive §10.3), `duplicate_conflict_rejection` (same idempotency key + different payload → REJECT; same key + same payload → idempotent replay, never a rejection), `out_of_sequence_rejection` (strictly monotonic per asset/channel; the first command of a channel never fails), `outside_capability_rejection` + `ramp_outside_capability_rejection` (target values and required ramp rates against the declared `AssetCapabilityWindow`; undeclared reactive capability makes a reactive target outside capability; SOC is structurally [0,1]), and `missing_evidence_rejection` (state-ranked journal evidence check). This module decides; the gates/adapters route verdicts through the lifecycle's rejection points — it never transitions state itself. | `command_record.rs` (`CommandRejectionRule` + one pure function per rule); per-rule tests |
| D-P4.14-4 | **Partial fleet success is per asset.** `AssetAcknowledgement` (one asset's command: awaiting/accepted/refused/timed-out/transport-failed, acceptance vocabulary only — there is no "delivered" state) and `FleetAcknowledgement` (computed from EVERY per-asset entry: `PartiallyAccepted` is the answer while ANY asset is outstanding). A portfolio-level acknowledgement is NEVER inferred from the first responding device (§14.3 final paragraph). An acceptance without its acceptance-record reference is not recordable. | `command_record.rs` (`AssetAcknowledgement`, `FleetAcknowledgement`); the multi-asset test |
| D-P4.14-5 | **The full journal replay is shared machinery.** `replay_full_lifecycle` re-walks a persisted journal through the domain engine (pinned per-entry clocks reproduce the persisted timestamps exactly) so every later operation applies from the true current state. Every frozen edge is mapped; an unmappable entry, a missing required reason/evidence, or an unparseable supersede link is a named `InconsistentJournal` error — fail-closed, never guessed around. P4.15 and P4.16 build on it. | `command_record.rs` (`replay_full_lifecycle`) |

### P4.15 — expiry, cancellation, supersede

| ID | Decision | Where |
|---|---|---|
| D-P4.15-1 | **Expiry is a validity-window SWEEP, decided once.** `sweep_validity_windows` is a PURE batch scan at one controlled-clock instant; `CommandExpiryService::run_sweep`/`apply_expiry` applies every decision through the store + the domain engine. The alternative — only a check at each transition attempt — was RECORDED but rejected as the sole mechanism: a command nobody touches would never expire. The domain's per-transition guard (`ValidityNotLapsed` — expiry never fires early; milestone must match the state) is RETAINED unchanged, and the check-at-transition path ALSO exists: the dispatch adapter's tracking poll checks the window at every poll and the domain engine re-checks at every attempt (P4.16). Only APPROVED / ISSUED / ACKNOWLEDGED / TRACKING can expire (the four states with frozen EXPIRED edges 10/15/20/24); the expiry time AND the milestone not reached are recorded (the journal reason IS the milestone text); EXPIRED has ONE outgoing edge (→ FALLBACK, frozen edge 27, where policy requires it — P4.17 owns the policies) and is NOT terminal — pinned against the frozen oracle in tests. The sweep's run cadence is a composition-root scheduling setting — **DEFAULT (recommendation): bounded interval, not decided here.** | `command_expiry.rs` (`sweep_validity_windows`, `ExpiryDecision`, `apply_expiry`, `run_sweep`); tests/command_expiry.rs (the sweep, the four milestones, the early-fire refusal, the EXPIRED-edge oracle test) |
| D-P4.15-2 | **Cancellation is an explicit operator/policy act.** APPROVED → CANCELLED and ISSUED → CANCELLED are the only frozen edges; every other state is a named `WrongState` error — never silently coerced, never used as a supersede synonym. Actor + reason are MANDATORY (the domain engine refuses empty/whitespace reasons; the actor is a required typed parameter). `DispatchCancelled` is emitted. | `command_expiry.rs` (`cancel`); tests (actor + reason, every-other-state refusals) |
| D-P4.15-3 | **Supersede is the replacement discipline.** Allowed exactly from the six frozen states (VALIDATED / FEASIBLE / APPROVED / ISSUED / ACKNOWLEDGED / TRACKING). The bidirectional links follow the P3.22/`schedule_revision.rs` pattern: the BACKWARD link is the journal row's `command:<uuid>` evidence (D-P4.10-3) and the replayed aggregate's `superseded_by`; the FORWARD link is the replacement's §14.3 field 17 — the persistence layer stores BOTH directions. Self-supersession is refused by the domain engine. The record is RETAINED for audit (append-only journal, nothing deleted). `DispatchSuperseded` is emitted — DISTINGUISHABLE from `DispatchCancelled` (different event, different state, different journal reason; tested side by side). | `command_expiry.rs` (`supersede`); tests (bidirectional links, retention, the superseded-vs-cancelled audit test) |
| D-P4.15-4 | **Event scope is caller-supplied, never guessed.** The frozen §15.3 dispatch payloads carry portfolio/site/asset IDs that the store seam's view does not hold, so `EventScope` comes from the command record the caller holds (the store row's scope columns); the tenant ALWAYS comes from the authenticated context (P0.16 rule 5). The service refuses to invent a scope. **Recorded payload gap:** the frozen payloads carry identity + time ONLY — actor/reason/references travel in the transition JOURNAL row (the evidence of record), fetched by command ID; no payload field was invented (P0.14/P1.09). | `command_expiry.rs` (`EventScope`, `EXPIRY_EVENT_PAYLOAD_GAP_NOTE`); the module header |
| D-P4.15-5 | **Clock discipline.** Every timestamp comes from the injected `LifecycleClock` — never a wall clock; the domain engine's timestamp-regression guard plus the sweep-only-at-controlled-instants design carry the §17.4 clock-drift risk. Time-quality monitoring remains a §17.4 follow-up (recorded, not implemented here). | `command_expiry.rs` (module header "Clock discipline") |

### P4.16 — simulation-only dispatch adapter

| ID | Decision | Where |
|---|---|---|
| D-P4.16-1 | **Issue: APPROVED → ISSUED only for a SIGNED, SEQUENCED command sent to the SIMULATOR.** The signing identity is the `SimulationKeyring` — a DEV/SIMULATION keyring producing a deterministic, NON-cryptographic signature reference (`sim-sig:<key_id>:<command_id>`, recorded per §14.3 field 15 as the ISSUED evidence). It is NOT a production PKI and MUST be replaced by the Stage 6/9-qualified signing identity before any non-simulation use; no security property beyond traceability is claimed. The adapter REFUSES an unsigned, unsequenced, or expired command with named errors (`check_issue_refusals`; the unsigned/unsequenced cases are additionally unrepresentable through the P1.06 constructor). Before ISSUED it runs the P4.14 issue re-validation over the FULL §14.3 record, cross-checks the caller's P4.13 authorization record against the journal's APPROVED evidence, and runs the P4.12 hook (`ensure_command_issueable`: the feasibility evidence must bind the ACTIVE model hash, §17.5 invariant 3). | `dispatch_simulator.rs` (`SimulationKeyring`, `check_issue_refusals`, `issue`); the refusal/criterion tests and the end-to-end test |
| D-P4.16-2 | **Acknowledgement is application-layer ACCEPTANCE, never physical delivery.** ISSUED → ACKNOWLEDGED only on the simulator's acceptance (mandatory acceptance-record reference as the journal evidence); a NACK moves ISSUED → FAILED (`Nack`), a transport timeout → FAILED (`TransportTimeout`), other transport failures → FAILED (`TransportFailure`) — the frozen triggers. Every type, state, and docstring says "acceptance"; the acceptance-vs-delivery boundary is pinned by test. | `dispatch_simulator.rs` (`SimulatedAcceptance`, the `issue` acknowledgement arms); the acknowledgement test |
| D-P4.16-3 | **Tracking: the acceptance criterion is DEPLOYMENT CONFIGURATION, not code.** `TrackingAcceptanceCriterion` (quantity, absolute tolerance, minimum satisfied samples, tracking timeout) is supplied per asset class by the composition root; this module validates it and applies it, never hard-codes a threshold. ACKNOWLEDGED → TRACKING requires an observed sample (edge 18 — none is a named error, no transition); the recorded evaluation order: (1) any sample outside the tolerance → FAILED (tracking deviation — a recorded deviation fails the command even if the asset recovers); (2) at least `min_satisfied_samples` within-tolerance observations in the poll → COMPLETED; (3) the timeout elapsed since the → TRACKING journal timestamp (the evidence of record, not an adapter guess) → FAILED (tracking timeout); (4) else keep tracking. Validity ending → EXPIRED with the state-matched milestone (`BeforeTrackingCompletion` from ACKNOWLEDGED, `BeforeCompletion` from TRACKING). A `PowerSetpoint`'s reactive component is recorded on the command but not separately tracked — recorded limitation. Observed-quantity vs criterion-quantity mismatch is fail-closed, never coerced. **The test fixtures' numeric values (0.2 MW, 1–2 samples, 5 000 ms) are TEST fixtures, not product defaults — DEFAULT, not decided.** | `dispatch_simulator.rs` (`TrackingAcceptanceCriterion`, `evaluate_tracking`, `poll_tracking`); the tracking tests |
| D-P4.16-4 | **§24 step 8 simulations run through the transport port's scripted results.** The recorded mapping to the Phase 2 simulator's fault vocabulary: `drop` → no acceptance (timeout), `source_invalid` → NACK, `out_of_range` → an out-of-tolerance tracking sample. The composition root's transport implements this mapping over `flexcore-simulator`; the test fake scripts the same semantics directly (flexcore-application cannot name the simulator crate — the dependency rule). | `dispatch_simulator.rs` (`SimulatorDispatchTransport`, the module header); the scripted-transport fake in tests |
| D-P4.16-5 | **Per-asset partial success.** Every command is single-asset (the §14.3 scope); a fleet operation is a set of commands and its acknowledgement is the P4.14 `FleetAcknowledgement` — computed per asset, never inferred from the first responding device. | `dispatch_simulator.rs` (`IssueOutcome.acknowledgement`); the partial-fleet test |
| D-P4.16-6 | **The hard target guard.** `SimulatorDispatchAdapter::configure` accepts ONLY `AdapterTargetCandidate::Simulator`; every `AdapterTargetCandidate::NonSimulator` kind (gateway, protocol adapter, device, field endpoint) is REFUSED with a named error, and the guard test exercises every kind. The transport port (`SimulatorDispatchTransport`) carries deliver + observe ONLY — there is no device-write method to grow, and the structural test pins the port shape and scans the source for forbidden write-path identifiers. Field dispatch requires Stage 6 (protocol adapters) + Stage 9 qualification and operator-approved control (v1.0.0); `SIMULATION_ONLY_STATEMENT` says so verbatim. | `dispatch_simulator.rs` (`configure`, `AdapterTargetCandidate`, `SIMULATION_ONLY_STATEMENT`); the guard test, the structural port test, the source scan |

## 2. What the guard chain covers (the "target guard" scope, recorded)

The dispatch target is guarded at THREE layers, and the tests walk all
three:

1. **Asset guard (P4.11 validation gate, before any issue):** the target
   asset must exist in the caller's tenant scope (a foreign and a missing
   asset are indistinguishable — P0.16 rule 5), carry a capability model,
   and declare the targeted quantities. Unknown/unusable-asset rejections
   are tested in `tests/dispatch_simulator.rs` through the same harness,
   proving the guard precedes any issue. Asset availability/deactivation is
   the P2.16 availability rule upstream of the gates — the command-gate
   seam's unusable outcomes are the two shown (unknown-in-tenant-scope,
   capability-model-missing); there is no per-asset "simulation-mode" flag
   in this phase's seam.
2. **Command-state/tenancy guard (adapter, at issue):** the adapter loads
   the command THROUGH the tenant-scoped store seam — a cross-tenant caller
   gets `NotFound` (existence never leaks), a wrong-state command gets a
   named `WrongState`, an unsigned/unsequenced/expired command gets its
   named refusal.
3. **Transport-target guard (adapter, at configuration):** only the
   simulator is an acceptable target (D-P4.16-6) — the "simulation-mode"
   requirement is expressed as the simulator-only transport guard, not a
   per-asset attribute.

## 3. Defects found and fixed in the interrupted partial work (recorded)

The predecessor was quota-killed mid-wave; completing the wave required
these surgical fixes (all in the wave's own files, none elsewhere):

1. `command_expiry.rs` called `self.persist(...)` but never defined it —
   the `persist` method was added (mirroring the gates'/adapter's seam
   write, returning the receipt the outbox write pairs with).
2. `dispatch_simulator.rs` derived `Eq` on `TrackingOutcome`, whose
   `TrackingVerdict` field carries `f64` — `Eq` cannot hold. Derived
   `PartialEq` only (with a comment).
3. `dispatch_simulator.rs` `emit_for_command` carried an unused `ctx`
   parameter — renamed `_ctx` (warning hygiene).
4. `tests/command_expiry.rs` called a nonexistent
   `Actor::new("").unwrap_err().into_actor_guard()` (a non-compiling
   leftover) — the test was rewritten to assert the type-level fact (an
   actor is a required, validated parameter) plus the actor recording.
5. `tests/command_expiry.rs` contained `unimplemented!` in the fake store's
   `insert_proposed` — replaced with a fail-closed `Backend` error (the
   suite seeds records directly; a creation attempt is a suite error).
6. `tests/command_expiry.rs` seeded terminal states (COMPLETED, REJECTED,
   FAILED, EXPIRED, CANCELLED, SUPERSEDED, FALLBACK) through a forward-only
   journal builder, so the store view never showed the seeded state and the
   loop tests would have failed — explicit replay-clean journal builders
   for the terminal states were added (`seed_journal` + per-state builders)
   and the affected tests rewired to them (with a state sanity assertion).

## 4. Gaps and limitations (recorded, none silently filled)

- **Nothing was built or run.** No `cargo build/check/test/clippy/fmt`, no
  installs, no test execution. The suites are written against the actual
  public APIs (read end-to-end before writing) but are unexecuted — the
  human build/test pass is the gate.
- **The transport implementation over `flexcore-simulator` is the
  composition root's** (the dependency rule forbids flexcore-application
  from naming it); the mapping of the simulator's fault vocabulary is
  recorded (D-P4.16-4) but the real adapter lands with the API/composition
  wiring (P4.30/P4.31 integration layer).
- **The event-payload gap (D-P4.15-4):** actor/reason/references travel in
  the journal, not the frozen payloads.
- **No production signing identity, no mTLS, no replay-protected transport**
  — P4.28's scope; the simulation keyring is dev-only and labelled as such.
- **Fallback (FAILED/EXPIRED → FALLBACK) is the transition only here;** the
  per-asset/site policies, the bounded hold, and the recovery rule are
  P4.17/P4.18's.
- **Research cut-off:** no external sources were consulted; the 7 November
  2026 cut-off is unchanged.

## 5. File/test index

| File | Content |
|---|---|
| `crates/flexcore-application/src/command_record.rs` | P4.14: the §14.3 completeness table + flat record, creation/issue validators, the five rejection-rule verdicts, per-asset/fleet acknowledgement, the shared full replay |
| `crates/flexcore-application/src/command_expiry.rs` | P4.15: the pure validity-window sweep + the expiry/cancel/supersede service over the store seam and the domain engine, with the frozen events |
| `crates/flexcore-application/src/dispatch_simulator.rs` | P4.16: the simulator transport port, the simulation keyring, the issue refusals, the tracking criterion + evaluation, the adapter (issue/ack/tracking), the hard target guard |
| `crates/flexcore-application/src/lib.rs` | Additive module decls + re-exports for the three modules |
| `crates/flexcore-application/tests/command_record.rs` | P4.14 suite: the 17-bullet table pin, 28 field corruptions, the issue-phase refusals, per-rule rejection tests, the multi-asset fleet test, the milestone mirror |
| `crates/flexcore-application/tests/command_expiry.rs` | P4.15 suite: the pure sweep (exactly the four expirable states, end-exclusive boundary), milestone-per-state expiry, early-fire refusal, EXPIRED-edge oracle pin, cancel actor/reason + every-other-state refusals, supersede bidirectional links + retention, SUPERSEDED-vs-CANCELLED audit distinction |
| `crates/flexcore-application/tests/dispatch_simulator.rs` | P4.16 suite (NEW): the simulation-only statement, the structural port test (exactly deliver+observe, no write path), the target-guard test (every non-simulator kind), the issue-refusal and criterion-validation unit tests, the pure tracking-order test, the end-to-end PROPOSED→…→COMPLETED lifecycle, the acceptance-vs-delivery test, NACK/timeout → FAILED, deviation/timeout at tracking, no-telemetry and quantity-mismatch fail-closed errors, expired-approval refusal + sweep expiry, expiry at the poll during tracking, supersede during tracking + adapter refusal, wrong-state/unknown-command/cross-tenant refusals, unknown/unusable-asset gate rejections, the per-asset partial-fleet test |

## 6. Review status and verification

- **Build/test sign-off: PENDING (human).** Nothing in this batch was
  compiled or executed (loop bans held); the ledger checkboxes are ticked
  as "implemented, awaiting human build/test" only.
- **Security review sign-off: PENDING** (standing status for this phase).
  Security review should confirm D-P4.16-1 (dev-only keyring, no security
  claim), D-P4.16-6 (the target guard), and the transport-port shape.
- **Determinism:** the suites use controlled clocks everywhere; no wall
  clock, no randomness. The §24 step 8 scripts are fixed inputs.
- **AGENTS.md §6 tools:** the canonical report under `docs/` was not
  touched by this batch; the §6 checks were run once at wave close and the
  output is recorded in the wave report (they scope the report and the
  agent harness, not the crates).
