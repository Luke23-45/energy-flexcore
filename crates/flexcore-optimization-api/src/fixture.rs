// P3.17 — In-crate reference fixtures for the `Optimizer` port.
//
// These are **synthetic reference formulations** (labelled as such — D1/D2
// are OPEN, so nothing here is market data; P3.14/P3.16 discipline). They
// give the port tests a stable problem to solve through the provider path:
// every fixture's optimum is hand-computable and recorded in
// [`ReferenceOptimum`], so the replay-skeleton tests (tests/replay_skeleton.rs)
// can assert end-to-end behaviour without a built solver; the real-solver
// replay tests land with P3.19/P3.26 (decision note §6 item 6).
//
// Fixtures are reference material, not the P3.26 scenario suite.

use crate::problem::{
    Coefficient, LinearRow, Objective, ObjectiveSense, OptimizationProblem, ProblemClass,
    ProblemProvenance, QuadraticTerm, RowBounds, SecondOrderCone, Variable, VariableKind,
    VariableUnit,
};
use flexcore_contracts::ids::FlexUuid;
use time::macros::offset;

/// A fixed, valid candidate identity for the reference fixtures
/// (`00000000-0000-0000-0000-000000000c4d1d`, nil-adjacent but non-nil).
fn fixture_candidate_id() -> FlexUuid {
    FlexUuid::parse("00000000-0000-0000-0000-0000000c4d1d")
        .expect("fixture candidate id parses")
}

/// The hand-computed optimum of a reference fixture, used by the
/// replay-skeleton tests and by the scripted backend outcomes.
#[derive(Debug, Clone, PartialEq)]
pub struct ReferenceOptimum {
    /// Expected objective value.
    pub objective_value: f64,
    /// Expected primal values per variable, in formulation order.
    pub column_values: Vec<f64>,
}

/// Builds the provenance block shared by the fixtures (synthetic label on;
/// P3.21 completes the full §12.6 snapshot plumbing later).
fn fixture_provenance() -> ProblemProvenance {
    ProblemProvenance {
        candidate_id: fixture_candidate_id(),
        input_snapshot_id: "synthetic-snapshot-p3.17-0001".to_string(),
        data_timestamp: Some(offset!(2026-11-07 00:00:00 UTC)),
        grid_model_version: None,
        objective_config_version: Some("synthetic-objective-v1".to_string()),
        envelope_version: None,
        forecast_version: None,
        data_provenance_label: Some("synthetic".to_string()),
    }
}

/// Reference LP (Tier 1): two storage discharges meet a shared portfolio
/// export floor over one interval; the cheaper unit is dispatched first.
///
/// Formulation (MW, `P > 0` = export):
/// ```text
/// min  2·bess_a_discharge + 3·bess_b_discharge
/// s.t. bess_a_discharge + bess_b_discharge >= 4   (portfolio export floor)
///      0 <= bess_a_discharge <= 5
///      0 <= bess_b_discharge <= 3
/// ```
/// Optimum: `bess_a_discharge = 4`, `bess_b_discharge = 0`, objective `8.0`.
pub fn reference_lp() -> OptimizationProblem {
    OptimizationProblem {
        name: "reference-portfolio-dispatch-lp".to_string(),
        declared_class: ProblemClass::LinearProgram,
        variables: vec![
            Variable {
                name: "bess_a_discharge".to_string(),
                kind: VariableKind::Continuous,
                unit: VariableUnit::ActivePowerMW,
                lower_bound: Some(0.0),
                upper_bound: Some(5.0),
            },
            Variable {
                name: "bess_b_discharge".to_string(),
                kind: VariableKind::Continuous,
                unit: VariableUnit::ActivePowerMW,
                lower_bound: Some(0.0),
                upper_bound: Some(3.0),
            },
        ],
        rows: vec![LinearRow {
            name: "portfolio_export_floor".to_string(),
            coefficients: vec![
                Coefficient {
                    variable: 0,
                    coefficient: 1.0,
                },
                Coefficient {
                    variable: 1,
                    coefficient: 1.0,
                },
            ],
            bounds: RowBounds::Lower(4.0),
        }],
        conic_constraints: Vec::new(),
        objective: Objective {
            sense: ObjectiveSense::Minimize,
            linear_terms: vec![
                Coefficient {
                    variable: 0,
                    coefficient: 2.0,
                },
                Coefficient {
                    variable: 1,
                    coefficient: 3.0,
                },
            ],
            quadratic_terms: Vec::new(),
            constant: 0.0,
            declared_units: "synthetic_cost_unit".to_string(),
        },
        declared_assumptions: vec![
            "synthetic reference fixture; single interval, no energy coupling"
                .to_string(),
            "linearised discharge only (P3.17 Tier 1 LP path)".to_string(),
        ],
        provenance: fixture_provenance(),
    }
}

