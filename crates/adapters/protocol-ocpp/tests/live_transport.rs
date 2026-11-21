//! FIM-F02.05 — WRITTEN-UNRUN integration tests for the live OCPP CSMS
//! transport stack (feature F-02; conventions §1.3: tests are written,
//! their execution is a human gate).
//!
//! HONESTY BOUNDARY (stated once, binding for every result): this suite is
//! WRITTEN-UNRUN — it has never compiled or executed. RFC 6455 conformance
//! and real charge-point interoperability are HUMAN LAB GATES (the FIM
//! standing decision 1, docs/releases/implementation_ledger/README.md).
//! A green run of this suite, if a human ever runs it, is evidence about
//! THIS adapter's recorded behaviour only — never a conformance,
//! certification, qualification, or interoperability result. Everything
//! below runs against in-memory values and pure functions; no socket is
//! ever opened.
//!
//! Pinned here (ledger FIM-F02.05): the handshake acceptance and refusal,
//! the frame codec round-trips, the boot / heartbeat / charging-profile
//! relay through the seam, and the protocol refusal reason — which is the
//! EXACT P17-E-1 recorded reason string, reused by composition (the
//! refusal text is taken from the real `session::negotiate` error, never
//! restated).

use std::sync::Arc;

use flexcore_domain::command_state::CommandState;
use flexcore_domain::ev_charging_composer::{
    compose_external_constraints_profile, DeviceRateConstraints, DispatchEnvelope,
    DispatchPeriod, RateUnit, SupplyContext,
};
use flexcore_domain::evse_registry::{EvseIdentity, EvseRegistry};
use flexcore_ocpp_adapter::csms::{
    CsmsLimits, CsmsSecurityRequirements, CsmsServer, CsmsTransport, UnwiredCsmsTransport,
    CSMS_TRANSPORT_SEAM_NOT_WIRED,
};
use flexcore_ocpp_adapter::dispatch::CsmsMessageDispatcher;
use flexcore_ocpp_adapter::framing::{Action, OcppFrame, UniqueId};
use flexcore_ocpp_adapter::model::ChargePointId;
use flexcore_ocpp_adapter::provisioning::ProvisioningPolicy;
use flexcore_ocpp_adapter::scope::csms_manifest;
use flexcore_ocpp_adapter::session::{negotiate, Subprotocol};
use flexcore_ocpp_adapter::transport::{LiveCsmsTransport, RelayRequest, RELAY_QUEUE_CAP};
use flexcore_ocpp_adapter::websocket::{
    self, AssemblerEvent, FrameError, HandshakeError, MessageAssembler, Opcode, RefusalStatus,
    MAX_WEBSOCKET_MESSAGE_BYTES,
};

const RFC_EXAMPLE_CLIENT_KEY: &str = "dGhlIHNhbXBsZSBub25jZQ==";
const RFC_EXAMPLE_ACCEPT_KEY: &str = "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=";

fn server() -> CsmsServer {
    CsmsServer::new(
        csms_manifest().expect("valid manifest"),
        CsmsLimits::new(4, 8).expect("valid limits"),
        CsmsSecurityRequirements::new(vec!["evse-device-ca".to_string()], true)
            .expect("valid security"),
    )
    .expect("valid server")
}

fn dispatcher_for(allowed: &str) -> CsmsMessageDispatcher {
    CsmsMessageDispatcher::new(
        EvseRegistry::new(),
        ProvisioningPolicy::allow_only(
            vec![allowed.to_string()],
            "the named depot fleet".to_string(),
        )
        .expect("valid policy"),
        60,
        8,
    )
    .expect("valid dispatcher")
}

fn handshake_request(protocols: &str) -> String {
    let mut request = format!(
        "GET /depot-cp-001 HTTP/1.1\r\n\
         Host: csms.depot.example:443\r\n\
         Upgrade: websocket\r\n\
         Connection: Upgrade\r\n\
         Sec-WebSocket-Key: {RFC_EXAMPLE_CLIENT_KEY}\r\n\
         Sec-WebSocket-Version: 13\r\n"
    );
    if !protocols.is_empty() {
        request.push_str(&format!("Sec-WebSocket-Protocol: {protocols}\r\n"));
    }
    request.push_str("\r\n");
    request
}

