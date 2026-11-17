// P17.15 — The simulated OCPP charge points (decision record
// `docs/phases-11-20/phase-17-ev-fleet-charging/records/44_p17_15_simulated_charge_points.md`;
// the P14.17 simulated-counterpart pattern, record 81, carried to the OCPP
// CSMS adapter).
//
// WHAT THIS MODULE IS, stated plainly:
//
// - **The charge-point side of OCPP-J, in-process and hardware-free.** A
//   [`SimulatedChargePoint`] speaks OCPP-J (the codec's array frames) to a
//   real `flexcore_ocpp_adapter::csms::CsmsServer` THROUGH the adapter's
//   named `CsmsTransport` seam — the same seam the unwired transport
//   refuses on (`CSMS_TRANSPORT_SEAM_NOT_WIRED`). Nothing here opens a
//   socket, a TLS session, or an HTTP handshake: the "wire" is the
//   function boundary between this module and the server skeleton, and
//   every frame crosses it through the adapter's own codec
//   (`OcppFrame::encode` / `OcppFrame::decode`).
// - **Deterministic by construction.** The session clock is the declared
//   scenario base instant plus fixed per-step offsets; unique ids are
//   step-indexed (`sim-NN`); the script is a fixed sequence. No wall-clock
//   read, no randomness — the Phase 2 simulator discipline at the OCPP
//   tier.
// - **Every identity names itself.** `sim-ocpp` prefixes every identity
//   string (the P14.17 rule: a record produced FROM a simulated peer is
//   identifiable from its identity alone, forever). Nothing here is
//   evidence about real charge-point behaviour.
// - **The V2G behaviour profile is SYNTHETIC and GATED.** The existing
//   Phase 2 EVSE profile (`crate::scenario::EvseParams`, consumed here)
//   is CHARGING-ONLY (profiles.rs: "no V2G") — that statement is
//   unchanged. The additive [`SimulatedV2gProfile`] models a discharge
//   window and power as STATED SYNTHETIC ASSUMPTIONS (nothing validated,
//   nothing measured) and can only attach to a charge point through the
//   FROZEN five-condition `V2xConfig` gate
//   (`flexcore_domain::ev_fleet_model::V2xConfig`): the link refuses a
//   disabled gate with the named [`SimOcppError::V2gGateDisabled`]. A
//   simulated discharge profile authorizes nothing — it is behaviour
//   fixture data for hardware-free tests, never a capability claim.
// - **The harness-fabricated ISSUED fixture is labelled.** Where the
//   script relays a CSMS-initiated charging profile, the lifecycle state
//   `CommandState::Issued` is a TEST FIXTURE constructed explicitly by the
//   harness (the same fixture the wave-1 adapter tests use) — it is not a
//   real dispatch, it never passed real gates, and the relay still runs
//   through the real ISSUED-only gate (`charging::into_relay_request`).
//
// WHAT THIS MODULE IS NOT: not a CSMS (the server skeleton under test is
// the adapter's own code, not simulated here); not a WebSocket, TLS, or
// OCPP-J-transport implementation (the transport seam stays a design
// seam for the REAL wire; this module is the in-process loopback the
// P14.17 promise named); not a conformance, interoperability, or
// certification result of any kind. Nothing has ever run: every test
// composing this module is WRITTEN-UNRUN and its execution is a human
// gate.

use flexcore_domain::command_state::CommandState;
use flexcore_domain::ev_fleet_model::V2xConfig;
use flexcore_ocpp_adapter::charging::{into_relay_request, record_accepted_status};
use flexcore_ocpp_adapter::csms::{CsmsServer, CsmsTransport, TransportError};
use flexcore_ocpp_adapter::framing::{Action, CallErrorCode, OcppFrame, UniqueId};
use flexcore_ocpp_adapter::model::{
    BootNotificationRequest, ChargePointId, SetChargingProfileRequest, SetChargingProfileStatus,
    StationStatus, StatusNotificationRequest, TransactionEventRequest, TransactionEventType,
};
use flexcore_ocpp_adapter::session::{negotiate, Subprotocol};

use crate::scenario::EvseParams;

/// The identity prefix every simulated OCPP charge point carries in its
/// identity strings (never presentable as a real charge point).
pub const SIM_OCPP_ID_PREFIX: &str = "sim-ocpp";

/// The recorded SYNTHETIC label of the V2G behaviour profile (travels with
/// every profile construction and every discharge value it produces).
pub const SIMULATED_V2G_IS_SYNTHETIC_STATEMENT: &str =
    "SYNTHETIC: the simulated V2G discharge behaviour is stated fixture data \
(shaped assumptions, never a validated physical model and never measured \
data), attached ONLY through the frozen five-condition V2xConfig gate and \
only for hardware-free testing. No result derived from it is evidence \
about real vehicle or EVSE behaviour, and it authorizes no dispatch — \
every real dispatch enters the frozen 14-state command lifecycle at \
PROPOSED.";

/// The recorded harness-fixture label for the ISSUED lifecycle state the
/// relay step uses (the honesty note for the CSMS-initiated profile).
pub const HARNESS_ISSUED_FIXTURE_STATEMENT: &str =
    "HARNESS FIXTURE: the CommandState::Issued value used to relay the \
CSMS-initiated charging profile is constructed explicitly by this \
simulated-peer harness to exercise the adapter's real ISSUED-only relay \
gate. It is not a real dispatch: no validation, feasibility, or \
authorization gate was passed, and no charge point was contacted.";

// --- Errors ---------------------------------------------------------------------

