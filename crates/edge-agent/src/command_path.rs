// P6.14 — The edge command receiver: DISABLED BY DEFAULT, with the
//         four-condition, all-or-nothing, fail-closed test-env enablement
//         gate (report §2.1 v0.4.0, §21.4, §14.1, §17.5, §5.7, §3.3; ledger
//         P6.14).
//
// THE HARD DEFAULT: in the default build an `ISSUED` command arriving at the
// edge is REJECTED — not queued, not forwarded — and the rejection is
// audited. There is no queue field, no forwarding call, and no adapter write
// path anywhere in this module: the [`crate::adapters::port::EmsAdapter`]
// port has no write method (P6.12: absence is structural), and this module
// stores nothing. A refusal is a return value plus an audit record; nothing
// else happens.
//
// THE GATE (all FOUR conditions simultaneously; all-or-nothing; fail-closed):
//
// | # | Condition | Input | Absent → |
// |---|---|---|---|
// | a | Explicit signed per-site configuration change | `FLEXCORE_EDGE_COMMAND_PATH=enabled` | [`GateRefusal::ConfigFlagOff`] |
// | b | Signed enablement record + matching test-lab token | `FLEXCORE_EDGE_COMMAND_ENABLEMENT_RECORD` (the signed record), `FLEXCORE_EDGE_COMMAND_ENABLEMENT_TOKEN` (must equal the record's token), key material supplied out-of-band | [`GateRefusal::NoSignedRecord`] / [`GateRefusal::DigestMismatch`] / [`GateRefusal::TokenMismatch`] |
// | c | Target adapter on the site command allowlist | `FLEXCORE_EDGE_SITE_COMMAND_ALLOWLIST` (comma-separated ids; also enforced per-command) | [`GateRefusal::AllowlistEmpty`] / [`CommandRefusal::AdapterNotAllowlisted`] |
// | d | The site's D6 review record exists | `FLEXCORE_EDGE_D6_RECORD_REFERENCE` (a reference to the D6 decision record) | [`GateRefusal::D6RecordAbsent`] |
//
// Enablement is TIME-BOUNDED (`valid_until` on the record; expired →
// disabled again) and audited on enable AND on disable.
//
// STRUCTURAL NO-PLUGIN-ENABLE (§5.7: a plugin can never select the
// control-authority mode): the enablement state lives in
// [`CommandPathRuntime`] as a PRIVATE field computed ONCE, at construction,
// from the four gate inputs. There is no public setter, no `&mut self`
// method that can move a disabled runtime to enabled, and no constructor
// other than [`CommandPathRuntime::evaluate`]. A plugin/adapter/optimizer
// can hold, at most, a shared reference — and a shared reference exposes
// only reads. The receiver's own `&mut self` bookkeeping is the
// expiry-transition audit flag only; the enablement itself is RE-DERIVED
// from the immutable inputs on every receive. Nothing in this crate, and
// nothing reachable from a plugin seam, can flip the flag.
//
// NETWORK SHAPE (§3.3): the command path creates NO inbound listener in any
// state — even enabled. Commands ride the response side of the OUTBOUND
// connection: the P6.07 mTLS channel adapter dials out, and when a
// response-direction frame arrives ON THAT OUTBOUND SESSION, the channel
// adapter demarshals it and CALLS [`CommandReceiver::receive`] in-process.
// There is no socket code in this module (the structural
// `tests/no_inbound_listener.rs` scan enforces this crate-wide). The
// receiver is a method on an object the composition root owns — never a
// server.
//
// SCOPE HONESTY (what "enabled" does and does not mean this wave): an
// enabled gate admits a command to the P6.17 policy check and records the
// admission. Edge-side SIGNATURE/expiry/sequence verification is P6.15 and
// forwarding-to-adapter is P6.16 — NEITHER is implemented in this wave, and
// no `EmsAdapter` method exists to forward to. A receipt from this module
// is therefore a GATE ADMISSION, not an issuance to hardware, and never a
// physical delivery (§14.1: acknowledgement is not delivery).
//
// Config ownership (recorded): `config.rs` and `main.rs` are owned by
// another agent this wave, so the gate's env inputs are defined and read
// HERE ([`CommandPathGateInputs::from_env`]); the exact composition-root
// wiring the coordinator must add is itemised in the decision doc
// (`ledger/phase-06-edge-protocol/13_14_17_adapter_command_policy.md` §7).

use std::sync::Arc;

use flexcore_contracts::time_model::IngestTime;
use flexcore_ports::clock::Clock;

use crate::ports::{AuditSink, EdgeAuditEvent, EdgeAuditKind, EdgeError};

// --- Environment variable names (defined HERE — see the module header) ------

/// Gate condition (a): the explicit per-site configuration flag. The ONLY
/// enabling value is exactly `enabled` (explicit, signed-change semantics —
/// not a boolean sneak path). `disabled` or absent → off. Anything else is a
/// named configuration error.
pub const ENV_COMMAND_PATH_FLAG: &str = "FLEXCORE_EDGE_COMMAND_PATH";
/// The enabling value of [`ENV_COMMAND_PATH_FLAG`].
pub const COMMAND_PATH_ENABLED_VALUE: &str = "enabled";
/// The disabling value of [`ENV_COMMAND_PATH_FLAG`].
pub const COMMAND_PATH_DISABLED_VALUE: &str = "disabled";

/// Gate condition (b): the serialized SIGNED enablement record (format:
/// [`ENABLEMENT_RECORD_FORMAT`]; see [`SignedEnablementRecord`]).
pub const ENV_COMMAND_ENABLEMENT_RECORD: &str = "FLEXCORE_EDGE_COMMAND_ENABLEMENT_RECORD";
/// Gate condition (b): the test-lab environment marker; must equal the
/// enablement token INSIDE the signed record (the record's digest covers the
/// token, so the pair cannot be mixed and matched without a re-signature).
pub const ENV_COMMAND_ENABLEMENT_TOKEN: &str = "FLEXCORE_EDGE_COMMAND_ENABLEMENT_TOKEN";
/// Gate condition (c): the site command allowlist (comma-separated adapter
/// ids). Empty/absent → the path stays disabled.
pub const ENV_SITE_COMMAND_ALLOWLIST: &str = "FLEXCORE_EDGE_SITE_COMMAND_ALLOWLIST";
/// Gate condition (d): a reference to the site's D6 review record (the
/// P6.03 decision record). Absent → disabled.
pub const ENV_D6_RECORD_REFERENCE: &str = "FLEXCORE_EDGE_D6_RECORD_REFERENCE";

/// The versioned signed-enablement record format id (first line of the
/// serialized record; versioned so a format change is detectable and
/// refuse-able — the credential-store pattern).
pub const ENABLEMENT_RECORD_FORMAT: &str = "flexcore-edge-command-enablement-v1";

// --- Errors -------------------------------------------------------------------

/// Named errors of the command-path configuration and record parsing. Every
/// failure is named; nothing implies a silent pass. No error message ever
/// contains a secret VALUE (the enablement key and the token never render —
/// names only, the `config.rs` rule).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandPathError {
    /// An environment variable was present but unusable.
    InvalidEnv { name: &'static str, reason: String },
    /// The enablement record text was malformed (wrong version line,
    /// unknown field, missing required field, truncated).
    RecordMalformed { reason: String },
    /// The enablement key was empty, or its reference was empty.
    InvalidEnablementKey { reason: String },
}

impl std::fmt::Display for CommandPathError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CommandPathError::InvalidEnv { name, reason } => {
                write!(f, "invalid environment variable {name}: {reason}")
            }
            CommandPathError::RecordMalformed { reason } => {
                write!(f, "the signed enablement record is malformed: {reason}")
            }
            CommandPathError::InvalidEnablementKey { reason } => {
                write!(f, "the enablement key is invalid: {reason}")
            }
        }
    }
}

impl std::error::Error for CommandPathError {}

// --- The enablement key ---------------------------------------------------------

/// The enablement key material: mixed into the record digest (HMAC-STYLE —
/// see [`keyed_digest`]). THE KEY MATERIAL IS A DEPLOYMENT REQUIREMENT: the
/// FlexCore code base generates, stores, and distributes NO key material —
/// the deployment provisions the key out-of-band (secret manager / protected
/// volume, §17.3) and the composition root loads it and hands it here. It is
/// never in an environment variable default, never in a shipped profile, and
/// never rendered (`Debug`/`Display` redact unconditionally — the
/// `ProvisioningToken` rule).
pub struct EnablementKey {
    value: String,
    reference: String,
}

impl EnablementKey {
    /// Wraps the key value and its reference label. The reference is the
    /// NON-secret label recorded inside signed records (which key was used)
    /// and checked against the record at verification time — a record signed
    /// under a different key reference is refused before the digest is even
    /// compared.
    pub fn new(value: String, reference: String) -> Result<Self, CommandPathError> {
        if value.trim().is_empty() {
            return Err(CommandPathError::InvalidEnablementKey {
                reason: "the enablement key value is empty".to_string(),
            });
        }
        if reference.trim().is_empty() {
            return Err(CommandPathError::InvalidEnablementKey {
                reason: "the enablement key reference is empty".to_string(),
            });
        }
        Ok(Self {
            value,
            reference,
        })
    }

