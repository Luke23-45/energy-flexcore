# Phase 15 — Forecasting and Asset Intelligence — Context

Date: 2026-11-14. Status: **NOT STARTED** (0/20 — see `ledger_phase_15.md`).
This directory is PLANNING ONLY: no code, crates, or manifests are created by
this program's planning step (program conventions §3).

## 1. Why this phase exists

The commercial research (gap analysis §3, Track C) lists forecasting among the
features no enterprise VPP platform sells without, and the asset-tech research
(gap analysis §7, Track F2) lists the BESS health, degradation, and warranty
capabilities that make storage revenue defensible. FlexCore has neither.
Specific findings, each with its source:

| # | Finding | Source / evidence |
|---|---|---|
| 1 | Forecasting is absent. The only forecast object in the repository is the P3.31 synthetic labeled fixture; the gap analysis verdict is "no forecasting engine". Commercial platforms sell this as table stakes: GE's forecasting model library [S-41], Tesla Autobidder/Opticaster 15-minute price forecasts [S-50], GridBeyond price + consumption forecasts [S-51] | gap analysis §3 row 3; §2 |
| 2 | Asset intelligence is absent. `crates/flexcore-domain/src/bess_model.rs` is power/energy/SOC only; its P3.06 degradation treatment is a linear throughput cost term carrying explicit placeholder metadata ("no verified degradation model is on record") — no calendar/cycle aging, no counters, no rainflow. Keyword verification (this context, 2026-11-14): "warranty" — zero hits in `crates/`, `apps/`, `plugins/`; "digital twin" — zero hits; SoH appears only as an explicit absence (`crates/flexcore-simulator/src/profiles.rs`: "no temperature, degradation, or state-of-health model") | gap analysis §7 F2; grep verification recorded here |
| 3 | Phase 16 CONSUMES this phase. M&V/baselining (row 7) and market bids (row 6) both consume forecasts; the dependency graph names P15 forecasts as a P16 input, and the §6 market build-list is forecast-fed | gap analysis §3 rows 3/6/7; §6; §9 |
| 4 | Phase 17 CONSUMES this phase. EV departure/SoC intelligence is named as a P15 output in the dependency graph | gap analysis §9 |
| 5 | Degradation-aware dispatch is how operators protect revenue against wear: BESS revenue must be read net of cycle cost [S-149], and Megapack-class warranty economics price throughput explicitly [S-150] | [S-149][S-150]; gap analysis §7 F2 |
| 6 | SoH/warranty capture supports claims: capacity-retention schedules, cycle/throughput limits, augmentation events are the reference warranty structures (Megapack CMA and Fluence Dispatchable Energy Guarantee class) | [S-150][S-151][S-152][S-153]; gap analysis §7 F2 |
| 7 | Fleet health scoring — deviation-from-normal models, alarm normalization into prioritized issues, availability KPI independent of O&M (the Nispera pattern [S-53]) — and telemetry-drift detection are absent | [S-53]; gap analysis §7 F2 |
| 8 | Interconnection records are absent: UL 1741 SB is NOT claimable by a platform — certified equipment must be ingested and listed (UL 1741 SB COC, IEEE 1547.1-2020 test reports), and there is no document store | [S-21][S-169][S-170]; gap analysis §4 (UL 1741 SB row → Phase 15), §7 F2 |

## 2. What exists today (the real seams)

All paths below exist in the Phase 0–10 workspace, which is WRITTEN-UNRUN —
written modules with records, not verified behavior.

