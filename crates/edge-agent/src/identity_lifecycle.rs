// P18.08 — Device identity LIFECYCLE FORMALIZATION: the IDevID/LDevID
//          model mapped onto the P9.33 enrollment ceremony, the credential
//          rotation schedule, and the revocation-propagation design
//          (phase-18 plan P18.08; sources [S-165] FDO, [S-166] IETF T2TRG
//          security-setup draft).
//
// THE MODEL (IEEE 802.1AR shape, as surveyed by the T2TRG draft [S-166] —
// UNVERIFIED against the 802.1AR standard text itself; the survey is the
// only secondary source this program holds):
//
// - **IDevID** (initial device identity): installed at the FACTORY, bound
//   to the device hardware, issued by the manufacturer — NOT by FlexCore.
//   FlexCore never generates, issues, or replaces an IDevID. It is
//   CONSUMED as (a) the anchor a zero-touch claim verifies a
//   factory-provisioned device against (P18.09, `crate::zero_touch`) and
//   (b) candidate attestation evidence anchor (P18.13,
//   `crate::attestation`), both subject to the P18-E-2 hardware posture
//   (PENDING PILOT HARDWARE SPEC, P6.34).
// - **LDevID** (locally-issued operational identity): issued by the
//   OPERATOR'S authority for the deployment. This is EXACTLY what the
//   P9.33 ceremony + P6.06 issuance seam already produce: the ceremony's
//   Approved → Active transition binds a `DeviceCredentialRecord` — the
//   LDevID term. No second issuance path exists and none is added here.
//
// THE MAPPING IS THE DELIVERABLE (record + this module): the P9.33
// ceremony states map onto the two-certificate model as
// `IdentityTermMap` below. The ceremony is NOT extended: every
// transition, refusal, and register discipline of
// `crate::enrollment_ceremony` is consumed verbatim. What phase 18 adds
// is (1) the CLASS of each term, (2) a rotation schedule that formalizes
// when a ceremony re-run is DUE (the P6.06 `CredentialExpiryMonitor`
// already surfaces expiry; a scheduled re-enrollment is a ceremony
// opened by the platform authority with a fresh single-use token —
// exactly the normal `begin_request` discipline), and (3) the
// revocation-propagation DESIGN (below; the runbook is the P18.08
// decision record).
//
// ROTATION SCHEDULE (constants are DEFAULT-RECOMMENDATIONS, recorded in
// the decision record — not sourced figures; all values are per-deployment
// configuration, pre-registered (X-8) before any run that could fill
// them):
// - A rotation is DUE when the active LDevID term enters the rotation
//   window before its recorded expiry (P6.06's warning window) — the
//   rotation is then a NORMAL ceremony re-run against a fresh
//   platform-authority token (single-use, short-lived; ledger P9.33 step
//   2 applies identically — there is no second token path).
// - A term with NO recorded expiry is never assumed fine: the
//   assessment is [`RotationAssessment::NoExpiryRecorded`] (the P6.06
//   `ExpiryStatus::NoExpiryRecorded` honesty rule carries over) and the
//   recommendation is an operator-reviewed re-issue, not a silent pass.
//
// REVOCATION PROPAGATION (the kernel interplay, read-only, never changed):
// the KERNEL's `crates/flexcore-kernel/src/revocation.rs` register is
// PLUGIN-scoped — keyed by (plugin id, version) and signer key REFERENCE,
// append-only, NO un-revoke — and stays BYTE-IDENTICAL (phase-18 ban 8;
// context constraint 2). Device-identity revocation is a DIFFERENT
// register family: the 0018 `edge_device_registry` REVOKED term row + the
// 0018 `edge_enrollment_journal` REVOKED event, propagated to the closed
// target set in [`RevocationTarget`] (registry → protocol trust chains →
// channel gate → update-authorization gate). The two registers never
// write each other; the only interplay is CONSUMPTION at gates.
//
// HONESTY BOUNDS: no device exists; no identity term is active; no
// certificate was ever issued or revoked; every flow here is design plus
// test doubles (phase-18 constraint 6). No CA is stood up (plan §4
// non-goals); issuance/rotation/revocation EXECUTION remains human PKI
// operations + the P14.03 operations design. Every 802.1AR/FDO-specific
// claim stays UNVERIFIED [S-165][S-166].

use std::collections::BTreeMap;

use crate::enrollment::DeviceCredentialRecord;

