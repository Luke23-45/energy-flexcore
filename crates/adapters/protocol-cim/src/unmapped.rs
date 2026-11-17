// P14.12 — The EXPLICITLY_UNMAPPED register and the fail-closed import
// completeness gate (decision record 44_p14_12_cim_cgmes_import.md; the
// openems.rs mapping-register pattern; the P4.02 completeness-gate
// pattern).
//
// THE DISCIPLINE (the plan §2 P14.12 row, verbatim): **every unmapped CIM
// class is EXPLICITLY_UNMAPPED, never guessed.** Structural enforcement:
//
// 1. The register holds ONE classification for EVERY object of the
//    profile — keyed by mRID, ordered (BTreeMap — the determinism rule).
//    [`ImportRegister::classify_profile`] classifies the whole document
//    in one pass; there is no way to add an object without a status.
// 2. [`ImportRegister::finalize`] REFUSES a summary while any object
//    carries an AMBIGUOUS status: an ambiguity means the mapping refused
//    to guess, and an import that proceeded anyway would silently drop
//    the object's data. The refusal names every ambiguous object.
// 3. The summary carries the mapped element lists, the explicitly-unmapped
//    register (with reasons), and the coverage-limitation statements —
//    the reviewed limitation record travels with the import wherever its
//    outputs are consumed (the P4.02 coverage-limitation pattern).
// 4. The network-model assembly ([`assemble_network_model`]) builds the
//    grid-api `VersionedNetworkModel` from the MAPPED elements only, then
//    runs the IR's own structural validation — a mapped model that
//    references a bus that was never mapped fails NAMED, not silently.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use flexcore_grid_api::port::{
    BranchSpec, BusSpec, ExternalGridSpec, GridModelRef, TransformerSpec, VersionedNetworkModel,
};

use crate::class_mapping::{BaseVoltageRegistry, CimNetworkMapping, ClassMappingStatus,
    MappingTarget};
use crate::model::ProfileDocument;

/// Errors returned by the register and the assembly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportError {
    /// The import was finalized while ambiguous objects remained — the
    /// mapping refused to guess and the import must not proceed anyway.
    AmbiguousObjectsRemain {
        count: usize,
        first_mrid: String,
    },
    /// The profile's mapped elements do not assemble into a valid
    /// network model (no buses, a dangling branch reference, not exactly
    /// one external grid) — re-raised NAMED from the network IR's own
    /// validation.
    ModelInvalid {
        reason: String,
    },
    /// The caller-supplied model identity failed the IR's validation.
    ModelRefInvalid {
        reason: String,
    },
    /// The declared base MVA is not a finite positive number.
    InvalidBaseMva {
        observed: String,
    },
}

impl std::fmt::Display for ImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ImportError::AmbiguousObjectsRemain { count, first_mrid } => write!(
                f,
                "{count} ambiguous object(s) remain — the mapping refused to guess and \
                 the import must not proceed (first: {first_mrid})"
            ),
            ImportError::ModelInvalid { reason } => {
                write!(f, "the mapped elements do not assemble into a valid network model: {reason}")
            }
            ImportError::ModelRefInvalid { reason } => {
                write!(f, "the declared model identity is invalid: {reason}")
            }
            ImportError::InvalidBaseMva { observed } => {
                write!(f, "the declared base MVA '{observed}' is not a finite positive number")
            }
        }
    }
}

impl std::error::Error for ImportError {}

/// The import register: ONE classification per profile object, keyed by
/// mRID, ordered. The register is the reviewed record of what the import
/// did and did not map — and of WHY.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImportRegister {
    classifications: BTreeMap<String, (String, ClassMappingStatus)>,
}

