// P15.15 — Interconnection-record ingestion: UL 1741 SB Certificates of
//         Conformance (COC), IEEE 1547.1-2020 test reports, certified
//         function categories, and settings-profile references per DEVICE
//         MODEL — a document store with model-number linkage to the
//         registry.
//
// Phase 15 plan §2 row P15.15; decision record:
// `docs/phases-11-20/phase-15-forecasting-asset-intelligence/records/
// 44_p15_15_interconnection_record_ingestion.md`.
// Sources: [S-21] (CEC: the UL 1741 SB addition to the Solar Equipment
// Lists — certified equipment is LISTED by the certification bodies; a
// platform cannot self-certify), [S-169] (the CHINT bulletin on
// IEEE 1547-2018 / UL 1741 SB), [S-170] (UL 1741 SB vs IEEE 1547.1-2020
// test procedures). No certification, listing, or conformance claim is
// made by or for FlexCore anywhere in this module.
//
// THE BOUNDARY, DECIDED ONCE:
//
// - **The platform LISTS certified equipment — it never claims
//   certification for itself, and it never certifies equipment.** An
//   interconnection record is a REFERENCE to a document issued by a
//   certification body / laboratory: who issued it, what device model it
//   covers, what it certifies. The record's honesty lives in its
//   reference and provenance; the certification itself belongs to the
//   document, never to this platform.
// - **Model-number linkage is EXACT string equality** on the registry's
//   `vendor_model` field (P0.10 §1 row 4 — `Asset::vendor_model`): no
//   fuzzy, no prefix, no normalization. A near-miss model number FLAGS
//   (`linkage` lookup returns nothing); it never matches (the P8.11
//   exact-equality discipline mirrored).
// - **Function categories reuse the P14.13 catalog** (`SettingFunctionId`
//   and `RideThroughCategory`, single home — never restated). The
//   certified-function list on a record declares which catalog functions
//   the document's test scope covered; it is the document's claim,
//   recorded, not this platform's assertion about any device.
// - **Records are immutable and additive-by-version**: a corrected or
//   renewed document is a NEW record referencing the new document
//   version; nothing is edited in place.
// - **Per-DEVICE-MODEL scope, tenant-neutral.** Certification is a
//   property of the equipment model, not of any tenant's instance of it.
//   The store is a platform-global equipment catalog (the same posture
//   as the P14.13 jurisdiction document store); no tenant scoping, no
//   RLS-by-tenant on the persistence side (the migration records this).
//
// Control status: records for the simulation-only 0.x line. No I/O, no
// field-write path, and nothing here implies certification, safety, or
// field-control capability.

use std::collections::BTreeMap;

use crate::ieee1547_settings::{RideThroughCategory, SettingFunctionId};

/// The interconnection-record schema version (a change is a NEW version,
/// never a silent edit).
pub const INTERCONNECTION_RECORD_SCHEMA_VERSION: u32 = 1;

/// The listing-not-certification statement (pinned by tests).
pub const PLATFORM_LISTS_NEVER_CERTIFIES_STATEMENT: &str = "FlexCore LISTS certified \
     equipment: an interconnection record is a reference to a document issued by a \
     certification body or test laboratory (who issued it, what device model it covers, \
     what it certifies). The platform NEVER claims certification for itself, never \
     certifies equipment, and never interprets a listed document as permission for any \
     control action [S-21][S-169][S-170]. Certification belongs to the document and its \
     issuer, never to this platform.";

/// The exact-linkage statement (pinned by tests).
pub const MODEL_LINKAGE_IS_EXACT_STATEMENT: &str = "Model-number linkage is EXACT string \
     equality on the registry's vendor_model field (P0.10 §1 row 4). No fuzzy matching, \
     no prefix matching, no case or whitespace normalization: a near-miss model number \
     resolves to nothing and FLAGS, never matches.";

/// The immutability statement (pinned by tests).
pub const INTERCONNECTION_RECORDS_IMMUTABLE_STATEMENT: &str = "An interconnection record \
     is immutable: a corrected, renewed, or withdrawn document is a NEW record \
     referencing the new document version. Nothing is edited in place; the record set is \
     the platform's evidence of what was listed, when, by whom.";

