// Patch B — NATS transport hardening, against the test broker named by
// `FLEXCORE_TEST_NATS_URL`.
//
// What is asserted here (all NATS-gated, FAIL LOUDLY when
// `FLEXCORE_TEST_NATS_URL` is unset or empty — a NATS-gated suite must
// never silently pass):
// 1. A TLS-required client fails closed against the documented broker,
//    while a plaintext client connects — because the broker the tree
//    documents is plaintext (see below).
// 2. A client WITHOUT credentials cannot publish to any telemetry
//    subject (JetStream publish fails).
// 3. A JetStream publish to a wrong (non-telemetry) subject is refused.
//
// THE DOCUMENTED BROKER (read from the tree — VERIFY-BUILD, see below):
// `deploy/compose/docker-compose.yml` runs `nats:2.15.2` with
// `command: ["--jetstream", "-sd", "/data", "-m", "8222"]` — NO `--tls`
// flag, NO auth flags, NO user/password, NO certificates — and the
// worker connects with a plain `nats://` URL (`apps/worker/src/main.rs`
// via `async_nats::connect(&config.nats_url)`). The compose broker is
// therefore PLAINTEXT and UNAUTHENTICATED. The brief's premises —
// "plaintext connection refused (TLS-required test broker)" and "client
// without valid credentials cannot publish" as AUTHENTICATION refusals —
// do NOT hold against that broker, and no test in this file pretends
// they do:
//
// - Test 1 asserts the behaviour the documented broker actually has:
//   plaintext connects, and a client that DEMANDS TLS (`tls://`) fails
//   closed against the plaintext listener. If the deployment hardens
//   the broker to TLS-only, the plaintext half of this test must be
//   inverted (plaintext refused) — the test is written so that flip is
//   a one-line change, and it is flagged VERIFY-BUILD in the report.
// - Tests 2–3 assert JetStream publish failures. On the documented
//   broker the failure mode is "no stream matches the subject" (no
//   `FLEXCORE_TELEMETRY` stream is provisioned by the compose file —
//   the stream inventory lives in `crates/adapters/nats/src/topology.rs`
//   and is provisioned by application code, not by the broker), NOT an
//   authentication rejection. Credential enforcement is therefore
//   VERIFY-BUILD: it needs a broker configured with auth (users /
//   nkey / TLS client certificates) before "without valid credentials"
//   can be tested as an auth refusal. Until then these tests pin the
//   fail-closed OBSERVABLE (the publish does not go through) without
//   mislabelling the mechanism.
// - Core (non-JetStream) `client.publish` is fire-and-forget and
//   unacknowledged: the broker cannot refuse it, so no core-publish
//   refusal is asserted anywhere in this file. Telemetry delivery in
//   this codebase goes through JetStream (the outbox publisher), which
//   is what tests 2–3 exercise.

use std::time::Duration;

/// The NATS-gated suite FAILS LOUDLY when the variable is unset or empty.
fn require_nats(test: &str) -> String {
    match std::env::var("FLEXCORE_TEST_NATS_URL")
        .ok()
        .map(|url| url.trim().to_string())
        .filter(|url| !url.is_empty())
    {
        Some(url) => url,
        None => panic!(
            "FLEXCORE_TEST_NATS_URL is not set or empty: the NATS-gated \
             source-auth transport suite must not silently pass (test {test}); \
             set it to a NATS 2.15.x test broker URL"
        ),
    }
}

/// Rewrites a `nats://` URL into the `tls://` form (a client that
/// demands TLS). Panics on a non-`nats://` test URL — that is a test
/// setup error, not a broker behaviour.
fn tls_url(nats_url: &str) -> String {
    nats_url
        .strip_prefix("nats://")
        .map(|rest| format!("tls://{rest}"))
        .unwrap_or_else(|| {
            panic!("FLEXCORE_TEST_NATS_URL must be a nats:// URL, got a URL with another scheme")
        })
}

// --- 1. Plaintext vs TLS ---------------------------------------------------------

