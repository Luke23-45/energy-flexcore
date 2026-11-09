// P3.14 / P3.15 — Scheduling objective definition, term validation, and
//                 cost-curve provenance (pure rules, no I/O, no solver).
//
// Implements ledger P3.14 ("Scheduling objective definition") and P3.15
// ("Objective-term validation and cost-curve provenance"), report §12.1 and
// §21.3. Decision record:
// `ledger/phase-03-flexibility-scheduling/14_16_objective_pcc.md`.
//
// THE OBJECTIVE, DECIDED ONCE (report §12.1):
//
//   min_x  Σ_t ( C_energy,t + C_degradation,t + C_deviation,t
//                + C_curtailment,t + C_service,t )
//
// - The five §12.1 terms are the CLOSED term vocabulary
//   ([`ObjectiveTerm`]). There is deliberately no catch-all "other" slot:
//   adding a term means adding an enum variant AND bumping
//   [`OBJECTIVE_SCHEMA_VERSION`] — the config type makes a silent objective
//   extension impossible by construction (ledger P3.14 step 3 / the task's
//   "schema-version change, not a silent field" requirement).
// - **The foundations §7.5 extra terms are NOT in the v0.2.0 objective.**
//   Flex discomfort, commitment deviation, reserve shortfall, and switching
//   costs appear in foundations §7.5 as terms an objective "may minimize"
//   (illustrative, not mandatory), while report §12.1 defines the objective
//   as exactly the five terms above. The exclusion and its reason are
//   carried in [`EXCLUDED_TERMS_STATEMENT`] (visible in the type's record
//   output, not a comment) and in the decision doc. Later adoption is a
//   recorded design decision, never a quiet extension.
// - **No objective is hard-coded as universal** (§12.1): every objective
//   carries an explicit, caller-supplied NAME; term weights, signs, and the
//   term set are recorded per schedule through [`ObjectiveConfig::record_fields`]
//   (the P3.21 schedule record composes these fields verbatim). Two configs
//   with different names or weights compare false under
//   [`ObjectiveConfig::same_problem`] — two schedules with different
//   objectives are never the same problem.
//
// THE DEFAULT v0.2.0 OBJECTIVE, DECIDED ONCE — SYNTHETIC, NOT A MARKET
// PRODUCT (D1/D2 OPEN):
//
//   [`ObjectiveConfig::default_synthetic`] = **`SYNTHETIC_ENERGY_COST_
//   DEVIATION_V1`**: minimise energy cost against the versioned synthetic
//   price curve ([`SyntheticPriceCurve::v1_default`]) plus a quadratic
//   deviation penalty against a versioned synthetic reference trajectory.
//   Every type this module uses for cost inputs is synthetic by
//   construction and labelled `SYNTHETIC` (config record, evaluation
//   provenance, and diagnostics — three of the labelling places; the fourth
//   is the P3.21 record). D1 (jurisdiction/market) and D2 (first
//   portfolio/service) are OPEN: nothing here is a market price, a market
//   product, or a service qualification.
//
// - The deviation term of the default objective is schedule-vs-reference
//   deviation on a SYNTHETIC reference trajectory. It is NOT the
//   foundations §7.5 "commitment deviation" term (excluded, see above) and
//   NOT the §12.2 PCC tracking term: the tracking term consumes the P3.16
//   synthetic commitments and is implemented, with its own provenance, in
//   the `pcc_tracking` module. Conflating the three would be a silent
//   extension of the objective — recorded as a decision boundary in the
//   decision doc.
//
// TERM DEFINITIONS AND UNITS (canonical units per §10.2; MW, MWh):
//
//   | Term         | Definition                                        | Weight/coefficient unit            | Sign rule |
//   |---|---|---|---|
//   | energy       | `−w_energy · price_t · P_export,t · Δt`           | dimensionless multiplier           | the NEGATIVE price-times-energy product (revenue convention): export at a positive price EARNS a credit (negative cost), import at a positive price incurs cost; at a negative price the signs flip (export incurs cost, import earns a credit). `w_energy ≥ 0`; the sign lives in the price. |
//   | degradation  | `w_deg · Σ_assets placeholder_cost_a` (P3.06)     | dimensionless multiplier           | `w_deg ≥ 0`; the P3.06 rate is ≥ 0. The input is the P3.06 linear-throughput PLACEHOLDER with its [`DegradationMetadata`] — never a measured degradation law. |
//   | deviation    | `k_dev · (P_t − P_ref,t)² · Δt`                   | currency-units/(MW²·h), `k_dev ≥ 0`| a pure penalty (≥ 0); negative coefficients are rejected. |
//   | curtailment  | `k_cur · E_curtailed,t · Δt`                      | currency-units/MWh, `k_cur ≥ 0`    | a pure penalty on curtailed energy (≥ 0). |
//   | service      | `k_srv · E_service,t · Δt`                        | currency-units/MWh, sign DECLARED  | positive = cost for the delivered service energy; negative = credit/reward. The sign is configuration, recorded per schedule. |
//
//   "currency-units" is deliberately generic (the P3.06 precedent): the cost
//   unit is an objective-configuration choice, not a canonical platform
//   unit, and no market currency is implied (D1 OPEN).
//
// PROVENANCE AND VALIDATION (ledger P3.15):
//
// - Every ENABLED term must trace to a named, VERSIONED input source
//   ([`TermProvenanceRecord`], recorded on every evaluation): the synthetic
//   price curve version, the P3.06 degradation placeholder with its
//   placeholder metadata, the synthetic reference trajectory version, the
//   named curtailment rule version, the named service rule version. A term
//   built from a missing or unversioned input is a VALIDATION ERROR
//   ([`ObjectiveError::UnversionedTermInput`] /
//   [`ObjectiveError::TermInputMissing`]) — no untracked constants.
// - **Dominance check**: a term whose absolute total reaches
//   `dominance_threshold` (configuration, default
//   [`DEFAULT_DOMINANCE_THRESHOLD`] = 100.0 = two orders of magnitude)
//   times the largest absolute total among the OTHER enabled terms with a
//   nonzero total triggers a [`ScalingWarning`] recorded in the evaluation
//   diagnostics (ledger P3.15 step 2). With fewer than two nonzero enabled
//   terms there is nothing to dominate and no warning is raised (recorded
//   rule). Solver scaling itself is a P3.19 concern — this check only
//   records the review obligation.
// - **Zero- and negative-price behaviour** (documented, tested): on a
//   zero-price interval the energy term contributes exactly nothing and the
//   schedule there is governed by the other enabled terms; on a
//   negative-price interval the energy signs flip — export incurs cost and
//   import earns a credit, so the documented objective rewards consumption
//   there. Both regimes are carried per interval in
//   [`IntervalObjectiveCost::price_regime`] with an explainable
//   diagnostic string ([`PriceRegime::explain`]).
//
// SCHEDULE SELECTION: [`select_minimising_candidate`] is a pure argmin over
// an explicitly supplied candidate set (a designed fixture device, and a
// test seam) — it is NOT a solver and claims no optimality beyond the
// enumerated candidates. The Tier 1 optimizer (P3.17) owns real
// optimisation; the objective module only defines and evaluates the
// objective.
//
// Control status: pure rules for the simulation-only 0.x line. No I/O, no
// persistence, no device-write path; a solved schedule is a CANDIDATE —
// feasibility, authorization, and dispatch are Phase 4 gates and are never
// implied here.

use flexcore_domain::bess_model::{DegradationMetadata, DEGRADATION_PLACEHOLDER_STATEMENT};

// --- Recorded constants ------------------------------------------------------

/// The objective configuration schema version. Adding an objective term (an
/// [`ObjectiveTerm`] variant) or changing a term's meaning REQUIRES bumping
/// this version — a config with an unsupported version is rejected, so a
/// schema change can never pass silently (ledger P3.14: the §7.5 extras must
/// be a schema-version change, not a silent field).
pub const OBJECTIVE_SCHEMA_VERSION: u32 = 1;

/// The only objective schema version this module supports.
pub const SUPPORTED_OBJECTIVE_SCHEMA_VERSION: u32 = OBJECTIVE_SCHEMA_VERSION;

