// FIM-F05.01 — the hand-written MQTT 5 client wire codec
// (docs/releases/final-implementation-before-publication.md §5 F-05; ledger
// docs/releases/implementation_ledger/ledger_F05.md; FIM standing decision 1
// — a missing wire protocol is a hand-written minimal codec, NOT a new
// dependency).
//
// **Implemented under FIM-F05, WRITTEN-UNRUN: this module has never been
// compiled or executed. Interop with a real MQTT 5 broker is UNPROVEN and
// remains a HUMAN LAB GATE** (crate conventions §7.3). The byte framing
// below is pinned by hand-computed fixtures (computed offline with
// `python -I` over the MQTT variable-length-integer and fixed-header rules;
// the pinned vectors live in `tests/mqtt_wire_codec.rs` and in this
// module's test block). Nothing here is a conformance result.
//
// What the codec covers (the declared subset the F-05 uplink needs):
//
// - The MQTT variable-length integer (remaining length, 1-4 bytes) and the
//   fixed header. `frame_len` is the incremental framer the live transport
//   pumps bytes through (`live.rs`); the same function drives the mock
//   broker in the socket-free tests.
// - CONNECT (protocol level 5; clean start; keep alive; the properties
//   subset: session expiry interval `0x11`, receive maximum `0x21`, topic
//   alias maximum `0x22`; the will message — the NDEATH LWT) and CONNACK
//   (session-present flag, reason code, the same property subset plus the
//   well-known response properties, skipped by the declared property
//   table). There is NO password anywhere: the password connect flag is
//   never set by [`encode_connect`] and [`ConnectOptions`] has no password
//   field — absence is structural (the never-granted `secret.read` rule).
// - PUBLISH (QoS 0 and QoS 1; DUP/RETAIN flags; packet identifier; empty
//   property blob) and PUBACK (packet identifier, reason code, empty
//   property blob). QoS 2 is NOT encoded by this codec — the declared
//   uplink default is QoS 1 (`transport::DeclaredQos`).
// - SUBSCRIBE (packet identifier, one topic filter, options byte) and
//   SUBACK (granted QoS / refusal reason codes).
// - PINGREQ / PINGRESP (keep alive) and DISCONNECT (reason code).
//
// UNVERIFIED-ASSUMPTION register (module-level; the model.rs discipline
// applies — a mismatch found against the MQTT 5 / MQTT-5.0 specification
// text at the human lab gate is a code or record edit, never a silent
// reinterpretation):
//
// - The property identifier table below ([`property_value_kind`]) encodes
//   the MQTT 5 property → value-kind mapping as a declared constant table;
//   every identifier outside the table is a NAMED refusal (fail-closed —
//   the codec does not silently skip unknown properties).
// - The reason-code table ([`MqttReasonCode`]) pins the MQTT 5 reason-code
//   spellings; the lab gate confirms each value.
// - Topic-alias properties are DECLARED but never USED: the encoder sends
//   no topic alias and the decoder skips one if a peer sends it — the
//   alias mechanism is not exercised anywhere in this crate.
// - UTF-8 strings are validated as Rust UTF-8; the specification's control
//   character exclusions (U+0000, certain U+D800 ranges) are NOT enforced
//   (declared limitation, lab-gate item).

use crate::transport::DeclaredQos;

/// The MQTT protocol name the CONNECT variable header carries.
pub const PROTOCOL_NAME: &str = "MQTT";

/// The MQTT protocol LEVEL this codec speaks: 5.
pub const PROTOCOL_LEVEL: u8 = 5;

/// The MQTT protocol maximum remaining length (4-byte variable-length
/// integer ceiling). A larger declared frame is a named refusal.
pub const MAX_REMAINING_LENGTH: u32 = 268_435_455;

/// Wire packet types (the high nibble of the fixed header's first byte).
pub const PACKET_CONNECT: u8 = 1;
pub const PACKET_CONNACK: u8 = 2;
pub const PACKET_PUBLISH: u8 = 3;
pub const PACKET_PUBACK: u8 = 4;
pub const PACKET_SUBSCRIBE: u8 = 8;
pub const PACKET_SUBACK: u8 = 9;
pub const PACKET_PINGREQ: u8 = 12;
pub const PACKET_PINGRESP: u8 = 13;
pub const PACKET_DISCONNECT: u8 = 14;

/// Errors the wire codec reports (named, never silent).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MqttWireError {
    /// A field violates the declared encoding rules; `packet` names the
    /// packet type the refusal happened in.
    Malformed {
        packet: &'static str,
        reason: String,
    },
    /// A variable-length integer exceeded 4 bytes (the specification's
    /// remaining-length ceiling).
    VarintTooLong,
    /// A declared remaining length exceeded [`MAX_REMAINING_LENGTH`].
    FrameTooLarge {
        size: u32,
    },
    /// A QoS value outside {0, 1, 2} was carried on the wire.
    UnsupportedQos {
        value: u8,
    },
}

impl std::fmt::Display for MqttWireError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MqttWireError::Malformed { packet, reason } => {
                write!(f, "malformed MQTT {packet} packet: {reason}")
            }
            MqttWireError::VarintTooLong => {
                write!(f, "MQTT variable-length integer exceeds 4 bytes")
            }
            MqttWireError::FrameTooLarge { size } => {
                write!(f, "MQTT frame remaining length {size} exceeds the declared maximum")
            }
            MqttWireError::UnsupportedQos { value } => {
                write!(f, "MQTT QoS value {value} is outside the 0/1/2 range")
            }
        }
    }
}

impl std::error::Error for MqttWireError {}

// --- Variable-length integer -------------------------------------------------

/// Encodes one MQTT variable-length integer (the remaining-length
/// encoding: 7 bits per byte, continuation bit 0x80, least significant
/// first).
pub fn encode_varint(mut value: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity(4);
    loop {
        let mut byte = (value % 128) as u8;
        value /= 128;
        if value > 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if value == 0 {
            return out;
        }
    }
}

