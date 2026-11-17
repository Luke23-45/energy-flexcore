# P15.08 — Degradation cost curve into the optimizer (INPUT DATA)

| Field | Value |
|---|---|
| Subtask | P15.08 — $/MWh throughput penalty or piecewise-linear cycle-aging cost as optimizer INPUT DATA per asset, feeding the existing `C_degradation` term (P3.14 closed vocabulary — schema-version bump only if the term shape changes, never a new class); solver-class guard tests written proving the MISOCP refusal and tier guards stay [S-148] |
| Wave / agent | Phase 15 wave 1, Agent B (P15.07–P15.11) |
| Status | WRITTEN-UNRUN: the data contract, the objective wiring, and the guard tests are written, never compiled, never executed. No cost curve was fitted, no degradation number was computed, no solver ran. |
| Sources | [S-148] Wankmüller et al. 2017 degradation penalty — cited as the reference for degradation-cost-aware dispatch being an optimizer INPUT; no penalty value, no curve shape, and no accuracy claim is taken from it. |
| Files touched | `crates/flexcore-optimization-api/src/degradation_cost.rs` (NEW), `crates/flexcore-optimization-api/src/lib.rs` (additive module registration), `crates/flexcore-optimization-api/src/problem.rs` (additive guard tests ONLY — the IR types and validators untouched), `crates/flexcore-application/src/scheduling_objective.rs` (additive consts, one error variant, one `ObjectiveInputs` field, validation + provenance wiring), `crates/flexcore-application/tests/scheduling_objective.rs` (additive tests). `bess_model.rs` NOT touched — the P3.06 `DegradationConfig` placeholder is superseded AS DATA (the declared curve), never rewritten. |

## 1. What landed (deliverable class: data contract + guard tests)

