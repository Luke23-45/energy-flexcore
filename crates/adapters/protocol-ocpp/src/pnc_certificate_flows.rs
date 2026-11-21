// FIM-F13.02 + FIM-F13.04 — ISO 15118 Plug & Charge OCPP 2.1 certificate
// flow handlers (code surface): Get15118EVCertificate, CertificateSigned,
// InstallCertificate, plus the five record-41 §3 ungate conditions enforced
// as fail-closed runtime gates.
//
// HONESTY HEADER (FIM-F13.06; FIM standing decision 2):
//
// - **Implemented-under-FIM-F13, CODE SURFACE ONLY.** The owner authorized
//   the CODE SURFACE of feature F-13 (ledger_F13.md). No CA is stood up, no
//   certificate exists, no key material exists, no PnC session ever
//   existed, and no ISO 15118 or OCA specification text is held by this
//   program. PKI interop is UNPROVEN.
// - **WRITTEN-UNRUN**: never compiled, never executed; execution is a
//   HUMAN LAB GATE (conventions §1.3; the FIM hard boundary).
// - **The five conditions are NOT met and are NOT claimed met** ([`
//   PncGateConditions`]: a named PKI operator; trust-anchor distribution
//   agreed; an OCSP service at fail-closed reliability; the OCA-text
//   verification pass; D5 closed naming OCPP and the edition — record
//   docs/phases-11-20/phase-17-ev-fleet-charging/records/
//   41_p17_12_pnc_certificate_management_design.md §3, all NOT-STARTED).
//   They are RUNTIME/CONFIGURATION GATES: the composition fills the
//   conditions record, each defaults to [`PncGateState::Unmet`], and every
//   handler below REFUSES while any is UNMET, naming the unmet condition.
//   Each condition is a HUMAN GATE (an owner/operator decision or an
//   operational contract — none can be satisfied by code).
// - **Message names are record-41 VERBATIM** ([`GET_15118_EV_CERTIFICATE`],
//   [`CERTIFICATE_SIGNED`], [`INSTALL_CERTIFICATE`]). The message names are
//   the ONLY pinned protocol detail (record 41 §2; its block letters are
//   UNVERIFIED against the OCA text). EVERY field spelling below is
//   UNVERIFIED-against-OCPP-2.1 and carried as declared data; record 41 §4
//   owns the implementation-time checks. A mismatch at that gate is a code
//   or record edit, never a silent reinterpretation.
// - **The closed framing action vocabulary is NOT widened.** None of these
//   names is a `crate::framing::Action` variant (record 41 §5.1); the
//   dispatch layer does not route them; these handlers are the record-level
//   code surface the composition wires when (and only when) the gates are
//   met.
// - **Issuance stays EXTERNAL.** The flow states reuse the wave-1 record-03
//   positions (REQUESTED → SIGNED_AVAILABLE → INSTALLED / REJECTED) and
//   model protocol POSITIONS only: the issuance act between REQUESTED and
//   SIGNED_AVAILABLE is the deployment's PKI operations — a HUMAN GATE this
//   module only records outcomes of, never performs.
// - **Get15118EVCertificate is a relay RECORD, not a router.** This program
//   implements no PKI routing (record 41 §2): the handler records the
//   request and stops; the response path belongs to the deployment's PKI
//   operations once (and only once) the gates are met.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::model::{CertificateSignedStatus, CertificateType, InstallCertificateRequest};
use crate::pnc_trust_store::{PncTrustStoreError, V2gRootCaTrustStore};

// --- The recorded message names (record 41, VERBATIM) --------------------------

/// The Get15118EVCertificate message name (record 41 §2, VERBATIM). NOT a
/// `crate::framing::Action` variant — the closed action vocabulary is not
/// widened by this module (record 41 §5.1).
pub const GET_15118_EV_CERTIFICATE: &str = "Get15118EVCertificate";

/// The CertificateSigned message name (record 41 §2, VERBATIM; the wave-1
/// `crate::framing::Action::CertificateSigned` vocabulary is untouched).
pub const CERTIFICATE_SIGNED: &str = "CertificateSigned";

/// The InstallCertificate message name (record 41 §2, VERBATIM; the wave-1
/// `crate::framing::Action::InstallCertificate` vocabulary is untouched).
pub const INSTALL_CERTIFICATE: &str = "InstallCertificate";

// --- The five record-41 §3 ungate conditions (FIM-F13.04) ----------------------

/// The five record-41 §3 ungate conditions, in the record's own order.
/// Each is a HUMAN GATE: an operational or owner decision no code can
/// satisfy. None is met; none is claimed met.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PncGateCondition {
    /// §3.1 — a named PKI operator for the deployment's V2G Root CA
    /// hierarchy (commercial entity, contract, custody decision).
    NamedPkiOperator,
    /// §3.2 — trust-anchor distribution agreed: who publishes the anchor
    /// set the CSMS validates against, and in what artifact form.
    TrustAnchorDistributionAgreement,
    /// §3.3 — an OCSP (or equivalent revocation) service available at the
    /// reliability the fail-closed posture requires.
    OcspServiceAvailabilityContract,
    /// §3.4 — the OCA-text verification pass over every UNVERIFIED item of
    /// record 41 §4 (and the Edition 2 publication-date conflict resolved).
    OcaTextVerificationPass,
    /// §3.5 — D5 closed by the human owner naming OCPP (and the edition).
    D5ClosureNamingOcppEdition,
}

