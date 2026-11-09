// P3.01 — Versioned capability-model contract (report §11.1).
//
// Implements ledger P3.01 ("Freeze the capability-model contract"). Decision
// record: `ledger/phase-03-flexibility-scheduling/01_04_capability_contract.md`.
// Normative sources: report §11.1 (generic DER capability model), §10.2 (units
// and sign conventions), §10.3 (time model), §10.5 (schema governance);
// foundations specification governs on conflict.
//
// What schema version 1 freezes:
//
// - A time-indexed feasible operating set X(i,t) over (P, Q, E, u) per §11.1:
//   active power (MW), reactive power (MVAr), energy (MWh), and the on/off or
//   mode indicator u as DECLARED (a `DeviceMode` constraint carrying an
//   explicit mode set). The capability model is **not** a single `max_mw`
//   field (§11.1).
// - Every constraint is a generic slot carrying: a declared constraint kind
//   (one of the twelve §11.1 categories — no per-technology fields), a
//   canonical unit (MW, MVAr, MWh, SOC fraction, MW/min, seconds,
//   dimensionless — **no kW or kWh anywhere in the schema**), a declared sign
//   convention (active power is frozen to **P > 0 = export at the point of
//   connection**; the reactive-power sign is DECLARED per grid model and
//   device profile, never assumed, §10.2), a validity window (§10.3), a
//   provenance flag (measured / configured / inferred / unknown), and a
//   structural expression (closed interval, numeric value set, or declared
//   mode set).
// - A schema version constant serialized as the `schema_version` field. A
//   payload carrying any other version is rejected at deserialization. Per
//   §10.5, breaking schema changes are deliberate and versioned; every
//   version bump requires a migration note in the decision record.
//
// Layering (recorded decision): this contract enforces STRUCTURAL sanity
// (ordered interval bounds, non-empty sets, kind/unit compatibility, UTC
// horizon start, sorted time slices). The NORMATIVE boundary rules — canonical
// unit spellings on text payloads, the P > 0 export assertion, SOC in [0,1]
// boundary-inclusive, non-negative charge/discharge/ramp, E_min ≤ E_max — are
// validators in flexcore-domain (P3.04, `capability_validate`) operating on
// these types; keeping them out of the contract keeps the schema descriptive
// and puts the structured error catalogue in one place.
//
// Provenance note: the provenance ENUM lives here (it is part of the
// versioned wire schema, §11.1: "each asset adapter must state which
// constraints are measured, configured, inferred, or unknown"); the narrowing
// RULE for unknown provenance lives in flexcore-domain (P3.02): unknown
// narrows the feasible set to a documented conservative default per asset
// class and never widens.
//
// Expressible vs unsupported: all twelve §11.1 constraint categories have a
// home as generic slots (see [`ConstraintKind`]). Schema version 1 does NOT
// express: intra-interval trajectories (per-interval bounds only), per-phase
// unbalanced quantities, embedded probability distributions, thermal/health
// models (declared bounds only, not a thermal model), and network topology
// (declared site bounds only — the grid model is a separate §10.1 entity
// consumed by the Phase 4 feasibility gate). The full list with reasons is in
// the decision record.
//
// Serialization note: enums serialize as their SCREAMING_SNAKE_CASE canonical
// names; timestamps serialize as RFC 3339 UTC strings; the serialized unit
// names are the canonical units only (there is no kW/kWh variant to serialize).
//
// Control status: contract types for the simulation-only 0.x line. Nothing
// here authorizes field control or device writes; a capability model is input
// to envelope estimation and candidate scheduling (Phase 3). A candidate
// schedule is never "feasible" — feasibility is the separate Phase 4 gate.

use serde::{Deserialize, Serialize};
use time::{Duration, OffsetDateTime};

use crate::ids::FlexUuid;
use crate::time_model::ValidityWindow;

/// The schema version of the capability-model contract frozen by P3.01
/// (report §10.5: breaking schema changes are deliberate and versioned).
///
/// Any change to the schema shape, the [`ConstraintKind`] / [`ConstraintUnit`]
/// / [`ConstraintSignConvention`] vocabularies, or the kind/unit compatibility
/// table requires bumping this constant and writing a migration note in
/// `ledger/phase-03-flexibility-scheduling/01_04_capability_contract.md`.
pub const CAPABILITY_SCHEMA_VERSION: u32 = 1;

/// Errors returned by the P3.01 capability-model contract.
#[derive(Debug, Clone, PartialEq)]
pub enum CapabilityModelError {
    /// The payload declares a schema version this contract does not support.
    /// Only [`CAPABILITY_SCHEMA_VERSION`] is accepted (§10.5 versioning).
    UnsupportedSchemaVersion {
        /// The version found in the payload.
        found: u32,
        /// The only supported version.
        supported: u32,
    },
    /// The asset identifier was the nil UUID. Capability identity must be a
    /// real, generated identifier (generation is an application-layer port
    /// concern, contracts `ids` module).
    NilAssetIdentifier,
    /// The horizon start did not carry the UTC offset. Contracts require UTC
    /// internally (§10.3); non-UTC offsets are rejected, never converted.
    NonUtcHorizonStart,
    /// The interval length must be a positive number of seconds.
    NonPositiveIntervalLength,
    /// The time-indexed set must cover at least one interval.
    ZeroIntervalCount,
    /// The time slices are not sorted strictly ascending by interval index
    /// (the canonical form of the schema is a sorted, sparse-or-dense list).
    UnsortedIntervalSlices,
    /// Two slices carry the same interval index.
    DuplicateIntervalIndex {
        /// The duplicated index.
        index: u32,
    },
    /// A slice carries an interval index outside `[0, interval_count)`.
    IntervalIndexOutOfRange {
        /// The offending index.
        index: u32,
        /// The declared interval count of the horizon.
        interval_count: u32,
    },
    /// The constraint expression is structurally invalid (both interval sides
    /// absent, lower > upper, empty value/mode set, non-finite number, empty
    /// mode name).
    InvalidExpression(&'static str),
    /// The declared constraint kind and unit are not compatible (for example
    /// a ramp-rate constraint in MWh). The compatibility table is part of the
    /// frozen schema; relaxing it is a schema-version change (§10.5).
    IncompatibleKindUnit {
        /// The declared kind name.
        kind: &'static str,
        /// The declared unit name.
        unit: &'static str,
    },
}

impl std::fmt::Display for CapabilityModelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CapabilityModelError::UnsupportedSchemaVersion { found, supported } => write!(
                f,
                "capability schema version {found} is not supported (supported version: {supported})"
            ),
            CapabilityModelError::NilAssetIdentifier => {
                write!(f, "capability model asset_id must not be the nil UUID")
            }
            CapabilityModelError::NonUtcHorizonStart => write!(
                f,
                "capability model horizon_start must carry the UTC offset; non-UTC offsets are rejected, not converted"
            ),
            CapabilityModelError::NonPositiveIntervalLength => {
                write!(f, "capability model interval_seconds must be positive")
            }
            CapabilityModelError::ZeroIntervalCount => {
                write!(f, "capability model interval_count must be at least 1")
            }
            CapabilityModelError::UnsortedIntervalSlices => write!(
                f,
                "capability model time slices must be sorted strictly ascending by interval index"
            ),
            CapabilityModelError::DuplicateIntervalIndex { index } => {
                write!(f, "capability model declares interval index {index} more than once")
            }
            CapabilityModelError::IntervalIndexOutOfRange { index, interval_count } => write!(
                f,
                "capability model interval index {index} is outside the horizon (interval_count = {interval_count})"
            ),
            CapabilityModelError::InvalidExpression(reason) => {
                write!(f, "constraint expression is invalid: {reason}")
            }
            CapabilityModelError::IncompatibleKindUnit { kind, unit } => write!(
                f,
                "constraint kind {kind} cannot be declared in unit {unit} (see the frozen kind/unit compatibility table)"
            ),
        }
    }
}

