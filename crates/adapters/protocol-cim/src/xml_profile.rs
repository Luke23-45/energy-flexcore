// FIM-F06.02 / FIM-F06.03 / FIM-F06.04 — the wired RDF/XML profile source
// (docs/releases/final-implementation-before-publication.md §5 feature
// F-06; ledger docs/releases/implementation_ledger/ledger_F06.md;
// decision record 44_p14_12_cim_cgmes_import.md).
//
// **Implemented under FIM-F06, WRITTEN-UNRUN: this module has never been
// compiled or executed. Interop with a real DSO CGMES profile is UNPROVEN
// and remains a HUMAN LAB GATE** (crate conventions §7.3; nothing here is
// a conformance result).
//
// WHAT THIS MODULE IS: [`XmlRdfProfileSource`] implements the EXISTING
// `crate::rdf_profile::RdfProfileSource` seam over the hand-written pull
// parser ([`crate::xml_pull`]) — producing EXACTLY the existing types
// (`ProfileDocument`, `CimObject`, `CimMRid`). The declared attribute
// constants of [`crate::model`] stay the single source of predicate
// semantics: this module adds NO class name and NO attribute spelling. An
// element whose 'Class.attribute' spelling does not match a declared
// constant is DROPPED and recorded as a parse warning
// ([`ProfileParseWarning`]) — never guessed into a key, never silently
// dropped.
//
// FIM wave 8B (Agent B) — the ADDITIVE spelling remediation (record 44a's
// 2026-11-21 addendum): the parse path resolves predicate spellings through
// `crate::spelling_remediation::resolve_attribute_key`, which additionally
// accepts the VERIFIED CGMES alias spellings (2 entries, each citing the
// 44a record and its [S-17x] source) and normalizes them onto the EXISTING
// declared constants. No declared key changed, no new canonical key exists,
// and the 9 verified-absent declared keys are recorded (NOT aliases) in
// that module's register. WRITTEN-UNRUN; real-profile interop stays the
// HUMAN LAB GATE.
//
// FIM-F06.03 — the completeness gate through the import path:
// [`import_profile_bytes`] / [`import_document`] run the parsed document
// through the EXISTING `crate::unmapped::ImportRegister::classify_profile`
// register and its `finalize` gate — which refuses while any AMBIGUOUS
// object remains — and the EXISTING `assemble_network_model`. There is no
// parallel gate and no parallel register: a parallel implementation could
// drift from the reviewed one, so none is written.
//
// FIM-F06.04 — unmapped classes flow through the EXISTING
// `crate::unmapped` surface: EXPLICITLY_UNMAPPED (the vocabulary
// recognizes the class and declares it unmapped, reason recorded) and
// UNMAPPED (no vocabulary entry, recorded) happen inside the register
// exactly as before; this module cannot and does not bypass that
// recording.
//
// THE NORMALIZATION RULES (the declared rules of `crate::rdf_profile`,
// now implemented — the P14.12 record §1):
//
// 1. an OBJECT element is any non-RDF-namespace child of the RDF root; its
//    mRID is its `rdf:ID` (fallback `rdf:about`, a leading '#' fragment is
//    stripped); its class name is the element's LOCAL name (the raw
//    spelling — recognition happens at the mapping, never at parse time);
// 2. an ATTRIBUTE element is a child of an object element whose local name
//    equals a DECLARED attribute constant; its value is the element's text
//    content (trimmed) or, when `rdf:resource` is present, the referenced
//    mRID (a leading '#' fragment is stripped);
// 3. NAMED refusals: an attribute element carrying BOTH `rdf:resource` and
//    text content; a duplicate predicate on one object; a nested element
//    inside an attribute element (the subset carries flat attribute
//    elements only); an object element without `rdf:ID`/`rdf:about`; text
//    content outside an attribute element;
// 4. object ordering is irrelevant: the document is a BTreeMap (the
//    determinism rule); the parse warnings are recorded in element order,
//    each naming the object it belonged to.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use flexcore_grid_api::port::{GridModelRef, VersionedNetworkModel};

use crate::model::{CimMRid, CimObject, ProfileDocument};
use crate::rdf_profile::{ProfileParseError, RdfProfileSource};
use crate::unmapped::{assemble_network_model, ImportRegister, ModelImportSummary};
use crate::xml_pull;

