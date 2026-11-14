// P9.33 — The edge enrollment CEREMONY as code: the ceremony state machine,
//         the nonce-based challenge with replay protection, the
//         single-use/short-lived token discipline, the platform-side
//         approval gate, and the append-only identity-lifecycle register
//         (report §18.1 secure site enrollment and identity; §17.3 mTLS +
//         short-lived credentials; §18.3; ledger P9.33).
//
// WHAT THIS MODULE EXTENDS (never re-implements): the P6.06 enrollment seam
// (`crate::enrollment` — `EnrollmentClient` / `CredentialStore` /
// `RevocationChecker`, the `DeviceCredentialRecord`, the
// `EnrollmentOrchestrator`). The actual token → credential EXCHANGE stays
// exactly where P6.06 put it; this module adds what P6.06 never specified —
// the CEREMONY around that exchange (ledger P9.33: "the part of identity
// P9.09 and P9.13 never specify"): the closed state machine, who may approve
// and when, how a compromised identity is revoked and re-enrolled WITHOUT
// the old identity vouching for the new one.
//
// THE CEREMONY (ledger step 1, written into the type system):
// - The enrollment token is issued by the PLATFORM IDENTITY AUTHORITY (the
//   control-plane enrollment service behind `POST /api/v1/edge/enroll`,
//   per the P6.06 module header); the edge GENERATES nothing and assumes
//   nothing. The token record here carries the token as a REFERENCE only
//   (`EnrollmentTokenRecord`) plus the named issuer, issue instant, and
//   expiry instant.
// - The token is SINGLE-USE and SHORT-LIVED: the ceremony enforces the
//   lifetime ceiling ([`DEFAULT_MAX_TOKEN_LIFETIME_MILLIS`]) at request
//   time, refuses an already-spent reference through the edge-side
//   [`TokenSingleUseLedger`] (the platform-side consumption is
//   authoritative — this ledger is the edge-local guard), and refuses an
//   expired token against the INJECTED clock (never wall time).
// - The token travels on the operator's out-of-band provisioning channel at
//   install time (a human delivers it to the site; it never rides a
//   discovery protocol), and the exchange itself rides the outbound-only
//   mTLS enrollment flow of P6.06/P6.07. Recorded here because the ledger
//   asks "the channel it travels on".
//
// THE STATE MACHINE (closed; every transition named; no self-approval path):
//
// ```text
// Unenrolled --BeginRequest----------> Requested
// Requested  --IssueChallenge--------> Challenged
// Challenged --AnswerChallenge-------> Challenged (answered flag set)
// Challenged --Approve---------------> Approved    [PLATFORM human/authority gate]
// Approved   --Activate--------------> Active      [valid P6.06 credential record]
// Requested|Challenged|Approved|Active --Revoke--> Revoked
// Revoked    --ResetForReEnrollment--> Unenrolled  [fresh token; the REVOKED identity
//                                                    can never be the authorizer]
// ```
//
// EVERY other edge is an [`EnrollmentCeremonyError::InvalidTransition`]
// refusal — nothing advances except along the named edges above. The
// Approve transition is THE HUMAN GATE: the ceremony only RECORDS a
// [`CeremonyApproval`] that names a platform-side approver and an authority
// reference; it contains no code path that can approve itself
// ([`EnrollmentCeremonyError::SelfApprovalRefused`] refuses an approval
// whose actor is the device itself, and an approval without an authority
// reference is not an approval). The ceremony EXECUTION — actually running
// a pilot enrollment — is the recorded UNRUN human gate (P9.33 step 4:
// one non-production edge, end-to-end); nothing here performs one.
//
// THE CHALLENGE (nonce-based, replay-protected): `issue_challenge` derives
// a ceremony nonce from the device identity, token reference, binding,
// INJECTED clock instant, and a caller entropy seed
// ([`derive_challenge_nonce`]); `answer_challenge` refuses a wrong nonce,
// an expired challenge (the TTL: [`DEFAULT_CHALLENGE_TTL_MILLIS`]), and a
// REPLAYED ceremony message (the replay memory keys the message kind + the
// nonce and never forgets within the ceremony — a repeated message is the
// named [`EnrollmentCeremonyError::CeremonyMessageReplayed`] with its
// first-seen instant). HONESTY: the nonce derivation is the test-grade
// FNV-1a-64 pattern (the P4.28/P6.14/P7.14 family) — an UN-GUESSABLE nonce
// needs a CSPRNG, which is a recorded DEPLOYMENT REQUIREMENT (no crypto
// dependency in this loop); the derivation is deterministic so the
// ceremony is testable, and the production replacement is a format-
// visible change (the `ceremony-msg:v1` prefix pins the scheme).
//
// THE IDENTITY REGISTER (lifecycle as an append-only record, mirroring the
// kernel's revocation pattern and the P9.31 `FileCommandIdBinding`
// framing): every named lifecycle transition appends an event to
// [`FileIdentityEventRegister`] — magic-headered, length-prefixed,
// CRC-32-checked, durable BEFORE the in-memory view moves, truncate-and-
// audit recovery, last-event-wins on replay. Revocation and re-enrollment
// are APPENDS: nothing is ever overwritten in place, so the full identity
// history (born → active → revoked → re-enrolled) survives a restart and
// is the evidence P9.09's revocation measurement reads.
//
// HONESTY BOUNDS (AGENTS.md rule 1, §13.3 discipline): this module is the
// ceremony STRUCTURE. No PKI exists (the P6.06 honesty record stands
// unchanged); no enrollment has ever been executed against a platform (the
// non-production end-to-end enrollment of ledger step 4 is the human gate,
// recorded UNRUN); the nonce derivation and every digest here are
// integrity checksums, NOT security primitives; and what the WRITTEN-UNRUN
// tests would and would not establish is itemised in
// `ledger/phase-09-pilot-qualification/33_edge_enrollment.md`.

use std::collections::BTreeMap;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use flexcore_ports::clock::Clock;

use crate::enrollment::{DeviceBinding, DeviceCredentialRecord, ProvisioningToken};
use crate::ports::{AuditSink, EdgeAuditEvent, EdgeAuditKind};

// --- Constants -----------------------------------------------------------------

/// The maximum enrollment-token lifetime the ceremony accepts (ledger
/// step 2: "single-use and short-lived"). 15 minutes: long enough for an
/// install window, far too short to become a standing credential (the
/// ledger's named risk). A platform issuing longer-lived tokens is refused
/// at `begin_request`.
pub const DEFAULT_MAX_TOKEN_LIFETIME_MILLIS: i64 = 15 * 60_000;
/// The challenge time-to-live: a challenge answered after this window is
/// refused (the ceremony must complete within the token's short life).
pub const DEFAULT_CHALLENGE_TTL_MILLIS: i64 = 5 * 60_000;
/// Hard sanity cap on one identity-register record payload (rejects absurd
/// lengths from a corrupt header instead of attempting an absurd
/// allocation).
const MAX_IDENTITY_RECORD_BYTES: u32 = 64 * 1024;

// --- The closed ceremony state vocabulary ---------------------------------------

/// The closed ceremony states (ledger P9.33: `Unenrolled → Requested →
/// Challenged → Approved → Active → Revoked`). CLOSED set: extending it is
/// a reviewed change. `name()` is the stable audit text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CeremonyState {
    /// No identity requested; the edge has nothing.
    Unenrolled,
    /// A valid, unspent, unexpired token was presented; the ceremony is open.
    Requested,
    /// A nonce challenge is outstanding (or answered, pending approval).
    Challenged,
    /// The PLATFORM-side approval was recorded (the human gate).
    Approved,
    /// A validated P6.06 credential record was stored under this identity.
    Active,
    /// The identity was revoked (compromise, decommission, authority action).
    Revoked,
}

impl CeremonyState {
    /// The stable state name (audit text, greppable).
    pub fn name(self) -> &'static str {
        match self {
            CeremonyState::Unenrolled => "UNENROLLED",
            CeremonyState::Requested => "REQUESTED",
            CeremonyState::Challenged => "CHALLENGED",
            CeremonyState::Approved => "APPROVED",
            CeremonyState::Active => "ACTIVE",
            CeremonyState::Revoked => "REVOKED",
        }
    }
}

/// The named ceremony transitions (the ONLY edges that exist). Each names
/// exactly one from-state set and one to-state; see the module header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CeremonyTransition {
    /// Unenrolled → Requested (a token was presented).
    BeginRequest,
    /// Requested → Challenged (the nonce challenge was issued).
    IssueChallenge,
    /// Challenged → Challenged (answered flag set; the response verified).
    AnswerChallenge,
    /// Challenged → Approved (the platform approval was recorded).
    Approve,
    /// Approved → Active (a validated credential record was bound).
    Activate,
    /// Requested/Challenged/Approved/Active → Revoked.
    Revoke,
    /// Revoked → Unenrolled (re-enrollment opened on a FRESH token).
    ResetForReEnrollment,
}

