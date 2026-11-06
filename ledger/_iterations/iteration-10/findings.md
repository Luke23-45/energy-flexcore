# Iteration 10 findings — §24 vertical-slice walkthrough + final whole-ledger robustness sweep

Reviewer: iterations 7–10 subagent. Date: 7 November 2026.
Scope: report §24 10-step vertical slice traced across phases 0–5; final robustness sweep
of all 11 phase ledgers (IDs, checkboxes, orphan references, simulation-only statements,
dates/versions/certification claims).
Method: read report §24 (lines ~2067–2086) and the owning subtasks in P0–P5 ledgers with
their acceptance criteria; scripted checks (subtask counts, checkbox counts, orphan
references, forward-phase references, date/version scans) run 7 Nov 2026.

Severity scale: Critical = a §24 step with no owning subtask, or a sweep failure that
invalidates the ledger; High = a step owned only implicitly; Medium = a real gap in one
release-gate check; Low = wording polish or a cross-phase note for the parent.

Counts: **High 0, Medium 1, Low 4. Critical 0.**

## (a) Missing subtasks

None. All ten §24 steps are owned (table below). No new subtask was needed; the one gap
found (M-10.1) was fixed by extending P10.09. P10 subtask total stays **30**.

## (b) §24 vertical-slice walkthrough — step ownership (all verified)

Report §24 is build-first: "Its steps land across v0.1.0–v0.3.0; they are not all promised
in the first public release." Each step below names the owning subtask(s) and the testable
acceptance that proves the step.

| §24 step | Owning subtask(s) | Testable acceptance (verified in the ledger) |
|---|---|---|
| 1. Tenant/portfolio/site/grid model + 4 simulated assets (BESS/PV/flexible load/EVSE) | P1.17, P1.26 (tenant/portfolio/site creation); P2.02 (entity graph incl. Site *—1 GridModel reference, PCC, bus reference); P2.03; P2.13–P2.14; P2.25 (≥2 sites, each BESS+PV+load+EVSE+site meter, via API, full 24 h horizon ingested) | P2.25 acceptance: two sites created through the API with the full asset set and a full 24 h horizon ingested |
| 2. Timestamped telemetry with quality flags | P2.07–P2.10; P2.15 (fault injection); P2.25 | Quality-flag handling and fault-injection cases with recorded outcomes |
| 3. Flexibility over a 24 h horizon | P3.03 (24-hour default horizon, 15-min interval as config, UTC internally, rollover supersedes never deletes); P3.12 (envelope; acceptance: strictly less than nameplate sum on a binding-constraint case) | Envelope strictly below nameplate sum where a constraint binds |
| 4. LP/MILP/QP schedule | P3.17 (Tier 1 LP/QP/MILP port; HiGHS never on SOCP); P3.24 (rolling-horizon re-scheduling, §24 steps 3–4) | Solver-class discipline in acceptance; rolling-horizon re-schedule produces a new schedule |
| 5. Feeder power-flow validation | P4.04–P4.07 (feasibility-gate pipeline); P4.08 (fail-closed) | Feasibility gate passes/fails per case; fail-closed on missing input |
| 6. PROPOSED command + validation/rejection/approval | P4.11 (PROPOSED + validation gate); P4.12 (VALIDATED→FEASIBLE/REJECTED); P4.13 (human approval/authorization gate) | Full transition matrix exercised; distinct validation, feasibility, and authorization gates |
| 7. Simulator-only dispatch | P4.16 (simulation-only dispatch adapter; hard guard test fails if any non-simulator target is configured; no device write path in code) | Guard test fails the build if a non-simulator target is configured |
| 8. Ack/tracking/partial failure/timeout simulation | P4.16 step 4 (simulate ack/tracking/partial failure/timeout; partial fleet failure per-asset) + P4.31 (fault injection incl. dropped/reordered/duplicated NATS messages, partial fleet response; integration on §24 steps 5–8; shadow evaluation) | Per-asset partial-failure handling; fault-injection outcomes recorded |
| 9. Console audit trail | P5.09 (Audit & Reports: full decision chain per command, schedule-vs-actual from measured values, failures/fallbacks with reasons, export preserves evidence refs) + P5.30 (exit-gate demo, operator runs it) | Decision chain retrievable per command; operator-run exit-gate demo |
| 10. Replay same scenario, same versions, verify reproducibility | P2.24 (deterministic replay); P5.18 (scenario record; replay uses only recorded inputs, missing input errors loudly); P5.19 (two clean runs match within pre-stated, pre-registered tolerances) | Two clean runs match within pre-registered tolerances |

