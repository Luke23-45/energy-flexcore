// P3.04 — Capability validation rules (schema, units, sign) (pure rules, no I/O).
//
// Implements ledger P3.04 ("Capability validation rules", report §11.1, §10.2,
// §19.2 test layer 1). Decision record:
// `ledger/phase-03-flexibility-scheduling/01_04_capability_contract.md`.
//
// Purpose: reject malformed capability data at the contract boundary BEFORE it
// can enter an envelope or a solve. Every rejection names the field, the
// offending value, and the rule — never a bare "invalid input" (ledger P3.04
// step 2).
//
// THE RULES, DECIDED ONCE (report §10.2, §11.2):
//
// - **Units**: a text-payload unit label must be one of the canonical units
//   `MW`, `MVAr`, `MVA`, `MWh` (§10.2 canonical set). `kW`/`kWh` — and any
//   other spelling — are rejected with a named error at the boundary; scaled
//   units are display-only and are converted at the adapter (§10.2 §5).
//   Note: the P3.01 schema's `ConstraintUnit` enum makes a scaled unit
//   unrepresentable in a typed model; this validator covers the text boundary
//   (adapter labels) before they map into the enum.
// - **Sign**: every active-power constraint asserts the frozen convention
//   **P > 0 = export at the point of connection** (§10.2). Any other declared
//   sign convention on an active-power constraint is a named error. Reactive
//   power must carry an explicitly DECLARED convention (injection-positive or
//   consumption-positive) — never a default, never inferred from the sign
//   alone (§10.2).
// - **SOC**: a fraction in [0,1] at the API boundary, **boundary-inclusive**:
//   exactly 0 and exactly 1 are valid (P3.04 risk note: over-strict
//   validation must not reject SOC 0 or 1). Delegated to the frozen
//   `StateOfCharge` contract type, whose range rule is the oracle.
// - **BESS magnitudes** (§11.2): `P_ch ≥ 0` and `P_dis ≥ 0` — these are
//   non-negative magnitudes, never signed powers.
// - **Energy window**: `E_min ≤ E_max` (§11.2), both finite.
// - **Ramp**: ramp rate ≥ 0 (§11.1), finite.
//
// Audit-safety (ledger P3.04 step 3): the error record is designed to be
// audit-visible — it carries only the field locator, the offending value, and
// the rule name. No internal paths, no secrets, no stack traces. Emitting the
// rejection as an audit event is the later audit/outbox wiring (§15.5); this
// module guarantees the message is safe to log.
//
// Layering with P3.01 (recorded): the contract enforces STRUCTURAL sanity
// (ordered bounds, non-empty sets, kind/unit compatibility). This module
// enforces the NORMATIVE boundary rules above, including the model-level walk
// ([`validate_capability_model`]) that checks every declared constraint's
// sign convention and SOC range.
//
// Control status: pure validation for the simulation-only 0.x line. No I/O,
// no device-write path.

use flexcore_contracts::capability::{
    CapabilityModel, ConstraintExpression, ConstraintKind, ConstraintSignConvention, ConstraintUnit,
};
use flexcore_contracts::units::StateOfCharge;

/// The canonical unit spellings accepted at the text boundary (report §10.2
/// canonical set). Everything else — including kW/kWh — is rejected with
/// [`CapabilityValidationRule::NonCanonicalUnit`].
pub const CANONICAL_CAPABILITY_UNITS: [&str; 4] = ["MW", "MVAr", "MVA", "MWh"];

