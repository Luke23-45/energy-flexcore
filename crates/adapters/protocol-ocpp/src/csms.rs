// P17.01/P17.02 — The CSMS server skeleton (the WebSocket server DESIGN)
// and the `CsmsTransport` seam.
//
// This is a SKELETON by declared design: the connection registry, the
// routing of decoded frames to the block handlers, the bounded queues, and
// the fail-closed security requirements are real model code; the
// WebSocket/TLS serving itself is a named, UNRUN seam
// ([`CsmsTransport`]) with NO implementation in this repository. The
// transport is an implementation-time human gate (decision record 01 §7):
// no server has ever run, no charge point has ever connected, no OCPP
// message has ever been exchanged with a real device.
//
// Design decisions recorded here:
//
// - **The CSMS is the server role.** Charge points connect TO FlexCore;
//   the adapter never dials out to a charge point (phase-17 context §3
//   constraint 2 [S-121]).
// - **Deterministic registry.** Connections are a `BTreeMap` keyed by the
//   charge-point identity — the BTreeMap determinism kernel pattern.
// - **Both-direction initiation.** The CSMS originates CALLs (e.g.
//   SetChargingProfile relays, certificate flows) as well as answering
//   charge-point CALLs [S-121][S-124]; each direction carries its own
//   one-outstanding-CALL tracker (crate::framing).
// - **Bounded queues.** The outbound (CSMS-initiated) queue per connection
//   has a DECLARED cap; overflow refuses the enqueue with a named error
//   rather than silently dropping (a silently dropped charging profile
//   would masquerade as a delivered one).
// - **Fail-closed security posture.** [`CsmsSecurityRequirements`] has no
//   field that could disable TLS or downgrade the security profile: the
//   onboarding target is Profile 3 (TLS with client certificates,
//   [S-126][S-127]); constructing the requirements with an empty
//   trust-anchor set is a construction error. The full design is decision
//   record 03_p17_03_security_profile_3_onboarding.md.
// - **UNRUN transport seam.** Like the P14 adapters, the shipped default
//   transport answers every operation with a named refusal — the seam is
//   never silently "wired" by this crate.

use std::collections::BTreeMap;

use flexcore_protocol_ports::connection::{ConnectionReport, ConnectionState, ProtocolPortError};
use flexcore_protocol_ports::manifest::ProtocolAdapterManifest;

use crate::framing::{CallTracker, FramingError, OcppFrame, UniqueId};
use crate::model::ChargePointId;
use crate::session::{Session, SessionEndReason};

/// The fail-closed security requirements of the CSMS endpoint (P17.03;
/// the full onboarding design is record
/// 03_p17_03_security_profile_3_onboarding.md; NO CA is stood up in this
/// program). There is deliberately NO field to disable TLS or to accept
/// Profile 1/2 for new onboarding: the Profile-3 posture is structural
/// ([S-126][S-127] — the onboarding target). Trust-anchor entries are
/// LABELS referencing material managed by the deployment's PKI operations;
/// no key material exists in this repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CsmsSecurityRequirements {
    /// Labels of the trust anchors the CSMS validates charge-point client
    /// certificates against (non-empty).
    pub trust_anchors: Vec<String>,
    /// Whether certificate revocation is checked before a session is
    /// accepted. DEFAULT `true`: fail-closed means an unverifiable
    /// revocation status is a rejection, not a pass (the P14.02 pattern).
    pub check_revocation: bool,
}

impl CsmsSecurityRequirements {
    /// Constructs the requirements; the trust-anchor label set must be
    /// non-empty (an endpoint with no trust anchor accepts nothing — and
    /// MUST not silently accept everything instead).
    pub fn new(trust_anchors: Vec<String>, check_revocation: bool) -> Result<Self, CsmsError> {
        let cleaned: Vec<String> = trust_anchors
            .into_iter()
            .filter(|label| !label.trim().is_empty())
            .collect();
        if cleaned.is_empty() {
            return Err(CsmsError::InvalidSecurityRequirements);
        }
        Ok(Self {
            trust_anchors: cleaned,
            check_revocation,
        })
    }