/// Errors of the simulated charge points. Every failure is named; the
/// adapter-side refusals are re-raised with their own messages so a
/// composition test can treat them identically to a real peer's refusals
/// (the P14.17 pattern).
#[derive(Debug, Clone, PartialEq)]
pub enum SimOcppError {
    /// A required field is empty. Silence is never a simulated peer.
    EmptyField(&'static str),
    /// The charge-point identity was refused (the adapter model's own
    /// validation, carried verbatim).
    InvalidIdentity(String),
    /// The declared scenario base instant is negative (the scenario clock
    /// is a UTC-milliseconds instant; a negative base is not representable
    /// as a declared UTC instant here).
    InvalidScenarioBase {
        base_millis: i64,
    },
    /// The consumed EVSE profile params are degenerate (negative power,
    /// SOC outside [0,1], or a non-finite jitter fraction).
    InvalidEvseParams(String),
    /// The V2G behaviour link was attempted with a DISABLED five-condition
    /// gate — the additive profile cannot bypass the frozen §11.5 gate.
    V2gGateDisabled,
    /// The V2G discharge power is not finite and positive.
    InvalidV2gPower {
        value: f64,
    },
    /// The subprotocol negotiation failed (the offered list named nothing
    /// this adapter accepts — the real negotiation function refused).
    NegotiationFailed(String),
    /// The CSMS server refused a step (the adapter's own named error,
    /// carried verbatim).
    ServerRefused(String),
    /// The codec refused a frame (the adapter's own named error, carried
    /// verbatim).
    Codec(String),
    /// A transaction payload failed the model's validation (the rule is
    /// carried verbatim).
    InvalidTransaction(String),
    /// The declared relay fixture is degenerate (it cannot compose a
    /// wire-valid payload through the real composer and relay gate).
    InvalidRelayFixture(String),
}

impl std::fmt::Display for SimOcppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SimOcppError::EmptyField(field) => write!(f, "field '{field}' is empty"),
            SimOcppError::InvalidIdentity(reason) => {
                write!(f, "the simulated charge point identity was refused: {reason}")
            }
            SimOcppError::InvalidScenarioBase { base_millis } => write!(
                f,
                "the scenario base instant {base_millis} is negative; the simulated \
                 session clock is a declared UTC-milliseconds instant"
            ),
            SimOcppError::InvalidEvseParams(reason) => {
                write!(f, "the consumed EVSE profile params are degenerate: {reason}")
            }
            SimOcppError::V2gGateDisabled => write!(
                f,
                "the V2G behaviour profile cannot attach: the five-condition V2xConfig \
                 gate is DISABLED (the §11.5 gate is consumed, never bypassed)"
            ),
            SimOcppError::InvalidV2gPower { value } => {
                write!(f, "the V2G discharge power {value} is not finite and positive")
            }
            SimOcppError::NegotiationFailed(reason) => {
                write!(f, "the simulated charge point's subprotocol offer was refused: {reason}")
            }
            SimOcppError::ServerRefused(reason) => write!(f, "the CSMS server refused: {reason}"),
            SimOcppError::Codec(reason) => write!(f, "the codec refused a frame: {reason}"),
            SimOcppError::InvalidTransaction(reason) => {
                write!(f, "the transaction payload failed validation: {reason}")
            }
            SimOcppError::InvalidRelayFixture(reason) => {
                write!(f, "the declared relay fixture is degenerate: {reason}")
            }
        }
    }
}

impl std::error::Error for SimOcppError {}

// --- The SYNTHETIC V2G behaviour profile -----------------------------------------

/// The SYNTHETIC V2G discharge behaviour profile (additive, P17.15). The
/// existing EVSE profile is CHARGING-ONLY and stays so; this profile is a
/// SEPARATE additive fixture that models a discharge window and power as
/// stated assumptions. It attaches to a charge point ONLY through the
/// frozen five-condition `V2xConfig` gate ([`SimOcppError::V2gGateDisabled`]
/// otherwise) — the gate is consumed, never bypassed, and the profile
/// authorizes nothing ([`SIMULATED_V2G_IS_SYNTHETIC_STATEMENT`]).
#[derive(Debug, Clone, PartialEq)]
pub struct SimulatedV2gProfile {
    /// The discharge window, seconds-of-day UTC (may wrap midnight; the
    /// same window semantics as the Phase 2 profiles' `in_window`).
    pub discharge_window: (u32, u32),
    /// The declared discharge power in MW (finite and positive).
    pub discharge_power_mw: f64,
}

impl SimulatedV2gProfile {
    /// Validates and constructs the profile (power must be finite and
    /// positive; the window shape is checked at use time by the window
    /// semantics below).
    pub fn new(discharge_window: (u32, u32), discharge_power_mw: f64) -> Result<Self, SimOcppError> {
        if !discharge_power_mw.is_finite() || discharge_power_mw <= 0.0 {
            return Err(SimOcppError::InvalidV2gPower {
                value: discharge_power_mw,
            });
        }
        Ok(Self {
            discharge_window,
            discharge_power_mw,
        })
    }
}

/// The V2G behaviour LINK: the SYNTHETIC profile bound to an ENABLED
/// five-condition gate. There is no way to attach a discharge profile to a
/// charge point except through this link's constructor — the §11.5 gate is
/// structurally in the path.
#[derive(Debug, Clone, PartialEq)]
pub struct V2gBehaviourLink {
    /// The SYNTHETIC profile.
    pub profile: SimulatedV2gProfile,
    /// The frozen five-condition gate (held enabled; consumed, never
    /// redefined).
    pub gate: V2xConfig,
}

impl V2gBehaviourLink {
    /// Attaches the profile to the gate. A DISABLED gate is a named
    /// refusal — the simulated V2G behaviour cannot exist outside the
    /// §11.5 enablement decision.
    pub fn attach(profile: SimulatedV2gProfile, gate: V2xConfig) -> Result<Self, SimOcppError> {
        if !gate.is_enabled() {
            return Err(SimOcppError::V2gGateDisabled);
        }
        Ok(Self { profile, gate })
    }

    /// Whether the declared discharge window covers this seconds-of-day
    /// instant (the Phase 2 window semantics; may wrap midnight).
    pub fn in_discharge_window(&self, seconds_of_day: u32) -> bool {
        let (start, end) = self.profile.discharge_window;
        if start < end {
            seconds_of_day >= start && seconds_of_day < end
        } else if start > end {
            seconds_of_day >= start || seconds_of_day < end
        } else {
            false
        }
    }

    /// The SYNTHETIC discharge power the profile declares at this
    /// seconds-of-day instant (0.0 outside the window). A LABELLED
    /// fixture value — never evidence, never a command.
    pub fn discharge_power_mw_at(&self, seconds_of_day: u32) -> f64 {
        if self.in_discharge_window(seconds_of_day) {
            self.profile.discharge_power_mw
        } else {
            0.0
        }
    }
}

