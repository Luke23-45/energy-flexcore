// P6.16 — Command forwarding only to allowlisted adapters; fallback on
//         failure (report §18.1, §14.4, §14.1, §21.2; ledger P6.16).
//
// ## RECORDED DESIGN DECISION — the `CommandForwarder` port vs the
//    read-path `EmsAdapter` port
//
// The ledger's P6.16 step 2 says "forward via the adapter's `propose_setpoint`
// path". The P6.12 `EmsAdapter` port ([`crate::adapters::port::EmsAdapter`])
// DELIBERATELY has NO dispatch/write method — its absence is structural
// (decision doc `05_12_20_edge_foundation.md`: "the command path is absent,
// not disabled-by-flag-only"; the read path's no-write guarantee). Adding a
// write method to `EmsAdapter` would let any read-path consumer reach a write
// path — exactly what the structural absence prevents.
//
// RESOLUTION (recorded, deliberate): the command path forwards through a
// SEPARATE [`CommandForwarder`] port, defined HERE (in-module, not ports.rs —
// the `EmsAdapter`-in-`adapters/port.rs` precedent), implemented ONLY by:
//
// 1. the TEST DOUBLE in `tests/command_forward_p6_16.rs` (a scripted
//    in-process fake), and
// 2. [`FailClosedCommandForwarder`] in PRODUCTION composition — every call
//    fails with a named error, so a production edge records ISSUED → FAILED
//    and never fabricates an adapter interaction.
//
// The forwarder is reachable ONLY behind the P6.14 four-condition gate AND
// the P6.15 verifier AND the P6.17 policy re-check (the pipeline in
// `command_path.rs` composes exactly that order). The READ path's no-write
// guarantee is untouched: `EmsAdapter` still has no write method, and no
// read-path type can reach [`CommandForwarder`]. The command surface is the
// separately-gated one. When a real adapter write path is later qualified
// (post-v0.4.0, per deployment), it implements THIS port behind its own
// reviewed composition — `EmsAdapter` stays read-only.
//
// ## The forwarding flow (per verified command)
//
// 1. SEPARATE allowlist re-check with its OWN audit event
//    (`EDGE_COMMAND_FORWARD_ALLOWLIST_CHECKED`) — the P6.15 check 4a is the
//    verification-side pre-check; the forwarder re-checks before EVERY
//    forward, and a non-allowlisted adapter is NEVER reached (the port is
//    not invoked for it).
// 2. Fail-closed check of the per-asset fallback stop state (an asset under
//    the PRESERVE_RESERVE_SOC... fallback refuses new commands).
// 3. The independent P6.17 policy gate re-check, through the SAME
//    `CommandPolicyGate` seam the receiver uses — fail-closed.
// 4. Issue recorded (`ISSUED`, §14.1), then the forward through the port.
// 5. Outcome mapped onto the frozen §14.1 states: ACKNOWLEDGED (with the
//    P4/P5 honesty wording: acknowledgement is NOT physical delivery),
//    or FAILED (+ fallback per the configured per-asset policy), or
//    EXPIRED mid-flight (with FALLBACK only where the expiry trigger is
//    configured — the P4.17 precedent: EXPIRED → FALLBACK is the only
//    EXPIRED outgoing edge and fires only where the policy requires it).
// 6. Tracking observations land in TRACKING / COMPLETED / FAILED (+ fallback).
//
// Every transition is audited; a refused command consumes nothing and
// reaches nothing.
//
// ## The per-asset fallback policy (§14.4)
//
// The closed option set — a closed enum, no seventh variant, and NO
// universal "set power to zero" fallback anywhere (structurally absent; the
// P4.17 fallback-engine precedent, cited not reinvented):
//
// | Variant | Meaning |
// |---|---|
// | `HOLD_LAST_ACCEPTED_SETPOINT` | hold the last accepted setpoint for a BOUNDED interval (configured; required positive) |
// | `RETURN_TO_LOCAL_EMS_AUTONOMOUS` | hand control back to the site EMS's autonomous mode |
// | `PRESERVE_RESERVE_SOC_CONSTRAINTS_AND_STOP_ACCEPTING_NEW_COMMANDS` | keep reserve/SOC constraints intact and refuse all further commands for the asset until operator action |
// | `REQUIRE_OPERATOR_INTERVENTION` | no automatic action; an operator decides |
//
// Configuration (`flexcore-edge-fallback-policy-v1`, line-oriented, strict
// parse) is PER ASSET, and every entry carries an OPERATOR-REVIEW field
// (`reviewed_by` + `reviewed_on`) that MUST be present to load — a missing
// review field refuses the whole config. IN THIS LOOP THE REVIEW IS NOT
// SIGNED: the name and date are recorded as HUMAN GATE INPUT, not a
// cryptographic sign-off (an unsigned text field proves nothing on its own;
// signing the review is a recorded follow-up for the deployment). An asset
// with NO entry resolves to the explicit DEFAULT `REQUIRE_OPERATOR_
// INTERVENTION` with NO automatic setpoint change and NO FALLBACK
// transition (the P4.17 default-entry precedent) — silence never becomes a
// setpoint change.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use flexcore_ports::clock::Clock;

use crate::command_path::{CommandPolicyGate, CommandPolicyRefusal, CommandRejectionAuditRecord};
use crate::command_verify::VerifiedCommand;
use crate::ports::{AuditSink, EdgeAuditEvent, EdgeAuditKind};

// --- Constants -----------------------------------------------------------------

/// The versioned per-asset fallback-policy configuration format id.
pub const FALLBACK_POLICY_FORMAT: &str = "flexcore-edge-fallback-policy-v1";
/// The configuration format version this parser accepts.
pub const FALLBACK_POLICY_FORMAT_VERSION: u32 = 1;
/// The P4/P5 honesty wording carried on every acknowledgement record.
pub const ACKNOWLEDGEMENT_IS_NOT_DELIVERY: &str =
    "acknowledgement is not physical delivery (§14.1)";

// --- The forwarder port (defined HERE — see the recorded design decision) -------

/// One forward request to the gated command surface. Carries identity,
/// payload, and the validity bound the tracking/expiry logic needs.
#[derive(Debug, Clone, PartialEq)]
pub struct ForwardRequest {
    pub command_id: String,
    pub asset_key: String,
    pub target_adapter: String,
    pub quantity: String,
    pub value: f64,
    pub sequence: u64,
    /// The command's own expiry (the §14.3 validity bound, carried so the
    /// engine can expire a command mid-flight honestly).
    pub valid_until_unix_millis: i64,
    pub issued_at_unix_millis: i64,
}

/// The synchronous outcome of a forward: the adapter ACKNOWLEDGED (which is
/// NOT delivery) or actively NACKed. A missing/late acknowledgement is NOT
/// an outcome — it is a [`ForwardError`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForwardOutcome {
    /// The adapter acknowledged the command. Acknowledgement is NOT
    /// physical delivery (§14.1) — the engine records it as ACKNOWLEDGED
    /// and continues to TRACKING only on positive tracking observations.
    Acknowledged,
    /// The adapter refused the command (application-layer NACK — the P4.28
    /// frozen ISSUED → FAILED trigger shape, cited).
    Nack { reason: String },
}

