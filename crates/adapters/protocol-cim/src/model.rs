// P14.12 — The CIM object vocabulary (decision record
// 44_p14_12_cim_cgmes_import.md; sources [S-17][S-18]).
//
// UNVERIFIED HONESTY (plan §7 note 6): NO CIM schema text or CGMES profile
// specification is in the repository. The class names below are common
// public CIM spellings ([S-17] the ENTSO-E CGMES overview; [S-18] the IEC
// 61968-5 catalog entry — the standards' texts were NOT consulted), and
// every attribute-name constant is a DECLARED mapping-profile key, not a
// specification quote. The implementation-time check confirms: exact class
// spellings per CGMES conformance profile, attribute owners and value
// units, the RDF resource-reference shape, and which profile (EQ, TP, SV,
// DY) each attribute belongs to. A mismatch is a recorded edit to the
// named constant, never a silent reinterpretation.
//
// THE VOCABULARY RULE: unknown class names are NOT a construction error —
// a real profile will contain classes this import never declared interest
// in. They flow into the EXPLICITLY_UNMAPPED register ([`crate::unmapped`])
// with a recorded reason. Guessing them into the network model is the one
// thing this crate must never do ([S-17][S-18] honesty; the plan §2
// P14.12 row).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

// --- Declared attribute-name constants (the mapping-profile keys) -----------
//
// All UNVERIFIED: the implementation-time check confirms the exact
// attribute owner, spelling, and value unit against the CIM schema text
// (and the CGMES profile that carries it). Each constant names its check.

/// The nominal voltage of a `BaseVoltage` object. UNVERIFIED — the check
/// confirms the class/attribute spelling and that the value unit is kV.
pub const ATTR_BASE_VOLTAGE_NOMINAL_KV: &str = "BaseVoltage.nominalV";

/// The BaseVoltage reference of a connectivity node (an RDF resource
/// reference carrying the referenced object's mRID). UNVERIFIED — the
/// real CIM path to a node's voltage level may go through VoltageLevel;
/// the check confirms the reference chain before this mapping composes.
pub const ATTR_NODE_BASE_VOLTAGE_REF: &str = "ConnectivityNode.BaseVoltage";

/// Line resistance, declared in ohm. UNVERIFIED — the check confirms the
/// attribute spelling and the ohm unit (and whether CGMES carries it per
/// length with a length attribute this mapping would need).
pub const ATTR_LINE_R_OHM: &str = "ACLineSegment.r";

/// Line reactance, declared in ohm. UNVERIFIED — same check as
/// [`ATTR_LINE_R_OHM`].
pub const ATTR_LINE_X_OHM: &str = "ACLineSegment.x";

/// The line's thermal rating, declared in amperes. UNVERIFIED — public
/// CGMES descriptions carry line ratings through OperationalLimitSet
/// objects, NOT a direct attribute; the check confirms whether a direct
/// attribute exists or the mapping must traverse the limit-set path.
pub const ATTR_LINE_RATED_CURRENT_A: &str = "ACLineSegment.ratedCurrent";

/// The transformer's rated apparent power, declared in VA. UNVERIFIED —
/// public CIM descriptions carry ratedS on TransformerEnd /
/// PowerTransformerEnd rather than the transformer itself; the check
/// confirms the owning class before this mapping composes.
pub const ATTR_TRANSFORMER_RATED_S_VA: &str = "PowerTransformer.ratedS";

/// The transformer short-circuit voltage vk, declared in percent.
/// UNVERIFIED — short-circuit data in CIM lives in TransformerEndInfo /
/// ShortCircuitTest objects; the check confirms the derivation path
/// (this mapping DECLARED a direct attribute and refuses when absent).
pub const ATTR_TRANSFORMER_VK_PERCENT: &str = "PowerTransformer.vkPercent";

/// The real part of vk, declared in percent. UNVERIFIED — same check as
/// [`ATTR_TRANSFORMER_VK_PERCENT`].
pub const ATTR_TRANSFORMER_VK_R_PERCENT: &str = "PowerTransformer.vkRPercent";

/// The high-voltage bus reference of a two-winding transformer. UNVERIFIED
/// — the winding/end structure is the real CIM carrier; the check confirms
/// how the two ends map to hv/lv before this declared key composes.
pub const ATTR_TRANSFORMER_HV_BUS_REF: &str = "PowerTransformer.hvBus";

/// The low-voltage bus reference of a two-winding transformer. UNVERIFIED
/// — see [`ATTR_TRANSFORMER_HV_BUS_REF`].
pub const ATTR_TRANSFORMER_LV_BUS_REF: &str = "PowerTransformer.lvBus";

/// The from-bus reference of a line. UNVERIFIED — the real path goes
/// through Terminals and connectivity; the check confirms the traversal
/// (this mapping DECLARED a direct reference and refuses when absent).
pub const ATTR_LINE_FROM_BUS_REF: &str = "ACLineSegment.fromBus";