// --- Errors -------------------------------------------------------------------

/// Why an interconnection record was refused. Named, never absorbed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InterconnectionRecordError {
    /// The device-model string was empty or whitespace-only.
    EmptyDeviceModel,
    /// The device-model string exceeded the registry's vendor_model
    /// bound (256 chars, the P15.09 source-bound convention mirrored).
    DeviceModelTooLong { len: usize, max: usize },
    /// The document reference was empty.
    EmptyDocumentRef,
    /// The document reference exceeded 256 characters.
    DocumentRefTooLong { len: usize, max: usize },
    /// The issuer reference was empty (who issued the document must be
    /// recorded — never silent).
    EmptyIssuer,
    /// The certified-function list was empty (a document that certifies
    /// nothing is not recordable).
    EmptyCertifiedFunctions,
    /// The record was already registered under the same (device model,
    /// document reference) identity.
    DuplicateRecord { device_model: String, document_ref: String },
    /// The record was not found for the requested device model.
    NotFound { device_model: String },
}

impl std::fmt::Display for InterconnectionRecordError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InterconnectionRecordError::EmptyDeviceModel => {
                write!(f, "the device-model string must be non-empty")
            }
            InterconnectionRecordError::DeviceModelTooLong { len, max } => write!(
                f,
                "the device-model string is {len} characters; the bound is {max}"
            ),
            InterconnectionRecordError::EmptyDocumentRef => {
                write!(f, "the document reference must be non-empty")
            }
            InterconnectionRecordError::DocumentRefTooLong { len, max } => write!(
                f,
                "the document reference is {len} characters; the bound is {max}"
            ),
            InterconnectionRecordError::EmptyIssuer => write!(
                f,
                "the issuer reference must be recorded (never silent) — who issued the \
                 document is part of the listing's honesty"
            ),
            InterconnectionRecordError::EmptyCertifiedFunctions => write!(
                f,
                "the certified-function list is empty; a document that certifies nothing \
                 is not recordable"
            ),
            InterconnectionRecordError::DuplicateRecord {
                device_model,
                document_ref,
            } => write!(
                f,
                "an interconnection record for device model {device_model} with document \
                 reference {document_ref} is already registered; records are immutable — \
                 a new document version is a NEW record"
            ),
            InterconnectionRecordError::NotFound { device_model } => write!(
                f,
                "no interconnection record exists for device model {device_model}; the \
                 linkage resolves to nothing and FLAGS — it never matches approximately"
            ),
        }
    }
}

impl std::error::Error for InterconnectionRecordError {}

// --- Closed vocabularies ---------------------------------------------------------

/// The document kind (closed; additive variants are a schema-version
/// event, never a silent widening).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum InterconnectionDocumentKind {
    /// A UL 1741 SB Certificate of Conformance (COC) [S-21][S-169].
    Ul1741SbCoc,
    /// An IEEE 1547.1-2020 test report [S-170].
    Ieee1547TestReport,
}

impl InterconnectionDocumentKind {
    /// All kinds, in declaration order.
    pub const ALL: [InterconnectionDocumentKind; 2] = [
        InterconnectionDocumentKind::Ul1741SbCoc,
        InterconnectionDocumentKind::Ieee1547TestReport,
    ];

    /// The canonical wire spelling (records and audit details).
    pub const fn name(self) -> &'static str {
        match self {
            InterconnectionDocumentKind::Ul1741SbCoc => "ul_1741_sb_coc",
            InterconnectionDocumentKind::Ieee1547TestReport => "ieee_1547_test_report",
        }
    }
}

/// The abnormal-condition-performance category the document's test scope
/// covered, as declared on the document. **UNVERIFIED (secondary-source
/// transcription):** the Category A / B classification vocabulary is
/// cited from [S-169][S-170] secondary material over IEEE 1547-2018
/// [S-20]; the implementation-time check against the [S-20] text must
/// confirm the category letters and their assignment rules. The category
/// is RECORDED DATA — this module never interprets what a category
/// implies for device behaviour (the P14.13 posture mirrored).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CertifiedFunctionCategory {
    /// The document's test scope covered Category A performance.
    /// UNVERIFIED against [S-20].
    CategoryA,
    /// The document's test scope covered Category B performance.
    /// UNVERIFIED against [S-20].
    CategoryB,
    /// The document does not declare an A/B category (older documents,
    /// or a scope the category vocabulary does not cover). Recorded as
    /// not-classified — never guessed.
    NotClassified,
}