// --- The simulated charge point ----------------------------------------------------

/// The simulated OCPP charge point: a validated identity, the declared
/// subprotocol offer, a declared boot payload, the CONSUMED Phase 2 EVSE
/// profile (connection windows, charging power — charging-only), the
/// optional SYNTHETIC V2G behaviour link, and the declared scenario-clock
/// base instant. Deterministic: same declaration, same frame sequence.
#[derive(Debug, Clone)]
pub struct SimulatedChargePoint {
    /// The charge-point identity (carries the `sim-ocpp` prefix; the
    /// registry key on the CSMS side).
    pub identity: ChargePointId,
    /// The subprotocol strings the charge point OFFERS at the handshake,
    /// in preference order (the real `negotiate` function selects from
    /// them; an unsupported offer is the named refusal — the same
    /// fail-closed negotiation a real peer meets).
    pub offered_subprotocols: Vec<String>,
    /// The declared boot payload (vendor/model labels — SYNTHETIC
    /// declarations, never a real product).
    pub boot: BootNotificationRequest,
    /// The CONSUMED Phase 2 EVSE profile (charging-only; connection
    /// windows and charging power). Consumed, never redefined: the
    /// profile's "no V2G" statement is unchanged and any discharge
    /// behaviour lives only in the optional V2G link below.
    pub evse_params: EvseParams,
    /// The optional SYNTHETIC V2G behaviour link (attached only through
    /// the five-condition gate).
    pub v2g: Option<V2gBehaviourLink>,
    /// The declared scenario-clock base instant (UTC milliseconds; every
    /// session timestamp is this plus a fixed per-step offset).
    pub scenario_base_millis: i64,
}

impl SimulatedChargePoint {
    /// Validates and constructs the simulated charge point. The identity
    /// is prefixed `sim-ocpp:`; the EVSE params are validated (finite
    /// non-negative power, SOC values in [0,1], finite jitter); the base
    /// instant must be non-negative.
    pub fn new(
        name: impl Into<String>,
        offered_subprotocols: Vec<String>,
        boot: BootNotificationRequest,
        evse_params: EvseParams,
        v2g: Option<V2gBehaviourLink>,
        scenario_base_millis: i64,
    ) -> Result<Self, SimOcppError> {
        let name = name.into();
        if name.trim().is_empty() {
            return Err(SimOcppError::EmptyField("name"));
        }
        let identity = ChargePointId::new(format!("{SIM_OCPP_ID_PREFIX}:{name}"))
            .map_err(|error| SimOcppError::InvalidIdentity(error.to_string()))?;
        if scenario_base_millis < 0 {
            return Err(SimOcppError::InvalidScenarioBase {
                base_millis: scenario_base_millis,
            });
        }
        Self::validate_evse_params(&evse_params)?;
        Ok(Self {
            identity,
            offered_subprotocols,
            boot,
            evse_params,
            v2g,
            scenario_base_millis,
        })
    }

    fn validate_evse_params(params: &EvseParams) -> Result<(), SimOcppError> {
        if !params.charging_power_mw.is_finite() || params.charging_power_mw < 0.0 {
            return Err(SimOcppError::InvalidEvseParams(format!(
                "charging_power_mw {} is not finite and non-negative",
                params.charging_power_mw
            )));
        }
        for (field, value) in [
            ("initial_soc", params.initial_soc),
            ("soc_target", params.soc_target),
        ] {
            if !value.is_finite() || !(0.0..=1.0).contains(&value) {
                return Err(SimOcppError::InvalidEvseParams(format!(
                    "{field} {value} is outside [0,1]"
                )));
            }
        }
        if !params.jitter_fraction.is_finite() || params.jitter_fraction < 0.0 {
            return Err(SimOcppError::InvalidEvseParams(format!(
                "jitter_fraction {} is not finite and non-negative",
                params.jitter_fraction
            )));
        }
        Ok(())
    }

    /// The SYNTHETIC discharge power this charge point declares at a
    /// seconds-of-day instant (0.0 without a V2G link — the charging-only
    /// default — and 0.0 outside the link's window).
    pub fn v2g_discharge_power_mw_at(&self, seconds_of_day: u32) -> f64 {
        match &self.v2g {
            Some(link) => link.discharge_power_mw_at(seconds_of_day),
            None => 0.0,
        }
    }

    fn unique_id(step: u32) -> UniqueId {
        UniqueId::new(format!("sim-{step:02}")).expect("step ids are short and non-empty")
    }

    /// Encodes one charge-point CALL frame (deterministic unique id from
    /// the step index).
    fn call_text(
        &self,
        step: u32,
        action: Action,
        payload: serde_json::Value,
    ) -> Result<String, SimOcppError> {
        let frame = OcppFrame::Call {
            unique_id: Self::unique_id(step),
            action,
            payload,
        };
        frame
            .encode()
            .map_err(|error| SimOcppError::Codec(error.to_string()))
    }

    /// Sends one charge-point CALL to the server (the in-process wire:
    /// the adapter's own decode path consumes it).
    fn send_call(
        &self,
        server: &mut CsmsServer,
        step: u32,
        action: Action,
        payload: serde_json::Value,
    ) -> Result<OcppFrame, SimOcppError> {
        let text = self.call_text(step, action, payload)?;
        server
            .handle_inbound_text(&self.identity, &text)
            .map_err(|error| SimOcppError::ServerRefused(error.to_string()))
    }
}

// Fixed per-step offsets of the default script (deterministic session
// clock; the scenario base instant is the anchor; no wall clock anywhere).
const OFFSET_STATUS_AVAILABLE_MILLIS: i64 = 1_000;
const OFFSET_HEARTBEAT_MILLIS: i64 = 2_000;
const OFFSET_STATUS_OCCUPIED_MILLIS: i64 = 3_000;
const OFFSET_TX_STARTED_MILLIS: i64 = 4_000;
const OFFSET_TX_UPDATED_MILLIS: i64 = 5_000;
const OFFSET_TX_ENDED_MILLIS: i64 = 6_000;
const OFFSET_STATUS_AVAILABLE_AGAIN_MILLIS: i64 = 7_000;