impl std::error::Error for CapabilityModelError {}

// --- Provenance -------------------------------------------------------------

/// Per-constraint provenance flag (report §11.1): each asset adapter must
/// state which constraints are measured, configured, inferred, or unknown.
///
/// The ENUM is part of the versioned wire schema (frozen here, P3.01). The
/// narrowing RULE for [`Provenance::Unknown`] lives in flexcore-domain
/// (P3.02): an unknown-provenance capability narrows the feasible set to a
/// documented conservative default per asset class and never widens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Provenance {
    /// The limit comes from a measurement of the asset.
    Measured,
    /// The limit comes from a configuration source (registry, device profile,
    /// nameplate).
    Configured,
    /// The limit was derived by a declared rule from other known values.
    Inferred,
    /// The limit is not known. Never silently treated as unconstrained; the
    /// P3.02 rule narrows it to the recorded conservative default.
    Unknown,
}

impl Provenance {
    /// All four provenance values, in §11.1 order.
    pub const ALL: [Provenance; 4] = [
        Provenance::Measured,
        Provenance::Configured,
        Provenance::Inferred,
        Provenance::Unknown,
    ];

    /// The canonical SCREAMING_SNAKE name from the frozen vocabulary.
    pub const fn name(self) -> &'static str {
        match self {
            Provenance::Measured => "MEASURED",
            Provenance::Configured => "CONFIGURED",
            Provenance::Inferred => "INFERRED",
            Provenance::Unknown => "UNKNOWN",
        }
    }
}

// --- Constraint kinds -------------------------------------------------------

/// The declared kind of a capability constraint — the generic slot vocabulary
/// covering every §11.1 category. There is deliberately **no field per
/// technology**: a BESS temperature derate, an inverter site limit, and an EV
/// fleet commitment are all generic slots whose meaning comes from `kind` +
/// `unit` + declared sign + provenance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ConstraintKind {
    /// Active power bounds (MW). Sign is frozen: **P > 0 = export** at the
    /// point of connection (§10.2).
    ActivePower,
    /// Reactive power bounds (MVAr). Sign is declared per grid model and
    /// device profile, never assumed (§10.2).
    ReactivePower,
    /// Energy or state-of-charge bounds (MWh, or SOC as a fraction in [0,1]).
    EnergySoc,
    /// Device mode / on-off indicator `u` as DECLARED (§11.1): an explicit
    /// mode set, never an assumed binary.
    DeviceMode,
    /// Temperature and health dependent limits (§11.1), declared as bounds
    /// (a MW derate or a dimensionless availability factor) with provenance —
    /// not as a thermal model.
    TemperatureHealth,
    /// Ramp-rate limit (MW per minute).
    RampRate,
    /// Minimum up-time (seconds).
    MinimumUpTime,
    /// Minimum down-time (seconds).
    MinimumDownTime,
    /// Duration and recovery constraints (§11.1), declared in seconds. The
    /// cross-interval enforcement of a duration is a scheduler (solver
    /// formulation) concern; the schema carries the declared duration.
    DurationRecovery,
    /// Site network constraints (§11.1) as declared bounds at the site point
    /// of connection — not a grid model. The grid model is a separate §10.1
    /// entity consumed by the Phase 4 feasibility gate.
    SiteNetwork,
    /// Forecast-condition dependent limits (§11.1), declared as bounds with
    /// provenance. Forecast distributions themselves live in the §10.1
    /// ForecastBundle entity and are referenced, not embedded here.
    ForecastCondition,
    /// Operator/customer commitments (§11.1), declared as bounds.
    Commitment,
}

impl ConstraintKind {
    /// All twelve §11.1 categories, in report order (power, energy/SOC, mode,
    /// temperature/health, ramp, min up, min down, duration/recovery, site
    /// network, forecast, commitments).
    pub const ALL: [ConstraintKind; 12] = [
        ConstraintKind::ActivePower,
        ConstraintKind::ReactivePower,
        ConstraintKind::EnergySoc,
        ConstraintKind::DeviceMode,
        ConstraintKind::TemperatureHealth,
        ConstraintKind::RampRate,
        ConstraintKind::MinimumUpTime,
        ConstraintKind::MinimumDownTime,
        ConstraintKind::DurationRecovery,
        ConstraintKind::SiteNetwork,
        ConstraintKind::ForecastCondition,
        ConstraintKind::Commitment,
    ];

