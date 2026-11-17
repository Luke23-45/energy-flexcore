<!-- P12.16 — Decision records + phase close (close/exit-gate record).
     CLOSE RECORD ONLY: this record reports the phase's written artifacts,
     verifies the E-decision register state, and carries the consistency
     audit. It makes NO run, build, backup, restore, drill, fault-injection,
     game-day, or timing claim — nothing in this phase has executed; every
     evidence cell in every artifact it reports is EMPTY/UNRUN. The ledger
     ticks for P12.13–P12.16 are the ORCHESTRATOR's action (conventions
     §2.2) — no agent ticked a checkbox. -->
# P12.16 — Decision Records and Phase Close (close/exit-gate record)

| Field | Value |
|---|---|
| Subtask | P12.16 (decision records + close: the P12-E-1/E-2/E-3 register verification, per-batch decision records, the cross-subtask consistency audit, HANDOVER + CHECKLIST + this close record) |
| Wave / agent | Phase 12 wave 2, Agent B |
| Status | WRITTEN-UNRUN close: this record reports the phase's written artifacts, quotes the three permitted-tool outputs verbatim in §2, and carries the consistency audit in §3. It makes NO claim about any unrun script, undeployed design, or undrilled procedure. |
| Sources | Program conventions (`../../01_PROGRAM_CONVENTIONS.md` §2 — ledger discipline and the close deliverables; §4 — agents never close E-decisions; §5 — human gates and §13.3 semantics); the phase plan §2 (P12.15–P12.16 rows), §4 (the three E-decision entries), §8 (the phase's acceptance criteria); the wave close-outs (`records/19`, `records/27`, `records/33`); the gap analysis §11 (the [S-1]..[S-172] register) |
| Files touched | `records/51_p12_15_resilience_acceptance_criteria.md` (new), `HANDOVER.md` (new), `CHECKLIST.md` (new), this record. The ledger, plan.md, context.md, `docs/foundations/*`, `docs/plan/*`, `.agent/*`, and every other agent's files are untouched — no checkbox was ticked by this agent. |
| Audit scope | §3 covers every phase-12 artifact PRESENT on disk at close time (2026-11-14): `context.md`, `plan.md`, `ledger_phase_12.md`, `HANDOVER.md`, `CHECKLIST.md`, records 01–06, 19, 21–27, 31–33, 51, and 52, the runbooks tree (`game_day_template.md`, `dr_site_loss.md`, `evidence_templates.md`), the experiments tree (5 Python scripts), and `deploy/compose/backup/` (7 files). Agent A's wave-2 records (31–33) and the P12.13 template file LANDED during this close (wait-checked, then re-read — the audit ran after the last of them, record 33, was present); the audit scope is stated here explicitly per the close instruction. |

## 1. The close position (expected end state)

Phase 12's implementation is complete: 16 of 16 subtasks implemented
(P12.01–P12.06 wave 1 Agent A; P12.07–P12.12 wave 1 Agent B; P12.13–P12.14
wave 2 Agent A; P12.15–P12.16 wave 2 Agent B), every one with a decision
record under `records/`. **The ledger's P12.13–P12.16 ticks are the
ORCHESTRATOR's action**, applied after this close per program conventions
§2.2 ("a checkbox is ticked only as 'implemented, awaiting human
build/test' with a decision-record citation") — no agent ticks a checkbox,
and this record writes the EXPECTED end state, not an applied one:

| Ledger row | Status at close | Expected citation (applied by the orchestrator) |
|---|---|---|
| P12.01–P12.12 | TICKED (by the orchestrator) — each citation verified to resolve to a file on disk (§3.1) | records 01–06, 21–26 (as already cited) |
| P12.13 | unticked at close | `records/31_p12_13_evidence_templates.md` |
| P12.14 | unticked at close | `records/32_p12_14_jetstream_rehearsal.md` |
| P12.15 | unticked at close | `records/51_p12_15_resilience_acceptance_criteria.md` |
| P12.16 | unticked at close | this record |

The tick text stays "implemented, awaiting human build/test" — never
"tested", "verified", "passed", or "works".

**E-decision register state at close (verified; no agent closed anything;
no register entry was missing, so none was added):**

| ID | Status | Register record (verified on disk) | DEFAULT (RECOMMENDATION) | Closer |
|---|---|---|---|---|
| P12-E-1 (standby topology + sync level) | OPEN | `records/05_p12_05_standby_replication.md` §4 (registered at wave 1) | one Patroni primary + one standby; `remote_apply` for the 0016 command/transition and 0008 audit transactions IF the measured commit-latency cost is accepted; `remote_write` fallback; DEPLOYMENT-gated | human owner, with the F-2/F-6 latency evidence and Phase 11 capacity signals |
| P12-E-2 (JetStream backup posture) | OPEN | `records/02_p12_02_backup_procedures.md` §4 (registered at wave 1); the P12.14 dependence recorded in `records/32_p12_14_jetstream_rehearsal.md` §2 without closing it | snapshot-based stream backup into the shared object-storage target; `num_replicas: 1` unchanged at design time (D-J5 holds); replication revisited at the second-cluster trigger | human owner, with the P12.14 JS-9 loss-window evidence |
| P12-E-3 (second DR site necessity) | OPEN | `records/25_p12_11_dr_runbook.md` §7, carried in `runbooks/dr_site_loss.md` §7 (registered at wave 1) | no second site in the current posture — DR is the site-loss runbook + the restore-into-isolation rehearsal; revisit at the Phase 20 multi-region trigger | human owner |

All three are mirrored in `plan.md` §4 and in `ledger_phase_12.md`
§Decision points as OPEN with their DEFAULTs — the register verification
the subtask calls for found no missing entry (each E-decision was already
registered by its wave-1 record exactly as the dispatch brief expected),
so no new register record was created in the 51–69 range; this table is
the verification of record. **No E-decision was closed by any agent**
(conventions §4).

## 2. Permitted-tool outputs (verbatim)

Run 2026-11-14, after `records/51`, `HANDOVER.md`, and `CHECKLIST.md`
were written and after the §3 audit sweeps; this close record quotes the
outputs and was the last file written. The tools check the report and the
agent harness, not phase records (the wave-1 close-outs' deviation 4
framing) — pasting the outputs here cannot change them.

`python tools/verify_docs.py`:

```text
report: docs\plan\Neryva_FlexCore_Product_and_Architecture_Report_v1.1.md
lines: 2241  sections: 28  headings: 127  tables: 32
OK: headings, section references, fences, tables, references, TOC
EXIT=0
```

`python tools/validate_agent.py`:

```text
skills: 5  roles: 5
OK: skills, roles, and README index are valid
EXIT=0
```

`python -I tools/check_dependency_rule.py` (the character rendered as
`ù` below is the console mojibake of the tool's separator glyph in this
Git Bash session — the same rendering all three prior phase-12 close-outs
record; quoted verbatim as required):

```text
OK ù dependency rule holds for 20 workspace members.
EXIT=0
```

All three exited 0. Per the honesty rules this states the tools' own
output about the report, the harness, and the workspace dependency graph —
not a claim about the unrun scripts, undeployed designs, or undrilled
procedures this phase delivered.

## 3. Consistency audit (performed LAST, over every artifact present at close)

The audit is an INSPECTION (grep/read-based), not a tool run; it makes no
claim beyond what was inspected. Agent A's wave-2 records (31–33) and
`runbooks/evidence_templates.md` landed during this close; they were
wait-checked for, then read in full, and the audit below includes them.

### 3.1 Ledger decision-record citations

- 16 subtask rows exist in `ledger_phase_12.md`; the 12 ticked rows
  (P12.01–P12.12) each carry a `(decision record: records/NN_*.md)`
  citation, and EVERY cited file resolves on disk (verified file by file:
  records 01–06, 21–26 — 12 of 12 EXIST).
- The 4 unticked rows (P12.13–P12.16) name their evidence in prose and
  await the orchestrator's citations per §1. No checkbox was ticked by
  this agent anywhere.

### 3.2 Table pipe counts

Every markdown table in the phase directory (`context.md`, `plan.md`,
`ledger_phase_12.md`, `HANDOVER.md`, `CHECKLIST.md`, all 19 records in
`records/` at close — this record included — and the 3 runbooks) was
swept for inconsistent pipe counts WITHIN each table (fenced code blocks
excluded): **zero inconsistencies found**.

### 3.3 [S-n] citation resolution

The set of `[S-n]` citations across the entire phase tree (including the
records, runbooks, experiments scripts, and `deploy/compose/backup/`) is
exactly: S-61, S-65, S-67, S-68, S-69, S-70, S-81, S-82. All eight
resolve to entries in the gap analysis §11 register (S-1..S-172 verified
against `00_GAP_ANALYSIS.md`) — **zero out-of-range citations**. The
register's UNVERIFIED items are never cited as fact: [S-68] is a
secondary comparison whose every capability row is marked UNVERIFIED
(record 01 §2); [S-65]'s `nats stream backup` CLI surface is UNVERIFIED
and NO restore-command shape is asserted anywhere (record 32 §2);
[S-81]/[S-82] are cited as practice discipline, never as conformance
sources (record 22 §3).

### 3.4 Honesty sweep (no run/build/backup/restore/drill/fault-injection/game-day/timing claims)

Sweep over every markdown artifact in the phase directory plus grep
sweeps over the Python/sh/config artifacts:

- No artifact claims a run, build, backup, restore, drill, fault
  injection, game day, snapshot, deletion, measurement, refusal
  observation, or deployment. Every execution-shaped statement is negated
  or labeled WRITTEN-UNRUN / UNRUN / UNDEPLOYED / UNMEASURED / EMPTY /
  UNSIGNED / NOT SENT / NOT GRANTED. The three permitted tools' outputs
  (§2) are the only executed programs in this phase.
- Every evidence cell in every template is EMPTY (record 03 §5, record 05
  §5, record 32 §6, `runbooks/dr_site_loss.md` §6,
  `runbooks/evidence_templates.md` §1–§7, `runbooks/game_day_template.md`
  §1–§7); the §1 threshold register's EMPTY state is its correct pre-run
  state (X-8) — no threshold is registered, and none was invented to make
  the register look used (record 31 §3.5).
- No `TODO`/`FIXME`/`XXX`/`unimplemented`/`todo!` placeholder exists
  anywhere in the phase tree (swept; the `<EMPTY>` and `<PLACEHOLDER>`
  cells are the intentional honest markers the conventions require, not
  placeholders).
- The RTO/RPO values (record 04) all carry RECOMMENDATION with EMPTY
  sign-off/basis cells; the T7 row asserts no recommendation at all (the
  no-invention rule); the judgement rule is pre-printed unsoftened (§4).
- No conformance, certification, safety, or protection-grade claim exists
  anywhere; the central-outage rule is cited, never re-argued; ADR-010
  holds in every record — no dispatch authority is added by any phase-12
  artifact, and the never-granted permission set is untouched.
- §13.3 semantics ("zero observed violations from a finite test set is a
  test outcome, never a guarantee") is stated in every record that could
  imply a capability and is generalized once in record 51 §2 with the
  claim-escalation rung ladder.

### 3.5 The cadence-consistency triangles

**Triangle 1 — retention ↔ RTO/RPO cadence ↔ snapshot posture (the
dispatch brief's triangle, records 01 ↔ 04 ↔ P12-E-2):**

| Edge | Verified consistency |
|---|---|
| record 01 §3/§5 ↔ record 04 §3 | record 01 §3's cadence row ("full weekly + incremental daily; WAL archiving continuous") matches record 04 §3 T1's mechanism cell, which cites P12.01 §3 back; record 01 §5's per-class rows ("every DB snapshot + WAL"; JetStream "per-stream snapshot cadence — see the P12.04 tier row") match the T1/T2 mechanism cells; the T5 row's "no separate backup (P12.01 §3)" matches record 01 §3's edge-store row. No contradiction between records 01, 02, 04 (record 04 §6 item 4's own review obligation holds). |
| record 04 §3 ↔ P12-E-2 | T2's RPO (≤ 60 min) is explicitly "the snapshot cadence bound; the P12-E-2 DEFAULT" and its mechanism is "per-stream snapshot cadence into the shared object-storage target" — the same posture P12-E-2 records as the DEFAULT in record 02 §4. The T1-variant row stays agnostic until P12-E-1 closes, as required. |
| retention ↔ P12-E-2 | record 01 §5's retention periods are EMPTY in every row (P9.10 §6 — no basis while D1 is OPEN), so no cadence or retention cell contradicts the snapshot posture; the JetStream row's cadence derives from the T2 tier row, which derives from the P12-E-2 DEFAULT. One direction of derivation, stated in both records. |

**Triangle 2 — the evidence-discipline triangle (Agent A's handoff,
record 33 §5 item 1):** `runbooks/evidence_templates.md` §1–§2 ↔ record 04
§3–§4 ↔ the per-drill evidence templates (records 03 §5, 05 §5, 32 §6,
`runbooks/dr_site_loss.md` §6, `runbooks/game_day_template.md` §6–§7).
Verified: the template file carries POINTERS to the owning records (its
§8 rule 4 states "pointers, not rewrites") and no pass criterion is
restated or modified anywhere; the requirement-cells copy block (§2)
carries record 04's RECOMMENDATION labels and EMPTY sign-off discipline
verbatim-in-substance, with the R-P9.24-2 consequence stated; the §1
register rule set (X-8) matches conventions §5 and records/03 §4 item 5 /
records/05 §5 preamble.

### 3.6 Findings (recorded, not fixed — none is this agent's file to fix)

1. **Stale expected-path citations to the P12.13 record (3 occurrences,
   UNFIXED — handoff for the orchestrator).** The P12.13 deliverable
   landed at `runbooks/evidence_templates.md` (record 31 §1; the
   deviation is Agent A's own recorded choice, record 33 §3 item 3), but
   three wave-1 citations still point at the expected path
   `records/13_p12_13_evidence_templates.md`, marked "not yet written":
   `records/03_p12_03_restore_drill_runbook.md` §4 prerequisite-table row
   5 and §5 preamble; `records/19_phase_12_wave1_agent_a_close_out.md`
   §3 item 1. Every other P12.13 reference in the phase cites the landed
   path (records 31/32/33; record 32 §5 prerequisite 6). The stale
   citations are in Agent A's files — outside this agent's ownership —
   so they are RECORDED here, not edited. The referenced content exists
   and is correct; only the path is stale. Resolution is a two-file
   citation update by the orchestrator (or at the next legitimate edit
   of records 03/19).
2. **The PENDING-P11-ANCHOR case did not arise**: the Phase 11 SLO
   register exists (`../phase-11-operational-robustness/records/
   02_p11_02_slo_register.md`), so record 21 §1's hypotheses are ANCHORED
   — recorded here so the ledger's wave-note ("hypotheses may be
   PENDING-P11-ANCHOR") is read as the conditional it was, not a
   description of the landed state.
3. **Two wave-1 close-outs carry the pre-reconciliation expected-path
   notes as historical record** (records 19 and 27 §4 describe the
   expected-path pattern used at wave-1 writing time); these are accurate
   as history and were reconciled at wave-1 close by the citing agents —
   no action needed beyond finding 1's three stale literals.

## 4. Deviations from the plan (all recorded, none silent)

1. **No separate P12-E register record was created in the 51–69 range.**
   The dispatch brief required verifying that P12-E-1/E-2/E-3 are
   registered OPEN with DEFAULTs and adding "any missing register entry"
   — the verification (§1 table) found all three registered OPEN by their
   wave-1 records, so nothing was missing and no new file was needed; the
   close record carries the verification of record instead (the Phase 11
   close created record 54 only because P11-E-1 had NO register record —
   a different situation).
2. **Record numbering: 51 for P12.15 and 52 for the close**, within the
   assigned 51–69 range; 53–69 are unused, not reserved against anything.
3. **The audit-scope wait-check is recorded**: Agent A's records 31–33
   landed while this close was being written; the instruction's
   wait-check-then-audit order was followed, and §3 states the scope
   explicitly (every artifact present at close time, including all of
   Agent A's outputs).
4. The permitted tools were run once, after records/51, HANDOVER.md, and
   CHECKLIST.md were final and after the audit sweeps; this record was
   written last and quotes those outputs (§2 states this order honestly).

## 5. Handoff notes for the orchestrator

1. Apply the ledger ticks for P12.13–P12.16 after this close, citing
   records 31, 32, 51, and this record per §1 (conventions §2.2/§2.3
   exact-match method or Write/Edit). Expected end state: 16/16.
2. Fix the three stale expected-path citations (§3.6 finding 1) — a
   two-file edit in records/03 and records/19 (Agent A's files), or carry
   them as recorded history; the landed path everywhere else is
   `runbooks/evidence_templates.md`.
3. The phase gate is NOT MET as a runtime claim. The human-gate queue, in
   dependency order: the first human build/test of the Phase 0–10
   workspace; a real deployment; then, in the operators' chosen order,
   the X-8 registrations, the drills (restore → stream → DR rehearsal;
   the failover drill needs multi-host + DCS), the scoped
   restore-authorization grant, and the quarterly game-day adoption
   (CHECKLIST §Human-gate-inputs has the full table).
4. The three E-decisions stay OPEN; their closers and closing evidence
   are named in §1's table. Phase 20 consumes P12-E-1/E-2/E-3 and the HA
   design unchanged (plan §5) — records, not new copies.
5. The claim vocabulary for everything that follows (builds, drills,
   sign-offs) is record 51 §2's rung ladder and §4's banned list: rung 3
   (repeated test outcomes) is the ceiling, and no number of green runs
   converts any claim into a guarantee (§13.3 semantics).

Status: **PHASE 12 WRITTEN-UNRUN, 16/16 subtasks implemented** — the
phase's deliverables are complete as written artifacts; every execution
act they describe is a human gate that has not happened and is not
scheduled. The phase close is complete per conventions §2.5 (HANDOVER,
CHECKLIST, this close/exit-gate record).
