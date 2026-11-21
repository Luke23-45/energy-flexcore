# W0.06 — Decide F-16 first jurisdiction

Workstream: FlexCore control-plane work

- **Objective:** Decide (or record a scheduled decision for) the F-16 first jurisdiction: which electricity market, regulatory environment, data-retention/legal environment and tariff source is the first target.
- **Design trace:** Design §16 gate G1; §1.3
- **Owns:** Owns §16 gate G1 (F-16 jurisdiction: electricity market, regulatory, data-retention/legal environment, tariff source).
- **Inputs / prerequisites:** Design §16 item 1; W0.04 (D1 record); design §3.2 (one real pilot jurisdiction).
- **Steps:**
  1. Identify the candidate jurisdiction(s) from the D1 record (W0.04); do not select one silently.
  2. For the chosen jurisdiction, record the electricity market, the regulatory environment, the data-retention/legal environment and the authoritative tariff source.
  3. If not decided in Wave 0, record the owner, decision venue and target date, and state which Wave 1 subtasks are blocked until it closes.
  4. File the decision or interim position in the decision register.
- **Deliverable / output:** A G1 gate record: the F-16 first jurisdiction with market, regulatory, data-retention/legal and tariff-source entries, or a scheduled decision with owner, venue and date.
- **Acceptance criteria / exit check:**
  - [ ] The record names one first jurisdiction, or records the decision as scheduled with owner, venue and date.
  - [ ] All four G1 dimensions (market, regulatory, data-retention/legal, tariff source) are addressed, none left implicit.
  - [ ] Blocked Wave 1 subtasks are named where the decision is still open.
  - [ ] The decision or interim position is filed in the decision register.
- **Owner role:** Product + legal
- **Dependencies:** W0.04
- **Verification method:** Inspection of the G1 record against design §16 item 1 and the D1 record.
- **Risks / failure modes:** Building a tariff pack or a compliance-oriented report for the wrong jurisdiction. Mitigation: G1 is a named gate, not an assumption.
