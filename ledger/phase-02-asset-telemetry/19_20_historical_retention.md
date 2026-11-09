# P2.19 / P2.20 — Historical view and retention/partition maintenance (design + decisions)

| Field | Value |
|---|---|
| Subtasks | P2.19 — Historical view (time-range queries, aggregation, pagination); P2.20 — Retention and partition maintenance |
| Report trace | §15.2 (`/telemetry/query`), §16.1 (time-window queries, partitioned tables), §16.3 (retention), §20.1 (Audit & Reports — data only); P0.07, P0.08, P0.09, P0.16 |
| Inputs | P2.05 (`05_telemetry_storage_design.md`, D-P1..D-P7 — partitioning, keys, indexes, retention mechanism); P2.06 (telemetry repository, migrations); P2.12 (query route, limits, keyset cursor) |
| Status | Implementation record for the P2.19/P2.20 decisions the ledger asked to make once. Code lives in `crates/adapters/postgres/src/telemetry.rs` (historical query + aggregation), `crates/adapters/postgres/src/partition_maintenance.rs` (partition maintenance), `apps/control-plane/src/routes_phase2.rs` (the historical view on the P2.12 route), and `apps/worker/src/maintenance.rs` (the maintenance job) |
| Decisions | D-P1..D-P7 remain the storage decisions; the decisions below are D-H1..D-H6 (P2.19) and D-R1..D-R6 (P2.20), made once for Phase 2 |

---

## 1. P2.19 — Historical view decisions

### D-H1 — Pagination: keyset cursor over the P2.05 primary key (decided once)

The historical query paginates with an **opaque versioned keyset cursor** over
the P2.05 primary key `(asset_id, event_time, source_sequence, quantity)`
(storage design §5). The historical query is single-asset by contract, so the
cursor payload and the SQL strictly-after comparison carry the three varying
legs `(event_time, source_sequence, quantity)`; `asset_id` is fixed by the
query itself. This is the ledger's named option "the P2.05 PK", chosen over
"(event_time, source_sequence)" alone because a sample's quantity rows share
`(asset_id, event_time, source_sequence)` (P2.05 §5 item 2) — a keyset
without the quantity leg could skip sibling-quantity rows at a page boundary.
It is consistent with the Phase 1 / P2.12 keyset decision (cursor pagination
for time-series, offset pagination rejected everywhere: offsets are unstable
on an append-heavy time-ordered table and a full scan waiting to happen).

Cursor wire format: `hist-v1:<event_time millis, zero-padded to 15>:<source_sequence, zero-padded to 19>:<quantity id>`,
parsed strictly — anything malformed is 422, never a silent "first page"
(the P2.12 rule). Recorded assumption: `event_time` is a non-negative epoch
millisecond value, so zero-padding preserves numeric order lexicographically.

Sort order: `ORDER BY event_time ASC, source_sequence ASC, quantity ASC` —
the historical view orders and filters by `event_time` ONLY (P0.09;
`ingest_time` is metadata and appears in responses, never in the ordering).
The `source_sequence` tiebreak orders two samples that share an instant; the
`quantity` tiebreak makes the key total (the PK).

### D-H2 — Aggregations supported: raw + bucketed count/mean/min/max (decided once)

Exactly two query modes exist:

| Mode | Selection | Output |
|---|---|---|
| Raw | no `bucket` parameter | one page of records, keyset-paginated (D-H1) |
| Bucketed | `bucket` ∈ the allowed width set | one bucket per occupied interval, each carrying `count`, `mean`, `min`, `max` of `value_num` |

- **Bucket widths (allowed set, recorded):** `1m`, `5m`, `15m`, `1h`, `1d`.
  Any other value is 422. The set is closed; extension is an explicit change
  to this document and the parser, never an arbitrary client-provided
  interval.
- **Bucketing is done in SQL** with PostgreSQL `date_bin(bucket_width,
  event_time, origin)`; the origin is the query's inclusive `from` bound, so
  the first bucket starts exactly at `from` and bucket boundaries are
  deterministic for a fixture. No TimescaleDB time_bucket (D-P7:
  TimescaleDB is not used in Phase 2).
- **Exactly one quantity per aggregation request**, and it must be a
  numeric-carrying quantity (`active_power`, `reactive_power`, `soc`,
  `energy`, `baseline_reference`). Aggregating across quantities would mix
  units (MW with MVAr with dimensionless fractions), which P0.07 forbids;
  boolean/text-state quantities have no numeric aggregate. Violations are 422.