// --- Handshake acceptance / refusal (FIM-F02.01) --------------------------

#[test]
fn the_handshake_accepts_the_recorded_subprotocol_with_the_rfc_accept_key() {
    let request = websocket::parse_upgrade_request(handshake_request("ocpp2.0.1").as_bytes())
        .expect("parses");
    let (subprotocol, accept_key) = websocket::negotiate_handshake(&request)
        .expect("the recorded subprotocol set is accepted");
    assert_eq!(subprotocol, Subprotocol::Ocpp201);
    // The RFC 6455 §1.3 example vector, byte-exact.
    assert_eq!(accept_key, RFC_EXAMPLE_ACCEPT_KEY);
    let response = websocket::build_switching_response(subprotocol, &accept_key);
    assert!(response.starts_with("HTTP/1.1 101 Switching Protocols\r\n"));
    assert!(response.contains(&format!("Sec-WebSocket-Accept: {RFC_EXAMPLE_ACCEPT_KEY}\r\n")));
    assert!(response.contains("Sec-WebSocket-Protocol: ocpp2.0.1\r\n"));
    // NO extension is negotiated: permessage-deflate is refused by the
    // header's absence (RFC 6455 §4.2.2.2).
    assert!(!response.contains("Sec-WebSocket-Extensions"));
}

#[test]
fn the_handshake_refusal_carries_the_exact_p17_e_1_recorded_reason() {
    // FIM-F02.02b: a charge point offering only `ocpp1.6` is refused with
    // the P17-E-1 reason — reused by COMPOSITION (the refusal text is the
    // real negotiate() error, byte-identical; no new vocabulary).
    let request = websocket::parse_upgrade_request(handshake_request("ocpp1.6").as_bytes())
        .expect("parses");
    let error = websocket::negotiate_handshake(&request).expect_err("1.6-only refused");
    let (status, detail) = websocket::refusal_for(&error);
    assert_eq!(status, RefusalStatus::BadRequest);
    let recorded = negotiate(&["ocpp1.6".to_string()]).expect_err("the recorded refusal");
    assert_eq!(detail, recorded.to_string());
    // The recorded reason fragments travel verbatim.
    assert!(detail.contains("P17-E-1"));
    assert!(detail.contains("1.6 is NOT backward compatible with 2.0.1"));
    let response = websocket::build_refusal_response(status, &detail);
    assert!(response.starts_with("HTTP/1.1 400 Bad Request\r\n"));
    assert!(response.ends_with(&detail));
}

#[test]
fn the_handshake_refuses_malformed_and_unnegotiated_requests_by_name() {
    // No subprotocol at all: no unspecified default.
    let request =
        websocket::parse_upgrade_request(handshake_request("").as_bytes()).expect("parses");
    assert_eq!(
        websocket::negotiate_handshake(&request),
        Err(HandshakeError::Negotiation(
            flexcore_ocpp_adapter::session::NegotiationError::NoSubprotocolOffered
        ))
    );
    // An unsupported version is answered with 426 and the required
    // version named.
    let bytes = handshake_request("ocpp2.0.1")
        .replace("Sec-WebSocket-Version: 13", "Sec-WebSocket-Version: 8");
    let request = websocket::parse_upgrade_request(bytes.as_bytes()).expect("parses");
    let (status, _) = websocket::refusal_for(
        &websocket::negotiate_handshake(&request).expect_err("version refused"),
    );
    assert_eq!(status, RefusalStatus::UpgradeRequired);
    // A non-GET request is refused.
    let bytes = handshake_request("ocpp2.0.1").replace("GET /", "POST /");
    assert!(websocket::parse_upgrade_request(bytes.as_bytes()).is_err());
    // An invalid accept key is refused.
    let bytes = handshake_request("ocpp2.0.1")
        .replace(RFC_EXAMPLE_CLIENT_KEY, "short");
    let request = websocket::parse_upgrade_request(bytes.as_bytes()).expect("parses");
    assert_eq!(
        websocket::negotiate_handshake(&request),
        Err(HandshakeError::InvalidSecWebSocketKey)
    );
}

// --- Frame codec round-trips (FIM-F02.01) ---------------------------------

