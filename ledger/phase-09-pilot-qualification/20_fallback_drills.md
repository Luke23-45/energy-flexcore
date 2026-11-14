<!-- P9.20 — Fallback and degraded-mode drills (decision record). DRILL PLAN + TEMPLATE ONLY: the fallback policy register is EMPTY (no asset, no site, no real operator review exists — the code's SIMULATED-OPERATOR reviews are explicitly not sign-offs); every drill/result cell EMPTY, status UNRUN/UNWITNESSED. No drill was injected, no fallback was observed, no time-to-fallback was measured. The drills and the witness are human gates (ENTRY_NOTE.md boundaries 1 and 6). -->
# P9.20 — Fallback and Degraded-Mode Drills (Plan + EMPTY Drill Records)

Date: 2026-11-14. Subtask: P9.20 (`ledger_phase_9.md` §P9.20). Owner
role: Integration engineering (with power systems and the site
operator). Report trace: §23 Stage 9; §14.4; §17.4 (central outage);
§21.2 (degraded modes). Inputs per the ledger: P9.02 asset register
(TEMPLATE — no site data), P9.04 integration design (TEMPLATE), P9.13
commissioning plan (TEMPLATE), the per-asset/site fallback policies
(**do not exist — §4 is the register they must be written into, with
the REAL operator review that §14.4 requires; the code's
SIMULATED-OPERATOR reviews are not it**). The degraded-case definitions
align with `39_performance_load_test.md` §3 M-8 (the same no-silent-loss
structural rule; that record owns the load-side measurements).

## 1. Honest status — the plain statement

**No drill has been run, no fallback policy exists for any asset or
site, no real equipment/site operator has reviewed any policy, and no
drill has been witnessed.** There is no pilot site, no asset, no site
operator, and no deployment to inject a failure into. Every drill and
result cell in §6 is deliberately EMPTY. The "expected behavior"
statements in §5 are derived FROM READING the named code on 2026-11-14
(each citation names the file and the mechanism verified to exist by
reading) — they are design statements about what the mechanisms DO,
not evidence that any fallback ever activated in any pilot context.
No time-to-fallback figure exists anywhere in this record.

## 2. The governing rule — §14.4, no universal fallback

Verbatim, the code constant `NO_UNIVERSAL_FALLBACK_STATEMENT`
(`crates/flexcore-application/src/fallback_policy.rs`, which quotes
report §14.4): "There is no universal "set power to zero" fallback
(report §14.4). Fallback policy is configured per asset/site and must
be reviewed with the equipment/site operator (in simulation, recorded
as a SIMULATED-OPERATOR review). The safe default for an asset/site
with no named policy is the explicit require-operator-intervention
entry — never silence, never an automatic setpoint change."

The §14.4 bounded fallback-behavior set (the closed
`FallbackPolicyVariant` set in the same file):

| Variant | Meaning |
|---|---|
| Hold last accepted setpoint | For a BOUNDED interval where approved; requires stated validity conditions — a hold policy with EMPTY validity conditions is a CONSTRUCTION ERROR (`HoldPolicyRequiresValidityConditions`) |
| Return to local EMS autonomous control | The site EMS resumes autonomous operation |
| Preserve reserve/SOC constraints | Constraint preservation is part of the policy record |
| Stop accepting new external commands | The admission path closes |
| Request a safe local operating mode | A bounded request, not a device write by FlexCore |
| Require operator intervention | The EXPLICIT default: `DEFAULT_ENTRY_STATEMENT` = "no fallback configured — require operator intervention" — every asset/site resolves to a named policy OR this entry; the default is intervention, never silence |

The validity-conditions rule, verbatim
(`VALIDITY_CONDITIONS_RULE_STATEMENT`): "A last-known setpoint is not
automatically safe when topology, asset state, or network constraints
have changed (report §14.4). Every hold-last-accepted-setpoint policy
MUST state its validity conditions; a hold policy with empty validity
conditions is a construction error, and the conditions are part of the
policy record."

**Honest label on operator review:** in the simulation-only 0.x line
the code records `SIMULATED-OPERATOR` reviews by the power-systems
engineer (`SIMULATED_OPERATOR_REVIEW_LABEL`: "…not a real operator
sign-off"). The §4 register's review column is filled only by the REAL
equipment/site operator at the pilot — replacing the simulated review
is precisely the human gate this subtask plans.

## 3. Design basis — the actual fallback and degraded-mode seams (read 2026-11-14)

| Surface | File | What it provides the drills |
|---|---|---|
| Fallback engine | `crates/flexcore-application/src/fallback_policy.rs` (P4.17/P4.18) | FALLBACK entry from FAILED and EXPIRED ONLY (frozen §14.1 edges 26–27, through the domain engine — never bypassed); `FallbackActivationRecord` carries WHICH policy fired, its parameters, and the bounded interval; the bounded hold EXPIRES after its configured interval and ESCALATES (`EscalationTarget`: require-operator-intervention or return-to-local-EMS-autonomous) — `BOUNDED_HOLD_STATEMENT`: "It NEVER holds indefinitely: the interval is a finite millisecond count enforced by type"; `ESCALATION_IS_NOT_A_TRANSITION_NOTE`: FALLBACK is terminal (no outgoing edge, D-P4.10-6) so escalation is an audit record, never a state-machine edge; `RECOVERY_RULE_STATEMENT`: recovery requires a NEW command through the FULL PROPOSED → VALIDATED → FEASIBLE → APPROVED chain — a direct FALLBACK → ISSUED attempt is rejected (`NotPermitted`); audit rows `dispatch_fallback_activations` / `dispatch_fallback_escalations`; actor `service:fallback-orchestrator` |
| Edge degraded mode | `crates/edge-agent/src/runner.rs` (P6.09) | Uplink failure → buffer-first degraded mode (`DegradedBufferingEntered`, audited); recovery → `DegradedBufferingExited` (audited) and drain; a full buffer PAUSES polling (audited backpressure) — never a silent drop |
| Telemetry buffer | `crates/edge-agent/src/buffer_file.rs` (P6.09) | Durable journal (`FLEXCORE-EDGE-TELEMETRY-JOURNAL`, CRC-32 framing); configured capacity bound; audited overflow policy; truncate-and-audit recovery (never silent) |
| Edge audit buffer | `crates/edge-agent/src/audit_buffer.rs` (P6.18) | BOUNDED audit journal; overflow = **audited drop-of-OLDEST**: `dropped_oldest` counter increments, `has_overflowed` health flag LATCHES, the drop is logged — never a silent overwrite, never unbounded growth; deliberately NO age-based drop (retention is a deployment `RetentionPolicy` decision, recorded as D-2, not a period in code); at-least-once IN-ORDER upload on reconnect; corrupt tail → truncate-and-audit |
| Site-local policy engine | `crates/edge-agent/src/policy.rs` (P6.17) | The edge enforces site policy LOCALLY — a disconnected edge still enforces something: unknown/disabled adapter REFUSED (fail-closed), per-adapter rate limits, operating windows; violation = REJECT + AUDIT, never log-and-continue |
| Adapter health | `crates/edge-agent/src/health.rs` (P6.17) | Closed vocabulary HEALTHY / DEGRADED_* / FAILING_*; health NEVER inferred from the absence of errors (no observation = `DEGRADED_UNVERIFIED`); `DEGRADED_CLOCK_DRIFT` (P6.31); while `FAILING_*`, ingest and commands are BLOCKED with an audited reason (`check_action_allowed`) |
| Clock quality | `crates/edge-agent/src/clock_quality.rs` (P6.31) | Closed vocabulary Unknown/Synchronized/Degraded/Unsynchronized; offset = MEDIAN of a bounded sample window; corrupt pair (beyond `MAX_ABS_OFFSET_MILLIS`, one day) REFUSED; profile defaults 100 ms tolerance / 30 000 ms sync timeout are DEFAULTS, not measurements |
| Command expiry vs drift | `crates/edge-agent/src/command_verify.rs` (P6.15) | The expiry check trusts ONLY `Synchronized` clock quality — a degraded/unsynchronized edge clock FAILS CLOSED (§17.4, §17.5 rule 5) |
| Simulation-only boundary | `dispatch_simulator.rs` `SIMULATION_ONLY_STATEMENT` | Every fallback activation in the 0.x line is a supervisory lifecycle fact about a SIMULATED command — not a device write, a protection action, or a safety function; site-local safety and protection remain independent of FlexCore (§17.5 invariant 10) |

The §21.2 requirement the drills exist to evidence: a central outage
must not disable site-local protective controls or force an unsafe
device state. FlexCore is supervisory only; local certified controllers
retain their functions (§14.5) — the drill OBSERVES this independence,
it does not create it.

## 4. The fallback policy register (TEMPLATE — every asset/site cell EMPTY)

One row per pilot asset/site, copied from the P9.02 reconciled registry
by the human gate. A row with no named policy resolves to the explicit
default entry (require operator intervention) — it is never left blank.

| Asset/site ID (from P9.02) | Named policy (variants from §2, parameters) | Hold validity conditions (MANDATORY for any hold policy) | Bounded hold interval + escalation target | REAL equipment/site operator review (name, date, signature) | Recorded |
|---|---|---|---|---|---|
| (EMPTY — no asset exists) | (EMPTY) | (EMPTY) | (EMPTY) | (EMPTY — UNSIGNED) | (EMPTY) |

## 5. Drill scenarios and expected behaviors (expected = cited code; observed = EMPTY)

| # | Drill (trigger injected) | Expected behavior (cited) | Trigger actually injected / observed fallback / time to fallback / local protection independent? |
|---|---|---|---|
| D-1 | **Central outage** (control plane stopped at the site) | Edge keeps polling + buffering in audited degraded mode (`runner.rs` `DegradedBufferingEntered`); site-local policy enforcement CONTINUES (`policy.rs` — a disconnected edge still enforces); site-local safety/protection unaffected (§17.5 invariant 10 — OBSERVED, not provided, by FlexCore); on recovery: audited exit + drain. NO fallback that forces an unsafe device state; the site EMS governs locally per its own design | (EMPTY — UNRUN) |
| D-2 | **Edge-to-central link loss** (network partition, central up) | Same degraded path as D-1 from the edge side; telemetry journal absorbs the outage window; drain time recorded after reconnect; at-least-once, in-order (§15.4) | (EMPTY — UNRUN) |
| D-3 | **Asset unavailability** (asset stops responding) | Health derives `FAILING_UNRESPONSIVE` through the explicit detection window on the last OBSERVED fact — never from silence-as-healthy (`health.rs`); while `FAILING_*`, ingest from and commands to that asset are BLOCKED with an audited reason; any in-flight command follows the §14.1 failure/fallback edges (FAILED → FALLBACK where the policy requires) | (EMPTY — UNRUN) |
| D-4 | **Solver timeout / scheduling worker loss** (worker slow, then down) | The schedule path records the failure with the §12.6 closed status vocabulary — it never silently relaxes (aligned with `39_performance_load_test.md` M-8); the structural rule is the same: no silent drop; every loss counted and audited; queue depth / pauses / recoveries recorded per M-8 | (EMPTY — UNRUN) |
| D-5 | **Stale telemetry** (feed frozen at the source) | Quality maps to `STALE`/`MISSING` (§10.4 — exactly one canonical state; never silently normalized); the freshness/staleness gate HOLDS or REJECTS per policy — a missing/uncertain/stale model is not a passing feasibility check (§17.5 invariant 9); commands built on the stale feed fail validation, not issue with a note | (EMPTY — UNRUN) |
| D-6 | **Clock drift** (site clock offset beyond tolerance) | `clock_quality.rs` derives Degraded/Unsynchronized → `DEGRADED_CLOCK_DRIFT` in health; `command_verify.rs` FAILS the expiry check CLOSED on any quality other than `Synchronized` — commands are HELD, fail-closed, and audited; a corrupt pair (offset > one day) is REFUSED, never estimated; the configured tolerance/sync-timeout values in force are recorded (the 100 ms / 30 000 ms module values are DEFAULTS, not measurements) | (EMPTY — UNRUN) |
| D-7 | **Audit-buffer overflow** (audit generation exceeds capacity) | Audited drop-of-OLDEST: `dropped_oldest` counter increments, `has_overflowed` latches, the drop is logged — never a silent overwrite (`audit_buffer.rs`); the latch is visible on the operator health surface (P6.17 consumes it); NO age-based drop; retention decision per the deployment `RetentionPolicy` (D-2); an overflow that is neither counted nor audited is a FAILING cell, never a pass (same rule as `39_...md` M-4) | (EMPTY — UNRUN) |
| D-8 | **Plugin-host crash** | The plugin cannot authorize, issue, or select the control-authority mode (§5.7; protected invariant); its disappearance removes its proposals/diagnostics only; the protected validation/authorization path is untouched; restart/recovery behavior recorded as observed | (EMPTY — UNRUN) |
| D-9 | **Out-of-process gRPC worker crash** | Requests fail with named errors (never hang, never default-allow); the schedule/dispatch path records the failure per its closed status vocabulary; recovery drain recorded | (EMPTY — UNRUN) |

Cross-cutting expectation for EVERY drill (ledger step 3): the record
captures the trigger, the observed fallback, the time to fallback, and
whether site-local safety/protection remained independent of FlexCore.
A drill that "passes" because the failure was never actually injected
is the ledger's named failure mode — each D-row records the injection
METHOD, not just the outcome.

## 6. DRILL RECORD — ALL CELLS EMPTY. NOTHING HAS BEEN DRILLED.

### 6.0 Drill conditions

| Drill | Date | Site | Asset(s) | Fallback policy in force (§4 row) | Injection method | Control-authority mode (§21.4) | Software versions | Operator present | Witness |
|---|---|---|---|---|---|---|---|---|---|
| EMPTY | (EMPTY — UNRUN) | (EMPTY) | (EMPTY) | (EMPTY) | (EMPTY) | (EMPTY) | (EMPTY) | (EMPTY) | (EMPTY — UNWITNESSED) |

### 6.1 Per-drill results

| Drill ref | Observed fallback | Time to fallback | Local protection independent of FlexCore? (evidence) | Losses (counted + audited; zero is a recorded result) | Pass/fail | Disposition of any failure |
|---|---|---|---|---|---|---|
| (EMPTY — UNRUN) | (EMPTY) | (EMPTY) | (EMPTY) | (EMPTY) | (EMPTY) | (EMPTY) |

## 7. The witness requirement (human gate)

The ledger's verification method: "Witness at least one drill (central-
outage drill) in person or by recorded evidence; inspect the fallback
register for operator review signatures." The witness is a PERSON, not
a document: the D-1 row's witness cell must carry a name and date, and
the §4 register must carry the real operator's review signature before
any drill against that asset counts. No agent output substitutes for
either.

## 8. What passing drills would and would not establish

**Would establish (exactly):** that THIS deployment, at the frozen
versions and the §4 policies in force, exhibited the §5 expected
behaviors under each actually-injected failure, with the recorded times
to fallback, no silent loss, and site-local protection observed
independent of FlexCore — at the drilled sites, on the drilled day.

**Would NOT establish:** protection-grade or safety-function capability
(AGENTS.md §3 — the independence observed is the LOCAL systems' own
property); behaviour at undrilled sites or under un-drilled failure
combinations; bounded-automation readiness (the pilot ceiling is
operator-approved control at most — ENTRY_NOTE boundary 3); certification
of any adapter, asset, or site; and not a substitute for P9.24 (backup/
restore rehearsal) or P9.36 (live incident drill), which are separate
human gates with their own records.

## 9. Reviewer-verification procedure and filling commands (human gate)

```sh
# (a) the documented-tools gate (run after ANY edit to this record):
python tools/verify_docs.py
python tools/validate_agent.py
python -I tools/check_dependency_rule.py

# (b) BEFORE any drill: fill §4 per pilot asset/site from the P9.02
#     registry; obtain the REAL equipment/site operator review per row
#     (the SIMULATED-OPERATOR code label is NOT a sign-off); any hold
#     policy must carry non-empty validity conditions or it cannot be
#     constructed in code at all.

# (c) THE DRILLS (human gate): inject each §5 trigger by its recorded
#     method; fill §6.0–§6.1 FROM THE OBSERVED RUN ONLY; every loss is
#     counted and audited (zero is a result, silence is a failing cell);
#     the central-outage drill (D-1) requires the named witness (§7).
# (d) FILE any drill failure with a disposition (fixed and re-drilled /
#     accepted residual risk carried to P9.27 / blocks the pilot);
#     re-drill after a fix. No open drill failure may exist at P9.28.
# (e) File the §4 register + §6 records as the P9.20 evidence named in
#     CHECKLIST.md ("Fallback drills passed").
```

Reviewer checks: (1) every §6 cell EMPTY/UNRUN in the committed
template; (2) every §5 expected-behavior claim traces to the cited file
(spot-check `NO_UNIVERSAL_FALLBACK_STATEMENT`, the overflow policy, and
the hold-on-drift rule); (3) no zero-power/universal fallback appears
anywhere; (4) every hold policy's validity-conditions cell is non-empty
at fill time; (5) D-1's witness cell is a named human.

## 10. Open risks carried to P9.27

| # | Risk | Basis |
|---|---|---|
| R-1 | A universal zero-setpoint fallback applied to an asset for which it is unsafe — the ledger's named failure mode; the code forbids it structurally, the register + real operator review are the human defence | §2, §4 |
| R-2 | A hold policy holding a stale setpoint after topology changed — mitigated by the mandatory validity conditions and bounded interval with escalation, but the conditions' CONTENT is judgement, reviewable only by the real operator | §2, §4 |
| R-3 | A drill that "passes" without a real injection — mitigated by the injection-method column and the witness, both human-enforced at fill time | §5, §6.0 |
| R-4 | No fallback policy, site, asset, or operator exists — every ledger acceptance criterion is UNMET and not claimed | §1, §4, §6 |
| R-5 | Local-protection independence is OBSERVED in drills, not guaranteed by FlexCore; if the site EMS design does not provide it, the drill exposes a customer-side finding, not a FlexCore fix | §3, §8 |
| R-6 | Audit-buffer overflow provably loses the OLDEST records by design (counted and latched, never silent); whether that loss window is acceptable is a customer/authority decision to be made before the pilot, not assumed here | §3 (audit_buffer.rs, D-2) |
| R-7 | Drill coverage of D-8/D-9 depends on the pilot composition actually containing a plugin host / gRPC worker; where it does not, the row records the composition's actual failure path | §5 D-8/D-9 |

## 11. Verification status (honest)

| Item | Status |
|---|---|
| `python tools/verify_docs.py` | RUN in the writing loop (output quoted in the completion report) |
| `python tools/validate_agent.py` | RUN in the writing loop (output quoted in the completion report) |
| `python -I tools/check_dependency_rule.py` | RUN in the writing loop (no dependency edge added) |
| The drills themselves (any cell of §6) | **NOT RUN — banned in this loop; human-gate input; all drill/result cells EMPTY** |
| Fallback policy register (§4) | **EMPTY — no asset, no policy, no real operator review exists** |

Status: **WRITTEN-UNRUN** — a drill plan cannot be "run"; the drills,
the policy reviews, and the witness are the human gates and have not
happened and are not scheduled. No fallback was activated; no pilot
exists and none is claimed.
