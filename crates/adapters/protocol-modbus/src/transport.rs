// P14.07 — The Modbus transport seam (TCP/RTU), designed not implemented.
//
// The wire transport is a DESIGNED SEAM (phase plan P14.07; conventions:
// no protocol-stack library dependencies — the model layers are real code;
// the transports WERE unwired seams at design time — see the FIM-F03 note
// below for what now exists). This module defines:
//
// - the transport CONFIGURATION types (TCP and RTU endpoints, declared per
//   device), and
// - the port trait a later transport implementation would satisfy
//   ([`ModbusTransport`]), plus
// - a named not-wired error so nothing here can pretend a device was
//   reached.
//
// No socket, no serial port, no Modbus framing, no CRC computation IN THIS
// MODULE. FIM-F03 (live Modbus TCP client,
// docs/releases/implementation_ledger/ledger_F03.md) added, ADDITIVELY:
// the framing codec ([`crate::codec`]) and the live TCP transport
// ([`crate::tcp_transport`]) that implements the trait below — WRITTEN-
// UNRUN; device interop is UNPROVEN and remains a HUMAN LAB GATE. This
// module stays the seam: the trait, the configuration types, and the
// [`UnwiredTransport`] refusal are unchanged; the RTU serial path REMAINS
// unwired (selecting `TransportConfig::Rtu` with the live TCP transport
// is the named `SeamNotWired` refusal, honestly stating that no RTU
// implementation exists).
//
// FIM-F03.04 — the refusal taxonomy was extended ADDITIVELY (every
// pre-existing variant and its meaning is preserved): unreachable device,
// connection refused, response timeout, Modbus exception code, malformed
// frame (bad TCP length — there is no CRC in TCP framing), unit-id
// mismatch, transaction-id mismatch, a read exceeding the declared
// polling budget, and a block_on refused inside an async context (the
// no-panic guard of the sync trait over the tokio runtime). The ONE total
// mapping from codec errors onto this taxonomy lives in
// [`crate::tcp_transport`].
//
// UNVERIFIED anchors: the Modbus TCP default port 502 and the RTU frame
// parameters (baud, parity, silent interval) are secondary-source readings
// and are configuration DEFAULTS nowhere in this crate — every transport
// parameter is declared per deployment, never assumed.

use serde::{Deserialize, Serialize};

/// Transport configuration for one device path: Modbus TCP or Modbus RTU.
/// Every parameter is declared; there are no defaults at this layer (even
/// the TCP port is explicit, so a config file cannot silently rely on the
/// well-known port).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "transport", rename_all = "snake_case")]
pub enum TransportConfig {
    /// Modbus TCP: host, explicit port, and the unit id behind the
    /// gateway (UNVERIFIED: unit-id semantics for TCP gateways routing to
    /// RTU slaves).
    Tcp {
        /// Gateway host or IP (non-empty).
        host: String,
        /// Explicit TCP port (the well-known 502 is a configuration
        /// choice, not a default — UNVERIFIED anchor).
        port: u16,
        /// Unit id of the target device behind the gateway.
        unit_id: u8,
    },
    /// Modbus RTU over a serial line. Serial parameters are declared per
    /// deployment (UNVERIFIED: RTU framing parameters per the protocol
    /// text).
    Rtu {
        /// Serial port name (non-empty).
        port_name: String,
        /// Declared baud rate.
        baud: u32,
        /// Declared parity (None/Odd/Even).
        parity: SerialParity,
        /// Unit id (slave address).
        unit_id: u8,
    },
}

impl TransportConfig {
    /// Validates the configuration (non-empty strings, unit id in the
    /// 1..=247 Modbus address range — UNVERIFIED range anchor).
    pub fn validate(&self) -> Result<(), TransportError> {
        let (unit_id, named) = match self {
            TransportConfig::Tcp { host, port, unit_id } => {
                if host.trim().is_empty() {
                    return Err(TransportError::EmptyField("tcp.host"));
                }
                if *port == 0 {
                    return Err(TransportError::EmptyField("tcp.port"));
                }
                (*unit_id, "tcp.unit_id")
            }
            TransportConfig::Rtu { port_name, baud, parity: _, unit_id } => {
                if port_name.trim().is_empty() {
                    return Err(TransportError::EmptyField("rtu.port_name"));
                }
                if *baud == 0 {
                    return Err(TransportError::EmptyField("rtu.baud"));
                }
                (*unit_id, "rtu.unit_id")
            }
        };
        if unit_id == 0 || unit_id > 247 {
            return Err(TransportError::UnitIdOutOfRange { value: unit_id, field: named });
        }
        Ok(())
    }
}

/// Serial parity (declared, never assumed).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SerialParity {
    None,
    Odd,
    Even,
}

