# W6.03 — Local roof imagery providers and verified installer quotes

Workstream: companion-repo work (separate identity/database — NOT FlexCore control-plane work)

- **Objective:** Add local roof imagery providers and verified installer quotes for F-17, per design §14 later phases, keeping generic assumptions visibly separate from actual vendor quotations.
- **Design trace:** Design §14 F-17 later phases
- **Inputs / prerequisites:** Design §14 F-17 later phases; W4.03 roof comparison; W5.02 roof model; W5.01 scope (no installer marketplace); W0.03 D4 record.
- **Steps:**
  1. List the local roof imagery providers proposed per jurisdiction, with coverage, quality and cost, and the manual fallback that remains.
  2. Define verified installer quotes: what makes a quote verified, what fields it carries, and how it is labelled as distinct from a generic estimate.
  3. State that generic assumptions stay visibly separate from actual vendor quotations, and that the product is not an installer marketplace or lead resale (W5.01 non-goals).
  4. Record the commercial terms per provider/quote source and the D4 position for each from W0.03.
- **Deliverable / output:** A roof-imagery and installer-quote note with providers per jurisdiction, the verified-quote definition, and the generic-vs-actual labelling rule.
- **Acceptance criteria / exit check:**
  - [ ] Providers are listed per jurisdiction with coverage, quality and cost; manual fallback remains.
  - [ ] Verified installer quotes have a stated definition and labelling distinct from generic estimates.
  - [ ] Generic assumptions stay visibly separate from actual vendor quotations.
  - [ ] The no-installer-marketplace non-goal is preserved.
- **Owner role:** Product + numerical engineering
- **Dependencies:** W4.03, W5.01, W5.02, W5.10, W0.03
- **Verification method:** Note checked against design §14 and W5.01 non-goals; provider terms checked against the D4 record.
- **Risks / failure modes:** Presenting a generic estimate as a vendor quotation, or drifting into lead resale. Mitigation: labelling and the non-goal are explicit.