impl CeremonyTransition {
    /// The stable transition name (audit text: `CEREMONY_BEGIN_REQUEST` …).
    pub fn name(self) -> &'static str {
        match self {
            CeremonyTransition::BeginRequest => "CEREMONY_BEGIN_REQUEST",
            CeremonyTransition::IssueChallenge => "CEREMONY_ISSUE_CHALLENGE",
            CeremonyTransition::AnswerChallenge => "CEREMONY_ANSWER_CHALLENGE",
            CeremonyTransition::Approve => "CEREMONY_APPROVE",
            CeremonyTransition::Activate => "CEREMONY_ACTIVATE",
            CeremonyTransition::Revoke => "CEREMONY_REVOKE",
            CeremonyTransition::ResetForReEnrollment => "CEREMONY_RESET_FOR_RE_ENROLLMENT",
        }
    }
}

// --- The token record and the single-use ledger -----------------------------------

/// The enrollment-token RECORD the ceremony consumes (ledger step 1). The
/// token VALUE is never held here — the caller proves a token exists by
/// passing the P6.06 [`ProvisioningToken`] (which the ceremony never
/// `expose()`s); the record carries the token's REFERENCE and its life
/// facts. An issuer that cannot be named cannot issue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnrollmentTokenRecord {
    /// The token's reference (the platform-side token id — a reference,
    /// never the secret value; no secret renders into any diagnostic).
    pub token_reference: String,
    /// WHO issued it: the named platform identity authority (e.g. the
    /// control-plane enrollment service instance). Empty = refused.
    pub issued_by: String,
    /// When the authority issued the token (unix millis, the authority's
    /// clock — recorded data, never derived here).
    pub issued_at_unix_millis: i64,
    /// When the token dies (unix millis). `expires_at <= now` is an expired
    /// token; `expires_at - issued_at` above
    /// [`DEFAULT_MAX_TOKEN_LIFETIME_MILLIS`] is refused as excessive.
    pub expires_at_unix_millis: i64,
}

impl EnrollmentTokenRecord {
    /// The token lifetime in milliseconds (caller-side arithmetic mirror;
    /// the ceremony validates it itself at request time).
    pub fn lifetime_millis(&self) -> i64 {
        self.expires_at_unix_millis
            .saturating_sub(self.issued_at_unix_millis)
    }
}

/// The EDGE-LOCAL single-use token ledger (ledger step 2: "test reuse of a
/// spent token … refused"). `mark_spent` refuses a reference that was
/// already consumed by ANY ceremony this edge ran — the local guard. The
/// PLATFORM-side consumption is the authoritative single-use enforcement
/// (the P6.06 issuer contract); this ledger is what the edge can check
/// without a round-trip.
#[derive(Debug, Default)]
pub struct TokenSingleUseLedger {
    spent_at: BTreeMap<String, i64>,
}

impl TokenSingleUseLedger {
    pub fn new() -> Self {
        Self::default()
    }

    /// Marks a token reference spent at `at_unix_millis`. A reference that
    /// is ALREADY spent is the named
    /// [`EnrollmentCeremonyError::EnrollmentTokenAlreadySpent`] — the reuse
    /// attempt is refused, not absorbed.
    pub fn mark_spent(
        &mut self,
        token_reference: &str,
        at_unix_millis: i64,
    ) -> Result<(), EnrollmentCeremonyError> {
        if token_reference.trim().is_empty() {
            return Err(EnrollmentCeremonyError::InvalidCeremonyInput {
                reason: "the token reference is mandatory".to_string(),
            });
        }
        if let Some(first_spent_at) = self.spent_at.get(token_reference) {
            return Err(EnrollmentCeremonyError::EnrollmentTokenAlreadySpent {
                spent_at_unix_millis: *first_spent_at,
            });
        }
        self.spent_at
            .insert(token_reference.to_string(), at_unix_millis);
        Ok(())
    }

    /// Whether a reference has been spent (diagnostics/tests).
    pub fn is_spent(&self, token_reference: &str) -> bool {
        self.spent_at.contains_key(token_reference)
    }

    /// How many distinct references are spent (diagnostics).
    pub fn len(&self) -> usize {
        self.spent_at.len()
    }

    /// Whether nothing is spent.
    pub fn is_empty(&self) -> bool {
        self.spent_at.is_empty()
    }
}

// --- The platform approval (THE human gate) -----------------------------------------

/// The platform-side APPROVAL record (ledger step 1: "who issues it"; the
/// recorded rule: approval is the platform-side human/authority gate and
/// the ceremony has NO self-approval path). The ceremony only RECORDS a
/// presented approval — it never generates one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CeremonyApproval {
    /// The platform-side approver (an authenticated human subject or the
    /// authority's automation ON BEHALF of a named human policy). Empty =
    /// refused.
    pub approver: String,
    /// The authority reference: the approval record/ticket id the approver
    /// signed on the platform side. Empty = refused (an approval that
    /// cannot be traced to an authority decision is not an approval).
    pub authority_reference: String,
    /// The device identity the approval names. Must equal the ceremony's
    /// device identity — an approval for a different device is refused.
    pub approved_device_identity: String,
    /// The tenant/site scope the approval grants (§16.2). Must equal the
    /// ceremony's configured binding.
    pub approved_tenant_id: String,
    pub approved_site_id: String,
    /// When the platform approved (unix millis, the platform's clock —
    /// recorded data).
    pub approved_at_unix_millis: i64,
}

impl CeremonyApproval {
    /// Validates the approval (fail-closed: approver, authority reference,
    /// device identity, and scope are all mandatory and must MATCH the
    /// ceremony).
    pub fn validate_against(
        &self,
        device_identity: &str,
        binding: &DeviceBinding,
    ) -> Result<(), EnrollmentCeremonyError> {
        let mut problems: Vec<String> = Vec::new();
        if self.approver.trim().is_empty() {
            problems.push("approver is empty".to_string());
        }
        if self.authority_reference.trim().is_empty() {
            problems.push(
                "authority_reference is empty (an approval without an authority decision is \
                 not an approval)"
                    .to_string(),
            );
        }
        if self.approved_device_identity.trim().is_empty() {
            problems.push("approved_device_identity is empty".to_string());
        } else if self.approved_device_identity != device_identity {
            problems.push(format!(
                "approved_device_identity '{}' does not name this ceremony's device",
                self.approved_device_identity
            ));
        }
        if self.approved_tenant_id != binding.tenant_id
            || self.approved_site_id != binding.site_id
        {
            problems.push(
                "the approved tenant/site scope does not match the ceremony's configured \
                 binding (§16.2)"
                    .to_string(),
            );
        }
        if self.approved_at_unix_millis <= 0 {
            problems.push("approved_at_unix_millis is not a positive instant".to_string());
        }
        if problems.is_empty() {
            Ok(())
        } else {
            Err(EnrollmentCeremonyError::ApprovalInvalid {
                reason: problems.join("; "),
            })
        }
    }
}

// --- Re-enrollment authorization (the compromise rule) -------------------------------

/// WHAT authorizes re-enrollment after compromise (ledger step 3). CLOSED
/// set: the ONLY legal basis is a fresh platform-authority ticket; the
/// compromised identity can NEVER be the authorizer — a `PriorIdentity`
/// basis is refused with the named
/// [`EnrollmentCeremonyError::CompromisedIdentityVouchingRefused`] (the
/// ledger's named risk: "re-enrollment that trusts the old (compromised)
/// identity to vouch for the new one").
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReEnrollmentAuthorization {
    /// A fresh ticket/reference raised by the platform identity authority
    /// (independent of the edge's compromised identity).
    PlatformAuthorityTicket { ticket_reference: String },
    /// The old (compromised) identity itself — structurally refused.
    PriorIdentity { reference: String },
}

// --- Errors -----------------------------------------------------------------------