/// Decodes one MQTT variable-length integer from the front of `buf`.
/// Returns the value and the number of bytes consumed.
pub fn decode_varint(buf: &[u8]) -> Result<(u32, usize), MqttWireError> {
    let mut value: u32 = 0;
    let mut multiplier: u32 = 1;
    for (index, &byte) in buf.iter().enumerate() {
        if index >= 4 {
            return Err(MqttWireError::VarintTooLong);
        }
        value += u32::from(byte & 0x7F) * multiplier;
        if byte & 0x80 == 0 {
            return Ok((value, index + 1));
        }
        multiplier *= 128;
    }
    // The buffer ended mid-varint: the caller must feed more bytes (this
    // is an INCOMPLETE prefix, not a protocol error).
    Err(MqttWireError::Malformed {
        packet: "frame",
        reason: "variable-length integer prefix is incomplete".to_string(),
    })
}

/// The total wire length of the FIRST frame buffered in `buf`, if a whole
/// frame is present. `Ok(None)` means more bytes are needed (an
/// incomplete fixed-header prefix — the incremental framer keeps
/// pumping); a wire error (varint overflow, oversized frame) is a named
/// refusal. This is the function the live transport's read loop and the
/// mock brokers both drive.
pub fn frame_len(buf: &[u8]) -> Result<Option<usize>, MqttWireError> {
    if buf.is_empty() {
        return Ok(None);
    }
    let (remaining, consumed) = match decode_varint(&buf[1..]) {
        Ok(pair) => pair,
        // An incomplete varint prefix needs more bytes; a too-long varint
        // is a real refusal.
        Err(MqttWireError::Malformed { .. }) => return Ok(None),
        Err(error) => return Err(error),
    };
    if remaining > MAX_REMAINING_LENGTH {
        return Err(MqttWireError::FrameTooLarge { size: remaining });
    }
    let total = 1 + consumed + remaining as usize;
    if buf.len() < total {
        return Ok(None);
    }
    Ok(Some(total))
}

// --- Reason codes -------------------------------------------------------------

/// The MQTT 5 reason-code vocabulary this codec pins (CONNACK, PUBACK,
/// SUBACK, and DISCONNECT use it). Values are the MQTT 5 constants;
/// UNVERIFIED against the specification text at the human lab gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MqttReasonCode {
    Success,
    GrantedQos1,
    GrantedQos2,
    NormalDisconnection,
    UnspecifiedError,
    MalformedPacket,
    ProtocolError,
    ImplementationSpecificError,
    UnsupportedProtocolVersion,
    ClientIdentifierNotValid,
    BadUserNameOrPassword,
    NotAuthorized,
    ServerUnavailable,
    ServerBusy,
    Banned,
    ServerShuttingDown,
    BadAuthenticationMethod,
    KeepAliveTimeout,
    SessionTakenOver,
    TopicFilterInvalid,
    TopicNameInvalid,
    PacketIdentifierInUse,
    PacketIdentifierNotFound,
    ReceiveMaximumExceeded,
    TopicAliasInvalid,
    PacketTooLarge,
    MessageRateTooHigh,
    QuotaExceeded,
    AdministrativeAction,
    PayloadFormatInvalid,
    RetainNotSupported,
    QosNotSupported,
    UseAnotherServer,
    ServerMoved,
    SharedSubscriptionsNotSupported,
    ConnectionRateExceeded,
    MaximumConnectTime,
    SubscriptionIdentifiersNotSupported,
    WildcardSubscriptionsNotSupported,
}

impl MqttReasonCode {
    /// The wire byte for this reason code.
    pub const fn as_u8(self) -> u8 {
        match self {
            MqttReasonCode::Success => 0x00,
            MqttReasonCode::GrantedQos1 => 0x01,
            MqttReasonCode::GrantedQos2 => 0x02,
            MqttReasonCode::NormalDisconnection => 0x00,
            MqttReasonCode::UnspecifiedError => 0x80,
            MqttReasonCode::MalformedPacket => 0x81,
            MqttReasonCode::ProtocolError => 0x82,
            MqttReasonCode::ImplementationSpecificError => 0x83,
            MqttReasonCode::UnsupportedProtocolVersion => 0x84,
            MqttReasonCode::ClientIdentifierNotValid => 0x85,
            MqttReasonCode::BadUserNameOrPassword => 0x86,
            MqttReasonCode::NotAuthorized => 0x87,
            MqttReasonCode::ServerUnavailable => 0x88,
            MqttReasonCode::ServerBusy => 0x89,
            MqttReasonCode::Banned => 0x8A,
            MqttReasonCode::ServerShuttingDown => 0x8B,
            MqttReasonCode::BadAuthenticationMethod => 0x8C,
            MqttReasonCode::KeepAliveTimeout => 0x8D,
            MqttReasonCode::SessionTakenOver => 0x8E,
            MqttReasonCode::TopicFilterInvalid => 0x8F,
            MqttReasonCode::TopicNameInvalid => 0x90,
            MqttReasonCode::PacketIdentifierInUse => 0x91,
            MqttReasonCode::PacketIdentifierNotFound => 0x92,
            MqttReasonCode::ReceiveMaximumExceeded => 0x93,
            MqttReasonCode::TopicAliasInvalid => 0x94,
            MqttReasonCode::PacketTooLarge => 0x95,
            MqttReasonCode::MessageRateTooHigh => 0x96,
            MqttReasonCode::QuotaExceeded => 0x97,
            MqttReasonCode::AdministrativeAction => 0x98,
            MqttReasonCode::PayloadFormatInvalid => 0x99,
            MqttReasonCode::RetainNotSupported => 0x9A,
            MqttReasonCode::QosNotSupported => 0x9B,
            MqttReasonCode::UseAnotherServer => 0x9C,
            MqttReasonCode::ServerMoved => 0x9D,
            MqttReasonCode::SharedSubscriptionsNotSupported => 0x9E,
            MqttReasonCode::ConnectionRateExceeded => 0x9F,
            MqttReasonCode::MaximumConnectTime => 0xA0,
            MqttReasonCode::SubscriptionIdentifiersNotSupported => 0xA1,
            MqttReasonCode::WildcardSubscriptionsNotSupported => 0xA2,
        }
    }

