# Ledger — Phase 3: Flexibility and Deterministic Schedule

- **Report:** `docs/plan/Neryva_FlexCore_Product_and_Architecture_Report_v1.1.md`
- **Stage:** §23 Stage 3 — Flexibility and deterministic schedule
- **Release target:** v0.2.0 (simulation only; see §2.1, §21.4)
- **Status:** Reconciled 2026-11-14: 31/31 subtasks implemented (code, tests, and decision records on disk); nothing installed, built, tested, or run — the Stage 3 exit gate is NOT held and awaits human build/test/review (see HANDOVER.md and the per-subtask decision records).
- **Non-negotiables (AGENTS.md):** no hallucinated facts; no safety/certification claims; no protection-grade claims; never conflate solver classes (HiGHS LP/QP/MIP, not SOCP; Clarabel convex conic, not MISOCP; AC power flow is not AC-OPF; IPOPT local, not global); simulation-first 0.x; verify before claiming done.
- **Spelling:** "license" (US), "acknowledgement" (per `.agent/WRITING.md`).
- **How to use this ledger:** work subtasks in ID order unless a dependency says otherwise. Check a box only when its acceptance criteria have evidence attached (test output, file path, review note). A checked box without evidence is a defect.

## Trace summary

| Report section | Covered by |
|---|---|
| §11.1 generic DER capability model | P3.01–P3.04 |
| §11.2 BESS model | P3.05–P3.07 |
| §11.3 inverter P-Q envelope | P3.08–P3.09 |
| §11.4 flexible load model | P3.10 |
| §11.5 EV fleet model | P3.11 |
| §11.6 portfolio flexibility aggregation | P3.12–P3.13 |
| §12.1 scheduling objective | P3.14–P3.15 |
| §12.2 market/PCC tracking | P3.16 |
| §12.3 optimization tiers | P3.17–P3.20 |
| §12.6 optimization result contract | P3.21–P3.22 |
| §24 vertical slice steps 3–4 | P3.23–P3.24 |
| §19.2–§19.4 testing/benchmarks (scheduling) | P3.25–P3.27 |
| §10 data contracts, §15 events, §16 storage | P3.28 |
| Security, docs, handover | P3.29–P3.30 |

## Subtasks

### P3.01 — Freeze the capability-model contract (§11.1)

- [x] **Objective:** Define the single capability-model interface every asset adapter implements, so Phase 3 work cannot drift into per-asset ad-hoc fields. — implemented, awaiting human build/test (decision record: `ledger/phase-03-flexibility-scheduling/01_04_capability_contract.md`)
- **Report trace:** §11.1, §9.1, §10.1.
- **Inputs/prerequisites:** Stage 2 asset registry and telemetry model (Phase 2 outputs); §11.1 constraint list.
- **Steps:**
  1. Write the capability contract as a versioned schema (Protobuf and/or Rust types per §8/§10.5): time-indexed feasible set `X(i,t)` over `(P, Q, E, u)`; per-constraint provenance flag measured / configured / inferred / unknown (§11.1).
  2. Require every constraint to carry units (MW, MVAr, MWh; §10.2), sign convention (`P > 0` export), and validity window.
  3. Record the contract in the phase PLAN.md data-contracts table and freeze it behind a version number; any later change needs a migration note (§10.5).
  4. Get power-systems review sign-off that the contract can express ramp, min up/down, duration/recovery, mode, temperature/health, and site-network constraints — or explicitly lists which it cannot.
- **Deliverable/output:** `flexcore-contracts` capability schema (versioned) + review note naming what is expressible and what is not.
- **Acceptance criteria / exit check:** (a) schema exists and is versioned; (b) all §11.1 constraint categories are either representable or listed as unsupported with a reason; (c) no asset adapter in this phase bypasses the contract.
- **Owner role:** Architect + power-systems engineer.
- **Dependencies:** Phase 2 registry/telemetry (consumed, not rebuilt); P3.01 gates P3.05, P3.10, P3.11.
- **Verification method:** Schema review against §11.1 checklist; `cargo test`/`pytest` schema-validation tests (added in P3.25).
- **Risks/failure modes:** Contract too narrow forces later breaking change; contract too vague lets adapters smuggle in undeclared constraints. Mitigation: explicit unsupported list, versioned schema.

### P3.02 — Capability provenance and unknown-state handling (§11.1)

- [x] **Objective:** Make "unknown" a first-class, visible state so an unmeasured constraint is never silently treated as unconstrained. — implemented, awaiting human build/test (decision record: `ledger/phase-03-flexibility-scheduling/01_04_capability_contract.md`)
- **Report trace:** §11.1, §10.4, §17.5 invariant 9.
- **Inputs/prerequisites:** P3.01.
- **Steps:**
  1. For each capability field, require a provenance value: measured, configured, inferred, or unknown.
  2. Define the rule: unknown capability narrows the feasible set to a documented conservative default (stated per asset class in P3.05–P3.11), never widens it.
  3. Propagate provenance into the flexibility envelope output (P3.12) and the schedule record, so a downstream reader can see which limits were inferred.
  4. Add a test: an asset with all-unknown ramp/temperature limits produces a strictly smaller envelope than the same asset with measured limits.
- **Deliverable/output:** Provenance rule documented in PLAN.md; provenance fields in the capability schema; one passing test per asset class.
- **Acceptance criteria / exit check:** Unknown fields are visible in envelope and schedule outputs; conservative-default rule is written down and tested; no silent widening in any test case.
- **Owner role:** Domain engineer.
- **Dependencies:** P3.01; feeds P3.12, P3.21.
- **Verification method:** Unit/property tests with unknown-provenance fixtures.
- **Risks/failure modes:** Conservative defaults over-restrict a real asset (acceptable in simulation; record as a limitation, not a fix-by-widening).

### P3.03 — Time model and horizon definition (§10.3, §11.1)

- [x] **Objective:** Fix the scheduling time grid so capability, forecast, envelope, and schedule all index the same intervals. — implemented, awaiting human build/test (decision record: `ledger/phase-03-flexibility-scheduling/01_04_capability_contract.md`)
- **Report trace:** §10.3, §11.1, §12.1.
- **Inputs/prerequisites:** P3.01; Stage 2 telemetry timestamps.
- **Steps:**
  1. Define horizon (24-hour default per §24 step 3), interval length (e.g. 15 min — record the choice as a configuration value, not a hard-coded constant), timezone handling (UTC internally, local time only at display), and interval semantics (start-inclusive, end-exclusive).
  2. Define how sub-interval telemetry is aggregated to the scheduling grid and how quality flags (§10.4) survive aggregation.
  3. Define horizon rollover: what happens to the previous schedule when a new horizon starts (supersede, never silently overwrite — cf. §14.2 SUPERSEDED).
  4. Document the time model in CONTEXT.md and in the schedule schema.
- **Deliverable/output:** Time-model section in the schedule contract; configuration for horizon/interval; rollover rule.
- **Acceptance criteria / exit check:** Two independently built fixtures on the same horizon produce identical interval boundaries; aggregation of GOOD/SUSPECT/BAD/MISSING/ESTIMATED/STALE inputs yields the documented output quality; rollover never deletes the prior schedule revision.
- **Owner role:** Domain engineer.
- **Dependencies:** P3.01; gates P3.12–P3.16.
- **Verification method:** Unit tests on boundary timestamps, DST-free UTC handling, and rollover.
- **Risks/failure modes:** Off-by-one interval errors; timezone confusion at display. Mitigation: UTC-only internally, property tests on interval arithmetic.

