// P3.18 — Shared conic fixtures and the Tier 1 / Tier 2 cross-check record
// (report §12.3, §11.3, §12.5; foundations §7.8; ledger P3.18 step 3).
//
// These are **synthetic reference formulations** (labelled as such — D1/D2
// are OPEN, so nothing here is market data; P3.14/P3.16 discipline), built
// in the problem IR's terms and shared between the two tiers so the P3.23
// comparison workflow can consume them:
//
// - [`inverter_cone_socp`] — the §11.3 inverter apparent-power cone
//   `P² + Q² ≤ S_max²` as a second-order cone `‖(P, Q)‖₂ ≤ S_max` (the Tier 2
//   path). Hand-computed optimum.
// - [`inverter_linearised_lp`] — the SAME operating scenario with the cone
//   replaced by the Tier 1 linearised box approximation `P ≤ S_max`,
//   `Q ≤ S_max`. Hand-computed optimum.
// - [`distflow_branch_socp`] — one DistFlow branch relaxation cone in the
//   canonical encoding `‖(2·P_ij, 2·Q_ij, v_i − l_ij)‖₂ ≤ v_i + l_ij`, for
//   the foundations §7.8 per-branch residual `g_ij = v_i·l_ij − (P_ij² +
//   Q_ij²)`. Hand-computed optimum.
//
// The reactive-power sign is never interpreted; sign convention is declared
// per grid model and device profile (§10.2). Every fixture's optimum is
// hand-computed and recorded, so the offline-deterministic tests (assembly,
// residuals, binding sets, comparison record) run without the solver; the
// real-solve tests are VERIFY-BUILD items (18_clarabel_tier2.md §7).
//
// Fixtures are reference material, not the P3.26 scenario suite.

use crate::clarabel_provider::{
    binding_cones, binding_linear_rows, distflow_branch_residual, BranchResidualSpec,
};
use crate::port::OptimizerError;
use crate::fixture::ReferenceOptimum;
use crate::problem::{
    Coefficient, LinearRow, Objective, ObjectiveSense, OptimizationProblem, ProblemClass,
    ProblemProvenance, RowBounds, SecondOrderCone, Variable, VariableKind, VariableUnit,
};
use crate::result::{Diagnostics, OptimizationResult, SolveStatus};
use flexcore_contracts::ids::FlexUuid;
use serde::{Deserialize, Serialize};
use time::macros::offset;

/// A fixed, valid candidate identity for the conic fixtures (nil-adjacent but
/// non-nil; distinct from the P3.17 fixture identity).
pub fn conic_fixture_candidate_id() -> FlexUuid {
    FlexUuid::parse("00000000-0000-0000-0000-0000001c0e1c").expect("fixture candidate id parses")
}

/// Builds the provenance block shared by the conic fixtures (synthetic label
/// on; P3.21 completes the full §12.6 snapshot plumbing).
fn conic_fixture_provenance() -> ProblemProvenance {
    ProblemProvenance {
        candidate_id: conic_fixture_candidate_id(),
        input_snapshot_id: "synthetic-snapshot-p3.18-0001".to_string(),
        data_timestamp: Some(offset!(2026-11-07 00:00:00 UTC)),
        grid_model_version: None,
        objective_config_version: Some("synthetic-objective-v1".to_string()),
        envelope_version: None,
        forecast_version: None,
        data_provenance_label: Some("synthetic".to_string()),
    }
}

/// The name of the shared inverter cone in [`inverter_cone_socp`].
pub const INVERTER_CONE_NAME: &str = "inverter_apparent_power_cone";

