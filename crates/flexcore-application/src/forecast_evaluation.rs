// P15.05 — The forecast evaluation framework (the X-8 extension to
//         forecasting): metrics, the backtesting protocol, pre-registered
//         thresholds, and the structural emptiness of every result cell.
//
// Phase 15 plan §2 row P15.05; decision record:
// `docs/phases-11-20/phase-15-forecasting-asset-intelligence/records/
// 05_p15_05_forecast_evaluation.md`.
// Frozen eval set: `benchmarks/forecast-eval-set-v1.json` (mirrors
// `benchmarks/research-set-v1.json`; a change is a NEW version).
//
// WHAT THIS IS: the metric vocabulary (MAE, RMSE, skill score vs the
// persistence reference), the backtesting protocol types, and the
// pre-registration discipline (X-8) applied to forecasting — registered
// BEFORE any run that could fill them, observed values kept STRUCTURALLY
// SEPARATE from registered thresholds.
//
// THE X-8 DISCIPLINE, EXTENDED TO FORECASTING (program conventions §5):
//
// 1. **Thresholds are registered before any run that could fill them.**
//    A [`RegisteredThreshold`] is part of the FROZEN eval-set artifact
//    (`benchmarks/forecast-eval-set-v1.json`, `registered_thresholds`),
//    written before any run exists. A threshold is never fitted after
//    seeing numbers.
// 2. **Observed values live structurally separate.** The
//    [`EvaluationRunRecord`] type cannot be constructed inside this
//    program: its authorization gate (`EvaluationRunGate`) has NO public
//    constructor — the human/lab run gate is the only thing that can
//    mint one (a VERIFY-BUILD item recorded in the decision record; the
//    type exists so the refusal is structural, not prose). Every result
//    cell in the frozen eval set is EMPTY; nothing in this program fills
//    one.
// 3. **No skill is claimed anywhere.** The skill score is DEFINED here
//    (1 − MAE_model / MAE_persistence) and COMPUTED nowhere in this
//    program (no evaluation run exists). A model "has skill" only when a
//    gated run shows a positive skill score against the persistence
//    reference on a frozen eval set under a pre-registered threshold —
//    and until then the standing claim is no-skill (P15.02's
//    NO_SKILL_STATEMENT).
// 4. **Anti-silent-growth**: the eval set is a versioned frozen artifact;
//    a change to ANY field is a NEW version id
//    (`forecast-eval-set-v2`, …) frozen as a NEW file — never an in-place
//    edit (the P8.19 discipline, mirrored verbatim in the JSON's
//    schema_note).
// 5. **Backtesting protocol frozen before the run**: windows, cadence,
//    and the train/test split rule are part of the frozen artifact; a
//    forecaster is evaluated only on windows it did not see (the P8.06
//    no-moving-the-baseline rule applied to time series).
//
// The metric functions themselves are HONEST ARITHMETIC (pure, tested
// against hand-computed values) — computing a metric over supplied data
// is mathematics, not a claim; what is forbidden is RECORDING a result as
// an evaluation outcome without the gated run, which the gate type makes
// structurally impossible.
//
// Control status: evaluation infrastructure for the simulation-only 0.x
// line. Nothing here authorizes, issues, or dispatches anything; an
// evaluation result is a RECORD about forecast quality, never a command
// and never a market/conformance claim.

/// The frozen eval-set version id (mirrors the P8.19 benchmark-set
/// versioning; the artifact is `benchmarks/forecast-eval-set-v1.json`).
pub const FORECAST_EVAL_SET_VERSION: &str = "forecast-eval-set-v1";

/// The eval-set id (the stable name across versions).
pub const FORECAST_EVAL_SET_ID: &str = "flexcore-forecast-eval-set";