/// The named default v0.2.0 objective (D1/D2 OPEN): minimise energy cost
/// against the versioned synthetic price curve plus a quadratic deviation
/// penalty against a versioned synthetic reference trajectory. The
/// SYNTHETIC prefix is part of the recorded name — the label travels in the
/// schedule record and in every display of the objective (ledger P3.14
/// risk note).
pub const DEFAULT_SYNTHETIC_OBJECTIVE_NAME: &str = "SYNTHETIC_ENERGY_COST_DEVIATION_V1";

/// The provenance label carried by every objective this module defines.
/// All v0.2.0 objectives are synthetic (D1/D2 OPEN; 0.x simulation-only).
pub const SYNTHETIC_PROVENANCE_LABEL: &str = "SYNTHETIC";

/// Default dominance threshold (ledger P3.15 step 2): a term whose absolute
/// total reaches 100× (= two orders of magnitude) the largest absolute total
/// among the other enabled nonzero terms triggers the scaling warning.
/// Configuration, recorded in the decision doc; overridable per config.
pub const DEFAULT_DOMINANCE_THRESHOLD: f64 = 100.0;

/// Default deviation coefficient of the synthetic objective.
/// ASSUMPTION (recorded in the decision doc; ledger open question 6):
/// 50 currency-units/(MW²·h) — a planning-aid value with no market meaning.
pub const DEFAULT_DEVIATION_COEFFICIENT_PER_MW2_H: f64 = 50.0;

/// The recorded exclusion of the foundations §7.5 extra objective terms.
/// Carried in the config record output ([`ObjectiveConfig::record_fields`]),
/// not a comment — the exclusion is part of the schedule record.
pub const EXCLUDED_TERMS_STATEMENT: &str =
    "Foundations \u{a7}7.5 extra objective terms (flex discomfort, commitment \
deviation, reserve shortfall, switching costs) are NOT part of the v0.2.0 \
objective: \u{a7}7.5 lists them as terms an objective may minimize \
(illustrative, not mandatory), while report \u{a7}12.1 defines the v0.2.0 \
objective as exactly the five recorded terms. Later adoption is a recorded \
design decision requiring an objective schema-version change, never a \
quiet extension.";

/// Version of the recorded default synthetic price curve. Its values are
/// configuration recorded in the decision doc with an ASSUMPTION label —
/// no market prices are invented (D1 OPEN).
pub const SYNTHETIC_PRICE_CURVE_V1_VERSION: &str = "synthetic-price-curve-v1";

/// Version of the recorded default synthetic reference trajectory (the
/// deviation term's reference). A flat zero-export reference — the neutral
/// synthetic baseline, recorded in the decision doc with an ASSUMPTION
/// label.
pub const SYNTHETIC_REFERENCE_TRAJECTORY_V1_VERSION: &str =
    "synthetic-reference-trajectory-v1";

/// The ASSUMPTION note carried with the synthetic price curve provenance on
/// every evaluation that uses it.
pub const SYNTHETIC_CURVE_ASSUMPTION_NOTE: &str =
    "ASSUMPTION: synthetic price curve recorded as configuration in the \
decision doc (14_16_objective_pcc.md); not measured data, not a market \
price, not a forecast of any market (D1 OPEN).";

/// The recorded scaling-review statement carried on every
/// [`ScalingWarning`] (solver scaling itself is a P3.19 concern — this
/// warning records the review obligation only).
pub const SCALING_REVIEW_STATEMENT: &str =
    "Scaling review required: the term dominates the objective by more than \
the configured dominance threshold. This warning records the review \
obligation in the schedule diagnostics; solver scaling itself is a P3.19 \
concern.";

/// The recorded default synthetic price curve (currency-units/MWh), indexed
/// by UTC hour offset from the horizon start modulo 24. Recorded
/// configuration with an ASSUMPTION label (see
/// [`SYNTHETIC_CURVE_ASSUMPTION_NOTE`]); designed to exercise the
/// documented zero-price (hour 12 = 0.0) and negative-price (hour 20 =
/// −12.0) behaviour of ledger P3.15 step 3. No market meaning.
pub const SYNTHETIC_PRICE_CURVE_V1_HOURLY: [f64; 24] = [
    42.0, 38.0, 35.0, 33.0, 32.0, 36.0, // h00–h05 night valley
    55.0, 72.0, 80.0, 74.0, 60.0, 50.0, // h06–h11 morning ramp
    0.0, 44.0, 40.0, 38.0, 52.0, 78.0, // h12–h17 midday: recorded ZERO-price hour
    95.0, 105.0, -12.0, 60.0, 50.0, 46.0, // h18–h23 evening peak; h20 recorded NEGATIVE-price hour
];

// --- Errors ------------------------------------------------------------------

/// Errors returned by objective configuration, input, and evaluation
/// validation. Each variant names the offending term/field/value — never a
/// bare "invalid input" (P3.04 error-catalogue style).
#[derive(Debug, Clone, PartialEq)]
pub enum ObjectiveError {
    /// The config declared an objective schema version this module does not
    /// support (the term set changed without this module following).
    UnsupportedSchemaVersion {
        /// The declared version.
        found: u32,
        /// The version this module supports ([`SUPPORTED_OBJECTIVE_SCHEMA_VERSION`]).
        supported: u32,
    },
    /// The objective name was empty — every objective is explicitly named;
    /// no unnamed/universal objective exists.
    EmptyObjectiveName,
    /// A weight or coefficient was NaN or infinite.
    NonFiniteWeight {
        /// The offending term.
        term: ObjectiveTerm,
        /// The offending value.
        value: f64,
    },
    /// A weight or coefficient that must be non-negative was negative
    /// (energy, degradation, deviation, curtailment; the service
    /// coefficient's sign is declared and may be negative — see the module
    /// header table).
    NegativeWeight {
        /// The offending term.
        term: ObjectiveTerm,
        /// The offending value.
        value: f64,
    },
    /// The dominance threshold was non-finite or below 1.0 (a threshold
    /// below 1 would flag terms SMALLER than their peers — meaningless).
    InvalidDominanceThreshold {
        /// The offending value.
        value: f64,
    },
    /// An input series (price curve, reference, curtailment, service) was
    /// empty; every series must cover the scheduling intervals.
    EmptySeries {
        /// Which series.
        series: &'static str,
    },
    /// A series length did not match the candidate schedule's interval
    /// count.
    SeriesLengthMismatch {
        /// Which series.
        series: &'static str,
        /// The expected length (the candidate's interval count).
        expected: usize,
        /// The actual length.
        found: usize,
    },
    /// A series value was NaN or infinite.
    NonFiniteSeriesValue {
        /// Which series.
        series: &'static str,
        /// The interval index.
        interval: u32,
        /// The offending value.
        value: f64,
    },
    /// A magnitude series (curtailed MW, service MW) was negative; the
    /// sign convention lives in the coefficient, not the magnitude.
    NegativeMagnitude {
        /// Which series.
        series: &'static str,
        /// The interval index.
        interval: u32,
        /// The offending value.
        value: f64,
    },
    /// An enabled term's input was not supplied at all (e.g. energy enabled
    /// with no price curve). Every enabled term must trace to a named
    /// input.
    TermInputMissing {
        /// The term whose input is missing.
        term: ObjectiveTerm,
        /// What exactly is missing.
        detail: String,
    },
    /// An enabled term's input was supplied WITHOUT a version — a term
    /// built from an unversioned input is a validation error (ledger P3.15
    /// step 1: no untracked constants).
    UnversionedTermInput {
        /// The term whose input was unversioned.
        term: ObjectiveTerm,
        /// Which input was unversioned.
        detail: String,
    },
    /// The interval length must be positive.
    NonPositiveIntervalSeconds,
    /// A candidate schedule had an empty interval series.
    EmptyCandidateSchedule {
        /// The candidate label.
        label: String,
    },
    /// A candidate schedule value was NaN or infinite.
    NonFiniteCandidateValue {
        /// The candidate label.
        label: String,
        /// The interval index.
        interval: u32,
        /// The offending value.
        value: f64,
    },
    /// The candidate set for [`select_minimising_candidate`] was empty.
    EmptyCandidateSet,
    /// Candidates in a selection set had different interval counts; a
    /// selection compares alternatives over the SAME horizon.
    CandidateSetInconsistent {
        /// The first candidate's label (the reference length).
        label: String,
        /// The reference interval count.
        expected: usize,
        /// The offending candidate's interval count.
        found: usize,
    },
    /// A rule reference (curtailment/service) or input version string was
    /// empty or whitespace.
    InvalidRuleRef {
        /// Which rule reference.
        field: &'static str,
    },
}

