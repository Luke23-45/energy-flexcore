# W4.03 — Compare manual roof input versus optional roof provider; decide roof imagery budget and coverage

Workstream: companion-repo work (separate identity/database — NOT FlexCore control-plane work)

- **Objective:** Compare manual roof input versus an optional roof provider, and decide the roof imagery budget and coverage for the launch jurisdiction, per design §10.1 and §16 G6.
- **Design trace:** Design §14 F-17 Phase 0; §16 gate G6; §10.1
- **Owns:** Owns §16 gate G6 (roof imagery budget and coverage: is a paid roof data provider economically viable for that jurisdiction, and what manual fallback is required).
- **Inputs / prerequisites:** Design §10.1; §16 item 6; §14 F-17 Phase 0; W4.01 launch jurisdiction.
- **Steps:**
  1. Compare manual roof-plane entry (user-drawn/declared planes and obstructions, with source shown) against an optional provider (Google Solar API buildingInsights/dataLayers/geoTiff, with country-specific coverage and HIGH/MEDIUM/BASE imagery quality, and source and imagery age shown).
  2. Record the coverage and quality limits for the launch jurisdiction, including that Google notes its coverage map is approximate and some requests near coverage boundaries return 404.
  3. Decide the roof imagery budget: whether a paid roof data provider is economically viable for that jurisdiction, and what manual fallback is required.
  4. Record the explicit roof-assessment-unavailable path, and state that no provider is a mandatory system dependency.
- **Deliverable / output:** A roof-input comparison and G6 decision record: manual vs provider comparison, coverage/quality limits, budget decision, manual fallback, and the unavailable path.
- **Acceptance criteria / exit check:**
  - [ ] Manual roof input and the optional provider are compared on coverage, quality and cost for the launch jurisdiction.
  - [ ] The coverage/quality limits (including the approximate coverage map and 404 boundary behaviour) are recorded.
  - [ ] The budget decision and the required manual fallback are recorded.
  - [ ] The roof-assessment-unavailable path is defined, and no provider is mandatory.
- **Owner role:** Product + numerical engineering
- **Dependencies:** W4.01
- **Verification method:** Comparison checked against design §10.1 and §16 item 6; provider coverage checked for the launch jurisdiction.
- **Risks / failure modes:** Making a paid roof provider mandatory and failing where it has no coverage. Mitigation: manual fallback and the unavailable path are mandatory.
