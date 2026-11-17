# Phase 15 — Forecasting and Asset Intelligence — Plan

Date: 2026-11-14. Status: **NOT STARTED** (0/20). Planning baseline only.
Ledger: `ledger_phase_15.md` (all boxes unticked). Conventions:
`../01_PROGRAM_CONVENTIONS.md` (binding). Context: `context.md`.

## 1. Scope statement

Phase 15 closes the two table-stakes gaps the gap analysis assigns to this
phase: forecasting (load/generation/price ingestion — commercial row 3,
§3) and asset intelligence (BESS degradation, warranty, health scoring,
drift detection, interconnection records — §7 Track F2 build-list). It is
the declared dependency of Phase 16 (M&V and market bids consume forecasts)
and Phase 17 (departure/SoC intelligence), per the §9 dependency graph.
Everything lands as INPUTS, RECORDS, and PROPOSALS per the ADR-010 extension
(conventions §6.3): no forecast, degradation price, or health score ever
authorizes or produces a dispatch command. Statistical baselines come first
and ML stays UNTRAINED (no runtime data exists); degradation costs enter the
optimizer only as linear/piecewise-linear data (conventions §6.5); the
release ladder, solver tiers, and the never-granted permission set are
untouched.

## 2. Subtask outline

| ID | Title | Summary | Key seams/files |
|---|---|---|---|
| P15.01 | Forecasting architecture record: forecasts-as-inputs boundary | The ADR-010 extension as an architecture record: forecast artifact types (load/PV/price, per site and per portfolio), horizons (DA/intra-day), provenance + signing pattern mirroring `crates/flexcore-research/src/artifact_signing.rs` (P8.32 provenance-record shape with provenance linked to the signature); `forecast.publish`/`telemetry.read` permission classes from `plugins/manifests/forecast-worker.manifest.json`, no dispatch permission | [S-41][S-50][S-51]; `crates/flexcore-research/src/artifact_signing.rs`, `crates/flexcore-application/src/forecast_fixture.rs` (labeling pattern) |
| P15.02 | Load forecasting service design (statistical baselines first) | Persistence, seasonal-naive, and degree-day regression baselines designed as deterministic, registry-frozen references; EXPLICIT no-skill claims until evaluation runs (P15.05); Python worker skeleton following the guarded-deps pattern; the P3.10 flexible-load baseline honesty (`SYNTHETIC_BASELINE_DISCLAIMER`) superseded only by evaluated baselines | `crates/flexcore-domain/src/flexible_load_model.rs`, `crates/flexcore-research/src/baselines.rs` (P8.06 freeze discipline), `apps/worker/python/gnn_worker/` (guarded-deps pattern) |
| P15.03 | PV generation forecasting design + weather-data ingestion seam | A weather-data ingestion PORT (build-vs-buy P15-E-1: DEFAULT buy irradiance/power from a commercial vendor, ingested as DATA with provenance; Solcast/DNV-class naming is a RECOMMENDATION label, not a register source — the analogous bought-forecast pattern is [S-112]; forecasting is table stakes per gap analysis §3 row 3, weather an input seam per §7); design only — no vendor integration lands without the E-decision | gap analysis §3 row 3, §7; [S-112] (analogy); new port under `crates/flexcore-ports/src/`; P15-E-1 |
| P15.04 | Price-signal ingestion design (ENTSO-E Transparency Platform + vendor seams) | ENTSO-E Transparency Platform REST adapter DESIGN: A44 day-ahead price documents [S-111]; the registered API token is a HUMAN GATE input (UNSIGNED until a human registers); vendor forecast seams (Amperon-class) as P15-E-2 [S-112]; prices are INGESTED DATA for later phases, never bid inputs here | [S-111][S-112]; new adapter under `crates/adapters/` (proposed); P15-E-2 |
| P15.05 | Forecast evaluation framework (X-8 extension) | X-8 pre-registration extended to forecasting: metrics (MAE, RMSE, skill score vs the persistence baseline), backtesting protocol, frozen eval sets mirroring `benchmarks/research-set-v1.json` (anti-silent-growth: a change is a NEW version); every result cell EMPTY; no skill claimed anywhere | [S-41]; `benchmarks/research-set-v1.json` (P8.19 shape), `crates/flexcore-research/src/baselines.rs`, `ledger/phase-08-advanced-coordination/` X-8 discipline |
| P15.06 | Model registry and forecast artifact versioning | Forecast artifacts and models versioned + digest-chained; model→artifact lineage; replay determinism mirroring `crates/flexcore-research/` patterns (P8.32 signing record shape, P8.06 version-stamped frozen registries); a registry change is a new version, never a silent edit | `crates/flexcore-research/src/artifact_signing.rs`, `crates/flexcore-research/src/baselines.rs`, `benchmarks/research-set-v1.json` |
| P15.07 | BESS degradation data model (domain extension) | `flexcore-domain` extension alongside `bess_model.rs`: monotonic counters (cumulative discharge MWh, FEC), rainflow DOD × mean-SOC histogram, temperature-exposure distribution, high-SOC dwell accumulation, periodic SoH observations with provenance — all carrying `capability_provenance` discipline (unknown narrows, never widens) | [S-145][S-146][S-147]; `crates/flexcore-domain/src/bess_model.rs`, `crates/flexcore-domain/src/capability_provenance.rs`, `crates/flexcore-contracts/src/units.rs` |
| P15.08 | Degradation cost curve into the optimizer | $/MWh throughput penalty or piecewise-linear cycle-aging cost as optimizer INPUT DATA per asset, feeding the existing `C_degradation` term (P3.14 closed vocabulary — schema-version bump if the term shape changes, never a new class); solver-class guard tests written proving the MISOCP refusal and tier guards stay | [S-148]; `crates/flexcore-optimization-api/src/problem.rs`, `crates/flexcore-application/src/scheduling_objective.rs`, `crates/flexcore-domain/src/bess_model.rs` (P3.06 `DegradationConfig` placeholder superseded as data) |
| P15.09 | SoH ingestion (BMS-reported, provenance + staleness) | BMS-reported SoH ingested with provenance and staleness rules (max-age bounds per the envelope-staleness pattern); platform-side SoH estimation DEFERRED and recorded as such; estimation methods surveyed, none implemented | [S-145]; `crates/flexcore-application/src/envelope_staleness.rs`, `crates/flexcore-ports/src/telemetry_source_contract.rs` |
| P15.10 | Warranty object model | Capacity-retention schedule, cycle/throughput limits, RTE/availability commitments, augmentation events; warranty-headroom computation (remaining allowance vs consumed counters from P15.07) — Megapack CMA and Fluence Dispatchable Energy Guarantee as reference STRUCTURES, no vendor conformance claims | [S-150][S-151][S-152][S-153]; `crates/flexcore-domain/src/` (new warranty module, proposed) |
| P15.11 | Warranty evidence-pack generator | Counters (P15.07) → timestamped, digest-chained claim-SUPPORT pack export; supports human warranty claims, never asserts them; export carries the artifact-signing provenance record shape | `crates/flexcore-research/src/artifact_signing.rs` (P8.32 pattern); `crates/flexcore-domain/src/` warranty module (P15.10) |
| P15.12 | Fleet health scoring | Deviation-from-normal models per asset class, alarm normalization into prioritized issues, availability KPI computation independent of O&M (Nispera pattern [S-53]); scores are RECORDS feeding operator views — never dispatch inputs | [S-53]; `crates/flexcore-domain/src/` + `crates/flexcore-application/src/` (new modules, proposed); telemetry contract |
| P15.13 | Telemetry-drift detection | Reporting-rate expectations + value plausibility bounds per asset class (twin-lite per gap analysis §7 — explicitly not grid state estimation); integration point = the Phase-2 telemetry source contract, no parallel ingestion path | gap analysis §7; `crates/flexcore-ports/src/telemetry_source_contract.rs`, `apps/worker/src/ingestion.rs` |
| P15.14 | Per-asset calibrated model records | Thermal/efficiency model RECORDS per asset (declared parameters with provenance, never fitted claims) + regression test fixtures written complete and UNRUN | `crates/flexcore-simulator/src/profiles.rs` (the recorded "no temperature, degradation, or state-of-health model" absence — superseded as recorded parameters) |
| P15.15 | Interconnection-record ingestion | UL 1741 SB COC, IEEE 1547.1-2020 test reports, certified function category, settings profile per device model; a document store with model-number linkage to the registry; the platform lists certified equipment — it never claims certification for itself [S-21][S-169][S-170] | [S-21][S-169][S-170]; `crates/flexcore-domain/src/` + registry seams (new document store, proposed) |
| P15.16 | Forecast-to-schedule integration | Flexibility envelopes and rolling-horizon scheduling consume real forecasts with staleness rules EXTENDED from `envelope_staleness.rs` (`ForecastPublished` trigger, horizon rollover, max-age bound); fallback to the persistence baseline (P15.02) on staleness — the fixture stays the labeled synthetic fallback source | `crates/flexcore-application/src/envelope_staleness.rs`, `crates/flexcore-application/src/flexibility_envelope.rs`, `rolling_horizon.rs`, `forecast_fixture.rs` |
| P15.17 | Python forecasting worker skeleton | Worker under `apps/worker/python/forecast_worker/`: guarded deps (imports inside functions, named DECISION-POINT errors), proposal/record outputs ONLY (no dispatch reach), stdout contract mirroring `gnn_worker` (one JSON request on stdin → one result envelope on stdout; malformed → NOTHING on stdout); self-tests runnable with nothing installed | [S-41]; `apps/worker/python/gnn_worker/` (pattern source), `plugins/manifests/forecast-worker.manifest.json` (permission classes) |
| P15.18 | Evaluation datasets policy (training-data manifest v2) | Training-data manifest v2: dataset registration discipline, no production data in simulation (Phase 8/9 discipline carried), EMPTY rows until the human generation gate, fail-closed scope checks extended to forecasting datasets | `apps/worker/python/training-data-manifest-v1.json` (v1 shape + `ledger/phase-08-advanced-coordination/35_training_data_provenance.md`) |
| P15.19 | Decision records | P15-E-1 (weather data build-vs-buy), P15-E-2 (price forecast vendor), P15-E-3 (degradation model chemistry coverage — LFP/NMC first per BLAST-Lite [S-147]) recorded OPEN with DEFAULTs; agents never close them | phase `records/` (created at implementation time); conventions §4 |
| P15.20 | Phase close and consistency audit | HANDOVER/CHECKLIST/close-record structure mirroring the Phase 0–10 conventions §2.5; consistency audit across the phase's records (citations resolve, units canonical, no skill claims, no dispatch reach) | phase directory; conventions §2.5 |

