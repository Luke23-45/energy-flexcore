// FIM-F03.05 — the SOCKET-LEVEL gated suite for the live Modbus TCP
// client (docs/releases/implementation_ledger/ledger_F03.md; the F-03
// acceptance criteria in docs/releases/final-implementation-before-
// publication.md §5).
//
// **WRITTEN-UNRUN.** These tests have never been compiled or executed —
// the no-build/no-test bans (program conventions §1) make execution a
// HUMAN GATE. Nothing here is a conformance result: conformance against a
// real Modbus TCP / SunSpec device is a HUMAN LAB GATE (interop UNPROVEN;
// crate conventions); this file only pins the live behaviours the
// in-module tests pin at the logic level over in-memory pipes
// (codec.rs, tcp_transport.rs, sunspec.rs).
//
// GATING (the recorded repo convention, as in the NATS adapter's
// `tests/integration.rs` and the 2030.5 adapter's
// `tests/live_transport_gated.rs`): every test in this file is gated on
// `FLEXCORE_TEST_MODBUS_HOST` and SKIPS CLEANLY (prints a note and
// returns) when the variable is unset or empty. A mis-set gate (a test
// needing a variable the operator did not set) FAILS LOUDLY instead —
// declared, never defaulted, exactly like the crate's configuration
// discipline.
//
// The gate variables (the human lab supplies ALL of them; there are NO
// defaults — the well-known port is a declared choice, not an assumed
// one):
//
//   FLEXCORE_TEST_MODBUS_HOST          the gateway/device host or IP
//                                      (the gate: unset/empty = skip)
//   FLEXCORE_TEST_MODBUS_PORT          the declared TCP port
//   FLEXCORE_TEST_MODBUS_UNIT_ID       the declared unit id (1..=247)
//   FLEXCORE_TEST_MODBUS_BASE_ADDRESS  the declared protocol address of
//                                      the SunSpec common block (the
//                                      4xxxx-style base is the lab's
//                                      configuration translation — never
//                                      assumed here)
//   FLEXCORE_TEST_MODBUS_BLOCK_ADDRESS the declared protocol address of
//                                      the registered family's model block
//                                      (the common block's first-model
//                                      pointer is UNVERIFIED — the lab
//                                      declares it, the reader refuses to
//                                      infer it)
//   FLEXCORE_TEST_MODBUS_UNREACHABLE_HOST
//                                      a host that must NOT answer (the
//                                      refusal test; e.g. a TEST-NET-3
//                                      address such as 203.0.113.1)
//
// The register map the full SunSpec read runs against is the IN-FILE
// fixture below (`lab_map()`): the lab EDITS it to match the device —
// the registry is reviewed data (record 23 §1), per-device maps are
// lab-verified before registration. A map/device mismatch at the gate is
// a recorded edit to THIS fixture (or the device's map), never a silent
// reinterpretation.

use flexcore_modbus_adapter::model::{RegisterType, RegisterUnitKind, SunSpecModel};
use flexcore_modbus_adapter::polling::GatewayPollingBudget;
use flexcore_modbus_adapter::registry::{DeviceModelKey, RegisterField, RegisterMap, RegisterMapRegistry};
use flexcore_modbus_adapter::sunspec::{SunSpecDeviceReader, SunSpecReadError};
use flexcore_modbus_adapter::tcp_transport::LiveTcpConfig;
use flexcore_modbus_adapter::transport::{
    ModbusTransport, SerialParity, TransportConfig, TransportError, UnwiredTransport,
};
use flexcore_modbus_adapter::units_map::ConversionError;

/// The parsed gate: `None` means "skip cleanly" (the NATS-adapter
/// convention). A partially-set gate fails loudly.
struct Gate {
    host: String,
    port: u16,
    unit_id: u8,
}

fn declared_u16(name: &str) -> u16 {
    match std::env::var(name) {
        Ok(value) if !value.trim().is_empty() => value.trim().parse().unwrap_or_else(|_| {
            panic!("{name} is set but is not a u16: {value:?}");
        }),
        _ => panic!("{name} must be declared when FLEXCORE_TEST_MODBUS_HOST is set"),
    }
}

