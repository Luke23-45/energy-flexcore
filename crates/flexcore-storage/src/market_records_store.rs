// P16.01 — MarketRecordsStore: the persistence contract for the
//         market-records store (programs, contracts, enrollments, market
//         events, deliveries, settlements, and the market-record retention
//         configuration rows; migration 0022).
//
// Phase 16 plan §2 row P16.01; decision record:
// `docs/phases-11-20/phase-16-market-integration-settlement/records/
// 01_p16_01_market_records_store.md`.
//
// The DOMAIN vocabulary (`flexcore_domain::market_program_model`,
// `flexcore_domain::market_registration_records`) owns the record types
// and lifecycle states; this module is the PostgreSQL persistence
// contract, following the recorded `schedule_store` home decision
// (D-P3.21-4) — a record-store trait here in flexcore-storage, with the
// same relocation path to flexcore-ports if a later subtask re-homes it.
//
// ## What this store is (and is not)
//
// The LR-074/190 gap closes at DESIGN level only: the §16.3
// market-records class was recorded DOES NOT EXIST
// (`ledger/phase-09-pilot-qualification/22_market_operational_procedures.md`
// §5) and this module is the schema + contract that removes the
// hidden-gap status. It is NOT a market connection: no market is
// contacted, no registration is filed, no settlement statement is
// received, and no qualification is claimed anywhere in this program
// (ADR-011; program conventions §7.3). Every table starts EMPTY.
//
// ## The audit-chain linkage (the P11 chain is consumed, never redefined)
//
// Every financial record row carries `audit_record_id uuid NOT NULL` —
// the P11 `audit_records` row (migration 0008; the P11.15 chain columns
// 0019) that the CALLER appends through the P11 audit sink in the SAME
// unit of work as the record insert. The keyed-HMAC chain treatment, the
// NULL-while-unkeyed semantics, and the LR-203 production-crypto
// requirement live on `audit_records` and are consumed; no chain columns
// exist on any market-records table. A market-records insert issued
// WITHOUT its paired audit append violates the write-path obligation
// below — the schema cannot detect it, so the obligation is binding on
// every caller and is restated in the adapter and the decision record.
//
// ## The append-only supersede model
//
// All six record tables are INSERT + SELECT only (the migration grants
// carry exactly that). A lifecycle transition, a contract change, or a
// corrected settlement is a NEW row whose `supersedes_*` link names the
// row it replaces (the D-P4.10-3 supersede discipline mirrored for
// financial records). A PRIMARY-KEY collision on insert maps to
// [`MarketRecordsStoreError::Conflict`] — records are immutable.
//
// ## Money discipline
//
// Settlement amounts travel as explicit DECIMAL TEXT (`amount_text`)
// with the currency as DATA (`currency_code`, three letters; ISO 4217 as
// recorded data, re-verified at use). The row constructor validates the
// decimal-text shape; NO numeric money column, NO float money
// arithmetic, and NO computed total exists anywhere in this contract.
//
// ## Retention rows (P0.16 §4)
//
// The retention configuration table stores per-tenant, per-record-class
// rows that are UNSET wherever the deployment has not decided. A
// CONFIGURED row must carry both the period and a recorded basis. No
// default period exists anywhere in this contract — a missing value can
// never silently become a built-in period (the P1.10 RetentionPolicy
// rule mirrored at rest).
//
// ## Adapter obligations (binding — identical to `interconnection_store`)
//
// 1. Every insert executes inside the CALLER's unit of work, paired
//    with its audit-record append (see above) — a use case pairs the
//    record write with the audit/outbox rows in the same transaction.
// 2. The (record_id, tenant_id) identity is the PRIMARY KEY; a collision
//    maps to [`MarketRecordsStoreError::Conflict`] — records are
//    immutable and a correction is a NEW superseding row.
// 3. Storage failures surface as [`MarketRecordsStoreError::Backend`]
//    with an opaque code + message only (never raw SQL or driver text).
// 4. Payload JSON fields must parse as JSON (the adapter owns the JSON
//    binding and validates vocabulary entry points declared by the
//    domain).
// 5. Tenant scoping: every statement runs under the caller's
//    transaction-scoped `flexcore.tenant_id` (the P1.12 §5.1 RLS
//    conventions); the tenant filters on the read methods are the
//    adapter's explicit WHERE clauses, never an implicit assumption.

/// Errors of the market-records store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarketRecordsStoreError {
    /// The row is incomplete or malformed (`field` names the problem).
    /// The record was NOT written.
    InvalidRecord {
        field: &'static str,
    },
    /// A record with the same (record_id, tenant_id) identity already
    /// exists. The record was NOT written — records are immutable and a
    /// correction is a NEW superseding row.
    Conflict {
        record_id: String,
        tenant_id: String,
    },
    /// A JSON payload field did not parse, or did not carry the shape
    /// the domain declares (the adapter owns the JSON binding and
    /// therefore the validation).
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

