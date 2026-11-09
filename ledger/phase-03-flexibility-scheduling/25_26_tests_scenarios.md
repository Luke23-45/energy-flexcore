# P3.25 + P3.26 — Decision note: test suites, coverage gaps, and the pre-registered scenario expectations

- **Date:** 2026-11-07.
- **Subtasks:** P3.25 (ledger_phase_3.md §P3.25) and P3.26 (§P3.26). Report
  trace: §19.2 layers 1–2 and 5, §19.3, §23 Stage 3.
- **Status that must be read first:** the ledger checkboxes for P3.25/P3.26
  are NOT ticked by this batch (coordinator-owned). Two recorded
  obligations remain open, stated here rather than hidden:
  1. **No coverage figure exists.** Cargo was not run while authoring (no
     build/test/clippy/fmt), so no number is claimed anywhere. The command
     for the real figure is in §4 below.
  2. **The scenario expectations below are PRE-REGISTERED and UNRUN.** They
     were written before any scenario execution (the anti-fitting rule,
     ledger P3.26 risk note). When the scenarios are first run, a mismatch
     against these tables is a FINDING to be investigated — never a reason
     to edit the tables to fit.

## 1. What was added (test files, all NEW; no production file touched)

| File | Subtask | Content |
|---|---|---|
| `crates/flexcore-domain/tests/property_suite.rs` | P3.25 | Property tests with stated generated-case counts (512 BESS schedules; 1024 provenance bounds; 512+512 P-Q points; 256 EV vehicles; 512 load event pairs) plus three unit-gap tests (SOC↔energy mapping incl. degenerate window; the standalone simultaneous charge/discharge detector; the UNKNOWN-provenance reserve floor). Seeded splitmix64 generators with forced boundary values (SOC 0.0/1.0, exact limits, on-circle P-Q points, exact-limit durations/gaps). |
| `crates/flexcore-optimization-api/tests/solver_io_suite.rs` | P3.25 | Solver I/O gate: all 15 `ProblemError` variants, each produced by exactly its malformed input; the same malformed set driven through the Tier 1 provider with ZERO backend calls; a 256-case property that any non-finite value in any IR coefficient slot rejects as `NonFiniteValue`; the §12.6 output gate at the provider boundary (incomplete optimal payloads refused as `IncompleteResult` after exactly one solve call); the 11-case `validate_complete` error matrix; the complete-outcome happy path. |
| `crates/flexcore-application/tests/property_suite.rs` | P3.25 | Envelope property (256 generated portfolios: upper ≤ Σ per-asset effective maxima after forecast/site coupling, exact boundary cases included); unknown-provenance asset never widens the envelope (128 cases, interval-exact equality); objective weight moves the argmin exactly at the analytically derived flip threshold (256 weight-sensitive cases); PCC tracking cost equals λ·Σ(P_PCC−C)²·Δt, zero exactly when met, strictly monotone in the mismatch (128 cases). |
| `scenarios/synthetic-portfolio/scenarios-phase3/README.md` + 3 scenario JSONs | P3.26 | The synthetic portfolio fixture and three pinned scenarios (§3). |
| `apps/sim-runner/tests/phase3_scenarios.rs` | P3.26 | Scenario execution: pinned-identity checks, replay determinism identity per scenario, the (c) dropout pipeline semantics, and the BESS/EV/load constraint assertions incl. the realised-SOC carry-forward. |

