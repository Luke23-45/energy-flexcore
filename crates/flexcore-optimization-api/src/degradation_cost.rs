// P15.08 — The degradation cost-curve data contract (optimizer INPUT DATA).
//
// Decision record:
// `docs/phases-11-20/phase-15-forecasting-asset-intelligence/records/
// 22_p15_08_degradation_cost_curve.md`.
//
// THE RULE, DECIDED ONCE (conventions §6.5; AGENTS.md rule 4 — absolute):
//
//   **Degradation cost enters the optimizer ONLY as linear or
//   piecewise-linear INPUT DATA per asset, feeding the existing
//   `C_degradation` term (P3.14 closed vocabulary). The solver-tier guards
//   and the MISOCP refusal stay; a data shape that would create a new
//   problem class is RESHAPED, never solved.**
//
// - The curves here are DATA the formulation assembler turns into linear
//   objective terms (and, for a convex piecewise-linear curve in a
//   minimization, an epigraph structure that is still an LP/QP/MILP). The
//   [`ProblemClass`] vocabulary, [`tier_guard`], and the providers' class
//   gates are UNTOUCHED by this module; the guard tests prove it
//   (`problem.rs` tests: an LP carrying degradation linear terms validates
//   as an LP; adding a cone is still a `ClassMismatch`; a MISOCP declaration
//   is still refused by [`tier_guard::guard_problem`]).
// - **Non-convex piecewise-linear data is REFUSED** with a named error and
//   the reshape guidance: representing a non-convex cost in a minimization
//   needs integer segment-selection variables — but the recorded rule
//   reshapes the DATA (a convex piecewise-linear approximation declared with
//   its provenance), it never adds a solver class.
// - **No degradation number is computed or claimed.** This module validates
//   declared curves and reads marginal costs OUT of them; it fits nothing,
//   measures nothing, and derives no parameter from data. Every curve value
//   is an INPUT with a required non-empty version and a provenance note
//   (the P3.15 no-untracked-constants discipline).
// - **Chemistry scope (P15-E-3, OPEN with DEFAULT (a))**: LFP and NMC are
//   the in-scope chemistries first, per BLAST-Lite's parameterized coverage
//   [S-147]. An out-of-scope-chemistry curve is constructible as declared
//   data but [`DegradationCostDeclaration::require_in_scope`] refuses it for
//   parameterized use — a RECORDED LIMITATION, never a silent extrapolation.
//   The label enum mirrors
//   `flexcore_domain::bess_degradation::DegradationChemistry` BY CITATION
//   (this crate depends on flexcore-contracts only — the
//   `artifact_signing.rs` mirror-by-citation posture).
// - **ADR-010 (conventions §6.3)**: a degradation price is INPUT DATA for
//   the simulation-line objective. It never authorizes or produces a
//   dispatch command.
// - The P3.06 `DegradationConfig` placeholder (`flexcore-domain::
//   bess_model`) is superseded AS DATA by these declared curves: the
//   placeholder metadata remains the evaluation-side record
//   (`flexcore-application::scheduling_objective`), while the optimizer
//   INPUT is the declared curve here. No degradation law is claimed by
//   either side.
//
// Control status: data contracts for the simulation-only 0.x line. No
// solver invocation, no I/O, no device-write path, no dispatch reach.

use serde::{Deserialize, Serialize};

// --- Recorded constants ------------------------------------------------------

/// The degradation cost-curve data-contract schema version. A structural
/// change to the curve shapes below is a VERSION BUMP, never a silent
/// reshape (the P3.14 schema-version discipline applied to input data).
pub const DEGRADATION_COST_DATA_SCHEMA_VERSION: u32 = 1;

/// The recorded honesty statement for the data contract.
pub const DEGRADATION_COST_DATA_STATEMENT: &str =
    "Degradation cost curves are optimizer INPUT DATA: linear or \
piecewise-linear, per asset, feeding the existing C_degradation term (P3.14 \
closed vocabulary). They carry no physics claim — every value is declared \
input with a version and a provenance note. Non-convex piecewise-linear \
data is refused (reshape the data, never the solver). Solver-tier guards \
and the MISOCP refusal are untouched. A degradation price never authorizes \
or produces a dispatch command (ADR-010).";

