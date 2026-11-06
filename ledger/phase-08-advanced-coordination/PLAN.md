# Phase 8 — Advanced Coordination Research: Plan

| Field | Value |
|---|---|
| Phase | 8 — Stage 8 Advanced coordination research |
| Report trace | §23 Stage 8; §13; §19; §2.1 (v1.x / v2.x) |
| Release | v1.x — Advanced coordination; v2.x — Advanced grid intelligence |
| Control status | Research only. Simulation / shadow. Proposals only (ADR-010). |

## 1. Scope

### In scope

- ADMM/DMPC provider with clear convexity assumptions, residuals,
  termination, infeasibility handling, communication-failure handling, and
  centralized fallback (§13.2).
- GNN warm-start/screening experiment: candidate AC-OPF warm starts and
  active-constraint/contingency screening, evaluated on unseen topologies
  by end-to-end feasible solve time (§13.1).
- MARL simulation/shadow worker: CTDE baseline, safety-projection layer
  with a defined infeasible-filter outcome, shadow action logging, evaluated
  under held-out regimes, outages, delays, and uncertainty (§13.3).
- Dynamic simulator integration for frequency/stability research (ANDES:
  time-domain simulation and eigenvalue analysis) and dynamic evaluation of
  research controllers under disturbances (§19.1).
- Co-simulation integration (HELICS) for multi-domain interaction (§19.1).
- Benchmark cases extended per §19.3, frozen deterministic baselines,
  an independent validation harness, reproducibility (seeds/versions/
  artifacts/rerun test), a cross-module comparison, and the benchmark
  paper/report (§23 Stage 8).
- Portfolio analytics research as reporting-only plugins (§5.3, §19.4).
- Per-feature qualification record stating what field use would require —
  and that this phase grants none of it (§2.1, §22.3).

### Non-goals (explicitly out of this phase)

- Field authority for any research module. v1.x/v2.x are per-feature
  qualification; shadow/simulation first; no automatic field authority by
  default (§2.1). Qualification itself is Stage 9 work, recorded in P8.29
  as not granted.
- Any claim of global convergence/optimality for nonconvex AC-OPF or
  mixed-integer local problems without a specific theorem and experiment
  (§13.2).
- Any safety guarantee from a finite test set (§13.3). Zero observed
  violations is a test outcome, not a guarantee.
- A research MARL worker issuing live device commands (§13.3) — prohibited,
  not deferred.
- Using a steady-state power-flow tool as evidence for transient frequency
  stability, or HELICS as the power-system physics model (§19.1).
- Claims that feeder-scale tests establish capability for a 10,000-bus
  utility network (§19.3).
- Productizing any research module (operator workflow, deployment model,
  upgrade path) — that is v1.0.0/Stage 9 work, not research work.
- New plugin SDK contracts beyond what Phase 7 defined. Research workers
  reuse the Phase 7 gRPC worker template and contracts (P8.04); if a
  contract gap is found, it is recorded as a Phase 7 follow-up, not fixed
  by inventing a parallel contract here.

## 2. Architecture decisions in play

| Decision | Report | Position in this phase |
|---|---|---|
| ADR-010 — ML/RL outputs are proposals only, cannot directly dispatch | §22.1, §13.4 | The phase's governing decision. Enforced in the worker response types (P8.04), not only in docs. |
| ADR-009 — Deterministic optimizer + independent feasibility validation is the baseline | §22.1 | The baseline every module is compared against (P8.06) and the gate every proposal passes through (P8.05). |
| ADR-004 — Python isolated to scientific/numerical/research workers | §22.1, §7.2 | GNN/MARL/ANDES/HELICS work lives in isolated Python workers (Class C); no Python in the control plane. |
| ADR-005 — Protected kernel non-replaceable | §22.1, §5.1 | Research plugins cannot touch authorization, tenant isolation, audit, lifecycle, or final dispatch validation. |
| ADR-012 — No hard-real-time or protection claims | §22.1 | Dynamic-simulation results are evidence about a declared model, not about the real grid's protection behaviour. |
| D3 — Network class and model fidelity | §25.2 | **Open gate.** Each experiment must state which network class it covers (balanced radial default; unbalanced/meshed only if explicitly scoped). |
| D4 — License and dependency audit | §25.2 | P8.03 audits the research stack; ANDES GPLv3 isolation is the headline item. |

## 3. Component breakdown

