// P17.01 — The OCPP-J JSON framing codec (CALL / CALLRESULT / CALLERROR)
// and the one-outstanding-CALL discipline.
//
// Module-level UNVERIFIED-ASSUMPTION register (phase-17 plan §7.5). The
// OCPP-J / "OCPP over WebSockets" transport rules are cited from secondary
// sources ([S-121][S-123]); no OCA transport text is in the repository.
// Implementation-time checks against the OCA text:
//
// 1. The message-type codes: 2 = CALL, 3 = CALLRESULT, 4 = CALLERROR.
// 2. The array shapes: CALL = `[2, uniqueId, action, payload]`;
//    CALLRESULT = `[3, uniqueId, payload]`;
//    CALLERROR = `[4, uniqueId, errorCode, errorDescription, errorDetails]`.
// 3. The error-code vocabulary spellings (`FormationViolation`,
//    `NotImplemented`, `SecurityError`, `InternalError`,
//    `PropertyConstraintViolation`, `OccurrenceConstraintViolation`,
//    `TypeConstraintViolation`, `RPCFrameworkError`).
// 4. The subprotocol strings (`ocpp2.0.1`, `ocpp2.1`) and the WebSocket
//    handshake header the transport seam must negotiate (session.rs).
// 5. The one-outstanding-CALL discipline: the strict per-direction limit
//    implemented here (at most one CALL awaiting a reply, per connection,
//    per direction) is the DISCIPLINE THE PHASE RECORDS; whether the OCA
//    transport text permits more than one outstanding CALL per direction in
//    a given OCPP version must be checked at implementation time — a
//    relaxation would be a reviewed, recorded code change, never a silent
//    one.
//
// The codec is pure: `serde_json` in, typed frames out, named errors
// everywhere. It never partially decodes (a malformed frame is refused
// WHOLE — the ACL whole-refusal discipline).

use serde::{Deserialize, Serialize};

use crate::model::ModelError;

/// The action-name vocabulary this design HANDLES (UNVERIFIED spellings —
/// module register items 1-2). The vocabulary is closed: an action outside
/// it is a named refusal at the design surface, never a loose pass-through.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Action {
    // Provisioning block (P17.04).
    BootNotification,
    StatusNotification,
    Heartbeat,
    TransactionEvent,
    // Security block (P17.03).
    SecurityEventNotification,
    SignCertificate,
    CertificateSigned,
    InstallCertificate,
    // Smart-charging block (P17.05).
    SetChargingProfile,
    GetCompositeSchedule,
    ClearChargingProfile,
}

impl Action {
    /// Canonical wire-ish name (UNVERIFIED spelling — module register).
    pub const fn name(self) -> &'static str {
        match self {
            Action::BootNotification => "BootNotification",
            Action::StatusNotification => "StatusNotification",
            Action::Heartbeat => "Heartbeat",
            Action::TransactionEvent => "TransactionEvent",
            Action::SecurityEventNotification => "SecurityEventNotification",
            Action::SignCertificate => "SignCertificate",
            Action::CertificateSigned => "CertificateSigned",
            Action::InstallCertificate => "InstallCertificate",
            Action::SetChargingProfile => "SetChargingProfile",
            Action::GetCompositeSchedule => "GetCompositeSchedule",
            Action::ClearChargingProfile => "ClearChargingProfile",
        }
    }

    /// Parses an action name into the closed vocabulary; an unknown name is
    /// a named error (fail-closed).
    pub fn parse(name: &str) -> Result<Self, FramingError> {
        const ALL: [Action; 11] = [
            Action::BootNotification,
            Action::StatusNotification,
            Action::Heartbeat,
            Action::TransactionEvent,
            Action::SecurityEventNotification,
            Action::SignCertificate,
            Action::CertificateSigned,
            Action::InstallCertificate,
            Action::SetChargingProfile,
            Action::GetCompositeSchedule,
            Action::ClearChargingProfile,
        ];
        for action in ALL {
            if action.name() == name {
                return Ok(action);
            }
        }
        Err(FramingError::UnknownAction { name: name.to_string() })
    }
}

