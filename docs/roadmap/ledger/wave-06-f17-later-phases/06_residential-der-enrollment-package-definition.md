# W6.06 — ResidentialDEREnrollmentPackage definition

Workstream: companion-repo work (separate identity/database — NOT FlexCore control-plane work)

- **Objective:** Define the ResidentialDEREnrollmentPackage from design §12: selected fields only (site location, equipment records, proposed PV/BESS capabilities, evidence digests, availability/operating constraints, consent metadata), signed and versioned.
- **Design trace:** Design §12
- **Inputs / prerequisites:** Design §12; W5.06 consent controls.
- **Steps:**
  1. Define the package as signed and versioned, containing only the selected fields from §12: customer-chosen site location, equipment records, proposed PV/BESS capabilities, source evidence references/digests, relevant availability/operating constraints, and consent metadata.
  2. State the minimization rule: no field outside the selected list is included. No household bills, interval data, raw addresses or occupancy data are included in the package under any condition; the selected-fields list is closed. Site location is included only in the form §12 requires (customer-chosen site location); raw postal addresses are never included.
  3. Define the versioning and signature rules (package version, signer, and how the aggregator verifies the signature and version before acting).
  4. Record the package as the only exchange format for the W6.05 handoff; no other home-plan data crosses the boundary.
- **Deliverable / output:** A ResidentialDEREnrollmentPackage definition with the selected fields, the minimization rule, and the versioning/signature rules.
- **Acceptance criteria / exit check:**
  - [ ] Only the six selected §12 field groups appear in the package definition.
  - [ ] The minimization rule is stated (no bills, interval data, raw addresses or occupancy data are included; the selected-fields list is closed, with no exceptions).
  - [ ] Versioning and signature rules are defined.
  - [ ] The package is stated as the only exchange format for the handoff.
- **Owner role:** Product + privacy/security
- **Dependencies:** W4.12, W5.06, W5.10
- **Verification method:** Package definition checked field by field against design §12.
- **Risks / failure modes:** Adding household bills or interval data to the enrollment package. Mitigation: the selected-fields list is closed.
