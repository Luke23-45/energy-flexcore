# W6.01 — More jurisdiction packs

Workstream: companion-repo work (separate identity/database — NOT FlexCore control-plane work)

- **Objective:** Add more jurisdiction packs for F-17, per design §14 later phases, each with the source, jurisdiction and validity-date discipline from foundations.md §7 item 3.
- **Design trace:** Design §14 F-17 later phases
- **Inputs / prerequisites:** Design §14 F-17 later phases; foundations.md §7 item 3; W4.01–W4.02 (first pack); W5.10 Phase 1 scope review; W4.08 tariff model.
- **Steps:**
  1. Select each additional jurisdiction explicitly (not silently) and record the user-validation evidence for it, following the W4.01 rule.
  2. For each pack, gather the authoritative tariff, net-metering/net-billing rules, equipment costs, incentives, financing assumptions and installation workflows from official sources, with source, jurisdiction and validity date per item (foundations.md §7 item 3).
  3. Apply the W4.08 tariff-model rules to each pack (executable tariff data; UNSUPPORTED_TARIFF_RULE refusal, never silent dropping).
  4. State that expired-pack labelling and re-validation are governed by W6.07; this subtask defines pack contents, not pack governance.
- **Deliverable / output:** Additional jurisdiction packs, each with authoritative sources, jurisdiction and validity date per item.
- **Acceptance criteria / exit check:**
  - [ ] Each additional jurisdiction is selected explicitly with user-validation evidence.
  - [ ] Every pack item has a source, jurisdiction and validity date.
  - [ ] The UNSUPPORTED_TARIFF_RULE refusal applies to every pack.
  - [ ] Pack expiry governance is owned by W6.07, not redefined here.
- **Owner role:** Product + local energy-market expert
- **Dependencies:** W4.01, W4.02, W4.08, W5.10
- **Verification method:** Packs checked against foundations.md §7 item 3 and design §14.
- **Risks / failure modes:** Launching a pack on another country's tariff. Mitigation: source, jurisdiction and validity date are required per item.
