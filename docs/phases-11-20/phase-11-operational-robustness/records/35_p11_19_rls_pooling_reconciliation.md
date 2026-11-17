# P11.19 — RLS and connection-pooling reconciliation

| Field | Value |
|---|---|
| Subtask | P11.19 — transaction-scoped `SET LOCAL` tenant context verified everywhere; ALL session-level `SET` usage in the postgres adapter audited (systematic grep); the PgBouncer transaction-mode compatibility plan written, not deployed [S-69][S-70] |
| Wave / agent | Phase 11 wave 2, Agent A |
| Status | The audit is a STATIC GREP + CODE-REVIEW artifact (patterns enumerated below; no database was run). The audit found NO defect requiring a code change, so NO production code was modified by this subtask (P11.15's keyed-chain work in the same adapter files follows the identical transaction-scoped discipline). The PgBouncer plan is WRITTEN, NOT DEPLOYED. |
| Sources | [S-69] (RLS leak across pooled connections — the transaction-scoping hazard), [S-70] (AWS multi-tenant RLS guide — the SET-local/current_setting pattern); gap analysis §5 Tier 0 row 9 |
| Files touched | none for code (audit clean); this record is the deliverable, plus record 31's chain code which reuses the audited pattern |

## 1. The verified pattern

Tenant context is set EXACTLY once per transaction, transaction-scoped,
inside an already-open transaction:

```sql
SELECT set_config('flexcore.tenant_id', $1, true)   -- is_local = true ⇒ SET LOCAL semantics
```

Under transaction pooling, `is_local = true` scoping is the load-bearing
property [S-69][S-70]: the setting dies with the transaction, so a pooled
connection handed to the next client carries NO residual tenant context.

## 2. The systematic audit (grep evidence, 2026-11-14)

Sweep: `set_config`, `SET LOCAL`, `current_setting`, `reset_all`,
`after_connect`, `on_acquire`, `before_acquire`, `test_before_acquire`,
`PREPARE`/`DEALLOCATE`/`LISTEN`/session-replication patterns across
`crates/adapters/postgres/src/**`, `crates/adapters/postgres/tests/**`,
`apps/**`, `python/**`, `deploy/**`.

| Location | Finding | Verdict |
|---|---|---|
| `src/store.rs` (2 sites: `begin`, the store-level audit sink) | `set_config(..., true)` inside an open transaction, immediately after `pool.begin()` | Correct — the canonical pattern |
| `src/lib.rs` | Documentation of the same pattern (no executable SQL) | Correct |
| `src/audit.rs` (P11.15 additions) | `verify_tenant_audit_chain`'s store method sets the same GUC in its own transaction | Correct (new code follows the pattern) |
| `src/unit_of_work.rs` and all other `src/*.rs` | NO tenant-context SQL at all — every statement runs inside the unit of work whose transaction already carries the GUC | Correct |
| RLS policies in migrations 0003–0018 | `current_setting('flexcore.tenant_id', true)` with `missing_ok = true` and `NULLIF(..., '')` — an UNSET context evaluates to NULL ⇒ the policy matches nothing ⇒ FAIL CLOSED | Correct — unset context can never widen access |
| Tests (`crates/adapters/postgres/tests/*`, `apps/worker/tests/*`) | All `set_config(..., true)` calls are inside their own transactions | Correct |
| Pool hooks (`after_connect`/`on_acquire`/`before_acquire`) | ZERO occurrences anywhere | Correct — no session-level state is planted at connection acquisition |
| Session-level `SET` statements | ZERO occurrences in src, migrations, apps, python, deploy | Correct |
| `partition_maintenance.rs` | Runs platform-level audit rows through the owner/ops (maintenance) path per its D-R5 decision; DDL (`DETACH`) in-transaction; no session SET | Correct — the recorded owner-path exception (P1.12 §5.2) |

**Audit verdict: no code fix required.** Every tenant-context write is
transaction-scoped; every read of the context fails closed when unset.

## 3. The PgBouncer transaction-mode compatibility plan (WRITTEN, NOT DEPLOYED)

This plan is a written artifact for the phase that deploys a pooler
(Phase 12/20 territory — no pooler is added here):

1. **Tenant context: compatible as-is.** `set_config(..., true)` inside
   the same transaction as the statements it scopes survives transaction
   pooling (the audit in §2 is the precondition). The ONE deployment rule:
   never set the GUC outside a transaction — the audited code never does.
2. **Prepared statements: the real hazard (ASSUMPTION, UNVERIFIED).** sqlx
   caches server-side prepared statements per connection; under
   transaction-mode pooling, server connections are shared between clients
   between transactions, and pooling layers historically could not track
   named prepared statements (PgBouncer gained prepared-statement support
   in 1.21 — UNVERIFIED external claim, verify at deployment). Mitigation
   options, in order: (a) deploy PgBouncer ≥ 1.21 with
   `max_prepared_statements` enabled and verify against THIS adapter's
   runtime-query shape; (b) configure the sqlx pool's statement-cache
   capacity to 0 (the exact option name is ASSUMPTION — verified at the
   implementing wave's first build). The adapter uses runtime `sqlx::query`
   only (no compile-time macros — the P1.13 recorded decision), which
   keeps both options available.
3. **Session features: none in use.** The audit found no `LISTEN`,
   advisory-lock, session-variable, `DISCARD`-reliant, or
   temp-table-reliant code in the adapter — nothing that assumes one
   client per server connection.
4. **Transaction semantics: preserved.** Units of work are single native
   transactions (`pool.begin()` … commit/rollback); row locks (`FOR
   UPDATE` — the P11.15 chain head state) live inside that transaction and
   survive transaction pooling by definition.
5. **Migration runner: excluded from pooling.** `run_migrations` and the
   partition-maintenance DDL path connect DIRECTLY (migrator/maintenance
   role, P1.12 §3); they must keep bypassing the pooler — DDL-in-transaction
   + a pooled multiplexer is not a tested combination, and the maintenance
   path relies on the owner-role exception.
6. **Deployment gate**: when a pooler is introduced, its config lands in
   the deployment tree with a decision record; the compatibility claims
   above are then re-verified against a real run (this plan makes no
   runtime-behavior claim).

## 4. ASSUMPTION register

1. Every verdict in §2 is static grep + code-review evidence; a real
   pooled run can surface timing/concurrency behaviors a static audit
   cannot (that is the deployment gate's job).
2. The PgBouncer 1.21 prepared-statement capability and the sqlx
   statement-cache option name are UNVERIFIED external claims, marked as
   such in §3.2.
