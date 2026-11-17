// P14.17 — The simulated SunSpec Modbus device (decision record 81; the
// seam is `flexcore_modbus_adapter::transport::ModbusTransport` — the
// UNWIRED TCP/RTU seam, record 23 §4).
//
// The DEVICE side of Modbus: it serves holding registers from a DECLARED
// block (the fixture's register table) to whoever polls it through the
// [`ModbusTransport`] trait — which is exactly the surface the real
// client adapter consumes. The register block the fixture serves is
// hand-declared data (values + a declared block start address); decoded
// through the adapter's own [`RegisterMapRegistry`] /
// `decode_register_window` path, the fixture exercises the SAME
// register→decoded-value pipeline a lab device would, with no serial
// line, no TCP socket, no Modbus framing, and no CRC.
//
// Deterministic: the block is static per construction (a fixture that
// needs value change over time declares multiple blocks and selects one
// explicitly — there is no hidden tick state). Honest: the served values
// are SYNTHETIC (the Phase 2 profile rule applies verbatim) — no decoded
// value from this fixture is evidence about real device behaviour, and
// every identity string carries the `sim-modbus` prefix.
//
// Read-only: the [`ModbusTransport`] trait has no write method (record
// 23 §4: absence is structural), and the simulated device serves reads
// and nothing else.

use flexcore_modbus_adapter::transport::{ModbusTransport, TransportConfig, TransportError};

/// The identity prefix every simulated Modbus device carries in its
/// identity strings (never presentable as a real device).
pub const SIM_MODBUS_ID_PREFIX: &str = "sim-modbus";

/// Errors the simulated Modbus device reports. Every failure is named;
/// the wire-shaped refusals re-use the adapter's own transport error
/// vocabulary so the simulated device's refusals are indistinguishable
/// in KIND from what a real peer would report (a composition test can
/// treat them identically).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SimModbusError {
    /// The transport configuration was refused (the adapter's own
    /// validation, re-raised).
    Config(TransportError),
    /// The declared register block is empty — a device serving no
    /// registers represents nothing (construction error).
    EmptyBlock,
}

impl std::fmt::Display for SimModbusError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SimModbusError::Config(error) => write!(f, "the simulated device config was refused: {error}"),
            SimModbusError::EmptyBlock => {
                write!(f, "the simulated Modbus device declares an empty register block")
            }
        }
    }
}

impl std::error::Error for SimModbusError {}

/// The simulated SunSpec Modbus device: a validated transport
/// configuration plus the DECLARED register block it serves.
#[derive(Debug, Clone)]
pub struct SimulatedModbusDevice {
    /// The device's identity string (carries the `sim-modbus` prefix).
    pub device_id: String,
    /// The validated transport configuration (the adapter-side config
    /// the real client would declare for this device path).
    pub config: TransportConfig,
    /// The first address of the declared register block.
    pub block_start_address: u16,
    /// The declared register block (synthetic values; read-only).
    block: Vec<u16>,
}

impl SimulatedModbusDevice {
    /// Constructs the simulated device over a validated transport
    /// config and a declared register block. The config is validated
    /// (fail-fast) exactly as the adapter's `UnwiredTransport::new`
    /// validates it — a broken composition surfaces at construction,
    /// never at poll time.
    pub fn new(
        device_id: impl Into<String>,
        config: TransportConfig,
        block_start_address: u16,
        block: Vec<u16>,
    ) -> Result<Self, SimModbusError> {
        config.validate()?;
        if block.is_empty() {
            return Err(SimModbusError::EmptyBlock);
        }
        Ok(Self {
            device_id: format!("{SIM_MODBUS_ID_PREFIX}:{}", device_id.into()),
            config,
            block_start_address,
            block,
        })
    }

    /// The declared block (read-only snapshot).
    pub fn block(&self) -> &[u16] {
        &self.block
    }
}

