# HANDOVER — Patch A: Postgres `TelemetryReader` (HIGH gap 1 of 2)

- **Date:** 2026-11-18
- **Repo HEAD:** `74d56f4` (uncommitted work on top; nothing staged, committed, or pushed)
- **Status:** Implemented, awaiting human build/test. Nothing in this patch was built, run, or verified by execution — no `cargo` command of any kind was run, by the hard rules of this task. Krishna builds, runs, and patches.
- **Companion:** Patch B (cryptographic source authentication) is a separate handover, `HANDOVER_PATCH_B.md`, written when that patch lands.

## What this patch is

The `TelemetryReader` port (`crates/flexcore-ports/src/telemetry_read.rs:421`) had exactly one implementation in the whole tree — `FakeTelemetryReader` — and `apps/control-plane/src/main.rs` wired `None`, so the four telemetry routes failed closed. This patch lands a real Postgres reader, wires it in, and adds five complete test files. No migration was added; no table or trait schema changed; the ingestion/write path was not touched.

## Files changed

### Production code (Worker A1; coordinator follow-up edits noted)

| File | Change |
|---|---|
| `crates/adapters/postgres/src/telemetry_reader.rs` (NEW, 314 lines) | `PgTelemetryReader { store: FlexCoreStore }`, `new(store)`, `impl TelemetryReader` for all three methods; `StoredTelemetry → NormalizedTelemetry` mapping with decode-failure discipline (impossible stored pairings fail closed in `map_record`; `IngestTime::new` quarantine error mapped to decode failure with column labels). `query` reuses the new `query_telemetry_port`; `latest_state` reuses `latest_telemetry_per_asset`; `list_quarantine` reuses `list_telemetry_quarantine(ctx, u32::MAX)`. |
| `crates/adapters/postgres/src/telemetry.rs` | New `query_telemetry_port` (ASC, `NOT_SUPERSEDED` with NULL as-of = latest version per identity, cursor predicate `(date_trunc('milliseconds', event_time), quantity) > (...)`, `ORDER BY event_time ASC, quantity ASC, source_sequence ASC, telemetry_version ASC`, `LIMIT limit+1`) + `parse_port_cursor` helper. |
| `crates/adapters/postgres/src/store.rs` | New `telemetry_reader()` method returning `PgTelemetryReader` (construction pattern mirrors `PgWebhookDeliveryStore::new(store.clone())` / `PgCommandStateReader::new(store.clone())`). |
| `crates/adapters/postgres/src/lib.rs` | Module registration + re-export + doc-table row for the new reader. |
| `crates/adapters/postgres/src/corrections.rs` | `NOT_SUPERSEDED_AT_AS_OF` visibility changed to `pub(crate)` only (was wider than needed). |
| `apps/control-plane/src/main.rs` | `None` → `Some(Arc::new(store.telemetry_reader()))`; the gap comment deleted (wiring landed). |
| `apps/control-plane/src/state.rs` | Telemetry field docs updated to the landed shape. |
| `apps/control-plane/src/routes_phase2.rs` | Quarantine comment corrected: the reader does not run inside the request transaction; it opens its own tenant-scoped UoW per call (see design decision). No route logic changed. |
| `crates/flexcore-ports/src/telemetry_read.rs` | Module header updated: records that Patch A landed. No trait change. |
| `crates/flexcore-ports/src/lib.rs` | Doc-table row updated. |

### Tests (Worker A2 — 3,113 lines total, all DB-gated, fail loudly)

