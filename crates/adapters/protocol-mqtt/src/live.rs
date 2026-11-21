// FIM-F05.03 / FIM-F05.04 / FIM-F05.05 — the LIVE MQTT 5 + Sparkplug B
// uplink transport (docs/releases/final-implementation-before-publication.md
// §5 F-05; ledger docs/releases/implementation_ledger/ledger_F05.md).
//
// **Implemented under FIM-F05, WRITTEN-UNRUN: this module has never been
// compiled or executed. Broker interop is UNPROVEN and remains a HUMAN LAB
// GATE** (crate conventions §7.3; the gated suite
// `tests/live_broker_gated.rs` and the socket-free suite
// `tests/live_session_socket_free.rs` are written-unrun evidence
// preparation, not results).
//
// What the module implements, against the seam designed in `transport.rs`:
//
// - **FIM-F05.03 — the live transport.** [`LiveMqttUplinkTransport`]
//   implements the UNCHANGED `MqttUplinkTransport` trait: `publish`
//   validates the payload, encodes it with the FIM-F05.02 Sparkplug wire
//   codec (`sparkplug_wire.rs`), frames it with the FIM-F05.01 MQTT 5
//   codec (`mqtt_wire.rs`), and returns the [`PublishReceipt`] ONLY after
//   the broker's PUBACK is read back (QoS 1; a QoS 0 publish has no
//   acknowledgement and its receipt is returned after the write is
//   flushed — declared honestly, the trait's own contract). QoS 2 is a
//   named refusal. [`UnwiredUplinkTransport`] stays UNTOUCHED as the
//   unconfigured default; the composition root selects the live one when
//   configured. The engine runs on its own OS thread with a private
//   current-thread tokio runtime (the sync house-rule trait must never
//   require an ambient async context); `publish` blocks the calling
//   thread for at most the declared `ack_timeout`.
// - **FIM-F05.04 — birth/data/death end-to-end.** On connect the engine
//   publishes NBIRTH then DBIRTH per declared device inventory, and
//   configures the MQTT will as the NDEATH LWT (topic
//   `spBv1.0/<group>/NDEATH/<node>`, payload = the encoded DEATH payload
//   carrying the session's bdSeq) — an UNCLEAN disconnect makes the
//   BROKER publish the will; the client never fakes a DEATH. Telemetry
//   publishes flow through unchanged (NDATA/DDATA are the caller's
//   payloads). The EXISTING `BirthDeathTracker` transitions are REUSED —
//   each constructed BIRTH is fed through a tracker as a self-check gate
//   before it may leave the edge (a self-refused BIRTH is a named
//   refusal, never a wire surprise); the socket-free test feeds the whole
//   published sequence through the same trackers.
// - **FIM-F05.05 — the command path.** When the config declares a
//   command-topic filter (an EXACT topic, no wildcards — validated), the
//   engine subscribes at connect and every PUBLISH on that topic is
//   decoded with the Sparkplug wire codec and delivered into a bounded
//   inbox ([`MqttCommandSource::poll_command`]). The edge-agent
//   composition (`crates/edge-agent/src/adapters/edge_to_cloud.rs`)
//   drains the inbox additively. A delivered command carries NO
//   authority: it enters the edge command lifecycle at PROPOSED and
//   nowhere else (the 14-state/27-edge lifecycle is untouched; nothing
//   here dispatches, authorizes, or relays). **No secret material is held
//   by the client** — the never-granted rule: the CONNECT carries the
//   declared username only, the password flag is never set
//   ([`ConnectOptions`] has no password field), and authentication
//   travels the TLS client-certificate identity; [`LiveMqttConfig`]
//   holds PATHS to deployment material, never material.
//
// TLS POLICY (mirroring the P6.07 rule and the ocpp/20305 fail-closed
// precedent): TLS via `tokio-rustls` per `MqttConnectionConfig` (the
// declared default `require_tls = true` is honoured); a plaintext endpoint
// (`mqtt://`) is refused UNLESS the config explicitly records a
// `plaintext_lab_posture_reason` — there is no flag that silently weakens
// the rule. Trust anchors load from the declared PEM path (never the
// ambient system store). `wss://` endpoints are a NAMED refusal (no
// WebSocket transport exists in this crate — declared limitation).
//
// UNVERIFIED-ASSUMPTION register (module-level; the model.rs discipline
// applies — a mismatch found at the human lab gate is a code or record
// edit, never a silent reinterpretation):
//
// - The one-seq-chain-per-edge-node reading (NBIRTH → DBIRTH → DATA share
//   the `seq` counter; the constructed-message chain here and the
//   caller's DATA chain are SEPARATE declared concerns — unifying them is
//   an implementation-time check against the specification text).
// - The will message is published with QoS 1 and retain = false.
// - Keep-alive: PINGREQ every `keep_alive / 2` seconds (>= 1 s); a
//   connection with no inbound bytes for `keep_alive + 1 s` is declared
//   lost (the engine exits WITHOUT a DISCONNECT — the broker-side will
//   fires, which is the FIM-F05.04 unclean-disconnect semantics). The
//   same holds for DROPPING the transport: dropping is an UNCLEAN
//   shutdown and the will fires; the graceful close is the explicit
//   `disconnect()` request.
// - Incoming QoS 1 publishes are PUBACKed on receipt (at-least-once
//   redelivery is the broker's concern; the inbox does not deduplicate).
// - No reconnection: after a connection loss the transport reports
//   `NotConnected` and the composition root owns redelivery IN ORDER
//   (the trait's own contract). A fresh session is a fresh `connect()`.

use std::collections::{BTreeMap, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc as std_mpsc, Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use futures::future::BoxFuture;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName};
use rustls::RootCertStore;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::mpsc;
use tokio::time::{timeout, Instant};
use tokio_rustls::TlsConnector;

use crate::birth_death::{BirthDeathTracker, Observation};
use crate::model::{
    BdSeq, DeviceId, SparkplugMessageKind, SparkplugMetric, SparkplugPayload, SparkplugSeq,
};
use crate::mqtt_wire::{
    encode_connect, encode_disconnect, encode_pingreq, encode_puback, encode_publish,
    encode_subscribe, decode_packet, frame_len, decode_varint, ConnectOptions, IncomingPacket,
    MqttReasonCode, MqttWireError, PublishOut, SubscribeOptions, WillMessage,
};
use crate::sparkplug_wire::{decode_payload, encode_payload, PayloadEncodingContext,
    SparkplugWireError};
use crate::topic::{ValidatedTopic, SPARKPLUG_B_NAMESPACE};
use crate::transport::{
    DeclaredQos, MqttConnectionConfig, MqttTransportError, MqttUplinkTransport, PublishReceipt,
};
use crate::uplink::SparkplugUplinkPlan;

/// The declared command-inbox capacity. A full inbox DROPS further
/// deliveries (declared, bounded — never unbounded).
const COMMAND_INBOX_CAP: usize = 1024;

/// The declared request-poll slice: the engine's bounded read never
/// waits longer than this before re-checking publish/disconnect
/// requests (the sequential serving loop's responsiveness bound —
/// request latency is at most one slice, never a full keep-alive
/// interval).
const REQUEST_POLL_SLICE: Duration = Duration::from_millis(100);

// --- Errors -------------------------------------------------------------------

/// The named errors of the live transport (the wire seam's
/// [`MqttTransportError`] vocabulary is re-raised at the trait boundary;
/// this richer surface names the live-specific refusals).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LiveUplinkError {
    /// The configuration violates a declared rule (TLS policy, keep
    /// alive, command-filter shape, endpoint shape).
    InvalidConfig {
        reason: String,
    },
    /// Deployment TLS material failed to load (a PATH problem — the
    /// material itself never enters this crate).
    MaterialLoadFailed {
        what: &'static str,
        reason: String,
    },
    /// Socket/TLS I/O failed.
    Io {
        reason: String,
    },
    /// A bounded phase (connect, acknowledgement wait) elapsed.
    Timeout {
        phase: &'static str,
    },
    /// The broker refused the CONNECT with a named reason code.
    ConnectRefused {
        reason: MqttReasonCode,
    },
    /// The broker refused the command SUBSCRIBE with a named reason code.
    SubscribeRefused {
        reason: MqttReasonCode,
    },
    /// The broker negatively acknowledged a publish.
    PublishRefused {
        reason: String,
    },
    /// The publish requires an established session and there is none.
    NotConnected {
        reason: String,
    },
    /// The declared QoS is not carried by this transport.
    QosUnsupported {
        qos: &'static str,
    },
    /// The wire codec refused a packet (re-raised).
    Wire(MqttWireError),
    /// The Sparkplug payload codec refused (re-raised).
    Payload(SparkplugWireError),
    /// The outbound self-check (`BirthDeathTracker`) refused a
    /// constructed BIRTH — the same trust rules apply to our own output.
    SelfCheckRefused {
        reason: String,
    },
    /// The session ended without a graceful DISCONNECT (the broker-side
    /// will fires — the unclean-disconnect semantics).
    ConnectionLost {
        reason: String,
    },
}