impl std::fmt::Display for ObjectiveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ObjectiveError::UnsupportedSchemaVersion { found, supported } => write!(
                f,
                "objective schema version {found} is not supported (supported: {supported}); \
                 the objective term set changed and this config predates it"
            ),
            ObjectiveError::EmptyObjectiveName => {
                write!(f, "the objective name must be non-empty: no objective is hard-coded as universal")
            }
            ObjectiveError::NonFiniteWeight { term, value } => {
                write!(f, "{term} weight/coefficient must be finite (found {value})")
            }
            ObjectiveError::NegativeWeight { term, value } => {
                write!(f, "{term} weight/coefficient must be >= 0 (found {value}); the sign convention is recorded in the module header")
            }
            ObjectiveError::InvalidDominanceThreshold { value } => {
                write!(f, "dominance threshold must be finite and >= 1.0 (found {value})")
            }
            ObjectiveError::EmptySeries { series } => {
                write!(f, "series '{series}' must not be empty")
            }
            ObjectiveError::SeriesLengthMismatch { series, expected, found } => write!(
                f,
                "series '{series}' has {found} entries but the schedule has {expected} intervals"
            ),
            ObjectiveError::NonFiniteSeriesValue { series, interval, value } => {
                write!(f, "series '{series}' value at interval {interval} must be finite (found {value})")
            }
            ObjectiveError::NegativeMagnitude { series, interval, value } => write!(
                f,
                "series '{series}' value at interval {interval} must be >= 0 (found {value}); \
                 the sign convention lives in the coefficient, not the magnitude"
            ),
            ObjectiveError::TermInputMissing { term, detail } => {
                write!(f, "enabled term {term} is missing its input: {detail}")
            }
            ObjectiveError::UnversionedTermInput { term, detail } => write!(
                f,
                "enabled term {term} has an unversioned input: {detail} — a term built from \
                 an unversioned input is a validation error (ledger P3.15)"
            ),
            ObjectiveError::NonPositiveIntervalSeconds => {
                write!(f, "the interval length must be a positive number of seconds")
            }
            ObjectiveError::EmptyCandidateSchedule { label } => {
                write!(f, "candidate '{label}' has an empty interval series")
            }
            ObjectiveError::NonFiniteCandidateValue { label, interval, value } => {
                write!(f, "candidate '{label}' value at interval {interval} must be finite (found {value})")
            }
            ObjectiveError::EmptyCandidateSet => {
                write!(f, "the candidate set is empty; at least one candidate is required")
            }
            ObjectiveError::CandidateSetInconsistent { label, expected, found } => write!(
                f,
                "candidate '{label}' has {found} intervals but the candidate set is anchored \
                 at {expected} (selection compares alternatives over the same horizon)"
            ),
            ObjectiveError::InvalidRuleRef { field } => {
                write!(f, "rule reference '{field}' must be non-empty and whitespace-free")
            }
        }
    }
}

impl std::error::Error for ObjectiveError {}

// --- Terms -------------------------------------------------------------------

/// The closed vocabulary of §12.1 objective terms. Closed by construction:
/// adding a variant is an [`OBJECTIVE_SCHEMA_VERSION`] bump (see the module
/// header — the foundations §7.5 extras are deliberately absent).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ObjectiveTerm {
    /// `C_energy` — energy settlement against the price curve.
    Energy,
    /// `C_degradation` — the P3.06 linear-throughput placeholder cost.
    Degradation,
    /// `C_deviation` — quadratic deviation from the versioned reference.
    Deviation,
    /// `C_curtailment` — penalty on curtailed energy.
    Curtailment,
    /// `C_service` — declared-sign cost/credit on delivered service energy.
    Service,
}

impl ObjectiveTerm {
    /// The canonical term name used in records and diagnostics.
    pub const fn as_str(self) -> &'static str {
        match self {
            ObjectiveTerm::Energy => "energy",
            ObjectiveTerm::Degradation => "degradation",
            ObjectiveTerm::Deviation => "deviation",
            ObjectiveTerm::Curtailment => "curtailment",
            ObjectiveTerm::Service => "service",
        }
    }

    /// The declared unit of the term's weight/coefficient (the module
    /// header table, in record form).
    pub const fn weight_unit(self) -> &'static str {
        match self {
            ObjectiveTerm::Energy => "dimensionless multiplier on (price [currency-units/MWh] x energy [MWh])",
            ObjectiveTerm::Degradation => "dimensionless multiplier on the P3.06 placeholder cost [currency-units]",
            ObjectiveTerm::Deviation => "currency-units/(MW^2*h)",
            ObjectiveTerm::Curtailment => "currency-units/MWh",
            ObjectiveTerm::Service => "currency-units/MWh (sign declared: negative = credit)",
        }
    }

    /// The declared sign convention of the term (recorded per schedule).
    pub const fn sign_convention(self) -> &'static str {
        match self {
            ObjectiveTerm::Energy => "term = -w * price * P_export * dt: export at a positive price earns a credit (negative cost), import at a positive price incurs cost; at a negative price the signs flip (export incurs cost, import earns a credit)",
            ObjectiveTerm::Degradation => "term = w * P3.06 placeholder cost (rate >= 0 per MWh of charge+discharge throughput); placeholder, not a measured law",
            ObjectiveTerm::Deviation => "term = k * (P_t - P_ref,t)^2 * dt, k >= 0: a pure penalty",
            ObjectiveTerm::Curtailment => "term = k * curtailed_energy * dt, k >= 0: a pure penalty",
            ObjectiveTerm::Service => "term = k * service_energy * dt; sign declared per config: positive = cost, negative = credit",
        }
    }

    /// All five terms, in §12.1 formula order.
    pub const ALL: [ObjectiveTerm; 5] = [
        ObjectiveTerm::Energy,
        ObjectiveTerm::Degradation,
        ObjectiveTerm::Deviation,
        ObjectiveTerm::Curtailment,
        ObjectiveTerm::Service,
    ];
}

impl std::fmt::Display for ObjectiveTerm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The objective provenance: an enum with exactly one variant. Every
/// objective this module defines is synthetic (D1/D2 OPEN; 0.x
/// simulation-only line) — there is no market-objective variant to
/// construct.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ObjectiveProvenance {
    /// Synthetic configuration; not a market product.
    Synthetic,
}

impl ObjectiveProvenance {
    /// The canonical label spelling (rendered in the schedule record).
    pub const fn as_str(self) -> &'static str {
        SYNTHETIC_PROVENANCE_LABEL
    }
}

// --- Versioned synthetic inputs ----------------------------------------------

/// The synthetic price curve: the ONLY price input this module accepts,
/// synthetic by construction and versioned by requirement. An empty version
/// is a validation error — an unversioned price curve can never reach an
/// evaluation (ledger P3.15 step 1).
#[derive(Debug, Clone, PartialEq)]
pub struct SyntheticPriceCurve {
    version: String,
    prices_per_interval: Vec<f64>,
}

impl SyntheticPriceCurve {
    /// Constructs and validates a synthetic price curve: non-empty,
    /// whitespace-free version; non-empty series; finite values (any sign —
    /// the documented zero- and negative-price behaviour lives in the
    /// energy term's sign convention).
    pub fn new(version: &str, prices_per_interval: Vec<f64>) -> Result<Self, ObjectiveError> {
        if version.is_empty() || version.chars().any(|c| c.is_whitespace()) {
            return Err(ObjectiveError::UnversionedTermInput {
                term: ObjectiveTerm::Energy,
                detail: "the price curve version must be non-empty and whitespace-free".to_string(),
            });
        }
        if prices_per_interval.is_empty() {
            return Err(ObjectiveError::EmptySeries { series: "price_curve" });
        }
        for (i, p) in prices_per_interval.iter().enumerate() {
            if !p.is_finite() {
                return Err(ObjectiveError::NonFiniteSeriesValue {
                    series: "price_curve",
                    interval: i as u32,
                    value: *p,
                });
            }
        }
        Ok(Self {
            version: version.to_string(),
            prices_per_interval,
        })
    }

