# W5.03 — Scenario comparison: grid-only, PV at candidate sizes, PV+battery, battery-only where appropriate

Workstream: companion-repo work (separate identity/database — NOT FlexCore control-plane work)

- **Objective:** Define F-17 scenario comparison from design §10.2–§10.4: grid-only, PV at candidate sizes, PV+battery, battery-only where appropriate; production is not bill savings; REopt as sizing/dispatch reference; never call a time-limited incumbent optimal.
- **Design trace:** Design §14 F-17 Phase 1; §10.2–§10.4
- **Inputs / prerequisites:** Design §10.2, §10.3, §10.4; W5.02 model; W4.09 provider seams; W4.10 REopt interim position.
- **Steps:**
  1. Define the scenario set: grid only, PV at several candidate sizes, PV plus battery with multiple capacities/power ratings, and optionally battery-only where appropriate.
  2. Apply §10.2: separate PV production, energy balance, and bill calculation; do not calculate savings as annual PV kWh times retail price unless that simplification is demonstrably valid for the tariff and is labelled as such; under net-billing or low export compensation, exported and self-consumed kWh have different economic value, and battery value depends on interval load, tariff and operating constraints.
  3. Apply §10.4: use measured interval load, provider-produced PV intervals and a valid tariff to form a constrained scenario; run grid-only, PV-only, PV+battery and candidate-size alternatives; record objective, constraints, engine/solver version, termination, infeasibility/optimality status, runtime and all assumptions; expose whether the result is optimal, feasible with a gap, timed out, infeasible or unsupported; do not call a time-limited incumbent optimal.
  4. Record that the existing FlexCore Optimizer port is reused only if the chosen home-planning formulation can be faithfully expressed in its problem IR and the selected provider declares support for all needed features, and that a nonlinear, mixed-integer or time-series problem must not be routed into a solver that does not support it.
  5. Record that the final G7 decision (REopt API vs local REopt.jl) is DEFERRED out of this subtask to W5.12: it is recorded only after the model is understood and user demand is validated — the W4.06 demand validation is the deferred decision's trigger — per design §10.4 step 6. This subtask consumes the W4.10 interim position only (the Phase 0 prototype uses the REopt API v3 proof-of-concept adapter); it must not record a final G7 decision.
  6. Apply the Tier B restriction from W4.07 (ADR-F17-04): scenario outputs built from monthly-only inputs must not claim measured hourly self-consumption, a precise battery size, peak-shaving savings, or an optimized dispatch result as though interval data existed.
- **Deliverable / output:** A scenario-comparison note with the scenario set, the production-vs-savings separation, and the REopt/optimizer rules.
- **Acceptance criteria / exit check:**
  - [ ] The scenario set (grid-only, PV at candidate sizes, PV+battery, battery-only where appropriate) is defined.
  - [ ] The §10.2 three-computation separation is stated, with the no-kWh-times-retail-price rule.
  - [ ] The §10.4 sequence is stated, including the never-call-a-time-limited-incumbent-optimal rule.
  - [ ] The FlexCore Optimizer reuse condition and the no-unsupported-solver-routing rule are stated.
  - [ ] The final G7 decision is recorded as deferred to W5.12 (not decided here); the W4.10 interim position is consumed, and the W4.06 demand validation is named as the deferred decision's trigger.
  - [ ] The Tier B restriction is stated: no monthly-only scenario output claims measured hourly self-consumption, a precise battery size, peak-shaving savings, or an optimized dispatch result.
- **Owner role:** Numerical engineering
- **Dependencies:** W5.02, W4.06, W4.07, W4.09, W4.10
- **Verification method:** Scenario rules checked against design §10.2–§10.4; solver-class discipline checked (HiGHS LP/QP/MIP, Clarabel conic, IPOPT local).
- **Risks / failure modes:** Calling a time-limited incumbent optimal, or calculating savings as PV kWh times retail price under net-billing. Mitigation: both are forbidden in the note.
