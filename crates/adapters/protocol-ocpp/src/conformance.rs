// P17.14 — The OCPP conformance use-case matrix SUBSET (ADDITIVE module;
// wave 2 Agent A).
//
// Decision record:
// `docs/phases-11-20/phase-17-ev-fleet-charging/records/43_p17_14_ocpp_conformance_suite.md`.
//
// WHAT THIS MODULE IS:
//
// - **The declared use-case matrix SUBSET** the written-unrun conformance
//   suite (`tests/conformance_suite.rs`) exercises: provisioning,
//   transactions, smart charging, security events, and firmware status —
//   the families the phase-17 plan §2 row P17.14 names — plus the negative
//   family (malformed frames, subprotocol mismatch, certificate failures).
//   Every use case is keyed (`UC-…`) so a lab run can pre-register against
//   exactly these ids (the X-8 pre-registration discipline; conventions
//   §5) and record its outcome separately from the written expectation.
// - **The shared negative-case vectors.** The malformed-frame vector below
//   RESTATES the `tests/framing_codec.rs` negative cases (the wave-1
//   close-out handoff: the conformance suite reuses them as its
//   malformed-frame subset — integration-test files cannot import each
//   other, so the reuse is by restatement with a citation, and a mismatch
//   between the two vectors is a recorded edit). The subprotocol-mismatch
//   and certificate-failure vectors carry the same discipline.
//
// WHAT THIS MODULE IS NOT — the honesty boundary, stated once:
//
// - **Not a conformance claim.** OCA/DEKRA-class OCPP conformance is a
//   human-gated lab process with fees, sign-offs, and the OCA's own test
//   harness (phase-17 plan risk 4; conventions §7.3). Nothing here has
//   ever RUN, no lab was ever engaged, and no certification,
//   qualification, or interoperability claim of any kind is made. The
//   matrix is EVIDENCE PREPARATION, never a result.
// - **Not the OCA test-case set.** The use-case ids below are THIS
//   PROGRAM's subset keys, not OCA certification-profile case ids; the
//   mapping to the OCA's official cases is UNVERIFIED until checked
//   against the OCA text (and the Edition 2 parts 5/6 certification-
//   profile contents, whose publication-date conflict is recorded in
//   `ledger/phase-06-edge-protocol/RESEARCH.md` §3).
// - **Not an adapter widening.** The matrix exercises the adapter's
//   CLOSED vocabulary and surfaces exactly as wave 1 wrote them; no
//   action, subprotocol, or state was added for the suite's sake
//   (P17-E-1 stays OPEN; the subprotocol set is not widened).

use crate::csms::{CsmsLimits, CsmsSecurityRequirements, CsmsServer};
use crate::framing::FramingError;

/// The honest-status statement the suite and every record carry.
pub const WRITTEN_UNRUN_STATEMENT: &str =
    "WRITTEN-UNRUN: the conformance suite is written test code that has \
never compiled or executed; execution and certification are HUMAN LAB \
GATES. Zero conformance, certification, qualification, or \
interoperability claims are made anywhere.";

/// The matrix-scope statement (a subset, keyed for pre-registration).
pub const MATRIX_SCOPE_STATEMENT: &str =
    "This is a SUBSET of the OCA conformance landscape: the families the \
phase-17 plan names (provisioning, transactions, smart charging, security \
events, firmware status) as THIS PROGRAM's keyed use cases. The mapping to \
the OCA's official certification test cases is UNVERIFIED until checked \
against the OCA text; the Edition 2 parts 5/6 certification-profile \
contents carry an unresolved publication-date conflict (phase-06 research \
§3 open verification task).";

/// The conformance family of a use case (the plan §2 subset + the negative
/// family).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConformanceFamily {
    /// Boot / status / heartbeat provisioning (P17.04 surfaces).
    Provisioning,
    /// TransactionEvent lifecycle records (P17.04 surfaces).
    Transactions,
    /// SetChargingProfile relay + reconciliation (P17.05 surfaces).
    SmartCharging,
    /// SecurityEventNotification ingestion + certificate flows (P17.03
    /// surfaces).
    SecurityEvents,
    /// Firmware status: OUTSIDE the closed handled vocabulary — the
    /// adapter's named fail-closed refusal IS the covered behaviour (the
    /// wire-level translation to an OCPP error is an UNVERIFIED
    /// transport-seam check).
    FirmwareStatus,
    /// Negative: malformed frames, subprotocol mismatch, certificate
    /// failures.
    Negative,
}

