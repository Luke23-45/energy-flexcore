//! P14.12 — CIM/CGMES network-model import adapter models
//! (crates/adapters/protocol-cim).
//!
//! Decision record:
//! `docs/phases-11-20/phase-14-protocol-device-connectivity/records/44_p14_12_cim_cgmes_import.md`.
//! Sources: [S-17] (ENTSO-E CIM for grid-model exchange — the CGMES
//! profile overview), [S-18] (IEC 61968-5:2020 — DER group management;
//! the standard text was NOT consulted). Plan: phase 14 plan §2 row
//! P14.12.
//!
//! What this crate IS (the honest scope):
//!
//! - The **CIM object vocabulary** ([`model`]): profile documents as typed
//!   data (objects with mRID, class name, declared attributes). The class
//!   vocabulary and every attribute-name spelling are UNVERIFIED
//!   secondary-source readings pending the CGMES/CIM schema text.
//! - The **RDF/XML profile-source SEAM** ([`rdf_profile`]): the parsing
//!   design as a source trait plus the shipped unwired default — no XML
//!   parser dependency lands in this batch (the same designed-seam rule
//!   as the other protocol adapters' wire transports).
//! - The **declared class mapping** ([`class_mapping`]) into the grid-api
//!   network IR (`flexcore-grid-api::port::VersionedNetworkModel`), with
//!   the openems.rs classification pattern carried forward verbatim:
//!   every object is EXACTLY ONE of MAPPED / EXPLICITLY_UNMAPPED /
//!   UNMAPPED / AMBIGUOUS — never mapped by guessing.
//! - The **EXPLICITLY_UNMAPPED register** ([`unmapped`]): the fail-closed
//!   completeness gate — an import summary exists only when every
//!   profile object is classified, and every ambiguous object is named.
//! - The **IEC 61968-5 DER-group registry** ([`der_groups`]): group
//!   declarations as registry data (payload structure UNVERIFIED); no
//!   group control path — commands go through the command lifecycle, not
//!   this crate.
//! - A **scope declaration + capability manifest** ([`scope`]) consuming
//!   the COMMON protocol port layer (P14.01,
//!   `crates/flexcore-protocol-ports`).
//!
//! What this crate is NOT:
//!
//! - Not an RDF/XML parser. No XML crate, no CGMES profile conformance;
//!   the parsing is a designed, UNWIRED seam.
//! - Not a full CIM profile: the class vocabulary covers the classes this
//!   import DECLARED an interest in; everything else is recorded as
//!   UNMAPPED — never guessed into the network model.
//! - Not connected: no profile file is read, no DSO boundary is touched,
//!   no model is exchanged.
//!
//! Standard-detail honesty (plan §7 note 6): every CIM class name, RDF
//! namespace, attribute spelling, and unit declaration below is
//! UNVERIFIED against the IEC 61970/61968 CIM schema and the CGMES
//! conformance profiles [S-17]; the implementation-time check is named at
//! each point. A mismatch is a recorded edit, never a silent
//! reinterpretation.

pub mod class_mapping;
pub mod der_groups;
pub mod model;
pub mod rdf_profile;
pub mod scope;
pub mod unmapped;

// P14.18 — the registered adapter id (the composition-registry allowlist
// key spelling; the P14.02 constant pattern — this constant is the
// canonical spelling, decision record 82). The spelling matches the id
// this crate's own scope tests construct with.
pub const ADAPTER_ID: &str = "cim-cgmes-import";