impl ImportRegister {
    /// Classifies the WHOLE profile document in one pass (the mapping is
    /// applied to every object; there is no unclassified path). BaseVoltage
    /// objects are registered into the nominal-kV registry before the
    /// connectivity-node mapping consumes it.
    pub fn classify_profile(document: &ProfileDocument) -> Self {
        let mapping = CimNetworkMapping::new();
        // Pass 1: build the nominal-kV registry from BaseVoltage objects.
        let mut voltage_registry = BaseVoltageRegistry::new();
        for object in document.objects() {
            if object.class_name == "BaseVoltage" {
                // A BaseVoltage whose declared attribute is missing or
                // invalid registers NOTHING — the nodes referencing it
                // become AMBIGUOUS (named below), never defaulted.
                let _ = voltage_registry.register(object);
            }
        }
        // Pass 2: classify every object.
        let mut classifications = BTreeMap::new();
        for object in document.objects() {
            let status = mapping.classify(object, &voltage_registry);
            classifications.insert(
                object.mrid.value().to_string(),
                (object.class_name.clone(), status),
            );
        }
        Self { classifications }
    }

    /// Every (mRID, class, status) entry, deterministically ordered by
    /// mRID.
    pub fn entries(&self) -> impl Iterator<Item = (&str, &str, &ClassMappingStatus)> {
        self.classifications
            .iter()
            .map(|(mrid, (class, status))| (mrid.as_str(), class.as_str(), status))
    }

    /// The status of one object by mRID.
    pub fn status(&self, mrid: &str) -> Option<&ClassMappingStatus> {
        self.classifications.get(mrid).map(|(_, status)| status)
    }

    /// The count of ambiguous objects (the finalize gate's input).
    pub fn ambiguous_count(&self) -> usize {
        self.classifications
            .values()
            .filter(|(_, status)| matches!(status, ClassMappingStatus::Ambiguous { .. }))
            .count()
    }

    /// The fail-closed completeness gate: the summary exists only when
    /// every object is classified (structural — it always is) AND no
    /// ambiguity remains (the mapping never guessed, so the import never
    /// proceeds past a refusal).
    pub fn finalize(&self) -> Result<ModelImportSummary, ImportError> {
        let mut ambiguous = Vec::new();
        for (mrid, (class, status)) in &self.classifications {
            if let ClassMappingStatus::Ambiguous { reason } = status {
                ambiguous.push(format!("{mrid} ({class}): {reason}"));
            }
        }
        if !ambiguous.is_empty() {
            return Err(ImportError::AmbiguousObjectsRemain {
                count: ambiguous.len(),
                first_mrid: ambiguous[0]
                    .split(' ')
                    .next()
                    .unwrap_or("unknown")
                    .to_string(),
            });
        }
        let mut summary = ModelImportSummary::default();
        for (mrid, (class, status)) in &self.classifications {
            match status {
                ClassMappingStatus::Mapped { target } => summary.record_mapped(mrid, class, target),
                ClassMappingStatus::ExplicitlyUnmapped { reason } => {
                    summary.record_explicitly_unmapped(mrid, class, reason)
                }
                ClassMappingStatus::Unmapped { reason } => {
                    summary.record_unmapped(mrid, class, reason)
                }
                // Ambiguous objects were refused by the gate above; this
                // arm is a defensive skip, never a silent pass (the gate
                // makes the path unreachable for honest inputs).
                ClassMappingStatus::Ambiguous { .. } => continue,
            }
        }
        Ok(summary)
    }
}

/// The import summary: the mapped elements per target kind, the
/// explicitly-unmapped register (with reasons), the unmapped register,
/// and the coverage-limitation statements that travel with the import
/// (the P4.02 pattern).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ModelImportSummary {
    pub mapped_buses: Vec<(String, BusSpec)>,
    pub mapped_branches: Vec<(String, BranchSpec)>,
    pub mapped_transformers: Vec<(String, TransformerSpec)>,
    pub mapped_external_grids: Vec<(String, ExternalGridSpec)>,
    pub explicitly_unmapped: Vec<ExplicitlyUnmappedEntry>,
    pub unmapped: Vec<(String, String, String)>,
}

impl ModelImportSummary {
    fn record_mapped(&mut self, mrid: &str, class: &str, target: &MappingTarget) {
        match target {
            MappingTarget::Bus(spec) => {
                self.mapped_buses.push((mrid.to_string(), spec.clone()))
            }
            MappingTarget::Branch(spec) => {
                self.mapped_branches.push((mrid.to_string(), spec.clone()))
            }
            MappingTarget::Transformer(spec) => {
                self.mapped_transformers.push((mrid.to_string(), spec.clone()))
            }
            MappingTarget::ExternalGrid(spec) => {
                self.mapped_external_grids.push((mrid.to_string(), spec.clone()))
            }
        }
        let _ = class; // the class name is recoverable from the target kind
    }

