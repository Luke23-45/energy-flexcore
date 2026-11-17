<!-- P12.06 — PgBouncer introduction design (transaction-mode pooling; SQLx
     prepared-statement compatibility validation plan). CONSUMES the P11.19
     record (records/35_p11_19_rls_pooling_reconciliation.md) — the RLS
     transaction-scoped SET LOCAL contract and the pooling compatibility
     plan are DEFINED THERE and consumed here, never redefined. No pooler
     is added, no config is deployed, no validation run has happened — the
     deployment gate is P11.19 §3.6's, inherited unchanged. -->
# P12.06 — PgBouncer Introduction Design (transaction mode — design + validation plan; nothing deployed, nothing run)

| Field | Value |
|---|---|
| Subtask | P12.06 — PgBouncer introduction design (`ledger_phase_12.md` §P12.06) |
| Wave / agent | Phase 12 wave 1, Agent A |
| Status | **DESIGN + VALIDATION PLAN — WRITTEN-UNDEPLOYED, NEVER RUN.** No PgBouncer instance exists; no config is added to the deployment tree; no validation exercise has run. |
| Sources | [S-69] (RLS leak across pooled connections — the transaction-scoping hazard); [S-70] (AWS multi-tenant RLS guide — the SET-local / current_setting pattern); **P11.19 record** `docs/phases-11-20/phase-11-operational-robustness/records/35_p11_19_rls_pooling_reconciliation.md` (§2 systematic audit; §3 the compatibility plan — consumed, never redefined); P11.19 §3.2's UNVERIFIED markers (PgBouncer 1.21 prepared-statement support; the sqlx statement-cache option name — ASSUMPTION); P1.13 recorded decision (runtime `sqlx::query` only, no compile-time macros); P12.05 §3.2 (the `SET LOCAL synchronous_commit` interaction); plan §2.2 C-POOL-1 (the pool-exhaustion chaos experiment references THIS design) |
| Files touched | this record only (per P11.19 §3.6, a pooler's config lands in the deployment tree only at the deployment gate, with its own decision record) |

## 1. Honest status — the plain statement

**No pooler is deployed; no pooler configuration exists in the deployment
tree; no validation exercise has run.** This record delivers what plan §2
assigns: the transaction-mode pooling design (§2), the SQLx
prepared-statement compatibility validation plan (§3), and the linkage to
the P11.19 contract (§4). The P11.19 record is the authority this subtask
consumes: its §2 audit verdict (every tenant-context write is
transaction-scoped; every read fails closed when unset) is the
precondition this design relies on, and its §3 plan is the plan this
subtask turns into a concrete validation matrix — nothing in P11.19 is
redefined, re-decided, or softened here.

## 2. Transaction-mode pooling design

| Element | Design | Status |
|---|---|---|
| Pooling mode | `transaction` mode only — server connections are shared between clients between transactions; this is the mode the P11.19 §3 compatibility plan was written for | DESIGN |
| What is safe as-is | The tenant GUC (`SELECT set_config('flexcore.tenant_id', $1, true)`) — `is_local = true` gives SET LOCAL semantics; the setting dies with the transaction, so a pooled server connection handed onward carries NO residual tenant context (P11.19 §3.1; [S-69][S-70]) | The P11.19 §2 audit is the static evidence; runtime evidence is §3 V-3's |
| What must stay off the pooler | `run_migrations` and the partition-maintenance DDL path connect DIRECTLY (migrator/maintenance roles, P1.12 §3) — P11.19 §3.5; DDL-in-transaction + a pooled multiplexer is not a tested combination | Inherited rule, unchanged |
| Session features | None in use — the P11.19 §2 audit found no `LISTEN`, advisory lock, session variable, `DISCARD`, or temp-table reliance anywhere in the adapter | Consumed verdict |
| Transaction semantics | Units of work are single native transactions (`pool.begin()` … commit/rollback); row locks (`FOR UPDATE` — the P11.15 chain head) live inside the transaction and survive transaction pooling by definition (P11.19 §3.4) | Consumed verdict |
| `synchronous_commit` interaction | The P12.05 §3.1 design sets `SET LOCAL synchronous_commit = remote_apply` inside command-lifecycle transactions — transaction-scoped, so it dies with the transaction like the tenant GUC; the pooler must not be relied on to carry any commit-durability state between transactions | Cross-link to record 05 §3.2 |
| Pool sizing | `<PLACEHOLDER>` — a sizing decision against real concurrency measurements; never invented here | Deployment decision |
| Config location | NOT in this repository at design time: when the deployment gate opens, the pooler config lands in the deployment tree with its own decision record (P11.19 §3.6, inherited verbatim-in-substance) | HARD GATE |
| Never-granted permissions | The pooler user is a database role question only; no permission-set change is in scope (ADR-010 untouched) | Statement |

Shape reference (ILLUSTRATIVE ONLY — not deployed, not committed to the
deployment tree; option names against the pinned PgBouncer version are
UNVERIFIED):

```ini
;; Illustrative transaction-mode shape — the deployed copy lands in the
;; deployment tree at the gate, never here.
[databases]
flexcore = host=<PLACEHOLDER> port=5432 dbname=flexcore

[pgbouncer]
pool_mode = transaction
listen_addr = <PLACEHOLDER>
listen_port = <PLACEHOLDER>
auth_type = <PLACEHOLDER>        ; secret material via secret management (P11.16)
admin_users = <PLACEHOLDER>
;; prepared-statement support and its exact option name are UNVERIFIED
;; (P11.19 §3.2) — see §3 V-2 before trusting any value here.
```

## 3. SQLx prepared-statement compatibility validation plan (written-unrun)

The real hazard under transaction pooling (P11.19 §3.2, consumed): sqlx
caches server-side prepared statements per connection, and pooling layers
historically could not track named prepared statements across clients
(PgBouncer gained prepared-statement support in 1.21 — UNVERIFIED external
claim per P11.19 §4.2). The adapter uses runtime `sqlx::query` only (the
P1.13 recorded decision), which keeps both mitigations available. The
validation plan, every step written-unrun:

| ID | Exercise | Procedure | Pass criterion | Status |
|---|---|---|---|---|
| V-1 | Static audit re-run | Re-execute the P11.19 §2 grep sweep (the same pattern set) against the adapter as it stands when the gate opens; confirm the verdict still holds (no session `SET`, no pool hooks planting state) | Same verdict as P11.19 §2 — any new session-level state is a FINDING | UNRUN |
| V-2 | Prepared-statement matrix | Run the adapter's query paths through the pooler twice: (a) PgBouncer ≥ 1.21 with prepared-statement support enabled (option name UNVERIFIED — verify against the pinned version) — exercise every runtime query path repeatedly across pooled connections; (b) sqlx statement-cache capacity 0 (the exact option name is ASSUMPTION per P11.19 §3.2 — verified at the implementing wave's first build) | Every query path completes with correct results under BOTH configurations; zero server-side "prepared statement does not exist" class errors; results recorded from the run | UNRUN |
| V-3 | RLS-under-pooling negative test ([S-69]) | With the pooler in transaction mode: tenant A's transaction commits; the SAME server connection is handed to tenant B's transaction; attempt cross-tenant reads both ways; include an UNSET-context transaction (the fail-closed path) | Tenant B sees none of tenant A's rows; unset context matches nothing (fail closed) — zero leakage; transcript recorded | UNRUN |
| V-4 | Command-lifecycle regression through the pooler | Drive a full command lifecycle (the closed 14-state vocabulary) through the pooler, including the P12.05 `SET LOCAL synchronous_commit` variant once P12-E-1 lands | Lifecycle completes; state vocabulary round-trips; zero re-issued commands; per-transition timings recorded (never judged against a threshold that was not X-8 pre-registered) | UNRUN |
| V-5 | Excluded-path check | Confirm migrations and partition maintenance still connect DIRECTLY, bypassing the pooler | Direct connections observed; the P11.19 §3.5 rule intact | UNRUN |

