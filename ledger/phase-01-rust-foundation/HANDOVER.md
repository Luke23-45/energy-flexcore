# HANDOVER — Phase 1: Modular Rust Foundation

Status: **closed (code written, nothing built or tested)** — see below. The
batch plan is at the bottom.

## What was implemented

All 32 subtasks (P1.01–P1.32) have their deliverables written. Full index:
`29_handover_pack.md` (artefact × location × status). Summary:

- **Workspace**: root `Cargo.toml` (12 members, edition 2024, pinned deps
  from `RESEARCH.md`, license field absent per D4), `rust-toolchain.toml`
  (1.99.1), `rustfmt.toml`, `clippy.toml`, `tools/check_dependency_rule.py`
  (P1.04 enforcement; passes the real tree).
- **Core crates**: flexcore-contracts (P0.07–P0.09 types + FlexUuid),
  flexcore-domain (§10.1 Stage 1 entities, DispatchCommand §14.3
  validation, TenantContext via `AuthenticatedClaims::from_verified`,
  P0.11 state machine with frozen-edge property tests), flexcore-ports
  (sync port traits + fakes), flexcore-application (4 registry use cases,
  outbox-in-unit-of-work, error taxonomy), flexcore-events (26 events,
  envelope, outbox record, P1.14 subject scheme), flexcore-storage +
  flexcore-audit (UnitOfWork, TransactionalWrite, retention config,
  append-only AuditSink), flexcore-authz (deny-by-default PolicySet).
- **Adapters**: postgres (8 migrations, RLS + roles flexcore_migrator/
  flexcore_app, PgUnitOfWork, FlexUuid↔PG text mapping, gated integration
  tests), nats (JetStream publisher with Nats-Msg-Id dedup, ack-before-
  mark, IdempotentConsumer + InboxStore + DLQ, gated integration tests).
- **Apps**: control-plane (Axum routes for tenants/portfolios/sites/assets,
  per-route authz, 401/403/404/422 envelope, OpenAPI, /health /ready
  /metrics, metrics middleware with trace-id JSON logs, config fail-fast,
  graceful shutdown, `dev-auth` feature NOT default), worker (publisher +
  consumer loops, PgInboxStore, probes, shutdown, integration test).
- **Deploy/CI**: `deploy/compose/` (postgres 18.6, nats 2.15.2, Dockerfiles,
  smoke.sh), `proto/` (Buf governance; protos land Phase 7),
  `.github/workflows/ci.yml` (§22.2 gates + dev-auth release gate).
- **Tests**: unit + property tests in every crate; security suite
  (deny-by-default, tenant isolation, vendor-free ports); fault-injection
  slices (duplicate/reorder/outage/poison/DB-failure/clock-skew).
- **Design docs**: `12_schema_design.md` (D-S1…D-S5), `14_jetstream_topology.md`
  (D-J1…D-J6), `23_24_test_coverage_note.md`, `26_exit_gate_record.md` (NOT
  RUN), `28_dependency_license_inventory.md` (D4 input), `30_boundary_audit.md`.

## How to build and test it

```sh
cargo generate-lockfile && git add Cargo.lock     # first lockfile (P1.03)
cargo test --workspace                            # unit tests (no services needed)
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
python tools/check_dependency_rule.py
# integration (needs services; Compose stack provides them):
docker compose -f deploy/compose/docker-compose.yml up --build
FLEXCORE_TEST_DATABASE_URL=postgres://flexcore_dev:flexcore_dev_password@localhost:5432/flexcore \
FLEXCORE_TEST_NATS_URL=nats://localhost:4222 cargo test --workspace
# then the P1.26 demonstration runbook: ledger/phase-01-rust-foundation/26_exit_gate_record.md
```

## Blockers

1. **P1.26 demonstration NOT RUN** (loop bans) — runbook in
   `26_exit_gate_record.md`; the Stage 1 gate cannot be recorded without it.
2. **P1.31 exit-gate review NOT HELD** — human reviewers required
   (`31_exit_gate_review.md`); ADRs stay "Proposed" until Stage 0/1 gates pass.
