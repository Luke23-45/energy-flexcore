// P7.18 — Worker supervision: timeouts, retries, crash isolation, and
//         backpressure over the Class C worker contract
//         (`python/workers/contracts/worker_api.proto`,
//         `flexcore.worker.v1`; ledger subtask P7.18).
//
// ## What this module is
//
// The supervision layer the P7.17 worker template presupposes: a slow
// or dead worker degrades the schedule, never the platform (ledger
// P7.18 objective). This module is deliberately PURE LOGIC — the state
// machine, policy, retry schedule, backpressure accounting, and audit
// port — so every rule below is deterministic and unit-testable with an
// injected [`Clock`] (the P1.07 port; [`flexcore_ports::fakes::
// FixedClock`] in tests). The actual gRPC transport (a grpcio-style
// Rust client over `flexcore.worker.v1`) is the deployment-stage seam:
// it implements [`WorkerTransport`] below and drives the pool through
// the recorded outcomes; no transport, no tokio, no new dependency is
// added here (no root `Cargo.toml` change in this loop).
//
// ## Placement decision (recorded)
//
// The supervisor lands in the EXISTING worker-serving app
// `apps/worker` (`flexcore-worker`) as an additive module — not a new
// crate, not the kernel. `apps/worker` is the platform's background-
// process composition root (P1.19); Class C workers are background
// processes of exactly the kind it hosts, and the P7.18 ledger hints
// name it. It depends on nothing beyond what the crate already has
// (std + `flexcore-ports` for the Clock port).
//
// ## Lifecycle vocabulary consistency seam (recorded — kernel not touched)
//
// The worker states here MIRROR the kernel's state NAMES from
// `crates/flexcore-kernel/src/lifecycle.rs::LifecycleState` (READY,
// DRAINING, FAILED; STARTING/STOPPING for the process-local endpoints)
// so the same concept never carries two names across the platform. The
// kernel state machine is NOT implemented or modified here — the
// supervision vocabulary is the worker-process view that maps onto it
// (a worker plugin's `Failed` transition is the kernel's, reported via
// the P7.07 lifecycle registry, not this module's). The seam is stated
// once, in [`LIFECYCLE_CONSISTENCY_SEAM`].
//
// ## Policy summary (every DEFAULT recorded in the decision record)
//
// - Per-call timeout (DEFAULT 30 000 ms, from the call start), enforced
//   as a [`CallDeadline`] computed from the injected clock; a timeout is
//   a RECORDED failure with diagnostics, never a hung schedule.
// - Bounded retries (DEFAULT 2 retries = 3 attempts total) with
//   exponential backoff (base 250 ms, cap 5 000 ms) plus seeded jitter
//   (cap 250 ms) — the schedule is a pure function of (seed, attempt),
//   so a P7.42 replay reproduces it exactly. Only IDEMPOTENT calls are
//   retried; a non-idempotent call is never retried, and a worker-side
//   "retryable" hint never overrides that.
// - Crash isolation: a crashed worker is marked FAILED, removed from
//   the ready set immediately, and the pool reports DEGRADED. Recovery
//   requires a successful probe AND a supervisor policy record
//   (`policy_ref`); there is no silent restart path.
// - Backpressure: a bounded in-flight cap per worker (DEFAULT 4,
//   mirroring the template's `DEFAULT_MAX_INFLIGHT`); beyond it, work is
//   REFUSED with the named `BackpressureRefused` error and audited —
//   never queued unboundedly, never silently dropped.
//
// ## Audit
//
// Worker registrations, accepted calls, timeouts, retries, crashes,
// backpressure refusals, probe recoveries, and reinstatements each
// produce one [`SupervisionAuditEvent`] through the in-module
// [`SupervisionAuditSink`] port (kernel-style: append-only by
// convention, audit-or-refuse — a failing sink refuses the action, so
// nothing happens without its audit record). The port lives HERE, not
// in `flexcore-audit`, because adding that dependency would require a
// `Cargo.toml` change outside this loop's edit scope; the port's shape
// mirrors `flexcore-audit::sink::AuditSink` so promotion is mechanical
// (recorded in the decision record).
//
// ## Scope and control status
//
// Simulation-only supervision logic (AGENTS.md §5): no control
// authority, no field device, no protection-grade function. The
// supervisor refuses work; it never issues anything.

use std::collections::BTreeMap;
use std::fmt;

use flexcore_ports::clock::Clock;

/// The Class C contract version this supervisor speaks (single source
/// mirrored by the template's `worker_template/errors.py` and the proto
/// header; the proto is the wire definition of record).
pub const CONTRACT_VERSION: &str = "flexcore.worker.v1";

/// The recorded lifecycle-vocabulary consistency seam: the state NAMES
/// below mirror `crates/flexcore-kernel/src/lifecycle.rs`
/// `LifecycleState` (READY / DRAINING / FAILED) without implementing or
/// modifying the kernel state machine.
pub const LIFECYCLE_CONSISTENCY_SEAM: &str =
    "crates/flexcore-kernel/src/lifecycle.rs::LifecycleState (names mirrored; kernel NOT touched)";

/// The DEFAULT per-call timeout, in milliseconds. Same value as the
/// kernel's `DEFAULT_DRAIN_DEADLINE_MS` (30 000) and the template's
/// `DEFAULT_DRAIN_GRACE_MS` — a coincidence of defaults, recorded as
/// such, not a shared constant.
pub const DEFAULT_CALL_TIMEOUT_MS: u64 = 30_000;

/// The DEFAULT bounded in-flight cap per worker (mirrors the template's
/// `DEFAULT_MAX_INFLIGHT`).
pub const DEFAULT_INFLIGHT_CAP: u32 = 4;

/// The DEFAULT supervision seed (deterministic jitter for P7.42).
/// Hex for "FLXP718" (FlexCore P7.18) so the default is stable across
/// builds and replays.
pub const DEFAULT_SUPERVISION_SEED: u64 = 0x464C_5843_5037_3138;

