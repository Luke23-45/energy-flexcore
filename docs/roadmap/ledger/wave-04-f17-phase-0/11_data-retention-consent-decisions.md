# W4.11 — Data retention and consent decisions

Workstream: companion-repo work (separate identity/database — NOT FlexCore control-plane work)

- **Objective:** Decide data retention and consent for the launch jurisdiction, per design §16 G8 and §11.3: legal retention, account deletion, backup purge and data-sharing obligations.
- **Design trace:** Design §16 gate G8; §11.3
- **Owns:** Owns §16 gate G8 for the F-17 launch jurisdiction (the F-16 side is W0.10).
- **Inputs / prerequisites:** Design §16 item 8; §11.3; W4.01 launch jurisdiction.
- **Steps:**
  1. Record the legal retention period that applies in the launch jurisdiction, and which data classes it applies to.
  2. Record the account-deletion behaviour (what is deleted, what is retained under a legal obligation, and how deletion is verified) and the backup-purge behaviour (backup expiry).
  3. Record the data-sharing obligations in the launch jurisdiction (what may be shared, with whom, under what consent).
  4. If any item is not decided in Phase 0, record the owner, decision venue and target date, and name the Wave 5 subtasks affected.
- **Deliverable / output:** A G8 retention and consent record: legal retention, account deletion, backup purge and data-sharing obligations for the launch jurisdiction, or scheduled decisions with owners.
- **Acceptance criteria / exit check:**
  - [ ] Legal retention is recorded per data class for the launch jurisdiction.
  - [ ] Account deletion and backup purge are defined, including what is retained under a legal obligation.
  - [ ] Data-sharing obligations are recorded.
  - [ ] Any undecided item has an owner, venue and date, and affected Wave 5 subtasks are named.
- **Owner role:** Product + privacy/security + legal
- **Dependencies:** W4.01
- **Verification method:** G8 record checked against design §16 item 8 and §11.3; legal position confirmed for the launch jurisdiction.
- **Risks / failure modes:** Promising deletion the backups cannot honour, or sharing without a legal basis. Mitigation: retention, deletion and backup purge are decided together.
