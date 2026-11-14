# Phase 8 — Advanced Coordination Research: Ledger

| Field | Value |
|---|---|
| Phase | 8 — Stage 8 Advanced coordination research |
| Report trace | §23 Stage 8; §13; §19; §2.1 (v1.x / v2.x) |
| Release | v1.x — Advanced coordination; v2.x — Advanced grid intelligence |
| Control status | **Research only. Simulation / shadow. ML/RL outputs are proposals
  only and cannot directly dispatch (ADR-010).** No research module gets
  field authority by default; per-feature qualification is required (§2.1). |
| Status | Ledger only — no code written, no decisions closed |
| Research cut-off | 7 November 2026 (report §27) |

## How to read this ledger

- Every subtask has an ID (`P8.01` …), a report trace (`§N` / `§N.M`),
  inputs, detailed steps, a concrete deliverable, testable acceptance
  criteria, an owner role, dependencies, a verification method, and risks.
  **No subtask is done until its acceptance criteria are evidenced.**
- Checkboxes `- [ ]` mark work not yet done. Tick a box only when the named
  deliverable exists and the verification method has been run and recorded.
- Canonical units and signs (§10.2): MW, MVAr, MVA, MWh; `P > 0` = export
  at the point of connection; reactive-power sign declared per grid model
  and device profile, never assumed; SOC is a fraction in [0,1] at the API
  boundary. Telemetry quality is exactly one of
  `GOOD` / `SUSPECT` / `BAD` / `MISSING` / `ESTIMATED` / `STALE` (§10.4).
- Command lifecycle (§14.1): `PROPOSED → VALIDATED → FEASIBLE → APPROVED →
  ISSUED → ACKNOWLEDGED → TRACKING → COMPLETED`, plus `REJECTED`, `FAILED`,
  `EXPIRED`, `CANCELLED`, `SUPERSEDED`, `FALLBACK`. A research proposal
  enters at `PROPOSED` and only through the protected path.
- **The honesty boundary (§13, ADR-010):** advanced AI is an extension
  layer, not a required dependency for safe baseline operation. GNN/DMPC/
  MARL outputs are proposals. They do not return an authorization token.
  The protected validation and authorization path remains mandatory (§13.4).
  A finite test set with zero observed constraint violations is **not** a
  universal safety guarantee (§13.3). Never allow a research MARL worker to
  issue live device commands (§13.3).
- **Solver-class discipline (AGENTS.md rule 4; §7.4):** HiGHS LP/QP/MIP not
  SOCP; Clarabel convex conic not MISOCP; AC power flow ≠ AC-OPF; IPOPT
  local not global. ADMM convergence guarantees rely on convexity and
  stated assumptions (§13.2) — for nonconvex AC-OPF or mixed-integer local
  problems, do not claim global convergence or global optimality without a
  specific theorem and experiment.
- This phase does **not** certify anything, does not qualify any feature for
  field use, and does not establish safety. Per-feature qualification
  (§2.1) is a separate, later decision.
- Open gates: D3 (network class/model fidelity — research must state which
  class each experiment covers) and D4 (license audit — ANDES is GPLv3, see
  RESEARCH.md R8-C2; PyTorch/PyG/RLlib licensing must be audited in P8.03).

## Prior-phase outputs consumed (per §23 Stages 1–7)

- Phases 1–5: workspace, API, telemetry, flexibility, deterministic
  scheduler, grid feasibility, command lifecycle, console, simulator,
  benchmarks cases (§19.3: IEEE 13/34/123-bus feeders, 39/118-bus systems,
  synthetic portfolios).
- Phase 6: edge agent, OpenEMS integration, one protocol adapter (research
  uses simulated/site-lab data only; the edge command path stays disabled
  by default).
- Phase 7: plugin SDK — manifests, WIT worlds, Wasmtime host, Python gRPC
  worker template, conformance suite, `research-gnn` / `research-marl`
  profiles (no dispatch authority), and the P7.31 handover listing the
  reusable worker contracts and the proposal-only boundary. If P7.31 was
  not delivered, P8.04 must fail and this phase must not define research
  providers against assumed contracts.

---

## P8.01 — Phase readiness check: Phases 1–7 outputs and the P7.31 handover

- [x] **P8.01 — Phase readiness check: Phases 1–7 outputs and the P7.31 handover** — implemented, awaiting human build/test (decision record: `ledger/phase-08-advanced-coordination/01_readiness_check.md`)
  - **Objective:** Verify, by running the checks, that the deterministic
    baseline, the benchmark cases, and the Phase 7 research profiles/worker
    contracts this phase will compare against actually exist and pass.
  - **Report trace:** §23 Stages 1–7; §19.3; §5.8; AGENTS.md §6–§7.
  - **Inputs / prerequisites:** Phase 1–7 ledgers and exit-gate evidence;
    P7.31 handover note; repo at the Phase 7 state.
  - **Steps:**
    1. Run `python tools/verify_docs.py` and `python tools/validate_agent.py`; record actual output.
    2. Run the workspace test suite and the §19.3 benchmark cases; record
       pass/fail and the deterministic baseline's results (these numbers
       are the comparison baseline for every research module — P8.06).
    3. Confirm the `research-gnn` / `research-marl` profiles exist, grant
       no dispatch authority, and that the gRPC worker template runs.
    4. List every gap; block research-provider design (P8.07 onward) on any
       gap in the deterministic baseline, the benchmark cases, or the
       worker contract.
  - **Deliverable / output:** Readiness-check record with command outputs,
    baseline benchmark results, profile/worker verification, and gap list.
  - **Acceptance criteria / exit check:** Both doc tools pass; tests pass;
    baseline benchmark results are recorded with versions/seeds; research
    profiles verified as no-dispatch; gap list empty or every gap owned.
  - **Owner role:** Research lead.
  - **Dependencies:** P7.31 (Phase 7).
  - **Verification method:** Re-run the quoted commands and the benchmark
    cases; compare results to the record.
  - **Risks / failure modes:** Comparing a research module against a
    baseline that was never run (the "strong deterministic baseline" in the
    exit gate would then be fiction); a research profile that silently
    grants dispatch authority.

## P8.02 — Research scope freeze: questions, hypotheses, and non-goals

- [x] **P8.02 — Research scope freeze: questions, hypotheses, and non-goals** — implemented, awaiting human build/test (decision record: `ledger/phase-08-advanced-coordination/02_research_scope_freeze.md`)
  - **Objective:** Freeze, in writing, the research questions, the
    hypotheses to be tested, and what is explicitly not claimed — before
    any model is built. Research without a pre-registered question is
    exploration, not an experiment.
  - **Report trace:** §13.1–§13.3; §19.1; §19.4.
  - **Inputs / prerequisites:** P8.01 baseline results; §13 module
    descriptions.
  - **Steps:**
    1. Write one research question per module (ADMM/DMPC, GNN warm-start/
       screening, safe MARL, dynamic stability) in falsifiable form, with
       the metric that would answer it (§19.4) and the threshold that
       would count as success — set **after defining the model, service,
       benchmark, and operational risk** (§19.4), never as a universal
       target like "zero violations" or "converges in 30 iterations".
    2. Pre-register the hypotheses, the baselines (P8.06), the held-out
       sets, and the analysis plan — before training or tuning begins.
    3. Write the non-claims list: no global optimality without a theorem;
       no safety guarantee from a finite test set; no field authority;
       no claim that a steady-state tool evidences transient stability
       (§19.1).
    4. Record the scope as a research plan document with an owner and a
       review date.
  - **Deliverable / output:** Research plan (questions, hypotheses,
    metrics, thresholds, baselines, held-out sets, non-claims).
  - **Acceptance criteria / exit check:** Every module has a falsifiable
    question, a named metric, and a pre-registered threshold; the
    non-claims list is present; no threshold is a universal target of the
    kind §19.4 forbids.
  - **Owner role:** Research lead.
  - **Dependencies:** P8.01.
  - **Verification method:** Independent reviewer checks each question
    against §13 and §19.4; any question without a metric fails.
  - **Risks / failure modes:** Research that "discovers" its question after
    seeing results; thresholds set after the numbers are known; a success
    criterion that is really a safety claim in disguise.

## P8.03 — License and dependency audit for the research stack (D4 follow-through)

- [x] **P8.03 — License and dependency audit for the research stack (D4 follow-through)** — implemented, awaiting human build/test (decision record: `ledger/phase-08-advanced-coordination/03_research_license_audit.md`)
  - **Objective:** Audit the research stack's licenses — notably ANDES
    (GPLv3), PyTorch, PyTorch Geometric, RLlib/Ray, HELICS, pandapower —
    and record the distribution/use consequences before any of it is
    depended on.
  - **Report trace:** §25.2 (D4); §9.3; §7.4; §22.2.
  - **Inputs / prerequisites:** RESEARCH.md (ANDES 2.0.0 GPLv3; PyTorch
    2.15; PyG 2.8.0; Ray/RLlib; Gymnasium 1.3.0); D4 status.
  - **Steps:**
    1. Enumerate every research dependency with license (SPDX), version
       pinned, and source; include training, simulation, and benchmark
       dependencies separately from runtime ones.
    2. Record the consequence of each copyleft item: ANDES GPLv3 means the
       dynamic-simulation work must be isolated as a research worker and
       its outputs (not the code) are what FlexCore consumes — record the
       isolation design and get legal's confirmation; do not link GPLv3
       code into the Apache-2.0-recommended core.
    3. Record the PyTorch/PyG version-skew finding (RESEARCH.md R8-C3):
       pin a verified-compatible pair; do not assume PyG 2.8 works with
       PyTorch 2.15.
    4. Add the audit to the SBOM inputs and to CI's license gate.
  - **Deliverable / output:** Research license-audit table + isolation
    design for copyleft items + pinned version pairs; D4 status update.
  - **Acceptance criteria / exit check:** Every research dependency has a
    recorded license and verdict; every copyleft item has a named
    isolation design confirmed by legal; the PyTorch/PyG pair is a
    verified-compatible pin, not an assumption.
  - **Owner role:** Legal + research lead.
  - **Dependencies:** P8.02.
  - **Verification method:** Re-run the enumeration from the lockfiles and
    diff against the audit table; confirm the ANDES isolation by
    inspecting the worker boundary.
  - **Risks / failure modes:** GPLv3 code linked into the core, forcing a
    license decision after the fact; a PyTorch/PyG pair that installs but
    fails at runtime (or silently changes numerics).

## P8.04 — Research worker contracts: result, provenance, and proposal-only boundary