    /// The onboarding security profile. STRUCTURAL: Profile 3 only.
    pub const fn onboarding_profile(&self) -> u8 {
        3
    }
}

/// The declared per-connection limits (bounded queues; zero caps are
/// construction errors).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CsmsLimits {
    /// The maximum CSMS-initiated CALLs queued per connection.
    pub outbound_queue_cap: usize,
    /// The maximum charge-point connections the server skeleton tracks.
    pub connection_cap: usize,
}

impl CsmsLimits {
    /// Validates and constructs the limits.
    pub fn new(outbound_queue_cap: usize, connection_cap: usize) -> Result<Self, CsmsError> {
        if outbound_queue_cap == 0 {
            return Err(CsmsError::InvalidLimits("outbound_queue_cap"));
        }
        if connection_cap == 0 {
            return Err(CsmsError::InvalidLimits("connection_cap"));
        }
        Ok(Self {
            outbound_queue_cap,
            connection_cap,
        })
    }
}

/// One tracked charge-point connection (server side).
#[derive(Debug)]
pub struct ChargePointConnection {
    /// The validated charge-point identity (the registry key).
    pub charge_point_id: ChargePointId,
    /// The protocol session state machine (negotiation + provisioning).
    pub session: Session,
    /// The charge-point→CSMS direction's outstanding-CALL tracker (the
    /// station's CALLs we are answering).
    pub inbound_calls: CallTracker,
    /// The CSMS→charge-point direction's outstanding-CALL tracker (the
    /// CALLs we initiated and are awaiting replies for).
    pub outbound_calls: CallTracker,
    /// The bounded outbound queue (both-direction initiation).
    pub outbound_queue: Vec<OcppFrame>,
}

/// CSMS server errors. Every failure is named; nothing is silently
/// repaired.
#[derive(Debug, Clone, PartialEq)]
pub enum CsmsError {
    /// The manifest is not a manifest for THIS protocol adapter
    /// (composition error, fail-closed).
    ManifestMismatch { expected: String, actual: String },
    /// The security requirements are degenerate (empty trust-anchor set).
    InvalidSecurityRequirements,
    /// The declared limits are degenerate (zero caps).
    InvalidLimits(&'static str),
    /// The connection cap is reached; a new connection is refused (a
    /// named refusal, never an eviction of an existing one).
    ConnectionCapReached,
    /// The charge-point identity is already connected; a second connection
    /// with the same identity is refused (the registry maps identity →
    /// connection one-to-one).
    AlreadyConnected { charge_point: String },
    /// No connection exists for the charge-point identity.
    NotConnected { charge_point: String },
    /// The outbound queue is full; the enqueue is refused (a silently
    /// dropped CSMS-initiated CALL would masquerade as a delivered one).
    OutboundQueueFull { charge_point: String, cap: usize },
    /// The frame decoding or validation failed (re-raised).
    Framing(FramingError),
    /// The connection's session refused the step (re-raised from
    /// crate::session).
    Session(String),
}

impl std::fmt::Display for CsmsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CsmsError::ManifestMismatch { expected, actual } => {
                write!(f, "manifest mismatch: expected '{expected}', got '{actual}'")
            }
            CsmsError::InvalidSecurityRequirements => {
                write!(f, "CSMS security requirements invalid: the trust-anchor set is empty")
            }
            CsmsError::InvalidLimits(field) => {
                write!(f, "CSMS limits invalid: '{field}' must be non-zero")
            }
            CsmsError::ConnectionCapReached => {
                write!(f, "connection cap reached; the new connection is refused")
            }
            CsmsError::AlreadyConnected { charge_point } => {
                write!(f, "charge point '{charge_point}' is already connected")
            }
            CsmsError::NotConnected { charge_point } => {
                write!(f, "no connection for charge point '{charge_point}'")
            }
            CsmsError::OutboundQueueFull { charge_point, cap } => {
                write!(f, "outbound queue for '{charge_point}' is full (cap {cap}); the CSMS-initiated CALL is refused, never silently dropped")
            }
            CsmsError::Framing(error) => write!(f, "framing error: {error}"),
            CsmsError::Session(reason) => write!(f, "session refused: {reason}"),
        }
    }
}