impl std::fmt::Display for MarketRecordsStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MarketRecordsStoreError::InvalidRecord { field } => write!(
                f,
                "the market record is invalid: '{field}' is missing or malformed"
            ),
            MarketRecordsStoreError::Conflict {
                record_id,
                tenant_id,
            } => write!(
                f,
                "a market record with identity ({record_id}, {tenant_id}) already exists; \
                 records are immutable and a correction is a NEW superseding row"
            ),
            MarketRecordsStoreError::InvalidPayloadJson { field } => write!(
                f,
                "the market record's {field} payload is not valid JSON (or does not carry \
                 the shape the domain declares)"
            ),
            MarketRecordsStoreError::Backend { code, message } => {
                write!(f, "storage backend failure ({code}): {message}")
            }
        }
    }
}

impl std::error::Error for MarketRecordsStoreError {}

// --- Shared validation helpers ------------------------------------------------

/// Validates the DECIMAL-TEXT shape of a settlement amount: an optional
/// leading minus, digits, an optional fractional part — no exponent, no
/// separators, no plus sign, no whitespace. The value is recorded text;
/// this check makes a non-decimal (or float-injected) amount
/// unrepresentable at the persistence boundary. Empty and bare-sign
/// strings are refused.
/// Public: the adapter re-applies it as read-path defence in depth (the
/// shape contract has one home — here).
pub fn is_decimal_text(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.is_empty() {
        return false;
    }
    let body = if bytes[0] == b'-' { &bytes[1..] } else { bytes };
    if body.is_empty() {
        return false;
    }
    let mut seen_dot = false;
    let mut seen_digit = false;
    for b in body {
        match b {
            b'0'..=b'9' => seen_digit = true,
            b'.' if !seen_dot => seen_dot = true,
            _ => return false,
        }
    }
    seen_digit
}

/// Validates a currency code as DATA: exactly three uppercase ASCII
/// letters. The ISO 4217 meaning is recorded data, re-verified at use —
/// this check is a shape check, not a semantic claim. Public: the
/// adapter re-applies it as read-path defence in depth (the shape
/// contract has one home — here).
pub fn is_currency_code_shape(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 3
        && bytes.iter().all(|b| matches!(b, b'A'..=b'Z'))
}

// --- Program rows ---------------------------------------------------------------

/// The closed program lifecycle vocabulary (the canonical spellings the
/// migration's CHECK accepts).
pub const MARKET_PROGRAM_STATES: [&str; 4] = ["DECLARED", "ACTIVE", "SUSPENDED", "RETIRED"];

/// One persisted market-program record (the insert/read row). Programs
/// are tenant-owned commercial records; the program's OWN baseline-method
/// declaration travels inside `program_config_json` (LR-073: the method
/// is the program's, never FlexCore's).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketProgramRow {
    /// The record identity (canonical UUID text; caller-supplied UUIDv7
    /// so identifier order is a stable time order — the P3.28 keyset
    /// rationale mirrored).
    pub program_id: String,
    /// The owning tenant (canonical UUID text).
    pub tenant_id: String,
    pub program_name: String,
    /// The jurisdiction as DECLARED data (recorded, with the record
    /// pack's source; ADR-011 — jurisdiction-specific records).
    pub jurisdiction: String,
    /// The lifecycle state (a `MARKET_PROGRAM_STATES` spelling).
    pub state: String,
    /// The program's declared rules as JSON (the adapter validates it
    /// parses as a JSON object; LR-073: the baseline-method declaration
    /// is the program's data).
    pub program_config_json: String,
    /// The program row this row replaces (the supersede model; `None`
    /// for a first declaration).
    pub supersedes_program_id: Option<String>,
    /// The P11 audit-records row appended in the same unit of work (the
    /// chain is consumed, never redefined — see the module header).
    pub audit_record_id: String,
    pub recorded_at_unix_ms: i64,
    pub recorded_by: String,
}

