# P4.17 + P4.18 + P4.19 — Decision note: asset-specific fallback policies, bounded hold and recovery, and dispatch timing honesty

- **Date:** 2026-11-10.
- **Subtasks:** P4.17 (ledger_phase_4.md §P4.17), P4.18 (§P4.18), P4.19 (§P4.19). Report trace: §14.4, §14.1–§14.2, §14.5, §0.3, §17.5, §19.4, §15.3 (`DispatchFallbackActivated`), §24 step 8.
- **Deliverable files:** `crates/flexcore-application/src/fallback_policy.rs` (P4.17 + P4.18), `crates/flexcore-application/src/timing_honesty.rs` (P4.19), `crates/flexcore-application/tests/fallback_policy.rs`, `crates/flexcore-application/tests/timing_honesty.rs`; module registration in `crates/flexcore-application/src/lib.rs`.
- **Status:** implemented, **awaiting human build/test**. Nothing was built, run, installed, or executed for this wave (loop bans held). Every acceptance criterion below has code + tests written but NOT executed.

## 1. P4.17 — Asset-specific fallback policies (§14.4)

### D-P4.17-1 — The governing rule is a type-level invariant (the §14.4 rule, enforced)

**Decision.** There is **no universal "set power to zero" fallback** anywhere in
the crate. This is enforced in three layers:

1. The exact §14.4 variant set is a closed enum
   (`FallbackPolicyVariant`: hold last accepted setpoint for a bounded
   interval where approved; return to local EMS autonomous control; preserve
   reserve/SOC constraints; stop accepting new external commands; request a
   safe local operating mode; require operator intervention). No "set to
   zero" variant exists and none may be invented.
2. Every asset/site resolves to a NAMED policy or the **explicit default
   entry** "no fallback configured — require operator intervention"
   (`FallbackPolicyRegistry::entry_for`; DEFAULT — the safe default is
   intervention, recorded, never silence, never an automatic setpoint
   change).
3. A structural test scans every `src/*.rs` file of
   `flexcore-application` for universal-fallback markers (`fn universal`,
   `UniversalFallback`, `zero_power`, `ZERO_POWER`, etc.) and fails if any
   exists — the in-test form of the ledger's "code search for a universal
   fallback" verification (scoped to this crate's sources; recorded).

### D-P4.17-2 — Hold policies MUST state validity conditions (§14.4, construction error)

A last-known setpoint is not automatically safe when topology, asset state,
or network constraints have changed. `HoldValidityConditions::new` REJECTS an
empty condition list (`FallbackPolicyError::HoldPolicyRequiresValidityConditions`)
— a hold without stated validity conditions cannot exist, and the conditions
are part of the policy record (`FallbackActivationRecord::validity_conditions`).
`FallbackPolicy::new` re-asserts the rule at the policy boundary so a
hand-assembled action cannot bypass it.

### D-P4.17-3 — The §14.4 operator review is recorded as SIMULATED-OPERATOR (simulation-only form)

