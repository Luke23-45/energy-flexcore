// P7.20 — Plugin health, readiness, and observability (report §5.5
// `health.readiness`, §5.6, §21.3; ledger `ledger_phase_7.md` P7.20;
// decision record
// `ledger/phase-07-plugin-sdk/19_20_36_config_health_path.md`).
//
// WHAT THIS MODULE IS: the host-side per-plugin health/readiness monitor
// over the closed vocabulary, plus the bounded stats snapshot the operator
// console (P7.25) consumes. It mirrors the P6.17 edge health discipline
// (`crates/edge-agent/src/health.rs`) — cited, adapted, not reinvented —
// with the plugin-path differences recorded below.
//
// THE CLOSED VOCABULARY (consistent with the kernel lifecycle states and
// the P6.17 edge discipline; exactly, no ninth reason):
//
// | Status | Meaning |
// |---|---|
// | `HEALTHY` | observed recently (inside the detection window), error rate inside limits, no exhaustion latched |
// | `DEGRADED_UNVERIFIED` | no observation has arrived yet — NEVER healthy-by-default (the P6.17 forbidden-inference rule, mirrored) |
// | `DEGRADED_<exhaustion>` | up but impaired by a resource exhaustion the limits layer raised (`crates/.../limits.rs` errors → closed reasons: `MEMORY_LIMIT`, `FUEL_EXHAUSTED`, `DEADLINE`, `OUTPUT_LIMIT`, `CALL_RATE`) |
// | `DEGRADED_ERROR_RATE` | the bounded-window error count reached the threshold |
// | `FAILING_UNRESPONSIVE` | the last responsive interaction is older than the detection window, or an explicit liveness probe failed — dependent actions are BLOCKED |
// | `FAILING_BINDINGS` | the bindings seam reports the plugin cannot run at all (`HostError::BindingsNotGenerated`) — dependent actions are BLOCKED |
//
// THE HARD RULE (the ledger's risk row, mirrored from P6.17): health is
// NEVER inferred from the absence of errors. A registered plugin with NO
// observation is `DEGRADED_UNVERIFIED` and stays there until the first
// observation lands; readiness is NEVER ready before the first successful
// call OR an explicit readiness probe through the binding seam.
//
// READINESS (the §5.5 `health.readiness` semantics, the P7.05-recorded
// seam): the closed readiness vocabulary is
// [`ReadinessState`] = { `UNOBSERVED`, `PROBE_FAILED`, `READY` }. A plugin
// becomes READY exactly two ways: (1) its FIRST successful call through the
// binding seam, or (2) an explicit readiness probe
// ([`PluginHealthMonitor::record_readiness_probe`], the host's binding-seam
// hook). A failed probe is `PROBE_FAILED` — it does not silently roll back
// to unobserved. A successful call ALSO does not clear a latched
// `PROBE_FAILED` from the readiness record (the probe verdict is its own
// observation); only the next successful probe or — after a crash/fail —
// re-registration resets it. Readiness is REPORTED, never enforced here:
// the readiness gate itself is the lifecycle's `Started → Ready` health
// gate (kernel `lifecycle.rs`, GATE_HEALTH) and the host composes this
// monitor's verdict into that gate.
//
// LIVENESS (the host's epoch/probe hooks): liveness is a POSITIVE,
// timestamped rule on the LAST OBSERVED fact — the successful-call and
// probe observations stamp `last_seen`; `now - last_seen >
// unresponsive_after` (the detection window) is `FAILING_UNRESPONSIVE`,
// and an explicit failed liveness probe
// ([`PluginHealthMonitor::record_liveness_probe`]) fails immediately.
// Silence produces FAILING only through the window on the last seen fact,
// never by default.
//
// RESOURCE EXHAUSTION → DEGRADATION (the closed mapping):
// [`exhaustion_reason`] maps the `limits.rs` enforcement errors to the
// closed DEGRADED reason set — `MemoryLimitExceeded` → `MEMORY_LIMIT`,
// `FuelExhausted` → `FUEL_EXHAUSTED`, `DeadlineExceeded` → `DEADLINE`,
// `OutputLimitExceeded` → `OUTPUT_LIMIT`, `CallRateExceeded` →
// `CALL_RATE`; `BindingsNotGenerated` (not an exhaustion) maps to
// `FAILING_BINDINGS`. A host error that maps to nothing is a named
// [`HealthError::UnmappableHostError`] — never silently folded into
// "healthy". An exhaustion DEGRADES the plugin (and is latched until a
// later successful call clears it — the recovery rule, a DEFAULT decision
// recorded as D-P7.20-2); the instance teardown itself is the limits
// layer's contract (`limits.rs`/`host.rs`), not re-decided here.
//
// STATS AND THE SNAPSHOT (no unbounded memory):
// per-plugin counters (calls, errors, timeouts, fuel used, last
// transition) live in a fixed-size struct ([`PluginStats`]) — a CLONABLE,
// BOUNDED snapshot; there is no per-call log anywhere. The error-sample
// list that feeds the error-rate derivation is PRUNED to the bounded
// window and hard-capped ([`MAX_ERROR_SAMPLES`], oldest dropped and
// counted). The observable snapshot ([`HealthSnapshot`], ordered by plugin
// id) renders through [`HealthSnapshot::json`] — a serializable snapshot
// method. THE TRANSPORT IS A RECORDED SEAM, NOT IMPLEMENTED: the gRPC/REST
// surface that carries this snapshot to the console is the P7.20 recorded
// gRPC transport seam (decision record §5; `10_11_plugin_host_broker.md`
// §1) and the P7.25 console seam; mTLS on that surface is the P7.34 trust
// boundary — none of the three is implemented in this module, and no
// secret or config value ever enters a snapshot or a log line (the
// P7.20/P7.19 rule).
//
// AUDIT: every health-state transition (and every refused observation —
// an observation for an unregistered plugin) is emitted through the
// [`HealthAuditSink`] port as a [`PluginHealthAuditRecord`]. The port is
// defined HERE (host-local): this crate depends on no flexcore crate (the
// recorded alignment seam of `broker.rs` §6), so the kernel's
// `AuditSink` is mirrored in shape, not imported. Transition ordering is
// derived deterministically from the assessment order, never from map
// iteration (the track map is a `BTreeMap`).
//
// House rules: fail-closed everywhere, named errors with stable machine
// names, no `unwrap`/`expect` on untrusted input, no `todo!()`, no clock
// of its own (every method takes `now_unix_millis` explicitly — the host
// supplies the time and owns its correctness, the `broker.rs` rule).

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Mutex;