// ---------------------------------------------------------------------------
// Policy
// ---------------------------------------------------------------------------

/// The supervision policy: every bound is explicit and validated at
/// construction (fail-closed — a zero timeout or an unbounded retry
/// count is refused, not defaulted).
///
/// DEFAULT-labelled values are decisions recorded in
/// `ledger/phase-07-plugin-sdk/17_18_workers_supervision.md` §5; the
/// ledger's per-contract override source (manifest
/// `resources.timeoutMs`) is applied by the composition layer that
/// builds this policy, never by the worker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SupervisionPolicy {
    /// Per-call timeout in milliseconds (DEFAULT
    /// [`DEFAULT_CALL_TIMEOUT_MS`]).
    pub call_timeout_ms: u64,
    /// Bounded retries per idempotent call (DEFAULT 2; 3 attempts
    /// total). Hard-capped at 8 by [`SupervisionPolicy::validate`] —
    /// unbounded retry amplification is the recorded P7.18 risk.
    pub max_retries: u32,
    /// Exponential-backoff base in milliseconds (DEFAULT 250).
    pub backoff_base_ms: u64,
    /// Backoff cap in milliseconds (DEFAULT 5 000) — the exponential
    /// growth stops here, only jitter is added afterwards.
    pub backoff_cap_ms: u64,
    /// Deterministic jitter cap in milliseconds (DEFAULT 250); the
    /// jitter is a pure function of (seed, attempt), never the wall
    /// clock (P7.42).
    pub jitter_cap_ms: u64,
    /// Bounded in-flight requests per worker (DEFAULT
    /// [`DEFAULT_INFLIGHT_CAP`]).
    pub inflight_cap: u32,
    /// Consecutive transport failures before a worker is marked FAILED
    /// (DEFAULT 3). A crash marks FAILED immediately, bypassing the
    /// threshold.
    pub failure_threshold: u32,
    /// Readiness-probe interval in milliseconds (DEFAULT 5 000); the
    /// probe cadence maps to the template's `Check` rpc.
    pub probe_interval_ms: u64,
    /// The deterministic jitter seed (DEFAULT
    /// [`DEFAULT_SUPERVISION_SEED`]).
    pub seed: u64,
}

impl Default for SupervisionPolicy {
    fn default() -> Self {
        Self {
            call_timeout_ms: DEFAULT_CALL_TIMEOUT_MS,
            max_retries: 2,      // DEFAULT: 3 attempts total, bounded.
            backoff_base_ms: 250, // DEFAULT.
            backoff_cap_ms: 5_000, // DEFAULT.
            jitter_cap_ms: 250,  // DEFAULT.
            inflight_cap: DEFAULT_INFLIGHT_CAP,
            failure_threshold: 3, // DEFAULT: consecutive-failure threshold.
            probe_interval_ms: 5_000, // DEFAULT.
            seed: DEFAULT_SUPERVISION_SEED,
        }
    }
}