    /// The canonical SCREAMING_SNAKE name from the frozen vocabulary.
    pub const fn name(self) -> &'static str {
        match self {
            ConstraintKind::ActivePower => "ACTIVE_POWER",
            ConstraintKind::ReactivePower => "REACTIVE_POWER",
            ConstraintKind::EnergySoc => "ENERGY_SOC",
            ConstraintKind::DeviceMode => "DEVICE_MODE",
            ConstraintKind::TemperatureHealth => "TEMPERATURE_HEALTH",
            ConstraintKind::RampRate => "RAMP_RATE",
            ConstraintKind::MinimumUpTime => "MINIMUM_UP_TIME",
            ConstraintKind::MinimumDownTime => "MINIMUM_DOWN_TIME",
            ConstraintKind::DurationRecovery => "DURATION_RECOVERY",
            ConstraintKind::SiteNetwork => "SITE_NETWORK",
            ConstraintKind::ForecastCondition => "FORECAST_CONDITION",
            ConstraintKind::Commitment => "COMMITMENT",
        }
    }
}

// --- Constraint units -------------------------------------------------------

/// The canonical unit of a capability constraint (report §10.2).
///
/// **No kW, kWh, or other scaled unit exists in this vocabulary.** Scaled
/// units are display-only and are converted at the adapter (§10.2 §5); a text
/// payload carrying "kW"/"kWh" is rejected at the contract boundary by the
/// P3.04 validators in flexcore-domain. Time-based constraints (minimum
/// up/down time, duration/recovery) are declared in seconds; ramp rate in MW
/// per minute; mode indicators and availability factors are dimensionless.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ConstraintUnit {
    /// Active power in **MW**.
    Megawatt,
    /// Reactive power in **MVAr**.
    Megavar,
    /// Energy in **MWh**.
    Megawatthour,
    /// State of charge as a **fraction in [0,1]** (§10.2; boundary-inclusive
    /// range enforced by the P3.04 validators).
    SocFraction,
    /// Ramp rate in **MW per minute**.
    MegawattPerMinute,
    /// Duration in **seconds** (minimum up/down time, duration/recovery).
    Second,
    /// Dimensionless quantities: mode indicators, counts, availability
    /// factors.
    Dimensionless,
}

impl ConstraintUnit {
    /// All canonical units, in §10.2 order (power, reactive, energy, SOC,
    /// ramp, time, dimensionless).
    pub const ALL: [ConstraintUnit; 7] = [
        ConstraintUnit::Megawatt,
        ConstraintUnit::Megavar,
        ConstraintUnit::Megawatthour,
        ConstraintUnit::SocFraction,
        ConstraintUnit::MegawattPerMinute,
        ConstraintUnit::Second,
        ConstraintUnit::Dimensionless,
    ];

    /// The canonical name from the frozen vocabulary.
    pub const fn name(self) -> &'static str {
        match self {
            ConstraintUnit::Megawatt => "MW",
            ConstraintUnit::Megavar => "MVAr",
            ConstraintUnit::Megawatthour => "MWh",
            ConstraintUnit::SocFraction => "SOC_FRACTION",
            ConstraintUnit::MegawattPerMinute => "MW_PER_MINUTE",
            ConstraintUnit::Second => "SECOND",
            ConstraintUnit::Dimensionless => "DIMENSIONLESS",
        }
    }
}

// --- Sign conventions -------------------------------------------------------

/// The declared sign convention of a capability constraint (report §10.2).
///
/// Active power is **frozen**: P > 0 means export/injection into the grid at
/// the point of connection. Reactive power is **declared**: each grid model
/// and device profile declares whether Q > 0 means injection or consumption,
/// and the declared convention travels with the constraint — there is no
/// default and it is never inferred from the sign alone (§10.2; foundations
/// §6.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ConstraintSignConvention {
    /// Active power: **P > 0 means export** at the point of connection
    /// (frozen rule, §10.2).
    ExportPositive,
    /// Reactive power under the declared **generator convention**: Q > 0
    /// means injection into the grid.
    InjectionPositive,
    /// Reactive power under the declared **load convention**: Q > 0 means
    /// consumption from the grid.
    ConsumptionPositive,
    /// A non-negative magnitude (charge/discharge power, ramp rate,
    /// durations); the sign carries no direction semantics.
    NonNegativeMagnitude,
    /// A fraction or dimensionless quantity (SOC in [0,1], mode indicators,
    /// availability factors); sign is not applicable.
    Unsigned,
}

impl ConstraintSignConvention {
    /// All declared sign conventions.
    pub const ALL: [ConstraintSignConvention; 5] = [
        ConstraintSignConvention::ExportPositive,
        ConstraintSignConvention::InjectionPositive,
        ConstraintSignConvention::ConsumptionPositive,
        ConstraintSignConvention::NonNegativeMagnitude,
        ConstraintSignConvention::Unsigned,
    ];

    /// The canonical SCREAMING_SNAKE name from the frozen vocabulary.
    pub const fn name(self) -> &'static str {
        match self {
            ConstraintSignConvention::ExportPositive => "EXPORT_POSITIVE",
            ConstraintSignConvention::InjectionPositive => "INJECTION_POSITIVE",
            ConstraintSignConvention::ConsumptionPositive => "CONSUMPTION_POSITIVE",
            ConstraintSignConvention::NonNegativeMagnitude => "NON_NEGATIVE_MAGNITUDE",
            ConstraintSignConvention::Unsigned => "UNSIGNED",
        }
    }
}

// --- Constraint expression --------------------------------------------------

/// The structural expression of one declared constraint.
///
/// Values are plain `f64` in the constraint's declared unit. The expression is
/// validated as part of [`DeclaredConstraint::new`] (ordered bounds, non-empty
/// sets, finite numbers); a bare `ConstraintExpression` constructed in Rust is
/// not guaranteed valid until it is embedded in a validated constraint.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ConstraintExpression {
    /// A closed interval bound; each side optional (`None` = unbounded on
    /// that side). At least one side must be declared, and `lower ≤ upper`
    /// where both are present. Bounds are inclusive.
    Interval {
        /// Inclusive lower bound in the constraint's unit, if declared.
        lower: Option<f64>,
        /// Inclusive upper bound in the constraint's unit, if declared.
        upper: Option<f64>,
    },
    /// A declared discrete set of allowed numeric values (for example
    /// numeric mode indicators of `u`).
    ValueSet {
        /// The allowed values; must be non-empty and finite.
        values: Vec<f64>,
    },
    /// A declared set of allowed operating modes (for example
    /// `"CHARGE"`, `"DISCHARGE"`, `"IDLE"`); must be non-empty.
    ModeSet {
        /// The allowed mode names; must be non-empty.
        modes: Vec<String>,
    },
}

