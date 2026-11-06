# Phase 8 — Advanced Coordination Research: Context

| Field | Value |
|---|---|
| Phase | 8 — Stage 8 Advanced coordination research |
| Report trace | §23 Stage 8; §13; §19; §2.1 (v1.x / v2.x) |
| Release | v1.x — Advanced coordination; v2.x — Advanced grid intelligence |
| Control status | Research only. Simulation / shadow. Proposals only (ADR-010). |

## 1. Background

Advanced AI is an extension layer, not a required dependency for safe
baseline operation (§13). The deterministic baseline — LP/MILP/QP scheduling
plus independent AC power-flow feasibility validation (ADR-009) — is the
product. Phase 8 asks whether three research directions can improve a
measured bottleneck in that baseline, and answers with experiments against
strong deterministic baselines, independently validated:

- **GNN grid intelligence (§13.1):** candidate AC-OPF warm-start generation,
  active-constraint prediction, contingency ranking/screening, topology
  embedding and transfer learning, surrogate estimates with uncertainty.
- **Distributed MPC / ADMM (§13.2):** decomposable local problems with
  explicit coupling constraints, solved by consensus/boundary variables.
- **Safe MARL (§13.3):** centralized training with decentralized execution
  (CTDE), in simulation/shadow only, with a safety-projection layer.

The report's release ladder places this work at v1.x (DMPC/ADMM, expanded
market adapters, topology-aware optimization research, portfolio analytics)
and v2.x (GNN acceleration, safe MARL research, dynamic stability research,
broader utility/DSO integrations) — both with per-feature qualification and
shadow/simulation first, no automatic field authority by default (§2.1).

The phase's governing honesty rules, all from the report:

1. A GNN output must pass the same physical validation as any other
   candidate, and must be compared with a non-GNN baseline on unseen
   topologies and operating regimes. Report end-to-end feasible solve time,
   not just model inference latency (§13.1).
2. Classical ADMM guarantees rely on convexity and stated assumptions. For
   nonconvex AC-OPF or mixed-integer local problems, do not claim global
   convergence or global optimality without a specific theorem and
   experiment (§13.2).
3. MARL runs only in the simulator/shadow environment in the initial
   research releases. A control-barrier-function or safety-projection layer
   must be derived from a valid system model and the correct relative
   degree; voltage and branch-current constraints are not automatically
   simple dynamic state constraints. A filter can be infeasible — define
   that outcome and its fallback. A finite test set with zero observed
   constraint violations is not a universal safety guarantee. Never allow a
   research MARL worker to issue live device commands (§13.3).
4. Research plugins return proposals (schedule, candidate setpoint vector,
   ranking/score, forecast distribution, uncertainty estimate, diagnostics).
   They do not return an authorization token. The protected validation and
   authorization path remains mandatory (§13.4, ADR-010).
5. Do not use a steady-state power-flow tool as evidence for transient
   frequency stability. Do not use HELICS itself as the power-system physics
   model (§19.1). Do not claim first-feeder tests establish capability for
   a 10,000-bus utility network (§19.3). Do not set arbitrary universal
   targets such as "zero violations proves safety" or "ADMM converges in 30
   iterations" (§19.4).

## 2. Key report sections to re-read

| Section | Why it matters here |
|---|---|
| §13.1–§13.4 | The three research modules and the research plugin boundary — the core of this phase. |
| §12.1–§12.6 | Scheduling objective, optimization tiers, grid-model provider selection, feasibility gate, result contract — the baseline and the gate research proposals pass through. |
| §11 | Capability models (what a proposal must respect: P/Q, SOC, ramp, duration, recovery). |
| §14.1–§14.5 | Command lifecycle (research proposals enter at PROPOSED), invariants, per-asset fallback, dispatch is not hard real-time. |
| §19.1–§19.4 | Simulation strategy (multi-fidelity tracks), test layers, benchmark cases, evaluation metrics — the experimental method of this phase. |
| §5.3, §5.8 | Research-algorithm plugin category; `research-gnn` / `research-marl` profiles (no dispatch authority). |
| §5.4 Class C, §7.4 | Out-of-process Python workers; PyTorch + PyG, RLlib, ANDES, HELICS, solver selection rule. |
| §9.2 | Plugin result requirements — provenance and diagnostics are eligibility conditions. |
| §10.2, §10.5 | Units/signs; Protobuf governance (`buf lint`, `buf breaking`). |
| §17.4–§17.5 | Threat model (compromised plugin, malicious telemetry); safety invariants (esp. rules 7, 9, 10). |
| §20.1, §20.2 | Simulation Lab workspace; proposed vs executed display. |
| §22.1 (ADR-009, ADR-010), §22.2–§22.3 | Baseline and proposal-only ADRs; CI gates; release gates (optimizer outputs independently validated). |
| §23 Stage 8 | Deliverables and exit gate (quoted in CHECKLIST.md). |
| §24 | Vertical slice — not in this phase; the slice is the control arm (see PLAN.md §10). |
| §25.2 | Deferred decisions; D3 (network class) and D4 (license audit) gate this phase's experiments. |
| §27 [R16], [R24], [R28] | HELICS; ANDES; HiGHS. |

