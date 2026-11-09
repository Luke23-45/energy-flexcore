// P3.11 — EV fleet capability model (pure rules, no I/O).
//
// Implements ledger P3.11 ("EV fleet model"), report §11.5 (with §11.1).
// Decision record:
// `ledger/phase-03-flexibility-scheduling/09_11_load_ev_models.md`.
//
// THE MODEL, EXACTLY AS SPECIFIED (report §11.5): the fleet flexibility
// model carries, per EVSE and vehicle:
//
//   EVSE rated power and phase · vehicle connection windows ·
//   required departure energy/SOC · minimum charging power and ramp ·
//   user override and opt-out · charging priority · site transformer limit ·
//   bidirectional capability (gated, see below)
//
// - Units are canonical (§10.2): MW for power, MWh for energy. SOC is a
//   fraction in [0,1] at the API boundary, boundary-inclusive. `P > 0` means
//   EXPORT, so charging is NEGATIVE net export for the fleet; charge and
//   discharge are carried as separate non-negative magnitudes (the §11.2
//   convention).
// - **Energy balance (declared convention, not physics)**: `E(t+1) = E(t) +
//   (P_charge − P_discharge)·Δt`, applied ONLY in intervals where the
//   vehicle is connected and not opted out. A NEGATIVE magnitude
//   (a violation) and a discharge while V2X is disabled contribute ZERO to
//   the balance — they are physical gates on impossible energy flow, and
//   each still reports its named violation (the REJECT honesty discipline
//   of the BESS model applies: nothing is silently repaired, and
//   over-limit magnitudes are used as declared so the trajectory shows
//   their consequence). No charging-efficiency term exists anywhere in the
//   scenario or registry and NO value is invented: the lossless balance is
//   a recorded limitation, listed as an unknown dimension on every
//   evaluation.
// - **V2X IS OFF BY DEFAULT, DECIDED ONCE (§11.5)**: bidirectional
//   capability is enabled ONLY when the EV, the EVSE, the protocol, the
//   interconnection agreement, AND the market rules ALL support it — five
//   conditions, each with recorded per-vehicle evidence
//   ([`V2xEvidence`]). [`V2xConfig::disabled`] is the default; enabling
//   without all five conditions and their evidence is a construction
//   error. In the synthetic portfolio V2X stays disabled except for the
//   NAMED test fixture documented in the decision record
//   (`v2x-enabled-fixture-001`, all five conditions documented and
//   labelled synthetic). Any discharge setpoint while V2X is disabled is a
//   named `DischargeWithV2xDisabled` violation.
// - **DEPARTURE ENERGY IS A HARD CONSTRAINT, DECIDED ONCE**: a vehicle must
//   reach its required departure SOC by its departure instant in every
//   feasible schedule ([`DepartureEnergyPolicy::Hard`]). The declaration is
//   exposed on the model and on every evaluation so the P3.17 problem IR
//   and the P3.19 relaxation ledger can carry it: any relaxation is a
//   NAMED, REVIEWED decision recorded in the schedule — this module has no
//   relaxation variant to call.
// - **OPT-OUT REMOVES THE VEHICLE FROM THE FEASIBLE SET FROM THE OPT-OUT
//   INSTANT ONWARD** (start-inclusive interval semantics): from that
//   instant the vehicle contributes nothing to the dispatchable capacity,
//   appears in no active-vehicle list, and any setpoint for it is a named
//   `DispatchedAfterOptOut` violation. Capacity is never silently retained;
//   re-derivation of the envelope is P3.24's job — this model only removes
//   the vehicle from the feasible set at the opt-out instant.
// - **OCPP 2.1 [R19] SCOPE STATEMENT**: OCPP 2.1 includes expanded
//   smart-charging and V2X-related capabilities [R19]. This module cites
//   OCPP 2.1 ONLY for capability SCOPE (what the protocol category
//   contains) — NEVER as a claim that any EVSE, vehicle, or device in this
//   portfolio implements or conforms to OCPP 2.1; device and market
//   support must be verified (§11.5). Nothing here claims conformance with
//   OCPP or any other standard.
// - **Site transformer limit**: applied per interval to the fleet NET
//   import (Σ charge − discharge over active vehicles). No export-side
//   limit data exists; the absence is recorded as an unknown dimension.
// - **Charging priority** is declared metadata (larger value = higher
//   priority); ordering the charging among vehicles is a scheduler (P3.17)
//   formulation concern — the model records priority on every per-vehicle
//   evaluation, it does not enforce an order.
//
// Control status: pure domain rules for the simulation-only 0.x line. No I/O,
// no solver coupling, no device-write path; a violation-free evaluation makes
// a schedule a CANDIDATE with respect to asset constraints — feasibility,
// authorization, and dispatch remain Phase 4 gates (never implied here).

/// Evaluation tolerance in each value's own unit (MW, MWh, SOC fraction):
/// comparisons are boundary-inclusive within this epsilon — a departure SOC
/// of exactly the required value and a fleet import of exactly the
/// transformer limit are feasible. Part of the module's API contract; the
/// P3.21 residual check reuses this constant rather than inventing its own.
pub const EV_FLEET_EVALUATION_TOLERANCE: f64 = 1e-9;

/// The recorded V2X default-state statement (decided once, §11.5).
pub const V2X_DISABLED_BY_DEFAULT_REASON: &str =
    "V2X is OFF by default (§11.5): bidirectional capability is enabled only \
when the EV, the EVSE, the protocol, the interconnection agreement, AND the \
market rules all support it. In the synthetic portfolio V2X stays disabled \
except for the named test fixture with all five conditions documented \
(decision record 09_11_load_ev_models.md).";

/// The recorded departure-energy policy statement (decided once).
pub const DEPARTURE_ENERGY_POLICY_STATEMENT: &str =
    "Departure energy is a HARD constraint: a vehicle must reach its required \
departure SOC by its departure instant in every feasible schedule. Any \
relaxation is a named, reviewed decision recorded in the schedule (P3.19 \
consumes this policy; the problem IR carries the declaration).";

/// The recorded OCPP 2.1 [R19] scope statement (ledger P3.11 acceptance).
pub const OCPP_SCOPE_STATEMENT: &str =
    "OCPP 2.1 includes expanded smart-charging and V2X-related capabilities \
[R19]. It is cited here for CAPABILITY SCOPE ONLY — never as a claim that \
any EVSE, vehicle, or device in this portfolio implements or conforms to \
OCPP 2.1; device and market support must be verified (report §11.5). \
Nothing in this module claims conformance with OCPP or any other standard.";

// --- Errors -----------------------------------------------------------------

