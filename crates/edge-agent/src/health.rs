// P6.17 — Per-adapter health monitoring (report §18.1, §18.3, §21.3; ledger
//         P6.17 steps 2–4).
//
// THE CLOSED HEALTH VOCABULARY (exactly, no seventh state):
//
// | Status | Meaning |
// |---|---|
// | `HEALTHY` | observed recently, data fresh, error rate inside limits, credential valid |
// | `DEGRADED_<reason>` | up but impaired: `UNVERIFIED`, `STALE_DATA`, `ERROR_RATE`, `POLICY_BLOCK`, `CREDENTIAL_EXPIRY` |
// | `FAILING_<reason>` | dependent actions are BLOCKED: `UNRESPONSIVE`, `CREDENTIAL_EXPIRY` |
//
// ADDITIVE EXTENSION (P6.31, the reviewed closed-vocabulary change cited in
// `ledger/phase-06-edge-protocol/25_31_32_console_clock_decommission.md`):
// the DEGRADED reason set gains `CLOCK_DRIFT` — the edge clock-quality
// input produced by `clock_quality.rs`. No existing variant or derivation
// rule is modified; the clock quality enters health only through
// `clock_quality.rs`'s seam, never through `assess` (which stays
// adapter-scoped).
//
// THE HARD RULE (the ledger's risk row): health is NEVER inferred from the
// absence of errors. "Unknown = healthy" is forbidden — an adapter with NO
// observation yet is `DEGRADED_UNVERIFIED`, and it stays degraded until the
// first observation lands. Every state is DERIVED from observed inputs:
// responsive checks, data staleness against the freshness limit, the error
// rate over a bounded window, the certificate expiry input (from the P6.06
// credential store), and last-seen timestamps. Silence produces
// `FAILING_UNRESPONSIVE` only through the explicit detection window on the
// LAST OBSERVED fact (`now - last_seen > unresponsive_after`) — a positive,
// timestamped rule, never a default.
//
// BLOCKED DEPENDENT ACTIONS: while an adapter is `FAILING_*`, ingesting
// from it and any command targeting it are blocked with an audited reason
// ([`AdapterHealthMonitor::check_action_allowed`]). A `DEGRADED_*` adapter
// may still flow — its data arrives flagged (P0.08) and its command-path
// gate is the P6.17 policy engine's job — the failure boundary is
// `FAILING_*`, exactly as the ledger names it.
//
// CONSOLE SURFACING IS P6.25's JOB (§20.1 Sites workspace). This module only
// EXPOSES the data: [`HealthSnapshot`] and [`AdapterHealthRecord`] are plain
// data with a `json()` rendering (the crate has no serde dependency this
// wave — see the decision doc's P6.25 seam note). Edge logs carry the
// adapter id and the correlation context the composition supplies — the
// same discipline as the audit events (no secrets, no raw backend text,
// P1.08).

use std::collections::HashMap;

// --- The closed vocabulary -----------------------------------------------------

/// The closed set of health REASONS. Extending this set is a reviewed change
/// (a closed vocabulary is the point — an open string field would let
/// "healthy-ish" back in).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthReason {
    /// No observation has arrived yet — the adapter is UNVERIFIED, never
    /// healthy-by-default (the forbidden-inference rule).
    Unverified,
    /// The last responsive check failed, or no observation arrived within
    /// the detection window.
    Unresponsive,
    /// The adapter's data is older than the freshness limit.
    StaleData,
    /// The error rate over the bounded window exceeded the threshold.
    ErrorRate,
    /// A site-policy violation targeted this adapter (the composition notes
    /// it here — the policy engine itself owns refusal).
    PolicyBlock,
    /// The device credential (P6.06 store) is expired or inside its
    /// rotation window.
    CredentialExpiry,
    // P6.31 (additive, reviewed change — see the header note): the edge
    // clock-quality input (`clock_quality.rs`) reports a degraded or
    // unsynchronized clock. Data flows but is flagged; the command path is
    // what fails closed (the P6.15 verifier seam), so this is DEGRADED_,
    // never FAILING_.
    ClockDrift,
}