impl SupervisionPolicy {
    /// Fail-closed validation. Every bound must be a real bound: a zero
    /// timeout, a zero in-flight cap, a zero backoff base, a retry count
    /// above the amplification bound, or a cap below the base are all
    /// refused with [`SupervisionError::InvalidPolicy`] — never
    /// silently corrected.
    pub fn validate(&self) -> Result<(), SupervisionError> {
        if self.call_timeout_ms == 0 {
            return Err(SupervisionError::InvalidPolicy {
                detail: "call_timeout_ms must be > 0 (a zero timeout would fail every call)"
                    .to_string(),
            });
        }
        if self.max_retries > 8 {
            return Err(SupervisionError::InvalidPolicy {
                detail: format!(
                    "max_retries {} exceeds the amplification bound (8)",
                    self.max_retries
                ),
            });
        }
        if self.backoff_base_ms == 0 {
            return Err(SupervisionError::InvalidPolicy {
                detail: "backoff_base_ms must be > 0".to_string(),
            });
        }
        if self.backoff_cap_ms < self.backoff_base_ms {
            return Err(SupervisionError::InvalidPolicy {
                detail: "backoff_cap_ms must be >= backoff_base_ms".to_string(),
            });
        }
        if self.inflight_cap == 0 {
            return Err(SupervisionError::InvalidPolicy {
                detail: "inflight_cap must be > 0 (zero would refuse all work; \
                         an unbounded cap is not expressible)"
                    .to_string(),
            });
        }
        if self.failure_threshold == 0 {
            return Err(SupervisionError::InvalidPolicy {
                detail: "failure_threshold must be > 0".to_string(),
            });
        }
        if self.probe_interval_ms == 0 {
            return Err(SupervisionError::InvalidPolicy {
                detail: "probe_interval_ms must be > 0".to_string(),
            });
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Retry schedule (deterministic — P7.42)
// ---------------------------------------------------------------------------

/// The deterministic retry scheduler: exponential backoff with a seeded
/// jitter, a pure function of `(seed, attempt)`.
///
/// `delay(attempt) = min(base * 2^attempt, cap) + jitter(attempt)` where
/// `jitter` derives from a xorshift64 generator seeded once. No wall
/// clock anywhere: a P7.42 replay with the same seed reproduces the
/// same schedule exactly, which is the point.
#[derive(Debug, Clone)]
pub struct RetryScheduler {
    seed: u64,
    backoff_base_ms: u64,
    backoff_cap_ms: u64,
    jitter_cap_ms: u64,
}

impl RetryScheduler {
    /// Builds the scheduler from a validated policy.
    pub fn new(policy: &SupervisionPolicy) -> Self {
        Self {
            seed: policy.seed,
            backoff_base_ms: policy.backoff_base_ms,
            backoff_cap_ms: policy.backoff_cap_ms,
            jitter_cap_ms: policy.jitter_cap_ms,
        }
    }

    /// The delay before retry number `attempt` (0-based: the delay
    /// after the first failure is `attempt = 0` → base). Overflow-safe:
    /// the exponential factor saturates at `u64::MAX` and the cap
    /// applies before jitter.
    pub fn delay_ms(&self, attempt: u32) -> u64 {
        let factor = 1u64
            .checked_shl(attempt.min(31))
            .unwrap_or(u64::MAX);
        let exponential = self.backoff_base_ms.saturating_mul(factor);
        exponential
            .min(self.backoff_cap_ms)
            .saturating_add(self.jitter(attempt))
    }

    /// Deterministic jitter in `[0, jitter_cap)`: xorshift64 over
    /// `(seed, attempt)`. A zero cap yields zero jitter (still
    /// deterministic).
    fn jitter(&self, attempt: u32) -> u64 {
        if self.jitter_cap_ms == 0 {
            return 0;
        }
        let mut x = self
            .seed
            .wrapping_add((u64::from(attempt) + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15));
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        x % self.jitter_cap_ms
    }
}

// ---------------------------------------------------------------------------
// Lifecycle vocabulary (mirrors the kernel names — see the seam note)
// ---------------------------------------------------------------------------

/// The supervised worker's lifecycle states. Names mirror
/// `crates/flexcore-kernel/src/lifecycle.rs::LifecycleState` (READY,
/// DRAINING, FAILED — [`LIFECYCLE_CONSISTENCY_SEAM`]); STARTING and
/// STOPPING are the process-local endpoints the kernel's vocabulary
/// covers as transitions, not plugin states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupervisedWorkerState {
    /// Registered, not yet probed ready.
    Starting,
    /// Accepting work (the only acquire-eligible state).
    Ready,
    /// Draining: finishing in-flight work, refusing new work.
    Draining,
    /// Stopped by policy (drain completed); removable.
    Stopping,
    /// Crashed or failed past the threshold; out of the ready set until
    /// an authorized recovery.
    Failed,
}

impl SupervisedWorkerState {
    /// The kernel-style NAME (the string the audit trail and any
    /// lifecycle mapping use — same vocabulary as the kernel's
    /// `LifecycleState::as_str`).
    pub fn as_str(self) -> &'static str {
        match self {
            SupervisedWorkerState::Starting => "STARTING",
            SupervisedWorkerState::Ready => "READY",
            SupervisedWorkerState::Draining => "DRAINING",
            SupervisedWorkerState::Stopping => "STOPPING",
            SupervisedWorkerState::Failed => "FAILED",
        }
    }
}

impl fmt::Display for SupervisedWorkerState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// Calls, failures, outcomes
// ---------------------------------------------------------------------------

/// The idempotency class of a call (P7.18 step 2: idempotency keys on
/// every request; no retry of a non-idempotent call). Mirrors the
/// proto's `DeterminismMetadata.idempotency_key`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallKind {
    /// An idempotent call carrying a non-empty idempotency key: safe to
    /// retry under the bounded policy (no duplicated side effect — the
    /// key is the worker's deduplication contract).
    Idempotent { idempotency_key: String },
    /// A non-idempotent call: NEVER retried, regardless of any
    /// worker-side retryable hint (the recorded P7.18 risk: a retry
    /// re-issuing a side effect).
    NonIdempotent,
}

impl CallKind {
    /// Whether the retry policy may ever retry this call. An
    /// `Idempotent` call with an empty key is refused at `acquire`
    /// (fail-closed), so retryability here is exact.
    pub fn is_retryable(&self) -> bool {
        matches!(self, CallKind::Idempotent { .. })
    }
}

/// The kind of transport failure the supervisor recorded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerFailureKind {
    /// The call deadline elapsed (the injected-clock backstop fired).
    Timeout,
    /// The worker process died mid-request (gRPC unavailable /
    /// connection reset at the transport seam).
    Crash,
    /// The worker answered but violated the contract (missing
    /// correlation echo, no provenance, version skew).
    ContractViolation,
}

impl WorkerFailureKind {
    /// The audit-trail name.
    pub fn as_str(self) -> &'static str {
        match self {
            WorkerFailureKind::Timeout => "TIMEOUT",
            WorkerFailureKind::Crash => "CRASH",
            WorkerFailureKind::ContractViolation => "CONTRACT_VIOLATION",
        }
    }
}

/// The deadline of one acquired call: the pure timeout backstop. The
/// real transport checks `expired_at(now)` against the injected clock;
/// the supervisor never reads a wall clock of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CallDeadline {
    started_at_ms: i64,
    timeout_ms: u64,
}

impl CallDeadline {
    /// Starts the deadline window at the clock's current instant.
    pub fn start(clock: &dyn Clock, timeout_ms: u64) -> Result<Self, SupervisionError> {
        Ok(Self {
            started_at_ms: now_millis(clock)?,
            timeout_ms,
        })
    }

    /// Whether the deadline has elapsed at `now_ms`.
    pub fn expired_at(&self, now_ms: i64) -> bool {
        now_ms >= self.started_at_ms.saturating_add_unsigned(self.timeout_ms)
    }

    /// The absolute deadline in milliseconds since the Unix epoch.
    pub fn deadline_ms(&self) -> i64 {
        self.started_at_ms.saturating_add_unsigned(self.timeout_ms)
    }

    /// The start instant in milliseconds since the Unix epoch.
    pub fn started_at_ms(&self) -> i64 {
        self.started_at_ms
    }

    /// The configured timeout in milliseconds.
    pub fn timeout_ms(&self) -> u64 {
        self.timeout_ms
    }
}

fn now_millis(clock: &dyn Clock) -> Result<i64, SupervisionError> {
    // The contracts timestamps are i128 millis; the supervision
    // arithmetic is i64. An out-of-range instant is a fail-closed error,
    // never a silent truncation.
    let raw = clock.now_ingest_time().unix_millis();
    i64::try_from(raw).map_err(|_| SupervisionError::ClockOutOfRange { millis: raw })
}

// ---------------------------------------------------------------------------
// Audit port (in-module, kernel-style; see the module header)
// ---------------------------------------------------------------------------

