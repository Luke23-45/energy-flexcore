# W6.02 — Utility-data connections

Workstream: companion-repo work (separate identity/database — NOT FlexCore control-plane work)

- **Objective:** Add utility-data connections for F-17, per design §14 later phases, as optional integrations with explicit homeowner consent, not as a global assumption.
- **Design trace:** Design §14 F-17 later phases
- **Inputs / prerequisites:** Design §14 F-17 later phases; §11.4 Green Button; W5.02 import; W5.06 consent controls.
- **Steps:**
  1. List the utility-data connections proposed, one per utility, with the connection mechanism (for example Green Button Connect My Data where supported) and the data it returns (usage/billing data through the ESPI model, interval varying by the utility meter data capability).
  2. Require explicit homeowner consent per connection, under the W5.06 specific revocable consent rule; no silent transfer and no connection without consent.
  3. Record the fallback that remains necessary (manual bill and CSV import) for utilities without a supported connection.
  4. Record the privacy/security review per connection (data minimization, provider-secret isolation, PII logging).
- **Deliverable / output:** A utility-data-connections note listing each connection with its mechanism, consent, fallback and review.
- **Acceptance criteria / exit check:**
  - [ ] Every connection is listed per utility with its mechanism and data.
  - [ ] Explicit homeowner consent is required per connection.
  - [ ] Manual bill and CSV import remain as the fallback.
  - [ ] A privacy/security review is recorded per connection.
- **Owner role:** Integration engineering + privacy/security
- **Dependencies:** W5.02, W5.06, W5.10
- **Verification method:** Connections checked against design §11.4 and §14; consent rule checked against W5.06.
- **Risks / failure modes:** Connecting a utility feed without homeowner consent. Mitigation: consent is a per-connection requirement.
