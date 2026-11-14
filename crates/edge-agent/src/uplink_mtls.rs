// P6.07 — Outbound mTLS channel (edge-initiated, NO inbound OT access;
//         report §3.3, §18.1, §18.2, §17.3, §17.4; ledger P6.07).
//
// STRUCTURAL GUARANTEE (the ledger acceptance criterion "no listener socket
// is opened on the edge"): this module contains NO listen/bind/accept call
// anywhere. The only socket operation is a CLIENT `TcpStream::connect` in
// [`RustlsUplinkTransport::dial`] — the edge dials OUT or fails. The
// guarantee is enforced by the structural test
// `crates/edge-agent/tests/no_inbound_listener.rs`, which lexically scans
// every file of this crate's `src/` for listener-type tokens and refuses
// their introduction. There is nothing to "disable": an inbound path does
// not exist in the code shape.
//
// TLS POLICY (the 03_d6_boundary_review.md checklist question C2 — answered
// as RECOMMENDATION/DEFAULT, D6 still OPEN, reviewer sign-off NOT SIGNED):
//
// | Parameter | Pinned value | Label |
// |---|---|---|
// | Minimum protocol version | **TLS 1.3** (`TlsMinimum::Tls13`, the config default); a `tls12-floor` option exists only because the D6 reviewer may require the 1.2 floor for a site device — it is opt-in, never the default | DEFAULT (preferred 1.3) |
// | Cipher suites | The rustls crypto-provider DEFAULTS (ring provider), recorded not customized: TLS 1.3 — AES-128-GCM-SHA256, AES-256-GCM-SHA384, CHACHA20-POLY1305-SHA256; TLS 1.2 (only if the floor is opted in) — ECDHE-ECDSA/RSA with AES-GCM/CHACHA20 suites as the provider offers them. NO suite list is invented here and none is hand-restricted yet: a per-site allowlist belongs to the signed D6 record | recorded (rustls defaults) |
// | Plaintext rejection | `MtlsUplinkConfig::validate` REFUSES `ws://` and `http://` endpoints and any scheme-less endpoint — only `wss://`/`https://` compose (ledger P6.07 step 3) | DEFAULT |
// | Server trust | Root CA pinned FROM THE CREDENTIAL RECORD (P6.06 `root_ca_pem`) — never from the ambient system store | DEFAULT |
// | Client identity | Client certificate from the P6.06 identity record; key material from the deployment's key source (§18.3), loaded per session, never logged | DEFAULT |
//
// Source of truth note: P6.03's record explicitly leaves the TLS minimum
// "site-specific; must come from the reviewer, not from code defaults"
// — so the values above are the RECOMMENDED baseline P6.07 pins while D6
// is OPEN, and `tls12-floor`/allowlist changes are configuration, not code
// changes.
//
// DELIVERY HONESTY (at-least-once): `forward` returns `Ok` ONLY after the
// transport's acknowledgement is read back (`UplinkTransport::
// send_and_await_ack`); an un-acked send is `Err` and the batch stays in
// the P6.09 buffer, which owns redelivery. The channel itself buffers
// NOTHING — no in-flight cache, no retry queue. "Ack" here is the
// control-plane's receipt of the batch, not physical delivery of anything
// downstream (acknowledgement is not delivery, §14.1 — the same rule as
// the command lifecycle).
//
// RUN STATUS (AGENTS.md rule 7): the rustls-touching paths
// ([`build_client_config`], [`RustlsUplinkTransport`]) are written, NOT
// built, NOT run — no TLS handshake has ever been executed by this wave
// (the ENTRY_NOTE loop bans). Tests exercise config validation, the
// fail-closed composition, and the reconnect state machine against a FAKE
// transport only.

use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use flexcore_ports::clock::Clock;

use crate::config::EdgeConfig;
use crate::enrollment::{DeviceCredentialRecord, FilesystemCredentialStore, PemBody};
use crate::ports::{EdgeError, EdgeTelemetrySample, UplinkChannel};

// --- TLS minimum -------------------------------------------------------------

/// The pinned minimum protocol version (the D6-review question C2; the
/// policy table in the module header).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TlsMinimum {
    /// TLS 1.3 only. THE DEFAULT (report §17.3 baseline posture; modern
    /// rustls default behaviour).
    Tls13,
    /// TLS 1.2 as the floor (1.2 and 1.3 both offered). Opt-in for sites
    /// whose D6 record requires it — never the default.
    Tls12Floor,
}

impl TlsMinimum {
    pub const DEFAULT: Self = Self::Tls13;

    pub fn as_str(self) -> &'static str {
        match self {
            TlsMinimum::Tls13 => "tls13",
            TlsMinimum::Tls12Floor => "tls12-floor",
        }
    }

    pub fn parse(raw: &str) -> Result<Self, EdgeError> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "tls13" => Ok(TlsMinimum::Tls13),
            "tls12-floor" => Ok(TlsMinimum::Tls12Floor),
            other => Err(EdgeError::UplinkForwardFailed {
                reason: format!(
                    "unknown TLS minimum '{other}'; expected 'tls13' (default) or 'tls12-floor' \
                     (opt-in per a site D6 record)"
                ),
            }),
        }
    }
}

