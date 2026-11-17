// P14.07 — The register-map registry, keyed per device model.
//
// The registry is REVIEWED DATA: for each SunSpec device model a
// deployment fronts, an operator registers the field map (labels, offsets
// within the model block, data types, scale-factor register offsets, unit
// kinds) that the lab verification confirmed for THAT device. Nothing is
// auto-discovered and nothing is guessed: a device model without a
// registered map is a named refusal, and a register read outside the
// mapped window is refused rather than decoded ad hoc.
//
// `BTreeMap` keyed by device-model key (house rule: deterministic
// ordering). The registry holds the map; the DECODER
// ([`decode_register_window`]) applies a map to a raw register window.

use std::collections::BTreeMap;

use crate::model::{RegisterType, RegisterUnitKind, SunSpecModel};

/// The registry key: the vendor/device-model string a deployment assigns
/// (e.g. `"acme-gateway/inv-3ph-x1"`). Opaque here on purpose — the
/// mapping from a physical device to a key is deployment configuration,
/// never runtime inference.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeviceModelKey(pub String);

impl DeviceModelKey {
    /// Validates the key (non-empty, no whitespace-only).
    pub fn new(value: impl Into<String>) -> Result<Self, RegistryError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(RegistryError::EmptyField("device_model_key"));
        }
        Ok(Self(value))
    }
}

impl std::fmt::Display for DeviceModelKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// One mapped field of a device model's register map.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterField {
    /// The field's stable label (the registry's own name; the mapping to
    /// SunSpec official point names is UNVERIFIED per [S-16] and reviewed
    /// per device model at implementation).
    pub label: String,
    /// The SunSpec model family the field belongs to (the block the field
    /// is read within).
    pub model: SunSpecModel,
    /// The field's offset IN REGISTERS from the start of the model's
    /// block. UNVERIFIED per-device: offsets are lab-verified per device
    /// model before a map is registered.
    pub offset_registers: u16,
    /// The register data type (determines [`RegisterType::register_count`]).
    pub register_type: RegisterType,
    /// The offset of the field's scale-factor register within the block,
    /// when the field has one (UNVERIFIED which fields carry SFs).
    pub scale_factor_offset: Option<u16>,
    /// The unit kind before adapter-boundary conversion.
    pub unit_kind: RegisterUnitKind,
}

impl RegisterField {
    /// Validates the field (non-empty label; a scale-factor offset that
    /// overlaps the field itself is refused — a desync guard, not a
    /// protocol rule).
    pub fn new(
        label: impl Into<String>,
        model: SunSpecModel,
        offset_registers: u16,
        register_type: RegisterType,
        scale_factor_offset: Option<u16>,
        unit_kind: RegisterUnitKind,
    ) -> Result<Self, RegistryError> {
        let label = label.into();
        if label.trim().is_empty() {
            return Err(RegistryError::EmptyField("register_field.label"));
        }
        if let Some(sf) = scale_factor_offset {
            let end = offset_registers + register_type.register_count();
            if sf >= offset_registers && sf < end {
                return Err(RegistryError::ScaleFactorOverlapsField {
                    label: label.clone(),
                });
            }
        }
        Ok(Self {
            label,
            model,
            offset_registers,
            register_type,
            scale_factor_offset,
            unit_kind,
        })
    }
}

/// The reviewed register map for ONE device model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterMap {
    /// The device model the map belongs to.
    pub device_model: DeviceModelKey,
    /// The SunSpec model family the device presents (single family per
    /// map; multi-model devices register per-block maps).
    pub family: SunSpecModel,
    /// The mapped fields, in registry order.
    pub fields: Vec<RegisterField>,
    /// The total block length in registers the map covers (fields must
    /// fit; the budget check uses this).
    pub block_length_registers: u16,
}

