// FIM-F05.04 / FIM-F05.05 / FIM-F05.06 — the SOCKET-FREE live-session
// suite: birth/data/death and the command path driven through the live
// transport, the hand-written codecs, the scripted mock broker, and the
// EXISTING BirthDeathTracker (ledger
// docs/releases/implementation_ledger/ledger_F05.md).
//
// **WRITTEN-UNRUN.** These tests have never been compiled or executed —
// the no-build/no-test bans (program conventions §1) make execution a
// HUMAN GATE. Nothing here is a conformance result: interop with a real
// MQTT 5 broker is a HUMAN LAB GATE (crate conventions §7.3). The gated
// real-broker suite is `tests/live_broker_gated.rs` (skips cleanly
// without its gate variable).
//
// SOCKET-FREE DISCIPLINE: no TCP socket is opened anywhere. The engine's
// `BrokerConnector` seam is injected with an in-memory
// `tokio::io::duplex` pair; the "broker" is a SCRIPTED task on the other
// half that responds with the PINNED broker frames (CONNACK, SUBACK,
// PUBACK — the python -I fixtures) and can push a command PUBLISH. The
// scripted frames pin the byte-level contract; the mock connector pins
// the seam (the production connector dials TLS out, and is never
// constructed here).

use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use flexcore_mqtt_adapter::birth_death::{BirthDeathTracker, Observation};
use flexcore_mqtt_adapter::live::{
    node_command_topic_filter, BrokerConnector, BrokerStream, DeviceBirthInventory,
    LiveMqttConfig, LiveMqttUplinkTransport, MqttCommandSource, SparkplugCommandDelivery,
};
use flexcore_mqtt_adapter::model::{
    BdSeq, DeviceId, EdgeNodeId, GroupId, MetricType, MetricValue, SparkplugMessageKind,
    SparkplugPayload, SparkplugSeq,
};
use flexcore_mqtt_adapter::mqtt_wire::{
    decode_packet, decode_varint, encode_puback, frame_len, IncomingPacket,
};
use flexcore_mqtt_adapter::sparkplug_wire::decode_payload;
use flexcore_mqtt_adapter::topic::ValidatedTopic;
use flexcore_mqtt_adapter::transport::{
    DeclaredQos, MqttConnectionConfig, MqttTransportError, MqttUplinkTransport,
};
use flexcore_mqtt_adapter::uplink::SparkplugUplinkPlan;
use futures::future::BoxFuture;
use tokio::io::{AsyncReadExt, AsyncWriteExt, DuplexStream};

fn unhex(hex: &str) -> Vec<u8> {
    hex.split_whitespace()
        .map(|byte| u8::from_str_radix(byte, 16).expect("valid hex byte"))
        .collect()
}

fn metric(
    name: &str,
    metric_type: MetricType,
    value: MetricValue,
) -> flexcore_mqtt_adapter::model::SparkplugMetric {
    flexcore_mqtt_adapter::model::SparkplugMetric::new(
        name.to_string(),
        None,
        metric_type,
        value,
    )
    .expect("valid metric")
}

/// The pinned broker frames (python -I fixtures; see
/// `tests/mqtt_wire_codec.rs` for the decoding counterparts).
const CONNACK_SUCCESS: &[u8] =
    &[0x20, 0x09, 0x00, 0x00, 0x06, 0x13, 0x00, 0x3C, 0x22, 0x00, 0x0A];
