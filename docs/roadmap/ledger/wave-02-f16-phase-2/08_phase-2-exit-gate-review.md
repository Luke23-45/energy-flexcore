# W2.08 — Phase 2 exit-gate review

Workstream: FlexCore control-plane work

- **Objective:** Hold the F-16 Phase 2 exit-gate review against the design §14 exit gate: an independently reviewed candidate maps to existing Core contracts without schema shortcuts, and the Core simulation can consume the approved asset; this does not yet claim field readiness.
- **Design trace:** Design §14 F-16 Phase 2 exit gate
- **Inputs / prerequisites:** W2.01–W2.07 outputs; design §14 F-16 Phase 2; W1.18 Phase 1 exit; W1.17 test plan.
- **Steps:**
  1. Verify with an independent reviewer (not the candidate author) that a candidate maps to existing Core contracts without schema shortcuts (no new AssetType values, no building/household fields on Core Site/Asset, only supported capability schema v1).
  2. Verify the Core simulation can consume the approved asset (the approved Asset/CapabilityModel is usable by the existing scheduling pipeline in simulation).
  3. Confirm the review record states explicitly that this does not yet claim field readiness.
  4. List any exit-gate item not met with its owner and follow-up, and hand over to Wave 3 only on the stated conditions.
  5. Verify the Wave 2 §13.3 test record (deferred from W1.17): ECM applicability, bundle interaction handling and no silent savings stacking (W2.01/W2.02); flexibility qualification state transitions and expiry/revocation (W2.03); mapping of only supported candidate types to existing AssetType and CapabilityModel v1 (W2.05) — all evidenced against the Wave 2 definitions.
  6. Verify ADR-F16-05 is filed by W2.07 (candidate → engineering qualification → onboarding draft → human approval, with owners and proposed status) and that the gate evidence is consistent with it.
- **Deliverable / output:** A Phase 2 exit-gate review record with the independent-review evidence, the simulation-consumption evidence, the no-field-readiness statement, and the Wave 3 handover conditions.
- **Acceptance criteria / exit check:**
  - [ ] An independent reviewer (not the candidate author) has verified the mapping.
  - [ ] The mapping uses existing Core contracts with no schema shortcuts.
  - [ ] Core simulation consumption of the approved asset is evidenced.
  - [ ] The record states field readiness is not claimed.
  - [ ] Any unmet item has a named owner and follow-up.
  - [ ] The Wave 2 §13.3 items deferred from W1.17 (ECM applicability/bundle interaction, qualification state transitions, AssetType/CapabilityModel mapping) are evidenced against the Wave 2 definitions.
  - [ ] ADR-F16-05 is filed by W2.07 with owners and proposed status, and the gate evidence is consistent with it.
- **Owner role:** Power systems + product
- **Dependencies:** W2.01, W2.02, W2.03, W2.04, W2.05, W2.06, W2.07, W1.17, W1.18
- **Verification method:** Exit-gate checklist against design §14 F-16 Phase 2; mapping and simulation evidence inspected.
- **Risks / failure modes:** Declaring field readiness on a simulation-consumption check. Mitigation: the no-field-readiness statement is part of the gate record.