/// The kind of a supervision audit event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupervisionAuditKind {
    /// A worker entered the pool.
    WorkerRegistered,
    /// A worker was probed ready.
    WorkerReady,
    /// A call was accepted under the in-flight cap.
    CallAccepted,
    /// A call completed successfully.
    CallCompleted,
    /// A recorded failure with diagnostics (timeout, crash, or
    /// contract violation — the ledger's "a timeout produces a recorded
    /// failure with diagnostics").
    FailureRecorded,
    /// A bounded retry was scheduled (with the deterministic delay).
    RetryScheduled,
    /// A retry was refused (non-idempotent, or retries exhausted).
    RetryRefused,
    /// A worker was marked FAILED (crash or failure threshold).
    WorkerFailed,
    /// Work was refused under backpressure (with the depth in the
    /// detail) — never silently dropped.
    BackpressureRefused,
    /// A probe succeeded on a failed worker.
    ProbeRecovered,
    /// A failed worker was reinstated under a policy record.
    WorkerReinstated,
    /// A worker entered the drain path.
    WorkerDraining,
}

impl SupervisionAuditKind {
    /// The audit-trail name (kernel `LifecycleAuditKind` style).
    pub fn as_str(self) -> &'static str {
        match self {
            SupervisionAuditKind::WorkerRegistered => "WORKER_REGISTERED",
            SupervisionAuditKind::WorkerReady => "WORKER_READY",
            SupervisionAuditKind::CallAccepted => "CALL_ACCEPTED",
            SupervisionAuditKind::CallCompleted => "CALL_COMPLETED",
            SupervisionAuditKind::FailureRecorded => "FAILURE_RECORDED",
            SupervisionAuditKind::RetryScheduled => "RETRY_SCHEDULED",
            SupervisionAuditKind::RetryRefused => "RETRY_REFUSED",
            SupervisionAuditKind::WorkerFailed => "WORKER_FAILED",
            SupervisionAuditKind::BackpressureRefused => "BACKPRESSURE_REFUSED",
            SupervisionAuditKind::ProbeRecovered => "PROBE_RECOVERED",
            SupervisionAuditKind::WorkerReinstated => "WORKER_REINSTATED",
            SupervisionAuditKind::WorkerDraining => "WORKER_DRAINING",
        }
    }
}

/// One supervision audit event: who, what, when (injected clock), and a
/// short diagnostic detail. No payload bodies, no secrets — identifiers
/// and reasons only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SupervisionAuditEvent {
    pub kind: SupervisionAuditKind,
    pub worker_id: String,
    pub request_id: String,
    pub attempt: u32,
    pub observed_at_ms: i64,
    pub detail: String,
}

impl fmt::Display for SupervisionAuditEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} worker={} request={} attempt={} at={} {}",
            self.kind.as_str(),
            self.worker_id,
            self.request_id,
            self.attempt,
            self.observed_at_ms,
            self.detail
        )
    }
}

/// The in-module audit port (kernel-style append-only by convention).
/// Mirrors `flexcore-audit::sink::AuditSink` in shape so promotion into
/// the platform audit crate is mechanical (recorded seam — see the
/// module header for why the port is in-module in this loop).
pub trait SupervisionAuditSink {
    /// Appends one record. `Err` means the record was NOT durably
    /// accepted, and audit-or-refuse applies: the caller refuses the
    /// supervised action.
    fn record(&self, event: SupervisionAuditEvent) -> Result<(), String>;
}

/// The in-memory sink: the composition/test implementation. Holds every
/// accepted record in arrival order (append-only; no update or delete
/// path exists).
#[derive(Debug, Default)]
pub struct MemorySupervisionAuditSink {
    records: std::sync::Mutex<Vec<SupervisionAuditEvent>>,
}

impl MemorySupervisionAuditSink {
    pub fn new() -> Self {
        Self::default()
    }

    /// The accepted records, in arrival order.
    pub fn records(&self) -> Vec<SupervisionAuditEvent> {
        self.records
            .lock()
            .expect("poisoned supervision audit sink mutex")
            .clone()
    }
}

impl SupervisionAuditSink for MemorySupervisionAuditSink {
    fn record(&self, event: SupervisionAuditEvent) -> Result<(), String> {
        self.records
            .lock()
            .expect("poisoned supervision audit sink mutex")
            .push(event);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// The named supervision errors. Every refusal names itself — the
/// ledger's failure modes (saturation silently dropping work, unbounded
/// retries) are each an explicit variant, never a bare string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SupervisionError {
    /// The worker's in-flight cap is reached: work is REFUSED (never
    /// queued unboundedly — the recorded backpressure rule).
    BackpressureRefused {
        worker_id: String,
        inflight: u32,
        cap: u32,
    },
    /// The worker is not in the acquire-eligible state.
    WorkerNotReady {
        worker_id: String,
        state: SupervisedWorkerState,
    },
    /// The worker id is unknown to the pool.
    UnknownWorker { worker_id: String },
    /// An idempotent call was offered without an idempotency key — the
    /// retry contract needs the key, fail-closed.
    MissingIdempotencyKey { worker_id: String },
    /// Recovery was attempted without the authorizing policy record.
    RecoveryNotAuthorized { worker_id: String },
    /// The audit sink refused the record — audit-or-refuse.
    AuditUnavailable { detail: String },
    /// The policy failed fail-closed validation.
    InvalidPolicy { detail: String },
    /// The clock produced an instant outside the i64 supervision
    /// arithmetic — fail-closed, never truncated.
    ClockOutOfRange { millis: i128 },
}

impl SupervisionError {
    /// The stable error name (the audit/log vocabulary).
    pub fn as_str(&self) -> &'static str {
        match self {
            SupervisionError::BackpressureRefused { .. } => "BACKPRESSURE_REFUSED",
            SupervisionError::WorkerNotReady { .. } => "WORKER_NOT_READY",
            SupervisionError::UnknownWorker { .. } => "UNKNOWN_WORKER",
            SupervisionError::MissingIdempotencyKey { .. } => "MISSING_IDEMPOTENCY_KEY",
            SupervisionError::RecoveryNotAuthorized { .. } => "RECOVERY_NOT_AUTHORIZED",
            SupervisionError::AuditUnavailable { .. } => "AUDIT_UNAVAILABLE",
            SupervisionError::InvalidPolicy { .. } => "INVALID_POLICY",
            SupervisionError::ClockOutOfRange { .. } => "CLOCK_OUT_OF_RANGE",
        }
    }
}