// --- The identity-class vocabulary --------------------------------------------------

/// The closed device-identity-class vocabulary (the 802.1AR shape as
/// surveyed by [S-166] — UNVERIFIED against the standard text).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DeviceIdentityClass {
    /// The factory-installed initial identity. Manufacturer-issued; never
    /// generated, issued, rotated, or replaced by FlexCore. Consumed only
    /// as evidence/verification input (P18.09 claim verification, P18.13
    /// attestation anchor — both subject to P18-E-2).
    IDevID,
    /// The locally-issued operational identity: exactly the credential
    /// term the P9.33 ceremony binds at Approved → Active.
    LDevID,
}

impl DeviceIdentityClass {
    /// The stable audit/vocabulary name.
    pub fn name(self) -> &'static str {
        match self {
            DeviceIdentityClass::IDevID => "IDEVID",
            DeviceIdentityClass::LDevID => "LDEVID",
        }
    }
}

/// The IDevID/LDevID mapping onto the P9.33 ceremony (the formalization
/// deliverable). The ceremony vocabulary is CONSUMED, never extended:
/// this type states which identity class each ceremony surface carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IdentityTermMap;

impl IdentityTermMap {
    /// The class of the identity the ceremony itself enrolls, rotates,
    /// and revokes: the LDevID. (The IDevID is never a ceremony subject —
    /// the ceremony has no transition that touches factory material.)
    pub const CEREMONY_SUBJECT: DeviceIdentityClass = DeviceIdentityClass::LDevID;

    /// The class of the identity a zero-touch claim verifies against the
    /// factory voucher (P18.09): the IDevID.
    pub const ZERO_TOUCH_VERIFICATION_ANCHOR: DeviceIdentityClass = DeviceIdentityClass::IDevID;

    /// The class of the identity the attestation evidence MAY reference
    /// as its anchor (P18.13) — subject to P18-E-2: without hardware the
    /// evidence is software-only and names NO anchor at all.
    pub const ATTESTATION_ANCHOR_CANDIDATE: DeviceIdentityClass = DeviceIdentityClass::IDevID;

    /// The recorded mapping statement (quoted by the P18.08 record).
    pub const MAPPING_STATEMENT: &'static str = "the P9.33 ceremony enrolls, rotates, and \
         revokes the LDevID (the locally-issued operational term — the DeviceCredentialRecord \
         bound at Approved-to-Active). The IDevID (factory-installed, manufacturer-issued) is \
         never a ceremony subject: FlexCore never generates, issues, or replaces one; it is \
         consumed only as verification/attestation evidence input, subject to P18-E-2 \
         (PENDING PILOT HARDWARE SPEC). [S-166] survey basis; UNVERIFIED against 802.1AR text.";
}

// --- The rotation schedule ------------------------------------------------------------

/// The rotation-schedule DEFAULTS (DEFAULT-RECOMMENDATIONS recorded in the
/// P18.08 decision record — not sourced figures; per-deployment
/// configuration overrides them, pre-registered (X-8) before any run).
pub const DEFAULT_ROTATION_WINDOW_MILLIS: i64 = 30 * 24 * 60 * 60_000;
pub const DEFAULT_MAX_TERM_LIFETIME_MILLIS: i64 = 90 * 24 * 60 * 60_000;

/// The closed rotation-assessment vocabulary for one active LDevID term
/// against the INJECTED clock (never wall time).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RotationAssessment {
    /// The term is inside its lifetime and outside the rotation window.
    Current,
    /// The term is inside the rotation window: a ceremony re-run is DUE
    /// (the platform authority issues a fresh single-use token; the
    /// normal `begin_request` discipline applies unchanged).
    RotateDue,
    /// The recorded expiry has passed against the injected clock: the
    /// term is expired (the P6.06 `CredentialExpired` rule — the channel
    /// must not be brought up on it; rotation is overdue).
    Expired,
    /// The term records NO expiry (the shipped seam's honest `None`):
    /// never assumed fine — an operator-reviewed re-issue is the
    /// recommendation, and every audit line says `NoExpiryRecorded`.
    NoExpiryRecorded,
    /// The term's lifetime EXCEEDS the configured maximum: the term is
    /// refused as excessive at assessment time (a standing long-lived
    /// credential is the ledger's named risk, mirrored from the token
    /// lifetime ceiling discipline).
    LifetimeExcessive,
}

