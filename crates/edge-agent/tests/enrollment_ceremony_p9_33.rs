// P9.33 — Integration tests: the edge enrollment CEREMONY and identity
//         lifecycle (ledger P9.33; report §18.1, §17.3, §18.3).
//
// EVERY TEST BELOW IS UNRUN (the loop bans in
// `ledger/phase-09-pilot-qualification/ENTRY_NOTE.md` boundary 5 forbid
// building and running tests; the human build/test pass owns execution).
// Nothing here claims a pass — these are the ceremony cases the ledger
// names, WRITTEN so a human run produces the attempt → outcome → audit
// record per case. The ceremony EXECUTION itself (one non-production edge,
// end-to-end, ledger step 4) is the recorded UNRUN human gate; these tests
// exercise the ceremony STRUCTURE, never a real platform.
//
// The matrix (each test hand-computed against the closed state machine):
// - happy path: every named transition fires in order, each is audited with
//   its dedicated event name, and the append-only register records the full
//   lifecycle;
// - illegal transitions: every state-machine edge that does not exist is
//   the named `CEREMONY_TRANSITION_INVALID` refusal — nothing advances off
//   the recorded graph;
// - tokens: reuse of a SPENT token and use of an EXPIRED token (including
//   the exactly-at-expiry boundary) are both refused; a token above the
//   short-lived ceiling and a token with no named issuer are refused;
// - failed enrollment (ledger step 4): the WRONG-SITE credential refusal
//   and the wrong-identity refusal are named errors, audited;
// - challenge: wrong nonce, replayed ceremony message, and an expired
//   challenge (TTL boundary hand-computed) are refused;
// - no self-approval: the device approving itself, an approval without an
//   authority reference, and an approval before the challenge is answered
//   are all refused;
// - compromise lifecycle: revoke (append-only), re-enrollment authorized BY
//   the compromised identity is refused (the ledger's named risk), and
//   re-enrollment on a fresh platform-authority ticket + fresh token runs
//   the full second term; the register keeps BOTH terms after a restart;
// - durable audit: a ceremony refusal composed with the P6.18
//   `FileAuditBuffer` survives an edge restart on disk (the P9.31 → P9.32
//   join discipline).

use std::sync::Arc;

use flexcore_ports::clock::Clock;
use flexcore_ports::fakes::FixedClock;

use flexcore_edge_agent::audit_buffer::FileAuditBuffer;
use flexcore_edge_agent::enrollment::{
    DeviceBinding, DeviceCredentialRecord, PemBody, ProvisioningToken,
};
use flexcore_edge_agent::enrollment_ceremony::{
    canonical_ceremony_message_binding, CeremonyApproval, CeremonyState, EnrollmentCeremony,
    EnrollmentCeremonyError, EnrollmentTokenRecord, FileIdentityEventRegister, IdentityEventKind,
    ReEnrollmentAuthorization, TokenSingleUseLedger, DEFAULT_CHALLENGE_TTL_MILLIS,
    DEFAULT_MAX_TOKEN_LIFETIME_MILLIS,
};
use flexcore_edge_agent::ports::{AuditSink, InMemoryAuditSink};

const BASE_MILLIS: i64 = 1_700_000_000_000;

// --- Shared fixtures (the P6.06/P9.31 test patterns, mirrored) ---------------------------

fn scratch_dir(label: &str) -> std::path::PathBuf {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "flexcore-edge-p933-it-{}-{}-{}",
        std::process::id(),
        label,
        COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&dir).expect("scratch dir creates");
    dir
}

fn binding() -> DeviceBinding {
    DeviceBinding::new("tenant-a".to_string(), "pilot-site-A".to_string())
}

fn token_record(reference: &str, issued_at: i64, expires_at: i64) -> EnrollmentTokenRecord {
    EnrollmentTokenRecord {
        token_reference: reference.to_string(),
        issued_by: "platform-identity-authority".to_string(),
        issued_at_unix_millis: issued_at,
        expires_at_unix_millis: expires_at,
    }
}

fn short_lived_token(reference: &str) -> EnrollmentTokenRecord {
    // Hand-computed: issued at BASE, expires 10 minutes later — inside the
    // 15-minute (900_000 ms) ceiling.
    token_record(reference, BASE_MILLIS, BASE_MILLIS + 10 * 60_000)
}

fn token(reference: &str) -> ProvisioningToken {
    ProvisioningToken::new(format!("token-value-for-{reference}")).expect("valid token")
}

