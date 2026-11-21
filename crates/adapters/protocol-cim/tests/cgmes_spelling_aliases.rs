//! FIM wave 8B (Agent B) — the pinned CGMES verified-spelling ALIAS suite
//! (record 44a_p14_12_fim_class_spelling_verification.md §3 and its
//! 2026-11-21 addendum; the remediation lives in
//! `flexcore_cim_adapter::spelling_remediation`).
//!
//! **WRITTEN-UNRUN.** These tests have never been compiled or executed —
//! the no-build/no-test bans (program conventions §1) make execution a
//! HUMAN GATE. Nothing here is a conformance result: interop with a real
//! DSO CGMES profile is UNPROVEN and remains a HUMAN LAB GATE (crate
//! conventions §7.3).
//!
//! WHAT IS PINNED (the wave-8B remediation pins, all ADDITIVE — the
//! existing pins of `tests/cgmes_import.rs` are untouched and stay true):
//!
//! 1. ALIAS ACCEPTED — a fixture carrying a VERIFIED CGMES spelling
//!    (record 44a §3's replacement spellings) normalizes to the SAME
//!    document and imports to the SAME network-IR model as the declared-
//!    spelling fixture, with ZERO parse warnings.
//! 2. DECLARED KEYS STILL ACCEPTED — every declared spelling parses and
//!    imports exactly as before the remediation (the old pin, restated on
//!    the alias-test fixtures' declared-spelling twins).
//! 3. A DUPLICATE UNDER TWO ACCEPTED SPELLINGS IS STILL A DUPLICATE — the
//!    declared spelling and its alias on ONE object refuse (the existing
//!    duplicate-predicate refusal fires on the RESOLVED key).
//! 4. UNKNOWN SPELLINGS STILL REFUSED — a spelling accepted by neither the
//!    alias table nor the declared vocabulary is still DROPPED with its
//!    recorded parse warning (never a key), and the class-level recording
//!    (EXPLICITLY_UNMAPPED / UNMAPPED) is unchanged through alias-carrying
//!    fixtures.
//! 5. THE HONEST COUNTS — 2 accepted aliases + 9 verified-absent entries
//!    = 11 = every MISMATCH (verified) verdict of record 44a §3. The
//!    wave task's "alias count == 9" is NOT honestly satisfiable (the 9 is
//!    the verified-absent count; see the remediation module header); these
//!    pins state the arithmetic the record actually supports.
//! 6. VERIFIED-ABSENT KEYS ARE NOT ALIASES — the register carries the 9
//!    named not-modelled statements, and the absent declared keys remain
//!    accepted DECLARED vocabulary (the import vocabulary never shrank).

use flexcore_cim_adapter::model::{
    CimMRid, ATTR_BASE_VOLTAGE_NOMINAL_KV, ATTR_EXTERNAL_GRID_BUS_REF,
    ATTR_EXTERNAL_GRID_VA_DEGREE, ATTR_EXTERNAL_GRID_VM_PU, ATTR_NODE_BASE_VOLTAGE_REF,
    ATTR_TRANSFORMER_HV_BUS_REF, ATTR_TRANSFORMER_LV_BUS_REF, ATTR_TRANSFORMER_RATED_S_VA,
    ATTR_TRANSFORMER_VK_PERCENT, ATTR_TRANSFORMER_VK_R_PERCENT,
};
use flexcore_cim_adapter::spelling_remediation::{
    alias_count, resolve_attribute_key, verified_absent_count, verified_absent_statements,
};
use flexcore_cim_adapter::xml_profile::{import_profile_bytes, XmlRdfProfileSource};
use flexcore_grid_api::port::GridModelRef;

fn source() -> XmlRdfProfileSource {
    XmlRdfProfileSource::new()
}

fn model_ref() -> GridModelRef {
    GridModelRef {
        model_id: "dso-feeder-42".to_string(),
        version: "1".to_string(),
        content_hash: "declared-by-the-versioned-model-store".to_string(),
    }
}

const RDF_OPEN: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<rdf:RDF xmlns:cim=\"http://iec.ch/TC57/2013/CIM-schema-cim16#\"\n\
         xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">";
const RDF_CLOSE: &str = "</rdf:RDF>";