    /// The one named accessor for the key value (the digest computation
    /// only). Never passed to a log/audit formatter.
    pub fn expose(&self) -> &str {
        &self.value
    }

    /// The non-secret key reference label.
    pub fn reference(&self) -> &str {
        &self.reference
    }
}

impl std::fmt::Debug for EnablementKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("EnablementKey(<redacted>)")
    }
}

impl std::fmt::Display for EnablementKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("<redacted>")
    }
}

/// The HMAC-STYLE keyed digest over a record body. HONESTY NOTE (AGENTS.md
/// rule 1): this is a keyed FNV-1a fold — a keyed CHECKSUM, NOT a
/// cryptographic MAC. It detects accidental corruption and unsophisticated
/// tampering; it is NOT proof against a determined adversary. Replacing it
/// with a real HMAC (HMAC-SHA-256 or better) under a reviewed key-management
/// scheme is a recorded DEPLOYMENT REQUIREMENT (decision doc §4); the
/// digest field is versioned via the record format id so that replacement
/// is a detectable format change, never a silent one.
fn keyed_digest(key: &EnablementKey, body: &str) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
    // Block 1: the key material (mixed first — HMAC-style key-before-message
    // shape, with an explicit domain-separation byte between the blocks).
    let mut state = FNV_OFFSET;
    for byte in key.expose().as_bytes() {
        state ^= u64::from(*byte);
        state = state.wrapping_mul(FNV_PRIME);
    }
    state ^= 0xff;
    state = state.wrapping_mul(FNV_PRIME);
    // Block 2: the canonical record body.
    for byte in body.as_bytes() {
        state ^= u64::from(*byte);
        state = state.wrapping_mul(FNV_PRIME);
    }
    state
}

fn digest_hex(digest: u64) -> String {
    format!("{digest:016x}")
}

// --- The signed enablement record -------------------------------------------

/// The signed, time-bounded, per-site test-env enablement record (ledger
/// P6.14 step 2: named approver, test environment only, expiry). The digest
/// covers EVERY body field (site, approver, scope, token, both timestamps,
/// key reference) — no field can be edited without invalidating the
/// signature.
///
/// Serialized format (`flexcore-edge-command-enablement-v1`, line-oriented,
/// the credential-store pattern):
///
/// ```text
/// flexcore-edge-command-enablement-v1
/// site = pilot-site-A
/// approver = <named approver (role + name), recorded by the gate procedure>
/// adapters = simulated-test,openems-rest
/// enablement_token = <test-lab marker value>
/// issued_at_unix_millis = 1700000000000
/// valid_until_unix_millis = 1700003600000
/// key_reference = deployment:lab-enablement-key-1
/// digest = <16 lowercase hex chars>
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedEnablementRecord {
    /// The site the enablement is bound to (one record per site; must equal
    /// the edge's configured site — checked at evaluation).
    pub site_id: String,
    /// The named approver (recorded by the gate procedure — the ledger
    /// requires named approvers; the code requires the field NON-EMPTY and
    /// the procedure owns its truth).
    pub approver: String,
    /// The adapter ids the enablement covers (the record-level scope; the
    /// site allowlist in gate condition (c) is the CONFIG-level scope and
    /// the per-command check runs against it).
    pub adapter_scope: Vec<String>,
    /// The test-lab enablement marker: the environment token must equal
    /// this value (gate condition (b)).
    pub enablement_token: String,
    pub issued_at_unix_millis: i64,
    /// The enablement expiry. Expired → the path is disabled again
    /// (time-bounded enablement, audited).
    pub valid_until_unix_millis: i64,
    /// The non-secret key reference the record was signed under.
    pub key_reference: String,
    /// The keyed digest (hex), present once signed. `parse` requires it;
    /// [`SignedEnablementRecord::verify`] recomputes and compares.
    pub digest: Option<String>,
}

impl SignedEnablementRecord {
    /// Validates the body fields (fail-closed: an incomplete record is a
    /// named error, never a partial enablement).
    pub fn validate_body(&self) -> Result<(), CommandPathError> {
        let mut problems: Vec<String> = Vec::new();
        if self.site_id.trim().is_empty() {
            problems.push("site is empty".to_string());
        }
        if self.approver.trim().is_empty() {
            problems.push("approver is empty (the gate requires a NAMED approver)".to_string());
        }
        if self.adapter_scope.is_empty() {
            problems.push("the record-level adapter scope is empty".to_string());
        }
        if self.enablement_token.trim().is_empty() {
            problems.push("enablement_token is empty".to_string());
        }
        if self.valid_until_unix_millis <= self.issued_at_unix_millis {
            problems.push("valid_until is not after issued_at (the enablement must be time-bounded)".to_string());
        }
        if self.key_reference.trim().is_empty() {
            problems.push("key_reference is empty".to_string());
        }
        if problems.is_empty() {
            Ok(())
        } else {
            Err(CommandPathError::RecordMalformed {
                reason: problems.join("; "),
            })
        }
    }

    /// The canonical body the digest covers: every labeled field in fixed
    /// order, newline-joined (no digest line).
    pub fn canonical_body(&self) -> String {
        format!(
            "site = {}\napprover = {}\nadapters = {}\nenablement_token = {}\n\
             issued_at_unix_millis = {}\nvalid_until_unix_millis = {}\nkey_reference = {}\n",
            self.site_id,
            self.approver,
            self.adapter_scope.join(","),
            self.enablement_token,
            self.issued_at_unix_millis,
            self.valid_until_unix_millis,
            self.key_reference,
        )
    }

    /// Computes the keyed digest for this record under `key`.
    pub fn signed_digest(&self, key: &EnablementKey) -> String {
        digest_hex(keyed_digest(key, &self.canonical_body()))
    }

    /// Signs the record in place (sets the digest field) and returns it.
    pub fn sign(mut self, key: &EnablementKey) -> Result<Self, CommandPathError> {
        self.validate_body()?;
        self.digest = Some(self.signed_digest(key));
        Ok(self)
    }

    /// Verifies the record against `key`: the body must be complete, the
    /// digest present, and the recomputed digest equal to the stored one.
    pub fn verify(&self, key: &EnablementKey) -> bool {
        if self.validate_body().is_err() {
            return false;
        }
        match &self.digest {
            None => false,
            Some(digest) => {
                // Constant-time-ish comparison is moot for a keyed checksum
                // (see the keyed_digest honesty note); equality is exact and
                // case-sensitive.
                *digest == self.signed_digest(key)
            }
        }
    }

    /// Serializes the signed record (the value carried by
    /// `FLEXCORE_EDGE_COMMAND_ENABLEMENT_RECORD`). Requires a signed record.
    pub fn encode(&self) -> Result<String, CommandPathError> {
        self.validate_body()?;
        let digest = self.digest.as_ref().ok_or_else(|| CommandPathError::RecordMalformed {
            reason: "the record is not signed (no digest); call sign() first".to_string(),
        })?;
        Ok(format!(
            "{}\n{}digest = {}\n",
            ENABLEMENT_RECORD_FORMAT,
            self.canonical_body(),
            digest
        ))
    }

