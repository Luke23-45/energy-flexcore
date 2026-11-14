// P2.24 — `flexcore-sim-runner`: the deterministic-replay CLI (composition
// root of the replay entrypoint).
//
// The recorded P2.24 entrypoint decision (ledger step 2;
// `ledger/phase-02-asset-telemetry/23_24_fault_replay.md`): replay is a
// DISTINCT entrypoint from background ingestion, so it is this minimal CLI,
// not a worker task. Two subcommands:
//
//     flexcore-sim-runner run --scenario <path> [--seed <n>] [--out <path>]
//         Executes the scenario version + seed through the deterministic
//         simulator and emits the canonical replay records (JSON array) to
//         `--out` or stdout. This output is the reference-file form.
//
//     flexcore-sim-runner replay --scenario <path> --reference <path>
//                               [--seed <n>] [--code-version <string>]
//                               [--report <path>]
//         Re-runs the scenario and compares against the reference records
//         file on the frozen comparison tuple
//         (asset_id, event_time, quantity, value, quality, source_sequence) —
//         never on ingest_time (P2.13 §3). Emits the replay report (JSON,
//         plus the human summary on stdout) to `--report` or stdout.
//
//     flexcore-sim-runner as-of --versions <path> [--as-of <unix-ms>]
//                              --decision-input <path> [--report <path>]
//         The P2.31 as-of (point-in-time) replay: re-derives the decision's
//         input snapshot from the versioned record history (a versions file
//         exported from stored telemetry; see the P2.31 runbook) at the
//         as-of instant, and compares it against the decision-input records
//         file on the frozen comparison tuple. Without --as-of the latest
//         version view is compared. Same exit-code contract.
//
//     flexcore-sim-runner benchmark --scenario <path> [--scaling <n,n>]
//                                   [--out <path>] [--code-version <string>]
//                                   [--require-complete]
//         The P3.27 benchmark: runs the scenario (and, with --scaling, the
//         deterministic ×2/×4 portfolio copies) through the replay pipeline
//         and emits one BenchmarkRecord JSON per portfolio size — runtime,
//         the §19.4 measured fields (unmeasurable ones recorded
//         `not_measured`, fail-loud, never a default zero), and the
//         hardware/container description. The figures are the v0.2.0
//         BASELINE for later comparison, never targets (report §19.4: no
//         arbitrary universal targets). A benchmark failure is structural
//         (bad record / --require-complete with unmeasured fields) — never
//         a runtime threshold.
//
//     flexcore-sim-runner scenario-run --scenario <path>
//                                      [--grid-backend <scripted|worker>]
//                                      [--out <path>] [run options]
//         The P5.14/P5.15 scenario-run composition (report §24 slice,
//         §19.1): telemetry ingest → schedule-fixture dispatch → grid
//         feasibility → issue/acknowledge/track against the REAL Phase 2
//         simulator and the REAL Phase 4 adapter chain, and emit the
//         ScenarioRunRecord JSON (adapter inventory real-vs-seam, versions,
//         quality accounting, availability changes, dropout roll, command
//         and grid-check outcomes). The grid backend is selectable:
//         `scripted` (the seam-level in-process fixture, the default) or
//         `worker` (the P5.15 Python worker process — written, never
//         executed in this loop; UNVERIFIED pandapower API).
//
// Exit-code contract (recorded, P2.24 step 4): replay pass → 0, replay
// mismatch → 1, usage/config error → 2. CI's P2.27 scenario-regression
// gate asserts the report's VERDICT, not just the exit code.
//
// Code version (P2.13 §4 — replay names scenario version + seed + code
// version): `--code-version`, else the `FLEXCORE_CODE_VERSION` environment
// variable, else the literal `dev-unrecorded` (an unrecorded code version
// is stated, never faked).
//
// Stored-telemetry reference mode: recorded as the follow-up wiring in
// `23_24_fault_replay.md` — the comparison is record-based, so reading the
// reference through the `TelemetryReader` port (once the Postgres read
// adapter, P2.19, is stable) maps onto `compare` without a redesign. In
// Phase 2 the CLI compares against the records file, which keeps the
// dependency surface free of database/broker infrastructure.
//
// Everything this binary produces is synthetic, simulation-only telemetry
// (the entire 0.x line): no field read, no field write, no control
// authority, no safety or certification implication.

use std::collections::BTreeMap;
use std::process::ExitCode;
use std::time::Duration;

use flexcore_contracts::ids::FlexUuid;
use flexcore_domain::ids::{CommandId, ScheduleId};
use flexcore_simulator::FaultKind;
use flexcore_sim_runner::benchmark::{self, BenchmarkRecord};
use flexcore_sim_runner::helics_orchestration::{
    compose_record, execute_federation, parse_federation_config, execution_record,
    HelicsCoSimRecord, ProcessFederateLauncher,
};
use flexcore_sim_runner::fault_injection::{
    build_fault_journal, run_fault_probes, ProbeOutcome, ScenarioFaultPlan,
};
use flexcore_sim_runner::load_run::{self, LoadSpec};
use flexcore_sim_runner::metrics::{legs, LegTimer, ScenarioRunTimings};
use flexcore_sim_runner::scenario_run::{
    CommandRunConfig, GridBackendSelection, PriorScheduleFixture, RecordedDropout,
    ScenarioRunHarness, SchedulingGridConfig,
};
use flexcore_sim_runner::{as_of, compare, report, replay};
use report::{
    exit_code_for_verdict, ReplayReport, EXIT_MISMATCH, EXIT_PASS, EXIT_USAGE,
};
use replay::ReplayError;

const USAGE: &str = "\
flexcore-sim-runner — deterministic scenario replay (P2.24), as-of replay (P2.31),
and benchmarking (P3.27)

USAGE:
    flexcore-sim-runner run --scenario <path> [--seed <n>] [--out <path>]
    flexcore-sim-runner replay --scenario <path> --reference <path>
                               [--seed <n>] [--code-version <string>]
                               [--report <path>]
    flexcore-sim-runner as-of --versions <path> [--as-of <unix-ms>]
                              --decision-input <path> [--report <path>]
    flexcore-sim-runner benchmark --scenario <path> [--scaling <n,n>]
                                  [--out <path>] [--code-version <string>]
                                  [--require-complete]
    flexcore-sim-runner scenario-run --scenario <path>
                                     [--grid-backend <scripted|worker>]
                                     [--out <path>] [run options]
    flexcore-sim-runner replay-run --record <path> [--baseline <path>]
                                   [--double-run] [--out <path>]
                                   [--report <path>] [--code-version <string>]
    flexcore-sim-runner load-run [--ingest-scenario <path> --ingest-iterations <n>]
                                 [--query-url <url> --query-count <n>]
                                 [--out <path>] [--code-version <string>]

SUBCOMMANDS:
    run        Execute the scenario (version + seed) and emit the canonical
               replay records as a JSON array (to --out, or stdout).
    replay     Re-run the scenario and compare against the reference records
               file; emit the replay report (JSON, plus a human summary on
               stdout; the report JSON goes to --report when given).
    as-of      Re-derive a past decision's input snapshot from the versioned
               record history at the as-of instant and compare it against the
               decision-input records file (P2.31); report JSON + human
               summary, --report when given.
    benchmark  Run the scenario (and with --scaling the deterministic ×2/×4
               portfolio copies) through the replay pipeline and emit one
               BenchmarkRecord JSON per portfolio size (P3.27). Records carry
               the runtime, the §19.4 measured fields (unmeasurable ones are
               `not_measured`, never a default zero), the hardware/container
               description, and the scaling-copy spec. The figures are the
               v0.2.0 BASELINE for later comparison — never targets (§19.4).
    scenario-run
               Run the full P5.14/P5.15 scenario composition against the
               REAL simulator and the REAL Phase 4 adapter chain (§24
               steps 5–8; telemetry ingest → dispatch → grid feasibility →
               tracking) and emit the ScenarioRunRecord JSON, which names
               every adapter real-vs-seam and every version (P5.14/P5.15).
               The record carries the P5.21/P5.22 fault journal (the
               scenario's fault configuration — the scenario_faults v3
               slot or the P2.15 fault plan) and, with --fault-probes,
               the composition-level fault probes (clock skew, partial
               fleet) are executed and recorded factually. With P5.23 the
               per-leg wall-clock timings are measured and attached.
    load-run   Execute the P5.24 declared load — N telemetry-ingest
               iterations through the real replay pipeline and/or M
               sequential HTTP GET round trips against a configured
               endpoint — and emit one LoadMeasurementRecord JSON. A
               single sequential client (no concurrency model); console
               browser-side figures are NOT measured (manual procedure,
               24_load_measurement.md). The figures are the v0.3.0 load
               BASELINE inputs, never targets (§19.4).
    replay-run Execute a ScenarioRecord v3 (a PINNED, replayable record,
               P5.18) from its recorded inputs ONLY — any input that
               cannot be reproduced from the record is an error, never a
               silent default — and emit the ReplayComparisonRecord JSON:
               the input-hash match, the per-leg comparisons with NAMED
               diffs, and the reproducibility verdict (P5.19). With
               --double-run the record executes twice in-process and BOTH
               legs compare (the P5.19 proof shape; the proof runs are the
               human's pre-registered run — never executed by the
               authoring loop). With --baseline the fresh run compares
               against a STORED run-record JSON (the telemetry leg is
               honestly `not_compared` there). Exit 0 identical/within
               tolerance, 1 mismatch, 2 usage/config error.

