// P13.19 — the webhook signature / tolerance / replay / idempotent-handler
// test suite (wave 2; WRITTEN-UNRUN — never compiled or executed;
// execution is the human build/test gate, conventions §1 item 3).
//
// Extends record 21's surfaces (the P13.09 signer and delivery service)
// at integration level — the record 21 §6 handoff named these as the
// wave-2 negative slices. Consumed, never redefined:
// - `apps/control-plane/src/webhook_signing.rs` — the HMAC-SHA-256 over
//   `timestamp.rawbody`, the 5-minute tolerance window
//   (`TOLERANCE_SECONDS = 300`), the strict single-element header parse,
//   the rolling secret set (current signs, both verify during overlap);
// - `apps/control-plane/src/webhook_service.rs` — the book-before-attempt
//   bookkeeping under the delivery idempotency key (duplicate-collapse,
//   same-key-different-content conflict), the bounded backoff, the
//   dead-letter promotion, and the idempotent-handler contract
//   (plan §2.3: receivers may observe duplicates; handlers must be
//   idempotent — this suite asserts the SENDER-side bookkeeping shape
//   that makes duplicates observable-and-collapsible).
//
// NO webhook has ever been signed, delivered, retried, or dead-lettered
// in any environment: every test runs against the in-memory store fake,
// an injected clock, and a local HTTP-port test double. The RFC 4231 /
// FIPS vector tests (the hand-written crypto's correctness gate) live in
// the signing module's own tests and are NOT duplicated here.
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};

use flexcore_contracts::ids::FlexUuid;
use flexcore_control_plane::webhook_service::{
    attempt_allowed, backoff_delay_secs, HttpPostOutcome, MemoryWebhookDeliveryStore,
    OutboundHttpPost, WebhookDeliveryError, WebhookDeliveryRecord, WebhookDispatcher,
    WebhookEndpointRecord, WebhookSecretResolver, MAX_ATTEMPTS,
};
use flexcore_control_plane::webhook_signing::{WebhookSecretSet, WebhookSigningError, WebhookSigningKey};
use flexcore_domain::ids::TenantId;

const TENANT_UUID: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f8a01";
const OTHER_TENANT_UUID: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f8a02";

fn tenant(uuid_text: &str) -> TenantId {
    TenantId::new(FlexUuid::parse(uuid_text).expect("valid test uuid"))
}

/// A deterministic 32-byte test key (NOT a generated secret — no secret
/// material is created by this suite; these bytes are fixed test vectors).
fn test_key(seed: u8) -> WebhookSigningKey {
    WebhookSigningKey::from_bytes(&[seed; 32]).expect("32 bytes clear the floor")
}

// --- Signature scheme (sign/verify round-trip over the RAW body) -------------------

#[test]
fn sign_then_verify_round_trips_over_the_raw_body() {
    let set = WebhookSecretSet::single(test_key(1));
    let raw_body = br#"{"event":"TelemetryQualityChanged","asset_id":"a-1"}"#;
    let header = set.sign(1_700_000_000, raw_body);
    assert!(header.starts_with("t=1700000000,v1="), "the header shape is t=<ts>,v1=<hex>");
    set.verify(&header, 1_700_000_000, raw_body)
        .expect("a signature verifies over the SAME raw bytes at the SAME timestamp");
}

#[test]
fn a_signature_over_a_reserialized_body_never_verifies() {
    // The RAW-body rule (plan §2.3): the signer and the verifier must see
    // the exact transmitted bytes; a re-serialized (key-reordered)
    // payload is a different message and must fail.
    let set = WebhookSecretSet::single(test_key(1));
    let raw_body = br#"{"b":2,"a":1}"#;
    let header = set.sign(1_700_000_000, raw_body);
    let reserialized = br#"{"a":1,"b":2}"#;
    assert_eq!(
        set.verify(&header, 1_700_000_000, reserialized),
        Err(WebhookSigningError::SignatureMismatch),
        "a re-serialized body never verifies (the signature is over the raw bytes)"
    );
}

// --- Tolerance window (the replay window; inclusive boundary, absolute) -------------

