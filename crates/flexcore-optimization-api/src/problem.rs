// P3.17 — Solver-neutral optimization problem IR (report §9.1, §12.3, §12.6).
//
// Every formulation handed to the [`Optimizer`](crate::port::Optimizer) port
// declares its problem class **per run**. The class declaration is the
// solver-class-discipline enforcement point (AGENTS.md rule 3.4 — absolute):
// a formulation declaring LP/QP/MILP is the only thing the Tier 1 provider
// will ever assemble, and a formulation declaring a conic class can never be
// routed to a solver that does not support that class — the provider rejects
// it with the §12.6 status `unsupported` before any solver invocation.
//
// Canonical units (report §10.2): MW, MVAr, MVA, MWh; `P > 0` means export at
// the point of connection; SOC is a fraction in [0,1] at the API boundary.
// Variables declare their unit explicitly; nothing is assumed.
//
// This module contains **no solver-specific type**. The dependency-direction
// test (`tests/dependency_direction.rs`) enforces that the port-surface
// modules, this one included, never name a concrete solver or binding.

use flexcore_contracts::ids::FlexUuid;
use serde::{Deserialize, Serialize};

/// The declared optimization problem class (report §12.3 tiers).
///
/// Tiers are problem classes, not quality rankings. Tier 1 = LP / convex QP /
/// MILP; Tier 2 = continuous SOCP; Tier 3 = MISOCP/MINLP; Tier 4 = nonlinear
/// AC-OPF (not built in this phase — see P3.20). Solver capabilities must be
/// checked against the actual declared class; a provider that cannot solve a
/// class returns `unsupported`, it never approximates silently.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProblemClass {
    /// Linear program (Tier 1).
    LinearProgram,
    /// Convex quadratic program (Tier 1; e.g. the §12.2 PCC tracking term).
    ConvexQuadraticProgram,
    /// Mixed-integer linear program (Tier 1; e.g. BESS mode binaries).
    MixedIntegerLinearProgram,
    /// Continuous second-order cone program (Tier 2 — not solvable by the
    /// Tier 1 provider; it rejects such declarations with `unsupported`).
    ContinuousSecondOrderConeProgram,
    /// Mixed-integer second-order cone program (Tier 3 — not built; see P3.20).
    MixedIntegerSecondOrderConeProgram,
    /// Nonlinear program (Tier 4 boundary — not built; see P3.20). This IR
    /// stores no nonlinear term structure; a `NonlinearProgram` declaration
    /// passes structural validation and every provider that cannot solve
    /// nonlinear problems must reject it with `unsupported`.
    NonlinearProgram,
}

impl ProblemClass {
    /// Short stable identifier used in records and diagnostics.
    pub fn as_str(self) -> &'static str {
        match self {
            ProblemClass::LinearProgram => "lp",
            ProblemClass::ConvexQuadraticProgram => "convex_qp",
            ProblemClass::MixedIntegerLinearProgram => "milp",
            ProblemClass::ContinuousSecondOrderConeProgram => "socp",
            ProblemClass::MixedIntegerSecondOrderConeProgram => "misocp",
            ProblemClass::NonlinearProgram => "nonlinear",
        }
    }

    /// The report §12.3 tier of this class: 1, 2, 3, or 4.
    pub fn tier(self) -> u8 {
        match self {
            ProblemClass::LinearProgram
            | ProblemClass::ConvexQuadraticProgram
            | ProblemClass::MixedIntegerLinearProgram => 1,
            ProblemClass::ContinuousSecondOrderConeProgram => 2,
            ProblemClass::MixedIntegerSecondOrderConeProgram => 3,
            ProblemClass::NonlinearProgram => 4,
        }
    }
}

/// Variable kind: continuous, general integer, or binary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VariableKind {
    /// Continuous variable.
    Continuous,
    /// General integer variable.
    Integer,
    /// Binary variable (bounds implied [0, 1]; explicit bounds are still
    /// validated and must be within [0, 1] if given).
    Binary,
}