impl HealthReason {
    /// The SCREAMING_SNAKE reason token used inside the status name.
    pub fn token(self) -> &'static str {
        match self {
            HealthReason::Unverified => "UNVERIFIED",
            HealthReason::Unresponsive => "UNRESPONSIVE",
            HealthReason::StaleData => "STALE_DATA",
            HealthReason::ErrorRate => "ERROR_RATE",
            HealthReason::PolicyBlock => "POLICY_BLOCK",
            HealthReason::CredentialExpiry => "CREDENTIAL_EXPIRY",
            HealthReason::ClockDrift => "CLOCK_DRIFT",
        }
    }
}

/// The closed health status vocabulary: HEALTHY, DEGRADED_<reason>,
/// FAILING_<reason> — exactly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthState {
    Healthy,
    Degraded(HealthReason),
    Failing(HealthReason),
}

impl HealthState {
    /// The status name (`HEALTHY`, `DEGRADED_STALE_DATA`, `FAILING_*` …) —
    /// the closed string form the console and logs render.
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

// --- Inputs and limits -----------------------------------------------------------

/// One observed fact about an adapter, fed by the composition loop (the
/// ingest path and the uplink/EMS checks). The monitor DERIVES health from
/// these; it never invents them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdapterObservation {
    /// A responsive check succeeded: the adapter answered and produced (or
    /// could produce) data whose event_time lags the edge clock by
    /// `event_age_millis`.
    Responsive { event_age_millis: i64 },
    /// A poll/read of the adapter failed (the adapter did not answer): the
    /// named, human-safe reason (never raw backend text, P1.08).
    PollFailed { reason: String },
}

/// The health derivation limits (validated at construction — a nonsense
/// limit is a composition error, never a silently weakened check).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HealthLimits {
    /// Data older than this (event_time behind the edge clock) is
    /// DEGRADED_STALE_DATA.
    pub freshness_limit_millis: i64,
    /// No observation within this window after the last seen one →
    /// FAILING_UNRESPONSIVE (the detection window).
    pub unresponsive_after_millis: i64,
    /// The bounded error-rate window.
    pub error_rate_window_millis: i64,
    /// Errors within the window at or above this count →
    /// DEGRADED_ERROR_RATE.
    pub max_errors_in_window: u32,
    /// The P6.06 credential rotation window: an expiry inside this window
    /// is DEGRADED_CREDENTIAL_EXPIRY (visible at least one rotation window
    /// before expiry — the P6.06 acceptance rule).
    pub credential_rotation_window_millis: i64,
}

impl HealthLimits {
    fn validate(&self) -> Result<(), HealthError> {
        let positive = [
            ("freshness_limit_millis", self.freshness_limit_millis),
            ("unresponsive_after_millis", self.unresponsive_after_millis),
            ("error_rate_window_millis", self.error_rate_window_millis),
            (
                "credential_rotation_window_millis",
                self.credential_rotation_window_millis,
            ),
        ];
        for (name, value) in positive {
            if value <= 0 {
                return Err(HealthError::InvalidLimits {
                    reason: format!("{name} must be positive"),
                });
            }
        }
        if self.max_errors_in_window == 0 {
            return Err(HealthError::InvalidLimits {
                reason: "max_errors_in_window must be positive".to_string(),
            });
        }
        Ok(())
    }
}

// --- Errors -----------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HealthError {
    /// Health was queried for an adapter this monitor has never seen: the
    /// honest answer is a named error, not a status (the caller composes
    /// the monitor over its adapter set; an unknown id is a wiring bug and
    /// must be loud).
    UnknownAdapter { adapter_id: String },
    InvalidLimits { reason: String },
}

impl std::fmt::Display for HealthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HealthError::UnknownAdapter { adapter_id } => {
                write!(
                    f,
                    "no health track exists for adapter '{adapter_id}' (it was never observed \
                     or registered); health is unknown, not healthy"
                )
            }
            HealthError::InvalidLimits { reason } => {
                write!(f, "invalid health limits: {reason}")
            }
        }
    }
}

impl std::error::Error for HealthError {}

// --- The per-adapter track ----------------------------------------------------------

