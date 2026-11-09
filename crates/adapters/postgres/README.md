# flexcore-postgres-adapter

P1.13 — the PostgreSQL adapter for Neryva FlexCore Stage 1: the P1.12 schema
(`migrations/`), SQLx implementations of the P1.07 repository/outbox and
P1.10 audit ports, transaction-scoped tenant context with row-level
security, and the integration test suite.

P2.06 — the Phase 2 telemetry storage: the partitioned `telemetry` table and
`telemetry_quarantine` (migration 0010, implementing P2.05 D-P1..D-P7), the
additive P2.03 registry extension (migration 0009), the telemetry repository
on `PgUnitOfWork`, and the ingest/query role model with per-role RLS tests.

Simulation-only baseline (report §5.7 / AGENTS.md §3): nothing here grants
control authority or writes to field devices.

## What is implemented

| Port (single home) | Implementation |
|---|---|
| `TenantRepository`, `PortfolioRepository`, `SiteRepository`, `AssetRepository` (flexcore-ports) | `PgUnitOfWork` — every statement inside the unit of work's one native transaction |
| `EventOutbox::append` (flexcore-ports) | `PgUnitOfWork` — the state change and the outbox row share the same transaction (report §6.3 step 1) |
| `EventOutbox::{pending, record_attempt, mark_delivered, mark_failed}` (flexcore-ports) | `FlexCoreStore` (publisher side, P1.15); its `append` deliberately fails closed — a transactional append without a unit of work would be a dual-write hole |
| `AuditSink` (flexcore-audit, append-only) | `PgUnitOfWork` (inside the transaction) and `FlexCoreStore` (self-scoping autocommit append) |
| `Clock` (flexcore-ports) | `SystemClock` (platform UTC) |
| `IdGenerator` (flexcore-ports) | `PostgresIdGenerator` — `SELECT uuidv7()` |
| Telemetry repository (P2.06; no port trait — the P2.11 contract is landing concurrently) | `PgUnitOfWork::{insert_telemetry, insert_telemetry_batch, query_telemetry, latest_telemetry_per_asset, quarantine_telemetry, list_telemetry_quarantine}` — all inside the unit-of-work transaction, all tenant-scoped |

## Telemetry storage (P2.06)

Implements `ledger/phase-02-asset-telemetry/05_telemetry_storage_design.md`
(P2.05, decisions D-P1..D-P7) exactly; migration 0010, one migration for the
table + indexes + RLS + grants so the table is never momentarily without its
policy (P1.12 §7):

- **One partitioned table** (D-P1) with a `quantity` column; PK
  `(asset_id, event_time, source_sequence, quantity)` (D-P3 — includes the
  partition key, which PostgreSQL requires). `telemetry_id` is a `uuidv7()`
  surrogate, not unique-constrained and never the identity (P2.04 D-T3).
- **`PARTITION BY RANGE (event_time)`, monthly partitions** (D-P2), named
  `telemetry_yYYYYmMM` with bounds `[first of month, first of next month)`.
  No partition is created by the migration: pre-creation ahead of the ingest
  frontier is the P2.20 maintenance job, and an empty parent keeps the
  out-of-range error path testable from the first insert. The honest
  partitioning note (P2.05 §4.2) applies verbatim: the mechanics are proven
  by tests; no volume win is claimed.
- **Composite DESC indexes** on `(asset_id, event_time)`,
  `(tenant_id, asset_id, event_time)` and `(asset_id, quantity, event_time)`.
  No latest-value partial index and no materialized current-state table in
  Phase 2 (D-P4).
- **Retention is not in the schema** (D-P5): no retention column, TTL
  trigger, or scheduled-delete rule anywhere; retention is configuration per
  §16.3 data class, executed as detach/drop of whole partitions by P2.20.
- **Roles** (D-P6): `flexcore_ingest` (INSERT+SELECT on
  `telemetry`/`telemetry_quarantine`) and `flexcore_query` (SELECT only) are
  distinct, created NOLOGIN in a guarded DO block like the Phase 1 roles;
  neither owns any table. Both are subject to the P1.12 §5.1
  `tenant_isolation` policy via `ENABLE` + `FORCE ROW LEVEL SECURITY` — an
  unset/empty `flexcore.tenant_id` fails closed.
