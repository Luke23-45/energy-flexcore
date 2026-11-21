# W2.02 — Measure scenarios and economics

Workstream: FlexCore control-plane work

- **Objective:** Define measure scenarios and economics from design §5.4: immutable scenarios, interacting ECMs not naively summed, versioned cashflow with NPV, simple payback only secondary, and no ROI when tariff/cost data are unsupported.
- **Design trace:** Design §5.4; §1.1 optimization row; §13.5
- **Inputs / prerequisites:** Design §5.4; W2.01 ECM catalog; W1.06 MeasureScenario record; W1.07 provenance; W1.20 tariff pack.
- **Steps:**
  1. Define the MeasureScenario rule: a scenario is immutable once run; a re-run or a changed input creates a new scenario.
  2. Define interaction handling: do not add measure savings linearly when measures interact (for example lighting changes altering cooling/heating loads; HVAC controls altering load profiles and peak demand; PV and batteries interacting with tariff and export rules); model bundled scenarios explicitly or label interaction effects as unmodeled.
  3. Define the economics model: a versioned cashflow model with NPV, discount-rate basis, energy-price escalation, O&M, replacement, tax/incentive treatment and finance terms; simple payback only as a secondary indicator.
  4. State the refusal rule: do not publish an ROI result when the tariff or local installed-cost data are unsupported.
  5. State the §1.1 optimization rule for F-16: the solver-neutral problem IR and provider capability declarations already exist; use the existing FlexCore optimizer only with an explicit supported formulation, and unsupported classes must refuse.
  6. Apply §13.5 to measure-scenario outputs: every scenario result must state what data was used and absent, what is measured/declared/provider-derived/assumed, which tariff/weather/equipment model versions were used, the solver status, the uncertainty and limitations, whether more data or professional inspection is required, and whether any financial conclusion is unsupported.
- **Deliverable / output:** A measure-scenario and economics note with the immutability, interaction, cashflow and refusal rules.
- **Acceptance criteria / exit check:**
  - [ ] MeasureScenario immutability (new run for new inputs) is stated.
  - [ ] Interacting ECMs are not naively summed; bundled scenarios are modelled explicitly or labelled unmodeled.
  - [ ] The versioned cashflow model with NPV and its inputs is defined; simple payback is secondary only.
  - [ ] The no-ROI-when-tariff-or-cost-unsupported rule is stated.
  - [ ] The §1.1 optimization rule is stated (existing FlexCore optimizer only with an explicit supported formulation; unsupported classes must refuse).
  - [ ] The §13.5 disclosure items are required on every measure-scenario output.
- **Owner role:** Power systems + numerical engineering
- **Dependencies:** W2.01, W1.06, W1.07, W1.20
- **Verification method:** Scenario and economics rules checked against design §5.4; interaction examples verified against the catalog.
- **Risks / failure modes:** Naively summing interacting measures and overstating savings. Mitigation: interaction handling is a mandatory scenario rule.