impl fmt::Display for SupervisionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SupervisionError::BackpressureRefused {
                worker_id,
                inflight,
                cap,
            } => write!(
                f,
                "backpressure refused: worker {worker_id} at in-flight \
                 {inflight}/{cap} — work refused, not queued"
            ),
            SupervisionError::WorkerNotReady { worker_id, state } => write!(
                f,
                "worker {worker_id} is not ready (state {state})"
            ),
            SupervisionError::UnknownWorker { worker_id } => {
                write!(f, "unknown worker {worker_id}")
            }
            SupervisionError::MissingIdempotencyKey { worker_id } => write!(
                f,
                "idempotent call to worker {worker_id} carries no \
                 idempotency key (fail-closed)"
            ),
            SupervisionError::RecoveryNotAuthorized { worker_id } => write!(
                f,
                "recovery of worker {worker_id} refused: no supervisor \
                 policy record authorizes the restart"
            ),
            SupervisionError::AuditUnavailable { detail } => {
                write!(f, "supervision audit unavailable: {detail}")
            }
            SupervisionError::InvalidPolicy { detail } => {
                write!(f, "invalid supervision policy: {detail}")
            }
            SupervisionError::ClockOutOfRange { millis } => {
                write!(f, "clock instant {millis} ms out of supervision range")
            }
        }
    }
}

impl std::error::Error for SupervisionError {}

// ---------------------------------------------------------------------------
// The retry decision
// ---------------------------------------------------------------------------

/// What the supervisor does after a recorded failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RetryDecision {
    /// Retry after the deterministic delay, as attempt `next_attempt`.
    RetryAfter { delay_ms: u64, next_attempt: u32 },
    /// No retry: the reason names itself.
    NoRetry { reason: &'static str },
}

// ---------------------------------------------------------------------------
// The pool
// ---------------------------------------------------------------------------

/// One supervised worker's accounting entry.
#[derive(Debug, Clone)]
struct SupervisedWorker {
    worker_id: String,
    state: SupervisedWorkerState,
    inflight: u32,
    consecutive_failures: u32,
}

/// The worker pool + supervisor: registers workers, admits work under
/// the backpressure cap, records failures with the deterministic retry
/// schedule, isolates crashes, and recovers only through the authorized
/// probe path.
///
/// Every mutating method takes the audit sink and the clock explicitly:
/// the pool holds no ambient authority, and audit-or-refuse applies to
/// each step (a failing sink refuses the action — nothing happens
/// without its record).
#[derive(Debug, Clone)]
pub struct WorkerPool {
    policy: SupervisionPolicy,
    retry: RetryScheduler,
    workers: BTreeMap<String, SupervisedWorker>,
    timeouts_recorded: u64,
    backpressure_refusals: u64,
    crashes_recorded: u64,
}

impl WorkerPool {
    /// Creates a pool over a validated policy (fail-closed on an
    /// invalid policy).
    pub fn new(policy: SupervisionPolicy) -> Result<Self, SupervisionError> {
        policy.validate()?;
        let retry = RetryScheduler::new(&policy);
        Ok(Self {
            policy,
            retry,
            workers: BTreeMap::new(),
            timeouts_recorded: 0,
            backpressure_refusals: 0,
            crashes_recorded: 0,
        })
    }

    /// The validated policy in force.
    pub fn policy(&self) -> &SupervisionPolicy {
        &self.policy
    }

    /// Counters (the saturation metrics the P7.18 verification inspects).
    pub fn timeouts_recorded(&self) -> u64 {
        self.timeouts_recorded
    }

    pub fn backpressure_refusals(&self) -> u64 {
        self.backpressure_refusals
    }

    pub fn crashes_recorded(&self) -> u64 {
        self.crashes_recorded
    }

    fn audit(
        &self,
        sink: &dyn SupervisionAuditSink,
        kind: SupervisionAuditKind,
        worker_id: &str,
        request_id: &str,
        attempt: u32,
        detail: String,
        clock: &dyn Clock,
    ) -> Result<(), SupervisionError> {
        let event = SupervisionAuditEvent {
            kind,
            worker_id: worker_id.to_string(),
            request_id: request_id.to_string(),
            attempt,
            observed_at_ms: now_millis(clock)?,
            detail,
        };
        sink.record(event).map_err(|e| SupervisionError::AuditUnavailable {
            detail: e,
        })
    }

    /// Registers a worker into the pool in the STARTING state (the
    /// template's initial state). Re-registering a known id is refused —
    /// identities are stable, replacements are new ids.
    pub fn register(
        &mut self,
        worker_id: &str,
        audit: &dyn SupervisionAuditSink,
        clock: &dyn Clock,
    ) -> Result<(), SupervisionError> {
        if self.workers.contains_key(worker_id) {
            return Err(SupervisionError::UnknownWorker {
                worker_id: format!("{worker_id} (already registered)"),
            });
        }
        self.audit(
            audit,
            SupervisionAuditKind::WorkerRegistered,
            worker_id,
            "",
            0,
            "registered in STARTING".to_string(),
            clock,
        )?;
        self.workers.insert(
            worker_id.to_string(),
            SupervisedWorker {
                worker_id: worker_id.to_string(),
                state: SupervisedWorkerState::Starting,
                inflight: 0,
                consecutive_failures: 0,
            },
        );
        Ok(())
    }