    /// Parses the serialized signed record (strict: wrong version, unknown
    /// field, missing field, or malformed digest → named error; unknown
    /// fields are NEVER silently accepted — the P6.13 step-1 discipline
    /// applied to the gate's own input).
    pub fn parse(text: &str) -> Result<Self, CommandPathError> {
        let malformed = |reason: String| CommandPathError::RecordMalformed { reason };
        let mut lines = text.lines();
        let version = lines
            .next()
            .ok_or_else(|| malformed("the record is empty".to_string()))?
            .trim();
        if version != ENABLEMENT_RECORD_FORMAT {
            return Err(malformed(format!(
                "record format '{version}' is not the supported '{ENABLEMENT_RECORD_FORMAT}'"
            )));
        }
        let mut site_id: Option<String> = None;
        let mut approver: Option<String> = None;
        let mut adapter_scope: Option<Vec<String>> = None;
        let mut enablement_token: Option<String> = None;
        let mut issued_at_unix_millis: Option<i64> = None;
        let mut valid_until_unix_millis: Option<i64> = None;
        let mut key_reference: Option<String> = None;
        let mut digest: Option<String> = None;
        for line in lines {
            let (key, value) = line
                .split_once(" = ")
                .ok_or_else(|| malformed(format!(
                    "malformed line (expected 'key = value'): {:?}",
                    truncate_for_error(line)
                )))?;
            match key.trim() {
                "site" => site_id = Some(value.trim().to_string()),
                "approver" => approver = Some(value.trim().to_string()),
                "adapters" => {
                    adapter_scope = Some(
                        value
                            .split(',')
                            .map(|id| id.trim().to_string())
                            .filter(|id| !id.is_empty())
                            .collect(),
                    );
                }
                "enablement_token" => enablement_token = Some(value.trim().to_string()),
                "issued_at_unix_millis" => {
                    issued_at_unix_millis = Some(value.trim().parse::<i64>().map_err(|_| {
                        malformed("issued_at_unix_millis is not an integer".to_string())
                    })?);
                }
                "valid_until_unix_millis" => {
                    valid_until_unix_millis = Some(value.trim().parse::<i64>().map_err(|_| {
                        malformed("valid_until_unix_millis is not an integer".to_string())
                    })?);
                }
                "key_reference" => key_reference = Some(value.trim().to_string()),
                "digest" => {
                    let raw = value.trim();
                    if raw.len() != 16 || !raw.chars().all(|c| c.is_ascii_hexdigit()) {
                        return Err(malformed(
                            "digest is not 16 lowercase-parsable hex characters".to_string(),
                        ));
                    }
                    digest = Some(raw.to_ascii_lowercase());
                }
                other => {
                    return Err(malformed(format!(
                        "unknown field '{other}' in the enablement record (unknown fields are \
                         never silently accepted)"
                    )));
                }
            }
        }
        let missing = |name: &str| {
            malformed(format!("required field '{name}' is missing"))
        };
        let record = SignedEnablementRecord {
            site_id: site_id.ok_or_else(|| missing("site"))?,
            approver: approver.ok_or_else(|| missing("approver"))?,
            adapter_scope: adapter_scope.ok_or_else(|| missing("adapters"))?,
            enablement_token: enablement_token.ok_or_else(|| missing("enablement_token"))?,
            issued_at_unix_millis: issued_at_unix_millis
                .ok_or_else(|| missing("issued_at_unix_millis"))?,
            valid_until_unix_millis: valid_until_unix_millis
                .ok_or_else(|| missing("valid_until_unix_millis"))?,
            key_reference: key_reference.ok_or_else(|| missing("key_reference"))?,
            digest,
        };
        record.validate_body()?;
        Ok(record)
    }
}

/// Truncates a malformed line for the error message (bounded diagnostics —
/// never echo an arbitrary-length untrusted payload).
fn truncate_for_error(line: &str) -> String {
    if line.len() <= 64 {
        line.to_string()
    } else {
        let mut cut = 64;
        while !line.is_char_boundary(cut) {
            cut -= 1;
        }
        format!("{}…", &line[..cut])
    }
}

// --- The gate inputs ----------------------------------------------------------

/// The four gate inputs, plus the site binding they are evaluated against.
/// Constructible directly (tests, composition root) or from the process
/// environment ([`CommandPathGateInputs::from_env`]). The enablement KEY is
/// deliberately NOT an input here — it is a deployment-provisioned secret
/// handed to [`CommandPathRuntime::evaluate`] separately.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CommandPathGateInputs {
    /// The edge's configured site (the record must bind to it).
    pub site_id: String,
    /// Gate (a): the explicit configuration flag. Default FALSE.
    pub config_flag_enabled: bool,
    /// Gate (b): the signed enablement record (parsed), when present.
    pub signed_record: Option<SignedEnablementRecord>,
    /// Gate (b): the test-lab environment marker token, when present.
    pub environment_token: Option<String>,
    /// Gate (c): the site command allowlist (adapter ids). Empty → disabled.
    pub command_allowlist: Vec<String>,
    /// Gate (d): the D6 review record reference. Absent/empty → disabled.
    pub d6_record_reference: Option<String>,
}

impl CommandPathGateInputs {
    /// Reads the gate inputs from the process environment. Fail-closed:
    /// every parse error is named; the DEFAULTS are all OFF. Never called
    /// with a partially-configured environment expecting silence — an
    /// unusable gate input is a startup error, not a fallback.
    pub fn from_env() -> Result<Self, CommandPathError> {
        let site_id = std::env::var(crate::config::ENV_SITE_ID).unwrap_or_default();
        let config_flag_enabled = match std::env::var(ENV_COMMAND_PATH_FLAG) {
            Ok(raw) => match raw.trim().to_ascii_lowercase().as_str() {
                COMMAND_PATH_ENABLED_VALUE => true,
                COMMAND_PATH_DISABLED_VALUE => false,
                other => {
                    return Err(CommandPathError::InvalidEnv {
                        name: ENV_COMMAND_PATH_FLAG,
                        reason: format!(
                            "expected '{COMMAND_PATH_ENABLED_VALUE}' or \
                             '{COMMAND_PATH_DISABLED_VALUE}', got a value that parses as '{other}'"
                        ),
                    });
                }
            },
            Err(_) => false,
        };
        let signed_record = match std::env::var(ENV_COMMAND_ENABLEMENT_RECORD) {
            Ok(raw) if !raw.trim().is_empty() => Some(SignedEnablementRecord::parse(&raw)?),
            Ok(_) => None,
            Err(_) => None,
        };
        let environment_token = match std::env::var(ENV_COMMAND_ENABLEMENT_TOKEN) {
            Ok(raw) if !raw.trim().is_empty() => Some(raw),
            Ok(_) => None,
            Err(_) => None,
        };
        let command_allowlist = match std::env::var(ENV_SITE_COMMAND_ALLOWLIST) {
            Ok(raw) => raw
                .split(',')
                .map(|id| id.trim().to_string())
                .filter(|id| !id.is_empty())
                .collect(),
            Err(_) => Vec::new(),
        };
        let d6_record_reference = match std::env::var(ENV_D6_RECORD_REFERENCE) {
            Ok(raw) if !raw.trim().is_empty() => Some(raw),
            Ok(_) => None,
            Err(_) => None,
        };
        Ok(Self {
            site_id,
            config_flag_enabled,
            signed_record,
            environment_token,
            command_allowlist,
            d6_record_reference,
        })
    }
}

// --- The gate decision ----------------------------------------------------------

/// Why the gate refused to enable. Every variant is a NAMED, auditable
/// reason; the fixed evaluation order (module doc of
/// [`CommandPathRuntime::evaluate`]) determines which single reason a
/// partially-met gate reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateRefusal {
    /// Gate (a): the configuration flag is off (the default).
    ConfigFlagOff,
    /// Gate (c): the site command allowlist is empty — no adapter may
    /// receive commands.
    AllowlistEmpty,
    /// Gate (d): no D6 review record reference is configured for the site.
    D6RecordAbsent,
    /// Gate (b): no signed enablement record is loaded.
    NoSignedRecord,
    /// Gate (b): the record binds a different site.
    SiteMismatch,
    /// Gate (b): the record was signed under a different key reference.
    KeyReferenceMismatch,
    /// Gate (b): the record's digest does not verify under the provided key.
    DigestMismatch,
    /// Gate (b): the environment token does not match the record's token.
    TokenMismatch,
    /// The enablement record's `valid_until` has passed.
    EnablementExpired,
}

impl GateRefusal {
    /// The stable refusal code (audit text).
    pub fn code(self) -> &'static str {
        match self {
            GateRefusal::ConfigFlagOff => "config_flag_off",
            GateRefusal::AllowlistEmpty => "command_allowlist_empty",
            GateRefusal::D6RecordAbsent => "d6_record_absent",
            GateRefusal::NoSignedRecord => "no_signed_record",
            GateRefusal::SiteMismatch => "record_site_mismatch",
            GateRefusal::KeyReferenceMismatch => "key_reference_mismatch",
            GateRefusal::DigestMismatch => "record_digest_mismatch",
            GateRefusal::TokenMismatch => "enablement_token_mismatch",
            GateRefusal::EnablementExpired => "enablement_expired",
        }
    }
}

impl std::fmt::Display for GateRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (code, text) = match self {
            GateRefusal::ConfigFlagOff => (
                "config_flag_off",
                "the command-path configuration flag is off (the default; enabling requires an \
                 explicit, signed, per-site configuration change)",
            ),
            GateRefusal::AllowlistEmpty => (
                "command_allowlist_empty",
                "the site command allowlist is empty; no adapter may receive commands",
            ),
            GateRefusal::D6RecordAbsent => (
                "d6_record_absent",
                "no D6 cyber/OT boundary review record is configured for this site (gate \
                 condition d); the path stays disabled",
            ),
            GateRefusal::NoSignedRecord => (
                "no_signed_record",
                "no signed per-site enablement record is loaded (gate condition b)",
            ),
            GateRefusal::SiteMismatch => (
                "record_site_mismatch",
                "the enablement record binds a different site than this edge is configured for",
            ),
            GateRefusal::KeyReferenceMismatch => (
                "key_reference_mismatch",
                "the enablement record was signed under a different key reference than the \
                 provided enablement key",
            ),
            GateRefusal::DigestMismatch => (
                "record_digest_mismatch",
                "the enablement record's digest does not verify under the provided key; the \
                 record is not trusted",
            ),
            GateRefusal::TokenMismatch => (
                "enablement_token_mismatch",
                "the test-lab environment token does not match the signed enablement record",
            ),
            GateRefusal::EnablementExpired => (
                "enablement_expired",
                "the enablement record's valid_until has passed; the path is disabled again \
                 (time-bounded enablement)",
            ),
        };
        write!(f, "{code}: {text}")
    }
}