## 3. Implementation wave plan (2 agents)

| Wave | Agent A | Agent B |
|---|---|---|
| 1 | P15.01–P15.06 (forecasting core) | P15.07–P15.11 (BESS degradation/warranty) |
| 2 | P15.12–P15.18 (health/records/integration) | P15.19–P15.20 (decisions + close) |

Disjoint ownership:

- **Wave 1, Agent A** owns: the new forecast modules under
  `crates/flexcore-application/src/` (artifact types, evaluation framework),
  the new weather/price ports under `crates/flexcore-ports/src/`, the model
  registry design, and the new `apps/worker/python/forecast_worker/`
  skeleton directory.
- **Wave 1, Agent B** owns: `crates/flexcore-domain/src/` degradation and
  warranty extensions (P15.07, P15.10, P15.11), the optimizer data-contract
  wiring in `crates/flexcore-optimization-api/src/` and
  `crates/flexcore-application/src/scheduling_objective.rs` (P15.08), SoH
  ingestion (P15.09).
- The two wave-1 ownership sets intersect only at module-declaration files
  (`crates/flexcore-application/src/lib.rs`, `crates/flexcore-domain/src/lib.rs`)
  and at `crates/flexcore-contracts/src/units.rs` (read-only for both).
  Protocol: Agent B's domain-side change lands in its batch first; Agent A
  re-reads before editing and records the dependence in its decision record.
  No other shared files exist.
