//! FIM-F06.06 — the pinned CGMES import suite
//! (docs/releases/implementation_ledger/ledger_F06.md; decision record
//! 44_p14_12_cim_cgmes_import.md).
//!
//! **WRITTEN-UNRUN.** These tests have never been compiled or executed —
//! the no-build/no-test bans (program conventions §1) make execution a
//! HUMAN GATE. Nothing here is a conformance result: interop with a real
//! DSO CGMES profile is UNPROVEN and remains a HUMAN LAB GATE (crate
//! conventions §7.3).
//!
//! Every test is DETERMINISTIC, SOCKET-FREE and FIXTURE-DRIVEN: the
//! profiles are hand-built miniature CGMES-shaped documents below (no file
//! I/O, no network, no environment gate — unlike the live-transport suites
//! of the other protocol crates, nothing external is needed to run this
//! file). The fixtures exercise the DECLARED class vocabulary through
//! `rdf:ID` / `rdf:resource` cross-references, the import mapping into the
//! grid-api IR, the EXISTING completeness gate, the EXPLICITLY_UNMAPPED /
//! UNMAPPED recording, the parser's entity handling, and the named
//! malformed/bounded-input refusals.
//!
//! The attribute spellings inside the fixtures are the DECLARED mapping
//! keys of `crate::model` (the P14.12 record's constants) — they pin the
//! normalization and the mapping, NOT the CGMES schema text. The
//! spelling-verification status of every declared constant is recorded in
//! `docs/phases-11-20/phase-14-protocol-device-connectivity/records/
//! 44a_p14_12_fim_class_spelling_verification.md` (FIM-F06.05).

use flexcore_cim_adapter::model::{
    CimMRid, ATTR_BASE_VOLTAGE_NOMINAL_KV, ATTR_EXTERNAL_GRID_BUS_REF,
    ATTR_EXTERNAL_GRID_VA_DEGREE, ATTR_EXTERNAL_GRID_VM_PU, ATTR_LINE_FROM_BUS_REF,
    ATTR_LINE_RATED_CURRENT_A, ATTR_LINE_R_OHM, ATTR_LINE_TO_BUS_REF, ATTR_LINE_X_OHM,
    ATTR_NODE_BASE_VOLTAGE_REF, ATTR_TRANSFORMER_HV_BUS_REF, ATTR_TRANSFORMER_LV_BUS_REF,
    ATTR_TRANSFORMER_RATED_S_VA, ATTR_TRANSFORMER_VK_PERCENT, ATTR_TRANSFORMER_VK_R_PERCENT,
};
use flexcore_cim_adapter::rdf_profile::ProfileParseError;
use flexcore_cim_adapter::unmapped::ImportError;
use flexcore_cim_adapter::xml_profile::{
    import_profile_bytes, ProfileImportError, XmlRdfProfileSource,
};
use flexcore_grid_api::port::GridModelRef;

/// The source under test (the module's bounded-input guard).
fn source() -> XmlRdfProfileSource {
    XmlRdfProfileSource::new()
}

/// The declared model identity the import path consumes (the versioned
/// model store owns the content hash — this import never recomputes it).
fn model_ref() -> GridModelRef {
    GridModelRef {
        model_id: "dso-feeder-42".to_string(),
        version: "1".to_string(),
        content_hash: "declared-by-the-versioned-model-store".to_string(),
    }
}

