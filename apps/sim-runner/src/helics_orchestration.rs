// P5.16 — The HELICS co-simulation orchestration module (§12.4, §19.1;
//         report §23 Stage 5 via ledger P5.16). NEW, additive: nothing
//         outside this module launches a federation, and the default
//         scenario runs WITHOUT HELICS (direct adapters) — co-simulation
//         is opt-in via `--helics-federation <config.json>`.
//
// ══════════════════════════════════════════════════════════════════════════
// HONESTY NOTE (recorded, travels with every record this module emits):
//
// 1. WRITTEN, NEVER EXECUTED. `helics==3.7.0` (RESEARCH.md §1, 2026-11-07)
//    was NEVER installed in this loop; no broker was ever started; no
//    federate ever joined a federation. The Python federate scripts this
//    module composes (`apps/worker/python/helics_federates/`) and THIS
//    module are UNVERIFIED until the human build/test pass executes the
//    federation against the pinned version. No co-simulation result
//    exists anywhere in this repository, and this module never fabricates
//    one: `execution_mode` is `"composed"` until a real execution
//    actually happened, and execution outcomes are the federates' exit
//    reports — validated, never synthesized.
// 2. HELICS COORDINATES; IT DOES NOT REPLACE PHYSICS (§12.4, §19.1 —
//    carried verbatim into every record): HELICS grants the time steps
//    and carries the value exchange between federates. The physics stay
//    in the existing models — the Phase 2 asset simulator's profiles and
//    the Phase 4 pandapower AC power-flow pipeline (driven through the
//    UNMODIFIED P5.15 worker protocol). A completed co-simulation run is
//    not a power-system validation by itself, and a steady-state result
//    is never transient-stability evidence.
// 3. FAIL-CLOSED (the P4.08 policy, applied per the P5.15 worker pattern):
//    spawn failure, timeout (hung process killed), non-zero exit with no
//    output, empty/unparseable output, wrong protocol tag, missing field,
//    non-finite number, a failed (`ok: false`) report, or a mismatched
//    final granted time — every one is a recorded fail-closed outcome.
//    There is NO path from a federate failure to a verified co-simulation
//    record, and no retry.
// 4. NO NEW DEPENDENCY: no `helics` Rust binding exists here and none is
//    added — an unverified dependency would violate the loop bans
//    (AGENTS.md §3 rule 1) and the dependency policy. The Rust side
//    composes and launches PROCESSES (the grid_worker.py subprocess
//    pattern, `crates/flexcore-grid-api/src/worker_backend.rs`): one
//    broker process + one process per federate, `std::process::Command`
//    + bounded wait (helper thread + `recv_timeout`; no async runtime).
// ══════════════════════════════════════════════════════════════════════════
//
// ## The composed topology (the run record names it verbatim)
//
//     control_federate --setpoint_p_mw (MW)--> asset_federate
//     asset_federate   --p_mw (MW) / q_mvar (MVAr)--> grid_federate
//                        (+ control_federate subscribes measurements)
//     grid_federate    --vm_pu (pu) / loading_percent (percent)--> control_federate
//
// All power values are injection-positive at the point of connection:
// P > 0 means export (§10.2). The federation config JSON (the protocol
// tag below) is scenario-level versioned data (P5.16 step 1; the P5.18
// scenario record consumes it as its federation field). The broker
// address is deployment configuration, never a constant.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::mpsc;
use std::time::Duration;

use serde::{Deserialize, Serialize};

// --- The recorded statements ---------------------------------------------------

/// The federation-config document tag (the federate scripts re-check it
/// fail-closed; see `federate_common.py`).
pub const HELICS_FEDERATION_PROTOCOL_ID: &str = "flexcore-helics-federation/1";

/// The federate exit-report document tag (the Python scripts emit it on
/// success; a failed federate emits NOTHING — fail-closed, never a false
/// report).
pub const HELICS_FEDERATE_REPORT_PROTOCOL_ID: &str = "flexcore-helics-federate/1";

/// The pinned co-simulation version (RESEARCH.md §1, 2026-11-07). The
/// provenance string is what the record carries, so a run can never lose
/// where the pin came from.
pub const HELICS_PINNED_VERSION: &str = "3.7.0";

/// The version's provenance + honesty string, carried on every record.
pub const HELICS_VERSION_PROVENANCE: &str = "\
    HELICS 3.7.0 per RESEARCH.md (2026-11-07, HELICS project CHANGELOG; \
    BSD-3-Clause); NEVER installed in this loop — re-verify the version \
    AND the pyhelics binding match (RESEARCH.md open task 3) at the first \
    federation run";

/// The role statement (§12.4, §19.1 — the sentence every record carries).
pub const HELICS_ROLE_STATEMENT: &str = "\
    HELICS coordinates the federation's time steps and value exchange; it \
    does NOT replace the federates' physics models. The physics stay in \
    the existing models: the Phase 2 asset simulator's profiles and the \
    Phase 4 pandapower AC power-flow pipeline (an AC power-flow \
    evaluation of specified injections, NOT an AC-OPF).";

/// The validation-boundary statement (§19.1 — the constraint the
/// Simulation Lab's claims must honour).
pub const HELICS_NOT_VALIDATION_STATEMENT: &str = "\
    A completed co-simulation run is not a power-system validation by \
    itself; a steady-state power-flow result is never transient-stability \
    evidence (report §19.1).";