fn credential(device_id: &str, tenant: &str, site: &str) -> DeviceCredentialRecord {
    DeviceCredentialRecord {
        device_id: device_id.to_string(),
        tenant_id: tenant.to_string(),
        site_id: site.to_string(),
        // Shipped P6.06 seam: no CA, no fabricated certificate — honest.
        certificate_pem: None,
        chain_pem: Vec::new(),
        root_ca_pem: Some(PemBody::new(
            "-----BEGIN CERTIFICATE-----\nTEST\n-----END CERTIFICATE-----\n".to_string(),
        )),
        private_key_reference: Some(std::path::PathBuf::from(
            "/var/lib/flexcore-edge/keys/device.key",
        )),
        issued_at_unix_millis: BASE_MILLIS,
        expires_at_unix_millis: Some(BASE_MILLIS + 30 * 86_400_000),
    }
}

fn approval() -> CeremonyApproval {
    CeremonyApproval {
        approver: "J. Doe (platform security lead)".to_string(),
        authority_reference: "pilot-enrollment-approval-001".to_string(),
        approved_device_identity: "edge-01".to_string(),
        approved_tenant_id: "tenant-a".to_string(),
        approved_site_id: "pilot-site-A".to_string(),
        approved_at_unix_millis: BASE_MILLIS + 1_000,
    }
}

/// The ceremony harness: audit sink + injected (advanceable) clock, with
/// the audit lines filtered by dedicated event name.
struct CeremonyHarness {
    audit: Arc<InMemoryAuditSink>,
    clock: Arc<FixedClock>,
}

impl CeremonyHarness {
    fn new(now_millis: i64) -> Self {
        Self {
            audit: Arc::new(InMemoryAuditSink::new()),
            clock: Arc::new(FixedClock::at_unix_millis(now_millis)),
        }
    }

    fn ceremony(
        &self,
        device: &str,
        register: Option<Arc<FileIdentityEventRegister>>,
    ) -> EnrollmentCeremony {
        EnrollmentCeremony::new(
            device.to_string(),
            binding(),
            register,
            Arc::clone(&self.audit) as Arc<dyn AuditSink>,
            Arc::clone(&self.clock) as Arc<dyn Clock + Send + Sync>,
        )
        .expect("ceremony composes")
    }

    fn audited(&self, event_name: &str) -> Vec<String> {
        self.audit
            .events()
            .iter()
            .filter(|event| event.detail.starts_with(event_name))
            .map(|event| event.detail.clone())
            .collect()
    }
}

/// Drives the ceremony from UNENROLLED to APPROVED (request → challenge →
/// answer → approval), the state right before activation.
fn drive_to_approved(ceremony: &mut EnrollmentCeremony, single_use: &mut TokenSingleUseLedger) {
    ceremony
        .begin_request(&token("tok-001"), short_lived_token("tok-001"), single_use)
        .expect("request opens");
    let nonce = ceremony
        .issue_challenge("lab-entropy-seed-1")
        .expect("challenge issues");
    ceremony.answer_challenge(&nonce).expect("challenge answers");
    ceremony
        .approve(&approval())
        .expect("the platform approval records");
}

// --- 1. The happy path: every named transition fires and is audited ---------

#[test]
fn the_happy_path_names_every_transition_and_audits_it() {
    let harness = CeremonyHarness::new(BASE_MILLIS + 1_000);
    let dir = scratch_dir("happy-path");
    let register = Arc::new(
        FileIdentityEventRegister::open(&dir.join("identity.register")).expect("register opens"),
    );
    let mut ceremony = harness.ceremony("edge-01", Some(Arc::clone(&register)));
    let mut single_use = TokenSingleUseLedger::new();

    assert_eq!(ceremony.state(), CeremonyState::Unenrolled);
    ceremony
        .begin_request(&token("tok-001"), short_lived_token("tok-001"), &mut single_use)
        .expect("request opens");
    assert_eq!(ceremony.state(), CeremonyState::Requested);
    let nonce = ceremony
        .issue_challenge("lab-entropy-seed-1")
        .expect("challenge issues");
    assert_eq!(ceremony.state(), CeremonyState::Challenged);
    ceremony.answer_challenge(&nonce).expect("challenge answers");
    assert_eq!(ceremony.state(), CeremonyState::Challenged);
    ceremony.approve(&approval()).expect("approval records");
    assert_eq!(ceremony.state(), CeremonyState::Approved);
    ceremony
        .activate(&credential("edge-01", "tenant-a", "pilot-site-A"))
        .expect("activation completes");
    assert_eq!(ceremony.state(), CeremonyState::Active);

    // Every transition was audited under its dedicated event name.
    for name in [
        "EDGE_ENROLLMENT_CEREMONY_REQUESTED",
        "EDGE_ENROLLMENT_CEREMONY_CHALLENGED",
        "EDGE_ENROLLMENT_CEREMONY_CHALLENGE_ANSWERED",
        "EDGE_ENROLLMENT_CEREMONY_APPROVED",
        "EDGE_ENROLLMENT_CEREMONY_ACTIVATED",
    ] {
        assert_eq!(
            harness.audited(name).len(),
            1,
            "exactly one {name} audit line expected"
        );
    }

    // The append-only register holds the full lifecycle IN ORDER.
    let history = register.history();
    let kinds: Vec<&str> = history.iter().map(|e| e.kind.name()).collect();
    assert_eq!(
        kinds,
        vec![
            "CEREMONY_REQUESTED",
            "CEREMONY_APPROVED",
            "CEREMONY_ACTIVATED"
        ],
        "the register is the lifecycle history: {kinds:?}"
    );
    assert_eq!(register.corrupt_tail_truncated(), 0);
    let _ = std::fs::remove_dir_all(&dir);
}