#[test]
fn the_tolerance_window_accepts_inside_and_rejects_at_the_boundary() {
    let set = WebhookSecretSet::single(test_key(2));
    let raw_body = b"payload".as_slice();
    let header = set.sign(1_000_000, raw_body);
    // Inside the window (±300 s): verifies.
    set.verify(&header, 1_000_000 + 300 - 1, raw_body)
        .expect("just inside the window verifies");
    set.verify(&header, 1_000_000 - 300 + 1, raw_body)
        .expect("just inside on the past side verifies");
    // At and beyond the boundary: rejected — FUTURE-stamped deliveries
    // are equally rejected (the window is absolute, not one-sided).
    assert_eq!(
        set.verify(&header, 1_000_000 + 300, raw_body),
        Err(WebhookSigningError::OutsideTolerance {
            header_timestamp: 1_000_000,
            now_unix_seconds: 1_000_300,
        }),
        "the boundary is exclusive: exactly 300 s off rejects"
    );
    assert!(set.verify(&header, 1_000_000 + 301, raw_body).is_err());
    assert!(set.verify(&header, 1_000_000 - 301, raw_body).is_err());
}

// --- Replay surfaces ----------------------------------------------------------------

#[test]
fn a_replayed_delivery_is_the_same_bytes_and_still_verifies_within_the_window() {
    // The honest replay model (plan §2.3 + [S-83]): an attacker replaying
    // the exact captured message inside the tolerance window CANNOT be
    // distinguished from a legitimate duplicate at the signature layer —
    // the window bounds the exposure, and the RECEIVER's handler
    // idempotency (asserted below via the bookkeeping) is what collapses
    // duplicates. The signature layer's job is to reject replays OUTSIDE
    // the window, which the tolerance tests above pin.
    let set = WebhookSecretSet::single(test_key(3));
    let raw_body = b"duplicate".as_slice();
    let header = set.sign(2_000_000, raw_body);
    set.verify(&header, 2_000_100, raw_body)
        .expect("an exact replay inside the window still verifies — the window + handler idempotency are the defense");
    assert!(
        set.verify(&header, 2_000_000 + 301, raw_body).is_err(),
        "the same replay outside the window is rejected"
    );
}

#[test]
fn a_tampered_timestamp_or_body_breaks_the_signature() {
    let set = WebhookSecretSet::single(test_key(4));
    let raw_body = b"original".as_slice();
    let header = set.sign(3_000_000, raw_body);
    // A captured header replayed with a NEW timestamp: the v1 digest is
    // over the ORIGINAL timestamp, so the verify recomputes with the
    // header's timestamp and mismatches only if the attacker changed the
    // header timestamp itself — which changes the signed message. The
    // strict parse accepts the well-formed header; the digest over the
    // modified (t, body) pair mismatches the attacker's v1.
    let tampered = header.replace("t=3000000", "t=3000001");
    assert!(
        set.verify(&tampered, 3_000_001, raw_body).is_err(),
        "a header with a swapped timestamp never verifies (the digest is over the header timestamp)"
    );
    assert!(
        set.verify(&header, 3_000_000, b"modified".as_slice()).is_err(),
        "a modified body under the original header never verifies"
    );
}

#[test]
fn the_rolling_overlap_verifies_both_and_the_closure_revokes_the_previous() {
    let current = test_key(5);
    let previous = test_key(6);
    let rolling = WebhookSecretSet::rolling(current, previous);
    let raw_body = b"overlap".as_slice();
    let signed_with_current = rolling.sign(4_000_000, raw_body);
    // During the overlap BOTH secrets verify (the signer signs with the
    // current only — construct the previous-signed header from its own set).
    let previous_set = WebhookSecretSet::single(test_key(6));
    let signed_with_previous = previous_set.sign(4_000_000, raw_body);
    rolling
        .verify(&signed_with_current, 4_000_000, raw_body)
        .expect("the current secret's signature verifies");
    rolling
        .verify(&signed_with_previous, 4_000_000, raw_body)
        .expect("during the overlap the previous secret's signature ALSO verifies");
    // After the overlap closes the set is replaced with a single-secret
    // one — the previous signature no longer verifies (revocation by
    // replacement, record 21 §1).
    let closed = WebhookSecretSet::single(test_key(5));
    assert!(
        closed.verify(&signed_with_previous, 4_000_000, raw_body).is_err(),
        "after the overlap closes, the revoked previous secret never verifies"
    );
}