/// Errors returned by EV fleet model construction and evaluation. Each
/// variant names the offending field/value — never a bare "invalid input"
/// (P3.04 error-catalogue style).
#[derive(Debug, Clone, PartialEq)]
pub enum EvFleetModelError {
    /// A numeric parameter was NaN or infinite; model parameters must be
    /// finite.
    NonFiniteParameter {
        /// The offending parameter field.
        field: &'static str,
        /// The offending value.
        value: f64,
    },
    /// A magnitude that must be positive (rated power, battery capacity,
    /// transformer limit) was zero or negative.
    NonPositiveQuantity {
        /// The offending field.
        field: &'static str,
        /// The offending value.
        value: f64,
    },
    /// A magnitude that must be non-negative (power limits) was negative.
    NegativeMagnitude {
        /// The offending field.
        field: &'static str,
        /// The offending value.
        value: f64,
    },
    /// An SOC value was outside [0,1] (boundary-inclusive).
    SocOutOfRange {
        /// The offending field.
        field: &'static str,
        /// The offending value.
        value: f64,
    },
    /// The minimum charging power exceeded the vehicle's maximum charging
    /// power.
    MinChargingAboveMax {
        /// The offending vehicle key.
        vehicle_key: String,
        /// The declared minimum in MW.
        min_mw: f64,
        /// The declared maximum in MW.
        max_mw: f64,
    },
    /// A vehicle's maximum charging power exceeded the EVSE rated power.
    VehiclePowerAboveEvseRating {
        /// The offending vehicle key.
        vehicle_key: String,
        /// The declared vehicle maximum in MW.
        vehicle_max_mw: f64,
        /// The EVSE rated power in MW.
        evse_rated_mw: f64,
    },
    /// A vehicle key was empty or duplicated across the fleet.
    InvalidVehicleKey {
        /// The offending key.
        vehicle_key: String,
    },
    /// A connection window was inverted (start ≥ end).
    InvertedConnectionWindow {
        /// The offending vehicle key.
        vehicle_key: String,
        /// The offending window index.
        window_index: usize,
        /// The window start in UTC milliseconds.
        start_millis: i64,
        /// The window end in UTC milliseconds.
        end_millis: i64,
    },
    /// No connection window was declared for a vehicle; connection windows
    /// are required configuration (never "always connected" by omission).
    NoConnectionWindowDeclared {
        /// The offending vehicle key.
        vehicle_key: String,
    },
    /// The departure instant was not covered by any connection window
    /// (strictly after a window start and at or before its end).
    DepartureOutsideConnectionWindows {
        /// The offending vehicle key.
        vehicle_key: String,
        /// The departure instant in UTC milliseconds.
        departure_millis: i64,
    },
    /// The departure instant did not fall exactly on an interval boundary of
    /// the scheduling grid (grid semantics are interval-based; a
    /// mid-interval departure is a P3.17 formulation concern).
    DepartureNotOnIntervalBoundary {
        /// The offending vehicle key.
        vehicle_key: String,
        /// The departure instant in UTC milliseconds.
        departure_millis: i64,
    },
    /// The departure instant lay outside the horizon (before its start or
    /// after its end).
    DepartureOutsideHorizon {
        /// The offending vehicle key.
        vehicle_key: String,
        /// The departure instant in UTC milliseconds.
        departure_millis: i64,
    },
    /// The opt-out instant fell outside the horizon (before its start or at
    /// or after its end; a mid-horizon opt-out is the only enforceable one
    /// at interval resolution).
    OptOutOutsideHorizon {
        /// The offending vehicle key.
        vehicle_key: String,
        /// The opt-out instant in UTC milliseconds.
        opt_out_millis: i64,
    },
    /// The EVSE phase reference was empty; the phase is declared
    /// configuration (§11.5: "EVSE rated power and phase").
    EmptyEvsePhase,
    /// The fleet had no vehicles; an empty fleet is a configuration error,
    /// not an empty pass-through.
    NoVehiclesDeclared,
    /// V2X enablement failed: the five §11.5 conditions were not all
    /// supported, or an evidence string was empty.
    V2xEnablementIncomplete {
        /// The recorded reason (audit-safe).
        reason: String,
    },
    /// The schedule's per-vehicle setpoint arrays did not match the model's
    /// interval count.
    ScheduleLengthMismatch {
        /// The offending vehicle key.
        vehicle_key: String,
        /// Number of setpoints supplied.
        setpoints: usize,
        /// Number of intervals in the horizon.
        intervals: usize,
    },
    /// A vehicle key in the schedule had no counterpart in the model
    /// configuration.
    UnknownScheduleVehicle {
        /// The unknown key.
        vehicle_key: String,
    },
    /// A setpoint value was NaN or infinite.
    NonFiniteSetpoint {
        /// The vehicle key.
        vehicle_key: String,
        /// The interval index.
        interval: u32,
        /// The offending field.
        field: &'static str,
        /// The offending value.
        value: f64,
    },
}

impl std::fmt::Display for EvFleetModelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EvFleetModelError::NonFiniteParameter { field, value } => {
                write!(f, "parameter '{field}' must be finite (found {value})")
            }
            EvFleetModelError::NonPositiveQuantity { field, value } => {
                write!(f, "parameter '{field}' must be strictly positive (found {value})")
            }
            EvFleetModelError::NegativeMagnitude { field, value } => {
                write!(f, "parameter '{field}' must be non-negative (found {value})")
            }
            EvFleetModelError::SocOutOfRange { field, value } => {
                write!(f, "parameter '{field}' must be an SOC fraction in [0,1] (found {value})")
            }
            EvFleetModelError::MinChargingAboveMax { vehicle_key, min_mw, max_mw } => {
                write!(f, "vehicle '{vehicle_key}': minimum charging power {min_mw} MW exceeds the maximum {max_mw} MW")
            }
            EvFleetModelError::VehiclePowerAboveEvseRating { vehicle_key, vehicle_max_mw, evse_rated_mw } => {
                write!(f, "vehicle '{vehicle_key}': maximum charging power {vehicle_max_mw} MW exceeds the EVSE rating {evse_rated_mw} MW")
            }
            EvFleetModelError::InvalidVehicleKey { vehicle_key } => {
                write!(f, "vehicle key '{vehicle_key}' is empty or duplicated")
            }
            EvFleetModelError::InvertedConnectionWindow { vehicle_key, window_index, start_millis, end_millis } => {
                write!(
                    f,
                    "vehicle '{vehicle_key}' window {window_index} is inverted (start {start_millis} >= end {end_millis} UTC ms)"
                )
            }
            EvFleetModelError::NoConnectionWindowDeclared { vehicle_key } => {
                write!(f, "vehicle '{vehicle_key}': at least one connection window must be declared")
            }
            EvFleetModelError::DepartureOutsideConnectionWindows { vehicle_key, departure_millis } => {
                write!(
                    f,
                    "vehicle '{vehicle_key}': departure instant {departure_millis} is not covered by any connection window"
                )
            }
            EvFleetModelError::DepartureNotOnIntervalBoundary { vehicle_key, departure_millis } => {
                write!(
                    f,
                    "vehicle '{vehicle_key}': departure instant {departure_millis} does not fall on an interval boundary of the scheduling grid"
                )
            }
            EvFleetModelError::DepartureOutsideHorizon { vehicle_key, departure_millis } => {
                write!(
                    f,
                    "vehicle '{vehicle_key}': departure instant {departure_millis} lies outside the scheduling horizon"
                )
            }
            EvFleetModelError::OptOutOutsideHorizon { vehicle_key, opt_out_millis } => {
                write!(
                    f,
                    "vehicle '{vehicle_key}': opt-out instant {opt_out_millis} lies outside the scheduling horizon"
                )
            }
            EvFleetModelError::EmptyEvsePhase => {
                write!(f, "the EVSE phase must be a declared non-empty reference (§11.5)")
            }
            EvFleetModelError::NoVehiclesDeclared => {
                write!(f, "the fleet must declare at least one vehicle")
            }
            EvFleetModelError::V2xEnablementIncomplete { reason } => {
                write!(f, "V2X enablement rejected: {reason}")
            }
            EvFleetModelError::ScheduleLengthMismatch { vehicle_key, setpoints, intervals } => {
                write!(f, "vehicle '{vehicle_key}': {setpoints} setpoints supplied but the horizon has {intervals} intervals")
            }
            EvFleetModelError::UnknownScheduleVehicle { vehicle_key } => {
                write!(f, "schedule contains vehicle '{vehicle_key}', which the fleet configuration does not declare")
            }
            EvFleetModelError::NonFiniteSetpoint { vehicle_key, interval, field, value } => {
                write!(f, "vehicle '{vehicle_key}' interval {interval}: setpoint '{field}' must be finite (found {value})")
            }
        }
    }
}