// --- 2. Every illegal transition is refused with the named error -------------

#[test]
fn every_illegal_transition_is_refused_with_the_named_error() {
    let harness = CeremonyHarness::new(BASE_MILLIS + 1_000);
    let mut ceremony = harness.ceremony("edge-01", None);
    let mut single_use = TokenSingleUseLedger::new();

    // From UNENROLLED: challenge, answer, approve, activate are all
    // non-edges; revoke has nothing to revoke.
    for attempt in [
        ("challenge", CeremonyAttempt::Challenge),
        ("answer", CeremonyAttempt::Answer {
            nonce: "0000000000000000".to_string(),
        }),
        ("approve", CeremonyAttempt::Approve(Box::new(approval()))),
        ("activate", CeremonyAttempt::Activate),
        ("revoke", CeremonyAttempt::Revoke),
    ] {
        let (label, attempt) = attempt;
        let error = apply(&mut ceremony, attempt).expect_err(&format!("{label} must refuse"));
        assert!(
            matches!(error, EnrollmentCeremonyError::InvalidTransition { .. }),
            "{label} from UNENROLLED is an invalid transition: {error}"
        );
        assert_eq!(error.name(), "CEREMONY_TRANSITION_INVALID");
        assert_eq!(ceremony.state(), CeremonyState::Unenrolled, "nothing advanced");
    }

    // REQUESTED: approve/activate/answer are non-edges.
    ceremony
        .begin_request(&token("tok-001"), short_lived_token("tok-001"), &mut single_use)
        .expect("request opens");
    for attempt in [
        CeremonyAttempt::Answer { nonce: "x".to_string() },
        CeremonyAttempt::Approve(Box::new(approval())),
        CeremonyAttempt::Activate,
    ] {
        let error = apply(&mut ceremony, attempt).expect_err("must refuse");
        assert!(matches!(error, EnrollmentCeremonyError::InvalidTransition { .. }));
    }

    // A SECOND request from REQUESTED is a non-edge (the ceremony is open).
    let error = ceremony
        .begin_request(&token("tok-002"), short_lived_token("tok-002"), &mut single_use)
        .expect_err("a second request while REQUESTED is refused");
    assert!(matches!(error, EnrollmentCeremonyError::InvalidTransition { .. }));

    // APPROVED: another approval and a new request are non-edges.
    let nonce = ceremony.issue_challenge("seed").expect("issues");
    ceremony.answer_challenge(&nonce).expect("answers");
    ceremony.approve(&approval()).expect("approves");
    assert!(matches!(
        ceremony.approve(&approval()),
        Err(EnrollmentCeremonyError::InvalidTransition { .. })
    ));

    // ACTIVE: a duplicate activation is a non-edge.
    ceremony
        .activate(&credential("edge-01", "tenant-a", "pilot-site-A"))
        .expect("activates");
    let error = ceremony
        .activate(&credential("edge-01", "tenant-a", "pilot-site-A"))
        .expect_err("a duplicate activation is refused");
    assert!(matches!(error, EnrollmentCeremonyError::InvalidTransition { .. }));

    // REVOKED: a second revoke is the named AlreadyRevoked (not a state
    // overwrite — the register is append-only).
    ceremony.revoke("security-lead", "compromise suspected").expect("revokes");
    let error = ceremony
        .revoke("security-lead", "again")
        .expect_err("a second revoke is refused");
    assert_eq!(error.name(), "CEREMONY_ALREADY_REVOKED");

    // Every refusal above was audited under the dedicated refusal name.
    assert!(
        !harness.audited("EDGE_ENROLLMENT_CEREMONY_REFUSED").is_empty(),
        "the ceremony refusals are audited"
    );
}

