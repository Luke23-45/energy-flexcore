# P4.36 — Audit-events read API (§15.2, §15.3, §16.2, §10.1, §10.5, §21.3)

Status: **implemented, awaiting human build/test**. Nothing was built, run,
installed, or executed in this subtask (loop bans hold). No migration was
written (none was needed — see §1); the DB-gated suite is env-gated on
`FLEXCORE_TEST_DATABASE_URL` per the recorded harness pattern, skips cleanly
when unset, and was NEVER EXECUTED. Every acceptance criterion below is
therefore implemented-and-unverified, not passed.

## 1. Verify-first result (ledger step 1)

Checked FIRST, per the verify-first instruction:

- **P1.10 audit crate/port** (`crates/flexcore-audit/src`): the write side
  lives there — `AuditSink` (append-only, exactly ONE method) + `AuditRecord`
  (the full §10.1 field set). There is **no read path** in the crate — by
  recorded design: the P1.10 sink port is "a one-way write valve … no read
  path, no query path", and a source-text test in `sink.rs` pins the port at
  exactly one method. The read side therefore could NOT be added to
  `flexcore-audit` without breaking a recorded P1.10 shape and its test.
- **flexcore-storage**: no audit store module existed (the audit table's
  storage counterpart is migration 0008 + the P1.13 adapter INSERT only).
- **API surface** (`apps/control-plane/src`): no `/api/v1/audit-events`
  route, DTO, OpenAPI path, or authorization action existed anywhere. The
  only prior trace is the FROZEN candidate list
  (`flexcore-contracts::api_paths::REST_RESOURCE_PATHS`, report §15.2
  verbatim) — a candidate path, not an implementation.
- **Conclusion**: no existing read API to close the subtask against; the
  read side was built as specified, with exactly ONE audit-events read
  contract (the "exactly-one rule" holds — no second, divergent path).

## 2. What was added

### 2.1 Storage contract — `flexcore-storage/src/audit_read.rs` (NEW)

`AuditEventReader` trait (SELECT-only) + `AuditEventRow` + `AuditEventFilter`
+ `AuditEventReadError`:

- Two methods only: `find_audit_event` (Option — a foreign-tenant or
  platform-level record reads as `None`) and `list_audit_events` (ordered by
  `(event_time, audit_id)`; the caller ALWAYS bounds the page — default 200,
  maximum 1000, the recorded P2.12/P3.28/P4.30 listing convention; no
  unbounded query).
- Home (**D-P4.36-1**, DEFAULT): flexcore-storage, under the same recorded
  home deviation as `schedule_store`/`command_store`/`feasibility_check`
  (D-P3.21-4: storage-side record-store contracts) — NOT flexcore-audit,
  because the P1.10 `AuditSink` port's recorded single-method write-valve
  shape (and its source-text test) forbids a read method there. The write
  valve stays untouched: nothing was added to `crates/flexcore-audit`.
- Filters (ledger step 2): exact `action` (the event-class discriminator,
  e.g. `command.approve`), `actor`, `object_type` + `object_id` (the
  command/schedule/entity filter shape — a command read names
  `object_type = "command"` and the command id), inclusive
  `from_event_unix_millis`/`to_event_unix_millis`, and the keyset cursor
  `after_audit_id`. Time bounds are INCLUSIVE on both ends
  (**D-P4.36-2**, DEFAULT: no prior phase pinned window inclusivity; the
  choice is recorded here and asserted in both test suites).
- `AuditEventRow::new` enforces the storage-level rules before anything is
  served: identities non-empty; the decision halves (`decision_outcome`,
  `decision_reason_reference`) present together with the outcome in the
  closed `ALLOWED`/`DENIED` vocabulary (a decision is outcome + reason
  reference, never a bare allow/deny — P1.11/P1.12 §4.7); `ingest_time` not
  before `event_time` (P0.09); a non-empty correction linkage when present.
- In-module source-text test: the trait declares exactly the two read
  methods and no insert/update/delete/append/write/amend/revise/edit/
  export/dump method may ever appear (ledger step 3 — the P1.10 port rule
  applied at the API layer; the append-only rule is preserved end to end,
  not re-stated as prose).
- Tenant scoping is a BINDING adapter obligation on the trait docs: explicit
  tenant filter on top of the `tenant_isolation` RLS policy; existence never
  leaks across a tenant boundary (§16.2, P0.16 rule 5). Platform-level
  NULL-tenant records are deliberately invisible to this port (P1.12 §5.2 —
  the 0008 policy matches no tenant context for them); the contract does not
  widen that least-privilege boundary.

### 2.2 PostgreSQL adapter — `crates/adapters/postgres/src/audit_read.rs` (NEW)

