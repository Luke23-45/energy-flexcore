// P14.12 — The declared CIM class mapping into the grid-api network IR
// (decision record 44_p14_12_cim_cgmes_import.md; sources [S-17][S-18]).
//
// THE CLASSIFICATION RULE (the `crates/edge-agent/src/adapters/openems.rs`
// pattern carried forward verbatim, plan §2 row P14.12): every profile
// object is classified EXACTLY ONE of
//
// - **MAPPED** — a valid, unambiguous mapping entry produced a concrete
//   grid-api network-IR element;
// - **EXPLICITLY_UNMAPPED** — the mapping vocabulary RECOGNIZES the class
//   and DECLARES it unmapped, with a recorded reason (the reviewed
//   limitation record);
// - **UNMAPPED** — no vocabulary entry for the class name at all;
//   recorded with that reason, never mapped by guessing;
// - **AMBIGUOUS** — the class is map-eligible but a required declared
//   value is missing, non-finite, or outside the declared unit rule;
//   never mapped by guessing (the units-are-never-guessed rule, and the
//   reactive-sign rule of `flexcore-contracts::units` applied to the
//   import surface).
//
// THE MAPPING TARGETS: the grid-api network IR
// (`flexcore-grid-api::port::VersionedNetworkModel`) is a BUS-BRANCH
// model — buses, branches, two-winding transformers, one external grid.
// It has NO load element, NO DER equipment element, NO switch state, NO
// topology-container hierarchy. That absence is the honest source of the
// EXPLICITLY_UNMAPPED reasons below: DER equipment is modelled in
// flexcore-domain envelopes (not the network IR), and an import that
// guessed a bus injection for a PowerElectronicsConnection would fabricate
// a generator the model does not own. The mapping refuses; the register
// ([`crate::unmapped`]) records.
//
// UNVERIFIED HONESTY: every attribute key and unit rule below is a
// DECLARED mapping-profile choice (the constants of [`crate::model`]) —
// the implementation-time check against the CGMES/CIM schema text
// [S-17] confirms the attribute owners, spellings, and units before this
// mapping composes against a real profile.

use serde::{Deserialize, Serialize};

use flexcore_grid_api::port::{BranchSpec, BusSpec, ExternalGridSpec, TransformerSpec};

use crate::model::{
    CimMRid, CimObject, CimKnownClass,
    ATTR_BASE_VOLTAGE_NOMINAL_KV, ATTR_EXTERNAL_GRID_BUS_REF,
    ATTR_EXTERNAL_GRID_VA_DEGREE, ATTR_EXTERNAL_GRID_VM_PU, ATTR_LINE_FROM_BUS_REF,
    ATTR_LINE_RATED_CURRENT_A, ATTR_LINE_R_OHM, ATTR_LINE_TO_BUS_REF, ATTR_LINE_X_OHM,
    ATTR_NODE_BASE_VOLTAGE_REF, ATTR_TRANSFORMER_HV_BUS_REF,
    ATTR_TRANSFORMER_LV_BUS_REF, ATTR_TRANSFORMER_RATED_S_VA, ATTR_TRANSFORMER_VK_PERCENT,
    ATTR_TRANSFORMER_VK_R_PERCENT,
};

/// The concrete network-IR element a MAPPED classification produced.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MappingTarget {
    /// A bus of the network IR.
    Bus(BusSpec),
    /// A branch (line / cable) of the network IR.
    Branch(BranchSpec),
    /// A two-winding transformer of the network IR.
    Transformer(TransformerSpec),
    /// The external grid / slack source of the network IR.
    ExternalGrid(ExternalGridSpec),
}

impl MappingTarget {
    /// Canonical kind name for the import summary.
    pub const fn kind(&self) -> &'static str {
        match self {
            MappingTarget::Bus(_) => "bus",
            MappingTarget::Branch(_) => "branch",
            MappingTarget::Transformer(_) => "transformer",
            MappingTarget::ExternalGrid(_) => "external_grid",
        }
    }
}

