// P15.07 — BESS degradation data model (pure domain records, no I/O).
//
// Decision record:
// `docs/phases-11-20/phase-15-forecasting-asset-intelligence/records/
// 21_p15_07_bess_degradation_data_model.md`.
//
// WHAT THIS MODULE IS: the accumulation-layer DATA MODEL for BESS
// degradation evidence — monotonic counters (cumulative discharge MWh,
// cumulative charge MWh, cumulative throughput MWh, full equivalent cycles),
// the rainflow-fed DOD × mean-SOC histogram, the temperature-exposure
// distribution, high-SOC dwell accumulation, and periodic SoH observations —
// every datum carrying the P3.02 capability-provenance discipline.
//
// WHAT THIS MODULE IS NOT (the honesty lines):
//
// - **No degradation model.** No fade equation, no semi-empirical aging law,
//   no degradation cost is computed or implied anywhere in this module. The
//   P3.06 placeholder statement (`bess_model::DEGRADATION_PLACEHOLDER_
//   STATEMENT`) still governs every degradation-cost use; P15.08 supersedes
//   the placeholder AS INPUT DATA, never as physics.
// - **No rainflow counting.** The histogram is the STORAGE for
//   rainflow-counted cycles: a caller (BMS analytics, a wave-2 worker)
//   extracts (DOD, mean-SOC) half-cycle pairs from an SOC time series and
//   records them here. The extraction itself is NOT implemented and is
//   recorded as such (`RAINFLOW_EXTRACTION_SEAM`).
// - **No dispatch authority (ADR-010, conventions §6.3).** Counters,
//   histograms, and SoH observations are INPUTS and RECORDS. A degradation
//   datum never authorizes or produces a dispatch command.
//
// PROVENANCE DISCIPLINE (P3.02, reused — never reimplemented): every
// recorded datum carries the versioned `Provenance` enum from
// `flexcore_contracts::capability`. Merged provenance is the WEAKEST of the
// observed provenances (MEASURED > CONFIGURED > INFERRED > UNKNOWN): a
// counter that has EVER received an UNKNOWN-provenance increment carries
// UNKNOWN provenance forever — unknown narrows, never widens. A
// counter/histogram whose provenance is UNKNOWN is not creditable for
// warranty-headroom purposes (P15.10) and is exported in evidence packs
// (P15.11) with its provenance label attached — the claimant, not the
// platform, decides what an unknown-provenance datum supports.
//
// UNITS (canonical, `flexcore-contracts::units` — READ-ONLY, untouched):
// energies in MWh, powers in MW, SOC as a fraction in [0,1] at the API
// boundary, FEC dimensionless, durations in seconds. Temperature is recorded
// in degrees Celsius — a DECLARED INPUT unit outside the canonical MW/MVAr/
// MVA/MWh freeze; the unit is named on every temperature field and never
// converted (recorded convention, not physics).
//
// CHEMISTRY COVERAGE (P15-E-3, registered OPEN with DEFAULT (a)): LFP and
// NMC are the in-scope chemistries first, per BLAST-Lite's parameterized
// semi-empirical coverage [S-146][S-147]. Any other chemistry is
// `DegradationChemistry::OutOfScope` — a RECORDED LIMITATION, never a silent
// extrapolation: no degradation parameter is credited for an out-of-scope
// chemistry anywhere downstream.
//
// Control status: pure domain records for the simulation-only 0.x line. No
// I/O, no solver coupling, no device-write path, no dispatch reach.

use flexcore_contracts::capability::Provenance;

use crate::bess_model::ProvenancedValue;
use crate::ids::AssetId;

// --- Recorded statements -----------------------------------------------------

/// The schema version of the degradation ledger data model. A structural
/// change to any accumulator below is a VERSION BUMP, never a silent reshape
/// (the P3.14 schema-version discipline applied to degradation data).
pub const DEGRADATION_LEDGER_SCHEMA_VERSION: u32 = 1;

/// The recorded honesty statement carried by every ledger export.
pub const DEGRADATION_LEDGER_STATEMENT: &str =
    "Degradation data model: accumulated INPUT DATA with provenance, not a \
degradation model. No fade equation, aging law, or degradation cost is \
computed here; the P3.06 placeholder discipline governs every downstream \
cost use. Rainflow extraction is NOT implemented — the histogram stores \
already-counted cycles. Every datum never authorizes or produces a dispatch \
command (ADR-010).";

/// The recorded rainflow seam (what this module does NOT do).
pub const RAINFLOW_EXTRACTION_SEAM: &str =
    "RECORDED SEAM — rainflow counting from an SOC time series is NOT \
implemented in FlexCore. The DOD x mean-SOC histogram accepts \
already-counted (DOD, mean-SOC) cycle pairs from the caller (BMS analytics \
or a wave-2 worker); the extraction method, its reversals convention, and \
its residue handling are the caller's declared responsibility.";

// --- Errors ------------------------------------------------------------------

/// Errors of the degradation data model. Each variant names the offending
/// field/value — never a bare "invalid input" (P3.04 error-catalogue style).
#[derive(Debug, Clone, PartialEq)]
pub enum DegradationLedgerError {
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
    /// Bin edges were not present, strictly ascending, or inside range.
    InvalidBinEdges {
        /// The offending edge table.
        field: &'static str,
        /// The recorded reason.
        reason: &'static str,
    },
    /// A monotonic counter was handed a negative delta (a cumulative counter
    /// never decreases; a decrease is a refusal, never a silent clamp).
    CounterReversed {
        /// The offending counter.
        field: &'static str,
        /// The negative delta offered.
        delta: f64,
    },
    /// Throughput was recorded without a configured positive usable energy
    /// (FEC normalization is impossible; nothing is invented).
    UsableEnergyNotConfigured,
    /// An SoH observation was appended out of observation order.
    SohObservationOutOfOrder {
        /// The last observation time on the ledger (unix millis).
        last_observed_at_unix_ms: i64,
        /// The offered observation time (unix millis).
        offered_observed_at_unix_ms: i64,
    },
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
}

