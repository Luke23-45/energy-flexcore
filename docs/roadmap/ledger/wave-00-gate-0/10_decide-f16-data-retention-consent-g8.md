# W0.10 — Decide F-16 data retention and consent (G8, F-16 side)

Workstream: FlexCore control-plane work

- **Objective:** Decide data retention and consent for the F-16 launch jurisdiction, per design §16 G8 and §6.4: legal retention period, evidence deletion, backup purge and data-sharing obligations for facility assessment data. Added in the adversarial-audit patch pass (2026-11-21): G8 was owned only by W4.11 for the F-17 launch jurisdiction; the F-16 side had no owning subtask.
- **Design trace:** Design §16 gate G8; §6.4; §1.3
- **Owns:** Owns §16 gate G8 for the F-16 launch jurisdiction (the F-17 side is W4.11).
- **Inputs / prerequisites:** Design §16 item 8; §6.4 data-class-specific retention; W0.06 G1 jurisdiction.
- **Steps:**
  1. Record the legal retention period that applies in the F-16 launch jurisdiction, and which assessment data classes it applies to (facility/building records, bills, meter datasets, evidence objects, assessment runs, qualification records).
  2. Record the evidence-deletion behaviour (what is deleted, what is retained under a legal obligation, and how deletion is verified) and the backup-purge behaviour (backup expiry), consistent with the §6.4 rule that retention, export and deletion behaviour is data-class-specific and explicit, with no default inferred from an unset retention policy.
  3. Record the data-sharing obligations in the F-16 launch jurisdiction (what assessment data may be shared, with whom, under what consent or legal basis).
  4. If any item is not decided in Wave 0, record the owner, decision venue and target date, and name the Wave 1 subtasks affected (W1.14 persistence, W1.09 ingestion).
- **Deliverable / output:** A G8 retention and consent record for the F-16 launch jurisdiction: legal retention, evidence deletion, backup purge and data-sharing obligations, or scheduled decisions with owners.
- **Acceptance criteria / exit check:**
  - [ ] Legal retention is recorded per assessment data class for the F-16 launch jurisdiction.
  - [ ] Evidence deletion and backup purge are defined, including what is retained under a legal obligation.
  - [ ] Data-sharing obligations are recorded.
  - [ ] Any undecided item has an owner, venue and date, and affected Wave 1 subtasks are named.
- **Owner role:** Product + privacy/security + legal
- **Dependencies:** W0.06
- **Verification method:** G8 record checked against design §16 item 8 and §6.4; legal position confirmed for the F-16 launch jurisdiction.
- **Risks / failure modes:** Promising deletion the backups cannot honour, or sharing facility data without a legal basis. Mitigation: retention, deletion and backup purge are decided together.