impl CertifiedFunctionCategory {
    /// The canonical wire spelling.
    pub const fn name(self) -> &'static str {
        match self {
            CertifiedFunctionCategory::CategoryA => "category_a",
            CertifiedFunctionCategory::CategoryB => "category_b",
            CertifiedFunctionCategory::NotClassified => "not_classified",
        }
    }
}

// --- The record -----------------------------------------------------------------

/// One interconnection document record per DEVICE MODEL (immutable;
/// additive by version). This is the ingested, listed-equipment datum —
/// a reference to a certification document, never a certification claim
/// by this platform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterconnectionRecord {
    /// The device model the document covers: EXACTLY the registry's
    /// `vendor_model` string (P0.10 §1 row 4). Exact-match linkage only.
    pub device_model: String,
    /// The document kind.
    pub document_kind: InterconnectionDocumentKind,
    /// The document reference (the issuer's document id / URI — how a
    /// human retrieves the actual document). Recorded data, bounded at
    /// 256 chars.
    pub document_ref: String,
    /// The document version as the ISSUER labels it (e.g. a COC revision
    /// or report revision). Recorded data — never invented here.
    pub document_version: String,
    /// Who issued the document (the certification body / laboratory
    /// reference). Recorded data, never silent.
    pub issuer: String,
    /// The abnormal-condition-performance category the test scope
    /// covered (recorded from the document; UNVERIFIED vocabulary — see
    /// the enum header).
    pub category: CertifiedFunctionCategory,
    /// The P14.13 catalog functions the document's test scope covered
    /// (the document's claim, recorded; sorted + deduplicated at
    /// construction).
    pub certified_functions: Vec<SettingFunctionId>,
    /// The ride-through categories the document's scope covered
    /// (recorded; sorted + deduplicated).
    pub ride_through_categories: Vec<RideThroughCategory>,
    /// The settings-profile reference this record links to (the P14.13
    /// settings-profile repository's jurisdiction/profile reference for
    /// this device model — a REFERENCE string; the profile itself lives
    /// in the P14.13 store).
    pub settings_profile_ref: String,
    /// When the record was ingested (unix millis; the INGESTION time,
    /// distinct from any document date — the P0.09 time-separation
    /// discipline).
    pub ingested_at_unix_ms: i64,
    /// The recorded provenance of the ingestion (who supplied the
    /// document copy — an operator upload, a vendor feed; never silent).
    pub ingestion_source: String,
}

const MAX_MODEL_REF_CHARS: usize = 256;
const MAX_DOCUMENT_REF_CHARS: usize = 256;

impl InterconnectionRecord {
    /// Validates and constructs one record (immutable once built).
    pub fn new(
        device_model: String,
        document_kind: InterconnectionDocumentKind,
        document_ref: String,
        document_version: String,
        issuer: String,
        category: CertifiedFunctionCategory,
        mut certified_functions: Vec<SettingFunctionId>,
        mut ride_through_categories: Vec<RideThroughCategory>,
        settings_profile_ref: String,
        ingested_at_unix_ms: i64,
        ingestion_source: String,
    ) -> Result<Self, InterconnectionRecordError> {
        if device_model.trim().is_empty() {
            return Err(InterconnectionRecordError::EmptyDeviceModel);
        }
        if device_model.len() > MAX_MODEL_REF_CHARS {
            return Err(InterconnectionRecordError::DeviceModelTooLong {
                len: device_model.len(),
                max: MAX_MODEL_REF_CHARS,
            });
        }
        if document_ref.trim().is_empty() {
            return Err(InterconnectionRecordError::EmptyDocumentRef);
        }
        if document_ref.len() > MAX_DOCUMENT_REF_CHARS {
            return Err(InterconnectionRecordError::DocumentRefTooLong {
                len: document_ref.len(),
                max: MAX_DOCUMENT_REF_CHARS,
            });
        }
        if issuer.trim().is_empty() {
            return Err(InterconnectionRecordError::EmptyIssuer);
        }
        if certified_functions.is_empty() {
            return Err(InterconnectionRecordError::EmptyCertifiedFunctions);
        }
        if ingestion_source.trim().is_empty() {
            return Err(InterconnectionRecordError::EmptyIssuer);
        }
        // Canonical order + dedup (deterministic record identity; the
        // BTreeMap-style ordering discipline of the kernel patterns).
        certified_functions.sort();
        certified_functions.dedup();
        ride_through_categories.sort();
        ride_through_categories.dedup();
        Ok(Self {
            device_model,
            document_kind,
            document_ref,
            document_version,
            issuer,
            category,
            certified_functions,
            ride_through_categories,
            settings_profile_ref,
            ingested_at_unix_ms,
            ingestion_source,
        })
    }
}