/// The validation-rule catalogue. Each variant names ONE rule; the rule code
/// (`code`) and the human-readable rule text (`Display`) are frozen so test
/// fixtures and audit consumers can match on them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityValidationRule {
    /// The declared unit is not a canonical unit (§10.2). kW/kWh are the
    /// canonical examples of what this rule rejects.
    NonCanonicalUnit,
    /// A numeric value was NaN or infinite; canonical quantities must be
    /// finite.
    NonFiniteValue,
    /// An active-power constraint did not declare the frozen export-positive
    /// convention (§10.2: P > 0 means export at the point of connection).
    ActivePowerSignNotExport,
    /// A reactive-power constraint did not declare an explicit convention
    /// (§10.2: the Q sign is declared per grid model and device profile,
    /// never assumed and never inferred from the sign alone).
    ReactiveSignNotDeclared,
    /// A SOC fraction was outside [0,1]. Exactly 0 and exactly 1 are valid
    /// (boundary-inclusive).
    SocOutOfRange,
    /// A charge-power magnitude was negative (§11.2: P_ch ≥ 0).
    NegativeChargePower,
    /// A discharge-power magnitude was negative (§11.2: P_dis ≥ 0).
    NegativeDischargePower,
    /// The energy window was inverted (§11.2: E_min ≤ E_max).
    EnergyLimitsInverted,
    /// A ramp rate was negative (§11.1: ramp ≥ 0).
    NegativeRampRate,
}

impl CapabilityValidationRule {
    /// The frozen machine-readable rule code.
    pub const fn code(self) -> &'static str {
        match self {
            CapabilityValidationRule::NonCanonicalUnit => "UNIT_NOT_CANONICAL",
            CapabilityValidationRule::NonFiniteValue => "VALUE_NOT_FINITE",
            CapabilityValidationRule::ActivePowerSignNotExport => "ACTIVE_POWER_SIGN_NOT_EXPORT",
            CapabilityValidationRule::ReactiveSignNotDeclared => "REACTIVE_SIGN_NOT_DECLARED",
            CapabilityValidationRule::SocOutOfRange => "SOC_OUT_OF_RANGE",
            CapabilityValidationRule::NegativeChargePower => "NEGATIVE_CHARGE_POWER",
            CapabilityValidationRule::NegativeDischargePower => "NEGATIVE_DISCHARGE_POWER",
            CapabilityValidationRule::EnergyLimitsInverted => "ENERGY_LIMITS_INVERTED",
            CapabilityValidationRule::NegativeRampRate => "NEGATIVE_RAMP_RATE",
        }
    }
}

impl std::fmt::Display for CapabilityValidationRule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CapabilityValidationRule::NonCanonicalUnit => write!(
                f,
                "unit must be one of the canonical units MW, MVAr, MVA, MWh (kW/kWh and scaled units are display-only, report §10.2)"
            ),
            CapabilityValidationRule::NonFiniteValue => {
                write!(f, "value must be finite (no NaN or infinity)")
            }
            CapabilityValidationRule::ActivePowerSignNotExport => write!(
                f,
                "active power must declare the export-positive convention: P > 0 means export at the point of connection (report §10.2)"
            ),
            CapabilityValidationRule::ReactiveSignNotDeclared => write!(
                f,
                "reactive power must declare an explicit sign convention (injection-positive or consumption-positive); the Q sign is never assumed (report §10.2)"
            ),
            CapabilityValidationRule::SocOutOfRange => write!(
                f,
                "SOC must be a fraction in [0,1], boundary-inclusive (report §10.2)"
            ),
            CapabilityValidationRule::NegativeChargePower => {
                write!(f, "charge power P_ch must be >= 0 (report §11.2)")
            }
            CapabilityValidationRule::NegativeDischargePower => {
                write!(f, "discharge power P_dis must be >= 0 (report §11.2)")
            }
            CapabilityValidationRule::EnergyLimitsInverted => {
                write!(f, "energy limits must satisfy E_min <= E_max (report §11.2)")
            }
            CapabilityValidationRule::NegativeRampRate => {
                write!(f, "ramp rate must be >= 0 (report §11.1)")
            }
        }
    }
}

/// A structured capability-validation failure: the field locator, the
/// offending value, and the rule. Never a bare "invalid input" (ledger P3.04
/// step 2).
///
/// Audit-safe by construction (ledger P3.04 step 3): the record carries only
/// the field locator, the offending value, and the rule — no internal paths,
/// no secrets, no stack traces. The `Display` output is safe to log as an
/// audit-visible event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityValidationError {
    /// The field locator as declared in the capability schema (for example
    /// `p_ch_max`, `soc`, or `intervals[3].constraints[0].sign`).
    pub field: String,
    /// The offending value, rendered for audit display (finite numbers as
    /// numbers, unit labels and sign-convention names verbatim).
    pub offending_value: String,
    /// The violated rule.
    pub rule: CapabilityValidationRule,
}