/// Named errors of the ceremony. Every failure is named
/// ([`EnrollmentCeremonyError::name`]); nothing implies a silent pass. No
/// error ever carries a token VALUE.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnrollmentCeremonyError {
    /// A ceremony input was unusable (empty identity, empty reason, …).
    InvalidCeremonyInput { reason: String },
    /// The attempted transition does not exist in the closed state
    /// machine. Names the from-state, the attempted transition, and the
    /// state the caller appears to want.
    InvalidTransition {
        from: &'static str,
        transition: &'static str,
        detail: String,
    },
    /// The presented token was already consumed (single-use, ledger step
    /// 2). `spent_at_unix_millis` is when it was FIRST consumed.
    EnrollmentTokenAlreadySpent { spent_at_unix_millis: i64 },
    /// The presented token has expired against the INJECTED clock (ledger
    /// step 2: "use of an expired token — refused").
    EnrollmentTokenExpired {
        expires_at_unix_millis: i64,
        now_unix_millis: i64,
    },
    /// The token's lifetime exceeds the ceremony's short-lived ceiling.
    EnrollmentTokenLifetimeExcessive {
        lifetime_millis: i64,
        maximum_millis: i64,
    },
    /// The token record names no issuer (an un-attributable token is
    /// refused).
    EnrollmentTokenIssuerUnnamed,
    /// The challenge was answered with the wrong nonce.
    ChallengeNonceMismatch,
    /// The challenge was answered after its time-to-live (INJECTED clock).
    ChallengeExpired {
        expires_at_unix_millis: i64,
        now_unix_millis: i64,
    },
    /// A ceremony message was REPLAYED (ledger P9.33 replay protection).
    /// `first_seen_at_unix_millis` is when the same message key was first
    /// accepted.
    CeremonyMessageReplayed {
        message_key: String,
        first_seen_at_unix_millis: i64,
    },
    /// The presented approval is invalid (missing fields, wrong device,
    /// wrong scope, non-positive instant).
    ApprovalInvalid { reason: String },
    /// THE SELF-APPROVAL REFUSAL: the approval's actor IS the device (or
    /// the ceremony was asked to approve without a distinct platform-side
    /// authority). There is no self-approval path — recorded, structural.
    SelfApprovalRefused { reason: String },
    /// The challenge must be ANSWERED before approval.
    ChallengeNotAnswered,
    /// The credential record failed P6.06 validation.
    CredentialInvalid { reason: String },
    /// The issued identity/binding does not match the ceremony's device or
    /// configured §16.2 scope (the wrong-site/wrong-identity refusal).
    BindingMismatch { reason: String },
    /// A re-enrollment was attempted WITH the compromised identity as the
    /// authorizer — refused by the recorded compromise rule.
    CompromisedIdentityVouchingRefused { reason: String },
    /// The identity is already revoked (a second revoke is refused; the
    /// register is append-only, not a state overwrite).
    AlreadyRevoked,
    /// The identity register could not be opened/loaded (fail-closed).
    RegisterUnavailable { reason: String },
    /// The identity register I/O failed (the event was NOT persisted).
    RegisterIo { reason: String },
    /// The file exists but is not a FlexCore edge identity register (wrong
    /// magic or version) — refused, never guessed at.
    NotAnIdentityRegister { reason: String },
}

impl EnrollmentCeremonyError {
    /// The stable machine-readable error name (the named-error table key).
    pub fn name(&self) -> &'static str {
        match self {
            EnrollmentCeremonyError::InvalidCeremonyInput { .. } => "CEREMONY_INPUT_INVALID",
            EnrollmentCeremonyError::InvalidTransition { .. } => "CEREMONY_TRANSITION_INVALID",
            EnrollmentCeremonyError::EnrollmentTokenAlreadySpent { .. } => {
                "CEREMONY_TOKEN_ALREADY_SPENT"
            }
            EnrollmentCeremonyError::EnrollmentTokenExpired { .. } => "CEREMONY_TOKEN_EXPIRED",
            EnrollmentCeremonyError::EnrollmentTokenLifetimeExcessive { .. } => {
                "CEREMONY_TOKEN_LIFETIME_EXCESSIVE"
            }
            EnrollmentCeremonyError::EnrollmentTokenIssuerUnnamed => {
                "CEREMONY_TOKEN_ISSUER_UNNAMED"
            }
            EnrollmentCeremonyError::ChallengeNonceMismatch => "CEREMONY_CHALLENGE_NONCE_MISMATCH",
            EnrollmentCeremonyError::ChallengeExpired { .. } => "CEREMONY_CHALLENGE_EXPIRED",
            EnrollmentCeremonyError::CeremonyMessageReplayed { .. } => {
                "CEREMONY_MESSAGE_REPLAYED"
            }
            EnrollmentCeremonyError::ApprovalInvalid { .. } => "CEREMONY_APPROVAL_INVALID",
            EnrollmentCeremonyError::SelfApprovalRefused { .. } => "CEREMONY_SELF_APPROVAL_REFUSED",
            EnrollmentCeremonyError::ChallengeNotAnswered => "CEREMONY_CHALLENGE_NOT_ANSWERED",
            EnrollmentCeremonyError::CredentialInvalid { .. } => "CEREMONY_CREDENTIAL_INVALID",
            EnrollmentCeremonyError::BindingMismatch { .. } => "CEREMONY_BINDING_MISMATCH",
            EnrollmentCeremonyError::CompromisedIdentityVouchingRefused { .. } => {
                "CEREMONY_COMPROMISED_IDENTITY_VOUCHING_REFUSED"
            }
            EnrollmentCeremonyError::AlreadyRevoked => "CEREMONY_ALREADY_REVOKED",
            EnrollmentCeremonyError::RegisterUnavailable { .. } => "CEREMONY_REGISTER_UNAVAILABLE",
            EnrollmentCeremonyError::RegisterIo { .. } => "CEREMONY_REGISTER_IO",
            EnrollmentCeremonyError::NotAnIdentityRegister { .. } => "CEREMONY_NOT_AN_IDENTITY_REGISTER",
        }
    }
}

impl std::fmt::Display for EnrollmentCeremonyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EnrollmentCeremonyError::InvalidCeremonyInput { reason } => {
                write!(f, "the ceremony input is invalid: {reason}")
            }
            EnrollmentCeremonyError::InvalidTransition {
                from,
                transition,
                detail,
            } => write!(
                f,
                "the transition {transition} does not exist from state {from} — refused by \
                 the closed ceremony state machine ({detail})"
            ),
            EnrollmentCeremonyError::EnrollmentTokenAlreadySpent {
                spent_at_unix_millis,
            } => write!(
                f,
                "the enrollment token was already consumed at {spent_at_unix_millis} \
                 (single-use; ledger P9.33 step 2)"
            ),
            EnrollmentCeremonyError::EnrollmentTokenExpired {
                expires_at_unix_millis,
                now_unix_millis,
            } => write!(
                f,
                "the enrollment token expired at {expires_at_unix_millis} (now \
                 {now_unix_millis}); an expired token is refused, never tolerated"
            ),
            EnrollmentCeremonyError::EnrollmentTokenLifetimeExcessive {
                lifetime_millis,
                maximum_millis,
            } => write!(
                f,
                "the enrollment token lives {lifetime_millis} ms, above the {maximum_millis} ms \
                 short-lived ceiling — a long-lived enrollment token becomes a standing \
                 credential (the ledger's named risk)"
            ),
            EnrollmentCeremonyError::EnrollmentTokenIssuerUnnamed => write!(
                f,
                "the token record names no issuing authority; an un-attributable token is \
                 refused (ledger step 1: the issuer is named)"
            ),
            EnrollmentCeremonyError::ChallengeNonceMismatch => {
                write!(f, "the presented challenge nonce does not match the issued nonce")
            }
            EnrollmentCeremonyError::ChallengeExpired {
                expires_at_unix_millis,
                now_unix_millis,
            } => write!(
                f,
                "the challenge expired at {expires_at_unix_millis} (now {now_unix_millis})"
            ),
            EnrollmentCeremonyError::CeremonyMessageReplayed {
                message_key,
                first_seen_at_unix_millis,
            } => write!(
                f,
                "ceremony message '{message_key}' was already seen at \
                 {first_seen_at_unix_millis} — a replayed ceremony message is refused"
            ),
            EnrollmentCeremonyError::ApprovalInvalid { reason } => {
                write!(f, "the ceremony approval is invalid: {reason}")
            }
            EnrollmentCeremonyError::SelfApprovalRefused { reason } => write!(
                f, "SELF-APPROVAL REFUSED: {reason} — approval is the platform-side \
                     human/authority gate; the ceremony has no self-approval path"
            ),
            EnrollmentCeremonyError::ChallengeNotAnswered => {
                write!(f, "the challenge must be answered before approval")
            }
            EnrollmentCeremonyError::CredentialInvalid { reason } => {
                write!(f, "the credential record is invalid: {reason}")
            }
            EnrollmentCeremonyError::BindingMismatch { reason } => {
                write!(f, "the issued identity/binding does not match the ceremony: {reason}")
            }
            EnrollmentCeremonyError::CompromisedIdentityVouchingRefused { reason } => write!(
                f,
                "COMPROMISED-IDENTITY VOUCHING REFUSED: {reason} — a revoked identity can \
                 never authorize its own successor (ledger P9.33 step 3)"
            ),
            EnrollmentCeremonyError::AlreadyRevoked => {
                write!(f, "the identity is already revoked (the register is append-only)")
            }
            EnrollmentCeremonyError::RegisterUnavailable { reason } => write!(
                f,
                "the identity register is unavailable; lifecycle transitions are refused \
                 fail-closed: {reason}"
            ),
            EnrollmentCeremonyError::RegisterIo { reason } => {
                write!(f, "the identity register I/O failed: {reason}")
            }
            EnrollmentCeremonyError::NotAnIdentityRegister { reason } => write!(
                f,
                "the file is not a FlexCore edge identity register (refused, never guessed): \
                 {reason}"
            ),
        }
    }
}

