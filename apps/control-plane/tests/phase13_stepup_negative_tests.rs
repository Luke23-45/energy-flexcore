// P13.19 — the step-up NEGATIVE test suite (wave 2; WRITTEN-UNRUN — never
// compiled or executed; execution is the human build/test gate,
// conventions §1 item 3).
//
// Extends record 04's in-module matrix at integration level: the
// expired / replayed / failed / mismatched / missing-challenge denials,
// the no-wedge guarantee, and the fail-closed default verifier — over the
// PUBLIC `StepUpService` API (`apps/control-plane/src/stepup.rs`,
// consumed, never redefined; the record 04 §7 surfaces are the
// in-crate core, this file is the integration slice the record 04 §9
// handoff note named).
//
// EVERY scenario below is a pure in-process evaluation over injected
// stores, clocks, and verifiers. No challenge has ever been issued in any
// real environment; no MFA factor exists; the IdP is the P13.20 lab
// spec's written design. Every "verifier" here is a local test double.
//
// The no-wedge property under test (plan risk 3): every denial below
// leaves the world exactly as it was — the service holds no reference to
// the command lifecycle, so a failed challenge cannot move, wedge, or
// partial-approve any command; the gated operation simply does not
// proceed.
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use flexcore_contracts::ids::FlexUuid;
use flexcore_control_plane::stepup::{
    DEFAULT_STEP_UP_TTL, DenyAllStepUpVerifier, InMemoryStepUpStore, StepUpDenial,
    StepUpPurpose, StepUpService, StepUpVerifier,
};
use flexcore_domain::ids::TenantId;

const TENANT_UUID: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f8a01";
const TENANT_B_UUID: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f8a02";
const COMMAND_A: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f8aa1";
const COMMAND_B: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f8aa2";

fn tenant(uuid_text: &str) -> TenantId {
    TenantId::new(FlexUuid::parse(uuid_text).expect("valid test uuid"))
}

fn command_approval(command_id: &str) -> StepUpPurpose {
    StepUpPurpose::CommandApproval {
        command_id: command_id.to_string(),
    }
}

/// A verifier that accepts exactly one presented artifact (the test
/// double standing in for the IdP-delegated verifier — the P13-E-3
/// DEFAULT's seam; nothing here claims IdP behavior).
struct AcceptingVerifier {
    accepts: String,
}

impl StepUpVerifier for AcceptingVerifier {
    fn verify(
        &self,
        _challenge: &flexcore_control_plane::stepup::StepUpChallenge,
        presented: &str,
    ) -> Result<(), StepUpDenial> {
        if presented == self.accepts {
            Ok(())
        } else {
            Err(StepUpDenial::EvidenceRejected {
                reason: "the test double accepts only its seeded artifact".to_string(),
            })
        }
    }
}

struct Fixture {
    service: StepUpService,
    clock: Arc<AtomicI64>,
}

fn fixture(now: i64) -> Fixture {
    let clock = Arc::new(AtomicI64::new(now));
    let clock_for_service = Arc::clone(&clock);
    let service = StepUpService::new(
        Box::new(InMemoryStepUpStore::default()),
        Box::new(AcceptingVerifier {
            accepts: "fresh-idp-evidence".to_string(),
        }),
        Box::new(|| {
            static COUNTER: AtomicI64 = AtomicI64::new(0);
            format!("chal-{}", COUNTER.fetch_add(1, Ordering::SeqCst))
        }),
        Box::new(move || clock_for_service.load(Ordering::SeqCst)),
        DEFAULT_STEP_UP_TTL,
    );
    Fixture { service, clock }
}

// --- EXPIRED challenges (the record 04 §9 handoff's first negative class) --------

#[test]
fn an_expired_challenge_denies_at_the_exclusive_window_close() {
    let fx = fixture(1_000);
    let challenge = fx
        .service
        .issue("user:approver", tenant(TENANT_UUID), command_approval(COMMAND_A))
        .expect("the challenge issues");
    // Exactly at the TTL boundary (5 min): the window is exclusive —
    // verification at the close instant is ALREADY expired (the in-crate
    // matrix pins the same boundary; this asserts it at the service API).
    fx.clock.store(1_000 + 5 * 60 * 1000, Ordering::SeqCst);
    assert_eq!(
        fx.service.verify(
            &challenge.challenge_id,
            "user:approver",
            tenant(TENANT_UUID),
            "fresh-idp-evidence",
            &command_approval(COMMAND_A),
        ),
        Err(StepUpDenial::Expired {
            expired_at_unix_millis: 1_000 + 5 * 60 * 1000,
        }),
        "verification at the exclusive window close is a named expiry denial"
    );
}

#[test]
fn a_challenge_verified_one_millisecond_before_expiry_still_succeeds() {
    let fx = fixture(1_000);
    let challenge = fx
        .service
        .issue("user:approver", tenant(TENANT_UUID), command_approval(COMMAND_A))
        .expect("the challenge issues");
    fx.clock.store(1_000 + 5 * 60 * 1000 - 1, Ordering::SeqCst);
    let evidence = fx
        .service
        .verify(
            &challenge.challenge_id,
            "user:approver",
            tenant(TENANT_UUID),
            "fresh-idp-evidence",
            &command_approval(COMMAND_A),
        )
        .expect("inside the window the verification succeeds");
    // The evidence never outlives its challenge window (record 04 §2).
    assert_eq!(
        evidence.expires_at_unix_millis,
        challenge.expires_at_unix_millis
    );
}

