// FIM-F03.03 — the SunSpec model read through the LIVE client
// (docs/releases/final-implementation-before-publication.md §5 F-03;
// ledger docs/releases/implementation_ledger/ledger_F03.md subtask
// FIM-F03.03), plus the deterministic register fixture.
//
// **Implemented under FIM-F03, WRITTEN-UNRUN: this module has never been
// compiled or executed, and no device has ever answered a SunSpec read.
// Device interop is UNPROVEN and remains a HUMAN LAB GATE** (crate
// conventions). The read flow below drives the EXISTING reviewed layers —
// the transport port trait, the register-map registry, the scale-factor
// rule, and the adapter-boundary unit conversion — unchanged; nothing in
// the model/registry/units boundary was edited to make this possible.
//
// The flow (over ANY [`ModbusTransport`] implementation — the live TCP
// transport in [`crate::tcp_transport`], or the deterministic in-memory
// fixture [`SimulatedRegisterTransport`] below):
//
// 1. **SunSpec header.** Read TWO registers at the DEPLOYMENT-DECLARED
//    base address and require the "SunS" common-block marker
//    ([`crate::model::SUNSPEC_MARKER`] = 0x53756E53 — UNVERIFIED per
//    [S-16]; the lab gate confirms). A mismatch is the named
//    NotSunSpecShaped refusal: a non-SunSpec device map is refused, not
//    guessed at. The base address is the protocol address of the common
//    block — the 4xxxx-style 40001 register base is a configuration
//    translation DECLARED per deployment, never assumed here (the
//    record-23 §4 UNVERIFIED anchor list; addressing-base anchor in
//    transport.rs).
// 2. **Model block walk.** Read the REGISTERED map's block
//    (`block_length_registers`) starting at the DEPLOYMENT-DECLARED model
//    block address, in budget-sized chunks ([`GatewayPollingBudget::
//    max_registers_per_read`] — the existing budget type governs the
//    walk, never a hardcoded split). The common block's "address of the
//    first model" pointer is UNVERIFIED per record 23 §4 #2, so this
//    module REFUSES to infer the block address from the header: it is a
//    declared argument of [`SunSpecDeviceReader::read_device`]. The
//    block's FIRST register is required to equal the registered family's
//    model id ([`crate::model::SunSpecModel::model_id`] — UNVERIFIED
//    anchor; a mismatch is the named ModelIdMismatch refusal).
//    Consequently, maps read through the live reader place their fields
//    at offset >= 1 (the id register is window index 0); the synthetic-
//    window tests in `tests/register_conversion.rs` drive the decoder
//    directly and are unaffected.
// 3. **Point decode.** Every mapped field is decoded through the
//    EXISTING [`crate::registry::decode_register_window`] (scale factors
//    applied by the existing rule) — raw register values convert THROUGH
//    the registry, never around it.
// 4. **Canonical conversion.** Every decoded point converts at the
//    adapter boundary through the EXISTING [`crate::units_map`] functions
//    (W → MW, VAr → MVAr under the REQUIRED declared reactive sign
//    convention, Wh → MWh, percent → SOC fraction through the named
//    `from_percent` constructor). Display-only kinds (volts, amperes,
//    ...) stay in source terms — the same boundary precedent. A refused
//    conversion refuses the WHOLE read (fail-closed): no row is ever
//    surfaced without its canonical value, quality, and provenance.
// 5. **Telemetry rows.** Successful rows carry the frozen
//    [`TelemetryQuality`] vocabulary (`GOOD` — a fresh, validated live
//    read; no mapped-source-state downgrade happens here, the non-
//    silencing rule is untouched) and the frozen [`Provenance`]
//    (`MEASURED` — the value came off the wire, never from a model).
//
// [`SimulatedRegisterTransport`] is the deterministic fixture: an
// in-memory register file implementing the EXISTING port trait (no
// socket, no thread) that serves ONLY its seeded windows and refuses
// everything else with the pre-existing `WindowOutOfRange` refusal (the
// P14.17 declared-window discipline). It exists so the SunSpec flow is
// pinned end-to-end without sockets, and as the seed shape for the
// simulated-device counterpart (P14.17, record 81).

use std::cell::RefCell;
use std::collections::BTreeMap;