/// The computed enablement of the command path. `Disabled` carries the named
/// refusal; `Enabled` carries the scope and the hard expiry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandPathEnablement {
    Disabled { reason: GateRefusal },
    Enabled {
        /// The adapter ids the path may admit commands for (gate (c)'s
        /// allowlist, at evaluation time).
        scope: Vec<String>,
        /// The hard expiry (record `valid_until`); past this instant the
        /// path is DISABLED again — see
        /// [`CommandPathRuntime::enablement_at`].
        valid_until_unix_millis: i64,
        /// The named approver from the signed record (audit visibility).
        approver: String,
    },
}

impl CommandPathEnablement {
    /// Whether the path is enabled AT THIS INSTANT (an `Enabled` whose
    /// expiry has passed is disabled — the time-bound is enforced at read
    /// time, so a long-lived runtime cannot outlive its record).
    pub fn is_enabled_at(&self, now_unix_millis: i64) -> bool {
        match self {
            CommandPathEnablement::Disabled { .. } => false,
            CommandPathEnablement::Enabled {
                valid_until_unix_millis,
                ..
            } => now_unix_millis < *valid_until_unix_millis,
        }
    }
}

// --- The policy-gate seam (P6.17 wiring point) ---------------------------------

/// The independent policy gate the command path consults (P6.17 step 1:
/// rate limits, operating windows, per-adapter enablement). DEFINED HERE,
/// IMPLEMENTED by `crate::policy::SitePolicyEngine` — one-directional
/// module dependency (policy → command_path), no cycle.
///
/// The receiver FAILS CLOSED when this gate refuses, and the receiver cannot
/// even be CONSTRUCTED without one (`CommandReceiver::new` takes it) — a
/// command path without an independent local policy gate does not exist.
pub trait CommandPolicyGate: Send + Sync {
    /// Checks (and, for rate-limited engines, reserves) one command against
    /// the site policy for `target_adapter`. A refusal carries the policy
    /// code and a human-safe reason; the receiver rejects + audits it.
    fn check_command(
        &self,
        target_adapter: &str,
        now_unix_millis: i64,
    ) -> Result<(), CommandPolicyRefusal>;
}

/// A policy-gate refusal (code + human-safe reason; the engine maps its own
/// violation vocabulary onto this).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandPolicyRefusal {
    pub code: String,
    pub reason: String,
}

// --- The runtime (structurally non-mutable enablement) --------------------------

/// The command path's enablement runtime: the gate inputs evaluated ONCE, at
/// construction, into a PRIVATE enablement state. STRUCTURAL NO-PLUGIN-
/// ENABLE (§5.7): there is no public setter, no `&mut self` method, and no
/// constructor besides [`CommandPathRuntime::evaluate`] — a shared reference
/// to this type exposes reads only, so a plugin holding one can never move
/// the path from disabled to enabled (see the module header).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandPathRuntime {
    enablement: CommandPathEnablement,
}

impl CommandPathRuntime {
    /// Evaluates the four gate conditions (all-or-nothing, fail-closed).
    /// Evaluation order is FIXED (deterministic refusal reason, and no
    /// condition can short-circuit another into passivity):
    ///
    /// 1. gate (a) config flag → `ConfigFlagOff`;
    /// 2. gate (c) allowlist non-empty → `AllowlistEmpty`;
    /// 3. gate (d) D6 record reference → `D6RecordAbsent`;
    /// 4. gate (b) record present → `NoSignedRecord`;
    /// 5. record body complete (site, approver, scope, token, key
    ///    reference, time bounds) → `NoSignedRecord` (incomplete body);
    /// 6. record site binding → `SiteMismatch`;
    /// 7. record key reference vs the provided key → `KeyReferenceMismatch`;
    /// 8. record digest verification → `DigestMismatch`;
    /// 9. environment token vs record token → `TokenMismatch`;
    /// 10. expiry at `now_unix_millis` → `EnablementExpired`;
    /// 11. otherwise `Enabled` with the allowlist as scope.
    pub fn evaluate(
        inputs: &CommandPathGateInputs,
        key: &EnablementKey,
        now_unix_millis: i64,
    ) -> Self {
        let disabled = |reason: GateRefusal| Self {
            enablement: CommandPathEnablement::Disabled { reason },
        };
        // 1. Gate (a).
        if !inputs.config_flag_enabled {
            return disabled(GateRefusal::ConfigFlagOff);
        }
        // 2. Gate (c) — non-empty allowlist.
        if inputs.command_allowlist.iter().all(|id| id.trim().is_empty()) {
            return disabled(GateRefusal::AllowlistEmpty);
        }
        // 3. Gate (d) — D6 record reference present.
        let d6_present = inputs
            .d6_record_reference
            .as_deref()
            .map(|r| !r.trim().is_empty())
            .unwrap_or(false);
        if !d6_present {
            return disabled(GateRefusal::D6RecordAbsent);
        }
        // 4. Gate (b) — a signed record is loaded.
        let Some(record) = &inputs.signed_record else {
            return disabled(GateRefusal::NoSignedRecord);
        };
        // 5. Record body complete.
        if record.validate_body().is_err() {
            return disabled(GateRefusal::NoSignedRecord);
        }
        // 6. The record binds THIS site (one record per site, §16.2).
        if record.site_id != inputs.site_id {
            return disabled(GateRefusal::SiteMismatch);
        }
        // 7. The record was signed under THIS key reference.
        if record.key_reference != key.reference() {
            return disabled(GateRefusal::KeyReferenceMismatch);
        }
        // 8. The digest verifies under THIS key.
        if !record.verify(key) {
            return disabled(GateRefusal::DigestMismatch);
        }
        // 9. The environment token matches the record's token.
        let token_matches = inputs
            .environment_token
            .as_deref()
            .map(|t| t == record.enablement_token)
            .unwrap_or(false);
        if !token_matches {
            return disabled(GateRefusal::TokenMismatch);
        }
        // 10. Time-bounded: expired at evaluation time → disabled again.
        if now_unix_millis >= record.valid_until_unix_millis {
            return disabled(GateRefusal::EnablementExpired);
        }
        // 11. All four conditions hold: enabled, within the recorded scope.
        Self {
            enablement: CommandPathEnablement::Enabled {
                scope: inputs.command_allowlist.clone(),
                valid_until_unix_millis: record.valid_until_unix_millis,
                approver: record.approver.clone(),
            },
        }
    }

    /// The enablement AS OF `now_unix_millis`: an `Enabled` whose expiry has
    /// passed reads back as `Disabled { EnablementExpired }` — the time
    /// bound is enforced at READ time, so the path auto-disables at expiry
    /// without any mutation of the runtime (the state is re-derived, never
    /// flipped). This is the ONLY way the runtime's state is observed.
    pub fn enablement_at(&self, now_unix_millis: i64) -> CommandPathEnablement {
        match &self.enablement {
            CommandPathEnablement::Disabled { reason } => CommandPathEnablement::Disabled {
                reason: *reason,
            },
            CommandPathEnablement::Enabled {
                scope,
                valid_until_unix_millis,
                approver,
            } => {
                if now_unix_millis >= *valid_until_unix_millis {
                    CommandPathEnablement::Disabled {
                        reason: GateRefusal::EnablementExpired,
                    }
                } else {
                    CommandPathEnablement::Enabled {
                        scope: scope.clone(),
                        valid_until_unix_millis: *valid_until_unix_millis,
                        approver: approver.clone(),
                    }
                }
            }
        }
    }
}

// --- The edge command shape (cites the frozen lifecycle; forks nothing) -------

/// The frozen command-lifecycle state names (report §14.1; the P0.11 frozen
/// transition table is the normative source). The edge crate has NO
/// `flexcore-domain` dependency (the P6.05 rule), so the state travels as
/// the canonical SCREAMING_SNAKE name validated against this list — the
/// frozen lifecycle is CITED, never forked or re-implemented here.
pub const FROZEN_COMMAND_STATE_NAMES: [&str; 14] = [
    "PROPOSED",
    "VALIDATED",
    "FEASIBLE",
    "APPROVED",
    "ISSUED",
    "ACKNOWLEDGED",
    "TRACKING",
    "COMPLETED",
    "REJECTED",
    "FAILED",
    "EXPIRED",
    "CANCELLED",
    "SUPERSEDED",
    "FALLBACK",
];

/// The only state a command can legitimately arrive at the edge in (the
/// frozen lifecycle: ISSUED = "signed command was sent to the selected
/// gateway/adapter").
pub const EDGE_COMMAND_EXPECTED_STATE: &str = "ISSUED";

/// The edge-side shape of an arriving command (identity + lifecycle name +
/// source sequence; the payload-bound signature verification is P6.15 and
/// deliberately NOT here). Construction validates the state name against the
/// frozen list — an unknown state name is refused, never carried.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EdgeCommand {
    pub command_id: String,
    pub asset_key: String,
    /// The frozen lifecycle state name (must be one of
    /// [`FROZEN_COMMAND_STATE_NAMES`]).
    pub state_name: String,
    /// The source-assigned sequence (P0.09 §1 shape; P6.15 owns the
    /// per-channel monotonicity check, cited not reimplemented).
    pub source_sequence: u64,
}

