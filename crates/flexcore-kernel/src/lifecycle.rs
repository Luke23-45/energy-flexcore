// P7.07 — Plugin lifecycle state machine with drain-before-replace (report
// §5.6, §14.1 by analogy, §6.4 state-machine pattern; ledger
// `ledger_phase_7.md` P7.07).
//
// The §5.6 state diagram is implemented as an EXPLICIT state machine: the
// states are a closed enum mirroring the report's list exactly, the
// transition table is a frozen constant ([`FROZEN_TRANSITIONS`]) copied from
// the §5.6 diagram, and every transition outside the table is a DISTINCT
// named error ([`LifecycleError::IllegalTransition`], name
// `ILLEGAL_TRANSITION`) — never a warning, never a silent no-op. The §5.6
// gate conditions (signature/provenance at `Verified`, manifest/API
// compatibility at `Compatible`, schema + secret references at `Configured`,
// dependency graph resolved at `Started`, health checks at `Ready`) are
// enforced as required gate outcomes on the forward transitions; a FAILED
// gate refuses the transition (fail-closed, state unchanged) and is
// audited — the §5.6 exit edges (`Rejected`/`Failed`) are then applied
// explicitly by [`PluginLifecycle::reject`] / [`PluginLifecycle::fail`].
//
// Drain-before-replace (the core rule, §5.6): a plugin being replaced moves
// `Ready → Draining`, stops accepting new work, and finishes or cancels
// outstanding operations through its [`DrainPort`] before it may reach
// `Stopped`. A replacement may only start once the incumbent is fully
// drained ([`LifecycleState::Stopped`]); anything earlier is the NAMED
// error [`LifecycleError::ReplacementBlockedUndrained`] — never a warning.
// A drain that hits its configured deadline is REFUSED (fail-closed): the
// `Draining → Stopped` transition is not committed, the incumbent is
// restored to `Ready` (active) pending an explicit operator decision, and
// the refusal is audited. There is no forced stop.
//
// Crash handling: a plugin fails into [`LifecycleState::Failed`] and STAYS
// there. The restart policy for v0.5.0 is EXPLICIT ONLY (DEFAULT decision,
// recorded in the decision record): there is no auto-restart anywhere in
// this module; the only path out of `Failed` is the audited explicit
// restart ([`PluginLifecycle::restart_explicitly`]), which starts a fresh
// lifecycle at `Discovered`. The failed lifecycle retains the state it
// failed from ([`PluginLifecycle::failed_from`]) as evidence for the
// P7.30 failure-mode drill.
//
// Audit hooks (§5.6 step 4; §5.1 audit integrity): EVERY transition —
// committed or refused — emits a [`LifecycleAuditRecord`] through the
// in-module [`AuditSink`] port. This port is deliberately DEFINED HERE, not
// in `flexcore-ports`: the kernel depends on no `flexcore-*` crate (the
// P7.05/P7.06 dependency decision in the crate's `Cargo.toml`), and the
// platform audit sink (`flexcore-audit::AuditSink`) and the edge-local one
// (`edge-agent::ports::AuditSink`) are different traits with the same name —
// the seam inventory (P7.04 §3.8) records that naming collision; this is a
// third, kernel-local port feeding the host's audit path. Timestamps come
// ONLY from the injected [`Clock`] — `SystemTime::now()` appears nowhere in
// this module (P7.42 determinism: same clock → same audit ordering).
//
// Determinism: all iteration is over fixed-size constant tables or
// `BTreeMap`-ordered data; the transition check is a pure predicate. No
// `unwrap`/`expect` on untrusted input anywhere.

use std::cell::Cell;
use std::fmt;

// --- States (closed enum — the §5.6 list, mirrored exactly) ----------------------

/// The plugin lifecycle states (report §5.6 diagram). CLOSED: adding a state
/// is a reviewed schema change to the frozen table, never an extension by a
/// caller. Ordering is declaration order for display only; no semantic
/// ordering is implied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LifecycleState {
    Discovered,
    Verified,
    Compatible,
    Configured,
    Started,
    Ready,
    Draining,
    Stopped,
    Rejected,
    Failed,
}

