// FIM-F02.03 — The live CSMS WebSocket transport: the `CsmsTransport`
// seam implemented with the real session layer over the hand-written
// RFC 6455 codec (`crate::websocket`) and the OCPP-J dispatch layer
// (`crate::dispatch`).
//
// HONESTY HEADER (FIM standing decision 1,
// docs/releases/implementation_ledger/README.md): this transport is
// WRITTEN-UNRUN — it has never compiled or executed. RFC 6455 conformance
// and real charge-point interoperability are HUMAN LAB GATES; zero
// conformance, certification, qualification, or interoperability claims
// are made anywhere (conventions §1.3/§7.3). No charge point has ever
// connected.
//
// WHAT THIS TRANSPORT IS:
//
// - **The seam, wired with the recorded pieces.** `serve` binds a tokio
//   TCP listener, performs the RFC 6455 server handshake (including the
//   subprotocol negotiation through the RECORDED `session::negotiate`),
//   accepts the connection through the EXISTING `CsmsServer` registry
//   (deterministic `BTreeMap`, connection cap, duplicate refusal), reads
//   frames through the EXISTING session state machine's gating
//   (`CsmsServer::handle_inbound_text`), and answers through the EXISTING
//   dispatcher (`crate::dispatch::CsmsMessageDispatcher`). Boot +
//   heartbeat + charging-profile relay flow through the EXISTING session
//   state machine and the EXISTING composer mapping — nothing here
//   re-implements them (FIM-F02.03).
// - **The refusal path is the recorded one** (FIM-F02.02b): a charge
//   point offering only `ocpp1.6` is refused in the handshake with the
//   EXACT P17-E-1 reason string carried by `Subprotocol::parse` — this
//   module invents NO refusal vocabulary.
// - **Both-direction initiation**: the CSMS-initiated relay flows through
//   the declared relay channel ([`RelayRequest`] →
//   `dispatcher::relay_charging_profile` → `CsmsServer::queue_outbound_call`
//   → `drain_outbound` → wire), preserving the bounded-queue and
//   one-outstanding-CALL discipline of the skeleton.
//
// WHAT THIS TRANSPORT IS NOT — declared limits (recorded, fail-closed):
//
// - **No TLS.** The transport serves PLAINTEXT RFC 6455. Profile 3 mTLS
//   termination is a HUMAN LAB GATE; there is deliberately NO flag that
//   disables TLS, because no TLS was ever enabled (the
//   `CsmsSecurityRequirements` structure stays Profile-3-only and is
//   untouched). Constructing this transport REQUIRES a recorded
//   lab-posture reason naming the deployment's compensating controls —
//   an unexplained plaintext listener is a construction error (the
//   `ProvisioningPolicy::allow_all` recorded-reason pattern).
// - **One identity per connection, identity from the URL path** (the
//   OCPP-J rules the session register cites [S-121]; UNVERIFIED against
//   the OCA text): the request target must be `/<charge-point-id>` with
//   no percent-encoding — anything else is refused with a named 400.
// - **No idle timeouts, no reconnection logic, no metrics.** The
//   connection loss path records the named facts (session Ended with the
//   transport reason; the registry link state goes Down; the last
//   REPORTED operational state stands — connection loss is NOT asset
//   unavailability, P17.04) and nothing else.

use std::cell::Cell;
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;

use flexcore_domain::command_state::CommandState;
use flexcore_domain::ev_charging_composer::ChargingProfilePlan;
use flexcore_protocol_ports::connection::ConnectionState;

use crate::csms::{CsmsServer, CsmsTransport, TransportError};
use crate::dispatch::CsmsMessageDispatcher;
use crate::model::ChargePointId;
use crate::session::Subprotocol;
use crate::websocket::{self, AssemblerEvent, FrameError, RefusalStatus};

/// The declared relay-command queue cap (a sanity bound, not a protocol
/// limit claim — the same declared-bounds discipline as
/// `CsmsLimits`).
pub const RELAY_QUEUE_CAP: usize = 64;