impl std::error::Error for EvFleetModelError {}

// --- V2X enablement (the five §11.5 conditions) --------------------------------

/// The per-vehicle evidence for ONE of the five §11.5 V2X conditions: the
/// condition must be supported AND the support must be traceable to a
/// recorded (non-empty) evidence string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct V2xConditionEvidence {
    /// Whether the condition is supported.
    pub supported: bool,
    /// The recorded evidence for the support (audit-safe; must be non-empty
    /// when `supported` is true).
    pub evidence: String,
}

/// The five §11.5 conditions for bidirectional (V2X) capability, each with
/// its per-vehicle evidence: the EV, the EVSE, the protocol, the
/// interconnection agreement, and the market rules must ALL support it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct V2xEvidence {
    /// The EV itself supports bidirectional power flow.
    pub ev: V2xConditionEvidence,
    /// The EVSE supports bidirectional power flow.
    pub evse: V2xConditionEvidence,
    /// The protocol in use supports the required bidirectional control
    /// (OCPP 2.1 is the scope reference for what this category contains
    /// [R19] — a scope citation, never a device-conformance claim).
    pub protocol: V2xConditionEvidence,
    /// The interconnection agreement permits export from the vehicle at the
    /// site.
    pub interconnection_agreement: V2xConditionEvidence,
    /// The applicable market rules permit the bidirectional service.
    pub market_rules: V2xConditionEvidence,
}

/// The bidirectional (V2X) configuration, DECIDED ONCE (§11.5): OFF by
/// default; enabled only with all five conditions evidenced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum V2xConfig {
    /// The default state: V2X off, with the recorded reason.
    DisabledByDefault,
    /// V2X enabled with all five §11.5 conditions evidenced per vehicle.
    Enabled {
        /// The recorded five-condition evidence.
        evidence: V2xEvidence,
    },
}

impl V2xConfig {
    /// The default: V2X disabled with the recorded reason.
    pub fn disabled() -> Self {
        Self::DisabledByDefault
    }

    /// Enables V2X. Rejected unless ALL five conditions are supported AND
    /// every evidence string is non-empty — an unevidenced enablement is a
    /// construction error, never a silent pass.
    pub fn enable(evidence: V2xEvidence) -> Result<Self, EvFleetModelError> {
        let conditions = [
            ("EV", &evidence.ev),
            ("EVSE", &evidence.evse),
            ("protocol", &evidence.protocol),
            ("interconnection agreement", &evidence.interconnection_agreement),
            ("market rules", &evidence.market_rules),
        ];
        for (name, condition) in conditions {
            if !condition.supported {
                return Err(EvFleetModelError::V2xEnablementIncomplete {
                    reason: format!(
                        "the {name} condition of the five §11.5 V2X conditions is not supported"
                    ),
                });
            }
            if condition.evidence.trim().is_empty() {
                return Err(EvFleetModelError::V2xEnablementIncomplete {
                    reason: format!("the {name} condition carries no recorded evidence"),
                });
            }
        }
        Ok(Self::Enabled { evidence })
    }

    /// Whether V2X is enabled.
    pub const fn is_enabled(&self) -> bool {
        matches!(self, Self::Enabled { .. })
    }
}

// --- The departure-energy policy (the hard-constraint declaration) --------------

/// The departure-energy policy, DECIDED ONCE: `Hard` is the ONLY variant.
/// The declaration is exposed so the P3.17 problem IR can carry it into the
/// optimization formulation and the P3.19 relaxation ledger can record any
/// named, reviewed relaxation against it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DepartureEnergyPolicy {
    /// A vehicle must reach its required departure SOC by its departure
    /// instant in every feasible schedule. No soft variant exists in this
    /// module.
    Hard,
}

impl DepartureEnergyPolicy {
    /// The recorded policy statement
    /// ([`DEPARTURE_ENERGY_POLICY_STATEMENT`]).
    pub const fn statement(self) -> &'static str {
        DEPARTURE_ENERGY_POLICY_STATEMENT
    }
}

// --- Configuration --------------------------------------------------------------

/// The EVSE configuration (report §11.5: "EVSE rated power and phase").
#[derive(Debug, Clone, PartialEq)]
pub struct EvseConfig {
    /// The EVSE rated power in MW (strictly positive).
    pub rated_power_mw: f64,
    /// The declared phase of the EVSE connection (a non-empty reference
    /// string; the P0.10 vocabulary does not freeze a phase enumeration, so
    /// none is invented).
    pub phase: String,
}

/// One vehicle of the fleet (report §11.5 vehicle-level fields).
#[derive(Debug, Clone, PartialEq)]
pub struct EvVehicleConfig {
    /// The vehicle key (unique, non-empty). The synthetic portfolio models
    /// one aggregate EV per EVSE; the key is the schedule-side reference.
    pub vehicle_key: String,
    /// The battery capacity in MWh (strictly positive).
    pub battery_capacity_mwh: f64,
    /// The SOC at horizon start, a fraction in [0,1].
    pub initial_soc: f64,
    /// The REQUIRED departure SOC, a fraction in [0,1] (the hard-constraint
    /// target).
    pub required_departure_soc: f64,
    /// The planned departure instant in UTC milliseconds (must fall on an
    /// interval boundary of the scheduling grid, inside the horizon, and be
    /// covered by a connection window).
    pub departure_millis: i64,
    /// Connection windows in UTC milliseconds, start-inclusive and
    /// end-exclusive (§10.3 interval semantics). Required: at least one.
    pub connection_windows_millis: Vec<(i64, i64)>,
    /// The vehicle's maximum charging power in MW (≤ the EVSE rated power).
    pub max_charging_power_mw: f64,
    /// The vehicle's minimum charging power in MW (0 declares "no
    /// minimum"; a charging setpoint between 0 and this value is a named
    /// violation).
    pub min_charging_power_mw: f64,
    /// The ramp limit in MW per minute, where configured; `None` records
    /// the dimension unknown (no ramp data exists in the scenario).
    pub ramp_limit_mw_per_minute: Option<f64>,
    /// The declared charging priority (larger value = higher priority;
    /// ordering is a P3.17 scheduler concern — recorded, not enforced).
    pub charging_priority: u32,
    /// The user override/opt-out instant in UTC milliseconds, when declared:
    /// from this instant (start-inclusive) the vehicle is REMOVED from the
    /// feasible set — it contributes no capacity and accepts no setpoint.
    pub opt_out_from_millis: Option<i64>,
}

/// The full EV fleet model configuration (report §11.5).
#[derive(Debug, Clone, PartialEq)]
pub struct EvFleetConfig {
    /// The EVSE configuration (rated power and phase).
    pub evse: EvseConfig,
    /// The vehicles of the fleet (at least one).
    pub vehicles: Vec<EvVehicleConfig>,
    /// The site transformer limit in MW (strictly positive), applied per
    /// interval to the fleet net import.
    pub site_transformer_limit_mw: f64,
    /// The V2X configuration — OFF by default (see the module header).
    pub v2x: V2xConfig,
    /// Interval length in seconds (§10.3 per-model configuration).
    pub interval_seconds: u32,
    /// The start-inclusive horizon start in UTC milliseconds (§10.3).
    pub horizon_start_millis: i64,
}

// --- Schedule types ----------------------------------------------------------------

/// One vehicle's setpoint for one interval: non-negative charge and
/// discharge magnitudes (the §11.2 convention). A discharge magnitude above
/// the tolerance while V2X is disabled is a named violation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EvSetpoint {
    /// Charging power in MW (a non-negative magnitude).
    pub charge_mw: f64,
    /// Discharging power in MW (a non-negative magnitude; must be 0 unless
    /// V2X is enabled).
    pub discharge_mw: f64,
}