impl std::error::Error for EnrollmentCeremonyError {}

// --- The nonce derivation and ceremony-message binding -------------------------------

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

fn fold(state: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *state ^= u64::from(*byte);
        *state = state.wrapping_mul(FNV_PRIME);
    }
}

fn fold_str(state: &mut u64, value: &str) {
    fold(state, &(value.len() as u64).to_le_bytes());
    fold(state, value.as_bytes());
}

/// The FNV-1a 64 of an EMPTY input is the offset basis — the definitional
/// known answer the known-answer tests pin (the genesis of every fold in
/// this module).
pub const CEREMONY_DIGEST_GENESIS: u64 = FNV_OFFSET;

/// Derives the ceremony challenge nonce: FNV-1a-64 over the
/// length-prefixed (device identity, token reference, tenant, site,
/// INJECTED clock instant, entropy seed). Deterministic so the ceremony is
/// testable with known-answer tests.
///
/// HONESTY (module header): this is the TEST-GRADE FNV-1a-64 derivation —
/// an integrity checksum, NOT a security primitive. A production ceremony
/// nonce comes from a CSPRNG under the platform authority — a recorded
/// DEPLOYMENT REQUIREMENT; the `ceremony-msg:v1` scheme prefix makes that
/// replacement a detectable format change, never a silent one.
pub fn derive_challenge_nonce(
    device_identity: &str,
    token_reference: &str,
    binding: &DeviceBinding,
    now_unix_millis: i64,
    entropy_seed: &str,
) -> String {
    let mut state = FNV_OFFSET;
    fold_str(&mut state, device_identity);
    fold_str(&mut state, token_reference);
    fold_str(&mut state, &binding.tenant_id);
    fold_str(&mut state, &binding.site_id);
    fold(&mut state, &now_unix_millis.to_le_bytes());
    fold_str(&mut state, entropy_seed);
    format!("{state:016x}")
}

/// The canonical ceremony-message binding string (the replay-protection key
/// shape; the known-answer tests assert it character-for-character).
pub fn canonical_ceremony_message_binding(
    message_kind: &str,
    device_identity: &str,
    nonce_hex: &str,
    at_unix_millis: i64,
) -> String {
    format!(
        "ceremony-msg:v1|kind:{message_kind}|device:{device_identity}|nonce:{nonce_hex}|at:{at_unix_millis}"
    )
}

// --- The identity-lifecycle register (append-only) -------------------------------------

/// The closed identity-event vocabulary of the append-only register.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentityEventKind {
    /// A ceremony opened for the identity (a token was presented).
    Requested,
    /// The platform approval was recorded (the human gate).
    Approved,
    /// A validated credential record was bound (the identity is active).
    Activated,
    /// The identity was revoked (compromise / decommission / authority).
    Revoked,
    /// Re-enrollment was opened on a FRESH token (the old identity did not
    /// authorize it).
    ReEnrollmentOpened,
}

impl IdentityEventKind {
    /// The stable wire/register code (u8; unknown codes on replay are
    /// corrupt-tail material, never guessed into a known kind).
    pub fn code(self) -> u8 {
        match self {
            IdentityEventKind::Requested => 1,
            IdentityEventKind::Approved => 2,
            IdentityEventKind::Activated => 3,
            IdentityEventKind::Revoked => 4,
            IdentityEventKind::ReEnrollmentOpened => 5,
        }
    }

    /// The stable audit/event text.
    pub fn name(self) -> &'static str {
        match self {
            IdentityEventKind::Requested => "CEREMONY_REQUESTED",
            IdentityEventKind::Approved => "CEREMONY_APPROVED",
            IdentityEventKind::Activated => "CEREMONY_ACTIVATED",
            IdentityEventKind::Revoked => "CEREMONY_REVOKED",
            IdentityEventKind::ReEnrollmentOpened => "CEREMONY_RE_ENROLLMENT_OPENED",
        }
    }

    fn from_code(code: u8) -> Option<Self> {
        match code {
            1 => Some(IdentityEventKind::Requested),
            2 => Some(IdentityEventKind::Approved),
            3 => Some(IdentityEventKind::Activated),
            4 => Some(IdentityEventKind::Revoked),
            5 => Some(IdentityEventKind::ReEnrollmentOpened),
            _ => None,
        }
    }
}

/// One append-only identity-lifecycle event (what the register persists;
/// the full history IS the identity's evidence trail).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentityEventRecord {
    pub kind: IdentityEventKind,
    /// When the transition happened (the INJECTED clock — never wall time).
    pub at_unix_millis: i64,
    /// WHO drove the transition (the platform approver, the operator, the
    /// authority ticket).
    pub actor: String,
    /// WHY (mandatory for Revoked/ReEnrollmentOpened; may be empty for the
    /// routine forward transitions).
    pub reason: String,
    /// The reference chain (authority ticket, approval record, fresh-token
    /// reference).
    pub reference: String,
}

/// The file-backed append-only identity-lifecycle register. Same framing
/// discipline as `FileCommandIdBinding` (`pilot_rejection_proof.rs`, P9.31)
/// and `FileAuditBuffer` (P6.18):
///
/// ```text
/// header:  b"FLEXCORE-EDGE-IDENTITY-REGISTER" + format version byte (0x01)
/// record:  [u32 LE payload_len][u32 LE crc32(payload)][payload]
/// payload: [u8 kind][i64 LE at][u32 actor_len][actor][u32 reason_len][reason]
///          [u32 ref_len][reference]
/// ```
///
/// **Append-only, never overwritten.** Revocation and re-enrollment are
/// NEW records; nothing is edited or removed in place, so the identity's
/// full history survives (the kernel revocation-register pattern: the
/// register is the record, not a mutable status cell).
///
/// **Recovery (truncate-and-audit, never silent).** On open the file is
/// replayed record by record; the FIRST malformed frame ends the replay and
/// the file is TRUNCATED at the last good record, with the loss counted
/// ([`FileIdentityEventRegister::corrupt_tail_truncated`]) and logged. A
/// wrong magic/version is a hard open error.
///
/// **Durability BEFORE state.** An event is flushed and `sync_data`-ed
/// before the in-memory history moves — a crash cannot lose a transition
/// the caller was told was recorded.
pub struct FileIdentityEventRegister {
    path: PathBuf,
    file: Mutex<std::fs::File>,
    events: Mutex<Vec<IdentityEventRecord>>,
    corrupt_tail_truncated: std::sync::atomic::AtomicU64,
    records_written: std::sync::atomic::AtomicU64,
}

impl FileIdentityEventRegister {
    /// The register magic header (30-byte label) + format version.
    fn magic() -> Vec<u8> {
        let mut magic = b"FLEXCORE-EDGE-IDENTITY-REGISTER".to_vec();
        magic.push(0x01);
        magic
    }

    /// Opens (or creates) the register at `path` and replays any existing
    /// events.
    pub fn open(path: &Path) -> Result<Self, EnrollmentCeremonyError> {
        if path.as_os_str().is_empty() {
            return Err(EnrollmentCeremonyError::RegisterUnavailable {
                reason: "the identity register path is empty".to_string(),
            });
        }
        let magic = Self::magic();
        let (file, replayed) = match std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
        {
            Ok(mut existing) => {
                let mut contents = Vec::new();
                existing
                    .read_to_end(&mut contents)
                    .map_err(|e| EnrollmentCeremonyError::RegisterIo {
                        reason: format!("reading the existing identity register failed: {e}"),
                    })?;
                if contents.is_empty() {
                    (existing, Vec::new())
                } else {
                    if contents.len() < magic.len() || contents[..magic.len()] != magic[..] {
                        return Err(EnrollmentCeremonyError::NotAnIdentityRegister {
                            reason: format!(
                                "the file at {} does not carry the FlexCore edge identity \
                                 register magic",
                                path.display()
                            ),
                        });
                    }
                    let mut events = Vec::new();
                    let mut offset = magic.len();
                    let mut corrupt_tail_truncated: u64 = 0;
                    while offset + 8 <= contents.len() {
                        let len = u32::from_le_bytes(
                            contents[offset..offset + 4].try_into().expect("4 bytes"),
                        );
                        if len > MAX_IDENTITY_RECORD_BYTES
                            || offset + 8 + len as usize > contents.len()
                        {
                            corrupt_tail_truncated += 1;
                            break;
                        }
                        let payload = &contents[offset + 8..offset + 8 + len as usize];
                        let stored_crc = u32::from_le_bytes(
                            contents[offset + 4..offset + 8].try_into().expect("4 bytes"),
                        );
                        if crc32(payload) != stored_crc {
                            corrupt_tail_truncated += 1;
                            break;
                        }
                        match decode_identity_payload(payload) {
                            Some(record) => {
                                events.push(record);
                                offset += 8 + len as usize;
                            }
                            None => {
                                corrupt_tail_truncated += 1;
                                break;
                            }
                        }
                    }
                    if offset < contents.len() && corrupt_tail_truncated == 0 {
                        // A torn partial frame is also a corrupt tail.
                        corrupt_tail_truncated += 1;
                    }
                    if corrupt_tail_truncated > 0 {
                        tracing::warn!(
                            truncated_records = corrupt_tail_truncated,
                            path = %path.display(),
                            "identity register: corrupt tail truncated; the affected events \
                             reverted to their last good record"
                        );
                        existing
                            .set_len(offset as u64)
                            .map_err(|e| EnrollmentCeremonyError::RegisterIo {
                                reason: format!(
                                    "truncating the corrupt identity-register tail failed: {e}"
                                ),
                            })?;
                    }
                    (existing, events)
                }
            }
            Err(_) => {
                let mut created = std::fs::OpenOptions::new()
                    .read(true)
                    .write(true)
                    .create_new(true)
                    .open(path)
                    .map_err(|e| EnrollmentCeremonyError::RegisterIo {
                        reason: format!("creating the identity register file failed: {e}"),
                    })?;
                created
                    .write_all(&magic)
                    .and_then(|_| created.flush())
                    .map_err(|e| EnrollmentCeremonyError::RegisterIo {
                        reason: format!("writing the identity register header failed: {e}"),
                    })?;
                (created, Vec::new())
            }
        };
        Ok(Self {
            path: path.to_path_buf(),
            file: Mutex::new(file),
            events: Mutex::new(replayed),
            corrupt_tail_truncated: std::sync::atomic::AtomicU64::new(0),
            records_written: std::sync::atomic::AtomicU64::new(0),
        })
    }