impl RegisterMap {
    /// Validates and constructs a register map: fields non-empty, every
    /// field within the block, no overlapping fields.
    pub fn new(
        device_model: DeviceModelKey,
        family: SunSpecModel,
        fields: Vec<RegisterField>,
        block_length_registers: u16,
    ) -> Result<Self, RegistryError> {
        if fields.is_empty() {
            return Err(RegistryError::EmptyField("register_map.fields"));
        }
        if block_length_registers == 0 {
            return Err(RegistryError::EmptyField("register_map.block_length_registers"));
        }
        let mut occupied: Vec<(u16, u16)> = Vec::new();
        for field in &fields {
            let end = field
                .offset_registers
                .checked_add(field.register_type.register_count())
                .ok_or(RegistryError::FieldOutOfRange {
                    label: field.label.clone(),
                })?;
            if end > block_length_registers {
                return Err(RegistryError::FieldOutOfRange {
                    label: field.label.clone(),
                });
            }
            for &(start, stop) in &occupied {
                if field.offset_registers < stop && start < end {
                    return Err(RegistryError::OverlappingFields {
                        a: field.label.clone(),
                    });
                }
            }
            occupied.push((field.offset_registers, end));
        }
        Ok(Self {
            device_model,
            family,
            fields,
            block_length_registers,
        })
    }

    /// The mapped field with the given label, if present.
    pub fn field(&self, label: &str) -> Option<&RegisterField> {
        self.fields.iter().find(|f| f.label == label)
    }
}

/// Errors returned by the registry and the decoder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryError {
    /// A required field is empty.
    EmptyField(&'static str),
    /// The device model is not registered. There is no default map and no
    /// runtime discovery: an unregistered model is a named refusal.
    DeviceModelNotRegistered { key: String },
    /// A field exceeds the map's block length.
    FieldOutOfRange { label: String },
    /// Two mapped fields overlap.
    OverlappingFields { a: String },
    /// A scale-factor register overlaps the field it scales.
    ScaleFactorOverlapsField { label: String },
    /// The read window is shorter than the field needs at its offset.
    WindowTooShort { label: String, need: u16, have: u16 },
    /// A scale factor needed by the map is missing from the window.
    ScaleFactorMissing { label: String },
    /// The raw window did not carry the SunSpec common-block marker
    /// (model-level refusal, surfaced here for the decode path).
    NotSunSpecShaped,
    /// The model layer refused the decoded value (non-finite, bad SF).
    Model(crate::model::ModelError),
}

