// FIM-F03.02 / FIM-F03.04 — the LIVE Modbus TCP client transport
// (docs/releases/final-implementation-before-publication.md §5 F-03;
// ledger docs/releases/implementation_ledger/ledger_F03.md subtasks
// FIM-F03.02 and FIM-F03.04).
//
// **Implemented under FIM-F03, WRITTEN-UNRUN: this module has never been
// compiled or executed, no TCP connection has ever been opened, and no
// device has ever answered. Interop with a real Modbus TCP device is
// UNPROVEN and remains a HUMAN LAB GATE** (crate conventions; the gated
// socket suite `tests/live_transport_gated.rs` skips cleanly without its
// `FLEXCORE_TEST_MODBUS_HOST` gate and is evidence preparation, not a
// result). The deterministic in-module tests below drive the framed
// exchange over in-memory tokio duplex pipes — no socket is opened.
//
// What the module implements, against the seam designed in
// `transport.rs` (the trait, the configuration types, and
// `UnwiredTransport` are UNCHANGED):
//
// - **FIM-F03.02 — the tokio TCP client.** [`LiveTcpTransport`]
//   implements the EXISTING synchronous `ModbusTransport` port trait by
//   owning its own current-thread tokio runtime and driving each read
//   with `block_on` (the trait is runtime-free by house rule; a concrete
//   adapter owns its I/O shape). Every await path is bounded: connect by
//   the declared `connect_timeout_millis`, each request/response by the
//   declared `response_timeout_millis` — there is NO unbounded wait
//   anywhere. Transaction ids are matched (a mismatched response is the
//   named refusal, FIM-F03.01's codec check); the connection is kept
//   across reads and re-established with a backoff derived from the
//   declared [`GatewayPollingBudget`] (`min_poll_interval_millis` is the
//   reconnect floor — the budget the gateway already enforces); every
//   read is budget-checked (`assert_read_fits`) BEFORE any wire traffic.
//   [`LiveTcpTransport::close`] performs a graceful shutdown; `Drop`
//   closes best-effort (a socket closed without the shutdown handshake —
//   the honest bound of a drop path).
// - **FIM-F03.04 — the ONE total refusal mapping.** [`map_codec_error`]
//   maps every [`CodecError`] onto the (additively extended)
//   [`TransportError`] taxonomy; [`map_io_error`] maps I/O errors. The
//   illegal-data-address exception (0x02) maps onto the pre-existing
//   `WindowOutOfRange` exactly as that variant's doc comment reserves.
//   Every refusal is named; nothing hangs, panics, or is swallowed. The
//   sync trait method refuses with `RuntimeContextRefused` instead of
//   panicking if called inside a running tokio runtime context.
// - **RTU stays unwired.** Selecting `TransportConfig::Rtu` with the
//   live TCP transport is the honest `SeamNotWired` refusal: no serial
//   implementation exists in this program.
//
// UNVERIFIED-ASSUMPTION register (module-level; record 23 §4 discipline
// — a mismatch found against the protocol text / a lab device is a code
// or record edit, never a silent reinterpretation):
//
// - The FC 0x03 read-holding-registers choice and the addressing base
//   (the caller passes PROTOCOL addresses — the 4xxxx-style 40001 base
//   is a configuration translation, declared per deployment, never
//   assumed here).
// - The reconnect backoff floor (the declared minimum poll interval) is
//   a deployment-budget choice, not a protocol constant.
// - No keep-alive, no pipelining: one outstanding request per
//   connection, synchronous request/response — the shape the port trait
//   declares. A peer that multiplexes responses on one connection is a
//   lab-gate finding, not a reinterpretation here.

use std::sync::atomic::{AtomicU16, Ordering};

use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::codec::{
    decode_mbap_header, decode_response, encode_request, CodecError, FrameDefect, PduRequest,
    PduResponse,
};
use crate::polling::GatewayPollingBudget;
use crate::transport::{ModbusTransport, TransportConfig, TransportError};