impl std::fmt::Display for DegradationLedgerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DegradationLedgerError::NonFiniteValue { field, value } => {
                write!(f, "{field} must be finite (found {value})")
            }
            DegradationLedgerError::NegativeValue { field, value } => {
                write!(f, "{field} must be >= 0 (found {value})")
            }
            DegradationLedgerError::ValueOutOfRange { field, value, lower, upper } => write!(
                f,
                "{field} must be in [{lower}, {upper}] inclusive (found {value})"
            ),
            DegradationLedgerError::InvalidBinEdges { field, reason } => {
                write!(f, "bin edges '{field}' are invalid: {reason}")
            }
            DegradationLedgerError::CounterReversed { field, delta } => write!(
                f,
                "counter '{field}' is monotonic and refuses the negative delta {delta}"
            ),
            DegradationLedgerError::UsableEnergyNotConfigured => {
                write!(
                    f,
                    "usable energy is not configured; FEC normalization is impossible \
                     and no value is invented (P3.02)"
                )
            }
            DegradationLedgerError::SohObservationOutOfOrder {
                last_observed_at_unix_ms,
                offered_observed_at_unix_ms,
            } => write!(
                f,
                "SoH observation at {offered_observed_at_unix_ms} ms precedes the last \
                 recorded observation at {last_observed_at_unix_ms} ms; the ledger is \
                 append-ordered, never re-ordered"
            ),
            DegradationLedgerError::EmptyField { field } => {
                write!(f, "{field} must be non-empty and non-whitespace")
            }
            DegradationLedgerError::FieldTooLong { field, max_chars } => {
                write!(f, "{field} exceeds the {max_chars}-character bound")
            }
        }
    }
}

impl std::error::Error for DegradationLedgerError {}

// --- Chemistry scope (P15-E-3) -----------------------------------------------

/// The cell chemistry a degradation ledger is declared for (P15-E-3, OPEN
/// with DEFAULT (a): LFP + NMC first per BLAST-Lite coverage [S-146][S-147]).
/// Out-of-scope chemistries are RECORDED LIMITATIONS, never silently
/// extrapolated: downstream degradation-parameter use is refused for them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DegradationChemistry {
    /// Lithium iron phosphate — in scope (P15-E-3 DEFAULT (a)) [S-147].
    Lfp,
    /// Lithium nickel manganese cobalt oxide — in scope (P15-E-3 DEFAULT (a))
    /// [S-147].
    Nmc,
    /// Any other chemistry: explicitly out of the P15-E-3 DEFAULT scope. The
    /// name is recorded data; no degradation parameter is credited for it.
    OutOfScope {
        /// The declared chemistry name (recorded, never interpreted).
        name: String,
    },
}

impl DegradationChemistry {
    /// Whether the chemistry is inside the P15-E-3 DEFAULT coverage (a)
    /// (LFP and NMC first [S-147]). Out-of-scope chemistries are recorded
    /// limitations, never silently extrapolated.
    pub fn is_in_scope(&self) -> bool {
        matches!(self, DegradationChemistry::Lfp | DegradationChemistry::Nmc)
    }

    /// The canonical label used in records and exports.
    pub fn label(&self) -> String {
        match self {
            DegradationChemistry::Lfp => "LFP".to_string(),
            DegradationChemistry::Nmc => "NMC".to_string(),
            DegradationChemistry::OutOfScope { name } => format!("OUT_OF_SCOPE({name})"),
        }
    }
}

// --- Provenance merge (weakest wins; P3.02 discipline) ------------------------

/// Merges two provenances into the WEAKEST of the two so a merged record
/// never overstates knowledge. Precedence (strongest first): MEASURED >
/// CONFIGURED > INFERRED > UNKNOWN. This is the same rule the P3.05 BESS
/// model applies (`bess_model::weaker_provenance`); it is restated here as a
/// private helper because the P3.05 function is private and the rule is
/// frozen — the two implementations are pinned equal by a test.
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

// --- Monotonic counters ------------------------------------------------------

/// A cumulative, monotonic counter (P15.07): the value only ever grows, the
/// provenance only ever weakens (unknown narrows, never widens). A fresh
/// counter starts at zero with UNKNOWN provenance — zero with unknown
/// provenance credits NOTHING (P3.02: unknown is never silently treated as
/// known data).
#[derive(Debug, Clone, PartialEq)]
pub struct MonotonicCounter {
    value: f64,
    provenance: Provenance,
    observations: u64,
}

impl MonotonicCounter {
    /// A fresh counter: value 0.0, provenance UNKNOWN, no observations.
    pub fn new() -> Self {
        Self {
            value: 0.0,
            provenance: Provenance::Unknown,
            observations: 0,
        }
    }

    /// Records a non-negative, finite delta. The provenance merges to the
    /// WEAKER of the current and the offered provenance — a counter that has
    /// ever received an UNKNOWN increment stays UNKNOWN forever.
    pub fn record(&mut self, delta: f64, provenance: Provenance) -> Result<(), DegradationLedgerError> {
        if !delta.is_finite() {
            return Err(DegradationLedgerError::NonFiniteValue { field: "counter_delta", value: delta });
        }
        if delta < 0.0 {
            return Err(DegradationLedgerError::CounterReversed { field: "counter_delta", delta });
        }
        self.value += delta;
        self.provenance = weaker_provenance(self.provenance, provenance);
        self.observations += 1;
        Ok(())
    }

    /// The cumulative value in the counter's declared unit.
    pub fn value(&self) -> f64 {
        self.value
    }

    /// The merged provenance (the weakest observed).
    pub fn provenance(&self) -> Provenance {
        self.provenance
    }

    /// The number of recorded increments.
    pub fn observations(&self) -> u64 {
        self.observations
    }

