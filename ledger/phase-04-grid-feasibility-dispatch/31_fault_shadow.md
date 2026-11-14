# Decision record — P4.31 Fault injection, shadow evaluation, and integration tests (§19.2 layers 4, 6, 9)

**Status:** implemented, awaiting human build/test. **Nothing was built,
installed, run, or executed** — no `cargo build/check/test/clippy/fmt`, no
docker, no database, no broker, no package install (loop bans; AGENTS.md
working agreement). Every test below is written and pre-registered UNRUN;
the human build/test pass is the gate. The phrase "pre-registered" below
means the expectations were written together with the suites and are
recorded here BEFORE any execution exists — none has run.

## 1. What was built

| Artefact | File | Content |
|---|---|---|
| Shadow-evaluation module (NEW) | `crates/flexcore-application/src/shadow_evaluation.rs` | The §21.4 comparison method: the read-only `ShadowBaselineSource` port (declared HERE — flexcore-application cannot name flexcore-simulator; the composition root implements it over the Phase 2 simulator), the fail-closed per-instant classification, the conservative overall verdict, and `ShadowReport` which CARRIES its limits/method/nothing-issued statements as fields. Registered additively in `crates/flexcore-application/src/lib.rs`. |
| Shadow-evaluation tests (NEW) | `crates/flexcore-application/tests/shadow_evaluation.rs` | 14 tests: agreement/divergence/missing/stale/future-stamped/quantity-mismatch classification, conservative composition, config validation, empty/future instants, baseline-source failure, statements-on-report, determinism double-run, read-only port-call shape, structural source scan (no `EvidenceReference`, no delivery path, one observe-only port method). |
| Fault-injection suite (NEW) | `crates/flexcore-application/tests/fault_injection.rs` | 12 tests over the P4.10–P4.18 public seams (outbox, transport, runner, expiry, fallback) — the per-fault table is §2. Header: `// P4.31 — Fault injection and dispatch resilience`. |
| Integration tests (NEW) | `crates/adapters/postgres/tests/dispatch_integration.rs` | 3 env-gated tests (`FLEXCORE_TEST_DATABASE_URL`, mirrors `persistence_wiring.rs`; skip cleanly when unset) over the §24 steps 5–8 path on real PostgreSQL. UNRUN. |
| CI wiring (additive) | `.github/workflows/ci.yml` | Two new steps in the existing `scenario-regression` job running the fault-injection and shadow-evaluation suites. No existing job or step changed. |
| Ledger checkbox | `ledger/phase-04-grid-feasibility-dispatch/ledger_phase_4.md` | P4.31 ticked as "implemented, awaiting human build/test". NEVER "tested"/"verified"/"passed". |
| This record | `ledger/phase-04-grid-feasibility-dispatch/31_fault_shadow.md` | Pre-registered (UNRUN) expectations, the shadow method + limits, the human-run gate inputs, decisions (DEFAULT), limitations. |

No existing `src/**` file was edited (the lib.rs addition is the additive
module registration only); no Phase 5 file was touched.

## 2. Pre-registered fault matrix (all UNRUN)

Every fault's expected outcome is a named rejection, a hold, or a fallback
— NEVER a silent pass. Faults are modelled at the adapter/outbox SEAM level
with in-test fakes over the public ports (the P1.16 pattern;
flexcore-application cannot name the broker, the simulator, or any
infrastructure crate). Deterministic: controlled clock everywhere, scripted
fakes, no randomness, no seeds.