/// The declared configuration of ONE live Modbus TCP client path.
/// Constructed through [`LiveTcpConfig::new`] — no `Default`, no
/// partially-filled configuration (the declared-never-defaulted
/// discipline the whole crate enforces). Every timeout is declared in
/// milliseconds; there are no protocol defaults here (even the well-known
/// port is an explicit configuration choice — transport.rs).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveTcpConfig {
    /// The TCP endpoint (must be a `TransportConfig::Tcp` variant; an RTU
    /// variant is refused at transport construction — the RTU seam stays
    /// unwired).
    pub tcp: TransportConfig,
    /// Connect timeout in milliseconds (> 0, declared).
    pub connect_timeout_millis: u64,
    /// Response timeout in milliseconds (> 0, declared) — bounds every
    /// request/response exchange (no hang anywhere).
    pub response_timeout_millis: u64,
    /// ADDITIONAL reconnect attempts after a failed first attempt
    /// (0 = one attempt, no reconnect). The backoff between attempts is
    /// the declared polling budget's minimum poll interval.
    pub reconnect_attempts: u32,
    /// The declared gateway polling budget: bounds the per-read register
    /// count (`assert_read_fits` before every read) and sets the
    /// reconnect backoff floor (`min_poll_interval_millis`).
    pub polling: GatewayPollingBudget,
}

impl LiveTcpConfig {
    /// Validates and constructs the configuration.
    pub fn new(
        tcp: TransportConfig,
        connect_timeout_millis: u64,
        response_timeout_millis: u64,
        reconnect_attempts: u32,
        polling: GatewayPollingBudget,
    ) -> Result<Self, TransportError> {
        tcp.validate()?;
        if connect_timeout_millis == 0 {
            return Err(TransportError::EmptyField("connect_timeout_millis"));
        }
        if response_timeout_millis == 0 {
            return Err(TransportError::EmptyField("response_timeout_millis"));
        }
        Ok(Self {
            tcp,
            connect_timeout_millis,
            response_timeout_millis,
            reconnect_attempts,
            polling,
        })
    }
}

/// The live Modbus TCP client: the FIM-F03 implementation of the
/// EXISTING [`ModbusTransport`] port trait (written-unrun; interop
/// UNPROVEN — HUMAN LAB GATE).
///
/// The transport owns its own current-thread tokio runtime (the port
/// trait is synchronous and runtime-free; a concrete adapter owns its I/O
/// shape). Reads are budget-checked, transaction-matched, timeout-bounded,
/// and reconnect-with-backoff; every failure surfaces as a named
/// [`TransportError`] (FIM-F03.04).
#[derive(Debug)]
pub struct LiveTcpTransport {
    /// The validated configuration (held unchanged).
    pub config: LiveTcpConfig,
    /// The gateway host (copied out for the named refusals).
    host: String,
    /// The gateway port (copied out for the named refusals).
    port: u16,
    /// The configured unit id (echo-checked on every response).
    unit_id: u8,
    /// The current-thread runtime the synchronous trait method drives.
    runtime: tokio::runtime::Runtime,
    /// The kept-alive connection (`None` = connect on next read).
    connection: tokio::sync::Mutex<Option<tokio::net::TcpStream>>,
    /// The next transaction id (wraps naturally at u16 bounds).
    next_transaction_id: AtomicU16,
}