/// The CALLERROR error-code vocabulary (UNVERIFIED spellings — module
/// register item 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CallErrorCode {
    FormationViolation,
    NotImplemented,
    SecurityError,
    InternalError,
    PropertyConstraintViolation,
    OccurrenceConstraintViolation,
    TypeConstraintViolation,
    RpcFrameworkError,
}

impl CallErrorCode {
    /// Canonical wire-ish name (UNVERIFIED spelling — module register).
    pub const fn name(self) -> &'static str {
        match self {
            CallErrorCode::FormationViolation => "FormationViolation",
            CallErrorCode::NotImplemented => "NotImplemented",
            CallErrorCode::SecurityError => "SecurityError",
            CallErrorCode::InternalError => "InternalError",
            CallErrorCode::PropertyConstraintViolation => "PropertyConstraintViolation",
            CallErrorCode::OccurrenceConstraintViolation => "OccurrenceConstraintViolation",
            CallErrorCode::TypeConstraintViolation => "TypeConstraintViolation",
            CallErrorCode::RpcFrameworkError => "RPCFrameworkError",
        }
    }
}

/// A unique-id on the wire (a non-empty string; generation is a
/// transport-seam concern — the codec validates shape only).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UniqueId(String);

impl UniqueId {
    /// Validates and constructs: non-empty, sanity-bounded (not a protocol
    /// limit claim — UNVERIFIED).
    pub fn new(value: String) -> Result<Self, FramingError> {
        if value.trim().is_empty() {
            return Err(FramingError::EmptyUniqueId);
        }
        if value.len() > 64 {
            return Err(FramingError::UniqueIdTooLong { len: value.len() });
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for UniqueId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A decoded OCPP-J frame (the three message types; UNVERIFIED codes and
/// shapes — module register items 1-2). The payload travels as
/// `serde_json::Value`; the action-specific payload decoding happens in
/// [`crate::model`] constructors, keeping the framing layer
/// action-agnostic.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum OcppFrame {
    /// `[2, uniqueId, action, payload]` — a request.
    Call {
        unique_id: UniqueId,
        action: Action,
        payload: serde_json::Value,
    },
    /// `[3, uniqueId, payload]` — a successful reply.
    CallResult {
        unique_id: UniqueId,
        payload: serde_json::Value,
    },
    /// `[4, uniqueId, errorCode, errorDescription, errorDetails]` — an
    /// error reply.
    CallError {
        unique_id: UniqueId,
        error_code: CallErrorCode,
        error_description: String,
        error_details: serde_json::Value,
    },
}

impl OcppFrame {
    /// The message-type code (UNVERIFIED — module register item 1).
    pub const CALL_CODE: u8 = 2;
    pub const CALLRESULT_CODE: u8 = 3;
    pub const CALLERROR_CODE: u8 = 4;

    /// The frame's unique id (the correlation key of the outstanding-CALL
    /// tracker).
    pub fn unique_id(&self) -> &UniqueId {
        match self {
            OcppFrame::Call { unique_id, .. } => unique_id,
            OcppFrame::CallResult { unique_id, .. } => unique_id,
            OcppFrame::CallError { unique_id, .. } => unique_id,
        }
    }

    /// Encodes the frame into the OCPP-J JSON array shape. Encoding is
    /// total for validated frames; the payload must already be a JSON
    /// value.
    pub fn encode(&self) -> Result<String, FramingError> {
        let array = match self {
            OcppFrame::Call {
                unique_id,
                action,
                payload,
            } => serde_json::json!([
                Self::CALL_CODE,
                unique_id.as_str(),
                action.name(),
                payload
            ]),
            OcppFrame::CallResult { unique_id, payload } => {
                serde_json::json!([Self::CALLRESULT_CODE, unique_id.as_str(), payload])
            }
            OcppFrame::CallError {
                unique_id,
                error_code,
                error_description,
                error_details,
            } => serde_json::json!([
                Self::CALLERROR_CODE,
                unique_id.as_str(),
                error_code.name(),
                error_description,
                error_details
            ]),
        };
        serde_json::to_string(&array).map_err(|_| FramingError::EncodeFailed)
    }

    /// Decodes one wire text frame into a typed frame. Refusal is WHOLE:
    /// a malformed array, unknown message-type code, wrong arity, empty
    /// unique id, or unknown action name is a named error — never a
    /// partially decoded frame (the ACL whole-refusal discipline).
    pub fn decode(text: &str) -> Result<Self, FramingError> {
        let value: serde_json::Value = serde_json::from_str(text)
            .map_err(|_| FramingError::MalformedJson)?;
        let items = value
            .as_array()
            .ok_or(FramingError::NotAnArray)?
            .clone();
        // An empty array IS an array (so not `NotAnArray`) but carries no
        // message-type code — the faithful refusal is `InvalidMessageType`
        // (the conformance vector pins `[]` to it).
        let code = items
            .first()
            .and_then(|item| item.as_u64())
            .ok_or(FramingError::InvalidMessageType)?
            .try_into()
            .map_err(|_| FramingError::InvalidMessageType)?;
        match code {
            Self::CALL_CODE => {
                if items.len() != 4 {
                    return Err(FramingError::CallArity { found: items.len() });
                }
                let unique_id = UniqueId::new(string_item(&items[1], "call.uniqueId")?)?;
                let action_name = string_item(&items[2], "call.action")?;
                let action = Action::parse(&action_name)?;
                let payload = items[3].clone();
                if !payload.is_object() {
                    return Err(FramingError::PayloadNotAnObject);
                }
                Ok(OcppFrame::Call {
                    unique_id,
                    action,
                    payload,
                })
            }
            Self::CALLRESULT_CODE => {
                if items.len() != 3 {
                    return Err(FramingError::CallResultArity { found: items.len() });
                }
                let unique_id = UniqueId::new(string_item(&items[1], "result.uniqueId")?)?;
                let payload = items[2].clone();
                if !payload.is_object() {
                    return Err(FramingError::PayloadNotAnObject);
                }
                Ok(OcppFrame::CallResult {
                    unique_id,
                    payload,
                })
            }
            Self::CALLERROR_CODE => {
                if items.len() != 5 {
                    return Err(FramingError::CallErrorArity { found: items.len() });
                }
                let unique_id = UniqueId::new(string_item(&items[1], "error.uniqueId")?)?;
                let code_name = string_item(&items[2], "error.errorCode")?;
                let error_code = parse_error_code(&code_name)?;
                let error_description = string_item(&items[3], "error.errorDescription")?;
                Ok(OcppFrame::CallError {
                    unique_id,
                    error_code,
                    error_description,
                    error_details: items[4].clone(),
                })
            }
            _ => Err(FramingError::InvalidMessageType),
        }
    }
}

fn string_item(value: &serde_json::Value, field: &'static str) -> Result<String, FramingError> {
    value
        .as_str()
        .map(|s| s.to_string())
        .ok_or(FramingError::ExpectedString { field })
}

fn parse_error_code(name: &str) -> Result<CallErrorCode, FramingError> {
    const ALL: [CallErrorCode; 8] = [
        CallErrorCode::FormationViolation,
        CallErrorCode::NotImplemented,
        CallErrorCode::SecurityError,
        CallErrorCode::InternalError,
        CallErrorCode::PropertyConstraintViolation,
        CallErrorCode::OccurrenceConstraintViolation,
        CallErrorCode::TypeConstraintViolation,
        CallErrorCode::RpcFrameworkError,
    ];
    for code in ALL {
        if code.name() == name {
            return Ok(code);
        }
    }
    Err(FramingError::UnknownErrorCode { name: name.to_string() })
}

/// Framing errors. Every failure is named; nothing is silently repaired.
#[derive(Debug, Clone, PartialEq)]
pub enum FramingError {
    /// The wire text is not valid JSON.
    MalformedJson,
    /// The frame is not a JSON array.
    NotAnArray,
    /// The message-type code is not 2/3/4 (UNVERIFIED codes — module
    /// register item 1).
    InvalidMessageType,
    /// A CALL has the wrong arity (expected 4).
    CallArity { found: usize },
    /// A CALLRESULT has the wrong arity (expected 3).
    CallResultArity { found: usize },
    /// A CALLERROR has the wrong arity (expected 5).
    CallErrorArity { found: usize },
    /// A required string element is missing or not a string.
    ExpectedString { field: &'static str },
    /// The unique id is empty. Silence is never a correlation id.
    EmptyUniqueId,
    /// The unique id exceeds the sanity bound (not a protocol claim).
    UniqueIdTooLong { len: usize },
    /// The action name is outside the closed handled vocabulary.
    UnknownAction { name: String },
    /// The CALLERROR code is outside the closed vocabulary.
    UnknownErrorCode { name: String },
    /// A payload is present but not a JSON object.
    PayloadNotAnObject,
    /// The frame could not be encoded (a serde failure — treated as
    /// internal, named, never retried silently).
    EncodeFailed,
    /// The payload failed the action-specific model validation
    /// (re-raised from `crate::model`).
    Model(ModelError),
}

impl std::fmt::Display for FramingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FramingError::MalformedJson => write!(f, "frame is not valid JSON"),
            FramingError::NotAnArray => write!(f, "frame is not a JSON array"),
            FramingError::InvalidMessageType => {
                write!(f, "message-type code is not 2 (CALL), 3 (CALLRESULT), or 4 (CALLERROR)")
            }
            FramingError::CallArity { found } => {
                write!(f, "CALL has {found} elements, expected 4")
            }
            FramingError::CallResultArity { found } => {
                write!(f, "CALLRESULT has {found} elements, expected 3")
            }
            FramingError::CallErrorArity { found } => {
                write!(f, "CALLERROR has {found} elements, expected 5")
            }
            FramingError::ExpectedString { field } => {
                write!(f, "frame element '{field}' is not a string")
            }
            FramingError::EmptyUniqueId => write!(f, "unique id is empty"),
            FramingError::UniqueIdTooLong { len } => {
                write!(f, "unique id of {len} characters exceeds the 64-character sanity bound")
            }
            FramingError::UnknownAction { name } => {
                write!(f, "action '{name}' is outside the handled vocabulary (fail-closed refusal, never a loose pass-through)")
            }
            FramingError::UnknownErrorCode { name } => {
                write!(f, "CALLERROR code '{name}' is outside the known vocabulary")
            }
            FramingError::PayloadNotAnObject => {
                write!(f, "frame payload is present but not a JSON object")
            }
            FramingError::EncodeFailed => write!(f, "frame encoding failed"),
            FramingError::Model(error) => write!(f, "payload failed model validation: {error}"),
        }
    }
}

impl std::error::Error for FramingError {}

impl From<ModelError> for FramingError {
    fn from(error: ModelError) -> Self {
        FramingError::Model(error)
    }
}

// --- The one-outstanding-CALL tracker --------------------------------------------

/// The outstanding-CALL tracker for ONE direction of ONE connection
/// (module register item 5: the strict per-direction, per-connection
/// one-outstanding-CALL discipline; UNVERIFIED whether the OCA transport
/// text permits more — the check is recorded in the phase-17 record 01).
///
/// The tracker is deliberately SEPARATE from the session state machine so
/// the two rules stay independently reviewable: the session tracks
/// connection lifecycle, the tracker tracks call correlation.
#[derive(Debug, Default)]
pub struct CallTracker {
    outstanding: Option<UniqueId>,
}

/// Tracker outcomes. Every path is named; nothing is silently replaced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallTrackError {
    /// A CALL was issued while another was outstanding. The second CALL is
    /// refused by the caller's own discipline — the tracker never
    /// replaces the first.
    CallAlreadyOutstanding { outstanding: String },
    /// A reply arrived for a CALL the tracker does not hold (duplicate
    /// reply, stale id after timeout, or corruption). The reply is
    /// reported as unmatched, never applied.
    UnknownCallId { received: String },
}

