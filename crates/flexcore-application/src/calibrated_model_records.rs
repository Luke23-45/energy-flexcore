// P15.14 — Per-asset calibrated model records: declared thermal and
//         efficiency model RECORDS per asset, with provenance — never
//         fitted claims — plus regression test fixtures written complete
//         and UNRUN.
//
// Phase 15 plan §2 row P15.14; decision record:
// `docs/phases-11-20/phase-15-forecasting-asset-intelligence/records/
// 43_p15_14_calibrated_model_records.md`.
// Sources: [S-168] (the digital-twin spectrum — the "calibrated per-asset
// model" end this record type prepares for, explicitly at the lite end:
// records, not a live twin), [S-53] (asset-performance-management
// context). No vendor or tooling claim is made.
//
// THE SUPERSESSION, RECORDED (not an edit): the Phase 2 simulator's
// behaviour profiles record their own absence —
// `crates/flexcore-simulator/src/profiles.rs`: "no electrochemical,
// temperature, degradation, or state-of-health model; constant
// efficiency". That recorded absence is SUPERSEDED AS RECORDED
// PARAMETERS by this module: the platform now has a home where a
// per-asset thermal/efficiency model can be DECLARED as data with
// provenance. The simulator profile file itself is NOT edited and stays
// synthetic; the supersession is a record-level fact, never a code
// change to the Phase 2 profile.
//
// THE HONESTY LINES, DECIDED ONCE:
//
// 1. **DECLARED, never fitted.** Every parameter in every record is a
//    DECLARED value with provenance and a recorded source. There is NO
//    fitting procedure anywhere in this module: no optimizer, no
//    regression, no calibration run. A "calibrated" claim would require
//    runtime data (the platform has never run) and an evaluation
//    discipline (X-8) that has not executed. The record TYPE is the
//    calibrated-model home; the CALIBRATION is a future, human-gated
//    activity that would land as a NEW record version naming its
//    evidence — never an in-place relabel.
// 2. **Records, not a twin.** A model record is data about an asset,
//    consumed by analytics and operator views. It is not a live
//    simulation, not a state estimator, and carries no dispatch reach
//    (ADR-010 extension, program conventions §6.3): no record
//    constructs, mutates, or influences a dispatch command.
// 3. **Canonical units.** Temperatures in degrees Celsius (the declared
//    ambient reference convention), time constants in hours,
//    efficiencies as fractions in (0, 1], power in MW (P > 0 export,
//    §10.2). Units travel IN the field names — never bare floats.
// 4. **Deterministic checkability.** The declared models come with pure
//    evaluation functions (first-order thermal step; piecewise-linear
//    efficiency interpolation) so a declared record can be CHECKED
//    against its declared expectations. The functions are arithmetic,
//    not claims.
//
// Control status: records and pure arithmetic for the simulation-only
// 0.x line. No I/O, no store, no scheduler wiring, no field-write path.
// No safety, certification, or skill claim is made.

use flexcore_domain::ids::AssetId;

/// The calibrated-model-record schema version (a change is a NEW version,
/// never a silent edit).
pub const CALIBRATED_MODEL_RECORD_SCHEMA_VERSION: u32 = 1;

/// The declared-not-fitted statement (pinned by tests).
pub const DECLARED_NOT_FITTED_STATEMENT: &str = "Every parameter in a calibrated-model \
     record is DECLARED with provenance and a recorded source. No fitting, calibration, \
     or estimation procedure exists in this module or produced these values: the platform \
     has never run, no runtime data exists, and no evaluation discipline (X-8) has \
     executed. A future calibration is a NEW record version naming its evidence — never \
     an in-place relabel of a declared record.";

/// The simulator-profile supersession statement (pinned by tests; the
/// profile file itself is NOT edited).
pub const SIMULATOR_PROFILE_SUPERSESSION_STATEMENT: &str = "The Phase 2 simulator's \
     recorded profile absence (profiles.rs: no temperature, degradation, or \
     state-of-health model; constant efficiency) is superseded AS RECORDED PARAMETERS: \
     this module is the home where a per-asset thermal/efficiency model is DECLARED as \
     data with provenance. The simulator profile is unchanged and stays synthetic; the \
     supersession is a record-level fact, never a code change to Phase 2.";

/// The ADR-010 statement (pinned by tests).
pub const MODEL_RECORD_IS_NOT_A_COMMAND_STATEMENT: &str = "A calibrated-model record is \
     data about an asset, consumed by analytics and operator views. It never authorizes, \
     produces, or influences a dispatch command (ADR-010 extension, program conventions \
     6.3).";