pub const LIFECYCLE_STATES: [LifecycleState; 10] = [
    LifecycleState::Discovered,
    LifecycleState::Verified,
    LifecycleState::Compatible,
    LifecycleState::Configured,
    LifecycleState::Started,
    LifecycleState::Ready,
    LifecycleState::Draining,
    LifecycleState::Stopped,
    LifecycleState::Rejected,
    LifecycleState::Failed,
];

impl LifecycleState {
    /// The stable machine-readable state name (the audit-record value).
    pub fn name(&self) -> &'static str {
        match self {
            LifecycleState::Discovered => "DISCOVERED",
            LifecycleState::Verified => "VERIFIED",
            LifecycleState::Compatible => "COMPATIBLE",
            LifecycleState::Configured => "CONFIGURED",
            LifecycleState::Started => "STARTED",
            LifecycleState::Ready => "READY",
            LifecycleState::Draining => "DRAINING",
            LifecycleState::Stopped => "STOPPED",
            LifecycleState::Rejected => "REJECTED",
            LifecycleState::Failed => "FAILED",
        }
    }

    /// Terminal states: no outgoing transition exists in the frozen table
    /// (except the explicit-restart OPERATION out of `Failed`, which is not
    /// a §5.6 edge — see [`PluginLifecycle::restart_explicitly`]).
    pub fn is_terminal(&self) -> bool {
        matches!(self, LifecycleState::Stopped | LifecycleState::Rejected | LifecycleState::Failed)
    }
}

impl fmt::Display for LifecycleState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

// --- The frozen transition table (report §5.6, verbatim edges) -------------------

/// The §5.6 transitions, FROZEN. Each entry is `(from, to)`; the comment is
/// the diagram's guard label. Changing this table is a reviewed baseline
/// change (AGENTS.md §3 rule 6), never a local edit.
pub const FROZEN_TRANSITIONS: [(LifecycleState, LifecycleState); 12] = [
    (LifecycleState::Discovered, LifecycleState::Verified),   // signature / provenance accepted
    (LifecycleState::Verified, LifecycleState::Compatible),   // manifest and API compatibility pass
    (LifecycleState::Compatible, LifecycleState::Configured), // schema and secret references valid
    (LifecycleState::Configured, LifecycleState::Started),    // dependency graph resolved
    (LifecycleState::Started, LifecycleState::Ready),         // health checks pass
    (LifecycleState::Ready, LifecycleState::Draining),        // disable / upgrade requested
    (LifecycleState::Draining, LifecycleState::Stopped),      // active work completed or cancelled
    (LifecycleState::Verified, LifecycleState::Rejected),     // verification failure
    (LifecycleState::Compatible, LifecycleState::Rejected),   // incompatible API
    (LifecycleState::Started, LifecycleState::Failed),        // initialization failure
    (LifecycleState::Ready, LifecycleState::Failed),          // health / runtime failure
    (LifecycleState::Failed, LifecycleState::Draining),       // failed plugin drained before removal
];

/// Crash-path edges (DEFAULT decision, recorded in the decision record):
/// the §5.6 diagram names `Failed` exits from `Started` and `Ready` only,
/// but a plugin can crash in ANY non-terminal state. This module adds the
/// remaining `→ Failed` edges so a crash is always representable (fail-closed
/// crash handling; a crash that could not be recorded would look healthy).
/// `Started → Failed` and `Ready → Failed` are already in the frozen table;
/// they are not duplicated here. `Rejected`/`Stopped`/`Failed` are terminal
/// and have no crash edge.
pub const CRASH_TRANSITIONS: [(LifecycleState, LifecycleState); 5] = [
    (LifecycleState::Discovered, LifecycleState::Failed),
    (LifecycleState::Verified, LifecycleState::Failed),
    (LifecycleState::Compatible, LifecycleState::Failed),
    (LifecycleState::Configured, LifecycleState::Failed),
    (LifecycleState::Draining, LifecycleState::Failed),
];

/// Whether `from → to` is a legal transition (frozen table + crash edges).
/// Pure predicate; the machine never consults anything else.
pub fn is_legal_transition(from: LifecycleState, to: LifecycleState) -> bool {
    FROZEN_TRANSITIONS.iter().any(|(f, t)| *f == from && *t == to)
        || CRASH_TRANSITIONS.iter().any(|(f, t)| *f == from && *t == to)
}

// --- Gates (the §5.6 step-2 preconditions, as required outcomes) ------------------