impl MarketProgramRow {
    /// Builds and validates one row. Storage-level rules: every text
    /// field non-empty; the state from the closed vocabulary; a
    /// non-negative record timestamp; the config payload a non-empty
    /// JSON object (parse validation is the adapter's; non-emptiness is
    /// checked here).
    pub fn new(
        program_id: String,
        tenant_id: String,
        program_name: String,
        jurisdiction: String,
        state: String,
        program_config_json: String,
        supersedes_program_id: Option<String>,
        audit_record_id: String,
        recorded_at_unix_ms: i64,
        recorded_by: String,
    ) -> Result<Self, MarketRecordsStoreError> {
        for (field, value) in [
            ("program_id", program_id.as_str()),
            ("tenant_id", tenant_id.as_str()),
            ("program_name", program_name.as_str()),
            ("jurisdiction", jurisdiction.as_str()),
            ("program_config_json", program_config_json.as_str()),
            ("audit_record_id", audit_record_id.as_str()),
            ("recorded_by", recorded_by.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(MarketRecordsStoreError::InvalidRecord { field });
            }
        }
        if !MARKET_PROGRAM_STATES.contains(&state.as_str()) {
            return Err(MarketRecordsStoreError::InvalidRecord { field: "state" });
        }
        if recorded_at_unix_ms < 0 {
            return Err(MarketRecordsStoreError::InvalidRecord {
                field: "recorded_at_unix_ms",
            });
        }
        Ok(Self {
            program_id,
            tenant_id,
            program_name,
            jurisdiction,
            state,
            program_config_json,
            supersedes_program_id,
            audit_record_id,
            recorded_at_unix_ms,
            recorded_by,
        })
    }
}

// --- Contract rows ---------------------------------------------------------------

/// The closed contract lifecycle vocabulary.
pub const MARKET_CONTRACT_STATES: [&str; 3] = ["RECORDED", "ACTIVE", "TERMINATED"];

/// One persisted market-contract record. `terms_json` carries the
/// recorded terms data (revenue-share splits enter as recorded data when
/// the P16.13 billing configuration lands); this contract stores what was
/// recorded, never an interpretation of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketContractRow {
    pub contract_id: String,
    pub tenant_id: String,
    pub program_id: String,
    /// The external contract reference (the counterparty's identifier).
    pub contract_ref: String,
    /// The lifecycle state (a `MARKET_CONTRACT_STATES` spelling).
    pub state: String,
    /// The recorded terms as JSON (adapter-validated parse).
    pub terms_json: String,
    pub supersedes_contract_id: Option<String>,
    pub audit_record_id: String,
    pub recorded_at_unix_ms: i64,
    pub recorded_by: String,
}

impl MarketContractRow {
    /// Builds and validates one row (same storage-level rules as
    /// [`MarketProgramRow::new`]).
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        contract_id: String,
        tenant_id: String,
        program_id: String,
        contract_ref: String,
        state: String,
        terms_json: String,
        supersedes_contract_id: Option<String>,
        audit_record_id: String,
        recorded_at_unix_ms: i64,
        recorded_by: String,
    ) -> Result<Self, MarketRecordsStoreError> {
        for (field, value) in [
            ("contract_id", contract_id.as_str()),
            ("tenant_id", tenant_id.as_str()),
            ("program_id", program_id.as_str()),
            ("contract_ref", contract_ref.as_str()),
            ("terms_json", terms_json.as_str()),
            ("audit_record_id", audit_record_id.as_str()),
            ("recorded_by", recorded_by.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(MarketRecordsStoreError::InvalidRecord { field });
            }
        }
        if !MARKET_CONTRACT_STATES.contains(&state.as_str()) {
            return Err(MarketRecordsStoreError::InvalidRecord { field: "state" });
        }
        if recorded_at_unix_ms < 0 {
            return Err(MarketRecordsStoreError::InvalidRecord {
                field: "recorded_at_unix_ms",
            });
        }
        Ok(Self {
            contract_id,
            tenant_id,
            program_id,
            contract_ref,
            state,
            terms_json,
            supersedes_contract_id,
            audit_record_id,
            recorded_at_unix_ms,
            recorded_by,
        })
    }
}

// --- Enrollment rows ---------------------------------------------------------------

/// The closed enrollment lifecycle vocabulary.
pub const MARKET_ENROLLMENT_STATES: [&str; 6] = [
    "REQUESTED",
    "ACCEPTED",
    "PARTICIPATING",
    "SUSPENDED",
    "WITHDRAWN",
    "REFUSED",
];

/// One persisted per-asset enrollment record with its participation
/// window (end-exclusive, §10.3) and its qualification-evidence
/// references. A `None` window means no window is declared yet — never
/// "always" (an undeclared window is never silently widened).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketEnrollmentRow {
    pub enrollment_id: String,
    pub tenant_id: String,
    pub program_id: String,
    pub asset_id: String,
    /// The lifecycle state (a `MARKET_ENROLLMENT_STATES` spelling).
    pub state: String,
    /// The participation-window start (unix millis; `None` = not
    /// declared).
    pub window_start_millis: Option<i64>,
    /// The participation-window end, END-EXCLUSIVE (unix millis; `None`
    /// = not declared). Present exactly when the start is present, and
    /// strictly greater.
    pub window_end_millis: Option<i64>,
    /// The qualification-evidence references as a JSON array (the
    /// adapter validates the parse and array shape).
    pub qualification_evidence_json: String,
    pub supersedes_enrollment_id: Option<String>,
    pub audit_record_id: String,
    pub recorded_at_unix_ms: i64,
    pub recorded_by: String,
}

