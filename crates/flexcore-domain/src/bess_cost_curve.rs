// FIM-F11 (Wave 5) — Measured-BESS degradation cost curves (P15.08
// continued): DOD × mean-SOC histogram bins → MWh-equivalent degradation
// cost as LINEAR / PIECEWISE-LINEAR optimizer INPUT DATA feeding the
// existing `C_degradation` term (pure domain computation, no I/O, no
// solver).
//
// IMPLEMENTED-UNDER-FIM-F11, WRITTEN-UNRUN: this module was written under
// the Final Implementation Milestone (Wave 5, feature F-11) and has never
// been compiled or executed. The first human build gates it; the pinned
// vectors in `tests/bess_fim_cost_curve.rs` are python-computed
// expectations, never runs.
//
// HUMAN LAB GATE (the honesty line, visible in the type — every parameter
// carries [`CostParameterStatus`]): every parameter in this module's
// recorded DEFAULT tables (`lfp_planning_aid_default`,
// `nmc_planning_aid_default`) is a SECONDARY-SOURCE planning-aid SHAPE VALUE
// labeled `UnverifiedSecondary` — no measured fleet exists and no recorded
// source in this repository pins a value. The P15-E-3 decision (chemistry
// coverage: LFP/NMC first is the RECOMMENDATION; the decision stays OPEN)
// plus measured-fleet calibration MUST confirm every parameter before
// production use. Nothing here is measured data; measured values arrive as
// INPUT. No measured value is invented anywhere in this module.
//
// THE MAPPING (declared convention, decided once — not physics):
//
//   cost per cycle in bin (i, j) =
//       base_cost_per_full_equivalent_cycle
//       * dod_mid(i)^dod_exponent
//       * (1 + mean_soc_dwell_coefficient * mean_soc_mid(j))
//   throughput per cycle in bin (i, j) = 2 * dod_mid(i) * usable_energy_mwh
//
// - The representative DOD / mean SOC of a bin is its MIDPOINT (declared).
// - A depth-d cycle charges d·E and discharges d·E — the P15.07 FEC
//   normalization convention applied in reverse; throughput is MWh
//   (canonical units freeze untouched; SOC/DOD are fractions).
// - The LINEAR-equivalent cost is total bin cost / total bin throughput
//   (MWh-equivalent: the cost expressed per MWh of throughput — the exact
//   shape the P3.06 linear placeholder used, now computed FROM MEASURED
//   histogram data instead of a configured rate).
// - The PIECEWISE-LINEAR projection lists the bins that carry cycles,
//   sorted by implied marginal cost (cost per cycle / throughput per cycle)
//   ASCENDING, each contributing a segment over its cumulative throughput;
//   adjacent segments with equal marginal cost merge. The ascending sort
//   makes the projection CONVEX by construction — the optimizer data
//   contract refuses non-convex piecewise-linear data (reshape the data,
//   never the solver; conventions §6.5) — and the PWL total cost at full
//   consumption equals the histogram total cost exactly (pinned).
// - An empty histogram (no recorded cycles) is REFUSED with a named error:
//   a zero rate would be indistinguishable from "no data" (P3.02 posture).
//
// ADR-010 (conventions §6.3): a degradation cost is INPUT DATA. It never
// authorizes or produces a dispatch command.
//
// THE C_degradation WIRING (the recorded split, P15.08): the optimizer
// INPUT shape is `flexcore-optimization-api::degradation_cost::
// DegradationCostDeclaration` (linear or piecewise-linear, per asset, with
// a required version and provenance note). This crate depends on
// flexcore-contracts only, so [`BessDegradationCurveShape`] and
// [`BessDegradationCostInput`] mirror that contract FIELD FOR FIELD by
// citation (the recorded mirror posture); the application layer maps
// [`BessDegradationCostInput`] into a `DegradationCostDeclaration` feeding
// the existing `C_degradation` term. The P3.06 LINEAR-throughput default in
// `bess_model` stays available and default — the curve is OPT-IN per asset
// configuration; nothing here changes `bess_model` semantics.
//
// Control status: pure domain computation for the simulation-only 0.x line.
// No I/O, no solver coupling, no device-write path, no dispatch reach.

use flexcore_contracts::capability::Provenance;

use crate::bess_degradation::{
    weaker_provenance, DegradationChemistry, DegradationLedgerError, DodMeanSocHistogram,
};
use crate::bess_model::ProvenancedValue;
use crate::ids::AssetId;
use crate::rainflow::RainflowDodMeanSocHistogram;

// --- Recorded statements -----------------------------------------------------

