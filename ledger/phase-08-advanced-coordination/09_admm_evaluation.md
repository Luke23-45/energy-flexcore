# P8.09 — ADMM/DMPC evaluation against baselines on the benchmark cases (TEMPLATE — every measured cell EMPTY)

Date: 2026-11-13. Subtask: P8.09 (`ledger_phase_8.md` §P8.09). Owner role:
research engineer (optimization). Report trace: §13.2; §19.3–§19.4; §23
Stage 8. Inputs: P8.07 (`07_admm_dmpc.md`) and P8.08
(`08_admm_operations.md`) — the provider this template evaluates; P8.05
(`05_06_harness_baselines.md` §2 — the independent validation harness); P8.06
(`05_06_harness_baselines.md` §3 — the frozen baseline registry); P8.02
(`02_research_scope_freeze.md` — question M1, prohibition X-8); ENTRY_NOTE.md
boundary 4.

## 1. Status of this record — a TEMPLATE, nothing measured, nothing fabricated

**Every measured cell in this document is EMPTY.** Per the phase boundaries
(ENTRY_NOTE boundary 4: "Every measurement is an EMPTY cell") the benchmark
runs are **human/lab gate inputs**: no benchmark figure, seed result,
runtime, residual statistic, convergence statistic, objective gap, fallback
frequency, or staleness count from a real run appears anywhere below, and
none was computed by any code in this loop (nothing was built or run).

What exists and is verifiable to the extent the loop allows:

- the **evaluation harness**
  (`crates/flexcore-research/src/admm_evaluation.rs`,
  `flexcore-research/admm-evaluation.v1`) — pure functions that assemble
  the per-case §19.4 statistics from INJECTED data (the P8.08
  `AdmmRunResult`s, the frozen P8.06 baseline objectives, the P8.05
  validation outcome, the human-recorded wall-clock runtimes), each
  pinned by HAND-COMPUTED self-tests. The harness was **written, never
  compiled or executed** (WRITTEN-UNCOMPILED; no cargo command ran in
  this loop);
- the **filling procedure** (§6) — the exact commands the human/lab pass
  runs and the cells they fill.

**The test-arithmetic / experiment-result distinction (stated explicitly,
as the subtask requires):** the self-tests inside `admm_evaluation.rs`
assert hand-derived values over toy problems (e.g. the toy fixed point
x = (2, 2) has objective 0.5·2·(2−0)² × 2 = 8.0; the infeasible toy's
fallback schedule x = (3, 3) has objective 18.0; the toy converged gap
against an injected baseline at 8.0 is exactly 0.0). That is **test-grade
arithmetic pinning the metric definitions** — it is NOT a benchmark
measurement, NOT a seed result, and NOT evidence about any §19.3 case. No
measured cell is filled by it.

## 2. The evaluation protocol, frozen before any run (ledger steps 1–2)

### 2.1 The case set

The cases are the §19.3 benchmark set via the frozen P8.06 entry point
(`baselines::BENCHMARK_ENTRY_POINT`: `apps/sim-runner` over
`scenarios/library/` lib-01…lib-08) plus the P8.19 versioned research-case
extensions. **No case list is invented here**: the concrete case ids, the
benchmark-set version cited by every row, and the per-case parameters are
`RECORD-AT-FIRST-RUN` fields, frozen when the first run happens. The
case families the ledger names, all TEMPLATE rows:

| Family | §19.3 trace | Case ids | Status |
|---|---|---|---|
| IEEE feeders (13/34/123-bus) | §19.3 | `RECORD-AT-FIRST-RUN` | EMPTY — run pending |
| Synthetic portfolios | §19.3 | `RECORD-AT-FIRST-RUN` | EMPTY |
| Multi-feeder substation constraint (the coupled class) | §19.3 | `RECORD-AT-FIRST-RUN` | EMPTY |
| Stressed scenarios (PV ramps, forecast error, comm loss, asset dropout, reverse power flow) | §19.3 + P8.19 | `RECORD-AT-FIRST-RUN` | EMPTY |

