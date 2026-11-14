<!-- P9.24 — Backup, restore, and disaster-recovery rehearsal (PLAN + EMPTY rehearsal record). The PLAN is written; NO backup configuration exists, NO restore has ever been performed, NO restore time or data-loss window has been measured, NO edge-outage test has run — every execution cell is EMPTY/UNRUN (human gate, ENTRY_NOTE boundaries 1 and 6). RPO/RTO are PLACEHOLDER REQUIREMENTS (customer-dependent, §21.2: do not set RPO/RTO without customer and service requirements); no measured value is stated anywhere. -->
# P9.24 — Backup, Restore, and Disaster-Recovery Rehearsal (Rehearsal Plan — nothing has been backed up or restored)

Date: 2026-11-14. Subtask: P9.24 (`ledger_phase_9.md` §P9.24).
Owner role: Platform engineering.
Report trace: §23 Stage 9; §17.3 (backup/restore tests); §21.2.
Inputs: P9.10 retention design (TEMPLATE —
`10_data_residency_retention.md`; its §5 store inventory is the filled
input this record cites); the pilot deployment (control plane, PostgreSQL,
NATS JetStream, edge agents) — **which does not exist**; the backup
configuration — **which does not exist**.

## 1. Honest status — the plain statement

**No backup configuration exists, no backup has been taken, no restore has
ever been performed, no restore time or data-loss window has been
measured, and no edge-outage test has run.** There is no pilot deployment
to back up. This record delivers the REHEARSAL PLAN the human gate
executes: the backup scope with the REAL stores cited by file and their
append-only/evidence properties (§4), the RPO/RTO placeholder requirements
(§5 — customer-dependent, EMPTY by the §21.2 rule), the rehearsal
procedure with integrity verification by citation of the actual mechanisms
(§6–§7), the edge-outage test plan (§8), and the rehearsal record as an
EMPTY template (§9). The ledger's acceptance criteria are NOT met and are
NOT claimed: no restore was performed and verified; no restored-record
completeness was checked; no measured time/loss window exists; no edge
behaviour was observed. A backup that has never been restored is the
ledger's named classic failure — until §9 is filled from a real run, this
plan is exactly that gap, recorded rather than hidden.

## 2. What exists to back up, by citation (read 2026-11-14)

The §16.1 storage stack: PostgreSQL is the transactional source of truth;
NATS JetStream carries the §15.4 event streams; the edge holds its own
bounded journals. The stores below are the ones the reviewed code
actually creates; §2.3 lists the classes with NO store (nothing to back
up, and the plan does not pretend otherwise).

### 2.1 Central PostgreSQL — `crates/adapters/postgres/migrations/`