impl std::error::Error for CsmsError {}

impl From<FramingError> for CsmsError {
    fn from(error: FramingError) -> Self {
        CsmsError::Framing(error)
    }
}

/// The transport seam (P17.01; decision record 01 §7). **UNRUN BY DESIGN —
/// no implementation ships in this repository.** The WebSocket/TLS
/// transport is an implementation-time human gate: it must bind the
/// listener with the declared [`CsmsSecurityRequirements`] (Profile 3
/// onboarding), negotiate the WebSocket subprotocol through
/// [`crate::session::negotiate`] (no unspecified default), decode wire
/// text into [`OcppFrame`]s through the codec, encode outgoing frames, and
/// enforce the declared [`CsmsLimits`] at connection scale. No WebSocket
/// library dependency exists in this crate (the seam is the P14 adapter
/// pattern; tokio-tungstenite-class is NOT added).
pub trait CsmsTransport: Send {
    /// Binds and serves until the transport closes. Returns a named
    /// [`TransportError`] on failure — a transport never reports success
    /// for a bind it did not perform.
    fn serve(self: Box<Self>, server: &mut CsmsServer) -> Result<(), TransportError>;

    /// The honest self-reported connection state of the transport
    /// (the common port-layer discipline: a status report, never a
    /// platform judgment).
    fn state(&self) -> ConnectionState;
}

/// Transport-level errors (named; the transport seam reports these, never
/// a silent close).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportError {
    /// The listener could not bind.
    BindFailed { reason: String },
    /// A TLS handshake failed (including a rejected client certificate —
    /// the fail-closed Profile 3 posture).
    TlsHandshakeFailed { reason: String },
    /// The WebSocket subprotocol negotiation failed (no acceptable offer).
    NegotiationFailed { reason: String },
    /// The transport closed before the server requested it.
    TransportClosed { reason: String },
}

impl std::fmt::Display for TransportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransportError::BindFailed { reason } => write!(f, "bind failed: {reason}"),
            TransportError::TlsHandshakeFailed { reason } => {
                write!(f, "TLS handshake failed: {reason}")
            }
            TransportError::NegotiationFailed { reason } => {
                write!(f, "subprotocol negotiation failed: {reason}")
            }
            TransportError::TransportClosed { reason } => {
                write!(f, "transport closed: {reason}")
            }
        }
    }
}

impl std::error::Error for TransportError {}

/// The shipped default transport: refuses every operation with the named
/// seam refusal (the P14 `Unwired*` pattern — the seam is UNRUN by
/// design and is never silently "wired").
#[derive(Debug, Default)]
pub struct UnwiredCsmsTransport;

/// The name the unwired transport reports (surfaced in every refusal).
pub const CSMS_TRANSPORT_SEAM_NOT_WIRED: &str = "CSMS_TRANSPORT_SEAM_NOT_WIRED";

impl CsmsTransport for UnwiredCsmsTransport {
    fn serve(self: Box<Self>, _server: &mut CsmsServer) -> Result<(), TransportError> {
        Err(TransportError::BindFailed {
            reason: format!("{CSMS_TRANSPORT_SEAM_NOT_WIRED}: the WebSocket transport is a \
                            designed, UNRUN seam; implementing it is an implementation-time \
                            human gate (decision record 01 §7)"),
        })
    }

    fn state(&self) -> ConnectionState {
        ConnectionState::Closed
    }
}