/// The recorded mapping statement (visible in records and exports, not a
/// comment).
pub const DEGRADATION_COST_MAPPING_STATEMENT: &str =
    "Histogram-to-cost mapping (declared convention, FIM-F11): cost per \
cycle = base * DOD_mid^exponent * (1 + dwell coefficient * mean-SOC mid); \
throughput per cycle = 2 * DOD_mid * usable energy; the linear-equivalent \
cost is total cost / total throughput (MWh); the piecewise-linear \
projection sorts bins by implied marginal cost ascending (convex by \
construction; the PWL total at full consumption equals the histogram total \
exactly). Parameters are secondary-source values, UNVERIFIED-SECONDARY \
where no recorded source exists; the P15-E-3 closure + measured-fleet \
calibration is the HUMAN LAB GATE. A degradation cost is INPUT DATA, never \
a dispatch command (ADR-010).";

/// The provenance note composed into every projection whose parameters are
/// UNVERIFIED-SECONDARY (bounded to the optimizer contract's 256-character
/// provenance-note limit so the mapping into a `DegradationCostDeclaration`
/// cannot fail downstream).
pub const UNVERIFIED_SECONDARY_PROVENANCE_NOTE: &str =
    "UNVERIFIED-SECONDARY planning-aid values (FIM-F11; source class: \
BLAST-Lite-parameterized cycle-aging family). Human gate: P15-E-3 closure \
+ fleet calibration before production use. Degradation cost is INPUT \
DATA, never a dispatch command (ADR-010).";

/// The provenance note composed into every projection whose parameters all
/// carry `RecordedSecondary` status (still secondary — never measured).
pub const RECORDED_SECONDARY_PROVENANCE_NOTE: &str =
    "Recorded-secondary parameters (FIM-F11; declared from repository \
records; still secondary, never measured). Human gate: P15-E-3 closure + \
fleet calibration before production use. Degradation cost is INPUT DATA, \
never a dispatch command (ADR-010).";

/// The source-class label carried by every parameter of the recorded
/// planning-aid DEFAULT tables (LFP/NMC first per the P15-E-3 RECOMMENDATION
/// [S-146][S-147] — scope reference only, no value is taken from any text).
pub const PLANNING_AID_SOURCE_CLASS: &str =
    "PLANNING-AID SHAPE VALUE: no recorded source in this repository; \
UNVERIFIED-SECONDARY secondary-source class (BLAST-Lite-parameterized \
semi-empirical cycle-aging family, scope reference only [S-146][S-147])";

/// The declared curve version of the LFP planning-aid default table. The
/// `-unverified` suffix is part of the recorded version: the P3.15
/// no-untracked-constants discipline, applied to an unverified table.
pub const LFP_PLANNING_AID_CURVE_VERSION: &str = "fim-f11-lfp-planning-aid-v0-unverified";

/// The declared curve version of the NMC planning-aid default table.
pub const NMC_PLANNING_AID_CURVE_VERSION: &str = "fim-f11-nmc-planning-aid-v0-unverified";

// --- Errors ------------------------------------------------------------------

/// Errors of the cost-curve mapping. Each variant names the offending
/// field/value — never a bare "invalid input" (P3.04 error-catalogue style).
#[derive(Debug, Clone, PartialEq)]
pub enum CostCurveError {
    /// The declared chemistry is outside the P15-E-3 DEFAULT scope: no
    /// degradation parameter is credited for it — a RECORDED LIMITATION,
    /// never a silent extrapolation.
    OutOfScopeChemistry {
        /// The declared chemistry label.
        label: String,
    },
    /// A numeric datum was NaN or infinite.
    NonFiniteValue {
        /// The offending field.
        field: &'static str,
        /// The offending value.
        value: f64,
    },
    /// A magnitude that must be non-negative was negative.
    NegativeValue {
        /// The offending field.
        field: &'static str,
        /// The offending value.
        value: f64,
    },
    /// A magnitude that must be strictly positive was zero or negative.
    NonPositiveValue {
        /// The offending field.
        field: &'static str,
        /// The offending value.
        value: f64,
    },
    /// A fraction was outside its declared inclusive range.
    ValueOutOfRange {
        /// The offending field.
        field: &'static str,
        /// The offending value.
        value: f64,
        /// The inclusive lower bound.
        lower: f64,
        /// The inclusive upper bound.
        upper: f64,
    },
    /// The DOD exponent was below 1.0 (a sub-linear depth response would
    /// price deep cycles below shallow ones per unit of depth — refused).
    ExponentBelowOne {
        /// The offending value.
        value: f64,
    },
    /// The usable energy carries UNKNOWN provenance: no MWh normalization is
    /// credited from unknown data (P3.02 — unknown narrows, never widens).
    UsableEnergyNotCreditable,
    /// The histogram carried no recorded cycles: nothing is priced and no
    /// zero rate is invented (P3.02 posture).
    EmptyHistogram,
    /// The per-bin count table did not match the declared bin grid.
    BinCountMismatch {
        /// The expected entry count (dod bins × mean-SOC bins).
        expected: usize,
        /// The offered entry count.
        found: usize,
    },
    /// The curve was declared without a version — an untracked constant is a
    /// validation error (the P3.15 discipline).
    UnversionedCurve,
    /// A required text field was empty or whitespace.
    EmptyField {
        /// The offending field.
        field: &'static str,
    },
    /// A text field exceeded its declared bound.
    FieldTooLong {
        /// The offending field.
        field: &'static str,
        /// The recorded bound.
        max_chars: usize,
    },
    /// A ledger error surfaced through the P15.07 histogram adapter.
    Ledger(DegradationLedgerError),
}

