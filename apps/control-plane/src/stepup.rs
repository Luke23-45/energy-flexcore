// P13.04 — The server-side step-up challenge protocol: the ENFORCED form
//         that replaces the UI-only P5.26 flow (`console/src/components/
//         stepUpApproval.tsx` — which records, honestly, that NO
//         server-verified step-up exists; this module is the mechanism
//         LR-048 names as the gap). Decision record: phase-13
//         `records/04_p13_04_step_up_mfa.md`.
//
// WRITTEN-UNRUN: no challenge has ever been issued or verified, no IdP is
// deployed, P13-E-3 stays OPEN (the recorded DEFAULT, IdP-delegated MFA
// primary with TOTP fallback, is implemented as the designed seam only),
// and no console flow consumes this yet (P13.17, wave 2). What is PROVEN
// is only the written surface: the code is complete and pinned by tests
// that have never been compiled or executed — live IdP interop is a HUMAN
// LAB GATE. Nothing here claims runtime behavior. (FIM-F07.06 honesty
// restatement: the pre-FIM wording "no challenge has ever been issued or
// verified" described what the code CAN now do once composed; the proven
// fact is written-unrun, not an eternal impossibility.)
//
// FIM-F07.03 (additive): a REAL verifier implementation
// ([`OidcStepUpVerifier`]) now validates FRESH IdP evidence — a
// recently-minted OIDC token whose authentication instant satisfies the
// registered max-age policy — against the composed IdP through the same
// JWKS port the P13.02 resolver uses. [`DenyAllStepUpVerifier`] REMAINS
// the shipped default: a deployment opts in by composing the IdP-backed
// verifier; absence of composition is a denial, never a pass. The
// APPROVED-gate composition helper ([`require_command_approval_evidence`])
// runs through the EXISTING [`StepUpService::verify`] API — no call-site
// semantics change, no new state, no reference to the command lifecycle.
//
// THE PROTOCOL (the enforced shape; the UI-only flow never grants
// authority — this one is the authority-evidence producer):
//
// 1. ISSUE. Before a high-impact act (the APPROVED gate input — a P4.13
//    `record_command_approval` call — or a registered high-impact console
//    action), the principal requests a step-up challenge bound to
//    (subject, tenant, purpose). The challenge is single-use,
//    time-bounded (DEFAULT TTL 5 minutes — a REGISTERED RECOMMENDATION),
//    and carries no authority of its own.
// 2. VERIFY. The principal answers the challenge with FRESH
//    authentication evidence produced by the IdP (the P13-E-3 DEFAULT:
//    the IdP owns the authenticator — MFA happens at the IdP; the
//    challenge consumes the IdP's authentication result). Verification is
//    delegated to the injected [`StepUpVerifier`]; nothing in this module
//    verifies MFA factors itself.
// 3. CONSUME. A VERIFIED step-up yields a single-use evidence token the
//    gated operation presents with its request (e.g. at the authorization
//    gate). The evidence token is bound to the same (subject, tenant,
//    purpose) triple and expires with the challenge window.
//
// FAILURE SEMANTICS (the plan-risk-3 rule, enforced structurally): a
// FAILED, MISSING, EXPIRED, or REPLAYED challenge is a RECORDED DENIAL —
// a named `StepUpDenial` value the caller audits — and NOTHING advances.
// At the command surface this means the command stays FEASIBLE and
// unapproved, exactly as in the no-step-up world; the approval gate can
// never wedge: there is no state in this module that holds a command, and
// the module has NO reference to the command lifecycle at all (the
// 14-state / 27-edge set in flexcore-domain is untouched by construction).
//
// DEFAULT-WIRING RULE (fail closed, the state.rs pattern): the shipped
// verifier is [`DenyAllStepUpVerifier`] — every verification denies with a
// named reason. A deployment opts in by composing the IdP-backed verifier;
// absence of composition is a denial, never a silent pass.

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::time::Duration;

use flexcore_contracts::ids::FlexUuid;
use flexcore_domain::ids::TenantId;

/// The DEFAULT challenge TTL. REGISTERED RECOMMENDATION (X-8: registered
/// before any run that could tune it): 5 minutes — long enough to complete
/// an IdP round-trip, short enough to bound the window a stolen evidence
/// token is usable in. Not a tested optimum.
pub const DEFAULT_STEP_UP_TTL: Duration = Duration::from_secs(5 * 60);

// --- The challenge ------------------------------------------------------------

/// The purpose a challenge is bound to. A closed vocabulary: a challenge
/// issued for one purpose can NEVER be consumed by another (the binding is
/// enforced at verification). Extension = a new variant (the closed-
/// vocabulary extension rule), never a free-form string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepUpPurpose {
    /// The APPROVED-gate input: authorizing one FEASIBLE command (the P4.13
    /// `record_command_approval` seam; the command id travels as the
    /// target). Binding the challenge to the command id means an evidence
    /// token for command A can never approve command B.
    CommandApproval {
        /// The command the approval targets (canonical UUID text).
        command_id: String,
    },
    /// A registered high-impact console action (the P13.17 wave-2 wiring
    /// consumes this; the action id is a stable surface id, not free text).
    ConsoleAction {
        /// The registered action id (e.g. `apikey.rotate`, `tenant.delete`).
        action_id: String,
    },
}