- [x] **P8.04 — Research worker contracts: result, provenance, and proposal-only boundary** — implemented, awaiting human build/test (decision record: `ledger/phase-08-advanced-coordination/04_worker_contracts.md`)
  - **Objective:** Define the contracts every research worker implements —
    result shape, provenance, and the enforced proposal-only boundary —
    reusing the Phase 7 worker template, not inventing a parallel one.
  - **Report trace:** §13.4; §9.2; §5.7; §22.1 (ADR-010); §7.4.
  - **Inputs / prerequisites:** P7.31 handover; P7.17 worker template;
    §9.2 result requirements.
  - **Steps:**
    1. Define the research result contract per module, extending the §9.2
       base (plugin/model/solver version, snapshot ID, horizon, problem
       class, termination status, diagnostics, reproducibility metadata)
       with module-specific fields: ADMM residuals/iteration counts;
       GNN warm-start provenance (model version, training data ref,
       topology scope); MARL policy version, training seed, environment
       version, safety-filter outcome.
    2. Enforce the proposal-only boundary in the contract: research
       workers return proposed schedules, candidate setpoint vectors,
       rankings/scores, forecast distributions, uncertainty estimates, or
       diagnostics — never an authorization token (§13.4). Implement the
       boundary in code (the worker's response type cannot carry an
       authorization token), not only in docs.
    3. Define the validation hook: every research proposal enters the same
       protected validation → feasibility → authorization path as any
       other candidate (§13.1: GNN output must pass the same physical
       validation as any other candidate).
    4. Apply `buf lint` / `buf breaking` to the research Protobuf schemas.
  - **Deliverable / output:** Research worker contracts (Protobuf + docs) +
    proposal-only enforcement + validation-hook specification.
  - **Acceptance criteria / exit check:** Every research result carries
    §9.2 provenance/diagnostics; the response type cannot express an
    authorization token (verified by type inspection); a research proposal
    that fails physical validation is rejected by the same gate as a
    deterministic candidate (tested in P8.05).
  - **Owner role:** Research lead + Rust engineer.
  - **Dependencies:** P7.31, P8.02.
  - **Verification method:** Type inspection of the response schemas;
    submit a deliberately infeasible research proposal and confirm
    rejection by the feasibility gate.
  - **Risks / failure modes:** A research result type that smuggles in an
    authorization field "for later"; validation applied to deterministic
    candidates but skipped for research ones.

## P8.05 — Independent validation harness (the validator that does not trust the researcher)

- [x] **P8.05 — Independent validation harness (the validator that does not trust the researcher)** — implemented, awaiting human build/test (decision record: `ledger/phase-08-advanced-coordination/05_06_harness_baselines.md`)
  - **Objective:** Build the independent validation harness that checks
    every research output against physical constraints and the deterministic
    baseline — owned separately from the research code, with its own tests.
  - **Report trace:** §13.1–§13.3; §19.2(5)(9); §12.5; §22.3 (optimizer
    outputs independently validated).
  - **Inputs / prerequisites:** P8.04 contracts; Phase 4 feasibility gate;
    §19.3 benchmark cases.
  - **Steps:**
    1. Implement the harness: it takes a research proposal + the input
       snapshot and re-checks asset capability (§11), network feasibility
       via AC power flow (§12.5 — power flow evaluates a state; it is not
       AC-OPF), SOC/ramp/duration, and command invariants (§14.3).
    2. Give the harness its own test suite, including cases where the
       research module is deliberately wrong (infeasible warm start,
       constraint-violating MARL action, non-converged ADMM) — the harness
       must catch each.
    3. Separate ownership: the harness is reviewed by someone who did not
       write the research module it validates; its code lives outside the
       research worker.
    4. Record the harness version with every validation result.
  - **Deliverable / output:** Independent validation harness + its test
    suite + ownership record.
  - **Acceptance criteria / exit check:** Harness catches every
    deliberately-wrong case; harness code is not importable by the research
    worker (no self-validation); every validation result records the
    harness version.
  - **Owner role:** Power-systems reviewer (not the module author).
  - **Dependencies:** P8.04.
  - **Verification method:** Run the deliberately-wrong suite; inspect the
    import graph for harness/research separation.
  - **Risks / failure modes:** The researcher validating their own output
    (the exact failure mode the exit gate's "independently validated"
    exists to prevent); a harness that shares the research module's buggy
    constraint code.

## P8.06 — Strong deterministic baselines (the comparison every module must beat)

- [x] **P8.06 — Strong deterministic baselines (the comparison every module must beat)** — implemented, awaiting human build/test (decision record: `ledger/phase-08-advanced-coordination/05_06_harness_baselines.md`)
  - **Objective:** Define, implement, and run the strong deterministic
    baselines each research module is compared against — and record their
    results before any research result is compared to them.
  - **Report trace:** §23 Stage 8 (exit gate); §13.1–§13.3; §19.3–§19.4;
    §12.3.
  - **Inputs / prerequisites:** P8.01 baseline benchmark results; P8.02
    research plan.
  - **Steps:**
    1. Define the baselines per module: ADMM/DMPC vs centralized
       LP/MILP/QP (HiGHS) and vs rule-based dispatch; GNN warm-start vs
       cold-start solver and vs a non-GNN heuristic warm start; MARL vs
       deterministic optimization **and** vs rule-based/PI baselines
       (§13.3) and vs centralized-training baselines where applicable.
    2. Run every baseline on the §19.3 benchmark cases with pinned solver
       versions, seeds, and tolerances; record objective value, feasible
       interval count, runtime, violations, and the §19.4 metrics.
    3. Freeze the baseline results (versioned artifact) before research
       results are compared — no moving the baseline after seeing research
       numbers.
    4. Document each baseline's assumptions and limits (e.g. the
       centralized baseline's problem class and solver; the rule-based
       baseline's rules) so a "win" against a weak baseline is visible.
  - **Deliverable / output:** Baseline definitions + frozen baseline
    results artifact + assumptions record.
  - **Acceptance criteria / exit check:** Every research module has at
    least one strong deterministic baseline and one rule-based baseline
    defined and run; results are frozen and versioned; assumptions are
    written down.
  - **Owner role:** Research lead + power-systems reviewer.
  - **Dependencies:** P8.01, P8.02.
  - **Verification method:** Re-run one baseline case from the frozen
    artifact's inputs and confirm the recorded result reproduces within
    tolerance.
  - **Risks / failure modes:** A straw-man baseline (e.g. an untuned
    rule-based controller) making a research module look good; baseline
    results edited after the comparison.

## P8.07 — ADMM/DMPC provider: formulation with explicit convexity assumptions

- [x] **P8.07 — ADMM/DMPC provider: formulation with explicit convexity assumptions** — implemented, awaiting human build/test (decision record: `ledger/phase-08-advanced-coordination/07_admm_dmpc.md`)
  - **Objective:** Formulate the DMPC/ADMM provider as decomposable local
    problems with explicit coupling constraints — and write down, as part
    of the formulation, the convexity assumptions the convergence claim
    depends on.
  - **Report trace:** §13.2; §12.3; §7.4.
  - **Inputs / prerequisites:** P8.02 plan; P8.04 contracts; P8.06
    baselines.
  - **Steps:**
    1. Write the formulation: minimize Σ f_k(x_k) subject to Σ A_k x_k ≤ b
       and x_k ∈ X_k (§13.2), with consensus/boundary variables for
       quantities that must agree or satisfy shared constraints. Do not
       force unrelated sites to set identical power values (§13.2).
    2. State the convexity assumptions explicitly (convex f_k, convex X_k,
       Slater-type conditions as applicable) and the problem classes the
       provider supports — and the classes it does not (nonconvex AC-OPF,
       mixed-integer local problems).
    3. Implement the provider as a Phase 7 gRPC worker (Class C) behind
       the `Optimizer` contract, declaring its problem class per §7.4.
    4. Record the formulation, assumptions, and solver choices in a
       design note that a power-systems reviewer signs.
  - **Deliverable / output:** ADMM/DMPC formulation + assumptions note
    (reviewer-signed) + provider skeleton behind the `Optimizer` contract.
  - **Acceptance criteria / exit check:** Formulation, assumptions, and
    supported/non-supported classes are written and signed; the provider
    compiles behind the contract; no convergence or optimality claim
    appears for a nonconvex/mixed-integer case.
  - **Owner role:** Research engineer (optimization).
  - **Dependencies:** P8.02, P8.04, P8.06.
  - **Verification method:** Reviewer reads the assumptions note against
    the implementation; grep the provider docs for "global optimum" /
    "converges" and confirm each occurrence is scoped to the stated
    assumptions.
  - **Risks / failure modes:** ADMM presented as generally convergent when
    the local problems are mixed-integer (then it is a heuristic — §13.2
    and the 2026 literature in RESEARCH.md say so); consensus variables
    that force unrelated sites to identical setpoints.

## P8.08 — ADMM/DMPC provider: residuals, termination, infeasibility, and centralized fallback

- [x] **P8.08 — ADMM/DMPC provider: residuals, termination, infeasibility, and centralized fallback** — implemented, awaiting human build/test (decision record: `ledger/phase-08-advanced-coordination/08_admm_operations.md`)
  - **Objective:** Implement the ADMM operational contract: primal/dual
    residuals, termination criteria, infeasibility handling, iteration
    budget, communication failure, stale participants, and centralized
    fallback (§13.2).
  - **Report trace:** §13.2; §19.4 (ADMM metrics); §19.2(6).
  - **Inputs / prerequisites:** P8.07 formulation.
  - **Steps:**
    1. Implement primal/dual residual computation and reporting on every
       iteration; define termination (residual tolerances + iteration
       budget) and record both in the result (§9.2 diagnostics).
    2. Define infeasibility behaviour: what the provider returns when the
       coupled problem is infeasible, and how that is distinguished from
       non-convergence within budget.
    3. Implement communication-failure and stale-participant handling:
       bounded staleness, what happens when a participant drops out
       mid-iteration, and when the run is abandoned.
    4. Implement the centralized fallback: on ADMM failure, the run falls
       back to the centralized deterministic solver (P8.06 baseline) and
       the fallback is recorded in the result — the schedule never comes
       from a half-converged distributed run presented as final.
  - **Deliverable / output:** ADMM operational implementation + fallback
    logic + tests for each failure mode.
  - **Acceptance criteria / exit check:** Residuals reported per run;
    termination/infeasibility/fallback each tested and behaving as
    specified; a dropped participant mid-run produces either a correct
    fallback or a recorded failure, never a silently partial result.
  - **Owner role:** Research engineer (optimization).
  - **Dependencies:** P8.07.
  - **Verification method:** Fault-injection tests (drop a participant,
    force infeasibility, exhaust the iteration budget) with recorded
    outcomes.
  - **Risks / failure modes:** A non-converged run reported as a solution
    because residuals were never checked; fallback that itself uses the
    failed ADMM output as a warm start without validation.

## P8.09 — ADMM/DMPC evaluation against baselines on the benchmark cases

- [x] **P8.09 — ADMM/DMPC evaluation against baselines on the benchmark cases** — implemented, awaiting human build/test (decision record: `ledger/phase-08-advanced-coordination/09_admm_evaluation.md`)
  - **Objective:** Evaluate the ADMM/DMPC provider against the frozen
    baselines (P8.06) on the §19.3 cases, reporting the §19.4 ADMM metrics
    honestly — including the cases where it loses.
  - **Report trace:** §13.2; §19.3–§19.4; §23 Stage 8.
  - **Inputs / prerequisites:** P8.07–P8.08; P8.05 harness; P8.06 frozen
    baselines.
  - **Steps:**
    1. Run the provider on the benchmark cases (IEEE feeders, synthetic
       portfolios, multi-feeder substation constraint, stressed scenarios
       per §19.3) with pinned versions/seeds.
    2. Report per case: primal/dual residual, objective gap where
       meaningful, iteration count, time, communication sensitivity
       (§19.4) — and the independent validation outcome (P8.05).
    3. Compare against the frozen baselines on the same cases; report wins,
       losses, and ties without selecting favourable cases.
    4. Write the evaluation note: where ADMM helps, where it does not, and
       what would change the answer (problem size, coupling strength,
       communication reliability).
  - **Deliverable / output:** ADMM evaluation report (per-case table +
    baseline comparison + evaluation note).
  - **Acceptance criteria / exit check:** All benchmark cases run and
    reported (no case silently dropped); every reported solution passed
    independent validation; losses are reported alongside wins; no global
    optimality claim appears outside the stated convexity assumptions.
  - **Owner role:** Research engineer (optimization).
  - **Dependencies:** P8.07, P8.08, P8.05, P8.06.
  - **Verification method:** Independent re-run of two cases from the
    recorded inputs; check the per-case table is complete against the
    §19.3 case list.
  - **Risks / failure modes:** Cherry-picked cases; objective gap reported
    where it is not meaningful (e.g. against a nonconvex baseline);
    communication sensitivity never tested despite being a headline ADMM
    metric.

