# Phase 12 — Survivability: Backup, DR and Resilience Engineering — Plan

Date: 2026-11-14. Status: **NOT STARTED**. Planning artifact: nothing is
installed, built, run, or drilled by this plan. Rules:
`../01_PROGRAM_CONVENTIONS.md`; sources: `../00_GAP_ANALYSIS.md` §11
(`[S-n]`); background: `context.md`.

## 1. Scope

Phase 12 designs and scripts FlexCore's survivability layer and closes the
0–10 backup/DR gap honestly: backup architecture and procedures for
PostgreSQL and JetStream (written-unrun), a restore drill runbook with EMPTY
timed-evidence templates, an RTO/RPO register whose values are
RECOMMENDATIONS pending operator sign-off, standby-replication and pooling
designs (deployment-gated), a chaos-experiment program anchored on Phase 11
SLOs, a graceful-degradation matrix, DR and game-day runbooks, an
edge-agent resilience review, a JetStream data-loss rehearsal design,
evidence templates, and honest acceptance criteria for what "resilient" may
mean afterward. Nothing in this phase takes a backup, performs a restore,
or injects a fault into a running system — every such act is a human gate
on a real deployment, which does not exist yet (P9.24 §2.3: no JetStream
stream configs, no pilot deployment, no backup configuration).