impl std::fmt::Display for CapabilityValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "field '{}': value {} violates rule {} ({})",
            self.field,
            self.offending_value,
            self.rule.code(),
            self.rule
        )
    }
}

impl std::error::Error for CapabilityValidationError {}

fn reject(
    field: &str,
    offending_value: impl std::fmt::Display,
    rule: CapabilityValidationRule,
) -> CapabilityValidationError {
    CapabilityValidationError {
        field: field.to_string(),
        offending_value: offending_value.to_string(),
        rule,
    }
}

fn require_finite(field: &str, value: f64) -> Result<(), CapabilityValidationError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(reject(field, value, CapabilityValidationRule::NonFiniteValue))
    }
}

// --- The validators ---------------------------------------------------------

/// Validates a text-payload unit label against the canonical set (§10.2).
///
/// Accepts exactly `MW`, `MVAr`, `MVA`, `MWh`. Rejects `kW`, `kWh`, lower- or
/// mixed-case spellings, and anything else with
/// [`CapabilityValidationRule::NonCanonicalUnit`] — adapters normalize vendor
/// units before the boundary, they do not pass them through.
pub fn validate_unit(field: &str, unit: &str) -> Result<(), CapabilityValidationError> {
    if CANONICAL_CAPABILITY_UNITS.contains(&unit) {
        Ok(())
    } else {
        Err(reject(
            field,
            unit,
            CapabilityValidationRule::NonCanonicalUnit,
        ))
    }
}

/// Asserts the frozen active-power sign semantics (§10.2): an active-power
/// constraint must declare `EXPORT_POSITIVE` — **P > 0 means export at the
/// point of connection**. A sign-flipped declaration (injection-positive,
/// consumption-positive, magnitude, unsigned) is rejected with
/// [`CapabilityValidationRule::ActivePowerSignNotExport`].
pub fn validate_active_power_sign(
    field: &str,
    sign: ConstraintSignConvention,
) -> Result<(), CapabilityValidationError> {
    if sign == ConstraintSignConvention::ExportPositive {
        Ok(())
    } else {
        Err(reject(
            field,
            sign.name(),
            CapabilityValidationRule::ActivePowerSignNotExport,
        ))
    }
}

/// Asserts an explicit reactive-power sign declaration (§10.2): a
/// reactive-power constraint must declare `INJECTION_POSITIVE` or
/// `CONSUMPTION_POSITIVE` — the Q sign is declared per grid model and device
/// profile, never assumed and never inferred from the sign alone.
pub fn validate_reactive_power_sign(
    field: &str,
    sign: ConstraintSignConvention,
) -> Result<(), CapabilityValidationError> {
    if matches!(
        sign,
        ConstraintSignConvention::InjectionPositive | ConstraintSignConvention::ConsumptionPositive
    ) {
        Ok(())
    } else {
        Err(reject(
            field,
            sign.name(),
            CapabilityValidationRule::ReactiveSignNotDeclared,
        ))
    }
}

/// Validates a SOC fraction: a fraction in [0,1], **boundary-inclusive** —
/// exactly 0 and exactly 1 are valid (§10.2). Delegated to the frozen
/// `StateOfCharge` contract type; a non-finite value is reported as
/// [`CapabilityValidationRule::NonFiniteValue`], an out-of-range value as
/// [`CapabilityValidationRule::SocOutOfRange`].
pub fn validate_soc(field: &str, fraction: f64) -> Result<(), CapabilityValidationError> {
    if !fraction.is_finite() {
        return Err(reject(field, fraction, CapabilityValidationRule::NonFiniteValue));
    }
    StateOfCharge::new(fraction)
        .map(|_| ())
        .map_err(|_| reject(field, fraction, CapabilityValidationRule::SocOutOfRange))
}

/// Validates a BESS charge-power magnitude (§11.2): `P_ch ≥ 0`, finite.
/// Zero is a valid magnitude.
pub fn validate_charge_power(field: &str, p_ch: f64) -> Result<(), CapabilityValidationError> {
    require_finite(field, p_ch)?;
    if p_ch < 0.0 {
        return Err(reject(field, p_ch, CapabilityValidationRule::NegativeChargePower));
    }
    Ok(())
}