impl StepUpPurpose {
    /// Whether two purposes are the SAME binding (exact match on variant
    /// and target — a command-approval purpose never matches a
    /// console-action purpose or another command).
    pub fn same_binding(&self, other: &StepUpPurpose) -> bool {
        self == other
    }

    /// The recorded text form (audit evidence).
    pub fn recorded_text(&self) -> String {
        match self {
            StepUpPurpose::CommandApproval { command_id } => {
                format!("command-approval:{command_id}")
            }
            StepUpPurpose::ConsoleAction { action_id } => {
                format!("console-action:{action_id}")
            }
        }
    }
}

/// A step-up challenge: an issued, unverified, single-use, time-bounded
/// request for fresh authentication. It grants NOTHING.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepUpChallenge {
    /// The challenge id (unique per issue).
    pub challenge_id: String,
    /// The principal the challenge is bound to.
    pub subject: String,
    /// The tenant the principal is acting in.
    pub tenant: TenantId,
    /// The purpose the challenge is bound to.
    pub purpose: StepUpPurpose,
    /// The issue instant (controlled clock, UTC millis).
    pub issued_at_unix_millis: i64,
    /// The expiry instant (issue + the configured TTL).
    pub expires_at_unix_millis: i64,
}

/// The single-use evidence a VERIFIED challenge yields. Bound to the
/// challenge's (subject, tenant, purpose) triple; consumed exactly once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepUpEvidence {
    /// The evidence id (unique per verified challenge).
    pub evidence_id: String,
    /// The challenge it came from.
    pub challenge_id: String,
    /// The subject (the same the challenge was bound to).
    pub subject: String,
    /// The tenant (the same the challenge was bound to).
    pub tenant: TenantId,
    /// The purpose (the same the challenge was bound to).
    pub purpose: StepUpPurpose,
    /// The verification instant (controlled clock, UTC millis).
    pub verified_at_unix_millis: i64,
    /// The evidence expiry (the challenge window's close — the evidence
    /// never outlives its challenge).
    pub expires_at_unix_millis: i64,
}

// --- Verification -------------------------------------------------------------

/// The verification port. The P13-E-3 DEFAULT (IdP-delegated MFA primary,
/// TOTP documented fallback) is implemented HERE ONLY as this port: the
/// IdP-backed verifier is a deployment artifact (no IdP exists — UNRUN);
/// the TOTP fallback would be a separate verifier implementation with its
/// own reviewed secret storage (P11.16 posture) — deliberately NOT built
/// in this batch.
pub trait StepUpVerifier: Send + Sync {
    /// Verifies the presented authentication evidence for one challenge.
    /// `evidence` is the fresh authentication artifact the principal
    /// produced at the IdP (opaque to this module; the verifier owns its
    /// format). A returned denial reason is recorded verbatim.
    fn verify(
        &self,
        challenge: &StepUpChallenge,
        presented: &str,
    ) -> Result<(), StepUpDenial>;
}

/// The shipped DEFAULT verifier: denies everything (fail closed). A
/// deployment composes the IdP-backed verifier; absence of composition is
/// a denial, never a pass.
pub struct DenyAllStepUpVerifier;

impl StepUpVerifier for DenyAllStepUpVerifier {
    fn verify(&self, _challenge: &StepUpChallenge, _presented: &str) -> Result<(), StepUpDenial> {
        Err(StepUpDenial::VerifierNotComposed)
    }
}

// --- The IdP-delegated verifier (FIM-F07.03) ------------------------------------

/// The DEFAULT freshness window for IdP step-up evidence. REGISTERED
/// RECOMMENDATION (X-8: registered before any run that could tune it):
/// 300 seconds, aligned with the challenge TTL — the evidence must come
/// from an authentication the principal performed DURING the step-up
/// window, not from an old session. No recorded policy precedes it (the
/// P13.04 record pins the TTL, not an evidence max-age); none is a tested
/// optimum.
pub const DEFAULT_STEP_UP_EVIDENCE_MAX_AGE: Duration = Duration::from_secs(300);

/// The claims of the fresh authentication evidence token (an OIDC JWT the
/// IdP minted for the reauthentication). Only the claims below participate;
/// everything else the IdP issues is ignored. Deserialization of the
/// optional claims is lenient (`Option` + defaults) so an IdP that omits a
/// claim yields `None`/empty rather than a parse failure — absence is then
/// handled by the fail-closed rules in [`OidcStepUpVerifier::verify`],
/// never by a panic or a guess.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct StepUpEvidenceClaims {
    /// The issuer. Validated against the configured issuer by the JWT
    /// crate's `Validation` and by the verifier's exact-match gate.
    pub iss: Option<String>,
    /// The audience. Validated against the configured audience by the
    /// crate's `Validation`.
    #[serde(default)]
    pub aud: crate::oidc::AudienceClaim,
    /// The subject of the reauthentication — MUST equal the challenge's
    /// subject (the evidence of another principal never verifies).
    pub sub: Option<String>,
    /// Expiry (seconds since the epoch). Validated by the crate's
    /// `Validation` with the configured leeway.
    pub exp: Option<i64>,
    /// The instant of the ACTUAL authentication event (the OIDC
    /// `auth_time` claim) — the preferred freshness anchor. Absence defers
    /// to `iat`; both absent fails closed.
    pub auth_time: Option<i64>,
    /// The token issue instant (the fallback freshness anchor when the IdP
    /// does not issue `auth_time`).
    pub iat: Option<i64>,
    /// The authentication-methods references the IdP asserts (e.g. `pwd`,
    /// `otp`, `mfa`). Checked against the configured required methods when
    /// the verifier is configured with any.
    #[serde(default)]
    pub amr: Vec<String>,
}

