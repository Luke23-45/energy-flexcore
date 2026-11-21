# W5.01 — Consumer authentication and separate data store; product scope statement

Workstream: companion-repo work (separate identity/database — NOT FlexCore control-plane work)

- **Objective:** Define the F-17 Phase 1 product scope and the consumer authentication and separate data store, from design §8.1–§8.3 and §11.1: a decision-support app, not a whole-house audit, marketplace, lender, interconnection portal or home controller.
- **Design trace:** Design §14 F-17 Phase 1; §11.1; §8.1–§8.3
- **Inputs / prerequisites:** Design §8.1–§8.3; §11.1; W4.12 ADR-F17-01; W4.13 Phase 0 exit.
- **Steps:**
  1. Write the product scope statement: F-17 is a consumer-facing decision-support application that compares grid-only, solar-only and solar-plus-storage options for an individual household, helping a person understand physical feasibility, expected generation, self-consumption, import/export behaviour, financial sensitivity and next steps for a site-specific quotation or inspection.
  2. Reproduce the §8.3 non-goals list: 3D design and installer sales workflows comparable to OpenSolar or Aurora; installer marketplace or lead resale; automated roof structural adequacy certification; automatic permit/interconnection filing; financing underwriting or a loan marketplace; automatic equipment procurement; residential dispatch/control; a generic home energy audit that claims to assess insulation, ventilation, combustion safety and building condition from bills alone.
  3. Define consumer authentication and the separate data store per §11.1 (separate product/repository: web, API, domain, adapters, workers; separate database and private object store), consistent with ADR-F17-01.
  4. State that the generic household energy audit is deprioritized (foundations.md §5) and explicitly not in this ledger.
  5. Cross-reference the §2.1 logical architecture record (W0.09) for the companion product's placement in the joint cross-product diagram.
  6. State the §11.1 closing prohibition for the first user test: do not build a generalized plugin system, installer CRM or microservice fleet (design §11.1).
- **Deliverable / output:** An F-17 product scope statement with the §8.3 non-goals list, a consumer-authentication and separate-data-store note, and the §11.1 first-user-test prohibition.
- **Acceptance criteria / exit check:**
  - [ ] The scope statement states F-17 as decision support for grid-only, solar-only and solar-plus-storage comparison.
  - [ ] All eight §8.3 non-goals appear, none dropped or softened.
  - [ ] Consumer authentication and the separate data store are defined per §11.1 and ADR-F17-01.
  - [ ] The generic household energy audit is recorded as deprioritized and not in this ledger.
  - [ ] The §11.1 prohibition is stated: no generalized plugin system, installer CRM or microservice fleet for the first user test.
- **Owner role:** Product + privacy/security
- **Dependencies:** W4.12, W4.13, W0.09
- **Verification method:** Scope checked against design §8.1–§8.3 and §11.1; non-goals list compared line by line.
- **Risks / failure modes:** A non-goal (for example an installer marketplace) entering through a Phase 1 feature. Mitigation: the scope note is the scope-creep test at the Phase 1 gate.