// --- Errors -------------------------------------------------------------------

/// Why a model record or evaluation was refused. Named, never absorbed.
#[derive(Debug, Clone, PartialEq)]
pub enum ModelRecordError {
    /// A declared parameter value was not finite.
    NonFiniteParameter { name: String },
    /// A declared efficiency was outside (0, 1].
    EfficiencyOutOfRange { name: String, value: f64 },
    /// A declared time constant was non-positive.
    NonPositiveTimeConstant { name: String, value: f64 },
    /// An efficiency curve had fewer than two breakpoints (a curve needs
    /// at least two points; a single point is a constant, declared as
    /// such, not as a curve).
    CurveTooShort { points: usize },
    /// The efficiency curve's power fractions were not strictly
    /// increasing (the interpolation is undefined on a non-monotone
    /// axis).
    CurveNotMonotonic { index: usize },
    /// A curve power fraction was outside [0, 1] (fractions of rated
    /// power).
    CurveFractionOutOfRange { index: usize, value: f64 },
    /// An evaluation input was non-finite.
    NonFiniteInput { name: String },
    /// An efficiency interpolation was requested outside the declared
    /// curve range. The record refuses rather than extrapolating —
    /// extrapolation is a model claim the declaration never made.
    InterpolationOutsideCurve { requested: f64, declared_max: f64 },
    /// The record's source string was empty (the provenance source must
    /// be recorded, never silent).
    EmptySource,
}

impl std::fmt::Display for ModelRecordError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModelRecordError::NonFiniteParameter { name } => {
                write!(f, "the declared parameter {name} is not finite")
            }
            ModelRecordError::EfficiencyOutOfRange { name, value } => write!(
                f,
                "the declared efficiency {name} is {value}; it must be in (0, 1]"
            ),
            ModelRecordError::NonPositiveTimeConstant { name, value } => write!(
                f,
                "the declared time constant {name} is {value}; it must be positive"
            ),
            ModelRecordError::CurveTooShort { points } => write!(
                f,
                "an efficiency curve needs at least 2 breakpoints; got {points} (a \
                 constant is declared as such, not as a one-point curve)"
            ),
            ModelRecordError::CurveNotMonotonic { index } => write!(
                f,
                "the efficiency curve's power fractions are not strictly increasing at \
                 index {index}; the interpolation is undefined on a non-monotone axis"
            ),
            ModelRecordError::CurveFractionOutOfRange { index, value } => write!(
                f,
                "the curve power fraction at index {index} is {value}; fractions of rated \
                 power are in [0, 1]"
            ),
            ModelRecordError::NonFiniteInput { name } => {
                write!(f, "the evaluation input {name} is not finite")
            }
            ModelRecordError::InterpolationOutsideCurve {
                requested,
                declared_max,
            } => write!(
                f,
                "efficiency interpolation requested at power fraction {requested}, beyond \
                 the declared curve maximum {declared_max}; the record refuses rather than \
                 extrapolating a claim the declaration never made"
            ),
            ModelRecordError::EmptySource => write!(
                f,
                "the record must carry a recorded source (never silent)"
            ),
        }
    }
}

impl std::error::Error for ModelRecordError {}

// --- Shared record vocabulary ---------------------------------------------------

/// One declared parameter: a named value in a unit-carrying field, with
/// provenance and a recorded source. No bare floats travel without a
/// name.
#[derive(Debug, Clone, PartialEq)]
pub struct DeclaredParameter {
    /// The parameter name (e.g. `tau_thermal_hours`).
    pub name: String,
    /// The declared value, in the unit the NAME carries.
    pub value: f64,
    /// The provenance of the declaration (Measured / Configured /
    /// Inferred / Unknown — unknown narrows, never widens, P3.02).
    pub provenance: flexcore_contracts::capability::Provenance,
    /// The recorded source of the declaration (non-empty; e.g. the
    /// nameplate datasheet reference, the operator's commissioning
    /// record).
    pub source: String,
}

impl DeclaredParameter {
    /// Validates and constructs one parameter.
    pub fn new(
        name: String,
        value: f64,
        provenance: flexcore_contracts::capability::Provenance,
        source: String,
    ) -> Result<Self, ModelRecordError> {
        if !value.is_finite() {
            return Err(ModelRecordError::NonFiniteParameter { name });
        }
        if source.trim().is_empty() {
            return Err(ModelRecordError::EmptySource);
        }
        Ok(Self {
            name,
            value,
            provenance,
            source,
        })
    }
}