impl MarketEnrollmentRow {
    /// Builds and validates one row: closed state vocabulary; the window
    /// pair is present together, bounded, non-negative, and never
    /// inverted (end-exclusive); non-negative timestamps.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        enrollment_id: String,
        tenant_id: String,
        program_id: String,
        asset_id: String,
        state: String,
        window_start_millis: Option<i64>,
        window_end_millis: Option<i64>,
        qualification_evidence_json: String,
        supersedes_enrollment_id: Option<String>,
        audit_record_id: String,
        recorded_at_unix_ms: i64,
        recorded_by: String,
    ) -> Result<Self, MarketRecordsStoreError> {
        for (field, value) in [
            ("enrollment_id", enrollment_id.as_str()),
            ("tenant_id", tenant_id.as_str()),
            ("program_id", program_id.as_str()),
            ("asset_id", asset_id.as_str()),
            ("qualification_evidence_json", qualification_evidence_json.as_str()),
            ("audit_record_id", audit_record_id.as_str()),
            ("recorded_by", recorded_by.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(MarketRecordsStoreError::InvalidRecord { field });
            }
        }
        if !MARKET_ENROLLMENT_STATES.contains(&state.as_str()) {
            return Err(MarketRecordsStoreError::InvalidRecord { field: "state" });
        }
        match (window_start_millis, window_end_millis) {
            (None, None) => {}
            (Some(start), Some(end)) => {
                if start < 0 || end <= start {
                    return Err(MarketRecordsStoreError::InvalidRecord {
                        field: "window_end_millis",
                    });
                }
            }
            _ => {
                return Err(MarketRecordsStoreError::InvalidRecord {
                    field: "window_start_millis",
                })
            }
        }
        if recorded_at_unix_ms < 0 {
            return Err(MarketRecordsStoreError::InvalidRecord {
                field: "recorded_at_unix_ms",
            });
        }
        Ok(Self {
            enrollment_id,
            tenant_id,
            program_id,
            asset_id,
            state,
            window_start_millis,
            window_end_millis,
            qualification_evidence_json,
            supersedes_enrollment_id,
            audit_record_id,
            recorded_at_unix_ms,
            recorded_by,
        })
    }
}

// --- Market-event rows ---------------------------------------------------------------

/// One persisted market-event record (a program event window, a received
/// market signal, a declared baseline day — the kind is DECLARED data
/// per the jurisdiction record pack; ADR-011).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketEventRow {
    pub event_id: String,
    pub tenant_id: String,
    pub program_id: String,
    /// The program's own event identifier (or the recording source's
    /// reference for simulated events).
    pub external_ref: String,
    /// The event kind as declared by the record pack (non-empty,
    /// bounded).
    pub event_kind: String,
    /// The event window start (unix millis).
    pub start_millis: i64,
    /// The event window end, END-EXCLUSIVE (unix millis), strictly
    /// greater than the start.
    pub end_millis: i64,
    /// The recorded event payload as JSON (adapter-validated parse).
    pub event_payload_json: String,
    pub audit_record_id: String,
    pub recorded_at_unix_ms: i64,
    pub recorded_by: String,
}

impl MarketEventRow {
    /// Builds and validates one row.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        event_id: String,
        tenant_id: String,
        program_id: String,
        external_ref: String,
        event_kind: String,
        start_millis: i64,
        end_millis: i64,
        event_payload_json: String,
        audit_record_id: String,
        recorded_at_unix_ms: i64,
        recorded_by: String,
    ) -> Result<Self, MarketRecordsStoreError> {
        for (field, value) in [
            ("event_id", event_id.as_str()),
            ("tenant_id", tenant_id.as_str()),
            ("program_id", program_id.as_str()),
            ("external_ref", external_ref.as_str()),
            ("event_kind", event_kind.as_str()),
            ("event_payload_json", event_payload_json.as_str()),
            ("audit_record_id", audit_record_id.as_str()),
            ("recorded_by", recorded_by.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(MarketRecordsStoreError::InvalidRecord { field });
            }
        }
        if start_millis < 0 || end_millis <= start_millis {
            return Err(MarketRecordsStoreError::InvalidRecord { field: "end_millis" });
        }
        if recorded_at_unix_ms < 0 {
            return Err(MarketRecordsStoreError::InvalidRecord {
                field: "recorded_at_unix_ms",
            });
        }
        Ok(Self {
            event_id,
            tenant_id,
            program_id,
            external_ref,
            event_kind,
            start_millis,
            end_millis,
            event_payload_json,
            audit_record_id,
            recorded_at_unix_ms,
            recorded_by,
        })
    }
}

