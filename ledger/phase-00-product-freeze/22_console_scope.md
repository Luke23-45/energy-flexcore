# P0.22 — Operator-console scope freeze (workspaces only, no UI build)

| Field | Value |
|---|---|
| Subtask | P0.22 — Freeze the operator-console scope (workspaces only, no UI build) |
| Report trace | §20.1 (primary navigation), §20.2 (design principles), §1.2 (console surface), §10.4 (telemetry quality states), §24 Stage 5 (console build stage) |
| Source | `docs/plan/Neryva_FlexCore_Product_and_Architecture_Report_v1.1.md` (product baseline; foundations specification governs on conflict per AGENTS.md §2) |
| Inputs | P0.08 (telemetry quality vocabulary), P0.11 (command state machine), P0.15 (API surface) |
| Status | Draft for Stage 0 review |
| Decisions | D1–D6 remain OPEN (§25.2); this document decides none of them |

---

## 1. The twelve workspaces (§20.1)

The initial web console should expose these workspaces (§20.1), reproduced in report order with the report's own content descriptions:

| # | Workspace | Report content description (§20.1) |
|---|---|---|
| 1 | **Overview** | fleet availability, current aggregate power, active schedules, key alarms, delivery status |
| 2 | **Portfolios** | portfolio composition, aggregation rules, commitments, flexibility |
| 3 | **Sites** | site overview, PCC, grid model, edge connectivity, local EMS status |
| 4 | **Assets** | asset state, capability, telemetry quality, alarms, limits, maintenance/outage status |
| 5 | **Grid** | topology/model version, feeder/substation boundaries, voltage/thermal validation, model health |
| 6 | **Schedules** | day-ahead/intraday planning, objective selection, schedule alternatives, scenario comparison |
| 7 | **Dispatch** | proposed commands, validation results, approvals, issue/acknowledgement/tracking lifecycle |
| 8 | **Market & Programs** | market region, program signals, commitments, adapter status, delivery evidence |
| 9 | **Simulation Lab** | scenarios, replay, benchmark runs, result comparison |
| 10 | **Plugins & Integrations** | installed plugins, capabilities, permissions, versions, health and configuration |
| 11 | **Audit & Reports** | decision evidence, operator actions, schedule vs actual, dispatch failures |
| 12 | **Administration** | identity integration, roles, tenants, policies, data retention, edge enrollment |

Count: 12 workspaces. "Should expose" is the report's own wording — the workspace list is the frozen scope, not a committed UI build.

## 2. Display rules (§20.2 — all nine, none dropped)

Reproduced from the §20.2 principles, in report order:

1. Show operational state and data freshness prominently.
2. Distinguish measured, estimated, stale, and simulated values visually.
3. Never display a proposed setpoint as if it were an executed command.
4. Keep candidate, feasible, approved, issued, acknowledged, and measured states visually distinct.
5. Explain each rejected command with the failing constraint and supporting evidence.
6. Show model version and timestamp for grid feasibility outcomes.
7. Require explicit confirmation for high-impact actions.
8. Keep emergency actions and normal scheduling visually separate.
9. Provide role-specific screens and reduce irrelevant controls.

Cross-references frozen by these rules:

- Rule 2 carries the P0.08 quality vocabulary (`GOOD`, `SUSPECT`, `BAD`, `MISSING`, `ESTIMATED`, `STALE`, §10.4) and the simulated-data distinction from P0.18: simulation-only values must never be rendered as operational measurements.
- Rule 4 carries the P0.11 lifecycle states (candidate/feasible map to the VALIDATED/FEASIBLE gates, approved/issued/acknowledged to APPROVED/ISSUED/ACKNOWLEDGED, §14); each state is visually distinct, and acknowledgement is never rendered as physical delivery (§14.3).
- Rules 3 and 5 name the ledger's failure mode: a proposed setpoint rendered as executed in a demo is exactly the error §20.2 forbids, and a rejection must always show its failing constraint plus evidence.
- Rule 8 keeps emergency actions visually separate from normal scheduling; it creates no emergency-dispatch capability — FlexCore performs no protection-grade functions (AGENTS.md §3.3; ADR-012).

## 3. Phase assignment: console is Phase 5 work

Phase 0 freezes scope only. The console is built in **Phase 5 (report Stage 5, §24 "Operator console and simulator integration")** and is **not built in Phases 0–2**: Phase 0 produces no code (AGENTS.md §1), and the Stage 5 deliverable list is the build commitment (fleet/sites/assets/grid/schedule/dispatch workspaces, simulator adapters and scenario replay, failure injection, operator audit and reports; exit gate: an operator can run a full simulated VPP scenario and inspect every scheduling and dispatch decision, §24 Stage 5).

## 4. What this document does not do

- It specifies no screens, layouts, components, routes, or visual design: §20.1/§20.2 define workspaces and principles, and Phase 5 owns the UI implementation.
- It promises no mobile app and no native desktop client — the console is a browser-based web application (§1.2; §21.4 console row).
- It sets no performance, accessibility, or availability target for the console.

## 5. Acceptance-criteria coverage (ledger P0.22)

| Ledger acceptance criterion | Where satisfied |
|---|---|
| All twelve workspaces are listed | §1 (counted; in §20.1 order) |
| All nine §20.2 display rules are stated | §2 (nine rules, in §20.2 order) |
| The note states the console is not built in Phase 0–2 | §3 (Phase 5 / Stage 5 assignment) |

Verification method (ledger): comparison against §20.1–§20.2 — performed in §§1–2.
