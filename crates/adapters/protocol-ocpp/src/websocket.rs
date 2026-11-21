// FIM-F02.01 — The hand-written RFC 6455 SERVER-side WebSocket codec.
//
// HONESTY HEADER (FIM standing decision 1,
// docs/releases/implementation_ledger/README.md): this module is a
// hand-written minimal codec implementing the RFC 6455 subset the OCPP-J
// transport needs — the HTTP/1.1 Upgrade handshake, the frame codec
// (FIN/opcode/mask, extended lengths 126/127), client-frame masking
// enforcement, ping/pong, the close handshake with codes, and bounded
// fragmentation reassembly. RFC 6455 CONFORMANCE AGAINST REAL PEERS IS A
// HUMAN LAB GATE: nothing here has ever run against a browser, a charge
// point, or an interop test suite, and no conformance or
// interoperability claim of any kind is made (conventions §1.3/§7.3;
// WRITTEN-UNRUN throughout).
//
// What is deliberately NOT implemented:
//
// - **No permessage-deflate.** The extension is refused in the handshake
//   response by OMITTING the |Sec-WebSocket-Extensions| header entirely
//   (RFC 6455 §4.2.2: a server not including the extension in its
//   response has declined it). No extension is ever negotiated.
// - **No TLS.** The transport serves plaintext RFC 6455 in the declared
//   LAB posture (transport.rs); Profile 3 mTLS termination is a HUMAN LAB
//   GATE. There is no flag that disables TLS, because no TLS was ever
//   enabled.
// - **No client role.** The CSMS is the server role (phase-17 context §3
//   constraint 2); this codec only decodes masked CLIENT frames and only
//   encodes UNMASKED server frames. The mask parameter of
//   [`encode_frame`] exists for the pinned tests (which simulate a
//   client), never for the shipped write path.
//
// UNVERIFIED-ASSUMPTION register (the phase-17 plan §7.5 discipline; the
// RFC 6455 text itself is the implementation-time check, and the OCA
// OCPP-J transport part is the check for how it is applied):
//
// 1. The server MUST close the connection with 1002 on an unmasked client
//    data frame (RFC 6455 §5.1: "a client MUST mask all frames... a
//    server MUST close the connection upon receiving a frame that is not
//    masked") — implemented as `UnmaskedClientFrame`.
// 2. Non-minimal length encodings (a 16-bit length below 126, a 64-bit
//    length below 65536) are protocol errors (RFC 6455 §5.2 "minimal
//    number of bytes") — refused with `NonMinimalLengthEncoding`.
// 3. The identity of the charge point travels in the URL request target
//    (`/<charge-point-id>`, per the OCPP-J rules the session register
//    cites [S-121]) — the transport layer (transport.rs) extracts it;
//    the exact path rules are UNVERIFIED against the OCA text.
// 4. OCPP-J carries JSON as TEXT frames; binary data frames are refused
//    with close code 1003 (unsupported data), never guessed at.
//
// All bounds below are DECLARED adapter sanity bounds (the crate's
// "sanity bound, not a protocol limit claim" pattern — see
// `framing.rs` `UniqueId` and `model.rs` `ChargePointId`).

use std::fmt;

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::session::{self, Subprotocol};

/// The RFC 6455 magic GUID appended to the client key for the
/// |Sec-WebSocket-Accept| value (RFC 6455 §1.3 / §4.2.2).
pub const WEBSOCKET_GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";

/// The only WebSocket version this transport accepts (RFC 6455 §4.2.1:
/// |Sec-WebSocket-Version: 13|).
pub const WEBSOCKET_VERSION: &str = "13";

/// The declared cap on the HTTP request head the handshake reads (a
/// sanity bound against unbounded input, not a protocol limit claim).
pub const MAX_HTTP_REQUEST_HEAD_BYTES: usize = 16 * 1024;

/// The declared cap on ONE assembled WebSocket message (a sanity bound,
/// not a protocol limit claim). OCPP-J JSON messages are small; 1 MiB is
/// a generous gate against unbounded fragmentation.
pub const MAX_WEBSOCKET_MESSAGE_BYTES: usize = 1024 * 1024;

/// Close-code constants used by this transport (RFC 6455 §7.4.1).
pub const CLOSE_NORMAL: u16 = 1000;
pub const CLOSE_PROTOCOL_ERROR: u16 = 1002;
pub const CLOSE_UNSUPPORTED_DATA: u16 = 1003;
pub const CLOSE_INVALID_PAYLOAD: u16 = 1007;
pub const CLOSE_POLICY_VIOLATION: u16 = 1008;
pub const CLOSE_MESSAGE_TOO_BIG: u16 = 1009;
pub const CLOSE_INTERNAL_ERROR: u16 = 1011;

/// Whether a received close code is one this transport accepts as
/// meaningful (RFC 6455 §7.4.1: 1000-1003, 1007-1011; 1004/1005/1006 and
/// the 3xxx/4xxx ranges are reserved/status-only and never appear on the
/// wire in a frame this codec generated).
pub const fn is_valid_received_close_code(code: u16) -> bool {
    matches!(code, 1000..=1003 | 1007..=1011)
}

// --- Opcode -----------------------------------------------------------------

/// The RFC 6455 frame opcode subset this codec handles (§5.2). Reserved
/// opcodes parse as errors, never as unknown-but-ignored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Opcode {
    Continuation,
    Text,
    Binary,
    Close,
    Ping,
    Pong,
}

impl Opcode {
    /// Parses the opcode nibble. Reserved values (0xB-0xF) are named
    /// errors — never silently ignored (fail-closed).
    pub const fn from_byte(byte: u8) -> Result<Self, FrameError> {
        match byte {
            0x0 => Ok(Opcode::Continuation),
            0x1 => Ok(Opcode::Text),
            0x2 => Ok(Opcode::Binary),
            0x8 => Ok(Opcode::Close),
            0x9 => Ok(Opcode::Ping),
            0xA => Ok(Opcode::Pong),
            _ => Err(FrameError::ReservedOpcode { opcode: byte }),
        }
    }

    /// Whether this opcode is a control frame (RFC 6455 §5.5).
    pub const fn is_control(self) -> bool {
        matches!(self, Opcode::Close | Opcode::Ping | Opcode::Pong)
    }
}

impl fmt::Display for Opcode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Opcode::Continuation => "continuation",
            Opcode::Text => "text",
            Opcode::Binary => "binary",
            Opcode::Close => "close",
            Opcode::Ping => "ping",
            Opcode::Pong => "pong",
        };
        write!(f, "{name}")
    }
}

// --- Frame codec ------------------------------------------------------------

