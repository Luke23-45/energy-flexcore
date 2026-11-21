// FIM-F05.06 — the pinned MQTT 5 wire codec suite (ledger
// docs/releases/implementation_ledger/ledger_F05.md).
//
// **WRITTEN-UNRUN.** These tests have never been compiled or executed —
// the no-build/no-test bans (program conventions §1) make execution a
// HUMAN GATE. Nothing here is a conformance result: interop with a real
// MQTT 5 broker is a HUMAN LAB GATE (crate conventions §7.3).
//
// FIXTURE METHOD: every byte vector below was computed OFFLINE with
// `python -I` over a documented MANUAL encoding of the MQTT 5 rules (the
// variable-length integer: 7-bit little-endian groups with the 0x80
// continuation bit; the fixed header: type nibble + flags, then the
// remaining length varint; the CONNECT body: protocol name "MQTT",
// level 5, connect flags, keep alive, properties, payload). No MQTT
// library was used or downloaded. The vectors pin the DECLARED encoder,
// not the specification.

use flexcore_mqtt_adapter::mqtt_wire::{
    decode_connack, decode_disconnect, decode_packet, decode_puback, decode_publish,
    decode_suback, decode_varint, encode_disconnect, encode_pingreq, encode_publish,
    encode_subscribe, frame_len, ConnectOptions, MqttReasonCode, PublishOut, SubscribeOptions,
};
use flexcore_mqtt_adapter::mqtt_wire as wire;
use flexcore_mqtt_adapter::transport::DeclaredQos;

fn unhex(hex: &str) -> Vec<u8> {
    hex.split_whitespace()
        .map(|byte| u8::from_str_radix(byte, 16).expect("valid hex byte"))
        .collect()
}

#[test]
fn varint_boundaries_round_trip() {
    for (value, hex) in [
        (0u32, "00"),
        (127, "7F"),
        (128, "80 01"),
        (321, "C1 02"),
        (16383, "FF 7F"),
        (2097152, "80 80 80 01"),
        (268_435_455, "FF FF FF 7F"),
    ] {
        assert_eq!(decode_varint(&unhex(hex)), Ok((value, unhex(hex).len())));
    }
    // Five continuation bytes exceed the 4-byte rule: a named refusal.
    assert!(decode_varint(&unhex("80 80 80 80 01")).is_err());
}

#[test]
fn connack_decodes_the_pinned_fixture_with_properties() {
    // python -I fixture: session present 0, reason Success, properties =
    // Server Keep Alive (0x13) = 60, Topic Alias Maximum (0x22) = 10.
    let fields =
        decode_connack(&unhex("00 00 06 13 00 3C 22 00 0A")).expect("decodable");
    assert!(!fields.session_present);
    assert_eq!(fields.reason_code, Some(MqttReasonCode::Success));
    assert_eq!(fields.server_keep_alive, Some(60));
    assert_eq!(fields.topic_alias_maximum, Some(10));
    // A session-present CONNACK decodes its flag.
    let fields = decode_connack(&unhex("01 00 00")).expect("decodable");
    assert!(fields.session_present);
}

#[test]
fn connack_skips_declared_properties_and_refuses_unknown_ones() {
    // python -I fixture: a Reason String (0x1F, "busy") and a User
    // Property (0x26, the pair "k"/"v") — both declared, both parsed,
    // both ignored on CONNACK.
    let fields = decode_connack(&unhex(
        "00 00 0E 1F 00 04 62 75 73 79 26 00 01 6B 00 01 76",
    ))
    .expect("declared properties parse");
    assert_eq!(fields.reason_string, Some("busy".to_string()));
    // An identifier OUTSIDE the declared table is a named refusal —
    // never a silent skip.
    let error = decode_connack(&unhex("00 00 02 99 41")).expect_err("unknown property");
    assert!(error.to_string().contains("declared table"));
}

#[test]
fn puback_decodes_full_and_minimal_variants() {
    // python -I fixture: pid 1, reason Success, empty properties.
    let fields = decode_puback(&unhex("00 01 00 00")).expect("decodable");
    assert_eq!(fields.packet_id, 1);
    assert_eq!(fields.reason_code, MqttReasonCode::Success);
    // The reason code and properties are legally omittable (they default
    // to Success and empty — the declared reading).
    let fields = decode_puback(&unhex("00 01")).expect("decodable");
    assert_eq!(fields.reason_code, MqttReasonCode::Success);
}

