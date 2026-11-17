# P14.07 — SunSpec Modbus client adapter: information models, register-map registry, polling budgets, unit conversion, transport seam

| Field | Value |
|---|---|
| Subtask | P14.07 (SunSpec Modbus client over TCP/RTU — information models 101/103, 201/203, 80x; gateway polling budgets; register-map registry keyed per device model; unit conversion at the adapter) |
| Wave / agent | Phase 14 wave 1, Agent B |
| Status | WRITTEN-UNRUN: model types, registry, budgets, conversion, and the transport SEAM — written, never compiled or executed; no device was reached, no register was read (conventions §1; first human build is the program gate) |
| Sources | [S-16] (SunSpec Modbus, sunspec.org/modbus — the program page; the model specifications themselves were NOT consulted, so every register-level detail below is UNVERIFIED); the contracts unit rule (`crates/flexcore-contracts/src/units.rs`); the P14.01 common port layer (Agent A, `crates/flexcore-protocol-ports`) |
| Files touched | `crates/adapters/protocol-modbus/` (NEW: `Cargo.toml`, `src/lib.rs`, `src/model.rs`, `src/registry.rs`, `src/units_map.rs`, `src/polling.rs`, `src/transport.rs`, `src/scope.rs`, `tests/register_conversion.rs`); additive root `Cargo.toml` entries; additive `tools/check_dependency_rule.py` entries |

## 1. What landed (deliverable class: design-and-model)

| Module | Content |
|---|---|
| `model.rs` | `SunSpecModel` families (101/103 inverters, 201/203 meters, 801/802/804/805 battery models) with the numeric ids pinned in ONE place; `SUNSPEC_MARKER` (0x53756E53 "SunS" — UNVERIFIED); `RegisterType` (U16/I16/U32/I32/Bitfield16 with register counts); `RegisterUnitKind` (Watts/Var/WattHours/…/Percent/Enumerated); `DecodedField` (source-terms ACL input); `apply_scale_factor` (value × 10^SF — UNVERIFIED rule) with a fail-closed plausibility band [-10, +10] |
| `registry.rs` | `RegisterMapRegistry`: BTreeMap of reviewed per-device-model `RegisterMap`s (`DeviceModelKey` → fields with offsets, types, SF-register offsets, unit kinds). No runtime discovery and no guessed maps: an unregistered device model is a named refusal; map construction refuses overlapping fields, out-of-range fields, and SF registers overlapping their field. `decode_register_window` applies the map to a raw window (i16 two's complement, big-endian u32/i32 pairs — UNVERIFIED) and refuses short windows and missing SFs |
| `units_map.rs` | The adapter-boundary conversion: W → `ActivePowerMW` (/1e6, frozen P > 0 = export sign passes through); VAr → `ReactivePowerMVAr` **requiring the declared `ReactiveSignConvention`** (no convention-free overload; direction only via `interpret_reactive_power`); Wh → `EnergyMWh`; percent → `StateOfCharge` through the named `from_percent` boundary constructor. Unit-kind mismatches are refused, never coerced |
| `polling.rs` | `GatewayPollingBudget` (declared, never defaulted): max registers per read (≤ 125 — the Modbus read limit, UNVERIFIED), devices per gateway, minimum poll interval; `assert_read_fits` / `assert_scan_fits` fail-closed; `reads_needed_for` block-splitting arithmetic |
| `transport.rs` | The TCP/RTU SEAM: `TransportConfig` (Tcp{host, port, unit_id} / Rtu{port_name, baud, parity, unit_id}) validated with no defaults (even the well-known ports are declared, never assumed); the sync `ModbusTransport` port trait (read holding registers — READ-ONLY, no write path, absence structural); `UnwiredTransport` answering every read with the named `SeamNotWired` refusal |
| `scope.rs` | CONSUMES the common port layer (P14.01): `client_scope()` (role CLIENT, control scope MONITOR-ONLY, resource class TELEMETRY only; the write direction is structural absence) and `client_manifest()` (the one common manifest shape, `SupervisoryRelayOnly`) |
| `tests/register_conversion.rs` | WRITTEN-UNRUN conversion tests: synthetic meter window → decode (SF applied) → canonical MW/MVAr/MWh; reactive sign is convention-dependent (same numeric sign maps to opposite directions under the two declared conventions); battery SOC percent → fraction with out-of-range refusal; fail-closed scope gate; budgets; unwired-transport refusal; unregistered-model refusal |

## 2. Dependence on Agent A's P14.01 layer (recorded)

Same reconciliation as record 21 §2: the crate was first written against a
crate-local grid-api-mirroring scope, then migrated onto the landed
`flexcore-protocol-ports` common scope/manifest in this batch. The common
CLIENT role is the Modbus reading direction; MONITOR-ONLY encodes the
read-only rule structurally (the common manifest cannot express command
relay in this scope, and the transport trait has no write method).

## 3. The reactive-sign rule (why the converter looks the way it does)

SunSpec meter blocks' power-direction conventions differ per model and
deployment wiring (UNVERIFIED per [S-16]; exactly the situation the frozen
contracts rule anticipates). The adapter therefore does the only honest
thing `crates/flexcore-contracts/src/units.rs` permits: it REQUIRES the
device profile's declared `ReactiveSignConvention` as an argument, computes
direction only through `interpret_reactive_power`, and never reads the sign
alone. A test pins that the same numeric VAr value maps to Injection under
one declared convention and Consumption under the other.

## 4. UNVERIFIED register (implementation-time checks against [S-16] and a lab device)

| # | Detail | Where |
|---|---|---|
| 1 | Model ids (101/103/201/203/801/802/804/805) and the family semantics | `model.rs::SunSpecModel::model_id` |
| 2 | Common-block marker value and the following registers | `model.rs::SUNSPEC_MARKER` |
| 3 | Scale-factor rule (×10^SF) and which fields carry SF registers | `model.rs::apply_scale_factor`; `registry.rs::RegisterField.scale_factor_offset` |
| 4 | Data types and byte order (16-bit two's complement; big-endian 32-bit pairs) | `registry.rs::decode_register_window` |
| 5 | Meter power-direction conventions (per model / wiring) | `units_map.rs` (the declared-convention requirement) |
| 6 | The 125-register read limit | `polling.rs::MAX_REGISTERS_PER_READ` |
| 7 | Function code and addressing base for the future read implementation | `transport.rs` trait doc |
| 8 | Well-known TCP port 502 / RTU framing parameters | `transport.rs::TransportConfig` (declared, never defaulted) |

Per-device register maps are lab-verified BEFORE registration (the registry
is reviewed data, record §1); a map that disagrees with the lab device is a
recorded edit to the registry entry — never a silent reinterpretation.

## 5. Honest boundaries

The transport is unwired: no Modbus TCP connection exists, no RTU serial
line was opened, no gateway was polled, no register was read. Polling
budgets are declared constraints, not measured limits. No SunSpec
conformance claim of any kind (conventions §7.3). The simulated-device
counterpart is P14.17's deliverable (wave 3); the written-unrun tests use
synthetic register windows only.

## 6. Permitted-tool outputs

Quoted verbatim in the batch close-out record
(`25_phase_14_wave1_agent_b_close_out.md`).