/// The hand-built miniature CGMES-shaped profile (FIM-F06.06 fixture): two
/// voltage levels, two buses, one line, one two-winding transformer, one
/// external grid with cross-references, one DER connection, one load, one
/// unknown class. The `&#46;` character reference inside the slack voltage
/// pins the entity handling THROUGH the full import path ("1&#46;02" →
/// "1.02").
const MINIATURE_CGMES_PROFILE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<rdf:RDF xmlns:cim="http://iec.ch/TC57/2013/CIM-schema-cim16#"
         xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
  <cim:BaseVoltage rdf:ID="_bv20">
    <cim:BaseVoltage.nominalV>20.0</cim:BaseVoltage.nominalV>
  </cim:BaseVoltage>
  <cim:BaseVoltage rdf:ID="_bv04">
    <cim:BaseVoltage.nominalV>0.4</cim:BaseVoltage.nominalV>
  </cim:BaseVoltage>
  <cim:ConnectivityNode rdf:ID="_busA">
    <cim:ConnectivityNode.BaseVoltage rdf:resource="#_bv20" />
  </cim:ConnectivityNode>
  <cim:ConnectivityNode rdf:ID="_busB">
    <cim:ConnectivityNode.BaseVoltage rdf:resource="#_bv04" />
  </cim:ConnectivityNode>
  <cim:ACLineSegment rdf:ID="_line1">
    <cim:ACLineSegment.r>1.5</cim:ACLineSegment.r>
    <cim:ACLineSegment.x>4.0</cim:ACLineSegment.x>
    <cim:ACLineSegment.ratedCurrent>400</cim:ACLineSegment.ratedCurrent>
    <cim:ACLineSegment.fromBus rdf:resource="#_busA" />
    <cim:ACLineSegment.toBus rdf:resource="#_busB" />
  </cim:ACLineSegment>
  <cim:PowerTransformer rdf:ID="_xfmr1">
    <cim:PowerTransformer.ratedS>630000</cim:PowerTransformer.ratedS>
    <cim:PowerTransformer.vkPercent>6.0</cim:PowerTransformer.vkPercent>
    <cim:PowerTransformer.vkRPercent>1.0</cim:PowerTransformer.vkRPercent>
    <cim:PowerTransformer.hvBus rdf:resource="#_busA" />
    <cim:PowerTransformer.lvBus rdf:resource="#_busB" />
  </cim:PowerTransformer>
  <cim:ExternalNetworkInjection rdf:ID="_grid1">
    <cim:ExternalNetworkInjection.bus rdf:resource="#_busA" />
    <cim:ExternalNetworkInjection.vmPu>1&#46;02</cim:ExternalNetworkInjection.vmPu>
    <cim:ExternalNetworkInjection.vaDegree>0.0</cim:ExternalNetworkInjection.vaDegree>
  </cim:ExternalNetworkInjection>
  <cim:EnergyConsumer rdf:ID="_load1" />
  <cim:PowerElectronicsConnection rdf:ID="_pec1" />
  <cim:EnergyScheduling rdf:ID="_mystery1" />
</rdf:RDF>
"#;

/// The same profile with the line's declared rated-current attribute
/// REMOVED (the mapping refuses to guess, the EXISTING gate refuses the
/// import — the FIM-F06.03 pin).
const AMBIGUOUS_PROFILE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<rdf:RDF xmlns:cim="http://iec.ch/TC57/2013/CIM-schema-cim16#"
         xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
  <cim:BaseVoltage rdf:ID="_bv20">
    <cim:BaseVoltage.nominalV>20.0</cim:BaseVoltage.nominalV>
  </cim:BaseVoltage>
  <cim:ConnectivityNode rdf:ID="_busA">
    <cim:ConnectivityNode.BaseVoltage rdf:resource="#_bv20" />
  </cim:ConnectivityNode>
  <cim:ACLineSegment rdf:ID="_line1">
    <cim:ACLineSegment.r>1.5</cim:ACLineSegment.r>
    <cim:ACLineSegment.x>4.0</cim:ACLineSegment.x>
    <cim:ACLineSegment.fromBus rdf:resource="#_busA" />
    <cim:ACLineSegment.toBus rdf:resource="#_busA" />
  </cim:ACLineSegment>
</rdf:RDF>
"#;