/// The recorded evaluation statement (pinned by tests).
pub const EVALUATION_STATEMENT: &str = "every forecast evaluation cell is EMPTY in this \
     program: no evaluation run exists, no metric has been computed over real data, and \
     no skill is claimed for any model. Thresholds are pre-registered (X-8) in the frozen \
     eval-set artifact BEFORE any run; observed values live structurally separate — an \
     EvaluationRunRecord cannot be constructed inside the program (no public gate \
     constructor). The evaluation record that cites a filled run must name the eval-set \
     version id and the gate authorization; an evaluation row without the version id is \
     invalid and must not be entered.";

/// The closed metric vocabulary (X-8: the names are registered, the
/// thresholds are pre-registered, the observed values are EMPTY).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ForecastMetric {
    /// Mean absolute error of the forecast vs the observations, MW.
    MaeMw,
    /// Root mean squared error of the forecast vs the observations, MW.
    RmseMw,
    /// Skill score vs the persistence reference:
    /// `1 − MAE_model / MAE_persistence` (unitless; positive only when
    /// the model beats persistence on the same window).
    SkillScoreVsPersistence,
}

impl ForecastMetric {
    /// The canonical metric name (records and pre-registrations).
    pub const fn name(self) -> &'static str {
        match self {
            ForecastMetric::MaeMw => "mae_mw",
            ForecastMetric::RmseMw => "rmse_mw",
            ForecastMetric::SkillScoreVsPersistence => "skill_score_vs_persistence",
        }
    }

    /// The metric's unit (the skill score is unitless).
    pub const fn unit(self) -> &'static str {
        match self {
            ForecastMetric::MaeMw => "MW",
            ForecastMetric::RmseMw => "MW",
            ForecastMetric::SkillScoreVsPersistence => "unitless",
        }
    }
}

/// Why a metric computation was refused. Named, matchable.
#[derive(Debug, Clone, PartialEq)]
pub enum ForecastMetricError {
    /// An input value was NaN or infinite.
    NonFiniteInput { index: usize },
    /// The errors list was empty (a metric over nothing is not a metric).
    EmptyInput,
    /// The persistence reference MAE was not positive, so the skill score
    /// is undefined (division by zero or a negative MAE are both
    /// refusals — a negative MAE is impossible for an honest computation,
    /// so it indicates corrupt input).
    PersistenceReferenceUndefined { persistence_mae_mw: f64 },
    /// The two series lengths differed.
    LengthMismatch { forecast_len: usize, observation_len: usize },
}

impl std::fmt::Display for ForecastMetricError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ForecastMetricError::NonFiniteInput { index } => {
                write!(f, "input value at index {index} is not finite")
            }
            ForecastMetricError::EmptyInput => {
                write!(f, "a metric requires at least one error term; an empty window is \
                           refused, never scored as perfect")
            }
            ForecastMetricError::PersistenceReferenceUndefined { persistence_mae_mw } => {
                write!(
                    f,
                    "the persistence reference MAE ({persistence_mae_mw} MW) does not \
                     define a skill score; skill requires a strictly positive reference"
                )
            }
            ForecastMetricError::LengthMismatch {
                forecast_len,
                observation_len,
            } => write!(
                f,
                "the forecast series ({forecast_len} values) and observation series \
                 ({observation_len} values) must have the same length"
            ),
        }
    }
}

impl std::error::Error for ForecastMetricError {}

/// Validates a finite series (fail-closed).
fn validate_series(values: &[f64]) -> Result<(), ForecastMetricError> {
    if values.is_empty() {
        return Err(ForecastMetricError::EmptyInput);
    }
    for (index, value) in values.iter().enumerate() {
        if !value.is_finite() {
            return Err(ForecastMetricError::NonFiniteInput { index });
        }
    }
    Ok(())
}

/// The pointwise errors of a forecast against observations: `forecast(t)
/// − observation(t)`, in interval order. Lengths must match; values must
/// be finite.
pub fn pointwise_errors(
    forecast: &[f64],
    observations: &[f64],
) -> Result<Vec<f64>, ForecastMetricError> {
    if forecast.len() != observations.len() {
        return Err(ForecastMetricError::LengthMismatch {
            forecast_len: forecast.len(),
            observation_len: observations.len(),
        });
    }
    validate_series(forecast)?;
    validate_series(observations)?;
    Ok(forecast
        .iter()
        .zip(observations.iter())
        .map(|(f, o)| f - o)
        .collect())
}