## 3. Canonical units and sign conventions (§10.2)

- MW, MVAr, MVA, MWh in persisted values and API contracts; UI may display
  kW/kWh.
- `P > 0` = export/injection into the grid at the point of connection;
  `P < 0` = import/consumption.
- Reactive-power sign is declared per grid model and device profile, never
  assumed.
- SOC is a fraction in [0,1] at the API boundary.
- Research outputs must carry units with their values: a warm-start vector,
  a ranking, or a setpoint without units, horizon, and snapshot reference is
  not a valid research result (§9.2, P8.04).

## 4. Telemetry quality (§10.4)

Exactly one per value: `GOOD` / `SUSPECT` / `BAD` / `MISSING` / `ESTIMATED` /
`STALE`. Research experiments must state which quality states their training
and evaluation data contain, and must not train on `BAD` data presented as
`GOOD` or evaluate on `ESTIMATED` data presented as measured. A proposal
derived from `STALE`/`MISSING` inputs must carry that provenance into its
result.

## 5. Terminology

| Term | Meaning in this phase |
|---|---|
| GNN | Graph Neural Network — here, a model over the network topology (buses as nodes, branches as edges) used for warm starts, constraint screening, or contingency ranking. |
| DMPC | Distributed Model Predictive Control — MPC decomposed into local subproblems coordinated through coupling constraints. |
| ADMM | Alternating Direction Method of Multipliers — a distributed optimization algorithm with classical convergence guarantees under convexity and stated assumptions; a heuristic (not a guaranteed method) on nonconvex/mixed-integer problems. |
| MARL | Multi-Agent Reinforcement Learning — multiple learning agents (per site/asset) trained together, here in simulation/shadow only. |
| CTDE | Centralized Training, Decentralized Execution — the §13.3 research baseline for MARL. |
| Warm start | An initial guess given to a solver. The solver still solves; the warm start's value is measured by end-to-end feasible solve time, not by inference latency. |
| Screening | Predicting which constraints/contingencies matter, so the solver can prioritize them. A screened-out constraint that was actually active is a miss with a cost. |
| Safety filter / safety projection | A layer that projects a proposed (e.g. MARL) action onto a safe set derived from a system model. It can be infeasible — that outcome must be defined. |
| Shadow mode | Running a research policy alongside (not instead of) the operating baseline, logging what it would have done, without issuing anything (§19.2(9)). |
| Primal/dual residual | ADMM convergence measures: primal residual (coupling-constraint violation), dual residual (change in dual variables). Reported per run (§19.4). |
| End-to-end feasible solve time | Wall time from problem input to a validated feasible solution — solver + validation — the §13.1 headline metric for GNN warm starts. |
| Held-out topology/regime | A network topology or operating regime excluded from training and used only for evaluation, to measure generalization. |
| Per-feature qualification | The §2.1 rule that each v1.x/v2.x advanced feature is qualified individually before any field use; qualification is not granted by this phase. |
| ANDES | An open-source Python package for power-system dynamic simulation (power flow, time-domain/transient stability, eigenvalue analysis). GPLv3-licensed — see RESEARCH.md. |
| HELICS | A co-simulation framework that synchronizes separate simulator federates across time domains. A coordinator, not a physics model. |