impl EvSetpoint {
    /// A charging setpoint.
    pub fn charge(charge_mw: f64) -> Self {
        Self { charge_mw, discharge_mw: 0.0 }
    }

    /// A discharging (V2X) setpoint.
    pub fn discharge(discharge_mw: f64) -> Self {
        Self { charge_mw: 0.0, discharge_mw }
    }

    /// An idle setpoint.
    pub fn idle() -> Self {
        Self { charge_mw: 0.0, discharge_mw: 0.0 }
    }
}

/// A candidate EV fleet schedule: one setpoint per vehicle per interval.
#[derive(Debug, Clone, PartialEq)]
pub struct EvFleetSchedule {
    /// One setpoint array per vehicle, in the same order and with the same
    /// keys as the model configuration's vehicles.
    pub per_vehicle: Vec<(String, Vec<EvSetpoint>)>,
}

// --- Violations ---------------------------------------------------------------------

/// A named, structured EV fleet violation (REJECT policy: nothing is
/// clipped or silently repaired). Every variant names the vehicle or
/// interval, the offending value, and the limit.
#[derive(Debug, Clone, PartialEq)]
pub enum EvViolation {
    /// A negative setpoint magnitude (never reinterpreted as the opposite
    /// direction).
    NegativeMagnitude {
        /// The vehicle key.
        vehicle_key: String,
        /// The interval index.
        interval: u32,
        /// The offending field.
        field: &'static str,
        /// The offending value in MW.
        value_mw: f64,
    },
    /// A discharge setpoint while V2X is disabled (§11.5: bidirectional
    /// capability only when all five conditions support it).
    DischargeWithV2xDisabled {
        /// The vehicle key.
        vehicle_key: String,
        /// The interval index.
        interval: u32,
        /// The declared discharge magnitude in MW.
        discharge_mw: f64,
    },
    /// A setpoint outside every connection window. The violation is named;
    /// the energy balance applies no energy in the interval (a disconnected
    /// vehicle cannot exchange power — a physical gate, not a clip).
    ChargingOutsideConnectionWindow {
        /// The vehicle key.
        vehicle_key: String,
        /// The interval index.
        interval: u32,
    },
    /// A setpoint at or after the vehicle's opt-out instant. The violation
    /// is named; the energy balance applies no energy (the opt-out is a
    /// physical gate, not a clip).
    DispatchedAfterOptOut {
        /// The vehicle key.
        vehicle_key: String,
        /// The interval index.
        interval: u32,
        /// The opt-out instant in UTC milliseconds.
        opt_out_millis: i64,
    },
    /// A charging setpoint between zero and the declared minimum (inclusive
    /// of the minimum is feasible).
    BelowMinimumChargingPower {
        /// The vehicle key.
        vehicle_key: String,
        /// The interval index.
        interval: u32,
        /// The declared charge magnitude in MW.
        charge_mw: f64,
        /// The configured minimum in MW.
        min_mw: f64,
    },
    /// A charging setpoint above the effective limit (min of the EVSE
    /// rating and the vehicle maximum).
    AboveMaxChargingPower {
        /// The vehicle key.
        vehicle_key: String,
        /// The interval index.
        interval: u32,
        /// The declared charge magnitude in MW.
        charge_mw: f64,
        /// The effective limit in MW.
        limit_mw: f64,
    },
    /// A discharge setpoint above the effective limit (the EVSE rating; a
    /// vehicle maximum applies to charging only — recorded convention).
    AboveMaxDischargePower {
        /// The vehicle key.
        vehicle_key: String,
        /// The interval index.
        interval: u32,
        /// The declared discharge magnitude in MW.
        discharge_mw: f64,
        /// The effective limit in MW (the EVSE rating).
        limit_mw: f64,
    },
    /// The net-power change between consecutive intervals exceeded the
    /// configured ramp limit (first interval exempt — no prior setpoint
    /// exists in the model).
    RampViolation {
        /// The vehicle key.
        vehicle_key: String,
        /// The interval index of the interval whose change from its
        /// predecessor exceeded the limit.
        interval: u32,
        /// The absolute net-power change in MW.
        net_change_mw: f64,
        /// The per-interval ramp limit in MW.
        limit_mw_per_interval: f64,
    },
    /// The fleet net import exceeded the site transformer limit.
    SiteTransformerLimitExceeded {
        /// The interval index.
        interval: u32,
        /// The fleet net import in MW.
        fleet_import_mw: f64,
        /// The site transformer limit in MW.
        limit_mw: f64,
    },
    /// The vehicle's stored energy exceeded the battery capacity at a
    /// trajectory boundary.
    EnergyAboveCapacity {
        /// The vehicle key.
        vehicle_key: String,
        /// The trajectory boundary index (0 = initial state).
        boundary: u32,
        /// The energy at the boundary in MWh.
        energy_mwh: f64,
        /// The battery capacity in MWh.
        capacity_mwh: f64,
    },
    /// The vehicle's stored energy fell below zero at a trajectory boundary.
    EnergyBelowZero {
        /// The vehicle key.
        vehicle_key: String,
        /// The trajectory boundary index.
        boundary: u32,
        /// The energy at the boundary in MWh.
        energy_mwh: f64,
    },
    /// THE HARD DEPARTURE-ENERGY VIOLATION: the vehicle did not reach its
    /// required departure SOC by its departure instant. Departure energy is
    /// a hard constraint; the schedule is not repaired and the shortfall is
    /// reported (any relaxation is a named, reviewed decision recorded in
    /// the schedule — P3.19).
    DepartureEnergyNotMet {
        /// The vehicle key.
        vehicle_key: String,
        /// The required departure energy in MWh.
        required_mwh: f64,
        /// The achieved departure energy in MWh.
        achieved_mwh: f64,
    },
}

impl std::fmt::Display for EvViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EvViolation::NegativeMagnitude { vehicle_key, interval, field, value_mw } => {
                write!(f, "vehicle '{vehicle_key}' interval {interval}: setpoint '{field}' is {value_mw} MW; magnitudes must be non-negative")
            }
            EvViolation::DischargeWithV2xDisabled { vehicle_key, interval, discharge_mw } => {
                write!(f, "vehicle '{vehicle_key}' interval {interval}: discharge {discharge_mw} MW while V2X is disabled (§11.5: enabled only when all five conditions support it)")
            }
            EvViolation::ChargingOutsideConnectionWindow { vehicle_key, interval } => {
                write!(f, "vehicle '{vehicle_key}' interval {interval}: setpoint outside every connection window")
            }
            EvViolation::DispatchedAfterOptOut { vehicle_key, interval, opt_out_millis } => {
                write!(f, "vehicle '{vehicle_key}' interval {interval}: setpoint at or after the opt-out instant {opt_out_millis} (the vehicle left the feasible set)")
            }
            EvViolation::BelowMinimumChargingPower { vehicle_key, interval, charge_mw, min_mw } => {
                write!(f, "vehicle '{vehicle_key}' interval {interval}: charge {charge_mw} MW is below the configured minimum {min_mw} MW")
            }
            EvViolation::AboveMaxChargingPower { vehicle_key, interval, charge_mw, limit_mw } => {
                write!(f, "vehicle '{vehicle_key}' interval {interval}: charge {charge_mw} MW exceeds the effective limit {limit_mw} MW")
            }
            EvViolation::AboveMaxDischargePower { vehicle_key, interval, discharge_mw, limit_mw } => {
                write!(f, "vehicle '{vehicle_key}' interval {interval}: discharge {discharge_mw} MW exceeds the effective limit {limit_mw} MW")
            }
            EvViolation::RampViolation { vehicle_key, interval, net_change_mw, limit_mw_per_interval } => {
                write!(f, "vehicle '{vehicle_key}' interval {interval}: net-power change {net_change_mw} MW exceeds the per-interval ramp limit {limit_mw_per_interval} MW")
            }
            EvViolation::SiteTransformerLimitExceeded { interval, fleet_import_mw, limit_mw } => {
                write!(f, "interval {interval}: fleet net import {fleet_import_mw} MW exceeds the site transformer limit {limit_mw} MW")
            }
            EvViolation::EnergyAboveCapacity { vehicle_key, boundary, energy_mwh, capacity_mwh } => {
                write!(f, "vehicle '{vehicle_key}' boundary {boundary}: energy {energy_mwh} MWh exceeds the battery capacity {capacity_mwh} MWh")
            }
            EvViolation::EnergyBelowZero { vehicle_key, boundary, energy_mwh } => {
                write!(f, "vehicle '{vehicle_key}' boundary {boundary}: energy {energy_mwh} MWh is below zero")
            }
            EvViolation::DepartureEnergyNotMet { vehicle_key, required_mwh, achieved_mwh } => {
                write!(f, "vehicle '{vehicle_key}': departure energy {achieved_mwh} MWh is below the required {required_mwh} MWh (HARD constraint, not relaxed)")
            }
        }
    }
}

