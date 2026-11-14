// P6.17 — The site-local policy engine (report §18.1, §18.3, §21.3; ledger
//         P6.17 step 1).
//
// The edge enforces site policy LOCALLY — a disconnected edge still enforces
// something (the ledger's risk row: "policy enforced only centrally, so a
// disconnected edge enforces nothing" is exactly what this module exists to
// prevent). The policy surface, per the ledger:
//
// - which adapters are ENABLED (no entry = refused — fail-closed);
// - per-adapter COMMAND RATE LIMITS (max commands per window);
// - per-adapter ALLOWED OPERATING WINDOWS (UTC millis-of-day);
// - violation behaviour = REJECT + AUDIT, never silently proceed. The
//   engine returns named [`PolicyViolation`]s; it never logs-and-continues,
//   never degrades to a default allow (the caller — the P6.14
//   [`crate::command_path::CommandReceiver`] through the
//   `CommandPolicyGate` seam — rejects and audits).
//
// Configuration: a VERSIONED, line-oriented site-policy configuration
// ([`SitePolicyConfig::parse`], format `flexcore-site-policy-v1` — the
// credential-store/enablement-record pattern). Parsing is STRICT: an
// unknown field, a wrong version, a duplicate adapter, or a missing
// required field is a named [`PolicyError`] — unknown fields are never
// silently accepted (the P6.13 step-1 discipline applied to the gate's own
// configuration). The in-code struct is the deployment format's code form;
// the deployment config format is documented in the decision doc (§5).
//
// Gate re-check wiring (P6.14 ↔ P6.17, no cycle): the command path defines
// the `CommandPolicyGate` port; THIS module implements it for
// [`SitePolicyEngine`] — the dependency is one-directional (policy →
// command_path), and the composition root hands the engine to the receiver
// as `Arc<dyn CommandPolicyGate>`. The receiver cannot even be constructed
// without one.
//
// Non-goals (the supervisory boundary, AGENTS.md rule 3): this module does
// not perform device control, protection, or fast safety behaviour — it
// gates the TEST-ENV command path and records enablement for health
// (P6.17). Rate limits here are SUPERVISORY admission limits, not a
// real-time control mechanism.

use std::collections::HashMap;
use std::sync::Mutex;

use crate::command_path::{CommandPolicyGate, CommandPolicyRefusal};

// --- Configuration ------------------------------------------------------------

/// The versioned site-policy configuration format id.
pub const SITE_POLICY_FORMAT: &str = "flexcore-site-policy-v1";
/// The configuration format version this parser accepts.
pub const SITE_POLICY_FORMAT_VERSION: u32 = 1;

/// A per-adapter command rate limit: at most `max_commands` admitted
/// commands in any `window_millis` window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RateLimit {
    pub max_commands: u32,
    pub window_millis: i64,
}

/// The allowed operating window, in UTC milliseconds-of-day
/// (`start <= t < end`; both in `0..86_400_000`; the window does not wrap —
/// a wrap is a validation error, recorded, never interpreted).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OperatingWindow {
    pub start_millis_of_day: i64,
    pub end_millis_of_day: i64,
}

pub const MILLIS_PER_DAY: i64 = 86_400_000;

/// The policy for ONE adapter. Every field is REQUIRED (the no-defaults
/// rule): an adapter with no policy entry is UNKNOWN and refused, never
/// defaulted to enabled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdapterPolicy {
    pub adapter_id: String,
    pub enabled: bool,
    pub rate_limit: RateLimit,
    pub operating_window: OperatingWindow,
}

/// The versioned site-policy configuration (in-code form of the
/// `flexcore-site-policy-v1` document).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SitePolicyConfig {
    /// Must equal [`SITE_POLICY_FORMAT_VERSION`] (versioned so a future
    /// format change is refuse-able, never silently mis-parsed).
    pub format_version: u32,
    /// One entry per adapter (duplicate ids are a validation error).
    pub adapters: Vec<AdapterPolicy>,
}

// --- Errors ---------------------------------------------------------------------

/// Named errors of the policy configuration and parsing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyError {
    /// The configuration text was malformed (wrong version line, unknown
    /// field, bad value, missing required field).
    Malformed { line: usize, reason: String },
    /// The configuration had no adapter entries (a policy that enables
    /// nothing is probably a mistake — refuse at load, never silently
    /// enforce nothing).
    NoAdapters,
    /// The same adapter id appeared twice.
    DuplicateAdapter { adapter_id: String },
    /// A policy field was out of range (zero/negative rate-limit values, a
    /// wrapped or empty operating window, an empty adapter id).
    InvalidPolicy { reason: String },
}

