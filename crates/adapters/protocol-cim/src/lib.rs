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
//!   secondary-source readings pending the CGMES/CIM schema text; the
//!   FIM-F06.05 implementation-time verification pass and its per-item
//!   verdicts are recorded in decision record
//!   `44a_p14_12_fim_class_spelling_verification.md`.
//! - The **RDF/XML profile-source SEAM** ([`rdf_profile`]): the source
//!   trait plus the shipped unwired default (`UnwiredProfileSource` — the
//!   DEFAULT until a composition root selects the real source).
//! - The **hand-written RDF/XML PULL parser** ([`xml_pull`], FIM-F06.01):
//!   a byte-level scanner over the CGMES profile SUBSET of RDF/XML. No XML
//!   crate enters the dependency line (FIM standing decision 1);
//!   CDATA/DTDs/XInclude/external entities are NAMED refusals. The
//!   implementation is WRITTEN-UNRUN, and interop with a real DSO profile
//!   is UNPROVEN — a HUMAN LAB GATE.
//! - The **wired profile source + import path** ([`xml_profile`],
//!   FIM-F06.02/03/04): `XmlRdfProfileSource` implements the seam over the
//!   parser, producing exactly the existing vocabulary; the import runs
//!   through the EXISTING register and completeness gate — no parallel
//!   gate, no parallel register.
//! - The **declared class mapping** ([`class_mapping`]) into the grid-api
//!   network IR (`flexcore-grid-api::port::VersionedNetworkModel`), with
//!   the openems.rs classification pattern carried forward verbatim:
//!   every object is EXACTLY ONE of MAPPED / EXPLICITLY_UNMAPPED /
//!   UNMAPPED / AMBIGUOUS — never mapped by guessing.
//! - The **EXPLICITLY_UNMAPPED register** ([`unmapped`]): the fail-closed
//!   completeness gate — an import summary exists only when every
//!   profile object is classified, and every ambiguous object is named.
//! - The **CGMES verified-spelling remediation register**
//!   ([`spelling_remediation`], FIM wave 8B): the ADDITIVE application of
//!   the verified MISMATCH verdicts of record
//!   `44a_p14_12_fim_class_spelling_verification.md` — the 2 VERIFIED
//!   replacement spellings accepted as aliases onto the canonical declared
//!   keys (e.g. `BaseVoltage.nominalVoltage` → the nominal-kV attribute the
//!   declared key `BaseVoltage.nominalV` serves), and the 9 verified-absent
//!   declared keys recorded with named not-modelled statements (NOT
//!   aliases). Every declared key stays accepted exactly as before; each
//!   remediation entry cites the 44a record and its [S-17x] source.
//!   WRITTEN-UNRUN; real-profile interop stays the HUMAN LAB GATE.
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
//! - Not a full RDF/XML processor: the FIM-F06 parser is a CGMES profile
//!   SUBSET scanner (no CDATA, no DTDs, no external entities, no XInclude
//!   — each a named refusal). No XML crate, no CGMES profile conformance
//!   claim of any kind (conventions §7.3); the parser has never been
//!   compiled or executed, and interop with a real DSO profile is
//!   UNPROVEN (HUMAN LAB GATE).
//! - Not a full CIM profile: the class vocabulary covers the classes this
//!   import DECLARED an interest in; everything else is recorded as
//!   UNMAPPED — never guessed into the network model.
//! - Not connected: no profile file has ever been READ BY A RUN (the
//!   FIM-F06 parsing is written-unrun), no DSO boundary is touched, no
//!   model is exchanged.
//!
//! Standard-detail honesty (plan §7 note 6): every CIM class name, RDF
//! namespace, attribute spelling, and unit declaration below is
//! UNVERIFIED against the IEC 61970/61968 CIM schema and the CGMES
//! conformance profiles [S-17], except where the FIM-F06.05 verification
//! record (`44a_p14_12_fim_class_spelling_verification.md`) marks a
//! per-item verdict with its source. A mismatch is a recorded edit, never
//! a silent reinterpretation.

pub mod class_mapping;
pub mod der_groups;
pub mod model;
pub mod rdf_profile;
pub mod scope;
// FIM wave 8B (Agent B; WRITTEN-UNRUN) — the CGMES verified-spelling
// remediation register (the additive application of record 44a's verified
// MISMATCH verdicts: 2 accepted aliases + 9 verified-absent entries; see
// the module header for the recorded honest arithmetic).
pub mod spelling_remediation;
pub mod unmapped;
pub mod xml_profile;
pub mod xml_pull;

// P14.18 — the registered adapter id (the composition-registry allowlist
// key spelling; the P14.02 constant pattern — this constant is the
// canonical spelling, decision record 82). The spelling matches the id
// this crate's own scope tests construct with.
pub const ADAPTER_ID: &str = "cim-cgmes-import";