/// The declared physical unit of a variable (report §10.2 canonical units).
///
/// Per-unit and fraction values are solver-representation concerns; at the
/// API boundary the canonical units are MW, MVAr, MVA, MWh and SOC fractions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VariableUnit {
    /// Active power P in MW; `P > 0` = export at the point of connection.
    ActivePowerMW,
    /// Reactive power Q in MVAr; sign declared per grid model and device
    /// profile, never assumed.
    ReactivePowerMVAr,
    /// Apparent power in MVA.
    ApparentPowerMVA,
    /// Energy in MWh.
    EnergyMWh,
    /// Voltage/current in per-unit on an explicit base (base recorded
    /// elsewhere in the governing grid model, not per-variable).
    PerUnit,
    /// State of charge as a fraction in [0, 1] at the API boundary.
    StateOfChargeFraction,
    /// Dimensionless quantity (e.g. a mode or commitment indicator).
    Dimensionless,
}

impl VariableUnit {
    /// Canonical unit string for records and diagnostics.
    pub fn canonical_unit(self) -> &'static str {
        match self {
            VariableUnit::ActivePowerMW => "MW",
            VariableUnit::ReactivePowerMVAr => "MVAr",
            VariableUnit::ApparentPowerMVA => "MVA",
            VariableUnit::EnergyMWh => "MWh",
            VariableUnit::PerUnit => "pu",
            VariableUnit::StateOfChargeFraction => "fraction",
            VariableUnit::Dimensionless => "dimensionless",
        }
    }
}

/// One decision variable of the formulation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Variable {
    /// Unique name within the problem (validated).
    pub name: String,
    /// Continuity / integrality of the variable.
    pub kind: VariableKind,
    /// Declared physical unit (canonical units only — report §10.2).
    pub unit: VariableUnit,
    /// Lower bound; `None` = unbounded below. Must be finite when `Some`
    /// (use `None`, not an infinity sentinel, for unbounded).
    pub lower_bound: Option<f64>,
    /// Upper bound; `None` = unbounded above. Must be finite when `Some`.
    pub upper_bound: Option<f64>,
}

/// A (variable index, coefficient) pair. The index refers to the position in
/// [`OptimizationProblem::variables`]; validated to be in range.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Coefficient {
    /// Variable index into `variables`.
    pub variable: usize,
    /// Coefficient value; must be finite (NaN/infinity rejected).
    pub coefficient: f64,
}

/// Bounds of one linear constraint row.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RowBounds {
    /// `row >= value`.
    Lower(f64),
    /// `row <= value`.
    Upper(f64),
    /// `row == value`.
    Equality(f64),
    /// `lower <= row <= upper`.
    Range { lower: f64, upper: f64 },
}

impl RowBounds {
    /// Returns `(lower, upper)` with `f64::INFINITY` / `f64::NEG_INFINITY`
    /// standing for unbounded sides. The worker adapter maps infinities to
    /// the pinned solver's infinity constant at build time (VERIFY-BUILD,
    /// decision note §6 item 4).
    pub fn as_lower_upper(self) -> (f64, f64) {
        match self {
            RowBounds::Lower(v) => (v, f64::INFINITY),
            RowBounds::Upper(v) => (f64::NEG_INFINITY, v),
            RowBounds::Equality(v) => (v, v),
            RowBounds::Range { lower, upper } => (lower, upper),
        }
    }
}

/// One linear constraint row `Σ aᵢ·xᵢ` subject to its declared bounds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LinearRow {
    /// Unique name within the problem (validated).
    pub name: String,
    /// Non-zero-sparse coefficients of the row.
    pub coefficients: Vec<Coefficient>,
    /// Row bounds.
    pub bounds: RowBounds,
}

/// One quadratic objective term contributing `coefficient · x_row · x_column`.
///
/// Stored as an upper-triangle term list (`row <= column`, validated); each
/// (row, column) pair appears at most once. The worker adapter translates
/// this into the pinned binding's Hessian representation — VERIFY-BUILD the
/// off-diagonal scaling convention at build time (decision note §6 item 3).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct QuadraticTerm {
    /// First variable index (<= `column`).
    pub row: usize,
    /// Second variable index (>= `row`).
    pub column: usize,
    /// Coefficient of the `x_row · x_column` product; must be finite.
    pub coefficient: f64,
}

