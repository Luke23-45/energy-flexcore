# W2.04 — Qualification checklist by resource

Workstream: FlexCore control-plane work

- **Objective:** Define the qualification checklist by resource from the design §5.5 table (physical identity, electrical mapping, controllability, capability, operating constraints, measurement, network/site limits, governance), with the rule that manual inspection identifies a candidate but cannot prove dispatchability.
- **Design trace:** Design §5.5 table; §1.1 flexible-load row
- **Inputs / prerequisites:** Design §5.5 table; W2.03 lifecycle; design §1.1 flexible-load and calibrated-model rows.
- **Steps:**
  1. Reproduce the eight checklist dimensions with their required evidence/decision per dimension: physical identity (equipment make/model, rated power/energy, equipment boundary, electrical connection, ownership/operator); electrical mapping (Core Site/PCC, phase, grid-model/bus reference, connection topology where required); controllability (local controller/BMS/BAS/EMS interface, command path, supported setpoint/mode, control permission and safe local fallback); capability (import/export limits, ramp, operating mode, power/energy window, response delay, duration, availability, recovery and rebound); operating constraints (comfort/process/service limits, maintenance windows, opt-out, safety and local override rules); measurement (appropriate meter point, cadence, clock quality, measurement quality and baseline evidence); network and site limits (PCC and transformer/import/export constraints; network feasibility remains a separate Core validation gate); governance (engineering reviewer, scope, expiry/review period, evidence snapshot and approval record).
  2. State the dispatchability rule: a manual inspection can identify a candidate; it cannot by itself prove dispatchability.
  3. State the thermal rule: a load with only a declared thermal time constant and no measured calibration is not promoted as a validated thermal asset; the existing P15.14 DeclaredThermalModel can record sourced parameters but does not fit or calibrate a physical model.
  4. Record the checklist as the evidence standard for a QUALIFIED verdict in W2.03.
  5. State the §1.1 flexible-load model reuse rule: the current model has baseline, reducible/increasable range, duration, recovery, rebound, availability and descriptors for comfort/process constraints; it is not a calibrated building thermal simulator.
- **Deliverable / output:** A qualification checklist note with the eight dimensions, the dispatchability rule and the thermal rule.
- **Acceptance criteria / exit check:**
  - [ ] All eight §5.5 checklist dimensions appear with their required evidence/decision.
  - [ ] The manual-inspection-cannot-prove-dispatchability rule is stated.
  - [ ] The declared-thermal-model rule (record sourced parameters; no fitting/calibration claim) is stated.
  - [ ] Network feasibility as a separate Core validation gate is stated.
  - [ ] The §1.1 flexible-load model reuse rule is stated (field inventory; not a calibrated building thermal simulator).
- **Owner role:** Power systems + flexibility reviewer
- **Dependencies:** W2.03
- **Verification method:** Checklist checked dimension by dimension against the design §5.5 table.
- **Risks / failure modes:** Qualifying a resource on inspection alone and treating it as dispatchable. Mitigation: the checklist is the evidence standard for QUALIFIED.