fn require_finite(value: f64) -> bool {
    value.is_finite()
}

impl ConstraintExpression {
    /// Structural validation (see the module header layering note): at least
    /// one interval side, ordered bounds, non-empty and finite sets, non-empty
    /// mode names.
    pub fn validate(&self) -> Result<(), CapabilityModelError> {
        match self {
            ConstraintExpression::Interval { lower, upper } => {
                if lower.is_none() && upper.is_none() {
                    return Err(CapabilityModelError::InvalidExpression(
                        "an interval bound must declare at least one side",
                    ));
                }
                if let Some(l) = lower {
                    if !require_finite(*l) {
                        return Err(CapabilityModelError::InvalidExpression(
                            "interval bounds must be finite",
                        ));
                    }
                }
                if let Some(u) = upper {
                    if !require_finite(*u) {
                        return Err(CapabilityModelError::InvalidExpression(
                            "interval bounds must be finite",
                        ));
                    }
                }
                if let (Some(l), Some(u)) = (lower, upper) {
                    if l > u {
                        return Err(CapabilityModelError::InvalidExpression(
                            "interval lower bound must be <= upper bound",
                        ));
                    }
                }
                Ok(())
            }
            ConstraintExpression::ValueSet { values } => {
                if values.is_empty() {
                    return Err(CapabilityModelError::InvalidExpression(
                        "a value set must contain at least one value",
                    ));
                }
                if values.iter().any(|v| !require_finite(*v)) {
                    return Err(CapabilityModelError::InvalidExpression(
                        "value-set entries must be finite",
                    ));
                }
                Ok(())
            }
            ConstraintExpression::ModeSet { modes } => {
                if modes.is_empty() {
                    return Err(CapabilityModelError::InvalidExpression(
                        "a mode set must contain at least one mode",
                    ));
                }
                if modes.iter().any(|m| m.trim().is_empty()) {
                    return Err(CapabilityModelError::InvalidExpression(
                        "mode names must be non-empty",
                    ));
                }
                Ok(())
            }
        }
    }
}

// --- Kind/unit compatibility ------------------------------------------------

/// The frozen kind/unit compatibility table (part of schema version 1;
/// recorded in the decision record). Relaxing or extending it is a schema-
/// version change per §10.5.
///
/// | Kind | Allowed units |
/// |---|---|
/// | `ACTIVE_POWER` | MW |
/// | `REACTIVE_POWER` | MVAr |
/// | `ENERGY_SOC` | MWh, SOC_FRACTION |
/// | `DEVICE_MODE` | DIMENSIONLESS |
/// | `TEMPERATURE_HEALTH` | MW, DIMENSIONLESS |
/// | `RAMP_RATE` | MW_PER_MINUTE |
/// | `MINIMUM_UP_TIME` | SECOND |
/// | `MINIMUM_DOWN_TIME` | SECOND |
/// | `DURATION_RECOVERY` | SECOND |
/// | `SITE_NETWORK` | MW, MVAr |
/// | `FORECAST_CONDITION` | MW, MVAr, MWh, DIMENSIONLESS |
/// | `COMMITMENT` | MW, MWh, DIMENSIONLESS |
fn kind_unit_compatible(kind: ConstraintKind, unit: ConstraintUnit) -> bool {
    use ConstraintKind as K;
    use ConstraintUnit as U;
    matches!(
        (kind, unit),
        (K::ActivePower, U::Megawatt)
            | (K::ReactivePower, U::Megavar)
            | (K::EnergySoc, U::Megawatthour | U::SocFraction)
            | (K::DeviceMode, U::Dimensionless)
            | (K::TemperatureHealth, U::Megawatt | U::Dimensionless)
            | (K::RampRate, U::MegawattPerMinute)
            | (K::MinimumUpTime, U::Second)
            | (K::MinimumDownTime, U::Second)
            | (K::DurationRecovery, U::Second)
            | (K::SiteNetwork, U::Megawatt | U::Megavar)
            | (
                K::ForecastCondition,
                U::Megawatt | U::Megavar | U::Megawatthour | U::Dimensionless
            )
            | (K::Commitment, U::Megawatt | U::Megawatthour | U::Dimensionless)
    )
}

// --- Declared constraint ----------------------------------------------------

/// One declared capability constraint: a generic slot with its kind, canonical
/// unit, declared sign convention, provenance, validity window, and
/// expression (see the module header for the frozen field set).
///
/// Construct via [`DeclaredConstraint::new`]; deserialization re-runs the same
/// validation, so a deserialized constraint always satisfies it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "DeclaredConstraintDto")]
pub struct DeclaredConstraint {
    /// The §11.1 category of this constraint.
    pub kind: ConstraintKind,
    /// The canonical unit of the expression values.
    pub unit: ConstraintUnit,
    /// The declared sign convention (§10.2).
    pub sign: ConstraintSignConvention,
    /// The provenance of this constraint (§11.1).
    pub provenance: Provenance,
    /// The validity window of this constraint (§10.3): the constraint applies
    /// from `valid_from` (inclusive) to `valid_until` (inclusive).
    pub validity: ValidityWindow,
    /// The structural expression of the constraint.
    pub expression: ConstraintExpression,
}

impl DeclaredConstraint {
    /// Constructs and validates a declared constraint: expression structure,
    /// and kind/unit compatibility per the frozen table.
    pub fn new(
        kind: ConstraintKind,
        unit: ConstraintUnit,
        sign: ConstraintSignConvention,
        provenance: Provenance,
        validity: ValidityWindow,
        expression: ConstraintExpression,
    ) -> Result<Self, CapabilityModelError> {
        expression.validate()?;
        if !kind_unit_compatible(kind, unit) {
            return Err(CapabilityModelError::IncompatibleKindUnit {
                kind: kind.name(),
                unit: unit.name(),
            });
        }
        Ok(Self {
            kind,
            unit,
            sign,
            provenance,
            validity,
            expression,
        })
    }
}