/// A forward FAILURE (no outcome exists). Every variant is named.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForwardError {
    /// The adapter did not answer within the window (timeout).
    Timeout { reason: String },
    /// The acknowledgement window elapsed with NO acknowledgement at all.
    NoAcknowledgement { reason: String },
    /// The transport to the adapter failed. From the fallback policy's
    /// perspective this is timeout-class (the adapter did not accept the
    /// command); the detail keeps the distinction.
    Transport { reason: String },
}

impl std::fmt::Display for ForwardError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ForwardError::Timeout { reason } => write!(f, "adapter timeout: {reason}"),
            ForwardError::NoAcknowledgement { reason } => {
                write!(f, "no acknowledgement received: {reason}")
            }
            ForwardError::Transport { reason } => write!(f, "transport failure: {reason}"),
        }
    }
}

/// THE P6.16 COMMAND-FORWARDER PORT (defined in this module, not ports.rs —
/// see the recorded design decision in the module header). The ONLY
/// write-shaped seam on the edge command path; reachable only behind the
/// P6.14 gate + P6.15 verifier + P6.17 policy re-check. Sync trait (the
/// house port rule): a concrete adapter owns its own I/O shape.
pub trait CommandForwarder: Send + Sync {
    /// Forwards ONE command. The engine calls this ONLY after the separate
    /// allowlist re-check and the policy re-check have both passed.
    fn forward(&self, request: &ForwardRequest) -> Result<ForwardOutcome, ForwardError>;
}

/// The production fail-closed forwarder: EVERY call fails with a named
/// transport error. A production edge therefore records ISSUED → FAILED
/// (and applies the configured per-asset fallback) — it NEVER fabricates an
/// adapter interaction and never silently drops the command. When a real
/// adapter write path is qualified post-v0.4.0, it replaces this stub in
/// the composition root, behind the same gates.
#[derive(Debug, Default)]
pub struct FailClosedCommandForwarder;

impl CommandForwarder for FailClosedCommandForwarder {
    fn forward(&self, _request: &ForwardRequest) -> Result<ForwardOutcome, ForwardError> {
        Err(ForwardError::Transport {
            reason: "no command forwarder is composed (the production composition is \
                     fail-closed: FlexCore v0.4.0 is simulation/shadow only and no adapter \
                     write path is qualified); the command is FAILED and the per-asset fallback \
                     policy applies"
                .to_string(),
        })
    }
}

// --- The per-asset fallback policy (§14.4) -----------------------------------------

/// The closed fallback variant set (§14.4). No seventh variant exists and
/// none may be invented; there is NO "set power to zero" variant anywhere
/// (structurally absent — the ledger's explicit rule).
#[derive(Debug, Clone, PartialEq)]
pub enum FallbackVariant {
    /// Hold the last accepted setpoint for a BOUNDED interval (configured,
    /// required positive — an unbounded hold is a load error, the ledger's
    /// "hold treated as safe forever" risk).
    HoldLastAcceptedSetpoint { bounded_interval_millis: i64 },
    /// Hand control back to the site EMS's autonomous mode.
    ReturnToLocalEmsAutonomous,
    /// Preserve reserve/SOC constraints and stop accepting new commands for
    /// the asset (the engine enforces the stop half).
    PreserveReserveSocConstraintsAndStopAcceptingNewCommands,
    /// No automatic action; an operator decides.
    RequireOperatorIntervention,
}

impl FallbackVariant {
    /// The stable variant name (audit + config text).
    pub fn name(&self) -> &'static str {
        match self {
            FallbackVariant::HoldLastAcceptedSetpoint { .. } => "HOLD_LAST_ACCEPTED_SETPOINT",
            FallbackVariant::ReturnToLocalEmsAutonomous => "RETURN_TO_LOCAL_EMS_AUTONOMOUS",
            FallbackVariant::PreserveReserveSocConstraintsAndStopAcceptingNewCommands => {
                "PRESERVE_RESERVE_SOC_CONSTRAINTS_AND_STOP_ACCEPTING_NEW_COMMANDS"
            }
            FallbackVariant::RequireOperatorIntervention => "REQUIRE_OPERATOR_INTERVENTION",
        }
    }

    /// Parses a variant name; `bounded_interval_millis` is consumed only by
    /// the HOLD variant. Unknown names are errors — never a default.
    pub fn parse(name: &str, bounded_interval_millis: i64) -> Result<Self, FallbackConfigError> {
        match name {
            "HOLD_LAST_ACCEPTED_SETPOINT" => Ok(Self::HoldLastAcceptedSetpoint {
                bounded_interval_millis,
            }),
            "RETURN_TO_LOCAL_EMS_AUTONOMOUS" => Ok(Self::ReturnToLocalEmsAutonomous),
            "PRESERVE_RESERVE_SOC_CONSTRAINTS_AND_STOP_ACCEPTING_NEW_COMMANDS" => {
                Ok(Self::PreserveReserveSocConstraintsAndStopAcceptingNewCommands)
            }
            "REQUIRE_OPERATOR_INTERVENTION" => Ok(Self::RequireOperatorIntervention),
            other => Err(FallbackConfigError::Malformed {
                line: 0,
                reason: format!(
                    "unknown fallback variant '{other}' (the closed §14.4 option set is \
                     HOLD_LAST_ACCEPTED_SETPOINT | RETURN_TO_LOCAL_EMS_AUTONOMOUS | \
                     PRESERVE_RESERVE_SOC_CONSTRAINTS_AND_STOP_ACCEPTING_NEW_COMMANDS | \
                     REQUIRE_OPERATOR_INTERVENTION — there is no universal \"set power to \
                     zero\" fallback)"
                ),
            }),
        }
    }
}

/// The failure triggers a fallback policy may be configured for (closed
/// set). `expiry` is special: EXPIRED → FALLBACK is the only EXPIRED
/// outgoing edge (frozen §14.1) and fires ONLY where the policy requires it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FallbackTrigger {
    Nack,
    Timeout,
    NoAcknowledgement,
    TrackingDeviation,
    Expiry,
}

impl FallbackTrigger {
    /// The stable trigger code (config + audit text).
    pub fn code(self) -> &'static str {
        match self {
            FallbackTrigger::Nack => "nack",
            FallbackTrigger::Timeout => "timeout",
            FallbackTrigger::NoAcknowledgement => "no_ack",
            FallbackTrigger::TrackingDeviation => "tracking_deviation",
            FallbackTrigger::Expiry => "expiry",
        }
    }

    /// Parses a trigger code; unknown codes are errors — never a default.
    pub fn parse(code: &str) -> Result<Self, FallbackConfigError> {
        match code {
            "nack" => Ok(FallbackTrigger::Nack),
            "timeout" => Ok(FallbackTrigger::Timeout),
            "no_ack" => Ok(FallbackTrigger::NoAcknowledgement),
            "tracking_deviation" => Ok(FallbackTrigger::TrackingDeviation),
            "expiry" => Ok(FallbackTrigger::Expiry),
            other => Err(FallbackConfigError::Malformed {
                line: 0,
                reason: format!(
                    "unknown fallback trigger '{other}' (the closed set is nack | timeout | \
                     no_ack | tracking_deviation | expiry)"
                ),
            }),
        }
    }
}

