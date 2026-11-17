// P18.13 — ATTESTATION design: boot-time evidence, measurement reporting,
//          and the verification policy the platform applies BEFORE
//          authorizing an update for a device (phase-18 plan P18.13;
//          source [S-166] T2TRG security-setup survey; the P6.34
//          secure-boot position
//          `ledger/phase-06-edge-protocol/34_secure_boot_position.md`).
//
// THE DECISION FRAME (P18-E-2, registered OPEN in the phase-18 ledger and
// in the P18.13 decision record; agents never close an E-decision):
//
// | Option | Meaning | Status |
// |---|---|---|
// | (a) hardware-optional design | the platform works WITH and WITHOUT a hardware anchor; the difference is stated honestly per case | **DEFAULT — implemented here** |
// | (b) hardware-required | rejected for 0.x: no fleet, no hardware spec | rejected option, recorded |
// | (c) ignore hardware | rejected: dishonest | rejected option, recorded |
//
// The assumption set is **PENDING PILOT HARDWARE SPEC** (the P6.34
// posture, verbatim discipline): no TPM/secure element is assumed, none
// is claimed, and no hardware determination is made anywhere in this
// module. Filling the determination is the pilot hardware spec's job
// (human owner).
//
// WHAT THE PLATFORM CHECKS BEFORE AUTHORIZING AN UPDATE
// ([`check_before_update_authorization`], fail-closed, in decision order):
//
// 1. **Evidence exists.** No evidence at all is
//    [`AttestationDecision::RefuseNoEvidence`] — absence is never a pass.
// 2. **The device is not revoked** (P18.08 propagation consumed at this
//    gate: [`AttestationDecision::RefuseRevokedDevice`]).
// 3. **The evidence is fresh** — `reported_at` within the policy's age
//    ceiling against the INJECTED instant
//    ([`AttestationDecision::RefuseEvidenceStale`]).
// 4. **Counters did not regress** — the reported boot counter must be
//    >= the last-seen counter ([`AttestationDecision::RefuseCounterRegression`];
//    a regression is the signature of a restored image or a rolled-back
//    state — exactly what an update authorization must not ride).
// 5. **Measurements match the policy mode** —
//    [`MeasurementCheckMode::NoMeasurementCheck`] records that NO
//    measurement assertion is made;
//    [`MeasurementCheckMode::BaselineDigestEquality`] requires every
//    expected component to match (an unexpected or missing component is
//    a mismatch);
//    [`MeasurementCheckMode::ReferenceRegisterMatch`] requires the
//    reported measurement set to match the deployment's reference
//    register values.
// 6. **Hardware-anchor posture** — if the policy requires an anchor
//    ([`AttestationVerificationPolicy::require_hardware_anchor`]) the
//    evidence must claim one; the DEFAULT policy does NOT (P18-E-2
//    DEFAULT (a)), and a claimed anchor is
//    [`TrustAnchorMode::HardwareAnchorClaimed`] — CLAIMED, never verified
//    by this platform (anchor verification needs a deployed verifier,
//    a DEPLOYMENT REQUIREMENT recorded in the decision record).
//
// THE HARDWARE-ABSENT FALLBACK, HONESTLY (the P18-E-2 DEFAULT):
//
// - **WITH a hardware anchor** (a future deployment, hardware spec
//   pending): evidence MAY be anchor-signed; verifying that signature is
//   real root-of-trust work requiring deployed hardware + a verifier
//   service — NEITHER EXISTS in this program. This module defines the
//   evidence SHAPE such a deployment would report; it performs no
//   signature verification and claims none.
// - **WITHOUT an anchor** (the only case that can exist today):
//   evidence is software-reported self-attestation — the device reports
//   its own measurements and counters, and NOTHING anchors their
//   truthfulness. The policy checks consistency, freshness, and
//   regression; it CANNOT detect a compromised reporter. The decision
//   record states the assurance bound verbatim; the platform never
//   upgrades software evidence into a hardware claim.
//
// DIGEST GRADE (conventions §6.6): the measurement digests CARRIED by the
// evidence type are opaque text; this module computes no digest and
// verifies no signature. Test-grade digest computation (the FNV-1a-64
// family) stays loudly labeled test-grade in the P18.14 test fixtures;
// production measurement digests (TPM PCRs where hardware exists, or a
// measured-boot software scheme where it does not) are a DEPLOYMENT
// REQUIREMENT and a human-gated choice — PENDING PILOT HARDWARE SPEC.
//
// HONESTY BOUNDS: no device exists; no evidence was ever collected,
// reported, or verified; no attestation decision has ever been made
// against a real device (phase-18 constraint 6). The gate is a PURE
// function over test doubles — the P18.06 orchestrator (Agent A) CONSUMES
// it; the fleet journal vocabulary
// ([`FleetAttestationState`]) rides the migration-0023
// `ATTESTATION_STATE` rows ([`crate::fleet_registry`]).