Out of scope: edge OTA / A/B slot orchestration (Phase 18); multi-region
and Kubernetes topology (Phase 20, which consumes P12's HA design);
identity, certification, and market concerns (their own phases); binding
RTO/RPO values (operator decision, human-gated); automatic health-check →
rollback wiring for edge updates (a recorded P10.12 composition contract,
not a Phase 12 deliverable).

## 2. Subtask outline (16)

| ID | Subtask | Output (all WRITTEN-UNRUN unless stated) | Citations / seams |
|---|---|---|---|
| P12.01 | Backup architecture design | pgBackRest vs WAL-G selection record (DEFAULT: pgBackRest — parallel backup, incremental, S3-compatible object-storage target), object-storage target shape, retention schedule per data class | [S-68]; P9.24 §3 decision template |
| P12.02 | Backup procedure scripts + configs | backup/verify scripts and configs, written-unrun; includes the JetStream snapshot posture that closes the LR-210 decision point (stream snapshots vs replication — DEFAULT recorded as P12-E-2) | [S-65]; `crates/adapters/nats/src/topology.rs`; LR-210 |
| P12.03 | Restore drill runbook | isolated-restore runbook with application-validation steps (outbox/command consistency, audit completeness, append-only property survival) and a timed-evidence template with EMPTY cells | P9.24 §5 R-0..R-10 (cited, not rewritten); P10.11 §3 |
| P12.04 | RTO/RPO register per system tier | per-tier RTO/RPO RECOMMENDATIONS over control plane, workers, edge, console, plus the backup cadence each implies; operator sign-off = human gate, cell EMPTY | [S-82]; LR-205; P9.24 §4 |
| P12.05 | Standby replication design | Patroni topology design, synchronous_commit choice for command-lifecycle tables (`remote_apply` trade-off [S-67]), failover drill plan; DEPLOYMENT-gated, recorded as P12-E-1 | [S-67]; `crates/adapters/postgres/migrations/` 0008/0016 |
| P12.06 | PgBouncer introduction design | transaction-mode pooling design, SQLx prepared-statement compatibility validation plan, linkage to the P11.19 transaction-scoped `SET LOCAL` RLS contract | [S-69][S-70]; P11.19 (forward citation, marked as such) |
| P12.07 | Chaos experiment program | steady-state hypotheses derived from P11 SLOs (PENDING-P11-ANCHOR if absent); the four defining experiments (§2.2) as repeatable scripts (written-unrun) | [S-81][S-82]; `apps/worker/src/supervision.rs`; `crates/flexcore-domain/src/command_state.rs` |
| P12.08 | Game-day runbook template + quarterly schedule | runbook template (hypothesis, blast radius, roles, abort criteria, evidence capture) plus a proposed quarterly cadence; every game day is human-gated | [S-82] |
| P12.09 | Fault-injection expansion | broader fault taxonomy in `apps/worker/python/drill/fault_injection.py` — network partition, clock skew (generalized from the existing `clock_skew` helper), partial write, slow consumer — as injected callables extending the existing stdlib-only pattern; complete test files written-unrun | `apps/worker/python/drill/fault_injection.py` (P8.24) |
| P12.10 | Graceful-degradation matrix | per-dependency-down behavior table (Postgres / NATS / worker / edge / cloud unreachable) mapped to the existing degraded-mode vocabulary; no silent capability claims | `crates/edge-agent/src/runner.rs`; `apps/worker/src/supervision.rs`; `docs/operations/deployment-upgrade-model.md` §5 |
| P12.11 | DR runbook | site-loss scenario, restore-into-isolation procedure, DNS/reconfiguration steps, communication checklist | P9.24 §5–§6; `docs/operations/deployment-upgrade-model.md` §6 |
| P12.12 | Edge-agent resilience review | review record: offline windows, buffer growth bounds, clock-drift policy interplay (P6.31 hold-on-drift), store-and-forward limits under prolonged outage | `crates/edge-agent/src/buffer_file.rs`; `crates/edge-agent/src/clock_quality.rs` |
| P12.13 | Backup/restore + DR evidence templates | run-record templates with EMPTY cells for every drill; X-8 pre-registration applies to any timing threshold before any run that could fill it | conventions §5; P9.24 §8; P10.11 §6 |
| P12.14 | JetStream data-loss rehearsal design | stream delete + restore-from-snapshot drill design mirroring the Postgres drill; scoped restore authorization inherited from P10.11 §4.4 | [S-65]; `crates/adapters/nats/src/topology.rs`; LR-210 |
| P12.15 | Resilience acceptance criteria + records | what "resilient" may and may not be claimed as: zero observed violations from finite tests is a test outcome, not a guarantee (§13.3 semantics) | conventions §5, §7 |
| P12.16 | Decision records + close | P12-E-1 / P12-E-2 / P12-E-3 register entries, per-batch decision records, cross-subtask consistency audit | conventions §2, §4 |

### 2.1 Ordering and in-wave dependencies

| Dependency | Direction | Note |
|---|---|---|
| P12.02 → P12.03, P12.11, P12.13 | same wave 1 (A) / wave 2 | the runbooks cite the P12.02 procedure shape; where a sibling record does not exist yet at writing time, it is cited by its expected path (the P10.11/P10.12 sibling-citation pattern) |
| P12.02 → P12.14 | wave 1 (A) → wave 2 | the JetStream drill executes the P12-E-2 DEFAULT posture; if P12-E-2 is still OPEN, the drill design executes the DEFAULT and records the dependence |
| P12.06 → P12.07 (d) | wave 1 (B), internal | the pool-exhaustion experiment's steady-state hypothesis references the P12.06 pooling design |
| P12.04 → P12.13 | wave 1 (A) → wave 2 | the evidence templates carry the RECOMMENDATION-labeled RTO/RPO cells and the X-8 pre-registration rule for any threshold |
| P12.15, P12.16 | wave 2, last | the consistency audit consumes every wave-1 and wave-2 artifact before the close record |

### 2.2 The four defining chaos experiments (P12.07 detail)

Every experiment is pre-registered: steady-state hypothesis first, blast
radius bounded by design, in-code oracle named, repeatable script
written-unrun, execution human-gated.

| ID | Injected fault | Steady-state hypothesis | Blast radius | In-code oracle |
|---|---|---|---|---|
| C-PG-1 | PostgreSQL killed mid-command-transition | a command observed in any of the 14 states before the kill is observed in the same or a strictly later valid state after recovery; no state outside the closed vocabulary; no already-issued command is re-issued by outbox replay | one lab instance, one synthetic tenant, simulation-only data | `crates/flexcore-domain/src/command_state.rs` (14-state / 27-edge table); migration 0007 outbox |
| C-NATS-1 | NATS outage during command flow | commands queue/park safely — pending publications hold in the outbox, durable consumers resume with redelivery after reconnect, zero message loss, DLQ only for poison messages | the lab NATS instance and its consumers only | `crates/adapters/nats/src/topology.rs` + `consumer.rs` (ack-after-commit); DLQ subjects outside `flexcore.v1` |
| C-WORKER-1 | Python worker crash mid-transition | redelivery plus idempotency produce no duplicate dispatch; the crash is recorded, the pool degrades visibly, recovery stays authorized | the supervised worker pool in the lab | `apps/worker/src/supervision.rs` (`record_crash`, `BackpressureRefused`, `PoolStatus::Degraded`) |
| C-POOL-1 | Connection-pool exhaustion | callers receive backpressure 429s, not hangs; no unbounded queueing | the API edge and pool in the lab | P12.06 pool design; Phase 11 API-edge limiter seam |

## 3. Wave plan (2 agents)

| Wave | Agent A | Agent B | Note |
|---|---|---|---|
| 1 | P12.01–P12.06 (backup/replication) | P12.07–P12.12 (chaos/degradation) | parallel; P12.07 marks hypotheses PENDING-P11-ANCHOR if the Phase 11 SLO records do not exist yet |
| 2 | P12.13–P12.16 (evidence/criteria/close) | — | consumes both wave-1 outputs; the consistency audit runs last |

## 4. Decision points (E-decisions — agents never close them)

| ID | Question | Options | DEFAULT / RECOMMENDATION | Closer |
|---|---|---|---|---|
| P12-E-1 | Patroni standby topology and `synchronous_commit` level for the command-lifecycle tables? | one standby + `remote_apply` / one standby + `remote_write` / async standby | DEFAULT (RECOMMENDATION): one Patroni primary + one standby, `synchronous_commit = remote_apply` for the command-lifecycle tables (0016 commands/transitions, 0008 audit) IF the measured commit-latency cost is accepted; `remote_write` is the documented fallback. DEPLOYMENT-gated — nothing deploys on single-host Compose | human owner, with P12.05 latency evidence and Phase 11 capacity signals |
| P12-E-2 | JetStream backup posture: stream snapshots vs clustering replication? | snapshot-based backup to object storage / NATS clustering / mirror on a second cluster | DEFAULT (RECOMMENDATION): snapshot-based stream backup into the same object-storage target as the PostgreSQL backups, `num_replicas: 1` unchanged at design time (D-J5 holds — clustering stays a deployment decision); replication revisited at a second-cluster trigger (Phase 20) | human owner, with P12.14 drill evidence |
| P12-E-3 | Is a second (DR) site required for v1.x? | warm DR site / cold restore-into-isolation posture / none | DEFAULT (RECOMMENDATION): no second site in the current posture — DR is the documented site-loss runbook plus the restore-into-isolation rehearsal; revisit at the Phase 20 multi-region trigger | human owner |

## 5. Cross-phase seams

| Direction | Seam | Discipline |
|---|---|---|
| Consumes from Phase 0–10 | P9.24 §5/§6 procedure, P10.11 RC checks, P10.12 rollback-over-restore rule | cited, never rewritten or re-decided |
| Consumes from Phase 11 | SLO records (steady-state hypotheses); the P11.19 transaction-scoped `SET LOCAL` RLS contract (forward citation) | defined once in the earlier phase, consumed here — never redefined (gap analysis §9) |
| Provides to Phase 20 | the HA design and latency evidence (P12.05/P12-E-1), the JetStream posture (P12-E-2), the DR-site trigger input (P12-E-3) | Phase 20 consumes the records, not new copies |
| ADR-010 boundary | every Phase 12 capability (backup, failover, degradation) adds no dispatch authority | the never-granted permission set stays byte-identical |

## 6. Risks

| # | Risk | Handling |
|---|---|---|
| 1 | Every drill and restore requires a real deployment — none exists; the JetStream stream configs are deployment artifacts that do not exist (P9.24 §2.3) | all procedures and templates land written-unrun; execution is a named human gate, never an agent act |
| 2 | Scripts are written-unrun; first execution may surface defects | the defect expectation is recorded in each artifact; the first human build of the Phase 0–10 workspace still precedes implementation (`../README.md` hard boundary 1) |
| 3 | Patroni on Compose is a design, not a promise | P12.05 states single-host Compose is the current shape; the standby design becomes deployable only against availability contracts (gap analysis §5 Tier 3 [S-67]) |
| 4 | RTO/RPO without operator input are placeholders | P12.04 labels every value RECOMMENDATION with the cadence assumption it implies; binding requires the signed sign-off cell (P9.24 §4 judgement rule inherited: a measured value worse than the requirement is a FINDING, never a re-defined requirement) |
| 5 | Chaos hypotheses depend on Phase 11 SLO records that may not exist yet | PENDING-P11-ANCHOR marking; no hypothesis is invented to fill the dependency |
| 6 | Carried 0–10 findings: restore with an older schema (P9.24 R-P9.24-4), the JetStream/DB loss-window gap (R-P9.24-7 / LR-210), the bounded edge audit buffer as an audited loss window (P9.24 R-P9.24-5) | consumed as drill assertions and review inputs, not re-derived |

## 7. Deliverables layout and verification

- This planning step produces exactly three files: `context.md`, `plan.md`,
  `ledger_phase_12.md` (conventions §8). The `records/` subdirectory is
  created at implementation time; implementation-time artifacts (scripts,
  configs, templates) land under the phase directory as the subtasks name
  them, never as new workspace crates.
- No crate, manifest, or workspace change is made by this phase's planning
  or by its implementation artifacts — backup scripts and drill modules are
  application/tooling files under `apps/worker/python/drill/` or the phase
  directory, and any dependency question goes through the additive
  `check_dependency_rule.py` update the conventions require.
- Permitted verification only (conventions §1): `python tools/verify_docs.py`,
  `python tools/validate_agent.py`, `python -I tools/check_dependency_rule.py`;
  both doc checks run after any documentation change and a task is not
  complete while either reports a failure.

## 8. Acceptance criteria (honest)

1. All 16 subtasks have complete written artifacts; ledger ticks read
   "implemented, awaiting human build/test" with a decision-record citation —
   never "tested", "verified", "passed", or "works" (conventions §2).
2. No backup is taken, no restore is performed, no fault is injected into a
   running system, no game day runs, and no timing is measured by this
   phase's writing; every evidence cell is EMPTY/UNRUN.
3. Every RTO/RPO value carries the RECOMMENDATION label and an EMPTY
   operator sign-off cell; P12-E-1..E-3 remain OPEN with their DEFAULTs
   recorded, never closed by an agent.
4. The chaos program states its steady-state hypotheses and blast radii and
   ships repeatable scripts — written-unrun — with the Phase 11 anchoring
   made explicit wherever it is not yet available.
5. The degradation matrix and the resilience acceptance criteria make only
   claims traceable to written code, under §13.3 semantics: zero observed
   violations from a finite test set is a test outcome, never a guarantee.