/// The UNVERIFIED-API statement (carried on every record).
pub const HELICS_UNVERIFIED_API_STATEMENT: &str = "\
    The federate scripts and this orchestration were WRITTEN, never \
    executed; helics==3.7.0 was never installed; every helics API call \
    follows the documented 3.7.0 API and is UNVERIFIED until the human \
    build/test pass runs the federation.";

/// The opt-in statement (the default is direct adapters, no HELICS).
pub const HELICS_OPT_IN_STATEMENT: &str = "\
    The default scenario runs WITHOUT HELICS (direct adapters); \
    co-simulation is opt-in via --helics-federation <config.json>. \
    HELICS is the time coordinator, not the power-flow solver.";

/// The sign-convention statement (§10.2 — carried on every record).
pub const HELICS_SIGN_CONVENTION_STATEMENT: &str = "\
    Every published power value is injection-positive at the point of \
    connection: P > 0 means export; voltage is per-unit on the declared \
    nominal base; SOC, where exchanged, is a fraction in [0,1].";

/// The canonical unit vocabulary a publication/subscription may declare
/// (MW, MVAr, MVA, MWh per §10.2; voltage pu in solver representation;
/// A and percent for the adapter result shapes this federation carries).
const CANONICAL_UNITS: [&str; 8] = ["MW", "MVAr", "MVA", "MWh", "kV", "A", "pu", "percent"];

// --- The federation configuration (scenario-level versioned data) --------------

/// The parsed federation configuration document (protocol tag
/// [`HELICS_FEDERATION_PROTOCOL_ID`]).
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct HelicsFederationConfig {
    /// The protocol tag; must equal [`HELICS_FEDERATION_PROTOCOL_ID`].
    pub protocol: String,
    /// The federation name (the value-exchange namespace).
    pub federation_name: String,
    /// The broker (address = deployment configuration).
    pub broker: HelicsBrokerConfig,
    /// The PINNED time step in seconds (positive, finite).
    pub time_step_seconds: f64,
    /// The stop time in seconds (>= the time step).
    pub stop_time_seconds: f64,
    /// The federates (at minimum one each: asset, grid, control).
    pub federates: Vec<HelicsFederateSpec>,
}

/// The broker configuration. The ADDRESS is deployment configuration
/// (never hard-coded); the script path and program name the broker
/// process the orchestrator composes.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct HelicsBrokerConfig {
    /// The broker address (e.g. `tcp://127.0.0.1:23404`).
    pub address: String,
    /// The expected value-federate count (must equal the federate list).
    pub expected_federates: u32,
    /// The broker's own supervision bound, seconds (fail-closed on expiry).
    #[serde(default = "default_broker_timeout")]
    pub startup_timeout_seconds: f64,
    /// The broker stand-alone script path.
    #[serde(default = "default_broker_script")]
    pub script_path: String,
    /// The interpreter program for the broker script (deployment config).
    #[serde(default = "default_interpreter")]
    pub program: String,
}

fn default_broker_timeout() -> f64 {
    60.0
}

fn default_broker_script() -> String {
    "apps/worker/python/helics_federates/broker.py".to_string()
}

fn default_interpreter() -> String {
    "python".to_string()
}

/// One federate's configuration.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct HelicsFederateSpec {
    /// The federate name (unique in the federation).
    pub name: String,
    /// The federate role (`asset` | `grid` | `control`).
    pub role: HelicsFederateRole,
    /// The federate script path.
    pub script_path: String,
    /// The interpreter program (deployment configuration; default `python`).
    #[serde(default = "default_interpreter")]
    pub interpreter: String,
    /// The role-specific options (opaque here; validated per role).
    #[serde(default)]
    pub options: BTreeMap<String, serde_json::Value>,
    /// The publications (topic, unit, kind).
    #[serde(default)]
    pub publications: Vec<HelicsEndpointSpec>,
    /// The subscriptions (topic, unit, kind).
    #[serde(default)]
    pub subscriptions: Vec<HelicsEndpointSpec>,
}

/// The federate role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum HelicsFederateRole {
    /// The asset/end-use federate (the Phase 2 simulator's records feed).
    Asset,
    /// The grid federate (the Phase 4 pandapower pipeline via the worker).
    Grid,
    /// The FlexCore/control federate (setpoint fixture; no optimizer).
    Control,
}

impl HelicsFederateRole {
    /// The stable text form the record carries.
    pub fn as_str(self) -> &'static str {
        match self {
            HelicsFederateRole::Asset => "asset",
            HelicsFederateRole::Grid => "grid",
            HelicsFederateRole::Control => "control",
        }
    }
}

/// One publication or subscription endpoint.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct HelicsEndpointSpec {
    /// The global topic name.
    pub topic: String,
    /// The declared unit (from the canonical vocabulary).
    pub unit: String,
    /// The value kind (this federation exchanges `double` values only).
    pub kind: String,
}

// --- The fail-closed error type -------------------------------------------------