    /// The register path (diagnostics).
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// How many corrupt tail records were truncated at open (the audited
    /// loss counter).
    pub fn corrupt_tail_truncated(&self) -> u64 {
        self.corrupt_tail_truncated
            .load(std::sync::atomic::Ordering::SeqCst)
    }

    /// How many records this instance has written (diagnostics).
    pub fn records_written(&self) -> u64 {
        self.records_written
            .load(std::sync::atomic::Ordering::SeqCst)
    }

    /// The number of events in the replayed history.
    pub fn len(&self) -> usize {
        self.events.lock().expect("poisoned identity register mutex").len()
    }

    /// Whether the history is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The full append-only history (evidence reading; never truncated by
    /// a status overwrite — the register IS the history).
    pub fn history(&self) -> Vec<IdentityEventRecord> {
        self.events
            .lock()
            .expect("poisoned identity register mutex")
            .clone()
    }

    /// Appends one lifecycle event. Durable BEFORE the in-memory history
    /// moves. There is no overwrite, no delete, and no "update in place" —
    /// append-only by construction.
    pub fn append(
        &self,
        kind: IdentityEventKind,
        at_unix_millis: i64,
        actor: &str,
        reason: &str,
        reference: &str,
    ) -> Result<(), EnrollmentCeremonyError> {
        if at_unix_millis <= 0 {
            return Err(EnrollmentCeremonyError::InvalidCeremonyInput {
                reason: "the event instant must be a positive unix-millis value".to_string(),
            });
        }
        let payload = encode_identity_payload(
            kind,
            at_unix_millis,
            actor,
            reason,
            reference,
        );
        let record_len = payload.len() as u32;
        let crc = crc32(&payload);
        let mut record = Vec::with_capacity(8 + payload.len());
        record.extend_from_slice(&record_len.to_le_bytes());
        record.extend_from_slice(&crc.to_le_bytes());
        record.extend_from_slice(&payload);
        {
            let mut file = self.file.lock().expect("poisoned identity register file mutex");
            file.seek(SeekFrom::End(0))
                .and_then(|_| file.write_all(&record))
                .and_then(|_| file.flush())
                .and_then(|_| file.sync_data())
                .map_err(|e| EnrollmentCeremonyError::RegisterIo {
                    reason: format!("appending the identity event failed: {e}"),
                })?;
        }
        self.events
            .lock()
            .expect("poisoned identity register mutex")
            .push(IdentityEventRecord {
                kind,
                at_unix_millis,
                actor: actor.to_string(),
                reason: reason.to_string(),
                reference: reference.to_string(),
            });
        self.records_written
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }
}

fn encode_identity_payload(
    kind: IdentityEventKind,
    at_unix_millis: i64,
    actor: &str,
    reason: &str,
    reference: &str,
) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.push(kind.code());
    payload.extend_from_slice(&at_unix_millis.to_le_bytes());
    payload.extend_from_slice(&(actor.len() as u32).to_le_bytes());
    payload.extend_from_slice(actor.as_bytes());
    payload.extend_from_slice(&(reason.len() as u32).to_le_bytes());
    payload.extend_from_slice(reason.as_bytes());
    payload.extend_from_slice(&(reference.len() as u32).to_le_bytes());
    payload.extend_from_slice(reference.as_bytes());
    payload
}

fn decode_identity_payload(payload: &[u8]) -> Option<IdentityEventRecord> {
    if payload.len() < 1 + 8 + 4 {
        return None;
    }
    let kind = IdentityEventKind::from_code(payload[0])?;
    let at_unix_millis = i64::from_le_bytes(payload[1..9].try_into().expect("8 bytes"));
    let mut cursor = 9usize;
    let (actor, cursor_next) = decode_len_string(payload, cursor)?;
    cursor = cursor_next;
    let (reason, cursor_next) = decode_len_string(payload, cursor)?;
    cursor = cursor_next;
    let (reference, _) = decode_len_string(payload, cursor)?;
    Some(IdentityEventRecord {
        kind,
        at_unix_millis,
        actor,
        reason,
        reference,
    })
}

fn decode_len_string(payload: &[u8], cursor: usize) -> Option<(String, usize)> {
    if payload.len() < cursor + 4 {
        return None;
    }
    let len = u32::from_le_bytes(payload[cursor..cursor + 4].try_into().expect("4 bytes"))
        as usize;
    if payload.len() < cursor + 4 + len {
        return None;
    }
    let value = std::str::from_utf8(&payload[cursor + 4..cursor + 4 + len])
        .ok()?
        .to_string();
    Some((value, cursor + 4 + len))
}

