# Ledger — Phase 5: Operator Console and Simulator Integration

- **Report:** `docs/plan/Neryva_FlexCore_Product_and_Architecture_Report_v1.1.md`
- **Stage:** §23 Stage 5 — Operator console and simulator integration
- **Release target:** v0.3.0 console surface within the 0.x simulation line (see §2.1, §21.4); console is the operator's view of simulation/shadow work, not a field-control tool in this phase.
- **Status:** Not started. No code in this phase until every subtask's acceptance criteria are met and recorded.
- **Non-negotiables (AGENTS.md):** no hallucinated facts; no safety/certification claims; no protection-grade claims; never conflate solver classes; simulation-first 0.x — the console must never display a simulated/proposed value as if it were executed or measured; verify before claiming done.
- **Spelling:** "license" (US), "acknowledgement" (per `.agent/WRITING.md`).
- **How to use this ledger:** work subtasks in ID order unless a dependency says otherwise. Check a box only when its acceptance criteria have evidence attached. A checked box without evidence is a defect.

## Trace summary

| Report section | Covered by |
|---|---|
| §20.1 workspaces 1–4 (Overview, Portfolios, Sites, Assets) | P5.02–P5.04 |
| §20.1 workspaces 5–7 (Grid, Schedules, Dispatch) | P5.04–P5.06 |
| §20.1 workspaces 8–12 (Market, Simulation Lab, Plugins, Audit, Admin) | P5.07–P5.09 |
| §20.2 design principles | P5.10–P5.13 |
| §19.1 simulator adapters + §12.4 HELICS co-simulation | P5.14–P5.17 |
| §19.2 scenario replay + §24 step 10 reproducibility | P5.18–P5.20 |
| §19.2 layer 6 failure injection (§23 Stage 5) | P5.21–P5.22 |
| §19.3–§19.4 benchmarks + metrics | P5.23–P5.24 |
| §15.1 live UI (SSE/WebSocket), §21.3 observability | P5.25 |
| §17 security, §16.2 tenancy, §21.4 control authority | P5.26–P5.27 |
| §7.5 frontend stack, §22.2 CI gates | P5.28 |
| Docs + handover (Stage 6 readiness) | P5.29–P5.30 |
| §20.1 item 1 (alarm contract — gap ownership) | P5.31 |
| §16.2 tenant-context display | P5.32 |
| §15.4 live-update ordering/dedup | P5.33 |

## Subtasks

### P5.01 — Console scaffold and frontend stack (§7.5, §21.4)

- [ ] **Objective:** Scaffold the operator console on the report's frontend stack, as a static bundle served by the control plane, with no native mobile or desktop client.
- **Report trace:** §7.5, §21.4 (build/distribution form), §8.1 (`web/console/`).
- **Inputs/prerequisites:** Phase 1–4 APIs (REST/OpenAPI); RESEARCH.md frontend versions.
- **Steps:**
  1. Scaffold with Vite + React + TypeScript using the official `react-ts` template path (§7.5); add TanStack Router and Query (or the recorded equivalent) for route/data layers.
  2. Pin versions in the lockfile (see RESEARCH.md for the versions verified as of 2026-11-07; re-verify at scaffold time and record any change in RESEARCH.md — do not silently upgrade).
  3. Configure the build to emit a static bundle served by the control plane or a CDN (§21.4); support the browser matrix in §21.4 (current and previous major versions of Chrome, Edge, Firefox, Safari).
  4. Generate the REST client types from the published OpenAPI (§7.5); add a test proving generated types match the live OpenAPI (contract drift fails the build).
  5. Verify the scaffold builds, lints, type-checks, and tests clean per §22.2 (frontend lint/type-check/test/build).