/// Validates a BESS discharge-power magnitude (§11.2): `P_dis ≥ 0`, finite.
/// Zero is a valid magnitude.
pub fn validate_discharge_power(field: &str, p_dis: f64) -> Result<(), CapabilityValidationError> {
    require_finite(field, p_dis)?;
    if p_dis < 0.0 {
        return Err(reject(field, p_dis, CapabilityValidationRule::NegativeDischargePower));
    }
    Ok(())
}

/// Validates an energy window (§11.2): both bounds finite and
/// `E_min ≤ E_max`. Equal bounds are a valid single-point window.
pub fn validate_energy_window(
    field: &str,
    e_min: f64,
    e_max: f64,
) -> Result<(), CapabilityValidationError> {
    require_finite(field, e_min)?;
    require_finite(field, e_max)?;
    if e_min > e_max {
        return Err(reject(
            field,
            format!("E_min = {e_min}, E_max = {e_max}"),
            CapabilityValidationRule::EnergyLimitsInverted,
        ));
    }
    Ok(())
}

/// Validates a ramp rate (§11.1): `ramp ≥ 0`, finite. Zero is a valid rate
/// (the P3.02 unknown-provenance default narrows ramping to exactly this).
pub fn validate_ramp_rate(field: &str, mw_per_min: f64) -> Result<(), CapabilityValidationError> {
    require_finite(field, mw_per_min)?;
    if mw_per_min < 0.0 {
        return Err(reject(field, mw_per_min, CapabilityValidationRule::NegativeRampRate));
    }
    Ok(())
}

/// Validates the full BESS capability boundary in one call (§11.2): SOC
/// window boundary-inclusive, `P_ch ≥ 0`, `P_dis ≥ 0`, `E_min ≤ E_max`,
/// ramp ≥ 0. Returns the first violation, named.
pub fn validate_bess_capability(
    soc: f64,
    p_ch_max: f64,
    p_dis_max: f64,
    e_min: f64,
    e_max: f64,
    ramp_mw_per_min: f64,
) -> Result<(), CapabilityValidationError> {
    validate_soc("soc", soc)?;
    validate_charge_power("p_ch_max", p_ch_max)?;
    validate_discharge_power("p_dis_max", p_dis_max)?;
    validate_energy_window("energy_window", e_min, e_max)?;
    validate_ramp_rate("ramp_mw_per_min", ramp_mw_per_min)
}

// --- Model-level walk -------------------------------------------------------

/// The expression bounds of an interval constraint, if the expression is an
/// interval.
fn interval_bounds(expression: &ConstraintExpression) -> Option<(Option<f64>, Option<f64>)> {
    match expression {
        ConstraintExpression::Interval { lower, upper } => Some((*lower, *upper)),
        _ => None,
    }
}

