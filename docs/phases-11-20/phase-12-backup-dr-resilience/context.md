# Phase 12 — Survivability: Backup, DR and Resilience Engineering — Context

Date: 2026-11-14. Status: **NOT STARTED** — 0/16 subtasks (see
`ledger_phase_12.md`). This is a planning artifact: no tool is installed, no
script runs, no drill is scheduled, and no backup exists. Citations `[S-n]`
resolve in `../00_GAP_ANALYSIS.md` §11; the rules in
`../01_PROGRAM_CONVENTIONS.md` bind everything below.

## 1. Why this phase exists

The Phase 0–10 program wrote backup, DR, and upgrade plans and left every
execution cell empty. Phase 12 turns survivability from plans-with-empty-
records into designed, scripted, human-gated procedures — without running
any of them. The named gaps:

| Gap | Register / evidence |
|---|---|
| A backup configuration that has never been restored | LR-204; P9.24 §1 ("until §9 is filled from a real run, this plan IS that gap") |
| RTO/RPO unsettable (no customer, D1 OPEN) | LR-205; P9.24 §4 (PLACEHOLDER requirements, EMPTY bases) |
| JetStream backup posture undecided (snapshots vs replication) | LR-210 (registered in `ledger/phase-10-v1-release-hardening/17_operational_risk_review.md`; carried concretely as R-P9.24-7) |
| No PITR design, no timed restore drill | gap analysis §5 Tier 1: "Tested PITR (pgBackRest/WAL-G), timed restore drills" [S-67][S-68] vs the LR-204 reality |
| No HA/standby design for the transactional store | gap analysis §5 Tier 3: Patroni + sync mode for lifecycle tables — "design now, deploy on availability contracts" [S-67] |
| No pooling layer; RLS-under-pooling interaction unaudited | gap analysis §5 Tier 0/3 [S-69][S-70] |
| Zero chaos engineering ("chaos": zero hits across 0–10) | gap analysis §5 Tier 1 [S-81][S-82] |

Engineering research behind the phase (Track D of the gap analysis): a
backup that has never been restored is not a backup — the restore drill is
the evidence, not the backup job [S-68]; chaos engineering is defined by a
steady-state hypothesis, a controlled blast radius, and scheduled game
days, not by ad-hoc fault poking [S-81][S-82].

## 2. What exists today (verified paths; all WRITTEN-UNRUN)

Everything below is written Phase 0–10 artifacts; none of it has ever been
compiled, run, backed up, or restored.

| Seam | Path | What Phase 12 builds on |
|---|---|---|
| The 0–10 backup/DR template (never executed) | `ledger/phase-09-pilot-qualification/24_backup_dr_rehearsal.md` | the R-0..R-10 restore-rehearsal procedure and E-1..E-6 edge-outage test — a PLAN whose §8 run record is entirely EMPTY (the LR-204 form) |
| RC backup/restore re-run plan | `ledger/phase-10-v1-release-hardening/11_backup_restore_rc.md` | RE-RUN-NOT-INHERIT discipline; RC-specific checks (schema-version match, stream-config match, scoped restore authorization) — run record EMPTY |
| RC upgrade/rollback matrix | `ledger/phase-10-v1-release-hardening/12_upgrade_rollback_rc.md` | rollback = re-activation of a previously verified version; an irreversible migration rolls back by restore-from-backup, never a down-migration |
| JetStream topology | `crates/adapters/nats/src/topology.rs` | 12 named streams (4 provisioned in Phase 1: `FLEXCORE_REGISTRY`, `FLEXCORE_STATE`, `FLEXCORE_AUDIT`, `FLEXCORE_DLQ`), `Limits` retention, `File` storage, `DiscardOld`, `num_replicas: 1` (D-J5: clustering is a deployment decision) — the object of the LR-210 posture decision and of P12.14's drill |
| Postgres partitioning | `crates/adapters/postgres/migrations/0010_telemetry_storage_p2_06.up.sql` | telemetry `PARTITION BY RANGE (event_time)` with monthly partitions pre-created by the P2.20 maintenance job — partition-aware backup/restore belongs in the procedure design |
| Worker supervision | `apps/worker/src/supervision.rs` | timeouts, bounded deterministic retries, crash isolation (immediate FAILED), `BackpressureRefused` (never unbounded queues), pool status Healthy/Degraded/Empty — the in-code behavior the chaos experiments assert |
| Edge degraded modes | `crates/edge-agent/src/runner.rs` | audited `DegradedBufferingEntered` / `DegradedBufferingExited` transitions around bounded store-and-forward |
| Edge CRC-32 journals | `crates/edge-agent/src/buffer_file.rs` | fixed magic header + `[u32 LE len][u32 LE crc32][payload]` framing, truncate-at-first-corrupt recovery — the framing restored copies are verified by |
| Fault-injection module | `apps/worker/python/drill/fault_injection.py` | P8.24 pure-stdlib scaffolding over INJECTED callables with a closed fault vocabulary (`none`/`error`/`drop`/`crash_before`/`crash_after`/`scripted_reply`/`replay_last`/`reorder_reversed`, plus `clock_skew` and `oversize` helpers) — the pattern P12.09 extends |
| Deployment/upgrade model | `docs/operations/deployment-upgrade-model.md` | §5 degraded modes and the central-outage rule (site-local safety stays authoritative); §6 the backup/restore model with placeholder RPO/RTO — the vocabulary P12.10's degradation matrix maps to |