/// The minimal importable profile around one BaseVoltage: a node and the
/// external grid (the IR's exactly-one-slack rule).
fn base_voltage_profile(nominal_element: &str) -> String {
    format!(
        "{RDF_OPEN}\n  \
        <cim:BaseVoltage rdf:ID=\"_bv20\">\n    \
        {nominal_element}\n  \
        </cim:BaseVoltage>\n  \
        <cim:ConnectivityNode rdf:ID=\"_busA\">\n    \
        <cim:ConnectivityNode.BaseVoltage rdf:resource=\"#_bv20\" />\n  \
        </cim:ConnectivityNode>\n  \
        <cim:ExternalNetworkInjection rdf:ID=\"_grid1\">\n    \
        <cim:ExternalNetworkInjection.bus rdf:resource=\"#_busA\" />\n    \
        <cim:ExternalNetworkInjection.vmPu>1.02</cim:ExternalNetworkInjection.vmPu>\n    \
        <cim:ExternalNetworkInjection.vaDegree>0.0</cim:ExternalNetworkInjection.vaDegree>\n  \
        </cim:ExternalNetworkInjection>\n\
        {RDF_CLOSE}\n"
    )
}

/// The transformer profile whose rating element is parameterized: the
/// declared spelling and its verified alias produce the SAME import.
fn transformer_profile(rated_s_element: &str) -> String {
    format!(
        "{RDF_OPEN}\n  \
        <cim:BaseVoltage rdf:ID=\"_bv20\">\n    \
        <cim:BaseVoltage.nominalV>20.0</cim:BaseVoltage.nominalV>\n  \
        </cim:BaseVoltage>\n  \
        <cim:ConnectivityNode rdf:ID=\"_busA\">\n    \
        <cim:ConnectivityNode.BaseVoltage rdf:resource=\"#_bv20\" />\n  \
        </cim:ConnectivityNode>\n  \
        <cim:ConnectivityNode rdf:ID=\"_busB\">\n    \
        <cim:ConnectivityNode.BaseVoltage rdf:resource=\"#_bv20\" />\n  \
        </cim:ConnectivityNode>\n  \
        <cim:PowerTransformer rdf:ID=\"_xfmr1\">\n    \
        {rated_s_element}\n    \
        <cim:PowerTransformer.vkPercent>6.0</cim:PowerTransformer.vkPercent>\n    \
        <cim:PowerTransformer.vkRPercent>1.0</cim:PowerTransformer.vkRPercent>\n    \
        <cim:PowerTransformer.hvBus rdf:resource=\"#_busA\" />\n    \
        <cim:PowerTransformer.lvBus rdf:resource=\"#_busB\" />\n  \
        </cim:PowerTransformer>\n  \
        <cim:ExternalNetworkInjection rdf:ID=\"_grid1\">\n    \
        <cim:ExternalNetworkInjection.bus rdf:resource=\"#_busA\" />\n    \
        <cim:ExternalNetworkInjection.vmPu>1.02</cim:ExternalNetworkInjection.vmPu>\n    \
        <cim:ExternalNetworkInjection.vaDegree>0.0</cim:ExternalNetworkInjection.vaDegree>\n  \
        </cim:ExternalNetworkInjection>\n\
        {RDF_CLOSE}\n"
    )
}

// --- 1. The verified alias is accepted and imports identically ---------------