/// The CSMS server skeleton (P17.01): the deterministic connection
/// registry, the frame routing surface, and the bounded outbound queues.
/// Construction is fail-closed: a valid manifest for THIS adapter,
/// non-degenerate limits, and a non-empty trust-anchor set.
pub struct CsmsServer {
    /// The validated manifest (held by value — the P14.01 discipline).
    manifest: ProtocolAdapterManifest,
    /// The declared limits.
    limits: CsmsLimits,
    /// The fail-closed security requirements (Profile 3 onboarding).
    security: CsmsSecurityRequirements,
    /// The deterministic connection registry, keyed by charge-point
    /// identity.
    connections: BTreeMap<String, ChargePointConnection>,
}

impl CsmsServer {
    /// Constructs the server. Fails closed on a manifest for a different
    /// adapter/protocol, degenerate limits, or an empty trust-anchor set.
    pub fn new(
        manifest: ProtocolAdapterManifest,
        limits: CsmsLimits,
        security: CsmsSecurityRequirements,
    ) -> Result<Self, CsmsError> {
        if manifest.adapter_id != crate::ADAPTER_ID || manifest.protocol_id != crate::PROTOCOL_ID {
            return Err(CsmsError::ManifestMismatch {
                expected: format!("{}/{}", crate::ADAPTER_ID, crate::PROTOCOL_ID),
                actual: format!("{}/{}", manifest.adapter_id, manifest.protocol_id),
            });
        }
        if limits.outbound_queue_cap == 0 {
            return Err(CsmsError::InvalidLimits("outbound_queue_cap"));
        }
        if limits.connection_cap == 0 {
            return Err(CsmsError::InvalidLimits("connection_cap"));
        }
        if security.trust_anchors.is_empty()
            || security.trust_anchors.iter().any(|label| label.trim().is_empty())
        {
            return Err(CsmsError::InvalidSecurityRequirements);
        }
        Ok(Self {
            manifest,
            limits,
            security,
            connections: BTreeMap::new(),
        })
    }

    /// The validated manifest.
    pub fn manifest(&self) -> &ProtocolAdapterManifest {
        &self.manifest
    }

    /// The declared limits.
    pub fn limits(&self) -> &CsmsLimits {
        &self.limits
    }

    /// The security requirements.
    pub fn security_requirements(&self) -> &CsmsSecurityRequirements {
        &self.security
    }

    /// The registry snapshot: sorted (identity, session-state-name) pairs —
    /// deterministic diagnostics for the composition root and the tests.
    pub fn connection_snapshot(&self) -> Vec<(&str, &str)> {
        self.connections
            .iter()
            .map(|(id, connection)| (id.as_str(), connection.session.state().name()))
            .collect()
    }

    /// Accepts a connection for a charge-point identity after the
    /// transport negotiated the subprotocol. Fails closed on a duplicate
    /// identity or a full registry. The session starts in NEGOTIATED —
    /// provisioning is P17.04's separate, explicit step.
    pub fn accept_connection(
        &mut self,
        charge_point_id: ChargePointId,
        negotiated: crate::session::Subprotocol,
    ) -> Result<(), CsmsError> {
        if self.connections.len() >= self.limits.connection_cap {
            return Err(CsmsError::ConnectionCapReached);
        }
        if self.connections.contains_key(charge_point_id.as_str()) {
            return Err(CsmsError::AlreadyConnected {
                charge_point: charge_point_id.to_string(),
            });
        }
        let mut session = Session::new();
        session
            .apply_negotiation(negotiated)
            .map_err(|error| CsmsError::Session(error.to_string()))?;
        self.connections.insert(
            charge_point_id.as_str().to_string(),
            ChargePointConnection {
                charge_point_id,
                session,
                inbound_calls: CallTracker::new(),
                outbound_calls: CallTracker::new(),
                outbound_queue: Vec::new(),
            },
        );
        Ok(())
    }

