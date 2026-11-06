# CONTEXT — Phase 2: Asset and Telemetry Foundation

## Background

Phase 2 is Report Stage 2 (§23): build the asset and telemetry foundation —
tenant/portfolio/site/asset registry, canonical telemetry model and units,
simulator telemetry source, data quality and stale detection, current-state
and historical views. Its exit gate is that synthetic BESS/PV/load/EVSE
portfolio telemetry is ingested, normalized, queried, and replayed
deterministically.

The registry and telemetry are separate bounded contexts (§4): the Asset
Registry owns asset identity, type, topology reference, capabilities,
constraints, and vendor metadata; Telemetry owns ingestion, normalization,
quality, time alignment, retention, and query. Neither owns the other's
writes.

## Key report sections to re-read

| Section | Why it matters here |
|---|---|
| §4 Core product modules | Asset Registry vs Telemetry ownership |
| §6.4–§6.5 | Pipeline pattern (evidence per stage); selective event sourcing; time-series tables |
| §9.1 | `TelemetrySource` contract (subscribe/decode) |
| §10 Domain model | Entities (§10.1), units/sign (§10.2), time model (§10.3), quality (§10.4) |
| §11 Capability modelling | Vocabulary only in this phase — capability-profile *reference* on assets; computation is Phase 3 |
| §15 APIs, events, messaging | REST resources (§15.2), event vocabulary (§15.3), NATS/outbox (§15.4–§15.5) |
| §16 Storage and tenancy | Partitioned telemetry tables (§16.1), RLS (§16.2), retention (§16.3) |
| §19 Simulation, testing, benchmarks | Tracks (§19.1), test layers (§19.2), benchmark cases (§19.3), no-arbitrary-targets rule (§19.4) |
| §20.1 Operator workspaces | Portfolios/Sites/Assets/Grid views — API/views data only in this phase |
| §21.3 Observability | Correlation fields; ingestion delay, stale telemetry, queue depth |
| §23 Stage 2 | Deliverables and exit gate |
| §24 Vertical slice | Steps 1–2 and 10 land here |
| §25.2 | D1–D3 still open or scheduled; D4 inventory delivered in Phase 1 |

## Canonical units/sign (from Phase 0, P0.07 — cited, not re-defined)

MW, MVAr, MVA, MWh. P > 0 = export at the point of connection. Reactive-power
sign declared per grid model/device profile, never assumed. SOC is a fraction
in [0,1] at the API boundary. Persisted values and API contracts use canonical
units; kW/kWh are display-only.

## Telemetry quality (from Phase 0, P0.08 — cited, not re-defined)

Exactly one per value: `GOOD`, `SUSPECT`, `BAD`, `MISSING`, `ESTIMATED`,
`STALE`. Phase 2 implements the assignment rules (P2.08): hard validation
failure or source-reported invalid → `BAD`; plausibility/range/consistency
failure on a well-formed value → `SUSPECT`; no value for an expected interval
→ `MISSING` (derived marker, not a measured value); derived/interpolated/
substituted/model-generated → `ESTIMATED` (only with an explicit derivation
rule); last known value older than the use-case freshness limit → `STALE`.

## Terminology

- "license" (US spelling); "acknowledgement" (noun).
- Registry vs telemetry: the registry owns *what an asset is*; telemetry owns
  *what an asset measured, when, and how good the measurement is*.
- Current state = latest by `event_time`, not latest arrival. A late record
  never regresses current state.
- Replay compares (`asset_id`, `event_time`, quantity, value, quality,
  `source_sequence`) — never `ingest_time`.
- `simulated: true` marks every simulator-derived value. Simulator data is
  never presented as measured.
- Retention is configuration per data class (§16.3); no universal retention
  period in code.

## Constraints (simulation-only 0.x)

- The entire 0.x line is simulation-only. Phase 2 ingests simulated telemetry
  only; no field reads, no device writes, no control authority above
  Simulation.
- The simulator's behaviour profiles are synthetic, with stated assumptions
  — not validated physical models.
- No performance targets: observed ingestion rates and query latencies are
  recorded as observations, never as promises (§19.4).

## Prior-phase outputs consumed

| Output | Consumed by |
|---|---|
| Phase 0: units/sign (P0.07), quality (P0.08), time model (P0.09), asset vocabulary (P0.10), events (P0.14), API surface (P0.15), tenancy/storage (P0.16) | Telemetry model (P2.04), quality rules (P2.08), API (P2.12) |
| Phase 1: `flexcore-contracts` (P1.05), domain (P1.06), ports (P1.07), application (P1.08), events (P1.09), storage/audit (P1.10), authz (P1.11) | Registry + telemetry implementation |
| Phase 1: schema/RLS (P1.12–P1.13), JetStream topology (P1.14), outbox publisher (P1.15), idempotent consumers (P1.16) | Ingestion + event publication (P2.07, P2.10, P2.16) |
| Phase 1: Axum API + OpenAPI (P1.17), worker (P1.19), observability (P1.20), Compose (P1.25), CI (P1.27) | API extensions, ingestion hosting, telemetry metrics, exit-gate runs |
| Phase 1: D4 license inventory (P1.28) | D4 stays OPEN; no new license decision in Phase 2 |

## D1–D6 relevance

| Gate | Phase 2 relevance |
|---|---|
| D1 jurisdiction/market | Market region on a portfolio is a label, not logic; baseline method undecided |
| D2 first portfolio/service | Exit-gate portfolio follows the P0.04 first-portfolio sheet |
| D3 network class/fidelity | Grid-model reference stored (model + version); no power-flow code in Phase 2 |
| D4 license/dependency audit | Inventory delivered in Phase 1; decision OPEN; new dependencies in Phase 2 go into the inventory |
| D5 first protocol/asset mix | No protocol code; simulator is the only telemetry source |
| D6 cyber/OT boundary review | No edge/OT code; source-auth stub status recorded honestly (P2.10) |

## Open questions

1. Telemetry table shape: one table for all quantities vs one per quantity
   (P2.05 — decide once, record it).
2. Partition interval: daily vs weekly vs monthly (P2.05 — decide from
   synthetic volume, record reasoning).
3. Telemetry record identity for dedup (P2.09 — decide once).
4. Ingestion hosting: worker task vs dedicated binary (P2.10 — decide once;
   default worker task).
5. Event granularity: `TelemetryReceived` per batch vs per record (P2.16 —
   decide once; default batch-level).
6. Current-state implementation: indexed query vs materialized table (P2.18 —
   decide once; default indexed query).
7. Pagination: cursor vs offset (P2.19 — decide once; default cursor).
8. Replay entrypoint: minimal `sim-runner` CLI vs worker job (P2.24 — decide
   once; default minimal `sim-runner`).
9. Retention execution: detach-then-drop vs archive-first (P2.20 — decide once).
10. Availability rule for `AssetAvailabilityChanged` (P2.16 — define once).