    /// Marks a STARTING worker READY after a successful probe (the
    /// template's STARTING → READY transition).
    pub fn mark_ready(
        &mut self,
        worker_id: &str,
        audit: &dyn SupervisionAuditSink,
        clock: &dyn Clock,
    ) -> Result<(), SupervisionError> {
        {
            let worker = self.worker_mut(worker_id)?;
            if worker.state != SupervisedWorkerState::Starting {
                return Err(SupervisionError::WorkerNotReady {
                    worker_id: worker_id.to_string(),
                    state: worker.state,
                });
            }
        }
        self.audit(
            audit,
            SupervisionAuditKind::WorkerReady,
            worker_id,
            "",
            0,
            "probe succeeded; READY".to_string(),
            clock,
        )?;
        self.worker_mut(worker_id)?.state = SupervisedWorkerState::Ready;
        Ok(())
    }

    /// Admits one call to a READY worker under the in-flight cap, and
    /// returns the call's deadline (the injected-clock backstop).
    ///
    /// Refusals, each audited and named: unknown worker, not READY,
    /// missing idempotency key on a retryable call, and — the
    /// backpressure rule — the in-flight cap reached (work is refused,
    /// never queued unboundedly).
    pub fn acquire(
        &mut self,
        worker_id: &str,
        request_id: &str,
        call: &CallKind,
        audit: &dyn SupervisionAuditSink,
        clock: &dyn Clock,
    ) -> Result<CallDeadline, SupervisionError> {
        if !self.workers.contains_key(worker_id) {
            return Err(SupervisionError::UnknownWorker {
                worker_id: worker_id.to_string(),
            });
        }
        // Fail-closed on the retry contract: an Idempotent call without
        // a key has no deduplication guarantee, so it is refused rather
        // than silently downgraded to non-retryable.
        if let CallKind::Idempotent { idempotency_key } = call {
            if idempotency_key.is_empty() {
                return Err(SupervisionError::MissingIdempotencyKey {
                    worker_id: worker_id.to_string(),
                });
            }
        }
        let state = self.workers[worker_id].state;
        let inflight = self.workers[worker_id].inflight;
        if state != SupervisedWorkerState::Ready {
            return Err(SupervisionError::WorkerNotReady {
                worker_id: worker_id.to_string(),
                state,
            });
        }
        if inflight >= self.policy.inflight_cap {
            self.backpressure_refusals += 1;
            self.audit(
                audit,
                SupervisionAuditKind::BackpressureRefused,
                worker_id,
                request_id,
                0,
                format!(
                    "in-flight {inflight}/{} — refused, not queued",
                    self.policy.inflight_cap
                ),
                clock,
            )?;
            return Err(SupervisionError::BackpressureRefused {
                worker_id: worker_id.to_string(),
                inflight,
                cap: self.policy.inflight_cap,
            });
        }
        let deadline = CallDeadline::start(clock, self.policy.call_timeout_ms)?;
        let inflight_after = {
            let worker = self.workers.get_mut(worker_id).expect("checked above");
            worker.inflight += 1;
            worker.inflight
        };
        self.audit(
            audit,
            SupervisionAuditKind::CallAccepted,
            worker_id,
            request_id,
            0,
            format!(
                "accepted; in-flight {inflight_after}/{}; deadline {} ms",
                self.policy.inflight_cap,
                deadline.deadline_ms()
            ),
            clock,
        )?;
        Ok(deadline)
    }

    /// Records a successful completion: releases the in-flight slot and
    /// resets the consecutive-failure counter.
    pub fn record_completed(
        &mut self,
        worker_id: &str,
        request_id: &str,
        audit: &dyn SupervisionAuditSink,
        clock: &dyn Clock,
    ) -> Result<(), SupervisionError> {
        {
            let worker = self.worker_mut(worker_id)?;
            worker.inflight = worker.inflight.saturating_sub(1);
            worker.consecutive_failures = 0;
        }
        self.audit(
            audit,
            SupervisionAuditKind::CallCompleted,
            worker_id,
            request_id,
            0,
            "completed".to_string(),
            clock,
        )
    }

    /// Records a timeout (the ledger's named failure): the call is
    /// over, the failure is recorded with diagnostics, and the retry
    /// decision follows the bounded idempotency policy — never a hung
    /// schedule.
    pub fn record_timeout(
        &mut self,
        worker_id: &str,
        request_id: &str,
        call: &CallKind,
        attempt: u32,
        deadline: &CallDeadline,
        audit: &dyn SupervisionAuditSink,
        clock: &dyn Clock,
    ) -> Result<RetryDecision, SupervisionError> {
        self.timeouts_recorded += 1;
        self.record_failure(
            worker_id,
            request_id,
            WorkerFailureKind::Timeout,
            call,
            attempt,
            format!(
                "deadline {} ms elapsed (started {})",
                deadline.deadline_ms(),
                deadline.started_at_ms()
            ),
            audit,
            clock,
        )
    }

    /// Records a crash: the worker is marked FAILED IMMEDIATELY (no
    /// threshold — a dead process gets no benefit of the doubt), removed
    /// from the ready set by its state, and the pool degrades. The
    /// in-flight slots of the dead worker are abandoned with the call.
    pub fn record_crash(
        &mut self,
        worker_id: &str,
        reason: &str,
        audit: &dyn SupervisionAuditSink,
        clock: &dyn Clock,
    ) -> Result<(), SupervisionError> {
        self.crashes_recorded += 1;
        {
            let worker = self.worker_mut(worker_id)?;
            worker.state = SupervisedWorkerState::Failed;
            worker.consecutive_failures = self.policy.failure_threshold;
        }
        self.audit(
            audit,
            SupervisionAuditKind::WorkerFailed,
            worker_id,
            "",
            0,
            format!("CRASH: {reason}; pool degraded until authorized recovery"),
            clock,
        )
    }