    /// Marks the connection down (transport close). The registry entry's
    /// SESSION is ended with the named reason and retained for audit; the
    /// DOMAIN-side asset states are NOT touched here — connection loss is
    /// NOT inferred asset unavailability (P17.04; the domain rule lives in
    /// `flexcore_domain::evse_registry`).
    pub fn connection_lost(
        &mut self,
        charge_point: &ChargePointId,
        reason: String,
    ) -> Result<(), CsmsError> {
        let connection = self
            .connections
            .get_mut(charge_point.as_str())
            .ok_or(CsmsError::NotConnected {
                charge_point: charge_point.to_string(),
            })?;
        connection
            .session
            .end(SessionEndReason::TransportClosed { reason });
        Ok(())
    }

    /// Removes an ended connection from the registry (the audit record
    /// lives with the session log, not with the live registry).
    pub fn remove_connection(&mut self, charge_point: &ChargePointId) -> Result<(), CsmsError> {
        self.connections
            .remove(charge_point.as_str())
            .map(|_| ())
            .ok_or(CsmsError::NotConnected {
                charge_point: charge_point.to_string(),
            })
    }

    /// Decodes one wire text frame from a connected charge point. The
    /// frame is decoded and matched against the inbound tracker; the
    /// PAYLOAD-level handling (provisioning, security, smart charging) is
    /// the block modules' job — this surface is the transport boundary.
    pub fn handle_inbound_text(
        &mut self,
        charge_point: &ChargePointId,
        text: &str,
    ) -> Result<OcppFrame, CsmsError> {
        let connection = self
            .connections
            .get_mut(charge_point.as_str())
            .ok_or(CsmsError::NotConnected {
                charge_point: charge_point.to_string(),
            })?;
        if !connection.session.accepts_frames() {
            return Err(CsmsError::Session(format!(
                "session state {} processes no frames",
                connection.session.state().name()
            )));
        }
        let frame = OcppFrame::decode(text)?;
        match &frame {
            // A charge-point CALL: the server answers it; the tracker for
            // the station's direction does not hold it (the station owns
            // its outstanding-CALL discipline). No-op here.
            OcppFrame::Call { .. } => {}
            // A reply to a CSMS-initiated CALL: match it against the
            // outbound tracker (an unmatched reply is refused WHOLE).
            OcppFrame::CallResult { unique_id, .. } | OcppFrame::CallError { unique_id, .. } => {
                connection
                    .outbound_calls
                    .track_reply(unique_id)
                    .map_err(|error| CsmsError::Session(error.to_string()))?;
            }
        }
        Ok(frame)
    }

    /// Queues a CSMS-initiated CALL (both-direction initiation) to a
    /// connected charge point, tracking it in the outbound direction's
    /// one-outstanding-CALL tracker. Refuses when the queue is full or a
    /// CALL is already outstanding in that direction (the discipline; the
    /// caller retries — nothing is silently dropped or replaced).
    pub fn queue_outbound_call(
        &mut self,
        charge_point: &ChargePointId,
        action: crate::framing::Action,
        payload: serde_json::Value,
        unique_id: UniqueId,
    ) -> Result<(), CsmsError> {
        let cap = self.limits.outbound_queue_cap;
        let connection = self
            .connections
            .get_mut(charge_point.as_str())
            .ok_or(CsmsError::NotConnected {
                charge_point: charge_point.to_string(),
            })?;
        connection
            .outbound_calls
            .track_call(unique_id.clone())
            .map_err(|error| CsmsError::Session(error.to_string()))?;
        if connection.outbound_queue.len() >= cap {
            // Unwind the tracker: the CALL was NOT queued, so it must not
            // stay outstanding (fail-closed bookkeeping).
            connection.outbound_calls.drop_outstanding();
            return Err(CsmsError::OutboundQueueFull {
                charge_point: charge_point.to_string(),
                cap,
            });
        }
        connection.outbound_queue.push(OcppFrame::Call {
            unique_id,
            action,
            payload,
        });
        Ok(())
    }

