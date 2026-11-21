# W1.16 — ASHRAE Standard 211 boundary

Workstream: FlexCore control-plane work

- **Objective:** Define the ASHRAE Standard 211 boundary for F-16 from design §7.2: Standard 211 (2018, reaffirmed 2023) plus Addendum a, the stated scope exclusions, software as evidence/report support for a qualified professional, no conformance claim from a checklist or BuildingSync export, and a licensed standard plus qualified review before any normative workflow.
- **Design trace:** Design §7.2
- **Inputs / prerequisites:** Design §7.2; W0.07 (G4 professional audit responsibility); W1.01 scope statement.
- **Steps:**
  1. State the standard and version: ASHRAE Standard 211 (2018, reaffirmed 2023) and Addendum a, as the commercial-building energy audit / decarbonization-assessment framework.
  2. State the stated scope exclusions: single-family houses, multifamily buildings of three stories or fewer, and manufactured/mobile/modular homes are excluded; this reinforces that F-16 is the commercial assessment context and F-17 is not an ASHRAE 211 home-audit engine.
  3. State the software boundary: the software may support evidence collection and report structuring that an appropriately qualified professional uses; it must not claim conformance from the mere presence of a checklist or BuildingSync export.
  4. Require the current licensed standard and qualified engineering review before implementing any standard-specific normative workflow or making conformance claims.
- **Deliverable / output:** An ASHRAE Standard 211 boundary note with the version, scope exclusions, software boundary and licensed-standard requirement.
- **Acceptance criteria / exit check:**
  - [ ] Standard 211 (2018, reaffirmed 2023) and Addendum a are named.
  - [ ] The scope exclusions (single-family houses; multifamily of three stories or fewer; manufactured/mobile/modular homes) are stated.
  - [ ] The software-as-support-for-a-qualified-professional boundary is stated.
  - [ ] No conformance claim from a checklist or BuildingSync export is permitted, and a licensed standard plus qualified review is required before any normative workflow.
- **Owner role:** Product + qualified engineering reviewer
- **Dependencies:** W0.07, W1.01
- **Verification method:** Comparison against design §7.2; ASHRAE references cited from design §17.
- **Risks / failure modes:** Presenting a checklist or a BuildingSync export as ASHRAE conformance. Mitigation: the boundary is stated here and enforced at the Phase 1 exit gate (W1.18).