impl RotationAssessment {
    /// The stable audit/vocabulary name.
    pub fn name(self) -> &'static str {
        match self {
            RotationAssessment::Current => "ROTATION_CURRENT",
            RotationAssessment::RotateDue => "ROTATION_DUE",
            RotationAssessment::Expired => "ROTATION_EXPIRED",
            RotationAssessment::NoExpiryRecorded => "ROTATION_NO_EXPIRY_RECORDED",
            RotationAssessment::LifetimeExcessive => "ROTATION_LIFETIME_EXCESSIVE",
        }
    }
}

/// The credential rotation schedule: the configuration plus the pure
/// assessment over one `DeviceCredentialRecord` at an injected instant.
/// The schedule computes WHEN a ceremony re-run is due; it never runs the
/// ceremony, issues anything, or touches the credential store.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CredentialRotationSchedule {
    /// The rotation window before expiry within which a re-run is DUE.
    pub rotation_window_millis: i64,
    /// The maximum lifetime an LDevID term may record. `None` = the
    /// deployment sets no ceiling (recorded honestly; the DEFAULT is
    /// [`DEFAULT_MAX_TERM_LIFETIME_MILLIS`]).
    pub max_term_lifetime_millis: Option<i64>,
}

impl CredentialRotationSchedule {
    /// The DEFAULT schedule (the recorded recommendation; the P18.08
    /// decision record owns the rationale and the X-8 pre-registration
    /// note).
    pub fn default_schedule() -> Self {
        Self {
            rotation_window_millis: DEFAULT_ROTATION_WINDOW_MILLIS,
            max_term_lifetime_millis: Some(DEFAULT_MAX_TERM_LIFETIME_MILLIS),
        }
    }

    /// Fail-closed construction: a non-positive window is refused (a
    /// zero/negative window would make every term permanently due, which
    /// silently defeats the schedule).
    pub fn new(
        rotation_window_millis: i64,
        max_term_lifetime_millis: Option<i64>,
    ) -> Result<Self, IdentityLifecycleError> {
        if rotation_window_millis <= 0 {
            return Err(IdentityLifecycleError::InvalidSchedule {
                reason: "the rotation window must be a positive millisecond value".to_string(),
            });
        }
        if let Some(maximum) = max_term_lifetime_millis {
            if maximum <= 0 {
                return Err(IdentityLifecycleError::InvalidSchedule {
                    reason: "the maximum term lifetime must be a positive millisecond value"
                        .to_string(),
                });
            }
            if maximum <= rotation_window_millis {
                return Err(IdentityLifecycleError::InvalidSchedule {
                    reason: "the maximum term lifetime must exceed the rotation window (a \
                             term would be born already due for rotation)"
                        .to_string(),
                });
            }
        }
        Ok(Self {
            rotation_window_millis,
            max_term_lifetime_millis,
        })
    }

    /// Assesses one active credential term against `now_unix_millis`.
    /// The record must pass P6.06 validation first (the caller composes
    /// the ceremony's own validated records; a failed validation is the
    /// named [`IdentityLifecycleError::CredentialInvalid`]).
    pub fn assess(
        &self,
        credential: &DeviceCredentialRecord,
        now_unix_millis: i64,
    ) -> Result<RotationAssessment, IdentityLifecycleError> {
        credential
            .validate()
            .map_err(|error| IdentityLifecycleError::CredentialInvalid {
                reason: error.to_string(),
            })?;
        if let Some(maximum) = self.max_term_lifetime_millis {
            if let Some(expires_at) = credential.expires_at_unix_millis {
                let lifetime = expires_at.saturating_sub(credential.issued_at_unix_millis);
                if lifetime > maximum {
                    return Ok(RotationAssessment::LifetimeExcessive);
                }
            }
        }
        let Some(expires_at) = credential.expires_at_unix_millis else {
            return Ok(RotationAssessment::NoExpiryRecorded);
        };
        if expires_at <= now_unix_millis {
            return Ok(RotationAssessment::Expired);
        }
        if expires_at - now_unix_millis <= self.rotation_window_millis {
            return Ok(RotationAssessment::RotateDue);
        }
        Ok(RotationAssessment::Current)
    }
}

// --- Revocation propagation (the DESIGN; the runbook is the record) ---------------------