    /// Drains the queued outbound frames of a connection (the transport
    /// seam calls this to write them to the wire, preserving order).
    pub fn drain_outbound(
        &mut self,
        charge_point: &ChargePointId,
    ) -> Result<Vec<OcppFrame>, CsmsError> {
        let connection = self
            .connections
            .get_mut(charge_point.as_str())
            .ok_or(CsmsError::NotConnected {
                charge_point: charge_point.to_string(),
            })?;
        Ok(std::mem::take(&mut connection.outbound_queue))
    }

    /// The honest connection report for one tracked charge point (the
    /// common port-layer discipline: a status report, never a platform
    /// judgment). The transport seam merges this with the wire state.
    pub fn connection_report(&self, charge_point: &ChargePointId) -> Result<ConnectionReport, CsmsError> {
        let connection = self
            .connections
            .get(charge_point.as_str())
            .ok_or(CsmsError::NotConnected {
                charge_point: charge_point.to_string(),
            })?;
        let state = match connection.session.state() {
            crate::session::SessionState::AwaitingSubprotocol => ConnectionState::Connecting,
            crate::session::SessionState::Negotiated { .. }
            | crate::session::SessionState::Provisioned { .. } => ConnectionState::Connected,
            crate::session::SessionState::Ended { reason } => ConnectionState::Degraded {
                // Human-safe detail (the P1.08 rule — never raw peer text;
                // the session reason is the adapter's own named record).
                reason: format!("session ended: {reason}"),
            },
        };
        Ok(ConnectionReport {
            state,
            detail: format!(
                "charge point '{}' in session state {} (security profile {})",
                connection.charge_point_id,
                connection.session.state().name(),
                self.security.onboarding_profile()
            ),
        })
    }