### P3.04 — Capability validation rules (schema, units, sign) (§11.1, §10.2)

- [x] **Objective:** Reject malformed capability data at the boundary before it can enter an envelope or a solve. — implemented, awaiting human build/test (decision record: `ledger/phase-03-flexibility-scheduling/01_04_capability_contract.md`)
- **Report trace:** §11.1, §10.2, §12.5 step 1 (by analogy), §19.2 layer 1.
- **Inputs/prerequisites:** P3.01–P3.03.
- **Steps:**
  1. Implement validators: units are MW/MVAr/MWh (reject kW/kWh at the contract boundary with a clear error, per §10.2); `P > 0` means export; SOC in [0,1]; `P_ch, P_dis >= 0`; `E_min <= E_max`; ramp >= 0.
  2. Return structured validation errors naming the field, the offending value, and the rule — never a bare "invalid input".
  3. Log validation rejections as audit-visible events (no secrets, §21.3).
  4. Cover with unit tests including sign-flip, unit-mix, SOC-out-of-range, and inverted-min/max cases.
- **Deliverable/output:** Capability validator module + error catalogue + tests.
- **Acceptance criteria / exit check:** Every listed malformed input is rejected with a named error; no malformed capability reaches the envelope calculator in tests; error messages contain no internal paths or secrets.
- **Owner role:** Rust engineer.
- **Dependencies:** P3.01–P3.03.
- **Verification method:** Unit tests; negative-case fixtures.
- **Risks/failure modes:** Over-strict validation rejects legitimate edge values (e.g. SOC exactly 0 or 1). Mitigation: boundary-inclusive tests.

### P3.05 — BESS capability model (§11.2)

- [x] **Objective:** Implement the §11.2 BESS model exactly as specified, including the discrete energy balance and the simultaneous charge/discharge rule. — implemented, awaiting human build/test (decision record: `ledger/phase-03-flexibility-scheduling/05_08_bess_pq_models.md`)
- **Report trace:** §11.2, §11.1.
- **Inputs/prerequisites:** P3.01–P3.04.
- **Steps:**
  1. Implement `P_net = P_dis − P_ch`; `E(t+1) = E(t) + η_ch·P_ch·Δt − P_dis/η_dis·Δt − P_aux·Δt`; `E_min ≤ E ≤ E_max`; `0 ≤ P_ch ≤ P_ch,max`; `0 ≤ P_dis ≤ P_dis,max`. The auxiliary-load term `−P_aux·Δt` comes from foundations §7.2, which the report §11.2 balance omits: include P_aux where auxiliary-load data exists; where it does not, record P_aux as unknown per P3.02 — never silently zero it without a note, because a silent zero understates energy drain.
  2. Implement the simultaneous charge/discharge rule: either a valid mixed-integer mode formulation or a justified convex relaxation plus post-solution checks (§11.2). Record which one is used and why. Do NOT label a binary-variable model as a continuous SOCP.
  3. Expose SOC-dependent power limits, reserve margins, temperature, degradation cost, and BMS constraints only where data is available; where unavailable, mark them unknown per P3.02 — do not invent values.
  4. Validate against a hand-computed 4-interval example (charge, discharge, idle, full) with exact expected SOC values.
- **Deliverable/output:** BESS capability module + hand-computed worked example + tests.
- **Acceptance criteria / exit check:** Energy balance reproduces the hand-computed example to stated tolerance; the auxiliary-load term is present where data exists or explicitly recorded unknown (never silently zero); simultaneous charge/discharge is either prevented by formulation or flagged by post-solution check in every test; no invented temperature/degradation values appear.
- **Owner role:** Power-systems engineer + Rust/Python engineer.
- **Dependencies:** P3.01–P3.04.
- **Verification method:** Unit tests; independent hand calculation; solver-output inspection for simultaneous charge/discharge.
- **Risks/failure modes:** Efficiency applied on the wrong side of the balance; relaxation silently allowing simultaneous charge/discharge. Mitigation: post-solution check is mandatory, not optional.

### P3.06 — BESS degradation and reserve treatment (§11.2, §12.1)

- [x] **Objective:** Define how degradation cost and reserve margins enter the model without inventing a degradation law. — implemented, awaiting human build/test (decision record: `ledger/phase-03-flexibility-scheduling/05_08_bess_pq_models.md`)
- **Report trace:** §11.2, §12.1.
- **Inputs/prerequisites:** P3.05.
- **Steps:**
  1. Survey what degradation/reserve data the Stage 2 registry actually holds; list gaps as unknowns.
  2. If no verified degradation model is available, implement degradation cost as a configurable linear throughput term with a default of zero and a documented warning that it is a placeholder, not a measured law.
  3. Implement reserve margin as a configurable SOC band (e.g. reserve floor) that narrows the usable energy window; record the chosen values as configuration, not as physics.
  4. Add tests proving reserve floor is never violated by a produced schedule.
- **Deliverable/output:** Degradation/reserve configuration + documented placeholder status + tests.
- **Acceptance criteria / exit check:** No degradation law is claimed without a source; reserve floor holds in all test schedules; placeholder status is visible in output metadata.
- **Owner role:** Power-systems engineer.
- **Dependencies:** P3.05.
- **Verification method:** Tests with reserve-floor fixtures; documentation review.
- **Risks/failure modes:** Placeholder mistaken for a validated model later. Mitigation: explicit "placeholder, not validated" metadata on every output that uses it.

### P3.07 — BESS edge cases: zero/near-zero SOC, full SOC, mode transitions (§11.2)

- [x] **Objective:** Prove the BESS model behaves at its boundaries instead of only in the comfortable middle. — implemented, awaiting human build/test (decision record: `ledger/phase-03-flexibility-scheduling/05_08_bess_pq_models.md`)
- **Report trace:** §11.2, §19.2 layers 1–2, §19.3 stressed scenarios.
- **Inputs/prerequisites:** P3.05.
- **Steps:**
  1. Build fixtures: SOC at exactly `E_min`, exactly `E_max`, one interval from full while discharging, one interval from empty while charging, and a charge→discharge transition.
  2. Assert: no SOC excursion outside `[E_min, E_max]`; no charge command when full (or it is clipped with a recorded warning, per the documented policy — pick one and document it); mode transitions respect ramp/min-up-down where configured.
  3. Record the clipping-vs-rejection policy decision in PLAN.md.
- **Deliverable/output:** Edge-case fixture set + policy note + tests.
- **Acceptance criteria / exit check:** All five fixtures pass; the clip/reject policy is written down and applied consistently; no fixture produces SOC outside bounds.
- **Owner role:** Power-systems engineer.
- **Dependencies:** P3.05.
- **Verification method:** Unit tests with exact expected SOC values.
- **Risks/failure modes:** Floating-point SOC drift across a 24 h horizon. Mitigation: tolerance stated in the contract; residual check in P3.21.

### P3.08 — Inverter P-Q envelope (§11.3)

- [x] **Objective:** Implement the configurable P-Q envelope, including the apparent-power limit and mode/voltage-dependent bounds. — implemented, awaiting human build/test (decision record: `ledger/phase-03-flexibility-scheduling/05_08_bess_pq_models.md`)
- **Report trace:** §11.3, §11.1.
- **Inputs/prerequisites:** P3.01–P3.04.
- **Steps:**
  1. Implement `P² + Q² ≤ (S_max)²` as the basic apparent-power constraint, documented as necessary but not complete (§11.3).
  2. Implement configurable `P_min(m,V) ≤ P ≤ P_max(m,V)` and `Q_min(P,V,m) ≤ Q ≤ Q_max(P,V,m)`; both charge and discharge represented.
  3. Do NOT use a power-factor inequality that assumes `P ≥ 0` for a bidirectional asset (§11.3). Add a test that fails if such an inequality is introduced.
  4. Record that IEEE 1547-2018 Clause 5 [R18] defines interconnection/interoperability requirements and is not a universal BMS or internal-protection specification; do not claim conformance from implementing the envelope.
