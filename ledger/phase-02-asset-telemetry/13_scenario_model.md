# P2.13 — Simulator scenario model (assets, profiles, seeds, determinism)

| Field | Value |
|---|---|
| Subtask | P2.13 — Define the simulator scenario model (assets, profiles, seeds, determinism) |
| Report trace | §19.1–§19.3, §24 steps 1–2, 10; §5.8 (`sim-lab` profile) |
| Inputs | P2.02 (registry model), P2.04 (canonical telemetry model); P0.23 (test strategy — deterministic scenario tests pin inputs, seeds, versions, outcomes) |
| Status | Design + the versioned exit-gate scenario artefact: `scenarios/synthetic-portfolio/scenario-v1.json` (+ directory README). No simulator code is written by this subtask (P2.14 consumes it) |
| Decisions | D1–D6 remain OPEN. Decisions D-SC1…D-SC6 below are made once for Phase 2 |

---

## 1. What a scenario is (ledger step 1)

A scenario is a **named, versioned, data-only artefact** that fully
determines what the simulator (P2.14) emits: the portfolio it simulates, the
simulation window and cadence, the per-asset behaviour profiles, the seeds,
and the fault-injection plan (empty in v1 — P2.15 fills it). A scenario
contains no code; the generator code lives in P2.14 and is versioned
separately (§4).

| Schema element | Content |
|---|---|
| `meta` | Scenario ID, human name, **version** (§4), a `synthetic: true` flag that must never be removed, and a description stating the synthetic status |
| `portfolio` | One tenant, one portfolio (operator, market region as a **label only** — P2.02; D1 open), and the sites. Each site carries its key, timezone (display-only, P0.09), PCC reference, grid-model reference (model + version — reference only, no power-flow code in Phase 2), and its assets. Every asset carries: `asset_key`, `asset_type` (BESS / PV / FLEXIBLE_LOAD / EVSE / SITE_METER per the P2.02 enum), **rated power (MW) and rated energy (MWh) where applicable**, a **capability-profile reference** (a reference string only — profile contents are Phase 3, P2.02 step 6), a **bus reference**, and its behaviour-profile parameters (§5). Asset `asset_key`s are stable scenario keys; the mapping to registry IDs happens at registration through the API (P2.25 step 2), never by direct SQL |
| `horizon` | The simulation window: `start` (UTC instant on the scenario clock) and `duration_hours` — 24 h for the exit-gate scenario (P2.25) |
| `cadence` | The per-scenario default emission interval (D-SC3: 1 minute) and optional per-asset overrides |
| `seeds` | One **master seed** plus the **per-asset derivation rule** (D-SC4) — never per-asset literal seeds hand-listed |
| `faults` | The fault-injection plan — an **empty skeleton in v1** (`{ "enabled": false, "injections": [] }`); P2.15 fills it with per-asset, per-interval fault configuration, versioned with the scenario (P2.15 step 2) |

The exit-gate scenario shape follows P2.25: one tenant, one portfolio, **at
least two sites**, each site with at least one asset of each of the four
exit-gate types (BESS, PV, flexible load, EVSE) plus a site meter.

## 2. Storage format (D-SC1)

**Decision: JSON** (`scenarios/synthetic-portfolio/scenario-v1.json`).
Rationale: `serde_json` is already in the workspace dependency inventory
(D4, Phase 1 — no new external crate), the scenario is consumed by Rust
tooling (P2.14) and emitted by the replay report (P2.24), and JSON keeps the
format decision from adding a dependency. TOML was considered for
readability and rejected on the dependency cost; readability is covered by
the directory README and this document instead.

## 3. Determinism scope (ledger step 2)

**The rule, verbatim in substance:** *same scenario version + same seed +
same code version = byte-identical telemetry records (same `event_time`,
values, quality, `source_sequence`)*.

The scope of that guarantee:

