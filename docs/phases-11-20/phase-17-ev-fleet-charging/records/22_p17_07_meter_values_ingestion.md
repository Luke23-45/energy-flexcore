# P17.07 — MeterValues ingestion + signed-meter-value hooks

| Field | Value |
|---|---|
| Subtask | P17.07 — MeterValues cadence configuration, MeterValuesSign hooks (the Eichrecht-grade path; exact block letters UNVERIFIED), and the adapter-boundary mapping onto the frozen six-state quality vocabulary — the non-silencing rule intact [S-135][S-136] |
| Wave / agent | Phase 17 wave 1, Agent B (P17.06–P17.11) |
| Status | WRITTEN-UNRUN: the mapping module, the cadence configuration, the signed-meter-value hooks, and the worker bridge are written with their tests, never compiled, never executed. No charge point is connected, no meter value was ever observed, no signed meter value was ever received. No metrology, Eichrecht, or AFIR compliance claim is made anywhere. |
| Sources | [S-124] ocpp.md OCPP 2.0.1 smart charging; [S-135] AFIR (EU) 2023/1804; [S-136] Eichrecht guide (elvo.io / ampeco.com). **Every measurand spelling, cadence-variable name, and MeterValuesSign detail below is SECONDARY-SOURCED and UNVERIFIED against the OCA text** — §4 registers the implementation-time checks. |
| A-dependence | Agent A's `protocol-ocpp` adapter (P17.01) is the upstream producer; its types are consumed BY CITATION via the adapter-neutral `EvInboundFrame::Telemetry` / `SignedMeterValue` shapes in `apps/worker/src/ev_ingest.rs` (A's layout record had not landed at batch time — recorded in record 21 §A-dependence). |
| Files touched | `crates/flexcore-application/src/ev_meter_values_ingest.rs` (NEW), `apps/worker/src/ev_ingest.rs` (NEW, shared with P17.06), `crates/flexcore-application/src/lib.rs` (additive registration), `apps/worker/src/lib.rs` (additive registration). |

## 1. What landed (deliverable class: ingestion-seam extension, existing pipeline)

| Piece | Where | Honest status |
|---|---|---|
| `MeterValuesCadenceConfig` — per-quantity sampled-value cadence (SoC / active power / energy register) plus per-quantity freshness limits; every value positive, no defaults; validated at construction and at routing time | `ev_meter_values_ingest.rs` | Implemented; UNRUN |
| The measurand mapping `sampled_value_quantity` — the recorded EV set (`SoC`, `Power.Active.Import`, `Energy.Active.Import.Register` — spellings UNVERIFIED [S-124]) onto the canonical `TelemetryQuantity` vocabulary; an unknown measurand is a named error, never a default quantity | same | Implemented; UNRUN |
| `MeterValuesQualityMapping` — the adapter-boundary declared mapping: every source-side quality code the adapter can report is declared with ONE canonical state (frozen six, consumed never redefined); the no-code case is the DECLARED assignment for a plain measured reading | same | Implemented; UNRUN |
| `sampled_value_quality` — the non-silencing rule in one pure function: an UNDECLARED code is a named error (never a default assignment — the ACL rule); staleness holds-or-lowers (`SUSPECT`/`BAD` degradation survives, `GOOD` becomes `STALE`); the output is exactly one of the frozen six — no seventh state | same | Implemented; UNRUN |
| `SignedMeterValueRecord` + `SignedMeterValueSink` — the MeterValuesSign hook seam: the signed value arrives OPAQUE (verbatim payload, never parsed), is recorded with provenance, and is forwarded to the sink; a failed write is a named `Err` the caller counts and retries (the no-silent-skip rule). The hook NEVER verifies a signature and NEVER claims compliance | same | Implemented; UNRUN |
| The worker bridge `EvInboundFrame::Telemetry` routing — telemetry-shaped frames are offered to the EXISTING P2.07 pipeline's bounded channel (`TelemetryOffer` over `IngestionQueue`); a shed is the retryable `TelemetryShed` outcome, counted by the pipeline's own metrics. **No parallel ingestion path exists** — decode, quality assignment, dedup, store, and publish are the existing pipeline's, unchanged | `apps/worker/src/ev_ingest.rs` | Implemented; UNRUN |

## 2. The frozen quality vocabulary (consumed, never redefined)

The six-state `TelemetryQuality` (`flexcore-contracts::quality`) is
CONSUMED at the adapter boundary exactly as the Phase 0 freeze requires:

- no seventh state is introduced anywhere (the tests iterate
  `TelemetryQuality::ALL` and pin the mapped output set inside it);
- the non-silencing rule is intact: no function maps a degradation to
  `GOOD`; a source-reported `SUSPECT`/`BAD` survives staleness; the
  pipeline's own hold-or-lower stage (`refine_quality`, P2.08) runs
  downstream unchanged;
- `GOOD` appears only as the declared assignment for a plain measured
  reading with no source-side code — a declared provider mapping (the
  P2.11 provider rule), not a conversion of a degradation.

## 3. The Eichrecht-grade path, stated honestly

The seam keeps the AFIR/Eichrecht-caliber metrological surface
ADDRESSABLE [S-135][S-136] and claims nothing:

1. signed meter values land as opaque evidence records with provenance;
2. signature verification is a metrological (calibration-body / lab)
   process — a HUMAN GATE this platform never performs and never claims;
3. the AFIR/Eichrecht scope verdict itself is P17.16's assessment record
   (wave 2) — a depot fleet behind a site meter is adjacent, not directly
   AFIR-scoped (the phase-17 context records this; this batch does not
   pre-empt the verdict).

## 4. UNVERIFIED register (implementation-time checks against the OCA text)

| # | Detail | Where |
|---|---|---|
| 1 | The exact OCPP variable names a charge point exposes for MeterValues reporting configuration (sampled-value intervals per measurand) [S-124][S-125] | `MeterValuesCadenceConfig` (the deployment configuration; the adapter's wire interaction is the adapter's check) |
| 2 | The measurand spellings `SoC`, `Power.Active.Import`, `Energy.Active.Import.Register` [S-124] | `sampled_value_quantity` |
| 3 | Whether OCPP sampled values carry any quality signal at all (this module assumes none — the declared no-code mapping is the honest consequence) [S-124] | `MeterValuesQualityMapping::no_source_code_quality` |
| 4 | The MeterValuesSign block/message names and field spellings in OCPP 2.1 [S-125][S-136] | `SignedMeterValueRecord` (the payload is OPAQUE, so a spelling mismatch does not touch this module's data model) |

## 5. Tests (written-unrun; execution is a human gate)

Cadence validation (per-quantity, positive, no defaults; the
per-quantity lookups; untracked quantities return `None`), the undeclared-
code named error, the no-code declared assignment, degradation survival
under staleness (SUSPECT stays SUSPECT, BAD stays BAD, GOOD becomes
STALE), the mapped output set inside the frozen six, the measurand
mapping, the opaque signed-record refusals, the sink receive-and-fail-
loudly contract, and the worker routing tests (queued / shed-at-limit /
proposal→record-never-command / signed→sink / failing sink named error /
the production `IngestionQueue` trait wiring).
