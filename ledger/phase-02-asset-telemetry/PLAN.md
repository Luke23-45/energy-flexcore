# PLAN — Phase 2: Asset and Telemetry Foundation

| Field | Value |
|---|---|
| Phase | Phase 2 — Report Stage 2 |
| Report trace | §23 Stage 2; §10–§11; §15; §19; §4; §6.5; §16 |
| Ledger | `ledger_phase_2.md` (P2.01–P2.31) |
| Status | Planning only — no code written by this ledger |

## 1. Scope

### In scope

- Registry: tenant/portfolio/site/asset domain model + application use cases
  + migrations, with `AssetRegistered`, `AssetCapabilityUpdated`,
  `AssetAvailabilityChanged` events via the outbox.
- Canonical telemetry model (fields, per-asset quantities, canonical units,
  quality, time, source identity/sequence) and its PostgreSQL partitioned
  storage (range-partitioned by `event_time`, indexes, RLS, retention as
  configuration).
- Ingestion pipeline (validate → normalize → quality-assign → dedup → store
  → publish), with quarantine, backpressure, and the `TelemetrySource` port.
- Data quality: per-value quality assignment (SUSPECT vs BAD boundary),
  stale detection with per-use-case freshness limits, gap/MISSING markers.
- Duplicate/out-of-order handling (idempotency key, source sequence,
  event_time-ordered current state, conflict quarantine).
- Simulator telemetry source (deterministic, seeded, synthetic BESS/PV/load/
  EVSE) + fault injection (stale, missing, suspect, bad, duplicates,
  out-of-order, conflicts).
- Telemetry event publication (`TelemetryReceived`,
  `TelemetryQualityChanged`, `AssetAvailabilityChanged`) via the outbox.
- Registry + telemetry API extensions (CRUD, `/telemetry/query`,
  current-state, historical, quarantine read) with OpenAPI.
- Current-state view (latest by `event_time`, freshness + quality) and
  historical view (time-range, aggregation, cursor pagination).
- Retention + partition maintenance (pre-creation job, approved
  detach/drop with audit).
- Telemetry observability (ingestion delay, staleness, queue depth, quality
  counts).
- Phase 2 security + fault-injection test slices; CI extensions (migration
  tests, scenario regression, telemetry contract tests).
- Deterministic replay (scenario → stored telemetry → replay report).
- Stage 2 exit gate on a synthetic multi-site portfolio (24-hour horizon).

### Non-goals (must NOT be built yet)

- No flexibility envelopes or capability-model computation — assets carry a
  capability-profile *reference* only; contents are Phase 3 (§11).
- No scheduling, optimization, or solver integration (Phase 3).
- No grid feasibility, command issue/acknowledgement, approvals (Phase 4).
- No operator console (Phase 5 — Phase 2 provides API/views data only).
- No edge agent, protocol adapters, or real site/field telemetry ingestion
  (Phase 6). The simulator is the only telemetry source in Phase 2.
- No market integration, no baseline method (depends on D1/D2).
- No TimescaleDB adoption (optional profile only, §16.1; revisit on measured
  volume).
- No Redis.
- No performance targets (observed numbers only, §19.4).
- No edits to `docs/foundations/*`, no edits to the report, no commits/pushes
  from this ledger work.

## 2. Architecture decisions in play

