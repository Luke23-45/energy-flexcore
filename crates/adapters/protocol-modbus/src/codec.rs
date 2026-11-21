// FIM-F03.01 — the MBAP header + PDU frame codec (docs/releases/
// final-implementation-before-publication.md §5 F-03; ledger
// docs/releases/implementation_ledger/ledger_F03.md subtask FIM-F03.01).
//
// **Implemented under FIM-F03, WRITTEN-UNRUN: this module has never been
// compiled or executed. Conformance against a real Modbus TCP peer is
// UNPROVEN and remains a HUMAN LAB GATE** (crate conventions; the pinned
// byte vectors below are hand-computed, not machine-checked against a
// device — they pin the codec's own behaviour, nothing else).
//
// This is a HAND-WRITTEN MINIMAL CODEC (FIM standing decision 1): Modbus
// TCP framing is NOT a dependency — no Modbus stack library exists in this
// program. The codec covers exactly the function-code subset the adapter's
// declared surface uses:
//
// - read function codes: 0x01 read coils, 0x02 read discrete inputs,
//   0x03 read holding registers, 0x04 read input registers;
// - write function codes: 0x05 write single coil, 0x06 write single
//   register, 0x0F write multiple coils, 0x10 write multiple registers —
//   encoded/decoded for WIRE COMPLETENESS ONLY. The transport surface
//   ([`crate::transport::ModbusTransport`]) has no write method: the
//   read-only posture is structural (transport.rs header), and the write
//   codecs exist so a peer's write traffic can be framed and named rather
//   than silently dropped.
// - exception responses (function code | 0x80 + exception code), decoded
//   into the named [`ModbusExceptionCode`] vocabulary for the refusal
//   taxonomy (FIM-F03.04).
//
// Wire layout (the record-23 discipline applies — every protocol-text
// claim below carries an UNVERIFIED anchor to be checked at the lab gate):
//
// - MBAP header: transaction id (u16, big-endian), protocol id (u16,
//   big-endian, must be 0), length (u16, big-endian = bytes AFTER the
//   length field, INCLUDING the unit id), unit id (u8). The frame is
//   `6 + length` bytes.
// - PDU request/response payloads are big-endian in every multi-byte
//   field (UNVERIFIED: Modbus is defined as big-endian in the protocol
//   text; a peer that disagrees is a lab-gate finding, never a silent
//   reinterpretation here).
// - 32-bit values span two 16-bit registers and their WORD ORDER is a
//   REGISTER-MAP concern ([`crate::registry::decode_register_window`] —
//   big-endian register pair, high word first): this codec carries opaque
//   16-bit registers and never re-orders words.
// - Coil/discrete bit packing is LSB-first within each byte
//   (UNVERIFIED anchor).
//
// Every decode refusal is a NAMED [`CodecError`] (the closed defect
// vocabulary [`FrameDefect`] carries the frame-level reasons); nothing
// here panics, truncates, or guesses. The mapping from codec errors onto
// the transport refusal taxonomy ([`crate::transport::TransportError`])
// lives in [`crate::tcp_transport`] (ONE total mapping, FIM-F03.04).

use crate::polling::MAX_REGISTERS_PER_READ;

/// The MBAP protocol id (UNVERIFIED anchor: the protocol text fixes it to
/// 0 for Modbus TCP; a non-zero protocol id is a named refusal).
pub const PROTOCOL_ID: u16 = 0;

/// The maximum MBAP length field this codec accepts: the Modbus TCP ADU
/// is limited to 260 bytes total (UNVERIFIED anchor), so the length field
/// is at most 260 − 6 = 254. A larger declared length is refused BEFORE
/// any allocation (no hang, no oversized read).
pub const MAX_ADU_LENGTH_FIELD: u16 = 254;

/// The maximum number of coils/discrete inputs a single read request may
/// address (UNVERIFIED anchor: 2000 = 0x07D0 in the protocol text).
pub const MAX_READ_BITS: u16 = 2_000;

/// The maximum number of bits a single write-multiple-coils request may
/// carry (UNVERIFIED anchor: 1968 = 0x07B0 in the protocol text).
pub const MAX_WRITE_BITS: u16 = 1_968;

/// The maximum number of registers a single write-multiple-registers
/// request may carry (UNVERIFIED anchor: 123 = 0x007B in the protocol
/// text). READ requests are bounded by [`MAX_REGISTERS_PER_READ`]
/// (polling.rs owns the 125-register read limit — ONE source).
pub const MAX_WRITE_REGISTERS: u16 = 123;

/// The Modbus function codes this codec covers (the read/write subset the
/// adapter's declared surface uses — see the module header).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FunctionCode {
    /// 0x01 — read coils.
    ReadCoils,
    /// 0x02 — read discrete inputs.
    ReadDiscreteInputs,
    /// 0x03 — read holding registers (the code the registry's maps are
    /// read with; UNVERIFIED addressing-base anchor — transport.rs).
    ReadHoldingRegisters,
    /// 0x04 — read input registers.
    ReadInputRegisters,
    /// 0x05 — write single coil (wire completeness only; no write path).
    WriteSingleCoil,
    /// 0x06 — write single register (wire completeness only).
    WriteSingleRegister,
    /// 0x0F — write multiple coils (wire completeness only).
    WriteMultipleCoils,
    /// 0x10 — write multiple registers (wire completeness only).
    WriteMultipleRegisters,
}

impl FunctionCode {
    /// The wire byte for this function code.
    pub const fn value(self) -> u8 {
        match self {
            FunctionCode::ReadCoils => 0x01,
            FunctionCode::ReadDiscreteInputs => 0x02,
            FunctionCode::ReadHoldingRegisters => 0x03,
            FunctionCode::ReadInputRegisters => 0x04,
            FunctionCode::WriteSingleCoil => 0x05,
            FunctionCode::WriteSingleRegister => 0x06,
            FunctionCode::WriteMultipleCoils => 0x0F,
            FunctionCode::WriteMultipleRegisters => 0x10,
        }
    }

    /// The function code for a wire byte, or `None` outside the covered
    /// subset (a covered-subset refusal, never a guess).
    pub const fn from_byte(byte: u8) -> Option<Self> {
        match byte {
            0x01 => Some(FunctionCode::ReadCoils),
            0x02 => Some(FunctionCode::ReadDiscreteInputs),
            0x03 => Some(FunctionCode::ReadHoldingRegisters),
            0x04 => Some(FunctionCode::ReadInputRegisters),
            0x05 => Some(FunctionCode::WriteSingleCoil),
            0x06 => Some(FunctionCode::WriteSingleRegister),
            0x0F => Some(FunctionCode::WriteMultipleCoils),
            0x10 => Some(FunctionCode::WriteMultipleRegisters),
            _ => None,
        }
    }