/// The hand-computed optimum of [`reference_lp`].
pub fn reference_lp_optimum() -> ReferenceOptimum {
    ReferenceOptimum {
        objective_value: 8.0,
        column_values: vec![4.0, 0.0],
    }
}

/// Reference QP (Tier 1): the §12.2 PCC tracking term as a convex quadratic —
/// `min 0.5·(p − 3)²` over a portfolio export limit `p ∈ [0, 2] MW`.
///
/// Expanded into the IR (upper-triangle convention `c·xᵢ·xⱼ`):
/// `min 0.5·p² − 3·p + 4.5`. Optimum at the bound: `p = 2`, objective `0.5`.
pub fn reference_qp() -> OptimizationProblem {
    OptimizationProblem {
        name: "reference-pcc-tracking-qp".to_string(),
        declared_class: ProblemClass::ConvexQuadraticProgram,
        variables: vec![Variable {
            name: "tracked_export_mw".to_string(),
            kind: VariableKind::Continuous,
            unit: VariableUnit::ActivePowerMW,
            lower_bound: Some(0.0),
            upper_bound: Some(2.0),
        }],
        rows: Vec::new(),
        conic_constraints: Vec::new(),
        objective: Objective {
            sense: ObjectiveSense::Minimize,
            linear_terms: vec![Coefficient {
                variable: 0,
                coefficient: -3.0,
            }],
            quadratic_terms: vec![QuadraticTerm {
                row: 0,
                column: 0,
                coefficient: 0.5,
            }],
            constant: 4.5,
            declared_units: "synthetic_cost_unit".to_string(),
        },
        declared_assumptions: vec![
            "synthetic reference fixture; §12.2-style tracking term against a \
             synthetic commitment"
                .to_string(),
        ],
        provenance: fixture_provenance(),
    }
}

/// The hand-computed optimum of [`reference_qp`].
pub fn reference_qp_optimum() -> ReferenceOptimum {
    ReferenceOptimum {
        objective_value: 0.5,
        column_values: vec![2.0],
    }
}

