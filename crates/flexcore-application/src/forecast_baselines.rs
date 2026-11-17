// P15.02 — The load forecasting service design: statistical baselines
//         first (persistence, seasonal-naive, degree-day regression) as
//         deterministic, registry-frozen references.
//
// Phase 15 plan §2 row P15.02; decision record:
// `docs/phases-11-20/phase-15-forecasting-asset-intelligence/records/
// 02_p15_02_load_forecasting_baselines.md`.
//
// WHAT THIS IS: the three statistical load baselines the plan names, each
// a FULLY SPECIFIED deterministic rule set frozen BEFORE any evaluation
// (the P8.06 discipline applied to forecasting: no moving the baseline
// after seeing numbers), plus the frozen baseline registry that pins their
// versions.
//
// THE HONESTY LINES (read before using any baseline output):
//
// 1. **EXPLICIT NO-SKILL CLAIMS.** No baseline here claims skill,
//    accuracy, or fitness for any market or settlement purpose. Skill is
//    a property an evaluation run PROVES against the persistence
//    reference on a frozen eval set (P15.05) — until such a run fills a
//    result cell, the standing claim is "no skill until proven otherwise"
//    (the program's recorded default). No metric, error, or score exists
//    in this module; the evaluation framework owns every number, and its
//    cells start EMPTY.
// 2. **SUPERSESSION OF THE P3.10 SYNTHETIC BASELINE, ONLY BY EVALUATION.**
//    The P3.10 flexible-load model's scenario-declared baseline
//    (`SYNTHETIC_BASELINE_DISCLAIMER`, mirrored below by citation) stays
//    the operative labeling until a statistical baseline is EVALUATED on
//    a frozen eval set; supersession is a recorded registry-version event
//    (see [`BASELINE_SUPERSESSION_RULE`]) — never a silent relabel. The
//    fixture (P3.31) remains the labeled synthetic fallback source
//    (P15.16's staleness fallback).
// 3. **DETERMINISM.** Every baseline is a pure function of its declared
//    inputs: no wall clock, no randomness, no float formatting tricks —
//    the same inputs produce the same series on every call (the replay
//    requirement, P15.06).
// 4. **CANONICAL UNITS AND SIGN.** All series values are MW (§10.2);
//    `P > 0` = export at the point of connection. The baselines are
//    SIGN-PRESERVING: persistence and seasonal-naive copy the declared
//    history values verbatim; the degree-day fit is fit in the units and
//    sign the training data carries. The CALLER declares the sign
//    convention of its series; these functions never flip or normalize
//    signs (the P14.07 declared-convention posture, mirrored).
// 5. **THE REGISTRY FREEZE** (P8.06 step 3 mirrored): a baseline change
//    is a NEW version, never a silent edit; freezing computes a digest
//    over the record's canonical description (FNV-1a 64, test-grade);
//    every record carries written assumptions/limits so a "win" against
//    a weak baseline is honestly visible.
//
// Control status: INPUTS for the simulation-only 0.x line (ADR-010
// extension, conventions §6.3). Nothing here authorizes, issues, or
// dispatches anything; a baseline output is an input to scheduling
// analytics, never a command. No safety, certification, or skill claim
// is made.

use serde::Serialize;

use crate::forecast_architecture::{
    ForecastArtifactKind, ForecastHorizon, ForecastScope,
};

/// The registry version: the version of the FORECAST-BASELINE SET as a
/// whole (adding a baseline is a registry version bump; editing one is a
/// record version bump — never silent).
pub const FORECAST_BASELINE_REGISTRY_VERSION: &str = "flexcore-application/forecast-baselines.v1";

/// The recorded no-skill statement (pinned by tests; the honesty line the
/// whole phase inherits).
pub const NO_SKILL_STATEMENT: &str = "NO baseline in the forecast-baseline registry claims \
     skill, accuracy, or fitness for any market or settlement purpose. Skill exists only as \
     a result of an X-8-gated evaluation run against the persistence reference on a frozen \
     eval set (P15.05); every evaluation cell is EMPTY until such a run. Until then the \
     standing claim for every model — statistical or ML — is: no skill, unproven, \
     UNTRAINED where training is involved.";

/// The supersession rule for the P3.10 synthetic baseline (pinned by
/// tests; see the module header).
pub const BASELINE_SUPERSESSION_RULE: &str = "the P3.10 flexible-load SYNTHETIC_BASELINE_\
     DISCLAIMER remains the operative baseline labeling until a statistical baseline is \
     EVALUATED on a frozen eval set (P15.05). Supersession is a recorded event: a NEW \
     forecast-baseline registry version whose record names the eval-set version and the \
     evaluation record that justified the supersession — never a silent relabel, never an \
     in-place edit of the P3.10 disclaimer. The P3.31 synthetic fixture remains the \
     labeled synthetic fallback source (P15.16 staleness fallback).";

/// The P3.10 disclaimer, mirrored BY CITATION (the domain constant is the
/// single home; this mirror is pinned to it by a test that asserts this
/// constant equals the domain constant verbatim).
pub const P3_10_SYNTHETIC_BASELINE_DISCLAIMER_MIRROR: &str =
    flexcore_domain::flexible_load_model::SYNTHETIC_BASELINE_DISCLAIMER;

// --- The frozen baseline registry (P8.06 discipline, mirrored) ------------------------

/// The kind of forecast baseline (the identity of the rule set).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ForecastBaselineKind {
    /// `next(t) = obs(t - 1)`: the last observed value carried forward for
    /// every horizon step (the persistence reference every other model
    /// must beat to claim skill).
    Persistence,
    /// `next(t) = obs(t - s)`: the value one seasonal period earlier
    /// (daily/weekly seasonality reference).
    SeasonalNaive,
    /// Ordinary least squares of load on heating/cooling degree days — a
    /// statistical, auditable weather-sensitive baseline.
    DegreeDayRegression,
}