use std::collections::BTreeMap;

use crate::fleet_registry::FleetRegistryError;

// --- The trust-anchor posture (honest vocabulary) ---------------------------------

/// The closed trust-anchor posture of one evidence report. The names are
/// deliberately not "verified": this platform verifies NO anchor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrustAnchorMode {
    /// The device REPORTS a hardware anchor (TPM/secure element). A
    /// REPORT, never a platform verification — the P6.34 posture stands
    /// until the pilot hardware spec fills the determination.
    HardwareAnchorClaimed,
    /// The device reports software-only evidence (no anchor claimed, or
    /// the reporter cannot reach one). The consistency checks run; NO
    /// root-of-trust assertion exists or is implied.
    SoftwareOnlyEvidence,
}

impl TrustAnchorMode {
    /// The stable audit/vocabulary name.
    pub fn name(self) -> &'static str {
        match self {
            TrustAnchorMode::HardwareAnchorClaimed => "HARDWARE_ANCHOR_CLAIMED",
            TrustAnchorMode::SoftwareOnlyEvidence => "SOFTWARE_ONLY_EVIDENCE",
        }
    }
}

/// The recorded honesty statement (quoted by the P18.13 record and pinned
/// by a test).
pub const HARDWARE_POSTURE_STATEMENT: &'static str = "every hardware determination is \
     PENDING PILOT HARDWARE SPEC (the P6.34 posture; P18-E-2 DEFAULT (a) hardware-optional). \
     A claimed anchor is a device REPORT this platform does not verify; software-only \
     evidence is consistency-checked self-attestation that cannot detect a compromised \
     reporter. No root of trust is assumed, claimed, or implied anywhere in this module.";

// --- The evidence shape -----------------------------------------------------------

/// One reported boot-time measurement. The digest is OPAQUE text (the
/// reporter's encoding; this module computes and validates nothing about
/// its content — only the policy mode decides whether and how it is
/// compared).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeasurementRecord {
    /// The measured component label (e.g. the bootloader/kernel stage
    /// name the deployment's measurement scheme defines).
    pub component: String,
    /// The reported digest (opaque text; the algorithm label below names
    /// the scheme so a scheme change is a detectable format change).
    pub digest_hex: String,
    /// The digest-scheme label (e.g. `test-fnv1a64` for the labeled
    /// test-grade family; a production scheme is a DEPLOYMENT
    /// REQUIREMENT — PENDING PILOT HARDWARE SPEC).
    pub digest_scheme: String,
}

/// One boot-time attestation evidence report from one device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttestationEvidence {
    /// The device identity the evidence names (the gate refuses evidence
    /// that names a different device than the one being authorized).
    pub device_id: String,
    /// When the evidence was reported (unix millis — recorded data from
    /// the reporting side; freshness is checked against the INJECTED
    /// verification instant, never wall time).
    pub reported_at_unix_millis: i64,
    pub anchor_mode: TrustAnchorMode,
    /// The reported measurements (BTreeMap-ordered by component for
    /// deterministic comparison).
    pub measurements: Vec<MeasurementRecord>,
    /// The reported boot counter (monotonicity is checked against the
    /// last-seen value; a regression is a named refusal).
    pub boot_counter: u64,
    /// A reference to the reporting session/transport record (mandatory:
    /// unattributable evidence is refused).
    pub report_reference: String,
}

