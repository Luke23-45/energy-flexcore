<!-- P12.01 — Backup architecture design (pgBackRest vs WAL-G selection record).
     SELECTION RECORD ONLY: no tool is installed, no backup is taken, no bucket
     exists, nothing is written to any object store. Every external tool-
     capability claim cites [S-68] (a secondary comparison source) and is marked
     UNVERIFIED at deployment. The P9.24 §3 decision template is filled here as
     RECOMMENDATION / DEFAULT rows — the human owner closes those deployment
     decisions; agents never close them. Every retention period stays EMPTY per
     P9.10 §6 (no basis may be stated while D1 is OPEN). -->
# P12.01 — Backup Architecture Design (pgBackRest vs WAL-G selection — nothing installed, nothing backed up)

| Field | Value |
|---|---|
| Subtask | P12.01 — Backup architecture design (`ledger_phase_12.md` §P12.01) |
| Wave / agent | Phase 12 wave 1, Agent A |
| Status | **SELECTION + DESIGN RECORD — WRITTEN-UNRUN.** No backup tool is installed (conventions §1), no backup exists, no bucket exists, no cadence runs. Every tool-capability claim is sourced from [S-68] and marked UNVERIFIED at deployment. |
| Sources | [S-68] (PostgreSQL backup tools comparison — secondary source; the gap analysis §11 entry), [S-67] (Patroni replication modes — §5 cross-ref); `ledger/phase-09-pilot-qualification/24_backup_dr_rehearsal.md` (P9.24 — §3 decision template, §2 store inventory); `ledger/phase-09-pilot-qualification/10_data_residency_retention.md` (P9.10 — §6 retention table, every period EMPTY); `ledger/phase-10-v1-release-hardening/11_backup_restore_rc.md` (P10.11 — RC re-run plan, run record EMPTY); `docs/operations/deployment-upgrade-model.md` §6 |
| Files touched | this record only (procedure scripts/configs are P12.02's deliverable: `deploy/compose/backup/`) |

## 1. Honest status — the plain statement

**No backup tool is installed, no backup configuration exists, no backup has
been taken, and no bucket or object-storage target exists.** This record
delivers the SELECTION the plan assigns: the pgBackRest vs WAL-G comparison
with the DEFAULT recorded (§2), the P9.24 §3 decision-template rows filled
as RECOMMENDATION/DEFAULT entries (§3), the object-storage target shape
(§4), and the per-data-class retention/cadence schedule structure (§5) —
every period cell EMPTY per the P9.10 §6 no-basis-no-deploy rule. LR-204
(a backup configuration that has never been restored) is NOT closed by this
record: a selection record is not a backup, and a backup is not credible
until the P12.03 drill executes P9.24 §5 against a real deployment. The
capability claims in §2 are sourced from [S-68] — a secondary comparison —
and are marked UNVERIFIED; they are design inputs, not verified facts.

## 2. Tool selection — pgBackRest vs WAL-G (DEFAULT: pgBackRest)

The comparison is over the properties the P9.24 §3 template names (the
METHOD decision: base backup + WAL archiving, PITR, an S3-compatible
target, parallel and incremental backups). Both candidates are WAL-archiving
tools per [S-68]; neither is installed here and none of the rows below was
tested in this loop.

| Property (the P9.24 §3 method question) | pgBackRest | WAL-G | Status of the claim |
|---|---|---|---|
| Parallel backup/restore (multi-process) | Yes (process-max) [S-68] | Yes (parallel upload) [S-68] | UNVERIFIED — cited from a secondary comparison; verify at deployment |
| Incremental backups | Yes (diff/incr levels) [S-68] | Yes (delta backups) [S-68] | UNVERIFIED — as above |
| S3-compatible object-storage target | Native repo type `s3` [S-68] | Native storage layer [S-68] | UNVERIFIED — as above |
| PITR from base backup + archived WAL | Yes [S-68] | Yes [S-68] | UNVERIFIED — as above |
| Built-in verification (`check`) against the live stanza | Yes (`check` command) [S-68] | Verify via backup listing/restore test | UNVERIFIED — as above |
| Configuration footprint | Stanza-based config file | Env-var driven | UNVERIFIED — as above |

**Decision recorded: DEFAULT = pgBackRest.** Rationale (design-level, not a
verified performance claim): the single integrated stanza config + WAL
archive-push/restore-get lifecycle and the explicit `check` subcommand map
cleanly onto the P9.24 §3 verify-job row ("does the un-restored backup pass
its own checks?" — with the standing rule that a verify job is NOT a
restore, P9.24 §3 last row). WAL-G remains the documented alternative; the
selection is reversible at deployment with no artifact sunk cost (no tool
was installed). The human owner closes the tool decision together with the
P9.24 §3 cells; this record supplies the recommended default only
(conventions §4).

## 3. The P9.24 §3 decision template — filled as RECOMMENDATION / DEFAULT rows

The P9.24 §3 template is cited, not rewritten. The rows below carry this
record's RECOMMENDATION for each cell; the human owner fills the authoritative
cells in the deployment's copy of the P9.24 §3 template. **No row here is a
decision — the label column governs.**

| P9.24 §3 row | RECOMMENDATION (DEFAULT label) | Basis / status |
|---|---|---|
| Backup tool/method | pgBackRest, base backup + continuous WAL archiving (PITR-capable) | §2 above; UNVERIFIED capability claims; human closes |
| Backup cadence + WAL archive interval | Full weekly + incremental daily; WAL archiving continuous (archive-push per completed segment) — the cadence each RPO recommendation implies is derived per tier in the P12.04 register | RECOMMENDATION — derives from the P12.04 tier table; no measured volume exists to size it |
| Backup storage location + residency constraint | S3-compatible object storage, bucket layout §4 below; residency constraint `<EMPTY>` | D1 OPEN (P9.10 §4/§7): no residency constraint can be stated |
| Backup encryption at rest / in transit | In transit: TLS to the object store. At rest: server-side or repo-cipher encryption `<EMPTY — deployment decision>`; the exact mechanism is UNVERIFIED until a tool version is pinned | P9.24 §3 row; §17.3 requires encryption in transport and storage — the mechanism cell stays EMPTY until the human chooses |
| Backup retention period + basis | Structure in §5 below; every period `<EMPTY>` | P9.10 §6: no period without a cited basis; D1 OPEN |
| JetStream stream replication/retention posture | Snapshot-based stream backup into the same object-storage target (P12-E-2 DEFAULT); `num_replicas: 1` unchanged at design time | Registered as P12-E-2, OPEN — record `02_p12_02_backup_procedures.md` §4 |
| Edge-store backup posture | No separate edge-journal backup in the default posture: the edge journals are bounded self-recovering stores whose platform copies exist once uploaded (at-least-once, in-order — P9.24 §2.2); revisit at deployment | RECOMMENDATION — P9.24 §2.2 left the cell EMPTY for this decision; recorded here as the default, human closes |
| Restore-environment provisioning method | The isolated clean environment of the P12.03 runbook (P9.24 §5 R-1) — provisioning method `<EMPTY — deployment decision>` | The P12.03 record owns the runbook; the provisioning cell stays a deployment decision |
| Backup verification job | `pgbackrest check` (stanza + archive verification) on a scheduled cadence — explicitly NOT a restore | UNVERIFIED subcommand behaviour [S-68]; the restore drill remains the evidence (LR-204) |

## 4. Object-storage target shape

One S3-compatible bucket per deployment (residency per the still-EMPTY D1
cells — P9.10 §4/§7), versioned prefixes, no secret material in any config
file (the P11.16 discipline: configuration carries references, never
values):

```text
s3://<bucket>
  pgbackrest/repo1/                 PostgreSQL: stanza repo (base + WAL archive)
    backup/<stanza>/...             base backups (full/incr per §3 cadence row)
    archive/<stanza>/...            archived WAL segments (the PITR input)
  nats-snapshots/<stream>/<snapshot-timestamp-UTC>/...
                                    JetStream per-stream snapshots (P12-E-2
                                    DEFAULT posture; streams per
                                    crates/adapters/nats/src/topology.rs)
  manifests/<store>/<artifact-id>.json
                                    integrity manifests (sha256 per file,
                                    produced by the P12.02 tooling)
```

| Shape rule | Statement |
|---|---|
| Shared target | PostgreSQL backups and JetStream snapshots share ONE object-storage target (the P12-E-2 DEFAULT wording) so the restore drill (P12.03) and the JetStream drill (P12.14) exercise one residency/encryption configuration |
| Immutability | Object-lock / write-once retention is a RECOMMENDATION (`<EMPTY>` mechanism cell) — the append-only/evidence classes (0008 audit_records, 0018 journal, 0016 command history per P9.24 §2.1) are the drivers; the mechanism is UNVERIFIED per provider and chosen at deployment |
| Residency | `<EMPTY>` — D1 OPEN; a location chosen before D1 closes can violate a discovered constraint (P9.24 §11 R-P9.24-6, carried unchanged) |
| Credentials | Object-store credentials are deployment secret-management inputs (P11.16 floor: file-per-host Compose floor, Vault DEFAULT); no credential value appears in any file in this repository |
| Naming | Snapshot/backup timestamps are UTC, sorted-sortable; artifact IDs are the manifest identifiers, not assumed provider-internal IDs |

## 5. Retention schedule per data class — structure with EMPTY periods

Per P9.10 §6: each period cell is filled ONLY from a stated basis (customer
contract / regime rule named at D1 closure) — a period with no cited basis
is not deployable. The cadence column is the backup cadence the P12.04
tier recommendation implies; the retention column stays EMPTY.

| Data class (P9.24 §2.1 stores) | Backup cadence implied (RECOMMENDATION) | Retention period | Basis (cited) |
|---|---|---|---|
| Registry (0002–0005) + asset_state (0006) | Restores atomically with every DB snapshot — no separate cadence | `<EMPTY>` | `<EMPTY>` |
| Outbox (0007) + commands/transitions (0016) | Every DB snapshot + WAL (dispatch evidence — completeness per P9.24 §6.5 checks) | `<EMPTY>` | `<EMPTY>` |
| audit_records (0008) + audit chain (0019) | Every DB snapshot + WAL; evidence class — the per-class RPO delta cell (P9.24 §4) may demand a tighter bound than telemetry; the customer decides | `<EMPTY>` | `<EMPTY>` |
| Telemetry (0010/0012, monthly partitions) | Every DB snapshot + WAL; partition-aware detach/archive per the P2.20 maintenance path is the deletion mechanism (0010 D-P5) — backup retention is a deployment decision, never the DDL's | `<EMPTY>` | `<EMPTY>` |
| Schedules/envelopes/feasibility (0013–0017) | Every DB snapshot + WAL (revision CHAINS restore whole — P9.24 §2.1) | `<EMPTY>` | `<EMPTY>` |
| Edge registry + enrollment journal (0018) | Every DB snapshot + WAL (append-only journal completeness — a revoked identity that looks active is the named failure) | `<EMPTY>` | `<EMPTY>` |
| JetStream streams (P1.14 provisioned set) | Per-stream snapshot cadence — see the P12.04 tier row for the event plane | `<EMPTY>` | `<EMPTY>` |
| Edge-local journals (P9.24 §2.2) | No separate backup in the default posture (§3 row above) | `<EMPTY>` | `<EMPTY>` |

## 6. What this record does NOT establish

1. It does not install, deploy, or exercise pgBackRest, WAL-G, or any
   object store — every capability row in §2 is UNVERIFIED until a real
   version is pinned and run by the human gate.
2. It does not close LR-204: a selection record is not a backup; the gap
   statement survives until the P12.03 drill fills its evidence from a
   real run.
3. It does not set any retention period or residency constraint (D1 OPEN,
   P9.10 §6) and does not state an RPO/RTO value — those live, as
   RECOMMENDATIONS, in the P12.04 register.
4. It does not choose a cloud provider, a bucket technology, or an
   encryption mechanism — every such cell is the human owner's.
5. It adds no dispatch authority: backup/restore is a data-plane
   survivability capability; the never-granted permission set
   (`dispatch.authorize`, `dispatch.issue`, `secret.read`, `network.any`)
   is untouched and ADR-010 is unaffected.

## 7. Reviewer-verification procedure (human)

1. Confirm §1's plain statement and that no backup tool, bucket, or
   artifact exists anywhere in the repository or its history.
2. Confirm every §2 capability row carries its UNVERIFIED marker and cites
   [S-68] — no claim is presented as tested fact.
3. Confirm §3 fills the P9.24 §3 rows as RECOMMENDATION/DEFAULT only and
   that the authoritative cells in the P9.24 record remain `<EMPTY>`.
4. Confirm §5's period and basis columns are EMPTY in every row and that
   the no-basis-no-deploy rule (P9.10 §6) is stated.
5. Confirm §4 carries no real bucket name, endpoint, or credential.
6. Trace the store classes in §5 to the migration files they cite
   (0002–0018) and to P9.24 §2.1.

## 8. Filling commands (for the human executor)

```sh
# (a) the documented-tools gate (run after ANY edit to this record):
python tools/verify_docs.py
python tools/validate_agent.py
python -I tools/check_dependency_rule.py

# (b) At D1 closure: fill P9.10 §6 per class, then the §5 period cells FROM
#     those rows — a period without the cited basis is rejected.
# (c) Choose the tool (confirm or overturn the §2 DEFAULT), the bucket,
#     the encryption mechanism, and the residency constraint; fill the
#     authoritative P9.24 §3 cells; only then is a backup configuration
#     deployable.
# (d) Execute the P12.03 restore drill against the chosen configuration;
#     LR-204 closes only when that drill's record is filled from a real run.
```

## 9. Findings and assumptions

| # | Item | Note |
|---|---|---|
| A-P12.01-1 | All tool-capability claims are sourced from [S-68], a secondary comparison article | UNVERIFIED; pin real versions at deployment and re-verify |
| A-P12.01-2 | The cadence rows assume the P12.04 tier RECOMMENDATIONS hold after sign-off | If the operator signs different values, §3/§5 cadence cells re-derive |
| A-P12.01-3 | The edge-store default posture (no separate backup) depends on the at-least-once, in-order upload mechanics recorded in P9.24 §2.2 | If a deployment changes the upload discipline, the posture row re-opens |
| F-P12.01-1 | Residency and encryption-at-rest cells are EMPTY while D1 is OPEN — a backup location chosen now can violate a later-discovered constraint | Carried unchanged from P9.24 §11 R-P9.24-6 |

Status: **WRITTEN-UNRUN** — a selection record cannot install or exercise a
backup tool; the installation, the first backup, and the first restore are
the human gates and have not happened and are not scheduled.