/// The orchestration error. `Config` is a composition/validation error
/// (the CLI maps it to exit 2); `ContractViolation` and `Unavailable`
/// are recorded fail-closed EXECUTION outcomes (never a pass, never a
/// retry — the P4.08 policy over the P5.15 pattern).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HelicsOrchestrationError {
    /// The federation config is invalid or incomplete.
    Config(String),
    /// A process produced a payload that violates the contract.
    ContractViolation(String),
    /// A process could not be spawned, hung past the bound, or died
    /// without an answer.
    Unavailable(String),
}

impl std::fmt::Display for HelicsOrchestrationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HelicsOrchestrationError::Config(message) => write!(f, "helics federation config: {message}"),
            HelicsOrchestrationError::ContractViolation(message) => {
                write!(f, "helics federate contract violation: {message}")
            }
            HelicsOrchestrationError::Unavailable(message) => write!(f, "helics federate unavailable: {message}"),
        }
    }
}

// --- Config parsing and validation (fail-closed, before ANY launch) -------------

/// Parses and validates the federation config document. Every rule below
/// is fail-closed: a violation is an error, the federation never
/// launches half-configured.
pub fn parse_federation_config(text: &str) -> Result<HelicsFederationConfig, HelicsOrchestrationError> {
    let config: HelicsFederationConfig = serde_json::from_str(text)
        .map_err(|error| HelicsOrchestrationError::Config(format!("not a valid federation config document: {error}")))?;
    validate_config(&config)?;
    Ok(config)
}

/// Validates an already-parsed config (public so callers that obtained a
/// config otherwise still walk the same rules).
pub fn validate_config(config: &HelicsFederationConfig) -> Result<(), HelicsOrchestrationError> {
    if config.protocol != HELICS_FEDERATION_PROTOCOL_ID {
        return Err(HelicsOrchestrationError::Config(format!(
            "protocol '{}' is not '{}'",
            config.protocol, HELICS_FEDERATION_PROTOCOL_ID
        )));
    }
    if config.federation_name.trim().is_empty() {
        return Err(HelicsOrchestrationError::Config("federation_name is empty".to_string()));
    }
    if !config.time_step_seconds.is_finite() || config.time_step_seconds <= 0.0 {
        return Err(HelicsOrchestrationError::Config(format!(
            "time_step_seconds must be a finite positive number, got {}",
            config.time_step_seconds
        )));
    }
    if !config.stop_time_seconds.is_finite() || config.stop_time_seconds < config.time_step_seconds {
        return Err(HelicsOrchestrationError::Config(format!(
            "stop_time_seconds must be a finite number >= time_step_seconds, got {}",
            config.stop_time_seconds
        )));
    }
    validate_broker(&config.broker, config.federates.len())?;

    if config.federates.is_empty() {
        return Err(HelicsOrchestrationError::Config("the federation declares no federates".to_string()));
    }

    // The three-federate minimum (P5.16 step 1): at minimum a grid
    // federate, an asset federate, and a FlexCore/control federate.
    for role in [HelicsFederateRole::Asset, HelicsFederateRole::Grid, HelicsFederateRole::Control] {
        if !config.federates.iter().any(|spec| spec.role == role) {
            return Err(HelicsOrchestrationError::Config(format!(
                "the federation declares no '{}' federate (the three-federate minimum)",
                role.as_str()
            )));
        }
    }

    // Unique, non-empty names.
    let mut names = BTreeSet::new();
    for spec in &config.federates {
        if spec.name.trim().is_empty() {
            return Err(HelicsOrchestrationError::Config("a federate name is empty".to_string()));
        }
        if !names.insert(spec.name.clone()) {
            return Err(HelicsOrchestrationError::Config(format!(
                "federate name '{}' is declared twice",
                spec.name
            )));
        }
        if spec.script_path.trim().is_empty() {
            return Err(HelicsOrchestrationError::Config(format!(
                "federate '{}' declares no script path",
                spec.name
            )));
        }
        validate_role_options(spec)?;
    }

    // Publications: globally unique topics, canonical units, double kind.
    let mut publication_topics = BTreeSet::new();
    for spec in &config.federates {
        for endpoint in &spec.publications {
            validate_endpoint(endpoint, &format!("publication of '{}'", spec.name))?;
            if !publication_topics.insert(endpoint.topic.clone()) {
                return Err(HelicsOrchestrationError::Config(format!(
                    "publication topic '{}' is declared more than once (by '{}')",
                    endpoint.topic, spec.name
                )));
            }
        }
    }

    // Subscriptions: every topic resolves to exactly one publication.
    for spec in &config.federates {
        for endpoint in &spec.subscriptions {
            validate_endpoint(endpoint, &format!("subscription of '{}'", spec.name))?;
            let matches = publication_topics
                .iter()
                .filter(|topic| **topic == endpoint.topic)
                .count();
            if matches == 0 {
                return Err(HelicsOrchestrationError::Config(format!(
                    "subscription '{}' of federate '{}' has no matching publication",
                    endpoint.topic, spec.name
                )));
            }
        }
    }

    Ok(())
}

