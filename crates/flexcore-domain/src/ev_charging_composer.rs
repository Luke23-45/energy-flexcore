// P17.05 — The smart-charging schedule composer: dispatch targets/envelopes
// → charging-profile plans (the dispatch path into chargers).
//
// Decision record:
// `docs/phases-11-20/phase-17-ev-fleet-charging/records/05_p17_05_smart_charging_composer.md`.
//
// The composer is PURE DOMAIN RULES: it turns a dispatch envelope (limits
// per interval in the canonical MW) into a `ChargingProfilePlan` — the
// domain-level payload shape the OCPP adapter
// (`crates/adapters/protocol-ocpp::charging`) maps onto a wire
// SetChargingProfile request. The adapter holds the wire types; the
// composer holds the RULES (the dependency rule: domain depends on nothing
// infrastructure).
//
// Module-level UNVERIFIED-ASSUMPTION register (phase-17 plan §7.5). Every
// OCPP smart-charging rule below is cited from secondary sources
// ([S-124] ocpp.md smart charging, [S-141] Ampcontrol OCPP guide) and
// names its implementation-time check against the OCA OCPP 2.0.1 Part 2
// (smart charging) text:
//
// 1. The four purposes (`ChargingStationExternalConstraints`,
//    `ChargingStationMaxProfile`, `TxDefaultProfile`, `TxProfile`) and
//    their spellings — register item 6 of the adapter's model.rs.
// 2. The composition PRECEDENCE order (external constraints first, then
//    station max, then Tx default, then Tx profile) and the within-purpose
//    stack-level rule (higher stack levels override lower ones). The order
//    recorded here is the SECONDARY-source order; the check is the OCA
//    text's "composition of charging profiles" section. A mismatch is a
//    recorded code/record edit, never a silent reinterpretation.
// 3. evseId 0 addresses the whole station.
// 4. `minChargingRate`: a schedule step with a positive limit BELOW the
//    schedule's minimum rate is NOT a valid alternative — it must be 0
//    (off). The composer enforces the pairing and records every
//    to-zero decision as a named record.
// 5. `isGridCritical` on the external-constraints purpose (a 2.1-era
//    field per secondary sources; 2.0.1 applicability is a check item).
// 6. The W/A rate units and the phases semantics (AC: amperes bind per
//    phase; the A↔W conversion uses the declared nominal voltage and the
//    declared phase count; DC has no phase factor). The exact formula
//    OCPP expects (whether `numberPhases` scales the amperes) is the
//    check item — the composer records its declared formula verbatim in
//    every conversion decision.
// 7. "Accepted" = STORED, not applied: the reconciliation against
//    GetCompositeSchedule is the application-evidence surface, and a
//    mismatch is a NAMED record ([`ReconciliationOutcome::Mismatch`]),
//    never a silent pass.
//
// Charging profiles are COMMANDS (phase-17 context §3 constraint 1). The
// plan is a payload CANDIDATE: it enters the command lifecycle at PROPOSED
// and passes the validation, feasibility, and authorization gates like any
// other candidate. The adapter may build the wire payload ONLY from an
// ISSUED lifecycle state (enforced there by typing); the composer itself
// never touches the lifecycle — it produces candidate shapes, not states.

/// The composer's tolerance for limit comparisons (in the wire unit; the
/// comparisons are boundary-inclusive within this epsilon — the P3.11
/// evaluation-tolerance discipline, wire-unit flavour).
pub const COMPOSER_EVALUATION_TOLERANCE: f64 = 1e-6;

/// The recorded composition-precedence statement (UNVERIFIED order —
/// module register item 2; the check is the OCA composition section).
pub const STACK_PRECEDENCE_STATEMENT: &str =
    "Recorded composition precedence (secondary sources, UNVERIFIED against \
     the OCA text): ChargingStationExternalConstraints first, then \
     ChargingStationMaxProfile, then TxDefaultProfile, then TxProfile; \
     within one purpose, higher stack levels override lower ones. The \
     GetCompositeSchedule reconciliation is the surface where the station's \
     ACTUAL composition is observed against this recorded order.";

/// The recorded stored-not-applied reconciliation statement (the honesty
/// rule, record 05 §6).
pub const RECONCILIATION_STATEMENT: &str =
    "Accepted means STORED, not applied: application evidence comes only \
     from the GetCompositeSchedule reconciliation. A mismatch surfaces as a \
     named ReconciliationOutcome::Mismatch record — never a silent pass and \
     never an adapter-side retry decision.";

/// The composer errors. Every failure names the offending field/value.
#[derive(Debug, Clone, PartialEq)]
pub enum ComposerError {
    /// An envelope with no periods — a profile that limits nothing is not
    /// a profile.
    EmptyPeriods,
    /// A numeric value was NaN or infinite.
    NonFiniteValue {
        field: &'static str,
        value: f64,
    },
    /// A limit magnitude was negative.
    NegativeLimit {
        field: &'static str,
        value: f64,
    },
    /// Period start offsets are not strictly ascending (duplicates are a
    /// composition defect, refused, never merged).
    NonAscendingPeriods {
        previous_start_s: u32,
        offending_start_s: u32,
    },
    /// The nominal voltage is missing or non-positive — no conversion is
    /// invented.
    InvalidVoltage {
        value: f64,
    },
    /// The declared phase count is not 1 or 3 for an AC supply.
    InvalidPhases {
        value: u8,
    },
    /// A Tx-class purpose was requested without a transaction id.
    TxProfileNeedsTransaction,
    /// A TxDefault/TxProfile purpose was requested at station scope
    /// (evseId 0) — those purposes are per-EVSE by definition (UNVERIFIED
    /// register item 3/6).
    StationScopePurpose {
        purpose: &'static str,
    },
    /// The stack level was negative.
    NegativeStackLevel {
        value: i32,
    },
    /// The declared minimum rate was negative or non-finite.
    InvalidMinRate {
        value: f64,
    },
}