/// The to-bus reference of a line. UNVERIFIED — see
/// [`ATTR_LINE_FROM_BUS_REF`].
pub const ATTR_LINE_TO_BUS_REF: &str = "ACLineSegment.toBus";

/// The slack-bus reference of an external network injection. UNVERIFIED —
/// the check confirms the class spelling (ExternalNetworkInjection vs
/// EnergySource) and the reference attribute.
pub const ATTR_EXTERNAL_GRID_BUS_REF: &str = "ExternalNetworkInjection.bus";

/// The slack voltage magnitude, declared per-unit. UNVERIFIED — the check
/// confirms the attribute and its per-unit convention.
pub const ATTR_EXTERNAL_GRID_VM_PU: &str = "ExternalNetworkInjection.vmPu";

/// The slack voltage angle, declared in degrees. UNVERIFIED — see
/// [`ATTR_EXTERNAL_GRID_VM_PU`].
pub const ATTR_EXTERNAL_GRID_VA_DEGREE: &str = "ExternalNetworkInjection.vaDegree";

// --- Identity ---------------------------------------------------------------

/// Errors returned by CIM model construction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CimModelError {
    /// An identifier or attribute key is empty. Silence is never a model.
    EmptyField(&'static str),
    /// The profile document contains two objects with the same mRID.
    DuplicateMRid {
        mrid: String,
    },
    /// The profile document is empty — an empty profile imports nothing
    /// and is refused (a non-empty network is the only honest import).
    EmptyProfile,
}

impl std::fmt::Display for CimModelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CimModelError::EmptyField(field) => write!(f, "CIM model field '{field}' is empty"),
            CimModelError::DuplicateMRid { mrid } => {
                write!(f, "duplicate object mRID '{mrid}' in the profile document")
            }
            CimModelError::EmptyProfile => {
                write!(f, "the profile document contains no objects")
            }
        }
    }
}

impl std::error::Error for CimModelError {}

/// The CIM object identifier (mRID). In RDF/XML profiles this is the
/// `rdf:ID` of the object element — the parse seam
/// ([`crate::rdf_profile`]) assigns it; this type validates non-empty.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct CimMRid(String);

impl CimMRid {
    pub fn new(raw: &str) -> Result<Self, CimModelError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(CimModelError::EmptyField("mrid"));
        }
        Ok(Self(trimmed.to_string()))
    }

    pub fn value(&self) -> &str {
        &self.0
    }
}

/// One CIM object as typed profile data: the mRID, the class name (the
/// raw spelling from the profile — recognition happens at the mapping,
/// never at parse time), and the declared attributes. Reference-valued
/// attributes carry the referenced object's mRID as the value (the parse
/// seam normalizes RDF resource references into that shape — UNVERIFIED
/// normalization rule named in [`crate::rdf_profile`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CimObject {
    pub mrid: CimMRid,
    /// The raw class name from the profile (e.g. `ACLineSegment`).
    /// UNVERIFIED spelling; the mapping vocabulary recognizes a declared
    /// subset and records everything else as UNMAPPED.
    pub class_name: String,
    /// Declared attributes, ordered (BTreeMap — the determinism rule).
    pub attributes: BTreeMap<String, String>,
}

impl CimObject {
    /// The value of one declared attribute, if present.
    pub fn attribute(&self, key: &str) -> Option<&str> {
        self.attributes.get(key).map(|v| v.as_str())
    }
}

/// The recognized-class vocabulary of this import (the classes the
/// mapping DECLARED an interest in). UNVERIFIED spellings — the
/// implementation-time check confirms each name against the CGMES
/// profile text [S-17].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CimKnownClass {
    /// A distribution line / cable.
    AcLineSegment,
    /// A two-winding power transformer.
    PowerTransformer,
    /// A voltage reference table entry.
    BaseVoltage,
    /// A connectivity point (the declared bus source of this import).
    ConnectivityNode,
    /// A load.
    EnergyConsumer,
    /// DER plant (inverter-based equipment).
    PowerElectronicsConnection,
    /// A DER unit within [`CimKnownClass::PowerElectronicsConnection`].
    PowerElectronicsUnit,
    /// An external grid / slack injection.
    ExternalNetworkInjection,
    /// A breaker.
    Breaker,
    /// A disconnector.
    Disconnector,
    /// A synchronous machine.
    SynchronousMachine,
    /// A terminal (connectivity refinement).
    Terminal,
    /// The container hierarchy root.
    Substation,
    /// A voltage level within a substation.
    VoltageLevel,
    /// A bay within a voltage level.
    Bay,
}

