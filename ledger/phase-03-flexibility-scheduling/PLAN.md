# Plan — Phase 3: Flexibility and Deterministic Schedule

- **Report:** `docs/plan/Neryva_FlexCore_Product_and_Architecture_Report_v1.1.md` (§23 Stage 3)
- **Release target:** v0.2.0, simulation only (§2.1, §21.4). No field write authority in this phase.
- **Ledger:** `ledger_phase_3.md` (P3.01–P3.31). This plan is the architecture companion; the ledger is the executable checklist.

## 1. Scope

### In scope

- Capability models for BESS (§11.2), inverter P-Q envelope (§11.3), flexible load (§11.4), EV fleet (§11.5), behind the generic capability contract (§11.1).
- Portfolio flexibility envelope as a constrained aggregation (§11.6), with staleness rules.
- Scheduling objective (§12.1) and PCC/market tracking (§12.2) with synthetic defaults, labelled as synthetic while D1/D2 are OPEN.
- Optimization tiers (§12.3): Tier 1 LP/QP/MILP via HiGHS; Tier 2 continuous SOCP via Clarabel; both behind the `Optimizer` port (§9.1).
- Optimization result contract (§12.6), schedule identity/revision/supersede, schedule comparison, rolling-horizon re-scheduling.
- Deterministic synthetic scenarios, benchmarks, API/event/persistence wiring (§15, §16), security review, handover to Phase 4.

### Non-goals (explicitly out of scope)

- Grid feasibility validation and AC power flow — Phase 4 (§12.4–§12.5). A schedule that "solves" is not feasible until Phase 4 says so.
- Command lifecycle, human approval, signed commands, dispatch — Phase 4 (§14).
- Operator console UI — Phase 5 (§20). Phase 3 exposes APIs only.
- Tier 3 MISOCP/MINLP and Tier 4 nonlinear AC-OPF (§12.3) — see ledger P3.20. Requests return `unsupported`.
- GNN, MARL, ADMM/DMPC (§13, Stage 8) — research plugins are proposals only and are not wired into this scheduler.
- Market integration, real price/commitment data, FERC Order 2222 qualification (§0.3, §12.1–§12.2). All prices and commitments in this phase are synthetic and labelled.
- Hard real-time, protection-grade, or safety claims (AGENTS.md rules 2–3).

## 2. Architecture decisions in play

| Decision | Position in this phase | Report trace |
|---|---|---|
| `Optimizer` port, not direct solver calls | Scheduling depends on the contract; HiGHS/Clarabel are providers behind it | §9.1, §5.2 |
| Tier 1 first (LP, then QP, then MILP) | Linearised models first; binaries only where a discrete decision is genuinely needed | §12.3 |
| Tier 2 continuous SOCP via Clarabel | Inverter cone and convex network approximations; integer input is rejected, not approximated | §12.3, §7.4 |
| SOCP relaxation exactness never assumed | Tier 2 output is a candidate; Phase 4 power flow is the independent check | §12.5 |
| Unknown capability narrows the envelope | Measured / configured / inferred / unknown provenance; unknown is conservative, never widening | §11.1, §17.5 inv. 9 |
| Schedules immutable, revisions supersede | New revision supersedes the old; nothing is mutated in place | §14.2, §12.6 |
| Synthetic objective/commitments labelled | D1/D2 OPEN, so no market product is implied | §12.1, §25.2 |
| Determinism by pinning | Solver versions, seeds, thread counts, and input snapshots are recorded per result | §12.6, §19.2 |

## 3. Component breakdown

| Component | Owner crate/worker (per §8.1) | Ledger |
|---|---|---|
| Capability contract + validators | `flexcore-contracts`, `flexcore-domain` | P3.01–P3.04 |
| BESS / P-Q / load / EV capability models | `flexcore-domain` | P3.05–P3.11 |
| Flexibility envelope calculator | `flexcore-application` (Flexibility context) | P3.12–P3.13 |
| Objective + PCC tracking | `flexcore-application` (Scheduling context) | P3.14–P3.16 |
| `Optimizer` port | `flexcore-optimization-api` | P3.17 |
| HiGHS Tier 1 provider | `plugins/builtin/optimizer-highs` + `python/workers/optimization-worker` | P3.17, P3.19 |
| Clarabel Tier 2 provider | `flexcore-optimization-api` provider / Python worker | P3.18–P3.19 |
| Result contract + schedule store | `flexcore-domain`, `flexcore-storage` (PostgreSQL) | P3.21–P3.22 |
| Schedule comparison + rolling horizon | `flexcore-application` | P3.23–P3.24 |
| REST + events + outbox | `apps/control-plane`, `flexcore-events` | P3.28 |
| Scenario + benchmark suites | `scenarios/synthetic-portfolio/`, `benchmarks/` | P3.26–P3.27 |

Dependency direction follows §6.2/§8.2: domain depends on nothing infrastructure; adapters depend on ports; apps are composition roots. No empty crates are created (§8, AGENTS.md §1).

## 4. Data contracts

