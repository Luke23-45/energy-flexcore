// P14.17 — The simulated MQTT broker sink (decision record 81; the seam
// is `flexcore_mqtt_adapter::transport::MqttUplinkTransport` + the
// session surface `birth_death::BirthDeathTracker` — the UNWIRED wire
// seam, record 43 §2).
//
// The UPLINK's receiving end: a simulated broker sink that accepts
// publishes through the [`MqttUplinkTransport`] trait, validates each
// payload against the adapter's OWN rules
// ([`SparkplugPayload::validate`]; the BIRTH-carries-seq rule), and
// records every accepted publish — plus, separately, feeds every
// accepted payload through a REAL [`BirthDeathTracker`] so the session
// state machine observes exactly what a broker-side consumer would.
//
// No broker connection, no TCP, no TLS handshake, no protobuf: the
// publishes are design-surface values and the sink stores them verbatim.
// The TLS posture is exercised the only way it can be without a socket:
// the connection config handed to the sink is validated through the
// adapter's own [`MqttConnectionConfig::validate`] (plaintext endpoints
// refused — the P6.07 rule), at construction, fail-fast.
//
// Deterministic and honest: publishes are recorded in publish order with
// sequence numbers; the sink's refusals are the named
// [`MqttTransportError::PublishRefused`] (added for exactly this seam in
// P14.17); every identity string carries the `sim-mqtt` prefix — never
// presentable as a real broker.

use std::sync::{Arc, Mutex};

use flexcore_mqtt_adapter::birth_death::BirthDeathTracker;
use flexcore_mqtt_adapter::model::SparkplugPayload;
use flexcore_mqtt_adapter::topic::ValidatedTopic;
use flexcore_mqtt_adapter::transport::{
    DeclaredQos, MqttConnectionConfig, MqttTransportError, MqttUplinkTransport, PublishReceipt,
};

/// The identity prefix every simulated MQTT peer carries in its identity
/// strings (never presentable as a real broker).
pub const SIM_MQTT_ID_PREFIX: &str = "sim-mqtt";

/// Errors the simulated broker reports. Every failure is named.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SimMqttError {
    /// The connection config was refused by the adapter's own TLS
    /// policy validation (re-raised; the simulated sink does not loosen
    /// the P6.07 rule).
    Config(MqttTransportError),
}

impl From<MqttTransportError> for SimMqttError {
    fn from(error: MqttTransportError) -> Self {
        SimMqttError::Config(error)
    }
}

impl std::fmt::Display for SimMqttError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SimMqttError::Config(error) => write!(f, "the simulated broker config was refused: {error}"),
        }
    }
}

impl std::error::Error for SimMqttError {}

/// One accepted publish, as the simulated broker records it: the topic,
/// the payload, the declared QoS, and the arrival sequence.
#[derive(Debug, Clone, PartialEq)]
pub struct AcceptedPublish {
    /// The arrival sequence (0-based, in publish order).
    pub sequence: u64,
    /// The topic the publish targeted (the validated topic string).
    pub topic: String,
    /// The payload as published (verbatim; no re-encoding).
    pub payload: SparkplugPayload,
    /// The declared QoS the publish carried.
    pub qos: DeclaredQos,
}

/// The simulated MQTT broker sink: the receiving end of the uplink seam.
#[derive(Debug, Clone)]
pub struct SimulatedMqttBroker {
    /// The broker's identity string (carries the `sim-mqtt` prefix).
    pub broker_id: String,
    /// The validated connection config the edge would dial out with (the
    /// TLS policy is the adapter's own; held for the record).
    pub config: MqttConnectionConfig,
    accepted: Arc<Mutex<Vec<AcceptedPublish>>>,
    /// The broker-side session observers, one per tracked edge node (the
    /// tracker consumes the SAME payloads the broker accepted — no
    /// parallel session vocabulary).
    trackers: Arc<Mutex<std::collections::BTreeMap<String, BirthDeathTracker>>>,
}

