<!-- P10.11 — Backup/restore test on the release candidate. TEMPLATE + RC test
     plan: RE-RUN, NOT INHERIT (the Phase 9 record's own rule, cited in §2).
     The procedure is CITED from P9.24, not rewritten. Every execution cell is
     EMPTY: no release candidate exists (ENTRY_NOTE boundary 1), no backup of
     any candidate has been taken, no restore has been performed on any
     candidate, no measured time or loss window exists, and the edge-outage
     re-test E-1..E-6 is carried UNRUN. RPO/RTO stay PLACEHOLDER requirements
     (§21.2: do not set RPO/RTO without customer and service requirements). -->
# P10.11 — Backup/Restore Test on the Release Candidate (RC re-run plan — nothing has been backed up or restored on any candidate)

Date: 2026-11-14. Subtask: P10.11 (`ledger_phase_10.md` §P10.11). Owner role:
Platform engineering. Report trace: §22.3 release gate 9 (first half); §21.2;
§17.3. Inputs: P10.01 scope freeze; **P9.24** (`ledger/phase-09-pilot-qualification/24_backup_dr_rehearsal.md` — the rehearsal procedure this subtask
re-runs, cited not rewritten); P9.10 retention design
(`ledger/phase-09-pilot-qualification/10_data_residency_retention.md` — its §6
edge rows and retention table stay the basis); the release-candidate
deployment — **which does not exist** (ENTRY_NOTE boundary 1: the candidate is
a described build procedure, never a built artifact).

## 1. Honest status — the plain statement

**No release candidate exists; no backup has been taken of any candidate; no
restore has been performed on any candidate; no restore time or data-loss
window has been measured on any candidate; the edge-outage re-test has not
run.** Phase 9's rehearsal itself is also UNRUN (its P9.24 record is a PLAN
with an EMPTY record — §8 there). This record delivers the **RC re-run plan**
(§3) that executes the P9.24 procedure against the release candidate, the
RC-specific verifications the ledger adds on top (§4 — schema-version match,
JetStream stream-config match, outbox/inbox survival, scoped restore
authorization), and the **EMPTY run record** (§6). The ledger's acceptance
criteria are NOT met and NOT claimed: no restore was performed and verified
on the candidate; no restored-record completeness was checked; no measured
time/loss window exists; no stream configuration was compared against the
P1.14 design on a real restore.

## 2. Re-run, NOT inherit — the Phase 9 record's own rule

