<!-- P20.12 — Backups at scale (retention-tier STRUCTURE per data class with
     EMPTY periods; restore-time objectives for large partitioned tables;
     backup verification cadence — the SCALE layer only). The P12
     backup/restore DESIGN is consumed by citation (P12.01–P12.03,
     LR-204/LR-205) and is never restated. Nothing was backed up, restored,
     timed, or scheduled; every objective and period cell is EMPTY. -->
# P20.12 — Backups at Scale (the SCALE layer only; P12 design consumed by citation; every objective cell EMPTY; nothing executed)

| Field | Value |
|---|---|
| Subtask | P20.12 — Backups at scale (`ledger_phase_20.md` §P20.12) |
| Wave / agent | Phase 20 wave 1, Agent B |
| Status | **SCALE-LAYER DESIGN RECORD — WRITTEN-UNRUN.** No backup was taken, no restore was run, no timing exists, no retention period is set, no verification job was scheduled or executed. Every objective, period, and cadence-value cell is EMPTY or an explicitly labeled structure with EMPTY value cells. |
| Consumed by citation (never restated) | P12.01 backup architecture (`docs/phases-11-20/phase-12-backup-dr-resilience/records/01_p12_01_backup_architecture.md` — pgBackRest DEFAULT, the §5 per-data-class schedule STRUCTURE with EMPTY periods); P12.02 procedures (`records/02_p12_02_backup_procedures.md`; artifacts `deploy/compose/backup/`); P12.03 restore drill runbook (`records/03_p12_03_restore_drill_runbook.md` — the restore-into-isolation discipline); P12.04 RTO/RPO register (`records/04_p12_04_rto_rpo_register.md` — RECOMMENDATIONS, sign-off EMPTY); P12.05 standby design (`records/05_p12_05_standby_replication.md` — standby-is-not-a-backup); LR-204 (a backup configuration that has never been restored) and LR-205 (RPO/RTO unsettable — no customer, D1 OPEN) per `ledger/phase-10-v1-release-hardening/17_operational_risk_review.md` §3 |
| Sources | [S-68] (PostgreSQL backup tools comparison — the same secondary source P12.01 cites; all tool-capability claims below inherit P12.01's UNVERIFIED-at-deployment markers); `crates/adapters/postgres/migrations/0010_telemetry_storage_p2_06.up.sql` (the monthly partitioning and D-P1..D-P5 decisions); `crates/adapters/postgres/src/partition_maintenance.rs` (the P2.20 detach/archive path) |
| Files touched | this record only |

## 1. Honest status — the plain statement

**No backup exists, no restore has ever been performed (LR-204 OPEN), and
no scale-level timing has ever been measured.** This record adds ONLY the
scale layer on top of the Phase 12 design — the retention-tier STRUCTURE
per data class (§2, periods EMPTY exactly as P12.01 §5 left them), the
restore-time-objective STRUCTURE for large partitioned tables (§3, every
timing EMPTY), and the backup-verification cadence STRUCTURE (§4). Where
this record says "tier", the value of each period lives in the P12.01 §5
/ P9.10 §6 cells and stays EMPTY while D1 is OPEN. Restating the P12
design — tool selection, procedures, drill steps, RTO/RPO recommendations
— would be a duplication finding; every such element is cited by path.

## 2. Retention tiers per data class — the scale STRUCTURE (periods EMPTY)

The tier vocabulary below is a scale-layer classification for operational
handling (backup copy count, storage tier, restore priority); it does NOT
introduce retention periods and never overrides the P12.01 §5 /
P13.14 registry basis rule:

| Tier | Data classes (P12.01 §5 rows, cited) | Scale-layer handling | Retention period |
|---|---|---|---|
| Tier E — evidence | `audit_records` (0008), audit chain (0019), command history (0016), edge enrollment journal (0018) | Highest restore priority; immutable-object storage is the P12.01 §4 RECOMMENDATION (mechanism cell EMPTY); verify-first cadence (§4) | `<EMPTY — P12.01 §5 / signed registry row governs>` |
| Tier L — lifecycle | Registry (0002–0005), asset_state (0006), outbox (0007), schedules/envelopes/feasibility (0013–0017) | Restores atomically with every DB snapshot (P12.01 §5, cited) | `<EMPTY>` |
| Tier T — telemetry | `telemetry` (0010/0012 monthly partitions), `telemetry_quarantine` | Partition-aware handling (§3); detach/archive via the P2.20 approved-operation path is the deletion mechanism (0010 D-P5, cited) — backup retention is a deployment decision, never the DDL's | `<EMPTY>` |
| Tier M — market | Market records (the P16 market-record seams — cited for interplay; the class lands in the registry's `market_records` data class) | Same DB-snapshot discipline as Tier L; any settlement-relevant completeness check is the P16 record's to define | `<EMPTY>` |
| Tier O — observability | Metrics store, traces, logs (the P11 store topology) | Best-effort class — observability data loss is not a dispatch-safety item (the P12.04 T7 posture, cited); no backup designed | n/a |
| Tier S — stream state | JetStream streams (`crates/adapters/nats/src/topology.rs`) | Per-stream snapshots into the shared object-storage target — the P12-E-2 DEFAULT posture, consumed by citation | `<EMPTY>` |

Rules: (1) no period anywhere — D1 OPEN, P9.10 §6; (2) no tier redefines
the P12.01 §5 rows — it classifies them for scale handling; (3) Tier M's
completeness obligations are the P16 records' — this record claims none.

## 3. Restore-time objectives for large partitioned tables — STRUCTURE, every timing EMPTY

The scale problem this section structures: the `telemetry` table is
PARTITION BY RANGE (event_time) with monthly partitions (0010 D-P1..D-P3),
so a full-table restore grows with retained history while most operational
restores need only recent partitions. Every timing below is `<EMPTY —
UNMEASURED>`; the first measurement is the human-gated drill (P12.03 /
P10.11 RC restore), never this record.

### 3.1 Restore-into-isolation timing structure

| Scenario (defined now, measured later) | What the drill measures | Objective value |
|---|---|---|
| RT-1 Full cluster restore-into-isolation (the P12.03/P9.24 §5 discipline, cited) including all retained telemetry partitions | Wall-clock from restore start to post-restore verification PASS (the P10.11 §4 checks — command/audit completeness, outbox/inbox idempotency, schema match — consumed by citation) | `<EMPTY — UNMEASURED>` |
| RT-2 Recent-partition restore: restore to a point in time (PITR from base + WAL, the P12.01 §3 cadence row) where the recovery window covers only recent monthly partitions | Wall-clock to verification PASS | `<EMPTY — UNMEASURED>` |
| RT-3 Verification cost as a function of retained partition count (the honest scaling question) | Measured verification step durations vs partition count — recorded as observations, never as a scaling law (the P3.27 no-scaling-law discipline, carried) | `<EMPTY — UNMEASURED>` |

No restore-time OBJECTIVE (a requirement) is set — that is the P12.04
register's signed-cell act (LR-205 OPEN). This section structures WHAT a
future drill measures; it sets no requirement and asserts no capability.

### 3.2 Partition-level restore options — options, all UNVERIFIED

| Option | Mechanism class | Status |
|---|---|---|
| PO-1 PITR to a point in time | Full-cluster restore from base + WAL to the target moment (the P12.01 §3 mechanism); recent partitions arrive with the cluster | UNVERIFIED [S-68] — the same tool-capability class P12.01 carries; not exercised |
| PO-2 Parallel restore of a base backup | pgBackRest-class parallel restore (`process-max` per the P12.01 §2 UNVERIFIED capability row, cited — not re-claimed) on restored-into-isolation hardware | UNVERIFIED [S-68]; timing EMPTY |
| PO-3 Selective partition detach/attach from a parallel-restored copy | Restore a scratch cluster, `DETACH PARTITION` / attach the needed monthly partitions into the recovery target (PostgreSQL partition DDL mechanics) | Mechanism-class description only; never exercised; the grants/RLS/trigger properties must re-verify after any attach (the P12.05 F-3-class inspection discipline, consumed) |
| PO-4 Archive-first recovery from the P2.20 detach path | The detach-then-archive operation (0010 D-P5; P2.20) already produces per-partition archives; recovery re-attaches from archive copies | UNVERIFIED; depends on the deployment having run the archive path |

All four are recorded so a future drill picks and measures, not
improvises; none is endorsed. PO-3/PO-4's honesty note: a selectively
restored telemetry set is NOT a point-in-time-consistent cluster unless
the drill proves the boundary partitions complete — the drill's
verification step exists exactly to catch that.

## 4. Backup verification cadence — the SCALE layer only (structure; values EMPTY)

The P12 design owns the verification SEMANTICS by citation: the
`pgbackrest check` verify job is explicitly NOT a restore (P12.01 §3),
and the restore drill (P12.03) is the only evidence that closes LR-204.
The scale layer adds the CADENCE STRUCTURE — how often each verification
class runs as the deployment grows; every value cell is a deployment
decision `<EMPTY>`:

| Verification class | What it is (cited) | Scale-layer cadence | Value |
|---|---|---|---|
| V-1 Tool self-check | `pgbackrest check` against the live stanza (P12.01 §3 — explicitly NOT a restore) | scheduled job | `<EMPTY — deployment decision>` |
| V-2 Snapshot manifest verification | The P12.02 integrity-manifest check (`snapshot_manifest.py` / `verify_backup.sh` — written-unrun artifacts, cited) | per snapshot batch | `<EMPTY>` |
| V-3 Restore drill | The P12.03 runbook's restore-into-isolation (LR-204's only closer) | drill cadence — STRUCTURE: full restore (RT-1 class) at least when the backup configuration or migration set changes; recent-partition timing (RT-2) on a recorded recurring cadence | `<EMPTY>` |
| V-4 Scale re-baseline | Re-run RT-1..RT-3 after any change in retained-partition count class (e.g. first time retention spans a materially larger history) | event-triggered | `<EMPTY>` |

The anti-silent-growth rule carries: a cadence change is a recorded
decision, never a silent edit of this table.

## 5. What this record does NOT establish

1. No backup, restore, timing, cadence value, or objective exists — every
   value cell is EMPTY; nothing was executed.
2. It does not restate the P12 design: the tool selection (P12.01 §2),
   procedures (P12.02), drill steps (P12.03), and RTO/RPO
   recommendations (P12.04) are consumed by path and remain those
   records' property.
3. It sets no retention period (D1 OPEN; P9.10 §6; P12.01 §5) and no
   restore-time requirement (LR-205 OPEN; the P12.04 sign-off cells).
4. It adds no dispatch authority (ADR-010) and touches no code, no
   migration, and no backup script (hard bans 6/7).
5. §13.3 semantics: a future passing drill is a tested-day outcome on a
   finite dataset, never a guarantee.

## 6. Reviewer-verification procedure (human)

1. Confirm §1's plain statement and that every value cell in §2–§4 is
   EMPTY, with LR-204/LR-205 carried as OPEN.
2. Confirm the consumption is by citation: trace §2's data classes to
   P12.01 §5 rows (not copies), §3's discipline to the P12.03 runbook,
   and §4's verification semantics to P12.01 §3 / P12.02 artifacts.
3. Confirm §3.2 marks every option UNVERIFIED and that no option is
   endorsed; confirm PO-3's attach re-verification note.
4. Confirm no ledger checkbox was ticked by this agent and no file
   outside this record was touched.

## 7. Findings and assumptions

| # | Item | Note |
|---|---|---|
| A-P20.12-1 | The tier vocabulary is this record's own scale-layer classification | No external source; it classifies recorded P12.01 §5 classes and adds no period or capability |
| A-P20.12-2 | All tool-capability mentions inherit P12.01's UNVERIFIED [S-68] markers | This record re-claims nothing; the markers are cited, not re-derived |
| F-P20.12-1 | No retention period can exist while D1 is OPEN, so the tier table's periods are EMPTY by necessity, not by choice | Carried; the P13.14 signed-registry row is the future filling mechanism (per-tenant, jurisdiction-qualified) |
| F-P20.12-2 | Restore timings for large partitioned tables have never been measured and cannot be until a real backup and a real restore exist (LR-204 OPEN) | Binding honesty statement |

Status: **WRITTEN-UNRUN** — a scale layer cannot back up or restore by
being written; the first backup, the first restore drill, and every
cadence decision are human gates that have not happened and are not
scheduled.
