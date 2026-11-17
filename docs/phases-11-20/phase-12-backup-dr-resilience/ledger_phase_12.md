# Phase 12 Ledger — Survivability: Backup, DR and Resilience Engineering

Date: 2026-11-14. Status: **IMPLEMENTED — awaiting human build/test/drill** — 16/16 subtasks ticked. Phase close: HANDOVER.md, CHECKLIST.md, records/52_p12_16_phase_close.md. No drill, backup, restore, or fault injection has ever run; the human-gate queue is in CHECKLIST.md.

Gate: this ledger moves only after the human gate on this roadmap AND the
first human build of the Phase 0–10 workspace are green (`../README.md`
hard boundary 1; `../01_PROGRAM_CONVENTIONS.md` §3). Until then every line
stays unticked and nothing in the phase claims implementation, run, or
verification status.

Tick convention (carried verbatim from the conventions): a checkbox is
ticked ONLY as "implemented, awaiting human build/test" with a
decision-record citation — never as "tested", "verified", "passed", or
"works". Tick flips are applied by `python -I` heredoc with exact-match
count==1 assertions, or by the Write/Edit tools for whole files. Decision
records go under `phase-12-backup-dr-resilience/records/`, created at
implementation time, not during planning.

## Subtask register

- [x] **P12.01 — Backup architecture design (pgBackRest vs WAL-G selection record).** Scope: selection record with DEFAULT pgBackRest (parallel backup + incremental + S3-compatible object-storage target), target shape, retention schedule per data class. Evidence: selection record citing [S-68] and the P9.24 §3 decision template; no tool installed. (decision record: `records/01_p12_01_backup_architecture.md`)
- [x] **P12.02 — Backup procedure scripts + configs (written-unrun).** Scope: backup and verify scripts/configs for PostgreSQL, plus the JetStream snapshot posture closing the LR-210 decision point (stream snapshots vs replication, DEFAULT recorded as P12-E-2). Evidence: scripts/configs labeled WRITTEN-UNRUN; P12-E-2 register entry. (decision record: `records/02_p12_02_backup_procedures.md`)
- [x] **P12.03 — Restore drill runbook (isolated restore; EMPTY timed cells).** Scope: isolated-restore procedure with application-validation steps (outbox/command consistency, audit completeness, append-only property survival) and a timed-evidence template with EMPTY cells. Evidence: runbook citing P9.24 §5 R-0..R-10 and P10.11 §3 by citation, not rewrite. (decision record: `records/03_p12_03_restore_drill_runbook.md`)
- [x] **P12.04 — RTO/RPO register per system tier (RECOMMENDATIONS; human-gated sign-off).** Scope: per-tier RTO/RPO recommendations over control plane, workers, edge, console, with the backup cadence each implies; operator sign-off = human gate, cell EMPTY. Evidence: register citing [S-82], LR-205, and P9.24 §4. (decision record: `records/04_p12_04_rto_rpo_register.md`)
- [x] **P12.05 — Standby replication design (Patroni; DEPLOYMENT-gated, P12-E-1).** Scope: Patroni topology design, synchronous_commit choice for the command-lifecycle tables with the remote_apply trade-off, failover drill plan. Evidence: design record citing [S-67] and migrations 0008/0016; P12-E-1 register entry. (decision record: `records/05_p12_05_standby_replication.md`)
- [x] **P12.06 — PgBouncer introduction design (transaction mode; SQLx validation plan).** Scope: transaction-mode pooling design, SQLx prepared-statement compatibility validation plan, linkage to the P11.19 transaction-scoped SET LOCAL RLS contract. Evidence: design + validation plan citing [S-69][S-70]; the P11.19 forward citation marked as such. (decision record: `records/06_p12_06_pgbouncer_introduction.md`)
- [x] **P12.07 — Chaos experiment program (four defining experiments, written-unrun).** Scope: steady-state hypotheses from Phase 11 SLOs (PENDING-P11-ANCHOR if absent) and repeatable scripts: (a) Postgres kill mid-command-transition → the 14-state machine resumes consistent; (b) NATS outage → commands queue/park safely, no loss; (c) Python worker crash mid-transition → redelivery + idempotency produce no duplicate dispatch; (d) connection-pool exhaustion → backpressure 429s, not hangs. Evidence: experiment scripts + hypothesis records citing [S-81][S-82]; all UNRUN. (decision record: `records/21_p12_07_chaos_program.md`)
- [x] **P12.08 — Game-day runbook template + quarterly schedule (human-gated).** Scope: runbook template (hypothesis, blast radius, roles, abort criteria, evidence capture) plus a proposed quarterly cadence. Evidence: template + schedule citing [S-82]; no game day runs. (decision record: `records/22_p12_08_game_days.md`)
- [x] **P12.09 — Fault-injection expansion of `apps/worker/python/drill/fault_injection.py`.** Scope: broader fault taxonomy (network partition, clock skew generalized, partial write, slow consumer) as injected callables extending the existing stdlib-only pattern, with complete test files. Evidence: extended module + tests written-unrun; no execution. (decision record: `records/23_p12_09_fault_injection_expansion.md`)
- [x] **P12.10 — Graceful-degradation matrix (per-dependency-down behavior).** Scope: behavior table for Postgres / NATS / worker / edge / cloud unreachable, mapped to the existing degraded-mode vocabulary; no silent capability claims. Evidence: matrix tracing `runner.rs`, `supervision.rs`, and `deployment-upgrade-model.md` §5 symbols. (decision record: `records/24_p12_10_degradation_matrix.md`)
- [x] **P12.11 — DR runbook (site-loss scenario).** Scope: site-loss scenario, restore-into-isolation procedure, DNS/reconfiguration steps, communication checklist. Evidence: runbook citing P9.24 §5–§6 and `deployment-upgrade-model.md` §6; UNRUN. (decision record: `records/25_p12_11_dr_runbook.md`)
- [x] **P12.12 — Edge-agent resilience review (buffers, drift, prolonged outage).** Scope: offline windows, buffer growth bounds, clock-drift policy interplay (P6.31 hold-on-drift), store-and-forward limits under prolonged outage. Evidence: review record tracing `buffer_file.rs` and `clock_quality.rs` bounds; no claims beyond written code. (decision record: `records/26_p12_12_edge_resilience_review.md`)
- [x] **P12.13 — Backup/restore + DR evidence templates (EMPTY cells; X-8 pre-registration).** Scope: run-record templates with EMPTY cells for every drill; any timing threshold pre-registered (X-8) before any run that could fill it. Evidence: EMPTY templates following the P9.24 §8 and P10.11 §6 shapes. (decision record: `records/31_p12_13_evidence_templates.md`)
- [x] **P12.14 — JetStream data-loss rehearsal design (delete + restore-from-snapshot).** Scope: stream delete + restore-from-snapshot drill mirroring the Postgres restore drill, with scoped restore authorization inherited from P10.11 §4.4. Evidence: drill design citing [S-65] and `topology.rs`; UNRUN. (decision record: `records/32_p12_14_jetstream_rehearsal.md`)
- [x] **P12.15 — Resilience acceptance criteria + records (§13.3 semantics).** Scope: what "resilient" may and may not be claimed as — zero observed violations from finite tests is a test outcome, not a guarantee. Evidence: criteria record citing conventions §5/§7. (decision record: `records/51_p12_15_resilience_acceptance_criteria.md`)
- [x] **P12.16 — Decision records + close (P12-E-1..E-3; consistency audit).** Scope: E-decision register entries (P12-E-1 Patroni topology, P12-E-2 JetStream backup, P12-E-3 DR site necessity, tied to Phase 20 multi-region), per-batch decision records, cross-subtask consistency audit. Evidence: register entries + audit record; E-decisions stay OPEN, human-closed. (decision record: `records/52_p12_16_phase_close.md`)

