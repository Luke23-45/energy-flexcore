# W3.04 — Expand measure catalogs and sector support based on real evidence

Workstream: FlexCore control-plane work

- **Objective:** Expand the measure catalogs and sector support based on real evidence, per design §14 F-16 Phase 3, rather than by adding unsourced measures.
- **Design trace:** Design §14 F-16 Phase 3
- **Inputs / prerequisites:** Design §14 F-16 Phase 3; W2.01 ECM catalog; W2.02 scenarios.
- **Steps:**
  1. Identify the expansion candidates from real pilot evidence (measured results, assessor observations, operator feedback) and record the evidence for each.
  2. Apply the W2.01 catalog rules to every new entry (versioned, sourced, applicability, prerequisites, savings method/version, cost basis/currency/region/validity, lifetime, O&M, interactions, limitations); no entry is added without a traceable source or a named engineering assumption.
  3. Record which sectors the expansion covers and which remain out of scope, with the evidence for the sector choice.
  4. Version the expanded catalog so earlier scenarios keep their catalog version.
- **Deliverable / output:** An expanded ECM catalog (new version) with per-entry evidence and sector coverage.
- **Acceptance criteria / exit check:**
  - [ ] Every new entry has recorded real evidence and meets the W2.01 catalog rules.
  - [ ] No entry carries a universal savings percentage.
  - [ ] Sector coverage and out-of-scope sectors are recorded with reasons.
  - [ ] The catalog is versioned and earlier scenarios keep their version.
- **Owner role:** Power systems + energy assessor
- **Dependencies:** W2.01, W2.02, W2.08
- **Verification method:** Expanded catalog checked against design §14 F-16 Phase 3 and the W2.01 rules; evidence traced per entry.
- **Risks / failure modes:** Expanding the catalog from generic lists instead of pilot evidence. Mitigation: real evidence is required per entry.