fn validate_broker(broker: &HelicsBrokerConfig, federate_count: usize) -> Result<(), HelicsOrchestrationError> {
    let address = broker.address.trim();
    if address.is_empty() {
        return Err(HelicsOrchestrationError::Config("broker.address is empty".to_string()));
    }
    let Some((scheme, rest)) = address.split_once("://") else {
        return Err(HelicsOrchestrationError::Config(format!(
            "broker.address '{}' is not of the scheme://host[:port] form",
            broker.address
        )));
    };
    if !matches!(scheme, "tcp" | "zmq" | "udp" | "inproc") {
        return Err(HelicsOrchestrationError::Config(format!(
            "broker.address scheme '{scheme}://' is not one of tcp/zmq/udp/inproc"
        )));
    }
    let host = rest.split(':').next().unwrap_or("");
    if host.is_empty() {
        return Err(HelicsOrchestrationError::Config(format!(
            "broker.address '{}' carries no host",
            broker.address
        )));
    }
    if let Some(port) = rest.split(':').nth(1) {
        if port.parse::<u16>().is_err() {
            return Err(HelicsOrchestrationError::Config(format!(
                "broker.address '{}' carries a non-numeric port",
                broker.address
            )));
        }
    }
    if broker.expected_federates as usize != federate_count {
        return Err(HelicsOrchestrationError::Config(format!(
            "broker.expected_federates is {} but the config declares {} value federates",
            broker.expected_federates, federate_count
        )));
    }
    if !broker.startup_timeout_seconds.is_finite() || broker.startup_timeout_seconds <= 0.0 {
        return Err(HelicsOrchestrationError::Config(
            "broker.startup_timeout_seconds must be a finite positive number".to_string(),
        ));
    }
    if broker.script_path.trim().is_empty() {
        return Err(HelicsOrchestrationError::Config("broker.script_path is empty".to_string()));
    }
    Ok(())
}

fn validate_endpoint(endpoint: &HelicsEndpointSpec, context: &str) -> Result<(), HelicsOrchestrationError> {
    if endpoint.topic.trim().is_empty() {
        return Err(HelicsOrchestrationError::Config(format!("{context}: topic is empty")));
    }
    if !CANONICAL_UNITS.contains(&endpoint.unit.as_str()) {
        return Err(HelicsOrchestrationError::Config(format!(
            "{context}: topic '{}' declares unit '{}' which is not in the canonical vocabulary \
             (MW, MVAr, MVA, MWh, kV, A, pu, percent)",
            endpoint.topic, endpoint.unit
        )));
    }
    if endpoint.kind != "double" {
        return Err(HelicsOrchestrationError::Config(format!(
            "{context}: topic '{}' declares kind '{}' but this federation exchanges 'double' values",
            endpoint.topic, endpoint.kind
        )));
    }
    Ok(())
}

/// The per-role option requirements (fail-closed BEFORE any launch):
/// the federates must point at their REAL data inputs — the Phase 2
/// records feed, the model IR + the unmodified worker, the setpoint
/// fixture — never at invented data.
fn validate_role_options(spec: &HelicsFederateSpec) -> Result<(), HelicsOrchestrationError> {
    let require_string = |key: &str| -> Result<String, HelicsOrchestrationError> {
        match spec.options.get(key) {
            Some(serde_json::Value::String(value)) if !value.trim().is_empty() => Ok(value.clone()),
            _ => Err(HelicsOrchestrationError::Config(format!(
                "federate '{}' (role '{}') requires options.{} (a non-empty string)",
                spec.name,
                spec.role.as_str(),
                key
            ))),
        }
    };
    let require_object = |key: &str| -> Result<(), HelicsOrchestrationError> {
        match spec.options.get(key) {
            Some(serde_json::Value::Object(map)) if !map.is_empty() => Ok(()),
            _ => Err(HelicsOrchestrationError::Config(format!(
                "federate '{}' (role '{}') requires options.{} (a non-empty object)",
                spec.name,
                spec.role.as_str(),
                key
            ))),
        }
    };
    match spec.role {
        HelicsFederateRole::Asset => {
            require_string("input_records_path")?;
            require_object("asset_key_map")?;
        }
        HelicsFederateRole::Grid => {
            require_string("grid_worker_script")?;
            require_string("model_ir_path")?;
            require_object("bus_map")?;
        }
        HelicsFederateRole::Control => {
            require_string("setpoints_path")?;
        }
    }
    Ok(())
}

// --- The launch plan ------------------------------------------------------------

/// The process kind of one planned process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessKind {
    /// The broker process (coordination only).
    Broker,
    /// A federate process (the role is carried).
    Federate(HelicsFederateRole),
}

/// One process the orchestrator will compose: program + argument list,
/// in launch order (broker first).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FederateProcessSpec {
    /// The label the outcome is matched by (the federate name, or
    /// `broker`).
    pub label: String,
    /// The process kind.
    pub kind: ProcessKind,
    /// The program to spawn.
    pub program: String,
    /// The argument list (script path + flags; the config path is the
    /// single source of federation truth — no flag duplicates it).
    pub args: Vec<String>,
}