impl std::fmt::Display for CallTrackError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CallTrackError::CallAlreadyOutstanding { outstanding } => {
                write!(f, "CALL already outstanding (unique id {outstanding}); the one-outstanding-CALL discipline refuses a second")
            }
            CallTrackError::UnknownCallId { received } => {
                write!(f, "reply unique id {received} matches no outstanding CALL (duplicate, stale, or corrupt)")
            }
        }
    }
}

impl std::error::Error for CallTrackError {}

impl CallTracker {
    /// A tracker with nothing outstanding.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers an outgoing CALL. Refuses when another CALL is
    /// outstanding (the discipline).
    pub fn track_call(&mut self, unique_id: UniqueId) -> Result<(), CallTrackError> {
        if let Some(outstanding) = &self.outstanding {
            return Err(CallTrackError::CallAlreadyOutstanding {
                outstanding: outstanding.to_string(),
            });
        }
        self.outstanding = Some(unique_id);
        Ok(())
    }

    /// Matches a reply (CALLRESULT or CALLERROR) to the outstanding CALL
    /// and clears the slot. An unmatched reply is refused, never applied.
    pub fn track_reply(&mut self, unique_id: &UniqueId) -> Result<(), CallTrackError> {
        let matched = self
            .outstanding
            .as_ref()
            .is_some_and(|outstanding| outstanding == unique_id);
        if matched {
            self.outstanding = None;
            Ok(())
        } else {
            Err(CallTrackError::UnknownCallId {
                received: unique_id.to_string(),
            })
        }
    }