impl LiveTcpTransport {
    /// Validates and constructs the live transport (no connection is
    /// opened here — the first read connects).
    pub fn new(config: LiveTcpConfig) -> Result<Self, TransportError> {
        let (host, port, unit_id) = match &config.tcp {
            TransportConfig::Tcp { host, port, unit_id } => (host.clone(), *port, *unit_id),
            // Honest refusal: the RTU serial seam remains unwired (FIM-F03
            // implemented the TCP path only; no serial implementation
            // exists in this program).
            TransportConfig::Rtu { .. } => return Err(TransportError::SeamNotWired),
        };
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| TransportError::RuntimeUnavailable)?;
        Ok(Self {
            host,
            port,
            unit_id,
            config,
            runtime,
            connection: tokio::sync::Mutex::new(None),
            next_transaction_id: AtomicU16::new(0),
        })
    }

    /// Connects (bounded by the declared connect timeout). A refused TCP
    /// connect is the named [`TransportError::ConnectionRefused`]; any
    /// other connect failure — including the declared timeout elapsing —
    /// is [`TransportError::Unreachable`] (an unreachable device never
    /// hangs past its declared timeout).
    async fn connect(&self) -> Result<tokio::net::TcpStream, TransportError> {
        let attempt = tokio::net::TcpStream::connect((self.host.as_str(), self.port));
        match tokio::time::timeout(
            std::time::Duration::from_millis(self.config.connect_timeout_millis),
            attempt,
        )
        .await
        {
            Ok(Ok(stream)) => Ok(stream),
            Ok(Err(error)) => Err(map_io_error(&self.host, self.port, &error)),
            Err(_elapsed) => Err(TransportError::Unreachable {
                host: self.host.clone(),
                port: self.port,
            }),
        }
    }

    /// One budget-checked, transaction-matched, timeout-bounded,
    /// reconnect-with-backoff holding-register read (the async body the
    /// synchronous trait method drives). Never hangs: every await path is
    /// bounded by a declared timeout; never panics; never swallows an
    /// error.
    async fn read_async(&self, start: u16, count: u16) -> Result<Vec<u16>, TransportError> {
        // The declared gateway budget gates the read BEFORE any wire
        // traffic (the transport honours the existing budget type,
        // FIM-F03.02).
        if let Err(error) = self.config.polling.assert_read_fits(count) {
            let (requested, budget) = match error {
                crate::polling::BudgetError::ReadTooLarge { requested, budget } => {
                    (requested, budget)
                }
                // assert_read_fits produces only ReadTooLarge; the total
                // arm keeps the mapping total without a panic.
                _ => (count, self.config.polling.max_registers_per_read),
            };
            return Err(TransportError::ReadExceedsBudget { requested, budget });
        }
        let request = PduRequest::ReadHoldingRegisters { start, count };
        let backoff = std::time::Duration::from_millis(
            u64::try_from(self.config.polling.min_poll_interval_millis).unwrap_or(u64::MAX),
        );
        let mut last_error: Option<TransportError> = None;
        // Attempt 0 is the first try; 1..=reconnect_attempts are the
        // additional reconnects, each after the declared backoff floor.
        for attempt in 0..=self.config.reconnect_attempts {
            if attempt > 0 {
                tokio::time::sleep(backoff).await;
            }
            let transaction_id = self.next_transaction_id.fetch_add(1, Ordering::Relaxed);
            let request_frame = match encode_request(transaction_id, self.unit_id, &request) {
                Ok(frame) => frame,
                Err(error) => return Err(map_codec_error(error, &request)),
            };
            let mut guard = self.connection.lock().await;
            if guard.is_none() {
                match self.connect().await {
                    Ok(stream) => *guard = Some(stream),
                    Err(error) => {
                        last_error = Some(error);
                        continue;
                    }
                }
            }
            let stream = match guard.as_mut() {
                Some(stream) => stream,
                // Totality arm: the connection was ensured above; no panic
                // exists on this path — the named refusal keeps the loop
                // exhaustive.
                None => {
                    last_error = Some(TransportError::Unreachable {
                        host: self.host.clone(),
                        port: self.port,
                    });
                    continue;
                }
            };
            match exchange(
                stream,
                &request,
                &request_frame,
                transaction_id,
                self.unit_id,
                self.config.response_timeout_millis,
            )
            .await
            {
                Ok(response) => {
                    return match response {
                        PduResponse::ReadHoldingRegisters { registers } => Ok(registers),
                        // decode_response guarantees the answered shape;
                        // the defensive arm is a named refusal, not a
                        // panic or a guess.
                        _ => Err(TransportError::MalformedFrame {
                            defect: FrameDefect::UnexpectedPayload,
                        }),
                    };
                }
                // A named protocol refusal (exception, transaction-id
                // mismatch, unit-id mismatch, malformed frame): surfaced
                // immediately, never retried into a swallowed error.
                Err(ExchangeError::Codec(error)) => return Err(map_codec_error(error, &request)),
                Err(ExchangeError::TimedOut) => {
                    // The response may still be in flight; the connection
                    // is NOT trustworthy for the next request — drop it.
                    *guard = None;
                    last_error = Some(TransportError::ResponseTimeout {
                        timeout_millis: self.config.response_timeout_millis,
                    });
                }
                Err(ExchangeError::Io(error)) => {
                    *guard = None;
                    last_error = Some(map_io_error(&self.host, self.port, &error));
                }
            }
        }
        match last_error {
            Some(error) => Err(error),
            None => Err(TransportError::Unreachable {
                host: self.host.clone(),
                port: self.port,
            }),
        }
    }

    /// Graceful close: shuts the connection down (half/both per the
    /// tokio shutdown) and forgets it. Idempotent — closing an already-
    /// closed transport is `Ok`. Never called on a device that was never
    /// connected is also `Ok` (there is nothing to close).
    pub fn close(&self) -> Result<(), TransportError> {
        if tokio::runtime::Handle::try_current().is_ok() {
            return Err(TransportError::RuntimeContextRefused);
        }
        self.runtime.block_on(async {
            match self.connection.lock().await.take() {
                Some(stream) => match stream.shutdown().await {
                    Ok(()) => Ok(()),
                    Err(error) => Err(map_io_error(&self.host, self.port, &error)),
                },
                None => Ok(()),
            }
        })
    }
}