OPTIONS:
    --scenario <path>      The versioned scenario JSON artefact.
    --reference <path>     Reference records file (a previous `run --out`).
    --versions <path>      The versioned record history (the canonical record
                           shape plus the correction metadata columns; the
                           SQL export is in the P2.31 runbook).
    --as-of <unix-ms>      The as-of instant in Unix milliseconds (optional;
                           absent = the latest version view).
    --decision-input <path>  The records file the past decision actually used.
    --scaling <n,n>        Comma-separated portfolio-size factors for the
                           scaling probe (e.g. '2,4'); each factor adds one
                           BenchmarkRecord to the output. Factors must be >= 2
                           here (1, the base scenario, is always measured).
    --seed <n>             Override the scenario's master seed (a seed
                           change is a scenario change for replay purposes:
                           a tampered-seed replay fails with a diff).
    --code-version <s>     The code version the run names (default: the
                           FLEXCORE_CODE_VERSION environment, else
                           'dev-unrecorded').
    --out <path>           Write the run's records JSON here (default stdout).
    --report <path>        Write the report JSON here (default stdout).
    --require-complete     (benchmark only) Fail (exit 1) if any §19.4
                           measured field is `not_measured` — fail-loud,
                           never a default zero.

LOAD-RUN OPTIONS (P5.24):
    --ingest-scenario <path>  The scenario the telemetry-ingest leg runs
                           (N sequential replay-pipeline iterations).
    --ingest-iterations <n> Iteration count for the ingest leg (>= 1).
    --query-url <url>      The http:// endpoint the query leg probes (M
                           sequential GET round trips; https is a named
                           error, never a silent fallback).
    --query-count <n>      Round-trip count for the query leg (>= 1).