/// Frame-decoding and -encoding errors. Every failure is named; nothing
/// is silently repaired (the whole-refusal discipline).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameError {
    /// A client data frame arrived WITHOUT a mask — RFC 6455 §5.1
    /// requires the connection to fail with 1002.
    UnmaskedClientFrame,
    /// The RSV1-3 bits were set; no extension was ever negotiated, so
    /// reserved bits are a protocol error (1002).
    ReservedBitsSet,
    /// The opcode is in the reserved range.
    ReservedOpcode { opcode: u8 },
    /// The declared payload length exceeds the declared frame cap.
    FrameTooBig { max: usize, found: usize },
    /// The buffer does not hold a complete frame (the async reader calls
    /// again with more bytes; a caller that never completes the frame is
    /// the only way this surfaces).
    IncompleteFrame { needed: usize, have: usize },
    /// A length encoding was not minimal (RFC 6455 §5.2).
    NonMinimalLengthEncoding,
    /// A control frame carried a fragmented or over-length payload
    /// (RFC 6455 §5.5: control frames MUST NOT be fragmented and their
    /// payload MUST be <= 125 bytes).
    ControlFrameViolation { reason: &'static str },
    /// A continuation arrived with no fragment in progress.
    UnexpectedContinuation,
    /// A new text/binary fragment started while one was in progress.
    NewFragmentWhileFragmenting,
    /// A binary data frame arrived — OCPP-J is JSON-over-TEXT only
    /// (module register item 4); refused with 1003, never guessed at.
    BinaryDataRefused,
    /// An assembled message was not valid UTF-8 (close 1007).
    InvalidUtf8,
    /// An assembled message exceeded the declared message cap (close
    /// 1009).
    MessageTooBig { max: usize, found: usize },
    /// A close frame's payload is malformed (bad length, bad UTF-8
    /// reason, or a reserved code).
    InvalidCloseFrame { reason: &'static str },
}

impl fmt::Display for FrameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FrameError::UnmaskedClientFrame => write!(
                f,
                "a client data frame arrived unmasked; the connection fails with close code \
                 {CLOSE_PROTOCOL_ERROR} (RFC 6455 §5.1)"
            ),
            FrameError::ReservedBitsSet => write!(
                f,
                "RSV bits set without a negotiated extension; the connection fails with \
                 close code {CLOSE_PROTOCOL_ERROR}"
            ),
            FrameError::ReservedOpcode { opcode } => {
                write!(f, "reserved opcode 0x{opcode:X} is refused, never ignored")
            }
            FrameError::FrameTooBig { max, found } => {
                write!(f, "frame payload of {found} bytes exceeds the declared cap {max}")
            }
            FrameError::IncompleteFrame { needed, have } => {
                write!(f, "incomplete frame: {needed} bytes needed, {have} available")
            }
            FrameError::NonMinimalLengthEncoding => write!(
                f,
                "non-minimal payload-length encoding (RFC 6455 §5.2); refused with close \
                 code {CLOSE_PROTOCOL_ERROR}"
            ),
            FrameError::ControlFrameViolation { reason } => {
                write!(f, "control-frame violation: {reason}")
            }
            FrameError::UnexpectedContinuation => write!(
                f,
                "a continuation frame arrived with no fragment in progress; refused with \
                 close code {CLOSE_PROTOCOL_ERROR}"
            ),
            FrameError::NewFragmentWhileFragmenting => write!(
                f,
                "a new data fragment started while a fragmented message was in progress; \
                 refused with close code {CLOSE_PROTOCOL_ERROR}"
            ),
            FrameError::BinaryDataRefused => write!(
                f,
                "a binary data frame arrived; OCPP-J is JSON-over-TEXT only — refused with \
                 close code {CLOSE_UNSUPPORTED_DATA}, never guessed at"
            ),
            FrameError::InvalidUtf8 => write!(
                f,
                "an assembled message is not valid UTF-8; refused with close code \
                 {CLOSE_INVALID_PAYLOAD}"
            ),
            FrameError::MessageTooBig { max, found } => write!(
                f,
                "assembled message of {found} bytes exceeds the declared cap {max}; refused \
                 with close code {CLOSE_MESSAGE_TOO_BIG}"
            ),
            FrameError::InvalidCloseFrame { reason } => {
                write!(f, "malformed close frame: {reason}")
            }
        }
    }
}

impl std::error::Error for FrameError {}

/// The close code the wire should carry for a frame-level error (the
/// codec's named mapping — module register items 1-2, 4).
pub const fn close_code_for(error: &FrameError) -> u16 {
    match error {
        FrameError::UnmaskedClientFrame
        | FrameError::ReservedBitsSet
        | FrameError::ReservedOpcode { .. }
        | FrameError::NonMinimalLengthEncoding
        | FrameError::ControlFrameViolation { .. }
        | FrameError::UnexpectedContinuation
        | FrameError::NewFragmentWhileFragmenting
        | FrameError::InvalidCloseFrame { .. } => CLOSE_PROTOCOL_ERROR,
        FrameError::FrameTooBig { .. } | FrameError::MessageTooBig { .. } => CLOSE_MESSAGE_TOO_BIG,
        FrameError::BinaryDataRefused => CLOSE_UNSUPPORTED_DATA,
        FrameError::InvalidUtf8 => CLOSE_INVALID_PAYLOAD,
        FrameError::IncompleteFrame { .. } => CLOSE_INTERNAL_ERROR,
    }
}

/// A decoded frame (payload already unmasked).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawFrame {
    pub fin: bool,
    pub opcode: Opcode,
    pub payload: Vec<u8>,
}

/// Encodes one frame. `mask` is `None` on the shipped server write path
/// (server frames are NEVER masked, RFC 6455 §5.1) and `Some(key)` only
/// in the pinned tests, which simulate the client side.
pub fn encode_frame(fin: bool, opcode: Opcode, payload: &[u8], mask: Option<[u8; 4]>) -> Vec<u8> {
    let mut out = Vec::with_capacity(payload.len() + 14);
    let first = (if fin { 0x80 } else { 0x00 })
        | match opcode {
            Opcode::Continuation => 0x0u8,
            Opcode::Text => 0x1,
            Opcode::Binary => 0x2,
            Opcode::Close => 0x8,
            Opcode::Ping => 0x9,
            Opcode::Pong => 0xA,
        };
    out.push(first);
    let mask_bit = if mask.is_some() { 0x80u8 } else { 0x00 };
    let len = payload.len();
    if len < 126 {
        out.push(mask_bit | len as u8);
    } else if len <= u16::MAX as usize {
        out.push(mask_bit | 126);
        out.extend_from_slice(&(len as u16).to_be_bytes());
    } else {
        out.push(mask_bit | 127);
        out.extend_from_slice(&(len as u64).to_be_bytes());
    }
    if let Some(key) = mask {
        out.extend_from_slice(&key);
        let mut masked = payload.to_vec();
        apply_mask(&mut masked, key);
        out.extend_from_slice(&masked);
    } else {
        out.extend_from_slice(payload);
    }
    out
}

/// Encodes a server TEXT frame (FIN set, unmasked — the shipped OCPP-J
/// write path).
pub fn encode_server_text(text: &str) -> Vec<u8> {
    encode_frame(true, Opcode::Text, text.as_bytes(), None)
}

/// Encodes a server PONG frame with the echoed ping payload (unmasked).
pub fn encode_pong(echo_payload: &[u8]) -> Vec<u8> {
    encode_frame(true, Opcode::Pong, echo_payload, None)
}