| Seam | Path | What it gives Phase 15 |
|---|---|---|
| Synthetic forecast fixture | `crates/flexcore-application/src/forecast_fixture.rs` (+ `scenarios/synthetic-portfolio/forecast-fixture-v1.json`, `forecast-fixture-v1-error-pv-ramp.json`; record `ledger/phase-03-flexibility-scheduling/31_forecast_fixture.md`) | P3.31: the single forecast source for v0.2.0 — per-asset per-interval PV/load/availability values over the P3.03 grid, four-place labeling (schema + type + envelope-input validation + result-record obligation), no bare-value accessor, no price data (D1/D2 OPEN). The labeling/provenance pattern every real forecast artifact (P15.01/P15.06) must extend |
| BESS domain model | `crates/flexcore-domain/src/bess_model.rs` | P3.05–07: power/energy/SOC balance, SOC↔energy window mapping, convex-relaxation path with the mandatory simultaneous charge/discharge detector, reserve SOC floor, and the P3.06 linear throughput degradation term with `DegradationMetadata` placeholder — the exact point P15.07 (counters, histograms, SoH observations) and P15.08 (real cost curves) extend |
| Flexible load model | `crates/flexcore-domain/src/flexible_load_model.rs` | P3.10: baseline/reducible/increasable/response-time/duration-recovery shape; the scenario-declared synthetic-baseline discipline (`SYNTHETIC_BASELINE_DISCLAIMER`) — the honesty pattern that statistical baselines (P15.02) must supersede with evaluated ones, never silently |
| Problem IR | `crates/flexcore-optimization-api/src/problem.rs` | P3.17: solver-neutral IR with `ProblemClass` (LP/QP/MILP/SOCP/MISOCP) and Tier-2 SOCP naming plus class-mismatch refusals — the guard P15.08 extends with tests proving degradation data never changes the declared class |
| Scheduling objective | `crates/flexcore-application/src/scheduling_objective.rs` | P3.14/15: the closed five-term objective vocabulary (`C_energy`, `C_degradation`, `C_deviation`, `C_curtailment`, `C_service`), `SYNTHETIC_ENERGY_COST_DEVIATION_V1`, schema-version-bump-only extension — where degradation cost enters as term DATA, never a new term or problem class |
| Capability provenance | `crates/flexcore-domain/src/capability_provenance.rs` | P3.02: MEASURED/CONFIGURED/INFERRED/UNKNOWN; unknown narrows, never widens — the provenance discipline the degradation counters and SoH observations (P15.07/P15.09) inherit |
| Envelope staleness | `crates/flexcore-application/src/envelope_staleness.rs` | P3.13: event-driven invalidation with the `ForecastPublished` trigger, horizon rollover, configured maximum-age bound, stale-never-used / superseded-version gates — the staleness rules P15.16 extends for real forecasts |
| Artifact signing | `crates/flexcore-research/src/artifact_signing.rs` | P8.32: versioned provenance record binding artifact ref, digest, digest algorithm, signer key REFERENCE, with training/config/framework provenance LINKED TO THE SIGNATURE — the pattern forecast and model artifacts mirror (P15.01/P15.06) |
| Baseline registry | `crates/flexcore-research/src/baselines.rs` | P8.06: frozen, version-stamped registry of deterministic baselines frozen BEFORE any comparison (no moving the baseline after seeing numbers) — the determinism pattern for persistence baselines (P15.02) and frozen eval sets (P15.05) |
| Guarded Python worker | `apps/worker/python/gnn_worker/` | The guarded-deps pattern: torch/PyG imports INSIDE functions raising named DECISION-POINT errors; the module parses, imports, and self-tests with nothing installed; one JSON request on stdin → one result envelope on stdout; malformed input → NOTHING on stdout; proposal-only output (no command reach) — the pattern P15.02/P15.17 follow |
| Training-data manifest | `apps/worker/python/training-data-manifest-v1.json` (record `ledger/phase-08-advanced-coordination/35_training_data_provenance.md`) | P8.35: family assignment at manifest time, every per-instance row EMPTY until the human generation gate, fail-closed scope checks — the discipline P15.18 extends to manifest v2 |
| Frozen benchmark set | `benchmarks/research-set-v1.json` | P8.19: frozen artifact, every run_status NOT_RUN, anti-silent-growth rule (a change is a NEW version), X-8 metric vocabulary without numeric thresholds — the eval-set shape P15.05 mirrors |
| Forecast plugin manifest | `plugins/manifests/forecast-worker.manifest.json` | The existing `forecast.publish` / `telemetry.read` permission classes and the grpc-worker shape — evidence the permission vocabulary already anticipates forecast producers with no dispatch reach (ADR-010, conventions §6.3) |
| Telemetry source contract | `crates/flexcore-ports/src/telemetry_source_contract.rs` (+ `telemetry_source.rs`) | The P2.x `TelemetrySource` port and contract — the integration point P15.13 (drift detection) and P15.09 (SoH ingestion) bind to, never a new parallel ingestion path |
| Canonical units | `crates/flexcore-contracts/src/units.rs` | P0.07/P1.05 freeze: MW/MVAr/MVA/MWh, P > 0 = export, reactive sign declared per model/profile, SOC a fraction in [0,1] — binding for every forecast value and degradation counter |

## 3. Constraints

1. **ADR-010 extension** (program conventions §6.3): forecasting, degradation
   pricing, and health scoring produce INPUTS, RECORDS, and PROPOSALS — they
   never authorize or produce dispatch commands. The `forecast-worker`
   manifest's permission set (`telemetry.read`, `forecast.publish` — no
   dispatch permission) is the shape every new producer inherits; the
   never-granted permission set stays byte-identical (conventions §6.4).
2. **Honesty about ML**: no model claims skill without evaluation runs. Every
   evaluation cell in this phase starts EMPTY; X-8 pre-registration applies
   (thresholds registered before any run that could fill them, never fitted
   after); "no-skill until proven otherwise against the persistence baseline"
   is the standing default (conventions §5; `benchmarks/research-set-v1.json`
   run_status discipline).
