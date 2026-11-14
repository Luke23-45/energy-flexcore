// P6.31 — Edge time synchronization and clock-quality monitoring (report
//         §10.3, §17.4, §14.3, §17.5 rule 5; ledger P6.31; decision record
//         `25_31_32_console_clock_decommission.md`).
//
// This module OWNS the edge clock quality. The P6.15 four-check verifier
// (`command_verify.rs`) consumes the clock-quality status as a SUPPLIED
// PARAMETER (its recorded P6.31 seam) — this module is that parameter's
// producer. Nothing in `command_verify.rs` detects drift; everything here
// does.
//
// THE CLOSED CLOCK-QUALITY VOCABULARY (exactly, no fifth state):
//
// | State | Meaning |
// |---|---|
// | `Unknown` | no observation has arrived yet — the DEFAULT, and it is NEVER healthy-by-default (P0.09 §4: unknown is recorded, never assumed) |
// | `Synchronized` | the estimated offset is inside the configured tolerance |
// | `Degraded` | a sync observation landed, but the estimated offset is BEYOND the configured tolerance (clock drift, §17.4) |
// | `Unsynchronized` | the sync source is unreachable (explicitly noted, or no fresh observation within the sync timeout) |
//
// THE HOLD-ON-DRIFT RULE (§17.4/§14.3/§17.5 rule 5, P4.08 fail-closed):
// expiry checks are trusted ONLY on `Synchronized`. On `Degraded`,
// `Unsynchronized`, or `Unknown` the edge does NOT trust expiry checks —
// commands are rejected (held) fail-closed and the condition is AUDITED
// ([`ClockQualityMonitor::expiry_check_allowed`] returns the named reason
// the caller audits; through the P6.15 verifier the rejection is the
// verifier's `expiry` reason with its own audit event).
//
// SYNC SOURCE IS A NAMED, ALLOWLISTED CONFIGURATION REFERENCE — never an
// assumption that the host clock is correct. Per deployment profile:
//
// | Profile | Sync source (recorded per deployment) | DEFAULT tolerance | DEFAULT sync timeout |
// |---|---|---|---|
// | Lab | a named allowlisted reference (e.g. the lab's time service) | 500 ms | 60 000 ms |
// | Pilot | a named allowlisted reference (the site's existing time service; the D6 record must list it — P6.03) | 100 ms | 30 000 ms |
//
// The tolerance (maximum acceptable offset) and the timeout are
// CONFIGURED per profile; the constants above are recorded DEFAULTS
// (decision record §2), not measurements.
//
// NO NTP CLIENT IS WRITTEN. A network time protocol client would be a new
// dependency and a NEW D6 network path (the site network must permit the
// sync source — the P6.03 D6 boundary review owns that allowlist;
// `03_d6_boundary_review.md`). Offset estimation therefore uses ONLY the
// bounded time-sample pairs the platform already exchanges: the
// event_time-vs-ingest_time honesty pairs (§10.3) —
// [`TimeSamplePair`] carries one reference-instant/edge-instant pair and
// its offset. SYNC-SOURCE REACHABILITY IS A DEPLOYMENT REQUIREMENT
// (recorded in the decision record): if the site firewall blocks the
// allowlisted source, this module reports `Unsynchronized` and the edge
// holds commands — it never falls back to the host clock.
//
// OFFSET ESTIMATION: the monitor keeps a BOUNDED window of recent sample
// offsets ([`SyncSourceConfig::sample_window`], newest appended, oldest
// dropped) and estimates the offset as the MEDIAN of the window — robust
// to one corrupt or unlucky pair. A pair whose absolute offset exceeds
// [`MAX_ABS_OFFSET_MILLIS`] (one day) is REFUSED as a corrupt pair, never
// folded into the estimate (a corrupt pair is not a drift measurement).
//
// HEALTH SEAM (P6.17, additive): [`ClockQualityMonitor::health_record`]
// renders the clock quality as a P6.17 health record through the ONE
// additively-extended health reason (`CLOCK_DRIFT` — the reviewed closed-
// vocabulary extension this subtask records). `Unknown` renders as
// `DEGRADED_UNVERIFIED` (never healthy-by-default), `Degraded` and
// `Unsynchronized` render as `DEGRADED_CLOCK_DRIFT` (data flows but is
// flagged — the command path, not ingest, is what fails closed), and only
// `Synchronized` renders `HEALTHY`.
//
// AUDIT: every state TRANSITION is audited through the edge `AuditSink`
// port. `EdgeAuditKind` is a closed vocabulary this wave does not own
// (the recorded P6.14 limitation pattern), so the dedicated event name
// LEADS the detail string on kind `EDGE_STARTUP`
// (`EDGE_CLOCK_QUALITY_CHANGED: ...`).

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use flexcore_contracts::time_model::ClockQuality;
use flexcore_ports::clock::Clock;