/// Encodes a server CLOSE frame with the named code and a human-safe
/// reason (unmasked). The reason is truncated to the RFC 6455 §5.5.1
/// 123-byte bound — a declared adapter truncation, never a silent loss of
/// protocol meaning (the code is the meaning).
pub fn encode_close(code: u16, reason: &str) -> Vec<u8> {
    let mut payload = code.to_be_bytes().to_vec();
    let reason_bytes = reason.as_bytes();
    if reason_bytes.len() > 123 {
        // Truncate on a UTF-8 character boundary (never mid-character).
        let mut end = 123;
        while !reason.is_char_boundary(end) {
            end -= 1;
        }
        payload.extend_from_slice(&reason_bytes[..end]);
    } else {
        payload.extend_from_slice(reason_bytes);
    }
    encode_frame(true, Opcode::Close, &payload, None)
}

/// XORs the masking key over the payload IN PLACE (RFC 6455 §5.3; the
/// operation is its own inverse).
pub fn apply_mask(payload: &mut [u8], key: [u8; 4]) {
    for (i, byte) in payload.iter_mut().enumerate() {
        *byte ^= key[i % 4];
    }
}

/// The parsed frame header: everything needed to size the payload read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameHeader {
    pub fin: bool,
    pub opcode: Opcode,
    /// Whether the sender masked the payload (client frames: MUST be
    /// true — register item 1).
    pub masked: bool,
    pub mask_key: Option<[u8; 4]>,
    pub payload_len: usize,
    /// Total bytes consumed by the header (including the mask key).
    pub header_len: usize,
}

/// Parses a frame header from the start of `buffer`. Requires at least
/// the header bytes to be present; a short buffer is a named error.
pub fn parse_frame_header(buffer: &[u8]) -> Result<FrameHeader, FrameError> {
    if buffer.len() < 2 {
        return Err(FrameError::IncompleteFrame { needed: 2, have: buffer.len() });
    }
    let fin = buffer[0] & 0x80 != 0;
    if buffer[0] & 0x70 != 0 {
        return Err(FrameError::ReservedBitsSet);
    }
    let opcode = Opcode::from_byte(buffer[0] & 0x0F)?;
    let masked = buffer[1] & 0x80 != 0;
    let len7 = buffer[1] & 0x7F;
    let mut offset = 2usize;
    let payload_len: usize = match len7 {
        len if len < 126 => len as usize,
        126 => {
            if buffer.len() < offset + 2 {
                return Err(FrameError::IncompleteFrame {
                    needed: offset + 2,
                    have: buffer.len(),
                });
            }
            let extended = u16::from_be_bytes([buffer[2], buffer[3]]) as usize;
            if extended < 126 {
                return Err(FrameError::NonMinimalLengthEncoding);
            }
            offset += 2;
            extended
        }
        _ => {
            if buffer.len() < offset + 8 {
                return Err(FrameError::IncompleteFrame {
                    needed: offset + 8,
                    have: buffer.len(),
                });
            }
            let mut bytes = [0u8; 8];
            bytes.copy_from_slice(&buffer[2..10]);
            let extended = u64::from_be_bytes(bytes);
            if extended < 65_536 {
                return Err(FrameError::NonMinimalLengthEncoding);
            }
            offset += 8;
            usize::try_from(extended)
                .map_err(|_| FrameError::FrameTooBig { max: usize::MAX, found: usize::MAX })?
        }
    };
    if opcode.is_control() {
        if !fin {
            return Err(FrameError::ControlFrameViolation {
                reason: "control frames must not be fragmented (RFC 6455 §5.5)",
            });
        }
        if payload_len > 125 {
            return Err(FrameError::ControlFrameViolation {
                reason: "control-frame payload exceeds 125 bytes (RFC 6455 §5.5)",
            });
        }
    }
    let mask_key = if masked {
        if buffer.len() < offset + 4 {
            return Err(FrameError::IncompleteFrame {
                needed: offset + 4,
                have: buffer.len(),
            });
        }
        let mut key = [0u8; 4];
        key.copy_from_slice(&buffer[offset..offset + 4]);
        offset += 4;
        Some(key)
    } else {
        None
    };
    Ok(FrameHeader {
        fin,
        opcode,
        masked,
        mask_key,
        payload_len,
        header_len: offset,
    })
}

/// Parses a COMPLETE frame from `buffer` (header + full payload,
/// unmasked when the sender masked it). This is the pure core the async
/// reader drives; the pinned tests exercise it byte-for-byte.
pub fn parse_frame_bytes(
    buffer: &[u8],
    max_frame_payload_bytes: usize,
) -> Result<RawFrame, FrameError> {
    let header = parse_frame_header(buffer)?;
    let total = header.header_len + header.payload_len;
    if buffer.len() < total {
        return Err(FrameError::IncompleteFrame { needed: total, have: buffer.len() });
    }
    if header.payload_len > max_frame_payload_bytes {
        return Err(FrameError::FrameTooBig {
            max: max_frame_payload_bytes,
            found: header.payload_len,
        });
    }
    if !header.masked {
        // The server role only decodes CLIENT frames; an unmasked client
        // frame fails the connection with 1002 (register item 1).
        return Err(FrameError::UnmaskedClientFrame);
    }
    let mut payload = buffer[header.header_len..total].to_vec();
    apply_mask(
        &mut payload,
        header
            .mask_key
            .expect("masked frames carry a key (checked above)"),
    );
    Ok(RawFrame {
        fin: header.fin,
        opcode: header.opcode,
        payload,
    })
}

/// Parses a close-frame payload into (code, reason). An empty payload is
/// the RFC 6455 §5.5.1 "no status code" form, represented internally as
/// 1005 (a status-only code that is never generated onto the wire by this
/// codec). A malformed payload (length 1, invalid UTF-8 reason, a code
/// outside the meaningful set) is a named error.
pub fn parse_close_payload(payload: &[u8]) -> Result<(u16, String), FrameError> {
    match payload.len() {
        0 => Ok((1005, String::new())),
        1 => Err(FrameError::InvalidCloseFrame {
            reason: "a close payload of exactly 1 byte is invalid (RFC 6455 §7.4.1)",
        }),
        _ => {
            let code = u16::from_be_bytes([payload[0], payload[1]]);
            if !is_valid_received_close_code(code) {
                return Err(FrameError::InvalidCloseFrame {
                    reason: "the close code is not in the meaningful received set",
                });
            }
            let reason = std::str::from_utf8(&payload[2..])
                .map_err(|_| FrameError::InvalidCloseFrame {
                    reason: "the close reason is not valid UTF-8",
                })?
                .to_string();
            Ok((code, reason))
        }
    }
}

// --- Bounded fragmentation reassembly ----------------------------------------

/// One assembled wire event the async read loop acts on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssemblerEvent {
    /// A complete TEXT message (the OCPP-J wire unit).
    Text(String),
    /// A PING: the transport answers with a PONG echoing the payload.
    Ping(Vec<u8>),
    /// A PONG: liveness evidence only — carried, never acted on.
    Pong(Vec<u8>),
    /// A CLOSE: the peer initiated the close handshake.
    Close { code: u16, reason: String },
}

/// The bounded fragmentation reassembler (RFC 6455 §5.4 / §5.6). Control
/// frames may interleave with fragments; a new data fragment may not
/// start while one is in progress; the assembled message is capped by the
/// declared bound (close 1009 on overflow).
#[derive(Debug)]
pub struct MessageAssembler {
    fragments: Vec<u8>,
    fragmenting: bool,
    max_message_bytes: usize,
}