    /// Whether a CALL is currently outstanding.
    pub fn has_outstanding(&self) -> bool {
        self.outstanding.is_some()
    }

    /// The outstanding unique id, when one exists.
    pub fn outstanding(&self) -> Option<&UniqueId> {
        self.outstanding.as_ref()
    }

    /// Drops the outstanding CALL (timeout/failure path). The reply, when
    /// it later arrives, will be reported as unmatched — the honest
    /// outcome.
    pub fn drop_outstanding(&mut self) {
        self.outstanding = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        BootNotificationRequest, ChargingProfilePurpose, ChargingRateUnit, ChargingSchedule,
        ChargingSchedulePeriod, ChargingProfileKind, SetChargingProfileRequest,
    };

    fn boot_call() -> (UniqueId, String) {
        let unique_id = UniqueId::new("id-1".to_string()).expect("valid");
        let request = BootNotificationRequest {
            vendor_name: "vendor".to_string(),
            charge_point_model: "model".to_string(),
            charge_point_serial_number: None,
            firmware_version: None,
            protocol: None,
        };
        let payload = serde_json::to_value(&request).expect("serializes");
        let frame = OcppFrame::Call {
            unique_id: unique_id.clone(),
            action: Action::BootNotification,
            payload,
        };
        let text = frame.encode().expect("encodes");
        (unique_id, text)
    }