impl std::fmt::Display for ComposerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ComposerError::EmptyPeriods => {
                write!(f, "the dispatch envelope has no periods; a profile that limits nothing is not a profile")
            }
            ComposerError::NonFiniteValue { field, value } => {
                write!(f, "value '{field}' must be finite (found {value})")
            }
            ComposerError::NegativeLimit { field, value } => {
                write!(f, "limit '{field}' must be non-negative (found {value})")
            }
            ComposerError::NonAscendingPeriods {
                previous_start_s,
                offending_start_s,
            } => {
                write!(f, "period start offsets are not strictly ascending ({previous_start_s} then {offending_start_s}); duplicates are refused, never merged")
            }
            ComposerError::InvalidVoltage { value } => {
                write!(f, "nominal voltage must be strictly positive (found {value}); no conversion is invented")
            }
            ComposerError::InvalidPhases { value } => {
                write!(f, "declared AC phase count must be 1 or 3 (found {value})")
            }
            ComposerError::TxProfileNeedsTransaction => {
                write!(f, "a TxProfile purpose requires a transaction id")
            }
            ComposerError::StationScopePurpose { purpose } => {
                write!(f, "purpose '{purpose}' is per-EVSE by definition and cannot target station scope (evseId 0)")
            }
            ComposerError::NegativeStackLevel { value } => {
                write!(f, "stack level must be non-negative (found {value})")
            }
            ComposerError::InvalidMinRate { value } => {
                write!(f, "the declared minimum rate must be finite and non-negative (found {value})")
            }
        }
    }
}

impl std::error::Error for ComposerError {}

// --- Inputs --------------------------------------------------------------------

/// One dispatch period: the limit for a time window, in the canonical MW.
/// `limit_mw = 0.0` means "no charging in this window" (an explicit
/// curtailment, carried as data).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DispatchPeriod {
    /// Start offset from the envelope start, in seconds (strictly
    /// ascending across the envelope).
    pub start_offset_s: u32,
    /// The limit in MW (non-negative; 0 = off).
    pub limit_mw: f64,
}

/// The dispatch envelope (the composer's power input, canonical units).
#[derive(Debug, Clone, PartialEq)]
pub struct DispatchEnvelope {
    /// The envelope start instant in UTC milliseconds (start-inclusive;
    /// the schedule anchor for the Absolute kind).
    pub start_millis: i64,
    /// The periods, ascending by start offset.
    pub periods: Vec<DispatchPeriod>,
}

/// The declared supply context of the targeted EVSE (the W/A conversion
/// inputs; module register item 6). The values are DECLARED per device
/// profile — reactive/voltage/phase data is never inferred (AGENTS.md §4
/// canonical-units rule).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SupplyContext {
    /// AC, single phase (one conductor pair; the phase factor is 1).
    AcSinglePhase {
        nominal_voltage_v: f64,
    },
    /// AC, three phases (the phase factor is 3).
    AcThreePhase {
        nominal_voltage_v: f64,
    },
    /// DC (no phase factor; the voltage is the DC bus voltage).
    Dc {
        nominal_voltage_v: f64,
    },
}

impl SupplyContext {
    /// The phase factor the declared formula uses: 1 (single-phase AC),
    /// 3 (three-phase AC), 1 (DC — no phase factor).
    pub const fn phase_factor(&self) -> f64 {
        match self {
            SupplyContext::AcSinglePhase { .. } => 1.0,
            SupplyContext::AcThreePhase { .. } => 3.0,
            SupplyContext::Dc { .. } => 1.0,
        }
    }

    /// The declared nominal voltage.
    pub fn nominal_voltage_v(&self) -> f64 {
        match self {
            SupplyContext::AcSinglePhase { nominal_voltage_v }
            | SupplyContext::AcThreePhase { nominal_voltage_v }
            | SupplyContext::Dc { nominal_voltage_v } => *nominal_voltage_v,
        }
    }

    /// Whether the supply is DC (the conversion formula branches on it).
    pub const fn is_dc(&self) -> bool {
        matches!(self, SupplyContext::Dc { .. })
    }
}

/// The rate unit the composed schedule speaks (module register item 6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RateUnit {
    /// Watts.
    W,
    /// Amperes (per the declared phase semantics).
    A,
}

impl RateUnit {
    /// Canonical wire-ish name.
    pub const fn name(self) -> &'static str {
        match self {
            RateUnit::W => "W",
            RateUnit::A => "A",
        }
    }
}

/// A minimum rate DECLARED with its own unit (module register item 4).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DeclaredMinRate {
    pub value: f64,
    pub unit: RateUnit,
}

/// The device-side rate constraints the composer honours (declared per
/// device profile; never inferred).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DeviceRateConstraints {
    /// The declared minimum charging rate (the unfulfillable-small-rate
    /// rule). `None` = the device declares no minimum.
    pub min_charging_rate: Option<DeclaredMinRate>,
}

// --- Output: the plan -------------------------------------------------------------