// --- 1. The declared thermal model ------------------------------------------------

/// A per-asset DECLARED first-order thermal model: the lumped RC
/// equivalent (`tau_thermal_hours`, `temp_rise_per_mw_c`) plus the
/// ambient reference. First-order and linear ON PURPOSE: the declared
/// model is checkable by hand, and complexity without data is a claim,
/// not a model.
#[derive(Debug, Clone, PartialEq)]
pub struct DeclaredThermalModel {
    /// The asset the model is declared for.
    pub asset_id: AssetId,
    /// The thermal time constant, hours (`tau_thermal_hours`).
    pub tau_thermal_hours: DeclaredParameter,
    /// The steady-state temperature rise per MW of dissipated power,
    /// degrees Celsius per MW (`temp_rise_per_mw_c`).
    pub temp_rise_per_mw_c: DeclaredParameter,
    /// The declared ambient reference temperature, degrees Celsius
    /// (`ambient_reference_c`).
    pub ambient_reference_c: DeclaredParameter,
}

impl DeclaredThermalModel {
    /// Validates and constructs one declared thermal model.
    pub fn new(
        asset_id: AssetId,
        tau_thermal_hours: DeclaredParameter,
        temp_rise_per_mw_c: DeclaredParameter,
        ambient_reference_c: DeclaredParameter,
    ) -> Result<Self, ModelRecordError> {
        if tau_thermal_hours.value <= 0.0 {
            return Err(ModelRecordError::NonPositiveTimeConstant {
                name: tau_thermal_hours.name.clone(),
                value: tau_thermal_hours.value,
            });
        }
        if !temp_rise_per_mw_c.value.is_finite() || !ambient_reference_c.value.is_finite() {
            return Err(ModelRecordError::NonFiniteParameter {
                name: if temp_rise_per_mw_c.value.is_finite() {
                    ambient_reference_c.name.clone()
                } else {
                    temp_rise_per_mw_c.name.clone()
                },
            });
        }
        Ok(Self {
            asset_id,
            tau_thermal_hours,
            temp_rise_per_mw_c,
            ambient_reference_c,
        })
    }

    /// The pure first-order step evaluation: the component temperature
    /// after `hours` at constant dissipated power `power_mw`, from an
    /// initial temperature `initial_temp_c`.
    ///
    /// `T(t) = T_ambient + P·k + (T_initial − (T_ambient + P·k)) · e^(−t/τ)`
    ///
    /// — the textbook first-order response of the DECLARED parameters.
    /// Arithmetic, not a claim: the result is exactly as valid as the
    /// declaration behind it.
    pub fn temperature_after(
        &self,
        power_mw: f64,
        initial_temp_c: f64,
        hours: f64,
    ) -> Result<f64, ModelRecordError> {
        for (name, value) in [
            ("power_mw", power_mw),
            ("initial_temp_c", initial_temp_c),
            ("hours", hours),
        ] {
            if !value.is_finite() {
                return Err(ModelRecordError::NonFiniteInput {
                    name: name.to_string(),
                });
            }
        }
        let steady_state =
            self.ambient_reference_c.value + power_mw * self.temp_rise_per_mw_c.value;
        let decay = (-(hours / self.tau_thermal_hours.value)).exp();
        Ok(steady_state + (initial_temp_c - steady_state) * decay)
    }
}

// --- 2. The declared efficiency model ----------------------------------------------

/// One efficiency-curve breakpoint: a power fraction of rated (in
/// [0, 1]) and the declared efficiency at that fraction (in (0, 1]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EfficiencyBreakpoint {
    /// The power fraction of rated (in [0, 1]).
    pub power_fraction: f64,
    /// The declared efficiency at that fraction (in (0, 1]).
    pub efficiency: f64,
}

/// A per-asset DECLARED efficiency curve: piecewise-linear over the
/// declared breakpoints, with the DECLARED interpolation rule (linear
/// inside the curve, refusal outside — never extrapolation). A constant
/// efficiency is declared as a single named parameter via
/// [`DeclaredConstantEfficiency`], not as a one-point curve.
#[derive(Debug, Clone, PartialEq)]
pub struct DeclaredEfficiencyCurve {
    /// The asset the model is declared for.
    pub asset_id: AssetId,
    /// The declared breakpoints, in STRICTLY INCREASING power-fraction
    /// order (validated at construction).
    pub breakpoints: Vec<EfficiencyBreakpoint>,
}