| Piece | Where | Honest status |
|---|---|---|
| `DegradationCostCurve` — `Linear { cost_per_mwh_throughput }` (≥ 0) or `PiecewiseLinear { segments }` (strictly ascending cumulative-throughput limits, piecewise-constant marginal cost ≥ 0); serde shapes; `DEGRADATION_COST_DATA_SCHEMA_VERSION = 1` | `degradation_cost.rs` | Implemented; UNRUN |
| Convexity gate: `validate_convex_for_minimization` — a piecewise curve's marginal costs must be non-decreasing so the epigraph formulation stays an LP/QP/MILP; non-convex data is refused with a named error carrying the RESHAPE-THE-DATA guidance (a convex piecewise-linear approximation declared with provenance) — never a new solver class | same | Implemented; UNRUN |
| `marginal_cost_at(cumulative_throughput)` — reads the declared marginal cost (linear rate; segment-resolved for PWL, lower-bound-exclusive/upper-inclusive); beyond the declared coverage: REFUSED, never extrapolated (P3.02) | same | Implemented; UNRUN |
| `DegradationCostDeclaration` — per-asset: required non-empty `curve_version` (P3.15 no-untracked-constants), REQUIRED non-empty `provenance_note`, chemistry label (mirrors `flexcore_domain::bess_degradation::DegradationChemistry` BY CITATION — this crate's edge set is flexcore-contracts + serde/serde_json/time/clarabel); `require_in_scope()` refuses out-of-scope-chemistry use (P15-E-3 DEFAULT (a): LFP/NMC first [S-147]) | same | Implemented; UNRUN |
| Guard tests (module): linear/PWL validation, segment ordering refusals, beyond-coverage refusal, non-convex refusal with guidance text, declaration/provenance validation, out-of-scope refusal | same | Written complete; UNRUN |
| Guard tests (problem.rs `#[cfg(test)]`, additive only): `p15_08_degradation_data_keeps_the_lp_an_lp` (validate OK, Tier 1, tier guard passes), `..._keeps_the_milp_a_milp`, `..._does_not_widen_the_class_when_cones_are_added` (ClassMismatch fires exactly as before), `..._does_not_relax_the_tier3_refusal` (MISOCP declaration carrying degradation linear terms still refused by `guard_problem` with `unsupported`, tier 3) | `problem.rs` | Written complete; UNRUN |
| Objective wiring: `DEGRADATION_INPUT_DATA_SCHEMA_VERSION = 2` (v1 = P3.06 placeholder metadata only; v2 = + P15.08 curve declarations) with the recorded reasoning that `OBJECTIVE_SCHEMA_VERSION` is NOT bumped — the term set and the `C_degradation` term SHAPE are unchanged, only the accepted INPUT DATA changed (P3.14: a bump is required for a term change); `ObjectiveInputs.degradation_cost_curves: Vec<DegradationCostDeclaration>` (empty default — every existing construction uses `..ObjectiveInputs::new(..)` and is unaffected); validation (curves with the term disabled → refused — unused data never passes silently; curve naming an asset without P3.06 metadata → refused; out-of-scope chemistry → refused; non-convex PWL → refused); curve provenance travels in `term_provenance` (source, version, chemistry, ADR-010 line); the degradation note names the v2 input-data contract | `scheduling_objective.rs` | Implemented; UNRUN |
| Objective wiring tests (additive): provenance/note travel, unused-data refusal, missing-metadata refusal, out-of-scope refusal, non-convex refusal | `tests/scheduling_objective.rs` | Written complete; UNRUN |
| `DEGRADATION_COST_DATA_STATEMENT` / `DEGRADATION_CHEMISTRY_SCOPE_STATEMENT` — the recorded discipline and the P15-E-3 DEFAULT, carried in code constants | `degradation_cost.rs` | Implemented; UNRUN |

## 2. Solver-class discipline — the absolute rule, proven by tests (WRITTEN-UNRUN)

1. The curves are DATA the formulation assembler turns into linear
   objective terms; the epigraph of a CONVEX piecewise-linear cost in a
   minimization is still an LP/QP/MILP. Nothing in this delivery touches
   `ProblemClass`, `tier()`, `validate_class_consistency`, `tier_guard`,
   or either provider. No new problem class exists.
2. The guard tests prove the negative space: degradation data present does
   NOT widen an LP to admit cones (ClassMismatch), does NOT relax the
   Tier-3 refusal (MISOCP + degradation data still `unsupported`), and
   does NOT change what an LP/MILP declaration validates as.
3. A data shape that would create a new problem class (non-convex PWL) is
   refused AT THE DATA CONTRACT with the reshape guidance — the DATA is
   reshaped, never the solver (conventions §6.5).
4. Solver-class vocabulary stays exact: HiGHS is LP/QP/MIP, not SOCP;
   Clarabel is convex conic, not MISOCP; nothing here claims otherwise.

## 3. The P3.06 placeholder — superseded AS DATA, in the recorded split

- The `DegradationConfig` placeholder in `bess_model.rs` is untouched: its
  placeholder metadata remains the EVALUATION-side record.
- The optimizer INPUT is the declared curve
  (`DegradationCostDeclaration`); the application-side objective wiring
  validates that a curve's asset also carries evaluated P3.06 metadata (the
  curve feeds the SAME `C_degradation` term the metadata evaluation
  produced). The evaluated `C_degradation` total remains the P3.06 output —
  the split is recorded in the degradation note, never silently merged.

## 4. ADR-010 (conventions §6.3)

A degradation price is INPUT DATA for the simulation-line objective. It
never authorizes or produces a dispatch command; the statement travels in
the data-contract constant, in the term-provenance note, and in the module
headers. The never-granted permission set is untouched (no permission
surface is edited by this subtask).

## 5. UNVERIFIED register (implementation-time checks)

| # | Detail | Where |
|---|---|---|
| 1 | Every curve value is ASSUMPTION-grade declared data until a human-gate parameterization per chemistry (P15-E-3 closure) supplies reviewed numbers | `DegradationCostDeclaration.provenance_note` (required) |
| 2 | Whether warranty-driven marginal-cost shapes in the reference literature [S-148] match the piecewise-constant marginal form — UNVERIFIED; the declared shapes are the only ones representable without a new problem class | `DegradationCostCurve` |

## 6. Verification (permitted tools)

Outputs of `python tools/verify_docs.py`, `python tools/validate_agent.py`,
`python -I tools/check_dependency_rule.py` quoted verbatim in
`26_phase_15_wave1_agent_b_close_out.md`. No build, no test run (conventions
§1.3).