// --- The bookkeeping contract: idempotency keys and duplicate collapse ---------------

struct StaticSecrets {
    material: Vec<u8>,
}

impl WebhookSecretResolver for StaticSecrets {
    fn resolve(&self, _reference: &str) -> Result<Vec<u8>, WebhookDeliveryError> {
        Ok(self.material.clone())
    }
}

struct RecordingHttp {
    outcomes: Mutex<Vec<HttpPostOutcome>>,
}

impl RecordingHttp {
    fn new(outcomes: Vec<HttpPostOutcome>) -> Self {
        Self {
            outcomes: Mutex::new(outcomes),
        }
    }
}

impl OutboundHttpPost for RecordingHttp {
    fn post(
        &self,
        _url: &str,
        _headers: &[(String, String)],
        _raw_body: &[u8],
    ) -> Result<HttpPostOutcome, WebhookDeliveryError> {
        let mut outcomes = self.outcomes.lock().expect("test mutex");
        if outcomes.is_empty() {
            Ok(HttpPostOutcome::Responded { code: 200 })
        } else {
            Ok(outcomes.remove(0))
        }
    }
}

fn endpoint(tenant_id: TenantId) -> WebhookEndpointRecord {
    WebhookEndpointRecord {
        endpoint_id: "ep-1".to_string(),
        tenant_id,
        url: "https://receiver.example/hook".to_string(),
        secret_current_ref: "vault-ref-current".to_string(),
        secret_previous_ref: None,
        active: true,
    }
}

fn delivery(tenant_id: TenantId, key: &str, body: &str) -> WebhookDeliveryRecord {
    WebhookDeliveryRecord {
        delivery_id: format!("del-{key}"),
        tenant_id,
        delivery_key: key.to_string(),
        endpoint_id: "ep-1".to_string(),
        event_subject: "flexcore.v1.tenant.a.telemetry".to_string(),
        raw_body: body.to_string(),
        status: "PENDING".to_string(),
        next_attempt_seq: 1,
        next_attempt_not_before_unix_millis: None,
        last_response_code: None,
    }
}

#[test]
fn the_idempotency_key_collapses_duplicate_enqueues() {
    let store = MemoryWebhookDeliveryStore::new();
    let clock = 5_000i64;
    let dispatcher = WebhookDispatcher::new(
        &store,
        &RecordingHttp::new(vec![]),
        &StaticSecrets {
            material: vec![7u8; 32],
        },
        Box::new(move || clock),
        Box::new(|_, _| Ok(endpoint(tenant(tenant_uuid_static())))),
    );
    let first = dispatcher
        .enqueue(
            tenant(TENANT_UUID),
            "idem-key-1".to_string(),
            "ep-1".to_string(),
            "flexcore.v1.tenant.a.telemetry".to_string(),
            "{\"raw\":true}".to_string(),
            "del-1".to_string(),
        )
        .expect("the first booking lands");
    // The SAME (tenant, key) with the SAME content collapses to the
    // existing row — the idempotent-handler contract's sender half.
    let duplicate = dispatcher
        .enqueue(
            tenant(TENANT_UUID),
            "idem-key-1".to_string(),
            "ep-1".to_string(),
            "flexcore.v1.tenant.a.telemetry".to_string(),
            "{\"raw\":true}".to_string(),
            "del-1-duplicate".to_string(),
        )
        .expect("the duplicate booking collapses (never a second row)");
    assert_eq!(first, duplicate, "the same key returns the same delivery row");
    assert_eq!(
        first.delivery_id, duplicate.delivery_id,
        "the duplicate's would-be id never materializes"
    );
}