use flexcore_contracts::capability::Provenance;
use flexcore_contracts::quality::TelemetryQuality;
use flexcore_contracts::units::{
    ActivePowerMW, EnergyMWh, ReactivePowerDirection, ReactivePowerMVAr, ReactiveSignConvention,
    StateOfCharge,
};

use crate::model::{DecodedField, RegisterUnitKind, SunSpecModel, SUNSPEC_MARKER};
use crate::polling::GatewayPollingBudget;
use crate::registry::{decode_register_window, DeviceModelKey, RegisterMapRegistry, RegistryError};
use crate::transport::{ModbusTransport, TransportError};
use crate::units_map::{
    percent_to_soc, var_to_reactive_power, watt_hours_to_energy, watts_to_active_power,
    ConversionError,
};

/// Errors of the live SunSpec read (FIM-F03.03/FIM-F03.04): every failure
/// is a named refusal carried from the layer that owns it — the read is
/// fail-closed whole (no partial row set, no swallowed error, no hang).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SunSpecReadError {
    /// The transport refused (unreachable, timeout, exception, malformed
    /// frame — the FIM-F03.04 taxonomy).
    Transport(TransportError),
    /// The registry or the decoder refused (unregistered device model,
    /// short window, missing scale factor).
    Registry(RegistryError),
    /// The adapter-boundary unit conversion refused (unit-kind mismatch,
    /// non-finite value, SOC outside [0,1]).
    Conversion(ConversionError),
    /// The device did not present the SunSpec "SunS" common-block marker
    /// at the declared base; the device map is refused, not guessed at.
    NotSunSpecShaped { got_marker: u32 },
    /// The model block's first register did not equal the registered
    /// family's model id (UNVERIFIED anchor: the block starts with the id
    /// register per [S-16]; a mismatch is refused, not reinterpreted).
    ModelIdMismatch { expected: u16, got: u16 },
    /// The declared model block address plus block length overflows the
    /// register space (a declared-window defect, refused up front).
    BlockAddressOverflow { model_block_address: u16 },
}

impl std::fmt::Display for SunSpecReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SunSpecReadError::Transport(error) => write!(f, "transport refused: {error}"),
            SunSpecReadError::Registry(error) => write!(f, "register map refused: {error}"),
            SunSpecReadError::Conversion(error) => write!(f, "unit conversion refused: {error}"),
            SunSpecReadError::NotSunSpecShaped { got_marker } => {
                write!(
                    f, "device did not present the SunSpec common-block marker (got {got_marker:#010x}, expected {SUNSPEC_MARKER:#010x}); refusing to guess a register layout"
                )
            }
            SunSpecReadError::ModelIdMismatch { expected, got } => {
                write!(f, "model block starts with model id {got}, not the registered family's id {expected}; refusing")
            }
            SunSpecReadError::BlockAddressOverflow { model_block_address } => {
                write!(
                    f, "declared model block address {model_block_address} plus the mapped block length overflows the register space"
                )
            }
        }
    }
}

impl std::error::Error for SunSpecReadError {}

impl From<TransportError> for SunSpecReadError {
    fn from(error: TransportError) -> Self {
        SunSpecReadError::Transport(error)
    }
}

/// The canonical reading of one telemetry row, produced ONLY through the
/// existing [`crate::units_map`] boundary functions (canonical units stay
/// MW / MVAr / MWh / SOC fraction — the frozen contract rule).
#[derive(Debug, Clone, PartialEq)]
pub enum CanonicalReading {
    /// Active power in MW (the frozen P > 0 = export sign passes through).
    ActivePower(ActivePowerMW),
    /// Reactive power in MVAr plus its direction, computed only through
    /// `interpret_reactive_power` under the DECLARED sign convention.
    ReactivePower(ReactivePowerMVAr, ReactivePowerDirection),
    /// Energy in MWh.
    Energy(EnergyMWh),
    /// State of charge as a fraction in [0,1].
    StateOfCharge(StateOfCharge),
    /// A display-only source value (volts, amperes, centi-hertz, celsius,
    /// enumerated): canonical voltage/current types are grid-model
    /// concerns (the units_map precedent); the value stays in SOURCE
    /// terms with its unit kind carried on the row.
    DisplayOnly(f64),
}

