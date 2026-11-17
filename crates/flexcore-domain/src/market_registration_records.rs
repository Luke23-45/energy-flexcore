// P16.03 — Registration data management: per-ISO registration records —
//         PJM per-DER + aggregation registration with EDC 45-day review
//         tracking [S-90]; ERCOT PUCT+ERCOT dual registration [S-100];
//         ISO-NE CAMS/eMarket asset lifecycle [S-103]; attestations
//         modeled as signed records.
//
// Phase 16 plan §2 row P16.03; decision record:
// `docs/phases-11-20/phase-16-market-integration-settlement/records/
// 03_p16_03_registration_data_management.md`.
//
// THE BOUNDARY, DECIDED ONCE:
//
// - **No registration exists and none is filed.** Every type here is a
//   RECORD of what a registration process WOULD track, written before
//   any jurisdiction is named (D1, OPEN) and before any counterparty
//   exists. No registration is filed, transmitted, or claimed anywhere
//   in this program (ADR-011; conventions §7.3).
// - **Jurisdiction-specific records, never a generic registration**
//   (ADR-011). The three record types below are shaped by the
//   jurisdiction materials cited (PJM [S-90], ERCOT [S-100][S-102],
//   ISO-NE [S-103]); a fourth jurisdiction enters as a NEW record type
//   in a record pack, never by widening these into a universal form.
// - **External dates and windows are SOURCED DATA with the
//   re-verify-at-use rule** (plan risk R-P16-1; the Phase 9 §7 capture
//   rule inherited). The PJM EDC 45-day review window is recorded as the
//   constant `PJM_EDC_REVIEW_WINDOW_DAYS` citing [S-90]; it is
//   re-verified against the primary source on the day of use, never
//   treated as a stable fact.
// - **Attestations are signed records — recorded as data, never
//   validated as claims.** An attestation record stores WHO signed,
//   WHAT statement, the algorithm LABEL, and the signature DIGEST. It
//   records that a signature exists; it does NOT verify the signature,
//   and it is never a qualification or conformance claim (conventions
//   §7.3). Production-grade crypto in signing surfaces is a DEPLOYMENT
//   REQUIREMENT (conventions §6.6; LR-203 inherited) — a record with a
//   test-grade label is never a release signature.
// - **The P11 audit chain is consumed, never redefined**: every
//   registration record carries its audit-chain LINK
//   (`flexcore_domain::market_program_model::AuditChainLink` — one
//   home, reused).
//
// Control status: records for the simulation-only 0.x line. No I/O, no
// filing path, no transmission path.

use std::collections::BTreeMap;

use crate::market_program_model::AuditChainLink;

/// The registration-record schema version (a change is a NEW version,
/// never a silent edit).
pub const MARKET_REGISTRATION_SCHEMA_VERSION: u32 = 1;

/// The no-filing statement (pinned by tests).
pub const NO_REGISTRATION_FILED_STATEMENT: &str = "No registration is filed, transmitted, \
     or claimed anywhere in this program (ADR-011; program conventions §7.3). These are \
     record types for what a registration process tracks, written before any jurisdiction \
     is named (D1, OPEN) and before any counterparty exists.";

/// The sourced-window statement (pinned by tests).
pub const SOURCED_WINDOW_STATEMENT: &str = "External windows and dates are SOURCED DATA \
     with the re-verify-at-use rule (plan risk R-P16-1): the PJM EDC 45-day review window \
     cites [S-90] and is re-verified against the primary source on the day of use — never \
     treated as a stable fact.";

/// The attestation statement (pinned by tests).
pub const ATTESTATION_IS_RECORD_STATEMENT: &str = "An attestation record stores WHO signed, \
     WHAT statement, the algorithm LABEL, and the signature DIGEST. It records that a \
     signature exists; it does NOT verify the signature and is NEVER a qualification or \
     conformance claim. Production-grade crypto is a deployment requirement (LR-203 \
     inherited); a test-grade label is never a release signature.";

/// The PJM EDC review window in days, cited from [S-90] (the PJM FERC
/// 2222 participation model overview). SOURCED DATA with the
/// re-verify-at-use rule — see `SOURCED_WINDOW_STATEMENT`.
pub const PJM_EDC_REVIEW_WINDOW_DAYS: i64 = 45;

// --- Errors -------------------------------------------------------------------