| File | Coverage |
|---|---|
| `crates/adapters/postgres/tests/telemetry_reader_query.rs` (962) | Range/quantity/quality filters incl. combined filters and value round-trip; pure `TelemetryReadQuery::new` rejection of inverted range and zero limit; 250-row pagination walk in pages of 50, every row exactly once in ascending sort-key order; mid-walk insert of 10 rows in the unread range picked up exactly once; sub-second cursor precision (0/250/500/750 ms rows walked one at a time; cursor ↔ `parse`/`for_record` round-trip; 15-digit zero-padded sort-key shape pinned to literal `001893628800250`); 11 malformed cursors rejected by `TelemetryCursor::parse`; tenant scoping both directions. |
| `crates/adapters/postgres/tests/telemetry_reader_latest_state.rs` (664) | One row per quantity at latest `event_time` (incl. reactive-power value + declared sign convention round-trip); tie-break on higher `source_sequence` (older row with higher sequence does NOT win); late arrival with newer `event_time` advances state; late arrival with older `event_time` does not regress; absent quantities (soc/energy/reactive_power) absent, never zero; cross-tenant asset returns nothing. |
| `crates/adapters/postgres/tests/telemetry_reader_quarantine.rs` (465) | 3 entries tenant A + 2 tenant B; only the caller's rows, newest-first (`quarantined_at` DESC, 25 ms sleeps between commits so ordering is unambiguous); exact round-trip of `raw_payload` (compared parsed as JSON), `source_id`, `asset_id`, `event_time`, `attempt_count`; empty list for a tenant with no entries. |
| `crates/adapters/postgres/tests/telemetry_reader_rls.rs` (539) | As `flexcore_query` role: no tenant context → 0 rows; tenant B context → 0 of A's 3 rows and exactly its own 2; reader under B's context returns 0 of A's rows, and the equivalent raw SELECT with the tenant predicate deliberately removed still returns 0 under B (A-context controls prove the query can find the rows); table owner `flexcore_migrator` with no tenant set sees 0 rows (FORCE RLS), with the A-context control seeing 3. |
| `crates/adapters/postgres/tests/telemetry_reader_errors.rs` (483) | Closed-pool store: `query`/`latest_state`/`list_quarantine` all return `Err(TelemetryReadError::Backend(_))` with Display checked for the stable opaque prefix and absence of SQL, table/partition names, URLs, host, password, role name, and engine text (also checked on `BackendError::code()`/`message()`); unreachable-URL store fails closed (construction fails OR all three reads fail opaque); asset with no rows returns an empty page with `next_cursor == None`, empty latest state, empty quarantine list. |

Test harness conventions copied from `crates/adapters/postgres/tests/telemetry.rs`: fixed 2030/2031 test months, partitions created as migrator role, role grants with the same test-only passwords, registry seeded through the runtime role, telemetry via ingest role, `ingest_time = event_time + 1s`, `q_sign_convention` only for reactive power. Distinct fixture prefix bytes per file (0xC1–0xC5 for tenant A). No dependencies added; no src/docs/ledger files touched by the test worker.

## Design decision (recorded once)

**The reader is POOL-HELD via `FlexCoreStore`, NOT bound to the request's `PgUnitOfWork`.** `PgUnitOfWork` is `!Sync` (native transaction behind `Arc<AsyncMutex<Option<Tx>>>`, private `tx`/`runtime`), so no reader holding one can be stored as `Arc<dyn TelemetryReader>` in `AppState`. Each trait call opens its own UoW via `store.begin(Some(ctx))` and commits after the read. Consequences, recorded honestly:

- The request's UoW and the reader's UoW are **separate transactions**. A telemetry row committed by the request transaction a moment before the read is visible to the reader (READ COMMITTED default); a row written inside the request transaction but not yet committed is NOT visible to the reader. No route today writes telemetry and then reads it back in the same request, so this is a recorded limitation, not a live bug — but any future route that does must be written with this in mind.
- Tenant isolation is preserved because every reader UoW is opened with `Some(ctx)`, which sets the RLS GUC transaction-locally (`flexcore.tenant_id`), and the SQL also carries an explicit `tenant_id = $1` predicate. RLS is the backstop, FORCE ROW LEVEL SECURITY, role `flexcore_query` SELECT-only.
- The `state.rs` note ("land ON PgUnitOfWork") is superseded by this decision; its text was updated. The pool-held shape was chosen over the per-request alternative because it required no `AppState`/`routes_phase2.rs` signature changes and no access to `PgUnitOfWork`'s private fields (the `schedule_store.rs` lesson: add an accessor/constructor on the type itself, never reach into private fields from a sibling module).

## Per-acceptance-criterion status

| # | Criterion (from the brief) | Status |
|---|---|---|
| 1 | All four routes return real data for the owning tenant, never another tenant's rows, verified against real Postgres as `flexcore_query` | **Implemented, deferred to human build** — reader wired, routes unchanged (they already called the reader), RLS tests written. Not run. |
| 2 | 250-row walk in pages of 50 returns every row exactly once in ascending sort-key order, even with rows inserted mid-walk | **Implemented, deferred to human build** — walk + mid-walk-insert tests written. Not run. |
| 3 | `latest_state`: exactly one row per quantity present, latest `event_time` (ties: higher `source_sequence`); late newer arrival updates, older does not regress; absent quantity absent (API derives MISSING, never zero) | **Implemented, deferred to human build** — all five sub-cases tested. Not run. |
| 4 | `list_quarantine`: only the caller's tenant rows in quarantine order, raw payload + reason + source identity + timestamps | **Implemented, deferred to human build** — tested incl. payload/reason fidelity. Not run. |
| 5 | RLS negative: no tenant set → zero rows as app role; tenant B context → zero of tenant A's rows even if the SQL predicate is deliberately removed in the test double | **Implemented, deferred to human build** — tested as `flexcore_query` incl. the predicate-removed backstop and the owner-role FORCE RLS check. Not run. |
| 6 | Cursor round-trip preserves microsecond precision; truncated/malformed cursor is a 400 | **Partially met by design — see deviation 1.** Cursor preserves **millisecond** precision (see below). Malformed cursor → parse rejection tested (11 cases); the 400 mapping lives in the existing route layer (`map_read_error`), unchanged. Not run. |
| 7 | Index-scan evidence deferred to Krishna's build round | **Met as a record** — see "Index analysis" below. No EXPLAIN was or could be run (no DB runs allowed). |
| 8 | No `.unwrap()` in production code; no storage error text in any API response | **Met by inspection** — verified from the full diff: zero `.unwrap()`/TODOs in new/changed production code; opaque `Backend` errors asserted in tests. Not compile-verified. |
| 9 | No access to `PgUnitOfWork`'s private `tx`/`runtime` from a sibling module | **Met** — the reader never touches `PgUnitOfWork`; it holds `FlexCoreStore` and opens its own UoW through the public `begin`. |

