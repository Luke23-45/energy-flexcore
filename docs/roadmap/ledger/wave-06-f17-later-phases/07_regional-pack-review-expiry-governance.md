# W6.07 — Regional pack review and expiry governance

Workstream: companion-repo work (separate identity/database — NOT FlexCore control-plane work)

- **Objective:** Define the expiry governance for regional energy planning packs: expired-pack labelling and source re-validation. Pack format and contents are defined in W6.01, not redefined here. Restated in the adversarial-audit patch pass (2026-11-21): the previous version required a local-expert review before offer, which has no basis in foundations.md §7 item 3 or design §14; that rule is deleted, and the expiry rules below are grounded in the validity-date rule and the W4.02 source discipline.
- **Design trace:** foundations.md §7 item 3; design §14 later phases
- **Inputs / prerequisites:** foundations.md §7 item 3; design §14 later phases; W6.01 jurisdiction packs; W6.04 financing/incentive support; W4.02 source discipline.
- **Steps:**
  1. State the boundary: pack contents and the source/jurisdiction/validity-date rule are defined in W6.01 (foundations.md §7 item 3: tariffs, net-metering/net-billing rules, equipment costs, incentives, financing assumptions, installation workflows; source, jurisdiction and validity date per item); this subtask defines no pack contents and no pack format.
  2. Record the expiry rule: a pack past its validity date is labelled expired, not silently used (foundations.md §7 item 3 requires a source, jurisdiction and validity date per pack item; a pack without a current validity window must not be offered as current).
  3. Record the re-validation triggers: validity-date expiry, a change in the pack's authoritative source, or a tariff change affecting the pack's tariff data (the W4.02 source discipline requires authoritative sources with validity dates; a pack whose source no longer matches must be re-validated before it is offered again).
  4. Record the governance record kept per pack (validity window, re-validation trigger state).
- **Deliverable / output:** A pack governance note: expiry labelling rule, re-validation triggers, and the per-pack governance record; no pack contents defined here.
- **Acceptance criteria / exit check:**
  - [ ] The boundary is stated: pack format and contents are defined in W6.01, not redefined here.
  - [ ] Expired-pack labelling (not silent use) and the re-validation triggers are stated, with their foundations §7 item 3 validity-date and W4.02 source-discipline basis.
  - [ ] The per-pack governance record (validity window, re-validation trigger state) is defined.
- **Owner role:** Product + local energy-market expert
- **Dependencies:** W6.01, W6.04, W5.10, W4.02
- **Verification method:** Governance rules checked against foundations.md §7 item 3 and the W4.02 source discipline; no pack contents duplicated from W6.01.
- **Risks / failure modes:** Using an expired regional pack as if it were current, or redefining pack contents here so W6.01 and W6.07 drift apart. Mitigation: expiry labelling is mandatory and this subtask defines no contents.
