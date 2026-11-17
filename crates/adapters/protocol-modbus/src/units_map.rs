// P14.07 — Unit conversion at the adapter boundary.
//
// Implements the contracts rule (crates/flexcore-contracts/src/units.rs;
// report §10.2, foundations §6.2): canonical units are MW, MVAr, MVA, MWh;
// scaled source units (W, VAr, Wh, %) are converted HERE, at the adapter —
// never in a contract, never in a domain model.
//
// The reactive-power rule is the load-bearing one: the numeric sign of a
// VAr reading has NO universal meaning. Each device profile declares
// whether Q > 0 means injection or consumption; the converter REQUIRES the
// declared convention as an argument — there is no default, no overload
// without it, and the direction is computed only through
// `interpret_reactive_power` (never from the sign alone).

use flexcore_contracts::units::{
    interpret_reactive_power, ActivePowerMW, EnergyMWh, ReactivePowerDirection,
    ReactivePowerMVAr, ReactiveSignConvention, StateOfCharge, UnitsError,
};

use crate::model::DecodedField;

/// Errors returned by the adapter-boundary conversion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConversionError {
    /// The field's unit kind cannot produce the requested canonical type
    /// (e.g. volts → MW). Refused, never coerced.
    UnitKindMismatch {
        label: String,
        unit_kind: &'static str,
    },
    /// The contracts unit constructor refused the converted value
    /// (non-finite; SOC outside [0,1]).
    Units(UnitsError),
}

impl std::fmt::Display for ConversionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConversionError::UnitKindMismatch { label, unit_kind } => {
                write!(
                    f, "field '{label}' carries unit kind '{unit_kind}', which cannot produce the requested canonical type"
                )
            }
            ConversionError::Units(err) => write!(f, "canonical unit construction refused: {err}"),
        }
    }
}

impl std::error::Error for ConversionError {}

impl From<UnitsError> for ConversionError {
    fn from(err: UnitsError) -> Self {
        ConversionError::Units(err)
    }
}

/// Converts a decoded field carrying [`crate::model::RegisterUnitKind::
/// Watts`] into the canonical [`ActivePowerMW`]. W → MW is /1,000,000;
/// the frozen sign rule (P > 0 = export at the point of connection) is the
/// contracts type's own — the adapter does not flip signs.
pub fn watts_to_active_power(field: &DecodedField) -> Result<ActivePowerMW, ConversionError> {
    if field.unit_kind != crate::model::RegisterUnitKind::Watts {
        return Err(ConversionError::UnitKindMismatch {
            label: field.label.clone(),
            unit_kind: field.unit_kind.name(),
        });
    }
    Ok(ActivePowerMW::new(field.value / 1_000_000.0)?)
}

/// Converts a decoded field carrying [`crate::model::RegisterUnitKind::
/// Var`] into the canonical [`ReactivePowerMVAr`]. VAr → MVAr is /1,000,000.
///
/// The SIGN of the numeric value is NOT interpreted here. The caller must
/// supply the device profile's DECLARED
/// [`ReactiveSignConvention`] (units.rs: declared per grid model and
/// device profile, never inferred); the direction is computed only through
/// `interpret_reactive_power`. There is deliberately no convention-free
/// overload.
pub fn var_to_reactive_power(
    field: &DecodedField,
    declared_convention: ReactiveSignConvention,
) -> Result<(ReactivePowerMVAr, ReactivePowerDirection), ConversionError> {
    if field.unit_kind != crate::model::RegisterUnitKind::Var {
        return Err(ConversionError::UnitKindMismatch {
            label: field.label.clone(),
            unit_kind: field.unit_kind.name(),
        });
    }
    let q = ReactivePowerMVAr::new(field.value / 1_000_000.0)?;
    Ok((q, interpret_reactive_power(declared_convention, q)))
}