#[test]
fn subscribe_encodes_the_pinned_byte_fixture() {
    // python -I fixture: pid 1, empty properties, the exact NCMD filter,
    // options 0x05 (QoS 1, No Local set).
    let frame = encode_subscribe(
        1,
        "spBv1.0/site-42/NCMD/edge-07",
        &SubscribeOptions::default(),
    )
    .expect("encodable");
    assert_eq!(
        frame,
        unhex(
            "82 22 00 01 00 00 1C 73 70 42 76 31 2E 30 2F 73 69 74 65 2D 34 32 \
             2F 4E 43 4D 44 2F 65 64 67 65 2D 30 37 05"
        )
    );
}

#[test]
fn suback_decodes_granted_and_refused_codes() {
    // python -I fixture: pid 1, empty properties, granted QoS 1.
    let fields = decode_suback(&unhex("00 01 00 01")).expect("decodable");
    assert_eq!(fields.packet_id, 1);
    assert_eq!(fields.reason_codes, vec![MqttReasonCode::GrantedQos1]);
    // A refusal code (0x87 NotAuthorized) decodes as a named refusal the
    // live transport maps onto SubscribeRefused.
    let fields = decode_suback(&unhex("00 01 00 87")).expect("decodable");
    assert_eq!(fields.reason_codes, vec![MqttReasonCode::NotAuthorized]);
}

#[test]
fn publish_qos1_fixture_round_trips() {
    // python -I fixture: QoS 1, topic spBv1.0/site-42/NDATA/edge-07,
    // pid 1, empty properties, a Sparkplug NDATA payload.
    let frame = unhex(
        "32 56 00 1D 73 70 42 76 31 2E 30 2F 73 69 74 65 2D 34 32 2F 4E 44 41 \
         54 41 2F 65 64 67 65 2D 30 37 00 01 00 08 E8 D7 95 FF BC 31 12 27 0A \
         1A 62 61 74 74 65 72 79 2D 31 2E 73 6F 63 2E 73 6F 63 5F 66 72 61 63 \
         74 69 6F 6E 18 07 41 3D 0A D7 A3 70 3D EA 3F 18 01 28 00",
    );
    let publish = decode_publish(frame[0], &frame[2..]).expect("decodable");
    assert_eq!(publish.topic, "spBv1.0/site-42/NDATA/edge-07");
    assert_eq!(publish.qos, DeclaredQos::AtLeastOnce);
    assert_eq!(publish.packet_id, Some(1));
    assert!(!publish.dup && !publish.retain);
    // Re-encoding the decoded publish reproduces the pinned bytes
    // byte-for-byte (the empty property blob is the declared output).
    let reencoded = encode_publish(&PublishOut {
        topic: publish.topic.clone(),
        payload: publish.payload.clone(),
        qos: publish.qos,
        packet_id: publish.packet_id.expect("qos 1 carries a pid"),
        retain: publish.retain,
        dup: publish.dup,
    })
    .expect("encodable");
    assert_eq!(reencoded, frame);
}

#[test]
fn publish_qos0_carries_no_packet_id() {
    // python -I fixture: QoS 0 (first byte 0x30).
    let frame = unhex(
        "30 54 00 1D 73 70 42 76 31 2E 30 2F 73 69 74 65 2D 34 32 2F 4E 44 41 \
         54 41 2F 65 64 67 65 2D 30 37 00 08 E8 D7 95 FF BC 31 12 27 0A 1A 62 \
         61 74 74 65 72 79 2D 31 2E 73 6F 63 2E 73 6F 63 5F 66 72 61 63 74 69 \
         6F 6E 18 07 41 3D 0A D7 A3 70 3D EA 3F 18 01 28 00",
    );
    let publish = decode_publish(frame[0], &frame[2..]).expect("decodable");
    assert_eq!(publish.qos, DeclaredQos::AtMostOnce);
    assert_eq!(publish.packet_id, None);
}