The P9.24 record states the rule this subtask operates under (its §7,
"would not establish" list, closing sentence): *"NOT the Phase 10 evidence —
P10.11 re-runs this rehearsal against the release candidate (cross-reference
per the ledger: Phase 10 does not inherit this run's numbers, it re-measures)."*
And its §10 filling step (g): *"Hand the record (not the numbers) to P9.32 …
and note the P10.11 re-run obligation — Phase 10 re-measures against the
release candidate; it does not inherit these numbers."*

Consequently:

1. **The PROCEDURE is cited; the NUMBERS are not.** §3 executes the P9.24 §5
   R-0..R-10 and §6 E-1..E-6 steps by citation; nothing measured in Phase 9
   (if anything ever is) is carried into this record's §6 — every measured
   cell here is filled from THIS re-run's clocks, logs, and counts only.
2. **The re-run target is a different artifact.** The candidate build is a
   different artifact from the pilot build (the ledger preamble: re-running
   against the frozen candidate "is verification, not duplication"). A run
   against any non-candidate build is recorded as `WRONG_ARTIFACT` — a
   finding, not a result (the ledger's named risk: "the test run against the
   pilot build instead of the candidate").
3. **The RPO/RTO requirement cells are re-opened here, not copied.** §5
   restates the requirement template with EMPTY cells; if Phase 9's cells
   are ever filled from a customer contract, THIS record still cites the
   customer requirements current at the candidate's gate date and judges the
   candidate's measured values against them.

## 3. The RC re-run procedure — by citation of P9.24 (the procedure is NOT rewritten)

The re-run executes the P9.24 procedure verbatim-in-substance against the
release candidate. The step table below is the citation map; the pass
criteria and pass/fail semantics live in P9.24 §5 and are NOT restated or
modified here.

| RC re-run step | Cited procedure | What the RC execution adds |
|---|---|---|
| RC-R-0 | P9.24 §5 R-0 (source-state record) | record the **release-candidate identifier** (P10.18 commit/tag/build ID + artifact hashes) alongside the versions/migration-set/counts; the identifier must match the frozen candidate — else `WRONG_ARTIFACT` |
| RC-R-1 | P9.24 §5 R-1 (clean-environment provisioning + isolation check) | identical; isolation never faces production |
| RC-R-2 | P9.24 §5 R-2 (restore into the clean environment) | identical; the §4 schema-version check below is the RC addition |
| RC-R-3 | P9.24 §5 R-3 (append-only/evidence properties survive the restore — schema-level trigger/grant inspection) | identical; the 0008 audit-records trigger/no-DELETE-grant, 0018 registry/journal triggers, 0012 correction columns |
| RC-R-4 | P9.24 §5 R-4 (audit integrity by COUNT and CHAIN) | identical; count-per-table equals RC-R-0's counts; edge audit chain via `crates/edge-agent/src/audit_proof.rs` :: `verify_against_anchor` / `recompute_chain_head` (FNV-1a-64 chain — tamper-EVIDENCE-grade, honestly bounded) |
| RC-R-5 | P9.24 §5 R-5 (edge-journal copies verified by FRAMING/CRC, by citation) | identical; `crates/edge-agent/src/buffer_file.rs` (magic `FLEXCORE-EDGE-TELEMETRY-JOURNAL` + version byte 0x01, `[u32 LE len][u32 LE crc32][payload]` framing) and `crates/edge-agent/src/audit_buffer.rs` (magic `FLEXCORE-EDGE- AUDIT-JOURNAL` + version byte 0x01, same framing) re-opened by the same reader mechanics; strict decode; truncate-at-first-corrupt with a counted `corrupt_tail_truncated` — a copy failing CRC is a COUNTED LOSS, never silently accepted |
| RC-R-6 | P9.24 §5 R-6 (command/outbox consistency; idempotent replay) | identical; zero re-issued command IDs (§6.3/§15.5); the RC addition in §4.3 below |
| RC-R-7 | P9.24 §5 R-7 (consistent-state probe + read-only API sweep) | identical; `/ready` probe; `/api/v1` sweep; command-state vocabulary round-trip |
| RC-R-8 | P9.24 §5 R-8 (measured restore time + data-loss window) | identical; numbers from THIS run's clocks/logs, never estimated — into §6, EMPTY now |
| RC-R-9 | P9.24 §5 R-9 (judgement vs requirements per the §4 judgement rule) | judged against §5's RC requirement cells; a measured value worse than the requirement is a FINDING with a disposition (fix and re-run / signed time-bounded acceptance carried to P10.26/P10.27 / blocks the go decision) — never a re-defined requirement |
| RC-R-10 | P9.24 §5 R-10 (teardown + destruction record) | identical; destruction recorded (residency/retention per P9.10 §6) |
| RC-E-1..E-6 | P9.24 §6 E-1..E-6 (edge-outage test — carried, see §4.4) | re-run against the candidate edge build; the edge build version must match the release candidate |

## 4. RC-specific verifications (the ledger's steps 4–5 — additions on top of P9.24)

### 4.1 Schema version of the restored data matches the candidate (ledger step 4)

The restored PostgreSQL migration set (per `crates/adapters/postgres/migrations/`)
must EQUAL the migration set the candidate control plane expects — a restore
that succeeds with an OLDER schema is the ledger's named risk and a FINDING
(`SCHEMA_VERSION_MISMATCH`). The check is recorded at RC-R-0/RC-R-2 and
cross-references **P10.13** (schema-migration compatibility test — concurrent
wave-3 sibling; its per-migration compatibility windows are the input this
check consumes; at the time of writing P10.13's record does not exist yet and
is cited by its expected path
`ledger/phase-10-v1-release-hardening/13_schema_migration_compat.md`).

### 4.2 JetStream stream configuration matches the P1.14 design (ledger step 5, first half)

The restored NATS JetStream configuration (subjects, streams, retention,
replication, acknowledgement, replay, dead-letter/quarantine policy per
§15.4) is compared against the P1.14 topology design
(`ledger/phase-01-rust-foundation/14_jetstream_topology.md` — the frozen
stream/subject mapping the P10.02 contract freeze carries). A stream or
subject that does not match the P1.14 design is a FINDING
(`STREAM_CONFIG_MISMATCH`), not a tolerated drift. Note the honest status:
stream configs are deployment artefacts that do not exist yet (P9.24 §2.3/§3
— the JetStream backup posture is a `<EMPTY>` decision cell there); the
comparison is executable only once a candidate deployment exists.

### 4.3 Outbox/inbox survival and idempotent replay (ledger step 5, second half)

Pending `outbox` (migration 0007) entries and inbox/idempotency records must
survive the restore: replay from the restored state issues NO duplicate
command (§15.5; the P9.24 R-6 criterion, re-asserted on the candidate). On
the edge side, the restart-surviving idempotency bindings
(`crates/edge-agent/src/pilot_rejection_proof.rs` :: `FileCommandIdBinding`
— command_id ↔ payload, refusing a conflicting payload under a used id) and
the monotonic sequence checkpoint (`FileSequenceCheckpoint`) must survive
and keep rejecting replayed pre-outage commands (the P9.24 E-5 criterion).

### 4.4 Scoped JetStream restore authorization (ledger step 5, third half — the CVE surface)

The restore is performed with **scoped JetStream restore authorization
only**; a restore to an unauthorized stream name must be REFUSED. This is
the CVE-2026-33222 restore-authorization surface (JetStream stream-restore
endpoint auth bypass, GHSA-9983-vrx2-fg9c, CVSS 4.9; fixed in NATS
**2.11.15 / 2.12.6** — every 2.15.x release includes the fix; sibling
CVE-2026-33223, Nats-Request-Info header spoof, same fix versions; Phase 1
`RESEARCH.md` records the CORRECTED fix-version history and the running-rule
"do not run a version affected by CVE-2026-33222 without the fix"). The RC
re-run records: the NATS server version in the clean environment (verified
against the fix-version rule above), the granted restore authorization scope,
and the observed refusal of an unauthorized stream-name restore
(`RESTORE_AUTHORIZATION_UNSCOPED` is the finding if the refusal does not
happen). No refusal evidence exists — the cell is EMPTY.

### 4.5 Edge-outage re-test E-1..E-6 — CARRIED, UNRUN

The P9.24 §6 edge-outage test is carried in full (E-1 edge baseline counters
— `FileAuditBuffer::appended_total` / `pending_len`, `FileLocalBuffer`
equivalents, `FileSequenceCheckpoint` state, `FileCommandIdBinding::len`;
E-2 stop-the-central degraded entry — `DegradedBufferingEntered` audited;
E-3 bounded behaviour — audited drop-of-OLDEST with `dropped_oldest`
incremented and `has_overflowed` LATCHED, never silent, never unbounded;
E-4 exactly-once in-order drain with fail-closed `commit_uploaded` and
`DegradedBufferingExited`; E-5 zero stale/expired/replayed command issuance
— expiry vs the edge clock with the clock-quality fail-closed rule, any
quality other than `Synchronized` failing the expiry check, per
`crates/edge-agent/src/command_verify.rs` — plus the sequence checkpoint;
E-6 the measured record). The re-run runs against the candidate edge build;
its results go in §6.2, EMPTY. Together with RC-R-2..R-9 this is the §21.2
degraded-mode evidence for the candidate (P9.24 §6 closing paragraph's
division of labour stands: the two records cross-reference, they do not
duplicate).

## 5. RPO/RTO on the RC — PLACEHOLDER requirements, EMPTY (§21.2 rule)

The §21.2 rule is unchanged for the RC gate: do not set RPO/RTO without
customer and service requirements — record the measured values and the
requirements they are judged against. The requirement cells are re-opened
for the candidate gate (§2 rule 3) and are EMPTY:

| Requirement | Value | Basis |
|---|---|---|
| RPO requirement in force at the candidate gate | `<EMPTY — PLACEHOLDER>` | Customer contract + service requirements (§21.2); P9.10 retention table |
| RTO requirement in force at the candidate gate | `<EMPTY — PLACEHOLDER>` | Same |
| Per-class RPO delta (audit evidence vs telemetry) | `<EMPTY>` | §17.3/§16.3 evidence classes; the customer decides |
| Measured restore time (candidate, from §6.1 RC-R-8) | `<EMPTY — UNMEASURED>` | Filled ONLY from the RC re-run |
| Measured data-loss window (candidate, from §6.1 RC-R-8) | `<EMPTY — UNMEASURED>` | Filled ONLY from the RC re-run |

Judgement rule (pre-printed, identical discipline to P9.24 §4): the measured
values are recorded against the requirements; a measured value WORSE than
the requirement is a FINDING with a disposition — never a re-defined
requirement, never a requirement fitted to a result.

## 6. The RC run record — ALL CELLS EMPTY (UNRUN)

Filled ONLY from a real run's clocks, logs, and counts; a filled cell
without a run is a fabrication defect.

**Run metadata (all EMPTY):** dates / rehearser / witness / clean-environment
ID / source-snapshot artifact IDs / **release-candidate identifier (must
equal the P10.18 candidate)** / software versions + migration set / backup
configuration used (the P9.24 §3 decision-row version) / RPO-RTO requirement
cells in force (the §5 row version) / NATS server version + CVE-2026-33222
fix-status check (§4.4).

### 6.1 Restore rehearsal on the candidate (§3)

| Step | Result | Measured value | Pass/fail | Disposition |
|---|---|---|---|---|
| RC-R-0 source state + candidate ID | (EMPTY — UNRUN) | | | |
| RC-R-1 isolation | (EMPTY — UNRUN) | | | |
| RC-R-2 restore | (EMPTY — UNRUN) | | | |
| RC-R-2b schema-version match (§4.1) | (EMPTY — UNRUN) | | | |
| RC-R-3 append-only properties survive | (EMPTY — UNRUN) | | | |
| RC-R-4 audit count + chain | (EMPTY — UNRUN) | | | |
| RC-R-5 edge-journal CRC replay | (EMPTY — UNRUN) | | | |
| RC-R-6 outbox/command consistency, zero re-issue | (EMPTY — UNRUN) | | | |
| RC-R-7 consistent-state probe + sweep | (EMPTY — UNRUN) | | | |
| RC-R-8 measured time + loss window | (EMPTY — UNRUN) | restore time `<EMPTY>` / loss window `<EMPTY>` | | |
| RC-R-9 judgement vs requirements | (EMPTY — UNRUN) | | | |
| RC-R-10 teardown + destruction record | (EMPTY — UNRUN) | | | |

### 6.2 Edge-outage re-test on the candidate (§4.5 — E-1..E-6 carried)

| Step | Result | Measured value | Pass/fail | Disposition |
|---|---|---|---|---|
| E-1 edge baseline | (EMPTY — UNRUN) | | | |
| E-2 outage + degraded entry | (EMPTY — UNRUN) | outage interval `<EMPTY>` | | |
| E-3 bounded behaviour + overflow counters | (EMPTY — UNRUN) | occupancy / `dropped_oldest` `<EMPTY>` | | |
| E-4 exactly-once, in-order drain | (EMPTY — UNRUN) | drain time `<EMPTY>` | | |
| E-5 zero stale command issuance | (EMPTY — UNRUN) | rejections counted `<EMPTY>` | | |
| E-6 measured record | (EMPTY — UNRUN) | | | |

### 6.3 JetStream verification on the candidate (§4.2/§4.3/§4.4)

| Check | Result | Evidence |
|---|---|---|
| Restored stream config matches the P1.14 design | (EMPTY — UNRUN) | comparison transcript `<EMPTY>` |
| Outbox/inbox state survived; zero duplicate publication | (EMPTY — UNRUN) | replay audit `<EMPTY>` |
| Scoped restore authorization; unauthorized stream-name restore refused | (EMPTY — UNRUN) | refusal transcript `<EMPTY>` |
| NATS server version carries the CVE-2026-33222 fix | (EMPTY — UNRUN) | version record `<EMPTY>` |

## 7. Reviewer-verification procedure (human)

1. Confirm §1's plain statement and that EVERY cell of §6 is EMPTY/UNRUN;
   no restore time, loss window, drain time, or refusal transcript is stated
   anywhere in this record.
2. Confirm §2 cites the P9.24 re-run rule and that this record contains NO
   inherited Phase 9 number (there are none to inherit — P9.24's record is
   itself EMPTY; the citation is of the RULE and the PROCEDURE, never of
   results).
3. Trace §3's citations into the actual sources: the P9.24 §5 R-0..R-10 and
   §6 E-1..E-6 step tables; `audit_proof.rs` :: `verify_against_anchor` /
   `recompute_chain_head`; both journal magics and the CRC framing in
   `buffer_file.rs` / `audit_buffer.rs`; `FileCommandIdBinding`,
   `FileSequenceCheckpoint`, `DegradedBufferingEntered`,
   `commit_uploaded`, `dropped_oldest` / `has_overflowed`; the clock-quality
   fail-closed rule in `command_verify.rs`; the 0007/0008/0012/0016/0018
   migration properties (grep the migration files); the P1.14 topology
   record `ledger/phase-01-rust-foundation/14_jetstream_topology.md`.
4. Confirm §4.4's CVE statement matches Phase 1 `RESEARCH.md` (fix versions
   2.11.15/2.12.6; 2.15.x includes the fix) and that the scoped-restore
   refusal requirement is stated as a pass criterion, not assumed.
5. Confirm §5's RPO/RTO cells are PLACEHOLDERS with EMPTY bases (no
   requirement set without customer and service requirements) and that the
   judgement rule forbids fitting requirements to results.