use crate::health::{AdapterHealthRecord, HealthReason, HealthState};
use crate::ports::{EdgeAuditEvent, EdgeAuditKind};

// --- The closed state vocabulary -------------------------------------------------

/// The closed clock-quality state of the edge clock (module header table).
/// `Unknown` is the construction state — never healthy-by-default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClockQualityState {
    /// No observation has arrived yet. Expiry checks are NOT trusted.
    Unknown,
    /// The estimated offset is inside the configured tolerance.
    Synchronized,
    /// A sync observation landed but the estimated offset is beyond the
    /// configured tolerance (§17.4 clock drift). Expiry checks NOT trusted.
    Degraded,
    /// The sync source is unreachable (noted outage, or no fresh
    /// observation within the sync timeout). Expiry checks NOT trusted.
    Unsynchronized,
}

impl ClockQualityState {
    /// The stable SCREAMING_SNAKE name (audit/health/console rendering).
    pub fn name(self) -> &'static str {
        match self {
            ClockQualityState::Unknown => "UNKNOWN",
            ClockQualityState::Synchronized => "SYNCHRONIZED",
            ClockQualityState::Degraded => "DEGRADED",
            ClockQualityState::Unsynchronized => "UNSYNCHRONIZED",
        }
    }

    /// Whether expiry checks are trusted in this state. TRUE only for
    /// `Synchronized` — the hold-on-drift rule, fail-closed on every
    /// non-synchronized state INCLUDING the initial `Unknown`.
    pub fn is_expiry_trusted(self) -> bool {
        matches!(self, ClockQualityState::Synchronized)
    }

    /// The P6.15 verifier seam mapping into the CONTRACTS closed
    /// three-value [`ClockQuality`] vocabulary (the parameter type
    /// `CommandVerifier::verify` already declares). Recorded conformance
    /// note (decision record §2): the contracts vocabulary has no
    /// `Degraded` value, and a degraded clock is by definition NOT
    /// synchronized within tolerance — so `Degraded` maps to
    /// `ClockQuality::Unsynchronized` (a true statement), `Unknown` maps
    /// to `ClockQuality::Unknown` (the verifier fails it closed — verified
    /// against `command_verify.rs`), and only `Synchronized` maps to
    /// `ClockQuality::Synchronized`.
    pub fn for_p6_15_verifier(self) -> ClockQuality {
        match self {
            ClockQualityState::Synchronized => ClockQuality::Synchronized,
            ClockQualityState::Degraded | ClockQualityState::Unsynchronized => {
                ClockQuality::Unsynchronized
            }
            ClockQualityState::Unknown => ClockQuality::Unknown,
        }
    }
}

// --- Deployment profiles and recorded DEFAULT tolerances --------------------------

/// The deployment profiles that carry a recorded sync source + tolerance
/// (ledger P6.31 step 1: "record it per deployment profile").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeSyncProfile {
    /// The lab profile (test environment).
    Lab,
    /// The pilot site profile (v0.4.0 first site integration).
    Pilot,
}

impl TimeSyncProfile {
    pub fn as_str(self) -> &'static str {
        match self {
            TimeSyncProfile::Lab => "lab",
            TimeSyncProfile::Pilot => "pilot",
        }
    }

    /// Parses the profile from configuration text (fail-closed: an unknown
    /// profile is a named error, never a default).
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim() {
            "lab" => Some(TimeSyncProfile::Lab),
            "pilot" => Some(TimeSyncProfile::Pilot),
            _ => None,
        }
    }
}

/// Recorded DEFAULT tolerance for the lab profile (maximum acceptable
/// offset, milliseconds). A DEFAULT decision (decision record §2), not a
/// measurement.
pub const DEFAULT_LAB_TOLERANCE_MILLIS: i64 = 500;
/// Recorded DEFAULT tolerance for the pilot profile (milliseconds).
pub const DEFAULT_PILOT_TOLERANCE_MILLIS: i64 = 100;
/// Recorded DEFAULT sync timeout for the lab profile: no fresh sync
/// observation inside this window moves the state to `Unsynchronized`.
pub const DEFAULT_LAB_SYNC_TIMEOUT_MILLIS: i64 = 60_000;
/// Recorded DEFAULT sync timeout for the pilot profile (milliseconds).
pub const DEFAULT_PILOT_SYNC_TIMEOUT_MILLIS: i64 = 30_000;
/// Recorded DEFAULT bounded sample window (pairs kept for the median).
pub const DEFAULT_SAMPLE_WINDOW: usize = 8;
/// A sample pair whose absolute offset exceeds one day is refused as a
/// corrupt pair (a corrupt pair is not a drift measurement).
pub const MAX_ABS_OFFSET_MILLIS: i64 = 86_400_000;