impl std::fmt::Display for PolicyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PolicyError::Malformed { line, reason } => {
                write!(f, "site policy line {line} is malformed: {reason}")
            }
            PolicyError::NoAdapters => {
                write!(
                    f,
                    "the site policy has no adapter entries; refusing to load a policy that \
                     enforces nothing"
                )
            }
            PolicyError::DuplicateAdapter { adapter_id } => {
                write!(f, "the site policy declares adapter '{adapter_id}' twice")
            }
            PolicyError::InvalidPolicy { reason } => {
                write!(f, "the site policy is invalid: {reason}")
            }
        }
    }
}

impl std::error::Error for PolicyError {}

impl SitePolicyConfig {
    /// Validates the configuration (fail-closed at load).
    pub fn validate(&self) -> Result<(), PolicyError> {
        if self.format_version != SITE_POLICY_FORMAT_VERSION {
            return Err(PolicyError::InvalidPolicy {
                reason: format!(
                    "format version {} is not the supported {}",
                    self.format_version, SITE_POLICY_FORMAT_VERSION
                ),
            });
        }
        if self.adapters.is_empty() {
            return Err(PolicyError::NoAdapters);
        }
        let mut seen: Vec<&str> = Vec::new();
        for adapter in &self.adapters {
            if adapter.adapter_id.trim().is_empty() {
                return Err(PolicyError::InvalidPolicy {
                    reason: "an adapter entry has an empty adapter_id".to_string(),
                });
            }
            if seen.contains(&adapter.adapter_id.as_str()) {
                return Err(PolicyError::DuplicateAdapter {
                    adapter_id: adapter.adapter_id.clone(),
                });
            }
            seen.push(&adapter.adapter_id);
            if adapter.rate_limit.max_commands == 0 {
                return Err(PolicyError::InvalidPolicy {
                    reason: format!(
                        "adapter '{}': rate limit max_commands must be positive",
                        adapter.adapter_id
                    ),
                });
            }
            if adapter.rate_limit.window_millis <= 0 {
                return Err(PolicyError::InvalidPolicy {
                    reason: format!(
                        "adapter '{}': rate limit window_millis must be positive",
                        adapter.adapter_id
                    ),
                });
            }
            let window = adapter.operating_window;
            if window.start_millis_of_day < 0
                || window.end_millis_of_day <= 0
                || window.start_millis_of_day >= MILLIS_PER_DAY
                || window.end_millis_of_day > MILLIS_PER_DAY
                || window.start_millis_of_day >= window.end_millis_of_day
            {
                return Err(PolicyError::InvalidPolicy {
                    reason: format!(
                        "adapter '{}': operating window {}..{} is invalid (must be \
                         0 <= start < end <= {MILLIS_PER_DAY}; no wrap)",
                        adapter.adapter_id,
                        window.start_millis_of_day,
                        window.end_millis_of_day
                    ),
                });
            }
        }
        Ok(())
    }