/// One recorded parse warning: an element the declared normalization
/// DROPPED — always named, never silent (the P14.12 normalization rule 2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileParseWarning {
    /// The mRID of the object the dropped element belonged to (empty for
    /// a drop directly under the RDF root).
    pub object_mrid: String,
    /// The dropped element's local spelling as it appeared.
    pub dropped_element: String,
    /// Why it was dropped (the honesty text a reviewer reads).
    pub reason: String,
}

/// The parse result WITH its warnings: the normalized [`ProfileDocument`]
/// (what [`RdfProfileSource::parse`] returns) plus the recorded drops.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XmlProfileParseReport {
    pub document: ProfileDocument,
    pub warnings: Vec<ProfileParseWarning>,
}

/// The wired RDF/XML profile source (FIM-F06.02): implements the EXISTING
/// [`RdfProfileSource`] seam over [`crate::xml_pull`]. [`UnwiredProfileSource`]
/// stays untouched and remains the shipped default until a composition
/// root constructs and selects this source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XmlRdfProfileSource {
    max_input_bytes: usize,
}

impl Default for XmlRdfProfileSource {
    fn default() -> Self {
        Self::new()
    }
}

impl XmlRdfProfileSource {
    /// The module default: the parser's bounded-input guard
    /// ([`crate::xml_pull::MAX_XML_INPUT_BYTES`]).
    pub fn new() -> Self {
        Self {
            max_input_bytes: xml_pull::MAX_XML_INPUT_BYTES,
        }
    }

    /// An explicit bounded-input limit (the pinned-test seam for the guard;
    /// a deployment with a legitimately larger ceiling re-declares it at
    /// its own reviewed composition boundary).
    pub fn with_max_input_bytes(max_input_bytes: usize) -> Self {
        Self { max_input_bytes }
    }

