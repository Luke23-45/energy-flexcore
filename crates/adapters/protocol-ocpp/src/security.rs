// P17.03 — OCPP security Profile 3 onboarding: per-device client
// certificates, the certificate flows, identity proofing at registration,
// and SecurityEventNotification ingestion.
//
// Decision record:
// `docs/phases-11-20/phase-17-ev-fleet-charging/records/03_p17_03_security_profile_3_onboarding.md`.
//
// Recorded rules (phase-17 context §3; conventions §4-§5; the P14.03
// interplay):
//
// - **Profile 3 is the onboarding target** [S-126][S-127]: TLS with
//   client-side certificates. Profiles 1 (unsecured transport) and 2 (TLS
//   with HTTP basic auth) are RECORDED, never accepted for new
//   onboarding — the policy is structural ([`OnboardingPolicy`]).
// - **NO CA is stood up anywhere in this program** (P14.03 interplay):
//   certificate flows carry LABELS/records only. There is no field in
//   this module that could hold key or certificate MATERIAL; issuance is
//   a deployment PKI-operations concern (a human/ops gate), and
//   SignCertificate REQUESTS are recorded as REQUESTS, never fulfilled.
// - **Identity proofing at registration** [S-126][S-133]: a charge point
//   registers only with a proofing record binding its identity to a
//   validated client certificate against a declared trust anchor; an
//   unproofed registration is refused (fail-closed).
// - **Security events are records, never platform judgments.** A
//   SecurityEventNotification is ingested as an EVENT-LOG record; an
//   unknown event type is classified `Unclassified` (explicitly named —
//   never guessed as benign, never guessed as critical). No automatic
//   revocation, session teardown, or other reaction is triggered by an
//   event record here; reactions are operator/policy decisions.
//
// Module-level UNVERIFIED register: the security block spellings
// (SignCertificate / CertificateSigned / InstallCertificate /
// SecurityEventNotification, the certificate-type strings, and the
// security-event type names) are secondary-sourced
// ([S-126][S-127][S-133]); the OCA Security Operations Guide [S-127] and
// the OCPP 2.0.1/2.1 security sections are the implementation-time checks.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::model::{
    CertificateType, InstallCertificateRequest, SecurityEventNotificationRequest,
    SignCertificateRequest,
};

/// The OCPP transport security profiles [S-126] (UNVERIFIED numbering
/// semantics — the check is the OCA security documentation).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SecurityProfile {
    /// Profile 1: unsecured transport (recorded, never accepted for new
    /// onboarding).
    Profile1,
    /// Profile 2: TLS with HTTP basic authentication (recorded, never
    /// accepted for new onboarding).
    Profile2,
    /// Profile 3: TLS with client-side certificates — the onboarding
    /// target.
    Profile3,
}

impl SecurityProfile {
    /// Canonical name for records.
    pub const fn name(self) -> &'static str {
        match self {
            SecurityProfile::Profile1 => "security-profile-1",
            SecurityProfile::Profile2 => "security-profile-2",
            SecurityProfile::Profile3 => "security-profile-3",
        }
    }
}

/// The onboarding policy (structural: Profile 3 only). There is no
/// constructor that could accept Profile 1/2 for new onboarding —
/// [`OnboardingPolicy::accepts_for_onboarding`] answers for the recorded
/// profiles honestly (Profile 1/2 are RECORDED as legacy interop facts,
/// never accepted).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct OnboardingPolicy;

impl OnboardingPolicy {
    /// Whether the profile is acceptable for NEW onboarding: Profile 3
    /// only.
    pub const fn accepts_for_onboarding(&self, profile: SecurityProfile) -> bool {
        matches!(profile, SecurityProfile::Profile3)
    }

    /// The recorded policy statement.
    pub const fn statement(&self) -> &'static str {
        "Profile 3 (TLS with client certificates) is the onboarding target \
         [S-126][S-127]; Profiles 1 and 2 are recorded legacy interop facts \
         and are NEVER accepted for new onboarding. Per-device client \
         certificates are validated against declared trust anchors; NO CA \
         is stood up in this program (P14.03 interplay) — issuance belongs \
         to the deployment's PKI operations."
    }
}

