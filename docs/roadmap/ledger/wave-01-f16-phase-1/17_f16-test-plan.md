# W1.17 — F-16 test plan

Workstream: FlexCore control-plane work

- **Objective:** Define the F-16 test plan from design §13.3 (domain/property tests, format/adapter contract tests, integration/security tests) and execute the Phase 1 scope of that plan.
- **Design trace:** Design §13.3
- **Inputs / prerequisites:** Design §13.3; W1.05–W1.16 outputs; W1.04 authorization matrix; W1.08 state machine.
- **Steps:**
  1. List the §13.3 domain/property tests for Phase 1 scope: Facility/building/boundary/meter relationships and tenant parent consistency; bill period/timezone and currency/unit validation; import/export sign and energy reconciliation; interval duplicates, gaps, late arrivals, DST short/long days and ambiguous local timestamps; provenance/quality preservation and unknown-narrows behavior. The following §13.3 items are NOT in Phase 1 scope and are deferred to the Wave 2 test record (W2.08 exit gate): ECM applicability, bundle interaction handling and no silent savings stacking (defined in W2.01/W2.02); flexibility qualification state transitions and expiry/revocation (defined in W2.03); mapping of only supported candidate types to existing AssetType and CapabilityModel v1 (defined in W2.05).
  2. List the §13.3 format/adapter contract tests for Phase 1 scope: BuildingSync v2.7.0 XSD plus selected Schematron validation, import/export round trips, unknown fields and invalid documents; utility bill parser fixtures from representative anonymized providers; interval meter imports with source row references and units; object-store upload checksum, malware quarantine, signed URL expiry and tenant-isolation tests; tariff source versions and unsupported-rule refusals.
  3. List the §13.3 integration/security tests for Phase 1 scope: RLS across every new tenant table with no cross-tenant FK or object access; authorization matrix across facility manager, assessor, reviewer, administrator and viewer; outbox atomicity and idempotent worker retry; evidence upload resource limits, XML XXE/entity-expansion tests, archive bomb and malformed CSV tests; assessment data cannot create an Asset until approval; no route from an assessment worker to command authorization or dispatch.
  4. Record the plan as a test list with the Phase 1 scope marked per item; execution of the plan is the work of this subtask, and its results are recorded as evidence, not asserted.
- **Deliverable / output:** An F-16 test plan listing the §13.3 tests in scope for Phase 1, with the execution results recorded as evidence per item.
- **Acceptance criteria / exit check:**
  - [ ] All three §13.3 test groups appear with their Phase 1 items listed.
  - [ ] The three Wave 2 items (ECM applicability/bundle interaction, qualification state transitions, AssetType/CapabilityModel mapping) are recorded as deferred to the Wave 2 test record (W2.08), not claimed as Phase 1 tests.
  - [ ] The authorization-matrix, RLS, XXE/archive-bomb and no-Asset-until-approval tests are present.
  - [ ] Each test item has a recorded result; an item that could not be run is recorded as NOT RUN with reason and owner, and does NOT count toward the exit check (it blocks the gate or is carried as an open item with owner and follow-up); no test is recorded as passed without evidence.
  - [ ] The plan states that no route from an assessment worker to command authorization or dispatch exists.
- **Owner role:** Platform engineering + security + power systems
- **Dependencies:** W1.04, W1.05, W1.06, W1.07, W1.08, W1.09, W1.10, W1.11, W1.12, W1.13, W1.14, W1.15, W1.16
- **Verification method:** Test list checked against design §13.3 item by item; results inspected as evidence, not summaries.
- **Risks / failure modes:** A test recorded as passed on verbal agreement, or a security test skipped because the happy path worked. Mitigation: only evidenced results count.