impl AttestationEvidence {
    /// Structural validation (fail-closed): identity, reference, positive
    /// instant, non-empty scheme labels, unique components.
    pub fn validate(&self) -> Result<(), AttestationError> {
        if self.device_id.trim().is_empty() {
            return Err(AttestationError::InvalidEvidence {
                reason: "the evidence names no device".to_string(),
            });
        }
        if self.report_reference.trim().is_empty() {
            return Err(AttestationError::InvalidEvidence {
                reason: "the evidence carries no report reference (unattributable evidence \
                         is refused)"
                    .to_string(),
            });
        }
        if self.reported_at_unix_millis <= 0 {
            return Err(AttestationError::InvalidEvidence {
                reason: "the report instant must be a positive unix-millis value".to_string(),
            });
        }
        let mut components = BTreeMap::new();
        for measurement in &self.measurements {
            if measurement.component.trim().is_empty() {
                return Err(AttestationError::InvalidEvidence {
                    reason: "a measurement names no component".to_string(),
                });
            }
            if measurement.digest_hex.trim().is_empty() {
                return Err(AttestationError::InvalidEvidence {
                    reason: format!(
                        "the measurement for '{}' carries no digest",
                        measurement.component
                    ),
                });
            }
            if measurement.digest_scheme.trim().is_empty() {
                return Err(AttestationError::InvalidEvidence {
                    reason: format!(
                        "the measurement for '{}' names no digest scheme",
                        measurement.component
                    ),
                });
            }
            if components.insert(measurement.component.clone(), ()).is_some() {
                return Err(AttestationError::InvalidEvidence {
                    reason: format!(
                        "the component '{}' is measured twice (duplicate components are \
                         refused — the measurement set must be unambiguous)",
                        measurement.component
                    ),
                });
            }
        }
        Ok(())
    }
}

// --- The verification policy -------------------------------------------------------

/// The closed measurement-check mode (policy knob 5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeasurementCheckMode {
    /// The platform records that NO measurement assertion is made and
    /// makes none. Honest by construction: a report that claims nothing
    /// verifies nothing.
    NoMeasurementCheck,
    /// Every expected component must be present and digest-equal; an
    /// unexpected or missing component is a mismatch. The expected map is
    /// the deployment's baseline (recorded configuration, pre-registered
    /// before any run that fills it — X-8).
    BaselineDigestEquality,
    /// The reported measurement set must match the deployment's reference
    /// register values (scheme + digest per component).
    ReferenceRegisterMatch,
}

impl MeasurementCheckMode {
    /// The stable audit/vocabulary name.
    pub fn name(self) -> &'static str {
        match self {
            MeasurementCheckMode::NoMeasurementCheck => "NO_MEASUREMENT_CHECK",
            MeasurementCheckMode::BaselineDigestEquality => "BASELINE_DIGEST_EQUALITY",
            MeasurementCheckMode::ReferenceRegisterMatch => "REFERENCE_REGISTER_MATCH",
        }
    }
}

/// The attestation verification policy: what the platform checks before
/// authorizing an update. All values are per-deployment configuration,
/// pre-registered (X-8) before any run that could fill them; the DEFAULT
/// policy below is the P18-E-2 DEFAULT (a) shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttestationVerificationPolicy {
    /// The maximum evidence age (milliseconds between `reported_at` and
    /// the verification instant).
    pub max_evidence_age_millis: i64,
    /// Whether the boot counter must be non-regressing (the DEFAULT
    /// policy keeps this ON — a regression refusal is the cheapest
    /// honest check available with or without hardware).
    pub require_non_regressing_counters: bool,
    pub measurement_check: MeasurementCheckMode,
    /// Whether an update authorization REQUIRES a claimed hardware
    /// anchor. The DEFAULT policy is `false` (P18-E-2 DEFAULT (a)):
    /// requiring an anchor today would refuse every device that can
    /// exist. A future deployment with a verified anchor posture may set
    /// it — that change is a policy decision, recorded, never silent.
    pub require_hardware_anchor: bool,
}

impl AttestationVerificationPolicy {
    /// The DEFAULT policy (P18-E-2 DEFAULT (a): hardware-optional). The
    /// evidence-age default (24 h) is a recorded recommendation, not a
    /// sourced figure.
    pub fn default_policy() -> Self {
        Self {
            max_evidence_age_millis: 24 * 60 * 60_000,
            require_non_regressing_counters: true,
            measurement_check: MeasurementCheckMode::BaselineDigestEquality,
            require_hardware_anchor: false,
        }
    }