/// Security errors (named; every refusal names the rule).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SecurityError {
    /// A record field is empty. Silence is never a certificate record.
    EmptyField(&'static str),
    /// The charge point has no identity-proofing record; registration is
    /// refused (fail-closed).
    NotProofed { charge_point: String },
    /// The proofing record does not bind THIS charge point's identity.
    ProofingMismatch {
        registered: String,
        proofed: String,
    },
    /// The certificate flow is not in the state the step requires.
    FlowStateInvalid {
        current: &'static str,
        attempted: &'static str,
    },
    /// The security-event type is empty (an unnamed event is refused —
    /// see the ingestion rules for the classification path).
    EmptySecurityEventType,
}

impl std::fmt::Display for SecurityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SecurityError::EmptyField(field) => write!(f, "field '{field}' is empty"),
            SecurityError::NotProofed { charge_point } => {
                write!(f, "charge point '{charge_point}' has no identity-proofing record; registration is refused (fail-closed)")
            }
            SecurityError::ProofingMismatch {
                registered,
                proofed,
            } => {
                write!(f, "the proofing record binds '{proofed}', not the registering '{registered}'")
            }
            SecurityError::FlowStateInvalid {
                current,
                attempted,
            } => {
                write!(f, "certificate flow is in state {current}; the step '{attempted}' is not permitted there")
            }
            SecurityError::EmptySecurityEventType => {
                write!(f, "the security-event type is empty; an unnamed event is refused")
            }
        }
    }
}

impl std::error::Error for SecurityError {}

/// One per-device client-certificate record (LABELS only — no material;
/// the material behind every label is a deployment PKI-operations
/// artifact).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceCertificateRecord {
    /// The charge point the certificate belongs to.
    pub charge_point_id: String,
    /// The certificate type (UNVERIFIED vocabulary — module register).
    pub certificate_type: CertificateType,
    /// The certificate LABEL (the deployment's handle to the material).
    pub certificate_label: String,
    /// The trust-anchor label the certificate chains to (declared; the
    /// anchor material is the deployment's).
    pub trust_anchor_label: String,
    /// The recorded instant in UTC milliseconds (the injected clock).
    pub recorded_millis: i64,
}

impl DeviceCertificateRecord {
    /// Validates: every label non-empty.
    pub fn new(
        charge_point_id: String,
        certificate_type: CertificateType,
        certificate_label: String,
        trust_anchor_label: String,
        recorded_millis: i64,
    ) -> Result<Self, SecurityError> {
        if charge_point_id.trim().is_empty() {
            return Err(SecurityError::EmptyField("charge_point_id"));
        }
        if certificate_label.trim().is_empty() {
            return Err(SecurityError::EmptyField("certificate_label"));
        }
        if trust_anchor_label.trim().is_empty() {
            return Err(SecurityError::EmptyField("trust_anchor_label"));
        }
        Ok(Self {
            charge_point_id,
            certificate_type,
            certificate_label,
            trust_anchor_label,
            recorded_millis,
        })
    }
}

/// The identity-proofing record at registration [S-126][S-133]: the
/// binding of a charge-point identity to a validated client certificate
/// against a declared trust anchor. Registration without this record is
/// refused ([`IdentityProofingLedger::require_proofed`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentityProofingRecord {
    /// The proofed charge-point identity.
    pub charge_point_id: String,
    /// The client-certificate label that was validated.
    pub certificate_label: String,
    /// The trust-anchor label used for validation.
    pub trust_anchor_label: String,
    /// Whether revocation was checked (fail-closed DEFAULT true).
    pub revocation_checked: bool,
    /// The proofing instant in UTC milliseconds (the injected clock).
    pub proofed_millis: i64,
}

/// The identity-proofing ledger (deterministic `BTreeMap` keyed by
/// charge-point identity). The certificate VALIDATION itself is the
/// transport seam's TLS layer + the deployment's trust anchors; this
/// ledger records the OUTCOME (the proofing decision), never the
/// material.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IdentityProofingLedger {
    records: BTreeMap<String, IdentityProofingRecord>,
}

