// FIM-F02.02 / FIM-F02.02b / FIM-F02.03 — The OCPP-J session layer over
// the WebSocket codec: CALL / CALLRESULT / CALLERROR dispatch with id
// correlation, bounded queues, and the boot / heartbeat / charging-profile
// relay flows through the EXISTING CSMS surfaces.
//
// HONESTY HEADER (FIM standing decision 1): this layer is WRITTEN-UNRUN —
// it has never compiled or executed. RFC 6455 conformance and real
// charge-point interoperability are HUMAN LAB GATES; zero conformance,
// certification, qualification, or interoperability claims are made
// anywhere (conventions §1.3/§7.3).
//
// WHAT FLOWS WHERE (and what does NOT change):
//
// - **The framing codec is unchanged** (`crate::framing`): every wire
//   text is decoded by `CsmsServer::handle_inbound_text` through
//   `OcppFrame::decode`, and every CSMS-initiated CALL is queued through
//   `CsmsServer::queue_outbound_call` into the EXISTING bounded outbound
//   queue with the EXISTING one-outstanding-CALL tracker. This module
//   adds NO queue of its own — boundedness stays the skeleton's declared
//   caps, and overflow refuses with the recorded
//   `CsmsError::OutboundQueueFull` (never a silent drop).
// - **Id correlation.** The CALLRESULT/CALLERROR matching for
//   CSMS-initiated CALLs is the skeleton's outbound `CallTracker` (an
//   unmatched reply is refused WHOLE — the recorded rule). This module
//   additionally mirrors the ACTION of the CSMS-initiated CALL it queued
//   (`SetChargingProfile` relay) so the station's reply is recorded
//   against the honesty rule (`charging::record_accepted_status`): an
//   Accepted is STORAGE evidence, never application evidence. The mirror
//   is a record, never a second tracker — it can never match a reply the
//   skeleton's tracker refused.
// - **The session state machine is unchanged** (`crate::session`): the
//   subprotocol is negotiated by the transport seam through the recorded
//   `session::negotiate` (a charge point offering only `ocpp1.6` is
//   refused with the EXACT P17-E-1 reason string — FIM-F02.02b reuses
//   `Subprotocol::parse`'s recorded vocabulary verbatim; no new refusal
//   vocabulary is invented anywhere in this module), and the boot
//   outcome enters through the session machine's existing transitions
//   (`Negotiated → Provisioned` on an ACCEPTED boot; `Ended{BootRejected}`
//   on a REJECTED one), recorded by the ADDITIVE `CsmsServer::
//   mark_boot_accepted` / `mark_boot_rejected` surfaces.
// - **Charging profiles are commands** (the frozen invariant): the relay
//   payload is built ONLY through `charging::into_relay_request`, which
//   is typed to the ISSUED lifecycle state — every other state is
//   refused with the found state named. This module never constructs a
//   lifecycle state from external input.
//
// Module-level UNVERIFIED-ASSUMPTION register (the phase-17 plan §7.5
// discipline; the OCA OCPP-J transport part is the implementation-time
// check):
//
// 1. The empty-response shape `{}` for StatusNotification /
//    TransactionEvent / SecurityEventNotification replies (the 2.0.1
//    EmptyRequestBody-class responses) — UNVERIFIED spelling.
// 2. A malformed payload for a KNOWN action is answered with the
//    recorded CALLERROR code `FormationViolation` (decode failure) rather
//    than tearing the connection — the decode path routes through
//    `malformed_payload_reply` (FIM2-R1-003 of the FIM final analysis;
//    this claim was FALSE in code before that fix — the reply builder was
//    dead and decode refusals tore the connection with 1008). The
//    `PropertyConstraintViolation` half of this item (a payload that
//    DECODES but fails model validation — the Provisioning refusals) is
//    still recorded-but-unrouted: those refusals keep their named
//    ProvisioningError path (the P17 provisioning records pin those
//    refusal strings); routing them in band is a reviewed change, not
//    taken here. Whether the OCA text mandates the exact codes is
//    UNVERIFIED (the vocabulary itself is the recorded one,
//    `crate::framing::CallErrorCode`).
// 3. The station-initiated certificate / composite-schedule CALLs
//    (SignCertificate, GetCompositeSchedule, ClearChargingProfile) are
//    answered `NotImplemented` here: their composition surfaces (the
//    P17.03 certificate ledgers, the P17.05 reconciliation) live with
//    the composition root, and the wire-level behaviour of a CSMS that
//    does not implement an action is an UNVERIFIED transport-seam check
//    (the UC-FW-01 note). No action was added to the closed vocabulary
//    for the sake of an answer.
// 4. The boot's `currentTime`/interval come from the injected clock and
//    the declared configuration — never from the peer's local clock
//    (the event_time discipline).

use flexcore_domain::command_state::CommandState;
use flexcore_domain::ev_charging_composer::ChargingProfilePlan;
use flexcore_domain::evse_registry::{EvseRegistry, TimestampPair};