    /// Parses profile bytes into the normalized document PLUS the recorded
    /// parse warnings (the dropped elements). Every refusal is named; a
    /// partial document is never a success.
    pub fn parse_reported(
        &self,
        bytes: &[u8],
    ) -> Result<XmlProfileParseReport, ProfileParseError> {
        let mut parser =
            xml_pull::XmlPullParser::with_max_bytes(bytes, self.max_input_bytes)
                .map_err(seam_error)?;
        let mut objects: Vec<CimObject> = Vec::new();
        let mut warnings: Vec<ProfileParseWarning> = Vec::new();
        let mut root_seen = false;
        let mut root_closed = false;
        let mut current: Option<ObjectBuilder> = None;
        let mut pending: Option<AttributeBuilder> = None;
        let mut skip_depth = 0usize;
        while let Some(event) = parser.next_event().map_err(seam_error)? {
            match event {
                xml_pull::XmlEvent::StartElement(start) => {
                    if !root_seen {
                        root_seen = true;
                        if start.name.namespace != xml_pull::RDF_NAMESPACE
                            || start.name.local != "RDF"
                        {
                            return Err(ProfileParseError::NotACimProfile {
                                reason: format!(
                                    "the root element is '{}:{}' — the declared profile \
                                     root is the RDF root '{}RDF' (normalization rule 1)",
                                    start.name.namespace, start.name.local, xml_pull::RDF_NAMESPACE
                                ),
                            });
                        }
                        continue;
                    }
                    if skip_depth > 0 {
                        skip_depth += 1;
                        continue;
                    }
                    if pending.is_some() {
                        return Err(ProfileParseError::NotWellFormed {
                            reason: "a nested element inside an attribute element — the \
                                     declared normalization carries flat attribute elements \
                                     only (normalization rule 3)"
                                .to_string(),
                        });
                    }
                    if current.is_none() {
                        // A direct child of the RDF root: an object
                        // element (rule 1) — or an RDF-syntax element
                        // to drop with a recorded warning.
                        if start.name.namespace == xml_pull::RDF_NAMESPACE {
                            warnings.push(ProfileParseWarning {
                                object_mrid: String::new(),
                                dropped_element: start.name.local.clone(),
                                reason: "an RDF-syntax element other than the root is \
                                         not an object element in this import — dropped, \
                                         recorded (never guessed into the vocabulary)"
                                    .to_string(),
                            });
                            skip_depth = 1;
                            continue;
                        }
                        let raw_mrid = start
                            .attribute(xml_pull::RDF_NAMESPACE, "ID")
                            .or_else(|| start.attribute(xml_pull::RDF_NAMESPACE, "about"))
                            .ok_or_else(|| ProfileParseError::NotWellFormed {
                                reason: format!(
                                    "the object element '{}' carries neither rdf:ID nor \
                                     rdf:about (normalization rule 1)",
                                    start.name.local
                                ),
                            })?
                            .to_string();
                        let mrid = CimMRid::new(&strip_fragment(&raw_mrid))
                            .map_err(ProfileParseError::Model)?;
                        current = Some(ObjectBuilder {
                            mrid,
                            class_name: start.name.local.clone(),
                            attributes: BTreeMap::new(),
                        });
                        continue;
                    }
                    let builder = current.as_mut().expect("checked above: an open object");
                    // Inside an object: an attribute element
                    // candidate (rule 2) — or an undeclared
                    // predicate to drop with a recorded warning.
                    // FIM wave 8B: the resolver accepts the VERIFIED CGMES
                    // alias spellings FIRST (record 44a's remediation,
                    // `crate::spelling_remediation`) and falls back to the
                    // declared-key table unchanged; a predicate accepted by
                    // neither is still dropped with its recorded warning.
                    let resolved = crate::spelling_remediation::resolve_attribute_key(
                        &start.name.local,
                    );
                    match resolved {
                        Some(key) => {
                            let resource = start
                                .attribute(xml_pull::RDF_NAMESPACE, "resource")
                                .map(strip_fragment);
                            pending = Some(AttributeBuilder {
                                key,
                                resource,
                                text: String::new(),
                            });
                        }
                        None => {
                            warnings.push(ProfileParseWarning {
                                object_mrid: builder.mrid.value().to_string(),
                                dropped_element: start.name.local.clone(),
                                reason: "the predicate is accepted by neither the verified \
                                         alias table nor a declared attribute constant of \
                                         crate::model — dropped, recorded (never guessed \
                                         into a key)"
                                    .to_string(),
                            });
                            skip_depth = 1;
                        }
                    }
                }
                xml_pull::XmlEvent::Text(text) => {
                    if skip_depth > 0 {
                        continue;
                    }
                    match pending.as_mut() {
                        Some(attribute) => attribute.text.push_str(&text),
                        None => {
                            return Err(ProfileParseError::NotWellFormed {
                                reason: "text content outside an attribute element — the \
                                         declared normalization carries attribute values in \
                                         attribute elements only (normalization rule 3)"
                                    .to_string(),
                            });
                        }
                    }
                }
                xml_pull::XmlEvent::EndElement(name) => {
                    if skip_depth > 0 {
                        skip_depth -= 1;
                        continue;
                    }
                    if let Some(attribute) = pending.take() {
                        // The closed element is the pending attribute
                        // element: complete it (rule 2).
                        let builder = current.as_mut().ok_or_else(|| {
                            ProfileParseError::NotWellFormed {
                                reason: "an attribute element completed with no open object \
                                         element"
                                    .to_string(),
                            }
                        })?;
                        if attribute.resource.is_some() && !attribute.text.trim().is_empty() {
                            return Err(ProfileParseError::NotWellFormed {
                                reason: format!(
                                    "the attribute element '{}' carries both rdf:resource \
                                     and text content (normalization rule 3)",
                                    attribute.key
                                ),
                            });
                        }
                        let value = match attribute.resource {
                            Some(resource) => resource,
                            None => attribute.text.trim().to_string(),
                        };
                        if builder.attributes.contains_key(attribute.key) {
                            return Err(ProfileParseError::NotWellFormed {
                                reason: format!(
                                    "the predicate '{}' appears twice on object '{}' \
                                     (normalization rule 3)",
                                    attribute.key,
                                    builder.mrid.value()
                                ),
                            });
                        }
                        builder.attributes.insert(attribute.key.to_string(), value);
                        continue;
                    }
                    if let Some(builder) = current.take() {
                        objects.push(CimObject {
                            mrid: builder.mrid,
                            class_name: builder.class_name,
                            attributes: builder.attributes,
                        });
                        continue;
                    }
                    // No open object and no pending attribute: the RDF
                    // root closed (a defensive named check — the parser
                    // already guarantees the tag match).
                    if name.namespace != xml_pull::RDF_NAMESPACE || name.local != "RDF" {
                        return Err(ProfileParseError::NotWellFormed {
                            reason: format!(
                                "the element '{}:{}' closed outside an object element",
                                name.namespace, name.local
                            ),
                        });
                    }
                    root_closed = true;
                }
            }
        }
        if !root_seen {
            return Err(ProfileParseError::NotACimProfile {
                reason: "the document contains no root element".to_string(),
            });
        }
        if !root_closed {
            // Unreachable while the parser refuses unclosed elements at
            // EOF — kept as a named defensive refusal, never a silent pass.
            return Err(ProfileParseError::NotWellFormed {
                reason: "the RDF root element never closed".to_string(),
            });
        }
        let document = ProfileDocument::new(objects).map_err(ProfileParseError::Model)?;
        Ok(XmlProfileParseReport {
            document,
            warnings,
        })
    }
}