/// The observed state of ONE adapter (the monitor's private record). Every
/// field is observation-fed; there is no default-healthy path anywhere.
#[derive(Debug, Clone, PartialEq, Eq)]
struct AdapterHealthTrack {
    /// Whether ANY observation has landed (the Unverified gate).
    observed: bool,
    /// The last time the adapter was SEEN responsive (unix millis).
    last_seen_unix_millis: Option<i64>,
    /// The event_time of the last responsive observation's data.
    last_event_unix_millis: Option<i64>,
    /// The last failed-check reason (human-safe).
    last_failure_reason: Option<String>,
    /// The last responsive check FAILED (a later Responsive clears it).
    unresponsive: bool,
    /// Error timestamps inside the bounded window (pruned on assess).
    error_times: Vec<i64>,
    /// A site-policy violation targeted this adapter (noted by the
    /// composition when the P6.14 receiver reports a policy refusal).
    policy_blocked: bool,
    /// The device credential's expiry, when the P6.06 store records one
    /// (`None` = no expiry recorded — the shipped-seam shape, reported in
    /// the record detail, never treated as "valid forever" for STATUS
    /// purposes and never silently either).
    cert_expires_at_unix_millis: Option<i64>,
}

impl AdapterHealthTrack {
    fn new() -> Self {
        Self {
            observed: false,
            last_seen_unix_millis: None,
            last_event_unix_millis: None,
            last_failure_reason: None,
            unresponsive: false,
            error_times: Vec::new(),
            policy_blocked: false,
            cert_expires_at_unix_millis: None,
        }
    }
}

// --- The health record and snapshot ---------------------------------------------------

/// One adapter's health, as of one assessment. Plain data — P6.25 renders
/// it (the console seam note is in the decision doc).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdapterHealthRecord {
    pub adapter_id: String,
    /// The closed-vocabulary status (`HEALTHY` / `DEGRADED_*` /
    /// `FAILING_*`).
    pub state: HealthState,
    /// Last seen responsive (unix millis), when observed at all.
    pub last_seen_unix_millis: Option<i64>,
    /// The age of the last data (edge clock minus event_time), when known.
    pub last_event_age_millis: Option<i64>,
    /// The human-safe detail behind the state (never raw backend text).
    pub detail: String,
}

impl AdapterHealthRecord {
    /// The status name string (the closed vocabulary, rendered).
    pub fn status_name(&self) -> String {
        self.state.name()
    }

    /// Whether dependent actions (ingest, command targeting) are blocked.
    pub fn blocks_dependent_actions(&self) -> bool {
        self.state.blocks_dependent_actions()
    }

    /// A flat JSON object rendering (the crate carries no serde dependency
    /// this wave; the strings are escaped — adapter ids are configuration
    /// values, but escape anyway).
    pub fn json(&self) -> String {
        format!(
            "{{\"adapter_id\":{},\"status\":{},\"last_seen_unix_millis\":{},\
             \"last_event_age_millis\":{},\"detail\":{}}}",
            json_string(&self.adapter_id),
            json_string(&self.status_name()),
            match self.last_seen_unix_millis {
                Some(t) => t.to_string(),
                None => "null".to_string(),
            },
            match self.last_event_age_millis {
                Some(age) => age.to_string(),
                None => "null".to_string(),
            },
            json_string(&self.detail),
        )
    }
}

/// The monitor-wide snapshot (P6.25's data source for the Sites workspace).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HealthSnapshot {
    /// One record per tracked adapter, ordered by adapter id (stable
    /// rendering).
    pub adapters: Vec<AdapterHealthRecord>,
}

impl HealthSnapshot {
    /// A flat JSON array rendering.
    pub fn json(&self) -> String {
        let records: Vec<String> = self.adapters.iter().map(AdapterHealthRecord::json).collect();
        format!("{{\"adapters\":[{}]}}", records.join(","))
    }
}

/// Escapes a string for the JSON rendering (the two mandatory escapes plus
/// the control characters a config string could plausibly carry).
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

// --- The monitor ----------------------------------------------------------------------

/// The kinds of dependent actions (for the audited block reason).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockedAction {
    /// Ingesting telemetry FROM the adapter.
    Ingest,
    /// A command TARGETING the adapter.
    Command,
}

impl BlockedAction {
    pub fn as_str(self) -> &'static str {
        match self {
            BlockedAction::Ingest => "ingest",
            BlockedAction::Command => "command",
        }
    }
}

/// The per-adapter health monitor. Composed by the supervision loop: the
/// loop feeds [`AdapterHealthMonitor::observe`] per poll, the P6.14
/// composition notes policy blocks, the P6.06 expiry assessment feeds the
/// credential input, and P6.25 reads the snapshot.
#[derive(Debug)]
pub struct AdapterHealthMonitor {
    limits: HealthLimits,
    tracks: HashMap<String, AdapterHealthTrack>,
}