/// The recorded kernel-interplay statement (quoted by the P18.08 record;
/// the kernel register stays byte-identical — phase-18 ban 8).
pub const KERNEL_REVOCATION_INTERPLAY_STATEMENT: &'static str = "the kernel P7.38 \
     revocation register (crates/flexcore-kernel/src/revocation.rs) is PLUGIN-scoped — \
     keyed by (plugin id, version) and signer key REFERENCE, append-only, no un-revoke — \
     and remains BYTE-IDENTICAL in phase 18. Device-identity revocation is a separate \
     register family (the 0018 edge_device_registry REVOKED term + the 0018 \
     edge_enrollment_journal REVOKED event). The registers never write each other; the \
     only interplay is consumption at gates; no percentage-canary reinterpretation of \
     the kernel staged-rollout policy arises here or anywhere in phase 18.";

/// The closed revocation-propagation TARGET vocabulary (the order IS the
/// design: identity register first, then the trust chains, then the
/// gates — a gate that still trusts a revoked identity is a design
/// violation the record names).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RevocationTarget {
    /// The 0018 identity evidence: a REVOKED term row in
    /// `edge_device_registry` and a REVOKED event in
    /// `edge_enrollment_journal` (append-only; INSERT-only grants).
    IdentityRegister,
    /// The protocol trust chains (P14.03 interplay): the device leaves the
    /// aggregator-side trust posture for its protocol (2030.5 et al.) —
    /// recorded as an operator/PKI-operations runbook action against the
    /// deployed chain; FlexCore stores anchor LABELS and never key
    /// material.
    ProtocolTrustChain,
    /// The outbound-channel gate: the edge's own fail-closed enrollment
    /// seam (`crate::enrollment::RevocationChecker`) refuses to bring up
    /// the channel on a revoked/expired identity, and the control plane
    /// refuses the session (P14.02 `MtlsRequirements` fail-closed
    /// revocation posture).
    UplinkChannelGate,
    /// The update-authorization gate: a revoked device is refused
    /// attestation/authorization for ANY update (P18.13 consumes this;
    /// `crate::attestation::AttestationDecision::RefuseRevokedDevice`).
    UpdateAuthorizationGate,
    /// The zero-touch surface: any outstanding voucher entry for the
    /// device is invalidated (P18.09 consumes this — a revoked device can
    /// never be (re)claimed; `crate::zero_touch`).
    ZeroTouchVoucher,
}

impl RevocationTarget {
    /// The stable runbook/vocabulary name.
    pub fn name(self) -> &'static str {
        match self {
            RevocationTarget::IdentityRegister => "IDENTITY_REGISTER",
            RevocationTarget::ProtocolTrustChain => "PROTOCOL_TRUST_CHAIN",
            RevocationTarget::UplinkChannelGate => "UPLINK_CHANNEL_GATE",
            RevocationTarget::UpdateAuthorizationGate => "UPDATE_AUTHORIZATION_GATE",
            RevocationTarget::ZeroTouchVoucher => "ZERO_TOUCH_VOUCHER",
        }
    }
}

/// The canonical propagation ORDER (the design's enforcement shape):
/// registry evidence first, trust chains next, then every gate. The
/// P18.10 decommissioning runbook composes the SAME order plus its
/// end-of-life steps; the ordering rule is validated here so both
/// surfaces share one definition.
pub const PROPAGATION_ORDER: [RevocationTarget; 5] = [
    RevocationTarget::IdentityRegister,
    RevocationTarget::ProtocolTrustChain,
    RevocationTarget::UplinkChannelGate,
    RevocationTarget::UpdateAuthorizationGate,
    RevocationTarget::ZeroTouchVoucher,
];

/// The closed per-target propagation status (a DESIGN vocabulary for
/// runbooks and test doubles — never a claim that a propagation ran).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropagationStepStatus {
    /// The step is specified but has not been performed (the honest
    /// default: no device exists, nothing propagates).
    SpecifiedNotPerformed,
    /// The step was RECORDED as performed in a test-double exercise
    /// (design validation only; never a real device).
    RecordedOnTestDouble,
    /// The step cannot complete without a deployment operation outside
    /// FlexCore (PKI operations, protocol-trust-chain action) — recorded
    /// as the named dependency, never silently skipped.
    RequiresDeploymentOperation,
}

impl PropagationStepStatus {
    /// The stable runbook/vocabulary name.
    pub fn name(self) -> &'static str {
        match self {
            PropagationStepStatus::SpecifiedNotPerformed => "SPECIFIED_NOT_PERFORMED",
            PropagationStepStatus::RecordedOnTestDouble => "RECORDED_ON_TEST_DOUBLE",
            PropagationStepStatus::RequiresDeploymentOperation => {
                "REQUIRES_DEPLOYMENT_OPERATION"
            }
        }
    }
}