    #[test]
    fn call_round_trips_through_the_codec() {
        let (unique_id, text) = boot_call();
        let decoded = OcppFrame::decode(&text).expect("decodes");
        match &decoded {
            OcppFrame::Call {
                unique_id: got_id,
                action,
                payload,
            } => {
                assert_eq!(got_id, &unique_id);
                assert_eq!(*action, Action::BootNotification);
                assert_eq!(payload["vendorName"], "vendor");
                assert_eq!(payload["chargePointModel"], "model");
            }
            other => panic!("expected a CALL, got {other:?}"),
        }
        // The wire shape is the recorded 4-element CALL array.
        let value: serde_json::Value = serde_json::from_str(&text).expect("json");
        let items = value.as_array().expect("array");
        assert_eq!(items.len(), 4);
        assert_eq!(items[0], serde_json::json!(2));
        assert_eq!(items[2], serde_json::json!("BootNotification"));
    }

    #[test]
    fn call_result_and_error_round_trip() {
        let unique_id = UniqueId::new("id-9".to_string()).expect("valid");
        let result = OcppFrame::CallResult {
            unique_id: unique_id.clone(),
            payload: serde_json::json!({"status": "Accepted"}),
        };
        let text = result.encode().expect("encodes");
        let decoded = OcppFrame::decode(&text).expect("decodes");
        assert!(matches!(decoded, OcppFrame::CallResult { .. }));
        let value: serde_json::Value = serde_json::from_str(&text).expect("json");
        assert_eq!(value[0], serde_json::json!(3));

        let error = OcppFrame::CallError {
            unique_id: unique_id.clone(),
            error_code: CallErrorCode::FormationViolation,
            error_description: "bad frame".to_string(),
            error_details: serde_json::json!({}),
        };
        let text = error.encode().expect("encodes");
        let decoded = OcppFrame::decode(&text).expect("decodes");
        match &decoded {
            OcppFrame::CallError {
                error_code,
                error_description,
                ..
            } => {
                assert_eq!(*error_code, CallErrorCode::FormationViolation);
                assert_eq!(error_description, "bad frame");
            }
            other => panic!("expected a CALLERROR, got {other:?}"),
        }
        let value: serde_json::Value = serde_json::from_str(&text).expect("json");
        assert_eq!(value[0], serde_json::json!(4));
        assert_eq!(value.as_array().expect("a CALLERROR is an array").len(), 5);
    }