/// The SAME objects as the miniature fixture, in a DIFFERENT element order
/// (the determinism-rule pin: the normalized document must not depend on
/// the document's element order).
const REORDERED_MINIATURE_CGMES_PROFILE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<rdf:RDF xmlns:cim="http://iec.ch/TC57/2013/CIM-schema-cim16#"
         xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
  <cim:EnergyScheduling rdf:ID="_mystery1" />
  <cim:PowerElectronicsConnection rdf:ID="_pec1" />
  <cim:EnergyConsumer rdf:ID="_load1" />
  <cim:ExternalNetworkInjection rdf:ID="_grid1">
    <cim:ExternalNetworkInjection.bus rdf:resource="#_busA" />
    <cim:ExternalNetworkInjection.vmPu>1&#46;02</cim:ExternalNetworkInjection.vmPu>
    <cim:ExternalNetworkInjection.vaDegree>0.0</cim:ExternalNetworkInjection.vaDegree>
  </cim:ExternalNetworkInjection>
  <cim:PowerTransformer rdf:ID="_xfmr1">
    <cim:PowerTransformer.hvBus rdf:resource="#_busA" />
    <cim:PowerTransformer.lvBus rdf:resource="#_busB" />
    <cim:PowerTransformer.ratedS>630000</cim:PowerTransformer.ratedS>
    <cim:PowerTransformer.vkPercent>6.0</cim:PowerTransformer.vkPercent>
    <cim:PowerTransformer.vkRPercent>1.0</cim:PowerTransformer.vkRPercent>
  </cim:PowerTransformer>
  <cim:ACLineSegment rdf:ID="_line1">
    <cim:ACLineSegment.toBus rdf:resource="#_busB" />
    <cim:ACLineSegment.fromBus rdf:resource="#_busA" />
    <cim:ACLineSegment.ratedCurrent>400</cim:ACLineSegment.ratedCurrent>
    <cim:ACLineSegment.x>4.0</cim:ACLineSegment.x>
    <cim:ACLineSegment.r>1.5</cim:ACLineSegment.r>
  </cim:ACLineSegment>
  <cim:ConnectivityNode rdf:ID="_busB">
    <cim:ConnectivityNode.BaseVoltage rdf:resource="#_bv04" />
  </cim:ConnectivityNode>
  <cim:ConnectivityNode rdf:ID="_busA">
    <cim:ConnectivityNode.BaseVoltage rdf:resource="#_bv20" />
  </cim:ConnectivityNode>
  <cim:BaseVoltage rdf:ID="_bv04">
    <cim:BaseVoltage.nominalV>0.4</cim:BaseVoltage.nominalV>
  </cim:BaseVoltage>
  <cim:BaseVoltage rdf:ID="_bv20">
    <cim:BaseVoltage.nominalV>20.0</cim:BaseVoltage.nominalV>
  </cim:BaseVoltage>
</rdf:RDF>
"#;

/// The fixture without its root close tag (the unclosed-element refusal
/// fixture; the trailing newline before the root close is kept so the
/// object elements themselves remain intact).
fn fixture_without_root_close() -> String {
    MINIATURE_CGMES_PROFILE
        .trim_end()
        .trim_end_matches("</rdf:RDF>")
        .to_string()
}

// --- The fixture parses and normalizes (FIM-F06.02) --------------------------

#[test]
fn the_miniature_profile_normalizes_into_the_cim_object_vocabulary() {
    let report = source()
        .parse_reported(MINIATURE_CGMES_PROFILE.as_bytes())
        .expect("the miniature profile is well-formed for the declared subset");
    assert_eq!(
        report.warnings.len(),
        0,
        "every predicate of the fixture is a declared key: {:?}",
        report.warnings
    );
    // Ten objects: two BaseVoltage, two ConnectivityNode, the line, the
    // transformer, the external grid, the load, the DER connection, the
    // unknown class.
    assert_eq!(report.document.len(), 10);
    let line = report
        .document
        .object(&CimMRid::new("_line1").expect("valid"))
        .expect("the line is present");
    assert_eq!(line.class_name, "ACLineSegment");
    assert_eq!(line.attribute(ATTR_LINE_R_OHM), Some("1.5"));
    assert_eq!(line.attribute(ATTR_LINE_X_OHM), Some("4.0"));
    assert_eq!(line.attribute(ATTR_LINE_RATED_CURRENT_A), Some("400"));
    assert_eq!(
        line.attribute(ATTR_LINE_FROM_BUS_REF),
        Some("_busA"),
        "the rdf:resource cross-reference becomes the referenced mRID"
    );
    assert_eq!(line.attribute(ATTR_LINE_TO_BUS_REF), Some("_busB"));
    let bus = report
        .document
        .object(&CimMRid::new("_busA").expect("valid"))
        .expect("bus A is present");
    assert_eq!(bus.class_name, "ConnectivityNode");
    assert_eq!(bus.attribute(ATTR_NODE_BASE_VOLTAGE_REF), Some("_bv20"));
    let base_voltage = report
        .document
        .object(&CimMRid::new("_bv20").expect("valid"))
        .expect("the BaseVoltage is present");
    assert_eq!(base_voltage.attribute(ATTR_BASE_VOLTAGE_NOMINAL_KV), Some("20.0"));
    let grid = report
        .document
        .object(&CimMRid::new("_grid1").expect("valid"))
        .expect("the external grid is present");
    assert_eq!(
        grid.attribute(ATTR_EXTERNAL_GRID_VM_PU),
        Some("1.02"),
        "the &#46; character reference decoded through the full path"
    );
    assert_eq!(grid.attribute(ATTR_EXTERNAL_GRID_VA_DEGREE), Some("0.0"));
    assert_eq!(grid.attribute(ATTR_EXTERNAL_GRID_BUS_REF), Some("_busA"));
    let transformer = report
        .document
        .object(&CimMRid::new("_xfmr1").expect("valid"))
        .expect("the transformer is present");
    assert_eq!(transformer.attribute(ATTR_TRANSFORMER_RATED_S_VA), Some("630000"));
    assert_eq!(transformer.attribute(ATTR_TRANSFORMER_VK_PERCENT), Some("6.0"));
    assert_eq!(transformer.attribute(ATTR_TRANSFORMER_VK_R_PERCENT), Some("1.0"));
    assert_eq!(transformer.attribute(ATTR_TRANSFORMER_HV_BUS_REF), Some("_busA"));
    assert_eq!(transformer.attribute(ATTR_TRANSFORMER_LV_BUS_REF), Some("_busB"));
}