impl SimulatedMqttBroker {
    /// Constructs the simulated broker sink over a connection config
    /// validated through the adapter's own TLS policy (fail-fast).
    pub fn new(broker_id: impl Into<String>, config: MqttConnectionConfig) -> Result<Self, SimMqttError> {
        config.validate()?;
        Ok(Self {
            broker_id: format!("{SIM_MQTT_ID_PREFIX}:{}", broker_id.into()),
            config,
            accepted: Arc::new(Mutex::new(Vec::new())),
            trackers: Arc::new(Mutex::new(std::collections::BTreeMap::new())),
        })
    }

    /// The accepted-publish log handle (shared with the trait impl).
    pub fn accepted_log(&self) -> Arc<Mutex<Vec<AcceptedPublish>>> {
        Arc::clone(&self.accepted)
    }

    /// The tracker handle for one edge node's session state (the
    /// broker-side consumer's view; created lazily on first publish).
    pub fn tracker(
        &self,
        edge_node_id: &str,
    ) -> Result<BirthDeathTracker, MqttTransportError> {
        let mut trackers = self
            .trackers
            .lock()
            .map_err(|_| MqttTransportError::NotConnected {
                reason: "the simulated broker's tracker mutex was poisoned".to_string(),
            })?;
        Ok(trackers
            .entry(edge_node_id.to_string())
            .or_insert_with(BirthDeathTracker::new)
            .clone())
    }

    /// The accepted publish count (a convenience for assertions).
    pub fn accepted_count(&self) -> usize {
        self.accepted.lock().expect("the simulated broker's log mutex must not be poisoned").len()
    }
}