/// One dimension recorded as unknown on an evaluation (P3.02: unknown is
/// visible, never silently treated as unconstrained or filled with invented
/// values).
#[derive(Debug, Clone, PartialEq)]
pub struct EvUnknownDimension {
    /// The dimension name (frozen vocabulary string).
    pub dimension: &'static str,
    /// The recorded explanation (audit-safe).
    pub note: String,
}

// --- Evaluation output ----------------------------------------------------------------

/// The per-vehicle evaluation record.
#[derive(Debug, Clone, PartialEq)]
pub struct EvVehicleEvaluation {
    /// The vehicle key.
    pub vehicle_key: String,
    /// The stored-energy trajectory at every horizon boundary (interval
    /// count + 1 entries; entry 0 is the initial state).
    pub energy_trajectory_mwh: Vec<f64>,
    /// The SOC at the departure instant (a fraction in [0,1]).
    pub soc_at_departure: f64,
    /// The required departure SOC (a fraction in [0,1]).
    pub required_departure_soc: f64,
    /// Whether the hard departure-energy constraint is met at the departure
    /// instant (boundary-inclusive within the tolerance).
    pub departure_energy_met: bool,
    /// The declared charging priority (recorded; ordering is a P3.17
    /// concern).
    pub charging_priority: u32,
    /// The opt-out instant, when declared.
    pub opt_out_from_millis: Option<i64>,
}

/// The evaluation of a candidate EV fleet schedule.
#[derive(Debug, Clone, PartialEq)]
pub struct EvFleetEvaluation {
    /// The fleet net active power per interval in MW under the frozen
    /// `P > 0 = export` rule: charging is NEGATIVE net export.
    pub fleet_net_export_mw: Vec<f64>,
    /// The fleet net import per interval in MW (Σ effective charge −
    /// effective discharge over active vehicles, under the same physical
    /// gates as the energy balance: negative magnitudes and V2X-disabled
    /// discharges contribute zero; over-limit magnitudes are used as
    /// declared — nothing is clipped).
    pub fleet_import_mw: Vec<f64>,
    /// The active-vehicle keys per interval (connected AND not opted out).
    /// A vehicle opted out mid-horizon disappears from the feasible set
    /// FROM THE OPT-OUT INSTANT onward — capacity is not silently retained.
    pub active_vehicle_keys_per_interval: Vec<Vec<String>>,
    /// The dispatchable charging capacity per interval in MW (Σ effective
    /// maximum charging power over the active vehicles). This is the
    /// feasible-set capacity input the P3.12 envelope consumes; a mid-
    /// horizon opt-out lowers it from the opt-out instant onward (the
    /// re-derivation of the envelope itself is P3.24's job).
    pub dispatchable_capacity_mw: Vec<f64>,
    /// Whether V2X was enabled for this evaluation.
    pub v2x_enabled: bool,
    /// The declared departure-energy policy (the hard-constraint
    /// declaration the problem IR carries).
    pub departure_policy: DepartureEnergyPolicy,
    /// The recorded departure-policy statement.
    pub departure_policy_statement: &'static str,
    /// The recorded V2X default-state statement (present regardless of the
    /// configured state — the default is part of the record).
    pub v2x_default_statement: &'static str,
    /// The per-vehicle evaluations (configuration order).
    pub per_vehicle: Vec<EvVehicleEvaluation>,
    /// All named violations; empty = the schedule carries no violations.
    pub violations: Vec<EvViolation>,
    /// Dimensions recorded unknown for this evaluation (P3.02: visible,
    /// never silently unconstrained).
    pub unknown_dimensions: Vec<EvUnknownDimension>,
}

// --- The model ---------------------------------------------------------------------------

/// The EV fleet capability model (report §11.5): EVSE, vehicles, connection
/// windows, departure energy, priority, opt-out, transformer limit, and the
/// gated bidirectional (V2X) capability. Pure feasibility/limits layer.
#[derive(Debug, Clone, PartialEq)]
pub struct EvFleetModel {
    evse_rated_power_mw: f64,
    evse_phase: String,
    vehicles: Vec<VehicleRecord>,
    site_transformer_limit_mw: f64,
    v2x: V2xConfig,
    interval_seconds: u32,
    horizon_start_millis: i64,
    horizon_end_millis: i64,
}

/// The validated internal vehicle record.
#[derive(Debug, Clone, PartialEq)]
struct VehicleRecord {
    vehicle_key: String,
    battery_capacity_mwh: f64,
    initial_soc: f64,
    required_departure_soc: f64,
    departure_millis: i64,
    connection_windows_millis: Vec<(i64, i64)>,
    max_charging_power_mw: f64,
    min_charging_power_mw: f64,
    ramp_limit_mw_per_minute: Option<f64>,
    charging_priority: u32,
    opt_out_from_millis: Option<i64>,
}

fn require_finite(field: &'static str, value: f64) -> Result<(), EvFleetModelError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(EvFleetModelError::NonFiniteParameter { field, value })
    }
}

fn require_positive(field: &'static str, value: f64) -> Result<(), EvFleetModelError> {
    require_finite(field, value)?;
    if value <= 0.0 {
        return Err(EvFleetModelError::NonPositiveQuantity { field, value });
    }
    Ok(())
}

fn require_non_negative(field: &'static str, value: f64) -> Result<(), EvFleetModelError> {
    require_finite(field, value)?;
    if value < 0.0 {
        return Err(EvFleetModelError::NegativeMagnitude { field, value });
    }
    Ok(())
}

fn require_soc(field: &'static str, value: f64) -> Result<(), EvFleetModelError> {
    require_finite(field, value)?;
    if value < 0.0 || value > 1.0 {
        return Err(EvFleetModelError::SocOutOfRange { field, value });
    }
    Ok(())
}