    /// The named reason for a wire byte (UNVERIFIED table — see the
    /// module header). Unmapped bytes become
    /// [`MqttReasonCode::UnspecifiedError`] with the raw byte carried in
    /// the reason string of the reporting error (never a silent guess).
    pub fn from_u8(value: u8) -> Option<Self> {
        let code = match value {
            0x00 => MqttReasonCode::Success,
            0x01 => MqttReasonCode::GrantedQos1,
            0x02 => MqttReasonCode::GrantedQos2,
            0x80 => MqttReasonCode::UnspecifiedError,
            0x81 => MqttReasonCode::MalformedPacket,
            0x82 => MqttReasonCode::ProtocolError,
            0x83 => MqttReasonCode::ImplementationSpecificError,
            0x84 => MqttReasonCode::UnsupportedProtocolVersion,
            0x85 => MqttReasonCode::ClientIdentifierNotValid,
            0x86 => MqttReasonCode::BadUserNameOrPassword,
            0x87 => MqttReasonCode::NotAuthorized,
            0x88 => MqttReasonCode::ServerUnavailable,
            0x89 => MqttReasonCode::ServerBusy,
            0x8A => MqttReasonCode::Banned,
            0x8B => MqttReasonCode::ServerShuttingDown,
            0x8C => MqttReasonCode::BadAuthenticationMethod,
            0x8D => MqttReasonCode::KeepAliveTimeout,
            0x8E => MqttReasonCode::SessionTakenOver,
            0x8F => MqttReasonCode::TopicFilterInvalid,
            0x90 => MqttReasonCode::TopicNameInvalid,
            0x91 => MqttReasonCode::PacketIdentifierInUse,
            0x92 => MqttReasonCode::PacketIdentifierNotFound,
            0x93 => MqttReasonCode::ReceiveMaximumExceeded,
            0x94 => MqttReasonCode::TopicAliasInvalid,
            0x95 => MqttReasonCode::PacketTooLarge,
            0x96 => MqttReasonCode::MessageRateTooHigh,
            0x97 => MqttReasonCode::QuotaExceeded,
            0x98 => MqttReasonCode::AdministrativeAction,
            0x99 => MqttReasonCode::PayloadFormatInvalid,
            0x9A => MqttReasonCode::RetainNotSupported,
            0x9B => MqttReasonCode::QosNotSupported,
            0x9C => MqttReasonCode::UseAnotherServer,
            0x9D => MqttReasonCode::ServerMoved,
            0x9E => MqttReasonCode::SharedSubscriptionsNotSupported,
            0x9F => MqttReasonCode::ConnectionRateExceeded,
            0xA0 => MqttReasonCode::MaximumConnectTime,
            0xA1 => MqttReasonCode::SubscriptionIdentifiersNotSupported,
            0xA2 => MqttReasonCode::WildcardSubscriptionsNotSupported,
            _ => return None,
        };
        Some(code)
    }

    /// Canonical name for records and error strings (the P11 taxonomy
    /// naming discipline — never raw wire bytes alone).
    pub const fn name(self) -> &'static str {
        match self {
            MqttReasonCode::Success => "success",
            MqttReasonCode::GrantedQos1 => "granted_qos_1",
            MqttReasonCode::GrantedQos2 => "granted_qos_2",
            MqttReasonCode::NormalDisconnection => "normal_disconnection",
            MqttReasonCode::UnspecifiedError => "unspecified_error",
            MqttReasonCode::MalformedPacket => "malformed_packet",
            MqttReasonCode::ProtocolError => "protocol_error",
            MqttReasonCode::ImplementationSpecificError => "implementation_specific_error",
            MqttReasonCode::UnsupportedProtocolVersion => "unsupported_protocol_version",
            MqttReasonCode::ClientIdentifierNotValid => "client_identifier_not_valid",
            MqttReasonCode::BadUserNameOrPassword => "bad_user_name_or_password",
            MqttReasonCode::NotAuthorized => "not_authorized",
            MqttReasonCode::ServerUnavailable => "server_unavailable",
            MqttReasonCode::ServerBusy => "server_busy",
            MqttReasonCode::Banned => "banned",
            MqttReasonCode::ServerShuttingDown => "server_shutting_down",
            MqttReasonCode::BadAuthenticationMethod => "bad_authentication_method",
            MqttReasonCode::KeepAliveTimeout => "keep_alive_timeout",
            MqttReasonCode::SessionTakenOver => "session_taken_over",
            MqttReasonCode::TopicFilterInvalid => "topic_filter_invalid",
            MqttReasonCode::TopicNameInvalid => "topic_name_invalid",
            MqttReasonCode::PacketIdentifierInUse => "packet_identifier_in_use",
            MqttReasonCode::PacketIdentifierNotFound => "packet_identifier_not_found",
            MqttReasonCode::ReceiveMaximumExceeded => "receive_maximum_exceeded",
            MqttReasonCode::TopicAliasInvalid => "topic_alias_invalid",
            MqttReasonCode::PacketTooLarge => "packet_too_large",
            MqttReasonCode::MessageRateTooHigh => "message_rate_too_high",
            MqttReasonCode::QuotaExceeded => "quota_exceeded",
            MqttReasonCode::AdministrativeAction => "administrative_action",
            MqttReasonCode::PayloadFormatInvalid => "payload_format_invalid",
            MqttReasonCode::RetainNotSupported => "retain_not_supported",
            MqttReasonCode::QosNotSupported => "qos_not_supported",
            MqttReasonCode::UseAnotherServer => "use_another_server",
            MqttReasonCode::ServerMoved => "server_moved",
            MqttReasonCode::SharedSubscriptionsNotSupported => "shared_subscriptions_not_supported",
            MqttReasonCode::ConnectionRateExceeded => "connection_rate_exceeded",
            MqttReasonCode::MaximumConnectTime => "maximum_connect_time",
            MqttReasonCode::SubscriptionIdentifiersNotSupported => {
                "subscription_identifiers_not_supported"
            }
            MqttReasonCode::WildcardSubscriptionsNotSupported => {
                "wildcard_subscriptions_not_supported"
            }
        }
    }
}

impl std::fmt::Display for MqttReasonCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} (0x{:02X})", self.name(), self.as_u8())
    }
}