impl TryFrom<DeclaredConstraintDto> for DeclaredConstraint {
    type Error = CapabilityModelError;

    fn try_from(dto: DeclaredConstraintDto) -> Result<Self, Self::Error> {
        Self::new(
            dto.kind,
            dto.unit,
            dto.sign,
            dto.provenance,
            dto.validity,
            dto.expression,
        )
    }
}

#[derive(Deserialize)]
struct DeclaredConstraintDto {
    kind: ConstraintKind,
    unit: ConstraintUnit,
    sign: ConstraintSignConvention,
    provenance: Provenance,
    validity: ValidityWindow,
    expression: ConstraintExpression,
}

// --- Time-indexed constraint sets -------------------------------------------

/// The declared constraints that apply to one interval of the horizon.
///
/// Interval semantics (frozen, §10.3 / P3.03): intervals are **start-inclusive
/// and end-exclusive**; the interval index runs over
/// `[0, interval_count)` of the enclosing [`TimeIndexedFeasibleSet`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IntervalConstraints {
    /// The interval index within the horizon (0-based, start-inclusive).
    pub interval_index: u32,
    /// The constraints declared for this interval (possibly empty: an empty
    /// slice means "nothing declared here"; consumers must treat undeclared
    /// dimensions per the P3.02 provenance rule, never as unconstrained).
    pub constraints: Vec<DeclaredConstraint>,
}

/// The time-indexed feasible operating set `X(i,t)` (report §11.1): a horizon
/// start, an interval length (per-model configuration, §10.3), an interval
/// count, and the per-interval declared constraints.
///
/// Intervals are start-inclusive and end-exclusive (P3.03 frozen semantics).
/// [`TimeIndexedFeasibleSet::interval_span`] is the single source of interval
/// arithmetic at the contract level.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "TimeIndexedFeasibleSetDto")]
pub struct TimeIndexedFeasibleSet {
    /// UTC start of the first interval (start-inclusive, §10.3: UTC
    /// internally; non-UTC offsets are rejected, never converted).
    pub horizon_start: OffsetDateTime,
    /// Interval length in seconds — a per-model configuration value, not a
    /// hard-coded constant (P3.03; the scheduling-grid default of 15 minutes
    /// is a recorded recommendation, not a report requirement).
    pub interval_seconds: u32,
    /// Number of intervals in the horizon (≥ 1).
    pub interval_count: u32,
    /// Per-interval declared constraints, sorted strictly ascending by
    /// interval index. Sparse coverage is allowed: a missing slice means
    /// "nothing declared for that interval" (see [`IntervalConstraints`]).
    pub slices: Vec<IntervalConstraints>,
}

impl TimeIndexedFeasibleSet {
    /// Constructs and validates a time-indexed feasible set: UTC horizon
    /// start, positive interval length, non-empty horizon, and sorted
    /// in-range slice indices.
    pub fn new(
        horizon_start: OffsetDateTime,
        interval_seconds: u32,
        interval_count: u32,
        slices: Vec<IntervalConstraints>,
    ) -> Result<Self, CapabilityModelError> {
        if !horizon_start.offset().is_utc() {
            return Err(CapabilityModelError::NonUtcHorizonStart);
        }
        if interval_seconds == 0 {
            return Err(CapabilityModelError::NonPositiveIntervalLength);
        }
        if interval_count == 0 {
            return Err(CapabilityModelError::ZeroIntervalCount);
        }
        let mut previous: Option<u32> = None;
        for slice in &slices {
            if slice.interval_index >= interval_count {
                return Err(CapabilityModelError::IntervalIndexOutOfRange {
                    index: slice.interval_index,
                    interval_count,
                });
            }
            match previous {
                Some(p) if p == slice.interval_index => {
                    return Err(CapabilityModelError::DuplicateIntervalIndex {
                        index: slice.interval_index,
                    });
                }
                Some(p) if p > slice.interval_index => {
                    return Err(CapabilityModelError::UnsortedIntervalSlices);
                }
                _ => {}
            }
            previous = Some(slice.interval_index);
        }
        Ok(Self {
            horizon_start,
            interval_seconds,
            interval_count,
            slices,
        })
    }

    /// The start-inclusive / end-exclusive bounds of interval `index`.
    ///
    /// The start of interval `index + 1` equals the (exclusive) end of
    /// interval `index`; an instant exactly on a boundary belongs to the
    /// later interval.
    pub fn interval_span(
        &self,
        index: u32,
    ) -> Result<(OffsetDateTime, OffsetDateTime), CapabilityModelError> {
        if index >= self.interval_count {
            return Err(CapabilityModelError::IntervalIndexOutOfRange {
                index,
                interval_count: self.interval_count,
            });
        }
        let offset_secs = i64::from(index) * i64::from(self.interval_seconds);
        let start = self.horizon_start + Duration::seconds(offset_secs);
        let end = start + Duration::seconds(i64::from(self.interval_seconds));
        Ok((start, end))
    }

    /// The exclusive end of the horizon (equal to the start-inclusive start
    /// of a hypothetical interval `interval_count`).
    pub fn horizon_end(&self) -> OffsetDateTime {
        self.horizon_start
            + Duration::seconds(i64::from(self.interval_count) * i64::from(self.interval_seconds))
    }
}

impl TryFrom<TimeIndexedFeasibleSetDto> for TimeIndexedFeasibleSet {
    type Error = CapabilityModelError;

    fn try_from(dto: TimeIndexedFeasibleSetDto) -> Result<Self, Self::Error> {
        Self::new(
            dto.horizon_start,
            dto.interval_seconds,
            dto.interval_count,
            dto.slices,
        )
    }
}

#[derive(Deserialize)]
struct TimeIndexedFeasibleSetDto {
    horizon_start: OffsetDateTime,
    interval_seconds: u32,
    interval_count: u32,
    slices: Vec<IntervalConstraints>,
}

// --- The capability model ---------------------------------------------------

