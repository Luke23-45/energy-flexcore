<!-- P12.03 — Restore drill runbook (isolated restore; timed-evidence template
     with EMPTY cells). The procedure of record is P9.24 §5 R-0..R-10 and §6
     E-1..E-6, CITED — never rewritten, never modified. P10.11 §3's RC
     re-run rule (RE-RUN, NOT INHERIT) is consumed unchanged. No drill has
     run; every measured cell is EMPTY/UNRUN; no timing threshold is
     registered here (any threshold is pre-registered via the P12.13 X-8
     template BEFORE any run that could fill it). -->
# P12.03 — Restore Drill Runbook (isolated restore — plan only, all evidence EMPTY)

| Field | Value |
|---|---|
| Subtask | P12.03 — Restore drill runbook (`ledger_phase_12.md` §P12.03) |
| Wave / agent | Phase 12 wave 1, Agent A |
| Status | **RUNBOOK — WRITTEN-UNRUN.** No drill has run; no restore has been performed; every measured cell in §5 is EMPTY/UNRUN. |
| Sources | P9.24 §5 R-0..R-10 and §6 E-1..E-6 (`ledger/phase-09-pilot-qualification/24_backup_dr_rehearsal.md` — the procedure of record, CITED not rewritten); P10.11 §3 (`ledger/phase-10-v1-release-hardening/11_backup_restore_rc.md` — the RC re-run rule, consumed); P10.11 §4 (schema-version match, stream-config match, scoped restore authorization — inherited); P12.01/§P12.02 records (the procedure artifacts the drill executes); P12.04 (the requirement cells the measured values are judged against); P12.13 evidence templates (sibling, expected path below) |
| Files touched | this record only |

## 1. Honest status — the plain statement

**No restore drill has ever run; no backup exists to restore; no deployment
exists to drill against.** This runbook adds only the Phase 12 drill
specifics on top of the procedure of record: the citation map (§2), the
application-validation steps this subtask names explicitly (§3), the
execution prerequisites (§4), and a timed-evidence template with EMPTY
cells (§5). LR-204 — a backup configuration that has never been restored —
is the gap this drill exists to close, and it stays OPEN until §5 is filled
from a real run by the human executor.

## 2. The procedure of record — citation map (NOT rewritten)

The drill executes the P9.24 procedure verbatim-in-substance. The table
below is a citation map only; the pass criteria and step semantics live in
P9.24 §5 and are not restated or modified here (the P10.11 §3 pattern).

| Drill step | Cited procedure | What the Phase 12 drill binds it to |
|---|---|---|
| R-0 | P9.24 §5 R-0 (source-state record) | the artifact IDs resolve into the P12.01 §4 target layout; the backup configuration used is the signed P9.24 §3 row set |
| R-1 | P9.24 §5 R-1 (clean/isolated environment, isolation check) | provisioning method per the P9.24 §3 row; NO access to production |
| R-2 | P9.24 §5 R-2 (restore PostgreSQL) | executed with the P12.02 procedure artifacts (`pgbackrest.conf` shape, PITR to the chosen instant) |
| R-3 | P9.24 §5 R-3 (append-only/evidence properties survive — schema-level) | the §3.3 validation below |
| R-4 | P9.24 §5 R-4 (audit integrity by COUNT and CHAIN) | the §3.2 validation below |
| R-5 | P9.24 §5 R-5 (edge-journal copies by CRC framing) | unchanged; P12.02 manifests (`snapshot_manifest.py --verify`) may be run in addition — additional evidence, never a substitute |
| R-6 | P9.24 §5 R-6 (outbox/command consistency; zero re-issue) | the §3.1 validation below |
| R-7 | P9.24 §5 R-7 (consistent-state probe + read-only API sweep) | unchanged |
| R-8 | P9.24 §5 R-8 (measured restore time + data-loss window) | into §5's EMPTY cells only |
| R-9 | P9.24 §5 R-9 (judgement vs requirements per the §4 judgement rule) | judged against the P12.04 register cells IN FORCE at the drill date |
| R-10 | P9.24 §5 R-10 (teardown + destruction record) | unchanged |
| E-1..E-6 | P9.24 §6 E-1..E-6 (edge-outage test) | carried in full; executed per its own pass criteria |

