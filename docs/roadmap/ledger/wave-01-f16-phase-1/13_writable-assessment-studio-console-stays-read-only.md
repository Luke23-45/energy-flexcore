# W1.13 — Writable Assessment Studio; operator console stays read-only

Workstream: FlexCore control-plane work

- **Objective:** Define the writable Assessment Studio as the F-16 writable surface, and preserve the read-only contract of the existing operator console.
- **Design trace:** Design §0; §6.1; §1.1 console row; §15 ADR-F16-06
- **Owns:** Owns ADR-F16-06 (dedicated writable Assessment Studio; existing operator console remains read-only).
- **Inputs / prerequisites:** Design §0, §6.1, §1.1 console row; W1.11 placement; W1.12 API surface.
- **Steps:**
  1. State the boundary: add a dedicated writable Assessment Studio; do not change the read-only contract of the existing operator console (console README: all console API calls are GET/read-only).
  2. Define what the Studio does (create/correct/review/export assessment work through the W1.12 API surface) and what the console may do later (optionally show read-only assessment outputs).
  3. Record that the Studio is a new writable React/TypeScript surface in the placement from W1.11, and that its writes go through the control plane with explicit action authorization.
  4. Record ADR-F16-06 as owned by this subtask. Name the owners (frontend/API owners) and the status (proposed, pending approval).
- **Deliverable / output:** An Assessment Studio boundary note with the writable/read-only split and ADR-F16-06.
- **Acceptance criteria / exit check:**
  - [ ] The writable Studio and the read-only console contract are both stated, with the console GET/read-only property cited.
  - [ ] The Studio write path (through the control plane, explicit action authorization) is stated.
  - [ ] The optional later read-only display of assessment outputs in the console is stated as optional, not as a Studio replacement.
  - [ ] ADR-F16-06 is recorded as owned here.
  - [ ] Owners (frontend/API owners) and status are recorded.
- **Owner role:** Frontend/API owners
- **Dependencies:** W1.11, W1.12
- **Verification method:** Comparison against design §0, §6.1 and §1.1; console README read-only property confirmed.
- **Risks / failure modes:** A write path added to the operator console, breaking its read-only contract. Mitigation: the split is a named ADR.