Disjointness (checked against the existing suites, per the ledger's "close
gaps, do not duplicate"): the new files re-test no existing named behaviour —
existing suites keep their fixtures; the property suites reuse the modules'
public APIs only. `test_rng` is `pub(crate)`, so the integration suites
re-state the same splitmix64 generator with their own fixed seeds (recorded
per test).

## 2. Static coverage map (module × test file × areas)

Honest status: this is a STATIC map from reading the suites — no measured
line/branch coverage exists yet (§4).

| Module | Covered by (integration) | Areas |
|---|---|---|
| flexcore-domain `bess_model` | `tests/bess_model.rs`, `tests/property_suite.rs`, sim-runner `phase3_scenarios.rs` | units/sign, SOC bounds (property + boundary fixtures), ramp/dwell, P bands, temperature derate, aux load, reserve, degradation placeholder, simultaneity detector, SOC↔energy mapping |
| flexcore-domain `inverter_pq_model` | `tests/inverter_pq_model.rs`, `tests/property_suite.rs` | S_max circle (property, both directions incl. on-circle), mode/P/Q windows, voltage rows, reactive sign declared |
| flexcore-domain `flexible_load_model` | `tests/flexible_load_model.rs`, `tests/property_suite.rs`, sim-runner suite | availability windows, duration/recovery (property, boundary-exact), rebound conventions, baseline sign |
| flexcore-domain `ev_fleet_model` | `tests/ev_fleet_model.rs`, `tests/property_suite.rs`, sim-runner suite | connection windows, hard departure energy (property + contrapositive), ramp, transformer limit, V2X gating, opt-out |
| flexcore-domain `capability_provenance` | in-crate tests, `tests/capability_validation.rs`, `tests/property_suite.rs` (1024 generated bounds) | narrowing rule, never-widens (property), propagation types |
| flexcore-domain `capability_validate` | `tests/capability_validation.rs` | units, sign, SOC, energy window, ramp, model walk |
| flexcore-domain `scheduling_time` | in-crate tests (16), `tests/capability_validation.rs` | boundary semantics, aggregation quality precedence, rollover |
| flexcore-domain `volt_var_scoping` | in-crate unit tests ONLY (6) — see gap G1 | curve sources/statuses, credit narrowing |
| flexcore-application `flexibility_envelope` | `tests/flexibility_envelope.rs`, `tests/property_suite.rs` | the site-coupling gate, forecast cap, unknown-narrows (property), energy cap, groups, loss, snapshot |
| flexcore-application `scheduling_objective` | `tests/scheduling_objective.rs`, `tests/property_suite.rs` | term definitions, versioned inputs, argmin weight sensitivity (property) |
| flexcore-application `pcc_tracking` | `tests/pcc_tracking.rs`, `tests/property_suite.rs` | multi-point separation, loss model, labels, quadratic cost (property) |
| flexcore-application `envelope_staleness`, `rolling_horizon`, `schedule_comparison`, `optimization_result_record`, `forecast_fixture` | their existing suites | unchanged; not duplicated |
| flexcore-optimization-api `problem`/`port`/`result`/`config`/`determinism`/`tier_guard`/providers | existing suites + `tests/solver_io_suite.rs` | IR validation (all variants), §12.6 gate, replay |
| sim-runner replay path | existing `tests/replay.rs` + `tests/phase3_scenarios.rs` | determinism identity, dropout window, canonical records |

## 3. Named coverage-gap list (untested invariants — explicitly, not silently)

- **G1 — `volt_var_scoping` has no integration-level test.** The six
  in-crate unit tests cover the pure helpers; no test composes the scoping
  into an envelope decision end to end. The envelope currently records
  `ReactiveEnvelopeNotComposed` as an unknown dimension, so the exposure is
  bounded, but the gap is named here per the ledger's rule.
- **G2 — BESS `soc_power_bands` + `temperature_derate` interplay is tested
  only with single-dimension fixtures** (each alone, never both narrowing
  the same interval); the property suite generates schedules over models
  with neither configured.
- **G3 — EV fleet multi-vehicle aggregation** (two vehicles sharing one
  EVSE/transformer limit simultaneously) has no property; existing tests
  are single-vehicle or opt-out cases. The scenario suite exercises one
  aggregate EV per EVSE (the fixture's declared shape).
- **G4 — the objective's curtailment and service terms** have config-level
  tests but no weight-sensitivity property (the energy/deviation terms
  carry the property in `property_suite.rs`).
- **G5 — real-solve determinism through the pinned Tier 1 worker adapter**
  and the Tier 2 replay over both conic fixtures: VERIFY-BUILD (decision
  note 19_20_determinism_tiers.md §1); the harness
  (`ReplayHarness::verify_replay`) is written and tested over scripted
  doubles; the real-solve leg needs the worker adapter.
- **G6 — scenario (b)'s effect is pinned at the forecast layer only** in
  this batch: the scenario JSON cannot carry a forecast reference the
  Phase 2 loader parses, so the pairing lives in the `phase3` pin block and
  the sim-runner test asserts the pin, not a forecast-driven schedule
  difference (that difference needs the real solve, G5).

## 4. The human's coverage command (recorded obligation — no figure claimed)

```sh
cargo llvm-cov --workspace --all-targets --html --output-dir target/coverage
# alternative, if llvm-cov is unavailable in the environment:
cargo tarpaulin --workspace --all-targets --out Html
```

Record the measured figure against the map in §2 and close (or re-scope)
the gaps in §3 with the run output in hand. P3.25's exit check ("coverage
figure is recorded") completes then.

## 5. P3.26 — the synthetic portfolio fixture and the three scenarios

The portfolio is consumed from Phase 2 (`scenarios/synthetic-portfolio/
scenario-v2.json`): one tenant, two sites, per site one BESS, PV, flexible
load, EVSE (+ site meter); the full named capacities/constraints travel in
each JSON and in the directory README. The scheduling grid is the P3.03
default (24 h horizon, 15-min intervals). Pinned solvers: Tier 1 HiGHS
1.15.1 (MIT), Tier 2 Clarabel.rs 0.11.1 (Apache-2.0), `thread_count = 1`,
replay tolerance 1e-9 (`DEFAULT_REPLAY_TOLERANCE`); master seed 20260601
with the Phase 2 per-asset derivation. Solver-class discipline travels in
the pin block (HiGHS = LP/QP/MILP, never SOCP; Clarabel = continuous conic,
no integer variables — AGENTS.md rule 4).

Fixture extensions recorded as ASSUMPTION (README, not invented in code):
EV battery capacity 0.06 MWh per aggregate EV; EV departure at the end of
the morning connection window with required SOC = the profile's
`soc_target`.

### Pre-registered expected outcomes — written BEFORE any run; UNRUN

Tolerances: constraint assertions use the models' own evaluation tolerances
(1e-9 in each value's unit: MWh, MW, SOC fraction); replay identity is
byte-exact; objective values and runtimes are RECORDED at run time, never
predicted here. "0 violations" means: the mapped schedule re-verified
through the flexcore-domain models reports an empty violation set within
those tolerances.

**Scenario (a) — normal day** (`scenario-a-normal-day.json`)

| Check | Expected outcome |
|---|---|
| Scenario load | passes all-or-nothing; synthetic flag true; seed 20260601 |
| Replay determinism | two runs byte-identical; comparison zero mismatches |
| Telemetry quality | every record GOOD; full coverage (1440 ticks per emitted quantity per asset) |
| BESS constraints (both sites) | 0 violations; SOC ∈ [E_min, E_max] at every boundary within 1e-9 |
| EV constraints (both sites) | 0 violations; departure SOC ≥ target (0.9 north / 0.85 south) within 1e-9 |
| Load constraints (both sites) | 0 violations; baseline-following net export ≤ 0 MW every interval |
| Real solve (VERIFY-BUILD) | status `optimal`; 0 asset/time-coupled violations; objective value + runtime recorded |

**Scenario (b) — PV ramp / forecast error** (`scenario-b-pv-ramp.json`)

| Check | Expected outcome |
|---|---|
| Scenario load / replay / telemetry | identical expectations to (a) — the fault plan stays disabled; the stressor is the forecast variant |
| Forecast input | the `v1-error-pv-ramp` fixture: PV values inside 12:00–13:00 UTC are exactly 20 % of the central fixture's values (80 % down), 0 % change outside |
| Envelope effect | the portfolio export upper bound inside the window is LOWER than scenario (a)'s by the corresponding PV forecast difference (within 1e-9); outside the window identical to (a) |
| BESS / EV / load constraints | identical expectations to (a): 0 violations — the ramp must not push any asset schedule out of its feasible set |
| Real solve (VERIFY-BUILD) | status `optimal`; objective value and runtime recorded; the objective MAY differ from (a) (the constraint set changed) — the difference is recorded, never fitted |

**Scenario (c) — asset dropout mid-horizon** (`scenario-c-dropout.json`)

| Check | Expected outcome |
|---|---|
| Fault plan | enabled; exactly one `drop` on `site-north-bess-1`, 12:00–13:00 UTC |
| Telemetry in window | NO frames for the dropped asset inside the window; the gap detector's MISSING markers cover it as an EXPECTED gap (per-asset plan pre-registers `MISSING` / `expected_gap`) |
| Telemetry outside window | every tick GOOD with full coverage, as (a) |
| Pre-dropout schedule | 0 violations up to the dropout boundary (interval 48 of 96) |
| Realised-state carry-forward | the post-dropout schedule starts EXACTLY at the realised boundary SOC (equal within 1e-9); the past is frozen, never re-solved |
| Post-dropout schedule | 0 violations over the remaining intervals; SOC stays inside the window |
| Roll record | the new revision records the `AssetOptOutDropout` trigger and the supersede link (P3.22/P3.24) |
| Real solve (VERIFY-BUILD) | status `optimal` on the re-solve; 0 violations; objective value and runtime recorded for BOTH the initial and the re-solve |

### The §23 Stage 3 exit gate

The gate ("schedules pass asset/time-coupled constraints on defined
synthetic cases") is met by tests 4 + the dropout test of
`apps/sim-runner/tests/phase3_scenarios.rs` (constraint assertions over the
pinned fixtures) once they run green under `cargo test -p
flexcore-sim-runner --test phase3_scenarios` — with the real-solve leg
closed via G5. A solved schedule remains a CANDIDATE: feasibility,
authorization, and dispatch are Phase 4 gates and nothing here implies
otherwise.

## 6. VERIFY-BUILD items for the coordinator / human

1. First `cargo test` over the three new test crates (the tests were
   authored without a build; compile fixes are expected to be mechanical).
2. The Tier 1 worker adapter build (17_optimizer_port.md §6 item 1) and the
   Tier 2 real-solve replay (18 §7 item 8) — then the scenario real-solve
   legs per §5.
3. The coverage figure command (§4) and the gap list review (§3).
4. `cargo llvm-cov`/`tarpaulin` availability in CI is a P3.27/P3.28
   concern (CI wiring is coordinator-owned; this batch did not touch
   `.github/workflows/**`).