    /// Canonical name (records/logs).
    pub const fn name(self) -> &'static str {
        match self {
            FunctionCode::ReadCoils => "read_coils",
            FunctionCode::ReadDiscreteInputs => "read_discrete_inputs",
            FunctionCode::ReadHoldingRegisters => "read_holding_registers",
            FunctionCode::ReadInputRegisters => "read_input_registers",
            FunctionCode::WriteSingleCoil => "write_single_coil",
            FunctionCode::WriteSingleRegister => "write_single_register",
            FunctionCode::WriteMultipleCoils => "write_multiple_coils",
            FunctionCode::WriteMultipleRegisters => "write_multiple_registers",
        }
    }
}

/// The Modbus exception codes this codec names (UNVERIFIED anchors: the
/// code values follow the protocol text's exception table; a code outside
/// the known set decodes as [`ModbusExceptionCode::Unknown`] and stays a
/// named refusal — nothing is dropped or coerced).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModbusExceptionCode {
    /// 0x01 — illegal function.
    IllegalFunction,
    /// 0x02 — illegal data address (the peer refused the register window;
    /// the transport maps this onto the existing
    /// [`crate::transport::TransportError::WindowOutOfRange`], exactly as
    /// that variant's doc comment requires).
    IllegalDataAddress,
    /// 0x03 — illegal data value.
    IllegalDataValue,
    /// 0x04 — server device failure.
    ServerDeviceFailure,
    /// 0x05 — acknowledge (request accepted, not yet done).
    Acknowledge,
    /// 0x06 — server device busy.
    ServerDeviceBusy,
    /// 0x08 — memory parity error.
    MemoryParityError,
    /// 0x0A — gateway path unavailable.
    GatewayPathUnavailable,
    /// 0x0B — gateway target device failed to respond.
    GatewayTargetDeviceFailedToRespond,
    /// Any code outside the known set: carried verbatim, still a named
    /// refusal (the vocabulary never silently grows, and never shrinks a
    /// peer's answer into a guess).
    Unknown(u8),
}

impl ModbusExceptionCode {
    /// The total byte → code mapping (a code outside the table decodes as
    /// [`ModbusExceptionCode::Unknown`]).
    pub const fn from_byte(byte: u8) -> Self {
        match byte {
            0x01 => ModbusExceptionCode::IllegalFunction,
            0x02 => ModbusExceptionCode::IllegalDataAddress,
            0x03 => ModbusExceptionCode::IllegalDataValue,
            0x04 => ModbusExceptionCode::ServerDeviceFailure,
            0x05 => ModbusExceptionCode::Acknowledge,
            0x06 => ModbusExceptionCode::ServerDeviceBusy,
            0x08 => ModbusExceptionCode::MemoryParityError,
            0x0A => ModbusExceptionCode::GatewayPathUnavailable,
            0x0B => ModbusExceptionCode::GatewayTargetDeviceFailedToRespond,
            other => ModbusExceptionCode::Unknown(other),
        }
    }

    /// The known code bytes (test table; `Unknown` is excluded on
    /// purpose — it is the catch-all arm, covered by its own test).
    pub const KNOWN_BYTES: [u8; 9] = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x08, 0x0A, 0x0B];

    /// Canonical name (records/logs); `Unknown` spells its raw byte.
    pub fn name(&self) -> String {
        match self {
            ModbusExceptionCode::IllegalFunction => "illegal_function".to_string(),
            ModbusExceptionCode::IllegalDataAddress => "illegal_data_address".to_string(),
            ModbusExceptionCode::IllegalDataValue => "illegal_data_value".to_string(),
            ModbusExceptionCode::ServerDeviceFailure => "server_device_failure".to_string(),
            ModbusExceptionCode::Acknowledge => "acknowledge".to_string(),
            ModbusExceptionCode::ServerDeviceBusy => "server_device_busy".to_string(),
            ModbusExceptionCode::MemoryParityError => "memory_parity_error".to_string(),
            ModbusExceptionCode::GatewayPathUnavailable => "gateway_path_unavailable".to_string(),
            ModbusExceptionCode::GatewayTargetDeviceFailedToRespond => {
                "gateway_target_device_failed_to_respond".to_string()
            }
            ModbusExceptionCode::Unknown(byte) => format!("unknown({byte:#04x})"),
        }
    }
}

/// One PDU-level request (the payload after the MBAP unit id). Owned
/// values: a request is constructed once and encoded once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PduRequest {
    /// 0x01 — read `count` coils from `start`.
    ReadCoils { start: u16, count: u16 },
    /// 0x02 — read `count` discrete inputs from `start`.
    ReadDiscreteInputs { start: u16, count: u16 },
    /// 0x03 — read `count` holding registers from `start` (the read the
    /// [`crate::transport::ModbusTransport`] port trait issues).
    ReadHoldingRegisters { start: u16, count: u16 },
    /// 0x04 — read `count` input registers from `start`.
    ReadInputRegisters { start: u16, count: u16 },
    /// 0x05 — write one coil (wire completeness only).
    WriteSingleCoil { address: u16, on: bool },
    /// 0x06 — write one register (wire completeness only).
    WriteSingleRegister { address: u16, value: u16 },
    /// 0x0F — write multiple coils, LSB-first packed (wire completeness
    /// only).
    WriteMultipleCoils { start: u16, bits: Vec<bool> },
    /// 0x10 — write multiple registers (wire completeness only).
    WriteMultipleRegisters { start: u16, values: Vec<u16> },
}

impl PduRequest {
    /// The function code this request uses.
    pub const fn kind(&self) -> FunctionCode {
        match self {
            PduRequest::ReadCoils { .. } => FunctionCode::ReadCoils,
            PduRequest::ReadDiscreteInputs { .. } => FunctionCode::ReadDiscreteInputs,
            PduRequest::ReadHoldingRegisters { .. } => FunctionCode::ReadHoldingRegisters,
            PduRequest::ReadInputRegisters { .. } => FunctionCode::ReadInputRegisters,
            PduRequest::WriteSingleCoil { .. } => FunctionCode::WriteSingleCoil,
            PduRequest::WriteSingleRegister { .. } => FunctionCode::WriteSingleRegister,
            PduRequest::WriteMultipleCoils { .. } => FunctionCode::WriteMultipleCoils,
            PduRequest::WriteMultipleRegisters { .. } => FunctionCode::WriteMultipleRegisters,
        }
    }