impl std::fmt::Display for LiveUplinkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LiveUplinkError::InvalidConfig { reason } => {
                write!(f, "live MQTT uplink config invalid: {reason}")
            }
            LiveUplinkError::MaterialLoadFailed { what, reason } => {
                write!(f, "live MQTT uplink TLS material failed to load ({what}): {reason}")
            }
            LiveUplinkError::Io { reason } => write!(f, "live MQTT uplink I/O error: {reason}"),
            LiveUplinkError::Timeout { phase } => {
                write!(f, "live MQTT uplink timed out: {phase}")
            }
            LiveUplinkError::ConnectRefused { reason } => {
                write!(f, "the broker refused the MQTT CONNECT: {reason}")
            }
            LiveUplinkError::SubscribeRefused { reason } => {
                write!(f, "the broker refused the command SUBSCRIBE: {reason}")
            }
            LiveUplinkError::PublishRefused { reason } => {
                write!(f, "the broker refused the publish: {reason}")
            }
            LiveUplinkError::NotConnected { reason } => {
                write!(f, "the MQTT uplink is not connected: {reason}")
            }
            LiveUplinkError::QosUnsupported { qos } => {
                write!(f, "the declared QoS {qos} is not carried by the live transport (declared subset: QoS 0/1)")
            }
            LiveUplinkError::Wire(error) => write!(f, "MQTT wire codec refused: {error}"),
            LiveUplinkError::Payload(error) => write!(f, "Sparkplug payload codec refused: {error}"),
            LiveUplinkError::SelfCheckRefused { reason } => {
                write!(f, "the outbound BIRTH self-check refused the message: {reason}")
            }
            LiveUplinkError::ConnectionLost { reason } => {
                write!(f, "the MQTT session was lost without a graceful DISCONNECT: {reason}")
            }
        }
    }
}

impl std::error::Error for LiveUplinkError {}

impl From<MqttWireError> for LiveUplinkError {
    fn from(error: MqttWireError) -> Self {
        LiveUplinkError::Wire(error)
    }
}

impl From<SparkplugWireError> for LiveUplinkError {
    fn from(error: SparkplugWireError) -> Self {
        LiveUplinkError::Payload(error)
    }
}

/// Maps a live-surface error onto the UNCHANGED seam vocabulary at the
/// trait boundary (the seam's variants carry the named reason).
fn map_live_error_to_transport(error: LiveUplinkError) -> MqttTransportError {
    match error {
        LiveUplinkError::InvalidConfig { reason } => MqttTransportError::InvalidConfig { reason },
        LiveUplinkError::NotConnected { reason } => MqttTransportError::NotConnected { reason },
        LiveUplinkError::ConnectionLost { reason } => MqttTransportError::NotConnected { reason },
        other => MqttTransportError::PublishRefused {
            reason: other.to_string(),
        },
    }
}

// --- Configuration --------------------------------------------------------------

/// One device's declared BIRTH inventory (the full metric list a DBIRTH
/// declares for that device).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeviceBirthInventory {
    pub device_id: DeviceId,
    pub metrics: Vec<SparkplugMetric>,
}

/// The live transport's configuration. NO secret material: paths and
/// declared identity bindings only (the never-granted rule — absence is
/// structural; see the module header). No `Default`: every field is a
/// declared deployment value (the no-defaults discipline).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LiveMqttConfig {
    /// The declared connection configuration (validated by the EXISTING
    /// `MqttConnectionConfig::validate` — the P6.07 TLS rule).
    pub connection: MqttConnectionConfig,
    /// The declared uplink identity (group + edge node; the BIRTH/DEATH
    /// topics derive from it).
    pub plan: SparkplugUplinkPlan,
    /// The declared username of the session (the credential binding's
    /// username; may be empty where the broker accepts username-less
    /// auth). NO password exists anywhere in this config.
    pub username: String,
    /// The bdSeq generation this session publishes (the composition root
    /// bumps it per reconnect — the declared saturating counter).
    pub bd_seq: BdSeq,
    /// The FULL node metric inventory the NBIRTH declares.
    pub node_birth_metrics: Vec<SparkplugMetric>,
    /// The per-device inventories, published as DBIRTHs after the NBIRTH.
    pub device_births: Vec<DeviceBirthInventory>,
    /// The trust-anchor PEM path (REQUIRED when TLS is required; loaded
    /// at connect, never the ambient system store — the P6.07 rule).
    pub trust_anchor_pem_path: String,
    /// Optional mTLS client identity PEM paths (declared together or not
    /// at all; the material loads at connect).
    pub client_certificate_pem_path: Option<String>,
    pub client_private_key_pem_path: Option<String>,
    /// The lab-posture escape hatch for PLAINTEXT uplinks: `require_tls =
    /// false` is refused UNLESS this names the recorded lab reason (the
    /// ocpp/20305 fail-closed precedent). Never for field deployments.
    pub plaintext_lab_posture_reason: Option<String>,
    /// Keep-alive in seconds (must be non-zero; PINGREQ every half).
    pub keep_alive_secs: u16,
    /// Session Expiry Interval property (declared; `None` omits it).
    pub session_expiry_secs: Option<u32>,
    /// Receive Maximum property (declared; `None` omits it).
    pub receive_maximum: Option<u16>,
    /// Topic Alias Maximum property — DECLARED, never used (no alias is
    /// ever sent by this transport).
    pub topic_alias_maximum: Option<u16>,
    /// The bounded connect phase (handshake + CONNACK + subscribe +
    /// births).
    pub connect_timeout: Duration,
    /// The bounded per-publish PUBACK wait.
    pub ack_timeout: Duration,
    /// The EXACT command topic the engine subscribes (no wildcards — a
    /// declared refusal otherwise). `None` = no subscription, no command
    /// path (the monitor-only posture).
    pub command_topic_filter: Option<String>,
}

impl LiveMqttConfig {
    /// Validates the configuration fail-closed. Every refusal is named;
    /// nothing is repaired or defaulted.
    pub fn validate(&self) -> Result<(), LiveUplinkError> {
        self.connection
            .validate()
            .map_err(|error| LiveUplinkError::InvalidConfig {
                reason: error.to_string(),
            })?;
        if self.connection.require_tls {
            if self.trust_anchor_pem_path.trim().is_empty() {
                return Err(LiveUplinkError::InvalidConfig {
                    reason: "TLS is required but trust_anchor_pem_path is empty".to_string(),
                });
            }
        } else if self
            .plaintext_lab_posture_reason
            .as_ref()
            .map_or(true, |reason| reason.trim().is_empty())
        {
            return Err(LiveUplinkError::InvalidConfig {
                reason: "plaintext (require_tls = false) is refused unless a \
                         plaintext_lab_posture_reason is recorded — the fail-closed rule"
                    .to_string(),
            });
        }
        if self.keep_alive_secs == 0 {
            return Err(LiveUplinkError::InvalidConfig {
                reason: "keep_alive_secs must be non-zero".to_string(),
            });
        }
        if self.connect_timeout.is_zero() || self.ack_timeout.is_zero() {
            return Err(LiveUplinkError::InvalidConfig {
                reason: "connect_timeout and ack_timeout must be non-zero".to_string(),
            });
        }
        if let Some(filter) = &self.command_topic_filter {
            if filter.trim().is_empty() {
                return Err(LiveUplinkError::InvalidConfig {
                    reason: "command_topic_filter is empty".to_string(),
                });
            }
            if filter.contains('+') || filter.contains('#') {
                return Err(LiveUplinkError::InvalidConfig {
                    reason: "command_topic_filter must be an EXACT topic (no wildcards; \
                             wildcard matching is not implemented and is refused, not guessed)"
                        .to_string(),
                });
            }
        }
        parse_endpoint(&self.connection.endpoint)?;
        Ok(())
    }
}

