# P16.08 — Telemetry export pipelines (DESIGN): per-market postures, cadence budgets, unwired sink

| Field | Value |
|---|---|
| Subtask | P16.08 (Per-market pipeline DESIGNS: ERCOT 2-second ICCP with net-consumer normalization and weighted ramp-rate aggregation [S-100][S-101]; NYISO 6-second SCADA + SOC [S-105][S-106]; PJM ≥10 MW telemetry posture [S-90]; adapter seams + cadence budgets; certification is a human-gated lab process. Record + design; no pipeline transmits) |
| Wave / agent | Phase 16 wave 1, Agent B |
| Status | WRITTEN-UNRUN: postures, budgets, declared formulas, and the unwired sink are written, never compiled, never executed. No export pipeline transmits — no socket, no ICCP/TASE.2 stack, no SCADA link exists anywhere. |
| Sources | [S-90] (PJM FERC 2222 participation model overview), [S-100] (ERCOT ADER pilot — 2-second ICCP), [S-101] (ERCOT ADER Phase 3.3 report), [S-104] (ISO-NE metering & telemetry checklist), [S-105] (NYISO Metering Fundamentals Dec 2024), [S-106] (NYISO DER 6-second telemetry MIWG deck) |
| Files touched | `crates/flexcore-application/src/telemetry_export_design.rs` (NEW); `crates/flexcore-application/src/lib.rs` (additive module line only) |

## 1. What landed

| Item | Content |
|---|---|
| `ExportPostureRecord` | One per-market posture RECORD: market (closed ErcotAder/NyisoEsr/PjmAggregation vocabulary — per-jurisdiction by design, ADR-011), declared cadence budget, required quantity set (NYISO carries SOC alongside power), transport seam REFERENCE (never a connection), recorded source. Fail-closed construction |
| Declared budget constants | `ERCOT_ICCP_BUDGET_MILLIS = 2_000` [S-100][S-101] (the confirmed 2-second ICCP figure; the "4-second"-class references are superseded per gap analysis §11 track E); `NYISO_SCADA_BUDGET_MILLIS = 6_000` [S-105][S-106]; `PJM_MIN_AGGREGATION_MW = 10.0` [S-90]. All carry the re-verify-at-use rule; none is a measured or met claim (X-8: budgets registered before any run that could fill them; observed values stay structurally separate) |
| `net_consumer_normalization` | The DECLARED export posture [S-101]: aggregates per-asset signed active power (canonical P>0 = export at the POI) and renders the aggregate in the DECLARED export convention (ExportPositive / ConsumptionPositive). MISSING gap markers are skipped, never zero-filled; empty batches and non-finite values are named refusals |
| `weighted_ramp_rate_aggregation` | The DECLARED capacity-weighted ramp-rate mean — the formula FlexCore declares it would use to PREPARE an export value, explicitly NOT a claim about any ISO's own aggregation (ISO-side rules UNVERIFIED; re-verify at use). Zero total weight is a named refusal |
| `TelemetryExportSink` + `UnwiredTelemetryExportSink` | The adapter SEAM, unwired: the default binding answers every call with the named `EXPORT_SEAM_NOT_WIRED` refusal — the refusal IS the guarantee that nothing can silently transmit. Posture/sink market mismatches are refused before the seam answers |
| Tests (written-unrun) | Inline suite: statement pins, budget constants vs cited figures, fail-closed posture construction, hand-computed normalization/aggregation values, unwired-sink behavior, label stability |

## 2. Recorded decisions

1. **A DESIGN record in code, not a transmitting pipeline** (`NO_PIPELINE_TRANSMITS_STATEMENT`, pinned by tests): the first live export is a human-gated lab process; no conformance, qualification, or certification claim is made by or for FlexCore (conventions §7.3).
2. **Single ingestion path preserved** (`EXPORT_READS_EXISTING_TELEMETRY_STATEMENT`): export batches compose from the EXISTING Phase-2 pipeline records (`NormalizedTelemetry`); no parallel ingestion, no second decoder, no side collection.
3. **Budgets are declared data, not measured claims** (`BUDGETS_ARE_DECLARED_STATEMENT`): each constant cites its source and the re-verify-at-use rule; cadence budgets are registered here BEFORE any run that could fill them (X-8 carryover).
4. **Jurisdiction specificity** (ADR-011): three named postures with named sources; a fourth market is a NEW posture record, never a hard-coded branch or a universal engine.
5. **SOC stays a fraction in [0, 1] in export** (the canonical-unit rule holds at the export boundary).

## 3. Honest boundaries

1. No pipeline transmits; no export value has ever been sent; no ISO telemetry requirement is claimed as met.
2. The aggregation formulas are FlexCore's DECLARED preparation posture — no claim is made about how any ISO computes its own aggregates.
3. Certification/conformance is a human-gated lab process, never claimed.
4. Every test is WRITTEN-UNRUN; execution is the human gate. The permitted-tool outputs are quoted in the batch close-out record (26).