/// The illegal-transition helper (an enum over the ceremony's mutating
/// calls, so the matrix above can loop).
enum CeremonyAttempt {
    BeginRequest,
    Challenge,
    Answer { nonce: String },
    Approve(Box<CeremonyApproval>),
    Activate,
    Revoke,
}

fn apply(
    ceremony: &mut EnrollmentCeremony,
    attempt: CeremonyAttempt,
) -> Result<(), EnrollmentCeremonyError> {
    let mut spent = TokenSingleUseLedger::new();
    match attempt {
        CeremonyAttempt::BeginRequest => ceremony
            .begin_request(&token("tok-x"), short_lived_token("tok-x"), &mut spent)
            .map(|_| ()),
        CeremonyAttempt::Challenge => ceremony.issue_challenge("seed").map(|_| ()),
        CeremonyAttempt::Answer { nonce } => ceremony.answer_challenge(&nonce),
        CeremonyAttempt::Approve(approval) => ceremony.approve(&approval),
        CeremonyAttempt::Activate => {
            ceremony
                .activate(&credential("edge-01", "tenant-a", "pilot-site-A"))
        }
        CeremonyAttempt::Revoke => ceremony.revoke("actor", "reason"),
    }
}

// --- 3. Spent and expired tokens are refused (ledger step 2) ------------------

#[test]
fn spent_token_reuse_and_expired_tokens_are_refused() {
    let harness = CeremonyHarness::new(BASE_MILLIS + 1_000);
    let mut single_use = TokenSingleUseLedger::new();

    // SPENT TOKEN REUSE (ledger step 2): ceremony 1 consumes tok-001; a
    // SECOND ceremony presenting the same reference is refused by the
    // edge-local single-use ledger with the FIRST-consumption instant.
    let mut first = harness.ceremony("edge-01", None);
    first
        .begin_request(&token("tok-001"), short_lived_token("tok-001"), &mut single_use)
        .expect("the first consumption succeeds");
    assert!(single_use.is_spent("tok-001"));

    let mut second = harness.ceremony("edge-02", None);
    let error = second
        .begin_request(&token("tok-001"), short_lived_token("tok-001"), &mut single_use)
        .expect_err("a spent token is refused");
    assert_eq!(error.name(), "CEREMONY_TOKEN_ALREADY_SPENT");
    assert!(
        matches!(error, EnrollmentCeremonyError::EnrollmentTokenAlreadySpent { spent_at_unix_millis }
            if spent_at_unix_millis == BASE_MILLIS + 1_000),
        "the refusal carries the FIRST consumption instant: {error}"
    );
    // The refusing ceremony is audited AND stayed UNENROLLED.
    assert_eq!(second.state(), CeremonyState::Unenrolled);
    assert_eq!(harness.audited("EDGE_ENROLLMENT_CEREMONY_REFUSED").len(), 1);

    // EXPIRED TOKEN (ledger step 2): the INJECTED clock at exactly the
    // expiry instant refuses (`expires_at <= now` — inclusive-expiry).
    let at_expiry = CeremonyHarness::new(BASE_MILLIS + 10 * 60_000);
    let mut ceremony = at_expiry.ceremony("edge-01", None);
    let error = ceremony
        .begin_request(&token("tok-003"), short_lived_token("tok-003"), &mut single_use)
        .expect_err("an exactly-at-expiry token is refused");
    assert!(matches!(
        error,
        EnrollmentCeremonyError::EnrollmentTokenExpired { .. }
    ));

    // One millisecond BEFORE expiry the same token is accepted.
    let before = CeremonyHarness::new(BASE_MILLIS + 10 * 60_000 - 1);
    let mut ceremony = before.ceremony("edge-01", None);
    ceremony
        .begin_request(&token("tok-003"), short_lived_token("tok-003"), &mut single_use)
        .expect("one millis before expiry the token is valid");

    // LIFETIME CEILING: a token living EXACTLY the 15-minute ceiling is
    // accepted; one millisecond more is refused (a long-lived enrollment
    // token becomes a standing credential — the ledger's named risk).
    let mut ceremony = harness.ceremony("edge-01", None);
    ceremony
        .begin_request(
            &token("tok-004"),
            token_record("tok-004", BASE_MILLIS, BASE_MILLIS + DEFAULT_MAX_TOKEN_LIFETIME_MILLIS),
            &mut single_use,
        )
        .expect("the ceiling-equal lifetime is accepted");
    let mut ceremony = harness.ceremony("edge-01", None);
    let error = ceremony
        .begin_request(
            &token("tok-005"),
            token_record(
                "tok-005",
                BASE_MILLIS,
                BASE_MILLIS + DEFAULT_MAX_TOKEN_LIFETIME_MILLIS + 1,
            ),
            &mut single_use,
        )
        .expect_err("an above-ceiling lifetime is refused");
    assert_eq!(error.name(), "CEREMONY_TOKEN_LIFETIME_EXCESSIVE");

    // UNNAMED ISSUER: a token with no issuing authority is refused.
    let mut unnamed = token_record("tok-006", BASE_MILLIS, BASE_MILLIS + 60_000);
    unnamed.issued_by = String::new();
    let mut ceremony = harness.ceremony("edge-01", None);
    let error = ceremony
        .begin_request(&token("tok-006"), unnamed, &mut single_use)
        .expect_err("an un-attributable token is refused");
    assert_eq!(error.name(), "CEREMONY_TOKEN_ISSUER_UNNAMED");
}