/// The charging-profile purpose the composer selected (module register
/// item 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanPurpose {
    /// Station-wide external constraints at evseId 0 (the site/depot
    /// limit path; the P14.15 DOE envelope and the site transformer limit
    /// surface as THIS purpose, marked grid-critical when the constraint
    /// is grid-imposed).
    ChargingStationExternalConstraints,
    /// Station-wide maximum profile (kept COMPOSABLE, not used by this
    /// batch's entry points — the purpose exists so the precedence record
    /// and the reconciliation can name it).
    ChargingStationMaxProfile,
    /// The per-EVSE default profile (applies to transactions started
    /// without a specific TxProfile).
    TxDefaultProfile,
    /// The per-transaction profile (binds to a station transaction id).
    TxProfile,
}

impl PlanPurpose {
    /// Canonical wire-ish name (module register item 1).
    pub const fn name(self) -> &'static str {
        match self {
            PlanPurpose::ChargingStationExternalConstraints => {
                "ChargingStationExternalConstraints"
            }
            PlanPurpose::ChargingStationMaxProfile => "ChargingStationMaxProfile",
            PlanPurpose::TxDefaultProfile => "TxDefaultProfile",
            PlanPurpose::TxProfile => "TxProfile",
        }
    }

    /// The precedence rank for the recorded order (0 = highest; module
    /// register item 2).
    pub const fn precedence_rank(self) -> u8 {
        match self {
            PlanPurpose::ChargingStationExternalConstraints => 0,
            PlanPurpose::ChargingStationMaxProfile => 1,
            PlanPurpose::TxDefaultProfile => 2,
            PlanPurpose::TxProfile => 3,
        }
    }
}

/// One composed schedule period (in the schedule's rate unit).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlanPeriod {
    /// Start offset from the schedule start, in seconds.
    pub start_offset_s: u32,
    /// The limit in the schedule's rate unit. 0.0 = off (the
    /// unfulfillable-small-rate rule, module register item 4).
    pub limit: f64,
    /// The declared phase count for the period, when the rate unit is A
    /// (module register item 6).
    pub number_phases: Option<u8>,
}

/// One recorded composer decision (audit-safe; the plan carries its own
/// decision log so every consumer can see what the composer did — nothing
/// is implicit).
#[derive(Debug, Clone, PartialEq)]
pub enum ComposerDecision {
    /// The canonical MW limit was converted to the schedule's rate unit
    /// with the recorded formula (verbatim; module register item 6).
    UnitConversion {
        start_offset_s: u32,
        from_mw: f64,
        to_value: f64,
        unit: RateUnit,
        formula: String,
    },
    /// A positive limit below the declared minimum rate was set to 0
    /// (off) — the unfulfillable-small-rate rule (module register item 4).
    MinRateToZero {
        start_offset_s: u32,
        requested: f64,
        min_rate: f64,
        unit: RateUnit,
    },
    /// The declared minimum rate was normalized into the schedule's unit
    /// with the recorded formula.
    MinRateConversion {
        from_value: f64,
        from_unit: RateUnit,
        to_value: f64,
        to_unit: RateUnit,
        formula: String,
    },
}

/// The composed charging-profile plan: the domain-level payload candidate.
/// Mapping to the wire SetChargingProfile request happens in the adapter
/// (`crates/adapters/protocol-ocpp::charging`), gated on an ISSUED
/// lifecycle state.
#[derive(Debug, Clone, PartialEq)]
pub struct ChargingProfilePlan {
    /// The selected purpose.
    pub purpose: PlanPurpose,
    /// The targeted EVSE in wire terms (0 = station; UNVERIFIED register
    /// item 3).
    pub evse_wire_id: u32,
    /// The grid-critical marking, for the external-constraints purpose
    /// (module register item 5).
    pub is_grid_critical: Option<bool>,
    /// The bound transaction id, for the TxProfile purpose.
    pub transaction_id: Option<String>,
    /// The declared stack level (non-negative; module register item 2).
    pub stack_level: i32,
    /// The schedule's rate unit.
    pub rate_unit: RateUnit,
    /// The schedule anchor (the envelope start) in UTC milliseconds —
    /// carried so the wire layer can build the Absolute-kind start
    /// schedule.
    pub start_millis: i64,
    /// The normalized declared minimum rate in the schedule's unit
    /// (module register item 4).
    pub min_charging_rate: Option<f64>,
    /// The composed periods (ascending; limits non-negative; the
    /// min-rate pairing enforced).
    pub periods: Vec<PlanPeriod>,
    /// The recorded decisions (the plan's own decision log).
    pub decisions: Vec<ComposerDecision>,
    /// The recorded precedence statement (travels with the plan).
    pub precedence_statement: &'static str,
}

// --- The composer -------------------------------------------------------------------

/// Validates the envelope (non-empty; finite, non-negative limits;
/// strictly ascending offsets).
fn validate_envelope(envelope: &DispatchEnvelope) -> Result<(), ComposerError> {
    if envelope.periods.is_empty() {
        return Err(ComposerError::EmptyPeriods);
    }
    let mut previous: Option<u32> = None;
    for period in &envelope.periods {
        if !period.limit_mw.is_finite() {
            return Err(ComposerError::NonFiniteValue {
                field: "limit_mw",
                value: period.limit_mw,
            });
        }
        if period.limit_mw < 0.0 {
            return Err(ComposerError::NegativeLimit {
                field: "limit_mw",
                value: period.limit_mw,
            });
        }
        if let Some(prev) = previous {
            if period.start_offset_s <= prev {
                return Err(ComposerError::NonAscendingPeriods {
                    previous_start_s: prev,
                    offending_start_s: period.start_offset_s,
                });
            }
        }
        previous = Some(period.start_offset_s);
    }
    Ok(())
}