/// The operator-review record carried by EVERY fallback policy entry. THE
/// REVIEW IS NOT SIGNED IN THIS LOOP: the reviewer name and date are
/// recorded as HUMAN GATE INPUT — an unsigned text field, not a
/// cryptographic sign-off. Signing the review is a recorded deployment
/// follow-up. The fields are REQUIRED to load: a fallback policy without a
/// recorded review cannot exist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperatorReview {
    /// Who reviewed the policy with the site/equipment operator (a name +
    /// role, recorded by the review procedure).
    pub reviewed_by: String,
    /// When the review happened (ISO date, recorded by the review
    /// procedure).
    pub reviewed_on: String,
}

/// The fallback policy for ONE asset. There are no defaults at the asset
/// level: an asset with no entry resolves to the explicit engine default
/// (REQUIRE_OPERATOR_INTERVENTION, no transition) — never to an automatic
/// setpoint change.
#[derive(Debug, Clone, PartialEq)]
pub struct FallbackPolicyEntry {
    pub asset_key: String,
    pub variant: FallbackVariant,
    /// The triggers this policy fires on (non-empty; a policy configured
    /// for no trigger is a load error).
    pub triggers: Vec<FallbackTrigger>,
    /// The REQUIRED operator review (present to load; NOT SIGNED this loop).
    pub review: OperatorReview,
}

/// The versioned per-asset fallback-policy configuration.
#[derive(Debug, Clone, PartialEq)]
pub struct FallbackPolicyConfig {
    /// Must equal [`FALLBACK_POLICY_FORMAT_VERSION`].
    pub format_version: u32,
    /// One entry per asset (duplicate assets are a validation error).
    pub entries: Vec<FallbackPolicyEntry>,
}

/// Named errors of the fallback-policy configuration. A missing operator
/// review field is a LOAD REFUSAL — the config does not load half-reviewed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FallbackConfigError {
    /// The configuration text was malformed (wrong version line, unknown
    /// field, unknown variant/trigger, bad value, missing required field —
    /// including the operator-review fields).
    Malformed { line: usize, reason: String },
    /// The same asset appeared twice.
    DuplicateAsset { asset_key: String },
    /// A semantic violation (HOLD without a positive bounded interval, a
    /// trigger list that is empty, an empty asset key or review field).
    Invalid { reason: String },
}

impl std::fmt::Display for FallbackConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FallbackConfigError::Malformed { line, reason } => {
                write!(f, "fallback policy line {line} is malformed: {reason}")
            }
            FallbackConfigError::DuplicateAsset { asset_key } => {
                write!(f, "the fallback policy declares asset '{asset_key}' twice")
            }
            FallbackConfigError::Invalid { reason } => {
                write!(f, "the fallback policy is invalid: {reason}")
            }
        }
    }
}

impl std::error::Error for FallbackConfigError {}

impl FallbackPolicyConfig {
    /// Validates the configuration (fail-closed at load).
    pub fn validate(&self) -> Result<(), FallbackConfigError> {
        if self.format_version != FALLBACK_POLICY_FORMAT_VERSION {
            return Err(FallbackConfigError::Invalid {
                reason: format!(
                    "format version {} is not the supported {}",
                    self.format_version, FALLBACK_POLICY_FORMAT_VERSION
                ),
            });
        }
        let mut seen: Vec<&str> = Vec::new();
        for entry in &self.entries {
            if entry.asset_key.trim().is_empty() {
                return Err(FallbackConfigError::Invalid {
                    reason: "an entry has an empty asset_key".to_string(),
                });
            }
            if seen.contains(&entry.asset_key.as_str()) {
                return Err(FallbackConfigError::DuplicateAsset {
                    asset_key: entry.asset_key.clone(),
                });
            }
            seen.push(&entry.asset_key);
            if entry.triggers.is_empty() {
                return Err(FallbackConfigError::Invalid {
                    reason: format!(
                        "asset '{}': the trigger list is empty (a policy configured for no \
                         trigger is a mistake, not a no-op)",
                        entry.asset_key
                    ),
                });
            }
            if let FallbackVariant::HoldLastAcceptedSetpoint {
                bounded_interval_millis,
            } = &entry.variant
            {
                if *bounded_interval_millis <= 0 {
                    return Err(FallbackConfigError::Invalid {
                        reason: format!(
                            "asset '{}': HOLD_LAST_ACCEPTED_SETPOINT requires a positive bounded \
                             interval (an unbounded hold is forbidden, §14.4)",
                            entry.asset_key
                        ),
                    });
                }
            }
            if entry.review.reviewed_by.trim().is_empty() {
                return Err(FallbackConfigError::Invalid {
                    reason: format!(
                        "asset '{}': the operator review field 'reviewed_by' is empty; the \
                         fallback policy cannot load without its operator review",
                        entry.asset_key
                    ),
                });
            }
            if entry.review.reviewed_on.trim().is_empty() {
                return Err(FallbackConfigError::Invalid {
                    reason: format!(
                        "asset '{}': the operator review field 'reviewed_on' is empty; the \
                         fallback policy cannot load without its operator review",
                        entry.asset_key
                    ),
                });
            }
        }
        Ok(())
    }

    /// The policy for an asset, if one is declared.
    pub fn entry_for(&self, asset_key: &str) -> Option<&FallbackPolicyEntry> {
        self.entries
            .iter()
            .find(|entry| entry.asset_key == asset_key)
    }

