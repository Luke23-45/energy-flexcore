<!-- P10.13 — Schema migration compatibility test. TEST SUITE + compatibility
     record: the suite is REAL test files WRITTEN-UNRUN over the actual
     migration set (crates/adapters/postgres/tests/migration_compat_p10_13.rs,
     additive; tests/integration.rs's up/down/up replay is cited, not
     rewritten). Compatibility windows are RECORDED POSITIONS from the
     migration files' own decisions, not verified results — every run cell is
     EMPTY (nothing installed, nothing migrated, no PostgreSQL server exists
     in this loop). Downgrade is NOT supported by the runtime path (the
     embedded sqlx migrator applies UP only) — recorded honestly, with the
     irreversible register (0001 roles, 0012 correction rows). -->
# P10.13 — Schema Migration Compatibility Test (compatibility record + WRITTEN-UNRUN suite; every run cell EMPTY)

Date: 2026-11-14. Subtask: P10.13 (`ledger_phase_10.md` §P10.13). Owner role:
Platform engineering. Report trace: §22.2 CI gate ("migration tests and schema
compatibility"); §10.5; §21.2 ("schema migrations with compatibility
windows"). Inputs: P10.01 scope freeze (`01_release_scope_freeze.md`); P10.02
contract freeze (`02_api_contract_freeze.md`, cross-referenced for ledger step
4); the actual migration set
`crates/adapters/postgres/migrations/` (0001..0018, enumerated below); the
recorded P1.13 harness decision (`tests/integration.rs` header: CI PostgreSQL
18 service, nothing installed here); P10.11 backup/restore (the
restore-from-backup consequence of any data-bearing rollback — its re-run
record is `11_backup_restore_rc.md`, EMPTY); P10.15 (the schema-level gate
ordering this suite pins is the persistence half of its bypass analysis).
Deliverable: `crates/adapters/postgres/tests/migration_compat_p10_13.rs`
(additive; no `src` logic touched; this record).

## 1. Honest status — the plain statement

**The suite is WRITTEN, never executed; nothing was installed; no PostgreSQL
server exists in this loop; no migration was ever applied or replayed in this
wave.** Per ENTRY_NOTE.md boundary 3 this is "implemented, awaiting human
build/test". **Zero observed failures are NOT claimed** — an unrun suite has
no observed failures and no observed passes. The compatibility windows in §4
are RECORDED POSITIONS derived from the migration files' own decision
comments (each file's header records its own additive/destructive character);
they are not measured results. The Postgres version floor (§5) is the
repository's recorded harness pin — **UNVERIFIED here: nothing was installed,
and no other PostgreSQL major version was ever tested against this migration
set.** The ledger's acceptance criteria are NOT met and NOT claimed:
forward migration on production-shaped data has NOT run (a), the rollback
window has NOT run (c is recorded, not proven).

| Field | Value |
|---|---|
| Release-candidate identifier | `<PENDING P10.18 — no candidate build exists>` |
| Suite result | `<EMPTY — WRITTEN-UNRUN; the human run fills this>` |
| Database the suite targets | PostgreSQL 18 (the recorded CI service; UNVERIFIED pin — nothing installed) |
| Migrations in the v1.0.0 candidate | 18 (0001..0018, up+down pairs — see §4) |

## 2. The framework's semantics (what "idempotent" and "drift-detected" mean here)

The migration runner is the embedded `sqlx::migrate::Migrator`
(`crates/adapters/postgres/src/store.rs`, `run_migrations`,
`sqlx::migrate!("migrations/")`), which fixes the semantics this record
uses — cited, not re-specified:

- **Forward apply, in version order, one pass**: applied migrations are
  recorded in `_sqlx_migrations`; a run applies only the versions above the
  highest applied one. There is **no undo in the runtime path** — the Rust
  `Migrator` applies UP migrations only; the recorded `*.down.sql` files are
  dev/CI replay artifacts, exercised by `tests/integration.rs`'s
  up/down/up test on an EMPTY database.
- **Idempotency** = skip-by-version: re-running over a fully migrated
  database is a successful no-op (bookkeeping rows neither re-executed nor
  duplicated). The suite pins this behaviourally (test 2).
- **Drift detection** = checksum validation: an applied migration whose
  recorded checksum no longer matches the deployed binary's embedded copy is
  refused (the sqlx `VersionMismatch` class; the adapter maps it to
  `E_STORE_MIGRATION` with the message "an applied migration no longer
  matches the deployed binary" — `store.rs::migration_failure`). The suite
  pins this behaviourally (test 2, tamper-then-restore).
- **Per-migration transactions**: the framework wraps each migration unless
  the file opts out; none of 0001..0018 opts out (verified by inspection —
  no `no-transaction` marker in any up file).

## 3. The test suite (ledger steps 2–3, mapped)

`crates/adapters/postgres/tests/migration_compat_p10_13.rs` — four tests,
WRITTEN-UNRUN, gated on `FLEXCORE_TEST_DATABASE_URL` (skip cleanly when
unset; CI provides the PostgreSQL 18 service):

| # | Test | Covers |
|---|---|---|
| T-1 | `the_migration_set_is_exactly_18_paired_and_ordered` (no DB) | step 1's enumeration: exactly versions 1..=18, ascending, descriptions pinned, up/down pairing (a missing down file is a compile error via `include_str!`) |
| T-2 | `forward_apply_is_ordered_idempotent_and_drift_detecting` | full forward apply from empty; bookkeeping = exactly 1..=18, none failed; idempotent re-run; checksum-drift refusal (step 2's "verify constraints" at the framework layer) |
| T-3 | `forward_migration_over_pilot_shaped_data_preserves_integrity` | step 2's production-shaped requirement: stage 0001..0008, seed a registry chain THROUGH THE REPOSITORIES + an audit row through the INSERT grant, apply 0009..0018 over the populated schema; the data survives (audit row unchanged, 0009's `ADD COLUMN ... DEFAULT` backfills), 0012's PK extension still enforces the duplicate classification, and a versioned correction appends without mutating the original row |
| T-4 | `append_only_and_gate_ordering_invariants_hold_across_the_chain` | step 2's "verify the application works" at the invariant layer: 0008's trigger refuses owner UPDATE/DELETE; the no-DELETE/no-UPDATE grant matrix on every append-only surface of the chain; 0016's column-level UPDATE exception is exactly (state, superseded_by_command_id); 0018's one-ACTIVE-term index refuses a second ACTIVE term and admits rotation; 0016's defence-in-depth refuses PROPOSED→ISSUED and PROPOSED→APPROVED (no command reaches ISSUED without APPROVED — schema level), admits APPROVED→ISSUED only with journal evidence, and demands the gate name on gate-class journal rows |

Known-risk note carried in the suite header (recorded, not assumed): test
T-3's stage-1 bookkeeping inserts name the sqlx 0.9 `_sqlx_migrations`
columns and reuse the embedded migrator's own checksums; if 0.9's bookkeeping
schema differs, the failure is a loud first-statement error in T-3 — a pin to
fix at the first human run, never a silent drift.

## 4. The migration set and its compatibility windows (ledger step 1 — RECORDED POSITIONS)

The window semantics (report §21.2): for migration N, "prior versions can run
against the new schema" = the PREVIOUS application version still works on the
schema AFTER N is applied (the rolling-upgrade window P10.12 §3.1 consumes);
"and vice versa" = the post-N application needs the post-N schema (a
migration that only adds surfaces keeps the window; one that reshapes what
the old binary reads closes it). These are positions from each file's own
decision comments — **unverified by any run**.

| Migration | Surface (from the file's own header) | Character | Window position (prior app on new schema) |
|---|---|---|---|
| 0001 | quality enum + guarded service roles | new type + roles (DO blocks, idempotent) | No prior schema: first migration; window starts here |
| 0002 | tenants | new table | OK (nothing reads it before) |
| 0003 | portfolios | new table + RLS | OK |
| 0004 | sites | new table + composite FK | OK |
| 0005 | assets | new table + composite FK | OK |
| 0006 | asset_state | new table | OK |
| 0007 | outbox | new table | OK |
| 0008 | audit_records | new table, append-only (grants + trigger) | OK |
| 0009 | registry lifecycle extension | ADD COLUMN with DEFAULT — additive | OK (new columns backfilled; old binary unaware) |
| 0010 | telemetry + quarantine + ingest/query roles | new tables + roles | OK |
| 0011 | telemetry read grant for flexcore_app | one GRANT — additive | OK |
| 0012 | correction versions | EXTENDS telemetry: 5 columns, 2 CHECKs, PK rebuilt (4-leg → 5-leg, name kept) | OK by position: old writes are version-1 rows under the default; the old binary's inserts still satisfy the extended PK. The window CLOSES in the other direction: the post-0012 binary cannot run on a pre-0012 schema (columns missing) |
| 0013 | schedule_results | new table | OK |
| 0014 | schedule_revisions + comparisons + 0014 UPDATE grant | new tables + column-level grant | OK |
| 0015 | flexibility_envelopes | new table | OK |
| 0016 | dispatch_commands + transitions | new tables + triggers + column-level grant | OK |
| 0017 | feasibility_check_records | new table, append-only grants | OK |
| 0018 | edge_device_registry + journal | new tables + append-only triggers + partial unique index | OK |

Window summary: every migration's up path keeps the previous application
version working on the migrated schema (the rolling-upgrade window holds by
position — every change is additive or default-backfilled); the reverse
direction closes at 0012 (post-0012 binary needs the post-0012 schema). No
migration renames or drops an existing column or table; the only destructive
steps in the whole set are 0012's PK drop/re-add (same constraint name
retained on purpose — the P2.09 duplicate classification matches it by name)
and 0012's down (see §6).

## 5. The Postgres version target (UNVERIFIED pins — recorded honestly)

| Item | Status |
|---|---|
| Target server | PostgreSQL 18 — the recorded harness decision (`tests/integration.rs` header; the Compose stack and CI service provide `postgres:18`) |
| Floor reason recorded from the migration files | 0002/0007/0008 default identifiers with `uuidv7()`; the repo's recorded floor is the 18 pin and the tests' own headers ("PostgreSQL 18"); the exact server version that introduced `uuidv7()` is **UNVERIFIED here** — nothing was installed in this loop |
| Tested against 14 / 15 / 16 / 17 | **NEVER TESTED — no compatibility matrix below 18 exists and none is claimed** |
| Target position | UNVERIFIED pin (the recorded P1.13 harness decision carried forward); the first human run confirms the server it runs against, and any lower major is an untested cell marked untested (never assumed) |

## 6. Downgrade and irreversibility (ledger step 5 — recorded, not hidden)

- **Downgrade is NOT supported by the runtime path.** The embedded migrator
  applies UP only; there is no undo and no operational down-migration.
  Consequence: a rollback over a data-bearing database is **restore-from-
  backup (P10.11)** — which is exactly why P10.11's RC restore must be
  tested first (`11_backup_restore_rc.md`, run record EMPTY). The down files
  exist as dev/CI replay artifacts (`tests/integration.rs` up/down/up on an
  empty database) and are never an operational rollback tool.
- **The irreversibility register** (from the down files' own recorded
  reasons):
  - **0001 (partial):** the down drops the enum only; the service roles are
    deliberately NOT dropped (role lifecycle belongs to the deployment
    bootstrap; a schema rollback never removes security principals other
    databases may reference). Recorded in `0001_quality_enum.down.sql` itself.
  - **0012 down = DATA LOSS, recorded in the file itself:** versioned
    corrections (`telemetry_version >= 2`) are irreversibly removed; the
    original rows are untouched. Even that down is not reachable from the
    runtime path (see above).
  - All other down files are recorded as exact reversals of their up files
    (their headers say so); no other irreversible step is recorded anywhere
    in the set.

## 7. The run record — ALL CELLS EMPTY (UNRUN)

| # | Test | Database / server version | Result | Evidence | Pass/fail |
|---|---|---|---|---|---|
| T-1 | (EMPTY — UNRUN) | n/a (no DB) | | | |
| T-2 | (EMPTY — UNRUN) | `<EMPTY>` | | | |
| T-3 | (EMPTY — UNRUN) | `<EMPTY>` | | | |
| T-4 | (EMPTY — UNRUN) | `<EMPTY>` | | | |

Ledger step 4 (Buf/contract breaking-change record): the migration set is
storage-only — it changes no Protobuf file, no REST resource, and no NATS
event subject; the contract surfaces are P10.02's freeze
(`02_api_contract_freeze.md`), whose drift-detection procedure owns the
breaking-change record for those surfaces. Cross-reference recorded; no
Protobuf/REST/NATS change is claimed or denied by THIS record.

Ledger step 3 (rollback window): recorded as the §6 position — the runtime
path has no down; the prior-application-version window (§4) is the declared
window and is UNRUN. Per P10.12 §3.1 rule 1, an irreversible-migration
rollback takes the restore-from-backup path only.

## 8. Reviewer-verification procedure (human)

1. Confirm §1's plain statement and that EVERY §7 cell is EMPTY/UNRUN and
   every §4/§5 window/version value is labelled a recorded position, not a
   result.
2. Trace the enumeration: `ls crates/adapters/postgres/migrations/` shows
   exactly the 18 up/down pairs of §4; `grep -n "sqlx::migrate" crates/
   adapters/postgres/src/store.rs` shows the embedded runner and its
   up-only surface; `grep -rn "no-transaction" migrations/` returns nothing.
3. Trace the irreversibility register: the 0001 and 0012 down-file headers
   record the roles-only and DATA-LOSS statements verbatim.
4. Confirm the suite is additive (a new file under `tests/` only; no `src`
   change) and mirrors the recorded harness patterns of
   `tests/integration.rs` / `tests/corrections.rs` / `tests/command_store.rs`
   (gating, temp databases, role bootstrap, partition creation, tenant GUC).
5. Confirm §3's T-4 gate-ordering assertions match 0016's encoded frozen
   edge table (PROPOSED→ISSUED and PROPOSED→APPROVED absent; APPROVED→ISSUED
   present; the journal evidence/gate CHECKs) — the schema-level half of
   P10.15's bypass analysis, by citation.
6. Confirm the ledger tick wording is "implemented, awaiting human
   build/test" — the suite is written; nothing was built, run, installed, or
   migrated.

## 9. Filling commands (for the human executor)

```sh
# (a) the documented-tools gate (run after ANY edit to this record):
python tools/verify_docs.py
python tools/validate_agent.py
python -I tools/check_dependency_rule.py

# (b) the WRITTEN-UNRUN suite (human build/test gate; requires the CI-shaped
#     PostgreSQL 18 service and FLEXCORE_TEST_DATABASE_URL set — nothing
#     was installed or run in this loop):
cargo test -p flexcore-postgres-adapter --test migration_compat_p10_13
#     the existing suites that share the seam:
cargo test -p flexcore-postgres-adapter --test integration
cargo test -p flexcore-postgres-adapter --test corrections
cargo test -p flexcore-postgres-adapter --test command_store

# (c) Fill §7 FROM THE RUN ONLY. A test that cannot execute is recorded with
#     its blocker, never as a pass and never silently dropped.
# (d) Confirm the server version actually used and resolve the §5 UNVERIFIED
#     pin against it; mark any lower-major cell untested, not assumed.
# (e) Hand the windows + irreversibility register to P10.12 (rollback path)
#     and the restore-from-backup dependency to P10.11; hand the T-4 results
#     to P10.15 (schema-level bypass evidence).
```

## 10. Findings (recorded, not fixed)

- **F-P10.13-1 (framework position):** downgrade is not part of the runtime
  path at all — the operational rollback story for a data-bearing database
  is restore-from-backup, and P10.11's restore re-run is EMPTY. The
  rollback leg of this subtask's ledger step 3 is therefore a dependency,
  not a result.
- **F-P10.13-2 (recorded asymmetry):** 0016's
  `dispatch_command_transitions` is immutability-by-GRANT only (no
  defence-in-depth trigger, unlike 0008/0018). The owner role can UPDATE or
  DELETE journal rows. Recorded honestly; the T-4 assertions pin the
  grant-layer behaviour and do not overclaim a trigger. Extending 0016 is a
  reviewed migration change outside this subtask's ownership.
- **F-P10.13-3 (version pin):** the PostgreSQL 18 target is an UNVERIFIED
  pin carried from the P1.13 harness decision; no server exists in this
  loop, no lower major has ever been tested, and the `uuidv7()` floor is
  recorded from the migration files' own usage, not from an installed
  server.
- **F-P10.13-4 (bookkeeping pin):** T-3's stage-1 bookkeeping insert names
  the sqlx 0.9 `_sqlx_migrations` shape; a 0.9 schema drift fails loudly at
  the first run and is a pin to fix then (suite header carries the same
  note).
- **F-P10.13-5 (concurrent sibling):** P10.12 (`12_upgrade_rollback_rc.md`)
  cites this record by the expected path and consumes §4's windows and §6's
  restore-from-backup rule; P10.11's restore re-run must be filled before
  the restore path is credible (P10.12 §3.1 rule 1's own statement).

## 11. Non-claims

No migration was applied, replayed, or rolled back in this loop; no
PostgreSQL server was installed or contacted; no compatibility window was
verified by execution; no failure or pass was observed from the unrun suite;
the PostgreSQL 18 position is an unverified pin, not a tested fact. Nothing
here establishes schema-compatibility across server majors, upgrade
readiness, or the rollback window — those are the human gates and have not
happened and are not scheduled. The entire evidence class is
simulation/lab-grade (AGENTS.md rules 2–5; ENTRY_NOTE boundaries 1 and 3).

Status: **WRITTEN-UNRUN** — a compatibility test cannot "run" by being
written; the server, the candidate, the pilot-shaped data, and the
executions are the human gates and have not happened and are not scheduled.