/// Errors returned by the transport seam.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportError {
    /// A configuration field is empty or zero.
    EmptyField(&'static str),
    /// A unit id is outside the Modbus address range (UNVERIFIED anchor).
    UnitIdOutOfRange { value: u8, field: &'static str },
    /// The requested register window is outside the addressable block
    /// the transport's peer declared (P14.17: the simulated device serves
    /// a DECLARED block; an out-of-window read is a named refusal, never
    /// a truncation). A real transport maps its peer's exception response
    /// onto this variant at implementation time.
    WindowOutOfRange { start_address: u16, count: u16 },
    /// The transport seam is not wired: no transport implementation
    /// exists in this phase. This is the ONLY error the seam's default
    /// path produces, and it names itself honestly. (FIM-F03: the TCP
    /// path is now implemented in [`crate::tcp_transport`]; the RTU
    /// serial path remains unwired — selecting an RTU config with the
    /// live TCP transport produces THIS refusal.)
    SeamNotWired,
    // FIM-F03.04 — the named live-transport refusals, added ADDITIVELY:
    // every refusal is a named variant, never a hang, never a panic,
    // never a swallowed error.
    /// The device was unreachable within the declared connect timeout or
    /// the connection failed with an I/O error other than an active
    /// refusal (route failure, DNS failure, reset during connect). The
    /// connect-timeout case is THIS refusal: an unreachable device never
    /// hangs past its declared timeout.
    Unreachable { host: String, port: u16 },
    /// The device actively refused the TCP connection (ECONNREFUSED —
    /// nothing is listening on the declared port).
    ConnectionRefused { host: String, port: u16 },
    /// The device did not answer within the declared response timeout
    /// (or the connection dropped while waiting). Never a hang: every
    /// await path is bounded by the caller's declared timeout.
    ResponseTimeout { timeout_millis: u64 },
    /// The device answered with a Modbus exception response
    /// (function code | 0x80 + the named exception code from
    /// [`crate::codec::ModbusExceptionCode`]). NOTE: the illegal-data-
    /// address exception (0x02) maps onto the pre-existing
    /// [`TransportError::WindowOutOfRange`] instead — that variant's doc
    /// comment reserves the mapping ("a real transport maps its peer's
    /// exception response onto this variant at implementation time").
    DeviceException { function_code: u8, exception: crate::codec::ModbusExceptionCode },
    /// The frame the peer produced violated the Modbus TCP framing rules
    /// (bad length field, short body, trailing bytes, protocol-id
    /// mismatch, byte-count desync) — the closed
    /// [`crate::codec::FrameDefect`] vocabulary carries the reason.
    MalformedFrame { defect: crate::codec::FrameDefect },
    /// The response carried a unit id other than the configured one.
    UnitIdMismatch { expected: u8, got: u8 },
    /// The response carried a transaction id other than the outstanding
    /// request's (FIM-F03.02: a mismatched response is refused, never
    /// accepted).
    TransactionIdMismatch { expected: u16, got: u16 },
    /// A planned read exceeded the declared gateway polling budget
    /// (FIM-F03.02: the live transport honours
    /// [`crate::polling::GatewayPollingBudget`] — an over-budget read is
    /// refused BEFORE any wire traffic).
    ReadExceedsBudget { requested: u16, budget: u16 },
    /// A synchronous transport call (`block_on`) was attempted inside a
    /// running tokio runtime context, which would panic. The transport
    /// refuses instead (the no-panic guard): call it from a plain thread
    /// or drive the async path directly.
    RuntimeContextRefused,
    /// The transport's own tokio runtime could not be constructed at
    /// transport construction time (resource exhaustion at the host —
    /// a named construction refusal, never a panic).
    RuntimeUnavailable,
}

impl std::fmt::Display for TransportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransportError::EmptyField(field) => {
                write!(f, "transport config field '{field}' is empty or zero")
            }
            TransportError::UnitIdOutOfRange { value, field } => {
                write!(f, "unit id {value} in '{field}' is outside the Modbus address range 1..=247 (UNVERIFIED anchor)")
            }
            TransportError::WindowOutOfRange { start_address, count } => {
                write!(
                    f,
                    "register window start={start_address} count={count} is outside the addressable block the peer declared"
                )
            }
            TransportError::SeamNotWired => {
                write!(
                    f, "the Modbus transport seam is not wired: no protocol connection exists in this \
                        program (no device is reached, no message is exchanged)"
                )
            }
            // FIM-F03.04 — the named live-transport refusals.
            TransportError::Unreachable { host, port } => {
                write!(
                    f, "the Modbus TCP device at {host}:{port} was unreachable within the declared connect timeout or the connection failed"
                )
            }
            TransportError::ConnectionRefused { host, port } => {
                write!(f, "the Modbus TCP device at {host}:{port} actively refused the connection")
            }
            TransportError::ResponseTimeout { timeout_millis } => {
                write!(f, "the device did not answer within the declared response timeout of {timeout_millis} ms")
            }
            TransportError::DeviceException { function_code, exception } => {
                write!(
                    f, "the device answered function code {function_code:#04x} with a Modbus exception: {}",
                    exception.name()
                )
            }
            TransportError::MalformedFrame { defect } => {
                write!(f, "the peer produced a malformed Modbus TCP frame: {defect:?}")
            }
            TransportError::UnitIdMismatch { expected, got } => {
                write!(f, "response unit id {got} does not match the configured unit id {expected}")
            }
            TransportError::TransactionIdMismatch { expected, got } => {
                write!(
                    f, "response transaction id {got} does not match the outstanding request {expected}; refusing"
                )
            }
            TransportError::ReadExceedsBudget { requested, budget } => {
                write!(f, "requested read of {requested} registers exceeds the declared polling budget of {budget}")
            }
            TransportError::RuntimeContextRefused => {
                write!(
                    f, "the synchronous transport call cannot block inside a running tokio runtime context; call it from a plain thread"
                )
            }
            TransportError::RuntimeUnavailable => {
                write!(f, "the transport's tokio runtime could not be constructed at transport construction time")
            }
        }
    }
}