    /// Fail-closed construction: a non-positive age ceiling is refused
    /// (zero evidence freshness is unattainable and would silently turn
    /// every authorization into a refusal — an inconsistent policy is
    /// refused, not tolerated).
    pub fn new(
        max_evidence_age_millis: i64,
        require_non_regressing_counters: bool,
        measurement_check: MeasurementCheckMode,
        require_hardware_anchor: bool,
    ) -> Result<Self, AttestationError> {
        if max_evidence_age_millis <= 0 {
            return Err(AttestationError::InvalidPolicy {
                reason: "the evidence-age ceiling must be a positive millisecond value"
                    .to_string(),
            });
        }
        Ok(Self {
            max_evidence_age_millis,
            require_non_regressing_counters,
            measurement_check,
            require_hardware_anchor,
        })
    }
}

// --- The decision vocabulary -------------------------------------------------------

/// The closed attestation decision vocabulary. Exactly one decision per
/// check; every refusal is named (nothing implies a silent pass).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttestationDecision {
    /// The policy is satisfied: the update authorization MAY proceed
    /// (the orchestrator still applies its own gates — cohort state,
    /// command-lifecycle blocking, P18.15 — attestation is one gate,
    /// never a bypass).
    AuthorizeUpdate,
    /// No evidence was presented (or evidence was unattributable).
    RefuseNoEvidence,
    /// The device is revoked (P18.08 propagation consumed at this gate).
    RefuseRevokedDevice,
    /// The evidence is older than the policy's age ceiling.
    RefuseEvidenceStale,
    /// The reported boot counter regressed below the last-seen value.
    RefuseCounterRegression,
    /// The evidence names a different device than the one being
    /// authorized.
    RefuseEvidenceDeviceMismatch,
    /// The measurements failed the policy's check mode.
    RefuseMeasurementMismatch,
    /// The policy requires a claimed hardware anchor and the evidence
    /// reports software-only.
    RefuseHardwareAnchorRequired,
}

impl AttestationDecision {
    /// The stable audit/vocabulary name.
    pub fn name(self) -> &'static str {
        match self {
            AttestationDecision::AuthorizeUpdate => "ATTESTATION_AUTHORIZE_UPDATE",
            AttestationDecision::RefuseNoEvidence => "ATTESTATION_REFUSE_NO_EVIDENCE",
            AttestationDecision::RefuseRevokedDevice => "ATTESTATION_REFUSE_REVOKED_DEVICE",
            AttestationDecision::RefuseEvidenceStale => "ATTESTATION_REFUSE_EVIDENCE_STALE",
            AttestationDecision::RefuseCounterRegression => {
                "ATTESTATION_REFUSE_COUNTER_REGRESSION"
            }
            AttestationDecision::RefuseEvidenceDeviceMismatch => {
                "ATTESTATION_REFUSE_EVIDENCE_DEVICE_MISMATCH"
            }
            AttestationDecision::RefuseMeasurementMismatch => {
                "ATTESTATION_REFUSE_MEASUREMENT_MISMATCH"
            }
            AttestationDecision::RefuseHardwareAnchorRequired => {
                "ATTESTATION_REFUSE_HARDWARE_ANCHOR_REQUIRED"
            }
        }
    }
}

// --- The gate (the pure check Agent A's orchestrator consumes) ----------------------

/// The baseline/reference values one check mode compares against (the
/// deployment's recorded configuration; BTreeMap-deterministic).
#[derive(Debug, Clone, Default)]
pub struct MeasurementExpectations {
    /// component → expected digest (opaque text, compared for equality).
    pub expected_digests: BTreeMap<String, String>,
}