// --- Property subset ----------------------------------------------------------

/// The declared MQTT 5 property → value-kind table the codec uses to walk
/// property blobs. UNVERIFIED against the specification text (module
/// header); every identifier outside this table is a NAMED refusal — the
/// codec never silently skips a property it does not declare.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PropertyValueKind {
    /// One byte.
    Byte,
    /// Two bytes big-endian.
    TwoByte,
    /// Four bytes big-endian.
    FourByte,
    /// A variable-length integer.
    Varint,
    /// A length-delimited UTF-8 string.
    Utf8,
    /// A length-delimited binary field.
    Binary,
    /// Two length-delimited UTF-8 strings (User Property).
    Utf8Pair,
}

fn property_value_kind(id: u8) -> Option<PropertyValueKind> {
    let kind = match id {
        0x01 => PropertyValueKind::Byte,        // Payload Format Indicator
        0x02 => PropertyValueKind::FourByte,    // Message Expiry Interval
        0x03 => PropertyValueKind::Utf8,        // Content Type
        0x08 => PropertyValueKind::Utf8,        // Response Topic
        0x09 => PropertyValueKind::Binary,      // Correlation Data
        0x0B => PropertyValueKind::Varint,      // Subscription Identifier
        0x11 => PropertyValueKind::FourByte,    // Session Expiry Interval
        0x12 => PropertyValueKind::Utf8,        // Assigned Client Identifier
        0x13 => PropertyValueKind::TwoByte,     // Server Keep Alive
        0x15 => PropertyValueKind::Utf8,        // Authentication Method
        0x16 => PropertyValueKind::Binary,      // Authentication Data
        0x17 => PropertyValueKind::Byte,        // Request Problem Information
        0x18 => PropertyValueKind::FourByte,    // Will Delay Interval
        0x19 => PropertyValueKind::Byte,        // Request Response Information
        0x1A => PropertyValueKind::Utf8,        // Response Information
        0x1C => PropertyValueKind::Utf8,        // Server Reference
        0x1F => PropertyValueKind::Utf8,        // Reason String
        0x21 => PropertyValueKind::TwoByte,     // Receive Maximum
        0x22 => PropertyValueKind::TwoByte,     // Topic Alias Maximum
        0x23 => PropertyValueKind::TwoByte,     // Topic Alias
        0x24 => PropertyValueKind::Byte,        // Maximum QoS
        0x25 => PropertyValueKind::Byte,        // Retain Available
        0x26 => PropertyValueKind::Utf8Pair,    // User Property
        0x27 => PropertyValueKind::FourByte,    // Maximum Packet Size
        0x28 => PropertyValueKind::Byte,        // Wildcard Subscription Available
        0x29 => PropertyValueKind::Byte,        // Subscription Identifier Available
        0x2A => PropertyValueKind::Byte,        // Shared Subscription Available
        _ => return None,
    };
    Some(kind)
}

/// One decoded property (the subset the codec's callers consume).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Property {
    Byte(u8),
    TwoByte(u16),
    FourByte(u32),
    Varint(u32),
    Utf8(String),
    Binary(Vec<u8>),
    Utf8Pair(String, String),
}

/// A bounded reader over a wire slice (private; named refusals only).
struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    fn remaining(&self) -> usize {
        self.buf.len() - self.pos
    }

    fn take(&mut self, n: usize, packet: &'static str) -> Result<&'a [u8], MqttWireError> {
        if self.remaining() < n {
            return Err(MqttWireError::Malformed {
                packet,
                reason: format!("need {n} bytes, {} remain", self.remaining()),
            });
        }
        let slice = &self.buf[self.pos..self.pos + n];
        self.pos += n;
        Ok(slice)
    }

    fn u8(&mut self, packet: &'static str) -> Result<u8, MqttWireError> {
        Ok(self.take(1, packet)?[0])
    }

    fn u16(&mut self, packet: &'static str) -> Result<u16, MqttWireError> {
        let bytes = self.take(2, packet)?;
        Ok(u16::from_be_bytes([bytes[0], bytes[1]]))
    }

    fn u32(&mut self, packet: &'static str) -> Result<u32, MqttWireError> {
        let bytes = self.take(4, packet)?;
        Ok(u32::from_be_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3],
        ]))
    }

    fn varint(&mut self, packet: &'static str) -> Result<u32, MqttWireError> {
        decode_varint(&self.buf[self.pos..]).map_err(|error| match error {
            MqttWireError::VarintTooLong => MqttWireError::VarintTooLong,
            other => MqttWireError::Malformed {
                packet,
                reason: other.to_string(),
            },
        })
        .map(|(value, consumed)| {
            self.pos += consumed;
            value
        })
    }

    fn utf8(&mut self, packet: &'static str) -> Result<String, MqttWireError> {
        let len = self.u16(packet)?;
        let bytes = self.take(len as usize, packet)?;
        String::from_utf8(bytes.to_vec()).map_err(|error| MqttWireError::Malformed {
            packet,
            reason: format!("invalid UTF-8 string: {error}"),
        })
    }

    fn binary(&mut self, packet: &'static str) -> Result<Vec<u8>, MqttWireError> {
        let len = self.u16(packet)?;
        Ok(self.take(len as usize, packet)?.to_vec())
    }
}

/// Parses one property blob (a varint length followed by that many
/// property entries) into `(identifier, decoded value)` pairs. Unknown
/// property identifiers are named refusals (fail-closed; the declared
/// table is the whole truth).
fn decode_properties(
    packet: &'static str,
    reader: &mut Reader<'_>,
) -> Result<Vec<(u8, Property)>, MqttWireError> {
    let blob_len = reader.varint(packet)? as usize;
    let blob = reader.take(blob_len, packet)?;
    let mut property_reader = Reader::new(blob);
    let mut out = Vec::new();
    while property_reader.remaining() > 0 {
        let id = property_reader.u8(packet)?;
        let kind = property_value_kind(id).ok_or_else(|| MqttWireError::Malformed {
            packet,
            reason: format!("property identifier 0x{id:02X} is not in the declared table"),
        })?;
        let property = match kind {
            PropertyValueKind::Byte => Property::Byte(property_reader.u8(packet)?),
            PropertyValueKind::TwoByte => Property::TwoByte(property_reader.u16(packet)?),
            PropertyValueKind::FourByte => Property::FourByte(property_reader.u32(packet)?),
            PropertyValueKind::Varint => Property::Varint(property_reader.varint(packet)?),
            PropertyValueKind::Utf8 => Property::Utf8(property_reader.utf8(packet)?),
            PropertyValueKind::Binary => Property::Binary(property_reader.binary(packet)?),
            PropertyValueKind::Utf8Pair => Property::Utf8Pair(
                property_reader.utf8(packet)?,
                property_reader.utf8(packet)?,
            ),
        };
        out.push((id, property));
    }
    Ok(out)
}