/// Composes the launch plan: the broker FIRST, then every federate in
/// config order, each with `--federation-config <path> --federate <name>`.
/// Pure composition — nothing is spawned here.
pub fn launch_plan(config: &HelicsFederationConfig, config_path: &str) -> Vec<FederateProcessSpec> {
    let mut plan = vec![FederateProcessSpec {
        label: "broker".to_string(),
        kind: ProcessKind::Broker,
        program: config.broker.program.clone(),
        args: vec![
            config.broker.script_path.clone(),
            "--federation-config".to_string(),
            config_path.to_string(),
            "--timeout-seconds".to_string(),
            format!("{}", config.broker.startup_timeout_seconds),
        ],
    }];
    for spec in &config.federates {
        plan.push(FederateProcessSpec {
            label: spec.name.clone(),
            kind: ProcessKind::Federate(spec.role),
            program: spec.interpreter.clone(),
            args: vec![
                spec.script_path.clone(),
                "--federation-config".to_string(),
                config_path.to_string(),
                "--federate".to_string(),
                spec.name.clone(),
            ],
        });
    }
    plan
}

// --- The launcher (real process path; tests inject a scripted double) -----------

/// The raw output of one launched process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FederateProcessOutput {
    /// The label the spec carried.
    pub label: String,
    /// The full stdout.
    pub stdout: String,
    /// The full stderr.
    pub stderr: String,
    /// Whether the process exited successfully.
    pub exit_ok: bool,
    /// Whether the process hit the bounded wait and was killed.
    pub timed_out: bool,
}

/// The process launcher seam. The REAL implementation spawns the
/// processes concurrently and waits on each with a bounded timeout
/// (killing stragglers); a scripted double serves the composition tests
/// (a federate contract test without any helics installation).
pub trait FederateLauncher {
    /// Runs ALL the specs (concurrently — the broker and the federates
    /// live together), each bounded by `timeout`. A timeout kills the
    /// process and is reported fail-closed, never retried.
    fn run_all(&mut self, specs: &[FederateProcessSpec], timeout: Duration) -> Vec<FederateProcessOutput>;
}

/// The REAL launcher: `std::process::Command`, all specs spawned
/// concurrently, per-process bounded wait (helper thread +
/// `recv_timeout`; no async runtime) — the P5.15 worker-process pattern,
/// widened to a process group. Fail-closed on spawn failure and timeout:
/// a hung process is KILLED and its outcome recorded fail-closed; the
/// other processes still run to their own bounded ends and every outcome
/// is reported — the execution record shows the whole truth, never a
/// first-error short-circuit.
pub struct ProcessFederateLauncher;

/// One concurrently-running planned process (or a spawn failure, which
/// is recorded fail-closed without any child ever existing).
struct RunningProcess {
    label: String,
    child: Option<std::process::Child>,
    output: mpsc::Receiver<(String, String)>,
}

impl FederateLauncher for ProcessFederateLauncher {
    fn run_all(&mut self, specs: &[FederateProcessSpec], timeout: Duration) -> Vec<FederateProcessOutput> {
        let mut running: Vec<RunningProcess> = Vec::new();
        for spec in specs {
            let label = spec.label.clone();
            let spawned = std::process::Command::new(&spec.program)
                .args(&spec.args)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn();
            match spawned {
                Ok(mut child) => {
                    let stdout_handle = child.stdout.take();
                    let stderr_handle = child.stderr.take();
                    let (sender, receiver) = mpsc::channel();
                    std::thread::spawn(move || {
                        let mut stdout_text = String::new();
                        if let Some(mut stdout) = stdout_handle {
                            let _ = std::io::Read::read_to_string(&mut stdout, &mut stdout_text);
                        }
                        let mut stderr_text = String::new();
                        if let Some(mut stderr) = stderr_handle {
                            let _ = std::io::Read::read_to_string(&mut stderr, &mut stderr_text);
                        }
                        let _ = sender.send((stdout_text, stderr_text));
                    });
                    running.push(RunningProcess {
                        label,
                        child: Some(child),
                        output: receiver,
                    });
                }
                Err(error) => {
                    // No child exists; the failure is synthesized
                    // fail-closed (the channel pattern keeps the wait
                    // loop uniform).
                    let (sender, receiver) = mpsc::channel();
                    let _ = sender.send((
                        String::new(),
                        format!(
                            "the process could not be spawned (program '{}'): {error}",
                            spec.program
                        ),
                    ));
                    running.push(RunningProcess {
                        label,
                        child: None,
                        output: receiver,
                    });
                }
            }
        }

        let mut outputs = Vec::new();
        for entry in running {
            let (stdout, stderr, timed_out) = match entry.output.recv_timeout(timeout) {
                Ok((stdout, stderr)) => (stdout, stderr, false),
                Err(_) => {
                    // Bounded wait expired: kill first (never leave a
                    // hung process), then reap so no zombie remains.
                    if let Some(child) = entry.child.as_mut() {
                        let _ = child.kill();
                        let _ = child.wait();
                    }
                    (
                        String::new(),
                        format!(
                            "the process did not answer within {:?}; it was killed and the \
                             outcome is fail-closed (no retry — D-P4.05-2)",
                            timeout
                        ),
                        true,
                    )
                }
            };
            let exit_ok = if timed_out {
                false
            } else {
                match entry.child.as_mut().map(|child| child.wait()) {
                    Some(Ok(status)) => status.success(),
                    _ => false, // spawn failure or reap failure: fail-closed
                }
            };
            outputs.push(FederateProcessOutput {
                label: entry.label,
                stdout,
                stderr,
                exit_ok,
                timed_out,
            });
        }
        outputs
    }
}

