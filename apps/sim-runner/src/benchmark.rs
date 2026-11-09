// P3.27 — Benchmark and performance measurement harness (report §19.3,
// §19.4; ledger `ledger/phase-03-flexibility-scheduling/ledger_phase_3.md`
// §P3.27).
//
// What this module is, HONESTLY:
//
// - A harness that runs a pinned P3.26 scenario through the replay pipeline,
//   times the run, and emits a machine-readable `BenchmarkRecord` (JSON,
//   P2.24 report style) per scenario × portfolio size.
// - A scaling-probe generator: deterministic ×2/×4 deep copies of the
//   scenario portfolio with asset-key-suffixed copies. NO scaling law is
//   claimed anywhere — two (or three) points are raw numbers only (ledger
//   P3.27 step 2, verbatim in the report template).
//
// What this module is NOT:
//
// - Not a scheduling-solve benchmark. The real-solve legs of P3.26 are
//   VERIFY-BUILD-gated (G5 in `25_26_tests_scenarios.md` §3); until they
//   land, the solver-side measured fields (`solver_runtime`,
//   `objective_value`, `feasible_interval_count`, `schedule_deviation`) are
//   recorded as `not_measured` with the reason — fail-loud, NEVER a default
//   zero (a silent zero would be an unmeasured claim).
// - Not a target-setting device. Report §19.4: "Do not set arbitrary
//   universal targets such as “zero violations in 1,000 cases proves
//   safety” or “ADMM converges in 30 iterations.”" The measured figures are
//   the v0.2.0 BASELINE for later comparison, not targets. No threshold
//   anywhere in this module, in CI, or in the report template gates on
//   runtime.
//
// Hardware/container capture: the record carries what the process can
// honestly observe at run time (std::env::consts, available_parallelism,
// /proc files on Linux, container-indicator files, and the
// FLEXCORE_BENCHMARK_CONTAINER override). Anything unobservable is recorded
// as `unavailable: <reason>` — never guessed, never defaulted. A CI run on a
// shared runner is a REGRESSION-context record, not a baseline figure; the
// baseline comes from the human's single recorded run (the report template
// in `ledger/phase-03-flexibility-scheduling/27_benchmark_report.md`).

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::replay::{run_scenario, ReplayError};

/// The record schema tag (bumped only with a migration note, §10.5
/// discipline).
pub const RECORD_SCHEMA: &str = "flexcore-benchmark-record/v1";

/// The baseline label every record carries. These figures are the v0.2.0
/// baseline for LATER COMPARISON — never a target (report §19.4).
pub const BASELINE_LABEL: &str =
    "v0.2.0-baseline (measured baseline for later comparison, not a target — report §19.4)";

/// The subtask tag.
pub const RECORD_SUBTASK: &str = "P3.27";

// --- Measured fields -------------------------------------------------------------

/// One measured (or explicitly NOT measured) figure. The `not_measured`
/// variant is the fail-loud contract: a field the harness could not measure
/// carries its REASON, never a zero, never a blank that reads as zero.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum MeasuredField {
    /// A figure actually produced by the recorded method on this run.
    Measured {
        /// The measured value. Finite by construction (the constructors
        /// reject non-finite input rather than serialize a NaN claim).
        value: f64,
        /// The unit, stated per §10.2 discipline (e.g. "s", "MW", "count").
        unit: String,
        /// How it was measured (the timer/calculation method, verbatim
        /// enough to reproduce).
        method: String,
    },
    /// The harness could not measure this on this run. The reason is
    /// mandatory and non-empty.
    NotMeasured { reason: String },
}

impl MeasuredField {
    /// Records a measured figure. Non-finite input is a hard error, never a
    /// serialized NaN/inf masquerading as a measurement.
    pub fn measured(value: f64, unit: &str, method: &str) -> Result<Self, ReplayError> {
        if !value.is_finite() || value < 0.0 {
            return Err(ReplayError::Config(format!(
                "a measured benchmark figure must be finite and non-negative, got {value}"
            )));
        }
        Ok(MeasuredField::Measured {
            value,
            unit: unit.to_string(),
            method: method.to_string(),
        })
    }

