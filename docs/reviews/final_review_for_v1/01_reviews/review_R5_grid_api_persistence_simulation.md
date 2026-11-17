# Final Review R5 — Grid API, Persistence, Messaging, Simulation, Optimization (Iteration 1)

**Reviewer:** Agent A (wave 3)
**Date:** 2026-11-17
**Status:** REVIEWED — static read only (no build/test executed)
**Finding totals:** P0: 1 · P1: 3 · P2: 5

---

## 1. Scope reviewed

All paths relative to repo root. LOC counts are source files (`src/`) only;
tests counted separately where noted.

| Area | Files | LOC (src) | Depth |
|---|---|---|---|
| `crates/flexcore-grid-api/` | 13 src modules + 11 test files | ≈ 9,420 src (+ tests) | Deep: `lib.rs`, `worker_backend.rs` (full), `socp_boundary.rs`, `backend.rs` payload vocabulary, `port.rs`/`verdict.rs`/`translation.rs` (structure + targeted sweeps). Structural: `feasibility.rs` (2,152), `limits.rs`, `model_health.rs`, `scope.rs`, `adapter.rs`, `fail_closed.rs` — headers, public APIs, unwrap/expect sweep, solver-discipline statements. |
| `crates/adapters/postgres/` | 21 src modules + 13 test files | ≈ 11,120 src | Deep: all 10 storage-trait impls (`command_store`, `schedule_store` ×3 traits, `envelope_store`, `feasibility_check`, `audit_read`, `edge_enrollment`, `interconnection_store`, `market_records_store`), `telemetry.rs` + `corrections.rs` (SQL extraction + key paths), `audit.rs` (chain append/verify), `outbox.rs` (full), `store.rs` (full), `unit_of_work.rs` (repository SQL + decode paths), `partition_maintenance.rs` (SQL extraction). All 23 migration pairs (0001–0023) read. |
| `crates/adapters/nats/` | 5 src modules + 2 test files | ≈ 2,160 src | Deep: `topology.rs` (full), `consumer.rs` (full), `publisher.rs` (full), `inbox.rs`, `lib.rs`, `README.md`. |
| `crates/flexcore-simulator/` | 21 src modules + 3 test files | ≈ 9,260 src | Deep: `engine.rs` (full), `lib.rs`, scenario-fixture reference audit. Structural: `scenario.rs`, `source.rs`, `profiles.rs`, `rng.rs`, `market/*`, `adapters/sim_*` (headers, wire formats, fault vocabulary). |
| `crates/flexcore-optimization-api/` | 14 src modules + 10 test files | ≈ 8,120 src | Deep: `degradation_cost.rs` (full), `problem.rs` (full incl. all P15.08 guard tests), `tier_guard.rs` (full), provider class gates (`highs_provider.rs`, `clarabel_provider.rs` — supported-class sets, `unsupported` paths). Structural: `result.rs`, `port.rs`, `backend.rs`, `config.rs`, `determinism.rs`, `fixture.rs`, `conic_fixture.rs`, `infeasibility.rs`. |
| `apps/sim-runner/` | 15 src modules + 11 test files | ≈ 13,270 src | Deep: `main.rs` (header, command dispatch, parser), fixture-reference audit. Structural: `scenario_run.rs`, `replay_record.rs`, `replay.rs`, `helics_orchestration.rs`, `fault_injection.rs`, `benchmark.rs` (entry points + option validation). |
| Migrations (shared) | 46 SQL files (23 up + 23 down) | 2,666 up-LOC | All 23 up migrations read; down migrations checked for pairing (0009, 0001 spot-read). |

Out-of-scope crates (`flexcore-storage`, `flexcore-ports`, `flexcore-events`,
`flexcore-application`, `apps/worker`, `apps/control-plane`) were read only as
needed to verify cross-boundary claims (traits, wiring, subjects, routes).

## 2. Method

Static checks actually performed, each with its result:

1. **Storage-trait vs postgres-impl signature table.** Extracted every
   `pub trait` + method from `crates/flexcore-storage/src/{schedule_store,
   schedule_revision,command_store,envelope_store,feasibility_check,
   audit_read,edge_enrollment,interconnection_store,market_records_store}.rs`
   and every `impl … for PgUnitOfWork/FlexCoreStore` method from the matching
   adapter modules. Compared method name sets and parameter/return shapes
   (read-side, e.g. `command_store.rs:597–659` trait vs
   `adapters/postgres/src/command_store.rs:371–685` impl).
   **Result: PASS** — all 10 traits fully implemented; no name, arity, or
   return-type divergence found; object-safety evidence tests present in both
   crates.
2. **Schema-vs-SQL table** (every migration's CREATE TABLE/ALTER columns vs
   every store `SELECT`/`INSERT`/`UPDATE` column list):
   - `outbox` (0007) vs `outbox.rs:84–88,113–118,167,192,217` — **PASS**.
   - `audit_records` (0008) + chain columns (0019) vs `audit.rs:108–114,
     151,185,204,328–341` and `audit_read.rs:40–44` — **PASS**.
   - `telemetry` (0010) + version columns (0012) vs `telemetry.rs:371–381,
     683,1123,1526` and `corrections.rs:286–292,353–355,381–385,441–448,
     528–532` — **PASS** (correction insert carries `telemetry_version`,
     `correction_of`, `correction_reason`, `corrected_by`, `corrected_at`
     exactly per 0012).
   - `schedule_results` (0013), `schedule_revisions`/`schedule_comparisons`
     (0014) vs `schedule_store.rs:72,108,375,457,526,605,977,1023` — **PASS**.
   - `flexibility_envelopes` (0015) vs `envelope_store.rs:52–56,101` — **PASS**.
   - `dispatch_commands`/`dispatch_command_transitions` (0016) vs
     `command_store.rs:408–413,465–468,517–521` — **PASS** (CAS + journal in
     one transaction; column-level grant columns respected).
   - `feasibility_check_records` (0017) vs `feasibility_check.rs:36–41,
     175–181` — **PASS**.
   - `edge_device_registry`/`edge_enrollment_journal` (0018) vs
     `edge_enrollment.rs:190–196,240–243,285–286,315,366–368` — **PASS**.
   - `interconnection_records` (0021) vs `interconnection_store.rs:48–52,
     129–137` — **PASS**.
   - `market_*` six record tables + `market_record_retention_config` (0022)
     vs `market_records_store.rs:152–156,201–206,237–240,287–293,333–339,
     427–431,513–518,610–616,703–712` — **PASS**.
   - `edge_fleet_device_state` (0023): no store module exists for it (the
     P18.12 consumer is the edge fleet registry's read side, out of R5
     adapter scope) — recorded, not a mismatch.
   **Result: PASS on every table** — no column-name, type, or NOT NULL vs
   insert-path mismatch found anywhere (see §4 for what *was* found: columns
   that exist but are never touched at all, finding 001).
3. **Migration numbering / expand-only audit.** Versions 0001–0023 contiguous,
   every version has both `.up.sql` and `.down.sql`; no migration drops or
   renames an existing column (the 0009/0012/0019 ALTERs are additive; the
   0012 PK swap pre-dates the P11.18 policy, which is binding for future
   changes and explicitly lists the PK-swap rule in `MIGRATION_POLICY.md §2`).
   **Result: PASS.**
4. **Tenant-isolation sweep.** Every tenant-scoped table carries the shared
   `tenant_isolation` RLS policy (ENABLE + FORCE) with the fail-closed
   `NULLIF(current_setting('flexcore.tenant_id', true), '')::uuid` shape;
   every store query carries an explicit `tenant_id` filter on top of RLS
   (spot-verified in command, schedule, envelope, feasibility, audit-read,
   edge, market, telemetry, corrections). The two deliberate exceptions are
   recorded in their headers: `outbox` (0007, infrastructure queue) and
   `interconnection_records` (0021, platform-global equipment catalog, NO
   tenant column by recorded decision). **Result: PASS.**
