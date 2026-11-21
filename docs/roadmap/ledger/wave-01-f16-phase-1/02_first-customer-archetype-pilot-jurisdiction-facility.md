# W1.02 — First customer archetype + pilot jurisdiction/facility

Workstream: FlexCore control-plane work

- **Objective:** Select the first customer archetype and the pilot jurisdiction and facility: one real pilot jurisdiction and one real facility/operator workflow for a multi-building or single-building commercial facility with an electricity account, meters and HVAC, and possibly PV/BESS/EV charging; no arbitrary manufacturing-process optimization.
- **Design trace:** Design §3.2; §16 gate G2
- **Owns:** Owns §16 gate G2 (F-16 first facility archetype: office, warehouse, hotel, campus or another class; the first ECM library and building survey depend on this choice).
- **Inputs / prerequisites:** Design §3.2; W0.06 (G1 jurisdiction); W1.01 scope statement.
- **Steps:**
  1. Select one archetype class (office, warehouse, hotel, campus or another named class) and record why it was chosen.
  2. Name one real pilot jurisdiction (consistent with W0.06) and one real facility and operator workflow; record the facility electricity account, meters, HVAC and any PV/BESS/EV charging present.
  3. Record explicitly that arbitrary manufacturing-process optimization is out of scope for the pilot, with the reason given in §3.2 (specific production, quality, thermal, safety and rebound constraints the general flexible-load model does not implement).
  4. Record the conditions under which process-specific templates could be added later (qualified engineering partner and measured data exist).
  5. Record the G1 revisit trigger: if the selected archetype genuinely constrains the jurisdiction (for example the archetype cannot be served in the W0.06 jurisdiction), record it as a G1 revisit trigger for W0.06, not as a hidden dependency — the jurisdiction decision is revisited, not silently changed here.
- **Deliverable / output:** A pilot definition sheet: archetype class, pilot jurisdiction, pilot facility and operator workflow, asset inventory present, the process-optimization exclusion, and the G1 revisit trigger.
- **Acceptance criteria / exit check:**
  - [ ] One archetype class is named; one real pilot jurisdiction and one real facility/operator workflow are named.
  - [ ] The facility inventory records the electricity account, meters, HVAC and any PV/BESS/EV charging present.
  - [ ] Arbitrary manufacturing-process optimization is recorded as out of scope with the §3.2 reason.
  - [ ] The later-addition condition (qualified engineering partner and measured data) is recorded.
  - [ ] The G1 revisit trigger is recorded: an archetype that constrains the jurisdiction is a revisit trigger for W0.06, not a silent change.
- **Owner role:** Product + power systems
- **Dependencies:** W0.06, W1.01
- **Verification method:** Pilot definition checked against design §3.2 and the G1 jurisdiction record; facility inventory confirmed with the pilot operator.
- **Risks / failure modes:** Starting with a process-heavy industrial site the general model cannot represent. Mitigation: the archetype is a named G2 gate decision, not a default.
