<!-- P12.05 — Standby replication design (Patroni topology; synchronous_commit
     choice for the command-lifecycle tables). DEPLOYMENT-GATED: nothing
     deploys — single-host Compose is the current shape (plan risk 3); no
     Patroni, no standby, no drill has run; every evidence cell EMPTY.
     P12-E-1 is registered OPEN with its DEFAULT (one Patroni primary + one
     standby; remote_apply for the 0016 command/transition and 0008 audit
     transactions IF the measured commit-latency cost is accepted;
     remote_write is the documented fallback). -->
# P12.05 — Standby Replication Design (Patroni — DEPLOYMENT-gated; P12-E-1 OPEN with DEFAULT; nothing deployed, nothing measured)

| Field | Value |
|---|---|
| Subtask | P12.05 — Standby replication design (`ledger_phase_12.md` §P12.05) |
| Wave / agent | Phase 12 wave 1, Agent A |
| Status | **DESIGN RECORD — WRITTEN-UNDEPLOYED.** No Patroni instance exists, no standby runs, no commit-latency number exists, no failover drill has run. Every evidence cell is EMPTY. |
| Sources | [S-67] (Patroni replication modes — the remote_apply / remote_write trade-off); `crates/adapters/postgres/migrations/0008_audit_records.up.sql` (append-only audit: grants + trigger + RLS); `crates/adapters/postgres/migrations/0016_commands_p4_10.up.sql` (immutable command rows; column-level UPDATE grant on exactly `state` + `superseded_by_command_id`; RLS); `crates/adapters/postgres/migrations/0019_audit_chain_p11_15.up.sql` (the P11.15 keyed chain); P11.19 record (`docs/phases-11-20/phase-11-operational-robustness/records/35_p11_19_rls_pooling_reconciliation.md` — the transaction-scoped GUC discipline, consumed); P12.04 register (T1-variant row); plan §4 (P12-E-1 wording); `ledger/phase-10-v1-release-hardening/24_deployment_upgrade_model.md` / `docs/operations/deployment-upgrade-model.md` (the deployment model this design slots into) |
| Files touched | this record only |

## 1. Honest status — the plain statement

**Nothing is deployed; no standby exists; no latency has been measured; no
failover has occurred or been drilled.** The gap analysis Tier 3 posture
([S-67], §5) is "design now, deploy on availability contracts" — this
record is that design. The current deployment shape is single-host Compose
(`deploy/compose/docker-compose.yml`): Patroni does not deploy on it, and
this record does not pretend otherwise (plan §6 risk 3). P12-E-1 is
registered OPEN with its DEFAULT; the decision closes with the human owner
using THIS design's drill-produced latency evidence and the Phase 11
capacity signals.

## 2. Topology design — one Patroni primary + one standby (the DEFAULT)