/// The shared inverter scenario, Tier 2 side (§11.3): the apparent-power cone
/// `P² + Q² ≤ S_max²` with `S_max = 5` MVA, as the second-order cone
/// `‖(p, q)‖₂ ≤ 5`. The inverter is scheduled to a fixed reactive output
/// `q = 3` MVAr (bounds `[3, 3]`; the reactive-power sign convention is
/// declared per grid model and device profile, never assumed) and the
/// objective maximises active export `p` (MW, `P > 0` = export).
///
/// Hand-computed optimum: `‖(p, 3)‖₂ ≤ 5` with `p ≥ 0` binds at
/// `p = √(25 − 9) = 4`; objective `4.0`.
pub fn inverter_cone_socp() -> OptimizationProblem {
    OptimizationProblem {
        name: "conic-fixture-inverter-cone-socp".to_string(),
        declared_class: ProblemClass::ContinuousSecondOrderConeProgram,
        variables: vec![
            Variable {
                name: "inverter_p_mw".to_string(),
                kind: VariableKind::Continuous,
                unit: VariableUnit::ActivePowerMW,
                lower_bound: Some(0.0),
                upper_bound: Some(10.0),
            },
            Variable {
                name: "inverter_q_mvar".to_string(),
                kind: VariableKind::Continuous,
                unit: VariableUnit::ReactivePowerMVAr,
                lower_bound: Some(3.0),
                upper_bound: Some(3.0),
            },
        ],
        rows: Vec::new(),
        conic_constraints: vec![SecondOrderCone {
            name: INVERTER_CONE_NAME.to_string(),
            norm_members: vec![
                Coefficient {
                    variable: 0,
                    coefficient: 1.0,
                },
                Coefficient {
                    variable: 1,
                    coefficient: 1.0,
                },
            ],
            norm_offset: 0.0,
            rhs_members: Vec::new(),
            rhs_offset: 5.0,
        }],
        objective: Objective {
            sense: ObjectiveSense::Maximize,
            linear_terms: vec![Coefficient {
                variable: 0,
                coefficient: 1.0,
            }],
            quadratic_terms: Vec::new(),
            constant: 0.0,
            declared_units: "synthetic_cost_unit".to_string(),
        },
        declared_assumptions: vec![
            "synthetic reference fixture; single interval; §11.3 apparent-power \
             capability only — not a complete universal inverter model (no mode- \
             or voltage-dependent limits)"
                .to_string(),
            "reactive-power sign convention declared per grid model and device \
             profile, never assumed"
                .to_string(),
        ],
        provenance: conic_fixture_provenance(),
    }
}

/// The hand-computed optimum of [`inverter_cone_socp`]:
/// `p = 4` MW, `q = 3` MVAr, objective `4.0`.
pub fn inverter_cone_socp_optimum() -> ReferenceOptimum {
    ReferenceOptimum {
        objective_value: 4.0,
        column_values: vec![4.0, 3.0],
    }
}

/// The name of the linearised export limit in [`inverter_linearised_lp`].
pub const INVERTER_LINEARISED_P_LIMIT: &str = "inverter_p_linearised_limit";

/// The name of the linearised reactive limit in [`inverter_linearised_lp`].
pub const INVERTER_LINEARISED_Q_LIMIT: &str = "inverter_q_linearised_limit";

/// The shared inverter scenario, Tier 1 side: the SAME operating scenario as
/// [`inverter_cone_socp`] with the apparent-power cone replaced by the Tier 1
/// linearised box approximation `p ≤ S_max`, `q ≤ S_max` (`S_max = 5`).
///
/// Hand-computed optimum: `p = 5` (the linearised limit), objective `5.0` —
/// exactly 1 MW MORE than the Tier 2 cone optimum at the same reactive
/// output, which is the overestimation the P3.23 cross-check exists to
/// expose.
pub fn inverter_linearised_lp() -> OptimizationProblem {
    OptimizationProblem {
        name: "conic-fixture-inverter-linearised-lp".to_string(),
        declared_class: ProblemClass::LinearProgram,
        variables: vec![
            Variable {
                name: "inverter_p_mw".to_string(),
                kind: VariableKind::Continuous,
                unit: VariableUnit::ActivePowerMW,
                lower_bound: Some(0.0),
                upper_bound: Some(10.0),
            },
            Variable {
                name: "inverter_q_mvar".to_string(),
                kind: VariableKind::Continuous,
                unit: VariableUnit::ReactivePowerMVAr,
                lower_bound: Some(3.0),
                upper_bound: Some(3.0),
            },
        ],
        rows: vec![
            LinearRow {
                name: INVERTER_LINEARISED_P_LIMIT.to_string(),
                coefficients: vec![Coefficient {
                    variable: 0,
                    coefficient: 1.0,
                }],
                bounds: RowBounds::Upper(5.0),
            },
            LinearRow {
                name: INVERTER_LINEARISED_Q_LIMIT.to_string(),
                coefficients: vec![Coefficient {
                    variable: 1,
                    coefficient: 1.0,
                }],
                bounds: RowBounds::Upper(5.0),
            },
        ],
        conic_constraints: Vec::new(),
        objective: Objective {
            sense: ObjectiveSense::Maximize,
            linear_terms: vec![Coefficient {
                variable: 0,
                coefficient: 1.0,
            }],
            quadratic_terms: Vec::new(),
            constant: 0.0,
            declared_units: "synthetic_cost_unit".to_string(),
        },
        declared_assumptions: vec![
            "synthetic reference fixture; the cone is linearised to a box for the \
             Tier 1 path — the approximation overestimates deliverable export at \
             non-zero reactive output"
                .to_string(),
        ],
        provenance: conic_fixture_provenance(),
    }
}