/// The classification of ONE profile object (the openems.rs pattern:
/// exactly one of four statuses, every refusal named).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "status")]
pub enum ClassMappingStatus {
    Mapped {
        target: MappingTarget,
    },
    /// The vocabulary recognizes the class and DECLARES it unmapped, with
    /// the recorded reason (the reviewed limitation record).
    ExplicitlyUnmapped {
        reason: String,
    },
    /// No vocabulary entry for this class name; recorded, never guessed.
    Unmapped {
        reason: String,
    },
    /// Map-eligible but a required declared value is missing or invalid;
    /// never mapped by guessing.
    Ambiguous {
        reason: String,
    },
}

impl ClassMappingStatus {
    /// Canonical status name for the import summary (the openems.rs
    /// vocabulary names).
    pub const fn name(&self) -> &'static str {
        match self {
            ClassMappingStatus::Mapped { .. } => "MAPPED",
            ClassMappingStatus::ExplicitlyUnmapped { .. } => "EXPLICITLY_UNMAPPED",
            ClassMappingStatus::Unmapped { .. } => "UNMAPPED",
            ClassMappingStatus::Ambiguous { .. } => "AMBIGUOUS",
        }
    }
}

/// Errors returned by the declared per-class mappers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MappingError {
    /// A required declared attribute is absent — the mapping refuses
    /// (never defaults). The reason names the attribute and its
    /// implementation-time check.
    MissingDeclaredAttribute {
        attribute: &'static str,
    },
    /// A declared attribute's value is not a finite number in the
    /// declared unit rule.
    InvalidNumber {
        attribute: &'static str,
        observed: String,
    },
}

impl std::fmt::Display for MappingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MappingError::MissingDeclaredAttribute { attribute } => {
                write!(f, "required declared attribute '{attribute}' is absent — the mapping refuses, never defaults")
            }
            MappingError::InvalidNumber { attribute, observed } => {
                write!(
                    f,
                    "declared attribute '{attribute}' is not a finite number in the declared \
                     unit rule (observed: '{observed}')"
                )
            }
        }
    }
}

impl std::error::Error for MappingError {}

/// Reads a required finite f64 attribute.
fn required_f64(object: &CimObject, attribute: &'static str) -> Result<f64, MappingError> {
    let raw = object
        .attribute(attribute)
        .ok_or(MappingError::MissingDeclaredAttribute { attribute })?;
    let value = raw
        .trim()
        .parse::<f64>()
        .map_err(|_| MappingError::InvalidNumber {
            attribute,
            observed: raw.to_string(),
        })?;
    if !value.is_finite() {
        return Err(MappingError::InvalidNumber {
            attribute,
            observed: raw.to_string(),
        });
    }
    Ok(value)
}

/// Reads a required mRID reference attribute.
fn required_ref(object: &CimObject, attribute: &'static str) -> Result<CimMRid, MappingError> {
    let raw = object
        .attribute(attribute)
        .ok_or(MappingError::MissingDeclaredAttribute { attribute })?;
    CimMRid::new(raw).map_err(|_| MappingError::InvalidNumber {
        attribute,
        observed: raw.to_string(),
    })
}

/// The declared CIM→network-IR mapping.
///
/// Constructed with [`CimNetworkMapping::new`]; every classification goes
/// through [`CimNetworkMapping::classify`] — the same method for every
/// object, so the mapping cannot grow an unlabeled side door.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CimNetworkMapping;

impl CimNetworkMapping {
    pub fn new() -> Self {
        Self
    }

    /// Classifies one profile object. The classification is total: every
    /// object receives exactly one status.
    pub fn classify(
        &self,
        object: &CimObject,
        voltage_registry: &BaseVoltageRegistry,
    ) -> ClassMappingStatus {
        let recognized = CimKnownClassVocabulary::recognize(&object.class_name);
        match recognized {
            Some(CimKnownClass::ConnectivityNode) => {
                self.classify_connectivity_node(object, voltage_registry)
            }
            Some(CimKnownClass::AcLineSegment) => self.classify_line(object),
            Some(CimKnownClass::PowerTransformer) => self.classify_transformer(object),
            Some(CimKnownClass::ExternalNetworkInjection) => {
                self.classify_external_grid(object)
            }
            Some(known) => explicitly_unmapped(known),
            None => ClassMappingStatus::Unmapped {
                reason: format!(
                    "no mapping-vocabulary entry for CIM class '{}'; recorded, never \
                     guessed ([S-17] honesty)",
                    object.class_name
                ),
            },
        }
    }