impl AdapterHealthMonitor {
    /// Builds the monitor over its limits.
    pub fn new(limits: HealthLimits) -> Result<Self, HealthError> {
        limits.validate()?;
        Ok(Self {
            limits,
            tracks: HashMap::new(),
        })
    }

    /// The limits (visible to composition and tests).
    pub fn limits(&self) -> &HealthLimits {
        &self.limits
    }

    /// Ensures a track exists for the adapter (registration by observation;
    /// a first observation lands on a fresh track in the UNVERIFIED state
    /// the instant before it is applied).
    fn track(&mut self, adapter_id: &str) -> &mut AdapterHealthTrack {
        self.tracks
            .entry(adapter_id.to_string())
            .or_insert_with(AdapterHealthTrack::new)
    }

    /// Feeds ONE observation. This is the only way state enters the
    /// monitor — health is derived from observations, never from silence.
    pub fn observe(&mut self, adapter_id: &str, at_unix_millis: i64, observation: AdapterObservation) {
        let track = self.track(adapter_id);
        track.observed = true;
        match observation {
            AdapterObservation::Responsive { event_age_millis } => {
                track.unresponsive = false;
                track.last_failure_reason = None;
                track.last_seen_unix_millis = Some(at_unix_millis);
                track.last_event_unix_millis = Some(at_unix_millis - event_age_millis);
            }
            AdapterObservation::PollFailed { reason } => {
                // A failed check is an OBSERVATION of failure: it does not
                // move last_seen (nothing was seen), it counts as an error
                // in the bounded window, and it marks the last responsive
                // check as failed.
                track.unresponsive = true;
                track.last_failure_reason = Some(reason);
                track.error_times.push(at_unix_millis);
            }
        }
    }

    /// Notes that a site-policy violation targeted this adapter (the
    /// composition calls this when the P6.14 receiver reports
    /// `CommandRefusal::PolicyRefused`). A later successful policy window
    /// clears it via [`AdapterHealthMonitor::clear_policy_block`].
    pub fn note_policy_block(&mut self, adapter_id: &str) {
        self.track(adapter_id).policy_blocked = true;
    }

    /// Clears a noted policy block (the composition decides when — e.g.
    /// after a policy window rolls over).
    pub fn clear_policy_block(&mut self, adapter_id: &str) {
        self.track(adapter_id).policy_blocked = false;
    }

    /// Feeds the certificate-expiry input from the P6.06 credential store
    /// (`expires_at_unix_millis`; `None` = the record records no expiry —
    /// the shipped-seam shape, kept in the detail, never interpreted as
    /// "valid forever").
    pub fn set_credential_expiry(&mut self, adapter_id: &str, expires_at_unix_millis: Option<i64>) {
        self.track(adapter_id).cert_expires_at_unix_millis = expires_at_unix_millis;
    }