impl std::fmt::Display for CostCurveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CostCurveError::OutOfScopeChemistry { label } => write!(
                f,
                "chemistry '{label}' is outside the P15-E-3 DEFAULT scope; no degradation \
                 parameter is credited for it (recorded limitation, never a silent \
                 extrapolation)"
            ),
            CostCurveError::NonFiniteValue { field, value } => {
                write!(f, "{field} must be finite (found {value})")
            }
            CostCurveError::NegativeValue { field, value } => {
                write!(f, "{field} must be >= 0 (found {value})")
            }
            CostCurveError::NonPositiveValue { field, value } => {
                write!(f, "{field} must be > 0 (found {value})")
            }
            CostCurveError::ValueOutOfRange { field, value, lower, upper } => write!(
                f,
                "{field} must be in [{lower}, {upper}] inclusive (found {value})"
            ),
            CostCurveError::ExponentBelowOne { value } => write!(
                f,
                "the DOD exponent must be >= 1.0 so deeper cycles are never priced below \
                 shallower ones per unit of depth (found {value})"
            ),
            CostCurveError::UsableEnergyNotCreditable => write!(
                f,
                "the usable energy carries UNKNOWN provenance; no MWh normalization is \
                 credited from unknown data (P3.02)"
            ),
            CostCurveError::EmptyHistogram => write!(
                f,
                "the histogram carries no recorded cycles; nothing is priced and no zero \
                 rate is invented (P3.02)"
            ),
            CostCurveError::BinCountMismatch { expected, found } => write!(
                f,
                "the per-bin count table has {found} entries but the declared bin grid has \
                 {expected}"
            ),
            CostCurveError::UnversionedCurve => write!(
                f,
                "the cost curve has no version: an untracked input is a validation error \
                 (P3.15 — no untracked constants)"
            ),
            CostCurveError::EmptyField { field } => {
                write!(f, "{field} must be non-empty and non-whitespace")
            }
            CostCurveError::FieldTooLong { field, max_chars } => {
                write!(f, "{field} exceeds the {max_chars}-character bound")
            }
            CostCurveError::Ledger(error) => write!(f, "degradation ledger error: {error}"),
        }
    }
}

impl std::error::Error for CostCurveError {}

fn validate_text(field: &'static str, value: &str) -> Result<(), CostCurveError> {
    if value.trim().is_empty() {
        return Err(CostCurveError::EmptyField { field });
    }
    if value.len() > 256 {
        return Err(CostCurveError::FieldTooLong { field, max_chars: 256 });
    }
    Ok(())
}

// --- Parameters ---------------------------------------------------------------

/// The declared status of a cost-curve parameter (the honesty line, visible
/// in the type — never a comment).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CostParameterStatus {
    /// No recorded source exists in the repository: UNVERIFIED-SECONDARY —
    /// the human gate (P15-E-3 closure + measured-fleet calibration) must
    /// confirm the value before production use.
    UnverifiedSecondary,
    /// A value recorded from a secondary source in the repository's decision
    /// records. STILL SECONDARY — never measured; measured values arrive as
    /// INPUT.
    RecordedSecondary,
}

/// One declared cost-curve parameter: its value, the source class it claims,
/// its status, and its provenance (P3.02 — the provenance merges weakest-wins
/// into the projection).
#[derive(Debug, Clone, PartialEq)]
pub struct CostCurveParameter {
    /// The declared value.
    pub value: f64,
    /// The declared source class (recorded data naming where the value
    /// claims to come from).
    pub source_class: String,
    /// The declared status (UNVERIFIED-SECONDARY or recorded-secondary).
    pub status: CostParameterStatus,
    /// The provenance of the declared value.
    pub provenance: Provenance,
}