impl std::fmt::Display for RegistryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RegistryError::EmptyField(field) => write!(f, "registry field '{field}' is empty"),
            RegistryError::DeviceModelNotRegistered { key } => {
                write!(f, "device model '{key}' has no registered register map; refusing to guess")
            }
            RegistryError::FieldOutOfRange { label } => {
                write!(f, "field '{label}' exceeds the mapped block length")
            }
            RegistryError::OverlappingFields { a } => {
                write!(f, "field '{a}' overlaps another mapped field")
            }
            RegistryError::ScaleFactorOverlapsField { label } => {
                write!(f, "scale-factor register for '{label}' overlaps the field itself")
            }
            RegistryError::WindowTooShort { label, need, have } => {
                write!(f, "read window too short for '{label}' (needs {need} registers at its offset, window has {have})")
            }
            RegistryError::ScaleFactorMissing { label } => {
                write!(f, "scale-factor register for '{label}' is outside the read window")
            }
            RegistryError::NotSunSpecShaped => {
                write!(f, "device map did not carry the SunSpec common-block marker")
            }
            RegistryError::Model(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for RegistryError {}

impl From<crate::model::ModelError> for RegistryError {
    fn from(err: crate::model::ModelError) -> Self {
        RegistryError::Model(err)
    }
}

/// The register-map registry: the reviewed per-device-model maps.
#[derive(Debug, Clone)]
pub struct RegisterMapRegistry {
    /// The adapter id the registry belongs to.
    pub adapter_id: String,
    maps: BTreeMap<DeviceModelKey, RegisterMap>,
}

impl RegisterMapRegistry {
    /// Constructs an empty registry for the given adapter id.
    pub fn new(adapter_id: impl Into<String>) -> Self {
        Self {
            adapter_id: adapter_id.into(),
            maps: BTreeMap::new(),
        }
    }

    /// Registers (or replaces) the map for a device model. Replacing is
    /// allowed but is a REVIEWED edit of the map — the caller owns the
    /// review; the registry records nothing silently.
    pub fn register(&mut self, map: RegisterMap) -> Result<(), RegistryError> {
        // Validate on the way in (RegisterMap::new already validated; a
        // hand-built map is re-checked here for overlap/consistency).
        RegisterMap::new(
            map.device_model.clone(),
            map.family,
            map.fields.clone(),
            map.block_length_registers,
        )?;
        self.maps.insert(map.device_model.clone(), map);
        Ok(())
    }

    /// The map for a device model, or the named refusal.
    pub fn map_for(&self, key: &DeviceModelKey) -> Result<&RegisterMap, RegistryError> {
        self.maps
            .get(key)
            .ok_or_else(|| RegistryError::DeviceModelNotRegistered {
                key: key.0.clone(),
            })
    }

    /// The number of registered maps.
    pub fn len(&self) -> usize {
        self.maps.len()
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.maps.is_empty()
    }
}

/// Decodes one mapped field from a raw register window that starts at the
/// device model's block start.
///
/// - 16-bit types read one register (i16 as two's complement — UNVERIFIED
///   byte order/interpretation per [S-16]).
/// - 32-bit types read big-endian register pairs — UNVERIFIED.
/// - Scale factors are read from the map's SF register and applied via
///   [`crate::model::apply_scale_factor`] (value × 10^SF — UNVERIFIED).
///
/// Returns the decoded field in SOURCE terms; conversion to canonical
/// units is [`crate::units_map`]'s job (one reviewable boundary).
pub fn decode_register_window(
    map: &RegisterMap,
    field: &RegisterField,
    window: &[u16],
) -> Result<crate::model::DecodedField, RegistryError> {
    let need = field.offset_registers + field.register_type.register_count();
    if window.len() < usize::from(need) {
        return Err(RegistryError::WindowTooShort {
            label: field.label.clone(),
            need,
            have: window.len() as u16,
        });
    }
    let raw_value: f64 = match field.register_type {
        RegisterType::U16 => f64::from(window[usize::from(field.offset_registers)]),
        RegisterType::I16 => {
            f64::from(window[usize::from(field.offset_registers)] as i16)
        }
        RegisterType::U32 => {
            let hi = u32::from(window[usize::from(field.offset_registers)]);
            let lo = u32::from(window[usize::from(field.offset_registers + 1)]);
            f64::from((hi << 16) | lo)
        }
        RegisterType::I32 => {
            let hi = u32::from(window[usize::from(field.offset_registers)]);
            let lo = u32::from(window[usize::from(field.offset_registers + 1)]);
            f64::from(((hi << 16) | lo) as i32)
        }
        RegisterType::Bitfield16 => {
            f64::from(window[usize::from(field.offset_registers)])
        }
    };
    let scaled = match field.scale_factor_offset {
        Some(sf_offset) => {
            let sf_index = usize::from(sf_offset);
            if window.len() <= sf_index {
                return Err(RegistryError::ScaleFactorMissing {
                    label: field.label.clone(),
                });
            }
            let sf = window[sf_index] as i16;
            crate::model::apply_scale_factor(&field.label, raw_value, sf)?
        }
        None => raw_value,
    };
    crate::model::DecodedField::new(map.family, field.label.clone(), field.unit_kind, scaled)
        .map_err(RegistryError::Model)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ModelError;

    fn field(label: &str, offset: u16, rt: RegisterType, kind: RegisterUnitKind) -> RegisterField {
        RegisterField::new(label, SunSpecModel::InverterThreePhase, offset, rt, None, kind)
            .expect("valid field")
    }

    fn map_with(fields: Vec<RegisterField>, length: u16) -> RegisterMap {
        RegisterMap::new(
            DeviceModelKey::new("test/inv").expect("valid key"),
            SunSpecModel::InverterThreePhase,
            fields,
            length,
        )
        .expect("valid map")
    }

    #[test]
    fn registry_refuses_unregistered_device_models() {
        let registry = RegisterMapRegistry::new("modbus-client");
        assert_eq!(
            registry.map_for(&DeviceModelKey::new("acme/inv-1").expect("valid key")),
            Err(RegistryError::DeviceModelNotRegistered {
                key: "acme/inv-1".to_string()
            })
        );
    }

    #[test]
    fn map_validation_refuses_overlap_and_out_of_range() {
        let fields = vec![field("W", 0, RegisterType::I16, RegisterUnitKind::Watts)];
        assert!(map_with(fields.clone(), 4).is_ok());
        // Field beyond the block length: refused.
        assert!(map_with(fields.clone(), 1).is_err());
        // Overlap: W at 0..1 and another at 0 refused.
        let overlapping = vec![
            field("W", 0, RegisterType::I16, RegisterUnitKind::Watts),
            field("WSF", 0, RegisterType::I16, RegisterUnitKind::Enumerated),
        ];
        assert_eq!(
            RegisterMap::new(
                DeviceModelKey::new("test/inv").expect("valid key"),
                SunSpecModel::InverterThreePhase,
                overlapping,
                4,
            ),
            Err(RegistryError::OverlappingFields { a: "WSF".to_string() })
        );
    }

    #[test]
    fn decode_i16_negative_watts_passes_through() {
        let m = map_with(vec![field("W", 0, RegisterType::I16, RegisterUnitKind::Watts)], 4);
        let f = m.field("W").expect("field").clone();
        // -1500 as two's-complement i16 (0xFA1C).
        let decoded = decode_register_window(&m, &f, &[0xFA1C]).expect("decode");
        assert_eq!(decoded.value, -1500.0);
        assert_eq!(decoded.unit_kind, RegisterUnitKind::Watts);
    }

    #[test]
    fn decode_u32_big_endian_pair() {
        let m = map_with(vec![field("WH", 0, RegisterType::U32, RegisterUnitKind::WattHours)], 4);
        let f = m.field("WH").expect("field").clone();
        let decoded = decode_register_window(&m, &f, &[0x0001, 0x86A0]).expect("decode");
        // 0x000186A0 = 100000.
        assert_eq!(decoded.value, 100_000.0);
    }

    #[test]
    fn decode_applies_scale_factor_from_the_map() {
        let m = map_with(
            vec![RegisterField::new(
                "W",
                SunSpecModel::MeterThreePhase,
                0,
                RegisterType::I16,
                Some(2),
                RegisterUnitKind::Watts,
            )
            .expect("valid field")],
            4,
        );
        let f = m.field("W").expect("field").clone();
        // Raw 12345 with SF = -1 → 1234.5 W.
        let decoded = decode_register_window(&m, &f, &[12_345, 0, 0xFFFF, 0]).expect("decode");
        assert_eq!(decoded.value, 1234.5);
        // SF missing from the window: named refusal.
        assert_eq!(
            decode_register_window(&m, &f, &[12_345]),
            Err(RegistryError::ScaleFactorMissing { label: "W".to_string() })
        );
        // Implausible SF: named refusal via the model layer.
        assert!(matches!(
            decode_register_window(&m, &f, &[12_345, 0, 99, 0]),
            Err(RegistryError::Model(ModelError::ImplausibleScaleFactor { .. }))
        ));
    }

    #[test]
    fn window_too_short_is_a_named_refusal() {
        let m = map_with(vec![field("W", 2, RegisterType::I16, RegisterUnitKind::Watts)], 4);
        let f = m.field("W").expect("field").clone();
        assert_eq!(
            decode_register_window(&m, &f, &[1]),
            Err(RegistryError::WindowTooShort {
                label: "W".to_string(),
                need: 3,
                have: 1
            })
        );
    }
}
