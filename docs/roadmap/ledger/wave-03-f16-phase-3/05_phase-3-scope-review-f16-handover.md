# W3.05 — Phase 3 scope review and F-16 handover

Workstream: FlexCore control-plane work

- **Objective:** Hold the F-16 Phase 3 scope review and hand over F-16, against the design §14 Phase 3 scope (efficiency savings M&V workflow in place, pilot integrations in place, catalogs expanded on real evidence). Design §14 defines NO exit gate for F-16 Phase 3, so this subtask is a scope review, not an exit-gate review: it records per-item verdicts and the F-16 handover, and must not present its verdicts as a §14 exit gate. Renamed and restated in the iteration-04 patch pass (2026-11-21); the same defect class as W5.10, fixed in iteration 03 for F-17 Phase 1.
- **Design trace:** Design §14 F-16 Phase 3
- **Inputs / prerequisites:** W3.01–W3.04 outputs; design §14 F-16 Phase 3; W2.08 Phase 2 exit.
- **Steps:**
  1. Check the Phase 3 scope item by item: the separately designed efficiency-savings verification workflow using licensed IPMVP material and practitioner review is in place; BMS/EMS/edge/interval integrations exist only for the actual pilot and security boundary; measure catalogs and sector support were expanded based on real evidence. Verify the efficiency-savings M&V results carry the §13.5 disclosure (W3.02).
  2. Verify the three-concepts naming discipline still holds across Phase 3 outputs (operational audit log, DR event M&V, efficiency savings M&V kept separate).
  3. List any item not met with its owner and follow-up, and record the F-16 handover: the complete F-16 record set, the open items, and the conditions for later F-16 work.
  4. Confirm F-16 remains post-v1 and outside the v1.0.0 release critical path.
- **Deliverable / output:** A Phase 3 scope-review and F-16 handover record with per-item verdicts, the naming-discipline check, the open-items list, and the handover. The record states explicitly that design §14 defines no F-16 Phase 3 exit gate and that this review is not one.
- **Acceptance criteria / exit check:**
  - [ ] Each Phase 3 scope item has a verdict with evidence cited from W3.01–W3.04.
  - [ ] The three-concepts naming discipline is verified across Phase 3 outputs.
  - [ ] Any unmet item has a named owner and follow-up.
  - [ ] The handover names the complete F-16 record set and open items, and confirms F-16 stays post-v1.
  - [ ] The record states that design §14 defines no F-16 Phase 3 exit gate and that this review is a scope review, not an exit gate.
- **Owner role:** Product + M&V practitioner
- **Dependencies:** W3.01, W3.02, W3.03, W3.04, W2.08
- **Verification method:** Scope checklist against design §14 F-16 Phase 3; naming-discipline check across Phase 3 outputs.
- **Risks / failure modes:** Handing over F-16 with a conflated M&V record or an unreviewed integration. Mitigation: both are explicit scope-review checks.