Use "license" (US spelling) and "acknowledgement" (per `.agent/WRITING.md`).
"Validation", "feasibility", and "authorization" are different gates.
AC power flow is not AC-OPF; IPOPT is local, not global; HiGHS is LP/QP/MIP,
not SOCP; Clarabel is convex conic, not MISOCP.

## 6. Constraints

- **Research only; simulation/shadow; proposals only (ADR-010).** No field
  authority, no automatic field authority by default (§2.1 v2.x).
- **No safety/certification/protection claims** (AGENTS.md rules 2–3).
  This phase produces evidence, not qualification.
- **No hallucinated facts** (AGENTS.md rule 1): framework versions,
  solver capabilities, and literature claims must trace to primary sources
  or be labelled ASSUMPTION. See RESEARCH.md — including the PyTorch/PyG
  version-skew finding, which must be resolved by pinning, not assumed away.
- **Solver-class and ADMM-convexity discipline** (§7.4, §13.2, AGENTS.md
  rule 4).
- **Minimal, surgical edits; verify before claiming done** (AGENTS.md
  rules 6–7). Do not edit `docs/foundations/*` or the report in this phase.
- **No arbitrary Rust dylib loading** (§5.4, ADR-006) — research workers
  are gRPC workers (Class C) or WASM components, never dylibs.

## 7. Prior-phase outputs consumed

Phases 1–5 per §23 Stages 1–5 (workspace, telemetry, flexibility,
deterministic scheduler, feasibility, command lifecycle, console,
simulator, §19.3 benchmark cases). Phase 6 (edge agent, OpenEMS, one
protocol adapter; research uses simulated/site-lab data only). Phase 7
(plugin SDK: manifests, WIT worlds, Wasmtime host, Python gRPC worker
template, conformance suite, `research-gnn`/`research-marl` profiles with
no dispatch authority, and the P7.31 handover). P8.01 verifies the baseline
benchmark results, the research profiles, and the worker contract before
research design starts. Prior-phase ledgers exist in this repo under
`ledger/phase-0*/` through `ledger/phase-07-plugin-sdk/`; where a prior
ledger's exit-gate evidence contradicts an assumption in this ledger, P8.01
must be re-run against the ledger, not only the report.

## 8. D1–D6 relevance

| Gate | Status at ledger writing | Relevance to Phase 8 |
|---|---|---|
| D1 Target jurisdiction/market | OPEN | Bounds market-adapter research; does not gate the experiments. |
| D2 First portfolio/service | OPEN | Bounds the asset mix the experiments model; record the assumed mix per experiment. |
| **D3 Network class/model fidelity** | **OPEN** | **Directly in scope: each experiment must state its network class (balanced radial default; unbalanced three-phase; meshed transmission out of scope unless explicitly approved). A GNN/ADMM result on one class is not evidence for another.** |
| **D4 License/dependency audit** | **OPEN** | **Directly in scope: P8.03 audits the research stack. ANDES is GPLv3 — isolation design + legal confirmation required before use.** |
| D5 First protocol/asset mix | OPEN (before v0.4.0) | Belongs to Phase 6; research uses simulated/site-lab data and does not select protocols. |
| D6 Cyber/OT boundary review | OPEN (before any read-only pilot) | Belongs to Phase 6/Stage 9; research runs in simulation/shadow and does not touch the OT boundary. |

## 9. Open questions

1. Which network class does each experiment cover? (D3 — P8.02/P8.07/P8.10
   must state it per module.)
2. Which exact PyTorch/PyG pair is pinned, given the version skew?
   (P8.03; RESEARCH.md R8-C3.)
3. Which RLlib API stack (new RLModule/Learner vs old ModelV2/Policy) and
   which Ray version? (P8.13; RESEARCH.md.)
4. Which ANDES models and test cases, and is the GPLv3 isolation design
   confirmed by legal? (P8.03/P8.16.)
5. What are the pre-registered success thresholds per module? (P8.02 —
   set after defining model/service/benchmark/risk, §19.4.)
6. Which disturbance set defines the dynamic evaluation? (P8.17.)
7. What would per-feature qualification require for each module, and who
   decides it? (P8.29 — a later decision, recorded here as not granted.)
