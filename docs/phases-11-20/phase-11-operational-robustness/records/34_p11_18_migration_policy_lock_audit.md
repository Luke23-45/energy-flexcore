# P11.18 — Expand/contract migration policy + lock-risk audit

| Field | Value |
|---|---|
| Subtask | P11.18 — the expand/contract migration policy document [S-71] + a lock-risk audit of migrations 0002–0018 against the lifecycle tables (0008_audit_records, 0016_commands_p4_10, 0017_feasibility_checks_p4_30), findings recorded, remediation scheduled |
| Wave / agent | Phase 11 wave 2, Agent A |
| Status | The policy is WRITTEN and binding. The audit is a STATIC CODE-REVIEW ARTIFACT: no migration was executed, no lock was observed, and no finding below is a claim about observed database behavior. Lock-level statements follow the PostgreSQL-documented lock model and are ASSUMPTION until a human observes a real run. |
| Sources | [S-71] (pgroll / zero-downtime migrations: additive change → dual-write/backfill → switch → contract removal; encore.dev zero-downtime article — the practice shape, no tool adopted); PostgreSQL-documented lock behavior (ASSUMPTION per the honesty rules); gap analysis §5 Tier 0 row 10 |
| Files touched | `crates/adapters/postgres/MIGRATION_POLICY.md` (new — the policy document, placed OUTSIDE `migrations/` because sqlx `migrate!` treats that directory's contents as versioned SQL files) |

## 1. The policy (summary; the binding text is MIGRATION_POLICY.md)

Four stages for every change to a populated table — expand (additive
migration) → dual-write/backfill (application code or bounded ops runbook,
never one giant migration UPDATE) → switch (adapter code moves to the new
shape) → contract (removal in a later migration) — and never an in-place
destructive ALTER. Per-statement rules: `ADD COLUMN NULL` safe;
`NOT NULL DEFAULT <constant>` version-gated safe; volatile-default ADD
COLUMN, non-concurrent `CREATE INDEX` on populated tables, validated
constraint adds, and PK swaps are PROHIBITED inside the transactional
embedded runner and must be scheduled as `CONCURRENTLY` / `NOT VALID` +
`VALIDATE` ops-runbook steps with the decision record naming the runbook.
The lifecycle (append-only evidence) tables are the strictest case: the
four-stage rule is mandatory, and stage 4 on them additionally requires
operator sign-off (evidence removal is never a code-only decision).

## 2. The audit (static review of migrations 0001–0019)

Context that bounds every finding: the embedded runner applies ALL
migrations to an EMPTY database at first start (no lock risk there); the
risk window is a NEW migration applied to an ALREADY-POPULATED database on
upgrade — exactly the case the policy governs.

| Migration | Statements on populated tables (at their application time) | Static finding | Remediation (scheduled) |
|---|---|---|---|
| 0001 | `CREATE TYPE telemetry_quality` | Safe (new type; its header records the ALTER TYPE evolution rule) | none |
| 0002–0007 | `CREATE TABLE` (+ indexes on the new tables, incl. outbox 0007's partial indexes) | Safe — everything is new and empty | none |
| 0008 (lifecycle) | Two non-concurrent `CREATE INDEX` on the NEW `audit_records` | Safe at creation; FUTURE index additions to the populated table must follow the policy's CONCURRENTLY runbook rule | Runbook rule applies prospectively |
| 0009 | `ADD COLUMN lifecycle_status ... NOT NULL DEFAULT 'active'` on populated `assets` (catalog-only per PostgreSQL 11+ — ASSUMPTION); `ADD CONSTRAINT assets_lifecycle_consistency CHECK` (validated) | MEDIUM: the validated CHECK holds `ACCESS EXCLUSIVE` for a full-table scan — reads AND writes blocked for the scan duration | Policy §2 (NOT VALID + VALIDATE) governs future changes; no retroactive remediation (single-host dev-era volumes at the time; recorded, not excused) |
| 0010 | `CREATE TYPE` ×2, `CREATE TABLE telemetry` + 3 non-concurrent indexes on the new table, quarantine table | Safe — new objects only | none |
| 0011 | Read grant only | Safe | none |
| 0012 | `ADD COLUMN telemetry_version ... NOT NULL DEFAULT 1` (catalog-only — ASSUMPTION); validated CHECK constraints ×2 (full scans); **`DROP CONSTRAINT telemetry_pkey` + composite re-add on populated `telemetry`** | HIGH — the most significant lock-risk migration in the set: the PK swap takes `ACCESS EXCLUSIVE`, scans for uniqueness, and rewrites dependent indexes; a long-running query blocks it and it blocks everything behind it | Policy: future PK changes only via the four-stage sequence (e.g., unique INDEX built CONCURRENTLY in a runbook, then swap); recorded as the worked example of the prohibition |
| 0013 | `CREATE TABLE schedule_results` | Safe | none |
| 0014 | `ADD CONSTRAINT schedule_results_candidate_tenant_key` (unique) on populated `schedule_results` — `ACCESS EXCLUSIVE` + scan; unique index on the NEW `schedule_revisions` | MEDIUM: the unique-constraint add on the populated results table has the 0009-class scan cost; the revision index is safe (new table) | Same NOT VALID/VALIDATE-where-applicable + runbook rule for future unique constraints |
| 0015 | `CREATE TABLE flexibility_envelopes` | Safe | none |
| 0016 (lifecycle) | `CREATE TABLE dispatch_commands` / `dispatch_command_transitions` + indexes on the new tables; column-level UPDATE grant | Safe — new objects only; the table-level grants are not DDL | Prospective policy applies |
| 0017 (lifecycle) | `CREATE TABLE feasibility_check_records` + 2 indexes on the new table | Safe | Prospective policy applies |
| 0018 (lifecycle) | Two `CREATE TABLE`s + indexes on the new tables | Safe | Prospective policy applies |
| 0019 (this wave, for completeness) | 4 nullable `ADD COLUMN`s on `audit_records` (catalog-only) + new `audit_chain_state` table; NO index, NO constraint on a populated table | Safe by construction — written to comply with this policy before the policy text landed | The planned `(tenant_id, chain_sequence)` index is SCHEDULED as a `CREATE INDEX CONCURRENTLY` runbook step (it cannot run inside the transactional embedded runner), named in record 31 |

## 3. Remediation schedule (all human-gated, nothing retroactive)

1. **No historical migration is rewritten** — rewriting applied migrations
   breaks the embedded runner's checksum discipline (P1.13). The findings
   stand as recorded history.
2. **Scheduled runbook items** (created when the trigger fires, per the
   policy): the 0019 `(tenant_id, chain_sequence)` index;
   `VALIDATE CONSTRAINT` splits for any future constraint on the
   lifecycle tables; CONCURRENTLY index builds for any future index on
   `audit_records`, `dispatch_commands`, `dispatch_command_transitions`,
   or `feasibility_check_records`.
3. **Trigger**: the first deployment that applies a NEW migration to a
   populated database is the human gate that exercises this policy end to
   end; that run is recorded UNRUN.

## 4. ASSUMPTION register

1. All lock-level statements (SHARE vs ACCESS EXCLUSIVE vs SHARE UPDATE
   EXCLUSIVE, catalog-only ADD COLUMN on PostgreSQL 11+, VALIDATE's lock
   level) follow the PostgreSQL-documented lock model — never observed in
   this program (no database was run).
2. The audit is a code review of the migration SQL text; a real upgrade
   run on a populated database can surface behaviors this static review
   cannot (lock queueing behind long-running transactions, statement
   timeouts) — those are the human run's findings, not this record's.
