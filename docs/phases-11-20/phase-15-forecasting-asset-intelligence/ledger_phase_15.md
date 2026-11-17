# Phase 15 — Forecasting and Asset Intelligence — Ledger

Date: 2026-11-14. Status: **IMPLEMENTED — awaiting human build/test** — 20 of 20 subtasks ticked. Phase close: HANDOVER.md, CHECKLIST.md, records/62_p15_20_phase_close.md. No forecast produced, no skill claimed, every evaluation cell EMPTY; the human-gate queue is in CHECKLIST.md.

**Gate.** No implementation starts before (a) human review of this roadmap
and (b) the first human build of the Phase 0–10 workspace is green (program
conventions §3; program README, hard status boundary 1). Every run, token,
vendor account, sign-off, threshold, and date is a HUMAN GATE INPUT recorded
UNRUN / EMPTY / UNSIGNED; forecasts, degradation prices, and health scores
are INPUTS/RECORDS per the ADR-010 extension — never dispatch commands.

**Tick convention (carried verbatim from Phases 0–10).** A checkbox is
ticked only as "implemented, awaiting human build/test" with a
decision-record citation — `- [x] ... (decision record:
\`records/NN_slug.md\`)` — never as "tested", "verified", "passed", or
"works". Tick flips use the exact-match count==1 method or Write/Edit;
decision records are created at implementation time under `records/`.

## Subtask register

- [x] **P15.01 — Forecasting architecture record: forecasts-as-inputs (decision record: `records/01_p15_01_forecasting_architecture.md`)
      boundary.** Artifact types (load/PV/price, site/portfolio), horizons
      (DA/intra-day), provenance + signing per P8.32. Evidence: the record
      cites the artifact types, horizons, permissions; no command path.
- [x] **P15.02 — Load forecasting service design (statistical baselines (decision record: `records/02_p15_02_load_forecasting_baselines.md`)
      first).** Persistence, seasonal-naive, degree-day baselines; no
      skill claimed until evaluation runs; guarded-deps worker skeleton.
      Evidence: the record cites the baselines and freeze discipline.
- [x] **P15.03 — PV generation forecasting design and weather-data (decision record: `records/03_p15_03_pv_weather_seam.md`)
      ingestion seam.** A weather port designed; build-vs-buy recorded as
      P15-E-1 (DEFAULT: buy, ingested as data with provenance). Evidence:
      the record cites the port shape, the E-decision dependence, and the
      provenance requirement; nothing lands un-decided.
- [x] **P15.04 — Price-signal ingestion design (ENTSO-E Transparency (decision record: `records/04_p15_04_price_ingestion_entsoe.md`)
      Platform + vendor seams).** A44 day-ahead REST adapter design
      [S-111]; token is a HUMAN GATE input; vendor seams as P15-E-2
      [S-112]. Evidence: the record cites the adapter design and the
      UNSIGNED token status; prices are data, never bids.
- [x] **P15.05 — Forecast evaluation framework (X-8 extension).** MAE / (decision record: `records/05_p15_05_forecast_evaluation.md`)
      RMSE / skill-vs-persistence metrics, backtesting protocol, frozen
      eval sets. Evidence: the record cites the metrics, the frozen
      eval-set versions, and the EMPTY tables; no cell is filled.
- [x] **P15.06 — Model registry and forecast artifact versioning.** (decision record: `records/06_p15_06_model_registry.md`)
      Versioned, digest-chained artifacts with model→artifact lineage and
      replay determinism per flexcore-research. Evidence: the record cites
      the versioning rules and lineage fields; changes are new versions,
      never silent edits.
- [x] **P15.07 — BESS degradation data model (domain extension).** (decision record: `records/21_p15_07_bess_degradation_data_model.md`)
      Monotonic counters (cumulative discharge MWh, FEC), rainflow
      DOD × mean-SOC histogram, temperature exposure, high-SOC dwell, SoH
      observations with provenance [S-145][S-146][S-147]. Evidence: the
      record cites the counters and histograms; unknown narrows.
- [x] **P15.08 — Degradation cost curve into the optimizer.** $/MWh (decision record: `records/22_p15_08_degradation_cost_curve.md`)
      throughput penalty or piecewise-linear aging cost as optimizer
      INPUT data per asset [S-148]; guard tests prove MISOCP refusal
      stays. Evidence: the record cites the data contract extension and
      the guard tests (written, UNRUN); no new problem class.
- [x] **P15.09 — SoH ingestion (BMS-reported, provenance + staleness).** (decision record: `records/23_p15_09_soh_ingestion.md`)
      BMS-reported SoH with provenance and staleness rules; platform-side
      estimation DEFERRED and recorded. Evidence: the record cites the
      staleness bounds and the deferral; methods surveyed, none
      implemented.