/// Mean absolute error, MW. Fixed accumulation order (interval order —
/// deterministic).
pub fn mean_absolute_error(errors: &[f64]) -> Result<f64, ForecastMetricError> {
    validate_series(errors)?;
    let sum: f64 = errors.iter().map(|e| e.abs()).sum();
    Ok(sum / errors.len() as f64)
}

/// Root mean squared error, MW. Fixed accumulation order (deterministic).
pub fn root_mean_squared_error(errors: &[f64]) -> Result<f64, ForecastMetricError> {
    validate_series(errors)?;
    let sum: f64 = errors.iter().map(|e| e * e).sum();
    Ok((sum / errors.len() as f64).sqrt())
}

/// The skill score vs the persistence reference:
/// `1 − MAE_model / MAE_persistence`. Unitless. Defined ONLY for a
/// strictly positive reference MAE; a non-positive reference is a named
/// refusal (it cannot indicate skill in any direction, and a negative
/// MAE indicates corrupt input).
pub fn skill_score_vs_persistence(
    model_mae_mw: f64,
    persistence_mae_mw: f64,
) -> Result<f64, ForecastMetricError> {
    if !model_mae_mw.is_finite() || !persistence_mae_mw.is_finite() {
        return Err(ForecastMetricError::NonFiniteInput { index: 0 });
    }
    if persistence_mae_mw <= 0.0 {
        return Err(ForecastMetricError::PersistenceReferenceUndefined {
            persistence_mae_mw,
        });
    }
    Ok(1.0 - model_mae_mw / persistence_mae_mw)
}

// --- Pre-registration (X-8) -----------------------------------------------------------

/// A pre-registered threshold: part of the FROZEN eval-set artifact,
/// written BEFORE any run that could fill it (X-8). The threshold value
/// is a RECORDED number (a commitment, not an observation); the observed
/// value that would compare against it lives in a gated
/// [`EvaluationRunRecord`], structurally separate — never in this type,
/// never in the frozen artifact (a filled run never edits the artifact).
#[derive(Debug, Clone, PartialEq)]
pub struct RegisteredThreshold {
    /// The metric the threshold applies to (closed vocabulary).
    pub metric: ForecastMetric,
    /// The registered bound (a number RECORDED before any run; semantics:
    /// "a run's observed value must be AT OR BETTER THAN this bound for
    /// the pre-registered success claim to hold").
    pub registered_bound: f64,
    /// The direction of "better" (closed vocabulary; a lower MAE is
    /// better, a higher skill score is better).
    pub better_when: ThresholdDirection,
    /// The eval-set version this threshold is registered under (X-8
    /// provenance: the threshold cannot outlive or silently transfer
    /// between set versions).
    pub eval_set_version: String,
    /// The date the threshold was registered (RECORDED data — the
    /// registration happened before any run by construction, because no
    /// run exists).
    pub registered_date: String,
}

/// The direction of "better" for a threshold (closed).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThresholdDirection {
    /// Lower observed values are better (MAE, RMSE).
    LowerIsBetter,
    /// Higher observed values are better (skill score).
    HigherIsBetter,
}

impl ThresholdDirection {
    /// The canonical spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            ThresholdDirection::LowerIsBetter => "lower-is-better",
            ThresholdDirection::HigherIsBetter => "higher-is-better",
        }
    }
}

/// Why a threshold registration was refused. Named, matchable.
#[derive(Debug, Clone, PartialEq)]
pub enum ThresholdError {
    /// The bound was NaN or infinite.
    NonFiniteBound,
    /// The eval-set version did not match the frozen version (a threshold
    /// registered against another set version is refused — X-8
    /// provenance).
    EvalSetVersionMismatch { expected: String, found: String },
    /// The registered date was empty.
    EmptyRegisteredDate,
}

