# P8.08 — ADMM/DMPC provider: residuals, termination, infeasibility, and centralized fallback

Date: 2026-11-13. Subtask: P8.08 (`ledger_phase_8.md` §P8.08). Owner role:
research engineer (optimization). Report trace: §13.2; §19.4 (ADMM
metrics); §19.2(6). Inputs: P8.07 (`07_admm_dmpc.md` — the formulation
this contract operates), P8.06 (`05_06_harness_baselines.md` — the frozen
fallback registry), report §13.2, §19.4, §19.2(6), ENTRY_NOTE.md
boundaries 1–6.

## 1. Status of this record — WRITTEN-UNCOMPILED

The operational contract is implemented in
`crates/flexcore-research/src/dmpc.rs` (~1500 lines with its in-module
test suite), over the P8.07 formulation in
`crates/flexcore-research/src/admm.rs`. **WRITTEN-UNCOMPILED**: no cargo
command was executed in this loop (the subtask's loop bans; ENTRY_NOTE.md).
Every type reference was written against source that was read, and every
test expectation is hand-computed arithmetic stated in the test comments —
the toys' optima, fixed points, and per-iteration traces are derived on
paper IN the comments. NOTHING here is evidence that the crate compiles or
that any test passes; the human build/test gate (AGENTS.md rule 7) owns
that evidence. **Every measured cell is EMPTY** (ENTRY_NOTE boundary 4):
no benchmark figure, seed result, runtime, or residual statistic from a
real run exists anywhere in this record or in the code.

## 2. The operational contract, step by step (the ledger's four steps)

### Step 1 — residuals and termination (§9.2 diagnostics)

- **Residuals on every run**: primal r = ‖a∘x − z‖ and dual
  s = ρ·‖a∘(z_new − z_old)‖ (the two-block ADMM residual definitions,
  Boyd et al. 2011 §3.2/§3.3 — the P8.07 primary source), computed every
  iteration; the final pair travels on `AdmmRunResult` together with the
  tolerances used (`epsilon_primal`, `epsilon_dual`) — a run that does not
  name its residuals and termination evidence is not a result.
- **Termination**: the Boyd-style absolute + relative tolerances
  (ε_pri = √n·ε_abs + ε_rel·max(‖a∘x‖, ‖z‖);
  ε_dual = √n·ε_abs + ε_rel·ρ‖u‖) AND the iteration budget
  (`TerminationCriteria`, validated fail-closed with named errors). The
  criteria are CONFIGURATION recorded on every result — NOT a universal
  target (X-8); the threshold pre-registration is the P8.09 evaluation's
  obligation. Exhausting the budget is `NON_CONVERGED_BUDGET_EXHAUSTED`
  — never a result.

### Step 2 — infeasibility, distinguished from non-convergence

`CoupledProblem::feasibility_certificate` (P8.07) is checked BEFORE any
iteration: for one linear coupling over closed boxes the certificate is
EXACT (if-and-only-if), so `INFEASIBLE_CERTIFICATE` is returned with
`iterations_run = 0` — distinguishable from non-convergence BY
CONSTRUCTION, never a timeout guess. Pinned by a hand-computed test
(demand 10 MW over boxes [0, 3] × 2 → min achievable aggregate −6 > b =
−10 → infeasible, zero iterations).

### Step 3 — communication failure and stale participants

Every participant answers through the injected `AdmmLocalWorker` port
(the composition root implements it over real worker processes; the
in-crate reference is the closed-form local solve). The policy:

- A worker that FLAGS its reply stale (honest re-serve) or ERRORS costs
  one staleness unit; the coordinator DISCARDS a flagged reply and reuses
  the participant's last accepted value (initialized to the box-clamped
  zero) — and every reuse is RECORDED as a `StaleEvent` on the result
  (`used_stale_values` flag too). Bounded by `max_stale_iterations`
  consecutive iterations (0 = none tolerated).