    /// Whether the counter carries any KNOWN-provenance datum (P3.02: only
    /// known data is creditable downstream).
    pub fn is_creditable(&self) -> bool {
        self.provenance != Provenance::Unknown && self.observations > 0
    }
}

impl Default for MonotonicCounter {
    fn default() -> Self {
        Self::new()
    }
}

// --- The DOD x mean-SOC histogram ---------------------------------------------

/// The rainflow-fed DOD × mean-SOC histogram (P15.07): the STORAGE for
/// already-counted cycles. Rows are DOD bins, columns are mean-SOC bins;
/// counts only grow. Rainflow extraction itself is NOT implemented
/// ([`RAINFLOW_EXTRACTION_SEAM`]).
#[derive(Debug, Clone, PartialEq)]
pub struct DodMeanSocHistogram {
    dod_bin_edges: Vec<f64>,
    mean_soc_bin_edges: Vec<f64>,
    counts: Vec<u64>,
    provenance: Provenance,
}

impl DodMeanSocHistogram {
    /// Constructs a histogram from declared, strictly ascending bin edges.
    /// Both edge tables must contain at least two edges in [0, 1]; DOD edges
    /// must start at 0.0 (a cycle of depth zero is not a cycle) and the last
    /// edge must be 1.0 for both tables so every value in [0, 1] resolves to
    /// exactly one bin.
    pub fn new(dod_bin_edges: Vec<f64>, mean_soc_bin_edges: Vec<f64>) -> Result<Self, DegradationLedgerError> {
        validate_edges(&dod_bin_edges, "dod_bin_edges", true)?;
        validate_edges(&mean_soc_bin_edges, "mean_soc_bin_edges", false)?;
        let dod_bins = dod_bin_edges.len() - 1;
        let soc_bins = mean_soc_bin_edges.len() - 1;
        Ok(Self {
            dod_bin_edges,
            mean_soc_bin_edges,
            counts: vec![0u64; dod_bins * soc_bins],
            provenance: Provenance::Unknown,
        })
    }

    /// Records one already-counted cycle: its depth of discharge (fraction in
    /// (0, 1]) and its mean SOC (fraction in [0, 1]). The provenance merges
    /// to the weakest observed (unknown narrows, never widens).
    pub fn record_cycle(&mut self, dod: f64, mean_soc: f64, provenance: Provenance) -> Result<(), DegradationLedgerError> {
        if !dod.is_finite() {
            return Err(DegradationLedgerError::NonFiniteValue { field: "dod", value: dod });
        }
        if dod <= 0.0 || dod > 1.0 {
            return Err(DegradationLedgerError::ValueOutOfRange {
                field: "dod",
                value: dod,
                lower: f64::EPSILON,
                upper: 1.0,
            });
        }
        if !mean_soc.is_finite() {
            return Err(DegradationLedgerError::NonFiniteValue { field: "mean_soc", value: mean_soc });
        }
        if !(0.0..=1.0).contains(&mean_soc) {
            return Err(DegradationLedgerError::ValueOutOfRange {
                field: "mean_soc",
                value: mean_soc,
                lower: 0.0,
                upper: 1.0,
            });
        }
        let dod_bin = bin_index(&self.dod_bin_edges, dod);
        let soc_bin = bin_index(&self.mean_soc_bin_edges, mean_soc);
        let dod_bins = self.dod_bin_edges.len() - 1;
        let soc_bins = self.mean_soc_bin_edges.len() - 1;
        let index = dod_bin * soc_bins + soc_bin;
        self.counts[index] += 1;
        self.provenance = weaker_provenance(self.provenance, provenance);
        Ok(())
    }

    /// The declared DOD bin edges.
    pub fn dod_bin_edges(&self) -> &[f64] {
        &self.dod_bin_edges
    }

    /// The declared mean-SOC bin edges.
    pub fn mean_soc_bin_edges(&self) -> &[f64] {
        &self.mean_soc_bin_edges
    }

    /// The counts, row-major: `dod_bin * mean_soc_bins + soc_bin`.
    pub fn counts(&self) -> &[u64] {
        &self.counts
    }

    /// The merged provenance (the weakest observed).
    pub fn provenance(&self) -> Provenance {
        self.provenance
    }

    /// The total number of recorded cycles.
    pub fn total_cycles(&self) -> u64 {
        self.counts.iter().sum()
    }
}

/// Validates a bin-edge table: at least two edges, finite, strictly
/// ascending, within [0, 1]; `must_start_at_zero` pins the first edge to 0.0
/// (DOD) and every table's last edge to 1.0 (total coverage).
fn validate_edges(edges: &[f64], field: &'static str, must_start_at_zero: bool) -> Result<(), DegradationLedgerError> {
    if edges.len() < 2 {
        return Err(DegradationLedgerError::InvalidBinEdges {
            field,
            reason: "at least two edges are required (one bin minimum)",
        });
    }
    for (i, edge) in edges.iter().enumerate() {
        if !edge.is_finite() {
            return Err(DegradationLedgerError::NonFiniteValue { field, value: *edge });
        }
        if *edge < 0.0 || *edge > 1.0 {
            return Err(DegradationLedgerError::ValueOutOfRange {
                field,
                value: *edge,
                lower: 0.0,
                upper: 1.0,
            });
        }
        if i > 0 && *edge <= edges[i - 1] {
            return Err(DegradationLedgerError::InvalidBinEdges {
                field,
                reason: "edges must be strictly ascending",
            });
        }
    }
    if must_start_at_zero && edges[0] != 0.0 {
        return Err(DegradationLedgerError::InvalidBinEdges {
            field,
            reason: "the first DOD edge must be 0.0 (a zero-depth cycle is not a cycle)",
        });
    }
    if edges[edges.len() - 1] != 1.0 {
        return Err(DegradationLedgerError::InvalidBinEdges {
            field,
            reason: "the last edge must be 1.0 so every value in [0,1] resolves to a bin",
        });
    }
    Ok(())
}

