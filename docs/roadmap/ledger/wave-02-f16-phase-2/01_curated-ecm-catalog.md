# W2.01 — Curated ECM catalog

Workstream: FlexCore control-plane work

- **Objective:** Build the small curated ECM catalog for the pilot archetype from design §5.4: each entry versioned and sourced, with applicability, prerequisites, savings method/version, cost basis/currency/region/validity, lifetime, O&M, interactions and limitations.
- **Design trace:** Design §5.4
- **Inputs / prerequisites:** Design §5.4; W1.02 pilot archetype; W1.06 ECM record; W1.18 Phase 1 exit.
- **Steps:**
  1. Select a small set of measure families for the pilot archetype (for example lighting controls, HVAC scheduling/controls, equipment replacement, compressed-air leak work, envelope improvement, onsite PV and storage), not an open-ended recommendation engine.
  2. For each catalog entry record: technology/category and applicability conditions; affected system and end-use boundary; required survey fields and evidence; prerequisites and exclusion conditions; savings calculation method and its version; low/base/high input assumptions where justified; installed-cost source, currency, region and validity date; useful life, maintenance and replacement assumptions; interactions with other measures; model limitations and verification requirements.
  3. Require each entry to be backed by a traceable source or a named engineering assumption; record no universal savings percentage.
  4. Version each entry so a later catalog change does not rewrite a past scenario.
- **Deliverable / output:** A versioned, sourced ECM catalog for the pilot archetype, with the §5.4 fields per entry.
- **Acceptance criteria / exit check:**
  - [ ] The catalog is small and scoped to the pilot archetype, not open-ended.
  - [ ] Every entry carries version, source, applicability, prerequisites, savings method/version, cost basis with currency/region/validity, lifetime, O&M, interactions and limitations.
  - [ ] No entry carries a universal savings percentage.
  - [ ] Entries are versioned so past scenarios keep their catalog version.
- **Owner role:** Power systems + energy assessor
- **Dependencies:** W1.02, W1.06, W1.18
- **Verification method:** Catalog checked entry by entry against design §5.4; sources traced to primary or named engineering assumptions.
- **Risks / failure modes:** An unsourced measure with a generic savings percentage entering the catalog. Mitigation: source-or-named-assumption is required per entry.
