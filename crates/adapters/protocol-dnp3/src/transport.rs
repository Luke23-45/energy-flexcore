// P14.08 — The DNP3 transport seam, designed not implemented.
//
// The wire transport (serial or TCP, link layer, application layer) is a
// DESIGNED SEAM (phase plan P14.08; conventions: no protocol-stack
// library dependencies). This module defines the port surface a later
// implementation would satisfy and a named not-wired refusal. No socket,
// no serial port, no CRC, no link-layer framing.
//
// The outstation-direction surface is READ/REPORT-shaped: the adapter
// hands the transport a static snapshot and a drained event queue; the
// transport (never written in this phase) would answer the master's
// polls. There is deliberately no "send control" method — controls
// ARRIVE (mapped at PROPOSED in `crate::outstation`); nothing here sends
// one.

use serde::{Deserialize, Serialize};

use crate::outstation::QueuedEvent;

/// The declared link configuration of a deployment path. Every parameter
/// is declared; there are no defaults at this layer. UNVERIFIED anchors:
/// the DNP3 default TCP port 20000 and the serial link parameters follow
/// the secondary-source reading [S-14][S-15] and are configuration
/// choices here, never assumptions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "link", rename_all = "snake_case")]
pub enum LinkConfig {
    /// DNP3 over TCP (declared host and port; the well-known 20000 is a
    /// configuration choice, not a default — UNVERIFIED anchor).
    Tcp {
        /// Host or IP of the utility-side termination point (non-empty).
        host: String,
        /// Declared TCP port.
        port: u16,
        /// Declared DNP3 source address (this outstation's link address).
        /// UNVERIFIED: link-address semantics per the standard text.
        outstation_address: u16,
        /// Declared DNP3 destination address (the master's link address).
        master_address: u16,
    },
    /// DNP3 over a serial line (parameters declared per deployment).
    Serial {
        /// Serial port name (non-empty).
        port_name: String,
        /// Declared baud rate.
        baud: u32,
        /// Declared DNP3 outstation link address.
        outstation_address: u16,
        /// Declared DNP3 master link address.
        master_address: u16,
    },
}

impl LinkConfig {
    /// Validates the link configuration.
    pub fn validate(&self) -> Result<(), TransportError> {
        match self {
            LinkConfig::Tcp { host, port, .. } => {
                if host.trim().is_empty() {
                    return Err(TransportError::EmptyField("tcp.host"));
                }
                if *port == 0 {
                    return Err(TransportError::EmptyField("tcp.port"));
                }
            }
            LinkConfig::Serial { port_name, baud, .. } => {
                if port_name.trim().is_empty() {
                    return Err(TransportError::EmptyField("serial.port_name"));
                }
                if *baud == 0 {
                    return Err(TransportError::EmptyField("serial.baud"));
                }
            }
        }
        Ok(())
    }
}

/// Errors returned by the transport seam.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportError {
    /// A configuration field is empty or zero.
    EmptyField(&'static str),
    /// The transport seam is not wired: no DNP3 stack exists in this
    /// phase. The refusal names itself honestly.
    SeamNotWired,
}

impl std::fmt::Display for TransportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransportError::EmptyField(field) => {
                write!(f, "link config field '{field}' is empty or zero")
            }
            TransportError::SeamNotWired => {
                write!(
                    f, "the DNP3 transport seam is not wired: no protocol connection exists in \
                        this program (no device is reached, no message is exchanged)"
                )
            }
        }
    }
}

impl std::error::Error for TransportError {}

/// The data the outstation-direction transport would REPORT on a poll: a
/// static snapshot (point index → source-scaled value) plus the drained
/// event queue. A pure record — the outstation model builds it; the
/// transport seam would carry it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct OutstationReport {
    /// The static (class-0-style) point values, deterministic by index.
    pub static_points: Vec<(u16, f64)>,
    /// The drained event queue, in queue order.
    pub events: Vec<QueuedEvent>,
}

/// The outstation-direction transport port. Deliberately minimal and
/// synchronous (the flexcore-ports house rule): an implementation owns
/// its own I/O shape and bridges to this surface.
pub trait Dnp3Transport {
    /// Serves the given report to the master side (answering polls /
    /// unsolicited reporting — UNVERIFIED semantics). No implementation
    /// exists in this phase: every call returns
    /// [`TransportError::SeamNotWired`].
    fn serve_report(&self, report: OutstationReport) -> Result<(), TransportError>;
}

/// A transport that is deliberately NOT wired: validates the link config
/// (so config errors surface before any wire exists) and refuses every
/// report with the named seam error.
#[derive(Debug, Clone)]
pub struct UnwiredTransport {
    /// The validated link configuration the future transport will use.
    pub config: LinkConfig,
}

impl UnwiredTransport {
    /// Validates and constructs the unwired transport.
    pub fn new(config: LinkConfig) -> Result<Self, TransportError> {
        config.validate()?;
        Ok(Self { config })
    }
}

impl Dnp3Transport for UnwiredTransport {
    fn serve_report(&self, _report: OutstationReport) -> Result<(), TransportError> {
        Err(TransportError::SeamNotWired)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_configs_validate() {
        LinkConfig::Tcp {
            host: "scada.example".to_string(),
            port: 20_000,
            outstation_address: 1,
            master_address: 2,
        }
        .validate()
        .expect("valid tcp link");
        assert_eq!(
            LinkConfig::Tcp {
                host: " ".to_string(),
                port: 20_000,
                outstation_address: 1,
                master_address: 2,
            }
            .validate(),
            Err(TransportError::EmptyField("tcp.host"))
        );
        assert_eq!(
            LinkConfig::Serial {
                port_name: String::new(),
                baud: 9_600,
                outstation_address: 1,
                master_address: 2,
            }
            .validate(),
            Err(TransportError::EmptyField("serial.port_name"))
        );
    }

    #[test]
    fn unwired_transport_refuses_every_report_with_the_named_seam_error() {
        let transport = UnwiredTransport::new(LinkConfig::Tcp {
            host: "scada.example".to_string(),
            port: 20_000,
            outstation_address: 1,
            master_address: 2,
        })
        .expect("valid config");
        assert_eq!(
            transport.serve_report(OutstationReport::default()),
            Err(TransportError::SeamNotWired)
        );
    }
}
