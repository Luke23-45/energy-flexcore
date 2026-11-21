# W4.02 — Gather authoritative tariff, installed-cost, incentive and interconnection sources for that jurisdiction

Workstream: companion-repo work (separate identity/database — NOT FlexCore control-plane work)

- **Objective:** Gather authoritative tariff, installed-cost, incentive and interconnection sources for the selected launch jurisdiction, per design §14 F-17 Phase 0.
- **Design trace:** Design §14 F-17 Phase 0
- **Inputs / prerequisites:** Design §14 F-17 Phase 0; W4.01 launch jurisdiction; design §10.3 tariff engine.
- **Steps:**
  1. Gather the official utility/regulator tariff schedules for the selected jurisdiction, with source and effective dates per schedule.
  2. Gather installed-cost sources for the selected jurisdiction (equipment, mounting, electrical work, permits, labor, taxes), with source, currency and validity date per item.
  3. Record incentive eligibility, amounts and effective dates from authoritative sources, with the same source/date discipline.
  4. Gather the interconnection data for the selected jurisdiction (interconnection rules and process) from authoritative utility/regulator sources, with source and effective date per item, so the decision report's next-steps (W5.05) and the enrollment handoff (W6.05) can consume them.
  5. Record any tariff, cost, incentive or interconnection item that could not be sourced authoritatively as a named gap, not as an assumed value.
- **Deliverable / output:** A tariff, installed-cost, incentive and interconnection source pack for the launch jurisdiction, with source and validity date per item and a named-gaps list.
- **Acceptance criteria / exit check:**
  - [ ] Every tariff schedule has an authoritative source and effective date.
  - [ ] Every installed-cost item has a source, currency and validity date.
  - [ ] Incentives have eligibility, amount, effective dates and source.
  - [ ] Interconnection rules/process data have an authoritative source and effective date.
  - [ ] Unsourced tariff, cost, incentive or interconnection items appear in the named-gaps list, never as assumed values.
- **Owner role:** Product + local energy-market expert
- **Dependencies:** W4.01
- **Verification method:** Source pack checked against official utility/regulator schedules; every item traced to a source and date.
- **Risks / failure modes:** Using a generic or another-country tariff for the launch jurisdiction. Mitigation: only authoritative sources for the selected jurisdiction count.
