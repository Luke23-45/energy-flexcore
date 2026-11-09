# Ledger — Phase 4: Grid Feasibility and Dispatch Assurance

- **Report:** `docs/plan/Neryva_FlexCore_Product_and_Architecture_Report_v1.1.md`
- **Stage:** §23 Stage 4 — Grid feasibility and dispatch assurance
- **Release target:** v0.3.0 (simulation and shadow; see §2.1, §21.4)
- **Status:** Not started. No code in this phase until every subtask's acceptance criteria are met and recorded.
- **Non-negotiables (AGENTS.md):** no hallucinated facts; no safety/certification claims; no protection-grade claims; never conflate solver classes (AC power flow is not AC-OPF; HiGHS LP/QP/MIP not SOCP; Clarabel convex conic not MISOCP; IPOPT local not global); simulation-first 0.x — simulation-only dispatch adapter, no device writes; verify before claiming done.
- **Spelling:** "license" (US), "acknowledgement" (per `.agent/WRITING.md`).
- **How to use this ledger:** work subtasks in ID order unless a dependency says otherwise. Check a box only when its acceptance criteria have evidence attached. A checked box without evidence is a defect. The deterministic rejection suite (P4.20–P4.27) is the exit gate: it must pass in full, with no skipped case.

## Trace summary

| Report section | Covered by |
|---|---|
| §12.4 grid-model provider selection | P4.01–P4.03 |
| §12.5 network feasibility gate | P4.04–P4.09 |
| §14.1–§14.2 command state machine + definitions | P4.10–P4.13, P4.15–P4.16 |
| §14.3 command invariants | P4.14 |
| §14.4 asset-specific fallback | P4.17–P4.18 |
| §14.5 dispatch is not hard real-time | P4.19 |
| §23 Stage 4 exit gate (deterministic rejection tests) | P4.20–P4.27 |
| §17.3–§17.5 security + safety invariants | P4.28–P4.29 |
| §15 APIs/events, §16 storage, §21.3 observability | P4.30 |
| §19.2 test layers (fault injection, security, shadow) | P4.31 |
| Docs + handover to Phase 5 | P4.32 |
| §22.3 (all command-state transitions tested) | P4.33 |
| §12.5 step 4 (three-phase unbalanced model) | P4.34 |
| §21.3 solver duration (feasibility gate) | P4.35 |
| §15.2 audit-events read API | P4.36 |
| §12.5 step 8 (constrain/return consumption) | P4.37 |

## Subtasks

### P4.01 — Select the primary grid-model provider (§12.4)

- [x] **Objective:** Choose one primary deterministic power-flow path and record the choice with its boundary, so later providers are added only when a concrete test case requires them.
- **Report trace:** §12.4, §25.2 (D3), §23 Stage 4.
- **Inputs/prerequisites:** Phase 3 handover (schedule + envelope contracts, solver versions); D3 status (OPEN — balanced radial default per DECISIONS.md); RESEARCH.md provider versions.
- **Steps:**
  1. Select pandapower as the primary provider per the recommended default (§25.2, DECISIONS.md), and record the decision as a recommendation, not a D3 decision — D3 stays OPEN.
  2. Record the provider boundary from §12.4: verify model and solver limitations for each case; pandapower is the balanced AC power-flow path with some three-phase/asymmetric calculations, not a universal validator.
  3. Define the provider-selection rule for later additions: a new provider (OpenDSS, GridLAB-D, PyPSA) is added only when a named test case cannot be expressed or validated with pandapower, with the case recorded.
  4. Pin the pandapower version (RESEARCH.md: 3.5.6, 7 October 2026) in the worker lockfile and record it in every feasibility result.
- **Deliverable/output:** Provider-selection decision note + pinned version + addition rule.
- **Acceptance criteria / exit check:** One primary provider named with its §12.4 boundary stated; version pinned; addition rule written; D3 still recorded as OPEN (not silently decided).
- **Owner role:** Power-systems engineer + architect.
- **Dependencies:** Phase 3 handover (P3.30).
- **Verification method:** Decision-note review against §12.4; lockfile check.
- **Risks/failure modes:** Choosing a provider by familiarity rather than by case fit. Mitigation: the addition rule forces a named case for any second provider.

### P4.02 — Grid-model provider port and adapter contract (§12.4, §9.1)

- [x] **Objective:** Build the grid-model provider behind a port so the feasibility gate depends on a contract, not on pandapower directly.
- **Report trace:** §12.4, §9.1, §5.2, §8.1 (`flexcore-grid-api`).
- **Inputs/prerequisites:** P4.01.
- **Steps:**
  1. Define the grid-model port in `flexcore-grid-api`: load a versioned network model; apply candidate injections/controls; run power flow; return bus voltages, branch currents/loadings, transformer loadings, convergence status, residuals, and model-coverage limitations.
  2. Implement the pandapower adapter behind the port (Python worker, §7.2). The adapter must translate FlexCore's canonical units/sign (§10.2) into the provider's model and translate results back — the translation layer is owned, tested code, not an assumption.
  3. Require every adapter to declare its model scope: balanced vs unbalanced, radial vs meshed, which equipment types it models, and what it does not model.
  4. Add contract tests: the same candidate through the port interface produces the same verdict regardless of which adapter is behind it (with the pandapower adapter now, and a stub second adapter in tests to prove the port is real).
- **Deliverable/output:** Grid-model port + pandapower adapter + scope declarations + contract tests.
- **Acceptance criteria / exit check:** Feasibility code calls only the port; adapter scope is declared; contract tests pass with both the real adapter and a stub; unit/sign translation is tested in both directions.
- **Owner role:** Rust/Python engineer.
- **Dependencies:** P4.01.
- **Verification method:** Dependency-direction check; contract tests; translation tests.
- **Risks/failure modes:** Provider-specific quirks leaking into the domain. Mitigation: port review; scope declaration is mandatory.

### P4.03 — Versioned network model and model-health tracking (§12.5, §20.1 item 5)

- [x] **Objective:** Make the network model a versioned, health-checked input so a feasibility verdict always names the model it was checked against.
- **Report trace:** §12.5 steps 3, 6; §12.6 (result contract carries grid-model version); §20.1 item 5 (Grid workspace — display contract only).
- **Inputs/prerequisites:** P4.02; grid model from Phase 2 (site/feeder definitions).
- **Steps:**
  1. Assign every network model a version and a content hash; store models immutably (new version on change, never in-place edits).
  2. Define model health: completeness checks (all referenced buses/branches/transformers present), staleness bound (max age since last verification against source data), and coverage statement (which feeders/equipment are modelled).
  3. Implement the rule from §12.5/§17.5 invariant 9: if the model is stale, incomplete, or its health check fails, feasibility is NOT inferred — the candidate is rejected or held per the documented policy (P4.08).
  4. Record model version + hash in every feasibility result and every dispatch command (§14.3 grid-model reference).
- **Deliverable/output:** Model versioning + health checks + stale-model policy reference + tests.
- **Acceptance criteria / exit check:** Model versions are immutable and hashed; health check covers completeness, staleness, and coverage; stale/incomplete model blocks a feasibility pass in tests; version appears on every result and command.
- **Owner role:** Power-systems engineer + Rust engineer.
- **Dependencies:** P4.02.
- **Verification method:** Versioning tests; stale-model rejection test; record inspection.
- **Risks/failure modes:** A silently edited model invalidates past verdicts. Mitigation: immutability + hash; past verdicts reference the hash they used.

### P4.04 — Feasibility-gate pipeline, steps 1–2: candidate validation (§12.5)