| Record element | Deterministic? | Why |
|---|---|---|
| `event_time` | **Yes** — on the scenario clock | Foundations §6.2 (adopted by P0.09 §5): simulation time is separate from wall-clock time; the horizon starts at the scenario's fixed `start` |
| Values | **Yes** — seeded profiles | Per-asset generators are seeded per D-SC4 and produce the same sequence for the same scenario version + seed + code version |
| Quality | **Yes** | v1 emits `GOOD` (P2.14 step 3); fault-injected states (P2.15) are scenario configuration, so they replay identically too |
| `source_sequence` | **Yes** — monotonically increasing per (asset, source), gap-free from 1 in the deterministic run | A record's identity (P2.04 §8) must replay identically |
| `source_id` | **Yes** | Fixed by the provider declaration |
| `ingest_time` | **NO — excluded, by construction** | It is the wall-clock moment FlexCore received the value; it depends on when the run happens, not on the scenario. **Replay comparisons use `event_time` and values, never `ingest_time`** (P2.13/P2.24) |

Consequences, recorded:

- The replay comparison fields (P2.24) are exactly
  `(asset_id, event_time, quantity, value, quality, source_sequence)` —
  `ingest_time` is never compared. A replay-comparison implementation that
  touches `ingest_time` is wrong by definition (tested in P2.24: two runs
  with different ingest times still match).
- The scenario clock and wall clock are distinct (P0.09 §5); `ingest_time >=
  event_time` can therefore not be assumed for far-future scenario windows —
  the construction rule (P0.09) is applied by the pipeline, and a scenario
  run against a wall clock *before* the scenario `start` is a legitimate
  backtest-shaped run, not an error.

## 4. Scenario versioning rule (ledger step 3)

**A scenario change creates a new version** — never an in-place edit of a
released version. The version is part of the artefact (`meta.version`,
semantic: `v1`, `v2`, … with the change recorded in the file's `meta.description`
or a sibling changelog entry). Replay (P2.24) **always names the scenario
version + seed + code version** — all three, because a replay is only
reproducible against the generator code that produced the original run:

- scenario version pins the portfolio, profiles, cadence, horizon, and fault
  plan;
- the seed pins the random stream within those profiles;
- the code version (toolchain + simulator code) pins the generation
  arithmetic. A generation change that breaks replay fails honestly with a
  diff (P2.24 acceptance; P2.27 CI) — it is an expected, recorded break, not
  a bug to hide.

Storing scenarios under `scenarios/synthetic-portfolio/` follows the §8.1
target tree (ledger step 5); the exit-gate scenario is the versioned
artefact P2.25 records and re-runs.

## 5. The four exit-gate asset behaviour profiles (ledger step 4)

**All four profiles are synthetic.** None is a validated physical model; none
is calibrated against measured data; each states its assumptions below and
in the scenario file. They exist to produce deterministic, canonical,
plausible-shape telemetry for platform testing (§23 Stage 2 exit gate) — the
§19.2 deterministic-scenario layer — and no result derived from them is
evidence about real device behaviour.

Per-asset quantities and units follow P2.04 §4 (BESS: P, Q, SOC,
availability, operating mode, health; PV: P, Q, availability; flexible load:
P, baseline reference where configured; EVSE: P, connection state; site
meter: P, Q at the PCC). Sign rule everywhere: P > 0 = export at the asset's
point of connection (P0.07 §2) — load consumption and EVSE charging are
negative.

### 5.1 BESS — charge/discharge shape respecting SOC [0,1] and rated limits

- **Shape (synthetic):** a fixed daily dispatch pattern on the scenario
  clock — charge during a configured overnight window, discharge during a
  configured evening window, near-zero exchange otherwise. Net power is
  `P_net = P_dis − P_ch` (P0.07 §6) and stays within the rated power in both
  directions.
- **SOC bookkeeping (synthetic, rectangular-efficiency):** SOC advances by
  charge/discharge power × cadence against the rated energy, with a single
  configured round-trip efficiency split and no leakage. **SOC is clamped to
  [0,1] by construction** — the generator never emits a step whose
  bookkeeping would leave [0,1]; it clamps and, in v1, does not flag the
  clamp (fault-driven SUSPECT cases are P2.15's, not this profile's).