/// One telemetry row of a live SunSpec read: the decoded source value,
/// its canonical conversion, and the frozen quality/provenance states.
#[derive(Debug, Clone, PartialEq)]
pub struct SunSpecTelemetryRow {
    /// The device model the row belongs to (the registry key).
    pub device_model: DeviceModelKey,
    /// The mapped field's stable label.
    pub label: String,
    /// The SunSpec model family the field was read within.
    pub model: SunSpecModel,
    /// The unit kind before adapter-boundary conversion.
    pub unit_kind: RegisterUnitKind,
    /// The decoded value in SOURCE terms (scale factor applied, before
    /// canonical conversion).
    pub source_value: f64,
    /// The canonical reading (through the existing units_map boundary).
    pub canonical: CanonicalReading,
    /// The frozen telemetry quality state (`GOOD` for a fresh, validated
    /// live read; the six-state vocabulary is never grown).
    pub quality: TelemetryQuality,
    /// The frozen provenance (`MEASURED` — the value came off the wire).
    pub provenance: Provenance,
}

/// The live SunSpec device reader over ANY [`ModbusTransport`]
/// implementation (FIM-F03.03). Constructed with the deployment-DECLARED
/// common-block base address and the gateway's declared polling budget
/// (which bounds every block-walk read — no hardcoded split).
#[derive(Debug)]
pub struct SunSpecDeviceReader<'a> {
    /// The transport the reads go through (live TCP or the deterministic
    /// fixture).
    transport: &'a dyn ModbusTransport,
    /// The DECLARED protocol address of the SunSpec common block (never
    /// inferred, never defaulted — the 4xxxx-style base is a deployment
    /// configuration translation, UNVERIFIED anchor per record 23 §4).
    base_address: u16,
    /// The budget-bounded chunk size for block walks.
    max_registers_per_read: u16,
}

impl<'a> SunSpecDeviceReader<'a> {
    /// Constructs the reader over a transport, the declared common-block
    /// base address, and the gateway's declared polling budget.
    pub fn new(
        transport: &'a dyn ModbusTransport,
        base_address: u16,
        budget: &GatewayPollingBudget,
    ) -> Self {
        Self {
            transport,
            base_address,
            max_registers_per_read: budget.max_registers_per_read,
        }
    }

    /// Reads one registered device model end to end (marker → block walk
    /// → decode → canonical conversion → rows). Fails closed: any refusal
    /// aborts the whole read with the named error.
    ///
    /// `model_block_address` is the deployment-DECLARED protocol address
    /// of the registered family's model block — the common block's
    /// first-model pointer is UNVERIFIED (record 23 §4 #2) and is NOT
    /// inferred from the header here. `reactive_convention` is the device
    /// profile's DECLARED [`ReactiveSignConvention`] (required even for
    /// maps with no VAr field: there is no convention-free overload, by
    /// the frozen contracts rule).
    pub fn read_device(
        &self,
        registry: &RegisterMapRegistry,
        key: &DeviceModelKey,
        model_block_address: u16,
        reactive_convention: ReactiveSignConvention,
    ) -> Result<Vec<SunSpecTelemetryRow>, SunSpecReadError> {
        let map = registry.map_for(key).map_err(SunSpecReadError::Registry)?;
        // 1. The SunSpec header: two registers at the declared base must
        //    carry the "SunS" marker (UNVERIFIED anchor; big-endian u32 —
        //    the registry's word-order convention).
        let marker_registers = self
            .transport
            .read_holding_registers(self.base_address, 2)
            .map_err(SunSpecReadError::Transport)?;
        let marker = match marker_registers.as_slice() {
            [high, low] => (u32::from(*high) << 16) | u32::from(*low),
            // Totality arm: the port trait's contract is exactly `count`
            // registers; a violating implementation is a named refusal,
            // never a panic or an out-of-bounds read.
            _ => {
                return Err(SunSpecReadError::Transport(TransportError::MalformedFrame {
                    defect: crate::codec::FrameDefect::UnexpectedPayload,
                }))
            }
        };
        if marker != SUNSPEC_MARKER {
            return Err(SunSpecReadError::NotSunSpecShaped { got_marker: marker });
        }
        // 2. The model block walk, in budget-sized chunks. The block
        //    address is declared; a block that would overflow the
        //    register space is refused up front.
        let mut window: Vec<u16> = Vec::with_capacity(usize::from(map.block_length_registers));
        let mut offset: u16 = 0;
        while offset < map.block_length_registers {
            let take = self.max_registers_per_read.min(map.block_length_registers - offset);
            let chunk_address = model_block_address
                .checked_add(offset)
                .ok_or(SunSpecReadError::BlockAddressOverflow { model_block_address })?;
            let chunk = self
                .transport
                .read_holding_registers(chunk_address, take)
                .map_err(SunSpecReadError::Transport)?;
            window.extend_from_slice(&chunk);
            offset += take;
        }
        // 3. The model id check (UNVERIFIED anchor: the block starts with
        //    the id register per [S-16]).
        let got_model_id = match window.first() {
            Some(&first) => first,
            None => {
                return Err(SunSpecReadError::Transport(TransportError::MalformedFrame {
                    defect: crate::codec::FrameDefect::UnexpectedPayload,
                }))
            }
        };
        if got_model_id != map.family.model_id() {
            return Err(SunSpecReadError::ModelIdMismatch {
                expected: map.family.model_id(),
                got: got_model_id,
            });
        }
        // 4. + 5. Decode and convert every mapped field; every refusal is
        //    fail-closed for the whole read.
        let mut rows = Vec::with_capacity(map.fields.len());
        for field in &map.fields {
            let decoded =
                decode_register_window(map, field, &window).map_err(SunSpecReadError::Registry)?;
            let canonical = canonical_reading(&decoded, reactive_convention)
                .map_err(SunSpecReadError::Conversion)?;
            rows.push(SunSpecTelemetryRow {
                device_model: map.device_model.clone(),
                label: decoded.label.clone(),
                model: decoded.model,
                unit_kind: decoded.unit_kind,
                source_value: decoded.value,
                canonical,
                quality: TelemetryQuality::Good,
                provenance: Provenance::Measured,
            });
        }
        Ok(rows)
    }
}