    /// Parses the versioned, line-oriented site-policy document. STRICT:
    /// unknown fields and wrong versions are named errors (never silently
    /// accepted). Format (fields separated by ` = `; `adapter` starts a
    /// block; `enabled`, `rate_limit`, `operating_window` apply to the
    /// current block; every field of every block is REQUIRED):
    ///
    /// ```text
    /// flexcore-site-policy-v1
    /// adapter = simulated-test
    /// enabled = true
    /// rate_limit = 5/60000
    /// operating_window = 0800-1800
    /// adapter = openems-rest
    /// enabled = false
    /// rate_limit = 2/60000
    /// operating_window = 0900-1700
    /// ```
    pub fn parse(text: &str) -> Result<Self, PolicyError> {
        let malformed = |line: usize, reason: String| PolicyError::Malformed { line, reason };
        let mut lines = text.lines();
        let version_line = lines
            .next()
            .ok_or_else(|| malformed(1, "the policy document is empty".to_string()))?
            .trim();
        if version_line != SITE_POLICY_FORMAT {
            return Err(malformed(
                1,
                format!(
                    "policy format '{version_line}' is not the supported '{SITE_POLICY_FORMAT}'"
                ),
            ));
        }
        let mut adapters: Vec<AdapterPolicy> = Vec::new();
        let mut current: Option<AdapterPolicy> = None;
        for (index, line) in lines.enumerate() {
            let line_no = index + 2; // the version line was line 1
            if line.trim().is_empty() {
                continue;
            }
            let (key, value) = line
                .split_once(" = ")
                .ok_or_else(|| malformed(line_no, "expected 'key = value'".to_string()))?;
            match key.trim() {
                "adapter" => {
                    if let Some(finished) = current.take() {
                        adapters.push(finished);
                    }
                    current = Some(AdapterPolicy {
                        adapter_id: value.trim().to_string(),
                        enabled: false,
                        rate_limit: RateLimit {
                            max_commands: 0,
                            window_millis: 0,
                        },
                        operating_window: OperatingWindow {
                            start_millis_of_day: 0,
                            end_millis_of_day: 0,
                        },
                    });
                }
                "enabled" => {
                    let entry = current.as_mut().ok_or_else(|| {
                        malformed(line_no, "'enabled' appears before any 'adapter' line".to_string())
                    })?;
                    entry.enabled = match value.trim() {
                        "true" => true,
                        "false" => false,
                        other => {
                            return Err(malformed(
                                line_no,
                                format!("'enabled' expects true|false, got '{other}'"),
                            ))
                        }
                    };
                }
                "rate_limit" => {
                    let entry = current.as_mut().ok_or_else(|| {
                        malformed(
                            line_no,
                            "'rate_limit' appears before any 'adapter' line".to_string(),
                        )
                    })?;
                    let (max, window) = value
                        .trim()
                        .split_once('/')
                        .ok_or_else(|| {
                            malformed(
                                line_no,
                                "'rate_limit' expects '<max>/<window_millis>'".to_string(),
                            )
                        })?;
                    entry.rate_limit = RateLimit {
                        max_commands: max.trim().parse::<u32>().map_err(|_| {
                            malformed(line_no, "rate limit max is not a u32".to_string())
                        })?,
                        window_millis: window.trim().parse::<i64>().map_err(|_| {
                            malformed(line_no, "rate limit window is not an integer".to_string())
                        })?,
                    };
                }
                "operating_window" => {
                    let entry = current.as_mut().ok_or_else(|| {
                        malformed(
                            line_no,
                            "'operating_window' appears before any 'adapter' line".to_string(),
                        )
                    })?;
                    let (start, end) = value.trim().split_once('-').ok_or_else(|| {
                        malformed(
                            line_no,
                            "'operating_window' expects 'HHMM-HHMM' (UTC)".to_string(),
                        )
                    })?;
                    let parse_hhmm = |raw: &str| -> Result<i64, PolicyError> {
                        if raw.len() != 4 || !raw.chars().all(|c| c.is_ascii_digit()) {
                            return Err(malformed(
                                line_no,
                                format!("'{raw}' is not HHMM"),
                            ));
                        }
                        let hours: i64 = raw[..2].parse::<u32>().map_err(|_| {
                            malformed(line_no, "window hours are not numeric".to_string())
                        })? as i64;
                        let minutes: i64 = raw[2..].parse::<u32>().map_err(|_| {
                            malformed(line_no, "window minutes are not numeric".to_string())
                        })? as i64;
                        if hours > 24 || minutes > 59 {
                            return Err(malformed(
                                line_no,
                                format!("'{raw}' is not a valid HHMM time".to_string()),
                            ));
                        }
                        Ok(hours * 3_600_000 + minutes * 60_000)
                    };
                    entry.operating_window = OperatingWindow {
                        start_millis_of_day: parse_hhmm(start.trim())?,
                        end_millis_of_day: parse_hhmm(end.trim())?,
                    };
                }
                other => {
                    return Err(malformed(
                        line_no,
                        format!(
                            "unknown field '{other}' (unknown fields are never silently accepted)"
                        ),
                    ));
                }
            }
        }
        if let Some(finished) = current.take() {
            adapters.push(finished);
        }
        let config = Self {
            format_version: SITE_POLICY_FORMAT_VERSION,
            adapters,
        };
        config.validate()?;
        Ok(config)
    }
}

// --- Violations -------------------------------------------------------------------