/// Parses the declared endpoint into `(host, port, tls)`. Supported
/// schemes: `mqtts://` and `ssl://` (TLS), `mqtt://` (plaintext — refused
/// by the config validation unless the lab posture is recorded). `wss://`
/// is a NAMED refusal (no WebSocket transport exists here). The declared
/// default ports are 8883 (TLS) and 1883 (plaintext).
fn parse_endpoint(endpoint: &str) -> Result<(String, u16, bool), LiveUplinkError> {
    let lowered = endpoint.to_lowercase();
    let (scheme, rest) = lowered
        .split_once("://")
        .ok_or_else(|| LiveUplinkError::InvalidConfig {
            reason: format!("endpoint '{endpoint}' carries no scheme"),
        })?;
    let tls = match scheme {
        "mqtts" | "ssl" => true,
        "mqtt" => false,
        "wss" => {
            return Err(LiveUplinkError::InvalidConfig {
                reason: "wss:// endpoints are not supported by this transport (declared \
                         limitation): use mqtts://"
                    .to_string(),
            })
        }
        other => {
            return Err(LiveUplinkError::InvalidConfig {
                reason: format!("endpoint scheme '{other}' is not declared"),
            })
        }
    };
    if rest.contains('/') {
        return Err(LiveUplinkError::InvalidConfig {
            reason: format!("endpoint '{endpoint}' carries a path; only host[:port] is declared"),
        });
    }
    let (host, port) = match rest.rsplit_once(':') {
        Some((host, port)) => {
            let port = port.parse::<u16>().map_err(|error| {
                LiveUplinkError::InvalidConfig {
                    reason: format!("endpoint port '{port}' does not parse: {error}"),
                }
            })?;
            (host.to_string(), port)
        }
        None => (rest.to_string(), if tls { 8883 } else { 1883 }),
    };
    if host.trim().is_empty() {
        return Err(LiveUplinkError::InvalidConfig {
            reason: format!("endpoint '{endpoint}' carries no host"),
        });
    }
    Ok((host, port, tls))
}

/// The declared command-topic filter for the edge node's NCMD surface:
/// `spBv1.0/<group>/NCMD/<node>` (the namespace constant plus the
/// DECLARED message-kind segment — built directly, because the
/// `ValidatedTopic` grammar refuses an edge-node-scoped command kind in
/// this batch's declared reading).
pub fn node_command_topic_filter(plan: &SparkplugUplinkPlan) -> String {
    format!(
        "{}/{}/{}/{}",
        SPARKPLUG_B_NAMESPACE,
        plan.group_id.value(),
        SparkplugMessageKind::Command.topic_segment(),
        plan.edge_node_id.value()
    )
}

// --- The broker connection (TLS via tokio-rustls; mockable seam) -----------------

/// The wire stream the engine pumps (a TLS client stream in production;
/// an in-memory duplex in the socket-free tests).
pub trait BrokerStream: AsyncRead + AsyncWrite + Unpin + Send + 'static {}

// Batch 68: the blanket implementation the trait needs — every
// AsyncRead + AsyncWrite + Unpin + Send stream IS a broker stream (the
// production TLS client stream, the plaintext TCP stream, and the
// in-memory duplex of the socket-free tests). Without it the trait had
// no implementations and neither `Box::new(stream)` cast compiled.
impl<T: AsyncRead + AsyncWrite + Unpin + Send + 'static> BrokerStream for T {}

/// The broker connector seam: `connect()` yields one established stream.
/// The production implementation dials OUT (TCP + TLS, the P6.07 rule);
/// the socket-free tests inject an in-memory duplex. NEVER a listener.
pub trait BrokerConnector: Send + Sync + 'static {
    fn connect(&self) -> BoxFuture<'static, Result<Box<dyn BrokerStream>, LiveUplinkError>>;
}

/// The production connector: `tokio-rustls` over TCP when TLS is
/// required, plain TCP only under the recorded lab posture.
#[derive(Clone)]
pub struct TlsBrokerConnector {
    host: String,
    port: u16,
    connector: Option<Arc<TlsConnector>>,
}

impl TlsBrokerConnector {
    /// Builds the connector from the validated config, loading the
    /// deployment's TLS material at construction (paths → PEM → DER;
    /// the material never persists in this crate). Mirrors the
    /// protocol-20305 fail-closed loading discipline.
    pub fn new(config: &LiveMqttConfig) -> Result<Self, LiveUplinkError> {
        let (host, port, tls) = parse_endpoint(&config.connection.endpoint)?;
        if tls != config.connection.require_tls {
            return Err(LiveUplinkError::InvalidConfig {
                reason: format!(
                    "endpoint scheme TLS posture ({tls}) disagrees with require_tls ({})",
                    config.connection.require_tls
                ),
            });
        }
        let connector = if config.connection.require_tls {
            let roots = load_trust_anchor(&config.trust_anchor_pem_path)?;
            let builder = rustls::ClientConfig::builder_with_provider(Arc::new(
                rustls::crypto::ring::default_provider(),
            ))
            .with_safe_default_protocol_versions()
            .map_err(|error| LiveUplinkError::MaterialLoadFailed {
                what: "protocol version selection",
                reason: error.to_string(),
            })?;
            let tls_config = match (
                &config.client_certificate_pem_path,
                &config.client_private_key_pem_path,
            ) {
                (Some(cert_path), Some(key_path)) => {
                    let chain = load_certificate_chain(cert_path)?;
                    let key = load_private_key(key_path)?;
                    builder
                        .with_root_certificates(roots)
                        .with_client_auth_cert(chain, key)
                        .map_err(|error| LiveUplinkError::MaterialLoadFailed {
                            what: "client certificate chain and private key pairing",
                            reason: error.to_string(),
                        })?
                }
                (None, None) => builder.with_root_certificates(roots).with_no_client_auth(),
                _ => {
                    return Err(LiveUplinkError::InvalidConfig {
                        reason: "client_certificate_pem_path and client_private_key_pem_path \
                                 must be declared together (an mTLS identity is a pair)"
                            .to_string(),
                    })
                }
            };
            Some(Arc::new(TlsConnector::from(Arc::new(tls_config))))
        } else {
            None
        };
        Ok(Self { host, port, connector })
    }
}

impl BrokerConnector for TlsBrokerConnector {
    fn connect(&self) -> BoxFuture<'static, Result<Box<dyn BrokerStream>, LiveUplinkError>> {
        let this = self.clone();
        Box::pin(async move {
            let address = format!("{}:{}", this.host, this.port);
            let tcp = tokio::net::TcpStream::connect(&address)
                .await
                .map_err(|error| LiveUplinkError::Io {
                    reason: format!("tcp connect to {address}: {error}"),
                })?;
            match &this.connector {
                Some(connector) => {
                    let server_name = ServerName::try_from(this.host.clone()).map_err(|error| {
                        LiveUplinkError::InvalidConfig {
                            reason: format!("broker host '{}' is not a TLS server name: {error}", this.host),
                        }
                    })?;
                    let stream = connector
                        .connect(server_name, tcp)
                        .await
                        .map_err(|error| LiveUplinkError::Io {
                            reason: format!("TLS handshake with {address}: {error}"),
                        })?;
                    Ok(Box::new(stream) as Box<dyn BrokerStream>)
                }
                None => Ok(Box::new(tcp) as Box<dyn BrokerStream>),
            }
        })
    }
}