/// One propagation step in a revocation/decommissioning plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevocationPropagationStep {
    pub target: RevocationTarget,
    /// What the step records (actor, ticket, or runbook reference —
    /// mandatory; an unattributable step is refused).
    pub actor: String,
    /// The reason/decommission ticket reference (mandatory).
    pub reference: String,
    pub status: PropagationStepStatus,
}

/// A revocation-propagation PLAN over one device identity: the ordered
/// steps plus the validation that the order matches
/// [`PROPAGATION_ORDER`] (a plan that authorizes a gate before the
/// registry evidence exists is refused — the record's named design
/// violation).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevocationPropagationPlan {
    pub device_id: String,
    pub reason: String,
    steps: BTreeMap<usize, RevocationPropagationStep>,
}

impl RevocationPropagationPlan {
    /// Builds a plan with one step per propagation target, in the
    /// canonical order. Actor, reason, and per-step references are
    /// mandatory (fail-closed): a revocation that cannot say WHO and WHY
    /// is refused, mirroring the ceremony's revoke discipline.
    pub fn new(
        device_id: &str,
        reason: &str,
        actor: &str,
        per_target_reference: &dyn Fn(RevocationTarget) -> String,
    ) -> Result<Self, IdentityLifecycleError> {
        if device_id.trim().is_empty() {
            return Err(IdentityLifecycleError::InvalidPlan {
                reason: "the device identity is mandatory".to_string(),
            });
        }
        if reason.trim().is_empty() {
            return Err(IdentityLifecycleError::InvalidPlan {
                reason: "the revocation reason is mandatory (an unexplained revocation is \
                         refused, mirroring the P9.33 revoke discipline)"
                    .to_string(),
            });
        }
        if actor.trim().is_empty() {
            return Err(IdentityLifecycleError::InvalidPlan {
                reason: "the revoking actor is mandatory".to_string(),
            });
        }
        let mut steps = BTreeMap::new();
        for (index, target) in PROPAGATION_ORDER.iter().enumerate() {
            let reference = per_target_reference(*target);
            if reference.trim().is_empty() {
                return Err(IdentityLifecycleError::InvalidPlan {
                    reason: format!(
                        "the propagation reference for {} is mandatory",
                        target.name()
                    ),
                });
            }
            steps.insert(
                index,
                RevocationPropagationStep {
                    target: *target,
                    actor: actor.to_string(),
                    reference,
                    status: PropagationStepStatus::SpecifiedNotPerformed,
                },
            );
        }
        Ok(Self {
            device_id: device_id.to_string(),
            reason: reason.to_string(),
            steps,
        })
    }

    /// The steps in the canonical order.
    pub fn steps(&self) -> Vec<RevocationPropagationStep> {
        self.steps.values().cloned().collect()
    }

    /// Records a status update on one step (a design/test-double
    /// recording — never a claim that a real propagation ran). Recording
    /// a step whose PREDECESSOR is still
    /// [`PropagationStepStatus::SpecifiedNotPerformed`] is refused: the
    /// gates come after the registry evidence and the trust chains.
    pub fn record_step_status(
        &mut self,
        target: RevocationTarget,
        status: PropagationStepStatus,
    ) -> Result<(), IdentityLifecycleError> {
        let position = PROPAGATION_ORDER
            .iter()
            .position(|t| *t == target)
            .expect("target comes from the closed PROPAGATION_ORDER vocabulary");
        if position > 0 {
            let predecessor = PROPAGATION_ORDER[position - 1];
            let predecessor_ready = self
                .steps
                .get(&(position - 1))
                .map(|step| step.status != PropagationStepStatus::SpecifiedNotPerformed)
                .unwrap_or(false);
            if !predecessor_ready {
                return Err(IdentityLifecycleError::PropagationOutOfOrder {
                    target: target.name(),
                    predecessor: predecessor.name(),
                });
            }
        }
        let step = self
            .steps
            .get_mut(&position)
            .expect("every PROPAGATION_ORDER target has a step");
        step.status = status;
        Ok(())
    }
}

// --- Errors -----------------------------------------------------------------------