// --- The document store ------------------------------------------------------------

/// The interconnection document store: a platform-global, per-device-
/// model catalog of immutable records (BTreeMap — deterministic
/// iteration; the P14.13 `SettingsDocumentStore` pattern mirrored).
/// Register-only: no update, no delete (records are immutable — see
/// [`INTERCONNECTION_RECORDS_IMMUTABLE_STATEMENT`]).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct InterconnectionDocumentStore {
    /// (device_model, document_ref) -> the record. The composite key is
    /// the record identity: one document version per device model,
    /// registered once.
    records: BTreeMap<(String, String), InterconnectionRecord>,
}

impl InterconnectionDocumentStore {
    /// An empty store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers one immutable record. A (device model, document
    /// reference) collision is a named refusal (a new document version
    /// is a NEW record, never an overwrite).
    pub fn register(
        &mut self,
        record: InterconnectionRecord,
    ) -> Result<(), InterconnectionRecordError> {
        let key = (record.device_model.clone(), record.document_ref.clone());
        if self.records.contains_key(&key) {
            return Err(InterconnectionRecordError::DuplicateRecord {
                device_model: key.0,
                document_ref: key.1,
            });
        }
        self.records.insert(key, record);
        Ok(())
    }

    /// The EXACT-linkage lookup: every record for the given device-model
    /// string (string equality with the registry's vendor_model; a
    /// near-miss resolves to an empty list and FLAGS — it never
    /// matches).
    pub fn records_for_device_model(
        &self,
        device_model: &str,
    ) -> Vec<&InterconnectionRecord> {
        self.records
            .iter()
            .filter(|((model, _), _)| model == device_model)
            .map(|(_, record)| record)
            .collect()
    }

    /// The settings-profile references linked to a device model (the
    /// exact-linkage view the settings workflow consumes; duplicates
    /// across documents are NOT deduplicated — the caller sees what was
    /// listed, per document).
    pub fn settings_profile_refs_for_device_model(
        &self,
        device_model: &str,
    ) -> Result<Vec<String>, InterconnectionRecordError> {
        let records = self.records_for_device_model(device_model);
        if records.is_empty() {
            return Err(InterconnectionRecordError::NotFound {
                device_model: device_model.to_string(),
            });
        }
        Ok(records
            .iter()
            .map(|record| record.settings_profile_ref.clone())
            .collect())
    }

    /// Every registered record, in deterministic (device model, document
    /// reference) order.
    pub fn all_records(&self) -> Vec<&InterconnectionRecord> {
        self.records.values().collect()
    }

    /// The number of registered records.
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Whether the store is empty.
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}

