# P1.29 — Phase 1 handover pack for Phase 2

Date: 2026-11-07. Status: implementation written; **nothing built, tested,
or verified** — all verification is the human's (loop rules). Cited by
`ledger/phase-02-asset-telemetry/ENTRY_NOTE.md`.

## 1. Handover artefacts (P1.01–P1.28)

| # | Artefact | Subtask | Location | Status |
|---|---|---|---|---|
| 1 | Entry note (gate exception) | P1.01 | `ledger/phase-01-rust-foundation/ENTRY_NOTE.md` | Recorded exception — gates NOT held |
| 2 | Toolchain pins | P1.02 | `rust-toolchain.toml`, `rustfmt.toml`, `clippy.toml` | Written; commands not run |
| 3 | Workspace root | P1.03 | `Cargo.toml` (12 members, edition 2024, pinned deps, license field ABSENT per D4) | Written; Cargo.lock pending first build |
| 4 | Dependency policy + check | P1.04 | `tools/check_dependency_rule.py` (allowed/forbidden edges; ADR exception mechanism) | Written; passes the real tree |
| 5 | flexcore-contracts | P1.05 | `crates/flexcore-contracts/` (units, quality, time, ids, api_paths + tests) | Written, unbuilt |
| 6 | flexcore-domain | P1.06 | `crates/flexcore-domain/` (entities, DispatchCommand §14.3 validation, TenantContext, P0.11 state machine + property tests) | Written, unbuilt |
| 7 | flexcore-ports | P1.07 | `crates/flexcore-ports/` (repository/outbox/clock/id/telemetry-source/tenant-context-source ports + fakes) | Written, unbuilt |
| 8 | flexcore-application | P1.08 | `crates/flexcore-application/` (4 registry use cases, outbox-in-unit-of-work, error taxonomy) | Written, unbuilt |
| 9 | flexcore-events | P1.09 | `crates/flexcore-events/` (26 typed events, envelope, outbox record, P1.14 subject scheme) | Written, unbuilt |
| 10 | flexcore-storage + flexcore-audit | P1.10 | `crates/flexcore-storage/`, `crates/flexcore-audit/` (UnitOfWork, TransactionalWrite, retention config; AuditRecord + append-only AuditSink) | Written, unbuilt |
| 11 | flexcore-authz | P1.11 | `crates/flexcore-authz/` (deny-by-default PolicySet, scopes, decisions; NOT the dispatch authorizer) | Written, unbuilt |
| 12 | Schema design | P1.12 | `ledger/phase-01-rust-foundation/12_schema_design.md` (decisions D-S1…D-S5) | Frozen design |
| 13 | Postgres adapter + migrations | P1.13 | `crates/adapters/postgres/` (8 migrations, RLS + roles, PgUnitOfWork, tests gated on FLEXCORE_TEST_DATABASE_URL) | Written, unbuilt |
| 14 | JetStream topology | P1.14 | `ledger/phase-01-rust-foundation/14_jetstream_topology.md` (D-J1…D-J6, 26-event subject table) | Frozen design |
| 15 | NATS adapter + publisher + consumers | P1.15, P1.16 | `crates/adapters/nats/` (publisher, IdempotentConsumer, InboxStore trait, DLQ, tests gated on FLEXCORE_TEST_NATS_URL) | Written, unbuilt |
| 16 | Control-plane API | P1.17 | `apps/control-plane/` (Axum routes: tenants/portfolios/sites/assets CRUD+list+get, authz per route, OpenAPI, /health //ready //metrics) | Written, unbuilt |
| 17 | Buf governance | P1.18 | `proto/` (decision recorded: protos land in Phase 7) | Written |
| 18 | Worker | P1.19 | `apps/worker/` (publisher/consumer hosting, PgInboxStore, probes, graceful shutdown) | Written, unbuilt |
| 19 | Observability / config / probes | P1.20–P1.22 | control-plane + worker modules (tracing, metrics, config fail-fast, probes, drain) | Written, unbuilt |
| 20 | Security + fault test suites | P1.23, P1.24 | `crates/flexcore-authz/tests/security_authz.rs`, `crates/flexcore-domain/tests/security_tenant_context.rs`, `crates/flexcore-application/tests/security_isolation.rs`, `crates/flexcore-ports/tests/security_vendor_free.rs`, `crates/adapters/{nats,postgres}/tests/fault_slice.rs`, coverage note `23_24_test_coverage_note.md` | Written, unbuilt |
| 21 | Compose stack | P1.25 | `deploy/compose/` (postgres 18.6, nats 2.15.2, Dockerfiles, smoke.sh, README) | Written, not run |
| 22 | Exit-gate record | P1.26 | `ledger/phase-01-rust-foundation/26_exit_gate_record.md` | NOT RUN — runbook only |
| 23 | CI workflow | P1.27 | `.github/workflows/ci.yml` (+ gate inventory in its header) | Written, not run |
| 24 | License inventory (D4 input) | P1.28 | `ledger/phase-01-rust-foundation/28_dependency_license_inventory.md` (D4 OPEN) | Written |

## 2. Implemented-API-subset gap (consumed by P2.12)

Implemented routes: `POST/GET /api/v1/tenants`, `/portfolios`, `/sites`,
`/assets` (create/list/get-by-id) + `/health`, `/ready`, `/metrics`,
`/api/v1/openapi.json`. Everything else in the P0.15 16-resource list —
including DELETE, telemetry query, current-state, quarantine, forecasts,
schedules, dispatch — is Phase 2+ work.

## 3. Phase 2 entry conditions

- Stage 1 exit gate: **not held** (recorded exception — `ENTRY_NOTE.md`);
  P1.26 runbook not executed. The human must run the P1.26 demonstration
  before any release claim.
- D4 inventory delivered; D4 itself OPEN (no license decision made).
- Capability-profile references exist on assets; their contents are Phase 3.

## 4. Open questions Phase 2 must not answer by itself

| Question | Owning gate |
|---|---|
| D1 jurisdiction/market | product + legal (scheduled) |
| D2 first portfolio/service + baseline method | product (scheduled; §11.4 baseline depends on D1/D2) |
| D3 network class/fidelity | power systems (scheduled; Phase 4 gated) |
| D4 license audit | legal + engineering (inventory delivered, decision open) |
| D5 first protocol/asset mix | product + integration (before v0.4.0) |
| D6 cyber/OT boundary | security (before any read-only pilot) |
| Retention values | deployment configuration, never code (P0.16) |
| Named gate owners + dates | Stage 0/1 gate meetings |