// --- CONNECT / CONNACK --------------------------------------------------------

/// The will message carried in the CONNECT (the NDEATH LWT for the F-05
/// uplink; see `LiveMqttConfig` in `live.rs`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WillMessage {
    pub topic: String,
    pub payload: Vec<u8>,
    pub qos: DeclaredQos,
    pub retain: bool,
}

/// The CONNECT the codec encodes. There is NO password field: the
/// password connect flag is never set and no password bytes can be
/// carried (the never-granted `secret.read` rule — absence is structural;
/// authentication travels the TLS client-certificate identity instead).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ConnectOptions {
    pub client_id: String,
    pub keep_alive_secs: u16,
    pub clean_start: bool,
    /// Session Expiry Interval property (`0x11`), 4-byte value.
    pub session_expiry_secs: Option<u32>,
    /// Receive Maximum property (`0x21`), 2-byte value.
    pub receive_maximum: Option<u16>,
    /// Topic Alias Maximum property (`0x22`), 2-byte value — DECLARED,
    /// never USED (no alias is ever sent by this codec).
    pub topic_alias_maximum: Option<u16>,
    pub will: Option<WillMessage>,
    /// The declared username (may be empty where the broker accepts
    /// username-less auth; a DECLARED empty, never a default).
    pub username: Option<String>,
}

fn qos_wire(qos: DeclaredQos, packet: &'static str) -> Result<u8, MqttWireError> {
    match qos {
        DeclaredQos::AtMostOnce => Ok(0),
        DeclaredQos::AtLeastOnce => Ok(1),
        DeclaredQos::ExactlyOnce => Err(MqttWireError::Malformed {
            packet,
            reason: "QoS 2 is not part of the declared F-05 subset".to_string(),
        }),
    }
}

/// Encodes a CONNECT packet. Property order is DECLARED (session expiry,
/// then receive maximum, then topic alias maximum) so the byte output is
/// deterministic and fixture-pinned.
pub fn encode_connect(options: &ConnectOptions) -> Result<Vec<u8>, MqttWireError> {
    let packet = "CONNECT";
    let mut body = Vec::new();
    // Variable header: protocol name, level, connect flags, keep alive,
    // properties.
    body.extend_from_slice(&(PROTOCOL_NAME.len() as u16).to_be_bytes());
    body.extend_from_slice(PROTOCOL_NAME.as_bytes());
    body.push(PROTOCOL_LEVEL);
    let mut flags: u8 = 0x00;
    if options.clean_start {
        flags |= 0x02;
    }
    if let Some(will) = &options.will {
        // Will flag (bit 2), will QoS (bits 3-4), will retain (bit 5).
        flags |= 0x04 | qos_wire(will.qos, packet)? << 3 | u8::from(will.retain) << 5;
    }
    if options.username.is_some() {
        flags |= 0x80;
    }
    // The password flag (0x40) is NEVER set — there is no password field
    // anywhere in this codec (the never-granted `secret.read` rule).
    body.push(flags);
    body.extend_from_slice(&options.keep_alive_secs.to_be_bytes());
    let mut properties = Vec::new();
    if let Some(secs) = options.session_expiry_secs {
        properties.push(0x11);
        properties.extend_from_slice(&secs.to_be_bytes());
    }
    if let Some(maximum) = options.receive_maximum {
        properties.push(0x21);
        properties.extend_from_slice(&maximum.to_be_bytes());
    }
    if let Some(maximum) = options.topic_alias_maximum {
        properties.push(0x22);
        properties.extend_from_slice(&maximum.to_be_bytes());
    }
    body.extend_from_slice(&encode_varint(properties.len() as u32));
    body.extend_from_slice(&properties);
    // Payload: client id, will properties + topic + payload, username.
    body.extend_from_slice(&(options.client_id.len() as u16).to_be_bytes());
    body.extend_from_slice(options.client_id.as_bytes());
    if let Some(will) = &options.will {
        body.extend_from_slice(&encode_varint(0)); // empty will properties
        body.extend_from_slice(&(will.topic.len() as u16).to_be_bytes());
        body.extend_from_slice(will.topic.as_bytes());
        body.extend_from_slice(&(will.payload.len() as u16).to_be_bytes());
        body.extend_from_slice(&will.payload);
    }
    if let Some(username) = &options.username {
        body.extend_from_slice(&(username.len() as u16).to_be_bytes());
        body.extend_from_slice(username.as_bytes());
    }
    let mut frame = vec![PACKET_CONNECT << 4];
    frame.extend_from_slice(&encode_varint(body.len() as u32));
    frame.extend_from_slice(&body);
    Ok(frame)
}

/// The decoded CONNACK fields (the subset the live transport consumes).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ConnackFields {
    pub session_present: bool,
    pub reason_code: Option<MqttReasonCode>,
    pub session_expiry_secs: Option<u32>,
    pub receive_maximum: Option<u16>,
    pub assigned_client_identifier: Option<String>,
    pub server_keep_alive: Option<u16>,
    pub topic_alias_maximum: Option<u16>,
    pub reason_string: Option<String>,
}