#[test]
fn the_normalization_is_independent_of_document_element_order() {
    // The determinism rule (P14.12 record §1 rule 4): a REORDERED fixture
    // (the same objects in a different element order) normalizes to the
    // SAME document — equality, not just shape.
    let report = source()
        .parse_reported(MINIATURE_CGMES_PROFILE.as_bytes())
        .expect("the fixture is well-formed for the declared subset");
    let reordered_report = source()
        .parse_reported(REORDERED_MINIATURE_CGMES_PROFILE.as_bytes())
        .expect("the reordered fixture is well-formed for the declared subset");
    assert_eq!(report.document, reordered_report.document);
}

// --- The import path through the grid-api IR (FIM-F06.02/03/04) --------------

#[test]
fn the_import_assembles_the_versioned_network_model_through_the_ir() {
    let imported = import_profile_bytes(
        &source(),
        MINIATURE_CGMES_PROFILE.as_bytes(),
        &model_ref(),
        1.0,
    )
    .expect("the miniature profile imports");
    let model = &imported.model;
    assert_eq!(model.buses.len(), 2, "the two ConnectivityNodes are the buses");
    assert_eq!(model.branches.len(), 1, "the ACLineSegment is the branch");
    assert_eq!(model.transformers.len(), 1, "the PowerTransformer is the transformer");
    assert_eq!(model.external_grids.len(), 1, "the ExternalNetworkInjection is the grid");
    // The declared VA→MVA canonical-unit conversion happened at the mapping
    // (the canonical-unit rule; no scaled unit crosses into the IR).
    assert_eq!(model.transformers[0].rated_mva, 0.63);
    assert_eq!(model.transformers[0].vk_percent, 6.0);
    assert_eq!(model.transformers[0].vk_r_percent, 1.0);
    // The declared slack rule: the external grid's bus is the one slack.
    assert_eq!(model.external_grids[0].bus_id, "_busA");
    let slack_count = model.buses.iter().filter(|bus| bus.is_slack).count();
    assert_eq!(slack_count, 1, "exactly one slack");
    assert!(model
        .buses
        .iter()
        .find(|bus| bus.bus_id == "_busA")
        .expect("bus A present")
        .is_slack);
}

#[test]
fn the_completeness_gate_refuses_the_import_while_any_ambiguous_object_remains() {
    // FIM-F06.03: the refusal is the EXISTING gate's vocabulary
    // (ImportError::AmbiguousObjectsRemain), reached through the import
    // path from raw bytes.
    let error = import_profile_bytes(
        &source(),
        AMBIGUOUS_PROFILE.as_bytes(),
        &model_ref(),
        1.0,
    )
    .expect_err("the ambiguity blocks the import");
    let ProfileImportError::Import(ImportError::AmbiguousObjectsRemain {
        count,
        first_mrid,
    }) = error
    else {
        panic!("expected the existing gate's ambiguity refusal, got {error:?}");
    };
    assert_eq!(count, 1);
    assert_eq!(first_mrid, "_line1", "the refusal names the object");
}