/// The bin index for a value against validated ascending edges: lower edge
/// inclusive, upper edge exclusive; the last bin includes its upper edge.
fn bin_index(edges: &[f64], value: f64) -> usize {
    for i in 1..edges.len() {
        if value < edges[i] || (i == edges.len() - 1 && value <= edges[i]) {
            return i - 1;
        }
    }
    edges.len() - 2
}

// --- Temperature exposure ------------------------------------------------------

/// The temperature-exposure distribution (P15.07): accumulated seconds per
/// declared temperature band. Temperatures are in DEGREES CELSIUS — a
/// declared input unit outside the canonical MW/MVAr/MVA/MWh freeze
/// (`flexcore-contracts::units` is read-only and untouched); the unit is
/// named on every field and never converted.
#[derive(Debug, Clone, PartialEq)]
pub struct TemperatureExposure {
    band_edges_c: Vec<f64>,
    seconds_per_band: Vec<u64>,
    provenance: Provenance,
}

impl TemperatureExposure {
    /// Constructs the distribution from declared, strictly ascending band
    /// edges in °C (at least two edges; the edge table is the declared
    /// configuration — no default band table is invented).
    pub fn new(band_edges_c: Vec<f64>) -> Result<Self, DegradationLedgerError> {
        if band_edges_c.len() < 2 {
            return Err(DegradationLedgerError::InvalidBinEdges {
                field: "band_edges_c",
                reason: "at least two edges are required (one band minimum)",
            });
        }
        for (i, edge) in band_edges_c.iter().enumerate() {
            if !edge.is_finite() {
                return Err(DegradationLedgerError::NonFiniteValue { field: "band_edges_c", value: *edge });
            }
            if i > 0 && *edge <= band_edges_c[i - 1] {
                return Err(DegradationLedgerError::InvalidBinEdges {
                    field: "band_edges_c",
                    reason: "edges must be strictly ascending",
                });
            }
        }
        Ok(Self {
            seconds_per_band: vec![0u64; band_edges_c.len() - 1],
            band_edges_c,
            provenance: Provenance::Unknown,
        })
    }

    /// Records `seconds` of exposure at `temperature_c`. Outside the declared
    /// edge range the exposure is refused — the band table is declared
    /// configuration and nothing is extrapolated beyond it (P3.02: no
    /// invented coverage). The provenance merges to the weakest observed.
    pub fn record_exposure(&mut self, temperature_c: f64, seconds: u64, provenance: Provenance) -> Result<(), DegradationLedgerError> {
        if !temperature_c.is_finite() {
            return Err(DegradationLedgerError::NonFiniteValue {
                field: "temperature_c",
                value: temperature_c,
            });
        }
        let first = self.band_edges_c[0];
        let last = self.band_edges_c[self.band_edges_c.len() - 1];
        if temperature_c < first || temperature_c > last {
            return Err(DegradationLedgerError::ValueOutOfRange {
                field: "temperature_c",
                value: temperature_c,
                lower: first,
                upper: last,
            });
        }
        let band = bin_index(&self.band_edges_c, temperature_c);
        self.seconds_per_band[band] += seconds;
        self.provenance = weaker_provenance(self.provenance, provenance);
        Ok(())
    }

    /// The declared band edges in °C.
    pub fn band_edges_c(&self) -> &[f64] {
        &self.band_edges_c
    }

    /// The accumulated seconds per band.
    pub fn seconds_per_band(&self) -> &[u64] {
        &self.seconds_per_band
    }

    /// The merged provenance (the weakest observed).
    pub fn provenance(&self) -> Provenance {
        self.provenance
    }

    /// The total accumulated exposure seconds.
    pub fn total_seconds(&self) -> u64 {
        self.seconds_per_band.iter().sum()
    }
}

// --- High-SOC dwell -------------------------------------------------------------

/// The high-SOC dwell accumulator (P15.07): total seconds spent at or above
/// a declared SOC threshold (calendar-aging-relevant dwell, [S-145]; the
/// threshold is DECLARED configuration, never physics).
#[derive(Debug, Clone, PartialEq)]
pub struct HighSocDwell {
    soc_threshold: f64,
    dwell_seconds: u64,
    provenance: Provenance,
}

impl HighSocDwell {
    /// Constructs the accumulator for a declared SOC threshold (fraction in
    /// [0, 1]).
    pub fn new(soc_threshold: f64) -> Result<Self, DegradationLedgerError> {
        if !soc_threshold.is_finite() {
            return Err(DegradationLedgerError::NonFiniteValue {
                field: "soc_threshold",
                value: soc_threshold,
            });
        }
        if !(0.0..=1.0).contains(&soc_threshold) {
            return Err(DegradationLedgerError::ValueOutOfRange {
                field: "soc_threshold",
                value: soc_threshold,
                lower: 0.0,
                upper: 1.0,
            });
        }
        Ok(Self {
            soc_threshold,
            dwell_seconds: 0,
            provenance: Provenance::Unknown,
        })
    }

    /// Records `seconds` of dwell at `soc` (fraction in [0, 1]). Dwell at or
    /// above the threshold accumulates; dwell below it is recorded as a
    /// zero-contribution observation (the observation count still grows, so
    /// a caller cannot confuse "nothing above threshold" with "no data").
    /// The provenance merges to the weakest observed.
    pub fn record_dwell(&mut self, soc: f64, seconds: u64, provenance: Provenance) -> Result<(), DegradationLedgerError> {
        if !soc.is_finite() {
            return Err(DegradationLedgerError::NonFiniteValue { field: "soc", value: soc });
        }
        if !(0.0..=1.0).contains(&soc) {
            return Err(DegradationLedgerError::ValueOutOfRange {
                field: "soc",
                value: soc,
                lower: 0.0,
                upper: 1.0,
            });
        }
        if soc >= self.soc_threshold {
            self.dwell_seconds += seconds;
        }
        self.provenance = weaker_provenance(self.provenance, provenance);
        Ok(())
    }