impl DeclaredEfficiencyCurve {
    /// Validates and constructs one declared efficiency curve.
    pub fn new(
        asset_id: AssetId,
        breakpoints: Vec<EfficiencyBreakpoint>,
    ) -> Result<Self, ModelRecordError> {
        if breakpoints.len() < 2 {
            return Err(ModelRecordError::CurveTooShort {
                points: breakpoints.len(),
            });
        }
        for (index, point) in breakpoints.iter().enumerate() {
            if !point.power_fraction.is_finite()
                || !(0.0..=1.0).contains(&point.power_fraction)
            {
                return Err(ModelRecordError::CurveFractionOutOfRange {
                    index,
                    value: point.power_fraction,
                });
            }
            if !point.efficiency.is_finite() || !(0.0..=1.0).contains(&point.efficiency)
            {
                return Err(ModelRecordError::EfficiencyOutOfRange {
                    name: "curve_efficiency".to_string(),
                    value: point.efficiency,
                });
            }
            if index > 0 && point.power_fraction <= breakpoints[index - 1].power_fraction
            {
                return Err(ModelRecordError::CurveNotMonotonic { index });
            }
        }
        Ok(Self {
            asset_id,
            breakpoints,
        })
    }

    /// The declared efficiency at a power fraction: linear interpolation
    /// inside the declared curve; a named refusal outside it (never
    /// extrapolation). The first breakpoint need not be at 0.0 — below
    /// it, the record refuses: behaviour below the declared curve is not
    /// a claim the declaration made.
    pub fn efficiency_at(&self, power_fraction: f64) -> Result<f64, ModelRecordError> {
        if !power_fraction.is_finite() {
            return Err(ModelRecordError::NonFiniteInput {
                name: "power_fraction".to_string(),
            });
        }
        let first = self.breakpoints[0];
        let last = self.breakpoints[self.breakpoints.len() - 1];
        if power_fraction < first.power_fraction || power_fraction > last.power_fraction
        {
            return Err(ModelRecordError::InterpolationOutsideCurve {
                requested: power_fraction,
                declared_max: last.power_fraction,
            });
        }
        // Linear scan (breakpoints are few and declared; determinism
        // beats cleverness — BTreeMap-style fixed order, no bsearch
        // tie-breaking subtleties).
        for window in self.breakpoints.windows(2) {
            let (lo, hi) = (window[0], window[1]);
            if power_fraction <= hi.power_fraction {
                let span = hi.power_fraction - lo.power_fraction;
                if span == 0.0 {
                    // Unreachable (strictly increasing), defensive only.
                    return Ok(lo.efficiency);
                }
                let t = (power_fraction - lo.power_fraction) / span;
                return Ok(lo.efficiency + t * (hi.efficiency - lo.efficiency));
            }
        }
        Ok(last.efficiency)
    }
}

/// A per-asset DECLARED constant efficiency (the Phase 2 simulator's
/// `sqrt(round_trip_efficiency)` split declared as data — see the
/// recorded profile convention).
#[derive(Debug, Clone, PartialEq)]
pub struct DeclaredConstantEfficiency {
    /// The asset the model is declared for.
    pub asset_id: AssetId,
    /// The declared constant efficiency (in (0, 1]).
    pub efficiency: DeclaredParameter,
}

impl DeclaredConstantEfficiency {
    /// Validates and constructs one declared constant efficiency.
    pub fn new(
        asset_id: AssetId,
        efficiency: DeclaredParameter,
    ) -> Result<Self, ModelRecordError> {
        if !(0.0..=1.0).contains(&efficiency.value) || efficiency.value == 0.0 {
            return Err(ModelRecordError::EfficiencyOutOfRange {
                name: efficiency.name.clone(),
                value: efficiency.value,
            });
        }
        Ok(Self {
            asset_id,
            efficiency,
        })
    }
}