    /// Records an explicitly unmeasured figure with its reason.
    pub fn not_measured(reason: &str) -> Self {
        MeasuredField::NotMeasured {
            reason: reason.to_string(),
        }
    }

    /// The measured value, if there is one. A `not_measured` field NEVER
    /// yields a number (there is no default zero to fall back to).
    pub fn value(&self) -> Option<f64> {
        match self {
            MeasuredField::Measured { value, .. } => Some(*value),
            MeasuredField::NotMeasured { .. } => None,
        }
    }
}

// --- Hardware / container description --------------------------------------------

/// The hardware/container description captured at run time (ledger P3.27
/// step 1: the hardware description is part of the record — a benchmark run
/// whose environment is unknown is not a reproducible measurement). Every
/// field is either observed or `unavailable: <reason>`; nothing is invented.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HardwareDescription {
    /// OS family / OS / architecture from `std::env::consts` (compiled-in
    /// facts about the binary's target platform, observed at run time).
    pub os: String,
    /// CPU model string, best effort: `/proc/cpuinfo` "model name" on Linux;
    /// `unavailable: <reason>` elsewhere — no dependency-free reader exists
    /// for other platforms and none is invented. Record it manually in the
    /// report template when the harness cannot.
    pub cpu_model: String,
    /// Logical core count from `std::thread::available_parallelism`
    /// (cgroup/affinity-aware on supported platforms).
    pub logical_cores: String,
    /// How `logical_cores` was obtained.
    pub logical_cores_source: String,
    /// Total memory, best effort: `/proc/meminfo` MemTotal on Linux; the
    /// raw value with its unit — never a recomputed guess.
    pub memory_total: String,
    /// The container flag: an explicit override, a detected indicator file,
    /// or an explicit no-indicator result (which is NOT proof of a
    /// non-container environment — see [`container_flag`]).
    pub container_flag: String,
    /// How the container flag was determined.
    pub container_detection: String,
    /// The honesty note that travels with every record.
    pub capture_note: String,
}

/// Captures the hardware/container description at run time.
pub fn capture_hardware() -> HardwareDescription {
    let os = format!(
        "family={} os={} arch={}",
        std::env::consts::FAMILY,
        std::env::consts::OS,
        std::env::consts::ARCH
    );

    let cpu_model = read_first_proc_cpuinfo_model_name().unwrap_or_else(|| {
        "unavailable: no dependency-free CPU-model reader on this platform; \
         record the CPU model manually in the benchmark report template"
            .to_string()
    });

    let (logical_cores, logical_cores_source) = match std::thread::available_parallelism() {
        Ok(count) => (
            count.get().to_string(),
            "std::thread::available_parallelism (cgroup/affinity-aware on supported platforms)"
                .to_string(),
        ),
        Err(error) => (
            format!("unavailable: {error}"),
            "std::thread::available_parallelism failed".to_string(),
        ),
    };

    let memory_total =
        read_proc_meminfo_total_kib().unwrap_or_else(|| {
            "unavailable: no dependency-free total-memory reader on this platform; \
             record memory manually in the benchmark report template"
                .to_string()
        });

    let (container_flag, container_detection) = detect_container();

    HardwareDescription {
        os,
        cpu_model,
        logical_cores,
        logical_cores_source,
        memory_total,
        container_flag,
        container_detection,
        capture_note: "captured at run time from std and platform files; every value the \
                       process cannot observe is marked 'unavailable', never guessed. This \
                       block describes the environment; it does not claim a controlled \
                       benchmark setup."
            .to_string(),
    }
}