/// One forecast-baseline definition: the frozen identity a forecaster is
/// compared against. Fields mirror the P8.06 `BaselineRecord` shape BY
/// CITATION (version, written assumptions/limits, canonical string,
/// freeze digest, version bump).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ForecastBaselineRecord {
    /// The stable baseline id (e.g. `forecast-baseline/persistence`).
    pub baseline_id: &'static str,
    /// The baseline version (a change is a NEW version, never a silent
    /// edit). Starts at `v1`.
    pub version: String,
    /// The rule-set identity.
    pub kind: ForecastBaselineKind,
    /// The forecast artifact kinds the baseline produces (closed
    /// vocabulary; the load baselines produce load artifacts; persistence
    /// and seasonal-naive are ALSO the PV references — recorded per
    /// record).
    pub artifact_kinds: Vec<ForecastArtifactKind>,
    /// The horizons the baseline is declared for (closed vocabulary).
    pub horizons: Vec<ForecastHorizon>,
    /// Assumptions and limits (P8.06 step 4 — what the baseline does NOT
    /// do, so a "win" against it is honestly visible). A record without
    /// written assumptions cannot be frozen.
    pub assumptions_and_limits: Vec<&'static str>,
}

impl ForecastBaselineRecord {
    /// The canonical digest input: stable field order, `|` separators —
    /// the deterministic string the frozen digest is computed over.
    pub fn canonical_string(&self) -> String {
        format!(
            "id:{}|version:{}|kind:{:?}|artifacts:{}|horizons:{}|assumptions:{}",
            self.baseline_id,
            self.version,
            self.kind,
            self.artifact_kinds
                .iter()
                .map(|k| k.wire_name())
                .collect::<Vec<_>>()
                .join(","),
            self.horizons
                .iter()
                .map(|h| h.wire_name())
                .collect::<Vec<_>>()
                .join(","),
            self.assumptions_and_limits.join(";"),
        )
    }

    /// The next version of this record (`v1` → `v2`, …): the ONLY legal
    /// way a baseline changes. A bump is a NEW record; the old frozen
    /// artifact stays frozen.
    pub fn bump_version(&self) -> ForecastBaselineRecord {
        let mut bumped = self.clone();
        bumped.version = next_version(&self.version);
        bumped
    }
}

/// `v<N>` → `v<N+1>` (unparseable versions get a `v2` suffix appended —
/// never silently reused).
fn next_version(version: &str) -> String {
    if let Some(rest) = version.strip_prefix('v') {
        if let Ok(n) = rest.parse::<u64>() {
            return format!("v{}", n + 1);
        }
    }
    format!("{version}.v2")
}

/// The baseline ids (stable).
pub const FORECAST_BASELINE_ID_PERSISTENCE: &str = "forecast-baseline/persistence";
pub const FORECAST_BASELINE_ID_SEASONAL_NAIVE: &str = "forecast-baseline/seasonal-naive";
pub const FORECAST_BASELINE_ID_DEGREE_DAY: &str = "forecast-baseline/degree-day-regression";

/// The full frozen registry: the baseline set every load/PV forecaster is
/// compared against. Order is fixed (registry order is part of the frozen
/// artifact). Every record names persistence as its reference baseline
/// (skill = beating persistence on a frozen eval set, P15.05).
pub fn forecast_baseline_registry() -> Vec<ForecastBaselineRecord> {
    vec![
        ForecastBaselineRecord {
            baseline_id: FORECAST_BASELINE_ID_PERSISTENCE,
            version: "v1".to_string(),
            kind: ForecastBaselineKind::Persistence,
            artifact_kinds: vec![
                ForecastArtifactKind::Load,
                ForecastArtifactKind::PvGeneration,
            ],
            horizons: vec![ForecastHorizon::DayAhead, ForecastHorizon::IntraDay],
            assumptions_and_limits: vec![
                "rule set (persistence.v1): forecast(t) = last observed value, carried \
                 forward unchanged for every horizon step; sign- and unit-preserving",
                "no seasonality, no weather sensitivity, no trend handling: a ramp, a \
                 daily cycle, or a weather event is invisible to this baseline by \
                 construction",
                "the REFERENCE baseline: skill = beating this on a frozen eval set \
                 (P15.05); until an evaluation run exists, NO model — this one included — \
                 claims skill",
                "requires at least one finite history value; an empty history is a named \
                 refusal, never a zero-fill",
            ],
        },
        ForecastBaselineRecord {
            baseline_id: FORECAST_BASELINE_ID_SEASONAL_NAIVE,
            version: "v1".to_string(),
            kind: ForecastBaselineKind::SeasonalNaive,
            artifact_kinds: vec![
                ForecastArtifactKind::Load,
                ForecastArtifactKind::PvGeneration,
            ],
            horizons: vec![ForecastHorizon::DayAhead, ForecastHorizon::IntraDay],
            assumptions_and_limits: vec![
                "rule set (seasonal-naive.v1): forecast(t) = observed value exactly one \
                 seasonal period earlier, for as many periods as the horizon spans; \
                 sign- and unit-preserving",
                "requires the history to span at least one full seasonal period plus one \
                 step; a shorter history is a named refusal, never a partial-period guess",
                "the season length is a DECLARED input (e.g. a day in steps); the rule \
                 never infers seasonality from the data",
                "the SECOND reference baseline (daily/weekly seasonality); no skill claim \
                 is made anywhere in this registry",
            ],
        },
        ForecastBaselineRecord {
            baseline_id: FORECAST_BASELINE_ID_DEGREE_DAY,
            version: "v1".to_string(),
            kind: ForecastBaselineKind::DegreeDayRegression,
            artifact_kinds: vec![ForecastArtifactKind::Load],
            horizons: vec![ForecastHorizon::DayAhead],
            assumptions_and_limits: vec![
                "rule set (degree-day.v1): ordinary least squares of load on (intercept, \
                 heating degree days, cooling degree days), fit by deterministic Gaussian \
                 elimination with first-max pivoting; prediction is linear in the \
                 declared forecast degree days",
                "degree days are DECLARED inputs (base temperature, per-step HDD/CDD \
                 values >= 0); the rule never infers a base temperature from the data",
                "requires at least three finite training samples and a non-singular \
                 design matrix; a singular matrix (e.g. collinear regressors) is a named \
                 refusal, never a pseudo-inverse",
                "load-only (PV is not degree-day-driven in this rule set); the fit is in \
                 the units and sign the training data carries — no sign normalization",
                "a linear weather model: no humidity, no wind, no nonlinearity; the \
                 weather input seam is P15.03 (P15-E-1 OPEN)",
                "no skill claim: skill requires an evaluation run against the persistence \
                 reference on a frozen eval set (P15.05)",
            ],
        },
    ]
}

