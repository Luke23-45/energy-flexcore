// P3.05/P3.06/P3.07 — BESS capability model, degradation/reserve treatment,
// and edge-case behaviour (pure rules, no I/O).
//
// Implements ledger P3.05 ("BESS capability model"), P3.06 ("BESS degradation
// and reserve treatment") and P3.07 ("BESS edge cases"), report §11.2 (with
// the auxiliary-load term from foundations §7.2) and §12.1. Decision record:
// `ledger/phase-03-flexibility-scheduling/05_08_bess_pq_models.md`.
//
// THE MODEL, EXACTLY AS SPECIFIED (report §11.2; foundations §7.2):
//
//   P_net = P_dis − P_ch
//   E(t+1) = E(t) + η_ch·P_ch·Δt − P_dis/η_dis·Δt − P_aux·Δt
//   E_min ≤ E ≤ E_max
//   0 ≤ P_ch ≤ P_ch,max     0 ≤ P_dis ≤ P_dis,max
//
// - Units are canonical (§10.2): MW for powers, MWh for energy, Δt in hours
//   derived from the model's `interval_seconds` configuration. SOC is a
//   fraction in [0,1] at the API boundary, boundary-inclusive (§10.2).
// - **SOC ↔ energy mapping (decided once)**: the SOC fraction is defined over
//   the effective energy window, `SOC = (E − E_min)/(E_max − E_min)`. A
//   degenerate window (`E_min == E_max`) maps its single energy value to
//   SOC 0.0. The mapping is a declared convention, not physics.
// - **Auxiliary load (foundations §7.2)**: `−P_aux·Δt` is applied wherever
//   auxiliary data exists (per interval). Where it does NOT exist the term is
//   omitted and the omission is RECORDED in the evaluation provenance — never
//   silently zeroed, because a silent zero understates energy drain (ledger
//   P3.05 step 1). This is a recorded deviation from the P3.02
//   collapse-to-zero fallback: that fallback targets feasible-set BOUNDS,
//   while P_aux is an energy-balance term whose "most conservative" value is
//   not zero.
// - **Unknown provenance narrows (frozen P3.02 rule, reused, not
//   reimplemented)**: a static bound (energy window, charge/discharge power
//   limit) declared with UNKNOWN provenance is narrowed with
//   [`crate::capability_provenance::narrow_interval`] for
//   `AssetClass::Bess` — the collapse-to-zero fallback applies to the bound,
//   and an empty intersection makes the energy dimension infeasible (never
//   repaired by widening).
//
// SIMULTANEOUS CHARGE/DISCHARGE — DECIDED ONCE (report §11.2):
//
//   The domain model uses the **convex-relaxation path with a MANDATORY
//   post-solution check**: the relaxed per-interval feasible set admits
//   P_ch > 0 and P_dis > 0 simultaneously (that is what an LP relaxation
//   sees), and [`detect_simultaneous_charge_discharge`] is the single
//   mandatory post-solution detector that every schedule must be run through
//   — it runs automatically inside [`BessModel::evaluate_schedule`] and is
//   exposed publicly for the Tier 1 MILP path (flexcore-optimization-api,
//   P3.17 problem IR) and the P3.21 schedule validator. A binary mode-
//   variable formulation (u_ch + u_dis ≤ 1) is a Tier 1 formulation choice
//   belonging to P3.17; if used there, the model is a **MIP** and must never
//   be labelled a continuous SOCP (AGENTS.md rule 4). This module never
//   claims solver classes; it defines the feasibility/limits layer only.
//
// WHERE DATA IS AVAILABLE ONLY (report §11.2 "where data is available";
// ledger P3.05 step 3): SOC-dependent power limits, reserve margins,
// temperature derates, degradation cost, and BMS constraints enter ONLY as
// explicitly configured inputs with recorded provenance. Anything not
// configured is recorded as an unknown dimension on every evaluation output
// ([`BessEvaluationProvenance::unknown_dimensions`]) — never silently
// ignored, never filled with invented values (P3.02).
//
// DEGRADATION (P3.06): no verified degradation model is on record. The cost
// is a configurable LINEAR throughput term, default ZERO, and every output
// that carries it carries explicit "PLACEHOLDER, not a measured degradation
// law" metadata ([`DegradationMetadata`]) — visible in the type, not in a
// comment.
//
// RESERVE (P3.06): the reserve margin is a configurable SOC floor that
// narrows the usable energy window. Its values are configuration, never
// physics. An UNKNOWN-provenance floor narrows to the most conservative
// reading — no usable energy credited (floor = 1.0) — unknown never widens.
//
// EDGE POLICY — CLIP vs REJECT, DECIDED ONCE (ledger P3.07 step 2):
//
//   **REJECT.** A setpoint or trajectory that violates a bound produces a
//   named, structured [`BessViolation`]; the model never modifies (clips) a
//   declared setpoint and never silently repairs a trajectory. Rationale:
//   this layer evaluates CANDIDATE schedules produced by an optimizer;
//   silently clipping would mask solver errors and break the honesty
//   discipline (the recorded schedule must be the schedule that was
//   checked). The only sanctioned adjustment is computational: a NEGATIVE
//   magnitude (a violation) contributes zero to the energy balance — the
//   violation is still reported, so nothing is silent.
//
// TOLERANCE (ledger P3.07 risk note): all comparisons use
// [`BESS_EVALUATION_TOLERANCE`] (1e-9 in the value's own unit: MWh, MW, or
// SOC fraction). Boundary values are feasible: SOC exactly 0 and exactly 1,
// power exactly at a limit, energy exactly at E_min/E_max. The tolerance is
// part of this module's API contract; the P3.21 residual check re-verifies
// the trajectory with the same constant rather than inventing its own.
//
// Control status: pure domain rules for the simulation-only 0.x line. No I/O,
// no solver coupling, no device-write path; a violation-free evaluation makes
// the schedule a CANDIDATE with respect to asset constraints — feasibility,
// authorization, and dispatch remain Phase 4 gates (never implied here).

use flexcore_contracts::capability::Provenance;

use crate::capability_provenance::{AssetClass, BoundInterval, NarrowedSet, narrow_interval};

/// Evaluation tolerance in each value's own unit (MWh, MW, SOC fraction):
/// comparisons are boundary-inclusive within this epsilon (ledger P3.07
/// risk note — floating-point SOC drift must be absorbed by a STATED
/// tolerance, not by clipping). Part of the module's API contract; P3.21
/// reuses this constant for its residual check.
pub const BESS_EVALUATION_TOLERANCE: f64 = 1e-9;

/// The recorded placeholder statement carried on every degradation output
/// (ledger P3.06: "explicit 'placeholder, not validated' metadata on every
/// output that uses it"). Visible in the output type, not a comment.
pub const DEGRADATION_PLACEHOLDER_STATEMENT: &str =
    "PLACEHOLDER: configurable linear throughput term, not a measured \
degradation law. No verified degradation model is on record (ledger P3.06); \
the default rate is zero and any configured rate is an assumption, not \
validated physics.";

// --- Errors -----------------------------------------------------------------