/// Why a registration-record operation was refused. Named, never
/// absorbed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarketRegistrationError {
    /// A required text field was empty (`field` names it).
    EmptyField {
        field: &'static str,
    },
    /// A text field exceeded its bound (`field` names it).
    FieldTooLong {
        field: &'static str,
        len: usize,
        max: usize,
    },
    /// The registration identity already exists as a current record (a
    /// status change is a NEW superseding record, never an overwrite).
    DuplicateRegistration {
        registration_id: String,
    },
    /// The supersede target was not found.
    RecordNotFound {
        registration_id: String,
    },
    /// The supersede target is already superseded (one current version).
    RecordSuperseded {
        registration_id: String,
    },
    /// The supersede target belongs to a different jurisdiction or
    /// tenant than the new record.
    SupersedeScopeMismatch,
    /// A review update advanced a review state along an edge outside
    /// the closed review vocabulary (`from`/`to` name the states).
    IllegalReviewTransition {
        from: String,
        to: String,
    },
    /// An attestation record carried a test-grade algorithm label in a
    /// position the deployment requires to be production-grade (the
    /// LR-203 requirement carried; the check is a LABEL check, never a
    /// cryptographic claim).
    TestGradeSignatureLabel {
        label: String,
    },
}

impl std::fmt::Display for MarketRegistrationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MarketRegistrationError::EmptyField { field } => {
                write!(f, "the registration field '{field}' must be non-empty")
            }
            MarketRegistrationError::FieldTooLong { field, len, max } => write!(
                f,
                "the registration field '{field}' is {len} characters; the bound is {max}"
            ),
            MarketRegistrationError::DuplicateRegistration { registration_id } => write!(
                f,
                "the registration {registration_id} already exists as a current record; a \
                 status change is a NEW superseding record, never an overwrite"
            ),
            MarketRegistrationError::RecordNotFound { registration_id } => {
                write!(f, "the registration {registration_id} was not found")
            }
            MarketRegistrationError::RecordSuperseded { registration_id } => write!(
                f,
                "the registration {registration_id} is already superseded; there is exactly \
                 one current version"
            ),
            MarketRegistrationError::SupersedeScopeMismatch => write!(
                f,
                "the supersede target belongs to a different jurisdiction or tenant than the \
                 new record"
            ),
            MarketRegistrationError::IllegalReviewTransition { from, to } => write!(
                f,
                "the EDC-review transition {from} → {to} is not in the closed review \
                 vocabulary"
            ),
            MarketRegistrationError::TestGradeSignatureLabel { label } => write!(
                f,
                "the signature algorithm label '{label}' is a test-grade label; production- \
                 grade crypto is a deployment requirement (LR-203 inherited) and a test-grade \
                 label is never a release signature"
            ),
        }
    }
}

impl std::error::Error for MarketRegistrationError {}

// --- Shared helpers ---------------------------------------------------------------

fn validate_text(
    field: &'static str,
    value: &str,
    max: usize,
) -> Result<(), MarketRegistrationError> {
    if value.trim().is_empty() {
        return Err(MarketRegistrationError::EmptyField { field });
    }
    if value.len() > max {
        return Err(MarketRegistrationError::FieldTooLong {
            field,
            len: value.len(),
            max,
        });
    }
    Ok(())
}

// --- Jurisdiction vocabulary -------------------------------------------------------

/// The closed jurisdiction vocabulary at design level: the four
/// jurisdictions the phase plan names. A fifth enters as a NEW variant in
/// a jurisdiction record pack (ADR-011 — additive, never a widened
/// generic).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum MarketJurisdiction {
    Pjm,
    Ercot,
    IsoNe,
    Caiso,
}

impl MarketJurisdiction {
    /// The canonical spelling.
    pub fn name(&self) -> &'static str {
        match self {
            MarketJurisdiction::Pjm => "PJM",
            MarketJurisdiction::Ercot => "ERCOT",
            MarketJurisdiction::IsoNe => "ISO_NE",
            MarketJurisdiction::Caiso => "CAISO",
        }
    }

    /// Parses the canonical spelling (fail-closed; no normalization).
    pub fn try_parse(name: &str) -> Option<Self> {
        match name {
            "PJM" => Some(MarketJurisdiction::Pjm),
            "ERCOT" => Some(MarketJurisdiction::Ercot),
            "ISO_NE" => Some(MarketJurisdiction::IsoNe),
            "CAISO" => Some(MarketJurisdiction::Caiso),
            _ => None,
        }
    }
}

// --- PJM: per-DER + aggregation registration with EDC review tracking [S-90] -------

/// The closed EDC-review state vocabulary (the PJM per-DER registration
/// passes the EDC's review; the window is the recorded
/// `PJM_EDC_REVIEW_WINDOW_DAYS` [S-90]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EdcReviewState {
    NotStarted,
    AwaitingEdc,
    ReviewInProgress,
    AcceptedRecorded,
    RejectedRecorded,
}

