<!-- P12.15 — Resilience acceptance criteria (what "resilient" may and may not
     be claimed as after Phase 12). CRITERIA RECORD ONLY: nothing was run,
     built, backed up, restored, drilled, injected, or measured by this
     record. It consumes wave-1 outputs by citation (records 24 and 26 are
     the traceability inputs; records 05 §6 and 06 §6 carry the §13.3
     statements this record generalizes). It closes nothing and claims
     nothing beyond what written artifacts establish. -->
# P12.15 — Resilience Acceptance Criteria (what "resilient" may and may not be claimed as after Phase 12)

| Field | Value |
|---|---|
| Subtask | P12.15 — Resilience acceptance criteria + records (`ledger_phase_12.md` §P12.15; plan §2 row P12.15) |
| Wave / agent | Phase 12 wave 2, Agent B |
| Status | **CRITERIA RECORD — WRITTEN, NOTHING EXECUTED.** No backup, restore, drill, fault injection, game day, or measurement exists anywhere in this program; every evidence cell in every consumed artifact is EMPTY/UNRUN. This record defines the claim vocabulary for the phase; it makes no claim of its own beyond the written artifacts it cites. |
| Sources | Program conventions (`../../01_PROGRAM_CONVENTIONS.md` §5 — human gates and §13.3 semantics; §7 — honesty rules); plan §1, §8 (the phase's own acceptance criteria, criterion 5 especially); record 24 (`records/24_p12_10_degradation_matrix.md` §1–§3 — the degradation matrix and its no-silent-capability rules); record 26 (`records/26_p12_12_edge_resilience_review.md` §1–§5 — the edge review and findings F-P12.12-1..5); record 21 (`records/21_p12_07_chaos_program.md` §4 — what running the experiments would and would not establish); records 05 §6 and 06 §6 (the §13.3 statements this record generalizes); P9.24 §4 (`ledger/phase-09-pilot-qualification/24_backup_dr_rehearsal.md` — the inherited judgement rule) |
| Files touched | this record only |

## 1. Honest status — the plain statement

**Phase 12 has produced written artifacts only.** No backup was taken, no
restore was performed, no fault was injected into any running system, no
game day ran, no drill executed, no timing was measured, no edge behavior
was observed. Every evidence cell in every artifact this phase produced is
EMPTY/UNRUN, every RTO/RPO value is a RECOMMENDATION with an EMPTY sign-off
cell, and all three E-decisions (P12-E-1/E-2/E-3) are OPEN. This record is
the acceptance-criteria deliverable plan §2 assigns: it enumerates, once
and for the phase, what may legitimately be claimed after Phase 12 (§3)
and what may NEVER be claimed (§4), under the §13.3 semantics that govern
the whole program: **zero observed violations from a finite test set is a
test outcome, never a guarantee** (conventions §5).

## 2. The semantics of record (§13.3, and the escalation ladder)

Every resilience claim anyone makes about this program's artifacts must
name its evidence rung. The rungs, weakest to strongest:

| Rung | What exists | Claimable form |
|---|---|---|
| 0 — written | the artifact exists in the repository | "written, complete, unrun" |
| 1 — reviewed | a static review traced it to written code (record 24, record 26) | "traced to written code cited by file and symbol; the code itself is WRITTEN-UNRUN" |
| 2 — run once | a human-gated drill/experiment ran and its evidence was recorded | "on `<date>`, at `<scale>`, under the registered thresholds, the run observed `<outcome>` — recorded in `<evidence file>`; a test outcome, not a guarantee" |
| 3 — repeated | multiple runs, multiple days, multiple scales | the same form, with the run inventory named; still never a guarantee |

No claim may skip rungs, and no number of rungs converts a claim into a
guarantee: rung 3 is the ceiling. The inherited P9.24 §4 judgement rule
applies at rungs 2–3: a measured value worse than the requirement in force
is a FINDING with a disposition, never a re-defined requirement.

## 3. Claims ALLOWED after Phase 12 (and their evidence)

Each allowed claim is exactly as strong as the written artifact it names —
no stronger.

| # | Claim allowed | Evidence (what exists) |
|---|---|---|
| A-1 | The written artifacts exist and are complete as written: the backup procedure tree (`deploy/compose/backup/`), the restore-drill runbook (`records/03`), the RTO/RPO register (`records/04`), the standby and pooling designs (`records/05`, `records/06`), the chaos program and its four pre-registered experiment scripts (`records/21`, `experiments/`), the game-day template and quarterly-cadence proposal (`records/22`, `runbooks/game_day_template.md`), the expanded fault-injection taxonomy with its test mirrors (`records/23`, `apps/worker/python/drill/`), the degradation matrix (`records/24`), the DR runbook (`records/25`, `runbooks/dr_site_loss.md`), the edge review (`records/26`), the evidence templates (`runbooks/evidence_templates.md`), and the JetStream rehearsal design (`records/32`) | each file is on disk with its honest WRITTEN-UNRUN label |
| A-2 | The procedures are COMPLETE in the sense that nothing needed to execute them is missing from the written layer: every drill names its prerequisites (records 03 §4, 32 §5), its pass criteria by citation (P9.24 §5/§6 — cited, never rewritten), its evidence template with EMPTY cells, and its human gate | the records' own prerequisite tables |
| A-3 | The scripts are REPEATABLE as written: every chaos script prints its pre-registration verbatim before anything runs and refuses to run without `--execute`, the `GO` gate, and operator-supplied lab-control commands (record 21 §3); the manifest tool is deterministic and fail-closed; the fault-injection module remains pure data-flow over an injected callable (record 23 §3) | the scripts' own gates, in their code and headers |
| A-4 | The degradation matrix is a static code-review artifact: every non-EMPTY cell traces WRITTEN code cited by file and symbol, and every cell the code does not specify says UNSPECIFIED (record 24 §1–§2) | record 24 |
| A-5 | The edge posture is BOUNDED, AUDITED, and FAIL-CLOSED as written: overflow is drop-of-OLDEST with a latched `has_overflowed` flag and counted drops (never silent); the effective offline retention window is min(capacity window, `max_age` window) — a derived static bound, never a measurement (F-P12.12-2); a prolonged central outage degrades the command path to HOLD (fail-closed on an untrusted clock) while telemetry buffers within the written bounds (F-P12.12-4) | record 26 §1–§3; both findings are compositions of written rules, not observed behavior |
| A-6 | When a human-gated run eventually fills an evidence cell, the OUTCOME of that run may be stated: e.g. "the C-PG-1 run of `<date>` observed no command outside the 14-state vocabulary" — stated as a test outcome on the tested day at the tested scale under the registered thresholds, judged per the inherited P9.24 §4 rule | rung 2 of §2; the evidence file the run produced |
| A-7 | The known gaps and loss windows are RECORDED (which is itself a claim this phase can make): the edge buffer is an audited LOSS window (R-P9.24-5, carried unchanged); the DB-vs-stream loss-window gap is visible and measurable only by the P12.14 drill (R-P9.24-7); the config-permitted buffer ceiling admits journal sizes no disk plan addresses (F-P12.12-1); drop counters reset per open (F-P12.12-3); the overflow latch is one-way and coarse (F-P12.12-5); no latency evidence exists for P12-E-1 (F-P12.05-1); no runtime RLS-under-pooling evidence exists (F-P12.06-1); LR-204 is not closed by a selection record (record 01 §1) | the findings registers in records 01, 05, 06, 24, 26 |
| A-8 | Three E-decisions are registered OPEN with DEFAULTs, human-closed (P12-E-1 record 05 §4; P12-E-2 records 02 §4 and 32 §2; P12-E-3 record 25 §7 / `runbooks/dr_site_loss.md` §7) | the register entries and `ledger_phase_12.md` §Decision points |

## 4. Claims NEVER allowed (regardless of what later runs show)

| # | Never claimable | Why (the rule that bans it) |
|---|---|---|
| N-1 | **"Resilient"** as an unconditional property of the system or any component | §13.3 semantics (conventions §5): zero observed violations from finite tests is a test outcome, never a guarantee; there is no finite test set from which resilience follows |
| N-2 | **"Fault-tolerant"** / "self-healing" / "graceful under failure" as properties | the degradation matrix (record 24) traces what written code SAYS it would do, written-unrun — a code-review mapping is never a runtime property (record 24 §2 rule 1) |
| N-3 | **"Highly available" / "HA" / "no single point of failure"** | no cluster, standby, or DCS exists or is deployed (record 05 §1; plan §6 risk 3); the P12.05 design is DEPLOYMENT-gated, and a single-standby topology has its own failure modes (record 05 §6 item 3) |
| N-4 | **"Proven recovery" / "restorable" / "backups are verified good"** | no backup exists; a verify job is NOT a restore (P9.24 §3, record 02 §1 rule 1); LR-204 stays open until the P12.03 drill fills its cells from a real run |
| N-5 | **"Zero data loss"** (anywhere) | two audited loss windows exist by design: the edge buffer's drop-of-oldest window (R-P9.24-5; record 26 §2) and the event-plane snapshot-cadence window (R-P9.24-7; record 02 §4); the honest posture is bounded-and-counted loss, never loss-free |
| N-6 | **RTO/RPO as achieved capability** ("recovers within 4 h", "RPO of 15 min") | every register value is a RECOMMENDATION with an EMPTY sign-off cell (record 04); nothing has been measured; an un-judged drill measures against nothing (R-P9.24-2) |
| N-7 | **"Tested" / "verified" / "passed" / "works"** as ledger or prose status | conventions §2.2: a checkbox is ticked only as "implemented, awaiting human build/test"; every evidence cell is EMPTY/UNRUN |
| N-8 | **Any guarantee from future green runs** | rung 3 of §2 is the ceiling: repeated test outcomes are still test outcomes; §13.3 semantics do not expire |
| N-9 | Any safety, certification, conformance, or protection-grade claim | conventions §7.3–§7.4 and AGENTS.md §3; the central-outage rule (`deployment-upgrade-model.md` §5) is site-local EMS authority — nothing in this phase tests or provides it (record 21 §4) |
| N-10 | Any new dispatch authority granted by recovery, failover, or degraded operation | ADR-010 and the never-granted permission set are untouched everywhere in this phase; a restored control plane resumes disabled until the per-site enablement gates are satisfied (record 25 §2) |

## 5. The claim-escalation discipline (binding on future fills)

1. **One run, one record.** A run's outcome is filled ONLY into the
   evidence cells of the record/template that ran (records 03 §5, 32 §6,
   `runbooks/evidence_templates.md` §3–§7, the game-day template) — a
   filled cell without a run is a fabrication defect (the P9.24 §8 rule).
2. **An observed-behavior annex to the degradation matrix may be filled
   ONLY cell by cell, from a real run's evidence, with the run's date and
   evidence file — never inferred** (record 24 §4 item 2).
3. **A finding is never upgraded by re-labeling.** A measured value worse
   than the requirement in force is a FINDING with a disposition (fix and
   re-run / signed time-bounded acceptance / blocks the gate) — never a
   re-defined requirement (the inherited P9.24 §4 rule; record 04 §4).
4. **Thresholds stay ahead of runs** (X-8): any timing threshold is
   registered in `runbooks/evidence_templates.md` §1 BEFORE the run that
   could fill it, and observed values live structurally separate from the
   register.
5. **Claims name their rung** (§2). Any future prose claim about this
   phase's deliverables must be expressible in the rung's claimable form;
   if it cannot, it is not a claim this program has earned.

## 6. Plan §8 criterion 5 — how this record satisfies it

Plan §8 criterion 5: "The degradation matrix and the resilience acceptance
criteria make only claims traceable to written code, under §13.3
semantics." Mapping:

- The degradation matrix (record 24) satisfies it by construction: every
  cell traces written code or says UNSPECIFIED.
- This record satisfies it by §3 (every allowed claim names its written
  artifact) and §4 (the banned list is explicit and absolute).
- The §13.3 statement appears in every wave-1 record that could have
  implied a capability (records 05 §6, 06 §6, 21 §4, 22 §3, 23 §3,
  24 §2, 25 §2, 26 §5) and is generalized here once, for the phase.

## 7. Reviewer-verification procedure (human)

1. Confirm §1's plain statement: no execution act exists anywhere in the
   phase; every consumed artifact's evidence cells are EMPTY/UNRUN.
2. Confirm every §3 allowed-claim row cites artifacts that exist on disk
   with their honest labels, and that no row claims runtime behavior.
3. Confirm §4's banned list matches the program's binding rules
   (conventions §2.2, §5, §7; plan §8; AGENTS.md §3) and adds no loophole.
4. Confirm §5's discipline is consistent with the evidence templates'
   register rules (`runbooks/evidence_templates.md` §1) and the matrix's
   annex rule (record 24 §4 item 2).
5. Confirm this record adds no dispatch authority and makes no
   certification or safety claim.

## 8. Non-claims

This record claims no resilience, no availability, no recoverability, no
loss-free behavior, no measured value, and no conformance. Its only
positive claims are about written artifacts existing and being complete as
written — which is the strongest claim this phase has earned. §13.3
semantics govern everything above and survive any future run.

## 9. Handoff notes

1. **The orchestrator's tick for P12.15** cites this record; the tick text
   stays "implemented, awaiting human build/test" (conventions §2.2).
2. **Future fills** (the first human build, the first drill, the first
   game day) consume §2's rung ladder and §5's discipline; the CHECKLIST
   and close record carry the same pointer.
3. **The edge findings F-P12.12-1..5** (record 26 §4) are inputs to
   deployment sizing and the operator visibility design, not to this
   record's claim vocabulary — they are carried in §3 row A-5/A-7 as
   recorded facts about written code, never as observed defects.