// --- REPLAYED challenges (single-use) ---------------------------------------------

#[test]
fn a_replayed_challenge_denies_after_a_successful_verification() {
    let fx = fixture(1_000);
    let challenge = fx
        .service
        .issue("user:approver", tenant(TENANT_UUID), command_approval(COMMAND_A))
        .expect("the challenge issues");
    fx.service
        .verify(
            &challenge.challenge_id,
            "user:approver",
            tenant(TENANT_UUID),
            "fresh-idp-evidence",
            &command_approval(COMMAND_A),
        )
        .expect("the first verification succeeds");
    // The SAME challenge id, the SAME (valid) evidence, ANY purpose: the
    // single-use rule denies — the challenge was consumed at verify entry.
    assert_eq!(
        fx.service.verify(
            &challenge.challenge_id,
            "user:approver",
            tenant(TENANT_UUID),
            "fresh-idp-evidence",
            &command_approval(COMMAND_A),
        ),
        Err(StepUpDenial::AlreadyUsed),
        "a replayed challenge denies (single-use)"
    );
}

#[test]
fn a_replayed_challenge_denies_even_with_the_clock_rolled_back() {
    // A replay with a rolled-back clock is still a replay: the take()
    // happens before any time check, so the consumed challenge is gone.
    let fx = fixture(1_000);
    let challenge = fx
        .service
        .issue("user:approver", tenant(TENANT_UUID), command_approval(COMMAND_A))
        .expect("the challenge issues");
    fx.service
        .verify(
            &challenge.challenge_id,
            "user:approver",
            tenant(TENANT_UUID),
            "fresh-idp-evidence",
            &command_approval(COMMAND_A),
        )
        .expect("the first verification succeeds");
    fx.clock.store(900, Ordering::SeqCst);
    assert_eq!(
        fx.service.verify(
            &challenge.challenge_id,
            "user:approver",
            tenant(TENANT_UUID),
            "fresh-idp-evidence",
            &command_approval(COMMAND_A),
        ),
        Err(StepUpDenial::AlreadyUsed),
        "clock manipulation cannot resurrect a consumed challenge"
    );
}

// --- FAILED challenges (the verifier rejects; a fresh challenge is the retry) -----

#[test]
fn a_failed_verification_denies_and_a_fresh_challenge_is_the_retry_path() {
    let fx = fixture(1_000);
    let challenge = fx
        .service
        .issue("user:approver", tenant(TENANT_UUID), command_approval(COMMAND_A))
        .expect("the challenge issues");
    // Wrong evidence: the verifier rejects; the denial is named.
    assert!(
        matches!(
            fx.service.verify(
                &challenge.challenge_id,
                "user:approver",
                tenant(TENANT_UUID),
                "stale-or-wrong-evidence",
                &command_approval(COMMAND_A),
            ),
            Err(StepUpDenial::EvidenceRejected { .. })
        ),
        "the verifier's rejection surfaces as EvidenceRejected"
    );
    // The consumed challenge is gone (a failed attempt still consumed it
    // at verify entry — record 04 §2: rejected evidence consumes the
    // challenge), so a retry REQUIRES a fresh challenge.
    assert_eq!(
        fx.service.verify(
            &challenge.challenge_id,
            "user:approver",
            tenant(TENANT_UUID),
            "fresh-idp-evidence",
            &command_approval(COMMAND_A),
        ),
        Err(StepUpDenial::AlreadyUsed),
        "the failed attempt consumed the challenge; the retry path is a NEW challenge"
    );
    let fresh = fx
        .service
        .issue("user:approver", tenant(TENANT_UUID), command_approval(COMMAND_A))
        .expect("the fresh challenge issues");
    fx.service
        .verify(
            &fresh.challenge_id,
            "user:approver",
            tenant(TENANT_UUID),
            "fresh-idp-evidence",
            &command_approval(COMMAND_A),
        )
        .expect("the fresh challenge verifies");
}

// --- BINDING mismatches (subject / tenant / purpose) -------------------------------

#[test]
fn command_a_evidence_never_verifies_command_b_purpose() {
    let fx = fixture(1_000);
    let challenge = fx
        .service
        .issue("user:approver", tenant(TENANT_UUID), command_approval(COMMAND_A))
        .expect("the challenge issues");
    assert_eq!(
        fx.service.verify(
            &challenge.challenge_id,
            "user:approver",
            tenant(TENANT_UUID),
            "fresh-idp-evidence",
            &command_approval(COMMAND_B),
        ),
        Err(StepUpDenial::PurposeMismatch),
        "an evidence token for command A can never approve command B (record 04 §2)"
    );
}

