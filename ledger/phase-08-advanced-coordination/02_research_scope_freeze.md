# P8.02 — Research scope freeze: questions, hypotheses, and non-goals

Date: 2026-11-13. Subtask: P8.02 (`ledger_phase_8.md` §P8.02).
Owner role: Research lead. Report trace: §13.1–§13.3; §19.1; §19.4.
Inputs: `PLAN.md` §1/§9; P8.01 (`01_readiness_check.md`); report §13;
§19.4; `CONTEXT.md`; RESEARCH.md. Dependency: P8.01 (recorded exception
— the baseline numbers do not exist yet; see §5 for what that means for
threshold pre-registration).

## 1. What this record is (and is not)

This record **freezes, in writing, the Phase 8 research scope — the
question list, the hypothesis form, and the non-claims — before any
model is built or any training run is planned** (ledger P8.02 objective:
"Research without a pre-registered question is exploration, not an
experiment"). It records decisions the report and PLAN.md already make
(§13, §19.1–§19.4, PLAN.md §1/§9); it decides nothing new. It does not
close or alter any of D1–D6 (`.agent/DECISIONS.md` — all six remain
OPEN; D3 constrains every experiment's network class, D4 is followed
through in `03_research_license_audit.md`). No new ADR is written.

Scope-freeze rules this record enforces:

- **Non-goals are PROHIBITIONS, not deferrals** (§3) — a prohibition
  cannot be re-litigated as a shortcut later (the P7.02 pattern).
- **Every question is falsifiable and carries a named metric** (§19.4).
  A question without a metric fails the ledger's verification method.
- **No threshold in this record is a universal target** of the kind
  §19.4 forbids ("zero violations proves safety", "converges in 30
  iterations"). See §5 for how thresholds get pre-registered.
- This record freezes the **question list**; the experiments (P8.06
  onward) define the numbers that answer them. Nothing here is a result.

## 2. In-scope research (frozen; each item traceable to PLAN.md §1 / report §)

| # | In scope | Report trace | What it means for this phase |
|---|---|---|---|
| S-1 | ADMM/DMPC provider with explicit convexity assumptions, residuals, termination, infeasibility handling, communication-failure handling, centralized fallback | §13.2; PLAN.md §1 | Formulation (P8.07) → operational contract (P8.08) → evaluation vs frozen baselines (P8.09). Nonconvex/mixed-integer instances are reported as heuristic behaviour, never as convergent/optimally solved (§13.2) |
| S-2 | GNN warm-start/screening on unseen topologies — two separate roles: candidate AC-OPF warm-start generation and active-constraint/contingency screening | §13.1; PLAN.md §1 | Design (P8.10) → isolated inference worker (P8.11) → evaluation by **end-to-end feasible solve time**, not inference latency, on held-out topologies (P8.12). The GNN warms the solver; it never replaces it |
| S-3 | MARL CTDE baseline + safety filter, in simulation/shadow only | §13.3; PLAN.md §1 | Design incl. safety-filter derivation and infeasible-filter outcome (P8.13) → shadow worker, never live dispatch (P8.14) → evaluation under held-out regimes, outages, delays, uncertainty (P8.15) |
| S-4 | ANDES dynamic integration (time-domain simulation, eigenvalue analysis) + dynamic evaluation of research controllers under disturbances; HELICS co-simulation for multi-domain interaction | §19.1; PLAN.md §1 | ANDES scope/models + steady-state boundary (P8.16), dynamic evaluation (P8.17), HELICS federation (P8.18). Every dynamic number carries its model/case reference |
| S-5 | Benchmark extensions per §19.3, frozen deterministic baselines, independent validation harness, reproducibility (seeds/versions/artifacts/rerun test), cross-module comparison, benchmark paper/report | §19.3; §23 Stage 8; PLAN.md §1 | P8.19, P8.06, P8.05, P8.20, P8.23, P8.26. The baselines are frozen BEFORE any research comparison (no moving the baseline after seeing research numbers) |
| S-6 | Portfolio analytics research as reporting-only plugins | §5.3; §19.4; PLAN.md §1 | P8.22. Analytics never influence dispatch (verified by inspection); validated against the deterministic baseline's accounting |
| S-7 | Per-feature qualification record stating what field use would require — and that this phase grants none of it | §2.1; §22.3; PLAN.md §1 | P8.29. Qualification is a later decision; the ceiling of this phase is simulation/shadow |

## 3. Out of scope — recorded AS PROHIBITIONS (not deferrals)

Each row is a prohibition the phase's records and code must uphold; none
can be converted into "later in this phase" by any implementer.

| # | Prohibition | Source | Enforcement point |
|---|---|---|---|
| X-1 | **No field authority for ANY research module.** No research output is a dispatch decision; every research proposal enters at `PROPOSED` through the protected validation → feasibility → authorization path and gains no authority from having been proposed. Per-feature qualification is not granted in this phase | §22.1 (ADR-010); §13.4; §2.1; P7.31 §4 (nine code-level enforcement points) | Worker response types cannot carry an authorization token (P8.04); broker never-granted set refuses `dispatch.authorize`/`dispatch.issue` structurally |
| X-2 | **No live device commands from MARL** — a research MARL worker has no code path to the dispatch gateway; this is prohibited, not deferred | §13.3; PLAN.md §1 non-goals; ENTRY_NOTE.md item 1 | P8.14 boundary test: routing a MARL action to dispatch must fail by construction |
| X-3 | **No global-convergence or global-optimality claim** for nonconvex AC-OPF or mixed-integer local problems without a specific theorem AND experiment; ADMM on such problems is reported as a heuristic | §13.2; AGENTS.md rule 4; RESEARCH.md §1 (R8-C1 confirms the boundary) | P8.07 assumptions note (reviewer-signed); grep discipline in P8.07's verification method |
| X-4 | **No safety guarantee from a finite test set.** Zero observed constraint violations is a test outcome, not a universal guarantee; evaluation notes must state the limitation in plain language | §13.3; §19.4; PLAN.md §1 non-goals | P8.15/P8.26 evaluation-note requirement |
| X-5 | **No steady-state power-flow tool as evidence for transient frequency stability** — pandapower/AC power flow evaluates a state; it is not AC-OPF and not a transient-stability study | §19.1; §12.5; AGENTS.md rule 4 | P8.16 boundary statement; §19.1 track table separates the tool per question |
| X-6 | **No HELICS-as-physics claims** — HELICS synchronizes federates; it is not the power-system model, and federation results are not power-system results | §19.1; P7.31 §8 | P8.18 boundary statement |
| X-7 | **No feeder-scale results presented as 10,000-bus capability.** The first distribution-feeder tests do not establish capability for a utility-scale network; transfer/scaling limits are reported with measured numbers and no claim beyond the tested range | §19.3; PLAN.md §1 non-goals | P8.19 not-tested list; P8.21 scaling report |
| X-8 | **No arbitrary universal targets** as success criteria — thresholds are set after defining the model, service, benchmark, and operational risk, never as "zero violations in N cases" or "converges in K iterations" | §19.4 (binding; also `benchmarks/README.md` no-targets rule) | §5 of this record; P8.06 threshold pre-registration |
| X-9 | **No productizing of research modules** — no operator workflow, deployment model, or upgrade path for GNN/MARL/ADMM outputs; that is v1.0.0/Stage 9 work | PLAN.md §1 non-goals; §23 Stage 9 | P8.29 records the qualification requirement without granting it |
| X-10 | **No new plugin SDK contracts** — research workers reuse the Phase 7 gRPC worker template and contracts; a contract gap is recorded as a Phase 7 follow-up, never fixed by inventing a parallel contract here | PLAN.md §1 non-goals; P7.31 §3 | P8.04 extends `worker_api.proto` only; buf governance (§10.5) |
| X-11 | **No bypass of the protected kernel** — research plugins cannot touch authorization, tenant isolation, audit, lifecycle, or final dispatch validation (ADR-005); no arbitrary Rust dylib loading (ADR-006) | §22.1 (ADR-005/006); P7.31 §4/§6 | Kernel manifest parse-time refusals; enforcement door; no-dylib gate |

## 4. Research questions and hypotheses — the frozen question list

Framing rule (ledger P8.02 step 1): one falsifiable question per module,
with the metric that would answer it (§19.4) and a success threshold set
**after** defining the model, service, benchmark, and operational risk
(§19.4). The metrics below are frozen now; the numeric thresholds are
pre-registered per §5 (before any training/tuning run) because the
models, service definitions, and frozen baselines they depend on do not
exist yet — setting a number here would fabricate the §19.4
precondition. Each module also records its **D3 obligation**: the
network class each experiment covers must be stated in that module's
design record (balanced radial is the 0.x default RECOMMENDATION;
unbalanced/meshed only if explicitly scoped).

### M1 — ADMM/DMPC (§13.2)

| Item | Frozen content |
|---|---|
| Question | On the §19.3 coupled cases (multiple feeders/sites under a common substation constraint), does the ADMM/DMPC provider produce a validated feasible schedule whose objective is within the pre-registered gap of the centralized HiGHS baseline, at a pre-registered coordination/communication cost, under the formulation's stated convexity assumptions? |
| Hypothesis (pre-registered form) | H-M1: for the convex problem classes the P8.07 formulation declares, ADMM/DMPC reaches termination (residual tolerances + iteration budget, P8.08) with objective gap and runtime inside the pre-registered thresholds on the declared cases. Falsified if residuals do not terminate within budget, or the objective gap/runtime exceeds the centralized baseline beyond the pre-registered threshold without a compensating coordination benefit, on any declared case |
| Metrics (§19.4 ADMM row) | primal/dual residual, objective gap **where meaningful** (not against a nonconvex baseline), iteration count, time, communication sensitivity |
| Baselines (P8.06) | centralized LP/MILP/QP (HiGHS) + rule-based dispatch |
| D3 obligation | P8.07 states the network class of every case (balanced radial default recommendation) |
| Non-claims that apply | X-3 (no global optimality/convergence beyond stated assumptions); nonconvex/MI runs reported as heuristic behaviour with residuals and fallback outcomes |

### M2 — GNN warm-start / screening (§13.1)

| Item | Frozen content |
|---|---|
| Question (a) warm-start | On topologies held out of training, does a GNN warm start reduce **end-to-end feasible solve time** (solver + validation, not inference latency) vs the cold-start solver and a non-GNN heuristic warm start, without reducing feasible-solver success rate? |
| Question (b) screening | On held-out topologies, does GNN screening rank constraints/contingencies that are actually active above inactive ones at a pre-registered miss rate and miss cost? |
| Hypotheses (pre-registered form) | H-M2a falsified if: no end-to-end speedup on held-out topologies, OR speedup accompanies a feasible-solver success-rate loss beyond the pre-registered threshold, OR a warm start that leads to an infeasible solution is counted as success (it is not — it fails the end-to-end metric). H-M2b falsified if: ranking quality or miss cost exceeds the pre-registered thresholds on held-out topologies. A warm-start win is NOT a screening win — the roles are never conflated into one headline number |
| Metrics (§19.4 GNN row) | feasible solver success, end-to-end time, speedup, generalization to unseen topologies; screening: ranking quality vs true active sets, cost of a missed constraint |
| Baselines (P8.06) | cold-start solver + non-GNN heuristic warm start |
| Held-out sets | named in P8.10 (unseen topologies and operating regimes, §13.1); leakage controls recorded there |
| D3 obligation | P8.10 states the topology classes in training vs held out |
| Non-claims that apply | X-7 (generalization claimed only over the tested range); X-1 (the GNN output passes the same physical validation as any other candidate — §13.1) |

### M3 — Safe MARL (§13.3)

| Item | Frozen content |
|---|---|
| Question | In simulation/shadow only, under held-out topologies, load/weather regimes, device outages, delayed/lost messages, and uncertain state estimates, does a CTDE MARL policy with a derived safety-projection layer match or beat the deterministic and rule-based/PI baselines on the pre-registered §19.4 MARL metrics — with every safety-filter activation and infeasibility logged? |
| Hypothesis (pre-registered form) | H-M3: constraint-violation frequency, return, and robustness under distribution shift are within the pre-registered thresholds vs the baselines on the declared evaluation suite. Falsified if the policy is worse than either baseline beyond threshold under shift, or if filter infeasibility produces unfiltered pass-through (that outcome is a design failure, not a result) |
| Metrics (§19.4 MARL row) | constraint-violation frequency, return, robustness under distribution shift, baseline comparison; plus filter activation/infeasibility statistics (P8.15) |
| Baselines (P8.06) | deterministic optimization + rule-based/PI baselines; centralized-training baselines where applicable (§13.3) |
| D3 obligation | P8.13 states the network class of the environment |
| Non-claims that apply | X-2 (never live dispatch), X-4 (a finite test set with zero observed violations is not a safety guarantee — stated in plain language in every evaluation note), X-8; the safety filter must be derived from a valid system model and the correct relative degree, with voltage/branch-current constraints not assumed to be simple dynamic state constraints (§13.3) |

### M4 — Dynamic stability research (§19.1 dynamic track; §13.2–§13.3 context)

| Item | Frozen content |
|---|---|
| Question | In the declared ANDES model (P8.16 records the models/cases/versions), do the research controllers' proposals — applied in shadow to the simulated system, never to devices — change frequency/transient behaviour vs the deterministic baseline controller on the recorded disturbance set? |
| Hypothesis (pre-registered form) | H-M4: the recorded disturbance set (P8.17) produces measurable, model-scoped differences (frequency nadir/settling, voltage behaviour, instability occurrences) between research proposals and the baseline controller; the claim is evidence about the DECLARED MODEL only. Falsified if no difference is measurable, or if a difference exists only outside the model's declared scope |
| Metrics | frequency nadir/settling, voltage behaviour, instability occurrences — each carrying its model/case reference |
| Baselines (P8.06/P8.17) | the deterministic baseline controller on the same disturbances |
| D3 obligation | P8.16 states the model class and cases (e.g. Kundur two-area, IEEE 14-bus with dynamic data are named in the ledger as candidate cases — selection recorded there) |
| Non-claims that apply | X-5 (steady-state tools are not transient-stability evidence; conversely dynamic results are not quasi-static feasibility evidence), X-6, ADR-012 (no protection/hard-real-time claims — dynamic results are evidence about a declared model, not the real grid) |

Portfolio analytics (S-6) carries no falsifiable research question in
this freeze: its exit check is validation of derived analytics against
the deterministic baseline's accounting within a stated tolerance on the
tested scenarios (P8.22), with limits recorded — it is reporting-only
scope, not a hypothesis-bearing module.

## 5. Pre-registration structure (what gets registered, when, and by whom)

| Element | Frozen now? | Owner | When |
|---|---|---|---|
| Question list + metrics (§4) | YES — this record | Research lead | P8.02 (now) |
| Baselines per module | Named per module (§4); defined/run/frozen in P8.06 | Research lead + power-systems reviewer | P8.06, BEFORE any research comparison |
| Held-out sets + leakage controls | Rule frozen (§4); concrete lists in P8.10/P8.13/P8.15 | Research engineer (ML/RL) | At each design record, before training/tuning begins |
| Numeric success thresholds per question | **NOT SET HERE** — pre-registered after the model, service, benchmark, and operational risk are defined (§19.4), and never as a universal target (X-8). The threshold slot per question is pre-registered in writing before any training/tuning run, at P8.06 freeze time at the earliest | Research lead + independent reviewer | P8.06 freeze; re-checked at P8.26 report review |
| Analysis plan (what is reported, incl. losses and "not run" cells) | Rule frozen (S-5); per-module plan in the evaluation templates | Research lead | P8.09/P8.12/P8.15 templates |
| Non-claims list (§3) | YES — this record | Research lead | P8.02 (now); restated in P8.26's report |

**Owner and review date (ledger P8.02 step 4):** owner = Research lead.
Review: at P8.06 baseline freeze (thresholds registered there), and again
at phase close (P8.30) when open research questions are carried forward.
No calendar date is invented (PLAN.md §6 sequences by relative effort).

## 6. What this freeze does NOT decide

- No threshold value, no baseline number, no "success" verdict — the
  experiments define those (P8.06 onward); every measured cell in the
  evaluation templates stays EMPTY until a human/lab run fills it
  (ENTRY_NOTE.md item 4).
- No framework selection (torch/PyG pair, Ray/RLlib stack) — recorded as
  DECISION-POINTs in `03_research_license_audit.md` §4 and resolved by
  P8.10/P8.13 with verified pins.
- No D-item closure (D3/D4 remain OPEN; `.agent/DECISIONS.md`).
- No per-feature qualification outcome — P8.29 records the requirement
  and that none of it is granted here (X-1, S-7).

## 7. Verification method

Independent reviewer checks each §4 question against §13 and §19.4: every
question must be falsifiable, carry a named §19.4 metric, and name its
baseline; any question without a metric fails. Every §3 prohibition must
trace to its cited source. Any threshold found in this record would be a
finding against it (X-8) — there are none.