fn declared_u8(name: &str) -> u8 {
    match std::env::var(name) {
        Ok(value) if !value.trim().is_empty() => value.trim().parse().unwrap_or_else(|_| {
            panic!("{name} is set but is not a u8: {value:?}");
        }),
        _ => panic!("{name} must be declared when FLEXCORE_TEST_MODBUS_HOST is set"),
    }
}

/// The gate: `None` means "skip cleanly".
fn gate() -> Option<Gate> {
    match std::env::var("FLEXCORE_TEST_MODBUS_HOST") {
        Ok(host) if !host.trim().is_empty() => Some(Gate {
            host: host.trim().to_string(),
            port: declared_u16("FLEXCORE_TEST_MODBUS_PORT"),
            unit_id: declared_u8("FLEXCORE_TEST_MODBUS_UNIT_ID"),
        }),
        _ => {
            eprintln!(
                "skipping: FLEXCORE_TEST_MODBUS_HOST is not set (prepare the device and export \
                 FLEXCORE_TEST_MODBUS_HOST/_PORT/_UNIT_ID/_BASE_ADDRESS/_BLOCK_ADDRESS as \
                 documented in this file's header)"
            );
            None
        }
    }
}

/// The declared unreachable-device gate: `None` means "skip cleanly".
fn unreachable_gate() -> Option<(String, u16)> {
    match std::env::var("FLEXCORE_TEST_MODBUS_UNREACHABLE_HOST") {
        Ok(host) if !host.trim().is_empty() => {
            Some((host.trim().to_string(), declared_u16("FLEXCORE_TEST_MODBUS_PORT")))
        }
        _ => {
            eprintln!(
                "skipping: FLEXCORE_TEST_MODBUS_UNREACHABLE_HOST is not set (export it together \
                 with FLEXCORE_TEST_MODBUS_PORT to run the refusal test)"
            );
            None
        }
    }
}

fn tcp_config(host: &str, port: u16, unit_id: u8) -> TransportConfig {
    TransportConfig::Tcp { host: host.to_string(), port, unit_id }
}

/// The declared live transport configuration (the lab gate's declared
/// timeouts; a 1-second connect timeout and a 2-second response timeout
/// are the LAB's declared choices, edited per deployment — no defaults).
fn live_config(config: TransportConfig) -> LiveTcpConfig {
    LiveTcpConfig::new(
        config,
        1_000,
        2_000,
        1,
        GatewayPollingBudget::new(125, 1, 1_000).expect("valid budget"),
    )
    .expect("valid live config")
}

/// The in-file lab fixture map (the lab edits this to match the device).
/// Three-phase meter, family 203: the model-id register at window index
/// 0, then W / VAr sharing one SF register, then WH as a big-endian u32
/// pair — the same shapes the deterministic tests pin.
fn lab_map() -> RegisterMap {
    let fields = vec![
        RegisterField::new(
            "W",
            SunSpecModel::MeterThreePhase,
            1,
            RegisterType::I16,
            Some(3),
            RegisterUnitKind::Watts,
        )
        .expect("valid field"),
        RegisterField::new(
            "VAr",
            SunSpecModel::MeterThreePhase,
            2,
            RegisterType::I16,
            Some(3),
            RegisterUnitKind::Var,
        )
        .expect("valid field"),
        RegisterField::new(
            "W_SF",
            SunSpecModel::MeterThreePhase,
            3,
            RegisterType::I16,
            None,
            RegisterUnitKind::Enumerated,
        )
        .expect("valid field"),
        RegisterField::new(
            "WH",
            SunSpecModel::MeterThreePhase,
            4,
            RegisterType::U32,
            None,
            RegisterUnitKind::WattHours,
        )
        .expect("valid field"),
    ];
    RegisterMap::new(
        DeviceModelKey::new("lab/fim-gate-device").expect("valid key"),
        SunSpecModel::MeterThreePhase,
        fields,
        6,
    )
    .expect("valid lab map")
}

fn declared_block_addresses() -> (u16, u16) {
    (
        declared_u16("FLEXCORE_TEST_MODBUS_BASE_ADDRESS"),
        declared_u16("FLEXCORE_TEST_MODBUS_BLOCK_ADDRESS"),
    )
}