Execution gate: V-1..V-5 run ONLY at the deployment gate, by the human
executor, on a real pooler — never in this loop, never as a simulation
substitute (a static audit cannot produce runtime evidence; P11.19 §4.1).

## 4. Linkage to the P11.19 contract (consumed, never redefined)

| P11.19 element | How this record uses it |
|---|---|
| §1 the verified pattern (`set_config(..., true)` inside the open transaction) | The load-bearing property the §2 design is built on — cited, unchanged |
| §2 the systematic audit (verdict: no code fix required) | The precondition; V-1 re-runs the sweep rather than assuming it still holds |
| §3.1 tenant context: compatible as-is | Adopted verbatim; the ONE deployment rule (never set the GUC outside a transaction) is inherited |
| §3.2 prepared statements (UNVERIFIED markers) | §3 V-2 operationalizes it; both mitigation options kept; the UNVERIFIED/ASSUMPTION labels carry through unchanged |
| §3.5 migration runner excluded | §2 table row; V-5 verifies |
| §3.6 deployment gate | The HARD GATE in §2 — no pooler config in the deployment tree until the gate opens with its own decision record |
| §4 assumption register | Carried: static audit ≠ runtime evidence; the 1.21 capability and the option name remain UNVERIFIED/ASSUMPTION |