impl ConformanceFamily {
    /// Canonical name for records.
    pub const fn name(self) -> &'static str {
        match self {
            ConformanceFamily::Provisioning => "PROVISIONING",
            ConformanceFamily::Transactions => "TRANSACTIONS",
            ConformanceFamily::SmartCharging => "SMART_CHARGING",
            ConformanceFamily::SecurityEvents => "SECURITY_EVENTS",
            ConformanceFamily::FirmwareStatus => "FIRMWARE_STATUS",
            ConformanceFamily::Negative => "NEGATIVE",
        }
    }
}

/// One use case of the declared subset: a keyed expectation the written-
/// unrun suite pins against the adapter's real surfaces. The expectation
/// text is the RECORDED behaviour of THIS adapter (hand-checked against
/// the wave-1 module code); the OCA-text check for the underlying protocol
/// detail remains the plan §7.5 UNVERIFIED register's job.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConformanceUseCase {
    /// This program's subset key (`UC-<FAMILY>-<NN>`), pre-registration
    /// ready.
    pub id: &'static str,
    /// The family.
    pub family: ConformanceFamily,
    /// The one-line summary.
    pub summary: &'static str,
    /// The adapter surfaces the suite exercises (module-path hints).
    pub surfaces: &'static [&'static str],
}