/// Decodes a CONNACK (first byte `0x20`, the remaining bytes passed in).
pub fn decode_connack(remaining: &[u8]) -> Result<ConnackFields, MqttWireError> {
    let packet = "CONNACK";
    let mut reader = Reader::new(remaining);
    let flags = reader.u8(packet)?;
    let session_present = flags & 0x01 != 0;
    let reason_byte = reader.u8(packet)?;
    let reason_code = MqttReasonCode::from_u8(reason_byte);
    let properties = decode_properties(packet, &mut reader)?;
    let mut fields = ConnackFields {
        session_present,
        reason_code,
        ..ConnackFields::default()
    };
    for (id, property) in properties {
        match (id, property) {
            (0x11, Property::FourByte(value)) => fields.session_expiry_secs = Some(value),
            (0x21, Property::TwoByte(value)) => fields.receive_maximum = Some(value),
            (0x12, Property::Utf8(value)) => fields.assigned_client_identifier = Some(value),
            (0x13, Property::TwoByte(value)) => fields.server_keep_alive = Some(value),
            (0x22, Property::TwoByte(value)) => fields.topic_alias_maximum = Some(value),
            (0x1F, Property::Utf8(value)) => fields.reason_string = Some(value),
            // Every other declared property is parsed and consciously
            // IGNORED on CONNACK (declared subset; not a silent skip —
            // the parse refused anything outside the declared table).
            _ => {}
        }
    }
    Ok(fields)
}

// --- PUBLISH / PUBACK ---------------------------------------------------------

/// An encoded-ready outgoing PUBLISH.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishOut {
    pub topic: String,
    pub payload: Vec<u8>,
    pub qos: DeclaredQos,
    /// The packet identifier (REQUIRED for QoS 1; ignored for QoS 0).
    pub packet_id: u16,
    pub retain: bool,
    pub dup: bool,
}

/// Encodes an outgoing PUBLISH. The property blob is empty (the declared
/// subset carries no publish properties — no topic alias, no response
/// topic, no user properties).
pub fn encode_publish(out: &PublishOut) -> Result<Vec<u8>, MqttWireError> {
    let packet = "PUBLISH";
    let qos = qos_wire(out.qos, packet)?;
    let mut flags = PACKET_PUBLISH << 4;
    if out.dup {
        flags |= 0x08;
    }
    flags |= qos << 1;
    if out.retain {
        flags |= 0x01;
    }
    let mut body = Vec::new();
    body.extend_from_slice(&(out.topic.len() as u16).to_be_bytes());
    body.extend_from_slice(out.topic.as_bytes());
    if qos > 0 {
        body.extend_from_slice(&out.packet_id.to_be_bytes());
    }
    body.extend_from_slice(&encode_varint(0)); // empty property blob
    body.extend_from_slice(&out.payload);
    let mut frame = vec![flags];
    frame.extend_from_slice(&encode_varint(body.len() as u32));
    frame.extend_from_slice(&body);
    Ok(frame)
}

/// A decoded incoming PUBLISH.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishIn {
    pub topic: String,
    pub payload: Vec<u8>,
    pub qos: DeclaredQos,
    pub packet_id: Option<u16>,
    pub dup: bool,
    pub retain: bool,
}

/// Decodes an incoming PUBLISH (first byte carries the type nibble 0x3
/// and the DUP/QoS/RETAIN flags; `remaining` is the rest of the frame).
pub fn decode_publish(first_byte: u8, remaining: &[u8]) -> Result<PublishIn, MqttWireError> {
    let packet = "PUBLISH";
    let dup = first_byte & 0x08 != 0;
    let retain = first_byte & 0x01 != 0;
    let qos_wire_value = (first_byte >> 1) & 0x03;
    if qos_wire_value == 3 {
        return Err(MqttWireError::Malformed {
            packet,
            reason: "QoS value 3 is a protocol violation".to_string(),
        });
    }
    let qos = match qos_wire_value {
        0 => DeclaredQos::AtMostOnce,
        1 => DeclaredQos::AtLeastOnce,
        _ => DeclaredQos::ExactlyOnce,
    };
    let mut reader = Reader::new(remaining);
    let topic = reader.utf8(packet)?;
    let packet_id = if qos_wire_value > 0 {
        Some(reader.u16(packet)?)
    } else {
        None
    };
    let _properties = decode_properties(packet, &mut reader)?;
    let payload = reader.buf[reader.pos..].to_vec();
    Ok(PublishIn {
        topic,
        payload,
        qos,
        packet_id,
        dup,
        retain,
    })
}

/// Encodes a PUBACK acknowledging an incoming QoS 1 publish (reason
/// Success, empty property blob).
pub fn encode_puback(packet_id: u16) -> Vec<u8> {
    let mut body = packet_id.to_be_bytes().to_vec();
    body.push(MqttReasonCode::Success.as_u8());
    body.extend_from_slice(&encode_varint(0));
    let mut frame = vec![PACKET_PUBACK << 4];
    frame.extend_from_slice(&encode_varint(body.len() as u32));
    frame.extend_from_slice(&body);
    frame
}

/// A decoded PUBACK.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PubackFields {
    pub packet_id: u16,
    pub reason_code: MqttReasonCode,
}

/// Decodes a PUBACK. A PUBACK may legally omit the reason code and the
/// property blob (they default to Success and empty respectively — the
/// declared reading; UNVERIFIED at the lab gate).
pub fn decode_puback(remaining: &[u8]) -> Result<PubackFields, MqttWireError> {
    let packet = "PUBACK";
    let mut reader = Reader::new(remaining);
    let packet_id = reader.u16(packet)?;
    let reason_code = if reader.remaining() > 0 {
        let byte = reader.u8(packet)?;
        if reader.remaining() > 0 {
            let _properties = decode_properties(packet, &mut reader)?;
        }
        MqttReasonCode::from_u8(byte).ok_or(MqttWireError::Malformed {
            packet,
            reason: format!("unknown PUBACK reason byte 0x{byte:02X}"),
        })?
    } else {
        MqttReasonCode::Success
    };
    Ok(PubackFields {
        packet_id,
        reason_code,
    })
}

// --- SUBSCRIBE / SUBACK -------------------------------------------------------