    /// The recorded default curve ([`SYNTHETIC_PRICE_CURVE_V1_VERSION`])
    /// expanded onto a scheduling grid: the price of interval `i` is the
    /// hourly value at `(i * interval_seconds / 3600) % 24` (hour offset
    /// from the horizon start, modulo 24 — recorded in the decision doc).
    pub fn v1_default(interval_count: usize, interval_seconds: u32) -> Self {
        assert!(interval_seconds > 0, "interval_seconds must be positive");
        assert!(interval_count > 0, "interval_count must be positive");
        let prices = (0..interval_count)
            .map(|i| {
                let hour = ((i as u64 * interval_seconds as u64) / 3600) % 24;
                SYNTHETIC_PRICE_CURVE_V1_HOURLY[hour as usize]
            })
            .collect();
        Self {
            version: SYNTHETIC_PRICE_CURVE_V1_VERSION.to_string(),
            prices_per_interval: prices,
        }
    }

    /// The curve version (travels into the term provenance record).
    pub fn version(&self) -> &str {
        &self.version
    }

    /// The per-interval prices in currency-units/MWh.
    pub fn prices_per_interval(&self) -> &[f64] {
        &self.prices_per_interval
    }
}

/// A versioned synthetic series (the deviation term's reference
/// trajectory). Synthetic by construction, versioned by requirement.
#[derive(Debug, Clone, PartialEq)]
pub struct SyntheticSeries {
    version: String,
    values_mw: Vec<f64>,
}

impl SyntheticSeries {
    /// Constructs and validates the series: non-empty, whitespace-free
    /// version; non-empty series; finite values.
    pub fn new(version: &str, values_mw: Vec<f64>) -> Result<Self, ObjectiveError> {
        if version.is_empty() || version.chars().any(|c| c.is_whitespace()) {
            return Err(ObjectiveError::UnversionedTermInput {
                term: ObjectiveTerm::Deviation,
                detail: "the reference trajectory version must be non-empty and whitespace-free"
                    .to_string(),
            });
        }
        if values_mw.is_empty() {
            return Err(ObjectiveError::EmptySeries { series: "reference" });
        }
        for (i, v) in values_mw.iter().enumerate() {
            if !v.is_finite() {
                return Err(ObjectiveError::NonFiniteSeriesValue {
                    series: "reference",
                    interval: i as u32,
                    value: *v,
                });
            }
        }
        Ok(Self {
            version: version.to_string(),
            values_mw,
        })
    }

    /// The recorded default reference: flat zero export — the neutral
    /// synthetic baseline ([`SYNTHETIC_REFERENCE_TRAJECTORY_V1_VERSION`];
    /// recorded in the decision doc with an ASSUMPTION label).
    pub fn v1_default_flat_zero(interval_count: usize) -> Self {
        assert!(interval_count > 0, "interval_count must be positive");
        Self {
            version: SYNTHETIC_REFERENCE_TRAJECTORY_V1_VERSION.to_string(),
            values_mw: vec![0.0; interval_count],
        }
    }

    /// The series version.
    pub fn version(&self) -> &str {
        &self.version
    }

    /// The per-interval reference values in MW (net export sign: `P > 0`
    /// export, §10.2).
    pub fn values_mw(&self) -> &[f64] {
        &self.values_mw
    }
}

/// A named, versioned rule reference (the curtailment rule, the service
/// rule). Both fields are required and validated — a rule without a
/// version is an untracked input and is rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleRef {
    name: String,
    version: String,
}

impl RuleRef {
    /// Constructs and validates the rule reference: name and version must
    /// be non-empty and whitespace-free.
    pub fn new(name: &str, version: &str) -> Result<Self, ObjectiveError> {
        for (field, value) in [("name", name), ("version", version)] {
            if value.is_empty() || value.chars().any(|c| c.is_whitespace()) {
                return Err(ObjectiveError::InvalidRuleRef { field });
            }
        }
        Ok(Self {
            name: name.to_string(),
            version: version.to_string(),
        })
    }

    /// The rule name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The rule version.
    pub fn version(&self) -> &str {
        &self.version
    }
}

/// One BESS asset's P3.06 degradation placeholder metadata, as consumed by
/// the degradation term. The metadata (not a bare rate) is the input, so
/// the placeholder status travels with the term (ledger P3.15 step 1).
#[derive(Debug, Clone, PartialEq)]
pub struct DegradationTermInput {
    /// The BESS asset identifier (registry reference string form).
    pub asset_id: String,
    /// The P3.06 metadata from `BessModel::evaluate_schedule`.
    pub metadata: DegradationMetadata,
}

// --- Configuration -----------------------------------------------------------

/// Energy-term configuration: `C_energy,t = −weight · price_t · P_t · Δt`.
/// The weight is a dimensionless multiplier ≥ 0; the sign convention lives
/// in the term definition (see the module header table).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EnergyTermConfig {
    /// Whether the term participates in the objective.
    pub enabled: bool,
    /// Dimensionless multiplier (≥ 0).
    pub weight: f64,
}

/// Degradation-term configuration: `C_degradation = weight · Σ P3.06
/// placeholder costs`. The weight is a dimensionless multiplier ≥ 0 on a
/// PLACEHOLDER cost — never a measured degradation law (P3.06).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DegradationTermConfig {
    /// Whether the term participates in the objective.
    pub enabled: bool,
    /// Dimensionless multiplier (≥ 0).
    pub weight: f64,
}

/// Deviation-term configuration: `C_deviation,t = coefficient · (P_t −
/// P_ref,t)² · Δt`. The coefficient carries units (currency-units/(MW²·h))
/// and must be ≥ 0 (a pure penalty).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DeviationTermConfig {
    /// Whether the term participates in the objective.
    pub enabled: bool,
    /// Coefficient in currency-units/(MW²·h) (≥ 0).
    pub coefficient_per_mw2_h: f64,
}

/// Curtailment-term configuration: `C_curtailment,t = coefficient ·
/// E_curtailed,t · Δt`. The coefficient carries units (currency-units/MWh)
/// and must be ≥ 0 (a pure penalty on curtailed energy).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CurtailmentTermConfig {
    /// Whether the term participates in the objective.
    pub enabled: bool,
    /// Coefficient in currency-units/MWh (≥ 0).
    pub coefficient_per_mwh: f64,
}

/// Service-term configuration: `C_service,t = coefficient · E_service,t ·
/// Δt`. The coefficient carries units (currency-units/MWh); its SIGN IS
/// DECLARED per config (positive = cost, negative = credit/reward) and is
/// recorded per schedule — the one term whose sign is a configuration
/// choice (§12.1: sign conventions are region/product-specific).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ServiceTermConfig {
    /// Whether the term participates in the objective.
    pub enabled: bool,
    /// Coefficient in currency-units/MWh (finite; sign declared).
    pub coefficient_per_mwh: f64,
}

/// The scheduling objective configuration (report §12.1): an explicitly
/// NAMED, fully recorded objective with per-term weights, signs, and units.
/// This type serialises (as ordered record fields) into the P3.21 schedule
/// record, so two schedules with different objectives are never compared as
/// if they were the same problem.
#[derive(Debug, Clone, PartialEq)]
pub struct ObjectiveConfig {
    schema_version: u32,
    name: String,
    provenance: ObjectiveProvenance,
    energy: EnergyTermConfig,
    degradation: DegradationTermConfig,
    deviation: DeviationTermConfig,
    curtailment: CurtailmentTermConfig,
    service: ServiceTermConfig,
    dominance_threshold: f64,
}

