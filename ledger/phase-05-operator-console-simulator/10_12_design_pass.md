# Decision Record — P5.10 (freshness/value types), P5.11 (state distinction + confirmations), P5.12 (accessibility, roles, demo honesty)

- **Date:** 2026-11-11 (Phase 5 wave 5a — the §20.2 design-principle pass; loop rules in ENTRY_NOTE.md).
- **Subtasks:** P5.10 (§20.2, §10.4), P5.11 (§20.2, §14.1, §12.5), P5.12 (§20.2, §21.4, §17.3).
- **Loop status:** NOTHING was installed, built, type-checked, linted, or run. No `npm install`, no `tsc`/`vite`/`vitest` execution. All code and tests are complete source written only; every test file carries an `UNRUN` header and the human executes them at first install. No checkbox in `ledger_phase_5.md` claims tested/verified/passed — the three ticked boxes read "implemented, awaiting human build/test".
- **TypeScript was never compiled.** The tree targets the existing scaffold config (strict + `exactOptionalPropertyTypes`, `verbatimModuleSyntax`, `noUnusedLocals`); whether it type-checks cleanly is exactly what the human's first `tsc -b` proves.
- **Concurrent-wave merge (recorded):** wave 5b (P5.13/P5.26) landed MID-WAVE on overlapping surfaces (`console/src/App.tsx` shell, `components/honesty.tsx`, `workspaces/Dispatch.tsx`). This pass's edits were composed with, not reverted by, that landing: the Dispatch approval flow now chains `ConfirmHighImpact` (P5.11, this pass) → the P5.26 step-up panel → the P5.13-qualified gap resolution. One contested edit was reconciled: an unused `HIGH_IMPACT_ACTIONS` import left in `Dispatch.tsx` by the merge was removed (`noUnusedLocals` would fail the human's type-check). No file was duplicated or forked.

## 1. Audit findings — workspace × principle × status before/after

Legend: **OK** = already satisfied before this pass (verified against the implementation, not assumed); **FIXED** = gap found and fixed here; **N/A-DECLARED** = the principle's subject does not exist on the surface and the absence is stated on-screen (the honest form).