/// The named identity-lifecycle errors. Nothing implies a silent pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdentityLifecycleError {
    /// The rotation-schedule configuration itself is invalid.
    InvalidSchedule { reason: String },
    /// The assessed credential failed P6.06 validation.
    CredentialInvalid { reason: String },
    /// A propagation plan was malformed.
    InvalidPlan { reason: String },
    /// A step was recorded before its predecessor (the gates come after
    /// the registry evidence).
    PropagationOutOfOrder { target: &'static str, predecessor: &'static str },
}

impl IdentityLifecycleError {
    /// The stable machine-readable error name.
    pub fn name(&self) -> &'static str {
        match self {
            IdentityLifecycleError::InvalidSchedule { .. } => "IDENTITY_SCHEDULE_INVALID",
            IdentityLifecycleError::CredentialInvalid { .. } => "IDENTITY_CREDENTIAL_INVALID",
            IdentityLifecycleError::InvalidPlan { .. } => "IDENTITY_PLAN_INVALID",
            IdentityLifecycleError::PropagationOutOfOrder { .. } => {
                "IDENTITY_PROPAGATION_OUT_OF_ORDER"
            }
        }
    }
}

impl std::fmt::Display for IdentityLifecycleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IdentityLifecycleError::InvalidSchedule { reason } => {
                write!(f, "the rotation schedule is invalid: {reason}")
            }
            IdentityLifecycleError::CredentialInvalid { reason } => {
                write!(f, "the credential failed P6.06 validation: {reason}")
            }
            IdentityLifecycleError::InvalidPlan { reason } => {
                write!(f, "the propagation plan is invalid: {reason}")
            }
            IdentityLifecycleError::PropagationOutOfOrder { target, predecessor } => write!(
                f,
                "the propagation step for {target} was recorded before its predecessor \
                 {predecessor} — the gates come after the registry evidence"
            ),
        }
    }
}

