# P2.28 — Phase 2 handover pack for Phase 3

Date: 2026-11-07. Index of every Phase 2 deliverable: location × version ×
status. Status vocabulary: **implemented, unbuilt, unrun** (the whole 0.x
line is simulation-only; verification is the human's — see `HANDOVER.md`
"Blockers").

## 1. Artefact index (P2.01–P2.27, P2.31)

| Subtask | Deliverable | Location | Status |
|---|---|---|---|
| P2.01 | Entry note + gate exception | `ledger/phase-02-asset-telemetry/ENTRY_NOTE.md` | done |
| P2.02 | Registry domain model (D2.x) | `02_registry_domain_model.md`; `crates/flexcore-domain/src/{entities,registry_rules}.rs` | implemented |
| P2.03 | Registry use cases + outbox-only events | `crates/flexcore-application/src/{create_portfolio,create_site,register_asset,update_asset_metadata,deactivate_asset,query_registry,event_assembly}.rs` | implemented |
| P2.04 | Canonical telemetry model (D-T1…T6) | `04_telemetry_model.md`; `crates/flexcore-contracts/src/{units,quality,time_model}.rs` | implemented |
| P2.05 | Partitioned storage design (D-P1…P7) | `05_telemetry_storage_design.md` | done (design) |
| P2.06 | Telemetry migrations + repository | `crates/adapters/postgres/migrations/0006..`/`src/telemetry.rs` | implemented |
| P2.07 | Pipeline design (7 stages, quarantine, backpressure) | `07_ingestion_pipeline_design.md` | done (design) |
| P2.08 | Quality/staleness/gap rules | `crates/flexcore-domain/src/quality_rules.rs` | implemented |
| P2.09 | Dedup identity + exact-vs-conflicting classification | `crates/flexcore-application/src/telemetry_dedup.rs`; savepoints in `telemetry.rs` | implemented |
| P2.10 | Ingestion service | `apps/worker/src/ingestion.rs` (+ composition wiring in `main.rs`) | implemented |
| P2.11 | `TelemetrySource` port + contract | `crates/flexcore-ports/src/telemetry_source.rs` (+ `telemetry_source_contract.rs`) | implemented |
| P2.12 | API extensions (telemetry query, state, quarantine) | `apps/control-plane/src/routes_phase2.rs` | implemented |
| P2.13 | Scenario model + versioning | `13_scenario_model.md`; `scenarios/synthetic-portfolio/` | implemented |
| P2.14 | Deterministic simulator | `crates/flexcore-simulator/` (splitmix64, per-asset seed) | implemented |
| P2.15 | Fault injection (7 types, scenario config) | simulator fault types + `scenario-v2.json` expected-quality plan | implemented |
| P2.16 | Event wiring (per-record granularity, prev/new quality, availability rule) | `apps/worker/src/ingestion.rs`; `crates/flexcore-application/src/availability.rs` | implemented |
| P2.17 | Registry API + views | `apps/control-plane/src/routes_phase2.rs` (views + keyset pagination) | implemented |
| P2.18 | Current-state view (latest by event_time, freshness) | `routes_phase2.rs` `get_asset_state`; freshness config | implemented |
| P2.19 | Historical view (D-H1…D-H6) | `routes_phase2.rs` history route; `telemetry.rs` history/aggregate | implemented |
| P2.20 | Retention/partition maintenance (D-R1…R6) | `crates/adapters/postgres/src/partition_maintenance.rs`; `apps/worker/src/maintenance.rs` | implemented |
| P2.21 | Telemetry observability (6 metric families, spans, no-secrets) | `apps/control-plane/src/metrics.rs`; `apps/worker/src/ingestion.rs` | implemented |
| P2.22 | Security test slice | `apps/control-plane/tests/security_phase2.rs`; `apps/worker/tests/security_phase2.rs` | implemented |
| P2.23 | Fault-injection suite (8 cases, fault-first) | `apps/worker/tests/fault_suite.rs` | implemented |
| P2.24 | Deterministic replay | `apps/sim-runner/` (`run`/`replay`, exit codes 0/1/2) | implemented |
| P2.25 | Exit-gate portfolio + runbook + coverage accounting | `scenarios/synthetic-portfolio/scenario-v2.json`; `25_exit_gate_runbook.md`; `apps/control-plane/tests/exit_portfolio.rs` | implemented (run = human) |
| P2.26 | Exit-gate demonstration record | `26_exit_gate_record.md` | **NOT RUN** (runbook ready) |
| P2.27 | CI extensions (migration/scenario-regression/contract) | `.github/workflows/ci.yml`; `27_ci_gate_inventory.md` | implemented (CI run = human) |
| P2.31 | Versioned corrections + as-of (foundations §6.4) | migration 0012; `corrections.rs`; `as_of.rs` sim-runner subcommand; `as_of` API params | implemented |

## 2. Capability-profile status (Phase 3 entry condition)

Asset capability-profile **references** exist on registry assets (P2.02
vocabulary, P0.10). The profiles' **contents are Phase 3 work**: the
simulator's behaviour profiles (BESS/PV/load/EVSE/meter shapes in
`crates/flexcore-simulator/src/profiles.rs`) are SYNTHETIC, documented as
such, and must never be treated as validated physical models.

## 3. Forecast non-delivery (stated boundary, not a gap)

No Phase 2 subtask produces a `ForecastBundle` or any forecast provider,
and no forecast artefact is handed over. Phase 3 envelope/scheduling
subtasks that name forecast inputs consume the Phase 3 synthetic forecast
fixture (**P3.31**) — §23 Stage 2 deliverables do not include forecasts.

## 4. Open questions Phase 3 must NOT answer by itself

| Question | Gate/owner |
|---|---|
| D1 jurisdiction/market; D2 first portfolio/service; D3 network class | Named human owners (`.agent/DECISIONS.md`, all SCHEDULED) |
| Baseline method | Depends on D1/D2 — blocked until they close |
| Retention values | Deployment configuration, never code (D-R4) |
| Solver selection per problem class | Phase 3 must apply the P0.13 solver-class discipline (HiGHS = LP/QP/MIP; Clarabel = convex conic, not MISOCP); any relaxation decision is an ADR |

## 5. Entry condition

Stage 2 exit gate NOT held (`26_exit_gate_record.md` — NOT RUN; review in
P2.29). Per the loop-rules exception in `ENTRY_NOTE.md`, Phase 3 proceeds;
the gate must be recorded before any release claim.
