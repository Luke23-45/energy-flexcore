// P17.01 — Subprotocol negotiation and the session state machine.
//
// Module-level UNVERIFIED-ASSUMPTION register (phase-17 plan §7.5):
//
// 1. The subprotocol strings `ocpp2.0.1` and `ocpp2.1` (and `ocpp1.6`,
//    which this adapter does NOT accept — P17-E-1 DEFAULT (a) excludes the
//    1.6 dual stack) are transcribed from secondary sources
//    [S-121][S-123]; the exact WebSocket `Sec-WebSocket-Protocol` header
//    semantics and the per-protocol spelling must be confirmed against the
//    OCA transport text at implementation time.
// 2. The 2.1-is-backward-compatible claim comes from the Phase 6 research
//    (`ledger/phase-06-edge-protocol/RESEARCH.md` §3); the additive
//    relationship is a recorded finding, not a verified conformance fact.
// 3. The exact session-teardown semantics on negotiation failure (close
//    code, timing) are a transport-seam concern — the state machine here
//    declares the OUTCOME (refusal), the seam owns the wire behavior.
//
// The negotiation policy is fail-closed: a connection that fails to
// negotiate a supported subprotocol is refused WHOLE — there is no
// "negotiate down to 1.6" fallback and no unspecified default.

use serde::{Deserialize, Serialize};

use crate::model::ChargePointId;

/// The WebSocket subprotocol the CSMS connection speaks (UNVERIFIED
/// spellings — module register item 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Subprotocol {
    /// OCPP 2.0.1 — the P17-E-1 DEFAULT baseline.
    Ocpp201,
    /// OCPP 2.1 — accepted ADDITIVELY (2.1 is backward compatible with
    /// 2.0.1 per the Phase 6 research record); 2.1-only features remain
    /// design records and are never exercised by this adapter.
    Ocpp21,
}

impl Subprotocol {
    /// The wire string (UNVERIFIED spelling — module register item 1).
    pub const fn wire_name(self) -> &'static str {
        match self {
            Subprotocol::Ocpp201 => "ocpp2.0.1",
            Subprotocol::Ocpp21 => "ocpp2.1",
        }
    }

    /// Parses a negotiated subprotocol string. Refusals are named — the
    /// negotiation NEVER falls back to an unspecified default, and
    /// `ocpp1.6` is refused with the P17-E-1 reason attached.
    pub fn parse(name: &str) -> Result<Self, NegotiationError> {
        match name {
            "ocpp2.0.1" => Ok(Subprotocol::Ocpp201),
            "ocpp2.1" => Ok(Subprotocol::Ocpp21),
            "ocpp1.6" => Err(NegotiationError::SubprotocolOutOfScope {
                offered: name.to_string(),
                reason:
                    "OCPP 1.6 is out of scope per the P17-E-1 DEFAULT (2.0.1 baseline + \
                     2.1 additive; 1.6 is NOT backward compatible with 2.0.1 and would be \
                     a dual stack — P17-E-1 stays OPEN, D5 stays human-closed)"
                        .to_string(),
            }),
            other => Err(NegotiationError::SubprotocolOutOfScope {
                offered: other.to_string(),
                reason: "not a supported subprotocol of this adapter".to_string(),
            }),
        }
    }
}

/// Negotiation errors. Every refusal names the offered value and the rule
/// that refused it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NegotiationError {
    /// The offered subprotocol string is outside the accepted set (1.6
    /// carries the P17-E-1 reason verbatim).
    SubprotocolOutOfScope {
        offered: String,
        reason: String,
    },
    /// The peer offered no subprotocol at all; a CSMS connection without a
    /// negotiated protocol is refused (fail-closed, no unspecified
    /// default).
    NoSubprotocolOffered,
}

impl std::fmt::Display for NegotiationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NegotiationError::SubprotocolOutOfScope { offered, reason } => {
                write!(f, "subprotocol '{offered}' refused: {reason}")
            }
            NegotiationError::NoSubprotocolOffered => {
                write!(
                    f,
                    "no subprotocol offered: a CSMS connection is refused rather than \
                     negotiated to an unspecified default (fail-closed)"
                )
            }
        }
    }
}

impl std::error::Error for NegotiationError {}