/// The declared use-case matrix SUBSET (in family order; the suite
/// `tests/conformance_suite.rs` pins every row against real code).
pub const CONFORMANCE_USE_CASES: &[ConformanceUseCase] = &[
    // --- Provisioning -----------------------------------------------------
    ConformanceUseCase {
        id: "UC-PROV-01",
        family: ConformanceFamily::Provisioning,
        summary: "a BootNotification CALL decodes through the codec and maps into a domain boot record for an allowlisted station (the ACCEPT decision is the provisioning policy's, fail-closed DEFAULT)",
        surfaces: &["framing::OcppFrame", "model::BootNotificationRequest", "provisioning::map_accepted_boot"],
    },
    ConformanceUseCase {
        id: "UC-PROV-02",
        family: ConformanceFamily::Provisioning,
        summary: "a StatusNotification maps through the DOMAIN's declared table and the link state is untouched by a status message",
        surfaces: &["provisioning::apply_status_notification", "domain::evse_registry::map_status_to_operational_state"],
    },
    ConformanceUseCase {
        id: "UC-PROV-03",
        family: ConformanceFamily::Provisioning,
        summary: "an unmapped status name is refused by the domain mapping (never approximated) and a status for an unknown EVSE id is refused (never auto-created)",
        surfaces: &["domain::evse_registry::map_status_to_operational_state", "provisioning::apply_status_notification"],
    },
    ConformanceUseCase {
        id: "UC-PROV-04",
        family: ConformanceFamily::Provisioning,
        summary: "a Heartbeat is liveness evidence only: no registry call exists for heartbeats (absence is structural)",
        surfaces: &["model::HeartbeatRequest"],
    },
    ConformanceUseCase {
        id: "UC-PROV-05",
        family: ConformanceFamily::Provisioning,
        summary: "a link loss touches ONLY the link state: the last REPORTED operational state stands and the rule statement travels verbatim (connection loss is NOT asset unavailability)",
        surfaces: &["provisioning::apply_link_loss", "domain::evse_registry::CONNECTION_LOSS_IS_NOT_UNAVAILABILITY"],
    },
    // --- Transactions -------------------------------------------------------
    ConformanceUseCase {
        id: "UC-TXN-01",
        family: ConformanceFamily::Transactions,
        summary: "TransactionEvent Started opens the transaction as Active; Updated keeps it Active and refreshes energy/timestamps",
        surfaces: &["provisioning::apply_transaction_event", "domain::evse_registry::EvseTransactionState"],
    },
    ConformanceUseCase {
        id: "UC-TXN-02",
        family: ConformanceFamily::Transactions,
        summary: "TransactionEvent Ended closes the transaction",
        surfaces: &["provisioning::apply_transaction_event"],
    },
    ConformanceUseCase {
        id: "UC-TXN-03",
        family: ConformanceFamily::Transactions,
        summary: "an update or an end for a transaction that never started is refused — the registry never invents a transaction to absorb an event",
        surfaces: &["provisioning::apply_transaction_event", "domain::evse_registry"],
    },
    ConformanceUseCase {
        id: "UC-TXN-04",
        family: ConformanceFamily::Transactions,
        summary: "a negative-energy or empty-transaction-id payload is refused by the model validation before any registry step",
        surfaces: &["model::TransactionEventRequest::validate"],
    },
    // --- Smart charging -----------------------------------------------------
    ConformanceUseCase {
        id: "UC-SC-01",
        family: ConformanceFamily::SmartCharging,
        summary: "the wire payload is built ONLY from an ISSUED lifecycle state (the command-relay-only rule, enforced by typing); every other state is refused with the found state named",
        surfaces: &["charging::into_relay_request", "domain::command_state::CommandState"],
    },
    ConformanceUseCase {
        id: "UC-SC-02",
        family: ConformanceFamily::SmartCharging,
        summary: "an Accepted SetChargingProfile response travels with the stored-not-applied statement: storage evidence, never application evidence",
        surfaces: &["charging::record_accepted_status", "model::SetChargingProfileStatus::STORED_NOT_APPLIED_STATEMENT"],
    },
    ConformanceUseCase {
        id: "UC-SC-03",
        family: ConformanceFamily::SmartCharging,
        summary: "the GetCompositeSchedule response decodes like-with-like (unit mismatch refused) and a drifted composite yields a NAMED mismatch — never a silent pass, never an adapter-side retry",
        surfaces: &["charging::parse_composite_schedule_response", "domain::ev_charging_composer::reconcile_composite_schedule"],
    },
    ConformanceUseCase {
        id: "UC-SC-04",
        family: ConformanceFamily::SmartCharging,
        summary: "the CSMS-initiated SetChargingProfile is queued once per direction (the one-outstanding-CALL discipline) and an overflow refuses with a named error, never a silent drop",
        surfaces: &["csms::CsmsServer::queue_outbound_call", "framing::CallTracker"],
    },
    // --- Security events ------------------------------------------------------
    ConformanceUseCase {
        id: "UC-SEC-01",
        family: ConformanceFamily::SecurityEvents,
        summary: "a SecurityEventNotification ingests as a bounded EVENT-LOG record with the DECLARED classification; ingestion triggers NO automatic reaction",
        surfaces: &["security::SecurityEventLedger", "security::classify_security_event"],
    },
    ConformanceUseCase {
        id: "UC-SEC-02",
        family: ConformanceFamily::SecurityEvents,
        summary: "an unknown event type classifies Unclassified — explicitly named, never guessed as benign or critical; an empty type is refused",
        surfaces: &["security::classify_security_event", "security::SecurityEventSeverity::Unclassified"],
    },
    ConformanceUseCase {
        id: "UC-SEC-03",
        family: ConformanceFamily::SecurityEvents,
        summary: "the certificate flow walks REQUESTED → SIGNED_AVAILABLE → INSTALLED with the issuance act EXTERNAL (a human/ops gate — no CA is stood up); a rejected confirmation ends the flow as Rejected with a reason",
        surfaces: &["security::CertificateFlowLedger", "security::CertificateFlowState"],
    },
    ConformanceUseCase {
        id: "UC-SEC-04",
        family: ConformanceFamily::SecurityEvents,
        summary: "an unproofed registration is refused (identity proofing at registration, fail-closed) and an open certificate flow refuses a second open",
        surfaces: &["security::IdentityProofingLedger", "security::SecurityError::NotProofed"],
    },
    // --- Firmware status -------------------------------------------------------
    ConformanceUseCase {
        id: "UC-FW-01",
        family: ConformanceFamily::FirmwareStatus,
        summary: "FirmwareStatusNotification is OUTSIDE the closed handled vocabulary: the decode refuses WHOLE with the named UnknownAction error (no loose pass-through); whether the wire-level translation is an OCPP NotImplemented CALLERROR is an UNVERIFIED transport-seam check",
        surfaces: &["framing::Action::parse", "framing::FramingError::UnknownAction"],
    },
    ConformanceUseCase {
        id: "UC-FW-02",
        family: ConformanceFamily::FirmwareStatus,
        summary: "no firmware-update action exists in the adapter's vocabulary and none was added for the suite (the closed vocabulary is pinned)",
        surfaces: &["framing::Action"],
    },
    // --- Negative ----------------------------------------------------------------
    ConformanceUseCase {
        id: "UC-NEG-01",
        family: ConformanceFamily::Negative,
        summary: "every malformed frame in the shared negative vector is refused WHOLE with the named error (the tests/framing_codec.rs negatives, restated with citation)",
        surfaces: &["framing::OcppFrame::decode", "malformed_frame_negatives"],
    },
    ConformanceUseCase {
        id: "UC-NEG-02",
        family: ConformanceFamily::Negative,
        summary: "a subprotocol mismatch (an unsupported offer, including ocpp1.6 with the P17-E-1 reason, and an empty offer) fails WHOLE with the named refusal — no unspecified default",
        surfaces: &["session::negotiate", "session::Subprotocol::parse", "subprotocol_mismatch_negatives"],
    },
    ConformanceUseCase {
        id: "UC-NEG-03",
        family: ConformanceFamily::Negative,
        summary: "certificate failures refuse with named errors: an empty CSR label, a second open on an open flow, a signed-send on an ABSENT flow, an unproofed registration, and an empty trust-anchor set",
        surfaces: &["certificate_failure_negatives", "security::SecurityError", "csms::CsmsError::InvalidSecurityRequirements"],
    },
    ConformanceUseCase {
        id: "UC-NEG-04",
        family: ConformanceFamily::Negative,
        summary: "an unmatched reply (CALLRESULT/CALLERROR for a CALL the tracker does not hold) is refused — never applied",
        surfaces: &["framing::CallTracker::track_reply", "csms::CsmsServer::handle_inbound_text"],
    },
];

