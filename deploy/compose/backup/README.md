<!-- P12.02 — Backup procedure scripts + configs (WRITTEN-UNRUN tree). No
     script here has ever run; no backup exists; no tool is installed in the
     authoring loop. The first human execution may surface defects — that is
     the recorded expectation, not an exception. -->

# Backup procedures — `deploy/compose/backup/` (WRITTEN-UNRUN, P12.02)

This tree holds the WRITTEN-UNRUN backup procedure artifacts that the
P12.02 subtask designs: backup, verification, and JetStream snapshot
scripts plus the pgBackRest stanza config. **Nothing here has ever run; no
backup exists; no tool is installed by this program** (the loop bans forbid
installs and docker commands — `01_PROGRAM_CONVENTIONS.md` §1). The first
human execution is the gate and may surface defects; record them, do not
assume the scripts are correct.

| File | Purpose | Status |
|---|---|---|
| `pgbackrest.conf` | Stanza config shape (S3-compatible repo, WAL archiving) — option values are `<PLACEHOLDER>`s; option names are UNVERIFIED against the deployed pgBackRest version [S-68] | WRITTEN-UNRUN |
| `backup_postgres.sh` | Runs a pgBackRest backup (full / diff / incr, env-selected) and records an outcome line | WRITTEN-UNRUN |
| `verify_backup.sh` | Runs the stanza `check` + records `info --output=json` — **a verify job is NOT a restore** (P9.24 §3 last row; LR-204) | WRITTEN-UNRUN |
| `jetstream_snapshot.sh` | Per-stream JetStream snapshots (the P12-E-2 DEFAULT posture) + integrity manifest | WRITTEN-UNRUN |
| `snapshot_manifest.py` | Stdlib-only manifest builder/verifier (sha256, deterministic JSON) | WRITTEN-UNRUN |
| `test_snapshot_manifest.py` | Complete stdlib `unittest` suite for the manifest tool — written, NOT executed (execution is a human gate) | WRITTEN-UNRUN |

## Posture decisions this tree implements

1. **PostgreSQL:** base backup + continuous WAL archiving via pgBackRest
   (the P12.01 DEFAULT, `records/01_p12_01_backup_architecture.md` §2) —
   PITR-capable, S3-compatible object-storage target.
2. **JetStream:** snapshot-based stream backup into the SAME object-storage
   target as the PostgreSQL backups — registered as **P12-E-2, OPEN** with
   that DEFAULT (`records/02_p12_02_backup_procedures.md` §4). It closes the
   LR-210 decision point AT RECORD LEVEL only: the decision itself stays
   OPEN for the human owner, with P12.14 drill evidence as the closing
   input. `num_replicas: 1` is unchanged at design time (D-J5 holds —
   clustering stays a deployment decision).
3. **Edge journals:** no separate backup in the default posture (P12.01 §3
   edge-store row) — the journals are bounded self-recovering stores whose
   platform copies exist once uploaded (P9.24 §2.2).
4. **Cadence:** the per-tier backup cadence each script is run at comes
   from the P12.04 register (RECOMMENDATIONS pending operator sign-off) —
   this tree hard-codes no cadence and no schedule.

## Secrets discipline (P11.16)

No secret value appears in any file in this tree. The pgBackRest S3
credentials and repo cipher passphrase are deployment secret-management
inputs (the Compose file-per-host floor under `../secrets/`, the Vault
DEFAULT beyond it). A committed credential value is a defect.

## Defect expectation

Scripts are written-unrun: the first real run may surface path, quoting,
version, or CLI-surface defects (the `nats` CLI surface in
`jetstream_snapshot.sh` is UNVERIFIED — [S-65]; the pgBackRest option names
are UNVERIFIED against the pinned version — [S-68]). The human executor
records every such defect against the affected record before re-running.

## Records

- `docs/phases-11-20/phase-12-backup-dr-resilience/records/01_p12_01_backup_architecture.md` (selection + target shape)
- `docs/phases-11-20/phase-12-backup-dr-resilience/records/02_p12_02_backup_procedures.md` (this tree's record; P12-E-2)
- `docs/phases-11-20/phase-12-backup-dr-resilience/records/03_p12_03_restore_drill_runbook.md` (the drill that makes a backup credible)