fn load_trust_anchor(path: &str) -> Result<RootCertStore, LiveUplinkError> {
    let file = std::fs::File::open(path).map_err(|error| LiveUplinkError::MaterialLoadFailed {
        what: "trust anchor",
        reason: format!("{path}: {error}"),
    })?;
    let mut reader = std::io::BufReader::new(file);
    let anchors: Vec<CertificateDer<'static>> = rustls_pemfile::certs(&mut reader)
        .collect::<Result<Vec<_>, std::io::Error>>()
        .map_err(|error| LiveUplinkError::MaterialLoadFailed {
            what: "trust anchor",
            reason: format!("{path}: PEM decoding failed: {error}"),
        })?;
    if anchors.is_empty() {
        return Err(LiveUplinkError::MaterialLoadFailed {
            what: "trust anchor",
            reason: format!("{path}: no PEM certificates found"),
        });
    }
    let mut roots = RootCertStore::empty();
    for anchor in anchors {
        roots.add(anchor).map_err(|error| {
            LiveUplinkError::MaterialLoadFailed {
                what: "trust anchor",
                reason: format!("{path}: {error}"),
            }
        })?;
    }
    Ok(roots)
}

fn load_certificate_chain(path: &str) -> Result<Vec<CertificateDer<'static>>, LiveUplinkError> {
    let file = std::fs::File::open(path).map_err(|error| LiveUplinkError::MaterialLoadFailed {
        what: "client certificate chain",
        reason: format!("{path}: {error}"),
    })?;
    let mut reader = std::io::BufReader::new(file);
    let chain: Vec<CertificateDer<'static>> = rustls_pemfile::certs(&mut reader)
        .collect::<Result<Vec<_>, std::io::Error>>()
        .map_err(|error| LiveUplinkError::MaterialLoadFailed {
            what: "client certificate chain",
            reason: format!("{path}: PEM decoding failed: {error}"),
        })?;
    if chain.is_empty() {
        return Err(LiveUplinkError::MaterialLoadFailed {
            what: "client certificate chain",
            reason: format!("{path}: no PEM certificates found"),
        });
    }
    Ok(chain)
}

fn load_private_key(path: &str) -> Result<PrivateKeyDer<'static>, LiveUplinkError> {
    let file = std::fs::File::open(path).map_err(|error| LiveUplinkError::MaterialLoadFailed {
        what: "client private key",
        reason: format!("{path}: {error}"),
    })?;
    let mut reader = std::io::BufReader::new(file);
    rustls_pemfile::private_key(&mut reader)
        .map_err(|error| LiveUplinkError::MaterialLoadFailed {
            what: "client private key",
            reason: format!("{path}: PEM decoding failed: {error}"),
        })?
        .ok_or(LiveUplinkError::MaterialLoadFailed {
            what: "client private key",
            reason: format!("{path}: no PEM private key found"),
        })
}

// --- The command path (FIM-F05.05) ------------------------------------------------

/// One command delivered from the subscribed command topic. It carries
/// NO authority: the edge-agent composition enters it into the command
/// lifecycle at PROPOSED and nowhere else (the frozen lifecycle is
/// untouched — no lifecycle state is constructed here).
#[derive(Debug, Clone, PartialEq)]
pub struct SparkplugCommandDelivery {
    /// The topic the command arrived on (the exact declared filter).
    pub topic: String,
    pub timestamp_millis: Option<u64>,
    pub uuid: Option<String>,
    pub seq: Option<SparkplugSeq>,
    pub bd_seq: Option<BdSeq>,
    pub metrics: Vec<SparkplugMetric>,
}

/// The additive command-delivery surface (the uplink trait itself stays
/// publish-only — existing types untouched). The composition root drains
/// the inbox; deliveries enter the command lifecycle at PROPOSED.
pub trait MqttCommandSource: Send {
    /// Pops the oldest delivered command, if any (non-blocking).
    fn poll_command(&mut self) -> Option<SparkplugCommandDelivery>;
    /// Whether the underlying session is currently established.
    fn is_connected_now(&self) -> bool;
}

// --- The engine ------------------------------------------------------------------

/// A request from the sync facade to the engine thread.
enum EngineRequest {
    Publish {
        topic: String,
        qos: DeclaredQos,
        payload: Vec<u8>,
        reply: std_mpsc::Sender<Result<(), LiveUplinkError>>,
    },
    Disconnect {
        reply: std_mpsc::Sender<Result<(), LiveUplinkError>>,
    },
}

/// A publish awaiting its PUBACK (QoS 1; the at-least-once receipt
/// contract — the receipt is released only when the PUBACK is read).
struct PendingPublish {
    reply: std_mpsc::Sender<Result<(), LiveUplinkError>>,
}

fn reply_request_error(request: EngineRequest, error: &LiveUplinkError) {
    match request {
        EngineRequest::Publish { reply, .. } => {
            let _ = reply.send(Err(error.clone()));
        }
        EngineRequest::Disconnect { reply } => {
            let _ = reply.send(Err(error.clone()));
        }
    }
}

fn next_packet_id(current: u16) -> u16 {
    if current == u16::MAX {
        1
    } else {
        current + 1
    }
}

/// Unix milliseconds (declared: a clock before the epoch saturates at 0;
/// the payload timestamp is advisory metadata, never a trust input).
fn unix_millis_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|since| since.as_millis() as u64)
        .unwrap_or(0)
}

/// Splits a complete frame into `(first_byte, remaining_after_varint)`.
fn split_frame(frame: &[u8]) -> Result<(u8, &[u8]), MqttWireError> {
    if frame.is_empty() {
        return Err(MqttWireError::Malformed {
            packet: "frame",
            reason: "empty frame".to_string(),
        });
    }
    let (_, varint_len) = decode_varint(&frame[1..])?;
    Ok((frame[0], &frame[1 + varint_len..]))
}

async fn pump_read(
    stream: &mut Box<dyn BrokerStream>,
    buf: &mut Vec<u8>,
) -> Result<usize, LiveUplinkError> {
    let mut chunk = [0u8; 4096];
    let read = stream.read(&mut chunk).await;
    let n = read.map_err(|error| LiveUplinkError::Io {
        reason: error.to_string(),
    })?;
    buf.extend_from_slice(&chunk[..n]);
    Ok(n)
}

/// Reads ONE broker packet (bounded wait; the cancellation-safe pump
/// appends into `buf`, so an interrupted wait never loses bytes).
async fn next_packet(
    stream: &mut Box<dyn BrokerStream>,
    buf: &mut Vec<u8>,
    wait: Duration,
) -> Result<IncomingPacket, LiveUplinkError> {
    let read = timeout(wait, async {
        loop {
            if let Some(total) = frame_len(buf).map_err(LiveUplinkError::Wire)? {
                let frame: Vec<u8> = buf.drain(..total).collect();
                let (first, rest) = split_frame(&frame).map_err(LiveUplinkError::Wire)?;
                return decode_packet(first, rest).map_err(LiveUplinkError::Wire);
            }
            if pump_read(stream, buf).await? == 0 {
                return Err(LiveUplinkError::ConnectionLost {
                    reason: "broker closed the connection (EOF)".to_string(),
                });
            }
        }
    })
    .await;
    match read {
        Ok(outcome) => outcome,
        Err(_) => Err(LiveUplinkError::Timeout {
            phase: "awaiting a broker packet",
        }),
    }
}

/// Constructs the NDEATH will message from the declared config (PURE —
/// unit-tested directly): topic `spBv1.0/<group>/NDEATH/<node>`, payload
/// = the encoded DEATH payload carrying the session bdSeq (no seq, no
/// metrics — the DEATH does not carry them in the declared reading).
fn will_message_for(config: &LiveMqttConfig) -> Result<WillMessage, SparkplugWireError> {
    let topic = ValidatedTopic::new(
        config.plan.group_id.clone(),
        SparkplugMessageKind::NodeDeath,
        config.plan.edge_node_id.clone(),
        None,
    )
    .map_err(|error| SparkplugWireError::Malformed {
        reason: error.to_string(),
    })?;
    let death_payload = SparkplugPayload {
        kind: SparkplugMessageKind::NodeDeath,
        seq: None,
        bd_seq: config.bd_seq,
        metrics: Vec::new(),
    };
    let payload = encode_payload(
        &death_payload,
        &PayloadEncodingContext {
            timestamp_millis: None,
            uuid: None,
        },
    )?;
    Ok(WillMessage {
        topic: topic.topic_string(),
        payload,
        qos: DeclaredQos::AtLeastOnce,
        retain: false,
    })
}