/// Errors returned by BESS model construction and schedule-evaluation entry
/// points. Each variant names the offending field/value — never a bare
/// "invalid input" (P3.04 error-catalogue style).
#[derive(Debug, Clone, PartialEq)]
pub enum BessModelError {
    /// A numeric parameter was NaN or infinite; model parameters must be
    /// finite.
    NonFiniteParameter {
        /// The offending parameter field.
        field: &'static str,
        /// The offending value.
        value: f64,
    },
    /// A power limit was negative (§11.2: 0 ≤ P_ch, P_dis magnitudes).
    NegativePowerLimit {
        /// The offending parameter field.
        field: &'static str,
        /// The offending value.
        value: f64,
    },
    /// The energy window was negative (E_min < 0; stored energy is a
    /// non-negative MWh magnitude).
    NegativeEnergyBound {
        /// The offending parameter field.
        field: &'static str,
        /// The offending value.
        value: f64,
    },
    /// The raw energy window was inverted (E_min > E_max; §11.2 requires
    /// E_min ≤ E_max before any provenance narrowing).
    InvertedEnergyWindow {
        /// The declared E_min.
        e_min_mwh: f64,
        /// The declared E_max.
        e_max_mwh: f64,
    },
    /// An efficiency was outside (0, 1] (§11.2: efficiencies are fractions;
    /// zero is excluded because P_dis/η_dis would divide by zero).
    EfficiencyOutOfRange {
        /// The offending parameter field.
        field: &'static str,
        /// The offending value.
        value: f64,
    },
    /// The interval length was zero (Δt must be positive).
    NonPositiveIntervalSeconds,
    /// The schedule's initial SOC was outside [0,1] (boundary-inclusive).
    InitialSocOutOfRange {
        /// The offending value.
        value: f64,
    },
    /// The schedule and its per-interval side data had different lengths.
    ScheduleLengthMismatch {
        /// Number of setpoints.
        setpoints: usize,
        /// Number of side entries (auxiliary load or temperature derate).
        side_entries: usize,
        /// Which side data mismatched.
        side: &'static str,
    },
    /// A setpoint value was NaN or infinite.
    NonFiniteSetpoint {
        /// The interval index.
        interval: u32,
        /// The offending field.
        field: &'static str,
        /// The offending value.
        value: f64,
    },
    /// An auxiliary-load magnitude was negative or non-finite (P_aux ≥ 0).
    InvalidAuxiliaryLoad {
        /// The interval index.
        interval: u32,
        /// The offending value.
        value: f64,
    },
    /// The reserve floor was outside [0,1] or non-finite.
    InvalidReserveFloor {
        /// The offending value.
        value: f64,
    },
    /// The degradation rate was negative or non-finite (a cost rate ≥ 0).
    InvalidDegradationRate {
        /// The offending value.
        value: f64,
    },
    /// The SOC-power band table was structurally invalid.
    InvalidSocPowerBands(&'static str),
    /// A temperature derate value was negative or non-finite.
    InvalidTemperatureDerate {
        /// The offending value.
        value: f64,
    },
    /// The transition constraints were structurally invalid.
    InvalidTransitionConstraints(&'static str),
}

impl std::fmt::Display for BessModelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BessModelError::NonFiniteParameter { field, value } => {
                write!(f, "parameter '{field}' must be finite (found {value})")
            }
            BessModelError::NegativePowerLimit { field, value } => {
                write!(f, "power limit '{field}' must be >= 0 MW (found {value})")
            }
            BessModelError::NegativeEnergyBound { field, value } => {
                write!(f, "energy bound '{field}' must be >= 0 MWh (found {value})")
            }
            BessModelError::InvertedEnergyWindow { e_min_mwh, e_max_mwh } => {
                write!(
                    f,
                    "energy window must satisfy E_min <= E_max (found E_min = {e_min_mwh}, E_max = {e_max_mwh})"
                )
            }
            BessModelError::EfficiencyOutOfRange { field, value } => {
                write!(f, "efficiency '{field}' must be in (0, 1] (found {value})")
            }
            BessModelError::NonPositiveIntervalSeconds => {
                write!(f, "interval_seconds must be positive")
            }
            BessModelError::InitialSocOutOfRange { value } => {
                write!(f, "initial SOC must be a fraction in [0,1], boundary-inclusive (found {value})")
            }
            BessModelError::ScheduleLengthMismatch { setpoints, side_entries, side } => {
                write!(
                    f,
                    "per-interval {side} data has {side_entries} entries but the schedule declares {setpoints} setpoints"
                )
            }
            BessModelError::NonFiniteSetpoint { interval, field, value } => {
                write!(f, "setpoints[{interval}].{field} must be finite (found {value})")
            }
            BessModelError::InvalidAuxiliaryLoad { interval, value } => {
                write!(
                    f,
                    "auxiliary load at interval {interval} must be a finite magnitude >= 0 MW (found {value})"
                )
            }
            BessModelError::InvalidReserveFloor { value } => {
                write!(f, "reserve floor must be a fraction in [0,1] (found {value})")
            }
            BessModelError::InvalidDegradationRate { value } => {
                write!(f, "degradation rate must be a finite cost rate >= 0 (found {value})")
            }
            BessModelError::InvalidSocPowerBands(reason) => {
                write!(f, "SOC power band table is invalid: {reason}")
            }
            BessModelError::InvalidTemperatureDerate { value } => {
                write!(f, "temperature derate must be a finite magnitude >= 0 MW (found {value})")
            }
            BessModelError::InvalidTransitionConstraints(reason) => {
                write!(f, "transition constraints are invalid: {reason}")
            }
        }
    }
}

impl std::error::Error for BessModelError {}

// --- Configuration types -----------------------------------------------------

/// A numeric model parameter together with its declared provenance (§11.1:
/// adapters state which limits are measured, configured, inferred, or
/// unknown). UNKNOWN provenance narrows per the frozen P3.02 rule.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProvenancedValue {
    /// The declared value in its canonical unit.
    pub value: f64,
    /// The declared provenance of the value.
    pub provenance: Provenance,
}

impl ProvenancedValue {
    /// A configured (registry/device-profile/nameplate-sourced) value.
    pub fn configured(value: f64) -> Self {
        Self { value, provenance: Provenance::Configured }
    }

    /// A measured value.
    pub fn measured(value: f64) -> Self {
        Self { value, provenance: Provenance::Measured }
    }

    /// An unknown value: the P3.02 rule narrows the dimension it bounds.
    pub fn unknown() -> Self {
        Self { value: f64::NAN, provenance: Provenance::Unknown }
    }
}

/// One operating mode of a setpoint interval, derived with the evaluation
/// tolerance. A SIMULTANEOUS interval (both magnitudes above tolerance) is
/// reported as [`BessOperatingMode::Charge`] for dwell bookkeeping and is
/// always flagged separately as a [`BessViolation::SimultaneousChargeDischarge`]
/// violation — the mode derivation never hides simultaneity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BessOperatingMode {
    /// The interval charges (P_ch above tolerance).
    Charge,
    /// The interval discharges (P_dis above tolerance).
    Discharge,
    /// The interval is idle (both magnitudes at or below tolerance).
    Idle,
}

/// One interval's charge/discharge setpoint. Both fields are non-negative
/// magnitudes (§11.2); a negative value is a named violation, never
/// reinterpreted as the opposite direction.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BessSetpoint {
    /// Charge power in MW (a non-negative magnitude).
    pub p_ch_mw: f64,
    /// Discharge power in MW (a non-negative magnitude).
    pub p_dis_mw: f64,
}

impl BessSetpoint {
    /// A charge setpoint.
    pub fn charge(p_ch_mw: f64) -> Self {
        Self { p_ch_mw, p_dis_mw: 0.0 }
    }

    /// A discharge setpoint.
    pub fn discharge(p_dis_mw: f64) -> Self {
        Self { p_ch_mw: 0.0, p_dis_mw }
    }

    /// An idle setpoint.
    pub fn idle() -> Self {
        Self { p_ch_mw: 0.0, p_dis_mw: 0.0 }
    }
}

/// One interval's auxiliary-load datum (foundations §7.2 term `−P_aux·Δt`).
///
/// [`AuxLoadPoint::Known`] applies the term with its declared provenance.
/// [`AuxLoadPoint::Unknown`] records the absence: the term is omitted from
/// the balance (no value is invented) and the omission is visible in the
/// evaluation provenance ([`AuxLoadState::Unknown`]) — never a silent zero.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AuxLoadPoint {
    /// Auxiliary load known for this interval: a finite magnitude ≥ 0 MW.
    Known {
        /// Auxiliary load in MW.
        p_aux_mw: f64,
        /// Provenance of the auxiliary-load value.
        provenance: Provenance,
    },
    /// Auxiliary load unknown for this interval: recorded, never silently
    /// zeroed (ledger P3.05 step 1).
    Unknown,
}

/// A candidate BESS schedule over the model's interval grid: initial SOC and
/// one setpoint plus one auxiliary-load datum per interval.
#[derive(Debug, Clone, PartialEq)]
pub struct BessSchedule {
    /// Initial SOC as a fraction in [0,1], boundary-inclusive (§10.2).
    pub initial_soc: f64,
    /// One setpoint per interval of the horizon.
    pub setpoints: Vec<BessSetpoint>,
    /// One auxiliary-load datum per interval (same length as `setpoints`).
    pub auxiliary_load: Vec<AuxLoadPoint>,
}

/// SOC-dependent power-limit band: for SOC values at or below `soc_upper`
/// (and above the previous band's upper bound), the declared band limits
/// apply. `None` on a side means "no band limit on that side" (the base
/// model limit still applies). Band limits are CONFIGURATION from the
/// capability model's declared constraints — never invented values.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SocPowerBand {
    /// Inclusive upper SOC fraction of this band (bands sorted strictly
    /// ascending; the last band's upper bound must be 1.0 so every SOC in
    /// [0,1] resolves to exactly one band).
    pub soc_upper: f64,
    /// Charge-power limit in MW within this band, if declared.
    pub p_ch_max_mw: Option<f64>,
    /// Discharge-power limit in MW within this band, if declared.
    pub p_dis_max_mw: Option<f64>,
}

/// Configurable per-interval temperature derate (report §11.2 "temperature …
/// where data is available"): a non-negative MW magnitude subtracted from
/// both power limits in that interval. Present only where temperature data
/// exists; absent or UNKNOWN-provenance data applies NO derate (no credit is
/// taken from unknown data, per the frozen P3.02 `TEMPERATURE_HEALTH` row)
/// and the absence is recorded as an unknown dimension.
#[derive(Debug, Clone, PartialEq)]
pub struct TemperatureDerate {
    /// Provenance of the derate data.
    pub provenance: Provenance,
    /// One derate value in MW per interval (length checked at evaluation).
    pub derate_mw_per_interval: Vec<f64>,
}