impl IdentityProofingLedger {
    /// An empty ledger.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a successful proofing outcome (the transport seam calls
    /// this after ITS validation succeeded — the ledger never fabricates
    /// a proofing).
    pub fn record(&mut self, record: IdentityProofingRecord) -> Result<(), SecurityError> {
        if record.charge_point_id.trim().is_empty() {
            return Err(SecurityError::EmptyField("charge_point_id"));
        }
        if record.certificate_label.trim().is_empty() {
            return Err(SecurityError::EmptyField("certificate_label"));
        }
        if record.trust_anchor_label.trim().is_empty() {
            return Err(SecurityError::EmptyField("trust_anchor_label"));
        }
        self.records
            .insert(record.charge_point_id.clone(), record);
        Ok(())
    }

    /// The proofing record, when one exists.
    pub fn proofing(&self, charge_point_id: &str) -> Option<&IdentityProofingRecord> {
        self.records.get(charge_point_id)
    }

    /// Fail-closed registration gate: refuses a charge point without a
    /// proofing record (the named [`SecurityError::NotProofed`]).
    pub fn require_proofed(&self, charge_point_id: &str) -> Result<&IdentityProofingRecord, SecurityError> {
        self.records.get(charge_point_id).ok_or_else(|| SecurityError::NotProofed {
            charge_point: charge_point_id.to_string(),
        })
    }
}

/// The certificate-flow state machine (one flow per charge point per
/// certificate type). The states model the recorded PROTOCOL positions;
/// the ISSUANCE itself (between Requested and SignedAvailable) is the
/// deployment's PKI operations — a human/ops gate, never this module's.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CertificateFlowState {
    /// A SignCertificate REQUEST was received and recorded (the CSR is a
    /// label; nothing was issued).
    Requested {
        csr_label: String,
    },
    /// The PKI operations produced a signed certificate and the
    /// CertificateSigned REQUEST was sent to the charge point.
    SignedAvailable {
        certificate_label: String,
    },
    /// The charge point confirmed installation (its Accepted response was
    /// recorded).
    Installed {
        certificate_label: String,
    },
    /// The flow ended in a named rejection (charge point rejected, PKI
    /// refused, or a timeout — the reason is human-safe).
    Rejected {
        reason: String,
    },
}

impl CertificateFlowState {
    /// Canonical name for records.
    pub const fn name(&self) -> &'static str {
        match self {
            CertificateFlowState::Requested { .. } => "REQUESTED",
            CertificateFlowState::SignedAvailable { .. } => "SIGNED_AVAILABLE",
            CertificateFlowState::Installed { .. } => "INSTALLED",
            CertificateFlowState::Rejected { .. } => "REJECTED",
        }
    }
}

/// One certificate flow (per charge point + certificate type).
#[derive(Debug, Clone)]
pub struct CertificateFlow {
    /// The charge point.
    pub charge_point_id: String,
    /// The certificate type the flow targets.
    pub certificate_type: CertificateType,
    /// The flow state.
    pub state: CertificateFlowState,
}

/// The certificate-flow ledger (deterministic, keyed by
/// `"<charge_point>#<certificate_type>"`).
#[derive(Debug, Clone, Default)]
pub struct CertificateFlowLedger {
    flows: BTreeMap<String, CertificateFlow>,
}

impl CertificateFlowLedger {
    /// An empty ledger.
    pub fn new() -> Self {
        Self::default()
    }

    fn key(charge_point_id: &str, certificate_type: CertificateType) -> String {
        format!("{charge_point_id}#{}", certificate_type.name())
    }

