# `scenarios/synthetic-portfolio/` — versioned simulator scenarios

This directory holds the versioned **scenario artefacts** the FlexCore
simulator consumes (report §8.1 target tree; ledger P2.13 step 5). The
exit-gate scenario is `scenario-v1.json` — one tenant, one portfolio, two
sites, each with BESS + PV + flexible load + EVSE + site meter, a 24-hour
horizon, a 1-minute cadence, and one master seed with a recorded per-asset
derivation rule.

## Synthetic status — read this first

**Everything in these files is synthetic.** The behaviour profiles
(`bess_daily_cycle`, `pv_clear_sky`, `load_baseline_reducible`,
`evse_connection_windows`, `site_meter_aggregate`) are shaped data with
stated assumptions, **not validated physical models** and not calibrated
against measured data:

- PV is a synthetic clear-sky shape with **no weather model**; curtailment
  is noted as a capability-profile reference but **not exercised in
  Phase 2**.
- The BESS dispatch is a fixed daily cycle with rectangular-efficiency SOC
  bookkeeping — no electrochemical, temperature, or degradation model; SOC
  stays in [0,1] by construction and power respects the rated limits.
- The flexible-load baseline is a declared shape with a reducible range —
  **no baseline method** is computed (D1/D2 open).
- EVSE connections follow declared windows — no mobility model, no V2G.
- The site meter emits the aggregated sum of its site's assets — a
  consistency fixture, not a physical PCC model.

Every value the simulator produces from these scenarios is **simulation-only
telemetry** (the entire 0.x line; ledger Phase 2 status row). It is never
measured data, never a field reading, and carries no safety, certification,
or control-authority implication. Downstream marking: every
simulator-derived value is distinguishable via its `source_id` (the
`simulated: true` flag of P2.17/P2.18 is derived from it).

## Determinism scope

Same scenario version + same seed + same code version = byte-identical
telemetry records (same `event_time`, values, quality, `source_sequence`).

**`ingest_time` is excluded from replay comparison — by construction.** It
is the wall-clock moment FlexCore received the value and depends on when a
run happens, not on the scenario. Replay (P2.24) compares
`(asset_id, event_time, quantity, value, quality, source_sequence)` and
never `ingest_time`.

## Versioning rule

A scenario change creates a **new version** (`scenario-v2.json`, …); a
released version is never edited in place. Replay always names the scenario
version + seed + code version — all three together are what make a run
reproducible. A generator-code change that breaks replay fails with a diff,
honestly (P2.24/P2.27).

## The `faults` section

The v1 skeleton (`{ "enabled": false, "injections": [] }`) was reserved by
P2.13 as the fill-in slot for the fault-injection plan, and P2.15 filled it
in place: the seven per-asset, per-interval injection definitions
(`drop`, `delay`, `out_of_range`, `source_invalid`, `duplicate`,
`out_of_order`, `conflicting_duplicate`) now live in `scenario-v1.json`'s
`faults` section, **versioned with the scenario** — fault injection is
scenario configuration, never ad-hoc code paths.

**The plan ships with `"enabled": false`**, so the v1 exit-gate run remains
all-`GOOD` telemetry with full per-asset coverage; fault behaviour is
reachable only by enabling the plan in configuration. SOURCE-level outcomes
per kind (pipeline end-to-end outcomes are P2.23's): drop = no frame for the
interval (absence; the platform's gap detector synthesizes `MISSING` — a
provider itself can never emit it, P2.04 D-T6); delay = released late on the
scenario clock so the value's age exceeds `freshness_limit_seconds` (the
P2.08 staleness rule marks it `STALE`); out_of_range = active power scaled
past the rated limit, code `OUT_OF_RANGE` → `SUSPECT`; source_invalid = code
`SOURCE_INVALID` → `BAD`; duplicate = identical re-emission under the same
identity; out_of_order = released after a later tick's frame;
conflicting_duplicate = same identity, different value.

**Scope, recorded:** these are simulator faults for testing the platform's
handling — they are **not claims about real device failure modes**. All
fault arithmetic is scenario-clock arithmetic; no fault path reads the wall
clock. Overlapping injections on one asset-tick apply in listed order.

## Format

JSON (decided once, D-SC1 in the P2.13 design document):
`serde_json` is already in the workspace dependency inventory, so the
scenario format adds no external crate. Design rationale for every schema
element is in `ledger/phase-02-asset-telemetry/13_scenario_model.md`.