- **Deliverable/output:** P-Q envelope module + per-mode limit tables (config) + tests.
- **Acceptance criteria / exit check:** Apparent-power limit holds in all test outputs; bidirectional (negative-P) cases pass; no `P ≥ 0` power-factor assumption exists in code or tests; IEEE 1547 scope is correctly stated in docs.
- **Owner role:** Power-systems engineer.
- **Dependencies:** P3.01–P3.04.
- **Verification method:** Unit tests incl. negative-P and Q-at-P-limits cases; code search for power-factor assumptions.
- **Risks/failure modes:** Envelope overstated by ignoring voltage dependence. Mitigation: voltage-dependent limits are configurable and default to the most restrictive documented value when voltage data is missing.

### P3.09 — Volt-var / volt-watt capability scoping (§11.3)

- [x] **Objective:** Decide explicitly, per device profile, which volt-var/volt-watt behaviours are modelled and which are out of scope for v0.2.0. — implemented, awaiting human build/test (decision record: `ledger/phase-03-flexibility-scheduling/09_11_load_ev_models.md`)
- **Report trace:** §11.3, §25.2 (D3 network class).
- **Inputs/prerequisites:** P3.08; D3 status (OPEN — balanced radial default per DECISIONS.md).
- **Steps:**
  1. For each inverter profile in the synthetic portfolio, record whether volt-var/volt-watt curves are configured, and if so their source (device datasheet, utility profile, or assumption — labelled as such).
  2. Where curves are absent, the scheduler must not assume a volt-var response; record the capability as unknown (P3.02).
  3. Document the scoping decision and its effect on the envelope in PLAN.md.
- **Deliverable/output:** Per-profile volt-var/volt-watt scoping table.
- **Acceptance criteria / exit check:** Every profile has a stated volt-var/volt-watt status; no schedule relies on an unconfigured curve; scoping table is reviewed by power-systems engineer.
- **Owner role:** Power-systems engineer.
- **Dependencies:** P3.08.
- **Verification method:** Review of scoping table against profiles; test that an unconfigured profile yields no volt-var contribution.
- **Risks/failure modes:** Assuming a default curve that the real inverter does not implement. Mitigation: unknown-by-default rule.

### P3.10 — Flexible load model (§11.4)

- [x] **Objective:** Implement the flexible-load model beyond a negative injection, with baseline, duration, recovery, and rebound. — implemented, awaiting human build/test (decision record: `ledger/phase-03-flexibility-scheduling/09_11_load_ev_models.md`)
- **Report trace:** §11.4, §11.1.
- **Inputs/prerequisites:** P3.01–P3.04.
- **Steps:**
  1. Implement fields: baseline demand, reducible/increasable range, response time, max event duration, min recovery time, rebound energy, comfort/process constraints, availability windows.
  2. Define the baseline rule for the synthetic portfolio explicitly (e.g. a named synthetic baseline method) and record that DR/market baseline and measurement rules are region/product-specific (§11.4) — the synthetic rule is not a market claim.
  3. Enforce duration/recovery/rebound in the envelope: an event that exceeds max duration, or starts before recovery completes, must be excluded or flagged.
  4. Test with a load that has a 2-hour max duration and 1-hour recovery across a 24-hour horizon.
- **Deliverable/output:** Flexible-load capability module + baseline-rule note + tests.
- **Acceptance criteria / exit check:** Duration, recovery, and rebound constraints hold in every test schedule; baseline method is named and labelled synthetic; no load is dispatched outside its availability window.
- **Owner role:** Domain engineer.
- **Dependencies:** P3.01–P3.04.
- **Verification method:** Unit tests; horizon-length scenario test.
- **Risks/failure modes:** Rebound energy ignored, overstating net flexibility. Mitigation: rebound is a required field (zero only if explicitly configured as zero with a reason).

### P3.11 — EV fleet model (§11.5)

- [x] **Objective:** Implement the EV fleet model with connection windows, departure energy, priority, and opt-out. — implemented, awaiting human build/test (decision record: `ledger/phase-03-flexibility-scheduling/09_11_load_ev_models.md`)
- **Report trace:** §11.5, §11.1.
- **Inputs/prerequisites:** P3.01–P3.04.
- **Steps:**
  1. Implement: EVSE rated power and phase; vehicle connection windows; required departure energy/SOC; min charging power and ramp; user override/opt-out; charging priority; site transformer limit.
  2. Bidirectional (V2X) capability is OFF by default and enabled only when the EV, EVSE, protocol, interconnection agreement, and market rules all support it (§11.5). Record the per-vehicle evidence for any enabled case; in the synthetic portfolio, keep it disabled unless a named test fixture enables it with all five conditions documented.
  3. Enforce departure-energy as a hard constraint in the schedule: a vehicle must reach its required SOC by departure in every feasible schedule.
  4. Test opt-out mid-horizon: an opted-out vehicle drops out of the envelope from the opt-out time, and the schedule is re-derived (see P3.24) rather than silently keeping its capacity.
- **Deliverable/output:** EV fleet capability module + V2X enablement rule + tests.
- **Acceptance criteria / exit check:** Departure SOC met in all feasible test schedules; opt-out removes capacity from the opt-out time onward; V2X is disabled by default and any enabled case documents all five §11.5 conditions; OCPP 2.1 [R19] is cited only for capability scope, never as a claim that a device supports it.
- **Owner role:** Domain engineer.
- **Dependencies:** P3.01–P3.04.
- **Verification method:** Unit/scenario tests; V2X default-off test.
- **Risks/failure modes:** Departure constraint softened by the solver to gain objective value. Mitigation: departure energy is a hard constraint; any relaxation is a named, reviewed decision (see P3.19).

### P3.12 — Portfolio flexibility envelope (§11.6)

- [x] **Objective:** Compute the portfolio flexibility envelope as a constrained aggregation, not a sum of nameplates. — implemented, awaiting human build/test (decision record: `ledger/phase-03-flexibility-scheduling/12_13_envelope.md`)
- **Report trace:** §11.6, §24 step 3.
- **Inputs/prerequisites:** P3.05–P3.11; synthetic forecasts from the Phase 3 fixture (P3.31) with labelled provenance. Phase 2 produces no forecasts — the earlier wording ("forecasts from Phase 2") was wrong; no Phase 2 subtask owns a ForecastBundle or forecast provider, and §23 Stage 2 deliverables do not include forecasts.
- **Steps:**
  1. Implement `P_portfolio(t) = Σ P_net(i,t) − P_loss/adjustment(t)` subject to each asset's time-coupled constraints and site/network coupling constraints (§11.6).
  2. Account for uncertainty and correlation: shared weather exposure, site limits, communication paths, common operating restrictions must narrow the envelope where they apply — document the correlation treatment used (even if it is a stated conservative derating, labelled as an assumption).
  3. Return both a central estimate and a confidence/availability descriptor where the model supports it (§11.6); where it does not, say so rather than inventing a confidence number.
  4. Emit the `FlexibilityEnvelopeUpdated` event (§15.3) with envelope version, horizon, and input snapshot references.