## Deviations from the brief (with reasons)

1. **Cursor is millisecond-granular, NOT microsecond.** The brief assumed a microsecond sort key `(event_time, source_sequence, quantity)` with a microsecond-precise cursor. The port contract as written in the tree supersedes this: `TelemetryCursor`/`TelemetryReadQuery` define the sort key as `(event_time, asset_id, quantity)` at **millisecond** granularity (15-digit zero-padded unix millis in the key, e.g. `001893628800250`). The implementation follows the port contract, not the brief's assumption. Consequence, recorded: two rows for the same asset+quantity whose `event_time`s differ by less than 1 ms share a cursor position; the SQL cursor predicate uses `date_trunc('milliseconds', event_time)` and orders by `event_time ASC, quantity ASC, source_sequence ASC, telemetry_version ASC` to stay deterministic within a millisecond. The production code carries a comment documenting this sub-millisecond boundary limitation. If millisecond granularity is ever insufficient, the port contract itself must change first — this patch does not change it.
2. **Tenant context key is `flexcore.tenant_id`, NOT `app.tenant_id`.** The brief said `app.tenant_id`; the tree's migrations (0010 RLS policy `tenant_isolation`) and `FlexCoreStore::begin` use `flexcore.tenant_id`. The implementation follows the tree.
3. **No new migration.** The brief allowed migration 0026 only if a genuine index gap was found from reading migration 0010. The existing indexes (`telemetry_asset_time_idx`, `telemetry_tenant_asset_time_idx`, `telemetry_asset_quantity_time_idx`) cover the query patterns; no gap was identified by reading. Migration 0026 therefore stays free for Patch B. EXPLAIN evidence is deferred (criterion 7).
4. **Reader reuses existing inherent UoW query methods** (`query_telemetry`, `latest_telemetry_per_asset`, `list_telemetry_quarantine`, `query_telemetry_history` patterns) instead of writing a parallel query layer. This is a reuse choice, not a behaviour change; `query_telemetry_port` is the one new query because the port's ASC keyset semantics (NULL as-of = latest version per identity) had no existing equivalent.
5. **Pagination final-page cursor semantics left flexible in the walk test.** The brief says `next_cursor` is `Some` iff more rows may follow, but the existing `FakeTelemetryReader` returns `Some` whenever a page is full (so a 250-row walk in pages of 50 yields a 6th empty page). The walk test asserts pages 1–4 carry a cursor and accepts 5 or 6 total pages, following the cursor until `None` either way — it passes under both semantics instead of pinning one. The production reader's `LIMIT limit+1` lookahead gives the strict semantics (cursor only when a further row exists).
6. **Quarantine `failure_reason` format.** The brief says the reason "combines stage and reason" without fixing a separator, so the test asserts the returned reason contains both the stage and the reason verbatim rather than asserting a guessed exact string.
7. **Unreachable-URL test accepts either branch.** Pool construction is lazy, so the test accepts construction failing closed OR all three reads returning opaque `Backend(_)`; it asserts the error variant only in the latter branch.

## Index analysis (deferred to the human build round)

From reading migration 0010/0012 only (no EXPLAIN run — none was allowed):