    /// The requested quantity the RESPONSE must echo: bits for coil
    /// reads, registers for register reads, elements for write-multiple
    /// echoes; single writes carry 0 (their echo shape is fixed).
    pub const fn quantity(&self) -> u16 {
        match self {
            PduRequest::ReadCoils { count, .. }
            | PduRequest::ReadDiscreteInputs { count, .. }
            | PduRequest::ReadHoldingRegisters { count, .. }
            | PduRequest::ReadInputRegisters { count, .. } => *count,
            PduRequest::WriteMultipleCoils { bits, .. } => bits.len() as u16,
            PduRequest::WriteMultipleRegisters { values, .. } => values.len() as u16,
            PduRequest::WriteSingleCoil { .. } | PduRequest::WriteSingleRegister { .. } => 0,
        }
    }
}

/// One decoded PDU-level response. The shape matches the REQUESTED
/// function code — [`decode_response`] refuses a response whose function
/// code does not answer the request (named refusal, never a guess).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PduResponse {
    /// 0x01 — the requested coils, in order.
    ReadCoils { bits: Vec<bool> },
    /// 0x02 — the requested discrete inputs, in order.
    ReadDiscreteInputs { bits: Vec<bool> },
    /// 0x03 — the requested holding registers, in order (big-endian
    /// per-register bytes; word order across registers is the registry's
    /// concern — module header).
    ReadHoldingRegisters { registers: Vec<u16> },
    /// 0x04 — the requested input registers, in order.
    ReadInputRegisters { registers: Vec<u16> },
    /// 0x05 — the echo: address + the coil state as the peer stored it.
    WriteSingleCoil { address: u16, on: bool },
    /// 0x06 — the echo: address + value.
    WriteSingleRegister { address: u16, value: u16 },
    /// 0x0F — the echo: start + quantity.
    WriteMultipleCoils { start: u16, quantity: u16 },
    /// 0x10 — the echo: start + quantity.
    WriteMultipleRegisters { start: u16, quantity: u16 },
}

/// The pinned, closed frame-defect vocabulary (the `reason` of a
/// [`CodecError::MalformedFrame`] refusal — FIM-F03.04). Nothing here is
/// a free-form string: every defect is one of these names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FrameDefect {
    /// The MBAP protocol id was not 0.
    ProtocolIdMismatch,
    /// The declared length field was below the minimum (2: unit id +
    /// function code) — a bad-length refusal.
    LengthFieldTooSmall,
    /// The declared length field exceeded [`MAX_ADU_LENGTH_FIELD`] — a
    /// bad-length refusal raised BEFORE any allocation.
    LengthFieldTooLarge,
    /// The declared length did not match the bytes actually carried.
    LengthFieldMismatch,
    /// The PDU payload was shorter than its function code requires.
    BodyTooShort,
    /// The PDU carried bytes beyond its function code's shape.
    TrailingBytes,
    /// The byte-count field of a read response disagreed with the
    /// requested quantity (or was not a whole register multiple).
    ByteCountMismatch,
    /// The peer answered a different function code than the request (or
    /// an exception for a different function code).
    UnexpectedResponseShape,
    /// The peer's response registers/bytes did not match what this
    /// module's contract guarantees (a defensive arm for trait-object
    /// misuse — never expected from a decoded frame).
    UnexpectedPayload,
}

/// Errors returned by the codec. Every refusal is named; the transport
/// maps these onto [`crate::transport::TransportError`] (ONE total
/// mapping in [`crate::tcp_transport`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodecError {
    /// An empty frame was handed to the decoder.
    FrameEmpty,
    /// Fewer than the 7 MBAP bytes were available.
    HeaderTooShort { got: usize },
    /// The MBAP protocol id was not [`PROTOCOL_ID`].
    ProtocolIdMismatch { expected: u16, got: u16 },
    /// The transaction id did not match the outstanding request
    /// (FIM-F03.02: a mismatched response is refused, never accepted).
    TransactionIdMismatch { expected: u16, got: u16 },
    /// The unit id did not match the configured device unit id.
    UnitIdMismatch { expected: u8, got: u8 },
    /// The response's function code does not answer the request.
    UnexpectedFunctionCode { expected: u8, got: u8 },
    /// The peer answered with a Modbus exception response.
    ExceptionResponse { function_code: u8, exception: ModbusExceptionCode },
    /// A frame-level defect (bad length, short body, trailing bytes —
    /// the closed [`FrameDefect`] vocabulary).
    MalformedFrame { defect: FrameDefect },
    /// A request count outside the function code's protocol limit
    /// (including zero — the UNVERIFIED limit anchors live on the
    /// constants).
    CountOutOfRange { count: u16, limit: u16 },
    /// A request's start address plus count overflowed the register
    /// space.
    AddressOverflow,
    /// A single-coil write carried a value that is neither 0x0000 nor
    /// 0xFF00 (UNVERIFIED anchor: the protocol text fixes those two
    /// coil-state encodings).
    InvalidCoilState { got: u16 },
}

impl std::fmt::Display for CodecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CodecError::FrameEmpty => write!(f, "the frame is empty"),
            CodecError::HeaderTooShort { got } => {
                write!(f, "the MBAP header needs 7 bytes, got {got}")
            }
            CodecError::ProtocolIdMismatch { expected, got } => {
                write!(f, "MBAP protocol id {got} is not the required {expected}")
            }
            CodecError::TransactionIdMismatch { expected, got } => {
                write!(f, "response transaction id {got} does not match the outstanding request {expected}; refusing")
            }
            CodecError::UnitIdMismatch { expected, got } => {
                write!(f, "response unit id {got} does not match the configured unit id {expected}; refusing")
            }
            CodecError::UnexpectedFunctionCode { expected, got } => {
                write!(f, "response function code {got:#04x} does not answer the request ({expected:#04x})")
            }
            CodecError::ExceptionResponse { function_code, exception } => {
                write!(f, "the device answered function code {function_code:#04x} with a Modbus exception: {}", exception.name())
            }
            CodecError::MalformedFrame { defect } => {
                write!(f, "malformed Modbus TCP frame: {defect:?}")
            }
            CodecError::CountOutOfRange { count, limit } => {
                write!(f, "request count {count} is outside the function code's limit 1..={limit}")
            }
            CodecError::AddressOverflow => {
                write!(f, "request start address plus count overflows the register space")
            }
            CodecError::InvalidCoilState { got } => {
                write!(f, "single-coil state {got:#06x} is neither 0x0000 nor 0xFF00")
            }
        }
    }
}

impl std::error::Error for CodecError {}

