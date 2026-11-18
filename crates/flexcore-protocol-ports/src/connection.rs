// P14.01 — The protocol-adapter connection lifecycle port (decision record
// 01_p14_01_adapter_architecture.md §5; mirrors the P6.12 `EmsAdapter`
// port discipline: protocol-shaped, not vendor-shaped; honest health;
// deliberate structural absence of a write path).
//
// The contract, precisely:
//
// - [`ProtocolConnection::manifest`] returns the adapter's reviewed
//   capability manifest (scope + capabilities + authority). Every adapter
//   holds it by value; the port exposes it by reference.
// - [`ProtocolConnection::connect`] establishes the adapter's protocol
//   association and returns a [`ConnectionReport`]. A connect that fails
//   leaves the adapter in [`ConnectionState::Unreachable`] or
//   [`ConnectionState::Closed`] — it never fabricates a connected state
//   (the fail-closed rule).
// - [`ProtocolConnection::disconnect`] tears the association down and
//   returns the final report. Disconnect is always allowed, even when the
//   adapter was never connected.
// - [`ProtocolConnection::state`] is the honest self-reported connection
//   state — a status report, never a platform judgement.
//
// **What the port deliberately does NOT define** (the P6.12 rule carried
// forward): no write path, no send path, no setpoint path, and no data path
// — data shapes are protocol-specific and live in the concrete adapter
// crates behind their own typed surfaces. Absence is structural. A
// `CommandRelay` capability in the manifest describes what the ADAPTER may
// be permitted later, at composition time, through the command lifecycle —
// never through this trait.
//
// **RECORDED DEFERRAL (FRV1-R6-001, final review R6, phase-14):** no
// concrete adapter implements this trait in the v1.0.0 baseline. The
// connection-state vocabulary travels through adapter-specific surfaces
// today (OCPP consumes [`ConnectionReport`]/[`ConnectionState`] from its
// `CsmsTransport` seam; the P14 adapters report transport state through
// their own seam types). Per-adapter `ProtocolConnection`
// implementations land with the first adapter composition; until then the
// shipped default is [`UnwiredProtocolConnection`], which refuses every
// lifecycle operation with the named `PROTOCOL_CONNECTION_SEAM_NOT_WIRED`
// refusal — the same fail-closed `Unwired*` pattern as the adapter-side
// transport seams (`UnwiredA44Transport`, `UnwiredCsmsTransport`, ...).
// The absence of an implementation can never look like a connection.

use crate::manifest::ProtocolAdapterManifest;

/// The connection state of a protocol adapter (the P2.11 provider-health
/// shape, connection-flavoured). It is a status report, not a platform
/// judgement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionState {
    /// The adapter has no protocol association (the state before the first
    /// connect and after every successful disconnect).
    Closed,
    /// A connect attempt is in flight; no data is trusted from an adapter
    /// in this state.
    Connecting,
    /// The protocol association is established.
    Connected,
    /// The association is up but impaired; the reason is human-safe (never
    /// raw peer or backend text — the P1.08 rule).
    Degraded { reason: String },
    /// The adapter cannot reach its peer; the reason is human-safe.
    Unreachable { reason: String },
}

impl ConnectionState {
    /// Canonical name for records, metrics, and logs (the P11 taxonomy
    /// consumers map onto these names; P14.21 consumes, never redefines).
    pub const fn name(&self) -> &'static str {
        match self {
            ConnectionState::Closed => "CLOSED",
            ConnectionState::Connecting => "CONNECTING",
            ConnectionState::Connected => "CONNECTED",
            ConnectionState::Degraded { .. } => "DEGRADED",
            ConnectionState::Unreachable { .. } => "UNREACHABLE",
        }
    }
}

/// The report a connect/disconnect returns: the resulting state plus a
/// human-safe detail line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionReport {
    /// The state AFTER the operation completed.
    pub state: ConnectionState,
    /// A human-safe detail line (never raw peer text; the P1.08 rule).
    pub detail: String,
}

/// Errors a protocol connection reports. Every failure is named; nothing
/// implies a silent pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtocolPortError {
    /// The operation requires an established association and the adapter
    /// has none. The adapter must NOT fabricate data — the fail-closed rule.
    NotConnected { reason: String },
    /// The adapter refused the operation by its own declared rules (scope
    /// gate, allowlist, capability gate). The refusal names the rule.
    Refused {
        operation: &'static str,
        reason: String,
    },
    /// The peer rejected the association (TLS failure, registration
    /// refusal, ...). The reason is human-safe.
    PeerRejected { reason: String },
    /// The declared protocol scope refused the request (re-raised from the
    /// scope gate; the adapter reports the refusal instead of approximating).
    Scope(crate::scope::ProtocolScopeError),
}