impl EvFleetModel {
    /// Constructs and validates the fleet model: positive rated power,
    /// capacity, and transformer limit; SOC values in [0,1]; unique
    /// non-empty vehicle keys; at least one vehicle and connection window
    /// per vehicle; departure instants on interval boundaries, inside the
    /// horizon, and covered by a connection window; opt-out instants inside
    /// the horizon; V2X enabled only with all five §11.5 conditions
    /// evidenced.
    pub fn new(config: EvFleetConfig) -> Result<Self, EvFleetModelError> {
        require_positive("evse.rated_power_mw", config.evse.rated_power_mw)?;
        if config.evse.phase.trim().is_empty() {
            return Err(EvFleetModelError::EmptyEvsePhase);
        }
        require_positive("site_transformer_limit_mw", config.site_transformer_limit_mw)?;
        if config.interval_seconds == 0 {
            return Err(EvFleetModelError::NonFiniteParameter {
                field: "interval_seconds",
                value: 0.0,
            });
        }
        if config.vehicles.is_empty() {
            return Err(EvFleetModelError::NoVehiclesDeclared);
        }
        let interval_millis = i64::from(config.interval_seconds) * 1000;
        let horizon_start = config.horizon_start_millis;

        let mut vehicles: Vec<VehicleRecord> = Vec::with_capacity(config.vehicles.len());
        let mut seen_keys: Vec<&str> = Vec::new();
        for vehicle in &config.vehicles {
            if vehicle.vehicle_key.trim().is_empty()
                || seen_keys.contains(&vehicle.vehicle_key.as_str())
            {
                return Err(EvFleetModelError::InvalidVehicleKey {
                    vehicle_key: vehicle.vehicle_key.clone(),
                });
            }
            seen_keys.push(&vehicle.vehicle_key);
            require_positive("battery_capacity_mwh", vehicle.battery_capacity_mwh)?;
            require_soc("initial_soc", vehicle.initial_soc)?;
            require_soc("required_departure_soc", vehicle.required_departure_soc)?;
            require_non_negative("max_charging_power_mw", vehicle.max_charging_power_mw)?;
            require_non_negative("min_charging_power_mw", vehicle.min_charging_power_mw)?;
            if vehicle.min_charging_power_mw > vehicle.max_charging_power_mw + EV_FLEET_EVALUATION_TOLERANCE
            {
                return Err(EvFleetModelError::MinChargingAboveMax {
                    vehicle_key: vehicle.vehicle_key.clone(),
                    min_mw: vehicle.min_charging_power_mw,
                    max_mw: vehicle.max_charging_power_mw,
                });
            }
            if vehicle.max_charging_power_mw > config.evse.rated_power_mw + EV_FLEET_EVALUATION_TOLERANCE {
                return Err(EvFleetModelError::VehiclePowerAboveEvseRating {
                    vehicle_key: vehicle.vehicle_key.clone(),
                    vehicle_max_mw: vehicle.max_charging_power_mw,
                    evse_rated_mw: config.evse.rated_power_mw,
                });
            }
            if let Some(ramp) = vehicle.ramp_limit_mw_per_minute {
                require_non_negative("ramp_limit_mw_per_minute", ramp)?;
            }
            if vehicle.connection_windows_millis.is_empty() {
                return Err(EvFleetModelError::NoConnectionWindowDeclared {
                    vehicle_key: vehicle.vehicle_key.clone(),
                });
            }
            for (window_index, (start, end)) in
                vehicle.connection_windows_millis.iter().enumerate()
            {
                if start >= end {
                    return Err(EvFleetModelError::InvertedConnectionWindow {
                        vehicle_key: vehicle.vehicle_key.clone(),
                        window_index,
                        start_millis: *start,
                        end_millis: *end,
                    });
                }
            }
            // Departure: on an interval boundary, inside the horizon
            // [start, end], and covered by a connection window (strictly
            // after a window start, at or before its end).
            let offset = vehicle.departure_millis - horizon_start;
            if offset < 0 {
                return Err(EvFleetModelError::DepartureOutsideHorizon {
                    vehicle_key: vehicle.vehicle_key.clone(),
                    departure_millis: vehicle.departure_millis,
                });
            }
            if offset % interval_millis != 0 {
                return Err(EvFleetModelError::DepartureNotOnIntervalBoundary {
                    vehicle_key: vehicle.vehicle_key.clone(),
                    departure_millis: vehicle.departure_millis,
                });
            }
            let departure_covered = vehicle
                .connection_windows_millis
                .iter()
                .any(|(start, end)| {
                    vehicle.departure_millis > *start && vehicle.departure_millis <= *end
                });
            if !departure_covered {
                return Err(EvFleetModelError::DepartureOutsideConnectionWindows {
                    vehicle_key: vehicle.vehicle_key.clone(),
                    departure_millis: vehicle.departure_millis,
                });
            }
            if let Some(opt_out) = vehicle.opt_out_from_millis {
                if opt_out < horizon_start {
                    return Err(EvFleetModelError::OptOutOutsideHorizon {
                        vehicle_key: vehicle.vehicle_key.clone(),
                        opt_out_millis: opt_out,
                    });
                }
                // An opt-out exactly at the departure instant or later is a
                // no-op for the feasible set within the horizon; anything
                // between horizon start and departure is enforceable. The
                // upper bound is the largest departure in the fleet, checked
                // after the loop (see below).
            }
            vehicles.push(VehicleRecord {
                vehicle_key: vehicle.vehicle_key.clone(),
                battery_capacity_mwh: vehicle.battery_capacity_mwh,
                initial_soc: vehicle.initial_soc,
                required_departure_soc: vehicle.required_departure_soc,
                departure_millis: vehicle.departure_millis,
                connection_windows_millis: vehicle.connection_windows_millis.clone(),
                max_charging_power_mw: vehicle.max_charging_power_mw,
                min_charging_power_mw: vehicle.min_charging_power_mw,
                ramp_limit_mw_per_minute: vehicle.ramp_limit_mw_per_minute,
                charging_priority: vehicle.charging_priority,
                opt_out_from_millis: vehicle.opt_out_from_millis,
            });
        }
        // The horizon end is the LARGEST departure instant in the fleet (a
        // scheduling horizon must cover every vehicle's departure). All
        // opt-out instants must lie strictly before the horizon end.
        let horizon_end_millis = vehicles
            .iter()
            .map(|v| v.departure_millis)
            .max()
            .expect("at least one vehicle is declared");
        for vehicle in &vehicles {
            if let Some(opt_out) = vehicle.opt_out_from_millis {
                if opt_out >= horizon_end_millis {
                    return Err(EvFleetModelError::OptOutOutsideHorizon {
                        vehicle_key: vehicle.vehicle_key.clone(),
                        opt_out_millis: opt_out,
                    });
                }
            }
        }
        Ok(Self {
            evse_rated_power_mw: config.evse.rated_power_mw,
            evse_phase: config.evse.phase,
            vehicles,
            site_transformer_limit_mw: config.site_transformer_limit_mw,
            v2x: config.v2x,
            interval_seconds: config.interval_seconds,
            horizon_start_millis: horizon_start,
            horizon_end_millis,
        })
    }

    /// The EVSE rated power in MW.
    pub fn evse_rated_power_mw(&self) -> f64 {
        self.evse_rated_power_mw
    }

    /// The declared EVSE phase.
    pub fn evse_phase(&self) -> &str {
        &self.evse_phase
    }

    /// The site transformer limit in MW.
    pub fn site_transformer_limit_mw(&self) -> f64 {
        self.site_transformer_limit_mw
    }

    /// The V2X configuration.
    pub fn v2x(&self) -> &V2xConfig {
        &self.v2x
    }

    /// Whether V2X is enabled for this fleet.
    pub fn v2x_enabled(&self) -> bool {
        self.v2x.is_enabled()
    }

    /// The declared departure-energy policy (the hard-constraint
    /// declaration the problem IR carries).
    pub fn departure_policy(&self) -> DepartureEnergyPolicy {
        DepartureEnergyPolicy::Hard
    }

    /// The interval length in seconds.
    pub const fn interval_seconds(&self) -> u32 {
        self.interval_seconds
    }

    /// The start-inclusive horizon start in UTC milliseconds.
    pub const fn horizon_start_millis(&self) -> i64 {
        self.horizon_start_millis
    }