impl PncGateCondition {
    /// All five, in record-41 §3 order (the refusal order — the FIRST unmet
    /// condition in this order is the one named).
    pub const ALL: [PncGateCondition; 5] = [
        PncGateCondition::NamedPkiOperator,
        PncGateCondition::TrustAnchorDistributionAgreement,
        PncGateCondition::OcspServiceAvailabilityContract,
        PncGateCondition::OcaTextVerificationPass,
        PncGateCondition::D5ClosureNamingOcppEdition,
    ];

    /// The stable machine-readable gate name (what a refusal carries).
    pub const fn name(self) -> &'static str {
        match self {
            PncGateCondition::NamedPkiOperator => "PNC_GATE_PKI_OPERATOR_NAMED",
            PncGateCondition::TrustAnchorDistributionAgreement => {
                "PNC_GATE_TRUST_ANCHOR_DISTRIBUTION_AGREED"
            }
            PncGateCondition::OcspServiceAvailabilityContract => "PNC_GATE_OCSP_SERVICE_CONTRACTED",
            PncGateCondition::OcaTextVerificationPass => "PNC_GATE_OCA_TEXT_VERIFICATION_PASS",
            PncGateCondition::D5ClosureNamingOcppEdition => "PNC_GATE_D5_CLOSED_NAMING_OCPP_EDITION",
        }
    }

    /// The record-41 §3 clause the condition comes from.
    pub const fn record_41_clause(self) -> &'static str {
        match self {
            PncGateCondition::NamedPkiOperator => "record-41 §3.1",
            PncGateCondition::TrustAnchorDistributionAgreement => "record-41 §3.2",
            PncGateCondition::OcspServiceAvailabilityContract => "record-41 §3.3",
            PncGateCondition::OcaTextVerificationPass => "record-41 §3.4",
            PncGateCondition::D5ClosureNamingOcppEdition => "record-41 §3.5",
        }
    }

    /// The human-gate statement: what satisfying the condition actually
    /// requires (a person/process/contract, never code).
    pub const fn human_gate_statement(self) -> &'static str {
        match self {
            PncGateCondition::NamedPkiOperator => {
                "a named PKI operator for the deployment's V2G Root CA hierarchy (commercial \
                 entity, contract, custody decision — HSM-class; a build-vs-buy E-decision \
                 for the human owner)"
            }
            PncGateCondition::TrustAnchorDistributionAgreement => {
                "trust-anchor distribution agreed: who publishes the anchor set the CSMS \
                 validates against, and in what artifact form"
            }
            PncGateCondition::OcspServiceAvailabilityContract => {
                "an OCSP (or equivalent revocation) service available to the deployment at \
                 the reliability the fail-closed posture requires — a fail-closed check \
                 against an unreachable responder denies every PnC session"
            }
            PncGateCondition::OcaTextVerificationPass => {
                "the OCA-text verification pass over every UNVERIFIED item of record 41 §4 \
                 (and the Edition 2 publication-date conflict resolved)"
            }
            PncGateCondition::D5ClosureNamingOcppEdition => {
                "D5 closed by the human owner naming OCPP (and the edition) — the PnC \
                 design rides on the P17-E-1 edition baseline, which stays OPEN"
            }
        }
    }
}

/// The recorded state of one gate condition. The honest default is
/// [`PncGateState::Unmet`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PncGateState {
    /// The condition is NOT satisfied (the default for all five).
    Unmet,
    /// The deployment recorded the condition as met, with the reference
    /// that evidences it (the operator's name/contract reference, the
    /// distribution-agreement reference, the OCSP service contract, the
    /// OCA verification-pass reference, the D5 closure record). Recording
    /// a MET state is a composition-root decision made WITH the human
    /// gate's evidence — the code never infers one.
    Met { reference: String },
}

/// The five-condition record the composition fills (FIM-F13.04). **All five
/// default to UNMET**, and the handlers refuse while any is UNMET. The
/// conditions are NOT met today and are NOT claimed met anywhere.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PncGateConditions {
    named_pki_operator: PncGateState,
    trust_anchor_distribution_agreement: PncGateState,
    ocsp_service_availability_contract: PncGateState,
    oca_text_verification_pass: PncGateState,
    d5_closure_naming_ocpp_edition: PncGateState,
}

impl Default for PncGateConditions {
    fn default() -> Self {
        Self::unmet()
    }
}

impl PncGateConditions {
    /// A conditions record with ALL FIVE conditions UNMET (the only state
    /// this repository ships; a composition that turns a condition MET
    /// does so explicitly, per condition, with its evidence reference).
    pub fn unmet() -> Self {
        Self {
            named_pki_operator: PncGateState::Unmet,
            trust_anchor_distribution_agreement: PncGateState::Unmet,
            ocsp_service_availability_contract: PncGateState::Unmet,
            oca_text_verification_pass: PncGateState::Unmet,
            d5_closure_naming_ocpp_edition: PncGateState::Unmet,
        }
    }

    /// Records ONE condition met with its evidence reference (fail-closed:
    /// an empty reference is refused — a condition "met" without evidence
    /// is not met).
    pub fn record_met(
        &mut self,
        condition: PncGateCondition,
        reference: String,
    ) -> Result<(), PncCertificateError> {
        if reference.trim().is_empty() {
            return Err(PncCertificateError::EmptyField("gate_reference"));
        }
        let state = match condition {
            PncGateCondition::NamedPkiOperator => &mut self.named_pki_operator,
            PncGateCondition::TrustAnchorDistributionAgreement => {
                &mut self.trust_anchor_distribution_agreement
            }
            PncGateCondition::OcspServiceAvailabilityContract => {
                &mut self.ocsp_service_availability_contract
            }
            PncGateCondition::OcaTextVerificationPass => &mut self.oca_text_verification_pass,
            PncGateCondition::D5ClosureNamingOcppEdition => {
                &mut self.d5_closure_naming_ocpp_edition
            }
        };
        *state = PncGateState::Met { reference };
        Ok(())
    }

