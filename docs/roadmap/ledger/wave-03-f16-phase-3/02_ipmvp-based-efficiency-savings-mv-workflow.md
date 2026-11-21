# W3.02 — IPMVP-based efficiency savings M&V workflow

Workstream: FlexCore control-plane work

- **Objective:** Define the IPMVP-based efficiency savings M&V workflow from design §7.3, using the IPMVP Core Concepts 2022 framework, with a licensed copy and practitioner review before protocol-specific details, and without reproducing licensed tables/formulas from summaries, and with trustworthy-result disclosure per §13.5 on every M&V result.
- **Design trace:** Design §7.3; §13.5; §15 ADR-F16-04
- **Owns:** Owns ADR-F16-04 (energy efficiency baseline/M&V is separate from P16 demand-response baseline/event M&V).
- **Inputs / prerequisites:** Design §7.3; §13.5; W3.01 context; W0.03 D4 record (IPMVP license/terms).
- **Steps:**
  1. State the framework: IPMVP Core Concepts 2022 is the relevant reference framework for energy-efficiency, renewable-energy and water savings verification, and savings are not directly measured but determined by comparing measured baseline and reporting-period consumption/demand on a consistent basis, with suitable adjustments.
  2. Require the current licensed copy of IPMVP Core Concepts 2022 and practitioner review before implementing protocol-specific details.
  3. State the prohibition: do not reproduce licensed tables/formulas from summaries.
  4. Define the workflow steps on the W3.01 context (plan version, baseline/reporting periods, adjustments, exclusions/uncertainty, reviewer and report status) and record ADR-F16-04 as owned by this subtask; name the owners (M&V practitioner, product) and the status (proposed, pending approval).
  5. Apply §13.5 to every efficiency-savings M&V result/report: every result must state what data was used and absent, what is measured/declared/provider-derived/assumed, which tariff/weather/equipment model versions were used, the uncertainty and limitations, whether more data or professional inspection is required, and whether any financial conclusion is unsupported. Solver status is recorded explicitly as N/A, not omitted silently: no solver runs in M&V determination — savings are determined by comparing measured baseline and reporting-period consumption/demand with adjustments, not by a solver.
- **Deliverable / output:** An IPMVP-based efficiency savings M&V workflow note with the framework, the licensed-copy requirement, the no-reproduction rule, ADR-F16-04, and the §13.5 disclosure on every M&V result.
- **Acceptance criteria / exit check:**
  - [ ] IPMVP Core Concepts 2022 is named as the framework, with the savings-are-compared-not-directly-measured statement.
  - [ ] A licensed copy plus practitioner review is required before protocol-specific details.
  - [ ] The do-not-reproduce-licensed-tables/formulas-from-summaries rule is stated.
  - [ ] ADR-F16-04 is recorded as owned here.
  - [ ] Owners (M&V practitioner, product) and status are recorded.
  - [ ] The §13.5 disclosure items are required on every M&V result/report, with solver status recorded explicitly as N/A (no solver runs in M&V determination).
- **Owner role:** M&V practitioner + product
- **Dependencies:** W3.01, W0.03
- **Verification method:** Workflow checked against design §7.3; IPMVP reference cited from design §17; D4 position for IPMVP checked in W0.03.
- **Risks / failure modes:** Implementing protocol details from a summary and publishing a licensed table. Mitigation: licensed copy and practitioner review are prerequisites.