/// The versioned capability model of one asset (report §11.1): asset
/// identity, schema version, validity window, and the time-indexed feasible
/// set with per-constraint provenance.
///
/// Construct via [`CapabilityModel::new`]; deserialization re-runs the same
/// validation and additionally rejects any payload whose `schema_version`
/// differs from [`CAPABILITY_SCHEMA_VERSION`] (§10.5 versioning).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "CapabilityModelDto")]
pub struct CapabilityModel {
    /// The schema version of this payload. Only
    /// [`CAPABILITY_SCHEMA_VERSION`] is accepted.
    pub schema_version: u32,
    /// The asset this model describes.
    pub asset_id: FlexUuid,
    /// The validity window of the model as a whole (§10.3): the model
    /// applies from `valid_from` (inclusive) to `valid_until` (inclusive).
    /// Individual constraints carry their own (narrower or equal) windows.
    pub validity: ValidityWindow,
    /// The time-indexed feasible set `X(i,t)`.
    pub feasible_set: TimeIndexedFeasibleSet,
}

impl CapabilityModel {
    /// Constructs and validates a capability model: supported schema
    /// version, non-nil asset identifier. The feasible set is validated by
    /// its own constructor.
    pub fn new(
        schema_version: u32,
        asset_id: FlexUuid,
        validity: ValidityWindow,
        feasible_set: TimeIndexedFeasibleSet,
    ) -> Result<Self, CapabilityModelError> {
        if schema_version != CAPABILITY_SCHEMA_VERSION {
            return Err(CapabilityModelError::UnsupportedSchemaVersion {
                found: schema_version,
                supported: CAPABILITY_SCHEMA_VERSION,
            });
        }
        if asset_id == FlexUuid::NIL {
            return Err(CapabilityModelError::NilAssetIdentifier);
        }
        Ok(Self {
            schema_version,
            asset_id,
            validity,
            feasible_set,
        })
    }
}

impl TryFrom<CapabilityModelDto> for CapabilityModel {
    type Error = CapabilityModelError;

    fn try_from(dto: CapabilityModelDto) -> Result<Self, Self::Error> {
        Self::new(dto.schema_version, dto.asset_id, dto.validity, dto.feasible_set)
    }
}

#[derive(Deserialize)]
struct CapabilityModelDto {
    schema_version: u32,
    asset_id: FlexUuid,
    validity: ValidityWindow,
    feasible_set: TimeIndexedFeasibleSet,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time_model::{ValidFrom, ValidUntil};
    use time::UtcOffset;

    fn utc_millis(millis: i64) -> OffsetDateTime {
        OffsetDateTime::from_unix_timestamp_nanos(i128::from(millis) * 1_000_000).unwrap()
    }

    fn window(from_millis: i64, until_millis: i64) -> ValidityWindow {
        ValidityWindow::new(
            ValidFrom::from_unix_millis(from_millis).unwrap(),
            ValidUntil::from_unix_millis(until_millis).unwrap(),
        )
        .unwrap()
    }

    fn sample_asset() -> FlexUuid {
        FlexUuid::parse("0192a5e0-8c3d-7e2f-9a1b-3c4d5e6f7a8b").unwrap()
    }

    fn sample_constraint(interval: i64) -> DeclaredConstraint {
        DeclaredConstraint::new(
            ConstraintKind::ActivePower,
            ConstraintUnit::Megawatt,
            ConstraintSignConvention::ExportPositive,
            Provenance::Measured,
            window(interval, interval + 900_000),
            ConstraintExpression::Interval {
                lower: Some(-2.0),
                upper: Some(3.0),
            },
        )
        .unwrap()
    }

    fn sample_set() -> TimeIndexedFeasibleSet {
        TimeIndexedFeasibleSet::new(
            utc_millis(1_700_000_000_000),
            900,
            4,
            vec![
                IntervalConstraints {
                    interval_index: 0,
                    constraints: vec![sample_constraint(1_700_000_000_000)],
                },
                IntervalConstraints {
                    interval_index: 3,
                    constraints: vec![
                        sample_constraint(1_700_000_000_000),
                        DeclaredConstraint::new(
                            ConstraintKind::DeviceMode,
                            ConstraintUnit::Dimensionless,
                            ConstraintSignConvention::Unsigned,
                            Provenance::Configured,
                            window(1_700_000_000_000, 1_700_003_600_000),
                            ConstraintExpression::ModeSet {
                                modes: vec!["CHARGE".to_string(), "IDLE".to_string()],
                            },
                        )
                        .unwrap(),
                    ],
                },
            ],
        )
        .unwrap()
    }

    fn sample_model() -> CapabilityModel {
        CapabilityModel::new(
            CAPABILITY_SCHEMA_VERSION,
            sample_asset(),
            window(1_700_000_000_000, 1_700_003_600_000),
            sample_set(),
        )
        .unwrap()
    }

    // --- construction ---------------------------------------------------

    #[test]
    fn sample_model_constructs() {
        let model = sample_model();
        assert_eq!(model.schema_version, CAPABILITY_SCHEMA_VERSION);
        assert_eq!(model.asset_id, sample_asset());
        assert_eq!(model.feasible_set.interval_count, 4);
        assert_eq!(model.feasible_set.slices.len(), 2);
    }

    #[test]
    fn unsupported_schema_version_is_rejected() {
        let err = CapabilityModel::new(
            CAPABILITY_SCHEMA_VERSION + 1,
            sample_asset(),
            window(0, 1),
            sample_set(),
        )
        .unwrap_err();
        assert_eq!(
            err,
            CapabilityModelError::UnsupportedSchemaVersion {
                found: CAPABILITY_SCHEMA_VERSION + 1,
                supported: CAPABILITY_SCHEMA_VERSION,
            }
        );
    }

    #[test]
    fn nil_asset_identifier_is_rejected() {
        assert_eq!(
            CapabilityModel::new(CAPABILITY_SCHEMA_VERSION, FlexUuid::NIL, window(0, 1), sample_set())
                .unwrap_err(),
            CapabilityModelError::NilAssetIdentifier
        );
    }