SCENARIO-RUN OPTIONS (P5.14/P5.15):
    --grid-backend <s>     `scripted` (default: the seam-level in-process
                           fixture — NOT a computed power flow) or `worker`
                           (the P5.15 Python worker process; its pandapower
                           3.5.6 API usage is UNVERIFIED until the human
                           build/test pass executes it). The run record
                           names which backend ran.
    --worker-program <p>   Interpreter to spawn for `worker` (default
                           `python`).
    --worker-script <p>    Worker entrypoint (default
                           apps/worker/python/grid_worker.py).
    --worker-timeout-ms <n> Bounded worker wait (default 30000). A timeout
                           is fail-closed, never a pass.
    --interval-minutes <n> The scheduling-grid interval length in minutes
                           (default 15).
    --dispatch-asset <key> The asset the §24 step-6 command targets
                           (default: the scenario's first asset key).
    --target-p-mw <f>      The command's target active power, export
                           positive (default 0.0).
    --target-q-mvar <f>    The command's target reactive power (default 0.0).
    --tracking-tolerance-mw <f>   The tracking acceptance tolerance (D-P4.16-3
                           deployment configuration; default 0.5).
    --tracking-min-samples <n>    Minimum satisfied samples (default 1).
    --tracking-timeout-ms <n>     Tracking timeout (default 1800000 = 30 min).
    --approver <s>         The authorizing identity (default
                           scenario-operator).
    --approver-role <s>    The authorizing role — must match the
                           composition's authorization seam
                           (default scenario-approver).
    --schedule-id <uuid>   The schedule revision the command is created
                           FROM (default a fixed composition fixture UUID).
    --revision <n>         The schedule revision number (default 1).
    --roll-schedule-id <uuid>  When set (with --roll-candidate-id), a prior
                           schedule fixture is supplied so a recorded
                           dropout triggers the REAL P3.24 rolling roll in
                           the run record.
    --roll-revision <n>    The prior revision number (default 1).
    --roll-candidate-id <uuid>  The prior revision's §12.6 candidate id.
    --roll-setpoint-mw <f> The prior plan's flat setpoint per asset
                           (default 0.0).

REPLAY-RUN OPTIONS (P5.18/P5.19):
    --record <path>        The ScenarioRecord v3 JSON (the pinned,
                           replayable record). Scenario/forecast paths in
                           the record resolve relative to the RECORD
                           file's directory; worker script paths are
                           repo-root-relative (the CLI convention).
    --baseline <path>      A STORED ScenarioRunRecord JSON (a previous
                           `scenario-run --out` artefact) to compare the
                           fresh run against. Required unless
                           --double-run is given.
    --double-run           Execute the record TWICE in-process and compare
                           both legs (telemetry + run record) — the P5.19
                           proof shape. The proof runs are the human's
                           pre-registered run; the authoring loop never
                           executes them.
    --out <path>           Write the fresh run's ScenarioRunRecord JSON
                           here (default stdout).
    --report <path>        Write the ReplayComparisonRecord JSON here
                           (default stdout).

HELICS OPTIONS (P5.16 — opt-in; the DEFAULT scenario runs WITHOUT HELICS,
direct adapters):
    --helics-federation <path>  The federation config JSON (scenario-level
                           versioned data; validated fail-closed by the
                           orchestrator BEFORE anything is composed).
                           When absent, the run is a direct-adapter run
                           and no co-sim record is emitted.
    --helics-execute        Actually launch the federation (broker + the
                           federates, concurrently, bounded timeout). The
                           federate scripts are UNVERIFIED-API: helics
                           3.7.0 was never installed in this loop; every
                           failure (spawn, timeout, contract, failed
                           report) is recorded FAIL-CLOSED in the run
                           record — never a pass, never a retry.
    --helics-timeout-ms <n> The bounded per-process wait (default
                           120000). A timeout kills the process and is
                           fail-closed.
    --helics-broker-program <p>  The interpreter for the broker script
                           (default: the config's broker.program).

EXIT CODES: 0 replay pass / benchmark record produced / scenario run record
produced / replay-run identical or within tolerance; 1 replay mismatch /
benchmark completeness or structural failure / replay-run mismatch;
2 usage/config error.
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = run_cli(&args);
    ExitCode::from(code as u8)
}

/// The CLI body, separated from `main` so the exit-code contract is one
/// return value. Only ever returns 0, 1, or 2.
fn run_cli(args: &[String]) -> i32 {
    match parse_command(args) {
        Ok(Command::Run(options)) => run_command(&options),
        Ok(Command::Replay(options)) => replay_command(&options),
        Ok(Command::AsOf(options)) => as_of_command(&options),
        Ok(Command::Benchmark(options)) => benchmark_command(&options),
        Ok(Command::ScenarioRun(options)) => scenario_run_command(&options),
        Ok(Command::ReplayRun(options)) => replay_run_command(&options),
        Ok(Command::LoadRun(options)) => load_run_command(&options),
        Err(CliError::Usage) => {
            eprint!("{USAGE}");
            EXIT_USAGE
        }
        Err(CliError::Failed(error)) => {
            eprintln!("error: {error}");
            EXIT_USAGE
        }
    }
}

// --- Argument parsing -----------------------------------------------------------

struct RunOptions {
    scenario_path: String,
    seed: Option<u64>,
    out: Option<String>,
}

struct ReplayOptions {
    scenario_path: String,
    reference_path: String,
    seed: Option<u64>,
    code_version: String,
    report_path: Option<String>,
}

struct AsOfOptions {
    versions_path: String,
    as_of_unix_ms: Option<i64>,
    decision_input_path: String,
    report_path: Option<String>,
}

struct BenchmarkOptions {
    scenario_path: String,
    scaling_factors: Vec<u32>,
    out_path: Option<String>,
    code_version: String,
    require_complete: bool,
}

/// The P5.14/P5.15 `scenario-run` options. Every dispatch parameter is
/// deployment configuration (D-P4.16-3): an option, never a hard-coded
/// constant; the defaults are the recorded composition fixtures.
struct ScenarioRunOptions {
    scenario_path: String,
    seed: Option<u64>,
    code_version: String,
    out_path: Option<String>,
    grid_backend: String,
    worker_program: Option<String>,
    worker_script: Option<String>,
    worker_timeout_ms: Option<u64>,
    interval_minutes: u64,
    dispatch_asset: Option<String>,
    target_p_mw: f64,
    target_q_mvar: f64,
    tracking_tolerance_mw: f64,
    tracking_min_samples: u32,
    tracking_timeout_ms: i64,
    approver: String,
    approver_role: String,
    schedule_id: String,
    revision: u64,
    roll_schedule_id: Option<String>,
    roll_revision: u64,
    roll_candidate_id: Option<String>,
    roll_setpoint_mw: f64,
    // P5.16 (opt-in HELICS co-simulation; absent = direct adapters).
    helics_federation: Option<String>,
    helics_execute: bool,
    helics_timeout_ms: u64,
    helics_broker_program: Option<String>,
    // P5.21 (opt-in composition-level fault probes: the clock-skew probe
    // and the partial-fleet probe for the slot-declared specs). The fault
    // JOURNAL itself is always built; only the PROBES are opt-in.
    fault_probes: bool,
}

/// The P5.24 `load-run` options (the declared load; validated by
/// `LoadSpec::validate` at execution time — fail-loud, exit 2).
struct LoadRunOptions {
    ingest_scenario: Option<String>,
    ingest_iterations: u32,
    query_url: Option<String>,
    query_count: u32,
    out_path: Option<String>,
    code_version: String,
}

enum Command {
    Run(RunOptions),
    Replay(ReplayOptions),
    AsOf(AsOfOptions),
    Benchmark(BenchmarkOptions),
    ScenarioRun(ScenarioRunOptions),
    ReplayRun(ReplayRunOptions),
    LoadRun(LoadRunOptions),
}

enum CliError {
    Usage,
    Failed(ReplayError),
}

fn flag_value(args: &[String], index: usize, flag: &str) -> Result<String, CliError> {
    args.get(index + 1)
        .cloned()
        .ok_or_else(|| CliError::Failed(ReplayError::Config(format!("{flag} requires a value"))))
}

/// Parses the argument list; anything unrecognised is a usage error (exit
/// code 2), never a silent ignore.
fn parse_command(args: &[String]) -> Result<Command, CliError> {
    let Some(command) = args.first() else {
        return Err(CliError::Usage);
    };
    match command.as_str() {
        "run" => parse_run(args),
        "replay" => parse_replay(args),
        "as-of" => parse_as_of(args),
        "benchmark" => parse_benchmark(args),
        "scenario-run" => parse_scenario_run(args),
        "replay-run" => parse_replay_run(args),
        "load-run" => parse_load_run(args),
        "--help" | "-h" | "help" => Err(CliError::Usage),
        other => Err(CliError::Failed(ReplayError::Config(format!(
            "unknown subcommand '{other}' (expected 'run', 'replay', 'as-of', 'benchmark', \
             'scenario-run', 'replay-run', or 'load-run')"
        )))),
    }
}

/// The fixed composition fixture schedule UUID (a deterministic stand-in,
/// recorded in the decision doc `14_15_adapters.md`; NOT a registry-issued
/// ID). The roll candidate id is supplied explicitly via
/// `--roll-candidate-id` when a rolling-roll fixture is configured.
const SCENARIO_RUN_SCHEDULE_UUID: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7ab1";

/// Parses the `scenario-run` arguments. Every value is validated here
/// (fail-fast, exit 2 on misuse); numeric flags carry their recorded
/// defaults.
fn parse_scenario_run(args: &[String]) -> Result<Command, CliError> {
    let mut options = ScenarioRunOptions {
        scenario_path: String::new(),
        seed: None,
        code_version: String::new(),
        out_path: None,
        grid_backend: "scripted".to_string(),
        worker_program: None,
        worker_script: None,
        worker_timeout_ms: None,
        interval_minutes: 15,
        dispatch_asset: None,
        target_p_mw: 0.0,
        target_q_mvar: 0.0,
        tracking_tolerance_mw: 0.5,
        tracking_min_samples: 1,
        tracking_timeout_ms: 1_800_000,
        approver: "scenario-operator".to_string(),
        approver_role: "scenario-approver".to_string(),
        schedule_id: SCENARIO_RUN_SCHEDULE_UUID.to_string(),
        revision: 1,
        roll_schedule_id: None,
        roll_revision: 1,
        roll_candidate_id: None,
        roll_setpoint_mw: 0.0,
        helics_federation: None,
        helics_execute: false,
        helics_timeout_ms: 120_000,
        helics_broker_program: None,
        fault_probes: false,
    };
    let mut index = 1;
    while index < args.len() {
        let flag = args[index].as_str();
        // The valueless P5.21 flag is handled before the fixed-stride
        // match (the same pattern parse_benchmark uses for
        // --require-complete).
        if flag == "--fault-probes" {
            options.fault_probes = true;
            index += 1;
            continue;
        }
        match flag {
            "--scenario" => options.scenario_path = flag_value(args, index, "--scenario")?,
            "--out" => options.out_path = Some(flag_value(args, index, "--out")?),
            "--code-version" => options.code_version = flag_value(args, index, "--code-version")?,
            "--grid-backend" => options.grid_backend = flag_value(args, index, "--grid-backend")?,
            "--worker-program" => {
                options.worker_program = Some(flag_value(args, index, "--worker-program")?)
            }
            "--worker-script" => {
                options.worker_script = Some(flag_value(args, index, "--worker-script")?)
            }
            "--worker-timeout-ms" => {
                options.worker_timeout_ms = Some(parse_u64_flag(
                    args,
                    index,
                    "--worker-timeout-ms",
                )?)
            }
            "--interval-minutes" => {
                options.interval_minutes = parse_u64_flag(args, index, "--interval-minutes")?
            }
            "--dispatch-asset" => {
                options.dispatch_asset = Some(flag_value(args, index, "--dispatch-asset")?)
            }
            "--target-p-mw" => {
                options.target_p_mw = parse_f64_flag(args, index, "--target-p-mw")?
            }
            "--target-q-mvar" => {
                options.target_q_mvar = parse_f64_flag(args, index, "--target-q-mvar")?
            }
            "--tracking-tolerance-mw" => {
                options.tracking_tolerance_mw = parse_f64_flag(
                    args,
                    index,
                    "--tracking-tolerance-mw",
                )?
            }
            "--tracking-min-samples" => {
                options.tracking_min_samples = parse_u64_flag(
                    args,
                    index,
                    "--tracking-min-samples",
                )? as u32
            }
            "--tracking-timeout-ms" => {
                options.tracking_timeout_ms = parse_u64_flag(
                    args,
                    index,
                    "--tracking-timeout-ms",
                )? as i64
            }
            "--approver" => options.approver = flag_value(args, index, "--approver")?,
            "--approver-role" => options.approver_role = flag_value(args, index, "--approver-role")?,
            "--schedule-id" => options.schedule_id = flag_value(args, index, "--schedule-id")?,
            "--revision" => options.revision = parse_u64_flag(args, index, "--revision")?,
            "--roll-schedule-id" => {
                options.roll_schedule_id = Some(flag_value(args, index, "--roll-schedule-id")?)
            }
            "--roll-revision" => {
                options.roll_revision = parse_u64_flag(args, index, "--roll-revision")?
            }
            "--roll-candidate-id" => {
                options.roll_candidate_id = Some(flag_value(args, index, "--roll-candidate-id")?)
            }
            "--roll-setpoint-mw" => {
                options.roll_setpoint_mw = parse_f64_flag(args, index, "--roll-setpoint-mw")?
            }
            "--helics-federation" => {
                options.helics_federation = Some(flag_value(args, index, "--helics-federation")?)
            }
            "--helics-execute" => {
                options.helics_execute = true;
            }
            "--helics-timeout-ms" => {
                options.helics_timeout_ms = parse_u64_flag(args, index, "--helics-timeout-ms")?
            }
            "--helics-broker-program" => {
                options.helics_broker_program = Some(flag_value(
                    args,
                    index,
                    "--helics-broker-program",
                )?)
            }
            "--seed" => {
                options.seed = Some(parse_u64_flag(args, index, "--seed")?);
            }
            other => {
                return Err(CliError::Failed(ReplayError::Config(format!(
                    "unknown option '{other}' for 'scenario-run'"
                ))))
            }
        }
        index += 2;
    }
    if options.scenario_path.is_empty() {
        return Err(CliError::Failed(ReplayError::Config(
            "'scenario-run' requires --scenario <path>".to_string(),
        )));
    }
    if options.grid_backend == "worker" && options.worker_program.is_none() {
        // The default interpreter name is applied at composition time.
        options.worker_program = Some("python".to_string());
    }
    if options.code_version.is_empty() {
        options.code_version = std::env::var("FLEXCORE_CODE_VERSION")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| "dev-unrecorded".to_string());
    }
    Ok(Command::ScenarioRun(options))
}

