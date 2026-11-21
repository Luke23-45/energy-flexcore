# W0.04 — Record D1 and D2 decisions, or recorded interim positions

Workstream: FlexCore control-plane work

- **Objective:** Record the D1 (jurisdiction/market) and D2 (first portfolio/service) decisions, or recorded interim positions with owner, venue, date and change conditions, because they determine tariff, market and acceptance conditions for the extension work.
- **Design trace:** Design §14 Gate 0 item 4; §1.3
- **Inputs / prerequisites:** Design §14 Gate 0 item 4; v1 ledger D1/D2 records (P0.03, P0.04); .agent/DECISIONS.md.
- **Steps:**
  1. Read the current D1 and D2 status from the v1 ledger and decision record; do not re-decide from memory.
  2. For each of D1 and D2, record either the decision with its consequences, or an interim position naming the owner, decision venue, target date and the conditions under which it changes.
  3. Record explicitly which later subtasks may proceed on an interim position and which must wait for the decision.
  4. File an ADR or decision-record entry for any decision actually made; label recommended defaults as recommendations, never as decisions.
- **Deliverable / output:** D1 and D2 decision/interim-position records in the Wave 0 record and the decision register.
- **Acceptance criteria / exit check:**
  - [ ] D1 and D2 each have either a decision with consequences or an interim position with owner, venue, date and change conditions.
  - [ ] No later subtask is recorded as proceeding on a D1/D2 assumption that is not in the interim-position record.
  - [ ] Any recommended default is labelled as a recommendation, not a decision.
  - [ ] Decisions actually made have a filed ADR or decision-record entry.
- **Owner role:** Product + legal (D1); Product (D2)
- **Dependencies:** W0.02
- **Verification method:** Inspection of .agent/DECISIONS.md D1/D2 rows and the v1 ledger P0.03/P0.04 records.
- **Risks / failure modes:** A tariff pack or compliance report built on an unrecorded jurisdiction assumption and invalidated later. Mitigation: interim positions carry explicit change conditions.