- Beyond the bound the run is `ABANDONED_PARTICIPANT_UNRESPONSIVE` with
  the participant AND iteration named — a dropped participant mid-run
  produces a recorded failure or a fallback, NEVER a silently partial
  result (pinned: the abandoned run's partial x is diagnostics-only and
  `final_schedule()` never returns it).
- Coverage is validated at construction (`WORKER_COVERAGE_MISSING`): a
  wiring gap is refused before any run, so it can never masquerade as a
  mid-run participant drop.

### Step 4 — the centralized fallback (the frozen P8.06 registry)

On ANY non-converged outcome (infeasible, budget-exhausted, abandoned)
the run invokes the injected `CentralizedFallbackSolver` port and RECORDS
the outcome on the result (`fallback` = the `FallbackOutcome` with
baseline id + version + reason + the honest schedule including deficits;
or `fallback_error` = the named refusal). The in-crate default,
`PriorityListFallback`, wraps `baselines::priority_list_dispatch` —
`baseline/rule-based-priority-list`, the FROZEN P8.06 registry's rule set
(`priority-list.v1`), NOT an unstudied heuristic; it serves demand-floor
couplings (every a_k = −1) and REFUSES other classes with
`FALLBACK_CLASS_NOT_SUPPORTED` (the composition root implements the port
over the Tier 1 HiGHS seam — `baseline/highs-tier1-lp` — for the full
supported class). Structural guarantees, pinned by tests:

- the schedule path (`final_schedule`) returns the converged distributed
  solution, the fallback's schedule, or NOTHING — there is no constructor
  for a schedule from a partial ADMM state, so a half-converged
  distributed run can never be presented as final;
- the fallback NEVER receives the failed ADMM output (no unvalidated warm
  start — the ledger's named risk);
- a refused fallback leaves NO schedule (recorded failure), never a
  silently partial one;
- the fallback's deficits are honest (supplied 6, deficit 4 on the
  infeasible toy — recorded, never silently filled).

## 3. The DMPC rolling horizon (the driver)

`plan_horizon` runs the operational contract across a demand horizon in
fixed interval order, carrying the rolling energy state forward: each
energy-limited participant's interval box is capped by remaining energy /
interval hours (when the remainder cannot sustain the declared minimum,
the box collapses to [0, 0] — the analogue of the frozen baseline's skip
rule); each interval's FINAL schedule (converged ADMM or recorded
fallback) updates the energy state; a recorded failure adopts nothing.
The FIRST interval's schedule is the committed receding-horizon step.
The receding-horizon property composes with the platform's EXISTING P3.24
rolling-horizon seam (`crates/flexcore-application/src/rolling_horizon.rs`)
BY CITATION — this crate does not depend on flexcore-application; the
composition root re-invokes `plan_horizon` each scheduling cycle with
refreshed SOC state.

**Recorded formulation limitation (honest)**: the inter-interval coupling
enters ONLY through the energy-state update; the multi-interval objective
is NOT jointly optimized. This is a stated restriction of the supported
class (a jointly-optimal multi-interval ADMM would be a formulation change
and a NEW formulation version), and no optimality claim is made for the
horizon objective.

## 4. The failure-mode test matrix (ledger deliverable: "tests for each failure mode")