// --- Delivery rows ---------------------------------------------------------------

/// The closed §10.4 quality vocabulary (the canonical spellings the
/// migration's CHECK accepts; `flexcore_contracts::quality` is the
/// single source of truth — these are its names, never a restatement).
pub const MARKET_DELIVERY_QUALITY_STATES: [&str; 6] = [
    "GOOD",
    "SUSPECT",
    "BAD",
    "MISSING",
    "ESTIMATED",
    "STALE",
];

/// One persisted per-interval delivery measurement. The value is a
/// MEASUREMENT (MWh, P > 0 = export at the point of connection) recorded
/// AS MEASURED — including negative values where the measurement says
/// so. The §10.4 quality state travels WITH the number; a quality flag
/// is never silently converted.
#[derive(Debug, Clone, PartialEq)]
pub struct MarketDeliveryRow {
    pub delivery_id: String,
    pub tenant_id: String,
    pub program_id: String,
    pub event_id: String,
    pub asset_id: String,
    pub interval_start_millis: i64,
    /// The interval end, END-EXCLUSIVE, strictly greater than the start.
    pub interval_end_millis: i64,
    /// The measured delivery magnitude in MWh (finite; no NaN/infinity).
    pub delivered_energy_mwh: f64,
    /// The §10.4 quality state of the value (a
    /// `MARKET_DELIVERY_QUALITY_STATES` spelling).
    pub measured_quality: String,
    pub audit_record_id: String,
    pub recorded_at_unix_ms: i64,
    pub recorded_by: String,
}

impl MarketDeliveryRow {
    /// Builds and validates one row: finite measurement, closed quality
    /// vocabulary, bounded end-exclusive interval, non-negative
    /// timestamps.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        delivery_id: String,
        tenant_id: String,
        program_id: String,
        event_id: String,
        asset_id: String,
        interval_start_millis: i64,
        interval_end_millis: i64,
        delivered_energy_mwh: f64,
        measured_quality: String,
        audit_record_id: String,
        recorded_at_unix_ms: i64,
        recorded_by: String,
    ) -> Result<Self, MarketRecordsStoreError> {
        for (field, value) in [
            ("delivery_id", delivery_id.as_str()),
            ("tenant_id", tenant_id.as_str()),
            ("program_id", program_id.as_str()),
            ("event_id", event_id.as_str()),
            ("asset_id", asset_id.as_str()),
            ("measured_quality", measured_quality.as_str()),
            ("audit_record_id", audit_record_id.as_str()),
            ("recorded_by", recorded_by.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(MarketRecordsStoreError::InvalidRecord { field });
            }
        }
        if interval_start_millis < 0 || interval_end_millis <= interval_start_millis {
            return Err(MarketRecordsStoreError::InvalidRecord {
                field: "interval_end_millis",
            });
        }
        if !delivered_energy_mwh.is_finite() {
            return Err(MarketRecordsStoreError::InvalidRecord {
                field: "delivered_energy_mwh",
            });
        }
        if !MARKET_DELIVERY_QUALITY_STATES.contains(&measured_quality.as_str()) {
            return Err(MarketRecordsStoreError::InvalidRecord {
                field: "measured_quality",
            });
        }
        if recorded_at_unix_ms < 0 {
            return Err(MarketRecordsStoreError::InvalidRecord {
                field: "recorded_at_unix_ms",
            });
        }
        Ok(Self {
            delivery_id,
            tenant_id,
            program_id,
            event_id,
            asset_id,
            interval_start_millis,
            interval_end_millis,
            delivered_energy_mwh,
            measured_quality,
            audit_record_id,
            recorded_at_unix_ms,
            recorded_by,
        })
    }
}

// --- Settlement rows ---------------------------------------------------------------

/// The closed statement-format vocabulary (the formats the plan names;
/// additions are additive migration decisions, never silent).
pub const MARKET_SETTLEMENT_SOURCE_FORMATS: [&str; 4] = [
    "PJM_MANUAL_29",
    "CAISO_SQMD",
    "EBGL_ISP",
    "PROGRAM_DECLARED",
];

