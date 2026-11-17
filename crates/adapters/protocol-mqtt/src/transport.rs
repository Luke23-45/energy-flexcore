// P14.11 — The MQTT wire SEAM (decision record 43_p14_11_mqtt_sparkplug_b_uplink.md;
// the record 02 §5 template: design the transport, land no dependency).
//
// There is NO MQTT client library in this crate's dependency line — no
// broker connection, no subscription, no publish has ever occurred. The
// wire transport is a designed seam: the trait below is the FULL surface a
// composition root would implement (over an MQTT 5 client chosen at
// implementation time, with the human-gated decision recorded), and
// [`UnwiredUplinkTransport`] is the shipped default that refuses every
// publish with [`MqttTransportError::SeamNotWired`].
//
// TLS POLICY (declared, mirroring the P6.07 uplink rule): the edge's MQTT
// hop is edge-INITIATED TLS or refused —
// [`MqttConnectionConfig::validate`] refuses plaintext broker schemes
// (`mqtt://` with TLS off) and requires either an `mqtts://` / `wss://`
// endpoint or an explicitly declared TLS policy with a pinned trust
// anchor reference from the deployment's credential record (never the
// ambient system store). No listener exists anywhere: the uplink dials
// OUT only (the P6.07 structural guarantee is a composition-root
// obligation; this crate contains no socket code at all to begin with).
//
// AT-LEAST-ONCE HONESTY: a publish is acknowledged by the broker per the
// MQTT QoS the composition root declares; the seam returns a
// [`PublishReceipt`] only after the transport's acknowledgement is read
// back. "Acknowledged" is the broker's receipt — NOT delivery to any
// consumer downstream (acknowledgement is not delivery, §14.1 — the same
// rule as the command lifecycle).

use serde::{Deserialize, Serialize};

use crate::model::SparkplugPayload;
use crate::topic::ValidatedTopic;

/// The declared MQTT QoS for uplink publishes.
///
/// UNVERIFIED — the exact QoS the Sparkplug B specification expects for
/// BIRTH/DATA/DEATH (public sources commonly state QoS 1) is an
/// implementation-time check against the text. The declared default here
/// is a design pin, recorded in the decision record — never a
/// specification quote.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeclaredQos {
    /// QoS 0 (at most once).
    AtMostOnce,
    /// QoS 1 (at least once) — the declared uplink default.
    AtLeastOnce,
    /// QoS 2 (exactly once).
    ExactlyOnce,
}

/// The declared connection configuration of the uplink.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MqttConnectionConfig {
    /// The broker endpoint (e.g. `mqtts://broker.example:8883`). Validated
    /// against the TLS policy below.
    pub endpoint: String,
    /// Whether the transport MUST use TLS (the declared default: true —
    /// plaintext uplinks are refused, the P6.07 rule).
    pub require_tls: bool,
    /// The client id the transport connects under (from the credential
    /// binding set; see [`crate::credentials`]).
    pub client_id: String,
    /// The declared QoS for uplink publishes (UNVERIFIED default — see
    /// [`DeclaredQos`]).
    pub qos: DeclaredQos,
    /// The reference to the trust-anchor source in the deployment's
    /// credential record (never the ambient system store — the P6.07
    /// rule). Required when TLS is required.
    pub trust_anchor_reference: String,
}

/// Errors returned by connection-config validation and the transport seam.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MqttTransportError {
    /// The configuration violates the declared TLS policy (plaintext
    /// endpoint with TLS required, missing trust-anchor reference, empty
    /// endpoint or client id).
    InvalidConfig {
        reason: String,
    },
    /// The publish requires an established connection and there is none.
    /// The adapter must NOT fabricate a receipt — the fail-closed rule.
    NotConnected {
        reason: String,
    },
    /// The peer refused the publish (P14.17: the simulated broker sink
    /// validates the payload and topic namespace before accepting; a
    /// malformed payload is a named refusal, never a fabricated receipt).
    /// A real transport maps its broker's negative acknowledgement onto
    /// this variant at implementation time.
    PublishRefused {
        reason: String,
    },
    /// The shipped [`UnwiredUplinkTransport`] refused the operation: no
    /// MQTT wire transport exists in this batch.
    SeamNotWired,
}

impl std::fmt::Display for MqttTransportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MqttTransportError::InvalidConfig { reason } => {
                write!(f, "MQTT connection config invalid: {reason}")
            }
            MqttTransportError::NotConnected { reason } => {
                write!(f, "MQTT uplink not connected: {reason}")
            }
            MqttTransportError::PublishRefused { reason } => {
                write!(f, "MQTT publish refused by the peer: {reason}")
            }
            MqttTransportError::SeamNotWired => {
                write!(
                    f,
                    "the MQTT wire transport is a designed, UNWIRED seam — no broker \
                     connection exists (decision record 43)"
                )
            }
        }
    }
}

impl std::error::Error for MqttTransportError {}