| Workspace | Freshness on every value (P5.10a) | Value type per value (P5.10b) | Canonical units (P5.10c) | Distinct states (P5.11a) | Model version+timestamp on feasibility outcomes (P5.11b) | High-impact confirmation (P5.11c) | Role line (P5.12a) | Colour-independence (P5.12b) |
|---|---|---|---|---|---|---|---|---|
| Overview | OK | OK (ValueTypeBadge + quality) | OK (MW + §10.2 sign label) | OK (delivery counts carry the ISSUED/ACKNOWLEDGED qualifications) | N/A-DECLARED (no feasibility records displayed) | N/A (no approval here) | OK | OK |
| Portfolios | FIXED (composition block had no freshness anchor) | N/A-DECLARED (registry facts; the freshness line now states they are never typed measured/estimated/simulated) | OK (MW/MWh in rated limits on Assets; envelope has interval counts) | OK (active/superseded rendered as text states) | N/A-DECLARED | N/A | OK | OK |
| Sites | OK | N/A-DECLARED (same registry-facts statement pattern) | OK | OK | OK (grid-model binding row: version + hash + checked-at) | N/A | OK | OK |
| Assets | FIXED (availability line carried no freshness; now renders the availability record's event time + age) | OK (per-value type + quality in the telemetry table) | OK (MW/MWh; availability unit `1`) | OK (availability states text-borne) | N/A-DECLARED | N/A | OK | OK |
| Grid | OK | OK (outcomes are record-borne) | OK (margins verbatim) | OK (PASSED / LIMIT_VIOLATED / FAIL_CLOSED as distinct labelled outcomes with distinct notes) | OK (verified, not re-implemented — P5.04) | N/A | OK | OK |
| Schedules | OK (FreshnessStamp) | N/A-DECLARED (§12.6 objective values carry recorded units; no telemetry typed here) | OK (objective value + units) | OK (revision states + closed solver-outcome vocabulary) | OK (grid-model version + site on the run detail) | N/A | FIXED (RoleScopeLine was missing from the wave-2b workspaces) | FIXED (two tables had no caption/scopes) |
| Dispatch | FIXED (command detail had no freshness stamp) | OK (proposed values carry the PROPOSED treatment until ACKNOWLEDGED, §20.2) | OK (payload verbatim) | OK (shared 14-state badge with unique text markers) | FIXED (rejection evidence carried model version but NOT the checked-at timestamp; added) | FIXED (inline checkbox replaced by the shared `ConfirmHighImpact` component) | FIXED (as Schedules) | OK |
| Market & Programs | OK (`Read at {readAt}` captions) | N/A-DECLARED (explicit no-market-adapter / synthetic statements) | N/A-DECLARED | OK | N/A-DECLARED | N/A | OK | OK |
| Simulation Lab | FIXED (runs section had no freshness anchor; readAt was passed but unused) | OK (every run labelled simulated, §20.2) | N/A-DECLARED (metrics not yet served — P5.08) | OK (FAULT-INJECTED tag is a distinct text state) | N/A-DECLARED | N/A (the clean re-run is a documented CLI command, not a console action) | OK | OK |
| Plugins & Integrations | FIXED (edge-connectivity list had no freshness anchor) | N/A-DECLARED | N/A-DECLARED | OK | N/A-DECLARED | N/A | OK | OK |
| Audit & Reports | OK (FreshnessStamp on trail + schedule-vs-actual; per-record event/ingest times) | OK (measured values carry quality + simulated flag) | OK | OK (decision-outcome + reason-reference per record) | N/A-DECLARED | N/A (no console-triggerable high-impact action; export is read-only) | FIXED (as Schedules) | OK |
| Administration | FIXED (edge-enrollment list had no freshness anchor) | N/A-DECLARED (retention values UNSET — stated) | N/A-DECLARED | OK | N/A-DECLARED | N/A | OK | OK |

Shell-level (all workspaces): **FIXED** — the §20.2 treatment legend now exists as a persistent UI element in the shell (`ValueTypeLegend` in `App.tsx`), and the shell gained the P5.12 skip link + `main` landmark.

## 2. What was built

### Shared components (additive)

| File | Content |
|---|---|
| `components/ValueType.tsx` (extended) | The **treatment record** (P5.10 step 4: the documented table of every visual treatment — value types, quality codes, freshness, lifecycle markers — so a later contributor changes a treatment by editing THIS module plus its pinned test, never by per-screen restyling) and the **`ValueTypeLegend`** UI element (P5.10 step 2: the legend rendered in the persistent shell, listing all five value types with their literal badge forms, the frozen six quality states, and the freshness rule). |
| `components/ConfirmHighImpact.tsx` (NEW) | The shared explicit-confirmation step (P5.11 step 3) with the recorded phase list `HIGH_IMPACT_ACTIONS` (see §3). Renders action + instance + summary BEFORE the control; the control is a labelled, `aria-required` native checkbox (keyboard-operable, colour-independent); the resolution renders only after confirmation; an unlisted action id renders fail-honest and offers NO control. |
| `components/honesty.tsx` (extended) | `CONSOLE_DEMO_DATA_STATEMENT` + `DemoDataBadge` (P5.12 step 3) — the demo-data flag DEFINED FOR FUTURE USE (no demo data exists; see §4). |
| `components/availability.tsx` (extended) | `AvailabilityLine` carries the availability record's own freshness (event time + age) — the P5.10 audit found the availability state displayed without a timestamp. |

### Workspace fixes (surgical, per the audit table)

- `workspaces/Dispatch.tsx` — FreshnessStamp on the command detail; rejection evidence now carries the checked-at timestamp next to model version/correlation id (P5.11 step 2 verified across Grid AND Dispatch); the approval flow uses the shared `ConfirmHighImpact`; RoleScopeLine added.
- `workspaces/Schedules.tsx` — RoleScopeLine; captions + `scope="col"` on the objective-terms and comparison tables; an explicit rejection-explanation note for non-optimal solver outcomes (the §12.6 record surface serves no failing-check/evidence field — the absence is named, the recorded candidate-status statement is shown verbatim, and the operator is pointed at the Phase 4 rejection-evidence surface).
- `workspaces/Portfolios.tsx` — freshness anchor + the registry-facts value-type statement on the composition block.
- `workspaces/Assets.tsx` — availability freshness passed through.
- `workspaces/SimulationLab.tsx` — freshness anchor on the runs section.
- `workspaces/PluginsIntegrations.tsx`, `workspaces/Administration.tsx` — freshness anchors on the edge lists.
- `App.tsx` — skip link (first focusable element), `main#workspace-main` landmark with `tabIndex={-1}`, persistent `ValueTypeLegend`.
- `workspaces/Dispatch.test.tsx` — the one pinned assertion updated (`approval-confirm` → `confirm-high-impact-checkbox`); all other existing tests untouched (their `toContain` assertions survive the additive changes).

### Tests (three new files, all UNRUN)

| File | Covers |
|---|---|
| `components/designPrinciples.test.tsx` | P5.10: the cross-workspace structural audit (every workspace module renders a freshness treatment and the RoleScopeLine; no workspace renders demo data); the functional one-value-of-each-type mapping (`valueTypeFor`); the legend renders all treatments. |
| `components/statePrinciples.test.tsx` | P5.11: the 14 lifecycle markers are unique and keyword-bearing (state readable from the display, not a tooltip); ISSUED/ACKNOWLEDGED simulated qualifications in the markers; model version + timestamp asserted on a Grid outcome row AND on the Dispatch rejection evidence; the high-impact list; the ConfirmHighImpact render contract (order, labelled required checkbox, children gated on confirmation, unknown action fail-honest). |
| `components/a11yPrinciples.test.tsx` | P5.12: shell landmarks (skip link, labelled nav, main id, focus-order positions); colour-independence (markers carry keywords + distinct symbols); labelled controls; the closed four-role vocabulary (no "auditor" role invented); RoleScopeLine structural audit; demo-badge honesty. |

## 3. The high-impact action list (P5.11 step 3 — the required record)

| Action | Console-triggerable this phase | Confirmation |
|---|---|---|
| approval | YES — the Dispatch authorization-gate flow | `ConfirmHighImpact` (explicit checkbox) → P5.26 step-up panel (typed target + re-validation) → the recorded missing-endpoint gap resolution (no approval is performed; the console never invents a POST) |
| cancellation | NO — no transition endpoint on the read-only P4.30 surface; CANCELLED is platform-recorded | N/A (list states why) |
| supersede | NO — no supersede transition endpoint; supersessions are platform-recorded (§14.2) | N/A (list states why) |
| fallback activation (operator-triggered) | NO — fallback is a policy activation (P4.17), never a console action | N/A (list states why) |

**DEFAULT:** the confirmation is a required labelled checkbox rather than a modal or typed re-entry (typed confirmation enters via the P5.26 step-up panel, which composes after this component). §20.2 and ledger P5.11 step 3 require *explicit confirmation*; the checkbox is explicit, informed (parameters render above the control), and keyboard-operable.

## 4. Demo-data honesty (P5.12 step 3)

**No demo data exists.** Every displayed value on every workspace comes from a tenant-scoped API read of platform records; fixtures live only in test files. The `DemoDataBadge` + `CONSOLE_DEMO_DATA_STATEMENT` are defined in `components/honesty.tsx` FOR FUTURE USE: any surface that ever renders injected demo data must render the badge next to every demo value. The designPrinciples test pins both halves: the badge exists and is correct, AND no workspace module references it (nothing renders demo data today). This matches the CONTEXT.md open-question-6 default position (empty states preferred; labelled demo only if ever needed).

## 5. Decisions (labelled DEFAULT where they are this pass's choice)

1. **DEFAULT — the legend lives in the persistent shell, not per workspace.** One `ValueTypeLegend` in `App.tsx` makes the treatments visible on every workspace without twelve per-file edits; a workspace can still add a local pointer, never a divergent treatment.
2. **DEFAULT — the treatment record lives in the component module header + the pinned test, not a separate design doc page.** The console's source is the documentation a contributor edits alongside the treatment; `ValueType.tsx` carries the table, and `designPrinciples.test.tsx` is the pinned enforcement (P5.10 step 4's "cannot silently change").
3. **DEFAULT — the P5.10 cross-workspace audit is structural (source-level) + functional on the shared core, not a full 12-workspace render matrix.** Rendering every container needs the effects/DOM environment the human's test run provides; the per-workspace render tests already exist (one per workspace from waves 2a–4b) and this pass's structural walk guarantees no module bypasses the shared treatments. Chosen honestly over a duplication of twelve render rigs.
4. **DEFAULT — value types for non-telemetry surfaces are N/A-DECLARED, not invented.** Registry facts, §12.6 records, audit records, and config shapes are not telemetry; forcing measured/estimated/simulated onto them would fabricate provenance. Where a surface displays such values the workspace states what kind of record it is; telemetry-typed values everywhere render the real badge.
5. **DEFAULT — no "auditor" role is invented.** The role vocabulary is the closed four from flexcore-authz policy.rs (platform-admin, tenant-admin, operator, viewer); the subtask brief's "auditor" path maps onto the audit-read actions a role holds. Role-specific visibility remains API-enforced (P5.26); the console renders the claims-derived role line everywhere and adds no console-side gating that would pretend to be enforcement.
6. **DEFAULT — skip link is an href anchor to `#workspace-main`** with `tabIndex={-1}` on the target, so it works before any JS routing interaction and without adding dependencies.

