# P0.23 — Simulation, testing, and benchmark strategy freeze (as a plan, not a suite)

| Field | Value |
|---|---|
| Subtask | P0.23 — Freeze the simulation, testing, and benchmark strategy (as a plan, not a suite) |
| Report trace | §19.1–§19.4 (simulation tracks, test layers, benchmark cases, evaluation metrics), §12.4 (grid-model provider selection and tool boundaries), §24 step 10 (reproducibility replay), §23 Stage 9 (HIL placement) |
| Source | `docs/plan/Neryva_FlexCore_Product_and_Architecture_Report_v1.1.md` (product baseline; foundations specification governs on conflict per AGENTS.md §2) |
| Inputs | P0.05 (network class/fidelity, D3), P0.12 (grid-model contract), P0.13 (solver-class discipline) — per ledger P0.23 dependencies; broader inputs P0.02, P0.10–P0.15, P0.19 |
| Status | Draft for Stage 0 review |
| Scope limit | This freezes a **strategy, not a test suite**: no benchmark is run and no threshold is set in Phase 0. No result may be quoted as a safety proof; no certification or protection-grade claim is made (AGENTS.md §3). |

---

## 1. Purpose

Freeze the test-layer list, simulation tracks, benchmark cases, and the no-arbitrary-targets rule, so later phases have a test strategy to implement (ledger P0.23). Thresholds are set per benchmark in later phases, never in Phase 0 (ledger risk/mitigation note).

## 2. Simulation tracks (§19.1 — all six)

Multi-fidelity test environment (§19.1):

| Track | Tools | Questions answered |
|---|---|---|
| Portfolio scheduling | Synthetic asset models + optimizer | Does schedule respect SOC, ramp, duration, and portfolio commitments? |
| Distribution quasi-static | pandapower, GridLAB-D, OpenDSS | Are voltage/current/transformer constraints respected? |
| Transmission/planning | PyPSA and selected benchmark cases | How does portfolio behaviour affect higher-level system studies? |
| Dynamic stability | ANDES / selected dynamic simulator | Does a proposed research controller affect frequency/transient behaviour in the declared model? |
| Multi-domain co-simulation | HELICS | How do grid, communication, market, and end-use simulators interact across time? |
| Hardware-in-the-loop | Vendor/partner HIL platform | Does command/control integration behave correctly with real controller hardware? |

### Tool-boundary rules (§19.1; §12.4)

1. **Do not use a steady-state power-flow tool as evidence for transient frequency stability** (§19.1). Consistent with §12.4: GridLAB-D is for distribution and time-series studies, not automatically a fast transient stability engine; ANDES serves the separate dynamic-stability validation track; PyPSA is not the sole distribution device-control validator.
2. **HELICS coordinates federates; it does not replace their physics models** (§19.1; §12.4, [R16]). It is the co-simulation orchestration layer, not a power-system model.

Per §12.4, the initial implementation chooses one primary deterministic power-flow path and adds other providers only when a concrete test case requires them; each tool's model and solver limitations are verified per case (§12.4, [R20]–[R24]). Solver-class discipline (P0.13) applies to any optimizer used in the portfolio-scheduling track.

## 3. Test layers (§19.2 — all ten)

| # | Layer | Scope (§19.2) |
|---|---|---|
| 1 | Unit tests | Units, sign conventions, asset constraints, state transitions, solver input/output validation |
| 2 | Property tests | Invariant preservation across generated asset states, timestamps, command sequences, and constraints |
| 3 | Contract tests | Every adapter and plugin passes the same interface suite |
| 4 | Integration tests | PostgreSQL, NATS, edge, plugin host, Python workers, and external simulators |
| 5 | Deterministic scenario tests | Pinned inputs, solver versions, random seeds, and expected outcomes/tolerances |
| 6 | Fault injection | Dropped/reordered/duplicated messages, clock skew, stale telemetry, unavailable assets, solver timeout, plugin crash, partial fleet response |
| 7 | Security tests | Tenant isolation, auth bypass, replay, command forgery, plugin privilege escalation, secrets exposure |
| 8 | Performance/load tests | Telemetry ingest rate, scheduling runtime, query latency, command fan-out, recovery |
| 9 | Shadow-mode evaluation | Compare proposed actions to existing operator/site behaviour without issuing commands |
| 10 | HIL / commissioning tests | Only after simulation, integration, and cyber reviews pass |