    // --- The mapped classes ---

    /// ConnectivityNode → BusSpec. Requires a declared BaseVoltage
    /// reference resolvable through the registry; a node whose voltage is
    /// not declared is AMBIGUOUS (never defaulted to a nominal guess).
    fn classify_connectivity_node(
        &self,
        object: &CimObject,
        voltage_registry: &BaseVoltageRegistry,
    ) -> ClassMappingStatus {
        let bv_ref = match required_ref(object, ATTR_NODE_BASE_VOLTAGE_REF) {
            Ok(reference) => reference,
            Err(error) => return ambiguous_from(error),
        };
        let nominal_kv = match voltage_registry.nominal_kv(&bv_ref) {
            Some(kv) => kv,
            None => {
                return ClassMappingStatus::Ambiguous {
                    reason: format!(
                        "the node's BaseVoltage reference '{}' is not in the profile's \
                         declared nominal-kV registry — the mapping refuses, never \
                         guesses a bus voltage",
                        bv_ref.value()
                    ),
                }
            }
        };
        if nominal_kv <= 0.0 {
            return ClassMappingStatus::Ambiguous {
                reason: format!(
                    "the referenced nominal voltage {nominal_kv} kV is not strictly positive"
                ),
            };
        }
        ClassMappingStatus::Mapped {
            target: MappingTarget::Bus(BusSpec {
                bus_id: object.mrid.value().to_string(),
                nominal_kv,
                // The slack designation is a DECLARED import decision, not
                // a CIM attribute: exactly one slack comes from the
                // external-grid mapping (see classify_external_grid), so
                // a bus starts non-slack and the assembler flips the one
                // declared slack.
                is_slack: false,
            }),
        }
    }

    /// ACLineSegment → BranchSpec. Requires the declared ohm attributes
    /// and a declared rated current (UNVERIFIED source — the real CGMES
    /// carrier may be OperationalLimitSet; the reason names the check).
    fn classify_line(&self, object: &CimObject) -> ClassMappingStatus {
        let r = match required_f64(object, ATTR_LINE_R_OHM) {
            Ok(value) => value,
            Err(error) => return ambiguous_from(error),
        };
        let x = match required_f64(object, ATTR_LINE_X_OHM) {
            Ok(value) => value,
            Err(error) => return ambiguous_from(error),
        };
        let rated = match required_f64(object, ATTR_LINE_RATED_CURRENT_A) {
            Ok(value) => value,
            Err(error) => return ambiguous_from(error),
        };
        let from = match required_ref(object, ATTR_LINE_FROM_BUS_REF) {
            Ok(value) => value,
            Err(error) => return ambiguous_from(error),
        };
        let to = match required_ref(object, ATTR_LINE_TO_BUS_REF) {
            Ok(value) => value,
            Err(error) => return ambiguous_from(error),
        };
        if r < 0.0 || x <= 0.0 || rated <= 0.0 {
            return ClassMappingStatus::Ambiguous {
                reason: format!(
                    "line ratings violate the declared bounds (r = {r} ohm must be \
                     non-negative, x = {x} ohm and rated = {rated} A must be strictly \
                     positive)"
                ),
            };
        }
        ClassMappingStatus::Mapped {
            target: MappingTarget::Branch(BranchSpec {
                branch_id: object.mrid.value().to_string(),
                from_bus: from.value().to_string(),
                to_bus: to.value().to_string(),
                r_ohm: r,
                x_ohm: x,
                rated_current_a: rated,
            }),
        }
    }