impl MessageAssembler {
    /// A reassembler with the declared message cap (non-zero; a zero cap
    /// is a construction error — the crate's degenerate-limits pattern).
    pub fn new(max_message_bytes: usize) -> Result<Self, FrameError> {
        if max_message_bytes == 0 {
            return Err(FrameError::MessageTooBig { max: 0, found: 0 });
        }
        Ok(Self {
            fragments: Vec::new(),
            fragmenting: false,
            max_message_bytes,
        })
    }

    /// Feeds one decoded frame; returns the completed wire event when one
    /// is finished. Every protocol violation is a named error; the
    /// caller maps it onto the wire close via [`close_code_for`].
    pub fn push(
        &mut self,
        fin: bool,
        opcode: Opcode,
        payload: Vec<u8>,
    ) -> Result<Option<AssemblerEvent>, FrameError> {
        match opcode {
            Opcode::Text => {
                if self.fragmenting {
                    return Err(FrameError::NewFragmentWhileFragmenting);
                }
                if fin {
                    let text = Self::utf8(payload)?;
                    return Ok(Some(AssemblerEvent::Text(text)));
                }
                self.check_bound(&payload)?;
                self.fragments = payload;
                self.fragmenting = true;
                Ok(None)
            }
            Opcode::Continuation => {
                if !self.fragmenting {
                    return Err(FrameError::UnexpectedContinuation);
                }
                self.check_bound(&payload)?;
                self.fragments.extend_from_slice(&payload);
                if fin {
                    self.fragmenting = false;
                    let text = Self::utf8(std::mem::take(&mut self.fragments))?;
                    return Ok(Some(AssemblerEvent::Text(text)));
                }
                Ok(None)
            }
            Opcode::Binary => Err(FrameError::BinaryDataRefused),
            Opcode::Ping => Ok(Some(AssemblerEvent::Ping(payload))),
            Opcode::Pong => Ok(Some(AssemblerEvent::Pong(payload))),
            Opcode::Close => {
                let (code, reason) = parse_close_payload(&payload)?;
                Ok(Some(AssemblerEvent::Close { code, reason }))
            }
        }
    }

    /// Whether a fragmented message is mid-assembly (the transport
    /// reports it honestly on teardown — an abandoned fragment is a
    /// dropped partial message, never a silently completed one).
    pub fn is_fragmenting(&self) -> bool {
        self.fragmenting
    }

    fn check_bound(&self, payload: &[u8]) -> Result<(), FrameError> {
        let found = self.fragments.len() + payload.len();
        if found > self.max_message_bytes {
            return Err(FrameError::MessageTooBig {
                max: self.max_message_bytes,
                found,
            });
        }
        Ok(())
    }

    fn utf8(payload: Vec<u8>) -> Result<String, FrameError> {
        String::from_utf8(payload).map_err(|_| FrameError::InvalidUtf8)
    }
}

// --- Handshake ---------------------------------------------------------------

/// Handshake errors. Every refusal names the rule; the detail that
/// travels onto the wire is the adapter's own named record, never raw
/// peer text (the P1.08 human-safe-detail rule).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HandshakeError {
    /// The request head could not be parsed as an HTTP/1.1 request.
    NotHttpRequest { reason: &'static str },
    /// The request head exceeded the declared cap.
    RequestHeadTooLarge { max: usize },
    /// A required handshake header is missing (RFC 6455 §4.2.1).
    MissingHeader { name: &'static str },
    /// The request is not a WebSocket upgrade (method/Upgrade/Connection).
    NotWebSocketUpgrade { reason: &'static str },
    /// The offered version is not 13 — answered with 426 Upgrade Required
    /// and the required version (RFC 6455 §4.2.2).
    UnsupportedVersion { found: String },
    /// The |Sec-WebSocket-Key| is missing, not base64, or not 16 bytes.
    InvalidSecWebSocketKey,
    /// The subprotocol negotiation failed (re-raised from
    /// crate::session — the P17-E-1 refusal travels verbatim).
    Negotiation(crate::session::NegotiationError),
}

impl fmt::Display for HandshakeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HandshakeError::NotHttpRequest { reason } => {
                write!(f, "the request head is not an HTTP/1.1 request: {reason}")
            }
            HandshakeError::RequestHeadTooLarge { max } => {
                write!(f, "the request head exceeded the declared cap of {max} bytes")
            }
            HandshakeError::MissingHeader { name } => {
                write!(f, "required handshake header '{name}' is missing")
            }
            HandshakeError::NotWebSocketUpgrade { reason } => {
                write!(f, "not a WebSocket upgrade: {reason}")
            }
            HandshakeError::UnsupportedVersion { found } => write!(
                f,
                "unsupported |Sec-WebSocket-Version| '{found}'; this endpoint requires \
                 version {WEBSOCKET_VERSION}"
            ),
            HandshakeError::InvalidSecWebSocketKey => write!(
                f,
                "|Sec-WebSocket-Key| is missing, not base64, or not 16 bytes"
            ),
            HandshakeError::Negotiation(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for HandshakeError {}

/// A parsed HTTP/1.1 upgrade request head (RFC 6455 §4.2.1). Header
/// names are normalized to lower case; duplicates are retained (the
/// subprotocol header may legally repeat).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpgradeRequest {
    /// The request target (the URL path — where the OCPP-J charge-point
    /// identity travels, register item 3; extracted by the transport).
    pub request_target: String,
    headers: Vec<(String, String)>,
}

impl UpgradeRequest {
    /// The first value of the named header (names are lower case here).
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }

    /// The offered subprotocols, in the peer's preference order,
    /// comma-split across all |Sec-WebSocket-Protocol| occurrences
    /// (RFC 6455 §4.3; the ordering is honoured, never reordered — the
    /// session negotiation rule).
    pub fn offered_subprotocols(&self) -> Vec<String> {
        self.headers
            .iter()
            .filter(|(key, _)| key == "sec-websocket-protocol")
            .flat_map(|(_, value)| value.split(','))
            .map(|token| token.trim().to_string())
            .filter(|token| !token.is_empty())
            .collect()
    }
}

/// Parses a complete HTTP/1.1 request head (up to and including the
/// blank line). Refuses anything that is not a GET with an HTTP/1.1
/// request line — named, never repaired.
pub fn parse_upgrade_request(bytes: &[u8]) -> Result<UpgradeRequest, HandshakeError> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| HandshakeError::NotHttpRequest { reason: "not valid UTF-8" })?;
    let (head, _body) = text
        .split_once("\r\n\r\n")
        .ok_or(HandshakeError::NotHttpRequest { reason: "no blank line terminator" })?;
    let mut lines = head.split("\r\n");
    let request_line = lines
        .next()
        .ok_or(HandshakeError::NotHttpRequest { reason: "empty request head" })?;
    let mut parts = request_line.split(' ');
    let method = parts
        .next()
        .ok_or(HandshakeError::NotHttpRequest { reason: "no method" })?;
    let target = parts
        .next()
        .ok_or(HandshakeError::NotHttpRequest { reason: "no request target" })?;
    let version = parts
        .next()
        .ok_or(HandshakeError::NotHttpRequest { reason: "no HTTP version" })?;
    if method != "GET" {
        return Err(HandshakeError::NotWebSocketUpgrade {
            reason: "the method is not GET",
        });
    }
    if version != "HTTP/1.1" {
        return Err(HandshakeError::NotHttpRequest {
            reason: "the version is not HTTP/1.1",
        });
    }
    let mut headers = Vec::new();
    for line in lines {
        if line.is_empty() {
            continue;
        }
        let (name, value) = line
            .split_once(':')
            .ok_or(HandshakeError::NotHttpRequest {
                reason: "a header line has no colon",
            })?;
        headers.push((name.trim().to_ascii_lowercase(), value.trim().to_string()));
    }
    Ok(UpgradeRequest {
        request_target: target.to_string(),
        headers,
    })
}

