# W5.09 — F-17 test plan

Workstream: companion-repo work (separate identity/database — NOT FlexCore control-plane work)

- **Objective:** Define the F-17 test plan from design §13.4 (PVWatts benchmark cases, tariff tests, bill-only vs interval cases, battery balance/SOC/reserve, REopt/SAM comparison, cashflow edge cases, consent/deletion/export/PII/IDOR tests) and execute the Phase 1 scope.
- **Design trace:** Design §13.4
- **Inputs / prerequisites:** Design §13.4; W5.02–W5.08 outputs; W4.05 benchmark record.
- **Steps:**
  1. List the §13.4 tests for Phase 1 scope: solar yield benchmark cases for fixed systems across representative latitude, tilt, azimuth and system sizes, comparing adapter output against pinned PVWatts V8 reference cases; station selection, source dataset and station distance recorded, with no-station/large-distance/error cases; roof provider unavailable, low-quality or stale imagery, with manual roof-plane fallback; tariff tests for flat, TOU, tiered, net-metered/net-billed and demand-charge tariffs, with unsupported tariff rule refusal; bill-only versus interval-data cases, proving monthly-only inputs never produce claims of measured hourly self-consumption; battery energy balance, SOC bounds, reserve, charge/discharge limits, efficiencies, degradation/replacement and infeasible schedules; independent comparison of representative cases with REopt API v3 and SAM, documenting differences in weather inputs, tariff interpretation and battery models rather than expecting blind numerical identity; cashflow edge cases (replacement timing, taxes, incentives, financing, discount-rate basis, cashflow sign changes and non-unique IRR); consent, deletion, export, PII logging, IDOR and provider-secret isolation tests.
  2. Record the plan as a test list with the Phase 1 scope marked per item; execution results are recorded as evidence, not asserted.
  3. Record the benchmark versions and reference cases used per test group.
- **Deliverable / output:** An F-17 test plan listing the §13.4 tests in scope for Phase 1, with execution results recorded as evidence per item.
- **Acceptance criteria / exit check:**
  - [ ] All §13.4 test groups appear with their Phase 1 items listed.
  - [ ] The bill-only vs interval proof and the unsupported-tariff refusal test are present.
  - [ ] The consent/deletion/export/PII/IDOR/provider-secret tests are present.
  - [ ] Each test item has a recorded result; an item that could not be run is recorded as NOT RUN with reason and owner, and does NOT count toward the exit check (it blocks the gate or is carried as an open item with owner and follow-up); no test is recorded as passed without evidence.
- **Owner role:** Platform engineering + numerical engineering + security
- **Dependencies:** W5.02, W5.03, W5.04, W5.05, W5.06, W5.07, W5.08, W4.05
- **Verification method:** Test list checked against design §13.4 item by item; results inspected as evidence.
- **Risks / failure modes:** A privacy test skipped because the happy path worked. Mitigation: only evidenced results count.