#[test]
fn the_same_key_with_different_content_is_a_named_conflict() {
    let store = MemoryWebhookDeliveryStore::new();
    let clock = 5_000i64;
    let dispatcher = WebhookDispatcher::new(
        &store,
        &RecordingHttp::new(vec![]),
        &StaticSecrets {
            material: vec![7u8; 32],
        },
        Box::new(move || clock),
        Box::new(|_, _| Ok(endpoint(tenant(tenant_uuid_static())))),
    );
    dispatcher
        .enqueue(
            tenant(TENANT_UUID),
            "idem-key-2".to_string(),
            "ep-1".to_string(),
            "flexcore.v1.tenant.a.telemetry".to_string(),
            "{\"attempt\":1}".to_string(),
            "del-2".to_string(),
        )
        .expect("the first booking lands");
    assert_eq!(
        dispatcher.enqueue(
            tenant(TENANT_UUID),
            "idem-key-2".to_string(),
            "ep-1".to_string(),
            "flexcore.v1.tenant.a.telemetry".to_string(),
            "{\"attempt\":2}".to_string(),
            "del-2b".to_string(),
        ),
        Err(WebhookDeliveryError::ConflictingDeliveryKey {
            key: "idem-key-2".to_string()
        }),
        "same key, different content is a conflict — the existing delivery wins"
    );
}

#[test]
fn the_idempotency_key_is_scoped_per_tenant() {
    // The key uniqueness is per (tenant, key): the same key string in
    // another tenant is a DIFFERENT delivery (the migration's UNIQUE
    // (tenant_id, delivery_key) contract, record 21 §1).
    let store = MemoryWebhookDeliveryStore::new();
    let clock = 5_000i64;
    let dispatcher = WebhookDispatcher::new(
        &store,
        &RecordingHttp::new(vec![]),
        &StaticSecrets {
            material: vec![7u8; 32],
        },
        Box::new(move || clock),
        Box::new(|tenant_id, _| {
            Ok(WebhookEndpointRecord {
                endpoint_id: "ep-1".to_string(),
                tenant_id: *tenant_id,
                url: "https://receiver.example/hook".to_string(),
                secret_current_ref: "vault-ref-current".to_string(),
                secret_previous_ref: None,
                active: true,
            })
        }),
    );
    dispatcher
        .enqueue(
            tenant(TENANT_UUID),
            "shared-key".to_string(),
            "ep-1".to_string(),
            "flexcore.v1.tenant.a.telemetry".to_string(),
            "{\"tenant\":\"a\"}".to_string(),
            "del-a".to_string(),
        )
        .expect("tenant A's booking lands");
    dispatcher
        .enqueue(
            tenant(OTHER_TENANT_UUID),
            "shared-key".to_string(),
            "ep-1".to_string(),
            "flexcore.v1.tenant.b.telemetry".to_string(),
            "{\"tenant\":\"b\"}".to_string(),
            "del-b".to_string(),
        )
        .expect("the SAME key string in tenant B is its own delivery (per-tenant key scope)");
}

// --- Attempt bookkeeping: book-before-attempt, retries, dead-letter ------------------

#[test]
fn a_failed_attempt_is_booked_before_the_call_and_scheduled_for_retry() {
    let store = MemoryWebhookDeliveryStore::new();
    let clock = Arc::new(AtomicI64::new(10_000));
    let clock_for_dispatcher = Arc::clone(&clock);
    let http = RecordingHttp::new(vec![HttpPostOutcome::TransportFailure {
        class: "TIMEOUT".to_string(),
    }]);
    let dispatcher = WebhookDispatcher::new(
        &store,
        &http,
        &StaticSecrets {
            material: vec![8u8; 32],
        },
        Box::new(move || clock_for_dispatcher.load(Ordering::SeqCst)),
        Box::new(|tenant_id, _| {
            Ok(WebhookEndpointRecord {
                endpoint_id: "ep-1".to_string(),
                tenant_id: *tenant_id,
                url: "https://receiver.example/hook".to_string(),
                secret_current_ref: "vault-ref-current".to_string(),
                secret_previous_ref: None,
                active: true,
            })
        }),
    );
    let booked = dispatcher
        .enqueue(
            tenant(TENANT_UUID),
            "retry-key".to_string(),
            "ep-1".to_string(),
            "flexcore.v1.tenant.a.telemetry".to_string(),
            "body".to_string(),
            "del-retry".to_string(),
        )
        .expect("the booking lands");
    let status = dispatcher
        .run_attempt(&booked)
        .expect("the attempt runs and records");
    assert_eq!(status, "PENDING", "a transport failure schedules a retry, never a silent drop");
    // The next attempt is not due before the backoff delay elapses (the
    // book-before-attempt + schedule shape; the per-attempt row ordering
    // is asserted by the signing/service in-crate tests).
    let scheduled = store
        .list_due_deliveries(10_001, 0, 10)
        .expect("the due list reads");
    assert!(
        scheduled.is_empty(),
        "the retry is not due before its backoff window"
    );
}