/// Validates the supply context (positive voltage; AC phases 1 or 3 —
/// enforced by the variants, re-checked on the voltage).
fn validate_supply(supply: &SupplyContext) -> Result<(), ComposerError> {
    let voltage = supply.nominal_voltage_v();
    if !voltage.is_finite() || voltage <= 0.0 {
        return Err(ComposerError::InvalidVoltage { value: voltage });
    }
    Ok(())
}

/// Validates the declared minimum rate (finite, non-negative).
fn validate_min_rate(min_rate: &Option<DeclaredMinRate>) -> Result<(), ComposerError> {
    if let Some(rate) = min_rate {
        if !rate.value.is_finite() || rate.value < 0.0 {
            return Err(ComposerError::InvalidMinRate { value: rate.value });
        }
    }
    Ok(())
}

/// Converts a canonical-MW limit into the schedule's rate unit with the
/// DECLARED formula, recording the decision verbatim (module register
/// item 6):
///
/// - to W: `W = MW * 1_000_000` (exact; the canonical-unit rule).
/// - to A, AC: `A = (MW * 1_000_000) / (nominal_voltage_v * phases)`.
/// - to A, DC: `A = (MW * 1_000_000) / nominal_voltage_v`.
fn convert_limit(
    start_offset_s: u32,
    limit_mw: f64,
    unit: RateUnit,
    supply: &SupplyContext,
    decisions: &mut Vec<ComposerDecision>,
) -> f64 {
    let watts = limit_mw * 1_000_000.0;
    match unit {
        RateUnit::W => {
            decisions.push(ComposerDecision::UnitConversion {
                start_offset_s,
                from_mw: limit_mw,
                to_value: watts,
                unit,
                formula: "W = MW * 1_000_000 (canonical MW, exact)".to_string(),
            });
            watts
        }
        RateUnit::A => {
            let amps = if supply.is_dc() {
                watts / supply.nominal_voltage_v()
            } else {
                watts / (supply.nominal_voltage_v() * supply.phase_factor())
            };
            let formula = if supply.is_dc() {
                format!(
                    "A = (MW * 1_000_000) / V_dc = ({limit_mw} * 1_000_000) / {} (DC, no phase factor)",
                    supply.nominal_voltage_v()
                )
            } else {
                format!(
                    "A = (MW * 1_000_000) / (V * phases) = ({limit_mw} * 1_000_000) / ({} * {})",
                    supply.nominal_voltage_v(),
                    supply.phase_factor()
                )
            };
            decisions.push(ComposerDecision::UnitConversion {
                start_offset_s,
                from_mw: limit_mw,
                to_value: amps,
                unit,
                formula,
            });
            amps
        }
    }
}

/// Normalizes the declared minimum rate into the schedule's unit
/// (recording the conversion decision) — module register item 4.
fn normalize_min_rate(
    min_rate: Option<DeclaredMinRate>,
    unit: RateUnit,
    supply: &SupplyContext,
    decisions: &mut Vec<ComposerDecision>,
) -> Option<f64> {
    let rate = min_rate?;
    match (rate.unit, unit) {
        (RateUnit::W, RateUnit::W) | (RateUnit::A, RateUnit::A) => Some(rate.value),
        (RateUnit::A, RateUnit::W) => {
            // A → W with the declared formula (AC: per-phase amperes times
            // voltage times the phase factor).
            let watts = if supply.is_dc() {
                rate.value * supply.nominal_voltage_v()
            } else {
                rate.value * supply.nominal_voltage_v() * supply.phase_factor()
            };
            let formula = if supply.is_dc() {
                format!(
                    "W = A * V_dc = {} * {} (DC, no phase factor)",
                    rate.value,
                    supply.nominal_voltage_v()
                )
            } else {
                format!(
                    "W = A * V * phases = {} * {} * {}",
                    rate.value,
                    supply.nominal_voltage_v(),
                    supply.phase_factor()
                )
            };
            decisions.push(ComposerDecision::MinRateConversion {
                from_value: rate.value,
                from_unit: RateUnit::A,
                to_value: watts,
                to_unit: RateUnit::W,
                formula,
            });
            Some(watts)
        }
        (RateUnit::W, RateUnit::A) => {
            let amps = if supply.is_dc() {
                rate.value / supply.nominal_voltage_v()
            } else {
                rate.value / (supply.nominal_voltage_v() * supply.phase_factor())
            };
            let formula = if supply.is_dc() {
                format!(
                    "A = W / V_dc = {} / {} (DC, no phase factor)",
                    rate.value,
                    supply.nominal_voltage_v()
                )
            } else {
                format!(
                    "A = W / (V * phases) = {} / ({} * {})",
                    rate.value,
                    supply.nominal_voltage_v(),
                    supply.phase_factor()
                )
            };
            decisions.push(ComposerDecision::MinRateConversion {
                from_value: rate.value,
                from_unit: RateUnit::W,
                to_value: amps,
                to_unit: RateUnit::A,
                formula,
            });
            Some(amps)
        }
    }
}