impl std::fmt::Display for ProtocolPortError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProtocolPortError::NotConnected { reason } => {
                write!(f, "protocol connection not established: {reason}")
            }
            ProtocolPortError::Refused { operation, reason } => {
                write!(f, "operation '{operation}' refused by declared rules: {reason}")
            }
            ProtocolPortError::PeerRejected { reason } => {
                write!(f, "peer rejected the association: {reason}")
            }
            ProtocolPortError::Scope(error) => {
                write!(f, "request exceeds the declared protocol scope: {error}")
            }
        }
    }
}

impl std::error::Error for ProtocolPortError {}

impl From<crate::scope::ProtocolScopeError> for ProtocolPortError {
    fn from(error: crate::scope::ProtocolScopeError) -> Self {
        ProtocolPortError::Scope(error)
    }
}

/// The protocol-adapter connection lifecycle port (P14.01). One
/// implementation per concrete protocol adapter; the composition registry
/// (the P6.12 allowlist pattern) decides which ids may be constructed.
///
/// Sync trait (house rule): a concrete adapter owns its own I/O shape
/// (blocking socket, async runtime bridge, ...) and bridges to this sync
/// surface; composition roots stay runtime-free.
pub trait ProtocolConnection: Send {
    /// The registered adapter id (the registry's allowlist key).
    fn id(&self) -> &str;

    /// The reviewed capability manifest (scope + capabilities + authority),
    /// held by value inside the adapter.
    fn manifest(&self) -> &ProtocolAdapterManifest;

    /// Establishes the protocol association. A failed connect leaves the
    /// adapter CLOSED or UNREACHABLE with a named reason — never a
    /// fabricated CONNECTED state.
    fn connect(&mut self) -> Result<ConnectionReport, ProtocolPortError>;

    /// Tears the association down. Always permitted; a disconnect on a
    /// never-connected adapter reports the CLOSED state honestly.
    fn disconnect(&mut self) -> Result<ConnectionReport, ProtocolPortError>;

    /// The honest self-reported connection state.
    fn state(&self) -> ConnectionState;
}

/// The name the unwired default reports in every refusal (surfaced in the
/// [`ProtocolPortError::Refused`] reason so a log or metric labels the
/// unwired seam by name).
pub const PROTOCOL_CONNECTION_SEAM_NOT_WIRED: &str = "PROTOCOL_CONNECTION_SEAM_NOT_WIRED";

/// The shipped default `ProtocolConnection` implementation: refuses every
/// lifecycle operation with the named `PROTOCOL_CONNECTION_SEAM_NOT_WIRED`
/// refusal (the FRV1-R6-001 recorded deferral; the adapter-side
/// `Unwired*` seam pattern lifted to the port layer). The manifest is held
/// by value exactly as a concrete adapter would hold it, so the port's
/// introspection surface stays honest while the lifecycle stays CLOSED:
/// `state()` always reports [`ConnectionState::Closed`] and `connect` /
/// `disconnect` always refuse. A wired connection can only arrive as a
/// deliberate, composed implementation — never as a silent default.
#[derive(Debug)]
pub struct UnwiredProtocolConnection {
    /// The validated manifest held by value (the P14.01 discipline).
    manifest: ProtocolAdapterManifest,
}

impl UnwiredProtocolConnection {
    /// Wraps an already-validated manifest. The manifest itself is NOT
    /// revalidated here — construction with a valid manifest is the
    /// composition root's reviewed step, as for every adapter.
    pub fn new(manifest: ProtocolAdapterManifest) -> Self {
        Self { manifest }
    }
}

impl ProtocolConnection for UnwiredProtocolConnection {
    fn id(&self) -> &str {
        &self.manifest.adapter_id
    }

    fn manifest(&self) -> &ProtocolAdapterManifest {
        &self.manifest
    }

    fn connect(&mut self) -> Result<ConnectionReport, ProtocolPortError> {
        Err(ProtocolPortError::Refused {
            operation: "connect",
            reason: format!(
                "{PROTOCOL_CONNECTION_SEAM_NOT_WIRED}: no adapter implements the \
                 ProtocolConnection port in this baseline (FRV1-R6-001 recorded \
                 deferral); per-adapter connection reporting lands with the first \
                 adapter composition"
            ),
        })
    }