- **Wave 2, Agent A** owns: health scoring and drift detection modules
  (P15.12, P15.13), calibrated model records (P15.14), interconnection
  records (P15.15), the staleness/fallback integration (P15.16), the
  `forecast_worker/` completion (P15.17), and the datasets policy (P15.18).
- **Wave 2, Agent B** owns: the three E-decision records and the close
  records (P15.19, P15.20). No file overlap with Agent A in wave 2.

## 4. Decision points (registered OPEN — agents never close E-decisions)

### P15-E-1 — Weather/irradiance data: build vs buy

- **Question:** where do PV-forecast weather inputs come from?
- **Options:** (a) buy irradiance/power from a commercial weather-data vendor
  (Solcast/DNV-class naming is a RECOMMENDATION, not a register source);
  (b) ingest raw public weather feeds and model irradiance in-platform;
  (c) site-measured irradiance only, no forecast weather.
- **DEFAULT (RECOMMENDATION):** (a) — bought service ingested as DATA with
  full provenance; forecasting is table stakes (gap analysis §3 row 3) and
  the F2 build-list treats weather as an input seam (§7), not a build item.
- **What closes it:** the human owner's budget and vendor selection; the
  vendor account/API key is a HUMAN GATE input (UNSIGNED until registered).

### P15-E-2 — Price-forecast vendor seam

- **Question:** are day-ahead/intra-day PRICE forecasts ingested from a
  vendor in addition to the ENTSO-E Transparency historical/actual prices?
- **Options:** (a) Amperon-class vendor forecasts ingested as data [S-112];
  (b) ENTSO-E actuals only, no vendor forecasts; (c) in-platform price
  forecasting (explicitly premature — no data exists).
- **DEFAULT (RECOMMENDATION):** (a) alongside the [S-111] adapter; Phase 16
  consumes whatever lands; nothing here bids.