// Step indices of the default script (deterministic unique ids `sim-NN`).
const STEP_CONNECT: u32 = 0;
const STEP_BOOT: u32 = 1;
const STEP_STATUS_AVAILABLE: u32 = 2;
const STEP_HEARTBEAT: u32 = 3;
const STEP_STATUS_OCCUPIED: u32 = 4;
const STEP_TX_STARTED: u32 = 5;
const STEP_TX_UPDATED: u32 = 6;
const STEP_RELAY: u32 = 7;
const STEP_TX_ENDED: u32 = 8;
const STEP_STATUS_AVAILABLE_AGAIN: u32 = 9;

/// The declared relay fixture: the domain plan inputs the harness composes
/// (through the REAL composer and the REAL ISSUED-only relay gate) for the
/// CSMS-initiated SetChargingProfile step. The `CommandState::Issued` used
/// inside is a labelled harness fixture
/// ([`HARNESS_ISSUED_FIXTURE_STATEMENT`]) — not a real dispatch.
#[derive(Debug, Clone, PartialEq)]
pub struct RelayFixture {
    /// The dispatch envelope the harness composes (canonical MW).
    pub envelope: flexcore_domain::ev_charging_composer::DispatchEnvelope,
    /// The supply context for the W/A conversion.
    pub supply: flexcore_domain::ev_charging_composer::SupplyContext,
    /// The device rate constraints.
    pub device_constraints: flexcore_domain::ev_charging_composer::DeviceRateConstraints,
    /// The stack level.
    pub stack_level: i32,
    /// The EVSE wire id the profile targets.
    pub evse_wire_id: u32,
    /// The CSMS-side charging profile id.
    pub charging_profile_id: i64,
}

impl RelayFixture {
    /// The default depot fixture: a 2-period external-constraints envelope
    /// (100 kW for the first 15 minutes, then 0), three-phase 400 V AC,
    /// stack level 10, station scope (evseId 0), profile id 1 — the same
    /// hand-computed shape the wave-1 composer tests use.
    pub fn depot_default(start_millis: i64) -> Self {
        Self {
            envelope: flexcore_domain::ev_charging_composer::DispatchEnvelope {
                start_millis,
                periods: vec![
                    flexcore_domain::ev_charging_composer::DispatchPeriod {
                        start_offset_s: 0,
                        limit_mw: 0.1,
                    },
                    flexcore_domain::ev_charging_composer::DispatchPeriod {
                        start_offset_s: 900,
                        limit_mw: 0.0,
                    },
                ],
            },
            supply: flexcore_domain::ev_charging_composer::SupplyContext::AcThreePhase {
                nominal_voltage_v: 400.0,
            },
            device_constraints: flexcore_domain::ev_charging_composer::DeviceRateConstraints {
                min_charging_rate: None,
            },
            stack_level: 10,
            evse_wire_id: 0,
            charging_profile_id: 1,
        }
    }
}

// --- The in-process OCPP-J loop ------------------------------------------------------

/// The in-process OCPP-J loop: the simulated charge point driving a REAL
/// `CsmsServer` through the adapter's own codec, negotiation, tracker, and
/// queue surfaces. This is the "wire" of the P14.17 promise: no socket, no
/// TLS, no HTTP — the function boundary plus the adapter's real
/// decode/encode path. Every step appends a named record to
/// [`SimulatedOcppLoop::log`].
#[derive(Debug)]
pub struct SimulatedOcppLoop {
    charge_point: SimulatedChargePoint,
    log: Vec<String>,
}

impl SimulatedOcppLoop {
    /// A loop over the given simulated charge point.
    pub fn new(charge_point: SimulatedChargePoint) -> Self {
        Self {
            charge_point,
            log: Vec::new(),
        }
    }

    /// The session log (named step records, in order — the honest record
    /// of what the loop did; identity strings carry the `sim-ocpp` prefix).
    pub fn log(&self) -> &[String] {
        &self.log
    }

    /// The simulated charge point's identity string.
    pub fn identity(&self) -> &str {
        self.charge_point.identity.as_str()
    }

    /// Step CONNECT: negotiate the subprotocol from the charge point's
    /// offer (the REAL negotiation function — an unsupported offer fails
    /// here exactly as it would on a real handshake) and accept the
    /// connection in the server's registry.
    pub fn connect(&mut self, server: &mut CsmsServer) -> Result<Subprotocol, SimOcppError> {
        let negotiated = negotiate(&self.charge_point.offered_subprotocols)
            .map_err(|error| SimOcppError::NegotiationFailed(error.to_string()))?;
        server
            .accept_connection(self.charge_point.identity.clone(), negotiated)
            .map_err(|error| SimOcppError::ServerRefused(error.to_string()))?;
        self.log.push(format!(
            "{} step {STEP_CONNECT} connect: negotiated {}",
            self.identity(),
            negotiated.wire_name()
        ));
        Ok(negotiated)
    }

    /// Step BOOT: send the declared BootNotification CALL. The server's
    /// real decode path consumes it; the provisioning RESPONSE construction
    /// (the CSMS-side answer) is the adapter block handlers' job and is NOT
    /// simulated here — the loop records the decoded frame, never an
    /// invented acceptance.
    pub fn send_boot(&mut self, server: &mut CsmsServer) -> Result<(), SimOcppError> {
        let payload = serde_json::to_value(&self.charge_point.boot)
            .map_err(|error| SimOcppError::Codec(error.to_string()))?;
        let decoded = self.charge_point.send_call(
            server,
            STEP_BOOT,
            Action::BootNotification,
            payload,
        )?;
        self.log.push(format!(
            "{} step {STEP_BOOT} boot: the server decoded the CALL ({})",
            self.identity(),
            decoded.unique_id()
        ));
        Ok(())
    }