const SUBACK_GRANTED_QOS1: &[u8] = &[0x90, 0x04, 0x00, 0x01, 0x00, 0x01];
const BROKER_DISCONNECT: &[u8] = &[0xE0, 0x02, 0x00, 0x00];
/// The broker's command publish (python -I fixture): QoS 1, pid 9, the
/// NCMD topic, a two-metric Sparkplug payload.
const PUBLISH_CMD_BYTES: &[u8] = &[
    0x32, 0x69, 0x00, 0x1C, 0x73, 0x70, 0x42, 0x76, 0x31, 0x2E, 0x30, 0x2F, 0x73, 0x69,
    0x74, 0x65, 0x2D, 0x34, 0x32, 0x2F, 0x4E, 0x43, 0x4D, 0x44, 0x2F, 0x65, 0x64, 0x67,
    0x65, 0x2D, 0x30, 0x37, 0x00, 0x09, 0x00, 0x12, 0x24, 0x0A, 0x1E, 0x69, 0x6E, 0x76,
    0x65, 0x72, 0x74, 0x65, 0x72, 0x2D, 0x31, 0x2E, 0x61, 0x76, 0x61, 0x69, 0x6C, 0x61,
    0x62, 0x6C, 0x65, 0x2E, 0x61, 0x76, 0x61, 0x69, 0x6C, 0x61, 0x62, 0x6C, 0x65, 0x18,
    0x01, 0x30, 0x01, 0x12, 0x1C, 0x0A, 0x0D, 0x6D, 0x65, 0x74, 0x65, 0x72, 0x2D, 0x31,
    0x2E, 0x65, 0x2E, 0x6D, 0x77, 0x68, 0x18, 0x04, 0x38, 0xD6, 0xFF, 0xFF, 0xFF, 0xFF,
    0xFF, 0xFF, 0xFF, 0xFF, 0x01, 0x18, 0x01, 0x28, 0x00,
];
/// The pinned CONNECT the engine must send (python -I): flags 0x8E
/// (username | will QoS 1 | will flag | clean start — and NEVER the
/// password bit), session expiry 3600, receive maximum 65535, the NDEATH
/// will (topic spBv1.0/site-42/NDEATH/edge-07, payload 28 00 = bdSeq 0).
const EXPECTED_CONNECT_BYTES: &[u8] = &[
    0x10, 0x58, 0x00, 0x04, 0x4D, 0x51, 0x54, 0x54, 0x05, 0x8E, 0x00, 0x3C, 0x08, 0x11,
    0x00, 0x00, 0x0E, 0x10, 0x21, 0xFF, 0xFF, 0x00, 0x0E, 0x65, 0x64, 0x67, 0x65, 0x2D,
    0x30, 0x37, 0x2D, 0x75, 0x70, 0x6C, 0x69, 0x6E, 0x6B, 0x00, 0x00, 0x1E, 0x73, 0x70,
    0x42, 0x76, 0x31, 0x2E, 0x30, 0x2F, 0x73, 0x69, 0x74, 0x65, 0x2D, 0x34, 0x32, 0x2F,
    0x4E, 0x44, 0x45, 0x41, 0x54, 0x48, 0x2F, 0x65, 0x64, 0x67, 0x65, 0x2D, 0x30, 0x37,
    0x00, 0x02, 0x28, 0x00, 0x00, 0x0E, 0x65, 0x64, 0x67, 0x65, 0x2D, 0x30, 0x37, 0x2D,
    0x75, 0x70, 0x6C, 0x69, 0x6E, 0x6B,
];
/// The pinned SUBSCRIBE the engine must send (python -I): pid 1, the
/// exact NCMD filter, options 0x05 (QoS 1, No Local).
const EXPECTED_SUBSCRIBE_BYTES: &[u8] = &[
    0x82, 0x22, 0x00, 0x01, 0x00, 0x00, 0x1C, 0x73, 0x70, 0x42, 0x76, 0x31, 0x2E, 0x30,
    0x2F, 0x73, 0x69, 0x74, 0x65, 0x2D, 0x34, 0x32, 0x2F, 0x4E, 0x43, 0x4D, 0x44, 0x2F,
    0x65, 0x64, 0x67, 0x65, 0x2D, 0x30, 0x37, 0x05,
];

// --- The mock connector and scripted broker ---------------------------------------

/// The socket-free connector: hands the engine one end of an in-memory
/// duplex pair (never a socket).
#[derive(Clone)]
struct MockConnector {
    half: Arc<Mutex<Option<DuplexStream>>>,
}

impl MockConnector {
    fn new(half: DuplexStream) -> Self {
        Self {
            half: Arc::new(Mutex::new(Some(half))),
        }
    }
}