#[test]
fn unmapped_and_explicitly_unmapped_classes_are_recorded_never_guessed() {
    // FIM-F06.04: the recording happens through the EXISTING
    // crate::unmapped surface — the DER connection and the load are
    // EXPLICITLY_UNMAPPED with their recorded reasons, the unknown class is
    // UNMAPPED, and all three travel in the coverage-limitation statements.
    let imported = import_profile_bytes(
        &source(),
        MINIATURE_CGMES_PROFILE.as_bytes(),
        &model_ref(),
        1.0,
    )
    .expect("the miniature profile imports");
    let statements = imported.summary.coverage_limitation_statements();
    assert!(
        statements
            .iter()
            .any(|s| s.starts_with("EXPLICITLY_UNMAPPED _pec1 (PowerElectronicsConnection)")),
        "the DER refusal travels verbatim: {statements:?}"
    );
    assert!(
        statements
            .iter()
            .any(|s| s.starts_with("EXPLICITLY_UNMAPPED _load1 (EnergyConsumer)")),
        "the load refusal travels verbatim"
    );
    assert!(
        statements
            .iter()
            .any(|s| s.starts_with("EXPLICITLY_UNMAPPED _bv20 (BaseVoltage)")),
        "the BaseVoltage reference-data disposition travels verbatim"
    );
    assert!(
        statements
            .iter()
            .any(|s| s.starts_with("UNMAPPED _mystery1 (EnergyScheduling)")),
        "the unknown class is recorded, never guessed"
    );
    // The DER/load objects contributed NOTHING to the network IR.
    assert!(
        !model_element_ids(&imported).iter().any(|id| id == "_pec1" || id == "_load1"),
        "the explicitly-unmapped objects never become network-IR elements"
    );
}

/// Every element id the assembled model carries (buses, branches,
/// transformers, the external grid's bus).
fn model_element_ids(imported: &flexcore_cim_adapter::xml_profile::ModelImport) -> Vec<String> {
    let model = &imported.model;
    let mut ids: Vec<String> = model.buses.iter().map(|b| b.bus_id.clone()).collect();
    ids.extend(model.branches.iter().map(|b| b.branch_id.clone()));
    ids.extend(model.transformers.iter().map(|t| t.transformer_id.clone()));
    ids.extend(model.external_grids.iter().map(|g| g.bus_id.clone()));
    ids
}

#[test]
fn a_dropped_predicate_is_recorded_as_a_parse_warning_and_never_a_key() {
    let with_bogus = MINIATURE_CGMES_PROFILE.replace(
        "    <cim:ACLineSegment.ratedCurrent>400</cim:ACLineSegment.ratedCurrent>\n",
        "    <cim:ACLineSegment.ratedCurrent>400</cim:ACLineSegment.ratedCurrent>\n    \
         <cim:ACLineSegment.bogusPredicate>9</cim:ACLineSegment.bogusPredicate>\n",
    );
    let report = source()
        .parse_reported(with_bogus.as_bytes())
        .expect("the profile still parses (a dropped predicate is not an error)");
    assert_eq!(report.document.len(), 10);
    assert_eq!(report.warnings.len(), 1);
    assert_eq!(report.warnings[0].object_mrid, "_line1");
    assert_eq!(report.warnings[0].dropped_element, "ACLineSegment.bogusPredicate");
    // The import proceeds on the DECLARED keys only.
    let imported = import_profile_bytes(&source(), with_bogus.as_bytes(), &model_ref(), 1.0)
        .expect("the import uses the declared keys only");
    assert_eq!(imported.model.branches.len(), 1);
}

// --- Named refusals: entities, malformed documents, the guard ----------------

#[test]
fn unknown_entities_and_control_references_are_named_refusals() {
    let unknown = MINIATURE_CGMES_PROFILE.replace(">20.0<", ">&nbsp;<");
    assert!(matches!(
        import_profile_bytes(&source(), unknown.as_bytes(), &model_ref(), 1.0),
        Err(ProfileImportError::Parse(ProfileParseError::NotWellFormed { .. }))
    ));
    let control = MINIATURE_CGMES_PROFILE.replace(">20.0<", ">&#7;<");
    assert!(matches!(
        source().parse_reported(control.as_bytes()),
        Err(ProfileParseError::NotWellFormed { .. })
    ));
}