## P8.10 — GNN experiment design: warm-start and screening, with topology scope stated

- [x] **P8.10 — GNN experiment design: warm-start and screening, with topology scope stated** — implemented, awaiting human build/test (decision record: `ledger/phase-08-advanced-coordination/10_gnn_warmstart.md`)
  - **Objective:** Design the GNN warm-start/screening experiment (§13.1)
    with the topology scope, training data, and success metric stated up
    front — and with the non-GNN baseline named.
  - **Report trace:** §13.1; §19.3–§19.4; §7.4 (PyTorch + PyG).
  - **Inputs / prerequisites:** P8.02 plan; P8.06 baselines; RESEARCH.md
    (PyTorch 2.15 / PyG 2.8.0 version skew).
  - **Steps:**
    1. Define the two roles separately: (a) candidate AC-OPF warm-start
       generation, (b) active-constraint prediction / contingency
       ranking/screening. Each gets its own question, metric, and baseline
       — a warm-start win is not a screening win.
    2. State the topology scope: which feeders/topologies are in training,
       which are held out (unseen topologies and operating regimes per
       §13.1), and how transfer is measured.
    3. Define the training data: source (simulator-generated cases, with
       generator version and seeds), size, and the leakage controls (no
       held-out topology in training, no test-case leakage through
       normalization statistics).
    4. Pin the stack (verified PyTorch/PyG pair per P8.03) and record the
       model architecture, hyperparameters, and seeds.
  - **Deliverable / output:** GNN experiment design (roles, scope, data,
    stack pins, metrics, baselines).
  - **Acceptance criteria / exit check:** Both roles have separate
    questions/metrics/baselines; held-out topologies named; leakage
    controls written; stack pins are verified-compatible.
  - **Owner role:** Research engineer (ML).
  - **Dependencies:** P8.02, P8.03, P8.06.
  - **Verification method:** Reviewer checks the held-out list against the
    training-data manifest for overlap; stack pins verified by install.
  - **Risks / failure modes:** Test topologies leaking into training
    (inflated generalization); warm-start and screening results conflated
    into one headline number; an unverified PyTorch/PyG pair failing
    mid-experiment.

## P8.11 — GNN warm-start/screening implementation (isolated inference worker)

- [x] **P8.11 — GNN warm-start/screening implementation (isolated inference worker)** — implemented, awaiting human build/test (decision record: `ledger/phase-08-advanced-coordination/11_gnn_inference_worker.md`)
  - **Objective:** Implement the GNN as an isolated Python inference worker
    (Phase 7 Class C) that returns candidates/rankings with uncertainty —
    and nothing that could be mistaken for a dispatch decision.
  - **Report trace:** §13.1; §13.4; §5.4 Class C; §7.4.
  - **Inputs / prerequisites:** P8.10 design; P8.04 contracts.
  - **Steps:**
    1. Implement training (offline, versioned artifacts in object storage
       per §6.5) and inference (isolated worker, CPU/GPU limits recorded)
       as separate steps with separate version pins.
    2. Implement the inference outputs: warm-start candidate (with the
       solver still solving — the GNN does not replace the solver), or a
       screening ranking with scores and uncertainty (§13.1: surrogate
       estimates with uncertainty).
    3. Return §9.2 + P8.04 provenance on every output: model version,
       training-data reference, input snapshot ID, topology scope.
    4. Test the worker's failure modes: model file missing, input topology
       outside the trained scope (must be flagged, not silently answered),
       inference timeout.
  - **Deliverable / output:** GNN training + inference worker + artifact
    store + failure-mode tests.
  - **Acceptance criteria / exit check:** Worker runs isolated; every
    output carries provenance and uncertainty where applicable;
    out-of-scope topology is flagged; a missing model file fails loudly.
  - **Owner role:** Research engineer (ML).
  - **Dependencies:** P8.10, P8.04.
  - **Verification method:** Run inference on an out-of-scope topology and
    confirm the flag; kill the worker and confirm the control plane
    survives.
  - **Risks / failure modes:** A GNN output presented as a solution rather
    than a warm start (the solver never runs); out-of-scope inputs answered
    confidently; training artifacts unversioned and unreproducible.

## P8.12 — GNN evaluation: end-to-end feasible solve time vs baselines, on unseen topologies

- [x] **P8.12 — GNN evaluation: end-to-end feasible solve time vs baselines, on unseen topologies** — implemented, awaiting human build/test (decision record: `ledger/phase-08-advanced-coordination/12_gnn_evaluation.md`)
  - **Objective:** Evaluate the GNN on unseen topologies and operating
    regimes, reporting **end-to-end feasible solve time, not just model
    inference latency** (§13.1), against the non-GNN baseline.
  - **Report trace:** §13.1; §19.3–§19.4; §23 Stage 8.
  - **Inputs / prerequisites:** P8.11 worker; P8.05 harness; P8.06 frozen
    baselines; P8.10 held-out sets.
  - **Steps:**
    1. Run the warm-start experiment on held-out topologies: measure
       end-to-end time to a **feasible** solution (solver + validation),
       feasible-solver success rate, and speedup vs the cold-start and
       heuristic-warm-start baselines (§19.4 GNN metrics).
    2. Run the screening experiment: measure ranking quality against the
       true active-constraint/contingency sets, and the cost of a missed
       constraint (a screened-out constraint that was actually active).
    3. Validate every GNN-proposed solution through the independent
       harness (P8.05) — a warm start that leads to an infeasible solution
       counts as a failure of the end-to-end metric, not as a fast
       inference.
    4. Write the evaluation note, including where the GNN does not
       generalize and what would be needed for it to.
  - **Deliverable / output:** GNN evaluation report (per-topology table +
    baseline comparison + evaluation note).
  - **Acceptance criteria / exit check:** Evaluation covers held-out
    topologies only for the generalization claim; end-to-end feasible
    solve time (not inference latency) is the headline metric; every
    counted "success" passed independent validation; misses and
    non-generalization are reported.
  - **Owner role:** Research engineer (ML).
  - **Dependencies:** P8.11, P8.05, P8.06, P8.10.
  - **Verification method:** Independent re-run of two held-out topologies
    from the recorded artifacts; verify the headline metric recomputes
    from the raw timings.
  - **Risks / failure modes:** Inference latency presented as solve time;
    held-out topologies that are near-duplicates of training ones;
    infeasible warm starts counted as successes because the solver "fixed"
    them (that is the solver's success, not the GNN's).

## P8.13 — Safe MARL design: CTDE baseline, safety filter, and the infeasible-filter outcome

- [x] **P8.13 — Safe MARL design: CTDE baseline, safety filter, and the infeasible-filter outcome** — implemented, awaiting human build/test (decision record: `ledger/phase-08-advanced-coordination/13_marl_design.md`)
  - **Objective:** Design the MARL research (simulation/shadow only) around
    a CTDE baseline, with a safety-projection layer whose derivation and
    infeasibility outcome are defined before training begins.
  - **Report trace:** §13.3; §19.1; §7.4 (RLlib / Gymnasium).
  - **Inputs / prerequisites:** P8.02 plan; P8.06 baselines; RESEARCH.md
    (RLlib/Ray, Gymnasium, ANDES `andes.rl`).
  - **Steps:**
    1. Define the MARL setup: centralized training with decentralized
       execution (CTDE) as the research baseline (§13.3); the environment
       (simulator + Gymnasium API), observation/action spaces, reward,
       and the agents' scope (per-site/per-asset — record the choice).
    2. Define the safety layer: a control-barrier-function or
       safety-projection layer **derived from a valid system model and the
       correct relative degree** (§13.3). Record the derivation and the
       model it depends on. Note explicitly: voltage and branch-current
       constraints are not automatically simple dynamic state constraints.
    3. Define the infeasible-filter outcome: a filter can be infeasible —
       define what happens then (fallback action, recorded failure, never
       an unfiltered action passed through silently).
    4. Select the framework explicitly (RLlib/Ray vs alternatives) with
       version pins and the API stack it targets (RLlib's new API stack
       vs old — record the choice; see RESEARCH.md).
  - **Deliverable / output:** MARL design (CTDE setup, environment spec,
    safety-filter derivation, infeasible-filter outcome, framework pins).
  - **Acceptance criteria / exit check:** CTDE baseline named; safety
    filter derived from a stated model with correct relative degree (or
    recorded as not yet derived — and training blocked until it is);
    infeasible-filter outcome defined; framework + API stack pinned.
  - **Owner role:** Research engineer (RL).
  - **Dependencies:** P8.02, P8.06.
  - **Verification method:** Power-systems reviewer checks the safety-filter
    derivation (model, relative degree) independently; any derivation gap
    blocks P8.14.
  - **Risks / failure modes:** A safety filter derived from the wrong model
    or wrong relative degree (unsafe and unnoticed); an infeasible filter
    that silently passes the raw action through; a framework API-stack
    mismatch discovered mid-training (RLlib new vs old stack).

## P8.14 — MARL simulation/shadow worker (never live dispatch)

- [x] **P8.14 — MARL simulation/shadow worker (never live dispatch)** — implemented, awaiting human build/test (decision record: `ledger/phase-08-advanced-coordination/14_marl_worker.md`)
  - **Objective:** Implement the MARL worker as a simulation/shadow worker
    (Phase 7 Class C) whose actions are logged and evaluated, never issued
    to devices.
  - **Report trace:** §13.3; §13.4; §19.2(9); §5.7.
  - **Inputs / prerequisites:** P8.13 design; P8.04 contracts; P7.17
    template.
  - **Steps:**
    1. Implement the worker: training (offline) + shadow inference, with
       a shadow action logger recording every proposed action, the
       safety-filter outcome, and the simulated consequence.
    2. Enforce the boundary in code: the worker has no path to the
       dispatch gateway, no authorization token, and no command-issuing
       capability; its output type is a proposal (§13.4, P8.04).
    3. Implement the safety filter from P8.13, including the
       infeasible-filter outcome, and log every filter activation and
       every infeasibility.
    4. Test the boundary: attempt to route a MARL action to dispatch from
       inside the worker and confirm there is no code path that allows it.
  - **Deliverable / output:** MARL simulation/shadow worker + shadow action
    logger + boundary tests.
  - **Acceptance criteria / exit check:** Worker runs in simulation/shadow
    only; every action is logged with its filter outcome; no code path
    from the worker to dispatch exists (verified by inspection + test);
    filter infeasibility is logged, never silent.
  - **Owner role:** Research engineer (RL).
  - **Dependencies:** P8.13, P8.04.
  - **Verification method:** Code-path inspection from worker output to
    dispatch (must be empty); attempt the routing in a test and confirm
    failure.
  - **Risks / failure modes:** A "shadow" worker with a dispatch call left
    in from an earlier prototype; a shadow log that records actions but
    not filter outcomes, making safety unevaluable.