    /// The recorded state of one condition.
    pub fn state(&self, condition: PncGateCondition) -> &PncGateState {
        match condition {
            PncGateCondition::NamedPkiOperator => &self.named_pki_operator,
            PncGateCondition::TrustAnchorDistributionAgreement => {
                &self.trust_anchor_distribution_agreement
            }
            PncGateCondition::OcspServiceAvailabilityContract => {
                &self.ocsp_service_availability_contract
            }
            PncGateCondition::OcaTextVerificationPass => &self.oca_text_verification_pass,
            PncGateCondition::D5ClosureNamingOcppEdition => &self.d5_closure_naming_ocpp_edition,
        }
    }

    /// The FIRST unmet condition in record-41 §3 order, when any is unmet.
    pub fn first_unmet(&self) -> Option<PncGateCondition> {
        PncGateCondition::ALL
            .into_iter()
            .find(|condition| matches!(self.state(*condition), PncGateState::Unmet))
    }

    /// The fail-closed gate every PnC handler passes through: refuses with
    /// the named [`PncCertificateError::GateConditionUnmet`] while ANY of
    /// the five conditions is UNMET.
    pub fn require_all_met(&self) -> Result<(), PncCertificateError> {
        match self.first_unmet() {
            Some(condition) => Err(PncCertificateError::GateConditionUnmet { condition }),
            None => Ok(()),
        }
    }

    /// The honest standing statement (what this repository ships).
    pub const ALL_UNMET_BY_DEFAULT_STATEMENT: &'static str =
        "ALL FIVE record-41 §3 ungate conditions default to UNMET and are NOT claimed met: \
         a named PKI operator, trust-anchor distribution agreed, an OCSP service at \
         fail-closed reliability, the OCA-text verification pass, and D5 closed naming \
         OCPP and the edition. Every PnC handler refuses while any is UNMET, naming the \
         unmet condition. The owner authorized the CODE SURFACE only (ledger_F13.md, \
         FIM standing decision 2); each condition is a HUMAN GATE.";
}

// --- The Get15118EVCertificate payload (UNVERIFIED fields; name VERBATIM) -------

/// The Get15118EVCertificate REQUEST payload (charge point → CSMS; record 41
/// §2). The message NAME is record-41 verbatim; the field spellings below
/// are UNVERIFIED-against-OCPP-2.1 (record 41 §4.1 owns the check) and are
/// carried verbatim as declared data, never parsed into protocol claims.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Get15118EVCertificateRequest {
    /// The requested certificate-operation discriminator (UNVERIFIED —
    /// record 41 §4.1). Carried verbatim.
    #[serde(rename = "action", skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    /// The certificate-schema label the station declares, when it declares
    /// one (UNVERIFIED — record 41 §4.1). Carried verbatim.
    #[serde(rename = "certificateSchema", skip_serializing_if = "Option::is_none")]
    pub certificate_schema_label: Option<String>,
}

impl Get15118EVCertificateRequest {
    /// Validates the declared labels (a whitespace-only label is refused,
    /// never defaulted; the check is a shape gate, not a protocol claim).
    pub fn validate(&self) -> Result<(), PncCertificateError> {
        if let Some(action) = &self.action {
            if action.trim().is_empty() {
                return Err(PncCertificateError::EmptyField("action"));
            }
        }
        if let Some(schema) = &self.certificate_schema_label {
            if schema.trim().is_empty() {
                return Err(PncCertificateError::EmptyField("certificate_schema_label"));
            }
        }
        Ok(())
    }

    /// The deterministic relay-record reference built from the declared
    /// labels (unspecified labels record as `unspecified`).
    pub fn request_reference(&self) -> String {
        format!(
            "get15118ev:{}:{}",
            self.action.as_deref().unwrap_or("unspecified"),
            self.certificate_schema_label
                .as_deref()
                .unwrap_or("unspecified")
        )
    }
}

// --- The certificate-flow ledger (record-03 positions, reused) ------------------

/// The PnC certificate-flow states — the wave-1 record-03 positions
/// (REQUESTED → SIGNED_AVAILABLE → INSTALLED / REJECTED), REUSED for the
/// PnC leg (record 41 §2: "the wave-1 flows ... REUSED as-is; issuance
/// stays EXTERNAL"). Issuance between REQUESTED and SIGNED_AVAILABLE is the
/// deployment's PKI operations (a HUMAN GATE); this ledger only records
/// outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PncCertificateFlowState {
    /// A Get15118EVCertificate request was recorded (the relay surface;
    /// nothing was routed and nothing was issued).
    EvCertificateRequested { request_reference: String },
    /// The CertificateSigned outcome was recorded: the EXTERNAL PKI
    /// operations produced the signed chain and the CertificateSigned
    /// REQUEST was sent to the charge point (the chain rides a LABEL).
    SignedAvailable { chain_label: String },
    /// The charge point's Accepted response to the CertificateSigned
    /// request was recorded.
    Installed { certificate_label: String },
    /// The flow ended in a named rejection (charge point rejected, PKI
    /// refused, or a timeout — the reason is human-safe).
    Rejected { reason: String },
}

impl PncCertificateFlowState {
    /// Canonical name for records (the record-03 names, verbatim).
    pub const fn name(&self) -> &'static str {
        match self {
            PncCertificateFlowState::EvCertificateRequested { .. } => "REQUESTED",
            PncCertificateFlowState::SignedAvailable { .. } => "SIGNED_AVAILABLE",
            PncCertificateFlowState::Installed { .. } => "INSTALLED",
            PncCertificateFlowState::Rejected { .. } => "REJECTED",
        }
    }
}