/// The first `/proc/cpuinfo` "model name" entry, if this is a Linux host
/// that exposes one. Absence is honest unavailability.
fn read_first_proc_cpuinfo_model_name() -> Option<String> {
    if std::env::consts::OS != "linux" {
        return None;
    }
    let text = std::fs::read_to_string("/proc/cpuinfo").ok()?;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("model name") {
            let value = rest.trim_start_matches(|c: char| c == ':' || c.is_whitespace());
            let value = value.trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

/// `/proc/meminfo` MemTotal in KiB, raw with its unit.
fn read_proc_meminfo_total_kib() -> Option<String> {
    if std::env::consts::OS != "linux" {
        return None;
    }
    let text = std::fs::read_to_string("/proc/meminfo").ok()?;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("MemTotal:") {
            let value = rest.trim();
            if !value.is_empty() {
                return Some(format!("{value} (/proc/meminfo MemTotal)"));
            }
        }
    }
    None
}

/// The container flag. Priority: the explicit `FLEXCORE_BENCHMARK_CONTAINER`
/// environment override (recorded verbatim — the operator knows best), then
/// the standard indicator files, then an explicit no-indicator result that
/// is NOT proof of anything beyond "no indicator was found".
fn detect_container() -> (String, String) {
    if let Ok(explicit) = std::env::var("FLEXCORE_BENCHMARK_CONTAINER") {
        if !explicit.trim().is_empty() {
            return (
                format!("explicit-env:{explicit}"),
                "FLEXCORE_BENCHMARK_CONTAINER environment override (recorded verbatim)"
                    .to_string(),
            );
        }
    }
    if std::path::Path::new("/.dockerenv").exists() {
        return (
            "indicator-detected:docker (/.dockerenv present)".to_string(),
            "/.dockerenv existence check".to_string(),
        );
    }
    if std::path::Path::new("/run/.containerenv").exists() {
        return (
            "indicator-detected:podman (/run/.containerenv present)".to_string(),
            "/run/.containerenv existence check".to_string(),
        );
    }
    (
        "no-indicator-found (not proof of a non-container run — record the actual \
         environment in the report template)"
            .to_string(),
        "no override set; /.dockerenv and /run/.containerenv absent".to_string(),
    )
}

// --- Scenario pin ------------------------------------------------------------------

/// The pinned scenario identity the record names (P2.13 §4 discipline
/// carried into the benchmark: the record is only reproducible against the
/// named scenario version + seed + code version).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScenarioPin {
    /// `meta.scenario_id`, verbatim.
    pub scenario_id: String,
    /// `meta.version`, verbatim.
    pub scenario_version: String,
    /// The master seed in effect (from the successful run).
    pub master_seed: u64,
    /// The `phase3.forecast_input` pin, verbatim (absent scenarios record
    /// null — never an invented pin).
    pub forecast_input: Option<String>,
    /// The `phase3.solver_pins` block, verbatim (including the solver-class
    /// discipline strings). Null when the scenario carries no pin block.
    pub solver_pins: Value,
    /// Where the pin fields came from.
    pub pin_note: String,
}

fn extract_pin(scenario: &Value, master_seed: u64) -> ScenarioPin {
    let phase3 = scenario.get("phase3");
    let string_at = |path: &[&str]| -> Option<String> {
        let mut cursor = scenario;
        for key in path {
            cursor = cursor.get(key)?;
        }
        cursor.as_str().map(str::to_string)
    };
    ScenarioPin {
        scenario_id: string_at(&["meta", "scenario_id"]).unwrap_or_default(),
        scenario_version: string_at(&["meta", "version"]).unwrap_or_default(),
        master_seed,
        forecast_input: phase3
            .and_then(|p| p.get("forecast_input"))
            .and_then(Value::as_str)
            .map(str::to_string),
        solver_pins: phase3
            .and_then(|p| p.get("solver_pins"))
            .cloned()
            .unwrap_or(Value::Null),
        pin_note: "meta.* and phase3.* fields verbatim from the scenario artefact; the seed \
                   comes from the successful run. A missing pin is null, never filled in."
            .to_string(),
    }
}

// --- Scaling probe ------------------------------------------------------------------

/// The scaling-probe description that travels inside the record (ledger
/// P3.27 step 2: "the copies are part of the record"). Factor 1 is the base
/// scenario with no copies.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScalingSpec {
    /// The portfolio-size factor: 1 = base scenario, 2/4 = the probe points.
    /// NO scaling law is claimed from these points — raw numbers only
    /// (ledger P3.27 step 2, verbatim in the report template).
    pub portfolio_factor: u32,
    /// The copy method, verbatim enough to reproduce.
    pub method: String,
    /// The suffixed site keys of the generated portfolio (empty for
    /// factor 1).
    pub copied_site_keys: Vec<String>,
    /// The number of asset keys in the generated portfolio.
    pub asset_key_count: usize,
}