use crate::host::HostError;

// --- The closed reason vocabulary -------------------------------------------------

/// The closed set of health REASONS (exactly; extending it is a reviewed
/// change — an open string field would let "healthy-ish" back in).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum HealthReason {
    /// No observation has arrived yet — the plugin is UNVERIFIED, never
    /// healthy-by-default (the P6.17 forbidden-inference rule).
    Unverified,
    /// No successful interaction inside the detection window, or an
    /// explicit liveness probe failed.
    Unresponsive,
    /// `HostError::MemoryLimitExceeded` — the memory-page limit.
    MemoryLimit,
    /// `HostError::FuelExhausted` — the metered-instruction budget.
    FuelExhausted,
    /// `HostError::DeadlineExceeded` — the wall-clock/epoch deadline.
    Deadline,
    /// `HostError::OutputLimitExceeded` — the per-call output cap.
    OutputLimit,
    /// `HostError::CallRateExceeded` — the call-rate cap.
    CallRate,
    /// The bounded-window error count reached the threshold.
    ErrorRate,
    /// `HostError::BindingsNotGenerated` — the plugin cannot run at all in
    /// this build; FAILING (dependent actions blocked).
    Bindings,
}

impl HealthReason {
    /// The SCREAMING_SNAKE reason token used inside the status name.
    pub fn token(self) -> &'static str {
        match self {
            HealthReason::Unverified => "UNVERIFIED",
            HealthReason::Unresponsive => "UNRESPONSIVE",
            HealthReason::MemoryLimit => "MEMORY_LIMIT",
            HealthReason::FuelExhausted => "FUEL_EXHAUSTED",
            HealthReason::Deadline => "DEADLINE",
            HealthReason::OutputLimit => "OUTPUT_LIMIT",
            HealthReason::CallRate => "CALL_RATE",
            HealthReason::ErrorRate => "ERROR_RATE",
            HealthReason::Bindings => "BINDINGS",
        }
    }
}

/// The closed health status vocabulary: HEALTHY, DEGRADED_<reason>,
/// FAILING_<reason> — exactly (the P6.17 shape, mirrored).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthState {
    Healthy,
    Degraded(HealthReason),
    Failing(HealthReason),
}

impl HealthState {
    /// The status name (`HEALTHY`, `DEGRADED_FUEL_EXHAUSTED`, `FAILING_*`)
    /// — the closed string form the snapshot renders.
    pub fn name(self) -> String {
        match self {
            HealthState::Healthy => "HEALTHY".to_string(),
            HealthState::Degraded(reason) => format!("DEGRADED_{}", reason.token()),
            HealthState::Failing(reason) => format!("FAILING_{}", reason.token()),
        }
    }

    /// Whether dependent actions are blocked (only `FAILING_*` blocks).
    pub fn blocks_dependent_actions(self) -> bool {
        matches!(self, HealthState::Failing(_))
    }
}

/// The closed readiness vocabulary (§5.5 `health.readiness`; see the
/// module header for the never-ready-before-observation rule).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ReadinessState {
    /// No readiness observation at all: no successful call, no probe.
    Unobserved,
    /// The explicit readiness probe FAILED (its own recorded verdict).
    ProbeFailed,
    /// READY — after the first successful call or a passed probe.
    Ready,
}

impl ReadinessState {
    /// The stable machine name (the snapshot value).
    pub fn name(self) -> &'static str {
        match self {
            ReadinessState::Unobserved => "UNOBSERVED",
            ReadinessState::ProbeFailed => "PROBE_FAILED",
            ReadinessState::Ready => "READY",
        }
    }
}