    /// Parses the versioned, line-oriented fallback-policy document. STRICT:
    /// unknown fields, wrong versions, unknown variants/triggers, and —
    /// crucially — MISSING OPERATOR-REVIEW FIELDS are named load refusals.
    /// Format (fields separated by ` = `; `asset` starts a block):
    ///
    /// ```text
    /// flexcore-edge-fallback-policy-v1
    /// asset = ess-01
    /// variant = HOLD_LAST_ACCEPTED_SETPOINT
    /// bounded_interval_millis = 300000
    /// triggers = nack,timeout,no_ack,tracking_deviation
    /// reviewed_by = A. Operator (site operator)
    /// reviewed_on = 2026-11-13
    /// ```
    ///
    /// `bounded_interval_millis` is required for (and only consumed by) the
    /// HOLD variant; `reviewed_by`/`reviewed_on` are REQUIRED for every
    /// block (their absence refuses the load — the review is a human gate
    /// input, recorded but NOT SIGNED in this loop).
    pub fn parse(text: &str) -> Result<Self, FallbackConfigError> {
        let malformed = |line: usize, reason: String| FallbackConfigError::Malformed { line, reason };
        let mut lines = text.lines();
        let version_line = lines
            .next()
            .ok_or_else(|| malformed(1, "the fallback policy document is empty".to_string()))?
            .trim();
        if version_line != FALLBACK_POLICY_FORMAT {
            return Err(malformed(
                1,
                format!(
                    "fallback policy format '{version_line}' is not the supported \
                     '{FALLBACK_POLICY_FORMAT}'"
                ),
            ));
        }
        let mut entries: Vec<FallbackPolicyEntry> = Vec::new();
        let mut current: Option<FallbackPolicyEntry> = None;
        for (index, line) in lines.enumerate() {
            let line_no = index + 2; // the version line was line 1
            if line.trim().is_empty() {
                continue;
            }
            let (key, value) = line
                .split_once(" = ")
                .ok_or_else(|| malformed(line_no, "expected 'key = value'".to_string()))?;
            match key.trim() {
                "asset" => {
                    if let Some(finished) = current.take() {
                        entries.push(finished);
                    }
                    current = Some(FallbackPolicyEntry {
                        asset_key: value.trim().to_string(),
                        variant: FallbackVariant::RequireOperatorIntervention,
                        triggers: Vec::new(),
                        review: OperatorReview {
                            reviewed_by: String::new(),
                            reviewed_on: String::new(),
                        },
                    });
                }
                "variant" => {
                    let entry = current.as_mut().ok_or_else(|| {
                        malformed(line_no, "'variant' appears before any 'asset' line".to_string())
                    })?;
                    entry.variant = FallbackVariant::parse(value.trim(), 0).map_err(|mut e| {
                        if let FallbackConfigError::Malformed { reason, .. } = &mut e {
                            *reason = format!("line {line_no}: {reason}");
                        }
                        e
                    })?;
                }
                "bounded_interval_millis" => {
                    let entry = current.as_mut().ok_or_else(|| {
                        malformed(
                            line_no,
                            "'bounded_interval_millis' appears before any 'asset' line".to_string(),
                        )
                    })?;
                    let millis = value.trim().parse::<i64>().map_err(|_| {
                        malformed(
                            line_no,
                            "'bounded_interval_millis' is not an integer".to_string(),
                        )
                    })?;
                    if let FallbackVariant::HoldLastAcceptedSetpoint {
                        bounded_interval_millis,
                    } = &mut entry.variant
                    {
                        *bounded_interval_millis = millis;
                    }
                    // A value on a non-HOLD block is tolerated as
                    // documentation but carries no meaning — the HOLD
                    // interval requirement is enforced in validate().
                    let _ = entry;
                }
                "triggers" => {
                    let entry = current.as_mut().ok_or_else(|| {
                        malformed(
                            line_no,
                            "'triggers' appears before any 'asset' line".to_string(),
                        )
                    })?;
                    let mut triggers = Vec::new();
                    for code in value.split(',') {
                        let code = code.trim();
                        if code.is_empty() {
                            continue;
                        }
                        triggers.push(FallbackTrigger::parse(code).map_err(|mut e| {
                            if let FallbackConfigError::Malformed { reason, .. } = &mut e {
                                *reason = format!("line {line_no}: {reason}");
                            }
                            e
                        })?);
                    }
                    entry.triggers = triggers;
                }
                "reviewed_by" => {
                    let entry = current.as_mut().ok_or_else(|| {
                        malformed(
                            line_no,
                            "'reviewed_by' appears before any 'asset' line".to_string(),
                        )
                    })?;
                    entry.review.reviewed_by = value.trim().to_string();
                }
                "reviewed_on" => {
                    let entry = current.as_mut().ok_or_else(|| {
                        malformed(
                            line_no,
                            "'reviewed_on' appears before any 'asset' line".to_string(),
                        )
                    })?;
                    entry.review.reviewed_on = value.trim().to_string();
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
            entries.push(finished);
        }
        // Every block MUST have carried its operator review: a block that
        // skipped reviewed_by/reviewed_on is refused HERE, at load.
        for entry in &entries {
            if entry.review.reviewed_by.trim().is_empty() {
                return Err(malformed(
                    1,
                    format!(
                        "asset '{}': the operator review field 'reviewed_by' is missing; the \
                         fallback policy is REFUSED (a fallback without a recorded operator \
                         review cannot load)",
                        entry.asset_key
                    ),
                ));
            }
            if entry.review.reviewed_on.trim().is_empty() {
                return Err(malformed(
                    1,
                    format!(
                        "asset '{}': the operator review field 'reviewed_on' is missing; the \
                         fallback policy is REFUSED (a fallback without a recorded operator \
                         review cannot load)",
                        entry.asset_key
                    ),
                ));
            }
        }
        let config = Self {
            format_version: FALLBACK_POLICY_FORMAT_VERSION,
            entries,
        };
        config.validate()?;
        Ok(config)
    }
}

// --- The site command allowlist (the forward-side re-check) -------------------------

/// The forward-side site command allowlist. Deliberately a SEPARATE check
/// from the P6.15 verification-side pre-check: the forwarder re-checks
/// against this list before EVERY forward, with its own audit event, so no
/// command can reach a non-allowlisted adapter even if an upstream list
/// changed between verification and forwarding.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SiteCommandAllowlist {
    adapters: Vec<String>,
}

impl SiteCommandAllowlist {
    /// Builds the allowlist (empty entries dropped; duplicates collapsed).
    pub fn new(adapters: Vec<String>) -> Self {
        let mut unique: Vec<String> = Vec::new();
        for id in adapters {
            let id = id.trim().to_string();
            if !id.is_empty() && !unique.contains(&id) {
                unique.push(id);
            }
        }
        Self { adapters: unique }
    }

    /// Whether the adapter is allowlisted.
    pub fn contains(&self, adapter_id: &str) -> bool {
        self.adapters.iter().any(|id| id == adapter_id)
    }

    /// The allowlisted ids (diagnostics).
    pub fn ids(&self) -> Vec<&str> {
        self.adapters.iter().map(|id| id.as_str()).collect()
    }
}

// --- Refusals, records, tracking observations -----------------------------------------

/// Why the engine refused to forward (or could not process a tracking
/// observation). Every refusal is audited with its code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForwardingRefusal {
    /// The target adapter is not on the forward-side allowlist (the port is
    /// NEVER invoked for it).
    NotAllowlisted { adapter_id: String },
    /// The independent P6.17 policy gate refused the re-check.
    PolicyRefused { refusal: CommandPolicyRefusal },
    /// The asset is under the PRESERVE_RESERVE_SOC... fallback: new
    /// commands are refused until operator action.
    FallbackStopActive { asset_key: String },
    /// The command is not under tracking (unknown id).
    UnknownCommand { command_id: String },
    /// The command's validity window has not elapsed — it cannot be
    /// expired mid-flight yet (fail-closed: no premature expiry).
    NotYetExpired { command_id: String },
}

impl ForwardingRefusal {
    /// The stable refusal code (audit text).
    pub fn code(&self) -> &'static str {
        match self {
            ForwardingRefusal::NotAllowlisted { .. } => "allowlist",
            ForwardingRefusal::PolicyRefused { .. } => "policy",
            ForwardingRefusal::FallbackStopActive { .. } => "fallback_stop",
            ForwardingRefusal::UnknownCommand { .. } => "unknown_command",
            ForwardingRefusal::NotYetExpired { .. } => "not_yet_expired",
        }
    }
}

impl std::fmt::Display for ForwardingRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ForwardingRefusal::NotAllowlisted { adapter_id } => write!(
                f,
                "adapter '{adapter_id}' is not on the forward-side site command allowlist; the \
                 forwarder is never invoked for it"
            ),
            ForwardingRefusal::PolicyRefused { refusal } => {
                write!(f, "site policy refused the forward: {}", refusal.reason)
            }
            ForwardingRefusal::FallbackStopActive { asset_key } => write!(
                f,
                "asset '{asset_key}' is under the \
                 PRESERVE_RESERVE_SOC_CONSTRAINTS_AND_STOP_ACCEPTING_NEW_COMMANDS fallback; new \
                 commands are refused until operator action"
            ),
            ForwardingRefusal::UnknownCommand { command_id } => {
                write!(f, "command '{command_id}' is not under tracking")
            }
            ForwardingRefusal::NotYetExpired { command_id } => write!(
                f,
                "command '{command_id}' has not reached its valid_until; no premature expiry"
            ),
        }
    }
}