    /// Assesses ONE adapter's health AS OF `now_unix_millis`. Derivation
    /// order (first match wins; FAILING outranks DEGRADED; every rule is a
    /// positive check on observed inputs):
    ///
    /// 1. never observed → `DEGRADED_UNVERIFIED` (never healthy);
    /// 2. last responsive check failed, or no observation within the
    ///    detection window → `FAILING_UNRESPONSIVE`;
    /// 3. credential expired → `FAILING_CREDENTIAL_EXPIRY`;
    /// 4. policy block noted → `DEGRADED_POLICY_BLOCK`;
    /// 5. credential inside the rotation window →
    ///    `DEGRADED_CREDENTIAL_EXPIRY`;
    /// 6. data older than the freshness limit → `DEGRADED_STALE_DATA`;
    /// 7. error rate at/above the threshold in the bounded window →
    ///    `DEGRADED_ERROR_RATE`;
    /// 8. otherwise — and ONLY otherwise — `HEALTHY`.
    pub fn assess(
        &self,
        adapter_id: &str,
        now_unix_millis: i64,
    ) -> Result<AdapterHealthRecord, HealthError> {
        let track = self.tracks.get(adapter_id).ok_or_else(|| HealthError::UnknownAdapter {
            adapter_id: adapter_id.to_string(),
        })?;
        // 1. The Unverified gate — unknown is NEVER healthy.
        if !track.observed {
            return Ok(AdapterHealthRecord {
                adapter_id: adapter_id.to_string(),
                state: HealthState::Degraded(HealthReason::Unverified),
                last_seen_unix_millis: None,
                last_event_age_millis: None,
                detail: "no observation has arrived yet; health is UNVERIFIED, never \
                         assumed healthy"
                    .to_string(),
            });
        }
        // 2. Responsiveness (the detection window on the last seen fact).
        let seen_recently = track
            .last_seen_unix_millis
            .map(|last_seen| now_unix_millis - last_seen <= self.limits.unresponsive_after_millis)
            .unwrap_or(false);
        if track.unresponsive || !seen_recently {
            return Ok(AdapterHealthRecord {
                adapter_id: adapter_id.to_string(),
                state: HealthState::Failing(HealthReason::Unresponsive),
                last_seen_unix_millis: track.last_seen_unix_millis,
                last_event_age_millis: track.last_event_age(now_unix_millis),
                detail: track.failure_detail("the adapter did not answer its responsive check"),
            });
        }
        // 3. Credential expiry (hard failed).
        if let Some(expires_at) = track.cert_expires_at_unix_millis {
            if now_unix_millis >= expires_at {
                return Ok(AdapterHealthRecord {
                    adapter_id: adapter_id.to_string(),
                    state: HealthState::Failing(HealthReason::CredentialExpiry),
                    last_seen_unix_millis: track.last_seen_unix_millis,
                    last_event_age_millis: track.last_event_age(now_unix_millis),
                    detail: format!(
                        "the device credential EXPIRED at {expires_at} (P6.06 credential \
                         store); the channel must not run on it"
                    ),
                });
            }
        }
        // 4. Policy block.
        if track.policy_blocked {
            return Ok(AdapterHealthRecord {
                adapter_id: adapter_id.to_string(),
                state: HealthState::Degraded(HealthReason::PolicyBlock),
                last_seen_unix_millis: track.last_seen_unix_millis,
                last_event_age_millis: track.last_event_age(now_unix_millis),
                detail: "a site-policy violation targeted this adapter; the policy engine \
                         owns the refusal, health surfaces the degradation"
                    .to_string(),
            });
        }
        // 5. Credential rotation due.
        if let Some(expires_at) = track.cert_expires_at_unix_millis {
            let remaining = expires_at - now_unix_millis;
            if remaining <= self.limits.credential_rotation_window_millis {
                return Ok(AdapterHealthRecord {
                    adapter_id: adapter_id.to_string(),
                    state: HealthState::Degraded(HealthReason::CredentialExpiry),
                    last_seen_unix_millis: track.last_seen_unix_millis,
                    last_event_age_millis: track.last_event_age(now_unix_millis),
                    detail: format!(
                        "the device credential enters the rotation window ({remaining} ms \
                         remaining; renewal must start now)"
                    ),
                });
            }
        }
        // 6. Data staleness (against the freshness limit).
        let stale = track
            .last_event_unix_millis
            .map(|last_event| now_unix_millis - last_event > self.limits.freshness_limit_millis)
            .unwrap_or(true); // no data event ever seen: NOT fresh — flagged, not healthy
        if stale {
            return Ok(AdapterHealthRecord {
                adapter_id: adapter_id.to_string(),
                state: HealthState::Degraded(HealthReason::StaleData),
                last_seen_unix_millis: track.last_seen_unix_millis,
                last_event_age_millis: track.last_event_age(now_unix_millis),
                detail: track.failure_detail("the adapter's data is older than the freshness limit"),
            });
        }
        // 7. Error rate over the bounded window.
        let window_start = now_unix_millis - self.limits.error_rate_window_millis;
        let errors_in_window = track
            .error_times
            .iter()
            .filter(|&&at| at > window_start && at <= now_unix_millis)
            .count();
        if errors_in_window as u32 >= self.limits.max_errors_in_window {
            return Ok(AdapterHealthRecord {
                adapter_id: adapter_id.to_string(),
                state: HealthState::Degraded(HealthReason::ErrorRate),
                last_seen_unix_millis: track.last_seen_unix_millis,
                last_event_age_millis: track.last_event_age(now_unix_millis),
                detail: format!(
                    "{errors_in_window} errors inside the {} ms window (threshold {})",
                    self.limits.error_rate_window_millis, self.limits.max_errors_in_window
                ),
            });
        }
        // 8. HEALTHY — reached only through every positive check above.
        Ok(AdapterHealthRecord {
            adapter_id: adapter_id.to_string(),
            state: HealthState::Healthy,
            last_seen_unix_millis: track.last_seen_unix_millis,
            last_event_age_millis: track.last_event_age(now_unix_millis),
            detail: "responsive, data fresh, error rate inside limits, credential valid"
                .to_string(),
        })
    }