| ID | Fault | Seam | Pre-registered outcome (UNRUN) | Test |
|---|---|---|---|---|
| F1 | Dropped NATS message (publish attempted, broker never acks) | outbox seam (`EventOutbox`) | the entry stays PENDING with its attempt count (no loss, never silently dropped); the command state is store-authoritative and unaffected (hold); recovery drains the entry | `f1_a_dropped_publication_stays_pending_with_its_attempt_count_and_drains_on_recovery` |
| F1b | Duplicated NATS message (same idempotency key) | outbox seam | the second append is refused with the named `DuplicateKey`; exactly ONE effective entry per key (§15.4/P4.24 semantics at the seam) | `f1b_a_duplicated_publish_under_one_key_is_refused_exactly_one_effective_entry` |
| F2 | Reordered delivery (seq 3 before seq 2 on one asset channel) | receive-point transport (`VerifyingDispatchTransport`, P4.28) | the out-of-order envelope is refused as an application-layer NACK (`out_of_order_sequence`); the inner transport NEVER sees it; the guard NEVER advances on a refusal; the next higher sequence is admitted | `f2_a_reordered_delivery_is_refused_at_the_receive_point_and_the_guard_never_advances` |
| F4 | Clock skew forward past validity, at issue | controlled clock | the issue is REFUSED (`ExpiredCommandRefused` — the adapter never expires, the sweep does); the sweep EXPIRES the command with the milestone not reached (`BeforeIssue`) recorded | `f4_clock_skew_forward_past_validity_refuses_the_issue_and_the_sweep_expires` |
| F5 | Clock skew forward during tracking | controlled clock | the poll EXPIRES the command (`BeforeCompletion`) even with an in-tolerance sample scripted — expiry wins over completion | `f5_clock_skew_during_tracking_expires_the_command_never_completes_it` |
| F6 | Stale telemetry at the feasibility check | feasibility runner seam (scripted double, the P4.22 pattern) | the `FailClosed` verdict REJECTS the command with `fail_closed:telemetry:STALE…` named; no feasibility evidence; `DispatchRejected` + `FeasibilityCheckCompleted` emitted (§17.5 invariant 9, P4.08) — never a pass | `f6_stale_telemetry_at_the_feasibility_check_fails_closed_with_the_trigger_named` |
| F7 | Missing telemetry in tracking | simulator transport | the poll is the named `NoTelemetryObserved` error with NO transition (the command HELD in ACKNOWLEDGED); the sweep later EXPIRES it (`BeforeTrackingCompletion`); a silent feed is never read as completion | `f7_missing_telemetry_holds_the_command_and_the_sweep_expires_it` |
| F8 | Unavailable asset at issue (transport down) | simulator transport | ISSUED → FAILED with the frozen `TransportFailure` trigger (journal reason "transport failure"); the site fallback policy ACTIVATES FALLBACK (frozen edge, audit record, `DispatchFallbackActivated`); the bounded hold ESCALATES after its interval (audit-record escalation, NOT a transition — FALLBACK is terminal); a direct FALLBACK → ISSUED issue attempt is refused | `f8_an_unavailable_asset_fails_the_issue_and_the_fallback_policy_activates_and_escalates` |
| F9a | Solver timeout (runner seam FAILS) | feasibility runner seam | the named `Seam` error surfaces with NO verdict, NO transition, NO event — the command HELD in VALIDATED (a timeout is never "no violation found", the P4.08 risk note) | `f9a_solver_timeout_seam_error_holds_the_command_with_no_transition_and_no_event` |
| F9b | Solver timeout reported as a verdict | feasibility runner seam | the `FailClosed` verdict REJECTS with `fail_closed:validator_timeout…` named (the P4.08 policy table's reject branch) | `f9b_solver_timeout_reported_as_a_verdict_rejects_with_the_reason_named` |
| F10 | Plugin crash (SIMULATED — no plugin kernel exists yet, v0.5.0; modelled as a poisoned/panicking proposal source) | feasibility runner seam | the panic PROPAGATES (fail-closed by construction): no partial transition, no event, nothing issued; the command HELD in VALIDATED. Containment (catch + supervisor restart) is the composition root's recorded duty — the application layer does NOT swallow panics (the broker-side poison path is the P1.16 consumer's, a different layer) | `f10_a_panicking_proposal_source_propagates_with_no_partial_transition_and_nothing_issued` |
| F11 | Partial fleet response (3 assets: accept / NACK / timeout) | simulator transport + P4.14 fleet acknowledgement | PER-ASSET acknowledgement (`Accepted` / `Refused` / `TimedOut`); `FleetAcknowledgement::portfolio_state()` = `PartiallyAccepted{accepted:1, assets:3}`, `fully_accepted() == false` — never inferred from the first responder (§14.3); the fallback fires ONLY for the two failed assets; the accepted asset is untouched | `f11_a_partial_fleet_response_is_per_asset_and_never_portfolio_inferred` |

## 3. Shadow-mode evaluation: method and limits (§21.4)

**Cited, never from memory** — `.agent/CONTEXT.md`, *Control authority*
table: "| Read-only / shadow | No [writes to devices] | Early pilots,
commissioning |" and "Mode is a deployment/site setting. Plugins cannot
select or raise it." `ledger/phase-04-grid-feasibility-dispatch/CONTEXT.md`
§5: "**Shadow mode:** proposed actions are computed and compared against
measured behaviour without issuing commands (§21.4). Shadow agreement is
not a safety claim."