use crate::charging::{into_relay_request, record_accepted_status};
use crate::csms::{CsmsError, CsmsServer};
use crate::framing::{Action, CallErrorCode, OcppFrame, UniqueId};
use crate::model::{
    BootNotificationRequest, BootNotificationResponse, BootStatus, ChargePointId,
    SecurityEventNotificationRequest, SetChargingProfileStatus, StatusNotificationRequest,
    TransactionEventRequest,
};
use crate::provisioning::{
    apply_accepted_boot, apply_status_notification, apply_transaction_event,
    map_accepted_boot, ProvisioningPolicy, ProvisioningError,
};
use crate::security::{SecurityEventLedger, SecurityEventRecord};

/// Dispatch errors. Every failure is named; nothing is silently repaired.
#[derive(Debug, Clone, PartialEq)]
pub enum DispatchError {
    /// The CSMS skeleton refused the step (re-raised unchanged — session
    /// gating, unmatched replies, queue overflow, connection cap).
    Csms(CsmsError),
    /// The payload did not decode into the recorded message shape.
    Payload { action: &'static str, reason: String },
    /// The provisioning / registry mapping refused (re-raised).
    Provisioning(ProvisioningError),
    /// The security-event ledger refused (named as the ledger's own
    /// message).
    Security(String),
    /// The charging relay surface refused (re-raised from
    /// `crate::charging`).
    Charging(crate::charging::ChargingError),
}

impl std::fmt::Display for DispatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DispatchError::Csms(error) => write!(f, "{error}"),
            DispatchError::Payload { action, reason } => {
                write!(f, "the {action} payload does not decode: {reason}")
            }
            DispatchError::Provisioning(error) => write!(f, "{error}"),
            DispatchError::Security(reason) => {
                write!(f, "the security-event ledger refused: {reason}")
            }
            DispatchError::Charging(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for DispatchError {}

impl From<CsmsError> for DispatchError {
    fn from(error: CsmsError) -> Self {
        DispatchError::Csms(error)
    }
}

impl From<ProvisioningError> for DispatchError {
    fn from(error: ProvisioningError) -> Self {
        DispatchError::Provisioning(error)
    }
}

impl From<crate::charging::ChargingError> for DispatchError {
    fn from(error: crate::charging::ChargingError) -> Self {
        DispatchError::Charging(error)
    }
}

/// The OCPP-J message dispatcher (FIM-F02.02): decodes inbound wire text
/// through the skeleton, answers charge-point CALLs, correlates replies
/// to CSMS-initiated CALLs, and relays ISSUED charging profiles. All
/// state is the composition's own (the registry, the boot policy, the
/// security-event ledger); the CSMS skeleton stays the connection
/// authority.
pub struct CsmsMessageDispatcher {
    registry: EvseRegistry,
    policy: ProvisioningPolicy,
    heartbeat_interval_seconds: u32,
    security_events: SecurityEventLedger,
    /// The action mirror of the CSMS-initiated CALL this dispatcher
    /// queued (record, never a second tracker — see the module header).
    outstanding_relay: Option<(UniqueId, Action)>,
    /// The stored-not-applied statement of the last recorded
    /// SetChargingProfile reply, when one was recorded.
    last_accepted_statement: Option<String>,
    /// Replies the skeleton matched but this dispatcher did not queue
    /// (CSMS-initiated CALLs queued by another composition surface).
    /// Counted honestly — never silently absorbed.
    untracked_replies: u64,
}

impl CsmsMessageDispatcher {
    /// Constructs the dispatcher. The security-event ledger cap must be
    /// non-zero (the ledger's own rule).
    pub fn new(
        registry: EvseRegistry,
        policy: ProvisioningPolicy,
        heartbeat_interval_seconds: u32,
        security_event_cap: usize,
    ) -> Result<Self, DispatchError> {
        let security_events = SecurityEventLedger::new(security_event_cap)
            .map_err(|error| DispatchError::Security(error.to_string()))?;
        Ok(Self {
            registry,
            policy,
            heartbeat_interval_seconds,
            security_events,
            outstanding_relay: None,
            last_accepted_statement: None,
            untracked_replies: 0,
        })
    }

    /// The boot policy (surfaced for the composition root's diagnostics).
    pub fn boot_policy(&self) -> &ProvisioningPolicy {
        &self.policy
    }

    /// The stored-not-applied statement of the last recorded
    /// SetChargingProfile reply, when one was recorded.
    pub fn last_accepted_statement(&self) -> Option<&str> {
        self.last_accepted_statement.as_deref()
    }

    /// The honest count of replies the skeleton matched against the
    /// outbound tracker but which this dispatcher had not queued (a
    /// CALL initiated by another composition surface). Counted, never
    /// silently absorbed.
    pub fn untracked_replies(&self) -> u64 {
        self.untracked_replies
    }

    /// The security-event ledger snapshot (oldest first).
    pub fn security_event_snapshot(&self) -> &[SecurityEventRecord] {
        self.security_events.snapshot()
    }

    /// Handles one inbound wire TEXT message from a connected charge
    /// point. Decodes through the skeleton (session gating + framing +
    /// tracker), answers charge-point CALLs with the reply frames to
    /// write (in order), and records station replies to CSMS-initiated
    /// CALLs (no wire reply — the correlation is the answer).
    pub fn handle_inbound_text(
        &mut self,
        server: &mut CsmsServer,
        charge_point: &ChargePointId,
        text: &str,
        now_millis: i64,
    ) -> Result<Vec<OcppFrame>, DispatchError> {
        let frame = server.handle_inbound_text(charge_point, text)?;
        match frame {
            OcppFrame::Call {
                unique_id,
                action,
                payload,
            } => self.answer_call(server, charge_point, unique_id, action, payload, now_millis),
            OcppFrame::CallResult { unique_id, payload } => {
                self.record_relay_reply(&unique_id, &payload);
                Ok(Vec::new())
            }
            OcppFrame::CallError {
                unique_id,
                error_code,
                error_description,
                ..
            } => {
                self.record_relay_failure(&unique_id, error_code, &error_description);
                Ok(Vec::new())
            }
        }
    }

    /// Relays a composed charging profile to a connected charge point:
    /// the payload is built ONLY through `charging::into_relay_request`
    /// (ISSUED-only, the command-relay-only rule) and queued through the
    /// skeleton's bounded queue with the one-outstanding-CALL tracker
    /// (overflow and a second outstanding CALL refuse with the recorded
    /// errors — nothing is silently dropped or replaced).
    pub fn relay_charging_profile(
        &mut self,
        server: &mut CsmsServer,
        charge_point: &ChargePointId,
        plan: &ChargingProfilePlan,
        lifecycle_state: CommandState,
        charging_profile_id: i64,
        unique_id: UniqueId,
    ) -> Result<(), DispatchError> {
        let request = into_relay_request(plan, lifecycle_state, charging_profile_id)?;
        let payload = serde_json::to_value(&request).map_err(|_| DispatchError::Payload {
            action: "SetChargingProfile",
            reason: "the validated payload failed to encode (internal, named, never retried \
                     silently)"
                .to_string(),
        })?;
        server.queue_outbound_call(
            charge_point,
            Action::SetChargingProfile,
            payload,
            unique_id.clone(),
        )?;
        self.outstanding_relay = Some((unique_id, Action::SetChargingProfile));
        Ok(())
    }

    /// Notes a transport-level link loss on the dispatcher's registry:
    /// ONLY the link state changes (the domain rule — connection loss is
    /// NOT asset unavailability; the rule statement travels back
    /// verbatim).
    pub fn note_link_loss(
        &mut self,
        charge_point: &ChargePointId,
        reason: String,
    ) -> Result<&'static str, DispatchError> {
        crate::provisioning::apply_link_loss(&mut self.registry, charge_point, reason)
            .map_err(DispatchError::from)
    }

    /// Converts the INBOUND-payload decode refusal into the register item 2
    /// in-band `FormationViolation` CALLERROR (FIM2-R1-003 of the FIM final
    /// analysis): the malformed payload for a known action is answered on
    /// the wire and the connection SURVIVES — the transport close path no
    /// longer sees this class. Every other dispatch error passes through.
    fn malformed_payload_reply(
        unique_id: UniqueId,
        error: DispatchError,
    ) -> Result<Vec<OcppFrame>, DispatchError> {
        match error {
            DispatchError::Payload { action, reason: _ } => {
                Ok(vec![Self::formation_violation_reply(unique_id, &action)])
            }
            other => Err(other),
        }
    }

    /// Answers one charge-point CALL.
    fn answer_call(
        &mut self,
        server: &mut CsmsServer,
        charge_point: &ChargePointId,
        unique_id: UniqueId,
        action: Action,
        payload: serde_json::Value,
        now_millis: i64,
    ) -> Result<Vec<OcppFrame>, DispatchError> {
        match action {
            Action::BootNotification => {
                let replies = self.answer_boot(server, charge_point, unique_id.clone(), payload, now_millis);
                match replies {
                    Ok(replies) => Ok(replies),
                    // The boot decode refusal is the same inbound-payload
                    // class (FIM2-R1-003): answered in band, never a tear-down.
                    Err(error @ DispatchError::Payload { .. }) => {
                        Self::malformed_payload_reply(unique_id, error)
                    }
                    Err(error) => Err(error),
                }
            }
            Action::Heartbeat => {
                // Liveness evidence ONLY: no registry call exists for
                // heartbeats (absence is structural — the UC-PROV-04
                // recorded behaviour). The response time is the injected
                // clock, never the peer's (register item 4).
                Ok(vec![OcppFrame::CallResult {
                    unique_id,
                    payload: serde_json::to_value(crate::model::HeartbeatResponse {
                        current_time_millis: now_millis,
                    })
                    .map_err(|_| DispatchError::Payload {
                        action: "Heartbeat",
                        reason: "the response failed to encode".to_string(),
                    })?,
                }])
            }
            Action::StatusNotification => {
                let request = match self.decode(&action, payload) {
                    Ok(request) => request,
                    Err(error) => return Self::malformed_payload_reply(unique_id, error),
                };
                apply_status_notification(&mut self.registry, charge_point, &request)?;
                Ok(vec![empty_result(unique_id)])
            }
            Action::TransactionEvent => {
                let request = match self.decode(&action, payload) {
                    Ok(request) => request,
                    Err(error) => return Self::malformed_payload_reply(unique_id, error),
                };
                apply_transaction_event(&mut self.registry, charge_point, &request, now_millis)?;
                Ok(vec![empty_result(unique_id)])
            }
            Action::SecurityEventNotification => {
                let request = match self.decode(&action, payload) {
                    Ok(request) => request,
                    Err(error) => return Self::malformed_payload_reply(unique_id, error),
                };
                self.security_events
                    .ingest(charge_point.as_str(), &request, now_millis)
                    .map_err(|error| DispatchError::Security(error.to_string()))?;
                Ok(vec![empty_result(unique_id)])
            }
            // The certificate and composite-schedule CALLs a station may
            // initiate: answered NotImplemented with the named reason
            // (register item 3) — the vocabulary is the recorded one, no
            // action was added, and the connection is NOT torn down.
            // Batch 68: `SetChargingProfile` belongs in this group too —
            // like `ClearChargingProfile` it is a charging-profile
            // composition-surface action this transport does not wire
            // (the match was non-exhaustive without it).
            Action::SignCertificate
            | Action::CertificateSigned
            | Action::InstallCertificate
            | Action::GetCompositeSchedule
            | Action::ClearChargingProfile
            | Action::SetChargingProfile => Ok(vec![OcppFrame::CallError {
                unique_id,
                error_code: CallErrorCode::NotImplemented,
                error_description: format!(
                    "the {} composition surface is not wired into this transport; the \
                     recorded closed vocabulary handles the action, the composition root \
                     owns the flow (P17.03/P17.05)",
                    action.name()
                ),
                error_details: serde_json::json!({}),
            }]),
        }
    }

    /// Answers a BootNotification through the EXISTING session state
    /// machine and the EXISTING provisioning mapping: the ACCEPT decision
    /// is the boot policy's (fail-closed DEFAULT accepts nothing unnamed);
    /// an ACCEPTED boot maps into the domain registry and walks the
    /// session Negotiated → Provisioned; a REJECTED boot ends the session
    /// with the recorded `BootRejected` reason (a retry is a NEW session).
    fn answer_boot(
        &mut self,
        server: &mut CsmsServer,
        charge_point: &ChargePointId,
        unique_id: UniqueId,
        payload: serde_json::Value,
        now_millis: i64,
    ) -> Result<Vec<OcppFrame>, DispatchError> {
        let request: BootNotificationRequest =
            serde_json::from_value(payload).map_err(|_| DispatchError::Payload {
                action: "BootNotification",
                reason: "the payload does not match the recorded BootNotification shape"
                    .to_string(),
            })?;
        let accepted = self.policy.decide(charge_point);
        if accepted {
            // Order (recorded here honestly): validate + map first (no
            // mutation), then the registry record, then the session
            // machine. A registry refusal surfaces as a named error and
            // leaves the session machine untouched — the station retries
            // with the connection still Negotiated.
            let record = map_accepted_boot(
                charge_point,
                &request,
                TimestampPair::new(now_millis, now_millis)
                    .map_err(ProvisioningError::from)?,
                self.heartbeat_interval_seconds,
            )?;
            apply_accepted_boot(&mut self.registry, record)?;
            server.mark_boot_accepted(charge_point)?;
        } else {
            server.mark_boot_rejected(charge_point)?;
        }
        let status = if accepted {
            BootStatus::Accepted
        } else {
            BootStatus::Rejected
        };
        let response = BootNotificationResponse {
            status,
            interval_seconds: self.heartbeat_interval_seconds,
            current_time_millis: now_millis,
        };
        let payload =
            serde_json::to_value(&response).map_err(|_| DispatchError::Payload {
                action: "BootNotification",
                reason: "the response failed to encode".to_string(),
            })?;
        Ok(vec![OcppFrame::CallResult { unique_id, payload }])
    }

    /// Records the station's reply to the CSMS-initiated CALL this
    /// dispatcher queued. The skeleton's tracker already refused
    /// unmatched replies before this runs; the mirror here exists to
    /// record the honesty rule against the reply.
    fn record_relay_reply(&mut self, unique_id: &UniqueId, payload: &serde_json::Value) {
        let tracked = self
            .outstanding_relay
            .as_ref()
            .is_some_and(|(id, _)| id == unique_id);
        if !tracked {
            // A CALL queued by another composition surface: counted,
            // never absorbed silently.
            self.untracked_replies += 1;
            return;
        }
        let (_, action) = self.outstanding_relay.as_ref().expect("checked above");
        if *action == Action::SetChargingProfile {
            match payload.get("status").and_then(|value| value.as_str()) {
                Some("Accepted") => {
                    let (statement, _) =
                        record_accepted_status(SetChargingProfileStatus::Accepted);
                    self.last_accepted_statement = Some(statement.to_string());
                }
                Some("Rejected") => {
                    let (statement, _) =
                        record_accepted_status(SetChargingProfileStatus::Rejected);
                    self.last_accepted_statement = Some(statement.to_string());
                }
                _ => {
                    // The reply status does not match the recorded
                    // vocabulary: named, recorded, never guessed.
                    self.last_accepted_statement = Some(format!(
                        "the SetChargingProfile reply status did not match the recorded \
                         Accepted/Rejected vocabulary: {payload}"
                    ));
                }
            }
        }
        self.outstanding_relay = None;
    }

    /// Records the station's CALLERROR for the CSMS-initiated CALL this
    /// dispatcher queued (a relay failure is a NAMED record surfaced to
    /// the operator — the adapter NEVER retries or re-issues on its own
    /// authority, the recorded charging rule).
    fn record_relay_failure(
        &mut self,
        unique_id: &UniqueId,
        error_code: CallErrorCode,
        error_description: &str,
    ) {
        let tracked = self
            .outstanding_relay
            .as_ref()
            .is_some_and(|(id, _)| id == unique_id);
        if !tracked {
            self.untracked_replies += 1;
            return;
        }
        self.last_accepted_statement = Some(format!(
            "the CSMS-initiated {} CALL failed with {} ({}); the record is surfaced, never \
             retried on the adapter's own authority",
            self.outstanding_relay
                .as_ref()
                .map(|(_, action)| action.name())
                .unwrap_or("CALL"),
            error_code.name(),
            error_description
        ));
        self.outstanding_relay = None;
    }

    /// Decodes a payload into the recorded message shape; a mismatch is
    /// answered with the recorded `FormationViolation` CALLERROR shape by
    /// the caller's reply path — here it is the named decode error.
    fn decode<T: serde::de::DeserializeOwned>(
        &self,
        action: &Action,
        payload: serde_json::Value,
    ) -> Result<T, DispatchError> {
        serde_json::from_value(payload).map_err(|_| DispatchError::Payload {
            action: action.name(),
            reason: "the payload does not match the recorded message shape".to_string(),
        })
    }

    /// Builds the FormationViolation CALLERROR for a payload that does
    /// not decode (register item 2 — UNVERIFIED code choice; the
    /// vocabulary is the recorded one). Takes the action NAME (the
    /// `DispatchError::Payload` field) — wired into the inbound decode
    /// path by FIM2-R1-003 of the FIM final analysis; previously dead.
    pub fn formation_violation_reply(
        unique_id: UniqueId,
        action_name: &str,
    ) -> OcppFrame {
        OcppFrame::CallError {
            unique_id,
            error_code: CallErrorCode::FormationViolation,
            error_description: format!(
                "the {action_name} payload does not match the recorded message shape"
            ),
            error_details: serde_json::json!({}),
        }
    }

    /// Builds the PropertyConstraintViolation CALLERROR for a payload
    /// that decodes but fails the recorded model validation (register
    /// item 2 — UNVERIFIED code choice).
    pub fn property_constraint_reply(
        unique_id: UniqueId,
        action: &Action,
        reason: &str,
    ) -> OcppFrame {
        OcppFrame::CallError {
            unique_id,
            error_code: CallErrorCode::PropertyConstraintViolation,
            error_description: format!(
                "the {} payload failed the recorded model validation: {reason}",
                action.name()
            ),
            error_details: serde_json::json!({}),
        }
    }
}

/// The recorded empty-response shape `{}` (register item 1 — UNVERIFIED).
fn empty_result(unique_id: UniqueId) -> OcppFrame {
    OcppFrame::CallResult {
        unique_id,
        payload: serde_json::json!({}),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::csms::{CsmsLimits, CsmsSecurityRequirements};
    use crate::model::HeartbeatResponse;
    use crate::scope::csms_manifest;
    use crate::session::Subprotocol;
    use flexcore_domain::command_state::CommandState;
    use flexcore_domain::ev_charging_composer::{
        compose_external_constraints_profile, DeviceRateConstraints, DispatchEnvelope,
        DispatchPeriod, RateUnit, SupplyContext,
    };
    use flexcore_domain::evse_registry::EvseRegistry;

    fn cp(name: &str) -> ChargePointId {
        ChargePointId::new(name.to_string()).expect("valid")
    }

    fn server() -> CsmsServer {
        CsmsServer::new(
            csms_manifest().expect("valid manifest"),
            CsmsLimits::new(4, 8).expect("valid limits"),
            CsmsSecurityRequirements::new(vec!["evse-device-ca".to_string()], true)
                .expect("valid security"),
        )
        .expect("valid server")
    }

    fn dispatcher_allow(name: &str) -> CsmsMessageDispatcher {
        CsmsMessageDispatcher::new(
            EvseRegistry::new(),
            ProvisioningPolicy::allow_only(
                vec![name.to_string()],
                "the named depot fleet".to_string(),
            )
            .expect("valid policy"),
            60,
            8,
        )
        .expect("valid dispatcher")
    }

    fn boot_text() -> String {
        OcppFrame::Call {
            unique_id: UniqueId::new("boot-1".to_string()).expect("valid"),
            action: Action::BootNotification,
            payload: serde_json::json!({
                "vendorName": "vendor",
                "chargePointModel": "model"
            }),
        }
        .encode()
        .expect("encodes")
    }

    #[test]
    fn an_accepted_boot_flows_through_the_session_machine_and_registry() {
        let mut server = server();
        let mut dispatcher = dispatcher_allow("depot-cp-001");
        server
            .accept_connection(cp("depot-cp-001"), Subprotocol::Ocpp201)
            .expect("accepted");
        let replies = dispatcher
            .handle_inbound_text(&mut server, &cp("depot-cp-001"), &boot_text(), 1_000)
            .expect("answered");
        // The session machine walked Negotiated → Provisioned (the
        // EXISTING transitions, unchanged).
        let snapshot = server.connection_snapshot();
        assert_eq!(snapshot, vec![("depot-cp-001", "PROVISIONED")]);
        // The reply is the recorded BootNotification response shape.
        assert_eq!(replies.len(), 1);
        match &replies[0] {
            OcppFrame::CallResult { unique_id, payload } => {
                assert_eq!(unique_id.as_str(), "boot-1");
                assert_eq!(payload["status"], "Accepted");
                assert_eq!(payload["interval"], 60);
                assert_eq!(payload["currentTime"], 1_000);
            }
            other => panic!("expected a CALLRESULT, got {other:?}"),
        }
    }

    #[test]
    fn a_rejected_boot_ends_the_session_with_the_recorded_reason_and_names_the_refusal() {
        let mut server = server();
        let mut dispatcher = dispatcher_allow("some-other-station");
        server
            .accept_connection(cp("depot-cp-001"), Subprotocol::Ocpp201)
            .expect("accepted");
        let replies = dispatcher
            .handle_inbound_text(&mut server, &cp("depot-cp-001"), &boot_text(), 1_000)
            .expect("answered");
        // The session ended with the recorded BootRejected reason.
        let snapshot = server.connection_snapshot();
        assert_eq!(snapshot, vec![("depot-cp-001", "ENDED")]);
        let report = server.connection_report(&cp("depot-cp-001")).expect("report");
        assert!(matches!(report.state,
            flexcore_protocol_ports::connection::ConnectionState::Degraded { .. }));
        // The station still receives the recorded Rejected status.
        match &replies[0] {
            OcppFrame::CallResult { payload, .. } => {
                assert_eq!(payload["status"], "Rejected");
            }
            other => panic!("expected a CALLRESULT, got {other:?}"),
        }
    }

    #[test]
    fn heartbeat_is_a_liveness_reply_with_the_injected_clock_only() {
        let mut server = server();
        let mut dispatcher = dispatcher_allow("depot-cp-001");
        server
            .accept_connection(cp("depot-cp-001"), Subprotocol::Ocpp201)
            .expect("accepted");
        let text = OcppFrame::Call {
            unique_id: UniqueId::new("hb-1".to_string()).expect("valid"),
            action: Action::Heartbeat,
            payload: serde_json::json!({"currentTime": 500}),
        }
        .encode()
        .expect("encodes");
        let replies = dispatcher
            .handle_inbound_text(&mut server, &cp("depot-cp-001"), &text, 900)
            .expect("answered");
        match &replies[0] {
            OcppFrame::CallResult { payload, .. } => {
                assert_eq!(payload["currentTime"], 900);
            }
            other => panic!("expected a CALLRESULT, got {other:?}"),
        }
        // The session state is untouched by a heartbeat (liveness only).
        assert_eq!(
            server.connection_snapshot(),
            vec![("depot-cp-001", "NEGOTIATED")]
        );
    }

    #[test]
    fn a_malformed_boot_payload_is_answered_with_the_recorded_formation_violation() {
        let mut server = server();
        let mut dispatcher = dispatcher_allow("depot-cp-001");
        server
            .accept_connection(cp("depot-cp-001"), Subprotocol::Ocpp201)
            .expect("accepted");
        let text = OcppFrame::Call {
            unique_id: UniqueId::new("boot-2".to_string()).expect("valid"),
            action: Action::BootNotification,
            payload: serde_json::json!({"vendorName": "vendor"}), // no model
        }
        .encode()
        .expect("encodes");
        let replies = dispatcher
            .handle_inbound_text(&mut server, &cp("depot-cp-001"), &text, 1_000)
            .expect("answered");
        match &replies[0] {
            OcppFrame::CallError {
                error_code,
                error_description,
                ..
            } => {
                assert_eq!(*error_code, CallErrorCode::FormationViolation);
                assert!(error_description.contains("BootNotification"));
            }
            other => panic!("expected a CALLERROR, got {other:?}"),
        }
    }

    #[test]
    fn status_transaction_and_security_events_flow_into_the_existing_surfaces() {
        let mut server = server();
        let mut dispatcher = dispatcher_allow("depot-cp-001");
        server
            .accept_connection(cp("depot-cp-001"), Subprotocol::Ocpp201)
            .expect("accepted");
        // Boot first (provisions the registry entry).
        dispatcher
            .handle_inbound_text(&mut server, &cp("depot-cp-001"), &boot_text(), 1_000)
            .expect("booted");
        // A status notification maps through the domain table.
        let text = OcppFrame::Call {
            unique_id: UniqueId::new("st-1".to_string()).expect("valid"),
            action: Action::StatusNotification,
            payload: serde_json::json!({
                "timestamp": 1_100,
                "evseId": 0,
                "connectorStatus": "Available"
            }),
        }
        .encode()
        .expect("encodes");
        let replies = dispatcher
            .handle_inbound_text(&mut server, &cp("depot-cp-001"), &text, 1_101)
            .expect("answered");
        assert_eq!(replies, vec![empty_result(UniqueId::new("st-1".to_string()).expect("valid"))]);
        // A transaction event opens a transaction record.
        let text = OcppFrame::Call {
            unique_id: UniqueId::new("tx-1".to_string()).expect("valid"),
            action: Action::TransactionEvent,
            payload: serde_json::json!({
                "eventType": "Started",
                "timestamp": 1_200,
                "evse": 0,
                "transactionId": "txn-1",
                "energyWh": 0.0
            }),
        }
        .encode()
        .expect("encodes");
        dispatcher
            .handle_inbound_text(&mut server, &cp("depot-cp-001"), &text, 1_201)
            .expect("answered");
        // A security event ingests as a RECORD (no reaction).
        let text = OcppFrame::Call {
            unique_id: UniqueId::new("sec-1".to_string()).expect("valid"),
            action: Action::SecurityEventNotification,
            payload: serde_json::json!({
                "timestamp": 1_300,
                "type": "InvalidCSMSCertificate"
            }),
        }
        .encode()
        .expect("encodes");
        dispatcher
            .handle_inbound_text(&mut server, &cp("depot-cp-001"), &text, 1_301)
            .expect("answered");
        assert_eq!(dispatcher.security_event_snapshot().len(), 1);
        assert_eq!(
            dispatcher.security_event_snapshot()[0].severity.name(),
            "CRITICAL"
        );
    }

    #[test]
    fn certificate_and_composite_calls_are_answered_notimplemented_with_named_reasons() {
        let mut server = server();
        let mut dispatcher = dispatcher_allow("depot-cp-001");
        server
            .accept_connection(cp("depot-cp-001"), Subprotocol::Ocpp201)
            .expect("accepted");
        for (action_name, action) in [
            ("SignCertificate", Action::SignCertificate),
            ("GetCompositeSchedule", Action::GetCompositeSchedule),
        ] {
            let text = OcppFrame::Call {
                unique_id: UniqueId::new(format!("u-{action_name}")).expect("valid"),
                action,
                payload: serde_json::json!({}),
            }
            .encode()
            .expect("encodes");
            let replies = dispatcher
                .handle_inbound_text(&mut server, &cp("depot-cp-001"), &text, 1_000)
                .expect("answered");
            match &replies[0] {
                OcppFrame::CallError {
                    error_code,
                    error_description,
                    ..
                } => {
                    assert_eq!(*error_code, CallErrorCode::NotImplemented);
                    assert!(error_description.contains(action_name));
                }
                other => panic!("expected a CALLERROR, got {other:?}"),
            }
        }
        // The connection is NOT torn down by an unanswered action.
        assert_eq!(
            server.connection_snapshot(),
            vec![("depot-cp-001", "NEGOTIATED")]
        );
    }

    #[test]
    fn the_charging_relay_is_issued_only_and_the_accepted_reply_records_stored_not_applied() {
        let mut server = server();
        let mut dispatcher = dispatcher_allow("depot-cp-001");
        server
            .accept_connection(cp("depot-cp-001"), Subprotocol::Ocpp201)
            .expect("accepted");
        let envelope = DispatchEnvelope {
            start_millis: 1_700_000_000_000,
            periods: vec![DispatchPeriod {
                start_offset_s: 0,
                limit_mw: 0.05,
            }],
        };
        let plan = compose_external_constraints_profile(
            &envelope,
            RateUnit::W,
            &SupplyContext::AcThreePhase {
                nominal_voltage_v: 400.0,
            },
            &DeviceRateConstraints {
                min_charging_rate: None,
            },
            3,
            true,
        )
        .expect("composed");
        // Every non-ISSUED lifecycle state is refused by the EXISTING
        // typing (this module never constructs a lifecycle state).
        for state in [
            CommandState::Proposed,
            CommandState::Validated,
            CommandState::Feasible,
            CommandState::Approved,
        ] {
            let error = dispatcher
                .relay_charging_profile(
                    &mut server,
                    &cp("depot-cp-001"),
                    &plan,
                    state,
                    7,
                    UniqueId::new("relay-1".to_string()).expect("valid"),
                )
                .expect_err("refused");
            assert!(error.to_string().contains(state.name()));
        }
        // The ISSUED relay queues through the EXISTING bounded queue.
        dispatcher
            .relay_charging_profile(
                &mut server,
                &cp("depot-cp-001"),
                &plan,
                CommandState::Issued,
                7,
                UniqueId::new("relay-1".to_string()).expect("valid"),
            )
            .expect("queued");
        let drained = server.drain_outbound(&cp("depot-cp-001")).expect("drained");
        assert_eq!(drained.len(), 1);
        let wire = drained[0].encode().expect("encodes");
        let value: serde_json::Value = serde_json::from_str(&wire).expect("json");
        assert_eq!(value[0], serde_json::json!(2));
        assert_eq!(value[2], serde_json::json!("SetChargingProfile"));
        assert_eq!(value[3]["chargingProfileId"], 7);
        assert_eq!(value[3]["isGridCritical"], true);
        // The station's Accepted reply records the stored-not-applied
        // statement (STORAGE evidence, never application evidence).
        let reply = OcppFrame::CallResult {
            unique_id: UniqueId::new("relay-1".to_string()).expect("valid"),
            payload: serde_json::json!({"status": "Accepted"}),
        }
        .encode()
        .expect("encodes");
        dispatcher
            .handle_inbound_text(&mut server, &cp("depot-cp-001"), &reply, 2_000)
            .expect("recorded");
        let statement = dispatcher
            .last_accepted_statement()
            .expect("the honesty statement is recorded");
        assert!(statement.contains("STORED, not applied"));
        // A station CALLERROR for a relay is a named record, never a
        // retry.
        dispatcher
            .relay_charging_profile(
                &mut server,
                &cp("depot-cp-001"),
                &plan,
                CommandState::Issued,
                8,
                UniqueId::new("relay-2".to_string()).expect("valid"),
            )
            .expect("queued");
        server.drain_outbound(&cp("depot-cp-001")).expect("drained");
        let failure = OcppFrame::CallError {
            unique_id: UniqueId::new("relay-2".to_string()).expect("valid"),
            error_code: CallErrorCode::InternalError,
            error_description: "station-side failure".to_string(),
            error_details: serde_json::json!({}),
        }
        .encode()
        .expect("encodes");
        dispatcher
            .handle_inbound_text(&mut server, &cp("depot-cp-001"), &failure, 2_100)
            .expect("recorded");
        let statement = dispatcher.last_accepted_statement().expect("recorded");
        assert!(statement.contains("never"));
        assert!(statement.contains("retried"));
    }

    #[test]
    fn link_loss_touches_only_the_link_state_and_counts_untracked_replies() {
        let mut server = server();
        let mut dispatcher = dispatcher_allow("depot-cp-001");
        server
            .accept_connection(cp("depot-cp-001"), Subprotocol::Ocpp201)
            .expect("accepted");
        dispatcher
            .handle_inbound_text(&mut server, &cp("depot-cp-001"), &boot_text(), 1_000)
            .expect("booted");
        // The registry link state goes down; the session keeps its state
        // until the skeleton records the transport close.
        let statement = dispatcher
            .note_link_loss(&cp("depot-cp-001"), "websocket closed".to_string())
            .expect("applied");
        assert!(statement.contains("NOT asset unavailability"));
        // An untracked reply (matched by the skeleton's tracker but
        // queued by another surface) is counted, never absorbed.
        let first = UniqueId::new("other-1".to_string()).expect("valid");
        server
            .queue_outbound_call(
                &cp("depot-cp-001"),
                Action::GetCompositeSchedule,
                serde_json::json!({}),
                first,
            )
            .expect("queued");
        let reply = OcppFrame::CallResult {
            unique_id: UniqueId::new("other-1".to_string()).expect("valid"),
            payload: serde_json::json!({}),
        }
        .encode()
        .expect("encodes");
        dispatcher
            .handle_inbound_text(&mut server, &cp("depot-cp-001"), &reply, 2_000)
            .expect("recorded");
        assert_eq!(dispatcher.untracked_replies(), 1);
    }

    #[test]
    fn heartbeat_response_carries_the_recorded_shape() {
        // The Heartbeat response payload shape is pinned (UNVERIFIED
        // register item 2).
        let response = HeartbeatResponse {
            current_time_millis: 42,
        };
        let value = serde_json::to_value(&response).expect("serializes");
        assert_eq!(value["currentTime"], 42);
    }
}
