# P2.30 — What Phase 2 explicitly did NOT build, decide, or claim

Date: 2026-11-07. Referenced from the Phase 3 handover pack
(`28_handover_pack.md`, per ledger step 5).

## 1. Not built

- No flexibility envelopes or capability-model computation (Phase 3) —
  only capability-profile REFERENCES exist on assets; contents are
  Phase 3.
- No scheduling, optimization, or solver integration (Phase 3).
- No grid feasibility, no command issue/track path beyond the P1.06
  domain state machine (Phase 4).
- No operator console (Phase 5); no edge agent or protocol adapters
  (Phase 6); no plugin host/WASM (Phase 7); no Python workers; no market
  integration.
- No real site/field telemetry ingestion — the ONLY provider is the
  simulator (P2.14); source authentication is the recorded Phase 2 STUB
  (`SOURCE_AUTH_STATUS`).
- No forecast artefact of any kind (see `28_handover_pack.md` §3 — a
  stated boundary; Phase 3 consumes its own P3.31 fixture).
- No TimescaleDB, no Redis, no Kubernetes/Helm (recorded decisions stand).
- No run demonstrations: P2.26 NOT RUN; CI never executed by this work.

## 2. Not decided

- D1–D6 all remain SCHEDULED with named-owner gates open
  (`.agent/DECISIONS.md`).
- No baseline method (depends on D1/D2).
- No retention values in code — configuration only (D-R4); the
  no-retention-configured behaviour is the recorded default.
- No TimescaleDB adoption (measured-volume trigger not met; P2.05).
- No alerting thresholds (deployment concern, never hard-coded).

## 3. Not claimed

- No field-read or device-write capability; no control authority above
  Simulation (0.x is simulation-only; field write authority begins at
  v1.0.0 with operator approval).
- No conformance/certification claim for any protocol or standard.
- No performance target — P2.26 records observations only, and it has
  NOT RUN, so NO performance numbers exist at all.
- Simulator profiles are SYNTHETIC, not validated physical models.
- Nothing is "verified"/"tested" by this work: all code is written,
  unbuilt, unrun — verification is entirely the human's.

## 4. Repository state

- Added (Phase 2 only): registry domain/application extensions; telemetry
  contracts/ports; postgres telemetry migrations (0006..0012) +
  `telemetry.rs`/`corrections.rs`/`partition_maintenance.rs`; worker
  ingestion/maintenance + composition wiring; control-plane Phase 2
  routes/DTOs/OpenAPI/metrics/security + history tests;
  `crates/flexcore-simulator`; `apps/sim-runner`;
  `scenarios/synthetic-portfolio/scenario-v2.json`; CI Phase 2 jobs;
  this ledger's documents.
- Workspace now 14 members; `tools/check_dependency_rule.py` passes
  ("OK — dependency rule holds for 14 workspace members").
- Untouched: `docs/foundations/*`, the report, `.agent/*` (except the
  Phase 0 gate-status update).
- Nothing committed or pushed by this work — commits are the human's
  (Conventional Commits, AGENTS.md §10).

## 5. Known Phase 2 inconsistencies resolved during the phase

- `tenant.delete` grant (PlatformAdmin-only) vs the api.rs cascade test
  using a TenantAdmin — fixed by coordinator (PlatformAdmin caller,
  tenant-scoped; security slice pins the TenantAdmin 403).
- Phase 1's vendor-free `SOURCES` array count vs listing (compile
  blocker) — fixed by the P2.27 agent (recorded in
  `27_ci_gate_inventory.md`).
- Worker `compose_ingestion` returned `None` (recorded blocker from
  P2.25/P2.31) — composition wiring LANDED in the coordinator follow-up
  batch (simulator as first `TelemetrySource`; port-only rule kept;
  `24_worker_source_wiring.md`).
