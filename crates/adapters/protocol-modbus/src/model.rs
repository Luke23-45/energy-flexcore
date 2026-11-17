// P14.07 — SunSpec information-model types.
//
// UNVERIFIED throughout (secondary source [S-16], sunspec.org/modbus): the
// model ids, the common-block marker, block layout, scale-factor rule
// (value × 10^SF), 32-bit big-endian register pairs, and the meter sign
// conventions must be checked against the SunSpec Modbus specification and
// a lab device at implementation time (plan §7 note 6). These types are
// the reviewed SHAPE of what will be checked, not a claim that any device
// was read.

use serde::{Deserialize, Serialize};

/// The SunSpec common-block marker: the first register pair of a SunSpec
/// device map. UNVERIFIED: the 0x53756E53 ("SunS") u32 marker value and
/// the meaning of the following registers (device id, address of the first
/// model) are checked against the SunSpec Modbus specification at
/// implementation.
pub const SUNSPEC_MARKER: u32 = 0x5375_6E53;

/// The SunSpec model families this client design covers. Exactly the
/// families the phase names (101/103 inverters, 201/203 meters, 80x
/// battery models); anything else is outside the declared scope
/// ([`crate::scope`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SunSpecModel {
    /// Single-phase inverter information model (UNVERIFIED id/semantics).
    InverterSinglePhase,
    /// Three-phase inverter information model (UNVERIFIED id/semantics).
    InverterThreePhase,
    /// Single-phase meter information model (UNVERIFIED id/semantics).
    MeterSinglePhase,
    /// Three-phase meter information model (UNVERIFIED id/semantics).
    MeterThreePhase,
    /// Battery base information model (UNVERIFIED id/semantics).
    BatteryBase,
    /// Battery status information model (UNVERIFIED id/semantics).
    BatteryStatus,
    /// Battery nameplate information model (UNVERIFIED id/semantics).
    BatteryNameplate,
    /// Battery capacity information model (UNVERIFIED id/semantics).
    BatteryCapacity,
}

impl SunSpecModel {
    /// The SunSpec model id this family maps to. UNVERIFIED against
    /// [S-16]: the numeric ids (101, 103, 201, 203, 801, 802, 804, 805)
    /// follow the secondary-source reading and MUST be checked against the
    /// SunSpec model list at implementation time; a mismatch is a recorded
    /// edit here.
    pub const fn model_id(self) -> u16 {
        match self {
            SunSpecModel::InverterSinglePhase => 101,
            SunSpecModel::InverterThreePhase => 103,
            SunSpecModel::MeterSinglePhase => 201,
            SunSpecModel::MeterThreePhase => 203,
            SunSpecModel::BatteryBase => 801,
            SunSpecModel::BatteryStatus => 802,
            SunSpecModel::BatteryNameplate => 804,
            SunSpecModel::BatteryCapacity => 805,
        }
    }

    /// Canonical name for records and error messages.
    pub const fn name(self) -> &'static str {
        match self {
            SunSpecModel::InverterSinglePhase => "inverter_single_phase",
            SunSpecModel::InverterThreePhase => "inverter_three_phase",
            SunSpecModel::MeterSinglePhase => "meter_single_phase",
            SunSpecModel::MeterThreePhase => "meter_three_phase",
            SunSpecModel::BatteryBase => "battery_base",
            SunSpecModel::BatteryStatus => "battery_status",
            SunSpecModel::BatteryNameplate => "battery_nameplate",
            SunSpecModel::BatteryCapacity => "battery_capacity",
        }
    }

    /// All declared model families (deterministic order).
    pub const ALL: [SunSpecModel; 8] = [
        SunSpecModel::InverterSinglePhase,
        SunSpecModel::InverterThreePhase,
        SunSpecModel::MeterSinglePhase,
        SunSpecModel::MeterThreePhase,
        SunSpecModel::BatteryBase,
        SunSpecModel::BatteryStatus,
        SunSpecModel::BatteryNameplate,
        SunSpecModel::BatteryCapacity,
    ];
}

/// The register data type of one field. UNVERIFIED: SunSpec 16-bit
/// signed/unsigned, big-endian 32-bit pairs, and bitfield16 types follow
/// the secondary-source reading [S-16].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RegisterType {
    /// Unsigned 16-bit.
    U16,
    /// Signed 16-bit (two's complement; e.g. negative W for import).
    I16,
    /// Unsigned 32-bit, big-endian register pair.
    U32,
    /// Signed 32-bit, big-endian register pair.
    I32,
    /// 16-bit bitfield (status words).
    Bitfield16,
}

