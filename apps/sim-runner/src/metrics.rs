// P5.23 — The additive per-leg timing surface for the scenario-run
//         composition (report §19.3–§19.4; ledger
//         `ledger/phase-05-operator-console-simulator/ledger_phase_5.md`
//         P5.23; the P4.35 timing-surface discipline,
//         `ledger/phase-04-grid-feasibility-dispatch/35_gate_benchmark_report.md`
//         §2).
//
// WHAT THIS MODULE IS:
//
// - A SMALL timing surface, exactly like the one P4.35 §2 recorded as the
//   missing piece for its limit-check columns: the composition root times
//   its legs (telemetry ingest pump, dispatch slice, horizon sweep, fault
//   probes, record build) with a wall-clock timer and records the durations
//   beside the run record, so the P5.23 benchmark template's per-leg
//   wall-clock columns are fillable FROM A PRODUCED RECORD.
//
// WHAT THIS MODULE IS NOT:
//
// - Not a scheduler benchmark. The §19.4 scheduling metrics (objective
//   value, schedule deviation, feasible interval count) need a real solve;
//   until one is wired into this composition they are `not_measured` in the
//   benchmark template — never zero-filled (the P3.27 rule, carried
//   forward).
// - Not a target-setting device. Report §19.4: "Do not set arbitrary
//   universal targets." No threshold anywhere in this module gates on a
//   duration; the figures are baseline inputs for later comparison.
//
// ## The measurement clock vs the state clock (P0.09 discipline)
//
// The run's STATE clock is the scenario clock ([`RunClock`],
// `scenario_run.rs`) — no wall clock is read for state anywhere. The
// MEASUREMENT clock is a different thing and is honestly wall-clock:
// `std::time::Instant` around a leg, like the P3.27 `BenchmarkRecord`
// measurement (a single sequential measurement, no repetitions, no warmup —
// the raw figure of THIS run). The two clocks are never mixed and the
// record never claims a scenario-clock instant for a measured duration.
//
// Every recorded duration is finite and non-negative or a named error —
// a NaN/inf/negative figure is refused at the constructor, never
// serialized (the P3.27 `MeasuredField` discipline, reused).

use serde::Serialize;

/// The timings-block schema tag (bumped only with a migration note, §10.5).
pub const METRICS_SCHEMA: &str = "flexcore-scenario-run-timings/v1";

/// The subtask tag.
pub const METRICS_SUBTASK: &str = "P5.23";

/// The statement carried on every timings block (rendered by consumers,
/// never paraphrased).
pub const TIMINGS_STATEMENT: &str = "\
    Per-leg wall-clock durations of ONE sequential scenario-run composition \
    (no repetitions, no warmup — the raw figures of THIS run, the P3.27 \
    measurement discipline). The run's STATE clock is the scenario clock; \
    these durations are MEASUREMENTS and never appear as scenario-clock \
    instants. They are baseline inputs for later comparison (§19.4), never \
    targets, and they authorize no conclusion about any other hardware, \
    load, or deployment.";

/// The leg names the composition times (closed in practice; the record
/// renders whatever legs were actually measured — an untimed leg is
/// ABSENT, never zero).
pub mod legs {
    /// The telemetry-ingest pump leg (the bounded pull through the real
    /// source during the horizon sweep).
    pub const TELEMETRY_INGEST_PUMP: &str = "telemetry_ingest_pump";
    /// The §24 steps 5–8 dispatch slice (gate chain + issue + tracking).
    pub const DISPATCH_SLICE: &str = "dispatch_slice";
    /// The full-horizon sweep (quality accounting over ALL telemetry).
    pub const HORIZON_SWEEP: &str = "horizon_sweep";
    /// The composition-level fault probes (clock skew, partial fleet),
    /// when run.
    pub const FAULT_PROBES: &str = "fault_probes";
    /// The run-record build (including the fault journal).
    pub const RECORD_BUILD: &str = "record_build";
}

/// One measured leg duration (finite, non-negative, with its method).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LegTiming {
    /// The leg name (see [`legs`]).
    pub leg: String,
    /// The measured duration, seconds (full precision, never rounded in
    /// the record).
    pub duration_seconds: f64,
    /// The measurement method, verbatim enough to reproduce.
    pub method: String,
}

/// The wall-clock measurement timer (MEASUREMENT only — never a state
/// clock; see the module header).
#[derive(Debug)]
pub struct LegTimer {
    started: std::time::Instant,
}

impl LegTimer {
    /// Starts the timer.
    pub fn start() -> Self {
        Self {
            started: std::time::Instant::now(),
        }
    }

    /// The elapsed duration in seconds (full f64 precision).
    pub fn elapsed_secs(&self) -> f64 {
        self.started.elapsed().as_secs_f64()
    }
}

/// The P5.23 timings block: the per-leg durations of one scenario-run
/// composition. Serializable (the additive `timings` field of the
/// `ScenarioRunRecord` JSON).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ScenarioRunTimings {
    /// The schema tag ([`METRICS_SCHEMA`]).
    pub record_schema: String,
    /// The subtask tag ([`METRICS_SUBTASK`]).
    pub subtask: String,
    /// The code version the run names (the same convention as the run
    /// record: `--code-version`, else `FLEXCORE_CODE_VERSION`, else
    /// `dev-unrecorded`).
    pub code_version: String,
    /// The scenario id (verbatim from the validated artefact).
    pub scenario_id: String,
    /// The scenario version.
    pub scenario_version: String,
    /// The master seed in effect.
    pub master_seed: u64,
    /// The measured legs, in the order they were recorded. An untimed leg
    /// is absent — never zero, never a placeholder.
    pub legs: Vec<LegTiming>,
    /// The verbatim statement ([`TIMINGS_STATEMENT`]).
    pub statement: String,
}