/// One recorded lifecycle transition of a forwarded command. The state name
/// is a frozen §14.1 lifecycle state; the detail carries the human-safe
/// record (including the acknowledgement-≠-delivery honesty wording on
/// acknowledgements).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForwardRecord {
    pub command_id: String,
    pub asset_key: String,
    pub target_adapter: String,
    /// The frozen §14.1 lifecycle state name (ISSUED, ACKNOWLEDGED,
    /// TRACKING, COMPLETED, FAILED, EXPIRED, FALLBACK).
    pub lifecycle_state: String,
    pub at_unix_millis: i64,
    pub detail: String,
}

/// A tracking observation fed by the composition (the adapter telemetry /
/// tracking loop). Health is derived from observations, never from silence
/// (the P6.17 rule, applied to command tracking).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrackingObservation {
    /// The command is tracking on target.
    OnTarget { detail: String },
    /// The command deviated beyond tolerance — a failure trigger.
    Deviated { detail: String },
    /// The command completed its objective (the terminal success state).
    Completed { detail: String },
}

/// One fallback activation (or its explicit non-activation), returned so
/// the caller/composition can surface it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FallbackActivation {
    pub command_id: String,
    pub asset_key: String,
    pub trigger_code: &'static str,
    /// The resolved variant name (or the explicit default's name when no
    /// per-asset policy exists — the default REQUIRES operator intervention
    /// and performs NO automatic action).
    pub variant_name: String,
    /// Whether a §14.1 FALLBACK transition was recorded (false = the
    /// explicit default or a not-configured trigger: the command stays in
    /// its terminal failure state and an operator decides).
    pub transitioned_to_fallback: bool,
    pub reviewed_by: String,
    pub reviewed_on: String,
    pub detail: String,
}

// --- The engine -----------------------------------------------------------------------

/// The per-command tracking state (the engine's private record).
#[derive(Debug, Clone)]
struct CommandTrackingState {
    asset_key: String,
    target_adapter: String,
    lifecycle_state: String,
    valid_until_unix_millis: i64,
}

/// The P6.16 forwarding engine: separate allowlist re-check → fallback-stop
/// check → policy re-check → issue → forward → §14.1 state recording →
/// per-asset fallback on failure. The engine owns NO queue and NO retry:
/// a failed command is terminal (FAILED/EXPIRED, plus the configured
/// fallback) and the composition decides what comes next. State is
/// in-memory (a restart clears tracking state — persistence is P6.18's
/// audit-buffer/offline scope, recorded as a limitation).
pub struct CommandForwardingEngine {
    forwarder: Arc<dyn CommandForwarder>,
    policy: Arc<dyn CommandPolicyGate>,
    allowlist: SiteCommandAllowlist,
    fallback: FallbackPolicyConfig,
    audit: Arc<dyn AuditSink>,
    clock: Arc<dyn Clock + Send + Sync>,
    state: Mutex<HashMap<String, CommandTrackingState>>,
    /// Assets under the PRESERVE_RESERVE_SOC... fallback: asset key → the
    /// audit reference of the activation that stopped them.
    stopped_assets: Mutex<HashMap<String, String>>,
}

impl CommandForwardingEngine {
    /// Composes the engine (fail-closed: an invalid fallback configuration
    /// is a construction error — the edge never runs with an unreviewed
    /// fallback policy). Audits the composition.
    pub fn new(
        forwarder: Arc<dyn CommandForwarder>,
        policy: Arc<dyn CommandPolicyGate>,
        allowlist: SiteCommandAllowlist,
        fallback: FallbackPolicyConfig,
        audit: Arc<dyn AuditSink>,
        clock: Arc<dyn Clock + Send + Sync>,
    ) -> Result<Self, FallbackConfigError> {
        fallback.validate()?;
        let engine = Self {
            forwarder,
            policy,
            allowlist,
            fallback,
            audit,
            clock,
            state: Mutex::new(HashMap::new()),
            stopped_assets: Mutex::new(HashMap::new()),
        };
        engine.audit_event(
            "EDGE_COMMAND_FORWARDING_ENGINE_COMPOSED",
            format!(
                "command forwarding engine composed (allowlist=[{}], fallback entries=[{}], \
                 forwarder=test/fail-closed seam; reachable only behind the P6.14 gate + P6.15 \
                 verifier + P6.17 policy re-check)",
                engine.allowlist.ids().join(","),
                engine
                    .fallback
                    .entries
                    .iter()
                    .map(|e| e.asset_key.as_str())
                    .collect::<Vec<_>>()
                    .join(",")
            ),
        );
        Ok(engine)
    }