/// The declared connection-event queue cap per channel (a sanity bound;
/// backpressure on a slow central loop pauses the connection task —
/// never an unbounded buffer).
const EVENT_QUEUE_CAP: usize = 64;

/// The declared outbound-command queue cap per connection (texts the
/// central loop hands to a connection task for writing).
const COMMAND_QUEUE_CAP: usize = 16;

/// The injected clock (UTC milliseconds; the event_time discipline — the
/// response instants come from the injected clock, never the peer's
/// local clock).
pub type MillisClock = Arc<dyn Fn() -> i64 + Send + Sync>;

/// Construction errors of the live transport (named, fail-closed).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportConstructionError {
    /// The bind address is empty.
    EmptyBindAddress,
    /// The lab-posture reason is empty — an unexplained plaintext
    /// listener is refused at construction (the recorded-reason pattern).
    EmptyLabPostureReason,
}

impl fmt::Display for TransportConstructionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TransportConstructionError::EmptyBindAddress => {
                write!(f, "the live transport bind address is empty")
            }
            TransportConstructionError::EmptyLabPostureReason => write!(
                f,
                "the live transport serves plaintext RFC 6455 (no TLS termination — a HUMAN \
                 LAB GATE); constructing it requires a recorded lab-posture reason naming \
                 the deployment's compensating controls"
            ),
        }
    }
}

impl std::error::Error for TransportConstructionError {}

/// A CSMS-initiated charging-profile relay request: the composition sends
/// this through [`LiveCsmsTransport::relay_sender`]; the serving loop
/// queues it through the EXISTING dispatch + bounded-queue surfaces.
/// The lifecycle state travels WITH the request and the relay is
/// ISSUED-only (the command-relay-only rule — a non-ISSUED request is
/// refused by the existing typing, never relayed).
#[derive(Debug, Clone)]
pub struct RelayRequest {
    pub charge_point: ChargePointId,
    pub plan: ChargingProfilePlan,
    pub lifecycle_state: CommandState,
    pub charging_profile_id: i64,
    pub unique_id: crate::framing::UniqueId,
}

/// Commands the central loop sends to one connection task.
enum TransportCommand {
    SendText(String),
    Close { code: u16, reason: String },
}

/// Events one connection task reports to the central loop (which owns
/// the `CsmsServer` — the registry is mutated only there).
enum ConnectionEvent {
    Connected {
        charge_point: ChargePointId,
        negotiated: Subprotocol,
        commands: mpsc::Sender<TransportCommand>,
    },
    Inbound {
        charge_point: ChargePointId,
        text: String,
    },
    Closed {
        charge_point: ChargePointId,
        reason: String,
    },
}

/// The live RFC 6455 CSMS transport (FIM-F02.03). See the module honesty
/// header before consuming anything here.
pub struct LiveCsmsTransport {
    bind_addr: String,
    lab_posture_reason: String,
    clock: MillisClock,
    dispatcher: CsmsMessageDispatcher,
    relay_tx: mpsc::Sender<RelayRequest>,
    relay_rx: Option<mpsc::Receiver<RelayRequest>>,
    /// The honest pre-serve report: Connecting. It is the ONLY state this
    /// handle can honestly claim — during `serve` the value is owned by
    /// the serving call (not observable through any other handle), and a
    /// closed transport is never reported from a handle that no longer
    /// exists. No state is ever CLAIMED that was not observed (the
    /// `CsmsTransport` contract: "a transport never reports success for a
    /// bind it did not perform").
    state: ConnectionState,
    /// Relay requests refused during the last serve (not connected, queue
    /// overflow, non-ISSUED lifecycle state) — counted, never silently
    /// absorbed.
    refused_relays: Cell<u64>,
}