/// One persisted settlement-statement record — the P16.09 ingestion
/// interface (Agent B's consumer): a statement arrives, is transcribed
/// AS STATED into decimal text, and every line item is recorded with its
/// own kind. NO total, NO revenue figure, NO payment is computed here —
/// reconciliation is P16.09/P16.12 work, and no settlement number is
/// claimed anywhere in this program (plan §8 criterion 4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketSettlementRow {
    pub settlement_id: String,
    pub tenant_id: String,
    pub program_id: String,
    /// The statement's own identifier (recorded data).
    pub statement_ref: String,
    /// The statement format (a `MARKET_SETTLEMENT_SOURCE_FORMATS`
    /// spelling).
    pub source_format: String,
    pub period_start_millis: i64,
    /// The statement period end, END-EXCLUSIVE, strictly greater than
    /// the start.
    pub period_end_millis: i64,
    /// The statement's own line-item kind (an MSRS/BLI name, an SQMD
    /// charge type, an ISP component — recorded data, never mapped into
    /// a universal taxonomy).
    pub line_item_kind: String,
    /// The amount as explicit DECIMAL TEXT exactly as transcribed (the
    /// decimal-text shape is validated; no float conversion exists).
    pub amount_text: String,
    /// The currency code as DATA (three uppercase letters; ISO 4217 as
    /// recorded data, re-verified at use).
    pub currency_code: String,
    /// The recorded statement payload as JSON (adapter-validated parse).
    pub statement_payload_json: String,
    pub supersedes_settlement_id: Option<String>,
    pub audit_record_id: String,
    pub recorded_at_unix_ms: i64,
    pub recorded_by: String,
}

impl MarketSettlementRow {
    /// Builds and validates one row: closed format vocabulary, the
    /// decimal-text amount shape, the three-letter currency shape, the
    /// bounded end-exclusive period.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        settlement_id: String,
        tenant_id: String,
        program_id: String,
        statement_ref: String,
        source_format: String,
        period_start_millis: i64,
        period_end_millis: i64,
        line_item_kind: String,
        amount_text: String,
        currency_code: String,
        statement_payload_json: String,
        supersedes_settlement_id: Option<String>,
        audit_record_id: String,
        recorded_at_unix_ms: i64,
        recorded_by: String,
    ) -> Result<Self, MarketRecordsStoreError> {
        for (field, value) in [
            ("settlement_id", settlement_id.as_str()),
            ("tenant_id", tenant_id.as_str()),
            ("program_id", program_id.as_str()),
            ("statement_ref", statement_ref.as_str()),
            ("line_item_kind", line_item_kind.as_str()),
            ("amount_text", amount_text.as_str()),
            ("currency_code", currency_code.as_str()),
            ("statement_payload_json", statement_payload_json.as_str()),
            ("audit_record_id", audit_record_id.as_str()),
            ("recorded_by", recorded_by.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(MarketRecordsStoreError::InvalidRecord { field });
            }
        }
        if !MARKET_SETTLEMENT_SOURCE_FORMATS.contains(&source_format.as_str()) {
            return Err(MarketRecordsStoreError::InvalidRecord { field: "source_format" });
        }
        if period_start_millis < 0 || period_end_millis <= period_start_millis {
            return Err(MarketRecordsStoreError::InvalidRecord {
                field: "period_end_millis",
            });
        }
        if !is_decimal_text(&amount_text) {
            return Err(MarketRecordsStoreError::InvalidRecord { field: "amount_text" });
        }
        if !is_currency_code_shape(&currency_code) {
            return Err(MarketRecordsStoreError::InvalidRecord {
                field: "currency_code",
            });
        }
        if recorded_at_unix_ms < 0 {
            return Err(MarketRecordsStoreError::InvalidRecord {
                field: "recorded_at_unix_ms",
            });
        }
        Ok(Self {
            settlement_id,
            tenant_id,
            program_id,
            statement_ref,
            source_format,
            period_start_millis,
            period_end_millis,
            line_item_kind,
            amount_text,
            currency_code,
            statement_payload_json,
            supersedes_settlement_id,
            audit_record_id,
            recorded_at_unix_ms,
            recorded_by,
        })
    }
}

// --- Retention configuration rows ---------------------------------------------------

/// The closed record-class vocabulary of the retention configuration.
pub const MARKET_RECORD_CLASSES: [&str; 8] = [
    "PROGRAM",
    "CONTRACT",
    "ENROLLMENT",
    "MARKET_EVENT",
    "DELIVERY",
    "SETTLEMENT",
    "MV_REPORT",
    "REGISTRATION",
];

/// The closed retention-state vocabulary (the P1.10
/// `RetentionSetting::Unset` semantics mirrored at rest).
pub const MARKET_RETENTION_STATES: [&str; 2] = ["UNSET", "CONFIGURED"];

/// One retention-configuration row (P0.16 §4): per tenant, per record
/// class. UNSET carries NO period and NO basis — a missing value can
/// never silently become a built-in period. CONFIGURED carries BOTH — a
/// period without a recorded basis is not a configurable retention row.
/// Enforcement (partition drops, deletion jobs) is infrastructure work
/// this contract does not contain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketRetentionConfigRow {
    pub tenant_id: String,
    /// The record class (a `MARKET_RECORD_CLASSES` spelling).
    pub record_class: String,
    /// The retention state (a `MARKET_RETENTION_STATES` spelling).
    pub retention_state: String,
    /// The configured period in days (`Some` exactly when CONFIGURED;
    /// positive).
    pub period_days: Option<i32>,
    /// The recorded legal/contractual basis (`Some` exactly when
    /// CONFIGURED; non-empty).
    pub basis: Option<String>,
    pub set_by: String,
    pub set_at_unix_ms: i64,
}