**Re-run, not inherit (P10.11 §2, consumed):** this drill's measured values
are filled from THIS drill's clocks, logs, and counts only. Nothing ever
measured by a Phase 9 or Phase 10 run (if any ever runs) is inherited; and
the P10.11 RC re-run retains its own obligation — the RC gate re-measures
against the release candidate and does not inherit this drill's numbers.

## 3. Application-validation steps (the subtask's named additions)

These are the R-3/R-4/R-6 criteria pointed at concrete checks; the criteria
themselves remain P9.24's.

### 3.1 Outbox / command consistency (the R-6 surface)

| Check | Against | Pass criterion (cited) |
|---|---|---|
| Outbox (0007) ↔ command rows (0016) agreement after restore | P9.24 §5 R-6 | the outbox and command records agree |
| Replay from the restored state | P9.24 §5 R-6 | NO already-issued command is re-published or re-issued (idempotent replay, §15.5) |
| Dispatch in flight at the snapshot instant | P9.24 §5 R-6 + §14.4 fallback rules (P9.20 register) | recovery follows the fallback rules; outcome recorded |

### 3.2 Audit completeness (the R-4 surface)

| Check | Against | Pass criterion (cited) |
|---|---|---|
| Per-table row counts vs the R-0 counts (audit_records 0008, command/transition 0016, edge journals 0018) | P9.24 §5 R-4(a) | zero count deltas, or deltas explained and counted |
| Edge-side audit chain vs its pre-outage anchor (`audit_proof.rs` :: `verify_against_anchor` / `recompute_chain_head`) | P9.24 §5 R-4(b) | anchor verification passes (FNV-1a-64 — tamper-EVIDENCE-grade, honestly bounded) |
| P11.15 keyed chain continuation after restore | record `31_p11_15_audit_chain_upgrade.md` (phase-11 records) | the restored chain verifies under the upgraded mechanism's own semantics — consumed, not redefined here |

### 3.3 Append-only property survival (the R-3 surface)

| Check | Against | Pass criterion (cited) |
|---|---|---|
| 0008 `audit_records`: trigger refuses UPDATE/DELETE; no DELETE grant to ANY role | P9.24 §5 R-3 | trigger/grant inspection transcript recorded — verified at the SCHEMA level, never assumed |
| 0018 registry + enrollment journal triggers refuse UPDATE/DELETE | P9.24 §5 R-3 | transcript recorded |
| 0016 command immutability: content columns have no update path; the column-level UPDATE grant covers exactly `state` + `superseded_by_command_id` | migration 0016 DDL (P4.10) | transcript recorded |
| 0012 correction rows: corrections are NEW rows; originals unmuted | P9.24 §5 R-3 | transcript recorded |

## 4. Execution prerequisites (all must hold before the drill runs)

| # | Prerequisite | Status |
|---|---|---|
| 1 | A real deployment exists (control plane, PostgreSQL, NATS JetStream) | NOT MET — none exists (P9.24 §2.3) |
| 2 | The authoritative P9.24 §3 backup-configuration cells are filled by the human owner | `<EMPTY>` |
| 3 | The P9.24 §4 / P12.04 requirement cells in force at the drill date are filled (an un-judged restore measures against nothing — P9.24 §11 R-P9.24-2) | `<EMPTY>` |
| 4 | A backup exists and its artifact IDs resolve (P12.02 procedures executed at least once) | NOT MET |
| 5 | Any timing threshold the drill will judge against is pre-registered (X-8) in the P12.13 template BEFORE the run | P12.13 sibling — expected path `runbooks/evidence_templates.md` (not yet written; cited by expected path per plan §2.1) |
| 6 | The isolated environment's NATS server version carries the CVE-2026-33222 fix rule (inherited from P10.11 §4.4 when stream restore is in scope) | `<EMPTY — UNRUN>` |

## 5. Timed-evidence template — ALL CELLS EMPTY (UNRUN)