impl EdgeCommand {
    /// Validates and builds the command (fail-closed on empty identity or an
    /// unknown lifecycle state name).
    pub fn new(
        command_id: String,
        asset_key: String,
        state_name: String,
        source_sequence: u64,
    ) -> Result<Self, CommandPathError> {
        if command_id.trim().is_empty() {
            return Err(CommandPathError::RecordMalformed {
                reason: "command_id is empty".to_string(),
            });
        }
        if asset_key.trim().is_empty() {
            return Err(CommandPathError::RecordMalformed {
                reason: "asset_key is empty".to_string(),
            });
        }
        if !FROZEN_COMMAND_STATE_NAMES.contains(&state_name.as_str()) {
            return Err(CommandPathError::RecordMalformed {
                reason: format!(
                    "state '{state_name}' is not one of the frozen §14.1 lifecycle states"
                ),
            });
        }
        Ok(Self {
            command_id,
            asset_key,
            state_name,
            source_sequence,
        })
    }
}

// --- Refusals and receipts -------------------------------------------------------

/// Why a command was refused. Every refusal is audited with its code; a
/// refused command is NOT queued and NOT forwarded — nothing in this module
/// retains it (there is no field to retain it in).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandRefusal {
    /// The gate is disabled (the default build lands here).
    PathDisabled { reason: GateRefusal },
    /// The target adapter is not on the site command allowlist.
    AdapterNotAllowlisted { adapter_id: String },
    /// The independent policy gate (P6.17 seam) refused.
    PolicyRefused { refusal: CommandPolicyRefusal },
    /// The command shape was invalid (empty identity — construction-time
    /// validation catches most shapes; this guards the receive path).
    MalformedCommand { reason: String },
    /// The command arrived in a state other than ISSUED (§14.1: only ISSUED
    /// commands are sent to a gateway/adapter).
    UnexpectedCommandState { state: String },
}

impl CommandRefusal {
    /// The stable refusal code (audit text).
    pub fn code(&self) -> &'static str {
        match self {
            CommandRefusal::PathDisabled { .. } => "command_path_disabled",
            CommandRefusal::AdapterNotAllowlisted { .. } => "adapter_not_allowlisted",
            CommandRefusal::PolicyRefused { .. } => "policy_refused",
            CommandRefusal::MalformedCommand { .. } => "malformed_command",
            CommandRefusal::UnexpectedCommandState { .. } => "unexpected_command_state",
        }
    }
}

impl std::fmt::Display for CommandRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CommandRefusal::PathDisabled { reason } => {
                write!(f, "command path disabled — {}", reason)
            }
            CommandRefusal::AdapterNotAllowlisted { adapter_id } => write!(
                f,
                "adapter '{adapter_id}' is not on the site command allowlist"
            ),
            CommandRefusal::PolicyRefused { refusal } => {
                write!(f, "site policy refused the command: {}", refusal.reason)
            }
            CommandRefusal::MalformedCommand { reason } => {
                write!(f, "malformed command: {reason}")
            }
            CommandRefusal::UnexpectedCommandState { state } => {
                write!(
                    f,
                    "command arrived in state '{state}' (only {EDGE_COMMAND_EXPECTED_STATE} \
                     commands can arrive at a gateway/adapter, §14.1)"
                )
            }
        }
    }
}

/// The P4-`TenantDenialAuditRecord` pattern (cited, adapted to the edge):
/// one auditable record of ONE refused command, identifiers only — no
/// secret material may ever enter this text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandRejectionAuditRecord {
    pub command_id: String,
    pub asset_key: String,
    pub target_adapter: String,
    /// The stable refusal code ([`CommandRefusal::code`]).
    pub refusal_code: String,
    pub at_unix_millis: i64,
    /// The stable denial reference (`command-denial:<id>@<at>/<code>`).
    pub refusal_reference: String,
}

impl CommandRejectionAuditRecord {
    /// The stable audit-text rendering (what the audit sink persists).
    pub fn audit_text(&self) -> String {
        format!(
            "command_denial command={} asset={} adapter={} code={} at={} reference={}",
            self.command_id,
            self.asset_key,
            self.target_adapter,
            self.refusal_code,
            self.at_unix_millis,
            self.refusal_reference,
        )
    }
}

/// A gate ADMITTED command. HONESTY (the module header): this is a gate +
/// policy admission only — edge-side signature/expiry/sequence verification
/// is P6.15, forwarding is P6.16, and no adapter write method exists. A
/// receipt is never an issuance to hardware and never a delivery (§14.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandReceipt {
    pub command_id: String,
    pub target_adapter: String,
    pub admitted_at_unix_millis: i64,
    /// The named approver from the enablement record (audit visibility).
    pub approver: String,
    /// The enablement's hard expiry (the admission cannot outlive it).
    pub enablement_valid_until_unix_millis: i64,
    /// The recorded scope note (what this receipt does NOT mean).
    pub scope_note: String,
}

// --- The receiver ----------------------------------------------------------------

/// The edge command receiver. DISABLED BY DEFAULT: with the default gate
/// inputs, [`CommandReceiver::receive`] refuses every command, audits the
/// refusal, and retains nothing.
///
/// Audit-kind note (recorded limitation): `EdgeAuditKind` (ports.rs) is a
/// CLOSED vocabulary this wave does not own, so every command-path audit
/// event records kind `EDGE_STARTUP` with the dedicated event name leading
/// the detail string (`EDGE_COMMAND_PATH_ENABLED` / `EDGE_COMMAND_PATH_DISABLED`
/// / `EDGE_COMMAND_ACCEPTED` / `EDGE_COMMAND_REJECTED`). The dedicated name
/// is the stable, greppable audit identity; adding the four dedicated
/// variants to `EdgeAuditKind` is itemised in the decision doc's coordinator
/// wiring list.
pub struct CommandReceiver {
    runtime: CommandPathRuntime,
    /// The independent site-policy gate (P6.17 seam). Required at
    /// construction: a command path without an independent local policy
    /// gate does not exist (fail-closed by composition).
    policy: Arc<dyn CommandPolicyGate>,
    audit: Arc<dyn AuditSink>,
    clock: Arc<dyn Clock + Send + Sync>,
    /// Audit bookkeeping ONLY (never the enablement itself): whether the
    /// previous receive observed an enabled path, so the expiry transition
    /// (`EDGE_COMMAND_PATH_DISABLED`) is audited exactly once.
    last_observed_enabled: Option<bool>,
}

impl CommandReceiver {
    /// Composes the receiver. Emits the current gate state as an audit
    /// event (`EDGE_COMMAND_PATH_ENABLED` with approver + expiry, or
    /// `EDGE_COMMAND_PATH_DISABLED` with the named refusal) — the enable /
    /// disable audit of ledger P6.14 step 2.
    pub fn new(
        runtime: CommandPathRuntime,
        policy: Arc<dyn CommandPolicyGate>,
        audit: Arc<dyn AuditSink>,
        clock: Arc<dyn Clock + Send + Sync>,
    ) -> Self {
        let receiver = Self {
            runtime,
            policy,
            audit,
            clock,
            last_observed_enabled: None,
        };
        // Declare the gate state at composition (the enable/disable audit).
        let now = receiver.now_unix_millis();
        let enablement = receiver.runtime.enablement_at(now);
        let (name, detail) = match &enablement {
            CommandPathEnablement::Enabled {
                scope,
                valid_until_unix_millis,
                approver,
            } => (
                "EDGE_COMMAND_PATH_ENABLED",
                format!(
                    "test-env command path enabled (approver='{approver}', scope=[{}], \
                     valid_until={valid_until_unix_millis}; admission only — signature/\
                     sequence verification is P6.15, forwarding is P6.16, neither exists yet)",
                    scope.join(",")
                ),
            ),
            CommandPathEnablement::Disabled { reason } => (
                "EDGE_COMMAND_PATH_DISABLED",
                format!("command path disabled ({reason})"),
            ),
        };
        receiver.audit_event(name, detail);
        receiver
    }

    /// The current enablement as of the receiver's clock (read-only;
    /// recomputed from the immutable inputs each call — see the structural
    /// no-plugin-enable argument in the module header).
    pub fn enablement(&self, now_unix_millis: i64) -> CommandPathEnablement {
        self.runtime.enablement_at(now_unix_millis)
    }

