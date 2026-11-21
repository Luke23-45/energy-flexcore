# W5.04 — Financial model

Workstream: companion-repo work (separate identity/database — NOT FlexCore control-plane work)

- **Objective:** Define the F-17 financial model from design §10.5: common horizon, versioned inputs, NPV/discounted cashflow, simple payback secondary only, IRR omitted when cashflow sign changes make it ambiguous, no invented probability distributions, and payback/ROI suppressed when local tariff/cost basis is unsupported.
- **Design trace:** Design §10.5
- **Inputs / prerequisites:** Design §10.5; W4.02 cost sources; W4.08 tariff model; W5.03 scenarios.
- **Steps:**
  1. Define the baseline comparison with a common analysis horizon and explicitly versioned inputs: installed system cost by component (modules, inverter, battery, mounting, electrical work, permits, labor, taxes); O&M and inverter/battery replacement assumptions; PV degradation and battery usable-capacity/degradation assumptions from sourced data; electricity price escalation and export rate assumptions; incentive eligibility, amount and effective dates; financing interest/term/down-payment or cash purchase assumptions; discount rate (nominal or real, consistently handled); taxes and fees if in scope.
  2. Define the outputs: undiscounted bill savings and discounted cashflow/NPV; simple payback only as a secondary indicator; IRR optional and omitted when cashflow sign changes make the result ambiguous or non-unique.
  3. State the sensitivity rule: provide low/base/high sensitivities only where the input ranges have a credible source; do not invent probability distributions.
  4. State the suppression rule: a solar-yield estimate without a supported local tariff and installed-cost basis is a technical potential estimate, not a financial recommendation; suppress payback/ROI rather than invent the missing inputs.
- **Deliverable / output:** A financial-model note with the versioned inputs, the NPV/payback/IRR rules, the sensitivity rule and the suppression rule.
- **Acceptance criteria / exit check:**
  - [ ] The common horizon and versioned inputs are listed in full.
  - [ ] NPV/discounted cashflow is the comparison output; simple payback is secondary only; IRR is omitted when ambiguous.
  - [ ] Sensitivities require a credible source; no invented probability distributions.
  - [ ] Payback/ROI are suppressed when the local tariff/cost basis is unsupported.
- **Owner role:** Numerical engineering + local energy-market expert
- **Dependencies:** W5.03, W4.02, W4.08
- **Verification method:** Financial model checked against design §10.5; inputs traced to the W4.02 source pack.
- **Risks / failure modes:** Publishing an ROI from invented tariff or cost inputs. Mitigation: suppression is mandatory where the basis is unsupported.