| Store (migration) | Contents | Backup-relevant properties (append-only / evidence) |
|---|---|---|
| `tenants`/`portfolios`/`sites`/`assets` (0002–0005) | Registry rows; no Phase 1 delete path ("a future delete needs an explicit, reviewed migration", 0002) | Registry is slow-changing and load-bearing for tenancy — must restore atomically with the rest of one consistent snapshot |
| `asset_state` (0006) | Current per-asset state (SOC [0,1], MW/MVAr, availability, health, event/ingest time) | CURRENT-state table: the newest row wins; a restore rolls state back to the snapshot instant (a data-loss-window item, §5) |
| `outbox` (0007) | §15.5 transactional-outbox event payloads | Restoring the outbox with the command records is what prevents replay of already-issued commands after restore (§6 step 6; the ledger's named risk) |
| `audit_records` (0008) | Append-only audit: actor, action, object refs, decision + reason reference, evidence jsonb, correlation_id | **Append-only by trigger + grants (no DELETE grant to ANY role); corrections are NEW rows (`corrected_audit_id`), never edits** — a restore must preserve row-for-row completeness; the §6.5 count check is the integrity gate |
| `telemetry` (0010, extended 0012) | Monthly partitions (`telemetry_yYYYYmMM`, RANGE on event_time); quality enum; quarantine; versioned corrections (`correction_reason`/`corrected_by`/`corrected_at`) | Corrections are new rows — ALL versions restore together; the DDL carries NO retention value/TTL/scheduled-delete (0010 D-P5) — backup retention is a deployment decision per P9.10 §6, never the DDL's |
| `schedule_results`/`schedule_revisions`/`flexibility_envelopes` (0013–0015) | Optimization outputs, revision chains, envelopes | Revision CHAINS: partial restore breaks chain integrity — restore whole |
| `dispatch_commands` + `dispatch_command_transitions` (0016) | Closed 14-state §14.2 vocabulary; immutable command rows per tenant; `valid_until_millis`; supersede links; `tenant_id` in PK + RLS | Command/transition history is dispatch EVIDENCE — completeness verified per §6.5 |
| `feasibility_check_records` (0017) | Feasibility evidence: model id/version, results | Evidence class — same completeness rule |
| `edge_device_registry` + `edge_enrollment_journal` (0018) | Device identities; append-only lifecycle journal (ENROLLED/ROTATED/REVOKED/EXPIRED/REJECTED) with actor; UPDATE/DELETE refused by trigger | **Append-only journal**: a restored registry whose journal is incomplete changes identity semantics (a revoked identity that looks active) — journal completeness is checked per §6.5 |

### 2.2 Edge-local stores — `crates/edge-agent/src/`

| Store | File | Backup-relevant properties |
|---|---|---|
| Telemetry journal | `buffer_file.rs` (P6.09; magic `FLEXCORE-EDGE-TELEMETRY-JOURNAL` + version byte 0x01) | Append-only CRC-32-framed journal (`[u32 LE len][u32 LE crc32][payload]`); disk-first append; audited drop-of-OLDEST with `dropped_oldest` + `has_overflowed` latch; truncate-and-audit recovery (`corrupt_tail_truncated`); age retention on `ingest_time` |
| Audit journal | `audit_buffer.rs` (P6.18; magic `FLEXCORE-EDGE-AUDIT-JOURNAL` + version byte 0x01) | Same framing discipline; in-order at-least-once upload with fail-closed `commit_uploaded` sequence check; bounded capacity; audited drop-of-oldest; monotonic sequences PERSIST in the journal |
| Command-id binding journal | `pilot_rejection_proof.rs` :: `FileCommandIdBinding` (P9.31) | Restart-surviving idempotency bindings (command_id ↔ payload); refuses a conflicting payload under a used id |
| Enrollment identity-lifecycle register | `enrollment_ceremony.rs` (P9.33) | Append-only; CRC-32 framing; a revoked identity can never authorize its successor |
| Update/rollback register | `update_verification.rs` (P9.34) | Append-only signed-update and rollback register; never-silent downgrade rule |

**Edge-store posture decision (the plan does not pre-decide it):** the
edge journals are BOUNDED, self-recovering stores whose platform copies
exist once uploaded (at-least-once, in-order — §15.4). Whether edge
journals are separately backed up, and at what cadence, is a deployment
decision against the P9.10 retention table (`10_data_residency_retention.md`
§6 edge rows, periods `<EMPTY>`): decision cell `<EMPTY>`. What the plan
REQUIRES regardless (§8): the edge survives a central outage within its
bounded buffer and drains without loss beyond the counted, audited
drop-of-oldest window.

### 2.3 Stores that DO NOT exist (nothing to back up — recorded, not hidden)

| §16.3 class | Status |
|---|---|
| Market records | No implemented store (`10_data_residency_retention.md` §5.C; P9.11/D1-gated) |
| Aggregated intervals | No implemented store |
| Simulation artifacts / exports / model files | §16.1 recommends S3-compatible object storage; no implementation cited |
| NATS JetStream streams | Stream configs are deployment artefacts (per-stream retention/limits, §15.4) — none recorded; the backup posture for JetStream is a §3 decision cell, `<EMPTY>` |
| Forecast outputs | Event-only (`ForecastPublished`, §15.3) |

## 3. Backup configuration — DOES NOT EXIST (the decision template)

Every cell below is a deployment decision the human gate fills; the plan
does not choose a tool, a cadence, or a topology (no tool is installed in
this loop, and none is assumed to exist).

| Decision | Value |
|---|---|
| Backup tool/method for PostgreSQL (e.g. base backup + WAL archiving — the METHOD is the deployment's to choose and justify) | `<EMPTY>` |
| Backup cadence + WAL archive interval | `<EMPTY>` (drives the achievable RPO bound, §5) |
| Backup storage location + residency constraint | `<EMPTY>` (per P9.10 §4/§7 — D1 OPEN, no residency constraint can be stated) |
| Backup encryption at rest / in transit | `<EMPTY>` (§17.3 encryption in transport and storage) |
| Backup retention period + basis | `<EMPTY>` (per P9.10 §6 — no universal period may be chosen, §16.3) |
| JetStream stream replication/retention posture | `<EMPTY>` |
| Edge-store backup posture (§2.2) | `<EMPTY>` |
| Restore-environment provisioning method (the §6 clean environment) | `<EMPTY>` |
| Backup verification job (does the un-restored backup pass its own checks?) | `<EMPTY>` (a verify job is NOT a restore — the ledger's criterion (a) stands) |

## 4. RPO / RTO — PLACEHOLDER requirements (§21.2 rule), EMPTY

Report §21.2 verbatim in substance: do not set RPO/RTO without customer
and service requirements — record the measured values and the requirements
they are judged against. Accordingly:

| Requirement | Value | Basis |
|---|---|---|
| RPO requirement (max tolerable data loss) | `<EMPTY — PLACEHOLDER>` | Customer contract + service requirements (§21.2); interacts with the P9.10 retention table |
| RTO requirement (max tolerable restore time) | `<EMPTY — PLACEHOLDER>` | Same |
| Per-class RPO delta (e.g. audit evidence vs telemetry) | `<EMPTY>` | The §17.3/§16.3 evidence classes may demand a tighter bound than telemetry; the customer decides |
| Achievable-RPO bound implied by the §3 cadence/WAL cells | `<EMPTY>` (computable only after §3 is filled) | Derived from the deployment's own backup mechanics |
| Measured restore time (from §9) | `<EMPTY — UNMEASURED>` | Filled ONLY from the §6 rehearsal run |
| Measured data-loss window (from §9) | `<EMPTY — UNMEASURED>` | Filled ONLY from the §6 rehearsal run |

Judgement rule (pre-printed): the measured values are recorded against the
requirements; a measured value WORSE than the requirement is a FINDING
with a disposition (fix and re-rehearse / customer-accepted residual risk
carried to P9.27 / blocks the pilot) — never a re-defined requirement.

## 5. The rehearsal procedure — restore into a CLEAN environment

The rehearsal is a real restore, executed by people, in an isolated
environment that never faces production traffic and never shares state
with the production deployment. Every step's result cells live in §9.

| Step | Procedure | Pass criterion |
|---|---|---|
| R-0 | Record the source state: software versions, migration set applied (a restored DB with an OLDER schema than the running code is the ledger's named risk — the migration set is recorded and must match the running control plane's expectations), backup artifact IDs, audit-record count per tenant, command/transition counts, outbox row count | Counts recorded; artifact IDs resolve |
| R-1 | Provision the clean/isolated environment (per the §3 method cell) with NO access to production; record the isolation check | Isolation verified and recorded |
| R-2 | Restore PostgreSQL from the chosen backup artifact into the clean environment | Restore completes without error; migration set matches R-0 |
| R-3 | Verify the append-only/evidence properties SURVIVED the restore (schema-level, not assumed): the 0008 audit-records trigger refuses UPDATE/DELETE and no DELETE grant exists; the 0018 registry/journal triggers refuse; the 0012 correction columns are populated | Trigger/grant inspection transcript recorded |
| R-4 | Verify audit integrity by COUNT and CHAIN: (a) row counts per table equal R-0's counts (the ledger's verification method: "the restored database's audit record count against the source"); (b) for the edge-side chain, export the edge audit journal and verify it against its pre-outage anchor via `audit_proof.rs` :: `verify_against_anchor` / `recompute_chain_head` (FNV-1a-64 chain — tamper-EVIDENCE-grade, honestly bounded; §2.2) | (a) zero count deltas, or deltas explained and counted; (b) anchor verification passes |
| R-5 | Verify edge-journal copies by FRAMING (CRC), by citation: a restored/copied telemetry or audit journal is re-opened by the same reader mechanics (`buffer_file.rs` / `audit_buffer.rs` replay): CRC-32 checked per record, strict decode, truncate-at-first-corrupt with a counted `corrupt_tail_truncated` — a copy that fails its CRC is a counted loss, never silently accepted | Replay transcript; any truncation counted |
| R-6 | Verify command/outbox consistency: the outbox (0007) and the command records (0016) agree; NO already-issued command is re-published or re-issued after restore (idempotent replay, §6.3/§15.5; the ledger's risk "an outbox state that replays already-issued commands"); recovery of any dispatch in flight at the snapshot follows the §14.4 fallback rules (P9.20 register) | Zero re-issued command IDs; replay audit shows idempotent handling |
| R-7 | Point the control plane at the restored store in the clean environment and verify it reaches a consistent state: `/ready` probe passes; a read-only API sweep (`/api/v1/tenants…assets`, telemetry query) returns the expected restored rows; the command-state vocabulary round-trips | Probe + sweep transcript |
| R-8 | Measure and record: total restore time, per-step times, and the observed data-loss window (snapshot instant → newest lost write, bounded by the §3 cadence/WAL cells) | Numbers from the run's clocks/logs, never estimated |
| R-9 | Judge the measured values against the §4 requirement cells; record the disposition per §4's judgement rule | Disposition recorded per value |
| R-10 | Tear down the clean environment; record destruction of the restored copies (residency/retention per P9.10) | Destruction recorded |

## 6. The edge-outage test (ledger step 4) — plan

Tested at a real (or lab-representative) edge against a stopped central
platform; this overlaps P9.20's D-1/D-2 drills but answers the BACKUP/
RESTORE question specifically: what the edge preserves while the restored
central platform is absent, and that nothing stale is issued on recovery.

| Step | Procedure | Pass criterion |
|---|---|---|
| E-1 | Record the edge baseline: buffer counters (`FileAuditBuffer::appended_total` / `pending_len`, `FileLocalBuffer` equivalents), sequence-checkpoint state (`FileSequenceCheckpoint`), binding-journal count (`FileCommandIdBinding::len` / `records_written`), site mode record | State recorded |
| E-2 | Stop the central platform; let the edge operate through telemetry, commands, and audit events for a recorded interval; confirm the audited degraded-mode entry (`DegradedBufferingEntered` — `runner.rs`) | Entry audited |
| E-3 | Confirm bounded behaviour throughout: capacity bound respected; any overflow is the audited drop-of-OLDEST with `dropped_oldest` incremented and `has_overflowed` LATCHED — never a silent overwrite, never unbounded growth (`audit_buffer.rs`, `buffer_file.rs`) | Counters recorded; zero unexplained losses |
| E-4 | Restore the central platform (or the restored clean-environment instance for a full-chain test) and confirm store-and-forward: every buffered record uploads EXACTLY ONCE (no loss, no duplication), IN ORDER — the at-least-once/in-order mechanics with fail-closed `commit_uploaded` (P6.18) — and the drain completes with `DegradedBufferingExited` audited | Upload counts reconcile exactly against E-1/E-3 counters |
| E-5 | Confirm NO command is issued from a stale buffered state: a command that expires during the outage is rejected on the recovery path (expiry vs the edge clock — `command_verify.rs`, and the clock-quality fail-closed rule: any quality other than `Synchronized` fails the expiry check); a replayed pre-outage command is rejected via the restart-surviving sequence checkpoint (`FileSequenceCheckpoint`) | Zero stale/expired/replayed commands issued; rejections audited |
| E-6 | Record the measured outage interval, buffer occupancy, drain time, and any counted losses | Numbers from the run |

**What the §17.3 backup/restore control and §21.2 require here:** the
restore rehearsal proves the central platform is recoverable; the edge
test proves the site survives the central platform's absence within its
bounded buffer — together they are the §21.2 degraded-mode evidence this
subtask feeds (P9.20's drills observe the same mechanics for the fallback
question; the two records cross-reference, they do not duplicate).

## 7. What the rehearsal would and would not establish

**Would establish (once run):** that THIS deployment's chosen backup
configuration restores to a consistent, complete, evidence-intact state in
a clean environment, with MEASURED restore time and data-loss window
judged against the customer's recorded RPO/RTO requirements; and that the
edge bounds its outage losses to the audited drop-of-oldest window with
zero stale command issuance on recovery — at the tested scale, versions,
and day.

**Would NOT establish:** RPO/RTO "guarantees" (the measured values are
one run's outcome, not a service-level guarantee — §21.2's honesty rule);
protection-grade or safety-function capability (AGENTS.md §3; site-local
protection independence is §17.5 invariant 10, observed per P9.20, never
provided by FlexCore); behaviour under failure modes not injected here
(e.g. corrupted backup artifacts, partial WAL loss — the plan's R-steps
can be extended, and an extension is recorded, not improvised);
bounded-automation readiness (ENTRY_NOTE boundary 3); and NOT the Phase 10
evidence — P10.11 re-runs this rehearsal against the release candidate
(cross-reference per the ledger: Phase 10 does not inherit this run's
numbers, it re-measures).

## 8. The rehearsal record — EMPTY template (UNRUN)

All measured cells EMPTY. Filled ONLY from a real run's clocks, logs, and
counts, per §5/§6; a filled cell without a run is a fabrication defect.

**Rehearsal metadata (all EMPTY):** dates / rehearser / witness / clean-
environment ID / source-snapshot artifact IDs / software versions +
migration set / backup configuration used (the §3 row version) / RPO/RTO
requirement cells in force (the §4 row version).

### 8.1 Restore rehearsal (§5)

| Step | Result | Measured value | Pass/fail | Disposition |
|---|---|---|---|---|
| R-0 source state | (EMPTY — UNRUN) | | | |
| R-1 isolation | (EMPTY — UNRUN) | | | |
| R-2 restore | (EMPTY — UNRUN) | | | |
| R-3 append-only properties survive | (EMPTY — UNRUN) | | | |
| R-4 audit count + chain | (EMPTY — UNRUN) | | | |
| R-5 edge-journal CRC replay | (EMPTY — UNRUN) | | | |
| R-6 outbox/command consistency, zero re-issue | (EMPTY — UNRUN) | | | |
| R-7 consistent-state probe + sweep | (EMPTY — UNRUN) | | | |
| R-8 measured time + loss window | (EMPTY — UNRUN) | restore time `<EMPTY>` / loss window `<EMPTY>` | | |
| R-9 judgement vs requirements | (EMPTY — UNRUN) | | | |
| R-10 teardown + destruction record | (EMPTY — UNRUN) | | | |

### 8.2 Edge-outage test (§6)

| Step | Result | Measured value | Pass/fail | Disposition |
|---|---|---|---|---|
| E-1 edge baseline | (EMPTY — UNRUN) | | | |
| E-2 outage + degraded entry | (EMPTY — UNRUN) | outage interval `<EMPTY>` | | |
| E-3 bounded behaviour + overflow counters | (EMPTY — UNRUN) | occupancy / `dropped_oldest` `<EMPTY>` | | |
| E-4 exactly-once, in-order drain | (EMPTY — UNRUN) | drain time `<EMPTY>` | | |
| E-5 zero stale command issuance | (EMPTY — UNRUN) | rejections counted `<EMPTY>` | | |
| E-6 measured record | (EMPTY — UNRUN) | | | |

### 8.3 Validated DR procedure (ledger step 5)

The disaster-recovery procedure text that the rehearsal VALIDATED is
recorded here with its measured timings — `<EMPTY>` until the run exists.
The procedure-of-record is this document's §5/§6 as corrected by the run
(version + revision note on any correction).

## 9. Reviewer-verification procedure (human)

1. Confirm §1's plain statement and that EVERY cell of §8 is
   EMPTY/UNRUN; no restore time, loss window, or drain time is stated
   anywhere in the record.
2. Confirm every §4 RPO/RTO cell is a PLACEHOLDER with an EMPTY basis —
   no requirement was set without customer and service requirements
   (§21.2), and no measured value is presented as a requirement.
3. Trace §2's citations (`grep -n "<symbol>" <file>` and against the
   migration files): the 0008 trigger/no-DELETE-grant shape, the 0010
   partitioning + D-P5 no-retention note, the 0012 correction columns,
   the 0016 command tables, the 0018 append-only journal, both journal
   magics and their CRC framing, `commit_uploaded`/`commit_forwarded`,
   `dropped_oldest`/`has_overflowed`, `DegradedBufferingEntered`,
   `FileSequenceCheckpoint`, and the `audit_proof.rs` functions.
4. Confirm §5 R-6 states the no-re-issue criterion and §5 R-3 verifies the
   append-only properties at the SCHEMA level after restore (not assumed).
5. Confirm §7's "would not establish" list is intact and the P10.11
   cross-reference reads as a re-run, not an inheritance.
6. Confirm the §3 decision template carries no chosen tool/cadence (none
   exists, none was installed).
7. Confirm the ledger tick wording is "implemented, awaiting human
   build/test" — the plan is written; nothing has been backed up or
   restored.

## 10. Filling commands (for the human executor)

```sh
# (a) the documented-tools gate (run after ANY edit to this record):
python tools/verify_docs.py
python tools/validate_agent.py
python -I tools/check_dependency_rule.py

# (b) Fill §3 (backup configuration) and §4 (RPO/RTO requirements) from the
#     customer contract and service requirements (§21.2), with the P9.10
#     retention table (§6 of 10_data_residency_retention.md) as the
#     retention basis — a period with no cited basis is not deployable.
# (c) Take the backup per §3; run the §5 restore rehearsal into the clean
#     environment; fill §8.1 FROM THE RUN ONLY (clocks/logs/counts).
# (d) Run the §6 edge-outage test; fill §8.2 from the run.
# (e) Judge the measured values per §4's rule; record dispositions;
#     file §8 as the P9.24 evidence named in CHECKLIST.md ("Backup/restore
#     rehearsed").
# (f) Record the validated DR procedure (§8.3) with its measured timings;
#     bump this record's version on any correction the run forces.
# (g) Hand the record (not the numbers) to P9.32 (it depends on this
#     rehearsal for the audit-integrity outage leg) and note the P10.11
#     re-run obligation — Phase 10 re-measures against the release
#     candidate; it does not inherit these numbers.
```

## 11. Open risks carried to P9.27 (this subtask's contribution)

| # | Risk | Basis |
|---|---|---|
| R-P9.24-1 | The classic failure: a backup configuration that has never been restored — until §8 is filled from a real run, this plan IS that gap | §1; ledger risk row |
| R-P9.24-2 | RPO/RTO requirements cannot be set (no customer, no contract, D1 OPEN) — an actual restore run before §4 is filled would measure against nothing; the §4 judgement rule forbids fitting requirements to results | §4; §21.2 |
| R-P9.24-3 | An outbox state that replays already-issued commands after restore — gated by R-6's zero-re-issue criterion, but the criterion is only as good as the run that exercises it | §5 R-6; §15.5; ledger risk row |
| R-P9.24-4 | A restore that succeeds with an OLDER schema than the running code — gated by R-0/R-2's migration-set match, unenforced by tooling today | §5 R-0/R-2; ledger risk row |
| R-P9.24-5 | The edge audit buffer is a bounded loss window (audited drop-of-oldest), not an archive — a central outage longer than the bound provably loses the OLDEST records (counted, latched, never silent); whether that window is acceptable is a customer/authority decision | §2.2, §6 E-3; `audit_buffer.rs`; P9.20 R-6, P9.23 R-4 |
| R-P9.24-6 | Backup residency/encryption/retention cells are EMPTY while D1 is OPEN — a backup location chosen before D1 closes can violate a residency constraint discovered later | §3; `10_data_residency_retention.md` §4/§7 |
| R-P9.24-7 | JetStream backup posture undecided (stream configs are deployment artefacts that do not exist) — an event-plane gap between the last stream state and the restored DB is a data-loss-window item the current plan measures only via the DB, not the streams | §2.3, §3 |
| R-P9.24-8 | Audit-chain integrity verification is FNV-1a-64 / CRC-32 tamper-EVIDENCE, not cryptographic proof — the rehearsal verifies the mechanisms as they are; the recorded cryptographic replacements (P9.36 §9) would force a re-rehearsal | §5 R-4/R-5; §2.2 |

Status: **WRITTEN-UNRUN** — a rehearsal plan cannot be "run" by writing
it; the backup configuration, the restore, the measurements, and the edge
test are the human gates and have not happened and are not scheduled. No
backup exists; no restore was performed; no RPO/RTO was measured; none is
claimed.