impl CostCurveParameter {
    /// Constructs and validates a parameter (finite value, non-empty bounded
    /// source class).
    pub fn new(
        value: f64,
        source_class: String,
        status: CostParameterStatus,
        provenance: Provenance,
    ) -> Result<Self, CostCurveError> {
        if !value.is_finite() {
            return Err(CostCurveError::NonFiniteValue { field: "value", value });
        }
        validate_text("source_class", &source_class)?;
        Ok(Self {
            value,
            source_class,
            status,
            provenance,
        })
    }
}

/// The per-chemistry cycle-cost parameter table (P15-E-3: LFP and NMC first,
/// per the RECOMMENDATION [S-146][S-147]; the decision stays OPEN — agents
/// never close it). An out-of-scope chemistry is refused with a named error:
/// a recorded limitation, never a silent extrapolation.
///
/// The declared FORM is a convention, not physics:
/// `cost_per_cycle(d, m) = base * d^exponent * (1 + coefficient * m)`.
#[derive(Debug, Clone, PartialEq)]
pub struct BessCycleCostParameters {
    /// The declared chemistry scope.
    pub chemistry: DegradationChemistry,
    /// The base cost per full equivalent cycle (currency-units; the cost of
    /// one full-depth cycle at mean SOC 0 under the declared form).
    pub base_cost_per_full_equivalent_cycle: CostCurveParameter,
    /// The DOD exponent (>= 1.0: deeper cycles are never priced below
    /// shallower ones per unit of depth).
    pub dod_exponent: CostCurveParameter,
    /// The mean-SOC dwell coefficient (>= 0: the high-SOC dwell factor).
    pub mean_soc_dwell_coefficient: CostCurveParameter,
}

impl BessCycleCostParameters {
    /// Constructs and validates a parameter table. The chemistry must be
    /// inside the P15-E-3 DEFAULT scope (LFP/NMC first [S-147]); every
    /// parameter is validated (finite values, exponent >= 1, coefficient
    /// >= 0, bounded source classes).
    pub fn new(
        chemistry: DegradationChemistry,
        base_cost_per_full_equivalent_cycle: CostCurveParameter,
        dod_exponent: CostCurveParameter,
        mean_soc_dwell_coefficient: CostCurveParameter,
    ) -> Result<Self, CostCurveError> {
        if !chemistry.is_in_scope() {
            return Err(CostCurveError::OutOfScopeChemistry {
                label: chemistry.label(),
            });
        }
        if base_cost_per_full_equivalent_cycle.value < 0.0 {
            return Err(CostCurveError::NegativeValue {
                field: "base_cost_per_full_equivalent_cycle",
                value: base_cost_per_full_equivalent_cycle.value,
            });
        }
        if dod_exponent.value < 1.0 {
            return Err(CostCurveError::ExponentBelowOne { value: dod_exponent.value });
        }
        if mean_soc_dwell_coefficient.value < 0.0 {
            return Err(CostCurveError::NegativeValue {
                field: "mean_soc_dwell_coefficient",
                value: mean_soc_dwell_coefficient.value,
            });
        }
        Ok(Self {
            chemistry,
            base_cost_per_full_equivalent_cycle,
            dod_exponent,
            mean_soc_dwell_coefficient,
        })
    }

    /// The recorded LFP planning-aid DEFAULT table: UNVERIFIED-SECONDARY
    /// shape values (see the module header — the HUMAN LAB GATE).
    pub fn lfp_planning_aid_default() -> Result<Self, CostCurveError> {
        let source_class = PLANNING_AID_SOURCE_CLASS.to_string();
        Ok(Self {
            chemistry: DegradationChemistry::Lfp,
            base_cost_per_full_equivalent_cycle: CostCurveParameter::new(
                20.0,
                source_class.clone(),
                CostParameterStatus::UnverifiedSecondary,
                Provenance::Configured,
            )?,
            dod_exponent: CostCurveParameter::new(
                2.0,
                source_class.clone(),
                CostParameterStatus::UnverifiedSecondary,
                Provenance::Configured,
            )?,
            mean_soc_dwell_coefficient: CostCurveParameter::new(
                0.5,
                source_class,
                CostParameterStatus::UnverifiedSecondary,
                Provenance::Configured,
            )?,
        })
    }

