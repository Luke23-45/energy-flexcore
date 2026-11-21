# W3.03 — BMS/EMS/edge/interval integrations for the actual pilot

Workstream: FlexCore control-plane work

- **Objective:** Add BMS/EMS/edge/interval integrations only for the actual pilot and its security boundary, per design §14 F-16 Phase 3.
- **Design trace:** Design §14 F-16 Phase 3
- **Inputs / prerequisites:** Design §14 F-16 Phase 3; W1.03 first asset integration; W0.02 existing security gates.
- **Steps:**
  1. List only the integrations the actual pilot needs, from the W1.03 asset-integration decision; do not add an integration for an asset or protocol that is not present at the pilot facility.
  2. For each integration, record the security boundary it crosses, the authorization it requires, and the data it may read (no integration in this subtask grants dispatch authority or any control write — design §0 product boundary and §5.5; see step 3).
  3. Record that no integration in this subtask grants dispatch authority or any control write (design §0 product boundary: an optimizer result is not authorization to control a device; §5.5: the candidate lifecycle is separate from dispatch authority).
  4. Record the pilot operator and security review for each integration.
- **Deliverable / output:** A pilot-integration note listing each BMS/EMS/edge/interval integration for the actual pilot with its security boundary and review.
- **Acceptance criteria / exit check:**
  - [ ] Every integration listed is tied to an asset/protocol present at the actual pilot facility.
  - [ ] Each integration has a recorded security boundary and authorization.
  - [ ] No integration grants dispatch authority or any control write.
  - [ ] Pilot operator and security review are recorded per integration.
- **Owner role:** Integration engineering + security
- **Dependencies:** W1.03, W2.08, W0.02
- **Verification method:** Integration list checked against the W1.03 decision and design §14 F-16 Phase 3; security boundaries reviewed.
- **Risks / failure modes:** Adding a speculative integration for a protocol the pilot does not use. Mitigation: only the actual pilot justifies an integration.