| Element | Design | Status |
|---|---|---|
| Nodes | One Patroni primary + one standby (physical streaming replication) — the P12-E-1 DEFAULT | DESIGN; nothing deployed |
| Cluster membership / leader election | Patroni against a DCS (etcd / Consul / ZooKeeper family) — the DCS CHOICE is a deployment sub-decision recorded `<EMPTY>` under P12-E-1; [S-67] is the mechanism source | UNVERIFIED specifics at deployment |
| Replication | Physical streaming WAL replication (Patroni's mechanism) — schema objects (triggers, grants, RLS policies) replicate with the data; no logical-replication complication enters the 0008/0016 append-only/immutability surface | DESIGN; re-verified by the failover drill's inspection step (§5 F-3) |
| Synchronous standby naming | The standby is registered in `synchronous_standby_names` so `remote_apply` has a member to wait for — exact option shape UNVERIFIED against the deployed PostgreSQL version; verify at deployment | DESIGN |
| Quorum / fencing | Single-standby topology: the sync member set is the one standby; split-brain behaviour is bounded by Patroni's DCS-backed fencing — the watchdog question is a deployment decision, `<EMPTY>` | DESIGN; not a capability claim |
| Backups interplay | pgBackRest (P12.01/P12.02) runs against the PRIMARY; the standby is NOT a backup — it bounds RPO/RTO for availability events, while PITR remains the recovery mechanism for corruption and operator-error events (a replica faithfully replicates damage too) | DESIGN; stated to prevent the classic "replica = backup" conflation |
| Deployment gate | NO Compose deployment; becomes deployable only against availability contracts (multi-host, DCS) — Phase 20 consumes this design (plan §5) | HARD GATE |

## 3. The synchronous_commit choice for the command-lifecycle tables

### 3.1 Mechanism

`synchronous_commit` is a transaction-scoped GUC in PostgreSQL — it can be
set per transaction with `SET LOCAL synchronous_commit = <level>`. The
design therefore does NOT flip the instance default: the instance stays at
its deployment default (`on`), and the COMMAND-LIFECYCLE TRANSACTIONS raise
their own durability:

```sql
-- Inside the command-lifecycle transaction (mirrors the P11.19 pattern:
-- set exactly once, inside the already-open transaction, transaction-scoped):
SET LOCAL synchronous_commit = remote_apply;
```

The affected transaction classes are the ones whose commit the 14-state
command lifecycle and the audit evidence depend on:

- **0016 `dispatch_commands` / `dispatch_command_transitions`** — the
  command rows (immutable per tenant; content columns with no update path)
  and the one-active-transition journal (compare-and-swap UPDATE shape,
  D-P4.10-2);
- **0008 `audit_records`** (and the 0019 P11.15 keyed-chain rows) — the
  append-only evidence: a commit acknowledged before the WAL is durably on
  the standby is a commit a primary loss could silently erase, which for
  audit evidence is the worst class of loss.

### 3.2 The trade-off (P12-E-1's substance) [S-67]

| Level | Wait at commit | Effect on the command path | Status |
|---|---|---|---|
| `remote_apply` | The commit waits until the standby has REPLAYED the WAL | Near-zero replica lag for the command-lifecycle tables; the standby is current for failover of those transactions; the cost is added commit latency on every lifecycle transaction | The DEFAULT, conditional: **IF the measured commit-latency cost is accepted** |
| `remote_write` (the documented fallback) | The commit waits only until the WAL is WRITTEN (not replayed) on the standby | Lower latency; small replay-lag window remains (the standby may hold un-replayed WAL) | The fallback if the measured cost is not accepted |
| async / `local` | No standby wait | Lowest latency; bounded replica lag = bounded RPO (a P12.04 T1 re-derivation) | The recorded third option; not the default |

**The mechanism-level discipline:** the `SET LOCAL` statement follows the
P11.19 audited pattern (transaction-scoped, inside the open transaction —
`records/35_p11_19_rls_pooling_reconciliation.md` §1–§2). Two interactions
are named now so the implementing wave records them:

1. **Under pooling (P12.06):** a transaction-scoped `SET LOCAL` survives
   transaction pooling by the same argument as the tenant GUC (it dies
   with the transaction) — but it is a per-connection cost negotiation
   point the P12.06 validation plan must exercise (record 06 §3 V-2).
2. **Where the statement lands:** the unit-of-work transaction openers for
   the command-lifecycle paths are the natural site; the audit-chain
   verification path (P11.15) and the outbox dispatch path each carry
   their own transaction. No code changes are made by THIS record — the
   change is an implementing-wave act gated on P12-E-1 closure.

### 3.3 What is NOT claimed

No latency number exists; no claim is made that `remote_apply`'s cost is
acceptable or unacceptable — the DEFAULT is explicitly conditional. No
claim is made that physical replication preserves anything beyond what the
failover drill inspects. No RPO value is asserted beyond the P12.04
register's T1-variant framing (near-zero lag bound for those transactions,
drill-measured, never assumed).

## 4. P12-E-1 — registered OPEN with its DEFAULT