/// The P5.18/P5.19 `replay-run` options.
struct ReplayRunOptions {
    record_path: String,
    baseline_path: Option<String>,
    double_run: bool,
    out_path: Option<String>,
    report_path: Option<String>,
    code_version: String,
}

/// Parses the `replay-run` arguments. `--double-run` is a valueless flag,
/// so this parser owns its loop instead of the fixed +2 stride. Exactly
/// one of `--double-run` / `--baseline` must be given — an ambiguous
/// comparison mode is a usage error, never a guess.
fn parse_replay_run(args: &[String]) -> Result<Command, CliError> {
    let mut options = ReplayRunOptions {
        record_path: String::new(),
        baseline_path: None,
        double_run: false,
        out_path: None,
        report_path: None,
        code_version: String::new(),
    };
    let mut index = 1;
    while index < args.len() {
        match args[index].as_str() {
            "--record" => options.record_path = flag_value(args, index, "--record")?,
            "--baseline" => {
                options.baseline_path = Some(flag_value(args, index, "--baseline")?)
            }
            "--double-run" => {
                options.double_run = true;
                index += 1;
                continue;
            }
            "--out" => options.out_path = Some(flag_value(args, index, "--out")?),
            "--report" => options.report_path = Some(flag_value(args, index, "--report")?),
            "--code-version" => options.code_version = flag_value(args, index, "--code-version")?,
            other => {
                return Err(CliError::Failed(ReplayError::Config(format!(
                    "unknown option '{other}' for 'replay-run'"
                ))))
            }
        }
        index += 2;
    }
    if options.record_path.is_empty() {
        return Err(CliError::Failed(ReplayError::Config(
            "'replay-run' requires --record <path> (a ScenarioRecord v3 JSON)".to_string(),
        )));
    }
    if options.double_run && options.baseline_path.is_some() {
        return Err(CliError::Failed(ReplayError::Config(
            "'replay-run' takes exactly one of --double-run or --baseline (the \
             comparison mode must be unambiguous)"
                .to_string(),
        )));
    }
    if !options.double_run && options.baseline_path.is_none() {
        return Err(CliError::Failed(ReplayError::Config(
            "'replay-run' requires --double-run (the in-process proof shape) or \
             --baseline <path> (a stored run record)"
                .to_string(),
        )));
    }
    if options.code_version.is_empty() {
        options.code_version = std::env::var("FLEXCORE_CODE_VERSION")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| "dev-unrecorded".to_string());
    }
    Ok(Command::ReplayRun(options))
}

fn parse_u64_flag(args: &[String], index: usize, flag: &str) -> Result<u64, CliError> {
    let raw = flag_value(args, index, flag)?;
    raw.parse::<u64>().map_err(|_| {
        CliError::Failed(ReplayError::Config(format!(
            "{flag} expects a non-negative integer, got '{raw}'"
        )))
    })
}

fn parse_f64_flag(args: &[String], index: usize, flag: &str) -> Result<f64, CliError> {
    let raw = flag_value(args, index, flag)?;
    raw.parse::<f64>().map_err(|_| {
        CliError::Failed(ReplayError::Config(format!(
            "{flag} expects a finite number, got '{raw}'"
        )))
    })
}

fn parse_run(args: &[String]) -> Result<Command, CliError> {
    let mut options = RunOptions {
        scenario_path: String::new(),
        seed: None,
        out: None,
    };
    let mut index = 1;
    while index < args.len() {
        match args[index].as_str() {
            "--scenario" => options.scenario_path = flag_value(args, index, "--scenario")?,
            "--seed" => {
                let raw = flag_value(args, index, "--seed")?;
                options.seed = Some(raw.parse::<u64>().map_err(|_| {
                    CliError::Failed(ReplayError::Config(format!(
                        "--seed expects an unsigned integer, got '{raw}'"
                    )))
                })?);
            }
            "--out" => options.out = Some(flag_value(args, index, "--out")?),
            other => {
                return Err(CliError::Failed(ReplayError::Config(format!(
                    "unknown option '{other}' for 'run'"
                ))))
            }
        }
        index += 2;
    }
    if options.scenario_path.is_empty() {
        return Err(CliError::Failed(ReplayError::Config(
            "'run' requires --scenario <path>".to_string(),
        )));
    }
    Ok(Command::Run(options))
}

fn parse_replay(args: &[String]) -> Result<Command, CliError> {
    let mut options = ReplayOptions {
        scenario_path: String::new(),
        reference_path: String::new(),
        seed: None,
        code_version: String::new(),
        report_path: None,
    };
    let mut index = 1;
    while index < args.len() {
        match args[index].as_str() {
            "--scenario" => options.scenario_path = flag_value(args, index, "--scenario")?,
            "--reference" => options.reference_path = flag_value(args, index, "--reference")?,
            "--report" => options.report_path = Some(flag_value(args, index, "--report")?),
            "--seed" => {
                let raw = flag_value(args, index, "--seed")?;
                options.seed = Some(raw.parse::<u64>().map_err(|_| {
                    CliError::Failed(ReplayError::Config(format!(
                        "--seed expects an unsigned integer, got '{raw}'"
                    )))
                })?);
            }
            "--code-version" => options.code_version = flag_value(args, index, "--code-version")?,
            other => {
                return Err(CliError::Failed(ReplayError::Config(format!(
                    "unknown option '{other}' for 'replay'"
                ))))
            }
        }
        index += 2;
    }
    if options.scenario_path.is_empty() {
        return Err(CliError::Failed(ReplayError::Config(
            "'replay' requires --scenario <path>".to_string(),
        )));
    }
    if options.reference_path.is_empty() {
        return Err(CliError::Failed(ReplayError::Config(
            "'replay' requires --reference <path> (a records file from a previous run)"
                .to_string(),
        )));
    }
    if options.code_version.is_empty() {
        options.code_version = std::env::var("FLEXCORE_CODE_VERSION")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| "dev-unrecorded".to_string());
    }
    Ok(Command::Replay(options))
}

