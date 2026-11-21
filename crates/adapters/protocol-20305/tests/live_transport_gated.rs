// FIM-F01.04 — the SOCKET-LEVEL pinned suite for the live IEEE 2030.5
// server transport (docs/releases/implementation_ledger/ledger_F01.md).
//
// **WRITTEN-UNRUN.** These tests have never been compiled or executed —
// the no-build/no-test bans (program conventions §1) make execution a
// HUMAN GATE. Nothing here is a conformance result: conformance against a
// real IEEE 2030.5 / CSIP client is a HUMAN LAB GATE (interop UNPROVEN;
// crate conventions §7.3); this file only pins the mTLS refusal behaviours
// and the live DERControl flow that the in-module unit tests pin at the
// logic level (transport.rs).
//
// GATING (the recorded repo convention, as in the NATS adapter's
// `tests/integration.rs`): every test in this file is gated on
// `FLEXCORE_TEST_20305_PKI_DIR` and SKIPS CLEANLY (prints a note and
// returns) when the variable is unset or empty. A mis-set gate (a
// non-directory) fails loudly instead.
//
// The gate needs a human-provided PKI directory with EXACTLY these PEM
// files (NO key material exists in this repository — the composition-root
// rule; the human lab supplies all of it):
//
//   $FLEXCORE_TEST_20305_PKI_DIR/
//     ca.pem                       trust anchor: the CA that signed BOTH
//                                 the server and the client identities
//     server.pem                   the AggregatorServer identity; its SAN
//                                 must include "localhost" (the client
//                                 dials with that server name)
//     server.key.pem               the server private key
//     client.pem                   a device identity that IS allowed
//     client.key.pem               its private key
//     client_not_allowed.pem       a device identity that is NOT allowed
//     client_not_allowed.key.pem   its private key
//
// The server-side fingerprint allowlist is COMPUTED at test time from
// `client.pem` with the crate's own `certificate_fingerprint_hex` — no
// fingerprint is hardcoded, and the un-allowed client's certificate must
// simply be a DIFFERENT certificate signed by the SAME CA (so the rustls
// client-certificate verifier accepts it and only the allowlist gate
// refuses it — that separation is the point of test B).
//
// The HTTP client side is a HAND-WRITTEN MINIMAL HTTP/1.1 exchange over a
// tokio-rustls client connection (the FIM standing-decision-1 pattern: a
// missing wire protocol is a hand-written minimal codec, not a new
// dependency). It writes one GET with `Connection: close`, reads to EOF,
// and parses only the status line; anything short of a parseable HTTP
// response is classified as a refusal.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use rustls::RootCertStore;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;

use flexcore_protocol_20305::addressing::AddressPlan;
use flexcore_protocol_20305::model::{
    DERControl, DERControlBase, DERControlList, ListMeta, Sfdi, TimeType,
};
use flexcore_protocol_20305::proposal::{ExternalControlProposal, ExternalEventSource};
use flexcore_protocol_20305::server::{
    AggregatorServer, AggregatorTransport, MtlsRequirements, ServerLimits, StoredResource,
};
use flexcore_protocol_20305::transport::{HyperTransport, LiveTransportConfig};
use flexcore_protocol_20305::ADAPTER_ID;