    /// The end-inclusive horizon end (the latest departure instant) in UTC
    /// milliseconds.
    pub const fn horizon_end_millis(&self) -> i64 {
        self.horizon_end_millis
    }

    /// The number of intervals in the horizon (derived from the grid).
    pub fn interval_count(&self) -> usize {
        ((self.horizon_end_millis - self.horizon_start_millis)
            / (i64::from(self.interval_seconds) * 1000)) as usize
    }

    /// The start-inclusive start of interval `index` in UTC milliseconds.
    pub fn interval_start_millis(&self, index: usize) -> i64 {
        self.horizon_start_millis
            + i64::from(self.interval_seconds) * 1000 * index as i64
    }

    /// The vehicle keys, in configuration order.
    pub fn vehicle_keys(&self) -> Vec<String> {
        self.vehicles.iter().map(|v| v.vehicle_key.clone()).collect()
    }

    /// Whether the vehicle is ACTIVE at an instant: connected AND not
    /// opted out (opt-out is start-inclusive: at the opt-out instant the
    /// vehicle is already out).
    pub fn vehicle_active_at(&self, vehicle_key: &str, instant_millis: i64) -> bool {
        let Some(record) = self.vehicles.iter().find(|v| v.vehicle_key == vehicle_key) else {
            return false;
        };
        if let Some(opt_out) = record.opt_out_from_millis {
            if instant_millis >= opt_out {
                return false;
            }
        }
        record
            .connection_windows_millis
            .iter()
            .any(|(start, end)| instant_millis >= *start && instant_millis < *end)
    }