| Decision | Status | Report trace |
|---|---|---|
| Registry owns asset identity/metadata; telemetry owns measurements | Implemented (P2.02–P2.03 vs P2.04–P2.06) | §4 |
| Canonical telemetry model (P2.04) | Designed from P0.07–P0.09 | §10.2–§10.4 |
| Partitioned telemetry tables (range by `event_time`) | Implemented (P2.05–P2.06) | §16.1 |
| Retention as configuration, detach/drop of partitions | Implemented (P2.20) | §16.3 |
| Ingestion as a pipeline with evidence per stage | Implemented (P2.07, P2.10) | §6.4 |
| Quality: SUSPECT vs BAD boundary (hard failure → BAD; plausibility → SUSPECT) | Decided in P2.08, recorded | §10.4 |
| Deduplication identity `(asset_id, event_time, source_sequence)` + idempotency key | Decided in P2.09 | §6.3 |
| Current state = latest by `event_time`, not latest arrival | Implemented (P2.18) | §10.3–§10.4 |
| Simulator as the only `TelemetrySource` provider | Implemented (P2.14) | §5.8 `sim-lab` |
| Deterministic replay compares event_time/values/quality/sequence, never ingest_time | Implemented (P2.24) | §19.2, §24 step 10 |
| TimescaleDB not used | Recorded with measured-volume revisit trigger (P2.05) | §16.1 |

## 3. Component breakdown

| Component | Location | Subtasks |
|---|---|---|
| Registry domain + application | `crates/flexcore-domain/`, `crates/flexcore-application/` (extended) | P2.02–P2.03 |
| Canonical telemetry model | Design doc + `flexcore-contracts` types (extended) | P2.04 |
| Telemetry storage | `migrations/` (telemetry partitioned table), `crates/adapters/postgres/` (telemetry repository) | P2.05–P2.06 |
| Ingestion pipeline + service | `apps/worker/` (ingestion task), pipeline modules | P2.07, P2.10 |
| Quality / stale / dedup | Quality module, staleness/gap detector, dedup module | P2.08–P2.09 |
| TelemetrySource port + simulator | `crates/flexcore-ports/` (port), `plugins/builtin/asset-simulator/` | P2.11, P2.13–P2.15 |
| API extensions | `apps/control-plane/` (registry + telemetry routes) | P2.12, P2.17 |
| Current-state + historical views | Query modules + API endpoints | P2.18–P2.19 |
| Retention / partition maintenance | Maintenance job | P2.20 |
| Observability (telemetry) | Metrics/tracing extensions | P2.21 |
| Replay | `apps/sim-runner/` (minimal CLI, decision in P2.24) or worker job | P2.24 |
| Exit-gate portfolio | `scenarios/synthetic-portfolio/` (versioned scenario) | P2.25–P2.26 |
| CI extensions | CI workflow (migration tests, scenario regression, contract tests) | P2.27 |

## 4. Data contracts

| Contract | Home | Notes |
|---|---|---|
| Registry entities (Tenant/Portfolio/Site/Asset/PCC/GridModel ref) | P2.02 design; domain + migrations | Asset-type enum per §2.2; backup generation excluded |
| Canonical telemetry record | P2.04 design; `flexcore-contracts` types | Canonical units; exactly one quality state; event/ingest time; source identity + sequence |
| Telemetry partitioned table | P2.05–P2.06 | `PARTITION BY RANGE (event_time)`; PK includes partition key; RLS |
| Ingestion pipeline stages | P2.07 design; P2.10 implementation | Evidence per stage; quarantine; backpressure |
| TelemetrySource port | P2.11 | subscribe/decode/source_id/health; tenant-scoped queries |
| Telemetry query API | P2.12, P2.19 | Time range required; quality filter (six states); cursor pagination; enforced limits |
| Current-state response | P2.18 | Value + unit + quality + event/ingest time + age + `simulated` flag |
| Scenario (versioned) | P2.13; `scenarios/synthetic-portfolio/` | Portfolio + horizon + cadence + profiles + seeds + versions |
| Replay report | P2.24 | Scenario version, seed, code version, counts, diffs, verdict |

## 5. Interfaces

- **External:** extended REST API under `/api/v1` (registry CRUD,
  `/telemetry/query`, `/assets/{id}/state`, quarantine read); OpenAPI
  regenerated (utoipa).
- **Internal:** `TelemetrySource` port (subscribe/decode); ingestion service
  consumes the port only; telemetry events (`TelemetryReceived`,
  `TelemetryQualityChanged`, `AssetAvailabilityChanged`) via the outbox →
  JetStream; consumers use the P1.16 inbox.