/// The hand-computed optimum of [`inverter_linearised_lp`]:
/// `p = 5` MW, `q = 3` MVAr, objective `5.0`.
pub fn inverter_linearised_lp_optimum() -> ReferenceOptimum {
    ReferenceOptimum {
        objective_value: 5.0,
        column_values: vec![5.0, 3.0],
    }
}

/// The name of the DistFlow branch relaxation cone in
/// [`distflow_branch_socp`].
pub const DISTFLOW_CONE_NAME: &str = "branch_relaxation_cone";

/// One DistFlow branch (i→j) in the canonical SOCP relaxation encoding
/// (foundations §7.8): the branch constraint `P_ij² + Q_ij² = v_i·l_ij`
/// relaxed to the second-order cone `‖(2·P_ij, 2·Q_ij, v_i − l_ij)‖₂ ≤ v_i +
/// l_ij`. The voltage, active flow, and reactive flow are pinned to setpoints
/// by equality rows so the optimum is hand-computable; the objective
/// minimises the squared branch current `l_ij`.
///
/// Variables (in order): `v_i` (per-unit squared, setpoint 1.0),
/// `l_ij` (per-unit squared), `P_ij` (MW, setpoint 2.0), `Q_ij` (MVAr,
/// setpoint 0.5). Hand-computed optimum: `l = (P² + Q²)/v = 4.25`, objective
/// `4.25`, cone binding (`g_ij = 1.0·4.25 − 4 − 0.25 = 0`).
///
/// `v_i` and `l_ij` are the SQUARED per-unit voltage and current magnitudes
/// (foundations §7.8); they declare the `PerUnit` unit with the squaring
/// convention recorded in the declared assumptions.
pub fn distflow_branch_socp() -> OptimizationProblem {
    OptimizationProblem {
        name: "conic-fixture-distflow-branch-socp".to_string(),
        declared_class: ProblemClass::ContinuousSecondOrderConeProgram,
        variables: vec![
            Variable {
                name: "branch_v_squared_pu".to_string(),
                kind: VariableKind::Continuous,
                unit: VariableUnit::PerUnit,
                lower_bound: Some(0.81),
                upper_bound: Some(1.21),
            },
            Variable {
                name: "branch_l_squared_pu".to_string(),
                kind: VariableKind::Continuous,
                unit: VariableUnit::PerUnit,
                lower_bound: Some(0.0),
                upper_bound: Some(100.0),
            },
            Variable {
                name: "branch_p_flow_mw".to_string(),
                kind: VariableKind::Continuous,
                unit: VariableUnit::ActivePowerMW,
                lower_bound: Some(-10.0),
                upper_bound: Some(10.0),
            },
            Variable {
                name: "branch_q_flow_mvar".to_string(),
                kind: VariableKind::Continuous,
                unit: VariableUnit::ReactivePowerMVAr,
                lower_bound: Some(-10.0),
                upper_bound: Some(10.0),
            },
        ],
        rows: vec![
            LinearRow {
                name: "v_setpoint".to_string(),
                coefficients: vec![Coefficient {
                    variable: 0,
                    coefficient: 1.0,
                }],
                bounds: RowBounds::Equality(1.0),
            },
            LinearRow {
                name: "p_setpoint".to_string(),
                coefficients: vec![Coefficient {
                    variable: 2,
                    coefficient: 1.0,
                }],
                bounds: RowBounds::Equality(2.0),
            },
            LinearRow {
                name: "q_setpoint".to_string(),
                coefficients: vec![Coefficient {
                    variable: 3,
                    coefficient: 1.0,
                }],
                bounds: RowBounds::Equality(0.5),
            },
        ],
        conic_constraints: vec![SecondOrderCone {
            name: DISTFLOW_CONE_NAME.to_string(),
            norm_members: vec![
                Coefficient {
                    variable: 2,
                    coefficient: 2.0,
                },
                Coefficient {
                    variable: 3,
                    coefficient: 2.0,
                },
                Coefficient {
                    variable: 0,
                    coefficient: 1.0,
                },
                Coefficient {
                    variable: 1,
                    coefficient: -1.0,
                },
            ],
            norm_offset: 0.0,
            rhs_members: vec![
                Coefficient {
                    variable: 0,
                    coefficient: 1.0,
                },
                Coefficient {
                    variable: 1,
                    coefficient: 1.0,
                },
            ],
            rhs_offset: 0.0,
        }],
        objective: Objective {
            sense: ObjectiveSense::Minimize,
            linear_terms: vec![Coefficient {
                variable: 1,
                coefficient: 1.0,
            }],
            quadratic_terms: Vec::new(),
            constant: 0.0,
            declared_units: "synthetic_cost_unit".to_string(),
        },
        declared_assumptions: vec![
            "synthetic reference fixture; one radial branch, foundations §7.8 \
             DistFlow relaxation; v_i and l_ij are SQUARED per-unit quantities \
             (per-unit voltage/current squared on an explicit base recorded \
             elsewhere in the governing grid model)"
                .to_string(),
            "the SOCP relaxation is exact here only because the setpoints make it \
             so; exactness is conditional and must be checked, never assumed \
             (report §12.5)"
                .to_string(),
        ],
        provenance: conic_fixture_provenance(),
    }
}

