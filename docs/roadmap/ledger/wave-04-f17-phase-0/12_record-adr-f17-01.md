# W4.12 — Record ADR-F17-01

Workstream: companion-repo work (separate identity/database — NOT FlexCore control-plane work)

- **Objective:** Record ADR-F17-01 from design §15 and §2.2: the household planner is a separate application/repository, with its own identity boundary and database, and must not create a FlexCore tenant or touch the FlexCore database.
- **Design trace:** Design §15 ADR-F17-01; §2.2
- **Owns:** Owns ADR-F17-01 (household planner is a separate application/repository, identity boundary and database; must not create a FlexCore tenant or touch the FlexCore database).
- **Inputs / prerequisites:** Design §15 ADR table; §2.2 product boundary decisions; W0.05 baseline record.
- **Steps:**
  1. Write ADR-F17-01 stating the decision: the household planner is a separate application and deployment, preferably in its own repository, with an independent consumer identity system, database, privacy/retention model and API; it is not a new FlexCore control-plane workspace.
  2. Record the prohibitions: it must not create an anonymous FlexCore tenant, connect to the FlexCore database, or receive any dispatch permission.
  3. Record the context (different principal, personal data, consent and deletion obligations, product cadence, usage profile and public traffic pattern) and the consequences (separate credentials, keys and object-store bucket; narrow, versioned enrollment exchange only under explicit consent, per design §12).
  4. Name the owners (product, privacy/security) and the status (proposed, pending approval).
- **Deliverable / output:** ADR-F17-01 as a filed architecture decision record.
- **Acceptance criteria / exit check:**
  - [ ] ADR-F17-01 states the separate application/repository, identity boundary and database.
  - [ ] The three prohibitions (no FlexCore tenant, no FlexCore database connection, no dispatch permission) are stated.
  - [ ] Context and consequences are recorded.
  - [ ] Owners (product, privacy/security) and status are recorded.
- **Owner role:** Product + privacy/security
- **Dependencies:** W0.05
- **Verification method:** ADR text checked against design §15 and §2.2; owners confirmed.
- **Risks / failure modes:** Inserting the planner into FlexCore tenancy for convenience. Mitigation: the boundary is a named ADR, not a guideline.