/// Objective sense.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObjectiveSense {
    /// Minimize the objective.
    Minimize,
    /// Maximize the objective.
    Maximize,
}

/// The objective: `sense (constant + Σ linear + Σ quadratic)` in the declared
/// units. Objective units are declared, not inferred (P3.14 records objective
/// configuration per schedule; while D1/D2 are OPEN the units string names
/// the synthetic cost unit explicitly).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Objective {
    /// Minimize or maximize.
    pub sense: ObjectiveSense,
    /// Linear terms.
    pub linear_terms: Vec<Coefficient>,
    /// Quadratic terms (upper triangle; empty for an LP or MILP).
    pub quadratic_terms: Vec<QuadraticTerm>,
    /// Constant offset; must be finite.
    pub constant: f64,
    /// Declared units of the objective value (e.g. `"synthetic_cost_unit"`
    /// while D1/D2 are OPEN). Non-empty (validated).
    pub declared_units: String,
}

/// One second-order cone constraint `‖diag(a)·x + b‖₂ ≤ cᵀx + d`.
///
/// This is the Tier 2 IR element (P3.18). It expresses the diagonal-norm form,
/// which covers the §11.3 inverter apparent-power cone `P² + Q² ≤ S²` as
/// `‖(P, Q)‖₂ ≤ S`. The Tier 1 path never consumes this type: a formulation
/// carrying cones must declare a conic class and is rejected by the Tier 1
/// provider with `unsupported`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SecondOrderCone {
    /// Unique name within the problem (validated).
    pub name: String,
    /// Norm-side members `(variable index, coefficient aᵢ)`.
    pub norm_members: Vec<Coefficient>,
    /// Norm-side offset `b`.
    pub norm_offset: f64,
    /// Right-hand-side linear members `(variable index, coefficient cᵢ)`.
    pub rhs_members: Vec<Coefficient>,
    /// Right-hand-side offset `d`.
    pub rhs_offset: f64,
}

/// Provenance carried with the formulation (report §9.2, §12.6; completed by
/// P3.21 wiring). Every field here is copied into the result record so a
/// result is auditable from the record alone.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProblemProvenance {
    /// Candidate identity of the output this solve will produce (§12.6
    /// "output candidate ID"). Assigned by the caller before the solve; the
    /// typed schedule-identity model is P3.22's.
    pub candidate_id: FlexUuid,
    /// Input snapshot identifier (required; §12.6 "input snapshot").
    pub input_snapshot_id: String,
    /// Data timestamp of the input snapshot, when known.
    pub data_timestamp: Option<time::OffsetDateTime>,
    /// Grid-model version traceable to the Phase 2 Site grid-model reference
    /// (P2.02) when known; required by P3.21 completeness wiring, optional at
    /// the port level.
    pub grid_model_version: Option<String>,
    /// Objective configuration version (P3.14) when known.
    pub objective_config_version: Option<String>,
    /// Envelope version (P3.13) when known.
    pub envelope_version: Option<String>,
    /// Forecast version when known.
    pub forecast_version: Option<String>,
    /// Data-provenance label, e.g. `"synthetic"` — required while D1/D2 are
    /// OPEN so no market product is implied (P3.14/P3.16 discipline).
    pub data_provenance_label: Option<String>,
}

/// A complete, solver-neutral optimization formulation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OptimizationProblem {
    /// Human-readable problem name (non-empty; validated).
    pub name: String,
    /// The declared problem class — the enforcement point for solver-class
    /// discipline. Must be consistent with the formulation body (validated).
    pub declared_class: ProblemClass,
    /// Decision variables (non-empty; unique names; validated).
    pub variables: Vec<Variable>,
    /// Linear constraint rows (unique names; validated).
    pub rows: Vec<LinearRow>,
    /// Second-order cone constraints (Tier 2 IR; must be empty for LP/QP/MILP
    /// declarations and non-empty for conic declarations; validated).
    pub conic_constraints: Vec<SecondOrderCone>,
    /// Objective.
    pub objective: Objective,
    /// Assumptions and uncertainty declared with the formulation (§9.2
    /// "declared problem class and assumptions"; copied into the result).
    pub declared_assumptions: Vec<String>,
    /// Provenance copied into the result record.
    pub provenance: ProblemProvenance,
}