/// The exhaustion/liveness mapping from the `limits.rs`/`host.rs` named
/// errors to the closed reason vocabulary (the recorded mapping table).
/// `None` for host errors that are not plugin-health inputs (loader errors
/// like `ComponentNotFound` are the composition's problem, never folded
/// into a running plugin's health).
pub fn exhaustion_reason(error: &HostError) -> Option<HealthReason> {
    match error {
        HostError::MemoryLimitExceeded { .. } => Some(HealthReason::MemoryLimit),
        HostError::FuelExhausted => Some(HealthReason::FuelExhausted),
        HostError::DeadlineExceeded { .. } => Some(HealthReason::Deadline),
        HostError::OutputLimitExceeded { .. } => Some(HealthReason::OutputLimit),
        HostError::CallRateExceeded { .. } => Some(HealthReason::CallRate),
        HostError::BindingsNotGenerated { .. } => Some(HealthReason::Bindings),
        _ => None,
    }
}

// --- Observations and limits --------------------------------------------------------

/// One observed fact about a plugin, fed by the host's call and probe
/// hooks. The monitor DERIVES health from these; it never invents them.
#[derive(Debug, Clone, PartialEq)]
pub enum PluginObservation {
    /// A call through the binding seam succeeded. `fuel_used` is the
    /// metered-instruction consumption the host recorded for the call
    /// (accumulated into the stats snapshot; `0` when the build carries no
    /// metering reading — recorded, never guessed).
    CallSucceeded { fuel_used: u64 },
    /// A call failed (the plugin answered with an error, or the seam
    /// refused): the named, human-safe reason (never raw backend text, no
    /// config values — the P1.08 log discipline).
    CallFailed { reason: String },
    /// A call hit its wall-clock deadline (counted as an error AND a
    /// timeout — the stats snapshot separates the two counters).
    CallTimedOut { reason: String },
}

/// The health derivation limits (validated at construction — a nonsense
/// limit is a composition error, never a silently weakened check).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HealthLimits {
    /// No successful interaction within this window after the last seen
    /// one → FAILING_UNRESPONSIVE (the detection window).
    pub unresponsive_after_millis: u64,
    /// The bounded error-rate window.
    pub error_rate_window_millis: u64,
    /// Errors within the window at or above this count →
    /// DEGRADED_ERROR_RATE.
    pub max_errors_in_window: u32,
}

impl HealthLimits {
    fn validate(&self) -> Result<(), HealthError> {
        if self.unresponsive_after_millis == 0 {
            return Err(HealthError::InvalidLimits {
                reason: "unresponsive_after_millis must be positive".to_string(),
            });
        }
        if self.error_rate_window_millis == 0 {
            return Err(HealthError::InvalidLimits {
                reason: "error_rate_window_millis must be positive".to_string(),
            });
        }
        if self.max_errors_in_window == 0 {
            return Err(HealthError::InvalidLimits {
                reason: "max_errors_in_window must be positive".to_string(),
            });
        }
        Ok(())
    }
}

/// Hard cap on the retained error samples per plugin (the bounded-memory
/// rule): the oldest sample is dropped — and still counted in
/// [`PluginStats::errors_total`] — when the cap is exceeded. The cap bounds
/// the track's memory independently of the window width.
pub const MAX_ERROR_SAMPLES: usize = 64;

// --- Errors -----------------------------------------------------------------------

/// Named errors of the monitor's construction and inputs. Every failure is
/// named; nothing implies a silent pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HealthError {
    /// Health was queried for a plugin this monitor has never registered:
    /// the honest answer is a named error, not a status (the P6.17
    /// `UnknownAdapter` rule — an unknown id is a wiring bug and must be
    /// loud).
    UnknownPlugin { plugin_id: String },
    /// A limit set failed validation.
    InvalidLimits { reason: String },
    /// A host error was fed as an observation but maps to nothing in the
    /// closed exhaustion vocabulary — never silently folded into healthy.
    UnmappableHostError { name: &'static str },
}

impl fmt::Display for HealthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HealthError::UnknownPlugin { plugin_id } => {
                write!(
                    f,
                    "no health track exists for plugin '{plugin_id}' (it was never registered); \
                     health is unknown, not healthy"
                )
            }
            HealthError::InvalidLimits { reason } => {
                write!(f, "invalid health limits: {reason}")
            }
            HealthError::UnmappableHostError { name } => {
                write!(
                    f,
                    "host error '{name}' is not a plugin-health input in the closed exhaustion \
                     vocabulary; it cannot be folded into a health observation"
                )
            }
        }
    }
}

impl std::error::Error for HealthError {}

// --- Stats (bounded, clonable) ------------------------------------------------------