/// The shared compose core: validate, convert, apply the min-rate pairing,
/// and assemble the plan.
fn compose(
    purpose: PlanPurpose,
    evse_wire_id: u32,
    is_grid_critical: Option<bool>,
    transaction_id: Option<String>,
    stack_level: i32,
    envelope: &DispatchEnvelope,
    unit: RateUnit,
    supply: &SupplyContext,
    device: &DeviceRateConstraints,
) -> Result<ChargingProfilePlan, ComposerError> {
    // Purpose/scope rules (module register items 3/6).
    match purpose {
        PlanPurpose::TxProfile => {
            if transaction_id.as_deref().map(|id| id.trim().is_empty()).unwrap_or(true) {
                return Err(ComposerError::TxProfileNeedsTransaction);
            }
            if evse_wire_id == 0 {
                return Err(ComposerError::StationScopePurpose {
                    purpose: purpose.name(),
                });
            }
        }
        PlanPurpose::TxDefaultProfile => {
            if evse_wire_id == 0 {
                return Err(ComposerError::StationScopePurpose {
                    purpose: purpose.name(),
                });
            }
        }
        _ => {}
    }
    if stack_level < 0 {
        return Err(ComposerError::NegativeStackLevel { value: stack_level });
    }
    validate_envelope(envelope)?;
    validate_supply(supply)?;
    validate_min_rate(&device.min_charging_rate)?;

    let mut decisions: Vec<ComposerDecision> = Vec::new();
    let min_charging_rate =
        normalize_min_rate(device.min_charging_rate, unit, supply, &mut decisions);

    let mut periods: Vec<PlanPeriod> = Vec::with_capacity(envelope.periods.len());
    for period in &envelope.periods {
        let mut limit = convert_limit(period.start_offset_s, period.limit_mw, unit, supply, &mut decisions);
        // The unfulfillable-small-rate rule (module register item 4): a
        // positive limit below the declared minimum is NOT a valid
        // alternative — it becomes 0 (off), and the decision is recorded.
        if let Some(min_rate) = min_charging_rate {
            if limit > COMPOSER_EVALUATION_TOLERANCE
                && limit + COMPOSER_EVALUATION_TOLERANCE < min_rate
            {
                decisions.push(ComposerDecision::MinRateToZero {
                    start_offset_s: period.start_offset_s,
                    requested: limit,
                    min_rate,
                    unit,
                });
                limit = 0.0;
            }
        }
        periods.push(PlanPeriod {
            start_offset_s: period.start_offset_s,
            limit,
            // The phase count rides the period when amperes are used on an
            // AC supply (module register item 6).
            number_phases: match (unit, supply.is_dc()) {
                (RateUnit::A, false) => Some(supply.phase_factor() as u8),
                _ => None,
            },
        });
    }

    Ok(ChargingProfilePlan {
        purpose,
        evse_wire_id,
        is_grid_critical,
        transaction_id,
        stack_level,
        rate_unit: unit,
        start_millis: envelope.start_millis,
        min_charging_rate,
        periods,
        decisions,
        precedence_statement: STACK_PRECEDENCE_STATEMENT,
    })
}

/// Composes a station-scope external-constraints profile at evseId 0 —
/// the site/depot-limit path. `grid_critical` marks the profile as
/// grid-imposed (module register item 5).
#[allow(clippy::too_many_arguments)]
pub fn compose_external_constraints_profile(
    envelope: &DispatchEnvelope,
    unit: RateUnit,
    supply: &SupplyContext,
    device: &DeviceRateConstraints,
    stack_level: i32,
    grid_critical: bool,
) -> Result<ChargingProfilePlan, ComposerError> {
    compose(
        PlanPurpose::ChargingStationExternalConstraints,
        0, // station scope (module register item 3)
        Some(grid_critical),
        None,
        stack_level,
        envelope,
        unit,
        supply,
        device,
    )
}

/// Composes a per-EVSE default profile (TxDefaultProfile).
#[allow(clippy::too_many_arguments)]
pub fn compose_tx_default_profile(
    envelope: &DispatchEnvelope,
    unit: RateUnit,
    supply: &SupplyContext,
    device: &DeviceRateConstraints,
    evse_wire_id: u32,
    stack_level: i32,
) -> Result<ChargingProfilePlan, ComposerError> {
    compose(
        PlanPurpose::TxDefaultProfile,
        evse_wire_id,
        None,
        None,
        stack_level,
        envelope,
        unit,
        supply,
        device,
    )
}

/// Composes a per-transaction profile (TxProfile), bound to the
/// station-side transaction id.
#[allow(clippy::too_many_arguments)]
pub fn compose_tx_profile(
    envelope: &DispatchEnvelope,
    unit: RateUnit,
    supply: &SupplyContext,
    device: &DeviceRateConstraints,
    evse_wire_id: u32,
    stack_level: i32,
    transaction_id: &str,
) -> Result<ChargingProfilePlan, ComposerError> {
    compose(
        PlanPurpose::TxProfile,
        evse_wire_id,
        None,
        Some(transaction_id.to_string()),
        stack_level,
        envelope,
        unit,
        supply,
        device,
    )
}

// --- Reconciliation ------------------------------------------------------------------

/// One observed period of the station's composite schedule (the
/// GetCompositeSchedule result, normalized into the plan's rate unit by
/// the CALLER — the observation carries its declared unit and the caller
/// records any conversion; this module compares like with like).
#[derive(Debug, Clone, PartialEq)]
pub struct CompositeScheduleObservation {
    /// The declared rate unit of the observation.
    pub rate_unit: RateUnit,
    /// The observed periods (ascending; the caller normalizes the shape).
    pub periods: Vec<PlanPeriod>,
}

