# Decision Record — P5.05 (Schedules), P5.06 (Dispatch), P5.09 (Audit & Reports)

- **Date:** 2026-11-11 (Phase 5 wave 2b; loop rules in ENTRY_NOTE.md).
- **Subtasks:** P5.05 (§20.1 item 6, §12.1, §12.6, P3.21–P3.23), P5.06
  (§20.1 item 7, §14.1–§14.2, P4.30), P5.09 (§20.1 item 11, §15.3, §21.3,
  §24 step 9, P4.36).
- **Loop status:** NOTHING was installed, built, type-checked, linted, or
  run. No `npm install`, no lockfile, no `node_modules`, no `tsc`/`vite`/
  `vitest` execution. All code and tests are complete source written only;
  every test file carries an `UNRUN` header and the human executes them at
  first install. No checkbox in `ledger_phase_5.md` claims
  tested/verified/passed — the three ticked boxes read "implemented,
  awaiting human build/test".
- **TypeScript was never compiled.** The tree targets the existing scaffold
  config (strict + `exactOptionalPropertyTypes`, `verbatimModuleSyntax`,
  `noUnusedLocals`); the code was written against those flags, but whether
  it type-checks cleanly is exactly what the human's first `tsc -b` proves.
  Honest limitation: no compile has occurred, so type-level defects may
  exist.

## 1. What was built

### Shared (new files, consumed by all three workspaces)

| File | Content |
|---|---|
| `console/src/components/stateVocabulary.ts` | The frozen display vocabularies, verbatim from the server sources: the 14 P0.11 command states + per-state meanings/markers, the three gates, the §12.6 solver outcomes, the schedule revision states + P3.24 roll triggers, the six §10.4 quality states, the 26 frozen §15.3 event names, the declared objective set (P3.14), the §24-step-9 decision-chain stages + event→stage map, time helpers. |
| `console/src/components/honesty.tsx` | Honesty primitives: explicit loading/error/empty states, the §20.2 freshness stamp, verbatim recorded-statement rendering, the 14-state lifecycle badge and the six-state quality badge (both fail-honest: an out-of-vocabulary value renders verbatim with an UNKNOWN marker, never mapped). |
| `console/src/api/schedules.ts` | NEW client for `/api/v1/schedules` + `/api/v1/optimization-runs` with the FULL Phase3ListParams (incl. the `schedule_id` full-chain filter). `flexibility.ts` (P5.01) deliberately untouched — this is an additive module, per file ownership. |

### P5.05 — Schedules (`console/src/workspaces/Schedules.tsx` + `.test.tsx`)

- Schedule revisions grouped per chain (ID + revision, horizon with
  start-inclusive/end-exclusive labelling, roll trigger against the closed
  P3.24 vocabulary, stored state, supersedes/superseded-by links,
  candidate_id), with the P3.21 candidate-status statement and the
  ScheduleValidated validation-scope caveat rendered VERBATIM from the
  record fields (never paraphrased).
- **Superseded revisions are retained and displayed** (state `superseded`
  renders as "SUPERSEDED (retained for audit; retrievable, never deleted)");
  the `schedule_id` full-chain filter is the retrieval path (P5.05 step 4).
- §12.6 run detail: solver-outcome status (closed vocabulary, verbatim if
  unknown), problem class, solver + version as recorded facts, provider,
  objective value + units, input-snapshot reference, grid-model version +
  site reference (the never-detached pairing shown).
- **Objective selection display (D1/D2 OPEN, honest):** the declared
  objective set has EXACTLY ONE entry — `SYNTHETIC_ENERGY_COST_DEVIATION_V1`,
  provenance `SYNTHETIC`, schema version 1, with all five terms
  (energy/degradation/deviation/curtailment/service) and their weight units
  and sign conventions copied verbatim from
  `crates/flexcore-application/src/scheduling_objective.rs`. The workspace
  states that selection means choosing among this declared set, that
  free-text objective entry bypassing the declared set does not exist, and
  that the objective is explicitly "not a market product … (D1/D2 OPEN)".
- Objective extraction from the persisted record JSON is DEFENSIVE
  (`extractObjectiveFromRecord` reads `input_snapshot.objective_name` /
  `objective_config_record`); when absent the UI says "the record JSON does
  not carry the objective config on this surface" — no inferred
  configuration.