/// The chemistry-scope statement (P15-E-3 DEFAULT (a); recorded, agents
/// never close the E-decision).
pub const DEGRADATION_CHEMISTRY_SCOPE_STATEMENT: &str =
    "P15-E-3 (OPEN, DEFAULT (a)): LFP and NMC first, per BLAST-Lite's \
parameterized semi-empirical coverage [S-147]. Per-chemistry parameters \
are INPUTS declared with provenance, never universal laws; out-of-scope \
chemistries are recorded limitations, never silently extrapolated.";

// --- Chemistry scope -----------------------------------------------------------

/// The declared chemistry of a cost curve. MIRRORS
/// `flexcore_domain::bess_degradation::DegradationChemistry` BY CITATION
/// (no domain dependency: this crate's allowed edge set is
/// flexcore-contracts + serde/serde_json/time/clarabel; the mirror is kept
/// aligned by the scope statement and the tests).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DegradationChemistryLabel {
    /// Lithium iron phosphate — in scope (P15-E-3 DEFAULT (a)) [S-147].
    Lfp,
    /// Lithium nickel manganese cobalt oxide — in scope (P15-E-3 DEFAULT
    /// (a)) [S-147].
    Nmc,
    /// Any other chemistry: explicitly out of the P15-E-3 DEFAULT scope —
    /// a recorded limitation, never a silent extrapolation.
    #[serde(rename = "out_of_scope")]
    OutOfScope {
        /// The declared chemistry name (recorded, never interpreted).
        name: String,
    },
}

impl DegradationChemistryLabel {
    /// Whether the chemistry is inside the P15-E-3 DEFAULT coverage (a).
    pub fn is_in_scope(&self) -> bool {
        matches!(
            self,
            DegradationChemistryLabel::Lfp | DegradationChemistryLabel::Nmc
        )
    }

    /// The canonical label spelling.
    pub fn label(&self) -> String {
        match self {
            DegradationChemistryLabel::Lfp => "LFP".to_string(),
            DegradationChemistryLabel::Nmc => "NMC".to_string(),
            DegradationChemistryLabel::OutOfScope { name } => format!("OUT_OF_SCOPE({name})"),
        }
    }
}

// --- Errors ------------------------------------------------------------------

/// Errors of degradation cost-curve construction and use.
#[derive(Debug, Clone, PartialEq)]
pub enum DegradationCostError {
    /// A numeric datum was NaN or infinite.
    NonFiniteValue {
        /// The offending field.
        field: &'static str,
        /// The offending value.
        value: f64,
    },
    /// A magnitude that must be non-negative was negative.
    NegativeValue {
        /// The offending field.
        field: &'static str,
        /// The offending value.
        value: f64,
    },
    /// A magnitude that must be strictly positive was zero or negative.
    NonPositiveValue {
        /// The offending field.
        field: &'static str,
        /// The offending value.
        value: f64,
    },
    /// The piecewise segments were not strictly ascending in cumulative
    /// throughput (the breakpoint table is the declared data — it is never
    /// silently re-sorted).
    NonAscendingSegments {
        /// The index of the first offending segment (0-based).
        segment_index: usize,
    },
    /// The requested cumulative throughput lies beyond the declared
    /// segments: refused, never extrapolated (P3.02: no invented coverage).
    OutOfDeclaredRange {
        /// The requested cumulative throughput in MWh.
        requested_cumulative_throughput_mwh: f64,
        /// The declared coverage limit in MWh.
        declared_limit_mwh: f64,
    },
    /// The curve was declared without a version — an untracked constant is
    /// a validation error (the P3.15 discipline).
    UnversionedCurve,
    /// A required text field was empty or whitespace.
    EmptyField {
        /// The offending field.
        field: &'static str,
    },
    /// A text field exceeded its declared bound.
    FieldTooLong {
        /// The offending field.
        field: &'static str,
        /// The recorded bound.
        max_chars: usize,
    },
    /// The curve's chemistry is outside the P15-E-3 DEFAULT scope and was
    /// used where in-scope parameters are required — a recorded limitation
    /// enforced, never silently extrapolated.
    OutOfScopeChemistry {
        /// The declared chemistry label.
        label: String,
    },
    /// The piecewise-linear data is non-convex (marginal costs decreasing
    /// across segments): refused with the reshape guidance — the DATA is
    /// reshaped (a convex piecewise-linear approximation), never the solver.
    NonConvexPiecewiseLinear {
        /// The index of the first decreasing marginal-cost step (0-based).
        segment_index: usize,
        /// The previous segment's marginal cost.
        previous_marginal_cost: f64,
        /// The offending segment's marginal cost.
        offered_marginal_cost: f64,
    },
}