impl EdcReviewState {
    pub fn name(&self) -> &'static str {
        match self {
            EdcReviewState::NotStarted => "NOT_STARTED",
            EdcReviewState::AwaitingEdc => "AWAITING_EDC",
            EdcReviewState::ReviewInProgress => "REVIEW_IN_PROGRESS",
            EdcReviewState::AcceptedRecorded => "ACCEPTED_RECORDED",
            EdcReviewState::RejectedRecorded => "REJECTED_RECORDED",
        }
    }

    pub fn try_parse(name: &str) -> Option<Self> {
        match name {
            "NOT_STARTED" => Some(EdcReviewState::NotStarted),
            "AWAITING_EDC" => Some(EdcReviewState::AwaitingEdc),
            "REVIEW_IN_PROGRESS" => Some(EdcReviewState::ReviewInProgress),
            "ACCEPTED_RECORDED" => Some(EdcReviewState::AcceptedRecorded),
            "REJECTED_RECORDED" => Some(EdcReviewState::RejectedRecorded),
            _ => None,
        }
    }

    /// The closed review-transition table (append-only updates walk
    /// exactly these edges). ACCEPTED_RECORDED and REJECTED_RECORDED are
    /// terminal.
    pub fn can_transition_to(self, to: EdcReviewState) -> bool {
        matches!(
            (self, to),
            (EdcReviewState::NotStarted, EdcReviewState::AwaitingEdc)
                | (EdcReviewState::AwaitingEdc, EdcReviewState::ReviewInProgress)
                | (EdcReviewState::ReviewInProgress, EdcReviewState::AcceptedRecorded)
                | (EdcReviewState::ReviewInProgress, EdcReviewState::RejectedRecorded)
        )
    }

    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            EdcReviewState::AcceptedRecorded | EdcReviewState::RejectedRecorded
        )
    }
}

/// One PJM per-DER entry inside an aggregation registration (the PJM
/// participation model registers DERs per-DER under an aggregation
/// [S-90]; the EDC is the distribution utility whose review the DER
/// passes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PjmDerRegistration {
    /// The DER reference (the platform asset id or the registry
    /// identifier the record pack declares — exact reference, recorded).
    der_ref: String,
    /// The EDC name as recorded (the distribution utility reviewing the
    /// DER).
    edc_name: String,
    /// The recorded review state as of `recorded_at_unix_ms` (tracking
    /// UPDATES are separate append-only `EdcReviewUpdate` entries; this
    /// field carries the state at registration time).
    initial_review_state: EdcReviewState,
}

impl PjmDerRegistration {
    pub fn new(
        der_ref: String,
        edc_name: String,
        initial_review_state: EdcReviewState,
    ) -> Result<Self, MarketRegistrationError> {
        validate_text("der_ref", &der_ref, 128)?;
        validate_text("edc_name", &edc_name, 128)?;
        Ok(Self {
            der_ref,
            edc_name,
            initial_review_state,
        })
    }

    pub fn der_ref(&self) -> &str {
        &self.der_ref
    }

    pub fn edc_name(&self) -> &str {
        &self.edc_name
    }

    pub fn initial_review_state(&self) -> EdcReviewState {
        self.initial_review_state
    }
}

/// One PJM aggregation registration record (per-DER registrations under
/// an aggregation [S-90]). Immutable; a status change is a NEW record
/// superseding this one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PjmAggregationRegistration {
    registration_id: String,
    tenant_id: String,
    /// The aggregation reference as recorded (the aggregation resource
    /// identifier the record pack declares).
    aggregation_ref: String,
    /// The per-DER registrations (at least one — an aggregation of zero
    /// DERs is not registrable).
    der_registrations: Vec<PjmDerRegistration>,
    supersedes_registration_id: Option<String>,
    audit_chain_link: AuditChainLink,
    recorded_at_unix_ms: i64,
    recorded_by: String,
}

impl PjmAggregationRegistration {
    pub fn new(
        registration_id: String,
        tenant_id: String,
        aggregation_ref: String,
        der_registrations: Vec<PjmDerRegistration>,
        supersedes_registration_id: Option<String>,
        audit_chain_link: AuditChainLink,
        recorded_at_unix_ms: i64,
        recorded_by: String,
    ) -> Result<Self, MarketRegistrationError> {
        validate_text("registration_id", &registration_id, 64)?;
        validate_text("tenant_id", &tenant_id, 64)?;
        validate_text("aggregation_ref", &aggregation_ref, 256)?;
        validate_text("recorded_by", &recorded_by, 128)?;
        if der_registrations.is_empty() {
            return Err(MarketRegistrationError::EmptyField {
                field: "der_registrations",
            });
        }
        if recorded_at_unix_ms < 0 {
            return Err(MarketRegistrationError::EmptyField {
                field: "recorded_at_unix_ms",
            });
        }
        Ok(Self {
            registration_id,
            tenant_id,
            aggregation_ref,
            der_registrations,
            supersedes_registration_id,
            audit_chain_link,
            recorded_at_unix_ms,
            recorded_by,
        })
    }

    pub fn registration_id(&self) -> &str {
        &self.registration_id
    }

    pub fn tenant_id(&self) -> &str {
        &self.tenant_id
    }