// F-03 acceptance criterion 2 — an unreachable device surfaces a named
// refusal, never a hang. The whole read is bounded by the declared
// connect timeout plus the declared backoff and response timeout; the
// assertion below also bounds the wall clock so a broken gate FAILS
// instead of hanging the gate runner.
#[test]
fn an_unreachable_device_surfaces_a_named_refusal_never_a_hang() {
    let Some((host, port)) = unreachable_gate() else { return };
    let config = live_config(tcp_config(&host, port, 1));
    let transport =
        flexcore_modbus_adapter::tcp_transport::LiveTcpTransport::new(config).expect("valid");
    let started = std::time::Instant::now();
    let outcome = transport.read_holding_registers(0, 2);
    // Bounded: connect timeout (1 s) + one backoff (1 s) + connect again —
    // far below the 30-second wall-clock guard.
    assert!(started.elapsed() < std::time::Duration::from_secs(30));
    match outcome {
        Err(TransportError::Unreachable { .. }) | Err(TransportError::ConnectionRefused { .. }) => {
            // The named refusal the acceptance criterion requires.
        }
        Err(other) => panic!("expected Unreachable/ConnectionRefused, got: {other}"),
        Ok(registers) => {
            panic!("the declared-unreachable device ANSWERED ({registers:?}) — the gate is mis-declared");
        }
    }
    transport.close().expect("graceful close");
}

// F-03 acceptance criterion 1 — a Modbus TCP device answers a SunSpec
// model read through the live client, end to end: marker → model block →
// registry decode → canonical units → rows carrying GOOD/MEASURED.
#[test]
fn a_live_device_answers_a_sunspec_model_read_through_the_live_client() {
    let Some(gate) = gate() else { return };
    let (base_address, block_address) = declared_block_addresses();
    let mut registry = RegisterMapRegistry::new(flexcore_modbus_adapter::ADAPTER_ID);
    registry.register(lab_map()).expect("valid lab map");
    let transport =
        flexcore_modbus_adapter::tcp_transport::LiveTcpTransport::new(live_config(tcp_config(
            &gate.host,
            gate.port,
            gate.unit_id,
        )))
        .expect("valid transport");
    let reader = SunSpecDeviceReader::new(
        &transport,
        base_address,
        &GatewayPollingBudget::new(125, 1, 1_000).expect("valid budget"),
    );
    // The whole read is bounded by the declared response timeout per wire
    // read; the wall-clock guard keeps a broken gate from hanging.
    let started = std::time::Instant::now();
    let outcome = reader.read_device(
        &registry,
        &DeviceModelKey::new("lab/fim-gate-device").expect("valid key"),
        block_address,
        flexcore_contracts::units::ReactiveSignConvention::InjectionPositive,
    );
    assert!(started.elapsed() < std::time::Duration::from_secs(30));
    let rows = match outcome {
        Ok(rows) => rows,
        // A non-SunSpec device at the gate is a LOUD gate failure with the
        // named refusal — the lab decides (fix the fixture map or the
        // device), never a silent reinterpretation.
        Err(SunSpecReadError::NotSunSpecShaped { got_marker }) => {
            panic!(
                "the gate device did not present the SunSpec marker (got {got_marker:#010x}); \
                 the lab map/device pair must be reviewed at the gate"
            );
        }
        Err(SunSpecReadError::Registry(error)) => {
            panic!("the lab fixture map refused: {error}");
        }
        Err(SunSpecReadError::Conversion(error)) => {
            panic!("a fixture row refused canonical conversion: {error}");
        }
        Err(error) => panic!("the live SunSpec read failed: {error}"),
    };
    assert_eq!(rows.len(), 4);
    for row in &rows {
        assert_eq!(row.quality, flexcore_contracts::quality::TelemetryQuality::Good);
        assert_eq!(row.provenance, flexcore_contracts::capability::Provenance::Measured);
    }
    transport.close().expect("graceful close");
}

