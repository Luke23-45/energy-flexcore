// P5.18 — The ScenarioRecord v3: scenarios as PINNED, replayable records,
//         and the `replay-run` execution path (report §19.2 layer 5,
//         §19.3, §24 step 10; ledger P5.18).
//
// A ScenarioRecord v3 is the additive evolution of the pinned-record
// pattern the repository already carries (the `phase3` pin block of
// `scenarios/synthetic-portfolio/scenarios-phase3/*.json`): a versioned
// JSON record that pins EVERY input a replay needs —
//
//   - the pinned telemetry scenario (path + id + version),
//   - the pinned forecast fixture (path + version) when a solve leg
//     consumes one,
//   - the pinned synthetic grid-model fixture (a NAMED fixture kind),
//   - the objective configuration (from the declared set — never
//     free-text),
//   - the dispatch/tracking parameters (deployment configuration,
//     D-P4.16-3 — options, never hard-coded constants),
//   - the backend selection (`scripted` / `worker_process`) with the
//     provider pin,
//   - the master seed (the ONLY randomness source) with the recorded
//     derivation rule,
//   - the clock configuration (start, cadence, scheduling interval),
//   - the pre-registered tolerances (P5.19's table reference),
//   - the pre-registered expected-outcome slot (P3.26 discipline: written
//     BEFORE any run; UNRUN at authoring),
//   - the nondeterminism-source inventory with each neutralization
//     (P5.19; thread_count pinned 1 — the P3.26 solver pin, cited not
//     re-pinned).
//
// ## The replay rule (P5.18 step 2, the acceptance core)
//
// Replay loads a scenario record and re-executes it WITHOUT taking new
// inputs. Any input that cannot be reproduced from the record is an
// ERROR, never a silent default:
//
//   - the pinned scenario file must exist, load all-or-nothing, and carry
//     EXACTLY the pinned scenario id + version;
//   - the scenario's master seed must equal the pinned seed (a seed
//     override does not exist on this path — `scenario_json_with_seed` is
//     deliberately NOT used here);
//   - the pinned clock fields must match the scenario artefact;
//   - a `worker_process` backend selection MUST carry its worker pin;
//   - the pre-registered expectation slot MUST be filled (a record
//     without pre-registered expectations is rejected — the P3.26
//     anti-fitting rule);
//   - `nondeterminism_controls.thread_count` MUST be 1 (the P3.26 solver
//     pin).
//
// ## The ReplayComparisonRecord (the P5.18/P5.19 output)
//
// One comparison carries: the input-hash check (recorded vs recomputed
// over the run-input sections), the per-leg comparisons with NAMED diffs
// (the telemetry leg on the frozen P2.24 tuple via `compare::compare`;
// the run-record leg via the structural JSON diff with the P5.19
// tolerance), and the verdict (`identical` / `within_tolerance` /
// `mismatch` — the reproducibility verdict the Simulation Lab displays,
// P5.08).
//
// Two comparison kinds:
//   - `double_run` — the same record executed twice in-process; BOTH legs
//     compare (telemetry + run record). This is the P5.19 proof shape;
//     the authoring loop does NOT execute it (the loop bans hold) — the
//     proof runs are the human's pre-registered run.
//   - `baseline` — a fresh run compared against a STORED run-record JSON
//     (`scenario-run --out` artefact). The run-record leg compares; the
//     telemetry leg is honestly `not_compared` (a run record does not
//     carry the telemetry records; a stored reference records file is the
//     `run --out` form and is compared by the existing `replay`
//     subcommand).
//
// Control status: everything here re-executes SYNTHETIC simulation
// scenarios (the 0.x line) — no field read/write, no control authority,
// no safety implication.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::replay::{record_from_normalized, read_scenario_text, ReplayError, ReplayRecord};
use crate::reproducibility::{
    check_reproducibility, structural_diff, DEFAULT_VALUE_ABSOLUTE_FLOOR,
    DEFAULT_VALUE_RELATIVE_TOLERANCE, VERDICT_IDENTICAL, VERDICT_MISMATCH,
    VERDICT_WITHIN_TOLERANCE,
};
use crate::scenario_run::{
    CommandRunConfig, GridBackendSelection, GridFixtureKind, PriorScheduleFixture,
    RecordedDropout, ScenarioRunHarness, SchedulingGridConfig,
};
use flexcore_contracts::ids::FlexUuid;
use flexcore_domain::ids::{CommandId, ScheduleId};
use flexcore_simulator::load_scenario;

// ---------------------------------------------------------------------------
// The record vocabulary
// ---------------------------------------------------------------------------

/// The record schema version. A record that names another version is
/// rejected — never silently parsed.
pub const SCENARIO_RECORD_SCHEMA_VERSION: &str = "3";

/// The scale classes of the library (P5.20: "start small and add scale
/// gradually").
pub const SCALE_CLASSES: [&str; 4] = ["minimal", "small", "medium", "stress"];

/// The backend selections a record can pin.
pub const BACKEND_SELECTIONS: [&str; 2] = ["scripted", "worker_process"];

/// The named synthetic grid-model fixtures.
pub const GRID_FIXTURE_KINDS: [&str; 2] = ["three_bus_radial", "meshed_two_feeder"];

/// The pre-registered expectation status at authoring time (UNRUN — the
/// Phase 5 loop bans: nothing was built or run).
pub const EXPECTATION_STATUS_UNRUN: &str = "UNRUN";

/// The comparison kinds.
pub const COMPARISON_DOUBLE_RUN: &str = "double_run";
pub const COMPARISON_BASELINE: &str = "baseline";

// ---------------------------------------------------------------------------
// The record schema (serde: missing fields are loud errors)
// ---------------------------------------------------------------------------

/// The ScenarioRecord v3 (the pinned, replayable record; §19.2 layer 5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScenarioRecordV3 {
    /// The schema version — must be `"3"`.
    pub schema_version: String,
    /// The record id (the library case id, e.g. `lib-01-minimal-single-asset`).
    pub record_id: String,
    /// The record version (a record change creates a NEW version).
    pub record_version: String,
    /// Synthetic status: the library records are synthetic-only; a
    /// non-synthetic record would need a named real model with confirmed
    /// licensing/availability (§19.3) — not present in this phase.
    pub synthetic: bool,
    /// The case's purpose (one paragraph).
    pub purpose: String,
    /// The §19.3 bullet this case maps to (cited, never invented).
    pub benchmark_reference: String,
    /// The expected runtime CLASS of the case (a declared class, never a
    /// measured figure — the P5.23 benchmark owns measured runtimes).
    pub runtime_class: String,
    /// The scale class (`minimal` / `small` / `medium` / `stress`).
    pub scale_class: String,
    /// The pinned inputs.
    pub inputs: RecordInputs,
    /// The pinned backend.
    pub backend: RecordBackend,
    /// The pinned seeds.
    pub seeds: RecordSeeds,
    /// The pre-registered tolerances.
    pub tolerances: RecordTolerances,
    /// The pre-registered expected outcomes (P3.26 discipline).
    pub expected_outcomes: ExpectedOutcomes,
    /// The nondeterminism inventory (P5.19).
    pub nondeterminism_controls: NondeterminismControls,
    /// The solver pins (the P3.26 pattern; a scripted-only record still
    /// names its solvers for the real-solve legs).
    pub solver_pins: Vec<SolverPin>,
    /// The per-case IEEE benchmark-case record (P5.20 step 2: every IEEE
    /// case is included or explicitly excluded WITH a reason — the
    /// mandatory per-case record; a library record without it is
    /// rejected).
    pub ieee_case_record: IeeeCaseRecord,
    /// The OPTIONAL recorded input hash (the recomputed hash must match
    /// when present — a mismatch is a tamper finding, never a silent
    /// re-hash). Absent = computed fresh at first pin.
    #[serde(default)]
    pub input_hash: Option<String>,
    /// P5.21 (additive extension - DRIFT NOTE): the OPTIONAL scenario-level
    /// fault configuration, the `scenario_faults` v3 slot
    /// (`flexcore-scenario-faults/v3`, validated fail-closed by
    /// `fault_injection::ScenarioFaultPlan::from_slot_struct` against the
    /// pinned telemetry scenario's portfolio). `None` = the record runs
    /// clean. The field was added by the wave-4b subtask (P5.21) AFTER the
    /// v3 schema landed (the wave-4a record): `#[serde(default)]` keeps
    /// pre-existing records deserializing unchanged, and the input hash is
    /// NOT affected (it covers only the inputs/backend/seeds/tolerances/
    /// controls blocks). RECORDED HONESTLY: the `replay-run` executor does
    /// NOT yet consume this field - the operative injection surface in
    /// this phase remains the telemetry scenario's own `faults` plan plus
    /// the top-level `scenario_faults` slot the scenario-run path reads.
    /// Declaring a fault slot here is configuration for the §19.2 layer 6
    /// injection controls, never a claim that a fault was injected.
    #[serde(default)]
    pub faults: Option<crate::fault_injection::ScenarioFaultsSlot>,
}