6. Confirm the ledger tick wording is "implemented, awaiting human
   build/test" — the RC re-run plan is written; nothing has been backed up
   or restored on any candidate.

## 8. Filling commands (for the human executor)

```sh
# (a) the documented-tools gate (run after ANY edit to this record):
python tools/verify_docs.py
python tools/validate_agent.py
python -I tools/check_dependency_rule.py

# (b) Fill §5 (RPO/RTO requirements in force at the candidate gate) from the
#     customer contract and service requirements (§21.2) with the P9.10
#     retention table as the retention basis — a period with no cited basis
#     is not deployable. Also fill the P9.24 §3 backup-configuration cells
#     the run will use (no tool is chosen here; none was installed).
# (c) Build/obtain the P10.18 release candidate; record its identifier; run
#     the §3 procedure (P9.24 §5 R-0..R-10 by citation) against the CANDIDATE
#     in the isolated clean environment; fill §6.1 FROM THE RUN ONLY.
# (d) Verify §4.1 (schema version), §4.2 (stream config vs P1.14), §4.3
#     (outbox/inbox survival, zero re-issue), §4.4 (scoped restore
#     authorization + unauthorized-stream-name refusal + NATS fix version);
#     fill §6.3 from the transcripts.
# (e) Run the §4.5 edge-outage re-test (E-1..E-6) against the candidate edge
#     build; fill §6.2 from the run.
# (f) Judge the measured values per §5's rule; record dispositions; file the
#     record as the P10.11 evidence named in CHECKLIST.md; hand the result
#     (not just the numbers — the whole record) to P10.17 (operational risk)
#     and P10.27 (go/no-go).
```