/// A named policy violation. Rejection + audit is the ONLY handling; the
/// engine never proceeds past a violation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyViolation {
    /// The adapter has no policy entry (fail-closed: unknown = refused).
    UnknownAdapter { adapter_id: String },
    /// The adapter's policy entry disables it.
    AdapterDisabled { adapter_id: String },
    /// The per-adapter rate limit was exceeded in the window.
    RateLimitExceeded {
        adapter_id: String,
        max_commands: u32,
        window_millis: i64,
    },
    /// The current UTC time-of-day is outside the adapter's operating
    /// window.
    OutsideOperatingWindow {
        adapter_id: String,
        start_millis_of_day: i64,
        end_millis_of_day: i64,
    },
}

impl PolicyViolation {
    /// The stable violation code (audit text).
    pub fn code(&self) -> &'static str {
        match self {
            PolicyViolation::UnknownAdapter { .. } => "policy_unknown_adapter",
            PolicyViolation::AdapterDisabled { .. } => "policy_adapter_disabled",
            PolicyViolation::RateLimitExceeded { .. } => "policy_rate_limit_exceeded",
            PolicyViolation::OutsideOperatingWindow { .. } => "policy_outside_operating_window",
        }
    }
}

impl std::fmt::Display for PolicyViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PolicyViolation::UnknownAdapter { adapter_id } => {
                write!(
                    f,
                    "adapter '{adapter_id}' has no site-policy entry; unknown adapters are \
                     refused, never defaulted"
                )
            }
            PolicyViolation::AdapterDisabled { adapter_id } => {
                write!(f, "adapter '{adapter_id}' is disabled by the site policy")
            }
            PolicyViolation::RateLimitExceeded {
                adapter_id,
                max_commands,
                window_millis,
            } => write!(
                f,
                "adapter '{adapter_id}' exceeded its command rate limit ({max_commands} per \
                 {window_millis} ms)"
            ),
            PolicyViolation::OutsideOperatingWindow {
                adapter_id,
                start_millis_of_day,
                end_millis_of_day,
            } => write!(
                f,
                "adapter '{adapter_id}' is outside its operating window ({}..{} UTC \
                 millis-of-day)",
                start_millis_of_day, end_millis_of_day
            ),
        }
    }
}

// --- The engine ---------------------------------------------------------------------

/// The per-adapter accepted-command timestamps (the rate-limit state).
#[derive(Debug, Default)]
struct RateWindows {
    accepted_at: HashMap<String, Vec<i64>>,
}

/// The site-local policy engine. Implements the command path's
/// [`CommandPolicyGate`] port (the P6.14 re-check seam) and exposes the
/// read-only surface health (P6.17) and the console (P6.25) consume.
///
/// The rate-limit state is behind a mutex so the engine composes as
/// `Arc<dyn CommandPolicyGate>` (the receiver's seam is `&self`). Lock
/// poisoning is a loud failure (the house `expect("poisoned …")` rule).
pub struct SitePolicyEngine {
    config: SitePolicyConfig,
    /// Index: adapter id → position in `config.adapters` (built once).
    index: HashMap<String, usize>,
    rate_windows: Mutex<RateWindows>,
}

impl SitePolicyEngine {
    /// Loads and validates the engine over a configuration.
    pub fn new(config: SitePolicyConfig) -> Result<Self, PolicyError> {
        config.validate()?;
        let mut index = HashMap::new();
        for (position, adapter) in config.adapters.iter().enumerate() {
            index.insert(adapter.adapter_id.clone(), position);
        }
        Ok(Self {
            config,
            index,
            rate_windows: Mutex::new(RateWindows::default()),
        })
    }

    /// The policy for an adapter, if one is declared.
    pub fn policy_for(&self, adapter_id: &str) -> Option<&AdapterPolicy> {
        self.index
            .get(adapter_id)
            .map(|&position| &self.config.adapters[position])
    }

    /// Whether the adapter is declared AND enabled (the read-only surface
    /// health and the console consume).
    pub fn is_adapter_enabled(&self, adapter_id: &str) -> bool {
        self.policy_for(adapter_id)
            .map(|policy| policy.enabled)
            .unwrap_or(false)
    }

    /// The declared adapter ids (diagnostics).
    pub fn adapter_ids(&self) -> Vec<&str> {
        self.config
            .adapters
            .iter()
            .map(|a| a.adapter_id.as_str())
            .collect()
    }