impl MqttUplinkTransport for SimulatedMqttBroker {
    /// Accepts one publish: the payload is validated against the
    /// adapter's OWN rules (a BIRTH/DATA payload without its required
    /// sequence is a named [`MqttTransportError::PublishRefused`] —
    /// never a fabricated receipt), the topic and payload are recorded
    /// verbatim, and the payload is fed to the edge node's
    /// [`BirthDeathTracker`] (the broker-side session view). The
    /// returned receipt is the simulated broker's acknowledgement —
    /// which, per the seam's own contract, is NOT delivery to any
    /// downstream consumer.
    fn publish(
        &mut self,
        topic: &ValidatedTopic,
        payload: &SparkplugPayload,
    ) -> Result<PublishReceipt, MqttTransportError> {
        // The adapter's own payload rule, applied by the receiving end.
        payload.validate().map_err(|error| MqttTransportError::PublishRefused {
            reason: format!("payload failed the adapter's own validation: {error}"),
        })?;
        let topic_string = topic.topic_string();
        let mut log = self
            .accepted
            .lock()
            .map_err(|_| MqttTransportError::NotConnected {
                reason: "the simulated broker's log mutex was poisoned".to_string(),
            })?;
        let sequence = log.len() as u64;
        log.push(AcceptedPublish {
            sequence,
            topic: topic_string,
            payload: payload.clone(),
            qos: self.config.qos,
        });
        drop(log);
        // The broker-side session view consumes the SAME payload (the
        // real BirthDeathTracker, not a parallel vocabulary).
        let mut trackers = self
            .trackers
            .lock()
            .map_err(|_| MqttTransportError::NotConnected {
                reason: "the simulated broker's tracker mutex was poisoned".to_string(),
            })?;
        let tracker = trackers
            .entry(topic.edge_node_id.value().to_string())
            .or_insert_with(BirthDeathTracker::new);
        let _observation = tracker.observe(payload);
        Ok(PublishReceipt { topic: topic.topic_string(), qos: self.config.qos })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flexcore_mqtt_adapter::model::{BdSeq, EdgeNodeId, GroupId, MetricValue, SparkplugMessageKind, SparkplugMetric, SparkplugSeq};

    fn broker() -> SimulatedMqttBroker {
        SimulatedMqttBroker::new(
            "broker-1",
            MqttConnectionConfig {
                endpoint: "mqtts://sim-broker.invalid:8883".to_string(),
                require_tls: true,
                client_id: "sim-edge-07".to_string(),
                qos: DeclaredQos::AtLeastOnce,
                trust_anchor_reference: "phase13-secrets/sim-trust-anchor".to_string(),
            },
        )
        .expect("valid simulated broker")
    }

    fn birth_payload(seq: u64) -> SparkplugPayload {
        SparkplugPayload {
            kind: SparkplugMessageKind::NodeBirth,
            seq: SparkplugSeq::new(seq).ok(),
            bd_seq: BdSeq::initial(),
            metrics: vec![
                SparkplugMetric::new(
                    "sim-asset-1.p.mw".to_string(),
                    None,
                    flexcore_mqtt_adapter::model::MetricType::Double,
                    MetricValue::Double(1.5),
                )
                .expect("valid metric"),
            ],
        }
    }

    fn birth_topic() -> ValidatedTopic {
        ValidatedTopic::new(
            GroupId::new("sim-site-42").expect("valid"),
            SparkplugMessageKind::NodeBirth,
            EdgeNodeId::new("sim-edge-07").expect("valid"),
            None,
        )
        .expect("valid topic")
    }

    #[test]
    fn plaintext_configs_are_refused_at_broker_construction() {
        // The sink does not loosen the P6.07 TLS rule: a plaintext
        // endpoint is refused by the adapter's own validation.
        assert!(matches!(
            SimulatedMqttBroker::new(
                "broker-2",
                MqttConnectionConfig {
                    endpoint: "mqtt://sim-broker.invalid:1883".to_string(),
                    require_tls: true,
                    client_id: "sim-edge-07".to_string(),
                    qos: DeclaredQos::AtLeastOnce,
                    trust_anchor_reference: "phase13-secrets/sim-trust-anchor".to_string(),
                },
            ),
            Err(SimMqttError::Config(MqttTransportError::InvalidConfig { .. }))
        ));
    }

    #[test]
    fn valid_publishes_are_recorded_in_order_and_acked() {
        let mut broker = broker();
        let topic = birth_topic();
        let receipt = broker.publish(&topic, &birth_payload(0)).expect("accepted");
        assert_eq!(receipt.qos, DeclaredQos::AtLeastOnce);
        assert!(receipt.topic.ends_with("/NBIRTH/sim-edge-07"));
        broker.publish(&topic, &birth_payload(1)).expect("accepted");
        assert_eq!(broker.accepted_count(), 2);
        let log = broker.accepted_log();
        let accepted = log.lock().expect("log mutex");
        assert_eq!(accepted[0].sequence, 0);
        assert_eq!(accepted[0].payload.seq, Some(SparkplugSeq::new(0).expect("valid")));
        assert_eq!(accepted[1].sequence, 1);
    }

    #[test]
    fn a_birth_without_its_required_seq_is_refused_never_acked() {
        let mut broker = broker();
        let mut payload = birth_payload(0);
        payload.seq = None; // BIRTH requires seq — the adapter's own rule.
        let error = broker
            .publish(&birth_topic(), &payload)
            .expect_err("refused");
        assert!(matches!(error, MqttTransportError::PublishRefused { .. }));
        assert_eq!(broker.accepted_count(), 0, "nothing was recorded");
    }

    #[test]
    fn the_broker_side_tracker_consumes_the_same_payloads() {
        let mut broker = broker();
        let topic = birth_topic();
        broker.publish(&topic, &birth_payload(0)).expect("accepted");
        // The broker-side session view observed the BIRTH through the
        // REAL tracker (no parallel session vocabulary): the session is
        // LIVE with the BIRTH's sequence.
        let tracker = broker.tracker("sim-edge-07").expect("tracker");
        assert!(
            matches!(
                tracker.state(),
                flexcore_mqtt_adapter::birth_death::NodeSessionState::Live { .. }
            ),
            "the tracker saw the NBIRTH"
        );
    }
}