// --- 4. The failed-enrollment cases: wrong site, wrong identity (step 4) ------

#[test]
fn the_wrong_site_and_wrong_identity_failed_enrollment_cases_are_refused_and_audited() {
    let harness = CeremonyHarness::new(BASE_MILLIS + 1_000);
    let mut ceremony = harness.ceremony("edge-01", None);
    let mut single_use = TokenSingleUseLedger::new();
    drive_to_approved(&mut ceremony, &mut single_use);

    // WRONG SITE (ledger step 4's failed-enrollment case): the issued
    // credential binds pilot-site-B, the ceremony's configured binding is
    // pilot-site-A — refused, the ceremony stays APPROVED (nothing was
    // activated), and the refusal is audited.
    let error = ceremony
        .activate(&credential("edge-01", "tenant-a", "pilot-site-B"))
        .expect_err("the wrong-site credential is refused");
    assert_eq!(error.name(), "CEREMONY_BINDING_MISMATCH");
    assert!(matches!(error, EnrollmentCeremonyError::BindingMismatch { .. }));
    assert_eq!(ceremony.state(), CeremonyState::Approved);
    assert!(
        harness
            .audited("EDGE_ENROLLMENT_CEREMONY_REFUSED")
            .iter()
            .any(|detail| detail.contains("WRONG-SITE")),
        "the wrong-site refusal is audited with its reason"
    );

    // WRONG IDENTITY: the credential names a different device.
    let error = ceremony
        .activate(&credential("edge-99", "tenant-a", "pilot-site-A"))
        .expect_err("a credential for another device is refused");
    assert_eq!(error.name(), "CEREMONY_BINDING_MISMATCH");

    // The happy path still completes afterwards with the RIGHT credential.
    ceremony
        .activate(&credential("edge-01", "tenant-a", "pilot-site-A"))
        .expect("the matching credential activates");
    assert_eq!(ceremony.state(), CeremonyState::Active);
}

// --- 5. Challenge nonce, replay, and expiry (replay protection) ---------------