- Alternatives/scenario comparison: candidate rows (objective value + units,
  status, solver, input snapshot) with the P3.23 shared-input rule ENFORCED
  IN THE COMPONENT (`assessComparison`): candidates whose
  `input_snapshot_id` differ (or fewer than two found) render the
  INCOMPARABLE verdict — the label is code, not reader discretion.

### P5.06 — Dispatch (`console/src/workspaces/Dispatch.tsx` + `.test.tsx`)

- Command list with the state filter over the closed 14-state vocabulary;
  each item renders the P0.11 badge (marker + exact spelling + meaning).
- Command detail: full immutable journal (seq, from → to, actor, timestamp,
  gate, reason, evidence reference), command payload verbatim, and the
  payload's `no_execute_endpoint_statement` + `simulation_only_statement`
  rendered VERBATIM.
- **The three gates as DISTINCT evidence** (`gateEvidence` → three separate
  panels/entries, one per gate, each with its own evidence reference and
  actor; non-gate journal entries are never presented as gate evidence;
  the panel states that the gates are never combined into one verdict).
- Rejection view (`rejectionView`): WHICH gate rejected, the failing check
  (journal reason), the evidence reference, plus the command-scoped
  feasibility-check record (LIMIT_VIOLATED failing constraint / FAIL_CLOSED
  reason, model version, correlation id). No check record → "none is
  invented".