/// Converts one decoded field into its canonical reading, through the
/// EXISTING [`crate::units_map`] boundary functions ONLY (raw register
/// values never bypass the boundary; display-only kinds stay in source
/// terms).
fn canonical_reading(
    field: &DecodedField,
    reactive_convention: ReactiveSignConvention,
) -> Result<CanonicalReading, ConversionError> {
    match field.unit_kind {
        RegisterUnitKind::Watts => watts_to_active_power(field).map(CanonicalReading::ActivePower),
        RegisterUnitKind::Var => var_to_reactive_power(field, reactive_convention)
            .map(|(q, direction)| CanonicalReading::ReactivePower(q, direction)),
        RegisterUnitKind::WattHours => watt_hours_to_energy(field).map(CanonicalReading::Energy),
        RegisterUnitKind::Percent => percent_to_soc(field).map(CanonicalReading::StateOfCharge),
        RegisterUnitKind::Volts
        | RegisterUnitKind::Amperes
        | RegisterUnitKind::CentiHertz
        | RegisterUnitKind::Celsius
        | RegisterUnitKind::Enumerated => Ok(CanonicalReading::DisplayOnly(field.value)),
    }
}

/// The deterministic register fixture: an in-memory register file that
/// implements the EXISTING [`ModbusTransport`] port trait (no socket, no
/// thread, no runtime). It serves ONLY its seeded windows — an
/// unseeded address is the pre-existing `WindowOutOfRange` refusal (the
/// P14.17 declared-window discipline) — and records the reads it served,
/// so the budget-bounded chunking of a block walk is pinned exactly.
#[derive(Debug)]
pub struct SimulatedRegisterTransport {
    registers: BTreeMap<u16, u16>,
    reads: RefCell<Vec<(u16, u16)>>,
}

impl SimulatedRegisterTransport {
    /// Constructs an empty fixture.
    pub fn new() -> Self {
        Self { registers: BTreeMap::new(), reads: RefCell::new(Vec::new()) }
    }

    /// Seeds one window (an overwrite of the same addresses replaces the
    /// values — a REVIEWED fixture edit, like the registry's own rule).
    pub fn seed(&mut self, start_address: u16, registers: &[u16]) {
        for (index, value) in registers.iter().enumerate() {
            if let Some(address) = start_address.checked_add(index as u16) {
                self.registers.insert(address, *value);
            }
        }
    }

    /// The reads this fixture served, in order ((start, count) pairs) —
    /// the chunking evidence for the block-walk budget test.
    pub fn recorded_reads(&self) -> Vec<(u16, u16)> {
        self.reads.borrow().clone()
    }
}