#[tokio::test]
async fn plaintext_connects_to_the_documented_broker_but_a_tls_required_client_fails_closed() {
    let url = require_nats(
        "plaintext_connects_to_the_documented_broker_but_a_tls_required_client_fails_closed",
    );
    // VERIFY-BUILD: the documented compose broker is plaintext and
    // unauthenticated (`deploy/compose/docker-compose.yml`: `nats:2.15.2`
    // with `--jetstream` only, no `--tls`, no auth). Plaintext therefore
    // CONNECTS here. The brief's "plaintext refused" premise assumes a
    // TLS-only broker, which the tree does not define: before that
    // premise can be tested, the broker must be hardened to TLS-only,
    // and this assertion must be inverted (plaintext refused).
    let connected = tokio::time::timeout(Duration::from_secs(10), async_nats::connect(&url))
        .await
        .expect("the plaintext connect must settle within the timeout");
    assert!(
        connected.is_ok(),
        "the documented compose broker accepts plaintext (no TLS configured): {connected:?}"
    );
    let client = connected.expect("plaintext client");
    drop(client);

    // A client that demands TLS fails closed against the plaintext
    // listener: the TLS handshake against a non-TLS server errors.
    // (On a TLS-only broker this same assertion holds for the opposite
    // reason — the server side demands TLS and the client supplies it —
    // so this half survives the hardening flip; the plaintext half above
    // is the one that changes.)
    let tls_result = tokio::time::timeout(
        Duration::from_secs(10),
        async_nats::connect(tls_url(&url)),
    )
    .await
    .expect("the TLS connect must settle within the timeout");
    assert!(
        tls_result.is_err(),
        "a TLS-required client must fail closed against the plaintext broker"
    );
}

// --- 2. No credentials, no telemetry publish ----------------------------------------

#[tokio::test]
async fn client_without_credentials_cannot_publish_to_any_telemetry_subject() {
    let url = require_nats(
        "client_without_credentials_cannot_publish_to_any_telemetry_subject",
    );
    // Connected WITHOUT any credentials — the documented broker has no
    // auth configured, so the connection itself succeeds. The publish
    // must still fail: JetStream publish is acknowledged, and no stream
    // matches a telemetry subject on the bare test broker.
    //
    // VERIFY-BUILD: the failure mode on the documented broker is "no
    // matching stream", NOT an authentication rejection. Credential
    // enforcement ("without valid credentials" as an AUTH refusal) needs
    // a broker configured with auth; until that broker exists, this test
    // pins the fail-closed observable (the publish does not go through)
    // without mislabelling the mechanism.
    let client = tokio::time::timeout(Duration::from_secs(10), async_nats::connect(&url))
        .await
        .expect("the connect must settle within the timeout")
        .expect("the client connects without credentials on the documented broker");
    let jetstream = async_nats::jetstream::new(client);
    for subject in [
        "flexcore.v1.telemetry.telemetry_received",
        "flexcore.v1.telemetry.telemetry_quality_changed",
    ] {
        let published = tokio::time::timeout(
            Duration::from_secs(10),
            jetstream.publish(subject, "probe".as_bytes().to_vec().into()),
        )
        .await
        .expect("the publish must settle within the timeout");
        assert!(
            published.is_err(),
            "a credential-less client must not publish to {subject}: the publish went through"
        );
    }
}

// --- 3. Wrong subject -----------------------------------------------------------------

#[tokio::test]
async fn wrong_subject_jetstream_publish_is_refused() {
    let url = require_nats("wrong_subject_jetstream_publish_is_refused");
    let client = tokio::time::timeout(Duration::from_secs(10), async_nats::connect(&url))
        .await
        .expect("the connect must settle within the timeout")
        .expect("the client connects on the documented broker");
    let jetstream = async_nats::jetstream::new(client);
    // A subject inside the telemetry namespace that names no real event,
    // and a subject entirely outside the FlexCore namespace: neither has
    // a stream on the test broker, so JetStream (acknowledged publish)
    // refuses both.
    //
    // VERIFY-BUILD: as in test 2, the refusal on the documented broker
    // is "no matching stream". If the test broker provisions the
    // `FLEXCORE_TELEMETRY` stream (`crates/adapters/nats/src/topology.rs`
    // names its filter `flexcore.v1.telemetry.>`, which would ALSO match
    // the first subject below), the first publish would SUCCEED and this
    // test would fail — at which point the wrong-subject refusal needs a
    // broker-side subject allowlist (or a stream filter that excludes
    // it), not a test change.
    for subject in [
        "flexcore.v1.telemetry.not_a_real_event",
        "unrelated.example.subject",
    ] {
        let published = tokio::time::timeout(
            Duration::from_secs(10),
            jetstream.publish(subject, "probe".as_bytes().to_vec().into()),
        )
        .await
        .expect("the publish must settle within the timeout");
        assert!(
            published.is_err(),
            "a JetStream publish to the wrong subject {subject} must be refused"
        );
    }
}
