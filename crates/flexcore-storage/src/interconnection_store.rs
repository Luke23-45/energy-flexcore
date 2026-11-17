// P15.15 — InterconnectionRecordStore: the persistence contract for the
//         interconnection document records (UL 1741 SB COCs, IEEE
//         1547.1-2020 test reports, certified function categories,
//         settings-profile references, per DEVICE MODEL; migration 0021).
//
// Phase 15 plan §2 row P15.15; decision record:
// `docs/phases-11-20/phase-15-forecasting-asset-intelligence/records/
// 44_p15_15_interconnection_record_ingestion.md`.
//
// The DOMAIN vocabulary (`flexcore_domain::interconnection_records`) owns
// the record type and the in-memory document store; this module is the
// PostgreSQL persistence contract, following the recorded
// `schedule_store` home decision (D-P3.21-4) — a record-store trait here
// in flexcore-storage, with the same relocation path to flexcore-ports if
// a later subtask re-homes it.
//
// ## The tenancy decision (recorded, different from the other stores)
//
// Interconnection records are per DEVICE MODEL — certification is a
// property of the equipment model, not of any tenant's instance of it.
// The store is a PLATFORM-GLOBAL equipment catalog (the same posture as
// the P14.13 jurisdiction document store in the domain): NO tenant_id
// column, NO tenant RLS. Writes are operator/composition-root actions
// (a document ingest), reads are registry-linkage lookups. The migration
// (0021) grants INSERT + SELECT only — the records are immutable:
// a corrected or renewed document is a NEW record, never an overwrite.
//
// ## Adapter obligations (binding — identical to `envelope_store`)
//
// 1. `insert_record` executes inside the CALLER's unit of work where the
//    caller has one (an ingestion use case pairs the record write with
//    its audit/outbox rows in the same transaction; a standalone
//    catalog-ingest path may use the store directly — the atomicity
//    obligation then reduces to "the insert is all-or-nothing", which
//    the adapter's single INSERT satisfies).
// 2. The (device_model, document_ref) identity is UNIQUE at the
//    database level; a collision maps to
//    [`InterconnectionStoreError::Conflict`] — records are immutable.
// 3. Storage failures surface as [`InterconnectionStoreError::Backend`]
//    with an opaque code + message only (never raw SQL or driver text).
// 4. The records are immutable: adapters grant INSERT and SELECT only.
//
// ## Row shape
//
// Typed columns carry the linkage and reference fields a reader filters
// on (device model, document kind, document reference, category); the
// certified-function and ride-through-category lists travel as JSON
// arrays of the P14.13 catalog NAMES (the canonical snake_case spellings)
// — opaque JSON text at this layer (this crate imports no serialization
// library; the adapter validates it parses as JSON on insert and that
// every entry names a catalog function).

/// Errors of the interconnection-record store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InterconnectionStoreError {
    /// The row is incomplete or malformed (`field` names the problem).
    /// The record was NOT written.
    InvalidRecord {
        field: &'static str,
    },
    /// A record with the same (device_model, document_ref) identity
    /// already exists. The record was NOT written — records are
    /// immutable and a new document version is a NEW record.
    Conflict {
        device_model: String,
        document_ref: String,
    },
    /// A JSON payload field did not parse, or a list entry did not name
    /// a P14.13 catalog function/category (the adapter owns the JSON
    /// binding and therefore the validation).
    InvalidPayloadJson {
        field: &'static str,
    },
    /// The storage backend failed. `code` and `message` are opaque and
    /// human-safe — never raw storage text.
    Backend {
        code: String,
        message: String,
    },
}

impl std::fmt::Display for InterconnectionStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InterconnectionStoreError::InvalidRecord { field } => write!(
                f,
                "the interconnection record is invalid: '{field}' is missing or malformed"
            ),
            InterconnectionStoreError::Conflict {
                device_model,
                document_ref,
            } => write!(
                f,
                "an interconnection record for device model {device_model} with document \
                 reference {document_ref} already exists; records are immutable and a new \
                 document version is a NEW record"
            ),
            InterconnectionStoreError::InvalidPayloadJson { field } => write!(
                f,
                "the interconnection record's {field} payload is not valid JSON (or an \
                 entry does not name a P14.13 catalog function/category)"
            ),
            InterconnectionStoreError::Backend { code, message } => {
                write!(f, "storage backend failure ({code}): {message}")
            }
        }
    }
}

impl std::error::Error for InterconnectionStoreError {}