    /// Evaluates a candidate schedule against the model. All applicable
    /// constraints are checked and every violation is reported (REJECT —
    /// nothing is clipped).
    pub fn evaluate_schedule(
        &self,
        schedule: &EvFleetSchedule,
    ) -> Result<EvFleetEvaluation, EvFleetModelError> {
        let interval_count = self.interval_count();
        let interval_millis = i64::from(self.interval_seconds) * 1000;
        let delta_hours = i64::from(self.interval_seconds) as f64 / 3600.0;
        let v2x_enabled = self.v2x.is_enabled();

        // --- schedule shape -------------------------------------------------
        let mut setpoints: Vec<Vec<EvSetpoint>> = Vec::with_capacity(self.vehicles.len());
        {
            let mut by_key: std::collections::HashMap<&str, &Vec<EvSetpoint>> =
                std::collections::HashMap::new();
            for (key, points) in &schedule.per_vehicle {
                if !self.vehicles.iter().any(|v| v.vehicle_key == *key) {
                    return Err(EvFleetModelError::UnknownScheduleVehicle {
                        vehicle_key: key.clone(),
                    });
                }
                if by_key.insert(key.as_str(), points).is_some() {
                    return Err(EvFleetModelError::InvalidVehicleKey {
                        vehicle_key: key.clone(),
                    });
                }
            }
            for record in &self.vehicles {
                let points = by_key.get(record.vehicle_key.as_str()).ok_or(
                    EvFleetModelError::UnknownScheduleVehicle {
                        vehicle_key: record.vehicle_key.clone(),
                    },
                )?;
                if points.len() != interval_count {
                    return Err(EvFleetModelError::ScheduleLengthMismatch {
                        vehicle_key: record.vehicle_key.clone(),
                        setpoints: points.len(),
                        intervals: interval_count,
                    });
                }
                setpoints.push((*points).clone());
            }
        }

        let mut violations: Vec<EvViolation> = Vec::new();
        let mut active_vehicle_keys_per_interval = Vec::with_capacity(interval_count);
        let mut dispatchable_capacity_mw = Vec::with_capacity(interval_count);
        let mut fleet_import_mw = Vec::with_capacity(interval_count);
        let mut per_vehicle: Vec<EvVehicleEvaluation> = Vec::with_capacity(self.vehicles.len());

        for (vehicle_index, record) in self.vehicles.iter().enumerate() {
            let points = &setpoints[vehicle_index];
            let capacity = record.battery_capacity_mwh;
            let mut trajectory = Vec::with_capacity(interval_count + 1);
            trajectory.push(record.initial_soc * capacity);
            // The net power of the previous interval for ramp checks (None =
            // first interval, exempt).
            let mut previous_net: Option<f64> = None;
            for interval in 0..interval_count {
                let setpoint = points[interval];
                if !setpoint.charge_mw.is_finite() {
                    return Err(EvFleetModelError::NonFiniteSetpoint {
                        vehicle_key: record.vehicle_key.clone(),
                        interval: interval as u32,
                        field: "charge_mw",
                        value: setpoint.charge_mw,
                    });
                }
                if !setpoint.discharge_mw.is_finite() {
                    return Err(EvFleetModelError::NonFiniteSetpoint {
                        vehicle_key: record.vehicle_key.clone(),
                        interval: interval as u32,
                        field: "discharge_mw",
                        value: setpoint.discharge_mw,
                    });
                }
                if setpoint.charge_mw < 0.0 {
                    violations.push(EvViolation::NegativeMagnitude {
                        vehicle_key: record.vehicle_key.clone(),
                        interval: interval as u32,
                        field: "charge_mw",
                        value_mw: setpoint.charge_mw,
                    });
                }
                if setpoint.discharge_mw < 0.0 {
                    violations.push(EvViolation::NegativeMagnitude {
                        vehicle_key: record.vehicle_key.clone(),
                        interval: interval as u32,
                        field: "discharge_mw",
                        value_mw: setpoint.discharge_mw,
                    });
                }
                let start_millis = self.interval_start_millis(interval);
                let connected = record
                    .connection_windows_millis
                    .iter()
                    .any(|(s, e)| start_millis >= *s && start_millis < *e);
                let opted_out = record
                    .opt_out_from_millis
                    .is_some_and(|opt_out| start_millis >= opt_out);
                let dispatching = setpoint.charge_mw > EV_FLEET_EVALUATION_TOLERANCE
                    || setpoint.discharge_mw > EV_FLEET_EVALUATION_TOLERANCE;
                if dispatching && opted_out {
                    violations.push(EvViolation::DispatchedAfterOptOut {
                        vehicle_key: record.vehicle_key.clone(),
                        interval: interval as u32,
                        opt_out_millis: record.opt_out_from_millis.unwrap_or_default(),
                    });
                }
                if dispatching && !connected {
                    violations.push(EvViolation::ChargingOutsideConnectionWindow {
                        vehicle_key: record.vehicle_key.clone(),
                        interval: interval as u32,
                    });
                }
                if setpoint.discharge_mw > EV_FLEET_EVALUATION_TOLERANCE && !v2x_enabled {
                    violations.push(EvViolation::DischargeWithV2xDisabled {
                        vehicle_key: record.vehicle_key.clone(),
                        interval: interval as u32,
                        discharge_mw: setpoint.discharge_mw,
                    });
                }
                if setpoint.discharge_mw > self.evse_rated_power_mw + EV_FLEET_EVALUATION_TOLERANCE
                {
                    violations.push(EvViolation::AboveMaxDischargePower {
                        vehicle_key: record.vehicle_key.clone(),
                        interval: interval as u32,
                        discharge_mw: setpoint.discharge_mw,
                        limit_mw: self.evse_rated_power_mw,
                    });
                }
                if setpoint.charge_mw > EV_FLEET_EVALUATION_TOLERANCE
                    && setpoint.charge_mw + EV_FLEET_EVALUATION_TOLERANCE
                        < record.min_charging_power_mw
                {
                    violations.push(EvViolation::BelowMinimumChargingPower {
                        vehicle_key: record.vehicle_key.clone(),
                        interval: interval as u32,
                        charge_mw: setpoint.charge_mw,
                        min_mw: record.min_charging_power_mw,
                    });
                }
                let effective_max = self.evse_rated_power_mw.min(record.max_charging_power_mw);
                if setpoint.charge_mw > effective_max + EV_FLEET_EVALUATION_TOLERANCE {
                    violations.push(EvViolation::AboveMaxChargingPower {
                        vehicle_key: record.vehicle_key.clone(),
                        interval: interval as u32,
                        charge_mw: setpoint.charge_mw,
                        limit_mw: effective_max,
                    });
                }
                let net = setpoint.charge_mw - setpoint.discharge_mw;
                if let Some(ramp) = record.ramp_limit_mw_per_minute {
                    if let Some(previous) = previous_net {
                        let limit_mw_per_interval =
                            ramp * (i64::from(self.interval_seconds) as f64 / 60.0);
                        let change = (net - previous).abs();
                        if change > limit_mw_per_interval + EV_FLEET_EVALUATION_TOLERANCE {
                            violations.push(EvViolation::RampViolation {
                                vehicle_key: record.vehicle_key.clone(),
                                interval: interval as u32,
                                net_change_mw: change,
                                limit_mw_per_interval,
                            });
                        }
                    }
                }
                previous_net = Some(net);
                // Energy balance gates (physical, never repairs): energy
                // flows ONLY while connected and not opted out; a NEGATIVE
                // magnitude (a violation) contributes zero; a discharge
                // magnitude while V2X is disabled contributes zero (the
                // hardware cannot discharge it). Every gate still reports
                // its named violation above — the balance never silently
                // repairs a setpoint, it only refuses impossible energy
                // flow.
                let energy = trajectory.last().copied().unwrap_or(0.0);
                let next = if connected && !opted_out {
                    let charge_effective = setpoint.charge_mw.max(0.0);
                    let discharge_effective =
                        if v2x_enabled { setpoint.discharge_mw.max(0.0) } else { 0.0 };
                    energy + (charge_effective - discharge_effective) * delta_hours
                } else {
                    energy
                };
                trajectory.push(next);
                if next > capacity + EV_FLEET_EVALUATION_TOLERANCE {
                    violations.push(EvViolation::EnergyAboveCapacity {
                        vehicle_key: record.vehicle_key.clone(),
                        boundary: (interval + 1) as u32,
                        energy_mwh: next,
                        capacity_mwh: capacity,
                    });
                }
                if next < -EV_FLEET_EVALUATION_TOLERANCE {
                    violations.push(EvViolation::EnergyBelowZero {
                        vehicle_key: record.vehicle_key.clone(),
                        boundary: (interval + 1) as u32,
                        energy_mwh: next,
                    });
                }
            }
            // The per-vehicle energy trajectory boundaries (interval_count
            // + 1 entries; entry 0 is the initial state) travel on the
            // per-vehicle evaluation below.
            let departure_boundary =
                ((record.departure_millis - self.horizon_start_millis) / interval_millis) as usize;
            let departure_energy = trajectory[departure_boundary];
            let required_energy = record.required_departure_soc * capacity;
            let departure_energy_met =
                departure_energy + EV_FLEET_EVALUATION_TOLERANCE >= required_energy;
            if !departure_energy_met {
                violations.push(EvViolation::DepartureEnergyNotMet {
                    vehicle_key: record.vehicle_key.clone(),
                    required_mwh: required_energy,
                    achieved_mwh: departure_energy,
                });
            }
            per_vehicle.push(EvVehicleEvaluation {
                vehicle_key: record.vehicle_key.clone(),
                energy_trajectory_mwh: trajectory,
                soc_at_departure: departure_energy / capacity,
                required_departure_soc: record.required_departure_soc,
                departure_energy_met,
                charging_priority: record.charging_priority,
                opt_out_from_millis: record.opt_out_from_millis,
            });
        }

        // --- per-interval fleet aggregation ---------------------------------
        // The import aggregation uses the SAME physical gates as the energy
        // balance: negative magnitudes and V2X-disabled discharges cannot
        // flow, so they contribute zero to the fleet import (their named
        // violations are still reported above). Over-limit magnitudes are
        // used as declared — the trajectory shows their consequence.
        for interval in 0..interval_count {
            let start_millis = self.interval_start_millis(interval);
            let mut active_keys: Vec<String> = Vec::new();
            let mut capacity = 0.0;
            let mut import = 0.0;
            for (vehicle_index, record) in self.vehicles.iter().enumerate() {
                let connected = record
                    .connection_windows_millis
                    .iter()
                    .any(|(s, e)| start_millis >= *s && start_millis < *e);
                let opted_out = record
                    .opt_out_from_millis
                    .is_some_and(|opt_out| start_millis >= opt_out);
                if connected && !opted_out {
                    active_keys.push(record.vehicle_key.clone());
                    capacity += self.evse_rated_power_mw.min(record.max_charging_power_mw);
                    let setpoint = setpoints[vehicle_index][interval];
                    let charge_effective = setpoint.charge_mw.max(0.0);
                    let discharge_effective =
                        if v2x_enabled { setpoint.discharge_mw.max(0.0) } else { 0.0 };
                    import += charge_effective - discharge_effective;
                }
            }
            active_vehicle_keys_per_interval.push(active_keys);
            dispatchable_capacity_mw.push(capacity);
            fleet_import_mw.push(import);
            if import > self.site_transformer_limit_mw + EV_FLEET_EVALUATION_TOLERANCE {
                violations.push(EvViolation::SiteTransformerLimitExceeded {
                    interval: interval as u32,
                    fleet_import_mw: import,
                    limit_mw: self.site_transformer_limit_mw,
                });
            }
        }
        let fleet_net_export_mw: Vec<f64> =
            fleet_import_mw.iter().map(|import| -import).collect();

        // --- recorded unknown dimensions -------------------------------------
        let unknown_dimensions = vec![
            EvUnknownDimension {
                dimension: "CHARGING_EFFICIENCY",
                note: "no charging-efficiency value exists in the scenario or \
registry; the energy balance applies P·Δt with no loss term (a recorded \
limitation — no value is invented; the SOC trajectory may overstate the \
delivered energy)"
                    .to_string(),
            },
            EvUnknownDimension {
                dimension: "SITE_EXPORT_LIMIT",
                note: "no export-side site limit data exists; the site \
transformer limit is applied to the fleet net import only — the absence is \
recorded, never silently unconstrained"
                    .to_string(),
            },
        ];
        // Ramp coverage: a vehicle without a configured ramp records the
        // dimension unknown.
        let unknown_dimensions = if self
            .vehicles
            .iter()
            .any(|v| v.ramp_limit_mw_per_minute.is_none())
        {
            let mut dimensions = unknown_dimensions;
            dimensions.push(EvUnknownDimension {
                dimension: "RAMP_RATE",
                note: "no ramp limit is configured for at least one vehicle; \
no ramp constraint is applied for it (the P3.02 unknown-narrows rule credits \
no ramp flexibility — recorded, never silent)"
                    .to_string(),
            });
            dimensions
        } else {
            unknown_dimensions
        };

        Ok(EvFleetEvaluation {
            fleet_net_export_mw,
            fleet_import_mw,
            active_vehicle_keys_per_interval,
            dispatchable_capacity_mw,
            v2x_enabled,
            departure_policy: self.departure_policy(),
            departure_policy_statement: DEPARTURE_ENERGY_POLICY_STATEMENT,
            v2x_default_statement: V2X_DISABLED_BY_DEFAULT_REASON,
            per_vehicle,
            violations,
            unknown_dimensions,
        })
    }
}
