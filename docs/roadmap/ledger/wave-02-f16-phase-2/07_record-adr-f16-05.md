# W2.07 — Record ADR-F16-05

Workstream: FlexCore control-plane work

- **Objective:** Record ADR-F16-05 from design §15: flexibility candidate goes through engineering qualification, then an onboarding draft, then human approval.
- **Design trace:** Design §15 ADR-F16-05; §5.5; §5.6
- **Owns:** Owns ADR-F16-05 (flexibility candidate to engineering qualification to onboarding draft to human approval).
- **Inputs / prerequisites:** Design §15 ADR table; W2.03 lifecycle; W2.04 checklist; W2.06 approval flow.
- **Steps:**
  1. Write ADR-F16-05 stating the decision: a flexibility candidate proceeds through engineering qualification, then an onboarding draft, then human approval; no step may be skipped.
  2. Record the context (the staged evidence process in §5.5 and the safe mapping in §5.6), the consequences (a draft has no control effect; qualification does not grant dispatch authority), and the alternatives considered (direct candidate-to-asset creation, rejected because it would bypass engineering review and human approval).
  3. Name the owners (power systems, security/OT, product) and the status (proposed, pending approval).
  4. Cross-reference W2.03, W2.04, W2.05 and W2.06 as the implementing subtasks.
- **Deliverable / output:** ADR-F16-05 as a filed architecture decision record.
- **Acceptance criteria / exit check:**
  - [ ] ADR-F16-05 states the candidate-to-qualification-to-draft-to-approval sequence with no skipped step.
  - [ ] Context, consequences and the rejected direct-creation alternative are recorded.
  - [ ] Owners (power systems, security/OT, product) and status are recorded.
  - [ ] W2.03–W2.06 are cross-referenced as implementing subtasks.
- **Owner role:** Power systems + security/OT + product
- **Dependencies:** W2.03, W2.04, W2.05, W2.06
- **Verification method:** ADR text checked against design §15 and §5.5/§5.6; owners confirmed.
- **Risks / failure modes:** Recording the ADR as approved before its owners have decided it. Mitigation: status is proposed pending approval until the owners record otherwise.