#[test]
fn client_frames_round_trip_through_masking_and_extended_lengths() {
    let key = [0x37u8, 0xFA, 0x21, 0x3D];
    let text = r#"[2,"0123456789abcdef","BootNotification",{}]"#;
    let wire = websocket::encode_frame(true, Opcode::Text, text.as_bytes(), Some(key));
    let frame =
        websocket::parse_frame_bytes(&wire, MAX_WEBSOCKET_MESSAGE_BYTES).expect("decodes");
    assert!(frame.fin);
    assert_eq!(frame.opcode, Opcode::Text);
    assert_eq!(frame.payload, text.as_bytes());
    // The extended-16-bit and 64-bit length boundaries.
    for len in [125usize, 126, 65_535, 65_536] {
        let payload = vec![0x41u8; len];
        let wire = websocket::encode_frame(true, Opcode::Text, &payload, Some(key));
        let frame =
            websocket::parse_frame_bytes(&wire, MAX_WEBSOCKET_MESSAGE_BYTES).expect("decodes");
        assert_eq!(frame.payload.len(), len);
    }
    // Server frames are UNMASKED (the shipped write-path discipline).
    let server_wire = websocket::encode_server_text("[3,\"r-1\",{}]");
    let header = websocket::parse_frame_header(&server_wire).expect("header");
    assert!(!header.masked);
}

#[test]
fn frame_protocol_violations_refuse_with_named_errors_and_codes() {
    let key = [0x11u8, 0x22, 0x33, 0x44];
    // An unmasked client data frame fails the connection with 1002
    // (RFC 6455 §5.1).
    let wire = websocket::encode_frame(true, Opcode::Text, b"hi", None);
    assert_eq!(
        websocket::parse_frame_bytes(&wire, MAX_WEBSOCKET_MESSAGE_BYTES),
        Err(FrameError::UnmaskedClientFrame)
    );
    assert_eq!(
        websocket::close_code_for(&FrameError::UnmaskedClientFrame),
        websocket::CLOSE_PROTOCOL_ERROR
    );
    // RSV bits set with no negotiated extension.
    let mut wire = websocket::encode_frame(true, Opcode::Text, b"hi", Some(key));
    wire[0] |= 0x40;
    assert_eq!(
        websocket::parse_frame_bytes(&wire, MAX_WEBSOCKET_MESSAGE_BYTES),
        Err(FrameError::ReservedBitsSet)
    );
    // A binary data frame: OCPP-J is JSON-over-TEXT only, refused with
    // 1003, never guessed at.
    let wire = websocket::encode_frame(true, Opcode::Binary, b"\x00\x01", Some(key));
    assert_eq!(
        websocket::parse_frame_bytes(&wire, MAX_WEBSOCKET_MESSAGE_BYTES),
        Ok(websocket::RawFrame {
            fin: true,
            opcode: Opcode::Binary,
            payload: vec![0x00, 0x01],
        })
    );
    let mut assembler = MessageAssembler::new(MAX_WEBSOCKET_MESSAGE_BYTES).expect("valid");
    assert_eq!(
        assembler.push(true, Opcode::Binary, vec![0x00, 0x01]),
        Err(FrameError::BinaryDataRefused)
    );
    assert_eq!(
        websocket::close_code_for(&FrameError::BinaryDataRefused),
        websocket::CLOSE_UNSUPPORTED_DATA
    );
}

#[test]
fn fragmented_messages_reassemble_within_the_declared_bound() {
    let mut assembler = MessageAssembler::new(MAX_WEBSOCKET_MESSAGE_BYTES).expect("valid");
    let key = [0u8; 4];
    let first =
        websocket::encode_frame(false, Opcode::Text, b"[2,\"id-1\"", Some(key));
    let second = websocket::encode_frame(
        true,
        Opcode::Continuation,
        b",\"Heartbeat\",{}]",
        Some(key),
    );
    let first = websocket::parse_frame_bytes(&first, MAX_WEBSOCKET_MESSAGE_BYTES)
        .expect("decodes");
    let second = websocket::parse_frame_bytes(&second, MAX_WEBSOCKET_MESSAGE_BYTES)
        .expect("decodes");
    assert_eq!(
        assembler.push(first.fin, first.opcode, first.payload),
        Ok(None)
    );
    assert_eq!(
        assembler.push(second.fin, second.opcode, second.payload),
        Ok(Some(AssemblerEvent::Text(
            "[2,\"id-1\",\"Heartbeat\",{}]".to_string()
        )))
    );
    // A continuation with no fragment in progress is a named refusal.
    assert_eq!(
        assembler.push(true, Opcode::Continuation, b"x".to_vec()),
        Err(FrameError::UnexpectedContinuation)
    );
}

