# P0.24 — Vertical-slice map (§24 steps → §23 stages)

| Field | Value |
|---|---|
| Subtask | P0.24 — Define the §24 vertical slice and map its steps to stages |
| Report trace | §24, §23 (Stages 0–6), §2.1 |
| Source | `docs/plan/Neryva_FlexCore_Product_and_Architecture_Report_v1.1.md` |
| Status | Draft for Stage 0 review |
| Decisions | D1–D6 remain OPEN (§25.2); this map decides none of them |

---

## 1. The ten vertical-slice steps (§24, verbatim in substance)

> 1. Create a tenant, portfolio, site, grid model, and four simulated assets: BESS, PV, flexible load, EVSE.
> 2. Generate timestamped telemetry with quality flags.
> 3. Calculate each resource's available flexibility over a defined 24-hour horizon.
> 4. Optimize a simple portfolio schedule using a solver appropriate to the chosen LP/MILP/QP formulation.
> 5. Validate the candidate against asset capability and a feeder power-flow model.
> 6. Create a PROPOSED command and demonstrate validation/rejection/approval transitions.
> 7. Send it only to a simulator adapter.
> 8. Simulate acknowledgement, tracking, partial failure, and timeout.
> 9. Display the complete decision/audit trail in the web console.
> 10. Replay the same scenario with the same versions and verify reproducibility.

Per §24: "This proves the product's actual value: orchestration and validated coordination. Advanced AI can then improve a measured bottleneck instead of being introduced speculatively."

## 2. Caveat — the slice is not a v0.1.0 commitment (§24, verbatim)

> This is the target vertical slice for the 0.x line. Its steps land across v0.1.0–v0.3.0; they are not all promised in the first public release.

Consistently, §2.1 assigns the slice's capabilities across releases: v0.1.0 (canonical models, simulator integration, telemetry, registry, portfolio views, replay — simulation only), v0.2.0 (flexibility envelopes, deterministic scheduling), v0.3.0 (feasibility checks, command lifecycle, approvals, audit — simulation and shadow mode). No slice step is a field write: step 7 sends only to a simulator adapter, and the whole 0.x line is simulation-only (§0, §2.1).

## 3. Step-to-stage map (§23)

Verified against §23 Stage deliverables and exit gates. Where the ledger's expected mapping and the report's stage content differ in nuance, the note column states the report's own wording.

| §24 step | Step (short) | §23 stage(s) | Basis in the report |
|---|---|---|---|
| 1 | Create tenant, portfolio, site, grid model, four simulated assets | Stage 2 | Stage 2 delivers the tenant/portfolio/site/asset registry and canonical telemetry model; its exit gate is a synthetic BESS/PV/load/EVSE portfolio |
| 2 | Timestamped telemetry with quality flags | Stage 2 | Stage 2 delivers the canonical telemetry model and units, simulator telemetry source, data quality and stale detection |
| 3 | Flexibility over a 24-hour horizon | Stage 3 | Stage 3 delivers capability models and flexibility envelope calculation |
| 4 | Optimize portfolio schedule (LP/MILP/QP-appropriate solver) | Stage 3 | Stage 3 delivers the LP/MILP/QP scheduler with declared problem class and the solver result/diagnostics contract |
| 5 | Validate against asset capability and feeder power-flow model | Stage 4 | Stage 4 delivers the selected grid-model provider, AC power-flow validation path, voltage/thermal/transformer constraints |
| 6 | PROPOSED command; validation/rejection/approval transitions | Stage 4 | Stage 4 delivers command lifecycle and audit, human approval and signed command model; exit gate rejects invalid, stale, infeasible, unauthorized, duplicate, expired candidates |
| 7 | Send only to a simulator adapter | Stage 4, with the real-site counterpart at Stage 6 | Stage 4 delivers the "simulation-only dispatch adapter" (the report's own wording); Stage 6 delivers the Rust edge agent and selected OpenEMS integration for the later read-only site path. The ledger's "Stages 4/6" holds with this distinction: the simulator adapter itself is Stage 4; Stage 6 is where outbound site integration lands |
| 8 | Simulate acknowledgement, tracking, partial failure, timeout | Stage 4 | Stage 4 owns the command lifecycle (§14 states, including ACKNOWLEDGED, TRACKING, FAILED, EXPIRED) and its exit gate |
| 9 | Decision/audit trail in the web console | Stage 5 | Stage 5 delivers fleet/sites/assets/grid/schedule/dispatch workspaces and operator audit and reports; exit gate: an operator can run a full simulated VPP scenario and inspect every scheduling and dispatch decision |
| 10 | Replay same scenario, same versions, verify reproducibility | Stages 2 and 5 | Stage 2's exit gate requires portfolio telemetry "replayed deterministically"; Stage 5 delivers simulator adapters and scenario replay |

Every step maps to at least one stage; no step is unmapped, and none is claimed for Stage 0.

## 4. What Phase 0 and Phase 1 contribute

- **Phase 0 (this phase) implements none of the ten steps.** Phase 0 freezes the vocabulary and contracts the steps use: canonical units and signs (P0.07), telemetry quality states (P0.08 — step 2's "quality flags"), time model (P0.09), asset/capability vocabulary (P0.10 — step 1's entities), command state machine (P0.11 — steps 6 and 8), grid-model contract (P0.12 — step 5), solver-class discipline (P0.13 — step 4), event vocabulary (P0.14), API surface (P0.15), tenancy and storage principles (P0.16 — step 1's tenant model), threat model (P0.17), control-authority modes (P0.18 — step 7's simulation-only rule), plugin model (P0.19), deployment form (P0.20), console scope (P0.22 — step 9), and test strategy (P0.23 — step 10's reproducibility check).
- **Phase 1 (report §23 Stage 1) provides the foundation the slice runs on**: Cargo workspace, contracts/domain/application/ports/adapters separation, Axum API, PostgreSQL migrations and tenant context, NATS event integration and outbox, tracing and metrics, CI and test harness. Its exit gate is one end-to-end non-control workflow through API, application use case, database, and event publication — a prerequisite for slice step 1, not slice delivery itself.
- Slice steps first land in Phase 2 (report Stage 2, steps 1–2). The map is re-checked at the Stage 1 and Stage 2 gates (ledger P0.24 risk note).

## 5. Self-check against P0.24 acceptance criteria

- All ten §24 steps listed: yes (§1 above).
- Each step mapped to at least one §23 stage: yes (§3 above).
- "Not all in v0.1.0" caveat stated with the report's own wording: yes (§2 above).
- No step claimed as delivered by Phase 0: yes (§4 above).