- **TimescaleDB is not used** (D-P7); revisit trigger is measured volume.
- **Quarantine** (`telemetry_quarantine`, P2.07 §3): tenant-scoped evidence
  rows (stage + rule, raw payload JSONB, source identity if present,
  attempt count). `asset_id` is nullable and carries deliberately NO FK —
  an unresolvable asset id is itself a quarantine reason.
- **New enums** `reactive_sign_convention` (`INJECTION_POSITIVE` /
  `CONSUMPTION_POSITIVE`) and `clock_quality` (`SYNCHRONIZED` /
  `UNSYNCHRONIZED` / `UNKNOWN`) — D-T4/D-T5; frozen vocabularies, no
  default Q convention.
- **Registry extension** (migration 0009, additive `ALTER TABLE` only):
  `assets.lifecycle_status` (`'active'` default), `deactivation_reason`,
  `deactivated_at` + the terminal-state consistency CHECK — the storage
  columns the P2.03 domain additions require. The Phase 1 tables are never
  recreated.
- **Outbox unchanged**: the P2.07 telemetry flow publishes
  `TelemetryReceived`/`TelemetryQualityChanged` through the existing
  `outbox` columns; no Phase 2 column was needed, so nothing in 0007 was
  touched (P2.06 step 1 records this explicitly).

### Telemetry repository decisions

- **Crate-local record type.** `NormalizedTelemetryRecord` mirrors the
  P2.04 §3 field set instead of binding to
  `flexcore_ports::telemetry_source::NormalizedTelemetry`, because the P2.11
  port contract is being implemented concurrently and its exact type may
  shift. The mapping is 1:1 field-for-field (documented in the
  `src/telemetry.rs` module header): whoever lands the P2.11 ingest path
  maps `NormalizedTelemetry` onto this record mechanically, or replaces the
  record with the port type. `Quantity` mirrors
  `TelemetryQuantity`; `TelemetryValue` mirrors `QuantityValue` (the
  reactive convention travels as its own field/column, P2.04 D-T4); the
  `unit` column is derived from the quantity and can never disagree.
- **Duplicate handling (P2.09)**: the insert is a plain `INSERT` — no
  `ON CONFLICT`. A PK collision is classified inside a `SAVEPOINT` (so the
  surrounding unit-of-work transaction stays usable): a colliding row that
  is identical in every value-bearing field is an
  **exact duplicate** → `ExactDuplicateAcknowledged`, no second row; same
  identity with different content is a **conflicting duplicate** →
  `TelemetryError::ConflictingDuplicate` — never overwritten, quarantined
  by the caller (P2.10) via `quarantine_telemetry`. A different quantity on
  the same sample identity is a legitimate multi-quantity row (P2.04 D-T2).
  Constraint classification is by constraint name (`telemetry_pkey`), never
  message text.
- **Out-of-range `event_time`**: PostgreSQL raises its partition-routing
  error (SQLSTATE 23514, no constraint name — recorded assumption, verified
  by the partition test); the repository maps it to
  `TelemetryError::OutOfRangeEventTime` so the pipeline quarantines and
  counts the record (P2.05 §7 item 2). The savepoint rollback keeps the
  unit of work usable afterwards (tested).
- **Repository-boundary re-checks**: tenant match against the context
  (`TenantMismatch` before any SQL), `ingest_time >= event_time` (P1.05
  semantics re-checked per P2.05 §3), value totality, quantity/value-kind
  pairing, SOC fraction range, finiteness, non-negative `bigint` source
  sequence — all enforced again at the database by CHECKs, so a bypassing
  writer is still rejected.
- **Reads**: `query_telemetry` (tenant-scoped asset set + inclusive
  `from`/`until` + named quantities, no wildcard), `latest_telemetry_per_asset`
  (`DISTINCT ON (asset_id, quantity)` by latest `event_time` — the P2.18
  read rule, never latest arrival). Both filter by the RLS GUC (set by
  `begin(Some(&ctx))`) and the explicit `tenant_id` column (defence in
  depth, the P1.13 pattern).

## Recorded decisions

## Recorded decisions

### Runtime queries (not compile-time macros)

All SQL runs through `sqlx::query` (runtime-checked). The compile-time
`query!` macros require generated sqlx-data (a build step against a live
database); the adapter instead keeps SQL beside the code it serves and
verifies it in the integration suite against real PostgreSQL 18. Offline
mode (`sqlx-data`) can be introduced later without changing call sites.

