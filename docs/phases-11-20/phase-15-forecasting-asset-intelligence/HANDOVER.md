# HANDOVER — Phase 15: Forecasting and Asset Intelligence

Status: **implementation complete (expected 20/20), everything
WRITTEN-UNRUN.** The ledger's P15.01–P15.11 ticks are applied (by the
orchestrator, citing records 01–06 and 21–25); the P15.12–P15.20 ticks
are the ORCHESTRATOR's action, applied after this close (program
conventions §2 — no agent ever ticks a checkbox), expected to cite
records 41–47 (wave 2, Agent A), 61 (wave 2, Agent B, the decision
register record), and the close record 62. The phase gate is **NOT
MET** as a runtime claim: no forecast is produced, no model is trained,
no evaluation cell is filled, no skill is claimed, no weather or price
data is ingested, no vendor is selected, no document is stored, no
migration is applied, no worker is executed, no score is computed from
real data — the first human build of the Phase 0–10 workspace is the
verification gate for every statement below.

## What was implemented (all WRITTEN, never run)

| Area | Subtasks | Records | Delivered (written) |
|---|---|---|---|
| Forecasting core | P15.01–P15.06 | 01, 02, 03, 04, 05, 06 | the forecasts-as-inputs architecture module (closed artifact/horizon/scope vocabularies, P8.32-mirrored provenance + fail-closed digest gate, producer permission vocabulary, never-granted mirror); the persistence / seasonal-naive / degree-day baselines as frozen registry references with explicit NO-SKILL statements + the guarded-deps Python worker skeleton; the weather-data ingestion PORT (P15-E-1 registered OPEN); the ENTSO-E A44 adapter crate (`flexcore-entsoe-adapter`, the 28th workspace member) with the UNSIGNED token gate, the unwired transport, and the price port (P15-E-2 registered OPEN); the X-8-extended evaluation framework with the frozen `forecast-eval-set-v1.json` (every cell EMPTY, `EvaluationRunGate` unconstructable in-program); the append-only digest-chained model registry with UNTRAINED as a first-class state |
| BESS degradation / warranty | P15.07–P15.11 | 21, 22, 23, 24, 25 | the degradation ledger domain module (monotonic counters, DOD × mean-SOC histogram, temperature exposure, high-SOC dwell, append-ordered SoH observations, weakest-provenance merge — P15-E-3 registered OPEN); the linear/piecewise-linear degradation cost-curve data contract feeding the existing `C_degradation` term as INPUT DATA (convexity gate, no new problem class) + four solver-class guard tests; BMS-reported SoH ingestion with staleness (platform-side estimation DEFERRED, methods surveyed NOT-IMPLEMENTED); the warranty object model with pure headroom computation (Megapack/Fluence-class reference STRUCTURES only); the digest-chained warranty evidence-pack generator (claim-SUPPORT, never a claim) |
| Health / records / integration | P15.12–P15.18 | 41, 42, 43, 44, 45, 46, 47 | fleet health scoring (deviation-from-normal, deterministic alarm normalization, O&M-independent availability KPI — scores are RECORDS); telemetry-drift detection (declared reporting-rate expectations + plausibility bounds, twin-lite, over the EXISTING telemetry port — no parallel ingestion); per-asset calibrated model RECORDS (declared thermal/efficiency parameters with provenance, never fitted); interconnection-record ingestion (UL 1741 SB COC / IEEE 1547.1-2020 report document store with exact model-number linkage — the platform LISTS certified equipment, never claims certification; migration 0021, never applied); forecast-to-schedule integration (source-order decision composing the P3.13 staleness functions, persistence fallback with carried reason, the fixture as the labeled synthetic fallback); the forecast-worker publication-record mode (additive; provenance links validated before computation; UNSIGNED); training-data manifest v2 (dataset registration, EMPTY rows, fail-closed scope, v1 carried BY REFERENCE) |
| Decisions + close | P15.19, P15.20 | 61, 62 | the consolidated E-decision register record (P15-E-1/E-2/E-3 verified OPEN with DEFAULTs; closers named); this HANDOVER + CHECKLIST + the close/exit-gate record with the consistency audit |

