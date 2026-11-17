// P14.08 — IEEE 1815.2-2025 DER-profile data-object model types.
//
// UNVERIFIED anchors (secondary sources [S-14][S-15]; plan §7 note 6):
// every object GROUP and VARIATION number, the event-class numbering, and
// the control-operation semantics below follow the secondary-source
// reading of the DNP3 object library and the IEEE 1815.2 DER profile.
// The implementation-time check — against the IEEE 1815.2-2025 standard
// text and, where possible, the utility's point list — must confirm each
// number before any wire-facing design consumes it. A mismatch is a
// recorded edit HERE (one place), never a silent reinterpretation.
//
// Scope notes: model types only — no framing, no link layer, no serial/TCP
// I/O. The quantities the platform exchanges follow the frozen contracts
// unit rule: values cross into this crate as canonical types
// (`ActivePowerMW`, `StateOfCharge`) and OUT as source-scaled points
// declared per profile.

use serde::{Deserialize, Serialize};

/// The DNP3 object groups the DER-profile design exchanges (UNVERIFIED
/// numbers — see the module header).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Dnp3ObjectGroup {
    /// Binary input (e.g. breaker status, availability flag).
    /// UNVERIFIED: group 1, variation 2.
    BinaryInput,
    /// Analog input, 32-bit with flag. UNVERIFIED: group 30, variation 1
    /// (scaling per the utility point list — declared per profile, never
    /// assumed).
    AnalogInput,
    /// Counter (e.g. energy accumulator). UNVERIFIED: group 20.
    Counter,
    /// Analog output status (the feedback point mirroring a commanded
    /// analog output). UNVERIFIED: group 40.
    AnalogOutputStatus,
    /// Analog output (the setpoint-carrying object). UNVERIFIED: group 41.
    AnalogOutput,
    /// Binary command / control output. UNVERIFIED: group 12 (control
    /// output block with QualifierCode and FunctionCode, and the
    /// select-before-operate arming sequence — semantics UNVERIFIED).
    ControlOutput,
}

impl Dnp3ObjectGroup {
    /// The UNVERIFIED group number this design maps the object to. Pinned
    /// in ONE place so a standard-text check that disagrees has exactly
    /// one edit site.
    pub const fn group_number(self) -> u8 {
        match self {
            Dnp3ObjectGroup::BinaryInput => 1,
            Dnp3ObjectGroup::Counter => 20,
            Dnp3ObjectGroup::AnalogInput => 30,
            Dnp3ObjectGroup::AnalogOutputStatus => 40,
            Dnp3ObjectGroup::AnalogOutput => 41,
            Dnp3ObjectGroup::ControlOutput => 12,
        }
    }

    /// Canonical name for records and error messages.
    pub const fn name(self) -> &'static str {
        match self {
            Dnp3ObjectGroup::BinaryInput => "binary_input",
            Dnp3ObjectGroup::Counter => "counter",
            Dnp3ObjectGroup::AnalogInput => "analog_input",
            Dnp3ObjectGroup::AnalogOutputStatus => "analog_output_status",
            Dnp3ObjectGroup::AnalogOutput => "analog_output",
            Dnp3ObjectGroup::ControlOutput => "control_output",
        }
    }
}

/// The DNP3 event class a point is assigned to (UNVERIFIED: class 0 =
/// static data, classes 1–3 = event classes by priority; the exact
/// class-number semantics are checked against the standard text at
/// implementation).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventClass {
    /// Class 0: static (current-value) data, reported on integrity polls.
    Class0,
    /// Class 1: highest-priority event data.
    Class1,
    /// Class 2: medium-priority event data.
    Class2,
    /// Class 3: lowest-priority event data.
    Class3,
}

impl EventClass {
    /// Canonical name for records and error messages.
    pub const fn name(self) -> &'static str {
        match self {
            EventClass::Class0 => "class_0",
            EventClass::Class1 => "class_1",
            EventClass::Class2 => "class_2",
            EventClass::Class3 => "class_3",
        }
    }
}