| Contract | Fields (minimum) | Versioned by |
|---|---|---|
| Capability model | time-indexed feasible set `(P, Q, E, u)`; per-constraint provenance; units; sign; validity window | Schema version (P3.01) |
| Flexibility envelope | per-interval `P_min/P_max` (MW, `P > 0` export), central estimate + availability descriptor, envelope version, input snapshot refs, computation time | Envelope version (P3.12–P3.13) |
| Schedule | schedule ID + revision, horizon/interval grid, per-asset per-interval `(P, Q)` setpoints (MW/MVAr), objective config + weights, envelope version, forecast version, solver config, trigger, supersedes link | Schedule revision (P3.22) |
| Optimization result | all §12.6 fields (status, problem class, solver/provider version, objective + units, primal/dual diagnostics, residuals/violations, runtime/resources, input snapshot + grid model version, uncertainty/assumptions, warnings, candidate ID) | Result record (P3.21) |
| Comparison record | shared input snapshot ID, candidate schedule IDs, §19.4 scheduling metrics per candidate | Comparison record (P3.23) |

Canonical units (§10.2): MW, MVAr, MVA, MWh. SOC is a fraction in [0,1] at the API boundary. Telemetry quality: GOOD / SUSPECT / BAD / MISSING / ESTIMATED / STALE (§10.4).

## 5. Interfaces

- **Inbound:** REST `/api/v1/flexibility/envelopes`, `/api/v1/schedules`, `/api/v1/optimization-runs` (§15.2); NATS events consumed: `AssetCapabilityUpdated`, `TelemetryQualityChanged`, `AssetAvailabilityChanged`, `ForecastPublished` (§15.3).
- **Outbound:** events `FlexibilityEnvelopeUpdated`, `ScheduleProposed`, `ScheduleValidated` (§15.3) via transactional outbox (§15.5) to NATS JetStream.
- **Internal:** `Optimizer` port (§9.1) between the scheduling application service and the HiGHS/Clarabel providers; gRPC/Protobuf between the Rust control plane and Python optimization workers (§15.1, §7.2 — Python is for numerical workers only, never the command authority).
- **Consumed from Phase 2:** asset registry, telemetry (with quality flags). Forecasts are NOT consumed from Phase 2 — no Phase 2 subtask produces them (see P2.28); Phase 3 owns its synthetic forecast fixture (P3.31). **Provided to Phase 4:** schedule ID + revision, envelope version, §12.6 result records. **Provided to Phase 5:** comparison and schedule APIs.

## 6. Sequencing

1. Contract + time model + validators (P3.01–P3.04) — nothing else starts before the contract freezes.
2. Asset capability models (P3.05–P3.11) — BESS first, then P-Q, load, EV; each with edge cases.
3. Envelope + staleness (P3.12–P3.13).
4. Objective + PCC tracking (P3.14–P3.16).
5. Optimizer port + Tier 1, then Tier 2, then config/determinism, then the Tier 3/4 guard (P3.17–P3.20).
6. Result contract, revisions, comparison, rolling horizon (P3.21–P3.24).
7. Tests, scenarios, benchmarks (P3.25–P3.27) — written alongside, closed here.
8. API/events/persistence, security review, handover (P3.28–P3.30).

Relative effort (no calendar dates): steps 1–2 are the largest share (capability correctness is the phase's risk centre); step 5 is next (two providers + determinism); steps 6–8 are smaller but gate the exit. Do not compress step 1 to "save time" — every later subtask depends on it.

## 7. Roles

| Role | Owns |
|---|---|
| Architect | Contract freeze, tier boundary, handover, exit-gate sign-off |
| Power-systems engineer | BESS/P-Q/load/EV models, envelope, objective, PCC tracking, scenario design |
| Rust engineer | Domain/application code, ports, API, persistence, outbox |
| Numerical (Python) engineer | Solver providers, determinism, benchmarks |
| QA | Unit/property/scenario/contract tests, coverage and gap list |
| Security engineer | P3.29 review |
| Product | Synthetic-default labelling, D1/D2 status tracking |

## 8. Risks and mitigations

| Risk | Mitigation |
|---|---|
| Nameplate-sum envelope overstates flexibility | Binding-shared-constraint test is a gate (P3.12) |
| Simultaneous charge/discharge via relaxation | Post-solution check mandatory (P3.05) |
| Solver-class conflation (HiGHS as SOCP, Clarabel as MISOCP) | Port enforces problem class; integer input to Tier 2 returns `unsupported` (P3.17–P3.18) |
| SOCP relaxation assumed exact | Caveat documented; Phase 4 independent power-flow check (P3.18) |
| Synthetic prices/commitments mistaken for market data | "Synthetic" label in every record and display contract (P3.14, P3.16) |
| Non-deterministic solves | Pinned versions/seeds/thread counts; replay tests (P3.19) |
| Mutating schedules breaks replay | Append-only revisions (P3.22) |
| D1/D2/D3 still OPEN | Defaults recorded as recommendations, never as decisions (DECISIONS.md) |

## 9. What must NOT be built yet

- Any grid feasibility or power-flow validation (Phase 4).
- Any command, approval, signature, or dispatch path (Phase 4).
- Any console UI (Phase 5).
- Tier 3/4 solvers, GNN/MARL/ADMM (Stage 8).
- Real market adapters or price feeds (D1 OPEN).
- A second general-purpose backend in Python (§7.2).

## 10. How the §24 vertical slice lands here

§24 steps 3–4 land in this phase: "calculate each resource's available flexibility over a defined 24-hour horizon" (P3.12, P3.26 scenario a) and "optimize a simple portfolio schedule using a solver appropriate to the chosen LP/MILP/QP formulation" (P3.17, P3.24). Steps 1–2 are Phase 2 inputs; steps 5–8 are Phase 4; steps 9–10 are Phase 5. The slice is not complete at the end of Phase 3, and this plan does not claim otherwise.