#[test]
fn the_verified_nominal_voltage_alias_imports_identically_to_the_declared_spelling() {
    // 44a §3 #1: the verified CGMES spelling is BaseVoltage.nominalVoltage
    // ([S-173] verbatim; [S-174]).
    let declared = base_voltage_profile(
        "<cim:BaseVoltage.nominalV>20.0</cim:BaseVoltage.nominalV>",
    );
    let alias = base_voltage_profile(
        "<cim:BaseVoltage.nominalVoltage>20.0</cim:BaseVoltage.nominalVoltage>",
    );
    let declared_report = source()
        .parse_reported(declared.as_bytes())
        .expect("the declared spelling parses (the old pin)");
    let alias_report = source()
        .parse_reported(alias.as_bytes())
        .expect("the verified alias spelling parses");
    assert_eq!(
        alias_report.warnings.len(),
        0,
        "a verified alias is an ACCEPTED spelling, never a dropped predicate: {:?}",
        alias_report.warnings
    );
    // The alias normalizes ONTO the declared key: the documents are EQUAL.
    assert_eq!(
        alias_report.document, declared_report.document,
        "the alias fixture normalizes to the same document as the declared fixture"
    );
    // And the import is the same: one bus at 20 kV, one external grid.
    let imported = import_profile_bytes(&source(), alias.as_bytes(), &model_ref(), 1.0)
        .expect("the alias fixture imports");
    assert_eq!(imported.model.buses.len(), 1);
    assert_eq!(imported.model.buses[0].nominal_kv, 20.0);
    assert!(imported.model.buses[0].is_slack);
    assert_eq!(imported.model.external_grids.len(), 1);
}

#[test]
fn the_verified_transformer_rating_alias_imports_identically_to_the_declared_spelling() {
    // 44a §3 #8: ratedS is verified on PowerTransformerEnd ([S-174]), NOT
    // on PowerTransformer. The alias normalizes the spelling onto the
    // transformer-rating attribute the declared key serves (the structural
    // end-object traversal stays the recorded UNVERIFIED work item).
    let declared = transformer_profile(
        "<cim:PowerTransformer.ratedS>630000</cim:PowerTransformer.ratedS>",
    );
    let alias = transformer_profile(
        "<cim:PowerTransformerEnd.ratedS>630000</cim:PowerTransformerEnd.ratedS>",
    );
    let declared_report = source()
        .parse_reported(declared.as_bytes())
        .expect("the declared spelling parses (the old pin)");
    let alias_report = source()
        .parse_reported(alias.as_bytes())
        .expect("the verified alias spelling parses");
    assert_eq!(alias_report.warnings.len(), 0);
    assert_eq!(alias_report.document, declared_report.document);
    let imported = import_profile_bytes(&source(), alias.as_bytes(), &model_ref(), 1.0)
        .expect("the alias fixture imports");
    // The canonical-unit conversion happened identically: 630000 VA →
    // 0.63 MVA (the declared mapping, unchanged).
    assert_eq!(imported.model.transformers.len(), 1);
    assert_eq!(imported.model.transformers[0].rated_mva, 0.63);
    assert_eq!(imported.model.transformers[0].vk_percent, 6.0);
    assert_eq!(imported.model.transformers[0].vk_r_percent, 1.0);
}

#[test]
fn both_aliases_resolve_through_the_full_import_seam() {
    // The unit-level resolver pins, restated through the crate's public
    // test surface (the parser consumes the SAME resolver).
    assert_eq!(
        resolve_attribute_key("BaseVoltage.nominalVoltage"),
        Some(ATTR_BASE_VOLTAGE_NOMINAL_KV)
    );
    assert_eq!(
        resolve_attribute_key("PowerTransformerEnd.ratedS"),
        Some(ATTR_TRANSFORMER_RATED_S_VA)
    );
    // And a profile carrying BOTH verified aliases at once imports cleanly.
    let both = format!(
        "{RDF_OPEN}\n  \
        <cim:BaseVoltage rdf:ID=\"_bv20\">\n    \
        <cim:BaseVoltage.nominalVoltage>20.0</cim:BaseVoltage.nominalVoltage>\n  \
        </cim:BaseVoltage>\n  \
        <cim:ConnectivityNode rdf:ID=\"_busA\">\n    \
        <cim:ConnectivityNode.BaseVoltage rdf:resource=\"#_bv20\" />\n  \
        </cim:ConnectivityNode>\n  \
        <cim:ConnectivityNode rdf:ID=\"_busB\">\n    \
        <cim:ConnectivityNode.BaseVoltage rdf:resource=\"#_bv20\" />\n  \
        </cim:ConnectivityNode>\n  \
        <cim:PowerTransformer rdf:ID=\"_xfmr1\">\n    \
        <cim:PowerTransformerEnd.ratedS>630000</cim:PowerTransformerEnd.ratedS>\n    \
        <cim:PowerTransformer.vkPercent>6.0</cim:PowerTransformer.vkPercent>\n    \
        <cim:PowerTransformer.vkRPercent>1.0</cim:PowerTransformer.vkRPercent>\n    \
        <cim:PowerTransformer.hvBus rdf:resource=\"#_busA\" />\n    \
        <cim:PowerTransformer.lvBus rdf:resource=\"#_busB\" />\n  \
        </cim:PowerTransformer>\n  \
        <cim:ExternalNetworkInjection rdf:ID=\"_grid1\">\n    \
        <cim:ExternalNetworkInjection.bus rdf:resource=\"#_busA\" />\n    \
        <cim:ExternalNetworkInjection.vmPu>1.02</cim:ExternalNetworkInjection.vmPu>\n    \
        <cim:ExternalNetworkInjection.vaDegree>0.0</cim:ExternalNetworkInjection.vaDegree>\n  \
        </cim:ExternalNetworkInjection>\n\
        {RDF_CLOSE}\n"
    );
    let report = source()
        .parse_reported(both.as_bytes())
        .expect("both verified aliases are accepted");
    assert_eq!(report.warnings.len(), 0);
    let transformer = report
        .document
        .object(&CimMRid::new("_xfmr1").expect("valid"))
        .expect("the transformer is present");
    assert_eq!(
        transformer.attribute(ATTR_TRANSFORMER_RATED_S_VA),
        Some("630000"),
        "the alias arrived under the CANONICAL declared key"
    );
    assert_eq!(
        transformer.attribute("PowerTransformerEnd.ratedS"),
        None,
        "no alias spelling is stored as a key — only the canonical vocabulary"
    );
}