## 9. Findings (recorded, not fixed)

- **F-P10.11-1 (dependency gap):** the release candidate does not exist
  (ENTRY_NOTE boundary 1); the P10.18 candidate identifier, the backup
  configuration cells (P9.24 §3), and the JetStream deployment artefacts
  (P9.24 §2.3/§3) are all EMPTY — the re-run is structurally unstartable
  until all three exist. Recorded, not papered over.
- **F-P10.11-2 (cross-reference gap):** P10.13 (schema-migration
  compatibility, ledger step 4's input) is a concurrent wave-3 subtask whose
  record does not exist yet; §4.1 cites its expected path. The RC restore's
  schema-version check consumes P10.13's compatibility windows once they
  exist.
- **F-P10.11-3 (carried from P9.24 §11, unchanged):** the edge audit buffer
  is a bounded loss window (audited drop-of-oldest), not an archive — a
  central outage longer than the bound provably loses the OLDEST records
  (counted, latched, never silent); whether that window is acceptable is a
  customer/authority decision, carried to P10.17/P10.26.
- **F-P10.11-4 (carried, unchanged):** audit-chain integrity verification is
  FNV-1a-64 / CRC-32 tamper-EVIDENCE, not cryptographic proof — the RC
  re-run verifies the mechanisms as they are; the recorded cryptographic
  replacements (P9.36 §9) would force a re-rehearsal of this subtask too.
- **F-P10.11-5 (carried, unchanged):** JetStream backup posture undecided
  (stream configs do not exist) — an event-plane gap between the last stream
  state and the restored DB is a data-loss-window item §6.3 makes visible
  but cannot currently measure; the R-8 loss window measures the DB, not the
  streams.

## 10. Non-claims

No candidate backup exists; no candidate restore was performed; no measured
restore time or loss window exists; no edge-outage re-test ran; no stream
configuration was compared; no CVE-33222 fix-status was verified on a real
server; no RPO/RTO requirement was set or met. Nothing here is a
disaster-recovery guarantee — a measured value from a single re-run is one
run's outcome, not a service-level guarantee (§21.2 honesty rule), and
nothing here grants protection-grade or live-control authority (AGENTS.md
rules 3, 5).

Status: **WRITTEN-UNRUN** — an RC re-run plan cannot "run" by being written;
the candidate, the backup configuration, the restore, the measurements, and
the edge re-test are the human gates and have not happened and are not
scheduled.