## P8.15 — MARL evaluation: held-out regimes, outages, delays, uncertainty

- [x] **P8.15 — MARL evaluation: held-out regimes, outages, delays, uncertainty** — implemented, awaiting human build/test (decision record: `ledger/phase-08-advanced-coordination/15_marl_evaluation.md`)
  - **Objective:** Evaluate the MARL policy under held-out topologies,
    load/weather regimes, device outages, delayed/lost messages, and
    uncertain state estimates — against deterministic and rule-based/PI
    baselines (§13.3).
  - **Report trace:** §13.3; §19.3–§19.4; §19.2(9).
  - **Inputs / prerequisites:** P8.14 worker; P8.05 harness; P8.06 frozen
    baselines.
  - **Steps:**
    1. Define the evaluation suite: held-out topologies and regimes; device
       outage cases; message delay/loss cases; state-estimate uncertainty
       cases. Each case names its baseline comparison.
    2. Run the suite with pinned seeds/versions; report per case:
       constraint-violation frequency, return, robustness under
       distribution shift, baseline comparison (§19.4 MARL metrics).
    3. Report the safety-filter statistics: activation rate, infeasibility
       rate, and the outcome of every infeasibility.
    4. Write the evaluation note stating plainly what the finite test set
       does and does not establish (§13.3: zero observed violations is
       not a universal safety guarantee).
  - **Deliverable / output:** MARL evaluation report (per-case table +
    baseline comparison + filter statistics + evaluation note).
  - **Acceptance criteria / exit check:** All evaluation cases run and
    reported; baselines compared on the same cases; filter statistics
    complete; the evaluation note contains the finite-test-set limitation
    in plain language.
  - **Owner role:** Research engineer (RL).
  - **Dependencies:** P8.14, P8.05, P8.06.
  - **Verification method:** Independent re-run of two cases from the
    recorded seeds/artifacts; check the case list is complete against the
    P8.13/P8.15 suite definition.
  - **Risks / failure modes:** Evaluation only on training regimes (no
    distribution shift tested despite the claim); message delay/loss never
    tested although the deployment will have them; a zero-violation run
    presented as a safety guarantee.

## P8.16 — Dynamic simulator integration (ANDES): scope, models, and the steady-state boundary

- [x] **P8.16 — Dynamic simulator integration (ANDES): scope, models, and the steady-state boundary** — implemented, awaiting human build/test (decision record: `ledger/phase-08-advanced-coordination/16_andes_integration.md`)
  - **Objective:** Integrate the dynamic simulator (ANDES) as a research
    worker for frequency/stability questions, with its model scope stated
    and the boundary against quasi-static power flow drawn explicitly.
  - **Report trace:** §19.1 (dynamic stability track); §7.4 (ANDES);
    §13.2–§13.3 (stability research context).
  - **Inputs / prerequisites:** P8.02 plan; P8.03 license audit (ANDES
    GPLv3 isolation); RESEARCH.md (ANDES 2.0.0).
  - **Steps:**
    1. Define the scope: which questions the dynamic simulator answers
       (e.g. does a proposed research controller affect frequency/
       transient behaviour in the declared model?) and which it does not.
    2. Select and record the ANDES models used (generators, exciters,
       governors, renewable models PVD1/ESD1, loads) and the test cases
       (e.g. Kundur two-area, IEEE 14-bus with dynamic data) with versions.
    3. Implement the integration as an isolated Python worker (Class C)
       behind a research contract; ANDES stays behind the worker boundary
       per the P8.03 isolation design (GPLv3).
    4. Write the boundary statement: do not use a steady-state power-flow
       tool as evidence for transient frequency stability, and do not use
       the dynamic simulator as a replacement for distribution
       quasi-static power flow (§19.1).
  - **Deliverable / output:** Dynamic-simulator integration (worker) +
    model/case record + boundary statement.
  - **Acceptance criteria / exit check:** Worker runs ANDES time-domain
    simulation and eigenvalue analysis on the recorded cases; model/case
    versions recorded; boundary statement present; ANDES isolation
    (P8.03) verified by inspection.
  - **Owner role:** Research engineer (power systems).
  - **Dependencies:** P8.02, P8.03.
  - **Verification method:** Run a time-domain simulation and an eigenvalue
    analysis from the recorded inputs and confirm the outputs; inspect the
    worker boundary for GPLv3 isolation.
  - **Risks / failure modes:** Dynamic results quoted as quasi-static
    feasibility evidence (or vice versa); an ANDES version/model change
    unrecorded, making the study unreproducible; GPLv3 code leaking into
    the core.

## P8.17 — Dynamic evaluation: research controllers under disturbances

- [x] **P8.17 — Dynamic evaluation: research controllers under disturbances** — implemented, awaiting human build/test (decision record: `ledger/phase-08-advanced-coordination/17_dynamic_evaluation.md`)
  - **Objective:** Evaluate the research controllers (ADMM/DMPC and MARL
    proposals, as applicable) in the dynamic simulator under disturbances,
    and report what the dynamic model says — and what it cannot say.
  - **Report trace:** §19.1; §19.3–§19.4; §13.3.
  - **Inputs / prerequisites:** P8.16 integration; P8.09/P8.15 results.
  - **Steps:**
    1. Define the disturbance set: faults, line trips, generator outages,
       load steps — each with the case file, disturbance parameters, and
       simulation settings (solver, step, end time).
    2. Run each research controller's proposals through the dynamic
       simulator in shadow (proposals applied to the simulated system,
       never to devices); record frequency nadir/settling, voltage
       behaviour, and any instability, with the model scope attached to
       every number.
    3. Compare against the deterministic baseline controller on the same
       disturbances.
    4. Write the evaluation note with the model limits: the dynamic model
       is a declared model, not the real grid; its results are evidence
       about the model, stated as such.
  - **Deliverable / output:** Dynamic evaluation report (per-disturbance
    table + baseline comparison + model-limits note).
  - **Acceptance criteria / exit check:** All disturbances run and
    reported; every number carries its model/case reference; baseline
    comparison on the same disturbances; model limits stated.
  - **Owner role:** Research engineer (power systems).
  - **Dependencies:** P8.16, P8.09, P8.15.
  - **Verification method:** Independent re-run of one disturbance from
    the recorded case files.
  - **Risks / failure modes:** A dynamic result presented without its model
    scope (implying it describes the real grid); disturbances chosen
    because the research controller handles them well.

## P8.18 — Co-simulation integration (HELICS) for multi-domain interaction