impl fmt::Display for TlsMinimum {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// --- Channel configuration ----------------------------------------------------

/// The outbound-channel configuration, validated fail-closed. `endpoint`
/// comes from the edge configuration's control-plane endpoint;
/// `server_name` is the TLS SNI + certificate-verification name (pinned —
/// it is NOT taken from an untrusted response).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MtlsUplinkConfig {
    /// The control-plane endpoint (`wss://host:port` or
    /// `https://host:port`). Plaintext schemes are REFUSED at validation.
    pub endpoint: String,
    /// The TLS server name the edge pins and verifies (config-sourced; the
    /// certificate the edge is shown must be issued FOR this name).
    pub server_name: String,
    /// The pinned minimum version (module-header policy table; DEFAULT
    /// TLS 1.3).
    pub tls_minimum: TlsMinimum,
    /// Outbound connect timeout (supervisory traffic; never a hard-real-time
    /// path).
    pub connect_timeout: Duration,
    /// Reconnect backoff initial wait (exponential ×2 to `backoff_max`).
    pub backoff_initial: Duration,
    /// Reconnect backoff cap.
    pub backoff_max: Duration,
    /// Idle keepalive window: a connected session unused for longer than
    /// this is RE-ESTABLISHED (a fresh handshake) instead of being trusted
    /// after an idle gap — TCP half-open connections must not be silently
    /// reused (§18.2).
    pub keepalive_interval: Duration,
}

impl MtlsUplinkConfig {
    /// Derives the channel configuration from the edge configuration
    /// (`EdgeConfig` carries the endpoint + the P6.07 additive variables).
    /// Fail-closed: a missing server name is a named error — the channel
    /// does not compose without an explicitly pinned server name.
    pub fn from_edge_config(config: &EdgeConfig) -> Result<Self, EdgeError> {
        let server_name = config.uplink_server_name.clone().ok_or_else(|| {
            EdgeError::UplinkForwardFailed {
                reason: format!(
                    "the mTLS channel requires an explicitly pinned TLS server name \
                     (set {}); refusing to derive trust from an unpinned name",
                    crate::config::ENV_UPLINK_SERVER_NAME
                ),
            }
        })?;
        Ok(Self {
            endpoint: config.control_plane_endpoint.clone(),
            server_name,
            tls_minimum: config.uplink_tls_minimum,
            connect_timeout: config.uplink_connect_timeout,
            backoff_initial: config.uplink_backoff_initial,
            backoff_max: config.uplink_backoff_max,
            keepalive_interval: config.uplink_keepalive_interval,
        })
    }

    /// Validation (fail-closed; every rule is a named error). The plaintext
    /// rejection implements ledger P6.07 step 3: "reject plaintext (`ws://`)
    /// and unauthenticated connections".
    pub fn validate(&self) -> Result<(), EdgeError> {
        let scheme = self
            .endpoint
            .split("://")
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase();
        if !matches!(scheme.as_str(), "wss" | "https") {
            return Err(EdgeError::UplinkForwardFailed {
                reason: format!(
                    "the uplink endpoint must be an explicit TLS scheme (wss:// or https://); \
                     got '{}'. Plaintext (ws://, http://) and scheme-less endpoints are REFUSED \
                     (ledger P6.07 step 3)",
                    if self.endpoint.contains("://") {
                        scheme.as_str()
                    } else {
                        "<no scheme>"
                    }
                ),
            });
        }
        if self.server_name.trim().is_empty() {
            return Err(EdgeError::UplinkForwardFailed {
                reason: "the pinned TLS server name is mandatory".to_string(),
            });
        }
        if self.connect_timeout.is_zero() {
            return Err(EdgeError::UplinkForwardFailed {
                reason: "the connect timeout must be positive".to_string(),
            });
        }
        if self.backoff_initial.is_zero() {
            return Err(EdgeError::UplinkForwardFailed {
                reason: "the reconnect backoff initial wait must be positive".to_string(),
            });
        }
        if self.backoff_max < self.backoff_initial {
            return Err(EdgeError::UplinkForwardFailed {
                reason: "the reconnect backoff cap must be >= the initial wait".to_string(),
            });
        }
        if self.keepalive_interval.is_zero() {
            return Err(EdgeError::UplinkForwardFailed {
                reason: "the keepalive interval must be positive".to_string(),
            });
        }
        Ok(())
    }

    /// Splits the validated endpoint into `(host, port)` for the dial.
    /// Defaults: `wss://` → 443, `https://` → 443. An explicit port wins.
    pub fn dial_target(&self) -> Result<(String, u16), EdgeError> {
        self.validate()?;
        parse_endpoint(&self.endpoint)
    }
}

