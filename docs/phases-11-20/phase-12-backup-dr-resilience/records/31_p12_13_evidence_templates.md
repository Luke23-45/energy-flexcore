<!-- P12.13 — Backup/restore + DR evidence templates (decision record).
     TEMPLATE-ONLY deliverable: every evidence cell in the template file is
     EMPTY (UNRUN / UNMEASURED / UNSIGNED / NOT WRITTEN); no drill has run,
     no threshold is registered, no observed value exists anywhere. The
     X-8 pre-registration rule is pre-printed in the template: any timing
     threshold is registered BEFORE any run that could fill it, and
     observed values live structurally separate. This record consolidates
     the wave-1 pieces by CITATION — it rewrites no criterion. -->
# P12.13 — Backup/Restore + DR Evidence Templates (template file delivered — all cells EMPTY, nothing has run)

| Field | Value |
|---|---|
| Subtask | P12.13 — Backup/restore + DR evidence templates (`ledger_phase_12.md` §P12.13) |
| Wave / agent | Phase 12 wave 2, Agent A |
| Status | **WRITTEN-UNRUN.** The template file is delivered; no drill of any kind has run; every evidence cell in it is EMPTY; no timing threshold is registered (the register's EMPTY state is its correct pre-run state); no observed value exists anywhere. |
| Sources | conventions §5 (human gates + the X-8 pre-registration rule, consumed verbatim-in-substance); P9.24 §8 (`ledger/phase-09-pilot-qualification/24_backup_dr_rehearsal.md` — the filled-cell-without-run fabrication rule, inherited); P9.24 §5 (the drill procedure the P12.03 rows execute — cited, never rewritten); P10.11 §6 (`ledger/phase-10-v1-release-hardening/11_backup_restore_rc.md` — the EMPTY-run-record pattern, inherited); `records/03_p12_03_restore_drill_runbook.md` §5 (the drill skeleton consolidated); `records/05_p12_05_standby_replication.md` §5 (the F-1..F-7 columns consolidated); `records/04_p12_04_rto_rpo_register.md` §3–§4 (the RECOMMENDATION cells + judgement rule — plan §2.1's P12.04 → P12.13 dependency); `records/21_p12_07_chaos_program.md` §3.5 + `experiments/lab_harness.py` (the evidence-JSON shape consolidated); `runbooks/game_day_template.md` §6–§7 (the game-day evidence capture consolidated); `records/32_p12_14_jetstream_rehearsal.md` §6 + `runbooks/dr_site_loss.md` §6 (the stream-drill and DR-rehearsal rows, pointed to) |
| Files touched | `runbooks/evidence_templates.md` (new — the template deliverable); this record |

## 1. Honest status — the plain statement

**No drill has ever run; no evidence exists.** This subtask delivers the
consolidating evidence-template surface the wave-1 records point at: one
template file (`runbooks/evidence_templates.md`) whose every cell is EMPTY
and whose X-8 pre-registration rule is PRE-PRINTED — any timing threshold
is registered in its §1 register BEFORE any run that could fill it, and
observed values live structurally separate (per-run copies and evidence
JSON files, never in the register). The templates are copy-per-run
skeletons: filling any cell is a human-gated act that happens FROM a real
run, by the human executor. No run is scheduled; no threshold is invented
to make the register look used.

## 2. What the template file carries (the consolidation map)

| Template section | Consolidates | Discipline carried |
|---|---|---|
| §0 inputs table | all wave-1 pieces, by landed path | citation, not duplication; criteria stay with their owning records |
| §1 threshold register (X-8) | conventions §5; records/03 §4 item 5; records/05 §5 preamble | pre-printed rule; EMPTY rows per drill/experiment; a run judged against an unregistered threshold is invalid for judgement (`UNREGISTERED-BASIS` finding); thresholds never re-registered to fit results |
| §2 requirement cells in force | records/04 §3–§4 | RECOMMENDATION label + EMPTY sign-off carried; an EMPTY sign-off means the run measures against NOTHING (the R-P9.24-2 finding, recorded as such); the judgement rule pre-printed |
| Template A — restore-drill rows | records/03 §5 | DR-1..DR-11 + E-1..E-6 rows, result cells only; criteria CITED |
| Template B — failover rows | records/05 §5 | F-1..F-7 evidence cells only; DEPLOYMENT-gated framing carried |
| Template C — stream drill + DR rehearsal pointers | records/32 §6; `runbooks/dr_site_loss.md` §6 | pointers only — no criteria duplicated |
| Template D — game-day evidence capture | `runbooks/game_day_template.md` §6–§7 | capture checklist with file-path cells; abort + findings rows carried |
| Template E — evidence-JSON shape | `experiments/lab_harness.py` (`Evidence`); records/21 §3.5 | the 14-cell + `oracle_findings[]` inventory, marked as a COPY subject to the harness's re-diff rule |
| §8 filling rules | P9.24 §8 (via records/03) | from-the-run-only; pre-registration first; structural separation; pointers not rewrites; verbatim evidence |

## 3. The X-8 pre-registration discipline as landed

1. **Rule placement:** the rule is pre-printed at the top of §1 of the
   template file — a reader of any template hits it before any evidence
   row.
2. **Registration is a human act:** the register's `Registered by / date`
   cells are EMPTY; the executor registers thresholds before the run, with
   a basis cell that says why the number.
3. **Observed values are structurally separate:** the register (§1) and
   the requirement cells (§2) carry only registered/signed values; every
   observed value lives in the per-run copies (§3–§6) or the evidence JSON
   files (§7). The two never share a cell.
4. **The chaos scripts' argparse defaults are not registrations:** a
   default in a script is not an X-8 registration; the register row must
   exist before the run (the template's register rule 3).
5. **No threshold exists yet** — and that is the correct state, not a
   gap: no deployment exists to run against, so no threshold has a
   legitimate basis to register.

## 4. What this subtask does NOT do

1. It does not run, schedule, or prepare any drill, game day, or
   rehearsal; nothing is filled from anything.
2. It does not register any threshold — inventing one to populate the
   register would contradict the no-invention rule (the records/04 §3 T7
   precedent).
3. It does not rewrite or re-decide any criterion — every pass criterion
   stays with its owning record; the templates carry result cells and
   pointers.
4. It does not close any E-decision or sign any cell; every sign-off and
   registration cell stays EMPTY (conventions §4–§5).
5. It adds no dispatch authority (ADR-010); evidence templates are
   record-keeping surfaces, not control surfaces.

## 5. Reviewer-verification procedure (human)

1. Confirm §1's plain statement: no drill ran; EVERY cell in
   `runbooks/evidence_templates.md` is EMPTY/UNRUN/UNMEASURED/UNSIGNED/
   NOT WRITTEN or an explicit `<EMPTY>` placeholder; no observed value
   exists anywhere.
2. Confirm §1 of the template pre-prints the X-8 rule verbatim-in-substance
   (register BEFORE the run; observed values structurally separate; no
   re-registration to fit results) and that no register row is filled.
3. Confirm Templates A/B cite records/03 §5 and records/05 §5 WITHOUT
   restating pass criteria, and that Template E marks its JSON inventory
   as a copy subject to the harness's re-diff rule.
4. Confirm §2 of the template carries the RECOMMENDATION label and the
   EMPTY sign-off discipline from records/04, and the R-P9.24-2
   empty-requirement consequence.
5. Confirm the consolidation map (§0/§2 here) names only files that
   actually exist at their landed paths.
6. Confirm no checkbox in `ledger_phase_12.md` was touched by this
   subtask.

## 6. Filling commands (for the human executor)

```sh
# (a) the documented-tools gate (run after ANY edit to this record):
python tools/verify_docs.py
python tools/validate_agent.py
python -I tools/check_dependency_rule.py

# (b) BEFORE any drill/experiment run: fill the §1 register row for it
#     (threshold, value, basis, registrant, date) — X-8.
# (c) Copy the §2 requirement cells from the P12.04 register version in
#     force at the run date; an EMPTY sign-off means the run measures
#     against nothing (record the R-P9.24-2 finding, never invent a
#     requirement).
# (d) Copy the relevant template (A–E) per run; fill it FROM THE RUN ONLY;
#     keep the evidence JSON files and record their paths + hashes.
# (e) Hand filled run copies to the owning records' judgement steps
#     (records/03 §5.2, records/05 F-7, records/32 JS-11, the game-day
#     template §8).
```

## 7. Non-claims

No drill ran; no threshold is registered; no observed value exists; no
requirement is set or signed; no capability, conformance, or
runtime-behaviour claim is made. Zero observed violations from any finite
run would be a test outcome, never a guarantee (§13.3 semantics;
conventions §5). Nothing here adds dispatch authority (ADR-010).

Status: **WRITTEN-UNRUN** — a template cannot produce evidence by being
written; every run it serves is a human gate that has not happened and is
not scheduled.