/// The reconciliation outcome (module register item 7; record 05 §6).
#[derive(Debug, Clone, PartialEq)]
pub enum ReconciliationOutcome {
    /// The observed composite matches the plan within the tolerance at
    /// every period — application evidence CONSISTENT with the plan (the
    /// reconciliation is evidence, never a guarantee of physical delivery;
    /// TRACKING remains the lifecycle's business).
    Aligned,
    /// The observed composite deviates from the plan. Every mismatch is
    /// named: a missing period, an extra period, or a limit deviation. The
    /// record surfaces to the operator; the adapter NEVER retries or
    /// re-issues on its own authority.
    Mismatch {
        details: Vec<ReconciliationMismatch>,
    },
    /// The observation speaks a different rate unit than the plan — a
    /// caller error, refused (never silently converted: the composite
    /// schedule's unit semantics are UNVERIFIED and the conversion belongs
    /// to the caller's declared mapping).
    UnitMismatch {
        plan_unit: RateUnit,
        observed_unit: RateUnit,
    },
}

/// One named mismatch entry.
#[derive(Debug, Clone, PartialEq)]
pub enum ReconciliationMismatch {
    /// The plan limits a period the observed composite does not carry.
    MissingPeriod {
        start_offset_s: u32,
        planned_limit: f64,
    },
    /// The observed composite carries a period the plan does not declare
    /// (another profile composed on top — the precedence statement's
    /// business).
    ExtraPeriod {
        start_offset_s: u32,
        observed_limit: f64,
    },
    /// The observed limit deviates beyond the tolerance.
    LimitDeviation {
        start_offset_s: u32,
        planned_limit: f64,
        observed_limit: f64,
    },
}