/// Transition constraints applied where configured (ledger P3.07 step 2:
/// "mode transitions respect ramp/min-up-down where configured").
///
/// - `ramp_limit_mw_per_minute`: a RAMP_RATE configuration value (MW/min);
///   converted to a per-interval limit using the model's `interval_seconds`.
///   The first interval is exempt (no prior setpoint exists in the model).
/// - `min_mode_dwell_intervals`: minimum number of consecutive intervals for
///   a Charge or Discharge episode (the min-up/min-down analogue at interval
///   resolution). An episode truncated by the horizon end is exempt. Idle
///   episodes have no configured dwell (recorded limitation: the §11.1
///   minimum-up/down-time cross-interval MILP formulation is a P3.17
///   concern).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransitionConstraints {
    /// Ramp limit in MW per minute, if configured (≥ 0, finite).
    pub ramp_limit_mw_per_minute: Option<f64>,
    /// Minimum dwell in intervals for charge/discharge episodes, if
    /// configured (≥ 1).
    pub min_mode_dwell_intervals: Option<u32>,
}

/// Reserve-margin configuration (P3.06): a configurable SOC floor narrowing
/// the usable energy window. Values are configuration, never physics.
///
/// UNKNOWN provenance narrows to the most conservative reading: no usable
/// energy is credited (effective floor 1.0) — unknown never widens (P3.02).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReserveConfig {
    /// Reserve floor as an SOC fraction in [0,1]: usable energy stays at or
    /// above `E_min + floor·(E_max − E_min)`.
    pub reserve_floor_soc: f64,
    /// Provenance of the reserve configuration.
    pub provenance: Provenance,
}

/// Degradation-cost configuration (P3.06): a LINEAR throughput term — the
/// recorded placeholder, not a measured law. The rate is expressed in
/// generic cost units per MWh of throughput (charge + discharge); the cost
/// unit is a configuration choice of the §12.1 objective, not a canonical
/// platform unit. Default rate: zero.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DegradationConfig {
    /// Linear cost rate per MWh of (P_ch + P_dis)·Δt throughput, ≥ 0.
    pub linear_cost_per_mwh_throughput: f64,
    /// Provenance of the rate.
    pub provenance: Provenance,
}

/// The full BESS model configuration (report §11.2). Static bounds carry
/// provenance; unknown provenance narrows per the frozen P3.02 rule.
#[derive(Debug, Clone, PartialEq)]
pub struct BessModelConfig {
    /// Minimum stored energy in MWh (≥ 0) with provenance.
    pub e_min_mwh: ProvenancedValue,
    /// Maximum stored energy in MWh (≥ 0, ≥ E_min) with provenance.
    pub e_max_mwh: ProvenancedValue,
    /// Maximum charge power in MW (≥ 0) with provenance.
    pub p_ch_max_mw: ProvenancedValue,
    /// Maximum discharge power in MW (≥ 0) with provenance.
    pub p_dis_max_mw: ProvenancedValue,
    /// Charge efficiency η_ch ∈ (0, 1]. Required configuration — no default
    /// is invented (recorded gap: the registry holds no efficiency data).
    pub eta_ch: f64,
    /// Discharge efficiency η_dis ∈ (0, 1]. Required configuration — no
    /// default is invented.
    pub eta_dis: f64,
    /// Interval length in seconds (§10.3 per-model configuration; Δt in the
    /// balance is this value in hours).
    pub interval_seconds: u32,
    /// SOC-dependent power-limit bands, where configured; `None` records the
    /// dimension unknown.
    pub soc_power_bands: Option<Vec<SocPowerBand>>,
    /// Temperature derate, where data exists; `None` records the dimension
    /// unknown.
    pub temperature_derate: Option<TemperatureDerate>,
    /// Ramp / dwell constraints, where configured; `None` records the
    /// dimensions unknown.
    pub transitions: Option<TransitionConstraints>,
    /// Reserve floor, where configured; `None` records the dimension
    /// unknown.
    pub reserve: Option<ReserveConfig>,
    /// Degradation-cost configuration; `None` means the linear term is not
    /// configured (cost zero, placeholder flag false — nothing is used).
    pub degradation: Option<DegradationConfig>,
}

// --- Output types ------------------------------------------------------------

/// A named, structured BESS schedule violation (P3.07 REJECT policy: nothing
/// is clipped or silently repaired). Every variant names the interval or
/// boundary, the offending value, and the limit.
#[derive(Debug, Clone, PartialEq)]
pub enum BessViolation {
    /// The energy dimension is infeasible after provenance narrowing (empty
    /// declared ∩ default intersection). No schedule can satisfy it; never
    /// repaired by widening (P3.02).
    EnergyWindowInfeasible {
        /// The trajectory boundary index (0 = initial state).
        boundary: u32,
    },
    /// A trajectory boundary exceeded E_max (§11.2: E ≤ E_max).
    EnergyAboveMax {
        /// The trajectory boundary index.
        boundary: u32,
        /// The energy at the boundary in MWh.
        energy_mwh: f64,
        /// The effective E_max in MWh.
        e_max_mwh: f64,
    },
    /// A trajectory boundary fell below E_min (§11.2: E ≥ E_min).
    EnergyBelowMin {
        /// The trajectory boundary index.
        boundary: u32,
        /// The energy at the boundary in MWh.
        energy_mwh: f64,
        /// The effective E_min in MWh.
        e_min_mwh: f64,
    },
    /// A trajectory boundary fell below the configured reserve floor
    /// (P3.06: the reserve margin narrows the usable window).
    EnergyBelowReserveFloor {
        /// The trajectory boundary index.
        boundary: u32,
        /// The energy at the boundary in MWh.
        energy_mwh: f64,
        /// The usable-window floor in MWh implied by the reserve SOC.
        floor_mwh: f64,
    },
    /// A negative charge magnitude (§11.2: P_ch ≥ 0). The balance treats it
    /// as zero; the violation is still reported (never silent).
    NegativeChargePower {
        /// The interval index.
        interval: u32,
        /// The offending value in MW.
        p_ch_mw: f64,
    },
    /// A negative discharge magnitude (§11.2: P_dis ≥ 0).
    NegativeDischargePower {
        /// The interval index.
        interval: u32,
        /// The offending value in MW.
        p_dis_mw: f64,
    },
    /// Charge power above the effective limit for that interval (base limit
    /// ∩ SOC band ∩ temperature derate).
    ChargePowerAboveLimit {
        /// The interval index.
        interval: u32,
        /// The declared setpoint in MW (unclipped — the REJECT policy never
        /// modifies setpoints).
        p_ch_mw: f64,
        /// The effective limit in MW.
        limit_mw: f64,
    },
    /// Discharge power above the effective limit for that interval.
    DischargePowerAboveLimit {
        /// The interval index.
        interval: u32,
        /// The declared setpoint in MW.
        p_dis_mw: f64,
        /// The effective limit in MW.
        limit_mw: f64,
    },
    /// Simultaneous charge and discharge above the tolerance (§11.2: the
    /// mandatory post-solution check; the relaxed set admits it, the check
    /// flags it — see the module header decision).
    SimultaneousChargeDischarge {
        /// The interval index.
        interval: u32,
        /// The charge magnitude in MW.
        p_ch_mw: f64,
        /// The discharge magnitude in MW.
        p_dis_mw: f64,
    },
    /// The net-power change between consecutive intervals exceeded the
    /// configured ramp limit.
    RampViolation {
        /// The interval index (of the interval whose change from its
        /// predecessor exceeded the limit).
        interval: u32,
        /// The absolute net-power change in MW.
        net_change_mw: f64,
        /// The per-interval ramp limit in MW.
        limit_mw_per_interval: f64,
    },
    /// A charge/discharge episode shorter than the configured minimum dwell
    /// (and not truncated by the horizon end).
    DwellViolation {
        /// The interval index at which the episode ended.
        interval: u32,
        /// The mode of the too-short episode.
        mode: BessOperatingMode,
        /// The episode length in intervals.
        held_intervals: u32,
        /// The required minimum in intervals.
        required_intervals: u32,
    },
}

impl std::fmt::Display for BessViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BessViolation::EnergyWindowInfeasible { boundary } => write!(
                f,
                "boundary {boundary}: energy window is infeasible after unknown-provenance narrowing (empty intersection; never widened)"
            ),
            BessViolation::EnergyAboveMax { boundary, energy_mwh, e_max_mwh } => write!(
                f,
                "boundary {boundary}: energy {energy_mwh} MWh exceeds E_max {e_max_mwh} MWh"
            ),
            BessViolation::EnergyBelowMin { boundary, energy_mwh, e_min_mwh } => write!(
                f,
                "boundary {boundary}: energy {energy_mwh} MWh is below E_min {e_min_mwh} MWh"
            ),
            BessViolation::EnergyBelowReserveFloor { boundary, energy_mwh, floor_mwh } => write!(
                f,
                "boundary {boundary}: energy {energy_mwh} MWh is below the reserve floor {floor_mwh} MWh (configured reserve margin, not physics)"
            ),
            BessViolation::NegativeChargePower { interval, p_ch_mw } => {
                write!(f, "interval {interval}: charge power {p_ch_mw} MW is negative (P_ch >= 0, report §11.2)")
            }
            BessViolation::NegativeDischargePower { interval, p_dis_mw } => {
                write!(f, "interval {interval}: discharge power {p_dis_mw} MW is negative (P_dis >= 0, report §11.2)")
            }
            BessViolation::ChargePowerAboveLimit { interval, p_ch_mw, limit_mw } => {
                write!(f, "interval {interval}: charge power {p_ch_mw} MW exceeds the effective limit {limit_mw} MW")
            }
            BessViolation::DischargePowerAboveLimit { interval, p_dis_mw, limit_mw } => {
                write!(f, "interval {interval}: discharge power {p_dis_mw} MW exceeds the effective limit {limit_mw} MW")
            }
            BessViolation::SimultaneousChargeDischarge { interval, p_ch_mw, p_dis_mw } => write!(
                f,
                "interval {interval}: simultaneous charge {p_ch_mw} MW and discharge {p_dis_mw} MW (mandatory post-solution check, report §11.2)"
            ),
            BessViolation::RampViolation { interval, net_change_mw, limit_mw_per_interval } => {
                write!(
                    f,
                    "interval {interval}: net-power change {net_change_mw} MW exceeds the per-interval ramp limit {limit_mw_per_interval} MW"
                )
            }
            BessViolation::DwellViolation { interval, mode, held_intervals, required_intervals } => {
                write!(
                    f,
                    "interval {interval}: {mode:?} episode held {held_intervals} intervals, below the configured minimum {required_intervals}"
                )
            }
        }
    }
}