/// A frozen forecast baseline: the record plus the digest over its
/// canonical description (FNV-1a 64 — the repo's test-grade checksum,
/// explicitly NOT a security primitive). Frozen BEFORE any evaluation
/// result is compared to it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FrozenForecastBaseline {
    /// The frozen record.
    pub record: ForecastBaselineRecord,
    /// The digest over the record's canonical description.
    pub definition_digest: String,
    /// The registry version the freeze happened under (provenance).
    pub registry_version: String,
}

/// Why a freeze was refused or a reproduction check failed. Named, never
/// absorbed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForecastFreezeError {
    /// The record's assumptions/limits list is empty — a baseline without
    /// written assumptions cannot be frozen (P8.06 step 4).
    AssumptionsMissing { baseline_id: String },
    /// A record's artifact-kind or horizon list is empty (a baseline
    /// declared for nothing cannot be frozen).
    DeclarationEmpty { baseline_id: String },
    /// The observed digest does not match the frozen one.
    DigestMismatch { expected: String, observed: String },
}

impl std::fmt::Display for ForecastFreezeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ForecastFreezeError::AssumptionsMissing { baseline_id } => write!(
                f,
                "baseline {baseline_id} carries no assumptions/limits record; it cannot be \
                 frozen until its assumptions are written down (P8.06 step 4)"
            ),
            ForecastFreezeError::DeclarationEmpty { baseline_id } => write!(
                f,
                "baseline {baseline_id} declares no artifact kinds or no horizons; it \
                 cannot be frozen until it declares what it produces and for what horizons"
            ),
            ForecastFreezeError::DigestMismatch { expected, observed } => write!(
                f,
                "the re-run digest {observed} does not match the frozen digest {expected}; \
                 the deviation is named, never absorbed"
            ),
        }
    }
}

impl std::error::Error for ForecastFreezeError {}

impl FrozenForecastBaseline {
    /// Freezes a record's DEFINITION (assumptions must be present and the
    /// declaration non-empty).
    pub fn freeze(
        record: &ForecastBaselineRecord,
    ) -> Result<FrozenForecastBaseline, ForecastFreezeError> {
        if record.assumptions_and_limits.is_empty() {
            return Err(ForecastFreezeError::AssumptionsMissing {
                baseline_id: record.baseline_id.to_string(),
            });
        }
        if record.artifact_kinds.is_empty() || record.horizons.is_empty() {
            return Err(ForecastFreezeError::DeclarationEmpty {
                baseline_id: record.baseline_id.to_string(),
            });
        }
        Ok(FrozenForecastBaseline {
            definition_digest: format!("{:016x}", fnv1a64(record.canonical_string().as_bytes())),
            record: record.clone(),
            registry_version: FORECAST_BASELINE_REGISTRY_VERSION,
        })
    }

    /// The reproduction check: a re-derived digest must match the frozen
    /// one or the deviation is NAMED, never absorbed.
    pub fn confirm_reproduction(&self, observed_digest: &str) -> Result<(), ForecastFreezeError> {
        if self.definition_digest == observed_digest {
            Ok(())
        } else {
            Err(ForecastFreezeError::DigestMismatch {
                expected: self.definition_digest.clone(),
                observed: observed_digest.to_string(),
            })
        }
    }
}

// ---------------------------------------------------------------------------
// Baseline rule sets (fully specified, deterministic)
// ---------------------------------------------------------------------------

/// Why a baseline input was refused. Every variant is a named error;
/// refusals never degrade into a zero-fill or a partial answer.
#[derive(Debug, Clone, PartialEq)]
pub enum ForecastBaselineError {
    /// A series value was NaN or infinite.
    NonFiniteValue { index: usize },
    /// The persistence rule needs at least one observed value.
    EmptyHistory,
    /// The seasonal-naive rule needs at least `season_length + 1`
    /// observed values.
    HistoryShorterThanSeason { season_length: usize, history_len: usize },
    /// The declared season length is zero (a season of zero steps is a
    /// formulation error, not a persistence alias).
    ZeroSeasonLength,
    /// The horizon asked for zero steps (a forecast of nothing is a
    /// formulation error).
    ZeroHorizonSteps,
    /// A degree-day value was negative (degree days are counts above a
    /// base temperature; a negative value is a formulation error).
    NegativeDegreeDay { index: usize, value: f64 },
    /// The degree-day training set has fewer than three samples.
    InsufficientTrainingSamples { found: usize },
    /// A training load value was non-finite.
    NonFiniteTrainingLoad { index: usize },
    /// The design matrix is singular (collinear regressors or zero
    /// variance); the rule refuses rather than pseudo-inverting.
    SingularDesignMatrix { reason: String },
    /// A forecast degree-day input was non-finite.
    NonFiniteForecastDegreeDay { index: usize },
    /// The HDD and CDD forecast vectors had different lengths.
    HorizonLengthMismatch { hdd_len: usize, cdd_len: usize },
}