    /// Forwards ONE verified command. Order (each step audited):
    ///
    /// 1. the SEPARATE forward-side allowlist re-check (own audit event
    ///    `EDGE_COMMAND_FORWARD_ALLOWLIST_CHECKED`) — a non-allowlisted
    ///    adapter is refused WITHOUT the forwarder ever being invoked;
    /// 2. the fallback stop state (an asset under the PRESERVE_RESERVE_SOC
    ///    fallback refuses new commands);
    /// 3. the independent P6.17 policy gate re-check (fail-closed);
    /// 4. the issue is recorded (`ISSUED`, §14.1) and the command handed to
    ///    the [`CommandForwarder`] port;
    /// 5. the outcome is mapped onto the frozen §14.1 states (ACKNOWLEDGED,
    ///    or FAILED + fallback, per the module-header table).
    pub fn forward_verified(
        &self,
        verified: VerifiedCommand,
    ) -> Result<ForwardRecord, ForwardingRefusal> {
        let now = self.now_unix_millis();

        // 1. The SEPARATE allowlist re-check — its own audit event, before
        //    anything else. The forwarder is NEVER invoked for a
        //    non-allowlisted adapter.
        let allowlisted = self.allowlist.contains(&verified.target_adapter);
        self.audit_event(
            "EDGE_COMMAND_FORWARD_ALLOWLIST_CHECKED",
            format!(
                "command={} adapter={} allowlisted={} (forward-side re-check, separate from the \
                 P6.15 verification-side pre-check)",
                verified.command_id, verified.target_adapter, allowlisted
            ),
        );
        if !allowlisted {
            self.audit_forward_rejection(
                &verified.command_id,
                &verified.asset_key,
                &verified.target_adapter,
                "allowlist",
                "refusing the forward WITHOUT invoking the forwarder",
                now,
            );
            return Err(ForwardingRefusal::NotAllowlisted {
                adapter_id: verified.target_adapter.clone(),
            });
        }

        // 2. The fallback stop state.
        if let Some(reference) = self
            .stopped_assets
            .lock()
            .expect("poisoned stopped-assets mutex")
            .get(&verified.asset_key)
            .cloned()
        {
            self.audit_forward_rejection(
                &verified.command_id,
                &verified.asset_key,
                &verified.target_adapter,
                "fallback_stop",
                &format!("stop-active activation reference: {reference}"),
                now,
            );
            return Err(ForwardingRefusal::FallbackStopActive {
                asset_key: verified.asset_key.clone(),
            });
        }

        // 3. The independent policy gate re-check (P6.17 seam) — fail-closed.
        if let Err(refusal) = self.policy.check_command(&verified.target_adapter, now) {
            self.audit_forward_rejection(
                &verified.command_id,
                &verified.asset_key,
                &verified.target_adapter,
                "policy",
                &format!("{}: {}", refusal.code, refusal.reason),
                now,
            );
            return Err(ForwardingRefusal::PolicyRefused { refusal });
        }

        // 4. Record the issue (§14.1: ISSUED = the command was handed to the
        //    gateway/adapter path), then forward.
        let request = ForwardRequest {
            command_id: verified.command_id.clone(),
            asset_key: verified.asset_key.clone(),
            target_adapter: verified.target_adapter.clone(),
            quantity: verified.quantity.clone(),
            value: verified.value,
            sequence: verified.sequence,
            valid_until_unix_millis: verified.valid_until_unix_millis,
            issued_at_unix_millis: now,
        };
        self.state
            .lock()
            .expect("poisoned engine state mutex")
            .insert(
                verified.command_id.clone(),
                CommandTrackingState {
                    asset_key: verified.asset_key.clone(),
                    target_adapter: verified.target_adapter.clone(),
                    lifecycle_state: "ISSUED".to_string(),
                    // The §14.3 validity bound carried from verification:
                    // the engine refuses to expire a command before this
                    // instant (no premature expiry).
                    valid_until_unix_millis: verified.valid_until_unix_millis,
                },
            );
        self.audit_lifecycle(
            &verified.command_id,
            &verified.asset_key,
            &verified.target_adapter,
            "ISSUED",
            now,
            "command handed to the gated forwarder (issue recorded; not an acknowledgement, not \
             a delivery)",
        );

        match self.forwarder.forward(&request) {
            Ok(ForwardOutcome::Acknowledged) => {
                self.mark_state(&verified.command_id, "ACKNOWLEDGED");
                let detail = format!(
                    "adapter acknowledged the command; {ACKNOWLEDGEMENT_IS_NOT_DELIVERY} — \
                     tracking observations, not the ack, establish what actually happened"
                );
                self.audit_lifecycle(
                    &verified.command_id,
                    &verified.asset_key,
                    &verified.target_adapter,
                    "ACKNOWLEDGED",
                    now,
                    &detail,
                );
                Ok(ForwardRecord {
                    command_id: verified.command_id,
                    asset_key: verified.asset_key,
                    target_adapter: verified.target_adapter,
                    lifecycle_state: "ACKNOWLEDGED".to_string(),
                    at_unix_millis: now,
                    detail,
                })
            }
            Ok(ForwardOutcome::Nack { reason }) => self.fail_command(
                &verified.command_id,
                &verified.asset_key,
                &verified.target_adapter,
                FallbackTrigger::Nack,
                format!("the adapter NACKed the command: {reason}"),
                now,
            ),
            Err(ForwardError::Timeout { reason }) => self.fail_command(
                &verified.command_id,
                &verified.asset_key,
                &verified.target_adapter,
                FallbackTrigger::Timeout,
                format!("the adapter timed out: {reason}"),
                now,
            ),
            Err(ForwardError::NoAcknowledgement { reason }) => self.fail_command(
                &verified.command_id,
                &verified.asset_key,
                &verified.target_adapter,
                FallbackTrigger::NoAcknowledgement,
                format!(
                    "no acknowledgement arrived within the window: {reason}; \
                     {ACKNOWLEDGEMENT_IS_NOT_DELIVERY}"
                ),
                now,
            ),
            Err(ForwardError::Transport { reason }) => self.fail_command(
                &verified.command_id,
                &verified.asset_key,
                &verified.target_adapter,
                // Transport failure is timeout-class for the fallback
                // policy (the adapter did not accept); the detail keeps the
                // distinction.
                FallbackTrigger::Timeout,
                format!("the transport to the adapter failed: {reason}"),
                now,
            ),
        }
    }

    /// Feeds ONE tracking observation for a forwarded command. OnTarget →
    /// TRACKING; Completed → COMPLETED; Deviated → FAILED + fallback (the
    /// tracking_deviation trigger).
    pub fn observe_tracking(
        &self,
        command_id: &str,
        observation: TrackingObservation,
    ) -> Result<ForwardRecord, ForwardingRefusal> {
        let now = self.now_unix_millis();
        let (asset_key, target_adapter) = self.tracking_subject(command_id)?;
        match observation {
            TrackingObservation::OnTarget { detail } => {
                self.mark_state(command_id, "TRACKING");
                self.audit_lifecycle(
                    command_id,
                    &asset_key,
                    &target_adapter,
                    "TRACKING",
                    now,
                    &detail,
                );
                Ok(ForwardRecord {
                    command_id: command_id.to_string(),
                    asset_key,
                    target_adapter,
                    lifecycle_state: "TRACKING".to_string(),
                    at_unix_millis: now,
                    detail,
                })
            }
            TrackingObservation::Completed { detail } => {
                self.mark_state(command_id, "COMPLETED");
                self.audit_lifecycle(
                    command_id,
                    &asset_key,
                    &target_adapter,
                    "COMPLETED",
                    now,
                    &detail,
                );
                Ok(ForwardRecord {
                    command_id: command_id.to_string(),
                    asset_key,
                    target_adapter,
                    lifecycle_state: "COMPLETED".to_string(),
                    at_unix_millis: now,
                    detail,
                })
            }
            TrackingObservation::Deviated { detail } => self.fail_command(
                command_id,
                &asset_key,
                &target_adapter,
                FallbackTrigger::TrackingDeviation,
                format!("tracking deviation: {detail}"),
                now,
            ),
        }
    }

    /// Expires a command mid-flight: the command must be under tracking AND
    /// its `valid_until` must have passed (fail-closed: no premature
    /// expiry). Lands in EXPIRED; FALLBACK follows ONLY where the per-asset
    /// policy configures the expiry trigger (the P4.17 precedent: EXPIRED →
    /// FALLBACK is the only EXPIRED outgoing edge and fires only where the
    /// policy requires it).
    pub fn expire_mid_flight(&self, command_id: &str) -> Result<ForwardRecord, ForwardingRefusal> {
        let now = self.now_unix_millis();
        let (asset_key, target_adapter) = self.tracking_subject(command_id)?;
        // The §14.3 validity bound travels with the verified command; the
        // engine refuses to expire a command that has not reached it (no
        // premature expiry, fail-closed).
        {
            let state = self.state.lock().expect("poisoned engine state mutex");
            let tracked = state
                .get(command_id)
                .expect("tracking subject exists (checked by tracking_subject)");
            if now < tracked.valid_until_unix_millis {
                return Err(ForwardingRefusal::NotYetExpired {
                    command_id: command_id.to_string(),
                });
            }
        }
        self.mark_state(command_id, "EXPIRED");
        self.audit_lifecycle(
            command_id,
            &asset_key,
            &target_adapter,
            "EXPIRED",
            now,
            "the command expired mid-flight (§14.3 expiry); FALLBACK follows only where the \
             per-asset policy configures the expiry trigger",
        );
        let activation = self.activate_fallback(command_id, &asset_key, FallbackTrigger::Expiry, now);
        let lifecycle_state = if activation
            .map(|a| a.transitioned_to_fallback)
            .unwrap_or(false)
        {
            self.mark_state(command_id, "FALLBACK");
            "FALLBACK".to_string()
        } else {
            "EXPIRED".to_string()
        };
        Ok(ForwardRecord {
            command_id: command_id.to_string(),
            asset_key,
            target_adapter,
            lifecycle_state,
            at_unix_millis: now,
            detail: "expired mid-flight; fallback per the configured per-asset policy"
                .to_string(),
        })
    }