// --- Configuration ------------------------------------------------------------------

/// The named sync-source configuration for ONE deployment profile. The
/// source reference is an ALLOWLISTED, deployment-recorded reference (the
/// D6 record must list it — P6.03); the edge never assumes the host clock
/// is correct and never discovers a source on its own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncSourceConfig {
    /// The deployment profile this configuration belongs to.
    pub profile: TimeSyncProfile,
    /// The named, allowlisted sync-source reference (configuration text;
    /// e.g. `"site-time-service-1"`). Non-empty — an unnamed source is a
    /// composition error (fail-closed).
    pub source_reference: String,
    /// The maximum acceptable offset in milliseconds (the tolerance).
    pub tolerance_millis: i64,
    /// No fresh sync observation inside this window → `Unsynchronized`.
    pub sync_timeout_millis: i64,
    /// The bounded sample window (pairs kept for the median estimate).
    pub sample_window: usize,
}

impl SyncSourceConfig {
    /// Builds the configuration with the profile's recorded DEFAULT
    /// tolerance and timeout (decision record §2). The source reference is
    /// mandatory: the deployment names its allowlisted source.
    pub fn for_profile(profile: TimeSyncProfile, source_reference: String) -> Self {
        let (tolerance, timeout) = match profile {
            TimeSyncProfile::Lab => (DEFAULT_LAB_TOLERANCE_MILLIS, DEFAULT_LAB_SYNC_TIMEOUT_MILLIS),
            TimeSyncProfile::Pilot => {
                (DEFAULT_PILOT_TOLERANCE_MILLIS, DEFAULT_PILOT_SYNC_TIMEOUT_MILLIS)
            }
        };
        Self {
            profile,
            source_reference,
            tolerance_millis: tolerance,
            sync_timeout_millis: timeout,
            sample_window: DEFAULT_SAMPLE_WINDOW,
        }
    }

    /// Validates the configuration (fail-closed: a nonsense tolerance, a
    /// non-positive timeout, an empty source reference, or a window
    /// smaller than one sample is a composition error, never a silently
    /// weakened check).
    pub fn validate(&self) -> Result<(), ClockQualityError> {
        if self.source_reference.trim().is_empty() {
            return Err(ClockQualityError::InvalidConfig {
                reason: "source_reference is empty (the sync source must be a named, \
                         allowlisted reference — never an assumption)"
                    .to_string(),
            });
        }
        if self.tolerance_millis <= 0 {
            return Err(ClockQualityError::InvalidConfig {
                reason: "tolerance_millis must be positive".to_string(),
            });
        }
        if self.sync_timeout_millis <= 0 {
            return Err(ClockQualityError::InvalidConfig {
                reason: "sync_timeout_millis must be positive".to_string(),
            });
        }
        if self.sample_window == 0 {
            return Err(ClockQualityError::InvalidConfig {
                reason: "sample_window must be at least one sample".to_string(),
            });
        }
        Ok(())
    }
}

// --- Errors ---------------------------------------------------------------------------

/// Named errors of the clock-quality monitor. Nothing is swallowed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClockQualityError {
    /// The configuration was rejected at construction.
    InvalidConfig { reason: String },
    /// The sample pair was refused (a corrupt pair is not a measurement).
    InvalidSample { reason: String },
}

impl std::fmt::Display for ClockQualityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClockQualityError::InvalidConfig { reason } => {
                write!(f, "the time-sync configuration is invalid: {reason}")
            }
            ClockQualityError::InvalidSample { reason } => {
                write!(f, "the time-sample pair was refused: {reason}")
            }
        }
    }
}

impl std::error::Error for ClockQualityError {}

// --- The sample pair (§10.3 event_time vs ingest_time honesty) -------------------------