    /// The declared SOC threshold.
    pub fn soc_threshold(&self) -> f64 {
        self.soc_threshold
    }

    /// The accumulated dwell seconds at or above the threshold.
    pub fn dwell_seconds(&self) -> u64 {
        self.dwell_seconds
    }

    /// The merged provenance (the weakest observed).
    pub fn provenance(&self) -> Provenance {
        self.provenance
    }
}

// --- SoH observations -------------------------------------------------------------

/// One periodic SoH observation (P15.07): a BMS-reported remaining-capacity
/// fraction with provenance, observation time, and the reporting source.
/// Platform-side SoH estimation is DEFERRED (P15.09) — every observation
/// here is REPORTED data, never a platform-computed estimate.
#[derive(Debug, Clone, PartialEq)]
pub struct SohObservation {
    /// The reported SoH as a fraction in [0, 1].
    pub soh_fraction: f64,
    /// The observation time (unix millis; the ledger is append-ordered on
    /// this field — out-of-order observations are refused, never re-ordered).
    pub observed_at_unix_ms: i64,
    /// The provenance of the reported value.
    pub provenance: Provenance,
    /// The reporting source reference (e.g. the BMS identity; recorded data).
    pub source: String,
}

impl SohObservation {
    /// Constructs and validates an observation.
    pub fn new(
        soh_fraction: f64,
        observed_at_unix_ms: i64,
        provenance: Provenance,
        source: String,
    ) -> Result<Self, DegradationLedgerError> {
        if !soh_fraction.is_finite() {
            return Err(DegradationLedgerError::NonFiniteValue {
                field: "soh_fraction",
                value: soh_fraction,
            });
        }
        if !(0.0..=1.0).contains(&soh_fraction) {
            return Err(DegradationLedgerError::ValueOutOfRange {
                field: "soh_fraction",
                value: soh_fraction,
                lower: 0.0,
                upper: 1.0,
            });
        }
        if source.trim().is_empty() {
            return Err(DegradationLedgerError::EmptyField { field: "source" });
        }
        if source.len() > 256 {
            return Err(DegradationLedgerError::FieldTooLong { field: "source", max_chars: 256 });
        }
        Ok(Self {
            soh_fraction,
            observed_at_unix_ms,
            provenance,
            source,
        })
    }
}

// --- The ledger -------------------------------------------------------------------

/// The per-asset degradation ledger (P15.07): the aggregate binding the
/// monotonic counters, the DOD × mean-SOC histogram, the temperature
/// exposure, the high-SOC dwell, and the append-ordered SoH observations for
/// one asset. Every datum carries provenance; unknown narrows, never widens.
///
/// The ledger is the P15.10 warranty-headroom input and the P15.11
/// evidence-pack source. It computes NO degradation cost and NO state of
/// health — it only accumulates reported/observed data.
#[derive(Debug, Clone, PartialEq)]
pub struct BessDegradationLedger {
    /// The ledger data-model schema version
    /// ([`DEGRADATION_LEDGER_SCHEMA_VERSION`]).
    pub schema_version: u32,
    /// The asset the ledger belongs to.
    pub asset_id: AssetId,
    /// The declared chemistry scope (P15-E-3; out-of-scope chemistries are
    /// recorded limitations).
    pub chemistry: DegradationChemistry,
    /// The declared usable energy (MWh, ≥ 0) used to normalize throughput to
    /// full equivalent cycles, with provenance. Required before any
    /// throughput can be recorded (no FEC value is invented).
    pub usable_energy_mwh: ProvenancedValue,
    /// Cumulative discharge energy in MWh (monotonic).
    pub cumulative_discharge_mwh: MonotonicCounter,
    /// Cumulative charge energy in MWh (monotonic).
    pub cumulative_charge_mwh: MonotonicCounter,
    /// Cumulative (charge + discharge) throughput in MWh (monotonic).
    pub cumulative_throughput_mwh: MonotonicCounter,
    /// Cumulative full equivalent cycles, dimensionless (monotonic; each
    /// increment is `(charge + discharge) MWh / (2 · usable energy MWh)` —
    /// the standard FEC normalization; the DEFINITION is a declared
    /// convention, not a measurement).
    pub full_equivalent_cycles: MonotonicCounter,
    /// The rainflow-fed DOD × mean-SOC histogram.
    pub dod_mean_soc_histogram: DodMeanSocHistogram,
    /// The temperature-exposure distribution (°C bands).
    pub temperature_exposure: TemperatureExposure,
    /// The high-SOC dwell accumulator.
    pub high_soc_dwell: HighSocDwell,
    /// The append-ordered SoH observations (P15.07/P15.09).
    pub soh_observations: Vec<SohObservation>,
}

impl BessDegradationLedger {
    /// Constructs and validates a ledger. The chemistry, usable energy,
    /// histogram bins, temperature bands, and dwell threshold are DECLARED
    /// inputs — no default is invented for any of them.
    pub fn new(
        asset_id: AssetId,
        chemistry: DegradationChemistry,
        usable_energy_mwh: ProvenancedValue,
        dod_bin_edges: Vec<f64>,
        mean_soc_bin_edges: Vec<f64>,
        temperature_band_edges_c: Vec<f64>,
        high_soc_threshold: f64,
    ) -> Result<Self, DegradationLedgerError> {
        if !usable_energy_mwh.value.is_finite() {
            return Err(DegradationLedgerError::NonFiniteValue {
                field: "usable_energy_mwh",
                value: usable_energy_mwh.value,
            });
        }
        if usable_energy_mwh.value < 0.0 {
            return Err(DegradationLedgerError::NegativeValue {
                field: "usable_energy_mwh",
                value: usable_energy_mwh.value,
            });
        }
        let histogram = DodMeanSocHistogram::new(dod_bin_edges, mean_soc_bin_edges)?;
        let temperature = TemperatureExposure::new(temperature_band_edges_c)?;
        let dwell = HighSocDwell::new(high_soc_threshold)?;
        Ok(Self {
            schema_version: DEGRADATION_LEDGER_SCHEMA_VERSION,
            asset_id,
            chemistry,
            usable_energy_mwh,
            cumulative_discharge_mwh: MonotonicCounter::new(),
            cumulative_charge_mwh: MonotonicCounter::new(),
            cumulative_throughput_mwh: MonotonicCounter::new(),
            full_equivalent_cycles: MonotonicCounter::new(),
            dod_mean_soc_histogram: histogram,
            temperature_exposure: temperature,
            high_soc_dwell: dwell,
            soh_observations: Vec::new(),
        })
    }