/// The placeholder metadata carried on EVERY evaluation output (P3.06
/// acceptance: "placeholder status is visible in output metadata").
#[derive(Debug, Clone, PartialEq)]
pub struct DegradationMetadata {
    /// True when a linear degradation term was configured and used.
    pub configured: bool,
    /// The configured linear rate (0.0 when not configured).
    pub linear_cost_per_mwh_throughput: f64,
    /// The schedule throughput in MWh: Σ (P_ch + P_dis)·Δt.
    pub throughput_mwh: f64,
    /// The computed placeholder cost: rate · throughput.
    pub cost: f64,
    /// True when the linear term was used — it is the recorded placeholder,
    /// never a measured law.
    pub is_placeholder: bool,
    /// The recorded placeholder statement ([`DEGRADATION_PLACEHOLDER_STATEMENT`]).
    pub placeholder_statement: &'static str,
}

/// The recorded state of the auxiliary-load data over the schedule
/// (foundations §7.2 term; ledger P3.05 step 1).
#[derive(Debug, Clone, PartialEq)]
pub enum AuxLoadState {
    /// Auxiliary load known for every interval; the balance applied
    /// `−P_aux·Δt` throughout. The recorded provenance is the WEAKEST
    /// per-interval provenance (never overstated).
    Known {
        /// The weakest per-interval provenance.
        provenance: Provenance,
    },
    /// Auxiliary load unknown for the listed intervals: the term was omitted
    /// there (no value invented) and the SOC trajectory may understate
    /// energy drain. Visible by construction — never a silent zero.
    Unknown {
        /// Interval indices where P_aux was unknown.
        intervals: Vec<u32>,
        /// The recorded explanation (audit-safe, no internal paths).
        note: String,
    },
}

/// Reserve-margin provenance on an evaluation output (P3.06).
#[derive(Debug, Clone, PartialEq)]
pub struct ReserveMetadata {
    /// The effective reserve floor as an SOC fraction (an UNKNOWN-provenance
    /// configuration narrows to 1.0 — no usable energy credited).
    pub reserve_floor_soc: f64,
    /// The usable-window floor in MWh implied by the floor.
    pub usable_min_mwh: f64,
    /// The declared provenance of the reserve configuration.
    pub provenance: Provenance,
}

/// One capability dimension recorded as unknown on an evaluation (P3.02:
/// unknown is visible, never silently treated as unconstrained or filled
/// with invented values).
#[derive(Debug, Clone, PartialEq)]
pub struct UnknownDimension {
    /// The dimension name (frozen vocabulary string).
    pub dimension: &'static str,
    /// The recorded explanation (audit-safe: no internal paths or secrets).
    pub note: String,
}

/// Static-parameter provenance of a constructed model (propagated, not
/// consumed — the P3.02 discipline).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BessParameterProvenance {
    /// The provenance of the effective energy window (the weaker of the two
    /// window bounds; UNKNOWN wins).
    pub energy_window: Provenance,
    /// The provenance of the charge-power limit.
    pub p_ch_max: Provenance,
    /// The provenance of the discharge-power limit.
    pub p_dis_max: Provenance,
}

/// The provenance record on a schedule evaluation: which parameters were
/// known, where auxiliary data was missing, what the reserve narrowing was,
/// and which dimensions stayed unknown.
#[derive(Debug, Clone, PartialEq)]
pub struct BessEvaluationProvenance {
    /// Always [`AssetClass::Bess`] (the P3.02 rule is applied per class).
    pub asset_class: AssetClass,
    /// Static-parameter provenance.
    pub parameters: BessParameterProvenance,
    /// The recorded auxiliary-load state over the schedule.
    pub auxiliary_load: AuxLoadState,
    /// Reserve metadata, when a reserve margin was configured (and the
    /// energy window was feasible).
    pub reserve: Option<ReserveMetadata>,
    /// Every capability dimension that had no data and was therefore NOT
    /// credited (P3.02: unknown is recorded, never silently unconstrained).
    pub unknown_dimensions: Vec<UnknownDimension>,
}

/// The result of evaluating a candidate BESS schedule against the §11.2
/// model: the SOC/energy trajectory, all named violations (empty = the
/// schedule satisfies the asset constraints within
/// [`BESS_EVALUATION_TOLERANCE`]), the placeholder degradation metadata, and
/// the provenance record.
///
/// A violation-free evaluation makes the schedule a CANDIDATE with respect
/// to asset constraints only — feasibility, authorization, and dispatch are
/// Phase 4 gates and are never implied here.
#[derive(Debug, Clone, PartialEq)]
pub struct BessScheduleEvaluation {
    /// Trajectory boundary energies in MWh: `n + 1` entries where `n` is the
    /// setpoint count; entry 0 is the initial state derived from
    /// `initial_soc`.
    pub interval_energies_mwh: Vec<f64>,
    /// Trajectory boundary SOCs as fractions in [0,1] (same indexing).
    pub interval_socs: Vec<f64>,
    /// All named violations, in evaluation order. Empty = candidate-feasible
    /// against the asset constraints.
    pub violations: Vec<BessViolation>,
    /// The degradation placeholder metadata (always present).
    pub degradation: DegradationMetadata,
    /// The provenance record.
    pub provenance: BessEvaluationProvenance,
}

// --- The model ---------------------------------------------------------------

/// The BESS capability model (report §11.2): the pure feasibility/limits
/// layer. Construct via [`BessModel::new`]; evaluate candidate schedules via
/// [`BessModel::evaluate_schedule`]. No solver, no I/O.
#[derive(Debug, Clone, PartialEq)]
pub struct BessModel {
    e_min_mwh: f64,
    e_max_mwh: f64,
    energy_feasible: bool,
    p_ch_max_mw: f64,
    p_dis_max_mw: f64,
    eta_ch: f64,
    eta_dis: f64,
    interval_seconds: u32,
    soc_power_bands: Option<Vec<SocPowerBand>>,
    temperature_derate: Option<TemperatureDerate>,
    transitions: Option<TransitionConstraints>,
    reserve: Option<ReserveConfig>,
    reserve_floor_effective: Option<f64>,
    usable_min_mwh: Option<f64>,
    degradation: Option<DegradationConfig>,
    parameter_provenance: BessParameterProvenance,
}

/// Merges two provenance values into the WEAKEST of the two, so a merged
/// record never overstates knowledge. Precedence (strongest first):
/// MEASURED > CONFIGURED > INFERRED > UNKNOWN.
fn weaker_provenance(a: Provenance, b: Provenance) -> Provenance {
    use Provenance as P;
    if a == P::Unknown || b == P::Unknown {
        return P::Unknown;
    }
    match (a, b) {
        (P::Inferred, _) | (_, P::Inferred) => P::Inferred,
        (P::Configured, _) | (_, P::Configured) => P::Configured,
        _ => P::Measured,
    }
}

fn require_finite(field: &'static str, value: f64) -> Result<(), BessModelError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(BessModelError::NonFiniteParameter { field, value })
    }
}