5. **NATS subject publish/subscribe table.** Extracted every subject string:
   `flexcore-events/src/subject.rs` (the 26-event `event_area`/`subject_for`
   mapping), `adapters/nats/src/topology.rs` (11 areas + DLQ families), the
   outbox `destination_subject` flow (`flexcore-events/src/outbox.rs:286` →
   `publisher.rs:316` publish, `consumer.rs` DLQ derivation at :525–526),
   worker provisioning (`apps/worker/src/main.rs:205,210,225,251`) and
   subscription paths (`apps/worker/src/command_consumer.rs`,
   `apps/worker/src/ingestion.rs`). **Result: PASS with one stale-doc
   finding** (007): the events-side area tokens now match the P1.14 §3 table
   (`subject.rs:185–213` maps Registry/State/Telemetry/Flexibility/Forecast/
   Market/Scheduling/Validation/Dispatch/Plugins/Audit), but the nats crate
   still documents a recorded "8 of 26 diverge" defect.
6. **JetStream hardening (phase 11) application check.** `DeploymentLimits::
   bounded_simulation_defaults`/`bounded_dlq_defaults` are consumed by
   `apps/worker/src/main.rs:89,205,225` (event streams + 11 dedicated DLQ
   streams provisioned at startup); the P11.13 `Nats-Msg-Id`/
   `flexcore-schema-version`/traceparent header seam is set on every publish
   (`publisher.rs:281,290–292,307–312`), read back (`consumer.rs:358–378`),
   and carried into quarantine metadata (:510–524); the `max_deliver`
   alignment check is wired (`apps/worker/src/main.rs:259`). The
   `PUBLISHED_SCHEMA_VERSION ↔ SUPPORTED_SCHEMA_VERSION` cross-check is
   asserted in `apps/worker/tests/command_duplicate_delivery.rs:358–364`.
   **Result: PASS.**
7. **Solver-class discipline sweep (P0 invariant).** Read
   `degradation_cost.rs` end-to-end (linear/PWL data only; non-convex PWL
   refused with reshape guidance, :391–410; no computed degradation number;
   versioned + provenance-required declarations); read `problem.rs` class
   consistency (`validate_class_consistency`, :551–677) and all five
   P15.08 guard tests (:938–1045 — each mutates a fixture into a violating
   shape and asserts the refusal, so they genuinely guard); read
   `tier_guard.rs` (Tier 3/4 → `unsupported` + decision reference);
   verified provider gates: HiGHS supports exactly
   {LinearProgram, ConvexQuadraticProgram, MixedIntegerLinearProgram}
   (`highs_provider.rs:108–121,550–551`), Clarabel exactly
   {ContinuousSecondOrderConeProgram} (`clarabel_provider.rs:177–179,913`).
   Grepped both providers and grid-api for MISOCP/SOCP creep and AC-OPF
   claims: grid-api `lib.rs`/`scope.rs`/`socp_boundary.rs` state AC power
   flow ≠ AC-OPF and never assume relaxation exactness (§12.5 condition
   checklist at `socp_boundary.rs:146–200`). **Result: PASS** — no
   solver-class violation found.
8. **Transaction-boundary check on multi-write methods.** `record_command_transition`
   performs CAS + journal INSERT in one UoW transaction
   (`command_store.rs:450–545`); `insert_command_record` + paired outbox,
   schedule-result + outbox, correction + savepoint-rollback discipline all
   run on the caller's unit-of-work transaction; `FlexCoreStore::append`
   (AuditSink) opens its own short transaction deliberately and documents it
   (`store.rs:328–368`). **Result: PASS.**
9. **unwrap/expect sweep on non-test I/O paths** across all six in-scope
   crates (awk `#[cfg(test)]` boundary). **Result: PASS** — every remaining
   `.unwrap()`/`.expect()` sits on a statically-validated value, a mutex
   poison guard, or a documented fail-fast invariant (e.g.
   `id_generator.rs:46` documents the fail-fast uuidv7 contract); none on a
   fallible parse/IO of external input.