// --- 2. A duplicate under two accepted spellings is still a duplicate --------

#[test]
fn the_declared_spelling_and_its_alias_on_one_object_are_a_named_duplicate_refusal() {
    // Both spellings resolve to the SAME canonical key, so carrying both on
    // one object is a duplicate predicate — the existing named refusal
    // fires on the RESOLVED key (never a silent overwrite, never two keys).
    let duplicated = transformer_profile(
        "<cim:PowerTransformer.ratedS>630000</cim:PowerTransformer.ratedS>",
    )
    .replace(
        "<cim:PowerTransformer.vkPercent>",
        "<cim:PowerTransformerEnd.ratedS>999999</cim:PowerTransformerEnd.ratedS>\n    \
         <cim:PowerTransformer.vkPercent>",
    );
    assert!(source().parse_reported(duplicated.as_bytes()).is_err());
}

// --- 3. Unknown spellings and the class recording are unchanged ---------------

#[test]
fn an_unknown_spelling_is_still_dropped_with_a_recorded_warning() {
    // A spelling accepted by neither the alias table nor the declared
    // vocabulary is NEVER a key: dropped with its recorded warning,
    // exactly as before the remediation.
    let with_bogus = base_voltage_profile(
        "<cim:BaseVoltage.nominalV>20.0</cim:BaseVoltage.nominalV>",
    )
    .replace(
        "  </cim:BaseVoltage>\n",
        "    <cim:BaseVoltage.bogusPredicate>9</cim:BaseVoltage.bogusPredicate>\n  \
         </cim:BaseVoltage>\n",
    );
    let report = source()
        .parse_reported(with_bogus.as_bytes())
        .expect("the profile still parses (a dropped predicate is not an error)");
    assert_eq!(report.warnings.len(), 1);
    assert_eq!(report.warnings[0].dropped_element, "BaseVoltage.bogusPredicate");
    // The import proceeds on the accepted keys only.
    let imported = import_profile_bytes(&source(), with_bogus.as_bytes(), &model_ref(), 1.0)
        .expect("the import uses the accepted keys only");
    assert_eq!(imported.model.buses.len(), 1);
}