/// THE GATE: what the platform checks before authorizing an update for
/// one device. PURE function; the INJECTED verification instant is a
/// parameter (no wall time). Fail-closed at every step; the FIRST failed
/// check is the decision (the decision record documents the order).
pub fn check_before_update_authorization(
    policy: &AttestationVerificationPolicy,
    evidence: Option<&AttestationEvidence>,
    device_id: &str,
    device_revoked: bool,
    last_seen_boot_counter: Option<u64>,
    expectations: &MeasurementExpectations,
    now_unix_millis: i64,
) -> Result<AttestationDecision, AttestationError> {
    // 1. Evidence exists.
    let Some(evidence) = evidence else {
        return Ok(AttestationDecision::RefuseNoEvidence);
    };
    evidence.validate()?;
    // 2. The device is not revoked.
    if device_revoked {
        return Ok(AttestationDecision::RefuseRevokedDevice);
    }
    // The evidence must name THIS device.
    if evidence.device_id != device_id {
        return Ok(AttestationDecision::RefuseEvidenceDeviceMismatch);
    }
    // 3. Freshness (injected instant).
    let age = now_unix_millis
        .checked_sub(evidence.reported_at_unix_millis)
        .ok_or(AttestationError::InvalidEvidence {
            reason: "the evidence reports a future instant (after the verification clock) — \
                     refused rather than negated into a fake freshness"
                .to_string(),
        })?;
    if age > policy.max_evidence_age_millis {
        return Ok(AttestationDecision::RefuseEvidenceStale);
    }
    // 4. Counter monotonicity.
    if policy.require_non_regressing_counters {
        if let Some(last_seen) = last_seen_boot_counter {
            if evidence.boot_counter < last_seen {
                return Ok(AttestationDecision::RefuseCounterRegression);
            }
        }
    }
    // 5. Measurement check mode.
    match policy.measurement_check {
        MeasurementCheckMode::NoMeasurementCheck => {
            // No assertion is made; nothing is checked. Recorded honestly.
        }
        MeasurementCheckMode::BaselineDigestEquality => {
            let reported: BTreeMap<&str, &str> = evidence
                .measurements
                .iter()
                .map(|m| (m.component.as_str(), m.digest_hex.as_str()))
                .collect();
            for (component, expected_digest) in &expectations.expected_digests {
                match reported.get(component.as_str()) {
                    Some(reported_digest) if reported_digest == expected_digest => {}
                    _ => return Ok(AttestationDecision::RefuseMeasurementMismatch),
                }
            }
            // An unexpected extra component is ALSO a mismatch (the
            // measurement set must be exactly the baseline set).
            for component in reported.keys() {
                if !expectations.expected_digests.contains_key(*component) {
                    return Ok(AttestationDecision::RefuseMeasurementMismatch);
                }
            }
        }
        MeasurementCheckMode::ReferenceRegisterMatch => {
            let reported: BTreeMap<&str, (&str, &str)> = evidence
                .measurements
                .iter()
                .map(|m| (m.component.as_str(), (m.digest_scheme.as_str(), m.digest_hex.as_str())))
                .collect();
            for (component, expected_digest) in &expectations.expected_digests {
                // The reference register encodes "scheme:digest" in its
                // expected value; the split is exact (no scheme means an
                // empty scheme, which never matches a labeled report).
                match reported.get(component.as_str()) {
                    Some((scheme, digest)) => {
                        let expected = expected_digest.as_str();
                        match expected.split_once(':') {
                            Some((expected_scheme, expected_digest)) => {
                                if scheme != expected_scheme || digest != expected_digest {
                                    return Ok(AttestationDecision::RefuseMeasurementMismatch);
                                }
                            }
                            None => {
                                if *digest != expected {
                                    return Ok(AttestationDecision::RefuseMeasurementMismatch);
                                }
                            }
                        }
                    }
                    None => return Ok(AttestationDecision::RefuseMeasurementMismatch),
                }
            }
            for component in reported.keys() {
                if !expectations.expected_digests.contains_key(*component) {
                    return Ok(AttestationDecision::RefuseMeasurementMismatch);
                }
            }
        }
    }
    // 6. Hardware-anchor posture.
    if policy.require_hardware_anchor
        && evidence.anchor_mode != TrustAnchorMode::HardwareAnchorClaimed
    {
        return Ok(AttestationDecision::RefuseHardwareAnchorRequired);
    }
    Ok(AttestationDecision::AuthorizeUpdate)
}

// --- The fleet-journal vocabulary (the ATTESTATION_STATE rows) ----------------------

/// The closed attestation-state vocabulary for the migration-0023
/// `ATTESTATION_STATE` journal rows (the `state_value` domain;
/// [`crate::fleet_registry`] delegates parsing here).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FleetAttestationState {
    /// No evidence has ever been received. Never healthy-by-default.
    NotReported,
    /// Fresh evidence passed the policy at the last check.
    Current,
    /// The last evidence aged out of the policy's freshness ceiling.
    Stale,
    /// The last evidence FAILED a policy check (any named refusal).
    Failed,
    /// The device was revoked (P18.08 propagation; the attestation gate
    /// refuses unconditionally).
    Revoked,
}