/// Constructs the CONNECT from the declared config (PURE — unit-tested
/// directly). The password flag is NEVER set: [`ConnectOptions`] has no
/// password field (the never-granted `secret.read` rule).
fn connect_options_for(
    config: &LiveMqttConfig,
    will: &WillMessage,
) -> Result<ConnectOptions, LiveUplinkError> {
    Ok(ConnectOptions {
        client_id: config.connection.client_id.clone(),
        keep_alive_secs: config.keep_alive_secs,
        clean_start: true,
        session_expiry_secs: config.session_expiry_secs,
        receive_maximum: config.receive_maximum,
        topic_alias_maximum: config.topic_alias_maximum,
        will: Some(will.clone()),
        username: if config.username.is_empty() {
            None
        } else {
            Some(config.username.clone())
        },
    })
}

/// One constructed BIRTH (node or device) ready to publish.
struct PlannedBirth {
    topic: ValidatedTopic,
    payload: SparkplugPayload,
}

/// Builds the NBIRTH + DBIRTH sequence (PURE): the seq chain is ONE chain
/// per edge node in the declared reading (NBIRTH seq 0, then each DBIRTH
/// continues it — UNVERIFIED; module header). Each payload is fed through
/// the EXISTING `BirthDeathTracker` as a self-check gate (a tracker per
/// node and per device — the machine's own transitions, reused).
fn plan_births(config: &LiveMqttConfig) -> Result<Vec<PlannedBirth>, LiveUplinkError> {
    let mut node_tracker = BirthDeathTracker::new();
    let mut device_trackers: BTreeMap<String, BirthDeathTracker> = BTreeMap::new();
    let mut seq = SparkplugSeq::new(0).map_err(|error| LiveUplinkError::SelfCheckRefused {
        reason: error.to_string(),
    })?;
    let mut planned = Vec::new();
    let mut push_birth = |planned: &mut Vec<PlannedBirth>,
                          topic: ValidatedTopic,
                          metrics: Vec<SparkplugMetric>,
                          seq: &mut SparkplugSeq,
                          tracker: &mut BirthDeathTracker|
     -> Result<(), LiveUplinkError> {
        let payload = SparkplugPayload {
            kind: topic.kind,
            seq: Some(*seq),
            bd_seq: config.bd_seq,
            metrics,
        };
        // The self-check: the SAME trust machine the platform side runs
        // judges our own outbound BIRTH. A refusal is a named error —
        // the message never leaves the edge.
        match tracker.observe(&payload) {
            Observation::Accepted { .. } => {}
            Observation::Refused { reason, .. } => {
                return Err(LiveUplinkError::SelfCheckRefused {
                    reason: format!("{} self-check refused: {}", topic.kind.topic_segment(), reason.name()),
                })
            }
        }
        payload.validate().map_err(|error| LiveUplinkError::SelfCheckRefused {
            reason: error.to_string(),
        })?;
        planned.push(PlannedBirth { topic, payload });
        *seq = seq.next();
        Ok(())
    };
    let node_topic = ValidatedTopic::new(
        config.plan.group_id.clone(),
        SparkplugMessageKind::NodeBirth,
        config.plan.edge_node_id.clone(),
        None,
    )
    .map_err(|error| LiveUplinkError::SelfCheckRefused {
        reason: error.to_string(),
    })?;
    push_birth(
        &mut planned,
        node_topic,
        config.node_birth_metrics.clone(),
        &mut seq,
        &mut node_tracker,
    )?;
    for device in &config.device_births {
        let device_topic = ValidatedTopic::new(
            config.plan.group_id.clone(),
            SparkplugMessageKind::DeviceBirth,
            config.plan.edge_node_id.clone(),
            Some(device.device_id.clone()),
        )
        .map_err(|error| LiveUplinkError::SelfCheckRefused {
            reason: error.to_string(),
        })?;
        let tracker = device_trackers
            .entry(device.device_id.value().to_string())
            .or_insert_with(BirthDeathTracker::new);
        push_birth(
            &mut planned,
            device_topic,
            device.metrics.clone(),
            &mut seq,
            tracker,
        )?;
    }
    Ok(planned)
}

async fn write_frame(stream: &mut Box<dyn BrokerStream>, frame: &[u8]) -> Result<(), LiveUplinkError> {
    stream
        .write_all(frame)
        .await
        .map_err(|error| LiveUplinkError::Io {
            reason: error.to_string(),
        })?;
    stream.flush().await.map_err(|error| LiveUplinkError::Io {
        reason: error.to_string(),
    })
}

/// Publishes one planned birth (QoS 1; the receipt waits for the PUBACK
/// within `ack_timeout`).
async fn publish_birth(
    stream: &mut Box<dyn BrokerStream>,
    planned: &PlannedBirth,
    packet_id: u16,
    ack_timeout: Duration,
    buf: &mut Vec<u8>,
) -> Result<(), LiveUplinkError> {
    let payload_bytes = encode_payload(
        &planned.payload,
        &PayloadEncodingContext {
            timestamp_millis: Some(unix_millis_now()),
            uuid: None,
        },
    )?;
    let frame = encode_publish(&PublishOut {
        topic: planned.topic.topic_string(),
        payload: payload_bytes,
        qos: DeclaredQos::AtLeastOnce,
        packet_id,
        retain: false,
        dup: false,
    })?;
    write_frame(stream, &frame).await?;
    match next_packet(stream, buf, ack_timeout).await? {
        IncomingPacket::Puback(puback) if puback.packet_id == packet_id => {
            if puback.reason_code == MqttReasonCode::Success {
                Ok(())
            } else {
                Err(LiveUplinkError::PublishRefused {
                    reason: format!(
                        "the broker PUBACKed birth {} with {}",
                        planned.topic.topic_string(),
                        puback.reason_code
                    ),
                })
            }
        }
        IncomingPacket::Puback(_) => Err(LiveUplinkError::Wire(MqttWireError::Malformed {
            packet: "PUBACK",
            reason: "PUBACK packet identifier does not match the outstanding publish"
                .to_string(),
        })),
        other => Err(LiveUplinkError::Wire(MqttWireError::Malformed {
            packet: "PUBACK",
            reason: format!("expected a PUBACK, got {other:?}"),
        })),
    }
}

/// The connect phase: CONNECT → CONNACK → (SUBSCRIBE → SUBACK) →
/// NBIRTH → DBIRTHs. Returns the next free packet identifier.
async fn perform_connect(
    stream: &mut Box<dyn BrokerStream>,
    config: &LiveMqttConfig,
    buf: &mut Vec<u8>,
) -> Result<u16, LiveUplinkError> {
    let will = will_message_for(config)?;
    let options = connect_options_for(config, &will)?;
    write_frame(stream, &encode_connect(&options)?).await?;
    match next_packet(stream, buf, config.connect_timeout).await? {
        IncomingPacket::Connack(fields) => {
            let reason = fields.reason_code.unwrap_or(MqttReasonCode::Success);
            if reason != MqttReasonCode::Success {
                return Err(LiveUplinkError::ConnectRefused { reason });
            }
        }
        other => {
            return Err(LiveUplinkError::Wire(MqttWireError::Malformed {
                packet: "CONNACK",
                reason: format!("the first broker packet was not a CONNACK: {other:?}"),
            }))
        }
    }
    if let Some(filter) = &config.command_topic_filter {
        write_frame(
            stream,
            &encode_subscribe(1, filter, &SubscribeOptions::default())?,
        )
        .await?;
        match next_packet(stream, buf, config.connect_timeout).await? {
            IncomingPacket::Suback(suback) => {
                // The granted codes (0x00-0x02) are successes; anything at
                // or above 0x80 is a refusal.
                let refused = suback.reason_codes.iter().copied().find(|code| code.as_u8() >= 0x80);
                if let Some(reason) = refused {
                    return Err(LiveUplinkError::SubscribeRefused { reason });
                }
            }
            other => {
                return Err(LiveUplinkError::Wire(MqttWireError::Malformed {
                    packet: "SUBACK",
                    reason: format!("expected a SUBACK, got {other:?}"),
                }))
            }
        }
    }
    let planned = plan_births(config)?;
    let mut packet_id = 1u16;
    for birth in &planned {
        publish_birth(stream, birth, packet_id, config.ack_timeout, buf).await?;
        packet_id = next_packet_id(packet_id);
    }
    Ok(packet_id)
}