impl ObjectiveConfig {
    /// Constructs and validates an objective configuration.
    ///
    /// Validation: supported schema version (a term-set change without this
    /// module following is rejected, never silently accepted); non-empty
    /// name (no universal/unnamed objective); finite weights with the
    /// recorded sign rules (energy/degradation/deviation/curtailment ≥ 0;
    /// service sign declared); dominance threshold finite and ≥ 1.0.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        schema_version: u32,
        name: String,
        provenance: ObjectiveProvenance,
        energy: EnergyTermConfig,
        degradation: DegradationTermConfig,
        deviation: DeviationTermConfig,
        curtailment: CurtailmentTermConfig,
        service: ServiceTermConfig,
        dominance_threshold: f64,
    ) -> Result<Self, ObjectiveError> {
        if schema_version != SUPPORTED_OBJECTIVE_SCHEMA_VERSION {
            return Err(ObjectiveError::UnsupportedSchemaVersion {
                found: schema_version,
                supported: SUPPORTED_OBJECTIVE_SCHEMA_VERSION,
            });
        }
        if name.is_empty() {
            return Err(ObjectiveError::EmptyObjectiveName);
        }
        if !energy.weight.is_finite() {
            return Err(ObjectiveError::NonFiniteWeight {
                term: ObjectiveTerm::Energy,
                value: energy.weight,
            });
        }
        if energy.weight < 0.0 {
            return Err(ObjectiveError::NegativeWeight {
                term: ObjectiveTerm::Energy,
                value: energy.weight,
            });
        }
        if !degradation.weight.is_finite() {
            return Err(ObjectiveError::NonFiniteWeight {
                term: ObjectiveTerm::Degradation,
                value: degradation.weight,
            });
        }
        if degradation.weight < 0.0 {
            return Err(ObjectiveError::NegativeWeight {
                term: ObjectiveTerm::Degradation,
                value: degradation.weight,
            });
        }
        if !deviation.coefficient_per_mw2_h.is_finite() {
            return Err(ObjectiveError::NonFiniteWeight {
                term: ObjectiveTerm::Deviation,
                value: deviation.coefficient_per_mw2_h,
            });
        }
        if deviation.coefficient_per_mw2_h < 0.0 {
            return Err(ObjectiveError::NegativeWeight {
                term: ObjectiveTerm::Deviation,
                value: deviation.coefficient_per_mw2_h,
            });
        }
        if !curtailment.coefficient_per_mwh.is_finite() {
            return Err(ObjectiveError::NonFiniteWeight {
                term: ObjectiveTerm::Curtailment,
                value: curtailment.coefficient_per_mwh,
            });
        }
        if curtailment.coefficient_per_mwh < 0.0 {
            return Err(ObjectiveError::NegativeWeight {
                term: ObjectiveTerm::Curtailment,
                value: curtailment.coefficient_per_mwh,
            });
        }
        if !service.coefficient_per_mwh.is_finite() {
            return Err(ObjectiveError::NonFiniteWeight {
                term: ObjectiveTerm::Service,
                value: service.coefficient_per_mwh,
            });
        }
        if !dominance_threshold.is_finite() || dominance_threshold < 1.0 {
            return Err(ObjectiveError::InvalidDominanceThreshold {
                value: dominance_threshold,
            });
        }
        Ok(Self {
            schema_version,
            name,
            provenance,
            energy,
            degradation,
            deviation,
            curtailment,
            service,
            dominance_threshold,
        })
    }

    /// The named default v0.2.0 objective
    /// ([`DEFAULT_SYNTHETIC_OBJECTIVE_NAME`]): energy term on (weight 1.0)
    /// plus the deviation penalty
    /// ([`DEFAULT_DEVIATION_COEFFICIENT_PER_MW2_H`]); degradation,
    /// curtailment, and service disabled (switchable per config); the
    /// default dominance threshold. SYNTHETIC everywhere — not a market
    /// product (D1/D2 OPEN).
    pub fn default_synthetic() -> Self {
        Self {
            schema_version: SUPPORTED_OBJECTIVE_SCHEMA_VERSION,
            name: DEFAULT_SYNTHETIC_OBJECTIVE_NAME.to_string(),
            provenance: ObjectiveProvenance::Synthetic,
            energy: EnergyTermConfig { enabled: true, weight: 1.0 },
            degradation: DegradationTermConfig { enabled: false, weight: 0.0 },
            deviation: DeviationTermConfig {
                enabled: true,
                coefficient_per_mw2_h: DEFAULT_DEVIATION_COEFFICIENT_PER_MW2_H,
            },
            curtailment: CurtailmentTermConfig { enabled: false, coefficient_per_mwh: 0.0 },
            service: ServiceTermConfig { enabled: false, coefficient_per_mwh: 0.0 },
            dominance_threshold: DEFAULT_DOMINANCE_THRESHOLD,
        }
    }

    /// The schema version.
    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// The explicit objective name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The objective provenance label.
    pub const fn provenance(&self) -> ObjectiveProvenance {
        self.provenance
    }

    /// The energy-term configuration.
    pub const fn energy(&self) -> &EnergyTermConfig {
        &self.energy
    }

    /// The degradation-term configuration.
    pub const fn degradation(&self) -> &DegradationTermConfig {
        &self.degradation
    }

    /// The deviation-term configuration.
    pub const fn deviation(&self) -> &DeviationTermConfig {
        &self.deviation
    }

    /// The curtailment-term configuration.
    pub const fn curtailment(&self) -> &CurtailmentTermConfig {
        &self.curtailment
    }

    /// The service-term configuration.
    pub const fn service(&self) -> &ServiceTermConfig {
        &self.service
    }

    /// The configured dominance threshold.
    pub const fn dominance_threshold(&self) -> f64 {
        self.dominance_threshold
    }

    /// Whether two configs declare the SAME problem (name, schema, term
    /// weights and signs, threshold). Two schedules produced under configs
    /// where this returns false are never comparable as the same problem
    /// (ledger P3.14 step 3).
    pub fn same_problem(&self, other: &ObjectiveConfig) -> bool {
        self == other
    }

    /// The ordered record fields this config contributes to the P3.21
    /// schedule record: objective identity, provenance label, every term's
    /// enablement/weight/unit/sign, the dominance threshold, and the
    /// recorded §7.5 exclusion statement. The record is the anti-conflation
    /// mechanism: two schedules with different objectives carry visibly
    /// different records.
    pub fn record_fields(&self) -> Vec<(String, String)> {
        let mut fields = vec![
            ("objective.schema_version".to_string(), self.schema_version.to_string()),
            ("objective.name".to_string(), self.name.clone()),
            ("objective.provenance".to_string(), self.provenance.as_str().to_string()),
            (
                "objective.not_a_market_product".to_string(),
                "true: synthetic objective for the simulation-only 0.x line (D1/D2 OPEN)".to_string(),
            ),
        ];
        for term in ObjectiveTerm::ALL {
            let (enabled, weight) = match term {
                ObjectiveTerm::Energy => (self.energy.enabled, self.energy.weight),
                ObjectiveTerm::Degradation => {
                    (self.degradation.enabled, self.degradation.weight)
                }
                ObjectiveTerm::Deviation => (
                    self.deviation.enabled,
                    self.deviation.coefficient_per_mw2_h,
                ),
                ObjectiveTerm::Curtailment => (
                    self.curtailment.enabled,
                    self.curtailment.coefficient_per_mwh,
                ),
                ObjectiveTerm::Service => (
                    self.service.enabled,
                    self.service.coefficient_per_mwh,
                ),
            };
            fields.push((format!("term.{term}.enabled"), enabled.to_string()));
            fields.push((format!("term.{term}.weight"), format!("{weight}")));
            fields.push((format!("term.{term}.weight_unit"), term.weight_unit().to_string()));
            fields.push((format!("term.{term}.sign_convention"), term.sign_convention().to_string()));
        }
        fields.push((
            "objective.dominance_threshold".to_string(),
            format!("{}", self.dominance_threshold),
        ));
        fields.push((
            "objective.excluded_terms".to_string(),
            EXCLUDED_TERMS_STATEMENT.to_string(),
        ));
        fields
    }
}

// --- Candidate and inputs ----------------------------------------------------