/// Generates the ×`factor` scaling copy of a scenario: a deep copy of EVERY
/// portfolio site, `factor` times, with the deterministic suffix
/// `--x<factor>-<k>` (k = 1..=factor) on `site_key` and every `asset_key`,
/// and `meta.scenario_id` suffixed `--x<factor>`. `faults.injections` and
/// `expected_quality.per_asset` asset keys are remapped to the copy-1 key of
/// the referenced asset (the scenario loader validates injection keys
/// against the portfolio). Every copy is suffixed — including the first —
/// so the base, ×2, and ×4 key sets are pairwise disjoint. Generation walks
/// the document in order: the same input yields byte-identical output.
///
/// Factor 1 returns the input unchanged. Factor 0 is a usage error.
pub fn scaling_scenario_json(scenario_json: &str, factor: u32) -> Result<String, ReplayError> {
    if factor == 0 {
        return Err(ReplayError::Config(
            "the scaling factor must be >= 1".to_string(),
        ));
    }
    if factor == 1 {
        return Ok(scenario_json.to_string());
    }
    let mut scenario: Value = serde_json::from_str(scenario_json)
        .map_err(|e| ReplayError::Config(format!("the scenario is not valid JSON: {e}")))?;

    let sites = scenario
        .get_mut("portfolio")
        .and_then(|p| p.get_mut("sites"))
        .and_then(Value::as_array_mut)
        .ok_or_else(|| ReplayError::Config("the scenario has no portfolio.sites array".to_string()))?
        .clone();

    let mut scaled_sites = Vec::new();
    let mut key_map: std::collections::BTreeMap<String, String> = std::collections::BTreeMap::new();
    for site in &sites {
        for k in 1..=factor {
            let mut copy = site.clone();
            let suffix = format!("--x{factor}-{k}");
            if let Some(site_key) = copy.get("site_key").and_then(Value::as_str) {
                let new_site_key = format!("{site_key}{suffix}");
                copy["site_key"] = Value::String(new_site_key);
            }
            if let Some(assets) = copy.get_mut("assets").and_then(Value::as_array_mut) {
                for asset in assets.iter_mut() {
                    if let Some(asset_key) = asset.get("asset_key").and_then(Value::as_str) {
                        let new_key = format!("{asset_key}{suffix}");
                        if k == 1 {
                            key_map.insert(asset_key.to_string(), new_key.clone());
                        }
                        asset["asset_key"] = Value::String(new_key);
                    }
                }
            }
            scaled_sites.push(copy);
        }
    }

    scenario["portfolio"]["sites"] = Value::Array(scaled_sites);

    // The scaled run is a DIFFERENT artefact identity — suffix the scenario
    // id so no record can be mistaken for the base scenario's.
    let scenario_id = scenario
        .get("meta")
        .and_then(|m| m.get("scenario_id"))
        .and_then(Value::as_str)
        .map(str::to_string);
    if let Some(id) = scenario_id {
        scenario["meta"]["scenario_id"] = Value::String(format!("{id}--x{factor}"));
    }

    // Remap the injection and expected-quality asset keys to their copy-1
    // keys (the loader rejects injection keys that are not in the
    // portfolio). Keys with no mapped original are left verbatim — the
    // loader's own validation then fails loudly if they are invalid.
    let remap = |value: &mut Value| {
        if let Some(key) = value.get("asset_key").and_then(Value::as_str) {
            if let Some(mapped) = key_map.get(key) {
                value["asset_key"] = Value::String(mapped.clone());
            }
        }
    };
    if let Some(injections) = scenario
        .get_mut("faults")
        .and_then(|f| f.get_mut("injections"))
        .and_then(Value::as_array_mut)
    {
        for injection in injections {
            remap(injection);
        }
    }
    if let Some(entries) = scenario
        .get_mut("expected_quality")
        .and_then(|q| q.get_mut("per_asset"))
        .and_then(Value::as_array_mut)
    {
        for entry in entries {
            remap(entry);
        }
    }

    serde_json::to_string(&scenario)
        .map_err(|e| ReplayError::Config(format!("the scaling copy could not be rendered: {e}")))
}