#[test]
fn wrong_nonce_replayed_messages_and_expired_challenges_are_refused() {
    let harness = CeremonyHarness::new(BASE_MILLIS + 1_000);
    let mut ceremony = harness.ceremony("edge-01", None);
    let mut single_use = TokenSingleUseLedger::new();
    ceremony
        .begin_request(&token("tok-001"), short_lived_token("tok-001"), &mut single_use)
        .expect("request opens");
    let nonce = ceremony.issue_challenge("lab-entropy-seed-1").expect("issues");
    assert_eq!(nonce.len(), 16, "the nonce is the 16-hex derivation");

    // WRONG NONCE: refused by name; the challenge remains outstanding.
    let error = ceremony
        .answer_challenge("0011223344556677")
        .expect_err("a wrong nonce is refused");
    assert_eq!(error.name(), "CEREMONY_CHALLENGE_NONCE_MISMATCH");
    assert_eq!(ceremony.state(), CeremonyState::Challenged);

    // THE CORRECT NONCE is accepted, and the ceremony-message binding for
    // it is the recorded known answer (hand-computed from the format).
    let expected_binding = canonical_ceremony_message_binding(
        "challenge-response",
        "edge-01",
        &nonce,
        BASE_MILLIS + 1_000,
    );
    assert!(
        expected_binding.starts_with("ceremony-msg:v1|kind:challenge-response|device:edge-01|"),
        "the message binding carries the versioned scheme: {expected_binding}"
    );
    ceremony.answer_challenge(&nonce).expect("answers");

    // REPLAY (the same ceremony message presented AGAIN): refused with the
    // first-seen instant — even though the nonce is CORRECT.
    let error = ceremony
        .answer_challenge(&nonce)
        .expect_err("a replayed ceremony message is refused");
    assert!(matches!(
        error,
        EnrollmentCeremonyError::CeremonyMessageReplayed { ref message_key, first_seen_at_unix_millis }
            if message_key == &expected_binding
                && first_seen_at_unix_millis == BASE_MILLIS + 1_000
    ), "the replay refusal names the message key and the first-seen instant: {error}");
    assert_eq!(error.name(), "CEREMONY_MESSAGE_REPLAYED");

    // EXPIRED CHALLENGE (hand-computed boundary): a fresh ceremony whose
    // clock advances by EXACTLY the TTL refuses the answer
    // (`expires_at <= now`); one millisecond inside the TTL answers fine.
    let harness = CeremonyHarness::new(BASE_MILLIS + 2_000);
    let mut ceremony = harness.ceremony("edge-01", None);
    let mut single_use = TokenSingleUseLedger::new();
    ceremony
        .begin_request(&token("tok-002"), short_lived_token("tok-002"), &mut single_use)
        .expect("request opens");
    let nonce = ceremony.issue_challenge("seed-2").expect("issues");
    harness.clock.advance_millis(DEFAULT_CHALLENGE_TTL_MILLIS);
    let error = ceremony
        .answer_challenge(&nonce)
        .expect_err("an exactly-at-TTL answer is refused");
    assert!(matches!(error, EnrollmentCeremonyError::ChallengeExpired { .. }));

    let harness = CeremonyHarness::new(BASE_MILLIS + 2_000);
    let mut ceremony = harness.ceremony("edge-01", None);
    let mut single_use = TokenSingleUseLedger::new();
    ceremony
        .begin_request(&token("tok-003"), short_lived_token("tok-003"), &mut single_use)
        .expect("request opens");
    let nonce = ceremony.issue_challenge("seed-3").expect("issues");
    harness.clock.advance_millis(DEFAULT_CHALLENGE_TTL_MILLIS - 1);
    ceremony.answer_challenge(&nonce).expect("one millis inside the TTL answers");
}

// --- 6. There is NO self-approval path -----------------------------------------

#[test]
fn there_is_no_self_approval_path() {
    let harness = CeremonyHarness::new(BASE_MILLIS + 1_000);
    let mut ceremony = harness.ceremony("edge-01", None);
    let mut single_use = TokenSingleUseLedger::new();
    ceremony
        .begin_request(&token("tok-001"), short_lived_token("tok-001"), &mut single_use)
        .expect("request opens");
    let nonce = ceremony.issue_challenge("seed").expect("issues");

    // Approval BEFORE the challenge is answered: refused.
    let error = ceremony
        .approve(&approval())
        .expect_err("an unanswered challenge cannot be approved");
    assert_eq!(error.name(), "CEREMONY_CHALLENGE_NOT_ANSWERED");
    ceremony.answer_challenge(&nonce).expect("answers");

    // SELF-APPROVAL: the approval actor IS the device — refused by name.
    let mut self_approval = approval();
    self_approval.approver = "edge-01".to_string();
    let error = ceremony
        .approve(&self_approval)
        .expect_err("a device cannot approve its own enrollment");
    assert_eq!(error.name(), "CEREMONY_SELF_APPROVAL_REFUSED");
    assert_eq!(ceremony.state(), CeremonyState::Challenged);

    // An approval with no authority reference is not an approval.
    let mut untraceable = approval();
    untraceable.authority_reference = String::new();
    let error = ceremony
        .approve(&untraceable)
        .expect_err("an approval without an authority reference is refused");
    assert_eq!(error.name(), "CEREMONY_APPROVAL_INVALID");

    // An approval naming another device / another scope is refused.
    let mut other_device = approval();
    other_device.approved_device_identity = "edge-99".to_string();
    let error = ceremony
        .approve(&other_device)
        .expect_err("an approval for another device is refused");
    assert_eq!(error.name(), "CEREMONY_APPROVAL_INVALID");
    let mut other_site = approval();
    other_site.approved_site_id = "pilot-site-B".to_string();
    let error = ceremony
        .approve(&other_site)
        .expect_err("an approval for another scope is refused");
    assert_eq!(error.name(), "CEREMONY_APPROVAL_INVALID");

    // The DISTINCT platform approval still records — the gate exists, it
    // just cannot be self-served.
    ceremony.approve(&approval()).expect("the platform approval records");
    assert_eq!(ceremony.state(), CeremonyState::Approved);
}

// --- 7. Compromise: revoke, and re-enrollment WITHOUT the old identity ----------