/// The outcome of one §5.6 gate check. The lifecycle does not itself verify
/// signatures or run health checks — the HOST does, and hands the outcome in;
/// the machine refuses to advance on anything but [`GateOutcome::Passed`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GateOutcome {
    Passed,
    Failed { reason: String },
}

impl GateOutcome {
    pub fn passed(&self) -> bool {
        matches!(self, GateOutcome::Passed)
    }

    pub fn failure_reason(&self) -> Option<&str> {
        match self {
            GateOutcome::Passed => None,
            GateOutcome::Failed { reason } => Some(reason.as_str()),
        }
    }
}

/// Gate names (stable, machine-readable; they appear in audit records and
/// error payloads).
pub const GATE_PROVENANCE: &str = "PROVENANCE";
pub const GATE_MANIFEST_API_COMPATIBILITY: &str = "MANIFEST_API_COMPATIBILITY";
pub const GATE_SCHEMA_SECRETS: &str = "SCHEMA_SECRETS";
pub const GATE_DEPENDENCY_GRAPH: &str = "DEPENDENCY_GRAPH";
pub const GATE_HEALTH: &str = "HEALTH";

// --- Drain port (the plugin-implemented quiescence seam) --------------------------

/// How a drain attempt ended. Fail-closed: an implementation that cannot
/// guarantee quiescence reports [`DrainOutcome::DeadlineExceeded`] rather
/// than pretending to be quiet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DrainOutcome {
    /// No outstanding work remains at or before the deadline; `cancelled` is
    /// the number of operations cancelled (rather than finished) on the way.
    Quiesced { cancelled: usize },
    /// The deadline was reached with work still outstanding.
    DeadlineExceeded { outstanding: usize },
}

/// The drain port a replaceable plugin implements (P7.07 step 3). The
/// lifecycle calls it exactly once per drain, with the configured deadline.
pub trait DrainPort {
    /// Outstanding (accepted, not yet finished or cancelled) work items.
    fn outstanding_work(&self) -> usize;
    /// Stop accepting new work, finish or cancel the outstanding operations,
    /// and report the outcome. `deadline_millis` is the injected clock's
    /// millisecond reading at which the drain must be complete.
    fn drain(&mut self, deadline_millis: u64) -> DrainOutcome;
}

// --- Drain policy -----------------------------------------------------------------

/// DEFAULT drain deadline: 30 seconds of injected-clock time. A DEFAULT
/// decision (recorded in the decision record): the §5.6 report text gives no
/// number; 30 s bounds a schedule-length drain without hiding a stuck plugin
/// for minutes. Overridable per composition via [`DrainPolicy`].
pub const DEFAULT_DRAIN_DEADLINE_MS: u64 = 30_000;

/// Per-plugin drain configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DrainPolicy {
    /// Drain deadline in milliseconds of [`Clock`] time, relative to the
    /// clock reading at the moment the drain runs.
    pub deadline_ms: u64,
}

impl Default for DrainPolicy {
    fn default() -> DrainPolicy {
        DrainPolicy { deadline_ms: DEFAULT_DRAIN_DEADLINE_MS }
    }
}

// --- Injected clock (P7.42 determinism) --------------------------------------------

/// The lifecycle's time source. Kernel-local mirror of the platform clock
/// port pattern (`flexcore-ports::clock::Clock` via `crates/edge-agent/src/
/// clock.rs`) narrowed to what the lifecycle needs; the kernel depends on no
/// `flexcore-*` crate, so the port is defined here and satisfied by
/// composition. NEVER implemented over `SystemTime::now()` inside this
/// crate — wall-clock reading happens in the adapter the composition root
/// injects.
pub trait Clock {
    /// Milliseconds since the implementation's epoch. Monotonicity across
    /// calls is the implementation's contract; the lifecycle never rewinds.
    fn now_millis(&self) -> u64;
}

/// A fixed, advanceable clock: the deterministic implementation for tests
/// and for replay (P7.42). Two runs with the same readings produce the same
/// audit ordering and timestamps.
#[derive(Debug, Default)]
pub struct FixedClock {
    millis: Cell<u64>,
}

impl FixedClock {
    pub fn new(millis: u64) -> FixedClock {
        FixedClock { millis: Cell::new(millis) }
    }

    pub fn set(&self, millis: u64) {
        self.millis.set(millis);
    }