### Sync ports bridged onto async sqlx

The P1.07 port traits are sync by recorded rule. Each `FlexCoreStore` owns
a private Tokio runtime; the sync port methods block on it. Callers inside
an async service must invoke the ports from a blocking context (e.g.
`tokio::task::spawn_blocking`).

### FlexUuid ↔ PostgreSQL uuid mapping

Identifiers travel as canonical hyphenated **text**: bound as text
parameters and cast with `::uuid` in the SQL statement, selected back with
`::text`. The workspace sqlx pin does not enable the `uuid` feature, and
this mapping keeps the contracts `FlexUuid` (`[u8; 16]`) the single
representation of record — the database only ever sees a canonical UUID
value because the cast rejects anything else. `correlation_id` on
`audit_records` is `text` per the P1.12 column design and carries the same
canonical text form.

### Tenant context

`FlexCoreStore::begin(Some(&ctx))` issues
`SELECT set_config('flexcore.tenant_id', $1, true)` on the unit of work's
transaction — SET LOCAL semantics, the variable name fixed by P1.12 §5.1.
The P1.12 policies read that setting; an unset/empty setting fails closed
(zero rows, no writes). Tenant-scoped queries additionally filter by the
context's tenant, and writes carrying a foreign-tenant entity are refused
with `TenantMismatch` before any SQL runs. `TenantRepository` carries no
context by recorded port decision (the tenant root is not tenant-owned
data, P1.12 §4.1).

### Unit of work

`begin` opens the native `sqlx::Transaction`; `commit`/`rollback` finalize
exactly once and return the flexcore-storage `FinalizedUnitOfWork`
(P1.10 vocabulary); dropping an unfinalized unit of work spawns a rollback
of its transaction on the store's runtime. The §6.3 step-1 atomicity rule
holds by construction: the repository write that produced the
`StateChangeReceipt` and the `EventOutbox::append` that consumes it run on
the same transaction.

### Outbox column mapping

`event_type` and `schema_version` are classified structurally from the
serialized envelope payload (`schema_version` from the envelope field,
`event_type` from the internally tagged event discriminant, per the P1.09
serialization rule); a payload that does not name both is refused
(`E_OUTBOX_PAYLOAD`) — the columns are NOT NULL and never guessed. The
payload is stored verbatim as jsonb. `created_at` comes from the port's
`EventTime`; the wrapped event's own `event_time` stays inside the payload
(P1.12 §2.2 — the P0.09 distinction is never silently lost).

### Error opacity

Every sqlx failure maps to a stable adapter code plus a fixed class message
(`error.rs`); error text is never inspected or forwarded. Uniqueness
violations map to `RepositoryError::Conflict` / `EventOutboxError::DuplicateKey`.

## Role model

| Role | Purpose |
|---|---|
| `flexcore_migrator` | Table owner; runs migrations and partition-management DDL (P2.20); never used at runtime |
| `flexcore_app` | Runtime role (control plane + worker); **never the owner** (P1.12 §6, [R25] caveat); `SELECT/INSERT/UPDATE` on registry/state/outbox tables, `SELECT/INSERT` only on `audit_records`; no `DELETE`/`TRUNCATE`/DDL anywhere |
| `flexcore_ingest` (P2.06, D-P6) | Telemetry ingestion runtime role; `INSERT, SELECT` on `telemetry` and `telemetry_quarantine` only |
| `flexcore_query` (P2.06, D-P6) | Telemetry query/API runtime role; `SELECT` only — no write path exists for it |

Recorded deviation from P1.12 §3.1: the two roles are created by migration
0001 inside guarded DO blocks (idempotent, `NOLOGIN`, no credentials ever in
a migration) so the migration set is self-contained for CI. Granting LOGIN
and credentials remains a deployment-bootstrap step — in tests, the harness
runs `ALTER ROLE ... LOGIN PASSWORD` against the test database with
test-only passwords. The migration user needs `CREATEROLE` (or superuser)
for the DO block; a privilege failure there is intentional fail-fast.

Append-only enforcement for `audit_records` (three layers, P1.12 §4.7):
grants (INSERT/SELECT only), a BEFORE UPDATE OR DELETE trigger that raises
unconditionally — including for the owner — and the append-only port.