/// The per-plugin invocation counters — a FIXED-SIZE, clonable snapshot
/// (no unbounded memory: there is no per-call record anywhere; the error
/// samples that feed the error-rate derivation are pruned to the window
/// and hard-capped). `Copy` makes every returned snapshot an independent,
/// bounded copy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PluginStats {
    /// Calls through the binding seam (successful or failed).
    pub calls_total: u64,
    /// Failed calls (including timeouts).
    pub errors_total: u64,
    /// Calls that hit the wall-clock deadline.
    pub timeouts_total: u64,
    /// Accumulated metered-instruction consumption (u64; saturating
    /// accumulation — the counter never overflows into a wrong small
    /// number).
    pub fuel_used: u64,
    /// The injected-clock reading of the last health-state transition
    /// (`None` before the first transition).
    pub last_transition_unix_millis: Option<u64>,
}

// --- The per-plugin track ------------------------------------------------------------

/// The observed state of ONE plugin (the monitor's private record). Every
/// field is observation-fed; there is no default-healthy path anywhere.
#[derive(Debug, Clone, PartialEq)]
struct PluginTrack {
    version: String,
    /// Whether ANY observation has landed (the UNVERIFIED gate).
    observed: bool,
    /// The last time the plugin was SEEN responsive (successful call or
    /// passed probe; unix millis).
    last_seen_unix_millis: Option<u64>,
    /// The last responsive interaction FAILED (a failed liveness probe, or
    /// — mirrored from the P6.17 track — any observed unresponsiveness);
    /// cleared by a later successful call or passed probe.
    unresponsive: bool,
    /// The last failed-call reason (human-safe).
    last_failure_reason: Option<String>,
    /// The latched exhaustion reason (`None` = none latched; cleared by a
    /// later successful call — the recovery rule).
    exhausted: Option<HealthReason>,
    /// The bindings verdict latched (`true` once
    /// `HostError::BindingsNotGenerated` is observed).
    bindings_failed: bool,
    /// Error timestamps inside the bounded window (pruned on assess;
    /// hard-capped).
    error_times: Vec<u64>,
    /// Readiness (never READY before the first successful call or a passed
    /// probe).
    readiness: ReadinessState,
    /// The bounded counters.
    stats: PluginStats,
}

impl PluginTrack {
    fn new(version: &str) -> Self {
        Self {
            version: version.to_string(),
            observed: false,
            last_seen_unix_millis: None,
            unresponsive: false,
            last_failure_reason: None,
            exhausted: None,
            bindings_failed: false,
            error_times: Vec::new(),
            readiness: ReadinessState::Unobserved,
            stats: PluginStats::default(),
        }
    }
}

// --- Audit port and records ----------------------------------------------------------

/// Which kind of health audit record this is (stable machine names).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthAuditKind {
    /// The plugin was registered into the monitor.
    Registration,
    /// A health-state transition (the `from`/`to` names carry the closed
    /// vocabulary states).
    Transition,
    /// An observation for an unregistered plugin was REFUSED (fail-closed;
    /// the caller's wiring is wrong and must be loud).
    Refused,
}

impl HealthAuditKind {
    pub fn name(self) -> &'static str {
        match self {
            HealthAuditKind::Registration => "REGISTRATION",
            HealthAuditKind::Transition => "TRANSITION",
            HealthAuditKind::Refused => "REFUSED",
        }
    }
}

/// One health audit record (§21.3 correlation discipline: plugin
/// id/version, transition, reason, timestamp — identifiers only; no config
/// values, no secrets).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginHealthAuditRecord {
    pub kind: HealthAuditKind,
    pub plugin_id: String,
    pub version: String,
    /// The state before the transition (`None` for registration).
    pub from: Option<String>,
    /// The state after the transition (`None` for registration).
    pub to: Option<String>,
    pub reason: String,
    pub timestamp_unix_millis: u64,
}

/// The health audit sink (host-local port — this crate depends on no
/// flexcore crate, the `broker.rs` §6 seam pattern). Implementations MUST
/// be append-only; `&self` + interior mutability so the monitor can hold a
/// shared reference.
pub trait HealthAuditSink {
    fn record(&self, record: PluginHealthAuditRecord);
}

/// An append-only in-memory [`HealthAuditSink`] — the deterministic
/// collector for tests and in-process compositions. Ordering is exactly
/// the emission order.
#[derive(Debug, Default)]
pub struct MemoryHealthAuditSink {
    records: Mutex<Vec<PluginHealthAuditRecord>>,
}

impl MemoryHealthAuditSink {
    pub fn new() -> MemoryHealthAuditSink {
        MemoryHealthAuditSink { records: Mutex::new(Vec::new()) }
    }

    /// The records in emission order (append-only).
    pub fn records(&self) -> Vec<PluginHealthAuditRecord> {
        match self.records.lock() {
            Ok(guard) => guard.clone(),
            Err(_) => Vec::new(),
        }
    }
}

impl HealthAuditSink for MemoryHealthAuditSink {
    fn record(&self, record: PluginHealthAuditRecord) {
        if let Ok(mut guard) = self.records.lock() {
            guard.push(record);
        }
    }
}

// --- The health record and snapshot ---------------------------------------------------