impl std::fmt::Display for DegradationCostError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DegradationCostError::NonFiniteValue { field, value } => {
                write!(f, "{field} must be finite (found {value})")
            }
            DegradationCostError::NegativeValue { field, value } => {
                write!(f, "{field} must be >= 0 (found {value})")
            }
            DegradationCostError::NonPositiveValue { field, value } => {
                write!(f, "{field} must be > 0 (found {value})")
            }
            DegradationCostError::NonAscendingSegments { segment_index } => write!(
                f,
                "piecewise segments must be strictly ascending in cumulative \
                 throughput (offending segment {segment_index})"
            ),
            DegradationCostError::OutOfDeclaredRange {
                requested_cumulative_throughput_mwh,
                declared_limit_mwh,
            } => write!(
                f,
                "cumulative throughput {requested_cumulative_throughput_mwh} MWh lies \
                 beyond the declared segment coverage {declared_limit_mwh} MWh; refused, \
                 never extrapolated (P3.02)"
            ),
            DegradationCostError::UnversionedCurve => {
                write!(
                    f,
                    "the cost curve has no version: an untracked input is a validation \
                     error (P3.15 — no untracked constants)"
                )
            }
            DegradationCostError::EmptyField { field } => {
                write!(f, "{field} must be non-empty and non-whitespace")
            }
            DegradationCostError::FieldTooLong { field, max_chars } => {
                write!(f, "{field} exceeds the {max_chars}-character bound")
            }
            DegradationCostError::OutOfScopeChemistry { label } => write!(
                f,
                "chemistry '{label}' is outside the P15-E-3 DEFAULT scope; no \
                 degradation parameter is credited for it (recorded limitation, never \
                 a silent extrapolation)"
            ),
            DegradationCostError::NonConvexPiecewiseLinear {
                segment_index,
                previous_marginal_cost,
                offered_marginal_cost,
            } => write!(
                f,
                "the piecewise-linear cost is non-convex: marginal cost falls from \
                 {previous_marginal_cost} to {offered_marginal_cost} at segment \
                 {segment_index}. Reshape the DATA (declare a convex piecewise-linear \
                 approximation with its provenance) — never a new solver class \
                 (conventions §6.5)"
            ),
        }
    }
}

impl std::error::Error for DegradationCostError {}

fn validate_text(field: &'static str, value: &str) -> Result<(), DegradationCostError> {
    if value.trim().is_empty() {
        return Err(DegradationCostError::EmptyField { field });
    }
    if value.len() > 256 {
        return Err(DegradationCostError::FieldTooLong { field, max_chars: 256 });
    }
    Ok(())
}

// --- The curve -----------------------------------------------------------------

/// One piecewise-linear segment: the cumulative-throughput bound (MWh,
/// strictly ascending across segments) and the marginal cost per MWh
/// within that segment (dimensioned currency-units/MWh — the P3.06/P3.14
/// "currency-units is deliberately generic" convention; no market currency
/// is implied, D1 OPEN).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PwlSegment {
    /// The cumulative-throughput bound of this segment in MWh (strictly
    /// ascending across the segment list).
    pub cumulative_throughput_limit_mwh: f64,
    /// The marginal cost per MWh of throughput within this segment (≥ 0).
    pub marginal_cost_per_mwh: f64,
}