/// Computes the |Sec-WebSocket-Accept| value (RFC 6455 §4.2.2): base64 of
/// SHA-1(client key + GUID). The hash is the hand-written pinned SHA-1 in
/// [`crate::sha1`] (module honesty header: RFC 6455 mandates SHA-1
/// exactly; no strength claim is made or needed).
pub fn compute_accept_key(client_key: &str) -> Result<String, HandshakeError> {
    let key_bytes = BASE64_STANDARD
        .decode(client_key)
        .map_err(|_| HandshakeError::InvalidSecWebSocketKey)?;
    if key_bytes.len() != 16 {
        return Err(HandshakeError::InvalidSecWebSocketKey);
    }
    let mut input = Vec::with_capacity(client_key.len() + WEBSOCKET_GUID.len());
    input.extend_from_slice(client_key.as_bytes());
    input.extend_from_slice(WEBSOCKET_GUID.as_bytes());
    let digest = crate::sha1::sha1(&input);
    Ok(BASE64_STANDARD.encode(digest))
}

/// Validates the upgrade request and negotiates the subprotocol. On
/// success returns the accepted [`Subprotocol`] and the computed accept
/// key. The negotiation is the RECORDED one ([`session::negotiate`]): no
/// unspecified default, `ocpp1.6` refused with the P17-E-1 reason
/// verbatim — this function never invents refusal vocabulary of its own.
pub fn negotiate_handshake(
    request: &UpgradeRequest,
) -> Result<(Subprotocol, String), HandshakeError> {
    let upgrade = request
        .header("upgrade")
        .ok_or(HandshakeError::MissingHeader { name: "Upgrade" })?;
    if !upgrade.split(',').any(|token| token.trim().eq_ignore_ascii_case("websocket")) {
        return Err(HandshakeError::NotWebSocketUpgrade {
            reason: "the Upgrade header does not offer the websocket token",
        });
    }
    let connection = request
        .header("connection")
        .ok_or(HandshakeError::MissingHeader { name: "Connection" })?;
    if !connection.split(',').any(|token| token.trim().eq_ignore_ascii_case("upgrade")) {
        return Err(HandshakeError::NotWebSocketUpgrade {
            reason: "the Connection header does not carry the upgrade token",
        });
    }
    let version = request
        .header("sec-websocket-version")
        .ok_or(HandshakeError::MissingHeader {
            name: "Sec-WebSocket-Version",
        })?;
    if version.trim() != WEBSOCKET_VERSION {
        return Err(HandshakeError::UnsupportedVersion {
            found: version.trim().to_string(),
        });
    }
    let key = request
        .header("sec-websocket-key")
        .ok_or(HandshakeError::MissingHeader {
            name: "Sec-WebSocket-Key",
        })?;
    let accept_key = compute_accept_key(key.trim())?;
    let offered = request.offered_subprotocols();
    if offered.is_empty() {
        // A CSMS connection without a negotiated protocol is refused
        // (fail-closed, no unspecified default — the session rule).
        return Err(HandshakeError::Negotiation(
            session::NegotiationError::NoSubprotocolOffered,
        ));
    }
    let subprotocol = session::negotiate(&offered).map_err(HandshakeError::Negotiation)?;
    Ok((subprotocol, accept_key))
}

/// Builds the RFC 6455 §4.2.2 switching-protocols response. The
/// |Sec-WebSocket-Extensions| header is deliberately ABSENT: no extension
/// (permessage-deflate included) is ever negotiated — the omission IS the
/// refusal (RFC 6455 §4.2.2.2).
pub fn build_switching_response(subprotocol: Subprotocol, accept_key: &str) -> String {
    format!(
        "HTTP/1.1 101 Switching Protocols\r\n\
         Upgrade: websocket\r\n\
         Connection: Upgrade\r\n\
         Sec-WebSocket-Accept: {accept_key}\r\n\
         Sec-WebSocket-Protocol: {}\r\n\
         \r\n",
        subprotocol.wire_name()
    )
}

/// The HTTP refusal statuses the handshake uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefusalStatus {
    /// 400 Bad Request — a malformed or refused upgrade (including a
    /// refused subprotocol offer).
    BadRequest,
    /// 426 Upgrade Required — the offered WebSocket version is not 13
    /// (the response names the required version, RFC 6455 §4.2.2).
    UpgradeRequired,
}

impl RefusalStatus {
    const fn status_line(self) -> &'static str {
        match self {
            RefusalStatus::BadRequest => "HTTP/1.1 400 Bad Request",
            RefusalStatus::UpgradeRequired => "HTTP/1.1 426 Upgrade Required",
        }
    }
}

/// Builds the refusal response for a failed handshake. `detail` is the
/// adapter's OWN named record (the negotiation / handshake error text —
/// the crate's recorded refusal vocabulary, never fabricated peer
/// reflection; the P1.08 human-safe-detail rule).
pub fn build_refusal_response(status: RefusalStatus, detail: &str) -> String {
    let extra = match status {
        RefusalStatus::UpgradeRequired => format!("Sec-WebSocket-Version: {WEBSOCKET_VERSION}\r\n"),
        RefusalStatus::BadRequest => String::new(),
    };
    let body = detail.as_bytes();
    format!(
        "{}\r\n\
         Connection: close\r\n\
         Content-Type: text/plain; charset=utf-8\r\n\
         Content-Length: {}\r\n\
         {}\r\n",
        status.status_line(),
        body.len(),
        extra,
        detail
    )
}

/// Maps a handshake error onto its refusal status and the named detail.
pub fn refusal_for(error: &HandshakeError) -> (RefusalStatus, String) {
    match error {
        HandshakeError::UnsupportedVersion { .. } => (
            RefusalStatus::UpgradeRequired,
            error.to_string(),
        ),
        other => (RefusalStatus::BadRequest, other.to_string()),
    }
}

// --- Async IO helpers (tokio) --------------------------------------------------