Selection discipline (the ledger's named risk — cherry-picked cases): this
family list is declared BEFORE any run and is frozen at the first run;
adding a case after seeing results requires a dated amendment here with
the reason, and the per-case table must be checked COMPLETE against the
§19.3 case list (the ledger's verification method: "check the per-case
table is complete against the §19.3 case list" — no case silently
dropped).

### 2.2 Pinned versions, seeds, and tolerances

| Item | Recorded value | Status |
|---|---|---|
| ADMM/DMPC provider | `flexcore-research/admm-formulation.v1` (P8.07) + `flexcore-research/admm-operations.v1` (P8.08) | frozen version stamps |
| Provider determinism | no randomness — `seed_supported = false` on every proposal (the P7.42 admission; same input → same result, byte-equality-pinned) | recorded |
| Tier 1 baseline | HiGHS `baseline/highs-tier1-lp`, solver 1.15.1, seed policy `Fixed` via `DeterminismMetadata` (P7.42), thread_count 1 (P3.26) | frozen registry record |
| Tier 2 baseline | Clarabel.rs `baseline/clarabel-tier2-conic`, 0.11.1, no seed (recorded honestly) | frozen registry record |
| Rule-based baseline | `baseline/rule-based-priority-list`, `priority-list.v1`, no randomness | frozen registry record |
| Termination criteria | `TerminationCriteria` recorded ON every result (ε_abs, ε_rel, budget, staleness bound) — CONFIGURATION, never a target | recorded |
| P8.05 harness | `flexcore-research/validation.v1` on every validation result | frozen |

**The §19.4 preconditions are still missing — this template is
pre-registration.** The numeric success thresholds (objective-gap bound,
tie tolerance, any iteration-count target) are NOT SET here, per
`02_research_scope_freeze.md` §5 and X-8: they are pre-registered in
writing after the model, service, benchmark, and operational risk are
defined — at P8.06 freeze time at the earliest, never as universal
targets, never after research numbers are known. The harness ENFORCES
this in code: `compare_to_baseline` refuses `None` tie tolerance with the
named `THRESHOLD_NOT_PRE_REGISTERED` error; the module computes statistics
and never emits a success/failure verdict on its own.

### 2.3 The per-case metrics (§19.4 ADMM row) — where each comes from

| Metric | Source | Rule |
|---|---|---|
| Primal/dual residual r, s | the run result, against the recorded Boyd absolute+relative tolerances (ε_pri = √n·ε_abs + ε_rel·max(‖a∘x‖, ‖z‖); ε_dual = √n·ε_abs + ε_rel·ρ‖u‖) | re-checked by the harness from the recorded pair, not trusted from the status field |
| Iteration count | `iterations_run` | min/max/mean over runs that iterated (zero-iteration certificate runs counted in the status counts, excluded from the aggregates — documented) |
| Objective gap WHERE MEANINGFUL | `objective_gap` over the injected frozen P8.06 baseline objectives | refused unless the caller declares the comparison meaningful — NEVER computed against a nonconvex or mixed-integer comparison (the ledger's named risk); `gap = admm − baseline` (negative = the ADMM objective smaller); relative gap `None` below the epsilon guard |
| Time | the HUMAN run's wall-clock, injected as `runtime_seconds_reported` | the harness NEVER measures time (no wall clock anywhere — the determinism discipline); negative/non-finite recordings refused |
| Communication sensitivity | `fallback_staleness_statistics` over the P8.08 failure-mode outcomes | fallback counts per reason, fallback-ERROR counts (recorded failures, never merged), total staleness events, runs-with-stale, named abandoned participants |
| Independent validation outcome | injected `ValidationOutcomeRecord` from the P8.05 harness | a reported solution with outcome `NotRun` is REFUSED at row assembly (`VALIDATION_OUTCOME_MISSING`) — the ledger's acceptance criterion enforced in code, not prose |

## 3. Per-case result table — ALL MEASURED CELLS EMPTY

One row per case; every measured cell is filled ONLY by executing §6's
commands. Each filled row must carry the pinned-version refs (provider
stamps, baseline versions, criteria, benchmark-set version) — a number
without them is invalid and must not be entered.

| Case id (RECORD-AT-FIRST-RUN) | Family | Status (P8.08 vocabulary) | Iterations | r (cost/MW units) vs ε_pri | s vs ε_dual | Obj vs Tier-1 (gap; meaningful?) | Obj vs Tier-2 (gap; meaningful?) | Obj vs priority-list (gap) | Fallback used (baseline/reason) | Stale events | Runtime (s, human-recorded) | P8.05 validation outcome | Win/Tie/Loss (pre-registered tolerance REQUIRED) |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| EMPTY | IEEE feeder | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY |
| EMPTY | IEEE feeder | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY |
| EMPTY | synthetic portfolio | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY |
| EMPTY | multi-feeder substation | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY |
| EMPTY | stressed scenario | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY |

Rules for whoever fills this table (recorded now, before any number):

- Cells come from the harness's `AdmmCaseEvaluation::report_line()` outputs
  and the aggregates (`convergence_statistics`,
  `fallback_staleness_statistics`), never from memory, never estimated. An
  in-band refusal (`VALIDATION_OUTCOME_MISSING`, `OBJECTIVE_GAP_NOT_MEANINGFUL`,
  `NO_BASELINE_INJECTED`, `THRESHOLD_NOT_PRE_REGISTERED`,
  `BASELINE_UNKNOWN`) is recorded AS THE REFUSAL in the cell — never as a
  blank and never as a number.
- A non-converged run is recorded as `NON_CONVERGED_BUDGET_EXHAUSTED`
  (with the fallback outcome), never as a result. An infeasible case is
  `INFEASIBLE_CERTIFICATE` with zero iterations. An abandoned run names
  its participant.
- The Win/Tie/Loss column DOES NOT EXIST until the pre-registered tie
  tolerance is injected from the P8.06 freeze record — the harness refuses
  otherwise. Wins, LOSSES, and ties are all reported (the ledger step 3:
  "report wins, losses, and ties without selecting favourable cases").
- The objective-gap columns are filled only where the comparison is
  declared meaningful; a non-meaningful cell records
  `OBJECTIVE_GAP_NOT_MEANINGFUL (reason)`, never a computed number.
- The units are canonical: MW/MWh in the schedules and deficits; the
  objective is in the declared class's cost units (per MW of curvature);
  `P > 0` = export at the point of connection (§10.2).

## 4. The evaluation note skeleton (ledger step 4 — written BEFORE the runs, as required)

The headings are frozen; the content is EMPTY until the runs exist:

- **Where ADMM helps** — EMPTY (hypothesis H-M1, P8.02 §4 M1, is the
  pre-registered form; nothing is known until the runs).
- **Where ADMM does not** — EMPTY; losses reported alongside wins.
- **What would change the answer** — EMPTY (problem size, coupling
  strength, communication reliability — the ledger's named axes; measured
  from the case matrix, not asserted).

Fixed limits stated now (independent of results):

- Convergence evidence is residual convergence under the P8.07 convexity
  assumptions (`ASSUMPTION_CLOSED_PROPER_CONVEX_BLOCKS` et al.) — no
  iteration-count guarantee, no uniqueness claim, NO global-optimality
  claim outside the supported class (X-3).
- **Solver-class discipline, VERBATIM (AGENTS.md rule 4; §7.4):** ADMM is
  NEITHER LP/QP/MIP (the HiGHS class) NOR convex conic (the Clarabel
  class); its convergence claim rests on the stated convexity assumptions,
  not on a solver's certificate; AC power flow is not AC-OPF. Nonconvex/
  mixed-integer instances are reported as heuristic behaviour with
  residuals and fallback outcomes, never as convergent (P8.02 X-3).
- A finite test set with zero observed violations (or zero non-converged
  runs) is a TEST OUTCOME, not a universal safety or convergence guarantee
  (§13.3; X-4).
- Proposals only (ADR-010): every evaluated schedule is a candidate that
  entered the protected path; nothing here is a dispatch decision.

## 5. What the harness computes vs what the human run supplies

| Supplied by | Content |
|---|---|
| `admm_evaluation.rs` (written) | the metric definitions and their refusal discipline; aggregates over INJECTED `AdmmRunResult`s; gap/comparison arithmetic with the named refusals; the canonical `report_line()` and the results digest (`results_digest` over `baselines::results_digest_for`) for the frozen-artifact discipline |
| The human/lab run | the §19.3 case runs themselves (benchmark-set version + P8.19 extensions); the frozen P8.06 baseline OBJECTIVES per case (the results artifact — which does not exist until the first run, `05_06_harness_baselines.md` §3); the wall-clock runtimes; the pre-registered tie tolerances; the per-case P8.05 validation outcomes; the fills of §3 and §4 |

## 6. Reviewer-verification procedure and exact filling commands (the human gate)

```sh
# (a) build + the in-module test suites (the hand-computed arithmetic):
cargo build -p flexcore-research
cargo test -p flexcore-research            # admm_evaluation suite runs here
cargo test -p flexcore-research --test admm_dmpc_proposal_flow
cargo clippy -p flexcore-research -- -D warnings
cargo fmt --check

# (b) the grep discipline carried from P8.07 (every "converge"/"optimal"
#     hit must sit inside a sentence scoped by the ASSUMPTION_* constants):
grep -n "converge\|global optimum\|optimality" crates/flexcore-research/src/admm_evaluation.rs

# (c) the documented-tools gate:
python tools/verify_docs.py
python tools/validate_agent.py
python -I tools/check_dependency_rule.py

# (d) FIRST RUN — the fills of §2.1/§3/§4 (human/lab inputs, in the
#     recorded isolated research environment):
#     1. freeze the P8.06 baseline results artifact per case (the
#        registry's freeze/`results_digest_for` path), with versions,
#        seeds, and tolerances recorded;
#     2. run the provider per case via the P8.08 operational contract
#        (recorded criteria; wall-clock recorded OUTSIDE the crate);
#     3. pass every proposal through the P8.05 harness; record the
#        outcome per case;
#     4. pre-register the tie tolerances (P8.06 freeze record) BEFORE
#        filling the Win/Tie/Loss column;
#     5. assemble the rows with `evaluate_case` / the aggregates; freeze
#        the digest (`results_digest`); fill §3/§4 from the recorded
#        outputs only;
#     6. independent re-run of two cases from the recorded inputs (the
#        ledger's verification method) and confirm reproduction.
```

A green (a)/(b)/(c) earns only a one-line "human build/test gate: PASSED
(date)" appended here; failures are recorded verbatim and the ledger tick
reverts to `- [ ]`. The measured cells of §3/§4 stay EMPTY until the (d)
runs fill them.

## 7. Acceptance criteria — honest status

| Ledger acceptance criterion | Status |
|---|---|
| All benchmark cases run and reported (no case silently dropped) | **UNRUN** — template with EMPTY cells (§2.1/§3); runs are the human/lab gate |
| Every reported solution passed independent validation | ENFORCED IN CODE at row assembly (`VALIDATION_OUTCOME_MISSING` refusal); no reported solution exists yet |
| Losses reported alongside wins | Rule recorded (§3 filling rules; §4); the comparison column requires the pre-registered tolerance (still missing) |
| No global-optimality claim outside the stated convexity assumptions | §4, written before any run; grep discipline in §6(b) |

## 8. Contract gaps → Phase 7 follow-ups (recorded, not invented)

1. **The P8.06 frozen RESULTS artifact does not exist** (`05_06_harness_baselines.md` §3 recorded this): the objective-gap columns cannot be filled until the first human/lab run freezes the baseline objectives per case. No gap fix is invented here; the registry's freeze/reproduction path is the recorded mechanism.
2. **Tier-1/Tier-2 baseline objectives per case require the composition root** to implement the `Optimizer` port over the HiGHS/Clarabel seams and emit the baseline objectives into `BaselineObjectiveInjection` — the seam is recorded (`OPTIMIZER_PORT_ALIGNMENT_SEAM`, P8.07 §5), not implemented; wiring it is the activation task, never a parallel contract.
3. **The Phase 7 gRPC worker shell** for the provider (P8.07 §5 `P7_GRPC_WORKER_SEAM`) remains the recorded activation task; the evaluation runs against the Rust operational contract until the shell composes it.
4. **Wall-clock runtime is outside the crate by design** (determinism; no wall clock anywhere in `flexcore-research`): the runtime column is a human-run measurement injected into the harness, not a module feature — a recorded separation, not a defect.

## 9. Non-claims (restated)

No global-convergence or global-optimality claim beyond the P8.07
assumptions; no iteration-count guarantee; no safety claim; no field
authority (proposals only, ADR-010); no verdict without a pre-registered
threshold; no objective gap against a nonconvex/mixed-integer comparison;
no benchmark, runtime, residual, convergence, fallback-frequency, or
staleness figure from a real run exists anywhere in this record or its
code. The tests' toy arithmetic pins the metric definitions and is not a
measurement.