- **Aggregates cover numeric rows only** (`value_num IS NOT NULL` in the
  WHERE clause). A `MISSING` gap marker has no value and contributes to no
  bucket (it is counted by nothing rather than counted as zero — a zero would
  fabricate a measurement).
- **No rounding anywhere (P0.07 stance):** `avg`/`min`/`max` are returned as
  PostgreSQL `double precision` verbatim and serialized as-is; neither the
  SQL nor the API layer rounds, re-scales, or re-units any value. Canonical
  units are preserved (MW/MVAr/MWh per the quantity, `1` dimensionless);
  `P > 0` = export at the point of connection is the sign convention of the
  stored values and aggregation never re-interprets it. A bucket's mean of
  MW values is a mean power in MW — never averaged with energy or converted.
  Empty buckets are absent from the response (no synthetic zero-fill).

### D-H3 — Limits are configuration, enforced with the stable error envelope (decided once)

| Limit | Value | Source |
|---|---|---|
| Maximum query time range | 7 days | REUSED from P2.12 (`MAX_QUERY_RANGE_MILLIS`, `routes_phase2.rs`) |
| Maximum page size (raw mode) | 1000 (default 200) | REUSED from P2.12 (`MAX_PAGE_SIZE` / `DEFAULT_PAGE_SIZE`) |
| Maximum buckets per aggregation request | 1000 | NEW, recorded here and as `MAX_HISTORY_BUCKETS` in `routes_phase2.rs` |

Bucket count for a request is computed as
`floor((until - from) / width) + 1` from the validated range and width;
exceeding the limit is 422. Every oversized or invalid request returns 422
with the stable error envelope (`{"error": {"code": "validation_rejected",
...}}`, `error.rs`) — never a truncated or silently-clipped result. The
values live with the route contract they bound (the recorded P2.12 pattern);
they are configuration in the sense the ledger requires — recorded once,
changeable in one place, never hard-coded inline at call sites.

### D-H4 — Partition pruning: representative queries and the EXPLAIN obligation

The SQL is written so pruning works: **the `event_time` bounds are always in
the WHERE clause against the partition key**, as plain `>=`/`<=` predicates
on `telemetry.event_time` (P2.05 §6 indexes: `(asset_id, event_time DESC)`,
`(tenant_id, asset_id, event_time DESC)`, `(asset_id, quantity, event_time
DESC)`). Representative queries as implemented:

Raw page (partition-pruning form; `$n` are bound parameters):

```sql
SELECT telemetry_id::text AS telemetry_id, tenant_id::text AS tenant_id, ...
FROM telemetry
WHERE tenant_id = $1::uuid
  AND asset_id = $2::uuid
  AND event_time >= $3 AND event_time <= $4        -- partition-key predicates
  AND quantity = ANY($5::text[])                   -- optional
  AND quality::text = $6                           -- optional
  AND (event_time, source_sequence, quantity) > ($7::timestamptz, $8::bigint, $9::text)
ORDER BY event_time ASC, source_sequence ASC, quantity ASC
LIMIT $n
```

Bucketed aggregation (same pruning shape):

```sql
SELECT date_bin($1::interval, event_time, $2::timestamptz) AS bucket_start,
       count(*)::bigint AS sample_count,
       avg(value_num) AS mean_value, min(value_num) AS min_value, max(value_num) AS max_value
FROM telemetry
WHERE tenant_id = $3::uuid AND asset_id = $4::uuid
  AND quantity = $5
  AND event_time >= $6 AND event_time <= $7        -- partition-key predicates
  AND value_num IS NOT NULL
  [AND quality::text = $8]
GROUP BY 1 ORDER BY 1
```

What makes pruning work (recorded): the two `event_time` range predicates
against the RANGE partition key (P2.05 D-P2, monthly partitions). The tenant
and asset predicates select the index (`telemetry_asset_time_idx` /
`telemetry_tenant_asset_time_idx`) but do not themselves prune partitions;
the time bounds prune. Parameterized bounds can prevent plan-time pruning —
the recorded P2.06 `EXPLAIN` test uses literal timestamps for that evidence,
and the same applies here.

**EXPLAIN obligation:** the human runs `EXPLAIN` (or `EXPLAIN ANALYZE`) on
the two representative queries above against a database with populated
partitions — the loop bans forbid this task from executing it. Acceptance
(echoing P2.19 criterion 4): the plan touches only the partitions
overlapping `[from, until]` and prunes the rest; per-partition scans use the
P2.05 composite indexes rather than sequential scans. The evidence output
belongs in the P2.19 acceptance record.

### D-H5 — Route execution path and the API read grant (recorded)