impl std::fmt::Display for ForecastBaselineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ForecastBaselineError::NonFiniteValue { index } => {
                write!(f, "history value at index {index} is not finite")
            }
            ForecastBaselineError::EmptyHistory => write!(
                f, "persistence requires at least one observed value; an empty history is \
                    refused, never zero-filled"
            ),
            ForecastBaselineError::HistoryShorterThanSeason {
                season_length,
                history_len,
            } => write!(
                f,
                "seasonal-naive requires a history of at least season_length + 1 = {} \
                 values; got {history_len}",
                season_length + 1
            ),
            ForecastBaselineError::ZeroSeasonLength => {
                write!(f, "the season length must be at least 1 step")
            }
            ForecastBaselineError::ZeroHorizonSteps => {
                write!(f, "the horizon must ask for at least 1 step")
            }
            ForecastBaselineError::NegativeDegreeDay { index, value } => write!(
                f,
                "degree-day value at index {index} is {value}; degree days are counts \
                 above a base temperature and are never negative"
            ),
            ForecastBaselineError::InsufficientTrainingSamples { found } => write!(
                f,
                "the degree-day fit requires at least 3 training samples; got {found}"
            ),
            ForecastBaselineError::NonFiniteTrainingLoad { index } => {
                write!(f, "training load at index {index} is not finite")
            }
            ForecastBaselineError::SingularDesignMatrix { reason } => write!(
                f,
                "the degree-day design matrix is singular ({reason}); the rule refuses, \
                 never pseudo-inverts"
            ),
            ForecastBaselineError::NonFiniteForecastDegreeDay { index } => {
                write!(f, "forecast degree-day value at index {index} is not finite")
            }
            ForecastBaselineError::HorizonLengthMismatch { hdd_len, cdd_len } => write!(
                f,
                "the forecast HDD vector ({hdd_len} values) and CDD vector ({cdd_len}                  values) must have the same length"
            ),
        }
    }
}

impl std::error::Error for ForecastBaselineError {}

/// Validates a history series (finite values; sign convention is the
/// caller's declaration — never normalized here).
fn validate_history(history: &[f64]) -> Result<(), ForecastBaselineError> {
    for (index, value) in history.iter().enumerate() {
        if !value.is_finite() {
            return Err(ForecastBaselineError::NonFiniteValue { index });
        }
    }
    Ok(())
}

/// THE PERSISTENCE RULE (persistence.v1): `forecast(t) = last observed
/// value`, carried forward unchanged for every horizon step. Deterministic,
/// sign- and unit-preserving.
pub fn persistence_forecast(
    history: &[f64],
    horizon_steps: usize,
) -> Result<Vec<f64>, ForecastBaselineError> {
    if horizon_steps == 0 {
        return Err(ForecastBaselineError::ZeroHorizonSteps);
    }
    validate_history(history)?;
    let last = *history.last().ok_or(ForecastBaselineError::EmptyHistory)?;
    Ok(vec![last; horizon_steps])
}

/// THE SEASONAL-NAIVE RULE (seasonal-naive.v1): `forecast(t) = observed
/// value exactly one seasonal period earlier`. The season length is a
/// DECLARED input in steps (e.g. 96 for a day at 15-minute cadence); the
/// rule never infers seasonality. Deterministic, sign- and
/// unit-preserving.
pub fn seasonal_naive_forecast(
    history: &[f64],
    season_length: usize,
    horizon_steps: usize,
) -> Result<Vec<f64>, ForecastBaselineError> {
    if horizon_steps == 0 {
        return Err(ForecastBaselineError::ZeroHorizonSteps);
    }
    if season_length == 0 {
        return Err(ForecastBaselineError::ZeroSeasonLength);
    }
    validate_history(history)?;
    if history.len() < season_length + 1 {
        return Err(ForecastBaselineError::HistoryShorterThanSeason {
            season_length,
            history_len: history.len(),
        });
    }
    let n = history.len();
    Ok((0..horizon_steps)
        .map(|step| {
            // The value exactly one season earlier than the step being
            // forecast: history index n - season_length + step, which
            // exists while step < season_length; for a horizon longer
            // than one season, the rule wraps to the same seasonal slot
            // of the LAST season in the history (declared wrap rule).
            let index = n - season_length + (step % season_length);
            history[index]
        })
        .collect())
}

/// One degree-day training pair: the load value and its declared heating
/// and cooling degree days for the same interval.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DegreeDaySample {
    /// The load value, MW, in the caller's declared sign convention.
    pub load_mw: f64,
    /// Heating degree days for the interval (>= 0).
    pub heating_degree_days: f64,
    /// Cooling degree days for the interval (>= 0).
    pub cooling_degree_days: f64,
}

/// The fitted degree-day regression: the three coefficients of
/// `load ≈ intercept + hdd_coef × HDD + cdd_coef × CDD`, plus the
/// baseline version the fit declares (the frozen-version discipline:
/// the result names its rule set).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DegreeDayFit {
    /// The intercept, MW (in the training data's sign convention).
    pub intercept_mw: f64,
    /// The heating-degree-day coefficient, MW per degree day.
    pub hdd_coef_mw_per_dd: f64,
    /// The cooling-degree-day coefficient, MW per degree day.
    pub cdd_coef_mw_per_dd: f64,
    /// The number of training samples the fit used.
    pub training_samples: usize,
    /// The baseline version that produced this fit.
    pub baseline_version: String,
}