    /// Records a SignCertificate REQUEST (the flow opens in REQUESTED).
    /// The CSR is a LABEL; nothing is issued. Overwrites only a
    /// terminal (Rejected) previous flow for the same target — an open
    /// flow is refused (one flow per target at a time).
    pub fn open_requested(
        &mut self,
        charge_point_id: &str,
        request: &SignCertificateRequest,
    ) -> Result<(), SecurityError> {
        charge_point_id_nonempty(charge_point_id)?;
        request.validate().map_err(|_| SecurityError::EmptyField("csr"))?;
        let key = Self::key(charge_point_id, request.type_of_certificate.unwrap_or(CertificateType::CsmsCertificate));
        if let Some(existing) = self.flows.get(&key) {
            if !matches!(existing.state, CertificateFlowState::Rejected { .. }) {
                return Err(SecurityError::FlowStateInvalid {
                    current: existing.state.name(),
                    attempted: "open_requested",
                });
            }
        }
        let certificate_type = request
            .type_of_certificate
            .unwrap_or(CertificateType::CsmsCertificate);
        self.flows.insert(
            key,
            CertificateFlow {
                charge_point_id: charge_point_id.to_string(),
                certificate_type,
                state: CertificateFlowState::Requested {
                    csr_label: request.csr.clone(),
                },
            },
        );
        Ok(())
    }

    /// Records that the PKI operations produced a signed certificate and
    /// the CertificateSigned REQUEST was sent (the transition REQUESTED →
    /// SIGNED_AVAILABLE). The issuance act is EXTERNAL (a human/ops gate
    /// in the deployment's PKI operations) — this method records the
    /// outcome, it does not perform the issuance.
    pub fn record_signed_sent(
        &mut self,
        charge_point_id: &str,
        certificate_type: CertificateType,
        certificate_label: String,
    ) -> Result<(), SecurityError> {
        if certificate_label.trim().is_empty() {
            return Err(SecurityError::EmptyField("certificate_label"));
        }
        let key = Self::key(charge_point_id, certificate_type);
        let flow = self.flows.get_mut(&key).ok_or(SecurityError::FlowStateInvalid {
            current: "ABSENT",
            attempted: "record_signed_sent",
        })?;
        match &flow.state {
            CertificateFlowState::Requested { .. } => {
                flow.state = CertificateFlowState::SignedAvailable { certificate_label };
                Ok(())
            }
            other => Err(SecurityError::FlowStateInvalid {
                current: other.name(),
                attempted: "record_signed_sent",
            }),
        }
    }

    /// Records the charge point's installation confirmation (the
    /// transition SIGNED_AVAILABLE → INSTALLED). `accepted` carries the
    /// charge point's own response; a rejection ends the flow as Rejected.
    pub fn record_install_confirmation(
        &mut self,
        charge_point_id: &str,
        certificate_type: CertificateType,
        accepted: bool,
        rejection_reason: String,
    ) -> Result<(), SecurityError> {
        let key = Self::key(charge_point_id, certificate_type);
        let flow = self.flows.get_mut(&key).ok_or(SecurityError::FlowStateInvalid {
            current: "ABSENT",
            attempted: "record_install_confirmation",
        })?;
        match &flow.state {
            CertificateFlowState::SignedAvailable { certificate_label } => {
                let label = certificate_label.clone();
                flow.state = if accepted {
                    CertificateFlowState::Installed {
                        certificate_label: label,
                    }
                } else {
                    CertificateFlowState::Rejected {
                        reason: if rejection_reason.trim().is_empty() {
                            "the charge point rejected the signed certificate".to_string()
                        } else {
                            rejection_reason
                        },
                    }
                };
                Ok(())
            }
            other => Err(SecurityError::FlowStateInvalid {
                current: other.name(),
                attempted: "record_install_confirmation",
            }),
        }
    }

    /// The flow, when one exists.
    pub fn flow(
        &self,
        charge_point_id: &str,
        certificate_type: CertificateType,
    ) -> Option<&CertificateFlow> {
        self.flows.get(&Self::key(charge_point_id, certificate_type))
    }
}

fn charge_point_id_nonempty(charge_point_id: &str) -> Result<(), SecurityError> {
    if charge_point_id.trim().is_empty() {
        Err(SecurityError::EmptyField("charge_point_id"))
    } else {
        Ok(())
    }
}

/// The security-event severity classification (the declared table; the
/// event-type names are UNVERIFIED — module register; the OCA Security
/// Operations Guide [S-127] is the implementation-time check). An unknown
/// type classifies as `Unclassified` — explicitly named, never guessed as
/// benign, never guessed as critical.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecurityEventSeverity {
    Critical,
    Notable,
    /// The reported type is outside the declared table: explicitly
    /// unclassified (never guessed either way).
    Unclassified,
}