/// One PnC certificate flow (per charge point per certificate type).
#[derive(Debug, Clone)]
pub struct PncCertificateFlow {
    pub charge_point_id: String,
    pub certificate_type: CertificateType,
    pub state: PncCertificateFlowState,
}

/// One recorded InstallCertificate outcome (the CSMS → charge-point
/// install leg, keyed separately from the issuance leg: installing a
/// root/sub-CA certificate has no REQUESTED predecessor).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PncInstallRecord {
    pub charge_point_id: String,
    pub certificate_type: CertificateType,
    /// The certificate LABEL the request carried (never material).
    pub certificate_label: String,
    /// The trust-store anchor whose bytes the installed certificate
    /// byte-equals (the fail-closed equality check's outcome).
    pub anchor_label: String,
    /// `None` while the charge point's confirmation is outstanding;
    /// `Some(true)` = the charge point accepted; `Some(false)` = rejected.
    pub confirmation: Option<bool>,
    /// The charge point's rejection reason, when it rejected one.
    pub rejection_reason: Option<String>,
    pub recorded_millis: i64,
}

/// The PnC certificate-flow ledger (deterministic `BTreeMap`s keyed
/// `"<charge_point>#<certificate_type>"`).
#[derive(Debug, Default)]
pub struct PncCertificateFlowLedger {
    flows: BTreeMap<String, PncCertificateFlow>,
    installs: BTreeMap<String, PncInstallRecord>,
}

impl PncCertificateFlowLedger {
    pub fn new() -> Self {
        Self::default()
    }

    fn key(charge_point_id: &str, certificate_type: CertificateType) -> String {
        format!("{charge_point_id}#{}", certificate_type.name())
    }

    /// Records a Get15118EVCertificate request (the flow opens at
    /// REQUESTED). Overwrites only a terminal (Rejected) previous flow —
    /// an open flow is refused (one flow per target at a time).
    pub fn open_ev_certificate_request(
        &mut self,
        charge_point_id: &str,
        certificate_type: CertificateType,
        request_reference: String,
    ) -> Result<(), PncCertificateError> {
        if request_reference.trim().is_empty() {
            return Err(PncCertificateError::EmptyField("request_reference"));
        }
        let key = Self::key(charge_point_id, certificate_type);
        if let Some(existing) = self.flows.get(&key) {
            if !matches!(existing.state, PncCertificateFlowState::Rejected { .. }) {
                return Err(PncCertificateError::FlowStateInvalid {
                    current: existing.state.name(),
                    attempted: "open_ev_certificate_request",
                });
            }
        }
        self.flows.insert(
            key,
            PncCertificateFlow {
                charge_point_id: charge_point_id.to_string(),
                certificate_type,
                state: PncCertificateFlowState::EvCertificateRequested { request_reference },
            },
        );
        Ok(())
    }

    /// Records the CertificateSigned outcome (REQUESTED →
    /// SIGNED_AVAILABLE). The issuance act is EXTERNAL (a HUMAN GATE in
    /// the deployment's PKI operations) — this records the outcome, never
    /// performs the issuance.
    pub fn record_certificate_signed(
        &mut self,
        charge_point_id: &str,
        certificate_type: CertificateType,
        chain_label: String,
    ) -> Result<(), PncCertificateError> {
        if chain_label.trim().is_empty() {
            return Err(PncCertificateError::EmptyField("chain_label"));
        }
        let key = Self::key(charge_point_id, certificate_type);
        let flow = self.flows.get_mut(&key).ok_or(
            PncCertificateError::FlowStateInvalid {
                current: "ABSENT",
                attempted: "record_certificate_signed",
            },
        )?;
        match &flow.state {
            PncCertificateFlowState::EvCertificateRequested { .. } => {
                flow.state = PncCertificateFlowState::SignedAvailable { chain_label };
                Ok(())
            }
            other => Err(PncCertificateError::FlowStateInvalid {
                current: other.name(),
                attempted: "record_certificate_signed",
            }),
        }
    }

    /// Records the charge point's response to the CertificateSigned
    /// request (SIGNED_AVAILABLE → INSTALLED on Accepted, → REJECTED on
    /// Rejected).
    pub fn record_certificate_signed_confirmation(
        &mut self,
        charge_point_id: &str,
        certificate_type: CertificateType,
        status: CertificateSignedStatus,
        rejection_reason: String,
    ) -> Result<(), PncCertificateError> {
        let key = Self::key(charge_point_id, certificate_type);
        let flow = self.flows.get_mut(&key).ok_or(
            PncCertificateError::FlowStateInvalid {
                current: "ABSENT",
                attempted: "record_certificate_signed_confirmation",
            },
        )?;
        match (&flow.state, status) {
            (
                PncCertificateFlowState::SignedAvailable { chain_label },
                CertificateSignedStatus::Accepted,
            ) => {
                let label = chain_label.clone();
                flow.state = PncCertificateFlowState::Installed { certificate_label: label };
                Ok(())
            }
            (
                PncCertificateFlowState::SignedAvailable { .. },
                CertificateSignedStatus::Rejected,
            ) => {
                flow.state = PncCertificateFlowState::Rejected {
                    reason: if rejection_reason.trim().is_empty() {
                        "the charge point rejected the signed certificate".to_string()
                    } else {
                        rejection_reason
                    },
                };
                Ok(())
            }
            (other, _) => Err(PncCertificateError::FlowStateInvalid {
                current: other.name(),
                attempted: "record_certificate_signed_confirmation",
            }),
        }
    }

