//! P14.12 — Class-mapping integration tests (decision record
//! 44_p14_12_cim_cgmes_import.md).
//!
//! WRITTEN-UNRUN: complete test files whose execution is a human gate
//! (conventions §1.3). These tests exercise the PUBLIC mapping surface
//! end-to-end: profile document → register → summary → network IR model,
//! the four classification statuses, the ambiguity gate, the
//! EXPLICITLY_UNMAPPED discipline, and the 61968-5 DER-group registry.
//! Nothing here reads a file — the RDF/XML source seam is unwired and
//! every profile in these tests is a constructed fixture.

use std::collections::BTreeMap;

use flexcore_cim_adapter::class_mapping::{
    BaseVoltageRegistry, CimNetworkMapping, ClassMappingStatus, MappingTarget,
};
use flexcore_cim_adapter::der_groups::{DerGroupDeclaration, DerGroupError, DerGroupRegistry};
use flexcore_cim_adapter::model::{
    CimMRid, CimObject, ProfileDocument, ATTR_BASE_VOLTAGE_NOMINAL_KV,
    ATTR_EXTERNAL_GRID_BUS_REF, ATTR_EXTERNAL_GRID_VA_DEGREE, ATTR_EXTERNAL_GRID_VM_PU,
    ATTR_LINE_FROM_BUS_REF, ATTR_LINE_RATED_CURRENT_A, ATTR_LINE_R_OHM, ATTR_LINE_TO_BUS_REF,
    ATTR_LINE_X_OHM, ATTR_NODE_BASE_VOLTAGE_REF,
};
use flexcore_cim_adapter::rdf_profile::{ProfileParseError, RdfProfileSource, UnwiredProfileSource};
use flexcore_cim_adapter::unmapped::{assemble_network_model, ImportRegister, ImportError};
use flexcore_grid_api::port::GridModelRef;

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

/// A small declared profile: two voltage levels, two buses, one line, one
/// slack, one DER connection, one load, one unknown class.
fn feeder_profile() -> ProfileDocument {
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
        object("_pec1", "PowerElectronicsConnection", &[]),
        object("_pec2", "PowerElectronicsConnection", &[]),
        object("_load1", "EnergyConsumer", &[]),
        object("_mystery1", "EnergyScheduling", &[]),
    ])
    .expect("valid profile")
}

// --- The RDF/XML seam -------------------------------------------------------

#[test]
fn the_profile_source_seam_is_unwired_and_names_it() {
    let source = UnwiredProfileSource;
    assert_eq!(
        source.parse(b"<rdf:RDF/>"),
        Err(ProfileParseError::SeamNotWired),
        "no RDF/XML parser dependency lands in this batch"
    );
}

// --- The classification discipline ------------------------------------------

#[test]
fn every_profile_object_is_classified_exactly_once() {
    let register = ImportRegister::classify_profile(&feeder_profile());
    assert_eq!(register.entries().count(), feeder_profile().len());
    for object in feeder_profile().objects() {
        assert!(
            register.status(object.mrid.value()).is_some(),
            "object {} has a classification",
            object.mrid.value()
        );
    }
    assert_eq!(register.ambiguous_count(), 0, "the fixture is unambiguous");
}

#[test]
fn the_four_statuses_appear_exactly_where_declared() {
    let register = ImportRegister::classify_profile(&feeder_profile());
    // MAPPED: the line, the buses, the external grid (BaseVoltage is
    // reference data — explicitly unmapped with its recorded reason).
    assert!(matches!(
        register.status("_line1"),
        Some(ClassMappingStatus::Mapped {
            target: MappingTarget::Branch(_)
        })
    ));
    assert!(matches!(
        register.status("_busA"),
        Some(ClassMappingStatus::Mapped {
            target: MappingTarget::Bus(_)
        })
    ));
    assert!(matches!(
        register.status("_grid1"),
        Some(ClassMappingStatus::Mapped {
            target: MappingTarget::ExternalGrid(_)
        })
    ));
    // EXPLICITLY_UNMAPPED: DER equipment and load (the recorded reasons).
    for mrid in ["_pec1", "_pec2", "_load1", "_bv20"] {
        assert!(matches!(
            register.status(mrid),
            Some(ClassMappingStatus::ExplicitlyUnmapped { .. })
        ));
    }
    // UNMAPPED: no vocabulary entry, recorded, never guessed.
    assert!(matches!(
        register.status("_mystery1"),
        Some(ClassMappingStatus::Unmapped { .. })
    ));
}

#[test]
fn an_ambiguity_blocks_finalize_and_names_the_object() {
    let broken = ProfileDocument::new(vec![
        object("_bv20", "BaseVoltage", &[(ATTR_BASE_VOLTAGE_NOMINAL_KV, "20.0")]),
        object("_busA", "ConnectivityNode", &[(ATTR_NODE_BASE_VOLTAGE_REF, "_bv20")]),
        object(
            "_line1",
            "ACLineSegment",
            &[
                (ATTR_LINE_R_OHM, "1.5"),
                (ATTR_LINE_X_OHM, "4.0"),
                // No rated current: the mapping refuses (the real CGMES
                // carrier may be OperationalLimitSet — UNVERIFIED check).
                (ATTR_LINE_FROM_BUS_REF, "_busA"),
                (ATTR_LINE_TO_BUS_REF, "_busA"),
            ],
        ),
    ])
    .expect("valid profile");
    let register = ImportRegister::classify_profile(&broken);
    assert_eq!(register.ambiguous_count(), 1);
    let error = register.finalize().expect_err("ambiguity blocks finalize");
    match error {
        ImportError::AmbiguousObjectsRemain { count, first_mrid } => {
            assert_eq!(count, 1);
            assert_eq!(first_mrid, "_line1", "the refusal names the object");
        }
        other => panic!("expected the ambiguity refusal, got {other:?}"),
    }
}

