// P14.17 — The CIM profile fixture source (decision record 81; the seam
// is `flexcore_cim_adapter::rdf_profile::RdfProfileSource` — the UNWIRED
// RDF/XML parse seam, record 44 §2).
//
// The IMPORT path's receiving end is a PARSER, not a device — so the
// simulated counterpart is a FIXTURE SOURCE: it serves a preconstructed
// [`ProfileDocument`] through the [`RdfProfileSource`] trait, so the
// mapping/register path downstream (the class mapping, the unmapped
// register, the IR assembly) is exercised against a declared document
// with NO XML parser, no RDF library, and no real CGMES file read.
//
// The normalization rule the real parser must satisfy (record 44 §2:
// objects keyed by mRID, declared-attribute entries, BTreeMap ordering)
// is satisfied BY CONSTRUCTION here — the fixture is a `ProfileDocument`
// built through the model's own validating constructor, so a downstream
// consumer cannot tell a fixture document from a parsed one by shape
// (only by its declared fixture identity in the attribute data).
//
// Honest byte handling: the trait's `parse` takes bytes; the fixture
// source does NOT interpret them (no parser exists). Empty bytes are
// refused with the named `NotWellFormed` error (a real parser would
// refuse them too — an empty document is not well-formed RDF under any
// reading); non-empty bytes are accepted as the FIXTURE SELECTION KEY
// but never decoded — declared explicitly in the module and the record,
// never silently. Every fixture object identity carries the `sim-cim`
// prefix.

use flexcore_cim_adapter::model::{CimMRid, CimObject, CimModelError, ProfileDocument};
use flexcore_cim_adapter::rdf_profile::{ProfileParseError, RdfProfileSource};

/// The identity prefix every fixture object's attributes carry (a
/// declared attribute entry on every fixture object so a document is
/// identifiable as simulated from its content alone).
pub const SIM_CIM_ID_PREFIX: &str = "sim-cim";

/// The declared attribute key the fixture identity travels under (a
/// fixture-only attribute name; a real parser would drop unknown
/// predicates — declared here so the fixture is honest about being a
/// fixture).
pub const SIM_CIM_FIXTURE_ATTR: &str = "Sim.fixtures.source";

/// The declared CIM classes the fixture vocabulary covers (the same
/// classes the adapter's mapping tests use; the fixture exercises the
/// SAME mapping path, not a parallel one).
pub const FIXTURE_CLASSES: [&str; 3] = ["BaseVoltage", "ACLineSegment", "PowerElectronicsConnection"];

/// Errors the fixture source reports. Every failure is named.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SimCimError {
    /// The model layer refused a fixture object (the constructor's own
    /// validation, re-raised).
    Model(CimModelError),
}

impl std::fmt::Display for SimCimError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SimCimError::Model(error) => write!(f, "the fixture document was refused by the model layer: {error}"),
        }
    }
}

impl std::error::Error for SimCimError {}

/// Builds the fixture `ProfileDocument`: the declared objects (mRID,
/// class, declared attributes) assembled through the model's own
/// validating constructor, with the fixture-identity attribute attached
/// to every object.
pub fn fixture_document(
    objects: Vec<(String, String, Vec<(String, String)>)>,
) -> Result<ProfileDocument, SimCimError> {
    let mut built = Vec::with_capacity(objects.len());
    for (mrid, class_name, attributes) in objects {
        let mut attrs = std::collections::BTreeMap::new();
        for (key, value) in attributes {
            attrs.insert(key, value);
        }
        attrs.insert(SIM_CIM_FIXTURE_ATTR.to_string(), SIM_CIM_ID_PREFIX.to_string());
        built.push(CimObject {
            mrid: CimMRid::new(&mrid)?,
            class_name,
            attributes: attrs,
        });
    }
    ProfileDocument::new(built).map_err(SimCimError::Model)
}

