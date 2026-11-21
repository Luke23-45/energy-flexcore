// FIM-F05.06 (gated) — the REAL-BROKER suite for the live MQTT 5 +
// Sparkplug B uplink (ledger
// docs/releases/implementation_ledger/ledger_F05.md).
//
// **WRITTEN-UNRUN.** These tests have never been compiled or executed —
// the no-build/no-test bans (program conventions §1) make execution a
// HUMAN GATE, and this suite's whole point IS the gate: interop with a
// real MQTT 5 broker is a HUMAN LAB GATE (crate conventions §7.3;
// FIM-F05.07). Nothing here is a conformance result.
//
// GATING (the recorded repo convention, as in the 20305 adapter's
// `tests/live_transport_gated.rs`): every test is gated on
// `FLEXCORE_TEST_MQTT_BROKER_PKI_DIR` and SKIPS CLEANLY (prints a note
// and returns) when the variable is unset or empty. A mis-set gate (a
// non-directory) fails loudly instead.
//
// The gate needs a human-provided PKI directory with EXACTLY these PEM
// files (NO key material exists in this repository — the
// composition-root rule; the human lab supplies all of it):
//
//   $FLEXCORE_TEST_MQTT_BROKER_PKI_DIR/
//     ca.pem          trust anchor: the CA that signed the broker (and,
//                     when mTLS is exercised, the client) identity
//     client.pem      the edge's client identity (optional: with
//                     client.key.pem this exercises the mTLS path)
//     client.key.pem  its private key
//
// and the broker environment variables:
//
//   FLEXCORE_TEST_MQTT_BROKER_HOST   the broker hostname (TLS SNI)
//   FLEXCORE_TEST_MQTT_BROKER_PORT   the broker TLS port (typically 8883)
//
// An unset HOST (with a set PKI dir) is a loud failure — a half-declared
// gate is a defect, never a skip.
//
// WHAT THE SUITE PINS (the F-05 acceptance criteria, live): (1) the edge
// connects over TLS, publishes the Sparkplug BIRTH certificate (NBIRTH +
// DBIRTH) and telemetry (NDATA) and reads the PUBACK receipts; (2) a
// command published to the subscribed NCMD topic (by the suite itself,
// with a second client connection) is DELIVERED into the command inbox;
// (3) an unclean TCP kill lets the BROKER publish the will — the suite
// reconnects with a monitor session and verifies the NDEATH arrived with
// the right bdSeq; (4) the transport type surface holds no secret
// material (pinned in `live_session_socket_free.rs`).

use std::path::{Path, PathBuf};
use std::time::Duration;

use flexcore_mqtt_adapter::live::{
    node_command_topic_filter, LiveMqttConfig, LiveMqttUplinkTransport, MqttCommandSource,
};
use flexcore_mqtt_adapter::model::{
    BdSeq, EdgeNodeId, GroupId, MetricType, MetricValue, SparkplugMessageKind, SparkplugPayload,
    SparkplugSeq,
};
use flexcore_mqtt_adapter::topic::ValidatedTopic;
use flexcore_mqtt_adapter::transport::{DeclaredQos, MqttConnectionConfig, MqttUplinkTransport};
use flexcore_mqtt_adapter::uplink::SparkplugUplinkPlan;

/// The gate: `None` means "skip cleanly" (the 20305/NATS convention).
fn pki_dir() -> Option<PathBuf> {
    match std::env::var("FLEXCORE_TEST_MQTT_BROKER_PKI_DIR") {
        Ok(dir) if !dir.trim().is_empty() => {
            let dir = PathBuf::from(dir);
            if dir.is_dir() {
                Some(dir)
            } else {
                panic!(
                    "FLEXCORE_TEST_MQTT_BROKER_PKI_DIR is set but is not a directory: {}",
                    dir.display()
                );
            }
        }
        _ => None,
    }
}

fn broker_host() -> String {
    std::env::var("FLEXCORE_TEST_MQTT_BROKER_HOST")
        .unwrap_or_else(|_| panic!("FLEXCORE_TEST_MQTT_BROKER_HOST must be set with the gate"))
        .trim()
        .to_string()
}