    /// Whether a dependent action against the adapter is allowed AS OF
    /// `now`. Only `FAILING_*` blocks; the block reason is returned for the
    /// caller's AUDITED refusal (reject + audit, never silent).
    pub fn check_action_allowed(
        &self,
        adapter_id: &str,
        action: BlockedAction,
        now_unix_millis: i64,
    ) -> Result<(), String> {
        let record = self.assess(adapter_id, now_unix_millis)?;
        if record.blocks_dependent_actions() {
            Err(format!(
                "{} against adapter '{}' is BLOCKED while health is {} (dependent actions are \
                 blocked on FAILING_*): {}",
                action.as_str(),
                adapter_id,
                record.status_name(),
                record.detail
            ))
        } else {
            Ok(())
        }
    }

    /// The snapshot across ALL tracked adapters, ordered by adapter id
    /// (stable rendering for P6.25).
    pub fn snapshot(&self, now_unix_millis: i64) -> HealthSnapshot {
        let mut ids: Vec<&String> = self.tracks.keys().collect();
        ids.sort();
        let mut adapters = Vec::with_capacity(ids.len());
        for id in ids {
            // Every tracked adapter was observed or registered through a
            // feed call, so assess cannot return UnknownAdapter here; an
            // impossible error degrades to an UNVERIFIED record rather than
            // panicking (health never crashes the loop).
            let record = self
                .assess(id, now_unix_millis)
                .unwrap_or_else(|_error| AdapterHealthRecord {
                    adapter_id: id.clone(),
                    state: HealthState::Degraded(HealthReason::Unverified),
                    last_seen_unix_millis: None,
                    last_event_age_millis: None,
                    detail: format!("health assessment failed: {_error}"),
                });
            adapters.push(record);
        }
        HealthSnapshot { adapters }
    }
}

impl AdapterHealthTrack {
    fn last_event_age(&self, now_unix_millis: i64) -> Option<i64> {
        self.last_event_unix_millis
            .map(|event| now_unix_millis - event)
    }