## 3. Constraints

1. **The program bans (conventions §1) apply in full.** No tool is
   installed (pgBackRest, WAL-G, Patroni, PgBouncer are selection records
   and design documents, never deployed software in this loop); no script,
   experiment, or drill runs; test files are written complete and left for
   the human build/test gate.
2. **Every drill, restore, and timing is a HUMAN GATE INPUT recorded UNRUN**
   (conventions §5). Timed-evidence cells are written as EMPTY templates and
   filled only from real runs by the human executor — a filled cell without
   a run is a fabrication defect (the P9.24 §8 rule, inherited).
3. **RTO/RPO values in Phase 12 records are RECOMMENDATIONS, not
   requirements.** The §21.2 rule (do not set RPO/RTO without customer and
   service requirements) is inherited from P9.24 §4 / P10.11 §5; Phase 12
   may propose per-tier recommendation values and the backup cadence each
   implies, but a value becomes binding only with operator sign-off — a
   human gate whose cell stays EMPTY.
4. **Dependency note: Phase 11 SLOs are the steady-state metrics the chaos
   experiments assume.** P12.07's hypotheses are anchored on the Phase 11
   SLO records; until those records exist the hypotheses are marked
   PENDING-P11-ANCHOR rather than invented. Cross-phase seams are defined
   once in the earlier phase and consumed, never redefined (gap analysis §9).
5. **Simulation-first and the release ladder do not move** (`../README.md`
   hard boundary 4): drills are designed for lab/isolated environments;
   nothing here grants field write authority or touches the never-granted
   permission set.
6. **Honesty rules (conventions §7):** no capability claim about unrun code;
   external claims cite `[S-n]` or are marked UNVERIFIED/ASSUMPTION; §13.3
   semantics — zero observed violations from a finite test set is a test
   outcome, never a guarantee — govern every "resilient" statement (P12.15).

## 4. Glossary (as the terms are used in this phase)

| Term | Meaning here |
|---|---|
| RTO | Recovery Time Objective — maximum tolerable time from failure to restored service; in Phase 12 records a RECOMMENDATION pending operator sign-off |
| RPO | Recovery Point Objective — maximum tolerable data-loss window (snapshot instant to newest lost write); same RECOMMENDATION status |
| PITR | Point-in-time recovery — rebuilding the database to a chosen instant from a base backup plus archived WAL |
| WAL archiving | Continuous archiving of PostgreSQL write-ahead-log segments; the mechanism that bounds the achievable RPO between base backups |
| Restore drill | A real restore into an isolated environment with timed, recorded evidence — the act that makes a backup credible [S-68] |
| Steady-state hypothesis | The pre-registered claim about normal system behavior that a chaos experiment verifies during and after fault injection [S-81] |
| Blast radius | The set of components and tenants an experiment may affect; controlled by design and by the human-gated runbook, never improvised [S-81][S-82] |
| Game day | A scheduled, human-supervised exercise that runs chosen experiments against steady-state hypotheses and records outcomes [S-82] |
| A/B (two-slot) layout | NOT a Phase 12 deliverable — edge A/B slot updates are Phase 18 (P10.12 §3.2 records the two-slot register shape only as the recommended edge deployment shape) |
| Failover | Automatic promotion of a standby to primary after the primary is judged failed (Patroni's role in the P12.05 design) |
| Standby | A continuously-recovering replica kept in sync by WAL streaming; a warm standby reduces RTO, the sync mode bounds RPO |
| synchronous_commit | The PostgreSQL commit-acknowledgment level (`local`, `remote_write`, `remote_apply`); `remote_apply` trades commit latency for a near-zero replica lag [S-67] |
| Degradation matrix | The per-dependency-down behavior table (P12.10) mapping each unreachable dependency to documented, audited degraded behavior — never a silent capability claim |