    /// The recorded NMC planning-aid DEFAULT table: UNVERIFIED-SECONDARY
    /// shape values (see the module header — the HUMAN LAB GATE).
    pub fn nmc_planning_aid_default() -> Result<Self, CostCurveError> {
        let source_class = PLANNING_AID_SOURCE_CLASS.to_string();
        Ok(Self {
            chemistry: DegradationChemistry::Nmc,
            base_cost_per_full_equivalent_cycle: CostCurveParameter::new(
                25.0,
                source_class.clone(),
                CostParameterStatus::UnverifiedSecondary,
                Provenance::Configured,
            )?,
            dod_exponent: CostCurveParameter::new(
                2.0,
                source_class.clone(),
                CostParameterStatus::UnverifiedSecondary,
                Provenance::Configured,
            )?,
            mean_soc_dwell_coefficient: CostCurveParameter::new(
                0.8,
                source_class,
                CostParameterStatus::UnverifiedSecondary,
                Provenance::Configured,
            )?,
        })
    }

    /// Whether ANY parameter of the table is UNVERIFIED-SECONDARY (the flag
    /// the projection records for the orchestrator).
    pub fn is_unverified_secondary(&self) -> bool {
        self.base_cost_per_full_equivalent_cycle.status == CostParameterStatus::UnverifiedSecondary
            || self.dod_exponent.status == CostParameterStatus::UnverifiedSecondary
            || self.mean_soc_dwell_coefficient.status == CostParameterStatus::UnverifiedSecondary
    }

    /// The declared per-cycle cost at depth `dod` and mean SOC `mean_soc`
    /// under the declared form. Inputs are validated (named errors).
    pub fn cost_per_cycle(&self, dod: f64, mean_soc: f64) -> Result<f64, CostCurveError> {
        if !dod.is_finite() {
            return Err(CostCurveError::NonFiniteValue { field: "dod", value: dod });
        }
        if dod <= 0.0 || dod > 1.0 {
            return Err(CostCurveError::ValueOutOfRange {
                field: "dod",
                value: dod,
                lower: f64::EPSILON,
                upper: 1.0,
            });
        }
        if !mean_soc.is_finite() {
            return Err(CostCurveError::NonFiniteValue { field: "mean_soc", value: mean_soc });
        }
        if !(0.0..=1.0).contains(&mean_soc) {
            return Err(CostCurveError::ValueOutOfRange {
                field: "mean_soc",
                value: mean_soc,
                lower: 0.0,
                upper: 1.0,
            });
        }
        Ok(self.base_cost_per_full_equivalent_cycle.value
            * dod.powf(self.dod_exponent.value)
            * (1.0 + self.mean_soc_dwell_coefficient.value * mean_soc))
    }

    /// The declared throughput of one depth-`dod` cycle against the usable
    /// energy: `2 * dod * usable_energy_mwh` (the P15.07 FEC normalization
    /// applied in reverse — MWh, canonical units).
    pub fn throughput_per_cycle(
        usable_energy_mwh: f64,
        dod: f64,
    ) -> Result<f64, CostCurveError> {
        if !usable_energy_mwh.is_finite() {
            return Err(CostCurveError::NonFiniteValue {
                field: "usable_energy_mwh",
                value: usable_energy_mwh,
            });
        }
        if usable_energy_mwh <= 0.0 {
            return Err(CostCurveError::NonPositiveValue {
                field: "usable_energy_mwh",
                value: usable_energy_mwh,
            });
        }
        if !dod.is_finite() {
            return Err(CostCurveError::NonFiniteValue { field: "dod", value: dod });
        }
        if dod <= 0.0 || dod > 1.0 {
            return Err(CostCurveError::ValueOutOfRange {
                field: "dod",
                value: dod,
                lower: f64::EPSILON,
                upper: 1.0,
            });
        }
        Ok(2.0 * dod * usable_energy_mwh)
    }
}

// --- The projection -------------------------------------------------------------

/// One piecewise-linear segment of the projected curve. Mirrors
/// `flexcore-optimization-api::degradation_cost::PwlSegment` FIELD FOR FIELD
/// by citation (this crate depends on flexcore-contracts only — the recorded
/// mirror posture).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BessPwlSegment {
    /// The cumulative-throughput bound of this segment in MWh (strictly
    /// ascending across the segment list).
    pub cumulative_throughput_limit_mwh: f64,
    /// The marginal cost per MWh of throughput within this segment (>= 0).
    pub marginal_cost_per_mwh: f64,
}