## 5. Cross-subtask linkage

- **P12.05:** the `SET LOCAL synchronous_commit` design is exercised by
  V-4; record 05 §3.2 names the interaction.
- **C-POOL-1 (plan §2.2):** the pool-exhaustion chaos experiment's
  steady-state hypothesis references THIS design (backpressure 429s, not
  hangs) — the experiment record is a wave-1 sibling (Agent B's P12.07;
  expected path `records/21_p12_07_*` per the NN_p12_NN_slug naming),
  cited by expected path, not yet written.
- **P12.04 T3/T4:** the stateless service tiers' RTO assumptions do not
  depend on the pooler; no register row changes.

## 6. What this record does NOT establish

1. No pooler exists; no config is committed; no runtime behaviour is
   claimed about PgBouncer, sqlx, or their interaction.
2. No compatibility verdict: V-1..V-5 are UNRUN — "compatible as-is" is
   P11.19's static-evidence position plus a plan, never a runtime claim.
3. No RLS guarantee under pooling: V-3 is the evidence, and it has not
   run; zero observed leakage from a finite test set is a test outcome,
   never a guarantee (§13.3 semantics).
4. No deployment-tree change: the gate (P11.19 §3.6) owns that act.
5. No dispatch authority (ADR-010 untouched).

## 7. Reviewer-verification procedure (human)

1. Confirm §1's plain statement; no pooler, no config file, no run.
2. Confirm §4 consumes P11.19 element-by-element with no redefinition —
   trace each row to `records/35_p11_19_rls_pooling_reconciliation.md`.
3. Confirm §3's V-1..V-5 are marked UNRUN, that V-2 keeps the UNVERIFIED/
   ASSUMPTION labels from P11.19 §3.2, and that V-3's fail-closed
   criterion matches the migration-policy shape (unset context ⇒ NULL ⇒
   no match).
4. Confirm the illustrative config block carries no real endpoint, no
   auth material, and is labeled illustrative.
5. Confirm §5's sibling citation for C-POOL-1 uses the expected-path
   pattern and is marked not-yet-written.
6. Confirm no checkbox in `ledger_phase_12.md` was touched by this
   subtask.

## 8. Filling commands (for the human executor)

```sh
# (a) the documented-tools gate (run after ANY edit to this record):
python tools/verify_docs.py
python tools/validate_agent.py
python -I tools/check_dependency_rule.py

# (b) At the deployment gate: deploy the pooler per the deployment's own
#     decision record (P11.19 §3.6), run V-1..V-5, fill the pass criteria
#     FROM THE RUN ONLY; pre-register any timing threshold (X-8) before
#     V-4's run.
# (c) Record the pooler's decision record and config in the deployment
#     tree; update this record's §2 status column from DESIGN to deployed
#     only after V-1..V-5 are green.
```

## 9. Findings and assumptions

| # | Item | Note |
|---|---|---|
| A-P12.06-1 | PgBouncer ≥ 1.21 prepared-statement support — UNVERIFIED external claim (P11.19 §3.2, carried) | Verify against the pinned version at deployment |
| A-P12.06-2 | The sqlx statement-cache-capacity option name — ASSUMPTION (P11.19 §3.2, carried) | Verified at the implementing wave's first build |
| A-P12.06-3 | C-POOL-1's backpressure semantics trace to the API-edge limiter seam and the P12.06 design | The experiment (Agent B) is the evidence; not yet written — expected-path citation |
| F-P12.06-1 | No runtime RLS-under-pooling evidence exists | V-3 is the generator; carried, not hidden |

Status: **WRITTEN-UNDEPLOYED** — a design cannot pool connections by being
written; the deployment gate and the validation run are the human gates
and have not happened and are not scheduled.