- [x] **P15.10 — Warranty object model.** Capacity-retention schedule, (decision record: `records/24_p15_10_warranty_object_model.md`)
      cycle/throughput limits, RTE/availability commitments, augmentation
      events, warranty-headroom computation [S-150][S-151][S-152][S-153].
      Evidence: the record cites the object fields and headroom rule;
      reference structures cited, no vendor conformance claimed.
- [x] **P15.11 — Warranty evidence-pack generator.** Counters → (decision record: `records/25_p15_11_warranty_evidence_pack.md`)
      timestamped, digest-chained claim-SUPPORT pack export. Evidence:
      the record cites the export structure and digest chain; the pack
      supports human claims, it never asserts them.
- [x] **P15.12 — Fleet health scoring.** Deviation-from-normal models per (decision record: `records/41_p15_12_fleet_health_scoring.md`)
      asset class, alarm normalization, availability KPI independent of
      O&M (Nispera pattern [S-53]). Evidence: the record cites the
      scoring classes and KPIs; scores are records, not dispatch inputs.
- [x] **P15.13 — Telemetry-drift detection.** Reporting-rate expectations (decision record: `records/42_p15_13_telemetry_drift_detection.md`)
      + value plausibility bounds per asset class (twin-lite); binds to
      the Phase-2 telemetry source contract. Evidence: the record cites
      the expectation/bound tables and the integration point; no parallel
      ingestion path is created.
- [x] **P15.14 — Per-asset calibrated model records.** Thermal/efficiency (decision record: `records/43_p15_14_calibrated_model_records.md`)
      model records with provenance-declared parameters + regression test
      fixtures written complete and UNRUN. Evidence: the record cites the
      record fields and the fixture paths; no parameter is fitted from
      data.
- [x] **P15.15 — Interconnection-record ingestion.** UL 1741 SB COC, (decision record: `records/44_p15_15_interconnection_record_ingestion.md`)
      IEEE 1547.1-2020 test reports, certified function category,
      settings profile per device model; document store with model-number
      linkage [S-21][S-169][S-170]. Evidence: the record cites the store
      fields; the platform lists certified equipment, never claims it.
- [x] **P15.16 — Forecast-to-schedule integration.** Envelopes/rolling (decision record: `records/45_p15_16_forecast_to_schedule_integration.md`)
      horizon consume real forecasts; staleness rules extended from
      `envelope_staleness`, persistence-baseline fallback. Evidence: the
      record cites the triggers and fallback; fixture stays labeled.
- [x] **P15.17 — Python forecasting worker skeleton.** Guarded deps, (decision record: `records/46_p15_17_forecast_worker_completion.md`)
      proposal/record outputs only (no dispatch reach), stdout contract
      mirroring `gnn_worker`. Evidence: the record cites the guard
      functions, the stdout contract, and the written-unrun self-tests.
- [x] **P15.18 — Evaluation datasets policy (training-data manifest v2).** (decision record: `records/47_p15_18_evaluation_datasets_policy.md`)
      Dataset registration discipline, no production data in simulation,
      EMPTY rows until the human generation gate, fail-closed scope.
      Evidence: the record cites the v2 schema and fail-closed rules; no
      production dataset is referenced anywhere.
- [x] **P15.19 — Decision records.** P15-E-1 (weather data build-vs-buy), (decision record: `records/61_p15_19_decision_records.md`)
      P15-E-2 (price forecast vendor), P15-E-3 (degradation chemistry
      coverage — LFP/NMC first per BLAST-Lite [S-147]) recorded OPEN with
      DEFAULTs. Evidence: the record cites the three E-decision entries;
      no E-decision is closed by an agent.
- [x] **P15.20 — Phase close and consistency audit.** HANDOVER, CHECKLIST, (decision record: `records/62_p15_20_phase_close.md`)
      close record per conventions §2.5; consistency audit. Evidence: the
      record cites the audit findings (citations resolve, units canonical,
      no skill claims, no dispatch reach) and the close-record paths.

## E-decision register (all OPEN — agents never close; see `plan.md` §4)

| ID | Question | DEFAULT (RECOMMENDATION) | What closes it |
|---|---|---|---|
| P15-E-1 | Weather/irradiance data: build vs buy | Buy from a commercial vendor, ingested as data with provenance | Human owner's budget + vendor selection (UNSIGNED account) |
| P15-E-2 | Price-forecast vendor seam | Amperon-class vendor forecasts ingested as data [S-112] | Human owner's vendor selection + jurisdiction register |
| P15-E-3 | Degradation-model chemistry coverage | LFP and NMC first per BLAST-Lite [S-147] | Human owner's fleet-composition review; per-chemistry parameters are human-gate inputs |