/// ONE bounded time-sample pair: the reference (allowlisted sync source)
/// reading of an instant, the edge clock's reading of the SAME instant,
/// and when the pair was observed on the edge clock. The offset is
/// `edge - reference` (positive = the edge clock runs ahead).
///
/// NO NTP CLIENT IS WRITTEN (module header): the pairs come from the
/// exchange the platform already performs; acquiring pairs from a named,
/// allowlisted source is the deployment's wiring responsibility (a
/// deployment requirement recorded in the decision record).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeSamplePair {
    /// The reference instant (unix millis) per the allowlisted sync source.
    pub reference_unix_millis: i64,
    /// The same instant as the edge clock read it (unix millis).
    pub edge_unix_millis: i64,
    /// When the pair was observed (edge clock, unix millis).
    pub observed_at_unix_millis: i64,
}

impl TimeSamplePair {
    /// The pair's offset: `edge - reference` (positive = the edge clock is
    /// ahead of the reference).
    pub fn offset_millis(&self) -> i64 {
        self.edge_unix_millis
            .saturating_sub(self.reference_unix_millis)
    }
}

// --- The monitor -------------------------------------------------------------------------

#[derive(Debug)]
struct MonitorInner {
    /// The bounded window of `(offset, observed_at)` pairs, oldest first.
    samples: VecDeque<(i64, i64)>,
    /// The last DERIVED state (from the last accepted estimate).
    last_state: Option<ClockQualityState>,
    /// The observed_at of the last accepted sync observation.
    last_synced_at: Option<i64>,
}

/// The edge clock-quality monitor (P6.31). Composed once per edge with the
/// profile's [`SyncSourceConfig`]; the composition loop feeds
/// [`ClockQualityMonitor::observe_pair`] per exchanged pair and
/// [`ClockQualityMonitor::note_source_unreachable`] on a known outage, and
/// the P6.15 composition reads
/// [`ClockQualityMonitor::p6_15_clock_quality`] (the verifier's parameter
/// seam) plus [`ClockQualityMonitor::expiry_check_allowed`] (the audited
/// hold-on-drift gate).
pub struct ClockQualityMonitor {
    config: SyncSourceConfig,
    clock: Arc<dyn Clock + Send + Sync>,
    audit: Arc<dyn crate::ports::AuditSink>,
    inner: Mutex<MonitorInner>,
}

impl ClockQualityMonitor {
    /// Builds the monitor. The configuration is validated (fail-closed);
    /// the audit sink is the edge `AuditSink` port (state TRANSITIONS are
    /// audited through it).
    pub fn new(
        config: SyncSourceConfig,
        clock: Arc<dyn Clock + Send + Sync>,
        audit: Arc<dyn crate::ports::AuditSink>,
    ) -> Result<Self, ClockQualityError> {
        config.validate()?;
        Ok(Self {
            config,
            clock,
            audit,
            inner: Mutex::new(MonitorInner {
                samples: VecDeque::new(),
                last_state: None,
                last_synced_at: None,
            }),
        })
    }

    /// The configuration (visible to composition and tests).
    pub fn config(&self) -> &SyncSourceConfig {
        &self.config
    }

    /// Feeds ONE time-sample pair. The pair is validated (a corrupt pair
    /// with an absurd offset is refused, never folded into the estimate),
    /// appended to the bounded window (oldest dropped beyond the window),
    /// and the state is re-derived from the window's MEDIAN offset:
    /// inside tolerance → `Synchronized`, beyond → `Degraded`.
    ///
    /// A state TRANSITION is audited (the hold-on-drift condition is never
    /// silent). Returns the new state.
    pub fn observe_pair(&self, pair: TimeSamplePair) -> Result<ClockQualityState, ClockQualityError> {
        let offset = pair.offset_millis();
        if offset.abs() > MAX_ABS_OFFSET_MILLIS {
            return Err(ClockQualityError::InvalidSample {
                reason: format!(
                    "the sample pair's offset ({offset} ms) exceeds the corrupt-pair cap \
                     ({MAX_ABS_OFFSET_MILLIS} ms); a corrupt pair is refused, never estimated"
                ),
            });
        }
        let mut state_changed = false;
        let new_state = {
            let mut inner = self.inner.lock().expect("poisoned clock-quality mutex");
            inner.samples.push_back((offset, pair.observed_at_unix_millis));
            while inner.samples.len() > self.config.sample_window {
                inner.samples.pop_front();
            }
            let median = median_offset(&inner.samples);
            let derived = if median.abs() <= self.config.tolerance_millis {
                ClockQualityState::Synchronized
            } else {
                ClockQualityState::Degraded
            };
            state_changed = inner.last_state != Some(derived);
            inner.last_state = Some(derived);
            inner.last_synced_at = Some(pair.observed_at_unix_millis);
            derived
        };
        if state_changed {
            self.audit_transition(new_state, pair.observed_at_unix_millis);
        }
        Ok(new_state)
    }