/// The fleet quantity a DER-profile point carries. The mapping from a
/// fleet quantity to a DNP3 object group is the adapter's reviewed
/// decision (recorded per point in the outstation model); the UNIT
/// scaling (e.g. MW × 1000 → kW for an integer analog input) is DECLARED
/// per profile — never assumed (contracts units.rs rule at the adapter
/// boundary).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DerPointKind {
    /// Fleet aggregate active power (canonical MW; scaled to the point's
    /// declared source unit at the adapter).
    ActivePowerExport,
    /// Fleet aggregate reactive power (canonical MVAr; sign declared per
    /// profile, never inferred).
    ReactivePower,
    /// Fleet aggregate state of charge (canonical fraction; scaled to the
    /// point's declared source unit at the adapter).
    StateOfCharge,
    /// Availability / on-line status of the fleet (binary).
    Availability,
    /// Accumulated delivered energy (canonical MWh; scaled at the adapter).
    EnergyDelivered,
    /// A commanded active-power setpoint (the output-side point: written
    /// only through the command lifecycle, never by an adapter).
    ActivePowerSetpoint,
}

impl DerPointKind {
    /// The DNP3 object group this design maps the quantity to (reviewed
    /// mapping, record 24 §3; group numbers themselves UNVERIFIED).
    pub const fn default_object_group(self) -> Dnp3ObjectGroup {
        match self {
            DerPointKind::ActivePowerExport
            | DerPointKind::ReactivePower
            | DerPointKind::StateOfCharge => Dnp3ObjectGroup::AnalogInput,
            DerPointKind::Availability => Dnp3ObjectGroup::BinaryInput,
            DerPointKind::EnergyDelivered => Dnp3ObjectGroup::Counter,
            DerPointKind::ActivePowerSetpoint => Dnp3ObjectGroup::AnalogOutputStatus,
        }
    }

    /// Canonical name for records and error messages.
    pub const fn name(self) -> &'static str {
        match self {
            DerPointKind::ActivePowerExport => "active_power_export",
            DerPointKind::ReactivePower => "reactive_power",
            DerPointKind::StateOfCharge => "state_of_charge",
            DerPointKind::Availability => "availability",
            DerPointKind::EnergyDelivered => "energy_delivered",
            DerPointKind::ActivePowerSetpoint => "active_power_setpoint",
        }
    }
}

/// The declared source-unit scaling of one point: how a canonical value
/// renders into the integer point the protocol carries. Declared per
/// profile — the platform never assumes a scale (contracts units.rs).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeclaredScaling {
    /// Multiplicative factor from the canonical unit to the source unit
    /// (e.g. MW → kW is 1000.0; declared, never inferred).
    pub canonical_to_source_factor: f64,
    /// The declared reactive sign convention applied to this point, when
    /// the point carries reactive power (required exactly then; no
    /// default — units.rs).
    pub reactive_sign_convention: Option<flexcore_contracts::units::ReactiveSignConvention>,
}

impl DeclaredScaling {
    /// Validates a declared scaling: the factor must be finite and
    /// non-zero; a reactive convention is required exactly for reactive
    /// points.
    pub fn validate(
        &self,
        point: DerPointKind,
    ) -> Result<(), ModelError> {
        if !self.canonical_to_source_factor.is_finite()
            || self.canonical_to_source_factor == 0.0
        {
            return Err(ModelError::InvalidScaling {
                point: point.name(),
            });
        }
        let reactive = matches!(point, DerPointKind::ReactivePower);
        if reactive != self.reactive_sign_convention.is_some() {
            return Err(ModelError::ReactiveConventionRequired {
                point: point.name(),
                reactive,
            });
        }
        Ok(())
    }
}