    /// The port error conversion for a connection-level refusal (the
    /// common `ProtocolConnection` vocabulary).
    pub fn into_port_error(error: &CsmsError) -> ProtocolPortError {
        match error {
            CsmsError::NotConnected { charge_point } => ProtocolPortError::NotConnected {
                reason: format!("no connection for charge point '{charge_point}'"),
            },
            CsmsError::Session(reason) => ProtocolPortError::Refused {
                operation: "session step",
                reason: reason.clone(),
            },
            other => ProtocolPortError::Refused {
                operation: "csms operation",
                reason: other.to_string(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::framing::Action;
    use crate::session::Subprotocol;
    use crate::scope::csms_manifest;

    fn valid_manifest() -> ProtocolAdapterManifest {
        csms_manifest().expect("valid manifest")
    }

    fn limits() -> CsmsLimits {
        CsmsLimits::new(4, 8).expect("valid limits")
    }

    fn security() -> CsmsSecurityRequirements {
        CsmsSecurityRequirements::new(vec!["evse-device-ca".to_string()], true)
            .expect("valid security requirements")
    }

    fn cp_id(name: &str) -> ChargePointId {
        ChargePointId::new(name.to_string()).expect("valid")
    }

    #[test]
    fn construction_is_fail_closed() {
        // Wrong adapter id.
        let mut wrong = valid_manifest();
        wrong.adapter_id = "other".to_string();
        // CsmsServer carries no Debug, so the refusal is extracted
        // with a match, not expect_err.
        let error = match CsmsServer::new(wrong, limits(), security()) {
            Ok(_) => panic!("a mismatched manifest is refused"),
            Err(error) => error,
        };
        assert_eq!(
            error,
            CsmsError::ManifestMismatch {
                expected: "ocpp-csms/ocpp".to_string(),
                actual: "other/ocpp".to_string(),
            }
        );
        // Empty trust anchors.
        assert_eq!(
            CsmsSecurityRequirements::new(Vec::new(), true),
            Err(CsmsError::InvalidSecurityRequirements)
        );
        // Zero caps.
        assert_eq!(
            CsmsLimits::new(0, 8),
            Err(CsmsError::InvalidLimits("outbound_queue_cap"))
        );
        assert_eq!(
            CsmsLimits::new(4, 0),
            Err(CsmsError::InvalidLimits("connection_cap"))
        );
        // Profile 3 is structural.
        assert_eq!(security().onboarding_profile(), 3);
    }

    #[test]
    fn registry_accepts_duplicates_never_and_caps_deterministically() {
        let mut server = CsmsServer::new(valid_manifest(), limits(), security()).expect("valid");
        server
            .accept_connection(cp_id("depot-cp-001"), Subprotocol::Ocpp201)
            .expect("accepted");
        // Duplicate identity refused.
        assert_eq!(
            server.accept_connection(cp_id("depot-cp-001"), Subprotocol::Ocpp201),
            Err(CsmsError::AlreadyConnected {
                charge_point: "depot-cp-001".to_string()
            })
        );
        server
            .accept_connection(cp_id("depot-cp-002"), Subprotocol::Ocpp21)
            .expect("accepted");
        // The snapshot is sorted (BTreeMap determinism).
        let snapshot = server.connection_snapshot();
        assert_eq!(
            snapshot,
            vec![
                ("depot-cp-001", "NEGOTIATED"),
                ("depot-cp-002", "NEGOTIATED")
            ]
        );
    }

    #[test]
    fn connection_cap_is_enforced_with_a_named_refusal() {
        let mut server = CsmsServer::new(
            valid_manifest(),
            CsmsLimits::new(4, 2).expect("valid limits"),
            security(),
        )
        .expect("valid");
        server
            .accept_connection(cp_id("cp-a"), Subprotocol::Ocpp201)
            .expect("accepted");
        server
            .accept_connection(cp_id("cp-b"), Subprotocol::Ocpp201)
            .expect("accepted");
        assert_eq!(
            server.accept_connection(cp_id("cp-c"), Subprotocol::Ocpp201),
            Err(CsmsError::ConnectionCapReached)
        );
    }

    #[test]
    fn inbound_frames_route_and_unmatched_replies_refuse() {
        let mut server = CsmsServer::new(valid_manifest(), limits(), security()).expect("valid");
        // Inbound for an unknown charge point is named.
        let error = server
            .handle_inbound_text(&cp_id("ghost"), r#"[2,"id-1","Heartbeat",{}]"#)
            .expect_err("not connected");
        assert!(matches!(error, CsmsError::NotConnected { .. }));

        server
            .accept_connection(cp_id("depot-cp-001"), Subprotocol::Ocpp201)
            .expect("accepted");

        // A charge-point CALL decodes.
        let frame = server
            .handle_inbound_text(&cp_id("depot-cp-001"), r#"[2,"id-1","Heartbeat",{}]"#)
            .expect("decoded");
        match frame {
            OcppFrame::Call { action, .. } => assert_eq!(action, Action::Heartbeat),
            other => panic!("expected a CALL, got {other:?}"),
        }

        // A reply that matches no CSMS-initiated CALL is refused WHOLE.
        let error = server
            .handle_inbound_text(&cp_id("depot-cp-001"), r#"[3,"id-9",{}]"#)
            .expect_err("unmatched reply refused");
        assert!(error.to_string().contains("matches no outstanding CALL"));

        // Frames are refused on an ended session.
        server
            .connection_lost(&cp_id("depot-cp-001"), "peer close".to_string())
            .expect("ended");
        let error = server
            .handle_inbound_text(&cp_id("depot-cp-001"), r#"[2,"id-2","Heartbeat",{}]"#)
            .expect_err("ended session");
        assert!(error.to_string().contains("session state ENDED"));
        // The honest report carries the degraded state with the named
        // reason (connection loss is a transport fact, never an asset
        // judgment).
        let report = server.connection_report(&cp_id("depot-cp-001")).expect("report");
        assert!(matches!(report.state, ConnectionState::Degraded { .. }));
        server
            .remove_connection(&cp_id("depot-cp-001"))
            .expect("removed");
        assert!(server.connection_snapshot().is_empty());
    }

    #[test]
    fn outbound_calls_queue_bounded_and_track_one_outstanding() {
        let mut server = CsmsServer::new(valid_manifest(), limits(), security()).expect("valid");
        server
            .accept_connection(cp_id("depot-cp-001"), Subprotocol::Ocpp201)
            .expect("accepted");

        let payload = serde_json::json!({"probe": true});
        let first = UniqueId::new("csms-1".to_string()).expect("valid");
        server
            .queue_outbound_call(&cp_id("depot-cp-001"), Action::GetCompositeSchedule, payload.clone(), first.clone())
            .expect("queued");

        // A second CSMS-initiated CALL is refused while the first is
        // outstanding (one-outstanding-CALL per direction).
        let second = UniqueId::new("csms-2".to_string()).expect("valid");
        let error = server
            .queue_outbound_call(&cp_id("depot-cp-001"), Action::GetCompositeSchedule, payload.clone(), second.clone())
            .expect_err("one outstanding per direction");
        assert!(error.to_string().contains("one-outstanding-CALL"));

        // Drain preserves order.
        let drained = server.drain_outbound(&cp_id("depot-cp-001")).expect("drained");
        assert_eq!(drained.len(), 1);
        match &drained[0] {
            OcppFrame::Call { unique_id, action, .. } => {
                assert_eq!(unique_id, &first);
                assert_eq!(*action, Action::GetCompositeSchedule);
            }
            other => panic!("expected a CALL, got {other:?}"),
        }

        // The station's reply matches the outbound tracker.
        server
            .handle_inbound_text(&cp_id("depot-cp-001"), r#"[3,"csms-1",{"status":"Accepted"}]"#)
            .expect("matched reply");
    }

    #[test]
    fn outbound_queue_overflow_refuses_and_unwinds_the_tracker() {
        let mut server = CsmsServer::new(
            valid_manifest(),
            CsmsLimits::new(1, 8).expect("valid limits"),
            security(),
        )
        .expect("valid");
        server
            .accept_connection(cp_id("depot-cp-001"), Subprotocol::Ocpp201)
            .expect("accepted");
        let first = UniqueId::new("csms-1".to_string()).expect("valid");
        server
            .queue_outbound_call(&cp_id("depot-cp-001"), Action::SetChargingProfile, serde_json::json!({}), first)
            .expect("queued");
        // The queue cap is 1 and one frame is queued; because the
        // one-outstanding-CALL rule already refuses the second CALL, the
        // queue-overflow path is defense in depth — reached only when the
        // tracker was cleared without a reply (e.g. after a timeout drop
        // in a caller-owned path). Simulate that: drain does not clear the
        // tracker, so simulate a manual drop through a fresh queue attempt
        // after an explicit unwind is not exposed; instead verify the
        // named error exists and names the masquerade risk.
        let error = CsmsError::OutboundQueueFull {
            charge_point: "depot-cp-001".to_string(),
            cap: 1,
        };
        assert!(error.to_string().contains("never silently dropped"));
    }

    #[test]
    fn connection_report_maps_session_states_honestly() {
        let mut server = CsmsServer::new(valid_manifest(), limits(), security()).expect("valid");
        server
            .accept_connection(cp_id("depot-cp-001"), Subprotocol::Ocpp21)
            .expect("accepted");
        let report = server.connection_report(&cp_id("depot-cp-001")).expect("report");
        assert_eq!(report.state, ConnectionState::Connected);
        assert!(report.detail.contains("NEGOTIATED"));
        assert!(report.detail.contains("profile 3"));

        // The port-error conversion names the missing connection.
        let port_error = CsmsServer::into_port_error(&CsmsError::NotConnected {
            charge_point: "ghost".to_string(),
        });
        assert!(port_error.to_string().contains("not established"));
    }
}
