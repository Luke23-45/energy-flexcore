//! P14.07 — SunSpec Modbus client: conversion tests (WRITTEN-UNRUN).
//!
//! These tests are COMPLETE but NEVER EXECUTED (conventions §1 ban 3).
//! They verify the end-to-end read path of record 23 with synthetic
//! register windows — the same class as the phase's simulator-first
//! policy (P14.17 lands the simulated device; this test file pins the
//! boundary contract in the meantime):
//!
//! register window → decode (registry + scale factor) → adapter-boundary
//! unit conversion (canonical MW / MVAr / MWh / SOC) → poll-plan budgets.

use flexcore_contracts::units::{ReactivePowerDirection, ReactiveSignConvention};
use flexcore_modbus_adapter::model::{RegisterType, RegisterUnitKind, SunSpecModel};
use flexcore_modbus_adapter::polling::{GatewayPollingBudget, BudgetError};
use flexcore_modbus_adapter::registry::{
    decode_register_window, DeviceModelKey, RegisterField, RegisterMap, RegisterMapRegistry,
    RegistryError,
};
use flexcore_modbus_adapter::scope::{client_manifest, client_scope};
use flexcore_protocol_ports::scope::{ControlScope, NotModelled, ProtocolRole, ResourceClass};
use flexcore_modbus_adapter::transport::{TransportConfig, UnwiredTransport};
use flexcore_modbus_adapter::units_map;

fn three_phase_meter_map() -> RegisterMap {
    RegisterMap::new(
        DeviceModelKey::new("lab/meter-3ph-1").expect("valid key"),
        SunSpecModel::MeterThreePhase,
        vec![
            RegisterField::new(
                "W",
                SunSpecModel::MeterThreePhase,
                0,
                RegisterType::I16,
                Some(2),
                RegisterUnitKind::Watts,
            )
            .expect("valid field"),
            RegisterField::new(
                "VAr",
                SunSpecModel::MeterThreePhase,
                1,
                RegisterType::I16,
                Some(2),
                RegisterUnitKind::Var,
            )
            .expect("valid field"),
            RegisterField::new(
                "W_SF",
                SunSpecModel::MeterThreePhase,
                2,
                RegisterType::I16,
                None,
                RegisterUnitKind::Enumerated,
            )
            .expect("valid field"),
            RegisterField::new(
                "WH",
                SunSpecModel::MeterThreePhase,
                3,
                RegisterType::U32,
                None,
                RegisterUnitKind::WattHours,
            )
            .expect("valid field"),
        ],
        8,
    )
    .expect("valid map")
}

/// Test 1: a synthetic meter window decodes and converts to canonical
/// units: W (with SF) → MW, VAr → MVAr under a DECLARED convention,
/// Wh → MWh.
#[test]
fn meter_window_decodes_and_converts_to_canonical_units() {
    let map = three_phase_meter_map();
    // W = 25000 with SF = -1 → 2500.0 W; VAr = 1000 with SF = -1 → 100.0 VAr;
    // WH = 0x000186A0 = 100000 Wh.
    let window: [u16; 8] = [25_000, 1_000, 0xFFFF, 0x0001, 0x86A0, 0, 0, 0];
    let w_field = map.field("W").expect("mapped").clone();
    let var_field = map.field("VAr").expect("mapped").clone();
    let wh_field = map.field("WH").expect("mapped").clone();

    let decoded_w = decode_register_window(&map, &w_field, &window).expect("decodes W");
    assert_eq!(decoded_w.value, 2_500.0);
    let mw = units_map::watts_to_active_power(&decoded_w).expect("converts");
    assert_eq!(mw.value(), 0.0025);

    let decoded_var = decode_register_window(&map, &var_field, &window).expect("decodes VAr");
    let (mvar, direction) = units_map::var_to_reactive_power(
        &decoded_var,
        // The convention is a REQUIRED argument: a device profile
        // declares it, the adapter never assumes it.
        ReactiveSignConvention::InjectionPositive,
    )
    .expect("converts");
    assert_eq!(mvar.value(), 0.0001);
    assert_eq!(direction, ReactivePowerDirection::Injection);

    let decoded_wh = decode_register_window(&map, &wh_field, &window).expect("decodes WH");
    let mwh = units_map::watt_hours_to_energy(&decoded_wh).expect("converts");
    assert_eq!(mwh.value(), 0.1);
}

/// Test 2: the same numeric VAr sign maps to OPPOSITE directions under
/// the two declared conventions — the sign is never a universal constant.
#[test]
fn reactive_sign_is_convention_dependent_never_assumed() {
    let map = three_phase_meter_map();
    let window: [u16; 8] = [0, 5_000, 0xFFFF, 0, 0, 0, 0, 0];
    let var_field = map.field("VAr").expect("mapped").clone();
    let decoded = decode_register_window(&map, &var_field, &window).expect("decodes");
    let (_, injection_direction) = units_map::var_to_reactive_power(
        &decoded,
        ReactiveSignConvention::InjectionPositive,
    )
    .expect("converts");
    let (_, consumption_direction) = units_map::var_to_reactive_power(
        &decoded,
        ReactiveSignConvention::ConsumptionPositive,
    )
    .expect("converts");
    assert_eq!(injection_direction, ReactivePowerDirection::Injection);
    assert_eq!(consumption_direction, ReactivePowerDirection::Consumption);
}

