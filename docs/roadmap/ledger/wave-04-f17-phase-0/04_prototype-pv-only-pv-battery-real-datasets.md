# W4.04 — Prototype PV-only and PV+battery scenarios using real interval AND monthly-only datasets

Workstream: companion-repo work (separate identity/database — NOT FlexCore control-plane work)

- **Objective:** Prototype PV-only and PV+battery scenarios using real interval AND monthly-only datasets, per design §14 F-17 Phase 0, so the prototype is tested on both evidence tiers, with trustworthy-result disclosure per §13.5 on every result shown to a homeowner.
- **Design trace:** Design §14 F-17 Phase 0; §13.5
- **Inputs / prerequisites:** Design §14 F-17 Phase 0; §13.5; §9.1 input quality tiers; W4.02 tariff/cost sources.
- **Steps:**
  1. Build the prototype to run PV-only and PV+battery scenarios on real interval datasets (Tier A) and on real monthly-only datasets (Tier B).
  2. Record the dataset provenance for each run (source, coverage, interval duration, timezone, quality, completeness).
  3. Verify the Tier B rule in the prototype: monthly-only inputs never produce claims of a precise battery size, hourly self-consumption, peak-shaving savings, or optimized dispatch results as though interval data existed.
  4. Record the prototype scope and its limits as a prototype, not as a product.
  5. State the prototype engine: the REopt API v3 proof-of-concept adapter per §10.4; the final G7 decision is recorded later in W5.12 (W4.10 records an interim position only) and is not a prerequisite for this prototype.
  6. Apply §13.5 to every prototype result shown to a homeowner (including in the W4.06 usability tests): every result must state what data was used and absent, what is measured/declared/provider-derived/assumed, which tariff/weather/equipment model versions were used, the solver status, the uncertainty and limitations, whether more data or professional inspection is required, and whether any financial conclusion is unsupported.
- **Deliverable / output:** A Phase 0 prototype that runs PV-only and PV+battery scenarios on real interval and monthly-only datasets, with per-run provenance, Tier B limits enforced, and the §13.5 disclosure on every prototype result shown to a homeowner.
- **Acceptance criteria / exit check:**
  - [ ] PV-only and PV+battery scenarios run on real interval datasets.
  - [ ] PV-only and PV+battery scenarios run on real monthly-only datasets.
  - [ ] Every run records dataset provenance (source, coverage, interval, timezone, quality, completeness).
  - [ ] Monthly-only runs produce no claim of a precise battery size, hourly self-consumption, peak-shaving savings, or an optimized dispatch result.
  - [ ] Every prototype result shown to a homeowner carries the §13.5 disclosure (data used/absent, measured/declared/provider-derived/assumed, tariff/weather/equipment versions, solver status, uncertainty/limitations, more-data/professional-inspection need, unsupported-financial-conclusion flag).
- **Owner role:** Numerical engineering
- **Dependencies:** W4.02, W4.07, W4.08, W4.09
- **Verification method:** Prototype runs inspected per dataset; Tier B outputs checked for hourly-measurement claims (zero permitted).
- **Risks / failure modes:** A monthly-bill prototype presenting an hourly battery dispatch as measured. Mitigation: the Tier B rule is enforced in the prototype and checked here.
