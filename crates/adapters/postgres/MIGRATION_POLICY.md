# PostgreSQL Migration Policy — expand/contract and lock-risk rules

P11.18 (Phase 11 — Operational Robustness and Observability). Status:
WRITTEN policy, binding for future changes to
`crates/adapters/postgres/migrations/`. Sources: [S-71] (the pgroll /
zero-downtime migration practice: additive change → dual-write/backfill →
switch → contract removal, never an in-place destructive ALTER); the
lock-behavior statements below follow the PostgreSQL ALTER TABLE / CREATE
INDEX documented lock levels and are marked ASSUMPTION where a specific
release's behavior matters (behavior was never observed in this program —
everything here is written, never run).

## 1. The four-stage rule (never in-place destructive change)

Every schema change to a populated table goes through the expand/contract
sequence [S-71]. An in-place destructive `ALTER` (drop/rename column,
narrow a type, change a constraint in place) on a table with live readers
or writers is PROHIBITED in every release line of this repository:

| Stage | What happens | Where it lives |
|---|---|---|
| 1. Expand | The new shape is ADDED alongside the old: nullable columns, a new table, a new constraint applied `NOT VALID` | A numbered migration (up + down) |
| 2. Dual-write / backfill | Writers populate both shapes; existing rows are backfilled in bounded batches by application code or a scheduled ops runbook — never by one giant `UPDATE` inside a migration | Application code / ops runbook |
| 3. Switch | Reads and writes move to the new shape behind the adapter's own code; the old shape is written by nothing | Application code change |
| 4. Contract | The old shape is removed only after stage 3 is complete and verified | A later numbered migration |

Stage boundaries are recorded in the decision record of the subtask that
performs them; a migration that skips a stage is a policy violation, not
a shortcut.

## 2. Lock-risk rules (per statement shape)

The embedded runner (`FlexCoreStore::run_migrations`, sqlx `migrate!`)
executes each migration INSIDE a transaction. Consequences, all ASSUMPTION
per the PostgreSQL-documented lock behavior until a human observes a real
run:

| Statement | Lock taken | Policy |
|---|---|---|
| `CREATE TABLE` | none on existing tables | Always safe |
| `ADD COLUMN ... NULL` | brief `ACCESS EXCLUSIVE` (catalog-only, no rewrite) | Safe as an expand step |
| `ADD COLUMN ... NOT NULL DEFAULT <constant>` | catalog-only on PostgreSQL 11+ | Allowed; version-gated ASSUMPTION |
| `ADD COLUMN ... NOT NULL DEFAULT <volatile>` | full table rewrite | PROHIBITED on populated tables |
| `CREATE INDEX` (non-concurrent) | `SHARE` — blocks all writes for the build | PROHIBITED on populated lifecycle tables inside the embedded runner; required as `CREATE INDEX CONCURRENTLY` — which CANNOT run inside the transactional embedded runner, so it is scheduled as an ops-runbook step with the decision record naming the runbook |
| `ADD CONSTRAINT ... CHECK` (validated) | `ACCESS EXCLUSIVE` for the full validation scan | Use `NOT VALID` in the migration + `VALIDATE CONSTRAINT` later (validation takes only `SHARE UPDATE EXCLUSIVE` — ASSUMPTION per the documented lock table) |
| `ADD CONSTRAINT ... FOREIGN KEY` (validated) | row-lock scan of the referencing table | Same NOT VALID/VALIDATE split for populated tables |
| `DROP CONSTRAINT` / `DROP COLUMN` | `ACCESS EXCLUSIVE` | Contract stage only, after stage 3 is verified |
| PK swap (`DROP CONSTRAINT telemetry_pkey` + re-add) | `ACCESS EXCLUSIVE` + full uniqueness scan + rewrite of dependent indexes | PROHIBITED as an in-place step; only via the four-stage sequence |
| `ALTER TYPE ... ADD VALUE` | safe only outside a prepared/locked transaction block | Enum additions follow their own recorded rule (0001's header note) |

## 3. Lifecycle tables (the protected set)

The append-only evidence tables are the strictest case — their history is
the audit trail, so the four-stage rule is mandatory and stage 4 on them
additionally requires an operator sign-off (evidence removal is never a
code-only decision):

- `audit_records` (0008) — plus its P11.15 chain columns (0019)
- `dispatch_commands`, `dispatch_command_transitions` (0016)
- `feasibility_check_records` (0017)
- `edge_device_registry`, `edge_enrollment_journal` (0018)
- `schedule_results`, `schedule_revisions` (0013/0014)
- `telemetry` + corrections columns (0010/0012), `outbox` (0007)

Changes to these tables in a later phase cite this policy in their
decision record and schedule any `CONCURRENTLY`/`VALIDATE` steps as
runbook items.

## 4. The P11.18 lock-risk audit

The audit of migrations 0002–0018 against this policy — per-migration
findings and the scheduled remediations — is recorded in the P11.18
decision record:
`docs/phases-11-20/phase-11-operational-robustness/records/34_p11_18_migration_policy_lock_audit.md`.
It is a STATIC code-review artifact: no migration was executed, no lock
was observed, and no finding below is a claim about observed database
behavior.