/// The negotiated-subprotocol selection the transport seam calls (the
/// adapter's side of the WebSocket handshake). Given the peer's OFFERED
/// list (preference order as offered), the CSMS picks its accepted set in
/// the recorded preference: `ocpp2.0.1` baseline first, `ocpp2.1`
/// additive second. If none is acceptable the negotiation fails WHOLE.
///
/// UNVERIFIED: whether OCPP-J requires the server to echo exactly one of
/// the offered protocols (module register item 1) — the outcome contract
/// (one accepted subprotocol or a refusal) is what this function
/// guarantees.
pub fn negotiate(offered: &[String]) -> Result<Subprotocol, NegotiationError> {
    if offered.is_empty() {
        return Err(NegotiationError::NoSubprotocolOffered);
    }
    for name in offered {
        // The baseline preference: accept the first offered string the
        // adapter supports, in the PEER's preference order (an honest
        // negotiation selects from what was offered, never reorders it).
        if let Ok(subprotocol) = Subprotocol::parse(name) {
            return Ok(subprotocol);
        }
    }
    // Report the first offered name in the error (the record names the
    // negotiation failure; the transport seam owns the wire close).
    Err(NegotiationError::SubprotocolOutOfScope {
        offered: offered[0].clone(),
        reason: "none of the offered subprotocols is supported (accepted: ocpp2.0.1 \
                 baseline, ocpp2.1 additive)"
            .to_string(),
    })
}

/// The CSMS session state machine (one per accepted WebSocket connection).
///
/// The states are the ADAPTER-side session lifecycle, distinct from the
/// honest self-reported transport [`ConnectionState`] of the common port
/// layer (`flexcore-protocol-ports::connection::ConnectionState`): the
/// transport reports the wire state; the session tracks the PROTOCOL
/// lifecycle (negotiated, provisioned, active) on top of it. The two are
/// never conflated: a session can only reach `Provisioned` after a boot
/// record was accepted, and connection loss (transport `Closed`) leaves the
/// session's last recorded states untouched — connection loss is NOT
/// inferred asset unavailability (P17.04; the domain registry rule).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionState {
    /// The transport accepted the connection but the subprotocol handshake
    /// has not completed. No OCPP frame is processed in this state.
    AwaitingSubprotocol,
    /// A subprotocol is negotiated; the session processes frames. The
    /// charge point is not yet provisioned (no accepted boot record).
    Negotiated {
        subprotocol: Subprotocol,
    },
    /// A BootNotification was ACCEPTED for this charge point (P17.04):
    /// the station is provisioned in the domain EVSE registry extension
    /// and heartbeat/status/transaction processing is active.
    Provisioned {
        subprotocol: Subprotocol,
        charge_point_id: ChargePointId,
    },
    /// The session ended (transport close, negotiation failure, or boot
    /// rejection). The final state is retained for audit; a new session is
    /// a NEW state machine, never a resurrection of this one.
    Ended {
        reason: SessionEndReason,
    },
}

impl SessionState {
    /// Canonical name for records.
    pub const fn name(&self) -> &'static str {
        match self {
            SessionState::AwaitingSubprotocol => "AWAITING_SUBPROTOCOL",
            SessionState::Negotiated { .. } => "NEGOTIATED",
            SessionState::Provisioned { .. } => "PROVISIONED",
            SessionState::Ended { .. } => "ENDED",
        }
    }
}

/// Why a session ended (named; the transport seam owns the wire detail).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionEndReason {
    /// The subprotocol negotiation failed (the offered set is carried for
    /// the record).
    NegotiationRefused { offered: Vec<String> },
    /// The transport closed (reason is human-safe — the P1.08 rule).
    TransportClosed { reason: String },
    /// The boot was REJECTED by the provisioning policy (P17.04); the
    /// station may retry with a new session.
    BootRejected { charge_point: String },
}

impl std::fmt::Display for SessionEndReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SessionEndReason::NegotiationRefused { offered } => {
                write!(f, "subprotocol negotiation refused (offered: {offered:?})")
            }
            SessionEndReason::TransportClosed { reason } => {
                write!(f, "transport closed: {reason}")
            }
            SessionEndReason::BootRejected { charge_point } => {
                write!(f, "boot rejected for charge point '{charge_point}'")
            }
        }
    }
}

/// Session transition errors. Every failure names the state and the
/// attempted step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionError {
    /// A frame arrived before the subprotocol was negotiated. The frame is
    /// refused (never processed, never queued-then-applied).
    NotNegotiated,
    /// A boot arrived for a session that is already provisioned to another
    /// charge point — a re-boot replaces the record only through the
    /// provisioning policy's explicit path (P17.04), never as a silent
    /// side effect of this transition.
    AlreadyProvisioned { charge_point: String },
    /// A boot arrived while the session is not in the Negotiated state.
    BootNotPermitted { current: String },
    /// A provisioning step was attempted on an ended session.
    SessionEnded { reason: String },
}