/// Structural / class-consistency errors of a formulation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProblemError {
    /// Problem name is empty.
    EmptyName,
    /// A formulation must declare at least one variable.
    NoVariables,
    /// A variable name occurs twice.
    DuplicateVariableName(String),
    /// A row name occurs twice.
    DuplicateRowName(String),
    /// A cone name occurs twice.
    DuplicateConeName(String),
    /// A variable index is out of range.
    VariableIndexOutOfRange { index: usize, variable_count: usize },
    /// A value that must be finite is NaN or infinite.
    NonFiniteValue { context: &'static str },
    /// `lower > upper` on a variable or row.
    InvertedBounds { context: String },
    /// A binary variable carries explicit bounds outside [0, 1].
    BinaryBoundsOutsideUnitInterval(String),
    /// A quadratic term is stored off the upper triangle.
    QuadraticTermNotUpperTriangle { row: usize, column: usize },
    /// A (row, column) quadratic pair occurs more than once.
    DuplicateQuadraticTerm { row: usize, column: usize },
    /// Objective units are undeclared.
    EmptyObjectiveUnits,
    /// The input snapshot reference is undeclared (§12.6 requires it on every
    /// result; a formulation cannot be solved without one).
    EmptyInputSnapshotId,
    /// The declared class does not match the formulation body.
    ClassMismatch { declared: ProblemClass, reason: String },
}

impl std::fmt::Display for ProblemError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProblemError::EmptyName => write!(f, "problem name is empty"),
            ProblemError::NoVariables => write!(f, "problem declares no variables"),
            ProblemError::DuplicateVariableName(n) => {
                write!(f, "duplicate variable name: {n}")
            }
            ProblemError::DuplicateRowName(n) => write!(f, "duplicate row name: {n}"),
            ProblemError::DuplicateConeName(n) => write!(f, "duplicate cone name: {n}"),
            ProblemError::VariableIndexOutOfRange { index, variable_count } => write!(
                f,
                "variable index {index} out of range (variable_count = {variable_count})"
            ),
            ProblemError::NonFiniteValue { context } => {
                write!(f, "non-finite value in {context}")
            }
            ProblemError::InvertedBounds { context } => {
                write!(f, "inverted bounds (lower > upper) in {context}")
            }
            ProblemError::BinaryBoundsOutsideUnitInterval(n) => write!(
                f,
                "binary variable '{n}' carries explicit bounds outside [0, 1]"
            ),
            ProblemError::QuadraticTermNotUpperTriangle { row, column } => write!(
                f,
                "quadratic term ({row}, {column}) is not stored upper-triangle"
            ),
            ProblemError::DuplicateQuadraticTerm { row, column } => write!(
                f,
                "duplicate quadratic term for pair ({row}, {column})"
            ),
            ProblemError::EmptyObjectiveUnits => {
                write!(f, "objective units are not declared")
            }
            ProblemError::EmptyInputSnapshotId => {
                write!(f, "input snapshot id is not declared")
            }
            ProblemError::ClassMismatch { declared, reason } => write!(
                f,
                "declared class '{}' does not match the formulation: {}",
                declared.as_str(),
                reason
            ),
        }
    }
}

impl std::error::Error for ProblemError {}

fn require_finite(value: f64, context: &'static str) -> Result<(), ProblemError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(ProblemError::NonFiniteValue { context })
    }
}

fn check_indices(
    coefficients: &[Coefficient],
    variable_count: usize,
    context: &'static str,
) -> Result<(), ProblemError> {
    for coefficient in coefficients {
        require_finite(coefficient.coefficient, context)?;
        if coefficient.variable >= variable_count {
            return Err(ProblemError::VariableIndexOutOfRange {
                index: coefficient.variable,
                variable_count,
            });
        }
    }
    Ok(())
}

