<!-- P10.23 — Operator-workflow stability check (check-design record). THE CHECK
     RECORD IS EMPTY: no walkthrough has been performed, no workflow has been
     repeated, no anomaly exists, and no console build under test exists
     (no release candidate). HONEST STATEMENT (binding): UI stability ≠
     correctness ≠ field readiness — a stability pass would establish only
     that the workflows repeat without NEW anomalies in the exercised
     browser cells, nothing more. -->
# P10.23 — Operator-Workflow Stability Check (Check Design + EMPTY Record — no walkthrough has happened)

Date: 2026-11-14. Subtask: P10.23 (`ledger_phase_10.md` §P10.23). Owner
role: Product (with frontend engineering and the customer operator lead).
Report trace: §20; §25 (pilot release: "stable operator workflow"). Inputs
per the ledger: P10.01 scope freeze (`01_release_scope_freeze.md`); P10.14
browser tests (`14_browser_os_matrix.md` — targets only, EVERY result cell
EMPTY); Phase 9 training evidence (P9.21 — register EMPTY, no trainee
exists) and procedures (P9.22) — cross-referenced. ENTRY_NOTE boundaries 1
and 4 apply. Deliverable: this record — the workflow inventory (§2), the
stability CHECK procedure with the pre-registered repetition count (§3),
and the check record as an EMPTY template (§5).

## 1. Honest status — the plain statement