    fn record_explicitly_unmapped(&mut self, mrid: &str, class: &str, reason: &str) {
        self.explicitly_unmapped.push(ExplicitlyUnmappedEntry {
            mrid: mrid.to_string(),
            class_name: class.to_string(),
            reason: reason.to_string(),
        });
    }

    fn record_unmapped(&mut self, mrid: &str, class: &str, reason: &str) {
        self.unmapped
            .push((mrid.to_string(), class.to_string(), reason.to_string()));
    }

    /// The coverage-limitation statements: every explicitly-unmapped and
    /// unmapped entry, as a human-readable record (surfaced verbatim
    /// wherever the import's outputs are consumed).
    pub fn coverage_limitation_statements(&self) -> Vec<String> {
        let mut statements = Vec::new();
        for entry in &self.explicitly_unmapped {
            statements.push(format!(
                "EXPLICITLY_UNMAPPED {} ({}): {}",
                entry.mrid, entry.class_name, entry.reason
            ));
        }
        for (mrid, class, reason) in &self.unmapped {
            statements.push(format!("UNMAPPED {mrid} ({class}): {reason}"));
        }
        statements
    }
}

/// One EXPLICITLY_UNMAPPED record: the object, its class, and the
/// recorded reason.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExplicitlyUnmappedEntry {
    pub mrid: String,
    pub class_name: String,
    pub reason: String,
}

