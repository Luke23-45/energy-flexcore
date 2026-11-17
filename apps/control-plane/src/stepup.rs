// P13.04 — The server-side step-up challenge protocol: the ENFORCED form
//         that replaces the UI-only P5.26 flow (`console/src/components/
//         stepUpApproval.tsx` — which records, honestly, that NO
//         server-verified step-up exists; this module is the mechanism
//         LR-048 names as the gap). Decision record: phase-13
//         `records/04_p13_04_step_up_mfa.md`.
//
// WRITTEN-UNRUN: no challenge has ever been issued or verified, no IdP is
// deployed (P13-E-3 stays OPEN — the recorded DEFAULT, IdP-delegated MFA
// primary with TOTP fallback, is implemented as the designed seam only),
// and no console flow consumes this yet (P13.17, wave 2). Nothing here
// claims runtime behavior.
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

use std::collections::HashMap;
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
/// design record names the table shape).
pub trait StepUpStore: Send + Sync {
    fn insert(&self, challenge: StepUpChallenge) -> Result<(), StepUpDenial>;
    /// Takes the challenge out (single-use by construction) — `None` when
    /// no challenge matches the id.
    fn take(&self, challenge_id: &str) -> Result<Option<StepUpChallenge>, StepUpDenial>;
}

/// The in-memory store: id → challenge. Bounded by the TTL sweep at
/// verification time (expired entries are dropped when encountered); a
/// production deployment swaps the durable implementation in through the
/// same port.
#[derive(Default)]
pub struct InMemoryStepUpStore {
    entries: Mutex<HashMap<String, StepUpChallenge>>,
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

    fn take(&self, challenge_id: &str) -> Result<Option<StepUpChallenge>, StepUpDenial> {
        let mut entries = self
            .entries
            .lock()
            .map_err(|_| StepUpDenial::StoreError {
                detail: "the in-memory challenge store lock is poisoned".to_string(),
            })?;
        Ok(entries.remove(challenge_id))
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
    /// world is exactly as it was before the call (a replayed-consumed
    /// challenge denies on every later attempt).
    pub fn verify(
        &self,
        challenge_id: &str,
        presenting_subject: &str,
        acting_tenant: TenantId,
        presented: &str,
        requested_purpose: &StepUpPurpose,
    ) -> Result<StepUpEvidence, StepUpDenial> {
        let now = (self.now_millis)();
        let challenge = self
            .store
            .take(challenge_id)?
            .ok_or(StepUpDenial::NoChallenge)?;
        // Binding checks BEFORE the verifier runs: a mismatched subject,
        // tenant, or purpose is a named denial without spending the
        // verifier's judgment (and without consuming the verifier's
        // judgment as oracle material).
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
            return Err(StepUpDenial::Expired {
                expired_at_unix_millis: challenge.expires_at_unix_millis,
            });
        }
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
        // Single-use: the replay is a named denial (AlreadyUsed or
        // NoChallenge — the store took the entry, so NoChallenge here),
        // and NOTHING about the earlier success is transferable.
        assert_eq!(replay, Err(StepUpDenial::NoChallenge));
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
        assert_eq!(
            svc.verify(
                &challenge.challenge_id,
                "approver-1",
                tenant(),
                "idp-evidence",
                &purpose()
            ),
            Err(StepUpDenial::NoChallenge)
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