    pub fn aggregation_ref(&self) -> &str {
        &self.aggregation_ref
    }

    pub fn der_registrations(&self) -> &[PjmDerRegistration] {
        &self.der_registrations
    }

    pub fn supersedes_registration_id(&self) -> Option<&str> {
        self.supersedes_registration_id.as_deref()
    }

    pub fn audit_chain_link(&self) -> &AuditChainLink {
        &self.audit_chain_link
    }

    pub fn recorded_at_unix_ms(&self) -> i64 {
        self.recorded_at_unix_ms
    }

    pub fn recorded_by(&self) -> &str {
        &self.recorded_by
    }
}

/// One append-only EDC-review tracking update (the 45-day window is
/// measured against the recorded `started_at_unix_ms` of the
/// AWAITING_EDC entry; the window cites [S-90] — re-verified at use).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EdcReviewUpdate {
    der_ref: String,
    seq: u64,
    to_state: EdcReviewState,
    /// For the AWAITING_EDC entry, when the review window started; for
    /// other entries, the recorded transition time. `None` where the
    /// entry does not carry a timestamp of its own kind.
    at_unix_ms: i64,
    audit_chain_link: AuditChainLink,
}

impl EdcReviewUpdate {
    pub fn new(
        der_ref: String,
        seq: u64,
        to_state: EdcReviewState,
        at_unix_ms: i64,
        audit_chain_link: AuditChainLink,
    ) -> Result<Self, MarketRegistrationError> {
        validate_text("der_ref", &der_ref, 128)?;
        if at_unix_ms < 0 {
            return Err(MarketRegistrationError::EmptyField { field: "at_unix_ms" });
        }
        Ok(Self {
            der_ref,
            seq,
            to_state,
            at_unix_ms,
            audit_chain_link,
        })
    }

    pub fn der_ref(&self) -> &str {
        &self.der_ref
    }

    pub fn seq(&self) -> u64 {
        self.seq
    }

    pub fn to_state(&self) -> EdcReviewState {
        self.to_state
    }

    pub fn at_unix_ms(&self) -> i64 {
        self.at_unix_ms
    }

    pub fn audit_chain_link(&self) -> &AuditChainLink {
        &self.audit_chain_link
    }
}

/// The EDC-review tracking journal: append-only updates per DER; the
/// CURRENT review state of a DER is the terminal-or-latest entry in
/// seq order (deterministic: BTreeMap keyed by (der_ref, seq)).
#[derive(Debug, Default)]
pub struct EdcReviewTracker {
    updates: BTreeMap<(String, u64), EdcReviewUpdate>,
}

impl EdcReviewTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends one review update, validating the edge from the DER's
    /// current state (the first update for a DER must start from
    /// NOT_STARTED semantics — i.e. it may be any state the record pack
    /// records, but subsequent updates must walk the closed table).
    pub fn append(&mut self, update: EdcReviewUpdate) -> Result<(), MarketRegistrationError> {
        let key = (update.der_ref().to_string(), update.seq());
        if self.updates.contains_key(&key) {
            return Err(MarketRegistrationError::DuplicateRegistration {
                registration_id: format!("{}#{}", update.der_ref(), update.seq()),
            });
        }
        if let Some((_, previous)) = self
            .updates
            .range((update.der_ref().to_string(), u64::MIN)..)
            .take_while(|((der, _), _)| der == update.der_ref())
            .last()
        {
            if !previous
                .to_state()
                .can_transition_to(update.to_state())
            {
                return Err(MarketRegistrationError::IllegalReviewTransition {
                    from: previous.to_state().name().to_string(),
                    to: update.to_state().name().to_string(),
                });
            }
        }
        self.updates.insert(key, update);
        Ok(())
    }

    /// The current review state for one DER (`None` = no update
    /// recorded).
    pub fn current_state(&self, der_ref: &str) -> Option<EdcReviewState> {
        self.updates
            .range((der_ref.to_string(), u64::MIN)..)
            .take_while(|((der, _), _)| der == der_ref)
            .last()
            .map(|((_, _), update)| update.to_state())
    }

    /// The recorded review-window start for one DER (the AWAITING_EDC
    /// entry's timestamp), used with `PJM_EDC_REVIEW_WINDOW_DAYS` [S-90]
    /// to derive the review due date at USE time (re-verified then —
    /// never stored as a claim).
    pub fn review_window_start(&self, der_ref: &str) -> Option<i64> {
        self.updates
            .range((der_ref.to_string(), u64::MIN)..)
            .take_while(|((der, _), _)| der == der_ref)
            .find(|((_, _), update)| update.to_state() == EdcReviewState::AwaitingEdc)
            .map(|((_, _), update)| update.at_unix_ms())
    }

    /// The recorded updates for one DER, ascending seq order.
    pub fn updates_for_der(&self, der_ref: &str) -> Vec<&EdcReviewUpdate> {
        self.updates
            .range((der_ref.to_string(), u64::MIN)..)
            .take_while(|((der, _), _)| der == der_ref)
            .map(|((_, _), update)| update)
            .collect()
    }
}

