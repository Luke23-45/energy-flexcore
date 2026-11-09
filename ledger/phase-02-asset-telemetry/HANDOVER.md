# HANDOVER — Phase 2: Asset and Telemetry Foundation

Status: **in progress** (batch plan below; completed at phase close).

## Batch plan (written before delegation, per loop rules)

Max 2 subagents at a time; file ownership disjoint per batch.

| Batch | Subtasks | Files owned | Notes |
|---|---|---|---|
| 0 (coordinator) | P2.01 | `ENTRY_NOTE.md` | Stage 1 gate NOT held (human reviewers); loop-rules exception recorded like Phase 1's; API-subset gap recorded |
| 1 | P2.02, P2.03 | `02_registry_domain_model.md`, `crates/flexcore-domain/**`, `crates/flexcore-application/**` | Registry design + domain/application implementation with outbox-only events |
| 1 | P2.04, P2.05 | `04_telemetry_model.md`, `05_telemetry_storage_design.md` | Canonical telemetry model + partitioned-storage design docs |
| 2 | P2.06 | `crates/adapters/postgres/**` (telemetry migrations + repository) | Extends, never recreates, Phase 1 tables |
| 2 | P2.07, P2.08 | `07_ingestion_pipeline_design.md`, `crates/flexcore-application/src/quality.rs` (new module) or a dedicated quality module home | Pipeline design + quality/stale/gap detector |
| 3 | P2.09, P2.10 | dedup module + ingestion service (worker task) | Dedup identity decision + pipeline implementation |
| 3 | P2.11, P2.13 | `flexcore-ports` TelemetrySource contract extension + `scenarios/synthetic-portfolio/**` | Port definition + scenario model/versioning |
| 4 | P2.14, P2.15 | simulator provider crate + fault-injection scenario config | Deterministic seeded generation + 7 fault types |
| 4 | P2.12, P2.17 | `apps/control-plane/**` API extensions + registry views | CRUD completion + telemetry query + quarantine endpoint |
| 5 | P2.16, P2.18 | event wiring + current-state view | Outbox-only, batch granularity decision |
| 5 | P2.19, P2.20 | historical view + retention/partition maintenance | Cursor pagination + manual/approved retention |
| 6 | P2.21, P2.22 | observability extensions + security test slice | Telemetry metrics + per-route isolation tests |
| 6 | P2.23, P2.24 | fault suite + replay entrypoint (`apps/sim-runner/`) | Deterministic replay + comparison excluding ingest_time |
| 7 | P2.25, P2.31 | exit portfolio scenario + versioned corrections/as-of | ≥2 sites × 4 asset types + meter; corrections append-only |
| 7 | P2.26, P2.27 | exit-gate record + CI extensions | Runbook + record (NOT RUN); scenario regression gates |
| close (coordinator) | P2.28, P2.29, P2.30 | `28_handover_pack.md`, `29_exit_gate_review.md`, `30_anti_scope_audit.md`, ledger ticks, this file | |

## What was implemented

(in progress — updated per batch; nothing built or tested, see the phase-close
sections when filled)

- **Batches 0–4 (P2.01–P2.15)**: done. Registry domain model + application use
  cases (outbox-only events, RESTRICT deletion guards); canonical telemetry
  model (D-T1…T6) + partitioned storage design (D-P1…P7); telemetry migrations
  + partitioned repository with savepoint-based exact-vs-conflicting duplicate
  classification and quarantine; 7-stage ingestion pipeline in the worker
  (quality via `flexcore_domain::quality_rules`, dedup via
  `TelemetryDeduplicator`, outbox-only events, bounded channel 10,000);
  `TelemetrySource` port contract; scenario model + `scenario-v1.json`;
  `crates/flexcore-simulator` (deterministic splitmix64, BESS/PV/load/EVSE/
  meter profiles, 7 fault types via scenario config, disabled by default,
  24 tests); API extensions (registry CRUD completion, telemetry query,
  current state, quarantine read, keyset cursor pagination).
- **Batch 5 (P2.16, P2.18)**: done. Event granularity DECIDED ONCE: per-record
  `TelemetryReceived` with a deterministic idempotency key (recorded in
  `apps/worker/src/ingestion.rs` header); `TelemetryQualityChanged` with
  previous+new states; availability rule DECIDED ONCE in
  `crates/flexcore-application/src/availability.rs` (SUSPECT-tolerant boundary)
  → `AssetAvailabilityChanged`; current-state view (latest by `event_time`,
  freshness + quality + age + `simulated` flag) at
  `GET /api/v1/assets/{id}/state`.
- **Workspace**: 14 members — `crates/flexcore-simulator` and
  `apps/sim-runner` added to `members` (root `Cargo.toml`) and to
  `COMPOSITION_CRATES`/`MEMBER_DIRS` in `tools/check_dependency_rule.py`
  (check passes: "OK — dependency rule holds for 14 workspace members").
  `flexcore-simulator` added to `[workspace.dependencies]`; control-plane
  gained a `sqlx` dev-dependency for the DB-gated integration tests;
  worker gained `flexcore-simulator` for composition (P2.10 step 5).
- **Batch 5b (P2.19, P2.20)**: done. Historical view
  (`GET /api/v1/telemetry/history`, keyset cursor D-H1, raw + `date_bin`
  count/mean/min/max D-H2, limits D-H3, pruning predicates D-H4) +
  partition maintenance (scheduled pre-creation D-R1, detach-first with
  separately-approved drop D-R2, catalog-bounds eligibility D-R3,
  unconfigured-retention visibility D-R4, same-transaction audit D-R5);
  migration 0011 (API read grant). Decisions in
  `19_20_historical_retention.md`.
