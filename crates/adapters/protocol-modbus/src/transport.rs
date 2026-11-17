// P14.07 — The Modbus transport seam (TCP/RTU), designed not implemented.
//
// The wire transport is a DESIGNED SEAM (phase plan P14.07; conventions:
// no protocol-stack library dependencies — the model layers are real code,
// the transports are never connected in this program). This module defines:
//
// - the transport CONFIGURATION types (TCP and RTU endpoints, declared per
//   device), and
// - the port trait a later transport implementation would satisfy
//   ([`ModbusTransport`]), plus
// - a named not-wired error so nothing here can pretend a device was
//   reached.
//
// No socket, no serial port, no Modbus framing, no CRC computation. When a
// transport lands (a later wave, behind a human lab gate), it implements
// the trait; the registry/decoder/units boundary above it is unchanged.
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
    /// path produces, and it names itself honestly.
    SeamNotWired,
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