3. **Cargo.lock** not generated (requires cargo) — human runs
   `cargo generate-lockfile` and commits (P1.03 criterion partial).
4. **Transitive license inventory** pending the first lockfile (P1.28 partial).

## Decisions still needed

- D1–D6 (all SCHEDULED; `.agent/DECISIONS.md`) — no jurisdiction, protocol,
  license, or network-class decision was made.
- Named gate owners + dates (gate meetings).
- Retention values (deployment configuration, never code).

## Assumptions

- Pinned deps (tokio 1.54.1, axum 0.8.9, sqlx 0.9.0, async-nats 0.50.0,
  tracing 0.1.44, tracing-subscriber 0.3.23, utoipa 5.5.0, tower 0.5.3,
  tower-http 0.7.0; serde/serde_json/time resolved at first lockfile) were
  written into manifests from `RESEARCH.md`; nothing was downloaded/built.
- The UUID crate was NOT added (unpinned); `flexcore-contracts` implements
  a minimal std-only FlexUuid; generation via the IdGenerator port.
- `AuthenticatedClaims::from_verified(VerifiedClaims)` added by coordinator
  (the single public construction path; verification is procedural at the
  reviewed API-middleware call site).
- Event subject areas reconciled to the frozen P1.14 table (8 events moved);
  4 stale literals updated.
- DB readiness check = begin+commit on a blocking thread (adapter has no
  ping); broker readiness = core request round-trip (VERIFY-BUILD noted).
- NATS integration-test `FixedClock::new(0)` misuse fixed to
  `FixedClock::at_unix_millis(0)` (coordinator).

## Known risks

1. **Everything is unbuilt.** The first `cargo build` is the real proof;
   async-nats 0.50 API details carry explicit `VERIFY-BUILD:` comments.
2. **Rust 1.100 (12 Nov 2026)** — do not bump the toolchain pin without the
   recorded release-note + full-workspace-beta-test procedure (RESEARCH §8).
3. **dev-auth**: never enable outside dev; CI fails if it becomes default.
4. **Gate debt**: Stage 0 + Stage 1 gates unheld — must be recorded before
   any release claim.

## Batch plan (written before delegation, per loop rules)

| Batch | Subtasks | Files owned | Notes |
|---|---|---|---|
| 0 (coordinator) | P1.01 | `ENTRY_NOTE.md` | Gate NOT held; loop-rules exception recorded |
| 0 (coordinator) | P1.02–P1.04 | toolchain files, root `Cargo.toml`, `tools/check_dependency_rule.py` | Shared manifests coordinator-only |
| 1 | P1.05, P1.09 (+ domain restructure) | `crates/flexcore-contracts/**`, `crates/flexcore-domain/**`, `crates/flexcore-events/**` | Value types → contracts; events out of domain |
| 1 | P1.12, P1.14 | `12_schema_design.md`, `14_jetstream_topology.md` | Design docs |
| 2 | P1.06 | `crates/flexcore-domain/**` | Entities + invariants + property tests |
| 2 | P1.10 | `crates/flexcore-storage/**`, `crates/flexcore-audit/**` | |
| 3 | P1.07, P1.08 | `crates/flexcore-ports/**`, `crates/flexcore-application/**` | |
| 3 | P1.11 | `crates/flexcore-authz/**` | |
| 4 | P1.13 | `crates/adapters/postgres/**` | Implements P1.12 design |
| 4 | P1.15, P1.16 | `crates/adapters/nats/**` | Publisher + consumers/inbox |
| 5 | P1.17, P1.20–P1.22 | `apps/control-plane/**` | API + observability + probes (integration agent) |
| 5 | P1.19 | `apps/worker/**` | Worker app |
| 5 | P1.18, P1.25 | `proto/**`, `deploy/compose/**` | Coordinator |
| 6 | P1.23, P1.24 | `crates/*/tests/security_*.rs`, `fault_slice.rs`, coverage note | New test files only |
| 7 | P1.26, P1.27, P1.28 | `26_exit_gate_record.md`, `.github/workflows/ci.yml`, `28_dependency_license_inventory.md` | Coordinator |
| close | P1.29–P1.32 | close-out docs, ledger ticks, this file | Coordinator |
