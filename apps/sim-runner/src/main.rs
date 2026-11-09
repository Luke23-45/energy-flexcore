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

use std::process::ExitCode;

use flexcore_sim_runner::benchmark::{self, BenchmarkRecord};
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

EXIT CODES: 0 replay pass / benchmark record produced; 1 replay mismatch /
benchmark completeness or structural failure; 2 usage/config error.
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

enum Command {
    Run(RunOptions),
    Replay(ReplayOptions),
    AsOf(AsOfOptions),
    Benchmark(BenchmarkOptions),
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
        "--help" | "-h" | "help" => Err(CliError::Usage),
        other => Err(CliError::Failed(ReplayError::Config(format!(
            "unknown subcommand '{other}' (expected 'run', 'replay', 'as-of', or 'benchmark')"
        )))),
    }
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