/// The decoded MBAP header (the transport reads these 7 bytes FIRST so a
/// bad length field is refused before the body is read or allocated).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MbapHeader {
    /// Transaction id (echo-checked against the outstanding request).
    pub transaction_id: u16,
    /// Protocol id (must be [`PROTOCOL_ID`]).
    pub protocol_id: u16,
    /// Length field: bytes after the length field, including the unit id.
    pub length: u16,
    /// Unit id.
    pub unit_id: u8,
}

/// Decodes and BOUNDS-CHECKS an MBAP header from the first 7 bytes of a
/// frame. The bounds check (protocol id, length 2..=[`MAX_ADU_LENGTH_FIELD`])
/// runs BEFORE the body is read or allocated (no hang, no oversized read).
pub fn decode_mbap_header(bytes: &[u8]) -> Result<MbapHeader, CodecError> {
    if bytes.len() < 7 {
        return Err(CodecError::HeaderTooShort { got: bytes.len() });
    }
    let transaction_id = u16::from_be_bytes([bytes[0], bytes[1]]);
    let protocol_id = u16::from_be_bytes([bytes[2], bytes[3]]);
    let length = u16::from_be_bytes([bytes[4], bytes[5]]);
    let unit_id = bytes[6];
    if protocol_id != PROTOCOL_ID {
        return Err(CodecError::ProtocolIdMismatch {
            expected: PROTOCOL_ID,
            got: protocol_id,
        });
    }
    if length < 2 {
        return Err(CodecError::MalformedFrame { defect: FrameDefect::LengthFieldTooSmall });
    }
    if length > MAX_ADU_LENGTH_FIELD {
        return Err(CodecError::MalformedFrame { defect: FrameDefect::LengthFieldTooLarge });
    }
    Ok(MbapHeader { transaction_id, protocol_id, length, unit_id })
}

/// Validates a read/multi-write count against its function-code limit.
fn assert_count(count: u16, limit: u16) -> Result<(), CodecError> {
    if count == 0 || count > limit {
        return Err(CodecError::CountOutOfRange { count, limit });
    }
    Ok(())
}

/// Encodes one complete Modbus TCP ADU (MBAP header + PDU) for `request`.
///
/// Every count limit is validated on the way in (fail-closed): a request
/// outside the function code's protocol limit is a named refusal, never a
/// truncated encode.
pub fn encode_request(
    transaction_id: u16,
    unit_id: u8,
    request: &PduRequest,
) -> Result<Vec<u8>, CodecError> {
    let pdu: Vec<u8> = match request {
        PduRequest::ReadCoils { start, count } => {
            assert_count(*count, MAX_READ_BITS)?;
            assert_window(*start, *count)?;
            pdu_read(FunctionCode::ReadCoils, *start, *count)
        }
        PduRequest::ReadDiscreteInputs { start, count } => {
            assert_count(*count, MAX_READ_BITS)?;
            assert_window(*start, *count)?;
            pdu_read(FunctionCode::ReadDiscreteInputs, *start, *count)
        }
        PduRequest::ReadHoldingRegisters { start, count } => {
            // polling.rs owns the read limit (ONE source): the codec's
            // register-read limit IS MAX_REGISTERS_PER_READ.
            assert_count(*count, MAX_REGISTERS_PER_READ)?;
            assert_window(*start, *count)?;
            pdu_read(FunctionCode::ReadHoldingRegisters, *start, *count)
        }
        PduRequest::ReadInputRegisters { start, count } => {
            assert_count(*count, MAX_REGISTERS_PER_READ)?;
            assert_window(*start, *count)?;
            pdu_read(FunctionCode::ReadInputRegisters, *start, *count)
        }
        PduRequest::WriteSingleCoil { address, on } => {
            let mut pdu = Vec::with_capacity(5);
            pdu.push(FunctionCode::WriteSingleCoil.value());
            pdu.extend_from_slice(&address.to_be_bytes());
            // UNVERIFIED anchor: 0xFF00 = on, 0x0000 = off (protocol text).
            let state: u16 = if *on { 0xFF00 } else { 0x0000 };
            pdu.extend_from_slice(&state.to_be_bytes());
            pdu
        }
        PduRequest::WriteSingleRegister { address, value } => {
            let mut pdu = Vec::with_capacity(5);
            pdu.push(FunctionCode::WriteSingleRegister.value());
            pdu.extend_from_slice(&address.to_be_bytes());
            pdu.extend_from_slice(&value.to_be_bytes());
            pdu
        }
        PduRequest::WriteMultipleCoils { start, bits } => {
            let count = u16::try_from(bits.len())
                .map_err(|_| CodecError::CountOutOfRange { count: u16::MAX, limit: MAX_WRITE_BITS })?;
            assert_count(count, MAX_WRITE_BITS)?;
            assert_window(*start, count)?;
            // count ≤ MAX_WRITE_BITS was asserted above, so the packed
            // byte count (≤ 246) always fits u16 and u8; the try_from
            // arms below are totality, not reachable failure paths.
            let byte_count_u16 = u16::try_from(bits.len().div_ceil(8))
                .map_err(|_| CodecError::CountOutOfRange { count, limit: MAX_WRITE_BITS })?;
            let byte_count = usize::from(byte_count_u16);
            let mut pdu = Vec::with_capacity(6 + byte_count);
            pdu.push(FunctionCode::WriteMultipleCoils.value());
            pdu.extend_from_slice(&start.to_be_bytes());
            pdu.extend_from_slice(&count.to_be_bytes());
            pdu.push(u8::try_from(byte_count)
                .map_err(|_| CodecError::CountOutOfRange { count, limit: MAX_WRITE_BITS })?);
            pdu.extend_from_slice(&pack_bits(bits));
            pdu
        }
        PduRequest::WriteMultipleRegisters { start, values } => {
            let count = u16::try_from(values.len()).map_err(|_| {
                CodecError::CountOutOfRange { count: u16::MAX, limit: MAX_WRITE_REGISTERS }
            })?;
            assert_count(count, MAX_WRITE_REGISTERS)?;
            assert_window(*start, count)?;
            let mut pdu = Vec::with_capacity(6 + values.len() * 2);
            pdu.push(FunctionCode::WriteMultipleRegisters.value());
            pdu.extend_from_slice(&start.to_be_bytes());
            pdu.extend_from_slice(&count.to_be_bytes());
            pdu.push((values.len() * 2) as u8);
            for value in values {
                pdu.extend_from_slice(&value.to_be_bytes());
            }
            pdu
        }
    };
    // MBAP: transaction id, protocol id, length (PDU + unit id), unit id.
    let length = (pdu.len() + 1) as u16;
    if usize::from(length) > usize::from(MAX_ADU_LENGTH_FIELD) {
        return Err(CodecError::MalformedFrame { defect: FrameDefect::LengthFieldTooLarge });
    }
    let mut frame = Vec::with_capacity(6 + pdu.len() + 1);
    frame.extend_from_slice(&transaction_id.to_be_bytes());
    frame.extend_from_slice(&PROTOCOL_ID.to_be_bytes());
    frame.extend_from_slice(&length.to_be_bytes());
    frame.push(unit_id);
    frame.extend_from_slice(&pdu);
    Ok(frame)
}