impl SecurityEventSeverity {
    /// Canonical name for records.
    pub const fn name(self) -> &'static str {
        match self {
            SecurityEventSeverity::Critical => "CRITICAL",
            SecurityEventSeverity::Notable => "NOTABLE",
            SecurityEventSeverity::Unclassified => "UNCLASSIFIED",
        }
    }
}

/// The declared classification table (UNVERIFIED names — module register;
/// the check names the security-event section of the OCA text). The set
/// is deliberately SMALL: a wrong criticality is worse than an honest
/// Unclassified.
pub fn classify_security_event(reported_type: &str) -> SecurityEventSeverity {
    match reported_type {
        // Critical-class names (UNVERIFIED spellings; the check is the
        // OCA text's security-event list).
        "InvalidCSMSCertificate" | "InvalidChargePointCertificate" | "RevokedCertificate"
        | "FailedAuthenticationAttempt" => SecurityEventSeverity::Critical,
        // Notable-class names (UNVERIFIED spellings).
        "ConfigChange" | "FirmwareUpdate" | "MasterKeySet" => SecurityEventSeverity::Notable,
        // Everything else: explicitly unclassified.
        _ => SecurityEventSeverity::Unclassified,
    }
}

/// One ingested security-event record (the EVENT-LOG resource class of
/// the adapter's scope; a RECORD — no automatic reaction is triggered by
/// ingestion; reactions are operator/policy decisions).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecurityEventRecord {
    /// The charge point that reported the event.
    pub charge_point_id: String,
    /// The reported type (carried verbatim).
    pub reported_type: String,
    /// The classified severity (Unclassified for unknown types — never
    /// guessed).
    pub severity: SecurityEventSeverity,
    /// The human-safe technical info label, when reported (carried
    /// verbatim, never parsed).
    pub tech_info: Option<String>,
    /// The event timestamp the station reported (UTC milliseconds).
    pub event_millis: i64,
    /// The receive instant (the injected clock; UTC milliseconds).
    pub received_millis: i64,
}

/// The security-event ledger (deterministic, bounded).
#[derive(Debug, Clone, Default)]
pub struct SecurityEventLedger {
    events: Vec<SecurityEventRecord>,
    cap: usize,
}

impl SecurityEventLedger {
    /// A ledger with a declared cap (non-zero; a zero cap is a
    /// construction error).
    pub fn new(cap: usize) -> Result<Self, SecurityError> {
        if cap == 0 {
            return Err(SecurityError::EmptyField("cap"));
        }
        Ok(Self {
            events: Vec::new(),
            cap,
        })
    }

    /// Ingests one SecurityEventNotification. An empty event type is
    /// refused (an unnamed event cannot be classified or audited); an
    /// unknown type is recorded as Unclassified. Ingestion is a RECORD —
    /// nothing reacts automatically.
    pub fn ingest(
        &mut self,
        charge_point_id: &str,
        request: &SecurityEventNotificationRequest,
        received_millis: i64,
    ) -> Result<SecurityEventRecord, SecurityError> {
        charge_point_id_nonempty(charge_point_id)?;
        request
            .validate()
            .map_err(|_| SecurityError::EmptySecurityEventType)?;
        let record = SecurityEventRecord {
            charge_point_id: charge_point_id.to_string(),
            reported_type: request.event_type.clone(),
            severity: classify_security_event(&request.event_type),
            tech_info: request.tech_info.clone(),
            event_millis: request.timestamp_millis,
            received_millis,
        };
        self.events.push(record.clone());
        // Bounded: the oldest entries drop beyond the cap (the bounded-
        // queue pattern; the drop is the declared policy, not a silent
        // loss — the cap is declared at construction).
        while self.events.len() > self.cap {
            self.events.remove(0);
        }
        Ok(record)
    }

    /// The ledger snapshot (oldest first).
    pub fn snapshot(&self) -> &[SecurityEventRecord] {
        &self.events
    }