    /// Records an InstallCertificate instruction (the CSMS → charge-point
    /// install leg) AFTER the trust-store byte-equality check passed. The
    /// charge point's confirmation is outstanding until
    /// [`PncCertificateFlowLedger::record_install_confirmation`].
    pub fn record_install_sent(
        &mut self,
        charge_point_id: &str,
        certificate_type: CertificateType,
        certificate_label: String,
        anchor_label: String,
        recorded_millis: i64,
    ) -> Result<(), PncCertificateError> {
        if certificate_label.trim().is_empty() {
            return Err(PncCertificateError::EmptyField("certificate_label"));
        }
        if anchor_label.trim().is_empty() {
            return Err(PncCertificateError::EmptyField("anchor_label"));
        }
        let key = Self::key(charge_point_id, certificate_type);
        self.installs.insert(
            key,
            PncInstallRecord {
                charge_point_id: charge_point_id.to_string(),
                certificate_type,
                certificate_label,
                anchor_label,
                confirmation: None,
                rejection_reason: None,
                recorded_millis,
            },
        );
        Ok(())
    }

    /// Records the charge point's response to the InstallCertificate
    /// instruction (Accepted / Rejected with a reason).
    pub fn record_install_confirmation(
        &mut self,
        charge_point_id: &str,
        certificate_type: CertificateType,
        accepted: bool,
        rejection_reason: String,
    ) -> Result<(), PncCertificateError> {
        let key = Self::key(charge_point_id, certificate_type);
        let record = self.installs.get_mut(&key).ok_or(
            PncCertificateError::FlowStateInvalid {
                current: "ABSENT",
                attempted: "record_install_confirmation",
            },
        )?;
        if record.confirmation.is_some() {
            return Err(PncCertificateError::FlowStateInvalid {
                current: if record.confirmation == Some(true) {
                    "INSTALLED"
                } else {
                    "REJECTED"
                },
                attempted: "record_install_confirmation",
            });
        }
        record.confirmation = Some(accepted);
        if !accepted {
            record.rejection_reason = Some(if rejection_reason.trim().is_empty() {
                "the charge point rejected the certificate installation".to_string()
            } else {
                rejection_reason
            });
        }
        Ok(())
    }

    /// The issuance-leg flow, when one exists.
    pub fn flow(
        &self,
        charge_point_id: &str,
        certificate_type: CertificateType,
    ) -> Option<&PncCertificateFlow> {
        self.flows.get(&Self::key(charge_point_id, certificate_type))
    }

    /// The install-leg record, when one exists.
    pub fn install_record(
        &self,
        charge_point_id: &str,
        certificate_type: CertificateType,
    ) -> Option<&PncInstallRecord> {
        self.installs
            .get(&Self::key(charge_point_id, certificate_type))
    }
}

// --- The relay record ----------------------------------------------------------

/// The Get15118EVCertificate relay OUTCOME. This program implements no PKI
/// routing (record 41 §2): the handler records the request and STOPS; the
/// response path is the deployment's PKI operations (a HUMAN GATE), which
/// exists only when the five conditions are met.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PncRelayOutcome {
    /// The request was recorded for the deployment's PKI operations.
    RecordedForPkiRouting,
}

impl PncRelayOutcome {
    /// Canonical name for records.
    pub const fn name(self) -> &'static str {
        match self {
            PncRelayOutcome::RecordedForPkiRouting => "RECORDED_FOR_PKI_ROUTING",
        }
    }
}

/// One recorded Get15118EVCertificate relay record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PncRelayRecord {
    /// The recorded message name ([`GET_15118_EV_CERTIFICATE`], verbatim).
    pub action: &'static str,
    pub charge_point_id: String,
    /// The certificate type the relay targets (the EV contract
    /// certificate: `V2gCertificate` — UNVERIFIED mapping, record 41 §4.5).
    pub certificate_type: CertificateType,
    pub request_reference: String,
    pub outcome: PncRelayOutcome,
    pub recorded_millis: i64,
}

// --- The handler surface ---------------------------------------------------------

/// The PnC certificate-flow handler surface (FIM-F13.02): the three
/// recorded flows, each validating against the five-condition gate set and
/// the V2G trust store before anything is recorded. Every refusal is named
/// and fail-closed.
#[derive(Debug, Default)]
pub struct PncCertificateSurface {
    ledger: PncCertificateFlowLedger,
}

impl PncCertificateSurface {
    pub fn new() -> Self {
        Self::default()
    }

    /// Handles Get15118EVCertificate (charge point → CSMS): the five gates
    /// must ALL be recorded met, the V2G trust store must hold at least one
    /// anchor, and the request shape must validate — otherwise the named
    /// refusal (the unmet gate condition is named first). On success the
    /// request is RECORDED for the deployment's PKI operations; nothing is
    /// routed, issued, or answered.
    pub fn handle_get_15118_ev_certificate(
        &mut self,
        trust_store: &V2gRootCaTrustStore,
        gates: &PncGateConditions,
        charge_point_id: &str,
        request: &Get15118EVCertificateRequest,
        now_millis: i64,
    ) -> Result<PncRelayRecord, PncCertificateError> {
        gates.require_all_met()?;
        if charge_point_id.trim().is_empty() {
            return Err(PncCertificateError::EmptyField("charge_point_id"));
        }
        request.validate()?;
        // The V2G hierarchy must exist in the store before an EV-certificate
        // request can even be recorded (fail-closed: an empty store refuses
        // by name).
        trust_store
            .require_any_anchor()
            .map_err(PncCertificateError::TrustStore)?;
        self.ledger.open_ev_certificate_request(
            charge_point_id,
            CertificateType::V2gCertificate,
            request.request_reference(),
        )?;
        Ok(PncRelayRecord {
            action: GET_15118_EV_CERTIFICATE,
            charge_point_id: charge_point_id.to_string(),
            certificate_type: CertificateType::V2gCertificate,
            request_reference: request.request_reference(),
            outcome: PncRelayOutcome::RecordedForPkiRouting,
            recorded_millis: now_millis,
        })
    }