impl BessModel {
    /// Constructs and validates the BESS model from its configuration.
    ///
    /// Validation and provenance narrowing (frozen P3.02 rule, reused via
    /// [`narrow_interval`]):
    /// - raw values finite; `E_min ≥ 0`, `E_min ≤ E_max`; power limits ≥ 0;
    ///   efficiencies in (0, 1]; `interval_seconds > 0`;
    /// - if EITHER energy-window bound carries UNKNOWN provenance, the
    ///   window is narrowed against the recorded conservative default for
    ///   (`AssetClass::Bess`, `ENERGY_SOC`); an empty intersection marks the
    ///   energy dimension infeasible (never widened back);
    /// - an UNKNOWN-provenance power limit narrows to zero credit (the
    ///   recorded collapse-to-zero fallback).
    pub fn new(config: BessModelConfig) -> Result<Self, BessModelError> {
        // Raw structural validation.
        require_finite("e_min_mwh", config.e_min_mwh.value)?;
        require_finite("e_max_mwh", config.e_max_mwh.value)?;
        require_finite("p_ch_max_mw", config.p_ch_max_mw.value)?;
        require_finite("p_dis_max_mw", config.p_dis_max_mw.value)?;
        if config.e_min_mwh.value < 0.0 {
            return Err(BessModelError::NegativeEnergyBound {
                field: "e_min_mwh",
                value: config.e_min_mwh.value,
            });
        }
        if config.e_max_mwh.value < 0.0 {
            return Err(BessModelError::NegativeEnergyBound {
                field: "e_max_mwh",
                value: config.e_max_mwh.value,
            });
        }
        if config.e_min_mwh.value > config.e_max_mwh.value {
            return Err(BessModelError::InvertedEnergyWindow {
                e_min_mwh: config.e_min_mwh.value,
                e_max_mwh: config.e_max_mwh.value,
            });
        }
        if config.p_ch_max_mw.value < 0.0 {
            return Err(BessModelError::NegativePowerLimit {
                field: "p_ch_max_mw",
                value: config.p_ch_max_mw.value,
            });
        }
        if config.p_dis_max_mw.value < 0.0 {
            return Err(BessModelError::NegativePowerLimit {
                field: "p_dis_max_mw",
                value: config.p_dis_max_mw.value,
            });
        }
        for (field, eta) in [("eta_ch", config.eta_ch), ("eta_dis", config.eta_dis)] {
            if !eta.is_finite() || eta <= 0.0 || eta > 1.0 {
                return Err(BessModelError::EfficiencyOutOfRange { field, value: eta });
            }
        }
        if config.interval_seconds == 0 {
            return Err(BessModelError::NonPositiveIntervalSeconds);
        }

        // Provenance narrowing (frozen P3.02 rule; this module reuses the
        // single P3.02 implementation, it does not reimplement it).
        let energy_provenance =
            weaker_provenance(config.e_min_mwh.provenance, config.e_max_mwh.provenance);
        let declared_window =
            BoundInterval::new(Some(config.e_min_mwh.value), Some(config.e_max_mwh.value))
                .expect("raw energy window already validated as finite and ordered");
        let narrowed = narrow_interval(
            AssetClass::Bess,
            flexcore_contracts::capability::ConstraintKind::EnergySoc,
            energy_provenance,
            declared_window,
        );
        let (e_min_mwh, e_max_mwh, energy_feasible) = match narrowed {
            NarrowedSet::Interval { lower, upper } => (
                lower.unwrap_or(config.e_min_mwh.value),
                upper.unwrap_or(config.e_max_mwh.value),
                true,
            ),
            NarrowedSet::Empty => {
                // Infeasible, never repaired by widening (P3.02). The raw
                // declared values are retained for the record; every
                // evaluation reports the infeasibility.
                (config.e_min_mwh.value, config.e_max_mwh.value, false)
            }
        };
        let p_ch_max_mw = match config.p_ch_max_mw.provenance {
            Provenance::Unknown => 0.0, // collapse-to-zero: no charge credit
            _ => config.p_ch_max_mw.value,
        };
        let p_dis_max_mw = match config.p_dis_max_mw.provenance {
            Provenance::Unknown => 0.0, // collapse-to-zero: no discharge credit
            _ => config.p_dis_max_mw.value,
        };

        // Optional configuration validation.
        if let Some(bands) = &config.soc_power_bands {
            validate_soc_power_bands(bands)?;
        }
        if let Some(derate) = &config.temperature_derate {
            for value in &derate.derate_mw_per_interval {
                if !value.is_finite() || *value < 0.0 {
                    return Err(BessModelError::InvalidTemperatureDerate { value: *value });
                }
            }
        }
        if let Some(transitions) = &config.transitions {
            validate_transition_constraints(transitions)?;
        }
        let reserve_floor_effective = match &config.reserve {
            Some(reserve) => {
                if !reserve.reserve_floor_soc.is_finite()
                    || !(0.0..=1.0).contains(&reserve.reserve_floor_soc)
                {
                    return Err(BessModelError::InvalidReserveFloor {
                        value: reserve.reserve_floor_soc,
                    });
                }
                // UNKNOWN-provenance reserve: no usable energy credited
                // (unknown never widens — the most conservative floor).
                Some(match reserve.provenance {
                    Provenance::Unknown => 1.0,
                    _ => reserve.reserve_floor_soc,
                })
            }
            None => None,
        };
        if let Some(degradation) = &config.degradation {
            if !degradation.linear_cost_per_mwh_throughput.is_finite()
                || degradation.linear_cost_per_mwh_throughput < 0.0
            {
                return Err(BessModelError::InvalidDegradationRate {
                    value: degradation.linear_cost_per_mwh_throughput,
                });
            }
        }

        let usable_min_mwh = if energy_feasible {
            reserve_floor_effective.map(|floor| {
                e_min_mwh + floor * (e_max_mwh - e_min_mwh)
            })
        } else {
            None
        };

        Ok(Self {
            e_min_mwh,
            e_max_mwh,
            energy_feasible,
            p_ch_max_mw,
            p_dis_max_mw,
            eta_ch: config.eta_ch,
            eta_dis: config.eta_dis,
            interval_seconds: config.interval_seconds,
            soc_power_bands: config.soc_power_bands,
            temperature_derate: config.temperature_derate,
            transitions: config.transitions,
            reserve: config.reserve,
            reserve_floor_effective,
            usable_min_mwh,
            degradation: config.degradation,
            parameter_provenance: BessParameterProvenance {
                energy_window: energy_provenance,
                p_ch_max: config.p_ch_max_mw.provenance,
                p_dis_max: config.p_dis_max_mw.provenance,
            },
        })
    }

    /// The effective minimum stored energy in MWh (after provenance
    /// narrowing).
    pub fn e_min_mwh(&self) -> f64 {
        self.e_min_mwh
    }

    /// The effective maximum stored energy in MWh (after provenance
    /// narrowing).
    pub fn e_max_mwh(&self) -> f64 {
        self.e_max_mwh
    }

    /// Whether the energy dimension is feasible after narrowing (an empty
    /// declared ∩ default intersection leaves no feasible energy, P3.02).
    pub fn energy_feasible(&self) -> bool {
        self.energy_feasible
    }

    /// The effective charge-power limit in MW (after provenance narrowing).
    pub fn p_ch_max_mw(&self) -> f64 {
        self.p_ch_max_mw
    }

    /// The effective discharge-power limit in MW (after provenance
    /// narrowing).
    pub fn p_dis_max_mw(&self) -> f64 {
        self.p_dis_max_mw
    }

    /// The charge efficiency η_ch.
    pub fn eta_ch(&self) -> f64 {
        self.eta_ch
    }

    /// The discharge efficiency η_dis.
    pub fn eta_dis(&self) -> f64 {
        self.eta_dis
    }

    /// The interval length in seconds (configuration, §10.3).
    pub fn interval_seconds(&self) -> u32 {
        self.interval_seconds
    }

    /// The usable energy window in MWh after the reserve narrowing:
    /// `(usable_min, E_max)`, where `usable_min` is `E_min` when no reserve
    /// is configured and `E_min + floor·(E_max − E_min)` when it is. `None`
    /// when the energy dimension is infeasible after provenance narrowing.
    pub fn usable_energy_window(&self) -> Option<(f64, f64)> {
        if !self.energy_feasible {
            return None;
        }
        Some((self.usable_min_mwh.unwrap_or(self.e_min_mwh), self.e_max_mwh))
    }

    /// Maps an energy value to the declared SOC convention:
    /// `SOC = (E − E_min)/(E_max − E_min)`; a degenerate window maps its
    /// single energy value to 0.0. See the module header (decided once).
    pub fn energy_to_soc(&self, energy_mwh: f64) -> f64 {
        if !self.energy_feasible || self.e_max_mwh <= self.e_min_mwh {
            return 0.0;
        }
        (energy_mwh - self.e_min_mwh) / (self.e_max_mwh - self.e_min_mwh)
    }

    /// Maps a SOC fraction to energy under the declared convention:
    /// `E = E_min + SOC·(E_max − E_min)`.
    pub fn soc_to_energy(&self, soc: f64) -> f64 {
        self.e_min_mwh + soc * (self.e_max_mwh - self.e_min_mwh)
    }

    /// The mandatory post-solution simultaneous charge/discharge detector
    /// (§11.2 decision; ledger P3.05 step 2). Public so the Tier 1 MILP path
    /// (P3.17 problem IR) and the P3.21 schedule validator call THIS
    /// implementation instead of reimplementing it. Returns the indices of
    /// all intervals where both magnitudes exceed
    /// [`BESS_EVALUATION_TOLERANCE`].
    pub fn detect_simultaneous_charge_discharge(setpoints: &[BessSetpoint]) -> Vec<u32> {
        setpoints
            .iter()
            .enumerate()
            .filter(|(_, sp)| {
                sp.p_ch_mw > BESS_EVALUATION_TOLERANCE
                    && sp.p_dis_mw > BESS_EVALUATION_TOLERANCE
            })
            .map(|(index, _)| index as u32)
            .collect()
    }

    /// Derives the operating mode of one setpoint with the evaluation
    /// tolerance (see [`BessOperatingMode`] for the simultaneous case).
    pub fn mode_of_setpoint(setpoint: &BessSetpoint) -> BessOperatingMode {
        if setpoint.p_ch_mw > BESS_EVALUATION_TOLERANCE {
            BessOperatingMode::Charge
        } else if setpoint.p_dis_mw > BESS_EVALUATION_TOLERANCE {
            BessOperatingMode::Discharge
        } else {
            BessOperatingMode::Idle
        }
    }