/// The projected curve shape. Mirrors
/// `flexcore-optimization-api::degradation_cost::DegradationCostCurve` FIELD
/// FOR FIELD by citation.
#[derive(Debug, Clone, PartialEq)]
pub enum BessDegradationCurveShape {
    /// A linear throughput penalty: one cost rate per MWh of (charge +
    /// discharge) throughput.
    Linear {
        /// The cost rate per MWh of throughput (>= 0).
        cost_per_mwh_throughput: f64,
    },
    /// A piecewise-linear cost over cumulative throughput (convex by
    /// construction — see the module header).
    PiecewiseLinear {
        /// The declared segments (non-empty, strictly ascending).
        segments: Vec<BessPwlSegment>,
    },
}

/// The cost projection of one asset's measured histogram under one declared
/// parameter table: both the linear-equivalent rate and the piecewise-linear
/// segments, the exact totals, and the merged provenance record.
#[derive(Debug, Clone, PartialEq)]
pub struct DegradationCostProjection {
    /// The declared curve version (the parameters' recorded version; the
    /// P3.15 no-untracked-constants discipline).
    pub curve_version: String,
    /// The declared chemistry scope (in-scope by construction — the mapping
    /// refuses out-of-scope chemistries).
    pub chemistry: DegradationChemistry,
    /// The MWh-equivalent cost: total bin cost / total bin throughput (MWh).
    pub linear_equivalent_cost_per_mwh: f64,
    /// The piecewise-linear projection (convex by construction; the PWL
    /// total cost at full consumption equals `total_cost` exactly).
    pub piecewise_segments: Vec<BessPwlSegment>,
    /// The total cost over the recorded cycles (currency-units).
    pub total_cost: f64,
    /// The total throughput of the recorded cycles (MWh).
    pub total_throughput_mwh: f64,
    /// The merged provenance (the weakest of the histogram, usable-energy,
    /// and parameter provenances — unknown narrows, never widens).
    pub provenance: Provenance,
    /// True when any parameter of the table is UNVERIFIED-SECONDARY.
    pub parameters_unverified_secondary: bool,
    /// The composed provenance note (bounded to the optimizer contract's
    /// 256-character limit).
    pub provenance_note: String,
    /// The recorded mapping statement ([`DEGRADATION_COST_MAPPING_STATEMENT`]).
    pub statement: &'static str,
}

/// One bin's contribution to the projection (internal accumulation record).
struct BinCost {
    bin_index: usize,
    cycles: f64,
    throughput: f64,
    cost: f64,
    marginal_cost_per_mwh: f64,
}

fn usable_energy_creditable(usable_energy: &ProvenancedValue) -> Result<f64, CostCurveError> {
    if !usable_energy.value.is_finite() {
        return Err(CostCurveError::NonFiniteValue {
            field: "usable_energy_mwh",
            value: usable_energy.value,
        });
    }
    if usable_energy.value <= 0.0 {
        return Err(CostCurveError::NonPositiveValue {
            field: "usable_energy_mwh",
            value: usable_energy.value,
        });
    }
    if usable_energy.provenance == Provenance::Unknown {
        return Err(CostCurveError::UsableEnergyNotCreditable);
    }
    Ok(usable_energy.value)
}