impl FleetAttestationState {
    /// The exact `state_value` string for the ATTESTATION_STATE rows.
    pub fn sql_value(self) -> &'static str {
        match self {
            FleetAttestationState::NotReported => "NOT_REPORTED",
            FleetAttestationState::Current => "CURRENT",
            FleetAttestationState::Stale => "STALE",
            FleetAttestationState::Failed => "FAILED",
            FleetAttestationState::Revoked => "REVOKED",
        }
    }

    /// Parses the journal's `state_value` for an attestation row. Unknown
    /// strings are the named [`FleetRegistryError::UnknownStateValue`]
    /// refusal.
    pub fn from_sql_value(value: &str) -> Result<Self, FleetRegistryError> {
        match value {
            "NOT_REPORTED" => Ok(FleetAttestationState::NotReported),
            "CURRENT" => Ok(FleetAttestationState::Current),
            "STALE" => Ok(FleetAttestationState::Stale),
            "FAILED" => Ok(FleetAttestationState::Failed),
            "REVOKED" => Ok(FleetAttestationState::Revoked),
            other => Err(FleetRegistryError::UnknownStateValue {
                kind: "ATTESTATION_STATE",
                encountered: other.to_string(),
            }),
        }
    }
}

// --- Errors -----------------------------------------------------------------------

/// The named attestation errors. Nothing implies a silent pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttestationError {
    /// The policy configuration itself is invalid.
    InvalidPolicy { reason: String },
    /// The evidence failed structural validation.
    InvalidEvidence { reason: String },
}

impl AttestationError {
    /// The stable machine-readable error name.
    pub fn name(&self) -> &'static str {
        match self {
            AttestationError::InvalidPolicy { .. } => "ATTESTATION_POLICY_INVALID",
            AttestationError::InvalidEvidence { .. } => "ATTESTATION_EVIDENCE_INVALID",
        }
    }
}

impl std::fmt::Display for AttestationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AttestationError::InvalidPolicy { reason } => {
                write!(f, "the attestation policy is invalid: {reason}")
            }
            AttestationError::InvalidEvidence { reason } => {
                write!(f, "the attestation evidence is invalid: {reason}")
            }
        }
    }
}