/// The canonical small fixture: one BaseVoltage, one ACLineSegment
/// referencing it, and one PowerElectronicsConnection — the shape the
/// adapter's mapping tests exercise.
pub fn canonical_fixture() -> Result<ProfileDocument, SimCimError> {
    fixture_document(vec![
        (
            "_sim_bv20".to_string(),
            "BaseVoltage".to_string(),
            vec![(
                flexcore_cim_adapter::model::ATTR_BASE_VOLTAGE_NOMINAL_KV.to_string(),
                "20.0".to_string(),
            )],
        ),
        (
            "_sim_line_1".to_string(),
            "ACLineSegment".to_string(),
            vec![
                (
                    flexcore_cim_adapter::model::ATTR_LINE_FROM_BUS_REF.to_string(),
                    "_sim_bv20".to_string(),
                ),
                (
                    flexcore_cim_adapter::model::ATTR_LINE_R_OHM.to_string(),
                    "0.35".to_string(),
                ),
            ],
        ),
        (
            "_sim_der_1".to_string(),
            "PowerElectronicsConnection".to_string(),
            vec![(
                flexcore_cim_adapter::model::ATTR_NODE_BASE_VOLTAGE_REF.to_string(),
                "_sim_bv20".to_string(),
            )],
        ),
    ])
}

/// The fixture implementation of the [`RdfProfileSource`] seam. It
/// serves its preconstructed document; the bytes parameter is the
/// fixture SELECTION KEY (any non-empty byte slice selects the fixture)
/// and is never decoded — there is no parser here, by declared design.
#[derive(Debug, Clone)]
pub struct FixtureProfileSource {
    /// The fixture identity (carries the `sim-cim` prefix).
    pub source_id: String,
    document: ProfileDocument,
}

impl FixtureProfileSource {
    /// Constructs the fixture source over a fixture document.
    pub fn new(source_id: impl Into<String>, document: ProfileDocument) -> Self {
        Self {
            source_id: format!("{SIM_CIM_ID_PREFIX}:{}", source_id.into()),
            document,
        }
    }

    /// The fixture document (a read-only snapshot).
    pub fn document(&self) -> &ProfileDocument {
        &self.document
    }
}

impl RdfProfileSource for FixtureProfileSource {
    fn parse(&self, bytes: &[u8]) -> Result<ProfileDocument, ProfileParseError> {
        if bytes.is_empty() {
            // An empty input is not well-formed RDF under any reading —
            // the same named refusal a real parser reports.
            return Err(ProfileParseError::NotWellFormed {
                reason: format!(
                    "the {} fixture source refuses empty input (an empty document is not well-formed RDF)",
                    self.source_id
                ),
            });
        }
        // Non-empty bytes select the fixture; they are NOT decoded
        // (declared honestly — no XML/RDF parser exists in this batch,
        // record 44 §2). The served document is the fixture itself.
        Ok(self.document.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fixture_document_is_a_valid_profile_document() {
        let document = canonical_fixture().expect("valid fixture");
        assert_eq!(document.len(), 3);
        // Every fixture object carries the fixture-identity attribute.
        for object in document.objects() {
            assert_eq!(
                object.attributes.get(SIM_CIM_FIXTURE_ATTR).map(String::as_str),
                Some(SIM_CIM_ID_PREFIX),
                "fixture identity travels on the object"
            );
        }
        // The declared fixture classes are covered.
        let classes: Vec<&str> = document.objects().map(|o| o.class_name.as_str()).collect();
        for declared in FIXTURE_CLASSES {
            assert!(classes.contains(&declared), "fixture covers {declared}");
        }
    }

    #[test]
    fn the_fixture_source_serves_through_the_real_seam() {
        let source = FixtureProfileSource::new("cgmes-fixture", canonical_fixture().expect("valid"));
        let parsed = source.parse(b"fixture-selection-key").expect("serves");
        assert_eq!(parsed.len(), 3);
        // Empty bytes: the named refusal (a real parser's answer).
        assert!(matches!(
            source.parse(b""),
            Err(ProfileParseError::NotWellFormed { .. })
        ));
    }

    #[test]
    fn a_degenerate_fixture_object_is_refused_by_the_model_layer() {
        // An empty mRID is refused by CimMRid's own validation.
        assert!(matches!(
            fixture_document(vec![(
                String::new(),
                "BaseVoltage".to_string(),
                Vec::new(),
            )]),
            Err(SimCimError::Model(CimModelError::EmptyField("mrid")))
        ));
    }
}