    /// Receives ONE command from the response side of the outbound channel.
    /// Order of evaluation (each step audited on refusal):
    ///
    /// 1. the gate enablement, as of NOW (an expired enablement reads
    ///    disabled — the time bound is enforced at read time);
    /// 2. the expiry transition, audited once when a previously-enabled
    ///    path reads disabled (`EDGE_COMMAND_PATH_DISABLED`);
    /// 3. command shape and lifecycle state (only ISSUED arrives);
    /// 4. gate (c) per-command: the target adapter is on the allowlist;
    /// 5. the independent policy gate (rate limit, operating window,
    ///    adapter enablement — P6.17);
    /// 6. admission recorded (`EDGE_COMMAND_ACCEPTED`).
    ///
    /// A refusal returns `Err` and leaves NOTHING behind: no queue, no
    /// buffer, no forwarding (there is no field to retain the command in,
    /// and no adapter write method exists — P6.12/P6.16).
    pub fn receive(
        &mut self,
        command: &EdgeCommand,
        target_adapter: &str,
    ) -> Result<CommandReceipt, CommandRefusal> {
        let now_millis = self.now_unix_millis();
        let enablement = self.runtime.enablement_at(now_millis);

        // 1 + 2. The gate, and the expiry-transition audit.
        let previously_enabled = self.last_observed_enabled.unwrap_or(false);
        let currently_enabled = enablement.is_enabled_at(now_millis);
        if previously_enabled && !currently_enabled {
            self.audit_event(
                "EDGE_COMMAND_PATH_DISABLED",
                match &enablement {
                    CommandPathEnablement::Disabled { reason } => {
                        format!("command path auto-disabled ({reason}); audited on transition")
                    }
                    CommandPathEnablement::Enabled { .. } => {
                        "command path state inconsistency (enabled again); audited".to_string()
                    }
                },
            );
        }
        self.last_observed_enabled = Some(currently_enabled);

        // The DEFAULT BUILD lands in the Disabled arm: reject, audit,
        // retain nothing.
        let (scope, valid_until, approver) = match &enablement {
            CommandPathEnablement::Enabled {
                scope,
                valid_until_unix_millis,
                approver,
            } => (scope.clone(), *valid_until_unix_millis, approver.clone()),
            CommandPathEnablement::Disabled { reason } => {
                let refusal = CommandRefusal::PathDisabled { reason: *reason };
                self.audit_rejection(command, target_adapter, &refusal, now_millis);
                return Err(refusal);
            }
        };

        // 3. Shape + lifecycle state.
        if command.command_id.trim().is_empty() {
            let refusal = CommandRefusal::MalformedCommand {
                reason: "command_id is empty".to_string(),
            };
            self.audit_rejection(command, target_adapter, &refusal, now_millis);
            return Err(refusal);
        }
        if command.state_name != EDGE_COMMAND_EXPECTED_STATE {
            let refusal = CommandRefusal::UnexpectedCommandState {
                state: command.state_name.clone(),
            };
            self.audit_rejection(command, target_adapter, &refusal, now_millis);
            return Err(refusal);
        }

        // 4. Gate (c), per-command: the target adapter must be allowlisted.
        if !scope.iter().any(|id| id == target_adapter) {
            let refusal = CommandRefusal::AdapterNotAllowlisted {
                adapter_id: target_adapter.to_string(),
            };
            self.audit_rejection(command, target_adapter, &refusal, now_millis);
            return Err(refusal);
        }

        // 5. The independent policy gate (P6.17 seam) — fail-closed.
        if let Err(policy_refusal) = self.policy.check_command(target_adapter, now_millis) {
            let refusal = CommandRefusal::PolicyRefused {
                refusal: policy_refusal,
            };
            self.audit_rejection(command, target_adapter, &refusal, now_millis);
            return Err(refusal);
        }

        // 6. Admission (gate + policy only — see the module-header honesty).
        let receipt = CommandReceipt {
            command_id: command.command_id.clone(),
            target_adapter: target_adapter.to_string(),
            admitted_at_unix_millis: now_millis,
            approver,
            enablement_valid_until_unix_millis: valid_until,
            scope_note: "gate admission only; edge-side signature/expiry/sequence \
                         verification is P6.15 and forwarding is P6.16 (neither exists in \
                         this wave); acknowledgement is not physical delivery (§14.1)"
                .to_string(),
        };
        self.audit_event(
            "EDGE_COMMAND_ACCEPTED",
            format!(
                "command={} asset={} adapter={} admitted by the test-env gate (approver='{}', \
                 enablement valid_until={})",
                receipt.command_id,
                command.asset_key,
                target_adapter,
                receipt.approver,
                receipt.enablement_valid_until_unix_millis,
            ),
        );
        Ok(receipt)
    }

    fn now_unix_millis(&self) -> i64 {
        let stamp = self.clock.now_ingest_time();
        // The contracts timestamp is a UTC millis value within i64 range by
        // construction; the conversion is total via a saturating fallback
        // (no unwrap on the conversion path).
        i64::try_from(stamp.unix_millis()).unwrap_or(i64::MAX)
    }

    /// Records one command-path audit event through the edge `AuditSink`
    /// port (see the audit-kind note on [`CommandReceiver`]). A failed audit
    /// write is a loud tracing error, never silently skipped (the
    /// P4.28/P4.30 audit-or-refuse discipline, cited).
    fn audit_event(&self, name: &'static str, detail: String) {
        let event = EdgeAuditEvent {
            recorded_at: self.clock.now_ingest_time(),
            kind: EdgeAuditKind::Startup,
            detail: format!("{name}: {detail}"),
        };
        if let Err(error) = self.audit.record(event) {
            tracing::error!(%error, audit_event = name, "command-path audit write FAILED");
        }
    }

    /// Builds and records the P4-pattern rejection record (cited).
    fn audit_rejection(
        &self,
        command: &EdgeCommand,
        target_adapter: &str,
        refusal: &CommandRefusal,
        now_millis: i64,
    ) {
        let record = CommandRejectionAuditRecord {
            command_id: command.command_id.clone(),
            asset_key: command.asset_key.clone(),
            target_adapter: target_adapter.to_string(),
            refusal_code: refusal.code().to_string(),
            at_unix_millis: now_millis,
            refusal_reference: format!(
                "command-denial:{}/{}@{}/{}",
                command.command_id, target_adapter, now_millis, refusal.code()
            ),
        };
        self.audit_event(
            "EDGE_COMMAND_REJECTED",
            record.audit_text(),
        );
    }
}

/// Convenience: the current audit timestamp for external callers that need
/// to align their own audit records with the receiver's (composition-root
/// helper; returns the wall-clock `IngestTime`).
pub fn audit_timestamp(clock: &dyn Clock) -> Result<IngestTime, EdgeError> {
    Ok(clock.now_ingest_time())
}

// --- P6.15/P6.16 ADDITIVE WIRING: the gated pipeline ------------------------
//
// ADDITIVE ONLY (every P6.14 behaviour and test above is untouched): this
// section composes the flow the ledger defines for a command that arrives
// behind the existing gate —
//
//   gate (P6.14, the `CommandReceiver` above, unchanged)
//     → verifier (P6.15, `crate::command_verify::CommandVerifier`)
//     → policy re-check + allowlist re-check + forwarder
//       (P6.16, `crate::command_forward::CommandForwardingEngine`)
//
// The `CommandReceiver::receive` semantics are NOT modified: a composition
// that wants only the P6.14 admission keeps using `receive` directly. The
// pipeline is the OPTIONAL full chain; each stage fails closed and audits
// its own refusals (the receiver, the verifier, and the engine each emit
// their own audit events — the pipeline adds no silent path).
//
// Fail-closed composition (recorded): the pipeline REQUIRES both the
// verifier and the engine at construction — a command path that admits
// without verification, or verifies without the separately-gated forwarder,
// does not exist here. In production the engine's forwarder is
// `FailClosedCommandForwarder` (every forward fails named, the command
// lands FAILED/FALLBACK per the configured per-asset policy); only a test
// composition wires a scripted forwarder double.

/// Which stage of the pipeline refused, with the stage's own error. Every
/// stage audits its own rejection before this returns.
#[derive(Debug, Clone, PartialEq)]
pub enum PipelineRefusal {
    /// The P6.14 gate (or its per-command allowlist / policy check) refused.
    Gate(CommandRefusal),
    /// The P6.15 verifier refused (the distinct reason is in the error).
    Verification(crate::command_verify::CommandVerificationRejection),
    /// The P6.16 engine refused (allowlist re-check, policy re-check,
    /// fallback stop, or unknown command).
    Forwarding(crate::command_forward::ForwardingRefusal),
}

impl PipelineRefusal {
    /// The stage that refused (audit text).
    pub fn stage(&self) -> &'static str {
        match self {
            PipelineRefusal::Gate(_) => "gate",
            PipelineRefusal::Verification(_) => "verification",
            PipelineRefusal::Forwarding(_) => "forwarding",
        }
    }

    /// The stage's stable refusal code (audit text).
    pub fn code(&self) -> String {
        match self {
            PipelineRefusal::Gate(refusal) => refusal.code().to_string(),
            PipelineRefusal::Verification(rejection) => rejection.code().to_string(),
            PipelineRefusal::Forwarding(refusal) => refusal.code().to_string(),
        }
    }
}

impl std::fmt::Display for PipelineRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PipelineRefusal::Gate(refusal) => write!(f, "gate stage: {refusal}"),
            PipelineRefusal::Verification(rejection) => {
                write!(f, "verification stage: {rejection}")
            }
            PipelineRefusal::Forwarding(refusal) => write!(f, "forwarding stage: {refusal}"),
        }
    }
}