/// One plugin's health, as of one assessment. Plain data — the P7.25
/// console consumes it through the snapshot (the transport is the recorded
/// gRPC/REST seam, not implemented here).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginHealthRecord {
    pub plugin_id: String,
    pub version: String,
    /// The closed-vocabulary status (`HEALTHY` / `DEGRADED_*` /
    /// `FAILING_*`).
    pub state: HealthState,
    /// The closed readiness state (`UNOBSERVED` / `PROBE_FAILED` / `READY`).
    pub readiness: ReadinessState,
    /// Last seen responsive (unix millis), when observed at all.
    pub last_seen_unix_millis: Option<u64>,
    /// The bounded counters snapshot.
    pub stats: PluginStats,
    /// The human-safe detail behind the state (never raw backend text, no
    /// config values, no secrets).
    pub detail: String,
}

impl PluginHealthRecord {
    /// The status name string (the closed vocabulary, rendered).
    pub fn status_name(&self) -> String {
        self.state.name()
    }

    /// Whether dependent actions are blocked (only `FAILING_*` blocks).
    pub fn blocks_dependent_actions(&self) -> bool {
        self.state.blocks_dependent_actions()
    }
}

/// The monitor-wide snapshot (P7.25's data source for the Plugins &
/// Integrations workspace — the gRPC/REST transport and the mTLS identity
/// on it are the recorded P7.20/P7.34 seams).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HealthSnapshot {
    /// One record per registered plugin, ordered by plugin id (stable
    /// rendering).
    pub plugins: Vec<PluginHealthRecord>,
}

impl HealthSnapshot {
    /// A flat JSON object rendering (hand-rolled, std-only — this crate
    /// carries no serde dependency in its non-wasmtime code; the strings
    /// are escaped). Deterministic: plugin order is the sorted track
    /// order, keys are in fixed positions.
    pub fn json(&self) -> String {
        let records: Vec<String> = self.plugins.iter().map(render_record_json).collect();
        format!("{{\"plugins\":[{}]}}", records.join(","))
    }
}

/// Escapes a string for the JSON rendering (the two mandatory escapes plus
/// the control characters a config string could plausibly carry — the
/// P6.17 `json_string` helper, mirrored).
fn json_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for character in value.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn render_record_json(record: &PluginHealthRecord) -> String {
    let last_seen = match record.last_seen_unix_millis {
        Some(t) => t.to_string(),
        None => "null".to_string(),
    };
    let last_transition = match record.stats.last_transition_unix_millis {
        Some(t) => t.to_string(),
        None => "null".to_string(),
    };
    format!(
        "{{\"plugin_id\":{},\"version\":{},\"status\":{},\"readiness\":{},\
         \"last_seen_unix_millis\":{},\"calls_total\":{},\"errors_total\":{},\
         \"timeouts_total\":{},\"fuel_used\":{},\"last_transition_unix_millis\":{},\
         \"detail\":{}}}",
        json_string(&record.plugin_id),
        json_string(&record.version),
        json_string(&record.status_name()),
        json_string(record.readiness.name()),
        last_seen,
        record.stats.calls_total,
        record.stats.errors_total,
        record.stats.timeouts_total,
        record.stats.fuel_used,
        last_transition,
        json_string(&record.detail),
    )
}

// --- The monitor ---------------------------------------------------------------------

/// The per-plugin health monitor. Composed by the host's call/probe hooks:
/// the host feeds [`PluginHealthMonitor::register`],
/// [`PluginHealthMonitor::record_call_result`],
/// [`PluginHealthMonitor::record_exhaustion`],
/// [`PluginHealthMonitor::record_readiness_probe`], and
/// [`PluginHealthMonitor::record_liveness_probe`]; the console path reads
/// [`PluginHealthMonitor::snapshot`].
pub struct PluginHealthMonitor<'a> {
    limits: HealthLimits,
    tracks: BTreeMap<String, PluginTrack>,
    audit: &'a dyn HealthAuditSink,
}

impl<'a> PluginHealthMonitor<'a> {
    /// Builds the monitor over its limits and audit sink.
    pub fn new(
        limits: HealthLimits,
        audit: &'a dyn HealthAuditSink,
    ) -> Result<Self, HealthError> {
        limits.validate()?;
        Ok(Self { limits, tracks: BTreeMap::new(), audit })
    }

    /// The limits (visible to composition and tests).
    pub fn limits(&self) -> &HealthLimits {
        &self.limits
    }

    fn track(&mut self, plugin_id: &str) -> Option<&mut PluginTrack> {
        self.tracks.get_mut(plugin_id)
    }

    /// Registers a plugin into the monitor: the track starts UNOBSERVED
    /// (DEGRADED_UNVERIFIED, never healthy) and the registration is
    /// audited. Re-registration of a known id REPLACES the track (a
    /// re-instantiated plugin is a fresh health story — the lifecycle's
    /// fresh-registration rule; the replacement is audited as a
    /// Registration record).
    pub fn register(&mut self, plugin_id: &str, version: &str, now_unix_millis: u64) {
        self.tracks.insert(plugin_id.to_string(), PluginTrack::new(version));
        self.audit.record(PluginHealthAuditRecord {
            kind: HealthAuditKind::Registration,
            plugin_id: plugin_id.to_string(),
            version: version.to_string(),
            from: None,
            to: None,
            reason: "plugin registered into the health monitor (UNOBSERVED, never healthy-by-default)"
                .to_string(),
            timestamp_unix_millis: now_unix_millis,
        });
    }

