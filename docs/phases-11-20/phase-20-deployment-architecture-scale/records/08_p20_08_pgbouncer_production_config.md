<!-- P20.08 — PgBouncer production config design: transaction-mode pooling
     config DESIGN; the SET LOCAL RLS contract consumed from P11.19 (cited,
     never restated); pool sizing per service; prepared-statement
     compatibility validation RESULTS gated on a REAL deployment
     [S-69][S-70]. Consumes P12.06 by citation (context constraint 4). -->
# P20.08 — PgBouncer Production Config Design (consumes P12.06/P11.19 — nothing deployed, no validation result)

| Field | Value |
|---|---|
| Subtask | P20.08 — PgBouncer production config design (`ledger_phase_20.md` §2, row P20.08) |
| Wave / agent | Phase 20 wave 1, Agent A |
| Status | **DESIGN RECORD — WRITTEN, NOTHING DEPLOYED, NO VALIDATION RUN.** No PgBouncer instance exists, no config file exists in any deployment tree, no prepared-statement compatibility result exists. Pool sizes are DESIGN allocations, not measurements. |
| Sources | [S-69] (RLS leak across pooled connections — the transaction-scoping hazard); [S-70] (AWS multi-tenant RLS guide — the SET-local/current_setting pattern); **P11.19 record** `docs/phases-11-20/phase-11-operational-robustness/records/35_p11_19_rls_pooling_reconciliation.md` (§1 the verified `set_config(..., true)` pattern; §3 the compatibility plan — DEFINED THERE, cited here, never restated); **P12.06 record** `docs/phases-11-20/phase-12-backup-dr-resilience/records/06_p12_06_pgbouncer_introduction.md` (§2 transaction-mode design, §3 the V-1..V-5 validation matrix — consumed); `crates/adapters/postgres/src/unit_of_work.rs` (the transaction-scope seam — one unit of work = one native transaction that already carries the tenant GUC); P12.05 §3.2 (the `SET LOCAL synchronous_commit` interaction, consumed) |
| Files touched | this record only |

## 1. Honest status — the plain statement

No pooler is deployed and none is scheduled: the current shape is
single-host Compose (never brought up), where pooling is not the binding
constraint. This record is the PRODUCTION-CONFIG DESIGN that P12.06's
introduction design calls for when a real deployment materializes — it
adds the production sizing, topology, and gate details on top of the
consumed designs, and it gates every compatibility RESULT on a real
deployment (the transaction mode + SQLx prepared-statement pair is
exactly the class of behavior that must be MEASURED on the deployed pair,
never asserted — plan §7 R-P20-4).

## 2. What is consumed (citations, never restated)

| Consumed record | Element | Status |
|---|---|---|
| P11.19 §1 | The verified pattern: tenant context set EXACTLY once per transaction via `set_config('flexcore.tenant_id', $1, true)` inside the open transaction — `is_local = true` gives SET LOCAL semantics, the load-bearing property under transaction pooling [S-69][S-70] | CITED — this record contains no restatement of the SQL pattern and adds no rule to it |
| P11.19 §2 | The static-audit verdict (no session-level state anywhere in the adapter) | CITED — the precondition; V-1 (P12.06) re-runs the sweep at the gate rather than assuming it holds |
| P11.19 §3.5 | Migration runner and partition-maintenance DDL connect DIRECTLY, bypassing the pooler | CITED — inherited unchanged; §3 row P-5 below enforces it in the production shape |
| P11.19 §3.6 | The deployment gate: pooler config lands in the deployment tree WITH ITS OWN DECISION RECORD, only at the gate | CITED — the HARD GATE this record operates inside |
| P12.06 §2 | Transaction-mode-only pooling; no session features in use; the `synchronous_commit` interaction (P12.05 §3.2) | CITED |
| P12.06 §3 | The validation matrix V-1..V-5 (static re-audit; prepared-statement matrix; RLS-under-pooling negative test; lifecycle regression; excluded-path check) | CITED — §4 below adds only the RESULT register, which is EMPTY |

## 3. The production config design (design allocations; nothing deployed)

