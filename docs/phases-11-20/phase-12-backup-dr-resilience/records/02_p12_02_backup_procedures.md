<!-- P12.02 — Backup procedure scripts + configs (WRITTEN-UNRUN). No script
     has run, no backup exists, no snapshot exists, no tool is installed.
     The JetStream posture closes the LR-210 decision point AT RECORD LEVEL
     ONLY: P12-E-2 is registered OPEN with its DEFAULT — the decision itself
     stays with the human owner (P12.14 drill evidence is the closing
     input). Scripts are complete, stdlib-only Python and POSIX-sh; no
     docker-compose.yml edit was made by this subtask. -->
# P12.02 — Backup Procedure Scripts + Configs (WRITTEN-UNRUN — nothing has run)

| Field | Value |
|---|---|
| Subtask | P12.02 — Backup procedure scripts + configs (`ledger_phase_12.md` §P12.02) |
| Wave / agent | Phase 12 wave 1, Agent A |
| Status | **WRITTEN-UNRUN.** No script has executed, no backup exists, no JetStream snapshot exists, no manifest exists. First execution is a human gate and may surface defects (recorded expectation, §6). |
| Sources | [S-65] (NATS JetStream docs — the snapshot CLI surface, UNVERIFIED); [S-68] (pgBackRest option surface, UNVERIFIED); P9.24 §2/§3 (store inventory, decision template); P10.11 §4.4 (scoped restore authorization — inherited, not restated); `crates/adapters/nats/src/topology.rs` (P1.14 §4.2 stream inventory, D-J5); `crates/adapters/postgres/migrations/0010_telemetry_storage_p2_06.up.sql` (partitioning — the D-P5 no-retention rule); `docs/phases-11-20/phase-12-backup-dr-resilience/records/01_p12_01_backup_architecture.md` (P12.01 selection) |
| Files touched | `deploy/compose/backup/` (new tree, owned by this subtask): `README.md`, `pgbackrest.conf`, `backup_postgres.sh`, `verify_backup.sh`, `jetstream_snapshot.sh`, `snapshot_manifest.py`, `test_snapshot_manifest.py`; this record. `deploy/compose/docker-compose.yml` was NOT edited. |

## 1. Honest status — the plain statement

**No backup procedure has ever run.** This record delivers the
WRITTEN-UNRUN procedure artifacts the subtask names: a PostgreSQL backup
script, a verification job, a JetStream snapshot script, a stdlib-only
manifest tool with its complete unittest suite, and the stanza-config
shape. Every artifact carries its honest label in-file. Three standing
rules govern everything here:

1. **A verify job is NOT a restore** (P9.24 §3 last row): `verify_backup.sh`
   passing establishes a configuration outcome, never restorability.
   Restorability is established only by the P12.03 drill.
2. **No schedule is baked in:** the cadence each script runs at comes from
   the P12.04 register (RECOMMENDATIONS, sign-off EMPTY); the deployment
   scheduler owns the wiring.
3. **No secret value in any file** (P11.16): credentials and the repo
   cipher passphrase are deployment secret-management inputs.

## 2. The `deploy/compose/backup/` tree