/// The serving loop (sequential, borrow-fight-free): drain requests →
/// bounded read → handle frames → keep-alive. A fatal outcome EXITS
/// WITHOUT a DISCONNECT (the broker-side will fires — the unclean
/// disconnect semantics); only the graceful paths write DISCONNECT.
async fn serve(
    stream: &mut Box<dyn BrokerStream>,
    config: &LiveMqttConfig,
    tok_rx: &mut mpsc::Receiver<EngineRequest>,
    inbox: &Arc<Mutex<VecDeque<SparkplugCommandDelivery>>>,
    next_packet_id_start: u16,
) -> Result<(), LiveUplinkError> {
    let ping_every = Duration::from_secs(std::cmp::max(1, u64::from(config.keep_alive_secs) / 2));
    let idle_limit = Duration::from_secs(u64::from(config.keep_alive_secs)) + Duration::from_secs(1);
    let mut buf: Vec<u8> = Vec::new();
    let mut pending: BTreeMap<u16, PendingPublish> = BTreeMap::new();
    // The LAST packet identifier used (the successor is the next free).
    let mut last_packet_id = next_packet_id_start;
    let mut next_ping = Instant::now() + ping_every;
    let mut last_rx = Instant::now();
    loop {
        // 1. Drain pending requests (non-blocking). A disconnected
        //    request channel is the graceful-shutdown signal.
        loop {
            match tok_rx.try_recv() {
                Ok(EngineRequest::Disconnect { reply }) => {
                    let written = write_frame(
                        stream,
                        &encode_disconnect(MqttReasonCode::NormalDisconnection),
                    )
                    .await;
                    let _ = reply.send(written);
                    return Ok(());
                }
                Ok(EngineRequest::Publish {
                    topic,
                    qos,
                    payload,
                    reply,
                }) => {
                    if qos == DeclaredQos::ExactlyOnce {
                        let _ = reply.send(Err(LiveUplinkError::QosUnsupported {
                            qos: "exactly_once",
                        }));
                        continue;
                    }
                    let packet_id = next_packet_id(last_packet_id);
                    let frame = encode_publish(&PublishOut {
                        topic: topic.clone(),
                        payload,
                        qos,
                        packet_id: if qos == DeclaredQos::AtLeastOnce {
                            packet_id
                        } else {
                            0
                        },
                        retain: false,
                        dup: false,
                    });
                    match frame {
                        Ok(frame) => {
                            if let Err(error) = write_frame(stream, &frame).await {
                                let _ = reply.send(Err(error.clone()));
                                return Err(error);
                            }
                            if qos == DeclaredQos::AtLeastOnce {
                                last_packet_id = packet_id;
                                pending.insert(packet_id, PendingPublish { reply });
                            } else {
                                // QoS 0: no broker acknowledgement exists;
                                // the receipt is released after the write
                                // is flushed (declared honestly).
                                let _ = reply.send(Ok(()));
                            }
                        }
                        Err(error) => {
                            let _ = reply.send(Err(LiveUplinkError::Wire(error)));
                        }
                    }
                }
                // Batch 68: `tok_rx` is a TOKIO mpsc receiver, so the
                // error arms must name tokio's `TryRecvError`, not
                // `std_mpsc`'s — the two types share a name and variants
                // but are distinct types.
                Err(mpsc::error::TryRecvError::Empty) => break,
                Err(mpsc::error::TryRecvError::Disconnected) => {
                    // The facade is gone: an UNCLEAN shutdown — the
                    // engine exits WITHOUT a DISCONNECT, so the broker
                    // publishes the NDEATH will (the FIM-F05.04
                    // semantics). A graceful close is the explicit
                    // `disconnect()` request, never a drop.
                    return Ok(());
                }
            }
        }
        // 2. The idle/keep-alive watch: no inbound bytes for the idle
        //    limit is a declared connection loss (no DISCONNECT is
        //    written — the broker-side will fires).
        if (Instant::now() - last_rx) > idle_limit {
            return Err(LiveUplinkError::ConnectionLost {
                reason: format!("no broker bytes for {idle_limit:?} (keep-alive timeout)"),
            });
        }
        // 3. Bounded read: at most the request-poll slice (so the
        //    request drain re-runs promptly), and never past the next
        //    PINGREQ deadline.
        let now = Instant::now();
        let wait = if next_ping > now {
            (next_ping - now).min(REQUEST_POLL_SLICE)
        } else {
            Duration::ZERO
        };
        if wait.is_zero() {
            if let Err(error) = write_frame(stream, &encode_pingreq()).await {
                return Err(error);
            }
            next_ping = Instant::now() + ping_every;
            continue;
        }
        match timeout(wait, pump_read(stream, &mut buf)).await {
            Err(_) => {} // bounded wait elapsed; loop continues
            Ok(Err(error)) => return Err(error),
            Ok(Ok(0)) => {
                return Err(LiveUplinkError::ConnectionLost {
                    reason: "broker closed the connection (EOF)".to_string(),
                })
            }
            Ok(Ok(_)) => {
                last_rx = Instant::now();
                // Handle every complete buffered frame.
                loop {
                    let total = match frame_len(&buf).map_err(LiveUplinkError::Wire)? {
                        Some(total) => total,
                        None => break,
                    };
                    let frame: Vec<u8> = buf.drain(..total).collect();
                    let (first, rest) = split_frame(&frame).map_err(LiveUplinkError::Wire)?;
                    let packet = decode_packet(first, rest).map_err(LiveUplinkError::Wire)?;
                    match packet {
                        IncomingPacket::Publish(publish) => {
                            // Acknowledge incoming QoS 1 publishes on
                            // receipt (declared; the redelivery discipline
                            // is the broker's).
                            if publish.qos == DeclaredQos::AtLeastOnce {
                                if let Some(packet_id) = publish.packet_id {
                                    write_frame(stream, &encode_puback(packet_id)).await?;
                                }
                            }
                            // The command path: ONLY the exact declared
                            // filter delivers; every other topic is
                            // consciously dropped (monitor-only
                            // elsewhere).
                            if Some(publish.topic.as_str()) == config.command_topic_filter.as_deref() {
                                let decoded = decode_payload(&publish.payload)?;
                                let delivery = SparkplugCommandDelivery {
                                    topic: publish.topic,
                                    timestamp_millis: decoded.timestamp_millis,
                                    uuid: decoded.uuid,
                                    seq: decoded.seq,
                                    bd_seq: decoded.bd_seq,
                                    metrics: decoded.metrics,
                                };
                                let mut guard = inbox
                                    .lock()
                                    .map_err(|_| LiveUplinkError::Io {
                                        reason: "command inbox mutex poisoned".to_string(),
                                    })?;
                                if guard.len() < COMMAND_INBOX_CAP {
                                    guard.push_back(delivery);
                                }
                                // A full inbox DROPS the delivery
                                // (declared, bounded — never unbounded).
                            }
                        }
                        IncomingPacket::Puback(puback) => {
                            if let Some(outstanding) = pending.remove(&puback.packet_id) {
                                if puback.reason_code == MqttReasonCode::Success {
                                    let _ = outstanding.reply.send(Ok(()));
                                } else {
                                    let _ = outstanding.reply.send(Err(LiveUplinkError::PublishRefused {
                                        reason: format!(
                                            "the broker PUBACKed packet {} with {}",
                                            puback.packet_id, puback.reason_code
                                        ),
                                    }));
                                }
                            }
                            // An unmatched PUBACK is dropped (declared:
                            // no outstanding publish carries it).
                        }
                        IncomingPacket::Pingresp => {}
                        IncomingPacket::Connack(_) => {
                            return Err(LiveUplinkError::Wire(MqttWireError::Malformed {
                                packet: "CONNACK",
                                reason: "an unexpected CONNACK arrived mid-session".to_string(),
                            }))
                        }
                        IncomingPacket::Suback(_) => {
                            return Err(LiveUplinkError::Wire(MqttWireError::Malformed {
                                packet: "SUBACK",
                                reason: "an unexpected SUBACK arrived mid-session".to_string(),
                            }))
                        }
                        IncomingPacket::Disconnect(fields) => {
                            return Err(LiveUplinkError::ConnectionLost {
                                reason: format!(
                                    "the broker disconnected the session: {}",
                                    fields.reason_code
                                ),
                            })
                        }
                    }
                }
            }
        }
    }
}