/// Reference MILP (Tier 1): a discharge with binary capacity-mode selection.
///
/// Formulation:
/// ```text
/// min  1·discharge_mw + 3·mode_high_capacity + 1·mode_low_capacity
/// s.t. discharge_mw - 5·mode_high_capacity - 2·mode_low_capacity <= 0
///      discharge_mw >= 4
///      0 <= discharge_mw <= 5 (MW), mode binaries in [0, 1]
/// ```
/// The demand floor forces `mode_high_capacity = 1` (the low mode alone can
/// only reach 2 MW); optimum: `discharge_mw = 4`, `mode_high = 1`,
/// `mode_low = 0`, objective `7.0`.
pub fn reference_milp() -> OptimizationProblem {
    OptimizationProblem {
        name: "reference-mode-selection-milp".to_string(),
        declared_class: ProblemClass::MixedIntegerLinearProgram,
        variables: vec![
            Variable {
                name: "discharge_mw".to_string(),
                kind: VariableKind::Continuous,
                unit: VariableUnit::ActivePowerMW,
                lower_bound: Some(0.0),
                upper_bound: Some(5.0),
            },
            Variable {
                name: "mode_high_capacity".to_string(),
                kind: VariableKind::Binary,
                unit: VariableUnit::Dimensionless,
                lower_bound: Some(0.0),
                upper_bound: Some(1.0),
            },
            Variable {
                name: "mode_low_capacity".to_string(),
                kind: VariableKind::Binary,
                unit: VariableUnit::Dimensionless,
                lower_bound: Some(0.0),
                upper_bound: Some(1.0),
            },
        ],
        rows: vec![
            LinearRow {
                name: "capacity_envelope".to_string(),
                coefficients: vec![
                    Coefficient {
                        variable: 0,
                        coefficient: 1.0,
                    },
                    Coefficient {
                        variable: 1,
                        coefficient: -5.0,
                    },
                    Coefficient {
                        variable: 2,
                        coefficient: -2.0,
                    },
                ],
                bounds: RowBounds::Upper(0.0),
            },
            LinearRow {
                name: "demand_floor".to_string(),
                coefficients: vec![Coefficient {
                    variable: 0,
                    coefficient: 1.0,
                }],
                bounds: RowBounds::Lower(4.0),
            },
        ],
        conic_constraints: Vec::new(),
        objective: Objective {
            sense: ObjectiveSense::Minimize,
            linear_terms: vec![
                Coefficient {
                    variable: 0,
                    coefficient: 1.0,
                },
                Coefficient {
                    variable: 1,
                    coefficient: 3.0,
                },
                Coefficient {
                    variable: 2,
                    coefficient: 1.0,
                },
            ],
            quadratic_terms: Vec::new(),
            constant: 0.0,
            declared_units: "synthetic_cost_unit".to_string(),
        },
        declared_assumptions: vec![
            "synthetic reference fixture; binary mode variables are the Tier 1 \
             MILP path for discrete decisions (P3.17 step 3)"
                .to_string(),
        ],
        provenance: fixture_provenance(),
    }
}

/// The hand-computed optimum of [`reference_milp`].
pub fn reference_milp_optimum() -> ReferenceOptimum {
    ReferenceOptimum {
        objective_value: 7.0,
        column_values: vec![4.0, 1.0, 0.0],
    }
}

/// Reference declared-SOCP formulation (Tier 2 IR): the §11.3 inverter
/// apparent-power cone `‖(p, q)‖₂ ≤ s` with `s ∈ [0, 10]` MVA.
///
/// This problem is **valid for the port** — the Tier 2 provider (P3.18) is
/// the one meant to solve it — and is used here to prove the Tier 1 provider
/// rejects it with `unsupported` without any solver invocation. Note the
/// reactive-power sign is not interpreted here; sign convention is declared
/// per grid model and device profile (§10.2), never assumed.
pub fn reference_socp_declaration() -> OptimizationProblem {
    OptimizationProblem {
        name: "reference-inverter-cone-socp".to_string(),
        declared_class: ProblemClass::ContinuousSecondOrderConeProgram,
        variables: vec![
            Variable {
                name: "inverter_p_mw".to_string(),
                kind: VariableKind::Continuous,
                unit: VariableUnit::ActivePowerMW,
                lower_bound: Some(-5.0),
                upper_bound: Some(5.0),
            },
            Variable {
                name: "inverter_q_mvar".to_string(),
                kind: VariableKind::Continuous,
                unit: VariableUnit::ReactivePowerMVAr,
                lower_bound: Some(-5.0),
                upper_bound: Some(5.0),
            },
            Variable {
                name: "inverter_s_mva".to_string(),
                kind: VariableKind::Continuous,
                unit: VariableUnit::ApparentPowerMVA,
                lower_bound: Some(0.0),
                upper_bound: Some(10.0),
            },
        ],
        rows: Vec::new(),
        conic_constraints: vec![SecondOrderCone {
            name: "inverter_apparent_power_cone".to_string(),
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
            rhs_members: vec![Coefficient {
                variable: 2,
                coefficient: 1.0,
            }],
            rhs_offset: 0.0,
        }],
        objective: Objective {
            sense: ObjectiveSense::Minimize,
            linear_terms: vec![Coefficient {
                variable: 2,
                coefficient: 1.0,
            }],
            quadratic_terms: Vec::new(),
            constant: 0.0,
            declared_units: "synthetic_cost_unit".to_string(),
        },
        declared_assumptions: vec![
            "synthetic reference fixture; the reactive-power sign convention is \
             declared per grid model and device profile, never assumed"
                .to_string(),
        ],
        provenance: fixture_provenance(),
    }
}