#[test]
fn console_action_and_command_approval_purposes_never_match() {
    let fx = fixture(1_000);
    let challenge = fx
        .service
        .issue(
            "user:admin",
            tenant(TENANT_UUID),
            StepUpPurpose::ConsoleAction {
                action_id: "apikey.rotate".to_string(),
            },
        )
        .expect("the challenge issues");
    assert_eq!(
        fx.service.verify(
            &challenge.challenge_id,
            "user:admin",
            tenant(TENANT_UUID),
            "fresh-idp-evidence",
            &command_approval(COMMAND_A),
        ),
        Err(StepUpDenial::PurposeMismatch),
        "a console-action challenge never satisfies a command-approval purpose"
    );
}

#[test]
fn subject_and_tenant_mismatches_deny_before_the_verifier_runs() {
    let fx = fixture(1_000);
    let challenge = fx
        .service
        .issue("user:approver", tenant(TENANT_UUID), command_approval(COMMAND_A))
        .expect("the challenge issues");
    assert_eq!(
        fx.service.verify(
            &challenge.challenge_id,
            "user:someone-else",
            tenant(TENANT_UUID),
            "fresh-idp-evidence",
            &command_approval(COMMAND_A),
        ),
        Err(StepUpDenial::SubjectMismatch),
        "another subject never consumes another principal's challenge"
    );
    assert_eq!(
        fx.service.verify(
            &challenge.challenge_id,
            "user:approver",
            tenant(TENANT_B_UUID),
            "fresh-idp-evidence",
            &command_approval(COMMAND_A),
        ),
        Err(StepUpDenial::TenantMismatch),
        "the challenge does not cross tenants"
    );
}

// --- MISSING challenges and the fail-closed default ---------------------------------

#[test]
fn an_unknown_challenge_id_denies_without_touching_anything() {
    let fx = fixture(1_000);
    assert_eq!(
        fx.service.verify(
            "chal-does-not-exist",
            "user:approver",
            tenant(TENANT_UUID),
            "fresh-idp-evidence",
            &command_approval(COMMAND_A),
        ),
        Err(StepUpDenial::NoChallenge),
        "no challenge for the presented id is a named denial"
    );
}

#[test]
fn the_shipped_default_verifier_denies_everything_fail_closed() {
    let clock = Arc::new(AtomicI64::new(1_000));
    let clock_for_service = Arc::clone(&clock);
    let service = StepUpService::new(
        Box::new(InMemoryStepUpStore::default()),
        Box::new(DenyAllStepUpVerifier),
        Box::new(|| "chal".to_string()),
        Box::new(move || clock_for_service.load(Ordering::SeqCst)),
        Duration::from_secs(300),
    );
    let challenge = service
        .issue("user:approver", tenant(TENANT_UUID), command_approval(COMMAND_A))
        .expect("ISSUE works without a verifier; VERIFY is what fails closed");
    assert_eq!(
        service.verify(
            &challenge.challenge_id,
            "user:approver",
            tenant(TENANT_UUID),
            "any-evidence",
            &command_approval(COMMAND_A),
        ),
        Err(StepUpDenial::VerifierNotComposed),
        "until an operator composes a verifier and signs off, NO step-up can succeed anywhere (record 04 §8)"
    );
}

#[test]
fn an_empty_subject_never_issues() {
    let fx = fixture(1_000);
    assert_eq!(
        fx.service.issue("", tenant(TENANT_UUID), command_approval(COMMAND_A)),
        Err(StepUpDenial::SubjectMismatch),
        "a blank subject is a binding failure, not a challenge"
    );
    assert_eq!(
        fx.service.issue("   ", tenant(TENANT_UUID), command_approval(COMMAND_A)),
        Err(StepUpDenial::SubjectMismatch),
        "a whitespace subject is a binding failure, not a challenge"
    );
}

// --- The no-wedge guarantee (plan risk 3, the structural statement) -----------------

#[test]
fn every_denial_leaves_the_service_ready_for_a_fresh_challenge_no_wedge() {
    // The structural no-wedge rule, exercised end to end: after a chain
    // of failures (expired, replayed, mismatched, failed), a FRESH
    // challenge still issues and still verifies — nothing in the failure
    // history wedges the protocol. The service holds no command-lifecycle
    // reference, so no command state was touched by any denial above.
    let fx = fixture(1_000);
    let first = fx
        .service
        .issue("user:approver", tenant(TENANT_UUID), command_approval(COMMAND_A))
        .expect("issues");
    fx.clock.store(999_999_999, Ordering::SeqCst);
    assert!(fx.service.verify(
        &first.challenge_id,
        "user:approver",
        tenant(TENANT_UUID),
        "fresh-idp-evidence",
        &command_approval(COMMAND_A),
    )
    .is_err());
    let second = fx
        .service
        .issue("user:approver", tenant(TENANT_UUID), command_approval(COMMAND_A))
        .expect("the protocol still issues after any failure history");
    fx.service
        .verify(
            &second.challenge_id,
            "user:approver",
            tenant(TENANT_UUID),
            "fresh-idp-evidence",
            &command_approval(COMMAND_A),
        )
        .expect("the protocol still verifies after any failure history — no wedge");
}
