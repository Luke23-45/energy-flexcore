// P13.09 / FIM-F08.03 — LIVE webhook HTTP delivery: the REAL
//             [`OutboundHttpPost`] implementation over reqwest, wired to
//             the existing webhook bookkeeping port.
//
// Implemented under FIM-F08 (final implementation milestone; the FIM
// standing decisions live in
// `docs/releases/implementation_ledger/README.md`). Contract source: plan
// §2.3 (the webhook delivery contract) and the P13.09 decision record
// (`docs/phases-11-20/phase-13-enterprise-identity-integration/records/
// 21_p13_09_webhook_service.md` §5, which names the real client as the
// composition step). The PORT, the signed-header scheme, the outcome
// vocabulary, the permanent-rejection classifier, and the dead-letter
// policy all live in [`crate::webhook_service`] and
// [`crate::webhook_signing`] and are consumed UNCHANGED — this module
// defines no new bookkeeping, no new signature rule, and no retry logic
// (the dispatcher owns the retry schedule; this transport NEVER retries).
//
// **WRITTEN-UNRUN.** This module has never been compiled or executed: no
// webhook has ever been transmitted over this client, no receiver has ever
// answered one, no timeout has ever fired. The first human build/test is
// the gate (program conventions §1). LIVE RECEIVER INTEROP (a real
// receiver verifying the transmitted signatures over real HTTPS) IS
// UNPROVEN — **HUMAN LAB GATE**.
//
// Design points (each is a recorded decision of this implementation):
//
// 1. DEPENDENCY POSTURE (FIM standing decision 1): `reqwest 0.13.5` is
//    already resolved in the committed `Cargo.lock` (transitive today, via
//    `opentelemetry-http`/`opentelemetry-otlp`), so its DIRECT-declaration
//    edge stays inside the lock's version set. Recorded honestly: the
//    DIRECT dependency line in `apps/control-plane/Cargo.toml` landed on
//    this wave — FIM-F07.01
//    added it for the JWKS fetcher (`HttpJwksSource`) with
//    `default-features = false, features = ["blocking",
//    "rustls-no-provider"]`; THIS module reuses that same edge for its
//    `blocking` client and adds NO second dependency line. The TLS feature
//    set that line carries (via the workspace-resolved rustls) is UNPROVEN
//    in this repo — no HTTPS request has ever been made by any code here;
//    live receiver interop over real HTTPS delivery is a **HUMAN LAB
//    GATE** (the gate regenerates `Cargo.lock` to promote reqwest to a
//    direct edge and resolves the TLS subgraph). A scheme reqwest cannot
//    serve (https without a working TLS backend) surfaces as a transport
//    failure — fail closed, a delivery is never silently downgraded.
//
// 2. SECRET POSTURE (P11.16, hard): NO secret material reaches this
//    module. The signature is computed by the dispatcher through the
//    EXISTING [`crate::webhook_service::WebhookSecretResolver`] (provider
//    references resolved at attempt time, material never persisted, never
//    logged); this transport receives only the ALREADY-SIGNED header
//    values and the raw body. No type here carries secret bytes; no log
//    line carries the body.
//
// 3. TIMEOUTS — connect + total, from THIS module's configuration
//    ([`WebhookHttpConfig`]; the endpoint record has no timeout fields, so
//    the budget is per-deployment configuration, not per-endpoint data).
//    The defaults are REGISTERED RECOMMENDATIONS (X-8: written before any
//    run could depend on them), never tuned values.
//
// 4. CLASSIFICATION — every outcome lands in the EXISTING
//    [`HttpPostOutcome`] vocabulary, closed over `Responded{code}` and
//    `TransportFailure{class}`: a received response maps to its status
//    code (the dispatcher's `is_permanent_rejection` classifier then
//    drives the dead-letter policy UNCHANGED); reqwest timeouts map to
//    `TIMEOUT`; connection failures map to `CONNECT_ERROR`; anything else
//    maps to `UNKNOWN` (the `TLS` class exists in the bookkeeping
//    vocabulary but reqwest's error surface does not expose a stable
//    TLS-specific discriminator, so a TLS handshake failure surfaces as
//    `UNKNOWN` — recorded honestly; the class vocabulary is unchanged).
//
// 5. BLOCKING CLIENT — the port trait is synchronous
//    (`OutboundHttpPost::post`), so the implementation uses
//    `reqwest::blocking`. The composition MUST run the dispatcher (and
//    therefore this transport) on a blocking thread
//    (`tokio::task::spawn_blocking` or a dedicated thread) — never inside
//    an async executor thread. Recorded as a composition requirement.

use crate::webhook_service::{HttpPostOutcome, OutboundHttpPost, WebhookDeliveryError};

/// The registered connect-timeout default in seconds (X-8 discipline:
/// written before any run could depend on it; changing it is a recorded
/// amendment to the P13.09 decision record).
pub const CONNECT_TIMEOUT_SECONDS_DEFAULT: u64 = 10;

/// The registered total-timeout default in seconds (the whole call —
/// connect, send, receive; X-8 discipline as above).
pub const TOTAL_TIMEOUT_SECONDS_DEFAULT: u64 = 30;