/// Test 3: a battery model's percent SOC converts through the named
/// `from_percent` boundary constructor, and out-of-range percentages are
/// refused at the boundary.
#[test]
fn battery_soc_converts_through_the_named_boundary_constructor() {
    let map = RegisterMap::new(
        DeviceModelKey::new("lab/bess-1").expect("valid key"),
        SunSpecModel::BatteryStatus,
        vec![RegisterField::new(
            "SOC",
            SunSpecModel::BatteryStatus,
            0,
            RegisterType::U16,
            None,
            RegisterUnitKind::Percent,
        )
        .expect("valid field")],
        2,
    )
    .expect("valid map");
    let soc_field = map.field("SOC").expect("mapped").clone();
    let decoded = decode_register_window(&map, &soc_field, &[73]).expect("decodes");
    let soc = units_map::percent_to_soc(&decoded).expect("converts");
    assert_eq!(soc.value(), 0.73);
    let bad = decode_register_window(&map, &soc_field, &[101]).expect("decodes");
    assert!(units_map::percent_to_soc(&bad).is_err(), "101% is refused");
}

/// Test 4: the common scope gate is fail-closed — telemetry reads are in
/// scope; command-relay and control mappings are refused (the write
/// direction is structural absence), and an empty `not_modelled` list is
/// a construction error.
#[test]
fn scope_gate_is_fail_closed() {
    let scope = client_scope("modbus-client").expect("valid scope");
    scope
        .assert_supports(ProtocolRole::Client, false, &[ResourceClass::Telemetry])
        .expect("telemetry reads are in scope");
    assert!(scope
        .assert_supports(ProtocolRole::Client, true, &[ResourceClass::Telemetry])
        .is_err(), "command relay is not declared (monitor-only)");
    assert!(scope
        .assert_supports(ProtocolRole::Client, false, &[ResourceClass::Control])
        .is_err(), "control mappings are outside the declared classes");
    // An empty not_modelled list is a construction error in the COMMON
    // scope constructor.
    assert_eq!(
        flexcore_protocol_ports::scope::ProtocolScopeDeclaration::new(
            "modbus-client".to_string(),
            "sunspec-modbus".to_string(),
            vec![ProtocolRole::Client],
            ControlScope::MonitorOnly,
            vec![ResourceClass::Telemetry],
            Vec::new(),
            "readings enter the telemetry path only; this adapter is NOT a dispatch authority"
                .to_string(),
            Vec::new(),
        ),
        Err(flexcore_protocol_ports::scope::ProtocolScopeError::EmptyNotModelled)
    );
}

/// Test 4b: the ONE common manifest shape constructs for this adapter
/// with the structurally relay-only authority declaration; the
/// never-granted permissions are not expressible.
#[test]
fn common_manifest_constructs_relay_free() {
    let manifest = client_manifest("modbus-client").expect("valid manifest");
    assert_eq!(
        manifest.authority,
        flexcore_protocol_ports::manifest::AuthorityDeclaration::SupervisoryRelayOnly
    );
    assert!(manifest.declares(flexcore_protocol_ports::manifest::DeclaredCapability::Read));
    assert!(!manifest
        .declares(flexcore_protocol_ports::manifest::DeclaredCapability::CommandRelay));
}

/// Test 5: polling budgets refuse oversized reads, too-fast intervals,
/// and device-count exceedances; the unwired transport answers every read
/// with the named seam refusal.
#[test]
fn budgets_and_unwired_transport_are_fail_closed() {
    let budget = GatewayPollingBudget::new(125, 8, 1_000).expect("valid budget");
    budget.assert_read_fits(125).expect("at the limit");
    assert_eq!(
        budget.assert_read_fits(126),
        Err(BudgetError::ReadTooLarge { requested: 126, budget: 125 })
    );
    assert_eq!(
        budget.assert_scan_fits(8, 999),
        Err(BudgetError::IntervalTooFast {
            requested_millis: 999,
            minimum_millis: 1_000
        })
    );
    let transport = UnwiredTransport::new(TransportConfig::Tcp {
        host: "gw.example".to_string(),
        port: 502,
        unit_id: 1,
    })
    .expect("valid config");
    assert!(matches!(
        flexcore_modbus_adapter::transport::ModbusTransport::read_holding_registers(
            &transport, 0, 8
        ),
        Err(flexcore_modbus_adapter::transport::TransportError::SeamNotWired)
    ));
}

/// Test 6: an unregistered device model is a named refusal — never a
/// guessed map.
#[test]
fn unregistered_device_models_are_refused() {
    let registry = RegisterMapRegistry::new("modbus-client");
    assert_eq!(
        registry.map_for(&DeviceModelKey::new("unknown/device").expect("valid key")),
        Err(RegistryError::DeviceModelNotRegistered {
            key: "unknown/device".to_string()
        })
    );
}