impl ModbusTransport for SimulatedModbusDevice {
    /// Serves `count` registers from `start_address` out of the declared
    /// block. A window that starts before the block, or overruns its
    /// end, is the named [`TransportError::WindowOutOfRange`] refusal —
    /// never a truncation and never a fabricated zero-fill (the same
    /// fail-closed rule the decode path applies to short windows).
    fn read_holding_registers(
        &self,
        start_address: u16,
        count: u16,
    ) -> Result<Vec<u16>, TransportError> {
        if count == 0 {
            return Err(TransportError::WindowOutOfRange { start_address, count });
        }
        let block_start = u32::from(self.block_start_address);
        let requested_start = u32::from(start_address);
        let requested_end = requested_start + u32::from(count);
        let block_end = block_start + self.block.len() as u32;
        if requested_start < block_start || requested_end > block_end {
            return Err(TransportError::WindowOutOfRange { start_address, count });
        }
        let offset = (requested_start - block_start) as usize;
        Ok(self.block[offset..offset + count as usize].to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flexcore_modbus_adapter::model::{RegisterType, RegisterUnitKind, SunSpecModel};
    use flexcore_modbus_adapter::registry::{
        decode_register_window, DeviceModelKey, RegisterField, RegisterMap, RegisterMapRegistry,
    };

    /// The simulated inverter block: raw W at offset 0 (i16), raw WH
    /// pair at offsets 1..3 (u32 big-endian), an SF register at offset 3.
    fn device() -> SimulatedModbusDevice {
        SimulatedModbusDevice::new(
            "inverter-1",
            TransportConfig::Tcp {
                host: "sim-gateway.invalid".to_string(),
                port: 502,
                unit_id: 1,
            },
            40_001,
            vec![
                0xFA1C, // i16 -1500 (importing 1500 W)
                0x0001, 0x86A0, // u32 100000 Wh
                0x0000, // SF register slot
            ],
        )
        .expect("valid simulated device")
    }

    #[test]
    fn the_device_serves_declared_windows_from_its_block() {
        let device = device();
        // Exact window at the block start.
        assert_eq!(
            device.read_holding_registers(40_001, 1).expect("serves"),
            vec![0xFA1C]
        );
        // A two-register pair within the block.
        assert_eq!(
            device.read_holding_registers(40_002, 2).expect("serves"),
            vec![0x0001, 0x86A0]
        );
    }

    #[test]
    fn out_of_block_windows_are_named_refusals_never_truncations() {
        let device = device();
        // Before the block.
        assert_eq!(
            device.read_holding_registers(40_000, 1),
            Err(TransportError::WindowOutOfRange { start_address: 40_000, count: 1 })
        );
        // Overrunning the block end.
        assert_eq!(
            device.read_holding_registers(40_002, 3),
            Err(TransportError::WindowOutOfRange { start_address: 40_002, count: 3 })
        );
        // Zero-count read refused too.
        assert_eq!(
            device.read_holding_registers(40_001, 0),
            Err(TransportError::WindowOutOfRange { start_address: 40_001, count: 0 })
        );
    }

    #[test]
    fn the_served_block_decodes_through_the_adapters_own_pipeline() {
        // The same register map the adapter's tests use, registered for
        // the simulated device model; the fixture's block decodes to the
        // SAME decoded values the hand-written window in the adapter
        // tests produces — the simulated device feeds the REAL decode
        // path, not a parallel one.
        let mut registry = RegisterMapRegistry::new("modbus-client");
        let map = RegisterMap::new(
            DeviceModelKey::new("sim-modbus/inverter-1").expect("valid key"),
            SunSpecModel::InverterThreePhase,
            vec![
                RegisterField::new(
                    "W",
                    SunSpecModel::InverterThreePhase,
                    0,
                    RegisterType::I16,
                    None,
                    RegisterUnitKind::Watts,
                )
                .expect("valid field"),
                RegisterField::new(
                    "WH",
                    SunSpecModel::InverterThreePhase,
                    1,
                    RegisterType::U32,
                    None,
                    RegisterUnitKind::WattHours,
                )
                .expect("valid field"),
            ],
            4,
        )
        .expect("valid map");
        registry.register(map).expect("registered");
        let device = device();
        let registered = registry
            .map_for(&DeviceModelKey::new("sim-modbus/inverter-1").expect("valid key"))
            .expect("map present");
        let window = device
            .read_holding_registers(40_001, 4)
            .expect("the block serves its full window");
        let w = registered.field("W").expect("W mapped").clone();
        let decoded = decode_register_window(registered, &w, &window).expect("decodes");
        assert_eq!(decoded.value, -1500.0, "the simulated block decodes like a real window");
        let wh = registered.field("WH").expect("WH mapped").clone();
        let decoded = decode_register_window(registered, &wh, &window).expect("decodes");
        assert_eq!(decoded.value, 100_000.0);
    }

    #[test]
    fn degenerate_constructions_are_refused() {
        // An invalid transport config (the adapter's own validation).
        assert!(matches!(
            SimulatedModbusDevice::new(
                "bad",
                TransportConfig::Tcp {
                    host: " ".to_string(),
                    port: 502,
                    unit_id: 1,
                },
                40_001,
                vec![1],
            ),
            Err(SimModbusError::Config(TransportError::EmptyField("tcp.host")))
        ));
        // An empty register block.
        assert_eq!(
            SimulatedModbusDevice::new(
                "empty",
                TransportConfig::Tcp {
                    host: "gw".to_string(),
                    port: 502,
                    unit_id: 1,
                },
                40_001,
                Vec::new(),
            ),
            Err(SimModbusError::EmptyBlock)
        );
    }
}