    /// Records one interval's energy throughput: charge and discharge
    /// magnitudes in MWh with provenance. Updates the three energy counters
    /// and the FEC counter (FEC increment = `(charge + discharge) /
    /// (2 · usable energy)`; refused when usable energy is not configured
    /// positive — nothing is invented).
    pub fn record_throughput(
        &mut self,
        charge_mwh: f64,
        discharge_mwh: f64,
        provenance: Provenance,
    ) -> Result<(), DegradationLedgerError> {
        for (field, value) in [("charge_mwh", charge_mwh), ("discharge_mwh", discharge_mwh)] {
            if !value.is_finite() {
                return Err(DegradationLedgerError::NonFiniteValue { field, value });
            }
            if value < 0.0 {
                return Err(DegradationLedgerError::NegativeValue { field, value });
            }
        }
        if self.usable_energy_mwh.value <= 0.0 {
            return Err(DegradationLedgerError::UsableEnergyNotConfigured);
        }
        self.cumulative_charge_mwh.record(charge_mwh, provenance)?;
        self.cumulative_discharge_mwh.record(discharge_mwh, provenance)?;
        self.cumulative_throughput_mwh
            .record(charge_mwh + discharge_mwh, provenance)?;
        let fec_increment = (charge_mwh + discharge_mwh)
            / (2.0 * self.usable_energy_mwh.value);
        self.full_equivalent_cycles.record(fec_increment, provenance)?;
        Ok(())
    }

    /// Appends one SoH observation in observation order (out-of-order
    /// appends are refused — the ledger is append-ordered, never re-ordered).
    pub fn record_soh_observation(&mut self, observation: SohObservation) -> Result<(), DegradationLedgerError> {
        if let Some(last) = self.soh_observations.last() {
            if observation.observed_at_unix_ms < last.observed_at_unix_ms {
                return Err(DegradationLedgerError::SohObservationOutOfOrder {
                    last_observed_at_unix_ms: last.observed_at_unix_ms,
                    offered_observed_at_unix_ms: observation.observed_at_unix_ms,
                });
            }
        }
        self.soh_observations.push(observation);
        Ok(())
    }

    /// The latest SoH observation, if any.
    pub fn latest_soh(&self) -> Option<&SohObservation> {
        self.soh_observations.last()
    }

    /// Whether the ledger's cumulative counters are creditable for warranty
    /// purposes: usable energy configured positive AND every counter carrying
    /// known provenance with at least one observation (P3.02: unknown never
    /// widens, and unknown data is never credited).
    pub fn counters_creditable(&self) -> bool {
        self.usable_energy_mwh.value > 0.0
            && self.usable_energy_mwh.provenance != Provenance::Unknown
            && self.cumulative_discharge_mwh.is_creditable()
            && self.cumulative_charge_mwh.is_creditable()
            && self.cumulative_throughput_mwh.is_creditable()
            && self.full_equivalent_cycles.is_creditable()
    }
}