    /// Step STATUS: send one StatusNotification CALL for the declared EVSE
    /// (deterministic timestamp: scenario base + the declared offset).
    pub fn send_status(
        &mut self,
        server: &mut CsmsServer,
        step: u32,
        evse_id: u32,
        status: StationStatus,
        offset_millis: i64,
    ) -> Result<(), SimOcppError> {
        let request = StatusNotificationRequest {
            timestamp_millis: self.charge_point.scenario_base_millis + offset_millis,
            evse_id,
            connector_status: status,
        };
        let payload = serde_json::to_value(&request)
            .map_err(|error| SimOcppError::Codec(error.to_string()))?;
        let decoded =
            self.charge_point
                .send_call(server, step, Action::StatusNotification, payload)?;
        self.log.push(format!(
            "{} step {step} status: {} for evse {} ({})",
            self.identity(),
            status.name(),
            evse_id,
            decoded.unique_id()
        ));
        Ok(())
    }

    /// Step HEARTBEAT: send one Heartbeat CALL (liveness evidence only —
    /// the adapter treats heartbeats as link-liveness records, never an
    /// operational state).
    pub fn send_heartbeat(&mut self, server: &mut CsmsServer) -> Result<(), SimOcppError> {
        let request = flexcore_ocpp_adapter::model::HeartbeatRequest {
            current_time_millis: self.charge_point.scenario_base_millis
                + OFFSET_HEARTBEAT_MILLIS,
        };
        let payload = serde_json::to_value(&request)
            .map_err(|error| SimOcppError::Codec(error.to_string()))?;
        let decoded =
            self.charge_point
                .send_call(server, STEP_HEARTBEAT, Action::Heartbeat, payload)?;
        self.log.push(format!(
            "{} step {STEP_HEARTBEAT} heartbeat: ({})",
            self.identity(),
            decoded.unique_id()
        ));
        Ok(())
    }

    /// Step TRANSACTION: send one TransactionEvent CALL (Started/Updated/
    /// Ended; deterministic timestamps and ids).
    pub fn send_transaction(
        &mut self,
        server: &mut CsmsServer,
        step: u32,
        event_type: TransactionEventType,
        transaction_id: &str,
        energy_wh: Option<f64>,
        offset_millis: i64,
    ) -> Result<(), SimOcppError> {
        if transaction_id.trim().is_empty() {
            return Err(SimOcppError::EmptyField("transaction_id"));
        }
        let request = TransactionEventRequest {
            event_type,
            timestamp_millis: self.charge_point.scenario_base_millis + offset_millis,
            evse_id: 0,
            transaction_id: transaction_id.to_string(),
            trigger_reason: None,
            energy_wh,
        };
        request
            .validate()
            .map_err(|error| SimOcppError::InvalidTransaction(error.to_string()))?;
        let payload = serde_json::to_value(&request)
            .map_err(|error| SimOcppError::Codec(error.to_string()))?;
        let decoded =
            self.charge_point
                .send_call(server, step, Action::TransactionEvent, payload)?;
        self.log.push(format!(
            "{} step {step} transaction: {} {} ({})",
            self.identity(),
            event_type.name(),
            transaction_id,
            decoded.unique_id()
        ));
        Ok(())
    }

    /// Step RELAY: the harness composes the CSMS-initiated charging
    /// profile through the REAL composer and the REAL ISSUED-only relay
    /// gate (the ISSUED state is the labelled harness fixture), queues it
    /// on the server, drains it, decodes it at the charge point, VALIDATES
    /// the payload with the model's own rules, and answers Accepted —
    /// recording the stored-not-applied statement verbatim in the log.
    /// Returns the answered profile id.
    pub fn relay_issued_profile(
        &mut self,
        server: &mut CsmsServer,
        fixture: &RelayFixture,
    ) -> Result<i64, SimOcppError> {
        // Harness-fixture composition through the REAL composer + relay
        // gate. The ISSUED state here is the labelled fixture — see
        // HARNESS_ISSUED_FIXTURE_STATEMENT.
        let plan = flexcore_domain::ev_charging_composer::compose_external_constraints_profile(
            &fixture.envelope,
            flexcore_domain::ev_charging_composer::RateUnit::W,
            &fixture.supply,
            &fixture.device_constraints,
            fixture.stack_level,
            true,
        )
        .map_err(|error| SimOcppError::InvalidRelayFixture(error.to_string()))?;
        let request =
            into_relay_request(&plan, CommandState::Issued, fixture.charging_profile_id)
                .map_err(|error| SimOcppError::InvalidRelayFixture(error.to_string()))?;
        if request.evse_id != fixture.evse_wire_id {
            return Err(SimOcppError::InvalidRelayFixture(format!(
                "the composed plan targets evse {} but the fixture declares {}",
                request.evse_id, fixture.evse_wire_id
            )));
        }
        let payload = serde_json::to_value(&request)
            .map_err(|error| SimOcppError::Codec(error.to_string()))?;
        let unique_id = SimulatedChargePoint::unique_id(STEP_RELAY);
        server
            .queue_outbound_call(
                &self.charge_point.identity,
                Action::SetChargingProfile,
                payload,
                unique_id.clone(),
            )
            .map_err(|error| SimOcppError::ServerRefused(error.to_string()))?;
        let drained = server
            .drain_outbound(&self.charge_point.identity)
            .map_err(|error| SimOcppError::ServerRefused(error.to_string()))?;
        if drained.len() != 1 {
            return Err(SimOcppError::ServerRefused(format!(
                "expected exactly one drained CSMS-initiated CALL, found {}",
                drained.len()
            )));
        }
        // The charge point decodes and VALIDATES the payload (the model's
        // own rules — a well-behaved station refuses a profile it cannot
        // interpret, it does not blindly accept).
        let payload_value = match &drained[0] {
            OcppFrame::Call { payload, .. } => payload.clone(),
            other => {
                return Err(SimOcppError::Codec(format!(
                    "expected a drained CALL, got {other:?}"
                )))
            }
        };
        let profile: SetChargingProfileRequest = serde_json::from_value(payload_value)
            .map_err(|error| SimOcppError::Codec(error.to_string()))?;
        if let Err(error) = profile.validate() {
            // The named refusal path: the station answers CALLERROR, the
            // server's outbound tracker matches it, and the log names the
            // model rule that failed.
            let text = call_error_text(
                &unique_id,
                CallErrorCode::PropertyConstraintViolation,
                &error.to_string(),
            )?;
            server
                .handle_inbound_text(&self.charge_point.identity, &text)
                .map_err(|e| SimOcppError::ServerRefused(e.to_string()))?;
            self.log.push(format!(
                "{} step {STEP_RELAY} relay: the station REFUSED the profile \
                 (model rule: {error}); a CALLERROR was returned",
                self.identity()
            ));
            return Err(SimOcppError::Codec(format!(
                "the simulated station refused the profile: {error}"
            )));
        }
        // Accept: the honest record is the stored-not-applied statement.
        let (statement, status) = record_accepted_status(SetChargingProfileStatus::Accepted);
        let reply = OcppFrame::CallResult {
            unique_id,
            payload: serde_json::json!({ "status": status.name() }),
        };
        let text = reply
            .encode()
            .map_err(|error| SimOcppError::Codec(error.to_string()))?;
        server
            .handle_inbound_text(&self.charge_point.identity, &text)
            .map_err(|error| SimOcppError::ServerRefused(error.to_string()))?;
        self.log.push(format!(
            "{} step {STEP_RELAY} relay: profile {} accepted — {statement}",
            self.identity(),
            profile.charging_profile_id
        ));
        Ok(profile.charging_profile_id)
    }