fn parse_as_of(args: &[String]) -> Result<Command, CliError> {
    let mut options = AsOfOptions {
        versions_path: String::new(),
        as_of_unix_ms: None,
        decision_input_path: String::new(),
        report_path: None,
    };
    let mut index = 1;
    while index < args.len() {
        match args[index].as_str() {
            "--versions" => options.versions_path = flag_value(args, index, "--versions")?,
            "--decision-input" => {
                options.decision_input_path = flag_value(args, index, "--decision-input")?
            }
            "--report" => options.report_path = Some(flag_value(args, index, "--report")?),
            "--as-of" => {
                let raw = flag_value(args, index, "--as-of")?;
                options.as_of_unix_ms = Some(raw.parse::<i64>().map_err(|_| {
                    CliError::Failed(ReplayError::Config(format!(
                        "--as-of expects a Unix-milliseconds integer, got '{raw}'"
                    )))
                })?);
            }
            other => {
                return Err(CliError::Failed(ReplayError::Config(format!(
                    "unknown option '{other}' for 'as-of'"
                ))))
            }
        }
        index += 2;
    }
    if options.versions_path.is_empty() {
        return Err(CliError::Failed(ReplayError::Config(
            "'as-of' requires --versions <path> (the versioned record history)".to_string(),
        )));
    }
    if options.decision_input_path.is_empty() {
        return Err(CliError::Failed(ReplayError::Config(
            "'as-of' requires --decision-input <path> (the records file the decision used)"
                .to_string(),
        )));
    }
    Ok(Command::AsOf(options))
}

/// Parses the `benchmark` arguments. `--require-complete` is a valueless
/// flag, so this parser owns its loop instead of the fixed +2 stride.
fn parse_benchmark(args: &[String]) -> Result<Command, CliError> {
    let mut options = BenchmarkOptions {
        scenario_path: String::new(),
        scaling_factors: Vec::new(),
        out_path: None,
        code_version: String::new(),
        require_complete: false,
    };
    let mut index = 1;
    while index < args.len() {
        match args[index].as_str() {
            "--scenario" => options.scenario_path = flag_value(args, index, "--scenario")?,
            "--out" => options.out_path = Some(flag_value(args, index, "--out")?),
            "--code-version" => options.code_version = flag_value(args, index, "--code-version")?,
            "--require-complete" => {
                options.require_complete = true;
                index += 1;
                continue;
            }
            "--scaling" => {
                let raw = flag_value(args, index, "--scaling")?;
                for part in raw.split(',') {
                    let part = part.trim();
                    let factor = part.parse::<u32>().map_err(|_| {
                        CliError::Failed(ReplayError::Config(format!(
                            "--scaling expects comma-separated integers >= 2, got '{part}'"
                        )))
                    })?;
                    if factor < 2 {
                        return Err(CliError::Failed(ReplayError::Config(format!(
                            "--scaling factors must be >= 2 (the base scenario, factor 1, \
                             is always measured); got {factor}"
                        ))));
                    }
                    options.scaling_factors.push(factor);
                }
            }
            other => {
                return Err(CliError::Failed(ReplayError::Config(format!(
                    "unknown option '{other}' for 'benchmark'"
                ))))
            }
        }
        index += 2;
    }
    if options.scenario_path.is_empty() {
        return Err(CliError::Failed(ReplayError::Config(
            "'benchmark' requires --scenario <path>".to_string(),
        )));
    }
    if options.code_version.is_empty() {
        options.code_version = std::env::var("FLEXCORE_CODE_VERSION")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| "dev-unrecorded".to_string());
    }
    Ok(Command::Benchmark(options))
}

/// Parses the `load-run` arguments (P5.24). The declared load is validated
/// at execution time by `LoadSpec::validate` (at least one leg, iteration
/// counts >= 1, http:// only) — a misconfigured load is exit 2, never a
/// silently empty record.
fn parse_load_run(args: &[String]) -> Result<Command, CliError> {
    let mut options = LoadRunOptions {
        ingest_scenario: None,
        ingest_iterations: 0,
        query_url: None,
        query_count: 0,
        out_path: None,
        code_version: String::new(),
    };
    let mut index = 1;
    while index < args.len() {
        match args[index].as_str() {
            "--ingest-scenario" => {
                options.ingest_scenario = Some(flag_value(args, index, "--ingest-scenario")?)
            }
            "--ingest-iterations" => {
                options.ingest_iterations = parse_u64_flag(args, index, "--ingest-iterations")?
                    as u32
            }
            "--query-url" => options.query_url = Some(flag_value(args, index, "--query-url")?),
            "--query-count" => {
                options.query_count = parse_u64_flag(args, index, "--query-count")? as u32
            }
            "--out" => options.out_path = Some(flag_value(args, index, "--out")?),
            "--code-version" => options.code_version = flag_value(args, index, "--code-version")?,
            other => {
                return Err(CliError::Failed(ReplayError::Config(format!(
                    "unknown option '{other}' for 'load-run'"
                ))))
            }
        }
        index += 2;
    }
    if options.code_version.is_empty() {
        options.code_version = std::env::var("FLEXCORE_CODE_VERSION")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| "dev-unrecorded".to_string());
    }
    Ok(Command::LoadRun(options))
}

// --- The subcommands --------------------------------------------------------------

/// `run`: scenario (+ optional seed override) → canonical records JSON.
fn run_command(options: &RunOptions) -> i32 {
    let scenario_json = match replay::read_scenario_text(&options.scenario_path) {
        Ok(text) => text,
        Err(error) => return fail(error),
    };
    let scenario_json = match options.seed {
        Some(seed) => match replay::scenario_json_with_seed(&scenario_json, seed) {
            Ok(text) => text,
            Err(error) => return fail(error),
        },
        None => scenario_json,
    };
    let run = match replay::run_scenario(&scenario_json) {
        Ok(run) => run,
        Err(error) => return fail(error),
    };
    let text = match replay::records_to_json(&run.records) {
        Ok(text) => text,
        Err(error) => return fail(error),
    };
    match &options.out {
        Some(path) => {
            if let Err(error) = replay::write_file(path, &text) {
                return fail(error);
            }
            println!(
                "run complete: scenario '{}:{}', seed {}, {} records written to {path}",
                run.scenario_id,
                run.scenario_version,
                run.master_seed,
                run.records.len()
            );
        }
        None => {
            println!("{text}");
        }
    }
    0
}

/// `replay`: re-run + compare + report, with the recorded exit codes.
fn replay_command(options: &ReplayOptions) -> i32 {
    let scenario_json = match replay::read_scenario_text(&options.scenario_path) {
        Ok(text) => text,
        Err(error) => return fail(error),
    };
    let scenario_json = match options.seed {
        Some(seed) => match replay::scenario_json_with_seed(&scenario_json, seed) {
            Ok(text) => text,
            Err(error) => return fail(error),
        },
        None => scenario_json,
    };
    let run = match replay::run_scenario(&scenario_json) {
        Ok(run) => run,
        Err(error) => return fail(error),
    };
    let reference = match replay::read_records_file(&options.reference_path) {
        Ok(records) => records,
        Err(error) => return fail(error),
    };
    let comparison = compare::compare(&reference, &run.records);
    let report = ReplayReport::build(
        &run.scenario_id,
        &run.scenario_version,
        run.master_seed,
        &options.code_version,
        comparison,
    );
    let json = match report.to_json() {
        Ok(json) => json,
        Err(error) => {
            return fail(ReplayError::Io(format!(
                "the replay report could not be serialized: {error}"
            )))
        }
    };
    match &options.report_path {
        Some(path) => {
            if let Err(error) = replay::write_file(path, &json) {
                return fail(error);
            }
        }
        None => {
            println!("{json}");
        }
    }
    print!("{}", report.human_summary());
    exit_code_for_verdict(&report.verdict)
}

/// Reports a usage/config/execution error with exit code 2.
fn fail(error: ReplayError) -> i32 {
    eprintln!("error: {error}");
    EXIT_USAGE
}

/// `as-of`: derive the decision's input snapshot from the versioned history
/// at the as-of instant and compare it against the decision-input file, with
/// the recorded exit codes.
fn as_of_command(options: &AsOfOptions) -> i32 {
    let report = match as_of::run_as_of(
        &options.versions_path,
        options.as_of_unix_ms,
        &options.decision_input_path,
    ) {
        Ok(report) => report,
        Err(error) => return fail(error),
    };
    let json = match report.to_json() {
        Ok(json) => json,
        Err(error) => {
            return fail(ReplayError::Io(format!(
                "the as-of report could not be serialized: {error}"
            )))
        }
    };
    match &options.report_path {
        Some(path) => {
            if let Err(error) = replay::write_file(path, &json) {
                return fail(error);
            }
        }
        None => {
            println!("{json}");
        }
    }
    print!("{}", report.human_summary());
    as_of::exit_code(&report)
}