- **What closes it:** the human owner's vendor selection and market-jurisdiction
  register entry (program README hard status boundary 4; conventions §4).

### P15-E-3 — Degradation-model chemistry coverage

- **Question:** which cell chemistries does the P15.07/P15.08 degradation
  data model parameterize first?
- **Options:** (a) LFP and NMC first per BLAST-Lite's parameterized
  semi-empirical model coverage [S-147]; (b) LFP only; (c) all chemistries
  from the start.
- **DEFAULT (RECOMMENDATION):** (a) LFP + NMC; per-chemistry parameters are
  INPUTS declared with provenance, never universal laws [S-145][S-146].
- **What closes it:** the human owner's fleet-composition review; parameters
  per chemistry are human-gate inputs before any cost curve activates.

## 5. Risks

| # | Risk | Treatment |
|---|---|---|
| 1 | No runtime data exists — the platform has never run — so every forecast evaluation cell is EMPTY by construction and no baseline can be validated against reality | Statistical baselines are shipped as designed, frozen references with explicit no-skill claims; all evaluation cells stay EMPTY; X-8 pre-registration governs any future run (P15.05) |
| 2 | ML honesty: a registered model could be presented as skillful before any evaluation run | Skill claims are structurally impossible until the P15.05 framework produces a filled, X-8-gated result; every artifact carries provenance labels; the registry records training state (UNTRAINED is a first-class state) |
| 3 | Vendor dependency for weather and price data: a bought service can stall the pipeline or change terms | Data lands through a port with provenance; staleness rules (P15.16) and the persistence-baseline fallback keep scheduling functional when vendor data is missing or stale; vendor selection is an E-decision, never an agent act |
| 4 | Degradation modeling is chemistry-specific: BLAST-Lite coverage limits [S-147] mean parameters do not generalize across fleets | P15-E-3 scopes coverage (LFP/NMC first); parameters are per-chemistry inputs with provenance; out-of-scope chemistries are recorded limitations, never silently extrapolated |
| 5 | The 0–10 base is WRITTEN-UNRUN: extensions are written against code that has never executed; the first human build may surface recorded defects | Plans assume the base compiles (program README hard status boundary 1); records describe written code, never observed behavior; the first human build gates all implementation |

## 6. Acceptance criteria (honest)

1. Every subtask has a decision record under `phase-15-forecasting-asset-intelligence/records/`
   (created at implementation time) citing the actual files touched and the
   [S-n] sources for every external claim.
2. The deliverables are DESIGNS, domain-model specifications, and test
   fixtures WRITTEN-UNRUN. No forecast is ever produced by this program's
   implementation; no skill is claimed anywhere; every evaluation cell stays
   EMPTY; all models register as UNTRAINED.
3. The optimizer data contract is extended ONLY as input data
   (linear/piecewise-linear `C_degradation` term data); the solver-class
   guard tests written in P15.08 prove the MISOCP refusal and tier guards
   stay; no new problem classes exist.
4. No forecast, degradation price, or health score has dispatch reach
   (ADR-010 extension, conventions §6.3); the never-granted permission set
   is byte-identical before and after.
5. No production data enters simulation; the training-data manifest v2 keeps
   the EMPTY-until-human-gate discipline with fail-closed scope checks.
6. P15-E-1, P15-E-2, and P15-E-3 remain OPEN with DEFAULTs recorded;
   `python -I tools/check_dependency_rule.py` stays green after every
   implementation batch; `python tools/verify_docs.py` and
   `python tools/validate_agent.py` report no failures.

## 7. Execution notes for implementers

1. Decision records go under `phase-15-forecasting-asset-intelligence/records/`,
   created at implementation time (conventions §2.4), named in batch order
   per the 0–10 record conventions.
2. Evaluation thresholds, backtesting windows, and skill-score targets are
   registered BEFORE any run that could fill them (X-8; conventions §5);
   observed values live structurally separate from registered thresholds.
3. No new workspace crate is expected: ports land in `crates/flexcore-ports/`,
   domain extensions in `crates/flexcore-domain/`, application use cases in
   `crates/flexcore-application/`, the worker in `apps/worker/python/`, the
   document store in the existing adapter/postgres pattern. If an implementer
   finds a new crate justified, it follows conventions §6.2 with the additive
   `check_dependency_rule.py` update that requires.
4. Vendor data (weather, price forecasts) is ingested as DATA with provenance
   records; no vendor benchmark or accuracy claim is ever carried as a
   FlexCore claim (honesty rules §7.2; UNVERIFIED register discipline).
5. Every batch ends with the three permitted checks quoted verbatim in the
   batch's decision record (conventions §1.3, §6.1) — no "verified" without
   output (AGENTS.md §3 rule 7).