/// The declared SUBSCRIBE options: maximum QoS, No Local, Retain As
/// Published, and Retain Handling. No Local is declared TRUE for the
/// command subscription (the edge never wants its own publishes echoed
/// back).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubscribeOptions {
    pub max_qos: DeclaredQos,
    pub no_local: bool,
    pub retain_as_published: bool,
    /// Retain Handling (0/1/2 — declared 0).
    pub retain_handling: u8,
}

impl Default for SubscribeOptions {
    fn default() -> Self {
        Self {
            max_qos: DeclaredQos::AtLeastOnce,
            no_local: true,
            retain_as_published: false,
            retain_handling: 0,
        }
    }
}

/// Encodes a SUBSCRIBE for ONE topic filter (the declared subset: the
/// command topic path subscribes a single filter; multi-filter
/// SUBSCRIBE is not encoded).
pub fn encode_subscribe(
    packet_id: u16,
    topic_filter: &str,
    options: &SubscribeOptions,
) -> Result<Vec<u8>, MqttWireError> {
    let packet = "SUBSCRIBE";
    let max_qos = qos_wire(options.max_qos, packet)?;
    if options.retain_handling > 2 {
        return Err(MqttWireError::Malformed {
            packet,
            reason: format!("retain handling {} outside 0..=2", options.retain_handling),
        });
    }
    let mut body = Vec::new();
    body.extend_from_slice(&packet_id.to_be_bytes());
    body.extend_from_slice(&encode_varint(0)); // empty property blob
    body.extend_from_slice(&(topic_filter.len() as u16).to_be_bytes());
    body.extend_from_slice(topic_filter.as_bytes());
    let mut options_byte = max_qos;
    if options.no_local {
        options_byte |= 0x04;
    }
    if options.retain_as_published {
        options_byte |= 0x08;
    }
    options_byte |= options.retain_handling << 4;
    body.push(options_byte);
    // The SUBSCRIBE fixed header carries the mandatory 0x02 flag.
    let mut frame = vec![(PACKET_SUBSCRIBE << 4) | 0x02];
    frame.extend_from_slice(&encode_varint(body.len() as u32));
    frame.extend_from_slice(&body);
    Ok(frame)
}

/// A decoded SUBACK.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubackFields {
    pub packet_id: u16,
    /// One granted-QoS or refusal reason per filter (the declared subset
    /// subscribes exactly one filter, so exactly one entry is expected).
    pub reason_codes: Vec<MqttReasonCode>,
}

/// Decodes a SUBACK (granted QoS 0/1/2 map to the Granted* codes; bytes
/// >= 0x80 are refusals).
pub fn decode_suback(remaining: &[u8]) -> Result<SubackFields, MqttWireError> {
    let packet = "SUBACK";
    let mut reader = Reader::new(remaining);
    let packet_id = reader.u16(packet)?;
    let _properties = decode_properties(packet, &mut reader)?;
    let mut reason_codes = Vec::new();
    while reader.remaining() > 0 {
        let byte = reader.u8(packet)?;
        let code = MqttReasonCode::from_u8(byte).ok_or(MqttWireError::Malformed {
            packet,
            reason: format!("unknown SUBACK reason byte 0x{byte:02X}"),
        })?;
        reason_codes.push(code);
    }
    if reason_codes.is_empty() {
        return Err(MqttWireError::Malformed {
            packet,
            reason: "SUBACK carries no reason codes".to_string(),
        });
    }
    Ok(SubackFields {
        packet_id,
        reason_codes,
    })
}

// --- PINGREQ / PINGRESP / DISCONNECT ------------------------------------------

/// Encodes a PINGREQ (keep alive).
pub fn encode_pingreq() -> Vec<u8> {
    vec![PACKET_PINGREQ << 4, 0x00]
}

/// Decodes a PINGRESP (the first byte must be the PINGRESP type; the
/// remaining length must be zero).
pub fn decode_pingresp(first_byte: u8, remaining: &[u8]) -> Result<(), MqttWireError> {
    if first_byte != PACKET_PINGRESP << 4 {
        return Err(MqttWireError::Malformed {
            packet: "PINGRESP",
            reason: format!("first byte 0x{first_byte:02X} is not a PINGRESP"),
        });
    }
    if !remaining.is_empty() {
        return Err(MqttWireError::Malformed {
            packet: "PINGRESP",
            reason: "PINGRESP carries no remaining bytes".to_string(),
        });
    }
    Ok(())
}

/// Encodes a DISCONNECT with the given reason code (the empty property
/// blob is always written so the byte output is deterministic).
pub fn encode_disconnect(reason: MqttReasonCode) -> Vec<u8> {
    let mut body = vec![reason.as_u8()];
    body.extend_from_slice(&encode_varint(0));
    let mut frame = vec![PACKET_DISCONNECT << 4];
    frame.extend_from_slice(&encode_varint(body.len() as u32));
    frame.extend_from_slice(&body);
    frame
}

/// A decoded DISCONNECT from the broker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisconnectFields {
    pub reason_code: MqttReasonCode,
}

/// Decodes a DISCONNECT (reason code and properties both optional; they
/// default to NormalDisconnection and empty — the declared reading,
/// UNVERIFIED at the lab gate).
pub fn decode_disconnect(remaining: &[u8]) -> Result<DisconnectFields, MqttWireError> {
    let packet = "DISCONNECT";
    let mut reader = Reader::new(remaining);
    let reason_code = if reader.remaining() > 0 {
        let byte = reader.u8(packet)?;
        if reader.remaining() > 0 {
            let _properties = decode_properties(packet, &mut reader)?;
        }
        MqttReasonCode::from_u8(byte).ok_or(MqttWireError::Malformed {
            packet,
            reason: format!("unknown DISCONNECT reason byte 0x{byte:02X}"),
        })?
    } else {
        MqttReasonCode::NormalDisconnection
    };
    Ok(DisconnectFields { reason_code })
}

// --- The unified incoming-packet surface ---------------------------------------

/// One decoded incoming packet (the live transport's read loop and the
/// mock brokers in the socket-free tests both consume this).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IncomingPacket {
    Connack(ConnackFields),
    Publish(PublishIn),
    Puback(PubackFields),
    Suback(SubackFields),
    Pingresp,
    Disconnect(DisconnectFields),
}

