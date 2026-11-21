# W4.13 — Phase 0 exit-gate review

Workstream: companion-repo work (separate identity/database — NOT FlexCore control-plane work)

- **Objective:** Hold the F-17 Phase 0 exit-gate review against the design §14 exit gate: the prototype can honestly tell a homeowner when solar, battery or neither is financially sensible under the selected local tariff; no installer CRM or advanced 3D design needed.
- **Design trace:** Design §14 F-17 Phase 0 exit gate
- **Inputs / prerequisites:** W4.01–W4.12 outputs; design §14 F-17 Phase 0; foundations.md §8 validation exercise.
- **Steps:**
  1. Check the exit gate with the prototype evidence: the prototype can honestly tell a homeowner when solar, battery or neither is financially sensible under the selected local tariff (W4.04–W4.05, W4.08); the per-case accuracy verdict is W4.05's.
  2. Verify the honesty conditions: the prototype recommends not installing or collecting more data where evidence is insufficient, and its financial conclusions are supported by the local tariff and cost basis (W4.02, W4.08), and every prototype result shown to a homeowner carries the §13.5 disclosure (W4.04).
  3. Confirm no installer CRM or advanced 3D design was needed for the gate, and record the usability verdict from W4.06.
  4. List any exit-gate item not met with its owner and follow-up, and hand over to Wave 5 only on the stated conditions.
- **Deliverable / output:** A Phase 0 exit-gate review record with a per-item verdict, the honesty-condition evidence, the usability verdict, and the Wave 5 handover conditions.
- **Acceptance criteria / exit check:**
  - [ ] The prototype honestly distinguishes solar, battery and neither under the selected local tariff.
  - [ ] The not-install / collect-more-data path is evidenced.
  - [ ] Financial conclusions are supported by the local tariff and cost basis.
  - [ ] No installer CRM or advanced 3D design is required for the gate; the usability verdict is recorded.
  - [ ] Any unmet item has a named owner and follow-up.
- **Owner role:** Product
- **Dependencies:** W4.01, W4.02, W4.03, W4.04, W4.05, W4.06, W4.07, W4.08, W4.09, W4.10, W4.11, W4.12
- **Verification method:** Exit-gate checklist against design §14 F-17 Phase 0; prototype and usability evidence inspected.
- **Risks / failure modes:** Declaring the gate met on a prototype that only ever recommends installing. Mitigation: the neither/not-install path is a required gate check.