### M-10.1 — No subtask ran the full §24 slice end-to-end on the release candidate
- **Severity: Medium.**
- **File + subtask:** `ledger/phase-10-v1-release-hardening/ledger_phase_10.md`, **P10.09**.
- Every step is owned and every gate is re-verified piecemeal in P10.03–P10.08 — but
  P10.09 step 4 ran only the step-10 replay ("replay the same scenario with the same
  versions"). Nothing ran steps 1–10 end-to-end against the candidate, so an integration
  failure between individually-passing gates could ship.
- **Patch applied:** P10.09 step 4 now runs the full §24 slice end-to-end on the candidate
  (steps 1–10 in order) and then the step-10 replay; acceptance (e) requires the run record
  to be filed alongside the replay.

## (c) Other-phase gaps found by the walkthrough (recorded precisely, NOT edited)

### L-10.1 — P3.12 acceptance does not name the 24 h horizon explicitly
- **Severity: Low.**
- **File + subtask:** `ledger/phase-03-*/ledger_phase_3.md`, **P3.12** (deferred — other phase).
- P3.12 owns the envelope and its acceptance is testable (strictly below nameplate sum on
  a binding-constraint case), but it does not state the 24 h horizon; the horizon is owned
  by P3.03 and the rolling horizon by P3.24. Suggested patch text for the parent: add to
  P3.12 acceptance "(e) the envelope covers the full 24-hour horizon at the configured
  interval (P3.03)". No contradiction — a coverage wording gap only.

### L-10.2 — P5.09 acceptance does not name validation/feasibility evidence display as one set
- **Severity: Low.**
- **File + subtask:** `ledger/phase-05-*/ledger_phase_5.md`, **P5.09** (deferred — other phase).
- P5.09 covers the full decision chain per command and the export preserving evidence refs;
  the validation/feasibility evidence display is split across P5.09/P5.11. Suggested patch
  text for the parent: add to P5.09 acceptance "(e) the audit view shows the validation,
  feasibility, and authorization evidence for a command as one set (P5.11 cross-reference)".
  No contradiction.

## (d) Final whole-ledger robustness sweep

| Check | Method | Result |
|---|---|---|
| Subtask counts | Count `### P?.??` headings per phase ledger | P0 30, P1 32, P2 30, P3 30, P4 32, P5 30, P6 30, P7 32, P8 30, P9 30, P10 30 — **332 total, matches the task brief**. P10 verified again after patching: 30 headings, 30 unchecked boxes, 0 checked. |
| Orphan references | Every `P?.??` reference resolved against its phase file's headings | **0 orphans across the whole ledger.** All P10 cross-references to P9 (P9.01, .03, .04, .06, .07, .08, .10, .12, .14–.27, .30) resolve. (An early grep miss on P9 headings was a pattern error — P9 uses `###`, not `##` — not a defect.) |
| Checkbox consistency | Count `- []` boxes per phase ledger | P0 114, P1 121, P2 124 (subtask + per-acceptance checkboxes); P3 30, P4 32, P5 30, P6 31, P7 33, P8 31, P9 31, P10 30 (single `- [] **Objective:**` box per subtask, lettered (a)–(d) acceptance). P6/P7/P8/P9 counts above 30 are subtask boxes plus intro/exit-gate boxes. See L-10.3. |
| Forward-phase dependencies | Parse every `Dependencies:` field | **None.** No phase's Dependencies field points to a later phase. Forward mentions are handover/consumed-by notes only (see iteration-08 findings). |
| Simulation-only stated where it matters | Grep simulation-only / shadow / read-only statements | P4.16 hard guard (fails if any non-simulator target configured); P10.01(b) guards certification claims; release ladder in the report and AGENTS.md consistent (0.x simulation-only; field write authority from v1.0.0 with operator approval; bounded automation post-1.0). No P10 text promises field control in 0.x. |
| Fake dates / versions / certification claims | Date/version scan of P10 files + spot-check of version claims (iteration 9) | **None found in P10.** No certification claim anywhere in P10; P10.01(b) explicitly guards against one. Research cut-off 7 November 2026 respected in all patched RESEARCH.md additions. |
| §22.3 gate mapping | Header mapping vs CHECKLIST | Item 9 mismatch found and fixed in iteration 8 (P10.11–P10.13). All 12 gates map one-for-one after the fix. |

### L-10.3 — Checkbox-format inconsistency: P0–P2 vs P3–P10
- **Severity: Low.**
- **File:** whole ledger (deferred — other phases; do NOT reformat).
- P0–P2 use a checkbox per subtask plus per-acceptance checkboxes (114/121/124 boxes);
  P3–P10 use one checkbox per subtask with lettered acceptance (30–33 boxes). Both formats
  are internally consistent and every subtask has exactly one objective checkbox; the
  difference is cosmetic and a reformat would churn 332 subtasks for no correctness gain.
  Recorded so a later reviewer does not mistake the count difference for missing subtasks.

### L-10.4 — P10.09 feeder "licensing" caveat is vague
- **Severity: Low.**
- **File + subtask:** `ledger_phase_10.md`, **P10.09**.
- Step 1 runs feeder cases "where licensing and model availability are confirmed". The
  caveat is honest but gives no rule for what happens to a case whose licensing is not
  confirmed. Acceptance (d) already requires a recorded reason for any dropped case, which
  covers it — so this is a suggestion, not a patch: when P10.09 runs, record the
  licensing-confirmation outcome per feeder case in the run record. No patch applied.

## (e) Stale research

None in iteration-10 scope. Versioned claims are covered in iteration 9.

## Remaining open Critical/High items after iteration 10

**None.** No Critical or High item remains open in Phase 10 after the patches applied in
iterations 7–10, and no Critical item was found anywhere in the ledger.

Open items of lower severity, for the parent to apply or track:

- **Medium (2, both in other phases' RESEARCH.md):** P7 Wasmtime CVE range/missing-flaw
  correction (M-9.3) and P1/P2 NATS CVE-2026-33222 fix-version wording + missing
  CVE-2026-33223 (M-9.4) — precise corrections recorded in iteration-09 findings.
- **Low (5, deferred):** L-10.1 (P3.12 horizon wording), L-10.2 (P5.09 evidence-display
  wording), L-10.3 (checkbox-format inconsistency — deliberately not reformatted),
  L-9.3 (SPP/CAISO FERC 2222 open verification), L-9.4 (OpenADR 3.1 certification profiles
  open verification), plus L-10.4 (P10.09 licensing-caveat suggestion, no patch) and the
  iteration-8 L-8.1 transitive-dependency note (deliberate, no patch).

## Applied vs deferred log

| Item | Disposition |
|---|---|
| M-10.1 | **Applied** — P10.09 step 4 (full §24 slice end-to-end on the candidate) + acceptance (e) |
| L-10.1 | **Deferred — other phase** — P3.12 wording, proposed patch text recorded above |
| L-10.2 | **Deferred — other phase** — P5.09 wording, proposed patch text recorded above |
| L-10.3 | **Deferred — deliberate** — checkbox-format inconsistency; reformat would churn 332 subtasks for no correctness gain |
| L-10.4 | **No patch** — covered by P10.09 acceptance (d); suggestion recorded for execution time |