/// One candidate portfolio schedule over the scheduling intervals: the net
/// active power at the declared evaluation point, in MW with the canonical
/// sign (`P > 0` export, §10.2). The §12.1 objective evaluates the
/// caller-declared net position; deriving it at the PCC (with losses) is
/// the P3.16 module's concern and the Tier 1 formulation's composition
/// (P3.17) — this module does not re-derive it.
#[derive(Debug, Clone, PartialEq)]
pub struct CandidatePortfolioSchedule {
    /// A label identifying the candidate in selection outcomes and errors.
    pub label: String,
    /// Per-interval net export in MW (`P > 0` export).
    pub net_export_mw: Vec<f64>,
}

/// The objective evaluation inputs. Every enabled term's input must be
/// present and versioned (see the module header); `Default` yields an
/// inputs value with no term inputs — terms enabled in the config but
/// missing here are validation errors at evaluation time.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ObjectiveInputs {
    /// The scheduling interval length in seconds (must be > 0).
    pub interval_seconds: u32,
    /// The versioned synthetic price curve (required iff the energy term is
    /// enabled).
    pub price_curve: Option<SyntheticPriceCurve>,
    /// The versioned synthetic reference trajectory in MW (required iff the
    /// deviation term is enabled).
    pub reference: Option<SyntheticSeries>,
    /// The per-asset P3.06 degradation placeholder metadata (required
    /// non-empty iff the degradation term is enabled).
    pub degradation: Vec<DegradationTermInput>,
    /// The named, versioned curtailment rule (required iff the curtailment
    /// term is enabled).
    pub curtailment_rule: Option<RuleRef>,
    /// The per-interval curtailed power in MW (≥ 0; required iff the
    /// curtailment term is enabled).
    pub curtailed_mw: Option<Vec<f64>>,
    /// The named, versioned service rule (required iff the service term is
    /// enabled).
    pub service_rule: Option<RuleRef>,
    /// The per-interval delivered service power in MW (≥ 0; required iff
    /// the service term is enabled).
    pub service_mw: Option<Vec<f64>>,
}

impl ObjectiveInputs {
    /// An inputs value with the given interval length and no term inputs.
    pub fn new(interval_seconds: u32) -> Self {
        Self { interval_seconds, ..Default::default() }
    }
}

// --- Evaluation shapes -------------------------------------------------------

/// The price regime of an interval, with the documented behaviour
/// explanation (ledger P3.15 step 3: zero/negative-price behaviour is
/// documented and explainable in diagnostics).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PriceRegime {
    /// price > 0.
    Positive,
    /// price == 0.
    Zero,
    /// price < 0.
    Negative,
}

impl PriceRegime {
    /// Classifies a price.
    pub fn of(price: f64) -> Self {
        if price > 0.0 {
            PriceRegime::Positive
        } else if price < 0.0 {
            PriceRegime::Negative
        } else {
            PriceRegime::Zero
        }
    }

    /// The documented behaviour of the energy term in this regime.
    pub const fn explain(self) -> &'static str {
        match self {
            PriceRegime::Positive => {
                "positive price: export earns a credit (negative energy cost), import incurs cost"
            }
            PriceRegime::Zero => {
                "zero price: the energy term contributes exactly nothing this interval; \
                 the schedule here is governed by the other enabled terms (e.g. deviation)"
            }
            PriceRegime::Negative => {
                "negative price: the energy signs flip — export incurs cost and import \
                 earns a credit; the documented objective rewards consumption on this interval"
            }
        }
    }
}

/// The per-interval cost breakdown of the interval-decomposable terms. The
/// degradation term is reported in the totals only: the P3.06
/// [`DegradationMetadata`] carries schedule totals (throughput, cost), not
/// per-interval costs, and this module does not re-derive what the P3.06
/// evaluation already recorded (recorded honestly rather than split
/// silently).
#[derive(Debug, Clone, PartialEq)]
pub struct IntervalObjectiveCost {
    /// The interval index.
    pub interval: u32,
    /// The price regime, when the energy term is enabled (None otherwise).
    pub price_regime: Option<PriceRegime>,
    /// `C_energy,t` in currency-units (0.0 when the term is disabled).
    pub energy: f64,
    /// `C_deviation,t` in currency-units (0.0 when the term is disabled).
    pub deviation: f64,
    /// `C_curtailment,t` in currency-units (0.0 when the term is disabled).
    pub curtailment: f64,
    /// `C_service,t` in currency-units (0.0 when the term is disabled).
    pub service: f64,
    /// The human-readable explanation of this interval's costs (regime
    /// explanation plus the term contributions) — the diagnostics channel
    /// the ledger requires for zero/negative-price intervals.
    pub diagnostics: String,
}

/// The per-term totals of an evaluation, in currency-units.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct TermTotals {
    /// Total `C_energy` over the horizon.
    pub energy: f64,
    /// Total `C_degradation` over the horizon (the P3.06 total, weighted).
    pub degradation: f64,
    /// Total `C_deviation` over the horizon.
    pub deviation: f64,
    /// Total `C_curtailment` over the horizon.
    pub curtailment: f64,
    /// Total `C_service` over the horizon.
    pub service: f64,
}

impl TermTotals {
    /// The value of the given term.
    pub fn get(self, term: ObjectiveTerm) -> f64 {
        match term {
            ObjectiveTerm::Energy => self.energy,
            ObjectiveTerm::Degradation => self.degradation,
            ObjectiveTerm::Deviation => self.deviation,
            ObjectiveTerm::Curtailment => self.curtailment,
            ObjectiveTerm::Service => self.service,
        }
    }

    /// The objective total Σ over the enabled terms (disabled terms
    /// contribute 0.0).
    pub fn objective_total(&self) -> f64 {
        self.energy + self.degradation + self.deviation + self.curtailment + self.service
    }
}

/// A recorded scaling warning (ledger P3.15 step 2): the named term
/// dominates the objective by at least the configured threshold relative to
/// the largest other enabled nonzero term. Recorded in the evaluation
/// diagnostics; the solver-scaling work itself is P3.19.
#[derive(Debug, Clone, PartialEq)]
pub struct ScalingWarning {
    /// The dominating term.
    pub term: ObjectiveTerm,
    /// The dominating term's absolute total.
    pub term_total: f64,
    /// The largest other term (by absolute total) among the enabled
    /// nonzero terms.
    pub other_term: ObjectiveTerm,
    /// The other term's absolute total.
    pub other_term_total: f64,
    /// `|term_total| / |other_term_total|`.
    pub ratio: f64,
    /// The configured threshold that fired.
    pub threshold: f64,
    /// Always true: the warning IS the recorded review obligation.
    pub review_required: bool,
    /// The recorded scaling-review statement
    /// ([`SCALING_REVIEW_STATEMENT`]).
    pub statement: &'static str,
}

/// The provenance of one enabled term's input (ledger P3.15 step 1: every
/// term traces to a named input version — no untracked constants).
#[derive(Debug, Clone, PartialEq)]
pub struct TermProvenanceRecord {
    /// The term.
    pub term: ObjectiveTerm,
    /// The named input source (e.g. "synthetic price curve",
    /// "P3.06 degradation placeholder").
    pub source: String,
    /// The input's version (or, for the P3.06 placeholder, its recorded
    /// placeholder identity — the placeholder status itself is the
    /// traceability).
    pub version: String,
    /// The recorded note (ASSUMPTION/placeholder statements travel here).
    pub note: String,
}

/// The result of evaluating one candidate schedule under an objective
/// config: totals, the per-interval breakdown, term provenance, and the
/// recorded scaling warnings (the diagnostics shape the ledger requires).
#[derive(Debug, Clone, PartialEq)]
pub struct ObjectiveEvaluation {
    /// The objective name (from the config; recorded per schedule).
    pub objective_name: String,
    /// The objective schema version.
    pub schema_version: u32,
    /// The objective provenance label (SYNTHETIC for every objective this
    /// module defines).
    pub provenance: ObjectiveProvenance,
    /// The number of intervals evaluated.
    pub interval_count: usize,
    /// The interval length in seconds.
    pub interval_seconds: u32,
    /// The per-term totals in currency-units.
    pub totals: TermTotals,
    /// The objective total (Σ over the enabled terms).
    pub total: f64,
    /// The per-interval breakdown of the interval-decomposable terms.
    pub per_interval: Vec<IntervalObjectiveCost>,
    /// The provenance of every enabled term's input.
    pub term_provenance: Vec<TermProvenanceRecord>,
    /// The recorded scaling warnings (empty when no term dominates).
    pub scaling_warnings: Vec<ScalingWarning>,
    /// The degradation note, when the degradation term is enabled: records
    /// whether the P3.06 placeholder was configured per asset and repeats
    /// the placeholder statement — the P3.06 metadata travels to the
    /// schedule diagnostics.
    pub degradation_note: Option<String>,
}