| Component | Location (per §8.1) | Owns | Does not own |
|---|---|---|---|
| ADMM/DMPC provider | `python/workers/` (Class C gRPC worker) behind the `Optimizer` contract | Decomposable local problems, coupling, residuals, fallback | Convergence claims beyond stated assumptions; dispatch authority |
| GNN worker | `python/workers/gnn-research/` | Training (offline), isolated inference, warm-start candidates, screening rankings + uncertainty | The solver (still solves); dispatch authority |
| MARL worker | `python/workers/marl-research/` | CTDE training (offline), shadow inference, safety filter, shadow action log | Live dispatch (no code path); safety guarantees |
| Dynamic simulator worker | `python/workers/` (ANDES integration) | Time-domain simulation, eigenvalue analysis, disturbance cases | Quasi-static power-flow feasibility (separate provider) |
| Co-simulation federation | HELICS federation | Time synchronization across federates | Any federate's physics |
| Independent validation harness | Separate from research workers | Physical re-validation of every research proposal | The research modules themselves |
| Benchmark set + baselines | `benchmarks/`, `scenarios/` | Versioned cases, frozen baseline results | Research results (separate artifacts) |
| Benchmark paper/report | `docs/` (research output) | Methods, results, limits, reproducibility | Product claims |

Dependency direction (§6.2, §8.2): research workers communicate through
Protobuf/gRPC or scenario artifacts; they do not import internal database
tables (§8.2). The validation harness is owned separately from the modules
it validates (P8.05).

## 4. Data contracts

| Contract | Form | Notes |
|---|---|---|
| Research result (base) | Protobuf, extending §9.2 | Plugin/model/solver version, input snapshot ID + timestamp, horizon + resolution, problem class + assumptions, termination status, objective + units, residuals, feasibility diagnostics, warnings/unmodelled constraints, execution time/resource use, reproducibility metadata (seed/config/artifact ref). |
| ADMM result (extension) | Protobuf | Primal/dual residuals per iteration, iteration count, termination reason, infeasibility flag, fallback flag + fallback source. |
| GNN result (extension) | Protobuf | Model version, training-data reference, topology scope, warm-start candidate or ranking + scores, uncertainty estimate. |
| MARL result (extension) | Protobuf | Policy version, training seed, environment version, proposed action, safety-filter outcome (incl. infeasibility), shadow log reference. |
| Dynamic result | Scenario artifact | Case/model versions, disturbance parameters, solver settings, frequency/voltage traces, stability outcome — every number carrying its model reference. |
| Proposal (all modules) | §13.4 types only | Proposed schedule, candidate setpoint vector, ranking/score, forecast distribution, uncertainty estimate, diagnostics. Never an authorization token. |
| Benchmark set | Versioned artifact | §19.3 cases + research additions + not-tested list; every evaluation cites the version. |

## 5. Interfaces

- **Research worker → control plane:** gRPC/Protobuf (Phase 7 Class C
  template). Proposals only.
- **Research worker → validation harness:** proposals + input snapshots;
  the harness re-checks capability, AC power flow, SOC/ramp/duration, and
  command invariants — independently of the worker.
- **GNN/MARL/ADMM → solver:** GNN warm-starts the solver; it does not
  replace it. ADMM is an `Optimizer` provider behind the contract.
- **Dynamic simulator → research evaluation:** time-domain/eigenvalue
  results as scenario artifacts; never as dispatch inputs.
- **Console → research:** Simulation Lab workspace shows proposals with
  provenance and validation outcomes, labelled as proposals (§20.1–§20.2).

## 6. Sequencing

1. **Readiness + scope + audit + contracts (P8.01–P8.05):** verify the
   baseline/profiles/worker template, freeze research questions, audit
   licenses (ANDES GPLv3 isolation), define worker contracts, build the
   independent harness.
2. **Baselines frozen (P8.06):** strong deterministic + rule-based baselines
   run and frozen before any research comparison.
3. **Modules, one at a time (P8.07–P8.09 ADMM; P8.10–P8.12 GNN;
   P8.13–P8.15 MARL):** formulation/design → implementation → evaluation
   vs the frozen baselines through the independent harness.
4. **Dynamics + co-simulation (P8.16–P8.18):** ANDES integration, dynamic
   evaluation, HELICS federation.
5. **Synthesis (P8.19–P8.23):** benchmark set, reproducibility, transfer/
   scaling, analytics, cross-module comparison.