/// Validates a whole capability model at the boundary (ledger P3.04 step 1
/// applied to the P3.01 schema):
///
/// - every `ACTIVE_POWER` constraint declares the frozen export-positive
///   convention (§10.2);
/// - every `REACTIVE_POWER` constraint declares an explicit convention
///   (§10.2: never assumed);
/// - every `ENERGY_SOC` constraint in `SOC_FRACTION` units carries interval
///   bounds within [0,1], boundary-inclusive;
/// - every `RAMP_RATE` constraint carries a non-negative lower bound.
///
/// The field locator names the slice index, the constraint index, and the
/// checked aspect, so the rejection points at the offending constraint.
pub fn validate_capability_model(
    model: &CapabilityModel,
) -> Result<(), CapabilityValidationError> {
    for slice in &model.feasible_set.slices {
        for (position, constraint) in slice.constraints.iter().enumerate() {
            let field = format!(
                "intervals[{}].constraints[{}].{}",
                slice.interval_index, position, constraint.kind.name()
            );
            match constraint.kind {
                ConstraintKind::ActivePower => {
                    validate_active_power_sign(&field, constraint.sign)?;
                }
                ConstraintKind::ReactivePower => {
                    validate_reactive_power_sign(&field, constraint.sign)?;
                }
                ConstraintKind::EnergySoc if constraint.unit == ConstraintUnit::SocFraction => {
                    if let Some((lower, upper)) = interval_bounds(&constraint.expression) {
                        if let Some(lower) = lower {
                            validate_soc(&field, lower)?;
                        }
                        if let Some(upper) = upper {
                            validate_soc(&field, upper)?;
                        }
                    }
                }
                ConstraintKind::RampRate => {
                    if let Some((lower, _)) = interval_bounds(&constraint.expression) {
                        if let Some(lower) = lower {
                            validate_ramp_rate(&field, lower)?;
                        }
                    }
                }
                _ => {}
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use flexcore_contracts::capability::{
        CAPABILITY_SCHEMA_VERSION, DeclaredConstraint, Provenance, TimeIndexedFeasibleSet,
    };
    use flexcore_contracts::ids::FlexUuid;
    use flexcore_contracts::time_model::{ValidFrom, ValidUntil, ValidityWindow};

    fn window() -> ValidityWindow {
        ValidityWindow::new(
            ValidFrom::from_unix_millis(0).unwrap(),
            ValidUntil::from_unix_millis(3_600_000).unwrap(),
        )
        .unwrap()
    }

    fn asset() -> FlexUuid {
        FlexUuid::parse("0192a5e0-8c3d-7e2f-9a1b-3c4d5e6f7a8b").unwrap()
    }

    fn constraint(
        kind: ConstraintKind,
        unit: ConstraintUnit,
        sign: ConstraintSignConvention,
        expression: ConstraintExpression,
    ) -> DeclaredConstraint {
        DeclaredConstraint::new(kind, unit, sign, Provenance::Measured, window(), expression)
            .unwrap()
    }

    fn one_slice_model(constraints: Vec<DeclaredConstraint>) -> CapabilityModel {
        // The domain crate takes no `time` dependency: the UTC horizon start
        // is obtained through the validating contract constructors
        // (`ValidFrom::from_unix_millis(...).timestamp()`), never by naming
        // the timestamp type.
        let horizon_start = ValidFrom::from_unix_millis(0).unwrap().timestamp();
        let set = TimeIndexedFeasibleSet::new(
            horizon_start,
            900,
            1,
            vec![flexcore_contracts::capability::IntervalConstraints {
                interval_index: 0,
                constraints,
            }],
        )
        .unwrap();
        CapabilityModel::new(CAPABILITY_SCHEMA_VERSION, asset(), window(), set).unwrap()
    }

    // --- units ----------------------------------------------------------

    #[test]
    fn canonical_units_are_accepted() {
        assert_eq!(validate_unit("energy_unit", "MW"), Ok(()));
        assert_eq!(validate_unit("reactive_unit", "MVAr"), Ok(()));
        assert_eq!(validate_unit("apparent_unit", "MVA"), Ok(()));
        assert_eq!(validate_unit("energy_unit", "MWh"), Ok(()));
    }

    #[test]
    fn kw_and_kwh_are_rejected_with_the_named_rule() {
        for unit in ["kW", "kWh", "mw", "MVAR", "megawatt", ""] {
            let err = validate_unit("power_unit", unit).unwrap_err();
            assert_eq!(err.rule, CapabilityValidationRule::NonCanonicalUnit);
            assert_eq!(err.field, "power_unit");
            assert_eq!(err.offending_value, unit);
            assert_eq!(err.rule.code(), "UNIT_NOT_CANONICAL");
        }
    }

    // --- sign semantics ---------------------------------------------------

    #[test]
    fn export_positive_is_the_only_accepted_active_power_sign() {
        assert_eq!(
            validate_active_power_sign("p", ConstraintSignConvention::ExportPositive),
            Ok(())
        );
        // Sign-flip cases: every non-export declaration is rejected by name.
        for sign in [
            ConstraintSignConvention::InjectionPositive,
            ConstraintSignConvention::ConsumptionPositive,
            ConstraintSignConvention::NonNegativeMagnitude,
            ConstraintSignConvention::Unsigned,
        ] {
            let err =
                validate_active_power_sign("intervals[0].constraints[0].sign", sign).unwrap_err();
            assert_eq!(err.rule, CapabilityValidationRule::ActivePowerSignNotExport);
            assert_eq!(err.offending_value, sign.name());
            assert_eq!(err.rule.code(), "ACTIVE_POWER_SIGN_NOT_EXPORT");
        }
    }

    #[test]
    fn reactive_power_requires_an_explicit_declared_convention() {
        assert_eq!(
            validate_reactive_power_sign("q", ConstraintSignConvention::InjectionPositive),
            Ok(())
        );
        assert_eq!(
            validate_reactive_power_sign("q", ConstraintSignConvention::ConsumptionPositive),
            Ok(())
        );
        for sign in [
            ConstraintSignConvention::ExportPositive,
            ConstraintSignConvention::NonNegativeMagnitude,
            ConstraintSignConvention::Unsigned,
        ] {
            let err = validate_reactive_power_sign("q", sign).unwrap_err();
            assert_eq!(err.rule, CapabilityValidationRule::ReactiveSignNotDeclared);
        }
    }

    // --- SOC ------------------------------------------------------------

    #[test]
    fn soc_exactly_zero_and_exactly_one_are_accepted() {
        // Boundary-inclusive (P3.04 risk note): over-strict validation must
        // not reject SOC 0 or 1.
        assert_eq!(validate_soc("soc", 0.0), Ok(()));
        assert_eq!(validate_soc("soc", 1.0), Ok(()));
        assert_eq!(validate_soc("soc", 0.5), Ok(()));
    }

    #[test]
    fn soc_out_of_range_is_rejected_with_the_named_rule() {
        for fraction in [-0.01, 1.01, -1.0, 2.0, 100.0] {
            let err = validate_soc("soc", fraction).unwrap_err();
            assert_eq!(err.rule, CapabilityValidationRule::SocOutOfRange);
            assert_eq!(err.offending_value, fraction.to_string());
            assert_eq!(err.rule.code(), "SOC_OUT_OF_RANGE");
        }
        let err = validate_soc("soc", f64::NAN).unwrap_err();
        assert_eq!(err.rule, CapabilityValidationRule::NonFiniteValue);
    }

    // --- magnitudes, windows, ramp ---------------------------------------

    #[test]
    fn charge_and_discharge_magnitudes_must_be_non_negative() {
        assert_eq!(validate_charge_power("p_ch_max", 0.0), Ok(()));
        assert_eq!(validate_charge_power("p_ch_max", 2.5), Ok(()));
        assert_eq!(validate_discharge_power("p_dis_max", 0.0), Ok(()));
        assert_eq!(validate_discharge_power("p_dis_max", 2.5), Ok(()));

        let err = validate_charge_power("p_ch_max", -0.1).unwrap_err();
        assert_eq!(err.rule, CapabilityValidationRule::NegativeChargePower);
        assert_eq!(err.rule.code(), "NEGATIVE_CHARGE_POWER");
        let err = validate_discharge_power("p_dis_max", -1.0).unwrap_err();
        assert_eq!(err.rule, CapabilityValidationRule::NegativeDischargePower);
        assert_eq!(err.offending_value, "-1");
    }

    #[test]
    fn energy_window_rejects_inverted_min_max() {
        assert_eq!(validate_energy_window("energy_window", 0.0, 10.0), Ok(()));
        // Equal bounds are a valid single-point window.
        assert_eq!(validate_energy_window("energy_window", 5.0, 5.0), Ok(()));

        let err = validate_energy_window("energy_window", 10.0, 5.0).unwrap_err();
        assert_eq!(err.rule, CapabilityValidationRule::EnergyLimitsInverted);
        assert_eq!(err.rule.code(), "ENERGY_LIMITS_INVERTED");
        assert!(err.offending_value.contains("E_min = 10"));
        assert!(err.offending_value.contains("E_max = 5"));
        assert_eq!(err.field, "energy_window");
    }

    #[test]
    fn ramp_rate_must_be_non_negative() {
        assert_eq!(validate_ramp_rate("ramp_mw_per_min", 0.0), Ok(()));
        assert_eq!(validate_ramp_rate("ramp_mw_per_min", 1.5), Ok(()));

        let err = validate_ramp_rate("ramp_mw_per_min", -2.0).unwrap_err();
        assert_eq!(err.rule, CapabilityValidationRule::NegativeRampRate);
        assert_eq!(err.rule.code(), "NEGATIVE_RAMP_RATE");
        assert_eq!(err.offending_value, "-2");
    }

    #[test]
    fn non_finite_values_are_rejected_everywhere() {
        assert_eq!(
            validate_charge_power("p_ch_max", f64::NAN).unwrap_err().rule,
            CapabilityValidationRule::NonFiniteValue
        );
        assert_eq!(
            validate_discharge_power("p_dis_max", f64::INFINITY).unwrap_err().rule,
            CapabilityValidationRule::NonFiniteValue
        );
        assert_eq!(
            validate_energy_window("energy_window", f64::NAN, 1.0)
                .unwrap_err()
                .rule,
            CapabilityValidationRule::NonFiniteValue
        );
        assert_eq!(
            validate_ramp_rate("ramp_mw_per_min", f64::NEG_INFINITY)
                .unwrap_err()
                .rule,
            CapabilityValidationRule::NonFiniteValue
        );
    }

    #[test]
    fn bess_boundary_accepts_valid_boundary_inputs() {
        // SOC exactly 0 / exactly 1, zero powers, single-point energy window,
        // zero ramp: all boundary values valid (P3.04 risk note).
        assert_eq!(validate_bess_capability(0.0, 0.0, 0.0, 0.0, 0.0, 0.0), Ok(()));
        assert_eq!(validate_bess_capability(1.0, 2.0, 2.0, 1.0, 10.0, 0.5), Ok(()));
    }

    #[test]
    fn bess_boundary_reports_the_first_violation_named() {
        let err = validate_bess_capability(1.5, 2.0, 2.0, 1.0, 10.0, 0.5).unwrap_err();
        assert_eq!(err.rule, CapabilityValidationRule::SocOutOfRange);
        assert_eq!(err.field, "soc");

        let err = validate_bess_capability(0.5, -1.0, 2.0, 1.0, 10.0, 0.5).unwrap_err();
        assert_eq!(err.rule, CapabilityValidationRule::NegativeChargePower);
        assert_eq!(err.field, "p_ch_max");
    }

    // --- model-level walk -------------------------------------------------

    #[test]
    fn model_walk_accepts_a_valid_model() {
        let model = one_slice_model(vec![
            constraint(
                ConstraintKind::ActivePower,
                ConstraintUnit::Megawatt,
                ConstraintSignConvention::ExportPositive,
                ConstraintExpression::Interval { lower: Some(-2.0), upper: Some(3.0) },
            ),
            constraint(
                ConstraintKind::ReactivePower,
                ConstraintUnit::Megavar,
                ConstraintSignConvention::ConsumptionPositive,
                ConstraintExpression::Interval { lower: Some(-1.0), upper: Some(1.0) },
            ),
            constraint(
                ConstraintKind::EnergySoc,
                ConstraintUnit::SocFraction,
                ConstraintSignConvention::Unsigned,
                ConstraintExpression::Interval { lower: Some(0.0), upper: Some(1.0) },
            ),
            constraint(
                ConstraintKind::RampRate,
                ConstraintUnit::MegawattPerMinute,
                ConstraintSignConvention::NonNegativeMagnitude,
                ConstraintExpression::Interval { lower: Some(0.0), upper: Some(5.0) },
            ),
        ]);
        assert_eq!(validate_capability_model(&model), Ok(()));
    }

    #[test]
    fn model_walk_rejects_a_sign_flipped_active_power_constraint() {
        let model = one_slice_model(vec![constraint(
            ConstraintKind::ActivePower,
            ConstraintUnit::Megawatt,
            ConstraintSignConvention::InjectionPositive,
            ConstraintExpression::Interval { lower: Some(-2.0), upper: Some(3.0) },
        )]);
        let err = validate_capability_model(&model).unwrap_err();
        assert_eq!(err.rule, CapabilityValidationRule::ActivePowerSignNotExport);
        assert_eq!(
            err.field,
            "intervals[0].constraints[0].ACTIVE_POWER"
        );
        assert_eq!(err.offending_value, "INJECTION_POSITIVE");
    }

    #[test]
    fn model_walk_rejects_an_undeclared_reactive_sign() {
        let model = one_slice_model(vec![constraint(
            ConstraintKind::ReactivePower,
            ConstraintUnit::Megavar,
            ConstraintSignConvention::ExportPositive,
            ConstraintExpression::Interval { lower: Some(-1.0), upper: Some(1.0) },
        )]);
        let err = validate_capability_model(&model).unwrap_err();
        assert_eq!(err.rule, CapabilityValidationRule::ReactiveSignNotDeclared);
        assert_eq!(err.field, "intervals[0].constraints[0].REACTIVE_POWER");
    }

    #[test]
    fn model_walk_rejects_an_out_of_range_soc_bound() {
        let model = one_slice_model(vec![constraint(
            ConstraintKind::EnergySoc,
            ConstraintUnit::SocFraction,
            ConstraintSignConvention::Unsigned,
            ConstraintExpression::Interval { lower: Some(0.0), upper: Some(1.5) },
        )]);
        let err = validate_capability_model(&model).unwrap_err();
        assert_eq!(err.rule, CapabilityValidationRule::SocOutOfRange);
        assert_eq!(err.offending_value, "1.5");
    }

    #[test]
    fn model_walk_rejects_a_negative_ramp_lower_bound() {
        let model = one_slice_model(vec![constraint(
            ConstraintKind::RampRate,
            ConstraintUnit::MegawattPerMinute,
            ConstraintSignConvention::NonNegativeMagnitude,
            ConstraintExpression::Interval { lower: Some(-1.0), upper: Some(5.0) },
        )]);
        let err = validate_capability_model(&model).unwrap_err();
        assert_eq!(err.rule, CapabilityValidationRule::NegativeRampRate);
        assert_eq!(err.offending_value, "-1");
    }

    // --- error catalogue and audit safety ---------------------------------

    #[test]
    fn every_rule_has_a_frozen_code_and_message() {
        let rules = [
            CapabilityValidationRule::NonCanonicalUnit,
            CapabilityValidationRule::NonFiniteValue,
            CapabilityValidationRule::ActivePowerSignNotExport,
            CapabilityValidationRule::ReactiveSignNotDeclared,
            CapabilityValidationRule::SocOutOfRange,
            CapabilityValidationRule::NegativeChargePower,
            CapabilityValidationRule::NegativeDischargePower,
            CapabilityValidationRule::EnergyLimitsInverted,
            CapabilityValidationRule::NegativeRampRate,
        ];
        for rule in rules {
            assert!(!rule.code().is_empty());
            assert!(!rule.to_string().is_empty());
            assert!(rule.code().chars().all(|c| c.is_ascii_uppercase() || c == '_'));
        }
    }

    #[test]
    fn error_display_names_field_value_and_rule() {
        let err = CapabilityValidationError {
            field: "intervals[2].constraints[1].RAMP_RATE".to_string(),
            offending_value: "-3.5".to_string(),
            rule: CapabilityValidationRule::NegativeRampRate,
        };
        let display = err.to_string();
        assert!(display.contains("intervals[2].constraints[1].RAMP_RATE"));
        assert!(display.contains("-3.5"));
        assert!(display.contains("NEGATIVE_RAMP_RATE"));
        assert!(display.contains("ramp rate must be >= 0"));
    }

    #[test]
    fn error_messages_are_audit_safe() {
        // Audit-safety (ledger P3.04 step 3): the message names the field, the
        // value, and the rule — and carries no internal path separators that
        // could leak filesystem or namespace structure.
        let err = CapabilityValidationError {
            field: "intervals[0].constraints[0].ACTIVE_POWER".to_string(),
            offending_value: "CONSUMPTION_POSITIVE".to_string(),
            rule: CapabilityValidationRule::ActivePowerSignNotExport,
        };
        let display = err.to_string();
        assert!(!display.contains('\\'), "no filesystem path separators");
        assert!(!display.contains(".rs"), "no source file references");
        assert!(!display.contains("C:"), "no drive paths");
    }
}