impl Drop for LiveTcpTransport {
    fn drop(&mut self) {
        // Best-effort graceful close on the way out. Inside a runtime
        // context (where block_on would panic) the fields simply drop and
        // the OS closes the socket — the honest bound of a drop path.
        if tokio::runtime::Handle::try_current().is_ok() {
            return;
        }
        let _ = self.close();
    }
}

impl ModbusTransport for LiveTcpTransport {
    fn read_holding_registers(
        &self,
        start_address: u16,
        count: u16,
    ) -> Result<Vec<u16>, TransportError> {
        // The no-panic guard: block_on inside a running runtime context
        // panics; this transport refuses instead (FIM-F03.04 — never a
        // panic). Drive the async path directly from runtime code.
        if tokio::runtime::Handle::try_current().is_ok() {
            return Err(TransportError::RuntimeContextRefused);
        }
        self.runtime.block_on(self.read_async(start_address, count))
    }
}

/// The outcome classes of one framed request/response exchange (the
/// transport's retry classification: `Codec` errors are named protocol
/// refusals surfaced immediately; `TimedOut` and `Io` are retryable with
/// a reconnect).
#[derive(Debug)]
pub(crate) enum ExchangeError {
    /// A socket I/O error mid-exchange (connection dropped or reset).
    Io(std::io::Error),
    /// The declared response timeout elapsed (bounded — no hang).
    TimedOut,
    /// The peer's answer violated the protocol (the named
    /// [`CodecError`] vocabulary).
    Codec(CodecError),
}

/// One framed request/response exchange over ANY byte stream (generic so
/// the deterministic tests drive it over in-memory tokio duplex pipes —
/// no socket). Bounds EVERY await by the declared response timeout.
///
/// Writes the encoded request frame, reads the 7-byte MBAP header,
/// bounds-checks it BEFORE allocating the body (no oversized read, no
/// hang), reads the declared-length body, and decodes the response
/// against the outstanding transaction id, unit id, and function code.
pub(crate) async fn exchange<S>(
    stream: &mut S,
    request: &PduRequest,
    request_frame: &[u8],
    expected_transaction_id: u16,
    expected_unit_id: u8,
    response_timeout_millis: u64,
) -> Result<PduResponse, ExchangeError>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let io = async {
        stream
            .write_all(request_frame)
            .await
            .map_err(ExchangeError::Io)?;
        // The header is bounds-checked BEFORE the body length is used for
        // an allocation (a bad length field never becomes an oversized
        // read).
        let mut header_bytes = [0u8; 7];
        stream
            .read_exact(&mut header_bytes)
            .await
            .map_err(ExchangeError::Io)?;
        let header = decode_mbap_header(&header_bytes).map_err(ExchangeError::Codec)?;
        let mut rest = vec![0u8; usize::from(header.length) - 1];
        stream
            .read_exact(&mut rest)
            .await
            .map_err(ExchangeError::Io)?;
        let mut frame = Vec::with_capacity(7 + rest.len());
        frame.extend_from_slice(&header_bytes);
        frame.extend_from_slice(&rest);
        let response = decode_response(
            &frame,
            expected_transaction_id,
            expected_unit_id,
            request.kind(),
            request.quantity(),
        )
        .map_err(ExchangeError::Codec)?;
        Ok::<PduResponse, ExchangeError>(response)
    };
    match tokio::time::timeout(
        std::time::Duration::from_millis(response_timeout_millis),
        io,
    )
    .await
    {
        Ok(outcome) => outcome,
        Err(_elapsed) => Err(ExchangeError::TimedOut),
    }
}