    /// PowerTransformer → TransformerSpec. Requires the declared rated-S
    /// (VA, converted to MVA at the boundary — the canonical-unit rule)
    /// and the short-circuit percentages (UNVERIFIED derivation path —
    /// the real CIM carrier may be TransformerEndInfo / ShortCircuitTest;
    /// the reason names the check).
    fn classify_transformer(&self, object: &CimObject) -> ClassMappingStatus {
        let rated_s_va = match required_f64(object, ATTR_TRANSFORMER_RATED_S_VA) {
            Ok(value) => value,
            Err(error) => return ambiguous_from(error),
        };
        let vk = match required_f64(object, ATTR_TRANSFORMER_VK_PERCENT) {
            Ok(value) => value,
            Err(error) => return ambiguous_from(error),
        };
        let vk_r = match required_f64(object, ATTR_TRANSFORMER_VK_R_PERCENT) {
            Ok(value) => value,
            Err(error) => return ambiguous_from(error),
        };
        let hv = match required_ref(object, ATTR_TRANSFORMER_HV_BUS_REF) {
            Ok(value) => value,
            Err(error) => return ambiguous_from(error),
        };
        let lv = match required_ref(object, ATTR_TRANSFORMER_LV_BUS_REF) {
            Ok(value) => value,
            Err(error) => return ambiguous_from(error),
        };
        // VA → MVA at the adapter boundary (the canonical-unit rule; no
        // scaled unit crosses into the network IR).
        let rated_mva = rated_s_va / 1_000_000.0;
        if rated_mva <= 0.0 || vk <= 0.0 || vk_r < 0.0 || vk_r > vk {
            return ClassMappingStatus::Ambiguous {
                reason: format!(
                    "transformer ratings violate the declared bounds (ratedMVA = \
                     {rated_mva}, vk = {vk}%, vk_r = {vk_r}%)"
                ),
            };
        }
        ClassMappingStatus::Mapped {
            target: MappingTarget::Transformer(TransformerSpec {
                transformer_id: object.mrid.value().to_string(),
                hv_bus: hv.value().to_string(),
                lv_bus: lv.value().to_string(),
                rated_mva,
                vk_percent: vk,
                vk_r_percent: vk_r,
            }),
        }
    }

    /// ExternalNetworkInjection → ExternalGridSpec. Requires the declared
    /// slack-bus reference and the declared per-unit magnitude and degree
    /// angle (UNVERIFIED spellings — see [`crate::model`]).
    fn classify_external_grid(&self, object: &CimObject) -> ClassMappingStatus {
        let bus = match required_ref(object, ATTR_EXTERNAL_GRID_BUS_REF) {
            Ok(value) => value,
            Err(error) => return ambiguous_from(error),
        };
        let vm_pu = match required_f64(object, ATTR_EXTERNAL_GRID_VM_PU) {
            Ok(value) => value,
            Err(error) => return ambiguous_from(error),
        };
        let va = match required_f64(object, ATTR_EXTERNAL_GRID_VA_DEGREE) {
            Ok(value) => value,
            Err(error) => return ambiguous_from(error),
        };
        if vm_pu <= 0.0 {
            return ClassMappingStatus::Ambiguous {
                reason: format!("the slack voltage magnitude {vm_pu} p.u. is not positive"),
            };
        }
        ClassMappingStatus::Mapped {
            target: MappingTarget::ExternalGrid(ExternalGridSpec {
                bus_id: bus.value().to_string(),
                vm_pu,
                va_degree: va,
            }),
        }
    }
}

/// The recognized classes the mapping DECLARES unmapped, each with its
/// recorded reason (the reviewed limitation record; the plan §2 P14.12
/// row: every unmapped CIM class is EXPLICITLY_UNMAPPED, never guessed).
fn explicitly_unmapped(known: CimKnownClass) -> ClassMappingStatus {
    let reason = match known {
        CimKnownClass::EnergyConsumer => {
            "the grid-api network IR (VersionedNetworkModel) has no load element; load \
             aggregation is not part of this import — unmapped, never guessed as a bus \
             injection"
        }
        CimKnownClass::PowerElectronicsConnection => {
            "DER equipment is modelled in flexcore-domain envelopes, not the network IR; \
             an import that guessed a bus injection would fabricate a generator the \
             model does not own — unmapped, recorded in the register"
        }
        CimKnownClass::PowerElectronicsUnit => {
            "a DER unit within a PowerElectronicsConnection; the connection itself is \
             explicitly unmapped (see its reason) — unmapped, recorded"
        }
        CimKnownClass::SynchronousMachine => {
            "machine equipment has no network-IR element in this import — unmapped, \
             recorded"
        }
        CimKnownClass::Breaker | CimKnownClass::Disconnector => {
            "switching state is not part of the bus-branch network IR in this import — \
             unmapped, recorded"
        }
        CimKnownClass::Terminal => {
            "terminal connectivity refinement is consumed by the declared bus/branch \
             reference attributes, not mapped as an element — unmapped, recorded"
        }
        CimKnownClass::Substation | CimKnownClass::VoltageLevel | CimKnownClass::Bay => {
            "the container hierarchy is not a network-IR element in this import — \
             unmapped, recorded"
        }
        // BaseVoltage is handled by classify_base_voltage; the match must
        // still be total.
        CimKnownClass::BaseVoltage => {
            "BaseVoltage is reference data (the nominal-kV registry the bus mapping \
             consumes), not a network-IR element — registered, not mapped"
        }
        CimKnownClass::AcLineSegment
        | CimKnownClass::ConnectivityNode
        | CimKnownClass::PowerTransformer
        | CimKnownClass::ExternalNetworkInjection => {
            "mapped classes never reach the explicitly-unmapped path"
        }
    };
    ClassMappingStatus::ExplicitlyUnmapped {
        reason: reason.to_string(),
    }
}

