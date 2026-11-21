# W1.03 — First asset integration decision

Workstream: FlexCore control-plane work

- **Objective:** Decide the first asset integration: which actual PV/BESS/HVAC-thermal/EVSE combination exists at the pilot facility and which protocol/EMS can expose it.
- **Design trace:** Design §16 gate G3; §5.5; §5.6
- **Owns:** Owns §16 gate G3 (F-16 first asset integration: which actual PV/BESS/HVAC-thermal/EVSE combination exists at the pilot facility and which protocol/EMS can expose it).
- **Inputs / prerequisites:** Design §16 item 3; W1.02 pilot definition; design §5.5 qualification checklist and §5.6 mapping table.
- **Steps:**
  1. Inventory the actual PV, BESS, HVAC/thermal and EVSE assets present at the pilot facility from the W1.02 definition; do not assume an asset class that is not present.
  2. For each asset present, record which protocol or EMS can expose it and what evidence of that interface exists.
  3. Record which combinations are in scope for Phase 1 and which are deferred, with the reason for each deferral.
  4. File the G3 decision or interim position with owner, venue and date where still open.
- **Deliverable / output:** A G3 integration decision record: the actual asset combination at the pilot facility, the exposing protocol/EMS per asset, and the in-scope/deferred split.
- **Acceptance criteria / exit check:**
  - [ ] Every asset class claimed present is evidenced from the pilot facility inventory.
  - [ ] Each in-scope asset has a named protocol/EMS that can expose it.
  - [ ] Deferred asset classes are listed with a reason, not silently dropped.
  - [ ] The G3 decision or interim position is filed with owner, venue and date where open.
- **Owner role:** Power systems + integration engineering
- **Dependencies:** W1.02
- **Verification method:** G3 record checked against the pilot facility inventory and design §5.5/§5.6; protocol/EMS claims checked with the pilot operator or vendor documentation.
- **Risks / failure modes:** Assuming an integration path that does not exist at the pilot site. Mitigation: only evidenced protocol/EMS entries count.