impl std::fmt::Display for SessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SessionError::NotNegotiated => {
                write!(f, "no frame is processed before the subprotocol is negotiated")
            }
            SessionError::AlreadyProvisioned { charge_point } => {
                write!(f, "session already provisioned to charge point '{charge_point}'; a re-boot goes through the provisioning policy's explicit path")
            }
            SessionError::BootNotPermitted { current } => {
                write!(f, "a boot is not permitted in session state {current}")
            }
            SessionError::SessionEnded { reason } => write!(f, "session ended: {reason}"),
        }
    }
}

impl std::error::Error for SessionError {}

/// One CSMS session (state machine + declared subprotocol).
#[derive(Debug)]
pub struct Session {
    state: SessionState,
}

impl Session {
    /// A new session starts AWAITING_SUBPROTOCOL: the transport seam must
    /// negotiate before any frame is processed.
    pub fn new() -> Self {
        Self {
            state: SessionState::AwaitingSubprotocol,
        }
    }

    /// The current state.
    pub fn state(&self) -> &SessionState {
        &self.state
    }

    /// Applies the negotiation outcome. Refuses on an ended session.
    pub fn apply_negotiation(
        &mut self,
        subprotocol: Subprotocol,
    ) -> Result<(), SessionError> {
        match &self.state {
            SessionState::AwaitingSubprotocol => {
                self.state = SessionState::Negotiated { subprotocol };
                Ok(())
            }
            SessionState::Ended { reason } => Err(SessionError::SessionEnded {
                reason: reason.to_string(),
            }),
            SessionState::Negotiated { subprotocol: current }
            | SessionState::Provisioned { subprotocol: current, .. } => {
                // A re-negotiation mid-session is not a transition in this
                // machine; the recorded rule: the subprotocol is fixed for
                // the session's life (UNVERIFIED against the OCA transport
                // text — the check is in the phase-17 record 01).
                debug_assert_eq!(
                    Subprotocol::wire_name(*current),
                    Subprotocol::wire_name(subprotocol)
                );
                Err(SessionError::SessionEnded {
                    reason: "subprotocol renegotiation is not a session transition".to_string(),
                })
            }
        }
    }

    /// Applies an ACCEPTED boot record (the provisioning policy decided
    /// ACCEPTED — the session machine only records the outcome; the ACCEPT
    /// itself is P17.04's decision, never this machine's).
    pub fn apply_boot_accepted(
        &mut self,
        charge_point_id: ChargePointId,
    ) -> Result<(), SessionError> {
        match &self.state {
            SessionState::Negotiated { subprotocol } => {
                self.state = SessionState::Provisioned {
                    subprotocol: *subprotocol,
                    charge_point_id,
                };
                Ok(())
            }
            SessionState::Provisioned { charge_point_id, .. } => {
                Err(SessionError::AlreadyProvisioned {
                    charge_point: charge_point_id.to_string(),
                })
            }
            SessionState::AwaitingSubprotocol => {
                Err(SessionError::BootNotPermitted {
                    current: self.state.name().to_string(),
                })
            }
            SessionState::Ended { reason } => Err(SessionError::SessionEnded {
                reason: reason.to_string(),
            }),
        }
    }

    /// Ends the session (named reason; retained for audit).
    pub fn end(&mut self, reason: SessionEndReason) {
        self.state = SessionState::Ended { reason };
    }

    /// Whether OCPP frames may be processed in this state (negotiated or
    /// provisioned only).
    pub fn accepts_frames(&self) -> bool {
        matches!(
            self.state,
            SessionState::Negotiated { .. } | SessionState::Provisioned { .. }
        )
    }

    /// Whether the session is provisioned (P17.04: provisioning-gated
    /// processing).
    pub fn is_provisioned(&self) -> bool {
        matches!(self.state, SessionState::Provisioned { .. })
    }
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn negotiation_accepts_the_recorded_set_and_refuses_the_rest() {
        assert_eq!(Subprotocol::parse("ocpp2.0.1"), Ok(Subprotocol::Ocpp201));
        assert_eq!(Subprotocol::parse("ocpp2.1"), Ok(Subprotocol::Ocpp21));
        // 1.6 is refused with the P17-E-1 reason attached.
        let error = Subprotocol::parse("ocpp1.6").expect_err("1.6 refused");
        match &error {
            NegotiationError::SubprotocolOutOfScope { offered, reason } => {
                assert_eq!(offered, "ocpp1.6");
                assert!(reason.contains("P17-E-1"));
            }
            other => panic!("expected a scope refusal, got {other:?}"),
        }
        // Everything else is refused, named.
        assert!(Subprotocol::parse("ocpp2.0").is_err());
        // The empty offer is a named refusal (no unspecified default).
        assert_eq!(negotiate(&[]), Err(NegotiationError::NoSubprotocolOffered));
    }