/// The per-case IEEE record (§19.3 licensing/availability discipline).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IeeeCaseRecord {
    /// Whether an IEEE case is included in this record (false everywhere
    /// in this phase — no IEEE model is available and licensing is
    /// unconfirmed).
    pub included: bool,
    /// The recorded reason (for the exclusion, or the confirmed
    /// license/availability basis for an inclusion).
    pub reason: String,
}

/// The pinned inputs of a record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecordInputs {
    /// The pinned telemetry scenario.
    pub scenario: ScenarioPin,
    /// The pinned forecast fixture, when a solve leg consumes one (the
    /// sim-runner composition does not solve — the pin's presence and the
    /// file are validated; consumption is the real-solve leg,
    /// VERIFY-BUILD).
    #[serde(default)]
    pub forecast_fixture: Option<ForecastPin>,
    /// The pinned synthetic grid-model fixture.
    pub grid_model: GridModelPin,
    /// The objective configuration (from the declared set — P5.05's rule
    /// applies to records too: no free-text objective).
    pub objective_config: ObjectivePin,
    /// The pinned dispatch slice, when the case dispatches.
    #[serde(default)]
    pub dispatch: Option<DispatchPin>,
    /// The clock configuration.
    pub clock: ClockPin,
}

/// The pinned telemetry scenario (path is relative to the RECORD file's
/// directory — recorded convention).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScenarioPin {
    pub path: String,
    pub scenario_id: String,
    pub version: String,
}

/// The pinned forecast fixture (path relative to the record file's
/// directory).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ForecastPin {
    pub path: String,
    pub version: String,
    pub note: String,
}

/// The pinned synthetic grid-model fixture.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GridModelPin {
    /// The named fixture (`three_bus_radial` / `meshed_two_feeder`).
    pub fixture: String,
    pub model_id: String,
    pub version: String,
    /// The recorded statement (what the fixture is and what it is NOT —
    /// e.g. balanced only, synthetic only, no IEEE feeder).
    pub statement: String,
}

/// The objective configuration pin (P3.14's declared set).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObjectivePin {
    /// The objective name from the declared set
    /// (e.g. `SYNTHETIC_ENERGY_COST_DEVIATION_V1`).
    pub objective: String,
    /// The objective provenance (`SYNTHETIC` — D1/D2 open).
    pub provenance: String,
    /// The recorded note (weights travel with the real-solve leg; the
    /// sim-runner composition does not solve).
    pub note: String,
}

/// The pinned dispatch slice (§24 steps 6–8). Every parameter is
/// deployment configuration (D-P4.16-3) — pinned per record, never a
/// hard-coded constant.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DispatchPin {
    /// The target asset key (must be a scenario asset; a replay error
    /// otherwise).
    pub asset_key: String,
    /// The target active power, MW, export positive (P0.07 §2).
    pub target_p_mw: f64,
    /// The target reactive power, MVAr, under the site's declared
    /// convention.
    pub target_q_mvar: f64,
    /// The tracking acceptance criterion (deployment configuration).
    pub tracking_tolerance_mw: f64,
    pub tracking_min_samples: u32,
    pub tracking_timeout_ms: i64,
    /// The authorizing approver identity + role (§17.3; REQUIRED — no
    /// auto-approve, P4.13).
    pub approver: String,
    pub approver_role: String,
    /// The schedule revision the command is created FROM (canonical UUID
    /// text; the recorded composition fixture).
    pub schedule_id: String,
    pub revision: u64,
    /// The prior-schedule fixture, when a recorded dropout must trigger
    /// the REAL P3.24 rolling roll in the run record.
    #[serde(default)]
    pub roll: Option<RollPin>,
}

/// The prior-schedule fixture pin (the P3.22 revision identity the roll
/// rolls from; the plan is superseded, never rewritten).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RollPin {
    pub schedule_id: String,
    pub revision: u64,
    pub candidate_id: String,
    pub setpoint_mw: f64,
}

/// The clock configuration (the scenario clock, P0.09 §5 — never a wall
/// clock).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClockPin {
    /// The horizon start (must equal the scenario artefact's
    /// `horizon.start`).
    pub start_utc: String,
    /// The telemetry cadence in seconds (must equal the artefact's
    /// `cadence.default_seconds`).
    pub cadence_seconds: u64,
    /// The scheduling-grid interval in seconds (the P3.03 default is 900).
    pub scheduling_interval_seconds: u64,
    /// The horizon length in hours (must equal the artefact's
    /// `horizon.duration_hours`).
    pub horizon_hours: u64,
}

/// The pinned backend.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecordBackend {
    /// `scripted` (the seam-level fixture backend, deterministic) or
    /// `worker_process` (the P5.15 real leg — WRITTEN, never executed in
    /// this loop; UNVERIFIED pandapower API).
    pub selection: String,
    /// The provider pin (`pandapower@3.5.6` — the P4.01 pin discipline).
    pub provider_pin: String,
    /// The worker pin, REQUIRED when the selection is `worker_process`.
    #[serde(default)]
    pub worker: Option<WorkerPin>,
}

/// The worker-process pin (script path is REPO-ROOT-relative — the CLI
/// convention; program + timeout recorded).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkerPin {
    pub program: String,
    pub script_path: String,
    pub timeout_ms: u64,
}

/// The pinned seeds (the ONLY randomness source; D-SC4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecordSeeds {
    pub master_seed: u64,
    /// The recorded per-asset derivation rule.
    pub derivation: String,
}

/// The pre-registered tolerances (P5.19's table reference travels).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecordTolerances {
    /// The telemetry leg rule (`byte_identical` for the deterministic
    /// simulator).
    pub telemetry_leg: String,
    /// The run-record leg rule (`byte_identical` on the scripted backend;
    /// `relative_1e-9` numeric on a worker leg).
    pub run_record_leg: String,
    /// The relative tolerance for floating-point legs (DEFAULT 1e-9).
    pub value_relative_tolerance: f64,
    /// The absolute floor for near-zero values (DEFAULT 1e-12).
    pub value_absolute_floor: f64,
    /// The reference to the pre-registered tolerance table
    /// (`18_20_replay_library.md` §tolerance-table).
    pub table_reference: String,
}

/// The pre-registered expected-outcome slot (P3.26 discipline: written
/// BEFORE any run; a mismatch is a finding, never a fitting opportunity).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExpectedOutcomes {
    /// Must be `true` — a record without pre-registered expectations is
    /// rejected.
    pub pre_registered: bool,
    /// The expectation status (`UNRUN` at authoring; the human's run
    /// updates the status, never the expectations).
    pub status: String,
    /// The expected outcomes, per leg/check (pre-registered statements).
    pub statements: Vec<String>,
    /// The reference to the full pre-registration (the decision record).
    pub reference: String,
}