impl OptimizationProblem {
    /// Validates structure and the class declaration against the formulation
    /// body. A provider must call this before any solver interaction; the
    /// Tier 1 provider does (and additionally gates the declared class
    /// against its supported set, returning `unsupported` — not an error —
    /// for classes it cannot solve).
    pub fn validate(&self) -> Result<(), ProblemError> {
        if self.name.trim().is_empty() {
            return Err(ProblemError::EmptyName);
        }
        if self.variables.is_empty() {
            return Err(ProblemError::NoVariables);
        }

        let variable_count = self.variables.len();
        let mut seen_names: Vec<&str> = Vec::with_capacity(variable_count);
        for variable in &self.variables {
            if seen_names.contains(&variable.name.as_str()) {
                return Err(ProblemError::DuplicateVariableName(variable.name.clone()));
            }
            seen_names.push(variable.name.as_str());
            if let Some(lower) = variable.lower_bound {
                require_finite(lower, "variable lower bound")?;
            }
            if let Some(upper) = variable.upper_bound {
                require_finite(upper, "variable upper bound")?;
            }
            if let (Some(lower), Some(upper)) = (variable.lower_bound, variable.upper_bound) {
                if lower > upper {
                    return Err(ProblemError::InvertedBounds {
                        context: format!("variable '{}'", variable.name),
                    });
                }
            }
            if variable.kind == VariableKind::Binary {
                let in_range = |b: Option<f64>| match b {
                    None => true,
                    Some(v) => (0.0..=1.0).contains(&v),
                };
                if !in_range(variable.lower_bound) || !in_range(variable.upper_bound) {
                    return Err(ProblemError::BinaryBoundsOutsideUnitInterval(
                        variable.name.clone(),
                    ));
                }
            }
        }

        let mut seen_rows: Vec<&str> = Vec::with_capacity(self.rows.len());
        for row in &self.rows {
            if seen_rows.contains(&row.name.as_str()) {
                return Err(ProblemError::DuplicateRowName(row.name.clone()));
            }
            seen_rows.push(row.name.as_str());
            check_indices(&row.coefficients, variable_count, "row coefficient")?;
            match row.bounds {
                RowBounds::Lower(v) | RowBounds::Upper(v) | RowBounds::Equality(v) => {
                    require_finite(v, "row bound")?;
                }
                RowBounds::Range { lower, upper } => {
                    require_finite(lower, "row bound")?;
                    require_finite(upper, "row bound")?;
                    if lower > upper {
                        return Err(ProblemError::InvertedBounds {
                            context: format!("row '{}'", row.name),
                        });
                    }
                }
            }
        }

        let mut seen_cones: Vec<&str> = Vec::with_capacity(self.conic_constraints.len());
        for cone in &self.conic_constraints {
            if seen_cones.contains(&cone.name.as_str()) {
                return Err(ProblemError::DuplicateConeName(cone.name.clone()));
            }
            seen_cones.push(cone.name.as_str());
            check_indices(&cone.norm_members, variable_count, "cone norm member")?;
            check_indices(&cone.rhs_members, variable_count, "cone rhs member")?;
            require_finite(cone.norm_offset, "cone norm offset")?;
            require_finite(cone.rhs_offset, "cone rhs offset")?;
        }

        for term in &self.objective.linear_terms {
            check_indices(std::slice::from_ref(term), variable_count, "objective term")?;
        }
        require_finite(self.objective.constant, "objective constant")?;
        if self.objective.declared_units.trim().is_empty() {
            return Err(ProblemError::EmptyObjectiveUnits);
        }

        let mut seen_pairs: Vec<(usize, usize)> = Vec::with_capacity(
            self.objective.quadratic_terms.len(),
        );
        for term in &self.objective.quadratic_terms {
            require_finite(term.coefficient, "quadratic term coefficient")?;
            if term.row >= variable_count || term.column >= variable_count {
                return Err(ProblemError::VariableIndexOutOfRange {
                    index: term.row.max(term.column),
                    variable_count,
                });
            }
            if term.row > term.column {
                return Err(ProblemError::QuadraticTermNotUpperTriangle {
                    row: term.row,
                    column: term.column,
                });
            }
            if seen_pairs.contains(&(term.row, term.column)) {
                return Err(ProblemError::DuplicateQuadraticTerm {
                    row: term.row,
                    column: term.column,
                });
            }
            seen_pairs.push((term.row, term.column));
        }

        if self.provenance.input_snapshot_id.trim().is_empty() {
            // §12.6 requires the input snapshot on every result; a formulation
            // without one can never produce a complete result record.
            return Err(ProblemError::EmptyInputSnapshotId);
        }

        self.validate_class_consistency()
    }