## `scenario-v2.json` — the P2.25 exit-gate artefact (expected-quality plan)

`scenario-v2.json` is the **P2.25 exit-gate scenario**. Its portfolio,
horizon, cadence, seeds, and fault injections are byte-identical to v1
(P2.25 step 1 verified v1 against the P2.13 rule and found one item
missing: a **per-asset expected-quality plan** tied to the P2.15 fault
types). Per the versioning rule above, the content change created a NEW
version rather than editing v1 in place:

- `meta.version` = `v2`; the bump and its rationale are recorded in
  `ledger/phase-02-asset-telemetry/25_31_exit_portfolio_corrections.md`.
- The new top-level `expected_quality` section names, per injected asset,
  the window, the fault type, the expected stored quality state inside the
  window, and whether the window is an **expected coverage gap** (the
  `drop` fault — covered only by the gap detector's synthesized `MISSING`
  markers) or **covered** (all other fault types still emit one value per
  tick). This is the accounting input the P2.25 coverage check needs so
  injected gaps are distinguishable from unexplained ones.
- The simulator ignores the section (unknown top-level keys are tolerated;
  only behaviour-profile `params` objects are strictly validated), so the
  `run`/`replay` CLI commands accept v2 unchanged.

The exit gate (P2.25/P2.26) runs **v2**; v1 remains for the P2.24 replay
regression tests.

## Synthetic forecast fixtures (P3.31)

`forecast-fixture-v1.json` and `forecast-fixture-v1-error-pv-ramp.json` are
the Phase 3 **synthetic forecast fixtures** (ledger P3.31) — the single
forecast source for v0.2.0, since no Phase 2 subtask produces forecasts
(P2.28). They carry per-asset per-interval forecast values (PV output MW,
load MW, availability fraction) over the P2.13 exit-gate scenario's horizon
and cadence (24 h, 60 s intervals, start-inclusive / end-exclusive — the
P3.03 scheduling-grid semantics), generated deterministically from the
scenario's master seed (20260601) with the P2.14 derivation rule applied to
namespaced stream keys: `seed(stream_key) = master_seed XOR
splitmix64(FNV-1a-64(stream_key))`, `stream_key =
<asset_key>/forecast/<forecast_version>`.

**Everything in these files is synthetic.** The fixture-level `provenance:
synthetic` label and a per-series label sit next to every value (one label
per series, plus the fixture-level one); the generator's typed schema makes
the label impossible to drop (values are only accessible through the labeled
accessor), and the envelope input validation (`validate_forecast_input`,
P3.12) rejects unlabelled or unversioned forecasts with named errors. The
values are **not measured data and not market prices** — there is no price
data in the fixtures (D1/D2 open) — and the forecast shapes are the
scenario's declared profiles evaluated on the grid, not a weather model.

Two versions, versioned separately:

- **`forecast-fixture-v1.json`** — the central forecast (`v1`): the
  scenario's `pv_clear_sky` and `load_baseline_reducible` shapes at each
  interval start plus seeded ±2 percent forecast noise; availability
  constant 1.0 (the scenario declares availability constant true). Series
  cover the PV and flexible-load assets only (BESS/EVSE setpoints are
  schedule decisions; meters are aggregates — recorded scope).
- **`forecast-fixture-v1-error-pv-ramp.json`** — the P3.26 error variant
  (`v1-error-pv-ramp`): for both PV series, inside the 12:00–13:00 UTC
  window the forecast asserts a **downward ramp with the stated magnitude of
  80 percent** of the clear-sky value (`error_magnitude_milli: 800`).
  Against the clear-sky truth this is the forecast-error scenario P3.26 (b)
  consumes.

Generation method (recorded for exact reproduction): the authority is the
generator `generate_central_fixture()` / `generate_error_pv_ramp_fixture()`
plus `to_json()` in
`crates/flexcore-application/src/forecast_fixture.rs` (P3.31). Same seed →
byte-identical artefact; values are quantized to whole milli units (round
half away from zero) and rendered with exactly three decimals so no float
formatting enters the files. **RECORDED-OUTPUT / VERIFY-BUILD:** the
committed JSON was produced by replicating that algorithm outside cargo; the
byte-equality tests (`cargo test -p flexcore-application --test
forecast_fixture`) are the verification obligation on the first build —
regeneration must reproduce both files byte-for-byte, and any trig-evaluation
difference that crosses a quantization boundary fails there, visibly.