/// The malformed-frame negative vector: (wire text, expected named refusal
/// as a debug string). RESTATES the `tests/framing_codec.rs`
/// `negative_framing_cases_refuse_whole_with_named_errors` cases (the
/// wave-1 close-out handoff; integration-test files cannot import each
/// other — the reuse is by restatement with this citation, and a
/// divergence between the two vectors is a recorded edit, never a silent
/// one).
pub fn malformed_frame_negatives() -> Vec<(&'static str, String)> {
    vec![
        ("", format!("{:?}", FramingError::MalformedJson)),
        ("[", format!("{:?}", FramingError::MalformedJson)),
        ("[]", format!("{:?}", FramingError::InvalidMessageType)),
        ("[2]", format!("{:?}", FramingError::CallArity { found: 1 })),
        (
            r#"[2,1,"Heartbeat",{}]"#,
            format!("{:?}", FramingError::ExpectedString { field: "call.uniqueId" }),
        ),
        (
            r#"[2,"","Heartbeat",{}]"#,
            format!("{:?}", FramingError::EmptyUniqueId),
        ),
        (
            r#"[2,"id","LegacyAction",{}]"#,
            format!(
                "{:?}",
                FramingError::UnknownAction { name: "LegacyAction".to_string() }
            ),
        ),
        (
            r#"[3,"id"]"#,
            format!("{:?}", FramingError::CallResultArity { found: 2 }),
        ),
        (
            r#"[4,"id","FormationViolation"]"#,
            format!("{:?}", FramingError::CallErrorArity { found: 3 }),
        ),
    ]
}

/// The subprotocol-mismatch negative vector: (offered list, expected
/// refusal summary). The `ocpp1.6` row carries the P17-E-1 reason (the
/// recorded DEFAULT (a): 2.0.1 baseline + 2.1 additive; 1.6 is NOT
/// backward compatible with 2.0.1 — adding it would be the dual-stack
/// option). P17-E-1 stays OPEN; the subprotocol set is NOT widened here.
pub fn subprotocol_mismatch_negatives() -> Vec<(Vec<&'static str>, &'static str)> {
    vec![
        (
            vec!["ocpp1.6"],
            "P17-E-1",
        ),
        (
            vec!["ocpp2.0"],
            "none of the offered subprotocols is supported",
        ),
        (
            vec!["ocpp1.6", "ocpp2.0"],
            "none of the offered subprotocols is supported",
        ),
        // The empty offer: the named NoSubprotocolOffered refusal (a
        // connection without a negotiated protocol is refused, never
        // defaulted).
        (vec![], "no subprotocol offered"),
    ]
}