6. **Assurance + reporting (P8.24–P8.30):** drill, security review,
   benchmark paper/report, console surfacing, CI, qualification record,
   retrospective.

Relative effort (no calendar dates): the evaluations (P8.09/P8.12/P8.15/
P8.17) and the benchmark paper (P8.26) are the bulk of the phase — the
experiments are the product here, not the code. The ADMM formulation work
(P8.07) and the MARL safety-filter derivation (P8.13) are the highest-risk
intellectual work and gate their implementations. The independent harness
(P8.05) is small in code and decisive in credibility: without it, the exit
gate's "independently validated" cannot be evidenced.

## 7. Roles

| Role | Owns |
|---|---|
| Research lead | Scope freeze, baselines, cross-module comparison, benchmark paper, retrospective |
| Research engineer (optimization) | ADMM/DMPC formulation, implementation, evaluation |
| Research engineer (ML) | GNN design, implementation, evaluation, transfer/scaling |
| Research engineer (RL) | MARL design, shadow worker, evaluation |
| Research engineer (power systems) | Dynamic simulator integration, dynamic evaluation, HELICS, independent validation harness ownership |
| Power-systems reviewer | Harness ownership, safety-filter derivation review, baseline review |
| Security engineer | Research-worker security review |
| Legal + research lead | D4 research-stack audit (notably ANDES GPLv3) |
| Frontend engineer | Research/shadow console surfacing |
| DevOps | CI gates for research code |
| Product | Per-feature qualification record routing (decision is later, not this phase) |

## 8. Risks and mitigations

| Risk | Mitigation |
|---|---|
| Baseline too weak, making research look good | P8.06 requires strong deterministic + rule-based baselines, frozen before comparison; assumptions written down. |
| Researcher validates their own output | P8.05 separate harness, separate owner, deliberately-wrong test cases. |
| ADMM claimed convergent on nonconvex/MI problems | P8.07 assumptions note (reviewer-signed); P8.09 reporting rules; RESEARCH.md literature. |
| GNN generalization overstated | P8.10 held-out topologies + leakage controls; P8.12 end-to-end feasible solve time as the headline metric. |
| MARL safety filter wrong or infeasible | P8.13 derivation review before training; infeasible-filter outcome defined; P8.15 filter statistics. |
| Finite test set presented as a safety guarantee | P8.15/P8.26 state the limitation in plain language; P8.29 qualification record. |
| ANDES GPLv3 contaminating the core | P8.03 isolation design, legal confirmation; worker-boundary inspection (P8.16). |
| PyTorch/PyG version skew breaking the stack | P8.03 verified-compatible pin; RESEARCH.md R8-C3. |
| Results unreproducible | P8.20 reproducibility records + rerun test by a second engineer; non-reproducing results excluded. |
| Dynamic results mistaken for real-grid evidence | P8.17 model-limits note on every number; §19.1 boundary statements. |

## 9. What must NOT be built yet

- Field authority or per-feature qualification for any research module
  (Stage 9; P8.29 records the requirement, not the grant).
- A MARL worker with any path to live dispatch.
- A safety filter derived from an invalid model or wrong relative degree
  (training is blocked until the derivation is reviewed — P8.13).
- Product features built on research outputs (operator workflows, deployment
  models, upgrade paths for GNN/MARL/ADMM).
- New SDK contracts (Phase 7 follow-up if a gap is found).
- Claims of global optimality, safety, or scale beyond the tested cases.

## 10. How the §24 vertical slice lands here

**Not in this phase at all.** The §24 slice lands across v0.1.0–v0.3.0
(Phases 1–5). Phase 8 does not re-build the slice, extend it, or add a step
to it.

What Phase 8 does with the slice, and only this:

- The slice's deterministic scheduler (step 4) and feasibility validation
  (step 5) are the **frozen baselines** (P8.06) that every research module
  is compared against — the slice is the control arm of the experiments.
- Research proposals are evaluated as **alternative candidates for step 6's
  PROPOSED command** in simulation/shadow only: they enter at `PROPOSED`
  and pass the same validation → feasibility → authorization gates; the
  slice's dispatch target remains the simulator adapter.
- Step 10's replay must now also reproduce research runs from their
  recorded seeds/versions/artifacts (P8.20) — the slice's reproducibility
  requirement extends to the research workers.

If the deterministic baseline regresses because of Phase 8 work, the phase
has failed its readiness obligation (P8.01) regardless of what the research
modules achieve.
