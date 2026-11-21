# W6.04 — Financing/incentive support where reliable data exists

Workstream: companion-repo work (separate identity/database — NOT FlexCore control-plane work)

- **Objective:** Add financing/incentive support for F-17 only where reliable data exists, per design §14 later phases.
- **Design trace:** Design §14 F-17 later phases
- **Inputs / prerequisites:** Design §14 F-17 later phases; W5.04 financial model; W4.02 incentive sources.
- **Steps:**
  1. List the financing/incentive options proposed per jurisdiction, with the authoritative source, eligibility, amount and effective dates for each.
  2. Apply the W5.04 rules: financing interest/term/down-payment or cash purchase assumptions are versioned inputs; incentives enter the cashflow only where their eligibility, amount and dates are sourced.
  3. State the refusal rule: where reliable financing or incentive data does not exist for a jurisdiction, the option is not offered or is labelled as unavailable, never invented.
  4. Record that the product is not financing underwriting or a loan marketplace (W5.01 non-goals).
- **Deliverable / output:** A financing/incentive support note listing options per jurisdiction with sourced eligibility, amounts and dates, and the refusal rule.
- **Acceptance criteria / exit check:**
  - [ ] Every option has an authoritative source, eligibility, amount and effective dates.
  - [ ] Financing/incentive inputs are versioned and enter the W5.04 cashflow only when sourced.
  - [ ] Unsupported options are not offered or are labelled unavailable, never invented.
  - [ ] The no-underwriting/no-loan-marketplace non-goal is preserved.
- **Owner role:** Product + local energy-market expert
- **Dependencies:** W4.02, W5.01, W5.04, W5.10
- **Verification method:** Options checked against design §14 and the W4.02 source pack; non-goal checked against W5.01.
- **Risks / failure modes:** Inventing an incentive to make a case look viable. Mitigation: only sourced incentives enter the cashflow.
