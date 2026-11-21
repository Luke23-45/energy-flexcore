//! FIM-F13.02/FIM-F13.04/FIM-F13.05 — WRITTEN-UNRUN integration tests for
//! the PnC OCPP 2.1 certificate-flow handlers: the flow state transitions
//! on fixtures, the five record-41 §3 gate-condition refusals (each naming
//! the unmet condition), and the trust-store validation (record 41 §2-§3;
//! the FIM hard boundary: execution is a HUMAN GATE).

use flexcore_ocpp_adapter::model::{CertificateSignedStatus, CertificateType, InstallCertificateRequest};
use flexcore_ocpp_adapter::pnc_certificate_flows::{
    Get15118EVCertificateRequest, PncCertificateFlowState, PncCertificateSurface,
    PncGateCondition, PncGateConditions, PncGateState, PncRelayOutcome, CERTIFICATE_SIGNED,
    GET_15118_EV_CERTIFICATE, INSTALL_CERTIFICATE,
};
use flexcore_ocpp_adapter::pnc_trust_store::{TrustAnchorRecord, V2gRootCaTrustStore};

const BASE: i64 = 1_700_000_000_000;
const ANCHOR_ROOT_DER: &[u8] = &[0x30, 0x03, 0x02, 0x01, 0x07];

/// The fixture trust store: ONE injected test-grade anchor.
fn anchored_store() -> V2gRootCaTrustStore {
    let mut store = V2gRootCaTrustStore::empty();
    store
        .add_anchor(
            TrustAnchorRecord::new(
                "v2g-root-1".to_string(),
                ANCHOR_ROOT_DER.to_vec(),
                "v2g-pki-operator-contract-1".to_string(),
                BASE - 1_000,
                BASE + 100_000,
                BASE,
            )
            .expect("valid anchor"),
        )
        .expect("added");
    store
}

/// The fixture gate record with ALL FIVE conditions recorded MET (with
/// evidence references) — the only configuration under which a flow may
/// proceed.
fn met_gates() -> PncGateConditions {
    let references = [
        "pki-operator:V2G-PKI-OPS-CONTRACT-1",
        "anchor-distribution:agreement-ref-1",
        "ocsp:responder-contract-ref-1",
        "oca-text-verification:pass-ref-1",
        "d5:closure-record-ref-1",
    ];
    let mut gates = PncGateConditions::unmet();
    for (index, condition) in PncGateCondition::ALL.into_iter().enumerate() {
        gates
            .record_met(condition, references[index].to_string())
            .expect("recorded");
    }
    gates
}

fn relay_request() -> Get15118EVCertificateRequest {
    Get15118EVCertificateRequest {
        action: Some("Install".to_string()),
        certificate_schema_label: None,
    }
}

#[test]
fn the_shipped_gate_record_is_all_unmet_and_refuses_naming_the_condition() {
    // The ONLY gate record this repository ships: ALL FIVE UNMET.
    let gates = PncGateConditions::unmet();
    for condition in PncGateCondition::ALL {
        assert_eq!(gates.state(condition), &PncGateState::Unmet);
    }
    let store = anchored_store();
    let mut surface = PncCertificateSurface::new();
    let error = surface
        .handle_get_15118_ev_certificate(&store, &gates, "depot-cp-001", &relay_request(), BASE)
        .unwrap_err();
    assert_eq!(error.name(), "PNC_GATE_CONDITION_UNMET");
    assert_eq!(
        error.gate_condition().map(PncGateCondition::name),
        Some("PNC_GATE_PKI_OPERATOR_NAMED"),
        "the FIRST unmet condition in record-41 §3 order is the named one"
    );
    // Refusing with the FIRST condition met advances the named condition.
    let mut gates = PncGateConditions::unmet();
    gates
        .record_met(PncGateCondition::NamedPkiOperator, "pki-operator-ref".to_string())
        .expect("recorded");
    let error = surface
        .handle_get_15118_ev_certificate(&store, &gates, "depot-cp-001", &relay_request(), BASE)
        .unwrap_err();
    assert_eq!(
        error.gate_condition().map(PncGateCondition::name),
        Some("PNC_GATE_TRUST_ANCHOR_DISTRIBUTION_AGREED")
    );
    // Every condition names its record-41 §3 clause and its human gate.
    for condition in PncGateCondition::ALL {
        assert!(condition.record_41_clause().starts_with("record-41 §3."));
        assert!(!condition.human_gate_statement().is_empty());
    }
}