/// Maps a connect/mid-session I/O error onto the named refusal taxonomy
/// (FIM-F03.04): an active TCP refusal is `ConnectionRefused`; everything
/// else is `Unreachable`. ONE mapping, total.
pub(crate) fn map_io_error(host: &str, port: u16, error: &std::io::Error) -> TransportError {
    match error.kind() {
        std::io::ErrorKind::ConnectionRefused => TransportError::ConnectionRefused {
            host: host.to_string(),
            port,
        },
        _ => TransportError::Unreachable {
            host: host.to_string(),
            port,
        },
    }
}

/// The request's addressed window (start, count) — the context the
/// illegal-data-address mapping needs to surface the pre-existing
/// `WindowOutOfRange` refusal with the REQUEST's own coordinates.
fn request_window(request: &PduRequest) -> (u16, u16) {
    match request {
        PduRequest::ReadCoils { start, count } | PduRequest::ReadDiscreteInputs { start, count }
        | PduRequest::ReadHoldingRegisters { start, count }
        | PduRequest::ReadInputRegisters { start, count } => (*start, *count),
        PduRequest::WriteMultipleCoils { start, bits } => (*start, bits.len() as u16),
        PduRequest::WriteMultipleRegisters { start, values } => (*start, values.len() as u16),
        PduRequest::WriteSingleCoil { address, .. } | PduRequest::WriteSingleRegister { address } => {
            (*address, 1)
        }
    }
}