/// Refuses a start..start+count window that overflows the u16 register
/// space (the encode-time arm of the overflow refusal).
fn assert_window(start: u16, count: u16) -> Result<(), CodecError> {
    if start.checked_add(count).is_none() {
        return Err(CodecError::AddressOverflow);
    }
    Ok(())
}

/// The read-request PDU: function code, start (big-endian), count
/// (big-endian).
fn pdu_read(function_code: FunctionCode, start: u16, count: u16) -> Vec<u8> {
    let mut pdu = Vec::with_capacity(5);
    pdu.push(function_code.value());
    pdu.extend_from_slice(&start.to_be_bytes());
    pdu.extend_from_slice(&count.to_be_bytes());
    pdu
}

/// Packs bits into bytes, LSB-first within each byte (UNVERIFIED anchor),
/// zero-padding the final byte.
fn pack_bits(bits: &[bool]) -> Vec<u8> {
    let mut bytes = vec![0u8; bits.len().div_ceil(8)];
    for (index, bit) in bits.iter().enumerate() {
        if *bit {
            bytes[index / 8] |= 1u8 << (index % 8);
        }
    }
    bytes
}

/// Unpacks `quantity` bits from packed bytes (LSB-first), refusing a
/// byte-count field that cannot carry the requested quantity.
fn unpack_bits(bytes: &[u8], quantity: u16) -> Result<Vec<bool>, CodecError> {
    let need = u16::try_from(quantity.div_ceil(8)).unwrap_or(u16::MAX);
    let declared = u16::try_from(bytes.len()).unwrap_or(u16::MAX);
    if declared < need {
        return Err(CodecError::MalformedFrame { defect: FrameDefect::ByteCountMismatch });
    }
    let mut bits = Vec::with_capacity(usize::from(quantity));
    for index in 0..usize::from(quantity) {
        bits.push(bytes[index / 8] & (1u8 << (index % 8)) != 0);
    }
    Ok(bits)
}