- **Assumptions (synthetic — stated, not claimed):** no electrochemical
  cell model, no temperature or degradation model, no state-of-health drift,
  constant efficiency, no auxiliary consumption, instantaneous power
  transitions (no ramp-rate model beyond the cadence). Availability is
  constant `true` in v1; operating mode and health are verbatim
  source-declared states (P2.04 §3 companion note) with values like
  `"CHARGING"`/`"DISCHARGING"`/`"IDLE"` and `"NOMINAL"`.

### 5.2 PV — diurnal synthetic clear-sky shape, zero at night

- **Shape (synthetic):** a clear-sky production curve — zero from sunset to
  sunrise, a smooth (cosine-based) bell between sunrise and sunset scaled by
  the rated AC power and a configured peak factor. The curve is a function
  of the scenario clock only.
- **Curtailment:** the asset carries a capability-profile **reference** for
  curtailment (Phase 3 computes envelopes, P2.02 step 6); the capability is
  **noted but not exercised in Phase 2** — no Phase 2 mechanism reduces PV
  output below the shape. This is a stated boundary, not a missing feature.
- **Assumptions (synthetic — stated, not claimed):** fixed clear-sky
  geometry, **no weather model** (no clouds, irradiance time series, or
  forecasts), no temperature/soiling/derating model beyond the peak factor,
  no inverter clipping dynamics beyond the rated-power cap, single fixed
  azimuth/tilt. Availability constant `true` in v1.

### 5.3 Flexible load — baseline + reducible range