/// The IdP-delegated step-up verifier (FIM-F07.03): validates that the
/// presented evidence is a FRESH, validly-signed token from the configured
/// IdP whose subject matches the challenge. The P13-E-3 DEFAULT (IdP owns
/// the authenticator) consumes the IdP's authentication RESULT — the token;
/// nothing here verifies an MFA factor itself.
///
/// Validation order (every failure is a NAMED [`StepUpDenial`], recorded
/// verbatim by the service):
///
/// 1. The evidence parses as a JWT whose `kid`/`alg` pass the configured
///    allow-list (DEFAULT RS256 only — the same defense as the login
///    path).
/// 2. Signature, `iss` (exact), `aud` (membership), `exp`/`nbf` — the
///    crate's `Validation` with the same recorded policy the P13.02
///    resolver uses (the JWKS arrives through the injected [`JwksSource`]
///    port; the fetch is lazy, cache-TTL governed, and UNRUN here).
/// 3. Subject binding: the token's `sub` must equal the challenge's
///    subject ([`StepUpDenial::SubjectMismatch`] otherwise).
/// 4. Freshness: the authentication instant (`auth_time`, falling back to
///    `iat`) must be no older than the configured max age
///    (DEFAULT [`DEFAULT_STEP_UP_EVIDENCE_MAX_AGE`]; absence of both
///    fails closed). The verifier's clock is INJECTED (the controlled-
///    clock discipline).
/// 5. Optional factor policy: when `required_amr` is non-empty, the
///    token's `amr` must contain at least one required method — a
///    DEPLOYMENT-level loosening/tightening, never a code default (the
///    empty default trusts the IdP's factor policy wholesale, which is
///    exactly the P13-E-3 DEFAULT's delegation).
///
/// The shipped DEFAULT verifier remains [`DenyAllStepUpVerifier`]; this
/// implementation composes ONLY where an operator opted in.
pub struct OidcStepUpVerifier {
    /// The identity-path configuration (issuer, audience, algorithm
    /// allow-list, leeway, JWKS cache TTL) — the SAME [`crate::oidc::OidcConfig`]
    /// shape the login path uses, so an IdP is configured once.
    config: crate::oidc::OidcConfig,
    jwks: crate::oidc::JwksCache,
    max_age: Duration,
    required_amr: Vec<String>,
    now_millis: Box<dyn Fn() -> i64 + Send + Sync>,
}

impl OidcStepUpVerifier {
    /// Builds the verifier. The JWKS source is injected; nothing here
    /// constructs a network stack, and NO fetch happens at construction.
    pub fn new(
        config: crate::oidc::OidcConfig,
        source: Box<dyn crate::oidc::JwksSource>,
        max_age: Duration,
        required_amr: Vec<String>,
        now_millis: Box<dyn Fn() -> i64 + Send + Sync>,
    ) -> Self {
        let ttl = config.jwks_cache_ttl;
        Self {
            config,
            jwks: crate::oidc::JwksCache::new(source, ttl),
            max_age,
            required_amr,
            now_millis,
        }
    }
}