    #[test]
    fn interval_boundaries_are_start_inclusive_end_exclusive() {
        let set = sample_set();
        let (s0, e0) = set.interval_span(0).unwrap();
        let (s1, _) = set.interval_span(1).unwrap();
        assert_eq!(s0, utc_millis(1_700_000_000_000));
        assert_eq!(e0, s0 + Duration::seconds(900));
        // End-exclusive: the next interval starts exactly where this one ends.
        assert_eq!(e0, s1);
        let (s_last, e_last) = set.interval_span(3).unwrap();
        assert_eq!(s_last, utc_millis(1_700_000_000_000) + Duration::seconds(2_700));
        assert_eq!(e_last, set.horizon_end());
        assert_eq!(
            set.interval_span(4),
            Err(CapabilityModelError::IntervalIndexOutOfRange {
                index: 4,
                interval_count: 4
            })
        );
    }

    #[test]
    fn set_validation_rejects_bad_grids() {
        let start = utc_millis(0);
        assert_eq!(
            TimeIndexedFeasibleSet::new(start, 0, 4, vec![]).unwrap_err(),
            CapabilityModelError::NonPositiveIntervalLength
        );
        assert_eq!(
            TimeIndexedFeasibleSet::new(start, 900, 0, vec![]).unwrap_err(),
            CapabilityModelError::ZeroIntervalCount
        );
        // Duplicate index.
        let dup = vec![
            IntervalConstraints { interval_index: 1, constraints: vec![] },
            IntervalConstraints { interval_index: 1, constraints: vec![] },
        ];
        assert_eq!(
            TimeIndexedFeasibleSet::new(start, 900, 4, dup).unwrap_err(),
            CapabilityModelError::DuplicateIntervalIndex { index: 1 }
        );
        // Unsorted slices.
        let unsorted = vec![
            IntervalConstraints { interval_index: 2, constraints: vec![] },
            IntervalConstraints { interval_index: 1, constraints: vec![] },
        ];
        assert_eq!(
            TimeIndexedFeasibleSet::new(start, 900, 4, unsorted).unwrap_err(),
            CapabilityModelError::UnsortedIntervalSlices
        );
        // Out-of-range index.
        let out_of_range = vec![IntervalConstraints { interval_index: 4, constraints: vec![] }];
        assert_eq!(
            TimeIndexedFeasibleSet::new(start, 900, 4, out_of_range).unwrap_err(),
            CapabilityModelError::IntervalIndexOutOfRange { index: 4, interval_count: 4 }
        );
    }

    #[test]
    fn non_utc_horizon_start_is_rejected_not_converted() {
        let non_utc = utc_millis(0).replace_offset(UtcOffset::from_hms(2, 0, 0).unwrap());
        assert_eq!(
            TimeIndexedFeasibleSet::new(non_utc, 900, 4, vec![]).unwrap_err(),
            CapabilityModelError::NonUtcHorizonStart
        );
    }

    #[test]
    fn expression_validation_rejects_structural_errors() {
        let v = window(0, 1);
        // Both interval sides absent.
        assert!(matches!(
            DeclaredConstraint::new(
                ConstraintKind::ActivePower,
                ConstraintUnit::Megawatt,
                ConstraintSignConvention::ExportPositive,
                Provenance::Measured,
                v,
                ConstraintExpression::Interval { lower: None, upper: None },
            ),
            Err(CapabilityModelError::InvalidExpression(_))
        ));
        // Inverted bounds.
        assert!(matches!(
            DeclaredConstraint::new(
                ConstraintKind::ActivePower,
                ConstraintUnit::Megawatt,
                ConstraintSignConvention::ExportPositive,
                Provenance::Measured,
                v,
                ConstraintExpression::Interval { lower: Some(5.0), upper: Some(2.0) },
            ),
            Err(CapabilityModelError::InvalidExpression(_))
        ));
        // Empty value set / empty mode set.
        assert!(matches!(
            DeclaredConstraint::new(
                ConstraintKind::DeviceMode,
                ConstraintUnit::Dimensionless,
                ConstraintSignConvention::Unsigned,
                Provenance::Configured,
                v,
                ConstraintExpression::ValueSet { values: vec![] },
            ),
            Err(CapabilityModelError::InvalidExpression(_))
        ));
        assert!(matches!(
            DeclaredConstraint::new(
                ConstraintKind::DeviceMode,
                ConstraintUnit::Dimensionless,
                ConstraintSignConvention::Unsigned,
                Provenance::Configured,
                v,
                ConstraintExpression::ModeSet { modes: vec![] },
            ),
            Err(CapabilityModelError::InvalidExpression(_))
        ));
        // Non-finite bound.
        assert!(matches!(
            DeclaredConstraint::new(
                ConstraintKind::ActivePower,
                ConstraintUnit::Megawatt,
                ConstraintSignConvention::ExportPositive,
                Provenance::Measured,
                v,
                ConstraintExpression::Interval { lower: Some(f64::NAN), upper: Some(1.0) },
            ),
            Err(CapabilityModelError::InvalidExpression(_))
        ));
    }

    #[test]
    fn kind_unit_incompatibility_is_rejected() {
        let v = window(0, 1);
        // Ramp rate in MWh is meaningless; the frozen table rejects it.
        assert_eq!(
            DeclaredConstraint::new(
                ConstraintKind::RampRate,
                ConstraintUnit::Megawatthour,
                ConstraintSignConvention::NonNegativeMagnitude,
                Provenance::Measured,
                v,
                ConstraintExpression::Interval { lower: Some(0.0), upper: Some(1.0) },
            )
            .unwrap_err(),
            CapabilityModelError::IncompatibleKindUnit { kind: "RAMP_RATE", unit: "MWh" }
        );
        // Every kind accepts at least one unit from the frozen table.
        for kind in ConstraintKind::ALL {
            let ok = ConstraintUnit::ALL
                .iter()
                .any(|&u| kind_unit_compatible(kind, u));
            assert!(ok, "kind {} has no compatible unit", kind.name());
        }
    }

    // --- vocabularies ---------------------------------------------------

    #[test]
    fn constraint_unit_vocabulary_has_no_scaled_units() {
        // The frozen unit vocabulary is canonical only: no kW/kWh variant
        // exists to serialize, and the name set is pinned exactly.
        let names: Vec<&str> = ConstraintUnit::ALL.iter().map(|u| u.name()).collect();
        assert_eq!(
            names,
            vec!["MW", "MVAr", "MWh", "SOC_FRACTION", "MW_PER_MINUTE", "SECOND", "DIMENSIONLESS"]
        );
        for name in &names {
            let upper = name.to_ascii_uppercase();
            assert!(!upper.contains("KW") && !upper.contains("KWH"));
        }
    }