    pub fn advance_by(&self, millis: u64) {
        self.millis.set(self.millis.get().saturating_add(millis));
    }

    pub fn reading(&self) -> u64 {
        self.millis.get()
    }
}

impl Clock for FixedClock {
    fn now_millis(&self) -> u64 {
        self.millis.get()
    }
}

// --- Audit port and records ---------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleAuditKind {
    /// Entry into the lifecycle (registration at `Discovered`).
    Registration,
    /// A committed transition (frozen table or crash edges).
    Transition,
    /// A refused transition or operation (fail-closed; state unchanged or
    /// restored — the record's `from`/`to` name the ATTEMPTED edge).
    Refused,
    /// The version change of a replacement, recorded after a completed
    /// drain (P7.07 step 3: "record the version change, then replace").
    VersionChange,
    /// The EXPLICIT restart out of `Failed` (the only restart path in v0.5.0).
    ExplicitRestart,
}

impl LifecycleAuditKind {
    pub fn name(&self) -> &'static str {
        match self {
            LifecycleAuditKind::Registration => "REGISTRATION",
            LifecycleAuditKind::Transition => "TRANSITION",
            LifecycleAuditKind::Refused => "REFUSED",
            LifecycleAuditKind::VersionChange => "VERSION_CHANGE",
            LifecycleAuditKind::ExplicitRestart => "EXPLICIT_RESTART",
        }
    }
}

impl fmt::Display for LifecycleAuditKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// One lifecycle audit record (§5.6 step 4: every transition, with plugin
/// ID/version, actor, and reason; timestamp from the injected clock).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleAuditRecord {
    pub kind: LifecycleAuditKind,
    pub plugin_id: String,
    pub version: String,
    /// `None` for the registration record (entry into the lifecycle).
    pub from: Option<LifecycleState>,
    /// `None` for version-change records (no state moves).
    pub to: Option<LifecycleState>,
    pub actor: String,
    pub reason: String,
    pub timestamp_millis: u64,
}

/// The lifecycle audit sink (kernel-local port; see the module header for
/// why it is not `flexcore-audit::AuditSink`). Implementations MUST be
/// append-only (§5.1 audit integrity): records are never rewritten or
/// removed. `&self` + interior mutability, so the lifecycle can hold a
/// shared reference for its whole life.
pub trait AuditSink {
    fn record(&self, record: LifecycleAuditRecord);
}

// --- Errors -------------------------------------------------------------------------

/// Every refusal is a DISTINCT named error ([`LifecycleError::name`]) —
/// never a warning, never a silent acceptance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LifecycleError {
    /// The attempted `from → to` edge is not in the frozen table (nor the
    /// crash set). The machine never invents a path.
    IllegalTransition { from: LifecycleState, to: LifecycleState, reason: String },
    /// A required §5.6 gate did not pass; the transition was refused and the
    /// state left unchanged (the §5.6 exit edges are applied explicitly via
    /// `reject` / `fail` / `crash`).
    GateRefused {
        from: LifecycleState,
        to: LifecycleState,
        gate: &'static str,
        reason: String,
    },
    /// A drain was requested but no [`DrainPort`] is attached. Fail-closed:
    /// a plugin that cannot report its outstanding work is never assumed
    /// quiet.
    DrainPortMissing,
    /// The drain deadline expired with work outstanding. The `Draining →
    /// Stopped` transition was NOT committed; the incumbent was restored to
    /// `Ready` (active) and the refusal audited — the operator decides.
    DrainDeadlineExceeded { outstanding: usize },
    /// A replacement was started while the incumbent was not fully drained
    /// (state is anything other than `Stopped`). The core drain-before-
    /// replace refusal: a NAMED error, never a warning.
    ReplacementBlockedUndrained { plugin_id: String, state: LifecycleState },
}

impl LifecycleError {
    /// The stable machine-readable error name (the named-error table key).
    pub fn name(&self) -> &'static str {
        match self {
            LifecycleError::IllegalTransition { .. } => "ILLEGAL_TRANSITION",
            LifecycleError::GateRefused { .. } => "GATE_REFUSED",
            LifecycleError::DrainPortMissing => "DRAIN_PORT_MISSING",
            LifecycleError::DrainDeadlineExceeded { .. } => "DRAIN_DEADLINE_EXCEEDED",
            LifecycleError::ReplacementBlockedUndrained { .. } => "REPLACEMENT_BLOCKED_UNDRAINED",
        }
    }
}