/// THE DEGREE-DAY RULE (degree-day.v1), fit step: ordinary least squares
/// of load on (1, HDD, CDD) via the normal equations, solved by
/// deterministic Gaussian elimination with partial pivoting (first-max
/// tie-break). Requires >= 3 finite samples and a non-singular design
/// matrix. Deterministic: no randomness, no wall clock.
pub fn fit_degree_day_regression(
    samples: &[DegreeDaySample],
) -> Result<DegreeDayFit, ForecastBaselineError> {
    if samples.len() < 3 {
        return Err(ForecastBaselineError::InsufficientTrainingSamples {
            found: samples.len(),
        });
    }
    for (index, sample) in samples.iter().enumerate() {
        if !sample.load_mw.is_finite() {
            return Err(ForecastBaselineError::NonFiniteTrainingLoad { index });
        }
        if sample.heating_degree_days < 0.0 {
            return Err(ForecastBaselineError::NegativeDegreeDay {
                index,
                value: sample.heating_degree_days,
            });
        }
        if sample.cooling_degree_days < 0.0 {
            return Err(ForecastBaselineError::NegativeDegreeDay {
                index,
                value: sample.cooling_degree_days,
            });
        }
    }

    // The normal equations: A^T A c = A^T y for A = [1, hdd, cdd].
    // Accumulated in fixed sample order (deterministic summation order).
    let mut m = [[0.0f64; 3]; 3];
    let mut rhs = [0.0f64; 3];
    for sample in samples {
        let row = [1.0, sample.heating_degree_days, sample.cooling_degree_days];
        for (i, a) in row.iter().enumerate() {
            for (j, b) in row.iter().enumerate() {
                m[i][j] += a * b;
            }
            rhs[i] += a * sample.load_mw;
        }
    }

    let coefficients = solve_3x3_gaussian_first_max_pivot(&m, &rhs)?;

    Ok(DegreeDayFit {
        intercept_mw: coefficients[0],
        hdd_coef_mw_per_dd: coefficients[1],
        cdd_coef_mw_per_dd: coefficients[2],
        training_samples: samples.len(),
        baseline_version: "degree-day.v1".to_string(),
    })
}

/// THE DEGREE-DAY RULE (degree-day.v1), predict step: linear prediction
/// over the declared forecast degree days, in the training data's units
/// and sign convention. The forecast degree days come from the declared
/// weather input (P15.03 seam / P15-E-1); the rule never invents weather.
pub fn degree_day_predict(
    fit: &DegreeDayFit,
    forecast_hdd: &[f64],
    forecast_cdd: &[f64],
) -> Result<Vec<f64>, ForecastBaselineError> {
    if forecast_hdd.len() != forecast_cdd.len() {
        return Err(ForecastBaselineError::HorizonLengthMismatch {
            hdd_len: forecast_hdd.len(),
            cdd_len: forecast_cdd.len(),
        });
    }
    if forecast_hdd.is_empty() {
        return Err(ForecastBaselineError::ZeroHorizonSteps);
    }
    for (index, value) in forecast_hdd.iter().enumerate() {
        if !value.is_finite() {
            return Err(ForecastBaselineError::NonFiniteForecastDegreeDay { index });
        }
        if *value < 0.0 {
            return Err(ForecastBaselineError::NegativeDegreeDay { index, value: *value });
        }
    }
    for (index, value) in forecast_cdd.iter().enumerate() {
        if !value.is_finite() {
            return Err(ForecastBaselineError::NonFiniteForecastDegreeDay {
                index: forecast_hdd.len() + index,
            });
        }
        if *value < 0.0 {
            return Err(ForecastBaselineError::NegativeDegreeDay { index, value: *value });
        }
    }
    Ok(forecast_hdd
        .iter()
        .zip(forecast_cdd.iter())
        .map(|(hdd, cdd)| {
            fit.intercept_mw + fit.hdd_coef_mw_per_dd * hdd + fit.cdd_coef_mw_per_dd * cdd
        })
        .collect())
}

/// Solves a 3x3 linear system by Gaussian elimination with partial
/// pivoting (first-max tie-break for determinism). Refuses a singular
/// matrix with a named reason (pivot below 1e-12 relative to the column's
/// max magnitude); never pseudo-inverts.
fn solve_3x3_gaussian_first_max_pivot(
    m: &[[f64; 3]; 3],
    rhs: &[f64; 3],
) -> Result<[f64; 3], ForecastBaselineError> {
    let mut a = *m;
    let mut b = *rhs;

    for col in 0..3 {
        // Partial pivot: the row (from `col` down) with the largest
        // absolute value in this column; ties resolve to the FIRST max
        // (deterministic).
        let mut pivot_row = col;
        let mut pivot_mag = a[col][col].abs();
        for row in (col + 1)..3 {
            if a[row][col].abs() > pivot_mag {
                pivot_row = row;
                pivot_mag = a[row][col].abs();
            }
        }
        // Singularity: a pivot magnitude that is zero to within a fixed
        // epsilon relative to nothing (the matrix entries themselves) is
        // refused. The epsilon is part of the frozen rule set.
        const SINGULAR_EPSILON: f64 = 1e-12;
        if pivot_mag < SINGULAR_EPSILON {
            return Err(ForecastBaselineError::SingularDesignMatrix {
                reason: format!(
                    "column {col} pivot magnitude {pivot_mag} is below the frozen epsilon \
                     {SINGULAR_EPSILON} (collinear regressors or zero variance)"
                ),
            });
        }
        a.swap(col, pivot_row);
        b.swap(col, pivot_row);

        // Eliminate below the pivot.
        for row in (col + 1)..3 {
            let factor = a[row][col] / a[col][col];
            for j in col..3 {
                a[row][j] -= factor * a[col][j];
            }
            b[row] -= factor * b[col];
        }
    }

    // Back-substitution.
    let mut x = [0.0f64; 3];
    for i in (0..3).rev() {
        let mut sum = b[i];
        for j in (i + 1)..3 {
            sum -= a[i][j] * x[j];
        }
        x[i] = sum / a[i][i];
    }
    Ok(x)
}

