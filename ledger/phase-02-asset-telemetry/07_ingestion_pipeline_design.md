# P2.07 — Ingestion pipeline design (validate → normalize → store → publish)

Provenance: subtask P2.07. Report trace: §6.4 (pipeline pattern), §3.4
(workflow steps 3–4), §15.3 (`TelemetryReceived`, `TelemetryQualityChanged`).
Inputs: P2.04 (`04_telemetry_model.md`, D-T1…T6), P2.05
(`05_telemetry_storage_design.md`, D-P1…P7), P2.06, P0.08. Status: design
for Phase 2 implementation (P2.10).

## 1. Stages

| # | Stage | Input | Output / evidence |
|---|---|---|---|
| 1 | Receive | Source payload + source identity | Source authenticated per the recorded Phase 2 stub status (simulator only, P2.10); raw payload + source id + arrival timestamp recorded for every later stage's evidence |
| 2 | Schema validation | Raw payload | Validated shape (record fields per D-T1…T3, required fields, types); failure → stage-7 quarantine with reason |
| 3 | Normalize (unit/sign) | Validated record | Canonical values (MW/MVAr/MWh, SOC [0,1]) with the source's declared unit/sign-mapping profile recorded; adapter- or source-declared; P > 0 = export at the PCC; Q convention carried on the record (D-T4) |
| 4 | Quality assignment | Normalized record | Exactly one of the six P0.08 states per value (rules implemented in P2.08); source quality code retained as diagnostic metadata |
| 5 | Duplicate / out-of-order handling | Quality-assigned record | Dedup identity `(asset_id, event_time, source_sequence)` (+ quantity, per D-P3 PK): exact duplicate → acknowledge, not stored twice; conflicting duplicate → quarantine with reason (P2.09) |
| 6 | Store | Accepted record | Insert via the P2.06 repository inside a unit of work; partition out-of-range errors → quarantine (D-P2 pre-creation is the P2.20 job) |
| 7 | Publish | Stored record(s) | `TelemetryReceived` per batch (granularity decision, P2.16) + `TelemetryQualityChanged` on quality-state change — via the OUTBOX only (§6.3 step 1; never a direct NATS publish from the pipeline) |

## 2. The no-silent-skip rule (§6.4)

Each stage records its evidence. A failure at any stage does NOT silently
skip the next gate: the failed record is **quarantined with a reason**,
**counted** (metric), and **retried where safe** (retryable failures only).
A failed record is never silently dropped, and never silently stored as
`GOOD`. The quarantine is the evidence trail for every rejected value.

## 3. Quarantine design

- Storage: a `telemetry_quarantine` table (migration lands with P2.10's
  implementation, extending the P2.06 migration set — not recreating it):
  raw payload (JSONB), failure reason (stage + rule), source_id,
  source_sequence if present, tenant_id, asset_id if resolvable,
  event_time if present, quarantined_at, attempt count.
- Queryable: read via the API (`GET /api/v1/telemetry/quarantine`,
  tenant-scoped, admin role — P2.12); count exposed as a metric (P2.21).
- Retry: retryable failures (transient store errors) are re-queued; shape
  errors are terminal (fix the source), remaining until the retention
  policy applies to the quarantine class (configuration, P0.16).

## 4. Ordering guarantee

Ingestion tolerates out-of-order arrival: the pipeline never assumes
arrival order equals event order. Late records are stored (valid history);
the current-state view is recomputed from the latest `event_time`, never
the latest arrival (P2.09; P2.18 implements the read rule). No global
ordering is promised (matches the P1.14 no-global-order rule).

## 5. Backpressure rule

- Ingestion is bounded: a bounded channel between the receive stage and
  the store stage. Limit: 10,000 buffered records per ingestion worker
  instance (recorded default; deployment configuration may override).
- At the limit: the service sheds load by a RECORDED policy — the receive
  stage rejects with a retryable error, increments the shed metric, and
  the source retries with backoff. Unbounded buffering and silent
  dropping are both prohibited (P2.10 tests the behaviour).

## 6. Publication per stage (summary)

| Event | Trigger | Path |
|---|---|---|
| `TelemetryReceived` | Batch stored (granularity decided in P2.16; batch default to bound volume) | Outbox, same transaction class as the store stage (P1.13 atomicity pattern) |
| `TelemetryQualityChanged` | An asset's quality state changes (P2.08), with previous + new states | Outbox |
| `AssetAvailabilityChanged` | Asset availability changes per the recorded availability rule (P2.16) | Outbox |

## 7. Acceptance-criteria mapping

| Criterion (ledger P2.07) | Section |
|---|---|
| All seven stages defined with inputs/outputs | §1 |
| No-silent-skip rule + quarantine design stated | §2, §3 |
| Backpressure limit + at-limit behaviour stated | §5 |
| `TelemetryReceived`/`TelemetryQualityChanged` publication via outbox stated per stage | §6 |