impl StepUpVerifier for OidcStepUpVerifier {
    fn verify(&self, challenge: &StepUpChallenge, presented: &str) -> Result<(), StepUpDenial> {
        // The presented evidence is the token the IdP produced for this
        // reauthentication; an optional `Bearer ` prefix is tolerated
        // (the wire shape), never required.
        let token = presented.strip_prefix("Bearer ").unwrap_or(presented).trim();
        if token.is_empty() {
            return Err(StepUpDenial::EvidenceRejected {
                reason: "no authentication evidence was presented".to_string(),
            });
        }
        let header = jsonwebtoken::decode_header(token).map_err(|_| {
            StepUpDenial::EvidenceRejected {
                reason: "the authentication evidence is not a parseable JWT".to_string(),
            }
        })?;
        let kid = header.kid.ok_or(StepUpDenial::EvidenceRejected {
            reason: "the authentication evidence names no signing key (kid)".to_string(),
        })?;
        if !self.config.algorithms.contains(&header.alg) {
            return Err(StepUpDenial::EvidenceRejected {
                reason: format!(
                    "the evidence algorithm {alg:?} is outside the configured allow-list",
                    alg = header.alg
                ),
            });
        }
        let now_millis = (self.now_millis)();
        let key = self
            .jwks
            .decoding_key(&self.config.issuer, &kid, now_millis)
            .map_err(|e| StepUpDenial::EvidenceRejected {
                reason: format!("the evidence signing key could not be resolved: {e}"),
            })?;
        let mut validation = jsonwebtoken::Validation::new(header.alg);
        validation.set_issuer(&[self.config.issuer.clone()]);
        validation.set_audience(&[self.config.audience.clone()]);
        validation.leeway = self.config.leeway_seconds;
        validation.validate_exp = true;
        validation.validate_nbf = true;
        validation.algorithms = self.config.algorithms.clone();
        let data = jsonwebtoken::decode::<StepUpEvidenceClaims>(
            token,
            &key.into_inner(),
            &validation,
        )
        .map_err(|_| StepUpDenial::EvidenceRejected {
            reason: "the authentication evidence failed signature or claim validation \
                     (issuer/audience/expiry)"
                .to_string(),
        })?;
        let claims = data.claims;
        // The evidence must belong to the presenting principal: a token
        // minted for another subject is a binding failure (named, and the
        // challenge is already consumed by the service at this point — a
        // fresh challenge is the retry path).
        if claims.sub.as_deref() != Some(challenge.subject.as_str()) {
            return Err(StepUpDenial::SubjectMismatch);
        }
        // Freshness: the authentication instant must be inside the window.
        // `auth_time` is the OIDC reauthentication instant; `iat` is the
        // fallback. BOTH absent fails closed (an unverifiable instant is
        // not a fresh one).
        let now_secs = now_millis.div_euclid(1000);
        let anchor = claims
            .auth_time
            .or(claims.iat)
            .ok_or_else(|| StepUpDenial::EvidenceRejected {
                reason: "the evidence carries no authentication instant (auth_time/iat absent)"
                    .to_string(),
            })?;
        let max_age_secs = i64::try_from(self.max_age.as_secs()).unwrap_or(i64::MAX);
        if now_secs.saturating_sub(anchor) > max_age_secs {
            return Err(StepUpDenial::EvidenceRejected {
                reason: format!(
                    "the authentication evidence is stale: authenticated at {anchor}, \
                     older than the {max_age_secs}s max age"
                ),
            });
        }
        // The optional factor policy (the P13-E-3 DEFAULT: the IdP owns the
        // authenticator; an empty requirement trusts it wholesale).
        if !self.required_amr.is_empty()
            && !self
                .required_amr
                .iter()
                .any(|required| claims.amr.iter().any(|presented_method| presented_method == required))
        {
            return Err(StepUpDenial::EvidenceRejected {
                reason: format!(
                    "the evidence authentication methods ({}) do not satisfy the required \
                     methods ({})",
                    claims.amr.join(", "),
                    self.required_amr.join(", ")
                ),
            });
        }
        Ok(())
    }
}

/// FIM-F07.03 — the APPROVED-gate step-up check: the ADDITIVE composition a
/// high-impact approval call site runs THROUGH THE EXISTING service API
/// before consuming the approval. No new state, no call-site semantics
/// change, no reference to the command lifecycle: the function builds the
/// `CommandApproval` purpose (binding the evidence to THE command — an
/// evidence token for command A can never approve command B) and delegates
/// to [`StepUpService::verify`]. A refusal leaves the command FEASIBLE and
/// unapproved — exactly the no-wedge guarantee (the module header). The
/// shipped-default service ([`DenyAllStepUpVerifier`]) refuses EVERY
/// approval, so a deployment that wires this gate without composing an
/// IdP-backed verifier fails closed.
pub fn require_command_approval_evidence(
    service: &StepUpService,
    challenge_id: &str,
    presenting_subject: &str,
    acting_tenant: TenantId,
    presented: &str,
    command_id: &str,
) -> Result<StepUpEvidence, StepUpDenial> {
    service.verify(
        challenge_id,
        presenting_subject,
        acting_tenant,
        presented,
        &StepUpPurpose::CommandApproval {
            command_id: command_id.to_string(),
        },
    )
}

/// A step-up denial: named reasons, every one a RECORDED DENIAL the caller
/// audits. Never a bare bool; never a wedge — a denial leaves the gated
/// operation exactly where it was.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepUpDenial {
    /// No challenge exists for the (subject, purpose) presented.
    NoChallenge,
    /// The challenge expired before verification.
    Expired {
        /// The expiry instant that passed.
        expired_at_unix_millis: i64,
    },
    /// The challenge was already consumed (single-use enforcement — a
    /// replay attempt).
    AlreadyUsed,
    /// The presented evidence does not belong to the presenting subject.
    SubjectMismatch,
    /// The challenge is bound to a different purpose than the one presented.
    PurposeMismatch,
    /// The challenge is bound to a different tenant than the acting one.
    TenantMismatch,
    /// The verifier rejected the authentication evidence (the detail is the
    /// verifier's recorded reason).
    EvidenceRejected { reason: String },
    /// No real verifier is composed (the shipped DEFAULT path).
    VerifierNotComposed,
    /// The challenge store failed (fail-closed bookkeeping).
    StoreError { detail: String },
}

impl std::fmt::Display for StepUpDenial {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StepUpDenial::NoChallenge => write!(f, "no step-up challenge for this subject and purpose"),
            StepUpDenial::Expired { expired_at_unix_millis } => {
                write!(f, "the step-up challenge expired at {expired_at_unix_millis}")
            }
            StepUpDenial::AlreadyUsed => write!(f, "the step-up challenge was already consumed (single-use)"),
            StepUpDenial::SubjectMismatch => write!(f, "the presented evidence belongs to another subject"),
            StepUpDenial::PurposeMismatch => write!(f, "the step-up challenge is bound to another purpose"),
            StepUpDenial::TenantMismatch => write!(f, "the step-up challenge is bound to another tenant"),
            StepUpDenial::EvidenceRejected { reason } => write!(f, "the authentication evidence was rejected: {reason}"),
            StepUpDenial::VerifierNotComposed => write!(f, "no step-up verifier is composed (fail-closed default)"),
            StepUpDenial::StoreError { detail } => write!(f, "the step-up challenge store failed: {detail}"),
        }
    }
}