**No operator-workflow walkthrough has been performed on any candidate
console, no workflow has been repeated the procedure's count of times, no
anomaly has been recorded, and no stability result of any kind exists.**
The candidate console build does not exist (ENTRY_NOTE boundary 1 — the
release candidate is a described build procedure, never a built artifact),
and no browser in the P10.14 matrix has ever loaded the console (that
record's every result cell is EMPTY). Every §5 record cell is deliberately
EMPTY. What this record delivers is the CHECK DESIGN, pre-registered so
the repetition count, the anomaly definition, and the blocking rules are
fixed before any human run and cannot be fitted to results afterwards.
The ledger's acceptance criteria (a)–(c) are NOT met and NOT claimed.

## 2. The workflow inventory (what the check walks) — cited to the console code

The §20.1 workspace registry (`console/src/workspaces/registry.ts`) is the
single source of truth for the twelve console areas; the check walks all
twelve, in registry order, on the candidate console build:

| # (§20.1) | Workspace id | Component (cited by path) | Workflow walked |
|---|---|---|---|
| 1 | overview | `console/src/workspaces/Overview.tsx` | aggregate read, key alarms (`components/alarmsPanel.tsx`, derived-on-read `GET /api/v1/alarms`; the alarm panel renders an honest error block when the read fails — never a fabricated alarm), delivery status |
| 2 | portfolios | `console/src/workspaces/Portfolios.tsx` | tenant-scoped portfolio read + export state |
| 3 | sites | `console/src/workspaces/Sites.tsx` (+ `SitesEdgeStatus.tsx`) | site read; edge connectivity/health/status surfaces |
| 4 | assets | `console/src/workspaces/Assets.tsx` | asset read; value-type quality display (`components/ValueType.tsx`) |
| 5 | grid | `console/src/workspaces/Grid.tsx` | grid-model read surfaces |
| 6 | schedules | `console/src/workspaces/Schedules.tsx` | schedule read/lifecycle display |
| 7 | dispatch | `console/src/workspaces/Dispatch.tsx` | THE full dispatch walkthrough (§3 item 2 below) |
| 8 | market-programs | `console/src/workspaces/MarketPrograms.tsx` | market/program read surfaces |
| 9 | simulation-lab | `console/src/workspaces/SimulationLab.tsx` | lab run/replay surfaces |
| 10 | plugins-integrations | `console/src/workspaces/PluginsIntegrations.tsx` (+ `PluginsWorkspace.tsx`) | plugin registry/lifecycle read surfaces |
| 11 | audit-reports | `console/src/workspaces/AuditReports.tsx` | audit trail read/export |
| 12 | administration | `console/src/workspaces/Administration.tsx` | admin/config read surfaces |

The three focused workflows the ledger names, composed from the inventory:

1. **Dispatch lifecycle in shadow (propose → validate → feasibility →
   approve → hold before issue).** The Dispatch workspace displays the
   14-state lifecycle with the P0.11 vocabulary exactly; shows validation,
   feasibility, and authorization evidence as THREE DISTINCT blocks; and
   performs NO approval from the console (no transition endpoint exists —
   the approval assembles its parameters with explicit confirmation and
   resolves read-only). The candidate-side walkthrough exercises exactly
   this surface and holds before issue; no state beyond the hold is
   reachable in shadow by design.
2. **Alarm handling.** The Overview alarms panel (derived-on-read; honest
   error, never fabricated). NOTE the carried P9.21 gap R-5 (P9.08 gap
   G3): the authz surface has NO alarm-acknowledgement action — the
   operator procedure routes acknowledgement outside FlexCore until the
   closed-enum extension is reviewed and added. The check records this gap
   in the walkthrough; it does not treat the missing action as a defect
   introduced by the release.
3. **Telemetry query + §20.2 modes display.** The value-type treatment
   (`ValueType.tsx`: measured / estimated / stale / simulated visually
   distinct) and the persistent control-authority-mode indicator; the
   check verifies the §20.2 principle list per item below.

## 3. The stability CHECK procedure (pre-registered; the human gate)

**Pre-registered parameters (fixed now, before any run; a change after a
run has started voids the run for the affected workflow):**

| Parameter | Value | Basis (recorded, so the count is not arbitrary) |
|---|---|---|
| N = repetitions per workflow per browser cell | **5** | large enough to surface flaky rendering/state regressions on a finite set of surfaces; NOT a statistical sample — recorded as a structural minimum, not a proven adequate number |
| Browser cells exercised | every P10.14 matrix cell that P10.14 records as tested (plus, per the ledger's named risk, the previous-major-version cells) | stability is browser-relative; an untested browser cell records the workflow UNTESTED there, never assumed |
| Environment | the candidate console build (P10.18) against a staging deployment; recorded build identifier | the check is meaningless against any other build |
| Executors | first pass by product/frontend; the ledger's verification method adds an independent second walkthrough (ideally the customer operator) | two-person discipline; the second walk is a separate recorded run |

**Per workflow, per repetition, per browser cell:**

1. Load the workspace from a cold console state; record load success and
   any console/network error.
2. Exercise the workflow's actions in order (per §2's "workflow walked"
   column); for the dispatch workflow hold before issue every time.
3. Record an ANOMALY for any of: a failed/incorrect render; a state
   distinction that fails (§4 list); a workspace that errors instead of
   showing its honest empty/error state; any control that appears to
   perform an action the read-only surface cannot perform; any
   high-impact action reachable without its explicit confirmation
   (`ConfirmHighImpact`); any emergency control rendered where the design
   defines none (the Dispatch workspace records the §20.2 emergency-
   separation item as an OPEN ITEM, not a feature — an emergency control
   appearing IS an anomaly).
4. Repeat N times; an anomaly that reproduces is a defect; one that does
   not reproduce is still recorded (flaky = unstable = a finding).
5. Record EVERY anomaly with: workspace, repetition index, browser cell,
   observed vs expected, screenshot/console-log artifact reference.

**Disposition rule (ledger step 5):** every anomaly gets a disposition
(fix / defer-with-rationale / blocks the release). A defect that HIDES a
command state, an approval, or a rejection reason blocks the release —
non-negotiable; a stability defect that does not hide state is dispositioned
on its own merits.

## 4. The §20.2 principle checklist (each verified with evidence, per run)

| # | §20.2 principle | Where the console encodes it (cited) |
|---|---|---|
| 1 | Operational state and freshness prominent | `Overview.tsx`, `SitesEdgeStatus.tsx` freshness surfaces |
| 2 | Measured / estimated / stale / simulated visually distinct | `components/ValueType.tsx` (`ValueTypeBadge`, legend); `Overview.tsx` value typing |
| 3 | Proposed never shown as executed | `components/honesty.tsx` (single source; `SIMULATED_STATE_MEANINGS`); `Dispatch.tsx` honesty renderings |
| 4 | Candidate/feasible/approved/issued/acknowledged/measured visually distinct | `components/stateVocabulary.ts` COMMAND_STATES; `Dispatch.tsx` lifecycle badges |
| 5 | Rejected commands explained with the failing constraint and evidence | `Dispatch.tsx` rejection surfaces (every rejection names its failing check) |
| 6 | Model version and timestamp shown for feasibility outcomes | `Dispatch.tsx` feasibility evidence block |
| 7 | Explicit confirmation for high-impact actions | `components/ConfirmHighImpact.tsx` |
| 8 | Emergency actions visually separate | recorded OPEN ITEM (P5.06 step 5) — the console defines NO emergency action; the check verifies none appears |
| 9 | Role-specific screens: a role sees only its relevant controls | `useCan` capability gating (`Dispatch.tsx` P5.26); denial renders the REASON, not a broken button |
| 10 | Control-authority mode displayed persistently | the persistent §21.4 mode indicator (P5.13; `components/modeGateNotice.tsx`) |

Each row's verification cell lives in the §5 record and stays EMPTY.

## 5. The check record — ALL CELLS EMPTY (UNRUN)

| Field | Value (EMPTY — fill at execution) |
|---|---|
| Release-candidate console build identifier (must equal P10.18) | |
| Staging deployment reference | |
| Browser cells exercised (from P10.14 tested cells) | |
| Executors (first walk / independent second walk) + dates | **UNSIGNED** |
| N actually completed per workflow × browser cell | |

**Per-workflow stability table** — one block per §2 workflow × browser cell;
all `(EMPTY — UNRUN)`:

| Workflow | Browser cell | Repetitions completed (N=5) | Anomalies observed (count) | Anomaly refs | Result (STABLE / UNSTABLE / NOT-EXERCISED-with-reason) |
|---|---|---|---|---|---|
| (one row per combination — ALL EMPTY) | | | | | |

**Anomaly register:**

| # | Workspace | Rep | Browser | Observed vs expected | Evidence artifact | Disposition (fix / defer / blocks the release) |
|---|---|---|---|---|---|---|
| (none — exists only when an anomaly is recorded from actual output) | | | | | | |

**§20.2 principle verification table** — one row per §4 principle, per
browser cell exercised:

| Principle (§4 #) | Browser cell | Verified? | Evidence artifact | Defect ref (if any) |
|---|---|---|---|---|
| (ALL EMPTY — UNRUN) | | | | |

## 6. What a stability pass would and would not establish

**It WOULD establish only:** that on the recorded candidate build, in the
recorded browser cells, the twelve §20.1 workspaces and the three focused
workflows completed N repetitions without NEW anomalies, and every §20.2
principle verified with recorded evidence — UI STABILITY on exercised
cells.

**It would NOT establish any of the following, and none may be written
about a filled record:**

- **Correctness.** UI stability is not behaviour correctness: the console
  can render consistently and still display data the platform computed
  wrongly; the approval and lifecycle GATES are application-layer
  (`routes_phase4.rs` exposes read-only dispatch-command resources) and
  their proof lives in P10.03/P10.15, never here.
- **Field readiness.** Nothing here grants or evidences field write
  authority; the walked workflow HOLDS BEFORE ISSUE by design, in shadow.
- **Safety, certification, or protection-grade properties** — AGENTS.md §3
  rules 2–3 bind every word of a filled record.
- **Operator competence.** Training/qualification is P9.21 (register
  EMPTY); a stable console and a qualified operator are different gates.
- **Stability on untested cells.** A browser cell P10.14 did not test is
  NOT-EXERCISED here — never assumed stable.

## 7. Reviewer-verification procedure

1. Confirm §1's plain statement stands verbatim; every §5 cell is
   EMPTY/UNRUN; no executed-check claim exists anywhere.
2. Confirm the §3 parameters (N=5, browser-cell rule, disposition rule)
   are pre-registered and unchanged, and the §2 inventory cites real
   console files (the twelve registry entries resolve to
   `console/src/workspaces/*.tsx`).
3. Confirm §6's non-claims are intact (stability ≠ correctness ≠ field
   readiness) and §4's principle list matches §20.2.
4. Confirm the carried P9.21 gap R-5 (no alarm-acknowledgement action)
   is stated as a carried gap, not a release defect.
5. For a filled future record: every anomaly traces to an artifact; every
   NOT-EXERCISED has a reason; the state-hiding-defect block rule was
   applied; the second-person walkthrough is a separate recorded run.
6. Confirm the ledger tick wording is "implemented, awaiting human
   build/test" — the check design is written; the check has not run.

## 8. Filling commands (for the human executor)

```sh
# (a) the documented-tools gate (run after ANY edit to this record):
python tools/verify_docs.py
python tools/validate_agent.py
python -I tools/check_dependency_rule.py

# (b) PRECONDITIONS: the P10.18 candidate console build exists; P10.14 has
#     tested browser cells recorded (both currently EMPTY — no run can start).

# (c) Serve the candidate console against the staging deployment; execute
#     the §3 procedure (workflow × repetition × browser cell), recording §5
#     FROM THE ACTUAL OUTPUT only. Hold before issue on every dispatch
#     repetition.

# (d) Independent second walkthrough (ideally the customer operator) as a
#     separate recorded run; reconcile anomalies between the two runs.

# (e) File the filled record as the P10.23 evidence in CHECKLIST.md; hand
#     the result to P10.17 (operator-error risk input) and P10.27.
```

## 9. Findings (recorded, not fixed)

- **F-P10.23-1:** no candidate console build exists; the check cannot
  start, and every browser cell in P10.14 is untested — the §3
  browser-cell rule therefore currently resolves to an empty set.
- **F-P10.23-2:** the §20.2 emergency-separation principle is an OPEN
  ITEM in the console (P5.06 step 5 — no emergency action is defined);
  the check verifies ABSENCE (no invented emergency control), and any
  appearance is an anomaly by pre-registration.
- **F-P10.23-3 (carried, P9.21 R-5 / P9.08 G3):** no alarm-acknowledgement
  action exists on the authz surface; the walkthrough records the gap and
  routes acknowledgement outside FlexCore, as the operator procedure
  trains.
- **F-P10.23-4 (carried, P10.14):** the browser/OS matrix has no tested
  cell, so cross-browser stability is entirely unestablished — the ledger's
  named risk ("a console regression that only appears on the previous
  major browser version") cannot even be looked for yet.

Status: **WRITTEN-UNRUN** — the stability check is a human gate; no
walkthrough, no repetition, no anomaly, and no result exists; every §5
cell is EMPTY and stays EMPTY until the human execution fills it from
actual console output. Nothing was built, run, or rendered, and no
stability claim is made.