#[test]
fn incoming_command_publish_decodes_for_the_command_path() {
    // python -I fixture: a broker-side QoS 1 publish on the NCMD topic,
    // pid 9, payload a Sparkplug DBIRTH-shaped metric set.
    let frame = unhex(
        "32 69 00 1C 73 70 42 76 31 2E 30 2F 73 69 74 65 2D 34 32 2F 4E 43 4D \
         44 2F 65 64 67 65 2D 30 37 00 09 00 12 24 0A 1E 69 6E 76 65 72 74 65 \
         72 2D 31 2E 61 76 61 69 6C 61 62 6C 65 2E 61 76 61 69 6C 61 62 6C 65 \
         18 01 30 01 12 1C 0A 0D 6D 65 74 65 72 2D 31 2E 65 2E 6D 77 68 18 04 \
         38 D6 FF FF FF FF FF FF FF FF 01 18 01 28 00",
    );
    let publish = decode_publish(frame[0], &frame[2..]).expect("decodable");
    assert_eq!(publish.topic, "spBv1.0/site-42/NCMD/edge-07");
    assert_eq!(publish.packet_id, Some(9));
    let decoded = flexcore_mqtt_adapter::sparkplug_wire::decode_payload(&publish.payload)
        .expect("the command payload decodes");
    assert_eq!(decoded.metrics.len(), 2);
    assert_eq!(decoded.metrics[0].name, "inverter-1.available.available");
    assert_eq!(
        decoded.metrics[0].value,
        flexcore_mqtt_adapter::model::MetricValue::Boolean(true)
    );
    assert_eq!(
        decoded.metrics[1].value,
        flexcore_mqtt_adapter::model::MetricValue::Int(-42)
    );
}

#[test]
fn pingreq_pingresp_and_disconnect_are_the_pinned_two_byte_shapes() {
    assert_eq!(encode_pingreq(), unhex("C0 00"));
    assert_eq!(
        decode_packet(0xD0, &[]),
        Ok(wire::IncomingPacket::Pingresp)
    );
    // DISCONNECT with reason + empty properties (the encoder's output).
    assert_eq!(
        encode_disconnect(MqttReasonCode::NormalDisconnection),
        unhex("E0 02 00 00")
    );
    let fields = decode_disconnect(&unhex("00 00")).expect("decodable");
    assert_eq!(fields.reason_code, MqttReasonCode::NormalDisconnection);
    // A bare DISCONNECT (reason omitted) defaults to normal.
    let fields = decode_disconnect(&[]).expect("decodable");
    assert_eq!(fields.reason_code, MqttReasonCode::NormalDisconnection);
}

#[test]
fn malformed_packets_are_named_refusals() {
    // QoS 3 on the wire is a protocol violation.
    let error = decode_publish(0x36, &unhex("00 01 00")).expect_err("qos 3 refused");
    assert!(error.to_string().contains("QoS value 3"));
    // A truncated frame prefix is "more bytes needed", never an error.
    assert_eq!(frame_len(&unhex("32 56 00 1D")), Ok(None));
    // A CONNECT on the inbound surface (a broker never sends CONNECT) is
    // outside the declared inbound subset.
    assert!(decode_packet(0x10, &[]).is_err());
    // A PUBACK with an unknown reason byte is refused.
    assert!(decode_puback(&unhex("00 01 77")).is_err());
}

#[test]
fn connect_options_carry_no_password_anywhere() {
    // The structural secret-absence pin (F-05 acceptance criterion 4):
    // ConnectOptions has NO password field, and the encoded flags byte
    // never sets the password bit (0x40) even with a username.
    let options = ConnectOptions {
        client_id: "edge-07-uplink".to_string(),
        keep_alive_secs: 60,
        clean_start: true,
        username: Some("edge-07-uplink".to_string()),
        ..ConnectOptions::default()
    };
    // A serialization walk (the same method credentials.rs pins the
    // binding with): no password-looking key can exist in the type
    // surface.
    let debug = format!("{options:?}").to_lowercase();
    assert!(!debug.contains("password") && !debug.contains("secret"));
    let _ = options; // the encoded-flags pin lives in mqtt_wire.rs tests
}