**HIL is not a Phase 0–2 activity** (ledger P0.23 risk note): it is layer 10 of 10, gated behind simulation, integration, and cyber reviews (§19.2 item 10), and HIL and field acceptance tests sit in the §23 roadmap at Stage 9, after site integration (Stage 6), plugin SDK (Stage 7), and customer pilot qualification (Stage 9 entry conditions). Phase 0 freezes the list; Phases 1–2 implement unit, property, contract, integration, deterministic scenario, fault-injection, security, and performance layers, with shadow-mode evaluation following v0.3.0's shadow capability (§2.1).

## 4. Benchmark cases (§19.3)

Start with a small set of recognised cases and add scale gradually (§19.3):

| Case set | Detail | Condition / caveat |
|---|---|---|
| IEEE 13-bus, 34-bus, 123-bus distribution test feeders | First distribution studies | Only where licensing and model availability are confirmed (§19.3) |
| IEEE 39-bus, 118-bus transmission test systems | Selected grid studies | Not a distribution device-control validator role (§12.4 PyPSA boundary) |
| Synthetic portfolios | BESS, PV, flexible load, EVSE | Deterministic, reproducible inputs (§24 step 10) |
| Multi-feeder/multi-site | Multiple feeders/sites connected through a common substation constraint | Tests portfolio-vs-network interaction (§19.3) |
| Stressed scenarios | PV ramps, forecast error, communication loss, asset dropout, reverse power flow, topology change | Maps to the §19.2 fault-injection layer and §12.5 feasibility-gate failure reporting |

**Scale rule (§19.3):** do not claim that the first distribution feeder tests establish capability for a 10,000-bus utility network.

**Reproducibility rule (§24 step 10):** replay the same scenario with the same versions and verify reproducibility; deterministic scenario tests pin inputs, solver versions, random seeds, and expected outcomes/tolerances (§19.2 item 5). Simulator benchmark results must be reproducible for the §22.3 release gates.

## 5. Evaluation metrics (§19.4 — full table)

| Area | Metrics |
|---|---|
| Scheduling | Objective value, schedule deviation, feasible interval count, runtime |
| Asset model | SOC/power violations, ramp violations, availability prediction error |
| Grid feasibility | Voltage violations, branch/transformer overload, AC power-flow convergence, residuals |
| Flexibility | Available vs delivered MW/MWh, uncertainty calibration, shortfall |
| Dispatch | Acknowledgement latency, tracking error, failure/expiry rate, recovery time |
| ADMM | Primal/dual residual, objective gap where meaningful, iteration count, time, communication sensitivity |
| GNN | Feasible solver success, end-to-end time, speedup, generalization to unseen topologies |
| MARL | Constraint violation frequency, return, robustness under distribution shift, baseline comparison |
| Platform | Availability, throughput, p95/p99 API/ingest latency, recovery, tenant isolation |

## 6. Threshold rule (§19.4)

**Do not set arbitrary universal targets** such as "zero violations in 1,000 cases proves safety" or "ADMM converges in 30 iterations" (§19.4). **Acceptance thresholds are set after defining the model, the service, the benchmark, and the operational risk** (§19.4).

Consequences for later phases:

- No Phase 0 artefact states a numeric pass/fail target for any metric area in §5.
- A benchmark result is evidence for a declared model and scenario, never a safety proof (§19.4; ledger risk note).
- Threshold-setting is a per-benchmark activity in later phases, tied to the §22.3 release gates and the deployment's operational risk review.

## 7. What this document does not do

- It runs no test and builds no suite; Phase 0 freezes the strategy only (ledger subtask title).
- It selects no grid-model provider beyond the §12.4 recommendation to choose one primary deterministic path; provider selection follows D3 and later phases (P0.12).
- It sets no acceptance threshold (§6).
- It makes no safety, certification, or protection-grade claim; dynamic-stability and HIL tracks are research/qualification tracks, and §0.3 non-promises apply (no hard real-time, no protection-grade control).

## 8. Acceptance-criteria coverage (ledger P0.23)

| Ledger acceptance criterion | Where satisfied |
|---|---|
| All six tracks, ten test layers, and benchmark cases listed | §2, §3, §4 |
| The two tool-boundary rules and the 10,000-bus scale rule stated | §2 (tool boundaries), §4 (scale rule) |
| The metrics table is reproduced | §5 (all nine areas) |
| The no-arbitrary-targets rule is stated | §6 |

Additional ledger steps covered: HIL placement (not Phase 0–2; §3 and §4), reproducibility rule §24 step 10 (§4), §12.4 tool boundaries and one-primary-path rule (§2), threshold-consequence rules (§6). Verification method (ledger): comparison against §19.1–§19.4.