    /// The shared failure path: releases the in-flight slot, counts the
    /// consecutive failure, marks the worker FAILED at the threshold
    /// (or on a crash), and computes the bounded retry decision.
    #[allow(clippy::too_many_arguments)]
    pub fn record_failure(
        &mut self,
        worker_id: &str,
        request_id: &str,
        kind: WorkerFailureKind,
        call: &CallKind,
        attempt: u32,
        detail: String,
        audit: &dyn SupervisionAuditSink,
        clock: &dyn Clock,
    ) -> Result<RetryDecision, SupervisionError> {
        {
            let worker = self.worker_mut(worker_id)?;
            worker.inflight = worker.inflight.saturating_sub(1);
            worker.consecutive_failures = worker.consecutive_failures.saturating_add(1);
        }
        self.audit(
            audit,
            SupervisionAuditKind::FailureRecorded,
            worker_id,
            request_id,
            attempt,
            format!(
                "failure kind {} attempt {attempt}: {detail}",
                kind.as_str()
            ),
            clock,
        )?;

        let threshold_reached = self.workers[worker_id].consecutive_failures
            >= self.policy.failure_threshold;
        if threshold_reached || kind == WorkerFailureKind::Crash {
            let consecutive = {
                let worker = self.workers.get_mut(worker_id).expect("checked above");
                worker.state = SupervisedWorkerState::Failed;
                worker.consecutive_failures
            };
            self.audit(
                audit,
                SupervisionAuditKind::WorkerFailed,
                worker_id,
                request_id,
                attempt,
                format!(
                    "marked FAILED (kind {}, consecutive failures {consecutive})",
                    kind.as_str(),
                ),
                clock,
            )?;
            return Ok(RetryDecision::NoRetry {
                reason: "worker failed; reschedule on a healthy worker",
            });
        }

        if !call.is_retryable() {
            self.audit(
                audit,
                SupervisionAuditKind::RetryRefused,
                worker_id,
                request_id,
                attempt,
                "non-idempotent call: never retried".to_string(),
                clock,
            )?;
            return Ok(RetryDecision::NoRetry {
                reason: "non-idempotent call",
            });
        }
        if attempt > self.policy.max_retries {
            self.audit(
                audit,
                SupervisionAuditKind::RetryRefused,
                worker_id,
                request_id,
                attempt,
                format!(
                    "retries exhausted ({} > {})",
                    attempt, self.policy.max_retries
                ),
                clock,
            )?;
            return Ok(RetryDecision::NoRetry {
                reason: "retries exhausted",
            });
        }
        let delay_ms = self.retry.delay_ms(attempt);
        let next_attempt = attempt + 1;
        self.audit(
            audit,
            SupervisionAuditKind::RetryScheduled,
            worker_id,
            request_id,
            next_attempt,
            format!("retry in {delay_ms} ms (deterministic schedule)"),
            clock,
        )?;
        Ok(RetryDecision::RetryAfter {
            delay_ms,
            next_attempt,
        })
    }

    /// Begins the drain path (READY → DRAINING; the template's
    /// vocabulary): the worker refuses new work, finishes in-flight
    /// work. A FAILED worker cannot drain — it is already out.
    pub fn drain(
        &mut self,
        worker_id: &str,
        audit: &dyn SupervisionAuditSink,
        clock: &dyn Clock,
    ) -> Result<(), SupervisionError> {
        {
            let worker = self.worker_mut(worker_id)?;
            if worker.state != SupervisedWorkerState::Ready {
                return Err(SupervisionError::WorkerNotReady {
                    worker_id: worker_id.to_string(),
                    state: worker.state,
                });
            }
            worker.state = SupervisedWorkerState::Draining;
        }
        self.audit(
            audit,
            SupervisionAuditKind::WorkerDraining,
            worker_id,
            "",
            0,
            "READY → DRAINING; no new work accepted".to_string(),
            clock,
        )
    }

    /// Completes the drain (DRAINING → STOPPING) once in-flight work
    /// has finished.
    pub fn finish_drain(
        &mut self,
        worker_id: &str,
        audit: &dyn SupervisionAuditSink,
        clock: &dyn Clock,
    ) -> Result<(), SupervisionError> {
        {
            let worker = self.worker_mut(worker_id)?;
            if worker.state != SupervisedWorkerState::Draining {
                return Err(SupervisionError::WorkerNotReady {
                    worker_id: worker_id.to_string(),
                    state: worker.state,
                });
            }
            worker.state = SupervisedWorkerState::Stopping;
        }
        self.audit(
            audit,
            SupervisionAuditKind::WorkerDraining,
            worker_id,
            "",
            0,
            "DRAINING → STOPPING".to_string(),
            clock,
        )
    }

    /// The probe result for one worker (maps to the template's `Check`
    /// rpc and health state machine): readiness means the READY state.
    /// A probe alone NEVER changes any state — recovery is the
    /// authorized path below.
    pub fn probe(&self, worker_id: &str) -> Result<bool, SupervisionError> {
        let worker = self.worker(worker_id)?;
        Ok(worker.state == SupervisedWorkerState::Ready)
    }

    /// The authorized recovery path: a probe succeeded on a FAILED
    /// worker AND a supervisor policy record (`policy_ref`, non-empty —
    /// the record that authorizes the restart) accompanies it. The
    /// worker re-enters STARTING (re-initialization) and reaches READY
    /// only through [`WorkerPool::mark_ready`]. Without the policy
    /// record the recovery is refused — a failed worker is never
    /// silently restarted (the recorded P7.18 rule).
    pub fn recover_on_probe_success(
        &mut self,
        worker_id: &str,
        policy_ref: &str,
        audit: &dyn SupervisionAuditSink,
        clock: &dyn Clock,
    ) -> Result<(), SupervisionError> {
        self.worker(worker_id)?;
        if policy_ref.is_empty() {
            return Err(SupervisionError::RecoveryNotAuthorized {
                worker_id: worker_id.to_string(),
            });
        }
        let state = self.workers[worker_id].state;
        if state != SupervisedWorkerState::Failed {
            return Err(SupervisionError::WorkerNotReady {
                worker_id: worker_id.to_string(),
                state,
            });
        }
        self.audit(
            audit,
            SupervisionAuditKind::ProbeRecovered,
            worker_id,
            "",
            0,
            format!("probe succeeded; restart authorized by {policy_ref}"),
            clock,
        )?;
        let worker = self.workers.get_mut(worker_id).expect("checked above");
        worker.state = SupervisedWorkerState::Starting;
        worker.inflight = 0;
        worker.consecutive_failures = 0;
        self.audit(
            audit,
            SupervisionAuditKind::WorkerReinstated,
            worker_id,
            "",
            0,
            format!("FAILED → STARTING under policy record {policy_ref}"),
            clock,
        )
    }