impl std::error::Error for StepUpDenial {}

// --- The challenge store port ---------------------------------------------------

/// The challenge bookkeeping port. The in-crate [`InMemoryStepUpStore`]
/// serves tests and single-process deployments; the durable store (the
/// Postgres implementation under RLS) is a composition-root artifact —
/// deliberately not built in this batch (no migration was added; the
/// design record names the table shape — a durable implementation must
/// also carry the consumed-id set `take` records, e.g. a
/// `consumed_challenge_ids` table keyed by challenge id, so a replay
/// denies as [`StepUpDenial::AlreadyUsed`] rather than `NoChallenge`).
pub trait StepUpStore: Send + Sync {
    fn insert(&self, challenge: StepUpChallenge) -> Result<(), StepUpDenial>;
    /// Reads the challenge WITHOUT consuming it — the binding checks in
    /// `verify` run against this read, so a mismatched subject, tenant,
    /// or purpose denies without spending the challenge (a failed binding
    /// check must never burn a live challenge belonging to someone else).
    /// `None` when no challenge matches the id.
    fn get(&self, challenge_id: &str) -> Result<Option<StepUpChallenge>, StepUpDenial>;
    /// Takes the challenge out (single-use by construction) AND records
    /// the id as consumed — `None` when no challenge matches the id. A
    /// later replay of a taken id denies as
    /// [`StepUpDenial::AlreadyUsed`] (see [`Self::was_consumed`]).
    fn take(&self, challenge_id: &str) -> Result<Option<StepUpChallenge>, StepUpDenial>;
    /// Whether the id was consumed by an earlier `take` — the replay
    /// check. `false` for ids that were never issued AND for ids dropped
    /// by the TTL sweep ([`Self::discard`]).
    fn was_consumed(&self, challenge_id: &str) -> Result<bool, StepUpDenial>;
    /// Drops the entry WITHOUT recording consumption — the TTL sweep:
    /// an expired challenge was never verified, so a later replay of its
    /// id is "no such challenge", not "already used".
    fn discard(&self, challenge_id: &str) -> Result<(), StepUpDenial>;
}

/// The in-memory store: id → challenge, plus the consumed-id set
/// `take` records (a replay denies as `AlreadyUsed`, never as
/// `NoChallenge`). Bounded by the TTL sweep at verification time
/// (expired entries are dropped when encountered); a production
/// deployment swaps the durable implementation in through the same port.
#[derive(Default)]
pub struct InMemoryStepUpStore {
    entries: Mutex<HashMap<String, StepUpChallenge>>,
    consumed: Mutex<HashSet<String>>,
}

impl StepUpStore for InMemoryStepUpStore {
    fn insert(&self, challenge: StepUpChallenge) -> Result<(), StepUpDenial> {
        let mut entries = self
            .entries
            .lock()
            .map_err(|_| StepUpDenial::StoreError {
                detail: "the in-memory challenge store lock is poisoned".to_string(),
            })?;
        entries.insert(challenge.challenge_id.clone(), challenge);
        Ok(())
    }

    fn get(&self, challenge_id: &str) -> Result<Option<StepUpChallenge>, StepUpDenial> {
        let entries = self
            .entries
            .lock()
            .map_err(|_| StepUpDenial::StoreError {
                detail: "the in-memory challenge store lock is poisoned".to_string(),
            })?;
        Ok(entries.get(challenge_id).cloned())
    }

    fn take(&self, challenge_id: &str) -> Result<Option<StepUpChallenge>, StepUpDenial> {
        let mut entries = self
            .entries
            .lock()
            .map_err(|_| StepUpDenial::StoreError {
                detail: "the in-memory challenge store lock is poisoned".to_string(),
            })?;
        let taken = entries.remove(challenge_id);
        if taken.is_some() {
            // The single-use tombstone: a replay of this id is AlreadyUsed.
            let mut consumed = self.consumed.lock().map_err(|_| StepUpDenial::StoreError {
                detail: "the in-memory challenge store lock is poisoned".to_string(),
            })?;
            consumed.insert(challenge_id.to_string());
        }
        Ok(taken)
    }

    fn was_consumed(&self, challenge_id: &str) -> Result<bool, StepUpDenial> {
        let consumed = self
            .consumed
            .lock()
            .map_err(|_| StepUpDenial::StoreError {
                detail: "the in-memory challenge store lock is poisoned".to_string(),
            })?;
        Ok(consumed.contains(challenge_id))
    }

    fn discard(&self, challenge_id: &str) -> Result<(), StepUpDenial> {
        let mut entries = self
            .entries
            .lock()
            .map_err(|_| StepUpDenial::StoreError {
                detail: "the in-memory challenge store lock is poisoned".to_string(),
            })?;
        entries.remove(challenge_id);
        Ok(())
    }
}

// --- The service -----------------------------------------------------------------