impl MarketRetentionConfigRow {
    /// Builds and validates one row: the UNSET/CONFIGURED shape rule is
    /// enforced here exactly as the migration's CHECK enforces it at
    /// rest.
    pub fn new(
        tenant_id: String,
        record_class: String,
        retention_state: String,
        period_days: Option<i32>,
        basis: Option<String>,
        set_by: String,
        set_at_unix_ms: i64,
    ) -> Result<Self, MarketRecordsStoreError> {
        for (field, value) in [
            ("tenant_id", tenant_id.as_str()),
            ("record_class", record_class.as_str()),
            ("set_by", set_by.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(MarketRecordsStoreError::InvalidRecord { field });
            }
        }
        if !MARKET_RECORD_CLASSES.contains(&record_class.as_str()) {
            return Err(MarketRecordsStoreError::InvalidRecord { field: "record_class" });
        }
        if !MARKET_RETENTION_STATES.contains(&retention_state.as_str()) {
            return Err(MarketRecordsStoreError::InvalidRecord {
                field: "retention_state",
            });
        }
        let configured = retention_state == "CONFIGURED";
        match (configured, period_days, basis.as_deref()) {
            (false, None, None) => {}
            (true, Some(days), Some(basis_text))
                if days > 0 && !basis_text.trim().is_empty() => {}
            _ => {
                return Err(MarketRecordsStoreError::InvalidRecord {
                    field: "period_days",
                })
            }
        }
        if set_at_unix_ms < 0 {
            return Err(MarketRecordsStoreError::InvalidRecord {
                field: "set_at_unix_ms",
            });
        }
        Ok(Self {
            tenant_id,
            record_class,
            retention_state,
            period_days,
            basis,
            set_by,
            set_at_unix_ms,
        })
    }
}

// --- Receipt ---------------------------------------------------------------------

/// The receipt of one persisted market record (the receipt convention of
/// this crate: its existence proves the insert was ISSUED inside the
/// caller's unit of work).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketRecordReceipt {
    /// The persisted record's identity (canonical UUID text).
    pub record_id: String,
    /// The owning tenant (canonical UUID text).
    pub tenant_id: String,
    /// The record class that was written (a `MARKET_RECORD_CLASSES`
    /// spelling for the six record tables' classes, `RETENTION_CONFIG`
    /// for the configuration upsert).
    pub record_class: String,
}

impl MarketRecordReceipt {
    /// Builds the receipt (adapter-side; all fields are taken as given
    /// from the executed insert).
    pub fn new(record_id: String, tenant_id: String, record_class: String) -> Self {
        Self {
            record_id,
            tenant_id,
            record_class,
        }
    }
}

/// The market-records persistence contract. One trait per record class
/// pair (insert + tenant-scoped read), plus the retention-configuration
/// upsert and read. An append-oriented financial-record store, not a
/// CRUD repository: no update path exists on any record class (the
/// supersede model is a NEW row), and the only mutable surface is the
/// retention configuration.
///
/// ## Home decision (recorded deviation from the P1.10 single-home rule)
///
/// Same recorded deviation as [`crate::schedule_store::ScheduleRecordStore`]
/// (D-P3.21-4): the trait lives in flexcore-storage; it is shaped to move
/// to flexcore-ports verbatim if a later subtask relocates it.
///
/// ## Write-path obligations (binding on every caller)
///
/// 1. Every insert runs inside the caller's unit of work, PAIRED with
///    its P11 audit-record append (the `audit_record_id` the row
///    carries names that appended record). The P11 chain is consumed,
///    never redefined.
/// 2. Reads are tenant-scoped: the adapter filters explicitly by the
///    tenant of the call AND runs under the transaction-scoped
///    `flexcore.tenant_id` (the P1.12 §5.1 RLS conventions).
/// 3. Reads are PAGE-BOUNDED (FRV1-R5-009, final review): every read
///    method takes a MANDATORY `limit` and the adapter ALWAYS applies it
///    — there is NO unbounded query (the same rule
///    `command_store`/`envelope_store`/`audit_read` state and enforce).
///    Breaking the trait's earlier unbounded signatures was free while
///    nothing consumed the trait (the store is unwired by design, see
///    the composition-root record); the first real consumer copies this
///    shape, not an unbounded one.
pub trait MarketRecordsStore {
    /// Inserts one market-program record (immutable; INSERT only). A
    /// (program_id, tenant_id) collision is
    /// [`MarketRecordsStoreError::Conflict`].
    fn insert_market_program(
        &self,
        row: &MarketProgramRow,
    ) -> Result<MarketRecordReceipt, MarketRecordsStoreError>;