- **Shape (synthetic):** a baseline consumption curve (smooth morning and
  evening peaks over a near-flat night, scaled to the site's rated peak)
  **plus a declared reducible range** — a MW band around the baseline within
  which the (future) flexibility layer could reduce consumption. In Phase 2
  the simulator emits the **baseline** as `active_power` (negative:
  consumption) and the reducible range travels as scenario data for Phase 3;
  where the scenario explicitly configures a baseline reference, the
  `baseline_reference` quantity (MW, P2.04 §4) is emitted alongside.
- **Assumptions (synthetic — stated, not claimed):** no metered or measured
  data behind the shape, no weather or occupancy model, **no baseline
  *method*** — the baseline is a scenario-declared shape, not a computed
  estimate (the D1/D2 baseline-method questions stay open; P2.04 §4 records
  the same boundary), no per-appliance disaggregation, no stochastic
  variation beyond the configured seeded jitter.

### 5.4 EVSE — connection windows + charging shape

- **Shape (synthetic):** deterministic daily connection windows per EVSE
  (e.g. a home-arrival window and a morning-departure window on the scenario
  clock), with `connection_state = true` inside the window and `false`
  outside; while connected, a constant-power charging shape bounded by the
  rated power that fills the configured EV SOC band — the emitted `soc`
  quantity (fraction [0,1], P2.04 §4) advances with the charging bookkeeping
  and resets per the configured daily pattern. `energy` (MWh) accumulates
  delivered energy per the same bookkeeping where the scenario enables it.
- **Assumptions (synthetic — stated, not claimed):** no mobility or traffic
  model (connections follow the declared windows, not arrival-time
  distributions), one aggregate EV per EVSE (no per-vehicle disaggregation),
  constant charging efficiency, no V2G (discharge) in Phase 2, no user
  behaviour randomness beyond the configured seeded jitter, connection
  transitions are instantaneous at window boundaries.

### 5.5 Site meter (the fifth asset per site)

Not one of the four exit-gate profiles, but present per P2.25: the site
meter emits `active_power` and `reactive_power` at the PCC (site net export
positive, P2.04 §4). In v1 its active power is the **aggregated sum of the
site's assets' net active power** at each tick — a consistency fixture for
the current-state view, stated as synthetic (a real PCC meter would include
non-modelled load and losses; the simulated sum deliberately does not).
Reactive power is a small configured constant per site under the site's
declared `q_sign_convention` (P2.04 D-T4 — the convention travels on the
record).

## 6. Cadence (D-SC3) and seeds (D-SC4)

**D-SC3 — cadence: 1 minute (60 s), scenario-wide default.** Rationale
recorded: the §24 vertical slice runs a 24-hour horizon, so 1 minute gives
1,440 samples per asset per day — enough for current-state freshness,
staleness boundaries (P2.08), historical bucketing (P2.19), and partitioning
to be exercised meaningfully, while the whole exit-gate portfolio (2 sites ×
5 assets) stays at a few hundred thousand records — a volume the pipeline
handles without backpressure drama and the monthly-partition design (P2.05)
is honest about. 15-second cadence was considered and rejected for v1: 4×
the volume with no additional Phase 2 gate coverage; per-asset overrides
remain available in the schema.

**D-SC4 — seeds: one master seed + a recorded per-asset derivation rule.**
The scenario carries one `master_seed` (u64). Each asset's stream is seeded
as `seed(asset) = master_seed XOR splitmix64(FNV-1a-64(asset_key))` — the
exact rule is recorded in the scenario file's `seeds.derivation` string and
implemented once in P2.14, so adding a site or asset in a future version
does not perturb the streams of the existing assets. The RNG algorithm and
seed handling are recorded with the simulator (P2.14 step 2) so a re-run
reproduces the stream; the derivation rule is deterministic string → u64
hashing plus a finalizer, with no wall-clock or global-state input. Profiles
without randomness (PV clear-sky, the load baseline shape itself) consume no
randomness at all; jitter parameters draw from their asset's stream only.

## 7. Fault-injection skeleton (P2.15 boundary)

The v1 scenario carries `faults: { "enabled": false, "injections": [] }` —
a reserved section, **empty by design in v1**. P2.15 fills it with
per-asset, per-interval fault configuration (drop → MISSING, delay → STALE,
out-of-range → SUSPECT, source-invalid → BAD, duplicates, out-of-order,
conflicting duplicates), versioned with the scenario — never as ad-hoc code
paths (P2.15 step 2). The exit-gate v1 run therefore expects all-GOOD
telemetry with accounted-for coverage (P2.25 step 4).

## 8. The exit-gate scenario artefact

`scenarios/synthetic-portfolio/scenario-v1.json` implements §1 exactly:
one tenant, one portfolio, two sites (`site-north`, `site-south`), each with
BESS + PV + flexible load + EVSE + site meter (rated values, capability-
profile references, bus references, per-asset profile parameters), a
24-hour horizon starting at a fixed UTC midnight, the 1-minute cadence, the
master seed + recorded derivation rule, and the empty `faults` skeleton.
`scenarios/synthetic-portfolio/README.md` states the synthetic status and
the determinism scope so nobody meeting the artefact without this document
mistakes it for a validated model.

## 9. Acceptance-criteria coverage (ledger P2.13)

| Ledger acceptance criterion | Where satisfied |
|---|---|
| Scenario schema defined (portfolio, horizon, cadence, profiles, seeds, versions) | §1 (schema table), §2 (format), §6 (cadence D-SC3, seeds D-SC4), §4 (versioning) |
| Determinism scope stated, with `ingest_time` explicitly excluded from replay comparison | §3 (the rule verbatim in substance; the per-element table; the never-compared rule) |
| All four exit-gate asset profiles defined with stated assumptions | §5.1 (BESS, SOC [0,1] + rated limits), §5.2 (PV, zero at night, curtailment noted not exercised), §5.3 (flexible load, baseline + reducible range), §5.4 (EVSE, connection windows + charging shape) — each with a stated-assumptions paragraph; §5.5 adds the site meter |
| Scenario stored under `scenarios/synthetic-portfolio/` with a version | §8; `scenarios/synthetic-portfolio/scenario-v1.json` (`meta.version = "v1"`) + directory README |

Verification method (ledger): scenario review against §19.3 and §24 —
performed in §§1–8; the determinism test itself is P2.24's
(same scenario + seed → identical records across two runs), built on the
scope defined here.

## 10. What this document does not do

- It writes no simulator code (P2.14), no fault injection (P2.15), no
  replay entrypoint (P2.24).
- It claims no validated physical model for any profile (§5 assumptions are
  the permanent record), no forecast capability (forecasts are a Phase 3
  synthetic fixture, P3.31 — no Phase 2 artefact produces one), and no
  control authority above Simulation.
- It decides no baseline method (D1/D2 open — the load baseline is a
  declared shape, §5.3) and no market logic (market region is a label).
