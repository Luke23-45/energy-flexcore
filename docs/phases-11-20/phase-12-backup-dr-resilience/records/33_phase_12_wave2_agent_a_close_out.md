<!-- P12 wave-2 Agent A close-out (P12.13–P12.14: records 31–32 + this
     record 33). No subtask was executed: no drill ran, no stream was
     deleted, no snapshot exists, no restore was performed, no game day
     ran, no fault was injected, no timing was measured. No ledger checkbox
     was ticked by this agent — tick flips are the conventions-disciplined
     process, not a wave-2 act. -->
# Phase 12 — Wave 2 Agent A Close-Out (P12.13–P12.14: evidence templates + JetStream rehearsal design)

| Field | Value |
|---|---|
| Scope | P12.13–P12.14 (plan §3 wave 2, Agent A); records 31–32 + this close-out (the assigned range 31–45 carries exactly these; remaining numbers unused) |
| Date | 2026-11-14 |
| Status | **WRITTEN-UNRUN wave:** every artifact is a written template/design; nothing is installed, built, run, drilled, deleted, backed up, or restored; every evidence cell is EMPTY/UNRUN; every threshold-register row is EMPTY (its correct pre-run state); no sign-off or registration cell is filled |
| Ledger | `ledger_phase_12.md` untouched (no checkbox flipped by this agent) |
| Permitted verification | all three tools run after the final edits; verbatim outputs in §4 |

## 1. Files created