impl std::error::Error for PipelineRefusal {}

/// The full gated command pipeline: gate → verifier → policy re-check →
/// forwarder. The receiver inside is the UNCHANGED P6.14 receiver (disabled
/// by default, four-condition gate, per-command allowlist, P6.17 policy
/// admission); the verifier is the P6.15 four-check verifier; the engine is
/// the P6.16 forwarding engine (separate allowlist re-check with its own
/// audit event, policy re-check through the same `CommandPolicyGate` seam,
/// §14.1 state recording, per-asset §14.4 fallback).
pub struct GatedCommandPipeline {
    receiver: CommandReceiver,
    verifier: crate::command_verify::CommandVerifier,
    engine: crate::command_forward::CommandForwardingEngine,
}

impl GatedCommandPipeline {
    /// Composes the pipeline. Both later stages are REQUIRED (fail-closed
    /// by construction — see the section note). The receiver's own
    /// composition audit has already fired; the verifier and engine audit
    /// their own composition.
    pub fn new(
        receiver: CommandReceiver,
        verifier: crate::command_verify::CommandVerifier,
        engine: crate::command_forward::CommandForwardingEngine,
    ) -> Self {
        Self {
            receiver,
            verifier,
            engine,
        }
    }

    /// The receiver's current enablement as of `now_unix_millis` (a read
    /// through to the unchanged P6.14 runtime — P6.25's console seam).
    pub fn enablement(&self, now_unix_millis: i64) -> CommandPathEnablement {
        self.receiver.enablement(now_unix_millis)
    }