// --- ERCOT: PUCT + ERCOT dual registration [S-100] ---------------------------------

/// The closed registration-leg state vocabulary (shared by the ERCOT
/// dual legs; each leg records its OWN state — the dual-registration
/// structure [S-100] means neither leg implies the other).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RegistrationLegState {
    Initiated,
    SubmittedRecorded,
    AcceptedRecorded,
    RefusedRecorded,
    Withdrawn,
}

impl RegistrationLegState {
    pub fn name(&self) -> &'static str {
        match self {
            RegistrationLegState::Initiated => "INITIATED",
            RegistrationLegState::SubmittedRecorded => "SUBMITTED_RECORDED",
            RegistrationLegState::AcceptedRecorded => "ACCEPTED_RECORDED",
            RegistrationLegState::RefusedRecorded => "REFUSED_RECORDED",
            RegistrationLegState::Withdrawn => "WITHDRAWN",
        }
    }

    pub fn try_parse(name: &str) -> Option<Self> {
        match name {
            "INITIATED" => Some(RegistrationLegState::Initiated),
            "SUBMITTED_RECORDED" => Some(RegistrationLegState::SubmittedRecorded),
            "ACCEPTED_RECORDED" => Some(RegistrationLegState::AcceptedRecorded),
            "REFUSED_RECORDED" => Some(RegistrationLegState::RefusedRecorded),
            "WITHDRAWN" => Some(RegistrationLegState::Withdrawn),
            _ => None,
        }
    }
}

/// One registration leg (PUCT or ERCOT) as recorded data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistrationLeg {
    /// Which leg (`PUCT` | `ERCOT` — the dual-registration structure
    /// [S-100]; the spelling is recorded data).
    leg_name: String,
    /// The leg's reference as recorded (the registration or docket
    /// identifier).
    leg_ref: String,
    state: RegistrationLegState,
    recorded_at_unix_ms: i64,
}

impl RegistrationLeg {
    pub fn new(
        leg_name: String,
        leg_ref: String,
        state: RegistrationLegState,
        recorded_at_unix_ms: i64,
    ) -> Result<Self, MarketRegistrationError> {
        validate_text("leg_name", &leg_name, 32)?;
        validate_text("leg_ref", &leg_ref, 128)?;
        if recorded_at_unix_ms < 0 {
            return Err(MarketRegistrationError::EmptyField {
                field: "recorded_at_unix_ms",
            });
        }
        Ok(Self {
            leg_name,
            leg_ref,
            state,
            recorded_at_unix_ms,
        })
    }

    pub fn leg_name(&self) -> &str {
        &self.leg_name
    }

    pub fn leg_ref(&self) -> &str {
        &self.leg_ref
    }

    pub fn state(&self) -> RegistrationLegState {
        self.state
    }

    pub fn recorded_at_unix_ms(&self) -> i64 {
        self.recorded_at_unix_ms
    }
}

/// One ERCOT dual-registration record: the PUCT leg AND the ERCOT leg,
/// each with its own recorded state ([S-100]; neither leg implies the
/// other — a QSE/SC-record pairing is the record pack's data).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErcotDualRegistration {
    registration_id: String,
    tenant_id: String,
    puct_leg: RegistrationLeg,
    ercot_leg: RegistrationLeg,
    supersedes_registration_id: Option<String>,
    audit_chain_link: AuditChainLink,
    recorded_at_unix_ms: i64,
    recorded_by: String,
}

impl ErcotDualRegistration {
    pub fn new(
        registration_id: String,
        tenant_id: String,
        puct_leg: RegistrationLeg,
        ercot_leg: RegistrationLeg,
        supersedes_registration_id: Option<String>,
        audit_chain_link: AuditChainLink,
        recorded_at_unix_ms: i64,
        recorded_by: String,
    ) -> Result<Self, MarketRegistrationError> {
        validate_text("registration_id", &registration_id, 64)?;
        validate_text("tenant_id", &tenant_id, 64)?;
        validate_text("recorded_by", &recorded_by, 128)?;
        if recorded_at_unix_ms < 0 {
            return Err(MarketRegistrationError::EmptyField {
                field: "recorded_at_unix_ms",
            });
        }
        Ok(Self {
            registration_id,
            tenant_id,
            puct_leg,
            ercot_leg,
            supersedes_registration_id,
            audit_chain_link,
            recorded_at_unix_ms,
            recorded_by,
        })
    }

    pub fn registration_id(&self) -> &str {
        &self.registration_id
    }

    pub fn tenant_id(&self) -> &str {
        &self.tenant_id
    }