    #[test]
    fn malformed_frames_are_refused_whole() {
        // Not JSON.
        assert_eq!(
            OcppFrame::decode("not json"),
            Err(FramingError::MalformedJson)
        );
        // Not an array.
        assert_eq!(
            OcppFrame::decode(r#"{"a":1}"#),
            Err(FramingError::NotAnArray)
        );
        // Invalid message type.
        assert_eq!(
            OcppFrame::decode(r#"[9,"id",{}]"#),
            Err(FramingError::InvalidMessageType)
        );
        // Wrong CALL arity.
        assert_eq!(
            OcppFrame::decode(r#"[2,"id","Heartbeat"]"#),
            Err(FramingError::CallArity { found: 3 })
        );
        // Unknown action: fail-closed refusal, never a loose pass-through.
        let error = OcppFrame::decode(r#"[2,"id","VendorSpecific",{ }]"#)
            .expect_err("unknown action refused");
        assert!(matches!(error, FramingError::UnknownAction { .. }));
        assert!(error.to_string().contains("VendorSpecific"));
        // Payload not an object.
        assert_eq!(
            OcppFrame::decode(r#"[2,"id","Heartbeat",[1]]"#),
            Err(FramingError::PayloadNotAnObject)
        );
        // Empty unique id.
        assert_eq!(
            OcppFrame::decode(r#"[3,"","{}"]"#),
            Err(FramingError::EmptyUniqueId)
        );
        // Unknown error code.
        assert!(matches!(
            OcppFrame::decode(r#"[4,"id","SomeCode","desc",{}]"#),
            Err(FramingError::UnknownErrorCode { .. })
        ));
        // CALLRESULT wrong arity.
        assert_eq!(
            OcppFrame::decode(r#"[3,"id","{}","extra"]"#),
            Err(FramingError::CallResultArity { found: 4 })
        );
    }

    #[test]
    fn unique_id_rules() {
        assert_eq!(UniqueId::new("   ".to_string()), Err(FramingError::EmptyUniqueId));
        let long = "x".repeat(65);
        assert!(matches!(
            UniqueId::new(long),
            Err(FramingError::UniqueIdTooLong { len: 65 })
        ));
    }

    #[test]
    fn action_vocabulary_is_closed_and_named() {
        assert_eq!(Action::parse("BootNotification"), Ok(Action::BootNotification));
        assert_eq!(
            Action::parse("SetChargingProfile"),
            Ok(Action::SetChargingProfile)
        );
        assert!(Action::parse("not-an-action").is_err());
        // Every handled action encodes and round-trips its name.
        for action in [
            Action::BootNotification,
            Action::StatusNotification,
            Action::Heartbeat,
            Action::TransactionEvent,
            Action::SecurityEventNotification,
            Action::SignCertificate,
            Action::CertificateSigned,
            Action::InstallCertificate,
            Action::SetChargingProfile,
            Action::GetCompositeSchedule,
            Action::ClearChargingProfile,
        ] {
            let unique_id =
                UniqueId::new(format!("id-{}", action.name())).expect("valid");
            let frame = OcppFrame::Call {
                unique_id,
                action,
                payload: serde_json::json!({}),
            };
            let decoded = OcppFrame::decode(&frame.encode().expect("encodes"))
                .expect("decodes");
            assert_eq!(decoded.unique_id().as_str(), format!("id-{}", action.name()));
        }
    }

    #[test]
    fn charging_profile_frame_carries_the_recorded_field_spellings() {
        let request = SetChargingProfileRequest {
            charging_profile_id: 42,
            evse_id: 0,
            is_grid_critical: Some(true),
            transaction_id: None,
            stack_level: 7,
            charging_profile_purpose: ChargingProfilePurpose::ChargingStationExternalConstraints,
            charging_profile_kind: ChargingProfileKind::Absolute,
            charging_schedule: ChargingSchedule {
                duration_s: Some(3600),
                start_schedule_millis: Some(0),
                charging_rate_unit: ChargingRateUnit::W,
                min_charging_rate: Some(0.0),
                charging_schedule_period: vec![
                    ChargingSchedulePeriod {
                        start_period_s: 0,
                        limit: 100_000.0,
                        number_phases: Some(3),
                    },
                    ChargingSchedulePeriod {
                        start_period_s: 1800,
                        limit: 0.0,
                        number_phases: Some(3),
                    },
                ],
            },
        };
        request.validate().expect("valid");
        let payload = serde_json::to_value(&request).expect("serializes");
        // The recorded field spellings (UNVERIFIED — pinned here so a
        // change is a conscious edit, module register item 6).
        assert_eq!(payload["chargingProfileId"], 42);
        assert_eq!(payload["evseId"], 0);
        assert_eq!(payload["isGridCritical"], true);
        assert_eq!(payload["stackLevel"], 7);
        assert_eq!(
            payload["chargingProfilePurpose"],
            "ChargingStationExternalConstraints"
        );
        assert_eq!(payload["chargingProfileKind"], "Absolute");
        assert_eq!(payload["chargingSchedule"]["chargingRateUnit"], "W");
        assert_eq!(payload["chargingSchedule"]["minChargingRate"], 0.0);
        assert_eq!(payload["chargingSchedule"]["chargingSchedulePeriod"][1]["limit"], 0.0);
        // The frame round-trips.
        let unique_id = UniqueId::new("cp-42".to_string()).expect("valid");
        let frame = OcppFrame::Call {
            unique_id: unique_id.clone(),
            action: Action::SetChargingProfile,
            payload,
        };
        let decoded = OcppFrame::decode(&frame.encode().expect("encodes")).expect("decodes");
        assert_eq!(decoded.unique_id(), &unique_id);
    }

    #[test]
    fn one_outstanding_call_per_direction() {
        let mut tracker = CallTracker::new();
        assert!(!tracker.has_outstanding());
        let first = UniqueId::new("call-1".to_string()).expect("valid");
        let second = UniqueId::new("call-2".to_string()).expect("valid");

        tracker.track_call(first.clone()).expect("tracked");
        assert!(tracker.has_outstanding());
        // A second CALL while the first is outstanding is refused — the
        // tracker never replaces the first.
        assert_eq!(
            tracker.track_call(second.clone()),
            Err(CallTrackError::CallAlreadyOutstanding {
                outstanding: "call-1".to_string()
            })
        );
        // The reply for the outstanding CALL clears the slot.
        tracker.track_reply(&first).expect("matched");
        assert!(!tracker.has_outstanding());
        // Now the second CALL can be tracked.
        tracker.track_call(second.clone()).expect("tracked");
        // An unmatched reply is refused, never applied.
        let stale = UniqueId::new("call-3".to_string()).expect("valid");
        assert_eq!(
            tracker.track_reply(&stale),
            Err(CallTrackError::UnknownCallId {
                received: "call-3".to_string()
            })
        );
        // The timeout path drops the CALL; a late reply reports unmatched.
        tracker.drop_outstanding();
        assert!(!tracker.has_outstanding());
        assert!(tracker.track_reply(&second).is_err());
        // After the drop, a new CALL is permitted again.
        tracker.track_call(stale.clone()).expect("tracked");
    }

    #[test]
    fn tracker_error_messages_name_the_discipline() {
        let error = CallTrackError::CallAlreadyOutstanding {
            outstanding: "call-1".to_string(),
        };
        assert!(error.to_string().contains("one-outstanding-CALL"));
        let error = CallTrackError::UnknownCallId {
            received: "call-9".to_string(),
        };
        assert!(error.to_string().contains("matches no outstanding CALL"));
    }
}