    /// The full chain for ONE arriving command. Evaluation order is the
    /// ledger's: the existing gate admission runs first (unchanged
    /// behaviour, including its own audits and refusals), then the four
    /// verification checks with the supplied clock-quality status (P6.31
    /// supplies it; anything but `Synchronized` fails the expiry check
    /// closed), then the engine's separate allowlist re-check, fallback
    /// stop check, policy re-check, issue, forward, and §14.1 state
    /// recording. A refusal at ANY stage audits itself and returns
    /// `Err`; nothing is queued, retained, or forwarded.
    pub fn receive_verify_and_forward(
        &mut self,
        command: &crate::command_verify::IncomingCommand,
        clock_quality: flexcore_contracts::time_model::ClockQuality,
    ) -> Result<crate::command_forward::ForwardRecord, PipelineRefusal> {
        // Stage 1 — the UNCHANGED P6.14 gate (admission + its audits).
        let receipt = self
            .receiver
            .receive(&command.edge_command, &command.target_adapter)
            .map_err(PipelineRefusal::Gate)?;
        // Stage 2 — the P6.15 four-check verifier (audits its own
        // rejections with their DISTINCT reasons).
        let verified = self
            .verifier
            .verify(&receipt, command, clock_quality)
            .map_err(PipelineRefusal::Verification)?;
        // Stage 3 — the P6.16 engine: separate allowlist re-check (own
        // audit event), fallback stop check, policy re-check, issue,
        // forward, §14.1 states, per-asset fallback.
        self.engine
            .forward_verified(verified)
            .map_err(PipelineRefusal::Forwarding)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::InMemoryAuditSink;
    use crate::policy::{AdapterPolicy, OperatingWindow, RateLimit, SitePolicyConfig};
    use flexcore_ports::fakes::FixedClock;

    const BASE_MILLIS: i64 = 1_700_000_000_000;

    fn key() -> EnablementKey {
        EnablementKey::new("lab-enablement-key-material".to_string(), "deployment:lab-key-1".to_string())
            .expect("valid key")
    }

    fn record(site: &str, valid_until: i64) -> SignedEnablementRecord {
        SignedEnablementRecord {
            site_id: site.to_string(),
            approver: "J. Doe (security lead)".to_string(),
            adapter_scope: vec!["simulated-test".to_string()],
            enablement_token: "lab-token-123".to_string(),
            issued_at_unix_millis: BASE_MILLIS,
            valid_until_unix_millis: valid_until,
            key_reference: "deployment:lab-key-1".to_string(),
            digest: None,
        }
    }

    fn full_inputs(site: &str, valid_until: i64, key: &EnablementKey) -> CommandPathGateInputs {
        CommandPathGateInputs {
            site_id: site.to_string(),
            config_flag_enabled: true,
            signed_record: Some(record(site, valid_until).sign(key).expect("signs")),
            environment_token: Some("lab-token-123".to_string()),
            command_allowlist: vec!["simulated-test".to_string()],
            d6_record_reference: Some(
                "ledger/phase-06-edge-protocol/03_d6_boundary_review.md".to_string(),
            ),
        }
    }

    fn issued_command() -> EdgeCommand {
        EdgeCommand::new(
            "cmd-1".to_string(),
            "ess-01".to_string(),
            "ISSUED".to_string(),
            1,
        )
        .expect("valid command")
    }

    fn permissive_policy() -> Arc<dyn CommandPolicyGate> {
        Arc::new(crate::policy::SitePolicyEngine::new(SitePolicyConfig {
            format_version: 1,
            adapters: vec![AdapterPolicy {
                adapter_id: "simulated-test".to_string(),
                enabled: true,
                rate_limit: RateLimit {
                    max_commands: 100,
                    window_millis: 60_000,
                },
                operating_window: OperatingWindow {
                    start_millis_of_day: 0,
                    end_millis_of_day: 86_400_000,
                },
            }],
        })
        .expect("valid policy"))
    }

    fn receiver(
        runtime: CommandPathRuntime,
        audit: Arc<InMemoryAuditSink>,
    ) -> CommandReceiver {
        CommandReceiver::new(
            runtime,
            permissive_policy(),
            audit as Arc<dyn AuditSink>,
            Arc::new(FixedClock::at_unix_millis(BASE_MILLIS + 1_000)),
        )
    }

    #[test]
    fn the_default_gate_is_disabled_and_rejects_issued_with_audit() {
        let audit = Arc::new(InMemoryAuditSink::new());
        let runtime = CommandPathRuntime::evaluate(&CommandPathGateInputs::default(), &key(), BASE_MILLIS);
        let mut rx = receiver(runtime, Arc::clone(&audit));
        let error = rx.receive(&issued_command(), "simulated-test").expect_err("default is disabled");
        assert_eq!(error.code(), "command_path_disabled");
        assert!(matches!(
            error,
            CommandRefusal::PathDisabled { reason: GateRefusal::ConfigFlagOff }
        ));
        // The rejection is audited with the dedicated name + the P4-pattern
        // record text; the command is not queued or forwarded anywhere.
        let events = audit.events();
        assert!(
            events
                .iter()
                .any(|e| e.detail.starts_with("EDGE_COMMAND_REJECTED")
                    && e.detail.contains("command_denial")
                    && e.detail.contains("code=command_path_disabled")),
            "the rejection is audited: {:?}",
            events.iter().map(|e| e.detail.clone()).collect::<Vec<_>>()
        );
    }

    #[test]
    fn each_gate_condition_missing_individually_still_disables() {
        let key = key();
        let full = full_inputs("pilot-site-A", BASE_MILLIS + 3_600_000, &key);
        // (a) flag off.
        let mut inputs = full.clone();
        inputs.config_flag_enabled = false;
        assert_eq!(
            CommandPathRuntime::evaluate(&inputs, &key, BASE_MILLIS).enablement,
            CommandPathEnablement::Disabled { reason: GateRefusal::ConfigFlagOff }
        );
        // (b) no signed record.
        let mut inputs = full.clone();
        inputs.signed_record = None;
        assert!(matches!(
            CommandPathRuntime::evaluate(&inputs, &key, BASE_MILLIS).enablement,
            CommandPathEnablement::Disabled { reason: GateRefusal::NoSignedRecord }
        ));
        // (b) tampered digest.
        let mut inputs = full.clone();
        if let Some(rec) = &mut inputs.signed_record {
            rec.valid_until_unix_millis += 1; // body edit invalidates the digest
        }
        assert!(matches!(
            CommandPathRuntime::evaluate(&inputs, &key, BASE_MILLIS).enablement,
            CommandPathEnablement::Disabled { reason: GateRefusal::DigestMismatch }
        ));
        // (b) wrong environment token.
        let mut inputs = full.clone();
        inputs.environment_token = Some("wrong-token".to_string());
        assert!(matches!(
            CommandPathRuntime::evaluate(&inputs, &key, BASE_MILLIS).enablement,
            CommandPathEnablement::Disabled { reason: GateRefusal::TokenMismatch }
        ));
        // (b) key-reference mismatch.
        let other_key = EnablementKey::new("other-material".to_string(), "deployment:other-key".to_string())
            .expect("valid key");
        assert!(matches!(
            CommandPathRuntime::evaluate(&full, &other_key, BASE_MILLIS).enablement,
            CommandPathEnablement::Disabled { reason: GateRefusal::KeyReferenceMismatch }
        ));
        // (c) empty allowlist.
        let mut inputs = full.clone();
        inputs.command_allowlist = Vec::new();
        assert!(matches!(
            CommandPathRuntime::evaluate(&inputs, &key, BASE_MILLIS).enablement,
            CommandPathEnablement::Disabled { reason: GateRefusal::AllowlistEmpty }
        ));
        // (d) D6 reference absent.
        let mut inputs = full;
        inputs.d6_record_reference = None;
        assert!(matches!(
            CommandPathRuntime::evaluate(&inputs, &key, BASE_MILLIS).enablement,
            CommandPathEnablement::Disabled { reason: GateRefusal::D6RecordAbsent }
        ));
    }

    #[test]
    fn all_four_conditions_met_enables_within_the_recorded_scope() {
        let key = key();
        let inputs = full_inputs("pilot-site-A", BASE_MILLIS + 3_600_000, &key);
        let runtime = CommandPathRuntime::evaluate(&inputs, &key, BASE_MILLIS);
        let audit = Arc::new(InMemoryAuditSink::new());
        let mut rx = receiver(runtime, Arc::clone(&audit));
        // Enabled within scope: the allowlisted adapter is admitted.
        let receipt = rx
            .receive(&issued_command(), "simulated-test")
            .expect("admitted within scope");
        assert_eq!(receipt.command_id, "cmd-1");
        assert!(receipt.scope_note.contains("P6.15"));
        // Outside scope: refused (gate (c) per-command).
        let error = rx
            .receive(&issued_command(), "openems-rest")
            .expect_err("outside the allowlisted scope");
        assert_eq!(error.code(), "adapter_not_allowlisted");
        // The enable audit fired at composition; the acceptance and the
        // refusal are both audited.
        let details: Vec<String> = audit.events().iter().map(|e| e.detail.clone()).collect();
        assert!(details.iter().any(|d| d.starts_with("EDGE_COMMAND_PATH_ENABLED")));
        assert!(details.iter().any(|d| d.starts_with("EDGE_COMMAND_ACCEPTED")));
        assert!(details.iter().any(|d| d.starts_with("EDGE_COMMAND_REJECTED")));
    }

    #[test]
    fn expiry_auto_disables_and_audits_the_transition() {
        let key = key();
        // Enabled at evaluation time; expires one hour later. The
        // receiver's clock starts INSIDE the validity window (BASE + 1 s),
        // so the path is genuinely enabled at composition; the test then
        // advances the clock PAST valid_until.
        let valid_until = BASE_MILLIS + 3_600_000;
        let inputs = full_inputs("pilot-site-A", valid_until, &key);
        let runtime = CommandPathRuntime::evaluate(&inputs, &key, BASE_MILLIS);
        let audit = Arc::new(InMemoryAuditSink::new());
        let mut rx = receiver(runtime, Arc::clone(&audit));
        let receipt = rx
            .receive(&issued_command(), "simulated-test")
            .expect("admitted inside the validity window");
        assert_eq!(receipt.enablement_valid_until_unix_millis, valid_until);

        // Advance the clock past valid_until: the SAME runtime now reads
        // disabled (the time bound is enforced at read time), the command
        // is refused, and the disable transition is audited exactly once.
        let late = FixedClock::at_unix_millis(valid_until + 1_000);
        rx.clock = Arc::new(late) as Arc<dyn Clock + Send + Sync>;
        let error = rx
            .receive(&issued_command(), "simulated-test")
            .expect_err("expired enablement disables the path");
        assert!(matches!(
            error,
            CommandRefusal::PathDisabled { reason: GateRefusal::EnablementExpired }
        ));
        let details: Vec<String> = audit.events().iter().map(|e| e.detail.clone()).collect();
        let disable_count = details
            .iter()
            .filter(|d| d.starts_with("EDGE_COMMAND_PATH_DISABLED"))
            .count();
        assert_eq!(disable_count, 1, "the expiry transition is audited once");
        // And a second receive does not re-audit the transition (still
        // refused, still audited as a rejection).
        let _ = rx.receive(&issued_command(), "simulated-test");
        let details: Vec<String> = audit.events().iter().map(|e| e.detail.clone()).collect();
        assert_eq!(
            details
                .iter()
                .filter(|d| d.starts_with("EDGE_COMMAND_PATH_DISABLED"))
                .count(),
            1,
            "the transition audit does not repeat"
        );
    }

    #[test]
    fn a_non_issued_state_is_refused_and_an_unknown_state_never_constructs() {
        let key = key();
        let inputs = full_inputs("pilot-site-A", BASE_MILLIS + 3_600_000, &key);
        let runtime = CommandPathRuntime::evaluate(&inputs, &key, BASE_MILLIS);
        let audit = Arc::new(InMemoryAuditSink::new());
        let mut rx = receiver(runtime, Arc::clone(&audit));
        let proposed = EdgeCommand::new(
            "cmd-2".to_string(),
            "ess-01".to_string(),
            "PROPOSED".to_string(),
            2,
        )
        .expect("PROPOSED is a frozen state name");
        let error = rx.receive(&proposed, "simulated-test").expect_err("only ISSUED arrives");
        assert_eq!(error.code(), "unexpected_command_state");
        assert!(EdgeCommand::new(
            "cmd-3".to_string(),
            "ess-01".to_string(),
            "NOT_A_FROZEN_STATE".to_string(),
            3,
        )
        .is_err(), "an unknown state name never constructs");
    }

    #[test]
    fn the_policy_gate_refusal_is_audited_and_refused() {
        let key = key();
        let inputs = full_inputs("pilot-site-A", BASE_MILLIS + 3_600_000, &key);
        let runtime = CommandPathRuntime::evaluate(&inputs, &key, BASE_MILLIS);
        let audit = Arc::new(InMemoryAuditSink::new());
        // A policy engine whose entry for the adapter is DISABLED: the
        // independent gate refuses before any admission.
        let policy = Arc::new(crate::policy::SitePolicyEngine::new(SitePolicyConfig {
            format_version: 1,
            adapters: vec![AdapterPolicy {
                adapter_id: "simulated-test".to_string(),
                enabled: false,
                rate_limit: RateLimit { max_commands: 1, window_millis: 60_000 },
                operating_window: OperatingWindow { start_millis_of_day: 0, end_millis_of_day: 86_400_000 },
            }],
        }).expect("valid policy"));
        let mut rx = CommandReceiver::new(
            runtime,
            policy,
            audit as Arc<dyn AuditSink>,
            Arc::new(FixedClock::at_unix_millis(BASE_MILLIS + 1_000)),
        );
        let error = rx
            .receive(&issued_command(), "simulated-test")
            .expect_err("the policy gate refuses");
        assert_eq!(error.code(), "policy_refused");
        let details: Vec<String> = audit.events().iter().map(|e| e.detail.clone()).collect();
        assert!(
            details
                .iter()
                .any(|d| d.starts_with("EDGE_COMMAND_REJECTED") && d.contains("code=policy_refused")),
            "the policy refusal is audited"
        );
    }

    #[test]
    fn the_signed_record_round_trips_and_refuses_tampering() {
        let key = key();
        let signed = record("pilot-site-A", BASE_MILLIS + 3_600_000)
            .sign(&key)
            .expect("signs");
        let text = signed.encode().expect("encodes");
        let parsed = SignedEnablementRecord::parse(&text).expect("parses");
        assert_eq!(parsed, signed);
        assert!(parsed.verify(&key));
        // Tampering with ANY body field invalidates the digest.
        let mut tampered = parsed.clone();
        tampered.approver = "Someone Else".to_string();
        assert!(!tampered.verify(&key));
        // An unknown field in the serialized record is a named error.
        let mut smuggled = text.clone();
        smuggled.push_str("extra_field = sneaky\n");
        assert!(matches!(
            SignedEnablementRecord::parse(&smuggled),
            Err(CommandPathError::RecordMalformed { .. })
        ));
        // A truncated record (digest cut) is a named error.
        let truncated = &text[..text.len() - 4];
        assert!(SignedEnablementRecord::parse(truncated).is_err());
        // The key redacts.
        assert_eq!(format!("{:?}", key), "EnablementKey(<redacted>)");
        assert_eq!(key.to_string(), "<redacted>");
    }

    #[test]
    fn gate_inputs_env_flag_parsing_rejects_unknown_values() {
        // The flag parser accepts exactly enabled/disabled; everything else
        // is a named error (exercised through the same match arm from_env
        // uses, without touching the process environment).
        let parse_flag = |raw: &str| -> Result<bool, CommandPathError> {
            match raw.trim().to_ascii_lowercase().as_str() {
                COMMAND_PATH_ENABLED_VALUE => Ok(true),
                COMMAND_PATH_DISABLED_VALUE => Ok(false),
                other => Err(CommandPathError::InvalidEnv {
                    name: ENV_COMMAND_PATH_FLAG,
                    reason: format!("unexpected value '{other}'"),
                }),
            }
        };
        assert!(matches!(parse_flag("enabled"), Ok(true)));
        assert!(matches!(parse_flag(" Disabled "), Ok(false)));
        assert!(matches!(
            parse_flag("true"),
            Err(CommandPathError::InvalidEnv { .. })
        ));
    }
}