| File | Role | Honest labels |
|---|---|---|
| `README.md` | Tree documentation: posture, secrets discipline, defect expectation | WRITTEN-UNRUN |
| `pgbackrest.conf` | Stanza config SHAPE: S3 repo, WAL archive, retention/parallel/process cells `<PLACEHOLDER>`; option names UNVERIFIED [S-68]; no cadence; no secrets | WRITTEN-UNRUN |
| `backup_postgres.sh` | One pgBackRest backup run (full/diff/incr, env-selected); appends a timestamped outcome line with the exit status; exit status is the backup's status — never a restorability claim | WRITTEN-UNRUN |
| `verify_backup.sh` | `check` + `info --output=json` recording; states the not-a-restore rule in its header; fails loudly if the record cannot be written | WRITTEN-UNRUN |
| `jetstream_snapshot.sh` | Per-stream snapshots (default: the four Phase 1 provisioned streams `FLEXCORE_REGISTRY`, `FLEXCORE_STATE`, `FLEXCORE_AUDIT`, `FLEXCORE_DLQ` per `topology.rs`); builds a manifest per stream; performs NO restore | WRITTEN-UNRUN |
| `snapshot_manifest.py` | Stdlib-only deterministic manifest builder/verifier (sorted paths, sha256, size); `--verify` fails closed on missing/unexpected/mismatched files; sha256 is integrity evidence, NOT a signature | WRITTEN-UNRUN |
| `test_snapshot_manifest.py` | Complete stdlib `unittest` suite (build determinism, self-exclusion, tamper/missing/extra counting, fail-closed, CLI exit codes) — written, NOT executed | WRITTEN-UNRUN |

## 3. PostgreSQL procedure shape

- **Method (P12.01 DEFAULT):** pgBackRest base backup + continuous WAL
  archiving, S3-compatible repo, PITR-capable. The Compose dev stack keeps
  PostgreSQL data in the `postgres-data` volume; a real deployment names
  its own `pg1-path` `<PLACEHOLDER>`.
- **Server-side settings are deployment configuration, never repository
  files:** `wal_level=replica`, `archive_mode=on`,
  `archive_command = 'pgbackrest --stanza=flexcore archive-push %p'`
  (shape per [S-68], UNVERIFIED) — recorded in `pgbackrest.conf` comments;
  no postgresql.conf is added to the repository.
- **Partition awareness:** telemetry monthly partitions
  (`PARTITION BY RANGE (event_time)`, migration 0010) restore as part of
  the whole-database snapshot; the P2.20 maintenance path (detach-then-drop
  under the maintenance role) is the deletion mechanism, and backup
  retention is a deployment decision — the DDL carries no retention value
  (0010 D-P5, carried unchanged).
- **Role discipline:** backups run under the deployment's designated backup
  role (a `<PLACEHOLDER>`); the runtime `flexcore_app` role and the
  migrator/maintenance roles (P1.12 §3) are not backup operators.

## 4. JetStream snapshot posture — P12-E-2 registered OPEN with its DEFAULT

LR-210 (JetStream backup posture undecided — stream configs are deployment
artefacts that do not exist, P9.24 §2.3) is closed AT RECORD LEVEL by this
registration, exactly as the plan §2 assigns to this subtask. The decision
itself is NOT closed — agents never close E-decisions (conventions §4).

| Field | Value |
|---|---|
| Decision ID | **P12-E-2** (registered in `ledger_phase_12.md` §Decision points — OPEN) |
| Question | JetStream backup posture: stream snapshots vs clustering replication vs mirror on a second cluster |
| Options | (a) snapshot-based backup to object storage; (b) NATS clustering (`num_replicas > 1`); (c) mirror on a second cluster |
| **DEFAULT (RECOMMENDATION)** | **Snapshot-based stream backup into the SAME object-storage target as the PostgreSQL backups** (`jetstream_snapshot.sh` + the P12.01 §4 `nats-snapshots/` prefix); **`num_replicas: 1` unchanged at design time** — D-J5 holds (`topology.rs`: clustering is a deployment decision); replication is revisited at the second-cluster trigger (Phase 20 multi-region) |
| Closer | The human owner, with P12.14 drill evidence (the stream delete + restore-from-snapshot drill) |
| Status | **OPEN** |

What the DEFAULT does and does not claim:

- It bounds the event-plane loss window by the snapshot cadence — the
  R-P9.24-7 gap (an event-plane gap between the last stream state and the
  restored DB is a data-loss-window item the DB-only restore drill cannot
  measure) is made VISIBLE by this posture and measured only by the P12.14
  drill, never by this script.
