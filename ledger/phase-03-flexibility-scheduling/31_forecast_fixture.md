# P3.31 — Synthetic forecast fixture: decisions and review record

- **Date:** 2026-11-07.
- **Scope:** P3.31 (synthetic forecast fixture — the single forecast source
  for v0.2.0; no Phase 2 forecast exists, P2.28).
- **Normative sources:** report §11.6 (portfolio aggregation consumes
  forecasts), §12.1 (scheduling objective), §15.3 (`ForecastPublished`),
  §10.2 (units and sign), §10.3 (time model); §23 Stage 2 (forecasts are NOT
  a Stage 2 deliverable); P2.13/P2.14/P2.25 (scenario model, seed rule,
  scenario v2); P3.03 (scheduling grid); foundations specification governs on
  conflict.
- **Files:** `crates/flexcore-application/src/forecast_fixture.rs` (+ lib.rs
  module declaration and re-exports),
  `crates/flexcore-application/tests/forecast_fixture.rs`,
  `scenarios/synthetic-portfolio/forecast-fixture-v1.json`,
  `scenarios/synthetic-portfolio/forecast-fixture-v1-error-pv-ramp.json`,
  `scenarios/synthetic-portfolio/README.md` (additive section).

## 1. Decisions recorded here

| # | Decision | Where implemented |
|---|---|---|
| D-FX-1 | The fixture is the single forecast source for v0.2.0: per-asset per-interval values (PV output MW, load MW, availability fraction) over the P3.03 grid semantics with the scenario's cadence (60 s) as the interval length; 24 h horizon from `2026-06-01T00:00:00Z`, 1440 intervals, start-inclusive / end-exclusive. `SchedulingGridConfig::new(86_400_000, 60_000)` is a valid P3.03 grid, so the P3.12 envelope and the P3.03 aggregation consume the fixture on the same grid. | `forecast_fixture.rs` (`FixtureGrid`) |
| D-FX-2 | Forecast series cover the scenario's PV and flexible-load assets only. BESS and EVSE setpoints are schedule decisions (P3.21/P3.22), not forecast inputs; site meters are aggregates. Availability is forecast per asset as a fraction in [0, 1]; the scenario declares availability constant true, so it is the constant 1.0. No second forecast is invented anywhere in Phase 3. | `forecast_fixture.rs` (`SERIES_SPECS`), README |
| D-FX-3 | Seed derivation: the P2.14 rule applied to namespaced stream keys — `seed(stream_key) = master_seed XOR splitmix64(FNV-1a-64(stream_key))`, `stream_key = <asset_key>/forecast/<forecast_version>`. One splitmix64 draw per interval per power series, in interval order. Master seed 20260601 from `scenario-v2.json`. Same seed → byte-identical fixture (test-asserted). Adding an asset or a forecast version never perturbs another series' stream. | `forecast_fixture.rs` (`derive_asset_seed`, `generate_power_series`) |
| D-FX-4 | Central forecast (`v1`): the scenario's declared shapes (`pv_clear_sky` cosine bell, `load_baseline_reducible` baseline — simulator formulas duplicated and test-pinned) evaluated at each interval's start-inclusive instant, plus seeded ±2 percent forecast noise (20 milli, integer-recorded). NOT a weather model; ASSUMPTION recorded in the artefact's `shape_note`. | `forecast_fixture.rs` (`pv_shape`, `load_shape`, `FixtureMethod::recorded`) |
| D-FX-5 | Error variant (`v1-error-pv-ramp`), versioned SEPARATELY from the central forecast: for both PV series, inside 2026-06-01T12:00:00Z–13:00:00Z (intervals [720, 780)) the forecast asserts a downward ramp with the stated magnitude **80 percent** of the clear-sky value (`error_magnitude_milli: 800`). Only downward variants are generated in v0.2.0 (recorded boundary). This is the P3.26 scenario (b) input. | `forecast_fixture.rs` (`generate_error_pv_ramp_fixture`, `ForecastErrorVariant`) |
| D-FX-6 | Quantization and formatting: every value quantized to whole milli units (round half away from zero) and rendered with exactly three decimals — the artefact is byte-reproducible **without float formatting**. The milli value is authoritative; the API-boundary value is `milli / 1000.0` MW (fraction for availability). | `forecast_fixture.rs` (`quantize`, `fmt_milli`) |
| D-FX-7 | Sign convention (§10.2): PV output ≥ 0 (export), load ≤ 0 (consumption negative, `P > 0` export at the point of connection); validation rejects sign violations with named errors. No price data anywhere in the fixture (D1/D2 OPEN); forecasts are never presented as measurements or market prices. | `validate_forecast_input` |
| D-FX-8 | Four-place synthetic labelling: places 1–2 (schema + type) are here — fixture-level `provenance: synthetic` plus per-series labels in the artefact, and in code values are ONLY accessible through `LabeledForecastSeries::values`, which returns `LabeledSeriesValues` carrying the label, version, and quantity (no bare-value accessor exists). Place 3 is `validate_forecast_input` (rejects unlabelled → `MissingProvenanceLabel`, unversioned → `MissingForecastVersion`); P3.12 calls it. Place 4 (result record) is P3.21's obligation and must reference the forecast version, never bare values. | `forecast_fixture.rs`; place 4 recorded as downstream obligation |
| D-FX-9 | **ForecastPublished payload gap (recorded honestly):** the frozen `ForecastPublishedPayload` (P0.14/P1.09) carries correlation/tenant/portfolio/site/asset + event_time only — NO forecast version and NO provenance field. flexcore-events was frozen for this subtask, so the version + provenance travel (a) in the envelope's idempotency key, deterministic from the forecast version: `forecast_published:<version>:<provenance>` (republication of one forecast version dedupes durably — a deliberate deviation from the default correlation-based key, recorded) and (b) in the fixture store/artefact. **Needs extension:** the recorded extension path is the P2.16 precedent — additive `serde(default)` payload fields (e.g. `forecast_version`, `provenance`) without a schema-version bump — for a future subtask that owns flexcore-events. Until then, consumers that need the version from the wire must read the fixture store referenced by the event's scope and key. | `forecast_publication`; flexcore-events untouched |
| D-FX-10 | The emission is a PURE assembly (`forecast_publication` → envelope + subject `flexcore.v1.forecast.forecast_published` + idempotency key). The wiring into the transactional outbox (state-change receipt, encoder, `EventOutbox` port) lands with P3.12/P3.28 via `event_assembly`. No ForecastPublished is emitted by this module at runtime. | `forecast_fixture.rs` |
| D-FX-11 | The application crate cannot parse JSON (manifest policy: no serialization dependency), so the generator transcribes the scenario's forecast-relevant constants; the integration test loads `scenario-v2.json` itself and asserts the transcribed values appear verbatim in the artefact (drift = test failure). Reading (parsing) committed fixtures into typed form is a composition-root concern (it owns serde); the crate owns the schema, the writer, and the validation. | `tests/forecast_fixture.rs` (`the_transcribed_scenario_constants_appear_verbatim_in_scenario_v2`) |