/// `benchmark`: scenario (+ optional scaling probe) → one BenchmarkRecord
/// JSON per portfolio size, with the P3.27 exit-code contract: 0 records
/// produced, 1 completeness/structural failure, 2 usage/config error. The
/// records are the v0.2.0 baseline (report §19.4) — no runtime threshold is
/// evaluated here, ever.
fn benchmark_command(options: &BenchmarkOptions) -> i32 {
    let scenario_json = match replay::read_scenario_text(&options.scenario_path) {
        Ok(text) => text,
        Err(error) => return fail(error),
    };

    // Factor 1 (the base scenario) is always measured, then the probe
    // factors in the order given — document order, so the output array is
    // deterministic for a given invocation.
    let mut factors = vec![1u32];
    factors.extend(options.scaling_factors.iter().copied());

    let mut records: Vec<BenchmarkRecord> = Vec::with_capacity(factors.len());
    for factor in factors {
        let record = if factor == 1 {
            benchmark::BenchmarkRecord::measure_base(&scenario_json, &options.code_version)
        } else {
            benchmark::BenchmarkRecord::measure_scaling(&scenario_json, factor, &options.code_version)
        };
        match record {
            Ok(record) => records.push(record),
            Err(error) => return fail(error),
        }
    }

    // Fail-loud completeness: with --require-complete, any not_measured
    // §19.4 field is a failure naming the field — never a default zero.
    if options.require_complete {
        for record in &records {
            if let Err(unmeasured) = record.validate_complete() {
                eprintln!(
                    "error: benchmark record for scenario '{}' (factor {}) is INCOMPLETE; \
                     not_measured fields: {} — no default zero is ever substituted",
                    record.scenario_pin.scenario_id,
                    record.scaling.portfolio_factor,
                    unmeasured.join(", ")
                );
                return EXIT_MISMATCH;
            }
        }
    }

    let json = match serde_json::to_string_pretty(&records) {
        Ok(json) => json,
        Err(error) => {
            return fail(ReplayError::Io(format!(
                "the benchmark records could not be serialized: {error}"
            )))
        }
    };
    match &options.out_path {
        Some(path) => {
            if let Err(error) = replay::write_file(path, &json) {
                return fail(error);
            }
        }
        None => {
            println!("{json}");
        }
    }
    for record in &records {
        println!(
            "benchmark record: scenario '{}', portfolio ×{}, replay runtime {:?}, \
             complete={} (unmeasured: {})",
            record.scenario_pin.scenario_id,
            record.scaling.portfolio_factor,
            record.measured.replay_runtime.value(),
            record.completeness.complete,
            if record.completeness.unmeasured.is_empty() {
                "none".to_string()
            } else {
                record.completeness.unmeasured.join(", ")
            }
        );
    }
    EXIT_PASS
}

/// `replay-run` (P5.18/P5.19): executes a ScenarioRecord v3 from its
/// PINNED inputs (any input that cannot be reproduced from the record is
/// an error, never a silent default) and emits the ReplayComparisonRecord
/// JSON — the input-hash match, the per-leg comparisons with NAMED diffs,
/// and the reproducibility verdict. Exit-code contract: identical or
/// within-tolerance verdict → 0, mismatch → 1, usage/config error → 2.
///
/// The `--double-run` proof shape executes the record twice in-process;
/// the authoring loop never executed it (the Phase 5 loop bans hold) —
/// the proof runs are the human's pre-registered run, and the verdict of
/// a REAL double run is the P5.19 acceptance evidence.
fn replay_run_command(options: &ReplayRunOptions) -> i32 {
    use flexcore_sim_runner::replay_record;

    let (record, record_dir) = match replay_record::load_scenario_record(&options.record_path) {
        Ok(pair) => pair,
        Err(error) => return fail(error),
    };

    let fresh = match replay_record::execute_run(&record, &record_dir, &options.code_version) {
        Ok(executed) => executed,
        Err(error) => return fail(error),
    };

    let comparison = if options.double_run {
        let second = match replay_record::execute_run(&record, &record_dir, &options.code_version)
        {
            Ok(executed) => executed,
            Err(error) => return fail(error),
        };
        match replay_record::compare_double_run(&record, &fresh, &second, &options.code_version) {
            Ok(comparison) => comparison,
            Err(error) => return fail(error),
        }
    } else {
        let baseline_path = options
            .baseline_path
            .as_ref()
            .expect("the parser required exactly one of --double-run / --baseline");
        let baseline_json =
            match std::fs::read_to_string(baseline_path) {
                Ok(text) => text,
                Err(error) => {
                    return fail(ReplayError::Io(format!(
                        "could not read the baseline run record '{baseline_path}': {error}"
                    )))
                }
            };
        match replay_record::compare_against_baseline(
            &record,
            &fresh,
            &baseline_json,
            &options.code_version,
        ) {
            Ok(comparison) => comparison,
            Err(error) => return fail(error),
        }
    };

    // The fresh run record (the artefact a future --baseline consumes).
    let run_json = match serde_json::to_string_pretty(&fresh.run_record) {
        Ok(json) => json,
        Err(error) => {
            return fail(ReplayError::Io(format!(
                "the run record could not be serialized: {error}"
            )))
        }
    };
    match &options.out_path {
        Some(path) => {
            if let Err(error) = replay::write_file(path, &run_json) {
                return fail(error);
            }
        }
        None => {
            println!("{run_json}");
        }
    }

    let json = match serde_json::to_string_pretty(&comparison) {
        Ok(json) => json,
        Err(error) => {
            return fail(ReplayError::Io(format!(
                "the comparison record could not be serialized: {error}"
            )))
        }
    };
    match &options.report_path {
        Some(path) => {
            if let Err(error) = replay::write_file(path, &json) {
                return fail(error);
            }
        }
        None => {
            println!("{json}");
        }
    }
    for leg in &comparison.legs {
        println!(
            "leg [{}] {}: {} diff(s) — {}",
            leg.leg, leg.verdict, leg.diff_count, leg.detail
        );
        for diff in leg.diffs.iter().take(10) {
            println!("  {diff}");
        }
    }
    println!(
        "replay-run verdict: {} — scenario '{}:{}', seed {}, record '{}:{}' ({})\n{}",
        comparison.verdict,
        comparison.scenario_id,
        comparison.scenario_version,
        comparison.master_seed,
        comparison.record_id,
        comparison.record_version,
        comparison.comparison_kind,
        comparison.verdict_statement
    );
    match comparison.verdict.as_str() {
        flexcore_sim_runner::reproducibility::VERDICT_IDENTICAL
        | flexcore_sim_runner::reproducibility::VERDICT_WITHIN_TOLERANCE => EXIT_PASS,
        flexcore_sim_runner::reproducibility::VERDICT_MISMATCH => EXIT_MISMATCH,
        other => {
            eprintln!("error: unknown reproducibility verdict '{other}' (fail-closed — a \
                       verdict spelling bug must not be able to green a gate)");
            EXIT_USAGE
        }
    }
}