// --- Boot / heartbeat / relay through the seam (FIM-F02.03) ----------------

#[test]
fn boot_and_heartbeat_flow_through_the_existing_session_machine() {
    let mut server = server();
    let mut dispatcher = dispatcher_for("depot-cp-001");
    server
        .accept_connection(ChargePointId::new("depot-cp-001".to_string()).expect("valid"), Subprotocol::Ocpp201)
        .expect("accepted");
    // The BootNotification CALL walks Negotiated → Provisioned (the
    // EXISTING transitions) and answers with the recorded response shape.
    let boot = OcppFrame::Call {
        unique_id: UniqueId::new("boot-1".to_string()).expect("valid"),
        action: Action::BootNotification,
        payload: serde_json::json!({
            "vendorName": "vendor",
            "chargePointModel": "model"
        }),
    }
    .encode()
    .expect("encodes");
    let replies = dispatcher
        .handle_inbound_text(&mut server, &ChargePointId::new("depot-cp-001".to_string()).expect("valid"), &boot, 1_000)
        .expect("answered");
    assert_eq!(
        server.connection_snapshot(),
        vec![("depot-cp-001", "PROVISIONED")]
    );
    match &replies[0] {
        OcppFrame::CallResult { payload, .. } => {
            assert_eq!(payload["status"], "Accepted");
            assert_eq!(payload["interval"], 60);
            assert_eq!(payload["currentTime"], 1_000);
        }
        other => panic!("expected a CALLRESULT, got {other:?}"),
    }
    // The Heartbeat answers liveness with the injected clock; no registry
    // call exists for heartbeats (absence is structural).
    let heartbeat = OcppFrame::Call {
        unique_id: UniqueId::new("hb-1".to_string()).expect("valid"),
        action: Action::Heartbeat,
        payload: serde_json::json!({"currentTime": 500}),
    }
    .encode()
    .expect("encodes");
    let replies = dispatcher
        .handle_inbound_text(&mut server, &ChargePointId::new("depot-cp-001".to_string()).expect("valid"), &heartbeat, 1_500)
        .expect("answered");
    match &replies[0] {
        OcppFrame::CallResult { payload, .. } => {
            assert_eq!(payload["currentTime"], 1_500);
        }
        other => panic!("expected a CALLRESULT, got {other:?}"),
    }
}

#[test]
fn the_charging_profile_relay_queues_through_the_existing_bounded_queue() {
    let mut server = server();
    let mut dispatcher = dispatcher_for("depot-cp-001");
    let charge_point = ChargePointId::new("depot-cp-001".to_string()).expect("valid");
    server
        .accept_connection(charge_point.clone(), Subprotocol::Ocpp201)
        .expect("accepted");
    let envelope = DispatchEnvelope {
        start_millis: 1_700_000_000_000,
        periods: vec![DispatchPeriod {
            start_offset_s: 0,
            limit_mw: 0.05,
        }],
    };
    let plan = compose_external_constraints_profile(
        &envelope,
        RateUnit::W,
        &SupplyContext::AcThreePhase {
            nominal_voltage_v: 400.0,
        },
        &DeviceRateConstraints {
            min_charging_rate: None,
        },
        3,
        true,
    )
    .expect("composed");
    // The relay is ISSUED-only (the command-relay-only rule, enforced by
    // the EXISTING typing; a non-ISSUED request is refused by name).
    let error = dispatcher
        .relay_charging_profile(
            &mut server,
            &charge_point,
            &plan,
            CommandState::Approved,
            7,
            UniqueId::new("relay-1".to_string()).expect("valid"),
        )
        .expect_err("refused");
    assert!(error.to_string().contains(CommandState::Approved.name()));
    // The ISSUED relay queues through the EXISTING bounded queue and
    // drains in order.
    dispatcher
        .relay_charging_profile(
            &mut server,
            &charge_point,
            &plan,
            CommandState::Issued,
            7,
            UniqueId::new("relay-1".to_string()).expect("valid"),
        )
        .expect("queued");
    let drained = server.drain_outbound(&charge_point).expect("drained");
    assert_eq!(drained.len(), 1);
    let wire = drained[0].encode().expect("encodes");
    let value: serde_json::Value = serde_json::from_str(&wire).expect("json");
    assert_eq!(value[0], serde_json::json!(2));
    assert_eq!(value[2], serde_json::json!("SetChargingProfile"));
    assert_eq!(value[3]["evseId"], 0);
    assert_eq!(value[3]["chargingSchedule"]["chargingSchedulePeriod"][0]["limit"], 50_000.0);
    // The station's Accepted reply records the stored-not-applied honesty
    // statement (STORAGE evidence, never application evidence).
    let reply = OcppFrame::CallResult {
        unique_id: UniqueId::new("relay-1".to_string()).expect("valid"),
        payload: serde_json::json!({"status": "Accepted"}),
    }
    .encode()
    .expect("encodes");
    dispatcher
        .handle_inbound_text(&mut server, &charge_point, &reply, 2_000)
        .expect("recorded");
    assert!(dispatcher
        .last_accepted_statement()
        .expect("recorded")
        .contains("STORED, not applied"));
}