    pub fn puct_leg(&self) -> &RegistrationLeg {
        &self.puct_leg
    }

    pub fn ercot_leg(&self) -> &RegistrationLeg {
        &self.ercot_leg
    }

    pub fn supersedes_registration_id(&self) -> Option<&str> {
        self.supersedes_registration_id.as_deref()
    }

    pub fn audit_chain_link(&self) -> &AuditChainLink {
        &self.audit_chain_link
    }

    pub fn recorded_at_unix_ms(&self) -> i64 {
        self.recorded_at_unix_ms
    }

    pub fn recorded_by(&self) -> &str {
        &self.recorded_by
    }
}

// --- ISO-NE: CAMS/eMarket asset lifecycle [S-103] ----------------------------------

/// The closed ISO-NE asset-lifecycle state vocabulary (the CAMS record
/// and the eMarket listing are the two surfaces the ISO-NE Order 2222
/// readiness materials name [S-103]; the states are recorded data —
/// the exact CAMS/eMarket status names are transcribed at use).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum IsoNeAssetState {
    CamRecorded,
    EmarketListed,
    ActiveRecorded,
    RetiredRecorded,
}

impl IsoNeAssetState {
    pub fn name(&self) -> &'static str {
        match self {
            IsoNeAssetState::CamRecorded => "CAMS_RECORDED",
            IsoNeAssetState::EmarketListed => "EMARKET_LISTED",
            IsoNeAssetState::ActiveRecorded => "ACTIVE_RECORDED",
            IsoNeAssetState::RetiredRecorded => "RETIRED_RECORDED",
        }
    }

    pub fn try_parse(name: &str) -> Option<Self> {
        match name {
            "CAMS_RECORDED" => Some(IsoNeAssetState::CamRecorded),
            "EMARKET_LISTED" => Some(IsoNeAssetState::EmarketListed),
            "ACTIVE_RECORDED" => Some(IsoNeAssetState::ActiveRecorded),
            "RETIRED_RECORDED" => Some(IsoNeAssetState::RetiredRecorded),
            _ => None,
        }
    }
}

/// One ISO-NE asset registration record (the CAMS reference and the
/// eMarket listing as recorded data [S-103]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IsoNeAssetRegistration {
    registration_id: String,
    tenant_id: String,
    /// The CAMS asset reference as recorded.
    cams_ref: String,
    /// The eMarket listing reference as recorded (`None` = not listed
    /// yet — recorded as missing, never assumed).
    emarket_ref: Option<String>,
    state: IsoNeAssetState,
    supersedes_registration_id: Option<String>,
    audit_chain_link: AuditChainLink,
    recorded_at_unix_ms: i64,
    recorded_by: String,
}

impl IsoNeAssetRegistration {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        registration_id: String,
        tenant_id: String,
        cams_ref: String,
        emarket_ref: Option<String>,
        state: IsoNeAssetState,
        supersedes_registration_id: Option<String>,
        audit_chain_link: AuditChainLink,
        recorded_at_unix_ms: i64,
        recorded_by: String,
    ) -> Result<Self, MarketRegistrationError> {
        validate_text("registration_id", &registration_id, 64)?;
        validate_text("tenant_id", &tenant_id, 64)?;
        validate_text("cams_ref", &cams_ref, 128)?;
        if let Some(emarket) = emarket_ref.as_deref() {
            validate_text("emarket_ref", emarket, 128)?;
        }
        validate_text("recorded_by", &recorded_by, 128)?;
        if recorded_at_unix_ms < 0 {
            return Err(MarketRegistrationError::EmptyField {
                field: "recorded_at_unix_ms",
            });
        }
        Ok(Self {
            registration_id,
            tenant_id,
            cams_ref,
            emarket_ref,
            state,
            supersedes_registration_id,
            audit_chain_link,
            recorded_at_unix_ms,
            recorded_by,
        })
    }

    pub fn registration_id(&self) -> &str {
        &self.registration_id
    }

    pub fn tenant_id(&self) -> &str {
        &self.tenant_id
    }

    pub fn cams_ref(&self) -> &str {
        &self.cams_ref
    }

    pub fn emarket_ref(&self) -> Option<&str> {
        self.emarket_ref.as_deref()
    }

    pub fn state(&self) -> IsoNeAssetState {
        self.state
    }

    pub fn supersedes_registration_id(&self) -> Option<&str> {
        self.supersedes_registration_id.as_deref()
    }

    pub fn audit_chain_link(&self) -> &AuditChainLink {
        &self.audit_chain_link
    }

    pub fn recorded_at_unix_ms(&self) -> i64 {
        self.recorded_at_unix_ms
    }

    pub fn recorded_by(&self) -> &str {
        &self.recorded_by
    }
}

// --- Attestations as signed records -----------------------------------------------