/// THE ONE total mapping from a codec error onto the transport refusal
/// taxonomy (FIM-F03.04). Every [`CodecError`] maps to exactly one named
/// [`TransportError`]; the illegal-data-address exception maps onto the
/// pre-existing `WindowOutOfRange` with the request's own coordinates
/// (that variant's doc comment reserves the mapping). The pinned test
/// `refusal_mapping_is_total_and_named` walks the whole taxonomy.
pub(crate) fn map_codec_error(error: CodecError, request: &PduRequest) -> TransportError {
    match error {
        CodecError::ExceptionResponse { function_code, exception } => match exception {
            crate::codec::ModbusExceptionCode::IllegalDataAddress => {
                let (start_address, count) = request_window(request);
                TransportError::WindowOutOfRange { start_address, count }
            }
            other => TransportError::DeviceException { function_code, exception: other },
        },
        CodecError::TransactionIdMismatch { expected, got } => {
            TransportError::TransactionIdMismatch { expected, got }
        }
        CodecError::UnitIdMismatch { expected, got } => {
            TransportError::UnitIdMismatch { expected, got }
        }
        CodecError::MalformedFrame { defect } => TransportError::MalformedFrame { defect },
        // A refused encode (impossible frame shape) or an answer from a
        // different function code is a malformed exchange, not a device
        // fault: it surfaces as the named frame defect it is.
        CodecError::FrameEmpty
        | CodecError::HeaderTooShort { .. }
        | CodecError::ProtocolIdMismatch { .. }
        | CodecError::UnexpectedFunctionCode { .. }
        | CodecError::InvalidCoilState { .. } => {
            TransportError::MalformedFrame { defect: FrameDefect::UnexpectedResponseShape }
        }
        // A count/overflow refusal carries the request's own window (the
        // pre-existing window refusal, reused for encode-time refusals).
        CodecError::CountOutOfRange { .. } | CodecError::AddressOverflow => {
            let (start_address, count) = request_window(request);
            TransportError::WindowOutOfRange { start_address, count }
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::codec::ModbusExceptionCode;

    /// A scripted device over an in-memory tokio duplex pipe: reads ONE
    /// request ADU (header + declared body), then writes the canned
    /// response bytes. NO SOCKET — the whole exchange is in-memory.
    async fn scripted_device(mut device: tokio::io::Duplex, response: Vec<u8>) {
        let mut header = [0u8; 7];
        device
            .read_exact(&mut header)
            .await
            .expect("device reads request header");
        let length = usize::from(u16::from_be_bytes([header[4], header[5]]));
        let mut rest = vec![0u8; length - 1];
        device
            .read_exact(&mut rest)
            .await
            .expect("device reads request body");
        device.write_all(&response).await.expect("device writes response");
    }

    /// A scripted device that reads the request and then deliberately
    /// never answers — the exchange must TIME OUT, never hang.
    async fn silent_device(mut device: tokio::io::Duplex) {
        let mut header = [0u8; 7];
        device
            .read_exact(&mut header)
            .await
            .expect("device reads request header");
        let length = usize::from(u16::from_be_bytes([header[4], header[5]]));
        let mut rest = vec![0u8; length - 1];
        device
            .read_exact(&mut rest)
            .await
            .expect("device reads request body");
        tokio::time::sleep(std::time::Duration::from_secs(60)).await;
    }

    /// Runs one exchange against a scripted device over in-memory pipes.
    async fn exchange_over_scripted_device(
        request: &PduRequest,
        response: Vec<u8>,
        response_timeout_millis: u64,
    ) -> Result<PduResponse, ExchangeError> {
        let (mut client, device) = tokio::io::duplex(128);
        tokio::spawn(scripted_device(device, response));
        let transaction_id: u16 = 0x1234;
        let frame = encode_request(transaction_id, 0x11, request).expect("encodes");
        exchange(
            &mut client,
            request,
            &frame,
            transaction_id,
            0x11,
            response_timeout_millis,
        )
        .await
    }

    fn read_two_registers() -> PduRequest {
        PduRequest::ReadHoldingRegisters { start: 0x006B, count: 2 }
    }

    // FIM-F03.05 — the framed exchange returns the pinned registers
    // (the codec's pinned response vector) over in-memory pipes.
    #[tokio::test]
    async fn exchange_returns_the_pinned_registers() {
        let response = vec![
            0x12, 0x34, 0x00, 0x00, 0x00, 0x07, 0x11, 0x03, 0x04, 0x01, 0xF4, 0x00, 0x64,
        ];
        let outcome = exchange_over_scripted_device(&read_two_registers(), response, 1_000).await;
        assert_eq!(
            outcome.expect("exchange succeeds"),
            PduResponse::ReadHoldingRegisters { registers: vec![500, 100] }
        );
    }

    // FIM-F03.02 — a response with a mismatched transaction id is refused
    // as the named error, never accepted.
    #[tokio::test]
    async fn exchange_refuses_a_mismatched_transaction_id() {
        let response = vec![
            0x12, 0x35, 0x00, 0x00, 0x00, 0x07, 0x11, 0x03, 0x04, 0x01, 0xF4, 0x00, 0x64,
        ];
        let outcome = exchange_over_scripted_device(&read_two_registers(), response, 1_000).await;
        assert!(matches!(
            outcome.expect_err("mismatched transaction id is refused"),
            ExchangeError::Codec(CodecError::TransactionIdMismatch { expected: 0x1234, got: 0x1235 })
        ));
    }

    // FIM-F03.04 — a wrong unit id is a named refusal.
    #[tokio::test]
    async fn exchange_refuses_a_wrong_unit_id() {
        let response = vec![
            0x12, 0x34, 0x00, 0x00, 0x00, 0x07, 0x12, 0x03, 0x04, 0x01, 0xF4, 0x00, 0x64,
        ];
        let outcome = exchange_over_scripted_device(&read_two_registers(), response, 1_000).await;
        assert!(matches!(
            outcome.expect_err("wrong unit id is refused"),
            ExchangeError::Codec(CodecError::UnitIdMismatch { expected: 0x11, got: 0x12 })
        ));
    }

    // FIM-F03.04 — the illegal-data-address exception surfaces as the
    // pre-existing WindowOutOfRange refusal with the REQUEST's own window
    // (the mapping that variant's doc comment reserves).
    #[tokio::test]
    async fn exception_surfaces_as_the_window_refusal_with_the_request_window() {
        let response = vec![0x12, 0x34, 0x00, 0x00, 0x00, 0x03, 0x11, 0x83, 0x02];
        let outcome = exchange_over_scripted_device(&read_two_registers(), response, 1_000).await;
        assert!(matches!(
            outcome.expect_err("exception response is refused"),
            ExchangeError::Codec(CodecError::ExceptionResponse {
                function_code: 0x83,
                exception: ModbusExceptionCode::IllegalDataAddress,
            })
        ));
        assert_eq!(
            map_codec_error(
                CodecError::ExceptionResponse {
                    function_code: 0x83,
                    exception: ModbusExceptionCode::IllegalDataAddress,
                },
                &read_two_registers(),
            ),
            TransportError::WindowOutOfRange { start_address: 0x006B, count: 2 }
        );
    }

    // FIM-F03.05 — a malformed length field is refused BEFORE the body is
    // read or allocated (the device writes only the 7-byte header).
    #[tokio::test]
    async fn malformed_length_is_refused_before_the_body_is_read() {
        let response = vec![0x12, 0x34, 0x00, 0x00, 0x00, 0x00, 0x11];
        let outcome = exchange_over_scripted_device(&read_two_registers(), response, 1_000).await;
        assert!(matches!(
            outcome.expect_err("zero length field is refused"),
            ExchangeError::Codec(CodecError::MalformedFrame {
                defect: FrameDefect::LengthFieldTooSmall
            })
        ));
    }

    // FIM-F03.04 — a silent device TIMES OUT (bounded), never hangs.
    #[tokio::test]
    async fn a_silent_device_times_out_bounded_never_hangs() {
        let (mut client, device) = tokio::io::duplex(128);
        tokio::spawn(silent_device(device));
        let transaction_id: u16 = 0x1234;
        let frame = encode_request(transaction_id, 0x11, &read_two_registers()).expect("encodes");
        let started = std::time::Instant::now();
        let outcome = exchange(
            &mut client,
            &read_two_registers(),
            &frame,
            transaction_id,
            0x11,
            100,
        )
        .await;
        // Bounded: the exchange returns in the order of the declared
        // timeout, not the silent device's 60-second sleep.
        assert!(started.elapsed() < std::time::Duration::from_secs(5));
        assert!(matches!(
            outcome.expect_err("a silent device is a named timeout"),
            ExchangeError::TimedOut
        ));
    }

    // FIM-F03.04 — the I/O mapping is total and named: an active refusal
    // is ConnectionRefused, everything else is Unreachable.
    #[test]
    fn io_error_mapping_is_total_and_named() {
        let refused = std::io::Error::from(std::io::ErrorKind::ConnectionRefused);
        assert_eq!(
            map_io_error("gw.example", 502, &refused),
            TransportError::ConnectionRefused { host: "gw.example".to_string(), port: 502 }
        );
        let unreachable = std::io::Error::from(std::io::ErrorKind::ConnectionReset);
        assert_eq!(
            map_io_error("gw.example", 502, &unreachable),
            TransportError::Unreachable { host: "gw.example".to_string(), port: 502 }
        );
        let other = std::io::Error::from(std::io::ErrorKind::TimedOut);
        assert!(matches!(
            map_io_error("gw.example", 502, &other),
            TransportError::Unreachable { .. }
        ));
    }

    // FIM-F03.05 — the refusal taxonomy is COMPLETE and NAMED: every
    // codec error maps to exactly one named transport refusal, and every
    // Modbus exception code (including unknown ones) stays named.
    #[test]
    fn refusal_mapping_is_total_and_named() {
        let request = read_two_registers();
        let cases = [
            (
                CodecError::FrameEmpty,
                TransportError::MalformedFrame { defect: FrameDefect::UnexpectedResponseShape },
            ),
            (
                CodecError::HeaderTooShort { got: 3 },
                TransportError::MalformedFrame { defect: FrameDefect::UnexpectedResponseShape },
            ),
            (
                CodecError::ProtocolIdMismatch { expected: 0, got: 5 },
                TransportError::MalformedFrame { defect: FrameDefect::UnexpectedResponseShape },
            ),
            (
                CodecError::TransactionIdMismatch { expected: 1, got: 2 },
                TransportError::TransactionIdMismatch { expected: 1, got: 2 },
            ),
            (
                CodecError::UnitIdMismatch { expected: 0x11, got: 0x12 },
                TransportError::UnitIdMismatch { expected: 0x11, got: 0x12 },
            ),
            (
                CodecError::UnexpectedFunctionCode { expected: 0x03, got: 0x04 },
                TransportError::MalformedFrame { defect: FrameDefect::UnexpectedResponseShape },
            ),
            (
                CodecError::MalformedFrame { defect: FrameDefect::LengthFieldTooLarge },
                TransportError::MalformedFrame { defect: FrameDefect::LengthFieldTooLarge },
            ),
            (
                CodecError::CountOutOfRange { count: 126, limit: 125 },
                TransportError::WindowOutOfRange { start_address: 0x006B, count: 2 },
            ),
            (
                CodecError::AddressOverflow,
                TransportError::WindowOutOfRange { start_address: 0x006B, count: 2 },
            ),
            (
                CodecError::InvalidCoilState { got: 0x1234 },
                TransportError::MalformedFrame { defect: FrameDefect::UnexpectedResponseShape },
            ),
        ];
        for (error, expected) in cases {
            assert_eq!(map_codec_error(error, &request), expected);
        }
        // Every known exception code maps, with exactly ONE code (the
        // illegal-data-address) surfacing as WindowOutOfRange.
        for byte in ModbusExceptionCode::KNOWN_BYTES {
            let exception = ModbusExceptionCode::from_byte(byte);
            let mapped = map_codec_error(
                CodecError::ExceptionResponse { function_code: 0x83, exception },
                &request,
            );
            if matches!(exception, ModbusExceptionCode::IllegalDataAddress) {
                assert_eq!(
                    mapped,
                    TransportError::WindowOutOfRange { start_address: 0x006B, count: 2 }
                );
            } else {
                assert!(matches!(mapped, TransportError::DeviceException { .. }));
                assert!(!mapped.to_string().is_empty());
            }
        }
        // An unknown exception code is still a NAMED device exception.
        assert!(matches!(
            map_codec_error(
                CodecError::ExceptionResponse {
                    function_code: 0x83,
                    exception: ModbusExceptionCode::Unknown(0x77),
                },
                &request,
            ),
            TransportError::DeviceException { .. }
        ));
    }

    // FIM-F03.02 — the declared configuration is fail-closed: zero
    // timeouts are undeclared, and the RTU variant is refused at
    // transport construction (the RTU seam stays unwired — SeamNotWired).
    #[test]
    fn config_validation_is_fail_closed() {
        let tcp = TransportConfig::Tcp {
            host: "gw.example".to_string(),
            port: 502,
            unit_id: 1,
        };
        let budget = GatewayPollingBudget::new(125, 1, 1_000).expect("valid budget");
        assert_eq!(
            LiveTcpConfig::new(tcp.clone(), 0, 1_000, 0, budget),
            Err(TransportError::EmptyField("connect_timeout_millis"))
        );
        assert_eq!(
            LiveTcpConfig::new(tcp.clone(), 1_000, 0, 0, budget),
            Err(TransportError::EmptyField("response_timeout_millis"))
        );
        LiveTcpConfig::new(tcp.clone(), 1_000, 1_000, 2, budget).expect("valid config");
        // An RTU variant passes config validation (it is a valid crate
        // config) but is refused at TRANSPORT construction — honestly.
        let rtu = TransportConfig::Rtu {
            port_name: "/dev/ttyUSB0".to_string(),
            baud: 9_600,
            parity: crate::transport::SerialParity::Even,
            unit_id: 3,
        };
        let rtu_config = LiveTcpConfig::new(rtu, 1_000, 1_000, 0, budget).expect("valid config");
        assert_eq!(
            LiveTcpTransport::new(rtu_config),
            Err(TransportError::SeamNotWired)
        );
    }

    // FIM-F03.02 — an over-budget read is refused BEFORE any wire
    // traffic (the host is never contacted; no socket is opened).
    #[test]
    fn an_over_budget_read_is_refused_before_any_wire_traffic() {
        let config = LiveTcpConfig::new(
            TransportConfig::Tcp {
                host: "gw.example".to_string(),
                port: 502,
                unit_id: 1,
            },
            1_000,
            1_000,
            0,
            GatewayPollingBudget::new(10, 1, 1_000).expect("valid budget"),
        )
        .expect("valid config");
        let transport = LiveTcpTransport::new(config).expect("valid transport");
        assert_eq!(
            transport.read_holding_registers(0, 11),
            Err(TransportError::ReadExceedsBudget { requested: 11, budget: 10 })
        );
        transport.close().expect("close is clean without a connection");
    }

    // The no-panic guard: the synchronous trait method refuses (never
    // panics) when called inside a running tokio runtime context.
    #[tokio::test]
    async fn the_sync_trait_method_refuses_inside_a_runtime_context() {
        let config = LiveTcpConfig::new(
            TransportConfig::Tcp {
                host: "gw.example".to_string(),
                port: 502,
                unit_id: 1,
            },
            1_000,
            1_000,
            0,
            GatewayPollingBudget::new(125, 1, 1_000).expect("valid budget"),
        )
        .expect("valid config");
        let transport = LiveTcpTransport::new(config).expect("valid transport");
        assert_eq!(
            transport.read_holding_registers(0, 1),
            Err(TransportError::RuntimeContextRefused)
        );
    }
}