The historical view is served by `GET /api/v1/telemetry/history` — a new
endpoint added by the P2.19 deliverable (the ledger's "historical view"),
kept separate from the untouched P2.12 `/api/v1/telemetry/query` route so
the P2.12 contract and its recorded cursor semantics are unchanged. On the
**Postgres** backend the route runs both modes inside ONE unit-of-work
transaction (`FlexCoreStore::begin(Some(&ctx))` → asset-existence check via
the repository port (cross-tenant/unknown asset → 404, no existence leak) →
the adapter history/aggregation call → commit), so RLS
(`flexcore.tenant_id`) scopes every row read. On the **Test** backend (the
in-memory port fakes, which have no SQL engine and therefore no `date_bin`
and no keyset SQL) the historical route fails closed with the opaque
infrastructure error — its coverage is the DB-gated test suite
(`apps/control-plane/tests/telemetry_history.rs`, gated on
`FLEXCORE_TEST_DATABASE_URL`), plus pure unit tests for the limit validators
and cursor encode/decode that need no database.

**Read grant (recorded role-model extension):** the API runtime connection
(`flexcore_app`, `FLEXCORE_DATABASE_URL`) had no telemetry privilege under
the P2.05 D-P6 grant set (0010 granted SELECT only to the dedicated
`flexcore_query` role), so the API path could not read telemetry at all.
Migration `0011_telemetry_api_read_grant_p2_19` grants `SELECT ON telemetry
TO flexcore_app` — exactly one read grant: the write path is unchanged
(`flexcore_ingest` remains the only INSERT path), neither runtime role owns
any table or holds DELETE/TRUNCATE/DDL, and the tenant_isolation RLS policy
with ENABLE + FORCE scopes every row read through the grant. The dedicated
`flexcore_query` role remains for direct query-path deployments.

## 2. P2.20 — Retention and partition-maintenance decisions

### D-R1 — Partition pre-creation: scheduled (decided once)

**Scheduled pre-creation is the mechanism**: a config-driven `tokio`
interval loop in the worker (`apps/worker/src/maintenance.rs`) creates the
next N monthly partitions ahead of the ingest frontier (N is configuration,
`FLEXCORE_MAINTENANCE_MONTHS_AHEAD`, default 2; D-P2 monthly partitions,
names `telemetry_yYYYYmMM`, bounds `[first day of month, first day of next
month)`). The alternative — create-on-demand during ingestion — is rejected
because it would need the ingestion role to hold DDL, which D-P6 forbids.

**The ingestion path's missing-partition behaviour is already implemented
and unchanged** (verified, referenced): an insert whose `event_time` falls
outside every existing partition is classified by the repository as
`TelemetryError::OutOfRangeEventTime` / `TelemetryBatchOutcome::
OutOfRangeEventTime` (`telemetry.rs`, SQLSTATE 23514 with no constraint
name), and the pipeline quarantines and counts it — never a silent drop,
never stored as GOOD (P2.05 §7 item 2).

The maintenance job needs DDL (CREATE/DETACH/DROP of partitions), which the
runtime role must not hold (D-P6). It therefore connects through its own
maintenance database URL (`FLEXCORE_MAINTENANCE_DATABASE_URL`, the
migrator/maintenance role); **when that URL is not configured the
maintenance loop is not started at all** — no maintenance without the
privileged path, fail-safe.

### D-R2 — Retention execution: detach first, drop only in a second approved step (decided once)

Retention execution in Phase 2 is a **manual/approved operation, not an
automatic delete**:

- The worker loop performs **detach only**, and only when BOTH hold:
  (a) a retention value for the data class is explicitly configured
  (`FLEXCORE_RETENTION_NORMALIZED_TELEMETRY_MONTHS` — the "normalized
  telemetry" data class, the only Phase 2 carrier per P2.05 §8 item 2), and
  (b) the run is flagged approved (`FLEXCORE_RETENTION_APPROVED=true`).
  Configured-but-not-approved is visible (a warning line at every pass) and
  deletes nothing.
- **Drop is a second, separately invoked step.** The worker loop never drops.
  The drop function (`drop_detached_telemetry_partition`) is an explicitly
  invoked operation for the approved ops path; it refuses any partition that
  is still attached to `telemetry` (detach first is structurally enforced).
  The regular `ALTER TABLE telemetry DETACH PARTITION <name>` is used rather
  than `DETACH CONCURRENTLY` because CONCURRENTLY cannot run inside a
  transaction and the audit record must commit atomically with the detach
  (D-R5); the brief ACCESS EXCLUSIVE lock is acceptable at Phase 2 volumes.
- Audit record per action (D-R5): actor, action
  (`telemetry.partition.detach` / `telemetry.partition.drop`), object
  (`telemetry_partition`, partition name), event/ingest timestamps, the
  config source, and the approved-run correlation id.

### D-R3 — Safety rule: never the current or in-flight partition (decided once)

Eligibility is computed **from the partition bounds** (read from the catalog
via `pg_inherits` + `pg_get_expr(relpartbound, ...)`):

- The retention cutoff is the first day of the month
  `retention_months` before the current month. A partition is eligible
  exactly when its upper bound (`until`) is `<=` the cutoff — i.e. the
  partition lies entirely outside the retention window.
- **Explicit refusal in code**: a detach/drop request for a partition whose
  bounds contain the current instant (the current/in-flight partition) or
  whose upper bound is in the future is refused with a typed
  `MaintenanceError::Refused` — never attempted, never silently skipped.
- A partition whose bounds cannot be parsed from the catalog is **never
  eligible** (fail-safe: unknown bounds → no retention action) and is
  reported.

### D-R4 — No-retention-configured behaviour (decided once)

When no retention value is configured for a data class, **no deletion
occurs and partitions accumulate unboundedly** — and that state is VISIBLE,
never silent: every maintenance pass emits a status line (and the pass
report carries the status value) stating that no retention is configured for
any data class. Retention values are configuration, never hard-coded
constants (P0.16 §4; D-P5: no universal retention period in code, in DDL, or
in this document). The metrics-family surface (a scrapeable metric) is
P2.21's deliverable; the pass report value recorded here is its input.

### D-R5 — Audit records: same transaction as the DDL (decided once)

Every detach and every drop writes one platform-level audit record
(`tenant_id` NULL — retention is not a tenant action) **in the same
transaction as the DDL**, via the `AuditSink` implementation on
`PgUnitOfWork` (`unit_of_work.rs` → `audit::insert_audit_record`): the
partition name, the action, the event/ingest timestamps, the actor
(`flexcore-worker:maintenance` or the invoking operator), the config source,
and the correlation id. A platform-level row under the RLS policy is
writable only through the owner/ops path (P1.12 §5.2, `0008` migration) —
the maintenance connection IS that path (D-R1); this is recorded, not
worked around.

### D-R6 — Partition-bounds source of truth (recorded)

Partition bounds come from the PostgreSQL catalog
(`pg_inherits` joined to `pg_class`, bounds from
`pg_get_expr(c.relpartbound, c.oid)`), parsed strictly against the canonical
`FOR VALUES FROM ('<ts>') TO ('<ts>')` rendering. VERIFY-BUILD: the exact
`relpartbound` text rendering is a recorded assumption verified by the
P2.20 partition tests on PostgreSQL 18; a rendering change yields parse
failure → the partition is skipped from retention (D-R3 fail-safe), never
mis-bounded. No migration is added for P2.20: no schema change is required
(partition creation is runtime DDL by design — migration 0010 records why a
migration must not create time-dependent partitions), and D-P5 forbids
retention values in DDL. The only P2.19/P2.20 migration is 0011, the API
read grant recorded in D-H5 (a GRANT, not a schema change).

## 3. Acceptance-criteria mapping (ledger)

| Ledger criterion | Where satisfied |
|---|---|
| P2.19: range/quality/aggregation/pagination tests | `apps/control-plane/tests/telemetry_history.rs` (DB-gated) + pure unit tests for validators/cursor |
| P2.19: aggregation matches a hand-computed fixture exactly | `aggregation_matches_a_hand_computed_fixture` (expected numbers computed literally in the test) |
| P2.19: pagination no duplicates/gaps | `pagination_walk_has_no_duplicates_or_gaps` (walks all pages) |
| P2.19: EXPLAIN shows pruning | D-H4 §2 — the representative queries + pruning predicates are recorded; the EXPLAIN run is the human's (loop bans) |
| P2.19: oversized range → 422 | `oversized_range_is_rejected_with_422` + `oversized_bucket_count_is_rejected_with_422` |
| P2.20: pre-creation test | `crates/adapters/postgres/tests/partition_maintenance.rs` (`pre_creation_creates_partitions_with_expected_names_and_bounds`) and `apps/worker/tests/maintenance.rs` |
| P2.20: only out-of-window partitions detached | `retention_detaches_only_out_of_window_partitions` (both test files) |
| P2.20: current-partition drop refused | `current_or_future_partition_detach_is_refused` / `detaching_the_current_partition_is_refused` |
| P2.20: no-retention-configured recorded + tested | D-R4; `no_retention_configured_deletes_nothing_and_reports_the_unconfigured_state` |
| P2.20: audit record per retention action | `every_retention_action_has_an_audit_record` |