    /// The declared cap.
    pub const fn cap(&self) -> usize {
        self.cap
    }
}

/// Convenience: validates an outgoing InstallCertificate request (the
/// CSMS → charge-point flow) before the transport encodes it — the label
/// must be non-empty (the model validation re-used).
pub fn validate_install_request(
    request: &InstallCertificateRequest,
) -> Result<(), SecurityError> {
    request
        .validate()
        .map_err(|_| SecurityError::EmptyField("certificate"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn onboarding_policy_is_structurally_profile_3() {
        let policy = OnboardingPolicy;
        assert!(policy.accepts_for_onboarding(SecurityProfile::Profile3));
        assert!(!policy.accepts_for_onboarding(SecurityProfile::Profile1));
        assert!(!policy.accepts_for_onboarding(SecurityProfile::Profile2));
        assert!(policy.statement().contains("NEVER accepted"));
        assert!(policy.statement().contains("NO CA is stood up"));
    }

    #[test]
    fn proofing_ledger_is_fail_closed() {
        let mut ledger = IdentityProofingLedger::new();
        // An unproofed registration is refused, named.
        let error = ledger
            .require_proofed("depot-cp-001")
            .expect_err("refused");
        assert!(matches!(error, SecurityError::NotProofed { .. }));
        assert!(error.to_string().contains("fail-closed"));

        // A proofing record with an empty label is refused.
        let bad = IdentityProofingRecord {
            charge_point_id: "depot-cp-001".to_string(),
            certificate_label: String::new(),
            trust_anchor_label: "anchor".to_string(),
            revocation_checked: true,
            proofed_millis: 1,
        };
        assert!(ledger.record(bad).is_err());

        // A valid record gates registration.
        ledger
            .record(IdentityProofingRecord {
                charge_point_id: "depot-cp-001".to_string(),
                certificate_label: "cert-label-1".to_string(),
                trust_anchor_label: "evse-device-ca".to_string(),
                revocation_checked: true,
                proofed_millis: 100,
            })
            .expect("recorded");
        let proofed = ledger.require_proofed("depot-cp-001").expect("proofed");
        assert_eq!(proofed.certificate_label, "cert-label-1");
        assert!(proofed.revocation_checked);
    }

    #[test]
    fn certificate_flow_walks_requested_to_installed_with_named_refusals() {
        let mut ledger = CertificateFlowLedger::new();
        let request = SignCertificateRequest {
            csr: "csr-label".to_string(),
            type_of_certificate: Some(CertificateType::CsmsCertificate),
        };
        ledger
            .open_requested("depot-cp-001", &request)
            .expect("opened");
        let flow = ledger
            .flow("depot-cp-001", CertificateType::CsmsCertificate)
            .expect("flow");
        assert_eq!(flow.state.name(), "REQUESTED");

        // The signed-send step requires the REQUESTED state.
        let error = ledger
            .record_signed_sent("ghost", CertificateType::CsmsCertificate, "cert".to_string())
            .expect_err("no flow");
        assert!(error.to_string().contains("ABSENT"));
        // A second open while the flow is open is refused (one flow per
        // target).
        assert!(ledger.open_requested("depot-cp-001", &request).is_err());

        ledger
            .record_signed_sent(
                "depot-cp-001",
                CertificateType::CsmsCertificate,
                "signed-label".to_string(),
            )
            .expect("signed");
        assert_eq!(
            ledger
                .flow("depot-cp-001", CertificateType::CsmsCertificate)
                .expect("flow")
                .state
                .name(),
            "SIGNED_AVAILABLE"
        );
        // The install confirmation closes the flow.
        ledger
            .record_install_confirmation(
                "depot-cp-001",
                CertificateType::CsmsCertificate,
                true,
                String::new(),
            )
            .expect("installed");
        let flow = ledger
            .flow("depot-cp-001", CertificateType::CsmsCertificate)
            .expect("flow");
        assert_eq!(flow.state.name(), "INSTALLED");

        // A rejected confirmation ends the flow as Rejected with a reason.
        ledger
            .open_requested("depot-cp-002", &request)
            .expect("opened");
        ledger
            .record_signed_sent("depot-cp-002", CertificateType::CsmsCertificate, "c2".to_string())
            .expect("signed");
        ledger
            .record_install_confirmation(
                "depot-cp-002",
                CertificateType::CsmsCertificate,
                false,
                String::new(),
            )
            .expect("recorded");
        let flow = ledger
            .flow("depot-cp-002", CertificateType::CsmsCertificate)
            .expect("flow");
        match &flow.state {
            CertificateFlowState::Rejected { reason } => {
                assert!(reason.contains("rejected"));
            }
            other => panic!("expected rejected, got {other:?}"),
        }
        // A rejected flow may be re-opened (the retry path).
        ledger
            .open_requested("depot-cp-002", &request)
            .expect("re-opened after rejection");
    }

    #[test]
    fn security_events_ingest_with_declared_classification() {
        let mut ledger = SecurityEventLedger::new(4).expect("valid cap");
        assert!(SecurityEventLedger::new(0).is_err());

        let event = |event_type: &str| SecurityEventNotificationRequest {
            timestamp_millis: 100,
            event_type: event_type.to_string(),
            tech_info: None,
        };
        // Declared names classify (UNVERIFIED spellings — module register).
        assert_eq!(
            classify_security_event("InvalidCSMSCertificate"),
            SecurityEventSeverity::Critical
        );
        assert_eq!(
            classify_security_event("ConfigChange"),
            SecurityEventSeverity::Notable
        );
        // Unknown names are Unclassified — never guessed.
        assert_eq!(
            classify_security_event("SomeVendorEvent"),
            SecurityEventSeverity::Unclassified
        );
        assert_eq!(SecurityEventSeverity::Unclassified.name(), "UNCLASSIFIED");

        let record = ledger
            .ingest("depot-cp-001", &event("InvalidChargePointCertificate"), 110)
            .expect("ingested");
        assert_eq!(record.severity, SecurityEventSeverity::Critical);
        ledger
            .ingest("depot-cp-001", &event("ConfigChange"), 120)
            .expect("ingested");
        ledger
            .ingest("depot-cp-001", &event("VendorSpecificThing"), 130)
            .expect("ingested");
        assert_eq!(ledger.snapshot().len(), 3);
        // An empty event type is refused.
        assert!(ledger.ingest("depot-cp-001", &event("   "), 140).is_err());
        // The cap drops the oldest (bounded ledger).
        ledger
            .ingest("depot-cp-001", &event("MasterKeySet"), 150)
            .expect("ingested");
        ledger
            .ingest("depot-cp-001", &event("FirmwareUpdate"), 160)
            .expect("ingested");
        assert_eq!(ledger.snapshot().len(), 4, "cap enforced");
        assert_eq!(
            ledger.snapshot().first().expect("oldest").reported_type,
            "ConfigChange",
            "the oldest (InvalidChargePointCertificate) was dropped; ConfigChange is now oldest"
        );
        assert_eq!(ledger.cap(), 4);
    }

    #[test]
    fn certificate_records_carry_labels_never_material() {
        let record = DeviceCertificateRecord::new(
            "depot-cp-001".to_string(),
            CertificateType::V2gRootCertificate,
            "cert-label".to_string(),
            "anchor-label".to_string(),
            1,
        )
        .expect("valid");
        assert_eq!(record.certificate_type.name(), "V2GRootCertificate");
        assert!(DeviceCertificateRecord::new(
            "depot-cp-001".to_string(),
            CertificateType::CsmsCertificate,
            "  ".to_string(),
            "anchor".to_string(),
            1,
        )
        .is_err());
        // The install-request validation re-uses the model rule.
        let install = InstallCertificateRequest {
            certificate_type: CertificateType::V2gRootCertificate,
            certificate: "label".to_string(),
        };
        validate_install_request(&install).expect("valid");
        let empty = InstallCertificateRequest {
            certificate_type: CertificateType::V2gRootCertificate,
            certificate: " ".to_string(),
        };
        assert!(validate_install_request(&empty).is_err());
    }
}