impl ScenarioRunTimings {
    /// Creates the block for a run identity. No legs are recorded yet.
    pub fn new(
        code_version: &str,
        scenario_id: &str,
        scenario_version: &str,
        master_seed: u64,
    ) -> Self {
        Self {
            record_schema: METRICS_SCHEMA.to_string(),
            subtask: METRICS_SUBTASK.to_string(),
            code_version: code_version.to_string(),
            scenario_id: scenario_id.to_string(),
            scenario_version: scenario_version.to_string(),
            master_seed,
            legs: Vec::new(),
            statement: TIMINGS_STATEMENT.to_string(),
        }
    }

    /// Records one leg's duration. A non-finite or negative duration is a
    /// NAMED error (never serialized); a duplicate leg name is a NAMED
    /// error (a leg is recorded once — an overwrite would hide the first
    /// measurement). A leg that was not timed simply never appears here.
    pub fn record(&mut self, leg: &str, duration_seconds: f64) -> Result<(), MetricsError> {
        if !duration_seconds.is_finite() || duration_seconds < 0.0 {
            return Err(MetricsError {
                detail: format!(
                    "a measured leg duration must be finite and non-negative, got \
                     {duration_seconds} (leg '{leg}') — no default zero is ever substituted"
                ),
            });
        }
        if self.legs.iter().any(|timing| timing.leg == leg) {
            return Err(MetricsError {
                detail: format!(
                    "the leg '{leg}' was already recorded — a leg is measured once per \
                     run and never silently overwritten"
                ),
            });
        }
        self.legs.push(LegTiming {
            leg: leg.to_string(),
            duration_seconds,
            method: "std::time::Instant around the leg (a single sequential \
                     measurement; wall-clock MEASUREMENT, never a state-clock \
                     instant — P0.09 discipline)"
                .to_string(),
        });
        Ok(())
    }

    /// Whether the leg was measured (the completeness check the benchmark
    /// template's fill rules use: a column without this leg's record stays
    /// EMPTY with its reason).
    pub fn has_leg(&self, leg: &str) -> bool {
        self.legs.iter().any(|timing| timing.leg == leg)
    }

    /// The measured duration of a leg, if recorded. An untimed leg NEVER
    /// yields a number (there is no default zero to fall back to).
    pub fn leg_duration(&self, leg: &str) -> Option<f64> {
        self.legs
            .iter()
            .find(|timing| timing.leg == leg)
            .map(|timing| timing.duration_seconds)
    }

    /// The JSON rendering (pretty; stable field order for reviewable diffs).
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

/// A timings error (named, fail-loud; the caller decides whether to abort
/// the run — the CLI records it as a configuration error, exit code 2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetricsError {
    /// What was refused and why (the leg name always travels).
    pub detail: String,
}

impl std::fmt::Display for MetricsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "scenario-run timings error: {}", self.detail)
    }
}

impl std::error::Error for MetricsError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_leg_is_recorded_once_and_never_overwritten() {
        let mut timings = ScenarioRunTimings::new("test-code", "s", "v1", 1);
        timings.record(legs::DISPATCH_SLICE, 0.25).unwrap();
        let second = timings.record(legs::DISPATCH_SLICE, 0.5);
        assert!(second.is_err(), "a duplicate leg is a named error");
        assert_eq!(timings.leg_duration(legs::DISPATCH_SLICE), Some(0.25));
    }

    #[test]
    fn a_non_finite_or_negative_duration_is_refused_never_zero_filled() {
        let mut timings = ScenarioRunTimings::new("test-code", "s", "v1", 1);
        assert!(timings.record(legs::HORIZON_SWEEP, f64::NAN).is_err());
        assert!(timings.record(legs::HORIZON_SWEEP, f64::INFINITY).is_err());
        assert!(timings.record(legs::HORIZON_SWEEP, -1.0).is_err());
        assert!(
            !timings.has_leg(legs::HORIZON_SWEEP),
            "a refused figure must not leave a leg behind"
        );
        assert_eq!(timings.leg_duration(legs::HORIZON_SWEEP), None);
    }

    #[test]
    fn an_untimed_leg_is_absent_never_zero() {
        let timings = ScenarioRunTimings::new("test-code", "s", "v1", 1);
        assert!(!timings.has_leg(legs::FAULT_PROBES));
        assert_eq!(timings.leg_duration(legs::FAULT_PROBES), None);
        let json = timings.to_json().unwrap();
        assert!(!json.contains("fault_probes"), "no placeholder leg");
        assert!(json.contains(METRICS_SCHEMA));
        assert!(json.contains(TIMINGS_STATEMENT));
    }

    #[test]
    fn the_block_carries_the_run_identity_and_the_statement() {
        let mut timings = ScenarioRunTimings::new("test-code", "scenario-x", "v2", 42);
        timings.record(legs::RECORD_BUILD, 0.001).unwrap();
        let value: serde_json::Value = serde_json::from_str(&timings.to_json().unwrap()).unwrap();
        assert_eq!(value["code_version"], "test-code");
        assert_eq!(value["scenario_id"], "scenario-x");
        assert_eq!(value["scenario_version"], "v2");
        assert_eq!(value["master_seed"], 42);
        assert_eq!(value["legs"][0]["leg"], legs::RECORD_BUILD);
        assert!(value["legs"][0]["method"].as_str().unwrap().contains("Instant"));
    }
}