    #[test]
    fn constraint_kind_vocabulary_covers_the_report_categories() {
        assert_eq!(ConstraintKind::ALL.len(), 12);
        let names: Vec<&str> = ConstraintKind::ALL.iter().map(|k| k.name()).collect();
        assert_eq!(
            names,
            vec![
                "ACTIVE_POWER",
                "REACTIVE_POWER",
                "ENERGY_SOC",
                "DEVICE_MODE",
                "TEMPERATURE_HEALTH",
                "RAMP_RATE",
                "MINIMUM_UP_TIME",
                "MINIMUM_DOWN_TIME",
                "DURATION_RECOVERY",
                "SITE_NETWORK",
                "FORECAST_CONDITION",
                "COMMITMENT",
            ]
        );
    }

    // --- serde round trips ----------------------------------------------

    #[test]
    fn capability_model_json_round_trip() {
        let model = sample_model();
        let json = serde_json::to_value(&model).unwrap();
        assert_eq!(json["schema_version"], serde_json::json!(1));
        let back: CapabilityModel = serde_json::from_value(json).unwrap();
        assert_eq!(back, model);
    }

    #[test]
    fn serde_rejects_unsupported_schema_version() {
        let mut json = serde_json::to_value(&sample_model()).unwrap();
        json["schema_version"] = serde_json::json!(2);
        let err = serde_json::from_value::<CapabilityModel>(json).unwrap_err();
        assert!(err.to_string().contains("schema version 2 is not supported"));
    }

    #[test]
    fn serde_rejects_non_utc_horizon_start() {
        let mut json = serde_json::to_value(&sample_model()).unwrap();
        json["feasible_set"]["horizon_start"] = serde_json::json!("1970-01-01T02:00:00+02:00");
        let err = serde_json::from_value::<CapabilityModel>(json).unwrap_err();
        assert!(err.to_string().contains("UTC"));
    }

    #[test]
    fn serde_rejects_structurally_invalid_constraints() {
        // Inverted interval bounds never deserialize (validation re-runs on
        // the deserialization path).
        let mut json = serde_json::to_value(&sample_model()).unwrap();
        json["feasible_set"]["slices"][0]["constraints"][0]["expression"] =
            serde_json::json!({ "Interval": { "lower": 5.0, "upper": 2.0 } });
        let err = serde_json::from_value::<CapabilityModel>(json).unwrap_err();
        assert!(err.to_string().contains("lower bound must be <= upper bound"));

        // An incompatible kind/unit pair never deserializes.
        let mut json = serde_json::to_value(&sample_model()).unwrap();
        json["feasible_set"]["slices"][0]["constraints"][0]["kind"] = serde_json::json!("RAMP_RATE");
        json["feasible_set"]["slices"][0]["constraints"][0]["unit"] =
            serde_json::json!("MWh");
        let err = serde_json::from_value::<CapabilityModel>(json).unwrap_err();
        assert!(err.to_string().contains("RAMP_RATE"));
    }

    #[test]
    fn serde_rejects_inverted_validity_windows() {
        let mut json = serde_json::to_value(&sample_model()).unwrap();
        json["validity"] = serde_json::json!({
            "valid_from": "1970-01-01T00:10:00Z",
            "valid_until": "1970-01-01T00:05:00Z",
        });
        let err = serde_json::from_value::<CapabilityModel>(json).unwrap_err();
        assert!(err.to_string().contains("valid_from"));
    }

    #[test]
    fn provenance_serde_round_trips_all_values() {
        for p in Provenance::ALL {
            let json = serde_json::to_value(&p).unwrap();
            assert_eq!(json, serde_json::json!(p.name()));
            let back: Provenance = serde_json::from_value(json).unwrap();
            assert_eq!(back, p);
        }
        assert!(serde_json::from_value::<Provenance>(serde_json::json!("GUESSED")).is_err());
    }

    #[test]
    fn constraint_kind_serde_round_trips_all_values() {
        for k in ConstraintKind::ALL {
            let back: ConstraintKind =
                serde_json::from_value(serde_json::to_value(&k).unwrap()).unwrap();
            assert_eq!(back, k);
        }
    }

    #[test]
    fn constraint_unit_serde_round_trips_and_rejects_scaled_units() {
        for u in ConstraintUnit::ALL {
            let back: ConstraintUnit =
                serde_json::from_value(serde_json::to_value(&u).unwrap()).unwrap();
            assert_eq!(back, u);
        }
        // kW/kWh are not in the vocabulary and can never deserialize.
        assert!(serde_json::from_value::<ConstraintUnit>(serde_json::json!("kW")).is_err());
        assert!(serde_json::from_value::<ConstraintUnit>(serde_json::json!("kWh")).is_err());
    }

    #[test]
    fn sign_convention_serde_round_trips_all_values() {
        for s in ConstraintSignConvention::ALL {
            let back: ConstraintSignConvention =
                serde_json::from_value(serde_json::to_value(&s).unwrap()).unwrap();
            assert_eq!(back, s);
        }
    }

    #[test]
    fn constraint_expression_serde_round_trips_all_variants() {
        let expressions = vec![
            ConstraintExpression::Interval { lower: Some(-1.5), upper: Some(2.0) },
            ConstraintExpression::Interval { lower: None, upper: Some(4.0) },
            ConstraintExpression::ValueSet { values: vec![0.0, 1.0, 2.0] },
            ConstraintExpression::ModeSet {
                modes: vec!["CHARGE".to_string(), "DISCHARGE".to_string()],
            },
        ];
        for e in expressions {
            let back: ConstraintExpression =
                serde_json::from_value(serde_json::to_value(&e).unwrap()).unwrap();
            assert_eq!(back, e);
        }
    }

    #[test]
    fn interval_constraints_serde_round_trip() {
        let slice = IntervalConstraints {
            interval_index: 7,
            constraints: vec![sample_constraint(0)],
        };
        let back: IntervalConstraints =
            serde_json::from_value(serde_json::to_value(&slice).unwrap()).unwrap();
        assert_eq!(back, slice);
    }
}