// --- Evaluation --------------------------------------------------------------

fn check_series(
    series: &'static str,
    values: &[f64],
    interval_count: usize,
    non_negative: bool,
) -> Result<(), ObjectiveError> {
    if values.len() != interval_count {
        return Err(ObjectiveError::SeriesLengthMismatch {
            series,
            expected: interval_count,
            found: values.len(),
        });
    }
    for (i, v) in values.iter().enumerate() {
        if !v.is_finite() {
            return Err(ObjectiveError::NonFiniteSeriesValue {
                series,
                interval: i as u32,
                value: *v,
            });
        }
        if non_negative && *v < 0.0 {
            return Err(ObjectiveError::NegativeMagnitude {
                series,
                interval: i as u32,
                value: *v,
            });
        }
    }
    Ok(())
}

/// Evaluates the objective for one candidate schedule under the config
/// (report §12.1). Validates that every ENABLED term has a present,
/// versioned input (ledger P3.15 step 1) and records the dominance check
/// (step 2) plus the per-interval price-regime diagnostics (step 3).
pub fn evaluate_objective(
    config: &ObjectiveConfig,
    candidate: &CandidatePortfolioSchedule,
    inputs: &ObjectiveInputs,
) -> Result<ObjectiveEvaluation, ObjectiveError> {
    if inputs.interval_seconds == 0 {
        return Err(ObjectiveError::NonPositiveIntervalSeconds);
    }
    if candidate.net_export_mw.is_empty() {
        return Err(ObjectiveError::EmptyCandidateSchedule { label: candidate.label.clone() });
    }
    for (i, v) in candidate.net_export_mw.iter().enumerate() {
        if !v.is_finite() {
            return Err(ObjectiveError::NonFiniteCandidateValue {
                label: candidate.label.clone(),
                interval: i as u32,
                value: *v,
            });
        }
    }
    let n = candidate.net_export_mw.len();
    let dt_h = inputs.interval_seconds as f64 / 3600.0;

    // --- Per-term input resolution (validation errors on missing/unversioned).
    let curve = if config.energy.enabled {
        let curve = inputs.price_curve.as_ref().ok_or_else(|| ObjectiveError::TermInputMissing {
            term: ObjectiveTerm::Energy,
            detail: "no price curve was supplied".to_string(),
        })?;
        if curve.version().is_empty() {
            return Err(ObjectiveError::UnversionedTermInput {
                term: ObjectiveTerm::Energy,
                detail: "the supplied price curve has an empty version".to_string(),
            });
        }
        check_series("price_curve", curve.prices_per_interval(), n, false)?;
        Some(curve)
    } else {
        None
    };

    let reference = if config.deviation.enabled {
        let reference = inputs.reference.as_ref().ok_or_else(|| {
            ObjectiveError::TermInputMissing {
                term: ObjectiveTerm::Deviation,
                detail: "no reference trajectory was supplied".to_string(),
            }
        })?;
        if reference.version().is_empty() {
            return Err(ObjectiveError::UnversionedTermInput {
                term: ObjectiveTerm::Deviation,
                detail: "the supplied reference trajectory has an empty version".to_string(),
            });
        }
        check_series("reference", reference.values_mw(), n, false)?;
        Some(reference)
    } else {
        None
    };

    if config.degradation.enabled && inputs.degradation.is_empty() {
        return Err(ObjectiveError::TermInputMissing {
            term: ObjectiveTerm::Degradation,
            detail: "no P3.06 degradation metadata was supplied".to_string(),
        });
    }
    for input in &inputs.degradation {
        if input.metadata.placeholder_statement != DEGRADATION_PLACEHOLDER_STATEMENT {
            return Err(ObjectiveError::TermInputMissing {
                term: ObjectiveTerm::Degradation,
                detail: format!(
                    "asset {} carries degradation metadata whose placeholder statement does \
                     not match the recorded P3.06 statement — the metadata must come from the \
                     P3.06 evaluation, not be restated here",
                    input.asset_id
                ),
            });
        }
    }

    let curtailment_rule = if config.curtailment.enabled {
        let rule = inputs
            .curtailment_rule
            .as_ref()
            .ok_or_else(|| ObjectiveError::TermInputMissing {
                term: ObjectiveTerm::Curtailment,
                detail: "no curtailment rule reference was supplied".to_string(),
            })?;
        if rule.version().is_empty() {
            return Err(ObjectiveError::UnversionedTermInput {
                term: ObjectiveTerm::Curtailment,
                detail: "the supplied curtailment rule has an empty version".to_string(),
            });
        }
        let series = inputs.curtailed_mw.as_ref().ok_or_else(|| {
            ObjectiveError::TermInputMissing {
                term: ObjectiveTerm::Curtailment,
                detail: "no curtailed-power series was supplied".to_string(),
            }
        })?;
        check_series("curtailed_mw", series, n, true)?;
        Some(rule)
    } else {
        None
    };

    let service_rule = if config.service.enabled {
        let rule =
            inputs.service_rule.as_ref().ok_or_else(|| ObjectiveError::TermInputMissing {
                term: ObjectiveTerm::Service,
                detail: "no service rule reference was supplied".to_string(),
            })?;
        if rule.version().is_empty() {
            return Err(ObjectiveError::UnversionedTermInput {
                term: ObjectiveTerm::Service,
                detail: "the supplied service rule has an empty version".to_string(),
            });
        }
        let series =
            inputs.service_mw.as_ref().ok_or_else(|| ObjectiveError::TermInputMissing {
                term: ObjectiveTerm::Service,
                detail: "no service-power series was supplied".to_string(),
            })?;
        check_series("service_mw", series, n, true)?;
        Some(rule)
    } else {
        None
    };

    // --- Per-interval costs.
    let mut per_interval = Vec::with_capacity(n);
    let mut totals = TermTotals::default();
    for t in 0..n {
        let p = candidate.net_export_mw[t];
        let mut interval = IntervalObjectiveCost {
            interval: t as u32,
            price_regime: None,
            energy: 0.0,
            deviation: 0.0,
            curtailment: 0.0,
            service: 0.0,
            diagnostics: String::new(),
        };
        let mut notes: Vec<String> = Vec::new();
        if let Some(curve) = curve {
            let price = curve.prices_per_interval()[t];
            let regime = PriceRegime::of(price);
            interval.price_regime = Some(regime);
            interval.energy = -config.energy.weight * price * p * dt_h;
            notes.push(format!(
                "{}; P = {p} MW at {price} currency-units/MWh -> C_energy = {}",
                regime.explain(),
                interval.energy
            ));
        }
        if let Some(reference) = reference {
            let ref_p = reference.values_mw()[t];
            let gap = p - ref_p;
            interval.deviation = config.deviation.coefficient_per_mw2_h * gap * gap * dt_h;
            notes.push(format!(
                "deviation from reference {} MW (version {}): gap = {gap} MW -> C_deviation = {}",
                ref_p,
                reference.version(),
                interval.deviation
            ));
        }
        if let (Some(rule), Some(series)) = (&curtailment_rule, &inputs.curtailed_mw) {
            let curtailed = series[t];
            interval.curtailment = config.curtailment.coefficient_per_mwh * curtailed * dt_h;
            notes.push(format!(
                "curtailment rule {} v{}: curtailed {curtailed} MW -> C_curtailment = {}",
                rule.name(),
                rule.version(),
                interval.curtailment
            ));
        }
        if let (Some(rule), Some(series)) = (&service_rule, &inputs.service_mw) {
            let service = series[t];
            interval.service = config.service.coefficient_per_mwh * service * dt_h;
            notes.push(format!(
                "service rule {} v{}: delivered {service} MW -> C_service = {} (sign declared by the coefficient)",
                rule.name(),
                rule.version(),
                interval.service
            ));
        }
        if notes.is_empty() {
            notes.push("no interval-decomposable terms enabled; this interval contributes 0.0".to_string());
        }
        interval.diagnostics = notes.join("; ");
        totals.energy += interval.energy;
        totals.deviation += interval.deviation;
        totals.curtailment += interval.curtailment;
        totals.service += interval.service;
        per_interval.push(interval);
    }

    // --- Degradation (P3.06 totals; not interval-decomposable — recorded,
    // not silently split).
    let mut degradation_note = None;
    if config.degradation.enabled {
        let mut configured_assets = 0usize;
        let mut raw_cost = 0.0;
        for input in &inputs.degradation {
            raw_cost += input.metadata.cost;
            if input.metadata.configured {
                configured_assets += 1;
            }
        }
        totals.degradation = config.degradation.weight * raw_cost;
        degradation_note = Some(format!(
            "degradation term uses the P3.06 linear-throughput PLACEHOLDER over {} asset(s) \
             ({} with the term configured); raw placeholder cost = {raw_cost} currency-units, \
             weighted total = {}. {}",
            inputs.degradation.len(),
            configured_assets,
            totals.degradation,
            DEGRADATION_PLACEHOLDER_STATEMENT
        ));
    }

    // --- Term provenance (ledger P3.15 step 1).
    let mut term_provenance = Vec::new();
    if let Some(curve) = curve {
        term_provenance.push(TermProvenanceRecord {
            term: ObjectiveTerm::Energy,
            source: "synthetic price curve".to_string(),
            version: curve.version().to_string(),
            note: SYNTHETIC_CURVE_ASSUMPTION_NOTE.to_string(),
        });
    }
    if config.degradation.enabled {
        for input in &inputs.degradation {
            term_provenance.push(TermProvenanceRecord {
                term: ObjectiveTerm::Degradation,
                source: "P3.06 BESS linear-throughput degradation placeholder".to_string(),
                version: "p3.06-placeholder".to_string(),
                note: format!(
                    "asset {}: configured = {}, is_placeholder = {}, rate = {} per MWh \
                     throughput; {}",
                    input.asset_id,
                    input.metadata.configured,
                    input.metadata.is_placeholder,
                    input.metadata.linear_cost_per_mwh_throughput,
                    input.metadata.placeholder_statement
                ),
            });
        }
    }
    if let Some(reference) = reference {
        term_provenance.push(TermProvenanceRecord {
            term: ObjectiveTerm::Deviation,
            source: "synthetic reference trajectory".to_string(),
            version: reference.version().to_string(),
            note: "ASSUMPTION: synthetic reference trajectory recorded as configuration; \
                   not a market commitment (the \u{a7}12.2 commitment-tracking term is the \
                   pcc_tracking module's separately recorded term)."
                .to_string(),
        });
    }
    if let Some(rule) = &curtailment_rule {
        term_provenance.push(TermProvenanceRecord {
            term: ObjectiveTerm::Curtailment,
            source: format!("curtailment rule '{}'", rule.name()),
            version: rule.version().to_string(),
            note: "named, versioned curtailment rule; the per-interval curtailed MW are \
                   derived under this rule, not from an untracked constant."
                .to_string(),
        });
    }
    if let Some(rule) = &service_rule {
        term_provenance.push(TermProvenanceRecord {
            term: ObjectiveTerm::Service,
            source: format!("service rule '{}'", rule.name()),
            version: rule.version().to_string(),
            note: "named, versioned service rule; the delivered service MW are measured \
                   against this rule, not from an untracked constant."
                .to_string(),
        });
    }

    // --- Dominance check (ledger P3.15 step 2).
    let scaling_warnings = dominance_warnings(
        &totals,
        &config,
        &[ObjectiveTerm::Energy, ObjectiveTerm::Deviation, ObjectiveTerm::Curtailment, ObjectiveTerm::Service],
    );

    Ok(ObjectiveEvaluation {
        objective_name: config.name().to_string(),
        schema_version: config.schema_version(),
        provenance: config.provenance(),
        interval_count: n,
        interval_seconds: inputs.interval_seconds,
        totals,
        total: totals.objective_total(),
        per_interval,
        term_provenance,
        scaling_warnings,
        degradation_note,
    })
}