impl std::error::Error for TransportError {}

/// The Modbus transport port: the surface a later TCP/RTU implementation
/// satisfies. Deliberately minimal and synchronous (the flexcore-ports
/// house rule: a concrete adapter owns its own I/O shape; the port stays
/// runtime-free).
///
/// No method here writes to a device: the transport surface is READ-ONLY
/// (phase 14 context §3 constraint 1 — adapters are read/monitor +
/// command-relay; absence of a write path is structural).
///
/// FIM-F03 (written-unrun): the trait is UNCHANGED. The live TCP
/// implementation ([`crate::tcp_transport::LiveTcpTransport`]) answers
/// reads with function code 0x03 (read holding registers) at the
/// configured unit id — the function-code/addressing choice recorded
/// below; it remains an UNVERIFIED anchor until the human lab gate.
/// Implementations must never hang, panic, or swallow an error: every
/// refusal is a named [`TransportError`].
pub trait ModbusTransport {
    /// Reads `count` holding registers starting at `start_address` from
    /// the configured device path. UNVERIFIED: the function-code choice
    /// (0x03 read holding registers) and register-address base (0- vs
    /// 4xxxx-style addressing) are implementation-time checks against the
    /// protocol text and a lab device.
    fn read_holding_registers(
        &self,
        start_address: u16,
        count: u16,
    ) -> Result<Vec<u16>, TransportError>;
}

/// A transport that is deliberately NOT wired. Every deployment starts
/// with this: it validates configuration (so config errors surface before
/// any wire exists) and answers every read with the named
/// [`TransportError::SeamNotWired`] refusal. It exists so a composition
/// root can hold a transport-shaped object without pretending to be
/// connected — and so a test that EXPECTS connection failure can pin the
/// refusal.
#[derive(Debug, Clone)]
pub struct UnwiredTransport {
    /// The validated configuration the future transport will use.
    pub config: TransportConfig,
}

impl UnwiredTransport {
    /// Validates and constructs the unwired transport.
    pub fn new(config: TransportConfig) -> Result<Self, TransportError> {
        config.validate()?;
        Ok(Self { config })
    }
}

impl ModbusTransport for UnwiredTransport {
    fn read_holding_registers(
        &self,
        _start_address: u16,
        _count: u16,
    ) -> Result<Vec<u16>, TransportError> {
        Err(TransportError::SeamNotWired)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tcp_config_validates_and_refuses_defaults() {
        TransportConfig::Tcp {
            host: "gw.example".to_string(),
            port: 502,
            unit_id: 1,
        }
        .validate()
        .expect("valid tcp config");
        assert_eq!(
            TransportConfig::Tcp {
                host: " ".to_string(),
                port: 502,
                unit_id: 1,
            }
            .validate(),
            Err(TransportError::EmptyField("tcp.host"))
        );
        assert_eq!(
            TransportConfig::Tcp {
                host: "gw.example".to_string(),
                port: 502,
                unit_id: 0,
            }
            .validate(),
            Err(TransportError::UnitIdOutOfRange {
                value: 0,
                field: "tcp.unit_id"
            })
        );
        assert_eq!(
            TransportConfig::Tcp {
                host: "gw.example".to_string(),
                port: 502,
                unit_id: 248,
            }
            .validate(),
            Err(TransportError::UnitIdOutOfRange {
                value: 248,
                field: "tcp.unit_id"
            })
        );
    }

    #[test]
    fn rtu_config_validates() {
        TransportConfig::Rtu {
            port_name: "/dev/ttyUSB0".to_string(),
            baud: 9_600,
            parity: SerialParity::Even,
            unit_id: 3,
        }
        .validate()
        .expect("valid rtu config");
        assert_eq!(
            TransportConfig::Rtu {
                port_name: String::new(),
                baud: 9_600,
                parity: SerialParity::None,
                unit_id: 3,
            }
            .validate(),
            Err(TransportError::EmptyField("rtu.port_name"))
        );
    }

    #[test]
    fn unwired_transport_refuses_every_read_with_the_named_seam_error() {
        let transport = UnwiredTransport::new(TransportConfig::Tcp {
            host: "gw.example".to_string(),
            port: 502,
            unit_id: 1,
        })
        .expect("valid config");
        // No device is reached; the refusal names the seam honestly.
        assert_eq!(
            transport.read_holding_registers(40_001, 2),
            Err(TransportError::SeamNotWired)
        );
    }
}