- **Deliverable/output:** Envelope calculator + event emission + correlation-treatment note + tests.
- **Acceptance criteria / exit check:** Envelope is strictly less than the nameplate sum on at least one documented test case where a shared constraint binds; central estimate + availability descriptor (or an explicit "not supported" note) present; event emitted with versioned inputs.
- **Owner role:** Power-systems engineer + Rust engineer.
- **Dependencies:** P3.05–P3.11.
- **Verification method:** Scenario tests with a binding shared site limit; comparison against naive nameplate sum.
- **Risks/failure modes:** Summing independent maxima overstates deliverable flexibility (§11.6). Mitigation: the binding-shared-constraint test is a gate, not a nice-to-have.

### P3.13 — Envelope staleness and recomputation triggers (§11.6, §10.4)

- [x] **Objective:** Define when an envelope is stale and must be recomputed, so a schedule never silently uses an outdated envelope. — implemented, awaiting human build/test (decision record: `ledger/phase-03-flexibility-scheduling/12_13_envelope.md`)
- **Report trace:** §11.6, §10.4, §15.3.
- **Inputs/prerequisites:** P3.12.
- **Steps:**
  1. Define staleness triggers: new telemetry with quality change, asset availability change, capability update, forecast update, horizon rollover, and a maximum age bound (configured, recorded).
  2. Attach an envelope version, computation timestamp, and input-snapshot reference to every envelope; a schedule must reference the envelope version it used (P3.21).
  3. Implement the trigger logic and a test where a `TelemetryQualityChanged` event invalidates the current envelope.
  4. Document the policy: a stale envelope is never used for a new schedule; the scheduler either recomputes or fails loudly.
- **Deliverable/output:** Staleness policy + trigger implementation + tests.
- **Acceptance criteria / exit check:** Each listed trigger invalidates the envelope in tests; a schedule referencing a superseded envelope version is rejected or re-derived; max-age bound is enforced.
- **Owner role:** Rust engineer.
- **Dependencies:** P3.12.
- **Verification method:** Event-driven tests; stale-envelope rejection test.
- **Risks/failure modes:** Envelope recomputed on every telemetry tick (performance). Mitigation: debouncing/threshold rules recorded as configuration, with the staleness guarantee preserved.

### P3.14 — Scheduling objective definition (§12.1)

- [x] **Objective:** Make the scheduling objective explicit, configurable, and region/product-specific — never a hard-coded universal objective. — implemented, awaiting human build/test (decision record: `ledger/phase-03-flexibility-scheduling/14_16_objective_pcc.md`)
- **Report trace:** §12.1, §12.2.
- **Inputs/prerequisites:** P3.12; D1/D2 status (both OPEN — no jurisdiction/market is decided).
- **Steps:**
  1. Implement the generic objective `min Σ_t (C_energy + C_degradation + C_deviation + C_curtailment + C_service)` (§12.1) with each term switchable/configurable and its units stated.
  2. Because D1/D2 are OPEN, the default objective for v0.2.0 is a named synthetic objective (e.g. minimise energy cost against a synthetic price curve plus deviation penalty), labelled synthetic and documented as not a market product.
  3. Record the objective choice, term weights, and sign conventions in the schedule record (P3.21) so two schedules with different objectives are never compared as if they were the same problem.
  4. Add a test proving a change in objective weights changes the schedule on a fixture designed to be weight-sensitive.
  5. Record the foundations §7.5 extra objective terms that are NOT in the v0.2.0 objective and why: flex discomfort, commitment deviation, reserve shortfall, and switching costs appear in foundations §7.5 as terms an objective "may minimize" — illustrative, not mandatory — while report §12.1 defines the objective as the five terms in step 1. The v0.2.0 objective is the §12.1 terms only; the §7.5 extras are not silently added, and any later adoption is a recorded decision, not a quiet extension.
- **Deliverable/output:** Objective configuration + synthetic default + tests.
- **Acceptance criteria / exit check:** Objective terms, weights, and signs are recorded per schedule; no objective is hard-coded as universal; weight-sensitivity test passes; the foundations §7.5 extra terms are recorded as not in the v0.2.0 objective, with the reason.
- **Owner role:** Power-systems engineer + product (for the synthetic-default label).
- **Dependencies:** P3.12.
- **Verification method:** Weight-sensitivity test; schedule-record inspection.
- **Risks/failure modes:** A synthetic price curve later mistaken for a market price. Mitigation: "synthetic" label in the record and in every display (see Phase 5, P5.12).

### P3.15 — Objective-term validation and cost-curve provenance (§12.1)

- [x] **Objective:** Prove each objective term is computed from traceable inputs, not from an untracked constant. — implemented, awaiting human build/test (decision record: `ledger/phase-03-flexibility-scheduling/14_16_objective_pcc.md`)
- **Report trace:** §12.1, §21.3 (correlation of records).
- **Inputs/prerequisites:** P3.14.
- **Steps:**
  1. For each term, record its input source (synthetic price curve version, degradation placeholder per P3.06, commitment per P3.16, curtailment rule).
  2. Validate term magnitudes: a term that dominates the objective by orders of magnitude triggers a warning and a recorded scaling review (solver scaling is a P3.19 concern).
  3. Test with a zero-price interval and a negative-price interval (if the synthetic curve supports it): the schedule must behave as the documented objective implies, and the behaviour must be explainable in the schedule diagnostics.
- **Deliverable/output:** Term-provenance table + scaling-warning rule + tests.
- **Acceptance criteria / exit check:** Every term traces to a named input version; dominance warning fires on the designed fixture; zero/negative-price behaviour is documented and matches the objective.
- **Owner role:** Domain engineer.
- **Dependencies:** P3.14.
- **Verification method:** Provenance inspection; fixture tests.
- **Risks/failure modes:** Silent scaling errors making one term irrelevant. Mitigation: dominance warning + diagnostics in P3.21.

### P3.16 — Market/PCC tracking (§12.2)

- [x] **Objective:** Track net power at the declared PCC/delivery point, with losses and sign conventions handled consistently. — implemented, awaiting human build/test (decision record: `ledger/phase-03-flexibility-scheduling/14_16_objective_pcc.md`)
- **Report trace:** §12.2, §10.2.
- **Inputs/prerequisites:** P3.14; site/PCC definitions from Phase 2.
- **Steps:**
  1. Implement the tracking term `min Σ_t λ_track·(P_PCC,t − P_commitment,t)² + …` (§12.2) against the declared PCC, not the sum of arbitrary bus injections.
  2. Include losses, sign conventions, and measurement points consistently; support multiple delivery points as separate tracking terms, never merged into one number.
  3. Because D1 is OPEN, commitments in v0.2.0 are synthetic test commitments, labelled as such; no market qualification is claimed (§0.3, §12.2).
  4. Test: a schedule that meets the asset sum but misses the PCC target (because of a modelled loss) scores worse than one that meets the PCC target.
- **Deliverable/output:** PCC tracking module + synthetic-commitment labelling + tests.
- **Acceptance criteria / exit check:** Tracking is computed at the declared PCC with losses included; multiple delivery points stay separate; PCC-miss test passes; no market claim appears in code or docs.
- **Owner role:** Power-systems engineer.
- **Dependencies:** P3.14.
- **Verification method:** Loss-aware fixture test; record inspection.
- **Risks/failure modes:** Tracking the wrong measurement point. Mitigation: PCC identity is a required, validated field in the schedule record.

### P3.17 — Optimizer port and Tier 1 (LP/QP/MILP) provider (§12.3, §9.1)