    /// Records ONE call observation (the host's binding-seam hook). A
    /// successful call stamps `last_seen`, clears the latched exhaustion
    /// and the failed-call reason (the recovery rule), and makes the plugin
    /// READY if it was not yet (the first-success rule). A failed/timed-out
    /// call stamps an error sample and the last-failure reason; a timeout
    /// additionally counts in the timeouts counter. Unknown plugin →
    /// refused (audited) with [`HealthError::UnknownPlugin`].
    pub fn record_call_result(
        &mut self,
        plugin_id: &str,
        now_unix_millis: u64,
        observation: PluginObservation,
    ) -> Result<(), HealthError> {
        if !self.tracks.contains_key(plugin_id) {
            return Err(self.refuse_unknown(plugin_id, now_unix_millis, "call observation"));
        }
        // The track exists (checked above); the None arm is unreachable and
        // touches nothing borrowed from self.
        let track = match self.track(plugin_id) {
            Some(track) => track,
            None => {
                return Err(HealthError::UnknownPlugin {
                    plugin_id: plugin_id.to_string(),
                })
            }
        };
        track.observed = true;
        track.stats.calls_total = track.stats.calls_total.saturating_add(1);
        match observation {
            PluginObservation::CallSucceeded { fuel_used } => {
                track.last_seen_unix_millis = Some(now_unix_millis);
                track.unresponsive = false;
                track.last_failure_reason = None;
                track.exhausted = None;
                track.stats.fuel_used = track.stats.fuel_used.saturating_add(fuel_used);
                if track.readiness != ReadinessState::Ready {
                    // The first successful call is the readiness gate (the
                    // probe-failed latch is a separate record, not cleared
                    // here — see the module header).
                    track.readiness = ReadinessState::Ready;
                }
            }
            PluginObservation::CallFailed { reason } => {
                track.stats.errors_total = track.stats.errors_total.saturating_add(1);
                track.last_failure_reason = Some(reason);
                track.error_times.push(now_unix_millis);
                Self::prune_error_samples(&mut track.error_times);
            }
            PluginObservation::CallTimedOut { reason } => {
                track.stats.errors_total = track.stats.errors_total.saturating_add(1);
                track.stats.timeouts_total = track.stats.timeouts_total.saturating_add(1);
                track.last_failure_reason = Some(reason);
                track.error_times.push(now_unix_millis);
                Self::prune_error_samples(&mut track.error_times);
            }
        }
        Ok(())
    }

    /// Records a resource-exhaustion event from the `limits.rs` enforcement
    /// path (the closed mapping). Unmappable host errors are a NAMED
    /// refusal — never folded into healthy. An exhaustion DEGRADES the
    /// plugin and LATCHES until a later successful call clears it.
    pub fn record_exhaustion(
        &mut self,
        plugin_id: &str,
        now_unix_millis: u64,
        error: &HostError,
    ) -> Result<(), HealthError> {
        let Some(reason) = exhaustion_reason(error) else {
            return Err(HealthError::UnmappableHostError { name: error.name() });
        };
        if !self.tracks.contains_key(plugin_id) {
            return Err(self.refuse_unknown(plugin_id, now_unix_millis, "exhaustion observation"));
        }
        let track = match self.track(plugin_id) {
            Some(track) => track,
            None => {
                return Err(HealthError::UnknownPlugin {
                    plugin_id: plugin_id.to_string(),
                })
            }
        };
        track.observed = true;
        track.stats.errors_total = track.stats.errors_total.saturating_add(1);
        // The plugin RAN until it exhausted the limit — that is an
        // observation of the plugin answering (it stamps liveness); the
        // instance teardown is the limits layer's contract.
        track.last_seen_unix_millis = Some(now_unix_millis);
        if reason == HealthReason::Bindings {
            track.bindings_failed = true;
        } else {
            track.exhausted = Some(reason);
        }
        track.error_times.push(now_unix_millis);
        Self::prune_error_samples(&mut track.error_times);
        Ok(())
    }

    /// Records an explicit READINESS probe through the binding seam (the
    /// host's probe hook). A passed probe makes the plugin READY; a failed
    /// probe latches `PROBE_FAILED` (its own recorded verdict).
    pub fn record_readiness_probe(
        &mut self,
        plugin_id: &str,
        now_unix_millis: u64,
        passed: bool,
    ) -> Result<(), HealthError> {
        if !self.tracks.contains_key(plugin_id) {
            return Err(self.refuse_unknown(plugin_id, now_unix_millis, "readiness probe"));
        }
        let track = match self.track(plugin_id) {
            Some(track) => track,
            None => {
                return Err(HealthError::UnknownPlugin {
                    plugin_id: plugin_id.to_string(),
                })
            }
        };
        track.observed = true;
        if passed {
            track.readiness = ReadinessState::Ready;
            track.last_seen_unix_millis = Some(now_unix_millis);
            track.unresponsive = false;
        } else {
            track.readiness = ReadinessState::ProbeFailed;
        }
        Ok(())
    }