/// The test-grade signature-algorithm labels (the LR-203 discipline: a
/// record carrying one of these labels is NEVER a release signature).
pub const TEST_GRADE_SIGNATURE_LABELS: [&str; 1] = ["fnv1a-test-grade"];

/// One attestation record: a SIGNED STATEMENT provided by a party,
/// recorded as data (who signed, what statement, the algorithm LABEL,
/// the signature DIGEST). The record stores that a signature exists; it
/// does NOT verify it and is NEVER a qualification or conformance claim
/// (`ATTESTATION_IS_RECORD_STATEMENT`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttestationRecord {
    attestation_id: String,
    tenant_id: String,
    jurisdiction: MarketJurisdiction,
    /// The subject the attestation covers (the registration, enrollment,
    /// or document reference — recorded).
    subject_ref: String,
    /// The attested statement text AS PROVIDED (recorded verbatim).
    attested_statement: String,
    /// The signer as recorded.
    signer: String,
    /// The signature-algorithm LABEL as recorded (a test-grade label is
    /// refused at construction — `TestGradeSignatureLabel`).
    signature_algorithm_label: String,
    /// The signature digest AS RECORDED (never interpreted).
    signature_digest: String,
    audit_chain_link: AuditChainLink,
    recorded_at_unix_ms: i64,
}

impl AttestationRecord {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        attestation_id: String,
        tenant_id: String,
        jurisdiction: MarketJurisdiction,
        subject_ref: String,
        attested_statement: String,
        signer: String,
        signature_algorithm_label: String,
        signature_digest: String,
        audit_chain_link: AuditChainLink,
        recorded_at_unix_ms: i64,
    ) -> Result<Self, MarketRegistrationError> {
        validate_text("attestation_id", &attestation_id, 64)?;
        validate_text("tenant_id", &tenant_id, 64)?;
        validate_text("subject_ref", &subject_ref, 256)?;
        validate_text("attested_statement", &attested_statement, 4096)?;
        validate_text("signer", &signer, 256)?;
        validate_text("signature_algorithm_label", &signature_algorithm_label, 64)?;
        validate_text("signature_digest", &signature_digest, 512)?;
        if TEST_GRADE_SIGNATURE_LABELS.contains(&signature_algorithm_label.as_str()) {
            return Err(MarketRegistrationError::TestGradeSignatureLabel {
                label: signature_algorithm_label,
            });
        }
        if recorded_at_unix_ms < 0 {
            return Err(MarketRegistrationError::EmptyField {
                field: "recorded_at_unix_ms",
            });
        }
        Ok(Self {
            attestation_id,
            tenant_id,
            jurisdiction,
            subject_ref,
            attested_statement,
            signer,
            signature_algorithm_label,
            signature_digest,
            audit_chain_link,
            recorded_at_unix_ms,
        })
    }

    pub fn attestation_id(&self) -> &str {
        &self.attestation_id
    }

    pub fn tenant_id(&self) -> &str {
        &self.tenant_id
    }

    pub fn jurisdiction(&self) -> MarketJurisdiction {
        self.jurisdiction
    }

    pub fn subject_ref(&self) -> &str {
        &self.subject_ref
    }

    pub fn attested_statement(&self) -> &str {
        &self.attested_statement
    }

    pub fn signer(&self) -> &str {
        &self.signer
    }

    pub fn signature_algorithm_label(&self) -> &str {
        &self.signature_algorithm_label
    }

    pub fn signature_digest(&self) -> &str {
        &self.signature_digest
    }

    pub fn audit_chain_link(&self) -> &AuditChainLink {
        &self.audit_chain_link
    }

    pub fn recorded_at_unix_ms(&self) -> i64 {
        self.recorded_at_unix_ms
    }
}

// --- The registration registry -------------------------------------------------------

/// The one-of-three registration-record variants (ADR-011:
/// jurisdiction-specific records — a fourth jurisdiction enters as a NEW
/// variant in a record pack, never by widening these).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistrationRecord {
    Pjm(PjmAggregationRegistration),
    Ercot(ErcotDualRegistration),
    IsoNe(IsoNeAssetRegistration),
}

impl RegistrationRecord {
    /// The record identity.
    pub fn registration_id(&self) -> &str {
        match self {
            RegistrationRecord::Pjm(record) => record.registration_id(),
            RegistrationRecord::Ercot(record) => record.registration_id(),
            RegistrationRecord::IsoNe(record) => record.registration_id(),
        }
    }

    /// The owning tenant.
    pub fn tenant_id(&self) -> &str {
        match self {
            RegistrationRecord::Pjm(record) => record.tenant_id(),
            RegistrationRecord::Ercot(record) => record.tenant_id(),
            RegistrationRecord::IsoNe(record) => record.tenant_id(),
        }
    }