impl std::fmt::Display for ThresholdError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ThresholdError::NonFiniteBound => {
                write!(f, "the registered bound must be finite")
            }
            ThresholdError::EvalSetVersionMismatch { expected, found } => write!(
                f,
                "the threshold names eval-set version '{found}' but the frozen set is \
                 '{expected}'; thresholds do not silently transfer between set versions"
            ),
            ThresholdError::EmptyRegisteredDate => {
                write!(f, "the registered date must be non-empty (X-8 provenance)")
            }
        }
    }
}

impl std::error::Error for ThresholdError {}

impl RegisteredThreshold {
    /// Registers one threshold against the FROZEN eval set (X-8: this is
    /// a pre-run commitment; the eval set has no run and every result
    /// cell is EMPTY by construction).
    pub fn register(
        metric: ForecastMetric,
        registered_bound: f64,
        better_when: ThresholdDirection,
        registered_date: String,
    ) -> Result<Self, ThresholdError> {
        if !registered_bound.is_finite() {
            return Err(ThresholdError::NonFiniteBound);
        }
        if registered_date.trim().is_empty() {
            return Err(ThresholdError::EmptyRegisteredDate);
        }
        Ok(Self {
            metric,
            registered_bound,
            better_when,
            eval_set_version: FORECAST_EVAL_SET_VERSION.to_string(),
            registered_date,
        })
    }

    /// Full validation (also usable on a parsed threshold).
    pub fn validate(&self) -> Result<(), ThresholdError> {
        if !self.registered_bound.is_finite() {
            return Err(ThresholdError::NonFiniteBound);
        }
        if self.eval_set_version != FORECAST_EVAL_SET_VERSION {
            return Err(ThresholdError::EvalSetVersionMismatch {
                expected: FORECAST_EVAL_SET_VERSION.to_string(),
                found: self.eval_set_version.clone(),
            });
        }
        if self.registered_date.trim().is_empty() {
            return Err(ThresholdError::EmptyRegisteredDate);
        }
        Ok(())
    }
}

// --- The backtesting protocol ---------------------------------------------------------

/// One backtesting window: the frozen train/test split of the eval set
/// (the P8.06 discipline applied to time series — a forecaster is
/// evaluated only on windows it did not see). Windows are DECLARED in the
/// frozen artifact; they are never chosen after seeing a model's outputs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BacktestWindow {
    /// The window id (e.g. `window/da-2026-06-01`).
    pub window_id: String,
    /// Train-window start (UTC millis, start-inclusive).
    pub train_from_millis: i64,
    /// Train-window end (UTC millis, start-inclusive of the test window).
    pub train_until_millis: i64,
    /// Test-window start (UTC millis, start-inclusive).
    pub test_from_millis: i64,
    /// Test-window end (UTC millis, end-exclusive).
    pub test_until_millis: i64,
}

/// Why a backtesting window was refused. Named, matchable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BacktestWindowError {
    /// The window id was empty or whitespace.
    EmptyWindowId,
    /// The train window was inverted or empty.
    InvertedTrainWindow { from_millis: i64, until_millis: i64 },
    /// The test window was inverted or empty.
    InvertedTestWindow { from_millis: i64, until_millis: i64 },
    /// The test window overlapped or preceded the train window (a
    /// forecaster must not be scored on data inside its training window).
    TestOverlapsTrain,
}

impl std::fmt::Display for BacktestWindowError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BacktestWindowError::EmptyWindowId => {
                write!(f, "the window id must be a non-empty string")
            }
            BacktestWindowError::InvertedTrainWindow { from_millis, until_millis } => {
                write!(f, "the train window [{from_millis}, {until_millis}) is inverted or empty")
            }
            BacktestWindowError::InvertedTestWindow { from_millis, until_millis } => {
                write!(f, "the test window [{from_millis}, {until_millis}) is inverted or empty")
            }
            BacktestWindowError::TestOverlapsTrain => write!(
                f,
                "the test window overlaps or precedes the train window; a forecaster is \
                 scored only on data it did not see (the frozen split rule)"
            ),
        }
    }
}