| Field | Value |
|---|---|
| Decision ID | **P12-E-1** (registered in `ledger_phase_12.md` §Decision points — OPEN) |
| Question | Patroni standby topology and `synchronous_commit` level for the command-lifecycle tables |
| Options | one standby + `remote_apply` / one standby + `remote_write` / async standby |
| **DEFAULT (RECOMMENDATION)** | One Patroni primary + one standby; `synchronous_commit = remote_apply` for the 0016 command/transition and 0008 audit transactions **IF the measured commit-latency cost is accepted**; `remote_write` is the documented fallback. **DEPLOYMENT-gated** — nothing deploys on single-host Compose |
| Closer | The human owner, with P12.05 latency evidence (§5's drill) and Phase 11 capacity signals |
| Status | **OPEN** |

## 5. Failover drill plan (written-unrun; every evidence cell EMPTY)

Executed only on a real multi-host deployment; no step runs in this loop.
Thresholds, if any are later judged against, are pre-registered (X-8) in
the P12.13 template BEFORE the run that could fill them.

| Step | Procedure | Pass criterion | Evidence |
|---|---|---|---|
| F-1 | Record the steady state: primary/standby identities, replication lag counters, command/transition counts per tenant (the R-0 discipline, cited from P9.24 §5 R-0) | State recorded | `<EMPTY — UNRUN>` |
| F-2 | Inject a command-lifecycle transaction under `remote_apply` and measure commit latency vs the instance-default path (the same transaction without the `SET LOCAL`) on real data volumes | Numbers from the run's clocks — the P12-E-1 latency evidence | `<EMPTY — UNMEASURED>` |
| F-3 | Stop the primary (controlled); Patroni promotes the standby; verify the promoted instance preserves the schema-level properties: 0008 trigger + no-DELETE-grant, 0016 immutability trigger + column-level grant, RLS policies, 0019 chain | Trigger/grant inspection transcript on the PROMOTED node | `<EMPTY — UNRUN>` |
| F-4 | Verify command continuity across the failover: every command observed in a lifecycle state before the failover is observed in the same or a strictly later valid state after (the closed 14-state vocabulary; the C-PG-1 oracle discipline, cited — not re-specified here); zero re-issued commands (the R-6 criterion, cited) | Zero anomalies; transcript recorded | `<EMPTY — UNRUN>` |
| F-5 | Verify audit continuity: pre-failover audit counts equal post-failover counts; the 0019 chain verifies on the promoted node | Zero count deltas; chain verifies | `<EMPTY — UNRUN>` |
| F-6 | Measure the failover interval (stop → promoted → `/ready`) and the observed data-loss window for command-lifecycle transactions | Numbers from the run — judged against the P12.04 T1-variant cells in force | `<EMPTY — UNMEASURED>` |
| F-7 | Judge per the inherited P9.24 §4 rule; record the disposition; feed the latency + interval numbers into the P12-E-1 closure input set | Disposition recorded | `<EMPTY — UNRUN>` |

## 6. What this design does NOT establish

1. No deployment, no cluster, no standby: everything in §2 is design; the
   Compose stack stays single-host and untouched.
2. No latency claim: §3.2's cost question is unanswered until F-2 runs.
3. No availability guarantee: a single-standby topology has its own
   failure modes (standby loss reverts to the pre-standby posture);
   nothing here is a durability or availability promise (§13.3 semantics:
   zero observed violations from finite drills is a test outcome, never a
   guarantee).
4. No code change: no migration, no adapter change, no config change is
   made by this record; the `SET LOCAL` insertion is an implementing-wave
   act gated on P12-E-1 closure and the dependency-rule checker.
5. No dispatch authority: failover is data-plane availability; the command
   lifecycle gates, the never-granted permission set, and ADR-010 are
   untouched.

## 7. Reviewer-verification procedure (human)

1. Confirm §1's plain statement and that every §5 evidence cell is
   EMPTY/UNRUN or UNMEASURED.
2. Confirm §3.1 scopes `remote_apply` to the transaction classes named
   (0016 commands/transitions, 0008 audit incl. the 0019 chain rows) and
   states the instance default stays unchanged.
3. Confirm §3.2 carries the three-level trade-off with `remote_write` as
   the documented fallback and the explicit conditional on the DEFAULT.
4. Confirm §4 registers P12-E-1 as OPEN with the DEFAULT, and the ledger
   decision table still reads OPEN.
5. Confirm §2 states the standby-is-not-a-backup rule and the Compose
   deployment gate; confirm `deploy/compose/docker-compose.yml` is
   untouched.
6. Confirm the P11.19 pattern is consumed (cited) and not redefined; trace
   the 0008/0016/0019 citations into the migration files.

## 8. Filling commands (for the human executor)

```sh
# (a) the documented-tools gate (run after ANY edit to this record):
python tools/verify_docs.py
python tools/validate_agent.py
python -I tools/check_dependency_rule.py

# (b) On an availability-contract deployment (multi-host + DCS): deploy
#     Patroni per §2, run the §5 drill, fill the evidence cells FROM THE
#     RUN ONLY, pre-registering any threshold (X-8) first.
# (c) Take the F-2/F-6 numbers to the P12-E-1 closure decision; record the
#     closure (or the fallback choice) in the decision register — the
#     agent never closes it.
```

## 9. Findings and assumptions

| # | Item | Note |
|---|---|---|
| A-P12.05-1 | `synchronous_commit` is transaction-scopable via `SET LOCAL` (PostgreSQL behaviour) | Standard-documented behaviour; verify the exact semantics against the pinned PostgreSQL 18 (itself an UNVERIFIED pin, P10.13) at deployment |
| A-P12.05-2 | Physical replication replicates triggers, grants, and RLS with the schema | Standard mechanism assumption; the F-3 inspection exists precisely because assumptions are not evidence |
| A-P12.05-3 | Patroni's DCS/fencing specifics are UNVERIFIED until a real deployment | [S-67] covers replication modes, not DCS sizing |
| F-P12.05-1 | No latency evidence exists; P12-E-1 cannot close | Carried by design — the drill (F-2) is the evidence generator |

Status: **WRITTEN-UNDEPLOYED** — a design cannot fail over by being
written; the deployment and the drill are the human gates and have not
happened and are not scheduled.