    fn disconnect(&mut self) -> Result<ConnectionReport, ProtocolPortError> {
        Err(ProtocolPortError::Refused {
            operation: "disconnect",
            reason: format!(
                "{PROTOCOL_CONNECTION_SEAM_NOT_WIRED}: no adapter implements the \
                 ProtocolConnection port in this baseline (FRV1-R6-001 recorded \
                 deferral)"
            ),
        })
    }

    fn state(&self) -> ConnectionState {
        ConnectionState::Closed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{AuthorityDeclaration, DeclaredCapability};
    use crate::scope::{ControlScope, NotModelled, ProtocolRole, ResourceClass};
    use std::time::Duration;

    fn scope() -> crate::scope::ProtocolScopeDeclaration {
        crate::scope::ProtocolScopeDeclaration::new(
            "test.adapter".to_string(),
            "test-protocol".to_string(),
            vec![ProtocolRole::Client],
            ControlScope::MonitorOnly,
            vec![ResourceClass::Telemetry],
            vec![NotModelled::new(
                "reviewed_limitation".to_string(),
                "the reviewed limitation record is mandatory and non-empty".to_string(),
            )
            .expect("valid")],
            "external events enter the command lifecycle at PROPOSED; this adapter \
             is NOT a dispatch authority"
                .to_string(),
            Vec::new(),
        )
        .expect("valid scope")
    }

    fn manifest() -> ProtocolAdapterManifest {
        ProtocolAdapterManifest::new(
            "test.adapter".to_string(),
            "test-protocol".to_string(),
            "test edition 1.0".to_string(),
            scope(),
            vec![DeclaredCapability::Read],
            Duration::from_secs(60),
            Vec::new(),
        )
        .expect("valid manifest")
    }

    #[test]
    fn connection_state_names_cover_the_five_states() {
        assert_eq!(ConnectionState::Closed.name(), "CLOSED");
        assert_eq!(ConnectionState::Connecting.name(), "CONNECTING");
        assert_eq!(ConnectionState::Connected.name(), "CONNECTED");
        assert_eq!(
            ConnectionState::Degraded {
                reason: "slow peer".to_string()
            }
            .name(),
            "DEGRADED"
        );
        assert_eq!(
            ConnectionState::Unreachable {
                reason: "no route".to_string()
            }
            .name(),
            "UNREACHABLE"
        );
    }

    #[test]
    fn error_display_carries_named_reasons_and_rule_names() {
        let error = ProtocolPortError::NotConnected {
            reason: "no association".to_string(),
        };
        assert!(error.to_string().contains("not established"));

        let error = ProtocolPortError::Refused {
            operation: "read",
            reason: "capability not declared".to_string(),
        };
        assert!(error.to_string().contains("read"));
        assert!(error.to_string().contains("refused by declared rules"));

        let error = ProtocolPortError::PeerRejected {
            reason: "registration refused".to_string(),
        };
        assert!(error.to_string().contains("peer rejected"));
    }

    #[test]
    fn connection_report_is_an_honest_state_plus_detail() {
        let report = ConnectionReport {
            state: ConnectionState::Connected,
            detail: "association established".to_string(),
        };
        assert_eq!(report.state, ConnectionState::Connected);
        assert_eq!(report.detail, "association established");
        // The authority declaration is always the one supervisory value.
        assert_eq!(
            manifest().authority,
            AuthorityDeclaration::SupervisoryRelayOnly
        );
    }

    #[test]
    fn the_unwired_default_refuses_every_lifecycle_operation_by_name() {
        // The FRV1-R6-001 recorded deferral: the shipped default fails
        // closed with the named seam refusal — never a fabricated
        // connection.
        let mut connection = UnwiredProtocolConnection::new(manifest());
        assert_eq!(connection.id(), "test.adapter");
        assert_eq!(connection.manifest().adapter_id, "test.adapter");
        assert_eq!(connection.state(), ConnectionState::Closed);

        let error = connection.connect().expect_err("unwired connect refuses");
        assert!(matches!(error, ProtocolPortError::Refused { .. }));
        assert!(error.to_string().contains(PROTOCOL_CONNECTION_SEAM_NOT_WIRED));
        assert!(error.to_string().contains("connect"));

        let error = connection
            .disconnect()
            .expect_err("unwired disconnect refuses");
        assert!(matches!(error, ProtocolPortError::Refused { .. }));
        assert!(error.to_string().contains(PROTOCOL_CONNECTION_SEAM_NOT_WIRED));

        // A refused connect never leaves a fabricated state behind.
        assert_eq!(connection.state(), ConnectionState::Closed);
    }
}
