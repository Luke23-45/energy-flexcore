# W4.07 — Input quality tiers

Workstream: companion-repo work (separate identity/database — NOT FlexCore control-plane work)

- **Objective:** Define the F-17 input quality tiers from design §9.1 (Tier A interval measured, B monthly measured, C household-declared, D default assumption), with the rule that monthly-only inputs never produce claims of measured hourly self-consumption.
- **Design trace:** Design §9.1; §15 ADR-F17-04
- **Owns:** Owns ADR-F17-04 (monthly-bill-only profile is not promoted to measured hourly data).
- **Inputs / prerequisites:** Design §9.1; §15 ADR-F17-04.
- **Steps:**
  1. Define the four tiers with their evidence and their permitted claims: Tier A interval measured (utility or meter-provided hourly/15-minute data with documented quality; suitable for much stronger self-consumption and battery dispatch analysis, subject to coverage and tariff correctness); Tier B monthly measured (actual monthly bills and consumption; suitable for annual/monthly solar-yield comparisons and a coarse bill model; not sufficient to claim precise hourly battery dispatch optimization); Tier C household-declared (appliance list, schedules or representative daily profiles supplied by the user; useful for indicative scenarios; explicitly estimated); Tier D default assumption (generic profile or equipment assumptions; use only when requested, show prominently, widen uncertainty, never label as measured). The tiers are defined from design §9.1 alone; the W4.04 prototype is a later verification consumer of these definitions, not a prerequisite for them.
  2. State the Tier B prohibition: if only monthly consumption is available, the planner can estimate PV production and compare broad solar-only scenarios, but must not report a precise battery size, hourly self-consumption, peak-shaving savings or optimized dispatch result as though interval data existed.
  3. Require every recommendation to state explicitly which tier it used.
  4. Record ADR-F17-04 as owned by this subtask. Name the owners (product, numerical validation) and the status (proposed, pending approval).
- **Deliverable / output:** An input-quality-tiers note with the four tiers, their permitted claims, the Tier B prohibition, and ADR-F17-04.
- **Acceptance criteria / exit check:**
  - [ ] All four tiers are defined with evidence and permitted claims.
  - [ ] The Tier B prohibition (no precise battery size, hourly self-consumption, peak-shaving savings or optimized dispatch from monthly-only data) is stated.
  - [ ] Every recommendation must state its tier.
  - [ ] ADR-F17-04 is recorded as owned here.
  - [ ] Owners (product, numerical validation) and status are recorded.
- **Owner role:** Product + numerical validation
- **Dependencies:** W4.01
- **Verification method:** Tiers checked against design §9.1; ADR-F17-04 reviewed by product and numerical validation.
- **Risks / failure modes:** Labelling a Tier D default as measured, or a Tier B profile as hourly. Mitigation: tier is a mandatory output field.