impl RegisterType {
    /// The number of 16-bit registers this type occupies.
    pub const fn register_count(self) -> u16 {
        match self {
            RegisterType::U16 | RegisterType::I16 | RegisterType::Bitfield16 => 1,
            RegisterType::U32 | RegisterType::I32 => 2,
        }
    }
}

/// The physical unit kind a decoded field carries, BEFORE adapter-boundary
/// conversion ([`crate::units_map`] turns these into canonical contract
/// types). Scaled-unit rule (contracts units.rs): scaled units exist only
/// at the source; conversion happens here, at the adapter, never in a
/// contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RegisterUnitKind {
    /// Active power in watts (W) — converted to MW.
    Watts,
    /// Reactive power in volt-amperes-reactive (VAr) — converted to MVAr
    /// under the DECLARED sign convention (no default).
    Var,
    /// Energy in watt-hours (Wh) — converted to MWh.
    WattHours,
    /// Voltage in volts (V) — kept as display-only metadata in this
    /// design (canonical voltage types are a grid-model concern).
    Volts,
    /// Current in amperes (A) — display-only in this design.
    Amperes,
    /// Frequency in centi-Hertz (cHz) — UNVERIFIED (SunSpec models carry
    /// Hz in 0.01 Hz steps); display-only in this design.
    CentiHertz,
    /// A percentage (state of charge, state of health) — converted to the
    /// canonical SOC fraction via the explicitly named `from_percent`
    /// boundary constructor.
    Percent,
    /// Temperature in degrees Celsius — display-only in this design.
    Celsius,
    /// A dimensionless enum/bitfield value (operating state, error code).
    Enumerated,
}

impl RegisterUnitKind {
    /// Canonical name for records and error messages.
    pub const fn name(self) -> &'static str {
        match self {
            RegisterUnitKind::Watts => "watts",
            RegisterUnitKind::Var => "var",
            RegisterUnitKind::WattHours => "watt_hours",
            RegisterUnitKind::Volts => "volts",
            RegisterUnitKind::Amperes => "amperes",
            RegisterUnitKind::CentiHertz => "centi_hertz",
            RegisterUnitKind::Percent => "percent",
            RegisterUnitKind::Celsius => "celsius",
            RegisterUnitKind::Enumerated => "enumerated",
        }
    }
}

/// One decoded field value: still in SOURCE terms (unit kind, raw numeric
/// value AFTER scale-factor application but BEFORE canonical-unit
/// conversion). This is the ACL input — the translation to canonical
/// contract types happens once, in [`crate::units_map`] (the
/// translation-at-boundary pattern, phase 14 context §2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DecodedField {
    /// The SunSpec model family the field belongs to.
    pub model: SunSpecModel,
    /// The field's stable label (e.g. `"W"`, `"WH"`, `"SOC"` — labels are
    /// this registry's own; the mapping to SunSpec's official point names
    /// is UNVERIFIED and lives in the register map, reviewed per device
    /// model).
    pub label: String,
    /// The unit kind before conversion.
    pub unit_kind: RegisterUnitKind,
    /// The decoded value (scale factor already applied by the decoder).
    pub value: f64,
}

impl DecodedField {
    /// Validates a decoded field (finite value; non-empty label).
    pub fn new(
        model: SunSpecModel,
        label: String,
        unit_kind: RegisterUnitKind,
        value: f64,
    ) -> Result<Self, ModelError> {
        if label.trim().is_empty() {
            return Err(ModelError::EmptyField("label"));
        }
        if !value.is_finite() {
            return Err(ModelError::NonFiniteValue);
        }
        Ok(Self {
            model,
            label,
            unit_kind,
            value,
        })
    }
}

/// Errors returned by the model layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelError {
    /// A required field is empty.
    EmptyField(&'static str),
    /// A decoded value is NaN or infinite; source values must be finite
    /// (the transport must never surface NaN — a NaN is a named refusal).
    NonFiniteValue,
    /// The device did not present the SunSpec common-block marker; the
    /// device map is not SunSpec-shaped and the adapter refuses instead of
    /// guessing a layout.
    NotSunSpecShaped,
    /// A declared scale-factor register is out of the read window, or its
    /// value is outside the plausibility band this design accepts
    /// ([-10, +10]; UNVERIFIED plausibility choice, recorded here).
    ImplausibleScaleFactor { label: String, sf: i16 },
}