    /// Checks the declared class against the formulation body. The IR cannot
    /// store nonlinear term structure, so a `NonlinearProgram` declaration
    /// passes structural validation here and is gated at the provider
    /// (documented on the [`ProblemClass::NonlinearProgram`] variant).
    fn validate_class_consistency(&self) -> Result<(), ProblemError> {
        let has_quadratic = !self.objective.quadratic_terms.is_empty();
        let has_integer = self
            .variables
            .iter()
            .any(|v| v.kind == VariableKind::Integer || v.kind == VariableKind::Binary);
        let has_cones = !self.conic_constraints.is_empty();

        let mismatch = |reason: String| ProblemError::ClassMismatch {
            declared: self.declared_class,
            reason,
        };

        match self.declared_class {
            ProblemClass::LinearProgram => {
                if has_quadratic {
                    return Err(mismatch(
                        "a linear program must not carry quadratic objective terms \
                         (declare convex_qp)"
                            .to_string(),
                    ));
                }
                if has_integer {
                    return Err(mismatch(
                        "a linear program must not declare integer/binary variables \
                         (declare milp)"
                            .to_string(),
                    ));
                }
                if has_cones {
                    return Err(mismatch(
                        "a linear program must not carry second-order cone constraints \
                         (declare socp/misocp)"
                            .to_string(),
                    ));
                }
            }
            ProblemClass::ConvexQuadraticProgram => {
                if !has_quadratic {
                    return Err(mismatch(
                        "a convex quadratic program must carry at least one quadratic \
                         objective term (a formulation without quadratic terms is an lp)"
                            .to_string(),
                    ));
                }
                if has_integer {
                    return Err(mismatch(
                        "a convex quadratic program must not declare integer/binary \
                         variables (a mixed-integer quadratic program is not a declared \
                         class; linearise as milp per report §12.3)"
                            .to_string(),
                    ));
                }
                if has_cones {
                    return Err(mismatch(
                        "a convex quadratic program must not carry second-order cone \
                         constraints (declare socp/misocp)"
                            .to_string(),
                    ));
                }
            }
            ProblemClass::MixedIntegerLinearProgram => {
                if has_quadratic {
                    return Err(mismatch(
                        "a mixed-integer linear program must not carry quadratic \
                         objective terms (a mixed-integer quadratic program is not a \
                         declared class; linearise per report §12.3)"
                            .to_string(),
                    ));
                }
                if !has_integer {
                    return Err(mismatch(
                        "a mixed-integer linear program must declare at least one \
                         integer/binary variable (a formulation without integrality is \
                         an lp)"
                            .to_string(),
                    ));
                }
                if has_cones {
                    return Err(mismatch(
                        "a mixed-integer linear program must not carry second-order cone \
                         constraints (that class is misocp, Tier 3 — see P3.20)"
                            .to_string(),
                    ));
                }
            }
            ProblemClass::ContinuousSecondOrderConeProgram => {
                if !has_cones {
                    return Err(mismatch(
                        "a continuous second-order cone program must carry at least one \
                         second-order cone constraint (a formulation without cones is an \
                         lp or convex_qp)"
                            .to_string(),
                    ));
                }
                if has_integer {
                    return Err(mismatch(
                        "a continuous second-order cone program must not declare \
                         integer/binary variables (that class is misocp, Tier 3)"
                            .to_string(),
                    ));
                }
            }
            ProblemClass::MixedIntegerSecondOrderConeProgram => {
                if !has_cones {
                    return Err(mismatch(
                        "a mixed-integer second-order cone program must carry at least \
                         one second-order cone constraint"
                            .to_string(),
                    ));
                }
                if !has_integer {
                    return Err(mismatch(
                        "a mixed-integer second-order cone program must declare at least \
                         one integer/binary variable (a formulation without integrality \
                         is socp)"
                            .to_string(),
                    ));
                }
            }
            ProblemClass::NonlinearProgram => {
                // No nonlinear term structure exists in this IR, so there is
                // nothing to check beyond structure; providers gate the class.
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::fixture;

    fn coefficient(variable: usize, coefficient: f64) -> Coefficient {
        Coefficient {
            variable,
            coefficient,
        }
    }

    #[test]
    fn reference_fixtures_validate() {
        fixture::reference_lp().validate().expect("lp validates");
        fixture::reference_qp().validate().expect("qp validates");
        fixture::reference_milp()
            .validate()
            .expect("milp validates");
        fixture::reference_socp_declaration()
            .validate()
            .expect("socp declaration validates");
        fixture::reference_misocp_declaration()
            .validate()
            .expect("misocp declaration validates");
        fixture::reference_nonlinear_declaration()
            .validate()
            .expect("nonlinear declaration validates");
    }

    #[test]
    fn lp_with_integer_variable_is_a_class_mismatch() {
        let mut problem = fixture::reference_lp();
        problem.variables[0].kind = VariableKind::Integer;
        match problem.validate() {
            Err(ProblemError::ClassMismatch { declared, .. }) => {
                assert_eq!(declared, ProblemClass::LinearProgram);
            }
            other => panic!("expected ClassMismatch, got {other:?}"),
        }
    }

    #[test]
    fn lp_with_quadratic_term_is_a_class_mismatch() {
        let mut problem = fixture::reference_lp();
        problem.objective.quadratic_terms = vec![QuadraticTerm {
            row: 0,
            column: 0,
            coefficient: 1.0,
        }];
        assert!(matches!(
            problem.validate(),
            Err(ProblemError::ClassMismatch { .. })
        ));
    }

    #[test]
    fn qp_without_quadratic_terms_is_a_class_mismatch() {
        let mut problem = fixture::reference_qp();
        problem.objective.quadratic_terms.clear();
        assert!(matches!(
            problem.validate(),
            Err(ProblemError::ClassMismatch { .. })
        ));
    }

    #[test]
    fn qp_with_integer_variable_is_a_class_mismatch() {
        let mut problem = fixture::reference_qp();
        problem.variables[0].kind = VariableKind::Integer;
        // The MIQP class is not declared anywhere; the mismatch reason must
        // point at the linearisation rule (§12.3), never at a silent solve.
        match problem.validate() {
            Err(ProblemError::ClassMismatch { reason, .. }) => {
                assert!(reason.contains("linearise"), "reason: {reason}");
            }
            other => panic!("expected ClassMismatch, got {other:?}"),
        }
    }

    #[test]
    fn milp_without_integer_variable_is_a_class_mismatch() {
        let mut problem = fixture::reference_milp();
        for variable in problem.variables.iter_mut() {
            variable.kind = VariableKind::Continuous;
        }
        assert!(matches!(
            problem.validate(),
            Err(ProblemError::ClassMismatch { .. })
        ));
    }

    #[test]
    fn milp_with_quadratic_term_is_a_class_mismatch() {
        let mut problem = fixture::reference_milp();
        problem.objective.quadratic_terms = vec![QuadraticTerm {
            row: 0,
            column: 0,
            coefficient: 1.0,
        }];
        assert!(matches!(
            problem.validate(),
            Err(ProblemError::ClassMismatch { .. })
        ));
    }

    #[test]
    fn socp_declaration_without_cone_is_a_class_mismatch() {
        let mut problem = fixture::reference_socp_declaration();
        problem.conic_constraints.clear();
        assert!(matches!(
            problem.validate(),
            Err(ProblemError::ClassMismatch { .. })
        ));
    }

    #[test]
    fn misocp_declaration_without_integer_variable_is_a_class_mismatch() {
        let mut problem = fixture::reference_misocp_declaration();
        for variable in problem.variables.iter_mut() {
            variable.kind = VariableKind::Continuous;
        }
        assert!(matches!(
            problem.validate(),
            Err(ProblemError::ClassMismatch { .. })
        ));
    }

    #[test]
    fn structural_errors_are_reported() {
        let mut problem = fixture::reference_lp();
        problem.variables[1].name = problem.variables[0].name.clone();
        assert!(matches!(
            problem.validate(),
            Err(ProblemError::DuplicateVariableName(_))
        ));

        let mut problem = fixture::reference_lp();
        problem.rows[0].coefficients = vec![coefficient(7, 1.0)];
        assert!(matches!(
            problem.validate(),
            Err(ProblemError::VariableIndexOutOfRange { .. })
        ));

        let mut problem = fixture::reference_lp();
        problem.rows[0].coefficients = vec![coefficient(0, f64::NAN)];
        assert!(matches!(
            problem.validate(),
            Err(ProblemError::NonFiniteValue { .. })
        ));

        let mut problem = fixture::reference_lp();
        problem.rows[0].bounds = RowBounds::Range { lower: 5.0, upper: 4.0 };
        assert!(matches!(
            problem.validate(),
            Err(ProblemError::InvertedBounds { .. })
        ));

        let mut problem = fixture::reference_qp();
        problem.objective.quadratic_terms.push(QuadraticTerm {
            row: 0,
            column: 0,
            coefficient: 0.25,
        });
        assert!(matches!(
            problem.validate(),
            Err(ProblemError::DuplicateQuadraticTerm { .. })
        ));

        let mut problem = fixture::reference_lp();
        problem.objective.declared_units = String::new();
        assert!(matches!(
            problem.validate(),
            Err(ProblemError::EmptyObjectiveUnits)
        ));
    }

    #[test]
    fn binary_bounds_must_stay_in_the_unit_interval() {
        let mut problem = fixture::reference_milp();
        problem.variables[1].upper_bound = Some(2.0);
        assert!(matches!(
            problem.validate(),
            Err(ProblemError::BinaryBoundsOutsideUnitInterval(_))
        ));
    }

    #[test]
    fn nonlinear_declaration_passes_structural_validation() {
        // The IR stores no nonlinear term structure; the declaration passes
        // structural validation and is gated at the provider (the Tier 1
        // provider returns `unsupported` — tested in tests/provider_status_mapping.rs).
        fixture::reference_nonlinear_declaration()
            .validate()
            .expect("structural validation only");
    }

    #[test]
    fn row_bounds_expand_with_infinities_for_unbounded_sides() {
        assert_eq!(RowBounds::Lower(2.0).as_lower_upper(), (2.0, f64::INFINITY));
        assert_eq!(
            RowBounds::Upper(-1.0).as_lower_upper(),
            (f64::NEG_INFINITY, -1.0)
        );
        assert_eq!(RowBounds::Equality(3.0).as_lower_upper(), (3.0, 3.0));
        assert_eq!(
            RowBounds::Range { lower: 1.0, upper: 2.0 }.as_lower_upper(),
            (1.0, 2.0)
        );
    }

    #[test]
    fn class_strings_and_tiers_are_stable() {
        let cases = [
            (ProblemClass::LinearProgram, "lp", 1u8),
            (ProblemClass::ConvexQuadraticProgram, "convex_qp", 1),
            (ProblemClass::MixedIntegerLinearProgram, "milp", 1),
            (ProblemClass::ContinuousSecondOrderConeProgram, "socp", 2),
            (ProblemClass::MixedIntegerSecondOrderConeProgram, "misocp", 3),
            (ProblemClass::NonlinearProgram, "nonlinear", 4),
        ];
        for (class, text, tier) in cases {
            assert_eq!(class.as_str(), text);
            assert_eq!(class.tier(), tier);
        }
    }
}