/// The endpoint parser behind [`MtlsUplinkConfig::dial_target`] (shared
/// with the production transport's dial, which carries the endpoint string
/// it was composed with).
fn parse_endpoint(endpoint: &str) -> Result<(String, u16), EdgeError> {
    let without_scheme = endpoint
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or_default();
    // Strip any path component; the channel dials the authority only.
    let authority = without_scheme
        .split(['/', '?'])
        .next()
        .unwrap_or_default()
        .trim()
        .to_string();
    let (host, port) = match authority.rsplit_once(':') {
        Some((h, p)) => {
            let port = p.parse::<u16>().map_err(|_| EdgeError::UplinkForwardFailed {
                reason: format!("endpoint port '{p}' is not a valid TCP port"),
            })?;
            (h.trim().to_string(), port)
        }
        None => (authority.clone(), 443u16),
    };
    if host.is_empty() {
        return Err(EdgeError::UplinkForwardFailed {
            reason: "the endpoint has no host".to_string(),
        });
    }
    Ok((host, port))
}

// --- Key material source (§18.3) ----------------------------------------------

/// The client PRIVATE KEY source. The key material lives OUTSIDE the
/// credential record (hardware-backed where the platform offers it, §18.3)
/// and is resolved per session through this seam. Implementations must
/// never log the material.
pub trait KeyMaterialSource: Send {
    /// Returns the PEM-encoded private key for the credential's
    /// `private_key_reference`.
    fn load_key(&self, reference: &PathBuf) -> Result<PemBody, EdgeError>;
}

/// Production source: reads the key file the credential record references.
/// The material is returned once per session build and never stored in the
/// channel state, never logged, and never placed in an audit event.
#[derive(Debug, Clone)]
pub struct FileKeyMaterialSource;

impl KeyMaterialSource for FileKeyMaterialSource {
    fn load_key(&self, reference: &PathBuf) -> Result<PemBody, EdgeError> {
        let raw = std::fs::read_to_string(reference).map_err(|error| {
            // The error text names the failure, never the contents.
            EdgeError::CredentialStoreFailed {
                reason: format!("the device private key is unreadable: {error}"),
            }
        })?;
        let pem = PemBody::new(raw);
        if pem.is_empty() {
            return Err(EdgeError::CredentialStoreFailed {
                reason: "the device private key file is empty".to_string(),
            });
        }
        Ok(pem)
    }
}

/// The complete TLS material the channel composes from (fail-closed: the
/// constructor refuses an incomplete identity). Built ONCE per channel
/// composition; the key material is consumed into the TLS client config
/// and is NOT retained by the channel afterwards.
#[derive(Debug)]
pub struct ChannelMaterial {
    pub root_ca_pem: PemBody,
    pub client_cert_pem: PemBody,
    pub client_key_pem: PemBody,
}

/// Assembles the channel material from the P6.06 identity. Fail-closed on
/// a missing root CA pin, a missing client certificate, or a missing
/// key-material source result (ledger P6.07: "a connection without a valid
/// client certificate is rejected").
pub fn assemble_material(
    credentials: &dyn FilesystemCredentialStoreLoad,
    key_source: &dyn KeyMaterialSource,
) -> Result<ChannelMaterial, EdgeError> {
    let record: DeviceCredentialRecord = credentials
        .load_identity()?
        .ok_or_else(|| EdgeError::CredentialStoreFailed {
            reason: "fail closed: no device identity is stored (the edge has not enrolled); \
                     the mTLS channel does not compose without the P6.06 identity"
                .to_string(),
        })?;
    record.validate()?;
    let root_ca_pem =
        record
            .root_ca_pem
            .clone()
            .ok_or_else(|| EdgeError::CredentialStoreFailed {
                reason: "fail closed: the stored identity carries no pinned platform root CA; \
                         the edge refuses to trust an unpinned central endpoint"
                    .to_string(),
            })?;
    let client_cert_pem = record
        .certificate_pem
        .clone()
        .ok_or_else(|| EdgeError::CredentialStoreFailed {
            reason: "fail closed: the stored identity carries no device certificate \
                     (certificate issuance is a deployment requirement; the edge does not \
                     dial out without a client certificate)"
                .to_string(),
        })?;
    let reference = record
        .private_key_reference
        .clone()
        .ok_or_else(|| EdgeError::CredentialStoreFailed {
            reason: "fail closed: the stored identity carries no private-key reference (§18.3)"
                .to_string(),
        })?;
    let client_key_pem = key_source.load_key(&reference)?;
    Ok(ChannelMaterial {
        root_ca_pem,
        client_cert_pem,
        client_key_pem,
    })
}

/// The minimal store-facing shape the material assembly needs (so tests can
/// supply a scripted identity without a filesystem). The production
/// [`FilesystemCredentialStore`] implements it.
pub trait FilesystemCredentialStoreLoad: Send {
    fn load_identity(&self) -> Result<Option<DeviceCredentialRecord>, EdgeError>;
}

impl FilesystemCredentialStoreLoad for FilesystemCredentialStore {
    fn load_identity(&self) -> Result<Option<DeviceCredentialRecord>, EdgeError> {
        crate::enrollment::CredentialStore::load(self)
    }
}