/// Assembles the grid-api `VersionedNetworkModel` from the summary's
/// MAPPED elements only. The model identity (`GridModelRef`) and the
/// base MVA are DECLARED by the caller (the versioned-model store owns
/// the content hash — P4.03 — and this import never recomputes it); the
/// assembled model is validated by the IR's own structural validation
/// before it is returned.
///
/// The exactly-one-slack rule: the network IR requires exactly one
/// external grid at a slack bus. This import marks the referenced bus of
/// the FIRST mapped external grid as the slack (a declared rule, recorded
/// in the decision record); a profile whose mapped external grids
/// disagree with the IR's exactly-one rule fails the IR's validation
/// NAMED.
pub fn assemble_network_model(
    model_ref: &GridModelRef,
    base_mva: f64,
    summary: &ModelImportSummary,
) -> Result<VersionedNetworkModel, ImportError> {
    model_ref.validate().map_err(|error| ImportError::ModelRefInvalid {
        reason: error.to_string(),
    })?;
    if !base_mva.is_finite() || base_mva <= 0.0 {
        return Err(ImportError::InvalidBaseMva {
            observed: format!("{base_mva}"),
        });
    }
    let mut buses: Vec<BusSpec> = summary
        .mapped_buses
        .iter()
        .map(|(_, bus)| bus.clone())
        .collect();
    let external_grids: Vec<ExternalGridSpec> = summary
        .mapped_external_grids
        .iter()
        .map(|(_, grid)| grid.clone())
        .collect();
    // The declared slack rule: the first mapped external grid's bus is
    // the slack. The IR's validation enforces "exactly one external grid
    // at a slack bus" — a profile with zero or several external grids
    // fails there, NAMED.
    if let Some(grid) = external_grids.first() {
        for bus in &mut buses {
            if bus.bus_id == grid.bus_id {
                bus.is_slack = true;
            }
        }
    }
    let model = VersionedNetworkModel {
        model_ref: model_ref.clone(),
        base_mva,
        buses,
        branches: summary
            .mapped_branches
            .iter()
            .map(|(_, branch)| branch.clone())
            .collect(),
        transformers: summary
            .mapped_transformers
            .iter()
            .map(|(_, transformer)| transformer.clone())
            .collect(),
        external_grids,
    };
    model.validate().map_err(|error| ImportError::ModelInvalid {
        reason: error.to_string(),
    })?;
    Ok(model)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        CimObject, CimMRid,
        ATTR_BASE_VOLTAGE_NOMINAL_KV, ATTR_EXTERNAL_GRID_BUS_REF,
        ATTR_EXTERNAL_GRID_VA_DEGREE, ATTR_EXTERNAL_GRID_VM_PU, ATTR_LINE_FROM_BUS_REF,
        ATTR_LINE_RATED_CURRENT_A, ATTR_LINE_R_OHM, ATTR_LINE_TO_BUS_REF, ATTR_LINE_X_OHM,
        ATTR_NODE_BASE_VOLTAGE_REF,
    };
    use std::collections::BTreeMap;

    fn object(mrid: &str, class: &str, attributes: &[(&str, &str)]) -> CimObject {
        let mut map = BTreeMap::new();
        for (key, value) in attributes {
            map.insert((*key).to_string(), (*value).to_string());
        }
        CimObject {
            mrid: CimMRid::new(mrid).expect("valid"),
            class_name: class.to_string(),
            attributes: map,
        }
    }

    fn small_profile() -> ProfileDocument {
        ProfileDocument::new(vec![
            object("_bv20", "BaseVoltage", &[(ATTR_BASE_VOLTAGE_NOMINAL_KV, "20.0")]),
            object("_bv04", "BaseVoltage", &[(ATTR_BASE_VOLTAGE_NOMINAL_KV, "0.4")]),
            object("_busA", "ConnectivityNode", &[(ATTR_NODE_BASE_VOLTAGE_REF, "_bv20")]),
            object("_busB", "ConnectivityNode", &[(ATTR_NODE_BASE_VOLTAGE_REF, "_bv04")]),
            object(
                "_line1",
                "ACLineSegment",
                &[
                    (ATTR_LINE_R_OHM, "1.5"),
                    (ATTR_LINE_X_OHM, "4.0"),
                    (ATTR_LINE_RATED_CURRENT_A, "400"),
                    (ATTR_LINE_FROM_BUS_REF, "_busA"),
                    (ATTR_LINE_TO_BUS_REF, "_busB"),
                ],
            ),
            object(
                "_grid1",
                "ExternalNetworkInjection",
                &[
                    (ATTR_EXTERNAL_GRID_BUS_REF, "_busA"),
                    (ATTR_EXTERNAL_GRID_VM_PU, "1.02"),
                    (ATTR_EXTERNAL_GRID_VA_DEGREE, "0.0"),
                ],
            ),
            object("_load1", "EnergyConsumer", &[]),
            object("_pec1", "PowerElectronicsConnection", &[]),
            object("_mystery1", "EnergyScheduling", &[]),
        ])
        .expect("valid profile")
    }

    #[test]
    fn every_object_is_classified_exactly_once() {
        let register = ImportRegister::classify_profile(&small_profile());
        assert_eq!(register.entries().count(), small_profile().len());
        // Every object of the document has an entry with its class name.
        for object in small_profile().objects() {
            assert!(
                register.status(object.mrid.value()).is_some(),
                "object {} is classified",
                object.mrid.value()
            );
        }
    }

    #[test]
    fn the_register_records_mapped_explicitly_unmapped_and_unmapped() {
        let register = ImportRegister::classify_profile(&small_profile());
        assert!(matches!(
            register.status("_line1"),
            Some(ClassMappingStatus::Mapped { .. })
        ));
        assert!(matches!(
            register.status("_load1"),
            Some(ClassMappingStatus::ExplicitlyUnmapped { .. })
        ));
        assert!(matches!(
            register.status("_pec1"),
            Some(ClassMappingStatus::ExplicitlyUnmapped { .. })
        ));
        assert!(matches!(
            register.status("_mystery1"),
            Some(ClassMappingStatus::Unmapped { .. })
        ));
        assert_eq!(register.ambiguous_count(), 0);
    }

    #[test]
    fn finalize_gates_on_ambiguity_never_on_silence() {
        // A profile with a line whose rated current is missing: the node
        // and line classifications become AMBIGUOUS and finalize refuses.
        let broken = ProfileDocument::new(vec![
            object("_bv20", "BaseVoltage", &[(ATTR_BASE_VOLTAGE_NOMINAL_KV, "20.0")]),
            object("_busA", "ConnectivityNode", &[(ATTR_NODE_BASE_VOLTAGE_REF, "_bv20")]),
            object(
                "_line1",
                "ACLineSegment",
                &[
                    (ATTR_LINE_R_OHM, "1.5"),
                    (ATTR_LINE_X_OHM, "4.0"),
                    // No rated current — the mapping refuses to guess.
                    (ATTR_LINE_FROM_BUS_REF, "_busA"),
                    (ATTR_LINE_TO_BUS_REF, "_busA"),
                ],
            ),
        ])
        .expect("valid profile");
        let register = ImportRegister::classify_profile(&broken);
        assert_eq!(register.ambiguous_count(), 1);
        let error = register.finalize().expect_err("ambiguity blocks finalize");
        assert!(matches!(
            error,
            ImportError::AmbiguousObjectsRemain { count: 1, .. }
        ));
    }

    #[test]
    fn the_summary_carries_the_coverage_limitation_record() {
        let register = ImportRegister::classify_profile(&small_profile());
        let summary = register.finalize().expect("no ambiguity");
        let statements = summary.coverage_limitation_statements();
        // The DER and load refusals travel with the import, verbatim.
        assert!(statements
            .iter()
            .any(|s| s.starts_with("EXPLICITLY_UNMAPPED _pec1 (PowerElectronicsConnection)")));
        assert!(statements
            .iter()
            .any(|s| s.starts_with("EXPLICITLY_UNMAPPED _load1 (EnergyConsumer)")));
        assert!(statements
            .iter()
            .any(|s| s.starts_with("UNMAPPED _mystery1 (EnergyScheduling)")));
    }

    #[test]
    fn assembly_produces_a_valid_network_ir_model() {
        let register = ImportRegister::classify_profile(&small_profile());
        let summary = register.finalize().expect("no ambiguity");
        let model_ref = GridModelRef {
            model_id: "dso-feeder-42".to_string(),
            version: "1".to_string(),
            content_hash: "declared-by-the-versioned-model-store".to_string(),
        };
        let model = assemble_network_model(&model_ref, 1.0, &summary).expect("valid model");
        assert_eq!(model.buses.len(), 2);
        assert_eq!(model.branches.len(), 1);
        // The declared slack rule: the external grid's bus is slack.
        let slack = model
            .buses
            .iter()
            .find(|bus| bus.bus_id == "_busA")
            .expect("bus A present");
        assert!(slack.is_slack, "the external grid's bus is the declared slack");
        assert!(
            !model
                .buses
                .iter()
                .filter(|bus| bus.is_slack)
                .any(|bus| bus.bus_id != "_busA"),
            "exactly one slack"
        );
    }

    #[test]
    fn assembly_refuses_invalid_declarations_and_dangling_models() {
        let register = ImportRegister::classify_profile(&small_profile());
        let summary = register.finalize().expect("no ambiguity");
        let bad_ref = GridModelRef {
            model_id: "  ".to_string(),
            version: "1".to_string(),
            content_hash: "x".to_string(),
        };
        assert!(matches!(
            assemble_network_model(&bad_ref, 1.0, &summary),
            Err(ImportError::ModelRefInvalid { .. })
        ));
        let good_ref = GridModelRef {
            model_id: "dso-feeder-42".to_string(),
            version: "1".to_string(),
            content_hash: "declared".to_string(),
        };
        assert!(matches!(
            assemble_network_model(&good_ref, 0.0, &summary),
            Err(ImportError::InvalidBaseMva { .. })
        ));
        // A summary whose branch references an unmapped bus fails the
        // IR's validation, NAMED.
        let mut dangling = summary.clone();
        dangling.mapped_buses.clear();
        dangling.mapped_external_grids.clear();
        let error =
            assemble_network_model(&good_ref, 1.0, &dangling).expect_err("dangling refs");
        assert!(matches!(error, ImportError::ModelInvalid { .. }));
    }
}
