<!-- P12.14 — JetStream data-loss rehearsal design (stream delete +
     restore-from-snapshot drill). DRILL DESIGN ONLY: no stream was deleted,
     no snapshot exists, no restore was performed, no timing was measured.
     Mirrors the Postgres drill-record structure (records/03). Executes the
     P12-E-2 DEFAULT posture (snapshot-based stream backup — records/02 §4,
     still OPEN); the dependence is recorded, the decision is NOT closed by
     this record. Restore authorization is inherited from P10.11 §4.4
     (ledger/phase-10-v1-release-hardening/11_backup_restore_rc.md) — cited,
     never re-decided. Every nats CLI claim is UNVERIFIED ([S-65]; the
     `nats stream backup` surface cited by jetstream_snapshot.sh has never
     run in this program, and no restore-command shape is asserted at all). -->
# P12.14 — JetStream Data-Loss Rehearsal Design (stream delete + restore-from-snapshot — plan only, all evidence EMPTY)

| Field | Value |
|---|---|
| Subtask | P12.14 — JetStream data-loss rehearsal design (`ledger_phase_12.md` §P12.14) |
| Wave / agent | Phase 12 wave 2, Agent A |
| Status | **DRILL DESIGN — WRITTEN-UNRUN.** No stream has been deleted, no snapshot exists to restore, no restore has been performed, no loss window has been measured, no refusal transcript exists. Every evidence cell in §6 is EMPTY/UNRUN. |
| Sources | P12-E-2 (`records/02_p12_02_backup_procedures.md` §4 — the snapshot DEFAULT posture, OPEN); [S-65] (NATS JetStream docs — the snapshot CLI surface, UNVERIFIED); P10.11 §4.4 (`ledger/phase-10-v1-release-hardening/11_backup_restore_rc.md` — scoped restore authorization + the CVE-2026-33222 fix-version rule, inherited); P10.11 §4.2 (the restored-stream-config-vs-P1.14-design check, inherited); P9.24 §5 (the Postgres drill structure this design mirrors — `ledger/phase-09-pilot-qualification/24_backup_dr_rehearsal.md`, cited not rewritten); P9.24 §11 R-P9.24-7 (the DB-vs-stream loss-window gap — the gap this drill measures); `deploy/compose/backup/jetstream_snapshot.sh` (the wave-1 snapshot procedure — WRITTEN-UNRUN); `deploy/compose/backup/snapshot_manifest.py` (integrity manifest); `crates/adapters/nats/src/topology.rs` (stream inventory, D-J5/J7); C-NATS-1 (`experiments/c_nats_1_nats_outage.py` — the outage/recovery experiment whose oracle metrics this drill reuses); P12.04 §3 T2 row (the requirement cells the measured values are judged against); P12.13 (`runbooks/evidence_templates.md` — the X-8 pre-registration surface) |
| Files touched | this record only |

## 1. Honest status — the plain statement

**No JetStream stream has ever been deleted, no stream snapshot exists
anywhere, no stream restore has been performed, and no event-plane loss
window has ever been measured.** Stream configs are deployment artifacts
that do not exist (P9.24 §2.3); no NATS server runs in this program. This
record is a drill DESIGN that mirrors the Postgres restore-drill record
structure (records/03): honest status, a citation map, the drill-specific
validation steps, execution prerequisites, and an EMPTY timed-evidence
template. It exists to close the R-P9.24-7 gap honestly: the DB-only
restore drill (P12.03) measures the database, not the streams — the
event-plane gap between the last stream state and the restored database is
measurable ONLY by this drill, and only once a real deployment and a real
snapshot exist.

## 2. The posture the drill executes — P12-E-2 DEFAULT, dependence recorded

| Field | Value |
|---|---|
| Decision ID | **P12-E-2** (`records/02_p12_02_backup_procedures.md` §4; `ledger_phase_12.md` §Decision points) |
| Status at design time | **OPEN** — the human owner closes it with THIS drill's evidence (the closer named in record 02 §4) |
| What the drill executes | The **DEFAULT (RECOMMENDATION)** posture: snapshot-based stream backup into the same object-storage target as the PostgreSQL backups, `num_replicas: 1` unchanged (D-J5 holds; `topology.rs`) |
| Dependence recording | If P12-E-2 is still OPEN when the drill runs, the drill executes the DEFAULT and RECORDS THE DEPENDENCE in the drill metadata (§6) — the drill never invents a posture of its own, and its outcome is evidence FOR the decision, never the decision itself |
| The snapshot procedure executed | `deploy/compose/backup/jetstream_snapshot.sh` (wave-1, WRITTEN-UNRUN) — per-stream `nats stream backup` into `SNAPSHOT_ROOT/<stream>/<stamp>/` + a `snapshot_manifest.py` manifest per stream. That script PERFORMS NO RESTORE by design; the restore leg belongs to this drill design |