// --- The live transport construction surface (FIM-F02.03) ------------------

#[test]
fn the_live_transport_constructs_fail_closed_and_reports_honestly() {
    // An unexplained plaintext listener is refused at construction.
    assert_eq!(
        LiveCsmsTransport::new(
            "127.0.0.1:0".to_string(),
            dispatcher_for("depot-cp-001"),
            "  ".to_string(),
            Arc::new(|| 0),
        ),
        Err(flexcore_ocpp_adapter::transport::TransportConstructionError::EmptyLabPostureReason)
    );
    // The honest pre-serve state is Connecting, never a claimed success.
    let transport = LiveCsmsTransport::new(
        "127.0.0.1:0".to_string(),
        dispatcher_for("depot-cp-001"),
        "lab posture: simulation-only deployment; the listener is reachable only on the \
         depot lab network (recorded reason, P17-E-2 DEFAULT edge-agent-hosted shape)"
            .to_string(),
        Arc::new(|| 1_000),
    )
    .expect("valid transport");
    assert_eq!(
        transport.state(),
        flexcore_protocol_ports::connection::ConnectionState::Connecting
    );
    assert_eq!(transport.relay_sender().capacity(), RELAY_QUEUE_CAP);
    assert!(transport.lab_posture_reason().contains("lab posture"));
}

#[test]
fn the_default_seam_refusal_is_unchanged_and_the_relay_request_is_declared_data() {
    // The shipped default transport is UNCHANGED — the live transport is
    // opt-in by construction, never a silent replacement.
    let mut server = server();
    let error = Box::new(UnwiredCsmsTransport)
        .serve(&mut server)
        .expect_err("the seam refusal stays for the default shape");
    assert!(error.to_string().contains(CSMS_TRANSPORT_SEAM_NOT_WIRED));
    // The relay request type is declared data: a plan, the lifecycle state
    // that travels WITH it, the profile id, and the correlation id.
    let request = RelayRequest {
        charge_point: ChargePointId::new("depot-cp-001".to_string()).expect("valid"),
        plan: compose_external_constraints_profile(
            &DispatchEnvelope {
                start_millis: 0,
                periods: vec![DispatchPeriod {
                    start_offset_s: 0,
                    limit_mw: 0.01,
                }],
            },
            RateUnit::W,
            &SupplyContext::AcThreePhase {
                nominal_voltage_v: 400.0,
            },
            &DeviceRateConstraints {
                min_charging_rate: None,
            },
            1,
            true,
        )
        .expect("composed"),
        lifecycle_state: CommandState::Issued,
        charging_profile_id: 1,
        unique_id: UniqueId::new("relay-1".to_string()).expect("valid"),
    };
    assert_eq!(request.lifecycle_state, CommandState::Issued);
    assert_eq!(request.charging_profile_id, 1);
}