| Path | Subtask |
|---|---|
| `docs/phases-11-20/phase-12-backup-dr-resilience/runbooks/evidence_templates.md` | P12.13 — the consolidating evidence-template file: X-8 pre-registration rule pre-printed with an EMPTY per-drill threshold register; the P12.04 RECOMMENDATION requirement-cells copy block (sign-off EMPTY); the P12.03 drill rows (criteria CITED from records/03 §5); the P12.05 F-1..F-7 evidence cells (records/05 §5); pointers to the P12.14 stream-drill rows (records/32 §6) and the DR-rehearsal rows (`runbooks/dr_site_loss.md` §6); the game-day evidence-capture checklist (game_day_template §6–§7); the `lab_harness.Evidence` JSON cell inventory (marked a COPY under the harness's re-diff rule); filling rules + non-claims. Every cell EMPTY. |
| `docs/phases-11-20/phase-12-backup-dr-resilience/records/31_p12_13_evidence_templates.md` | P12.13 — decision record for the template file |
| `docs/phases-11-20/phase-12-backup-dr-resilience/records/32_p12_14_jetstream_rehearsal.md` | P12.14 — the stream delete + restore-from-snapshot drill DESIGN (JS-0..JS-12), mirroring records/03's structure; executes the P12-E-2 DEFAULT with the dependence recorded; restore authorization inherited from P10.11 §4.4 (cited); EMPTY timed-evidence template |
| `docs/phases-11-20/phase-12-backup-dr-resilience/records/33_phase_12_wave2_agent_a_close_out.md` | this record |

Files modified: none (no shared file touched; the wave-1 records, the
runbooks tree's existing files, `deploy/compose/backup/*`, and the
experiments tree are consumed by citation only and are byte-untouched).

## 2. How the wave-1 handoffs were consumed

| Handoff (source) | Disposition |
|---|---|
| records/19 §3 "To wave 2" item 1 (P12.13 carries the P12.03 skeleton, the F-columns, the X-8 rule, the RECOMMENDATION RTO/RPO cells) | All four landed in `runbooks/evidence_templates.md` §1–§4, by citation |
| records/27 §4 item 1 (one discipline, three instantiations: `lab_harness.Evidence`, game-day §6–§7, DR runbook §6) | Template E carries the JSON-cell inventory as a copy subject to the harness's re-diff rule; Templates C/D point at the DR runbook and game-day template without duplicating criteria |
| records/27 §4 item 3 (P12.14 cites the C-NATS-1 experiment and `jetstream_snapshot.sh` rather than re-deriving) | records/32 §2–§3 cite both at their landed paths; the snapshot procedure is named as the JS-1 producer; the C-NATS-1 oracle surface is cited, not re-specified (JS-8) |
| records/02 §4 (P12-E-2 OPEN, snapshot DEFAULT, closer = human owner with P12.14 drill evidence) | records/32 §2 records the dependence exactly: the drill executes the DEFAULT under the OPEN decision and neither closes nor re-opens it |
| P10.11 §4.4 (scoped restore authorization — found via the 0–10 ledger tree: `ledger/phase-10-v1-release-hardening/11_backup_restore_rc.md`) | records/32 §4 consumes it unchanged (scoped authorization, the `RESTORE_AUTHORIZATION_UNSCOPED` finding name, the CVE-2026-33222 fix-version rule as cited there); §5 prerequisite 4 marks the scope NOT GRANTED |

## 3. Deviations and honest notes

| # | Item | Disposition |
|---|---|---|
| 1 | No tick was applied to any `ledger_phase_12.md` checkbox | Per the hard bans — tick flips are not a wave agent act |
| 2 | Record numbers 34–45 in the assigned range were left unwritten | The subtask instruction names exactly one record per subtask (31, 32) and the close-out (33); extra numbers are unused, not reserved against B (B owns 51–69) |
| 3 | The P12.13 template landed at `runbooks/evidence_templates.md` rather than a new `evidence/` tree | The subtask offered either; wave 1 created the `runbooks/` tree, so the template extends it (one tree, discoverable beside the runbooks it serves) |
| 4 | No restore-command CLI shape is named anywhere in records/32 | Deliberate: `nats stream backup` is UNVERIFIED [S-65] (never run in this program) and no restore surface was verifiable at all; the exact command is to be recorded FROM the first execution (records/32 §2) |
| 5 | No threshold is registered in the template's §1 register | The register's EMPTY state is its correct pre-run state (X-8): registering numbers without a deployment or basis would violate the no-invention rule |
| 6 | `verify_docs.py`/`validate_agent.py` check the report and the harness, not phase records | Their output is quoted as the permitted verification gate requires; record-level correctness is enforced by each record's reviewer procedure and the human gate |
| 7 | The `check_dependency_rule.py` output contains a non-ASCII byte rendered as `ù` in this shell | Quoted verbatim; treated as cosmetic encoding of the tool's separator (same as both wave-1 close-outs) |
| 8 | No drill, deletion, restore, snapshot, measurement, or fault injection exists | The core honesty statement of the whole wave |

## 4. Permitted verification — verbatim outputs

All three permitted tools were run from the repository root after the
final edits. Outputs quoted verbatim (the non-ASCII character in the
third output is the tool's own byte output, reproduced as-is):

```text
$ python tools/verify_docs.py
report: docs\plan\Neryva_FlexCore_Product_and_Architecture_Report_v1.1.md
lines: 2241  sections: 28  headings: 127  tables: 32
OK: headings, section references, fences, tables, references, TOC
EXIT=0

$ python tools/validate_agent.py
skills: 5  roles: 5
OK: skills, roles, and README index are valid
EXIT=0

$ python -I tools/check_dependency_rule.py
OK ù dependency rule holds for 20 workspace members.
EXIT=0
```

All three exit codes were 0. No failure is reported by any tool; per the
honesty rules this states the tools' own output, not a claim about the
unrun scripts, undeployed designs, or undrilled procedures.

## 5. Handoff notes for Agent B / the orchestrator

1. **Agent B (P12.15/P12.16, records 51–69):** the consistency audit's
   evidence-discipline triangle is `runbooks/evidence_templates.md` §1–§2 ↔
   records/04 §3–§4 ↔ the per-drill evidence templates in records/03 §5,
   records/05 §5, records/32 §6, `runbooks/dr_site_loss.md` §6, and
   `runbooks/game_day_template.md` §6–§7 — the templates carry pointers,
   the criteria stay in the owning records; a duplicated criterion would
   be a defect.
2. **P12.15:** the §13.3 semantics framing ("zero observed violations from
   finite tests is a test outcome, never a guarantee") is carried in the
   non-claims sections of records/31, records/32, and the template file —
   consume those wordings rather than re-deriving.
3. **P12.16:** the P12-E-2 closure input set is records/32 §6 (JS-9's
   event-plane loss window is the R-P9.24-7 measurement) + records/04 §3
   T2; the decision stays OPEN until the human owner closes it — the drill
   design neither closes nor re-opens it (records/32 §2).
4. **Human gates pending** (all recorded UNRUN/UNSIGNED): every drill the
   templates serve; the first execution of `jetstream_snapshot.sh` (CLI
   surface UNVERIFIED, [S-65]); the scoped restore-authorization grant
   (records/32 §5 prerequisite 4); any threshold registration (X-8) and any
   sign-off cell in records/04.
5. **Carried UNVERIFIED items (not defects yet):** the `nats` CLI surface
   [S-65] — no restore-command shape is asserted anywhere by this wave;
   the pgBackRest option names [S-68] (records/02-carried); PostgreSQL 18
   remains an UNVERIFIED pin (P10.13).

## 6. Non-claims (stated once, governing everything above)

No drill ran; no stream was deleted; no snapshot exists; no restore was
performed; no game day ran; no fault was injected into any running system;
no timing was measured; no threshold was registered; no refusal was
observed; no edge behavior was observed. Zero observed violations from any
finite run would be a test outcome, never a guarantee (§13.3 semantics;
conventions §5). Nothing here is a capability, conformance, safety, or
protection-grade claim, and nothing adds dispatch authority (ADR-010; the
never-granted permission set is untouched).

Status: **WAVE 2 (Agent A portion) WRITTEN-UNRUN** — the wave's
deliverables are complete as written artifacts; every execution act they
describe is a human gate that has not happened and is not scheduled.