/// Reconciles a composed plan against the station's observed composite
/// schedule. Like-with-like only: a unit mismatch is a named refusal, a
/// period-shape mismatch is a named record. Empty observations are legal
/// input (the station may report an empty composite — every planned period
/// then mismatches as MissingPeriod).
pub fn reconcile_composite_schedule(
    plan: &ChargingProfilePlan,
    observation: &CompositeScheduleObservation,
) -> ReconciliationOutcome {
    if observation.rate_unit != plan.rate_unit {
        return ReconciliationOutcome::UnitMismatch {
            plan_unit: plan.rate_unit,
            observed_unit: observation.rate_unit,
        };
    }
    let mut details: Vec<ReconciliationMismatch> = Vec::new();
    let mut observed_iter = observation.periods.iter().peekable();
    for planned in &plan.periods {
        // Advance the observation to (or past) the planned offset.
        while let Some(observed) = observed_iter.peek() {
            match observed.start_offset_s.cmp(&planned.start_offset_s) {
                std::cmp::Ordering::Less => {
                    details.push(ReconciliationMismatch::ExtraPeriod {
                        start_offset_s: observed.start_offset_s,
                        observed_limit: observed.limit,
                    });
                    let _ = observed_iter.next();
                }
                std::cmp::Ordering::Greater => break,
                std::cmp::Ordering::Equal => break,
            }
        }
        match observed_iter.peek() {
            Some(observed) if observed.start_offset_s == planned.start_offset_s => {
                // Invariant: `peek()` returned `Some` on this same iterator
                // above and nothing consumed it since — `next()` cannot fail
                // here (final-review FRV1-R7-010: guarded, not hidden).
                let observed = observed_iter.next().expect("peeked");
                if (observed.limit - planned.limit).abs() > COMPOSER_EVALUATION_TOLERANCE {
                    details.push(ReconciliationMismatch::LimitDeviation {
                        start_offset_s: planned.start_offset_s,
                        planned_limit: planned.limit,
                        observed_limit: observed.limit,
                    });
                }
            }
            _ => {
                details.push(ReconciliationMismatch::MissingPeriod {
                    start_offset_s: planned.start_offset_s,
                    planned_limit: planned.limit,
                });
            }
        }
    }
    // Any remaining observed periods are extras.
    for observed in observed_iter {
        details.push(ReconciliationMismatch::ExtraPeriod {
            start_offset_s: observed.start_offset_s,
            observed_limit: observed.limit,
        });
    }
    if details.is_empty() {
        ReconciliationOutcome::Aligned
    } else {
        ReconciliationOutcome::Mismatch { details }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn envelope() -> DispatchEnvelope {
        DispatchEnvelope {
            start_millis: 1_700_000_000_000,
            periods: vec![
                DispatchPeriod {
                    start_offset_s: 0,
                    limit_mw: 0.1, // 100 kW
                },
                DispatchPeriod {
                    start_offset_s: 900,
                    limit_mw: 0.0, // off
                },
            ],
        }
    }

    fn supply_ac_three() -> SupplyContext {
        SupplyContext::AcThreePhase {
            nominal_voltage_v: 400.0,
        }
    }

    fn device() -> DeviceRateConstraints {
        DeviceRateConstraints {
            min_charging_rate: None,
        }
    }

    #[test]
    fn external_constraints_plan_is_station_scope_and_grid_critical() {
        let plan = compose_external_constraints_profile(
            &envelope(),
            RateUnit::W,
            &supply_ac_three(),
            &device(),
            10,
            true,
        )
        .expect("composed");
        assert_eq!(plan.purpose, PlanPurpose::ChargingStationExternalConstraints);
        assert_eq!(plan.evse_wire_id, 0);
        assert_eq!(plan.is_grid_critical, Some(true));
        assert_eq!(plan.rate_unit, RateUnit::W);
        assert_eq!(plan.stack_level, 10);
        assert_eq!(plan.precedence_statement, STACK_PRECEDENCE_STATEMENT);
        // W conversion is exact: 0.1 MW -> 100_000 W; 0.0 MW -> 0.0 W.
        assert_eq!(plan.periods[0].limit, 100_000.0);
        assert_eq!(plan.periods[1].limit, 0.0);
        assert!(plan.periods.iter().all(|p| p.number_phases.is_none()));
        // The decision log carries both conversions verbatim.
        assert_eq!(plan.decisions.len(), 2);
        match &plan.decisions[0] {
            ComposerDecision::UnitConversion {
                from_mw,
                to_value,
                formula,
                ..
            } => {
                assert_eq!(*from_mw, 0.1);
                assert_eq!(*to_value, 100_000.0);
                assert!(formula.contains("1_000_000"));
            }
            other => panic!("expected a conversion decision, got {other:?}"),
        }
    }

    #[test]
    fn ampere_conversion_uses_the_declared_formula() {
        // 0.1 MW over 400 V three-phase: A = 100_000 / (400 * 3) = 83.333...
        let plan = compose_tx_default_profile(
            &envelope(),
            RateUnit::A,
            &supply_ac_three(),
            &device(),
            1,
            5,
        )
        .expect("composed");
        assert!((plan.periods[0].limit - 100_000.0 / 1_200.0).abs() < COMPOSER_EVALUATION_TOLERANCE);
        assert_eq!(plan.periods[0].number_phases, Some(3));
        // The formula is recorded verbatim and names the phase factor.
        match &plan.decisions[0] {
            ComposerDecision::UnitConversion { formula, .. } => {
                assert!(formula.contains("400"), "formula names the voltage: {formula}");
                assert!(formula.contains("3"), "formula names the phase factor: {formula}");
            }
            other => panic!("expected a conversion decision, got {other:?}"),
        }

        // DC: no phase factor. 0.1 MW over 500 V DC: A = 100_000 / 500 = 200.
        let supply_dc = SupplyContext::Dc {
            nominal_voltage_v: 500.0,
        };
        let plan = compose_tx_default_profile(&envelope(), RateUnit::A, &supply_dc, &device(), 1, 5)
            .expect("composed");
        assert!((plan.periods[0].limit - 200.0).abs() < COMPOSER_EVALUATION_TOLERANCE);
        assert_eq!(plan.periods[0].number_phases, None, "DC carries no phase count");
    }

    #[test]
    fn min_rate_rule_sets_unfulfillable_limits_to_zero() {
        // Device minimum: 6 A (AC three-phase). A composed limit below 6 A
        // becomes 0 (off); a limit at or above it passes through.
        let device = DeviceRateConstraints {
            min_charging_rate: Some(DeclaredMinRate {
                value: 6.0,
                unit: RateUnit::A,
            }),
        };
        let envelope = DispatchEnvelope {
            start_millis: 0,
            periods: vec![
                DispatchPeriod {
                    start_offset_s: 0,
                    limit_mw: 0.001, // 1 kW -> 1000/(400*3) ≈ 0.83 A < 6 A -> 0
                },
                DispatchPeriod {
                    start_offset_s: 600,
                    limit_mw: 0.01, // 10 kW -> 10000/1200 ≈ 8.33 A >= 6 A -> stays
                },
            ],
        };
        let plan = compose_tx_default_profile(&envelope, RateUnit::A, &supply_ac_three(), &device, 2, 5)
            .expect("composed");
        assert_eq!(plan.periods[0].limit, 0.0, "below the minimum -> 0 (off)");
        assert!(plan.periods[1].limit > 6.0);
        // The to-zero decision is recorded with the requested value.
        let to_zero = plan
            .decisions
            .iter()
            .find(|d| matches!(d, ComposerDecision::MinRateToZero { .. }))
            .expect("to-zero decision recorded");
        match to_zero {
            ComposerDecision::MinRateToZero {
                requested,
                min_rate,
                unit,
                ..
            } => {
                assert!((*requested - 1000.0 / 1200.0).abs() < COMPOSER_EVALUATION_TOLERANCE);
                assert_eq!(*min_rate, 6.0);
                assert_eq!(*unit, RateUnit::A);
            }
            other => panic!("unexpected decision {other:?}"),
        }

        // The same envelope in W: the 6 A minimum normalizes to
        // 6 * 400 * 3 = 7200 W; the 1 kW limit is below it -> 0.
        let plan_w = compose_tx_default_profile(&envelope, RateUnit::W, &supply_ac_three(), &device, 2, 5)
            .expect("composed");
        assert_eq!(plan_w.min_charging_rate, Some(7200.0));
        assert_eq!(plan_w.periods[0].limit, 0.0);
        assert_eq!(plan_w.periods[1].limit, 10_000.0);
        match plan_w.decisions.first().expect("decision") {
            ComposerDecision::MinRateConversion {
                from_value,
                from_unit,
                to_value,
                to_unit,
                formula,
            } => {
                assert_eq!(*from_value, 6.0);
                assert_eq!(*from_unit, RateUnit::A);
                assert_eq!(*to_value, 7200.0);
                assert_eq!(*to_unit, RateUnit::W);
                assert!(formula.contains("400") && formula.contains("3"));
            }
            other => panic!("expected a min-rate conversion, got {other:?}"),
        }
    }

    #[test]
    fn purpose_scope_rules_are_enforced() {
        // TxProfile without a transaction id.
        assert_eq!(
            compose_tx_profile(&envelope(), RateUnit::W, &supply_ac_three(), &device(), 1, 5, "  "),
            Err(ComposerError::TxProfileNeedsTransaction)
        );
        // TxProfile at station scope.
        assert_eq!(
            compose_tx_profile(&envelope(), RateUnit::W, &supply_ac_three(), &device(), 0, 5, "txn-1"),
            Err(ComposerError::StationScopePurpose {
                purpose: "TxProfile"
            })
        );
        // TxDefaultProfile at station scope.
        assert_eq!(
            compose_tx_default_profile(&envelope(), RateUnit::W, &supply_ac_three(), &device(), 0, 5),
            Err(ComposerError::StationScopePurpose {
                purpose: "TxDefaultProfile"
            })
        );
        // Negative stack level.
        assert_eq!(
            compose_external_constraints_profile(&envelope(), RateUnit::W, &supply_ac_three(), &device(), -1, false),
            Err(ComposerError::NegativeStackLevel { value: -1 })
        );
        // Invalid supply data: no conversion is invented.
        let bad_supply = SupplyContext::AcThreePhase {
            nominal_voltage_v: 0.0,
        };
        assert_eq!(
            compose_tx_default_profile(&envelope(), RateUnit::A, &bad_supply, &device(), 1, 5),
            Err(ComposerError::InvalidVoltage { value: 0.0 })
        );
        // Non-ascending periods.
        let bad_envelope = DispatchEnvelope {
            start_millis: 0,
            periods: vec![
                DispatchPeriod {
                    start_offset_s: 600,
                    limit_mw: 0.1,
                },
                DispatchPeriod {
                    start_offset_s: 600,
                    limit_mw: 0.1,
                },
            ],
        };
        assert!(matches!(
            compose_tx_default_profile(&bad_envelope, RateUnit::W, &supply_ac_three(), &device(), 1, 5),
            Err(ComposerError::NonAscendingPeriods { .. })
        ));
        // Negative limit.
        let bad_envelope = DispatchEnvelope {
            start_millis: 0,
            periods: vec![DispatchPeriod {
                start_offset_s: 0,
                limit_mw: -0.1,
            }],
        };
        assert!(matches!(
            compose_tx_default_profile(&bad_envelope, RateUnit::W, &supply_ac_three(), &device(), 1, 5),
            Err(ComposerError::NegativeLimit { .. })
        ));
    }

    #[test]
    fn reconciliation_names_every_mismatch_and_stays_honest() {
        let plan = compose_external_constraints_profile(
            &envelope(),
            RateUnit::W,
            &supply_ac_three(),
            &device(),
            10,
            true,
        )
        .expect("composed");

        // Aligned: the station's composite matches the plan.
        let observation = CompositeScheduleObservation {
            rate_unit: RateUnit::W,
            periods: vec![
                PlanPeriod {
                    start_offset_s: 0,
                    limit: 100_000.0,
                    number_phases: None,
                },
                PlanPeriod {
                    start_offset_s: 900,
                    limit: 0.0,
                    number_phases: None,
                },
            ],
        };
        assert_eq!(
            reconcile_composite_schedule(&plan, &observation),
            ReconciliationOutcome::Aligned
        );

        // Deviations are named: an extra period, a limit deviation, and a
        // missing planned period.
        let observation = CompositeScheduleObservation {
            rate_unit: RateUnit::W,
            periods: vec![
                PlanPeriod {
                    start_offset_s: 0,
                    limit: 50_000.0, // deviation
                    number_phases: None,
                },
                PlanPeriod {
                    start_offset_s: 450, // extra (another profile composed on top)
                    limit: 10_000.0,
                    number_phases: None,
                },
            ],
        };
        let outcome = reconcile_composite_schedule(&plan, &observation);
        match outcome {
            ReconciliationOutcome::Mismatch { details } => {
                assert!(details.contains(&ReconciliationMismatch::LimitDeviation {
                    start_offset_s: 0,
                    planned_limit: 100_000.0,
                    observed_limit: 50_000.0,
                }));
                assert!(details.contains(&ReconciliationMismatch::ExtraPeriod {
                    start_offset_s: 450,
                    observed_limit: 10_000.0,
                }));
                assert!(details.contains(&ReconciliationMismatch::MissingPeriod {
                    start_offset_s: 900,
                    planned_limit: 0.0,
                }));
            }
            other => panic!("expected a mismatch, got {other:?}"),
        }

        // An empty observation: every planned period is missing.
        let observation = CompositeScheduleObservation {
            rate_unit: RateUnit::W,
            periods: Vec::new(),
        };
        match reconcile_composite_schedule(&plan, &observation) {
            ReconciliationOutcome::Mismatch { details } => {
                assert_eq!(details.len(), plan.periods.len());
            }
            other => panic!("expected a mismatch, got {other:?}"),
        }

        // A unit mismatch is a named refusal (never silently converted).
        let observation = CompositeScheduleObservation {
            rate_unit: RateUnit::A,
            periods: Vec::new(),
        };
        assert_eq!(
            reconcile_composite_schedule(&plan, &observation),
            ReconciliationOutcome::UnitMismatch {
                plan_unit: RateUnit::W,
                observed_unit: RateUnit::A,
            }
        );

        // The honesty statements travel.
        assert!(RECONCILIATION_STATEMENT.contains("STORED, not applied"));
        assert!(STACK_PRECEDENCE_STATEMENT.contains("UNVERIFIED against"));
    }

    #[test]
    fn precedence_ranks_follow_the_recorded_order() {
        assert_eq!(
            PlanPurpose::ChargingStationExternalConstraints.precedence_rank(),
            0
        );
        assert_eq!(PlanPurpose::ChargingStationMaxProfile.precedence_rank(), 1);
        assert_eq!(PlanPurpose::TxDefaultProfile.precedence_rank(), 2);
        assert_eq!(PlanPurpose::TxProfile.precedence_rank(), 3);
    }
}