## Test harness: CI service, not testcontainers

Recorded decision (CONTEXT.md open question 5): the integration tests run
against a CI **PostgreSQL 18 service** (P1.27), mirroring the Compose stack
(P1.25). No container runtime is created by the tests.

### Running the tests

```sh
# 1. Provide a PostgreSQL 18 server and a test database (user may run DDL
#    and needs CREATEROLE for the guarded role block):
export FLEXCORE_TEST_DATABASE_URL=postgres://postgres:postgres@localhost:5432/flexcore_test

# 2. Run the crate tests; without the variable every integration test
#    skips cleanly and only the DB-free unit tests run:
cargo test -p flexcore-postgres-adapter
```

Coverage: migrations up/down/up (in a dedicated temporary database), RLS
isolation for the runtime role and the owner role, outbox atomicity with
mid-transaction failure injection, SOC [0,1] constraint rejection, seventh
quality-value rejection, and audit append-only/tenant scoping. Test data
uses run-unique identifiers, so repeated runs against a persistent
database do not collide (rows accumulate; retention is an operations
concern, never schema — D-S3).

P2.06 coverage (`tests/telemetry.rs`): partition routing (a row lands in
its month's partition child table), partition pruning (`EXPLAIN` for a
time-bounded query scans the in-range partition and prunes the others),
RLS isolation for `flexcore_ingest` and `flexcore_query` per tenant
(including the no-DELETE/no-INSERT privilege model), duplicate-PK
classification (exact vs conflicting vs sibling quantity), the typed
out-of-range `event_time` path with savepoint recovery, batch insert with
per-record outcomes, the latest-value-per-asset query, and quarantine
write/list tenant scoping. Test months are fixed constants (2030/2031),
so the tests never collide with `now()`-derived partitions.

## Recorded deviations from the P1.12 design

- **`name`/`description` columns** on `tenants` (description), `portfolios`,
  `sites`, `assets`: the P1.07 repository ports must round-trip the P1.06
  domain entities, which carry these descriptive fields; P1.12 §4 lists
  only the P0.10 key fields. Additive, does not touch D-S1..D-S5.
- **`UNIQUE (idp_reference)`** on `tenants`: the port documents the
  one-tenant-per-identity-provider-reference rule as a uniqueness anchor.
- **Roles in a migration** (see role model above) instead of deployment
  bootstrap only.
- **Read reconstruction loads the parent chain** (a Site row loads its
  Portfolio, which loads its Tenant): the P1.06 entity constructors derive
  child tenant scoping from parent entities, so an entity cannot be
  reassembled without them. Rows written by this adapter always round-trip;
  rows with NULLs in fields the domain requires non-empty (e.g.
  `sites.grid_model_reference`) cannot be decoded into the domain entity —
  they are a decode failure, not silent data loss.
- **Asset registration writes the initial `asset_state` row** (D-S2, P1.12
  §4.5 Phase 1 convention): same transaction, `event_time = ingest_time`,
  `quality = 'MISSING'`, NULL measurements.

## Layout

```text
migrations/           0001..0010, up + down (P1.12 §7 discipline; 0009 = P2.03
                      registry extension, 0010 = P2.06 telemetry storage)
src/lib.rs            decisions and port/implementation map
src/error.rs          sqlx → opaque port-error mapping
src/uuid_map.rs       FlexUuid ↔ text/uuid mapping
src/enum_map.rs       asset-type and quality vocabulary mappings
src/outbox.rs         outbox classification + SQL helpers
src/audit.rs          AuditRecord → audit_records INSERT (append-only)
src/clock.rs          SystemClock
src/id_generator.rs   PostgresIdGenerator (uuidv7, D-S1)
src/store.rs          FlexCoreStore: pool, migrations, begin, publisher outbox, audit sink
src/unit_of_work.rs   PgUnitOfWork: the unit-of-work-scoped port bundle
src/telemetry.rs      P2.06 telemetry repository: partitioned-table insert/query,
                      latest-per-asset, quarantine, duplicate classification
tests/integration.rs  gated on FLEXCORE_TEST_DATABASE_URL
tests/telemetry.rs    P2.06 partition/RLS/duplicate/quarantine tests, same gate
```