    /// Evaluates a candidate schedule against the §11.2 model: builds the
    /// SOC/energy trajectory, applies every configured limit, runs the
    /// mandatory post-solution simultaneous charge/discharge detector, and
    /// returns all violations with the full provenance record.
    ///
    /// Policy (decided once): REJECT, never clip. Violations do not stop the
    /// evaluation — every violation is reported. A negative magnitude
    /// contributes zero to the balance (and is reported); an over-limit
    /// magnitude is used as declared so the trajectory shows its
    /// consequence.
    pub fn evaluate_schedule(
        &self,
        schedule: &BessSchedule,
    ) -> Result<BessScheduleEvaluation, BessModelError> {
        if schedule.auxiliary_load.len() != schedule.setpoints.len() {
            return Err(BessModelError::ScheduleLengthMismatch {
                setpoints: schedule.setpoints.len(),
                side_entries: schedule.auxiliary_load.len(),
                side: "auxiliary load",
            });
        }
        if let Some(derate) = &self.temperature_derate {
            if derate.derate_mw_per_interval.len() != schedule.setpoints.len() {
                return Err(BessModelError::ScheduleLengthMismatch {
                    setpoints: schedule.setpoints.len(),
                    side_entries: derate.derate_mw_per_interval.len(),
                    side: "temperature derate",
                });
            }
        }
        if !schedule.initial_soc.is_finite()
            || !(0.0..=1.0).contains(&schedule.initial_soc)
        {
            return Err(BessModelError::InitialSocOutOfRange {
                value: schedule.initial_soc,
            });
        }

        let dt_hours = f64::from(self.interval_seconds) / 3600.0;
        let mut energies = Vec::with_capacity(schedule.setpoints.len() + 1);
        let mut violations = Vec::new();
        let mut e = self.soc_to_energy(schedule.initial_soc);
        energies.push(e);
        self.check_boundary(0, e, &mut violations);

        let ramp_limit_per_interval = self.transitions.and_then(|t| t.ramp_limit_mw_per_minute)
            .map(|per_minute| per_minute * (f64::from(self.interval_seconds) / 60.0));
        let min_dwell = self.transitions.and_then(|t| t.min_mode_dwell_intervals);

        let mut prev_net: Option<f64> = None;
        let mut prev_mode: Option<BessOperatingMode> = None;
        let mut episode_start: u32 = 0;
        let mut throughput_mwh = 0.0;
        let mut aux_unknown_intervals: Vec<u32> = Vec::new();
        let mut aux_weakest_provenance: Option<Provenance> = None;

        for (position, setpoint) in schedule.setpoints.iter().enumerate() {
            let interval = position as u32;
            if !setpoint.p_ch_mw.is_finite() {
                return Err(BessModelError::NonFiniteSetpoint {
                    interval,
                    field: "p_ch_mw",
                    value: setpoint.p_ch_mw,
                });
            }
            if !setpoint.p_dis_mw.is_finite() {
                return Err(BessModelError::NonFiniteSetpoint {
                    interval,
                    field: "p_dis_mw",
                    value: setpoint.p_dis_mw,
                });
            }

            // Auxiliary load: applied where known, recorded where not.
            let p_aux_mw = match schedule.auxiliary_load[position] {
                AuxLoadPoint::Known { p_aux_mw, provenance } => {
                    if !p_aux_mw.is_finite() || p_aux_mw < 0.0 {
                        return Err(BessModelError::InvalidAuxiliaryLoad { interval, value: p_aux_mw });
                    }
                    aux_weakest_provenance = Some(match aux_weakest_provenance {
                        Some(prev) => weaker_provenance(prev, provenance),
                        None => provenance,
                    });
                    p_aux_mw
                }
                AuxLoadPoint::Unknown => {
                    aux_unknown_intervals.push(interval);
                    0.0 // omission recorded in provenance — never a silent zero
                }
            };

            // Effective per-interval power limits.
            let soc_t = self.energy_to_soc(e);
            let (ch_limit, dis_limit) = self.effective_power_limits(soc_t, position);

            // Magnitude and limit checks (REJECT policy: report, never clip).
            if setpoint.p_ch_mw < -BESS_EVALUATION_TOLERANCE {
                violations.push(BessViolation::NegativeChargePower {
                    interval,
                    p_ch_mw: setpoint.p_ch_mw,
                });
            }
            if setpoint.p_dis_mw < -BESS_EVALUATION_TOLERANCE {
                violations.push(BessViolation::NegativeDischargePower {
                    interval,
                    p_dis_mw: setpoint.p_dis_mw,
                });
            }
            if setpoint.p_ch_mw > ch_limit + BESS_EVALUATION_TOLERANCE {
                violations.push(BessViolation::ChargePowerAboveLimit {
                    interval,
                    p_ch_mw: setpoint.p_ch_mw,
                    limit_mw: ch_limit,
                });
            }
            if setpoint.p_dis_mw > dis_limit + BESS_EVALUATION_TOLERANCE {
                violations.push(BessViolation::DischargePowerAboveLimit {
                    interval,
                    p_dis_mw: setpoint.p_dis_mw,
                    limit_mw: dis_limit,
                });
            }

            // Mandatory post-solution simultaneous charge/discharge check.
            if setpoint.p_ch_mw > BESS_EVALUATION_TOLERANCE
                && setpoint.p_dis_mw > BESS_EVALUATION_TOLERANCE
            {
                violations.push(BessViolation::SimultaneousChargeDischarge {
                    interval,
                    p_ch_mw: setpoint.p_ch_mw,
                    p_dis_mw: setpoint.p_dis_mw,
                });
            }

            // Ramp check (first interval exempt: no prior setpoint exists).
            let net = setpoint.p_dis_mw - setpoint.p_ch_mw;
            if let (Some(limit), Some(previous)) = (ramp_limit_per_interval, prev_net) {
                let change = (net - previous).abs();
                if change > limit + BESS_EVALUATION_TOLERANCE {
                    violations.push(BessViolation::RampViolation {
                        interval,
                        net_change_mw: change,
                        limit_mw_per_interval: limit,
                    });
                }
            }
            prev_net = Some(net);

            // Dwell bookkeeping (charge/discharge episodes; horizon-truncated
            // episodes are exempt by construction: a change is only observed
            // while the horizon continues).
            let mode = Self::mode_of_setpoint(setpoint);
            if let Some(previous) = prev_mode {
                if mode != previous {
                    if let Some(required) = min_dwell {
                        let held = interval - episode_start;
                        let checked_mode = match previous {
                            BessOperatingMode::Idle => None,
                            other => Some(other),
                        };
                        if let Some(checked) = checked_mode {
                            if held < required {
                                violations.push(BessViolation::DwellViolation {
                                    interval,
                                    mode: checked,
                                    held_intervals: held,
                                    required_intervals: required,
                                });
                            }
                        }
                    }
                    episode_start = interval;
                }
            }
            prev_mode = Some(mode);

            // §11.2 discrete energy balance (foundations §7.2 aux term).
            // A negative magnitude contributes zero (reported above); an
            // over-limit magnitude is used as declared (REJECT policy).
            let p_ch = setpoint.p_ch_mw.max(0.0);
            let p_dis = setpoint.p_dis_mw.max(0.0);
            throughput_mwh += (p_ch + p_dis) * dt_hours;
            e += self.eta_ch * p_ch * dt_hours
                - p_dis / self.eta_dis * dt_hours
                - p_aux_mw * dt_hours;
            energies.push(e);
            self.check_boundary(interval + 1, e, &mut violations);
        }

        let degradation = match &self.degradation {
            Some(config) => DegradationMetadata {
                configured: true,
                linear_cost_per_mwh_throughput: config.linear_cost_per_mwh_throughput,
                throughput_mwh,
                cost: config.linear_cost_per_mwh_throughput * throughput_mwh,
                is_placeholder: true,
                placeholder_statement: DEGRADATION_PLACEHOLDER_STATEMENT,
            },
            None => DegradationMetadata {
                configured: false,
                linear_cost_per_mwh_throughput: 0.0,
                throughput_mwh,
                cost: 0.0,
                is_placeholder: false,
                placeholder_statement: DEGRADATION_PLACEHOLDER_STATEMENT,
            },
        };

        let auxiliary_load = if aux_unknown_intervals.is_empty() {
            AuxLoadState::Known {
                provenance: aux_weakest_provenance.unwrap_or(Provenance::Configured),
            }
        } else {
            AuxLoadState::Unknown {
                intervals: aux_unknown_intervals,
                note: "P_aux unknown for the listed intervals; the auxiliary term \
-P_aux·Δt is omitted from the energy balance there (no value is invented) and \
the SOC trajectory may understate energy drain (foundations §7.2; ledger P3.05)"
                    .to_string(),
            }
        };

        let reserve = match (self.reserve, self.usable_min_mwh) {
            (Some(config), Some(usable_min)) => Some(ReserveMetadata {
                reserve_floor_soc: self.reserve_floor_effective.unwrap_or(0.0),
                usable_min_mwh: usable_min,
                provenance: config.provenance,
            }),
            _ => None,
        };

        let provenance = BessEvaluationProvenance {
            asset_class: AssetClass::Bess,
            parameters: self.parameter_provenance,
            auxiliary_load,
            reserve,
            unknown_dimensions: self.unknown_dimensions(),
        };

        let socs = energies.iter().map(|&energy| self.energy_to_soc(energy)).collect();

        Ok(BessScheduleEvaluation {
            interval_energies_mwh: energies,
            interval_socs: socs,
            violations,
            degradation,
            provenance,
        })
    }