#[test]
fn exhausting_the_attempts_dead_letters_and_counts() {
    let store = MemoryWebhookDeliveryStore::new();
    let clock = Arc::new(AtomicI64::new(20_000));
    let clock_for_dispatcher = Arc::clone(&clock);
    let always_fail = FailingHttp;
    let dispatcher = WebhookDispatcher::new(
        &store,
        &always_fail,
        &StaticSecrets {
            material: vec![9u8; 32],
        },
        Box::new(move || clock_for_dispatcher.load(Ordering::SeqCst)),
        Box::new(|tenant_id, _| {
            Ok(WebhookEndpointRecord {
                endpoint_id: "ep-1".to_string(),
                tenant_id: *tenant_id,
                url: "https://receiver.example/hook".to_string(),
                secret_current_ref: "vault-ref-current".to_string(),
                secret_previous_ref: None,
                active: true,
            })
        }),
    );
    dispatcher
        .enqueue(
            tenant(TENANT_UUID),
            "dead-key".to_string(),
            "ep-1".to_string(),
            "flexcore.v1.tenant.a.telemetry".to_string(),
            "body".to_string(),
            "del-dead".to_string(),
        )
        .expect("the booking lands");
    // Drive the bounded attempt schedule to exhaustion.
    let mut status = String::new();
    for attempt in 1..=MAX_ATTEMPTS {
        assert!(attempt_allowed(attempt), "attempt {attempt} is within the bound");
        status = dispatcher
            .run_attempt(&store_due(&clock.load(Ordering::SeqCst), &store))
            .expect("each scheduled attempt runs");
        // Advance past the backoff window so the next attempt is due.
        let next_delay = backoff_delay_secs(attempt + 1) as i64 * 1_000 + 1;
        clock.fetch_add(next_delay, Ordering::SeqCst);
        if status == "DEAD_LETTERED" {
            break;
        }
    }
    assert_eq!(
        status, "DEAD_LETTERED",
        "exhausting the bound dead-letters the delivery (nothing silently dropped)"
    );
    assert!(
        attempt_allowed(MAX_ATTEMPTS + 1) == false,
        "beyond the bound no attempt is allowed"
    );
    assert_eq!(
        store.count_dead_letters().expect("the depth reads"),
        1,
        "the dead-letter depth metric reflects exactly the one exhausted delivery"
    );
}

fn store_due(_now: i64, store: &MemoryWebhookDeliveryStore) -> WebhookDeliveryRecord {
    store
        .list_due_deliveries(i64::MAX / 2, 0, 1)
        .expect("the due list reads")
        .into_iter()
        .next()
        .expect("exactly the scheduled delivery is due")
}

struct FailingHttp;

impl OutboundHttpPost for FailingHttp {
    fn post(
        &self,
        _url: &str,
        _headers: &[(String, String)],
        _raw_body: &[u8],
    ) -> Result<HttpPostOutcome, WebhookDeliveryError> {
        Ok(HttpPostOutcome::Responded { code: 500 })
    }
}

fn tenant_uuid_static() -> &'static str {
    TENANT_UUID
}

#[test]
fn the_backoff_schedule_is_bounded_and_registered() {
    // Attempt 1 immediate; then 2s * 2^(attempt-2), capped at 300 s
    // (record 21 §3 registered defaults — re-asserted at the public API).
    assert_eq!(backoff_delay_secs(1), 0, "the first retry is immediate");
    assert_eq!(backoff_delay_secs(2), 2);
    assert_eq!(backoff_delay_secs(3), 4);
    assert_eq!(backoff_delay_secs(4), 8);
    for attempt in 10..=MAX_ATTEMPTS {
        assert!(
            backoff_delay_secs(attempt) <= 300,
            "the schedule is capped at 300 s"
        );
    }
    assert!(!attempt_allowed(MAX_ATTEMPTS + 1));
    assert!(attempt_allowed(MAX_ATTEMPTS));
}