## Decision points (OPEN — agents never close them)

| ID | Question | Status | DEFAULT recorded |
|---|---|---|---|
| P12-E-1 | Patroni standby topology and synchronous_commit level for the command-lifecycle tables | OPEN | remote_apply for 0008/0016 tables IF latency accepted; remote_write fallback; DEPLOYMENT-gated |
| P12-E-2 | JetStream backup posture (stream snapshots vs replication) | OPEN | snapshot-based backup to the shared object-storage target; replication revisited at a second-cluster trigger |
| P12-E-3 | Second (DR) site necessity for v1.x | OPEN | no second site; DR = site-loss runbook + restore-into-isolation rehearsal; revisit at the Phase 20 multi-region trigger |

## Wave notes

| Wave | Subtasks | Note |
|---|---|---|
| 1 | P12.01–P12.06 (Agent A, backup/replication); P12.07–P12.12 (Agent B, chaos/degradation) | parallel; P12.07 hypotheses may be PENDING-P11-ANCHOR |
| 2 | P12.13–P12.16 | evidence templates, acceptance criteria, decision records, consistency audit — last |

## Carried-gap closures this register tracks

| Gap | Register ID | Closing subtasks |
|---|---|---|
| Backup configuration never restored | LR-204 | P12.01–P12.03, P12.13 |
| RTO/RPO unsettable | LR-205 | P12.04 (recommendations + human-gated sign-off) |
| JetStream backup posture undecided | LR-210 | P12.02, P12.14 (P12-E-2) |
| No HA/standby design | gap analysis §5 Tier 3 | P12.05 (P12-E-1, deployment-gated) |
| No pooling design; RLS-under-pooling unaudited | [S-69][S-70] | P12.06 |
| Zero chaos engineering | [S-81][S-82] | P12.07–P12.09 |

Status: **NOT STARTED (0/16).** The next status change happens only through
the conventions-disciplined tick process above; wave assignment is in
`plan.md` §3.