/// The step-up service: issue + verify over the injected store, verifier,
/// id generator, and controlled clock. This struct holds NO reference to
/// the command lifecycle — a failed verification cannot affect a command's
/// state, which is exactly the no-wedge guarantee.
pub struct StepUpService {
    store: Box<dyn StepUpStore>,
    verifier: Box<dyn StepUpVerifier>,
    ids: Box<dyn Fn() -> String + Send + Sync>,
    now_millis: Box<dyn Fn() -> i64 + Send + Sync>,
    ttl: Duration,
}

impl StepUpService {
    /// Builds the service with the shipped fail-closed DEFAULT verifier
    /// unless a real one is composed.
    pub fn new(
        store: Box<dyn StepUpStore>,
        verifier: Box<dyn StepUpVerifier>,
        ids: Box<dyn Fn() -> String + Send + Sync>,
        now_millis: Box<dyn Fn() -> i64 + Send + Sync>,
        ttl: Duration,
    ) -> Self {
        Self {
            store,
            verifier,
            ids,
            now_millis,
            ttl,
        }
    }

    /// Issues a challenge bound to (subject, tenant, purpose). The challenge
    /// grants nothing; it is the request for fresh authentication.
    pub fn issue(
        &self,
        subject: &str,
        tenant: TenantId,
        purpose: StepUpPurpose,
    ) -> Result<StepUpChallenge, StepUpDenial> {
        if subject.trim().is_empty() {
            return Err(StepUpDenial::SubjectMismatch);
        }
        let now = (self.now_millis)();
        let ttl_millis = i64::try_from(self.ttl.as_millis()).unwrap_or(i64::MAX);
        let challenge = StepUpChallenge {
            challenge_id: (self.ids)(),
            subject: subject.to_string(),
            tenant,
            purpose,
            issued_at_unix_millis: now,
            expires_at_unix_millis: now.saturating_add(ttl_millis),
        };
        self.store.insert(challenge.clone())?;
        Ok(challenge)
    }

    /// Verifies the presented evidence for a challenge. On success the
    /// challenge is consumed (single-use) and the single-use evidence value
    /// is returned; on failure a named [`StepUpDenial`] is returned and the
    /// world is exactly as it was before the call — except that a replay
    /// of a CONSUMED challenge denies as [`StepUpDenial::AlreadyUsed`] on
    /// every later attempt (the store remembers consumed ids; a
    /// never-issued id still denies as `NoChallenge`).
    pub fn verify(
        &self,
        challenge_id: &str,
        presenting_subject: &str,
        acting_tenant: TenantId,
        presented: &str,
        requested_purpose: &StepUpPurpose,
    ) -> Result<StepUpEvidence, StepUpDenial> {
        let now = (self.now_millis)();
        // The binding checks run against a NON-CONSUMING read: a
        // mismatched subject, tenant, or purpose is a named denial
        // without spending the challenge (the old form took the challenge
        // first, so the first binding failure burned it and every later
        // check on the same challenge reported NoChallenge instead of
        // the binding it actually violates) and without spending the
        // verifier's judgment as oracle material.
        let challenge = match self.store.get(challenge_id)? {
            Some(challenge) => challenge,
            // Gone: consumed (a replay — AlreadyUsed) or never issued /
            // swept (NoChallenge). The distinction is the audit trail: a
            // replay attempt is an attack signal, not a typo.
            None => {
                if self.store.was_consumed(challenge_id)? {
                    return Err(StepUpDenial::AlreadyUsed);
                }
                return Err(StepUpDenial::NoChallenge);
            }
        };
        if challenge.subject != presenting_subject {
            return Err(StepUpDenial::SubjectMismatch);
        }
        if challenge.tenant != acting_tenant {
            return Err(StepUpDenial::TenantMismatch);
        }
        if !challenge.purpose.same_binding(requested_purpose) {
            return Err(StepUpDenial::PurposeMismatch);
        }
        if challenge.expires_at_unix_millis <= now {
            // Expired entries are dropped when encountered (the store's
            // TTL-sweep discipline) — discarded, NOT taken: expiry is not
            // consumption, so a later replay of this id is "no such
            // challenge", not "already used".
            let _ = self.store.discard(challenge_id);
            return Err(StepUpDenial::Expired {
                expired_at_unix_millis: challenge.expires_at_unix_millis,
            });
        }
        // Single-use consumption happens here, once the binding and the
        // lifetime have passed: a verifier rejection consumes the
        // challenge (a challenge whose evidence was rejected must not be
        // re-presentable), and so does a success.
        // Taken between the `get` above and here only by a concurrent
        // consumption — which is, by definition, AlreadyUsed.
        let challenge = self
            .store
            .take(challenge_id)?
            .ok_or(StepUpDenial::AlreadyUsed)?;
        // The verifier's judgment — the IdP-delegated authentication
        // result (P13-E-3 DEFAULT; the shipped default denies).
        self.verifier.verify(&challenge, presented)?;
        Ok(StepUpEvidence {
            evidence_id: (self.ids)(),
            challenge_id: challenge.challenge_id,
            subject: challenge.subject,
            tenant: challenge.tenant,
            purpose: challenge.purpose,
            verified_at_unix_millis: now,
            expires_at_unix_millis: challenge.expires_at_unix_millis,
        })
    }
}