// --- Tests ------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use flexcore_contracts::ids::FlexUuid;

    fn asset() -> AssetId {
        AssetId::new(FlexUuid::parse("0192a5e0-8c3d-7e2f-9a1b-3c4d5e6f7a8c").unwrap())
    }

    fn usable() -> ProvenancedValue {
        ProvenancedValue::configured(10.0)
    }

    fn ledger() -> BessDegradationLedger {
        BessDegradationLedger::new(
            asset(),
            DegradationChemistry::Lfp,
            usable(),
            vec![0.0, 0.25, 0.5, 1.0],
            vec![0.0, 0.5, 1.0],
            vec![0.0, 25.0, 45.0],
            0.9,
        )
        .unwrap()
    }

    // --- chemistry scope (P15-E-3) --------------------------------------

    #[test]
    fn lfp_and_nmc_are_in_scope_per_the_e3_default() {
        assert!(DegradationChemistry::Lfp.is_in_scope());
        assert!(DegradationChemistry::Nmc.is_in_scope());
    }

    #[test]
    fn out_of_scope_chemistry_is_a_recorded_limitation() {
        let chemistry = DegradationChemistry::OutOfScope { name: "LTO".to_string() };
        assert!(!chemistry.is_in_scope());
        assert_eq!(chemistry.label(), "OUT_OF_SCOPE(LTO)");
    }

    // --- monotonic counters ------------------------------------------------

    #[test]
    fn fresh_counter_is_unknown_and_zero_and_credits_nothing() {
        let counter = MonotonicCounter::new();
        assert_eq!(counter.value(), 0.0);
        assert_eq!(counter.provenance(), Provenance::Unknown);
        assert!(!counter.is_creditable());
    }

    #[test]
    fn counter_accumulates_and_keeps_the_weakest_provenance() {
        let mut counter = MonotonicCounter::new();
        counter.record(3.0, Provenance::Measured).unwrap();
        counter.record(2.0, Provenance::Configured).unwrap();
        assert_eq!(counter.value(), 5.0);
        // Weakest observed wins: Measured then Configured -> Configured.
        assert_eq!(counter.provenance(), Provenance::Configured);
        assert!(counter.is_creditable());
    }

    #[test]
    fn counter_that_ever_received_unknown_stays_unknown_forever() {
        let mut counter = MonotonicCounter::new();
        counter.record(1.0, Provenance::Measured).unwrap();
        counter.record(0.0, Provenance::Unknown).unwrap();
        counter.record(4.0, Provenance::Measured).unwrap();
        assert_eq!(counter.value(), 5.0);
        assert_eq!(counter.provenance(), Provenance::Unknown);
        assert!(!counter.is_creditable(), "unknown narrows, never widens");
    }

    #[test]
    fn counter_refuses_negative_and_non_finite_deltas() {
        let mut counter = MonotonicCounter::new();
        counter.record(1.0, Provenance::Measured).unwrap();
        assert_eq!(
            counter.record(-0.5, Provenance::Measured),
            Err(DegradationLedgerError::CounterReversed { field: "counter_delta", delta: -0.5 })
        );
        assert_eq!(
            counter.record(f64::NAN, Provenance::Measured),
            Err(DegradationLedgerError::NonFiniteValue { field: "counter_delta", value: f64::NAN })
        );
        // Refused deltas change nothing.
        assert_eq!(counter.value(), 1.0);
    }

    // --- histogram ---------------------------------------------------------

    #[test]
    fn histogram_stores_counted_cycles_in_declared_bins() {
        let mut histogram =
            DodMeanSocHistogram::new(vec![0.0, 0.25, 0.5, 1.0], vec![0.0, 0.5, 1.0]).unwrap();
        // DOD 0.3 -> bin 1; mean SOC 0.4 -> bin 0.
        histogram.record_cycle(0.3, 0.4, Provenance::Measured).unwrap();
        // DOD 0.6 -> bin 2; mean SOC exactly 0.5 -> bin 0 (lower edge inclusive).
        histogram.record_cycle(0.6, 0.5, Provenance::Measured).unwrap();
        // DOD exactly 1.0 -> last bin; mean SOC 1.0 -> last bin.
        histogram.record_cycle(1.0, 1.0, Provenance::Measured).unwrap();
        let dod_bins = 3;
        let soc_bins = 2;
        assert_eq!(histogram.counts().len(), dod_bins * soc_bins);
        assert_eq!(histogram.counts()[1 * soc_bins + 0], 1);
        assert_eq!(histogram.counts()[2 * soc_bins + 0], 1);
        assert_eq!(histogram.counts()[2 * soc_bins + 1], 1);
        assert_eq!(histogram.total_cycles(), 3);
        assert_eq!(histogram.provenance(), Provenance::Measured);
    }

    #[test]
    fn histogram_refuses_zero_depth_and_out_of_range_values() {
        let mut histogram =
            DodMeanSocHistogram::new(vec![0.0, 0.5, 1.0], vec![0.0, 1.0]).unwrap();
        assert!(matches!(
            histogram.record_cycle(0.0, 0.5, Provenance::Measured),
            Err(DegradationLedgerError::ValueOutOfRange { field: "dod", .. })
        ));
        assert!(matches!(
            histogram.record_cycle(0.3, 1.5, Provenance::Measured),
            Err(DegradationLedgerError::ValueOutOfRange { field: "mean_soc", .. })
        ));
        assert_eq!(histogram.total_cycles(), 0);
    }

    #[test]
    fn histogram_edges_are_validated() {
        assert!(matches!(
            DodMeanSocHistogram::new(vec![0.0, 0.5, 1.0], vec![0.2, 1.0]),
            Err(DegradationLedgerError::InvalidBinEdges { field: "dod_bin_edges", .. })
        ));
        assert!(matches!(
            DodMeanSocHistogram::new(vec![0.0, 0.5, 0.9], vec![0.0, 1.0]),
            Err(DegradationLedgerError::InvalidBinEdges { field: "dod_bin_edges", .. })
        ));
        assert!(matches!(
            DodMeanSocHistogram::new(vec![0.0, 0.5, 0.5, 1.0], vec![0.0, 1.0]),
            Err(DegradationLedgerError::InvalidBinEdges { field: "dod_bin_edges", .. })
        ));
        assert!(matches!(
            DodMeanSocHistogram::new(vec![0.0, 1.0], vec![0.0]),
            Err(DegradationLedgerError::InvalidBinEdges { field: "mean_soc_bin_edges", .. })
        ));
    }

    // --- temperature exposure ------------------------------------------------

    #[test]
    fn temperature_exposure_accumulates_declared_bands() {
        let mut exposure = TemperatureExposure::new(vec![0.0, 25.0, 45.0]).unwrap();
        exposure.record_exposure(10.0, 3600, Provenance::Measured).unwrap();
        // Exactly on an upper edge resolves to that band (lower-inclusive).
        exposure.record_exposure(25.0, 120, Provenance::Measured).unwrap();
        exposure.record_exposure(45.0, 60, Provenance::Measured).unwrap();
        assert_eq!(exposure.seconds_per_band(), &[3600, 120, 60]);
        assert_eq!(exposure.total_seconds(), 3780);
    }

    #[test]
    fn temperature_exposure_refuses_values_outside_the_declared_bands() {
        let mut exposure = TemperatureExposure::new(vec![0.0, 45.0]).unwrap();
        // Below the declared floor: refused, never extrapolated.
        assert!(matches!(
            exposure.record_exposure(-10.0, 60, Provenance::Measured),
            Err(DegradationLedgerError::ValueOutOfRange { field: "temperature_c", .. })
        ));
        assert_eq!(exposure.total_seconds(), 0);
    }

    // --- high-SOC dwell --------------------------------------------------------

    #[test]
    fn high_soc_dwell_accumulates_at_or_above_the_threshold() {
        let mut dwell = HighSocDwell::new(0.9).unwrap();
        dwell.record_dwell(0.95, 600, Provenance::Measured).unwrap();
        dwell.record_dwell(0.9, 100, Provenance::Measured).unwrap(); // boundary accumulates
        dwell.record_dwell(0.5, 5000, Provenance::Measured).unwrap(); // below: zero contribution
        assert_eq!(dwell.dwell_seconds(), 700);
    }

    // --- SoH observations --------------------------------------------------------

    #[test]
    fn soh_observations_append_in_order_and_refuse_out_of_order() {
        let mut ledger = ledger();
        ledger
            .record_soh_observation(
                SohObservation::new(1.0, 1_000, Provenance::Measured, "bms-a".to_string()).unwrap(),
            )
            .unwrap();
        // Equal timestamps are append-legal (same sample instant).
        ledger
            .record_soh_observation(
                SohObservation::new(0.99, 1_000, Provenance::Measured, "bms-a".to_string()).unwrap(),
            )
            .unwrap();
        assert_eq!(
            ledger.record_soh_observation(
                SohObservation::new(0.98, 999, Provenance::Measured, "bms-a".to_string()).unwrap(),
            ),
            Err(DegradationLedgerError::SohObservationOutOfOrder {
                last_observed_at_unix_ms: 1_000,
                offered_observed_at_unix_ms: 999,
            })
        );
        assert_eq!(ledger.latest_soh().unwrap().soh_fraction, 0.99);
    }

    #[test]
    fn soh_observation_validates_range_and_source() {
        assert!(matches!(
            SohObservation::new(1.5, 0, Provenance::Measured, "bms".to_string()),
            Err(DegradationLedgerError::ValueOutOfRange { field: "soh_fraction", .. })
        ));
        assert!(matches!(
            SohObservation::new(0.9, 0, Provenance::Measured, "  ".to_string()),
            Err(DegradationLedgerError::EmptyField { field: "source" })
        ));
    }

    // --- the ledger ---------------------------------------------------------------

    #[test]
    fn ledger_records_throughput_counters_and_fec_normalization() {
        let mut ledger = ledger();
        // 6 MWh charge + 4 MWh discharge over a 10 MWh usable window:
        // FEC increment = 10 / (2 * 10) = 0.5 full equivalent cycles.
        ledger.record_throughput(6.0, 4.0, Provenance::Measured).unwrap();
        assert_eq!(ledger.cumulative_charge_mwh.value(), 6.0);
        assert_eq!(ledger.cumulative_discharge_mwh.value(), 4.0);
        assert_eq!(ledger.cumulative_throughput_mwh.value(), 10.0);
        assert_eq!(ledger.full_equivalent_cycles.value(), 0.5);
        assert_eq!(ledger.full_equivalent_cycles.provenance(), Provenance::Measured);
        assert!(ledger.counters_creditable());
    }

    #[test]
    fn ledger_refuses_negative_throughput_and_unknown_usable_energy() {
        let mut ledger = ledger();
        assert!(matches!(
            ledger.record_throughput(-1.0, 2.0, Provenance::Measured),
            Err(DegradationLedgerError::NegativeValue { field: "charge_mwh", .. })
        ));
        // A ledger without a positive usable energy credits no FEC. (An
        // UNKNOWN-provenance usable energy carries a NaN value and is
        // refused at construction: non-finite model parameters are refused
        // everywhere, per the P3.05 `require_finite` posture.)
        let zero_energy = BessDegradationLedger::new(
            asset(),
            DegradationChemistry::Nmc,
            ProvenancedValue::configured(0.0),
            vec![0.0, 1.0],
            vec![0.0, 1.0],
            vec![0.0, 45.0],
            0.9,
        )
        .unwrap();
        assert!(matches!(
            zero_energy.record_throughput(1.0, 1.0, Provenance::Measured),
            Err(DegradationLedgerError::UsableEnergyNotConfigured)
        ));
        // An UNKNOWN-provenance usable energy (NaN value) is refused at
        // construction — no NaN ever enters the ledger.
        assert!(matches!(
            BessDegradationLedger::new(
                asset(),
                DegradationChemistry::Nmc,
                ProvenancedValue::unknown(),
                vec![0.0, 1.0],
                vec![0.0, 1.0],
                vec![0.0, 45.0],
                0.9,
            ),
            Err(DegradationLedgerError::NonFiniteValue { field: "usable_energy_mwh", .. })
        ));
    }

    #[test]
    fn ledger_counters_lose_creditability_when_any_provenance_is_unknown() {
        let mut ledger = ledger();
        ledger.record_throughput(1.0, 1.0, Provenance::Measured).unwrap();
        // One UNKNOWN-provenance increment narrows the whole ledger's
        // creditability: unknown narrows, never widens (P3.02).
        ledger.record_throughput(0.0, 0.0, Provenance::Unknown).unwrap();
        assert!(!ledger.counters_creditable());
        assert_eq!(ledger.full_equivalent_cycles.provenance(), Provenance::Unknown);
    }

    #[test]
    fn ledger_carries_the_schema_version_and_statements() {
        let ledger = ledger();
        assert_eq!(ledger.schema_version, DEGRADATION_LEDGER_SCHEMA_VERSION);
        assert!(DEGRADATION_LEDGER_STATEMENT.contains("not a degradation model"));
        assert!(RAINFLOW_EXTRACTION_SEAM.contains("NOT implemented"));
    }

    #[test]
    fn weaker_provenance_matches_the_frozen_p302_precedence() {
        // The rule is frozen (P3.02): MEASURED > CONFIGURED > INFERRED >
        // UNKNOWN; merged = weakest. Pinned here so the private helper can
        // never drift from the recorded rule.
        use Provenance as P;
        assert_eq!(weaker_provenance(P::Measured, P::Measured), P::Measured);
        assert_eq!(weaker_provenance(P::Measured, P::Configured), P::Configured);
        assert_eq!(weaker_provenance(P::Configured, P::Inferred), P::Inferred);
        assert_eq!(weaker_provenance(P::Inferred, P::Unknown), P::Unknown);
        assert_eq!(weaker_provenance(P::Measured, P::Unknown), P::Unknown);
    }
}