/// A degradation cost curve (P15.08): LINEAR or piecewise-linear. This is
/// the ONLY cost shape the optimizer accepts as degradation INPUT DATA —
/// anything else (quadratic, exponential, chemistry-resolved fade
/// integration) is not representable without a new problem class and is
/// therefore not a declared shape (conventions §6.5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum DegradationCostCurve {
    /// A linear throughput penalty: one cost rate per MWh of
    /// (charge + discharge) throughput, ≥ 0. This is the exact shape the
    /// P3.06 placeholder used — now a DECLARED INPUT instead of a
    /// placeholder default.
    #[serde(rename_all = "snake_case")]
    Linear {
        /// The cost rate per MWh of throughput (≥ 0).
        cost_per_mwh_throughput: f64,
    },
    /// A piecewise-linear cycle-aging cost over cumulative throughput: the
    /// segment list is strictly ascending in `cumulative_throughput_limit_
    /// mwh`; the marginal cost per MWh is piecewise-constant. For a
    /// minimization, the curve must be convex (marginal costs
    /// non-decreasing) so the epigraph formulation stays an LP/QP/MILP —
    /// non-convex data is refused ([`DegradationCostError::
    /// NonConvexPiecewiseLinear`]; reshape the data, never the solver).
    #[serde(rename_all = "snake_case")]
    PiecewiseLinear {
        /// The declared segments (non-empty, strictly ascending).
        segments: Vec<PwlSegment>,
    },
}

impl DegradationCostCurve {
    /// Validates the curve's structure (finite, non-negative, ordered).
    pub fn validate(&self) -> Result<(), DegradationCostError> {
        match self {
            DegradationCostCurve::Linear {
                cost_per_mwh_throughput,
            } => {
                if !cost_per_mwh_throughput.is_finite() {
                    return Err(DegradationCostError::NonFiniteValue {
                        field: "cost_per_mwh_throughput",
                        value: *cost_per_mwh_throughput,
                    });
                }
                if *cost_per_mwh_throughput < 0.0 {
                    return Err(DegradationCostError::NegativeValue {
                        field: "cost_per_mwh_throughput",
                        value: *cost_per_mwh_throughput,
                    });
                }
                Ok(())
            }
            DegradationCostCurve::PiecewiseLinear { segments } => {
                if segments.is_empty() {
                    return Err(DegradationCostError::EmptyField { field: "segments" });
                }
                let mut previous_limit: Option<f64> = None;
                for (index, segment) in segments.iter().enumerate() {
                    if !segment.cumulative_throughput_limit_mwh.is_finite() {
                        return Err(DegradationCostError::NonFiniteValue {
                            field: "cumulative_throughput_limit_mwh",
                            value: segment.cumulative_throughput_limit_mwh,
                        });
                    }
                    if segment.cumulative_throughput_limit_mwh <= 0.0 {
                        return Err(DegradationCostError::NonPositiveValue {
                            field: "cumulative_throughput_limit_mwh",
                            value: segment.cumulative_throughput_limit_mwh,
                        });
                    }
                    if let Some(previous) = previous_limit {
                        if segment.cumulative_throughput_limit_mwh <= previous {
                            return Err(DegradationCostError::NonAscendingSegments {
                                segment_index: index,
                            });
                        }
                    }
                    if !segment.marginal_cost_per_mwh.is_finite() {
                        return Err(DegradationCostError::NonFiniteValue {
                            field: "marginal_cost_per_mwh",
                            value: segment.marginal_cost_per_mwh,
                        });
                    }
                    if segment.marginal_cost_per_mwh < 0.0 {
                        return Err(DegradationCostError::NegativeValue {
                            field: "marginal_cost_per_mwh",
                            value: segment.marginal_cost_per_mwh,
                        });
                    }
                    previous_limit = Some(segment.cumulative_throughput_limit_mwh);
                }
                Ok(())
            }
        }
    }

    /// The convexity check for minimization use (the recorded rule): the
    /// marginal costs of a piecewise-linear curve must be non-decreasing so
    /// the epigraph formulation stays an LP/QP/MILP. Linear curves are
    /// convex by construction. A refusal names the decreasing step and
    /// carries the reshape guidance.
    pub fn validate_convex_for_minimization(&self) -> Result<(), DegradationCostError> {
        match self {
            DegradationCostCurve::Linear { .. } => Ok(()),
            DegradationCostCurve::PiecewiseLinear { segments } => {
                self.validate()?;
                for index in 1..segments.len() {
                    let (previous, offered) =
                        (&segments[index - 1], &segments[index]);
                    if offered.marginal_cost_per_mwh < previous.marginal_cost_per_mwh {
                        return Err(DegradationCostError::NonConvexPiecewiseLinear {
                            segment_index: index,
                            previous_marginal_cost: previous.marginal_cost_per_mwh,
                            offered_marginal_cost: offered.marginal_cost_per_mwh,
                        });
                    }
                }
                Ok(())
            }
        }
    }