/// The transport configuration (one deployment): the connect and total
/// budgets applied to every call this client makes (module header
/// point 3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebhookHttpConfig {
    /// The connect budget in seconds. DEFAULT
    /// [`CONNECT_TIMEOUT_SECONDS_DEFAULT`].
    pub connect_timeout_secs: u64,
    /// The total budget in seconds. DEFAULT
    /// [`TOTAL_TIMEOUT_SECONDS_DEFAULT`].
    pub total_timeout_secs: u64,
}

impl Default for WebhookHttpConfig {
    fn default() -> Self {
        Self {
            connect_timeout_secs: CONNECT_TIMEOUT_SECONDS_DEFAULT,
            total_timeout_secs: TOTAL_TIMEOUT_SECONDS_DEFAULT,
        }
    }
}

/// The LIVE outbound HTTP transport: the real [`OutboundHttpPost`] over a
/// `reqwest::blocking` client (module header points 1 and 5). The type
/// carries ONLY the client and its budgets — no secret material, no body
/// cache, no endpoint state (the endpoint record stays the dispatcher's).
/// `Debug` is derived so the composition's logging shows the transport
/// shape (the client's own Debug carries no credentials; the secret-posture
/// test pins that no secret text can appear).
#[derive(Debug)]
pub struct ReqwestOutboundHttpPost {
    client: reqwest::blocking::Client,
}

impl ReqwestOutboundHttpPost {
    /// Builds the transport with its budgets. Client construction is
    /// fail-closed: a reqwest builder failure is a named backend error,
    /// never a panic and never a guessed client.
    pub fn new(config: WebhookHttpConfig) -> Result<Self, WebhookDeliveryError> {
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(config.connect_timeout_secs))
            .timeout(std::time::Duration::from_secs(config.total_timeout_secs))
            .build()
            .map_err(|_| WebhookDeliveryError::Backend {
                code: "E_TRANSPORT_CONFIG".to_string(),
                message: "the outbound webhook HTTP client could not be constructed \
                          (check the timeout configuration)"
                    .to_string(),
            })?;
        Ok(Self { client })
    }

    /// Maps a reqwest failure onto the closed transport-failure class
    /// vocabulary (module header point 4). The error's own text is NEVER
    /// carried into the outcome (the class is the bookkeeping's; the
    /// error text may name infrastructure internals).
    fn classify(error: &reqwest::Error) -> String {
        if error.is_timeout() {
            "TIMEOUT".to_string()
        } else if error.is_connect() {
            "CONNECT_ERROR".to_string()
        } else {
            "UNKNOWN".to_string()
        }
    }
}