/// The mapping core over a validated bin grid: `cycles_per_bin` holds the
/// per-bin CYCLE counts (row-major, `dod_bin * mean_soc_bins + soc_bin`;
/// fractional values are legal — half cycles count 0.5). Returns the exact
/// totals, the linear-equivalent rate, and the convex PWL projection.
fn project_bins(
    parameters: &BessCycleCostParameters,
    usable_energy_mwh: f64,
    dod_bin_edges: &[f64],
    mean_soc_bin_edges: &[f64],
    cycles_per_bin: &[f64],
) -> Result<DegradationCostProjection, CostCurveError> {
    let dod_bins = dod_bin_edges.len() - 1;
    let soc_bins = mean_soc_bin_edges.len() - 1;
    let expected = dod_bins * soc_bins;
    if cycles_per_bin.len() != expected {
        return Err(CostCurveError::BinCountMismatch {
            expected,
            found: cycles_per_bin.len(),
        });
    }
    let mut bins: Vec<BinCost> = Vec::new();
    let mut total_cost = 0.0f64;
    let mut total_throughput = 0.0f64;
    for dod_bin in 0..dod_bins {
        // The representative DOD / mean SOC of a bin is its midpoint
        // (declared convention; the first DOD edge is 0.0 and the edges are
        // strictly ascending, so every DOD midpoint is strictly positive).
        let dod_mid = 0.5 * (dod_bin_edges[dod_bin] + dod_bin_edges[dod_bin + 1]);
        let throughput_per_cycle = 2.0 * dod_mid * usable_energy_mwh;
        for soc_bin in 0..soc_bins {
            let cycles = cycles_per_bin[dod_bin * soc_bins + soc_bin];
            if !cycles.is_finite() {
                return Err(CostCurveError::NonFiniteValue {
                    field: "cycles_per_bin",
                    value: cycles,
                });
            }
            if cycles < 0.0 {
                return Err(CostCurveError::NegativeValue {
                    field: "cycles_per_bin",
                    value: cycles,
                });
            }
            if cycles == 0.0 {
                continue;
            }
            let mean_soc_mid = 0.5 * (mean_soc_bin_edges[soc_bin] + mean_soc_bin_edges[soc_bin + 1]);
            let cost_per = parameters.cost_per_cycle(dod_mid, mean_soc_mid)?;
            let bin_cost = cycles * cost_per;
            let bin_throughput = cycles * throughput_per_cycle;
            total_cost += bin_cost;
            total_throughput += bin_throughput;
            bins.push(BinCost {
                bin_index: dod_bin * soc_bins + soc_bin,
                cycles,
                throughput: bin_throughput,
                cost: bin_cost,
                marginal_cost_per_mwh: cost_per / throughput_per_cycle,
            });
        }
    }
    if bins.is_empty() {
        return Err(CostCurveError::EmptyHistogram);
    }
    // The piecewise-linear projection: bins sorted by implied marginal cost
    // ascending (deterministic tie-break by bin index) — convex by
    // construction; adjacent equal marginal costs merge.
    bins.sort_by(|a, b| {
        a.marginal_cost_per_mwh
            .total_cmp(&b.marginal_cost_per_mwh)
            .then(a.bin_index.cmp(&b.bin_index))
    });
    let mut segments: Vec<BessPwlSegment> = Vec::new();
    let mut cumulative = 0.0f64;
    for bin in &bins {
        cumulative += bin.throughput;
        match segments.last_mut() {
            Some(last) if last.marginal_cost_per_mwh == bin.marginal_cost_per_mwh => {
                last.cumulative_throughput_limit_mwh = cumulative;
            }
            _ => segments.push(BessPwlSegment {
                cumulative_throughput_limit_mwh: cumulative,
                marginal_cost_per_mwh: bin.marginal_cost_per_mwh,
            }),
        }
    }
    let unverified = parameters.is_unverified_secondary();
    let provenance_note = if unverified {
        UNVERIFIED_SECONDARY_PROVENANCE_NOTE.to_string()
    } else {
        RECORDED_SECONDARY_PROVENANCE_NOTE.to_string()
    };
    let merged_provenance = weaker_provenance(
        weaker_provenance(
            parameters.base_cost_per_full_equivalent_cycle.provenance,
            parameters.dod_exponent.provenance,
        ),
        parameters.mean_soc_dwell_coefficient.provenance,
    );
    Ok(DegradationCostProjection {
        curve_version: match parameters.chemistry {
            DegradationChemistry::Lfp => LFP_PLANNING_AID_CURVE_VERSION.to_string(),
            DegradationChemistry::Nmc => NMC_PLANNING_AID_CURVE_VERSION.to_string(),
            DegradationChemistry::OutOfScope { .. } => {
                return Err(CostCurveError::OutOfScopeChemistry {
                    label: parameters.chemistry.label(),
                });
            }
        },
        chemistry: parameters.chemistry.clone(),
        linear_equivalent_cost_per_mwh: total_cost / total_throughput,
        piecewise_segments: segments,
        total_cost,
        total_throughput_mwh: total_throughput,
        provenance: merged_provenance,
        parameters_unverified_secondary: unverified,
        provenance_note,
        statement: DEGRADATION_COST_MAPPING_STATEMENT,
    })
}

/// Projects a rainflow histogram (FIM-F11, half-cycle-faithful) into the
/// MWh-equivalent degradation cost under the declared parameter table. The
/// usable energy must be finite, positive, and known-provenance (P3.02: no
/// MWh normalization is credited from unknown data).
pub fn project_rainflow_histogram(
    parameters: &BessCycleCostParameters,
    usable_energy: &ProvenancedValue,
    histogram: &RainflowDodMeanSocHistogram,
) -> Result<DegradationCostProjection, CostCurveError> {
    let usable = usable_energy_creditable(usable_energy)?;
    let mut projection = project_bins(
        parameters,
        usable,
        histogram.dod_bin_edges(),
        histogram.mean_soc_bin_edges(),
        &histogram.cycles_per_bin(),
    )?;
    projection.provenance =
        weaker_provenance(projection.provenance, histogram.provenance());
    projection.provenance = weaker_provenance(projection.provenance, usable_energy.provenance);
    Ok(projection)
}