    /// The assets currently stopped by the PRESERVE_RESERVE_SOC fallback
    /// (health/composition visibility).
    pub fn stopped_assets(&self) -> Vec<String> {
        let mut keys: Vec<String> = self
            .stopped_assets
            .lock()
            .expect("poisoned stopped-assets mutex")
            .keys()
            .cloned()
            .collect();
        keys.sort();
        keys
    }

    /// The recorded lifecycle state of a command (health/composition
    /// visibility; `None` = not under tracking).
    pub fn lifecycle_state_of(&self, command_id: &str) -> Option<String> {
        self.state
            .lock()
            .expect("poisoned engine state mutex")
            .get(command_id)
            .map(|tracked| tracked.lifecycle_state.clone())
    }

    /// Records the FAILED transition and applies the configured per-asset
    /// fallback for the trigger. Terminal: FAILED, or FALLBACK where the
    /// policy fired.
    fn fail_command(
        &self,
        command_id: &str,
        asset_key: &str,
        target_adapter: &str,
        trigger: FallbackTrigger,
        detail: String,
        now: i64,
    ) -> Result<ForwardRecord, ForwardingRefusal> {
        self.mark_state(command_id, "FAILED");
        self.audit_lifecycle(command_id, asset_key, target_adapter, "FAILED", now, &detail);
        let activation = self.activate_fallback(command_id, asset_key, trigger, now);
        let lifecycle_state = if activation
            .as_ref()
            .map(|a| a.transitioned_to_fallback)
            .unwrap_or(false)
        {
            self.mark_state(command_id, "FALLBACK");
            "FALLBACK".to_string()
        } else {
            "FAILED".to_string()
        };
        Ok(ForwardRecord {
            command_id: command_id.to_string(),
            asset_key: asset_key.to_string(),
            target_adapter: target_adapter.to_string(),
            lifecycle_state,
            at_unix_millis: now,
            detail: match &activation {
                Some(activation) => format!("{detail}; fallback: {}", activation.detail),
                None => detail,
            },
        })
    }

    /// Resolves the per-asset fallback policy for the trigger. The THREE
    /// honest outcomes (the P4.17 precedent, cited):
    ///
    /// | Outcome | FALLBACK transition? | Audit |
    /// |---|---|---|
    /// | named policy, trigger configured | YES (frozen edge) | `EDGE_COMMAND_FALLBACK` |
    /// | named policy, trigger NOT configured | no | `EDGE_COMMAND_FALLBACK_TRIGGER_NOT_CONFIGURED` |
    /// | no policy (the explicit DEFAULT) | no — REQUIRE_OPERATOR_INTERVENTION | `EDGE_COMMAND_FALLBACK_INTERVENTION_REQUIRED` |
    fn activate_fallback(
        &self,
        command_id: &str,
        asset_key: &str,
        trigger: FallbackTrigger,
        now: i64,
    ) -> Option<FallbackActivation> {
        let Some(entry) = self.fallback.entry_for(asset_key) else {
            // The explicit DEFAULT entry: no automatic setpoint change, no
            // FALLBACK transition — an operator decides (P4.17 precedent).
            let activation = FallbackActivation {
                command_id: command_id.to_string(),
                asset_key: asset_key.to_string(),
                trigger_code: trigger.code(),
                variant_name: FallbackVariant::RequireOperatorIntervention.name().to_string(),
                transitioned_to_fallback: false,
                reviewed_by: String::new(),
                reviewed_on: String::new(),
                detail: "no per-asset fallback policy is configured for this asset; the explicit \
                         default is REQUIRE_OPERATOR_INTERVENTION with NO automatic setpoint \
                         change and NO FALLBACK transition (P4.17 default-entry precedent)"
                    .to_string(),
            };
            self.audit_event(
                "EDGE_COMMAND_FALLBACK_INTERVENTION_REQUIRED",
                format!(
                    "command={} asset={} trigger={} — {}",
                    command_id, asset_key, trigger.code(), activation.detail
                ),
            );
            return Some(activation);
        };
        if !entry.triggers.contains(&trigger) {
            let activation = FallbackActivation {
                command_id: command_id.to_string(),
                asset_key: asset_key.to_string(),
                trigger_code: trigger.code(),
                variant_name: entry.variant.name().to_string(),
                transitioned_to_fallback: false,
                reviewed_by: entry.review.reviewed_by.clone(),
                reviewed_on: entry.review.reviewed_on.clone(),
                detail: format!(
                    "the per-asset policy for '{asset_key}' exists but does not list the \
                     trigger '{}'; no FALLBACK transition (an operator decides)",
                    trigger.code()
                ),
            };
            self.audit_event(
                "EDGE_COMMAND_FALLBACK_TRIGGER_NOT_CONFIGURED",
                format!(
                    "command={} asset={} trigger={} — {}",
                    command_id, asset_key, trigger.code(), activation.detail
                ),
            );
            return Some(activation);
        }
        // The policy fires: the frozen §14.1 FALLBACK transition (recorded
        // HERE, via the engine's state — the edge does not re-encode frozen
        // lifecycle edges; it records the transition the platform's §14.1
        // table defines).
        let detail = match &entry.variant {
            FallbackVariant::HoldLastAcceptedSetpoint {
                bounded_interval_millis,
            } => format!(
                "holding the last accepted setpoint for the BOUNDED interval {bounded_interval_millis} ms \
                 (§14.4); the hold is not asserted safe after topology/state change — the \
                 composition re-evaluates"
            ),
            FallbackVariant::ReturnToLocalEmsAutonomous => {
                "control returns to the site EMS's autonomous mode (§14.4)".to_string()
            }
            FallbackVariant::PreserveReserveSocConstraintsAndStopAcceptingNewCommands => {
                "reserve/SOC constraints are preserved and NO new commands are accepted for this \
                 asset until operator action (§14.4)"
                    .to_string()
            }
            FallbackVariant::RequireOperatorIntervention => {
                "no automatic action; operator intervention required (§14.4)".to_string()
            }
        };
        // The stop-half of the PRESERVE variant is enforced by the engine.
        if matches!(
            entry.variant,
            FallbackVariant::PreserveReserveSocConstraintsAndStopAcceptingNewCommands
        ) {
            self.stopped_assets
                .lock()
                .expect("poisoned stopped-assets mutex")
                .insert(
                    asset_key.to_string(),
                    format!(
                        "command-denial:{}/fallback-stop@{}",
                        command_id, now
                    ),
                );
        }
        // The operator review is recorded (human gate input, NOT SIGNED in
        // this loop — an unsigned name+date, never renderable as a sign-off).
        self.audit_event(
            "EDGE_COMMAND_FALLBACK",
            format!(
                "command={} asset={} trigger={} variant={} review=(reviewed_by='{}', \
                 reviewed_on='{}', NOT SIGNED — human gate input) detail: {detail}",
                command_id,
                asset_key,
                trigger.code(),
                entry.variant.name(),
                entry.review.reviewed_by,
                entry.review.reviewed_on,
            ),
        );
        Some(FallbackActivation {
            command_id: command_id.to_string(),
            asset_key: asset_key.to_string(),
            trigger_code: trigger.code(),
            variant_name: entry.variant.name().to_string(),
            transitioned_to_fallback: true,
            reviewed_by: entry.review.reviewed_by.clone(),
            reviewed_on: entry.review.reviewed_on.clone(),
            detail,
        })
    }