`impl AuditEventReader for PgUnitOfWork` on the exact P4.30
`feasibility_check.rs` pattern:

- SELECT-only projection over `audit_records` (migration 0008 — the table is
  already append-only by grants + trigger; NO new migration,
  **D-P4.36-3**, DEFAULT: the read API needs no schema change).
- Explicit `tenant_id = $1::uuid` filter on top of RLS; bound parameters
  everywhere (a small `Bind` enum counts placeholders in push order so no
  predicate and the LIMIT can collide — the listing statement is always
  LIMIT-terminated).
- Keyset cursor = anchor subselect on `(event_time, audit_id)` over the
  cursor record (same shape as the P4.30 check listing).
- `event_time`/`ingest_time` are `timestamptz` (0008) — decoded via the
  driver's UTC `OffsetDateTime` and converted to UTC unix millis at the
  contract boundary (the feasibility table stores millis; 0008 predates that
  convention, so the conversion lives in the adapter — recorded).
- jsonb reference columns decode through the `col_json_ref` discipline
  (JSON string round-trips exactly); the `decision` jsonb decodes into its
  two halves.
- In-module source-text test: the module never issues a write statement
  (statement heads assembled with `format!` so the test does not contain
  them). `lib.rs`'s `every_implemented_port_is_object_safe` evidence test
  extended additively with the reader.
- Runtime role unchanged: `flexcore_app` holds INSERT+SELECT on
  `audit_records` since 0008 — the read port exercises the SELECT grant
  only (least privilege).

### 2.3 REST resource — `apps/control-plane/src/routes_phase4.rs` + `dto_phase4.rs` (additive)

- `GET /api/v1/audit-events` — tenant-scoped listing (tenant from
  authenticated claims, never a request field, §16.2), filters as §2.1,
  keyset `cursor`, `limit` (default 200 / max 1000, out-of-range = 422).