**CLI honesty (standing, non-negotiable for this drill):** the
`nats stream backup` command shape is cited from [S-65] and is **UNVERIFIED**
— it has never run in this program. No restore-command shape is asserted
ANYWHERE in this record: the restore leg's CLI surface is marked
`<UNVERIFIED — verify against the pinned nats CLI at first human execution>`
and the exact command the executor uses is recorded FROM the execution, not
named in advance. A CLI surface that differs from the citation is a defect
recorded against records/02 / this record, not an improvisation.

## 3. The drill — citation map (mirrors records/03's structure)

The step semantics follow the Postgres drill's shape (records/03 §2): a
citation map that binds each step to its authority without restating or
modifying criteria. The stream-specific additions (delete, scoped-restore
authorization, manifest verification, consumer resumption) are §4's named
surfaces.

| Drill step | Authority consumed | What this drill binds it to |
|---|---|---|
| JS-0 source-state record | the R-0 discipline (P9.24 §5 R-0, cited via records/03 §2) | per-stream message counts, consumer/ack state, the stream-config snapshot in force (vs `topology.rs`: `FLEXCORE_REGISTRY`, `FLEXCORE_STATE`, `FLEXCORE_AUDIT`, `FLEXCORE_DLQ` + the `FLEXCORE_DLQ_<AREA>` streams the P11.13 topology records) |
| JS-1 snapshot production | the P12-E-2 DEFAULT posture (record 02 §4) | `jetstream_snapshot.sh` run (or a snapshot already produced by the deployment's cadence); artifact IDs resolve into the P12.01 §4 `nats-snapshots/` prefix |
| JS-2 isolation check | P9.24 §5 R-1 (cited) | the drill environment shares NO state with anything facing production; the drill deletes and restores COPIES in isolation only |
| JS-3 restore-authorization scoping | **P10.11 §4.4** (inherited, §4 below) | the granted restore-authorization scope recorded BEFORE any restore exists; the NATS server version checked against the CVE-2026-33222 fix-version rule |
| JS-4 stream delete (the fault) | this drill's defining fault | the four Phase 1 streams deleted one at a time, in the isolated environment; the deletion is the data-loss event the drill rehearses |
| JS-5 restore from snapshot | the P12-E-2 DEFAULT posture + P10.11 §4.4 scoping | restore each stream from its snapshot artifact; exact CLI shape UNVERIFIED (§2) |
| JS-6 restored-config comparison | P10.11 §4.2 (inherited) | the restored stream configuration (subjects, retention, replication, acknowledgement, replay) compared against the recorded topology design; a mismatch is a FINDING (`STREAM_CONFIG_MISMATCH`), never tolerated drift |
| JS-7 content-integrity verification | `snapshot_manifest.py --verify` (wave-1) | the manifest verifies against the restored artifacts; sha256 is integrity EVIDENCE, not a signature (record 02 §2) |
| JS-8 consumer resumption | the C-NATS-1 oracle surface (cited, not re-specified) | durable consumers re-bind to the restored streams; redelivery after reconnect observed via `flexcore_command_consumer_duplicate_deliveries_total`; `flexcore_commands_stuck` read against its threshold |
| JS-9 measured restore time + loss window | the R-8 discipline (P9.24 §5 R-8, cited via records/03) | restore wall time per stream; the observed event-plane data-loss window (messages published after the snapshot instant, lost by the deletion) — from the run's clocks/logs/counts ONLY; this is the R-P9.24-7 measurement |
| JS-10 unauthorized-restore refusal probe | **P10.11 §4.4** (inherited, §4 below) | a restore attempt to an unauthorized stream name must be REFUSED; `RESTORE_AUTHORIZATION_UNSCOPED` is the finding if the refusal does not happen |
| JS-11 judgement vs requirements | the P9.24 §4 judgement rule (inherited via records/03 §5.2) | judged against the P12.04 §3 T2 cells IN FORCE at the drill date; a worse-than-requirement value is a FINDING, never a re-defined requirement |
| JS-12 teardown + destruction record | the R-10 discipline (P9.24 §5 R-10, cited via records/03) | restored copies destroyed; destruction recorded (residency per P9.10) |

## 4. Restore authorization — inherited from P10.11 §4.4 (cited, not re-decided)

The scope of this drill's restore authorization is the P10.11 §4.4
discipline, consumed unchanged
(`ledger/phase-10-v1-release-hardening/11_backup_restore_rc.md` §4.4):

1. **Scoped restore authorization only:** the drill's restore operates
   under authorization scoped to the named streams being rehearsed; a
   restore to an unauthorized stream name must be REFUSED.
2. **The refusal is evidence:** the observed refusal of an unauthorized
   stream-name restore is recorded (§6 JS-10 row); `RESTORE_AUTHORIZATION_UNSCOPED`
   is the finding if the refusal does not happen. No refusal evidence
   exists today — the cell is EMPTY.
3. **The CVE-2026-33222 fix-version rule is consumed, not re-verified:** the
   isolated environment's NATS server version is recorded and checked
   against the fix-version rule as P10.11 §4.4 records it (fix in NATS
   2.11.15 / 2.12.6; every 2.15.x release includes the fix — cited from
   that record's own sourcing; this record adds no independent version
   verification). The running rule stands: do not run a version affected by
   CVE-2026-33222 without the fix.
4. **No authorization is granted by this record.** The drill design names
   the scoping discipline; granting scoped restore authorization on a real
   deployment is a deployment/operator act (a human gate), never an agent
   act, and this record performs none.

## 5. Execution prerequisites (all must hold before the drill runs)

| # | Prerequisite | Status |
|---|---|---|
| 1 | A real deployment with NATS JetStream and the provisioned streams exists (stream configs are deployment artifacts — P9.24 §2.3) | NOT MET — none exists |
| 2 | A stream snapshot exists, produced by `jetstream_snapshot.sh` (or the deployment's cadence), with its manifest and resolvable artifact IDs | NOT MET |
| 3 | `jetstream_snapshot.sh` has executed at least once by the human executor (the script is WRITTEN-UNRUN; its first run may surface CLI-surface drift, [S-65]) | NOT MET |
| 4 | The scoped restore-authorization scope for the drill is granted and recorded by the operator (P10.11 §4.4 discipline) | NOT GRANTED — `<EMPTY>` |
| 5 | The isolated environment's NATS server version carries the CVE-2026-33222 fix per the P10.11 §4.4 rule | `<EMPTY — UNRUN>` |
| 6 | Any timing threshold the drill will judge against is pre-registered (X-8) in the P12.13 template (`runbooks/evidence_templates.md` §1) BEFORE the run | Template landed; the register cells are EMPTY (correct state — registration happens before a real run, by the human executor) |
| 7 | The P12.04 §3 T2 requirement cells in force at the drill date are filled/signed (an un-judged drill measures against nothing — the P9.24 §11 R-P9.24-2 finding applies) | `<EMPTY>` |

## 6. Timed-evidence template — ALL CELLS EMPTY (UNRUN)

Filled ONLY from a real run's clocks, logs, and counts; a filled cell
without a run is a fabrication defect (the P9.24 §8 rule, inherited). The
X-8 pre-registration header lives in the P12.13 template
(`runbooks/evidence_templates.md` §1), which also carries this drill's
threshold-register row.

**Drill metadata (all EMPTY):** dates / executor / witness / isolated-environment
ID / snapshot artifact IDs per stream / manifest IDs / NATS server version +
CVE-2026-33222 fix-status check / restore-authorization scope granted /
P12-E-2 status at drill time (DEFAULT executed under OPEN — the dependence
record) / stream-config baseline (the JS-0 record) / requirement row-version
in force.

| Step | Result | Measured value | Pass/fail | Disposition |
|---|---|---|---|---|
| JS-0 source-state record | (EMPTY — UNRUN) | per-stream counts `<EMPTY>` | | |
| JS-1 snapshot production/verification | (EMPTY — UNRUN) | snapshot artifacts `<EMPTY>` | | |
| JS-2 isolation check | (EMPTY — UNRUN) | | | |
| JS-3 authorization scope + NATS fix-version check | (EMPTY — UNRUN) | scope + version `<EMPTY>` | | |
| JS-4 stream delete (per stream) | (EMPTY — UNRUN) | deletion timestamps `<EMPTY>` | | |
| JS-5 restore from snapshot (per stream) | (EMPTY — UNRUN) | restore wall time `<EMPTY>` per stream | | |
| JS-6 restored-config vs recorded topology | (EMPTY — UNRUN) | comparison transcript `<EMPTY>` | | |
| JS-7 manifest verification on restored artifacts | (EMPTY — UNRUN) | verify outcome `<EMPTY>` | | |
| JS-8 consumer resumption + redelivery | (EMPTY — UNRUN) | duplicate-deliveries delta `<EMPTY>` | | |
| JS-9 measured loss window (R-P9.24-7) | (EMPTY — UNRUN) | event-plane loss window `<EMPTY>` | | |
| JS-10 unauthorized-restore refusal probe | (EMPTY — UNRUN) | refusal transcript `<EMPTY>` | | |
| JS-11 judgement vs P12.04 T2 cells in force | (EMPTY — UNRUN) | | | |
| JS-12 teardown + destruction record | (EMPTY — UNRUN) | | | |

## 7. Reviewer-verification procedure (human)

1. Confirm §1's plain statement and that EVERY cell of §6 is EMPTY/UNRUN;
   no restore time, loss window, refusal transcript, or config comparison
   is stated anywhere.
2. Confirm §2 records the P12-E-2 dependence exactly: the drill executes
   the DEFAULT under an OPEN decision and records the dependence in the
   drill metadata — it neither closes nor re-opens the decision.
3. Confirm NO restore-command CLI shape is asserted anywhere in this
   record; the restore surface is marked UNVERIFIED and the exact command
   is to be recorded FROM the execution.
4. Confirm §4 cites P10.11 §4.4 unchanged (scoped authorization, the
   refusal finding name, the fix-version rule) without re-deciding any
   criterion, and that §3's JS-6 check cites P10.11 §4.2.
5. Confirm §5 marks every prerequisite's real status and that prerequisite
   6 states the X-8 pre-registration obligation before any threshold
   exists.
6. Confirm the C-NATS-1 oracle surface is cited (not re-specified) and that
   `crates/adapters/nats/src/topology.rs` names match the record.
7. Confirm no checkbox in `ledger_phase_12.md` was touched by this subtask.

## 8. Filling commands (for the human executor)

```sh
# (a) the documented-tools gate (run after ANY edit to this record):
python tools/verify_docs.py
python tools/validate_agent.py
python -I tools/check_dependency_rule.py

# (b) Pre-register any timing threshold (X-8) in the P12.13 template
#     BEFORE the run that could fill it.
# (c) On a real deployment, with prerequisites §5.1–§5.7 met: produce the
#     snapshot (jetstream_snapshot.sh), run the §3 drill (JS-0..JS-12) in
#     the ISOLATED environment, fill §6 FROM THE RUN ONLY.
# (d) Record the exact nats CLI restore command used (the surface is
#     UNVERIFIED until then); record CLI drift as a defect against
#     records/02 / this record.
# (e) Judge per the inherited P9.24 §4 rule; hand the evidence to the
#     P12-E-2 closure decision (the human owner's act) and to the P12.16
#     consistency audit.
```

## 9. Non-claims

No stream was deleted; no snapshot exists; no restore was performed; no
loss window was measured; no refusal was observed; no CLI command shape is
verified; no consumer behaviour was observed. A single run's outcome is one
run's outcome, never a service-level guarantee (§13.3 semantics;
conventions §5). This drill rehearses data-plane survivability ONLY: it
adds no dispatch authority, grants no permission, and touches no command
lifecycle gate (ADR-010; the never-granted permission set is untouched).
The drill deletes and restores stream state in an isolated lab environment
under simulation-only data — nothing here authorizes deletion or restore
against any deployment facing real traffic.

Status: **WRITTEN-UNRUN** — a drill design cannot delete a stream or
restore one by being written; the deployment, the snapshot, the scoped
authorization, and the drill are the human gates and have not happened and
are not scheduled.
