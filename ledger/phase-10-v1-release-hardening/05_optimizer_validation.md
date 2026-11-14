# P10.05 — Optimizer Independent Validation (record)

- **Subtask:** P10.05 (§22.3 release gate 4; report §12.3, §12.5–§12.6, §9.2).
- **Status:** WRITTEN-UNRUN — implemented, awaiting human build/test (ENTRY_NOTE
  boundary 3). No `cargo` command was executed in this loop. Additionally,
  ENTRY_NOTE boundary 4: the benchmark/measurement cells of this subtask are
  EMPTY by design (see §3) — no optimizer has been built or run in this loop,
  and the validation recorded here is of the CONTRACT, not a solver binary.
- **Dependencies honesty:** P10.01/P10.18 are concurrent waves; the validation
  set must be re-run against the frozen release-candidate identifier. P9.03
  (pilot grid model) and P9.19 (shadow cases) are cross-references only — no
  shadow case was re-run, no pilot model was loaded.

## 1. The solver-class discipline, verbatim (AGENTS.md rule 4 / report §12.3)

| Solver / provider | Declared class (recorded, test-pinned) | NEVER |
|---|---|---|
| HiGHS (`HighsTier1Provider`, `builtin.optimizer-highs-tier1`, pin `highspy 1.15.1`) | LP / convex QP / MILP (§12.3 Tier 1) | NOT an SOCP solver; a conic or nonlinear declaration is refused `unsupported` BEFORE any backend invocation (call counter == 0, pinned by test) |
| Clarabel (`ClarabelTier2Provider`, `builtin.optimizer-clarabel-tier2`, pin `0.11.1`) | Continuous convex conic (SOCP, §12.3 Tier 2) | NOT MISOCP — no integer variables; the integrality gate refuses an integer formulation `unsupported` before the solver runs (invocation counter == 0, pinned); NOT an LP/QP solver — an LP declaration is refused by the class gate |
| IPOPT | LOCAL nonlinear only; NO global-optimality guarantee (`IPOPT_LOCALITY_NOTE`) | Not wired as a scheduler solver; any future use is a Stage 8 item |
| Tier 3 (MISOCP/MINLP), Tier 4 (nonlinear AC-OPF) | NOT BUILT (`TIER_BOUNDARY_POLICY`) | A Tier 3/4 request is refused `unsupported` with the decision reference and the reformulation rule |
| AC power flow | A feasibility/validation tool (§12.5) | NOT AC-OPF; never a global-optimality claim |

## 2. What was produced (files)

| File | Content |
|---|---|
| `crates/flexcore-optimization-api/tests/p10_05_result_contract_validation.rs` | The §12.6 contract slice: the closed six-status vocabulary (`unbounded` is NOT a status — an unbounded objective maps to `numerical_failure`, never a pass); the provider/class table exact-match (HiGHS declares exactly LP/QP/MILP, `SecondOrderCones` never declared; Clarabel declares exactly continuous SOCP); wrong-class declarations refused `unsupported` with the backend/solver invocation counters at zero; infeasible-with-conflict-evidence (never a relaxed schedule), timeout, and unbounded→`numerical_failure` outcomes; the §12.6 completeness contract asserted on optimal/feasible/infeasible/timeout/unsupported records (`validate_complete`), and a NIL-candidate-id result refused as a provider bug (`IncompleteResult`), not a status; the Tier 3/4 boundary guard with the decision reference and reformulation rule; a capability declaration naming a Tier 3/4 class fails the boundary check; the empty-benchmark-cell honesty guard (the fixture optimums asserted are the frozen P3.17 reference values, not measurements). |
| `crates/flexcore-research/tests/p10_05_independent_validation.rs` | The independent-validation consumption path (P8.05 harness, step 2 of the subtask): deliberately-wrong optimizer outputs caught — a limit-violating setpoint vector refused `PHYSICAL_REVALIDATION_LIMIT_VIOLATED` with the constraint named; producer self-declared "feasible/optimal/safe" diagnostics change nothing (the proposal type has no verdict field — the optimizer never validates itself); non-converged / failed validation is `PHYSICAL_REVALIDATION_FAIL_CLOSED` and a gate seam error is `PHYSICAL_REVALIDATION_SEAM_FAILED` — never an inferred pass (§12.5 closing rule; §17.5 invariant 9); a full pass yields the sealed `ValidatedProposal` with the hand-computed `feasibility:attempt/<id>:model/<mid>:hash/<h>` evidence reference and the harness version; stale/unknown/superseded snapshots, expired windows and incomplete provenance refuse with ZERO gate attempts (the gate is physically the last stage); the gate is an injected port (different code path from the optimizer) with the recorded alignment seam pinned. |