// --- The rustls client config (UNRUN — no TLS has ever been executed) --------

/// Builds the rustls client configuration from the channel material, with
/// the pinned minimum protocol version (module-header policy table) and the
/// provider's DEFAULT cipher suites (recorded, not customized).
///
/// `pub` so the FAIL-CLOSED refusals are testable (the tests exercise only
/// the refusal paths — no handshake is ever executed here; the builder
/// performs no network I/O).
///
/// UNPINNED-REVERIFY: the direct `rustls` + `rustls-pemfile` dependencies
/// carry caret ranges (see `crates/edge-agent/Cargo.toml` and the D4 note
/// in `06_07_identity_channel.md`); the first human lockfile resolves and
/// re-verifies them.
pub fn build_client_config(
    material: &ChannelMaterial,
    tls_minimum: TlsMinimum,
) -> Result<rustls::ClientConfig, EdgeError> {
    use rustls::version::{TLS12, TLS13};

    let mut roots = rustls::RootCertStore::empty();
    let mut ca_reader = std::io::BufReader::new(material.root_ca_pem.expose().as_bytes());
    for cert in rustls_pemfile::certs(&mut ca_reader) {
        let cert = cert.map_err(|error| EdgeError::UplinkForwardFailed {
            reason: format!("the pinned platform root CA is not parsable PEM: {error}"),
        })?;
        roots.add(cert).map_err(|error| EdgeError::UplinkForwardFailed {
            reason: format!("the pinned platform root CA was refused: {error}"),
        })?;
    }
    // An empty root store would silently trust NOTHING (every handshake
    // fails); the explicit refusal names the real cause instead.
    if roots.is_empty() {
        return Err(EdgeError::UplinkForwardFailed {
            reason: "fail closed: the pinned platform root CA contains no parsable \
                     certificate; the edge refuses to dial with an empty trust store"
                .to_string(),
        });
    }

    let mut cert_reader =
        std::io::BufReader::new(material.client_cert_pem.expose().as_bytes());
    let client_certs: Vec<rustls::pki_types::CertificateDer<'static>> =
        rustls_pemfile::certs(&mut cert_reader)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| EdgeError::UplinkForwardFailed {
                reason: format!("the device certificate is not parsable PEM: {error}"),
            })?;
    if client_certs.is_empty() {
        return Err(EdgeError::UplinkForwardFailed {
            reason: "fail closed: no client certificate could be parsed from the stored \
                     identity".to_string(),
        });
    }
    let mut key_reader = std::io::BufReader::new(material.client_key_pem.expose().as_bytes());
    let client_key = rustls_pemfile::private_key(&mut key_reader)
        .map_err(|error| EdgeError::UplinkForwardFailed {
            reason: format!("the device private key is not parsable PEM: {error}"),
        })?
        .ok_or_else(|| EdgeError::UplinkForwardFailed {
            reason: "fail closed: no private key could be parsed from the key source".to_string(),
        })?;

    let builder = match tls_minimum {
        TlsMinimum::Tls13 => rustls::ClientConfig::builder_with_protocol_versions(&[&TLS13]),
        TlsMinimum::Tls12Floor => {
            rustls::ClientConfig::builder_with_protocol_versions(&[&TLS12, &TLS13])
        }
    };
    builder
        .with_root_certificates(roots)
        .with_client_auth_cert(client_certs, client_key)
        .map_err(|error| EdgeError::UplinkForwardFailed {
            reason: format!("the mTLS client configuration was refused: {error}"),
        })
}

// --- The payload encoder seam -------------------------------------------------

/// Encodes a forwarded batch into the wire payload. THE TELEMETRY WIRE
/// FORMAT IS OWNED BY P6.08/P6.09 (read path + store-and-forward); this
/// wave ships no encoder — the shipped default refuses (fail-closed by
/// absence, the P6.05 rule), so the channel can be fully exercised WITHOUT
/// fabricating a payload shape that a later phase would have to own.
pub trait BatchPayloadEncoder: Send {
    fn encode(&self, samples: &[EdgeTelemetrySample]) -> Result<Vec<u8>, EdgeError>;
}

/// The shipped encoder: refuses every batch with the honest reason. The
/// composition wires the P6.08/P6.09 encoder when the read path lands.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoBatchPayloadEncoder;

impl BatchPayloadEncoder for NoBatchPayloadEncoder {
    fn encode(&self, _samples: &[EdgeTelemetrySample]) -> Result<Vec<u8>, EdgeError> {
        Err(EdgeError::UplinkForwardFailed {
            reason: "no telemetry payload encoder is composed; the wire format is owned by \
                     the P6.08/P6.09 read path — the channel refuses to invent one"
                .to_string(),
        })
    }
}

// --- The transport seam ---------------------------------------------------------