// --- Exit-report parsing (the fail-closed payload boundary) ----------------------

/// The federate/broker exit report (the `flexcore-helics-federate/1`
/// document the Python scripts emit on SUCCESS; a failed federate emits
/// nothing at all — fail-closed, never a false report).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FederateExitReport {
    /// The protocol tag; must equal [`HELICS_FEDERATE_REPORT_PROTOCOL_ID`].
    pub protocol: String,
    /// `"federate"` or `"broker"`.
    pub kind: String,
    /// The federation name the process joined.
    pub federation: String,
    /// The federate name (or `broker`).
    pub federate: String,
    /// The role (federate reports only).
    pub role: Option<String>,
    /// The granted-step count (federate reports only).
    pub granted_steps: Option<u64>,
    /// The final granted time in seconds (federate reports only).
    pub final_granted_time_seconds: Option<f64>,
    /// The published-value count.
    pub published_values: Option<u64>,
    /// The received-value count.
    pub received_values: Option<u64>,
    /// The skipped-publication count (absences, never fabricated values).
    pub skipped_publications: Option<u64>,
    /// Success flag (a `false` report is a contract violation here — a
    /// failing federate exits non-zero WITHOUT a report; an `ok:false`
    /// document is therefore malformed by construction).
    pub ok: bool,
    /// The status text (carries the role statement).
    pub status_text: String,
    /// The installed binding's version, as reported (never assumed).
    pub helics_version_reported: Option<String>,
}

/// Parses and validates one exit report: protocol tag, kind, expected
/// process identity, `ok`, and finiteness of the final granted time.
pub fn parse_exit_report(
    text: &str,
    expected_process: &str,
    expected_kind: &str,
) -> Result<FederateExitReport, HelicsOrchestrationError> {
    if text.trim().is_empty() {
        return Err(HelicsOrchestrationError::ContractViolation(format!(
            "process '{}' produced no exit report on stdout",
            expected_process
        )));
    }
    let first_line = text.lines().next().unwrap_or("").trim();
    let report: FederateExitReport = serde_json::from_str(first_line).map_err(|error| {
        HelicsOrchestrationError::ContractViolation(format!(
            "process '{}' produced a non-contract-shaped exit report: {error}",
            expected_process
        ))
    })?;
    if report.protocol != HELICS_FEDERATE_REPORT_PROTOCOL_ID {
        return Err(HelicsOrchestrationError::ContractViolation(format!(
            "process '{}' reported protocol '{}' but this orchestrator speaks '{}'",
            expected_process, report.protocol, HELICS_FEDERATE_REPORT_PROTOCOL_ID
        )));
    }
    if report.kind != expected_kind {
        return Err(HelicsOrchestrationError::ContractViolation(format!(
            "process '{}' reported kind '{}' but was composed as '{}'",
            expected_process, report.kind, expected_kind
        )));
    }
    if report.federate != expected_process {
        return Err(HelicsOrchestrationError::ContractViolation(format!(
            "the exit report names process '{}' but was expected from '{}'",
            report.federate, expected_process
        )));
    }
    if !report.ok {
        return Err(HelicsOrchestrationError::ContractViolation(format!(
            "process '{}' reported ok:false ('{}') — a failing federate must fail-closed \
             WITHOUT a report; an ok:false document is malformed by construction",
            expected_process, report.status_text
        )));
    }
    if let Some(time) = report.final_granted_time_seconds {
        if !time.is_finite() {
            return Err(HelicsOrchestrationError::ContractViolation(format!(
                "process '{}' reported a non-finite final granted time",
                expected_process
            )));
        }
    }
    Ok(report)
}

// --- Execution -------------------------------------------------------------------

/// One process's run outcome: the validated report, or the fail-closed
/// reason. Exactly one of the two is present — an outcome is never both
/// and never neither.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FederateRunOutcome {
    /// The process label (federate name, or `broker`).
    pub label: String,
    /// The kind (`broker`, or the federate role).
    pub kind: String,
    /// The validated exit report, when the process completed
    /// contract-shaped.
    pub report: Option<FederateExitReport>,
    /// The fail-closed reason, when the process failed (spawn failure,
    /// timeout, non-contract output, failed report).
    pub fail_closed_reason: Option<String>,
    /// Whether the bounded wait expired and the process was killed.
    pub timed_out: bool,
}

/// The execution outcome of the whole federation.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HelicsCosimExecution {
    /// The broker's outcome (coordination only — it carries no values).
    pub broker: FederateRunOutcome,
    /// The federates' outcomes, in config order.
    pub federates: Vec<FederateRunOutcome>,
    /// Whether the time synchronisation is verified: every federate
    /// completed with a final granted time equal to the configured stop
    /// time (within tolerance) AND equal granted-step counts. Recorded
    /// honestly as false whenever any federate failed.
    pub time_sync_verified: bool,
    /// Whether the whole execution succeeded (broker + all federates +
    /// time sync). A false value is an honest recorded outcome, never a
    /// silent pass.
    pub execution_ok: bool,
}

/// The tolerance (seconds) for the final-granted-time comparison.
const TIME_SYNC_TOLERANCE_SECONDS: f64 = 1e-6;