/// Reads one HTTP request head byte-at-a-time until the blank line (the
/// head is small and the byte-at-a-time form cannot over-read into the
/// WebSocket frames that follow; the declared cap refuses unbounded
/// heads). WRITTEN-UNRUN — part of the human lab gate.
pub async fn read_http_request_head<R>(reader: &mut R) -> Result<Vec<u8>, HandshakeError>
where
    R: AsyncReadExt + Unpin,
{
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        let read = reader
            .read(&mut byte)
            .await
            .map_err(|_| HandshakeError::NotHttpRequest {
                reason: "the peer closed the connection mid-head",
            })?;
        if read == 0 {
            return Err(HandshakeError::NotHttpRequest {
                reason: "the peer closed the connection mid-head",
            });
        }
        head.push(byte[0]);
        if head.len() > MAX_HTTP_REQUEST_HEAD_BYTES {
            return Err(HandshakeError::RequestHeadTooLarge {
                max: MAX_HTTP_REQUEST_HEAD_BYTES,
            });
        }
        if head.ends_with(b"\r\n\r\n") {
            return Ok(head);
        }
    }
}

/// Reads one complete frame from the wire (header, then exactly the
/// payload the header declares), enforcing the declared frame cap and the
/// client-masking rule. WRITTEN-UNRUN — part of the human lab gate.
pub async fn read_frame<R>(reader: &mut R) -> Result<RawFrame, FrameError>
where
    R: AsyncReadExt + Unpin,
{
    let mut head = [0u8; 2];
    reader.read_exact(&mut head).await.map_err(io_to_frame_error)?;
    // Size the full header from the fixed fields before reading the rest:
    // 2 bytes are read; extended length adds 2 or 8; a masked sender adds
    // 4 more.
    let len7 = head[1] & 0x7F;
    let extra_length_bytes = match len7 {
        len if len < 126 => 0usize,
        126 => 2,
        _ => 8,
    };
    let mask_bytes = if head[1] & 0x80 != 0 { 4usize } else { 0usize };
    let header_len = 2 + extra_length_bytes + mask_bytes;
    let mut bytes = vec![0u8; header_len];
    bytes[0..2].copy_from_slice(&head);
    reader
        .read_exact(&mut bytes[2..])
        .await
        .map_err(io_to_frame_error)?;
    let full_header = parse_frame_header(&bytes)?;
    if full_header.payload_len > MAX_WEBSOCKET_MESSAGE_BYTES {
        return Err(FrameError::FrameTooBig {
            max: MAX_WEBSOCKET_MESSAGE_BYTES,
            found: full_header.payload_len,
        });
    }
    let mut payload = vec![0u8; full_header.payload_len];
    reader.read_exact(&mut payload).await.map_err(io_to_frame_error)?;
    if let Some(key) = full_header.mask_key {
        apply_mask(&mut payload, key);
    } else {
        // The server role only decodes client frames; an unmasked client
        // frame fails the connection with 1002 (register item 1).
        return Err(FrameError::UnmaskedClientFrame);
    }
    Ok(RawFrame {
        fin: full_header.fin,
        opcode: full_header.opcode,
        payload,
    })
}

/// Writes one pre-encoded frame. WRITTEN-UNRUN — part of the human lab
/// gate.
pub async fn write_frame<W>(writer: &mut W, bytes: &[u8]) -> std::io::Result<()>
where
    W: AsyncWriteExt + Unpin,
{
    writer.write_all(bytes).await?;
    writer.flush().await
}