**The method** (`shadow_evaluation.rs`, recorded once in
`SHADOW_METHOD_STATEMENT`): each proposed command's requested value is
compared against the simulator's BASELINE (no-command) behaviour at the
same instants, within a configured absolute tolerance in the quantity's
canonical unit. The comparison is read-only over the `ShadowBaselineSource`
port — the module has NO command-delivery path (the structural test pins:
no `EvidenceReference`, no `fn deliver`, no
`SimulatorDispatchTransport`, no outbox, no journal write).

**Fail-closed classification** — a comparison instant is `NotComparable`
(never agreement) when the baseline is MISSING (§10.4), STALE (older than
the configured max age), stamped in the FUTURE (clock skew, §17.4), or
carries a different quantity (never coerced — the P4.16 mismatch rule).
The overall verdict is the conservative composition: any not-comparable
entry → `NotComparable`; else any divergence → `Divergence`; else
`Agreement`. A single bad sample is never averaged away.

**The limits — carried ON THE REPORT TYPE** (the ledger P4.31 risk
mitigation: "the shadow report states its limits in its header"):
`ShadowReport` has `limits_statement`, `method_statement`, and
`issued_statement` fields populated from the module constants, and
`evidence_status()` always returns `not_evidence_for_field_dispatch`.
`SHADOW_LIMITS_STATEMENT` in full: *shadow agreement is NOT a safety claim:
agreement with a synthetic simulator baseline shows only that the proposal
is consistent with the modelled behaviour profile; shadow results are
NEVER evidence for field dispatch, are not §14.3 evidence references, and
are not attachable to the authorization gate; the entire 0.x line is
simulation-only and operator-approved field control remains a v1.0.0
matter (§21.4).*

Additional recorded limits: the baseline is the Phase 2 synthetic behaviour
profile — a stated assumption, never a validated physical model (P2.15);
the simulator models NO local protection (the recorded boundary of
`29_safety_invariant_matrix.md` §invariant 10), so shadow agreement says
nothing about protection interactions; the comparison tolerance and
baseline max-age are deployment configuration, not validated thresholds;
the shadow report is not wired into any gate, event, or persistence
surface in this phase (it is a report type + pure method; consuming it is
a Phase 5 display-contract matter, P5.xx display contracts only).

## 4. Integration tests: the legs recorded as human-run gate inputs

The ledger step names PostgreSQL, NATS, the Python power-flow worker, and
the simulator adapter TOGETHER, end-to-end on the §24 steps 5–8 scenario.
What is implemented vs what the human/CI runs:

| Leg | Status in this subtask | The real run (gate input) |
|---|---|---|
| PostgreSQL | REAL in the suite (env-gated on `FLEXCORE_TEST_DATABASE_URL`, UNRUN — mirrors `persistence_wiring.rs`) | The human sets the URL locally, or CI's `integration-tests` job provides the PostgreSQL 18 service. The suite covers the §24 path: command persisted → gate journal rows with DISTINCT evidence → §12.5 feasibility-check record → §15.3 outbox publish records → read back through the §15.2 store surface (`list_commands`, `list_feasibility_checks`) → tenant isolation (foreign tenant reads nothing) → CAS refuses a redelivered gate transition → check records are append-only → publish records drain with zero loss and refuse redelivery. |
| NATS | The suite asserts the REAL SEAM's input records (the durable outbox rows apps/worker's `OutboxPublisher` polls: idempotency keys, destination subjects, attempt counts, durable duplicate refusal). The broker itself is NOT touched (loop bans). | The REAL broker publish is CI's `integration-tests` job (`FLEXCORE_TEST_NATS_URL`) plus the P1.24 broker fault slice, which already cover the publisher/consumer halves; this suite adds the DB-side zero-loss/duplicate semantics the publisher depends on. |
| Python power-flow worker (pandapower 3.5.6) | The suite asserts the RECORD the grid-model port's result leaves: the migration-0017 feasibility-check row (model id/version/content hash, outcome, margins, coverage, actor, correlation) and the journal evidence binding THE MODEL HASH (§17.5 invariant 3). The worker is pinned (RESEARCH.md: 3.5.6, 7 October 2026) but NOT INSTALLED in this environment. | The REAL pandapower call is the human's run (install pandapower 3.5.6, run the worker against the grid-api port, confirm the produced check row matches the fixture shape). This suite never fabricates a power-flow verdict of its own; AC power flow is an evaluation, not AC-OPF — no solver-class claim is made anywhere here. |
| Simulator adapter | The §24 steps 6–8 issue/ack/tracking machinery is covered by the flexcore-application suites over the `SimulatorDispatchTransport` port (P4.16, existing) plus the F2/F5/F7/F8/F11 fault cases above; the persistence half (command rows, journal, evidence) is in the new DB suite. | No device write path exists in any of it; the adapter's transport target is the simulator only (P4.16 hard guard, unchanged). |