/// Counts the asset keys of a parsed scenario.
fn asset_key_count(scenario: &Value) -> usize {
    scenario
        .get("portfolio")
        .and_then(|p| p.get("sites"))
        .and_then(Value::as_array)
        .map(|sites| {
            sites
                .iter()
                .map(|site| {
                    site.get("assets")
                        .and_then(Value::as_array)
                        .map(std::vec::Vec::len)
                        .unwrap_or(0)
                })
                .sum()
        })
        .unwrap_or(0)
}

/// Collects the site keys of a parsed scenario.
fn site_keys(scenario: &Value) -> Vec<String> {
    scenario
        .get("portfolio")
        .and_then(|p| p.get("sites"))
        .and_then(Value::as_array)
        .map(|sites| {
            sites
                .iter()
                .filter_map(|site| {
                    site.get("site_key").and_then(Value::as_str).map(str::to_string)
                })
                .collect()
        })
        .unwrap_or_default()
}

// --- The record ----------------------------------------------------------------------

/// The measured figures of one scenario × portfolio-size point. The four
/// §19.4 scheduling metrics are ALL present; the ones this pipeline cannot
/// measure yet are `not_measured` with the reason (fail-loud, never a
/// default zero).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MeasuredBlock {
    /// Wall-clock runtime of the full replay pipeline (the only figure this
    /// harness can measure today), seconds.
    pub replay_runtime: MeasuredField,
    /// Scheduling-solve runtime, seconds — `not_measured` until the
    /// VERIFY-BUILD G5 real-solve legs land.
    pub solver_runtime: MeasuredField,
    /// The §19.4 objective value — `not_measured` until a real solve
    /// exists.
    pub objective_value: MeasuredField,
    /// The §19.4 feasible interval count — `not_measured` until a real
    /// solve exists.
    pub feasible_interval_count: MeasuredField,
    /// The §19.4 schedule deviation — `not_measured` until a real solve
    /// exists.
    pub schedule_deviation: MeasuredField,
    /// The number of canonical records the replay pipeline produced
    /// (deterministic for a pinned scenario; a factual count, not a
    /// performance figure).
    pub replay_record_count: usize,
}

/// The run context: who ran it and when (the only volatile field of the
/// record; excluded from the stability view).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunContext {
    /// The code version the run names (`--code-version`, the
    /// `FLEXCORE_CODE_VERSION` environment, or `dev-unrecorded` — an
    /// unrecorded code version is stated, never faked; P2.13 §4).
    pub code_version: String,
    /// Wall-clock generation instant, Unix milliseconds. Volatile metadata.
    pub generated_at_unix_ms: i64,
}

/// One benchmark record: scenario × portfolio size, with the measured
/// figures, the hardware/container description, and the scaling-probe
/// description. Machine-readable JSON (P2.24 report style).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BenchmarkRecord {
    /// The schema tag ([`RECORD_SCHEMA`]).
    pub record_schema: String,
    /// The subtask tag ([`RECORD_SUBTASK`]).
    pub subtask: String,
    /// The baseline label ([`BASELINE_LABEL`]) — these figures are the
    /// v0.2.0 baseline for later comparison, never a target.
    pub baseline_label: String,
    /// The pinned scenario identity.
    pub scenario_pin: ScenarioPin,
    /// The scaling-probe description (factor 1 = base scenario).
    pub scaling: ScalingSpec,
    /// The hardware/container description captured at run time.
    pub hardware: HardwareDescription,
    /// The measured figures.
    pub measured: MeasuredBlock,
    /// The completeness summary (recomputed by [`BenchmarkRecord::
    /// validate_complete`]; the authoritative check is the recompute).
    pub completeness: Completeness,
    /// The run context.
    pub run_context: RunContext,
}

/// The record completeness summary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Completeness {
    /// True only when every measured field is actually measured.
    pub complete: bool,
    /// The names of the `not_measured` fields (empty when complete).
    pub unmeasured: Vec<String>,
}

impl BenchmarkRecord {
    /// Measures the BASE scenario (factor 1): runs the replay pipeline once,
    /// times it, and records the hardware description and scenario pin.
    pub fn measure_base(scenario_json: &str, code_version: &str) -> Result<Self, ReplayError> {
        Self::measure_factor(scenario_json, 1, code_version)
    }