## 6. Honest limitations

- **Nothing was executed**: no install, no build, no type-check, no lint, no test run. All three test files are complete but UNRUN (headers say so).
- **TypeScript was never compiled** — type-level defects are possible despite strict-mode discipline.
- **renderToString cannot execute effects**: the checkbox `onChange` flow, the container `useEffect` loads, and real focus behaviour (tab order in a live browser, focus after clicking the skip link) are NOT exercised by these tests — the structural assertions verify presence and ordering in the rendered HTML only.
- **Real assistive-technology verification is the human's**: no screen reader ran; the a11y assertions are structural/DOM-attribute checks (landmarks, labels, roles, text-borne treatments), which are necessary but not sufficient for WCAG conformance. The §21.4 browser-matrix run and the keyboard-only approval walkthrough (P5.12 exit check) are UNRUN and remain the human's gate — no browser-matrix result is recorded here because none was produced.
- **The P5.10 acceptance criterion "loads each workspace with a fixture containing one value of each type and verifies each renders with the correct treatment" is satisfied in two layers** (structural walk + per-workspace render tests that already exist), not as a single all-twelve render harness — recorded as a deliberate scope choice (§5.3), not a completed matrix run.
- **Concurrent wave 5b landed mid-pass** on App.tsx / honesty.tsx / Dispatch.tsx; the merge was reconciled (one unused import removed) but the combined result has never been compiled — the human's build is the first integration check of BOTH passes.
- Freshness for list surfaces without served per-record timestamps remains the console-read timestamp (the §20.2-allowed form); per-record event/ingest times render wherever the records carry them.