#[test]
fn compromise_revocation_and_re_enrollment_never_trusts_the_old_identity() {
    let harness = CeremonyHarness::new(BASE_MILLIS + 1_000);
    let dir = scratch_dir("compromise");
    let register = Arc::new(
        FileIdentityEventRegister::open(&dir.join("identity.register")).expect("register opens"),
    );
    let mut ceremony = harness.ceremony("edge-01", Some(Arc::clone(&register)));
    let mut single_use = TokenSingleUseLedger::new();
    drive_to_approved(&mut ceremony, &mut single_use);
    ceremony
        .activate(&credential("edge-01", "tenant-a", "pilot-site-A"))
        .expect("term 1 activates");

    // REVOKE (compromise): actor + reason mandatory; the revocation is an
    // APPEND — the register keeps the active term AND the revoked term.
    ceremony
        .revoke("security-lead", "device compromise suspected (P9.09 cross-reference)")
        .expect("revokes");
    assert_eq!(ceremony.state(), CeremonyState::Revoked);

    // THE COMPROMISE RULE: re-enrollment authorized BY the revoked identity
    // itself is refused — the ledger's named risk, structurally.
    let error = ceremony
        .begin_re_enrollment(
            &ReEnrollmentAuthorization::PriorIdentity {
                reference: "credential:edge-01@term-1".to_string(),
            },
            "successor for the compromised edge-01",
        )
        .expect_err("the compromised identity cannot vouch for its successor");
    assert_eq!(error.name(), "CEREMONY_COMPROMISED_IDENTITY_VOUCHING_REFUSED");
    assert_eq!(ceremony.state(), CeremonyState::Revoked, "nothing reset");

    // THE LEGAL PATH: a FRESH platform-authority ticket. The fresh TOKEN
    // for the new term goes through the NORMAL `begin_request` discipline
    // (single-use, short-lived, issuer-named) — there is no second token
    // path to maintain.
    ceremony
        .begin_re_enrollment(
            &ReEnrollmentAuthorization::PlatformAuthorityTicket {
                ticket_reference: "pilot-re-enroll-ticket-001".to_string(),
            },
            "re-enrollment after compromise; the compromised term never authorized it",
        )
        .expect("re-enrollment opens");
    assert_eq!(ceremony.state(), CeremonyState::Unenrolled);

    // The fresh token is REQUIRED: a re-presented OLD reference is refused
    // by the single-use ledger (the old term buys nothing).
    let error = ceremony
        .begin_request(&token("tok-001"), short_lived_token("tok-001"), &mut single_use)
        .expect_err("the first term's token is spent");
    assert_eq!(error.name(), "CEREMONY_TOKEN_ALREADY_SPENT");

    // Term 2 runs the FULL ceremony to Active on a FRESH token.
    ceremony
        .begin_request(&token("tok-fresh-2"), short_lived_token("tok-fresh-2"), &mut single_use)
        .expect("term 2 opens");
    let nonce = ceremony.issue_challenge("seed-term-2").expect("issues");
    ceremony.answer_challenge(&nonce).expect("answers");
    ceremony.approve(&approval()).expect("approves");
    ceremony
        .activate(&credential("edge-01", "tenant-a", "pilot-site-A"))
        .expect("term 2 activates");
    assert_eq!(ceremony.state(), CeremonyState::Active);

    // THE REGISTER IS THE HISTORY: both terms, in order, append-only.
    let kinds: Vec<&str> = register.history().iter().map(|e| e.kind.name()).collect();
    assert_eq!(
        kinds,
        vec![
            "CEREMONY_REQUESTED",       // term 1
            "CEREMONY_APPROVED",
            "CEREMONY_ACTIVATED",
            "CEREMONY_REVOKED",         // the compromise
            "CEREMONY_RE_ENROLLMENT_OPENED",
            "CEREMONY_REQUESTED",       // term 2
            "CEREMONY_APPROVED",
            "CEREMONY_ACTIVATED",
        ],
        "the append-only register keeps BOTH terms: {kinds:?}"
    );
    // The re-enrollment event carries the authority ticket reference (the
    // old identity authorized nothing).
    let re_enroll = register
        .history()
        .iter()
        .find(|e| e.kind == IdentityEventKind::ReEnrollmentOpened)
        .expect("the re-enrollment event exists");
    assert_eq!(re_enroll.reference, "pilot-re-enroll-ticket-001");
    assert!(
        harness.audited("EDGE_RE_ENROLLMENT_OPENED").len() == 1,
        "the re-enrollment is audited once"
    );

    // The register survives a restart with the full history.
    drop(ceremony);
    let reopened = FileIdentityEventRegister::open(&dir.join("identity.register"))
        .expect("reopens");
    assert_eq!(reopened.len(), 8, "the full history survives the restart");
    assert_eq!(reopened.corrupt_tail_truncated(), 0);
    let _ = std::fs::remove_dir_all(&dir);
}