/// The ONE outbound transport seam. Implementations DIAL OUT or fail;
/// there is no listen/bind/accept anywhere on this surface (the structural
/// test scans this file).
pub trait UplinkTransport: Send {
    /// Opens ONE outbound mTLS connection (client certificate + pinned
    /// server verification). Idempotent-ish: dialing an established
    /// transport re-dials (fresh handshake), never silently reuses.
    fn dial(&mut self) -> Result<(), EdgeError>;
    /// Sends one encoded payload and returns `Ok` ONLY after reading the
    /// control-plane's acknowledgement (at-least-once honesty; the module
    /// header). An un-acked send is `Err` and the payload is treated as
    /// NOT delivered.
    fn send_and_await_ack(&mut self, payload: &[u8]) -> Result<(), EdgeError>;
    /// Whether the transport currently holds an established session.
    fn is_established(&self) -> bool;
    /// Closes the session (clean teardown before a re-dial).
    fn close(&mut self);
}

/// Builds a transport from the assembled channel material. The seam keeps
/// the channel's reconnect state machine rustls-agnostic and testable with
/// a fake: the production factory ([`RustlsUplinkTransportFactory`]) is the
/// one that builds the REAL rustls client config inside `build` — a fake
/// factory is still handed the complete [`ChannelMaterial`] and the pinned
/// [`TlsMinimum`], so composition cannot skip the material assembly
/// (fail-closed credentials remain the channel's own responsibility).
pub trait UplinkTransportFactory: Send {
    fn build(
        &mut self,
        material: &ChannelMaterial,
        tls_minimum: TlsMinimum,
    ) -> Result<Box<dyn UplinkTransport>, EdgeError>;
}

// --- Channel status (the health vocabulary, P6.17 consumer) ---------------------

/// A health snapshot of the channel. The ledger P6.07 step 4 distinction:
/// THIS is "channel down" (the uplink path); "site down" is the EMS read
/// path (`EmsSource` poll failures in the runner) — the two are separate
/// signals and neither implies the other.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChannelStatus {
    /// A session is established and fresh (within the keepalive window).
    pub connected: bool,
    /// Consecutive failed dial/send attempts since the last success (drives
    /// the exponential backoff; resets to 0 on success).
    pub consecutive_failures: u32,
    /// The backoff wait the NEXT reconnect attempt must observe
    /// (milliseconds).
    pub current_backoff_millis: i64,
}

// --- The channel ----------------------------------------------------------------

/// One backoff-wait computation: exponential ×2 from `initial`, capped at
/// `max`. Failure #1 waits `initial`, #2 waits 2×initial, … Deterministic
/// (no jitter) — recorded decision: the supervisory uplink tolerates
/// synchronized retries in the 0.x fleet sizes, and determinism keeps the
/// reconnect state machine testable; jitter is an acceptable later
/// refinement, not a correctness requirement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackoffSchedule {
    pub initial_millis: i64,
    pub max_millis: i64,
}

impl BackoffSchedule {
    pub fn new(initial: Duration, max: Duration) -> Result<Self, EdgeError> {
        if initial.is_zero() || max < initial {
            return Err(EdgeError::UplinkForwardFailed {
                reason: "backoff must be positive and the cap >= the initial wait".to_string(),
            });
        }
        Ok(Self {
            initial_millis: initial.as_millis() as i64,
            max_millis: max.as_millis() as i64,
        })
    }

    /// The wait before the NEXT attempt, given the count of consecutive
    /// failures so far (>= 1).
    pub fn wait_for(&self, consecutive_failures: u32) -> i64 {
        if consecutive_failures == 0 {
            return self.initial_millis;
        }
        let doubling = 2u64
            .saturating_pow(consecutive_failures.saturating_sub(1))
            .saturating_mul(self.initial_millis.max(0) as u64);
        (doubling as i64).clamp(self.initial_millis, self.max_millis)
    }
}

/// The channel state (private; exposed through [`MtlsUplinkChannel::status`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChannelState {
    Disconnected {
        last_attempt_millis: i64,
        consecutive_failures: u32,
    },
    Connected {
        last_activity_millis: i64,
    },
}

/// The outbound mTLS [`UplinkChannel`]: edge-initiated, mutually
/// authenticated, fail-closed on composition, offline-first in operation
/// (the runner buffers when this channel is down; recovery drains in
/// order).
pub struct MtlsUplinkChannel {
    config: MtlsUplinkConfig,
    transport: Box<dyn UplinkTransport>,
    encoder: Box<dyn BatchPayloadEncoder>,
    clock: Arc<dyn Clock + Send + Sync>,
    backoff: BackoffSchedule,
    state: ChannelState,
}