    /// The human-safe detail: the recorded failure reason when present,
    /// else the supplied fallback (never raw backend text, P1.08).
    fn failure_detail(&self, fallback: &str) -> String {
        match &self.last_failure_reason {
            Some(reason) => reason.clone(),
            None => fallback.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn limits() -> HealthLimits {
        HealthLimits {
            freshness_limit_millis: 30_000,
            unresponsive_after_millis: 60_000,
            error_rate_window_millis: 300_000,
            max_errors_in_window: 3,
            credential_rotation_window_millis: 7 * 86_400_000,
        }
    }

    const BASE: i64 = 1_700_000_000_000;

    #[test]
    fn health_is_never_healthy_before_the_first_observation() {
        let mut monitor = AdapterHealthMonitor::new(limits()).expect("valid limits");
        // The adapter was REGISTERED (expiry fed) but never observed:
        // DEGRADED_UNVERIFIED, never HEALTHY.
        monitor.set_credential_expiry("simulated-test", Some(BASE + 86_400_000));
        let record = monitor.assess("simulated-test", BASE).expect("assessed");
        assert_eq!(record.status_name(), "DEGRADED_UNVERIFIED");
        assert!(!record.blocks_dependent_actions());
    }

    #[test]
    fn an_unobserved_adapter_is_an_unknown_track_named_error() {
        let monitor = AdapterHealthMonitor::new(limits()).expect("valid limits");
        assert!(matches!(
            monitor.assess("ghost", BASE),
            Err(HealthError::UnknownAdapter { .. })
        ));
    }

    #[test]
    fn adapter_failure_becomes_failing_unresponsive_within_the_detection_window() {
        let mut monitor = AdapterHealthMonitor::new(limits()).expect("valid limits");
        // Observed responsive at BASE.
        monitor.observe(
            "simulated-test",
            BASE,
            AdapterObservation::Responsive { event_age_millis: 1_000 },
        );
        assert_eq!(
            monitor.assess("simulated-test", BASE + 1_000).expect("assessed").status_name(),
            "HEALTHY"
        );
        // The adapter dies: NO further observation. At 60 s after last
        // seen the detection window closes → FAILING_UNRESPONSIVE.
        let record = monitor
            .assess("simulated-test", BASE + 61_000)
            .expect("assessed");
        assert_eq!(record.status_name(), "FAILING_UNRESPONSIVE");
        assert!(record.blocks_dependent_actions());
        // An explicit failed poll fails even faster.
        let mut monitor = AdapterHealthMonitor::new(limits()).expect("valid limits");
        monitor.observe(
            "simulated-test",
            BASE,
            AdapterObservation::Responsive { event_age_millis: 1_000 },
        );
        monitor.observe(
            "simulated-test",
            BASE + 1_000,
            AdapterObservation::PollFailed {
                reason: "the responsive check timed out".to_string(),
            },
        );
        let record = monitor.assess("simulated-test", BASE + 2_000).expect("assessed");
        assert_eq!(record.status_name(), "FAILING_UNRESPONSIVE");
        assert!(record.detail.contains("timed out"));
        // Recovery: a responsive observation clears it.
        monitor.observe(
            "simulated-test",
            BASE + 3_000,
            AdapterObservation::Responsive { event_age_millis: 2_000 },
        );
        assert_eq!(
            monitor.assess("simulated-test", BASE + 3_100).expect("assessed").status_name(),
            "HEALTHY"
        );
    }

    #[test]
    fn stale_data_is_degraded_never_healthy() {
        let mut monitor = AdapterHealthMonitor::new(limits()).expect("valid limits");
        monitor.observe(
            "simulated-test",
            BASE,
            AdapterObservation::Responsive { event_age_millis: 1_000 },
        );
        // Data event at BASE-1_000; at BASE+45_000 the age is 46 s — beyond
        // the 30 s freshness limit → DEGRADED_STALE_DATA (still responsive,
        // so not FAILING).
        let record = monitor.assess("simulated-test", BASE + 45_000).expect("assessed");
        assert_eq!(record.status_name(), "DEGRADED_STALE_DATA");
        assert_eq!(
            record.last_event_age_millis,
            Some(45_000 + 1_000)
        );
    }

    #[test]
    fn error_rate_over_the_bounded_window_is_degraded() {
        let mut monitor = AdapterHealthMonitor::new(limits()).expect("valid limits");
        monitor.observe(
            "simulated-test",
            BASE,
            AdapterObservation::Responsive { event_age_millis: 100 },
        );
        // Three failed polls inside the 5-minute window (threshold 3):
        // DEGRADED_ERROR_RATE. Responsive checks between failures keep
        // unresponsive clear.
        for tick in 1..=3i64 {
            monitor.observe(
                "simulated-test",
                BASE,
                AdapterObservation::Responsive { event_age_millis: 100 },
            );
            monitor.observe(
                "simulated-test",
                BASE + tick * 1_000,
                AdapterObservation::PollFailed { reason: format!("poll {tick} failed") },
            );
            monitor.observe(
                "simulated-test",
                BASE + tick * 1_000 + 100,
                AdapterObservation::Responsive { event_age_millis: 100 },
            );
        }
        let record = monitor.assess("simulated-test", BASE + 4_000).expect("assessed");
        assert_eq!(record.status_name(), "DEGRADED_ERROR_RATE");
    }

    #[test]
    fn a_policy_block_is_visible_in_health() {
        let mut monitor = AdapterHealthMonitor::new(limits()).expect("valid limits");
        monitor.observe(
            "simulated-test",
            BASE,
            AdapterObservation::Responsive { event_age_millis: 100 },
        );
        monitor.note_policy_block("simulated-test");
        let record = monitor.assess("simulated-test", BASE + 1_000).expect("assessed");
        assert_eq!(record.status_name(), "DEGRADED_POLICY_BLOCK");
        // Cleared → healthy again (fresh data, no errors).
        monitor.clear_policy_block("simulated-test");
        assert_eq!(
            monitor.assess("simulated-test", BASE + 1_000).expect("assessed").status_name(),
            "HEALTHY"
        );
    }

    #[test]
    fn credential_expiry_is_a_health_input_with_two_thresholds() {
        let mut monitor = AdapterHealthMonitor::new(limits()).expect("valid limits");
        monitor.observe(
            "simulated-test",
            BASE,
            AdapterObservation::Responsive { event_age_millis: 100 },
        );
        // Expires 30 days out: valid.
        monitor.set_credential_expiry("simulated-test", Some(BASE + 30 * 86_400_000));
        assert_eq!(
            monitor.assess("simulated-test", BASE + 1_000).expect("assessed").status_name(),
            "HEALTHY"
        );
        // Inside the 7-day rotation window: DEGRADED (visible at least one
        // rotation window before expiry — the P6.06 acceptance rule).
        monitor.set_credential_expiry("simulated-test", Some(BASE + 3 * 86_400_000));
        assert_eq!(
            monitor.assess("simulated-test", BASE + 1_000).expect("assessed").status_name(),
            "DEGRADED_CREDENTIAL_EXPIRY"
        );
        // Expired: FAILING (dependent actions blocked).
        monitor.set_credential_expiry("simulated-test", Some(BASE - 1));
        let record = monitor.assess("simulated-test", BASE + 1_000).expect("assessed");
        assert_eq!(record.status_name(), "FAILING_CREDENTIAL_EXPIRY");
        assert!(record.blocks_dependent_actions());
    }

    #[test]
    fn a_failing_adapter_blocks_ingest_and_command_targeting_with_auditable_reasons() {
        let mut monitor = AdapterHealthMonitor::new(limits()).expect("valid limits");
        monitor.observe(
            "simulated-test",
            BASE,
            AdapterObservation::Responsive { event_age_millis: 100 },
        );
        // Healthy: both allowed.
        assert!(monitor
            .check_action_allowed("simulated-test", BlockedAction::Ingest, BASE + 1_000)
            .is_ok());
        assert!(monitor
            .check_action_allowed("simulated-test", BlockedAction::Command, BASE + 1_000)
            .is_ok());
        // FAILING: both blocked, each with a reason naming the action and
        // the status (the caller audits it).
        monitor.observe(
            "simulated-test",
            BASE + 2_000,
            AdapterObservation::PollFailed { reason: "device unreachable".to_string() },
        );
        for action in [BlockedAction::Ingest, BlockedAction::Command] {
            let reason = monitor
                .check_action_allowed("simulated-test", action, BASE + 3_000)
                .expect_err("blocked while FAILING");
            assert!(reason.contains(action.as_str()));
            assert!(reason.contains("FAILING_UNRESPONSIVE"));
        }
    }

    #[test]
    fn the_snapshot_renders_the_closed_vocabulary_and_is_ordered() {
        let mut monitor = AdapterHealthMonitor::new(limits()).expect("valid limits");
        monitor.observe(
            "aaa-adapter",
            BASE,
            AdapterObservation::Responsive { event_age_millis: 100 },
        );
        monitor.observe(
            "bbb-adapter",
            BASE,
            AdapterObservation::Responsive { event_age_millis: 100 },
        );
        monitor.observe(
            "bbb-adapter",
            BASE + 500,
            AdapterObservation::PollFailed { reason: "down".to_string() },
        );
        let snapshot = monitor.snapshot(BASE + 1_000);
        let names: Vec<String> = snapshot.adapters.iter().map(|r| r.status_name()).collect();
        assert_eq!(
            names,
            vec!["HEALTHY".to_string(), "FAILING_UNRESPONSIVE".to_string()],
            "snapshot is ordered by adapter id with the closed vocabulary"
        );
        // The JSON rendering carries the statuses and the closed names only.
        let rendered = snapshot.json();
        assert!(rendered.contains("\"status\":\"HEALTHY\""));
        assert!(rendered.contains("\"status\":\"FAILING_UNRESPONSIVE\""));
        assert!(rendered.contains("\"adapter_id\":\"aaa-adapter\""));
    }

    #[test]
    fn nonsense_limits_are_composition_errors() {
        let mut bad = limits();
        bad.freshness_limit_millis = 0;
        assert!(matches!(
            AdapterHealthMonitor::new(bad),
            Err(HealthError::InvalidLimits { .. })
        ));
        let mut bad = limits();
        bad.max_errors_in_window = 0;
        assert!(matches!(
            AdapterHealthMonitor::new(bad),
            Err(HealthError::InvalidLimits { .. })
        ));
    }
}