fn ambiguous_from(error: MappingError) -> ClassMappingStatus {
    ClassMappingStatus::Ambiguous {
        reason: error.to_string(),
    }
}

/// The declared nominal-kV registry the bus mapping consumes (fed from
/// the profile's BaseVoltage objects).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct BaseVoltageRegistry {
    nominal_kv_by_mrid: std::collections::BTreeMap<CimMRid, f64>,
}

impl BaseVoltageRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers one BaseVoltage object. Returns a named error when the
    /// object lacks the declared nominal-kV attribute or the value is
    /// invalid — the registry never stores a guessed voltage.
    pub fn register(&mut self, object: &CimObject) -> Result<(), MappingError> {
        let nominal_kv = required_f64(object, ATTR_BASE_VOLTAGE_NOMINAL_KV)?;
        if nominal_kv <= 0.0 {
            return Err(MappingError::InvalidNumber {
                attribute: ATTR_BASE_VOLTAGE_NOMINAL_KV,
                observed: format!("{nominal_kv}"),
            });
        }
        self.nominal_kv_by_mrid
            .insert(object.mrid.clone(), nominal_kv);
        Ok(())
    }

    /// The nominal kV declared for one BaseVoltage mRID.
    pub fn nominal_kv(&self, mrid: &CimMRid) -> Option<f64> {
        self.nominal_kv_by_mrid.get(mrid).copied()
    }
}

/// The class-recognition step (kept in one place so the vocabulary and
/// the spelling constants cannot diverge).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CimKnownClassVocabulary;

