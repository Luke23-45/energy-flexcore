// FIM-F04.04 — the SOCKET-LEVEL pinned suite for the live OpenADR 3.0 VTN
// HTTP transport (docs/releases/implementation_ledger/ledger_F04.md).
//
// **WRITTEN-UNRUN.** These tests have never been compiled or executed —
// the no-build/no-test bans (program conventions §1) make execution a
// HUMAN GATE. Nothing here is a conformance result: interoperability with
// a real OpenADR 3.0 VEN is a HUMAN LAB GATE (interop UNPROVEN; crate
// conventions §7.3); this file only pins the live mTLS/plaintext postures
// and the registration → poll round-trip that the in-module unit tests pin
// at the logic level (transport.rs / serving.rs).
//
// GATING (the recorded repo convention, as in the protocol-20305
// `tests/live_transport_gated.rs`): every test in this file is gated on
// `FLEXCORE_TEST_OPENADR_PKI_DIR` and SKIPS CLEANLY (prints a note and
// returns) when the variable is unset or empty. A mis-set gate (a
// non-directory) fails loudly instead.
//
// The mTLS tests need a human-provided PKI directory with EXACTLY these
// PEM files (NO key material exists in this repository — the
// composition-root rule; the human lab supplies all of it):
//
//   $FLEXCORE_TEST_OPENADR_PKI_DIR/
//     ca.pem          trust anchor: the CA that signed BOTH the server
//                     and the client (VEN) identities
//     server.pem      the VTN server identity; its SAN must include
//                     "localhost" (the client dials with that server name)
//     server.key.pem  the server private key
//     client.pem      the VEN client identity (client certificates are
//                     MANDATORY under the mTLS posture)
//     client.key.pem  the client private key
//
// The HTTP client side is a HAND-WRITTEN MINIMAL HTTP/1.1 exchange over a
// tokio-rustls client connection (the FIM standing-decision-1 pattern: a
// missing wire protocol is a hand-written minimal codec, not a new
// dependency). It writes one request with `Connection: close`, reads to
// EOF, and parses only the status line and the body after the header
// terminator; anything short of a parseable HTTP response is classified as
// a refusal.
//
// The wire payloads are the crate's JSON design shapes (record 21 §5):
// even a passing run here pins THIS crate's design surface, not OpenADR
// 3.0 conformance — a real 3.0 VEN will not interoperate until the
// UNVERIFIED path/payload register is checked at the human lab gate.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use rustls::RootCertStore;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;

use flexcore_openadr_adapter::model::{OpenAdrEvent, Ven};
use flexcore_openadr_adapter::serving::VtnServingState;
use flexcore_openadr_adapter::scope::DeliveryMode;
use flexcore_openadr_adapter::transport::{LiveTransportConfig, LiveVtnTransport, WireSecurityConfig};

/// The gate: `None` means "skip cleanly" (the NATS-adapter convention).
fn pki_dir() -> Option<PathBuf> {
    match std::env::var("FLEXCORE_TEST_OPENADR_PKI_DIR") {
        Ok(dir) if !dir.trim().is_empty() => {
            let dir = PathBuf::from(dir);
            if dir.is_dir() {
                Some(dir)
            } else {
                panic!(
                    "FLEXCORE_TEST_OPENADR_PKI_DIR is set but is not a directory: {}",
                    dir.display()
                );
            }
        }
        _ => {
            eprintln!(
                "skipping: FLEXCORE_TEST_OPENADR_PKI_DIR is not set (prepare the PKI \
                 directory documented in this file's header and export \
                 FLEXCORE_TEST_OPENADR_PKI_DIR=<path>)"
            );
            None
        }
    }
}

/// An ephemeral port for the run (bind-and-drop; the small bind race is a
/// recorded test-only tradeoff — a deployment never does this).
fn ephemeral_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("ephemeral bind");
    listener.local_addr().expect("local addr").port()
}