    /// The jurisdiction the record belongs to.
    pub fn jurisdiction(&self) -> MarketJurisdiction {
        match self {
            RegistrationRecord::Pjm(_) => MarketJurisdiction::Pjm,
            RegistrationRecord::Ercot(_) => MarketJurisdiction::Ercot,
            RegistrationRecord::IsoNe(_) => MarketJurisdiction::IsoNe,
        }
    }

    /// The supersede link, if any.
    pub fn supersedes_registration_id(&self) -> Option<&str> {
        match self {
            RegistrationRecord::Pjm(record) => record.supersedes_registration_id(),
            RegistrationRecord::Ercot(record) => record.supersedes_registration_id(),
            RegistrationRecord::IsoNe(record) => record.supersedes_registration_id(),
        }
    }
}

/// The BTreeMap-backed registration registry: register-only plus the
/// supersede model (a status change is a NEW record superseding the old
/// one; every version retained). Keyed by (jurisdiction, tenant,
/// registration_id).
#[derive(Debug, Default)]
pub struct MarketRegistrationRegistry {
    records: BTreeMap<(MarketJurisdiction, String, String), RegistrationRecord>,
    superseded_by: BTreeMap<(MarketJurisdiction, String, String), (MarketJurisdiction, String, String)>,
    edc_review_tracker: EdcReviewTracker,
    attestations: BTreeMap<(MarketJurisdiction, String, String), AttestationRecord>,
}

impl MarketRegistrationRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers one registration record. A duplicate CURRENT identity
    /// is a refusal; the supersede target must exist in the SAME
    /// jurisdiction and tenant and not already be superseded.
    pub fn register(&mut self, record: RegistrationRecord) -> Result<(), MarketRegistrationError> {
        let key = (
            record.jurisdiction(),
            record.tenant_id().to_string(),
            record.registration_id().to_string(),
        );
        if self.superseded_by.contains_key(&key) {
            return Err(MarketRegistrationError::RecordSuperseded {
                registration_id: record.registration_id().to_string(),
            });
        }
        if self.records.contains_key(&key) {
            return Err(MarketRegistrationError::DuplicateRegistration {
                registration_id: record.registration_id().to_string(),
            });
        }
        if let Some(superseded) = record.supersedes_registration_id() {
            let target_key = (record.jurisdiction(), record.tenant_id().to_string(), superseded.to_string());
            if !self.records.contains_key(&target_key) {
                return Err(MarketRegistrationError::RecordNotFound {
                    registration_id: superseded.to_string(),
                });
            }
            if self.superseded_by.contains_key(&target_key) {
                return Err(MarketRegistrationError::RecordSuperseded {
                    registration_id: superseded.to_string(),
                });
            }
            self.superseded_by.insert(target_key, key.clone());
        }
        self.records.insert(key, record);
        Ok(())
    }

    /// The EDC-review tracking journal (PJM [S-90]).
    pub fn edc_review_tracker(&mut self) -> &mut EdcReviewTracker {
        &mut self.edc_review_tracker
    }

    /// The EDC-review tracking journal (read-only).
    pub fn edc_reviews(&self) -> &EdcReviewTracker {
        &self.edc_review_tracker
    }

    /// Records one attestation (signed records as data). A duplicate
    /// attestation identity is a refusal.
    pub fn record_attestation(
        &mut self,
        attestation: AttestationRecord,
    ) -> Result<(), MarketRegistrationError> {
        let key = (
            attestation.jurisdiction(),
            attestation.tenant_id().to_string(),
            attestation.attestation_id().to_string(),
        );
        if self.attestations.contains_key(&key) {
            return Err(MarketRegistrationError::DuplicateRegistration {
                registration_id: attestation.attestation_id().to_string(),
            });
        }
        self.attestations.insert(key, attestation);
        Ok(())
    }

    /// The current (non-superseded) registration records for one tenant
    /// and jurisdiction, ascending registration_id order.
    pub fn current_records_for_tenant(
        &self,
        jurisdiction: MarketJurisdiction,
        tenant_id: &str,
    ) -> Vec<&RegistrationRecord> {
        self.records
            .range((jurisdiction, tenant_id.to_string(), String::new())..)
            .take_while(|((juris, tenant, _), _)| {
                *juris == jurisdiction && tenant == tenant_id
            })
            .filter(|(key, _)| !self.superseded_by.contains_key(key))
            .map(|(_, record)| record)
            .collect()
    }

    /// The recorded attestations for one tenant and jurisdiction,
    /// ascending attestation_id order.
    pub fn attestations_for_tenant(
        &self,
        jurisdiction: MarketJurisdiction,
        tenant_id: &str,
    ) -> Vec<&AttestationRecord> {
        self.attestations
            .range((jurisdiction, tenant_id.to_string(), String::new())..)
            .take_while(|((juris, tenant, _), _)| {
                *juris == jurisdiction && tenant == tenant_id
            })
            .map(|(_, attestation)| attestation)
            .collect()
    }
}