    // --- internals ------------------------------------------------------

    /// Effective per-interval power limits: base limit ∩ SOC band ∩
    /// temperature derate, floored at zero. Only configured data narrows;
    /// nothing is invented.
    fn effective_power_limits(&self, soc: f64, interval: usize) -> (f64, f64) {
        let mut ch = self.p_ch_max_mw;
        let mut dis = self.p_dis_max_mw;
        if let Some(bands) = &self.soc_power_bands {
            if let Some(band) = bands.iter().find(|band| soc <= band.soc_upper) {
                if let Some(limit) = band.p_ch_max_mw {
                    ch = ch.min(limit);
                }
                if let Some(limit) = band.p_dis_max_mw {
                    dis = dis.min(limit);
                }
            }
        }
        if let Some(derate) = &self.temperature_derate {
            // Unknown provenance takes no credit from unknown data (P3.02):
            // the derate is not applied and the absence is recorded as an
            // unknown dimension.
            if derate.provenance != Provenance::Unknown {
                if let Some(&value) = derate.derate_mw_per_interval.get(interval) {
                    ch = (ch - value).max(0.0);
                    dis = (dis - value).max(0.0);
                }
            }
        }
        (ch.max(0.0), dis.max(0.0))
    }

    /// Boundary checks: energy window (feasibility, E_min, E_max) and the
    /// reserve floor, all boundary-inclusive within the tolerance.
    fn check_boundary(&self, boundary: u32, energy_mwh: f64, violations: &mut Vec<BessViolation>) {
        if !self.energy_feasible {
            violations.push(BessViolation::EnergyWindowInfeasible { boundary });
            return;
        }
        if energy_mwh > self.e_max_mwh + BESS_EVALUATION_TOLERANCE {
            violations.push(BessViolation::EnergyAboveMax {
                boundary,
                energy_mwh,
                e_max_mwh: self.e_max_mwh,
            });
        }
        if energy_mwh < self.e_min_mwh - BESS_EVALUATION_TOLERANCE {
            violations.push(BessViolation::EnergyBelowMin {
                boundary,
                energy_mwh,
                e_min_mwh: self.e_min_mwh,
            });
        }
        if let Some(usable_min) = self.usable_min_mwh {
            if energy_mwh < usable_min - BESS_EVALUATION_TOLERANCE {
                violations.push(BessViolation::EnergyBelowReserveFloor {
                    boundary,
                    energy_mwh,
                    floor_mwh: usable_min,
                });
            }
        }
    }

    /// The dimensions recorded unknown for every evaluation of this model
    /// (P3.02: unknown is visible, never silently unconstrained).
    fn unknown_dimensions(&self) -> Vec<UnknownDimension> {
        let mut dimensions = Vec::new();
        if self.soc_power_bands.is_none() {
            dimensions.push(UnknownDimension {
                dimension: "SOC_DEPENDENT_POWER_LIMITS",
                note: "not configured; no SOC-dependent power limits are credited \
(unknown per P3.02 — no values invented)"
                    .to_string(),
            });
        }
        match &self.temperature_derate {
            None => dimensions.push(UnknownDimension {
                dimension: "TEMPERATURE_HEALTH",
                note: "no temperature data exists; no temperature derate is applied \
(unknown per P3.02 — no values invented)"
                    .to_string(),
            }),
            Some(derate) if derate.provenance == Provenance::Unknown => {
                dimensions.push(UnknownDimension {
                    dimension: "TEMPERATURE_HEALTH",
                    note: "temperature data carries UNKNOWN provenance; no derate is \
applied (no credit taken from unknown data, P3.02 TEMPERATURE_HEALTH row)"
                        .to_string(),
                });
            }
            Some(_) => {}
        }
        if self.reserve.is_none() {
            dimensions.push(UnknownDimension {
                dimension: "RESERVE_MARGIN",
                note: "no reserve margin configured; the full energy window is usable \
(configuration absence, recorded — not a physics claim)"
                    .to_string(),
            });
        }
        let ramp_configured = matches!(&self.transitions, Some(t) if t.ramp_limit_mw_per_minute.is_some());
        if !ramp_configured {
            dimensions.push(UnknownDimension {
                dimension: "RAMP_RATE",
                note: "no ramp limit configured; no inter-interval ramp constraint is \
enforced (unknown per P3.02 — no values invented)"
                    .to_string(),
            });
        }
        let dwell_configured =
            matches!(&self.transitions, Some(t) if t.min_mode_dwell_intervals.is_some());
        if !dwell_configured {
            dimensions.push(UnknownDimension {
                dimension: "MINIMUM_UP_DOWN_TIME",
                note: "no minimum dwell configured; mode transitions are not \
dwell-constrained (the §11.1 cross-interval MILP formulation is a P3.17 concern)"
                    .to_string(),
            });
        }
        dimensions.push(UnknownDimension {
            dimension: "BMS_CONSTRAINTS",
            note: "no BMS constraint data exists in the Phase 2 registry or the \
capability inputs; no BMS constraints are modelled (recorded gap, decision \
document §3)"
                .to_string(),
        });
        dimensions
    }
}

fn validate_soc_power_bands(bands: &[SocPowerBand]) -> Result<(), BessModelError> {
    if bands.is_empty() {
        return Err(BessModelError::InvalidSocPowerBands(
            "the band table must contain at least one band",
        ));
    }
    let mut previous: Option<f64> = None;
    for band in bands {
        if !band.soc_upper.is_finite() || band.soc_upper <= 0.0 || band.soc_upper > 1.0 {
            return Err(BessModelError::InvalidSocPowerBands(
                "each band's soc_upper must be a finite fraction in (0, 1]",
            ));
        }
        if let Some(prev) = previous {
            if band.soc_upper <= prev {
                return Err(BessModelError::InvalidSocPowerBands(
                    "band soc_upper values must be strictly ascending",
                ));
            }
        }
        for limit in [band.p_ch_max_mw, band.p_dis_max_mw] {
            if let Some(limit) = limit {
                if !limit.is_finite() || limit < 0.0 {
                    return Err(BessModelError::InvalidSocPowerBands(
                        "band power limits must be finite magnitudes >= 0",
                    ));
                }
            }
        }
        previous = Some(band.soc_upper);
    }
    if bands[bands.len() - 1].soc_upper < 1.0 {
        return Err(BessModelError::InvalidSocPowerBands(
            "the last band's soc_upper must be 1.0 so every SOC in [0,1] resolves to a band",
        ));
    }
    Ok(())
}