impl std::error::Error for IdentityLifecycleError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::enrollment::PemBody;

    const BASE: i64 = 1_700_000_000_000;

    fn credential(expires_at: Option<i64>) -> DeviceCredentialRecord {
        DeviceCredentialRecord {
            device_id: "edge-01".to_string(),
            tenant_id: "tenant-a".to_string(),
            site_id: "pilot-site-A".to_string(),
            certificate_pem: Some(PemBody::new(
                "-----BEGIN CERTIFICATE-----\nTEST\n-----END CERTIFICATE-----\n".to_string(),
            )),
            chain_pem: Vec::new(),
            root_ca_pem: Some(PemBody::new(
                "-----BEGIN CERTIFICATE-----\nTEST\n-----END CERTIFICATE-----\n".to_string(),
            )),
            private_key_reference: None,
            issued_at_unix_millis: BASE,
            expires_at_unix_millis: expires_at,
        }
    }

    #[test]
    fn the_mapping_statement_names_the_ceremony_subject() {
        // The formalization: the ceremony's subject is the LDevID; the
        // IDevID is only ever verification/attestation input.
        assert_eq!(IdentityTermMap::CEREMONY_SUBJECT, DeviceIdentityClass::LDevID);
        assert_eq!(
            IdentityTermMap::ZERO_TOUCH_VERIFICATION_ANCHOR,
            DeviceIdentityClass::IDevID
        );
        assert_eq!(
            IdentityTermMap::ATTESTATION_ANCHOR_CANDIDATE,
            DeviceIdentityClass::IDevID
        );
        assert!(IdentityTermMap::MAPPING_STATEMENT.contains("never a ceremony subject"));
    }

    #[test]
    fn the_rotation_schedule_assesses_against_the_injected_instant() {
        let schedule = CredentialRotationSchedule::default_schedule();
        // A 90-day term issued at BASE, assessed just after issue: current.
        let fresh = credential(Some(BASE + DEFAULT_MAX_TERM_LIFETIME_MILLIS));
        assert_eq!(
            schedule
                .assess(&fresh, BASE + 1_000)
                .expect("valid credential"),
            RotationAssessment::Current
        );
        // Inside the 30-day window: rotation due.
        let due = credential(Some(BASE + DEFAULT_MAX_TERM_LIFETIME_MILLIS));
        assert_eq!(
            schedule
                .assess(&due, BASE + DEFAULT_MAX_TERM_LIFETIME_MILLIS - 10 * 24 * 60 * 60_000)
                .expect("valid credential"),
            RotationAssessment::RotateDue
        );
        // Past expiry: expired.
        assert_eq!(
            schedule
                .assess(&due, BASE + DEFAULT_MAX_TERM_LIFETIME_MILLIS + 1_000)
                .expect("valid credential"),
            RotationAssessment::Expired
        );
        // A term longer than the ceiling: refused as excessive.
        let excessive = credential(Some(BASE + DEFAULT_MAX_TERM_LIFETIME_MILLIS + 1));
        assert_eq!(
            schedule
                .assess(&excessive, BASE + 1_000)
                .expect("valid credential"),
            RotationAssessment::LifetimeExcessive
        );
        // No expiry recorded: NEVER assumed fine.
        assert_eq!(
            schedule
                .assess(&credential(None), BASE + 1_000)
                .expect("valid credential"),
            RotationAssessment::NoExpiryRecorded
        );
    }

    #[test]
    fn the_schedule_refuses_an_inconsistent_configuration() {
        assert_eq!(
            CredentialRotationSchedule::new(0, None).unwrap_err().name(),
            "IDENTITY_SCHEDULE_INVALID"
        );
        assert_eq!(
            CredentialRotationSchedule::new(-1, None).unwrap_err().name(),
            "IDENTITY_SCHEDULE_INVALID"
        );
        assert_eq!(
            CredentialRotationSchedule::new(30 * 24 * 60 * 60_000, Some(30 * 24 * 60 * 60_000))
                .unwrap_err()
                .name(),
            "IDENTITY_SCHEDULE_INVALID",
            "a maximum equal to the window makes every term born due"
        );
    }

    #[test]
    fn the_propagation_plan_enforces_the_canonical_order() {
        let mut plan = RevocationPropagationPlan::new(
            "edge-01",
            "decommissioning design exercise",
            "operator",
            &|target| format!("ref-for-{}", target.name()),
        )
        .expect("valid plan");
        assert_eq!(plan.steps().len(), PROPAGATION_ORDER.len());
        // The FIRST gate step cannot be recorded before the registry step.
        assert_eq!(
            plan.record_step_status(
                RevocationTarget::UpdateAuthorizationGate,
                PropagationStepStatus::RecordedOnTestDouble
            )
            .unwrap_err()
            .name(),
            "IDENTITY_PROPAGATION_OUT_OF_ORDER"
        );
        // In order: registry → trust chain → channel → update gate → voucher.
        for target in PROPAGATION_ORDER {
            plan.record_step_status(target, PropagationStepStatus::RecordedOnTestDouble)
                .expect("the predecessor is recorded");
        }
        let steps = plan.steps();
        assert!(steps.iter().all(|step| step.status
            == PropagationStepStatus::RecordedOnTestDouble));
        // Every recorded step is a TEST-DOUBLE recording — the vocabulary
        // carries no "performed on a real device" state at all.
        assert!(PropagationStepStatus::name(PropagationStepStatus::SpecifiedNotPerformed)
            .contains("NOT_PERFORMED"));
    }

    #[test]
    fn the_plan_refuses_unattributable_or_unexplained_inputs() {
        assert_eq!(
            RevocationPropagationPlan::new("", "reason", "operator", &|_| "ref".to_string())
                .unwrap_err()
                .name(),
            "IDENTITY_PLAN_INVALID"
        );
        assert_eq!(
            RevocationPropagationPlan::new("edge-01", "  ", "operator", &|_| "ref".to_string())
                .unwrap_err()
                .name(),
            "IDENTITY_PLAN_INVALID"
        );
        assert_eq!(
            RevocationPropagationPlan::new("edge-01", "reason", " ", &|_| "ref".to_string())
                .unwrap_err()
                .name(),
            "IDENTITY_PLAN_INVALID"
        );
        assert_eq!(
            RevocationPropagationPlan::new("edge-01", "reason", "operator", &|_| "  ".to_string())
                .unwrap_err()
                .name(),
            "IDENTITY_PLAN_INVALID",
            "an untraceable step reference is refused"
        );
    }

    #[test]
    fn the_kernel_interplay_statement_stays_scoped() {
        // The interplay is consumption-only; the statement pins both the
        // byte-identical rule and the no-canary-reinterpretation rule.
        assert!(KERNEL_REVOCATION_INTERPLAY_STATEMENT.contains("BYTE-IDENTICAL"));
        assert!(KERNEL_REVOCATION_INTERPLAY_STATEMENT.contains("no percentage-canary"));
        assert!(KERNEL_REVOCATION_INTERPLAY_STATEMENT.contains("no un-revoke"));
    }
}