impl MtlsUplinkChannel {
    /// Composes the channel: validates the config, loads the P6.06
    /// identity, assembles the TLS material (fail-closed on ANY missing
    /// piece), and hands the material + the pinned minimum version to the
    /// transport factory (the production factory builds the rustls client
    /// config there). The channel starts
    /// DISCONNECTED — nothing dials until the first `forward`/`ensure_connected`
    /// (offline-first; the composition decides the dial cadence).
    pub fn compose(
        config: MtlsUplinkConfig,
        credentials: &dyn FilesystemCredentialStoreLoad,
        key_source: &dyn KeyMaterialSource,
        encoder: Box<dyn BatchPayloadEncoder>,
        transport_factory: &mut dyn UplinkTransportFactory,
        clock: Arc<dyn Clock + Send + Sync>,
    ) -> Result<Self, EdgeError> {
        config.validate()?;
        let material = assemble_material(credentials, key_source)?;
        let transport = transport_factory.build(&material, config.tls_minimum)?;
        Ok(Self {
            backoff: BackoffSchedule::new(config.backoff_initial, config.backoff_max)?,
            config,
            transport,
            encoder,
            clock,
            state: ChannelState::Disconnected {
                last_attempt_millis: i64::MIN / 2,
                consecutive_failures: 0,
            },
        })
    }

    /// The health snapshot (`ChannelStatus` — the "channel down" signal;
    /// distinct from "site down" on the EMS read path).
    pub fn status(&self) -> ChannelStatus {
        let (connected, failures) = match self.state {
            ChannelState::Connected { .. } => (self.transport.is_established(), 0),
            ChannelState::Disconnected {
                consecutive_failures,
                ..
            } => (false, consecutive_failures),
        };
        ChannelStatus {
            connected,
            consecutive_failures: failures,
            current_backoff_millis: self.backoff.wait_for(failures.max(1)),
        }
    }

    /// The pinned configuration (diagnostics name the endpoint SCHEME and
    /// server name only; the endpoint may embed credentials — never
    /// rendered, the config.rs redaction rule).
    pub fn pinned_policy(&self) -> (&str, &str, TlsMinimum) {
        // Returns (scheme, server_name, tls_minimum) — the endpoint string
        // itself is deliberately NOT exposed here.
        let scheme = self
            .config
            .endpoint
            .split("://")
            .next()
            .unwrap_or_default();
        (scheme, &self.config.server_name, self.config.tls_minimum)
    }

    /// Ensures a usable session BEFORE a send: a fresh connected session is
    /// reused; a connected session past the keepalive window is closed and
    /// re-dialed (fresh handshake — no silent half-open reuse); a
    /// disconnected channel dials only after its backoff window has
    /// elapsed (a too-early attempt is a named error, not a silent spin).
    pub fn ensure_connected(&mut self) -> Result<(), EdgeError> {
        let now = self.clock.now_ingest_time().unix_millis();
        match self.state {
            ChannelState::Connected {
                last_activity_millis,
            } => {
                let idle = now.saturating_sub(last_activity_millis);
                if idle < self.config.keepalive_interval.as_millis() as i64 {
                    return Ok(());
                }
                // Keepalive elapsed: re-establish (fresh handshake).
                self.transport.close();
                self.dial(now)
            }
            ChannelState::Disconnected {
                last_attempt_millis,
                consecutive_failures,
            } => {
                if consecutive_failures > 0 {
                    let wait = self.backoff.wait_for(consecutive_failures);
                    let since = now.saturating_sub(last_attempt_millis);
                    if since < wait {
                        return Err(EdgeError::UplinkForwardFailed {
                            reason: format!(
                                "reconnect backoff window still open ({} ms of {} ms)",
                                since, wait
                            ),
                        });
                    }
                }
                self.dial(now)
            }
        }
    }

    fn dial(&mut self, now: i64) -> Result<(), EdgeError> {
        match self.transport.dial() {
            Ok(()) => {
                self.state = ChannelState::Connected {
                    last_activity_millis: now,
                };
                Ok(())
            }
            Err(error) => {
                let failures = match self.state {
                    ChannelState::Disconnected {
                        consecutive_failures,
                        ..
                    } => consecutive_failures + 1,
                    _ => 1,
                };
                self.state = ChannelState::Disconnected {
                    last_attempt_millis: now,
                    consecutive_failures: failures,
                };
                Err(error)
            }
        }
    }
}

impl UplinkChannel for MtlsUplinkChannel {
    fn is_connected(&self) -> bool {
        self.status().connected
    }

    /// Forwards one batch OUT (at-least-once, ack-confirmed). The empty
    /// batch is `Ok` (nothing to deliver — the runner never sends one).
    /// Delivery honesty: `Ok` is returned ONLY after the transport read
    /// the acknowledgement; the P6.09 buffer owns redelivery of any `Err`.
    fn forward(&mut self, samples: &[EdgeTelemetrySample]) -> Result<(), EdgeError> {
        if samples.is_empty() {
            return Ok(());
        }
        let payload = self.encoder.encode(samples)?;
        self.ensure_connected()?;
        let now = self.clock.now_ingest_time().unix_millis();
        match self.transport.send_and_await_ack(&payload) {
            Ok(()) => {
                self.state = ChannelState::Connected {
                    last_activity_millis: now,
                };
                Ok(())
            }
            Err(error) => {
                self.transport.close();
                let failures = match self.state {
                    ChannelState::Disconnected {
                        consecutive_failures,
                        ..
                    } => consecutive_failures + 1,
                    _ => 1,
                };
                self.state = ChannelState::Disconnected {
                    last_attempt_millis: now,
                    consecutive_failures: failures,
                };
                Err(error)
            }
        }
    }
}