fn io_to_frame_error(_: std::io::Error) -> FrameError {
    // A mid-frame IO failure is an abrupt transport close, not a frame
    // protocol error; the caller surfaces it as a connection-lost event.
    // The named variant exists so the async path has one error type.
    FrameError::IncompleteFrame { needed: 0, have: 0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The RFC 6455 §1.3 example request, verbatim head.
    fn rfc_example_request() -> Vec<u8> {
        b"GET /chat HTTP/1.1\r\n\
          Host: example.com:8001\r\n\
          Upgrade: websocket\r\n\
          Connection: Upgrade\r\n\
          Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
          Sec-WebSocket-Version: 13\r\n\
          Sec-WebSocket-Protocol: ocpp2.0.1\r\n\
          \r\n"
            .to_vec()
    }

    #[test]
    fn the_accept_key_computes_the_rfc_6455_example_value() {
        // RFC 6455 §1.3: base64(SHA-1(key + GUID)) == this exact value.
        let accept = compute_accept_key("dGhlIHNhbXBsZSBub25jZQ==").expect("valid key");
        assert_eq!(accept, "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=");
    }

    #[test]
    fn the_handshake_negotiates_and_builds_the_switching_response() {
        let request = parse_upgrade_request(&rfc_example_request()).expect("parses");
        assert_eq!(request.request_target, "/chat");
        let (subprotocol, accept_key) = negotiate_handshake(&request).expect("negotiated");
        assert_eq!(subprotocol, Subprotocol::Ocpp201);
        assert_eq!(accept_key, "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=");
        let response = build_switching_response(subprotocol, &accept_key);
        assert!(response.starts_with("HTTP/1.1 101 Switching Protocols\r\n"));
        assert!(response.contains("Upgrade: websocket\r\n"));
        assert!(response.contains("Connection: Upgrade\r\n"));
        assert!(response.contains("Sec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=\r\n"));
        assert!(response.contains("Sec-WebSocket-Protocol: ocpp2.0.1\r\n"));
        // NO extension is ever negotiated: permessage-deflate is refused
        // by the header's absence (RFC 6455 §4.2.2.2).
        assert!(!response.contains("Sec-WebSocket-Extensions"));
        assert!(!response.contains("permessage-deflate"));
    }

    #[test]
    fn the_handshake_offers_are_read_in_peer_preference_order() {
        let bytes = b"GET / HTTP/1.1\r\n\
                      Host: h\r\n\
                      Upgrade: websocket\r\n\
                      Connection: upgrade\r\n\
                      Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
                      Sec-WebSocket-Version: 13\r\n\
                      Sec-WebSocket-Protocol: ocpp1.6, ocpp2.0.1\r\n\
                      \r\n";
        let request = parse_upgrade_request(bytes).expect("parses");
        let offered = request.offered_subprotocols();
        assert_eq!(
            offered,
            vec!["ocpp1.6".to_string(), "ocpp2.0.1".to_string()]
        );
        // The negotiation selects from the offered order (1.6 skipped per
        // the P17-E-1 DEFAULT; 2.0.1 accepted).
        let (subprotocol, _) = negotiate_handshake(&request).expect("negotiated");
        assert_eq!(subprotocol, Subprotocol::Ocpp201);
    }

    #[test]
    fn an_ocpp_1_6_only_offer_is_refused_with_the_recorded_p17_e_1_reason() {
        let bytes = b"GET / HTTP/1.1\r\n\
                      Host: h\r\n\
                      Upgrade: websocket\r\n\
                      Connection: upgrade\r\n\
                      Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
                      Sec-WebSocket-Version: 13\r\n\
                      Sec-WebSocket-Protocol: ocpp1.6\r\n\
                      \r\n";
        let request = parse_upgrade_request(bytes).expect("parses");
        let error = negotiate_handshake(&request).expect_err("1.6-only refused");
        let (status, detail) = refusal_for(&error);
        assert_eq!(status, RefusalStatus::BadRequest);
        // The EXACT recorded P17-E-1 reason travels verbatim (no new
        // vocabulary was invented here — this is session::negotiate's
        // refusal, restated by the transport).
        assert!(detail.contains("P17-E-1"));
        let response = build_refusal_response(status, &detail);
        assert!(response.starts_with("HTTP/1.1 400 Bad Request\r\n"));
        assert!(response.contains("P17-E-1"));
        assert!(response.ends_with(&detail));
    }

    #[test]
    fn handshake_refusals_are_named() {
        // Not an upgrade (no Upgrade header).
        let bytes = b"GET / HTTP/1.1\r\nHost: h\r\n\r\n";
        let request = parse_upgrade_request(bytes).expect("parses");
        assert_eq!(
            negotiate_handshake(&request),
            Err(HandshakeError::MissingHeader { name: "Upgrade" })
        );
        // Wrong version: 426 with the required version named.
        let bytes = b"GET / HTTP/1.1\r\n\
                      Host: h\r\n\
                      Upgrade: websocket\r\n\
                      Connection: upgrade\r\n\
                      Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
                      Sec-WebSocket-Version: 8\r\n\
                      \r\n";
        let request = parse_upgrade_request(bytes).expect("parses");
        let error = negotiate_handshake(&request).expect_err("version refused");
        let (status, detail) = refusal_for(&error);
        assert_eq!(status, RefusalStatus::UpgradeRequired);
        let response = build_refusal_response(status, &detail);
        assert!(response.starts_with("HTTP/1.1 426 Upgrade Required\r\n"));
        assert!(response.contains("Sec-WebSocket-Version: 13\r\n"));
        // A bad key is named.
        let bytes = b"GET / HTTP/1.1\r\n\
                      Host: h\r\n\
                      Upgrade: websocket\r\n\
                      Connection: upgrade\r\n\
                      Sec-WebSocket-Key: not-base64!!\r\n\
                      Sec-WebSocket-Version: 13\r\n\
                      \r\n";
        let request = parse_upgrade_request(bytes).expect("parses");
        assert_eq!(
            negotiate_handshake(&request),
            Err(HandshakeError::InvalidSecWebSocketKey)
        );
        // A 16-byte key is required (a 12-byte base64 payload is refused).
        let bytes = b"GET / HTTP/1.1\r\n\
                      Host: h\r\n\
                      Upgrade: websocket\r\n\
                      Connection: upgrade\r\n\
                      Sec-WebSocket-Key: c2hvcnQta2V5LWhlcmU=\r\n\
                      Sec-WebSocket-Version: 13\r\n\
                      \r\n";
        let request = parse_upgrade_request(bytes).expect("parses");
        assert_eq!(
            negotiate_handshake(&request),
            Err(HandshakeError::InvalidSecWebSocketKey)
        );
        // An empty offer is refused (no unspecified default).
        let bytes = b"GET / HTTP/1.1\r\n\
                      Host: h\r\n\
                      Upgrade: websocket\r\n\
                      Connection: upgrade\r\n\
                      Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
                      Sec-WebSocket-Version: 13\r\n\
                      \r\n";
        let request = parse_upgrade_request(bytes).expect("parses");
        assert!(matches!(
            negotiate_handshake(&request),
            Err(HandshakeError::Negotiation(
                crate::session::NegotiationError::NoSubprotocolOffered
            ))
        ));
    }

    #[test]
    fn client_frames_round_trip_masked_and_server_frames_unmasked() {
        // A client TEXT frame, masked with the RFC-style key: the codec
        // decodes it and the payload survives.
        let key = [0x37, 0xFA, 0x21, 0x3D];
        let text = r#"[2,"id-1","Heartbeat",{}]"#;
        let wire = encode_frame(true, Opcode::Text, text.as_bytes(), Some(key));
        let frame = parse_frame_bytes(&wire, MAX_WEBSOCKET_MESSAGE_BYTES).expect("decodes");
        assert!(frame.fin);
        assert_eq!(frame.opcode, Opcode::Text);
        assert_eq!(frame.payload, text.as_bytes());

        // A 126-boundary sweep: the extended-16-bit length kicks in at 126.
        for len in [125usize, 126, 127, 65_535, 65_536] {
            let payload = vec![0x41u8; len];
            let wire = encode_frame(true, Opcode::Text, &payload, Some(key));
            let frame =
                parse_frame_bytes(&wire, MAX_WEBSOCKET_MESSAGE_BYTES).expect("decodes");
            assert_eq!(frame.payload.len(), len);
            assert_eq!(frame.payload, payload);
        }

        // The server write path is UNMASKED (the shipped discipline).
        let server_wire = encode_server_text("hello");
        let header = parse_frame_header(&server_wire).expect("header");
        assert!(!header.masked);
        assert!(header.mask_key.is_none());
    }

    #[test]
    fn frame_protocol_violations_are_named() {
        let key = [0x11u8, 0x22, 0x33, 0x44];
        // An unmasked client frame fails with the §5.1 rule.
        let wire = encode_frame(true, Opcode::Text, b"hi", None);
        assert_eq!(
            parse_frame_bytes(&wire, MAX_WEBSOCKET_MESSAGE_BYTES),
            Err(FrameError::UnmaskedClientFrame)
        );
        // RSV bits set without a negotiated extension.
        let mut wire = encode_frame(true, Opcode::Text, b"hi", Some(key));
        wire[0] |= 0x40; // RSV1
        assert_eq!(
            parse_frame_bytes(&wire, MAX_WEBSOCKET_MESSAGE_BYTES),
            Err(FrameError::ReservedBitsSet)
        );
        // A reserved opcode.
        let mut wire = encode_frame(true, Opcode::Text, b"hi", Some(key));
        wire[0] = (wire[0] & 0xF0) | 0xC;
        assert!(matches!(
            parse_frame_bytes(&wire, MAX_WEBSOCKET_MESSAGE_BYTES),
            Err(FrameError::ReservedOpcode { opcode: 0xC })
        ));
        // A non-minimal 16-bit length.
        let mut wire = encode_frame(true, Opcode::Text, b"hi", Some(key));
        wire[1] = (wire[1] & 0x80) | 126;
        wire[2] = 0;
        wire[3] = 2;
        assert_eq!(
            parse_frame_bytes(&wire, MAX_WEBSOCKET_MESSAGE_BYTES),
            Err(FrameError::NonMinimalLengthEncoding)
        );
        // An oversized frame exceeds the declared cap.
        let payload = vec![0u8; 300];
        let wire = encode_frame(true, Opcode::Text, &payload, Some(key));
        assert_eq!(
            parse_frame_bytes(&wire, 128),
            Err(FrameError::FrameTooBig { max: 128, found: 300 })
        );
        // A control frame must not be fragmented.
        let wire = encode_frame(false, Opcode::Ping, b"p", Some(key));
        assert!(matches!(
            parse_frame_bytes(&wire, MAX_WEBSOCKET_MESSAGE_BYTES),
            Err(FrameError::ControlFrameViolation { .. })
        ));
        // A control frame over 125 bytes.
        let payload = vec![0u8; 126];
        let wire = encode_frame(true, Opcode::Ping, &payload, Some(key));
        assert!(matches!(
            parse_frame_bytes(&wire, MAX_WEBSOCKET_MESSAGE_BYTES),
            Err(FrameError::ControlFrameViolation { .. })
        ));
    }

    #[test]
    fn fragmented_text_messages_reassemble_within_the_bound() {
        let mut assembler = MessageAssembler::new(MAX_WEBSOCKET_MESSAGE_BYTES).expect("valid");
        let key = [0x00, 0x00, 0x00, 0x00];
        // Fragment 1 (text, not FIN).
        let frame = parse_frame_bytes(
            &encode_frame(false, Opcode::Text, b"[2,\"id\"", Some(key)),
            MAX_WEBSOCKET_MESSAGE_BYTES,
        )
        .expect("decodes");
        assert_eq!(assembler.push(frame.fin, frame.opcode, frame.payload), Ok(None));
        // An interleaved control frame is legal mid-fragmentation.
        let frame = parse_frame_bytes(
            &encode_frame(true, Opcode::Ping, b"still-here", Some(key)),
            MAX_WEBSOCKET_MESSAGE_BYTES,
        )
        .expect("decodes");
        assert_eq!(
            assembler.push(frame.fin, frame.opcode, frame.payload),
            Ok(Some(AssemblerEvent::Ping(b"still-here".to_vec())))
        );
        // Fragment 2 (continuation, FIN): the message completes.
        let frame = parse_frame_bytes(
            &encode_frame(true, Opcode::Continuation, b",\"Heartbeat\",{}]", Some(key)),
            MAX_WEBSOCKET_MESSAGE_BYTES,
        )
        .expect("decodes");
        assert_eq!(
            assembler.push(frame.fin, frame.opcode, frame.payload),
            Ok(Some(AssemblerEvent::Text(
                "[2,\"id\",\"Heartbeat\",{}]".to_string()
            )))
        );
        assert!(!assembler.is_fragmenting());
    }

    #[test]
    fn fragmentation_violations_and_bounds_are_named() {
        // A zero message cap is a construction error.
        assert!(MessageAssembler::new(0).is_err());
        let mut assembler = MessageAssembler::new(16).expect("valid");
        let key = [0u8; 4];
        // A continuation with no fragment in progress.
        let frame = parse_frame_bytes(
            &encode_frame(true, Opcode::Continuation, b"x", Some(key)),
            128,
        )
        .expect("decodes");
        assert_eq!(
            assembler.push(frame.fin, frame.opcode, frame.payload),
            Err(FrameError::UnexpectedContinuation)
        );
        // A new fragment while one is in progress.
        let frame = parse_frame_bytes(
            &encode_frame(false, Opcode::Text, b"start", Some(key)),
            128,
        )
        .expect("decodes");
        assert_eq!(assembler.push(frame.fin, frame.opcode, frame.payload.clone()), Ok(None));
        let frame = parse_frame_bytes(
            &encode_frame(false, Opcode::Text, b"again", Some(key)),
            128,
        )
        .expect("decodes");
        assert_eq!(
            assembler.push(frame.fin, frame.opcode, frame.payload),
            Err(FrameError::NewFragmentWhileFragmenting)
        );
        // The assembled bound: overflowing is named with the cap.
        let frame = parse_frame_bytes(
            &encode_frame(true, Opcode::Continuation, b"0123456789ABCDEF", Some(key)),
            128,
        )
        .expect("decodes");
        assert_eq!(
            assembler.push(frame.fin, frame.opcode, frame.payload),
            Err(FrameError::MessageTooBig { max: 16, found: 21 })
        );
        // Binary data is refused outright (OCPP-J is text-only).
        let mut fresh = MessageAssembler::new(1024).expect("valid");
        let frame = parse_frame_bytes(
            &encode_frame(true, Opcode::Binary, b"\x00\x01", Some(key)),
            1024,
        )
        .expect("decodes");
        assert_eq!(
            fresh.push(frame.fin, frame.opcode, frame.payload),
            Err(FrameError::BinaryDataRefused)
        );
        // Non-UTF-8 text is refused with the 1007 mapping.
        let frame = parse_frame_bytes(
            &encode_frame(true, Opcode::Text, &[0xFF, 0xFE], Some(key)),
            1024,
        )
        .expect("decodes");
        assert_eq!(
            fresh.push(frame.fin, frame.opcode, frame.payload),
            Err(FrameError::InvalidUtf8)
        );
    }

    #[test]
    fn close_frames_parse_codes_and_refuse_malformed_payloads() {
        // A generated close frame carries the code + reason in its
        // payload (the server frame is unmasked — extract the payload
        // bytes directly; parse_frame_bytes is the CLIENT-direction
        // decoder, which refuses unmasked frames by design).
        let wire = encode_close(CLOSE_NORMAL, "going away");
        let header = parse_frame_header(&wire).expect("header");
        assert_eq!(header.opcode, Opcode::Close);
        assert!(!header.masked);
        let payload =
            wire[header.header_len..header.header_len + header.payload_len].to_vec();
        assert_eq!(
            parse_close_payload(&payload),
            Ok((CLOSE_NORMAL, "going away".to_string()))
        );
        // Empty payload: the no-status form.
        assert_eq!(parse_close_payload(&[]), Ok((1005, String::new())));
        // A 1-byte payload is invalid.
        assert!(parse_close_payload(&[0x03]).is_err());
        // A reserved code is invalid.
        assert!(parse_close_payload(&[0x03, 0xEC]).is_err());
        // A non-UTF-8 reason is invalid.
        assert!(parse_close_payload(&[0x03, 0xE8, 0xFF]).is_err());
    }

    #[test]
    fn close_code_mapping_names_every_frame_error() {
        assert_eq!(close_code_for(&FrameError::UnmaskedClientFrame), CLOSE_PROTOCOL_ERROR);
        assert_eq!(close_code_for(&FrameError::ReservedBitsSet), CLOSE_PROTOCOL_ERROR);
        assert_eq!(
            close_code_for(&FrameError::ReservedOpcode { opcode: 0xF }),
            CLOSE_PROTOCOL_ERROR
        );
        assert_eq!(
            close_code_for(&FrameError::NonMinimalLengthEncoding),
            CLOSE_PROTOCOL_ERROR
        );
        assert_eq!(
            close_code_for(&FrameError::ControlFrameViolation { reason: "x" }),
            CLOSE_PROTOCOL_ERROR
        );
        assert_eq!(close_code_for(&FrameError::UnexpectedContinuation), CLOSE_PROTOCOL_ERROR);
        assert_eq!(
            close_code_for(&FrameError::NewFragmentWhileFragmenting),
            CLOSE_PROTOCOL_ERROR
        );
        assert_eq!(
            close_code_for(&FrameError::InvalidCloseFrame { reason: "x" }),
            CLOSE_PROTOCOL_ERROR
        );
        assert_eq!(
            close_code_for(&FrameError::FrameTooBig { max: 1, found: 2 }),
            CLOSE_MESSAGE_TOO_BIG
        );
        assert_eq!(
            close_code_for(&FrameError::MessageTooBig { max: 1, found: 2 }),
            CLOSE_MESSAGE_TOO_BIG
        );
        assert_eq!(close_code_for(&FrameError::BinaryDataRefused), CLOSE_UNSUPPORTED_DATA);
        assert_eq!(close_code_for(&FrameError::InvalidUtf8), CLOSE_INVALID_PAYLOAD);
        assert_eq!(
            close_code_for(&FrameError::IncompleteFrame { needed: 1, have: 0 }),
            CLOSE_INTERNAL_ERROR
        );
    }
}