3. **Build-vs-buy**: weather/irradiance data and price forecasts are bought
   services per the DEFAULT recommendation [S-112 and gap analysis §7]; the
   registered vendor/token is a HUMAN GATE input. Recorded as P15-E-1 and
   P15-E-2; vendor-class names in this phase are RECOMMENDATION labels, not
   register sources (honesty rules §7.2).
4. **Statistical baselines first, ML later**: no data exists yet — the
   platform has never run — so persistence, seasonal-naive, and degree-day
   regression baselines are the only implementable forecasters until runtime
   data accumulates. ML models are designed, registered, and left UNTRAINED
   in this phase.
5. **Solver-class discipline** (conventions §6.5; AGENTS.md rule 4): degradation
   costs enter the optimizer only as linear/piecewise-linear DATA per asset
   (`C_degradation` term data), never as new problem classes; the P3.18–P3.20
   tier guards and MISOCP refusals stay, and P15.08 adds guard tests proving it.
6. **Production data stays out of simulation**: the training-data manifest
   discipline from Phases 8/9 (P8.35; `training-data-manifest-v1.json`) —
   dataset registration, EMPTY rows until a human gate, fail-closed scope —
   extends to manifest v2 (P15.18). No production telemetry ever trains a
   simulation-stage model.
7. **Loop bans and the WRITTEN-UNRUN base** (conventions §1; README hard
   status boundary 1): no installs, builds, or test runs; the first human
   build of the Phase 0–10 workspace gates all implementation; every plan
   here assumes that base compiles.

## 4. Glossary

| Term | Meaning as used in this phase |
|---|---|
| Forecast horizon | The lead time a forecast covers; this phase distinguishes day-ahead (DA) and intra-day horizons per artifact |
| Skill score | Forecast accuracy relative to a reference baseline (here: persistence); a model has skill only when the score beats the baseline on a frozen eval set — EMPTY until an X-8-gated run |
| Backtesting | Evaluating a forecaster on historical windows it did not see, with the eval protocol frozen before the run (P8.06 discipline applied to time series) |
| Persistence baseline | next(t) = obs(t); the deterministic reference every load/PV model must beat to claim skill |
| Seasonal-naive baseline | next(t) = obs(t − one seasonal period); the second reference baseline for daily/weekly seasonality |
| Degree-day regression | Linear regression of load on heating/cooling degree days — a statistical, auditable weather-sensitive baseline |
| Model registry | The versioned store binding model/artifact identity, digests, training provenance, and lineage (P8.32 pattern; P15.06) |
| Calendar aging | Capacity/impedance fade driven by time, temperature, and SoC dwell — independent of cycling [S-145] |
| Cycle aging | Fade driven on charge/discharge throughput and depth — parameterized per chemistry [S-146][S-147] |
| DOD | Depth of discharge — the fraction of usable energy cycled in one event; rainflow-counted into DOD × mean-SOC histograms |
| FEC | Full equivalent cycles — cumulative throughput normalized to full cycles; a monotonic counter (P15.07) |
| Rainflow counting | The standard method turning an SOC time series into closed half-cycle amplitude/mean pairs — the histogram input of cycle-aging models [S-146][S-147] |
| SoH | State of health — remaining capacity relative to nameplate; BMS-reported values are ingested with provenance (P15.09); platform-side estimation is DEFERRED |
| Capacity-retention schedule | Warranty term: minimum guaranteed capacity retention vs time/throughput (e.g., x% at year n) [S-150][S-151] |
| Throughput warranty | Warranty term: maximum MWh/cycles before coverage terms change [S-152][S-153] |
| Augmentation | Adding capacity to restore a degraded system within warranty terms; recorded as events in the warranty object (P15.10) |
| Deviation-from-normal scoring | Health scoring against the asset's own learned/declared normal envelope, not fleet averages — the Nispera pattern [S-53] |
| Telemetry drift | Gradual divergence of reported values/rates from plausibility bounds or reporting-rate expectations — a twin-lite signal, not grid state estimation (gap analysis §7) |

## 5. Boundaries — what this phase does NOT do

1. **No dispatch authority of any kind** — ADR-010 extends, never relaxes
   (conventions §6.3); the release ladder and simulation/shadow ceiling are
   untouched (conventions §6.7).
2. **No ML training or evaluation run** — designs, registries, and EMPTY
   evaluation cells only; training/evaluation are X-8-gated human/lab runs.
3. **No market bidding** — Phase 16 consumes forecasts; this phase produces
   price-signal INGESTION and vendor-forecast seams, not bids (ADR-011
   boundary unchanged).
4. **No platform-side SoH estimation, no grid state estimation** — SoH is
   BMS-reported with provenance (P15.09); drift/twin-lite work (P15.13/P15.14)
   stays asset-local, explicitly not grid state estimation (gap analysis §7).
5. **No new problem classes** — degradation data enters the existing IR as
   linear/piecewise-linear term data (constraint 5); no MISOCP/MINLP creep.
