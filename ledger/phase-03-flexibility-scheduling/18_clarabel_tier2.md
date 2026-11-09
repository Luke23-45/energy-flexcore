# P3.18 — Decision note: the Tier 2 (continuous SOCP) provider via Clarabel

- **Date:** 2026-11-07.
- **Subtask:** P3.18 (ledger_phase_3.md §P3.18). Report trace: §12.3, §12.5, §11.3, §7.4. Foundations trace: §7.8.
- **Deliverable crate:** `crates/flexcore-optimization-api` (core crate).

## 1. Recorded decision — D-P3.18-1: Tier 2 solves in-process with the native Rust crate, behind the same `Optimizer` port

**Decision.** The Tier 2 (continuous SOCP) solve path is the **native Rust
crate `clarabel`** invoked **in-process** by
`crates/flexcore-optimization-api/src/clarabel_provider.rs`
(`ClarabelTier2Provider`), implementing the same `Optimizer` port as the Tier
1 provider (report §9.1). Unlike Tier 1 (decision D-P3.17-1, Python worker),
there is **no transport boundary and therefore no `Tier2Backend` seam**: the
`Tier1Backend` seam exists because the Tier 1 solver executes in a separate
Python worker; Clarabel.rs is a native Rust implementation (RESEARCH.md §2:
"the Rust crate is a native implementation, not a binding, which fits the
Rust control plane §7.3"). A seam here would be a pass-through with nothing
to hide, so `src/backend.rs` is intentionally untouched.

The provider owns the full pipeline in Rust, mirroring the Tier 1 structure:
configuration validation → integrality gate → formulation validation → class
gate → conic assembly → translation to the pinned solver's standard form →
solve → §12.6 status mapping → completeness self-check. The in-process
analogue of the Tier 1 backend call counter is
`ClarabelTier2Provider::solver_invocations()` (an atomic counter incremented
immediately before the solver invocation); tests prove every gate-rejection
path leaves it at zero.

**Version pin (recorded).**

| Item | Value | Source |
|---|---|---|
| Tier 2 solver | **Clarabel.rs 0.11.1** (the RUST crate `clarabel`) | RESEARCH.md §2 (docs.rs, CHANGELOG entry 0.11.1 dated 2025-11-06; read 2026-11-07). The R package `clarabel` 0.11.3 tracks Clarabel.rs 0.11.1 — do not confuse the two. |
| Class | Convex conic: LP, QP, SOCP, SDP, exponential/power cones. **No integer variables — not MISOCP.** | RESEARCH.md §2; report [R14] |
| License | **Apache-2.0** | RESEARCH.md §2. Recorded here as D4-audit input; the formal D4 audit (gate, before v0.1.0) is still OPEN and owned by legal + engineering. |

**Manifest handoff (coordinator owns manifests; no manifest dependency line
was edited by this subtask).** The provider code compiles once these lines
land:

1. In the root `Cargo.toml`, `[workspace.dependencies]`:

   ```toml
   clarabel = "=0.11.1"
   ```

2. In `crates/flexcore-optimization-api/Cargo.toml`, `[dependencies]`:

   ```toml
   clarabel = { workspace = true }
   ```

   (A header note to this effect is already in the crate manifest, matching
   the P3.17 handover pattern.)
3. `tools/check_dependency_rule.py`: if the external-dependency policy
   validates core-crate dependencies, allow `clarabel` for
   `flexcore-optimization-api`. The exact-equality manifest scan in
   `tests/dependency_direction.rs` has already been updated (see §3) to admit
   exactly this one additional dependency and nothing else.

## 2. Problem-class discipline (AGENTS.md rule 3.4 — absolute)

- Clarabel is a **convex conic** solver: LP/QP/SOCP/SDP plus exponential and
  power cones. It has **no integer variables** and is **not a MISOCP solver**.
  It is never described as MISOCP-capable, and no global-optimality or
  exactness claim is made for anything it returns.
- Gate order in `solve()` (all before any solver invocation; tested with the
  invocation counter at zero):
  1. Per-run `SolverConfig` and provider-level `Tier2ProviderConfig`
     validation (`OptimizerError::InvalidConfig`).
  2. **Integrality gate**: any `Integer`/`Binary` variable → in-band
     `unsupported` result stating that the pinned solver is continuous conic
     and not MISOCP, and that an integer formulation is never silently solved
     as if it were continuous (ledger P3.18 step 2; ENTRY_NOTE discipline).
     The gate fires before structural validation, so it catches
     integer-carrying formulations whatever their declared class (a MILP or
     MISOCP declaration handed to Tier 2 is rejected the same way).
  3. `OptimizationProblem::validate()` (`OptimizerError::MalformedProblem`).
  4. **Class gate**: only `ContinuousSecondOrderConeProgram` is solved; every
     other declared class (LP/QP/MILP → Tier 1; MISOCP Tier 3 and nonlinear
     Tier 4 — not built, P3.20) → in-band `unsupported`.
- The capability surface declares `SecondOrderCones` and **never**
  `Integrality` (tested).

## 3. Dependency-direction scan update (P3.17 test, additive change)

`tests/dependency_direction.rs` (P3.17) asserted the crate manifest's
dependency list to be **exactly** the core-crate set. The coordinator's
P3.18 manifest lines would have broken that exact-equality assertion, so the
test was updated in place (this is the only edit to a P3.17 test):

- The allowed manifest set is extended by exactly `clarabel` (needle
  assembled from halves, per the test's own convention).
- The port-surface source scan now also rejects the Tier 2 solver name, so
  `port`/`problem`/`result`/`config`/`backend` stay solver-neutral for both
  tiers; the provider modules (`highs_provider.rs`, `clarabel_provider.rs`)
  remain the only places naming a solver.

## 4. Status mapping (§12.6 — closed six-status vocabulary)

The native solver statuses (`clarabel::solver::SolverStatus`, 12 variants at
0.11.1) map onto §12.6 as recorded in `clarabel_provider.rs` (function
`map_clarabel_status`, exhaustive-match-guarded in tests):

| SolverStatus (clarabel 0.11.1) | §12.6 status | Notes |
|---|---|---|
| Solved | `optimal` | proven optimal **for the declared conic formulation only** |
| AlmostSolved | `feasible` | reduced-accuracy solution; no proven-optimality claim (the §12.6 `feasible` definition); explicit warning recorded |
| PrimalInfeasible | `infeasible` | carries an explicit not-available reason for conflict evidence (§5) — never a relaxed schedule |
| AlmostPrimalInfeasible | `numerical_failure` | a reduced-accuracy infeasibility certificate is never published as an infeasible verdict (Tier 1 discipline: uncertain outcomes are never mapped to `infeasible`) |
| DualInfeasible | `numerical_failure` | dual infeasibility implies an unbounded primal objective; the closed vocabulary has no `unbounded` value — mirrors the Tier 1 `Unbounded` mapping; verbatim status recorded |
| AlmostDualInfeasible | `numerical_failure` | same reasoning, reduced accuracy |
| MaxIterations | `timeout` | the note records that the limit was the iteration budget |
| MaxTime | `timeout` | |
| NumericalError / InsufficientProgress / CallbackTerminated / Unsolved | `numerical_failure` | fail-closed |

No other §12.6 status string exists; `SolveStatus` is a closed enum.

## 5. Configuration translation and the residual tolerance

- The per-run `SolverConfig` (P3.17 surface, unchanged) translates to
  `DefaultSettings`: `max_threads` pinned from `thread_count` (default 1;
  multithreaded-KKT determinism not assumed), `time_limit`,
  `tol_feas ← primal_feasibility_tolerance`, `tol_gap_abs/rel ←
  dual_feasibility_tolerance` (the pinned solver has no separate
  dual-feasibility knob), `equilibrate_enable ← false` when
  `ScalingPolicy::Disabled`. A configured `mip_relative_gap` is not
  applicable to a continuous conic solve and is recorded as ignored.
- The **per-cone relaxation residual tolerance** is a provider-owned item:
  `Tier2ProviderConfig { relaxation_residual_tolerance }`, default `1e-4`
  (a provider-chosen diagnostic threshold, recorded and reviewable — not a
  solver tolerance and not a certified feasibility margin. Rationale: the
  pinned solver's feasibility/optimality tolerances sit at 1e-7..1e-8 on
  unit-scaled data and a cone slack scales quadratically in the primal
  variables, so a 1e-6-level threshold would flag numerically binding cones
  as violated; `1e-4` leaves that margin). It lives in the provider
  configuration rather than the per-run `SolverConfig` because the
  `SolverConfig` is the frozen P3.17 contract; it is validated per solve.

## 6. Schema note for the frozen-IR review (no IR change made)

The P3.17 problem IR already carries the Tier 2 element needed for the
inverter cone: `SecondOrderCone` (`‖diag(a)·x + b‖₂ ≤ cᵀx + d`), which
expresses the §11.3 apparent-power constraint directly as `‖(P, Q)‖₂ ≤
S_max`. **No additive IR extension was made or needed in this batch**
(problem.rs is frozen for P3.18).

Recorded limitation: the foundations §7.8 residual
`g_ij = v_i·l_ij − (P_ij² + Q_ij²)` needs the variable **roles** of a branch
cone (`v_i`, `l_ij`, `P_ij`, `Q_ij`). The IR carries no role annotation, so:

- The provider reports, per run, the generic per-cone slack
  `slack = (rhs(x))² − ‖diag(a)·x + b‖₂` for EVERY declared cone, evaluated
  against the configured tolerance (`relaxation_residual_report`); for a
  branch cone encoded canonically as `‖(2·P_ij, 2·Q_ij, v_i − l_ij)‖₂ ≤
  v_i + l_ij`, `slack = 4·g_ij` exactly (tested against hand-computed
  values). Violations and slack beyond tolerance are reported in the result
  warnings, never hidden, and never replace the Phase 4 AC power-flow
  verification.
- The exact §7.8 formula is exposed as `distflow_branch_residual`, applied
  per branch via explicit `BranchResidualSpec` role records carried with the
  shared fixtures (`conic_fixture::distflow_branch_specs`) — the P3.23
  comparison workflow consumes these.
- **Recommendation for the frozen-IR review:** an optional
  `BranchRelaxationCone { cone_name, branch, send_voltage_squared,
  branch_current_squared, send_active_flow, send_reactive_flow }` annotation
  on `OptimizationProblem` would let any provider compute per-branch
  residuals without out-of-band specs. Not implemented here to keep the IR
  frozen; recorded so the review can decide.

## 7. VERIFY-BUILD list (established repo pattern; all offline-unverifiable)

The pinned-version API facts below were taken from docs.rs `clarabel` 0.11.1
and the crate's own `examples/rust/example_socp.rs` / `example_qp.rs`
(2026-11-07) — no build ran in this batch (per subtask constraint). All
carry `VERIFY-BUILD:` markers in code:

1. The real-solve tests (`tests/clarabel_provider.rs::verify_build_*`): the
   continuous SOCP and DistFlow fixtures solving through the port path with
   their hand-computed optima. They run once the §1 manifest lines land.
2. `DefaultSolver::new(&P, &q, &A, &b, &cones, settings)` returns
   `Result<Self, SolverError>`; `SolverError` must implement `Display`.
3. Second-order cone element order: the scalar `t` is the FIRST element of a
   `SecondOrderConeT(dim)` block (`‖(u₁,…,u_{dim−1})‖₂ ≤ t`), as demonstrated
   by the crate's own SOCP example.
4. Objective convention `minimize ½·xᵀPx + qᵀx` and constant handling:
   `solution.obj_val` excludes the IR objective constant; the provider
   re-applies it (and the maximisation sign flip) when reporting.
5. Quadratic-objective P-matrix translation (`P_ii = 2·c_ii`, symmetric
   `P_ij = P_ji = c_ij`, sign-flipped for maximisation).
6. Settings field names/types and default tolerance values (`max_threads`,
   `time_limit`, `tol_feas`, `tol_gap_abs/rel`, `equilibrate_enable`).
7. Dual sign conventions: raw `z` multipliers are recorded per linear row
   (rows are emitted before cones); upper-bound rows are encoded with
   negated coefficients, and `Range` rows expand to two conic rows — duals
   are recorded as not-available when any range row exists rather than
   partially recomputed (tested).
8. Deterministic replay over the real solver (same inputs + pinned version +
   same config → identical result records) lands with P3.26; this batch's
   tests are the offline-deterministic parts plus the VERIFY-BUILD real-solve
   tests.

## 8. Cross-check hook for P3.23 (ledger step 3)

`src/conic_fixture.rs` provides the shared fixture pair and the record — not
the P3.23 workflow:

- `inverter_cone_socp` (Tier 2, §11.3 cone `‖(P, Q)‖₂ ≤ 5` with `Q` pinned to
  3 MVAr, maximise `P`; hand optimum `P = 4`) and
  `inverter_linearised_lp` (Tier 1, the SAME scenario with the box
  linearisation `P ≤ 5`, `Q ≤ 5`; hand optimum `P = 5`). The linearisation
  overestimates deliverable export by exactly 1 MW at the shared operating
  point — the gap the cross-check exists to expose.
- `build_comparison_record` produces the comparison record fields
  (`Tier1Tier2ComparisonRecord`): per-candidate problem class, status,
  objective value + units, binding sets (linear rows via
  `binding_linear_rows`, cones via `binding_cones`), and the objective
  difference, with the Phase 4 caveat recorded in the notes.
- `distflow_branch_socp` adds the canonical foundations §7.8 branch fixture
  (hand optimum `l = 4.25`, `g_ij = 0`, cone binding).

## 9. What P3.18 does NOT do

- No MISOCP: the integrality gate rejects every integer-carrying
  formulation before any solver invocation (tested, counter-proved); Tier 3
  is not built (P3.20).
- No exactness claim, no global-optimality claim, no feasibility claim: the
  §12.5 caveat (`SOCP_EXACTNESS_CAVEAT`) is recorded on the capability
  surface and verbatim on every solved result. The residual is diagnostic
  evidence only; it does not replace the Phase 4 AC power-flow verification.
- No Tier 1 change: `backend.rs`, `highs_provider.rs`, `port.rs`,
  `problem.rs`, `result.rs`, `config.rs` are untouched; `lib.rs` received
  only additive module declarations, doc rows, and re-exports.
- No warm-start claim (the persistent solver API exists in the 0.11.x line —
  RESEARCH.md §2 — but nothing is verified).
- No command authority, no field control, no dispatch: the entire 0.x line
  is simulation-only, and every solved schedule remains a candidate.