- **Simulator:** a built-in `TelemetrySource` provider; the only provider in
  Phase 2; deterministic and seeded.

## 6. Sequencing

1. Entry gate (P2.01) → registry design (P2.02) → registry implementation
   (P2.03).
2. Telemetry model (P2.04) → storage design (P2.05) → migrations/repositories
   (P2.06).
3. Pipeline design (P2.07) → quality/stale (P2.08) + dedup (P2.09) in
   parallel → ingestion service (P2.10).
4. Source contract (P2.11) → API extensions (P2.12) → scenario model (P2.13)
   → simulator (P2.14) → fault injection (P2.15).
5. Event wiring (P2.16) → registry views (P2.17) → current-state (P2.18) →
   historical (P2.19) → retention (P2.20) → observability (P2.21).
6. Security (P2.22) + fault-injection (P2.23) suites → replay (P2.24) →
   exit-gate portfolio (P2.25) → exit-gate demonstration (P2.26) → CI (P2.27)
   → handover (P2.28) → exit-gate review (P2.29) → anti-scope audit (P2.30).

Relative effort: the dominant cost is the ingestion pipeline + quality/stale/
dedup work with its tests (P2.07–P2.10, P2.22–P2.23), then the simulator +
replay (P2.13–P2.15, P2.24) and the query views (P2.18–P2.19). No calendar
dates are asserted; sequencing is by dependency, per §23.

## 7. Roles

| Role | Responsibilities |
|---|---|
| Power systems | Telemetry model quantities, quality rules, simulator profiles, exit-gate portfolio, replay witness |
| Platform engineering (Rust) | Registry, storage, ingestion, API, simulator, replay, CI |
| Principal engineering | Entry/exit gates, handover, boundary decisions (P2.05/P2.11/P2.18/P2.24 decision points) |
| Security (reviewer) | Telemetry/registry isolation tests, source-auth tests |

## 8. Risks and mitigations

| Risk | Mitigation |
|---|---|
| kW or Q-sign assumption leaking into persisted telemetry | P2.04 design review + contract tests; newtypes from P1.05 |
| STALE from ingest_time instead of event_time | P2.08 boundary tests |
| ESTIMATED silently substituted for missing data | P2.08 rule: ESTIMATED only with an explicit derivation rule + test |
| Current state regressed by a late record | Latest-by-event_time rule + test (P2.18) |
| Conflicting duplicate silently overwriting stored data | Quarantine-on-conflict (P2.09) |
| Missing partition crashing ingestion at a boundary | Pre-creation job + error-path test (P2.20, P2.23) |
| Simulator mistaken for a validated physical model | Assumptions stated per profile; handover states synthetic status (P2.13, P2.28) |
| Simulator data presented as measured | `simulated: true` flag + quality on every value (P2.17–P2.18) |
| Replay compared on ingest_time (always fails) or skipped (always passes) | Comparison-field test + CI replay verdict (P2.24, P2.27) |
| Unbounded telemetry query | Enforced range/page/row limits + 422 tests (P2.12, P2.19) |

## 9. What must NOT be built yet

See §1 non-goals. In particular: no flexibility/capability computation, no
scheduling, no feasibility, no dispatch path, no console, no edge/protocol
adapters, no real field telemetry, no market integration, no TimescaleDB.

## 10. How the §24 vertical slice lands here

Steps 1–2 land in Phase 2: step 1 (create tenant/portfolio/site/grid model/
four simulated assets) — the grid-model reference is stored, the four
simulated assets are registered and telemetered; step 2 (timestamped telemetry
with quality flags) is the ingestion pipeline. Step 10 (replay the same
scenario and verify reproducibility) is implemented here as deterministic
replay (P2.24) and demonstrated at the exit gate (P2.26). Steps 3–9 do not
land in Phase 2.