impl CimKnownClassVocabulary {
    /// Recognizes a profile class name against the declared vocabulary.
    /// UNVERIFIED spellings — the implementation-time check confirms each
    /// name against the CGMES profile text [S-17].
    pub fn recognize(class_name: &str) -> Option<CimKnownClass> {
        let names = [
            (CimKnownClass::AcLineSegment, CimKnownClass::AcLineSegment.name()),
            (CimKnownClass::PowerTransformer, CimKnownClass::PowerTransformer.name()),
            (CimKnownClass::BaseVoltage, CimKnownClass::BaseVoltage.name()),
            (CimKnownClass::ConnectivityNode, CimKnownClass::ConnectivityNode.name()),
            (CimKnownClass::EnergyConsumer, CimKnownClass::EnergyConsumer.name()),
            (
                CimKnownClass::PowerElectronicsConnection,
                CimKnownClass::PowerElectronicsConnection.name(),
            ),
            (CimKnownClass::PowerElectronicsUnit, CimKnownClass::PowerElectronicsUnit.name()),
            (
                CimKnownClass::ExternalNetworkInjection,
                CimKnownClass::ExternalNetworkInjection.name(),
            ),
            (CimKnownClass::Breaker, CimKnownClass::Breaker.name()),
            (CimKnownClass::Disconnector, CimKnownClass::Disconnector.name()),
            (CimKnownClass::SynchronousMachine, CimKnownClass::SynchronousMachine.name()),
            (CimKnownClass::Terminal, CimKnownClass::Terminal.name()),
            (CimKnownClass::Substation, CimKnownClass::Substation.name()),
            (CimKnownClass::VoltageLevel, CimKnownClass::VoltageLevel.name()),
            (CimKnownClass::Bay, CimKnownClass::Bay.name()),
        ];
        names
            .into_iter()
            .find(|(_, name)| *name == class_name)
            .map(|(known, _)| known)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn object(mrid: &str, class: &str, attributes: &[(&str, &str)]) -> CimObject {
        let mut map = std::collections::BTreeMap::new();
        for (key, value) in attributes {
            map.insert((*key).to_string(), (*value).to_string());
        }
        CimObject {
            mrid: CimMRid::new(mrid).expect("valid"),
            class_name: class.to_string(),
            attributes: map,
        }
    }

    fn registry_with_bv20() -> BaseVoltageRegistry {
        let mut registry = BaseVoltageRegistry::new();
        registry
            .register(&object(
                "_bv20",
                "BaseVoltage",
                &[(ATTR_BASE_VOLTAGE_NOMINAL_KV, "20.0")],
            ))
            .expect("valid");
        registry
    }

    #[test]
    fn a_complete_line_maps_to_a_branch() {
        let mapping = CimNetworkMapping::new();
        let registry = registry_with_bv20();
        let line = object(
            "_line1",
            "ACLineSegment",
            &[
                (ATTR_LINE_R_OHM, "1.5"),
                (ATTR_LINE_X_OHM, "4.0"),
                (ATTR_LINE_RATED_CURRENT_A, "400"),
                (ATTR_LINE_FROM_BUS_REF, "_busA"),
                (ATTR_LINE_TO_BUS_REF, "_busB"),
            ],
        );
        let status = mapping.classify(&line, &registry);
        match status {
            ClassMappingStatus::Mapped {
                target: MappingTarget::Branch(branch),
            } => {
                assert_eq!(branch.branch_id, "_line1");
                assert_eq!(branch.r_ohm, 1.5);
                assert_eq!(branch.x_ohm, 4.0);
                assert_eq!(branch.rated_current_a, 400.0);
            }
            other => panic!("expected a mapped branch, got {other:?}"),
        }
    }

    #[test]
    fn a_line_missing_the_rated_current_is_ambiguous_never_defaulted() {
        let mapping = CimNetworkMapping::new();
        let registry = registry_with_bv20();
        let line = object(
            "_line1",
            "ACLineSegment",
            &[
                (ATTR_LINE_R_OHM, "1.5"),
                (ATTR_LINE_X_OHM, "4.0"),
                // No rated current — the real carrier may be
                // OperationalLimitSet (UNVERIFIED); the mapping refuses.
                (ATTR_LINE_FROM_BUS_REF, "_busA"),
                (ATTR_LINE_TO_BUS_REF, "_busB"),
            ],
        );
        let status = mapping.classify(&line, &registry);
        match &status {
            ClassMappingStatus::Ambiguous { reason } => {
                assert!(
                    reason.contains("ratedCurrent"),
                    "the ambiguity names the missing attribute: {reason}"
                );
            }
            other => panic!("expected an ambiguous classification, got {other:?}"),
        }
        assert_eq!(status.name(), "AMBIGUOUS");
    }

    #[test]
    fn a_node_with_an_unresolvable_voltage_is_ambiguous_never_defaulted() {
        let mapping = CimNetworkMapping::new();
        let empty_registry = BaseVoltageRegistry::new();
        let node = object(
            "_busA",
            "ConnectivityNode",
            &[(ATTR_NODE_BASE_VOLTAGE_REF, "_bv20")],
        );
        let status = mapping.classify(&node, &empty_registry);
        assert!(matches!(
            &status,
            ClassMappingStatus::Ambiguous { .. }
        ));
        // With the registry populated, the node maps to a bus.
        let status = mapping.classify(&node, &registry_with_bv20());
        match status {
            ClassMappingStatus::Mapped {
                target: MappingTarget::Bus(bus),
            } => {
                assert_eq!(bus.bus_id, "_busA");
                assert_eq!(bus.nominal_kv, 20.0);
                assert!(!bus.is_slack);
            }
            other => panic!("expected a mapped bus, got {other:?}"),
        }
    }

    #[test]
    fn a_transformer_converts_va_to_canonical_mva() {
        let mapping = CimNetworkMapping::new();
        let registry = registry_with_bv20();
        let transformer = object(
            "_xfmr1",
            "PowerTransformer",
            &[
                (ATTR_TRANSFORMER_RATED_S_VA, "630000"),
                (ATTR_TRANSFORMER_VK_PERCENT, "6.0"),
                (ATTR_TRANSFORMER_VK_R_PERCENT, "1.0"),
                (ATTR_TRANSFORMER_HV_BUS_REF, "_busA"),
                (ATTR_TRANSFORMER_LV_BUS_REF, "_busB"),
            ],
        );
        let status = mapping.classify(&transformer, &registry);
        match status {
            ClassMappingStatus::Mapped {
                target: MappingTarget::Transformer(spec),
            } => {
                assert_eq!(spec.rated_mva, 0.63, "VA converted to canonical MVA");
                assert_eq!(spec.vk_percent, 6.0);
            }
            other => panic!("expected a mapped transformer, got {other:?}"),
        }
    }

    #[test]
    fn der_and_load_classes_are_explicitly_unmapped_with_recorded_reasons() {
        let mapping = CimNetworkMapping::new();
        let registry = registry_with_bv20();
        let pec = object("_pec1", "PowerElectronicsConnection", &[]);
        let status = mapping.classify(&pec, &registry);
        match &status {
            ClassMappingStatus::ExplicitlyUnmapped { reason } => {
                assert!(
                    reason.contains("flexcore-domain envelopes"),
                    "the recorded reason names where DER modelling lives: {reason}"
                );
            }
            other => panic!("expected an explicitly-unmapped classification, got {other:?}"),
        }
        let load = object("_load1", "EnergyConsumer", &[]);
        assert!(matches!(
            mapping.classify(&load, &registry),
            ClassMappingStatus::ExplicitlyUnmapped { .. }
        ));
        let breaker = object("_brk1", "Breaker", &[]);
        assert!(matches!(
            mapping.classify(&breaker, &registry),
            ClassMappingStatus::ExplicitlyUnmapped { .. }
        ));
    }

    #[test]
    fn an_unknown_class_is_unmapped_never_guessed() {
        let mapping = CimNetworkMapping::new();
        let registry = registry_with_bv20();
        let exotic = object("_x1", "EnergyScheduling", &[]);
        let status = mapping.classify(&exotic, &registry);
        match &status {
            ClassMappingStatus::Unmapped { reason } => {
                assert!(
                    reason.contains("EnergyScheduling"),
                    "the reason names the unrecognized class: {reason}"
                );
            }
            other => panic!("expected an unmapped classification, got {other:?}"),
        }
    }

    #[test]
    fn the_base_voltage_registry_refuses_guessed_voltages() {
        let mut registry = BaseVoltageRegistry::new();
        // Missing attribute.
        assert_eq!(
            registry.register(&object("_bv1", "BaseVoltage", &[])),
            Err(MappingError::MissingDeclaredAttribute {
                attribute: ATTR_BASE_VOLTAGE_NOMINAL_KV,
            })
        );
        // Non-positive value.
        assert!(registry
            .register(&object("_bv1", "BaseVoltage", &[(ATTR_BASE_VOLTAGE_NOMINAL_KV, "-5")]))
            .is_err());
        // Non-numeric value.
        assert!(registry
            .register(&object("_bv1", "BaseVoltage", &[(ATTR_BASE_VOLTAGE_NOMINAL_KV, "20 kV")]))
            .is_err());
    }

    #[test]
    fn vocabulary_recognition_covers_the_declared_classes() {
        for known in [
            CimKnownClass::AcLineSegment,
            CimKnownClass::PowerTransformer,
            CimKnownClass::BaseVoltage,
            CimKnownClass::ConnectivityNode,
            CimKnownClass::EnergyConsumer,
            CimKnownClass::PowerElectronicsConnection,
            CimKnownClass::PowerElectronicsUnit,
            CimKnownClass::ExternalNetworkInjection,
            CimKnownClass::Breaker,
            CimKnownClass::Disconnector,
            CimKnownClass::SynchronousMachine,
            CimKnownClass::Terminal,
            CimKnownClass::Substation,
            CimKnownClass::VoltageLevel,
            CimKnownClass::Bay,
        ] {
            assert_eq!(
                CimKnownClassVocabulary::recognize(known.name()),
                Some(known),
                "the vocabulary recognizes its own spelling: {}",
                known.name()
            );
        }
        assert_eq!(CimKnownClassVocabulary::recognize("NotACimClass"), None);
    }
}