/// The hand-computed optimum of [`distflow_branch_socp`]:
/// `v = 1.0`, `l = 4.25`, `P = 2.0`, `Q = 0.5`, objective `4.25`.
pub fn distflow_branch_socp_optimum() -> ReferenceOptimum {
    ReferenceOptimum {
        objective_value: 4.25,
        column_values: vec![1.0, 4.25, 2.0, 0.5],
    }
}

/// The branch residual spec for [`distflow_branch_socp`]: variable indices of
/// `v_i`, `l_ij`, `P_ij`, `Q_ij` in the fixture's variable order.
pub fn distflow_branch_specs() -> Vec<BranchResidualSpec> {
    vec![BranchResidualSpec {
        branch: "12".to_string(),
        cone_name: DISTFLOW_CONE_NAME.to_string(),
        send_voltage_squared: 0,
        branch_current_squared: 1,
        send_active_flow: 2,
        send_reactive_flow: 3,
    }]
}

/// One side of a [`Tier1Tier2ComparisonRecord`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TierComparisonSide {
    /// The declared problem class this side solved (per-candidate class is a
    /// required comparison field — the classes MUST differ across the cross
    /// check).
    pub problem_class: ProblemClass,
    /// The §12.6 status of the solve.
    pub status: SolveStatus,
    /// Objective value, when a solution was produced.
    pub objective_value: Option<f64>,
    /// Declared objective units.
    pub objective_units: Option<String>,
    /// Names of constraints binding at the solution (linear rows and, for the
    /// conic side, second-order cones).
    pub binding_constraints: Vec<String>,
}

/// The recorded Tier 1 (linearised) vs Tier 2 (conic) cross-check for one
/// candidate on a shared fixture (ledger P3.18 step 3; consumed by the P3.23
/// comparison workflow — this is the record and its builder, not the
/// workflow). The record compares the per-candidate problem class, the
/// objective, and the binding sets; it does NOT compare feasibility and it
/// never substitutes for the Phase 4 validation gate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tier1Tier2ComparisonRecord {
    /// Candidate identity of the compared solves (from the Tier 2 result; a
    /// mismatch with the Tier 1 result is recorded as a note).
    pub candidate_id: Option<FlexUuid>,
    /// The Tier 1 linearised side.
    pub tier1: TierComparisonSide,
    /// The Tier 2 conic side.
    pub tier2: TierComparisonSide,
    /// `tier1.objective_value − tier2.objective_value` when both are present.
    /// A positive difference on a maximisation means the linearised
    /// approximation overestimates the deliverable objective (as on the
    /// shared inverter fixture); interpret against the declared objective
    /// sense and units.
    pub objective_difference: Option<f64>,
    /// Recorded notes (purpose, caveats, identity mismatches).
    pub notes: Vec<String>,
}