/// Errors returned by the model layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelError {
    /// A scaling declaration is invalid (non-finite or zero factor).
    InvalidScaling { point: &'static str },
    /// A reactive point lacks its declared sign convention, or a
    /// non-reactive point declares one (units.rs: the convention is
    /// present exactly when reactive power is present).
    ReactiveConventionRequired { point: &'static str, reactive: bool },
    /// A converted point value is not finite.
    NonFiniteValue,
}

impl std::fmt::Display for ModelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModelError::InvalidScaling { point } => {
                write!(f, "declared scaling for point '{point}' is zero or non-finite")
            }
            ModelError::ReactiveConventionRequired { point, reactive } => {
                if *reactive {
                    write!(
                        f, "reactive point '{point}' lacks its declared sign convention (never inferred; units.rs)"
                    )
                } else {
                    write!(
                        f, "non-reactive point '{point}' declares a reactive sign convention"
                    )
                }
            }
            ModelError::NonFiniteValue => {
                write!(f, "converted point value is not finite")
            }
        }
    }
}

impl std::error::Error for ModelError {}

#[cfg(test)]
mod tests {
    use super::*;
    use flexcore_contracts::units::ReactiveSignConvention;

    #[test]
    fn group_numbers_are_pinned_in_one_place() {
        // UNVERIFIED values pinned so a standard-text check that disagrees
        // has exactly one place to edit.
        assert_eq!(Dnp3ObjectGroup::BinaryInput.group_number(), 1);
        assert_eq!(Dnp3ObjectGroup::Counter.group_number(), 20);
        assert_eq!(Dnp3ObjectGroup::AnalogInput.group_number(), 30);
        assert_eq!(Dnp3ObjectGroup::AnalogOutputStatus.group_number(), 40);
        assert_eq!(Dnp3ObjectGroup::AnalogOutput.group_number(), 41);
        assert_eq!(Dnp3ObjectGroup::ControlOutput.group_number(), 12);
    }

    #[test]
    fn point_kinds_map_to_declared_object_groups() {
        assert_eq!(
            DerPointKind::ActivePowerExport.default_object_group(),
            Dnp3ObjectGroup::AnalogInput
        );
        assert_eq!(
            DerPointKind::Availability.default_object_group(),
            Dnp3ObjectGroup::BinaryInput
        );
        assert_eq!(
            DerPointKind::EnergyDelivered.default_object_group(),
            Dnp3ObjectGroup::Counter
        );
        assert_eq!(
            DerPointKind::ActivePowerSetpoint.default_object_group(),
            Dnp3ObjectGroup::AnalogOutputStatus
        );
    }

    #[test]
    fn reactive_points_require_exactly_the_declared_convention() {
        let with = DeclaredScaling {
            canonical_to_source_factor: 1000.0,
            reactive_sign_convention: Some(ReactiveSignConvention::InjectionPositive),
        };
        assert!(with.validate(DerPointKind::ReactivePower).is_ok());
        // Missing convention on a reactive point: refused.
        let without = DeclaredScaling {
            canonical_to_source_factor: 1000.0,
            reactive_sign_convention: None,
        };
        assert_eq!(
            without.validate(DerPointKind::ReactivePower),
            Err(ModelError::ReactiveConventionRequired {
                point: "reactive_power",
                reactive: true
            })
        );
        // A convention on a NON-reactive point: also refused.
        assert_eq!(
            with.validate(DerPointKind::ActivePowerExport),
            Err(ModelError::ReactiveConventionRequired {
                point: "active_power_export",
                reactive: false
            })
        );
    }

    #[test]
    fn invalid_scalings_are_refused() {
        let zero = DeclaredScaling {
            canonical_to_source_factor: 0.0,
            reactive_sign_convention: None,
        };
        assert_eq!(
            zero.validate(DerPointKind::ActivePowerExport),
            Err(ModelError::InvalidScaling {
                point: "active_power_export"
            })
        );
        let nan = DeclaredScaling {
            canonical_to_source_factor: f64::NAN,
            reactive_sign_convention: None,
        };
        assert_eq!(
            nan.validate(DerPointKind::ActivePowerExport),
            Err(ModelError::InvalidScaling {
                point: "active_power_export"
            })
        );
    }
}