    /// Records an explicit LIVENESS probe (the host's epoch/probe hook).
    /// A failed liveness probe is an observation of unresponsiveness —
    /// FAILING_UNRESPONSIVE at the next assessment (dependent actions
    /// blocked).
    pub fn record_liveness_probe(
        &mut self,
        plugin_id: &str,
        now_unix_millis: u64,
        alive: bool,
    ) -> Result<(), HealthError> {
        if !self.tracks.contains_key(plugin_id) {
            return Err(self.refuse_unknown(plugin_id, now_unix_millis, "liveness probe"));
        }
        let track = match self.track(plugin_id) {
            Some(track) => track,
            None => {
                return Err(HealthError::UnknownPlugin {
                    plugin_id: plugin_id.to_string(),
                })
            }
        };
        track.observed = true;
        if alive {
            track.last_seen_unix_millis = Some(now_unix_millis);
            track.unresponsive = false;
        } else {
            // A failed liveness probe is an OBSERVATION of failure (the
            // P6.17 rule: the failed check marks the last responsive check
            // as failed; it does not move last_seen — nothing was seen).
            track.unresponsive = true;
            track.last_failure_reason =
                Some("the host's liveness probe reported the plugin unresponsive".to_string());
            track.error_times.push(now_unix_millis);
            Self::prune_error_samples(&mut track.error_times);
        }
        Ok(())
    }

    /// The readiness verdict for one plugin. An unregistered plugin is a
    /// named error — readiness unknown, never ready.
    pub fn readiness(&self, plugin_id: &str) -> Result<ReadinessState, HealthError> {
        self.tracks
            .get(plugin_id)
            .map(|track| track.readiness)
            .ok_or_else(|| HealthError::UnknownPlugin {
                plugin_id: plugin_id.to_string(),
            })
    }

    /// Assesses ONE plugin's health AS OF `now_unix_millis`. Derivation
    /// order (first match wins; FAILING outranks DEGRADED; every rule is a
    /// positive check on observed inputs — the P6.17 order, adapted):
    ///
    /// 1. never observed → `DEGRADED_UNVERIFIED` (never healthy);
    /// 2. bindings failed → `FAILING_BINDINGS`;
    /// 3. last responsive interaction outside the detection window, or a
    ///    failed liveness probe → `FAILING_UNRESPONSIVE`;
    /// 4. exhaustion latched → `DEGRADED_<reason>`;
    /// 5. error rate at/above the threshold in the bounded window →
    ///    `DEGRADED_ERROR_RATE`;
    /// 6. otherwise — and ONLY otherwise — `HEALTHY`.
    pub fn assess(
        &self,
        plugin_id: &str,
        now_unix_millis: u64,
    ) -> Result<PluginHealthRecord, HealthError> {
        let track = self.tracks.get(plugin_id).ok_or_else(|| HealthError::UnknownPlugin {
            plugin_id: plugin_id.to_string(),
        })?;
        let version = track.version.clone();
        let readiness = track.readiness;
        let last_seen = track.last_seen_unix_millis;
        let stats = track.stats;
        // A total record builder over the cloned track facts (clones the
        // version per call — the builder is reusable across the rule arms).
        let base = move |state: HealthState, detail: String| {
            Ok(PluginHealthRecord {
                plugin_id: plugin_id.to_string(),
                version: version.clone(),
                state,
                readiness,
                last_seen_unix_millis: last_seen,
                stats,
                detail,
            })
        };
        // 1. The Unverified gate — unknown is NEVER healthy.
        if !track.observed {
            return base(
                HealthState::Degraded(HealthReason::Unverified),
                "no observation has arrived yet; health is UNVERIFIED, never assumed healthy"
                    .to_string(),
            );
        }
        // 2. Bindings failure — the plugin cannot run at all.
        if track.bindings_failed {
            return base(
                HealthState::Failing(HealthReason::Bindings),
                "the bindings seam reports the plugin cannot run in this build \
                 (BINDINGS_NOT_GENERATED)"
                    .to_string(),
            );
        }
        // 3. Liveness (the failed-check flag, or the detection window on
        //    the last seen fact).
        let seen_recently = track
            .last_seen_unix_millis
            .map(|last| now_unix_millis.saturating_sub(last) <= self.limits.unresponsive_after_millis)
            .unwrap_or(false);
        if track.unresponsive || !seen_recently {
            return base(
                HealthState::Failing(HealthReason::Unresponsive),
                track
                    .last_failure_reason
                    .clone()
                    .unwrap_or_else(|| "no successful interaction within the detection window"
                        .to_string()),
            );
        }
        // 4. Latched exhaustion.
        if let Some(reason) = track.exhausted {
            return base(
                HealthState::Degraded(reason),
                format!(
                    "resource exhaustion latched ({}) — the limits layer tore the instance down; \
                     health degrades until a successful call clears it",
                    reason.token()
                ),
            );
        }
        // 5. Error rate over the bounded window.
        let window_start = now_unix_millis.saturating_sub(self.limits.error_rate_window_millis);
        let errors_in_window = track
            .error_times
            .iter()
            .filter(|&&at| at > window_start && at <= now_unix_millis)
            .count();
        if errors_in_window as u32 >= self.limits.max_errors_in_window {
            return base(
                HealthState::Degraded(HealthReason::ErrorRate),
                format!(
                    "{errors_in_window} errors inside the {} ms window (threshold {})",
                    self.limits.error_rate_window_millis, self.limits.max_errors_in_window
                ),
            );
        }
        // 6. HEALTHY — reached only through every positive check above.
        base(
            HealthState::Healthy,
            "responsive within the detection window, error rate inside limits, no exhaustion \
             latched"
                .to_string(),
        )
    }

