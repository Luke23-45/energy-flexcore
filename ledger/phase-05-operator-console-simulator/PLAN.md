# Plan — Phase 5: Operator Console and Simulator Integration

- **Report:** `docs/plan/Neryva_FlexCore_Product_and_Architecture_Report_v1.1.md` (§23 Stage 5)
- **Release target:** v0.3.0 console surface within the 0.x simulation line (§2.1, §21.4). Simulation/shadow only; no field control.
- **Ledger:** `ledger_phase_5.md` (P5.01–P5.30). This plan is the architecture companion; the ledger is the executable checklist.

## 1. Scope

### In scope

- Operator console (§20.1, all 12 workspaces): Overview, Portfolios, Sites, Assets, Grid, Schedules, Dispatch, Market & Programs, Simulation Lab, Plugins & Integrations, Audit & Reports, Administration.
- §20.2 design principles applied workspace-wide: freshness, value-type distinction, lifecycle-state distinction, rejection explanations, model version/timestamp on feasibility outcomes, explicit confirmation for high-impact actions, role-specific screens, emergency/normal separation.
- Simulator adapters and scenario replay (§19.1–§19.2): asset simulator, grid (pandapower) adapter, HELICS co-simulation federation; scenario definition, replay, reproducibility verification (§24 step 10); failure injection (§19.2 layer 6).
- Benchmarks and §19.4 metrics; performance/load measurement; live updates (SSE/WebSocket); console security (RBAC, tenancy, step-up, mode enforcement); CI gates; operator guide; exit-gate demonstration (§24 steps 9–10).

### Non-goals (explicitly out of scope)

- Edge agent, protocol adapters, OpenEMS integration — Stage 6.
- Plugin SDK / WASM host — Stage 7 (Plugins workspace shows registered plugins only).
- GNN/MARL/ADMM research modules — Stage 8.
- Field control authority — v1.0.0 (§21.4). The console in this phase operates simulation/shadow workflows.
- HIL testing — Stage 9 qualification; never implied by a simulation result.
- Mobile app, native desktop client (§21.4).
- Kubernetes/Helm artefacts (§21.1 — after core contracts stabilize).

## 2. Architecture decisions in play

| Decision | Position in this phase | Report trace |
|---|---|---|
| Console is a view, never the authority | All state changes go through the control-plane API; the console hiding a button is not enforcement | §20.2, §17.5, §5.7 |
| Simulated values are labelled simulated everywhere | Value-type treatments are components, not optional props | §20.2, §21.4 |
| Proposed is never displayed as executed | Distinct lifecycle-state treatments; acknowledgement is acceptance, not delivery | §20.2, §14.1 |
| HELICS coordinates, never replaces physics | Grid/asset federates own their models | §12.4, §19.1 |
| Replay uses only recorded inputs | Missing input errors loudly; tolerances pre-registered | §19.2, §24 step 10 |
| Failure injection is visible | Injected runs are labelled; clean re-run is one action away | §19.2, §20.2 |
| Mode is configuration, not a plugin choice | Simulation/read-only/shadow in this phase; promotion needs §22.3 gates + site acceptance | §21.4, §5.7 |

## 3. Component breakdown

| Component | Location (per §8.1) | Ledger |
|---|---|---|
| Console app | `web/console/` (Vite + React + TS) | P5.01 |
| Workspaces 1–12 | `web/console/` feature modules + `web/ui-contracts/` | P5.02–P5.09 |
| Design system (state/value treatments, legend) | `web/console/` shared components | P5.10–P5.12 |
| Live updates | SSE/WebSocket from control plane | P5.25 |
| Asset simulator integration | `plugins/builtin/asset-simulator`, `apps/sim-runner` | P5.14 |
| Grid simulator adapter | `python/workers/powerflow-pandapower/` (Phase 4 adapter) | P5.15 |
| HELICS federation + FlexCore federate | Simulator environment (Linux containers) | P5.16 |
| Scenario library + replay runner | `scenarios/`, `apps/sim-runner` | P5.18–P5.20 |
| Failure injection | Scenario runner | P5.21–P5.22 |
| Benchmarks | `benchmarks/` | P5.23–P5.24 |

## 4. Data contracts

| Contract | Consumed from | Shown as |
|---|---|---|
| Schedule + comparison (Phase 3) | `/api/v1/schedules`, `/api/v1/optimization-runs` | ID + revision, objective config, §12.6 status, per-candidate §19.4 metrics |
| Feasibility result (Phase 4) | `/api/v1/feasibility-checks` | Per-limit pass/fail + margin, model version + hash + timestamp, coverage limitations |
| Dispatch command + audit (Phase 4) | `/api/v1/dispatch-commands`, `/api/v1/audit-events` | Full lifecycle chain with evidence references; rejection reason + failing constraint |
| Telemetry (Phase 2) | `/api/v1/telemetry/query` | Value + quality (GOOD/SUSPECT/BAD/MISSING/ESTIMATED/STALE) + freshness |
| Scenario record | `scenarios/` | Pinned inputs, versions, seeds, expected outcomes/tolerances, reproducibility verdict |
| HELICS federation config | Scenario record | Federates, pub/sub, time step, broker, version |

Canonical units (§10.2): MW, MVAr, MVA, MWh; `P > 0` export; SOC [0,1].

## 5. Interfaces