// --- The production transport (UNRUN — never executed by this wave) -------------

/// The production transport: one outbound TCP connection per session, TLS
/// via rustls `ClientConnection`, synchronous IO (the `UplinkChannel` port
/// is sync by house rule — the adapter owns the blocking shape). The
/// acknowledgement contract: after the payload bytes, the control plane
/// answers `ACK\n`; anything else (EOF, timeout, wrong bytes) is
/// "NOT delivered".
pub struct RustlsUplinkTransport {
    endpoint: String,
    server_name: String,
    tls: Arc<rustls::ClientConfig>,
    connect_timeout: Duration,
    session: Option<rustls::StreamOwned<rustls::ClientConnection, std::net::TcpStream>>,
}

impl RustlsUplinkTransport {
    pub fn new(endpoint: String, server_name: String, connect_timeout: Duration, tls: rustls::ClientConfig) -> Self {
        Self {
            endpoint,
            server_name,
            tls: Arc::new(tls),
            connect_timeout,
            session: None,
        }
    }
}

/// The production transport factory: holds the pinned endpoint/server
/// name/timeout from composition and hands each built transport the REAL
/// rustls client config the channel composed (a fake factory must be
/// handed it too — composition cannot skip the TLS build).
#[derive(Debug, Clone)]
pub struct RustlsUplinkTransportFactory {
    endpoint: String,
    server_name: String,
    connect_timeout: Duration,
}

impl RustlsUplinkTransportFactory {
    pub fn new(config: &MtlsUplinkConfig) -> Self {
        Self {
            endpoint: config.endpoint.clone(),
            server_name: config.server_name.clone(),
            connect_timeout: config.connect_timeout,
        }
    }
}

impl UplinkTransportFactory for RustlsUplinkTransportFactory {
    fn build(
        &mut self,
        material: &ChannelMaterial,
        tls_minimum: TlsMinimum,
    ) -> Result<Box<dyn UplinkTransport>, EdgeError> {
        // THE production TLS build: pinned minimum version + provider
        // default suites + client identity + pinned root CA (all UNRUN).
        let tls = build_client_config(material, tls_minimum)?;
        Ok(Box::new(RustlsUplinkTransport::new(
            self.endpoint.clone(),
            self.server_name.clone(),
            self.connect_timeout,
            tls,
        )))
    }
}

impl UplinkTransport for RustlsUplinkTransport {
    fn dial(&mut self) -> Result<(), EdgeError> {
        use std::net::ToSocketAddrs as _;
        // Always a FRESH connection: close any previous session first.
        self.close();
        let (host, port) = parse_endpoint(&self.endpoint)?;
        let server_name = rustls::pki_types::ServerName::try_from(self.server_name.clone())
            .map_err(|error| EdgeError::UplinkForwardFailed {
                reason: format!("the pinned TLS server name is not a valid ServerName: {error}"),
            })?;
        // OUTBOUND ONLY: resolve the pinned endpoint and connect to it.
        let addrs: Vec<std::net::SocketAddr> = (host.as_str(), port)
            .to_socket_addrs()
            .map_err(|error| EdgeError::UplinkForwardFailed {
                reason: format!("the pinned endpoint could not be resolved: {error}"),
            })?
            .collect();
        let mut last_error: Option<std::io::Error> = None;
        let mut stream = None;
        for addr in addrs {
            match std::net::TcpStream::connect_timeout(&addr, self.connect_timeout) {
                Ok(s) => {
                    stream = Some(s);
                    break;
                }
                Err(error) => last_error = Some(error),
            }
        }
        let stream = stream.ok_or_else(|| EdgeError::UplinkForwardFailed {
            reason: format!(
                "the outbound connection to the pinned endpoint failed: {}",
                last_error
                    .map(|e| e.to_string())
                    .unwrap_or_else(|| "no address resolved".to_string())
            ),
        })?;
        let connection = rustls::ClientConnection::new(Arc::clone(&self.tls), server_name)
            .map_err(|error| EdgeError::UplinkForwardFailed {
                reason: format!("the TLS session could not be established: {error}"),
            })?;
        self.session = Some(rustls::StreamOwned::new(connection, stream));
        Ok(())
    }