/// Builds the cross-check record from the two solved results on the shared
/// fixture pair. Binding sets are computed at `binding_tolerance` from the
/// results' primal diagnostics; a side without primal diagnostics records an
/// empty binding set plus a note rather than a fabricated one.
pub fn build_comparison_record(
    (tier1_problem, tier1_result): (&OptimizationProblem, &OptimizationResult),
    (tier2_problem, tier2_result): (&OptimizationProblem, &OptimizationResult),
    binding_tolerance: f64,
) -> Result<Tier1Tier2ComparisonRecord, OptimizerError> {
    let mut notes = vec![
        "P3.18 cross-check: the Tier 1 linearised approximation and the Tier 2 \
         conic formulation describe the SAME operating scenario; the objective \
         difference quantifies the linearisation gap, and the binding sets show \
         which constraint each tier drives to its limit"
            .to_string(),
        "the comparison is diagnostic; neither side is a feasibility verdict — \
         both results are candidates for the Phase 4 validation gate (report \
         §12.5)"
            .to_string(),
    ];

    let tier1_binding = binding_constraints(tier1_problem, tier1_result, binding_tolerance)?;
    let tier2_binding = binding_constraints(tier2_problem, tier2_result, binding_tolerance)?;

    if tier1_binding.is_empty() {
        notes.push("tier 1 side has no recorded binding constraints (no primal \
                    diagnostics or none within tolerance)"
            .to_string());
    }
    if tier2_binding.is_empty() {
        notes.push("tier 2 side has no recorded binding constraints (no primal \
                    diagnostics or none within tolerance)"
            .to_string());
    }

    let candidate_id = tier2_result.candidate_id;
    if tier1_result.candidate_id != candidate_id {
        notes.push(format!(
            "candidate identity mismatch: tier 1 = {:?}, tier 2 = {:?}",
            tier1_result.candidate_id, candidate_id
        ));
    }

    let objective_difference = match (
        tier1_result.objective_value,
        tier2_result.objective_value,
    ) {
        (Some(a), Some(b)) => Some(a - b),
        _ => None,
    };

    Ok(Tier1Tier2ComparisonRecord {
        candidate_id,
        tier1: TierComparisonSide {
            problem_class: tier1_problem.declared_class,
            status: tier1_result.status,
            objective_value: tier1_result.objective_value,
            objective_units: tier1_result.objective_units.clone(),
            binding_constraints: tier1_binding,
        },
        tier2: TierComparisonSide {
            problem_class: tier2_problem.declared_class,
            status: tier2_result.status,
            objective_value: tier2_result.objective_value,
            objective_units: tier2_result.objective_units.clone(),
            binding_constraints: tier2_binding,
        },
        objective_difference,
        notes,
    })
}

/// Binding constraint names for one side: linear rows within tolerance of a
/// bound, plus (for the conic side) cones with slack within tolerance. A
/// result without primal diagnostics yields an empty set (recorded as a note
/// by [`build_comparison_record`], never fabricated).
fn binding_constraints(
    problem: &OptimizationProblem,
    result: &OptimizationResult,
    tolerance: f64,
) -> Result<Vec<String>, OptimizerError> {
    let primal = match &result.primal {
        Diagnostics::Available(primal) => primal
            .values
            .iter()
            .map(|value| value.value)
            .collect::<Vec<f64>>(),
        Diagnostics::NotAvailable { .. } => return Ok(Vec::new()),
    };
    let mut binding = binding_linear_rows(problem, &primal, tolerance)?;
    binding.extend(binding_cones(problem, &primal, tolerance)?.into_iter());
    Ok(binding)
}