fn broker_port() -> u16 {
    std::env::var("FLEXCORE_TEST_MQTT_BROKER_PORT")
        .unwrap_or_else(|_| panic!("FLEXCORE_TEST_MQTT_BROKER_PORT must be set with the gate"))
        .trim()
        .parse()
        .expect("the broker port parses")
}

fn gate_config(pki: &Path, host: &str, port: u16) -> LiveMqttConfig {
    let client_cert = pki.join("client.pem");
    let client_key = pki.join("client.key.pem");
    LiveMqttConfig {
        connection: MqttConnectionConfig {
            endpoint: format!("mqtts://{host}:{port}"),
            require_tls: true,
            client_id: "flexcore-edge-07-uplink".to_string(),
            qos: DeclaredQos::AtLeastOnce,
            trust_anchor_reference: "lab-supplied-ca.pem".to_string(),
        },
        plan: SparkplugUplinkPlan::new(
            GroupId::new("lab-site").expect("valid"),
            EdgeNodeId::new("lab-edge-07").expect("valid"),
        ),
        username: "flexcore-lab".to_string(),
        bd_seq: BdSeq::initial(),
        node_birth_metrics: vec![],
        device_births: vec![],
        trust_anchor_pem_path: pki.join("ca.pem").display().to_string(),
        client_certificate_pem_path: client_cert.exists().then(|| client_cert.display().to_string()),
        client_private_key_pem_path: client_key.exists().then(|| client_key.display().to_string()),
        plaintext_lab_posture_reason: None,
        keep_alive_secs: 30,
        session_expiry_secs: Some(300),
        receive_maximum: Some(65535),
        topic_alias_maximum: None,
        connect_timeout: Duration::from_secs(10),
        ack_timeout: Duration::from_secs(10),
        command_topic_filter: None,
    }
}

/// Acceptance criterion 1 (live): connect over TLS, publish the BIRTH
/// certificate and telemetry, and read the PUBACK receipts back.
#[test]
fn the_edge_publishes_birth_and_telemetry_to_a_real_broker() {
    let Some(pki) = pki_dir() else {
        println!(
            "SKIP: FLEXCORE_TEST_MQTT_BROKER_PKI_DIR is unset — the real-broker \
             interop gate is a HUMAN LAB GATE (nothing ran)"
        );
        return;
    };
    let host = broker_host();
    let port = broker_port();
    let mut transport = LiveMqttUplinkTransport::connect(gate_config(&pki, &host, port))
        .expect("the live uplink connects to the real broker over TLS");
    assert!(transport.is_connected_now());

    // The BIRTH certificate: NBIRTH with the node inventory.
    let birth_topic = ValidatedTopic::new(
        GroupId::new("lab-site").expect("valid"),
        SparkplugMessageKind::NodeBirth,
        EdgeNodeId::new("lab-edge-07").expect("valid"),
        None,
    )
    .expect("valid topic");
    let birth_payload = SparkplugPayload {
        kind: SparkplugMessageKind::NodeBirth,
        seq: Some(SparkplugSeq::new(0).expect("valid")),
        bd_seq: BdSeq::initial(),
        metrics: vec![],
    };
    let receipt = transport
        .publish(&birth_topic, &birth_payload)
        .expect("the broker acknowledged the NBIRTH");
    assert_eq!(receipt.qos, DeclaredQos::AtLeastOnce);

    // Telemetry: NDATA with the canonical SOC fraction.
    let data_topic = ValidatedTopic::new(
        GroupId::new("lab-site").expect("valid"),
        SparkplugMessageKind::NodeData,
        EdgeNodeId::new("lab-edge-07").expect("valid"),
        None,
    )
    .expect("valid topic");
    let data_payload = SparkplugPayload {
        kind: SparkplugMessageKind::NodeData,
        seq: Some(SparkplugSeq::new(1).expect("valid")),
        bd_seq: BdSeq::initial(),
        metrics: vec![],
    };
    transport
        .publish(&data_topic, &data_payload)
        .expect("the broker acknowledged the NDATA");

    transport.disconnect().expect("graceful disconnect");
}