fn validate_transition_constraints(
    transitions: &TransitionConstraints,
) -> Result<(), BessModelError> {
    if let Some(ramp) = transitions.ramp_limit_mw_per_minute {
        if !ramp.is_finite() || ramp < 0.0 {
            return Err(BessModelError::InvalidTransitionConstraints(
                "the ramp limit must be a finite magnitude >= 0 MW per minute",
            ));
        }
    }
    if let Some(dwell) = transitions.min_mode_dwell_intervals {
        if dwell == 0 {
            return Err(BessModelError::InvalidTransitionConstraints(
                "the minimum dwell must be at least 1 interval",
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_config() -> BessModelConfig {
        BessModelConfig {
            e_min_mwh: ProvenancedValue::configured(0.0),
            e_max_mwh: ProvenancedValue::configured(8.0),
            p_ch_max_mw: ProvenancedValue::configured(5.0),
            p_dis_max_mw: ProvenancedValue::configured(5.0),
            eta_ch: 0.9,
            eta_dis: 0.9,
            interval_seconds: 3_600,
            soc_power_bands: None,
            temperature_derate: None,
            transitions: None,
            reserve: None,
            degradation: None,
        }
    }

    // --- construction ---------------------------------------------------

    #[test]
    fn base_model_constructs_and_exposes_effective_bounds() {
        let model = BessModel::new(base_config()).unwrap();
        assert_eq!(model.e_min_mwh(), 0.0);
        assert_eq!(model.e_max_mwh(), 8.0);
        assert_eq!(model.p_ch_max_mw(), 5.0);
        assert_eq!(model.p_dis_max_mw(), 5.0);
        assert!(model.energy_feasible());
        assert_eq!(model.usable_energy_window(), Some((0.0, 8.0)));
    }

    #[test]
    fn construction_rejects_structurally_invalid_configuration() {
        let mut config = base_config();
        config.eta_ch = 0.0;
        assert_eq!(
            BessModel::new(config),
            Err(BessModelError::EfficiencyOutOfRange { field: "eta_ch", value: 0.0 })
        );

        let mut config = base_config();
        config.eta_dis = 1.1;
        assert!(matches!(
            BessModel::new(config),
            Err(BessModelError::EfficiencyOutOfRange { field: "eta_dis", .. })
        ));

        let mut config = base_config();
        config.e_min_mwh = ProvenancedValue::configured(9.0);
        assert_eq!(
            BessModel::new(config),
            Err(BessModelError::InvertedEnergyWindow { e_min_mwh: 9.0, e_max_mwh: 8.0 })
        );

        let mut config = base_config();
        config.p_ch_max_mw = ProvenancedValue::configured(-1.0);
        assert_eq!(
            BessModel::new(config),
            Err(BessModelError::NegativePowerLimit { field: "p_ch_max_mw", value: -1.0 })
        );

        let mut config = base_config();
        config.e_min_mwh = ProvenancedValue::configured(-0.5);
        assert_eq!(
            BessModel::new(config),
            Err(BessModelError::NegativeEnergyBound { field: "e_min_mwh", value: -0.5 })
        );

        let mut config = base_config();
        config.interval_seconds = 0;
        assert_eq!(BessModel::new(config), Err(BessModelError::NonPositiveIntervalSeconds));

        let mut config = base_config();
        config.e_max_mwh = ProvenancedValue::configured(f64::NAN);
        assert!(matches!(
            BessModel::new(config),
            Err(BessModelError::NonFiniteParameter { field: "e_max_mwh", .. })
        ));
    }

    // --- provenance narrowing (frozen P3.02 rule) -------------------------

    #[test]
    fn unknown_provenance_power_limits_narrow_to_zero_credit() {
        let mut config = base_config();
        config.p_ch_max_mw = ProvenancedValue::unknown();
        config.p_dis_max_mw = ProvenancedValue::unknown();
        let model = BessModel::new(config).unwrap();
        assert_eq!(model.p_ch_max_mw(), 0.0);
        assert_eq!(model.p_dis_max_mw(), 0.0);
        assert_eq!(model.parameter_provenance.p_ch_max, Provenance::Unknown);
    }

    #[test]
    fn unknown_provenance_energy_window_narrows_and_can_be_infeasible() {
        // Window containing 0 narrows to the single point [0, 0].
        let mut config = base_config();
        config.e_min_mwh = ProvenancedValue::unknown();
        config.e_max_mwh = ProvenancedValue::unknown();
        let model = BessModel::new(config).unwrap();
        assert_eq!(model.e_max_mwh(), 0.0);
        assert!(model.energy_feasible());

        // Window excluding 0: the intersection is empty → infeasible, never
        // widened back (P3.02).
        let mut config = base_config();
        config.e_min_mwh = ProvenancedValue::configured(0.5);
        config.e_max_mwh = ProvenancedValue::unknown();
        let model = BessModel::new(config).unwrap();
        assert!(!model.energy_feasible());
        let schedule = BessSchedule {
            initial_soc: 0.0,
            setpoints: vec![BessSetpoint::idle()],
            auxiliary_load: vec![AuxLoadPoint::Unknown],
        };
        let evaluation = model.evaluate_schedule(&schedule).unwrap();
        assert_eq!(evaluation.violations.len(), 2);
        assert!(matches!(
            evaluation.violations[0],
            BessViolation::EnergyWindowInfeasible { boundary: 0 }
        ));
    }

    // --- helpers ----------------------------------------------------------

    #[test]
    fn detector_finds_exactly_the_simultaneous_intervals() {
        let setpoints = vec![
            BessSetpoint::charge(2.0),
            BessSetpoint::discharge(3.0),
            BessSetpoint::idle(),
            BessSetpoint { p_ch_mw: 1.0, p_dis_mw: 1.0 },
            BessSetpoint { p_ch_mw: BESS_EVALUATION_TOLERANCE, p_dis_mw: 1.0 },
        ];
        assert_eq!(
            BessModel::detect_simultaneous_charge_discharge(&setpoints),
            vec![3]
        );
    }

    #[test]
    fn mode_derivation_matches_the_tolerance() {
        assert_eq!(
            BessModel::mode_of_setpoint(&BessSetpoint::charge(0.5)),
            BessOperatingMode::Charge
        );
        assert_eq!(
            BessModel::mode_of_setpoint(&BessSetpoint::discharge(0.5)),
            BessOperatingMode::Discharge
        );
        assert_eq!(
            BessModel::mode_of_setpoint(&BessSetpoint::idle()),
            BessOperatingMode::Idle
        );
        // Simultaneity resolves to Charge for dwell bookkeeping; the
        // simultaneous case itself is flagged separately as a violation.
        assert_eq!(
            BessModel::mode_of_setpoint(&BessSetpoint { p_ch_mw: 1.0, p_dis_mw: 1.0 }),
            BessOperatingMode::Charge
        );
    }

    #[test]
    fn soc_mapping_follows_the_declared_window_convention() {
        let model = BessModel::new(base_config()).unwrap();
        assert_eq!(model.soc_to_energy(0.0), 0.0);
        assert_eq!(model.soc_to_energy(1.0), 8.0);
        assert_eq!(model.energy_to_soc(4.0), 0.5);
        // Degenerate window maps its single value to 0.0 (documented).
        let mut config = base_config();
        config.e_min_mwh = ProvenancedValue::configured(3.0);
        config.e_max_mwh = ProvenancedValue::configured(3.0);
        let degenerate = BessModel::new(config).unwrap();
        assert_eq!(degenerate.energy_to_soc(3.0), 0.0);
    }

    #[test]
    fn weaker_provenance_never_overstates_knowledge() {
        use Provenance as P;
        assert_eq!(weaker_provenance(P::Measured, P::Configured), P::Configured);
        assert_eq!(weaker_provenance(P::Measured, P::Inferred), P::Inferred);
        assert_eq!(weaker_provenance(P::Configured, P::Inferred), P::Inferred);
        assert_eq!(weaker_provenance(P::Measured, P::Unknown), P::Unknown);
        assert_eq!(weaker_provenance(P::Measured, P::Measured), P::Measured);
    }

    // --- reserve configuration ------------------------------------------

    #[test]
    fn reserve_floor_narrows_the_usable_energy_window() {
        let mut config = base_config();
        config.reserve = Some(ReserveConfig {
            reserve_floor_soc: 0.25,
            provenance: Provenance::Configured,
        });
        let model = BessModel::new(config).unwrap();
        // floor 0.25 of the window [0, 8] → usable from 2.0 MWh.
        assert_eq!(model.usable_energy_window(), Some((2.0, 8.0)));
    }

    #[test]
    fn unknown_provenance_reserve_floor_credits_no_usable_energy() {
        let mut config = base_config();
        config.reserve = Some(ReserveConfig {
            reserve_floor_soc: 0.25,
            provenance: Provenance::Unknown,
        });
        let model = BessModel::new(config).unwrap();
        assert_eq!(model.usable_energy_window(), Some((8.0, 8.0)));
    }

    #[test]
    fn invalid_reserve_floor_is_rejected() {
        let mut config = base_config();
        config.reserve = Some(ReserveConfig {
            reserve_floor_soc: 1.5,
            provenance: Provenance::Configured,
        });
        assert_eq!(
            BessModel::new(config),
            Err(BessModelError::InvalidReserveFloor { value: 1.5 })
        );
    }

    // --- optional-configuration validation --------------------------------

    #[test]
    fn invalid_soc_band_tables_are_rejected() {
        let mut config = base_config();
        config.soc_power_bands = Some(vec![]);
        assert!(matches!(
            BessModel::new(config),
            Err(BessModelError::InvalidSocPowerBands(_))
        ));

        let mut config = base_config();
        config.soc_power_bands = Some(vec![
            SocPowerBand { soc_upper: 0.5, p_ch_max_mw: None, p_dis_max_mw: None },
            SocPowerBand { soc_upper: 0.5, p_ch_max_mw: None, p_dis_max_mw: None },
        ]);
        assert!(matches!(
            BessModel::new(config),
            Err(BessModelError::InvalidSocPowerBands(_))
        ));

        let mut config = base_config();
        config.soc_power_bands = Some(vec![SocPowerBand {
            soc_upper: 0.9,
            p_ch_max_mw: None,
            p_dis_max_mw: None,
        }]);
        assert!(matches!(
            BessModel::new(config),
            Err(BessModelError::InvalidSocPowerBands(_))
        ));
    }

    #[test]
    fn invalid_transition_constraints_are_rejected() {
        let mut config = base_config();
        config.transitions = Some(TransitionConstraints {
            ramp_limit_mw_per_minute: Some(-1.0),
            min_mode_dwell_intervals: None,
        });
        assert!(matches!(
            BessModel::new(config),
            Err(BessModelError::InvalidTransitionConstraints(_))
        ));

        let mut config = base_config();
        config.transitions = Some(TransitionConstraints {
            ramp_limit_mw_per_minute: None,
            min_mode_dwell_intervals: Some(0),
        });
        assert!(matches!(
            BessModel::new(config),
            Err(BessModelError::InvalidTransitionConstraints(_))
        ));
    }

    #[test]
    fn invalid_degradation_rate_is_rejected() {
        let mut config = base_config();
        config.degradation = Some(DegradationConfig {
            linear_cost_per_mwh_throughput: -0.5,
            provenance: Provenance::Configured,
        });
        assert_eq!(
            BessModel::new(config),
            Err(BessModelError::InvalidDegradationRate { value: -0.5 })
        );
    }
}
