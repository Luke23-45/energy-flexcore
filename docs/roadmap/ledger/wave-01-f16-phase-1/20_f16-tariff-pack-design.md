# W1.20 — F-16 tariff-pack design

Workstream: FlexCore control-plane work

- **Objective:** Design the F-16 tariff pack (versioned tariff data for the pilot jurisdiction) from design §1.3 item 4 and §6.1 (tariff_data port), built only after the first F-16 pilot portfolio and jurisdiction are selected (D2/W1.02 and G1/W0.06). Added in the adversarial-audit patch pass (2026-11-21): W1.11 places the tariff_data port but no subtask designed the pack itself. This subtask is numbered after the Phase 1 exit-gate review (W1.18) because review-added subtasks continue the sequence and existing IDs are never renumbered; in execution order it is worked before W1.10, whose baseline uses the tariff/billing window from this pack.
- **Design trace:** Design §1.3 item 4; §6.1 tariff_data port
- **Inputs / prerequisites:** Design §1.3 item 4; §6.1; W0.06 G1 jurisdiction; W0.04 D2 pilot portfolio; W1.02 pilot facility; W1.11 placement.
- **Steps:**
  1. Record that no tariff pack is built before the first F-16 pilot portfolio and jurisdiction are selected (D2/W1.02 pilot facility and G1/W0.06 jurisdiction, design §1.3 item 4); a pack built on an unrecorded portfolio or jurisdiction assumption is invalid.
  2. Define the pack contents for the pilot jurisdiction: the tariff schedules that apply, with source, jurisdiction and validity date per item, following the versioned tariff pack access in the §6.1 tariff_data port.
  3. Record the pack versioning rule: each pack version names its source schedules and validity window; a superseded pack version is retained, not overwritten.
  4. State the consumers: W1.10 baseline analysis (tariff/billing window) and W2.02 measure economics (tariff data for savings and ROI decisions).
- **Deliverable / output:** An F-16 tariff-pack design: the pack contents, source/validity discipline, versioning rule, and its consumers.
- **Acceptance criteria / exit check:**
  - [ ] The no-pack-before-pilot-portfolio-and-jurisdiction rule is stated with its §1.3 item 4 basis.
  - [ ] The pack contents are defined with source, jurisdiction and validity date per item.
  - [ ] The pack versioning rule (superseded versions retained) is stated.
  - [ ] The consumers (W1.10 baseline, W2.02 economics) are named.
- **Owner role:** Product + power systems
- **Dependencies:** W0.04, W0.06, W1.02, W1.11
- **Verification method:** Pack design checked against design §1.3 item 4 and §6.1; pilot portfolio traced to the W1.02 pilot definition and jurisdiction traced to the W0.06 G1 record.
- **Risks / failure modes:** Building a tariff pack before a pilot portfolio exists or for the wrong jurisdiction, or overwriting a superseded pack version. Mitigation: the pilot portfolio and G1 gate the pack, and versions are retained.