/// Maps one vector row onto the real negotiation error (the suite's helper
/// — keeping the expectation mapping in ONE place; every row must be
/// refused by the real `negotiate` function, the empty offer included).
pub fn expected_negotiation_refusal(offered: &[&str]) -> String {
    let list: Vec<String> = offered.iter().map(|s| s.to_string()).collect();
    match crate::session::negotiate(&list) {
        Err(error) => error.to_string(),
        Ok(_) => panic!("the negative vector must only contain refused offers"),
    }
}

/// The certificate-failure negative vector: (description, expected named
/// refusal). Each row names the rule the suite asserts against the real
/// security surfaces.
pub fn certificate_failure_negatives() -> Vec<(&'static str, &'static str)> {
    vec![
        (
            "an empty CSR label is refused (an unsigned empty request is never defaulted)",
            "field 'csr' is empty",
        ),
        (
            "a second open on an OPEN certificate flow is refused (one flow per target at a time)",
            "is not permitted there",
        ),
        (
            "a signed-send on an ABSENT flow is refused (the flow must exist in REQUESTED)",
            "ABSENT",
        ),
        (
            "an unproofed registration is refused (identity proofing at registration, fail-closed)",
            "fail-closed",
        ),
        (
            "an empty trust-anchor set is refused (an endpoint with no trust anchor accepts nothing — and must not silently accept everything instead)",
            "trust-anchor set is empty",
        ),
    ]
}

/// The server constructor helper the suite composes (deterministic,
/// non-empty trust anchors, sane caps) — the same construction every
/// wave-1 test uses, in one place. Every construction failure is carried
/// as its message (the errors are all named).
pub fn suite_server() -> Result<CsmsServer, String> {
    let manifest = crate::scope::csms_manifest().map_err(|error| error.to_string())?;
    let limits = CsmsLimits::new(4, 8).map_err(|error| error.to_string())?;
    let security =
        CsmsSecurityRequirements::new(vec!["evse-device-ca".to_string()], true)
            .map_err(|error| error.to_string())?;
    CsmsServer::new(manifest, limits, security).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_matrix_declares_every_named_family() {
        let families: Vec<ConformanceFamily> = CONFORMANCE_USE_CASES
            .iter()
            .map(|uc| uc.family)
            .collect();
        for family in [
            ConformanceFamily::Provisioning,
            ConformanceFamily::Transactions,
            ConformanceFamily::SmartCharging,
            ConformanceFamily::SecurityEvents,
            ConformanceFamily::FirmwareStatus,
            ConformanceFamily::Negative,
        ] {
            assert!(
                families.contains(&family),
                "family {} missing from the matrix",
                family.name()
            );
        }
    }

    #[test]
    fn every_use_case_id_is_unique_and_prefixed() {
        let mut seen: Vec<&str> = Vec::new();
        for uc in CONFORMANCE_USE_CASES {
            assert!(uc.id.starts_with("UC-"), "id {}", uc.id);
            assert!(!seen.contains(&uc.id), "duplicate id {}", uc.id);
            seen.push(uc.id);
        }
    }

    #[test]
    fn the_negative_vectors_carry_the_expected_refusal_text() {
        // Malformed frames: every row's text matches the codec's debug
        // refusal.
        for (text, expected) in malformed_frame_negatives() {
            let error = crate::framing::OcppFrame::decode(text).expect_err(text);
            assert_eq!(format!("{error:?}"), expected, "text: {text}");
        }
        // Subprotocol mismatches: every row is refused, with the expected
        // fragment in the message.
        for (offered, fragment) in subprotocol_mismatch_negatives() {
            let expected = expected_negotiation_refusal(&offered);
            assert!(
                expected.contains(fragment),
                "offered {offered:?}: expected fragment '{fragment}' in '{expected}'"
            );
        }
    }

    #[test]
    fn the_honest_status_statements_travel() {
        assert!(WRITTEN_UNRUN_STATEMENT.contains("never compiled or executed"));
        assert!(WRITTEN_UNRUN_STATEMENT.contains("HUMAN LAB GATES"));
        assert!(MATRIX_SCOPE_STATEMENT.contains("SUBSET"));
        assert!(MATRIX_SCOPE_STATEMENT.contains("UNVERIFIED"));
    }
}