impl Default for SimulatedRegisterTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl ModbusTransport for SimulatedRegisterTransport {
    fn read_holding_registers(
        &self,
        start_address: u16,
        count: u16,
    ) -> Result<Vec<u16>, TransportError> {
        self.reads.borrow_mut().push((start_address, count));
        if count == 0 {
            return Err(TransportError::WindowOutOfRange { start_address, count });
        }
        let last_address = start_address
            .checked_add(count - 1)
            .ok_or(TransportError::WindowOutOfRange { start_address, count })?;
        let mut registers = Vec::with_capacity(usize::from(count));
        for address in start_address..=last_address {
            match self.registers.get(&address) {
                Some(value) => registers.push(*value),
                // An unseeded address is outside the DECLARED block: the
                // named refusal, never a zero-fill.
                None => return Err(TransportError::WindowOutOfRange { start_address, count }),
            }
        }
        Ok(registers)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::RegisterType;
    use crate::registry::RegisterField;

    fn key(name: &str) -> DeviceModelKey {
        DeviceModelKey::new(name).expect("valid key")
    }

    fn meter_map() -> RegisterMap {
        // The lab fixture map: one three-phase meter block, family 203,
        // with the model-id register at window index 0 and the mapped
        // fields at offsets >= 1 (see the module header's layout note).
        // Shapes mirror the reviewed conversion test (W / VAr sharing one
        // SF register, WH as a big-endian u32 pair).
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
        RegisterMap::new(key("lab/meter-3ph-1"), SunSpecModel::MeterThreePhase, fields, 6)
            .expect("valid map")
    }

    fn registered_meter_registry() -> RegisterMapRegistry {
        let mut registry = RegisterMapRegistry::new(crate::ADAPTER_ID);
        registry.register(meter_map()).expect("valid map");
        registry
    }

    /// Seeds the fixture: the SunSpec marker at the declared base
    /// (0x5375 / 0x6E53 = 0x53756E53 "SunS"), then the model block at its
    /// declared address — model id 203, W = 0xFA24 (-1500 as i16), VAr =
    /// 0x1388 (5000), SF = 0x0000, WH = 0x0001/0x86A0 (100000, big-endian
    /// pair).
    fn seeded_meter_fixture() -> SimulatedRegisterTransport {
        let mut fixture = SimulatedRegisterTransport::new();
        fixture.seed(100, &[0x5375, 0x6E53]);
        fixture.seed(120, &[203, 0xFA24, 0x1388, 0x0000, 0x0001, 0x86A0]);
        fixture
    }

    fn meter_reader<'a>(
        fixture: &'a SimulatedRegisterTransport,
    ) -> SunSpecDeviceReader<'a> {
        SunSpecDeviceReader::new(
            fixture,
            100,
            &GatewayPollingBudget::new(125, 1, 1_000).expect("valid budget"),
        )
    }

    // FIM-F03.05 — the full SunSpec read maps the fixture block into
    // telemetry rows with canonical units, GOOD quality, and MEASURED
    // provenance (all through the existing registry + units_map layers).
    #[test]
    fn a_seeded_device_reads_into_canonical_telemetry_rows() {
        let fixture = seeded_meter_fixture();
        let registry = registered_meter_registry();
        let reader = meter_reader(&fixture);
        let rows = reader
            .read_device(
                &registry,
                &key("lab/meter-3ph-1"),
                120,
                flexcore_contracts::units::ReactiveSignConvention::InjectionPositive,
            )
            .expect("the seeded device reads");
        assert_eq!(rows.len(), 4);
        // W: -1500 W → -0.00015 MW (frozen sign rule passes through).
        assert_eq!(rows[0].label, "W");
        assert_eq!(rows[0].source_value, -1500.0);
        match &rows[0].canonical {
            CanonicalReading::ActivePower(mw) => {
                assert!((mw.value() - (-0.0015)).abs() < 1e-12);
            }
            other => panic!("expected an active-power row, got {other:?}"),
        }
        assert_eq!(rows[0].quality, TelemetryQuality::Good);
        assert_eq!(rows[0].provenance, Provenance::Measured);
        // VAr: 5000 VAr → 0.005 MVAr, Injection under the declared
        // convention (the same numeric under the opposite convention maps
        // to Consumption — the sign is never universal).
        assert_eq!(rows[1].label, "VAr");
        match &rows[1].canonical {
            CanonicalReading::ReactivePower(q, direction) => {
                assert!((q.value() - 0.005).abs() < 1e-12);
                assert_eq!(*direction, ReactivePowerDirection::Injection);
            }
            other => panic!("expected a reactive-power row, got {other:?}"),
        }
        // WH: 100000 Wh → 0.1 MWh.
        assert_eq!(rows[2].label, "WH");
        match &rows[2].canonical {
            CanonicalReading::Energy(mwh) => {
                assert!((mwh.value() - 0.1).abs() < 1e-12);
            }
            other => panic!("expected an energy row, got {other:?}"),
        }
        // W_SF: an enumerated SF register → display-only in source terms.
        assert_eq!(rows[3].label, "W_SF");
        assert_eq!(rows[3].canonical, CanonicalReading::DisplayOnly(0.0));
        assert_eq!(rows[3].quality, TelemetryQuality::Good);
    }

    // The reactive direction follows the DECLARED convention, not the
    // numeric sign: the same fixture maps to Consumption under the
    // opposite convention (the frozen units rule, pinned at the live-flow
    // level).
    #[test]
    fn the_reactive_direction_follows_the_declared_convention() {
        let fixture = seeded_meter_fixture();
        let registry = registered_meter_registry();
        let reader = meter_reader(&fixture);
        let rows = reader
            .read_device(
                &registry,
                &key("lab/meter-3ph-1"),
                120,
                flexcore_contracts::units::ReactiveSignConvention::ConsumptionPositive,
            )
            .expect("reads");
        match &rows[1].canonical {
            CanonicalReading::ReactivePower(_, direction) => {
                assert_eq!(*direction, ReactivePowerDirection::Consumption);
            }
            other => panic!("expected a reactive-power row, got {other:?}"),
        }
    }

    // A non-SunSpec device map is refused with the named marker refusal.
    #[test]
    fn a_non_sunspec_device_map_is_refused() {
        let mut fixture = SimulatedRegisterTransport::new();
        fixture.seed(100, &[0x0000, 0x0000]);
        let registry = RegisterMapRegistry::new(crate::ADAPTER_ID);
        let reader = SunSpecDeviceReader::new(
            &fixture,
            100,
            &GatewayPollingBudget::new(125, 1, 1_000).expect("valid budget"),
        );
        assert_eq!(
            reader.read_device(
                &registry,
                &key("lab/meter-3ph-1"),
                120,
                flexcore_contracts::units::ReactiveSignConvention::InjectionPositive,
            ),
            Err(SunSpecReadError::NotSunSpecShaped { got_marker: 0 })
        );
    }

    // A model block whose first register is not the registered family's
    // id is refused (the UNVERIFIED layout anchor is a named refusal).
    #[test]
    fn a_model_id_mismatch_is_refused() {
        let mut fixture = SimulatedRegisterTransport::new();
        fixture.seed(100, &[0x5375, 0x6E53]);
        fixture.seed(120, &[101, 0xFA24, 0x1388, 0x0000, 0x0001, 0x86A0]);
        let registry = registered_meter_registry();
        let reader = meter_reader(&fixture);
        assert_eq!(
            reader.read_device(
                &registry,
                &key("lab/meter-3ph-1"),
                120,
                flexcore_contracts::units::ReactiveSignConvention::InjectionPositive,
            ),
            Err(SunSpecReadError::ModelIdMismatch { expected: 203, got: 101 })
        );
    }

    // An unregistered device model is the registry's own named refusal.
    #[test]
    fn an_unregistered_device_model_is_refused() {
        let fixture = seeded_meter_fixture();
        let registry = RegisterMapRegistry::new(crate::ADAPTER_ID);
        let reader = meter_reader(&fixture);
        assert!(matches!(
            reader.read_device(
                &registry,
                &key("lab/meter-3ph-1"),
                120,
                flexcore_contracts::units::ReactiveSignConvention::InjectionPositive,
            ),
            Err(SunSpecReadError::Registry(
                RegistryError::DeviceModelNotRegistered { .. }
            ))
        ));
    }

    // A read outside the fixture's seeded windows is the pre-existing
    // declared-window refusal (P14.17 discipline, carried by the trait).
    #[test]
    fn an_unseeded_window_is_the_named_window_refusal() {
        let fixture = SimulatedRegisterTransport::new();
        assert_eq!(
            fixture.read_holding_registers(200, 4),
            Err(TransportError::WindowOutOfRange { start_address: 200, count: 4 })
        );
    }

    // FIM-F03.03 — the block walk is chunked by the DECLARED budget, and
    // the recorded reads show exactly the split (budget of 10 over a
    // 25-register block = 10 + 10 + 5).
    #[test]
    fn the_block_walk_is_chunked_by_the_declared_budget() {
        let mut fixture = SimulatedRegisterTransport::new();
        fixture.seed(100, &[0x5375, 0x6E53]);
        // A 25-register inverter block: id 103 at window index 0, then
        // zeros (no mapped fields beyond one — the walk is the point).
        fixture.seed(120, &[103]);
        fixture.seed(121, &[0u16; 24]);
        let fields = vec![RegisterField::new(
            "W",
            SunSpecModel::InverterThreePhase,
            1,
            RegisterType::I16,
            None,
            RegisterUnitKind::Watts,
        )
        .expect("valid field")];
        let map = RegisterMap::new(
            key("lab/inv-3ph-1"),
            SunSpecModel::InverterThreePhase,
            fields,
            25,
        )
        .expect("valid map");
        let mut registry = RegisterMapRegistry::new(crate::ADAPTER_ID);
        registry.register(map).expect("registers");
        let reader = SunSpecDeviceReader::new(
            &fixture,
            100,
            &GatewayPollingBudget::new(10, 1, 1_000).expect("valid budget"),
        );
        let rows = reader
            .read_device(
                &registry,
                &key("lab/inv-3ph-1"),
                120,
                flexcore_contracts::units::ReactiveSignConvention::InjectionPositive,
            )
            .expect("the seeded device reads");
        assert_eq!(rows.len(), 1);
        // W = 0 raw with no SF register -> 0.0 W -> 0.0 MW (the canonical
        // boundary still applies).
        match &rows[0].canonical {
            CanonicalReading::ActivePower(mw) => {
                assert!((mw.value() - 0.0).abs() < 1e-15);
            }
            other => panic!("expected an active-power row, got {other:?}"),
        }
        // The walk: one 2-register marker read, then 10 + 10 + 5.
        assert_eq!(
            fixture.recorded_reads(),
            vec![(100, 2), (120, 10), (130, 10), (140, 5)]
        );
    }

    // A refused conversion refuses the WHOLE read (fail-closed): an
    // out-of-range SOC percent produces no rows at all, just the named
    // conversion refusal.
    #[test]
    fn a_refused_conversion_refuses_the_whole_read() {
        let mut fixture = SimulatedRegisterTransport::new();
        fixture.seed(100, &[0x5375, 0x6E53]);
        // Battery base block (id 801) with SOC = 150 percent.
        fixture.seed(120, &[801, 0x0096]);
        let fields = vec![RegisterField::new(
            "SOC",
            SunSpecModel::BatteryBase,
            1,
            RegisterType::U16,
            None,
            RegisterUnitKind::Percent,
        )
        .expect("valid field")];
        let map = RegisterMap::new(key("lab/bess-1"), SunSpecModel::BatteryBase, fields, 2)
            .expect("valid map");
        let mut registry = RegisterMapRegistry::new(crate::ADAPTER_ID);
        registry.register(map).expect("registers");
        let reader = meter_reader(&fixture);
        assert!(matches!(
            reader.read_device(
                &registry,
                &key("lab/bess-1"),
                120,
                flexcore_contracts::units::ReactiveSignConvention::InjectionPositive,
            ),
            Err(SunSpecReadError::Conversion(_))
        ));
    }

    // FIM-F03.04 — the read error type is spellable end to end.
    #[test]
    fn every_read_error_displays() {
        let cases = [
            SunSpecReadError::Transport(TransportError::SeamNotWired),
            SunSpecReadError::Registry(RegistryError::NotSunSpecShaped),
            SunSpecReadError::Conversion(ConversionError::UnitKindMismatch {
                label: "W".to_string(),
                unit_kind: "volts",
            }),
            SunSpecReadError::NotSunSpecShaped { got_marker: 0 },
            SunSpecReadError::ModelIdMismatch { expected: 203, got: 101 },
            SunSpecReadError::BlockAddressOverflow { model_block_address: 65_535 },
        ];
        for error in cases {
            assert!(!error.to_string().is_empty());
        }
    }
}