    /// Notes an EXPLICIT sync-source outage (the deployment's wiring
    /// reports the allowlisted source unreachable — e.g. a failed fetch of
    /// a reference pair). `Unsynchronized` until a fresh in-tolerance pair
    /// arrives. A transition is audited. Calling this before ANY
    /// observation is still `Unsynchronized` (a noted outage is a known
    /// fact — the edge knows its source is down, which is more than
    /// `Unknown`).
    pub fn note_source_unreachable(&self, at_unix_millis: i64) -> ClockQualityState {
        let mut state_changed = false;
        let state = {
            let mut inner = self.inner.lock().expect("poisoned clock-quality mutex");
            state_changed = inner.last_state != Some(ClockQualityState::Unsynchronized);
            inner.last_state = Some(ClockQualityState::Unsynchronized);
            ClockQualityState::Unsynchronized
        };
        if state_changed {
            self.audit_transition(state, at_unix_millis);
        }
        state
    }

    /// The current clock-quality state AS OF `now_unix_millis`:
    ///
    /// 1. no observation ever → `Unknown` (never healthy-by-default);
    /// 2. the last sync observation is older than `sync_timeout_millis`
    ///    (the source went quiet) → `Unsynchronized`;
    /// 3. otherwise the last derived state (`Synchronized` / `Degraded`).
    ///
    /// Pure read: this method NEVER audits (it is called per command; the
    /// transition audit happens at the transition).
    pub fn quality_at(&self, now_unix_millis: i64) -> ClockQualityState {
        let inner = self.inner.lock().expect("poisoned clock-quality mutex");
        match (inner.last_state, inner.last_synced_at) {
            (None, _) => ClockQualityState::Unknown,
            (Some(state), Some(last_synced)) => {
                if now_unix_millis - last_synced > self.config.sync_timeout_millis {
                    ClockQualityState::Unsynchronized
                } else {
                    state
                }
            }
            (Some(state), None) => state,
        }
    }

    /// THE HOLD-ON-DRIFT GATE (§17.4/§14.3/P4.08): `Ok(())` only on
    /// `Synchronized`; otherwise `Err` with the named reason the caller
    /// must AUDIT (reject + audit, never silent). This is the edge-side
    /// pre-check; the P6.15 verifier enforces the same rule through the
    /// [`ClockQualityState::for_p6_15_verifier`] parameter and fails the
    /// expiry check closed on any non-`Synchronized` value.
    pub fn expiry_check_allowed(&self, now_unix_millis: i64) -> Result<(), String> {
        let state = self.quality_at(now_unix_millis);
        if state.is_expiry_trusted() {
            Ok(())
        } else {
            Err(format!(
                "edge clock quality is {} (sync source '{}'); expiry checks are NOT trusted on \
                 a non-synchronized clock — commands are rejected/held fail-closed per the \
                 P4.08 policy (§17.4, §17.5 rule 5; P6.31)",
                state.name(),
                self.config.source_reference
            ))
        }
    }

    /// The P6.15 verifier parameter (the recorded seam): the contracts
    /// [`ClockQuality`] value the composition passes to
    /// `CommandVerifier::verify`. See
    /// [`ClockQualityState::for_p6_15_verifier`] for the recorded mapping.
    pub fn p6_15_clock_quality(&self, now_unix_millis: i64) -> ClockQuality {
        self.quality_at(now_unix_millis).for_p6_15_verifier()
    }

    /// The clock quality as a P6.17 health state (the additive seam):
    /// `Synchronized` → `HEALTHY`; `Degraded` / `Unsynchronized` →
    /// `DEGRADED_CLOCK_DRIFT`; `Unknown` → `DEGRADED_UNVERIFIED` (never
    /// healthy-by-default). The mapping never invents a `FAILING_*` state
    /// for the clock: ingest keeps flowing while the clock is degraded
    /// (flagged), and the COMMAND path is the surface that fails closed —
    /// through the verifier seam, not through health.
    pub fn health_status(&self, now_unix_millis: i64) -> HealthState {
        match self.quality_at(now_unix_millis) {
            ClockQualityState::Synchronized => HealthState::Healthy,
            ClockQualityState::Degraded | ClockQualityState::Unsynchronized => {
                HealthState::Degraded(HealthReason::ClockDrift)
            }
            ClockQualityState::Unknown => HealthState::Degraded(HealthReason::Unverified),
        }
    }