impl MqttConnectionConfig {
    /// Validates the configuration against the declared TLS policy.
    /// Plaintext is refused (the P6.07 rule); a TLS-requiring config
    /// without a trust-anchor reference is refused.
    pub fn validate(&self) -> Result<(), MqttTransportError> {
        if self.endpoint.trim().is_empty() {
            return Err(MqttTransportError::InvalidConfig {
                reason: "endpoint is empty".to_string(),
            });
        }
        if self.client_id.trim().is_empty() {
            return Err(MqttTransportError::InvalidConfig {
                reason: "client_id is empty".to_string(),
            });
        }
        if self.require_tls {
            let lowered = self.endpoint.to_lowercase();
            let tls_scheme =
                lowered.starts_with("mqtts://") || lowered.starts_with("wss://") || lowered.starts_with("ssl://");
            if !tls_scheme {
                return Err(MqttTransportError::InvalidConfig {
                    reason: format!(
                        "endpoint '{}' is not a TLS scheme and the uplink requires TLS \
                         (the P6.07 rule: the edge dials out over TLS or fails)",
                        self.endpoint
                    ),
                });
            }
            if self.trust_anchor_reference.trim().is_empty() {
                return Err(MqttTransportError::InvalidConfig {
                    reason: "TLS is required but no trust-anchor reference is declared".to_string(),
                });
            }
        }
        Ok(())
    }
}

/// The receipt of ONE acknowledged publish (the broker's acknowledgement —
/// NOT delivery to any consumer downstream; §14.1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublishReceipt {
    /// The topic the publish targeted.
    pub topic: String,
    /// The declared QoS the publish used.
    pub qos: DeclaredQos,
}

/// The MQTT uplink wire SEAM: the full surface a composition root would
/// implement over an MQTT 5 client chosen at implementation time. Sync
/// trait (the `flexcore-ports` house rule); the adapter owns no runtime.
///
/// Deliberately NO subscribe method and NO receive method: this batch is
/// the UPLINK OPTION (MonitorOnly) — the command path does not exist here.
/// A future command-relay surface is a reviewed scope change (the common
/// scope gate refuses it today; the P6.14-style disabled-by-default gate
/// is where it would land, not here).
pub trait MqttUplinkTransport {
    /// Publishes one validated Sparkplug payload to one validated topic.
    /// Returns the receipt only after the transport's acknowledgement is
    /// read back; an un-acked publish is `Err` and the caller (the
    /// edge-agent buffer seam, wave 3 composition) owns redelivery
    /// in order.
    fn publish(
        &mut self,
        topic: &ValidatedTopic,
        payload: &SparkplugPayload,
    ) -> Result<PublishReceipt, MqttTransportError>;
}

/// The shipped default transport: refuses every publish with
/// [`MqttTransportError::SeamNotWired`]. It exists so a composition root
/// has a constructible, honest placeholder — and so nothing can silently
/// "publish" without a real transport.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct UnwiredUplinkTransport;

impl MqttUplinkTransport for UnwiredUplinkTransport {
    fn publish(
        &mut self,
        _topic: &ValidatedTopic,
        _payload: &SparkplugPayload,
    ) -> Result<PublishReceipt, MqttTransportError> {
        Err(MqttTransportError::SeamNotWired)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plaintext_endpoints_are_refused_when_tls_is_required() {
        let config = MqttConnectionConfig {
            endpoint: "mqtt://broker.example:1883".to_string(),
            require_tls: true,
            client_id: "edge-07-uplink".to_string(),
            qos: DeclaredQos::AtLeastOnce,
            trust_anchor_reference: "phase13-secrets/uplink-trust-anchor".to_string(),
        };
        assert!(matches!(
            config.validate(),
            Err(MqttTransportError::InvalidConfig { .. })
        ));
    }

    #[test]
    fn tls_config_requires_a_trust_anchor_reference() {
        let config = MqttConnectionConfig {
            endpoint: "mqtts://broker.example:8883".to_string(),
            require_tls: true,
            client_id: "edge-07-uplink".to_string(),
            qos: DeclaredQos::AtLeastOnce,
            trust_anchor_reference: "  ".to_string(),
        };
        assert!(matches!(
            config.validate(),
            Err(MqttTransportError::InvalidConfig { .. })
        ));
    }

    #[test]
    fn a_valid_tls_config_passes() {
        let config = MqttConnectionConfig {
            endpoint: "mqtts://broker.example:8883".to_string(),
            require_tls: true,
            client_id: "edge-07-uplink".to_string(),
            qos: DeclaredQos::AtLeastOnce,
            trust_anchor_reference: "phase13-secrets/uplink-trust-anchor".to_string(),
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn the_unwired_transport_refuses_every_publish() {
        let mut transport = UnwiredUplinkTransport;
        let topic = crate::topic::ValidatedTopic::new(
            crate::model::GroupId::new("site-42").expect("valid"),
            crate::model::SparkplugMessageKind::NodeBirth,
            crate::model::EdgeNodeId::new("edge-07").expect("valid"),
            None,
        )
        .expect("valid topic");
        let payload = crate::model::SparkplugPayload {
            kind: crate::model::SparkplugMessageKind::NodeBirth,
            seq: crate::model::SparkplugSeq::new(0).ok(),
            bd_seq: crate::model::BdSeq::initial(),
            metrics: Vec::new(),
        };
        assert_eq!(
            transport.publish(&topic, &payload),
            Err(MqttTransportError::SeamNotWired)
        );
    }
}