// --- Tests (WRITTEN-UNRUN — no cargo command was executed in this loop;
// --- the human build/test gate owns the run. Every expectation below is
// --- constructed input, never an observed outcome. Nothing is
// --- fabricated.) -----------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn record(model: &str, doc_ref: &str) -> InterconnectionRecord {
        InterconnectionRecord::new(
            model.to_string(),
            InterconnectionDocumentKind::Ul1741SbCoc,
            doc_ref.to_string(),
            "rev-1".to_string(),
            "test-lab-ref".to_string(),
            CertifiedFunctionCategory::CategoryB,
            vec![SettingFunctionId::VoltVar, SettingFunctionId::VoltWatt],
            vec![RideThroughCategory::CategoryII],
            "settings-profile/ca-rule-21".to_string(),
            1_700_000_000_000,
            "operator-upload".to_string(),
        )
        .expect("valid constructed record")
    }

    #[test]
    fn statements_are_the_recorded_strings() {
        assert!(PLATFORM_LISTS_NEVER_CERTIFIES_STATEMENT.contains("LISTS certified equipment"));
        assert!(PLATFORM_LISTS_NEVER_CERTIFIES_STATEMENT.contains("NEVER claims certification"));
        assert!(PLATFORM_LISTS_NEVER_CERTIFIES_STATEMENT.contains("[S-21][S-169][S-170]"));
        assert!(MODEL_LINKAGE_IS_EXACT_STATEMENT.contains("EXACT string equality"));
        assert!(INTERCONNECTION_RECORDS_IMMUTABLE_STATEMENT.contains("NEW record"));
    }

    #[test]
    fn certified_functions_are_sorted_and_deduplicated() {
        let record = InterconnectionRecord::new(
            "inverter-x/model-y".to_string(),
            InterconnectionDocumentKind::Ieee1547TestReport,
            "doc-1".to_string(),
            "rev-1".to_string(),
            "lab".to_string(),
            CertifiedFunctionCategory::CategoryA,
            vec![
                SettingFunctionId::VoltWatt,
                SettingFunctionId::ConstantPf,
                SettingFunctionId::VoltVar,
                SettingFunctionId::VoltWatt,
            ],
            vec![],
            "profile-ref".to_string(),
            0,
            "operator-upload".to_string(),
        )
        .expect("valid record");
        // Canonical (declaration) order after sort: ConstantPf < VoltVar
        // < VoltWatt (the enum's derived Ord is declaration order).
        assert_eq!(
            record.certified_functions,
            vec![
                SettingFunctionId::ConstantPf,
                SettingFunctionId::VoltVar,
                SettingFunctionId::VoltWatt,
            ]
        );
        assert!(record.ride_through_categories.is_empty());
    }

    #[test]
    fn linkage_is_exact_string_equality() {
        let mut store = InterconnectionDocumentStore::new();
        store.register(record("inverter-x/model-y", "doc-1")).expect("registered");
        // Exact match resolves.
        assert_eq!(store.records_for_device_model("inverter-x/model-y").len(), 1);
        // A near-miss (whitespace, case, prefix) resolves to NOTHING and
        // flags — it never matches.
        assert!(store.records_for_device_model("inverter-x/model-y ").is_empty());
        assert!(store.records_for_device_model("Inverter-X/Model-Y").is_empty());
        assert!(store.records_for_device_model("inverter-x").is_empty());
        // The settings-profile lookup for a near-miss is a named
        // NotFound, never an approximate answer.
        let err = store
            .settings_profile_refs_for_device_model("inverter-x/model-y ")
            .expect_err("near-miss flags");
        assert_eq!(
            err,
            InterconnectionRecordError::NotFound {
                device_model: "inverter-x/model-y ".to_string()
            }
        );
        // Exact linkage returns the profile reference.
        assert_eq!(
            store
                .settings_profile_refs_for_device_model("inverter-x/model-y")
                .expect("exact match"),
            vec!["settings-profile/ca-rule-21".to_string()]
        );
    }

    #[test]
    fn duplicate_identity_is_refused_and_records_are_immutable() {
        let mut store = InterconnectionDocumentStore::new();
        store.register(record("m-1", "doc-1")).expect("registered");
        let err = store
            .register(record("m-1", "doc-1"))
            .expect_err("duplicate identity refused");
        assert_eq!(
            err,
            InterconnectionRecordError::DuplicateRecord {
                device_model: "m-1".to_string(),
                document_ref: "doc-1".to_string(),
            }
        );
        // A NEW document version is a NEW record under the same model.
        store.register(record("m-1", "doc-2")).expect("new document version");
        assert_eq!(store.records_for_device_model("m-1").len(), 2);
        assert_eq!(store.len(), 2);
        assert!(!store.is_empty());
    }

    #[test]
    fn construction_validates_model_refs_issuers_and_function_lists() {
        let err = InterconnectionRecord::new(
            "  ".to_string(),
            InterconnectionDocumentKind::Ul1741SbCoc,
            "doc".to_string(),
            "v".to_string(),
            "issuer".to_string(),
            CertifiedFunctionCategory::NotClassified,
            vec![SettingFunctionId::VoltVar],
            vec![],
            "p".to_string(),
            0,
            "s".to_string(),
        )
        .expect_err("empty model refused");
        assert_eq!(err, InterconnectionRecordError::EmptyDeviceModel);
        let err = InterconnectionRecord::new(
            "m".to_string(),
            InterconnectionDocumentKind::Ul1741SbCoc,
            "  ".to_string(),
            "v".to_string(),
            "issuer".to_string(),
            CertifiedFunctionCategory::NotClassified,
            vec![SettingFunctionId::VoltVar],
            vec![],
            "p".to_string(),
            0,
            "s".to_string(),
        )
        .expect_err("empty document ref refused");
        assert_eq!(err, InterconnectionRecordError::EmptyDocumentRef);
        let err = InterconnectionRecord::new(
            "m".to_string(),
            InterconnectionDocumentKind::Ul1741SbCoc,
            "doc".to_string(),
            "v".to_string(),
            " ".to_string(),
            CertifiedFunctionCategory::NotClassified,
            vec![SettingFunctionId::VoltVar],
            vec![],
            "p".to_string(),
            0,
            "s".to_string(),
        )
        .expect_err("empty issuer refused");
        assert_eq!(err, InterconnectionRecordError::EmptyIssuer);
        let err = InterconnectionRecord::new(
            "m".to_string(),
            InterconnectionDocumentKind::Ul1741SbCoc,
            "doc".to_string(),
            "v".to_string(),
            "issuer".to_string(),
            CertifiedFunctionCategory::NotClassified,
            vec![],
            vec![],
            "p".to_string(),
            0,
            "s".to_string(),
        )
        .expect_err("empty function list refused");
        assert_eq!(err, InterconnectionRecordError::EmptyCertifiedFunctions);
        let long_model = "m".repeat(257);
        let err = InterconnectionRecord::new(
            long_model,
            InterconnectionDocumentKind::Ul1741SbCoc,
            "doc".to_string(),
            "v".to_string(),
            "issuer".to_string(),
            CertifiedFunctionCategory::NotClassified,
            vec![SettingFunctionId::VoltVar],
            vec![],
            "p".to_string(),
            0,
            "s".to_string(),
        )
        .expect_err("over-bound model refused");
        assert!(matches!(
            err,
            InterconnectionRecordError::DeviceModelTooLong { .. }
        ));
    }

    #[test]
    fn the_store_lists_all_records_deterministically() {
        let mut store = InterconnectionDocumentStore::new();
        store.register(record("m-b", "doc-1")).expect("registered");
        store.register(record("m-a", "doc-2")).expect("registered");
        let models: Vec<&str> = store
            .all_records()
            .iter()
            .map(|record| record.device_model.as_str())
            .collect();
        // BTreeMap order: deterministic, model-sorted.
        assert_eq!(models, vec!["m-a", "m-b"]);
    }

    #[test]
    fn every_interconnection_error_renders_non_empty_prose() {
        let errors = [
            InterconnectionRecordError::EmptyDeviceModel,
            InterconnectionRecordError::DeviceModelTooLong { len: 5, max: 4 },
            InterconnectionRecordError::EmptyDocumentRef,
            InterconnectionRecordError::DocumentRefTooLong { len: 5, max: 4 },
            InterconnectionRecordError::EmptyIssuer,
            InterconnectionRecordError::EmptyCertifiedFunctions,
            InterconnectionRecordError::DuplicateRecord {
                device_model: "m".to_string(),
                document_ref: "d".to_string(),
            },
            InterconnectionRecordError::NotFound {
                device_model: "m".to_string(),
            },
        ];
        for error in errors {
            assert!(!error.to_string().is_empty());
        }
    }
}