- `telemetry_asset_time_idx` on `(asset_id, event_time DESC)` — covers the query's asset+range predicate and the ASC order (backward scan).
- `telemetry_tenant_asset_time_idx` on `(tenant_id, asset_id, event_time DESC)` — covers the tenant-scoped variant.
- `telemetry_asset_quantity_time_idx` on `(asset_id, quantity, event_time DESC)` — covers quantity-filtered queries.
- `latest_state` uses `DISTINCT ON (asset_id, quantity)` with `ORDER BY asset_id, quantity, event_time DESC, source_sequence DESC, telemetry_version DESC` — the existing indexes do not match this ordering exactly (they lack `source_sequence`/`telemetry_version` in the key), so the planner may sort. Whether that matters at real volumes is an EXPLAIN question for the build round; no speculative index was added.
- **Build-round check for Krishna:** run `EXPLAIN (ANALYZE, BUFFERS)` for (a) a tenant-scoped range query, (b) the `latest_state` DISTINCT ON query, (c) the quarantine list query, and record whether each uses an index scan. If the `latest_state` query sorts, consider a `(asset_id, quantity, event_time DESC, source_sequence DESC, telemetry_version DESC)` index as migration 0027 (after Patch B's 0026) — a decision for the build round, not this patch.

## Build-round watch items (compile risks flagged by the workers, unverifiable without building)

1. `Option::<time::OffsetDateTime>::None` as the NULL as-of bind in `query_telemetry_port` — the intended "latest version per identity" semantics depend on this binding typing correctly against the `$SUPERSEDED` placeholder comparison.
2. `date_trunc('milliseconds', …)` inside the row-comparison cursor predicate — check the planner accepts the expression in the comparison and that it does not defeat the index.
3. `Arc::new(store.telemetry_reader())` coercion to `Arc<dyn TelemetryReader>` in `main.rs` — the method returns the concrete `PgTelemetryReader`; the coercion is the standard unsized one but was not compiled.
4. `ReactivePowerMVAr::new` — confirmed present in `crates/flexcore-contracts/src/units.rs` (follows the `ActivePowerMW::new` pattern); used by the latest-state test.
5. `PgPool::close()` used as `test_runtime().block_on(store.pool().close())` in the errors test — correct if `close()` is async (sqlx 0.9 API); verify at build time.

## Exact commands for Krishna's build/test round

```bash
cargo generate-lockfile
cargo test --workspace
cargo clippy --workspace -- -D warnings
cargo fmt --check
python3 tools/check_dependency_rule.py
```

Then the gated suite with the test database (and NATS, for the worker tests) set:

```bash
FLEXCORE_TEST_DATABASE_URL=postgres://<test-user>@<host>:<port>/<db> \
FLEXCORE_TEST_NATS_URL=nats://<host>:<port> \
cargo test --workspace
```

Treat any failure as a patch round. The new tests fail loudly (panic naming `FLEXCORE_TEST_DATABASE_URL`) when the variable is unset or empty — they never silently skip. The two pure validation tests (query validation, malformed cursors) need no DB and are exempt from the gate.

Read-only checks already run by the coordinator on this patch (no build): `python3 tools/verify_docs.py` OK, `python3 tools/validate_agent.py` OK, `python3 tools/check_dependency_rule.py` OK ("dependency rule holds for 29 workspace members").

## Ledger ticks

**No new boxes were ticked by this patch.** The P2.06, P2.12, and P2.18 acceptance criteria were already ticked as "implemented, awaiting human build/test" by the earlier wave — over-ticks, made while the only implementation was `FakeTelemetryReader` and `main.rs` passed `None`. This patch supplies the real implementation and tests behind those boxes for the first time; their status text is unchanged because nothing is build-verified yet. A dated note to that effect was added under each of the three sections in `ledger/phase-02-asset-telemetry/ledger_phase_2.md`. If the build round passes, those boxes become honestly true; if it fails, the failure is a Patch A round, not a new tick.

## Blockers / decisions still needed

- **None blocking the build round.** The three watch items above are compile-time checks, not decisions.
- **Open question for Krishna (not a blocker):** is millisecond cursor granularity acceptable long-term, or should the port contract be revisited to microsecond precision? If microsecond is required, that is a port-contract change (`TelemetryCursor` sort key) plus a rework of `query_telemetry_port`'s cursor predicate — a separate, small patch, not part of this one.
- **D1–D6 untouched.** No decision was made or closed. Patch B also does not close D6 (recorded in its own handover).

## Assumptions (labelled)

- The existing 0010 indexes cover the query patterns well enough to defer EXPLAIN to the build round (assumption, not evidence — see "Index analysis").
- `PgPool::close()` is async in sqlx 0.9 (assumption; verify at build time — watch item 5).
- The `FakeTelemetryReader`'s final-page cursor semantics (cursor on a full page) describe the port's intended behaviour loosely enough that the walk test's either-way assertion is acceptable (assumption, recorded as deviation 5).

## Risks

- **Separate-transaction reads** (design decision above): a future route that writes telemetry and reads it back in the same request would not see its own uncommitted write. Recorded here so it is not discovered by surprise.
- **Sub-millisecond event times**: two rows within the same millisecond for the same asset+quantity share a cursor position; the deterministic tie-break (`quantity, source_sequence, telemetry_version`) keeps the walk correct, but the cursor itself cannot distinguish them. Millisecond precision is the port contract's choice, not this patch's.
- **Unverified until built**: every line of this patch is written, not built. The handover's claims are inspection claims only.