- [x] **P8.18 — Co-simulation integration (HELICS) for multi-domain interaction** — implemented, awaiting human build/test (decision record: `ledger/phase-08-advanced-coordination/18_helics_cosim.md`)
  - **Objective:** Integrate HELICS co-simulation so grid, communication,
    market, and end-use simulators can interact across time domains — with
    HELICS used as the coordinator, not as the physics model.
  - **Report trace:** §19.1 (multi-domain co-simulation track); §7.4
    (HELICS [R16]).
  - **Inputs / prerequisites:** P8.16 dynamic integration; P8.01 simulator
    adapters.
  - **Steps:**
    1. Define the federation: which simulators participate (grid, comms,
       market, end-use), their time steps, and the exchanged quantities.
    2. Implement the HELICS federation with pinned HELICS version and
       federate versions; record the time-synchronization scheme.
    3. Run a defined multi-domain scenario (e.g. communication delay
       affecting a MARL/DMPC proposal's delivery to the grid model) and
       record the interaction effect.
    4. Write the boundary statement: HELICS synchronizes federates; it is
       not itself the power-system physics model (§19.1).
    5. Record why no deterministic-baseline comparison applies to the
       federation itself (HELICS is the coordinator, not a decision module
       — §19.1) and define the comparison that does apply: the same
       multi-domain scenario run federated vs unfederated (or with the
       interaction disabled), so the measured interaction effect has a
       control.
  - **Deliverable / output:** HELICS federation + scenario run record +
    boundary statement.
  - **Acceptance criteria / exit check:** Federation runs with all named
    federates; the scenario's interaction effect is measured and recorded;
    boundary statement present; versions pinned.
  - **Owner role:** Research engineer.
  - **Dependencies:** P8.16.
  - **Verification method:** Re-run the federation from the recorded
    configuration; confirm each federate's outputs are present.
  - **Risks / failure modes:** HELICS results presented as power-system
    results (the coordinator mistaken for the model); time-step mismatch
    between federates silently corrupting the interaction.

## P8.19 — Benchmark cases: extend §19.3 with research cases (and record what was not tested)

- [x] **P8.19 — Benchmark cases: extend §19.3 with research cases (and record what was not tested)** — implemented, awaiting human build/test (decision record: `ledger/phase-08-advanced-coordination/19_benchmark_cases.md`; versioned artifact `benchmarks/research-set-v1.json` + not-tested list `benchmarks/research-set-v1-not-tested.json` — WRITTEN-UNRUN, every `run_status` NOT_RUN)
  - **Objective:** Extend the benchmark set with the research-specific
    cases (stressed scenarios, topology changes, communication loss, asset
    dropout, reverse power flow) and record explicitly which important
    cases were **not** tested.
  - **Report trace:** §19.3; §19.4.
  - **Inputs / prerequisites:** P8.01 baseline cases; P8.09/P8.12/P8.15
    evaluation needs.
  - **Steps:**
    1. List the §19.3 base cases and the research additions, each with its
       source, version, and parameters. Name the IEEE 39-bus and 118-bus
       transmission cases explicitly where they are used; a §19.3 case
       that is not listed is not run.
    2. Add the stressed scenarios: PV ramps, forecast error, communication
       loss, asset dropout, reverse power flow, topology change (§19.3).
    3. Write the not-tested list: important cases that were not run (e.g.
       a 10,000-bus network — §19.3 forbids claiming first-feeder tests
       establish capability for one) with the reason.
    4. Version the benchmark set as an artifact; every evaluation in this
       phase cites the benchmark-set version.
  - **Deliverable / output:** Versioned benchmark set + not-tested list.
  - **Acceptance criteria / exit check:** Benchmark set versioned; every
    evaluation cites it; not-tested list exists and names at least the
    scale cases that were not run.
  - **Owner role:** Research lead.
  - **Dependencies:** P8.01.
  - **Verification method:** Check each evaluation report's cited
    benchmark version against the artifact.
  - **Risks / failure modes:** A benchmark set that grows silently between
    evaluations, making comparisons incomparable; scale claims made from
    small-feeder results.

## P8.20 — Reproducibility: seeds, versions, artifacts, and the rerun test

- [x] **P8.20 — Reproducibility: seeds, versions, artifacts, and the rerun test** — implemented, awaiting human build/test (decision record: `ledger/phase-08-advanced-coordination/20_reproducibility.md` — the reproducibility standard: record schema `flexcore-research/reproducibility-record.v1` + seed-derivation/rerun-check tooling in `apps/worker/python/reproducibility/` (WRITTEN-NEVER-RUN); every rerun cell EMPTY — no rerun has happened; tolerances owned by P8.38, none pre-registered)
  - **Objective:** Make every research result reproducible from its
    recorded inputs: seeds, solver/model/framework versions, configs, and
    artifacts — and prove it by re-running.
  - **Report trace:** §19.2(5); §9.2; §6.5.
  - **Inputs / prerequisites:** P8.04 contracts; P8.19 benchmark set.
  - **Steps:**
    1. Define the reproducibility record per run: benchmark-set version,
       solver versions, PyTorch/PyG/RLlib/ANDES versions, model artifact
       references, dataset version and preprocessing version, seeds,
       configs, input snapshot IDs.
    2. Store run artifacts (configs, seeds, model files, outputs) in
       object storage (§6.5) with artifact references in the results.
    3. Implement the rerun test: a second engineer re-runs a selected
       result from the record alone on a clean environment and confirms
       reproduction within the tolerance recorded for that module *before*
       the rerun (tolerances are declared per module in the
       reproducibility standard — see P8.38 — never fitted after the
       rerun).
    4. Record the rerun outcome; any result that does not reproduce is
       marked as such and excluded from the benchmark report's claims.
  - **Deliverable / output:** Reproducibility standard + artifact store +
    rerun-test records.
  - **Acceptance criteria / exit check:** Every reported result has a
    complete reproducibility record; at least one result per module passed
    the rerun test; non-reproducing results are marked and excluded.
  - **Owner role:** Research lead.
  - **Dependencies:** P8.04, P8.19.
  - **Verification method:** The rerun test itself, performed by a second
    engineer.
  - **Risks / failure modes:** Seeds recorded but the data pipeline not
    (same seed, different data); GPU nondeterminism unrecorded and
    presented as exact reproduction; artifacts stored only on the
    researcher's machine.

## P8.21 — Topology-aware optimization research (transfer and scaling)

- [x] **P8.21 — Topology-aware optimization research (transfer and scaling)** — implemented, awaiting human build/test (decision record: `ledger/phase-08-advanced-coordination/21_topology_research.md` — protocol + EMPTY measured cells; depends on P8.09/P8.12 fills, the P8.35 hold-out confirmation, and the P8.06 threshold pre-registration)
  - **Objective:** Run the topology-aware research thread: measure how the
    GNN and ADMM approaches transfer across topologies and scale with
    problem size — and where they stop transferring or scaling.
  - **Report trace:** §13.1 (topology embedding and transfer learning);
    §2.1 (v2.x topology-aware optimization research); §19.3–§19.4.
  - **Inputs / prerequisites:** P8.12, P8.09.
  - **Steps:**
    1. Define the transfer experiment: train on one set of topologies,
       evaluate on unseen ones (varying size, meshing, DER mix); record
       the transfer metric per topology.
    2. Define the scaling experiment: problem size vs runtime and vs
       solution quality for the research modules and the centralized
       baseline on the same cases.
    3. Report the transfer/scaling limits honestly: the topology size and
       class beyond which the approach degrades, with the measured numbers.
    4. Do not claim capability for a 10,000-bus utility network from
       feeder-scale tests (§19.3).
  - **Deliverable / output:** Transfer/scaling evaluation report.
  - **Acceptance criteria / exit check:** Transfer measured on unseen
    topologies with per-topology results; scaling measured against the
    centralized baseline; limits stated with numbers; no scale claim
    beyond the tested range.
  - **Owner role:** Research engineer (ML/optimization).
  - **Dependencies:** P8.12, P8.09.
  - **Verification method:** Independent re-run of one transfer case and
    one scaling point.
  - **Risks / failure modes:** Transfer claimed from topologies that differ
    only trivially from training ones; scaling extrapolated linearly from
    two points.

## P8.22 — Portfolio analytics research (derived reporting, not dispatch)

- [x] **P8.22 — Portfolio analytics research (derived reporting, not dispatch)** — implemented, awaiting human build/test (decision record: `ledger/phase-08-advanced-coordination/22_portfolio_analytics.md`)
  - **Objective:** Build the portfolio-analytics research outputs
    (settlement, performance, KPI derivations) as reporting-only plugins
    that never influence dispatch.
  - **Report trace:** §5.3 (analytics/report category); §19.4
    (flexibility metrics).
  - **Inputs / prerequisites:** P8.01; P7.15 profiles.
  - **Steps:**
    1. Define the analytics: available vs delivered MW/MWh, uncertainty
       calibration, shortfall (§19.4 flexibility metrics), and any
       settlement/performance derivations — each with its formula, inputs,
       and uncertainty treatment.
    2. Implement as reporting plugins behind the SDK (Phase 7), with no
       path to the dispatch lifecycle.
    3. Validate the analytics against the deterministic baseline's
       accounting on the same scenarios; report discrepancies.
    4. Record the analytics' limits (e.g. calibration assessed on the
       tested regimes only).
  - **Deliverable / output:** Analytics plugins + validation record.
  - **Acceptance criteria / exit check:** Analytics reproduce the baseline
    accounting within stated tolerance on the tested scenarios; no dispatch
    influence (verified by inspection); limits recorded.
  - **Owner role:** Research engineer.
  - **Dependencies:** P8.01.
  - **Verification method:** Recompute one analytic from raw scenario data
    independently and compare.
  - **Risks / failure modes:** An analytic that silently becomes a dispatch
    input (e.g. a "flexibility score" used to rank dispatch candidates
    without validation); uncertainty calibration claimed beyond the tested
    regimes.

## P8.23 — Cross-module comparison: one report, all modules, same cases, same metrics

- [x] **P8.23 — Cross-module comparison: one report, all modules, same cases, same metrics** — implemented, awaiting human build/test (decision record: `ledger/phase-08-advanced-coordination/23_cross_module_comparison.md` — ONE report template under the same-cases/same-metrics contract (refuse rather than merge); case×module matrix with every measured cell EMPTY, every row citing flexcore-research-benchmark-set-v1; filling gated on the P8.06/P8.09/P8.12/P8.15/P8.17 fills; no winner claim possible or pre-stated)
  - **Objective:** Produce the single cross-module comparison: ADMM/DMPC,
    GNN, and MARL each against the frozen baselines on the same cases with
    the same metrics — so the modules are comparable and none is judged on
    a private benchmark.
  - **Report trace:** §23 Stage 8 (exit gate); §19.4.
  - **Inputs / prerequisites:** P8.09, P8.12, P8.15, P8.06, P8.19.
  - **Steps:**
    1. Assemble the comparison table: rows are benchmark cases, columns
       are modules + baselines, cells are the §19.4 metrics for that
       module (objective, feasible count, runtime, violations, residuals,
       return, robustness).
    2. Use identical cases, seeds, and metric definitions for every column;
       any cell that could not be run is marked "not run" with a reason,
       never left blank or filled with another module's number.
    3. Include the independent validation outcome per cell (P8.05).
    4. Write the comparison narrative: which module wins where, where all
       lose to the deterministic baseline, and what that implies for the
       next research step.
  - **Deliverable / output:** Cross-module comparison report.
  - **Acceptance criteria / exit check:** Same cases/seeds/metrics for all
    modules; no missing cells without a reason; validation outcomes
    present; narrative reports losses as well as wins; a failed spot-check
    (verification method) blocks the comparison report until the cell is
    corrected or re-run.
  - **Owner role:** Research lead.
  - **Dependencies:** P8.09, P8.12, P8.15, P8.06, P8.19.
  - **Verification method:** Spot-check three cells by re-running them from
    their recorded inputs.
  - **Risks / failure modes:** Each module judged on its favourite cases;
    a metric defined differently per module; a "not run" hidden as a blank.

## P8.24 — Failure-mode and robustness drill (research modules)

- [x] **P8.24 — Failure-mode and robustness drill (research modules)** — implemented, awaiting human build/test (decision record: `ledger/phase-08-advanced-coordination/24_failure_drill.md`)
  - **Objective:** Run a combined drill over the §19.2(6) fault set against
    the research workers and record what each fault does to their outputs.
  - **Report trace:** §19.2(6); §13.3 (delayed/lost messages, uncertain
    state estimates).
  - **Inputs / prerequisites:** P8.08, P8.11, P8.14.
  - **Steps:**
    1. Drill each fault: dropped/reordered/duplicated messages, clock skew,
       stale telemetry, unavailable assets, solver timeout, plugin/worker
       crash, partial fleet response, communication loss between ADMM
       participants.
    2. For each fault and module record: behaviour, whether the output was
       flagged/rejected/fell back, and whether any invalid proposal reached
       the validation gate.
    3. Fix every fault that lets an invalid proposal pass as valid or that
       crashes the control plane; re-drill until clean.
  - **Deliverable / output:** Drill report with per-fault-per-module table.
  - **Acceptance criteria / exit check:** Every fault/module cell has a
    recorded outcome; zero faults let an invalid proposal pass validation
    or crash the control plane.
  - **Owner role:** Research lead.
  - **Dependencies:** P8.08, P8.11, P8.14.
  - **Verification method:** Independent witness re-runs two fault/module
    cells and reproduces the outcomes.
  - **Risks / failure modes:** Drill run only on the happy-path simulator;
    a worker crash that takes down the validation harness with it.

## P8.25 — Security review of research workers (untrusted code, data, and models)

- [x] **P8.25 — Security review of research workers (untrusted code, data, and models)** — implemented, awaiting human build/test (decision record: `ledger/phase-08-advanced-coordination/25_security_review.md`)
  - **Objective:** Review the research workers as untrusted code paths:
    model files, training data, and worker outputs are all treated as
    untrusted input to the protected platform.
  - **Report trace:** §19.2(7); §17.4; §5.7; §9.3.
  - **Inputs / prerequisites:** P8.11, P8.14, P8.16; P7.29 security report.
  - **Steps:**
    1. Test model-file handling: malformed/corrupted model artifacts,
       oversized models, models with unexpected ops — each must fail safely
       without executing arbitrary code or crashing the host. Name the
       allowed model formats in writing and forbid arbitrary-code
       deserialization (unguarded pickle or equivalent) for every format
       used; a format that cannot be loaded without executing code is not
       an allowed format.
    2. Test training-data handling: poisoned or malformed training records
       must not silently corrupt the experiment record (record the data
       validation in place).
    3. Test worker-output handling: oversized, malformed, or
       out-of-range research outputs must be rejected by the contract
       validation (P8.04), never passed to the feasibility gate.
    4. Record findings; every high/critical finding blocks the exit gate
       until fixed and re-tested.
  - **Deliverable / output:** Research security-review report.
  - **Acceptance criteria / exit check:** All three input classes tested;
    zero open high/critical findings; no model file can execute code on
    load (deserialization safety verified for the formats used).
  - **Owner role:** Security engineer.
  - **Dependencies:** P8.11, P8.14, P8.16.
  - **Verification method:** Independent re-run of the hostile-input cases.
  - **Risks / failure modes:** A model format with arbitrary-code
    deserialization (e.g. unguarded pickle) used for artifacts; a research
    output that bypasses contract validation because it arrives over an
    internal channel.

## P8.26 — Benchmark paper/report (the Stage 8 deliverable)