/// The foundations §7.8 residual of the shared DistFlow fixture at an
/// arbitrary primal point (variable order: `v`, `l`, `P`, `Q`). Exposed for
/// the P3.23 comparison workflow and the offline tests.
pub fn distflow_fixture_residual(v: f64, l: f64, p: f64, q: f64) -> f64 {
    distflow_branch_residual(v, l, p, q)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clarabel_provider::cone_slack;

    #[test]
    fn every_conic_fixture_declares_its_class_and_validates() {
        assert_eq!(
            inverter_cone_socp().declared_class,
            ProblemClass::ContinuousSecondOrderConeProgram
        );
        assert_eq!(
            inverter_linearised_lp().declared_class,
            ProblemClass::LinearProgram
        );
        assert_eq!(
            distflow_branch_socp().declared_class,
            ProblemClass::ContinuousSecondOrderConeProgram
        );
        inverter_cone_socp().validate().expect("socp validates");
        inverter_linearised_lp().validate().expect("lp validates");
        distflow_branch_socp().validate().expect("socp validates");
    }

    #[test]
    fn fixtures_carry_the_synthetic_label_and_snapshot() {
        for problem in [
            inverter_cone_socp(),
            inverter_linearised_lp(),
            distflow_branch_socp(),
        ] {
            assert_eq!(
                problem.provenance.data_provenance_label.as_deref(),
                Some("synthetic"),
                "fixture {} must be labelled synthetic (D1/D2 OPEN)",
                problem.name
            );
            assert!(!problem.provenance.input_snapshot_id.is_empty());
            assert!(problem.provenance.candidate_id != FlexUuid::NIL);
        }
    }

    #[test]
    fn hand_optima_align_with_formulation_order() {
        assert_eq!(
            inverter_cone_socp_optimum().column_values.len(),
            inverter_cone_socp().variables.len()
        );
        assert_eq!(
            inverter_linearised_lp_optimum().column_values.len(),
            inverter_linearised_lp().variables.len()
        );
        assert_eq!(
            distflow_branch_socp_optimum().column_values.len(),
            distflow_branch_socp().variables.len()
        );
    }

    #[test]
    fn cones_are_binding_at_the_hand_computed_optima() {
        let socp = inverter_cone_socp();
        let slack = cone_slack(
            &socp.conic_constraints[0],
            &inverter_cone_socp_optimum().column_values,
        )
        .expect("primal aligned");
        assert!(
            slack.abs() <= 1e-12,
            "the inverter cone must bind at p = 4, q = 3 (slack = {slack})"
        );

        let distflow = distflow_branch_socp();
        let slack = cone_slack(
            &distflow.conic_constraints[0],
            &distflow_branch_socp_optimum().column_values,
        )
        .expect("primal aligned");
        assert!(
            slack.abs() <= 1e-12,
            "the branch cone must bind at the hand-computed optimum (slack = {slack})"
        );
    }

    #[test]
    fn the_linearised_side_overestimates_export_by_exactly_one_megawatt() {
        // The whole point of the shared fixture pair: at q = 3 MVAr the box
        // linearisation allows 5 MW where the cone allows 4 MW.
        let difference = inverter_linearised_lp_optimum().objective_value
            - inverter_cone_socp_optimum().objective_value;
        assert!((difference - 1.0).abs() <= 1e-12);
    }

    #[test]
    fn branch_residual_at_the_optimum_is_zero_and_specs_are_in_range() {
        let optimum = distflow_branch_socp_optimum().column_values;
        let residual = distflow_fixture_residual(optimum[0], optimum[1], optimum[2], optimum[3]);
        assert!(residual.abs() <= 1e-12, "g_ij must be 0 at the optimum");

        let count = distflow_branch_socp().variables.len();
        for spec in distflow_branch_specs() {
            assert!(spec.send_voltage_squared < count);
            assert!(spec.branch_current_squared < count);
            assert!(spec.send_active_flow < count);
            assert!(spec.send_reactive_flow < count);
        }
    }

    #[test]
    fn branch_residual_hand_computed_values() {
        // Loose point: g = 1.0·4.5 − 4 − 0.25 = 0.25.
        assert!((distflow_fixture_residual(1.0, 4.5, 2.0, 0.5) - 0.25).abs() <= 1e-12);
        // Violated point: g = 1.0·4.0 − 4 − 0.25 = −0.25.
        assert!((distflow_fixture_residual(1.0, 4.0, 2.0, 0.5) - (-0.25)).abs() <= 1e-12);
    }
}
