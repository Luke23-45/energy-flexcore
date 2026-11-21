# W4.01 — Select the single launch country/utility jurisdiction

Workstream: companion-repo work (separate identity/database — NOT FlexCore control-plane work)

- **Objective:** Select the single launch country/utility jurisdiction for F-17, explicitly and with validation of where actual users will be found; do not silently choose a country.
- **Design trace:** Design §16 gate G5; §14 F-17 Phase 0; foundations.md §6
- **Owns:** Owns §16 gate G5 (F-17 launch jurisdiction: which country and distribution utility will supply authoritative tariff, export-credit, installed-cost, incentive and interconnection data).
- **Inputs / prerequisites:** Design §16 item 5; §14 F-17 Phase 0; foundations.md §6 (launch with ONE explicitly selected geographic market, not silently chosen).
- **Steps:**
  1. List the candidate countries/utilities considered and the user-validation evidence for each (where actual prospective users are, per foundations.md §6).
  2. Select ONE country and distribution utility as the launch jurisdiction, and record the selection explicitly with its reason.
  3. Record what the selection determines: the authoritative tariff, export-credit, installed-cost, incentive and interconnection data sources that later subtasks must use.
  4. File the G5 decision or interim position with owner, venue and date where still open.
- **Deliverable / output:** A G5 launch-jurisdiction record: the single selected country/utility, the candidates considered, the user-validation evidence, and the data sources the selection determines.
- **Acceptance criteria / exit check:**
  - [ ] Exactly one launch country and distribution utility is selected and named.
  - [ ] Candidate countries/utilities and the user-validation evidence are recorded.
  - [ ] The tariff, export-credit, installed-cost, incentive and interconnection data sources are named as determined by the selection.
  - [ ] The G5 decision or interim position is filed with owner, venue and date where open.
- **Owner role:** Product
- **Dependencies:** W0.08
- **Verification method:** G5 record checked against design §16 item 5 and foundations.md §6; selection is explicit, not silent.
- **Risks / failure modes:** Silently defaulting to a home country without validating where users are. Mitigation: the selection is a named G5 gate with user-validation evidence.