- **Deliverable/output:** Console scaffold + pinned lockfile + generated API client + CI wiring.
- **Acceptance criteria / exit check:** Scaffold builds/lints/type-checks/tests clean; generated client matches OpenAPI; browser matrix is recorded as the test target; no mobile/desktop client code exists.
- **Owner role:** Frontend engineer.
- **Dependencies:** Phase 1–4 APIs (consumed as contracts).
- **Verification method:** `npm run build/lint/typecheck/test` output (or the repo's recorded equivalents); OpenAPI drift test.
- **Risks/failure modes:** Generated client drifting from the API. Mitigation: drift test in CI.

### P5.02 — Overview workspace (§20.1 item 1)

- [ ] **Objective:** Build the Overview workspace: fleet availability, current aggregate power, active schedules, key alarms, delivery status.
- **Report trace:** §20.1 item 1, §20.2.
- **Inputs/prerequisites:** P5.01; Phase 2–4 APIs (telemetry, schedules, dispatch, alarms as defined by prior phases — record any missing alarm contract as a gap, do not invent one).
- **Steps:**
  1. Display fleet availability, current aggregate power (MW, `P > 0` export, with the sign convention labelled), active schedules (ID + revision), key alarms, and delivery status — each with its data-freshness timestamp (§20.2).
  2. Distinguish measured, estimated, stale, and simulated values visually (§20.2) — use distinct, documented visual treatments (not colour alone; see P5.12 accessibility).
  3. Show the control-authority mode (Simulation / read-only/shadow) prominently for the deployment (§21.4); in this phase the mode is Simulation or read-only/shadow only.
  4. Test with the §24 vertical-slice scenario data: the overview reflects the simulated portfolio state correctly and labels simulated values as simulated.
- **Deliverable/output:** Overview workspace + freshness/value-type treatments + tests.
- **Acceptance criteria / exit check:** All five §20.1 item 1 elements present with freshness shown; value types visually distinct; mode shown; no simulated value presented as measured.
- **Owner role:** Frontend engineer.
- **Dependencies:** P5.01.
- **Verification method:** UI tests with fixture data; visual review of value-type treatments.
- **Risks/failure modes:** A simulated aggregate mistaken for a measured one. Mitigation: the simulated label is part of the component, not an optional prop.

### P5.03 — Portfolios, Sites, and Assets workspaces (§20.1 items 2–4)

- [ ] **Objective:** Build the Portfolios, Sites, and Assets workspaces on the Phase 2 registry contracts.
- **Report trace:** §20.1 items 2–4, §20.2, §10.4.
- **Inputs/prerequisites:** P5.01; Phase 2 registry/telemetry APIs.
- **Steps:**
  1. Portfolios: composition, aggregation rules, commitments, flexibility (envelope summary from Phase 3, P3.12 — with envelope version and computation time shown).
  2. Sites: site overview, PCC, grid model (version + hash, P4.03), edge connectivity, local EMS status. Edge connectivity in this phase shows the simulator/edge-lab state honestly — do not display a real edge agent as connected when none exists.
  3. Assets: asset state, capability, telemetry quality (GOOD/SUSPECT/BAD/MISSING/ESTIMATED/STALE per value), alarms, limits, maintenance/outage status.
  4. Enforce role-specific visibility (§20.2): a user sees only the tenants/sites/assets their role permits (P5.26); test with two roles.
- **Deliverable/output:** Three workspaces + role-visibility tests.
- **Acceptance criteria / exit check:** All listed elements present per workspace; telemetry quality shown per value; grid-model version shown on Sites; role-visibility tests pass; no invented edge/EMS state.
- **Owner role:** Frontend engineer.
- **Dependencies:** P5.01.
- **Verification method:** UI/role tests; fixture-driven review.
- **Risks/failure modes:** Displaying a placeholder edge connection as live. Mitigation: connectivity state comes from the API only, with an explicit "no data" state.

### P5.04 — Grid workspace (§20.1 item 5)

- [ ] **Objective:** Build the Grid workspace: topology/model version, feeder/substation boundaries, voltage/thermal validation, model health.
- **Report trace:** §20.1 item 5, §12.5, §20.2, §19.4 (grid feasibility metrics).
- **Inputs/prerequisites:** P5.01; Phase 4 feasibility results and model health (P4.03, P4.06–P4.07).
- **Steps:**
  1. Display topology/model version (version + hash), feeder/substation boundaries, voltage/thermal validation outcomes (per-limit pass/fail + margin, from P4.06), and model health (completeness, staleness, coverage).
  2. Show the model version and timestamp for every grid feasibility outcome (§20.2).
  3. Show coverage limitations explicitly: network parts the model does not cover are labelled uncovered, never shown as passing.
  4. Test with a feasibility result that has a binding transformer limit and one with a coverage limitation; both display correctly.
- **Deliverable/output:** Grid workspace + tests.
- **Acceptance criteria / exit check:** Model version + timestamp on every outcome; margins shown per limit; uncovered parts labelled uncovered; both fixture cases display correctly.
- **Owner role:** Frontend engineer + power-systems engineer.
- **Dependencies:** P5.01.
- **Verification method:** Fixture-driven UI tests.
- **Risks/failure modes:** A coverage gap displayed as a green pass. Mitigation: uncovered is a distinct state in the component model.

### P5.05 — Schedules workspace (§20.1 item 6)

- [ ] **Objective:** Build the Schedules workspace: day-ahead/intraday planning, objective selection, schedule alternatives, scenario comparison.
- **Report trace:** §20.1 item 6, §12.1, §12.6, §23 Stage 3 (comparison workflow, P3.23).
- **Inputs/prerequisites:** P5.01; Phase 3 schedule/comparison APIs (P3.21–P3.23).
- **Steps:**
  1. Display schedules with ID + revision, horizon, objective configuration (terms/weights, P3.14), input snapshot references, solver + version, §12.6 status, and objective value with units.
  2. Implement objective selection as choosing among declared, configured objectives — never free-text objective entry that bypasses the declared set.
  3. Implement schedule alternatives and scenario comparison from the P3.23 comparison record: shared input snapshot, per-candidate §19.4 metrics (objective value, schedule deviation, feasible interval count, runtime). Comparisons across different input snapshots are labelled incomparable (P3.23).
  4. Show superseded revisions as SUPERSEDED, retained and retrievable — never hidden or deleted.
- **Deliverable/output:** Schedules workspace + comparison view + tests.
- **Acceptance criteria / exit check:** All schedule metadata present; objective selection is from the declared set; comparisons show per-candidate metrics and enforce the shared-input rule; superseded revisions visible.
- **Owner role:** Frontend engineer.
- **Dependencies:** P5.01.
- **Verification method:** UI tests with Phase 3 fixture data; comparison-rule tests.
- **Risks/failure modes:** Two schedules on different inputs compared as if equivalent. Mitigation: the incomparable label is enforced by the component.

### P5.06 — Dispatch workspace (§20.1 item 7)

- [ ] **Objective:** Build the Dispatch workspace: proposed commands, validation results, approvals, issue/acknowledgement/tracking lifecycle.
- **Report trace:** §20.1 item 7, §14.1–§14.2, §20.2.
- **Inputs/prerequisites:** P5.01; Phase 4 command APIs (P4.30).
- **Steps:**
  1. Display proposed commands with their state, validation results, feasibility results, and approvals — each state visually distinct (§20.2): candidate, feasible, approved, issued, acknowledged, and measured states must not look alike.
  2. Never display a proposed setpoint as if it were an executed command (§20.2). Proposed values carry a "proposed" treatment until the command reaches ACKNOWLEDGED, and measured values are shown only from telemetry.
  3. Explain each rejected command with the failing constraint and supporting evidence (§20.2) — the rejection view links to the feasibility/validation evidence references (P4.07, P4.11).
  4. Implement the approval action with explicit confirmation for high-impact actions (§20.2) and step-up approval where required (§17.3); the approval records actor, scope, limits, and expiry (P4.13).
  5. Keep emergency actions and normal scheduling visually separate (§20.2) — in this phase, emergency actions are display-only placeholders only if the report's emergency-action concept is defined elsewhere; do not invent an emergency dispatch action. (If no report definition exists, record this as an open item, not a feature.)
- **Deliverable/output:** Dispatch workspace + approval flow + rejection explanations + tests.
- **Acceptance criteria / exit check:** States visually distinct; proposed never looks executed; rejections show failing constraint + evidence link; approval requires explicit confirmation and records scope/limits/expiry; no invented emergency action.
- **Owner role:** Frontend engineer.
- **Dependencies:** P5.01.
- **Verification method:** UI tests per state; approval-flow tests; rejection-view tests.
- **Risks/failure modes:** A proposed setpoint read as an executed one. Mitigation: distinct visual treatments are tested, not just styled.

### P5.07 — Market & Programs, Plugins & Integrations, Administration workspaces (§20.1 items 8, 10, 12)

- [ ] **Objective:** Build the three supporting workspaces honestly: show what exists, and show what does not exist yet instead of inventing it.
- **Report trace:** §20.1 items 8, 10, 12; §25.2 (D1, D5); §5 (plugins — Stage 7).
- **Inputs/prerequisites:** P5.01; Phase 1–4 APIs as they exist.
- **Steps:**
  1. Market & Programs: market region, program signals, commitments, adapter status, delivery evidence — with synthetic commitments labelled synthetic (D1 OPEN; P3.16). Where no market adapter exists, show "no market adapter configured" rather than fabricated signals.
  2. Plugins & Integrations: installed plugins, capabilities, permissions, versions, health, configuration — showing only actually installed/registered plugins. No plugin may be shown as able to bypass authorization, tenant isolation, audit, command lifecycle, or final dispatch validation (§5.7).
  3. Administration: identity integration, roles, tenants, policies, data retention, edge enrollment — with retention shown as the configured per-class values (§16.3), and edge enrollment showing the simulation/lab state honestly (no real edge agent is enrolled in this phase).
  4. Each workspace has an explicit empty state ("not configured") with no fabricated data.
- **Deliverable/output:** Three workspaces + empty states + tests.
- **Acceptance criteria / exit check:** No fabricated market signals, plugins, or edge enrollments; synthetic labels present; empty states exist and are tested; plugin capabilities shown match the registered manifest.
- **Owner role:** Frontend engineer.
- **Dependencies:** P5.01.
- **Verification method:** UI tests incl. empty states; manifest comparison.
- **Risks/failure modes:** Demo data mistaken for live integrations. Mitigation: empty states are the default; any demo data is labelled demo (see also P5.12).

### P5.08 — Simulation Lab workspace (§20.1 item 9)

- [ ] **Objective:** Build the Simulation Lab workspace: scenarios, replay, benchmark runs, result comparison.
- **Report trace:** §20.1 item 9, §19.1–§19.4, §24 step 10.
- **Inputs/prerequisites:** P5.01; P5.14–P5.20 (adapters, replay, benchmarks).
- **Steps:**
  1. List scenarios with their pinned inputs, versions, and seeds; show run history with status and §19.4 metrics.
  2. Provide replay controls that re-run a scenario from its recorded inputs (P5.18–P5.20) and show the reproducibility verdict (identical within stated tolerance, or not).
  3. Provide benchmark-run comparison: same metrics, same conditions, side by side — never across different conditions without a label.
  4. Label every Simulation Lab result as simulated, with the scenario and version it came from.
- **Deliverable/output:** Simulation Lab workspace + tests.
- **Acceptance criteria / exit check:** Scenarios list pinned inputs/versions/seeds; replay shows a reproducibility verdict; benchmark comparisons are condition-labelled; all results labelled simulated.
- **Owner role:** Frontend engineer + numerical engineer.
- **Dependencies:** P5.01, P5.14–P5.20.
- **Verification method:** UI tests with recorded runs; replay-verdict tests.
- **Risks/failure modes:** A replay that silently uses new inputs. Mitigation: replay inputs are the recorded inputs; any deviation is flagged (P5.19).

### P5.09 — Audit & Reports workspace (§20.1 item 11)

- [ ] **Objective:** Build the Audit & Reports workspace: decision evidence, operator actions, schedule vs actual, dispatch failures.
- **Report trace:** §20.1 item 11, §15.3, §21.3, §24 step 9.
- **Inputs/prerequisites:** P5.01; Phase 4 audit data (P4.30).
- **Steps:**
  1. Display the complete decision/audit trail (§24 step 9): per command, the chain schedule → validation evidence → feasibility evidence → authorization decision → issue → acknowledgement → tracking → outcome, with timestamps, actors, and evidence references.
  2. Provide schedule-vs-actual views: requested vs measured response, with tracking error and the acceptance criterion (P4.16); measured values only from telemetry, never from the command itself.
  3. Provide dispatch-failure reporting: failures, expiries, and fallback activations with reasons and evidence links (P4.17, P4.30).
  4. Provide operator-action records (`OperatorActionRecorded`, §15.3) with actor and timestamp; support export of the audit trail for review (format recorded; content is the same evidence, not a summary that drops references).
- **Deliverable/output:** Audit & Reports workspace + export + tests.
- **Acceptance criteria / exit check:** Full decision chain viewable per command; schedule-vs-actual uses telemetry-measured values; failures/fallbacks listed with reasons; export preserves evidence references.
- **Owner role:** Frontend engineer.
- **Dependencies:** P5.01.
- **Verification method:** UI tests with a full lifecycle fixture; export-content checks.
- **Risks/failure modes:** Audit export dropping evidence references. Mitigation: export is tested for reference preservation.

### P5.10 — Design principles: freshness and value types (§20.2)

- [ ] **Objective:** Apply the §20.2 freshness and value-type principles across every workspace, not just Overview.
- **Report trace:** §20.2, §10.4.
- **Inputs/prerequisites:** P5.02–P5.09.
- **Steps:**
  1. Show operational state and data freshness prominently on every workspace (last-update timestamp + age, and STALE marking where quality is STALE).
  2. Distinguish measured, estimated, stale, and simulated values visually everywhere, with a documented legend; the legend itself is a UI element, not tribal knowledge.
  3. Add a cross-workspace audit: a checklist run that loads each workspace with a fixture containing one value of each type and verifies each renders with the correct treatment.
  4. Record the visual treatments in the console's design documentation so a later contributor cannot silently change a treatment.
- **Deliverable/output:** Freshness/value-type treatments applied workspace-wide + legend + cross-workspace audit + design note.
- **Acceptance criteria / exit check:** Freshness shown on every workspace; four value types distinct everywhere; legend present; cross-workspace audit passes.
- **Owner role:** Frontend engineer.
- **Dependencies:** P5.02–P5.09.
- **Verification method:** Cross-workspace audit output; design-note review.
- **Risks/failure modes:** One workspace forgetting the treatments. Mitigation: the audit covers every workspace.

### P5.11 — Design principles: state distinction and rejection explanations (§20.2)

- [ ] **Objective:** Enforce the remaining §20.2 state and explanation principles: distinct lifecycle states, model version/timestamp on feasibility outcomes, explicit confirmation for high-impact actions.
- **Report trace:** §20.2, §14.1, §12.5.
- **Inputs/prerequisites:** P5.04–P5.06.
- **Steps:**
  1. Keep candidate, feasible, approved, issued, acknowledged, and measured states visually distinct across Schedules, Grid, and Dispatch (shared state-treatment tokens, not per-screen improvisation).
  2. Show model version and timestamp for every grid feasibility outcome (§20.2; P5.04).
  3. Require explicit confirmation for high-impact actions (§20.2) — define the high-impact action list for this phase (approval, cancellation, supersede, fallback activation where operator-triggered) and implement the confirmation step for each; record the list.
  4. Test each principle with fixtures: a reviewer must be able to tell the state from the display alone, without reading a tooltip.
- **Deliverable/output:** Shared state treatments + confirmation flows + high-impact list + tests.
- **Acceptance criteria / exit check:** States distinguishable without tooltips in review; model version/timestamp on all feasibility outcomes; confirmation required for every listed high-impact action.
- **Owner role:** Frontend engineer.
- **Dependencies:** P5.04–P5.06.
- **Verification method:** Reviewer walkthrough with fixtures; confirmation-flow tests.
- **Risks/failure modes:** State shown by colour alone (inaccessible). Mitigation: treatments combine label + icon/shape + colour (see P5.12).

### P5.12 — Accessibility, role-specific screens, and demo-data honesty (§20.2, §21.4)

- [ ] **Objective:** Make the console usable by role and by assistive technology, and keep any demo data visibly demo.
- **Report trace:** §20.2, §21.4 (browser matrix), §17.3 (RBAC).
- **Inputs/prerequisites:** P5.02–P5.11.
- **Steps:**
  1. Provide role-specific screens and reduce irrelevant controls (§20.2): each role sees only its permitted workspaces/actions (P5.26); test with at least two roles.
  2. Meet the accessibility baseline for the browser matrix: states and value types are not conveyed by colour alone; interactive elements are keyboard-reachable and labelled; the console is tested with keyboard-only navigation on at least one workflow (e.g. approve a command).
  3. Any demo/synthetic data in the console is labelled demo/synthetic wherever it appears (portfolio figures, commitments, scenario data); no demo value is presented as a live measurement.
  4. Run the browser-matrix check (§21.4) on the console build and record the result per browser.
- **Deliverable/output:** Role-specific views + accessibility checks + demo labelling + browser-matrix record.
- **Acceptance criteria / exit check:** Role tests pass; keyboard-only approval workflow completes; no state/value conveyed by colour alone; demo labels present; browser-matrix results recorded.
- **Owner role:** Frontend engineer.
- **Dependencies:** P5.02–P5.11.
- **Verification method:** Role tests; keyboard walkthrough; browser-matrix run.
- **Risks/failure modes:** Accessibility treated as a post-hoc pass. Mitigation: the keyboard workflow test is part of the exit checklist (CHECKLIST.md).

### P5.13 — Simulation honesty in the console (§20.2, §21.4, §14.1)

- [ ] **Objective:** Guarantee the console never presents simulation as field reality: every simulated issue, acknowledgement, and tracking result is labelled simulated, and acknowledgement is never shown as physical delivery.
- **Report trace:** §20.2, §21.4, §14.1, §14.2.
- **Inputs/prerequisites:** P5.06, P5.09; Phase 4 handover (P4.32 simulation-only boundary).
- **Steps:**
  1. Add a persistent console-level indicator of control-authority mode (Simulation / read-only/shadow) on every workspace (§21.4).
  2. In Dispatch and Audit views, label simulator-issued commands as "issued to simulator" (not "issued to device") and simulator acknowledgements as application-layer acceptance (§14.1).
  3. Add a console text review: search all display strings for "dispatched to", "executed", "delivered", "applied to device" and verify each is either correctly qualified (simulator) or removed.
  4. Test with the full §24 lifecycle fixture: at every state, the display states the correct (simulated) meaning.
- **Deliverable/output:** Mode indicator + qualified display strings + text-review record + tests.
- **Acceptance criteria / exit check:** Mode indicator on every workspace; no unqualified "executed/delivered/applied" string remains; lifecycle-fixture display review passes.
- **Owner role:** Frontend engineer + architect.
- **Dependencies:** P5.06, P5.09.
- **Verification method:** String review; lifecycle-fixture display tests.
- **Risks/failure modes:** A single unqualified string in an error toast. Mitigation: the string review covers error/empty states too.

### P5.14 — Simulator adapters: asset simulator integration (§19.1, §8.1)

- [ ] **Objective:** Integrate the simulator adapters so the console and the control plane run the §24 vertical slice against simulated assets, not stubs.
- **Report trace:** §19.1 (simulation strategy), §8.1 (`plugins/builtin/asset-simulator`, `apps/sim-runner`), §24 steps 1–2.
- **Inputs/prerequisites:** Phase 2 asset simulator; P5.01.
- **Steps:**
  1. Wire the asset simulator (BESS, PV, flexible load, EVSE — §24 step 1) as the telemetry and acknowledgement source for the Phase 5 scenario runs, through the same adapter interfaces the control plane uses (not a console-side mock).
  2. Generate timestamped telemetry with quality flags (§24 step 2) through the simulator; verify the console displays the quality flags it carries (P5.03).
  3. Record the simulator version and the asset-model versions used per run (P5.19 reproducibility).
  4. Test: a simulator asset dropout mid-run is visible in the console (availability change) and triggers the Phase 3/4 re-schedule/reject paths (P3.24, P4.08, P4.21).
- **Deliverable/output:** Simulator integration + version recording + dropout test.
- **Acceptance criteria / exit check:** Console runs against the real asset simulator through platform interfaces; quality flags displayed; dropout visible and handled by the platform paths; versions recorded per run.
- **Owner role:** Rust/Python engineer.
- **Dependencies:** Phase 2 simulator.
- **Verification method:** Integration tests; dropout scenario.
- **Risks/failure modes:** Console mock diverging from the real simulator. Mitigation: no console-side simulator mock for scenario runs; fixtures are platform-level.

### P5.15 — Grid simulator adapter integration (§19.1, §12.4)

- [ ] **Objective:** Integrate the grid simulator adapter (pandapower, per Phase 4) into the scenario runs so feasibility results in the console come from the real adapter.
- **Report trace:** §19.1, §12.4, §12.5, §20.1 item 5.
- **Inputs/prerequisites:** P4.02 adapter; P5.14.
- **Steps:**
  1. Wire the pandapower adapter into the scenario-runner path so a scenario's feasibility checks execute through the Phase 4 pipeline (P4.04–P4.07), not a reimplementation.
  2. Display the adapter/provider version and model version on Grid workspace outcomes (P5.04).
  3. Test with the IEEE 13/34/123-bus cases where licensing and model availability are confirmed (§19.3) — or record explicitly that no IEEE feeder was available and the synthetic feeder was used instead. Do not claim an IEEE case was run if it was not.
  4. Record the adapter version per run (P5.19).
- **Deliverable/output:** Grid-adapter scenario integration + version display + feeder-case record.
- **Acceptance criteria / exit check:** Feasibility in scenarios runs through the Phase 4 pipeline; versions displayed; feeder cases used (or their absence) recorded honestly.
- **Owner role:** Numerical engineer.
- **Dependencies:** P4.02, P5.14.
- **Verification method:** Integration tests; fixture review.
- **Risks/failure modes:** A console-side power-flow reimplementation. Mitigation: the scenario path calls the platform pipeline; a code review checks for duplicate logic.

### P5.16 — HELICS co-simulation integration (§12.4, §19.1)

- [ ] **Objective:** Integrate HELICS as a co-simulation orchestrator for a multi-federate scenario, with HELICS coordinating — not replacing — the physics models.
- **Report trace:** §12.4 (HELICS row), §19.1 (multi-domain co-simulation), §7.4.
- **Inputs/prerequisites:** P5.14–P5.15; RESEARCH.md HELICS version (3.7.0).
- **Steps:**
  1. Define one HELICS federation for a Phase 5 scenario: at minimum a grid federate (pandapower adapter), an asset/end-use federate (asset simulator), and a FlexCore/control federate exchanging setpoints/measurements. Record the federation configuration (federates, publications/subscriptions, time step, broker) as versioned scenario data.
  2. Implement the FlexCore HELICS federate (Python `pyhelics`/helics binding per the pinned version, or the documented equivalent) behind the simulator-adapter interface — HELICS is the time coordinator, not the power-flow solver.
  3. Run the federation end-to-end and verify time synchronisation: federates advance in step, messages arrive at the intended simulation times, and the run terminates cleanly.
  4. State in docs and in the Simulation Lab display: HELICS coordinates federates; it does not replace their physics models (§19.1). Do not present HELICS results as a power-system validation by itself.
  5. Record the HELICS version and federation config per run (P5.19).
- **Deliverable/output:** HELICS federation definition + FlexCore federate + end-to-end run + docs note.
- **Acceptance criteria / exit check:** Three-federate run completes with time synchronisation verified; setpoints/measurements exchanged at intended times; HELICS role stated correctly in docs and UI; version/config recorded per run.
- **Owner role:** Numerical engineer.
- **Dependencies:** P5.14–P5.15.
- **Verification method:** Federation run output; time-sync assertions; config review.
- **Risks/failure modes:** HELICS presented as the physics model. Mitigation: the role statement is in the run record and the UI.
- **Open verification:** the Python `pyhelics` binding version matching HELICS 3.7.0 must be confirmed at implementation time (RESEARCH.md open task 3).

### P5.17 — Additional simulator adapters: scoping decision (§19.1, §12.4)

- [ ] **Objective:** Decide explicitly, per simulator track in §19.1, which adapters are integrated in Phase 5 and which are deferred — and record the decision.
- **Report trace:** §19.1, §12.4, §23 Stage 5.
- **Inputs/prerequisites:** P5.14–P5.16.
- **Steps:**
  1. For each §19.1 track (portfolio scheduling, distribution quasi-static, transmission/planning, dynamic stability, multi-domain co-simulation, HIL), record: integrated in Phase 5, or deferred with a reason and a trigger.
  2. Default positions (to confirm at implementation): portfolio scheduling and distribution quasi-static are integrated (P5.14–P5.15); multi-domain co-simulation is integrated via HELICS (P5.16); transmission/planning (PyPSA) is a documented optional comparison only if a named case needs it; dynamic stability (ANDES) and HIL are deferred — HIL requires Stage 9 qualification and is never implied by a simulation result.
  3. Do not use a steady-state power-flow tool as evidence for transient frequency stability (§19.1) — record this as a constraint on the Simulation Lab's claims.
  4. File the scoping table in PLAN.md.
- **Deliverable/output:** Simulator-track scoping table.
- **Acceptance criteria / exit check:** Every §19.1 track has a recorded integrated/deferred decision with a reason; ANDES/HIL deferral is explicit; no steady-state result is presented as stability evidence.
- **Owner role:** Architect + power-systems engineer.
- **Dependencies:** P5.14–P5.16.
- **Verification method:** Scoping-table review.
- **Risks/failure modes:** Scope creep into HIL. Mitigation: HIL deferral is explicit and tied to Stage 9.

### P5.18 — Scenario definition and replay (§19.2 layer 5, §24 step 10)

- [ ] **Objective:** Define scenarios as pinned, replayable records and implement replay that re-executes the same scenario from the same inputs.
- **Report trace:** §19.2 layer 5, §24 step 10, §19.3.
- **Inputs/prerequisites:** P5.14–P5.16; P3.26 scenarios.
- **Steps:**
  1. Define the scenario record: scenario ID + version, input snapshot (telemetry, forecasts, capability versions, envelope/schedule inputs), solver/provider/adapter versions, simulator versions, federation config (if HELICS), random seeds, clock configuration, and expected outcomes/tolerances (pre-registered, P3.26 discipline).
  2. Implement replay (`apps/sim-runner`, §8.1) that loads a scenario record and re-executes it without taking new inputs; any input that cannot be reproduced from the record is an error, not a silent default.
  3. Cover the §24 vertical slice as the first replayable scenario (steps 1–8, with steps 9–10 being this phase's console display and replay verification).
  4. Store scenarios under `scenarios/` (§8.1) with a README stating they are synthetic unless a named real feeder/model is used (with its license/availability confirmed, §19.3).
- **Deliverable/output:** Scenario record schema + replay runner + §24 slice scenario + tests.
- **Acceptance criteria / exit check:** Scenario record contains all listed fields; replay uses only recorded inputs; missing-input replay errors loudly; §24 slice is replayable.
- **Owner role:** Rust/Python engineer.
- **Dependencies:** P5.14–P5.16.
- **Verification method:** Replay tests with tampered/missing inputs; record inspection.
- **Risks/failure modes:** Replay silently substituting current data for recorded data. Mitigation: the error-on-missing-input rule is tested.

### P5.19 — Replay reproducibility verification (§24 step 10, §19.2 layer 5)

- [ ] **Objective:** Prove replay reproducibility: the same scenario, same versions, same seeds produce the same results within a stated tolerance.
- **Report trace:** §24 step 10, §19.2 layer 5, §22.3 (simulator benchmark results are reproducible).
- **Inputs/prerequisites:** P5.18.
- **Steps:**
  1. Run the §24 slice scenario twice from a clean state with identical recorded inputs and assert: schedules, feasibility outcomes, command states, and audit trails match within the stated tolerance (tolerance stated per output type before the runs).
  2. Run a negative control: change one input (e.g. a seed, a solver version, a telemetry value) and verify the replay either produces a different result (and reports the difference) or errors — never silently reproduces the old result.
  3. Record both runs (and the negative control) in the Simulation Lab (P5.08) with the reproducibility verdict.
  4. Add the reproducibility check to CI as part of the scenario regression suite (§22.2).
- **Deliverable/output:** Reproducibility verification report + negative control + CI wiring.
- **Acceptance criteria / exit check:** Two clean runs match within pre-stated tolerances; negative control behaves as specified; verdicts visible in Simulation Lab; CI runs the check.
- **Owner role:** QA + numerical engineer.
- **Dependencies:** P5.18.
- **Verification method:** Run outputs; tolerance comparison; CI log.
- **Risks/failure modes:** Tolerance fitted after the runs. Mitigation: tolerances are pre-registered in the scenario record.

### P5.20 — Scenario library: benchmark cases (§19.3)

- [ ] **Objective:** Build the Phase 5 scenario library from the §19.3 benchmark cases, starting small and adding scale gradually.
- **Report trace:** §19.3, §19.4.
- **Inputs/prerequisites:** P5.18–P5.19.
- **Steps:**
  1. Include: synthetic portfolios with BESS, PV, flexible load, EVSE; multiple feeders/sites through a common substation constraint; stressed scenarios (PV ramps, forecast error, communication loss, asset dropout, reverse power flow, topology change).
  2. Include IEEE 13/34/123-bus distribution feeders and IEEE 39/118-bus transmission systems only where licensing and model availability are confirmed (§19.3) — record per case whether it is included, and if not, why.
  3. For each scenario, record its purpose, inputs, expected outcomes, and tolerances; store under `scenarios/` with the §19.3 naming.
  4. State the scale limit in the library README: the first feeder tests do not establish capability for a 10,000-bus utility network (§19.3).
- **Deliverable/output:** Scenario library + per-case licensing/availability record + README.
- **Acceptance criteria / exit check:** Synthetic + stressed scenarios present; each IEEE case is included or explicitly excluded with a reason; scale-limit statement present; every scenario has pre-registered expectations.
- **Owner role:** Power-systems engineer + QA.
- **Dependencies:** P5.18–P5.19.
- **Verification method:** Library review; per-case record check.
- **Risks/failure modes:** An IEEE case claimed without a license check. Mitigation: per-case record is mandatory.

### P5.21 — Failure injection in the simulator (§19.2 layer 6, §23 Stage 5)

- [ ] **Objective:** Implement failure injection for simulator runs: dropped/reordered/duplicated messages, clock skew, stale telemetry, unavailable assets, solver timeout, plugin crash, partial fleet response.
- **Report trace:** §19.2 layer 6, §23 Stage 5, §17.4.
- **Inputs/prerequisites:** P5.14–P5.16, P4.31 (platform fault-injection suites — reuse, do not duplicate).
- **Steps:**
  1. Implement injection controls in the scenario runner (per-scenario injection spec: fault type, target, timing, duration) for each §19.2 layer 6 fault.
  2. For each fault, pre-register the expected platform outcome (reject/hold/fallback/failed, per Phase 4) and the expected console display (state shown, freshness shown, no silent pass).
  3. Run the injection suite against the §24 slice scenario and record outcomes; verify the console shows the fault's effect (e.g. STALE telemetry flagged, partial fleet failure shown per asset).
  4. Cross-reference, do not duplicate, the Phase 4 fault-injection suites (P4.31): this subtask adds the simulator-runner injection controls and the console display checks; the platform behaviour tests stay in Phase 4.
- **Deliverable/output:** Injection controls + injection suite + console-display checks.
- **Acceptance criteria / exit check:** Every listed fault is injectable with a pre-registered expected outcome; outcomes match; console displays each fault's effect; no duplication of P4.31 platform tests.
- **Owner role:** QA + Rust/Python engineer.
- **Dependencies:** P5.14–P5.16, P4.31.
- **Verification method:** Injection-suite output; console-display checks.
- **Risks/failure modes:** Injection that only affects the console mock, not the platform. Mitigation: injection happens in the scenario runner/platform path.

### P5.22 — Failure-injection evidence and operator visibility (§19.2 layer 6, §20.2)

- [ ] **Objective:** Make injected failures visible to the operator with their cause and the platform's response, so a failure is never a silent blank screen.
- **Report trace:** §19.2 layer 6, §20.2, §21.3.
- **Inputs/prerequisites:** P5.21.
- **Steps:**
  1. In the console, show active injections (in Simulation Lab and on affected workspaces) with fault type, target, and timing — an operator can always tell a run is fault-injected.
  2. Show the platform's response per fault (rejected/held/failed/fallback) with its evidence link (P5.09 audit trail).
  3. Provide a "clear injections and re-run clean" action in Simulation Lab that replays the same scenario without injections (P5.18) for comparison.
  4. Test: an operator reviewing a fault-injected run can state the fault, the affected assets, and the platform's response from the display alone.
- **Deliverable/output:** Injection visibility + clear-and-rerun action + tests.
- **Acceptance criteria / exit check:** Injections visible on affected workspaces; responses shown with evidence links; clean re-run works; reviewer test passes.
- **Owner role:** Frontend engineer + QA.
- **Dependencies:** P5.21.
- **Verification method:** Reviewer walkthrough with a fault-injected run.
- **Risks/failure modes:** Fault-injected results presented as clean results. Mitigation: the injection indicator is part of the run record and the display.

### P5.23 — Benchmark runs and evaluation metrics (§19.3–§19.4)

- [ ] **Objective:** Run the Phase 5 benchmarks and report the §19.4 metrics honestly, with no arbitrary universal targets.
- **Report trace:** §19.3–§19.4, §22.2.
- **Inputs/prerequisites:** P5.18–P5.20.
- **Steps:**
  1. For each scenario-library case, record the §19.4 metrics for the areas it exercises: scheduling (objective value, schedule deviation, feasible interval count, runtime); asset model (SOC/power violations, ramp violations, availability prediction error); grid feasibility (voltage violations, branch/transformer overload, AC power-flow convergence, residuals); flexibility (available vs delivered MW/MWh, uncertainty calibration, shortfall); dispatch (acknowledgement latency, tracking error, failure/expiry rate, recovery time); platform (availability, throughput, p95/p99 API/ingest latency, recovery, tenant isolation).
  2. Record hardware/container conditions per run; do not compare metrics across different conditions without a label.
  3. Set no arbitrary universal targets (§19.4: "do not set arbitrary universal targets such as zero violations in 1,000 cases proves safety"); record measured figures as the v0.3.0 baseline.
  4. Publish the benchmark results in the Simulation Lab (P5.08) and in the phase handover (P5.30).
- **Deliverable/output:** Benchmark report (measured figures) + Simulation Lab display.
- **Acceptance criteria / exit check:** Metrics recorded per scenario per applicable area; conditions recorded; no invented target; figures visible in Simulation Lab.
- **Owner role:** Numerical engineer + QA.
- **Dependencies:** P5.18–P5.20.
- **Verification method:** Benchmark output; metric-completeness check.
- **Risks/failure modes:** A metric omitted because it looked bad. Mitigation: the metric list is fixed per area; omissions are recorded with a reason.

### P5.24 — Performance and load measurement (§19.2 layer 8, §19.4)

- [ ] **Objective:** Measure console and platform performance under load: telemetry ingest rate, scheduling runtime, query latency, command fan-out, recovery.
- **Report trace:** §19.2 layer 8, §19.4 (platform metrics), §21.3.
- **Inputs/prerequisites:** P5.23; Phase 1–4 services running.
- **Steps:**
  1. Measure: telemetry ingest rate, scheduling runtime (already in P3.27 — reuse, do not re-measure differently), API query latency (p95/p99), command fan-out time, and recovery time after a service restart.
  2. Measure console performance: initial bundle load and workspace render time on the §21.4 browser matrix (record per browser); do not claim a figure for a browser not tested.
  3. Record the load conditions (portfolio size, telemetry rate, concurrent users simulated) with every figure.
  4. Add the load tests to CI at a smoke level (full load tests are a release-gate activity, §22.3 — record which level was run here).
- **Deliverable/output:** Performance report + CI smoke wiring.
- **Acceptance criteria / exit check:** All five platform measures recorded with conditions; console figures recorded per browser tested; load level (smoke vs full) stated.
- **Owner role:** QA + frontend engineer.
- **Dependencies:** P5.23.
- **Verification method:** Performance-test output.
- **Risks/failure modes:** Load figures from an unloaded laptop presented as deployment figures. Mitigation: conditions are part of every figure.

### P5.25 — Live updates: SSE/WebSocket and observability (§15.1, §21.3)

- [ ] **Objective:** Wire live console updates over SSE/WebSocket and correlate them with the platform's observability records.
- **Report trace:** §15.1, §21.3, §7.5.
- **Inputs/prerequisites:** P5.01; Phase 1–4 events (NATS) and tracing.
- **Steps:**
  1. Implement live updates for telemetry, schedule state, and dispatch lifecycle via SSE or WebSocket (§15.1) from the control plane; the console subscribes per tenant/site scope (P5.26) and never receives another tenant's stream.
  2. Handle connection loss honestly: on disconnect, the console shows a "live updates paused — data may be stale" state with the last-update time, and re-syncs on reconnect (no silent gap-filling).
  3. Correlate console views with trace IDs (§21.3): a displayed dispatch state links to the same command/schedule IDs and trace ID as the audit record.
  4. Measure ingestion delay, stale telemetry, queue depth, solver duration/status, command delivery/acknowledgement, and tracking deviation (§21.3) and surface the freshness-relevant ones in the console (P5.10).
  5. Never put secrets or unnecessary personal/customer data in logs or in the live stream.
- **Deliverable/output:** Live-update channel + disconnect handling + trace correlation + tests.
- **Acceptance criteria / exit check:** Live updates flow for telemetry/schedule/dispatch; disconnect shows stale state and re-syncs; trace IDs match audit records; no cross-tenant stream leakage in tests; no secrets in stream/logs.
- **Owner role:** Frontend + Rust engineer.
- **Dependencies:** P5.01.
- **Verification method:** Live-update tests incl. disconnect/reconnect; tenant-stream tests; log scan.
- **Risks/failure modes:** Silent gap-filling after reconnect. Mitigation: re-sync is explicit and the stale period is shown.

### P5.26 — Console security: RBAC, tenancy, and step-up approval (§17, §16.2, §20.2)

- [ ] **Objective:** Enforce console-side security: tenant-scoped views, RBAC on every action, step-up approval for high-impact actions, and audit of denials.
- **Report trace:** §17.2–§17.4, §16.2, §20.2, §21.3.
- **Inputs/prerequisites:** P5.02–P5.09; Phase 1 identity/RBAC; P4.13, P4.28.
- **Steps:**
  1. Derive tenant context from authenticated claims/authorized context, never from untrusted request body fields (§16.2); the console never lets a user select another tenant's data by editing a request.
  2. Enforce RBAC plus resource/site/asset-scoped authorization on every console action (view and mutate); the API enforces it too — the console hiding a button is not the enforcement (P4.29 invariant-7 discipline applied to the UI).
  3. Require MFA/step-up approval for high-impact administrative/control actions where required (§17.3); the approval flow from P5.06 is the implementation.
  4. Audit cross-tenant access denials (§16.2) and unauthorized console actions; test with a user attempting another tenant's workspace and an unauthorized approval.
  5. Keep emergency actions and normal scheduling visually separate (§20.2; see P5.06 open item if no emergency action is defined).
- **Deliverable/output:** Console authorization enforcement + denial audit + tests.
- **Acceptance criteria / exit check:** Cross-tenant access denied at API and console, and audited; unauthorized approval blocked; step-up required where configured; hidden-button-only "enforcement" does not exist (API tests prove it).
- **Owner role:** Security engineer + frontend engineer.
- **Dependencies:** P5.02–P5.09.
- **Verification method:** Tenant/role tests at API and UI levels; audit inspection.
- **Risks/failure modes:** UI-only authorization. Mitigation: API-level tests are the gate; UI tests are secondary.

### P5.27 — Control-authority mode enforcement in the console (§21.4, §17.5)

- [ ] **Objective:** Make the console enforce (not just display) the deployment's control-authority mode.
- **Report trace:** §21.4, §17.5, §5.7.
- **Inputs/prerequisites:** P5.13, P5.26; deployment mode configuration from Phase 1.
- **Steps:**
  1. Read the mode (Simulation / read-only/shadow / operator-approved / bounded automation) from deployment + per-site configuration (§21.4); a plugin can never select or raise it (§21.4, §5.7). In this phase the configured mode is Simulation or read-only/shadow only (P5.02); the other modes are listed only as the configuration vocabulary.
  2. In Simulation and read-only/shadow modes, the console disables (not just hides) any action that would issue a command to a non-simulator target; the API rejects such attempts independently (P4.16 guard).
  3. Display the mode per site where modes differ by site (mixed-mode deployment, §21.4).
  4. Test: in Simulation mode, an attempt to issue outside the simulator is rejected at API and console; mode promotion requires the §22.3 release gates plus site-specific acceptance — record that no promotion is in scope for this phase.
- **Deliverable/output:** Mode enforcement + per-site display + tests.
- **Acceptance criteria / exit check:** Mode read from configuration; non-simulator issue disabled and API-rejected in Simulation mode; mixed-mode display works; no promotion path in this phase.
- **Owner role:** Security engineer + frontend engineer.
- **Dependencies:** P5.13, P5.26.
- **Verification method:** Mode-enforcement tests; mixed-mode fixture.
- **Risks/failure modes:** A mode flag in the console that the API ignores. Mitigation: API rejection is tested independently.

### P5.28 — CI quality gates for the console (§22.2, §7.5)

- [ ] **Objective:** Wire the console into the repo's CI quality gates so a failing lint, type-check, test, or build blocks the change.
- **Report trace:** §22.2, §7.5, §21.4 (browser/OS matrix).
- **Inputs/prerequisites:** P5.01, P5.25.
- **Steps:**
  1. Add frontend lint/type-check/test/build to CI (§22.2); add the OpenAPI drift test (P5.01) and the scenario reproducibility check (P5.19) to the same pipeline.
  2. Add the supported browser/OS matrix test (§21.4) at the level recorded in P5.12 (full matrix is a §22.3 release-gate activity — record which level runs in CI here).
  3. Add dependency scanning and SBOM generation for the console's dependencies (§22.2); record licenses for the D4 audit (React MIT, Vite MIT, TanStack MIT — verify at scaffold time, do not assume).
  4. Verify a deliberately broken change (failing test, type error, lint error) fails CI; record the verification.
- **Deliverable/output:** CI wiring + scan/SBOM outputs + break-verification record.
- **Acceptance criteria / exit check:** CI runs lint/type-check/test/build + drift + reproducibility checks; a deliberate break fails CI; scans run; licenses recorded.
- **Owner role:** Frontend engineer + DevOps/CI owner.
- **Dependencies:** P5.01, P5.25.
- **Verification method:** CI run logs; deliberate-break test.
- **Risks/failure modes:** CI that runs but never fails. Mitigation: the deliberate-break verification.

### P5.29 — Documentation and operator guide (§20, §24)

- [ ] **Objective:** Document the console for its operator: what each workspace shows, what each state means, and what the console cannot do.
- **Report trace:** §20.1–§20.2, §24 steps 9–10, §21.4.
- **Inputs/prerequisites:** P5.02–P5.13.
- **Steps:**
  1. Write the operator guide: workspace-by-workspace meaning, state/value-type legend (P5.10–P5.11), how to read a rejection (P5.06), how to read the audit trail (P5.09), how to run and replay a scenario (P5.08, P5.18–P5.19), and how to interpret a fault-injected run (P5.22).
  2. State the console's limits in the guide: simulation/shadow only in this phase; no field control; acknowledgement is not physical delivery; shadow agreement is not a safety claim; steady-state results are not stability evidence (P5.17).
  3. Document the browser matrix and the role model (which roles see and do what).
  4. Review the guide with a non-author reader (operator or QA) for comprehensibility; record the review.
- **Deliverable/output:** Operator guide + review record.
- **Acceptance criteria / exit check:** Guide covers all listed topics; limits stated; role/browser documentation present; non-author review recorded.
- **Owner role:** Technical writer + frontend engineer.
- **Dependencies:** P5.02–P5.13.
- **Verification method:** Guide review; non-author read-through.
- **Risks/failure modes:** Guide describing a future console instead of this one. Mitigation: the guide is reviewed against the built console.

### P5.30 — Exit-gate demonstration and handover to Stage 6 (§23 Stage 5)

- [ ] **Objective:** Demonstrate the §23 Stage 5 exit gate end-to-end and hand over to Stage 6 with an honest record of what is ready and what is not.
- **Report trace:** §23 Stage 5 exit gate, §24 steps 9–10, §22.3.
- **Inputs/prerequisites:** P5.01–P5.33.
- **Steps:**
  1. Run the full demonstration: an operator runs a complete simulated VPP scenario (§24 steps 1–8) in the console, inspects every scheduling and dispatch decision (step 9: the audit trail) — including at least one rejected command inspected with its failing constraint and evidence link — and replays the same scenario with the same versions and verifies reproducibility (step 10: P5.19).
  2. Record the demonstration: scenario ID + version, operator actions taken, decisions inspected, reproducibility verdict, benchmark figures (P5.23), and any defect found.
  3. Update PLAN.md/CONTEXT.md/CHECKLIST.md in this directory with actual outcomes (versions, scenarios run, browser matrix results, open items).
  4. Write the handover note for Stage 6 (edge/protocol integration): console contracts, simulator integration points, HELICS federation definition, known limitations (unbalanced three-phase from v0.4.0; real adapters not built; HIL deferred; operator-approved control is v1.0.0), and the §22.3 release-gate status for v0.3.0.
  5. Run `python tools/verify_docs.py` and `python tools/validate_agent.py` (AGENTS.md §6) and record their output; fix failures before claiming the gate.
  6. Obtain exit-gate sign-off: "an operator can run a full simulated VPP scenario and inspect every scheduling and dispatch decision" — demonstrated, not asserted. If any item fails, the phase is not complete — record what is missing.
- **Deliverable/output:** Demonstration record + updated phase docs + handover note + verification-tool output + gate sign-off.
- **Acceptance criteria / exit check:** Full scenario demonstrated in the console with audit inspection and verified replay; handover note filed; both verification tools pass; CHECKLIST.md fully evidenced; no subtask checked without evidence.
- **Owner role:** Phase lead (architect).
- **Dependencies:** P5.01–P5.33.
- **Verification method:** Demonstration record; checklist review; tool output; gate sign-off.
- **Risks/failure modes:** A scripted demo that cannot be repeated by the operator alone. Mitigation: the operator runs it (not the developer presenting it) — record who operated.

### P5.31 — Alarm model and contract: own the gap behind §20.1 item 1 "key alarms" (§20.1 item 1, §15.3)

- [ ] **Objective:** Resolve the alarm-contract gap P5.02 records but no subtask owns: either define the minimal alarm model the Overview workspace displays, or formally escalate it to the owning phase with an owner and a date — so "key alarms" (§20.1 item 1) is buildable, not permanently "to be recorded as a gap".
- **Report trace:** §20.1 item 1, §15.3 (event vocabulary — no Alarm event exists), §20.2.
- **Inputs/prerequisites:** P5.02 (gap as recorded there); Phase 2 telemetry/quality model (alarm sources, if any); P9.25 (pilot alert catalogue — the consumer, not the definer).
- **Steps:**
  1. Inventory alarm sources that exist by Phase 5: telemetry quality transitions (BAD/STALE), feasibility rejections, dispatch failures/expiries, fallback activations, edge/adapter health (P6.17). Record which sources exist and which do not.
  2. Define the minimal alarm contract (or prove it already exists in a prior phase and cite it): alarm ID, source, severity, state (active/acknowledged/cleared), tenant/site/asset scope, first/last-seen timestamps, evidence link. Alarms are derived from events (§15.3), never invented by the console.
  3. If the contract cannot be defined inside Phase 5 (e.g. it needs a Phase 1/2 event or API), escalate formally: named owning phase + subtask, owner, and date — and record the Overview workspace as showing "alarms unavailable — contract pending" rather than fabricated alarms.
  4. Wire the Overview workspace (P5.02) to the contract (or the explicit unavailable state) and test with a fixture containing one alarm per source.
- **Deliverable/output:** Alarm contract definition (or a dated escalation record) + Overview wiring + tests.
- **Acceptance criteria / exit check:** Either the contract exists and Overview displays real alarms per source, or the escalation record exists with owner/date and Overview shows the explicit unavailable state; no fabricated alarm appears in either case.
- **Owner role:** Architect + frontend engineer.
- **Dependencies:** P5.02; Phase 2 telemetry model (read-only).
- **Verification method:** Contract review; Overview fixture tests; escalation-record inspection where applicable.
- **Risks/failure modes:** Demo alarms mistaken for live alarms. Mitigation: every alarm carries its source and evidence link; demo data is labelled demo (P5.12).

### P5.32 — Tenant-context display and tenant-switch guard (§16.2, §20.2)

- [ ] **Objective:** Make the current tenant visible on every workspace and guard tenant switching, so an operator can never act under the wrong tenant without a visible cue.
- **Report trace:** §16.2 (tenant context from authenticated claims), §20.2 (role-specific screens).
- **Inputs/prerequisites:** P5.26 (tenant context derived from claims); P5.02–P5.09 (workspaces).
- **Steps:**
  1. Add a persistent tenant indicator (tenant name/ID) to the console chrome, visible on every workspace; it is populated from the authenticated session, never from a URL parameter or request field.
  2. If tenant switching is supported, the switch re-derives the full context (claims, roles, scopes) and re-loads every workspace; a switch that fails to re-derive context leaves the previous tenant's data visible with a stale-context warning — never silently mixed data.
  3. Test: a user with two tenants sees only the active tenant's data after a switch; an attempt to force another tenant via a manipulated request is denied at the API and audited (P5.26).
  4. Record the single-tenant case explicitly: if the deployment is single-tenant, the indicator still shows (it is the proof the context is what the operator thinks it is).
- **Deliverable/output:** Tenant indicator + switch guard + tests.
- **Acceptance criteria / exit check:** Tenant shown on every workspace; switch re-derives context; cross-tenant forcing denied at API and audited; single-tenant deployments still show the indicator.
- **Owner role:** Frontend engineer + security engineer.
- **Dependencies:** P5.26.
- **Verification method:** UI tests with two tenants; manipulated-request denial test; audit inspection.
- **Risks/failure modes:** Tenant shown from an untrusted field (spoofable). Mitigation: the indicator is populated from the authenticated session only.

### P5.33 — Live-update ordering and deduplication (§15.4, §15.1)

- [ ] **Objective:** Make the live-update stream (§15.1) robust to at-least-once delivery: out-of-order or duplicate events must never overwrite fresher displayed state.
- **Report trace:** §15.4 (at-least-once, idempotent consumers), §15.1, §21.3.
- **Inputs/prerequisites:** P5.25 (live updates, disconnect/re-sync); P4.24 (command idempotency/sequence discipline — reuse the pattern, do not reinvent it).
- **Steps:**
  1. Require a per-stream sequence (or event-time + source-sequence) on every live event; the console keeps the last-applied sequence per stream (telemetry, schedule, dispatch) and discards events at or below it.
  2. Define the stale-event rule: an event older than the currently displayed state for the same entity is discarded and counted (not applied, not silently dropped — the discard count is observable in diagnostics).
  3. Handle duplicates: a redelivered event produces no visible change and no duplicate audit/derived record.
  4. Test with a reordered/redelivered event stream (including a stale dispatch state arriving after a fresher one) and verify the displayed state is the freshest, with discards counted.
- **Deliverable/output:** Ordering/dedup implementation + discard diagnostics + tests.
- **Acceptance criteria / exit check:** Reordered/redelivered streams display the freshest state; stale events discarded and counted; duplicates cause no duplicate side effect.
- **Owner role:** Frontend + Rust engineer.
- **Dependencies:** P5.25.
- **Verification method:** Reordered-stream tests; discard-count inspection.
- **Risks/failure modes:** Sequence gaps treated as errors that freeze the UI. Mitigation: a gap is a diagnostic (possible loss, P5.25 re-sync applies), not a freeze — record the rule.

## Cross-phase references

- Consumes from Phase 3: schedule/comparison APIs (P3.21–P3.23), envelope (P3.12). See `ledger_phase_3.md`.
- Consumes from Phase 4: command lifecycle + audit (P4.10–P4.16, P4.30), feasibility evidence (P4.07), rejection evidence (P4.27), simulator adapter boundary (P4.16). See `ledger_phase_4.md`.
- Consumed by Stage 6: console contracts, simulator integration points, HELICS federation definition (P5.16), scenario library (P5.20).
- NOT built here: edge agent, protocol adapters (Stage 6), plugin SDK (Stage 7), research modules (Stage 8), field control authority (v1.0.0).