- [x] **Objective:** Build the `Optimizer` port (§9.1) and a Tier 1 provider behind it, so scheduling depends on a contract, not on HiGHS directly. — implemented, awaiting human build/test (decision record: `ledger/phase-03-flexibility-scheduling/17_optimizer_port.md`)
- **Report trace:** §12.3, §9.1, §5.2, §7.4 solver selection rule.
- **Inputs/prerequisites:** P3.14–P3.16; RESEARCH.md solver findings (HiGHS 1.15.1: LP/QP/MIP only).
- **Steps:**
  1. Define the `Optimizer` port: declared problem class, supported constraint types, scaling, tolerances, warm-start support, licensing, determinism, diagnostics (§7.4).
  2. Implement the Tier 1 provider using HiGHS (via highspy or the Rust binding — decide once and record the choice with a version pin; the pinned version in RESEARCH.md §1 is highspy 1.15.1, the Python binding, and no Rust HiGHS binding version was researched, so if the Rust binding is chosen its version must be researched and pinned first per RESEARCH.md §6 task 6) for LP/QP/MILP formulations only. State in code and docs: HiGHS is not an SOCP solver.
  3. Build the LP formulation first (linearised asset models); add QP (tracking term) and MILP (BESS mode binaries, EV/flexible-load discrete decisions) as separate, tested formulation paths with the problem class declared per run.
  4. Pin the solver version in a lockfile; record it in every optimization result (P3.21).
- **Deliverable/output:** `Optimizer` port + HiGHS Tier 1 provider + formulation modules (LP, QP, MILP) + pinned versions.
- **Acceptance criteria / exit check:** Scheduling code calls only the port, never HiGHS directly (verified by a dependency-direction check); each formulation declares its problem class; HiGHS is never invoked on an SOCP formulation; version is pinned and recorded; the binding choice (highspy vs Rust binding) and its version pin are recorded.
- **Owner role:** Rust/Python engineer (numerical worker).
- **Dependencies:** P3.14–P3.16.
- **Verification method:** Dependency check (crate/module graph); formulation-class tests; version-pinning check.
- **Risks/failure modes:** Leaky abstraction (HiGHS-specific options in domain code). Mitigation: port review; options pass through a provider-owned config struct.

### P3.18 — Tier 2 (continuous SOCP) provider via Clarabel (§12.3)

- [x] **Objective:** Provide the Tier 2 continuous-SOCP path for convex network approximations and inverter capability constraints, using Clarabel correctly. — implemented, awaiting human build/test (decision record: `ledger/phase-03-flexibility-scheduling/18_clarabel_tier2.md`)
- **Report trace:** §12.3, §7.4, §11.3.
- **Inputs/prerequisites:** P3.17; RESEARCH.md (Clarabel.rs 0.11.1: LP/QP/SOCP/SDP, exponential/power cones; no integer variables).
- **Steps:**
  1. Implement the Tier 2 provider behind the same `Optimizer` port, using Clarabel (Rust crate or Python, per the pinned version).
  2. Restrict Tier 2 to continuous conic problems. Any formulation needing integer variables must be rejected by the Tier 2 provider with `unsupported` status (§12.6) — never silently solved as if it were continuous. Clarabel is not MISOCP.
  3. Implement the inverter apparent-power cone `P² + Q² ≤ (S_max)²` as a second-order cone where the Tier 2 path is used, and cross-check its result against the Tier 1 linearised approximation on a shared fixture (P3.23).
  4. Document the SOCP relaxation exactness caveat (§12.5): exactness is conditional and must be checked, not assumed — the Tier 2 result is a candidate for Phase 4 validation, never a feasibility verdict by itself.
  5. Record the per-branch SOCP relaxation residual required by foundations §7.8: `g_ij = v_i·l_ij − (P_ij² + Q_ij²)` per cone/constraint, with a configured tolerance recorded in the provider configuration. The residual is diagnostic evidence that the relaxation held (or did not) on a given run; it does not replace the Phase 4 AC power-flow verification, and a large residual is reported, never hidden.
- **Deliverable/output:** Clarabel Tier 2 provider + integer-rejection test + cone formulation + exactness caveat note.
- **Acceptance criteria / exit check:** Tier 2 solves a continuous SOCP fixture; integer-containing formulation returns `unsupported`, not a wrong answer; cross-check vs Tier 1 is recorded; exactness caveat is in the provider docs; the per-branch relaxation residual and its tolerance are recorded per run.
- **Owner role:** Numerical engineer.
- **Dependencies:** P3.17.
- **Verification method:** Fixture solves; integer-rejection test; Tier 1 vs Tier 2 comparison on a shared case.
- **Risks/failure modes:** MISOCP attempted through Clarabel; SOCP relaxation assumed exact. Mitigation: hard rejection + documented caveat + Phase 4 independent power-flow check.

### P3.19 — Solver configuration, tolerances, determinism, and failure handling (§12.3, §12.6, §7.4)