## 5. Decisions (all DEFAULT unless noted)

| ID | Decision | Label |
|---|---|---|
| D-P4.31-1 | Shadow evaluation is a NEW application-layer module over a locally declared read-only port (`ShadowBaselineSource`), not an extension of `dispatch_simulator.rs` — the crate cannot name flexcore-simulator, and the transport port of P4.16 is command-delivery-shaped (deliver + observe), which is exactly what shadow must NOT have. | DEFAULT (the seam pattern of the crate) |
| D-P4.31-2 | The shadow comparison is per-instant against baseline SAMPLES (value-level), not schedule-level aggregate comparison — P3.23 owns schedule-vs-schedule comparison; this phase's shadow compares one proposed command against the no-command baseline. | DEFAULT |
| D-P4.31-3 | Staleness/future-stamp/quantity-mismatch/absence are all `NotComparable` (fail-closed), and the overall verdict composes conservatively. A lenient composition (majority agreement) was rejected: the ledger's never-silent-pass rule applied to shadow. | DEFAULT |
| D-P4.31-4 | The plugin crash is modelled as a PANICKING proposal source whose panic PROPAGATES (asserted with `catch_unwind`); no catch-and-continue path is added. The P1.16 poison/DLQ path remains the broker-side containment; an application-level catch-and- supervisor policy is deferred to the plugin host (v0.5.0, Stage 7). | DEFAULT |
| D-P4.31-5 | The fault suite lives in flexcore-application (seam-level, no broker/DB) while the integration suite lives in the postgres adapter (env-gated) — matching the recorded harness pattern (`persistence_wiring.rs` for DB, `nats/tests/fault_slice.rs` for broker). CI runs the fault+shadow suites in `scenario-regression` (pure) and the DB suite in `integration-tests` (services already provided). | DEFAULT |
| D-P4.31-6 | F1's "drop" is proxied as an attempted-but-unacked publish (record_attempt without mark_delivered) — the same failure path the P1.24 outage proxy uses; a hard process-kill variant remains the P1.25 Compose stack's job (recorded there). | DEFAULT (P1.24 precedent) |

## 6. Honest limitations

1. **Everything is UNRUN.** No suite has executed. The expectations are
   pre-registered, not passing. The acceptance criterion "every injected
   fault yields its pre-registered outcome" is implemented-and-unverified.
2. **The shadow method compares VALUES, not consequences.** It compares
   the requested setpoint against the baseline behaviour profile; it does
   not simulate the network response to the command (that is the
   feasibility gate's job) and it does not model protection (recorded
   boundary, invariant 10).
3. **Shadow is not wired into any workflow yet.** The module and report
   exist and are tested; no composition root invokes them in this phase
   (the composition root implements `ShadowBaselineSource` over
   flexcore-simulator — that wiring is the human/Stage-6 follow-up,
   recorded, not assumed).
4. **The pandapower leg is record-level only here.** The DB suite proves
   the check record and evidence chain; the real worker call (pinned
   3.5.6, not installed) is the human's gate input (§4).
5. **The F2 receive-point refusal uses the P4.28
   `VerifyingDispatchTransport`,** whose payload-bound signature is the
   recorded fnv1a64 SIMULATION stand-in (non-cryptographic; D-P4.28-5's
   mTLS gap stands unchanged).
6. **F10 proves the seam, not a plugin.** No plugin kernel exists
   (v0.5.0); the crash shape (a panicking proposal source) is structural.
7. **Determinism is asserted in-suite** (double-run assertions where the
   ledger asks for them, e.g. the shadow determinism test); the P4.27
   two-clean-runs discipline applies to the human's build/test pass, not
   to this subtask.
8. **CI additions take effect on the human's first run** — the two new
   scenario-regression steps and the env-gated DB suite are written but
   never executed.

## 7. Verification-tool output (recorded after the run)

Recorded in §8 below; nothing was executed before this record was
written.

## 8. Tool runs performed in this subtask (the only executions allowed)

The following WERE run (allowed by the loop bans: Python heredocs +
`tools/verify_docs.py` + `tools/validate_agent.py` + `python -I
tools/check_dependency_rule.py`). Their actual output is quoted in the
final task report; no cargo, docker, database, or broker command was run.