#[test]
fn the_relay_refuses_without_anchors_and_records_without_routing_when_open() {
    let gates = met_gates();
    // An EMPTY trust store refuses even with all gates met (fail-closed).
    let mut surface = PncCertificateSurface::new();
    assert_eq!(
        surface
            .handle_get_15118_ev_certificate(
                &V2gRootCaTrustStore::empty(),
                &gates,
                "depot-cp-001",
                &relay_request(),
                BASE,
            )
            .unwrap_err()
            .name(),
        "PNC_TRUST_STORE_REFUSED"
    );
    // Whitespace-declared labels are refused.
    let mut surface = PncCertificateSurface::new();
    let whitespace = Get15118EVCertificateRequest {
        action: Some("  ".to_string()),
        certificate_schema_label: None,
    };
    assert_eq!(
        surface
            .handle_get_15118_ev_certificate(&anchored_store(), &gates, "depot-cp-001", &whitespace, BASE)
            .unwrap_err()
            .name(),
        "PNC_FIELD_EMPTY"
    );
    // Open gates + anchored store: the request is RECORDED (and nothing is
    // routed — this program implements no PKI routing).
    let mut surface = PncCertificateSurface::new();
    let record = surface
        .handle_get_15118_ev_certificate(&anchored_store(), &gates, "depot-cp-001", &relay_request(), BASE)
        .expect("recorded");
    assert_eq!(record.action, GET_15118_EV_CERTIFICATE);
    assert_eq!(record.outcome, PncRelayOutcome::RecordedForPkiRouting);
    let flow = surface
        .flow("depot-cp-001", CertificateType::V2gCertificate)
        .expect("flow");
    assert_eq!(flow.state.name(), "REQUESTED");
    // A second open while the flow is open is refused.
    assert!(surface
        .handle_get_15118_ev_certificate(&anchored_store(), &gates, "depot-cp-001", &relay_request(), BASE)
        .is_err());
}

#[test]
fn the_signed_leg_walks_requested_signed_available_installed_and_rejected() {
    let gates = met_gates();
    let mut surface = PncCertificateSurface::new();
    assert_eq!(CERTIFICATE_SIGNED, "CertificateSigned");

    // CertificateSigned without an open flow: the named state refusal.
    assert_eq!(
        surface
            .handle_certificate_signed(
                &gates,
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
            &anchored_store(),
            &gates,
            "depot-cp-001",
            &relay_request(),
            BASE,
        )
        .expect("recorded");
    // The gates are still checked on the signed leg.
    assert_eq!(
        surface
            .handle_certificate_signed(
                &PncGateConditions::unmet(),
                "depot-cp-001",
                CertificateType::V2gCertificate,
                "chain-label-1".to_string(),
            )
            .unwrap_err()
            .name(),
        "PNC_GATE_CONDITION_UNMET"
    );
    surface
        .handle_certificate_signed(
            &gates,
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
    // Accepted confirmation → INSTALLED.
    surface
        .record_certificate_signed_confirmation(
            "depot-cp-001",
            CertificateType::V2gCertificate,
            CertificateSignedStatus::Accepted,
            String::new(),
        )
        .expect("installed");
    match &surface
        .flow("depot-cp-001", CertificateType::V2gCertificate)
        .expect("flow")
        .state
    {
        PncCertificateFlowState::Installed { certificate_label } => {
            assert_eq!(certificate_label, "chain-label-1");
        }
        other => panic!("expected INSTALLED, got {other:?}"),
    }
    // The Rejected confirmation ends the flow REJECTED with a reason.
    surface
        .handle_get_15118_ev_certificate(
            &anchored_store(),
            &gates,
            "depot-cp-002",
            &relay_request(),
            BASE,
        )
        .expect("recorded");
    surface
        .handle_certificate_signed(
            &gates,
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
    // A rejected flow may be re-opened (the retry path).
    surface
        .handle_get_15118_ev_certificate(
            &anchored_store(),
            &gates,
            "depot-cp-002",
            &relay_request(),
            BASE,
        )
        .expect("re-opened after rejection");
}

#[test]
fn the_install_leg_requires_a_trust_store_byte_equality_and_confirms_by_name() {
    let gates = met_gates();
    assert_eq!(INSTALL_CERTIFICATE, "InstallCertificate");
    let mut surface = PncCertificateSurface::new();
    let request = InstallCertificateRequest {
        certificate_type: CertificateType::V2gRootCertificate,
        certificate: "v2g-root-label".to_string(),
    };
    // A certificate whose bytes match NO anchor: the named refusal.
    assert_eq!(
        surface
            .handle_install_certificate(
                &anchored_store(),
                &gates,
                "depot-cp-001",
                &request,
                &[0x30, 0x03, 0x02, 0x01, 0x0c],
                BASE,
            )
            .unwrap_err()
            .name(),
        "PNC_INSTALL_CERTIFICATE_NOT_ANCHORED"
    );
    // The gates are still checked first.
    assert_eq!(
        surface
            .handle_install_certificate(
                &anchored_store(),
                &PncGateConditions::unmet(),
                "depot-cp-001",
                &request,
                ANCHOR_ROOT_DER,
                BASE,
            )
            .unwrap_err()
            .name(),
        "PNC_GATE_CONDITION_UNMET"
    );
    // The byte-equal certificate records with its anchor label.
    surface
        .handle_install_certificate(
            &anchored_store(),
            &gates,
            "depot-cp-001",
            &request,
            ANCHOR_ROOT_DER,
            BASE,
        )
        .expect("recorded");
    let record = surface
        .install_record("depot-cp-001", CertificateType::V2gRootCertificate)
        .expect("install record");
    assert_eq!(record.anchor_label, "v2g-root-1");
    assert!(record.confirmation.is_none());
    // The charge point's rejection is recorded with a reason.
    surface
        .record_install_certificate_confirmation(
            "depot-cp-001",
            CertificateType::V2gRootCertificate,
            false,
            String::new(),
        )
        .expect("recorded");
    let record = surface
        .install_record("depot-cp-001", CertificateType::V2gRootCertificate)
        .expect("install record");
    assert_eq!(record.confirmation, Some(false));
    assert!(record
        .rejection_reason
        .as_deref()
        .expect("reason")
        .contains("rejected"));
    // A second confirmation is the named state refusal.
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