    /// Whether a dependent action against the plugin is allowed AS OF
    /// `now`. Only `FAILING_*` blocks; the block reason is returned for
    /// the caller's AUDITED refusal (reject + audit, never silent).
    pub fn check_action_allowed(
        &self,
        plugin_id: &str,
        now_unix_millis: u64,
    ) -> Result<(), String> {
        let record = self.assess(plugin_id, now_unix_millis)?;
        if record.blocks_dependent_actions() {
            Err(format!(
                "dependent action against plugin '{}' is BLOCKED while health is {} \
                 (dependent actions are blocked on FAILING_*): {}",
                plugin_id,
                record.status_name(),
                record.detail
            ))
        } else {
            Ok(())
        }
    }

    /// The snapshot across ALL registered plugins, ordered by plugin id
    /// (stable rendering for the P7.25 console seam). Every registered
    /// track assesses successfully here (assessment over a registered id
    /// cannot return `UnknownPlugin`), so the snapshot is total.
    pub fn snapshot(&self, now_unix_millis: u64) -> HealthSnapshot {
        // BTreeMap iteration: sorted plugin ids (determinism).
        let mut plugins = Vec::with_capacity(self.tracks.len());
        for (id, _track) in &self.tracks {
            let record = self
                .assess(id, now_unix_millis)
                .unwrap_or_else(|_error| PluginHealthRecord {
                    plugin_id: id.clone(),
                    version: String::new(),
                    state: HealthState::Degraded(HealthReason::Unverified),
                    readiness: ReadinessState::Unobserved,
                    last_seen_unix_millis: None,
                    stats: PluginStats::default(),
                    detail: format!("health assessment failed: {_error}"),
                });
            plugins.push(record);
        }
        HealthSnapshot { plugins }
    }

    // -- Internals -----------------------------------------------------------

    /// Emits the audited refusal for an observation on an unregistered
    /// plugin and returns the named error.
    fn refuse_unknown(&self, plugin_id: &str, now_unix_millis: u64, what: &str) -> HealthError {
        self.audit.record(PluginHealthAuditRecord {
            kind: HealthAuditKind::Refused,
            plugin_id: plugin_id.to_string(),
            version: String::new(),
            from: None,
            to: None,
            reason: format!("{what} refused: the plugin was never registered (fail-closed)"),
            timestamp_unix_millis: now_unix_millis,
        });
        HealthError::UnknownPlugin {
            plugin_id: plugin_id.to_string(),
        }
    }

    /// Drops the oldest error samples beyond the hard cap (bounded memory;
    /// the dropped samples are still counted in `errors_total`).
    fn prune_error_samples(samples: &mut Vec<u64>) {
        if samples.len() > MAX_ERROR_SAMPLES {
            let excess = samples.len() - MAX_ERROR_SAMPLES;
            samples.drain(..excess);
        }
    }
}

// Transition auditing: the monitor derives state transitions by comparing
// the assessment BEFORE and AFTER an observation is applied. The host's
// composition loop (or the test) calls [`PluginHealthMonitor::audit_transitions`]
// with the `before` snapshot; transitions between the two assessments are
// emitted as Transition records. This keeps the monitor pure (no interior
// state machine to drift) while making every transition auditable.
impl<'a> PluginHealthMonitor<'a> {
    /// Emits a Transition audit record for every plugin whose status
    /// changed between `before` (a snapshot taken before the observations)
    /// and the current assessment at `now_unix_millis`. Records are
    /// emitted in sorted plugin-id order (determinism).
    pub fn audit_transitions(&self, before: &HealthSnapshot, now_unix_millis: u64) {
        for record in &self.snapshot(now_unix_millis).plugins {
            if let Some(previous) = before.plugins.iter().find(|p| p.plugin_id == record.plugin_id)
            {
                if previous.status_name() != record.status_name() {
                    self.audit.record(PluginHealthAuditRecord {
                        kind: HealthAuditKind::Transition,
                        plugin_id: record.plugin_id.clone(),
                        version: record.version.clone(),
                        from: Some(previous.status_name()),
                        to: Some(record.status_name()),
                        reason: record.detail.clone(),
                        timestamp_unix_millis: now_unix_millis,
                    });
                }
            }
        }
    }
}