/// One nondeterminism control (source + neutralization; the P5.19
/// inventory entry the record carries).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NondeterminismControl {
    pub source: String,
    pub neutralization: String,
}

/// The record's nondeterminism inventory (P5.19: EVERY source listed and
/// neutralized; thread count pinned 1 — the P3.26 solver pin, cited).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NondeterminismControls {
    /// MUST be 1 (the P3.26 solver pin).
    pub thread_count: u32,
    /// The (source, neutralization) pairs — seeded from the checker's
    /// inventory.
    pub controls: Vec<NondeterminismControl>,
}

/// One solver pin (the P3.26 pattern; solver-class discipline travels —
/// AGENTS.md rule 4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SolverPin {
    pub solver: String,
    pub version: String,
    pub license: String,
    /// The solver class (HiGHS = LP/QP/MILP, never SOCP; Clarabel =
    /// continuous conic; the grid leg = AC power flow, never AC-OPF).
    pub class: String,
}

// ---------------------------------------------------------------------------
// Validation (fail-loud — replay never substitutes a missing input)
// ---------------------------------------------------------------------------

impl ScenarioRecordV3 {
    /// Validates the record (all-or-nothing). Every rule violation is a
    /// named [`ReplayError::Config`].
    pub fn validate(&self) -> Result<(), ReplayError> {
        let config = |detail: String| ReplayError::Config(detail);
        if self.schema_version != SCENARIO_RECORD_SCHEMA_VERSION {
            return Err(config(format!(
                "the record names schema_version '{}' but this runner implements \
                 '{}'; a different schema is not silently parsed",
                self.schema_version, SCENARIO_RECORD_SCHEMA_VERSION
            )));
        }
        for (field, value) in [
            ("record_id", self.record_id.as_str()),
            ("record_version", self.record_version.as_str()),
            ("purpose", self.purpose.as_str()),
            ("benchmark_reference", self.benchmark_reference.as_str()),
            ("runtime_class", self.runtime_class.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(config(format!("the record field '{field}' is empty")));
            }
        }
        if !self.synthetic {
            return Err(config(
                "the record is not marked synthetic: a non-synthetic case requires a \
                 named real model with CONFIRMED licensing/availability (§19.3); none \
                 is recorded in this phase — synthetic fixtures only"
                    .to_string(),
            ));
        }
        if self.ieee_case_record.reason.trim().is_empty() {
            return Err(config(
                "the per-case IEEE record carries no reason (every IEEE case is included \
                 or explicitly excluded WITH a recorded reason — the mandatory per-case \
                 record, P5.20)"
                    .to_string(),
            ));
        }
        if self.ieee_case_record.included {
            // An inclusion requires the confirmed basis IN the reason.
            let reason = self.ieee_case_record.reason.to_lowercase();
            if !reason.contains("license") || !reason.contains("confirm") {
                return Err(config(
                    "an IEEE case inclusion must record the CONFIRMED licensing and \
                     model-availability basis in its reason (§19.3; P5.20) — an \
                     unconfirmed inclusion is exactly the failure mode the per-case \
                     record exists to prevent"
                        .to_string(),
                ));
            }
        }
        if !SCALE_CLASSES.contains(&self.scale_class.as_str()) {
            return Err(config(format!(
                "unknown scale_class '{}' (the classes are: {})",
                self.scale_class,
                SCALE_CLASSES.join(", ")
            )));
        }
        // The pinned scenario.
        for (field, value) in [
            ("inputs.scenario.path", self.inputs.scenario.path.as_str()),
            (
                "inputs.scenario.scenario_id",
                self.inputs.scenario.scenario_id.as_str(),
            ),
            ("inputs.scenario.version", self.inputs.scenario.version.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(config(format!("the record field '{field}' is empty")));
            }
        }
        // The forecast fixture, when pinned.
        if let Some(forecast) = &self.inputs.forecast_fixture {
            if forecast.path.trim().is_empty() || forecast.version.trim().is_empty() {
                return Err(config(
                    "the pinned forecast fixture carries an empty path or version"
                        .to_string(),
                ));
            }
        }
        // The grid model pin.
        if !GRID_FIXTURE_KINDS.contains(&self.inputs.grid_model.fixture.as_str()) {
            return Err(config(format!(
                "unknown grid-model fixture '{}' (the named fixtures are: {})",
                self.inputs.grid_model.fixture,
                GRID_FIXTURE_KINDS.join(", ")
            )));
        }
        for (field, value) in [
            (
                "inputs.grid_model.model_id",
                self.inputs.grid_model.model_id.as_str(),
            ),
            (
                "inputs.grid_model.version",
                self.inputs.grid_model.version.as_str(),
            ),
            (
                "inputs.grid_model.statement",
                self.inputs.grid_model.statement.as_str(),
            ),
        ] {
            if value.trim().is_empty() {
                return Err(config(format!("the record field '{field}' is empty")));
            }
        }
        // The objective pin.
        if self.inputs.objective_config.objective.trim().is_empty()
            || self.inputs.objective_config.provenance.trim().is_empty()
        {
            return Err(config(
                "the objective pin is incomplete (objective + provenance are required; \
                 the objective comes from the DECLARED set — never free-text)"
                    .to_string(),
            ));
        }
        // The dispatch pin, when present.
        if let Some(dispatch) = &self.inputs.dispatch {
            if dispatch.asset_key.trim().is_empty() {
                return Err(config("the dispatch pin names no asset_key".to_string()));
            }
            if dispatch.approver.trim().is_empty() || dispatch.approver_role.trim().is_empty()
            {
                return Err(config(
                    "the dispatch pin names no approver (authorization is REQUIRED — no \
                     auto-approve, P4.13)"
                        .to_string(),
                ));
            }
            if FlexUuid::parse(&dispatch.schedule_id).is_err() {
                return Err(config(format!(
                    "the dispatch pin's schedule_id '{}' is not a canonical UUID",
                    dispatch.schedule_id
                )));
            }
            if dispatch.revision == 0 {
                return Err(config("the dispatch pin's revision must be >= 1".to_string()));
            }
            if dispatch.tracking_tolerance_mw < 0.0 || dispatch.tracking_timeout_ms <= 0 {
                return Err(config(
                    "the dispatch pin's tracking criterion is not usable (a negative \
                     tolerance or a non-positive timeout)"
                        .to_string(),
                ));
            }
            if let Some(roll) = &dispatch.roll {
                if FlexUuid::parse(&roll.schedule_id).is_err()
                    || FlexUuid::parse(&roll.candidate_id).is_err()
                {
                    return Err(config(
                        "the roll pin's schedule_id / candidate_id are not canonical UUIDs"
                            .to_string(),
                    ));
                }
                if roll.revision == 0 {
                    return Err(config("the roll pin's revision must be >= 1".to_string()));
                }
            }
        }
        // The clock pin.
        if self.inputs.clock.start_utc.trim().is_empty()
            || self.inputs.clock.cadence_seconds == 0
            || self.inputs.clock.scheduling_interval_seconds == 0
            || self.inputs.clock.horizon_hours == 0
        {
            return Err(config(
                "the clock pin is incomplete (start_utc, cadence_seconds, \
                 scheduling_interval_seconds, horizon_hours are all required; the clock \
                 is the SCENARIO clock — never a wall clock)"
                    .to_string(),
            ));
        }
        // The backend pin.
        if !BACKEND_SELECTIONS.contains(&self.backend.selection.as_str()) {
            return Err(config(format!(
                "unknown backend selection '{}' (the selections are: {})",
                self.backend.selection,
                BACKEND_SELECTIONS.join(", ")
            )));
        }
        if self.backend.provider_pin.trim().is_empty() {
            return Err(config("the backend pin names no provider version".to_string()));
        }
        if self.backend.selection == "worker_process" {
            let Some(worker) = &self.backend.worker else {
                return Err(config(
                    "the worker_process backend pin carries no worker pin (program, \
                     script_path, timeout) — replay never fills a missing pin with a \
                     default"
                        .to_string(),
                ));
            };
            if worker.program.trim().is_empty()
                || worker.script_path.trim().is_empty()
                || worker.timeout_ms == 0
            {
                return Err(config(
                    "the worker pin is incomplete (program, script_path, and a positive \
                     timeout are required; the timeout is fail-closed)"
                        .to_string(),
                ));
            }
        }
        // The seeds.
        if self.seeds.derivation.trim().is_empty() {
            return Err(config(
                "the seed derivation rule is not recorded (D-SC4: the derivation travels \
                 with the seed)"
                    .to_string(),
            ));
        }
        // The tolerances.
        if self.tolerances.telemetry_leg.trim().is_empty()
            || self.tolerances.run_record_leg.trim().is_empty()
            || self.tolerances.table_reference.trim().is_empty()
        {
            return Err(config(
                "the tolerance pin is incomplete (telemetry_leg, run_record_leg, and the \
                 table reference are required; tolerances are pre-registered BEFORE the \
                 runs — never fitted after)"
                    .to_string(),
            ));
        }
        if !self
            .tolerances
            .value_relative_tolerance
            .is_finite()
            || self.tolerances.value_relative_tolerance <= 0.0
            || !self.tolerances.value_absolute_floor.is_finite()
            || self.tolerances.value_absolute_floor < 0.0
        {
            return Err(config(
                "the tolerance values are not usable (a positive finite relative \
                 tolerance and a non-negative finite floor are required)"
                    .to_string(),
            ));
        }
        // The pre-registered expectations.
        if !self.expected_outcomes.pre_registered {
            return Err(config(
                "the record has no pre-registered expected outcomes (the P3.26 \
                 anti-fitting rule: expectations are written BEFORE any run)"
                    .to_string(),
            ));
        }
        if self.expected_outcomes.status.trim().is_empty() {
            return Err(config(
                "the expectation status is empty (UNRUN at authoring; a run updates the \
                 status, never the expectations)"
                    .to_string(),
            ));
        }
        if self.expected_outcomes.statements.is_empty() {
            return Err(config(
                "the expectation slot carries no statements (every leg/check expectation \
                 is pre-registered)"
                    .to_string(),
            ));
        }
        if self.expected_outcomes.reference.trim().is_empty() {
            return Err(config(
                "the expectation slot names no pre-registration reference".to_string(),
            ));
        }
        // The nondeterminism inventory.
        if self.nondeterminism_controls.thread_count != 1 {
            return Err(config(format!(
                "the record's thread_count is {} but the solver pin is thread_count = 1 \
                 (the P3.26 pin; an un-pinned thread count is not reproducible and is \
                 rejected, not defaulted)",
                self.nondeterminism_controls.thread_count
            )));
        }
        if self.nondeterminism_controls.controls.is_empty() {
            return Err(config(
                "the nondeterminism inventory is empty (EVERY source must be listed and \
                 neutralized — wall clock, iteration order, thread count, ...)"
                    .to_string(),
            ));
        }
        for control in &self.nondeterminism_controls.controls {
            if control.source.trim().is_empty() || control.neutralization.trim().is_empty() {
                return Err(config(
                    "a nondeterminism control carries an empty source or neutralization"
                        .to_string(),
                ));
            }
        }
        // The solver pins, when present, carry their classes.
        for pin in &self.solver_pins {
            if pin.solver.trim().is_empty()
                || pin.version.trim().is_empty()
                || pin.class.trim().is_empty()
            {
                return Err(config(
                    "a solver pin is incomplete (solver, version, and class are required \
                     — the class discipline is AGENTS.md rule 4)"
                        .to_string(),
                ));
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// The input hash (the pin-identity tag)
// ---------------------------------------------------------------------------

fn fnv1a64(bytes: &[u8]) -> u64 {
    const OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = OFFSET_BASIS;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

/// Computes the record's input hash: two FNV-1a-64 digests over the
/// canonical JSON of the RUN-INPUT sections (inputs, backend, seeds,
/// tolerances, nondeterminism_controls). The pre-registration text
/// (purpose, benchmark_reference, expected_outcomes) is NOT a run input
/// and is NOT hashed — changing it is a documentation change, not a
/// scenario change. A digest-identity tag, NOT a cryptographic checksum
/// (the repo's recorded FNV derivation discipline).
pub fn input_hash(record: &ScenarioRecordV3) -> Result<String, ReplayError> {
    let inputs = serde_json::to_value(&record.inputs)
        .map_err(|e| ReplayError::Config(format!("the record inputs could not be rendered: {e}")))?;
    let backend = serde_json::to_value(&record.backend)
        .map_err(|e| ReplayError::Config(format!("the record backend could not be rendered: {e}")))?;
    let seeds = serde_json::to_value(&record.seeds)
        .map_err(|e| ReplayError::Config(format!("the record seeds could not be rendered: {e}")))?;
    let tolerances = serde_json::to_value(&record.tolerances).map_err(|e| {
        ReplayError::Config(format!("the record tolerances could not be rendered: {e}"))
    })?;
    let controls = serde_json::to_value(&record.nondeterminism_controls).map_err(|e| {
        ReplayError::Config(format!(
            "the record nondeterminism inventory could not be rendered: {e}"
        ))
    })?;
    // serde_json's default map is ordered, so the rendered form is
    // canonical (sorted keys) — the same document always hashes the same.
    let document = serde_json::json!({
        "inputs": inputs,
        "backend": backend,
        "seeds": seeds,
        "tolerances": tolerances,
        "nondeterminism_controls": controls,
    });
    let text = serde_json::to_string(&document)
        .map_err(|e| ReplayError::Config(format!("the input-hash document failed: {e}")))?;
    let h1 = fnv1a64(text.as_bytes());
    let h2 = fnv1a64(format!("flexcore-sim-runner/input-hash/{text}").as_bytes());
    Ok(format!("{h1:016x}{h2:016x}"))
}

// ---------------------------------------------------------------------------
// Loading (paths resolve relative to the RECORD file's directory)
// ---------------------------------------------------------------------------

/// Loads and validates a scenario record from its JSON file. Returns the
/// record and its directory (the path-resolution base).
pub fn load_scenario_record(path: &str) -> Result<(ScenarioRecordV3, String), ReplayError> {
    let text = std::fs::read_to_string(path).map_err(|e| {
        ReplayError::Io(format!("could not read the scenario record '{path}': {e}"))
    })?;
    let record: ScenarioRecordV3 = serde_json::from_str(&text).map_err(|e| {
        ReplayError::Config(format!(
            "the scenario record '{path}' is not a ScenarioRecord v3: {e}"
        ))
    })?;
    record.validate()?;
    let record_dir = std::path::Path::new(path)
        .parent()
        .map(|parent| parent.to_string_lossy().to_string())
        .unwrap_or_else(|| ".".to_string());
    Ok((record, record_dir))
}

/// Resolves a record-relative input path.
fn resolve_input(record_dir: &str, path: &str) -> String {
    if std::path::Path::new(path).is_absolute() {
        path.to_string()
    } else {
        format!("{record_dir}/{path}")
    }
}

// ---------------------------------------------------------------------------
// The pinned-input verification (the loud-error core, P5.18 step 2)
// ---------------------------------------------------------------------------

/// Reads and verifies the pinned scenario artefact against the record's
/// pins. EVERY mismatch is a named error — replay never substitutes an
/// input, never silently defaults one.
pub fn verify_pinned_inputs(
    record: &ScenarioRecordV3,
    record_dir: &str,
) -> Result<String, ReplayError> {
    let scenario_path = resolve_input(record_dir, &record.inputs.scenario.path);
    let scenario_json = read_scenario_text(&scenario_path)?;

    // The identity pins: scenario id + version EXACTLY.
    let value: Value = serde_json::from_str(&scenario_json).map_err(|e| {
        ReplayError::Scenario(format!("the pinned scenario is not valid JSON: {e}"))
    })?;
    let meta = value
        .get("meta")
        .ok_or_else(|| ReplayError::Scenario("the pinned scenario has no meta section".into()))?;
    let scenario_id = meta
        .get("scenario_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            ReplayError::Scenario("the pinned scenario has no meta.scenario_id".into())
        })?;
    let scenario_version = meta
        .get("version")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ReplayError::Scenario("the pinned scenario has no meta.version".into()))?;
    if scenario_id != record.inputs.scenario.scenario_id {
        return Err(ReplayError::Config(format!(
            "the pinned scenario carries id '{scenario_id}' but the record pins '{}'; \
             replay never substitutes a different scenario",
            record.inputs.scenario.scenario_id
        )));
    }
    if scenario_version != record.inputs.scenario.version {
        return Err(ReplayError::Config(format!(
            "the pinned scenario carries version '{scenario_version}' but the record \
             pins '{}'; replay never substitutes a different version",
            record.inputs.scenario.version
        )));
    }

    // The clock pins.
    let start = value
        .get("horizon")
        .and_then(|h| h.get("start"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| ReplayError::Scenario("the pinned scenario has no horizon.start".into()))?;
    if start != record.inputs.clock.start_utc {
        return Err(ReplayError::Config(format!(
            "the pinned scenario starts at '{start}' but the record pins '{}'; replay \
             never shifts the clock",
            record.inputs.clock.start_utc
        )));
    }
    let duration_hours = value
        .get("horizon")
        .and_then(|h| h.get("duration_hours"))
        .and_then(|v| v.as_u64())
        .ok_or_else(|| {
            ReplayError::Scenario("the pinned scenario has no horizon.duration_hours".into())
        })?;
    if duration_hours != record.inputs.clock.horizon_hours {
        return Err(ReplayError::Config(format!(
            "the pinned scenario runs {duration_hours} h but the record pins {} h",
            record.inputs.clock.horizon_hours
        )));
    }
    let cadence = value
        .get("cadence")
        .and_then(|c| c.get("default_seconds"))
        .and_then(|v| v.as_u64())
        .ok_or_else(|| {
            ReplayError::Scenario("the pinned scenario has no cadence.default_seconds".into())
        })?;
    if cadence != record.inputs.clock.cadence_seconds {
        return Err(ReplayError::Config(format!(
            "the pinned scenario cadence is {cadence} s but the record pins {} s",
            record.inputs.clock.cadence_seconds
        )));
    }

    // The seed pin: the seed is a recorded input; an override does not
    // exist on this path (`scenario_json_with_seed` is deliberately not
    // used here — replay uses ONLY recorded inputs).
    let seed = value
        .get("seeds")
        .and_then(|s| s.get("master_seed"))
        .and_then(|v| v.as_u64())
        .ok_or_else(|| ReplayError::Scenario("the pinned scenario has no master seed".into()))?;
    if seed != record.seeds.master_seed {
        return Err(ReplayError::Config(format!(
            "the pinned scenario's master seed is {seed} but the record pins {}; a seed \
             change is a SCENARIO change — replay never re-seeds a pinned record",
            record.seeds.master_seed
        )));
    }

    // The forecast fixture pin, when present: the file must exist (the
    // consumption itself is the real-solve leg, VERIFY-BUILD — the
    // sim-runner composition does not solve).
    if let Some(forecast) = &record.inputs.forecast_fixture {
        let forecast_path = resolve_input(record_dir, &forecast.path);
        std::fs::read_to_string(&forecast_path).map_err(|e| {
            ReplayError::Io(format!(
                "the pinned forecast fixture '{}' could not be read: {e}",
                forecast.path
            ))
        })?;
    }

    // The full all-or-nothing scenario validation (the P2.13 rule).
    load_scenario(&scenario_json)?;

    Ok(scenario_json)
}

// ---------------------------------------------------------------------------
// The execution (one run from the record)
// ---------------------------------------------------------------------------

/// One executed run: the run record AND the telemetry records (both
/// comparison legs).
#[derive(Debug, Clone, PartialEq)]
pub struct ExecutedRun {
    pub run_record: crate::scenario_run::ScenarioRunRecord,
    pub telemetry: Vec<ReplayRecord>,
}

/// Executes the record ONCE from its pinned inputs (no new inputs; every
/// mismatch with a pin is a loud error). The dispatch slice (when pinned)
/// runs FIRST, then the full horizon, then the record is built and the
/// P4.37 closed loop is planned and recorded.
pub fn execute_run(
    record: &ScenarioRecordV3,
    record_dir: &str,
    code_version: &str,
) -> Result<ExecutedRun, ReplayError> {
    let scenario_json = verify_pinned_inputs(record, record_dir)?;
    let run_record = run_from_verified(record, &scenario_json, code_version)?;
    // The telemetry leg: the deterministic simulator through the same
    // projection the P2.24 replay uses (ingest_time excluded by record
    // shape, P2.13 §3).
    let telemetry = crate::replay::collect_normalized(&scenario_json)?
        .iter()
        .map(record_from_normalized)
        .collect();
    Ok(ExecutedRun {
        run_record,
        telemetry,
    })
}

/// Runs the harness from an already-verified scenario text.
fn run_from_verified(
    record: &ScenarioRecordV3,
    scenario_json: &str,
    code_version: &str,
) -> Result<crate::scenario_run::ScenarioRunRecord, ReplayError> {
    let backend = match record.backend.selection.as_str() {
        "scripted" => GridBackendSelection::Scripted,
        "worker_process" => {
            let worker = record
                .backend
                .worker
                .as_ref()
                .expect("validated: a worker_process selection carries its pin");
            GridBackendSelection::WorkerProcess {
                program: worker.program.clone(),
                script_path: worker.script_path.clone(),
                timeout: std::time::Duration::from_millis(worker.timeout_ms),
            }
        }
        other => {
            return Err(ReplayError::Config(format!(
                "unknown backend selection '{other}'"
            )))
        }
    };
    let fixture = GridFixtureKind::from_name(&record.inputs.grid_model.fixture)
        .map_err(|error| ReplayError::Config(error.to_string()))?;

    // The prior-schedule fixture: supplied ONLY when the dispatch pin
    // carries a roll pin (a recorded dropout then triggers the REAL P3.24
    // rolling roll in the run record). Without a roll pin the prior
    // schedule is None — the roll does not fire, and that is the recorded
    // composition, never a silent default.
    let prior_schedule =
        match record.inputs.dispatch.as_ref().and_then(|dispatch| dispatch.roll.as_ref()) {
            Some(roll) => {
                let setpoints = derive_asset_key_setpoints(scenario_json, roll.setpoint_mw);
                if setpoints.is_empty() {
                    return Err(ReplayError::Config(
                        "the roll pin fixture could not derive the pinned scenario's \
                         asset keys"
                            .to_string(),
                    ));
                }
                Some(PriorScheduleFixture {
                    schedule_id: roll.schedule_id.clone(),
                    revision: roll.revision.max(1),
                    candidate_id: roll.candidate_id.clone(),
                    setpoints_mw: setpoints,
                })
            }
            None => None,
        };

    let harness = ScenarioRunHarness::new_with_grid_fixture(
        scenario_json,
        SchedulingGridConfig {
            interval_millis: record.inputs.clock.scheduling_interval_seconds.max(1) as i64
                * 1_000,
        },
        backend,
        prior_schedule,
    )
    .map_err(|error| ReplayError::Config(error.to_string()))?;

    let mut commands = Vec::new();
    let mut grid_checks = Vec::new();
    let mut command_id_text: Option<String> = None;

    if let Some(dispatch) = &record.inputs.dispatch {
        // The §24 step-6 dispatch slice at the recorded instants (the same
        // derivation the `scenario-run` CLI uses: issue one cadence after
        // the start, poll after the tracking timeout).
        let cadence_millis = harness.scenario().cadence_seconds.max(1) as i64 * 1_000;
        let issue_at = harness.scenario().start_ms + cadence_millis;
        let poll_at = issue_at + dispatch.tracking_timeout_ms.max(cadence_millis);
        let command_config = CommandRunConfig {
            asset_key: dispatch.asset_key.clone(),
            target_p_mw: dispatch.target_p_mw,
            target_q_mvar: dispatch.target_q_mvar,
            tracking_tolerance_mw: dispatch.tracking_tolerance_mw,
            tracking_min_satisfied_samples: dispatch.tracking_min_samples,
            tracking_timeout_millis: dispatch.tracking_timeout_ms,
            approver: dispatch.approver.clone(),
            approver_role: dispatch.approver_role.clone(),
        };
        let command_id = CommandId::new(crate::scenario_run::deterministic_uuid(&format!(
            "flexcore-sim-runner/command/{}",
            harness.scenario().meta.scenario_id
        )));
        command_id_text = Some(command_id.to_string());
        let schedule_uuid = FlexUuid::parse(&dispatch.schedule_id)
            .map_err(|error| {
                ReplayError::Config(format!(
                    "the dispatch pin's schedule_id '{}' is not a canonical UUID: {error}",
                    dispatch.schedule_id
                ))
            })?;
        let schedule_id = ScheduleId::new(schedule_uuid);
        let slice = harness
            .run_dispatch_slice(
                &command_config,
                command_id,
                &schedule_id,
                dispatch.revision.max(1),
                issue_at,
                poll_at,
            )
            .map_err(|error| ReplayError::Config(error.to_string()))?;
        commands.push(slice.command);
        grid_checks.push(slice.grid_check);
    }

    // The full horizon (quality accounting over ALL telemetry).
    harness
        .run_to_horizon_end()
        .map_err(|error| ReplayError::Config(error.to_string()))?;

    // The recorded dropouts come from the scenario's OWN fault plan
    // (configuration, P2.15 — never synthesized).
    let dropouts: Vec<RecordedDropout> = {
        let scenario = harness.scenario();
        if scenario.faults.enabled {
            scenario
                .faults
                .injections
                .iter()
                .filter(|spec| spec.kind == flexcore_simulator::FaultKind::Drop)
                .map(|spec| RecordedDropout {
                    asset_key: spec.asset_key.clone(),
                    from_millis: spec.from_ms,
                })
                .collect()
        } else {
            Vec::new()
        }
    };

    let mut run_record = harness
        .record(code_version, commands, grid_checks, dropouts)
        .map_err(|error| ReplayError::Config(error.to_string()))?;

    // The P4.37 closed loop at the scenario level: route the captured
    // pipeline evidence through the frozen rule and record the decision.
    // The re-scheduling consumer stays the existing P3.24 trigger path
    // (the frozen trigger set is NOT modified — the boundary is recorded
    // ON the summary).
    match (harness.take_closed_loop_observation(), command_id_text) {
        (Some(observation), Some(command_id)) => {
            run_record
                .consumption_decisions
                .push(crate::feasibility_consumption::closed_loop_summary(
                    &observation, &command_id,
                ));
        }
        (Some(observation), None) => {
            // A pipeline ran without a dispatch slice (not reachable via
            // this path today); record it rather than lose it.
            run_record
                .consumption_decisions
                .push(crate::feasibility_consumption::not_routed_summary(
                    &format!("gate:{}", observation.now_millis),
                ));
        }
        (None, Some(command_id)) => {
            // The slice ended before the feasibility gate: record that
            // honestly — no consumption decision is invented.
            run_record
                .consumption_decisions
                .push(crate::feasibility_consumption::not_routed_summary(&command_id));
        }
        (None, None) => {}
    }

    Ok(run_record)
}

/// Derives the prior plan's flat setpoint per asset key (the roll pin's
/// recorded setpoint) from the pinned scenario's asset keys (the same
/// derivation the `scenario-run` CLI uses).
fn derive_asset_key_setpoints(
    scenario_json: &str,
    setpoint_mw: f64,
) -> std::collections::BTreeMap<String, f64> {
    let mut setpoints = std::collections::BTreeMap::new();
    if let Ok(value) = serde_json::from_str::<Value>(scenario_json) {
        if let Some(sites) = value
            .get("portfolio")
            .and_then(|portfolio| portfolio.get("sites"))
            .and_then(|sites| sites.as_array())
        {
            for site in sites {
                if let Some(assets) = site.get("assets").and_then(|assets| assets.as_array()) {
                    for asset in assets {
                        if let Some(key) = asset.get("asset_key").and_then(|key| key.as_str()) {
                            setpoints.insert(key.to_string(), setpoint_mw);
                        }
                    }
                }
            }
        }
    }
    setpoints
}

// ---------------------------------------------------------------------------
// The comparison record (the P5.18/P5.19 output)
// ---------------------------------------------------------------------------

/// One compared leg's outcome, with its NAMED diffs.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LegOutcome {
    /// The leg name (`input_hash` / `telemetry` / `run_record`).
    pub leg: String,
    /// `pass` / `within_tolerance` / `fail` / `not_compared`.
    pub verdict: String,
    /// The number of named differences (mismatches + within-tolerance
    /// deviations).
    pub diff_count: usize,
    /// The named diffs (mismatch paths / records first).
    pub diffs: Vec<String>,
    /// The recorded detail (what was compared, at what rule).
    pub detail: String,
}

/// The ReplayComparisonRecord: input-hash match, per-leg comparison, the
/// named diffs, and the reproducibility verdict.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ReplayComparisonRecord {
    pub record_id: String,
    pub record_version: String,
    pub comparison_kind: String,
    pub scenario_id: String,
    pub scenario_version: String,
    pub master_seed: u64,
    pub code_version: String,
    /// The recomputed input hash over the run-input sections.
    pub input_hash: String,
    /// Whether the recomputed hash matches the record's RECORDED hash
    /// (`true` when the record carries no recorded hash — computed fresh).
    pub input_hash_match: bool,
    pub legs: Vec<LegOutcome>,
    /// The reproducibility verdict (`identical` / `within_tolerance` /
    /// `mismatch`).
    pub verdict: String,
    /// The verdict statement (what the verdict means).
    pub verdict_statement: String,
    /// The tolerance rule the comparison applied (from the record).
    pub tolerance_rule: String,
}

/// Builds the input-hash leg.
fn input_hash_leg(record: &ScenarioRecordV3, recomputed: &str) -> LegOutcome {
    match &record.input_hash {
        Some(recorded) if recorded != recomputed => LegOutcome {
            leg: "input_hash".to_string(),
            verdict: "fail".to_string(),
            diff_count: 1,
            diffs: vec![format!(
                "the record's recorded input hash '{recorded}' does not match the \
                 recomputed '{recomputed}' — the record's pinned inputs changed after \
                 pinning; this is a tamper finding, never a silent re-hash"
            )],
            detail: "the recomputed hash over the run-input sections vs the recorded \
                     hash"
                .to_string(),
        },
        _ => LegOutcome {
            leg: "input_hash".to_string(),
            verdict: "pass".to_string(),
            diff_count: 0,
            diffs: Vec::new(),
            detail: if record.input_hash.is_some() {
                "the recomputed hash matches the recorded hash".to_string()
            } else {
                "the record carries no recorded hash; computed fresh at first pin"
                    .to_string()
            },
        },
    }
}

/// Builds the telemetry leg for two executed runs (byte-exact — the
/// deterministic simulator; no tolerance applies).
fn telemetry_leg(
    reference: &ExecutedRun,
    candidate: &ExecutedRun,
) -> LegOutcome {
    let comparison = crate::compare::compare(&reference.telemetry, &candidate.telemetry);
    let mut diffs = Vec::new();
    for mismatch in comparison.mismatches.iter().take(50) {
        diffs.push(format!(
            "{} {} {} ms seq {}: {}",
            mismatch.asset_id,
            mismatch.quantity,
            mismatch.event_time_unix_ms,
            mismatch
                .reference
                .as_ref()
                .or(mismatch.candidate.as_ref())
                .map(|record| record.source_sequence)
                .unwrap_or(0),
            match mismatch.field {
                crate::compare::DiffField::Value => "value differs",
                crate::compare::DiffField::Quality => "quality differs",
                crate::compare::DiffField::MissingInCandidate => "missing in candidate",
                crate::compare::DiffField::MissingInReference => "missing in reference",
            }
        ));
    }
    if comparison.mismatches.len() > 50 {
        diffs.push(format!(
            "... {} further mismatches (the full list is the comparison's input, not \
             truncated data)",
            comparison.mismatches.len() - 50
        ));
    }
    let verdict = if comparison.is_pass() { "pass" } else { "fail" };
    LegOutcome {
        leg: "telemetry".to_string(),
        verdict: verdict.to_string(),
        diff_count: comparison.mismatches.len(),
        diffs,
        detail: format!(
            "the frozen P2.24 tuple (asset_id, event_time, quantity, value, quality, \
             source_sequence); ingest_time excluded by record shape (P2.13 §3); \
             byte-exact ({})",
            reference.telemetry.len()
        ),
    }
}

/// Builds the run-record leg from two run records (structural JSON diff at
/// the record's tolerance).
fn run_record_leg(
    reference: &crate::scenario_run::ScenarioRunRecord,
    candidate: &crate::scenario_run::ScenarioRunRecord,
    relative: f64,
    floor: f64,
) -> Result<LegOutcome, ReplayError> {
    let reference_value = serde_json::to_value(reference)
        .map_err(|e| ReplayError::Config(format!("the reference run record failed to render: {e}")))?;
    let candidate_value = serde_json::to_value(candidate)
        .map_err(|e| ReplayError::Config(format!("the candidate run record failed to render: {e}")))?;
    let mut diffs = Vec::new();
    structural_diff(
        &reference_value,
        &candidate_value,
        "",
        relative,
        floor,
        &mut diffs,
    );
    let (mismatches, within): (Vec<&crate::reproducibility::StructuralDiff>, Vec<_>) = diffs
        .iter()
        .partition(|diff| diff.kind == "mismatch");
    let mut named = Vec::new();
    for diff in mismatches.iter().take(50) {
        named.push(format!(
            "{}: reference {} vs candidate {}",
            diff.path, diff.reference, diff.candidate
        ));
    }
    if mismatches.len() > 50 {
        named.push(format!(
            "... {} further mismatches",
            mismatches.len() - 50
        ));
    }
    for diff in within.iter().take(20) {
        named.push(format!(
            "within tolerance {}: reference {} vs candidate {}",
            diff.path, diff.reference, diff.candidate
        ));
    }
    let verdict = if mismatches.is_empty() {
        if within.is_empty() {
            VERDICT_IDENTICAL
        } else {
            VERDICT_WITHIN_TOLERANCE
        }
    } else {
        VERDICT_MISMATCH
    };
    Ok(LegOutcome {
        leg: "run_record".to_string(),
        verdict: verdict.to_string(),
        diff_count: diffs.len(),
        diffs: named,
        detail: format!(
            "the ScenarioRunRecord JSON, structural diff (mismatch paths named; numeric \
             deviations within the relative tolerance {relative:e} recorded, floor \
             {floor:e})"
        ),
    })
}

/// Builds the full comparison for two executed runs of the SAME record
/// (the `double_run` kind — the P5.19 proof shape).
pub fn compare_double_run(
    record: &ScenarioRecordV3,
    run_a: &ExecutedRun,
    run_b: &ExecutedRun,
    code_version: &str,
) -> Result<ReplayComparisonRecord, ReplayError> {
    let recomputed = input_hash(record)?;
    let input_leg = input_hash_leg(record, &recomputed);
    let telemetry = telemetry_leg(run_a, run_b);
    let run_record = run_record_leg(
        &run_a.run_record,
        &run_b.run_record,
        record.tolerances.value_relative_tolerance,
        record.tolerances.value_absolute_floor,
    )?;
    finish_comparison(
        record,
        COMPARISON_DOUBLE_RUN,
        code_version,
        recomputed,
        input_leg,
        vec![telemetry, run_record?],
    )
}

/// Builds the comparison against a STORED baseline run-record JSON (the
/// `baseline` kind). The telemetry leg is honestly `not_compared` (a run
/// record does not carry the telemetry records).
pub fn compare_against_baseline(
    record: &ScenarioRecordV3,
    fresh: &ExecutedRun,
    baseline_json: &str,
    code_version: &str,
) -> Result<ReplayComparisonRecord, ReplayError> {
    let recomputed = input_hash(record)?;
    let input_leg = input_hash_leg(record, &recomputed);
    let baseline_value: Value = serde_json::from_str(baseline_json).map_err(|e| {
        ReplayError::Config(format!(
            "the baseline run record is not a ScenarioRunRecord JSON: {e}"
        ))
    })?;
    let fresh_value = serde_json::to_value(&fresh.run_record)
        .map_err(|e| ReplayError::Config(format!("the fresh run record failed to render: {e}")))?;
    let mut diffs = Vec::new();
    structural_diff(
        &baseline_value,
        &fresh_value,
        "",
        record.tolerances.value_relative_tolerance,
        record.tolerances.value_absolute_floor,
        &mut diffs,
    );
    let mismatches: Vec<&crate::reproducibility::StructuralDiff> = diffs
        .iter()
        .filter(|diff| diff.kind == "mismatch")
        .collect();
    let within: Vec<&crate::reproducibility::StructuralDiff> = diffs
        .iter()
        .filter(|diff| diff.kind == "within_tolerance")
        .collect();
    let mut named = Vec::new();
    for diff in mismatches.iter().take(50) {
        named.push(format!(
            "{}: baseline {} vs candidate {}",
            diff.path, diff.reference, diff.candidate
        ));
    }
    if mismatches.len() > 50 {
        named.push(format!("... {} further mismatches", mismatches.len() - 50));
    }
    for diff in within.iter().take(20) {
        named.push(format!(
            "within tolerance {}: baseline {} vs candidate {}",
            diff.path, diff.reference, diff.candidate
        ));
    }
    let verdict = if mismatches.is_empty() {
        if within.is_empty() {
            VERDICT_IDENTICAL
        } else {
            VERDICT_WITHIN_TOLERANCE
        }
    } else {
        VERDICT_MISMATCH
    };
    let run_record_leg_outcome = LegOutcome {
        leg: "run_record".to_string(),
        verdict: verdict.to_string(),
        diff_count: diffs.len(),
        diffs: named,
        detail: format!(
            "the fresh run record vs the STORED baseline run record, structural diff at \
             relative {} / floor {}",
            record.tolerances.value_relative_tolerance,
            record.tolerances.value_absolute_floor
        ),
    };
    let telemetry_leg_outcome = LegOutcome {
        leg: "telemetry".to_string(),
        verdict: "not_compared".to_string(),
        diff_count: 0,
        diffs: Vec::new(),
        detail: "the baseline run record does not carry the telemetry records; compare \
                 telemetry with `double_run` or with the `replay` subcommand against a \
                 stored records file (the `run --out` form)"
            .to_string(),
    };
    finish_comparison(
        record,
        COMPARISON_BASELINE,
        code_version,
        recomputed,
        input_leg,
        vec![telemetry_leg_outcome, run_record_leg_outcome],
    )
}

/// Assembles the comparison record and computes the verdict.
fn finish_comparison(
    record: &ScenarioRecordV3,
    kind: &str,
    code_version: &str,
    recomputed: String,
    input_leg: LegOutcome,
    legs: Vec<LegOutcome>,
) -> Result<ReplayComparisonRecord, ReplayError> {
    let mut all_legs = Vec::with_capacity(legs.len() + 1);
    all_legs.push(input_leg);
    all_legs.extend(legs);

    // The checker verdict (P5.19): any failing leg ⇒ mismatch; any
    // within-tolerance deviation ⇒ within_tolerance; else identical. A
    // `not_compared` leg contributes neither.
    let leg_tuples: Vec<(&str, usize, usize, Vec<String>)> = all_legs
        .iter()
        .map(|leg| {
            let mismatches = if leg.verdict == "fail" || leg.verdict == VERDICT_MISMATCH {
                leg.diff_count.max(1)
            } else {
                0
            };
            let within = if leg.verdict == VERDICT_WITHIN_TOLERANCE {
                leg.diff_count.max(1)
            } else {
                0
            };
            (leg.leg.as_str(), mismatches, within, leg.diffs.clone())
        })
        .collect();
    let tolerance_rule = format!(
        "telemetry: {}, run record: {}, relative {}, floor {} (pre-registered in the \
         record: {})",
        record.tolerances.telemetry_leg,
        record.tolerances.run_record_leg,
        record.tolerances.value_relative_tolerance,
        record.tolerances.value_absolute_floor,
        record.tolerances.table_reference
    );
    let verdict = check_reproducibility(&leg_tuples, &tolerance_rule);

    Ok(ReplayComparisonRecord {
        record_id: record.record_id.clone(),
        record_version: record.record_version.clone(),
        comparison_kind: kind.to_string(),
        scenario_id: record.inputs.scenario.scenario_id.clone(),
        scenario_version: record.inputs.scenario.version.clone(),
        master_seed: record.seeds.master_seed,
        code_version: code_version.to_string(),
        input_hash: recomputed,
        input_hash_match: all_legs[0].verdict == "pass",
        legs: all_legs,
        verdict: verdict.verdict,
        verdict_statement: verdict.statement,
        tolerance_rule,
    })
}

// ---------------------------------------------------------------------------
// Unit tests (the schema + hash + comparison core; the scenario-level
// execution is exercised in tests/scenario_replay_p5_18.rs)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_record() -> ScenarioRecordV3 {
        serde_json::from_str(
            r#"{
            "schema_version": "3",
            "record_id": "test-record",
            "record_version": "v1",
            "synthetic": true,
            "purpose": "the schema round-trip fixture",
            "benchmark_reference": "§19.3: synthetic portfolios with BESS, PV, flexible load, and EVSE",
            "runtime_class": "seconds-class on the scripted backend; NOT a measured figure",
            "scale_class": "minimal",
            "inputs": {
                "scenario": {"path": "fixtures/scenario.json", "scenario_id": "s", "version": "v1"},
                "grid_model": {"fixture": "three_bus_radial", "model_id": "feeder-a", "version": "1", "statement": "synthetic"},
                "objective_config": {"objective": "SYNTHETIC_ENERGY_COST_DEVIATION_V1", "provenance": "SYNTHETIC", "note": "declared set"},
                "clock": {"start_utc": "2026-06-01T00:00:00Z", "cadence_seconds": 60, "scheduling_interval_seconds": 900, "horizon_hours": 24}
            },
            "backend": {"selection": "scripted", "provider_pin": "pandapower@3.5.6"},
            "seeds": {"master_seed": 20260601, "derivation": "seed(asset_key) = master_seed XOR splitmix64(FNV-1a-64(asset_key))"},
            "tolerances": {"telemetry_leg": "byte_identical", "run_record_leg": "byte_identical", "value_relative_tolerance": 1e-9, "value_absolute_floor": 1e-12, "table_reference": "18_20_replay_library.md"},
            "expected_outcomes": {"pre_registered": true, "status": "UNRUN", "statements": ["load passes"], "reference": "18_20_replay_library.md"},
            "nondeterminism_controls": {"thread_count": 1, "controls": [{"source": "wall_clock", "neutralization": "scenario clock only"}]},
            "solver_pins": [{"solver": "HiGHS", "version": "1.15.1", "license": "MIT", "class": "LP/QP/MILP (never SOCP)"}],
            "ieee_case_record": {"included": false, "reason": "excluded: no IEEE model is available and licensing is unconfirmed (synthetic fixtures only)"}
        }"#,
        )
        .expect("the fixture parses")
    }

    #[test]
    fn a_valid_minimal_record_round_trips_through_json() {
        let record = minimal_record();
        record.validate().expect("the minimal record is valid");
        let text = serde_json::to_string_pretty(&record).unwrap();
        let parsed: ScenarioRecordV3 = serde_json::from_str(&text).unwrap();
        assert_eq!(parsed, record);
    }

    #[test]
    fn the_input_hash_is_stable_and_covers_the_run_inputs() {
        let record = minimal_record();
        let hash_a = input_hash(&record).unwrap();
        let hash_b = input_hash(&record).unwrap();
        assert_eq!(hash_a, hash_b, "the same record hashes the same");
        let mut perturbed = record.clone();
        perturbed.seeds.master_seed = 20260602;
        let hash_c = input_hash(&perturbed).unwrap();
        assert_ne!(hash_a, hash_c, "a seed change changes the input hash");
        let mut perturbed = record.clone();
        perturbed.backend.selection = "worker_process".to_string();
        perturbed.backend.worker = Some(WorkerPin {
            program: "python".to_string(),
            script_path: "apps/worker/python/grid_worker.py".to_string(),
            timeout_ms: 30_000,
        });
        let hash_d = input_hash(&perturbed).unwrap();
        assert_ne!(hash_a, hash_d, "a backend change changes the input hash");
    }

    #[test]
    fn a_wrong_schema_version_is_rejected_not_parsed() {
        let mut record = minimal_record();
        record.schema_version = "2".to_string();
        let error = record.validate().unwrap_err();
        assert!(error.to_string().contains("schema_version"));
    }

    #[test]
    fn a_worker_selection_without_a_worker_pin_is_rejected() {
        let mut record = minimal_record();
        record.backend.selection = "worker_process".to_string();
        let error = record.validate().unwrap_err();
        assert!(error.to_string().contains("worker pin"));
        record.backend.worker = Some(WorkerPin {
            program: "python".to_string(),
            script_path: "apps/worker/python/grid_worker.py".to_string(),
            timeout_ms: 30_000,
        });
        record.validate().expect("the pin completes the selection");
    }

    #[test]
    fn an_unpinned_thread_count_is_rejected() {
        let mut record = minimal_record();
        record.nondeterminism_controls.thread_count = 4;
        let error = record.validate().unwrap_err();
        assert!(error.to_string().contains("thread_count"));
    }

    #[test]
    fn missing_pre_registered_expectations_are_rejected() {
        let mut record = minimal_record();
        record.expected_outcomes.pre_registered = false;
        let error = record.validate().unwrap_err();
        assert!(error.to_string().contains("pre-registered"));
        let mut record = minimal_record();
        record.expected_outcomes.statements.clear();
        assert!(record.validate().is_err());
    }

    #[test]
    fn an_unknown_fixture_or_backend_name_is_rejected() {
        let mut record = minimal_record();
        record.inputs.grid_model.fixture = "ieee13bus".to_string();
        assert!(record.validate().is_err());
        let mut record = minimal_record();
        record.backend.selection = "gRPC".to_string();
        assert!(record.validate().is_err());
    }

    #[test]
    fn the_input_hash_leg_reports_a_recorded_hash_mismatch_as_a_tamper_finding() {
        let mut record = minimal_record();
        record.input_hash = Some("deadbeefdeadbeefdeadbeefdeadbeef".to_string());
        let recomputed = input_hash(&record).unwrap();
        let leg = input_hash_leg(&record, &recomputed);
        assert_eq!(leg.verdict, "fail");
        assert!(leg.diffs[0].contains("tamper finding"));
        // Matching recorded hash passes; absent recorded hash passes with
        // the fresh-computation note.
        record.input_hash = Some(recomputed.clone());
        assert_eq!(input_hash_leg(&record, &recomputed).verdict, "pass");
        record.input_hash = None;
        assert_eq!(input_hash_leg(&record, &recomputed).verdict, "pass");
    }
}