    /// Handles CertificateSigned (CSMS → charge point): the signed chain
    /// (a LABEL) of the externally-issued EV certificate is recorded into
    /// the flow opened by the Get15118EVCertificate relay (REQUESTED →
    /// SIGNED_AVAILABLE). The issuance act itself is the deployment's PKI
    /// operations — a HUMAN GATE.
    pub fn handle_certificate_signed(
        &mut self,
        gates: &PncGateConditions,
        charge_point_id: &str,
        certificate_type: CertificateType,
        chain_label: String,
    ) -> Result<(), PncCertificateError> {
        gates.require_all_met()?;
        if charge_point_id.trim().is_empty() {
            return Err(PncCertificateError::EmptyField("charge_point_id"));
        }
        self.ledger
            .record_certificate_signed(charge_point_id, certificate_type, chain_label)
    }

    /// Records the charge point's response to the CertificateSigned
    /// request (Accepted → INSTALLED / Rejected → REJECTED).
    pub fn record_certificate_signed_confirmation(
        &mut self,
        charge_point_id: &str,
        certificate_type: CertificateType,
        status: CertificateSignedStatus,
        rejection_reason: String,
    ) -> Result<(), PncCertificateError> {
        self.ledger
            .record_certificate_signed_confirmation(
                charge_point_id,
                certificate_type,
                status,
                rejection_reason,
            )
    }

    /// Handles InstallCertificate (CSMS → charge point): the certificate
    /// being installed must byte-equal an anchor recorded in the trust
    /// store (the CSMS never instructs a charge point to trust a
    /// certificate its own store does not record — fail-closed), the five
    /// gates must all be met, and the request shape must validate.
    /// `certificate_der` is the composition-supplied material for the
    /// request's label (a PUBLIC certificate; no key material exists).
    pub fn handle_install_certificate(
        &mut self,
        trust_store: &V2gRootCaTrustStore,
        gates: &PncGateConditions,
        charge_point_id: &str,
        request: &InstallCertificateRequest,
        certificate_der: &[u8],
        now_millis: i64,
    ) -> Result<(), PncCertificateError> {
        gates.require_all_met()?;
        if charge_point_id.trim().is_empty() {
            return Err(PncCertificateError::EmptyField("charge_point_id"));
        }
        request
            .validate()
            .map_err(|_| PncCertificateError::EmptyField("certificate"))?;
        if certificate_der.is_empty() {
            return Err(PncCertificateError::EmptyField("certificate_der"));
        }
        let anchor = match trust_store.find_anchor_by_certificate(certificate_der, now_millis) {
            Ok(anchor) => anchor,
            Err(PncTrustStoreError::CertificateNotAnchored) => {
                return Err(PncCertificateError::InstallCertificateNotAnchored);
            }
            Err(error) => return Err(PncCertificateError::TrustStore(error)),
        };
        self.ledger.record_install_sent(
            charge_point_id,
            request.certificate_type,
            request.certificate.clone(),
            anchor.anchor_label.clone(),
            now_millis,
        )
    }

    /// Records the charge point's response to the InstallCertificate
    /// instruction (Accepted / Rejected with a reason).
    pub fn record_install_certificate_confirmation(
        &mut self,
        charge_point_id: &str,
        certificate_type: CertificateType,
        accepted: bool,
        rejection_reason: String,
    ) -> Result<(), PncCertificateError> {
        self.ledger.record_install_confirmation(
            charge_point_id,
            certificate_type,
            accepted,
            rejection_reason,
        )
    }

    /// The issuance-leg flow, when one exists.
    pub fn flow(
        &self,
        charge_point_id: &str,
        certificate_type: CertificateType,
    ) -> Option<&PncCertificateFlow> {
        self.ledger.flow(charge_point_id, certificate_type)
    }

    /// The install-leg record, when one exists.
    pub fn install_record(
        &self,
        charge_point_id: &str,
        certificate_type: CertificateType,
    ) -> Option<&PncInstallRecord> {
        self.ledger.install_record(charge_point_id, certificate_type)
    }
}

// --- Errors -----------------------------------------------------------------------

/// The named PnC certificate-flow errors. Every refusal names the rule;
/// nothing implies a silent pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PncCertificateError {
    /// A record field is empty. Silence is never a certificate record.
    EmptyField(&'static str),
    /// The flow is not in the state the step requires.
    FlowStateInvalid {
        current: &'static str,
        attempted: &'static str,
    },
    /// A record-41 §3 ungate condition is UNMET (the named condition).
    GateConditionUnmet { condition: PncGateCondition },
    /// The trust store refused (delegated named error).
    TrustStore(PncTrustStoreError),
    /// The certificate to install does not byte-equal any trust-store
    /// anchor: the CSMS never instructs a charge point to trust a
    /// certificate its own store does not record.
    InstallCertificateNotAnchored,
}