#[test]
fn the_class_level_recording_is_unchanged_through_an_alias_fixture() {
    // The spelling remediation touched NOTHING at the class level: a
    // recognized-but-unmapped class is still EXPLICITLY_UNMAPPED with its
    // recorded reason, an unknown class is still UNMAPPED, and both travel
    // in the coverage-limitation statements.
    let profile = base_voltage_profile(
        "<cim:BaseVoltage.nominalVoltage>20.0</cim:BaseVoltage.nominalVoltage>",
    )
    .replace(
        "  <cim:ExternalNetworkInjection",
        "  <cim:EnergyConsumer rdf:ID=\"_load1\" />\n  \
         <cim:EnergyScheduling rdf:ID=\"_mystery1\" />\n  \
         <cim:ExternalNetworkInjection",
    );
    let imported = import_profile_bytes(&source(), profile.as_bytes(), &model_ref(), 1.0)
        .expect("the alias fixture imports");
    let statements = imported.summary.coverage_limitation_statements();
    assert!(
        statements
            .iter()
            .any(|s| s.starts_with("EXPLICITLY_UNMAPPED _load1 (EnergyConsumer)")),
        "the load refusal travels verbatim: {statements:?}"
    );
    assert!(
        statements
            .iter()
            .any(|s| s.starts_with("UNMAPPED _mystery1 (EnergyScheduling)")),
        "the unknown class is recorded, never guessed"
    );
}

// --- 4. The honest counts and the verified-absent register --------------------

#[test]
fn the_remediation_counts_cover_every_verified_mismatch_verdict() {
    // THE HONEST ARITHMETIC (the remediation module header; the wave task's
    // "alias count == 9" is not honestly satisfiable — the 9 is the
    // verified-ABSENT count): 2 accepted aliases + 9 verified-absent
    // entries = 11 = every MISMATCH (verified) verdict of record 44a §3.
    assert_eq!(alias_count(), 2);
    assert_eq!(verified_absent_count(), 9);
    assert_eq!(alias_count() + verified_absent_count(), 11);
}

#[test]
fn the_verified_absent_register_carries_the_named_not_modelled_statements() {
    let statements = verified_absent_statements();
    assert_eq!(statements.len(), 9);
    for statement in &statements {
        assert!(
            statement.starts_with("VERIFIED_ABSENT "),
            "each statement is a named record: {statement}"
        );
        assert!(
            statement.contains("44a") && statement.contains("[S-17"),
            "each statement cites the 44a record and its source: {statement}"
        );
    }
    // The two UNVERIFIED items of record 44a §3 (the vk / vkR short-circuit
    // spellings) are NOT in the register and NOT aliased — they stay
    // labeled UNVERIFIED exactly as the record requires, and their declared
    // keys remain the accepted import vocabulary.
    assert_eq!(
        resolve_attribute_key(ATTR_TRANSFORMER_VK_PERCENT),
        Some(ATTR_TRANSFORMER_VK_PERCENT)
    );
    assert_eq!(
        resolve_attribute_key(ATTR_TRANSFORMER_VK_R_PERCENT),
        Some(ATTR_TRANSFORMER_VK_R_PERCENT)
    );
}

#[test]
fn a_verified_absent_key_is_still_accepted_declared_vocabulary() {
    // The verified-absent keys are NOT aliases and the declared vocabulary
    // never shrank: EVERY declared key the record verified absent resolves
    // to ITSELF through the resolver (accepted exactly as before), and a
    // fixture carrying a declared key the record verified absent from
    // CGMES still parses and imports exactly as before (the fixtures of
    // tests/cgmes_import.rs pin the same on the full profile).
    for absent_key in [
        ATTR_NODE_BASE_VOLTAGE_REF,
        ATTR_TRANSFORMER_HV_BUS_REF,
        ATTR_TRANSFORMER_LV_BUS_REF,
        ATTR_EXTERNAL_GRID_BUS_REF,
        ATTR_EXTERNAL_GRID_VM_PU,
        ATTR_EXTERNAL_GRID_VA_DEGREE,
    ] {
        assert_eq!(
            resolve_attribute_key(absent_key),
            Some(absent_key),
            "the verified-absent key stays accepted declared vocabulary: {absent_key}"
        );
    }
    let imported = import_profile_bytes(
        &source(),
        base_voltage_profile("<cim:BaseVoltage.nominalV>20.0</cim:BaseVoltage.nominalV>")
            .as_bytes(),
        &model_ref(),
        1.0,
    )
    .expect("the declared vocabulary imports unchanged");
    assert_eq!(
        imported.model.buses[0].nominal_kv, 20.0,
        "the declared key still serves the mapping"
    );
}