- **Inbound (console):** REST/OpenAPI from the control plane (generated client, P5.01); SSE/WebSocket live updates (§15.1); OIDC/RBAC identity (Phase 1).
- **Outbound (console):** operator actions (approval, cancellation, scenario run/replay, injection control) through the REST API only — no direct device, database, NATS, or plugin access from the browser (§17.2 zones; §20.2 "frontend plugin mechanism must not define authoritative permissions or directly issue device commands", §7.5).
- **Simulator side:** asset simulator, pandapower adapter, and HELICS federates exchange setpoints/measurements through platform adapter interfaces; the console observes via the API, never by joining the federation directly.
- **Consumed from Phases 3–4:** schedule/comparison, feasibility, command/audit APIs. **Provided to Stage 6:** console contracts, simulator integration points, HELICS federation definition, scenario library.

## 6. Sequencing

1. Scaffold + generated client + CI (P5.01, P5.28 started).
2. Workspaces in dependency order: Overview → Portfolios/Sites/Assets → Grid → Schedules → Dispatch → supporting workspaces → Simulation Lab → Audit (P5.02–P5.09).
3. Design principles applied and audited workspace-wide (P5.10–P5.13).
4. Simulator adapters: asset, grid, HELICS, scoping decision (P5.14–P5.17).
5. Scenario definition, replay, reproducibility, library (P5.18–P5.20).
6. Failure injection + visibility (P5.21–P5.22).
7. Benchmarks, performance, live updates, security, mode enforcement (P5.23–P5.27).
8. Operator guide + exit-gate demonstration + handover (P5.29–P5.30).

Relative effort (no calendar dates): the 12 workspaces are the largest share, then simulator/HELICS integration, then replay/reproducibility. The design-principle audits (P5.10–P5.13) are small in code but gate the exit — do not treat them as polish to cut.

## 7. Roles

| Role | Owns |
|---|---|
| Architect | Console/authority boundary, HELICS scoping, handover, exit-gate sign-off |
| Frontend engineer | Scaffold, all workspaces, design system, live updates, accessibility |
| Rust engineer | API/live-update server side, scenario runner, mode enforcement |
| Numerical (Python) engineer | Grid adapter, HELICS federate, benchmarks |
| Power-systems engineer | Grid/Schedules/Dispatch content correctness, scenario library, benchmark interpretation |
| QA | Replay/reproducibility, failure injection, browser matrix, demonstration |
| Security engineer | RBAC/tenancy/step-up, mode enforcement, denial audit |
| Technical writer | Operator guide (P5.29) |

## 8. Risks and mitigations

| Risk | Mitigation |
|---|---|
| Simulated value read as measured/executed | Value-type + state treatments tested workspace-wide (P5.10–P5.13) |
| Console-only authorization | API enforcement tested independently (P5.26) |
| HELICS mistaken for the physics model | Role stated in run record and UI (P5.16) |
| Replay using unrecorded inputs | Error-on-missing-input; pre-registered tolerances (P5.18–P5.19) |
| Fault-injected run presented as clean | Injection indicator on run record and display (P5.22) |
| Steady-state result presented as stability evidence | Scoping table + guide limits (P5.17, P5.29) |
| IEEE feeder claimed without license check | Per-case availability record (P5.20) |
| Generated API client drift | Drift test in CI (P5.01, P5.28) |

## 9. What must NOT be built yet

- Edge agent / protocol adapters (Stage 6).
- Plugin SDK / WASM host (Stage 7).
- Research modules (Stage 8).
- Field control authority, HIL, Kubernetes/Helm artefacts.
- Any emergency dispatch action not defined in the report (P5.06 open item).

## 10. How the §24 vertical slice lands here

§24 steps 9–10 land in this phase: "display the complete decision/audit trail in the web console" (P5.09, P5.30 demonstration) and "replay the same scenario with the same versions and verify reproducibility" (P5.18–P5.19). Steps 1–2 are Phase 2, steps 3–4 Phase 3, steps 5–8 Phase 4. The vertical slice is complete only when this phase's demonstration (P5.30) runs it end-to-end with an operator at the console.

## 11. §19.1 simulator-track scoping (P5.17)

The full decision record — reasons, deferral triggers, D-gate pendings, and the plugin-kernel note — is `17_scoping_decision.md`. The table, as filed there (ledger P5.17 step 4):

| §19.1 track | Decision (DEFAULT) | Phase / trigger |
|---|---|---|
| Portfolio scheduling | INTEGRATED (P5.14) | Phase 5; new asset classes re-open |
| Distribution quasi-static | INTEGRATED, pandapower only, under the D3 default (balanced radial) (P5.15) | Unbalanced three-phase: pending D3, T0–T5 triggers (`34_unbalanced_deferral.md`); GridLAB-D/OpenDSS: a named case only (§12.4) |
| Transmission/planning (PyPSA) | DEFERRED — optional comparison only, pending D3 | Re-open when D3 admits transmission studies AND a named §19.3 case requires it |
| Dynamic stability (ANDES) | DEFERRED — explicit | Stage 8, with a named research controller + declared dynamic model |
| Multi-domain co-simulation (HELICS) | INTEGRATED, minimal three federates (P5.16); market/communication legs PENDING D1 / deferred | Market federate: pending D1; communication federate: Stage 6 |
| Hardware-in-the-loop | DEFERRED — explicit | Stage 9, after §19.2 layers 1–9 + the cyber review (D6) |

Constraint carried with the table (§19.1): a steady-state power-flow tool is not evidence for transient frequency stability; a completed co-simulation is not a power-system validation by itself; HELICS coordinates — it does not replace the federates' physics models.
