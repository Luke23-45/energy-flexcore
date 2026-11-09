# P2.05 — Telemetry storage design: partitioned table, indexes, retention (design)

| Field | Value |
|---|---|
| Subtask | P2.05 — Design the telemetry storage: partitioned tables, indexes, retention |
| Report trace | §16.1 (partitioned telemetry tables), §16.3 (retention), §6.5 (time-series tables); `RESEARCH.md` §1 (PostgreSQL declarative partitioning), §2 (PostgreSQL 18 pin), §4 (TimescaleDB) |
| Inputs | P2.04 (canonical record model, decisions D-T1…D-T6); P1.12 (schema discipline: roles, RLS, migration discipline, D-S1/D-S3/D-S4); `ledger/phase-01-rust-foundation/RESEARCH.md` §6; `ledger/phase-02-asset-telemetry/RESEARCH.md` §1 |
| Status | Design for Phase 2 implementation (P2.06 implements exactly this; P2.09/P2.18/P2.20 consume decisions made here). No migration is written by this document |
| Decisions | D1–D6 remain OPEN (§25.2); this document decides none of them. Decisions D-P1…D-P7 below are made once for Phase 2 |

---

## 1. Scope

This document designs the PostgreSQL storage for normalized telemetry: the
partitioned table, its key and uniqueness design, indexes, partition
management, retention mechanics, row-level security, and the TimescaleDB
decision. It follows the P1.12 schema discipline (UUIDv7 surrogates, `timestamptz`
UTC, RLS with `FORCE`, roles, migration rules) and extends — never recreates —
the Phase 1 tables. The record model is P2.04; the dedup semantics are P2.09;
the maintenance implementation is P2.20.

## 2. Decisions made once