impl RdfProfileSource for XmlRdfProfileSource {
    fn parse(&self, bytes: &[u8]) -> Result<ProfileDocument, ProfileParseError> {
        self.parse_reported(bytes).map(|report| report.document)
    }
}

/// One in-flight object element: the mRID, the raw class spelling, and the
/// declared attributes collected so far.
struct ObjectBuilder {
    mrid: CimMRid,
    class_name: String,
    attributes: BTreeMap<String, String>,
}

/// One in-flight attribute element: its canonical DECLARED key (an
/// accepted verified alias resolves onto a declared constant — the
/// spelling_remediation register; a declared spelling resolves to itself),
/// an optional `rdf:resource` reference, and the buffered text content.
struct AttributeBuilder {
    key: &'static str,
    resource: Option<String>,
    text: String,
}

/// The declared attribute keys of [`crate::model`] — THE single source of
/// predicate semantics. A predicate element matches when its local name
/// equals one constant spelling; everything else is dropped with a
/// recorded warning (the declared normalization rule 2).
///
/// FIM wave 8B: the PARSER no longer calls this function directly — the
/// parse path resolves predicates through
/// [`crate::spelling_remediation::resolve_attribute_key`], which accepts
/// the VERIFIED CGMES alias spellings (record 44a's remediation) and falls
/// back to THIS table unchanged. The declared-key table itself stays
/// exactly as before: no constant was renamed, no key was re-pointed, and
/// this function's own pinned test
/// (`the_declared_attribute_keys_cover_every_constant_exactly`) is
/// untouched.
pub(crate) fn declared_attribute_key(local: &str) -> Option<&'static str> {
    const DECLARED: [&str; 15] = [
        crate::model::ATTR_BASE_VOLTAGE_NOMINAL_KV,
        crate::model::ATTR_NODE_BASE_VOLTAGE_REF,
        crate::model::ATTR_LINE_R_OHM,
        crate::model::ATTR_LINE_X_OHM,
        crate::model::ATTR_LINE_RATED_CURRENT_A,
        crate::model::ATTR_LINE_FROM_BUS_REF,
        crate::model::ATTR_LINE_TO_BUS_REF,
        crate::model::ATTR_TRANSFORMER_RATED_S_VA,
        crate::model::ATTR_TRANSFORMER_VK_PERCENT,
        crate::model::ATTR_TRANSFORMER_VK_R_PERCENT,
        crate::model::ATTR_TRANSFORMER_HV_BUS_REF,
        crate::model::ATTR_TRANSFORMER_LV_BUS_REF,
        crate::model::ATTR_EXTERNAL_GRID_BUS_REF,
        crate::model::ATTR_EXTERNAL_GRID_VM_PU,
        crate::model::ATTR_EXTERNAL_GRID_VA_DEGREE,
    ];
    DECLARED.into_iter().find(|key| *key == local)
}

/// Strips a leading '#' fragment from an `rdf:about` / `rdf:resource`
/// value (the referenced object's mRID is the fragment identifier; a value
/// without the fragment is taken verbatim).
fn strip_fragment(raw: &str) -> String {
    raw.strip_prefix('#').unwrap_or(raw).to_string()
}

/// Maps a parser refusal onto the seam's error vocabulary: the seam
/// carries a human-safe reason (the P1.08 rule), the named parser variant
/// stays one layer down ([`crate::xml_pull::XmlParseError`]).
fn seam_error(error: xml_pull::XmlParseError) -> ProfileParseError {
    ProfileParseError::NotWellFormed {
        reason: error.to_string(),
    }
}

/// The combined import-path error: the parse refusal or the EXISTING
/// register/assembly refusal — never merged into a blanket.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileImportError {
    /// The profile bytes were refused by the source seam.
    Parse(ProfileParseError),
    /// The parsed document was refused by the EXISTING import gate
    /// ([`crate::unmapped::ImportError`]).
    Import(crate::unmapped::ImportError),
}

