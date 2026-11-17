//! P17.01 — WRITTEN-UNRUN integration tests for the OCPP-J framing codec
//! and the CSMS session/registry surfaces (decision record
//! 01_p17_01_ocpp_csms_adapter_architecture.md; conventions §1.3: tests
//! are written, their execution is a human gate).
//!
//! Every expected value below is hand-computed against the RECORDED
//! (UNVERIFIED) wire shapes — these tests pin the recorded spellings so a
//! change is a conscious edit, never a silent reinterpretation. Nothing
//! here connects to a device; everything runs against in-memory values.

use flexcore_ocpp_adapter::csms::{
    CsmsLimits, CsmsSecurityRequirements, CsmsServer, UnwiredCsmsTransport,
    CSMS_TRANSPORT_SEAM_NOT_WIRED,
};
use flexcore_ocpp_adapter::framing::{
    CallTracker, CallTrackError, FramingError, OcppFrame, UniqueId,
};
use flexcore_ocpp_adapter::model::ChargePointId;
use flexcore_ocpp_adapter::scope::csms_manifest;
use flexcore_ocpp_adapter::session::{negotiate, Session, SessionEndReason, Subprotocol};
use flexcore_protocol_ports::connection::ConnectionState;
use flexcore_protocol_ports::manifest::{AuthorityDeclaration, DeclaredCapability};

fn security() -> CsmsSecurityRequirements {
    CsmsSecurityRequirements::new(vec!["evse-device-ca".to_string()], true).expect("valid")
}

fn server() -> CsmsServer {
    CsmsServer::new(
        csms_manifest().expect("valid manifest"),
        CsmsLimits::new(8, 8).expect("valid limits"),
        security(),
    )
    .expect("valid server")
}

#[test]
fn the_call_frame_is_exactly_the_recorded_four_element_array() {
    // CALL = [2, uniqueId, action, payload] (UNVERIFIED — pinned). The
    // PAYLOAD object's key order is serde_json's (BTreeMap, alphabetical
    // by default) — the pinned shape is the ARRAY shape, never a key
    // order.
    let unique_id = UniqueId::new("0123456789abcdef".to_string()).expect("valid");
    let payload = serde_json::json!({
        "vendorName": "Vendor",
        "chargePointModel": "Model-1"
    });
    let frame = OcppFrame::Call {
        unique_id: unique_id.clone(),
        action: flexcore_ocpp_adapter::framing::Action::BootNotification,
        payload,
    };
    let text = frame.encode().expect("encodes");
    let value: serde_json::Value = serde_json::from_str(&text).expect("json");
    let items = value.as_array().expect("array");
    assert_eq!(items.len(), 4);
    assert_eq!(items[0], serde_json::json!(2));
    assert_eq!(items[1], serde_json::json!("0123456789abcdef"));
    assert_eq!(items[2], serde_json::json!("BootNotification"));
    assert_eq!(items[3]["vendorName"], serde_json::json!("Vendor"));
    assert_eq!(
        items[3]["chargePointModel"],
        serde_json::json!("Model-1")
    );
    let decoded = OcppFrame::decode(&text).expect("decodes");
    assert_eq!(decoded, frame);
}

#[test]
fn the_call_result_frame_is_exactly_the_recorded_three_element_array() {
    let unique_id = UniqueId::new("r-1".to_string()).expect("valid");
    let frame = OcppFrame::CallResult {
        unique_id,
        payload: serde_json::json!({"currentTime": 0, "interval": 60, "status": "Accepted"}),
    };
    let text = frame.encode().expect("encodes");
    let value: serde_json::Value = serde_json::from_str(&text).expect("json");
    assert_eq!(value[0], serde_json::json!(3));
    assert_eq!(value[1], serde_json::json!("r-1"));
    assert_eq!(value[2]["status"], serde_json::json!("Accepted"));
    assert_eq!(value.as_array().expect("array").len(), 3);
}

#[test]
fn the_call_error_frame_is_exactly_the_recorded_five_element_array() {
    let unique_id = UniqueId::new("e-1".to_string()).expect("valid");
    let frame = OcppFrame::CallError {
        unique_id,
        error_code: flexcore_ocpp_adapter::framing::CallErrorCode::NotImplemented,
        error_description: "not implemented".to_string(),
        error_details: serde_json::json!({}),
    };
    let text = frame.encode().expect("encodes");
    let value: serde_json::Value = serde_json::from_str(&text).expect("json");
    assert_eq!(value[0], serde_json::json!(4));
    assert_eq!(value[2], serde_json::json!("NotImplemented"));
    assert_eq!(value.as_array().expect("array").len(), 5);
}