// --- Tests (WRITTEN-UNRUN — no cargo command was executed in this loop;
// --- the human build/test gate owns the run. Every expected value below
// --- is hand-computed from the rule sets above over CONSTRUCTED inputs.
// --- Nothing is fabricated.) -----------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use flexcore_contracts::capability::Provenance;
    use flexcore_contracts::ids::FlexUuid;

    fn uid(n: u128) -> FlexUuid {
        FlexUuid::from_bytes(n.to_be_bytes())
    }

    fn asset(tag: u128) -> AssetId {
        AssetId::new(uid(tag))
    }

    fn param(name: &str, value: f64) -> DeclaredParameter {
        DeclaredParameter::new(
            name.to_string(),
            value,
            Provenance::Configured,
            "declared-test-source".to_string(),
        )
        .expect("valid parameter")
    }

    fn thermal_model() -> DeclaredThermalModel {
        DeclaredThermalModel::new(
            asset(0xB001),
            param("tau_thermal_hours", 2.0),
            param("temp_rise_per_mw_c", 10.0),
            param("ambient_reference_c", 20.0),
        )
        .expect("valid thermal model")
    }

    #[test]
    fn statements_are_the_recorded_strings() {
        assert!(DECLARED_NOT_FITTED_STATEMENT.contains("DECLARED with provenance"));
        assert!(DECLARED_NOT_FITTED_STATEMENT.contains("never an in-place relabel"));
        assert!(
            SIMULATOR_PROFILE_SUPERSESSION_STATEMENT
                .contains("superseded AS RECORDED PARAMETERS")
        );
        assert!(
            SIMULATOR_PROFILE_SUPERSESSION_STATEMENT
                .contains("The simulator profile is unchanged and stays synthetic")
        );
        assert!(MODEL_RECORD_IS_NOT_A_COMMAND_STATEMENT.contains("never authorizes"));
    }

    #[test]
    fn thermal_step_is_the_textbook_first_order_response() {
        let model = thermal_model();
        // HAND CHECK: tau = 2 h, rise = 10 C/MW, ambient = 20 C.
        // At P = 1 MW, steady state = 30 C. From 20 C after exactly one
        // tau (2 h): 30 + (20 - 30)·e^(-1) = 30 - 10/e ≈ 26.32120558828557.
        let temp = model
            .temperature_after(1.0, 20.0, 2.0)
            .expect("valid inputs");
        let expected = 30.0 - 10.0 / std::f64::consts::E;
        assert!((temp - expected).abs() < 1e-12, "got {temp}");
        // At zero elapsed time the temperature is the initial one.
        let temp = model
            .temperature_after(1.0, 20.0, 0.0)
            .expect("valid inputs");
        assert!((temp - 20.0).abs() < 1e-12);
        // At 10 tau the response is within 5e-5 of steady state.
        let temp = model
            .temperature_after(1.0, 20.0, 20.0)
            .expect("valid inputs");
        assert!((temp - 30.0).abs() < 5e-5);
        // Negative power (import/cooling-side convention of the caller)
        // travels verbatim: steady state at P = -1 MW is 10 C.
        let temp = model
            .temperature_after(-1.0, 20.0, 1e9)
            .expect("valid inputs");
        assert!((temp - 10.0).abs() < 1e-6);
    }

    #[test]
    fn thermal_model_refuses_non_positive_tau_and_non_finite_values() {
        let err = DeclaredThermalModel::new(
            asset(0xB001),
            param("tau_thermal_hours", 0.0),
            param("temp_rise_per_mw_c", 10.0),
            param("ambient_reference_c", 20.0),
        )
        .expect_err("zero tau refused");
        assert_eq!(
            err,
            ModelRecordError::NonPositiveTimeConstant {
                name: "tau_thermal_hours".to_string(),
                value: 0.0,
            }
        );
        let model = thermal_model();
        let err = model
            .temperature_after(f64::NAN, 20.0, 1.0)
            .expect_err("NaN power refused");
        assert_eq!(
            err,
            ModelRecordError::NonFiniteInput {
                name: "power_mw".to_string()
            }
        );
    }

    #[test]
    fn efficiency_curve_interpolates_linearly_and_refuses_outside() {
        // Declared curve: (0.0, 0.90), (0.5, 0.94), (1.0, 0.92).
        let curve = DeclaredEfficiencyCurve::new(
            asset(0xB001),
            vec![
                EfficiencyBreakpoint { power_fraction: 0.0, efficiency: 0.90 },
                EfficiencyBreakpoint { power_fraction: 0.5, efficiency: 0.94 },
                EfficiencyBreakpoint { power_fraction: 1.0, efficiency: 0.92 },
            ],
        )
        .expect("valid curve");
        // HAND CHECK: at 0.25 -> 0.90 + (0.04)·(0.25/0.5) = 0.92.
        let eta = curve.efficiency_at(0.25).expect("inside curve");
        assert!((eta - 0.92).abs() < 1e-12, "got {eta}");
        // At the breakpoints themselves: exact.
        assert!((curve.efficiency_at(0.5).unwrap() - 0.94).abs() < 1e-12);
        assert!((curve.efficiency_at(1.0).unwrap() - 0.92).abs() < 1e-12);
        // Below the first breakpoint: refused, never extrapolated.
        let err = curve.efficiency_at(-0.1).expect_err("below-curve refused");
        assert!(matches!(
            err,
            ModelRecordError::InterpolationOutsideCurve { .. }
        ));
        // Above the last: refused, never extrapolated.
        let err = curve.efficiency_at(1.1).expect_err("above-curve refused");
        assert_eq!(
            err,
            ModelRecordError::InterpolationOutsideCurve {
                requested: 1.1,
                declared_max: 1.0,
            }
        );
    }

    #[test]
    fn efficiency_curve_construction_validates() {
        let err = DeclaredEfficiencyCurve::new(
            asset(0xB001),
            vec![EfficiencyBreakpoint { power_fraction: 0.0, efficiency: 0.9 }],
        )
        .expect_err("one-point curve refused");
        assert_eq!(err, ModelRecordError::CurveTooShort { points: 1 });
        let err = DeclaredEfficiencyCurve::new(
            asset(0xB001),
            vec![
                EfficiencyBreakpoint { power_fraction: 0.5, efficiency: 0.9 },
                EfficiencyBreakpoint { power_fraction: 0.5, efficiency: 0.94 },
            ],
        )
        .expect_err("non-increasing axis refused");
        assert_eq!(err, ModelRecordError::CurveNotMonotonic { index: 1 });
        let err = DeclaredEfficiencyCurve::new(
            asset(0xB001),
            vec![
                EfficiencyBreakpoint { power_fraction: 0.0, efficiency: 1.2 },
                EfficiencyBreakpoint { power_fraction: 1.0, efficiency: 0.9 },
            ],
        )
        .expect_err("efficiency above 1 refused");
        assert_eq!(
            err,
            ModelRecordError::EfficiencyOutOfRange {
                name: "curve_efficiency".to_string(),
                value: 1.2,
            }
        );
    }

    #[test]
    fn constant_efficiency_and_parameter_validation() {
        let constant = DeclaredConstantEfficiency::new(
            asset(0xB001),
            param("eta_constant", 0.95),
        )
        .expect("valid constant efficiency");
        assert_eq!(constant.efficiency.value, 0.95);
        let err = DeclaredConstantEfficiency::new(asset(0xB001), param("eta", 0.0))
            .expect_err("zero efficiency refused");
        assert_eq!(
            err,
            ModelRecordError::EfficiencyOutOfRange {
                name: "eta".to_string(),
                value: 0.0,
            }
        );
        let err = DeclaredParameter::new(
            "p".to_string(),
            f64::INFINITY,
            Provenance::Configured,
            "s".to_string(),
        )
        .expect_err("infinite parameter refused");
        assert_eq!(
            err,
            ModelRecordError::NonFiniteParameter {
                name: "p".to_string()
            }
        );
        let err = DeclaredParameter::new(
            "p".to_string(),
            1.0,
            Provenance::Configured,
            "  ".to_string(),
        )
        .expect_err("empty source refused");
        assert_eq!(err, ModelRecordError::EmptySource);
    }

    #[test]
    fn every_model_record_error_renders_non_empty_prose() {
        let errors = [
            ModelRecordError::NonFiniteParameter { name: "p".to_string() },
            ModelRecordError::EfficiencyOutOfRange {
                name: "e".to_string(),
                value: 1.2,
            },
            ModelRecordError::NonPositiveTimeConstant {
                name: "t".to_string(),
                value: 0.0,
            },
            ModelRecordError::CurveTooShort { points: 1 },
            ModelRecordError::CurveNotMonotonic { index: 1 },
            ModelRecordError::CurveFractionOutOfRange { index: 0, value: 2.0 },
            ModelRecordError::NonFiniteInput { name: "x".to_string() },
            ModelRecordError::InterpolationOutsideCurve {
                requested: 1.5,
                declared_max: 1.0,
            },
            ModelRecordError::EmptySource,
        ];
        for error in errors {
            assert!(!error.to_string().is_empty());
        }
    }
}