/// Reference declared-MISOCP formulation (Tier 3 — not built; P3.20): the
/// inverter cone plus one binary switching variable. Used only to prove the
/// Tier 1 provider rejects the declaration with `unsupported`.
pub fn reference_misocp_declaration() -> OptimizationProblem {
    let mut problem = reference_socp_declaration();
    problem.name = "reference-inverter-cone-misocp".to_string();
    problem.declared_class = ProblemClass::MixedIntegerSecondOrderConeProgram;
    problem.variables.push(Variable {
        name: "tap_switch_binary".to_string(),
        kind: VariableKind::Binary,
        unit: VariableUnit::Dimensionless,
        lower_bound: Some(0.0),
        upper_bound: Some(1.0),
    });
    problem
}

/// Reference declared-nonlinear formulation (Tier 4 boundary — not built;
/// P3.20). The IR stores no nonlinear term structure, so the body is the
/// linear LP; the declaration alone is what the Tier 1 provider must reject
/// with `unsupported`.
pub fn reference_nonlinear_declaration() -> OptimizationProblem {
    let mut problem = reference_lp();
    problem.name = "reference-nonlinear-declaration".to_string();
    problem.declared_class = ProblemClass::NonlinearProgram;
    problem
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_fixture_declares_its_class_and_validates() {
        assert_eq!(reference_lp().declared_class, ProblemClass::LinearProgram);
        assert_eq!(
            reference_qp().declared_class,
            ProblemClass::ConvexQuadraticProgram
        );
        assert_eq!(
            reference_milp().declared_class,
            ProblemClass::MixedIntegerLinearProgram
        );
        assert_eq!(
            reference_socp_declaration().declared_class,
            ProblemClass::ContinuousSecondOrderConeProgram
        );
        assert_eq!(
            reference_misocp_declaration().declared_class,
            ProblemClass::MixedIntegerSecondOrderConeProgram
        );
        assert_eq!(
            reference_nonlinear_declaration().declared_class,
            ProblemClass::NonlinearProgram
        );

        reference_lp().validate().expect("lp validates");
        reference_qp().validate().expect("qp validates");
        reference_milp().validate().expect("milp validates");
        reference_socp_declaration()
            .validate()
            .expect("socp validates");
        reference_misocp_declaration()
            .validate()
            .expect("misocp validates");
        reference_nonlinear_declaration()
            .validate()
            .expect("nonlinear validates");
    }

    #[test]
    fn fixtures_carry_the_synthetic_label_and_snapshot() {
        for problem in [
            reference_lp(),
            reference_qp(),
            reference_milp(),
            reference_socp_declaration(),
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
    fn fixture_optima_match_formulation_order() {
        assert_eq!(
            reference_lp_optimum().column_values.len(),
            reference_lp().variables.len()
        );
        assert_eq!(
            reference_qp_optimum().column_values.len(),
            reference_qp().variables.len()
        );
        assert_eq!(
            reference_milp_optimum().column_values.len(),
            reference_milp().variables.len()
        );
    }
}