    /// Up to `limit` program records for ONE tenant, ascending
    /// program_id order (page-bounded — no unbounded query, FRV1-R5-009).
    fn market_program_rows_for_tenant(
        &self,
        tenant_id: &str,
        limit: u32,
    ) -> Result<Vec<MarketProgramRow>, MarketRecordsStoreError>;

    /// Inserts one market-contract record.
    fn insert_market_contract(
        &self,
        row: &MarketContractRow,
    ) -> Result<MarketRecordReceipt, MarketRecordsStoreError>;

    /// Up to `limit` contract records for ONE tenant's program, ascending
    /// contract_id order (page-bounded — no unbounded query, FRV1-R5-009).
    fn market_contract_rows_for_program(
        &self,
        tenant_id: &str,
        program_id: &str,
        limit: u32,
    ) -> Result<Vec<MarketContractRow>, MarketRecordsStoreError>;

    /// Inserts one per-asset enrollment record.
    fn insert_market_enrollment(
        &self,
        row: &MarketEnrollmentRow,
    ) -> Result<MarketRecordReceipt, MarketRecordsStoreError>;

    /// Up to `limit` enrollment records for ONE tenant's program,
    /// ascending enrollment_id order (page-bounded — no unbounded query,
    /// FRV1-R5-009).
    fn market_enrollment_rows_for_program(
        &self,
        tenant_id: &str,
        program_id: &str,
        limit: u32,
    ) -> Result<Vec<MarketEnrollmentRow>, MarketRecordsStoreError>;

    /// Inserts one market-event record.
    fn insert_market_event(
        &self,
        row: &MarketEventRow,
    ) -> Result<MarketRecordReceipt, MarketRecordsStoreError>;

    /// Up to `limit` event records for ONE tenant's program, ascending
    /// event_id order (page-bounded — no unbounded query, FRV1-R5-009).
    fn market_event_rows_for_program(
        &self,
        tenant_id: &str,
        program_id: &str,
        limit: u32,
    ) -> Result<Vec<MarketEventRow>, MarketRecordsStoreError>;

    /// Inserts one per-interval delivery record.
    fn insert_market_delivery(
        &self,
        row: &MarketDeliveryRow,
    ) -> Result<MarketRecordReceipt, MarketRecordsStoreError>;

    /// Up to `limit` delivery records for ONE tenant's event, ascending
    /// (asset_id, interval_start_millis) order — the M&V evidence-assembly
    /// read (P16.05), page-bounded (no unbounded query, FRV1-R5-009; the
    /// per-event interval-wise delivery volume is exactly the read the
    /// finding flagged as potentially large). A full M&V assembly walks
    /// the pages with a keyset cursor on the caller side if the caller
    /// needs every interval.
    fn market_delivery_rows_for_event(
        &self,
        tenant_id: &str,
        event_id: &str,
        limit: u32,
    ) -> Result<Vec<MarketDeliveryRow>, MarketRecordsStoreError>;

    /// Inserts one settlement-statement record (the P16.09 ingestion
    /// write; Agent B consumes this interface BY CITATION, never by
    /// redefinition).
    fn insert_market_settlement(
        &self,
        row: &MarketSettlementRow,
    ) -> Result<MarketRecordReceipt, MarketRecordsStoreError>;

    /// Up to `limit` settlement records for ONE tenant's program,
    /// ascending settlement_id order (page-bounded — no unbounded query,
    /// FRV1-R5-009).
    fn market_settlement_rows_for_program(
        &self,
        tenant_id: &str,
        program_id: &str,
        limit: u32,
    ) -> Result<Vec<MarketSettlementRow>, MarketRecordsStoreError>;

    /// Upserts one retention-configuration row (the ONLY mutable
    /// surface: the (tenant_id, record_class) PK is upserted; the
    /// UNSET/CONFIGURED shape rule is enforced by the row constructor
    /// and the migration's CHECK).
    fn upsert_market_retention_config(
        &self,
        row: &MarketRetentionConfigRow,
    ) -> Result<MarketRecordReceipt, MarketRecordsStoreError>;

    /// Up to `limit` retention-configuration rows for ONE tenant,
    /// ascending record_class order (page-bounded — no unbounded query,
    /// FRV1-R5-009).
    fn market_retention_config_rows_for_tenant(
        &self,
        tenant_id: &str,
        limit: u32,
    ) -> Result<Vec<MarketRetentionConfigRow>, MarketRecordsStoreError>;
}