## 3. EMPTY benchmark cells (honest status — ENTRY_NOTE boundary 4)

- **Provider/class table:** the DECLARED rows above are test-pinned. The
  MEASURED cells (solver version resolution against a built binding, runtime,
  residuals on real models) are EMPTY — no optimizer binary was built or run.
- **Validation runs:** ZERO runs exist. The acceptance criteria "(b) every
  optimizer output in the validation set was checked by the independent
  feasibility path" and "(c) infeasible/timeout cases result in reject/hold"
  are proven at the CONTRACT level by the two test files; the RUN evidence
  (per-run residuals against the pilot grid model, P9.19 shadow cross-reference)
  requires the P10.18 release candidate and is a human gate input.
- **Fill-in commands for the human gate:**

  ```sh
  cargo test -p flexcore-optimization-api --test p10_05_result_contract_validation
  cargo test -p flexcore-research --test p10_05_independent_validation
  # Then, on the release candidate with the P9.03 pilot grid model:
  #   run the §12.5 feasibility gate over each candidate injection and file
  #   the per-run residuals in this record's §5 table (currently empty).
  ```

## 4. Acceptance-criteria coverage and honest gaps

| P10.05 criterion | Covered here | NOT executable in this loop |
|---|---|---|
| (a) every provider's declared class matches the problem it solved | The declared-class tables are pinned and the class/integrality/tier gates refuse every mismatch `unsupported` before any solve (invocation counters pinned at zero). | A solved-problem-vs-declared-class audit over REAL solves needs built providers (the scripted backend is a documented test double). |
| (b) every optimizer output checked by the independent feasibility path | The consumption path is proven: the harness refuses anything the injected §12.5 gate does not pass; the validator and optimizer are different code paths (injected port + recorded seam). | The per-run evidence file (runs, residuals) on the candidate is EMPTY — human gate. |
| (c) infeasible/timeout → reject/hold, never an inferred pass | `infeasible` carries conflict evidence and no objective; `timeout` and `unbounded`→`numerical_failure` never publish a solution; a FailClosed gate verdict and a seam error are refusals; stale/unknown models refuse with zero gate attempts. | The dispatch-policy wiring (reject vs hold per §14 policy) is the command-lifecycle path (P10.03's suite) — cross-referenced. |
| (d) every run carries the full §12.6 contract | `validate_complete` asserted on every mapped outcome in the suites; an incomplete result is a named provider bug (`OptimizerError::IncompleteResult`). | Contract-only; real runs carry their own records once they exist. |

## 5. Validation-run table (EMPTY — human gate input)

| Run | Candidate injection | Declared class | Independent-path verdict | Residuals | Disposition |
|---|---|---|---|---|---|
| (none) | — | — | — | — | — |

## 6. Findings (recorded, not fixed — src logic is outside this subtask's ownership)

- **F-P10.05-1 (observation, no action taken):** the §12.6 vocabulary has no
  `unbounded` status by design; both providers map unbounded outcomes to
  `numerical_failure` with the verbatim status text recorded. This is the
  correct closed-vocabulary behaviour, but a consumer reading only the status
  cannot distinguish "unbounded" from "solver error" without the recorded
  status text/diagnostics. Recommendation: keep the verbatim
  `solver_status_text` in any release-notes guidance to operators.
- **F-P10.05-2 (observation, no action taken):** the HiGHS provider's
  `unsupported` warning text names the decision note but the word "unsupported"
  itself does not appear in the warning (the status carries it). The P10.05
  suite asserts the warning via "no solve was attempted" instead. No defect;
  recorded so future text edits do not silently break the suite's assumption.
- **F-P10.05-3 (recorded constraint):** the FNV-1a-64-style config digest and
  the test-grade digest honesty of the kernel/research seams apply unchanged —
  a production digest is a DEPLOYMENT REQUIREMENT (recorded in those modules);
  the P10.05 suites do not re-assert it. The determinism record (pinned
  version, thread count, config digest) IS asserted present on every mapped
  result.

## 7. Ledger tick

`P10.05` in `ledger_phase_10.md` was ticked in the same wave as this record,
with the ENTRY_NOTE boundary 3/4 meaning: **contract-level tests implemented,
awaiting human build/test; measurement cells EMPTY by design.**