    /// The marginal cost per MWh at a cumulative throughput. For a linear
    /// curve, the rate. For a piecewise curve, the marginal cost of the
    /// segment containing the throughput (lower bound exclusive, upper
    /// bound inclusive; the FIRST segment contains zero). Beyond the
    /// declared coverage: refused, never extrapolated.
    pub fn marginal_cost_at(
        &self,
        cumulative_throughput_mwh: f64,
    ) -> Result<f64, DegradationCostError> {
        if !cumulative_throughput_mwh.is_finite() {
            return Err(DegradationCostError::NonFiniteValue {
                field: "cumulative_throughput_mwh",
                value: cumulative_throughput_mwh,
            });
        }
        if cumulative_throughput_mwh < 0.0 {
            return Err(DegradationCostError::NegativeValue {
                field: "cumulative_throughput_mwh",
                value: cumulative_throughput_mwh,
            });
        }
        match self {
            DegradationCostCurve::Linear {
                cost_per_mwh_throughput,
            } => Ok(*cost_per_mwh_throughput),
            DegradationCostCurve::PiecewiseLinear { segments } => {
                for segment in segments {
                    if cumulative_throughput_mwh <= segment.cumulative_throughput_limit_mwh {
                        return Ok(segment.marginal_cost_per_mwh);
                    }
                }
                Err(DegradationCostError::OutOfDeclaredRange {
                    requested_cumulative_throughput_mwh: cumulative_throughput_mwh,
                    declared_limit_mwh: segments[segments.len() - 1]
                        .cumulative_throughput_limit_mwh,
                })
            }
        }
    }
}

// --- The declaration -------------------------------------------------------------

/// One asset's declared degradation cost data (P15.08): the curve, its
/// required version, its chemistry scope, and its provenance note. This is
/// the per-asset record the formulation assembler consumes; the evaluated
/// `C_degradation` total on the application side remains the P3.06
/// evaluation output (recorded split: DATA vs evaluation record).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DegradationCostDeclaration {
    /// The asset the curve applies to (registry reference string form).
    pub asset_id: String,
    /// The declared curve version (required non-empty — the P3.15
    /// no-untracked-constants discipline).
    pub curve_version: String,
    /// The declared chemistry scope (P15-E-3; out-of-scope chemistries are
    /// recorded limitations).
    pub chemistry: DegradationChemistryLabel,
    /// The declared curve.
    pub curve: DegradationCostCurve,
    /// The provenance note (where the values came from, and their assumed
    /// status — REQUIRED non-empty: a curve without provenance is an
    /// untracked constant).
    pub provenance_note: String,
}

impl DegradationCostDeclaration {
    /// Constructs and validates a declaration (curve structure, version,
    /// provenance note).
    pub fn new(
        asset_id: String,
        curve_version: String,
        chemistry: DegradationChemistryLabel,
        curve: DegradationCostCurve,
        provenance_note: String,
    ) -> Result<Self, DegradationCostError> {
        validate_text("asset_id", &asset_id)?;
        if curve_version.trim().is_empty() {
            return Err(DegradationCostError::UnversionedCurve);
        }
        if curve_version.len() > 256 {
            return Err(DegradationCostError::FieldTooLong {
                field: "curve_version",
                max_chars: 256,
            });
        }
        validate_text("provenance_note", &provenance_note)?;
        curve.validate()?;
        Ok(Self {
            asset_id,
            curve_version,
            chemistry,
            curve,
            provenance_note,
        })
    }