- [x] **Objective:** Make solver behaviour reproducible and its failures diagnosable instead of silent. — implemented, awaiting human build/test (decision record: `ledger/phase-03-flexibility-scheduling/19_20_determinism_tiers.md`)
- **Report trace:** §12.6, §7.4, §19.2 layer 5.
- **Inputs/prerequisites:** P3.17–P3.18.
- **Steps:**
  1. Define per-provider configuration: time limit, MIP gap, feasibility/optimality tolerances, scaling approach, thread count (fixed for determinism in tests; record the value used).
  2. Map every solver outcome to the §12.6 status vocabulary: optimal / feasible / infeasible / timeout / numerical_failure / unsupported — no other status strings.
  3. Define the infeasible-schedule policy: return the infeasibility evidence (which constraints conflict, per the solver's diagnostics where available), never a silently relaxed schedule. Any relaxation (e.g. softening a departure constraint) requires a named, reviewed decision recorded in the schedule (see P3.11).
  4. Add deterministic-replay tests: same inputs + same pinned versions + same seed/config produce byte-identical (or tolerance-identical, with the tolerance stated) schedules.
- **Deliverable/output:** Solver config schema + status mapping + infeasibility policy + determinism tests.
- **Acceptance criteria / exit check:** All six §12.6 statuses are reachable in tests; infeasible runs return conflict evidence, not a relaxed schedule; replay test passes with stated tolerance.
- **Owner role:** Numerical engineer.
- **Dependencies:** P3.17–P3.18.
- **Verification method:** Status-coverage tests; replay tests with pinned versions.
- **Risks/failure modes:** Non-deterministic threading changes results between runs. Mitigation: fixed thread count in tests; thread count recorded in every result.

### P3.20 — Tier 3/4 boundary: what is NOT built in this phase (§12.3)

- [x] **Objective:** Draw a hard, documented line so MISOCP/MINLP and nonlinear AC-OPF are not attempted (or claimed) in Phase 3. — implemented, awaiting human build/test (decision record: `ledger/phase-03-flexibility-scheduling/19_20_determinism_tiers.md`)
- **Report trace:** §12.3, §13 (research boundary), §24 (build-first: no speculative AI).
- **Inputs/prerequisites:** P3.17–P3.19.
- **Steps:**
  1. Record in PLAN.md: Tier 3 (MISOCP/MINLP) only when discrete decisions materially require it and a suitable solver is available; Tier 4 (nonlinear AC-OPF) is advanced research/validated use cases only. Neither is in scope for v0.2.0.
  2. If a formulation needs Tier 3, the correct Phase 3 response is to reformulate as Tier 1 MILP or Tier 2 continuous SOCP, or to record the gap as an open item for Stage 8 — not to force it through an unsuitable solver.
  3. Add a guard: the scheduler rejects a Tier 3/4 formulation request with `unsupported` and a pointer to this decision, rather than attempting it.
  4. Note IPOPT's role honestly: local nonlinear solve only, no global-optimality guarantee (§7.4); IPOPT is not wired as a Phase 3 scheduler solver unless a specific Tier 4 research case justifies it, in which case it lands in Stage 8, not here.
- **Deliverable/output:** Tier-boundary decision note + `unsupported` guard + tests.
- **Acceptance criteria / exit check:** Tier 3/4 requests return `unsupported` with the decision reference; no Tier 3/4 code path exists in the Phase 3 scheduler; IPOPT is not presented as a global optimizer anywhere.
- **Owner role:** Architect.
- **Dependencies:** P3.17–P3.19.
- **Verification method:** Guard tests; code review for Tier 3/4 claims.
- **Risks/failure modes:** Scope creep into AC-OPF. Mitigation: the guard test fails the build if a Tier 3/4 path is added without updating this decision.

### P3.21 — Optimization result contract (§12.6)

- [x] **Objective:** Implement the §12.6 result contract in full so every schedule is auditable from its result record alone. — implemented, awaiting human build/test (decision record: `ledger/phase-03-flexibility-scheduling/21_result_contract.md`)
- **Report trace:** §12.6, §21.3, §15.3.
- **Inputs/prerequisites:** P3.17–P3.19.
- **Steps:**
  1. Implement the result record with all §12.6 fields: status; problem class and solver/provider version; objective value and units; primal and dual diagnostics where available; constraint residuals and violations; runtime and resource use; input snapshot and grid model version; uncertainty and assumptions; warnings; output candidate ID.
  2. Require the input snapshot to include: capability versions, envelope version (P3.13), forecast version, objective configuration (P3.14), solver configuration (P3.19), and telemetry snapshot reference. The "grid model version" in step 1 comes from the Phase 2 Site grid-model reference (P2.02) — the version of the network model the site references — not from a Phase 3 or Phase 4 model, which do not exist yet; record which site's reference supplied it.
  3. Emit `ScheduleProposed` (§15.3) carrying the result record reference; persist the record in PostgreSQL (§16.1) before any downstream use.
  4. Add a completeness test: a result missing any required field fails validation.
- **Deliverable/output:** Result-contract schema + persistence + event + completeness tests.
- **Acceptance criteria / exit check:** Every §12.6 field is present on every result in tests; a missing-field result is rejected; result references the exact input snapshot used; the grid model version is traceable to the Phase 2 Site grid-model reference (P2.02); residuals/violations are populated (or explicitly "not available" with a reason per solver).
- **Owner role:** Rust engineer.
- **Dependencies:** P3.17–P3.19, P3.13.
- **Verification method:** Completeness tests; snapshot-reference checks; review of a sample result record end-to-end.
- **Risks/failure modes:** Diagnostics dropped for one solver path. Mitigation: completeness test runs per provider, not just per happy path.

### P3.22 — Schedule revision, supersede, and candidate identity (§12.6, §14.2)

- [x] **Objective:** Give every schedule a stable identity and revision history so a newer schedule supersedes rather than erases its predecessor. — implemented, awaiting human build/test (decision record: `ledger/phase-03-flexibility-scheduling/22_24_revisions_comparison_rolling.md`)
- **Report trace:** §12.6 (candidate ID), §14.2 (SUPERSEDED), §15.3.
- **Inputs/prerequisites:** P3.21.
- **Steps:**
  1. Assign each schedule a unique ID and a monotonically increasing revision per portfolio/horizon; a re-solve creates a new revision, never mutates the old one.
  2. When a new revision is produced, mark the prior revision SUPERSEDED (retained for audit, §14.2) with a supersedes/superseded-by reference.
  3. Ensure dispatch commands (Phase 4) reference schedule ID + revision; a command referencing a superseded revision is rejected in Phase 4 (cross-reference P4.12).
  4. Test the full chain: solve → revise (new input) → old revision marked SUPERSEDED → old revision still retrievable.
- **Deliverable/output:** Schedule identity/revision implementation + tests.
- **Acceptance criteria / exit check:** Revisions are immutable and monotonic; supersede links are complete in both directions; superseded schedules remain queryable; no in-place mutation in tests.
- **Owner role:** Rust engineer.
- **Dependencies:** P3.21.
- **Verification method:** Revision-chain tests; audit-query checks.
- **Risks/failure modes:** Mutable schedule records make replay impossible. Mitigation: immutability enforced at the storage layer (append-only revisions).

### P3.23 — Schedule comparison workflow (§23 Stage 3)

- [x] **Objective:** Build the schedule-comparison workflow so alternatives (LP vs MILP, Tier 1 vs Tier 2, different objectives) can be compared on the same inputs. — implemented, awaiting human build/test (decision record: `ledger/phase-03-flexibility-scheduling/22_24_revisions_comparison_rolling.md`)
- **Report trace:** §23 Stage 3, §20.1 item 6 (Schedules workspace — display contract only; UI itself is Phase 5), §19.4.
- **Inputs/prerequisites:** P3.17–P3.22.
- **Steps:**
  1. Define the comparison record: shared input snapshot ID, candidate schedule IDs, and per-candidate metrics (§19.4 scheduling: objective value, schedule deviation, feasible interval count, runtime).
  2. Implement comparison over a fixed input snapshot: same envelope, same forecasts, same objective config — only the formulation/solver tier varies. A comparison across different inputs must be rejected or labelled incomparable.
  3. Include the Tier 1 vs Tier 2 cross-check from P3.18 as the first comparison case.
  4. Expose the comparison via API (REST, §15.2 `/api/v1/schedules` and `/api/v1/optimization-runs`) for the Phase 5 console; do not build the console UI here.
- **Deliverable/output:** Comparison API + record schema + first Tier 1 vs Tier 2 comparison case + tests.
- **Acceptance criteria / exit check:** Comparisons on shared inputs produce all four §19.4 scheduling metrics per candidate; cross-input comparison is rejected/labelled; API returns the comparison record; no UI code is added in this phase.
- **Owner role:** Rust engineer + numerical engineer.
- **Dependencies:** P3.17–P3.22.
- **Verification method:** Comparison tests with shared vs differing inputs; API contract tests.
- **Risks/failure modes:** Comparing schedules built on different inputs as if equivalent. Mitigation: input-snapshot equality is enforced, not assumed.

### P3.24 — Rolling-horizon re-scheduling (§12.1, §24 steps 3–4)

- [x] **Objective:** Implement rolling-horizon scheduling: re-solve on new information, carrying forward the realised state, without rewriting history. — implemented, awaiting human build/test (decision record: `ledger/phase-03-flexibility-scheduling/22_24_revisions_comparison_rolling.md`)
- **Report trace:** §12.1, §23 Stage 3, §24 steps 3–4, §10.3.
- **Inputs/prerequisites:** P3.13, P3.21–P3.23.
- **Steps:**
  1. Define the roll trigger set: horizon advance, envelope invalidation (P3.13), asset opt-out/dropout (P3.11), forecast update, operator request. Record the trigger in each new schedule revision.
  2. Carry forward realised state (actual SOC, actual load response) as the initial condition of the new solve; never re-solve the past as if it had not happened.
  3. Handle the overlap: intervals already executed keep their realised values; only future intervals are re-optimized. Test with a mid-horizon asset dropout.
  4. Emit `ScheduleProposed` for each new revision with its trigger and supersede link (P3.22).
- **Deliverable/output:** Rolling-horizon scheduler + trigger set + overlap handling + tests.
- **Acceptance criteria / exit check:** Re-solve after a mid-horizon dropout produces a feasible schedule that respects realised SOC; past intervals are unchanged; trigger is recorded per revision; supersede chain is intact.
- **Owner role:** Numerical engineer.
- **Dependencies:** P3.13, P3.21–P3.23.
- **Verification method:** Mid-horizon dropout scenario test; realised-state carry-forward test.
- **Risks/failure modes:** Re-solving the past creates a schedule that contradicts measured reality. Mitigation: realised intervals are frozen inputs, enforced by test.

### P3.25 — Unit and property tests for models and solver I/O (§19.2 layers 1–2)

- [x] **Objective:** Cover the Phase 3 domain with unit and property tests that would catch sign, unit, and invariant errors. — implemented, awaiting human build/test (decision record: `ledger/phase-03-flexibility-scheduling/25_26_tests_scenarios.md`)
- **Report trace:** §19.2 layers 1–2, §10.2, §11.
- **Inputs/prerequisites:** P3.01–P3.24 (tests are written alongside each subtask; this subtask closes coverage gaps).
- **Steps:**
  1. Unit tests: units, sign conventions, asset constraints, SOC bounds, ramp limits, P-Q envelope, departure SOC, duration/recovery.
  2. Property tests: invariant preservation across generated asset states, timestamps, and constraint sets (e.g. SOC never leaves bounds; envelope never exceeds the sum of per-asset maxima after coupling; P-Q points stay inside `S_max`).
  3. Solver I/O tests: malformed solver input rejected; solver output validated against the §12.6 contract before use (P3.21).
  4. Measure coverage of the capability/scheduling modules and record the figure; any untested invariant gets a named gap entry, not silence.
- **Deliverable/output:** Test suites + coverage report + gap list.
- **Acceptance criteria / exit check:** All listed unit/property areas have tests; property tests run with a stated number of generated cases; coverage figure is recorded; gaps are listed explicitly.
- **Owner role:** Rust/Python engineer + QA.
- **Dependencies:** P3.01–P3.24.
- **Verification method:** `cargo test` / `pytest` output; coverage report.
- **Risks/failure modes:** Property tests that pass vacuously (generator never hits the boundary). Mitigation: generators are reviewed to include boundary values.

### P3.26 — Deterministic scenario tests and synthetic portfolio (§19.2 layer 5, §19.3)

- [x] **Objective:** Prove the §23 Stage 3 exit gate on defined synthetic cases with pinned inputs, versions, and seeds. — implemented, awaiting human build/test (decision record: `ledger/phase-03-flexibility-scheduling/25_26_tests_scenarios.md`)
- **Report trace:** §19.2 layer 5, §19.3, §23 Stage 3 exit gate, §24 steps 3–4.
- **Inputs/prerequisites:** P3.17–P3.24.
- **Steps:**
  1. Build the synthetic portfolio (§24 step 1, as consumed from Phase 2): BESS, PV, flexible load, EVSE, with named capacities and constraints — record the full fixture so it is reproducible.
  2. Define at least three deterministic scenarios: (a) normal day (all assets available); (b) PV ramp / forecast error; (c) asset dropout mid-horizon. Each pins inputs, solver versions, seeds, and expected outcomes/tolerances.
  3. For each scenario, assert: schedules pass asset/time-coupled constraints; SOC/ramp/departure violations are zero within stated tolerance; objective and runtime are recorded.
  4. Store scenarios under `scenarios/synthetic-portfolio/` (per §8.1) with a README stating they are synthetic, not field data.
- **Deliverable/output:** Scenario suite + expected-outcome tables + README.
- **Acceptance criteria / exit check:** All three scenarios pass with pinned versions/seeds; expected outcomes and tolerances are written before the run (not fitted after); the §23 Stage 3 exit gate ("schedules pass asset/time-coupled constraints on defined synthetic cases") is demonstrably met.
- **Owner role:** Numerical engineer + QA.
- **Dependencies:** P3.17–P3.24.
- **Verification method:** Scenario-run output with pinned versions; pre-registered expected outcomes.
- **Risks/failure modes:** Expected outcomes fitted to the observed output. Mitigation: expected outcomes are written before execution and reviewed separately.

### P3.27 — Benchmark and performance measurement (§19.3, §19.4)

- [x] **Objective:** Measure scheduling runtime and scaling honestly, without setting arbitrary universal targets. — implemented, awaiting human build/test (decision record: `ledger/phase-03-flexibility-scheduling/27_benchmark_report.md`)
- **Report trace:** §19.3, §19.4, §22.2 (scenario regression suite).
- **Inputs/prerequisites:** P3.26.
- **Steps:**
  1. Record runtime, objective value, feasible interval count, and schedule deviation for each scenario in P3.26, with hardware/container description.
  2. Run a scaling probe: portfolio size ×2 and ×4 (synthetic copies) and record runtime growth; do not claim a scaling law from two points — report the raw numbers.
  3. Set no universal runtime target (§19.4: do not set arbitrary targets); instead record the measured figures as the v0.2.0 baseline for later comparison.
  4. Add the scenario suite to CI as a regression suite (§22.2).
- **Deliverable/output:** Benchmark report (measured figures only) + CI regression wiring.
- **Acceptance criteria / exit check:** Measured figures exist for all P3.26 scenarios and both scaling probes; hardware is recorded; no unmeasured performance claim appears; CI runs the suite.
- **Owner role:** Numerical engineer.
- **Dependencies:** P3.26.
- **Verification method:** Benchmark output; CI run log.
- **Risks/failure modes:** Benchmark run on a developer laptop presented as a deployment figure. Mitigation: hardware/container is part of the record.

### P3.28 — API, events, and persistence wiring (§15, §16)

- [x] **Objective:** Wire Phase 3 outputs into the platform contracts (REST, events, PostgreSQL) so Phase 4 and Phase 5 consume them through interfaces, not internals. — implemented, awaiting human build/test (decision record: `ledger/phase-03-flexibility-scheduling/28_api_wiring.md`)
- **Report trace:** §15.1–§15.3, §15.5, §16.1–§16.2, §10.5.
- **Inputs/prerequisites:** P3.12, P3.21–P3.23.
- **Steps:**
  1. Expose `GET/POST /api/v1/flexibility/envelopes`, `/api/v1/schedules`, `/api/v1/optimization-runs` (§15.2) with OpenAPI (§7.3 Utoipa) and versioned base path `/api/v1`.
  2. Emit `FlexibilityEnvelopeUpdated`, `ScheduleProposed`, `ScheduleValidated` (§15.3) via the transactional outbox (§15.5) so state and event publish atomically. "Validated" in Phase 3 means asset/time-coupled constraint validation (P3.26) — explicitly NOT network feasibility, which is Phase 4 (§12.5); the event payload and docs must say so, so no Phase 4 reader mistakes a Phase 3 `ScheduleValidated` for a feasibility verdict.
  3. Persist envelopes, schedules, and optimization runs in PostgreSQL with `tenant_id` on every tenant-owned record (§16.2); verify RLS behaviour per §16.2 (application role must not own tenant tables or bypass RLS).
  4. Add contract tests: API responses validate against OpenAPI; events validate against the event schema; cross-tenant access is denied in tests.
- **Deliverable/output:** API endpoints + OpenAPI + outbox emission + persistence + contract tests.
- **Acceptance criteria / exit check:** All three resources are reachable and schema-valid; events are emitted atomically with state (outbox test with a forced publish failure); cross-tenant read is denied; no endpoint exposes an "execute arbitrary command" operation (§15.2); `ScheduleValidated` is documented and tested as asset/time-coupled validation only, not network feasibility.
- **Owner role:** Rust engineer.
- **Dependencies:** P3.12, P3.21–P3.23.
- **Verification method:** API contract tests; outbox failure test; tenant-isolation tests.
- **Risks/failure modes:** Event published without the state commit (or vice versa). Mitigation: outbox pattern is tested with failure injection.

### P3.29 — Security review for Phase 3 (§17)

- [x] **Objective:** Close the Phase 3 security surface: tenant isolation, authorization on schedule actions, and no secrets in solver/config paths. — implemented, awaiting human build/test (decision record: `ledger/phase-03-flexibility-scheduling/29_security_review.md`)
- **Report trace:** §17.2–§17.4, §16.2, §21.3.
- **Inputs/prerequisites:** P3.28.
- **Steps:**
  1. Verify: schedule/envelope reads and writes are tenant-scoped (§17.5 invariant 8); solver workers receive only the tenant-scoped inputs they need; no cross-tenant data in solver logs.
  2. Verify: creating or re-solving a schedule requires an authorized role; the actor identity is recorded in the schedule record (§21.3).
  3. Scan: no secrets in solver configuration, scenario fixtures, or logs; dependency scan on HiGHS/Clarabel/CVXPY pins (license check feeds D4).
  4. Record the review outcome and any findings in the phase handover note; findings block the exit gate until fixed or explicitly accepted with a reason.
- **Deliverable/output:** Phase 3 security-review note + scan outputs.
- **Acceptance criteria / exit check:** Tenant-isolation tests pass; authorization is enforced on schedule mutations; scans run with no unaddressed high-severity finding; review note is filed.
- **Owner role:** Security engineer.
- **Dependencies:** P3.28.
- **Verification method:** Tenant-isolation and auth tests; dependency/secret scans.
- **Risks/failure modes:** Solver worker logs leaking portfolio data. Mitigation: log review is part of the checklist, not assumed clean.

### P3.30 — Documentation and handover to Phase 4 (§23 Stage 3 exit gate)

- [x] **Objective:** Hand Phase 4 a complete, honest package: what was built, what the exit gate evidence is, and what is still open. — implemented, awaiting human build/test (decision record: `ledger/phase-03-flexibility-scheduling/30_handover_phase4.md`)
- **Report trace:** §23 Stage 3 exit gate, §24 steps 3–4, §22.2.
- **Inputs/prerequisites:** P3.01–P3.29, P3.31.
- **Steps:**
  1. Update PLAN.md/CONTEXT.md/CHECKLIST.md in this directory with actual outcomes (versions pinned, scenarios run, gaps found).
  2. Write the handover note: envelope and schedule contracts (with versions), solver versions, the synthetic forecast fixture (P3.31 — version, horizon, provenance label), scenario results, known limitations (placeholder degradation, synthetic objective/commitments, SOCP exactness caveat, Tier 3/4 out of scope), and the exact inputs Phase 4 consumes (schedule ID + revision, envelope version, §12.6 result records).
  3. Run `python tools/verify_docs.py` and `python tools/validate_agent.py` (AGENTS.md §6) and record their output; fix failures before claiming the gate.
  4. Obtain exit-gate sign-off: schedules pass asset/time-coupled constraints on the defined synthetic cases (P3.26 evidence attached). If any item fails, the phase is not complete — record what is missing.
- **Deliverable/output:** Updated phase docs + handover note + verification-tool output + gate sign-off.
- **Acceptance criteria / exit check:** Handover note exists and lists contracts, versions, evidence, and open items; both verification tools pass; CHECKLIST.md is fully evidenced; no subtask is checked without evidence.
- **Owner role:** Phase lead (architect).
- **Dependencies:** P3.01–P3.29, P3.31.
- **Verification method:** Checklist review; tool output; gate sign-off record.
- **Risks/failure modes:** Handover omits a limitation that Phase 4 then treats as a guarantee. Mitigation: the limitations list is a required section of the handover note.

### P3.31 — Synthetic forecast fixture (owned by Phase 3; no Phase 2 forecast exists)

- [x] **Objective:** Build the synthetic forecast fixture that Phase 3's envelope and scheduling subtasks consume, with labelled provenance — because no Phase 2 subtask produces forecasts and §23 Stage 2 deliverables do not include them. The fixture is the single forecast source for v0.2.0; nothing in Phase 3 may invent a second, untracked forecast. — implemented, awaiting human build/test (decision record: `ledger/phase-03-flexibility-scheduling/31_forecast_fixture.md`)
- **Report trace:** §11.6, §12.1, §15.3 (`ForecastPublished`); §23 Stage 2 (forecasts not a Stage 2 deliverable); P2.28 (forecast non-delivery).
- **Inputs/prerequisites:** P3.01–P3.03 (capability contract, time model); P2.13 (scenario model — the fixture reuses the exit-gate scenario's horizon, cadence, and seeds where applicable).
- **Steps:**
  1. Define the fixture schema: per-asset per-interval forecast values (PV output, load, availability) with a forecast version, horizon, interval grid, and a mandatory `provenance: synthetic` label plus the generating scenario version + seed.
  2. Generate the fixture deterministically from the P2.13 scenario (same seed → byte-identical fixture); record the generation method so a second person reproduces it exactly.
  3. Define the forecast-error variants the P3.26 scenarios need (notably the PV ramp / forecast-error scenario): at least a central forecast and one error variant with a stated error magnitude, both versioned separately.
  4. Emit `ForecastPublished` (§15.3) for the fixture version, carrying the version + provenance label; the envelope (P3.12) and result contract (P3.21) reference the forecast version, never bare values.
  5. Add tests: fixture regenerates byte-identically from the seed; every forecast value carries the synthetic label; an unlabelled forecast is rejected by the envelope input validation.
- **Deliverable/output:** Synthetic forecast fixture (versioned) + generation method + `ForecastPublished` emission + tests.
- **Acceptance criteria / exit check:** Fixture is deterministic from its seed; every value is labelled synthetic with a version; forecast version is referenced by the envelope and result records; no Phase 3 code path consumes an unlabelled or unversioned forecast; the fixture is listed in the P3.30 handover note.
- **Owner role:** Power-systems engineer + Rust engineer.
- **Dependencies:** P3.01–P3.03, P2.13.
- **Verification method:** Determinism re-run; label/version inspection on every forecast input path; handover-note check.
- **Risks/failure modes:** A synthetic forecast later mistaken for a measured forecast or a market price signal. Mitigation: the synthetic label is in the schema, the event, the envelope input, and the result record — four places, not one.

## Cross-phase references

- Consumes from Phase 2: asset registry, telemetry model and quality flags. Forecast inputs are NOT consumed from Phase 2 (no Phase 2 subtask produces them — see P2.28); the synthetic forecast fixture is P3.31, owned by Phase 3.
- Consumed by Phase 4: schedule ID + revision (P3.22), envelope version (P3.13), §12.6 result records (P3.21), comparison records (P3.23). See `ledger_phase_4.md` P4.01–P4.04.
- Displayed by Phase 5: schedules workspace and comparison views consume the P3.28 API; see `ledger_phase_5.md` P5.10–P5.12.
