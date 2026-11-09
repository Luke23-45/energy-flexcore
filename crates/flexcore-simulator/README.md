# flexcore-simulator

The deterministic synthetic telemetry simulator — the **Phase 2
`TelemetrySource` provider** (ledger P2.14) and its scenario-driven
**fault-injection mode** (P2.15). It implements the `TelemetrySource` port
(`flexcore-ports::telemetry_source`) and nothing else about the ingestion
path: no database, no broker, no pipeline internals, no direct storage
access.

## Scope and control status

Everything this crate emits is **synthetic, simulation-only telemetry**
(the entire 0.x line). The behaviour profiles are shaped data with stated
assumptions — **not validated physical models** and not calibrated against
measured data. The fault-injection mode produces **simulator faults for
testing the platform's handling** (quality mapping, staleness, dedup,
quarantine); it makes **no claim about real device failure modes**. Nothing
here touches a live grid, carries field write authority, or implies any
safety or certification property. FlexCore is a supervisory orchestration
layer and performs no primary frequency response, synthetic inertia, relay
protection, inverter inner-loop control, or sub-cycle control.

## What it does

- Loads the versioned scenario
  `scenarios/synthetic-portfolio/scenario-v1.json` (P2.13) into typed,
  validated structs. A scenario that fails validation is rejected whole —
  never partially loaded.
- Generates, per asset and per scenario-clock tick, canonical telemetry for
  the four exit-gate asset types plus the site meter: BESS (fixed daily
  charge/discharge cycle, rectangular-efficiency SOC bookkeeping clamped to
  [0,1] within the configured operating band), PV (synthetic clear-sky
  cosine bell, zero at night; curtailment is noted as a capability
  reference but **not exercised**), flexible load (declared baseline shape
  plus seeded jitter; the reducible range travels as scenario data), EVSE
  (declared connection windows, constant-power charging, EV SOC
  bookkeeping), and the site meter (the **aggregated sum of its site's
  assets' net active power** — a consistency fixture; **losses = 0 and
  non-modelled load = 0 by construction**, deliberately not a physical PCC
  model).
- Emits **canonical records only** (P2.04): canonical units via the
  `flexcore-contracts` newtypes, `P > 0` = export, SOC a fraction in [0,1],
  exactly one quality state per value (`GOOD` by default), `source_sequence`
  monotonically increasing per asset (gap-free in a fault-free run),
  `event_time` on the scenario clock, `ingest_time` stamped from the
  scenario clock at emission (the scenario clock is the only clock a
  deterministic simulator may read; this keeps `ingest_time >= event_time`
  true by construction — see the determinism note below).
- Implements `start` / `pause` / `resume` / `stop` and a **bounded,
  backpressure-aware emission rate**: generation is pull-driven. Nothing is
  generated unless the consumer pulls a frame, and each pull yields at most
  one frame (one asset, one tick). A paused or stopped source yields no
  frames.
- Declares the **provider rule** (P2.11 step 4) structurally: source ID
  (`sim://<scenario_id>` — never the reserved `flexcore:` namespace), the
  scenario default cadence, the quality-state mapping
  (`GOOD`→`GOOD`, `OUT_OF_RANGE`→`SUSPECT`, `SOURCE_INVALID`→`BAD`; the
  original code travels as `source_quality_code`), and clock behaviour
  (`ClockQuality::Synchronized` — the scenario clock is the simulation's
  time reference by construction).

## Determinism (P2.13 §3 scope)

Same scenario version + same seed + same code version ⇒ identical records
on `(asset, event_time, quantity, value, quality, source_sequence)`.
**`ingest_time` is excluded from replay comparison by construction** — it
is the moment of emission, not scenario data.

**RNG and seed handling (P2.14 step 2, recorded here for reproducibility):**

- Algorithm: **splitmix64** (Steele et al. 2014, public-domain reference
  implementation) — golden-ratio increment `0x9E3779B97F4A7C15` on the
  state, then the standard finalizer (`0xBF58476D1CE4E5B9`,
  `0x94D049BB133111EB`, final xor-shift 31). The same finalizer, applied to
  the hashed asset key, is the `splitmix64(x)` in the derivation rule.
- Per-asset seed derivation (D-SC4, verbatim as recorded in the scenario's
  `seeds.derivation`): `seed(asset_key) = master_seed XOR
  splitmix64(FNV-1a-64(asset_key))` where FNV-1a-64 is the standard offset
  basis `0xcbf29ce484222325` / prime `0x100000001b3` over the UTF-8 bytes
  of the key. Adding an asset in a future scenario version therefore does
  not perturb existing assets' streams.
- Streams are consumed in a fixed order (sites in scenario order, assets in
  listed order, one draw set per tick). Profiles without randomness (PV,
  the load baseline shape, the meter) consume no randomness; only the
  configured jitter draws from the asset's stream. Faults act on generated
  frames, never on the generator state — enabling a fault does not change
  any other tick's values.

## Fault injection (P2.15)

The fault plan is **scenario configuration, not code branches**: the
per-asset, per-interval injections live in the scenario's versioned
`faults` section and are parsed/validated like the rest of the scenario.
The seven fault kinds and their SOURCE-level outcomes (pipeline end-to-end
outcomes are P2.23's):

| Fault kind | Config | Source-level outcome |
|---|---|---|
| `drop` | — | No frame for the interval; sequence numbering is unchanged (the absence is the gap the pipeline's detector turns into `MISSING` — a provider can never emit `MISSING` itself, P2.04 D-T6) |
| `delay` | `delay_seconds` | Frame released `delay_seconds` late on the scenario clock; the record's age (release − `event_time`) exceeds the configured freshness limit ⇒ `STALE` per the P2.08 staleness rule (reused from `flexcore-domain::quality_rules`, not re-implemented) |
| `out_of_range` | `factor` | Active power scaled to `factor ×` rated, source code `OUT_OF_RANGE` ⇒ `SUSPECT` |
| `source_invalid` | — | Source code `SOURCE_INVALID` on every sample of the frame ⇒ `BAD` |
| `duplicate` | — | The frame is emitted twice, byte-identical in identity and value |
| `out_of_order` | `hold_ticks` | Frame held `hold_ticks` ticks and released after a later tick's frame ⇒ arrival order violates event/sequence order |
| `conflicting_duplicate` | — | The frame is emitted twice with the same identity (`source_sequence`, `event_time`, quantity) but a different value |

All fault arithmetic is **scenario-clock arithmetic**; no fault test or
fault path reads the wall clock. Injections apply per asset over
`[from, until)` on the scenario clock; overlapping injections on one
asset-tick apply in listed order.

**Decision recorded (P2.15 step 2):** the fault plan is encoded in
`scenarios/synthetic-portfolio/scenario-v1.json`'s `faults` section — the
section was explicitly reserved as the fill-in slot by P2.13 ("empty
skeleton in v1; P2.15 fills it"), so filling it keeps the scenario a single
versioned artefact instead of forking a second file. The plan ships with
`"enabled": false`, so the v1 exit-gate run remains all-`GOOD` with full
coverage; fault tests enable the plan by flipping that one configuration
flag on the parsed scenario — no fault logic is reachable without scenario
configuration.

## Workspace membership (coordinator note)

The root `Cargo.toml` was deliberately not edited. Add to
`[workspace] members`:

```toml
    "crates/flexcore-simulator",
```

and to `tools/check_dependency_rule.py` (composition-side classification):
`"crates/flexcore-simulator"` in `MEMBER_DIRS` and
`"flexcore-simulator"` in `COMPOSITION_CRATES`.