// --- The FNV-1a 64 digest (the repo's test-grade checksum pattern) -------

/// The FNV-1a 64-bit hash (deterministic, dependency-free, explicitly NOT
/// a security primitive).
fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

// ---------------------------------------------------------------------------
// Tests (WRITTEN-UNRUN — no cargo command was executed in this loop; the
// human build/test gate owns the run. Every expected value below is
// hand-computed from the rule sets above, a published FNV-1a 64 test
// vector, or a literal copied from this module's own constants. Nothing
// is fabricated.)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fnv1a64_matches_published_test_vectors() {
        // Published FNV-1a 64 test vectors (the same pins the P8.06
        // baseline registry uses): empty input = offset basis, "a",
        // "foobar".
        assert_eq!(fnv1a64(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a64(b"a"), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(fnv1a64(b"foobar"), 0x8594_4171_f739_67e8);
    }

    #[test]
    fn registry_constants_are_the_recorded_strings() {
        assert_eq!(
            FORECAST_BASELINE_REGISTRY_VERSION,
            "flexcore-application/forecast-baselines.v1"
        );
        assert!(NO_SKILL_STATEMENT.contains("no skill, unproven, UNTRAINED"));
        assert!(BASELINE_SUPERSESSION_RULE.contains("never a silent relabel"));
        assert!(BASELINE_SUPERSESSION_RULE.contains("P15.05"));
    }

    #[test]
    fn the_p3_10_disclaimer_mirror_is_verbatim() {
        // The mirror is BY CITATION and pinned to the domain constant
        // verbatim: the domain is the single home.
        assert_eq!(
            P3_10_SYNTHETIC_BASELINE_DISCLAIMER_MIRROR,
            flexcore_domain::flexible_load_model::SYNTHETIC_BASELINE_DISCLAIMER
        );
        assert!(P3_10_SYNTHETIC_BASELINE_DISCLAIMER_MIRROR.contains("SYNTHETIC baseline"));
    }

    #[test]
    fn registry_has_exactly_three_frozen_records() {
        let registry = forecast_baseline_registry();
        assert_eq!(registry.len(), 3);
        assert_eq!(registry[0].baseline_id, "forecast-baseline/persistence");
        assert_eq!(registry[1].baseline_id, "forecast-baseline/seasonal-naive");
        assert_eq!(registry[2].baseline_id, "forecast-baseline/degree-day-regression");
        for record in &registry {
            assert_eq!(record.version, "v1");
            assert!(!record.assumptions_and_limits.is_empty());
            assert!(!record.artifact_kinds.is_empty());
            assert!(!record.horizons.is_empty());
        }
        // Every record's assumptions name the no-skill posture.
        assert!(registry[0].assumptions_and_limits[2].contains("NO model"));
        assert!(registry[2].assumptions_and_limits[5].contains("no skill claim"));
    }

    #[test]
    fn freeze_digest_is_deterministic_and_sensitive_to_content() {
        let registry = forecast_baseline_registry();
        let frozen = FrozenForecastBaseline::freeze(&registry[0]).expect("assumptions present");
        let frozen_again =
            FrozenForecastBaseline::freeze(&registry[0]).expect("assumptions present");
        assert_eq!(frozen, frozen_again, "the same record freezes to the same digest");
        assert_eq!(frozen.registry_version, FORECAST_BASELINE_REGISTRY_VERSION);

        let mut changed = registry[0].clone();
        changed.version = "v2".to_string();
        let changed_frozen = FrozenForecastBaseline::freeze(&changed).unwrap();
        assert_ne!(frozen.definition_digest, changed_frozen.definition_digest);
    }

    #[test]
    fn freeze_refuses_a_record_without_written_assumptions_or_declaration() {
        let registry = forecast_baseline_registry();
        let mut naked = registry[0].clone();
        naked.assumptions_and_limits = Vec::new();
        let error = FrozenForecastBaseline::freeze(&naked).expect_err("no assumptions");
        assert_eq!(
            error,
            ForecastFreezeError::AssumptionsMissing {
                baseline_id: "forecast-baseline/persistence".to_string()
            }
        );
        let mut undeclared = registry[0].clone();
        undeclared.horizons = Vec::new();
        let error = FrozenForecastBaseline::freeze(&undeclared)
            .expect_err("no horizons declared");
        assert_eq!(
            error,
            ForecastFreezeError::DeclarationEmpty {
                baseline_id: "forecast-baseline/persistence".to_string()
            }
        );
    }

    #[test]
    fn version_bumps_are_the_only_legal_change() {
        let registry = forecast_baseline_registry();
        assert_eq!(registry[0].bump_version().version, "v2");
        let mut nine = registry[0].clone();
        nine.version = "v9".to_string();
        assert_eq!(nine.bump_version().version, "v10");
        let mut weird = registry[0].clone();
        weird.version = "weird".to_string();
        assert_eq!(weird.bump_version().version, "weird.v2");
    }

    #[test]
    fn reproduction_confirm_is_exact_and_mismatch_is_named() {
        let registry = forecast_baseline_registry();
        let frozen = FrozenForecastBaseline::freeze(&registry[0]).unwrap();
        frozen
            .confirm_reproduction(&frozen.definition_digest)
            .expect("the frozen digest reproduces");
        let error = frozen
            .confirm_reproduction("0000000000000000")
            .expect_err("a deviation is named, never absorbed");
        assert_eq!(
            error,
            ForecastFreezeError::DigestMismatch {
                expected: frozen.definition_digest.clone(),
                observed: "0000000000000000".to_string(),
            }
        );
    }

    // --- persistence: hand-computed known answers ---------------------------

    #[test]
    fn persistence_carries_the_last_value_forward() {
        // HAND CHECK: history [-0.4, -0.5, -0.6] MW (consumption negative);
        // last value -0.6 carried forward for 3 steps.
        let forecast =
            persistence_forecast(&[-0.4, -0.5, -0.6], 3).expect("valid history");
        assert_eq!(forecast, vec![-0.6, -0.6, -0.6]);
        // The sign convention of the caller is preserved verbatim.
        let forecast =
            persistence_forecast(&[0.0, 1.5], 1).expect("valid history");
        assert_eq!(forecast, vec![1.5]);
    }

    #[test]
    fn persistence_refuses_empty_or_non_finite_input() {
        let err = persistence_forecast(&[], 1).expect_err("empty history refused");
        assert_eq!(err, ForecastBaselineError::EmptyHistory);
        let err = persistence_forecast(&[0.5, f64::NAN], 1)
            .expect_err("NaN refused");
        assert_eq!(err, ForecastBaselineError::NonFiniteValue { index: 1 });
        let err = persistence_forecast(&[0.5], 0).expect_err("zero horizon refused");
        assert_eq!(err, ForecastBaselineError::ZeroHorizonSteps);
    }

    // --- seasonal-naive: hand-computed known answers ------------------------

    #[test]
    fn seasonal_naive_returns_the_value_one_season_earlier() {
        // HAND CHECK: history of 5 values, season_length 3, horizon 3.
        // Step 0 -> index 5-3+0 = 2 -> -0.2; step 1 -> index 3 -> -0.3;
        // step 2 -> index 4 -> -0.4.
        let forecast =
            seasonal_naive_forecast(&[-0.0, -0.1, -0.2, -0.3, -0.4], 3, 3)
                .expect("history spans one season plus one step");
        assert_eq!(forecast, vec![-0.2, -0.3, -0.4]);
    }

    #[test]
    fn seasonal_naive_wraps_within_the_last_season_for_longer_horizons() {
        // HAND CHECK: history of 4 values, season_length 2, horizon 4.
        // step 0 -> index 4-2+0 = 2 -> 30.0; step 1 -> index 3 -> 40.0;
        // step 2 wraps to (2 % 2)=0 -> index 2 -> 30.0; step 3 -> 40.0.
        let forecast = seasonal_naive_forecast(&[10.0, 20.0, 30.0, 40.0], 2, 4)
            .expect("history spans one season plus one step");
        assert_eq!(forecast, vec![30.0, 40.0, 30.0, 40.0]);
    }

    #[test]
    fn seasonal_naive_refuses_short_history_zero_season_and_non_finite() {
        let err = seasonal_naive_forecast(&[1.0, 2.0], 3, 1)
            .expect_err("history shorter than season+1 refused");
        assert_eq!(
            err,
            ForecastBaselineError::HistoryShorterThanSeason {
                season_length: 3,
                history_len: 2,
            }
        );
        let err = seasonal_naive_forecast(&[1.0, 2.0, 3.0], 0, 1)
            .expect_err("zero season refused");
        assert_eq!(err, ForecastBaselineError::ZeroSeasonLength);
        let err = seasonal_naive_forecast(&[1.0, f64::INFINITY, 3.0], 1, 1)
            .expect_err("infinite value refused");
        assert_eq!(err, ForecastBaselineError::NonFiniteValue { index: 1 });
    }

    // --- degree-day regression: hand-computed known answers -----------------

    /// A degenerate but HAND-VERIFIABLE training set: the design matrix is
    /// diagonal-dominant and the exact solution is recoverable by hand.
    /// Samples: (hdd, cdd, load) chosen so the normal equations resolve to
    /// intercept = 1.0, hdd_coef = 2.0, cdd_coef = 3.0. We construct
    /// samples with orthogonal regressors: two pure-HDD points, two
    /// pure-CDD points, and one all-zero point.
    fn orthogonal_training() -> Vec<DegreeDaySample> {
        vec![
            DegreeDaySample { load_mw: 1.0, heating_degree_days: 0.0, cooling_degree_days: 0.0 },
            DegreeDaySample { load_mw: 3.0, heating_degree_days: 1.0, cooling_degree_days: 0.0 },
            DegreeDaySample { load_mw: 1.0, heating_degree_days: 0.0, cooling_degree_days: 0.0 },
            DegreeDaySample { load_mw: 1.0, heating_degree_days: 0.0, cooling_degree_days: 0.0 },
            DegreeDaySample { load_mw: 4.0, heating_degree_days: 0.0, cooling_degree_days: 1.0 },
        ]
    }

    #[test]
    fn the_degree_day_fit_recovers_exact_coefficients_on_orthogonal_regressors() {
        // With regressors [1, hdd, cdd] where hdd and cdd are mutually
        // exclusive across samples, the normal equations decouple:
        // - intercept: mean of the three all-zero samples = 1.0 (the
        //   three zeros contribute 3 samples at load 1.0);
        //   HAND CHECK of the full 3x3 system:
        //   A^T A = [[5,1,1],[1,1,0],[1,0,1]]; A^T y = [10,3,4].
        //   Solve: from rows 2/3: hdd = 3 - intercept, cdd = 4 - intercept.
        //   Row 1: 5i + (3-i) + (4-i) = 10 -> 3i + 7 = 10 -> i = 1.0.
        //   hdd = 2.0, cdd = 3.0. EXACT.
        let fit = fit_degree_day_regression(&orthogonal_training())
            .expect("orthogonal training set is non-singular");
        assert_eq!(fit.training_samples, 5);
        assert_eq!(fit.baseline_version, "degree-day.v1");
        let close = |a: f64, b: f64| (a - b).abs() < 1e-9;
        assert!(close(fit.intercept_mw, 1.0), "intercept {}", fit.intercept_mw);
        assert!(close(fit.hdd_coef_mw_per_dd, 2.0), "hdd {}", fit.hdd_coef_mw_per_dd);
        assert!(close(fit.cdd_coef_mw_per_dd, 3.0), "cdd {}", fit.cdd_coef_mw_per_dd);
    }

    #[test]
    fn the_degree_day_predict_is_linear_in_the_declared_degree_days() {
        let fit = DegreeDayFit {
            intercept_mw: 1.0,
            hdd_coef_mw_per_dd: 2.0,
            cdd_coef_mw_per_dd: 3.0,
            training_samples: 5,
            baseline_version: "degree-day.v1".to_string(),
        };
        // HAND CHECK: (hdd, cdd) = (2, 1) -> 1 + 4 + 3 = 8.0;
        // (0, 0) -> 1.0. The sign convention of the fit carries through
        // (a consumption-negative fit would predict negative values).
        let out = degree_day_predict(&fit, &[2.0, 0.0], &[1.0, 0.0]).expect("valid inputs");
        assert_eq!(out, vec![8.0, 1.0]);
    }

    #[test]
    fn degree_day_refuses_insufficient_negative_nonfinite_and_singular_inputs() {
        let err = fit_degree_day_regression(&orthogonal_training()[..2])
            .expect_err("two samples refused");
        assert_eq!(
            err,
            ForecastBaselineError::InsufficientTrainingSamples { found: 2 }
        );
        let mut samples = orthogonal_training();
        samples[0].heating_degree_days = -1.0;
        let err = fit_degree_day_regression(&samples)
            .expect_err("negative HDD refused");
        assert_eq!(
            err,
            ForecastBaselineError::NegativeDegreeDay { index: 0, value: -1.0 }
        );
        let mut samples = orthogonal_training();
        samples[1].load_mw = f64::NAN;
        let err = fit_degree_day_regression(&samples)
            .expect_err("NaN load refused");
        assert_eq!(
            err,
            ForecastBaselineError::NonFiniteTrainingLoad { index: 1 }
        );
        // SINGULAR: all samples identical (zero variance in every column).
        let constant = vec![
            DegreeDaySample { load_mw: 1.0, heating_degree_days: 0.0, cooling_degree_days: 0.0 };
            3
        ];
        let err = fit_degree_day_regression(&constant)
            .expect_err("zero-variance design refused");
        assert!(matches!(
            err,
            ForecastBaselineError::SingularDesignMatrix { .. }
        ));
    }

    #[test]
    fn degree_day_predict_validates_its_inputs() {
        let fit = DegreeDayFit {
            intercept_mw: 1.0,
            hdd_coef_mw_per_dd: 2.0,
            cdd_coef_mw_per_dd: 3.0,
            training_samples: 3,
            baseline_version: "degree-day.v1".to_string(),
        };
        let err = degree_day_predict(&fit, &[], &[]).expect_err("empty horizon refused");
        assert_eq!(err, ForecastBaselineError::ZeroHorizonSteps);
        let err = degree_day_predict(&fit, &[1.0], &[]).expect_err("length mismatch refused");
        assert_eq!(
            err,
            ForecastBaselineError::HorizonLengthMismatch {
                hdd_len: 1,
                cdd_len: 0,
            }
        );
        let err = degree_day_predict(&fit, &[-1.0], &[0.0])
            .expect_err("negative forecast HDD refused");
        assert_eq!(
            err,
            ForecastBaselineError::NegativeDegreeDay { index: 0, value: -1.0 }
        );
        let err = degree_day_predict(&fit, &[0.0], &[f64::INFINITY])
            .expect_err("infinite forecast CDD refused");
        assert_eq!(
            err,
            ForecastBaselineError::NonFiniteForecastDegreeDay { index: 1 }
        );
    }

    #[test]
    fn every_baseline_error_renders_non_empty_prose() {
        let errors = [
            ForecastBaselineError::NonFiniteValue { index: 0 },
            ForecastBaselineError::EmptyHistory,
            ForecastBaselineError::HistoryShorterThanSeason {
                season_length: 3,
                history_len: 1,
            },
            ForecastBaselineError::ZeroSeasonLength,
            ForecastBaselineError::ZeroHorizonSteps,
            ForecastBaselineError::NegativeDegreeDay { index: 0, value: -1.0 },
            ForecastBaselineError::InsufficientTrainingSamples { found: 1 },
            ForecastBaselineError::NonFiniteTrainingLoad { index: 0 },
            ForecastBaselineError::SingularDesignMatrix {
                reason: "fixture".to_string(),
            },
            ForecastBaselineError::NonFiniteForecastDegreeDay { index: 0 },
            ForecastBaselineError::HorizonLengthMismatch {
                hdd_len: 2,
                cdd_len: 1,
            },
        ];
        for error in errors {
            assert!(!error.to_string().is_empty());
        }
    }
}