- Approval flow (explicit confirmation, §20.2): assembles actor (from the
  claims-derived session, never free text), scope (tenant/portfolio/site/
  asset/schedule+revision), limits (only from the payload's `limits` field
  when present — otherwise an explicit "the console does not invent limit
  figures" statement), and expiry (the §14.3 valid-until bound). Resolution
  is **always the missing-endpoint gap or a not-in-this-state note** — see
  §3 below.
- **Simulated labelling (P5.13 wording, tested):** `simulatedMeaning(state)`
  — ISSUED = "issued to the SIMULATOR"; ACKNOWLEDGED = "APPLICATION-LAYER
  ACCEPTANCE by the simulator — NOT physical delivery"; TRACKING/COMPLETED/
  FAILED/FALLBACK all carry the SIMULATED qualification. The meanings live
  in the state vocabulary itself, so the qualification cannot be forgotten
  per screen.
- **Emergency actions = OPEN ITEM, not a feature:** §20.2 requires visual
  separation but no emergency action is defined for this phase (CONTEXT.md
  open question 1); the workspace renders a persistent open-item note and
  no emergency control (P5.06 step 5).

### P5.09 — Audit & Reports (`console/src/workspaces/AuditReports.tsx` + `.test.tsx`)

- Trail over GET /api/v1/audit-events with the P4.36 filters (action, actor,
  object_type, object_id, inclusive from/to window) and keyset cursor
  pagination (accumulating "Load next page"); an unparseable time bound is
  reported as "not sent" — never silently dropped or guessed.
- Every §10.1 field displayed per record: actor, action, object,
  **decision_outcome + decision_reason_reference** (a missing reason
  reference next to an outcome renders a visible "defective record" note —
  the two are recorded together, P1.11), **evidence**, **before_ref /
  after_ref** (pointers, not payload copies), the distinct P0.09 event vs
  ingest times, the §21.3 correlation id, and the **correction linkage**
  (`corrected_audit_id` — "a correction is a NEW row; the original is never
  edited or removed"). The append-only statement is rendered verbatim.
- Decision chain per command (§24 step 9): the loaded records for one
  object_id, event-time ordered, classified onto the eight chain stages via
  the frozen §15.3 event-name map; stages with no match render "no loaded
  record classified to this stage … nothing is inferred into a stage";
  actions outside the map render verbatim under "unclassified".
- Schedule vs actual: requested = the persisted command payload (verbatim
  JSON); measured = ONLY the telemetry query result (per-asset window the
  operator specifies), each value with its §10.4 quality state and the
  simulated flag. **No tracking verdict is computed** — the P4.16
  acceptance-criterion/tracking-error record has no console read endpoint
  (gap, §3); the view explicitly draws no agreement conclusion.
- Dispatch failures/expiries/fallback activations: filtered from the loaded
  records (DispatchFailed / DispatchExpired / DispatchFallbackActivated)
  with their evidence references; empty scope renders an honest empty state.
- Operator actions: the frozen `OperatorActionRecorded` event class with
  actor + timestamp + evidence.
- **Export:** client-side JSON over the CURRENTLY LOADED records only
  (`buildAuditExport`), preserving every served field verbatim — tested by
  JSON round-trip for evidence, decision/reason references, before/after
  refs, and correction linkage. The export states its scope (a copy of the
  loaded pages, "not the complete trail") because the server has no export
  endpoint by design.

## 2. Lifecycle-vocabulary mapping (P5.06/P0.11)

The display vocabulary IS the server vocabulary — no renaming, no
aliases, no merging:

| Source (server) | Console display |
|---|---|
| `command_state.rs` `CommandState::name()` (P0.11 freeze, 14 states) | `COMMAND_STATES` — exact strings, frozen order: PROPOSED, VALIDATED, FEASIBLE, APPROVED, ISSUED, ACKNOWLEDGED, TRACKING, COMPLETED, REJECTED, FAILED, EXPIRED, CANCELLED, SUPERSEDED, FALLBACK. Badge: marker + `<strong>{state}</strong>` + the §14.1 meaning. Unknown string → verbatim + "[UNKNOWN STATE …]" (never mapped). |
| `command_lifecycle.rs` `Gate::as_str()` | `GATES` / journal `gate` field rendered verbatim: `validation`, `feasibility`, `authorization` — three distinct evidence blocks. |
| §12.6 solver outcome (dto_phase3.rs) | `SOLVER_OUTCOMES` verbatim (optimal/feasible/infeasible/timeout/numerical_failure/unsupported); unknown → verbatim + marker. |
| Roll trigger (routes_phase3.rs, P3.24) | `ROLL_TRIGGERS` verbatim; unknown → verbatim + marker. |
| Schedule revision state (dto_phase3.rs `active`/`superseded`) | Displayed as recorded; `superseded` additionally rendered "SUPERSEDED (retained for audit; retrievable, never deleted)". |
| §10.4 quality (dto.rs) | `QUALITY_STATES` verbatim via QualityBadge; unknown → verbatim + marker. |
| §15.3 events (events.rs FROZEN_EVENT_NAMES, 26) | `FROZEN_EVENT_NAMES` verbatim; chain stage classification only via `EVENT_STAGE_MAP`. |
| P3.14 objective (scheduling_objective.rs) | `DECLARED_OBJECTIVES` verbatim (name/provenance/term names/weight units/sign conventions). |

The per-state meanings carry the §20.2/P5.13 qualifications structurally:
PROPOSED…APPROVED all state that the value is still proposed (never
executed); ISSUED/ACKNOWLEDGED/TRACKING/COMPLETED/FAILED/FALLBACK all carry
the SIMULATED qualification; ACKNOWLEDGED states "NOT physical delivery";
CANCELLED and SUPERSEDED are stated as DISTINCT (§14.2).

## 3. Missing-endpoint gap records (honest, not invented)

1. **No transition endpoint for the authorization gate (P5.06).**
   `apps/control-plane/src/routes_phase4.rs` exposes GET-only resources
   (`/api/v1/dispatch-commands` (+`/{command_id}`),
   `/api/v1/feasibility-checks` (+`/{check_id}`), `/api/v1/audit-events`
   (+`/{event_id}`)). The §14.1 gates are application-layer, not REST —
   and no gate-safe transition endpoint is exposed to a console client.
   The console therefore does NOT invent a POST: the approval flow
   displays and requires explicit confirmation of the approval parameters,
   then resolves (tested, `resolveApprovalAction`) to the recorded gap —
   "NO approval was performed from the console; the command state stays
   read-only and the gate remains the application-layer authorization
   action." This matches the Phase 4 handover §1 ("no generic execute
   endpoint, §15.2") and the P4.30 read-only contract. A future subtask
   that adds a gate-safe, action-scoped transition endpoint can replace
   `resolveApprovalAction` at a single seam.
2. **No read endpoint for the P3.23 comparison records (P5.05).** Neither
   `ScheduleComparisonRecord` (`schedule_comparison.rs`) nor the P3.18
   Tier 1 vs Tier 2 cross-check record (`conic_fixture.rs`
   `Tier1Tier2ComparisonRecord`) is served by any GET route
   (routes_phase3.rs serves envelopes/schedules/optimization-runs only).
   The comparison view therefore derives comparability CLIENT-SIDE from
   `input_snapshot_id` equality (the shared-input rule still enforced),
   shows per-candidate metrics only from the §12.6 records, and states the
   gap on-screen; the Tier 1 vs Tier 2 record (the recorded 1 MW
   linearisation gap) is NOT displayed as if available.
3. **No read endpoint for the P4.16 acceptance criterion / tracking-error
   record (P5.09).** The schedule-vs-actual view juxtaposes requested
   (command payload) with measured (telemetry) values and explicitly
   computes NO verdict; the gap is stated on-screen.
4. **No server export endpoint (P5.09).** By design (append-only trail,
   P1.10/P4.36); the export is client-side over the loaded records with an
   honest scope note, and it preserves references (tested).

## 4. Decisions (labelled DEFAULT where they are this wave's choice)

1. **DEFAULT — new additive API module instead of editing `flexibility.ts`.**
   File ownership for wave 2b names NEW api files; `schedules.ts` mirrors
   the full Phase3ListParams (the chain filter is a P5.05 requirement) and
   leaves the P5.01 module untouched.
2. **DEFAULT — containers take an optional injectable `client` prop.** The
   workspaces keep the `api` singleton as the default dependency; tests
   inject `createApiClient({fetchImpl})`. Same pattern as the P5.01
   endpoint modules. App.tsx needed NO edit (export names unchanged:
   `SchedulesWorkspace`, `DispatchWorkspace`, `AuditReportsWorkspace`).
3. **DEFAULT — tests render via `react-dom/server` `renderToString`.** The
   scaffold has no @testing-library/jsdom dependency and none may be
   installed; renderToString works in the node Vitest environment over the
   presentational components, while the containers' data loading is tested
   through the pure loaders against a mocked fetch. Effect-driven container
   flows are NOT rendered in tests (a recorded limitation: `useEffect`
   behaviour needs the real DOM test the human runs).
4. **DEFAULT — defensive JSON readers, never schema assumptions.** The §12.6
   record payload and the command payload are `serde_json::Value` on the
   wire; extractors (`extractObjectiveFromRecord`, the payload `limits`
   read) return "absent" honestly instead of guessing shapes.
5. **DEFAULT — fail-honest unknown values.** Any value outside a closed
   vocabulary renders verbatim with an explicit UNKNOWN marker. This is
   chosen over throwing so a vocabulary drift on the server degrades into
   visible honesty rather than a broken screen — the drift still fails the
   vocabulary tests, which pin the exact sets.
6. **DEFAULT — the approval checkbox is a required confirmation step with no
   server effect.** Confirmed-but-ungateable is the honest state until a
   transition endpoint exists (gap record 1). The confirmation UI is
   deliberately kept so the P5.26/P5.11 step-up/confirmation work can wire
   the real gate at the same seam.

## 5. Honest limitations

- **Nothing was executed**: no install, no build, no type-check, no lint,
  no test run. The three test files are complete but UNRUN (headers say so).
  The containers' `useEffect` flows, the checkbox onChange flow, and the
  Blob download are unexercised by construction (renderToString cannot run
  effects; downloads need a DOM).
- **TypeScript was never compiled** — type-level defects are possible
  despite the strict-mode discipline applied (exactOptionalPropertyTypes
  hazards were explicitly handled: no explicit `undefined` into optional
  props; two documented casts restore DTO shapes on wire objects built
  without undefined values).
- The registry was NOT edited (ids/titles unchanged, export names kept) —
  zero overlap with the concurrent wave-2a agent's file ownership.
- Freshness is displayed as the console-read timestamp per loaded dataset
  (§20.2); STALE telemetry marking comes from the per-value quality state
  where the records carry it. The P5.10/P5.12 workspace-wide legend and
  treatments are later waves and may restyle (not remove) these contracts.
- Live-update integration (P5.25 stream) is not consumed by these three
  workspaces yet; they read the REST evidence surfaces directly. Wiring the
  stream into these views is a later-wave concern (P5.33 owns ordering).
- The objective term list, weight units, and sign conventions were copied
  from `scheduling_objective.rs` as recorded facts; no solver-capability
  claim is made anywhere (AGENTS.md rule 4: HiGHS appears only as a
  recorded solver name/version from the record).
- Export is a Blob download of loaded records only; it is not a bulk trail
  export and never will be from the client side — a server-side export,
  if ever added, is a P4.36-owner decision (the append-only statement says
  there is no such endpoint).