    /// The clock quality as a P6.17 [`AdapterHealthRecord`] (the P6.25
    /// console reads the same record shape; the "adapter id" is the
    /// edge-wide clock track `edge-clock`).
    pub fn health_record(&self, now_unix_millis: i64) -> AdapterHealthRecord {
        let state = self.quality_at(now_unix_millis);
        let health = self.health_status(now_unix_millis);
        let detail = match state {
            ClockQualityState::Unknown => {
                "no sync observation has arrived yet; clock quality is UNKNOWN, never assumed \
                 synchronized (sync source '".to_string()
                    + &self.config.source_reference
                    + "')"
            }
            ClockQualityState::Synchronized => format!(
                "offset estimate inside the {} ms tolerance against sync source '{}'",
                self.config.tolerance_millis, self.config.source_reference
            ),
            ClockQualityState::Degraded => format!(
                "offset estimate BEYOND the {} ms tolerance against sync source '{}' — expiry \
                 checks are not trusted (hold-on-drift)",
                self.config.tolerance_millis, self.config.source_reference
            ),
            ClockQualityState::Unsynchronized => format!(
                "sync source '{}' unreachable or quiet beyond the {} ms timeout — expiry \
                 checks are not trusted (hold-on-drift)",
                self.config.sync_timeout_millis, self.config.source_reference
            ),
        };
        AdapterHealthRecord {
            adapter_id: "edge-clock".to_string(),
            state: health,
            last_seen_unix_millis: {
                let inner = self.inner.lock().expect("poisoned clock-quality mutex");
                inner.last_synced_at
            },
            last_event_age_millis: None,
            detail,
        }
    }

    /// Records ONE state-transition audit event. `EdgeAuditKind` is a
    /// closed vocabulary this wave does not own (the recorded P6.14
    /// limitation pattern), so the dedicated event name LEADS the detail
    /// string on kind `EDGE_STARTUP`. A failed audit write is logged
    /// loudly and never suppresses the state change (the clock fact is
    /// independent of the audit write, same discipline as
    /// `EnrollmentOrchestrator::assess_expiry`).
    fn audit_transition(&self, new_state: ClockQualityState, at_unix_millis: i64) {
        let detail = format!(
            "EDGE_CLOCK_QUALITY_CHANGED: clock quality is now {} at {} (sync source '{}', \
             tolerance {} ms)",
            new_state.name(),
            at_unix_millis,
            self.config.source_reference,
            self.config.tolerance_millis
        );
        let recorded_at = self.clock.now_ingest_time();
        let event = EdgeAuditEvent {
            recorded_at,
            kind: EdgeAuditKind::Startup,
            detail,
        };
        if let Err(error) = self.audit.record(event) {
            tracing::error!(
                %error,
                state = new_state.name(),
                "clock-quality TRANSITION AUDIT WRITE FAILED — the transition is NOT journaled"
            );
        }
    }
}

/// The median offset of the bounded window (integer arithmetic; for an
/// even count the mean of the two middle values, deterministic). An empty
/// window has no estimate (the caller never calls this empty — the monitor
/// guards it).
fn median_offset(samples: &VecDeque<(i64, i64)>) -> i64 {
    let mut offsets: Vec<i64> = samples.iter().map(|(offset, _)| *offset).collect();
    offsets.sort_unstable();
    let count = offsets.len();
    if count % 2 == 1 {
        offsets[count / 2]
    } else {
        let lower = offsets[count / 2 - 1];
        let upper = offsets[count / 2];
        lower + (upper - lower) / 2
    }
}

// --- The P4.08-style audited hold helper (composition-side) -----------------------------