impl std::fmt::Display for ProfileImportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProfileImportError::Parse(error) => write!(f, "the profile was refused at parse: {error}"),
            ProfileImportError::Import(error) => write!(f, "the import was refused by the gate: {error}"),
        }
    }
}

impl std::error::Error for ProfileImportError {}

impl From<ProfileParseError> for ProfileImportError {
    fn from(error: ProfileParseError) -> Self {
        ProfileImportError::Parse(error)
    }
}

impl From<crate::unmapped::ImportError> for ProfileImportError {
    fn from(error: crate::unmapped::ImportError) -> Self {
        ProfileImportError::Import(error)
    }
}

/// One successful import: the assembled grid-api `VersionedNetworkModel`
/// plus the summary that travelled through the EXISTING completeness gate.
#[derive(Debug)]
pub struct ModelImport {
    pub model: VersionedNetworkModel,
    pub summary: ModelImportSummary,
}

/// FIM-F06.02 + FIM-F06.03 + FIM-F06.04 — the full import path from
/// profile BYTES: parse (the wired seam) → `ImportRegister::classify_profile`
/// (the EXISTING register — the EXPLICITLY_UNMAPPED/UNMAPPED recording) →
/// `finalize` (the EXISTING completeness gate — refuses while any AMBIGUOUS
/// object remains) → `assemble_network_model` (the EXISTING assembly, the
/// IR's own structural validation). No parallel gate, no parallel register.
pub fn import_profile_bytes(
    source: &XmlRdfProfileSource,
    bytes: &[u8],
    model_ref: &GridModelRef,
    base_mva: f64,
) -> Result<ModelImport, ProfileImportError> {
    let report = source.parse_reported(bytes)?;
    import_document(&report.document, model_ref, base_mva).map_err(ProfileImportError::Import)
}