impl CimKnownClass {
    /// The declared class-name spelling. UNVERIFIED — see the module
    /// documentation.
    pub const fn name(self) -> &'static str {
        match self {
            CimKnownClass::AcLineSegment => "ACLineSegment",
            CimKnownClass::PowerTransformer => "PowerTransformer",
            CimKnownClass::BaseVoltage => "BaseVoltage",
            CimKnownClass::ConnectivityNode => "ConnectivityNode",
            CimKnownClass::EnergyConsumer => "EnergyConsumer",
            CimKnownClass::PowerElectronicsConnection => "PowerElectronicsConnection",
            CimKnownClass::PowerElectronicsUnit => "PowerElectronicsUnit",
            CimKnownClass::ExternalNetworkInjection => "ExternalNetworkInjection",
            CimKnownClass::Breaker => "Breaker",
            CimKnownClass::Disconnector => "Disconnector",
            CimKnownClass::SynchronousMachine => "SynchronousMachine",
            CimKnownClass::Terminal => "Terminal",
            CimKnownClass::Substation => "Substation",
            CimKnownClass::VoltageLevel => "VoltageLevel",
            CimKnownClass::Bay => "Bay",
        }
    }
}

/// A parsed profile document: the objects, ordered by mRID (BTreeMap —
/// the determinism rule; the import result must not depend on the
/// document's element order).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileDocument {
    objects: BTreeMap<CimMRid, CimObject>,
}

impl ProfileDocument {
    /// Assembles the profile document; refuses an empty document and
    /// duplicate mRIDs (a duplicate would make the import depend on which
    /// copy the parser saw — never accepted).
    pub fn new(objects: Vec<CimObject>) -> Result<Self, CimModelError> {
        if objects.is_empty() {
            return Err(CimModelError::EmptyProfile);
        }
        let mut map = BTreeMap::new();
        for object in objects {
            if map.contains_key(&object.mrid) {
                return Err(CimModelError::DuplicateMRid {
                    mrid: object.mrid.value().to_string(),
                });
            }
            map.insert(object.mrid.clone(), object);
        }
        Ok(Self { objects: map })
    }

    /// The objects, deterministically ordered by mRID.
    pub fn objects(&self) -> impl Iterator<Item = &CimObject> {
        self.objects.values()
    }

    /// The object with the given mRID, if present.
    pub fn object(&self, mrid: &CimMRid) -> Option<&CimObject> {
        self.objects.get(mrid)
    }

    /// The number of objects in the document.
    pub fn len(&self) -> usize {
        self.objects.len()
    }

    /// Whether the document is empty (defensive; [`ProfileDocument::new`]
    /// refuses empty documents).
    pub fn is_empty(&self) -> bool {
        self.objects.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn object(mrid: &str, class: &str, key: &str, value: &str) -> CimObject {
        let mut attributes = BTreeMap::new();
        attributes.insert(key.to_string(), value.to_string());
        CimObject {
            mrid: CimMRid::new(mrid).expect("valid"),
            class_name: class.to_string(),
            attributes,
        }
    }

    #[test]
    fn profile_documents_refuse_empty_and_duplicates() {
        assert_eq!(
            ProfileDocument::new(Vec::new()),
            Err(CimModelError::EmptyProfile)
        );
        assert_eq!(
            ProfileDocument::new(vec![
                object("_line1", "ACLineSegment", ATTR_LINE_R_OHM, "1.5"),
                object(" _line1 ", "BaseVoltage", ATTR_BASE_VOLTAGE_NOMINAL_KV, "20"),
            ]),
            Err(CimModelError::DuplicateMRid {
                mrid: "_line1".to_string(),
            })
        );
    }

    #[test]
    fn objects_are_ordered_by_mrid_regardless_of_document_order() {
        let document =
            ProfileDocument::new(vec![
                object("_zzz", "BaseVoltage", ATTR_BASE_VOLTAGE_NOMINAL_KV, "20"),
                object("_aaa", "ACLineSegment", ATTR_LINE_R_OHM, "1.5"),
            ])
            .expect("valid");
        let mrids: Vec<&str> = document.objects().map(|o| o.mrid.value()).collect();
        assert_eq!(mrids, vec!["_aaa", "_zzz"], "BTreeMap determinism");
    }

    #[test]
    fn attribute_lookup_is_declared_key_only() {
        let document = ProfileDocument::new(vec![object(
            "_line1",
            "ACLineSegment",
            ATTR_LINE_R_OHM,
            "1.5",
        )])
        .expect("valid");
        let line = document.object(&CimMRid::new("_line1").expect("valid")).expect("present");
        assert_eq!(line.attribute(ATTR_LINE_R_OHM), Some("1.5"));
        assert_eq!(line.attribute(ATTR_LINE_X_OHM), None, "absent is None, never defaulted");
    }

    #[test]
    fn mrid_rejects_empty() {
        assert_eq!(CimMRid::new("  "), Err(CimModelError::EmptyField("mrid")));
    }
}