    /// Step END: the named session end (the server's real end path — the
    /// registry entry's session is ended with the reason and retained for
    /// audit; connection loss is NOT asset unavailability on the domain
    /// side, and this loop touches nothing there).
    pub fn end(&mut self, server: &mut CsmsServer, reason: &str) -> Result<(), SimOcppError> {
        if reason.trim().is_empty() {
            return Err(SimOcppError::EmptyField("reason"));
        }
        server
            .connection_lost(&self.charge_point.identity, reason.to_string())
            .map_err(|error| SimOcppError::ServerRefused(error.to_string()))?;
        self.log
            .push(format!("{} step end: session ended ({reason})", self.identity()));
        Ok(())
    }

    /// The DEFAULT SCRIPT (the deterministic depot session): connect →
    /// boot → status(Available) → heartbeat → status(Occupied) →
    /// transaction Started → Updated → [relay + Accepted answer, when a
    /// fixture is declared] → transaction Ended → status(Available) →
    /// named end. Returns the number of logged steps.
    pub fn run_default_script(
        &mut self,
        server: &mut CsmsServer,
        relay_fixture: Option<&RelayFixture>,
    ) -> Result<usize, SimOcppError> {
        let before = self.log.len();
        self.connect(server)?;
        self.send_boot(server)?;
        self.send_status(
            server,
            STEP_STATUS_AVAILABLE,
            0,
            StationStatus::Available,
            OFFSET_STATUS_AVAILABLE_MILLIS,
        )?;
        self.send_heartbeat(server)?;
        self.send_status(
            server,
            STEP_STATUS_OCCUPIED,
            0,
            StationStatus::Occupied,
            OFFSET_STATUS_OCCUPIED_MILLIS,
        )?;
        self.send_transaction(
            server,
            STEP_TX_STARTED,
            TransactionEventType::Started,
            "sim-txn-1",
            Some(0.0),
            OFFSET_TX_STARTED_MILLIS,
        )?;
        self.send_transaction(
            server,
            STEP_TX_UPDATED,
            TransactionEventType::Updated,
            "sim-txn-1",
            Some(7500.0),
            OFFSET_TX_UPDATED_MILLIS,
        )?;
        if let Some(fixture) = relay_fixture {
            self.relay_issued_profile(server, fixture)?;
        }
        self.send_transaction(
            server,
            STEP_TX_ENDED,
            TransactionEventType::Ended,
            "sim-txn-1",
            Some(42_000.0),
            OFFSET_TX_ENDED_MILLIS,
        )?;
        self.send_status(
            server,
            STEP_STATUS_AVAILABLE_AGAIN,
            0,
            StationStatus::Available,
            OFFSET_STATUS_AVAILABLE_AGAIN_MILLIS,
        )?;
        self.end(server, "script complete")?;
        Ok(self.log.len() - before)
    }
}

/// The CALLERROR frame text the station's refusal path returns (the real
/// codec encodes it; the server's outbound tracker matches it).
fn call_error_text(
    unique_id: &UniqueId,
    code: CallErrorCode,
    description: &str,
) -> Result<String, SimOcppError> {
    let frame = OcppFrame::CallError {
        unique_id: unique_id.clone(),
        error_code: code,
        error_description: description.to_string(),
        error_details: serde_json::json!({}),
    };
    frame
        .encode()
        .map_err(|error| SimOcppError::Codec(error.to_string()))
}

// --- The transport-seam implementation -----------------------------------------------

/// The simulated `CsmsTransport` (the P17.15 deliverable): the in-process
/// loopback wired THROUGH the adapter's named `CsmsTransport` seam — the
/// same seam `UnwiredCsmsTransport` refuses with
/// `CSMS_TRANSPORT_SEAM_NOT_WIRED`. Its `serve` runs the DEFAULT SCRIPT
/// against the server and returns Ok only when every step completed; any
/// step failure is the named [`TransportError`] (a transport never reports
/// success for an exchange it did not complete).
///
/// There is no socket, no TLS, no HTTP anywhere: this is the hardware-free
/// counterpart the P14.17 promise named for the OCPP adapter, and it makes
/// no conformance or interoperability claim.
#[derive(Debug)]
pub struct SimulatedCsmsTransport {
    loop_: SimulatedOcppLoop,
    relay_fixture: Option<RelayFixture>,
}

impl SimulatedCsmsTransport {
    /// Builds the transport over a validated simulated charge point.
    pub fn new(charge_point: SimulatedChargePoint) -> Result<Self, SimOcppError> {
        Ok(Self {
            loop_: SimulatedOcppLoop::new(charge_point),
            relay_fixture: None,
        })
    }

    /// Declares the relay fixture for the default script (no fixture =
    /// the CSMS-initiated profile step is skipped).
    pub fn with_relay_fixture(mut self, fixture: RelayFixture) -> Self {
        self.relay_fixture = Some(fixture);
        self
    }

    /// The session log accumulated so far (also reachable after `serve`).
    pub fn log(&self) -> &[String] {
        self.loop_.log()
    }
}