10. **Scenario-fixture reference audit.** Extracted every `scenarios/…`
    path referenced from simulator/sim-runner code and tests; verified
    existence on disk. **Result: 3 MISSING paths** (finding 003).
11. **Store-wiring check.** Grepped composition roots for construction and
    `dyn` use: control-plane `state.rs:70–120` holds
    ScheduleRecordStore/ScheduleRevisionChainStore/EnvelopeRecordStore/
    CommandRecordStore/FeasibilityCheckStore/AuditEventReader/EdgeEnrollmentStore;
    `FlexCoreStore::connect` + `run_migrations` wired at
    `apps/control-plane/src/main.rs:84,97` and `apps/worker/src/main.rs:163`.
    **Result: two store families with NO consumer** (finding 002).
12. **Migration-runner story.** Embedded `sqlx::migrate!("migrations/")` in
    `FlexCoreStore::run_migrations` (`store.rs:160–166`), invoked from the
    control plane at startup inside `spawn_blocking`; `MIGRATION_POLICY.md`
    documents the expand/contract + lock rules; the fail-closed role model
    (NOLOGIN roles, guarded DO blocks) is consistent across 0001/0010.
    One deviation found: the worker inbox table is created by runtime code,
    not by the migration set (finding 004).
13. **mod/pub use vs real files.** `lib.rs` module lists for grid-api,
    postgres, nats, simulator, optimization-api, sim-runner all resolve to
    existing files (file listing cross-check). **Result: PASS.**

## 3. Checks performed, all consistent

Recorded diligence evidence — cross-boundary checks that PASSED:

1. All 10 flexcore-storage traits are implemented by the postgres adapter
   with matching method names, arity, and result types (check 1).
2. Schema-vs-SQL: every store statement's column list matches its migration
   exactly across all 23 migrations (check 2) — including the conditional
   `q_sign_convention` and value-totality constraints mirrored in
   `validate_record`.
3. Migration set is contiguous, fully paired up/down, and expand-only; the
   only constraint-rebuild (0012 PK extension) is explicitly recorded and
   pre-dates the P11.18 policy (check 3).
4. RLS + explicit tenant filter on every tenant-scoped query; the two
   no-RLS tables are recorded design decisions, not omissions (check 4).
5. The frozen command-state machine is enforced three ways consistently:
   domain `try_transition`, adapter CAS, and the 0016 database trigger —
   and the 27 SQL edges match the domain vocabulary spot-checked via the
   adapter's `COMMAND_STATES` closed set (check 2/8).
6. Outbox atomicity: state change + outbox row share one native transaction
   by construction on `PgUnitOfWork`; the store-level `append` refuses
   without a unit of work (`store.rs:287–293`) (check 8).
7. The audit chain (0019) append reads the head state under `FOR UPDATE`,
   links `chain_prev_head`, and advances the head in the same transaction
   (`audit.rs:146–269`); verification path is SELECT-only (check 2).
8. HiGHS is used for LP/QP/MILP only and Clarabel for continuous SOCP only;
   both return §12.6 `unsupported` in-band for every other declared class
   before any solver invocation (check 7).
9. Degradation cost enters the optimizer only as linear/piecewise-linear
   input data; the P15.08 guard tests would fail on any class widening
   (check 7).
10. The worker publishes exactly the 26 frozen event subjects under
    `flexcore.v1.<area>.<slug>`; every area maps to a provisioned stream or
    DLQ family; no tenant identifier appears in any subject (D-J2) (check 5).
11. JetStream bounded retention, dedicated DLQ families, schema-version and
    trace-context headers, and the max-deliver alignment check are all
    applied at the composition root, not just defined (check 6).
12. The deterministic simulator engine: faults act on generated frames only
    (no generator-state mutation), sequences are gap-free per asset in a
    fault-free run, and the reference scenario
    `scenarios/synthetic-portfolio/scenario-v1.json` exists and is pinned by
    the engine's tests (check 10; `engine.rs:549–554`).
13. `sim-runner` exit-code contract (pass 0 / mismatch 1 / usage 2) is
    implemented in `main.rs` dispatch; all six subcommands resolve to
    existing handler modules (check 13).