/// One persisted interconnection record (the insert/read row). Every
/// field is validated at construction; the linkage is the EXACT
/// device-model string (the registry's vendor_model — no fuzzy match,
/// the domain statement mirrored).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterconnectionRecordRow {
    /// The record identity (canonical UUID text; caller-supplied UUIDv7
    /// so identifier order is a stable time order — the P3.28 keyset
    /// rationale mirrored).
    pub record_id: String,
    /// The device model the document covers: EXACTLY the registry's
    /// vendor_model string (P0.10 §1 row 4).
    pub device_model: String,
    /// The document kind (`ul_1741_sb_coc` | `ieee_1547_test_report` —
    /// the domain vocabulary's canonical spellings).
    pub document_kind: String,
    /// The document reference (the issuer's document id / URI).
    pub document_ref: String,
    /// The document version as the ISSUER labels it.
    pub document_version: String,
    /// The issuer reference (the certification body / laboratory).
    pub issuer: String,
    /// The abnormal-condition-performance category
    /// (`category_a` | `category_b` | `not_classified`).
    pub category: String,
    /// The P14.13 catalog function names the document's scope covered,
    /// as a JSON array of the canonical snake_case spellings (opaque
    /// text here; adapter-validated).
    pub certified_functions_json: String,
    /// The ride-through category names, as a JSON array of the
    /// canonical spellings (opaque text here; adapter-validated).
    pub ride_through_categories_json: String,
    /// The settings-profile reference (the P14.13 profile repository's
    /// reference for this device model).
    pub settings_profile_ref: String,
    /// When the record was ingested (unix millis; the INGESTION time —
    /// the P0.09 time-separation discipline).
    pub ingested_at_unix_ms: i64,
    /// The recorded provenance of the ingestion (who supplied the
    /// document copy).
    pub ingestion_source: String,
}

impl InterconnectionRecordRow {
    /// Builds and validates one row. Storage-level rules: every text
    /// field non-empty; the kind and category names from the closed
    /// domain vocabularies; a non-negative ingest timestamp; non-empty
    /// JSON payloads (parse + vocabulary validation is the adapter's).
    pub fn new(
        record_id: String,
        device_model: String,
        document_kind: String,
        document_ref: String,
        document_version: String,
        issuer: String,
        category: String,
        certified_functions_json: String,
        ride_through_categories_json: String,
        settings_profile_ref: String,
        ingested_at_unix_ms: i64,
        ingestion_source: String,
    ) -> Result<Self, InterconnectionStoreError> {
        for (field, value) in [
            ("record_id", record_id.as_str()),
            ("device_model", device_model.as_str()),
            ("document_kind", document_kind.as_str()),
            ("document_ref", document_ref.as_str()),
            ("document_version", document_version.as_str()),
            ("issuer", issuer.as_str()),
            ("category", category.as_str()),
            ("certified_functions_json", certified_functions_json.as_str()),
            (
                "ride_through_categories_json",
                ride_through_categories_json.as_str(),
            ),
            ("settings_profile_ref", settings_profile_ref.as_str()),
            ("ingestion_source", ingestion_source.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(InterconnectionStoreError::InvalidRecord { field });
            }
        }
        if document_kind != "ul_1741_sb_coc" && document_kind != "ieee_1547_test_report" {
            return Err(InterconnectionStoreError::InvalidRecord {
                field: "document_kind",
            });
        }
        if category != "category_a"
            && category != "category_b"
            && category != "not_classified"
        {
            return Err(InterconnectionStoreError::InvalidRecord { field: "category" });
        }
        if ingested_at_unix_ms < 0 {
            return Err(InterconnectionStoreError::InvalidRecord {
                field: "ingested_at_unix_ms",
            });
        }
        Ok(Self {
            record_id,
            device_model,
            document_kind,
            document_ref,
            document_version,
            issuer,
            category,
            certified_functions_json,
            ride_through_categories_json,
            settings_profile_ref,
            ingested_at_unix_ms,
            ingestion_source,
        })
    }
}

/// The receipt of one persisted interconnection record (the receipt
/// convention of this crate: its existence proves the insert was ISSUED
/// inside the caller's unit of work).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterconnectionRecordReceipt {
    /// The persisted record's identity (canonical UUID text).
    pub record_id: String,
    /// The device model the record lists (the exact-linkage key).
    pub device_model: String,
}

impl InterconnectionRecordReceipt {
    /// Builds the receipt (adapter-side; both fields are taken as given
    /// from the executed insert).
    pub fn new(record_id: String, device_model: String) -> Self {
        Self {
            record_id,
            device_model,
        }
    }
}

/// The interconnection-record persistence contract. One trait, three
/// operations — an append-oriented catalog store, not a CRUD repository.
///
/// ## Home decision (recorded deviation from the P1.10 single-home rule)
///
/// Same recorded deviation as [`crate::schedule_store::ScheduleRecordStore`]
/// (D-P3.21-4): the trait lives in flexcore-storage; it is shaped to move
/// to flexcore-ports verbatim if a later subtask relocates it.
pub trait InterconnectionRecordStore {
    /// Inserts one interconnection record (immutable; INSERT only). A
    /// (device_model, document_ref) collision is
    /// [`InterconnectionStoreError::Conflict`]; storage failures are
    /// [`InterconnectionStoreError::Backend`] with an opaque code +
    /// message.
    fn insert_interconnection_record(
        &self,
        row: &InterconnectionRecordRow,
    ) -> Result<InterconnectionRecordReceipt, InterconnectionStoreError>;

    /// Every record for ONE device model, in ascending record_id order
    /// (the exact-linkage read; a near-miss model number lists NOTHING —
    /// the domain's exact-equality statement mirrored at the persistence
    /// layer).
    fn records_for_device_model(
        &self,
        device_model: &str,
    ) -> Result<Vec<InterconnectionRecordRow>, InterconnectionStoreError>;
}