/// The document-level import path (a parsed [`ProfileDocument`] in, the
/// existing gate and assembly out).
pub fn import_document(
    document: &ProfileDocument,
    model_ref: &GridModelRef,
    base_mva: f64,
) -> Result<ModelImport, crate::unmapped::ImportError> {
    let register = ImportRegister::classify_profile(document);
    let summary = register.finalize()?;
    let model = assemble_network_model(model_ref, base_mva, &summary)?;
    Ok(ModelImport { model, summary })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ATTR_LINE_RATED_CURRENT_A;

    fn source() -> XmlRdfProfileSource {
        XmlRdfProfileSource::new()
    }

    fn parse(bytes: &[u8]) -> XmlProfileParseReport {
        source().parse_reported(bytes).expect("well-formed profile")
    }

    fn parse_error(bytes: &[u8]) -> ProfileParseError {
        source().parse_reported(bytes).expect_err("expected a refusal")
    }

    const RDF_ROOT_OPEN: &str = "<rdf:RDF xmlns:cim=\"http://iec.ch/TC57/2013/CIM-schema-cim16#\" \
        xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">";
    const RDF_ROOT_CLOSE: &str = "</rdf:RDF>";

    #[test]
    fn the_declared_attribute_keys_cover_every_constant_exactly() {
        // The declared-key table and the crate::model constants cannot
        // diverge: every constant is recognized, and the table's spellings
        // are the constants themselves (the single-source rule).
        for key in [
            crate::model::ATTR_BASE_VOLTAGE_NOMINAL_KV,
            crate::model::ATTR_NODE_BASE_VOLTAGE_REF,
            crate::model::ATTR_LINE_R_OHM,
            crate::model::ATTR_LINE_X_OHM,
            crate::model::ATTR_LINE_RATED_CURRENT_A,
            crate::model::ATTR_LINE_FROM_BUS_REF,
            crate::model::ATTR_LINE_TO_BUS_REF,
            crate::model::ATTR_TRANSFORMER_RATED_S_VA,
            crate::model::ATTR_TRANSFORMER_VK_PERCENT,
            crate::model::ATTR_TRANSFORMER_VK_R_PERCENT,
            crate::model::ATTR_TRANSFORMER_HV_BUS_REF,
            crate::model::ATTR_TRANSFORMER_LV_BUS_REF,
            crate::model::ATTR_EXTERNAL_GRID_BUS_REF,
            crate::model::ATTR_EXTERNAL_GRID_VM_PU,
            crate::model::ATTR_EXTERNAL_GRID_VA_DEGREE,
        ] {
            assert_eq!(
                declared_attribute_key(key),
                Some(key),
                "the declared key table recognizes the constant spelling: {key}"
            );
        }
        assert_eq!(declared_attribute_key("ACLineSegment.notDeclared"), None);
    }

    #[test]
    fn the_miniature_profile_normalizes_into_the_profile_document() {
        let document = format!(
            "{RDF_ROOT_OPEN}\n  \
            <cim:BaseVoltage rdf:ID=\"_bv20\">\n    \
            <cim:BaseVoltage.nominalV>20.0</cim:BaseVoltage.nominalV>\n  \
            </cim:BaseVoltage>\n  \
            <cim:ACLineSegment rdf:ID=\"_line1\">\n    \
            <cim:ACLineSegment.r> 1.5 </cim:ACLineSegment.r>\n    \
            <cim:ACLineSegment.fromBus rdf:resource=\"#_busA\" />\n  \
            </cim:ACLineSegment>\n\
            {RDF_ROOT_CLOSE}\n"
        );
        let report = parse(document.as_bytes());
        assert_eq!(report.warnings.len(), 0);
        assert_eq!(report.document.len(), 2);
        let line = report
            .document
            .object(&CimMRid::new("_line1").expect("valid"))
            .expect("present");
        assert_eq!(line.class_name, "ACLineSegment");
        assert_eq!(
            line.attribute(crate::model::ATTR_LINE_R_OHM),
            Some("1.5"),
            "text values are trimmed (rule 2)"
        );
        assert_eq!(
            line.attribute(crate::model::ATTR_LINE_FROM_BUS_REF),
            Some("_busA"),
            "an rdf:resource reference becomes the referenced mRID, the '#' fragment \
             stripped (rule 2)"
        );
    }

    #[test]
    fn rdf_about_is_the_declared_fallback_and_is_fragment_stripped() {
        let document = format!(
            "{RDF_ROOT_OPEN}\n  \
            <cim:Breaker rdf:about=\"#_brk1\" />\n\
            {RDF_ROOT_CLOSE}"
        );
        let report = parse(document.as_bytes());
        let breaker = report
            .document
            .object(&CimMRid::new("_brk1").expect("valid"))
            .expect("present");
        assert_eq!(breaker.class_name, "Breaker");
    }

    #[test]
    fn undeclared_predicates_are_dropped_and_recorded_never_silent() {
        let document = format!(
            "{RDF_ROOT_OPEN}\n  \
            <cim:ACLineSegment rdf:ID=\"_line1\">\n    \
            <cim:ACLineSegment.notADeclaredKey>9</cim:ACLineSegment.notADeclaredKey>\n    \
            <cim:ACLineSegment.r>1.5</cim:ACLineSegment.r>\n  \
            </cim:ACLineSegment>\n\
            {RDF_ROOT_CLOSE}"
        );
        let report = parse(document.as_bytes());
        assert_eq!(report.document.len(), 1, "the profile still parses");
        assert_eq!(report.warnings.len(), 1, "the drop is recorded");
        assert_eq!(report.warnings[0].dropped_element, "ACLineSegment.notADeclaredKey");
        assert_eq!(report.warnings[0].object_mrid, "_line1");
        assert!(
            report.warnings[0].reason.contains("never guessed"),
            "the warning states the no-guess rule: {}",
            report.warnings[0].reason
        );
        let line = report
            .document
            .object(&CimMRid::new("_line1").expect("valid"))
            .expect("present");
        assert_eq!(line.attribute(crate::model::ATTR_LINE_R_OHM), Some("1.5"));
    }

    #[test]
    fn an_object_without_rdf_id_or_about_is_a_named_refusal() {
        let document = format!(
            "{RDF_ROOT_OPEN}\n  <cim:Breaker />\n{RDF_ROOT_CLOSE}"
        );
        assert!(matches!(
            parse_error(document.as_bytes()),
            ProfileParseError::NotWellFormed { .. }
        ));
    }

    #[test]
    fn a_duplicate_predicate_on_one_object_is_a_named_refusal() {
        let document = format!(
            "{RDF_ROOT_OPEN}\n  \
            <cim:ACLineSegment rdf:ID=\"_line1\">\n    \
            <cim:ACLineSegment.r>1.5</cim:ACLineSegment.r>\n    \
            <cim:ACLineSegment.r>2.5</cim:ACLineSegment.r>\n  \
            </cim:ACLineSegment>\n\
            {RDF_ROOT_CLOSE}"
        );
        assert!(matches!(
            parse_error(document.as_bytes()),
            ProfileParseError::NotWellFormed { .. }
        ));
    }

    #[test]
    fn resource_plus_text_and_nested_elements_are_named_refusals() {
        let both = format!(
            "{RDF_ROOT_OPEN}\n  \
            <cim:ACLineSegment rdf:ID=\"_line1\">\n    \
            <cim:ACLineSegment.fromBus rdf:resource=\"#_busA\">text</cim:ACLineSegment.fromBus>\n  \
            </cim:ACLineSegment>\n\
            {RDF_ROOT_CLOSE}"
        );
        assert!(matches!(
            parse_error(both.as_bytes()),
            ProfileParseError::NotWellFormed { .. }
        ));
        let nested = format!(
            "{RDF_ROOT_OPEN}\n  \
            <cim:ACLineSegment rdf:ID=\"_line1\">\n    \
            <cim:ACLineSegment.r><cim:inner /></cim:ACLineSegment.r>\n  \
            </cim:ACLineSegment>\n\
            {RDF_ROOT_CLOSE}"
        );
        assert!(matches!(
            parse_error(nested.as_bytes()),
            ProfileParseError::NotWellFormed { .. }
        ));
    }

    #[test]
    fn text_outside_an_attribute_element_is_a_named_refusal() {
        let document = format!(
            "{RDF_ROOT_OPEN}\n  <cim:Breaker rdf:ID=\"_brk1\">loose</cim:Breaker>\n\
            {RDF_ROOT_CLOSE}"
        );
        assert!(matches!(
            parse_error(document.as_bytes()),
            ProfileParseError::NotWellFormed { .. }
        ));
    }

    #[test]
    fn a_non_rdf_root_is_not_a_cim_profile() {
        assert!(matches!(
            parse_error(b"<not-the-root/>"),
            ProfileParseError::NotACimProfile { .. }
        ));
    }

    #[test]
    fn duplicate_mrids_re_raise_through_the_model_layer() {
        let document = format!(
            "{RDF_ROOT_OPEN}\n  \
            <cim:Breaker rdf:ID=\"_brk1\" />\n  \
            <cim:Breaker rdf:ID=\"_brk1\" />\n\
            {RDF_ROOT_CLOSE}"
        );
        assert!(matches!(
            parse_error(document.as_bytes()),
            ProfileParseError::Model(
                crate::model::CimModelError::DuplicateMRid { .. }
            )
        ));
    }

    #[test]
    fn an_empty_profile_is_the_named_model_error() {
        let document = format!("{RDF_ROOT_OPEN}{RDF_ROOT_CLOSE}");
        assert!(matches!(
            parse_error(document.as_bytes()),
            ProfileParseError::Model(crate::model::CimModelError::EmptyProfile)
        ));
    }

    #[test]
    fn the_bounded_input_guard_flows_through_the_seam_as_a_reason() {
        let bounded = XmlRdfProfileSource::with_max_input_bytes(64);
        let error = bounded
            .parse_reported(b"<rdf:RDF xmlns:cim=\"urn:cim\" xmlns:rdf=\"urn:rdf\"><cim:Breaker rdf:ID=\"_brk1\" /></rdf:RDF>")
            .expect_err("the guard refuses");
        let ProfileParseError::NotWellFormed { reason } = error else {
            panic!("expected a NotWellFormed refusal");
        };
        assert!(
            reason.contains("bounded-input guard") && reason.contains("bytes"),
            "the human-safe reason names the guard and the sizes: {reason}"
        );
    }

    #[test]
    fn the_trait_impl_returns_the_document_and_keeps_the_unwired_default() {
        // The trait object shape a composition root consumes; the shipped
        // default stays the refusing one (FIM-F06.02's untouched seam).
        let wired: &dyn RdfProfileSource = &source();
        let document = format!("{RDF_ROOT_OPEN}<cim:Breaker rdf:ID=\"_brk1\" />{RDF_ROOT_CLOSE}");
        assert_eq!(wired.parse(document.as_bytes()).expect("parses").len(), 1);
        let unwired: &dyn RdfProfileSource = &crate::rdf_profile::UnwiredProfileSource;
        assert_eq!(
            unwired.parse(document.as_bytes()),
            Err(ProfileParseError::SeamNotWired)
        );
    }

    #[test]
    fn the_import_path_runs_the_existing_gate_and_assembly() {
        // FIM-F06.03: the full bytes-to-model path; the gate is the
        // EXISTING finalize (an ambiguity refuses with the existing
        // ImportError vocabulary). The fixture carries exactly one
        // external grid — the IR's exactly-one-slack rule.
        let document = format!(
            "{RDF_ROOT_OPEN}\n  \
            <cim:BaseVoltage rdf:ID=\"_bv20\">\n    \
            <cim:BaseVoltage.nominalV>20.0</cim:BaseVoltage.nominalV>\n  \
            </cim:BaseVoltage>\n  \
            <cim:ConnectivityNode rdf:ID=\"_busA\">\n    \
            <cim:ConnectivityNode.BaseVoltage rdf:resource=\"#_bv20\" />\n  \
            </cim:ConnectivityNode>\n  \
            <cim:ExternalNetworkInjection rdf:ID=\"_grid1\">\n    \
            <cim:ExternalNetworkInjection.bus rdf:resource=\"#_busA\" />\n    \
            <cim:ExternalNetworkInjection.vmPu>1.02</cim:ExternalNetworkInjection.vmPu>\n    \
            <cim:ExternalNetworkInjection.vaDegree>0.0</cim:ExternalNetworkInjection.vaDegree>\n  \
            </cim:ExternalNetworkInjection>\n\
            {RDF_ROOT_CLOSE}"
        );
        let report = parse(document.as_bytes());
        let model_ref = flexcore_grid_api::port::GridModelRef {
            model_id: "dso-feeder-42".to_string(),
            version: "1".to_string(),
            content_hash: "declared-by-the-versioned-model-store".to_string(),
        };
        let imported =
            import_document(&report.document, &model_ref, 1.0).expect("the gate passes");
        assert_eq!(imported.model.buses.len(), 1);
        assert_eq!(imported.model.buses[0].bus_id, "_busA");
        assert_eq!(imported.model.buses[0].nominal_kv, 20.0);
        assert!(
            imported.model.buses[0].is_slack,
            "the external grid's bus is the declared slack"
        );
        assert_eq!(imported.model.external_grids.len(), 1);
    }

    #[test]
    fn the_import_path_refuses_on_ambiguity_through_the_existing_gate() {
        // A line missing the declared rated current (the declared key is
        // ATTR_LINE_RATED_CURRENT_A): the mapping refuses to guess, the
        // EXISTING finalize gate refuses the import, NAMED.
        let document = format!(
            "{RDF_ROOT_OPEN}\n  \
            <cim:BaseVoltage rdf:ID=\"_bv20\">\n    \
            <cim:BaseVoltage.nominalV>20.0</cim:BaseVoltage.nominalV>\n  \
            </cim:BaseVoltage>\n  \
            <cim:ConnectivityNode rdf:ID=\"_busA\">\n    \
            <cim:ConnectivityNode.BaseVoltage rdf:resource=\"#_bv20\" />\n  \
            </cim:ConnectivityNode>\n  \
            <cim:ACLineSegment rdf:ID=\"_line1\">\n    \
            <cim:ACLineSegment.r>1.5</cim:ACLineSegment.r>\n    \
            <cim:ACLineSegment.x>4.0</cim:ACLineSegment.x>\n    \
            <cim:ACLineSegment.fromBus rdf:resource=\"#_busA\" />\n    \
            <cim:ACLineSegment.toBus rdf:resource=\"#_busA\" />\n  \
            </cim:ACLineSegment>\n\
            {RDF_ROOT_CLOSE}"
        );
        let report = parse(document.as_bytes());
        let model_ref = flexcore_grid_api::port::GridModelRef {
            model_id: "dso-feeder-42".to_string(),
            version: "1".to_string(),
            content_hash: "declared".to_string(),
        };
        assert!(matches!(
            import_document(&report.document, &model_ref, 1.0),
            Err(crate::unmapped::ImportError::AmbiguousObjectsRemain { .. })
        ));
        // The missing attribute is the declared constant, named in the
        // reason (the record 44 check stays attached).
        let register = ImportRegister::classify_profile(&report.document);
        let error = register.finalize().expect_err("the gate refuses");
        let crate::unmapped::ImportError::AmbiguousObjectsRemain { count, first_mrid } = error
        else {
            panic!("expected the ambiguity refusal");
        };
        assert_eq!(count, 1);
        assert_eq!(first_mrid, "_line1");
        assert!(matches!(
            register.status("_line1"),
            Some(crate::class_mapping::ClassMappingStatus::Ambiguous { reason })
                if reason.contains(ATTR_LINE_RATED_CURRENT_A)
        ));
    }
}