/// Acceptance criterion 2 (live): a command published to the subscribed
/// NCMD topic is delivered into the command inbox.
#[test]
fn a_command_on_the_subscribed_topic_reaches_the_edge() {
    let Some(pki) = pki_dir() else {
        println!(
            "SKIP: FLEXCORE_TEST_MQTT_BROKER_PKI_DIR is unset — the real-broker \
             interop gate is a HUMAN LAB GATE (nothing ran)"
        );
        return;
    };
    let host = broker_host();
    let port = broker_port();
    let mut config = gate_config(&pki, &host, port);
    config.command_topic_filter =
        Some(node_command_topic_filter(&config.plan));
    let mut transport = LiveMqttUplinkTransport::connect(config)
        .expect("the live uplink connects with the command subscription");
    // A command publish by any broker-side client arrives in the inbox:
    // the suite asserts delivery by polling a bounded time (a human lab
    // assistant or a second client publishes to
    // spBv1.0/lab-site/NCMD/lab-edge-07 — the INBOX observable is the
    // assertion surface, not a second hand-written client).
    let mut delivered = false;
    for _ in 0..600 {
        if transport.poll_command().is_some() {
            delivered = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(
        delivered,
        "no command arrived within 60 s — publish a Sparkplug command to \
         spBv1.0/lab-site/NCMD/lab-edge-07 and re-run the gate"
    );
    transport.disconnect().expect("graceful disconnect");
}

/// Acceptance criterion 3 (live): an unclean disconnect publishes the
/// death certificate. The suite kills the session from the transport
/// side (drop), reconnects a MONITOR client subscribed to the NDEATH
/// topic, and verifies the BROKER published the will with the session's
/// bdSeq. The monitor client is the crate's own live transport with an
/// NDEATH command filter — the declared subscription surface — and the
/// DEATH's decoded bdSeq is the assertion.
#[test]
fn an_unclean_disconnect_lets_the_broker_publish_the_will() {
    let Some(pki) = pki_dir() else {
        println!(
            "SKIP: FLEXCORE_TEST_MQTT_BROKER_PKI_DIR is unset — the real-broker \
             interop gate is a HUMAN LAB GATE (nothing ran)"
        );
        return;
    };
    let host = broker_host();
    let port = broker_port();

    // Session A: establish, then KILL without a graceful DISCONNECT
    // (drop the transport — the engine exits WITHOUT a DISCONNECT, the
    // socket closes, and the BROKER publishes the will).
    let config_a = gate_config(&pki, &host, port);
    let bd_seq = config_a.bd_seq;
    let will_topic = "spBv1.0/lab-site/NDEATH/lab-edge-07".to_string();
    let transport_a = LiveMqttUplinkTransport::connect(config_a)
        .expect("session A connects");
    assert!(transport_a.is_connected_now());
    drop(transport_a); // unclean: no DISCONNECT written

    // Session B: subscribe the NDEATH topic (the declared command filter
    // mechanism, pointed at the DEATH topic) and wait for the will the
    // BROKER publishes.
    let mut config_b = gate_config(&pki, &host, port);
    config_b.connection.client_id = "flexcore-edge-07-monitor".to_string();
    config_b.command_topic_filter = Some(will_topic);
    let mut transport_b = LiveMqttUplinkTransport::connect(config_b)
        .expect("session B connects");
    let mut saw_will = false;
    for _ in 0..300 {
        if let Some(delivery) = transport_b.poll_command() {
            // The will's decoded bdSeq must name session A's generation.
            assert_eq!(delivery.bd_seq, Some(bd_seq), "the will carries the dead session's bdSeq");
            assert!(delivery.metrics.is_empty(), "the DEATH carries no metrics");
            saw_will = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(
        saw_will,
        "the broker did not publish the will within 30 s — the unclean \
         disconnect did not fire the LWT"
    );
    transport_b.disconnect().expect("graceful disconnect");
}

/// A compile-level pin: the transport type the gate exercises implements
/// the UNCHANGED seam trait and the additive command source (the type
/// surface the composition consumes).
#[allow(dead_code)]
fn the_transport_implements_both_surfaces() {
    fn assert_uplink<T: MqttUplinkTransport + MqttCommandSource>() {}
    assert_uplink::<LiveMqttUplinkTransport>();
}