/// Decodes one complete Modbus TCP ADU into a [`PduResponse`].
///
/// Checks, in order: frame length consistency, transaction id, unit id,
/// function code, then the payload shape of the ANSWERED function code.
/// An exception response (function code | 0x80) decodes into the named
/// [`ModbusExceptionCode`] vocabulary. `expected_quantity` is the
/// requested bit/register count for read codes (ignored for single-write
/// echoes — pass the request's [`PduRequest::quantity`]).
pub fn decode_response(
    frame: &[u8],
    expected_transaction_id: u16,
    expected_unit_id: u8,
    expected_function_code: FunctionCode,
    expected_quantity: u16,
) -> Result<PduResponse, CodecError> {
    let header = decode_mbap_header(frame)?;
    if usize::from(header.length) != frame.len() - 6 {
        return Err(CodecError::MalformedFrame {
            defect: FrameDefect::LengthFieldMismatch,
        });
    }
    if header.transaction_id != expected_transaction_id {
        return Err(CodecError::TransactionIdMismatch {
            expected: expected_transaction_id,
            got: header.transaction_id,
        });
    }
    if header.unit_id != expected_unit_id {
        return Err(CodecError::UnitIdMismatch {
            expected: expected_unit_id,
            got: header.unit_id,
        });
    }
    let pdu = &frame[7..];
    let function_byte = pdu[0];
    // Exception responses carry the requested function code | 0x80.
    if function_byte & 0x80 != 0 {
        let masked = function_byte & 0x7F;
        if masked != expected_function_code.value() {
            return Err(CodecError::UnexpectedFunctionCode {
                expected: expected_function_code.value(),
                got: masked,
            });
        }
        if pdu.len() < 2 {
            return Err(CodecError::MalformedFrame { defect: FrameDefect::BodyTooShort });
        }
        if pdu.len() > 2 {
            return Err(CodecError::MalformedFrame { defect: FrameDefect::TrailingBytes });
        }
        return Err(CodecError::ExceptionResponse {
            function_code: function_byte,
            exception: ModbusExceptionCode::from_byte(pdu[1]),
        });
    }
    if function_byte != expected_function_code.value() {
        return Err(CodecError::UnexpectedFunctionCode {
            expected: expected_function_code.value(),
            got: function_byte,
        });
    }
    let body = &pdu[1..];
    let response = match expected_function_code {
        FunctionCode::ReadCoils | FunctionCode::ReadDiscreteInputs => {
            if body.is_empty() {
                return Err(CodecError::MalformedFrame { defect: FrameDefect::BodyTooShort });
            }
            let byte_count = usize::from(body[0]);
            if byte_count == 0 || body.len() < 1 + byte_count {
                return Err(CodecError::MalformedFrame { defect: FrameDefect::BodyTooShort });
            }
            if byte_count != usize::from(expected_quantity.div_ceil(8)) {
                return Err(CodecError::MalformedFrame { defect: FrameDefect::ByteCountMismatch });
            }
            if body.len() > 1 + byte_count {
                return Err(CodecError::MalformedFrame { defect: FrameDefect::TrailingBytes });
            }
            let bits = unpack_bits(&body[1..1 + byte_count], expected_quantity)?;
            if expected_function_code == FunctionCode::ReadCoils {
                PduResponse::ReadCoils { bits }
            } else {
                PduResponse::ReadDiscreteInputs { bits }
            }
        }
        FunctionCode::ReadHoldingRegisters | FunctionCode::ReadInputRegisters => {
            if body.is_empty() {
                return Err(CodecError::MalformedFrame { defect: FrameDefect::BodyTooShort });
            }
            let byte_count = usize::from(body[0]);
            if byte_count == 0 || body.len() < 1 + byte_count {
                return Err(CodecError::MalformedFrame { defect: FrameDefect::BodyTooShort });
            }
            if byte_count != usize::from(expected_quantity) * 2 {
                return Err(CodecError::MalformedFrame { defect: FrameDefect::ByteCountMismatch });
            }
            if body.len() > 1 + byte_count {
                return Err(CodecError::MalformedFrame { defect: FrameDefect::TrailingBytes });
            }
            let mut registers = Vec::with_capacity(usize::from(expected_quantity));
            for pair in body[1..1 + byte_count].chunks_exact(2) {
                registers.push(u16::from_be_bytes([pair[0], pair[1]]));
            }
            if expected_function_code == FunctionCode::ReadHoldingRegisters {
                PduResponse::ReadHoldingRegisters { registers }
            } else {
                PduResponse::ReadInputRegisters { registers }
            }
        }
        FunctionCode::WriteSingleCoil => {
            if body.len() < 4 {
                return Err(CodecError::MalformedFrame { defect: FrameDefect::BodyTooShort });
            }
            if body.len() > 4 {
                return Err(CodecError::MalformedFrame { defect: FrameDefect::TrailingBytes });
            }
            let address = u16::from_be_bytes([body[0], body[1]]);
            let state = u16::from_be_bytes([body[2], body[3]]);
            let on = match state {
                0xFF00 => true,
                0x0000 => false,
                other => return Err(CodecError::InvalidCoilState { got: other }),
            };
            PduResponse::WriteSingleCoil { address, on }
        }
        FunctionCode::WriteSingleRegister => {
            if body.len() < 4 {
                return Err(CodecError::MalformedFrame { defect: FrameDefect::BodyTooShort });
            }
            if body.len() > 4 {
                return Err(CodecError::MalformedFrame { defect: FrameDefect::TrailingBytes });
            }
            PduResponse::WriteSingleRegister {
                address: u16::from_be_bytes([body[0], body[1]]),
                value: u16::from_be_bytes([body[2], body[3]]),
            }
        }
        FunctionCode::WriteMultipleCoils | FunctionCode::WriteMultipleRegisters => {
            if body.len() < 4 {
                return Err(CodecError::MalformedFrame { defect: FrameDefect::BodyTooShort });
            }
            if body.len() > 4 {
                return Err(CodecError::MalformedFrame { defect: FrameDefect::TrailingBytes });
            }
            let (start, quantity) = (u16::from_be_bytes([body[0], body[1]]), u16::from_be_bytes([body[2], body[3]]));
            if expected_function_code == FunctionCode::WriteMultipleCoils {
                PduResponse::WriteMultipleCoils { start, quantity }
            } else {
                PduResponse::WriteMultipleRegisters { start, quantity }
            }
        }
    };
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FIM-F03.05 — hand-computed pinned vector: a 0x03 read-holding-
    /// registers request encodes to exactly these bytes.
    /// txn=0x1234, proto=0, length=6 (unit + 5 PDU bytes), unit=0x11,
    /// fc=0x03, start=0x006B, count=0x0002.
    #[test]
    fn read_holding_registers_request_encodes_to_the_pinned_vector() {
        let frame = encode_request(
            0x1234,
            0x11,
            &PduRequest::ReadHoldingRegisters { start: 0x006B, count: 2 },
        )
        .expect("encodes");
        assert_eq!(
            frame,
            vec![0x12, 0x34, 0x00, 0x00, 0x00, 0x06, 0x11, 0x03, 0x00, 0x6B, 0x00, 0x02]
        );
    }

    /// Pinned vector: the matching success response decodes to the two
    /// registers 0x01F4 (500) and 0x0064 (100), big-endian per register.
    #[test]
    fn read_holding_registers_response_decodes_to_the_pinned_vector() {
        let frame = [
            0x12, 0x34, 0x00, 0x00, 0x00, 0x07, 0x11, 0x03, 0x04, 0x01, 0xF4, 0x00, 0x64,
        ];
        let response = decode_response(&frame, 0x1234, 0x11, FunctionCode::ReadHoldingRegisters, 2)
            .expect("decodes");
        assert_eq!(
            response,
            PduResponse::ReadHoldingRegisters { registers: vec![500, 100] }
        );
    }

    /// Pinned vector: an exception frame (fc 0x83, exception code 0x02)
    /// decodes to the named IllegalDataAddress refusal.
    #[test]
    fn exception_frame_decodes_to_the_named_refusal() {
        let frame = [0x12, 0x34, 0x00, 0x00, 0x00, 0x03, 0x11, 0x83, 0x02];
        assert_eq!(
            decode_response(&frame, 0x1234, 0x11, FunctionCode::ReadHoldingRegisters, 2),
            Err(CodecError::ExceptionResponse {
                function_code: 0x83,
                exception: ModbusExceptionCode::IllegalDataAddress,
            })
        );
    }

    /// FIM-F03.05 — malformed-length refusals: a length field of 0, a
    /// length field above the ADU bound, and a declared length that does
    /// not match the carried bytes. No allocation happens past the bound.
    #[test]
    fn malformed_lengths_are_named_refusals() {
        // length = 0: below the minimum of 2 (unit id + function code).
        assert_eq!(
            decode_mbap_header(&[0x12, 0x34, 0x00, 0x00, 0x00, 0x00, 0x11]),
            Err(CodecError::MalformedFrame { defect: FrameDefect::LengthFieldTooSmall })
        );
        // length = 0x00FF (255) > MAX_ADU_LENGTH_FIELD (254).
        assert_eq!(
            decode_mbap_header(&[0x12, 0x34, 0x00, 0x00, 0x00, 0xFF, 0x11]),
            Err(CodecError::MalformedFrame { defect: FrameDefect::LengthFieldTooLarge })
        );
        // Header says 7 body bytes; the frame carries only 6.
        let frame = [
            0x12, 0x34, 0x00, 0x00, 0x00, 0x07, 0x11, 0x03, 0x04, 0x01, 0xF4, 0x00,
        ];
        assert_eq!(
            decode_response(&frame, 0x1234, 0x11, FunctionCode::ReadHoldingRegisters, 2),
            Err(CodecError::MalformedFrame { defect: FrameDefect::LengthFieldMismatch })
        );
    }

    /// FIM-F03.05 — the transaction-id mismatch refusal (a mismatched
    /// response is refused, never accepted; FIM-F03.02's named error).
    #[test]
    fn transaction_id_mismatch_is_a_named_refusal() {
        let frame = [
            0x12, 0x35, 0x00, 0x00, 0x00, 0x07, 0x11, 0x03, 0x04, 0x01, 0xF4, 0x00, 0x64,
        ];
        assert_eq!(
            decode_response(&frame, 0x1234, 0x11, FunctionCode::ReadHoldingRegisters, 2),
            Err(CodecError::TransactionIdMismatch { expected: 0x1234, got: 0x1235 })
        );
    }

    /// The unit-id mismatch refusal (FIM-F03.04's named refusal).
    #[test]
    fn unit_id_mismatch_is_a_named_refusal() {
        let frame = [
            0x12, 0x34, 0x00, 0x00, 0x00, 0x07, 0x12, 0x03, 0x04, 0x01, 0xF4, 0x00, 0x64,
        ];
        assert_eq!(
            decode_response(&frame, 0x1234, 0x11, FunctionCode::ReadHoldingRegisters, 2),
            Err(CodecError::UnitIdMismatch { expected: 0x11, got: 0x12 })
        );
    }

    /// Round-trip: coils pack LSB-first and the byte-count field matches
    /// ceil(quantity / 8). Pinned vector: txn=0xABCD, unit=0x01, one data
    /// byte 0x2D = 0b00101101 → coils 0..4 = [on, off, on, on, off]; the
    /// two pad bits are never surfaced.
    #[test]
    fn read_coils_round_trips_lsb_first_with_the_pinned_vector() {
        let request = PduRequest::ReadCoils { start: 0x0013, count: 5 };
        let frame = encode_request(0xABCD, 0x01, &request).expect("encodes");
        assert_eq!(
            frame,
            vec![0xAB, 0xCD, 0x00, 0x00, 0x00, 0x06, 0x01, 0x01, 0x00, 0x13, 0x00, 0x05]
        );
        // The device answers one data byte 0x2D (0b00101101); the response
        // MBAP length is 1 + 1 + 1 + 1 = 4.
        let response_frame = [
            0xAB, 0xCD, 0x00, 0x00, 0x00, 0x04, 0x01, 0x01, 0x01, 0x2D,
        ];
        assert_eq!(
            decode_response(&response_frame, 0xABCD, 0x01, FunctionCode::ReadCoils, 5).expect("decodes"),
            PduResponse::ReadCoils { bits: vec![true, false, true, true, false] }
        );
    }

    /// Discrete inputs round-trip (pinned request vector: txn=0x00AA,
    /// unit=0x03, start=0x00C8, count=16).
    #[test]
    fn read_discrete_inputs_request_encodes_to_the_pinned_vector() {
        let frame =
            encode_request(0x00AA, 0x03, &PduRequest::ReadDiscreteInputs { start: 0x00C8, count: 16 })
                .expect("encodes");
        assert_eq!(
            frame,
            vec![0x00, 0xAA, 0x00, 0x00, 0x00, 0x06, 0x03, 0x02, 0x00, 0xC8, 0x00, 0x10]
        );
    }

    /// Input registers response (pinned vector: registers [1, 0x0203]).
    #[test]
    fn read_input_registers_response_decodes_to_the_pinned_vector() {
        let frame = [
            0x00, 0xAA, 0x00, 0x00, 0x00, 0x07, 0x03, 0x04, 0x04, 0x00, 0x01, 0x02, 0x03,
        ];
        assert_eq!(
            decode_response(&frame, 0x00AA, 0x03, FunctionCode::ReadInputRegisters, 2).expect("decodes"),
            PduResponse::ReadInputRegisters { registers: vec![1, 0x0203] }
        );
    }

    /// Write-single-coil and write-single-register round-trips (the
    /// wire-completeness subset; the transport exposes no write method).
    #[test]
    fn single_writes_round_trip() {
        let coil = PduRequest::WriteSingleCoil { address: 0x00AC, on: true };
        let frame = encode_request(0x0003, 0x01, &coil).expect("encodes");
        assert_eq!(
            frame,
            vec![0x00, 0x03, 0x00, 0x00, 0x00, 0x06, 0x01, 0x05, 0x00, 0xAC, 0xFF, 0x00]
        );
        // The echo is byte-identical to the request PDU.
        assert_eq!(
            decode_response(&frame, 0x0003, 0x01, FunctionCode::WriteSingleCoil, 0).expect("decodes"),
            PduResponse::WriteSingleCoil { address: 0x00AC, on: true }
        );
        let register = PduRequest::WriteSingleRegister { address: 0x0005, value: 0x0A10 };
        let frame = encode_request(0x0001, 0x02, &register).expect("encodes");
        assert_eq!(
            frame,
            vec![0x00, 0x01, 0x00, 0x00, 0x00, 0x06, 0x02, 0x06, 0x00, 0x05, 0x0A, 0x10]
        );
        assert_eq!(
            decode_response(&frame, 0x0001, 0x02, FunctionCode::WriteSingleRegister, 0).expect("decodes"),
            PduResponse::WriteSingleRegister { address: 0x0005, value: 0x0A10 }
        );
    }

    /// Multiple-coils / multiple-registers round-trips with pinned
    /// vectors (LSB-first packing; 5 bits → one byte 0x0B).
    #[test]
    fn multiple_writes_round_trip_with_pinned_vectors() {
        let coils = PduRequest::WriteMultipleCoils {
            start: 0x0014,
            bits: vec![true, true, false, true, false],
        };
        let frame = encode_request(0x0007, 0x01, &coils).expect("encodes");
        assert_eq!(
            frame,
            vec![0x00, 0x07, 0x00, 0x00, 0x00, 0x08, 0x01, 0x0F, 0x00, 0x14, 0x00, 0x05, 0x01, 0x0B]
        );
        assert_eq!(
            decode_response(&frame, 0x0007, 0x01, FunctionCode::WriteMultipleCoils, 0).expect("decodes"),
            PduResponse::WriteMultipleCoils { start: 0x0014, quantity: 5 }
        );
        let registers = PduRequest::WriteMultipleRegisters {
            start: 0x0003,
            values: vec![0x1234, 0xABCD],
        };
        let frame = encode_request(0x0002, 0x05, &registers).expect("encodes");
        assert_eq!(
            frame,
            vec![
                0x00, 0x02, 0x00, 0x00, 0x00, 0x0B, 0x05, 0x10, 0x00, 0x03, 0x00, 0x02, 0x04,
                0x12, 0x34, 0xAB, 0xCD,
            ]
        );
        assert_eq!(
            decode_response(&frame, 0x0002, 0x05, FunctionCode::WriteMultipleRegisters, 0)
                .expect("decodes"),
            PduResponse::WriteMultipleRegisters { start: 0x0003, quantity: 2 }
        );
    }

    /// Count limits are fail-closed on encode: zero, over-limit, and a
    /// register read above the polling.rs limit (ONE source: 125).
    #[test]
    fn count_limits_are_enforced_on_encode() {
        assert_eq!(
            encode_request(1, 1, &PduRequest::ReadHoldingRegisters { start: 0, count: 0 }),
            Err(CodecError::CountOutOfRange { count: 0, limit: MAX_REGISTERS_PER_READ })
        );
        assert_eq!(
            encode_request(1, 1, &PduRequest::ReadHoldingRegisters { start: 0, count: 126 }),
            Err(CodecError::CountOutOfRange { count: 126, limit: MAX_REGISTERS_PER_READ })
        );
        assert_eq!(
            encode_request(1, 1, &PduRequest::ReadCoils { start: 0, count: 2_001 }),
            Err(CodecError::CountOutOfRange { count: 2_001, limit: MAX_READ_BITS })
        );
        // start + count overflows the register space.
        assert_eq!(
            encode_request(1, 1, &PduRequest::ReadHoldingRegisters { start: 0xFFFF, count: 2 }),
            Err(CodecError::AddressOverflow)
        );
    }

    /// A read response whose byte-count field disagrees with the
    /// requested quantity is a named refusal (desync guard).
    #[test]
    fn byte_count_mismatch_is_a_named_refusal() {
        // The device declares 4 data bytes for a 3-register request.
        let frame = [
            0x12, 0x34, 0x00, 0x00, 0x00, 0x07, 0x11, 0x03, 0x04, 0x01, 0xF4, 0x00, 0x64,
        ];
        assert_eq!(
            decode_response(&frame, 0x1234, 0x11, FunctionCode::ReadHoldingRegisters, 3),
            Err(CodecError::MalformedFrame { defect: FrameDefect::ByteCountMismatch })
        );
        // The device declares 4 data bytes but carries only 2 (the MBAP
        // length matches the carried bytes, so the mismatch is INSIDE the
        // PDU: byte-count 4 vs a 2-byte body).
        let short = [
            0x12, 0x34, 0x00, 0x00, 0x00, 0x06, 0x11, 0x03, 0x04, 0x01, 0xF4,
        ];
        assert_eq!(
            decode_response(&short, 0x1234, 0x11, FunctionCode::ReadHoldingRegisters, 2),
            Err(CodecError::MalformedFrame { defect: FrameDefect::BodyTooShort })
        );
    }

    /// Trailing bytes beyond the function code's shape are refused.
    #[test]
    fn trailing_bytes_are_refused() {
        // A 0x06 echo carrying one extra byte.
        let frame = [
            0x00, 0x01, 0x00, 0x00, 0x00, 0x07, 0x02, 0x06, 0x00, 0x05, 0x0A, 0x10, 0x00,
        ];
        assert_eq!(
            decode_response(&frame, 0x0001, 0x02, FunctionCode::WriteSingleRegister, 0),
            Err(CodecError::MalformedFrame { defect: FrameDefect::TrailingBytes })
        );
    }

    /// A single-coil echo carrying neither 0x0000 nor 0xFF00 is refused
    /// (the UNVERIFIED coil-state anchor is a named refusal, not a
    /// truthiness guess).
    #[test]
    fn invalid_coil_state_is_refused() {
        let frame = [
            0x00, 0x03, 0x00, 0x00, 0x00, 0x06, 0x01, 0x05, 0x00, 0xAC, 0x12, 0x34,
        ];
        assert_eq!(
            decode_response(&frame, 0x0003, 0x01, FunctionCode::WriteSingleCoil, 0),
            Err(CodecError::InvalidCoilState { got: 0x1234 })
        );
    }

    /// A response to a DIFFERENT function code is refused (never
    /// reinterpreted to fit the request).
    #[test]
    fn wrong_function_code_is_refused() {
        let frame = [
            0x00, 0xAA, 0x00, 0x00, 0x00, 0x07, 0x03, 0x04, 0x04, 0x00, 0x01, 0x02, 0x03,
        ];
        assert_eq!(
            decode_response(&frame, 0x00AA, 0x03, FunctionCode::ReadHoldingRegisters, 2),
            Err(CodecError::UnexpectedFunctionCode { expected: 0x03, got: 0x04 })
        );
    }

    /// The exception-code vocabulary is total: every known byte maps to
    /// its name, and an unknown byte stays a named `Unknown` refusal.
    #[test]
    fn exception_code_mapping_is_total() {
        for byte in ModbusExceptionCode::KNOWN_BYTES {
            let code = ModbusExceptionCode::from_byte(byte);
            assert!(!matches!(code, ModbusExceptionCode::Unknown(_)));
            assert!(!code.name().is_empty());
        }
        assert_eq!(
            ModbusExceptionCode::from_byte(0x77),
            ModbusExceptionCode::Unknown(0x77)
        );
        assert_eq!(ModbusExceptionCode::Unknown(0x77).name(), "unknown(0x77)");
    }

    /// The covered function-code byte set round-trips through
    /// `from_byte`, and bytes outside the subset are `None` (a covered-
    /// subset refusal — the codec names what it does not cover).
    #[test]
    fn function_code_byte_mapping_round_trips() {
        let covered = [
            FunctionCode::ReadCoils,
            FunctionCode::ReadDiscreteInputs,
            FunctionCode::ReadHoldingRegisters,
            FunctionCode::ReadInputRegisters,
            FunctionCode::WriteSingleCoil,
            FunctionCode::WriteSingleRegister,
            FunctionCode::WriteMultipleCoils,
            FunctionCode::WriteMultipleRegisters,
        ];
        for code in covered {
            assert_eq!(FunctionCode::from_byte(code.value()), Some(code));
            assert!(!code.name().is_empty());
        }
        assert_eq!(FunctionCode::from_byte(0x07), None);
        assert_eq!(FunctionCode::from_byte(0x11), None);
    }

    /// Every codec error carries a non-empty Display (the refusal
    /// taxonomy is spellable end to end).
    #[test]
    fn every_codec_error_displays() {
        let cases = [
            CodecError::FrameEmpty,
            CodecError::HeaderTooShort { got: 3 },
            CodecError::ProtocolIdMismatch { expected: 0, got: 5 },
            CodecError::TransactionIdMismatch { expected: 1, got: 2 },
            CodecError::UnitIdMismatch { expected: 1, got: 2 },
            CodecError::UnexpectedFunctionCode { expected: 0x03, got: 0x04 },
            CodecError::ExceptionResponse {
                function_code: 0x83,
                exception: ModbusExceptionCode::IllegalDataAddress,
            },
            CodecError::MalformedFrame { defect: FrameDefect::LengthFieldTooSmall },
            CodecError::CountOutOfRange { count: 0, limit: 125 },
            CodecError::AddressOverflow,
            CodecError::InvalidCoilState { got: 0x1234 },
        ];
        for error in cases {
            assert!(!error.to_string().is_empty());
        }
    }
}