    /// The in-scope gate (P15-E-3): refuses an out-of-scope-chemistry
    /// declaration where in-scope parameters are required — a recorded
    /// limitation enforced, never a silent extrapolation.
    pub fn require_in_scope(&self) -> Result<(), DegradationCostError> {
        if self.chemistry.is_in_scope() {
            Ok(())
        } else {
            Err(DegradationCostError::OutOfScopeChemistry {
                label: self.chemistry.label(),
            })
        }
    }

    /// The marginal cost per MWh at a cumulative throughput (see
    /// [`DegradationCostCurve::marginal_cost_at`]).
    pub fn marginal_cost_at(
        &self,
        cumulative_throughput_mwh: f64,
    ) -> Result<f64, DegradationCostError> {
        self.curve.marginal_cost_at(cumulative_throughput_mwh)
    }
}

// --- Tests ------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- linear curves ------------------------------------------------------

    #[test]
    fn linear_curves_validate_and_read_their_rate() {
        let curve = DegradationCostCurve::Linear {
            cost_per_mwh_throughput: 12.5,
        };
        curve.validate().unwrap();
        curve.validate_convex_for_minimization().unwrap();
        assert_eq!(curve.marginal_cost_at(0.0).unwrap(), 12.5);
        assert_eq!(curve.marginal_cost_at(9_999.0).unwrap(), 12.5);
    }

    #[test]
    fn linear_curves_reject_negative_and_non_finite_rates() {
        assert!(matches!(
            DegradationCostCurve::Linear { cost_per_mwh_throughput: -1.0 }.validate(),
            Err(DegradationCostError::NegativeValue { field: "cost_per_mwh_throughput", .. })
        ));
        assert!(matches!(
            DegradationCostCurve::Linear { cost_per_mwh_throughput: f64::NAN }.validate(),
            Err(DegradationCostError::NonFiniteValue { field: "cost_per_mwh_throughput", .. })
        ));
    }

    // --- piecewise-linear curves ---------------------------------------------

    fn segments(values: &[(f64, f64)]) -> Vec<PwlSegment> {
        values
            .iter()
            .map(|(limit, cost)| PwlSegment {
                cumulative_throughput_limit_mwh: *limit,
                marginal_cost_per_mwh: *cost,
            })
            .collect()
    }

    #[test]
    fn convex_piecewise_curves_validate_and_resolve_segments() {
        let curve = DegradationCostCurve::PiecewiseLinear {
            segments: segments(&[(100.0, 8.0), (500.0, 8.0), (2_000.0, 11.0)]),
        };
        curve.validate().unwrap();
        // Non-decreasing marginal costs: convex.
        curve.validate_convex_for_minimization().unwrap();
        // Lower bound exclusive, upper bound inclusive.
        assert_eq!(curve.marginal_cost_at(0.0).unwrap(), 8.0);
        assert_eq!(curve.marginal_cost_at(100.0).unwrap(), 8.0);
        assert_eq!(curve.marginal_cost_at(100.0 + 1e-9).unwrap(), 8.0);
        assert_eq!(curve.marginal_cost_at(500.0).unwrap(), 8.0);
        assert_eq!(curve.marginal_cost_at(501.0).unwrap(), 11.0);
        assert_eq!(curve.marginal_cost_at(2_000.0).unwrap(), 11.0);
    }

    #[test]
    fn non_convex_piecewise_data_is_refused_with_reshape_guidance() {
        let curve = DegradationCostCurve::PiecewiseLinear {
            segments: segments(&[(100.0, 10.0), (500.0, 6.0), (2_000.0, 12.0)]),
        };
        curve.validate().unwrap(); // structurally valid data
        let error = curve.validate_convex_for_minimization().unwrap_err();
        assert!(matches!(
            error,
            DegradationCostError::NonConvexPiecewiseLinear {
                previous_marginal_cost,
                offered_marginal_cost,
                ..
            } if previous_marginal_cost == 10.0 && offered_marginal_cost == 6.0
        ));
        // The refusal carries the reshape-the-data guidance verbatim.
        assert!(error.to_string().contains("Reshape the DATA"));
    }

    #[test]
    fn beyond_declared_coverage_is_refused_never_extrapolated() {
        let curve = DegradationCostCurve::PiecewiseLinear {
            segments: segments(&[(100.0, 8.0), (500.0, 9.0)]),
        };
        assert!(matches!(
            curve.marginal_cost_at(500.0 + 1.0),
            Err(DegradationCostError::OutOfDeclaredRange {
                requested_cumulative_throughput_mwh,
                declared_limit_mwh,
            } if requested_cumulative_throughput_mwh == 501.0 && declared_limit_mwh == 500.0
        ))
    }

    #[test]
    fn segment_tables_are_validated() {
        assert!(matches!(
            DegradationCostCurve::PiecewiseLinear { segments: vec![] }.validate(),
            Err(DegradationCostError::EmptyField { field: "segments" })
        ));
        assert!(matches!(
            DegradationCostCurve::PiecewiseLinear {
                segments: segments(&[(500.0, 8.0), (100.0, 9.0)])
            }
            .validate(),
            Err(DegradationCostError::NonAscendingSegments { segment_index: 1 })
        ));
        assert!(matches!(
            DegradationCostCurve::PiecewiseLinear {
                segments: segments(&[(100.0, 8.0), (100.0, 9.0)])
            }
            .validate(),
            Err(DegradationCostError::NonAscendingSegments { segment_index: 1 })
        ));
        assert!(matches!(
            DegradationCostCurve::PiecewiseLinear {
                segments: segments(&[(100.0, -1.0)])
            }
            .validate(),
            Err(DegradationCostError::NegativeValue { field: "marginal_cost_per_mwh", .. })
        ));
        assert!(matches!(
            DegradationCostCurve::PiecewiseLinear {
                segments: segments(&[(0.0, 8.0)])
            }
            .validate(),
            Err(DegradationCostError::NonPositiveValue {
                field: "cumulative_throughput_limit_mwh",
                ..
            })
        ));
    }

    // --- declarations ---------------------------------------------------------

    fn declaration(chemistry: DegradationChemistryLabel) -> DegradationCostDeclaration {
        DegradationCostDeclaration::new(
            "bess-a".to_string(),
            "p15-08-lfp-curve-v1".to_string(),
            chemistry,
            DegradationCostCurve::Linear {
                cost_per_mwh_throughput: 7.25,
            },
            "ASSUMPTION: declared planning-aid rate with no measured basis".to_string(),
        )
        .unwrap()
    }

    #[test]
    fn declarations_require_version_and_provenance() {
        assert!(matches!(
            DegradationCostDeclaration::new(
                "bess-a".to_string(),
                "  ".to_string(),
                DegradationChemistryLabel::Lfp,
                DegradationCostCurve::Linear { cost_per_mwh_throughput: 1.0 },
                "note".to_string(),
            ),
            Err(DegradationCostError::UnversionedCurve)
        ));
        assert!(matches!(
            DegradationCostDeclaration::new(
                "bess-a".to_string(),
                "v1".to_string(),
                DegradationChemistryLabel::Lfp,
                DegradationCostCurve::Linear { cost_per_mwh_throughput: 1.0 },
                " ".to_string(),
            ),
            Err(DegradationCostError::EmptyField { field: "provenance_note" })
        ));
    }

    #[test]
    fn in_scope_chemistry_is_the_e3_default_lfp_and_nmc() {
        assert!(declaration(DegradationChemistryLabel::Lfp).require_in_scope().is_ok());
        assert!(declaration(DegradationChemistryLabel::Nmc).require_in_scope().is_ok());
        let out = declaration(DegradationChemistryLabel::OutOfScope {
            name: "LTO".to_string(),
        });
        assert!(matches!(
            out.require_in_scope(),
            Err(DegradationCostError::OutOfScopeChemistry { .. })
        ));
        assert_eq!(out.chemistry.label(), "OUT_OF_SCOPE(LTO)");
    }

    // --- recorded statements ------------------------------------------------------

    #[test]
    fn the_data_contract_statement_carries_the_discipline() {
        assert!(DEGRADATION_COST_DATA_STATEMENT.contains("INPUT DATA"));
        assert!(DEGRADATION_COST_DATA_STATEMENT.contains("never authorizes"));
        assert!(DEGRADATION_CHEMISTRY_SCOPE_STATEMENT.contains("LFP and NMC first"));
        assert!(DEGRADATION_COST_DATA_STATEMENT.contains("refused"));
    }
}