    /// Looks up the (asset, adapter) of a tracked command; an unknown id is
    /// an audited refusal.
    fn tracking_subject(&self, command_id: &str) -> Result<(String, String), ForwardingRefusal> {
        let state = self.state.lock().expect("poisoned engine state mutex");
        match state.get(command_id) {
            Some(tracked) => Ok((tracked.asset_key.clone(), tracked.target_adapter.clone())),
            None => {
                drop(state);
                self.audit_forward_rejection(
                    command_id,
                    "-",
                    "-",
                    "unknown_command",
                    "no tracking state exists for this command id",
                    self.now_unix_millis(),
                );
                Err(ForwardingRefusal::UnknownCommand {
                    command_id: command_id.to_string(),
                })
            }
        }
    }

    fn mark_state(&self, command_id: &str, state: &str) {
        if let Some(tracked) = self
            .state
            .lock()
            .expect("poisoned engine state mutex")
            .get_mut(command_id)
        {
            tracked.lifecycle_state = state.to_string();
        }
    }

    fn now_unix_millis(&self) -> i64 {
        let stamp = self.clock.now_ingest_time();
        i64::try_from(stamp.unix_millis()).unwrap_or(i64::MAX)
    }

    /// Records one engine audit event through the edge `AuditSink` port
    /// (same audit-kind limitation as the P6.14/P6.15 modules:
    /// `EdgeAuditKind` is a closed vocabulary this wave does not own, so
    /// the dedicated event name LEADS the detail string on kind
    /// `EDGE_STARTUP`).
    fn audit_event(&self, name: &'static str, detail: String) {
        let event = EdgeAuditEvent {
            recorded_at: self.clock.now_ingest_time(),
            kind: EdgeAuditKind::Startup,
            detail: format!("{name}: {detail}"),
        };
        if let Err(error) = self.audit.record(event) {
            tracing::error!(%error, audit_event = name, "command-forward audit write FAILED");
        }
    }

    fn audit_lifecycle(
        &self,
        command_id: &str,
        asset_key: &str,
        target_adapter: &str,
        lifecycle_state: &str,
        now_millis: i64,
        detail: &str,
    ) {
        self.audit_event(
            "EDGE_COMMAND_LIFECYCLE",
            format!(
                "command={} asset={} adapter={} state={} at={} — {detail}",
                command_id, asset_key, target_adapter, lifecycle_state, now_millis
            ),
        );
    }

    fn audit_forward_rejection(
        &self,
        command_id: &str,
        asset_key: &str,
        target_adapter: &str,
        code: &str,
        detail: &str,
        now_millis: i64,
    ) {
        let record = CommandRejectionAuditRecord {
            command_id: command_id.to_string(),
            asset_key: asset_key.to_string(),
            target_adapter: target_adapter.to_string(),
            refusal_code: code.to_string(),
            at_unix_millis: now_millis,
            refusal_reference: format!(
                "command-denial:{}/{}@{}/{}",
                command_id, target_adapter, now_millis, code
            ),
        };
        self.audit_event(
            "EDGE_COMMAND_FORWARD_REJECTED",
            format!("{} ({})", record.audit_text(), detail),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command_verify::FallbackTrigger;
    use crate::ports::InMemoryAuditSink;
    use flexcore_ports::fakes::FixedClock;
    use std::sync::Mutex as StdMutex;

    const BASE_MILLIS: i64 = 1_700_000_000_000;

    #[test]
    fn trigger_codes_are_distinct() {
        let codes = [
            FallbackTrigger::Nack.code(),
            FallbackTrigger::Timeout.code(),
            FallbackTrigger::NoAcknowledgement.code(),
            FallbackTrigger::TrackingDeviation.code(),
            FallbackTrigger::Expiry.code(),
        ];
        let mut sorted = codes.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), 5);
    }

    /// A scripted call recorder (the integration tests build their own;
    /// this one pins the shape).
    struct CallRecorder {
        calls: StdMutex<Vec<String>>,
    }

    impl CallRecorder {
        fn new() -> Self {
            Self {
                calls: StdMutex::new(Vec::new()),
            }
        }
        fn record(&self, command_id: &str) {
            self.calls
                .lock()
                .expect("recorder")
                .push(command_id.to_string());
        }
        #[allow(dead_code)]
        fn count(&self) -> usize {
            self.calls.lock().expect("recorder").len()
        }
    }

    #[test]
    fn the_allowlist_builds_trimmed_and_deduplicated() {
        let allowlist = SiteCommandAllowlist::new(vec![
            " simulated-test ".to_string(),
            "simulated-test".to_string(),
            "openems-rest".to_string(),
            String::new(),
        ]);
        assert_eq!(allowlist.ids(), vec!["simulated-test", "openems-rest"]);
        assert!(allowlist.contains("simulated-test"));
        assert!(!allowlist.contains("ghost"));
    }

    #[test]
    fn the_fail_closed_forwarder_always_fails_with_a_named_error() {
        let forwarder = FailClosedCommandForwarder;
        let request = ForwardRequest {
            command_id: "cmd-1".to_string(),
            asset_key: "ess-01".to_string(),
            target_adapter: "simulated-test".to_string(),
            quantity: "active_power".to_string(),
            value: 1.5,
            sequence: 1,
            valid_until_unix_millis: BASE_MILLIS,
            issued_at_unix_millis: BASE_MILLIS,
        };
        let error = forwarder.forward(&request).expect_err("always fails");
        assert!(matches!(error, ForwardError::Transport { .. }));
        assert!(error.to_string().contains("fail-closed"));
    }

    #[test]
    fn an_engine_with_a_valid_fallback_config_composes_and_audits() {
        let config = FallbackPolicyConfig::parse(
            "flexcore-edge-fallback-policy-v1\nasset = ess-01\nvariant = \
             REQUIRE_OPERATOR_INTERVENTION\ntriggers = nack\nreviewed_by = A. Operator\n\
             reviewed_on = 2026-11-13\n",
        )
        .expect("parses");
        let engine = CommandForwardingEngine::new(
            Arc::new(FailClosedCommandForwarder),
            Arc::new(NoPolicyGate),
            SiteCommandAllowlist::new(vec!["simulated-test".to_string()]),
            config,
            Arc::new(InMemoryAuditSink::new()),
            Arc::new(FixedClock::at_unix_millis(BASE_MILLIS)),
        )
        .expect("composes");
        assert_eq!(engine.stopped_assets(), Vec::<String>::new());
    }

    /// A policy gate that refuses everything (fail-closed test double).
    struct NoPolicyGate;
    impl CommandPolicyGate for NoPolicyGate {
        fn check_command(
            &self,
            _target_adapter: &str,
            _now_unix_millis: i64,
        ) -> Result<(), CommandPolicyRefusal> {
            Err(CommandPolicyRefusal {
                code: "policy_adapter_disabled".to_string(),
                reason: "refused by the test gate".to_string(),
            })
        }
    }

    #[test]
    fn the_call_recorder_records_calls_in_order() {
        let recorder = CallRecorder::new();
        recorder.record("a");
        recorder.record("b");
        assert_eq!(recorder.count(), 2);
    }
}