impl std::error::Error for BacktestWindowError {}

impl BacktestWindow {
    /// Validates and constructs one frozen window (fail-closed).
    pub fn new(
        window_id: String,
        train_from_millis: i64,
        train_until_millis: i64,
        test_from_millis: i64,
        test_until_millis: i64,
    ) -> Result<Self, BacktestWindowError> {
        if window_id.trim().is_empty() {
            return Err(BacktestWindowError::EmptyWindowId);
        }
        if train_from_millis >= train_until_millis {
            return Err(BacktestWindowError::InvertedTrainWindow {
                from_millis: train_from_millis,
                until_millis: train_until_millis,
            });
        }
        if test_from_millis >= test_until_millis {
            return Err(BacktestWindowError::InvertedTestWindow {
                from_millis: test_from_millis,
                until_millis: test_until_millis,
            });
        }
        if test_from_millis < train_until_millis {
            return Err(BacktestWindowError::TestOverlapsTrain);
        }
        Ok(Self {
            window_id,
            train_from_millis,
            train_until_millis,
            test_from_millis,
            test_until_millis,
        })
    }
}

// --- The gated evaluation record (structurally unreachable in-program) --

/// The evaluation-run authorization gate. **There is NO public
/// constructor**: the human/lab run gate is the only thing that can mint
/// a gate (a VERIFY-BUILD item recorded in the decision record — the
/// constructor is added at the human gate, in the composition/lab
/// runtime, never in this program's code). This makes a filled
/// [`EvaluationRunRecord`] structurally impossible inside the program:
/// the refusal is a type, not prose.
///
/// The zero-sized private field means no instance can be built outside
/// this module, and no function in this module builds one — hence the
/// `dead_code` allowance: the type is DELIBERATELY never constructed in
/// this program (that is its entire function; the constructor is minted
/// by the human/lab run gate in the composition/lab runtime, a
/// VERIFY-BUILD item recorded in the decision record).
#[allow(dead_code)]
pub struct EvaluationRunGate {
    _private: (),
}

/// The observed-result record of ONE gated evaluation run. Unconstructable
/// in-program (see [`EvaluationRunGate`]); the type exists so the record
/// shape is REVIEWED and FROZEN before the first run (X-8: the record
/// vocabulary is registered, the values are EMPTY), and so any code that
/// would fabricate a filled record cannot compile.
#[derive(Debug, Clone, PartialEq)]
pub struct EvaluationRunRecord {
    /// The eval-set version id the run cites (a run row without the
    /// version id is invalid and must not be entered — the P8.19
    /// citation rule, mirrored).
    pub eval_set_version: String,
    /// The model reference (the registry ref, P15.06).
    pub model_ref: String,
    /// The window id the run scored.
    pub window_id: String,
    /// The metric (closed vocabulary).
    pub metric: ForecastMetric,
    /// The observed value (the number the GATED run produced — never
    /// present in-program).
    pub observed_value: f64,
    /// The gate authorization carried by the run (the human/lab gate's
    /// record reference; REQUIRED — an unattributed run is invalid).
    pub gate_authorization_ref: String,
}