/// The dominance rule (recorded in the module header): for each enabled
/// term with a nonzero total, if its absolute total reaches
/// `threshold ×` the largest absolute total among the OTHER enabled nonzero
/// terms, a warning is recorded. With fewer than two nonzero enabled terms
/// there is nothing to dominate — no warning (recorded rule).
///
/// `interval_terms` lists the terms whose totals were computed per interval
/// in [`evaluate_objective`]; the degradation total is included
/// separately when enabled (it is a schedule total from the P3.06
/// metadata).
fn dominance_warnings(
    totals: &TermTotals,
    config: &ObjectiveConfig,
    interval_terms: &[ObjectiveTerm],
) -> Vec<ScalingWarning> {
    let mut enabled: Vec<ObjectiveTerm> = interval_terms.to_vec();
    if config.degradation.enabled {
        enabled.push(ObjectiveTerm::Degradation);
    }
    let nonzero: Vec<(ObjectiveTerm, f64)> = enabled
        .iter()
        .map(|t| (*t, totals.get(*t).abs()))
        .filter(|(_, v)| *v > 0.0)
        .collect();
    let mut warnings = Vec::new();
    for (term, magnitude) in &nonzero {
        let others: Vec<(ObjectiveTerm, f64)> =
            nonzero.iter().copied().filter(|(t, _)| t != term).collect();
        let Some((other_term, other_magnitude)) =
            others.iter().copied().max_by(|a, b| a.1.total_cmp(&b.1))
        else {
            continue; // nothing to dominate against (recorded rule)
        };
        let ratio = magnitude / other_magnitude;
        if ratio >= config.dominance_threshold() {
            warnings.push(ScalingWarning {
                term: *term,
                term_total: totals.get(*term),
                other_term,
                other_term_total: totals.get(other_term),
                ratio,
                threshold: config.dominance_threshold(),
                review_required: true,
                statement: SCALING_REVIEW_STATEMENT,
            });
        }
    }
    warnings
}

/// The outcome of a pure argmin over an explicitly supplied candidate set:
/// the chosen index and every candidate's evaluation (so the selection is
/// always inspectable). This is a fixture/test seam and a formulation
/// input — NOT a solver and not an optimality claim beyond the enumerated
/// candidates.
#[derive(Debug, Clone, PartialEq)]
pub struct SelectionOutcome {
    /// The index into the candidate slice of the minimising candidate.
    pub chosen_index: usize,
    /// The label of the chosen candidate.
    pub chosen_label: String,
    /// Every candidate's evaluation, in candidate order.
    pub evaluations: Vec<ObjectiveEvaluation>,
}

/// Selects the minimising candidate over an explicitly supplied set (pure
/// argmin; see the module header for the scope statement). All candidates
/// must cover the same interval count and every enabled term input must be
/// valid for every candidate.
pub fn select_minimising_candidate(
    config: &ObjectiveConfig,
    candidates: &[CandidatePortfolioSchedule],
    inputs: &ObjectiveInputs,
) -> Result<SelectionOutcome, ObjectiveError> {
    if candidates.is_empty() {
        return Err(ObjectiveError::EmptyCandidateSet);
    }
    let expected = candidates[0].net_export_mw.len();
    for candidate in candidates {
        if candidate.net_export_mw.len() != expected {
            return Err(ObjectiveError::CandidateSetInconsistent {
                label: candidate.label.clone(),
                expected,
                found: candidate.net_export_mw.len(),
            });
        }
    }
    let mut evaluations = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        evaluations.push(evaluate_objective(config, candidate, inputs)?);
    }
    let mut chosen_index = 0;
    for (i, evaluation) in evaluations.iter().enumerate() {
        if evaluation.total < evaluations[chosen_index].total {
            chosen_index = i;
        }
    }
    Ok(SelectionOutcome {
        chosen_index,
        chosen_label: candidates[chosen_index].label.clone(),
        evaluations,
    })
}