impl BrokerConnector for MockConnector {
    fn connect(
        &self,
    ) -> BoxFuture<
        'static,
        Result<Box<dyn BrokerStream>, flexcore_mqtt_adapter::live::LiveUplinkError>,
    > {
        let slot = Arc::clone(&self.half);
        Box::pin(async move {
            slot.lock()
                .expect("mock connector slot")
                .take()
                .map(|stream| Box::new(stream) as Box<dyn BrokerStream>)
                .ok_or(flexcore_mqtt_adapter::live::LiveUplinkError::Io {
                    reason: "the mock broker duplex was already consumed".to_string(),
                })
        })
    }
}

/// What the scripted broker observed (ordered; the test consumes the
/// channel in script order).
enum BrokerEvent {
    /// The raw CONNECT frame bytes.
    Connect(Vec<u8>),
    /// The raw SUBSCRIBE frame bytes.
    Subscribe(Vec<u8>),
    /// An inbound publish: (topic, payload, packet id).
    Publish(String, Vec<u8>, u16),
    /// The engine's PUBACK for the command publish (raw bytes).
    CommandPuback(Vec<u8>),
    /// The graceful DISCONNECT the engine wrote (raw bytes).
    Disconnect(Vec<u8>),
    /// EOF: the engine closed the stream WITHOUT a DISCONNECT (the
    /// unclean-disconnect path — the broker-side will would fire).
    StreamClosedWithoutDisconnect,
}

/// Reads one complete frame from the mock broker half (`None` on EOF).
async fn read_frame(stream: &mut DuplexStream) -> Option<Vec<u8>> {
    tokio::time::timeout(Duration::from_secs(15), async {
        let mut buf: Vec<u8> = Vec::new();
        loop {
            if let Some(total) = frame_len(&buf).ok()? {
                return Some(buf.drain(..total).collect::<Vec<u8>>());
            }
            let mut chunk = [0u8; 4096];
            let n = stream.read(&mut chunk).await.ok()?;
            if n == 0 {
                return None; // EOF: the peer closed without a full frame
            }
            buf.extend_from_slice(&chunk[..n]);
        }
    })
    .await
    .ok()?
}

/// Splits a publish frame into (topic, payload, packet id) via the crate
/// decoder (the broker script reuses the SAME codec the engine uses).
fn parse_publish(frame: &[u8]) -> (String, Vec<u8>, u16) {
    let (_, varint_len) = decode_varint(&frame[1..]).expect("frame varint");
    match decode_packet(frame[0], &frame[1 + varint_len..]).expect("frame decodes") {
        IncomingPacket::Publish(publish) => (
            publish.topic,
            publish.payload,
            publish.packet_id.expect("qos 1 publish carries a pid"),
        ),
        other => panic!("expected a PUBLISH, got {other:?}"),
    }
}

fn config(command_filter: Option<String>) -> LiveMqttConfig {
    LiveMqttConfig {
        connection: MqttConnectionConfig {
            endpoint: "mqtts://mock-broker.lab:8883".to_string(),
            require_tls: true,
            client_id: "edge-07-uplink".to_string(),
            qos: DeclaredQos::AtLeastOnce,
            trust_anchor_reference: "phase13-secrets/uplink-trust-anchor".to_string(),
        },
        plan: SparkplugUplinkPlan::new(
            GroupId::new("site-42").expect("valid"),
            EdgeNodeId::new("edge-07").expect("valid"),
        ),
        username: "edge-07-uplink".to_string(),
        bd_seq: BdSeq::initial(),
        node_birth_metrics: Vec::new(),
        device_births: vec![DeviceBirthInventory {
            device_id: DeviceId::new("inverter-1").expect("valid"),
            metrics: vec![metric(
                "inverter-1.p.mw",
                MetricType::Double,
                MetricValue::Double(3.5),
            )],
        }],
        trust_anchor_pem_path: "/etc/flexcore/mqtt/ca.pem".to_string(),
        client_certificate_pem_path: None,
        client_private_key_pem_path: None,
        plaintext_lab_posture_reason: None,
        keep_alive_secs: 60,
        session_expiry_secs: Some(3600),
        receive_maximum: Some(65535),
        topic_alias_maximum: None,
        connect_timeout: Duration::from_secs(10),
        ack_timeout: Duration::from_secs(10),
        command_topic_filter: command_filter,
    }
}