fn load_pem_certs(path: &Path) -> Vec<CertificateDer<'static>> {
    let file = std::fs::File::open(path)
        .unwrap_or_else(|error| panic!("open {}: {error}", path.display()));
    let mut reader = std::io::BufReader::new(file);
    let certs: Vec<CertificateDer<'static>> = rustls_pemfile::certs(&mut reader)
        .collect::<Result<Vec<_>, _>>()
        .unwrap_or_else(|error| panic!("decode {}: {error}", path.display()));
    assert!(!certs.is_empty(), "{}: no PEM certificates", path.display());
    certs
}

fn load_pem_key(path: &Path) -> PrivateKeyDer<'static> {
    let file = std::fs::File::open(path)
        .unwrap_or_else(|error| panic!("open {}: {error}", path.display()));
    let mut reader = std::io::BufReader::new(file);
    rustls_pemfile::private_key(&mut reader)
        .unwrap_or_else(|error| panic!("decode {}: {error}", path.display()))
        .unwrap_or_else(|| panic!("{}: no PEM private key", path.display()))
}

/// The composition-root-prepared serving state: the operator surface has
/// accepted `ven-1` and the DR program's event (modification 2) is queued
/// for it. The wire flow below exercises the registration UPDATE over this
/// record and the poll delivery of the queued event.
fn prepared_state() -> VtnServingState {
    let mut state =
        VtnServingState::new("openadr-vtn", vec![DeliveryMode::Polling, DeliveryMode::Push])
            .expect("valid state");
    state
        .registry
        .register(
            Ven::new(
                "ven-1".to_string(),
                "VEN 1".to_string(),
                None,
                60_000,
                true,
            )
            .expect("valid ven"),
        )
        .expect("operator enrolment");
    let descriptor = flexcore_openadr_adapter::model::EventDescriptor::new(
        "evt-live".to_string(),
        2,
        "lab-market".to_string(),
        3,
        false,
    )
    .expect("valid descriptor");
    let signal = flexcore_openadr_adapter::model::EventSignal::new(
        "simple".to_string(),
        flexcore_openadr_adapter::model::SignalUnits::Kilowatts,
        vec![flexcore_openadr_adapter::model::Interval::new(
            1,
            1_700_000_000_000,
            1_800_000,
            1_500.0,
        )
        .expect("valid interval")],
    )
    .expect("valid signal");
    let event = OpenAdrEvent::new(
        descriptor,
        vec!["ven-1".to_string()],
        vec![signal],
        "program-lab".to_string(),
    )
    .expect("valid event");
    state.queue_event(event).expect("queued");
    state
}

/// Spawns the live transport on a background thread (it builds and blocks
/// on its own current-thread runtime, exactly as a composition root
/// would). The thread OWNS the serving state and hands it BACK through the
/// join handle, so the test can inspect the delivery log after shutdown.
fn spawn_live_server(
    config: LiveTransportConfig,
    state: VtnServingState,
) -> (
    std::thread::JoinHandle<VtnServingState>,
    Arc<tokio::sync::Notify>,
) {
    let transport = LiveVtnTransport::new(config, design())
        .expect("the live transport constructs fail-closed");
    let shutdown = transport.shutdown_handle();
    let handle = std::thread::spawn(move || {
        let mut state = state;
        transport
            .serve(&mut state)
            .expect("the live server serves until shutdown or a named failure");
        state
    });
    (handle, shutdown)
}

/// The canonical designed surface (the route data the serving layer reads).
fn design() -> flexcore_openadr_adapter::endpoints::VtnEndpointDesign {
    flexcore_openadr_adapter::endpoints::VtnEndpointDesign::canonical_vtn_design()
        .expect("valid canonical design")
}

