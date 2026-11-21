# W6.05 — VPP enrollment handoff, optional

Workstream: companion-repo work (separate identity/database — NOT FlexCore control-plane work)

- **Objective:** Define the optional VPP enrollment handoff from design §12 steps 1–6: explicit opt-in, a signed versioned ResidentialDEREnrollmentPackage, aggregator verification, tenant-operator approval, no field-control credentials to the planner, and revocation/offboarding by the aggregator.
- **Design trace:** Design §14 F-17 later phases; §12 steps 1–6
- **Inputs / prerequisites:** Design §12; W6.06 enrollment package; W4.12 ADR-F17-01; W5.06 consent controls.
- **Steps:**
  1. Reproduce the §12 six steps: the planner creates a signed, versioned ResidentialDEREnrollmentPackage only when the homeowner explicitly opts into a residential aggregation program; the named aggregator receives the package through a separate integration endpoint; the aggregator verifies customer authorization, equipment ownership, grid/site mapping, interconnection rules, telemetry path and control permissions; a tenant operator reviews and approves creation of the appropriate FlexCore Site, Asset and capability records inside the aggregator tenant; device communications and dispatch remain on the qualified aggregator/site integration path and the household planner never receives field-control credentials; revocation and contract termination are handled by the aggregator enrollment/offboarding workflow, with explicit consequences for the Core assets and retained operational records.
  2. State that there is no automatic synchronization of home plans into the VPP platform, and that a residential user is not made a FlexCore tenant by default (the aggregator is normally the FlexCore tenant/operator and the household is an external customer/resource owner).
  3. State that sharing requires the separate, specific, revocable consent from W5.06, and that no silent transfer to FlexCore occurs.
  4. Record the handoff as optional: it is offered only where an aggregator program exists and the homeowner opts in.
- **Deliverable / output:** A VPP enrollment handoff note with the §12 six steps, the no-automatic-sync rule, the consent rule, and the optional status.
- **Acceptance criteria / exit check:**
  - [ ] All six §12 steps appear with their stated checks.
  - [ ] No automatic synchronization and no default FlexCore tenancy for residential users are stated.
  - [ ] Separate, specific, revocable consent is required; no silent transfer occurs.
  - [ ] The planner never receives field-control credentials.
  - [ ] The handoff is stated as optional.
- **Owner role:** Product + privacy/security + power systems
- **Dependencies:** W6.06, W4.12, W5.06, W5.10
- **Verification method:** Handoff checked step by step against design §12; boundary checked against ADR-F17-01.
- **Risks / failure modes:** Auto-syncing home plans into the VPP platform, or handing field-control credentials to the planner. Mitigation: both are forbidden in the note.