| # | Element | Production design | Status |
|---|---|---|---|
| P-1 | Pooling mode | `transaction` mode ONLY (P12.06 §2, consumed); session mode is out of design scope | DESIGN |
| P-2 | Pool topology | One PgBouncer instance per service class is NOT the design: ONE pooler tier fronts the app database; the runtime roles (`flexcore_app` per 0001/0010 grant shape) share the tier; migrator/maintenance bypass it entirely (P-5) | DESIGN |
| P-3 | Control-plane pool size | `default_pool_size` design allocation: **20** server connections for the control-plane's role, sized to the accept-path concurrency the P20.02 ladder will measure — the allocation is a STARTING VALUE, re-derived from the run, never a measured result | DESIGN ALLOCATION |
| P-4 | Worker pool size | design allocation: **10** server connections per worker replica (the worker's consumer loop holds one transaction per in-flight command; replicas multiply the client count, not this per-replica allocation) | DESIGN ALLOCATION |
| P-5 | Excluded paths | `run_migrations` and partition-maintenance DDL connect DIRECT to postgres (migrator/maintenance roles, P1.12 §3; P11.19 §3.5 consumed) — enforced in the production shape by separate connection strings, never routed through the pooler | DESIGN (inherited rule) |
| P-6 | Prepared statements | Both P11.19 §3.2 mitigation options stay available: (a) PgBouncer ≥ 1.21 prepared-statement support enabled (capability and option name UNVERIFIED — carried labels), (b) sqlx statement-cache capacity 0 (option name ASSUMPTION — carried). The production config records which was chosen ONLY after V-2 produces a result on the deployed pair | DESIGN; result `<EMPTY — UNRUN>` |
| P-7 | Auth | `auth_type` and userlist content are deployment decisions fed from the secrets manager (P11.16 posture); NO credential material appears in this record or any config committed here | DESIGN |
| P-8 | `synchronous_commit` interaction | The pooler is not relied on to carry any commit-durability state between transactions (P12.06 §2 row, consumed); the P12.05 `SET LOCAL` design dies with its transaction like the tenant GUC | CITED |
| P-9 | Config location | NOT in this repository at design time; at the gate the config lands in the deployment tree with its own decision record (P11.19 §3.6) | HARD GATE |

The sizing allocations (P-3, P-4) are pre-registered STARTING VALUES (X-8)
— written before any run that could measure against them, re-derived after
the P20.02 ladder and the P12.06 V-4 lifecycle regression produce real
concurrency figures.

## 4. Prepared-statement compatibility — the RESULT register (EMPTY)

P12.06 §3 defines the exercises; this register is where their RESULTS are
recorded, and every cell is EMPTY until a real deployment produces them:

| Exercise (P12.06 §3) | Result on the deployed pair | Status |
|---|---|---|
| V-1 static audit re-run | `<EMPTY — UNRUN>` | UNRUN |
| V-2 prepared-statement matrix | `<EMPTY — UNRUN>` | UNRUN |
| V-3 RLS-under-pooling negative test | `<EMPTY — UNRUN>` | UNRUN |
| V-4 lifecycle regression through the pooler | `<EMPTY — UNRUN>` | UNRUN |
| V-5 excluded-path check | `<EMPTY — UNRUN>` | UNRUN |

A static audit or a design argument can NEVER fill these cells
(P11.19 §4.1's position, consumed): "compatible" is a measured outcome on
the deployed pooler + adapter pair, produced by the human executor.

## 5. What this record does NOT establish

1. No pooler exists; no config file is committed anywhere; no runtime
   behavior is claimed about PgBouncer, sqlx, or their interaction.
2. No compatibility verdict: V-1..V-5 results are EMPTY — no "works under
   transaction pooling" claim is made anywhere.
3. No measured sizing: P-3/P-4 are starting allocations to be re-derived
   from a run.
4. No deployment-tree change: the P11.19 §3.6 gate owns that act.
5. No isolation weakening: transaction pooling is DESIGNED AROUND the RLS
   fail-closed contract, never around it — tenant isolation is the reason
   this design is this careful, and no configuration in it widens access.

## 6. Reviewer-verification procedure (human)

1. Confirm §2 cites P11.19/P12.06 element-by-element with no restatement
   (the SET LOCAL pattern appears in this record only as a citation
   pointer).
2. Confirm §4's register is entirely EMPTY/UNRUN.
3. Confirm P-5 keeps migrations/maintenance OFF the pooler and P-7
   carries no credential material.
4. Confirm the P-3/P-4 sizes are labeled design allocations (X-8
   pre-registration), not measurements.
5. Confirm no config file exists in any deployment tree.

## 7. Filling commands (for the human executor)

```sh
# (a) the documented-tools gate (run after ANY edit to this record):
python tools/verify_docs.py
python tools/validate_agent.py
python -I tools/check_dependency_rule.py

# (b) At the deployment gate (P11.19 §3.6): create the pooler with its own
#     decision record, run P12.06 §3's V-1..V-5 on the deployed pair,
#     fill §4 FROM THE RUN ONLY, re-derive P-3/P-4 from the observed
#     concurrency, and record the chosen prepared-statement posture (P-6).
```

## 8. Findings and assumptions

| # | Item | Note |
|---|---|---|
| A-P20.08-1 | PgBouncer ≥ 1.21 prepared-statement support — UNVERIFIED (carried from P11.19 §3.2 / P12.06 A-1) | Verified at the gate against the pinned pooler version |
| A-P20.08-2 | The sqlx statement-cache option name — ASSUMPTION (carried) | Verified at the implementing wave's first build |
| A-P20.08-3 | The P-3/P-4 starting sizes assume the accept-path concurrency envelope of P20.03's design allocations | Re-derived from the first real run; never treated as measured |
| F-P20.08-1 | No runtime compatibility evidence exists | V-1..V-5 are the generators; carried, not hidden |

Status: **WRITTEN — NOTHING DEPLOYED, NO VALIDATION RESULT**; a config
design cannot pool a connection by being written; the deployment gate and
the V-run are the human gates and have not happened and are not scheduled.