    #[test]
    fn negotiation_selects_from_the_offered_order_and_fails_whole() {
        // Peer preference order is honoured (never reordered).
        let negotiated =
            negotiate(&["ocpp2.1".to_string(), "ocpp2.0.1".to_string()]).expect("negotiated");
        assert_eq!(negotiated, Subprotocol::Ocpp21);
        let negotiated =
            negotiate(&["ocpp2.0.1".to_string(), "ocpp2.1".to_string()]).expect("negotiated");
        assert_eq!(negotiated, Subprotocol::Ocpp201);
        // An offer with 1.6 among others: 1.6 is skipped, a supported one
        // is selected.
        let negotiated = negotiate(&[
            "ocpp1.6".to_string(),
            "ocpp2.0.1".to_string(),
        ])
        .expect("negotiated");
        assert_eq!(negotiated, Subprotocol::Ocpp201);
        // No acceptable offer: the negotiation fails WHOLE, named.
        let error = negotiate(&["ocpp1.6".to_string()])
            .expect_err("unsupported set refused");
        assert!(error.to_string().contains("ocpp1.6"));
    }

    #[test]
    fn session_walks_negotiation_then_provisioning() {
        let mut session = Session::new();
        assert_eq!(session.state().name(), "AWAITING_SUBPROTOCOL");
        // Frames are refused before negotiation.
        assert!(!session.accepts_frames());

        session
            .apply_negotiation(Subprotocol::Ocpp201)
            .expect("negotiated");
        assert!(session.accepts_frames());
        assert!(!session.is_provisioned());

        // A boot before negotiation is refused; after negotiation it
        // provisions.
        session
            .apply_boot_accepted(ChargePointId::new("depot-cp-001".to_string()).expect("valid"))
            .expect("provisioned");
        assert!(session.is_provisioned());
        match session.state() {
            SessionState::Provisioned {
                subprotocol,
                charge_point_id,
            } => {
                assert_eq!(*subprotocol, Subprotocol::Ocpp201);
                assert_eq!(charge_point_id.as_str(), "depot-cp-001");
            }
            other => panic!("expected provisioned, got {other:?}"),
        }
    }

    #[test]
    fn session_refuses_illegal_transitions() {
        // Boot before negotiation.
        let mut session = Session::new();
        let error = session
            .apply_boot_accepted(ChargePointId::new("cp".to_string()).expect("valid"))
            .expect_err("boot before negotiation refused");
        assert!(matches!(error, SessionError::BootNotPermitted { .. }));

        // Double provisioning is refused (re-boot goes through the explicit
        // policy path, never a silent replacement).
        session.apply_negotiation(Subprotocol::Ocpp21).expect("negotiated");
        session
            .apply_boot_accepted(ChargePointId::new("cp-1".to_string()).expect("valid"))
            .expect("provisioned");
        let error = session
            .apply_boot_accepted(ChargePointId::new("cp-2".to_string()).expect("valid"))
            .expect_err("double provisioning refused");
        assert!(matches!(error, SessionError::AlreadyProvisioned { .. }));

        // An ended session refuses everything.
        let mut ended = Session::new();
        ended.end(SessionEndReason::TransportClosed {
            reason: "peer close".to_string(),
        });
        assert!(ended.apply_negotiation(Subprotocol::Ocpp201).is_err());
        assert!(ended
            .apply_boot_accepted(ChargePointId::new("cp".to_string()).expect("valid"))
            .is_err());
        match ended.state() {
            SessionState::Ended { reason } => {
                assert!(reason.to_string().contains("peer close"));
            }
            other => panic!("expected ended, got {other:?}"),
        }
    }

    #[test]
    fn negotiation_failure_and_boot_rejection_end_the_session_named() {
        let mut session = Session::new();
        session.end(SessionEndReason::NegotiationRefused {
            offered: vec!["ocpp1.6".to_string()],
        });
        match session.state() {
            SessionState::Ended { reason } => {
                assert!(reason.to_string().contains("ocpp1.6"));
            }
            other => panic!("expected ended, got {other:?}"),
        }
        let mut session = Session::new();
        session.apply_negotiation(Subprotocol::Ocpp201).expect("negotiated");
        session.end(SessionEndReason::BootRejected {
            charge_point: "cp-9".to_string(),
        });
        assert_eq!(session.state().name(), "ENDED");
        assert!(!session.accepts_frames());
    }
}