Filled ONLY from a real run's clocks, logs, and counts; a filled cell
without a run is a fabrication defect (the P9.24 §8 rule, inherited). The
full evidence pack (including the E-1..E-6 edge-outage rows and the
X-8 pre-registration header) is the P12.13 deliverable (expected path
`runbooks/evidence_templates.md`); §5 carries the drill-specific
timing skeleton.

**Drill metadata (all EMPTY):** dates / executor / witness / isolated-environment
ID / artifact IDs restored / software versions + migration set / backup
configuration row-version used / requirement row-version in force.

### 5.1 Isolated restore timings

| Step | Result | Measured value | Pass/fail | Disposition |
|---|---|---|---|---|
| DR-1 source-state record (R-0) | (EMPTY — UNRUN) | | | |
| DR-2 isolation check (R-1) | (EMPTY — UNRUN) | | | |
| DR-3 PostgreSQL restore (R-2) | (EMPTY — UNRUN) | restore wall time `<EMPTY>` | | |
| DR-4 PITR target-instant reach (R-2) | (EMPTY — UNRUN) | WAL replay duration `<EMPTY>` | | |
| DR-5 append-only survival checks (R-3, §3.3) | (EMPTY — UNRUN) | | | |
| DR-6 audit count + chain (R-4, §3.2) | (EMPTY — UNRUN) | | | |
| DR-7 edge-journal CRC replay (R-5) | (EMPTY — UNRUN) | | | |
| DR-8 outbox/command consistency + zero re-issue (R-6, §3.1) | (EMPTY — UNRUN) | re-issue count `<EMPTY>` | | |
| DR-9 consistent-state probe + sweep (R-7) | (EMPTY — UNRUN) | | | |
| DR-10 judgement vs requirements (R-9) | (EMPTY — UNRUN) | | | |
| DR-11 teardown + destruction (R-10) | (EMPTY — UNRUN) | | | |

### 5.2 Judgement rule (inherited verbatim-in-substance from P9.24 §4)

The measured values are recorded against the requirements in force; a
measured value WORSE than the requirement is a FINDING with a disposition
(fix and re-drill / signed time-bounded acceptance / blocks the gate) —
never a re-defined requirement, never a requirement fitted to a result.

## 6. Reviewer-verification procedure (human)

1. Confirm §1's plain statement and that EVERY cell of §5 is EMPTY/UNRUN;
   no restore time, loss window, or re-issue count is stated anywhere.
2. Confirm §2 cites P9.24 §5 R-0..R-10 and §6 E-1..E-6 WITHOUT restating or
   modifying the pass criteria, and that the P10.11 re-run-not-inherit rule
   is consumed unchanged.
3. Confirm §3's checks trace to the migration files (0007/0008/0012/0016/
   0018) and to `crates/edge-agent/src/audit_proof.rs`, and that each
   criterion is cited, not re-decided.
4. Confirm §4 marks every prerequisite's real status and that the X-8
   pre-registration obligation is stated before any threshold exists.
5. Confirm the P12.13 sibling citation uses the expected-path pattern and
   is marked as not-yet-written.
6. Confirm no checkbox in `ledger_phase_12.md` was touched by this subtask.

## 7. Filling commands (for the human executor)

```sh
# (a) the documented-tools gate (run after ANY edit to this record):
python tools/verify_docs.py
python tools/validate_agent.py
python -I tools/check_dependency_rule.py

# (b) Fill §5 FROM THE RUN ONLY (clocks/logs/counts), after prerequisites
#     §4.1–§4.6 hold. Pre-register any threshold (X-8) in the P12.13
#     template BEFORE the run that could fill it.
# (c) Judge per §5.2's inherited rule; record dispositions; hand the
#     record to the phase close (P12.16) and to the P10.11 RC re-run
#     obligation (which re-measures, never inherits).
```

## 8. Non-claims

No drill ran; no restore was performed; no measured time or loss window
exists; no threshold is registered; no restorability is claimed for any
backup (none exists). A single run's outcome is one run's outcome, not a
service-level guarantee (§21.2 honesty rule; conventions §5). No dispatch
authority is added or implied (ADR-010).

Status: **WRITTEN-UNRUN** — a runbook cannot restore anything by being
written; the drill is the human gate and has not happened and is not
scheduled.