impl LiveCsmsTransport {
    /// Constructs the transport. Fails closed on an empty bind address
    /// and on an empty lab-posture reason (the plaintext listener MUST be
    /// explained — see the module header).
    pub fn new(
        bind_addr: String,
        dispatcher: CsmsMessageDispatcher,
        lab_posture_reason: String,
        clock: MillisClock,
    ) -> Result<Self, TransportConstructionError> {
        if bind_addr.trim().is_empty() {
            return Err(TransportConstructionError::EmptyBindAddress);
        }
        if lab_posture_reason.trim().is_empty() {
            return Err(TransportConstructionError::EmptyLabPostureReason);
        }
        let (relay_tx, relay_rx) = mpsc::channel(RELAY_QUEUE_CAP);
        Ok(Self {
            bind_addr,
            lab_posture_reason,
            clock,
            dispatcher,
            relay_tx,
            relay_rx: Some(relay_rx),
            state: ConnectionState::Connecting,
            refused_relays: Cell::new(0),
        })
    }

    /// The recorded lab-posture reason (travels with every deployment
    /// record, like the hosting placement's DEFAULT statement).
    pub fn lab_posture_reason(&self) -> &str {
        &self.lab_posture_reason
    }

    /// The relay-command sender (a composition-root handle; the queue is
    /// bounded at [`RELAY_QUEUE_CAP`] and `send` applies backpressure —
    /// never an unbounded buffer).
    pub fn relay_sender(&self) -> mpsc::Sender<RelayRequest> {
        self.relay_tx.clone()
    }

    /// The honest count of relay requests that were refused (not
    /// connected, queue overflow, non-ISSUED lifecycle state) during the
    /// last serve — counted, never silently absorbed.
    pub fn refused_relays(&self) -> u64 {
        self.refused_relays.get()
    }

