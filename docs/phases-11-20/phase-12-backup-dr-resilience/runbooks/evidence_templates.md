<!-- P12.13 — Backup/restore + DR evidence templates. TEMPLATE FILE: every
     evidence cell in every template below is EMPTY (UNRUN / UNMEASURED /
     UNSIGNED / NOT WRITTEN); no drill has run, no threshold is registered,
     no observed value exists anywhere. The X-8 pre-registration rule is
     PRE-PRINTED: any timing threshold is registered BEFORE any run that
     could fill it, and observed values are kept STRUCTURALLY SEPARATE from
     registered thresholds. This file CONSOLIDATES the wave-1 pieces — it
     cites them, never rewrites their criteria. -->
# Backup/Restore + DR Evidence Templates (P12.13 — all cells EMPTY; nothing has run)

Date: 2026-11-14. Subtask: P12.13. Status: **TEMPLATE FILE — WRITTEN-UNRUN.**
No drill of any kind has run; no timing, lag, window, count, or transcript
exists anywhere in this file or behind it. Every template below is a
copy-per-run skeleton whose cells fill ONLY from a real run's clocks, logs,
and counts — a filled cell without a run is a fabrication defect (the P9.24
§8 rule, inherited). Conventions §5 (human gates) and X-8 (pre-registration)
govern everything here.

## 0. What this file consolidates (the wave-1 inputs, cited — not rewritten)

| Input | Landed location | What is carried here |
|---|---|---|
| The P12.03 restore-drill skeleton | `records/03_p12_03_restore_drill_runbook.md` §5 | the DR-1..DR-11 timed-evidence rows (§3 below); the criteria stay P9.24 §5's, cited |
| The P12.05 failover evidence columns | `records/05_p12_05_standby_replication.md` §5 | the F-1..F-7 evidence rows (§4 below) |
| The game-day evidence capture | `runbooks/game_day_template.md` §6–§7 | the evidence-capture checklist (§6 below) |
| The four chaos experiments' evidence JSON shape | `experiments/lab_harness.py` (`Evidence`), `records/21_p12_07_chaos_program.md` §3.5 | the evidence-JSON cell inventory (§7 below) |
| The JetStream stream-drill evidence rows | `records/32_p12_14_jetstream_rehearsal.md` §6 | the JS-1..JS-12 rows pointer (§5 below) |
| The DR site-loss rehearsal record | `runbooks/dr_site_loss.md` §6 | the DR-rehearsal rows pointer (§5 below) |
| The RTO/RPO RECOMMENDATION cells | `records/04_p12_04_rto_rpo_register.md` §3 | the requirement-cells-in-force template (§2 below), sign-off EMPTY |
| The X-8 pre-registration rule | conventions §5; records/03 §4 item 5; records/05 §5 preamble | the threshold register (§1 below), pre-printed rule |

This file adds NO criterion, threshold, or value of its own. Where a
sibling record owns the pass criteria, the row here is a POINTER, and the
filled copy lives with that record's run record — never here.

## 1. The threshold register (X-8) — registered BEFORE any run that could fill it