/// Waits until the listener accepts plain TCP connections (server-startup
/// synchronisation; bounded so a broken gate FAILS rather than hangs).
async fn wait_for_listener(port: u16) {
    for _ in 0..100 {
        if TcpStream::connect(("127.0.0.1", port)).await.is_ok() {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    panic!("the live server did not accept within 2 seconds");
}

/// The outcome of one minimal HTTP/1.1 client exchange.
enum ClientOutcome {
    /// A parseable HTTP response (status line + body after the header
    /// terminator).
    Served { status: u16, body: Vec<u8> },
    /// No valid HTTP response was served: the connection was refused
    /// (handshake failure, alert, EOF, timeout, or a parse failure).
    Refused,
}

/// One minimal HTTP/1.1 exchange over tokio-rustls, with an optional
/// client certificate identity. Bounded by a 5-second timeout so a broken
/// gate fails loudly instead of hanging.
async fn tls_client_exchange(
    pki: &Path,
    port: u16,
    method: &str,
    path: &str,
    body: Option<&str>,
    client_identity: Option<(&str, &str)>,
) -> ClientOutcome {
    let exchange = async {
        let mut roots = RootCertStore::empty();
        for anchor in load_pem_certs(&pki.join("ca.pem")) {
            roots.add(anchor).expect("trust anchor added");
        }
        let builder = rustls::ClientConfig::builder().with_root_certificates(roots);
        let config = match client_identity {
            Some((cert_name, key_name)) => builder.with_client_auth_cert(
                load_pem_certs(&pki.join(cert_name)),
                load_pem_key(&pki.join(key_name)),
            ),
            None => Ok(builder.with_no_client_auth()),
        }
        .expect("client config");
        let connector = TlsConnector::from(Arc::new(config));
        let server_name = ServerName::try_from("localhost".to_string())
            .expect("server name (the server.pem SAN must cover localhost)");
        let tcp = TcpStream::connect(("127.0.0.1", port)).await.expect("tcp connect");
        let mut tls = connector.connect(server_name, tcp).await?;
        let request = match body {
            Some(body) => format!(
                "{method} {path} HTTP/1.1\r\nHost: localhost\r\nContent-Type: \
                 application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            ),
            None => format!(
                "{method} {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"
            ),
        };
        tls.write_all(request.as_bytes()).await?;
        let mut raw = Vec::new();
        tls.read_to_end(&mut raw).await?;
        Ok::<Vec<u8>, std::io::Error>(raw)
    };
    let raw = match tokio::time::timeout(std::time::Duration::from_secs(5), exchange).await {
        Ok(Ok(raw)) => raw,
        Ok(Err(_)) | Err(_) => return ClientOutcome::Refused,
    };
    parse_minimal_response(&raw)
}

/// One minimal PLAIN HTTP/1.1 exchange (the plaintext posture; no TLS).
async fn plain_client_exchange(port: u16, method: &str, path: &str) -> ClientOutcome {
    let exchange = async {
        let mut tcp = TcpStream::connect(("127.0.0.1", port)).await?;
        let request = format!(
            "{method} {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"
        );
        tcp.write_all(request.as_bytes()).await?;
        let mut raw = Vec::new();
        tcp.read_to_end(&mut raw).await?;
        Ok::<Vec<u8>, std::io::Error>(raw)
    };
    let raw = match tokio::time::timeout(std::time::Duration::from_secs(5), exchange).await {
        Ok(Ok(raw)) => raw,
        Ok(Err(_)) | Err(_) => return ClientOutcome::Refused,
    };
    parse_minimal_response(&raw)
}

/// Parses the minimal response: a status line, then the body.
fn parse_minimal_response(raw: &[u8]) -> ClientOutcome {
    let text = String::from_utf8_lossy(raw);
    let status_line = match text.lines().next() {
        Some(line) => line,
        None => return ClientOutcome::Refused,
    };
    let mut parts = status_line.split(' ');
    let _version = parts.next();
    let status: u16 = match parts.next().and_then(|code| code.parse().ok()) {
        Some(code) => code,
        None => return ClientOutcome::Refused,
    };
    let body = match raw.windows(4).position(|window| window == b"\r\n\r\n") {
        Some(index) => raw[index + 4..].to_vec(),
        None => Vec::new(),
    };
    ClientOutcome::Served { status, body }
}

// --- FIM-F04.04 (a): a client without a certificate is refused (mTLS) ------

#[tokio::test]
async fn a_client_without_a_certificate_is_refused() {
    let Some(pki) = pki_dir() else { return };
    let port = ephemeral_port();
    let config = LiveTransportConfig {
        bind_address: std::net::SocketAddr::from(([127, 0, 0, 1], port)),
        security: WireSecurityConfig::Mtls {
            certificate_chain_path: pki.join("server.pem").display().to_string(),
            private_key_path: pki.join("server.key.pem").display().to_string(),
            trust_anchor_paths: vec![pki.join("ca.pem").display().to_string()],
        },
    };
    let (server_thread, shutdown) = spawn_live_server(config, prepared_state());
    wait_for_listener(port).await;

    // The anonymous client (no client certificate) gets no HTTP response
    // at all: the required client-certificate verifier refuses it
    // fail-closed at the TLS layer.
    let outcome = tls_client_exchange(&pki, port, "GET", "/vens", None, None).await;
    match outcome {
        ClientOutcome::Refused => {}
        ClientOutcome::Served { status, .. } => {
            panic!(
                "an anonymous client was SERVED a response (status {status}) — the \
                 mandatory client-certificate posture is broken"
            );
        }
    }

    shutdown.notify_one();
    server_thread.join().expect("server thread stops cleanly");
}

// --- FIM-F04.04 (b): the live registration → poll round-trip over mTLS ------

#[tokio::test]
async fn b_the_live_registration_and_poll_round_trip_serves_over_mtls() {
    let Some(pki) = pki_dir() else { return };
    let port = ephemeral_port();
    let config = LiveTransportConfig {
        bind_address: std::net::SocketAddr::from(([127, 0, 0, 1], port)),
        security: WireSecurityConfig::Mtls {
            certificate_chain_path: pki.join("server.pem").display().to_string(),
            private_key_path: pki.join("server.key.pem").display().to_string(),
            trust_anchor_paths: vec![pki.join("ca.pem").display().to_string()],
        },
    };
    let (server_thread, shutdown) = spawn_live_server(config, prepared_state());
    wait_for_listener(port).await;

    // 1. A NEW VEN registers over the wire claiming acceptance: it is
    //    CREATED (201) and returned UNACCEPTED — the operator surface owns
    //    the flag (the model.rs rule), never the wire.
    let wire_ven = serde_json::json!({
        "ven_id": "ven-2",
        "ven_name": "VEN 2",
        "push_endpoint": null,
        "poll_frequency_millis": 60_000,
        "accepted": true
    });
    let outcome = tls_client_exchange(
        &pki,
        port,
        "POST",
        "/registration",
        Some(wire_ven.to_string().as_str()),
        Some(("client.pem", "client.key.pem")),
    )
    .await;
    let body = match outcome {
        ClientOutcome::Served { status: 201, body } => body,
        ClientOutcome::Served { status, .. } => panic!("the new VEN got status {status}; expected 201"),
        ClientOutcome::Refused => panic!("the ALLOWED client was refused — the mTLS posture is broken"),
    };
    let served: serde_json::Value = serde_json::from_slice(&body).expect("decodes");
    assert_eq!(served["ven_id"], "ven-2");
    assert_eq!(
        served["accepted"], false,
        "a wire registration is never self-accepted"
    );

    // 2. The pre-registered VEN re-registers over the wire: UPDATED (200)
    //    and the operator acceptance is PRESERVED.
    let wire_ven = serde_json::json!({
        "ven_id": "ven-1",
        "ven_name": "VEN 1 renamed",
        "push_endpoint": null,
        "poll_frequency_millis": 30_000,
        "accepted": false
    });
    let outcome = tls_client_exchange(
        &pki,
        port,
        "POST",
        "/registration",
        Some(wire_ven.to_string().as_str()),
        Some(("client.pem", "client.key.pem")),
    )
    .await;
    let body = match outcome {
        ClientOutcome::Served { status: 200, body } => body,
        ClientOutcome::Served { status, .. } => panic!("the update got status {status}; expected 200"),
        ClientOutcome::Refused => panic!("the ALLOWED client was refused — the mTLS posture is broken"),
    };
    let served: serde_json::Value = serde_json::from_slice(&body).expect("decodes");
    assert_eq!(served["accepted"], true, "the wire cannot flip operator acceptance");
    assert_eq!(served["poll_frequency_millis"], 30_000);

    // 3. The VEN polls and is served the queued event: the event set the
    //    plan declares, each event carrying the version in force (its
    //    modification number) in its own body.
    let outcome = tls_client_exchange(
        &pki,
        port,
        "GET",
        "/poll/ven-1",
        None,
        Some(("client.pem", "client.key.pem")),
    )
    .await;
    let body = match outcome {
        ClientOutcome::Served { status: 200, body } => body,
        ClientOutcome::Served { status, .. } => panic!("the poll got status {status}; expected 200"),
        ClientOutcome::Refused => panic!("the ALLOWED client was refused — the mTLS posture is broken"),
    };
    let events: serde_json::Value = serde_json::from_slice(&body).expect("decodes");
    let events = events.as_array().expect("the event set is a JSON array");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0]["descriptor"]["event_id"], "evt-live");
    assert_eq!(
        events[0]["descriptor"]["modification_number"], 2,
        "the served event carries the version in force"
    );

    // 4. The wire-level error path: an unknown VEN poll is the recorded
    //    registry refusal text, verbatim.
    let outcome = tls_client_exchange(
        &pki,
        port,
        "GET",
        "/poll/ven-ghost",
        None,
        Some(("client.pem", "client.key.pem")),
    )
    .await;
    match outcome {
        ClientOutcome::Served { status: 404, body } => {
            let record: serde_json::Value = serde_json::from_slice(&body).expect("decodes");
            let expected = flexcore_openadr_adapter::vtn::VtnRegistryError::VenNotRegistered {
                ven_id: "ven-ghost".to_string(),
            }
            .to_string();
            assert_eq!(record["error"], expected, "the recorded error text is verbatim");
        }
        ClientOutcome::Served { status, .. } => {
            panic!("the unknown-VEN poll got status {status}; expected 404");
        }
        ClientOutcome::Refused => panic!("the ALLOWED client was refused"),
    }

    shutdown.notify_one();
    // The thread hands the serving state back: the delivery log records
    // the poll delivery with the event version in force (FIM-F04.03).
    let state = server_thread.join().expect("server thread stops cleanly");
    let log = state.delivery_log();
    assert_eq!(log.len(), 1);
    assert_eq!(log[0].ven_id, "ven-1");
    assert_eq!(log[0].event_id, "evt-live");
    assert_eq!(log[0].modification_number, 2, "the version in force at delivery");
    assert_eq!(log[0].mode, DeliveryMode::Polling);
}

// --- FIM-F04.04 (c): the plaintext posture serves only as configured -------

#[tokio::test]
async fn c_the_recorded_plaintext_posture_serves_the_designed_surface() {
    let Some(_gate) = pki_dir() else { return };
    let port = ephemeral_port();
    // The plaintext posture carries its recorded lab-posture reason (the
    // recorded-posture-reason precedent): it travels with the deployment.
    let config = LiveTransportConfig {
        bind_address: std::net::SocketAddr::from(([127, 0, 0, 1], port)),
        security: WireSecurityConfig::Plaintext {
            lab_posture_reason: "lab posture: isolated test-lab network; the VTN listener is \
                 reachable only inside the lab segment"
                .to_string(),
        },
    };
    let (server_thread, shutdown) = spawn_live_server(config, prepared_state());
    wait_for_listener(port).await;

    // The plain client is served the operator inventory (the
    // FlexCore-specific designed surface) with the pre-accepted VEN.
    let outcome = plain_client_exchange(port, "GET", "/vens").await;
    let body = match outcome {
        ClientOutcome::Served { status: 200, body } => body,
        ClientOutcome::Served { status, .. } => panic!("the inventory got status {status}; expected 200"),
        ClientOutcome::Refused => panic!("the plaintext posture did not serve as configured"),
    };
    let vens: serde_json::Value = serde_json::from_slice(&body).expect("decodes");
    let vens = vens.as_array().expect("the VEN inventory is a JSON array");
    assert_eq!(vens.len(), 1);
    assert_eq!(vens[0]["ven_id"], "ven-1");
    assert_eq!(vens[0]["accepted"], true);

    shutdown.notify_one();
    let _state = server_thread.join().expect("server thread stops cleanly");
}