/// Converts a decoded field carrying [`crate::model::RegisterUnitKind::
/// WattHours`] into the canonical [`EnergyMWh`]. Wh → MWh is /1,000,000.
pub fn watt_hours_to_energy(field: &DecodedField) -> Result<EnergyMWh, ConversionError> {
    if field.unit_kind != crate::model::RegisterUnitKind::WattHours {
        return Err(ConversionError::UnitKindMismatch {
            label: field.label.clone(),
            unit_kind: field.unit_kind.name(),
        });
    }
    Ok(EnergyMWh::new(field.value / 1_000_000.0)?)
}

/// Converts a decoded field carrying [`crate::model::RegisterUnitKind::
/// Percent`] into the canonical SOC fraction, through the explicitly named
/// `from_percent` boundary constructor (units.rs: a percent value is never
/// persisted as canonical SOC).
pub fn percent_to_soc(field: &DecodedField) -> Result<StateOfCharge, ConversionError> {
    if field.unit_kind != crate::model::RegisterUnitKind::Percent {
        return Err(ConversionError::UnitKindMismatch {
            label: field.label.clone(),
            unit_kind: field.unit_kind.name(),
        });
    }
    Ok(StateOfCharge::from_percent(field.value)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{RegisterUnitKind, SunSpecModel};

    fn field_of(kind: RegisterUnitKind, label: &str, value: f64) -> DecodedField {
        DecodedField::new(SunSpecModel::InverterThreePhase, label.to_string(), kind, value)
            .expect("valid field")
    }

    #[test]
    fn watts_convert_to_canonical_mw() {
        let mw = watts_to_active_power(&field_of(RegisterUnitKind::Watts, "W", 1_500_000.0))
            .expect("converts");
        assert_eq!(mw.value(), 1.5);
        assert!(mw.value() > 0.0, "P > 0 = export (frozen rule)");
        let import = watts_to_active_power(&field_of(RegisterUnitKind::Watts, "W", -250_000.0))
            .expect("converts");
        assert_eq!(import.value(), -0.25);
    }

    #[test]
    fn var_conversion_requires_the_declared_convention() {
        let field = field_of(RegisterUnitKind::Var, "VAr", 2_000_000.0);
        // Injection-positive: +2 MVAr is injection.
        let (q, direction) =
            var_to_reactive_power(&field, ReactiveSignConvention::InjectionPositive).expect("v");
        assert_eq!(q.value(), 2.0);
        assert_eq!(direction, ReactivePowerDirection::Injection);
        // The SAME numeric value under consumption-positive is consumption:
        // the sign is never a universal constant.
        let (_, direction) =
            var_to_reactive_power(&field, ReactiveSignConvention::ConsumptionPositive).expect("v");
        assert_eq!(direction, ReactivePowerDirection::Consumption);
    }

    #[test]
    fn watt_hours_convert_to_canonical_mwh() {
        let mwh = watt_hours_to_energy(&field_of(RegisterUnitKind::WattHours, "WH", 500_000.0))
            .expect("converts");
        assert_eq!(mwh.value(), 0.5);
    }

    #[test]
    fn percent_converts_through_the_named_soc_boundary_constructor() {
        let soc = percent_to_soc(&field_of(RegisterUnitKind::Percent, "SOC", 73.0)).expect("v");
        assert_eq!(soc.value(), 0.73);
        // Out-of-range percent is refused by the contracts constructor.
        assert!(matches!(
            percent_to_soc(&field_of(RegisterUnitKind::Percent, "SOC", 101.0)),
            Err(ConversionError::Units(UnitsError::OutOfRange))
        ));
    }

    #[test]
    fn unit_kind_mismatches_are_refused_never_coerced() {
        assert_eq!(
            watts_to_active_power(&field_of(RegisterUnitKind::Volts, "V", 400.0)),
            Err(ConversionError::UnitKindMismatch {
                label: "V".to_string(),
                unit_kind: "volts"
            })
        );
        assert_eq!(
            percent_to_soc(&field_of(RegisterUnitKind::Watts, "W", 1.0)),
            Err(ConversionError::UnitKindMismatch {
                label: "W".to_string(),
                unit_kind: "watts"
            })
        );
    }
}