impl OutboundHttpPost for ReqwestOutboundHttpPost {
    /// POSTs the raw body to `url` with the given headers (the signature
    /// header among them — computed by the dispatcher through the secret
    /// resolver; module header point 2). Returns the outcome in the
    /// EXISTING vocabulary; never retries; never logs the body; never
    /// panics on receiver misbehavior (a malformed response surfaces as
    /// its status code or a transport failure).
    fn post(
        &self,
        url: &str,
        headers: &[(String, String)],
        raw_body: &[u8],
    ) -> Result<HttpPostOutcome, WebhookDeliveryError> {
        // The URL is validated by reqwest itself at request build time; a
        // malformed URL is a builder failure → the UNKNOWN class (fail
        // closed, never a guessed outcome).
        let mut request = self.client.post(url);
        for (name, value) in headers {
            request = request.header(name, value);
        }
        let request = request.body(raw_body.to_vec());
        match request.send() {
            Ok(response) => Ok(HttpPostOutcome::Responded {
                code: response.status().as_u16(),
            }),
            Err(error) => Ok(HttpPostOutcome::TransportFailure {
                class: Self::classify(&error),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};

    /// Binds an ephemeral 127.0.0.1 listener (loopback-only — the recorded
    /// FIM allowance: deterministic, no external host, no gate) and
    /// returns its URL plus a handle collecting what was received.
    fn loopback_receiver(
        respond_with: u16,
    ) -> (String, Arc<Mutex<Vec<(String, Vec<(String, String)>, Vec<u8>)>>>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("ephemeral loopback bind");
        let address = listener.local_addr().expect("local addr");
        let received: Arc<Mutex<Vec<(String, Vec<(String, String)>, Vec<u8>)>>> =
            Arc::new(Mutex::new(Vec::new()));
        let received_handle = received.clone();
        std::thread::spawn(move || {
            // Serve exactly one request, then close (each test uses a
            // fresh receiver; a hung receiver cannot wedge the suite —
            // the client's total timeout bounds the call).
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buffer = vec![0u8; 65536];
                let read = stream.read(&mut buffer).unwrap_or(0);
                let raw = String::from_utf8_lossy(&buffer[..read]).to_string();
                // Parse the request line + headers (a minimal, honest
                // parse for the loopback receiver only).
                let mut parts = raw.split("\r\n\r\n");
                let head = parts.next().unwrap_or("").to_string();
                let body = parts.next().unwrap_or("").as_bytes().to_vec();
                let mut lines = head.split("\r\n");
                let request_line = lines.next().unwrap_or("").to_string();
                let headers: Vec<(String, String)> = lines
                    .filter_map(|line| {
                        line.split_once(':').map(|(name, value)| {
                            (name.trim().to_ascii_lowercase(), value.trim().to_string())
                        })
                    })
                    .collect();
                received_handle.lock().expect("received lock").push((
                    request_line,
                    headers,
                    body,
                ));
                let response = format!(
                    "HTTP/1.1 {} X\r\ncontent-length: 0\r\nconnection: close\r\n\r\n",
                    respond_with
                );
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.flush();
            }
        });
        (format!("http://{address}/hook"), received)
    }

    #[test]
    fn the_config_defaults_are_the_registered_recommendations() {
        let config = WebhookHttpConfig::default();
        assert_eq!(config.connect_timeout_secs, CONNECT_TIMEOUT_SECONDS_DEFAULT);
        assert_eq!(config.total_timeout_secs, TOTAL_TIMEOUT_SECONDS_DEFAULT);
    }

    #[test]
    fn a_live_loopback_delivery_reaches_the_receiver_with_the_signed_headers() {
        let (url, received) = loopback_receiver(200);
        let transport =
            ReqwestOutboundHttpPost::new(WebhookHttpConfig::default()).expect("transport built");
        let outcome = transport
            .post(
                &url,
                &[
                    ("content-type".to_string(), "application/json".to_string()),
                    (
                        crate::webhook_signing::scheme::SIGNATURE_HEADER.to_string(),
                        "t=1700000000,v1=ab".to_string(),
                    ),
                ],
                br#"{"event":"ScheduleProposed"}"#,
            )
            .expect("the call resolves");
        assert_eq!(outcome, HttpPostOutcome::Responded { code: 200 });
        assert!(outcome.is_success());
        let calls = received.lock().expect("received lock");
        assert_eq!(calls.len(), 1);
        let (request_line, headers, body) = &calls[0];
        assert!(request_line.starts_with("POST /hook "), "the request line: {request_line}");
        assert_eq!(body, br#"{"event":"ScheduleProposed"}"#, "the RAW body is transmitted");
        let signature = headers
            .iter()
            .find(|(name, _)| name == "flexcore-signature")
            .expect("the signature header travels");
        assert!(signature.starts_with("t=1700000000,v1="), "the header value: {signature}");
    }

    #[test]
    fn a_receiver_rejection_and_a_transport_failure_land_in_the_existing_vocabulary() {
        // A non-2xx response is a Responded{code} — the dispatcher's
        // classifier (unchanged) then decides retry vs dead-letter.
        let (url, _received) = loopback_receiver(410);
        let transport =
            ReqwestOutboundHttpPost::new(WebhookHttpConfig::default()).expect("transport built");
        let outcome = transport.post(&url, &[], b"{}").expect("the call resolves");
        assert_eq!(outcome, HttpPostOutcome::Responded { code: 410 });
        assert!(outcome.is_permanent_rejection(), "410 is a permanent rejection");
        assert_eq!(outcome.failure_class().as_deref(), Some("NON_2XX"));

        // A connection refusal (nothing is listening on the port anymore)
        // is a CONNECT_ERROR transport failure — retryable per the
        // unchanged classifier.
        let listener = TcpListener::bind("127.0.0.1:0").expect("ephemeral loopback bind");
        let dead_url = format!("http://{}/hook", listener.local_addr().expect("addr"));
        drop(listener);
        let transport =
            ReqwestOutboundHttpPost::new(WebhookHttpConfig::default()).expect("transport built");
        let outcome = transport.post(&dead_url, &[], b"{}").expect("the call resolves");
        assert_eq!(
            outcome,
            HttpPostOutcome::TransportFailure { class: "CONNECT_ERROR".to_string() },
        );
        assert!(!outcome.is_permanent_rejection());
        assert_eq!(outcome.failure_class().as_deref(), Some("CONNECT_ERROR"));
    }

    #[test]
    fn a_malformed_url_fails_closed_in_the_unknown_class() {
        let transport =
            ReqwestOutboundHttpPost::new(WebhookHttpConfig::default()).expect("transport built");
        let outcome = transport
            .post("not-a-url", &[], b"{}")
            .expect("the call resolves (never panics, never errors)");
        assert_eq!(
            outcome,
            HttpPostOutcome::TransportFailure { class: "UNKNOWN".to_string() },
            "a malformed URL is a fail-closed UNKNOWN transport failure",
        );
    }

    #[test]
    fn the_transport_carries_no_secret_material() {
        // The type's only fields are the client and (via the config) the
        // budgets: there is no secret field to inspect. The pin is
        // structural — the constructor takes no secret, `post` takes no
        // secret, and the Debug of the transport names no credential.
        let transport =
            ReqwestOutboundHttpPost::new(WebhookHttpConfig::default()).expect("transport built");
        let debugged = format!("{transport:?}");
        assert!(!debugged.contains("whsec_"));
        assert!(!debugged.contains("secret"));
        assert!(!debugged.contains("v1="));
    }
}