- **Batch 6 (P2.23, P2.24)**: done. 8-case fault-injection suite
  (`apps/worker/tests/fault_suite.rs`, fault-first assertions) +
  `apps/sim-runner` replay CLI (`run`/`replay`, comparison on the frozen
  P2.24 tuple — `ingest_time` structurally absent, diff report, exit codes
  0/1/2). Decisions in `23_24_fault_replay.md`.
- **Batch 7 (P2.25, P2.31)**: done. `scenario-v2.json` (expected-quality
  plan tied to the fault types), human runbook `25_exit_gate_runbook.md`,
  coverage/fault accounting tests; foundations §6.4 versioned corrections
  (migration 0012, `corrections.rs` append-only versions, as-of reads,
  `as_of` API params, `sim-runner as-of` decision-snapshot replay).
  Decisions in `25_31_exit_portfolio_corrections.md`.
- **Batch 7b (P2.27)**: done. CI: `migration-tests` (PG 18 service),
  `scenario-regression` (asserts the replay REPORT verdict), 
  `telemetry-contract` (generic provider suite in `flexcore-ports`);
  gate inventory in `27_ci_gate_inventory.md`.
- **Coordinator follow-up (P2.10/P2.24/P2.27 wiring)**: done. Worker
  `compose_ingestion` implemented (scenario all-or-nothing load, tenant +
  asset mapping via the repositories — a missing key fails, never a
  reduced portfolio; `FLEXCORE_SIMULATOR_*` config; port-only rule kept);
  source bridge spawned at startup; contract suite instantiated against
  the real simulator in `apps/sim-runner/tests/contract.rs` (non-vacuous
  fault variants included). Decision note: `24_worker_source_wiring.md`.
  This closes the runbook §0 blocker and the `27_ci_gate_inventory.md`
  follow-up in code.
- **P2.26**: exit-gate record written as **NOT RUN** with the readiness
  table + demonstration sequence (`26_exit_gate_record.md`) — the human's
  run fills it (same class as the Phase 1 gate debt).

## How to build and test it

```sh
cargo generate-lockfile                       # if not yet done in Phase 1
cargo test --workspace                        # unit tests (no services)
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
python tools/check_dependency_rule.py         # 14 members, exit 0
# integration (Compose stack: postgres 18.6 + nats 2.15.2):
docker compose -f deploy/compose/docker-compose.yml up --build
FLEXCORE_TEST_DATABASE_URL=postgres://flexcore_dev:flexcore_dev_password@localhost:5432/flexcore \
FLEXCORE_TEST_NATS_URL=nats://localhost:4222 cargo test --workspace
# P2.26 exit-gate demonstration (fills 26_exit_gate_record.md):
#   ledger/phase-02-asset-telemetry/25_exit_gate_runbook.md
# EXPLAIN evidence (P2.19 D-H4, P2.31 visibility predicate): run the
# representative queries from 19_20_historical_retention.md against the stack.
```

## Blockers

1. **P2.26 demonstration NOT RUN** (loop bans) — runbook in
   `25_exit_gate_runbook.md`; record in `26_exit_gate_record.md`.
2. **P2.29 review NOT HELD** — human reviewers required
   (`29_exit_gate_review.md`); replay report is a required exhibit.
3. **CI never executed** (loop bans) — the three new jobs
   (migration-tests, scenario-regression, telemetry-contract) are written,
   unrun; VERIFY-CI items in `27_ci_gate_inventory.md`.
4. **EXPLAIN evidence** (P2.19/P2.31) — human runs the recorded queries.
5. **First `cargo build`** is the real proof for all Phase 2 code;
   VERIFY-BUILD comments mark the spots (sqlx `date_bin` binding,
   `relpartbound` parsing, `connect_timeout` URL param, 0012 up/down).

## Decisions still needed

- D1–D6 (all SCHEDULED; `.agent/DECISIONS.md`) — Phase 3 must not answer
  D1–D3 itself (`28_handover_pack.md` §4).
- Baseline method (depends on D1/D2); retention values (deployment
  configuration, D-R4).
- Named gate owners + dates for the Stage 2 review.

## Assumptions

- Pinned-dep policy unchanged from Phase 1; no new external dependencies
  were introduced in Phase 2 (sqlx/tokio reuse; `date_bin` is SQL, not a
  crate).
- Correction linkage has NO database FK (`correction_of` is
  procedure-enforced in the same transaction) — recorded in
  `25_31_exit_portfolio_corrections.md`.
- `AssetAvailabilityChanged` keeps its frozen payload shape; consumers
  re-derive availability from telemetry.
- Per-record `TelemetryReceived` granularity (frozen single-asset payload)
  — revisit as a schema-version change only.
- The telemetry-contract CI job instantiates the generic suite against an
  in-ports deterministic stand-in; the live-simulator instantiation is
  the worker-wiring batch's sim-runner test (see `24_worker_source_wiring.md`).

## Known risks

1. **Everything is unbuilt** — the first `cargo build` is the real proof;
   VERIFY-BUILD comments carry the flagged spots.
2. **Gate debt**: Stage 0 + 1 + 2 gates unheld — must be recorded before
   any release claim (v0.1.0).
3. **Cardinality**: telemetry metrics use closed label vocabularies
   (`27_ci_gate_inventory.md`/`21_22_observability_security.md`); a new
   quarantine reason class or event type is a code change, never a free
   label.
4. **Corrections are append-only by procedure**: the DB has no FK/FORCE
   guard against raw-SQL in-place updates; RLS + roles are the guard.
5. **dev-auth / maintenance URL**: the worker's maintenance path needs a
   privileged (BYPASSRLS test-analogue) URL — keep it out of the app role.