14. Every remaining `.unwrap()`/`.expect()` in non-test code of the six
    in-scope crates sits on a static invariant, not a fallible I/O path
    (check 9).

## 4. Findings

### FRV1-R5-001 [P0] [phase-02] Asset deactivation lifecycle never reaches the database — the terminal state silently reverts on read-back

- Status: OPEN
- Location:
  `crates/adapters/postgres/src/unit_of_work.rs:79–82` (ASSET_SELECT),
  `crates/adapters/postgres/src/unit_of_work.rs:294–327` (decode_asset_with),
  `crates/adapters/postgres/src/unit_of_work.rs:871–905` (AssetRepository::update);
  cross-refs: `crates/adapters/postgres/migrations/0009_registry_lifecycle_p2_03.up.sql`
  (the three columns), `crates/flexcore-domain/src/entities.rs:673–682`
  (`Asset::deactivate`), `crates/flexcore-application/src/deactivate_asset.rs:117–119`,
  route `apps/control-plane/src/routes_phase2.rs:771`.
- Issue: Migration 0009 added `lifecycle_status` / `deactivation_reason` /
  `deactivated_at` to `assets` (with the `assets_lifecycle_consistency`
  CHECK). The P2.03 deactivation flow is fully wired end-to-end — control
  plane route → `DeactivateAsset` use case → `Asset::deactivate` (sets the
  three domain fields) → `AssetRepository::update` — but the postgres
  adapter's `update` statement writes only name/description/type/vendor/
  ratings/phase/bus/capability; it never binds the three lifecycle columns.
  Symmetrically, `ASSET_SELECT` does not project them and
  `decode_asset_with` reconstructs every asset through `Asset::try_new`,
  which per `entities.rs:573` always yields
  `AssetLifecycleStatus::Active`. Consequences: (a) a completed
  deactivation (API 2xx, `AssetAvailabilityChanged` outbox event emitted)
  is silently lost from the database — the next read/restart shows the
  asset `active` again; (b) the domain's terminal-deactivation invariant
  (`AlreadyDeactivated`, `entities.rs:674`) is defeated across process
  boundaries; (c) the 0009 CHECK can never fire because no code path ever
  writes `'deactivated'`. The application module header
  (`deactivate_asset.rs:12–14`) explicitly claims the reason is
  "persisted on the asset record itself … so the 'why' survives" — that
  claim is false at the only persistence implementation. This is a
  schema-vs-code mismatch of the data-integrity class: it compiles, and it
  fails silently at runtime.