impl PncCertificateError {
    /// The stable machine-readable error name.
    pub fn name(&self) -> &'static str {
        match self {
            PncCertificateError::EmptyField(_) => "PNC_FIELD_EMPTY",
            PncCertificateError::FlowStateInvalid { .. } => "PNC_FLOW_STATE_INVALID",
            PncCertificateError::GateConditionUnmet { .. } => "PNC_GATE_CONDITION_UNMET",
            PncCertificateError::TrustStore(_) => "PNC_TRUST_STORE_REFUSED",
            PncCertificateError::InstallCertificateNotAnchored => {
                "PNC_INSTALL_CERTIFICATE_NOT_ANCHORED"
            }
        }
    }

    /// The unmet gate condition, when the refusal is a gate refusal.
    pub fn gate_condition(&self) -> Option<PncGateCondition> {
        match self {
            PncCertificateError::GateConditionUnmet { condition } => Some(*condition),
            _ => None,
        }
    }
}

impl std::fmt::Display for PncCertificateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PncCertificateError::EmptyField(field) => write!(f, "field '{field}' is empty"),
            PncCertificateError::FlowStateInvalid { current, attempted } => {
                write!(f, "certificate flow is in state {current}; the step '{attempted}' is not permitted there")
            }
            PncCertificateError::GateConditionUnmet { condition } => write!(
                f,
                "the record-41 §3 ungate condition '{}' ({}) is UNMET — every PnC flow \
                 refuses while any condition is unmet (the conditions are NOT claimed \
                 met; this is a HUMAN GATE: {})",
                condition.name(),
                condition.record_41_clause(),
                condition.human_gate_statement(),
            ),
            PncCertificateError::TrustStore(error) => write!(f, "the trust store refused: {error}"),
            PncCertificateError::InstallCertificateNotAnchored => write!(
                f,
                "the certificate to install does not byte-equal any trust-store anchor — \
                 the CSMS never instructs a charge point to trust a certificate its own \
                 store does not record"
            ),
        }
    }
}