## Blockers carried (human gate inputs)

1. **The first human build** (`cargo build --workspace` → clippy →
   `cargo test --workspace`, plus the Python worker self-tests and the
   migration-0021 apply/down cycle): every Rust and Python artifact in
   this phase is written against the WRITTEN-UNRUN 0–10 base,
   including the new 28th workspace member, five new domain/application
   modules, the optimizer data-contract wiring, the storage trait and
   postgres adapter, and every written-unrun test file. Failures at
   that gate are recorded and fixed as code edits — never silently.
2. **Every evaluation cell stays EMPTY until an X-8-gated human/lab
   run**: the frozen eval set registers placeholder-freeze bounds (0.0
   = no threshold registered); the `EvaluationRunGate` constructor is
   minted by the human/lab runtime, never in this program's code
   (record 05 §3); observed values live structurally separate from
   registered thresholds.
3. **The three E-decisions are OPEN** (P15-E-1 record 03 §3, verified
   in 61 §1; P15-E-2 record 04 §3, verified in 61 §1; P15-E-3 record
   21 §4, verified in 61 §1) — agents never close them; the closers
   and the closing evidence are named in record 61 §2.
4. **No vendor data exists**: the weather vendor account, the ENTSO-E
   API token, and any price-forecast account are UNSIGNED human-gate
   inputs; no request is ever sent by this program (the A44 transport
   is an UNWIRED seam); no weather/price sample exists in the
   repository.
5. **Per-chemistry degradation parameters are human-gate inputs**: no
   cost curve is activated until P15-E-3 closes with a reviewed
   parameter set (record 22 §5 item 1 — every curve value is
   ASSUMPTION-grade until then).
6. **Production signing is a DEPLOYMENT REQUIREMENT**: the forecast
   artifacts, the model registry, and the warranty evidence packs all
   carry test-grade FNV-1a 64 digests explicitly labelled NOT security
   primitives; production SHA-256 digests + managed signing identities
   are pre-registered requirements (records 01 §1, 06 §1, 11 §1, 25 §1,
   46 §1).

## Decisions still needed (the closers)

- **P15-E-1 (weather/irradiance data build-vs-buy)**: the human
  owner's budget and vendor selection; the vendor account/API key is
  UNSIGNED until registered. Closes the weather port's first real
  implementation.
- **P15-E-2 (price-forecast vendor seam)**: the human owner's vendor
  selection and market-jurisdiction register entry (plus the ENTSO-E
  token registration). Closes the A44 transport wiring and any
  vendor-forecast ingestion; Phase 16 consumes whatever lands.
- **P15-E-3 (degradation-model chemistry coverage)**: the human
  owner's fleet-composition review, with per-chemistry parameters as
  human-gate inputs before any cost curve activates.
- The dataset-generation human gate (record 47): the manifest-v2
  forecasting rows stay EMPTY until a human authorizes a
  simulator/synthetic generation run; the poisoning-detection run
  remains UNRUN (carried unchanged from Phase 8).

## Known risks / recorded deviations

See the close record `records/62_p15_20_phase_close.md` §4 for the
full audit list. Headlines: no forecast produced, no model trained,
every evaluation cell EMPTY, no skill claimed, no vendor data ingested,
no score computed from real data, no drift detected, no document
ingested, no migration applied (every result cell EMPTY/UNRUN);
platform-side SoH estimation DEFERRED (record 23); rainflow extraction
NOT implemented (record 21 — the histogram stores caller-counted
cycles); the two EU-side precision items are carried as UNVERIFIED
registers ([S-111] wire details in record 04 §4; the [S-169]/[S-170]
category vocabulary in record 44 §2.4); `OBJECTIVE_SCHEMA_VERSION` NOT
bumped (record 22 §1 — the term shape is unchanged, the input-data
contract carries its own v2); the storage trait landed in
`flexcore-storage` per the D-P3.21-4 recorded home deviation (record
48 §4.1); migration 0021 carries no tenant RLS by the recorded
platform-global tenancy decision (record 44 §2.2). Full close
position: `records/62_p15_20_phase_close.md`.