| # | Fault injected | Expected recorded outcome (hand-computed) |
|---|---|---|
| 1 | none — toy 2, ρ = 2 | `CONVERGED`, exactly 2 iterations, x = (2, 2) exact, r = s = 0 |
| 2 | none — toy 3 (asymmetric) | converges to the hand-derived analytic optimum (5/3, 10/3), λ = 4/3, coupling met |
| 3 | demand 10 over boxes [0,3]² | `INFEASIBLE_CERTIFICATE`, 0 iterations, fallback supplied 6 / deficit 4 |
| 4 | budget = 1 on toy 3 | `NON_CONVERGED_BUDGET_EXHAUSTED`, residual recorded, fallback supplied 5 / deficit 0 |
| 5 | worker never answers, staleness bound 0 | `ABANDONED_PARTICIPANT_UNRESPONSIVE` at iteration 1, participant named, fallback recorded |
| 6 | worker stale forever, bound 1 | reuse within bound (event recorded), abandon at iteration 2 |
| 7 | worker stale once, then healthy | converged run with `used_stale_values = true` and one recorded event — staleness visible, never silent |
| 8 | export-cap class (a = +1), budget 1 | fallback refuses the class (`FALLBACK_CLASS_NOT_SUPPORTED`), `final_schedule() = None` — recorded failure, never a partial schedule |
| 9 | coverage gap at construction | `WORKER_COVERAGE_MISSING` (refused before any run) |
| 10 | bad criteria / bad ρ | `NON_POSITIVE_EPSILON` / `ZERO_ITERATION_BUDGET` / `INVALID_PENALTY` |
| 11 | two identical runs | byte-identical results (determinism) |
| 12 | proposal into the P8.05 harness | full pass; expired window refused (`PROPOSAL_EXPIRED`); incomplete provenance refused (`PROVENANCE_INCOMPLETE`); limit-violated gate answer refused (`PHYSICAL_REVALIDATION_LIMIT_VIOLATED`) |

All WRITTEN-UNRUN — the human build/test gate runs them (`cargo test -p
flexcore-research`).

## 5. §19.4 metrics honesty (what is computed here and what is not)

Computed in-run and recorded: primal/dual residual, iteration count, the
termination evidence, and the communication-failure/staleness statistics
(events + flag) — the machinery communication sensitivity (§19.4) is
measured AGAIN at P8.09 evaluation time. NOT computed here, deliberately:
RUNTIME (no wall clock anywhere — determinism; the time metric is a
human-run measurement) and the OBJECTIVE GAP (an evaluation-time metric
against the frozen P8.06 baselines, owned by P8.09; computing a gap
in-run would need the centralized solution and would tempt an in-run
optimality claim).

## 6. How outputs flow (P8.05 / P8.06 composition)

Identical to `07_admm_dmpc.md` §6 and restated for this subtask's exit
check: every `DmpcPlan`'s first step is packaged by
`proposal_for_first_step` as a `validation::ResearchProposal` and enters
the P8.05 INDEPENDENT harness (fixed stage order, physical re-validation
LAST through the injected platform gate); comparison against the frozen
P8.06 baseline records happens at P8.09 evaluation time from the frozen
artifacts. The proposal's provenance is honest by construction:
termination_status = the named run status (never the bare "ok");
`seed_supported = false` (no randomness — the P7.42 admission);
`model_artifact_ref` = `07_admm_dmpc.md` (the reviewer-signed formulation
governing the provider); diagnostics carry every staleness/fallback event
and the ADR-010 boundary statement.

## 7. Exact filling commands (the human gate)

```sh
cargo build -p flexcore-research
cargo test -p flexcore-research            # the §4 matrix runs here
cargo test -p flexcore-research --test admm_dmpc_proposal_flow
cargo clippy -p flexcore-research -- -D warnings
cargo fmt --check
python tools/verify_docs.py
python tools/validate_agent.py
python -I tools/check_dependency_rule.py
```

A green run earns only a one-line "human build/test gate: PASSED (date)"
appended here; failures are recorded verbatim and the ledger tick reverts
to `- [ ]`.

## 8. Non-claims (restated; X-3/X-8)

Termination evidence is residual convergence under the P8.07 convexity
assumptions — no iteration-count guarantee, no uniqueness claim, no
global-optimality claim outside the supported class, no safety claim, no
field authority (proposals only, ADR-010); the fallback baseline's own
limits travel with its frozen record ("the strength of this baseline is
exactly the written rule set"); no benchmark, runtime, or residual
statistic from a real run exists in this record.
