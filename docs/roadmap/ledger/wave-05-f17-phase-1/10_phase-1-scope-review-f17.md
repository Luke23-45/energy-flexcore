# W5.10 — Phase 1 scope review

Workstream: companion-repo work (separate identity/database — NOT FlexCore control-plane work)

- **Objective:** Hold the F-17 Phase 1 scope review against the design §14 Phase 1 scope (consumer authentication and separate data store; property, bill/interval import, tariff selection and manual/optional roof model; PV-only and PV+battery scenario comparison; immutable calculation report; data export, deletion and consent controls). Design §14 defines NO exit gate for F-17 Phase 1, so this subtask is a scope review, not an exit-gate review: it records per-item verdicts and the conditions for Wave 6 to start, and must not present its verdicts as a §14 exit gate. Renamed and restated in the adversarial-audit patch pass (2026-11-21).
- **Design trace:** Design §14 F-17 Phase 1 (scope list; no exit gate defined)
- **Inputs / prerequisites:** W5.01–W5.09, W5.11 and W5.12 outputs; design §14 F-17 Phase 1; W4.13 Phase 0 exit.
- **Steps:**
  1. Check the Phase 1 scope item by item against W5.01–W5.09, W5.11 and W5.12 evidence: consumer authentication and separate data store defined; property, bill/interval import, tariff selection and manual/optional roof model defined; PV-only and PV+battery scenario comparison defined; immutable calculation report with model/source versions, caveats and financial sensitivities defined; data export, deletion and consent controls defined; §8.2 nine-step journey traced (W5.11); final G7 decision recorded (W5.12).
  2. Verify the Phase 1 product still meets the Phase 0 honesty conditions (it can recommend not installing or collecting more data where evidence is insufficient).
  3. List any item not met with its owner and follow-up, and record the conditions for Wave 6 to start.
- **Deliverable / output:** A Phase 1 scope-review record with per-item verdicts, the honesty-condition check, and the Wave 6 start conditions. The record states explicitly that design §14 defines no F-17 Phase 1 exit gate and that this review is not one.
- **Acceptance criteria / exit check:**
  - [ ] Each §14 F-17 Phase 1 scope item has a verdict with evidence cited from W5.01–W5.09, W5.11 and W5.12.
  - [ ] The immutable calculation report is evidenced with versions, caveats and sensitivities.
  - [ ] The not-install / collect-more-data path is verified still present.
  - [ ] Any unmet item has a named owner and follow-up; Wave 6 start conditions are stated.
  - [ ] The record states that design §14 defines no F-17 Phase 1 exit gate and that this review is a scope review, not an exit gate.
- **Owner role:** Product
- **Dependencies:** W5.01, W5.02, W5.03, W5.04, W5.05, W5.06, W5.07, W5.08, W5.09, W5.11, W5.12, W4.13
- **Verification method:** Scope checklist against design §14 F-17 Phase 1; evidence inspected per item.
- **Risks / failure modes:** Presenting the scope review as a §14 exit gate, or starting Wave 6 while export or deletion controls are untested. Mitigation: the no-exit-gate statement is part of the record, and each scope item needs evidence from its owning subtask.