impl CsmsTransport for SimulatedCsmsTransport {
    fn serve(self: Box<Self>, server: &mut CsmsServer) -> Result<(), TransportError> {
        let mut inner = *self;
        let fixture = inner.relay_fixture.clone();
        inner
            .loop_
            .run_default_script(server, fixture.as_ref())
            .map_err(|error| match error {
                SimOcppError::NegotiationFailed(reason) => {
                    TransportError::NegotiationFailed { reason }
                }
                other => TransportError::TransportClosed {
                    reason: format!("sim-ocpp: {other}"),
                },
            })
    }

    /// The honest self-report outside `serve`: an in-process loopback has
    /// no persistent connection state of its own — before `serve` runs,
    /// nothing was connected, and `Closed` is the only honest answer.
    fn state(&self) -> flexcore_protocol_ports::connection::ConnectionState {
        flexcore_protocol_ports::connection::ConnectionState::Closed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flexcore_ocpp_adapter::csms::{CsmsLimits, CsmsSecurityRequirements};
    use flexcore_ocpp_adapter::scope::csms_manifest;

    fn evse_params() -> EvseParams {
        EvseParams {
            connection_windows: vec![(17 * 3600 + 1800, 8 * 3600)], // wraps midnight
            charging_power_mw: 0.05,
            initial_soc: 0.3,
            soc_target: 0.9,
            emit_energy: true,
            jitter_fraction: 0.0,
        }
    }

    fn boot() -> BootNotificationRequest {
        BootNotificationRequest {
            vendor_name: "sim-vendor".to_string(),
            charge_point_model: "sim-model-1".to_string(),
            charge_point_serial_number: Some("sim-sn-001".to_string()),
            firmware_version: Some("sim-fw-1".to_string()),
            protocol: Some("OCPP 2.0.1 (simulated)".to_string()),
        }
    }

    fn charge_point(name: &str) -> SimulatedChargePoint {
        SimulatedChargePoint::new(
            name,
            vec!["ocpp2.0.1".to_string()],
            boot(),
            evse_params(),
            None,
            1_700_000_000_000,
        )
        .expect("valid simulated charge point")
    }

    fn server() -> CsmsServer {
        CsmsServer::new(
            csms_manifest().expect("valid manifest"),
            CsmsLimits::new(4, 8).expect("valid limits"),
            CsmsSecurityRequirements::new(vec!["sim-trust-anchor".to_string()], true)
                .expect("valid security"),
        )
        .expect("valid server")
    }

    #[test]
    fn identities_carry_the_sim_prefix_never_a_real_name() {
        let cp = charge_point("depot-1");
        assert!(cp.identity.as_str().starts_with("sim-ocpp:"));
        assert!(cp.identity.as_str().contains("depot-1"));
        // An empty name is refused.
        assert_eq!(
            SimulatedChargePoint::new(
                "  ",
                vec!["ocpp2.0.1".to_string()],
                boot(),
                evse_params(),
                None,
                1_700_000_000_000,
            ),
            Err(SimOcppError::EmptyField("name"))
        );
        // A negative scenario base is refused.
        assert_eq!(
            SimulatedChargePoint::new(
                "depot-1",
                vec!["ocpp2.0.1".to_string()],
                boot(),
                evse_params(),
                None,
                -1,
            ),
            Err(SimOcppError::InvalidScenarioBase { base_millis: -1 })
        );
    }

    #[test]
    fn evse_params_are_validated_fail_fast() {
        let mut params = evse_params();
        params.charging_power_mw = -1.0;
        assert!(matches!(
            SimulatedChargePoint::new(
                "cp",
                vec!["ocpp2.0.1".to_string()],
                boot(),
                params,
                None,
                0
            ),
            Err(SimOcppError::InvalidEvseParams(_))
        ));
        let mut params = evse_params();
        params.initial_soc = 1.5;
        assert!(matches!(
            SimulatedChargePoint::new(
                "cp",
                vec!["ocpp2.0.1".to_string()],
                boot(),
                params,
                None,
                0
            ),
            Err(SimOcppError::InvalidEvseParams(_))
        ));
    }

    #[test]
    fn the_v2g_profile_is_synthetic_and_gated_by_the_five_conditions() {
        let profile = SimulatedV2gProfile::new((22 * 3600, 6 * 3600), 0.02).expect("valid");
        // Attaching to a DISABLED gate is the named refusal — the §11.5
        // gate is structurally in the path.
        assert_eq!(
            V2gBehaviourLink::attach(profile.clone(), V2xConfig::disabled()),
            Err(SimOcppError::V2gGateDisabled)
        );
        // A degenerate power is refused.
        assert_eq!(
            SimulatedV2gProfile::new((0, 3600), -0.5),
            Err(SimOcppError::InvalidV2gPower { value: -0.5 })
        );
        // A charge point without a link is charging-only (0.0 everywhere).
        let cp = charge_point("depot-1");
        assert_eq!(cp.v2g_discharge_power_mw_at(23 * 3600), 0.0);
        assert_eq!(cp.v2g_discharge_power_mw_at(3 * 3600), 0.0);
        // The statement travels verbatim.
        assert!(SIMULATED_V2G_IS_SYNTHETIC_STATEMENT.contains("SYNTHETIC"));
        assert!(SIMULATED_V2G_IS_SYNTHETIC_STATEMENT.contains("five-condition V2xConfig gate"));
        assert!(SIMULATED_V2G_IS_SYNTHETIC_STATEMENT.contains("authorizes no dispatch"));
    }

    #[test]
    fn the_gated_v2g_link_declares_discharge_only_in_its_window() {
        // The full five-condition evidence set (synthetic labels — the
        // named-fixture discipline of the domain record 09_11).
        let evidence_for = |text: &str| flexcore_domain::ev_fleet_model::V2xConditionEvidence {
            supported: true,
            evidence: text.to_string(),
        };
        let gate = V2xConfig::enable(flexcore_domain::ev_fleet_model::V2xEvidence {
            ev: evidence_for("ev-evidence"),
            evse: evidence_for("evse-evidence"),
            protocol: evidence_for("protocol-evidence"),
            interconnection_agreement: evidence_for("interconnection-evidence"),
            market_rules: evidence_for("market-evidence"),
        })
        .expect("enabled");
        let profile = SimulatedV2gProfile::new((22 * 3600, 6 * 3600), 0.02).expect("valid");
        let link = V2gBehaviourLink::attach(profile, gate).expect("attached");
        // Window semantics (wrap midnight): inside at 23:00 and 02:00,
        // outside at 12:00.
        assert!(link.in_discharge_window(23 * 3600));
        assert!(link.in_discharge_window(2 * 3600));
        assert!(!link.in_discharge_window(12 * 3600));
        assert_eq!(link.discharge_power_mw_at(23 * 3600), 0.02);
        assert_eq!(link.discharge_power_mw_at(2 * 3600), 0.02);
        assert_eq!(link.discharge_power_mw_at(12 * 3600), 0.0);
    }

    #[test]
    fn the_default_script_runs_end_to_end_through_the_real_adapter_surfaces() {
        let mut server = server();
        let mut loop_ = SimulatedOcppLoop::new(charge_point("depot-1"));
        let steps = loop_
            .run_default_script(&mut server, None)
            .expect("script complete");
        // Every step logged: connect, boot, 2 statuses + heartbeat,
        // 3 transactions, end = 10 entries without the relay step.
        assert_eq!(steps, 10);
        // Every log line names the simulated identity.
        for line in loop_.log() {
            assert!(line.contains("sim-ocpp:depot-1"), "line: {line}");
        }
        // The registry recorded the session end (the real end path).
        let snapshot = server.connection_snapshot();
        assert_eq!(snapshot, vec![("sim-ocpp:depot-1", "ENDED")]);
        // The end is a transport fact: the honest report carries the
        // named reason (connection loss is NOT asset unavailability — the
        // adapter's own rule, untouched here).
        let report = server
            .connection_report(&ChargePointId::new("sim-ocpp:depot-1".to_string()).expect("valid"))
            .expect("report");
        assert!(report.detail.contains("script complete"));
    }

    #[test]
    fn the_relay_step_runs_the_real_issued_only_gate_and_records_stored_not_applied() {
        let mut server = server();
        let mut loop_ = SimulatedOcppLoop::new(charge_point("depot-1"));
        let fixture = RelayFixture::depot_default(1_700_000_000_000);
        loop_.connect(&mut server).expect("connected");
        let profile_id = loop_
            .relay_issued_profile(&mut server, &fixture)
            .expect("relayed");
        assert_eq!(profile_id, 1);
        // The log carries the stored-not-applied statement verbatim.
        let relay_line = loop_
            .log()
            .iter()
            .find(|line| line.contains("relay"))
            .expect("relay logged");
        assert!(relay_line.contains("STORED, not applied"), "{relay_line}");
        assert!(relay_line.contains("profile 1"));
        // The harness-fixture statement is available for every consumer.
        assert!(HARNESS_ISSUED_FIXTURE_STATEMENT.contains("HARNESS FIXTURE"));
        assert!(HARNESS_ISSUED_FIXTURE_STATEMENT.contains("not a real dispatch"));
    }

    #[test]
    fn an_invalid_profile_payload_fails_the_station_side_model_rule() {
        // The station-side negative path: a hand-built INVALID profile
        // payload (duplicate schedule-period starts — the model's
        // NonAscendingPeriod rule) fails the station's validation, which
        // is the rule the relay step's CALLERROR branch keys on. The
        // station never blindly accepts a payload it cannot interpret.
        let payload = serde_json::json!({
            "chargingProfileId": 9,
            "evseId": 0,
            "stackLevel": 1,
            "chargingProfilePurpose": "ChargingStationExternalConstraints",
            "chargingProfileKind": "Absolute",
            "chargingSchedule": {
                "chargingRateUnit": "W",
                "chargingSchedulePeriod": [
                    {"startPeriod": 0, "limit": 100.0},
                    {"startPeriod": 0, "limit": 50.0}
                ]
            }
        });
        let profile: SetChargingProfileRequest = serde_json::from_value(payload).expect("decodes");
        let error = profile.validate().expect_err("duplicate starts refused");
        assert!(error.to_string().contains("strictly ascending"));
    }

    #[test]
    fn the_transport_serves_through_the_named_seam_and_reports_closed_before_serve() {
        // The honest pre-serve state: Closed (nothing connected yet).
        let transport = SimulatedCsmsTransport::new(charge_point("depot-1")).expect("valid");
        assert!(matches!(
            transport.state(),
            flexcore_protocol_ports::connection::ConnectionState::Closed
        ));
        // serve runs the whole script through the REAL server surfaces and
        // reports success only for a completed exchange.
        let mut server = server();
        let transport =
            SimulatedCsmsTransport::new(charge_point("depot-1")).expect("valid transport");
        Box::new(transport)
            .serve(&mut server)
            .expect("the in-process loopback completed");
        assert_eq!(
            server.connection_snapshot(),
            vec![("sim-ocpp:depot-1", "ENDED")]
        );
    }

    #[test]
    fn the_transport_serves_the_relay_fixture_through_the_seam() {
        let mut server = server();
        let transport = SimulatedCsmsTransport::new(charge_point("depot-1"))
            .expect("valid transport")
            .with_relay_fixture(RelayFixture::depot_default(1_700_000_000_000));
        Box::new(transport)
            .serve(&mut server)
            .expect("the loopback with the relay fixture completed");
        assert_eq!(
            server.connection_snapshot(),
            vec![("sim-ocpp:depot-1", "ENDED")]
        );
    }

    #[test]
    fn an_unsupported_subprotocol_offer_fails_the_real_negotiation() {
        // The 1.6 offer is refused with the P17-E-1 reason — the simulated
        // peer meets the SAME fail-closed negotiation a real peer would.
        let cp = SimulatedChargePoint::new(
            "legacy",
            vec!["ocpp1.6".to_string()],
            boot(),
            evse_params(),
            None,
            0,
        )
        .expect("valid charge point");
        let mut server = server();
        let mut loop_ = SimulatedOcppLoop::new(cp);
        let error = loop_.connect(&mut server).expect_err("1.6 refused");
        assert!(matches!(error, SimOcppError::NegotiationFailed(_)));
        assert!(error.to_string().contains("P17-E-1"));
        // Nothing was registered: the refusal is WHOLE.
        assert!(server.connection_snapshot().is_empty());
    }
}