#[test]
fn cdata_dtd_and_xinclude_are_named_refusals_through_the_seam() {
    let cdata = MINIATURE_CGMES_PROFILE.replace(">20.0<", "><![CDATA[20.0]]><");
    assert!(matches!(
        source().parse_reported(cdata.as_bytes()),
        Err(ProfileParseError::NotWellFormed { .. })
    ));
    let with_dtd = format!(
        "<!DOCTYPE rdf:RDF>\n{}",
        MINIATURE_CGMES_PROFILE
    );
    assert!(matches!(
        source().parse_reported(with_dtd.as_bytes()),
        Err(ProfileParseError::NotWellFormed { .. })
    ));
    let xinclude = MINIATURE_CGMES_PROFILE.replace(
        "  <cim:BaseVoltage rdf:ID=\"_bv20\">",
        "  <xi:include xmlns:xi=\"http://www.w3.org/2001/XInclude\" href=\"other.xml\" />\n  \
         <cim:BaseVoltage rdf:ID=\"_bv20\">",
    );
    assert!(matches!(
        source().parse_reported(xinclude.as_bytes()),
        Err(ProfileParseError::NotWellFormed { .. })
    ));
}

#[test]
fn malformed_documents_are_named_refusals_never_partial_successes() {
    // Unclosed element (the root never closes).
    let unclosed = fixture_without_root_close();
    assert!(matches!(
        source().parse_reported(unclosed.as_bytes()),
        Err(ProfileParseError::NotWellFormed { .. })
    ));
    // Mismatched close tag.
    let mismatched = MINIATURE_CGMES_PROFILE.replace("</cim:BaseVoltage>", "</cim:NotBaseVoltage>");
    assert!(matches!(
        source().parse_reported(mismatched.as_bytes()),
        Err(ProfileParseError::NotWellFormed { .. })
    ));
    // Duplicate attribute on one start tag.
    let duplicated = MINIATURE_CGMES_PROFILE.replace(
        "<cim:BaseVoltage rdf:ID=\"_bv20\">",
        "<cim:BaseVoltage rdf:ID=\"_bv20\" rdf:ID=\"_bv20\">",
    );
    assert!(matches!(
        source().parse_reported(duplicated.as_bytes()),
        Err(ProfileParseError::NotWellFormed { .. })
    ));
    // An undeclared prefix on an element (the 'x' prefix binds nothing).
    let with_undeclared = format!(
        "{}\n  <x:Breaker rdf:ID=\"_brk1\" />\n</rdf:RDF>",
        fixture_without_root_close()
    );
    assert!(matches!(
        source().parse_reported(with_undeclared.as_bytes()),
        Err(ProfileParseError::NotWellFormed { .. })
    ));
    // Stray non-whitespace text outside the root.
    let stray = format!("junk\n{}", MINIATURE_CGMES_PROFILE);
    assert!(matches!(
        source().parse_reported(stray.as_bytes()),
        Err(ProfileParseError::NotWellFormed { .. })
    ));
    // Not an RDF root at all.
    assert!(matches!(
        source().parse_reported(b"<schedule/>"),
        Err(ProfileParseError::NotACimProfile { .. })
    ));
    // Duplicate mRID across two object elements (the model layer's named
    // error re-raised through the seam).
    let duplicated_mrid = MINIATURE_CGMES_PROFILE.replace(
        "  <cim:EnergyConsumer rdf:ID=\"_load1\" />",
        "  <cim:EnergyConsumer rdf:ID=\"_load1\" />\n  \
         <cim:EnergyConsumer rdf:ID=\"_load1\" />",
    );
    assert!(matches!(
        source().parse_reported(duplicated_mrid.as_bytes()),
        Err(ProfileParseError::Model(
            flexcore_cim_adapter::model::CimModelError::DuplicateMRid { .. }
        ))
    ));
}

#[test]
fn the_bounded_input_guard_refuses_oversized_input_before_any_scan() {
    // The explicit-limit constructor (the pinned seam for the guard; the
    // module default is MAX_XML_INPUT_BYTES, far above every fixture here).
    let bounded = XmlRdfProfileSource::with_max_input_bytes(256);
    let error = bounded
        .parse_reported(MINIATURE_CGMES_PROFILE.as_bytes())
        .expect_err("the guard refuses the fixture at a 256-byte ceiling");
    let ProfileParseError::NotWellFormed { reason } = error else {
        panic!("expected a NotWellFormed refusal");
    };
    assert!(
        reason.contains("bounded-input guard"),
        "the human-safe reason names the guard and the sizes: {reason}"
    );
    // The same source at the module default parses the fixture.
    assert!(source()
        .parse_reported(MINIATURE_CGMES_PROFILE.as_bytes())
        .is_ok());
}