#[test]
fn negative_framing_cases_refuse_whole_with_named_errors() {
    let cases: Vec<(&str, FramingError)> = vec![
        ("", FramingError::MalformedJson),
        ("[", FramingError::MalformedJson),
        ("[]", FramingError::InvalidMessageType),
        ("[2]", FramingError::CallArity { found: 1 }),
        ("[2,1,\"Heartbeat\",{}]", FramingError::ExpectedString { field: "call.uniqueId" }),
        ("[2,\"\",\"Heartbeat\",{}]", FramingError::EmptyUniqueId),
        (
            "[2,\"id\",\"LegacyAction\",{}]",
            FramingError::UnknownAction {
                name: "LegacyAction".to_string(),
            },
        ),
        ("[3,\"id\"]", FramingError::CallResultArity { found: 2 }),
        ("[4,\"id\",\"FormationViolation\"]", FramingError::CallErrorArity { found: 3 }),
    ];
    for (text, expected) in cases {
        assert_eq!(OcppFrame::decode(text), Err(expected), "text: {text}");
    }
}

#[test]
fn the_unwired_transport_refuses_with_the_named_seam_refusal() {
    let mut server = server();
    let transport = UnwiredCsmsTransport;
    let error = Box::new(transport)
        .serve(&mut server)
        .expect_err("the seam is UNRUN by design");
    assert!(error.to_string().contains(CSMS_TRANSPORT_SEAM_NOT_WIRED));
    // The honest state of an unwired transport is Closed.
    assert_eq!(UnwiredCsmsTransport.state(), ConnectionState::Closed);
}

#[test]
fn the_session_walks_negotiation_provisioning_and_end() {
    let mut session = Session::new();
    // The peer offers 1.6 first and 2.0.1 second: 1.6 is skipped (out of
    // scope per P17-E-1 DEFAULT), 2.0.1 is accepted.
    let negotiated = negotiate(&[
        "ocpp1.6".to_string(),
        "ocpp2.0.1".to_string(),
    ])
    .expect("negotiated");
    assert_eq!(negotiated, Subprotocol::Ocpp201);
    session.apply_negotiation(negotiated).expect("negotiated");
    let charge_point = ChargePointId::new("depot-cp-001".to_string()).expect("valid");
    session.apply_boot_accepted(charge_point).expect("provisioned");
    assert!(session.is_provisioned());
    // The end is named and retained for audit.
    session.end(SessionEndReason::TransportClosed {
        reason: "backhaul outage".to_string(),
    });
    assert_eq!(session.state().name(), "ENDED");
    assert!(!session.accepts_frames());
}

#[test]
fn the_tracker_enforces_one_outstanding_call_per_direction() {
    let mut tracker = CallTracker::new();
    let call = UniqueId::new("call-1".to_string()).expect("valid");
    tracker.track_call(call.clone()).expect("tracked");
    let other = UniqueId::new("call-2".to_string()).expect("valid");
    assert_eq!(
        tracker.track_call(other),
        Err(CallTrackError::CallAlreadyOutstanding {
            outstanding: "call-1".to_string()
        })
    );
    tracker.track_reply(&call).expect("matched");
    assert!(!tracker.has_outstanding());
}

#[test]
fn the_server_registry_is_deterministic_and_fail_closed() {
    let mut server = server();
    for name in ["cp-3", "cp-1", "cp-2"] {
        server
            .accept_connection(
                ChargePointId::new(name.to_string()).expect("valid"),
                Subprotocol::Ocpp201,
            )
            .expect("accepted");
    }
    let snapshot = server.connection_snapshot();
    assert_eq!(
        snapshot,
        vec![("cp-1", "NEGOTIATED"), ("cp-2", "NEGOTIATED"), ("cp-3", "NEGOTIATED")]
    );
    // Duplicate identity: refused, never replaced.
    assert!(server
        .accept_connection(
            ChargePointId::new("cp-1".to_string()).expect("valid"),
            Subprotocol::Ocpp201
        )
        .is_err());
}

#[test]
fn the_manifest_declares_relay_only_authority_and_the_recorded_capabilities() {
    let manifest = csms_manifest().expect("valid manifest");
    assert_eq!(
        manifest.authority,
        AuthorityDeclaration::SupervisoryRelayOnly
    );
    assert!(manifest.declares(DeclaredCapability::CommandRelay));
    assert!(manifest.declares(DeclaredCapability::ExternalEventIngest));
    assert!(manifest.declares(DeclaredCapability::BoundedHistory));
    // The scope gate refuses a client-role request: the CSMS is the
    // server role only.
    let scope = &manifest.scope;
    assert!(!scope.covers_role(flexcore_protocol_ports::scope::ProtocolRole::Client));
    // The mandatory limitation record travels (non-empty, with the 1.6
    // exclusion and the no-CA rule).
    let limitations = manifest.coverage_limitation_statements().join("\n");
    assert!(limitations.contains("no_ocpp_1_6"));
    assert!(limitations.contains("no_pki_ca_operations"));
}