/// Projects a P15.07 ledger histogram (whole counted cycles — each recorded
/// cycle counts as ONE FULL cycle; the ledger type's recorded semantics are
/// unchanged) into the MWh-equivalent degradation cost under the declared
/// parameter table.
pub fn project_ledger_histogram(
    parameters: &BessCycleCostParameters,
    usable_energy: &ProvenancedValue,
    histogram: &DodMeanSocHistogram,
) -> Result<DegradationCostProjection, CostCurveError> {
    let usable = usable_energy_creditable(usable_energy)?;
    let cycles_per_bin: Vec<f64> = histogram.counts().iter().map(|&count| count as f64).collect();
    let mut projection = project_bins(
        parameters,
        usable,
        histogram.dod_bin_edges(),
        histogram.mean_soc_bin_edges(),
        &cycles_per_bin,
    )?;
    projection.provenance = weaker_provenance(projection.provenance, histogram.provenance());
    projection.provenance = weaker_provenance(projection.provenance, usable_energy.provenance);
    Ok(projection)
}

// --- The optimizer-input mirror ----------------------------------------------------

/// Which projected shape the optimizer input carries (both are computed by
/// every projection; the choice is the caller's declared configuration —
/// the P3.06 linear default stays available and default).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CurveShapeChoice {
    /// The linear-equivalent MWh rate (the exact shape the P3.06 linear
    /// placeholder consumed).
    LinearEquivalent,
    /// The convex piecewise-linear projection.
    PiecewiseLinear,
}

/// One asset's declared degradation cost input, mirroring
/// `flexcore-optimization-api::degradation_cost::DegradationCostDeclaration`
/// FIELD FOR FIELD by citation (`asset_id` is the typed domain identifier;
/// the application layer renders its registry reference string form when it
/// maps this into the optimizer declaration feeding the existing
/// `C_degradation` term). OPT-IN per asset configuration: the P3.06 linear
/// default in `bess_model` stays available and default.
#[derive(Debug, Clone, PartialEq)]
pub struct BessDegradationCostInput {
    /// The asset the curve applies to.
    pub asset_id: AssetId,
    /// The declared curve version (required non-empty — the P3.15
    /// no-untracked-constants discipline).
    pub curve_version: String,
    /// The declared chemistry scope (in-scope by construction).
    pub chemistry: DegradationChemistry,
    /// The declared curve shape (linear or piecewise-linear — the only
    /// shapes the optimizer accepts as degradation INPUT DATA).
    pub curve: BessDegradationCurveShape,
    /// The merged provenance of the projection.
    pub provenance: Provenance,
    /// The provenance note (required non-empty, bounded to 256 characters —
    /// the optimizer contract's bound, mirrored so the mapping cannot fail
    /// downstream).
    pub provenance_note: String,
    /// True when any parameter behind the curve is UNVERIFIED-SECONDARY (the
    /// HUMAN LAB GATE flag for the orchestrator).
    pub parameters_unverified_secondary: bool,
}

/// Builds the optimizer-input mirror from a projection. The provenance note
/// defaults to the projection's composed note; an override must satisfy the
/// same non-empty, 256-character bound.
pub fn bess_degradation_cost_input(
    asset_id: AssetId,
    projection: &DegradationCostProjection,
    shape: CurveShapeChoice,
    provenance_note_override: Option<String>,
) -> Result<BessDegradationCostInput, CostCurveError> {
    let note = match provenance_note_override {
        Some(note) => note,
        None => projection.provenance_note.clone(),
    };
    validate_text("provenance_note", &note)?;
    if projection.curve_version.trim().is_empty() {
        return Err(CostCurveError::UnversionedCurve);
    }
    if projection.curve_version.len() > 256 {
        return Err(CostCurveError::FieldTooLong {
            field: "curve_version",
            max_chars: 256,
        });
    }
    let curve = match shape {
        CurveShapeChoice::LinearEquivalent => BessDegradationCurveShape::Linear {
            cost_per_mwh_throughput: projection.linear_equivalent_cost_per_mwh,
        },
        CurveShapeChoice::PiecewiseLinear => {
            BessDegradationCurveShape::PiecewiseLinear {
                segments: projection.piecewise_segments.clone(),
            }
        }
    };
    Ok(BessDegradationCostInput {
        asset_id,
        curve_version: projection.curve_version.clone(),
        chemistry: projection.chemistry.clone(),
        curve,
        provenance: projection.provenance,
        provenance_note: note,
        parameters_unverified_secondary: projection.parameters_unverified_secondary,
    })
}