// --- 8. The identity register: append-only framing, restart, foreign file -------

#[test]
fn the_identity_register_round_trips_refuses_foreign_files_and_truncates_corrupt_tails() {
    let dir = scratch_dir("register");
    let path = dir.join("identity.register");
    {
        let register = FileIdentityEventRegister::open(&path).expect("opens");
        register
            .append(IdentityEventKind::Requested, BASE_MILLIS + 1, "platform-identity-authority", "", "tok-1")
            .expect("appends");
        register
            .append(IdentityEventKind::Revoked, BASE_MILLIS + 2, "security-lead", "compromise", "")
            .expect("appends");
    }
    let reopened = FileIdentityEventRegister::open(&path).expect("reopens");
    assert_eq!(reopened.len(), 2);
    let history = reopened.history();
    assert_eq!(history[0].kind, IdentityEventKind::Requested);
    assert_eq!(history[1].kind, IdentityEventKind::Revoked);
    assert_eq!(history[1].reason, "compromise");

    // A FOREIGN FILE is refused, never guessed at.
    let foreign = dir.join("not-a-register");
    std::fs::write(&foreign, b"something else entirely").expect("writes");
    assert!(matches!(
        FileIdentityEventRegister::open(&foreign),
        Err(EnrollmentCeremonyError::NotAnIdentityRegister { .. })
    ));

    // A CORRUPT TAIL is truncated with a counted loss, never silent.
    let corrupted = dir.join("corrupted.register");
    {
        let register = FileIdentityEventRegister::open(&corrupted).expect("opens");
        register
            .append(IdentityEventKind::Activated, BASE_MILLIS + 3, "ceremony", "", "edge-01")
            .expect("appends");
    }
    // Append garbage after the last good record (a torn frame).
    let mut raw = std::fs::read(&corrupted).expect("reads");
    raw.extend_from_slice(&[0xFF, 0x00, 0x00, 0x00, 0x99]);
    std::fs::write(&corrupted, &raw).expect("corrupts the tail");
    let recovered = FileIdentityEventRegister::open(&corrupted).expect("recovers");
    assert_eq!(recovered.len(), 1, "only the good record survives");
    assert_eq!(recovered.corrupt_tail_truncated(), 1, "the loss is counted");
    let _ = std::fs::remove_dir_all(&dir);
}

// --- 9. The durable audit trail (the P9.33 → P9.32 join) --------------------------

#[test]
fn ceremony_refusals_are_durably_audited_by_the_edge_audit_buffer() {
    // The ceremony's audit trail is not an in-memory convenience: composed
    // with the P6.18 FileAuditBuffer, a refusal survives an edge restart on
    // disk — the evidence the acceptance decision reads (P9.32's surface).
    let dir = scratch_dir("durable-audit");
    let journal = dir.join("audit.journal");
    let audit = Arc::new(
        FileAuditBuffer::open(
            &journal,
            100,
            Arc::new(FixedClock::at_unix_millis(BASE_MILLIS + 1_000)),
        )
        .expect("audit journal opens"),
    );
    let clock: Arc<dyn Clock + Send + Sync> = Arc::new(FixedClock::at_unix_millis(BASE_MILLIS + 1_000));
    let mut ceremony = EnrollmentCeremony::new(
        "edge-01".to_string(),
        binding(),
        None,
        Arc::clone(&audit) as Arc<dyn AuditSink>,
        Arc::clone(&clock),
    )
    .expect("ceremony composes");
    let mut single_use = TokenSingleUseLedger::new();
    // One spent-token refusal: audited through the DURABLE buffer.
    ceremony
        .begin_request(&token("tok-001"), short_lived_token("tok-001"), &mut single_use)
        .expect("the first consumption succeeds");
    let _ = ceremony
        .begin_request(&token("tok-001"), short_lived_token("tok-001"), &mut single_use);
    drop(ceremony);
    drop(audit);

    // The edge RESTARTS: the refusal evidence is still on disk.
    let reopened = FileAuditBuffer::open(
        &journal,
        100,
        Arc::new(FixedClock::at_unix_millis(BASE_MILLIS + 9_000)),
    )
    .expect("reopens");
    let pending = reopened.drain(100);
    assert!(
        pending
            .iter()
            .any(|record| record.detail.starts_with("EDGE_ENROLLMENT_CEREMONY_REFUSED")
                && record.detail.contains("SPENT-TOKEN REUSE")),
        "the spent-token refusal survived the restart in the durable audit buffer: {:?}",
        pending.iter().map(|r| r.detail.clone()).collect::<Vec<_>>()
    );
    let _ = std::fs::remove_dir_all(&dir);
}