#[test]
fn a_guessed_voltage_is_never_a_bus() {
    let mapping = CimNetworkMapping::new();
    let empty_registry = BaseVoltageRegistry::new();
    let node = object(
        "_busX",
        "ConnectivityNode",
        &[(ATTR_NODE_BASE_VOLTAGE_REF, "_bv-not-declared")],
    );
    let status = mapping.classify(&node, &empty_registry);
    match &status {
        ClassMappingStatus::Ambiguous { reason } => {
            assert!(
                reason.contains("never guesses"),
                "the ambiguity states the no-guess rule: {reason}"
            );
        }
        other => panic!("expected an ambiguity, got {other:?}"),
    }
}

// --- Summary, coverage limitations, and assembly ----------------------------

#[test]
fn the_summary_travels_with_its_coverage_limitation_record() {
    let register = ImportRegister::classify_profile(&feeder_profile());
    let summary = register.finalize().expect("no ambiguity");
    let statements = summary.coverage_limitation_statements();
    assert!(
        statements
            .iter()
            .any(|s| s.starts_with("EXPLICITLY_UNMAPPED _pec1 (PowerElectronicsConnection)")),
        "the DER refusal travels verbatim: {statements:?}"
    );
    assert!(statements
        .iter()
        .any(|s| s.starts_with("UNMAPPED _mystery1 (EnergyScheduling)")));
}

#[test]
fn assembly_builds_a_valid_network_ir_from_mapped_elements_only() {
    let register = ImportRegister::classify_profile(&feeder_profile());
    let summary = register.finalize().expect("no ambiguity");
    let model_ref = GridModelRef {
        model_id: "dso-feeder-42".to_string(),
        version: "1".to_string(),
        content_hash: "declared-by-the-versioned-model-store".to_string(),
    };
    let model = assemble_network_model(&model_ref, 1.0, &summary).expect("valid model");
    assert_eq!(model.buses.len(), 2);
    assert_eq!(model.branches.len(), 1);
    assert_eq!(model.transformers.len(), 0);
    assert_eq!(model.external_grids.len(), 1);
    // The declared slack rule: the external grid's bus is the one slack.
    assert!(
        model
            .buses
            .iter()
            .filter(|bus| bus.is_slack)
            .all(|bus| bus.bus_id == "_busA")
            && model.buses.iter().any(|bus| bus.is_slack),
        "exactly the external grid's bus is slack"
    );
    // The DER and load objects contribute NOTHING to the network IR —
    // the model above has no element for _pec1/_pec2/_load1.
    assert!(
        !model
            .branches
            .iter()
            .any(|branch| branch.branch_id == "_pec1" || branch.branch_id == "_load1")
    );
}

#[test]
fn assembly_refuses_a_bad_model_identity_and_base() {
    let register = ImportRegister::classify_profile(&feeder_profile());
    let summary = register.finalize().expect("no ambiguity");
    let bad_ref = GridModelRef {
        model_id: String::new(),
        version: "1".to_string(),
        content_hash: "x".to_string(),
    };
    assert!(matches!(
        assemble_network_model(&bad_ref, 1.0, &summary),
        Err(ImportError::ModelRefInvalid { .. })
    ));
    let good_ref = GridModelRef {
        model_id: "m".to_string(),
        version: "1".to_string(),
        content_hash: "x".to_string(),
    };
    assert!(matches!(
        assemble_network_model(&good_ref, -1.0, &summary),
        Err(ImportError::InvalidBaseMva { .. })
    ));
}

// --- The 61968-5 DER-group registry ------------------------------------------

#[test]
fn der_groups_refuse_members_outside_the_profile() {
    let profile = feeder_profile();
    let register = ImportRegister::classify_profile(&profile);
    let declaration = DerGroupDeclaration::new(
        "south-fleet".to_string(),
        vec![
            CimMRid::new("_pec1").expect("valid"),
            CimMRid::new("_not-there").expect("valid"),
        ],
        Vec::new(),
    )
    .expect("valid declaration");
    assert_eq!(
        DerGroupRegistry::assemble(vec![declaration], &profile, &register),
        Err(DerGroupError::MemberNotInProfile {
            mrid: "_not-there".to_string(),
        })
    );
}

#[test]
fn der_groups_record_member_mapping_statuses_verbatim() {
    let profile = feeder_profile();
    let register = ImportRegister::classify_profile(&profile);
    let declaration = DerGroupDeclaration::new(
        "south-fleet".to_string(),
        vec![
            CimMRid::new("_pec1").expect("valid"),
            CimMRid::new("_pec2").expect("valid"),
        ],
        vec!["the declared DER fleet of feeder south".to_string()],
    )
    .expect("valid declaration");
    let registry =
        DerGroupRegistry::assemble(vec![declaration], &profile, &register).expect("valid");
    assert_eq!(registry.group_ids(), vec!["south-fleet"]);
    let members = registry.members("south-fleet").expect("present");
    for member in members {
        assert_eq!(member.class_name, "PowerElectronicsConnection");
        // The member's network-mapping status travels verbatim: the
        // registry surface and the network-IR surface cannot be confused.
        assert_eq!(member.network_mapping_status, "EXPLICITLY_UNMAPPED");
    }
}