// A WRONG unit id behind the gateway must surface SOME named refusal —
// promptly, never a hang. The specific variant is device-dependent
// (UnitIdMismatch from an echoing gateway, or a DeviceException /
// WindowOutOfRange from a gateway forwarding the request to an absent
// slave), so the assertion is: a named error within the declared bounds,
// never silence.
#[test]
fn a_wrong_unit_id_surfaces_a_named_refusal_never_a_hang() {
    let Some(gate) = gate() else { return };
    let wrong_unit_id = if gate.unit_id == 247 { 246 } else { gate.unit_id + 1 };
    let config = live_config(tcp_config(&gate.host, gate.port, wrong_unit_id));
    let transport =
        flexcore_modbus_adapter::tcp_transport::LiveTcpTransport::new(config).expect("valid");
    let started = std::time::Instant::now();
    let outcome = transport.read_holding_registers(0, 2);
    assert!(started.elapsed() < std::time::Duration::from_secs(30));
    assert!(
        outcome.is_err(),
        "a wrong unit id was ANSWERED with registers — the device echoes every unit id; \
         record that behaviour in the lab notes"
    );
    let error = outcome.expect_err("named refusal");
    assert!(!error.to_string().is_empty(), "the refusal is spellable");
    transport.close().expect("graceful close");
}

// The unwired-transport refusal still holds at the gate (an RTU config
// against the live TCP transport is honestly refused — the RTU seam
// stays unwired; FIM-F03 did not touch it).
#[test]
fn the_rtu_seam_stays_unwired_at_the_gate() {
    let Some(gate) = gate() else { return };
    let rtu = TransportConfig::Rtu {
        port_name: "/dev/ttyUSB0".to_string(),
        baud: 9_600,
        parity: SerialParity::Even,
        unit_id: gate.unit_id,
    };
    // The unwired transport still refuses every read with the named seam
    // error — unchanged by FIM-F03.
    let unwired = UnwiredTransport::new(rtu).expect("valid config");
    assert_eq!(
        unwired.read_holding_registers(0, 2),
        Err(TransportError::SeamNotWired)
    );
    // And the live TCP transport refuses an RTU config at construction.
    let config = live_config(TransportConfig::Rtu {
        port_name: "/dev/ttyUSB0".to_string(),
        baud: 9_600,
        parity: SerialParity::Even,
        unit_id: gate.unit_id,
    });
    assert_eq!(
        flexcore_modbus_adapter::tcp_transport::LiveTcpTransport::new(config),
        Err(TransportError::SeamNotWired)
    );
}

// The canonical-unit boundary holds at the gate: whatever the device
// answered, a Watts row converts to MW (never W), a Wh row to MWh (never
// Wh), and a refused conversion refuses the read (no raw-value rows
// exist). This pins the frozen units rule at the live boundary.
#[test]
fn canonical_units_hold_at_the_live_boundary() {
    let Some(gate) = gate() else { return };
    let (base_address, block_address) = declared_block_addresses();
    let mut registry = RegisterMapRegistry::new(flexcore_modbus_adapter::ADAPTER_ID);
    registry.register(lab_map()).expect("valid lab map");
    let transport =
        flexcore_modbus_adapter::tcp_transport::LiveTcpTransport::new(live_config(tcp_config(
            &gate.host,
            gate.port,
            gate.unit_id,
        )))
        .expect("valid transport");
    let reader = SunSpecDeviceReader::new(
        &transport,
        base_address,
        &GatewayPollingBudget::new(125, 1, 1_000).expect("valid budget"),
    );
    let rows = match reader.read_device(
        &registry,
        &DeviceModelKey::new("lab/fim-gate-device").expect("valid key"),
        block_address,
        flexcore_contracts::units::ReactiveSignConvention::InjectionPositive,
    ) {
        Ok(rows) => rows,
        Err(SunSpecReadError::Conversion(ConversionError::Units(_))) => {
            // The device answered an out-of-range SOC/percent — the whole
            // read refused (fail-closed), which is exactly the pinned
            // behaviour.
            return;
        }
        Err(error) => panic!("the live SunSpec read failed: {error}"),
    };
    for row in &rows {
        match &row.canonical {
            flexcore_modbus_adapter::sunspec::CanonicalReading::ActivePower(mw) => {
                assert!(
                    mw.value().abs() < 100.0,
                    "a Watts→MW row must be in MW (got {} — a raw W value would be ~1e6 larger)",
                    mw.value()
                );
            }
            flexcore_modbus_adapter::sunspec::CanonicalReading::Energy(mwh) => {
                assert!(
                    mwh.value().abs() < 100.0,
                    "a Wh→MWh row must be in MWh (got {} — a raw Wh value would be ~1e6 larger)",
                    mwh.value()
                );
            }
            _ => {}
        }
    }
    transport.close().expect("graceful close");
}