The report assigns the fallback-policy review to the equipment/site operator.
In this simulation-only 0.x line the review is recorded by the power-systems
engineer as a `SimulatedOperatorReview` carrying the
`SIMULATED-OPERATOR` label ("recorded by the power-systems engineer in the
simulation-only 0.x line; not a real operator sign-off"). It is attached to
every activation record. This is NOT a real operator sign-off and never
renderable as one.

### D-P4.17-4 — FALLBACK entry only where the policy requires it, through the frozen edges

`FallbackEngine::activate_fallback` enters FALLBACK from FAILED and EXPIRED
**only** where the resolved policy lists that trigger, and only through the
P4.10 domain engine (`CommandLifecycle::record_fallback` — frozen §14.1 edges
26–27; this module never bypasses `try_transition` and never re-encodes an
edge). The command's actual state comes from the persisted journal replay
(the P4.14 `replay_full_lifecycle` generalization, REUSED), so the activation
input cannot spoof FAILED vs EXPIRED (mismatch →
`InconsistentJournal`, fail-closed).

Three non-activating outcomes are each audit-visible:

| Outcome | Meaning | State transition? | Event? |
|---|---|---|---|
| `Activated` | named policy fired on its configured trigger | FAILED/EXPIRED → FALLBACK (frozen edge) | `DispatchFallbackActivated` |
| `InterventionRequired` | no named policy — the explicit default entry | none | none |
| `TriggerNotConfigured` | named policy exists, but not for this trigger | none | none |

**EXPIRED → FALLBACK is the only EXPIRED outgoing edge** (frozen §14.1), and
it fires only where the policy requires it — proven by the
`TriggerNotConfigured` test (a FAILED-only policy evaluated on EXPIRED: no
transition, audit record written).

### D-P4.17-5 — Recorded integration point: activation input scope

The engine takes site/asset/portfolio as input; the composition root derives
them from the P4.14 command record row. Tenant scoping is the `TenantContext`
(P0.16 rule 5); a missing command in scope is `UnknownCommand`.

### D-P4.17-6 — Recorded integration point: "stop accepting new external commands"

`FallbackPolicy::accepts_external_commands` reports the declared variant; the
command-creation wiring (P4.11 composition) consumes it. No command-creation
code was modified in this wave (file ownership).

### D-P4.17-7 — Recorded payload-fit gap (frozen event not extended)

`DispatchFallbackActivated` (§15.3, frozen via P0.14) is a `Copy` payload
carrying identity + time ONLY — no field for the policy name, parameters, or
bounded interval. Those facts travel in (a) the transition journal's evidence
reference `fallback-policy:<name>` and (b) the `FallbackActivationRecord`
audit row. The frozen payload shape is NOT changed; the gap is recorded in
`DISPATCH_FALLBACK_PAYLOAD_GAP_NOTE`. The composition root may add a separate
non-frozen detail event later.

## 2. P4.18 — Fallback activation, bounded hold, and recovery (§14.4, §17.5)

P4.18 is implemented inside `fallback_policy.rs` (no separate module needed —
the hold types and the engine were designed together).

### D-P4.18-1 — The bounded hold is finite by construction and enforced in code

`BoundedHoldInterval::from_millis` rejects zero/negative intervals; the
interval is a strictly positive finite millisecond count
(`BOUNDED_HOLD_STATEMENT`). `FallbackEngine::evaluate_hold_expiration`
returns `HoldActive { expires_at }` strictly before the expiry instant and
`Escalated` at or after it — the hold NEVER holds indefinitely, and the
evaluation is controlled-clock based (never wall-clock). The recorded P4.18
risk ("indefinite hold masking a failed asset") is mitigated in code, not by
operator memory.

### D-P4.18-2 — Escalation is an audit record, NOT a command transition

**Decision.** The ledger's escalation wording ("escalates to
require-operator-intervention or return-to-local") is implemented as an
OPERATIONAL posture change recorded on the activation
(`FallbackEscalationRecord` through the `FallbackAuditSink`), because the
frozen §14.1 table gives FALLBACK **no outgoing edge** (terminal, P4.10
freeze decision D-P4.10-6). Inventing a FALLBACK → anything transition would
violate the frozen-edge oracle; a state-machine edge is neither taken nor
invented (`ESCALATION_IS_NOT_A_TRANSITION_NOTE`). The two escalation targets
are exactly the ledger's two (`EscalationTarget`).

### D-P4.18-3 — Recovery requires the full gate chain; no shortcut exists

After FALLBACK, recovery for the asset is a NEW command through the FULL
PROPOSED → VALIDATED → FEASIBLE → APPROVED chain (`RECOVERY_RULE_STATEMENT`).
This module deliberately provides NO recovery shortcut function; the
FALLBACK → ISSUED shortcut is proven rejected by the frozen-edge oracle
(`try_transition` → `NotPermitted`; the lifecycle engine refuses, the journal
is untouched, and FALLBACK is terminal). The recovery test drives a fresh
command through the P4.10 engine and asserts all three gates appear in order
in the journal — recovery works only through the gates.

### D-P4.18-4 — Every activation and escalation is audit-visible (§17.5)

`FallbackAuditSink` (a seam; the composition root implements it over
flexcore-storage; the P4.30 audit wiring consumes the same rows) records
every activation (policy, parameters, bounded interval, validity conditions,
simulated review, outcome) and every escalation (policy, target, interval,
instants, outcome). Fail-closed: an audit failure is an error, never a
skipped record (tested).

## 3. P4.19 — Dispatch timing honesty (§14.5)

### D-P4.19-1 — The boundary is quoted and enforced structurally

`TIMING_BOUNDARY_STATEMENT` quotes §14.5 verbatim: supervisory setpoint
delivery only; no primary-frequency, synthetic-inertia, relay,
inverter-inner-loop, or sub-cycle claims; those functions remain in local
certified controllers and hardware unless a separate engineered and
qualified architecture is developed. `NOT_PROMISED_FUNCTIONS` names all six
non-performed functions; `SUPERVISORY_ONLY_STATEMENT` states the §0.3
scope. No hard-real-time, frequency, inertia, relay, inner-loop, or sub-cycle
capability is implemented or claimed anywhere in this crate.

### D-P4.19-2 — Latencies are MEASURED distributions, never guarantees or targets

`measure_from_journal` computes one command's acknowledgement latency from
the P4.10 transition journal (ISSUED → ACKNOWLEDGED, controlled clock — the
journal is the evidence of record, never a stopwatch) plus the tracking
deviation supplied by the simulator's tracking telemetry. `TimingRecorder`
accumulates samples under ONE fixed `TimingConditions` set (simulator, load,
measurement method — all required non-empty) and reports the MEASURED
DISTRIBUTION: histogram, p50/p95/p99 (nearest-rank), maxima. The report type
deliberately exposes NO best-case accessor and NO target field; the report
text always embeds the conditions, the no-guarantee statement
(`NO_LATENCY_GUARANTEE_STATEMENT`, §19.4), and the §14.5 boundary. A
recorder with no samples has no distribution (error, never an empty report);
a FAILED command has no latency sample (fail-closed, never estimated
around). No latency figure is quoted in this note: none has been measured
(nothing was run).

### D-P4.19-3 — The claim scan (ledger step 3), recorded result

Grep scan over the Phase 4 dispatch sources (`fallback_policy.rs`,
`timing_honesty.rs`, `command_gates.rs`, `command_record.rs`,
`command_expiry.rs`, `dispatch_simulator.rs`, `command_lifecycle.rs`,
`command_state.rs`) for the claim terms ("real-time", "frequency response",
"synthetic inertia", "relay", "inner-loop", "sub-cycle", "latency
guarantee"): **every occurrence is a boundary/prohibition statement** (module
control-status headers quoting §14.5, the `NOT_PROMISED_FUNCTIONS` list, the
no-guarantee constants). Zero claiming-context occurrences found. For Phase
5, display strings must repeat the acknowledgement-is-acceptance-not-delivery
and no-timing-guarantee wording (P5.13 consumes this boundary).

### D-P4.19-4 — Remaining functions stay in local certified controllers

Recorded in `TIMING_BOUNDARY_STATEMENT` verbatim per §14.5; site-local
safety and protection remain independent of FlexCore (§17.5 invariant 10).

## 4. File and test index

| File | Content |
|---|---|
| `crates/flexcore-application/src/fallback_policy.rs` | P4.17 + P4.18: policy variants, registry + default entry, validity conditions, simulated-operator review, activation engine over the frozen edges, bounded hold + escalation, audit sink seam, activation/escalation records |
| `crates/flexcore-application/src/timing_honesty.rs` | P4.19: boundary constants, conditions, journal-based measurement, recorder, measured distribution |
| `crates/flexcore-application/src/lib.rs` | Module registration + re-exports (appended additively; existing declarations untouched) |
| `crates/flexcore-application/tests/fallback_policy.rs` | 17 tests: three policy variants; validity-conditions construction error; named-policy/override/duplicate-scope registry rules; explicit-default entry resolution; FAILED→FALLBACK and EXPIRED→FALLBACK with full record + event; trigger-not-configured (EXPIRED with a FAILED-only policy); trigger/state-spoof rejection; audit-failure fail-closed; hold active/expiry/escalation (both targets); no-bounded-hold case; full-gate-chain recovery; FALLBACK→ISSUED rejection; structural no-universal-zero-fallback scan |
| `crates/flexcore-application/tests/timing_honesty.rs` | 7 tests: five-command §24 step 8 measurement with distribution assertions; report text carries conditions + boundary statements; empty recorder errors; FAILED command yields no sample; non-finite/negative deviation rejected; boundary constants; unpromised-function list; no best-case/target surface |

Seam reuse (no restatement): the command store is the P4.13
`CommandGateStore` seam; the journal replay is the P4.14
`replay_full_lifecycle`; the outbox/encoder/ids/clock are the same injected
ports the P4.11–P4.13 gates use.

## 5. Honest limitations

- **Nothing was built, run, installed, or executed.** The loop bans held for
  this wave. Every test above is WRITTEN, never executed; "implemented,
  awaiting human build/test" is the honest status of every criterion. The
  structural scan (D-P4.17-1 layer 3) runs only when the suite runs.
- The no-universal-fallback source scan is scoped to
  `flexcore-application/src/*.rs` (one directory level, non-recursive) — the
  recorded in-test form of the ledger's "code search for a universal
  fallback". A repo-wide scan is the human verification step.
- The P4.19 claim scan (D-P4.19-3) is a recorded grep over the Phase 4
  dispatch sources, not a CI-enforced repo-wide check; a test cannot grep
  the whole repository. Phase 5 display strings are out of this phase's
  file ownership.
- No escalation updates the stored activation row (the escalation is a
  separate audit row keyed by command); correlating an activation with its
  later escalation is by `command_id` + timestamps. P4.30's audit wiring
  owns any richer correlation.
- `valid_until` bounds on the test fixtures are chosen so the expiry
  fixtures lapse at the right milestones; the P4.15 sweep (another wave's
  file) is the production expiry driver — this wave drives expiry only
  through the P4.10 `record_expiry` guard.
- Bounded-hold expiry uses millisecond counts against the injected
  controlled clock; no timer thread exists in this crate (scheduling the
  evaluation is composition-root work, per the port/adapter split).