    /// Measures the ×`factor` scaling probe: generates the copy
    /// ([`scaling_scenario_json`]), runs it through the replay pipeline,
    /// times it, and records the copy spec as part of the record.
    pub fn measure_scaling(
        scenario_json: &str,
        factor: u32,
        code_version: &str,
    ) -> Result<Self, ReplayError> {
        Self::measure_factor(scenario_json, factor, code_version)
    }

    fn measure_factor(scenario_json: &str, factor: u32, code_version: &str) -> Result<Self, ReplayError> {
        // The artifact actually measured: the base scenario at factor 1, the
        // generated copy otherwise. The pin is extracted from THIS artefact
        // so a scaling record names its suffixed identity, never the base
        // scenario's.
        let effective_json = if factor == 1 {
            scenario_json.to_string()
        } else {
            scaling_scenario_json(scenario_json, factor)?
        };
        let effective_value: Value = serde_json::from_str(&effective_json)
            .map_err(|e| ReplayError::Config(format!("the scenario is not valid JSON: {e}")))?;

        let copied_site_keys = if factor == 1 {
            Vec::new()
        } else {
            site_keys(&effective_value)
        };

        let start = std::time::Instant::now();
        let run = run_scenario(&effective_json)?;
        let elapsed = start.elapsed().as_secs_f64();

        let scaling = ScalingSpec {
            portfolio_factor: factor,
            method: if factor == 1 {
                "base scenario, no copies".to_string()
            } else {
                "deep copy of every portfolio site, factor times, deterministic suffix \
                 '--x<factor>-<k>' (k = 1..=factor) on site_key and every asset_key; \
                 meta.scenario_id suffixed '--x<factor>'; faults.injections and \
                 expected_quality.per_asset asset keys remapped to the copy-1 key. \
                 Generated in document order — byte-identical for the same input."
                    .to_string()
            },
            copied_site_keys,
            asset_key_count: asset_key_count(&effective_value),
        };

        let replay_runtime = MeasuredField::measured(
            elapsed,
            "s",
            "std::time::Instant around the full replay pipeline (scenario load through \
             canonical record rendering); a single sequential run, no repetitions, no \
             warmup — the raw figure of THIS run",
        )?;

        let record = BenchmarkRecord {
            record_schema: RECORD_SCHEMA.to_string(),
            subtask: RECORD_SUBTASK.to_string(),
            baseline_label: BASELINE_LABEL.to_string(),
            scenario_pin: extract_pin(&effective_value, run.master_seed),
            scaling,
            hardware: capture_hardware(),
            measured: MeasuredBlock {
                replay_runtime,
                solver_runtime: MeasuredField::not_measured(
                    "no real scheduling solve is wired into the sim-runner replay pipeline \
                     yet (VERIFY-BUILD G5, ledger/phase-03-flexibility-scheduling/\
                     25_26_tests_scenarios.md §3); the recorded runtime is the replay/\
                     generation pipeline only and is NEVER presented as scheduling-solve \
                     time",
                ),
                objective_value: MeasuredField::not_measured(
                    "no solved schedule exists in this pipeline (VERIFY-BUILD G5); the \
                     §19.4 objective value is recorded here only once a pinned real solve \
                     produces one — never inferred, never zero-filled",
                ),
                feasible_interval_count: MeasuredField::not_measured(
                    "no solved schedule exists in this pipeline (VERIFY-BUILD G5); the \
                     deterministic candidate evaluations of P3.26 are covered by \
                     apps/sim-runner/tests/phase3_scenarios.rs and are NOT a solver's \
                     feasible-interval count",
                ),
                schedule_deviation: MeasuredField::not_measured(
                    "no solved schedule and no realised-telemetry comparison exist in this \
                     pipeline (VERIFY-BUILD G5); the §19.4 schedule deviation is recorded \
                     only from a real solve — never inferred, never zero-filled",
                ),
                replay_record_count: run.records.len(),
            },
            completeness: Completeness {
                complete: false,
                unmeasured: Vec::new(),
            },
            run_context: RunContext {
                code_version: code_version.to_string(),
                generated_at_unix_ms: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis() as i64)
                    .unwrap_or(0),
            },
        };
        Ok(record.with_recomputed_completeness())
    }

    /// Recomputes the completeness summary from the measured block.
    pub fn with_recomputed_completeness(mut self) -> Self {
        let mut unmeasured = Vec::new();
        if self.measured.replay_runtime.value().is_none() {
            unmeasured.push("replay_runtime".to_string());
        }
        if self.measured.solver_runtime.value().is_none() {
            unmeasured.push("solver_runtime".to_string());
        }
        if self.measured.objective_value.value().is_none() {
            unmeasured.push("objective_value".to_string());
        }
        if self.measured
            .feasible_interval_count
            .value()
            .is_none()
        {
            unmeasured.push("feasible_interval_count".to_string());
        }
        if self.measured.schedule_deviation.value().is_none() {
            unmeasured.push("schedule_deviation".to_string());
        }
        self.completeness = Completeness {
            complete: unmeasured.is_empty(),
            unmeasured,
        };
        self
    }

    /// The fail-loud completeness check: Ok only when every §19.4 measured
    /// field carries an actual measurement. There is no default zero to
    /// fall back to — an unmeasured field is an error with the field named.
    pub fn validate_complete(&self) -> Result<(), Vec<String>> {
        let recomputed = self.clone().with_recomputed_completeness();
        if recomputed.completeness.complete {
            Ok(())
        } else {
            Err(recomputed.completeness.unmeasured)
        }
    }

    /// The JSON rendering (pretty; stable field order for reviewable diffs).
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// The replay-stability view: the record with the volatile wall-clock
    /// instant removed and every measured field reduced to its status tag
    /// (the measured NUMBER may legitimately differ between runs; everything
    /// else must not). Two runs of the same pinned scenario produce equal
    /// stability views — that is the testable determinism contract.
    pub fn stability_json(&self) -> Result<String, serde_json::Error> {
        let mut value = serde_json::to_value(self)?;
        if let Some(context) = value.get_mut("run_context").and_then(Value::as_object_mut) {
            context.remove("generated_at_unix_ms");
        }
        if let Some(measured) = value.get_mut("measured").and_then(Value::as_object_mut) {
            for (_name, field) in measured.iter_mut() {
                if let Some(field_object) = field.as_object_mut() {
                    field_object.remove("value");
                }
            }
        }
        serde_json::to_string_pretty(&value)
    }
}

