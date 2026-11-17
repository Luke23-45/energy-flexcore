<!-- P12 wave-1 Agent A close-out (records 01–06 + this record; 07–18 are
     reserved for the wave-2 subtasks P12.13–P12.16 that consume both
     waves' outputs). No subtask was executed, no backup taken, no restore
     performed, no timing measured, no fault injected. No ledger checkbox
     was ticked by this agent — tick flips are the conventions-disciplined
     process, not a wave-1 act. -->
# Phase 12 — Wave 1 Agent A Close-Out (P12.01–P12.06: backup / replication / pooling)

| Field | Value |
|---|---|
| Scope | P12.01–P12.06 (plan §3 wave 1, Agent A — backup/replication); records 01–06 + this close-out |
| Date | 2026-11-14 |
| Status | **WRITTEN-UNRUN wave:** every artifact is written; nothing is installed, built, run, drilled, backed up, or restored; every evidence cell is EMPTY/UNRUN; every RTO/RPO value is a RECOMMENDATION with an EMPTY sign-off cell. |
| Ledger | `ledger_phase_12.md` untouched (no checkbox flipped by this agent — tick flips follow conventions §2, applied after human review of this wave) |
| Permitted verification | all three tools run after the final edits; verbatim outputs in §4 |

## 1. Files created

| Path | Subtask |
|---|---|
| `docs/phases-11-20/phase-12-backup-dr-resilience/records/01_p12_01_backup_architecture.md` | P12.01 — pgBackRest-vs-WAL-G selection (DEFAULT pgBackRest), P9.24 §3 rows as RECOMMENDATION/DEFAULT, object-storage target shape, per-class retention structure (periods EMPTY) |
| `docs/phases-11-20/phase-12-backup-dr-resilience/records/02_p12_02_backup_procedures.md` | P12.02 — procedure-tree record; P12-E-2 registered OPEN with the snapshot-DEFAULT (LR-210 addressed at record level) |
| `docs/phases-11-20/phase-12-backup-dr-resilience/records/03_p12_03_restore_drill_runbook.md` | P12.03 — citation map to P9.24 §5 R-0..R-10 / §6 E-1..E-6 / P10.11 §3 (never rewritten); application-validation steps; EMPTY timed-evidence template |
| `docs/phases-11-20/phase-12-backup-dr-resilience/records/04_p12_04_rto_rpo_register.md` | P12.04 — per-tier RTO/RPO RECOMMENDATIONS (T1–T7) with implied cadences; sign-off cells EMPTY; judgement rule inherited pre-printed |
| `docs/phases-11-20/phase-12-backup-dr-resilience/records/05_p12_05_standby_replication.md` | P12.05 — Patroni 1+1 topology design; transaction-scoped `remote_apply` for 0016/0008 transactions (conditional DEFAULT); failover drill F-1..F-7 (all EMPTY); P12-E-1 registered OPEN |
| `docs/phases-11-20/phase-12-backup-dr-resilience/records/06_p12_06_pgbouncer_introduction.md` | P12.06 — transaction-mode pooling design consuming P11.19; SQLx prepared-statement validation matrix V-1..V-5 (UNRUN); C-POOL-1 expected-path citation |
| `docs/phases-11-20/phase-12-backup-dr-resilience/records/19_phase_12_wave1_agent_a_close_out.md` | this record |
| `deploy/compose/backup/README.md` | P12.02 — tree documentation (posture, secrets discipline, defect expectation) |
| `deploy/compose/backup/pgbackrest.conf` | P12.02 — stanza config shape (all site cells `<PLACEHOLDER>`; option names UNVERIFIED [S-68]) |
| `deploy/compose/backup/backup_postgres.sh` | P12.02 — POSIX-sh one-shot backup runner; outcome line records exit status, never restorability |
| `deploy/compose/backup/verify_backup.sh` | P12.02 — `check` + `info` recording; carries the not-a-restore rule in its header |
| `deploy/compose/backup/jetstream_snapshot.sh` | P12.02 — per-stream snapshots (default: the four Phase 1 provisioned streams per `topology.rs`); performs NO restore; CLI surface UNVERIFIED [S-65] |
| `deploy/compose/backup/snapshot_manifest.py` | P12.02 — stdlib-only deterministic sha256 manifest builder/verifier, fail-closed |
| `deploy/compose/backup/test_snapshot_manifest.py` | P12.02 — complete stdlib unittest suite, written-unrun (execution is the human gate) |

Files modified: none (no shared file touched; `deploy/compose/docker-compose.yml` intentionally NOT edited — no edit was required).

## 2. Decision records

| ID | Record | Status |
|---|---|---|
| P12-E-2 (JetStream backup posture) | `02_p12_02_backup_procedures.md` §4 | **OPEN** with the DEFAULT (snapshot-based stream backup into the shared object-storage target; `num_replicas: 1` unchanged, D-J5 holds; replication revisited at the Phase 20 second-cluster trigger). LR-210 is addressed AT RECORD LEVEL only; the human owner closes it with P12.14 drill evidence. |
| P12-E-1 (standby topology + sync level) | `05_p12_05_standby_replication.md` §4 | **OPEN** with the DEFAULT (one Patroni primary + one standby; `remote_apply` for 0016/0008 transactions IF the measured commit-latency cost is accepted; `remote_write` the documented fallback; DEPLOYMENT-gated). |
| P12-E-3 (DR site necessity) | NOT this agent's registration surface — it is the wave-2/P12.16 register entry | Untouched; the ledger table already carries it as OPEN |

No E-decision was closed by this agent (conventions §4).

## 3. Cross-agent handoff notes

**To Agent B (P12.07–P12.12, records 21–39):**

1. The C-POOL-1 pool-exhaustion experiment may cite `06_p12_06_pgbouncer_introduction.md` §2–§3 by its landed path (the design and the V-matrix exist).
2. The chaos experiment scripts and runbooks may cite the P12.02 procedure shape by expected path — now landed at `deploy/compose/backup/` (files `backup_postgres.sh`, `verify_backup.sh`, `jetstream_snapshot.sh`, `snapshot_manifest.py`).
3. The P12.02 manifests are integrity evidence only (sha256, not signatures); do not promote them into signing claims.
4. `deploy/compose/docker-compose.yml` was not modified; no coordination conflict exists on the shared-adjacent file.

**To wave 2 (P12.13–P12.16, records 07–18 reserved):**

1. The P12.13 evidence templates (expected path `runbooks/evidence_templates.md`) carry: the P12.03 drill skeleton (§5), the P12.05 F-1..F-7 evidence columns, the X-8 pre-registration rule for every threshold (P12.03 §4 item 5; P12.05 §5 preamble), and the RECOMMENDATION-labeled RTO/RPO cells from `04_p12_04_rto_rpo_register.md` §3 (plan §2.1 dependency).
2. P12.15's acceptance criteria should consume the §13.3 statements already placed in records 05 §6 and 06 §6.
3. P12.16's consistency audit should verify the cadence consistency triangle: P12.01 §3/§5 ↔ P12.04 §3 cadence column ↔ P12-E-2's snapshot cadence.
4. Records 07–18 remain unassigned in my range except where wave 2 names them; I wrote exactly 01–06 plus this close-out at 19 per the "last number in your range" instruction.

**Known expectations to carry forward (not defects yet — first-execution candidates):** the `nats stream backup` CLI surface [S-65] and the pgBackRest option names [S-68] are UNVERIFIED; the sqlx statement-cache option name is an ASSUMPTION (P11.19-carried); PostgreSQL 18 remains an UNVERIFIED pin (P10.13).

## 4. Permitted verification — verbatim outputs

All three permitted tools were run from the repository root after the
final edits. Outputs quoted verbatim (the non-ASCII character in the
third output is the tool's own byte output, reproduced as-is):

```text
$ python tools/verify_docs.py
report: docs\plan\Neryva_FlexCore_Product_and_Architecture_Report_v1.1.md
lines: 2241  sections: 28  headings: 127  tables: 32
OK: headings, section references, fences, tables, references, TOC

$ python tools/validate_agent.py
skills: 5  roles: 5
OK: skills, roles, and README index are valid

$ python -I tools/check_dependency_rule.py
OK ù dependency rule holds for 20 workspace members.
```

All three exit codes were 0. No failure is reported by any tool; per the
honesty rules this states the tools' own output, not a claim about the
unrun scripts or undeployed designs.

## 5. Deviations and honest notes

| # | Item | Disposition |
|---|---|---|
| 1 | No tick was applied to any `ledger_phase_12.md` checkbox | Per the hard bans — tick flips are not a wave-1 agent act |
| 2 | Record numbers 07–18 were left unwritten | The subtask instruction assigns one record per subtask (01–06) and the close-out at the last number of the range (19); wave 2 owns the remaining numbers |
| 3 | The P12.03/P12.06 sibling citations (P12.13 templates; C-POOL-1) use the expected-path pattern and are marked not-yet-written | Plan §2.1's pattern, applied as instructed |
| 4 | `verify_docs.py`/`validate_agent.py` check the report and the harness, not phase records | Their output is quoted as the permitted verification gate requires; record-level correctness is enforced by this record's per-record reviewer procedures and the human gate |
| 5 | The `check_dependency_rule.py` output contains a non-ASCII byte rendered as `ù` in this shell | Quoted verbatim; treated as cosmetic encoding of the tool's separator |
| 6 | No backup, restore, snapshot, measurement, or fault injection exists | The core honesty statement of the whole wave |

Status: **WAVE 1 WRITTEN-UNRUN** — the wave's deliverables are complete as
written artifacts; every execution act they describe is a human gate that
has not happened and is not scheduled.