/// The gate: `None` means "skip cleanly" (the NATS-adapter convention).
fn pki_dir() -> Option<PathBuf> {
    match std::env::var("FLEXCORE_TEST_20305_PKI_DIR") {
        Ok(dir) if !dir.trim().is_empty() => {
            let dir = PathBuf::from(dir);
            if dir.is_dir() {
                Some(dir)
            } else {
                panic!(
                    "FLEXCORE_TEST_20305_PKI_DIR is set but is not a directory: {}",
                    dir.display()
                );
            }
        }
        _ => {
            eprintln!(
                "skipping: FLEXCORE_TEST_20305_PKI_DIR is not set (prepare the PKI \
                 directory documented in this file's header and export \
                 FLEXCORE_TEST_20305_PKI_DIR=<path>)"
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

/// Builds an AggregatorServer whose control list carries one registered
/// DERControl (the composition-root registration the live flow serves).
fn server_with_registered_control() -> (AggregatorServer, AddressPlan) {
    let plan = AddressPlan::csip_default();
    let mut server = AggregatorServer::new(
        flexcore_protocol_20305::scope::aggregator_manifest(ADAPTER_ID).expect("valid manifest"),
        plan.clone(),
        ServerLimits::new(8, 8).expect("valid limits"),
        MtlsRequirements::new(vec!["csip-device-ca".to_string()], true).expect("valid tls"),
    )
    .expect("valid server");
    server
        .install(
            plan.der_control_list.clone(),
            StoredResource::DERControlList(DERControlList {
                meta: ListMeta::unpaged(1),
                poll_rate: Some(60),
                der_controls: vec![DERControl::new(
                    "ctrl-live".to_string(),
                    Some(TimeType(1)),
                    Some(TimeType(1000)),
                    Some(300),
                    None,
                    None,
                    DERControlBase {
                        op_mod_gen_lim_w: Some(2000.0),
                        ..DERControlBase::default()
                    },
                )
                .expect("valid control")],
            }),
        )
        .expect("install");
    (server, plan)
}

/// Spawns the live transport on a background thread (it builds and blocks
/// on its own current-thread runtime, exactly as a composition root
/// would) and returns the thread handle plus the shutdown handle. The
/// thread OWNS the server: the handler borrow never crosses threads.
fn spawn_live_server(
    pki: &Path,
    port: u16,
    server: AggregatorServer,
) -> (std::thread::JoinHandle<()>, Arc<tokio::sync::Notify>) {
    let config = LiveTransportConfig {
        bind_address: std::net::SocketAddr::from(([127, 0, 0, 1], port)),
        certificate_chain_path: pki.join("server.pem").display().to_string(),
        private_key_path: pki.join("server.key.pem").display().to_string(),
        trust_anchor_paths: vec![pki.join("ca.pem").display().to_string()],
        // The allowlist is computed at test time from client.pem — no
        // fingerprint is hardcoded anywhere.
        allowed_client_fingerprints: vec![
            flexcore_protocol_20305::transport::certificate_fingerprint_hex(
                &load_pem_certs(&pki.join("client.pem"))[0],
            ),
        ],
        check_revocation: false,
    };
    let transport = HyperTransport::new(config).expect("valid transport");
    let shutdown = transport.shutdown_handle();
    let handle = std::thread::spawn(move || {
        let mut server = server;
        Box::new(transport)
            .serve(&mut server)
            .expect("the live server serves until shutdown or a named failure");
    });
    (handle, shutdown)
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
    /// A parseable HTTP status line (and the body bytes after the header
    /// terminator).
    Served { status: u16, body: Vec<u8> },
    /// No valid HTTP response was served: the connection was refused
    /// (handshake failure, alert, EOF, timeout, or a parse failure).
    Refused,
}

/// One minimal HTTP/1.1 GET over tokio-rustls, with an optional client
/// certificate identity. Bounded by a 5-second timeout so a broken gate
/// fails loudly instead of hanging.
async fn client_get(
    pki: &Path,
    port: u16,
    path: &str,
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
        let tcp = TcpStream::connect(("127.0.0.1", port))
            .await
            .expect("tcp connect");
        let mut tls = connector.connect(server_name, tcp).await?;
        tls.write_all(
            format!("GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
                .as_bytes(),
        )
        .await?;
        let mut raw = Vec::new();
        tls.read_to_end(&mut raw).await?;
        Ok::<Vec<u8>, std::io::Error>(raw)
    };
    let raw = match tokio::time::timeout(std::time::Duration::from_secs(5), exchange).await {
        Ok(Ok(raw)) => raw,
        Ok(Err(_)) | Err(_) => return ClientOutcome::Refused,
    };
    // Parse the minimal response: a status line, then the body.
    let text = String::from_utf8_lossy(&raw);
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
    // The body starts after the header terminator (best-effort: the pinned
    // assertions only read JSON bodies of 200 responses).
    let body = match raw.windows(4).position(|window| window == b"\r\n\r\n") {
        Some(index) => raw[index + 4..].to_vec(),
        None => Vec::new(),
    };
    ClientOutcome::Served { status, body }
}

// --- FIM-F01.04 (a): a request without a client certificate is refused ----

#[tokio::test]
async fn a_request_without_a_client_certificate_is_refused() {
    let Some(pki) = pki_dir() else { return };
    let port = ephemeral_port();
    let (server, _plan) = server_with_registered_control();
    let (server_thread, shutdown) = spawn_live_server(&pki, port, server);
    wait_for_listener(port).await;

    // The anonymous client (no client certificate) gets no HTTP response
    // at all: the mTLS handshake refuses it fail-closed.
    let outcome = client_get(&pki, port, "/derc", None).await;
    match outcome {
        ClientOutcome::Refused => {}
        ClientOutcome::Served { status, .. } => {
            panic!(
                "an anonymous client was SERVED a response (status {status}) — the \
                 fail-closed mTLS posture is broken"
            );
        }
    }

    shutdown.notify_one();
    server_thread.join().expect("server thread stops cleanly");
}

// --- FIM-F01.04 (b): a certificate not in the allowlist is refused ---------

#[tokio::test]
async fn b_client_certificate_not_in_the_allowlist_is_refused() {
    let Some(pki) = pki_dir() else { return };
    let port = ephemeral_port();
    let (server, _plan) = server_with_registered_control();
    let (server_thread, shutdown) = spawn_live_server(&pki, port, server);
    wait_for_listener(port).await;

    // The client presents a VALID certificate chain signed by the same CA
    // (so the rustls verifier accepts it) whose fingerprint is not in the
    // server's allowlist: the post-handshake gate refuses it fail-closed.
    let outcome = client_get(
        &pki,
        port,
        "/derc",
        Some(("client_not_allowed.pem", "client_not_allowed.key.pem")),
    )
    .await;
    match outcome {
        ClientOutcome::Refused => {}
        ClientOutcome::Served { status, .. } => {
            panic!(
                "an un-listed client was SERVED a response (status {status}) — the \
                 fingerprint allowlist is broken"
            );
        }
    }

    shutdown.notify_one();
    server_thread.join().expect("server thread stops cleanly");
}

// --- FIM-F01.03 + FIM-F01.04 (c): the DERControl flow over the live
//     mTLS server ----------------------------------------------------------

#[tokio::test]
async fn c_registered_der_control_flows_over_the_live_mtls_server_to_the_proposed_mapping() {
    let Some(pki) = pki_dir() else { return };
    let port = ephemeral_port();
    let (server, plan) = server_with_registered_control();
    let control_path = plan.der_control_list.as_str().to_string();
    let (server_thread, shutdown) = spawn_live_server(&pki, port, server);
    wait_for_listener(port).await;

    // The ALLOWED client polls the declared control list over the live
    // mTLS connection and is served what the deterministic store holds.
    let outcome = client_get(
        &pki,
        port,
        &control_path,
        Some(("client.pem", "client.key.pem")),
    )
    .await;
    let body = match outcome {
        ClientOutcome::Served { status: 200, body } => body,
        ClientOutcome::Served { status, .. } => {
            panic!("the allowed client got status {status}; expected 200");
        }
        ClientOutcome::Refused => {
            panic!(
                "the ALLOWED client was refused — the allowlist gate wrongly denies \
                 a listed certificate"
            );
        }
    };
    let list: DERControlList = serde_json::from_slice(&body).expect("the served list decodes");
    assert_eq!(list.der_controls.len(), 1);
    assert_eq!(list.der_controls[0].mrid, "ctrl-live");

    // The served control feeds the UNCHANGED DERControl → PROPOSED
    // mapping: external control objects enter the command lifecycle at
    // PROPOSED and nowhere else (the frozen-vocabulary rule, pinned at
    // the live-transport level).
    let scope = flexcore_protocol_20305::scope::aggregator_scope(ADAPTER_ID).expect("valid scope");
    let proposal = ExternalControlProposal::from_der_control(
        &scope,
        ExternalEventSource::DerControl {
            program_mrid: "prog-1".to_string(),
            control_mrid: list.der_controls[0].mrid.clone(),
            sfdi: Sfdi::new(1_234_567_891).expect("valid sfdi"),
        },
        list.der_controls[0].start,
        None,
        "DERControl served by the live FIM-F01 mTLS transport".to_string(),
    )
    .expect("valid proposal");
    assert_eq!(
        proposal.lifecycle_entry_state,
        flexcore_domain::command_state::CommandState::Proposed
    );
    // The only adapter-recordable transition from the entry state is the
    // PROPOSED-gate rejection; ISSUED is refused by the frozen table.
    assert!(matches!(
        proposal.adapter_may_transition_to(flexcore_domain::command_state::CommandState::Issued),
        Err(flexcore_protocol_20305::proposal::ProposalError::FrozenTransitionRefused(_))
    ));

    shutdown.notify_one();
    server_thread.join().expect("server thread stops cleanly");
}