    /// Checks one command against the policy and, when admitted, records it
    /// against the rate limit. On any violation the state is UNCHANGED (a
    /// refused command does not consume budget) and the violation is
    /// returned — the caller rejects and audits, never silently proceeds.
    pub fn admit_command(
        &self,
        adapter_id: &str,
        now_unix_millis: i64,
    ) -> Result<(), PolicyViolation> {
        let Some(policy) = self.policy_for(adapter_id) else {
            return Err(PolicyViolation::UnknownAdapter {
                adapter_id: adapter_id.to_string(),
            });
        };
        if !policy.enabled {
            return Err(PolicyViolation::AdapterDisabled {
                adapter_id: adapter_id.to_string(),
            });
        }
        let millis_of_day = now_unix_millis.rem_euclid(MILLIS_PER_DAY);
        if millis_of_day < policy.operating_window.start_millis_of_day
            || millis_of_day >= policy.operating_window.end_millis_of_day
        {
            return Err(PolicyViolation::OutsideOperatingWindow {
                adapter_id: adapter_id.to_string(),
                start_millis_of_day: policy.operating_window.start_millis_of_day,
                end_millis_of_day: policy.operating_window.end_millis_of_day,
            });
        }
        let mut windows = self.rate_windows.lock().expect("poisoned policy mutex");
        let window = &mut windows.accepted_at.entry(adapter_id.to_string()).or_default();
        let window_start = now_unix_millis - policy.rate_limit.window_millis;
        window.retain(|&at| at > window_start);
        if window.len() as u32 >= policy.rate_limit.max_commands {
            return Err(PolicyViolation::RateLimitExceeded {
                adapter_id: adapter_id.to_string(),
                max_commands: policy.rate_limit.max_commands,
                window_millis: policy.rate_limit.window_millis,
            });
        }
        window.push(now_unix_millis);
        Ok(())
    }

    /// How many commands this engine has admitted for the adapter inside
    /// the current rate-limit window (health/diagnostics visibility).
    pub fn admitted_in_window(&self, adapter_id: &str, now_unix_millis: i64) -> usize {
        let Some(policy) = self.policy_for(adapter_id) else {
            return 0;
        };
        let windows = self.rate_windows.lock().expect("poisoned policy mutex");
        windows
            .accepted_at
            .get(adapter_id)
            .map(|timestamps| {
                let window_start = now_unix_millis - policy.rate_limit.window_millis;
                timestamps
                    .iter()
                    .filter(|&&at| at > window_start && at <= now_unix_millis)
                    .count()
            })
            .unwrap_or(0)
    }
}