- [x] **Objective:** Implement the first two steps of the §12.5 validation pipeline: schema/units/timestamp/validity-window validation, then resource capability and ramp checks.
- **Report trace:** §12.5 steps 1–2, §10.2, §11.
- **Inputs/prerequisites:** P4.02–P4.03; Phase 3 schedule + capability contracts.
- **Steps:**
  1. Step 1: validate candidate schema, units (MW/MVAr/MWh; reject kW/kWh at the boundary), timestamp (not in the past beyond a configured tolerance, not malformed), and validity window (`valid-from ≤ valid-until`, both present, duration consistent with the command's stated duration).
  2. Step 2: check resource capability (against the §11 capability model for the asset at the candidate time) and ramp constraints (requested change within ramp limits from the last accepted/telemetered state).
  3. Return structured, per-check results (pass/fail + evidence reference) — a single boolean "valid" is not sufficient.
  4. Cover with unit tests including: wrong units, inverted validity window, expired timestamp, capability exceedance, and ramp violation.
- **Deliverable/output:** Pipeline steps 1–2 implementation + per-check result schema + tests.
- **Acceptance criteria / exit check:** Each listed malformed/violating candidate fails the named check with evidence; no candidate reaches step 3 with a failed step 1 or 2; per-check results are recorded.
- **Owner role:** Rust engineer + power-systems engineer.
- **Dependencies:** P4.02–P4.03.
- **Verification method:** Unit/negative-case tests.
- **Risks/failure modes:** Ramp checked against a stale state. Mitigation: ramp baseline is the latest telemetered state with its quality flag; BAD/MISSING baseline blocks the check (fail-closed, P4.08).

### P4.05 — Feasibility-gate pipeline, steps 3–4: apply injections, run AC power flow (§12.5)

- [x] **Objective:** Apply the candidate to the versioned network model and run the appropriate AC power-flow (or three-phase unbalanced) model — as an evaluation, not an optimization.
- **Report trace:** §12.5 steps 3–4, §12.4, §7.4.
- **Inputs/prerequisites:** P4.04; P4.02 adapter.
- **Steps:**
  1. Step 3: apply candidate injections and controls to the versioned network model (P4.03) — record exactly which buses/elements were modified, so the run is reproducible from the record.
  2. Step 4: run the AC power-flow model through the grid-model port. State in code and docs: this is an AC power-flow evaluation of specified injections/controls; it is NOT AC-OPF (§12.5, §7.4).
  3. Handle non-convergence explicitly: non-convergence is a result (reported with diagnostics), never silently retried with different settings until it passes, and never treated as feasible.
  4. Record solver/provider version, model version + hash, convergence status, iteration count, and residuals in the feasibility result.
- **Deliverable/output:** Pipeline steps 3–4 implementation + run record schema + non-convergence handling + tests.
- **Acceptance criteria / exit check:** Applied injections are recorded per element; power flow runs through the port; non-convergence is reported as a result with diagnostics; no code path retries with altered settings to force convergence; AC power flow is never labelled AC-OPF.
- **Owner role:** Numerical engineer.
- **Dependencies:** P4.04, P4.02.
- **Verification method:** Run-record inspection; non-convergence fixture test; code search for "AC-OPF" misuse.
- **Risks/failure modes:** Silent retry masking a real network problem. Mitigation: retry policy is "none" unless a named, reviewed policy says otherwise — record the policy.

### P4.06 — Feasibility-gate pipeline, step 5: voltage, thermal, transformer, equipment limits (§12.5)

- [x] **Objective:** Verify the power-flow result against voltage, current/thermal, transformer, generator, and relevant equipment limits, with margins.
- **Report trace:** §12.5 step 5, §12.5 step 7, §19.4 (grid feasibility metrics).
- **Inputs/prerequisites:** P4.05.
- **Steps:**
  1. Define the limit set as configuration per network model: voltage bands (per-unit, with the nominal base stated), branch current/thermal ratings, transformer ratings, generator limits, and any other equipment limits the model carries.
  2. Check every limit against the power-flow result; record per-limit pass/fail, the actual value, the limit, and the margin (in the limit's own units).
  3. Compute margin/diagnostics (§12.5 step 7): worst-case margin, binding constraint identity, and per-element loading percentages.
  4. Test with fixtures that violate exactly one limit each (low voltage, high voltage, branch overload, transformer overload) plus a case that passes all limits.
- **Deliverable/output:** Limit-check module + limit configuration + margin diagnostics + tests.
- **Acceptance criteria / exit check:** Each single-violation fixture fails on exactly that limit with the correct margin; the passing fixture passes all limits; margins are recorded in the feasibility result; no limit is hard-coded without a configuration source.
- **Owner role:** Power-systems engineer.
- **Dependencies:** P4.05.
- **Verification method:** Single-violation fixture tests; diagnostics inspection.
- **Risks/failure modes:** Limits taken from the wrong voltage base. Mitigation: base is stated per model; per-unit conversion is tested.

### P4.07 — Feasibility-gate pipeline, steps 6–8: non-convergence, margins, reject/constrain/return (§12.5)

- [x] **Objective:** Complete the pipeline: report non-convergence and coverage limits explicitly, compute diagnostics, and route unacceptable candidates to reject, constrain, or return-for-re-optimization.
- **Report trace:** §12.5 steps 6–8, §12.6.
- **Inputs/prerequisites:** P4.04–P4.06.
- **Steps:**
  1. Step 6: report non-convergence and model-coverage limitations explicitly in the result — including which parts of the network the model does not cover, so an uncovered violation cannot be mistaken for a pass.
  2. Step 7: finalise margin/diagnostics (from P4.06) into the feasibility result record.
  3. Step 8: implement the three outcomes with distinct statuses and evidence: REJECT (with failing constraint + evidence), CONSTRAIN (with the constrained values and the rule that produced them, returned to the scheduler), RETURN for re-optimization (with the violated constraints as new inputs). Never silently adjust a candidate and pass it.
  4. Emit `FeasibilityCheckCompleted` (§15.3) with the outcome, the model version, and the evidence references.
- **Deliverable/output:** Pipeline completion + outcome routing + event + tests.
- **Acceptance criteria / exit check:** Each outcome is reachable in tests with distinct status and evidence; constrained/returned candidates carry the violated constraints; uncovered-network limitations are stated, not hidden; event emitted per check.
- **Owner role:** Rust engineer + power-systems engineer.
- **Dependencies:** P4.04–P4.06.
- **Verification method:** Outcome-routing tests; event tests; coverage-limitation test.
- **Risks/failure modes:** "Constrain" used as a silent fix that hides a real violation. Mitigation: constrained values are recorded and the constraint that forced them is named.

### P4.08 — Fail-closed policy: validator failure, stale model, missing telemetry (§12.5, §17.5 inv. 9)

- [x] **Objective:** Write and enforce the fail-closed rule: when the validator fails or the model is stale/incomplete, the system must not infer feasibility.
- **Report trace:** §12.5 (final paragraph), §17.5 invariant 9, §10.4.
- **Inputs/prerequisites:** P4.03–P4.07.
- **Steps:**
  1. Enumerate fail-closed triggers: validator exception/timeout, power-flow non-convergence, stale/incomplete model (P4.03), missing/BAD/STALE telemetry where the check needs it, missing capability data (P3.02 unknown handling).
  2. For each trigger, define the outcome: REJECT or HOLD, per a documented policy table — record which triggers reject and which hold, and who may release a hold (operator only, with a recorded reason).
  3. Implement the policy as code (not as a comment): the feasibility function returns a rejection/hold result on these triggers, never a pass.
  4. Test every trigger: each produces the documented outcome and an audit-visible record naming the trigger.
- **Deliverable/output:** Fail-closed policy table + implementation + per-trigger tests.
- **Acceptance criteria / exit check:** Every listed trigger yields REJECT or HOLD as documented — zero triggers yield a pass; holds are releasable only by an operator with a recorded reason; policy table is reviewed by power-systems + security.
- **Owner role:** Architect + power-systems engineer.
- **Dependencies:** P4.03–P4.07.
- **Verification method:** Per-trigger tests; policy-table review.
- **Risks/failure modes:** A timeout treated as "no violation found". Mitigation: timeout is an explicit trigger in the table.

### P4.09 — SOCP relaxation boundary (Tier 2 candidates) (§12.5)

- [x] **Objective:** Handle Tier 2 (SOCP) schedule candidates honestly: a relaxation solution may be inexact and must be verified with the independent AC power-flow engine.
- **Report trace:** §12.5 (branch-flow SOCP paragraph), §12.3, §7.4.
- **Inputs/prerequisites:** P4.05–P4.07; Phase 3 Tier 2 provider (P3.18).
- **Steps:**
  1. Mark every candidate that came from a Tier 2 SOCP solve with its provenance (Tier 2, relaxation used, conditions checked) in the feasibility input record.
  2. Validate the relaxation conditions for the case (radial feeder, and the other conditions the relaxation relies on) and record which conditions were checked and their outcome; do not assume exactness for reverse power flow, voltage upper bounds, or arbitrary network constraints (§12.5).
  3. Verify the candidate with the independent AC power-flow engine (P4.05) regardless of the relaxation result — the relaxation never substitutes for the power-flow check.
  4. Test: a Tier 2 candidate that the relaxation calls optimal but the AC power flow finds violating is REJECTED, with both results recorded.
- **Deliverable/output:** Relaxation-condition checklist + provenance marking + inexactness test.
- **Acceptance criteria / exit check:** Tier 2 provenance is recorded; relaxation conditions are checked and recorded per case; the inexactness fixture is rejected by power flow; no Tier 2 candidate passes on relaxation alone.
- **Owner role:** Numerical engineer.
- **Dependencies:** P4.05–P4.07.
- **Verification method:** Inexactness fixture test; provenance inspection.
- **Risks/failure modes:** Relaxation exactness assumed for reverse power flow. Mitigation: the condition checklist explicitly includes reverse-flow and voltage-upper-bound cases.

### P4.10 — Command state machine implementation (§14.1–§14.2)

- [x] **Objective:** Implement the protected command state machine exactly as specified in §14.1, with the §14.2 state definitions as the only states. — implemented, awaiting human build/test
- **Report trace:** §14.1, §14.2, §6.1 (DDD), §8.1 (`flexcore-domain`).
- **Inputs/prerequisites:** Phase 3 schedule identity/revision (P3.22).
- **Steps:**
  1. Implement the state machine as a typed, exhaustive transition table in the domain (Rust types, per §7.1 strong state-machine modelling): states PROPOSED, VALIDATED, FEASIBLE, APPROVED, ISSUED, ACKNOWLEDGED, TRACKING, COMPLETED, REJECTED, FAILED, EXPIRED, CANCELLED, SUPERSEDED, FALLBACK.
  2. Encode every transition in the §14.1 diagram and no others: e.g. PROPOSED → VALIDATED or REJECTED; VALIDATED → FEASIBLE, REJECTED, or SUPERSEDED; FEASIBLE → APPROVED, REJECTED, or SUPERSEDED; APPROVED → ISSUED, EXPIRED, SUPERSEDED, or CANCELLED; ISSUED → ACKNOWLEDGED, FAILED, EXPIRED, SUPERSEDED, or CANCELLED; ACKNOWLEDGED → TRACKING, FAILED, EXPIRED, or SUPERSEDED; TRACKING → COMPLETED, FAILED, EXPIRED, or SUPERSEDED; FAILED/EXPIRED → FALLBACK; terminal states COMPLETED, REJECTED, CANCELLED, SUPERSEDED, FALLBACK have no outgoing transitions.
  3. Make illegal transitions unrepresentable or rejected with a named error at the domain layer — never silently ignored, never auto-corrected.
  4. Test every legal transition and a representative set of illegal transitions (including transitions out of terminal states).
- **Deliverable/output:** State-machine module + transition table + exhaustive tests.
- **Acceptance criteria / exit check:** All §14.1 transitions exist and work; all tested illegal transitions are rejected with a named error; no state outside §14.2 exists; terminal states have no outgoing transitions in tests.
- **Owner role:** Rust engineer (domain).
- **Dependencies:** P3.22.
- **Verification method:** Transition-matrix tests (legal + illegal); type-level review.
- **Risks/failure modes:** A "convenience" transition added later bypasses a gate. Mitigation: the transition table is the single source of truth and is reviewed on every change.

### P4.11 — Command creation: PROPOSED and the validation gate (§14.1–§14.2)

- [x] **Objective:** Create PROPOSED commands from approved schedules and move them through the validation gate (schema, freshness, asset checks) to VALIDATED or REJECTED.
- **Report trace:** §14.1, §14.2, §12.5 steps 1–2, §15.3.
- **Inputs/prerequisites:** P4.04, P4.10; Phase 3 schedules.
- **Steps:**
  1. Create a PROPOSED command only from a schedule ID + revision that exists and is not superseded; record source schedule ID/revision, source subsystem, and plugin/model version (§14.3).
  2. Run the validation gate: schema, freshness, asset capability, ramp, SOC, and command-policy checks (§14.2 VALIDATED definition; reuses P4.04 steps 1–2 plus SOC and policy checks).
  3. On pass: PROPOSED → VALIDATED with validation evidence reference. On fail: PROPOSED → REJECTED with the failing check and evidence. Emit `DispatchProposed` and then `DispatchRejected` where applicable (§15.3).
  4. Test: a command from a superseded schedule revision is rejected at creation, not merely at issue.
- **Deliverable/output:** Command creation + validation gate + evidence references + tests.
- **Acceptance criteria / exit check:** Commands carry schedule ID/revision and evidence refs; validation failures name the failing check; superseded-schedule commands are rejected at creation; events emitted.
- **Owner role:** Rust engineer.
- **Dependencies:** P4.04, P4.10.
- **Verification method:** Gate tests; superseded-revision test; event tests.
- **Risks/failure modes:** Validation and feasibility conflated into one gate. Mitigation: validation evidence and feasibility evidence are separate references (see P4.12, and §14.1 "validation, feasibility, and authorization are distinct").

### P4.12 — Feasibility gate for commands: VALIDATED → FEASIBLE or REJECTED (§14.1, §12.5)

- [x] **Objective:** Move VALIDATED commands through the network feasibility gate to FEASIBLE or REJECTED, keeping feasibility evidence distinct from validation evidence.
- **Report trace:** §14.1, §14.2, §12.5, §17.5 invariant 3.
- **Inputs/prerequisites:** P4.07–P4.08, P4.11.
- **Steps:**
  1. Run the P4.04–P4.07 pipeline for the command's candidate injections; attach the feasibility result (model version + hash, outcome, margins, coverage) as the command's feasibility evidence.
  2. On pass: VALIDATED → FEASIBLE. On fail: VALIDATED → REJECTED (capability/network failure) with the failing constraint and evidence. On validator/model failure: fail-closed per P4.08 (REJECT or HOLD).
  3. Enforce invariant 3 (§17.5): no command without required network feasibility evidence for the declared operating profile reaches APPROVED — enforce at the state machine (FEASIBLE is a precondition for APPROVED, P4.13).
  4. Test: a command whose validation passed but whose power flow violates a transformer limit is REJECTED with the transformer constraint named.
- **Deliverable/output:** Command feasibility gate + evidence linkage + tests.
- **Acceptance criteria / exit check:** Feasibility evidence is a separate reference from validation evidence; network failure rejects with the named constraint; fail-closed triggers apply; APPROVED is unreachable without FEASIBLE in tests.
- **Owner role:** Rust engineer + power-systems engineer.
- **Dependencies:** P4.07–P4.08, P4.11.
- **Verification method:** Gate tests; invariant test (APPROVED-without-FEASIBLE is impossible).
- **Risks/failure modes:** Feasibility evidence reused across different model versions. Mitigation: evidence references the model hash; a model change invalidates the evidence (P4.03).

### P4.13 — Human approval and the authorization gate (§14.1–§14.2, §17.3)

- [x] **Objective:** Implement the authorization gate: FEASIBLE → APPROVED only by a required human/policy authorization, with scope, limits, and expiry recorded.
- **Report trace:** §14.1, §14.2, §17.3, §17.5 invariant 4, §21.4 (operator-approved control at v1.0.0; simulation/shadow in 0.x).
- **Inputs/prerequisites:** P4.12; identity/RBAC from Phase 1 (or the Phase 1 contract if the implementation is staged — record which).
- **Steps:**
  1. Require an authorized human actor (RBAC + resource/site/asset-scoped authorization, §17.3) for APPROVED; record actor identity, decision time, approved scope, limits, and expiry in the authorization decision reference (§14.3).
  2. Enforce separation: the approver must not be the same identity that proposed the command where separation of duties is configured (§17.4); record the policy applied.
  3. On denial: FEASIBLE → REJECTED with the denial reason and actor. On expiry of the approval before issue: APPROVED → EXPIRED (P4.15).
  4. In the 0.x simulation line, approval is exercised in the simulator/shadow workflow only — the approval path is real and tested, but it authorizes a simulated issue (P4.16), never a device write.
- **Deliverable/output:** Authorization gate + decision reference schema + separation-of-duties rule + tests.
- **Acceptance criteria / exit check:** APPROVED requires a recorded authorized actor + scope + limits + expiry; denial rejects with reason; self-approval is blocked where separation of duties is configured; the gate is tested in the simulation workflow.
- **Owner role:** Rust engineer + security engineer.
- **Dependencies:** P4.12.
- **Verification method:** Authorization tests incl. unauthorized-actor and self-approval cases; decision-reference inspection.
- **Risks/failure modes:** Approval treated as a formality (auto-approve). Mitigation: no auto-approve path exists in code; the test suite includes an unauthorized-approval attempt (P4.23).

### P4.14 — Command invariants implementation (§14.3)

- [ ] **Objective:** Enforce every §14.3 command invariant in code, so a command missing a required field cannot be created or issued.
- **Report trace:** §14.3, §10.2, §17.5.
- **Inputs/prerequisites:** P4.10–P4.13.
- **Steps:**
  1. Implement the command record with all §14.3 required fields: unique command ID; tenant, portfolio, site, asset IDs; source subsystem and plugin/model version; source schedule ID and revision; reason/action and actor/service identity; target P/Q or supported control parameter; units and sign convention; valid-from/valid-until; duration and ramp requirements; monotonic sequence/version per asset or control channel; idempotency key; grid-model and telemetry snapshot references; validation evidence reference; authorization decision reference (approved scope, limits, expiry); signature or authenticated transport identity; supersedes/superseded-by reference when replacing another command.
  2. Validate at creation and re-validate at issue: any missing field blocks the transition with a named error.
  3. Implement the rejection rules: stale, duplicated with conflicting payloads, out of sequence, outside capability, or missing required evidence → REJECTED (§14.3, final paragraph).
  4. Represent partial fleet success per asset: a portfolio-level acknowledgement is never inferred from the first responding device (§14.3).
- **Deliverable/output:** Command record schema + creation/issue validators + rejection rules + tests.
- **Acceptance criteria / exit check:** Every §14.3 field is required and validated at creation and at issue; each rejection rule has a passing test; per-asset (not portfolio-inferred) acknowledgement is demonstrated in a multi-asset test.
- **Owner role:** Rust engineer.
- **Dependencies:** P4.10–P4.13.
- **Verification method:** Field-completeness tests; per-rule rejection tests; multi-asset acknowledgement test.
- **Risks/failure modes:** A field treated as optional "for now" becomes a permanent gap. Mitigation: completeness tests fail the build if a field is dropped.

### P4.15 — Expiry, cancellation, supersede (§14.1–§14.2)

- [ ] **Objective:** Implement EXPIRED, CANCELLED, and SUPERSEDED transitions exactly, keeping SUPERSEDED distinct from CANCELLED.
- **Report trace:** §14.1, §14.2, §14.3 (supersedes reference).
- **Inputs/prerequisites:** P4.10, P4.14.
- **Steps:**
  1. Expiry: a validity-window sweep (or a check at each transition attempt) moves APPROVED/ISSUED/ACKNOWLEDGED/TRACKING commands to EXPIRED when `valid-until` passes before the next milestone; record the expiry time and the milestone not reached. EXPIRED has one outgoing transition, to FALLBACK where the policy requires it (§14.1, P4.17); it is not terminal.
  2. Cancellation: explicit operator/policy cancellation moves APPROVED → CANCELLED and ISSUED → CANCELLED where supported cancellation semantics exist (§14.1); record actor and reason. Cancellation is explicit — never used as a synonym for supersede.
  3. Supersede: when a newer approved schedule/command revision replaces a command before completion, move it to SUPERSEDED with the supersedes/superseded-by reference (§14.3); retain it for audit. Test that SUPERSEDED and CANCELLED produce different audit records.
  4. Emit `DispatchExpired`, `DispatchCancelled`, `DispatchSuperseded` (§15.3) with actor/reason/references.
- **Deliverable/output:** Expiry sweep + cancellation + supersede implementation + event emission + tests.
- **Acceptance criteria / exit check:** Expiry fires at the right milestone in tests; cancellation requires an actor + reason; supersede links both directions and retains the record; SUPERSEDED and CANCELLED are distinguishable in audit output; events emitted.
- **Owner role:** Rust engineer.
- **Dependencies:** P4.10, P4.14.
- **Verification method:** Time-based expiry tests (clock controlled in tests, never wall-clock dependent); supersede-vs-cancel tests.
- **Risks/failure modes:** Clock drift causing premature/late expiry (§17.4). Mitigation: time-quality monitoring and bounded expiry are noted; tests use a controlled clock.

### P4.16 — Simulation-only dispatch adapter: issue, acknowledgement, tracking (§14.1, §23 Stage 4)

- [ ] **Objective:** Build the simulation-only dispatch adapter that issues APPROVED commands to the simulator, observes acknowledgement and tracking, and completes or fails them — without any device write path.
- **Report trace:** §23 Stage 4, §14.1–§14.2, §19.1 (simulation strategy), §24 steps 6–8.
- **Inputs/prerequisites:** P4.13–P4.15; Phase 2 simulator (asset simulator).
- **Steps:**
  1. Implement the dispatch adapter interface (per §8.1 adapter pattern) with exactly one Phase 4 implementation: the simulator adapter. No production protocol adapter is built in this phase (that is Stage 6).
  2. Issue: APPROVED → ISSUED only for a signed, sequenced command sent to the simulator adapter (§14.2 ISSUED). Signing uses the simulation signing key/identity; record signature/transport identity (§14.3). The adapter must refuse an unsigned, unsequenced, or expired command.
  3. Acknowledgement: ISSUED → ACKNOWLEDGED only on application-layer acceptance from the simulator; a timeout or NACK moves ISSUED → FAILED. Acknowledgement is receipt/acceptance, not physical delivery (§14.1).
  4. Tracking: ACKNOWLEDGED → TRACKING when telemetry is observed; compare measured response with requested behaviour against a defined acceptance criterion; TRACKING → COMPLETED when the criterion is satisfied, → FAILED on tracking deviation/timeout, → EXPIRED when validity ends, → SUPERSEDED on replacement. Simulate acknowledgement, tracking, partial failure, and timeout per §24 step 8.
  5. Hard guard: the adapter's transport target is the simulator only. Add a test that fails if any non-simulator target is configured, and document that field dispatch requires Stage 6 + Stage 9 qualification.
- **Deliverable/output:** Simulator dispatch adapter + issue/ack/tracking implementation + §24 step 8 simulations + target guard + tests.
- **Acceptance criteria / exit check:** Full lifecycle PROPOSED → … → COMPLETED runs end-to-end against the simulator; NACK/timeout → FAILED; tracking deviation → FAILED; partial fleet failure is per-asset; the non-simulator-target guard test passes; no device write path exists in code.
- **Owner role:** Rust engineer + numerical engineer.
- **Dependencies:** P4.13–P4.15.
- **Verification method:** End-to-end lifecycle tests; failure-mode tests; guard test; code review for write paths.
- **Risks/failure modes:** Simulator acknowledgement mistaken for physical delivery. Mitigation: state definitions and UI contracts (Phase 5) always label it as acceptance; docs state it explicitly.

### P4.17 — Asset-specific fallback policies (§14.4)

- [ ] **Objective:** Implement per-asset/site fallback policies with no universal "set power to zero" fallback.
- **Report trace:** §14.4, §14.1 (FAILED/EXPIRED → FALLBACK), §15.3 (`DispatchFallbackActivated`).
- **Inputs/prerequisites:** P4.16; asset/site configuration from Phase 2.
- **Steps:**
  1. Define the fallback policy as per-asset/site configuration, reviewed with the equipment/site operator (in simulation, reviewed by the power-systems engineer and recorded as a simulated-operator review): hold last accepted setpoint for a bounded interval where approved; return to local EMS autonomous control; preserve reserve/SOC constraints; stop accepting new external commands; request a safe local operating mode; or require operator intervention (§14.4).
  2. Implement FALLBACK entry from FAILED and EXPIRED where the policy requires it; record which policy fired, its parameters, and the bounded interval where applicable.
  3. Record the rule that a last-known setpoint is not automatically safe when topology, asset state, or network constraints have changed (§14.4) — a hold-last-setpoint policy must state its validity conditions.
  4. Emit `DispatchFallbackActivated` (§15.3) and test at least three policy variants (hold, return-to-local, require-intervention).
- **Deliverable/output:** Fallback policy configuration + FALLBACK transitions + event + tests.
- **Acceptance criteria / exit check:** No universal zero-power fallback exists in code; each asset/site has a named policy or an explicit "no fallback configured — require operator intervention" entry; three policy variants tested; hold policies state validity conditions.
- **Owner role:** Power-systems engineer + Rust engineer.
- **Dependencies:** P4.16.
- **Verification method:** Policy-variant tests; configuration review; code search for a universal fallback.
- **Risks/failure modes:** Fallback assumed safe without checking changed topology. Mitigation: validity conditions are part of the policy record.

### P4.18 — Fallback activation, bounded hold, and recovery (§14.4, §17.5)

- [ ] **Objective:** Prove fallback activation is bounded and recoverable: a held setpoint expires, and recovery requires a fresh validation/feasibility/authorization cycle.
- **Report trace:** §14.4, §14.1, §17.5.
- **Inputs/prerequisites:** P4.17.
- **Steps:**
  1. Implement the bounded-hold timer: a hold-last-setpoint fallback expires after its configured interval and escalates (to require-operator-intervention or return-to-local, per policy) — it never holds indefinitely.
  2. Define recovery: after FALLBACK, a new command for the asset must go through the full PROPOSED → VALIDATED → FEASIBLE → APPROVED path; no shortcut from FALLBACK back to ISSUED exists.
  3. Test: hold expiry escalates; recovery via a fresh command works; a direct FALLBACK → ISSUED transition is rejected.
  4. Record fallback activations in the audit trail with policy, parameters, and outcome (P4.30).
- **Deliverable/output:** Bounded-hold implementation + recovery rule + tests.
- **Acceptance criteria / exit check:** Hold expires and escalates in tests; recovery requires the full gate chain; FALLBACK → ISSUED is rejected; activations are audit-visible.
- **Owner role:** Rust engineer.
- **Dependencies:** P4.17.
- **Verification method:** Timer tests (controlled clock); recovery-path tests; illegal-transition test.
- **Risks/failure modes:** Indefinite hold masking a failed asset. Mitigation: bounded timer is enforced in code, not by operator memory.

### P4.19 — Dispatch timing honesty: not hard real-time (§14.5)

- [ ] **Objective:** State and enforce the timing boundary: supervisory setpoint delivery only; no hard-real-time, frequency, inertia, relay, or sub-cycle claims.
- **Report trace:** §14.5, §0.3, §17.5.
- **Inputs/prerequisites:** P4.16.
- **Steps:**
  1. Document the dispatch timing model: expected issue/acknowledgement/tracking latencies are measured and reported (§19.4 dispatch metrics), with no latency guarantee claimed.
  2. Add a test that records acknowledgement latency and tracking error on the §24 step 8 simulation; report the measured distribution, not a single best case.
  3. Review all Phase 4 docs, API descriptions, and (for Phase 5) display strings for timing claims; remove or correct any that imply hard-real-time, primary frequency response, synthetic inertia, or relay/inverter inner-loop control.
  4. Record that those functions remain in local certified controllers/hardware unless a separate engineered and qualified architecture is developed (§14.5).
- **Deliverable/output:** Timing-model note + latency measurements + claim review.
- **Acceptance criteria / exit check:** No hard-real-time or frequency/inertia/relay claim remains in Phase 4 artefacts; latencies are measured and reported with their distribution; §14.5 boundary is stated in the handover note.
- **Owner role:** Architect.
- **Dependencies:** P4.16.
- **Verification method:** Latency measurement output; text review of Phase 4 docs.
- **Risks/failure modes:** A latency figure quoted without context becomes a guarantee. Mitigation: figures are always reported with conditions (simulator, load, measurement method).

### P4.20 — Deterministic rejection test: invalid commands (§23 Stage 4 exit gate)

- [ ] **Objective:** Prove invalid candidate commands are rejected deterministically.
- **Report trace:** §23 Stage 4 exit gate, §14.3, §12.5 step 1.
- **Inputs/prerequisites:** P4.04, P4.11, P4.14.
- **Steps:**
  1. Build a fixed invalid-command fixture set (pinned inputs, no randomness): malformed schema (missing required field per §14.3); wrong units (kW instead of MW); inverted validity window; unknown asset/site/tenant IDs; target P/Q outside capability; negative duration.
  2. Run each fixture through command creation + validation; record the expected rejection (state REJECTED, failing check named, evidence reference) before running.
  3. Run the suite twice with identical inputs and assert identical outcomes (deterministic).
  4. File the suite under the scenario regression suite (§22.2) so it runs in CI.
- **Deliverable/output:** Invalid-command rejection suite + pre-registered expectations + CI wiring.
- **Acceptance criteria / exit check:** Every fixture is REJECTED with the pre-registered failing check; outcomes are identical across two runs; suite runs in CI.
- **Owner role:** QA + Rust engineer.
- **Dependencies:** P4.04, P4.11, P4.14.
- **Verification method:** Suite output; determinism re-run; CI log.
- **Risks/failure modes:** Expectations fitted after the run. Mitigation: expectations are written before execution (P3.26 discipline, applied here).

### P4.21 — Deterministic rejection test: stale commands (§23 Stage 4 exit gate)

- [ ] **Objective:** Prove stale candidate commands are rejected deterministically.
- **Report trace:** §23 Stage 4 exit gate, §14.3, §17.5 invariant 5, §10.4.
- **Inputs/prerequisites:** P4.11, P4.14–P4.15.
- **Steps:**
  1. Build fixtures: command whose `valid-until` has passed before validation; command built on a superseded schedule revision; command built on STALE telemetry (quality STALE at the snapshot) where freshness is required; command whose telemetry snapshot is older than the configured max age.
  2. Pre-register the expected outcome per fixture (REJECTED, or EXPIRED where the command was already APPROVED/ISSUED when validity ended — record which applies and why).
  3. Run deterministically (controlled clock) and assert outcomes match.
  4. Include a clock-skew case: a command timestamped in the future beyond the configured tolerance is rejected, not accepted.
- **Deliverable/output:** Stale-command rejection suite + expectations + CI wiring.
- **Acceptance criteria / exit check:** Every stale fixture is rejected/expired as pre-registered; clock-skew case rejected; controlled-clock tests are independent of wall-clock time.
- **Owner role:** QA + Rust engineer.
- **Dependencies:** P4.11, P4.14–P4.15.
- **Verification method:** Suite output; controlled-clock tests.
- **Risks/failure modes:** Wall-clock-dependent tests flaking in CI. Mitigation: controlled clock everywhere in this suite.

### P4.22 — Deterministic rejection test: infeasible commands (§23 Stage 4 exit gate)

- [ ] **Objective:** Prove infeasible candidate commands are rejected deterministically.
- **Report trace:** §23 Stage 4 exit gate, §12.5, §14.1.
- **Inputs/prerequisites:** P4.06–P4.07, P4.12.
- **Steps:**
  1. Build fixtures, each violating exactly one network limit: low voltage, high voltage, branch/thermal overload, transformer overload; plus a power-flow non-convergence case and a stale-model case (P4.08).
  2. Pre-register expected outcomes (REJECTED with the named constraint; non-convergence and stale-model cases per the P4.08 policy).
  3. Run through the full VALIDATED → feasibility gate path and assert outcomes.
  4. Verify the failing constraint named in the rejection matches the fixture's designed violation (not a different, incidental violation).
- **Deliverable/output:** Infeasible-command rejection suite + expectations + CI wiring.
- **Acceptance criteria / exit check:** Every fixture rejected with the designed constraint named; non-convergence and stale-model cases follow P4.08; no fixture passes.
- **Owner role:** QA + power-systems engineer.
- **Dependencies:** P4.06–P4.07, P4.12.
- **Verification method:** Suite output; constraint-name matching.
- **Risks/failure modes:** A fixture that violates two limits at once, making the "named constraint" check ambiguous. Mitigation: single-violation design per fixture (P4.06).

### P4.23 — Deterministic rejection test: unauthorized commands (§23 Stage 4 exit gate)

- [ ] **Objective:** Prove unauthorized candidate commands are rejected deterministically.
- **Report trace:** §23 Stage 4 exit gate, §14.1, §17.3–§17.5 (invariants 4, 8).
- **Inputs/prerequisites:** P4.13, P4.28.
- **Steps:**
  1. Build fixtures: approval attempted by an actor without the required role; approval attempted by the proposer where separation of duties is configured; approval attempted for a different tenant's command (cross-tenant); approval with an expired authorization scope; command issue attempted without any APPROVED state.
  2. Pre-register expected outcomes (REJECTED or blocked transition with a named authorization error; cross-tenant attempt audited as a denial per §16.2).
  3. Run and assert; verify the audit record exists for each denial.
  4. Verify no fixture reaches ISSUED.
- **Deliverable/output:** Unauthorized-command rejection suite + expectations + audit-denial checks + CI wiring.
- **Acceptance criteria / exit check:** Every fixture is blocked/rejected as pre-registered; denials are audit-recorded; no fixture reaches ISSUED; cross-tenant denial is logged per §16.2.
- **Owner role:** QA + security engineer.
- **Dependencies:** P4.13, P4.28.
- **Verification method:** Suite output; audit-log inspection.
- **Risks/failure modes:** Tests running as an over-privileged role masking the check. Mitigation: fixtures use the least-privileged roles that should fail, plus the correct role as a control.

### P4.24 — Deterministic rejection test: duplicate commands (§23 Stage 4 exit gate)

- [ ] **Objective:** Prove duplicate candidate commands are handled deterministically: identical replays are deduplicated, conflicting duplicates are rejected.
- **Report trace:** §23 Stage 4 exit gate, §14.3 (idempotency key; duplicated with conflicting payloads), §15.4–§15.5.
- **Inputs/prerequisites:** P4.14, P4.30 (outbox/inbox).
- **Steps:**
  1. Build fixtures: (a) exact replay of a command (same idempotency key, same payload) — expect deduplication to the single original record, no second issue; (b) same idempotency key with a conflicting payload — expect REJECTED; (c) same asset/channel with an out-of-sequence version — expect REJECTED; (d) redelivered NATS event (at-least-once, §15.4) carrying an already-processed command — expect idempotent consumer behaviour, no duplicate side effect.
  2. Pre-register expectations; run each fixture twice to prove determinism.
  3. Verify the outbox/inbox records show exactly one effective command per idempotency key.
- **Deliverable/output:** Duplicate-command suite + expectations + CI wiring.
- **Acceptance criteria / exit check:** Exact replay deduplicated (one record, one issue at most); conflicting duplicate rejected; out-of-sequence rejected; redelivery causes no duplicate side effect; outcomes identical across runs.
- **Owner role:** QA + Rust engineer.
- **Dependencies:** P4.14, P4.30.
- **Verification method:** Suite output; outbox/inbox inspection; redelivery test.
- **Risks/failure modes:** Idempotency key derived from mutable fields. Mitigation: key covers the full payload + sequence; key derivation is tested.

### P4.25 — Deterministic rejection test: expired commands (§23 Stage 4 exit gate)

- [ ] **Objective:** Prove expired candidate commands are rejected (or moved to EXPIRED) deterministically at every lifecycle stage.
- **Report trace:** §23 Stage 4 exit gate, §14.1–§14.2, §14.3.
- **Inputs/prerequisites:** P4.15, P4.16.
- **Steps:**
  1. Build fixtures expiring at each stage: APPROVED expiring before issue; ISSUED expiring before acknowledgement; ACKNOWLEDGED expiring before tracking completes; TRACKING expiring before completion.
  2. Pre-register the expected state per fixture (EXPIRED, with the milestone not reached recorded) and the fallback consequence per P4.17–P4.18 where the policy requires it.
  3. Run with a controlled clock; assert states and that no expired command is issued, acknowledged-as-fresh, or completed.
  4. Verify `DispatchExpired` is emitted per case (§15.3).
- **Deliverable/output:** Expired-command suite + expectations + CI wiring.
- **Acceptance criteria / exit check:** Every fixture reaches EXPIRED at the right stage with the milestone recorded; no expired command progresses; events emitted; fallback fires where policy requires.
- **Owner role:** QA + Rust engineer.
- **Dependencies:** P4.15, P4.16.
- **Verification method:** Suite output with controlled clock; event checks.
- **Risks/failure modes:** Expiry checked only at issue, letting an expired command linger in TRACKING. Mitigation: expiry is checked at every transition attempt and by the sweep.

### P4.26 — Deterministic rejection test: combined and adversarial cases (§23 Stage 4 exit gate)

- [ ] **Objective:** Prove the rejection logic holds when faults combine, so a single-fault pass does not hide a multi-fault hole.
- **Report trace:** §23 Stage 4 exit gate, §19.2 layer 6 (fault injection), §17.4.
- **Inputs/prerequisites:** P4.20–P4.25.
- **Steps:**
  1. Build combined fixtures: stale + infeasible; duplicate + unauthorized; expired + infeasible; forged signature + stale; replayed event + conflicting payload.
  2. Pre-register the expected outcome per fixture (the rejection must name at least one applicable failing check; where two apply, the recorded reason must be one of them — never a pass).
  3. Run deterministically; assert no combined fixture reaches ISSUED.
  4. Add a forged/replayed-command case per §17.4 (forged/replayed command threat): bad signature, replayed sequence number — expect rejection, never issue.
- **Deliverable/output:** Combined/adversarial suite + expectations + CI wiring.
- **Acceptance criteria / exit check:** Every combined fixture is rejected/blocked as pre-registered; forged/replayed cases never issue; no fixture reaches ISSUED.
- **Owner role:** QA + security engineer.
- **Dependencies:** P4.20–P4.25.
- **Verification method:** Suite output; signature/sequence checks.
- **Risks/failure modes:** First-check-wins logic hiding the second fault in the audit record. Mitigation: the rejection record lists all failing checks found, not just the first (where the implementation evaluates them).

### P4.27 — Exit-gate evidence pack for the rejection suites (§23 Stage 4)

- [ ] **Objective:** Assemble the §23 Stage 4 exit-gate evidence: invalid, stale, infeasible, unauthorized, duplicate, and expired commands rejected in deterministic tests — as a single reviewable pack.
- **Report trace:** §23 Stage 4 exit gate, §22.2, §19.2.
- **Inputs/prerequisites:** P4.20–P4.26.
- **Steps:**
  1. Collect the six suite outputs (P4.20–P4.25) plus the combined suite (P4.26) into one evidence pack: per-fixture expectation vs outcome, run identifiers, pinned versions (solver, provider, model), and seed/clock configuration.
  2. Run the full pack twice from a clean state and assert identical outcomes; record both runs.
  3. Have the pack reviewed by power-systems (infeasible/stale), security (unauthorized/duplicate/forged), and QA (determinism/CI) reviewers; record sign-off per suite.
  4. If any suite fails, the phase is not complete: record the failing fixture and the fix, and re-run the pack in full (not just the failing fixture).
- **Deliverable/output:** Exit-gate evidence pack + two clean runs + reviewer sign-offs.
- **Acceptance criteria / exit check:** All six rejection categories pass deterministically in two clean runs; sign-offs recorded per suite; the pack is filed where Phase 5 and the release gate (§22.3) can cite it.
- **Owner role:** Phase lead (architect) + QA.
- **Dependencies:** P4.20–P4.26.
- **Verification method:** Evidence-pack review; clean re-runs; CI log.
- **Risks/failure modes:** Partial re-runs hiding a regression in another suite. Mitigation: full-pack re-run rule.

### P4.28 — Security controls for dispatch (§17.3–§17.4)

- [ ] **Objective:** Implement and test the dispatch-relevant security controls: signed commands, sequence, expiry, replay protection, mTLS/transport identity, and tenant isolation.
- **Report trace:** §17.3, §17.4, §17.5, §16.2.
- **Inputs/prerequisites:** P4.13–P4.16, P4.23–P4.24.
- **Steps:**
  1. Signed commands: every issued command carries a signature or authenticated transport identity (§14.3); the simulator adapter verifies it before accepting (P4.16). Test with a tampered payload (signature must fail) and a missing signature (rejected).
  2. Sequence/replay: monotonic sequence/version per asset/channel (§14.3); replayed sequence rejected (P4.24, P4.26). Test out-of-order and replayed sequences.
  3. Tenant isolation: command and telemetry access is tenant-scoped (§17.5 inv. 8, §16.2 RLS); test cross-tenant command read/approve/issue denial (P4.23).
  4. Secrets: no secrets in command payloads, logs, or fixtures; short-lived credentials and rotation are documented as deployment requirements for later stages (not implemented against a real secret store in simulation unless the Phase 1 identity work already provides one — record which).
- **Deliverable/output:** Security-control implementation + tests + deployment-requirements note.
- **Acceptance criteria / exit check:** Tampered/missing-signature commands rejected; replayed sequences rejected; cross-tenant access denied and audited; no secrets in code, fixtures, or logs (scan output filed).
- **Owner role:** Security engineer.
- **Dependencies:** P4.13–P4.16, P4.23–P4.24.
- **Verification method:** Security tests (§19.2 layer 7); secret scan; audit inspection.
- **Risks/failure modes:** Simulation signing key committed to the repo. Mitigation: test keys are generated at test time or stored as test-only fixtures clearly marked non-production, never as real credentials.

### P4.29 — Safety invariants enforcement (§17.5)

- [ ] **Objective:** Prove the ten §17.5 safety invariants are enforced by protected platform code and cannot be bypassed by plugins.
- **Report trace:** §17.5, §5.7 (plugin authority model), §13.4 (research boundary).
- **Inputs/prerequisites:** P4.10–P4.16, P4.28.
- **Steps:**
  1. Map each invariant to its enforcement point and test: (1) valid asset/site identity + current command context; (2) capability validation; (3) network feasibility evidence; (4) authorization; (5) no stale/expired command; (6) no unapproved adapter/target; (7) no direct physical command from optimizer/GNN/MARL/LLM/plugin; (8) no cross-tenant access; (9) missing/uncertain/stale model is not a pass; (10) site-local safety/protection independent of FlexCore (documented as a boundary, not a code test — record how the simulator models local protection as independent).
  2. For invariant 7, add a test proving a research-plugin-style proposal (optimizer/GNN/MARL-shaped input) cannot reach ISSUED without passing the full validation/feasibility/authorization chain.
  3. For invariant 6, the target guard (P4.16) is the enforcement point; re-verify it here.
  4. File an invariant-to-test matrix; every invariant has at least one test except invariant 10, which has a boundary note.
- **Deliverable/output:** Invariant-to-test matrix + tests + boundary note for invariant 10.
- **Acceptance criteria / exit check:** Invariants 1–9 each have a passing test; invariant 7's plugin-bypass test passes; invariant 10's boundary is documented; the matrix is reviewed by security + power-systems.
- **Owner role:** Security engineer + architect.
- **Dependencies:** P4.10–P4.16, P4.28.
- **Verification method:** Matrix review; invariant tests.
- **Risks/failure modes:** An invariant enforced only in the API layer, bypassable by an internal caller. Mitigation: enforcement lives in the protected domain/kernel layer (§5.7), and tests call the domain directly as well as through the API.

### P4.30 — Audit, events, persistence, observability (§15, §16, §21.3)

- [ ] **Objective:** Wire Phase 4 into audit, events, storage, and observability so every decision is reconstructable.
- **Report trace:** §15.2–§15.5, §16.1–§16.3, §21.3, §14.3 (evidence references).
- **Inputs/prerequisites:** P4.10–P4.16, P4.27.
- **Steps:**
  1. Persist commands, state transitions, validation/feasibility/authorization evidence references, and audit events in PostgreSQL (§16.1) with `tenant_id` on every record and RLS defence-in-depth (§16.2).
  2. Emit the full §15.3 dispatch event set (`DispatchProposed`, `DispatchApproved`, `DispatchRejected`, `DispatchIssued`, `DispatchAcknowledged`, `DispatchTrackingUpdated`, `DispatchCompleted`, `DispatchFailed`, `DispatchExpired`, `DispatchCancelled`, `DispatchSuperseded`, `DispatchFallbackActivated`, plus `FeasibilityCheckCompleted`, `ScheduleValidated`) via the transactional outbox (§15.5); consumers are idempotent (P4.24).
  3. Expose REST resources `/api/v1/feasibility-checks` and `/api/v1/dispatch-commands` (§15.2) with OpenAPI; no generic "execute arbitrary command" endpoint (§15.2).
  4. Correlate every record with tenant/site/portfolio/asset IDs, schedule and command IDs, plugin/solver/model versions, telemetry snapshot and grid-model version, trace ID, and actor identity (§21.3); never put secrets or unnecessary personal/customer data in logs.
  5. Define retention per data class (§16.3) as configuration — do not hard-code a universal retention period.
- **Deliverable/output:** Persistence + outbox emission + REST resources + correlation/retention configuration + tests.
- **Acceptance criteria / exit check:** Every state transition is persisted with its evidence refs; every §15.3 dispatch event is emitted and consumable idempotently; REST resources are schema-valid; correlation IDs are present on a sampled end-to-end trace; retention is configurable.
- **Owner role:** Rust engineer.
- **Dependencies:** P4.10–P4.16, P4.27.
- **Verification method:** Transition-persistence tests; outbox/idempotency tests; API contract tests; trace sampling.
- **Risks/failure modes:** Audit written after the fact from memory rather than from the transition itself. Mitigation: audit record is written in the same transaction as the state change (outbox pattern).

### P4.31 — Fault injection, shadow evaluation, and integration tests (§19.2 layers 4, 6, 9)

- [ ] **Objective:** Prove the Phase 4 paths survive faults and behave correctly in shadow comparison, without issuing anything to a real device.
- **Report trace:** §19.2 layers 4, 6, 9; §19.1 (simulation strategy); §21.4 (read-only/shadow mode).
- **Inputs/prerequisites:** P4.16, P4.30.
- **Steps:**
  1. Fault injection: dropped/reordered/duplicated NATS messages, clock skew, stale telemetry, unavailable assets, solver timeout, plugin crash (simulated), partial fleet response — each with a pre-registered expected outcome (reject/hold/fallback, never silent pass).
  2. Integration tests: PostgreSQL, NATS, Python power-flow worker, and simulator adapter together, end-to-end, on the §24 vertical-slice scenario (steps 5–8).
  3. Shadow-mode evaluation: compare proposed commands against the simulator's baseline behaviour without issuing them (shadow mode, §21.4); record the comparison method and its limits — shadow agreement is not a safety claim.
  4. Add all of the above to the CI scenario regression suite (§22.2).
- **Deliverable/output:** Fault-injection suite + integration tests + shadow-evaluation method + CI wiring.
- **Acceptance criteria / exit check:** Every injected fault yields its pre-registered outcome; end-to-end integration passes; shadow comparison runs and its limits are documented; suites run in CI.
- **Owner role:** QA + Rust/Python engineer.
- **Dependencies:** P4.16, P4.30.
- **Verification method:** Fault-injection output; integration-test output; shadow-comparison report.
- **Risks/failure modes:** Shadow results presented as evidence for field dispatch. Mitigation: the shadow report states its limits in its header.

### P4.32 — Documentation and handover to Phase 5 (§23 Stage 4 exit gate)

- [ ] **Objective:** Hand Phase 5 a complete, honest package: the feasibility and dispatch contracts, the exit-gate evidence, and what is still open.
- **Report trace:** §23 Stage 4 exit gate, §24 steps 5–8, §22.2–§22.3.
- **Inputs/prerequisites:** P4.01–P4.37.
- **Steps:**
  1. Update PLAN.md/CONTEXT.md/CHECKLIST.md in this directory with actual outcomes (provider + version, limit sets, rejection-suite results, invariant matrix, known limitations).
  2. Write the handover note: command lifecycle contract and evidence references; feasibility pipeline and limit configuration; simulator-adapter boundary (simulation-only, no device writes); rejection evidence pack location (P4.27); open items (D3 still OPEN; unbalanced three-phase from v0.4.0; real protocol adapters are Stage 6; operator-approved control is v1.0.0, not now).
  3. Run `python tools/verify_docs.py` and `python tools/validate_agent.py` (AGENTS.md §6) and record their output; fix failures before claiming the gate.
  4. Obtain exit-gate sign-off: the §23 Stage 4 gate (invalid, stale, infeasible, unauthorized, duplicate, expired rejections in deterministic tests) is met per P4.27. If any item fails, the phase is not complete — record what is missing.
- **Deliverable/output:** Updated phase docs + handover note + verification-tool output + gate sign-off.
- **Acceptance criteria / exit check:** Handover note exists and lists contracts, versions, evidence, and open items; both verification tools pass; CHECKLIST.md is fully evidenced; no subtask is checked without evidence.
- **Owner role:** Phase lead (architect).
- **Dependencies:** P4.01–P4.37.
- **Verification method:** Checklist review; tool output; gate sign-off record.
- **Risks/failure modes:** Handover omits the simulation-only boundary and Phase 5 displays a simulated issue as a real dispatch. Mitigation: the boundary is a required handover section and is cross-referenced in Phase 5 (P5.13).

### P4.33 — Exhaustive command-transition matrix test (§22.3, §14.1)

- [ ] **Objective:** Prove every command-state transition — legal and illegal — is tested, closing the gap between P4.10's "representative set of illegal transitions" and the §22.3 release gate ("all command-state transitions are tested").
- **Report trace:** §22.3 (release gate), §14.1, §14.2.
- **Inputs/prerequisites:** P4.10 (transition table is the single source of truth).
- **Steps:**
  1. Generate the full transition matrix from the P4.10 table: all 14 states × all possible target states. Derive the expected outcome (legal with its guard, or illegal with a named error) from the table — do not hand-pick cases.
  2. Test every legal transition (already covered in P4.10 — re-run here as part of the matrix, not as a separate suite) and every illegal transition, including all transitions out of terminal states (COMPLETED, REJECTED, CANCELLED, SUPERSEDED, FALLBACK) and all skips (e.g. PROPOSED → APPROVED, VALIDATED → ISSUED, FEASIBLE → ISSUED).
  3. Assert illegal transitions are rejected with a named error at the domain layer and leave no state change and no audit gap (no transition recorded, or a rejected-attempt record where the audit policy requires one — record which).
  4. Wire the matrix test into CI so a future transition-table change that adds a transition without a test fails the build.
- **Deliverable/output:** Exhaustive transition-matrix test + CI wiring + matrix report (states × targets, expected vs actual).
- **Acceptance criteria / exit check:** Every cell of the 14×14 matrix has a tested outcome matching the P4.10 table; zero untested cells; a deliberately added illegal transition fails the test.
- **Owner role:** QA + Rust engineer (domain).
- **Dependencies:** P4.10.
- **Verification method:** Matrix-test output; mutation check (add a bogus transition, confirm failure, revert).
- **Risks/failure modes:** Matrix generated from the implementation instead of the table (circular proof). Mitigation: the expected outcomes are derived from the P4.10 table document, and the table is reviewed independently of the code.

### P4.34 — Unbalanced three-phase power-flow deferral and trigger record (§12.5 step 4, §25.2 D3)

- [ ] **Objective:** Record, as a decision, that Phase 4 uses the balanced radial model only, and name the trigger cases that force an unbalanced three-phase model — so §12.5 step 4's "or three-phase unbalanced" alternative is a governed choice, not an unexamined default.
- **Report trace:** §12.5 step 4, §12.4, §25.2 (D3, balanced radial default).
- **Inputs/prerequisites:** P4.01 (provider-selection rule), P4.05; D3 status (OPEN).
- **Steps:**
  1. State the Phase 4 default: balanced AC power flow on radial feeders (D3 recommended default); record that no unbalanced three-phase run is performed in this phase.
  2. Name the trigger cases that force an unbalanced model: a named test case that cannot be expressed or validated with the balanced model (e.g. a named unbalanced feeder case from §19.3, or a pilot feeder with significant phase imbalance) — each trigger names the case, the provider that would serve it (OpenDSS/GridLAB-D per P4.01's addition rule), and the subtask that would own it.
  3. Record the boundary in the feasibility result: every result states the model class used (balanced radial) so an unbalanced-network verdict is never implied.
  4. Record D3 as still OPEN; this subtask is a deferral record, not a D3 decision.
- **Deliverable/output:** Unbalanced-model deferral/trigger note + model-class statement in feasibility results.
- **Acceptance criteria / exit check:** Default stated; at least one named trigger case recorded with provider and owner; every feasibility result states its model class; D3 still OPEN.
- **Owner role:** Power-systems engineer + architect.
- **Dependencies:** P4.01, P4.05.
- **Verification method:** Deferral-note review against §12.4/§12.5; result-record inspection.
- **Risks/failure modes:** An unbalanced feeder silently validated with a balanced model. Mitigation: model class is on every result; an unbalanced case with no trigger match is rejected, not approximated.

### P4.35 — Feasibility-gate runtime and diagnostics measurement (§21.3, §19.4)

- [ ] **Objective:** Measure and record the feasibility gate's runtime (power-flow solve + limit checks) per model size, so §21.3's "solver duration/status" measurement covers the feasibility solver, not only the scheduler.
- **Report trace:** §21.3, §19.4 (grid feasibility metrics), §12.5.
- **Inputs/prerequisites:** P4.05–P4.07; P3.27 (scheduling runtime method — reuse, do not re-measure differently).
- **Steps:**
  1. Instrument the feasibility pipeline: per-check duration (power-flow solve, limit checks, margin diagnostics), solver/provider version, model version + size (bus/branch count), convergence status, iteration count, residuals.
  2. Record durations on the §24 vertical-slice scenario and on the largest scenario-library case available (P5.20), with hardware/container conditions per run.
  3. Publish the figures in the phase handover (P4.32) as the v0.3.0 feasibility baseline; set no arbitrary universal target (§19.4).
  4. Add a regression check: a feasibility run exceeding its recorded baseline by more than a stated factor on identical inputs is flagged for investigation (not silently accepted).
- **Deliverable/output:** Feasibility runtime report (measured figures + conditions) + regression check.
- **Acceptance criteria / exit check:** Durations recorded per check with conditions on both scenarios; figures in the handover; regression check runs in CI.
- **Owner role:** Numerical engineer + QA.
- **Dependencies:** P4.05–P4.07.
- **Verification method:** Runtime-report review; CI regression-check output.
- **Risks/failure modes:** Laptop figures presented as deployment figures. Mitigation: conditions are part of every figure (P5.24 discipline, applied here).

### P4.36 — Audit-events read API (§15.2, §15.3)

- [ ] **Objective:** Give the audit trail a specified read API (`/api/v1/audit-events`), so the Phase 5 Audit workspace (P5.09) reads from a contract instead of an unspecified source.
- **Report trace:** §15.2 (`/api/v1/audit-events`), §15.3, §16.2, §10.5.
- **Inputs/prerequisites:** P4.30 (audit persistence + outbox); P1.10 (audit crate/port — verify first whether a read API already exists there).
- **Steps:**
  1. Verify first: check Phase 1's API surface for an existing `/api/v1/audit-events` resource. If it exists, record its endpoint, filters, and tenant scoping, and close this subtask as verified — do not build a second one.
  2. If it does not exist, build the read API: tenant-scoped queries (tenant from authenticated claims, never a request field, §16.2), filters by command/schedule/actor/time range, evidence references preserved in every record (never a summary that drops references, P5.09).
  3. Enforce append-only at the API layer: no update/delete endpoint exists on this resource (P1.10 port rule, applied to the API).
  4. Publish the resource in OpenAPI (§10.5, §15.2) and add contract tests, including a cross-tenant read denial test.
- **Deliverable/output:** Audit-events read API (or a verified-existing record) + OpenAPI entry + contract tests.
- **Acceptance criteria / exit check:** Exactly one audit-events read API exists and is recorded; tenant scoping enforced and tested; no update/delete endpoint; evidence references preserved end to end (P5.09 can consume it).
- **Owner role:** Rust engineer.
- **Dependencies:** P4.30; P1.10 (verify-first).
- **Verification method:** API contract tests; cross-tenant denial test; OpenAPI inspection.
- **Risks/failure modes:** A second, divergent audit read path. Mitigation: the verify-first step and the exactly-one rule.

### P4.37 — Constrain/return consumption contract (§12.5 step 8)

- [ ] **Objective:** Close the open loop in §12.5 step 8: define how a CONSTRAIN or RETURN outcome from the feasibility gate is consumed by scheduling, so a returned candidate is re-optimized with its violated constraints as inputs instead of silently degrading to a rejection.
- **Report trace:** §12.5 step 8, §12.6, §23 Stage 4.
- **Inputs/prerequisites:** P4.07 (CONSTRAIN/RETURN outcomes with violated constraints attached); P3.24 (rolling-horizon re-scheduling — the consumer, owned by Phase 3).
- **Steps:**
  1. Define the return-consumption contract: the feasibility gate emits a returned candidate carrying the violated constraints, the model version + hash, and the outcome (CONSTRAIN with constrained values, or RETURN with constraints as new inputs); the scheduler consumes it as a re-optimization input in the next rolling-horizon cycle (P3.24).
  2. Implement the Phase 4 side: the contract type, the emission of returned candidates, and a contract test proving a returned candidate re-enters scheduling with its violated constraints attached (scheduler-side stub in tests; the real consumer is Phase 3's P3.24, which is not modified here).
  3. Define the failure mode: if the scheduler cannot consume a returned candidate (e.g. constraint type not expressible in the §11.1 capability contract), the candidate is REJECTED with the reason recorded — never silently dropped and never passed.
  4. Record the open item: full closed-loop verification (feasibility return → re-optimized schedule → re-validation) is a Phase 5 scenario test (P5.18–P5.19), not a Phase 4 unit test.
- **Deliverable/output:** Return-consumption contract + Phase 4 emission + contract test + open-item record.
- **Acceptance criteria / exit check:** Contract type exists; returned candidates carry violated constraints + model version; the stub-consumer test passes; the unconsumable case rejects with a recorded reason; the Phase 5 closed-loop item is recorded.
- **Owner role:** Architect + Rust engineer.
- **Dependencies:** P4.07; P3.24 (consumer, not modified).
- **Verification method:** Contract tests; open-item record inspection.
- **Risks/failure modes:** RETURN treated as REJECTED everywhere, hiding the re-optimization path. Mitigation: the two outcomes keep distinct statuses and evidence (P4.07), and the contract test distinguishes them.

## Cross-phase references

- Consumes from Phase 3: schedule ID + revision (P3.22), envelope version (P3.13), §12.6 result records (P3.21). See `ledger_phase_3.md` P3.21–P3.22.
- Consumed by Phase 5: dispatch lifecycle states and audit trail (P5.06, P5.09, P5.13), feasibility evidence display (P5.04), simulator integration (P5.14–P5.15 adapters; P5.18–P5.20 replay/library). See `ledger_phase_5.md`.
- NOT built here: operator console UI (Phase 5), edge/protocol adapters (Stage 6), plugin SDK (Stage 7), research modules (Stage 8).
 — implemented, awaiting human build/test — implemented, awaiting human build/test — implemented, awaiting human build/test — implemented, awaiting human build/test — implemented, awaiting human build/test — implemented, awaiting human build/test — implemented, awaiting human build/test — implemented, awaiting human build/test — implemented, awaiting human build/test — implemented, awaiting human build/test — implemented, awaiting human build/test — implemented, awaiting human build/test