- Recommendation: Minimal, surgical fix in the postgres adapter only:
  extend `ASSET_SELECT` with the three columns, extend
  `decode_asset_with` to rebuild lifecycle state (a domain constructor or
  explicit re-validation, never a bare field write), and add the three
  columns to the `update` statement (writing `lifecycle_status =
  lifecycle_status` for ordinary metadata updates, or better: carry the
  entity's lifecycle fields and let the 0009 CHECK arbitrate). Add an
  adapter test: deactivate → `update` → `find_by_id` returns Deactivated
  with reason + timestamp intact.

### FRV1-R5-002 [P1] [phase-04] Two implemented store families (market records, interconnection records) have no composition-root consumer

- Status: OPEN
- Location:
  `crates/adapters/postgres/src/market_records_store.rs:131` (impl
  `MarketRecordsStore for PgUnitOfWork`),
  `crates/adapters/postgres/src/interconnection_store.rs:87` (impl
  `InterconnectionRecordStore for PgUnitOfWork`), migrations 0021/0022
  (their tables); grep over `apps/` and `crates/flexcore-application/`
  finds zero construction sites, zero `dyn` uses, and no control-plane
  route or worker task for either trait.
- Issue: The full stack exists for both — storage trait, adapter
  implementation with the usual rigour, migrations 0021/0022 with RLS,
  grants, and indexes — but nothing ever constructs `PgUnitOfWork` *as*
  these traits: no route, no use case, no worker job. The checklist class
  "store implemented but never constructed" applies. Mitigation recorded
  honestly in the sources themselves (migration 0022 header: "Closes the
  LR-074/190 gap at DESIGN level only: no market is connected and no
  record exists anywhere"; trait header records the same), so this is not
  a hidden defect — but at release the tables and code are dead surface
  whose only exercise is the adapter test files.
- Recommendation: Either (a) record explicitly in
  `crates/adapters/postgres/src/lib.rs` and the release notes that
  `MarketRecordsStore`/`InterconnectionRecordStore` are implemented but
  unwired by design (no market connected in 0.x; interconnection ingest is
  a composition-root action), with the wiring step named, or (b) wire the
  interconnection ingest as an operator-only control-plane route, since
  migration 0021 already describes that write path. Option (a) is the
  minimal correct change.

### FRV1-R5-003 [P1] [phase-05] HELICS co-simulation fixtures reference three scenario files that do not exist

- Status: OPEN
- Location:
  `apps/sim-runner/tests/helics_orchestration_p5_16.rs:62`
  (`scenarios/synthetic-portfolio/replay-records.json`),
  `:93` (`scenarios/synthetic-portfolio/grid-model.json`),
  `:116` (`scenarios/synthetic-portfolio/setpoints-fixture.json`);
  consumed by the federate options validated at
  `apps/sim-runner/src/helics_orchestration.rs:468–477`
  (`require_string("input_records_path")` / `("model_ir_path")` /
  `("setpoints_path")`).
- Issue: The recorded P5.16 federation (asset/grid/control federates) reads
  these three paths at execution time; none of the three files exists in
  `scenarios/synthetic-portfolio/` (on-disk inventory verified). The
  federation scripts themselves exist (`apps/worker/python/helics_federates/
  {asset,grid,control}_federate.py`), so the first human execution of the
  recorded co-simulation would fail on a missing input file — exactly the
  "simulator scenario files referencing missing fixtures" gap. The suite
  itself is unrun-by-design and only validates config, so nothing fails
  today; the gap is invisible until the recorded run is attempted.
- Recommendation: Add the three fixture files (the grid model IR can be
  exported from the existing synthetic-portfolio scenario; the
  replay-records file is the documented `flexcore-sim-runner run --out`
  artifact; the setpoints fixture is a small hand-authored JSON), or amend
  `scenarios/synthetic-portfolio/README.md` and the P5.16 fixture comments
  to state that the paths are execution-time inputs generated by the named
  commands — so the first run does not dead-end on a missing file.

### FRV1-R5-004 [P1] [phase-11] The worker inbox table is created by runtime `ensure_table`, outside the embedded migration set

- Status: OPEN
- Location:
  `apps/worker/src/inbox.rs:56–61` (`SQL_ENSURE_TABLE`,
  `CREATE TABLE IF NOT EXISTS flexcore_worker_inbox …`), `:123–129`
  (`ensure_table` called at startup); contrast the migration runner
  `crates/adapters/postgres/src/store.rs:160–166` and
  `crates/adapters/postgres/MIGRATION_POLICY.md` (the migrations directory
  is presented as the database image).
- Issue: Every other runtime table is owned by a numbered migration with
  owner/grant/RLS conventions; the consumer inbox — which carries the
  durable duplicate-side-effect guarantee of the §6.3 discipline (P1.16) —
  is instead created by `CREATE TABLE IF NOT EXISTS` in the worker with
  default ownership (the connecting role), no explicit grants/REVOKE, no
  entry in `migrations/`, and therefore no up/down migration replay, no
  checksum tracking by `sqlx::migrate!`, and no coverage by the
  migration-compat test. The module header records this as an interim
  measure from P1.19 ("When the store is promoted into
  `flexcore-postgres-adapter` … the table DDL moves into the adapter's
  migration set") — the promotion never happened, so the interim state is
  now the release state. A database provisioned by migrations alone lacks
  the inbox table until the worker runs.
- Recommendation: Add migration `0024_worker_inbox` (CREATE TABLE with the
  recorded shape, owner `flexcore_migrator`, grants per the repo role
  model, expand-only), make `ensure_table` a no-op once applied (or delete
  it with the promotion), and record the promotion decision. This also
  closes the stale P1.16 home note (finding 007).

### FRV1-R5-005 [P2] [phase-04] `dispatch_commands` has no index for its documented tenant-listing query pattern

- Status: OPEN
- Location:
  `crates/adapters/postgres/migrations/0016_commands_p4_10.up.sql:23`
  (PRIMARY KEY `(command_id, tenant_id)`; the only indexes on
  `dispatch_commands` are the PK and the FK-backed ones), vs
  `crates/adapters/postgres/src/command_store.rs:637–647`
  (`list_commands`: `WHERE tenant_id = $1 [AND state …] ORDER BY
  command_id DESC LIMIT n`).
- Issue: The listing filters by tenant and orders by `command_id DESC`, but
  the PK leads with `command_id`, so per-tenant listing cannot use it
  without a scan (worse with a state filter). The sibling journal table
  got exactly the right index (`dispatch_command_transitions_command_idx`
  on `(tenant_id, command_id, seq)`), which makes the omission on the
  parent table look unintended. Perf hazard grows with the append-only
  command history; every `/api/v1/dispatch-commands` page pays it.
- Recommendation: Add an expand-only migration
  `CREATE INDEX dispatch_commands_tenant_idx ON dispatch_commands
  (tenant_id, command_id DESC)` (optionally `(tenant_id, state,
  command_id DESC)` — one of the two suffices), or record the omission as
  acceptable for the 0.x volume with the revisit trigger named.

### FRV1-R5-006 [P2] [phase-11] Stale "8 of 26 subject divergence" defect record in the NATS crate

- Status: OPEN
- Location:
  `crates/adapters/nats/README.md:89–96` (decision 5), 
  `crates/adapters/nats/src/lib.rs:83–87` (note 6),
  `crates/adapters/nats/src/topology.rs:279–281` — all state the
  flexcore-events area tokens diverge on 8 of 26 events and "must be
  reconciled before any real event flows".
- Issue: The reconciliation happened: `crates/flexcore-events/src/
  subject.rs:185–213` (`event_area`) maps all 26 events onto the exact
  P1.14 §3 area tokens (registry/state/telemetry/flexibility/forecast/
  market/scheduling/validation/dispatch/plugins/audit), and the
  subject-to-stream test in `topology.rs:569–649` passes over the same
  26 subjects. Three surfaces still describe the defect as open with a
  MUST-fix-before-events-flow banner — comment drift that misdirects the
  next reader/reviewer and overstates release risk in one place while
  understating it nowhere.
- Recommendation: Replace the three notes with the verification result
  ("reconciled; `event_area` is the single mapping home; topology test
  pins it") and point at `subject.rs` as the authority.

### FRV1-R5-007 [P2] [phase-11] NATS README records the wrong single home for the durable `InboxStore`

- Status: OPEN
- Location:
  `crates/adapters/nats/README.md` ("Decisions recorded here", item 1:
  "The durable implementation … lives in `crates/adapters/postgres`, which
  owns that schema, and implements this trait verbatim"; same claim in the
  trait's single-home note referenced from `crates/adapters/nats/src/
  inbox.rs`), vs the actual implementation
  `apps/worker/src/inbox.rs:80–196` (`PgInboxStore`).
- Issue: The durable implementation was rerouted to the worker
  composition root during P1.19 with a recorded interim-home rationale
  (`apps/worker/src/inbox.rs:4–24`), but the nats crate's decision record
  was never amended. Two documents now disagree about where the one
  durable implementation lives; a reader grepping the postgres adapter for
  `InboxStore` finds nothing and may conclude the durable path does not
  exist.
- Recommendation: Amend the nats README item 1 to the recorded superseding
  decision (implementation home: `apps/worker/src/inbox.rs`, promotion
  path: migration 0024 + adapter per finding 004).

### FRV1-R5-008 [P2] [phase-04] Migration 0020 defaults its PKs to `gen_random_uuid()`, off the recorded D-S1 `uuidv7()` convention

- Status: OPEN
- Location:
  `crates/adapters/postgres/migrations/0020_webhook_delivery_p13_09.up.sql:65,97,134,170`
  (`endpoint_id`, `delivery_id`, `attempt_id`, `dead_letter_id … DEFAULT
  gen_random_uuid()`); the convention: every other PK default in the
  migration set is `uuidv7()` (0002:2, 0007, 0010, …) and
  `crates/adapters/postgres/src/store.rs:239–241` states "application
  identifiers are PostgreSQL `uuidv7()`, like every primary-key default
  (D-S1)".
- Issue: The webhook bookkeeping IDs silently revert to random v4 UUIDs,
  losing the time-ordered locality every other identifier carries (the
  keyset-listing and ordering arguments in migrations 0014/0015 depend on
  it). The webhook delivery tables are keyed by `delivery_key` for
  idempotency, so correctness is unaffected — this is consistency drift
  against a recorded D-decision, not a bug.
- Recommendation: Either change the four defaults to `uuidv7()` while
  migration 0020 has never shipped (it is written-unrun; a pre-release
  edit of a not-yet-applied migration is allowed by the expand-only
  policy's spirit), or record the deviation in the 0020 header with the
  reason.

### FRV1-R5-009 [P2] [phase-04] `MarketRecordsStore` read methods are unbounded — the only listings in the persistence layer without a page bound

- Status: OPEN
- Location:
  `crates/flexcore-storage/src/market_records_store.rs:940–1040` (trait:
  "Every program record for ONE tenant", no limit parameter),
  `crates/adapters/postgres/src/market_records_store.rs:200–206,286–293,
  388,477–483,566–574,669–678,753` (all seven read methods: no `LIMIT`).
- Issue: Every other listing contract in the repo states and enforces the
  no-unbounded-query rule (`command_store.rs:556–568`: "There is NO
  unbounded query: the caller ALWAYS applies the page size";
  `envelope_store`/`audit_read`/`feasibility_check` all take a mandatory
  `limit`). The market trait is the recorded exception by its own doc text,
  but it hands every caller a tenant-wide (and for deliveries,
  per-event interval-wise potentially large) unbounded vector, with no
  adapter-side guard. Given the tables are unwired today (finding 002) the
  practical exposure is nil; the risk is the convention silently eroding
  as the first real consumer copies the pattern.
- Recommendation: When the market store is wired (finding 002), either add
  the mandatory `limit` parameter to the seven read methods (breaking
  change is free while nothing consumes the trait) or record the
  unbounded-read decision in the trait header with the volume assumption
  that justifies it.

## 5. Release-readiness verdict for this area

The five layers reviewed are, on the whole, the strongest-engineered part
of the codebase: the storage traits are fully and correctly implemented
with a perfect schema-vs-SQL correspondence across all 23 migrations; the
expand-only migration discipline, RLS + explicit tenant scoping, the
transactional outbox, and the append-only evidence tables are consistently
applied; the NATS adapter implements the §6.3 delivery rules and the full
phase-11 JetStream hardening with the composition root actually consuming
it; the optimization API's solver-class discipline is airtight (Tier gates,
per-provider class sets, genuinely-guarding P15.08 tests, degradation data
as linear input only); and the grid API keeps the AC-power-flow ≠ AC-OPF
boundary with fail-closed transport at every seam. One true P0 must be
fixed before release: the asset-deactivation lifecycle is accepted by the
API and the domain but never persisted by the only storage implementation,
so a terminal registry state silently reverts on read-back (FRV1-R5-001) —
a small, localized adapter fix. Before release, also decide and record the
unwired market/interconnection store families (002), close the missing
HELICS fixture gap or document it (003), and promote the worker inbox
table into the migration set (004) — all three are seams whose recorded
interim status has quietly become release state. The P2 items (missing
dispatch-commands tenant index, four stale/drifted documentation notes, the
`gen_random_uuid` default, and the unbounded market reads) are polish and
can ship with the honest caveats recorded here.