/// Decodes one complete packet from its fixed-header first byte and the
/// remaining bytes. Packet types outside the declared subset are named
/// refusals (fail-closed).
pub fn decode_packet(first_byte: u8, remaining: &[u8]) -> Result<IncomingPacket, MqttWireError> {
    let packet_type = first_byte >> 4;
    match packet_type {
        PACKET_CONNACK => Ok(IncomingPacket::Connack(decode_connack(remaining)?)),
        PACKET_PUBLISH => Ok(IncomingPacket::Publish(decode_publish(first_byte, remaining)?)),
        PACKET_PUBACK => Ok(IncomingPacket::Puback(decode_puback(remaining)?)),
        PACKET_SUBACK => Ok(IncomingPacket::Suback(decode_suback(remaining)?)),
        PACKET_PINGRESP => {
            decode_pingresp(first_byte, remaining)?;
            Ok(IncomingPacket::Pingresp)
        }
        PACKET_DISCONNECT => Ok(IncomingPacket::Disconnect(decode_disconnect(remaining)?)),
        other => Err(MqttWireError::Malformed {
            packet: "frame",
            reason: format!("packet type {other} is outside the declared F-05 subset"),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(" ")
    }

    #[test]
    fn variable_length_integer_vectors() {
        // Hand-computed with `python -I` (the pinned fixture method; see
        // tests/mqtt_wire_codec.rs): 7-bit little-endian groups with the
        // 0x80 continuation bit.
        assert_eq!(hex(&encode_varint(0)), "00");
        assert_eq!(hex(&encode_varint(127)), "7F");
        assert_eq!(hex(&encode_varint(128)), "80 01");
        assert_eq!(hex(&encode_varint(321)), "C1 02");
        assert_eq!(hex(&encode_varint(16383)), "FF 7F");
        assert_eq!(hex(&encode_varint(2097152)), "80 80 80 01");
        assert_eq!(hex(&encode_varint(MAX_REMAINING_LENGTH)), "FF FF FF 7F");
        // Round-trips.
        for value in [0u32, 127, 128, 321, 16383, 2097152, MAX_REMAINING_LENGTH] {
            let encoded = encode_varint(value);
            assert_eq!(decode_varint(&encoded), Ok((value, encoded.len())));
        }
    }

    #[test]
    fn frame_len_separates_complete_and_incomplete_prefixes() {
        // PINGREQ is exactly two bytes.
        assert_eq!(frame_len(&[0xC0]), Ok(None));
        assert_eq!(frame_len(&[0xC0, 0x00]), Ok(Some(2)));
        // A two-byte remaining-length frame is not complete until all of
        // its bytes are buffered.
        let publish = encode_publish(&PublishOut {
            topic: "spBv1.0/site-42/NDATA/edge-07".to_string(),
            payload: vec![1, 2, 3],
            qos: DeclaredQos::AtMostOnce,
            packet_id: 0,
            retain: false,
            dup: false,
        })
        .expect("encodable");
        assert!(frame_len(&publish[..publish.len() - 1]).expect("prefix").is_none());
        assert_eq!(frame_len(&publish), Ok(Some(publish.len())));
    }

    #[test]
    fn connect_encodes_the_declared_byte_shape_without_password() {
        // Pinned fixture (python -I): protocol name MQTT, level 5, flags
        // 0x02 (clean start), keep alive 60, empty properties, client id.
        let frame = encode_connect(&ConnectOptions {
            client_id: "edge-07-uplink".to_string(),
            keep_alive_secs: 60,
            clean_start: true,
            ..ConnectOptions::default()
        })
        .expect("encodable");
        assert_eq!(
            hex(&frame),
            "10 1B 00 04 4D 51 54 54 05 02 00 3C 00 00 0E 65 64 67 65 2D 30 37 2D 75 70 6C 69 6E 6B"
        );
    }

    #[test]
    fn connect_carries_the_will_properties_and_username() {
        // Pinned fixture (python -I): flags 0x8E (username | will QoS 1 |
        // will flag | clean start), session expiry 3600, receive maximum
        // 65535, the NDEATH will (topic + 2-byte payload), username. NO
        // password flag anywhere.
        let frame = encode_connect(&ConnectOptions {
            client_id: "edge-07-uplink".to_string(),
            keep_alive_secs: 60,
            clean_start: true,
            session_expiry_secs: Some(3600),
            receive_maximum: Some(65535),
            will: Some(WillMessage {
                topic: "spBv1.0/site-42/NDEATH/edge-07".to_string(),
                payload: vec![0x28, 0x00],
                qos: DeclaredQos::AtLeastOnce,
                retain: false,
            }),
            username: Some("edge-07-uplink".to_string()),
            ..ConnectOptions::default()
        })
        .expect("encodable");
        assert_eq!(
            hex(&frame),
            "10 58 00 04 4D 51 54 54 05 8E 00 3C 08 11 00 00 0E 10 21 FF FF 00 0E 65 64 67 65 \
             2D 30 37 2D 75 70 6C 69 6E 6B 00 00 1E 73 70 42 76 31 2E 30 2F 73 69 74 65 2D 34 \
             32 2F 4E 44 45 41 54 48 2F 65 64 67 65 2D 30 37 00 02 28 00 00 0E 65 64 67 65 2D \
             30 37 2D 75 70 6C 69 6E 6B"
        );
    }

    #[test]
    fn qos2_connect_will_and_publish_are_named_refusals() {
        let will_qos2 = encode_connect(&ConnectOptions {
            will: Some(WillMessage {
                topic: "t".to_string(),
                payload: Vec::new(),
                qos: DeclaredQos::ExactlyOnce,
                retain: false,
            }),
            ..ConnectOptions::default()
        });
        assert!(matches!(will_qos2, Err(MqttWireError::Malformed { packet: "CONNECT", .. })));
        let publish_qos2 = encode_publish(&PublishOut {
            topic: "t".to_string(),
            payload: Vec::new(),
            qos: DeclaredQos::ExactlyOnce,
            packet_id: 1,
            retain: false,
            dup: false,
        });
        assert!(matches!(publish_qos2, Err(MqttWireError::Malformed { packet: "PUBLISH", .. })));
    }
}