impl std::fmt::Display for ModelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModelError::EmptyField(field) => write!(f, "SunSpec model field '{field}' is empty"),
            ModelError::NonFiniteValue => {
                write!(f, "decoded value is not finite; refusing instead of surfacing NaN")
            }
            ModelError::NotSunSpecShaped => {
                write!(f, "device did not present the SunSpec common-block marker; refusing to guess a register layout")
            }
            ModelError::ImplausibleScaleFactor { label, sf } => {
                write!(
                    f, "scale factor {sf} for field '{label}' is outside the plausibility band [-10, +10]; refusing"
                )
            }
        }
    }
}

impl std::error::Error for ModelError {}

/// Applies a scale factor to a raw register value: `value × 10^SF`
/// (UNVERIFIED rule — checked against [S-16]). Scale factors outside the
/// plausibility band [-10, +10] are refused: a bad SF register read
/// (offset desync, wrong map) must never silently rescale a fleet reading.
pub fn apply_scale_factor(label: &str, raw: f64, sf: i16) -> Result<f64, ModelError> {
    if !(-10..=10).contains(&sf) {
        return Err(ModelError::ImplausibleScaleFactor { label: label.to_string(), sf });
    }
    let scaled = raw * 10f64.powi(i32::from(sf));
    if !scaled.is_finite() {
        return Err(ModelError::NonFiniteValue);
    }
    Ok(scaled)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_ids_match_the_declared_secondary_source_reading() {
        // UNVERIFIED values pinned so a spec check that disagrees has
        // exactly one place to edit.
        assert_eq!(SunSpecModel::InverterSinglePhase.model_id(), 101);
        assert_eq!(SunSpecModel::InverterThreePhase.model_id(), 103);
        assert_eq!(SunSpecModel::MeterSinglePhase.model_id(), 201);
        assert_eq!(SunSpecModel::MeterThreePhase.model_id(), 203);
        assert_eq!(SunSpecModel::BatteryBase.model_id(), 801);
        assert_eq!(SunSpecModel::BatteryStatus.model_id(), 802);
        assert_eq!(SunSpecModel::BatteryNameplate.model_id(), 804);
        assert_eq!(SunSpecModel::BatteryCapacity.model_id(), 805);
    }

    #[test]
    fn register_type_counts_are_one_or_two_registers() {
        assert_eq!(RegisterType::U16.register_count(), 1);
        assert_eq!(RegisterType::I16.register_count(), 1);
        assert_eq!(RegisterType::Bitfield16.register_count(), 1);
        assert_eq!(RegisterType::U32.register_count(), 2);
        assert_eq!(RegisterType::I32.register_count(), 2);
    }

    #[test]
    fn scale_factor_rule_is_value_times_ten_pow_sf() {
        assert_eq!(apply_scale_factor("W", 1_500.0, 0).expect("sf 0"), 1_500.0);
        assert_eq!(apply_scale_factor("W", 1_500.0, 1).expect("sf 1"), 15_000.0);
        assert_eq!(apply_scale_factor("SOC", 50.0, -1).expect("sf -1"), 5.0);
    }

    #[test]
    fn implausible_scale_factors_are_refused() {
        assert_eq!(
            apply_scale_factor("W", 1.0, 11),
            Err(ModelError::ImplausibleScaleFactor { label: "W".to_string(), sf: 11 })
        );
        assert_eq!(
            apply_scale_factor("W", 1.0, -11),
            Err(ModelError::ImplausibleScaleFactor { label: "W".to_string(), sf: -11 })
        );
    }

    #[test]
    fn decoded_fields_reject_empty_labels_and_non_finite_values() {
        assert_eq!(
            DecodedField::new(SunSpecModel::BatteryBase, String::new(), RegisterUnitKind::Percent, 0.5),
            Err(ModelError::EmptyField("label"))
        );
        assert_eq!(
            DecodedField::new(SunSpecModel::BatteryBase, "SOC".to_string(), RegisterUnitKind::Percent, f64::NAN),
            Err(ModelError::NonFiniteValue)
        );
    }
}