- [x] **P8.26 — Benchmark paper/report (the Stage 8 deliverable)** — implemented, awaiting human build/test (decision record: `ledger/phase-08-advanced-coordination/26_benchmark_paper.md` — the paper TEMPLATE: methodology + benchmark-set-v1 citation + comparability contract + reproducibility standard written; every per-module/cross-module/dynamic/transfer result cell EMPTY; limitations + not-tested list (NT-1..NT-11) FILLED from the frozen artifact; conclusions explicitly conditional; independent review record UNRUN)
  - **Objective:** Write the benchmark paper/report (§23 Stage 8): methods,
    baselines, results, limits, and reproducibility — honest about losses
    and about what was not tested.
  - **Report trace:** §23 Stage 8; §19.4; §27 (references).
  - **Inputs / prerequisites:** P8.09, P8.12, P8.15, P8.17, P8.21, P8.23,
    P8.20.
  - **Steps:**
    1. Write the report: research questions (P8.02), methods, benchmark-set
       version, baselines (frozen, P8.06), per-module results, cross-module
       comparison (P8.23), dynamic results (P8.17), transfer/scaling
       (P8.21), limits, not-tested list (P8.19), reproducibility records
       (P8.20).
    2. Cite external claims to primary sources; label every assumption as
       an assumption (AGENTS.md rule 1; `.agent/WRITING.md`).
    3. State the honesty boundary in the report itself: proposals only
       (ADR-010), no safety guarantee from finite tests, no field
       authority, no global optimality outside stated assumptions.
    4. Have the report reviewed by an independent reviewer (not a module
       author) before it is called final; record the review.
  - **Deliverable / output:** Benchmark paper/report + independent review
    record.
  - **Acceptance criteria / exit check:** Report exists and covers every
    input listed above; losses and not-tested items are present; honesty
    boundary stated in the report; independent review recorded with no
    unaddressed major comment.
  - **Owner role:** Research lead.
  - **Dependencies:** P8.09, P8.12, P8.15, P8.17, P8.21, P8.23, P8.20.
  - **Verification method:** Reviewer checks each claimed number against
    its evaluation report; any number without a source fails.
  - **Risks / failure modes:** A report that reads as a product claim
    ("FlexCore now does safe MARL dispatch"); results reported without the
    benchmark-set version, making them unverifiable.

## P8.27 — Operator console: research and shadow-mode surfacing

- [x] **P8.27 — Operator console: research and shadow-mode surfacing** — implemented, awaiting human build/test (decision record: `ledger/phase-08-advanced-coordination/27_console_research.md` — read-only GET /api/v1/research/status + Research workspace section mirroring the P7.25 pattern; RECORDED Phase 8 status with every run fact an honest gap (TEMPLATE_EMPTY / NO_RUN / zero proposals); proposals-only labels, simulation-shadow mode lock, confirmation-gated LOCAL promotion request only; UI walkthrough + negative test UNRUN)
  - **Objective:** Show research proposals and shadow-mode comparisons in
    the console (Simulation Lab workspace) — always labelled as proposals,
    never as executed actions.
  - **Report trace:** §20.1 (Simulation Lab); §20.2; §19.2(9).
  - **Inputs / prerequisites:** P8.14 shadow logger; P8.23 comparison;
    Phase 5 console.
  - **Steps:**
    1. Extend the Simulation Lab workspace: research proposals with their
       provenance (module/model version, snapshot, validation outcome),
       shadow comparisons vs the deterministic baseline, and the
       validation result for each proposal.
    2. Label every research output as a proposal/simulation result;
       distinguish it visually from validated schedules and executed
       commands (§20.2).
    3. Show the safety-filter statistics and infeasibility outcomes for
       MARL, and the residual/termination status for ADMM.
    4. Require explicit confirmation for any action that would promote a
       research proposal toward the dispatch path (and record that such
       promotion still requires the protected gates and, for any field use,
       per-feature qualification).
    5. Test the promotion path negatively: attempt to promote a research
       proposal toward dispatch without the protected validation →
       feasibility → authorization gates and confirm it is refused and
       audited; confirm the console cannot issue, authorize, or fast-track
       a proposal.
  - **Deliverable / output:** Console changes + UI test record.
  - **Acceptance criteria / exit check:** An operator can distinguish a
    research proposal from a validated schedule and from an executed
    command; provenance and validation outcome are visible per proposal;
    no research output is displayed as executed; a promotion attempt that
    skips a protected gate is refused and audited.
  - **Owner role:** Frontend engineer.
  - **Dependencies:** P8.14, P8.23.
  - **Verification method:** Operator walkthrough against the lab; confirm
    each state is distinguishable.
  - **Risks / failure modes:** A shadow proposal displayed alongside real
    dispatch data without a label; a "promote" button that skips the
    protected gates.

## P8.28 — CI gates for research code (tests, lint, reproducibility checks)

- [x] **P8.28 — CI gates for research code (tests, lint, reproducibility checks)** — implemented, awaiting human build/test (decision record: `ledger/phase-08-advanced-coordination/28_ci_gates.md` — four ADDITIVE ci.yml jobs: research-python-gates + research-worker-conformance behind the fail-closed UNPINNED-REVERIFY pin gate (pin gap recorded, no version invented), research-rust-harness-gates (cargo test -p flexcore-research + the frozen artifact STRUCTURAL check), research-reproducibility-check fail-closed stub (UNRUN until the frozen baseline artifact + P8.38 pre-registrations exist); green run UNRUN — human gate input)
  - **Objective:** Wire the §22.2 CI gates to the research code and add
    reproducibility checks: pinned versions, seeded runs, and the
    conformance suite for research workers.
  - **Report trace:** §22.2; §19.2; §9.3.
  - **Inputs / prerequisites:** P8.04, P8.20, P7.27 CI.
  - **Steps:**
    1. Extend CI: Python Ruff + type checks + pytest for research workers;
       Rust gates for the harness; scenario regression suite including the
       research benchmark cases.
    2. Add a CI reproducibility check: a seeded benchmark run reproduces
       the frozen P8.06 baseline artifact within the pre-recorded
       tolerance (P8.20) — the check compares against the frozen artifact,
       never against its own output.
    3. Add the research-worker conformance suite (P7.21 extended to
       research contracts) as a CI job.
    4. Record a green CI run on the phase branch.
  - **Deliverable / output:** CI configuration + green run record.
  - **Acceptance criteria / exit check:** All §22.2 gates run on the new
    code and pass; the seeded reproducibility check passes in CI; the
    research conformance suite runs in CI.
  - **Owner role:** DevOps.
  - **Dependencies:** P8.04, P8.20.
  - **Verification method:** Inspect the CI run; break a seed/version pin
    on a scratch branch and confirm CI fails.
  - **Risks / failure modes:** CI that runs unit tests but never the
    benchmark cases; a reproducibility check that passes because it
    compares against itself.

## P8.29 — Per-feature qualification record: what would field use require (and why it is not granted here)