/// CRC-32 (IEEE, reflected) — the same checksum family as the checkpoint,
/// the audit journal, and the P9.31 binding journal (an integrity checksum
/// for corruption detection, not a security primitive).
fn crc32(bytes: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

// --- The ceremony -----------------------------------------------------------------

/// The P9.33 enrollment ceremony: the state machine, the challenge, the
/// replay memory, and the audit trail over ONE device identity. The
/// ceremony composes the P6.06 types (`ProvisioningToken`,
/// `DeviceBinding`, `DeviceCredentialRecord`) and the edge `AuditSink`;
/// the actual token → credential exchange stays the P6.06
/// `EnrollmentOrchestrator`'s job — the ceremony gates WHEN that exchange
/// may run and records the lifecycle around it.
pub struct EnrollmentCeremony {
    device_identity: String,
    binding: DeviceBinding,
    state: CeremonyState,
    token_record: Option<EnrollmentTokenRecord>,
    challenge: Option<CeremonyChallenge>,
    challenge_answered: bool,
    /// The replay memory: ceremony-message key → first-seen instant. Never
    /// forgets within the ceremony's lifetime (a repeated message is a
    /// replay, whatever the clock says now).
    replay_memory: BTreeMap<String, i64>,
    register: Option<Arc<FileIdentityEventRegister>>,
    audit: Arc<dyn AuditSink>,
    clock: Arc<dyn Clock + Send + Sync>,
}

#[derive(Debug, Clone)]
struct CeremonyChallenge {
    nonce_hex: String,
    issued_at_unix_millis: i64,
    expires_at_unix_millis: i64,
}

impl EnrollmentCeremony {
    /// Composes the ceremony for one device identity with its configured
    /// §16.2 binding. `register` is the append-only lifecycle register
    /// (pass `None` only where the deployment has no identity-register
    /// volume — transitions then proceed WITHOUT lifecycle evidence, which
    /// the audits record loudly; the recommended composition always passes
    /// one).
    pub fn new(
        device_identity: String,
        binding: DeviceBinding,
        register: Option<Arc<FileIdentityEventRegister>>,
        audit: Arc<dyn AuditSink>,
        clock: Arc<dyn Clock + Send + Sync>,
    ) -> Result<Self, EnrollmentCeremonyError> {
        if device_identity.trim().is_empty() {
            return Err(EnrollmentCeremonyError::InvalidCeremonyInput {
                reason: "the device identity is mandatory".to_string(),
            });
        }
        if !binding.is_complete() {
            return Err(EnrollmentCeremonyError::InvalidCeremonyInput {
                reason: "the configured tenant/site binding is incomplete (§16.2)".to_string(),
            });
        }
        let ceremony = Self {
            device_identity,
            binding,
            state: CeremonyState::Unenrolled,
            token_record: None,
            challenge: None,
            challenge_answered: false,
            replay_memory: BTreeMap::new(),
            register,
            audit,
            clock,
        };
        ceremony.audit_event(
            "EDGE_ENROLLMENT_CEREMONY_COMPOSED",
            EdgeAuditKind::Startup,
            format!(
                "device='{}' tenant='{}' site='{}' state={} register={} (ceremony structure \
                 only; the enrollment EXECUTION is the recorded UNRUN human gate, P9.33)",
                ceremony.device_identity,
                ceremony.binding.tenant_id,
                ceremony.binding.site_id,
                ceremony.state.name(),
                ceremony.register.is_some(),
            ),
        );
        Ok(ceremony)
    }

    /// The current ceremony state.
    pub fn state(&self) -> CeremonyState {
        self.state
    }

    /// The device identity the ceremony gates.
    pub fn device_identity(&self) -> &str {
        &self.device_identity
    }

    /// The configured §16.2 binding.
    pub fn binding(&self) -> &DeviceBinding {
        &self.binding
    }

    /// THE HUMAN-GATE NOTE (a queryable fact for compositions and reviews):
    /// the ceremony EXECUTION — running a real enrollment against the
    /// platform — is a human gate (ledger P9.33 step 4, recorded UNRUN).
    /// This module executes NO enrollment; it gates and records one.
    pub fn execution_is_a_human_gate() -> &'static str {
        "the enrollment ceremony EXECUTION (one non-production edge, end-to-end, including a \
         failed-enrollment case) is the recorded UNRUN human gate (ledger P9.33 step 4); this \
         module is the ceremony structure, not an executed enrollment"
    }

    // -- BeginRequest: Unenrolled → Requested -----------------------------------

    /// Opens the ceremony: presents the single-use provisioning token (the
    /// P6.06 type — the ceremony never reads its value) with the authority's
    /// token record. Validates issuer attribution, the short-lived ceiling,
    /// and expiry against the INJECTED clock; marks the token spent in the
    /// edge-local single-use ledger (a reused reference is refused). The
    /// ACTUAL token consumption on the platform side remains the P6.06
    /// issuer contract's job.
    pub fn begin_request(
        &mut self,
        token: &ProvisioningToken,
        token_record: EnrollmentTokenRecord,
        single_use: &mut TokenSingleUseLedger,
    ) -> Result<(), EnrollmentCeremonyError> {
        if self.state != CeremonyState::Unenrolled {
            return Err(EnrollmentCeremonyError::InvalidTransition {
                from: self.state.name(),
                transition: CeremonyTransition::BeginRequest.name(),
                detail: "a ceremony request can only begin from UNENROLLED".to_string(),
            });
        }
        // Presence without value: the ceremony proves a token was presented
        // (the P6.06 constructor already refused an empty one) and never
        // calls expose().
        let _token_present = token;
        if token_record.issued_by.trim().is_empty() {
            return Err(EnrollmentCeremonyError::EnrollmentTokenIssuerUnnamed);
        }
        if token_record.token_reference.trim().is_empty() {
            return Err(EnrollmentCeremonyError::InvalidCeremonyInput {
                reason: "the token record carries no token reference".to_string(),
            });
        }
        let now = self.now_unix_millis();
        let lifetime = token_record.lifetime_millis();
        if lifetime <= 0 {
            return Err(EnrollmentCeremonyError::InvalidCeremonyInput {
                reason: "the token record expires at or before its issue instant".to_string(),
            });
        }
        if lifetime > DEFAULT_MAX_TOKEN_LIFETIME_MILLIS {
            return Err(EnrollmentCeremonyError::EnrollmentTokenLifetimeExcessive {
                lifetime_millis: lifetime,
                maximum_millis: DEFAULT_MAX_TOKEN_LIFETIME_MILLIS,
            });
        }
        if token_record.expires_at_unix_millis <= now {
            return Err(EnrollmentCeremonyError::EnrollmentTokenExpired {
                expires_at_unix_millis: token_record.expires_at_unix_millis,
                now_unix_millis: now,
            });
        }
        single_use
            .mark_spent(&token_record.token_reference, now)
            .map_err(|error| {
                self.audit_event(
                    "EDGE_ENROLLMENT_CEREMONY_REFUSED",
                    EdgeAuditKind::EnrollmentRejected,
                    format!(
                        "SPENT-TOKEN REUSE refused (ledger P9.33 step 2): device='{}' err={} \
                         ({} at {}) — the platform-side consumption remains authoritative; \
                         this is the edge-local guard",
                        self.device_identity,
                        error.name(),
                        error,
                        now
                    ),
                );
                error
            })?;
        self.token_record = Some(token_record);
        self.state = CeremonyState::Requested;
        self.append_register(
            IdentityEventKind::Requested,
            now,
            &self.token_record
                .as_ref()
                .map(|t| t.issued_by.clone())
                .unwrap_or_default(),
            "",
            &self.token_record
                .as_ref()
                .map(|t| t.token_reference.clone())
                .unwrap_or_default(),
        )?;
        self.audit_event(
            "EDGE_ENROLLMENT_CEREMONY_REQUESTED",
            EdgeAuditKind::EnrollmentCompleted,
            format!(
                "device='{}' token_ref='{}' issued_by='{}' expires_at={} (single-use, \
                 short-lived; ledger P9.33 step 1)",
                self.device_identity,
                self.token_record.as_ref().map(|t| t.token_reference.as_str()).unwrap_or(""),
                self.token_record.as_ref().map(|t| t.issued_by.as_str()).unwrap_or(""),
                self.token_record.as_ref().map(|t| t.expires_at_unix_millis).unwrap_or(0),
            ),
        );
        Ok(())
    }

    // -- IssueChallenge: Requested → Challenged ---------------------------------

    /// Issues the nonce challenge. The nonce is derived from the device
    /// identity, token reference, binding, the INJECTED clock instant, and
    /// the caller's entropy seed (`derive_challenge_nonce`; the CSPRNG
    /// replacement is the recorded DEPLOYMENT REQUIREMENT). The challenge
    /// carries the TTL [`DEFAULT_CHALLENGE_TTL_MILLIS`].
    pub fn issue_challenge(&mut self, entropy_seed: &str) -> Result<String, EnrollmentCeremonyError> {
        if self.state != CeremonyState::Requested {
            return Err(EnrollmentCeremonyError::InvalidTransition {
                from: self.state.name(),
                transition: CeremonyTransition::IssueChallenge.name(),
                detail: "a challenge can only be issued while the ceremony is REQUESTED"
                    .to_string(),
            });
        }
        if entropy_seed.trim().is_empty() {
            return Err(EnrollmentCeremonyError::InvalidCeremonyInput {
                reason: "the challenge entropy seed is mandatory (an empty seed is not entropy)"
                    .to_string(),
            });
        }
        let now = self.now_unix_millis();
        let token_reference = self
            .token_record
            .as_ref()
            .map(|t| t.token_reference.clone())
            .unwrap_or_default();
        let nonce_hex = derive_challenge_nonce(
            &self.device_identity,
            &token_reference,
            &self.binding,
            now,
            entropy_seed,
        );
        self.challenge = Some(CeremonyChallenge {
            nonce_hex: nonce_hex.clone(),
            issued_at_unix_millis: now,
            expires_at_unix_millis: now + DEFAULT_CHALLENGE_TTL_MILLIS,
        });
        self.challenge_answered = false;
        self.state = CeremonyState::Challenged;
        self.audit_event(
            "EDGE_ENROLLMENT_CEREMONY_CHALLENGED",
            EdgeAuditKind::Startup,
            format!(
                "device='{}' challenge issued (nonce_hex={}, ttl={} ms; test-grade derivation, \
                 CSPRNG is the recorded DEPLOYMENT REQUIREMENT)",
                self.device_identity, nonce_hex, DEFAULT_CHALLENGE_TTL_MILLIS
            ),
        );
        Ok(nonce_hex)
    }

    // -- AnswerChallenge: Challenged → Challenged (answered) --------------------

    /// Answers the challenge. Refuses a wrong nonce, an expired challenge
    /// (INJECTED clock), and a REPLAYED ceremony message (the same message
    /// key — kind + nonce — presented twice, wherever in the ceremony it
    /// reappears). On success the challenge is marked answered and approval
    /// becomes possible.
    pub fn answer_challenge(&mut self, nonce_presented: &str) -> Result<(), EnrollmentCeremonyError> {
        if self.state != CeremonyState::Challenged {
            return Err(EnrollmentCeremonyError::InvalidTransition {
                from: self.state.name(),
                transition: CeremonyTransition::AnswerChallenge.name(),
                detail: "a challenge can only be answered while the ceremony is CHALLENGED"
                    .to_string(),
            });
        }
        if nonce_presented.trim().is_empty() {
            return Err(EnrollmentCeremonyError::InvalidCeremonyInput {
                reason: "the presented nonce is mandatory".to_string(),
            });
        }
        let now = self.now_unix_millis();
        let challenge = self
            .challenge
            .as_ref()
            .expect("challenge exists in the CHALLENGED state");
        let message_key = canonical_ceremony_message_binding(
            "challenge-response",
            &self.device_identity,
            nonce_presented,
            challenge.issued_at_unix_millis,
        );
        if let Some(first_seen_at) = self.replay_memory.get(&message_key) {
            let error = EnrollmentCeremonyError::CeremonyMessageReplayed {
                message_key: message_key.clone(),
                first_seen_at_unix_millis: *first_seen_at,
            };
            self.audit_event(
                "EDGE_ENROLLMENT_CEREMONY_REFUSED",
                EdgeAuditKind::EnrollmentRejected,
                format!(
                    "REPLAYED ceremony message refused (ledger P9.33 replay protection): \
                     device='{}' err={} — first seen at {}",
                    self.device_identity,
                    error.name(),
                    first_seen_at
                ),
            );
            return Err(error);
        }
        self.replay_memory.insert(message_key, challenge.issued_at_unix_millis);
        if challenge.expires_at_unix_millis <= now {
            let error = EnrollmentCeremonyError::ChallengeExpired {
                expires_at_unix_millis: challenge.expires_at_unix_millis,
                now_unix_millis: now,
            };
            self.audit_event(
                "EDGE_ENROLLMENT_CEREMONY_REFUSED",
                EdgeAuditKind::EnrollmentRejected,
                format!("device='{}' err={}", self.device_identity, error),
            );
            return Err(error);
        }
        if nonce_presented != challenge.nonce_hex {
            self.audit_event(
                "EDGE_ENROLLMENT_CEREMONY_REFUSED",
                EdgeAuditKind::EnrollmentRejected,
                format!(
                    "device='{}' err={} (a wrong nonce is refused, never retried-blind)",
                    self.device_identity,
                    EnrollmentCeremonyError::ChallengeNonceMismatch.name()
                ),
            );
            return Err(EnrollmentCeremonyError::ChallengeNonceMismatch);
        }
        self.challenge_answered = true;
        self.audit_event(
            "EDGE_ENROLLMENT_CEREMONY_CHALLENGE_ANSWERED",
            EdgeAuditKind::Startup,
            format!(
                "device='{}' challenge answered; awaiting the PLATFORM approval (the human \
                 gate)",
                self.device_identity
            ),
        );
        Ok(())
    }

    // -- Approve: Challenged → Approved (THE platform human gate) ----------------

    /// Records the platform-side approval (Challenged → Approved). The
    /// ceremony NEVER generates an approval: it validates the presented
    /// [`CeremonyApproval`] (approver, authority reference, device, scope),
    /// refuses a self-approval (the actor IS the device), and refuses an
    /// unanswered challenge. This call is the RECORDED gate — the actual
    /// platform-side decision is made by a human/authority on the control
    /// plane, not here.
    pub fn approve(&mut self, approval: &CeremonyApproval) -> Result<(), EnrollmentCeremonyError> {
        if self.state != CeremonyState::Challenged {
            return Err(EnrollmentCeremonyError::InvalidTransition {
                from: self.state.name(),
                transition: CeremonyTransition::Approve.name(),
                detail: "approval applies only to a CHALLENGED ceremony".to_string(),
            });
        }
        if !self.challenge_answered {
            return Err(EnrollmentCeremonyError::ChallengeNotAnswered);
        }
        approval
            .validate_against(&self.device_identity, &self.binding)
            .map_err(|error| {
                self.audit_event(
                    "EDGE_ENROLLMENT_CEREMONY_REFUSED",
                    EdgeAuditKind::EnrollmentRejected,
                    format!("device='{}' err={}", self.device_identity, error),
                );
                error
            })?;
        // THE NO-SELF-APPROVAL PATH: the approver must be a DISTINCT
        // platform-side subject — the device approving itself (or the
        // ceremony being asked to approve "as itself") is refused by name.
        if approval.approver.trim().eq_ignore_ascii_case(&self.device_identity) {
            let error = EnrollmentCeremonyError::SelfApprovalRefused {
                reason: format!(
                    "the approval actor names the device itself ('{}'); a device cannot \
                     approve its own enrollment",
                    self.device_identity
                ),
            };
            self.audit_event(
                "EDGE_ENROLLMENT_CEREMONY_REFUSED",
                EdgeAuditKind::EnrollmentRejected,
                format!("device='{}' err={}", self.device_identity, error),
            );
            return Err(error);
        }
        self.state = CeremonyState::Approved;
        self.append_register(
            IdentityEventKind::Approved,
            self.now_unix_millis(),
            &approval.approver,
            "",
            &approval.authority_reference,
        )?;
        self.audit_event(
            "EDGE_ENROLLMENT_CEREMONY_APPROVED",
            EdgeAuditKind::EnrollmentCompleted,
            format!(
                "device='{}' approved_by='{}' authority_ref='{}' (platform-side human gate \
                 recorded; ledger P9.33)",
                self.device_identity, approval.approver, approval.authority_reference
            ),
        );
        Ok(())
    }

    // -- Activate: Approved → Active ----------------------------------------------

    /// Activates the identity with the issued P6.06 credential record
    /// (Approved → Active). The record must pass the P6.06 validation, name
    /// THIS device, and match the ceremony's configured binding (the
    /// wrong-site refusal — the ledger's failed-enrollment case). The
    /// exchange that PRODUCED the record was the P6.06 orchestrator's; the
    /// ceremony only gates and records its acceptance.
    pub fn activate(&mut self, credential: &DeviceCredentialRecord) -> Result<(), EnrollmentCeremonyError> {
        if self.state != CeremonyState::Approved {
            return Err(EnrollmentCeremonyError::InvalidTransition {
                from: self.state.name(),
                transition: CeremonyTransition::Activate.name(),
                detail: "activation applies only to an APPROVED ceremony".to_string(),
            });
        }
        credential
            .validate()
            .map_err(|error| EnrollmentCeremonyError::CredentialInvalid {
                reason: error.to_string(),
            })?;
        if credential.device_id != self.device_identity {
            return Err(EnrollmentCeremonyError::BindingMismatch {
                reason: format!(
                    "the credential names device '{}' but the ceremony gates '{}'",
                    credential.device_id, self.device_identity
                ),
            });
        }
        if credential.tenant_id != self.binding.tenant_id
            || credential.site_id != self.binding.site_id
        {
            return Err(EnrollmentCeremonyError::BindingMismatch {
                reason: format!(
                    "the credential binds tenant/site '{}/{}' but the ceremony's configured \
                     binding is '{}/{}' (the WRONG-SITE refusal, ledger P9.33 step 4)",
                    credential.tenant_id,
                    credential.site_id,
                    self.binding.tenant_id,
                    self.binding.site_id
                ),
            });
        }
        let now = self.now_unix_millis();
        self.state = CeremonyState::Active;
        self.append_register(
            IdentityEventKind::Activated,
            now,
            "ceremony",
            "",
            &credential.device_id,
        )?;
        self.audit_event(
            "EDGE_ENROLLMENT_CEREMONY_ACTIVATED",
            EdgeAuditKind::EnrollmentCompleted,
            format!(
                "device='{}' activated into tenant/site '{}/{}' (certificate issued: {}); \
                 identity lifecycle: ACTIVE",
                self.device_identity,
                self.binding.tenant_id,
                self.binding.site_id,
                credential.certificate_pem.is_some()
            ),
        );
        Ok(())
    }

    // -- Revoke: Requested/Challenged/Approved/Active → Revoked --------------------

    /// Revokes the identity (compromise, decommission, or authority
    /// action). Allowed from every live state except UNENROLLED (nothing
    /// to revoke) and REVOKED (already revoked — the named
    /// [`EnrollmentCeremonyError::AlreadyRevoked`]). Actor and reason are
    /// mandatory; the revocation is APPENDED to the register — the
    /// identity's history keeps both the active and the revoked terms.
    pub fn revoke(&mut self, actor: &str, reason: &str) -> Result<(), EnrollmentCeremonyError> {
        if self.state == CeremonyState::Unenrolled {
            return Err(EnrollmentCeremonyError::InvalidTransition {
                from: self.state.name(),
                transition: CeremonyTransition::Revoke.name(),
                detail: "an UNENROLLED identity has nothing to revoke".to_string(),
            });
        }
        if self.state == CeremonyState::Revoked {
            return Err(EnrollmentCeremonyError::AlreadyRevoked);
        }
        if actor.trim().is_empty() || reason.trim().is_empty() {
            return Err(EnrollmentCeremonyError::InvalidCeremonyInput {
                reason: "the revoking actor and the reason are mandatory (an unattributable \
                         or unexplained revocation is refused)"
                    .to_string(),
            });
        }
        let now = self.now_unix_millis();
        let previous = self.state.name();
        self.state = CeremonyState::Revoked;
        self.append_register(IdentityEventKind::Revoked, now, actor, reason, "")?;
        self.audit_event(
            "EDGE_IDENTITY_REVOKED",
            EdgeAuditKind::EnrollmentRejected,
            format!(
                "device='{}' revoked by='{}' from state={} reason='{}' (append-only register \
                 event; the revoked term stays in the history)",
                self.device_identity, actor, previous, reason
            ),
        );
        Ok(())
    }

    // -- ResetForReEnrollment: Revoked → Unenrolled ---------------------------------

    /// Opens re-enrollment after compromise (Revoked → Unenrolled). The
    /// authorization basis MUST be a fresh platform-authority ticket
    /// ([`ReEnrollmentAuthorization::PlatformAuthorityTicket`]); a
    /// `PriorIdentity` basis — the compromised identity vouching for its
    /// successor — is refused by name. The reason is mandatory. The FRESH
    /// TOKEN for the new term is presented through the NORMAL
    /// `begin_request` afterwards (so the single-use ledger consumes it
    /// exactly once, and the issuer/lifetime/expiry discipline of ledger
    /// step 2 applies to re-enrollment tokens identically — there is no
    /// second token path to maintain). After this call the ceremony is
    /// UNENROLLED; the register keeps the whole history (revoked term +
    /// re-enrollment record).
    pub fn begin_re_enrollment(
        &mut self,
        authorization: &ReEnrollmentAuthorization,
        reason: &str,
    ) -> Result<(), EnrollmentCeremonyError> {
        if self.state != CeremonyState::Revoked {
            return Err(EnrollmentCeremonyError::InvalidTransition {
                from: self.state.name(),
                transition: CeremonyTransition::ResetForReEnrollment.name(),
                detail: "re-enrollment after compromise applies only to a REVOKED identity"
                    .to_string(),
            });
        }
        if reason.trim().is_empty() {
            return Err(EnrollmentCeremonyError::InvalidCeremonyInput {
                reason: "the re-enrollment reason is mandatory (compromise re-enrollment is \
                         never silent)"
                    .to_string(),
            });
        }
        let ticket = match authorization {
            ReEnrollmentAuthorization::PlatformAuthorityTicket { ticket_reference } => {
                if ticket_reference.trim().is_empty() {
                    return Err(EnrollmentCeremonyError::InvalidCeremonyInput {
                        reason: "the platform authority ticket reference is mandatory"
                            .to_string(),
                    });
                }
                ticket_reference.clone()
            }
            ReEnrollmentAuthorization::PriorIdentity { reference } => {
                let error = EnrollmentCeremonyError::CompromisedIdentityVouchingRefused {
                    reason: format!(
                        "the re-enrollment was authorized BY the prior (revoked) identity \
                         reference (presented, value not echoed); the ledger's named risk \
                         're-enrollment that trusts the compromised identity to vouch for \
                         the new one' is refused structurally"
                    ),
                };
                self.audit_event(
                    "EDGE_ENROLLMENT_CEREMONY_REFUSED",
                    EdgeAuditKind::EnrollmentRejected,
                    format!(
                        "device='{}' err={} (prior-identity basis refused; the presented \
                         reference length was {} bytes)",
                        self.device_identity,
                        error.name(),
                        reference.len()
                    ),
                );
                return Err(error);
            }
        };
        let now = self.now_unix_millis();
        self.token_record = None;
        self.challenge = None;
        self.challenge_answered = false;
        self.state = CeremonyState::Unenrolled;
        self.append_register(IdentityEventKind::ReEnrollmentOpened, now, "platform-authority", reason, &ticket)?;
        self.audit_event(
            "EDGE_RE_ENROLLMENT_OPENED",
            EdgeAuditKind::Startup,
            format!(
                "device='{}' re-enrollment opened (authority ticket recorded; the revoked \
                 identity did NOT authorize it; the fresh token goes through the NORMAL \
                 single-use/short-lived `begin_request` discipline; reason='{}')",
                self.device_identity, reason
            ),
        );
        Ok(())
    }

    // -- Internals -----------------------------------------------------------------

    fn now_unix_millis(&self) -> i64 {
        let stamp = self.clock.now_ingest_time();
        i64::try_from(stamp.unix_millis()).unwrap_or(i64::MAX)
    }

    /// Appends one lifecycle event to the composed register. Fail-closed:
    /// a register write failure is the named [`EnrollmentCeremonyError::RegisterIo`]
    /// and the CALLER's transition has already been applied — the audit
    /// line records the evidence gap loudly (the transition happened; the
    /// register could not prove it).
    fn append_register(
        &self,
        kind: IdentityEventKind,
        at_unix_millis: i64,
        actor: &str,
        reason: &str,
        reference: &str,
    ) -> Result<(), EnrollmentCeremonyError> {
        let Some(register) = &self.register else {
            self.audit_event(
                "EDGE_IDENTITY_REGISTER_ABSENT",
                EdgeAuditKind::Startup,
                format!(
                    "device='{}' lifecycle event {} had NO register to append to (the \
                     deployment composed no identity register; the audit line is the only \
                     evidence)",
                    self.device_identity,
                    kind.name()
                ),
            );
            return Ok(());
        };
        let result = register.append(kind, at_unix_millis, actor, reason, reference);
        if let Err(error) = &result {
            self.audit_event(
                "EDGE_IDENTITY_REGISTER_WRITE_FAILED",
                EdgeAuditKind::Startup,
                format!(
                    "device='{}' lifecycle event {} FAILED to persist: {} — the transition \
                     happened but the register could not prove it",
                    self.device_identity,
                    kind.name(),
                    error
                ),
            );
        }
        result
    }

    /// Records one audit event through the edge `AuditSink` (the
    /// dedicated-event-name-leads-detail pattern on the closed
    /// `EdgeAuditKind` vocabulary, as in P6.14/P9.31/P9.32).
    fn audit_event(&self, name: &'static str, kind: EdgeAuditKind, detail: String) {
        let event = EdgeAuditEvent {
            recorded_at: self.clock.now_ingest_time(),
            kind,
            detail: format!("{name}: {detail}"),
        };
        if let Err(error) = self.audit.record(event) {
            tracing::error!(%error, audit_event = name, "enrollment-ceremony audit write FAILED");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: i64 = 1_700_000_000_000;

    fn binding() -> DeviceBinding {
        DeviceBinding::new("tenant-a".to_string(), "pilot-site-A".to_string())
    }

    #[test]
    fn the_state_names_and_the_digest_genesis_are_the_recorded_constants() {
        // Hand-computed known answers: the closed state names, and the
        // FNV-1a-64 genesis (the definitional offset basis 0xcbf29ce484222325
        // — the only digest constant hand-derivable without a 64-bit
        // multiply; the full-digest known answers are the canonical STRING
        // assertions below, hand-derived from the documented format).
        assert_eq!(CeremonyState::Unenrolled.name(), "UNENROLLED");
        assert_eq!(CeremonyState::Requested.name(), "REQUESTED");
        assert_eq!(CeremonyState::Challenged.name(), "CHALLENGED");
        assert_eq!(CeremonyState::Approved.name(), "APPROVED");
        assert_eq!(CeremonyState::Active.name(), "ACTIVE");
        assert_eq!(CeremonyState::Revoked.name(), "REVOKED");
        assert_eq!(CEREMONY_DIGEST_GENESIS, 0xcbf2_9ce4_8422_2325);
        // The canonical ceremony-message binding is asserted
        // character-for-character (hand-computed from the format).
        assert_eq!(
            canonical_ceremony_message_binding("challenge-response", "edge-01", "0011abcdef004455", 1_700_000_000_123),
            "ceremony-msg:v1|kind:challenge-response|device:edge-01|nonce:0011abcdef004455|at:1700000000123"
        );
    }

    #[test]
    fn the_nonce_derivation_is_deterministic_and_input_sensitive() {
        // Same inputs → the same nonce (the determinism the known-answer
        // discipline needs); any changed input → a different nonce (64-bit
        // FNV-1a: a checksum, not a security primitive — recorded).
        let a = derive_challenge_nonce("edge-01", "tok-1", &binding(), BASE, "seed-1");
        let b = derive_challenge_nonce("edge-01", "tok-1", &binding(), BASE, "seed-1");
        assert_eq!(a, b, "the derivation is deterministic");
        assert_eq!(a.len(), 16, "the nonce is 16 lowercase hex chars");
        assert!(a.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()));
        assert_ne!(
            a,
            derive_challenge_nonce("edge-01", "tok-1", &binding(), BASE, "seed-2"),
            "a changed entropy seed changes the nonce"
        );
        assert_ne!(
            a,
            derive_challenge_nonce("edge-01", "tok-2", &binding(), BASE, "seed-1"),
            "a changed token reference changes the nonce"
        );
        assert_ne!(
            a,
            derive_challenge_nonce("edge-01", "tok-1", &binding(), BASE + 1, "seed-1"),
            "a changed clock instant changes the nonce"
        );
    }
}