/// The engine body: connector → connect phase → serving loop. On ANY
/// exit the connected flag falls and outstanding requests are answered
/// with the terminal error (fail-closed: no fabricated receipts).
async fn engine_main(
    connector: Arc<dyn BrokerConnector>,
    config: LiveMqttConfig,
    mut tok_rx: mpsc::Receiver<EngineRequest>,
    inbox: Arc<Mutex<VecDeque<SparkplugCommandDelivery>>>,
    connected: Arc<AtomicBool>,
    ready_tx: std_mpsc::Sender<Result<(), LiveUplinkError>>,
) {
    let stream = match connector.connect().await {
        Ok(stream) => stream,
        Err(error) => {
            let _ = ready_tx.send(Err(error));
            return;
        }
    };
    let mut stream = stream;
    let mut buf: Vec<u8> = Vec::new();
    let next_packet_id_start =
        match timeout(config.connect_timeout, perform_connect(&mut stream, &config, &mut buf)).await {
            Ok(Ok(next)) => next,
            Ok(Err(error)) => {
                let _ = ready_tx.send(Err(error));
                return;
            }
            Err(_) => {
                let _ = ready_tx.send(Err(LiveUplinkError::Timeout {
                    phase: "connect",
                }));
                return;
            }
        };
    connected.store(true, Ordering::Relaxed);
    let _ = ready_tx.send(Ok(()));
    let outcome = serve(
        &mut stream,
        &config,
        &mut tok_rx,
        &inbox,
        next_packet_id_start,
    )
    .await;
    connected.store(false, Ordering::Relaxed);
    // Fail-closed drain: every still-outstanding request gets the
    // terminal error — never a fabricated receipt.
    let terminal: Option<LiveUplinkError> = outcome.err();
    while let Ok(request) = tok_rx.try_recv() {
        match &terminal {
            None => reply_request_error(
                request,
                &LiveUplinkError::NotConnected {
                    reason: "the session closed gracefully".to_string(),
                },
            ),
            Some(error) => reply_request_error(request, error),
        }
    }
}

// --- The sync facade -------------------------------------------------------------

/// The live MQTT 5 + Sparkplug B uplink transport: the real
/// implementation of the UNCHANGED `MqttUplinkTransport` seam. The
/// engine thread owns the socket; the facade is sync by the house rule
/// and blocks at most `ack_timeout` per publish.
pub struct LiveMqttUplinkTransport {
    request_tx: Option<std_mpsc::Sender<EngineRequest>>,
    inbox: Arc<Mutex<VecDeque<SparkplugCommandDelivery>>>,
    connected: Arc<AtomicBool>,
    qos: DeclaredQos,
    ack_timeout: Duration,
    worker: Option<JoinHandle<()>>,
}

impl LiveMqttUplinkTransport {
    /// Connects (blocking, bounded by `connect_timeout`): TCP + TLS per
    /// the declared posture, CONNECT with the NDEATH will, CONNACK,
    /// optional command SUBSCRIBE, NBIRTH + DBIRTHs with PUBACK waits.
    /// A failure is a named refusal — never a half-established session.
    pub fn connect(config: LiveMqttConfig) -> Result<Self, LiveUplinkError> {
        config.validate()?;
        let connector: Arc<dyn BrokerConnector> = Arc::new(TlsBrokerConnector::new(&config)?);
        Self::connect_with_connector(config, connector)
    }

    /// The connector-injection constructor: the production path is
    /// [`LiveMqttUplinkTransport::connect`] (TCP + TLS via
    /// `TlsBrokerConnector`); the socket-free tests inject an in-memory
    /// duplex broker instead. The declared config rules are validated
    /// identically — only the WIRE is substituted, never the posture.
    pub fn connect_with_connector(
        config: LiveMqttConfig,
        connector: Arc<dyn BrokerConnector>,
    ) -> Result<Self, LiveUplinkError> {
        config.validate()?;
        let qos = config.connection.qos;
        let ack_timeout = config.ack_timeout;
        // Captured BEFORE the closure moves the config.
        let connect_timeout = config.connect_timeout;
        let (request_tx, request_rx) = std_mpsc::channel::<EngineRequest>();
        let (tok_tx, tok_rx) = mpsc::channel::<EngineRequest>(64);
        let inbox = Arc::new(Mutex::new(VecDeque::new()));
        let connected = Arc::new(AtomicBool::new(false));
        let (ready_tx, ready_rx) = std_mpsc::channel::<Result<(), LiveUplinkError>>();
        // The engine thread gets its OWN handles (the facade keeps the
        // originals).
        let engine_inbox = Arc::clone(&inbox);
        let engine_connected = Arc::clone(&connected);
        let worker = std::thread::Builder::new()
            .name("flexcore-mqtt-uplink".to_string())
            .spawn(move || {
                let bridge = std::thread::spawn(move || {
                    for request in request_rx {
                        if let Err(request) = tok_tx.blocking_send(request) {
                            // Batch 68: `blocking_send` returns
                            // `SendError<EngineRequest>`; the failed request
                            // itself is its `.0` field.
                            reply_request_error(
                                request.0,
                                &LiveUplinkError::NotConnected {
                                    reason: "the uplink engine has shut down".to_string(),
                                },
                            );
                        }
                    }
                });
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build();
                match runtime {
                    Ok(runtime) => {
                        runtime.block_on(engine_main(
                            connector,
                            config,
                            tok_rx,
                            engine_inbox,
                            engine_connected,
                            ready_tx,
                        ));
                    }
                    Err(error) => {
                        let _ = ready_tx.send(Err(LiveUplinkError::Io {
                            reason: format!("engine runtime: {error}"),
                        }));
                    }
                }
                let _ = bridge.join();
            })
            .map_err(|error| LiveUplinkError::Io {
                reason: format!("uplink engine thread: {error}"),
            })?;
        match ready_rx.recv_timeout(connect_timeout) {
            Ok(Ok(())) => Ok(Self {
                request_tx: Some(request_tx),
                inbox,
                connected,
                qos,
                ack_timeout,
                worker: Some(worker),
            }),
            Ok(Err(error)) => Err(error),
            Err(_) => Err(LiveUplinkError::Timeout {
                phase: "connect",
            }),
        }
    }

    /// The graceful disconnect (bounded by `ack_timeout`): the engine
    /// writes DISCONNECT (NormalDisconnection) and the session ends; the
    /// will does NOT fire on this path.
    pub fn disconnect(mut self) -> Result<(), LiveUplinkError> {
        let outcome = if let Some(sender) = self.request_tx.as_ref() {
            let (reply_tx, reply_rx) = std_mpsc::channel();
            let sent = sender
                .send(EngineRequest::Disconnect { reply: reply_tx })
                .is_ok();
            if sent {
                reply_rx
                    .recv_timeout(self.ack_timeout)
                    .unwrap_or(Err(LiveUplinkError::Timeout {
                        phase: "disconnect",
                    }))
            } else {
                // The engine is already gone: the session is closed.
                Ok(())
            }
        } else {
            Ok(())
        };
        self.request_tx.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        outcome
    }
}