impl fmt::Display for LifecycleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LifecycleError::IllegalTransition { from, to, reason } => write!(
                f,
                "illegal lifecycle transition {} -> {} ({reason}); the §5.6 transition table is frozen",
                from.name(),
                to.name()
            ),
            LifecycleError::GateRefused { from, to, gate, reason } => write!(
                f,
                "gate {gate} refused the transition {} -> {}: {reason} (fail-closed; the state is unchanged and the refusal audited)",
                from.name(),
                to.name()
            ),
            LifecycleError::DrainPortMissing => {
                write!(f, "no drain port is attached; a plugin that cannot report and abandon its outstanding work is never assumed quiescent")
            }
            LifecycleError::DrainDeadlineExceeded { outstanding } => write!(
                f,
                "drain deadline expired with {outstanding} operation(s) outstanding; the stop is refused (fail-closed), the incumbent stays active, and the operator decides"
            ),
            LifecycleError::ReplacementBlockedUndrained { plugin_id, state } => write!(
                f,
                "replacement of plugin '{plugin_id}' is blocked: the incumbent is {state}, not fully drained (drain-before-replace, report §5.6)"
            ),
        }
    }
}

impl std::error::Error for LifecycleError {}

// --- The machine ----------------------------------------------------------------------

/// One plugin's lifecycle. Owns the current state, the attached drain port,
/// and the audit wiring; every state change (and every refusal) goes through
/// it. `Rejected`/`Stopped`/`Failed` are terminal except for the two
/// recorded operations: `Failed → Draining` (§5.6) and the explicit restart.
pub struct PluginLifecycle<'a> {
    plugin_id: String,
    version: String,
    state: LifecycleState,
    /// The state the plugin was in when it last entered `Failed` — retained
    /// as drill evidence for P7.30 (the failed plugin holds its last state).
    failed_from: Option<LifecycleState>,
    policy: DrainPolicy,
    clock: &'a dyn Clock,
    audit: &'a dyn AuditSink,
    drain: Option<&'a mut dyn DrainPort>,
}