/// `load-run` (P5.24): executes the declared load (telemetry-ingest
/// iterations and/or query round trips) and emits one LoadMeasurementRecord
/// JSON. Exit 0 on a produced record, 2 on a configuration/measurement
/// error — a failed measurement is never recorded as success. No runtime
/// threshold is evaluated anywhere (report §19.4: the figures are the
/// v0.3.0 load baseline inputs for later comparison, never targets).
fn load_run_command(options: &LoadRunOptions) -> i32 {
    let spec = LoadSpec {
        ingest_scenario: options.ingest_scenario.clone(),
        ingest_iterations: options.ingest_iterations,
        query_url: options.query_url.clone(),
        query_count: options.query_count,
    };
    let record = match load_run::run_load(&spec, &options.code_version) {
        Ok(record) => record,
        Err(error) => return fail(ReplayError::Config(error.to_string())),
    };
    let json = match record.to_json() {
        Ok(json) => json,
        Err(error) => {
            return fail(ReplayError::Io(format!(
                "the load measurement record could not be serialized: {error}"
            )))
        }
    };
    match &options.out_path {
        Some(path) => {
            if let Err(error) = replay::write_file(path, &json) {
                return fail(error);
            }
        }
        None => {
            println!("{json}");
        }
    }
    // The honest stdout summary: every measured figure and every explicit
    // not_measured with its reason — nothing hidden, nothing averaged.
    let results = &record.results;
    for (name, field) in [
        ("telemetry_ingest_rate", &results.telemetry_ingest_rate),
        (
            "telemetry_ingest_iteration_p95_ms",
            &results.telemetry_ingest_iteration_p95_ms,
        ),
        (
            "telemetry_ingest_iteration_max_ms",
            &results.telemetry_ingest_iteration_max_ms,
        ),
        ("query_latency_p50_ms", &results.query_latency_p50_ms),
        ("query_latency_p95_ms", &results.query_latency_p95_ms),
        ("query_latency_p99_ms", &results.query_latency_p99_ms),
        ("command_fan_out_time", &results.command_fan_out_time),
        (
            "recovery_time_after_restart",
            &results.recovery_time_after_restart,
        ),
    ] {
        match field.value() {
            Some(value) => println!("load: {name} = {value} (measured)"),
            None => println!("load: {name} = not_measured"),
        }
    }
    for note in &record.scope_notes {
        println!("scope: {note}");
    }
    EXIT_PASS
}