/// Executes the federation: composes the plan, runs ALL processes
/// concurrently through the launcher with the bounded timeout, and
/// validates every report fail-closed. Process-level failures are
/// RECORDED (the returned execution carries the fail-closed reasons and
/// `execution_ok: false`) — only a launcher that cannot run at all
/// surfaces here is impossible: every process failure lands in its
/// outcome.
pub fn execute_federation(
    config: &HelicsFederationConfig,
    config_path: &str,
    launcher: &mut dyn FederateLauncher,
    timeout: Duration,
) -> HelicsCosimExecution {
    let plan = launch_plan(config, config_path);
    let outputs: BTreeMap<String, FederateProcessOutput> = launcher
        .run_all(&plan, timeout)
        .into_iter()
        .map(|output| (output.label.clone(), output))
        .collect();

    let broker = outcome_for(&outputs, "broker", ProcessKind::Broker);
    let mut federates = Vec::new();
    for spec in &config.federates {
        federates.push(outcome_for(
            &outputs,
            &spec.name,
            ProcessKind::Federate(spec.role),
        ));
    }

    let mut time_sync_verified = true;
    for outcome in &federates {
        match &outcome.report {
            Some(report) => {
                let final_time = report.final_granted_time_seconds.unwrap_or(f64::NAN);
                let steps = report.granted_steps.unwrap_or(0);
                if (final_time - config.stop_time_seconds).abs() > TIME_SYNC_TOLERANCE_SECONDS {
                    time_sync_verified = false;
                }
                if steps == 0 {
                    time_sync_verified = false;
                }
            }
            None => {
                time_sync_verified = false;
            }
        }
    }
    // Equal granted-step counts across federates (the pinned uniform
    // period; a divergence is recorded, not coerced).
    if let Some(first) = federates
        .first()
        .and_then(|outcome| outcome.report.as_ref())
        .and_then(|report| report.granted_steps)
    {
        for outcome in &federates {
            if outcome
                .report
                .as_ref()
                .and_then(|report| report.granted_steps)
                != Some(first)
            {
                time_sync_verified = false;
            }
        }
    }

    let execution_ok = broker.report.is_some()
        && federates.iter().all(|outcome| outcome.report.is_some())
        && time_sync_verified;

    HelicsCosimExecution {
        broker,
        federates,
        time_sync_verified,
        execution_ok,
    }
}

/// Maps one process output to its outcome (fail-closed on every failure
/// mode: missing output, timeout, non-zero exit, empty/malformed/
/// mismatched report).
fn outcome_for(
    outputs: &BTreeMap<String, FederateProcessOutput>,
    label: &str,
    kind: ProcessKind,
) -> FederateRunOutcome {
    let kind_str = match kind {
        ProcessKind::Broker => "broker".to_string(),
        ProcessKind::Federate(role) => role.as_str().to_string(),
    };
    let Some(output) = outputs.get(label) else {
        return FederateRunOutcome {
            label: label.to_string(),
            kind: kind_str,
            report: None,
            fail_closed_reason: Some(format!(
                "process '{}' produced no output at all (launcher returned none) — fail-closed",
                label
            )),
            timed_out: false,
        };
    };
    if output.timed_out {
        let reason = output.stderr.trim();
        return FederateRunOutcome {
            label: label.to_string(),
            kind: kind_str,
            report: None,
            fail_closed_reason: Some(if reason.is_empty() {
                "the process exceeded the bounded wait and was killed — fail-closed".to_string()
            } else {
                reason.to_string()
            }),
            timed_out: true,
        };
    }
    if !output.exit_ok {
        return FederateRunOutcome {
            label: label.to_string(),
            kind: kind_str,
            report: None,
            fail_closed_reason: Some(format!(
                "the process exited unsuccessfully{}",
                if output.stderr.trim().is_empty() {
                    String::new()
                } else {
                    format!(" (stderr: {})", output.stderr.trim())
                }
            )),
            timed_out: false,
        };
    }
    let expected_kind = if kind == ProcessKind::Broker {
        "broker"
    } else {
        "federate"
    };
    match parse_exit_report(&output.stdout, label, expected_kind) {
        Ok(report) => FederateRunOutcome {
            label: label.to_string(),
            kind: kind_str,
            report: Some(report),
            fail_closed_reason: None,
            timed_out: false,
        },
        Err(error) => FederateRunOutcome {
            label: label.to_string(),
            kind: kind_str,
            report: None,
            fail_closed_reason: Some(error.to_string()),
            timed_out: false,
        },
    }
}

// --- The co-simulation record (the run record's co-sim topology) -----------------

/// One federate's trace in the co-simulation record.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HelicsFederateTrace {
    /// The federate name.
    pub name: String,
    /// The role.
    pub role: String,
    /// The script path.
    pub script_path: String,
    /// The publications (topic + unit, verbatim from the config).
    pub publications: Vec<String>,
    /// The subscriptions (topic + unit, verbatim from the config).
    pub subscriptions: Vec<String>,
    /// The execution outcome (absent in composed-only records).
    pub execution: Option<FederateRunOutcome>,
}