- `GET /api/v1/audit-events/{event_id}` — one record with EVERY §10.1
  reference preserved end to end (decision + reason reference, evidence
  reference, before/after references, correction linkage): never a summary
  that drops references (P5.09's consumption contract). Malformed id = 404
  (a resource reference, not a filter); cross-tenant = 404, no existence
  leak.
- DTOs: `AuditEventDto`, `AuditEventPageEnvelope`, `AuditEventListParams`,
  mapper `audit_event_dto`, and the carried statement
  `AUDIT_APPEND_ONLY_STATEMENT` (read-only + append-only + no
  export/dump + corrections-are-new-records), asserted by the contract
  tests.
- **NO generic export/dump endpoint** — none exists and none may be added
  (§15.2 prohibition + the append-only rule); the contract test probes
  export/dump-shaped paths for 404.
- **§21.3 correlation**: every served record carries actor (an identity
  reference, never a credential), the audited object pair, the decision +
  reason reference, the evidence reference, the distinct P0.09 event/ingest
  times, and the trace id. No secret and no unnecessary personal or
  customer data appears in any field (structurally asserted in the contract
  test).

### 2.4 Authorization + OpenAPI (additive)

- `flexcore-authz/src/action.rs`: new closed-enum variant `AuditEventRead`,
  id `audit.event.read` (variant + `id` arm + `parse` arm + round-trip test
  entry — the closed-enum extension rule). No `audit.event.write` /
  `delete` / `export` / `append` action exists; the contract test pins that
  they parse to `None`.
- `apps/control-plane/src/auth.rs`: `phase4_default_policies` grants
  `audit.event.read` to all four roles at tenant scope (the same read-surface
  discipline as `command.read`/`feasibility.read`; deny-by-default holds —
  the empty policy set is 403 in the contract test).
- `openapi.rs`: both paths registered from the route annotations (the spec
  stays GENERATED, never hand-written); schemas registered; the info
  description extended with one sentence. The contract test asserts the
  generated spec carries both paths with GET operations ONLY.

### 2.5 Wiring + fakes (additive)

- `state.rs`: `UnitOfWork` (new `audit_events` field), `Backend::Test`, the
  Postgres one-transaction coercion (`audit_events: shared.clone()` — the
  read runs inside the caller's unit of work, RLS applied), and a NEW
  `for_tests_with_phase4_audit` constructor. `for_tests_with_phase4` keeps
  its recorded signature and defaults the audit fake internally, so NO
  existing call site or existing test changed.
- `phase4_fakes.rs`: `FakeAuditEventReader` (tenant-scoped, same filter/
  keyset semantics as the adapter; `seed` is a TEST-ONLY helper — the port
  itself has no write method).
- `lib.rs` (control-plane) module comments updated; `routes.rs` needed no
  change (the routes live in `phase4_router()`); `main.rs` needed no change
  (the Postgres composition root constructs no record stores directly).

## 3. Tests (all written, NONE executed — UNRUN)

- `apps/control-plane/tests/api_phase4.rs` — additive test functions ONLY
  (the P4.30 tests above them are untouched): 401 unauthenticated on both
  routes; 403 under the empty policy set; least-privilege action id +
  absence of write/delete/export actions; list + detail OpenAPI-shape
  validation against the GENERATED spec; tenant scoping in listings and
  cross-tenant detail = 404 (both directions, no leak); full §21.3
  correlation + append-only statement + no-secret structural check; class /
  time-window / object filters; malformed filter/cursor/limit = 422;
  GET-only structural check + live POST probes 405 + export/dump-shaped
  paths 404.
- `crates/adapters/postgres/tests/audit_read.rs` (NEW, env-gated on
  `FLEXCORE_TEST_DATABASE_URL`, skips cleanly when unset): round trip from
  the P1.10 `AuditSink` write path through the read port with every
  reference preserved (decision halves, evidence, after-reference,
  correction linkage; the original record unchanged on re-read); foreign
  tenant reads as `None` and leaks nothing into listings; action filter,
  INCLUSIVE time window, keyset cursor paging (no gaps/overlaps), object
  filters. Uses the recorded harness pattern (per-test migrations, fixture
  UUIDs with a 0xC4 marker, per-test committed units of work).

## 4. Honest limitations / open items

- **Nothing built or run** (loop bans): no `cargo build`, no `cargo test`,
  no docker, no database. The DB-gated suite has never executed; migrations
  were never applied (none were added). All Rust above compiles only in the
  sense of careful review against the existing patterns — the human
  build/test pass is the gate.
- **§16.2 denial logging**: the §16.2 "audit cross-tenant access denials"
  duty is NOT wired in this subtask (**D-P4.36-4**, recorded): the routes
  return the reasoned 403 (action + deny reason, per the P1.17 pattern) and
  the P4.28 `SecurityAuditSink` seam remains the recorded audit point for
  denials; adding a second denial-audit path here would create exactly the
  divergent duplicate the verify-first step exists to prevent. Cross-tenant
  READ attempts are NOT distinguishable from unknown-id reads by design
  (both 404 — no existence leak), so there is nothing to log without
  weakening that rule. Flagged for the composition root / Phase 5 review.
- **No `audit_records` partitioning/retention**: 0008 has no TTL (P0.16 §4);
  unchanged here.
- **Action filter is exact-match** (DEFAULT): the event-class filter matches
  the whole `action` string, not a dotted prefix. A prefix query can be
  added later as an additive filter field without breaking the contract
  (§21.4 — 0.x may evolve with a documented note).
- **P5.09 consumption contract** (what Phase 5 may rely on): GET
  `/api/v1/audit-events` with the filters and keyset cursor of §2.1, GET
  `/api/v1/audit-events/{event_id}`, response schemas `AuditEventDto` /
  `AuditEventPageEnvelope` in the generated OpenAPI, action
  `audit.event.read` (all four roles, tenant scope), every §10.1 reference
  served on every record, and the append-only guarantee (no update/delete/
  export endpoint exists or will be added to this resource).

## 5. Files touched

| File | Change |
|---|---|
| `crates/flexcore-storage/src/audit_read.rs` | NEW — the read contract + tests |
| `crates/flexcore-storage/src/lib.rs` | module registered (docs) |
| `crates/adapters/postgres/src/audit_read.rs` | NEW — the PgUnitOfWork impl + tests |
| `crates/adapters/postgres/src/lib.rs` | module registered; port-table row; object-safety evidence extended |
| `crates/adapters/postgres/tests/audit_read.rs` | NEW — DB-gated suite (UNRUN) |
| `crates/flexcore-authz/src/action.rs` | additive `AuditEventRead` variant/id/parse/test entry |
| `apps/control-plane/src/routes_phase4.rs` | additive routes + handlers + error mapping |
| `apps/control-plane/src/dto_phase4.rs` | additive DTOs + statement + mapper |
| `apps/control-plane/src/auth.rs` | additive grant + docs |
| `apps/control-plane/src/openapi.rs` | additive paths/schemas/description |
| `apps/control-plane/src/state.rs` | additive UnitOfWork/Backend/constructor wiring |
| `apps/control-plane/src/phase4_fakes.rs` | additive `FakeAuditEventReader` |
| `apps/control-plane/src/lib.rs` | module comments |
| `apps/control-plane/tests/api_phase4.rs` | additive P4.36 test fns only |
| `ledger/phase-04-grid-feasibility-dispatch/ledger_phase_4.md` | P4.36 checkbox ticked |
| `ledger/phase-04-grid-feasibility-dispatch/HANDOVER.md` | wave 6b row updated |