**The pre-printed rule (carried from conventions §5, unchanged):** any
timing threshold — an outage hold, a recovery budget, a drain window, a
concurrency level, a duration, any number a run's outcome will be judged
against — is REGISTERED in this section BEFORE any run that could fill it.
Registration is a human act (the executor's), recorded with its basis, and
it happens before the fault, not after the result. Observed values NEVER
live in this section — they live in the per-run copies and evidence JSON
files (§3–§7), structurally separate from the register.

| Drill / experiment this threshold governs | Threshold name | Registered value (pre-run) | Basis (why this number) | Registered by / date |
|---|---|---|---|---|
| (P12.03 restore drill) | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` |
| (P12.05 failover drill) | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` |
| (P12.14 JetStream stream drill) | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` |
| (DR site-loss rehearsal) | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` |
| (C-PG-1) | `<EMPTY>` | `<EMPTY>` | `<EMPTY: script CLI arguments, records/21 §3.4>` | `<EMPTY>` |
| (C-NATS-1) | `<EMPTY>` | `<EMPTY>` | `<EMPTY: script CLI arguments, records/21 §3.4>` | `<EMPTY>` |
| (C-WORKER-1) | `<EMPTY>` | `<EMPTY>` | `<EMPTY: script CLI arguments, records/21 §3.4>` | `<EMPTY>` |
| (C-POOL-1) | `<EMPTY>` | `<EMPTY>` | `<EMPTY: script CLI arguments, records/21 §3.4>` | `<EMPTY>` |

Register rules (binding on every template below):

1. A run whose outcome is judged against a threshold NOT registered here
   before the run is an invalid run for judgement purposes — the finding is
   the missing pre-registration, and the result is recorded as
   UNREGISTERED-BASIS, never silently judged.
2. A threshold is never re-registered to fit a result (the P9.24 §4
   judgement rule, inherited via records/03 §5.2 and records/04 §4).
3. The chaos experiments' CLI defaults (outage hold, budgets) are the
   pre-registered values ONLY when named in this register before the run —
   a default in a script's argparse is not a registration.

## 2. Requirement cells in force (from the P12.04 register — RECOMMENDATIONS, sign-off EMPTY)

Copy this block into a run record ONCE PER RUN, reading the values from the
P12.04 register version IN FORCE at the drill date. If the sign-off cell
for a tier is EMPTY at the drill date, the run measures against NOTHING for
that tier — which is exactly the P9.24 §11 R-P9.24-2 finding, recorded as
such (never a silently invented requirement).

| Tier | Value in force at the run date (from records/04 §3) | Label | Operator sign-off status | Basis cell |
|---|---|---|---|---|
| T1 — transactional store | `<EMPTY — copy at run time>` | RECOMMENDATION | `<EMPTY — human gate>` | `<EMPTY>` |
| T2 — event plane | `<EMPTY — copy at run time>` | RECOMMENDATION | `<EMPTY — human gate>` | `<EMPTY>` |
| T3/T4/T6 — stateless services | `<EMPTY — copy at run time>` | RECOMMENDATION | `<EMPTY — human gate>` | `<EMPTY>` |
| T5 — edge agent | `<EMPTY — copy at run time>` | RECOMMENDATION | `<EMPTY — human gate>` | `<EMPTY>` |

The judgement rule (inherited, pre-printed — records/04 §4): a measured
value WORSE than the requirement in force is a FINDING with a disposition
(fix and re-run / signed time-bounded acceptance / blocks the gate) — never
a re-defined requirement, never a requirement fitted to a result.

## 3. Template A — the P12.03 restore-drill timed evidence (per-run copy; criteria CITED from records/03 §5)

Copy-per-run. The step semantics and pass criteria live in
`records/03_p12_03_restore_drill_runbook.md` §2 (the P9.24 §5 citation map)
and §3; the rows here carry RESULT cells only. Every cell starts EMPTY.

**Metadata (all EMPTY):** run date / executor / witness / isolated-environment
ID / artifact IDs restored / software versions + migration set / backup
configuration row-version used / requirement row-version in force (§2 copy
attached) / threshold-register row-version in force (§1).

| Step | Result | Measured value | Pass/fail | Disposition |
|---|---|---|---|---|
| DR-1 source-state record (R-0) | (EMPTY — UNRUN) | | | |
| DR-2 isolation check (R-1) | (EMPTY — UNRUN) | | | |
| DR-3 PostgreSQL restore (R-2) | (EMPTY — UNRUN) | restore wall time `<EMPTY>` | | |
| DR-4 PITR target-instant reach (R-2) | (EMPTY — UNRUN) | WAL replay duration `<EMPTY>` | | |
| DR-5 append-only survival checks (R-3) | (EMPTY — UNRUN) | | | |
| DR-6 audit count + chain (R-4) | (EMPTY — UNRUN) | | | |
| DR-7 edge-journal CRC replay (R-5) | (EMPTY — UNRUN) | | | |
| DR-8 outbox/command consistency + zero re-issue (R-6) | (EMPTY — UNRUN) | re-issue count `<EMPTY>` | | |
| DR-9 consistent-state probe + sweep (R-7) | (EMPTY — UNRUN) | | | |
| DR-10 judgement vs requirements (R-9) | (EMPTY — UNRUN) | | | |
| DR-11 teardown + destruction (R-10) | (EMPTY — UNRUN) | | | |
| E-1..E-6 edge-outage test | (EMPTY — UNRUN) | outage interval `<EMPTY>` / drain time `<EMPTY>` | | |

## 4. Template B — the P12.05 failover drill evidence (per-run copy; criteria CITED from records/05 §5)

Copy-per-run. The step procedures and pass criteria live in
`records/05_p12_05_standby_replication.md` §5; the rows here carry evidence
cells only. Every cell starts EMPTY. DEPLOYMENT-gated: this template runs
only on an availability-contract deployment (multi-host + DCS).

**Metadata (all EMPTY):** run date / executor / witness / deployment ID
(multi-host + DCS) / Patroni + DCS versions / sync mode in force (the
P12-E-1 DEFAULT variant executed) / requirement row-version in force (§2)
/ threshold-register row-version (§1).

| Step | Evidence |
|---|---|
| F-1 steady-state record (primary/standby identities, lag counters, per-tenant counts) | `<EMPTY — UNRUN>` |
| F-2 `remote_apply` commit latency vs instance-default path (the P12-E-1 latency evidence) | `<EMPTY — UNMEASURED>` |
| F-3 schema-level property inspection on the PROMOTED node (0008 trigger + no-DELETE-grant, 0016 immutability + column-level grant, RLS, 0019 chain) | `<EMPTY — UNRUN>` |
| F-4 command continuity across failover (closed 14-state vocabulary; zero re-issued) | `<EMPTY — UNRUN>` |
| F-5 audit continuity (pre/post counts; 0019 chain verifies on the promoted node) | `<EMPTY — UNRUN>` |
| F-6 failover interval (stop → promoted → `/ready`) + observed loss window for command-lifecycle transactions | `<EMPTY — UNMEASURED>` |
| F-7 judgement + P12-E-1 closure input set | `<EMPTY — UNRUN>` |

## 5. Template C — the JetStream stream drill and the DR rehearsal (POINTERS — criteria live in their records)

Both drills carry their own per-run evidence rows in their owning records;
this template does not duplicate their criteria. A filled run copy lives
with the owning record; this section records only that a run happened and
where its evidence is.

| Drill | Owning record (the criteria + the EMPTY evidence template) | Run copy filled from | Status |
|---|---|---|---|
| JetStream stream delete + restore-from-snapshot | `records/32_p12_14_jetstream_rehearsal.md` §6 (JS-1..JS-12) | the owning record's §6, from the run only | `<EMPTY — UNRUN>` |
| DR site-loss rehearsal (restore-into-isolation + DNS/reconfiguration) | `runbooks/dr_site_loss.md` §6 (DR-1..DR-12 + N-1..N-5) | the owning record's §6, from the run only | `<EMPTY — UNRUN>` |

## 6. Template D — game-day evidence capture (per-session copy; criteria CITED from the game-day template)

Copy-per-session. The roles, abort criteria, and timeline live in
`runbooks/game_day_template.md` (§3–§6); the rows here carry the
EVIDENCE-CAPTURE checklist only. Every cell starts EMPTY.

| Evidence item | Captured as | Reference |
|---|---|---|
| Evidence JSON files (the experiment scripts' output) | file paths + their sha256 | `<EMPTY — NOT CAPTURED>` |
| Metrics screenshots / exposition snapshots | file paths | `<EMPTY — NOT CAPTURED>` |
| Transition-journal excerpts (C-PG-1) | file paths | `<EMPTY — NOT CAPTURED>` |
| Outbox delta readings (C-NATS-1) | file paths | `<EMPTY — NOT CAPTURED>` |
| The threshold-register rows in force (§1 of this file) | register row-version | `<EMPTY>` |
| The requirement cells in force (§2 of this file) | register row-version | `<EMPTY>` |
| Abort fired? (yes/no + criterion, evidence kept verbatim) | verdict | `<EMPTY>` |
| Post-run findings + dispositions | table rows in the game-day template §8 | `<EMPTY — NOT WRITTEN>` |

## 7. Template E — the chaos experiments' evidence-JSON shape (the `lab_harness.Evidence` cell inventory)

Each experiment script (C-PG-1, C-NATS-1, C-WORKER-1, C-POOL-1) writes one
evidence JSON via `lab_harness.Evidence` (`experiments/lab_harness.py`). The
cell inventory below is the transcription of that shape — it is a COPY for
template readers, never a second authority: if the harness changes, this
inventory is re-diffed and re-cited, and a mismatch is a FINDING (the
harness's own copy rule, applied). Every cell starts `(EMPTY — UNRUN)` in
the harness and stays EMPTY until a real run fills it.

| JSON cell | Filled by | Starts as |
|---|---|---|
| `experiment_id` | the recorder constructor | the experiment's ID |
| `executed_by` | the run | `(EMPTY — UNRUN)` |
| `witnessed_by` | the run | `(EMPTY — UNRUN)` |
| `lab_profile` | the run (URLs/env named) | `(EMPTY — UNRUN)` |
| `pre_fault_probe` | the run's probes | `(EMPTY — UNRUN)` |
| `fault_observed` | the run (the fault step) | `(EMPTY — UNRUN)` |
| `during_fault_probe` | the run's probes | `(EMPTY — UNRUN)` |
| `post_recovery_probe` | the run's probes | `(EMPTY — UNRUN)` |
| `oracle_results` | the run's oracle checks | `(EMPTY — UNRUN)` |
| `abort_fired` | the run | `(EMPTY — UNRUN)` |
| `wall_clock_start_unix_ms` | the run's clock at begin | `(EMPTY — UNRUN)` |
| `wall_clock_end_unix_ms` | the run's clock at end | `(EMPTY — UNRUN)` |
| `notes` | the run (errors recorded verbatim) | `(EMPTY — UNRUN)` |
| `oracle_findings[]` | the recorder (`check`, `passed`, `detail`) | empty list |

Rules carried with this shape (from the harness and records/21 §3.5,
unchanged): the recorder never computes a verdict — pass/fail judgement is
the human gate's against the pre-registered hypothesis; a failed oracle
check is a FINDING kept verbatim, never hidden, never auto-retried; the
experiments' timing CLI arguments are registered (X-8, §1) BEFORE the run.

## 8. Filling rules (binding on every template above)

1. **From the run only.** Every observed cell fills from the run's own
   clocks, logs, probes, and counts. A filled cell without a run is a
   fabrication defect (the P9.24 §8 rule, inherited).
2. **Pre-registration first (X-8).** No run that could fill a threshold-
   judged cell starts before §1's register row exists for it.
3. **Structural separation.** Registered values (§1–§2) and observed
   values (§3–§7 copies) never share a cell; a run record carries BOTH the
   register row-version and the requirement row-version it was judged
   against.
4. **Pointers, not rewrites.** Where an owning record holds the criteria
   (records/03, records/05, records/32, `runbooks/dr_site_loss.md`,
   `runbooks/game_day_template.md`), this file's rows are pointers; the
   criteria are never restated or modified here.
5. **Verbatim evidence.** Failures, aborts, and errors are recorded
   verbatim and kept — never hidden, never auto-retried.

## 9. Non-claims

No drill ran; no threshold is registered (the §1 register's EMPTY state is
its correct pre-run state, not a gap); no observed value exists anywhere in
this file; no game day is scheduled; no deployment exists to run anything
against. Zero observed violations from any finite run would be a TEST
OUTCOME, never a guarantee (§13.3 semantics; conventions §5). Nothing here
is a capability, conformance, or runtime-behaviour claim, and nothing adds
dispatch authority (ADR-010).

Status: **WRITTEN-UNRUN** — a template cannot produce evidence by being
written; every run it serves is a human gate that has not happened and is
not scheduled.