fn recv_event(receiver: &mpsc::Receiver<BrokerEvent>) -> BrokerEvent {
    receiver
        .recv_timeout(Duration::from_secs(15))
        .expect("the scripted broker produced the expected event")
}

/// Waits (bounded) until the transport reports the session lost.
fn wait_until_lost(transport: &LiveMqttUplinkTransport) -> bool {
    for _ in 0..100 {
        if !transport.is_connected_now() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}

/// Rebuilds a typed payload from a decoded wire payload (the tracker
/// consumes the model type; the decoded kind is a topic property, so the
/// test re-declares it from the topic segment it asserted).
fn to_payload(
    topic: &str,
    decoded: &flexcore_mqtt_adapter::sparkplug_wire::DecodedPayload,
) -> SparkplugPayload {
    let kind = if topic.contains("/NBIRTH/") {
        SparkplugMessageKind::NodeBirth
    } else if topic.contains("/DBIRTH/") {
        SparkplugMessageKind::DeviceBirth
    } else if topic.contains("/NDATA/") {
        SparkplugMessageKind::NodeData
    } else if topic.contains("/DDATA/") {
        SparkplugMessageKind::DeviceData
    } else {
        SparkplugMessageKind::NodeDeath
    };
    SparkplugPayload {
        kind,
        seq: decoded.seq,
        bd_seq: decoded.bd_seq.unwrap_or_else(BdSeq::initial),
        metrics: decoded.metrics.clone(),
    }
}

/// Drains the command inbox with a bounded retry (the delivery is pushed
/// asynchronously by the engine thread).
fn poll_command(transport: &mut LiveMqttUplinkTransport) -> Option<SparkplugCommandDelivery> {
    for _ in 0..200 {
        if let Some(delivery) = transport.poll_command() {
            return Some(delivery);
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    None
}

// --- The tests ---------------------------------------------------------------------

/// FIM-F05.04 + FIM-F05.05: connect (CONNECT with the pinned LWT) →
/// SUBSCRIBE (pinned) → NBIRTH → DBIRTH → NDATA publish with a PUBACK
/// receipt → command delivery through the inbox → graceful DISCONNECT.
/// The EXISTING BirthDeathTracker judges the whole published sequence.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn birth_data_death_and_command_flow_through_the_scripted_broker() {
    let (mut broker_half, engine_half) = tokio::io::duplex(65_536);
    let (events_tx, events_rx) = mpsc::channel();
    let filter = node_command_topic_filter(&config(None).plan);
    let engine_config = config(Some(filter.clone()));
    tokio::spawn(async move {
        // 1. The engine's CONNECT — pinned byte-for-byte (LWT config).
        let connect = read_frame(&mut broker_half).await;
        events_tx
            .send(BrokerEvent::Connect(connect.expect("CONNECT arrives")))
            .ok();
        // 2. CONNACK success.
        broker_half
            .write_all(CONNACK_SUCCESS)
            .await
            .expect("connack write");
        // 3. The command SUBSCRIBE — pinned byte-for-byte.
        let subscribe = read_frame(&mut broker_half).await;
        events_tx
            .send(BrokerEvent::Subscribe(subscribe.expect("SUBSCRIBE arrives")))
            .ok();
        // 4. SUBACK granted QoS 1.
        broker_half
            .write_all(SUBACK_GRANTED_QOS1)
            .await
            .expect("suback write");
        // 5. NBIRTH then DBIRTH; each PUBACKed.
        for _ in 0..2 {
            let frame = read_frame(&mut broker_half).await.expect("birth arrives");
            let (topic, payload, pid) = parse_publish(&frame);
            events_tx.send(BrokerEvent::Publish(topic, payload, pid)).ok();
            broker_half
                .write_all(&encode_puback(pid))
                .await
                .expect("puback write");
        }
        // 6. The NDATA publish the test issues after connect.
        let frame = read_frame(&mut broker_half).await.expect("data arrives");
        let (topic, payload, pid) = parse_publish(&frame);
        events_tx.send(BrokerEvent::Publish(topic, payload, pid)).ok();
        broker_half
            .write_all(&encode_puback(pid))
            .await
            .expect("puback write");
        // 7. The broker pushes a command publish (the pinned fixture).
        broker_half
            .write_all(PUBLISH_CMD_BYTES)
            .await
            .expect("command write");
        // 8. The engine PUBACKs the incoming command publish (QoS 1).
        let ack = read_frame(&mut broker_half)
            .await
            .expect("command puback arrives");
        events_tx.send(BrokerEvent::CommandPuback(ack)).ok();
        // 9. The graceful DISCONNECT — written when the test calls
        //    transport.disconnect(); pinned byte-for-byte.
        let disconnect = read_frame(&mut broker_half).await.expect("disconnect arrives");
        events_tx.send(BrokerEvent::Disconnect(disconnect)).ok();
    });
    let mut transport = LiveMqttUplinkTransport::connect_with_connector(
        engine_config,
        Arc::new(MockConnector::new(engine_half)),
    )
    .expect("the engine connects through the mock broker");
    assert!(transport.is_connected_now());

    // The CONNECT the broker saw is the PINNED LWT-carrying frame.
    match recv_event(&events_rx) {
        BrokerEvent::Connect(bytes) => assert_eq!(bytes, EXPECTED_CONNECT_BYTES),
        other => panic!("expected the CONNECT event, got {other:?}"),
    }
    // The SUBSCRIBE the broker saw is the PINNED exact-filter frame.
    match recv_event(&events_rx) {
        BrokerEvent::Subscribe(bytes) => assert_eq!(bytes, EXPECTED_SUBSCRIBE_BYTES),
        other => panic!("expected the SUBSCRIBE event, got {other:?}"),
    }
    // NBIRTH then DBIRTH — fed through the EXISTING trackers (node and
    // device scopes are separate tracker instances, the declared machine
    // shape at both scopes).
    let mut node_tracker = BirthDeathTracker::new();
    let mut device_tracker = BirthDeathTracker::new();
    for (expected_topic, tracker) in [
        ("spBv1.0/site-42/NBIRTH/edge-07", &mut node_tracker),
        ("spBv1.0/site-42/DBIRTH/edge-07/inverter-1", &mut device_tracker),
    ] {
        match recv_event(&events_rx) {
            BrokerEvent::Publish(topic, payload, pid) => {
                assert_eq!(topic, expected_topic);
                assert!(pid >= 1);
                let decoded = decode_payload(&payload).expect("the birth payload decodes");
                assert_eq!(decoded.bd_seq, Some(BdSeq::initial()));
                // The self-check rule runs BOTH directions: the tracker
                // accepts the engine's own BIRTH.
                let observation = tracker.observe(&to_payload(&topic, &decoded));
                assert!(
                    matches!(observation, Observation::Accepted { .. }),
                    "the published BIRTH must satisfy the existing machine: {observation:?}"
                );
            }
            other => panic!("expected a birth event, got {other:?}"),
        }
    }

    // NDATA: the canonical SOC fraction, continuing the node seq chain
    // (NBIRTH carried seq 0 → the node tracker expects seq 1).
    let ndata_topic = ValidatedTopic::new(
        GroupId::new("site-42").expect("valid"),
        SparkplugMessageKind::NodeData,
        EdgeNodeId::new("edge-07").expect("valid"),
        None,
    )
    .expect("valid topic");
    let ndata_payload = SparkplugPayload {
        kind: SparkplugMessageKind::NodeData,
        seq: Some(SparkplugSeq::new(1).expect("valid")),
        bd_seq: BdSeq::initial(),
        metrics: vec![metric(
            "battery-1.soc.soc_fraction",
            MetricType::Double,
            MetricValue::Double(0.82),
        )],
    };
    let receipt = transport.publish(&ndata_topic, &ndata_payload).expect("published");
    assert_eq!(receipt.topic, "spBv1.0/site-42/NDATA/edge-07");
    assert_eq!(receipt.qos, DeclaredQos::AtLeastOnce);
    match recv_event(&events_rx) {
        BrokerEvent::Publish(topic, payload, _pid) => {
            assert_eq!(topic, "spBv1.0/site-42/NDATA/edge-07");
            let decoded = decode_payload(&payload).expect("the data payload decodes");
            assert_eq!(
                decoded.metrics[0].value,
                MetricValue::Double(0.82),
                "the canonical SOC fraction travels the wire unchanged"
            );
            let observation = node_tracker.observe(&to_payload(&topic, &decoded));
            assert!(matches!(observation, Observation::Accepted { .. }));
        }
        other => panic!("expected the NDATA event, got {other:?}"),
    }

    // The command path: the broker's pinned command publish is acked and
    // delivered into the inbox.
    match recv_event(&events_rx) {
        BrokerEvent::CommandPuback(bytes) => assert_eq!(bytes, encode_puback(9)),
        other => panic!("expected the command PUBACK event, got {other:?}"),
    }
    let delivery = poll_command(&mut transport).expect("the command is delivered");
    assert_eq!(delivery.topic, filter);
    assert_eq!(delivery.metrics.len(), 2);
    assert_eq!(
        delivery.metrics[0].value,
        MetricValue::Boolean(true),
        "a delivered command carries decoded metrics, NO authority"
    );

    // Graceful disconnect: the engine writes the pinned DISCONNECT bytes
    // and the session is over.
    transport.disconnect().expect("graceful disconnect");
    match recv_event(&events_rx) {
        BrokerEvent::Disconnect(bytes) => assert_eq!(bytes, unhex("E0 02 00 00")),
        other => panic!("expected the DISCONNECT event, got {other:?}"),
    }
}

/// FIM-F05.04 (unclean disconnect): the mock broker vanishes mid-session
/// (EOF) — the engine declares the connection LOST WITHOUT writing a
/// DISCONNECT (the broker-side will fires, which is the NDEATH
/// semantics), and every later publish refuses named. The LWT was
/// preconfigured in the CONNECT itself (pinned bytes).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_unclean_disconnect_loses_the_session_named_and_the_lwt_is_preconfigured() {
    let (mut broker_half, engine_half) = tokio::io::duplex(65_536);
    let (events_tx, events_rx) = mpsc::channel();
    // No command subscription and no devices: CONNECT → CONNACK → NBIRTH.
    let engine_config = config(None);
    tokio::spawn(async move {
        let connect = read_frame(&mut broker_half).await.expect("CONNECT arrives");
        events_tx.send(BrokerEvent::Connect(connect)).ok();
        broker_half
            .write_all(CONNACK_SUCCESS)
            .await
            .expect("connack write");
        let frame = read_frame(&mut broker_half).await.expect("NBIRTH arrives");
        let (topic, _payload, pid) = parse_publish(&frame);
        assert_eq!(topic, "spBv1.0/site-42/NBIRTH/edge-07");
        events_tx.send(BrokerEvent::Publish(topic, Vec::new(), pid)).ok();
        broker_half
            .write_all(&encode_puback(pid))
            .await
            .expect("puback write");
        // The broker VANISHES: drop the stream half — EOF, no DISCONNECT.
        drop(broker_half);
        events_tx.send(BrokerEvent::StreamClosedWithoutDisconnect).ok();
    });
    let mut transport = LiveMqttUplinkTransport::connect_with_connector(
        engine_config,
        Arc::new(MockConnector::new(engine_half)),
    )
    .expect("connected");
    match recv_event(&events_rx) {
        BrokerEvent::Connect(bytes) => {
            // Byte-level LWT pin: the will flag is set, the will QoS is 1,
            // the password flag is NEVER set, and the will payload is the
            // pinned NDEATH bytes (28 00).
            assert_eq!(bytes[0], 0x10);
            assert_eq!(bytes[9] & 0x04, 0x04, "the will flag is set");
            assert_eq!(bytes[9] & 0x08, 0x08, "the will QoS is 1");
            assert_eq!(bytes[9] & 0x40, 0, "no password flag — the never-granted rule");
            assert!(
                bytes.windows(2).any(|w| w[0] == 0x28 && w[1] == 0x00),
                "the will payload is the pinned NDEATH bytes"
            );
        }
        other => panic!("expected the CONNECT event, got {other:?}"),
    }
    recv_event(&events_rx); // the NBIRTH observation
    recv_event(&events_rx); // the broker closed
    // The engine declares the session lost (bounded poll).
    assert!(wait_until_lost(&transport), "the EOF must be declared a connection loss");
    // A publish after the loss is a NAMED refusal, never a fabricated
    // receipt (the composition owns redelivery in order).
    let topic = ValidatedTopic::new(
        GroupId::new("site-42").expect("valid"),
        SparkplugMessageKind::NodeData,
        EdgeNodeId::new("edge-07").expect("valid"),
        None,
    )
    .expect("valid topic");
    let payload = SparkplugPayload {
        kind: SparkplugMessageKind::NodeData,
        seq: Some(SparkplugSeq::new(1).expect("valid")),
        bd_seq: BdSeq::initial(),
        metrics: Vec::new(),
    };
    let error = transport.publish(&topic, &payload).expect_err("refused after loss");
    assert!(
        matches!(error, MqttTransportError::NotConnected { .. }),
        "{error}"
    );
    // Dropping the transport joins the engine (no leaked threads).
    drop(transport);
}

/// FIM-F05.04: the BROKER disconnecting the session is also a loss — and
/// the engine exits WITHOUT writing a DISCONNECT (the will fires).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_broker_disconnect_ends_the_session_without_a_client_disconnect() {
    let (mut broker_half, engine_half) = tokio::io::duplex(65_536);
    let (events_tx, events_rx) = mpsc::channel();
    let engine_config = config(None);
    tokio::spawn(async move {
        let connect = read_frame(&mut broker_half).await.expect("CONNECT arrives");
        events_tx.send(BrokerEvent::Connect(connect)).ok();
        broker_half
            .write_all(CONNACK_SUCCESS)
            .await
            .expect("connack write");
        let frame = read_frame(&mut broker_half).await.expect("NBIRTH arrives");
        let (_topic, _payload, pid) = parse_publish(&frame);
        broker_half
            .write_all(&encode_puback(pid))
            .await
            .expect("puback write");
        // The broker kicks the session.
        broker_half
            .write_all(BROKER_DISCONNECT)
            .await
            .expect("disconnect write");
        // The engine must NOT answer with its own DISCONNECT: the next
        // thing the broker observes is EOF (the will fires broker-side).
        match read_frame(&mut broker_half).await {
            Some(frame) => panic!("the engine wrote {frame:?} after the broker kicked it"),
            None => events_tx.send(BrokerEvent::StreamClosedWithoutDisconnect).ok(),
        }
    });
    let mut transport = LiveMqttUplinkTransport::connect_with_connector(
        engine_config,
        Arc::new(MockConnector::new(engine_half)),
    )
    .expect("connected");
    recv_event(&events_rx);
    assert!(
        wait_until_lost(&transport),
        "a broker DISCONNECT ends the session"
    );
    match recv_event(&events_rx) {
        BrokerEvent::StreamClosedWithoutDisconnect => {}
        other => panic!("expected a will-firing close, got {other:?}"),
    }
    drop(transport);
}

/// FIM-F05.05: the delivery and config type surfaces carry NO secret
/// material (the structural-absence pin — the F-05 acceptance criterion
/// 4 method, applied to the live surface).
#[test]
fn the_command_delivery_and_config_surfaces_are_secret_free() {
    let delivery = SparkplugCommandDelivery {
        topic: "spBv1.0/site-42/NCMD/edge-07".to_string(),
        timestamp_millis: None,
        uuid: None,
        seq: Some(SparkplugSeq::new(1).expect("valid")),
        bd_seq: Some(BdSeq::initial()),
        metrics: Vec::new(),
    };
    let debug = format!("{delivery:?}").to_lowercase();
    assert!(
        !debug.contains("password") && !debug.contains("secret"),
        "{debug}"
    );
    let serialized = serde_json::to_string(&config(None)).expect("serializable");
    let lowered = serialized.to_lowercase();
    assert!(
        !lowered.contains("password") && !lowered.contains("secret_value"),
        "the live config must not carry secret-looking fields: {serialized}"
    );
}