impl CommandPolicyGate for SitePolicyEngine {
    /// The P6.14 re-check seam: the command path consults THIS engine as an
    /// additional, independent gate (rate limit, operating window, adapter
    /// enablement). A violation maps onto the receiver's
    /// [`CommandPolicyRefusal`] with its stable code and human-safe reason.
    fn check_command(
        &self,
        target_adapter: &str,
        now_unix_millis: i64,
    ) -> Result<(), CommandPolicyRefusal> {
        self.admit_command(target_adapter, now_unix_millis).map_err(|violation| {
            CommandPolicyRefusal {
                code: violation.code().to_string(),
                reason: violation.to_string(),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy_text() -> String {
        format!(
            "{SITE_POLICY_FORMAT}\nadapter = simulated-test\nenabled = true\n\
             rate_limit = 2/60000\noperating_window = 0000-2400\n"
        )
    }

    #[test]
    fn the_versioned_document_parses_strictly() {
        let config = SitePolicyConfig::parse(&policy_text()).expect("parses");
        assert_eq!(config.format_version, 1);
        assert_eq!(config.adapters.len(), 1);
        assert_eq!(config.adapters[0].adapter_id, "simulated-test");
        assert!(config.adapters[0].enabled);
        assert_eq!(config.adapters[0].rate_limit.max_commands, 2);
        assert_eq!(config.adapters[0].rate_limit.window_millis, 60_000);
        assert_eq!(config.adapters[0].operating_window.start_millis_of_day, 0);
        assert_eq!(config.adapters[0].operating_window.end_millis_of_day, MILLIS_PER_DAY);
    }

    #[test]
    fn unknown_fields_wrong_versions_and_bad_values_are_named_errors() {
        // Unknown field.
        let smuggled = format!("{}\nextra_field = sneaky\n", SITE_POLICY_FORMAT);
        assert!(matches!(
            SitePolicyConfig::parse(&smuggled),
            Err(PolicyError::Malformed { .. })
        ));
        // Wrong version line.
        let wrong = "flexcore-site-policy-v0\nadapter = a\n".to_string();
        assert!(matches!(
            SitePolicyConfig::parse(&wrong),
            Err(PolicyError::Malformed { line: 1, .. })
        ));
        // Field before any adapter block.
        let orphaned = format!("{SITE_POLICY_FORMAT}\nenabled = true\n");
        assert!(matches!(
            SitePolicyConfig::parse(&orphaned),
            Err(PolicyError::Malformed { .. })
        ));
        // Wrapped/empty operating window is a validation error.
        let mut config = SitePolicyConfig::parse(&policy_text()).expect("parses");
        config.adapters[0].operating_window = OperatingWindow {
            start_millis_of_day: 3_600_000,
            end_millis_of_day: 3_600_000,
        };
        assert!(matches!(
            SitePolicyConfig::validate(&config),
            Err(PolicyError::InvalidPolicy { .. })
        ));
    }

    #[test]
    fn rate_limit_refuses_the_third_command_and_a_refusal_consumes_no_budget() {
        let config = SitePolicyConfig::parse(&policy_text()).expect("parses");
        let engine = SitePolicyEngine::new(config).expect("loads");
        let now = 1_700_000_000_000;
        assert!(engine.admit_command("simulated-test", now).is_ok());
        assert!(engine.admit_command("simulated-test", now + 1).is_ok());
        // Third within the window: refused (limit 2), and the refusal does
        // not consume budget (state unchanged).
        assert!(matches!(
            engine.admit_command("simulated-test", now + 2),
            Err(PolicyViolation::RateLimitExceeded { .. })
        ));
        assert_eq!(engine.admitted_in_window("simulated-test", now + 2), 2);
        // After the window passes, budget is available again.
        assert!(engine
            .admit_command("simulated-test", now + 60_001)
            .is_ok());
    }

    #[test]
    fn unknown_disabled_and_out_of_window_adapters_refuse() {
        let config = SitePolicyConfig::parse(&policy_text()).expect("parses");
        let engine = SitePolicyEngine::new(config).expect("loads");
        let noon = 1_700_000_000_000; // within the 0000-2400 window
        // Unknown adapter: refused, never defaulted.
        assert!(matches!(
            engine.admit_command("ghost-adapter", noon),
            Err(PolicyViolation::UnknownAdapter { .. })
        ));
        // Disabled adapter.
        let mut config = SitePolicyConfig::parse(&policy_text()).expect("parses");
        config.adapters[0].enabled = false;
        let engine = SitePolicyEngine::new(config).expect("loads");
        assert!(matches!(
            engine.admit_command("simulated-test", noon),
            Err(PolicyViolation::AdapterDisabled { .. })
        ));
        // Outside the operating window (window 0900-1700 UTC; the probe is
        // 03:00 UTC).
        let config = SitePolicyConfig::parse(&format!(
            "{SITE_POLICY_FORMAT}\nadapter = simulated-test\nenabled = true\n\
             rate_limit = 5/60000\noperating_window = 0900-1700\n"
        ))
        .expect("parses");
        let engine = SitePolicyEngine::new(config).expect("loads");
        let three_am_utc = 1_700_000_000_000 - (1_700_000_000_000 % MILLIS_PER_DAY) + 3 * 3_600_000;
        assert!(matches!(
            engine.admit_command("simulated-test", three_am_utc),
            Err(PolicyViolation::OutsideOperatingWindow { .. })
        ));
    }

    #[test]
    fn the_engine_implements_the_command_policy_gate_seam() {
        let config = SitePolicyConfig::parse(&policy_text()).expect("parses");
        let engine = SitePolicyEngine::new(config).expect("loads");
        let now = 1_700_000_000_000;
        // Admitted twice, refused third — through the seam the receiver
        // calls.
        let gate: &dyn CommandPolicyGate = &engine;
        assert!(gate.check_command("simulated-test", now).is_ok());
        assert!(gate.check_command("simulated-test", now + 1).is_ok());
        let refusal = gate
            .check_command("simulated-test", now + 2)
            .expect_err("rate limit exceeded");
        assert_eq!(refusal.code, "policy_rate_limit_exceeded");
        assert!(refusal.reason.contains("rate limit"));
    }

    #[test]
    fn a_policy_with_no_adapters_refuses_to_load() {
        let config = SitePolicyConfig {
            format_version: 1,
            adapters: Vec::new(),
        };
        assert!(matches!(
            SitePolicyEngine::new(config),
            Err(PolicyError::NoAdapters)
        ));
    }
}