    /// The pool status: `Healthy` while every registered worker is
    /// READY, `Degraded` as soon as any worker is FAILED (the named,
    /// visible degradation — never a silent one), `Empty` with no
    /// workers. DRAINING/STOPPING/STARTING workers are neither healthy
    /// nor failed; the status names them.
    pub fn status(&self) -> PoolStatus {
        if self.workers.is_empty() {
            return PoolStatus::Empty;
        }
        let mut failed = Vec::new();
        let mut unavailable = Vec::new();
        for worker in self.workers.values() {
            match worker.state {
                SupervisedWorkerState::Failed => failed.push(worker.worker_id.clone()),
                SupervisedWorkerState::Ready => {}
                _ => unavailable.push(worker.worker_id.clone()),
            }
        }
        if failed.is_empty() {
            PoolStatus::Healthy { unavailable }
        } else {
            PoolStatus::Degraded { failed_workers: failed, unavailable }
        }
    }

    /// How many workers are in the acquire-eligible READY state.
    pub fn ready_worker_count(&self) -> usize {
        self.workers
            .values()
            .filter(|w| w.state == SupervisedWorkerState::Ready)
            .count()
    }

    fn worker(&self, worker_id: &str) -> Result<&SupervisedWorker, SupervisionError> {
        self.workers
            .get(worker_id)
            .ok_or_else(|| SupervisionError::UnknownWorker {
                worker_id: worker_id.to_string(),
            })
    }

    fn worker_mut(&mut self, worker_id: &str) -> Result<&mut SupervisedWorker, SupervisionError> {
        self.workers
            .get_mut(worker_id)
            .ok_or_else(|| SupervisionError::UnknownWorker {
                worker_id: worker_id.to_string(),
            })
    }
}

/// The pool's health, as the P7.20 aggregation and the console surface
/// it. Degradation is a named, visible state — never silent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PoolStatus {
    /// No workers registered.
    Empty,
    /// Every registered worker is READY; `unavailable` names any
    /// DRAINING/STOPPING/STARTING workers.
    Healthy { unavailable: Vec<String> },
    /// At least one worker is FAILED: the pool is degraded until an
    /// authorized recovery returns it to service.
    Degraded {
        failed_workers: Vec<String>,
        unavailable: Vec<String>,
    },
}

impl PoolStatus {
    /// True when any worker is FAILED (the degraded predicate).
    pub fn is_degraded(&self) -> bool {
        matches!(self, PoolStatus::Degraded { .. })
    }
}

// ---------------------------------------------------------------------------
// The transport seam
// ---------------------------------------------------------------------------

/// The outcome the real gRPC transport reports for one supervised call
/// (the seam between this pure module and the future
/// `flexcore.worker.v1` client). The transport implements the call;
/// this module implements every policy rule about the outcome.
///
/// UNVERIFIED — grpc/tonic client API: no gRPC client crate is a
/// dependency in this loop (no root `Cargo.toml` change); the transport
/// lands with the deployment-stage worker client and drives the pool
/// through exactly these outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportOutcome {
    /// The worker answered within the deadline with a contract-valid
    /// response (correlation echo present, provenance complete).
    Completed,
    /// The deadline elapsed before an answer.
    Timeout,
    /// The worker process died or the connection failed mid-request.
    Crash { reason: String },
    /// The worker answered but violated the contract.
    ContractViolation { reason: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal clock stub for the in-module tests (the integration
    /// suite uses the port's `FixedClock`).
    struct FixedMs(i64);

    impl Clock for FixedMs {
        fn now_event_time(&self) -> flexcore_contracts::time_model::EventTime {
            flexcore_contracts::time_model::EventTime::from_unix_millis(self.0)
                .expect("test instant is valid")
        }
        fn now_ingest_time(&self) -> flexcore_contracts::time_model::IngestTime {
            flexcore_contracts::time_model::IngestTime::from_unix_millis(self.0)
                .expect("test instant is valid")
        }
    }

    #[test]
    fn default_policy_is_valid() {
        assert!(SupervisionPolicy::default().validate().is_ok());
    }

    #[test]
    fn zero_timeout_is_refused() {
        let policy = SupervisionPolicy {
            call_timeout_ms: 0,
            ..SupervisionPolicy::default()
        };
        assert!(matches!(
            policy.validate(),
            Err(SupervisionError::InvalidPolicy { .. })
        ));
    }

    #[test]
    fn retry_schedule_is_a_pure_function_of_seed_and_attempt() {
        let mut policy = SupervisionPolicy::default();
        policy.seed = 42;
        let a = RetryScheduler::new(&policy);
        let b = RetryScheduler::new(&policy);
        for attempt in 0..6u32 {
            assert_eq!(a.delay_ms(attempt), b.delay_ms(attempt));
        }
    }

    #[test]
    fn delay_is_bounded_by_cap_plus_jitter() {
        let mut policy = SupervisionPolicy::default();
        policy.backoff_base_ms = 250;
        policy.backoff_cap_ms = 5_000;
        policy.jitter_cap_ms = 250;
        let scheduler = RetryScheduler::new(&policy);
        for attempt in 0..10u32 {
            let delay = scheduler.delay_ms(attempt);
            let factor = 1u64.checked_shl(attempt.min(31)).unwrap_or(u64::MAX);
            let exp = 250u64.saturating_mul(factor).min(5_000);
            assert!(delay >= exp && delay <= exp + 250);
        }
    }
}