/// Composes the hold-on-drift gate WITH its audit: `Ok(())` on
/// `Synchronized`; on any other state the named reason is recorded through
/// `audit` (dedicated event name leading the detail on the
/// closed-vocabulary kind, the recorded P6.14 limitation pattern) and the
/// error is returned. This is the function a command-path composition
/// calls BEFORE handing a command to the P6.15 verifier, so the hold
/// condition is audited even when no command arrives (a proactive hold is
/// also audited).
pub fn audited_expiry_check(
    monitor: &ClockQualityMonitor,
    audit: &dyn crate::ports::AuditSink,
    now_unix_millis: i64,
) -> Result<(), String> {
    match monitor.expiry_check_allowed(now_unix_millis) {
        Ok(()) => Ok(()),
        Err(reason) => {
            let recorded_at = monitor.clock.now_ingest_time();
            let event = EdgeAuditEvent {
                recorded_at,
                kind: EdgeAuditKind::Startup,
                detail: format!("EDGE_CLOCK_HOLD_ON_DRIFT: {reason}"),
            };
            if let Err(error) = audit.record(event) {
                tracing::error!(
                    %error,
                    "clock hold-on-drift AUDIT WRITE FAILED — the hold is NOT journaled"
                );
            }
            Err(reason)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flexcore_ports::fakes::FixedClock;

    const BASE: i64 = 1_700_000_000_000;

    fn monitor() -> ClockQualityMonitor {
        let config =
            SyncSourceConfig::for_profile(TimeSyncProfile::Pilot, "site-time-service-1".to_string());
        ClockQualityMonitor::new(
            config,
            Arc::new(FixedClock::at_unix_millis(BASE)),
            Arc::new(crate::ports::InMemoryAuditSink::new()),
        )
        .expect("valid config")
    }

    fn pair(edge_minus_reference: i64, at: i64) -> TimeSamplePair {
        TimeSamplePair {
            reference_unix_millis: at - edge_minus_reference,
            edge_unix_millis: at,
            observed_at_unix_millis: at,
        }
    }

    #[test]
    fn unknown_before_the_first_observation_and_never_trusted() {
        let monitor = monitor();
        assert_eq!(monitor.quality_at(BASE), ClockQualityState::Unknown);
        assert!(!ClockQualityState::Unknown.is_expiry_trusted());
        assert!(monitor.expiry_check_allowed(BASE).is_err());
        // The verifier seam maps Unknown → ClockQuality::Unknown, which the
        // P6.15 verifier fails closed (verified against command_verify.rs).
        assert_eq!(
            monitor.p6_15_clock_quality(BASE),
            ClockQuality::Unknown
        );
    }

    #[test]
    fn in_tolerance_offset_is_synchronized_and_trusted() {
        let monitor = monitor();
        let state = monitor
            .observe_pair(pair(80, BASE))
            .expect("valid pair (pilot tolerance 100 ms)");
        assert_eq!(state, ClockQualityState::Synchronized);
        assert_eq!(monitor.quality_at(BASE), ClockQualityState::Synchronized);
        assert!(monitor.expiry_check_allowed(BASE).is_ok());
        assert_eq!(
            monitor.p6_15_clock_quality(BASE),
            ClockQuality::Synchronized
        );
        assert_eq!(monitor.health_status(BASE), HealthState::Healthy);
    }

    #[test]
    fn offset_beyond_tolerance_is_degraded_and_hold_on_drift_applies() {
        let monitor = monitor();
        let state = monitor
            .observe_pair(pair(900, BASE))
            .expect("valid pair (900 ms beyond the 100 ms pilot tolerance)");
        assert_eq!(state, ClockQualityState::Degraded);
        assert!(!monitor.expiry_check_allowed(BASE).is_ok());
        // The seam maps Degraded → Unsynchronized (NOT synchronized within
        // tolerance — a true statement in the contracts vocabulary).
        assert_eq!(
            monitor.p6_15_clock_quality(BASE),
            ClockQuality::Unsynchronized
        );
        let health = monitor.health_record(BASE);
        assert_eq!(health.status_name(), "DEGRADED_CLOCK_DRIFT");
        assert!(health.detail.contains("BEYOND"));
    }

    #[test]
    fn the_tolerance_boundary_is_respected_inclusively() {
        let monitor = monitor();
        // Exactly AT the tolerance (100 ms pilot default) is INSIDE it:
        let state = monitor.observe_pair(pair(100, BASE)).expect("pair");
        assert_eq!(state, ClockQualityState::Synchronized);
        // One millisecond beyond it is Degraded:
        let state = monitor.observe_pair(pair(101, BASE + 1_000)).expect("pair");
        assert_eq!(state, ClockQualityState::Degraded);
    }

    #[test]
    fn a_source_outage_is_unsynchronized_and_recovery_restores_trust() {
        let monitor = monitor();
        monitor.observe_pair(pair(10, BASE)).expect("pair");
        // Explicit outage:
        let state = monitor.note_source_unreachable(BASE + 1_000);
        assert_eq!(state, ClockQualityState::Unsynchronized);
        assert!(monitor.expiry_check_allowed(BASE + 2_000).is_err());
        assert_eq!(
            monitor.p6_15_clock_quality(BASE + 2_000),
            ClockQuality::Unsynchronized
        );
        // Recovery: a fresh in-tolerance pair restores Synchronized.
        let state = monitor
            .observe_pair(pair(10, BASE + 5_000))
            .expect("pair");
        assert_eq!(state, ClockQualityState::Synchronized);
        assert!(monitor.expiry_check_allowed(BASE + 5_000).is_ok());
        assert_eq!(monitor.health_status(BASE + 5_000), HealthState::Healthy);
    }

    #[test]
    fn a_quiet_source_ages_into_unsynchronized_past_the_timeout() {
        let monitor = monitor(); // pilot timeout default 30_000 ms
        monitor.observe_pair(pair(10, BASE)).expect("pair");
        // Just inside the timeout: still the derived state.
        assert_eq!(
            monitor.quality_at(BASE + DEFAULT_PILOT_SYNC_TIMEOUT_MILLIS),
            ClockQualityState::Synchronized
        );
        // One millisecond past it: the source went quiet → Unsynchronized.
        assert_eq!(
            monitor.quality_at(BASE + DEFAULT_PILOT_SYNC_TIMEOUT_MILLIS + 1),
            ClockQualityState::Unsynchronized
        );
    }

    #[test]
    fn a_corrupt_pair_is_refused_never_estimated() {
        let monitor = monitor();
        let error = monitor
            .observe_pair(pair(MAX_ABS_OFFSET_MILLIS + 1, BASE))
            .expect_err("a one-day offset is a corrupt pair, refused");
        assert!(matches!(error, ClockQualityError::InvalidSample { .. }));
        // The refusal left the state untouched (still Unknown).
        assert_eq!(monitor.quality_at(BASE), ClockQualityState::Unknown);
    }

    #[test]
    fn the_median_of_the_bounded_window_is_the_estimate() {
        let monitor = monitor();
        // Window of 3: offsets 900, 900, 10 → median 900 → Degraded; the
        // single good pair does not mask the drift (and vice versa: the
        // bounded window keeps the estimate robust to one unlucky pair).
        monitor.observe_pair(pair(900, BASE)).expect("pair");
        monitor.observe_pair(pair(900, BASE + 1_000)).expect("pair");
        let state = monitor
            .observe_pair(pair(10, BASE + 2_000))
            .expect("pair");
        assert_eq!(state, ClockQualityState::Degraded);
    }

    #[test]
    fn nonsense_configuration_is_a_named_composition_error() {
        let mut config =
            SyncSourceConfig::for_profile(TimeSyncProfile::Lab, "lab-time-1".to_string());
        config.source_reference = "  ".to_string();
        assert!(matches!(
            SyncSourceConfig::validate(&config),
            Err(ClockQualityError::InvalidConfig { .. })
        ));
        let mut config =
            SyncSourceConfig::for_profile(TimeSyncProfile::Lab, "lab-time-1".to_string());
        config.tolerance_millis = 0;
        assert!(matches!(
            SyncSourceConfig::validate(&config),
            Err(ClockQualityError::InvalidConfig { .. })
        ));
        let mut config =
            SyncSourceConfig::for_profile(TimeSyncProfile::Lab, "lab-time-1".to_string());
        config.sample_window = 0;
        assert!(matches!(
            SyncSourceConfig::validate(&config),
            Err(ClockQualityError::InvalidConfig { .. })
        ));
    }

    #[test]
    fn the_hold_on_drift_gate_audits_its_refusals() {
        let monitor = monitor();
        let audit = crate::ports::InMemoryAuditSink::new();
        // No observation yet → hold, and the hold is AUDITED.
        let error = audited_expiry_check(&monitor, &audit, BASE)
            .expect_err("Unknown holds commands");
        assert!(error.contains("UNKNOWN"));
        let events = audit.events();
        assert_eq!(events.len(), 1);
        assert!(
            events[0].detail.starts_with("EDGE_CLOCK_HOLD_ON_DRIFT"),
            "the dedicated event name leads the detail: {}",
            events[0].detail
        );
        // Synchronized → no hold, no event.
        monitor.observe_pair(pair(5, BASE)).expect("pair");
        assert!(audited_expiry_check(&monitor, &audit, BASE).is_ok());
        assert_eq!(audit.events().len(), 1);
    }

    #[test]
    fn unknown_renders_as_degraded_unverified_never_healthy() {
        let monitor = monitor();
        let record = monitor.health_record(BASE);
        assert_eq!(record.status_name(), "DEGRADED_UNVERIFIED");
        assert!(record.detail.contains("UNKNOWN"));
        assert_eq!(record.adapter_id, "edge-clock");
    }
}