| ID | Decision | Rationale | Alternative considered and rejected |
|---|---|---|---|
| D-P1 | **One partitioned table** (`telemetry`) for all quantities, with a `quantity` column (per P2.04 D-T2, one row per `(asset_id, event_time, quantity)`) | One identity pattern, one index set, one RLS policy, one partition-management and retention path; cross-quantity queries (current-state per asset, replay comparison on `(asset_id, event_time, quantity, …)`) are single-table. Storage follows the report's default recommendation (ledger P2.05 step 1) | One table per quantity: rejected — nine quantity keys would mean nine partitioned tables, nine index/RLS/retention/maintenance paths, and multi-table joins for every per-asset query, for no selectivity gain that the composite indexes do not already provide |
| D-P2 | `PARTITION BY RANGE (event_time)`; **monthly** partitions for Phase 2, bounds `[first day of month, first day of next month)` (inclusive lower, exclusive upper — PostgreSQL range rule) | Monthly is the ledger default for the synthetic portfolio (P2.05 step 2); §3 records the volume reasoning and the production-revisit note | Daily/weekly: rejected for Phase 2 — the synthetic portfolio produces trivially small partitions, so finer intervals add catalog/plan overhead and partition-count churn with no pruning benefit monthly does not give |
| D-P3 | **Primary key: `(asset_id, event_time, source_sequence, quantity)`** — the partition key `event_time` is included (PostgreSQL requires every PK/unique constraint on a partitioned table to include all partition-key columns). `telemetry_id` is a `NOT NULL DEFAULT uuidv7()` surrogate column, **not** unique-constrained and not the PK (P2.04 D-T3). The dedup identity at sample level is `(asset_id, event_time, source_sequence)` per P2.09; §5 records exactly how uniqueness and PK interact | Satisfies the PostgreSQL rule (RESEARCH §1: PK/unique must include the partition key); encodes the P2.09 dedup identity plus the per-quantity row grain; avoids a second unique index on `(telemetry_id, event_time)` that dedup never uses | `(telemetry_id, event_time)` as PK with a second unique constraint on the business identity: rejected — two unique indexes per partition for one real constraint; `telemetry_id` ordering is prohibited anyway (P1.12 §2.3) |
| D-P4 | **No latest-value partial index** and no materialized current-state table in Phase 2. The current-state view (P2.18) is served by the `(asset_id, event_time DESC)` index (and `(asset_id, quantity, event_time DESC)` for per-quantity latest); a materialized current-state table is a later optimisation only on measured need (the P2.18 step 2 default, decided here once) | A partial index expressing "latest per asset" cannot be defined declaratively — the frontier moves per asset and per quantity; the composite DESC index already answers latest-N lookups with partition pruning. P2.18's acceptance requires `EXPLAIN` to show index use, which these indexes provide | Partial index `WHERE event_time >= <frontier>`: rejected — the frontier is not a constant and would need constant re-indexing; materialized state table now: rejected — double-write consistency cost before any measured need (P2.18 default) |
| D-P5 | **Retention is configuration per §16.3 data class** (P0.16 §4); execution is **detach of whole partitions, then drop** (or archive-first — the detach-then-drop vs archive-first choice is P2.20's recorded decision, CONTEXT open question 9). No retention value, default period, or per-class duration appears in this design, in any DDL, or in any code path | P0.16 §4 and §16.3: retention is configurable by customer contract, regional market rules, and privacy/security requirements; a value in DDL would be a universal retention period in code (P1.12 D-S3). Detach/drop of partitions is the mechanism whole-table retention maps to (RESEARCH §1: far cheaper than bulk `DELETE`, no `VACUUM` overhead) | A hard-coded default retention period: rejected — explicitly the thing §16.3 forbids |
| D-P6 | **Three roles, two of them distinct runtime roles:** `flexcore_migrator` (owner, migrations only — P1.12 §3.1), `flexcore_ingest` (telemetry `INSERT` + `SELECT`), `flexcore_query` (telemetry `SELECT` only). **Neither runtime role owns any table**; both are subject to the `tenant_isolation` RLS policy with `ENABLE` + `FORCE ROW LEVEL SECURITY` | §16.2 rules 1–4 and P0.16 §2–§3: `tenant_id` on every row, RLS as defence in depth, the runtime role must not own tenant tables, `FORCE` against the [R25] owner-bypass caveat. Distinct ingest/query roles keep the write path and read path separately grantable and separately testable (P1.12 §5.3 tests all service roles) | A single `flexcore_app` role for ingest+query: rejected — the ledger requires the ingestion role and the query role to be distinct (P2.05 step 7); one role would make an ingestion-path RLS bypass indistinguishable from a query-path one |
| D-P7 | **TimescaleDB is not used in Phase 2.** It remains an optional deployment profile only (§16.1 row 3; P0.16 §1); the revisit trigger is measured telemetry volume requiring a dedicated time-series store (P2.26 performance note; §25.1) | The open baseline must not make proprietary/optional extensions mandatory (§16.1; P0.16 §1 corollary); Phase 2 volume is synthetic and small — the trigger is not met, and the Phase 2 anti-scope audit records the non-adoption (P2.30) | Adopting TimescaleDB for Phase 2: rejected — no measured need, adds a proprietary dependency to the baseline |

## 3. Table design

One table, `telemetry`, created by a Phase 2 migration (P2.06) that **extends**
the Phase 1 schema — the Phase 1 registry tables (`tenants`, `portfolios`,
`sites`, `assets`, `asset_state`), quality enum (`telemetry_quality`, P1.12
§3.2), and outbox are the base and are not recreated. Column table follows the
P1.12 §4 format; "FK" = foreign key.

| Column | Type | Constraints / notes |
|---|---|---|
| `telemetry_id` | `uuid` | `NOT NULL DEFAULT uuidv7()` (P1.12 D-S1); surrogate only — never used for dedup or ordering (P2.04 D-T3, §5) |
| `tenant_id` | `uuid` | `NOT NULL` (P0.16 rule 1); composite FK → `assets(asset_id, tenant_id)` — proves the denormalized tenant matches the asset's tenant (P1.12 §4.5 pattern) |
| `asset_id` | `uuid` | `NOT NULL`; same composite FK → `assets(asset_id, tenant_id)`; the measured asset must exist in the registry (P2.02) |
| `quantity` | `text` | `NOT NULL`, `CHECK (quantity IN ('active_power', 'reactive_power', 'soc', 'energy', 'availability', 'operating_mode', 'health', 'connection_state', 'baseline_reference'))` — the frozen Phase 2 set (P2.04 §4); extension is an explicit migration, never a silent widening |
| `event_time` | `timestamptz` | `NOT NULL` — **partition key**; UTC internal (P0.09 §2); when the measurement occurred (P0.09 §1) |
| `ingest_time` | `timestamptz` | `NOT NULL` — when FlexCore received it; distinct column, never merged with `event_time` (P0.09 §1); `ingest_time >= event_time` re-checked at the repository boundary (P1.05 `TimeRecord::validate` semantics) |
| `source_sequence` | `bigint` | `NOT NULL`, `CHECK (source_sequence >= 0)` — source-assigned ordering identifier, mandatory (P2.04 §8) |
| `source_id` | `text` | `NOT NULL`, `CHECK (length(btrim(source_id)) > 0)` — source identity; the `flexcore:` namespace is reserved for platform-derived records (P2.04 D-T6); measured sources must never use it |
| `quality` | `telemetry_quality` | `NOT NULL` — the Phase 1 enum (P1.12 §3.2), exactly the six P0.08 states; a seventh value is rejected by the database (D-S4) |
| `source_quality_code` | `text` | `NULL` — original richer source state as diagnostic metadata (P0.08 §3); never a seventh canonical state |
| `value_num` | `double precision` | `NULL`; numeric value in the record's canonical unit; `CHECK (value_num IS NULL OR (value_num <> 'NaN'::float8 AND value_num <> 'Infinity'::float8 AND value_num <> '-Infinity'::float8))` — the finite rule (P1.05 `UnitsError::NotFinite`) enforced at the database too |
| `value_text` | `text` | `NULL`, `CHECK (value_text IS NULL OR length(btrim(value_text)) > 0)` — source-declared state values carried verbatim (P2.04 D-T5 companion note) |
| `value_bool` | `boolean` | `NULL` — boolean state values |
| `unit` | `text` | `NOT NULL`, `CHECK (unit IN ('MW', 'MVAr', 'MVA', 'MWh', '1'))` — the canonical unit vocabulary (P0.07 §1; `1` = dimensionless: SOC fraction, boolean and state values). No kW/kWh and no scaled unit exists anywhere in the table (P0.07 §5) |
| `q_sign_convention` | enum `reactive_sign_convention` (`'INJECTION_POSITIVE'`, `'CONSUMPTION_POSITIVE'`) | `NULL`; `CHECK ((quantity = 'reactive_power') = (q_sign_convention IS NOT NULL))` — the declared convention travels on the record (P2.04 D-T4); no default exists |
| `clock_quality` | enum `clock_quality` (`'SYNCHRONIZED'`, `'UNSYNCHRONIZED'`, `'UNKNOWN'`) | `NOT NULL` — recorded synchronisation status, explicit `UNKNOWN`, never assumed (P2.04 D-T5; P0.09 §4). The enum type is created by the same Phase 2 migration, before the table |

Value-field totality CHECK (one table-level constraint):

- exactly one of `value_num` / `value_text` / `value_bool` is non-null,
  **except** that all three are `NULL` exactly when `quality = 'MISSING'`
  (a MISSING record is a derived gap marker with no value — P2.04 §3).
- the `(quantity, value kind, unit)` triple is valid per the P2.04 §4 mapping
  (e.g. `active_power` → numeric + `MW`; `soc` → numeric + `1`;
  `availability` → boolean + `1`; `operating_mode` → text + `1`) — implemented
  as a CHECK constraint enumerating the allowed triples; the P2.04 §4 table is
  the specification of its contents.
- SOC range: `CHECK (quantity <> 'soc' OR (value_num >= 0 AND value_num <= 1))`
  — the fraction-in-[0,1] rule (P0.07 §4), the D-S5 forward constraint applied
  to its Phase 2 consumer.

No `site_id`/`portfolio_id` columns (P2.04 D-T1: resolved by JOIN through
`assets` → `sites` → `portfolios`; `tenant_id` denormalized only for RLS and
scope, proven consistent by the composite FK).

No retention column, no TTL trigger, no scheduled-delete rule anywhere in the
schema (D-P5; P1.12 §9).

## 4. Partitioning

### 4.1 Method and interval (D-P2)

- `PARTITION BY RANGE (event_time)`; partitions are monthly, bounds
  `[first day of month, first day of next month)` — inclusive lower, exclusive
  upper (PostgreSQL range rule, RESEARCH §1). Partition name convention:
  `telemetry_yYYYYmMM` (e.g. `telemetry_y2026m12`).
- **Why monthly for Phase 2** (recorded reasoning per ledger step 2): the
  synthetic exit-gate portfolio is at least two sites with five assets each
  plus meters (P2.25), at the simulator's per-asset cadence — the resulting
  per-month row count is small in absolute terms. Monthly partitions keep the
  partition count low across the Phase 2 horizon while still giving exact
  time-window pruning for the P2.19 historical queries and a whole-partition
  unit for retention (D-P5). Daily or weekly partitions would multiply catalog
  entries and maintenance steps with no measurable benefit at this scale.
- **Production-revisit note:** production volume (real adapters, higher
  cadences, more sites) would revisit the interval — finer partitions and/or
  sub-partitioning are supported by PostgreSQL declarative partitioning
  (RESEARCH §1) — and would be decided from measured volume, not
  anticipatorily. This is a recorded revisit trigger, not a Phase 2 commitment.

### 4.2 The honesty note (recorded verbatim in substance, per ledger step 2 and `RESEARCH.md` §1)

> Partitioning benefits are normally worthwhile only when a table would
> otherwise be very large — the PostgreSQL rule of thumb is a table exceeding
> the physical memory of the database server. The Phase 2 synthetic portfolio
> does not meet that bar, and its size does not justify partitioning on
> measured volume. The partitioned shape is the **designed** form for the
> telemetry volume trajectory (§16.1: "PostgreSQL partitioned telemetry
> tables — start here; measure before adding a separate analytics database"),
> adopted now so the storage model, dedup identity, retention mechanics, and
> query paths do not have to be redesigned under load. It is **not claimed as
> a measured Phase 2 win**, and no Phase 2 performance observation (P2.26)
> may be presented as evidence that partitioning was needed.

Consequences of adopting the designed shape honestly: P2.06 must still prove
the mechanics (partition routing, pruning via `EXPLAIN`, out-of-range
handling) on real PostgreSQL 18 — those are correctness tests of the design,
not volume claims.

## 5. Primary key, uniqueness, and the P2.09 dedup identity

Recorded once here, exactly how uniqueness and the PK interact (ledger step 3;
P2.09 step 1 consumes this):

1. **PK** = `(asset_id, event_time, source_sequence, quantity)`. It includes
   the partition key `event_time` — a PostgreSQL requirement for any PK or
   unique constraint on a partitioned table (RESEARCH §1); PostgreSQL rejects
   the table definition otherwise. This is also the risk note's named failure
   mode (ledger P2.05 "Risks": PK without the partition key).
2. **Row grain:** the PK keys one row = one quantity of one sample
   (P2.04 D-T2). Within one sample, rows for different quantities share
   `(asset_id, event_time, source_sequence)` and differ in `quantity`.
3. **Dedup identity (P2.09):** a sample's identity is
   `(asset_id, event_time, source_sequence)`. On insert, the PK constraint
   rejects a second row with the same `(asset_id, event_time,
   source_sequence, quantity)`; the repository (P2.06 step 3) treats a PK
   collision on the sample identity as a candidate duplicate and applies the
   P2.09 rule: an exact duplicate (all quantity rows identical) is
   acknowledged without a second row; a conflicting duplicate (same identity,
   different values) is quarantined with a reason — never silently
   overwritten, never silently accepted.
4. **`telemetry_id` uniqueness:** the `uuidv7()` surrogate is not covered by
   any unique constraint and needs none — the composite identity is the sole
   uniqueness authority, and `uuidv7()` collisions are not relied upon for any
   correctness property. It exists as a stable row handle for diagnostics and
   event payloads; ordering by it is prohibited (P1.12 §2.3).
5. **`tenant_id` in the key:** not needed — `asset_id` is globally unique
   (UUIDv7 PK on the Phase 1 `assets` table) and the composite FK
   `(asset_id, tenant_id)` proves tenant consistency; reads are scoped by RLS
   (§7). Adding `tenant_id` to the PK would widen every unique index for no
   additional guarantee.
6. **Forward note (P2.31):** versioned corrections will need multiple
   versions of one `(asset_id, event_time, quantity)` distinguished by a
   version dimension — P2.31 extends this key by an explicit migration
   (extending, not recreating, per the P1.12 migration discipline). This
   document fixes only the Phase 2 initial key; the P2.31 change is recorded
   there and lands as its own migration.

## 6. Indexes

Created on the partitioned parent (PostgreSQL propagates parent indexes to
existing and future partitions). Ledger step 4 requires the first two; the
third is recorded here as a design addition for per-quantity queries.

| Index | Definition | Serves |
|---|---|---|
| `telemetry_asset_time_idx` | `(asset_id, event_time DESC)` | Current-state/history per asset (P2.18/P2.19): latest-N lookup, time-bounded scans, reverse ordering |
| `telemetry_tenant_asset_time_idx` | `(tenant_id, asset_id, event_time DESC)` | Tenant-scoped listing and portfolio/site-level queries after the registry JOIN; the ledger-required tenant query pattern |
| `telemetry_asset_quantity_time_idx` | `(asset_id, quantity, event_time DESC)` | Per-quantity current state and per-quantity history (P2.18/P2.19 queries filter quantity); avoids scanning sibling quantities of the same asset |

The latest-value decision (D-P4): no partial index for "latest per asset" is
created in Phase 2, and no materialized current-state table either — the
composite DESC indexes above serve the P2.18 query with partition pruning, and
`EXPLAIN` evidence is a P2.18 acceptance item. A materialized current-state
table becomes a recorded optimisation only on measured need (P2.18 step 2
default, applied once here).

## 7. Partition management

PostgreSQL does not auto-create partitions: inserting a row that maps to no
existing partition errors (RESEARCH §1). Both paths are specified (ledger
step 5):

1. **Pre-creation ahead of the ingest frontier (default):** a scheduled
   maintenance job (the P2.20 worker job) creates the next N monthly
   partitions ahead of the ingest frontier; N is configuration. Scheduled
   pre-creation is the default per P2.20 step 1 (create-on-demand during
   ingestion is the alternative P2.20 may record; this design fixes only the
   default and the required error path).
2. **Out-of-range error path:** an insert whose `event_time` falls outside all
   existing partitions is rejected by PostgreSQL with an error. The ingestion
   path (P2.10) must handle that error explicitly: the record (or batch
   member) is quarantined with a reason, counted by the quarantine metric, and
   retried where safe — never silently dropped and never written as `GOOD`
   (P2.07 pipeline rule; the P2.23 fault suite exercises a missing partition
   as a named fault: clear error, no silent drop).
3. **Clock-skew interaction:** an `event_time` far in the future or past
   (P2.23 clock fault) either lands in a pre-created partition (flagged
   SUSPECT per P2.08) or hits the out-of-range path (item 2) — either way it
   is never silently accepted as current GOOD state.

## 8. Retention (D-P5)

1. **Retention is configuration per data class** (§16.3; P0.16 §4): the
   retention value for each class is set by deployment/operations
   configuration, driven by customer contract, regional market rules, and
   privacy/security requirements. **No universal retention period appears in
   code**, in DDL, in a partition-drop schedule, or in this document.
2. **Phase 2 data-class note (recorded honestly):** Phase 2 stores normalized
   telemetry only (the simulator emits canonical records; there is no raw
   high-frequency class table). Of the seven §16.3 classes, only "normalized
   telemetry" has a Phase 2 carrier here; the raw class applies when real
   adapters arrive (later phases), and the other classes belong to their own
   phases' storage.
3. **Execution is detach/drop of whole partitions** (RESEARCH §1: far cheaper
   than bulk `DELETE`, no `VACUUM` overhead; `DETACH PARTITION ...
   CONCURRENTLY` needs only a `SHARE UPDATE EXCLUSIVE` lock, PG 14+).
   Whether detached partitions are dropped immediately or archived first is
   the P2.20 recorded decision (CONTEXT open question 9) — this design fixes
   the mechanism, not that choice.
4. **Safety rule carried into P2.20:** retention never touches the current or
   in-flight partition; only partitions fully outside the configured window
   are eligible (P2.20 step 3).
5. **No-retention-configured behaviour:** when no value is configured for a
   data class, no deletion occurs — the explicit recorded behaviour and its
   visibility (metric/alert) are P2.20 step 5's decision; this design requires
   only that the behaviour is written down and observable, never silent.
6. Any Phase 2 dev-only default in the Compose stack is configuration outside
   the schema, marked dev-only (P1.12 D-S3 pattern).

## 9. Row-Level Security and roles (D-P6)

1. `tenant_id` is on every row (`NOT NULL`, composite-FK-proven, §3).
2. One policy, exactly the P1.12 §5.1 text, applied to `telemetry`:

```sql
CREATE POLICY tenant_isolation ON telemetry
  USING      (tenant_id = NULLIF(current_setting('flexcore.tenant_id', true), '')::uuid)
  WITH CHECK (tenant_id = NULLIF(current_setting('flexcore.tenant_id', true), '')::uuid);
```

   with `ENABLE ROW LEVEL SECURITY` **and** `FORCE ROW LEVEL SECURITY`. An
   unset/empty tenant setting yields NULL → zero rows and no writes: fails
   closed, never open (P1.12 §5.1).
3. **Distinct roles, neither owns the tables** (ledger step 7):

| Role | Privileges on `telemetry` | Notes |
|---|---|---|
| `flexcore_migrator` | `ALL` (owner) — migrations only | Never used at runtime (P1.12 §3.1); `FORCE` keeps even owner sessions under the policy while the flag stands (P1.12 §5.3) |
| `flexcore_ingest` | `INSERT`, `SELECT` | The ingestion service's runtime role; tenant context set per transaction from the tenant-scoped source identity (P2.22; a source presenting another tenant's identity is rejected) |
| `flexcore_query` | `SELECT` | The query/API path's runtime role; no write path exists for it |

   Neither runtime role holds `DELETE`, `TRUNCATE`, or DDL (retention executes
   via the P2.20 approved-operation path under the migrator/maintenance role,
   with an audit record per action — P2.20 step 2; §16.2 rule 6 audits are
   application-written as in P1.12 §5.3).
4. RLS, grants, and the table are created in the same migration, so the table
   is never momentarily without its policy (P1.12 §7 ordering guarantee).
   All three roles are covered by the P1.13/P1.23 all-service-roles RLS tests,
   extended to `telemetry` by P2.06/P2.22.
5. The quarantine table (P2.07) follows the same discipline — its policy and
   role grants are specified with the P2.07 design and implemented by the
   same P2.06 migration set; it is not designed here.

## 10. TimescaleDB decision (D-P7)

- **Not used in Phase 2.** TimescaleDB is an optional time-series extension:
  evaluate as an optional deployment profile; do not make proprietary/optional
  features mandatory for the open baseline (§16.1 row 3; P0.16 §1 frozen
  corollary).
- **Revisit trigger:** measured telemetry volume requiring a dedicated
  time-series store — evidenced by the P2.26 performance note (observed
  numbers, never targets, §19.4) or a later phase's measured workload
  (§25.1). The revisit decision then checks the then-current TimescaleDB
  version and license from official documentation before proposing adoption
  (`RESEARCH.md` §4 open verification task — no version or license is cited
  here, because none was checked).
- Until the trigger fires, the PostgreSQL 18 partitioned table is the
  telemetry storage of record (§16.1 row 2), and the Phase 2 anti-scope audit
  records the non-adoption (P2.30).

## 11. Alignment with P1.12 discipline and the research record

| Discipline item (P1.12 / RESEARCH) | Applied here |
|---|---|
| D-S1 UUIDv7 surrogates via `uuidv7()`; UUID ordering never business ordering | `telemetry_id` (§3, §5 item 4) |
| D-S3 no retention values in schema or code | §8 (D-P5) |
| D-S4 PostgreSQL enum, frozen vocabulary, seventh value structurally rejected | `quality` reuses the Phase 1 enum; `reactive_sign_convention` / `clock_quality` enums created by the same migration set (§3) |
| D-S5 SOC constraint forward-applied to future SOC columns | `soc` range CHECK (§3) |
| §2.2 `timestamptz` UTC; `event_time`/`ingest_time` distinct | §3; P2.04 §7 |
| §5.1 RLS policy text, ENABLE + FORCE, fail-closed | §9 |
| §7 migration discipline (numbered, forward-only, up/down or recorded irreversible, RLS+grants in the same migration) | §3, §9 item 4; P2.06 implements |
| `RESEARCH.md` §1 (PG partitioning): PK must include partition key; no auto-creation; detach/drop retention; pruning | §4, §5, §7, §8 |
| `RESEARCH.md` §1 honesty rule (partitioning pays only on very large tables) | §4.2 honesty note |

## 12. Acceptance-criteria coverage (ledger P2.05)

| Ledger acceptance criterion | Where satisfied |
|---|---|
| Partitioning method, interval, and rationale are recorded | §4.1 (D-P2: RANGE(event_time), monthly, reasoning) |
| The honesty note is recorded: partitioning is the designed shape for the volume trajectory, not a measured Phase 2 win | §4.2 (verbatim-substance note) |
| Primary key includes the partition key | §5 items 1–2 (D-P3: `(asset_id, event_time, source_sequence, quantity)`) |
| Indexes cover the P2.18/P2.19 query patterns (current-state + history) | §6 (three composite indexes; D-P4 latest-value decision recorded) |
| Partition pre-creation and the out-of-range error path are specified | §7 (scheduled pre-creation default; quarantine-and-count error path) |
| Retention is configuration, not schema; no universal period hard-coded | §8 (D-P5; per-§16.3-class configuration; detach/drop mechanism; no value anywhere) |
| TimescaleDB non-use is recorded with the revisit trigger (measured volume) | §10 (D-P7; revisit trigger = measured volume via P2.26/§25.1) |

Verification method (ledger): design review against §16.1/§16.3 and the PostgreSQL partitioning docs — performed in §4 (against `RESEARCH.md` §1), §8 (against §16.3/P0.16 §4), §9 (against §16.2/P1.12 §5); the `EXPLAIN` review of the planned queries is P2.19's evidence, not this document's.