- `jetstream_snapshot.sh` performs NO restore. Restore authorization is
  scoped per the inherited P10.11 §4.4 discipline (an unauthorized
  stream-name restore must be REFUSED; `RESTORE_AUTHORIZATION_UNSCOPED` is
  the finding if the refusal does not happen) — the P12.14 drill design
  owns that surface.
- The `nats stream backup` CLI surface is UNVERIFIED ([S-65]; never run in
  this program) — verify against the pinned CLI at first human execution.

## 5. Edge-store posture (the P9.24 §2.2 decision cell)

DEFAULT recorded in P12.01 §3 and implemented by omission here: the edge
journals (`buffer_file.rs`, `audit_buffer.rs`, the id-binding and enrollment
stores) are bounded, self-recovering stores whose platform copies exist
once uploaded (at-least-once, in-order); no separate edge-journal backup
script ships in this tree. The cell stays with the human owner; a
deployment that changes the upload discipline re-opens the posture.

## 6. Defect expectation (recorded, not hidden)

Scripts are WRITTEN-UNRUN. The first human execution may surface: path and
environment wiring mistakes; pgBackRest option-name drift against the
pinned version ([S-68] is a secondary source); `nats` CLI surface drift
([S-65]); manifest edge cases (permission failures, symlink behaviour,
very large snapshots); Compose-volume access constraints for the staging
root. Each such defect is recorded against this record (or record 01 for
selection-level issues) before the next run — the defect expectation is an
honesty obligation, not a disclaimer of correctness.

## 7. Reviewer-verification procedure (human)

1. Confirm §1's plain statement: no script has run; no backup, snapshot, or
   manifest artifact exists anywhere in the repository or its history.
2. Confirm `test_snapshot_manifest.py` is complete (no skipped bodies, no
   placeholder assertions) and carries the written-unrun label; execution
   is the human gate.
3. Confirm no file under `deploy/compose/backup/` contains a secret value,
   a real endpoint, or a real bucket name; confirm `pgbackrest.conf`
   carries `<PLACEHOLDER>` in every site cell.
4. Confirm `jetstream_snapshot.sh` performs no restore and states the
   scoped-restore-authorization inheritance (P10.11 §4.4); confirm the
   default stream set matches `crates/adapters/nats/src/topology.rs`.
5. Confirm `verify_backup.sh` states the not-a-restore rule (P9.24 §3 last
   row) in its own header.
6. Confirm §4 registers P12-E-2 as OPEN with the DEFAULT — and that the
   ledger's decision table still reads OPEN (no agent closes it).
7. Confirm no checkbox in `ledger_phase_12.md` was touched by this
   subtask.

## 8. Filling commands (for the human executor)

```sh
# (a) the documented-tools gate (run after ANY edit to this record):
python tools/verify_docs.py
python tools/validate_agent.py
python -I tools/check_dependency_rule.py

# (b) Execute the manifest test suite (the written-unrun human gate):
python -m unittest discover -s deploy/compose/backup -p "test_*.py" -v
#     (or run from the directory: python test_snapshot_manifest.py)

# (c) At deployment: install pgBackRest + the nats CLI (deployment act,
#     never this loop), fill pgbackrest.conf's authoritative copy, wire the
#     cadence from the signed P12.04 register, and run backup_postgres.sh /
#     verify_backup.sh / jetstream_snapshot.sh under the deployment's
#     secret management.
# (d) Establish restorability ONLY via the P12.03 drill (LR-204); establish
#     the stream posture via the P12.14 drill, then close P12-E-2.
```

## 9. Non-claims

No backup exists; no snapshot exists; no restore was performed; no timing
was measured; no CLI or option name is verified; no cadence is committed;
no capability or runtime-behaviour claim is made about any script. ADR-010
is untouched: these procedures add data-plane survivability tooling only —
no dispatch authority, no permission change, no control-plane behaviour
change.

Status: **WRITTEN-UNRUN** — procedure scripts cannot take a backup by being
written; the first backup, the first snapshot, and the first restore are
the human gates and have not happened and are not scheduled.
