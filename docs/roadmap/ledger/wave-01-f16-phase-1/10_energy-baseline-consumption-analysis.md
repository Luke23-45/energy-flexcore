# W1.10 — Energy baseline and consumption analysis

Workstream: FlexCore control-plane work

- **Objective:** Define the F-16 energy baseline and consumption analysis from design §5.3, with the completeness diagnostics and the data-readiness refusal, and with trustworthy-result disclosure per §13.5.
- **Design trace:** Design §5.3; §1.1 forecasts and weather rows; §13.5
- **Inputs / prerequisites:** Design §5.3; §13.5; W1.07 provenance; W1.09 ingestion; W1.20 tariff pack.
- **Steps:**
  1. Define the baseline question the service must answer (which period, boundary, fuels, meters, adjustments and model produced the baseline).
  2. Reproduce the §5.3 minimum design: reconcile bills against interval meter totals where both exist; preserve import and export separately; distinguish gross facility load, onsite generation, storage charge/discharge, grid import and grid export where evidence supports them; calculate monthly totals and demand peaks from the actual tariff/billing window (W1.20 tariff pack); support weather normalization only when weather, measurement periods and relevant operating variables are available; retain production, occupancy, operating hours, expansions, closures and other non-routine changes as explicit adjustment records; return data completeness, missing intervals, meter coverage and model diagnostics; produce a range or a data-readiness refusal where the evidence cannot support a normalized baseline.
  3. State the limits: 12 months of bills is a reasonable initial target for seasonality coverage, not a universal guarantee of an accurate baseline; higher-frequency meter data and operating context are required for more detailed load analysis; a bill-derived monthly profile must not be presented as an hourly load curve.
  4. State that P15.02 degree-day regression may be a reference for a separately specified weather-normalization method but is not itself proof of a compliant whole-building baseline or savings result.
  5. Apply §13.5: every result must state what data was used and absent, what is measured/declared/provider-derived/assumed, which tariff/weather/equipment model versions were used, the solver status, the uncertainty and limitations, whether more data or professional inspection is required, and whether any financial conclusion is unsupported.
  6. State the §1.1 reuse rules: the PV model declarations are UNTRAINED and unevaluated, and no accuracy claims are made for them; the vendor-neutral, tenant-scoped weather seam already exists but has no real provider adapter, it is appropriate for F-16, and it is not directly reusable by F-17's independent consumer identity boundary.
- **Deliverable / output:** An energy-baseline analysis note with the minimum design, the limits, the P15.02 boundary, and the §13.5 disclosure requirements.
- **Acceptance criteria / exit check:**
  - [ ] The baseline question (period, boundary, fuels, meters, adjustments, model) is stated.
  - [ ] All §5.3 minimum-design items appear, including the range-or-refusal rule.
  - [ ] The 12-months-is-a-target-not-a-guarantee limit and the bill-profile-is-not-an-hourly-curve rule are stated.
  - [ ] The P15.02 boundary is stated.
  - [ ] The §13.5 disclosure items are listed as required output for every result.
  - [ ] The §1.1 reuse rules are stated (PV model declarations UNTRAINED and unevaluated with no accuracy claims; weather seam exists without a real provider adapter, appropriate for F-16, not directly reusable by F-17).
- **Owner role:** Power systems + numerical engineering
- **Dependencies:** W1.07, W1.09, W1.20
- **Verification method:** Comparison against design §5.3 and §13.5; disclosure checklist checked per output type.
- **Risks / failure modes:** Presenting a bill-derived monthly profile as an hourly load curve, or a normalized baseline where evidence cannot support one. Mitigation: the range-or-refusal rule and the §13.5 disclosure are mandatory outputs.