## 2. RECORDED-OUTPUT / VERIFY-BUILD (honesty over assertion)

The committed JSON artefacts were **not** produced by running the Rust
generator (cargo was not run in the authoring session — no build, check, or
test). They were produced by replicating the generator's algorithm exactly
(same RNG primitives, same shape formulas, same op order, same quantization,
same writer layout) outside cargo, and are marked **RECORDED-OUTPUT** with
the generation method recorded in the scenario README.

**VERIFY-BUILD obligations on the first build** (`cargo test -p
flexcore-application --test forecast_fixture`):

- `committed_central_artefact_matches_generator` — byte equality of
  `generate_central_fixture().to_json()` and the committed `v1` file.
- `committed_error_variant_artefact_matches_generator` — the same for the
  error variant.
- The one platform-dependent piece is the trigonometric shape evaluation
  (`cos`); a libm 1-ulp difference large enough to cross a milli-quantization
  boundary (probability ≈ 10⁻¹⁰ per value) would surface as a visible test
  failure, not silent drift. If it fires, regenerate the artefacts with the
  built generator and re-commit, updating this note.

RNG and quantization behaviour is pinned independently of the artefacts by
the recorded test vectors (`rng_matches_the_recorded_p2_14_vectors`), so the
duplicated P2.14 primitives cannot drift from the simulator.

## 3. Gaps and limitations

- **Payload extension needed** (D-FX-9): `ForecastPublished` cannot carry the
  version + provenance on the wire until flexcore-events adds them additively.
- **No fixture reader in the crate** (D-FX-11): the composition root's
  adapter parses the committed JSON and constructs through
  `validate_forecast_input`; until that adapter exists, the fixtures are
  consumed via the generator (which is byte-identical to the artefacts once
  VERIFY-BUILD passes).
- The availability forecast is the constant 1.0 (scenario assumption); a
  varying availability forecast is a future fixture version, not a code
  change to the validation (the schema already carries per-interval
  availability).
- Handover note (P3.30) must list the fixture versions, horizon, provenance
  label, and the place-4 result-record obligation (ledger P3.31 acceptance).

## 4. Verification status

- `python tools/verify_docs.py` / `validate_agent.py`: not applicable to this
  code-only change (no report edit); the ledger checkbox for P3.31 remains
  unchecked — evidence is the byte-equality test run recorded under
  VERIFY-BUILD above plus the listed files.
- Not run in this session: `cargo test` (authoring constraint). All claimed
  behaviour is pinned by the committed tests; nothing above is asserted as
  executed.
