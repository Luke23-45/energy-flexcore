# W1.04 — User roles and least-privilege permission profiles

Workstream: FlexCore control-plane work

- **Objective:** Define the F-16 user roles and least-privilege permission profiles (facility manager, energy assessor, flexibility reviewer/power-systems engineer, tenant operator/administrator, viewer) and map them to FlexCore authorization via explicit action grants.
- **Design trace:** Design §3.3
- **Inputs / prerequisites:** Design §3.3; design §6.2 proposed action grants; existing flexcore-authz model.
- **Steps:**
  1. List the five §3.3 roles with their stated responsibilities (facility manager starts an assessment and supplies evidence; assessor records inspections and prepares the report; flexibility reviewer accepts or rejects qualification; tenant operator/administrator manages permissions and approves Core creation/linking; viewer reads allowed results).
  2. Define least-privilege action grants per role using explicit actions (for example assessment.create, assessment.read, assessment.evidence.upload, assessment.run, flexibility_candidate.review, asset_onboarding.propose, asset_onboarding.approve); do not define one broad auditor-can-do-everything role.
  3. Record that these are proposed permission profiles mapped through explicit action grants, and that no new global role enum is assumed to exist.
  4. Record the authorization matrix (role x action) as the test oracle for W1.17.
- **Deliverable / output:** A roles and permission-profiles note with the five roles, their responsibilities, the explicit action grants per role, and the authorization matrix.
- **Acceptance criteria / exit check:**
  - [ ] All five §3.3 roles appear with their stated responsibilities.
  - [ ] No role is defined as a broad can-do-everything role; permissions are explicit action grants.
  - [ ] The note states the grants are proposed names mapped through FlexCore authorization, not existing grants assumed present.
  - [ ] The authorization matrix covers every proposed action for every role.
- **Owner role:** Product + security
- **Dependencies:** W1.01
- **Verification method:** Comparison against design §3.3 and §6.2; matrix completeness check (every action x every role has an entry).
- **Risks / failure modes:** A broad auditor role accumulating dispatch-adjacent permissions. Mitigation: least-privilege grants are explicit and the matrix is the W1.17 test oracle.