/// `scenario-run` (P5.14/P5.15): the full scenario composition against the
/// REAL Phase 2 simulator and the REAL Phase 4 adapter chain, emitting the
/// ScenarioRunRecord JSON. Exit 0 on a produced record (whatever verdicts
/// the REAL gates reached — a rejection is an honest outcome, not a
/// failure); exit 2 on composition/config errors.
///
/// Order matters and is recorded: the dispatch slice runs FIRST (issue
/// inside the horizon, tracking poll after), then the horizon completes so
/// the record's quality/availability accounting covers the FULL telemetry
/// history. Nothing here reads the wall clock; every instant is the
/// scenario clock.
fn scenario_run_command(options: &ScenarioRunOptions) -> i32 {
    let scenario_json = match replay::read_scenario_text(&options.scenario_path) {
        Ok(text) => text,
        Err(error) => return fail(error),
    };
    let scenario_json = match options.seed {
        Some(seed) => match replay::scenario_json_with_seed(&scenario_json, seed) {
            Ok(text) => text,
            Err(error) => return fail(error),
        },
        None => scenario_json,
    };

    // The grid backend selection (the run record names it; P5.15).
    let grid_backend = match options.grid_backend.as_str() {
        "scripted" => GridBackendSelection::Scripted,
        "worker" => GridBackendSelection::WorkerProcess {
            program: options
                .worker_program
                .clone()
                .unwrap_or_else(|| "python".to_string()),
            script_path: options
                .worker_script
                .clone()
                .unwrap_or_else(|| "apps/worker/python/grid_worker.py".to_string()),
            timeout: Duration::from_millis(options.worker_timeout_ms.unwrap_or(30_000)),
        },
        other => {
            return fail(ReplayError::Config(format!(
                "--grid-backend expects 'scripted' or 'worker', got '{other}'"
            )))
        }
    };

    // The prior-schedule fixture (when the operator supplied one), so a
    // recorded dropout triggers the REAL P3.24 rolling roll.
    let prior_schedule = match (&options.roll_schedule_id, &options.roll_candidate_id) {
        (Some(schedule_id), Some(candidate_id)) => {
            let scenario_text = match serde_json::from_str::<serde_json::Value>(&scenario_json) {
                Ok(value) => value,
                Err(error) => {
                    return fail(ReplayError::Config(format!(
                        "the scenario is not valid JSON: {error}"
                    )))
                }
            };
            let mut setpoints = BTreeMap::new();
            if let Some(sites) = scenario_text
                .get("portfolio")
                .and_then(|portfolio| portfolio.get("sites"))
                .and_then(|sites| sites.as_array())
            {
                for site in sites {
                    if let Some(assets) = site.get("assets").and_then(|assets| assets.as_array()) {
                        for asset in assets {
                            if let Some(key) = asset.get("asset_key").and_then(|key| key.as_str()) {
                                setpoints.insert(key.to_string(), options.roll_setpoint_mw);
                            }
                        }
                    }
                }
            }
            if setpoints.is_empty() {
                return fail(ReplayError::Config(
                    "the roll fixture could not derive the scenario's asset keys"
                        .to_string(),
                ));
            }
            Some(PriorScheduleFixture {
                schedule_id: schedule_id.clone(),
                revision: options.roll_revision.max(1),
                candidate_id: candidate_id.clone(),
                setpoints_mw: setpoints,
            })
        }
        _ => None,
    };

    let harness = match ScenarioRunHarness::new(
        &scenario_json,
        SchedulingGridConfig {
            interval_millis: (options.interval_minutes.max(1) as i64) * 60_000,
        },
        grid_backend,
        prior_schedule,
    ) {
        Ok(harness) => harness,
        Err(error) => return fail(ReplayError::Config(error.to_string())),
    };

    // The recorded dropouts come from the scenario's OWN fault plan
    // (configuration, P2.15 — never synthesized here). The dispatch-slice
    // asset must exist; the target is an option, never a hard-coded value.
    let dropouts: Vec<RecordedDropout> = {
        let scenario = harness.scenario();
        if scenario.faults.enabled {
            scenario
                .faults
                .injections
                .iter()
                .filter(|spec| spec.kind == FaultKind::Drop)
                .map(|spec| RecordedDropout {
                    asset_key: spec.asset_key.clone(),
                    from_millis: spec.from_ms,
                })
                .collect()
        } else {
            Vec::new()
        }
    };

    // The §24 step-6 dispatch slice: issue one cadence after the start,
    // poll after the tracking timeout. The target asset defaults to the
    // scenario's FIRST asset key (deterministic; the scenario file pins it).
    let dispatch_asset = options
        .dispatch_asset
        .clone()
        .unwrap_or_else(|| harness.scenario().sites[0].assets[0].asset_key.clone());
    let cadence_millis = harness.scenario().cadence_seconds.max(1) as i64 * 1_000;
    let issue_at = harness.scenario().start_ms + cadence_millis;
    let poll_at = issue_at + options.tracking_timeout_ms.max(cadence_millis);
    let command_config = CommandRunConfig {
        asset_key: dispatch_asset,
        target_p_mw: options.target_p_mw,
        target_q_mvar: options.target_q_mvar,
        tracking_tolerance_mw: options.tracking_tolerance_mw,
        tracking_min_satisfied_samples: options.tracking_min_samples,
        tracking_timeout_millis: options.tracking_timeout_ms,
        approver: options.approver.clone(),
        approver_role: options.approver_role.clone(),
    };
    let command_id = CommandId::new(flexcore_sim_runner::scenario_run::deterministic_uuid(
        &format!("flexcore-sim-runner/command/{}", harness.scenario().meta.scenario_id),
    ));
    let schedule_uuid = match FlexUuid::parse(&options.schedule_id) {
        Ok(uuid) => uuid,
        Err(error) => {
            return fail(ReplayError::Config(format!(
                "--schedule-id '{}' is not a canonical UUID: {error}",
                options.schedule_id
            )))
        }
    };
    let schedule_id = ScheduleId::new(schedule_uuid);
    // P5.23 — the dispatch-slice leg is wall-clock MEASURED (a measurement,
    // never a state instant; the run's state stays on the scenario clock).
    let dispatch_timer = LegTimer::start();
    let slice = match harness.run_dispatch_slice(
        &command_config,
        command_id,
        &schedule_id,
        options.revision.max(1),
        issue_at,
        poll_at,
    ) {
        Ok(slice) => slice,
        Err(error) => return fail(ReplayError::Config(error.to_string())),
    };
    let slice_secs = dispatch_timer.elapsed_secs();

    // The full horizon (quality accounting + availability changes over ALL
    // telemetry, not just the slice window) — measured the same way.
    let horizon_timer = LegTimer::start();
    if let Err(error) = harness.run_to_horizon_end() {
        return fail(ReplayError::Config(error.to_string()));
    }
    let horizon_secs = horizon_timer.elapsed_secs();

    let record = match harness.record(
        &options.code_version,
        vec![slice.command],
        vec![slice.grid_check],
        dropouts,
    ) {
        Ok(record) => record,
        Err(error) => return fail(ReplayError::Config(error.to_string())),
    };

    // P5.23 — the per-leg timings block (additive; an untimed leg is
    // absent, never zero). A refused figure aborts the run as a config
    // error — a timing bug must never be able to corrupt the record.
    let mut timings = ScenarioRunTimings::new(
        &options.code_version,
        &record.scenario_id,
        &record.scenario_version,
        record.master_seed,
    );
    if let Err(error) = timings.record(legs::DISPATCH_SLICE, slice_secs) {
        return fail(ReplayError::Config(error.to_string()));
    }
    if let Err(error) = timings.record(legs::HORIZON_SWEEP, horizon_secs) {
        return fail(ReplayError::Config(error.to_string()));
    }

    // P5.21/P5.22 — the fault plan (the `scenario_faults` v3 slot, or the
    // 1:1 derivation from the scenario's validated P2.15 fault plan) and
    // the fault journal. The JOURNAL is always built (the record carries
    // `fault_journal` whenever the scenario configures any fault); the
    // composition-level PROBES are opt-in via --fault-probes.
    let fault_plan = match ScenarioFaultPlan::from_scenario_json(&scenario_json) {
        Ok(plan) => plan,
        Err(error) => return fail(ReplayError::Config(error.to_string())),
    };
    let mut probe_outcomes: Vec<ProbeOutcome> = Vec::new();
    if options.fault_probes && fault_plan.enabled {
        run_fault_probes(
            &harness,
            &fault_plan,
            &command_config,
            &schedule_id,
            options.revision.max(1),
            &mut probe_outcomes,
        );
    }
    let journal = build_fault_journal(&fault_plan, &record, &probe_outcomes);
    let record = record
        .with_fault_journal(journal)
        .with_timings(timings);

    // P5.18/P4.37 — the closed loop at the SCENARIO level: the slice's
    // captured pipeline evidence (the GatePipelineOutcome + the step-5
    // limit report) is routed through the frozen D-P4.07-1 rule and fed
    // to the frozen P4.37 consumption contract; the decision is RECORDED
    // on the run record. The re-scheduling consumer stays the existing
    // P3.24 rolling-horizon trigger path — the frozen trigger set is NOT
    // modified (the boundary statement travels ON the summary). When the
    // slice ended before the feasibility pipeline, the summary honestly
    // says `not_routed` — no consumption decision is invented.
    let record = {
        let mut record = record;
        if let Some(observation) = harness.take_closed_loop_observation() {
            record.consumption_decisions.push(
                flexcore_sim_runner::feasibility_consumption::closed_loop_summary(
                    &observation,
                    &command_id.to_string(),
                ),
            );
        } else {
            record.consumption_decisions.push(
                flexcore_sim_runner::feasibility_consumption::not_routed_summary(
                    &command_id.to_string(),
                ),
            );
        }
        record
    };

    // P5.16 — the OPT-IN HELICS co-simulation leg. Absent
    // `--helics-federation` ⇒ a direct-adapter run, no co-sim record,
    // nothing composed or launched. Present ⇒ the config is validated
    // FAIL-CLOSED before anything is composed; `--helics-execute` then
    // launches broker + federates with the bounded timeout, and every
    // outcome (including every failure) is recorded — never a pass.
    let helics_record: Option<HelicsCoSimRecord> = match &options.helics_federation {
        None => None,
        Some(config_path) => {
            let config_text = match std::fs::read_to_string(config_path) {
                Ok(text) => text,
                Err(error) => {
                    return fail(ReplayError::Config(format!(
                        "the HELICS federation config '{}' could not be read: {error}",
                        config_path
                    )))
                }
            };
            let config = match parse_federation_config(&config_text) {
                Ok(config) => config,
                Err(error) => return fail(ReplayError::Config(error.to_string())),
            };
            let broker_config = if options.helics_execute {
                let mut config = config.clone();
                if let Some(program) = &options.helics_broker_program {
                    config.broker.program = program.clone();
                }
                config
            } else {
                config.clone()
            };
            if options.helics_execute {
                let execution = execute_federation(
                    &broker_config,
                    config_path,
                    &mut ProcessFederateLauncher,
                    Duration::from_millis(options.helics_timeout_ms),
                );
                Some(execution_record(&broker_config, &execution))
            } else {
                Some(compose_record(&broker_config))
            }
        }
    };

    let json = match &helics_record {
        None => match serde_json::to_string_pretty(&record) {
            Ok(json) => json,
            Err(error) => {
                return fail(ReplayError::Io(format!(
                    "the scenario-run record could not be serialized: {error}"
                )))
            }
        },
        Some(cosim) => match serde_json::to_string_pretty(&serde_json::json!({
            "scenario_run": record,
            "helics_cosim": cosim,
        })) {
            Ok(json) => json,
            Err(error) => {
                return fail(ReplayError::Io(format!(
                    "the scenario-run record could not be serialized: {error}"
                )))
            }
        },
    };
    match &options.out_path {
        Some(path) => {
            if let Err(error) = replay::write_file(path, &json) {
                return fail(error);
            }
        }
        None => {
            println!("{json}");
        }
    }
    for adapter in &record.adapters {
        println!(
            "adapter [{}] {}: {} ({})",
            adapter.implementation.as_str(),
            adapter.leg,
            adapter.adapter,
            adapter.detail
        );
    }
    for command in &record.commands {
        println!(
            "command {}: asset {} → state {}, issue {}, tracking {:?} (acceptance is \
             application-layer, never physical delivery)",
            command.command_id,
            command.asset_key,
            command.final_state,
            command.issue_outcome,
            command.tracking_verdict
        );
    }
    for check in &record.grid_checks {
        println!(
            "grid check via {} ({}): {}{}",
            check.backend,
            check.provider_id_version,
            check.verdict,
            check
                .reason
                .as_ref()
                .map(|reason| format!(" — {reason}"))
                .unwrap_or_default()
        );
    }
    for decision in &record.consumption_decisions {
        println!(
            "closed loop [{}] outcome {} → action {}{} (the re-scheduling consumer \
             stays the P3.24 trigger path; the frozen trigger set is NOT modified)",
            decision.command_id,
            decision.outcome_code,
            decision.action,
            decision
                .route
                .as_ref()
                .map(|route| format!(" (route: {route})"))
                .unwrap_or_default()
        );
    }
    if let Some(journal) = &record.fault_journal {
        println!(
            "fault journal (P5.21/P5.22): {} fault spec(s), {} observed this run, {} \
             cross-referenced to the P4.31 Phase 4 suites — unobserved faults are \
             recorded 'not_observed_in_this_run', never passed",
            journal.entries.len(),
            journal.observed_count,
            journal.cross_referenced_count
        );
    }
    if let Some(cosim) = &helics_record {
        println!(
            "helics co-sim: federation '{}', broker {}, pinned time step {}s, execution_mode {} \
             ({} — HELICS coordinates time steps and value exchange; it does NOT replace the \
             federates' physics models, report §12.4/§19.1)",
            cosim.federation_name,
            cosim.broker_address,
            cosim.time_step_seconds,
            cosim.execution_mode,
            match (cosim.execution_ok, cosim.time_sync_verified) {
                (Some(ok), Some(sync)) => format!("execution_ok={}, time_sync_verified={}", ok, sync),
                _ => "not executed (composed topology only)".to_string(),
            }
        );
    }
    println!(
        "scenario-run record: scenario '{}:{}', seed {}, {} ingested records, \
         statements {} (simulation-only; see the record JSON)",
        record.scenario_id,
        record.scenario_version,
        record.master_seed,
        record.records_ingested,
        record.statements.len()
    );
    EXIT_PASS
}