// ---------------------------------------------------------------------------
// Tests (WRITTEN-UNRUN — no cargo command was executed in this loop; the
// human build/test gate owns the run. Every numeric expectation below is
// HAND-COMPUTED arithmetic over constructed inputs — never an observed
// model output, never an evaluation result. Nothing is fabricated.)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metric_arithmetic_is_hand_computed_exactly() {
        // HAND CHECK: errors [1.0, -3.0, 2.0] -> |errors| sum 6.0,
        // MAE = 2.0; squares 1+9+4 = 14, /3 = 4.666..., RMSE = sqrt(14/3).
        let errors = pointwise_errors(&[2.0, -1.0, 5.0], &[1.0, 2.0, 3.0])
            .expect("equal-length finite series");
        assert_eq!(errors, vec![1.0, -3.0, 2.0]);
        let mae = mean_absolute_error(&errors).expect("non-empty finite errors");
        assert!((mae - 2.0).abs() < 1e-12, "MAE {mae}");
        let rmse = root_mean_squared_error(&errors).expect("non-empty finite errors");
        let expected_rmse = (14.0f64 / 3.0).sqrt();
        assert!((rmse - expected_rmse).abs() < 1e-12, "RMSE {rmse} vs {expected_rmse}");
    }

    #[test]
    fn the_skill_score_is_hand_computed_and_refuses_a_nonpositive_reference() {
        // HAND CHECK: model MAE 1.5 vs persistence MAE 3.0 ->
        // skill = 1 - 0.5 = 0.5. POSITIVE skill would mean the model beat
        // persistence on this constructed window — this is arithmetic
        // over constructed numbers, NOT an evaluation result (no run
        // exists; the eval-set cells stay EMPTY).
        let skill = skill_score_vs_persistence(1.5, 3.0).expect("positive reference");
        assert!((skill - 0.5).abs() < 1e-12, "skill {skill}");
        // A model worse than persistence scores negative.
        let skill = skill_score_vs_persistence(4.5, 3.0).expect("positive reference");
        assert!((skill - (-0.5)).abs() < 1e-12, "skill {skill}");
        // A zero/negative reference is a named refusal, never a division
        // by zero.
        let err = skill_score_vs_persistence(1.0, 0.0)
            .expect_err("zero reference refused");
        assert_eq!(
            err,
            ForecastMetricError::PersistenceReferenceUndefined {
                persistence_mae_mw: 0.0
            }
        );
        let err = skill_score_vs_persistence(1.0, -2.0)
            .expect_err("negative reference refused");
        assert_eq!(
            err,
            ForecastMetricError::PersistenceReferenceUndefined {
                persistence_mae_mw: -2.0
            }
        );
    }

    #[test]
    fn metric_inputs_are_validated_fail_closed() {
        let err = mean_absolute_error(&[]).expect_err("empty errors refused");
        assert_eq!(err, ForecastMetricError::EmptyInput);
        let err = mean_absolute_error(&[f64::NAN]).expect_err("NaN refused");
        assert_eq!(err, ForecastMetricError::NonFiniteInput { index: 0 });
        let err = pointwise_errors(&[1.0], &[1.0, 2.0]).expect_err("length mismatch refused");
        assert_eq!(
            err,
            ForecastMetricError::LengthMismatch {
                forecast_len: 1,
                observation_len: 2,
            }
        );
        let err = pointwise_errors(&[f64::INFINITY], &[1.0]).expect_err("infinite refused");
        assert_eq!(err, ForecastMetricError::NonFiniteInput { index: 0 });
    }

    #[test]
    fn the_metric_vocabulary_is_closed_and_typed() {
        assert_eq!(ForecastMetric::MaeMw.name(), "mae_mw");
        assert_eq!(ForecastMetric::RmseMw.name(), "rmse_mw");
        assert_eq!(
            ForecastMetric::SkillScoreVsPersistence.name(),
            "skill_score_vs_persistence"
        );
        assert_eq!(ForecastMetric::MaeMw.unit(), "MW");
        assert_eq!(ForecastMetric::SkillScoreVsPersistence.unit(), "unitless");
    }

    #[test]
    fn thresholds_register_against_the_frozen_eval_set_only() {
        let threshold = RegisteredThreshold::register(
            ForecastMetric::SkillScoreVsPersistence,
            0.0,
            ThresholdDirection::HigherIsBetter,
            "2026-11-14".to_string(),
        )
        .expect("a pre-run registration");
        assert_eq!(threshold.eval_set_version, FORECAST_EVAL_SET_VERSION);
        threshold.validate().expect("the registration validates");
        // A threshold naming another set version is refused (X-8
        // provenance: no silent transfer between set versions).
        let mut transferred = threshold.clone();
        transferred.eval_set_version = "forecast-eval-set-v9".to_string();
        let err = transferred.validate().expect_err("version mismatch refused");
        assert_eq!(
            err,
            ThresholdError::EvalSetVersionMismatch {
                expected: FORECAST_EVAL_SET_VERSION.to_string(),
                found: "forecast-eval-set-v9".to_string(),
            }
        );
        // Non-finite bounds and empty dates are refused.
        let err = RegisteredThreshold::register(
            ForecastMetric::MaeMw,
            f64::NAN,
            ThresholdDirection::LowerIsBetter,
            "2026-11-14".to_string(),
        )
        .expect_err("NaN bound refused");
        assert_eq!(err, ThresholdError::NonFiniteBound);
        let err = RegisteredThreshold::register(
            ForecastMetric::MaeMw,
            1.0,
            ThresholdDirection::LowerIsBetter,
            "  ".to_string(),
        )
        .expect_err("empty date refused");
        assert_eq!(err, ThresholdError::EmptyRegisteredDate);
    }

    #[test]
    fn backtest_windows_enforce_the_frozen_split_rule() {
        let window = BacktestWindow::new(
            "window/fixture".to_string(),
            0,
            86_400_000,
            86_400_000,
            172_800_000,
        )
        .expect("a valid contiguous window");
        assert_eq!(window.window_id, "window/fixture");
        let err = BacktestWindow::new(
            "  ".to_string(),
            0,
            1,
            1,
            2,
        )
        .expect_err("empty id refused");
        assert_eq!(err, BacktestWindowError::EmptyWindowId);
        let err = BacktestWindow::new(
            "w".to_string(),
            10,
            10,
            10,
            20,
        )
        .expect_err("empty train window refused");
        assert_eq!(
            err,
            BacktestWindowError::InvertedTrainWindow {
                from_millis: 10,
                until_millis: 10,
            }
        );
        let err = BacktestWindow::new("w".to_string(), 0, 10, 5, 20)
            .expect_err("test inside train refused");
        assert_eq!(err, BacktestWindowError::TestOverlapsTrain);
    }

    #[test]
    fn every_metric_and_window_error_renders_non_empty_prose() {
        // Three different error types, so the prose is collected as
        // Strings: one array cannot hold ForecastMetricError,
        // ThresholdError, and BacktestWindowError together.
        let errors = [
            ForecastMetricError::NonFiniteInput { index: 0 }.to_string(),
            ForecastMetricError::EmptyInput.to_string(),
            ForecastMetricError::PersistenceReferenceUndefined { persistence_mae_mw: 0.0 }
                .to_string(),
            ForecastMetricError::LengthMismatch {
                forecast_len: 1,
                observation_len: 2,
            }
            .to_string(),
            ThresholdError::NonFiniteBound.to_string(),
            ThresholdError::EvalSetVersionMismatch {
                expected: "a".to_string(),
                found: "b".to_string(),
            }
            .to_string(),
            ThresholdError::EmptyRegisteredDate.to_string(),
            BacktestWindowError::EmptyWindowId.to_string(),
            BacktestWindowError::InvertedTrainWindow {
                from_millis: 1,
                until_millis: 0,
            }
            .to_string(),
            BacktestWindowError::InvertedTestWindow {
                from_millis: 1,
                until_millis: 0,
            }
            .to_string(),
            BacktestWindowError::TestOverlapsTrain.to_string(),
        ];
        for error in errors {
            assert!(!error.is_empty());
        }
    }

    #[test]
    fn the_evaluation_statement_carries_the_structural_emptiness() {
        assert!(EVALUATION_STATEMENT.contains("every forecast evaluation cell is EMPTY"));
        assert!(EVALUATION_STATEMENT.contains("no skill is claimed"));
        assert!(EVALUATION_STATEMENT.contains("structurally separate"));
        assert_eq!(FORECAST_EVAL_SET_VERSION, "forecast-eval-set-v1");
        assert_eq!(FORECAST_EVAL_SET_ID, "flexcore-forecast-eval-set");
        assert_eq!(ThresholdDirection::LowerIsBetter.as_str(), "lower-is-better");
        assert_eq!(ThresholdDirection::HigherIsBetter.as_str(), "higher-is-better");
    }
}