// --- A small self-check ------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_not_measured_field_never_yields_a_value_or_a_zero() {
        let field = MeasuredField::not_measured("the reason is mandatory");
        assert_eq!(field.value(), None);
        let json = serde_json::to_string(&field).unwrap();
        assert!(json.contains("not_measured"));
        assert!(!json.contains("\"value\""), "no numeric value may appear");
        assert!(!json.contains("0"), "no default zero may appear");
    }

    #[test]
    fn a_non_finite_or_negative_figure_is_refused_not_serialized() {
        assert!(MeasuredField::measured(f64::NAN, "s", "t").is_err());
        assert!(MeasuredField::measured(f64::INFINITY, "s", "t").is_err());
        assert!(MeasuredField::measured(-1.0, "s", "t").is_err());
    }

    #[test]
    fn the_hardware_capture_marks_unobservables_instead_of_guessing() {
        let hardware = capture_hardware();
        for (name, value) in [
            ("os", &hardware.os),
            ("cpu_model", &hardware.cpu_model),
            ("logical_cores", &hardware.logical_cores),
            ("memory_total", &hardware.memory_total),
            ("container_flag", &hardware.container_flag),
        ] {
            assert!(!value.trim().is_empty(), "{name} must never be blank");
        }
        // The honesty notes travel with the record on every platform.
        assert!(hardware.capture_note.contains("unavailable"));
        assert!(hardware.container_detection.contains("override") || hardware.container_detection.contains("check"));
    }

    #[test]
    fn factor_zero_is_a_usage_error_and_factor_one_is_the_identity() {
        let scenario = r#"{"meta":{"scenario_id":"s"},"portfolio":{"sites":[]}}"#;
        assert!(scaling_scenario_json(scenario, 0).is_err());
        assert_eq!(scaling_scenario_json(scenario, 1).unwrap(), scenario);
    }
}
