// P14.12 — The RDF/XML profile-source SEAM (decision record
// 44_p14_12_cim_cgmes_import.md; the record 02 §5 template: design the
// transport, land no dependency).
//
// There is NO XML/RDF parser in this crate's dependency line. The
// RDF/XML parsing of a CGMES profile is a designed seam exactly like the
// other protocol adapters' wire transports: the trait below is the FULL
// surface a composition root would implement (over an XML parser chosen
// at implementation time, with the human-gated decision recorded), and
// [`UnwiredProfileSource`] is the shipped default that refuses every
// parse with [`ProfileParseError::SeamNotWired`].
//
// THE DECLARED NORMALIZATION RULE (what a real parser must produce): a
// profile is normalized into a [`ProfileDocument`] where —
//
// 1. every object element becomes one [`crate::model::CimObject`] whose
//    mRID is the element's `rdf:ID` (UNVERIFIED: whether the target
//    profiles use `rdf:ID` or `rdf:about`, and the exact `cim:` namespace
//    URIs, are implementation-time checks against the CGMES profile
//    text [S-17]);
// 2. every attribute element becomes one declared-attribute entry keyed
//    by the DECLARED attribute constants of [`crate::model`] (the
//    normalization maps the raw RDF predicate to the mapping-profile
//    key; an un-mappable predicate is DROPPED and recorded by the parser
//    as a parse warning — never guessed into a key);
// 3. every resource reference (`rdf:resource`) becomes the referenced
//    object's mRID as the attribute value (the mapping's declared
//    reference shape);
// 4. object ordering is irrelevant: the [`ProfileDocument`] stores its
//    objects in a BTreeMap, so the import result must not depend on the
//    document's element order (the determinism rule).
//
// Parse errors are named, never swallowed; a partial profile is never a
// successful parse.

use crate::model::{ProfileDocument, CimModelError};

/// Errors returned by the profile-source seam.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileParseError {
    /// The bytes are not a parseable RDF/XML document (a real parser
    /// reports position and element; this seam carries a human-safe
    /// reason — the P1.08 rule, never raw parser output).
    NotWellFormed {
        reason: String,
    },
    /// The document parsed but is not a CIM profile (no recognized
    /// namespace, no object elements). UNVERIFIED — the exact namespace
    /// URIs are an implementation-time check against the CGMES text
    /// [S-17].
    NotACimProfile {
        reason: String,
    },
    /// The document violates the declared normalization rules (duplicate
    /// mRIDs, an empty document) — re-raised from the model layer.
    Model(crate::model::CimModelError),
    /// The shipped [`UnwiredProfileSource`] refused the operation: no
    /// RDF/XML parser exists in this batch.
    SeamNotWired,
}

impl std::fmt::Display for ProfileParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProfileParseError::NotWellFormed { reason } => {
                write!(f, "the profile bytes are not well-formed RDF/XML: {reason}")
            }
            ProfileParseError::NotACimProfile { reason } => {
                write!(f, "the document is not a recognized CIM profile: {reason}")
            }
            ProfileParseError::Model(error) => write!(f, "the parsed document is not a valid profile: {error}"),
            ProfileParseError::SeamNotWired => write!(
                f,
                "the RDF/XML profile parser is a designed, UNWIRED seam — no parser \
                 dependency exists (decision record 44)"
            ),
        }
    }
}

impl std::error::Error for ProfileParseError {}

impl From<CimModelError> for ProfileParseError {
    fn from(error: CimModelError) -> Self {
        ProfileParseError::Model(error)
    }
}

/// The RDF/XML profile-source seam: the full surface a composition root
/// would implement over an XML parser chosen at implementation time.
/// Sync trait (the `flexcore-ports` house rule); the adapter owns no I/O
/// runtime.
///
/// Deliberately NO write method and NO network method: model import is a
/// one-way parse of supplied bytes. Where the bytes come from (a DSO
/// file drop, a controlled fetch) is a composition-root concern with its
/// own reviewed security boundary — never this crate's.
pub trait RdfProfileSource {
    /// Parses profile bytes into a normalized [`ProfileDocument`].
    /// Returns a named error for anything that is not a valid profile;
    /// a partial document is never a success.
    fn parse(&self, bytes: &[u8]) -> Result<ProfileDocument, ProfileParseError>;
}

/// The shipped default source: refuses every parse with
/// [`ProfileParseError::SeamNotWired`]. It exists so a composition root
/// has a constructible, honest placeholder — and so nothing can silently
/// "import" without a real parser.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct UnwiredProfileSource;

impl RdfProfileSource for UnwiredProfileSource {
    fn parse(&self, _bytes: &[u8]) -> Result<ProfileDocument, ProfileParseError> {
        Err(ProfileParseError::SeamNotWired)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{CimObject, CimMRid};
    use std::collections::BTreeMap;

    #[test]
    fn the_unwired_source_refuses_every_parse() {
        let source = UnwiredProfileSource;
        assert_eq!(
            source.parse(b"<not-a-profile/>"),
            Err(ProfileParseError::SeamNotWired)
        );
    }

    #[test]
    fn model_errors_re_raise_through_the_seam() {
        // The seam's From<CimModelError> keeps the named model errors
        // intact: a duplicate mRID is a Model error, not a parse
        // blanket.
        let error = ProfileParseError::from(CimModelError::EmptyProfile);
        assert_eq!(error, ProfileParseError::Model(CimModelError::EmptyProfile));
    }

    #[test]
    fn the_normalization_shape_is_the_profile_document() {
        // The seam's declared output type: objects keyed by mRID, with
        // declared-attribute entries. This test constructs one by hand to
        // pin the shape a real parser must produce (no parser exists).
        let mut attributes = BTreeMap::new();
        attributes.insert(
            crate::model::ATTR_BASE_VOLTAGE_NOMINAL_KV.to_string(),
            "20.0".to_string(),
        );
        let object = CimObject {
            mrid: CimMRid::new("_bv20").expect("valid"),
            class_name: "BaseVoltage".to_string(),
            attributes,
        };
        let document = ProfileDocument::new(vec![object]).expect("valid");
        assert_eq!(document.len(), 1);
    }
}