/// Generates a challenge/evidence id from a UUID (the composition root's
/// id-generator seam in function form). Named helper so call sites stay
/// uniform; the id format is opaque downstream.
pub fn id_from_uuid(uuid: FlexUuid) -> String {
    format!("stepup:{uuid}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicI64, Ordering};

    fn tenant() -> TenantId {
        TenantId::new(
            flexcore_contracts::ids::FlexUuid::parse("0192a5e0-0000-7e2f-9a1b-3c4d5e6f9b01")
                .unwrap(),
        )
    }

    fn other_tenant() -> TenantId {
        TenantId::new(
            flexcore_contracts::ids::FlexUuid::parse("0192a5e0-0000-7e2f-9a1b-3c4d5e6f9b02")
                .unwrap(),
        )
    }

    fn purpose() -> StepUpPurpose {
        StepUpPurpose::CommandApproval {
            command_id: "0192a5e0-0000-7e2f-9a1b-3c4d5e6f9b03".to_string(),
        }
    }

    fn other_purpose() -> StepUpPurpose {
        StepUpPurpose::ConsoleAction {
            action_id: "apikey.rotate".to_string(),
        }
    }

    /// A counter-based clock (the controlled-clock discipline; every test
    /// advances time explicitly). Shared into the service through an `Arc`
    /// closure — no unsafe anywhere in this module.
    #[derive(Clone)]
    struct TestClock(std::sync::Arc<AtomicI64>);

    impl TestClock {
        fn new() -> Self {
            Self(std::sync::Arc::new(AtomicI64::new(1_000)))
        }
        fn now(&self) -> i64 {
            self.0.load(Ordering::SeqCst)
        }
        fn advance(&self, millis: i64) {
            self.0.fetch_add(millis, Ordering::SeqCst);
        }
        fn as_closure(&self) -> Box<dyn Fn() -> i64 + Send + Sync> {
            let counter = self.0.clone();
            Box::new(move || counter.load(Ordering::SeqCst))
        }
    }

    fn service(
        clock: &TestClock,
        verifier: Box<dyn StepUpVerifier>,
        ttl: Duration,
    ) -> StepUpService {
        StepUpService::new(
            Box::new(InMemoryStepUpStore::default()),
            verifier,
            Box::new(|| id_from_uuid(FlexUuid::nil())),
            clock.as_closure(),
            ttl,
        )
    }

    struct AcceptingVerifier;

    impl StepUpVerifier for AcceptingVerifier {
        fn verify(
            &self,
            _challenge: &StepUpChallenge,
            presented: &str,
        ) -> Result<(), StepUpDenial> {
            if presented == "idp-evidence" {
                Ok(())
            } else {
                Err(StepUpDenial::EvidenceRejected {
                    reason: "test: evidence does not match".to_string(),
                })
            }
        }
    }

    #[test]
    fn issue_then_verify_yields_bound_single_use_evidence() {
        let clock = TestClock::new();
        let svc = service(&clock, Box::new(AcceptingVerifier), DEFAULT_STEP_UP_TTL);
        let challenge = svc
            .issue("approver-1", tenant(), purpose())
            .expect("issue succeeds");
        let evidence = svc
            .verify(
                &challenge.challenge_id,
                "approver-1",
                tenant(),
                "idp-evidence",
                &purpose(),
            )
            .expect("verify succeeds");
        assert_eq!(evidence.subject, "approver-1");
        assert_eq!(evidence.tenant, tenant());
        assert_eq!(evidence.purpose, purpose());
        assert_eq!(evidence.expires_at_unix_millis, challenge.expires_at_unix_millis);
    }

    #[test]
    fn a_consumed_challenge_never_verifies_again_replay_is_a_recorded_denial() {
        let clock = TestClock::new();
        let svc = service(&clock, Box::new(AcceptingVerifier), DEFAULT_STEP_UP_TTL);
        let challenge = svc.issue("approver-1", tenant(), purpose()).unwrap();
        svc.verify(
            &challenge.challenge_id,
            "approver-1",
            tenant(),
            "idp-evidence",
            &purpose(),
        )
        .expect("first verify consumes the challenge");
        let replay = svc.verify(
            &challenge.challenge_id,
            "approver-1",
            tenant(),
            "idp-evidence",
            &purpose(),
        );
        // Single-use: the replay is a named denial — AlreadyUsed,
        // not NoChallenge: the store remembers the consumed id, so a
        // replay is distinguishable from a never-issued id (and NOTHING
        // about the earlier success is transferable).
        assert_eq!(replay, Err(StepUpDenial::AlreadyUsed));
    }

    #[test]
    fn an_expired_challenge_is_a_named_denial_not_a_wedge() {
        let clock = TestClock::new();
        let ttl = Duration::from_secs(60);
        let svc = service(&clock, Box::new(AcceptingVerifier), ttl);
        let challenge = svc.issue("approver-1", tenant(), purpose()).unwrap();
        clock.advance(61_000);
        let verdict = svc.verify(
            &challenge.challenge_id,
            "approver-1",
            tenant(),
            "idp-evidence",
            &purpose(),
        );
        assert_eq!(
            verdict,
            Err(StepUpDenial::Expired {
                expired_at_unix_millis: challenge.expires_at_unix_millis
            })
        );
    }

    #[test]
    fn a_missing_challenge_is_a_named_denial() {
        let clock = TestClock::new();
        let svc = service(&clock, Box::new(AcceptingVerifier), DEFAULT_STEP_UP_TTL);
        assert_eq!(
            svc.verify("no-such-challenge", "approver-1", tenant(), "idp-evidence", &purpose()),
            Err(StepUpDenial::NoChallenge)
        );
    }

    #[test]
    fn binding_enforcement_subject_tenant_purpose() {
        let clock = TestClock::new();
        let svc = service(&clock, Box::new(AcceptingVerifier), DEFAULT_STEP_UP_TTL);
        let challenge = svc.issue("approver-1", tenant(), purpose()).unwrap();
        // Wrong subject.
        assert_eq!(
            svc.verify(&challenge.challenge_id, "approver-2", tenant(), "idp-evidence", &purpose()),
            Err(StepUpDenial::SubjectMismatch)
        );
        // Wrong tenant.
        assert_eq!(
            svc.verify(
                &challenge.challenge_id,
                "approver-1",
                other_tenant(),
                "idp-evidence",
                &purpose()
            ),
            Err(StepUpDenial::TenantMismatch)
        );
        // Wrong purpose (an evidence token for command A can never approve
        // command B, and a command-approval challenge never serves a
        // console action).
        let other_command = StepUpPurpose::CommandApproval {
            command_id: "0192a5e0-0000-7e2f-9a1b-3c4d5e6f9b04".to_string(),
        };
        assert_eq!(
            svc.verify(
                &challenge.challenge_id,
                "approver-1",
                tenant(),
                "idp-evidence",
                &other_command
            ),
            Err(StepUpDenial::PurposeMismatch)
        );
        assert_eq!(
            svc.verify(
                &challenge.challenge_id,
                "approver-1",
                tenant(),
                "idp-evidence",
                &other_purpose()
            ),
            Err(StepUpDenial::PurposeMismatch)
        );
    }

    #[test]
    fn rejected_evidence_is_a_named_denial_and_the_challenge_is_consumed() {
        // The single-use store contract takes the challenge at verify
        // entry: a verifier rejection consumes it (a challenge whose
        // evidence was rejected must not be re-presentable — a fresh
        // challenge is the retry path, never a reused one).
        let clock = TestClock::new();
        let svc = service(&clock, Box::new(AcceptingVerifier), DEFAULT_STEP_UP_TTL);
        let challenge = svc.issue("approver-1", tenant(), purpose()).unwrap();
        assert_eq!(
            svc.verify(
                &challenge.challenge_id,
                "approver-1",
                tenant(),
                "wrong-evidence",
                &purpose()
            ),
            Err(StepUpDenial::EvidenceRejected {
                reason: "test: evidence does not match".to_string()
            })
        );
        // The rejected attempt consumed the challenge at verify entry,
        // so the retry is AlreadyUsed — the same denial as a replay after
        // a success (consumed is consumed, whatever consumed it).
        assert_eq!(
            svc.verify(
                &challenge.challenge_id,
                "approver-1",
                tenant(),
                "idp-evidence",
                &purpose()
            ),
            Err(StepUpDenial::AlreadyUsed)
        );
    }

    #[test]
    fn the_shipped_default_verifier_denies_everything_fail_closed() {
        let clock = TestClock::new();
        let svc = service(&clock, Box::new(DenyAllStepUpVerifier), DEFAULT_STEP_UP_TTL);
        let challenge = svc.issue("approver-1", tenant(), purpose()).unwrap();
        assert_eq!(
            svc.verify(
                &challenge.challenge_id,
                "approver-1",
                tenant(),
                "idp-evidence",
                &purpose()
            ),
            Err(StepUpDenial::VerifierNotComposed)
        );
    }

    #[test]
    fn purpose_recorded_text_is_stable_and_variant_distinct() {
        assert_eq!(
            purpose().recorded_text(),
            "command-approval:0192a5e0-0000-7e2f-9a1b-3c4d5e6f9b03"
        );
        assert_eq!(other_purpose().recorded_text(), "console-action:apikey.rotate");
        assert!(!purpose().same_binding(&other_purpose()));
        assert!(purpose().same_binding(&purpose()));
    }

    #[test]
    fn empty_subject_never_issues() {
        let clock = TestClock::new();
        let svc = service(&clock, Box::new(AcceptingVerifier), DEFAULT_STEP_UP_TTL);
        assert_eq!(
            svc.issue("   ", tenant(), purpose()),
            Err(StepUpDenial::SubjectMismatch)
        );
    }

    #[test]
    fn denials_render_readable_audit_text() {
        let denials = [
            StepUpDenial::NoChallenge,
            StepUpDenial::Expired {
                expired_at_unix_millis: 42,
            },
            StepUpDenial::AlreadyUsed,
            StepUpDenial::SubjectMismatch,
            StepUpDenial::PurposeMismatch,
            StepUpDenial::TenantMismatch,
            StepUpDenial::EvidenceRejected {
                reason: "r".to_string(),
            },
            StepUpDenial::VerifierNotComposed,
            StepUpDenial::StoreError {
                detail: "d".to_string(),
            },
        ];
        for denial in denials {
            let text = denial.to_string();
            assert!(!text.is_empty());
        }
    }
}
