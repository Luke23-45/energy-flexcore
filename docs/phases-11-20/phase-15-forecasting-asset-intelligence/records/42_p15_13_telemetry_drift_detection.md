# P15.13 — Telemetry-drift detection: reporting-rate expectations and value plausibility bounds per asset class (twin-lite)

| Field | Value |
|---|---|
| Subtask | P15.13 (reporting-rate expectations + value plausibility bounds per asset class — twin-lite per gap analysis §7, explicitly NOT grid state estimation; integration point = the Phase-2 telemetry source contract, NO parallel ingestion path) |
| Wave / agent | Phase 15 wave 2, Agent A |
| Status | WRITTEN-UNRUN: the module and its tests are written, never compiled, never executed. No drift was detected from real data; every test value is hand-computed over CONSTRUCTED records. |
| Sources | [S-168] (the MDPI DER state-estimation twin — cited for the digital-twin SPECTRUM this feature deliberately stays at the lite end of; no state-estimation capability is claimed); [S-53] (asset-performance-management context) |
| Files touched | `crates/flexcore-application/src/telemetry_drift.rs` (NEW); `crates/flexcore-application/src/lib.rs` (additive module line + pub use block) |

## 1. What landed (deliverable class: pure evaluation over the existing telemetry port)

| Item | Content |
|---|---|
| Reporting-rate expectations (`ReportingRateExpectation`, `evaluate_reporting_rate`) | Declared expected cadence + tolerance band per asset class; an observed inter-arrival gap drifts when its deviation from the expected interval EXCEEDS the tolerance (boundary values are not violations — the P2.08 equality convention); a violating-gap threshold fires the `TooSparse` / `TooFrequent` verdict with the count carried; an empty record run is a refusal, never a clean bill of health (fail-closed) |
| Value plausibility bounds (`PlausibilityBounds`, `evaluate_plausibility` / `evaluate_plausibility_run`) | Declared inclusive bands per quantity (the P2.04 §4 vocabulary; MW for power, [0, 1] for SOC), sign-preserving; a record of another quantity or a MISSING gap marker is skipped by the lens (the MISSING state is the pipeline's own supervision); deterministic BTreeMap per-quantity band table; the run refuses empty input |
| Integration discipline | Every input is a `flexcore_ports::telemetry_source::NormalizedTelemetry` record delivered by the EXISTING Phase-2 `TelemetrySource` port (`telemetry_source_contract.rs` — the recorded integration point). No raw frames, no second decoder, no side store, no separate subscription: the P2.10 pipeline stages are unchanged (`NO_PARALLEL_INGESTION_STATEMENT`) |
| Quality non-interference | A drift verdict NEVER rewrites, upgrades, or downgrades a record's quality state (the P0.08 six-state vocabulary is owned by the P2.08 quality assignment); a drift record is a separate, additive operator-view record (`QUALITY_NON_INTERFERENCE_STATEMENT`) |
| Honesty constants | `TWIN_LITE_NOT_STATE_ESTIMATION_STATEMENT` (no network model, no state estimate, no observability analysis), `DRIFT_RECORD_IS_NOT_A_COMMAND_STATEMENT` (ADR-010) |

## 2. Honest boundaries

1. Bounds are DECLARED, never learned: the platform has never run; there is no data to learn from, and no learning procedure exists in the module.
2. Not grid state estimation: nothing here computes a grid quantity; [S-168] names the spectrum, this module records its place at the lite end.
3. No dispatch reach (ADR-010 extension, conventions §6.3).
4. Every test is WRITTEN-UNRUN; the permitted-tool outputs are quoted in the close-out record (48).
