# Phase 3 deterministic scenarios (`scenarios-phase3/`) — SYNTHETIC

**These scenarios are SYNTHETIC. They are not field data, not measurements,
and not a market product.** Every value in every file is a recorded fixture
value for the simulation-only 0.x line (AGENTS.md rule 5). D1–D3 are OPEN:
no market interpretation exists or is implied.

## What this directory is

The P3.26 deterministic scenario suite (ledger
`ledger/phase-03-flexibility-scheduling/ledger_phase_3.md` §P3.26; report
§19.2 layer 5, §19.3, §23 Stage 3): three pinned, reproducible scenarios over
the synthetic portfolio consumed from Phase 2 (scenario-v2 — one tenant,
two sites, per site one BESS, PV, flexible load, EVSE, plus a site meter;
full capacities and constraints travel inside each JSON and in
`../scenario-v2.json`).

| File | Scenario | Faults | Forecast input |
|---|---|---|---|
| `scenario-a-normal-day.json` | (a) normal day, all assets available | disabled | `../forecast-fixture-v1.json` (central, v1) |
| `scenario-b-pv-ramp.json` | (b) PV ramp / forecast error | disabled | `../forecast-fixture-v1-error-pv-ramp.json` (v1-error-pv-ramp: PV down 80 % of clear-sky inside 12:00–13:00 UTC) |
| `scenario-c-dropout.json` | (c) asset dropout mid-horizon | ONE `drop` on `site-north-bess-1`, 12:00–13:00 UTC | `../forecast-fixture-v1.json` (central, v1) |

The portfolio, horizon (24 h from 2026-06-01T00:00:00Z), cadence (60 s) and
master seed (20260601, per-asset derivation `seed(asset_key) = master_seed XOR
splitmix64(FNV-1a-64(asset_key))`) are byte-identical across all three files
and with the Phase 2 exit-gate scenario v2 — only identity, fault plan, and
the `phase3` pin block differ. Scenario (b) stresses the *forecast*, not the
telemetry: the scenario artefact is unchanged from (a) and the error lives in
the separately versioned forecast fixture (P3.31), never silently in the
scenario. Scenario (c) is the telemetry-side picture of the P3.24
asset-dropout roll trigger.

## The `phase3` pin block

Each file carries a top-level `phase3` object (ignored by the Phase 2
scenario loader, which parses a fixed key set) recording the P3.26 pins:
the 24 h / 15-min scheduling grid, the pinned solvers (Tier 1 HiGHS 1.15.1,
MIT — LP/QP/MILP and never SOCP; Tier 2 Clarabel.rs 0.11.1, Apache-2.0 —
continuous conic incl. SOCP, no integer variables), `thread_count = 1`,
replay tolerance 1e-9, the master seed, the forecast input, and a pointer to
the pre-registered expected outcomes.

## Named capacities and constraints (the synthetic portfolio fixture)

| Asset | Rated values | Declared constraint highlights |
|---|---|---|
| `site-north-bess-1` | 2.0 MW / 4.0 MWh | charge 01:00–05:00 @ 1.0 MW; discharge 18:00–21:00 @ 1.5 MW; SOC 0.5 → floor 0.05 / ceiling 0.95; round-trip η 0.9 |
| `site-south-bess-1` | 1.0 MW / 2.0 MWh | charge 00:30–04:30 @ 0.5 MW; discharge 17:00–20:30 @ 0.8 MW; SOC 0.4 → floor 0.05 / ceiling 0.95; round-trip η 0.9 |
| `site-north-pv-1` | 1.5 MW | clear-sky bell 05:30–21:00 UTC, peak factor 0.9; curtailment noted, not exercised |
| `site-south-pv-1` | 2.5 MW | clear-sky bell, peak factor 0.9 |
| `site-north-load-1` | 0.8 MW peak | morning/evening-peak baseline; reducible 0.3 MW |
| `site-south-load-1` | 1.2 MW peak | morning/evening-peak baseline; reducible 0.4 MW |
| `site-north-evse-1` | 0.05 MW | connection 17:30–08:00 UTC (wraps midnight); charge @ 0.05 MW; SOC 0.3 → target 0.9 |
| `site-south-evse-1` | 0.15 MW | connections 06:00–09:00 and 16:00–19:00 UTC; charge @ 0.15 MW; SOC 0.25 → target 0.85 |
| `site-*-meter-1` | 3.0 / 3.5 MW | PCC consistency fixture (aggregated site net power) |

Fixture extensions the JSONs cannot carry (recorded here, labelled
ASSUMPTION — synthetic values, no measured basis): the EV battery capacity is
0.06 MWh per aggregate EV, and the Phase 3 scheduling interpretation of the
EVSE profile is "departure at the end of the connection window that precedes
it, required departure SOC = the profile's `soc_target`" (north: departure
08:00 UTC, required 0.9; south: departure 09:00 UTC, required 0.85).

## Expected outcomes

Pre-registered BEFORE any run (the anti-fitting rule) in
`ledger/phase-03-flexibility-scheduling/25_26_tests_scenarios.md` — including
the honest status that they are pre-registered and UNRUN. Do not edit that
document's expectation tables after a run to make a failing scenario pass;
a mismatch is a finding, not a fitting opportunity.

## Execution

The offline-deterministic parts run as `cargo test -p flexcore-sim-runner
--test phase3_scenarios` (scenario load, replay determinism identity, and
asset/time-coupled constraint assertions through the domain models). The
real-solve legs (pinned HiGHS worker adapter, Tier 2 replay through
`ReplayHarness::verify_replay`) are VERIFY-BUILD-gated and remain a recorded
obligation — see the decision document above.