impl<'a> PluginLifecycle<'a> {
    /// Register a plugin: entry into the lifecycle at `Discovered`, audited.
    pub fn register(
        plugin_id: &str,
        version: &str,
        clock: &'a dyn Clock,
        audit: &'a dyn AuditSink,
        policy: DrainPolicy,
    ) -> PluginLifecycle<'a> {
        let lifecycle = PluginLifecycle {
            plugin_id: plugin_id.to_string(),
            version: version.to_string(),
            state: LifecycleState::Discovered,
            failed_from: None,
            policy,
            clock,
            audit,
            drain: None,
        };
        lifecycle.emit(
            LifecycleAuditKind::Registration,
            None,
            Some(LifecycleState::Discovered),
            actor_composition_root(),
            "plugin registered into the lifecycle at Discovered",
        );
        lifecycle
    }

    pub fn plugin_id(&self) -> &str {
        &self.plugin_id
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn state(&self) -> LifecycleState {
        self.state
    }

    /// The state the plugin was in when it entered `Failed` (P7.30 drill
    /// evidence). `None` while the plugin has never failed.
    pub fn failed_from(&self) -> Option<LifecycleState> {
        self.failed_from
    }

    pub fn policy(&self) -> DrainPolicy {
        self.policy
    }

    // -- The gated forward path (§5.6 step 2; each gate is handed in) ----

    /// `Discovered → Verified` (signature / provenance accepted). P7.14
    /// supplies the actual verification; this machine only enforces the
    /// gate outcome.
    pub fn verify(&mut self, provenance: GateOutcome, actor: &str, reason: &str) -> Result<(), LifecycleError> {
        self.gated_transition(LifecycleState::Verified, GATE_PROVENANCE, provenance, actor, reason)
    }

    /// `Verified → Compatible` (manifest and API compatibility pass; the
    /// P7.05 validator and the §9.3 range check are the real gate).
    pub fn mark_compatible(
        &mut self,
        compatibility: GateOutcome,
        actor: &str,
        reason: &str,
    ) -> Result<(), LifecycleError> {
        self.gated_transition(
            LifecycleState::Compatible,
            GATE_MANIFEST_API_COMPATIBILITY,
            compatibility,
            actor,
            reason,
        )
    }

    /// `Compatible → Configured` (schema and secret references valid).
    pub fn configure(
        &mut self,
        configuration: GateOutcome,
        actor: &str,
        reason: &str,
    ) -> Result<(), LifecycleError> {
        self.gated_transition(LifecycleState::Configured, GATE_SCHEMA_SECRETS, configuration, actor, reason)
    }

    /// `Configured → Started` (dependency graph resolved — the P7.06
    /// resolver's plan is the real evidence).
    pub fn start(
        &mut self,
        dependency_graph: GateOutcome,
        actor: &str,
        reason: &str,
    ) -> Result<(), LifecycleError> {
        self.gated_transition(LifecycleState::Started, GATE_DEPENDENCY_GRAPH, dependency_graph, actor, reason)
    }

    /// `Started → Ready` (health checks pass).
    pub fn mark_ready(&mut self, health: GateOutcome, actor: &str, reason: &str) -> Result<(), LifecycleError> {
        self.gated_transition(LifecycleState::Ready, GATE_HEALTH, health, actor, reason)
    }

    // -- Drain-before-replace ---------------------------------------------

    /// `Ready → Draining` (disable / upgrade requested, §5.6) or the §5.6
    /// cleanup edge `Failed → Draining`. The plugin's [`DrainPort`] must be
    /// attached here; a plugin with no port is never drained, never stopped,
    /// and never replaced ([`LifecycleError::DrainPortMissing`] is refused
    /// up front rather than discovered mid-drain).
    pub fn begin_drain(
        &mut self,
        drain: &'a mut dyn DrainPort,
        actor: &str,
        reason: &str,
    ) -> Result<(), LifecycleError> {
        let to = LifecycleState::Draining;
        if !is_legal_transition(self.state, to) {
            return Err(self.illegal(to, "drain may only begin from Ready (disable/upgrade) or Failed (cleanup)"));
        }
        self.drain = Some(drain);
        self.commit(to, actor, reason);
        Ok(())
    }

    /// `Draining → Stopped` (active work completed or cancelled, §5.6).
    /// Runs the attached drain port against the configured deadline:
    ///
    /// - quiesced → the transition is committed and audited;
    /// - deadline exceeded → the transition is REFUSED (fail-closed), the
    ///   incumbent is restored to `Ready` (active) pending an explicit
    ///   operator decision, and the refusal is audited
    ///   ([`LifecycleError::DrainDeadlineExceeded`]). There is no forced
    ///   stop; the operator decides.
    pub fn complete_drain(&mut self, actor: &str, reason: &str) -> Result<DrainOutcome, LifecycleError> {
        if self.state != LifecycleState::Draining {
            return Err(self.illegal(
                LifecycleState::Stopped,
                "complete_drain is only legal from Draining (begin the drain first)",
            ));
        }
        let port = match self.drain.as_deref_mut() {
            Some(port) => port,
            None => return Err(LifecycleError::DrainPortMissing),
        };
        let deadline = self.clock.now_millis().saturating_add(self.policy.deadline_ms);
        let outcome = port.drain(deadline);
        match outcome {
            DrainOutcome::Quiesced { cancelled } => {
                self.commit(LifecycleState::Stopped, actor, reason);
                Ok(DrainOutcome::Quiesced { cancelled })
            }
            DrainOutcome::DeadlineExceeded { outstanding } => {
                // REFUSED: the attempted edge is Draining → Stopped; the
                // incumbent is restored to Ready (active) — the operator
                // decides. Both records are emitted (refusal + restoration).
                self.emit(
                    LifecycleAuditKind::Refused,
                    Some(LifecycleState::Draining),
                    Some(LifecycleState::Stopped),
                    actor,
                    &format!(
                        "drain deadline exceeded with {outstanding} operation(s) outstanding; stop refused (fail-closed)"
                    ),
                );
                self.state = LifecycleState::Ready;
                self.emit(
                    LifecycleAuditKind::Transition,
                    Some(LifecycleState::Draining),
                    Some(LifecycleState::Ready),
                    actor,
                    "incumbent restored to Ready after refused drain; awaiting operator decision",
                );
                Err(LifecycleError::DrainDeadlineExceeded { outstanding })
            }
        }
    }

    /// The replacement gate, used by the P7.08 registry's
    /// `swap_provider`: the incumbent must be fully drained
    /// ([`LifecycleState::Stopped`]) before a replacement may start. Any
    /// other state — including `Ready` (active, no drain begun) and
    /// `Draining` (drain in progress, not quiesced) — is the NAMED error
    /// [`LifecycleError::ReplacementBlockedUndrained`], audited as a
    /// refusal. Never a warning.
    pub fn require_fully_drained(&mut self, actor: &str, reason: &str) -> Result<(), LifecycleError> {
        if self.state == LifecycleState::Stopped {
            return Ok(());
        }
        self.emit(
            LifecycleAuditKind::Refused,
            Some(self.state),
            Some(LifecycleState::Stopped),
            actor,
            &format!(
                "replacement refused: the incumbent is not fully drained (drain-before-replace, report §5.6): {reason}"
            ),
        );
        Err(LifecycleError::ReplacementBlockedUndrained {
            plugin_id: self.plugin_id.clone(),
            state: self.state,
        })
    }

    /// Record the replacement's version change AFTER a completed drain
    /// (P7.07 step 3: "record the version change, then replace"). Legal only
    /// from `Stopped`; audited as a [`LifecycleAuditKind::VersionChange`]
    /// record carrying the OLD version (the new version is in the reason).
    pub fn record_version_change(
        &mut self,
        new_version: &str,
        actor: &str,
        reason: &str,
    ) -> Result<(), LifecycleError> {
        if self.state != LifecycleState::Stopped {
            return Err(self.illegal(
                self.state,
                "a version change is recorded only after the incumbent is fully drained (Stopped)",
            ));
        }
        self.emit(
            LifecycleAuditKind::VersionChange,
            Some(LifecycleState::Stopped),
            None,
            actor,
            &format!(
                "version change {} -> {new_version} recorded after completed drain: {reason}",
                self.version
            ),
        );
        Ok(())
    }

    // -- Rejected / Failed exits (§5.6) ------------------------------------

    /// `Verified → Rejected` (verification failure) or `Compatible →
    /// Rejected` (incompatible API) — the §5.6 exit edges, applied
    /// explicitly after a refused gate.
    pub fn reject(&mut self, actor: &str, reason: &str) -> Result<(), LifecycleError> {
        let to = LifecycleState::Rejected;
        if !is_legal_transition(self.state, to) {
            return Err(self.illegal(to, "Rejected is reachable only from Verified or Compatible (§5.6)"));
        }
        self.commit(to, actor, reason);
        Ok(())
    }

    /// `Started → Failed` (initialization failure) or `Ready → Failed`
    /// (health / runtime failure) — the §5.6 exit edges.
    pub fn fail(&mut self, actor: &str, reason: &str) -> Result<(), LifecycleError> {
        let to = LifecycleState::Failed;
        if !FROZEN_TRANSITIONS.iter().any(|(f, t)| *f == self.state && *t == to) {
            return Err(self.illegal(
                to,
                "Failed via fail() is reachable only from Started or Ready (§5.6); use crash() elsewhere",
            ));
        }
        self.failed_from = Some(self.state);
        self.commit(to, actor, reason);
        Ok(())
    }

    /// Crash path: `Failed` from ANY non-terminal state (DEFAULT decision —
    /// the §5.6 diagram names `Started`/`Ready`; a crash in any other state
    /// must still be representable and audited, never silent). `Started`
    /// and `Ready` reach `Failed` through the frozen table's own edges;
    /// the remaining non-terminal states through [`CRASH_TRANSITIONS`].
    /// The plugin STAYS `Failed`: no auto-restart exists in v0.5.0.
    pub fn crash(&mut self, actor: &str, reason: &str) -> Result<(), LifecycleError> {
        let to = LifecycleState::Failed;
        if !is_legal_transition(self.state, to) {
            return Err(self.illegal(
                to,
                "a crash is only representable from a non-terminal state (terminal states hold their outcome)",
            ));
        }
        self.failed_from = Some(self.state);
        self.commit(to, actor, reason);
        Ok(())
    }

    /// The EXPLICIT restart out of `Failed` — the ONLY restart path in
    /// v0.5.0 (DEFAULT decision: no auto-restart; recorded in the decision
    /// record). Consumes the failed lifecycle (which holds its last state
    /// as drill evidence until this call) and returns a fresh lifecycle at
    /// `Discovered`; the restart itself is audited as
    /// [`LifecycleAuditKind::ExplicitRestart`].
    pub fn restart_explicitly(self, actor: &str, reason: &str) -> Result<PluginLifecycle<'a>, LifecycleError> {
        if self.state != LifecycleState::Failed {
            return Err(self.illegal(
                LifecycleState::Discovered,
                "an explicit restart is only legal from Failed; there is no auto-restart",
            ));
        }
        let PluginLifecycle { plugin_id, version, failed_from, policy, clock, audit, .. } = self;
        let fresh = PluginLifecycle {
            plugin_id,
            version,
            state: LifecycleState::Discovered,
            failed_from,
            policy,
            clock,
            audit,
            drain: None,
        };
        fresh.emit(
            LifecycleAuditKind::ExplicitRestart,
            Some(LifecycleState::Failed),
            Some(LifecycleState::Discovered),
            actor,
            &format!("explicit operator restart (no auto-restart in v0.5.0): {reason}"),
        );
        Ok(fresh)
    }

    // -- Internals -----------------------------------------------------------

    fn gated_transition(
        &mut self,
        to: LifecycleState,
        gate: &'static str,
        outcome: GateOutcome,
        actor: &str,
        reason: &str,
    ) -> Result<(), LifecycleError> {
        if !is_legal_transition(self.state, to) {
            return Err(self.illegal(to, "the §5.6 transition table is frozen"));
        }
        match &outcome {
            GateOutcome::Passed => {
                self.commit(to, actor, reason);
                Ok(())
            }
            GateOutcome::Failed { reason: gate_reason } => {
                let text = format!("gate {gate} failed: {gate_reason}; caller reason: {reason}");
                self.emit(
                    LifecycleAuditKind::Refused,
                    Some(self.state),
                    Some(to),
                    actor,
                    &text,
                );
                Err(LifecycleError::GateRefused { from: self.state, to, gate, reason: text })
            }
        }
    }

    fn commit(&mut self, to: LifecycleState, actor: &str, reason: &str) {
        let from = self.state;
        self.state = to;
        self.emit(LifecycleAuditKind::Transition, Some(from), Some(to), actor, reason);
    }

    /// Emit an audit record. `from_override` is the state the plugin was in
    /// BEFORE the change this record describes (refusals and restorations
    /// pass it explicitly); when `None` the current state is used (the
    /// registration record's entry point).
    fn emit(
        &self,
        kind: LifecycleAuditKind,
        from_override: Option<LifecycleState>,
        to: Option<LifecycleState>,
        actor: &str,
        reason: &str,
    ) {
        let from = from_override.or(Some(self.state));
        let record = LifecycleAuditRecord {
            kind,
            plugin_id: self.plugin_id.clone(),
            version: self.version.clone(),
            from,
            to,
            actor: actor.to_string(),
            reason: reason.to_string(),
            timestamp_millis: self.clock.now_millis(),
        };
        self.audit.record(record);
    }

    fn illegal(&self, to: LifecycleState, reason: &str) -> LifecycleError {
        LifecycleError::IllegalTransition {
            from: self.state,
            to,
            reason: reason.to_string(),
        }
    }
}

/// The actor attributed to lifecycle registration (the composition root
/// registers plugins; §6.4 — composition happens at the composition root).
fn actor_composition_root() -> &'static str {
    "composition-root"
}

/// An append-only in-memory [`AuditSink`] — the deterministic collector used
/// by tests and by future in-process compositions that only need to retain
/// the records for inspection. Ordering is exactly the emission order.
#[derive(Debug, Default)]
pub struct MemoryAuditSink {
    records: std::cell::RefCell<Vec<LifecycleAuditRecord>>,
}

impl MemoryAuditSink {
    pub fn new() -> MemoryAuditSink {
        MemoryAuditSink { records: std::cell::RefCell::new(Vec::new()) }
    }

    /// The records in emission order (append-only; the borrow is brief and
    /// single-threaded — the lifecycle is not `Sync`).
    pub fn records(&self) -> std::cell::Ref<Vec<LifecycleAuditRecord>> {
        self.records.borrow()
    }
}

impl AuditSink for MemoryAuditSink {
    fn record(&self, record: LifecycleAuditRecord) {
        self.records.borrow_mut().push(record);
    }
}