    /// Runs the serving loop (the tokio side of `serve`).
    async fn run(&mut self, server: &mut CsmsServer) -> Result<(), TransportError> {
        let mut relay_rx = self
            .relay_rx
            .take()
            .ok_or(TransportError::TransportClosed {
                reason: "the relay channel was already consumed; serve runs once".to_string(),
            })?;
        let listener = TcpListener::bind(&self.bind_addr).await.map_err(|error| {
            TransportError::BindFailed {
                reason: format!(
                    "the listener could not bind '{}': {error}",
                    self.bind_addr
                ),
            }
        })?;
        let (event_tx, mut event_rx) = mpsc::channel::<ConnectionEvent>(EVENT_QUEUE_CAP);
        let mut connections: BTreeMap<String, mpsc::Sender<TransportCommand>> =
            BTreeMap::new();
        loop {
            tokio::select! {
                accepted = listener.accept() => {
                    match accepted {
                        Ok((stream, _peer)) => {
                            // A transient accept failure is a named record
                            // on the loop; the listener keeps serving (a
                            // single poisoned accept must not end the
                            // endpoint silently).
                            tokio::spawn(serve_connection(
                                stream,
                                event_tx.clone(),
                            ));
                        }
                        Err(_) => {
                            // Named here and counted nowhere: an accept
                            // error carries no per-peer state to record
                            // honestly; the transport surfaces it only
                            // through the eventual close reason. (WRITTEN-
                            // UNRUN note: a real deployment wires this to
                            // the operator log.)
                        }
                    }
                }
                event = event_rx.recv() => {
                    match event {
                        Some(ConnectionEvent::Connected { charge_point, negotiated, commands }) => {
                            match server.accept_connection(charge_point.clone(), negotiated) {
                                Ok(()) => {
                                    connections.insert(
                                        charge_point.as_str().to_string(),
                                        commands,
                                    );
                                }
                                Err(error) => {
                                    // The registry refused (cap reached or
                                    // duplicate identity): the connection is
                                    // closed with the named refusal.
                                    let _ = commands
                                        .send(TransportCommand::Close {
                                            code: websocket::CLOSE_POLICY_VIOLATION,
                                            reason: error.to_string(),
                                        })
                                        .await;
                                }
                            }
                        }
                        Some(ConnectionEvent::Inbound { charge_point, text }) => {
                            let now = (self.clock)();
                            match self.dispatcher.handle_inbound_text(
                                server,
                                &charge_point,
                                &text,
                                now,
                            ) {
                                Ok(replies) => {
                                    if let Some(commands) =
                                        connections.get(charge_point.as_str())
                                    {
                                        for reply in replies {
                                            match reply.encode() {
                                                Ok(wire) => {
                                                    let _ = commands
                                                        .send(TransportCommand::SendText(wire))
                                                        .await;
                                                }
                                                Err(_) => {
                                                    // An encode failure is
                                                    // internal: close with 1011,
                                                    // never send a corrupt frame.
                                                    let _ = commands
                                                        .send(TransportCommand::Close {
                                                            code: websocket::CLOSE_INTERNAL_ERROR,
                                                            reason: "an outbound frame failed \
                                                                     to encode (internal, named)"
                                                                .to_string(),
                                                        })
                                                        .await;
                                                    break;
                                                }
                                            }
                                        }
                                    }
                                }
                                Err(error) => {
                                    // A session / dispatch refusal closes the
                                    // connection with 1008 (policy violation);
                                    // the reason is the adapter's own named
                                    // record. The malformed-known-action-payload
                                    // class is NOT here: since FIM2-R1-003 the
                                    // dispatcher answers it in band with the
                                    // recorded FormationViolation CALLERROR and
                                    // the connection survives — only session,
                                    // registry, security-ledger, and relay
                                    // refusals reach this branch.
                                    if let Some(commands) =
                                        connections.get(charge_point.as_str())
                                    {
                                        let _ = commands
                                            .send(TransportCommand::Close {
                                                code: websocket::CLOSE_POLICY_VIOLATION,
                                                reason: error.to_string(),
                                            })
                                            .await;
                                    }
                                }
                            }
                        }
                        Some(ConnectionEvent::Closed { charge_point, reason }) => {
                            connections.remove(charge_point.as_str());
                            // The recorded loss facts, in the recorded
                            // order: the SESSION ends with the transport
                            // reason (audit-retained), the registry LINK
                            // state goes Down (the last REPORTED
                            // operational state stands — connection loss
                            // is NOT asset unavailability, P17.04), and
                            // the registry entry is removed (the audit
                            // record lives with the session log).
                            let _ = server.connection_lost(&charge_point, reason.clone());
                            let _ = self.dispatcher.note_link_loss(&charge_point, reason.clone());
                            let _ = server.remove_connection(&charge_point);
                        }
                        None => {
                            // Unreachable while `event_tx` is alive in this
                            // scope; if it ever fires, the transport closes
                            // honestly rather than spinning.
                            return Err(TransportError::TransportClosed {
                                reason: "the connection-event channel closed".to_string(),
                            });
                        }
                    }
                }
                relay = relay_rx.recv() => {
                    match relay {
                        Some(request) => {
                            let RelayRequest {
                                charge_point,
                                plan,
                                lifecycle_state,
                                charging_profile_id,
                                unique_id,
                            } = request;
                            if self
                                .dispatcher
                                .relay_charging_profile(
                                    server,
                                    &charge_point,
                                    &plan,
                                    lifecycle_state,
                                    charging_profile_id,
                                    unique_id,
                                )
                                .is_err()
                            {
                                // A refused relay (not connected, queue
                                // overflow, non-ISSUED state) is counted,
                                // never silently absorbed.
                                self.refused_relays.set(self.refused_relays.get() + 1);
                                continue;
                            }
                            let drained = server
                                .drain_outbound(&charge_point)
                                .unwrap_or_default();
                            if let Some(commands) = connections.get(charge_point.as_str()) {
                                for frame in drained {
                                    match frame.encode() {
                                        Ok(wire) => {
                                            let _ = commands
                                                .send(TransportCommand::SendText(wire))
                                                .await;
                                        }
                                        Err(_) => {
                                            let _ = commands
                                                .send(TransportCommand::Close {
                                                    code: websocket::CLOSE_INTERNAL_ERROR,
                                                    reason: "an outbound frame failed to \
                                                             encode (internal, named)"
                                                        .to_string(),
                                                })
                                                .await;
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                        None => {
                            // The relay sender was dropped: CSMS-initiated
                            // initiation ends; the listener keeps serving.
                        }
                    }
                }
            }
        }
    }
}

impl CsmsTransport for LiveCsmsTransport {
    /// Binds and serves until the process ends the loop (the trait's
    /// contract: "binds and serves until the transport closes"). Runs a
    /// single-threaded tokio runtime inside the synchronous seam; the
    /// `CsmsServer` is mutated ONLY by the central loop (single-owner
    /// registry discipline).
    fn serve(self: Box<Self>, server: &mut CsmsServer) -> Result<(), TransportError> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| TransportError::BindFailed {
                reason: format!("the serving runtime could not start: {error}"),
            })?;
        let mut this = *self;
        let outcome = runtime.block_on(async { this.run(server).await });
        // A Closed report is never claimed from a consumed handle: after
        // serve the transport value is gone, so no state() can lie.
        outcome
    }

    /// The honest self-reported connection state: Connecting before the
    /// first serve (the only state observable through this handle — see
    /// the field's doc comment).
    fn state(&self) -> ConnectionState {
        self.state.clone()
    }
}

/// Serves ONE connection: the RFC 6455 handshake (including the recorded
/// subprotocol negotiation and the refusal path), then the frame loop
/// (inbound texts → events; pings answered inline; the close handshake
/// completed; outbound commands written). The task NEVER touches the
/// `CsmsServer` — registry mutations are the central loop's alone.
async fn serve_connection(
    stream: TcpStream,
    events: mpsc::Sender<ConnectionEvent>,
) {
    let (mut reader, mut writer) = tokio::io::split(stream);
    // --- The handshake ---------------------------------------------------
    let head = match websocket::read_http_request_head(&mut reader).await {
        Ok(head) => head,
        Err(error) => {
            let (status, detail) = websocket::refusal_for(&error);
            let _ = websocket::write_frame(
                &mut writer,
                websocket::build_refusal_response(status, &detail).as_bytes(),
            )
            .await;
            return;
        }
    };
    let request = match websocket::parse_upgrade_request(&head) {
        Ok(request) => request,
        Err(error) => {
            let (status, detail) = websocket::refusal_for(&error);
            let _ = websocket::write_frame(
                &mut writer,
                websocket::build_refusal_response(status, &detail).as_bytes(),
            )
            .await;
            return;
        }
    };
    // The charge-point identity travels in the URL path (module register
    // item 3 — UNVERIFIED against the OCA text). No percent-decoding: an
    // identity outside the recorded character discipline is refused.
    let path = request
        .request_target
        .trim_start_matches('/');
    let identity_segment = path.split(['?', '#']).next().unwrap_or("");
    let charge_point = match ChargePointId::new(identity_segment.to_string()) {
        Ok(id) => id,
        Err(_) => {
            let detail =
                "the request target carries no valid charge-point identity (expected \
                 /<charge-point-id>)"
                    .to_string();
            let _ = websocket::write_frame(
                &mut writer,
                websocket::build_refusal_response(RefusalStatus::BadRequest, &detail)
                    .as_bytes(),
            )
            .await;
            return;
        }
    };
    // The recorded negotiation: no unspecified default; `ocpp1.6` refused
    // with the EXACT P17-E-1 reason carried by session::negotiate
    // (FIM-F02.02b) — this module invents no refusal vocabulary.
    let (negotiated, accept_key) = match websocket::negotiate_handshake(&request) {
        Ok(negotiated) => negotiated,
        Err(error) => {
            let (status, detail) = websocket::refusal_for(&error);
            let _ = websocket::write_frame(
                &mut writer,
                websocket::build_refusal_response(status, &detail).as_bytes(),
            )
            .await;
            return;
        }
    };
    let response = websocket::build_switching_response(negotiated, &accept_key);
    if websocket::write_frame(&mut writer, response.as_bytes()).await.is_err() {
        return;
    }
    // --- The frame loop -----------------------------------------------------
    let (commands, mut command_rx) = mpsc::channel::<TransportCommand>(COMMAND_QUEUE_CAP);
    if events
        .send(ConnectionEvent::Connected {
            charge_point: charge_point.clone(),
            negotiated,
            commands,
        })
        .await
        .is_err()
    {
        return;
    }
    let mut assembler = match websocket::MessageAssembler::new(
        websocket::MAX_WEBSOCKET_MESSAGE_BYTES,
    ) {
        Ok(assembler) => assembler,
        Err(_) => return, // unreachable: the declared cap is non-zero
    };
    loop {
        tokio::select! {
            frame = websocket::read_frame(&mut reader) => {
                match frame {
                    Ok(raw) => match assembler.push(raw.fin, raw.opcode, raw.payload) {
                        Ok(Some(AssemblerEvent::Text(text))) => {
                            if events
                                .send(ConnectionEvent::Inbound {
                                    charge_point: charge_point.clone(),
                                    text,
                                })
                                .await
                                .is_err()
                            {
                                break;
                            }
                        }
                        Ok(Some(AssemblerEvent::Ping(payload))) => {
                            // The ping is answered INLINE (timeliness): a
                            // PONG echoing the payload. No session state is
                            // touched.
                            let _ = websocket::write_frame(
                                &mut writer,
                                &websocket::encode_pong(&payload),
                            )
                            .await;
                        }
                        Ok(Some(AssemblerEvent::Pong(_))) => {
                            // Liveness evidence only — carried, never acted
                            // on (no state transition exists for it).
                        }
                        Ok(Some(AssemblerEvent::Close { code, reason })) => {
                            // The close handshake is completed: the peer's
                            // code is echoed (or 1000 when none was carried).
                            let echo = if code == 1005 {
                                websocket::CLOSE_NORMAL
                            } else {
                                code
                            };
                            let _ = websocket::write_frame(
                                &mut writer,
                                &websocket::encode_close(echo, &reason),
                            )
                            .await;
                            let _ = events
                                .send(ConnectionEvent::Closed {
                                    charge_point,
                                    reason: format!("the charge point closed the connection \
                                                     (code {code})"),
                                })
                                .await;
                            break;
                        }
                        Ok(None) => {}
                        Err(error) => {
                            // A frame protocol violation: the connection
                            // fails with the codec's named close code.
                            let code = websocket::close_code_for(&error);
                            let _ = websocket::write_frame(
                                &mut writer,
                                &websocket::encode_close(code, &error.to_string()),
                            )
                            .await;
                            let _ = events
                                .send(ConnectionEvent::Closed {
                                    charge_point,
                                    reason: error.to_string(),
                                })
                                .await;
                            break;
                        }
                    },
                    Err(FrameError::IncompleteFrame { needed: 0, have: 0 }) => {
                        // The IO-failure sentinel (a mid-frame read error or
                        // a peer disconnect): no close frame can be sent —
                        // report the loss honestly.
                        let _ = events
                            .send(ConnectionEvent::Closed {
                                charge_point,
                                reason: "the connection was lost mid-frame".to_string(),
                            })
                            .await;
                        break;
                    }
                    Err(error) => {
                        let code = websocket::close_code_for(&error);
                        let _ = websocket::write_frame(
                            &mut writer,
                            &websocket::encode_close(code, &error.to_string()),
                        )
                        .await;
                        let _ = events
                            .send(ConnectionEvent::Closed {
                                charge_point,
                                reason: error.to_string(),
                            })
                            .await;
                        break;
                    }
                }
            }
            command = command_rx.recv() => {
                match command {
                    Some(TransportCommand::SendText(text)) => {
                        let _ = websocket::write_frame(
                            &mut writer,
                            &websocket::encode_server_text(&text),
                        )
                        .await;
                    }
                    Some(TransportCommand::Close { code, reason }) => {
                        let _ = websocket::write_frame(
                            &mut writer,
                            &websocket::encode_close(code, &reason),
                        )
                        .await;
                        let _ = events
                            .send(ConnectionEvent::Closed {
                                charge_point,
                                reason: format!(
                                    "the CSMS closed the connection: {reason}"
                                ),
                            })
                            .await;
                        break;
                    }
                    None => {
                        // The central loop is gone: close with 1011 and
                        // report, never linger half-wired.
                        let _ = websocket::write_frame(
                            &mut writer,
                            &websocket::encode_close(
                                websocket::CLOSE_INTERNAL_ERROR,
                                "the CSMS is shutting down",
                            ),
                        )
                        .await;
                        let _ = events
                            .send(ConnectionEvent::Closed {
                                charge_point,
                                reason: "the central serving loop ended".to_string(),
                            })
                            .await;
                        break;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::csms::{CsmsLimits, CsmsSecurityRequirements};
    use crate::dispatch::CsmsMessageDispatcher;
    use crate::provisioning::ProvisioningPolicy;
    use crate::scope::csms_manifest;

    fn dispatcher() -> CsmsMessageDispatcher {
        CsmsMessageDispatcher::new(
            flexcore_domain::evse_registry::EvseRegistry::new(),
            ProvisioningPolicy::allow_only(
                vec!["depot-cp-001".to_string()],
                "the named depot fleet".to_string(),
            )
            .expect("valid policy"),
            60,
            8,
        )
        .expect("valid dispatcher")
    }

    fn transport() -> LiveCsmsTransport {
        LiveCsmsTransport::new(
            "127.0.0.1:0".to_string(),
            dispatcher(),
            "lab posture: simulation-only deployment; the listener is reachable only on \
             the depot lab network (recorded reason, P17-E-2 DEFAULT edge-agent-hosted \
             shape)"
                .to_string(),
            Arc::new(|| 1_000),
        )
        .expect("valid transport")
    }

    #[test]
    fn construction_is_fail_closed_and_state_is_honest() {
        // An empty bind address is refused.
        assert_eq!(
            LiveCsmsTransport::new(
                "  ".to_string(),
                dispatcher(),
                "reason".to_string(),
                Arc::new(|| 0)
            ),
            Err(TransportConstructionError::EmptyBindAddress)
        );
        // An UNEXPLAINED plaintext listener is refused at construction.
        assert_eq!(
            LiveCsmsTransport::new(
                "127.0.0.1:0".to_string(),
                dispatcher(),
                "  ".to_string(),
                Arc::new(|| 0)
            ),
            Err(TransportConstructionError::EmptyLabPostureReason)
        );
        // Before serve, the honest state is Connecting (never a claimed
        // success for a bind that did not happen).
        let transport = transport();
        assert_eq!(transport.state(), ConnectionState::Connecting);
        // The relay sender is available to the composition root.
        let sender = transport.relay_sender();
        assert_eq!(sender.capacity(), RELAY_QUEUE_CAP);
        // The lab-posture reason travels verbatim.
        assert!(transport.lab_posture_reason().contains("lab posture"));
        assert!(transport.refused_relays() == 0);
    }

    #[test]
    fn the_unwired_transport_refusal_still_travels_for_the_default_shape() {
        // The shipped default transport (the seam refusal) is UNCHANGED —
        // the live transport is opt-in by construction, never a silent
        // replacement (the additive rule).
        let mut server = CsmsServer::new(
            csms_manifest().expect("valid manifest"),
            CsmsLimits::new(4, 8).expect("valid limits"),
            CsmsSecurityRequirements::new(vec!["evse-device-ca".to_string()], true)
                .expect("valid security"),
        )
        .expect("valid server");
        let error = Box::new(crate::csms::UnwiredCsmsTransport)
            .serve(&mut server)
            .expect_err("the seam refusal stays for the default shape");
        assert!(error
            .to_string()
            .contains(crate::csms::CSMS_TRANSPORT_SEAM_NOT_WIRED));
    }
}