impl std::error::Error for AttestationError {}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: i64 = 1_700_000_000_000;
    const DAY: i64 = 24 * 60 * 60_000;

    fn evidence(device: &str) -> AttestationEvidence {
        AttestationEvidence {
            device_id: device.to_string(),
            reported_at_unix_millis: BASE,
            anchor_mode: TrustAnchorMode::SoftwareOnlyEvidence,
            measurements: vec![
                MeasurementRecord {
                    component: "bootloader".to_string(),
                    digest_hex: "aaaa0000bbbb1111".to_string(),
                    digest_scheme: "test-fnv1a64".to_string(),
                },
                MeasurementRecord {
                    component: "agent".to_string(),
                    digest_hex: "cccc2222dddd3333".to_string(),
                    digest_scheme: "test-fnv1a64".to_string(),
                },
            ],
            boot_counter: 7,
            report_reference: "report-ref-1".to_string(),
        }
    }

    fn baseline() -> MeasurementExpectations {
        let mut expected = BTreeMap::new();
        expected.insert("bootloader".to_string(), "aaaa0000bbbb1111".to_string());
        expected.insert("agent".to_string(), "cccc2222dddd3333".to_string());
        MeasurementExpectations {
            expected_digests: expected,
        }
    }

    #[test]
    fn the_default_policy_refuses_absent_evidence_and_revoked_devices() {
        let policy = AttestationVerificationPolicy::default_policy();
        assert_eq!(
            check_before_update_authorization(
                &policy,
                None,
                "edge-01",
                false,
                None,
                &MeasurementExpectations::default(),
                BASE + 1_000
            )
            .expect("pure check"),
            AttestationDecision::RefuseNoEvidence,
            "absence is never a pass"
        );
        let evidence = evidence("edge-01");
        assert_eq!(
            check_before_update_authorization(
                &policy,
                Some(&evidence),
                "edge-01",
                true,
                None,
                &baseline(),
                BASE + 1_000
            )
            .expect("pure check"),
            AttestationDecision::RefuseRevokedDevice,
            "the P18.08 revocation propagation is consumed at this gate"
        );
    }

    #[test]
    fn the_gate_checks_in_decision_order_with_named_refusals() {
        let policy = AttestationVerificationPolicy::default_policy();
        // Device mismatch.
        let mismatched = evidence("edge-other");
        assert_eq!(
            check_before_update_authorization(
                &policy, Some(&mismatched), "edge-01", false, None, &baseline(), BASE + 1_000
            )
            .expect("pure check"),
            AttestationDecision::RefuseEvidenceDeviceMismatch
        );
        // Stale evidence (older than the 24 h ceiling).
        let evidence = evidence("edge-01");
        assert_eq!(
            check_before_update_authorization(
                &policy, Some(&evidence), "edge-01", false, None, &baseline(), BASE + DAY + 1
            )
            .expect("pure check"),
            AttestationDecision::RefuseEvidenceStale
        );
        // Counter regression (reported 7, last seen 8).
        assert_eq!(
            check_before_update_authorization(
                &policy, Some(&evidence), "edge-01", false, Some(8), &baseline(), BASE + 1_000
            )
            .expect("pure check"),
            AttestationDecision::RefuseCounterRegression
        );
        // Measurement mismatch (one digest altered).
        let mut tampered = evidence("edge-01");
        tampered.measurements[1].digest_hex = "deadbeefdeadbeef".to_string();
        assert_eq!(
            check_before_update_authorization(
                &policy, Some(&tampered), "edge-01", false, None, &baseline(), BASE + 1_000
            )
            .expect("pure check"),
            AttestationDecision::RefuseMeasurementMismatch
        );
        // An unexpected EXTRA component is also a mismatch.
        let mut extra = evidence("edge-01");
        extra.measurements.push(MeasurementRecord {
            component: "unexpected-stage".to_string(),
            digest_hex: "eeee4444ffff5555".to_string(),
            digest_scheme: "test-fnv1a64".to_string(),
        });
        assert_eq!(
            check_before_update_authorization(
                &policy, Some(&extra), "edge-01", false, None, &baseline(), BASE + 1_000
            )
            .expect("pure check"),
            AttestationDecision::RefuseMeasurementMismatch
        );
        // The happy path: everything checks.
        assert_eq!(
            check_before_update_authorization(
                &policy, Some(&evidence), "edge-01", false, Some(7), &baseline(), BASE + 1_000
            )
            .expect("pure check"),
            AttestationDecision::AuthorizeUpdate
        );
    }

    #[test]
    fn the_hardware_anchor_knobs_honor_p18_e_2_default_a() {
        // The DEFAULT policy does NOT require an anchor: software-only
        // evidence authorizes (the honest bound: consistency checks only).
        let default_policy = AttestationVerificationPolicy::default_policy();
        assert!(!default_policy.require_hardware_anchor);
        let software = evidence("edge-01");
        assert_eq!(
            check_before_update_authorization(
                &default_policy, Some(&software), "edge-01", false, None, &baseline(), BASE + 1_000
            )
            .expect("pure check"),
            AttestationDecision::AuthorizeUpdate
        );
        // A deployment that later sets the requirement refuses
        // software-only evidence BY NAME.
        let anchor_required = AttestationVerificationPolicy::new(
            DAY,
            true,
            MeasurementCheckMode::BaselineDigestEquality,
            true,
        )
        .expect("valid policy");
        assert_eq!(
            check_before_update_authorization(
                &anchor_required,
                Some(&software),
                "edge-01",
                false,
                None,
                &baseline(),
                BASE + 1_000
            )
            .expect("pure check"),
            AttestationDecision::RefuseHardwareAnchorRequired
        );
        // A CLAIMED anchor satisfies the requirement — a claim, never a
        // platform verification (the P6.34 posture).
        let mut claimed = evidence("edge-01");
        claimed.anchor_mode = TrustAnchorMode::HardwareAnchorClaimed;
        assert_eq!(
            check_before_update_authorization(
                &anchor_required, Some(&claimed), "edge-01", false, None, &baseline(), BASE + 1_000
            )
            .expect("pure check"),
            AttestationDecision::AuthorizeUpdate
        );
    }

    #[test]
    fn the_reference_register_mode_compares_scheme_and_digest() {
        let policy = AttestationVerificationPolicy::new(
            DAY,
            true,
            MeasurementCheckMode::ReferenceRegisterMatch,
            false,
        )
        .expect("valid policy");
        let mut register = BTreeMap::new();
        register.insert("bootloader".to_string(), "test-fnv1a64:aaaa0000bbbb1111".to_string());
        register.insert("agent".to_string(), "test-fnv1a64:cccc2222dddd3333".to_string());
        let expectations = MeasurementExpectations {
            expected_digests: register,
        };
        let evidence = evidence("edge-01");
        assert_eq!(
            check_before_update_authorization(
                &policy, Some(&evidence), "edge-01", false, None, &expectations, BASE + 1_000
            )
            .expect("pure check"),
            AttestationDecision::AuthorizeUpdate
        );
        // A scheme change (same digest text) is a mismatch.
        let mut rescheme = evidence("edge-01");
        rescheme.measurements[0].digest_scheme = "sha256-256bit".to_string();
        assert_eq!(
            check_before_update_authorization(
                &policy, Some(&rescheme), "edge-01", false, None, &expectations, BASE + 1_000
            )
            .expect("pure check"),
            AttestationDecision::RefuseMeasurementMismatch,
            "a digest-scheme change is a detectable format change, never a silent pass"
        );
    }

    #[test]
    fn no_measurement_check_records_that_nothing_is_asserted() {
        let policy = AttestationVerificationPolicy::new(
            DAY,
            true,
            MeasurementCheckMode::NoMeasurementCheck,
            false,
        )
        .expect("valid policy");
        let mut empty = evidence("edge-01");
        empty.measurements.clear();
        assert_eq!(
            check_before_update_authorization(
                &policy, Some(&empty), "edge-01", false, None, &baseline(), BASE + 1_000
            )
            .expect("pure check"),
            AttestationDecision::AuthorizeUpdate,
            "the mode makes no measurement assertion — honestly, not as a pass of one"
        );
    }

    #[test]
    fn evidence_validation_refuses_duplicates_and_gaps() {
        let mut duplicate = evidence("edge-01");
        duplicate.measurements.push(MeasurementRecord {
            component: "bootloader".to_string(),
            digest_hex: "aaaa0000bbbb1111".to_string(),
            digest_scheme: "test-fnv1a64".to_string(),
        });
        assert_eq!(
            duplicate.validate().unwrap_err().name(),
            "ATTESTATION_EVIDENCE_INVALID"
        );
        let mut anonymous = evidence("edge-01");
        anonymous.report_reference = " ".to_string();
        assert_eq!(
            anonymous.validate().unwrap_err().name(),
            "ATTESTATION_EVIDENCE_INVALID"
        );
        assert_eq!(
            AttestationVerificationPolicy::new(0, true, MeasurementCheckMode::NoMeasurementCheck, false)
                .unwrap_err()
                .name(),
            "ATTESTATION_POLICY_INVALID"
        );
    }

    #[test]
    fn the_fleet_attestation_vocabulary_is_the_journal_value_domain() {
        for state in [
            FleetAttestationState::NotReported,
            FleetAttestationState::Current,
            FleetAttestationState::Stale,
            FleetAttestationState::Failed,
            FleetAttestationState::Revoked,
        ] {
            assert_eq!(
                FleetAttestationState::from_sql_value(state.sql_value()).expect("round trip"),
                state
            );
        }
        assert_eq!(
            FleetAttestationState::from_sql_value("current").unwrap_err().name(),
            "FLEET_STATE_VALUE_UNKNOWN"
        );
    }

    #[test]
    fn the_hardware_posture_statement_stays_pending() {
        assert!(HARDWARE_POSTURE_STATEMENT.contains("PENDING PILOT HARDWARE SPEC"));
        assert!(HARDWARE_POSTURE_STATEMENT.contains("cannot detect a compromised reporter"));
        assert!(HARDWARE_POSTURE_STATEMENT.contains("No root of trust is assumed"));
    }
}
