# W0.07 — Decide professional audit responsibility

Workstream: FlexCore control-plane work

- **Objective:** Decide (or record a scheduled decision for) professional audit responsibility: who signs formal audit reports, and what qualifications, insurance and liability apply in the chosen market.
- **Design trace:** Design §16 gate G4; §7.2
- **Owns:** Owns §16 gate G4 (professional audit responsibility: who signs formal audit reports; qualifications/insurance/liability in the chosen market).
- **Inputs / prerequisites:** Design §16 item 4; §7.2 (ASHRAE Standard 211 boundary); W0.06 (first jurisdiction).
- **Steps:**
  1. Identify who would sign a formal audit report in the chosen market, and what qualifications that signer must hold.
  2. Record the insurance and liability position that applies to signed reports in that market.
  3. State explicitly what the software does and does not do: it supports evidence collection and report structuring for a qualified professional; it is not an auditor replacement and does not sign off reports.
  4. If not decided in Wave 0, record the owner, decision venue and target date, and name the Wave 1/2 subtasks affected.
- **Deliverable / output:** A G4 gate record: the signer, qualifications, insurance and liability position for formal audit reports, or a scheduled decision with owner, venue and date.
- **Acceptance criteria / exit check:**
  - [ ] The record names who signs formal audit reports, or records the decision as scheduled with owner, venue and date.
  - [ ] Qualifications, insurance and liability are each addressed for the chosen market.
  - [ ] The record states the software does not sign off reports and is not an auditor replacement.
  - [ ] Affected later subtasks are named where the decision is still open.
- **Owner role:** Product + legal + qualified engineering reviewer
- **Dependencies:** W0.06
- **Verification method:** Review of the G4 record against design §16 item 4 and §7.2; qualified-reviewer acknowledgement where the decision is made.
- **Risks / failure modes:** Implying the software certifies or signs off an audit. Mitigation: the sign-off boundary is stated in the gate record and repeated in W1.16.