impl std::error::Error for PncCertificateError {}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: i64 = 1_700_000_000_000;

    fn one_anchor_store() -> V2gRootCaTrustStore {
        let mut store = V2gRootCaTrustStore::empty();
        store
            .add_anchor(
                crate::pnc_trust_store::TrustAnchorRecord::new(
                    "v2g-root-1".to_string(),
                    vec![0x30, 0x03, 0x02, 0x01, 0x07],
                    "pki-operator-reference-1".to_string(),
                    BASE - 1_000,
                    BASE + 100_000,
                    BASE,
                )
                .expect("valid anchor"),
            )
            .expect("added");
        store
    }

    fn met_gates() -> PncGateConditions {
        let mut gates = PncGateConditions::unmet();
        for (index, condition) in PncGateCondition::ALL.into_iter().enumerate() {
            gates
                .record_met(condition, format!("evidence-{index}"))
                .expect("recorded");
        }
        gates
    }

    #[test]
    fn all_five_conditions_default_to_unmet_and_are_refused_by_name() {
        let gates = PncGateConditions::unmet();
        // The first unmet condition in record-41 §3 order is the named one.
        let error = gates.require_all_met().unwrap_err();
        assert_eq!(error.name(), "PNC_GATE_CONDITION_UNMET");
        assert_eq!(
            error.gate_condition().map(PncGateCondition::name),
            Some("PNC_GATE_PKI_OPERATOR_NAMED")
        );
        assert!(error.to_string().contains("record-41 §3.1"));
        assert!(error.to_string().contains("HUMAN GATE"));
        // The standing statement names the not-met discipline.
        assert!(PncGateConditions::ALL_UNMET_BY_DEFAULT_STATEMENT.contains("NOT claimed met"));
        // Recording met without evidence is refused.
        let mut gates = PncGateConditions::unmet();
        assert_eq!(
            gates
                .record_met(PncGateCondition::NamedPkiOperator, "  ".to_string())
                .unwrap_err()
                .name(),
            "PNC_FIELD_EMPTY"
        );
        // One condition met still refuses with the NEXT unmet named.
        gates
            .record_met(PncGateCondition::NamedPkiOperator, "operator-ref".to_string())
            .expect("recorded");
        let error = gates.require_all_met().unwrap_err();
        assert_eq!(
            error.gate_condition().map(PncGateCondition::name),
            Some("PNC_GATE_TRUST_ANCHOR_DISTRIBUTION_AGREED")
        );
    }

    #[test]
    fn the_ev_certificate_relay_refuses_closed_and_records_when_open() {
        let store = one_anchor_store();
        let request = Get15118EVCertificateRequest {
            action: Some("Install".to_string()),
            certificate_schema_label: None,
        };
        // All-unmet gates: refuse, naming the unmet condition.
        let mut surface = PncCertificateSurface::new();
        let error = surface
            .handle_get_15118_ev_certificate(&store, &PncGateConditions::unmet(), "depot-cp-001", &request, BASE)
            .unwrap_err();
        assert!(matches!(
            error,
            PncCertificateError::GateConditionUnmet { .. }
        ));
        // Met gates but an EMPTY trust store: refuse by name.
        let empty_store = V2gRootCaTrustStore::empty();
        let mut surface = PncCertificateSurface::new();
        assert_eq!(
            surface
                .handle_get_15118_ev_certificate(
                    &empty_store,
                    &met_gates(),
                    "depot-cp-001",
                    &request,
                    BASE
                )
                .unwrap_err()
                .name(),
            "PNC_TRUST_STORE_REFUSED"
        );
        // Open gates + anchored store: the relay RECORDS (and routes nothing).
        let mut surface = PncCertificateSurface::new();
        let record = surface
            .handle_get_15118_ev_certificate(&store, &met_gates(), "depot-cp-001", &request, BASE)
            .expect("recorded");
        assert_eq!(record.action, "Get15118EVCertificate");
        assert_eq!(record.outcome.name(), "RECORDED_FOR_PKI_ROUTING");
        assert_eq!(record.certificate_type.name(), "V2gCertificate");
        let flow = surface
            .flow("depot-cp-001", CertificateType::V2gCertificate)
            .expect("flow");
        assert_eq!(flow.state.name(), "REQUESTED");
        // A second open while the flow is open is refused.
        assert!(surface
            .handle_get_15118_ev_certificate(&store, &met_gates(), "depot-cp-001", &request, BASE)
            .is_err());
    }

    #[test]
    fn the_signed_and_install_legs_walk_the_record_03_positions() {
        let store = one_anchor_store();
        let mut surface = PncCertificateSurface::new();
        // CertificateSigned without an open flow is a named state refusal.
        assert_eq!(
            surface
                .handle_certificate_signed(
                    &met_gates(),
                    "depot-cp-001",
                    CertificateType::V2gCertificate,
                    "chain-label-1".to_string(),
                )
                .unwrap_err()
                .name(),
            "PNC_FLOW_STATE_INVALID"
        );
        surface
            .handle_get_15118_ev_certificate(
                &store,
                &met_gates(),
                "depot-cp-001",
                &Get15118EVCertificateRequest { action: None, certificate_schema_label: None },
                BASE,
            )
            .expect("recorded");
        surface
            .handle_certificate_signed(
                &met_gates(),
                "depot-cp-001",
                CertificateType::V2gCertificate,
                "chain-label-1".to_string(),
            )
            .expect("signed recorded");
        assert_eq!(
            surface
                .flow("depot-cp-001", CertificateType::V2gCertificate)
                .expect("flow")
                .state
                .name(),
            "SIGNED_AVAILABLE"
        );
        surface
            .record_certificate_signed_confirmation(
                "depot-cp-001",
                CertificateType::V2gCertificate,
                CertificateSignedStatus::Accepted,
                String::new(),
            )
            .expect("installed");
        assert_eq!(
            surface
                .flow("depot-cp-001", CertificateType::V2gCertificate)
                .expect("flow")
                .state
                .name(),
            "INSTALLED"
        );
        // Rejection path ends the flow REJECTED with a reason.
        surface
            .handle_get_15118_ev_certificate(
                &store,
                &met_gates(),
                "depot-cp-002",
                &Get15118EVCertificateRequest { action: None, certificate_schema_label: None },
                BASE,
            )
            .expect("recorded");
        surface
            .handle_certificate_signed(
                &met_gates(),
                "depot-cp-002",
                CertificateType::V2gCertificate,
                "chain-label-2".to_string(),
            )
            .expect("signed recorded");
        surface
            .record_certificate_signed_confirmation(
                "depot-cp-002",
                CertificateType::V2gCertificate,
                CertificateSignedStatus::Rejected,
                String::new(),
            )
            .expect("recorded");
        match &surface
            .flow("depot-cp-002", CertificateType::V2gCertificate)
            .expect("flow")
            .state
        {
            PncCertificateFlowState::Rejected { reason } => assert!(reason.contains("rejected")),
            other => panic!("expected REJECTED, got {other:?}"),
        }
    }

    #[test]
    fn install_certificate_requires_a_trust_store_byte_equality() {
        let store = one_anchor_store();
        let mut surface = PncCertificateSurface::new();
        let request = InstallCertificateRequest {
            certificate_type: CertificateType::V2gRootCertificate,
            certificate: "root-cert-label".to_string(),
        };
        // A certificate whose bytes match NO anchor is refused by name.
        assert_eq!(
            surface
                .handle_install_certificate(
                    &store,
                    &met_gates(),
                    "depot-cp-001",
                    &request,
                    &[0x30, 0x03, 0x02, 0x01, 0x09],
                    BASE,
                )
                .unwrap_err()
                .name(),
            "PNC_INSTALL_CERTIFICATE_NOT_ANCHORED"
        );
        // The byte-equal certificate records with its anchor label.
        surface
            .handle_install_certificate(
                &store,
                &met_gates(),
                "depot-cp-001",
                &request,
                &[0x30, 0x03, 0x02, 0x01, 0x07],
                BASE,
            )
            .expect("recorded");
        let record = surface
            .install_record("depot-cp-001", CertificateType::V2gRootCertificate)
            .expect("install record");
        assert_eq!(record.anchor_label, "v2g-root-1");
        assert!(record.confirmation.is_none());
        surface
            .record_install_certificate_confirmation(
                "depot-cp-001",
                CertificateType::V2gRootCertificate,
                true,
                String::new(),
            )
            .expect("confirmed");
        // A second confirmation is a named state refusal.
        assert_eq!(
            surface
                .record_install_certificate_confirmation(
                    "depot-cp-001",
                    CertificateType::V2gRootCertificate,
                    true,
                    String::new(),
                )
                .unwrap_err()
                .name(),
            "PNC_FLOW_STATE_INVALID"
        );
    }

    #[test]
    fn message_names_are_record_41_verbatim_and_not_framing_actions() {
        assert_eq!(GET_15118_EV_CERTIFICATE, "Get15118EVCertificate");
        assert_eq!(CERTIFICATE_SIGNED, "CertificateSigned");
        assert_eq!(INSTALL_CERTIFICATE, "InstallCertificate");
        // The closed framing vocabulary is NOT widened: none of these names
        // parses as a framing::Action (record 41 §5.1).
        for name in [GET_15118_EV_CERTIFICATE] {
            assert!(
                crate::framing::Action::parse(name).is_err(),
                "{name} must never become a framing::Action variant (record 41 §5.1)"
            );
        }
    }
}
