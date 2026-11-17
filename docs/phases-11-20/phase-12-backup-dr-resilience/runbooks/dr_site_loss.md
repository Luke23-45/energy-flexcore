<!-- P12.11 — DR runbook (site-loss scenario). RUNBOOK TEMPLATE + PROCEDURE
     DESIGN: nothing has ever been rehearsed, restored, or reconfigured;
     every execution cell is EMPTY/UNRUN. Cites P9.24 §5–§6 and
     deployment-upgrade-model.md §6 — cited, NEVER rewritten or
     re-decided. Restores reference Agent A's P12.02 procedure
     (records/02_p12_02_backup_procedures.md + deploy/compose/backup/) —
     first cited by expected path per plan §2.1, reconciled to Agent A's
     landed names at close. -->
# DR Runbook — Site-Loss Scenario and Restore-Into-Isolation (P12.11 — written-unrun)

Date: 2026-11-14. Subtask: P12.11. Status: **PROCEDURE DESIGN, WRITTEN-UNRUN.**
No site loss has occurred or been simulated; no restore has been performed; no
DNS change has been made; no communication has been sent. Every execution cell
below is EMPTY/UNRUN and fills ONLY from a real rehearsal by the human gate.
The procedure-of-record for the restore mechanics is **P9.24 §5 (R-0..R-10)
and §6 (E-1..E-6)** — cited throughout, never rewritten; the backup/restore
model of record is **`docs/operations/deployment-upgrade-model.md` §6** — same
discipline. The backup procedure this runbook executes is Agent A's P12.02
output, cited by EXPECTED PATH (`records/02_p12_02_backup_procedures.md`,
`deploy/compose/backup/` — sibling-citation pattern, plan §2.1; reconciled
to Agent A's landed record name and artifact set at close).

## 1. Scenario definition (what "site loss" means here)

| Element | Definition (this runbook) |
|---|---|
| The lost site | the Zone 2 central application site (control plane, PostgreSQL, NATS, plugin host, workers, console — the P9.05 zone model, `deployment-upgrade-model.md` §2), or the hosting venue containing it |
| NOT lost by assumption | the site-local EMS/protection layer (Zone 4/5, customer-owned) — the central-outage rule (`deployment-upgrade-model.md` §5, verbatim intent): a central outage must not disable site-local protective controls or force an unsafe device state; FlexCore's absence degrades orchestration, never site safety |
| Edge agents during site loss | the E-1..E-6 posture (P9.24 §6): bounded store-and-forward within the audited buffer bounds, no new commands while disconnected, audited drop-of-oldest beyond the bound (R-P9.24-5 — the bounded loss window, recorded, never silent) |
| The declared recovery objective | NONE is set here. RTO/RPO for the DR scenario are the P12.04 RECOMMENDATION register's values pending operator sign-off — an EMPTY sign-off cell means this runbook has NO binding recovery objective, and §5's judgement cells judge against P9.24 §4's PLACEHOLDER requirement cells, never against invented numbers |

## 2. Trigger and decision gates (human decisions, recorded)

| Gate | Decision | Owner | Record |
|---|---|---|---|
| T-0 Site-loss declaration | is the Zone 2 site actually lost (vs degraded)? | incident commander (human) | `<EMPTY>` |
| T-1 Activate this runbook | proceed with DR, or wait/recover in place? | incident commander + operator | `<EMPTY>` |
| T-2 Scope the restore | which backup artifact set (Agent A's P12.02 procedure output, `deploy/compose/backup/` — `backup_postgres.sh`, `pgbackrest.conf`, `verify_backup.sh`), which instant (latest base backup; PITR to a chosen instant is per the P12.01/P12.02 backup design — DEFAULT posture, P12-E-2 for JetStream snapshots) | operator | `<EMPTY>` |
| T-3 Isolation check | the target environment shares NO state with anything facing production traffic (the P9.24 §5 R-1 isolation criterion) | operator | `<EMPTY>` |
| T-4 Go/no-go for DNS/reconfiguration | the restored instance is validated (§4) BEFORE any client-facing reconfiguration | incident commander | `<EMPTY>` |

## 3. Restore-into-isolation procedure (executes P9.24 §5 — cited, not rewritten)

The steps below are the DR-runbook view of P9.24 §5's R-0..R-10; each row
names the R-step it executes. A run fills cells ONLY from the run.

| DR step | Executes | Procedure (summary — the authority is the cited step) | Result cell |
|---|---|---|---|
| DR-1 | R-0 | record the source state: software versions, migration set, artifact IDs, audit counts per tenant, command/transition counts, outbox rows | `<EMPTY — UNRUN>` |
| DR-2 | R-1 | provision the isolated environment (method = the P9.24 §3 restore-environment cell, filled by the deployment; consumes Agent A's P12.02 scripts, `deploy/compose/backup/`) | `<EMPTY — UNRUN>` |
| DR-3 | R-2 | restore PostgreSQL from the chosen artifact set (the P12.02 procedure; the DEFAULT backup design is P12.01's) | `<EMPTY — UNRUN>` |
| DR-4 | R-3 | verify the append-only/evidence properties survived at the SCHEMA level (0008 audit trigger + no DELETE grant; 0018 journal triggers; 0012 correction rows) | `<EMPTY — UNRUN>` |
| DR-5 | R-4 | audit integrity by count and chain (per-tenant counts vs DR-1; the edge audit journal vs its anchor via `audit_proof.rs :: verify_against_anchor`) | `<EMPTY — UNRUN>` |
| DR-6 | R-5 | edge-journal copies verified by CRC framing replay (`buffer_file.rs` / `audit_buffer.rs` mechanics); a failed CRC is a counted loss, never silently accepted | `<EMPTY — UNRUN>` |
| DR-7 | R-6 | command/outbox consistency: zero already-issued command re-issued by replay; in-flight dispatch at the snapshot follows the §14.4 fallback rules (P9.20 register) | `<EMPTY — UNRUN>` |
| DR-8 | R-7 | point a control-plane instance at the restored store, in the isolated environment: `/ready` passes; read-only API sweep returns the restored rows | `<EMPTY — UNRUN>` |
| DR-9 | R-8 | measure: total restore time, per-step times, observed data-loss window — from the run's clocks/logs only | `<EMPTY — UNMEASURED>` |
| DR-10 | R-9 | judge measured values against the §4 requirement cells (P9.24 §4; worse-than-requirement is a FINDING, never a re-defined requirement) | `<EMPTY — UNRUN>` |
| DR-11 | R-10 | tear down + record destruction of the restored copies (residency per P9.10) | `<EMPTY — UNRUN>` |
| DR-12 | P10.11 §4.1–4.4 | the RC-specific re-checks (schema-version match, JetStream stream-config match, outbox/inbox survival, scoped JetStream restore authorization — an unauthorized stream-name restore must be refused) ride the same rehearsal when executed against a release candidate | `<EMPTY — UNRUN>` |

The JetStream leg restores stream state per the recorded P12-E-2 DEFAULT
posture (snapshot-based backup into the same object-storage target — cited
from plan §4; the drill design is P12.14's, wave 2). If P12-E-2 is still
OPEN when a rehearsal runs, the rehearsal executes the DEFAULT and records
the dependence — never a posture of its own.

## 4. DNS / reconfiguration steps (written-unrun; all values are deployment placeholders)

| Step | Procedure | Guard |
|---|---|---|
| N-1 | Freeze the CURRENT DNS/TLS state: record every hostname the clients use (console, API, edge uplink endpoints, OTel collector), their current targets, and TTLs — `<EMPTY>` (the host inventory is a deployment input, `deployment-upgrade-model.md` §2 placeholders) | recorded BEFORE any change |
| N-2 | Prepare the reconfiguration plan in the isolated environment FIRST: point a TEST client at the restored instance using the same hostname resolution path (a hosts-file override, never a live DNS edit) | no live DNS change before T-4 go/no-go |
| N-3 | On T-4 go: change the client-facing records to the recovery target with the recorded TTLs; the change is a named operator act with a named approver, recorded | approver `<EMPTY>`; change-window record `<EMPTY>` |
| N-4 | Edge reconnection honesty: edges reconnect by their own outbound session (no inbound listener exists — the F1 rule, `deployment-upgrade-model.md` §2); after reconfiguration, confirm each edge re-establishes its session and drains its buffered journal per E-4 (exactly-once, in-order — P9.24 §6), with `DegradedBufferingExited` audited | per-edge drain record `<EMPTY>` |
| N-5 | Rollback of the reconfiguration: revert the DNS records; the rollback is the reverse named act, never an improvisation; no silent downgrade of any component version (the P10.12 rollback register rules govern versions) | rollback record `<EMPTY>` |

## 5. Communication checklist (all cells EMPTY until a real event or rehearsal)

| Audience | Trigger | Channel (deployment decision) | Message template | Sent |
|---|---|---|---|---|
| Incident commander ↔ operator | T-0/T-1 | `<EMPTY>` | site-loss declaration + runbook activation, timestamped | `<EMPTY — NOT SENT>` |
| Operator ↔ customer contact (if any) | T-1 | `<EMPTY>` | service status per the customer contract (P9.10); NO RTO/RPO promise — the register's sign-off cell governs what may be promised | `<EMPTY — NOT SENT>` |
| Operator ↔ edge/site owners | N-3 | `<EMPTY>` | reconfiguration window + expected edge reconnection behaviour | `<EMPTY — NOT SENT>` |
| Evidence keeper ↔ program record | after DR-11 | this runbook's §6 | the filled rehearsal record reference | `<EMPTY — NOT WRITTEN>` |

## 6. Rehearsal record — EMPTY template (UNRUN)

Filled ONLY from a real run's clocks, logs, and counts; a filled cell
without a run is a fabrication defect (the P9.24 §8 rule, inherited).
Metadata: dates / rehearser / witness / isolated-environment ID / artifact
IDs / migration set / backup-procedure version (Agent A's P12.02 record
version) / RTO-RPO register version (P12.04) — all `<EMPTY>`.

| Step | Result | Measured value | Pass/fail | Disposition |
|---|---|---|---|---|
| DR-1 source state | (EMPTY — UNRUN) | | | |
| DR-2 isolation | (EMPTY — UNRUN) | | | |
| DR-3 restore | (EMPTY — UNRUN) | | | |
| DR-4 append-only properties survive | (EMPTY — UNRUN) | | | |
| DR-5 audit count + chain | (EMPTY — UNRUN) | | | |
| DR-6 edge-journal CRC replay | (EMPTY — UNRUN) | | | |
| DR-7 zero re-issue | (EMPTY — UNRUN) | | | |
| DR-8 consistent-state probe + sweep | (EMPTY — UNRUN) | | | |
| DR-9 measured time + loss window | (EMPTY — UNRUN) | restore time `<EMPTY>` / loss window `<EMPTY>` | | |
| DR-10 judgement vs requirements | (EMPTY — UNRUN) | | | |
| DR-11 teardown + destruction | (EMPTY — UNRUN) | | | |
| DR-12 RC-specific checks | (EMPTY — UNRUN) | | | |
| N-1..N-5 DNS/reconfiguration | (EMPTY — UNRUN) | | | |
| §5 communications | (EMPTY — NOT SENT) | | | |

## 7. Decision registration — P12-E-3 (registered OPEN; agents never close it)

| Field | Value |
|---|---|
| ID | P12-E-3 |
| Question | Is a second (DR) site required for v1.x? |
| Options | warm DR site / cold restore-into-isolation posture / none |
| DEFAULT (RECOMMENDATION) | **no second site in the current posture** — DR is this documented site-loss runbook plus the restore-into-isolation rehearsal (§3–§6); revisit at the Phase 20 multi-region trigger |
| Closer | the human owner (plan §4) |
| Status | **OPEN** — registered here per the subtask instruction; nothing in this runbook closes it, and a second site is neither designed nor promised |

## 8. Honest boundaries

- This runbook cites P9.24 §5–§6 and `deployment-upgrade-model.md` §6 —
  it adds orchestration and communication structure around them, and it
  never re-decides their criteria (the defined-once discipline).
- Nothing here grants field write authority or touches the never-granted
  permission set; DR recovery restores a supervisory control plane whose
  command path stays disabled until the recorded per-site enablement gates
  (`deployment-upgrade-model.md` §4) are satisfied again.
- A successful rehearsal would establish the tested-day facts only (§6's
  filled cells) — never a guarantee, never a conformance claim (§13.3).
- Agent A's P12.02 paths were first cited as EXPECTED paths (plan §2.1)
  and reconciled at close to the landed record
  (`records/02_p12_02_backup_procedures.md`) and artifact set
  (`deploy/compose/backup/`); the reconciliation is also recorded in
  `records/25_p12_11_dr_runbook.md`.