- [x] **P8.29 — Per-feature qualification record: what would field use require (and why it is not granted here)** — implemented, awaiting human build/test (decision record: `ledger/phase-08-advanced-coordination/29_qualification_record.md`)
  - **Objective:** Write, per research module, the qualification record
    stating what field use would require — and record explicitly that this
    phase grants none of it.
  - **Report trace:** §2.1 (v1.x/v2.x per-feature qualification); §22.3;
    §23 Stage 9.
  - **Inputs / prerequisites:** P8.23, P8.26.
  - **Steps:**
    1. For each module, list the qualification steps field use would need:
       site-specific validation, HIL tests, cyber review, commissioning,
       market/interconnection qualification where applicable, and the
       Stage 9 gate (§22.3, §23 Stage 9).
    2. Record the current status of each step (not started) and the
       evidence that would close it.
    3. State in the record: no module in this phase is qualified for field
       use; shadow/simulation is the ceiling (§2.1 v2.x).
    4. Route the record to product for the per-feature qualification
       decision (a later decision, not this phase's).
  - **Deliverable / output:** Per-feature qualification record.
  - **Acceptance criteria / exit check:** Every module has a qualification
    list with statuses; the "not qualified" statement is present per
    module; the record is routed to product with a date.
  - **Owner role:** Research lead + product.
  - **Dependencies:** P8.23, P8.26.
  - **Verification method:** Reviewer confirms no module's record claims
    or implies qualification.
  - **Risks / failure modes:** A qualification list that reads as a
    checklist already half-done; field use granted by implication in a
    later phase because this record was never written.

## P8.30 — Phase retrospective, open items, and exit-gate record

- [x] **P8.30 — Phase retrospective, open items, and exit-gate record** — implemented, awaiting human build/test (decision record: `ledger/phase-08-advanced-coordination/30_retrospective_exit_gate.md`)
  - **Objective:** Close the phase honestly: record what passed, what did
    not, every open item with an owner, and the exit-gate position.
  - **Report trace:** §22.3; §23 Stage 8.
  - **Inputs / prerequisites:** P8.01–P8.39; CHECKLIST.md.
  - **Steps:**
    1. Complete CHECKLIST.md: every exit-gate item ticked with evidence,
       or explicitly failed with a reason.
    2. List deferred work with owners: WASI 0.3-dependent research
       workers (if any), HELICS federation extensions, additional test
       systems, and any module whose evaluation was inconclusive.
    3. Record the research questions that remain open and the next
       experiment each needs (P8.02 plan updated).
    4. State plainly what this phase is not: not a product feature, not a
       safety case, not field-qualified; research proposals only.
  - **Deliverable / output:** Retrospective + open-items register +
    exit-gate record.
  - **Acceptance criteria / exit check:** CHECKLIST.md complete with
    evidence links; every open item has an owner; the "research only, not
    field-qualified" statement is present in plain language.
  - **Owner role:** Research lead.
  - **Dependencies:** All P8.01–P8.39.
  - **Verification method:** Reviewer reads CHECKLIST.md and the
    retrospective and can trace every claim to a deliverable from
    P8.01–P8.29.
  - **Risks / failure modes:** A phase declared done with inconclusive
    evaluations presented as positive results; open research questions
    dropped without a record.

---


## P8.31 — Research-worker tenant isolation and scoped data access

- [x] **P8.31 — Research-worker tenant isolation and scoped data access** — implemented, awaiting human build/test (decision record: `ledger/phase-08-advanced-coordination/31_tenant_isolation.md`)
  - **Objective:** Scope every research worker to a tenant/site and prove
    a worker cannot read or serve another tenant's data — the P8 ledger
    previously contained zero tenant-isolation requirements.
  - **Report trace:** §17.4 (cross-tenant leakage); §16.2 (multi-tenancy);
    §5.7.
  - **Inputs / prerequisites:** P8.04 contracts; P7.11 broker; P7.12
    matrix.
  - **Steps:**
    1. State the scoping rule per worker (ADMM/DMPC, GNN, MARL, ANDES,
       HELICS, analytics): each worker request carries a tenant/site
       scope; training data, model artifacts, and inference inputs are
       all scoped.
    2. Test cross-tenant reads: a worker scoped to tenant A attempts to
       read tenant B telemetry/topology/training data; every attempt is
       denied and audited.
    3. Test model transfer: a GNN trained on tenant A's topologies/data
       must not serve tenant B without a recorded approval; confirm the
       serving path refuses unapproved cross-tenant use.
    4. Record the scoping rule in each worker's contract (P8.04).
  - **Deliverable / output:** Worker scoping rule + cross-tenant negative
    test record.
  - **Acceptance criteria / exit check:** Every worker has a recorded
    scope; all cross-tenant attempts denied and audited; unapproved
    cross-tenant model serving refused.
  - **Owner role:** Security engineer + research lead.
  - **Dependencies:** P8.04.
  - **Verification method:** Independent re-run of the cross-tenant suite.
  - **Risks / failure modes:** Scoping enforced at inference but not at
    training-data selection; a shared model artifact that leaks tenant
    A's data through its weights with no treatment recorded.

## P8.32 — Model-artifact signing, provenance, and verification at load

- [x] **P8.32 — Model-artifact signing, provenance, and verification at load** — implemented, awaiting human build/test (decision record: `ledger/phase-08-advanced-coordination/32_model_artifact_signing.md` — the digest-and-record signing layer for model artifacts in `crates/flexcore-research/src/artifact_signing.rs` (provenance-v1 mirror of kernel P7.14 by citation: versioned records with LINKED training provenance, fail-closed `NoArtifactVerifier` default, append-only revocation, the composed load gate that runs the P8.31 serving register first and refuses unsigned/mismatched/revoked artifacts with named errors, audit on every outcome); the Python worker load sites (F-1/F-2/F-3) get the verification wired at their with-framework activation wave — the end-to-end load path is NOT live; no crypto crate added (production verifier recorded as a decision point); WRITTEN-UNCOMPILED, negative suites written-unrun)
  - **Objective:** Require model artifacts to be signed and verified at
    load, so a tampered model swapped into object storage is rejected
    instead of trusted — the research equivalent of P7.14 plugin signing,
    which previously had no counterpart for models.
  - **Report trace:** §9.3 (plugin compatibility and supply chain, by
    analogy); §17.4 (supply-chain compromise); §6.5 (artifact storage).
  - **Inputs / prerequisites:** P8.11 GNN worker; P8.14 MARL worker;
    P8.20 artifact store; P7.14 signing scheme (as reference).
  - **Steps:**
    1. Define the signing scheme for model artifacts: which keys sign,
       how signatures are verified at load, and how revocation is handled.
    2. Implement verification in every worker's load path: an unsigned or
       tampered artifact is refused before inference/training use, with
       the refusal audited.
    3. Record provenance per artifact: training-data reference, training
       config/seed, framework versions — linked to the signature, not
       stored separately where it can drift.
    4. Test: tamper with a signed artifact and attempt load; attempt load
       of an unsigned artifact; attempt use of a revoked artifact.
  - **Deliverable / output:** Model signing/verification implementation +
    negative-test record.
  - **Acceptance criteria / exit check:** Tampered, unsigned, and revoked
    artifacts are all refused at load and audited; every loaded artifact
    carries linked provenance.
  - **Owner role:** Security engineer + research engineer (ML).
  - **Dependencies:** P8.11, P8.14, P8.20.
  - **Verification method:** Tamper with a signed artifact and attempt
    load; confirm refusal.
  - **Risks / failure modes:** Verification that warns instead of refusing;
    provenance stored apart from the artifact and silently mismatched.

## P8.33 — Research-worker identity, mTLS, and secret handling

- [x] **P8.33 — Research-worker identity, mTLS, and secret handling** — implemented, awaiting human build/test (decision record: `ledger/phase-08-advanced-coordination/33_identity_mtls_secrets.md` — the identity model in `crates/flexcore-research/src/worker_identity.rs` (deployment-bound identities over the closed P8.31 worker-family vocabulary, config-shaped mTLS posture mirroring plugin-host transport_trust.rs rule-for-rule — plaintext refused, pinned CA/server name, short-lived TTL, recorded rotation owner, revocation checking configured — with the rustls/gRPC wiring recorded as a seam, nothing installed; append-only identity revocation with the fail-closed admission door; secrets as scheme-prefixed REFERENCES ONLY (env:/file:/secret-store:, kernel manifest rule) with the whole-tree worker-config forbidden-key scan; the gRPC binding of the research workers is the recorded Phase 7 follow-up (S-1)); WRITTEN-UNCOMPILED, negative suites written-unrun)
  - **Objective:** Give every research worker a service identity with
    mTLS to the control plane, and state how it obtains credentials —
    without secrets in worker config, environment, or training scripts.
  - **Report trace:** §17.3 (mTLS, short-lived credentials, secrets
    manager; no secrets in plugin config or source control); §5.4 Class C.
  - **Inputs / prerequisites:** P8.04 contracts; P7.17 template; P7.34
    (plugin-host identity, as reference).
  - **Steps:**
    1. Assign each research worker a service identity with mTLS,
       short-lived credentials, rotation, and revocation; record issuance
       and rotation owner.
    2. Route all credential needs (dataset/model-store access, simulator
       access) through secret references resolved by the host — never as
       values in worker config, environment variables, or training scripts.
    3. Test: a worker presenting a revoked identity is refused; a worker
       config containing a literal secret value is rejected at load and
       the attempt audited; no secret appears in worker logs.
  - **Deliverable / output:** Worker identity design + secret-handling
    record + negative tests.
  - **Acceptance criteria / exit check:** mTLS + rotation/revocation
    implemented; revoked identity refused; literal-secret config rejected;
    logs contain no secrets.
  - **Owner role:** Security engineer.
  - **Dependencies:** P8.04.
  - **Verification method:** Inspect worker configs and logs for secret
    values; attempt connection with a revoked identity.
  - **Risks / failure modes:** Training scripts that embed dataset
    credentials because "research is offline"; mTLS on the control-plane
    path but plaintext on the worker-to-artifact-store path.

## P8.34 — Proposal validity windows and stale-proposal rejection

- [x] **P8.34 — Proposal validity windows and stale-proposal rejection** — implemented, awaiting human build/test (decision record: `ledger/phase-08-advanced-coordination/34_proposal_windows.md`)
  - **Objective:** Give every research proposal a validity window and
    prove a stale proposal is rejected — so a proposal computed against
    old conditions cannot look current.
  - **Report trace:** §14.3 (valid-from/valid-until on every command);
    §17.5 invariant 5 (no stale or expired command); §13.4.
  - **Inputs / prerequisites:** P8.04 contracts; P8.05 harness.
  - **Steps:**
    1. Extend the P8.04 result contract: every research proposal carries
       `valid-from`/`valid-until` and the input snapshot ID it was
       computed against.
    2. Enforce in the validation hook: a proposal whose window has elapsed
       or whose snapshot is superseded is rejected by the same gate as an
       infeasible proposal — never passed to feasibility as if fresh.
    3. Test: submit an expired proposal, a proposal against a superseded
       snapshot, and a proposal with a missing window; confirm rejection
       in each case.
  - **Deliverable / output:** Validity-window contract extension + test
    record.
  - **Acceptance criteria / exit check:** Every proposal carries a window
    and snapshot ID; expired/superseded/missing-window proposals are all
    rejected.
  - **Owner role:** Research lead + Rust engineer.
  - **Dependencies:** P8.04, P8.05.
  - **Verification method:** Submit the three hostile proposals and record
    the rejections.
  - **Risks / failure modes:** A window that is set from the proposal's own
    clock with no bound; validity checked at proposal time but not at
    gate time.

## P8.35 — Training-data provenance, license, and poisoning controls

- [x] **P8.35 — Training-data provenance, license, and poisoning controls** — implemented, awaiting human build/test (decision record: `ledger/phase-08-advanced-coordination/35_training_data_provenance.md` — the training-data provenance STANDARD + the held-out-topology manifest `apps/worker/python/training-data-manifest-v1.json` (flexcore-research/training-data-manifest.v1): the P8.10 section-4 hold-out RECOMMENDATION CONFIRMED at the record level with findings F-1 (lib-06/07/08 share the three_bus_radial fixture with the training side — regime hold-out, not topology) and F-2 (the meshed lib-03 family excluded from the declared balanced-radial scope; D3 OPEN); license register + poisoning controls as RECORDED PROCEDURES (quarantine-with-a-record; the poisoning-detection run is a HUMAN GATE, UNRUN); every per-instance cell EMPTY — no dataset exists)
  - **Objective:** Record where every training dataset comes from, under
    what license, at what version — and prove malformed or poisoned
    records cannot silently corrupt the experiment record.
  - **Report trace:** §9.3 (supply chain, by analogy); §19.3–§19.4;
    §13.1 (training-data scope).
  - **Inputs / prerequisites:** P8.03 license audit; P8.10 GNN design;
    P8.13 MARL design; P8.20 artifact store.
  - **Steps:**
    1. Inventory every training dataset per module: source, license,
       version/snapshot ID, generation method (simulator version + seeds
       where synthetic), and the leakage controls it is subject to.
    2. Record the license consequence per dataset (distribution/use
       limits) alongside the P8.03 code-dependency audit; a dataset whose
       license forbids the intended use is excluded, not used quietly.
    3. Implement data validation at ingestion: schema, range, and
       duplication checks; poisoned/malformed records are quarantined
       with a record, never silently dropped or silently trained on.
    4. Version the dataset with the model artifact (P8.32) so a rerun
       (P8.20/P8.38) trains on the same data.
  - **Deliverable / output:** Training-data inventory + validation record.
  - **Acceptance criteria / exit check:** Every dataset has source/license/
    version recorded; every license consequence has a disposition;
    poisoned/malformed records are quarantined with a record; dataset
    version is linked to the model artifact.
  - **Owner role:** Research lead.
  - **Dependencies:** P8.03, P8.10, P8.13.
  - **Verification method:** Re-run ingestion on a dataset with injected
    poisoned records and confirm quarantine; check the inventory against
    the artifact store.
  - **Risks / failure modes:** A dataset "borrowed" without a license
    check; silently dropped bad records that change the training
    distribution without a record.

## P8.36 — Research-stack CVE/advisory check at pin time

- [x] **P8.36 — Research-stack CVE/advisory check at pin time** — implemented, awaiting human build/test (decision record: `ledger/phase-08-advanced-coordination/36_cve_pin_check.md`)
  - **Objective:** Confirm the pinned research-stack versions carry no
    known critical vulnerability at implementation time — the check P8
    `RESEARCH.md` §7 task 4 assigns to P8.25, which P8.25 never performs.
  - **Report trace:** §22.2 (license/security review; container/
    dependency scanning); §17.3 (vulnerability management).
  - **Inputs / prerequisites:** P8.03 audit; P8.25 security review;
    RESEARCH.md §7 task 4.
  - **Steps:**
    1. For each pinned research dependency (PyTorch, PyG, Ray/RLlib,
       Gymnasium, ANDES, HELICS, pandapower), check the advisory
       databases (GHSA/OSV/vendor advisories) at pin time and record the
       result per version.
    2. Any known critical/high issue blocks use of that version until a
       fixed version is pinned or a compensating measure is recorded with
       an owner — record the decision either way.
    3. Re-run the check when any research version is re-pinned; record the
       check date with the pin.
  - **Deliverable / output:** Research CVE-check record per pinned version.
  - **Acceptance criteria / exit check:** Every pinned version has a
    recorded advisory check with a date; zero known critical/high issues
    are open without a recorded decision and owner.
  - **Owner role:** Security engineer.
  - **Dependencies:** P8.03.
  - **Verification method:** Re-run one advisory lookup independently and
    compare.
  - **Risks / failure modes:** A check run once at audit time and never
    repeated when versions move; an advisory database that does not cover
    the framework in question, recorded as "no issues found".

## P8.37 — Unit and property tests for research workers

- [x] **P8.37 — Unit and property tests for research workers** — implemented, awaiting human build/test (decision record: `ledger/phase-08-advanced-coordination/37_research_tests.md` — unit suites mapped per worker (the existing pytest mirrors) + NEW hand-written fixed-seed property suites (stdlib-only generators; no property-testing library installed): marl P-VOCAB/P-ENVELOPE/P-SCAN/P-REFUSE/P-STATS; gnn P-SCOPE (exact-equality scope enforcement, near-misses never pass)/P-GRAPH/P-FEATURE/P-OVERLAP; reproducibility P-SEED/P-SCHEMA/P-RERUN; portfolio P-REPORT/P-ENERGY/P-CALIB; andes P-REFUSE/P-EXTREME/P-ROCOF/P-SETTLE/P-DAMP — each property states its oracle and its fixed seed. WRITTEN-NEVER-RUN: the pytest and CI runs are human gates; per-worker coverage FIGURES land at the first run; the Rust ADMM provider's property suite is the crates owner's recorded follow-up (concurrent file ownership)
  - **Objective:** Require unit and property tests for the research
    workers themselves (§19.2(1)–(2)), not only failure-mode tests and CI
    pytest runs — so the lowest test layers exist for the code the
    evaluations rest on.
  - **Report trace:** §19.2(1) (unit tests); §19.2(2) (property tests);
    §10.2 (units/signs).
  - **Inputs / prerequisites:** P8.07, P8.11, P8.14; P8.28 CI.
  - **Steps:**
    1. Write unit tests per worker: units and sign conventions (§10.2),
       solver input/output validation, state transitions, error shapes,
       boundary values (SOC 0/1, zero-length horizons, empty action
       sets).
    2. Write property tests: invariant preservation across generated
       asset states, timestamps, command sequences, and constraints
       (e.g. a worker never returns a setpoint outside the declared
       capability envelope; a warm start never changes the problem
       definition).
    3. Run both suites in CI (P8.28) with coverage recorded per worker;
       a worker with no unit/property suite fails the phase's test-layer
       check.
  - **Deliverable / output:** Unit + property test suites per research
    worker + CI run record.
  - **Acceptance criteria / exit check:** Every research worker has a unit
    suite and a property suite; both run in CI; coverage per worker is
    recorded.
  - **Owner role:** Research engineer (per module).
  - **Dependencies:** P8.07, P8.11, P8.14.
  - **Verification method:** Inspect the CI run; run one property suite
    with a fresh seed and confirm it passes.
  - **Risks / failure modes:** Property tests that only generate the
    happy-path distribution; unit tests that mock the solver instead of
    validating its I/O.

## P8.38 — Training-run reproducibility and GPU-nondeterminism disclosure

- [x] **P8.38 — Training-run reproducibility and GPU-nondeterminism disclosure** — implemented, awaiting human build/test (decision record: `ledger/phase-08-advanced-coordination/38_gpu_nondeterminism.md` — the disclosure standard completing P8.20: per-module tolerance-CLASS rule declared (exact for the deterministic modules and solver baselines; within_tolerance for ML INFERENCE reruns; distribution for TRAINING reruns), tolerance VALUES EMPTY — registered before each module's first rerun (X-8), never fitted after; nondeterminism-source register as recorded RISKS (GPU kernel reduction order, atomics, autotuned library algos, parallel data-loading order, framework threading, version sensitivity — nothing installed to verify; the torch/PyG and RL-framework DECISION-POINTs carried OPEN); mitigation procedure (verbatim seed derivation, deterministic-mode flags, CPU fallback, thread/version pinning); the disclosure block structurally enforced on every record (NONDETERMINISM_SOURCES_REQUIRED); rerun set defined, UNRUN — every measured cell EMPTY)
  - **Objective:** Make the P8.20 rerun test honest about what reproduces:
    distinguish inference reruns from training reruns, pre-record
    tolerances per module, and disclose GPU nondeterminism instead of
    presenting training results as exactly reproducible.
  - **Report trace:** §19.2(5); §9.2; §6.5.
  - **Inputs / prerequisites:** P8.20 reproducibility standard; P8.06
    frozen baselines; P8.10/P8.13 experiment designs.
  - **Steps:**
    1. Declare per module, before any rerun: the tolerance for an
       inference rerun and the tolerance (or distribution) for a training
       rerun, with the basis for each — never fitted after the rerun.
    2. Run the rerun set: at least one inference rerun per module, at
       least one training rerun where training is claimed reproducible,
       and one GNN held-out-topology inference rerun from artifacts
       (so the P8.12 headline metric itself is re-proven).
    3. Record nondeterminism sources per run (GPU nondeterminism,
       parallel data loading order, framework version sensitivity) and
       state which results are exact-reproduction claims and which are
       distribution/tolerance claims.
    4. Any result that does not reproduce within its pre-recorded
       tolerance is marked as such and excluded from the benchmark
       report's claims (P8.26).
  - **Deliverable / output:** Training/inference rerun record per module +
    nondeterminism disclosure.
  - **Acceptance criteria / exit check:** Tolerances pre-recorded per
    module; rerun set complete (inference per module, training where
    claimed, one GNN held-out rerun); nondeterminism disclosed per run;
    non-reproducing results excluded from claims.
  - **Owner role:** Research lead.
  - **Dependencies:** P8.20, P8.06.
  - **Verification method:** Second engineer performs one training rerun
    and one inference rerun from the records alone.
  - **Risks / failure modes:** A training rerun compared with an inference
    tolerance and declared a failure (or vice versa); GPU nondeterminism
    discovered only when a reviewer asks.

## P8.39 — Co-simulation and dynamic cross-checks

- [x] **P8.39 — Co-simulation and dynamic cross-checks** — implemented, awaiting human build/test (decision record: `ledger/phase-08-advanced-coordination/39_cosim_crosschecks.md` — the cross-check DESIGN written BEFORE any run (agreement evidence on the overlapping domain only, never validation against reality; X-5 structural separation of steady-state and dynamic blocks enforced in code; HELICS transport not physics) + EMPTY result tables CC-1..CC-4 (every measured cell EMPTY — the ANDES/pandapower/HELICS runs are human/lab gate inputs; the X-8 tolerance registry EMPTY until the pre-run dated amendment); pure stdlib cross-check module `apps/worker/python/andes_worker/cosim_crosschecks.py` + pytest mirror, WRITTEN-UNRUN with hand-computed self-tests; refusals: mismatched time bases/index bases, undeclared sign conventions, unregistered tolerances, merge keys, federate dropout unnamed; contract gaps recorded)
  - **Objective:** Test the two cross-checks P8.16/P8.18 note as risks but
    never test: HELICS time-step mismatch detection, and ANDES power-flow
    consistency against pandapower on the same case.
  - **Report trace:** §19.1 (multi-domain co-simulation, dynamic
    stability); §19.2(6).
  - **Inputs / prerequisites:** P8.16 ANDES integration; P8.18 HELICS
    federation; P8.01 simulator adapters.
  - **Steps:**
    1. Test HELICS time-step mismatch: configure federates with
       mismatched time steps and confirm the federation detects and
       reports the mismatch (or refuses to run) instead of silently
       corrupting the interaction.
    2. Test ANDES cross-consistency: run ANDES power flow and pandapower
       power flow on the same case with the same inputs and compare
       voltages/flows within a pre-recorded tolerance; record and explain
       any difference beyond tolerance.
    3. Test federate dropout: a federate that stops responding mid-run
       must be detected and reported, never silently treated as
       "no change".
  - **Deliverable / output:** Cross-check test record.
  - **Acceptance criteria / exit check:** Mismatch detected/reported;
    ANDES vs pandapower comparison recorded within (or explained beyond)
    tolerance; federate dropout detected.
  - **Owner role:** Research engineer.
  - **Dependencies:** P8.16, P8.18.
  - **Verification method:** Independent re-run of the mismatch case.
  - **Risks / failure modes:** A mismatch test that only checks the
    configuration file, not the running federation; a cross-check tolerance
    fitted after seeing the difference.

## Subtask index

| ID | Title | Trace |
|---|---|---|
| P8.01 | Phase readiness check (Phases 1–7, P7.31 handover) | §23 Stages 1–7 |
| P8.02 | Research scope freeze (questions, hypotheses, non-goals) | §13.1–§13.3 |
| P8.03 | License/dependency audit (research stack) | §25.2, §9.3 |
| P8.04 | Research worker contracts (proposal-only boundary) | §13.4, §9.2 |
| P8.05 | Independent validation harness | §13.1–§13.3, §19.2 |
| P8.06 | Strong deterministic baselines (frozen) | §23 Stage 8, §13 |
| P8.07 | ADMM/DMPC formulation + convexity assumptions | §13.2 |
| P8.08 | ADMM residuals, termination, fallback | §13.2, §19.4 |
| P8.09 | ADMM/DMPC evaluation vs baselines | §13.2, §19.3–§19.4 |
| P8.10 | GNN experiment design (roles, scope, data) | §13.1 |
| P8.11 | GNN warm-start/screening implementation (isolated worker) | §13.1, §13.4 |
| P8.12 | GNN evaluation (end-to-end feasible solve time, unseen topologies) | §13.1, §19.4 |
| P8.13 | Safe MARL design (CTDE, safety filter, infeasible-filter outcome) | §13.3 |
| P8.14 | MARL simulation/shadow worker (never live dispatch) | §13.3–§13.4 |
| P8.15 | MARL evaluation (held-out regimes, outages, delays, uncertainty) | §13.3, §19.4 |
| P8.16 | Dynamic simulator integration (ANDES) | §19.1, §7.4 |
| P8.17 | Dynamic evaluation (research controllers under disturbances) | §19.1, §19.4 |
| P8.18 | Co-simulation integration (HELICS) | §19.1, §7.4 |
| P8.19 | Benchmark cases extended + not-tested list | §19.3 |
| P8.20 | Reproducibility (seeds, versions, artifacts, rerun test) | §19.2, §9.2 |
| P8.21 | Topology-aware optimization research (transfer, scaling) | §13.1, §2.1 |
| P8.22 | Portfolio analytics research (reporting only) | §5.3, §19.4 |
| P8.23 | Cross-module comparison (same cases, same metrics) | §23 Stage 8, §19.4 |
| P8.24 | Failure-mode and robustness drill | §19.2, §13.3 |
| P8.25 | Security review of research workers | §19.2, §17.4 |
| P8.26 | Benchmark paper/report | §23 Stage 8 |
| P8.27 | Operator console: research/shadow surfacing | §20.1–§20.2 |
| P8.28 | CI gates for research code | §22.2, §19.2 |
| P8.29 | Per-feature qualification record (not granted here) | §2.1, §22.3 |
| P8.30 | Retrospective, open items, exit-gate record | §22.3 |
| P8.31 | Research-worker tenant isolation, scoped data access | §17.4, §16.2 |
| P8.32 | Model-artifact signing, provenance, verification at load | §9.3, §17.4 |
| P8.33 | Research-worker identity, mTLS, secret handling | §17.3 |
| P8.34 | Proposal validity windows, stale-proposal rejection | §14.3, §17.5 |
| P8.35 | Training-data provenance, license, poisoning controls | §9.3, §19.3 |
| P8.36 | Research-stack CVE/advisory check at pin time | §22.2, §17.3 |
| P8.37 | Unit and property tests (research workers) | §19.2 |
| P8.38 | Training-run reproducibility, GPU-nondeterminism disclosure | §19.2, §9.2 |
| P8.39 | Co-simulation and dynamic cross-checks | §19.1, §19.2 |

**Subtask count: 39.**