    fn send_and_await_ack(&mut self, payload: &[u8]) -> Result<(), EdgeError> {
        use std::io::Read as _;
        use std::io::Write as _;
        let session = self
            .session
            .as_mut()
            .ok_or_else(|| EdgeError::UplinkForwardFailed {
                reason: "no established session; dial first".to_string(),
            })?;
        // One frame: payload + terminator. The control plane answers the
        // 4-byte receipt `ACK\n` (module header: ack = receipt of the
        // batch, not downstream delivery).
        session
            .write_all(payload)
            .and_then(|_| session.write_all(b"\n#FLEXCORE-BATCH-END\n"))
            .map_err(|error| EdgeError::UplinkForwardFailed {
                reason: format!("the outbound send failed: {error}"),
            })?;
        session.flush().map_err(|error| EdgeError::UplinkForwardFailed {
            reason: format!("the outbound send could not be flushed: {error}"),
        })?;
        // Read until the ack terminator, bounded (a hostile/garbage peer
        // must not grow the read unboundedly).
        let mut ack = Vec::new();
        let cap = 64usize;
        let mut byte = [0u8; 1];
        loop {
            let read = session.read(&mut byte).map_err(|error| {
                EdgeError::UplinkForwardFailed {
                    reason: format!("the acknowledgement read failed: {error}"),
                }
            })?;
            if read == 0 {
                return Err(EdgeError::UplinkForwardFailed {
                    reason: "the connection closed before the acknowledgement — the batch is \
                             NOT confirmed delivered"
                        .to_string(),
                });
            }
            ack.push(byte[0]);
            if ack.len() > cap {
                return Err(EdgeError::UplinkForwardFailed {
                    reason: "the acknowledgement exceeded the bounded read; treating the \
                             batch as NOT confirmed"
                        .to_string(),
                });
            }
            if ack.ends_with(b"ACK\n") {
                return Ok(());
            }
            if ack.ends_with(b"NACK\n") {
                return Err(EdgeError::UplinkForwardFailed {
                    reason: "the control plane refused the batch (NACK); the buffer will \
                             re-forward it in order"
                        .to_string(),
                });
            }
        }
    }

    fn is_established(&self) -> bool {
        self.session.is_some()
    }

    fn close(&mut self) {
        self.session = None; // dropping the StreamOwned closes the TCP session
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tls_minimum_defaults_and_parses() {
        assert_eq!(TlsMinimum::DEFAULT, TlsMinimum::Tls13);
        assert_eq!(
            TlsMinimum::parse("tls13").expect("parses"),
            TlsMinimum::Tls13
        );
        assert_eq!(
            TlsMinimum::parse(" TLS12-FLOOR ").expect("parses"),
            TlsMinimum::Tls12Floor
        );
        assert!(TlsMinimum::parse("ssl3").is_err());
    }

    #[test]
    fn config_validation_rejects_plaintext_and_scheme_less_endpoints() {
        let base = MtlsUplinkConfig {
            endpoint: "wss://central.example:8443".to_string(),
            server_name: "central.example".to_string(),
            tls_minimum: TlsMinimum::DEFAULT,
            connect_timeout: Duration::from_millis(10_000),
            backoff_initial: Duration::from_millis(1_000),
            backoff_max: Duration::from_millis(60_000),
            keepalive_interval: Duration::from_millis(30_000),
        };
        base.validate().expect("valid");
        for bad in [
            "ws://central.example:8443",   // plaintext websocket — refused
            "http://central.example",      // plaintext http — refused
            "central.example:8443",        // scheme-less — refused
        ] {
            let mut config = base.clone();
            config.endpoint = bad.to_string();
            assert!(
                config.validate().is_err(),
                "endpoint '{bad}' must be refused"
            );
        }
        let mut config = base.clone();
        config.server_name = "  ".to_string();
        assert!(config.validate().is_err());
        let mut config = base.clone();
        config.backoff_max = Duration::from_millis(500);
        assert!(config.validate().is_err());
    }

    #[test]
    fn dial_target_parses_host_port_with_defaults() {
        let mut config = MtlsUplinkConfig {
            endpoint: "wss://central.example:8443/uplink".to_string(),
            server_name: "central.example".to_string(),
            tls_minimum: TlsMinimum::DEFAULT,
            connect_timeout: Duration::from_millis(10_000),
            backoff_initial: Duration::from_millis(1_000),
            backoff_max: Duration::from_millis(60_000),
            keepalive_interval: Duration::from_millis(30_000),
        };
        assert_eq!(
            config.dial_target().expect("parses"),
            ("central.example".to_string(), 8443)
        );
        config.endpoint = "https://central.example".to_string();
        assert_eq!(
            config.dial_target().expect("parses"),
            ("central.example".to_string(), 443)
        );
        config.endpoint = "ws://central.example".to_string();
        assert!(config.dial_target().is_err(), "plaintext is refused");
    }

    #[test]
    fn backoff_is_exponential_and_capped() {
        let schedule =
            BackoffSchedule::new(Duration::from_millis(1_000), Duration::from_millis(60_000))
                .expect("valid");
        assert_eq!(schedule.wait_for(0), 1_000);
        assert_eq!(schedule.wait_for(1), 1_000);
        assert_eq!(schedule.wait_for(2), 2_000);
        assert_eq!(schedule.wait_for(3), 4_000);
        assert_eq!(schedule.wait_for(7), 64_000);
        assert_eq!(
            schedule.wait_for(30),
            60_000,
            "capped at the configured maximum"
        );
    }
}