/// The co-simulation record the scenario-run record names the co-sim
/// topology through (P5.16 step 1; the P5.18 scenario record consumes it
/// as its federation field). Serializable.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HelicsCoSimRecord {
    /// The version pin + provenance (never a bare version number).
    pub helics_version_pin: String,
    /// The federation name.
    pub federation_name: String,
    /// The broker address (deployment configuration, verbatim).
    pub broker_address: String,
    /// The pinned time step (seconds).
    pub time_step_seconds: f64,
    /// The stop time (seconds).
    pub stop_time_seconds: f64,
    /// The composed topology, one arrow per publication → subscriber
    /// match, verbatim from the config.
    pub topology_summary: String,
    /// `"composed"` (validated, never launched) or `"executed"`.
    pub execution_mode: String,
    /// Whether the whole execution succeeded (executed records only).
    pub execution_ok: Option<bool>,
    /// Whether the time synchronisation was verified (executed records
    /// only; recorded honestly as false whenever anything failed).
    pub time_sync_verified: Option<bool>,
    /// The federate traces (topology + outcomes).
    pub federates: Vec<HelicsFederateTrace>,
    /// The recorded statements (role, validation boundary, UNVERIFIED
    /// API, opt-in, sign convention, simulation-only).
    pub statements: Vec<String>,
}

/// Composes the record from a VALIDATED config — no execution. This is
/// the record the run carries when co-simulation is configured but not
/// executed (`execution_mode: "composed"`): an honest topology
/// statement, never a claim that a federation ran.
pub fn compose_record(config: &HelicsFederationConfig) -> HelicsCoSimRecord {
    let topology = topology_summary(config);
    HelicsCoSimRecord {
        helics_version_pin: HELICS_VERSION_PROVENANCE.to_string(),
        federation_name: config.federation_name.clone(),
        broker_address: config.broker.address.clone(),
        time_step_seconds: config.time_step_seconds,
        stop_time_seconds: config.stop_time_seconds,
        topology_summary: topology,
        execution_mode: "composed".to_string(),
        execution_ok: None,
        time_sync_verified: None,
        federates: config
            .federates
            .iter()
            .map(|spec| HelicsFederateTrace {
                name: spec.name.clone(),
                role: spec.role.as_str().to_string(),
                script_path: spec.script_path.clone(),
                publications: spec
                    .publications
                    .iter()
                    .map(|endpoint| format!("{} ({})", endpoint.topic, endpoint.unit))
                    .collect(),
                subscriptions: spec
                    .subscriptions
                    .iter()
                    .map(|endpoint| format!("{} ({})", endpoint.topic, endpoint.unit))
                    .collect(),
                execution: None,
            })
            .collect(),
        statements: statements(),
    }
}

/// Composes the record for an EXECUTED federation (the outcomes embedded
/// verbatim — including every fail-closed reason).
pub fn execution_record(config: &HelicsFederationConfig, execution: &HelicsCosimExecution) -> HelicsCoSimRecord {
    let mut record = compose_record(config);
    record.execution_mode = "executed".to_string();
    record.execution_ok = Some(execution.execution_ok);
    record.time_sync_verified = Some(execution.time_sync_verified);
    record.statements.push(format!(
        "execution outcome: execution_ok={}, time_sync_verified={} — a false value is an \
         honest fail-closed outcome, never a silent pass (P4.08 policy)",
        execution.execution_ok, execution.time_sync_verified
    ));
    for (spec, outcome) in config.federates.iter().zip(&execution.federates) {
        if let Some(trace) = record.federates.iter_mut().find(|trace| trace.name == spec.name) {
            trace.execution = Some(outcome.clone());
        }
    }
    record
}

/// The topology summary: one arrow per publication → subscriber match.
fn topology_summary(config: &HelicsFederationConfig) -> String {
    let mut arrows: Vec<String> = Vec::new();
    for publisher in &config.federates {
        for publication in &publisher.publications {
            let subscribers: Vec<&str> = config
                .federates
                .iter()
                .filter(|consumer| {
                    consumer.name != publisher.name
                        && consumer.subscriptions.iter().any(|sub| sub.topic == publication.topic)
                })
                .map(|consumer| consumer.name.as_str())
                .collect();
            if subscribers.is_empty() {
                arrows.push(format!(
                    "{} --{} ({})--> (no subscriber)",
                    publisher.name, publication.topic, publication.unit
                ));
            } else {
                arrows.push(format!(
                    "{} --{} ({})--> {}",
                    publisher.name,
                    publication.topic,
                    publication.unit,
                    subscribers.join(", ")
                ));
            }
        }
    }
    arrows.join("; ")
}

/// The statement list every record carries (the honesty note is part of
/// the data, never a comment).
fn statements() -> Vec<String> {
    vec![
        HELICS_ROLE_STATEMENT.to_string(),
        HELICS_NOT_VALIDATION_STATEMENT.to_string(),
        HELICS_UNVERIFIED_API_STATEMENT.to_string(),
        HELICS_OPT_IN_STATEMENT.to_string(),
        HELICS_SIGN_CONVENTION_STATEMENT.to_string(),
        "Every command in a scenario run is issued to the SIMULATOR and nothing else; the \
         co-simulation federates carry federation VALUES (setpoints from a pre-computed \
         fixture, measurements from the existing models), not commands — the command \
         lifecycle and its gates live in the platform, untouched by any federate (§14.1, §21.4)."
            .to_string(),
    ]
}
