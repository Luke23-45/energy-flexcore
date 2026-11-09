# P0.13 — Solver-class discipline freeze

| Field | Value |
|---|---|
| Subtask | P0.13 — Freeze the solver-class discipline |
| Report trace | §7.4 (Python numerical stack; solver selection rule), §12.3 (optimization tiers), §12.6 (optimization result contract), §11.2 (BESS binary-variable rule); AGENTS.md §3 rule 4 |
| Source | `docs/plan/Neryva_FlexCore_Product_and_Architecture_Report_v1.1.md` (product baseline; foundations specification governs on conflict per AGENTS.md §2) |
| Inputs | P0.12 (grid-model contract) |
| Status | Draft for Stage 0 review |
| Decisions | D1–D6 remain OPEN (§25.2); no solver is selected by this document — provider selection follows the §7.4 provider-contract rule in later phases |

---

## 1. Purpose

Freeze the solver vocabulary so no later document or implementation conflates solver classes (ledger P0.13). The rule is absolute: solver capabilities must be checked against the actual problem class (§12.3, closing sentence), and the classes LP/MILP, SOCP, MISOCP, nonlinear AC power flow, and AC-OPF must not be conflated (§7.4 solver selection rule, opening sentence).

## 2. Solver-class table

| Solver / tool | Problem class it actually solves | What it must never be described as | Report trace |
|---|---|---|---|
| HiGHS | LP / QP / MIP | **Not SOCP.** HiGHS should not be described as an SOCP solver (§12.3); "LP/QP/MIP class; not an SOCP solver" (§7.4, [R28]) | §7.4, §12.3 |
| Clarabel | Convex conic optimization, including SOCP (Python/Rust implementations) | **Not MISOCP.** Clarabel supports continuous conic optimization including SOCP but not integer variables (§12.3); "Continuous conic problems; do not claim integer support" (§7.4) | §7.4, §12.3 |
| CVXPY | Modelling layer whose capabilities depend on the selected solver | Not itself a solver class; it confers no capability beyond the solver behind it (§7.4 "Convex SOCP" row: "CVXPY where suitable") | §7.4 |
| IPOPT | Local nonlinear optimization | **No global-optimality guarantee.** IPOPT can solve nonlinear optimization formulations locally; it does not guarantee global optimality for nonconvex AC-OPF (§7.4); "Local nonlinear optimization; no universal global optimum guarantee" (§7.4) | §7.4 |
| AC power flow | Evaluates the network state for specified injections/controls | **Not AC-OPF.** It is not the same as solving AC-OPF (§7.4); a feasibility evaluation, not an optimization (§12.5) | §7.4, §12.3, §12.5 |
| AC-OPF | Nonlinear optimal power-flow optimization | Not a power-flow feasibility check; universal AC-OPF global optimality is a §0.3 non-promise | §0.3, §7.4, §12.3 |

Verification note (ledger P0.13): Phase 0 artefacts are checked for "SOCP" attached to HiGHS, "MISOCP" attached to Clarabel, and "global" attached to IPOPT — zero hits required. This document contains none of those claims; the phrases above occur only inside explicit negations.

## 3. Optimization tiers (§12.3 — all four, plus the validation row)

| Tier | Formulation | Initial use (§12.3) |
|---|---|---|
| Tier 1 | LP / QP / MILP | Portfolio dispatch and scheduling with linearized network/asset models |
| Tier 2 | Continuous SOCP | Convex network approximations and inverter capability constraints where applicable |
| Tier 3 | MISOCP / MINLP | Only when discrete decisions materially require it and a suitable solver is available |
| Tier 4 | Nonlinear AC-OPF | Advanced planning/optimization research and validated use cases |
| Validation | AC power flow / three-phase unbalanced power flow | Evaluate a candidate schedule/dispatch against the chosen network model |

The validation row is not a fifth tier: it is the independent feasibility evaluation every candidate passes through (§12.5 pipeline; `12_grid_model_contract.md` §6).

## 4. Provider-contract declaration list (§7.4)

The solver provider contract must declare (§7.4, closing sentence):

| # | Required declaration |
|---|---|
| 1 | Problem classes |
| 2 | Supported constraint types |
| 3 | Scaling |
| 4 | Tolerances |
| 5 | Warm-start support |
| 6 | Licensing |
| 7 | Determinism |
| 8 | Diagnostic outputs |

This mirrors the Strategy-pattern guardrail: a provider must declare capability/problem class and diagnostics (§6.4 Strategy row). Licensing declarations use the US spelling "license" per `.agent/WRITING.md`.

## 5. Mixed-integer conic rule

A mixed-integer conic problem needs a solver that actually supports that class, or a different linearized/MILP formulation (§7.4). A continuous-conic solver (Clarabel) plus an integer outer loop is not MISOCP capability, and a mixed-integer problem may not be labelled SOCP to make it fit a continuous solver.

## 6. BESS binary-variable rule (§11.2)

If simultaneous charge and discharge must be prohibited in an optimization formulation, use a valid mixed-integer mode formulation or a justified convex relaxation plus post-solution checks. **Do not label a model with binary charge/discharge variables as a continuous SOCP** (§11.2, closing sentence of the constraint block).

Consequence: a BESS scheduling model that introduces binary charge/discharge variables is a mixed-integer model (MIP/MILP — or MISOCP if conic constraints and a capable solver are used, per §5), regardless of how convex the remaining constraints are. It is never "continuous SOCP".

## 7. Solver results and diagnostics (§12.6)

Every optimization result returns, at minimum (§12.6):

- `status`: optimal / feasible / infeasible / timeout / numerical_failure / unsupported;
- problem class and solver/provider version;
- objective value and units;
- primal and dual diagnostics where available;
- constraint residuals and violations;
- runtime and resource use;
- input snapshot and grid model version;
- uncertainty and assumptions;
- warnings; and
- output candidate ID.

The problem class and solver version are first-class result fields: a result that does not name its class and solver cannot be interpreted under the discipline in §2. The `optimal` status is a solver status for the declared class — for a local nonlinear solve it carries no global-optimality meaning (§7.4; §0.3 "universal AC-OPF global optimality" non-promise).

## 8. What this document does not do

- It selects no solver and no provider; selection follows the §7.4 provider-contract rule in later phases (§4).
- It makes no solver-capability claim beyond the cited report text; external solver claims are report citations ([R28] for HiGHS), not independent assertions (AGENTS.md §3 rule 1).
- It makes no performance, safety, or certification claim.

## 9. Acceptance-criteria coverage (ledger P0.13)

| Ledger acceptance criterion | Where satisfied |
|---|---|
| HiGHS LP/QP/MIP only; Clarabel convex conic, not MISOCP; IPOPT local only | §2 table, with explicit "must never be described as" column |
| Four tiers from §12.3 reproduced | §3 |
| Provider-contract declaration list from §7.4 reproduced | §4 |

Additional ledger steps covered: mixed-integer conic rule (§5), BESS binary-variable rule (§6), solver result contract (§7). Verification method (ledger): grep Phase 0 artefacts for misattached class claims — zero hits; this document's only occurrences are explicit negations (§2 note).