impl Drop for LiveMqttUplinkTransport {
    fn drop(&mut self) {
        // Drop the request sender FIRST (the bridge drains; the engine
        // exits WITHOUT a DISCONNECT — an UNCLEAN shutdown, so the
        // broker publishes the NDEATH will), then join.
        self.request_tx.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl MqttUplinkTransport for LiveMqttUplinkTransport {
    fn publish(
        &mut self,
        topic: &ValidatedTopic,
        payload: &SparkplugPayload,
    ) -> Result<PublishReceipt, MqttTransportError> {
        if !self.connected.load(Ordering::Relaxed) {
            return Err(MqttTransportError::NotConnected {
                reason: "the live MQTT session is not established (never connected, or lost)"
                    .to_string(),
            });
        }
        payload
            .validate()
            .map_err(|error| MqttTransportError::PublishRefused {
                reason: error.to_string(),
            })?;
        let payload_bytes =
            encode_payload(
                payload,
                &PayloadEncodingContext {
                    timestamp_millis: Some(unix_millis_now()),
                    uuid: None,
                },
            )
            .map_err(|error| MqttTransportError::PublishRefused {
                reason: error.to_string(),
            })?;
        let topic_string = topic.topic_string();
        let (reply_tx, reply_rx) = std_mpsc::channel();
        let sender = self.request_tx.as_ref().ok_or_else(|| {
            MqttTransportError::NotConnected {
                reason: "the uplink engine has shut down".to_string(),
            }
        })?;
        sender
            .send(EngineRequest::Publish {
                topic: topic_string.clone(),
                qos: self.qos,
                payload: payload_bytes,
                reply: reply_tx,
            })
            .map_err(|_| MqttTransportError::NotConnected {
                reason: "the uplink engine has shut down".to_string(),
            })?;
        match reply_rx.recv_timeout(self.ack_timeout) {
            Ok(Ok(())) => Ok(PublishReceipt {
                topic: topic_string,
                qos: self.qos,
            }),
            Ok(Err(error)) => Err(map_live_error_to_transport(error)),
            Err(_) => Err(MqttTransportError::PublishRefused {
                reason: format!(
                    "no broker acknowledgement within {:?} (the composition owns redelivery in order)",
                    self.ack_timeout
                ),
            }),
        }
    }
}

impl MqttCommandSource for LiveMqttUplinkTransport {
    fn poll_command(&mut self) -> Option<SparkplugCommandDelivery> {
        self.inbox.lock().ok()?.pop_front()
    }

    fn is_connected_now(&self) -> bool {
        self.connected.load(Ordering::Relaxed)
    }
}

// --- Tests ------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{EdgeNodeId, GroupId, MetricType, MetricValue};

    fn config() -> LiveMqttConfig {
        LiveMqttConfig {
            connection: MqttConnectionConfig {
                endpoint: "mqtts://broker.example:8883".to_string(),
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
            device_births: Vec::new(),
            trust_anchor_pem_path: "/etc/flexcore/mqtt/ca.pem".to_string(),
            client_certificate_pem_path: None,
            client_private_key_pem_path: None,
            plaintext_lab_posture_reason: None,
            keep_alive_secs: 60,
            session_expiry_secs: Some(3600),
            receive_maximum: Some(65535),
            topic_alias_maximum: None,
            connect_timeout: Duration::from_secs(5),
            ack_timeout: Duration::from_secs(5),
            command_topic_filter: None,
        }
    }

    #[test]
    fn plaintext_is_refused_without_a_recorded_lab_posture() {
        let mut config = config();
        config.connection.require_tls = false;
        config.connection.endpoint = "mqtt://broker.example:1883".to_string();
        assert!(matches!(
            config.validate(),
            Err(LiveUplinkError::InvalidConfig { .. })
        ));
        // With the recorded lab reason the config is accepted (the
        // ocpp/20305 fail-closed precedent: an explicit recorded reason,
        // never a silent flag).
        config.plaintext_lab_posture_reason =
            Some("declared lab posture: broker on loopback, no DER attached".to_string());
        config.validate().expect("lab posture accepted");
    }

    #[test]
    fn wss_endpoints_are_a_named_refusal() {
        let mut config = config();
        config.connection.endpoint = "wss://broker.example:443".to_string();
        assert!(matches!(
            config.validate(),
            Err(LiveUplinkError::InvalidConfig { .. })
        ));
    }

    #[test]
    fn wildcard_command_filters_are_refused() {
        let mut config = config();
        config.command_topic_filter = Some("spBv1.0/site-42/NCMD/+".to_string());
        assert!(matches!(
            config.validate(),
            Err(LiveUplinkError::InvalidConfig { .. })
        ));
    }

    #[test]
    fn the_config_type_surface_carries_no_secret_material() {
        // FIM-F05.05 acceptance criterion 4: NO secret material is held
        // by the client. The serialized config carries no password or
        // secret field — and the type has nowhere to put one (the
        // P6.12 structural-absence rule).
        let serialized =
            serde_json::to_string(&config()).expect("serializable");
        let lowered = serialized.to_lowercase();
        assert!(
            !lowered.contains("password") && !lowered.contains("secret_value"),
            "the live config must not carry secret-looking fields: {serialized}"
        );
    }

    #[test]
    fn the_connect_never_sets_the_password_flag_and_carries_the_lwt() {
        // The will is the NDEATH LWT with the session bdSeq (pinned
        // fixture: bdSeq 0 encodes to 28 00 — see sparkplug_wire.rs).
        let config = config();
        let will = will_message_for(&config).expect("will constructible");
        assert_eq!(will.topic, "spBv1.0/site-42/NDEATH/edge-07");
        assert_eq!(will.payload, vec![0x28, 0x00]);
        assert_eq!(will.qos, DeclaredQos::AtLeastOnce);
        assert!(!will.retain);
        let frame = encode_connect(&connect_options_for(&config, &will).expect("options"))
            .expect("encodable");
        // flags byte at index 9 (10=len, 1B=remaining, 00 04 4D 51 54 54
        // = MQTT, 05 = level, flags): username (0x80) | will QoS 1
        // (0x08) | will flag (0x04) | clean start (0x02) — and NEVER the
        // password bit (0x40).
        assert_eq!(frame[9] & 0x40, 0, "the password flag is never set");
        assert_eq!(frame[9], 0x8E);
    }

    #[test]
    fn births_are_planned_with_the_reused_self_check_transitions() {
        // NBIRTH seq 0, then the device DBIRTH seq 1 (the one-chain
        // declared reading); both pass the SAME BirthDeathTracker
        // transitions the platform side runs.
        let mut config = config();
        config.node_birth_metrics = vec![
            SparkplugMetric::new(
                "edge-07.soc.soc_fraction".to_string(),
                None,
                MetricType::Double,
                MetricValue::Double(0.5),
            )
            .expect("valid"),
        ];
        config.device_births = vec![DeviceBirthInventory {
            device_id: DeviceId::new("inverter-1").expect("valid"),
            metrics: vec![
                SparkplugMetric::new(
                    "inverter-1.p.mw".to_string(),
                    None,
                    MetricType::Double,
                    MetricValue::Double(3.5),
                )
                .expect("valid"),
            ],
        }];
        let planned = plan_births(&config).expect("planned");
        assert_eq!(planned.len(), 2);
        assert_eq!(planned[0].topic.topic_string(), "spBv1.0/site-42/NBIRTH/edge-07");
        assert_eq!(planned[0].payload.seq, Some(SparkplugSeq::new(0).expect("valid")));
        assert_eq!(
            planned[1].topic.topic_string(),
            "spBv1.0/site-42/DBIRTH/edge-07/inverter-1"
        );
        assert_eq!(planned[1].payload.seq, Some(SparkplugSeq::new(1).expect("valid")));
        // A BIRTH whose seq chain would break is self-refused by the
        // reused machine (construct a payload out of band to prove the
        // gate fires).
        let broken = SparkplugPayload {
            kind: SparkplugMessageKind::NodeBirth,
            seq: Some(SparkplugSeq::new(200).expect("valid")),
            bd_seq: BdSeq::initial(),
            metrics: Vec::new(),
        };
        let mut tracker = BirthDeathTracker::new();
        tracker.observe(&planned[0].payload.clone());
        assert!(matches!(
            tracker.observe(&broken),
            crate::birth_death::Observation::Refused { .. }
        ));
    }

    #[test]
    fn node_command_filter_uses_the_declared_namespace_and_segment() {
        let plan = SparkplugUplinkPlan::new(
            GroupId::new("site-42").expect("valid"),
            EdgeNodeId::new("edge-07").expect("valid"),
        );
        assert_eq!(node_command_topic_filter(&plan), "spBv1.0/site-42/NCMD/edge-07");
    }

    #[test]
    fn endpoints_parse_with_declared_default_ports() {
        assert_eq!(
            parse_endpoint("mqtts://broker.example:8883").expect("parses"),
            ("broker.example".to_string(), 8883, true)
        );
        assert_eq!(
            parse_endpoint("mqtt://broker.example").expect("parses"),
            ("broker.example".to_string(), 1883, false)
        );
        assert!(parse_endpoint("broker.example:8883").is_err());
        assert!(parse_endpoint("mqtts://broker.example/path").is_err());
    }

    #[test]
    fn packet_ids_cycle_without_zero() {
        assert_eq!(next_packet_id(1), 2);
        assert_eq!(next_packet_id(u16::MAX), 1);
    }
}
