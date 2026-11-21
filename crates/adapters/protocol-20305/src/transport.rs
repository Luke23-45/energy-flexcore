// FIM-F01.01 / FIM-F01.02 / FIM-F01.03 / FIM-F01.05 — the LIVE IEEE 2030.5
// server transport (docs/releases/final-implementation-before-publication.md
// §5 F-01; ledger docs/releases/implementation_ledger/ledger_F01.md).
//
// **Implemented under FIM-F01, WRITTEN-UNRUN: this module has never been
// compiled or executed. Interop with a real IEEE 2030.5 / CSIP client is
// UNPROVEN and remains a HUMAN LAB GATE** (crate conventions §7.3; the
// `tests/live_transport_gated.rs` suite is written-unrun evidence
// preparation that skips cleanly without its gate environment — nothing in
// this module is a conformance result).
//
// What the module implements, against the seam designed in `server.rs`:
//
// - **FIM-F01.01 — the mTLS listener.** [`LiveTransportConfig`] is the
//   fail-closed configuration struct the composition root fills (bind
//   address, server certificate-chain path, private-key path, trust-anchor
//   PEM paths, allowed client-certificate fingerprints). There are NO
//   hardcoded paths and NO secrets in code: every PEM file is loaded at
//   [`HyperTransport::new`] construction time and the construction fails
//   closed on missing/malformed material. Client certificates are
//   STRUCTURALLY mandatory — `rustls` is built with a required
//   `WebPkiClientVerifier` (no anonymous-accept variant exists in this
//   module), and AFTER a successful handshake the peer's leaf certificate
//   must be in the SHA-256 fingerprint allowlist
//   ([`authorize_client_certificate`]); absent, empty, or unlisted peer
//   certificates close the connection with a named
//   [`TransportError::PeerRejected`]. An EMPTY allowlist denies every
//   client (deny-all is the fail-closed direction), and the
//   `check_revocation` flag is honoured the only fail-closed way a
//   transport without a revocation facility can: constructing with the
//   flag set `true` is a named refusal — the transport never silently
//   ignores a declared control it cannot perform.
// - **FIM-F01.02 — HTTP/1.1 serving.** [`HyperTransport`] implements the
//   EXISTING `server::AggregatorTransport` seam (the trait is unchanged;
//   the simulator's in-process `SimulatedAggregatorTransport` continues to
//   implement it too). It wraps accepted connections in `tokio-rustls` and
//   serves them with `hyper::server::conn::http1` through
//   [`AggregatorHttpService`], which maps wire requests onto the design
//   surface (`server::ResourceRequest`) and serves them through the
//   existing `ResourceRequestHandler` routing (the deterministic
//   BTreeMap store, the bounded lists, the declared address plan — all
//   UNCHANGED server-skeleton code). Mapping decisions:
//   `GET`/`POST` map onto [`RequestMethod`]; any other verb is a 405; the
//   wire path becomes a validated [`ResourceHref`]; the query parameters
//   `sfdi`/`limit`/`offset` (the declared design-surface names) are parsed
//   into [`ResourceQuery`]; a POST body must decode as JSON or the request
//   is a 400. Responses carry the design-surface status code and the JSON
//   design-surface encoding with `Content-Type: application/json`.
// - **FIM-F01.03 — the DERControl flow.** A DERControl registered into the
//   deterministic store (the composition root's `install()` — the server
//   skeleton's POST surface deliberately stays responses/log-events only,
//   exactly as decision record 02 §5 pins it) is served over the live
//   path by a `GET` of the declared control-list href; the served control
//   feeds the UNCHANGED `proposal::ExternalControlProposal` mapping and
//   enters the command lifecycle at PROPOSED and nowhere else. The pinned
//   test drives this end-to-end through the HTTP service without a
//   socket, and the gated live-socket suite repeats it against a real
//   mTLS connection.
//
// UNVERIFIED-ASSUMPTION register (module-level; the model.rs discipline
// applies — a mismatch found against the IEEE 2030.5-2018 text / CSIP
// Implementation Guide is a code or record edit, never a silent
// reinterpretation):
//
// - **Wire encoding.** The HTTP bodies here are the JSON design surface of
//   `model.rs`. The standard's own wire encoding (EXI/XML — record 02 §4)
//   is UNVERIFIED and is NOT implemented; a real CSIP client speaking EXI
//   will not interoperate until the encoding question is verified at the
//   human lab gate. JSON-over-HTTPS is the declared design surface, not a
//   protocol claim.
// - **Query parameter names** (`sfdi`, `limit`, `offset`) and the absence
//   of percent-decoding are the design surface's declared spellings —
//   UNVERIFIED against the standard's query mechanics (addressing.rs
//   header). Unknown parameter names are IGNORED (the standard's
//   parameter set is UNVERIFIED; refusing unknown names would break a
//   real client on a spelling the lab gate then confirms).
// - **The fingerprint↔LFDI relation.** The allowlist hashes the DER bytes
//   of the client leaf certificate with SHA-256 (full 64-hex digest). The
//   2030.5 LFDI derivation (hash input, truncation) is UNVERIFIED and is
//   the security record 03 §3 implementation-time check; the allowlist is
//   deliberately expressed in raw SHA-256 fingerprints so a verified
//   LFDI-mismatch is a configuration-level change, not a code change.
// - **Shutdown semantics.** The [`HyperTransport::shutdown_handle`] stops
//   the listener and abandons in-flight connections immediately; a
//   graceful drain is not part of this written-unrun transport and is not
//   claimed.
// - **Revocation.** No OCSP/CRL facility exists in this transport (the
//   declared `check_revocation=true` posture of `server::MtlsRequirements`
//   cannot be performed here — see the construction refusal above). The
//   gap is a recorded HONEST LIMITATION, resolved only by an explicit
//   revocation facility, never by weakening the check silently.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::convert::Infallible;
use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;

use bytes::Bytes;
use futures::stream::{FuturesUnordered, StreamExt};
use http::header::CONTENT_TYPE;
use http::request::Request as HttpRequest;
use http::response::Response as HttpResponse;
use http::{Method, StatusCode};
use http_body::Body as HttpBody;
use http_body_util::{BodyExt, Full};
use hyper::server::conn::http1;
use hyper::service::Service;
// Batch 68: the hyper 1.x bridge from tokio streams to hyper's
// `hyper::rt::Read + Write` (see the `TokioIo` note in
// `protocol-openadr/src/transport.rs`).
use hyper_util::rt::TokioIo;
use rustls::RootCertStore;
use rustls::pki_types::CertificateDer;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Notify;
use tokio_rustls::TlsAcceptor;

use crate::addressing::{ResourceHref, ResourceQuery};
use crate::model::Sfdi;
use crate::server::{
    AggregatorTransport, RequestMethod, ResourceRequest, ResourceRequestHandler, ResourceResponse,
    ResponseStatus, TransportError,
};

// --- FIM-F01.01: the fail-closed live-transport configuration -----------------

/// The configuration the composition root fills for the live mTLS/HTTP1
/// AggregatorServer transport (FIM-F01.01). NO path or key material is
/// hardcoded anywhere in this crate: every field is a deployment
/// declaration, validated fail-closed at [`HyperTransport::new`].
///
/// The struct is serde-derivable so a composition root can load it from
/// its deployment configuration; it is a transport-layer record and does
/// not touch the frozen platform vocabularies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct LiveTransportConfig {
    /// The address the listener binds (declared by the deployment; no
    /// default port is assumed anywhere in this crate).
    pub bind_address: SocketAddr,
    /// Path to the server certificate-chain PEM (leaf first, then
    /// intermediates). Loaded at construction; never embedded in code.
    pub certificate_chain_path: String,
    /// Path to the server private-key PEM (PKCS#8, SEC1, or RSA —
    /// whatever `rustls_pemfile` decodes). Loaded at construction; never
    /// embedded in code.
    pub private_key_path: String,
    /// Paths to the trust-anchor PEM certificates the server validates
    /// DEVICE certificates against. Non-empty is enforced (an aggregator
    /// with no trust anchor accepts nothing — and MUST not silently accept
    /// everything instead; the `server::MtlsRequirements` rule).
    pub trust_anchor_paths: Vec<String>,
    /// Lowercase hex SHA-256 digests (over the DER bytes) of the client
    /// leaf certificates allowed to connect. An EMPTY set denies every
    /// client (deny-all is the fail-closed direction; it makes the server
    /// unreachable until the composition root declares at least one
    /// fingerprint). Uppercase hex input is normalized to lowercase at
    /// construction (the fingerprint's hex case carries no meaning; the
    /// normalization is deterministic, not a loosening).
    pub allowed_client_fingerprints: Vec<String>,
    /// Whether certificate revocation is checked before a session is
    /// accepted (the `server::MtlsRequirements` registered-recommendation
    /// default is `true`). THIS transport has no revocation facility, and
    /// an unverifiable revocation status is a rejection, not a pass — so
    /// constructing with `true` is a NAMED refusal
    /// ([`TransportError::ConfigInvalid`]), never a silent weakening of
    /// the declared check. A deployment sets `false` knowingly and records
    /// the gap, or gates the transport until a revocation facility exists.
    pub check_revocation: bool,
}

impl LiveTransportConfig {
    /// Validates the configuration fail-closed. Every refusal is named;
    /// nothing is repaired or defaulted.
    pub fn validate(&self) -> Result<(), TransportError> {
        if self.certificate_chain_path.trim().is_empty() {
            return Err(TransportError::ConfigInvalid {
                field: "certificate_chain_path",
                reason: "the server certificate-chain path is empty".to_string(),
            });
        }
        if self.private_key_path.trim().is_empty() {
            return Err(TransportError::ConfigInvalid {
                field: "private_key_path",
                reason: "the server private-key path is empty".to_string(),
            });
        }
        if self.trust_anchor_paths.is_empty()
            || self
                .trust_anchor_paths
                .iter()
                .any(|path| path.trim().is_empty())
        {
            return Err(TransportError::ConfigInvalid {
                field: "trust_anchor_paths",
                reason: "the trust-anchor path set is empty or contains an empty path \
                         (fail-closed: no trust anchor accepts nothing, and MUST not \
                         accept everything instead)"
                    .to_string(),
            });
        }
        for fingerprint in &self.allowed_client_fingerprints {
            let normalized = fingerprint.to_lowercase();
            if !is_sha256_hex(&normalized) {
                return Err(TransportError::ConfigInvalid {
                    field: "allowed_client_fingerprints",
                    reason: format!(
                        "fingerprint '{fingerprint}' is not 64 hex characters"
                    ),
                });
            }
        }
        if self.check_revocation {
            return Err(TransportError::ConfigInvalid {
                field: "check_revocation",
                reason: "revocation checking is DECLARED but has NO implementation in \
                         this written-unrun transport (FIM-F01 honest limitation): an \
                         unverifiable revocation status is a rejection, not a pass, so \
                         the transport refuses to construct with the flag set rather \
                         than silently ignoring it. Set it false knowingly and record \
                         the gap, or gate the transport until a revocation facility \
                         exists."
                    .to_string(),
            });
        }
        Ok(())
    }

    /// The normalized (lowercase) fingerprint allowlist as an ordered set
    /// (the deterministic-kernel pattern: same inputs, same set).
    pub fn normalized_allowlist(&self) -> BTreeSet<String> {
        self.allowed_client_fingerprints
            .iter()
            .map(|fingerprint| fingerprint.to_lowercase())
            .collect()
    }
}

/// Whether the string is exactly 64 hex characters (a SHA-256 digest's
/// lowercase-hex shape).
fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64 && value.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
}

// --- FIM-F01.01: certificate fingerprints and the allowlist gate --------------

/// The lowercase-hex SHA-256 fingerprint of a certificate's DER bytes (the
/// allowlist key; see the module header for the UNVERIFIED LFDI relation).
pub fn certificate_fingerprint_hex(certificate: &CertificateDer<'_>) -> String {
    hex_lower(Sha256::digest(certificate.as_ref()).as_slice())
}

/// Lowercase-hex encodes the bytes (a private local helper — no hex
/// dependency is added for two lines of encoding).
fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

/// The fail-closed post-handshake client-certificate gate (FIM-F01.01):
/// the peer MUST present a certificate (a rustls `ServerConfig` with a
/// required verifier already refuses anonymous clients at the protocol
/// layer — this gate re-asserts the same rule at the application layer so
/// the allowlist decision is pinned and testable without a socket), and
/// the leaf certificate's fingerprint MUST be in the allowlist. An absent
/// certificate, an empty list, or an unlisted fingerprint is a named
/// [`TransportError::PeerRejected`]. Returns the accepted fingerprint on
/// success.
pub fn authorize_client_certificate(
    peer_certificates: Option<&[CertificateDer<'_>]>,
    allowlist: &BTreeSet<String>,
) -> Result<String, TransportError> {
    let Some(certificates) = peer_certificates else {
        return Err(TransportError::PeerRejected {
            reason: "no client certificate was presented (fail-closed mTLS posture, \
                     FIM-F01.01)"
                .to_string(),
        });
    };
    let Some(leaf) = certificates.first() else {
        return Err(TransportError::PeerRejected {
            reason: "the client certificate list is empty (fail-closed)".to_string(),
        });
    };
    let fingerprint = certificate_fingerprint_hex(leaf);
    if !allowlist.contains(&fingerprint) {
        return Err(TransportError::PeerRejected {
            reason: format!(
                "client certificate fingerprint '{fingerprint}' is not in the \
                 allowlist (fail-closed; an empty allowlist denies every client)"
            ),
        });
    }
    Ok(fingerprint)
}

// --- FIM-F01.01: TLS material loading and ServerConfig construction -----------

/// Loads the server certificate chain (PEM; leaf first).
fn load_cert_chain(path: &str) -> Result<Vec<CertificateDer<'static>>, TransportError> {
    let file = std::fs::File::open(path).map_err(|error| TransportError::MaterialLoadFailed {
        what: "server certificate chain",
        reason: format!("{path}: {error}"),
    })?;
    let mut reader = std::io::BufReader::new(file);
    let chain: Vec<CertificateDer<'static>> = rustls_pemfile::certs(&mut reader)
        .collect::<Result<Vec<_>, std::io::Error>>()
        .map_err(|error| TransportError::MaterialLoadFailed {
            what: "server certificate chain",
            reason: format!("{path}: PEM decoding failed: {error}"),
        })?;
    if chain.is_empty() {
        return Err(TransportError::MaterialLoadFailed {
            what: "server certificate chain",
            reason: format!("{path}: no PEM certificates found"),
        });
    }
    Ok(chain)
}

/// Loads the server private key (PEM).
fn load_private_key(path: &str) -> Result<rustls::pki_types::PrivateKeyDer<'static>, TransportError> {
    let file = std::fs::File::open(path).map_err(|error| TransportError::MaterialLoadFailed {
        what: "server private key",
        reason: format!("{path}: {error}"),
    })?;
    let mut reader = std::io::BufReader::new(file);
    let key = rustls_pemfile::private_key(&mut reader)
        .map_err(|error| TransportError::MaterialLoadFailed {
            what: "server private key",
            reason: format!("{path}: PEM decoding failed: {error}"),
        })?
        .ok_or(TransportError::MaterialLoadFailed {
            what: "server private key",
            reason: format!("{path}: no PEM private key found"),
        })?;
    Ok(key)
}

/// Loads the trust anchors into a `RootCertStore` (each path must carry at
/// least one PEM certificate; a failed anchor is a named refusal — the
/// store is never silently short-loaded).
fn load_trust_anchors(paths: &[String]) -> Result<RootCertStore, TransportError> {
    let mut roots = RootCertStore::empty();
    for path in paths {
        let file = std::fs::File::open(path).map_err(|error| {
            TransportError::MaterialLoadFailed {
                what: "trust anchor",
                reason: format!("{path}: {error}"),
            }
        })?;
        let mut reader = std::io::BufReader::new(file);
        let anchors: Vec<CertificateDer<'static>> = rustls_pemfile::certs(&mut reader)
            .collect::<Result<Vec<_>, std::io::Error>>()
            .map_err(|error| TransportError::MaterialLoadFailed {
                what: "trust anchor",
                reason: format!("{path}: PEM decoding failed: {error}"),
            })?;
        if anchors.is_empty() {
            return Err(TransportError::MaterialLoadFailed {
                what: "trust anchor",
                reason: format!("{path}: no PEM certificates found"),
            });
        }
        for anchor in anchors {
            roots.add(anchor).map_err(|error| {
                TransportError::MaterialLoadFailed {
                    what: "trust anchor",
                    reason: format!("{path}: {error}"),
                }
            })?;
        }
    }
    Ok(roots)
}

/// Builds the `rustls` server configuration: the client-certificate
/// verifier is REQUIRED (there is no anonymous-accept variant in this
/// module — the mTLS posture is structural, `server::MtlsRequirements`),
/// the trust anchors come from the declared PEM paths, and the provider is
/// the pinned ring default (the committed lock graph resolves rustls with
/// ring; the FIM dependency decision forbids adding aws-lc-rs).
fn build_tls_server_config(config: &LiveTransportConfig) -> Result<rustls::ServerConfig, TransportError> {
    let chain = load_cert_chain(&config.certificate_chain_path)?;
    let key = load_private_key(&config.private_key_path)?;
    let roots = load_trust_anchors(&config.trust_anchor_paths)?;
    let verifier = rustls::server::WebPkiClientVerifier::builder(Arc::new(roots))
        .build()
        .map_err(|error| TransportError::TlsConfigFailed {
            reason: format!("client-certificate verifier construction: {error}"),
        })?;
    rustls::ServerConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
        .with_safe_default_protocol_versions()
        .map_err(|error| TransportError::TlsConfigFailed {
            reason: format!("protocol version selection: {error}"),
        })?
        // Client certificates are MANDATORY: the required verifier is the
        // only posture this module builds (no field can disable it).
        .with_client_cert_verifier(verifier)
        .with_single_cert(chain, key)
        .map_err(|error| TransportError::MaterialLoadFailed {
            what: "server certificate chain and private key pairing",
            reason: error.to_string(),
        })
}

// --- FIM-F01.02: the wire ↔ design-surface HTTP mapping -----------------------

/// A malformed wire query (the transport's named refusal; the parameter
/// names themselves are UNVERIFIED — module header). Private: the wire
/// refusal is answered to the client as the design-surface
/// [`ResponseStatus::BadRequest`], so the error does not join the crate's
/// public vocabularies.
#[derive(Debug, Clone, PartialEq, Eq)]
enum WireQueryError {
    /// A query pair is not `name=value`.
    MalformedPair(String),
    /// A known parameter's value does not decode (named with the
    /// parameter's declared spelling).
    InvalidValue {
        parameter: &'static str,
        value: String,
    },
}

/// Parses the wire query string into the design-surface [`ResourceQuery`].
/// The declared parameter names are `sfdi`, `limit`, `offset`
/// (UNVERIFIED spellings — module header); unknown names are IGNORED (the
/// standard's parameter set is UNVERIFIED, and a real client must not be
/// broken on a spelling the lab gate has yet to confirm). No
/// percent-decoding is performed: every modelled value is numeric
/// (module header).
fn parse_query(
    raw: Option<&str>,
    href: &ResourceHref,
) -> Result<Option<ResourceQuery>, WireQueryError> {
    let Some(raw) = raw else {
        return Ok(None);
    };
    if raw.is_empty() {
        return Ok(None);
    }
    let mut sfdi: Option<Sfdi> = None;
    let mut limit: Option<u32> = None;
    let mut offset: Option<u32> = None;
    for pair in raw.split('&') {
        let (name, value) = pair
            .split_once('=')
            .ok_or_else(|| WireQueryError::MalformedPair(pair.to_string()))?;
        match name {
            "sfdi" => {
                let parsed: u64 = value
                    .parse()
                    .map_err(|_| WireQueryError::InvalidValue {
                        parameter: "sfdi",
                        value: value.to_string(),
                    })?;
                // Zero SFDI is refused fail-closed (model.rs rule); the
                // refusal is named, never defaulted.
                let sfdi_value =
                    Sfdi::new(parsed).map_err(|_| WireQueryError::InvalidValue {
                        parameter: "sfdi",
                        value: value.to_string(),
                    })?;
                sfdi = Some(sfdi_value);
            }
            "limit" => {
                limit = Some(value.parse().map_err(|_| {
                    WireQueryError::InvalidValue {
                        parameter: "limit",
                        value: value.to_string(),
                    }
                })?);
            }
            "offset" => {
                offset = Some(value.parse().map_err(|_| {
                    WireQueryError::InvalidValue {
                        parameter: "offset",
                        value: value.to_string(),
                    }
                })?);
            }
            // Unknown parameter names are ignored (module header).
            _ => {}
        }
    }
    Ok(Some(ResourceQuery {
        href: href.clone(),
        sfdi,
        limit,
        offset,
    }))
}

/// The shared handler cell: HTTP/1.1 serves each request sequentially per
/// connection, but MULTIPLE connections run concurrently in the one
/// current-thread serving loop — the shared `Rc<RefCell<..>>` multiplexes
/// the single `&mut dyn ResourceRequestHandler` borrow across them. The
/// design-surface `handle` is synchronous, so the borrow is taken and
/// dropped within one poll; it is never held across an await.
type SharedHandlerCell<'h> = Rc<RefCell<&'h mut dyn ResourceRequestHandler>>;

/// Maps one wire request onto the design surface, calls the EXISTING
/// `ResourceRequestHandler` routing (the deterministic store, bounded
/// lists, declared plan — all unchanged server-skeleton code), and maps
/// the design-surface response back to the wire. FIM-F01.02.
async fn serve_wire_request<B: HttpBody<Data = Bytes> + 'static>(
    request: HttpRequest<B>,
    handler: SharedHandlerCell<'_>,
) -> HttpResponse<Full<Bytes>>
where
    B::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    // 1. The verb: GET reads and bounded POST writes only (the server
    //    skeleton's declared surface); any other verb is a 405.
    let method = match *request.method() {
        Method::GET => RequestMethod::Get,
        Method::POST => RequestMethod::Post,
        _ => {
            return design_to_wire(ResourceResponse::status_only(
                ResponseStatus::NotAllowed,
            ));
        }
    };
    // 2. The path: a validated ResourceHref. A malformed path is a named
    //    400 (it never reaches the store; the skeleton's 404 discipline
    //    covers declared-but-missing and undeclared alike).
    let path = request.uri().path().to_string();
    let href = match ResourceHref::new(path) {
        Ok(href) => href,
        Err(_) => {
            return design_to_wire(ResourceResponse::status_only(
                ResponseStatus::BadRequest,
            ));
        }
    };
    // 3. The query parameters (declared spellings; UNVERIFIED — module
    //    header). A malformed known parameter is a named 400.
    let query = match parse_query(request.uri().query(), &href) {
        Ok(query) => query,
        Err(_) => {
            return design_to_wire(ResourceResponse::status_only(
                ResponseStatus::BadRequest,
            ));
        }
    };
    // 4. The body: POST bodies must decode as JSON (the design-surface
    //    encoding) or the request is a named 400; GET bodies are ignored.
    let body = if method == RequestMethod::Post {
        match request.into_body().collect().await {
            Ok(collected) => match serde_json::from_slice::<serde_json::Value>(
                collected.to_bytes().as_ref(),
            ) {
                Ok(value) => Some(value),
                Err(_) => {
                    return design_to_wire(ResourceResponse::status_only(
                        ResponseStatus::BadRequest,
                    ));
                }
            },
            Err(_) => {
                return design_to_wire(ResourceResponse::status_only(
                    ResponseStatus::BadRequest,
                ));
            }
        }
    } else {
        None
    };
    // 5. The design-surface call: the borrow is taken and released within
    //    this synchronous block (never held across an await).
    let design_response = {
        let mut handler = handler.borrow_mut();
        handler.handle(&ResourceRequest {
            method,
            href,
            query,
            body,
        })
    };
    // 6. The wire response.
    design_to_wire(design_response)
}

/// Maps a design-surface [`ResourceResponse`] onto the wire: the declared
/// status code, the JSON design-surface encoding, `Content-Type:
/// application/json`. An encoding failure degrades to a named 500 —
/// never to an Ok status with an empty body.
fn design_to_wire(response: ResourceResponse) -> HttpResponse<Full<Bytes>> {
    let body_bytes = match response.body {
        Some(value) => match serde_json::to_vec(&value) {
            Ok(bytes) => bytes,
            Err(_) => {
                return empty_wire_response(StatusCode::INTERNAL_SERVER_ERROR);
            }
        },
        None => Vec::new(),
    };
    let status = StatusCode::from_u16(response.status.code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    HttpResponse::builder()
        .status(status)
        .header(CONTENT_TYPE, "application/json")
        .body(Full::new(Bytes::from(body_bytes)))
        .unwrap_or_else(|_| empty_wire_response(StatusCode::INTERNAL_SERVER_ERROR))
}

/// An empty wire response with the given status (the encoding-failure
/// degradation path).
fn empty_wire_response(status: StatusCode) -> HttpResponse<Full<Bytes>> {
    HttpResponse::builder()
        .status(status)
        .body(Full::new(Bytes::new()))
        .unwrap_or_else(|_| HttpResponse::new(Full::new(Bytes::new())))
}

/// The hyper service that serves the CSIP resource tree over HTTP/1.1
/// (FIM-F01.02). The handler is the shared cell; each request's future
/// owns a clone of it. Generic over the request body so the socket-free
/// pinned tests can drive the service directly with `Full<Bytes>` bodies
/// while the live loop feeds it hyper's `Incoming`.
pub struct AggregatorHttpService<'h> {
    handler: SharedHandlerCell<'h>,
}

impl<'h> AggregatorHttpService<'h> {
    /// Wraps a shared handler cell.
    pub fn new(handler: SharedHandlerCell<'h>) -> Self {
        Self { handler }
    }
}

impl<'h, B> Service<HttpRequest<B>> for AggregatorHttpService<'h>
where
    B: HttpBody<Data = Bytes> + Send + 'static,
    B::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    type Response = HttpResponse<Full<Bytes>>;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + 'h>>;

    // Batch 68: this implements `hyper::service::Service` (hyper 1.x),
    // whose `call` takes `&self` and which has NO `poll_ready` — the
    // previous `poll_ready` + `call(&mut self)` shape was tower's
    // `Service`, not hyper's. The body only clones the shared handler,
    // so `&self` is sufficient.
    fn call(&self, request: HttpRequest<B>) -> Self::Future {
        let handler = Rc::clone(&self.handler);
        Box::pin(async move {
            let response = serve_wire_request(request, handler).await;
            Ok::<_, Infallible>(response)
        })
    }
}

// --- FIM-F01.01 + FIM-F01.02: the live mTLS/HTTP1 transport --------------------

/// The LIVE AggregatorServer transport (FIM-F01): a tokio TCP accept loop
/// wrapping every connection in `tokio-rustls` (client certificates
/// structurally mandatory + the fingerprint allowlist gate) and serving it
/// with hyper 1.x HTTP/1.1 through [`AggregatorHttpService`] onto the
/// EXISTING `server::AggregatorTransport` seam's handler routing.
///
/// **WRITTEN-UNRUN; interop with a real 2030.5 client is UNPROVEN and
/// remains a HUMAN LAB GATE** (module header).
pub struct HyperTransport {
    /// The validated configuration (held by value — the composition root
    /// declares it, this transport never mutates it).
    config: LiveTransportConfig,
    /// The normalized fingerprint allowlist.
    allowlist: BTreeSet<String>,
    /// The eagerly-built TLS configuration (construction is fail-closed:
    /// missing material refuses HERE, never at first accept).
    tls_config: Arc<rustls::ServerConfig>,
    /// The shutdown signal (the composition root's stop mechanism).
    shutdown: Arc<Notify>,
}

impl HyperTransport {
    /// Constructs the transport fail-closed: the configuration is
    /// validated and ALL TLS material is loaded and the `rustls` server
    /// configuration is built BEFORE anything binds. Every refusal is a
    /// named [`TransportError`].
    pub fn new(config: LiveTransportConfig) -> Result<Self, TransportError> {
        config.validate()?;
        let allowlist = config.normalized_allowlist();
        let tls_config = Arc::new(build_tls_server_config(&config)?);
        Ok(Self {
            config,
            allowlist,
            tls_config,
            shutdown: Arc::new(Notify::new()),
        })
    }

    /// The validated configuration.
    pub fn config(&self) -> &LiveTransportConfig {
        &self.config
    }

    /// The shutdown handle: the composition root clones this BEFORE
    /// calling `serve` (which consumes the transport box) and notifies it
    /// to stop the serving loop. A stored permit makes the signal
    /// race-free: a notify arriving before the loop's first await still
    /// stops the transport.
    pub fn shutdown_handle(&self) -> Arc<Notify> {
        Arc::clone(&self.shutdown)
    }

    /// The current-thread serving loop (FIM-F01.01 + FIM-F01.02): bind,
    /// accept, TLS-wrap, gate the peer certificate against the allowlist,
    /// serve HTTP/1.1 through the design-surface handler. Returns
    /// `Ok(())` on a shutdown signal; every other outcome is a named
    /// [`TransportError`].
    async fn serve_current_thread(
        &self,
        handler: &mut dyn ResourceRequestHandler,
    ) -> Result<(), TransportError> {
        let listener = TcpListener::bind(self.config.bind_address)
            .await
            .map_err(|error| TransportError::BindFailed {
                reason: format!("{}: {error}", self.config.bind_address),
            })?;
        let acceptor = TlsAcceptor::from(Arc::clone(&self.tls_config));
        let allowlist = self.allowlist.clone();
        let handler: SharedHandlerCell<'_> = Rc::new(RefCell::new(handler));
        let mut connections: FuturesUnordered<
            Pin<Box<dyn Future<Output = Result<(), TransportError>> + '_>>,
        > = FuturesUnordered::new();
        loop {
            tokio::select! {
                _ = self.shutdown.notified() => {
                    // The composition root's stop signal: the listener
                    // closes and in-flight connections are abandoned
                    // immediately (module header: no graceful drain is
                    // implemented or claimed).
                    return Ok(());
                }
                accepted = listener.accept() => {
                    match accepted {
                        Ok((stream, _peer)) => {
                            let acceptor = acceptor.clone();
                            let allowlist = allowlist.clone();
                            let handler = Rc::clone(&handler);
                            connections.push(Box::pin(Self::serve_one_connection(
                                acceptor, allowlist, handler, stream,
                            )));
                        }
                        Err(error) => {
                            return Err(TransportError::TransportClosed {
                                reason: format!("accept failed: {error}"),
                            });
                        }
                    }
                }
                finished = connections.next(), if !connections.is_empty() => {
                    // Per-connection failures (handshake refusals, peer
                    // rejections, malformed HTTP) are fail-closed
                    // CONNECTION outcomes and do not stop the server:
                    // every one is a named TransportError value that this
                    // loop drops (the transport has no logging facility
                    // in this batch — observability is the P14.21 seam).
                    let _ = finished;
                }
            }
        }
    }

    /// Serves ONE accepted TCP connection to completion: TLS handshake
    /// (fail-closed on any error), the post-handshake fingerprint gate
    /// (fail-closed), then hyper HTTP/1.1 over the TLS stream.
    async fn serve_one_connection(
        acceptor: TlsAcceptor,
        allowlist: BTreeSet<String>,
        handler: SharedHandlerCell<'_>,
        stream: TcpStream,
    ) -> Result<(), TransportError> {
        let tls_stream = acceptor
            .accept(stream)
            .await
            .map_err(|error| TransportError::TlsHandshakeFailed {
                reason: error.to_string(),
            })?;
        // The fail-closed allowlist gate: absent, empty, or unlisted peer
        // certificates close the connection BEFORE any HTTP byte is read.
        let (_io, server_connection) = tls_stream.get_ref();
        authorize_client_certificate(server_connection.peer_certificates(), &allowlist)?;
        let service = AggregatorHttpService::new(handler);
        // Batch 68: `TokioIo` bridges the tokio-rustls stream to hyper
        // 1.x's `hyper::rt::Read + Write` (the stream implements tokio's
        // `AsyncRead/AsyncWrite`, not hyper's traits).
        http1::Builder::new()
            .serve_connection(TokioIo::new(tls_stream), service)
            .await
            .map_err(|error| TransportError::TransportClosed {
                reason: format!("http1 connection error: {error}"),
            })?;
        Ok(())
    }
}

impl AggregatorTransport for HyperTransport {
    /// Blocks on a dedicated current-thread tokio runtime and serves until
    /// the shutdown handle fires (see [`HyperTransport::shutdown_handle`])
    /// or a named failure occurs. The seam trait is UNCHANGED — this is an
    /// ADDED live implementation alongside the simulator's in-process one.
    fn serve(
        self: Box<Self>,
        handler: &mut dyn ResourceRequestHandler,
    ) -> Result<(), TransportError> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| TransportError::BindFailed {
                reason: format!("tokio current-thread runtime: {error}"),
            })?;
        runtime.block_on(self.serve_current_thread(handler))
    }
}

// --- Pinned tests (FIM-F01.04) -------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::addressing::AddressPlan;
    use crate::model::{DERControl, DERControlBase, DERControlList, ListMeta, TimeType};
    use crate::server::{AggregatorServer, MtlsRequirements, ServerLimits, StoredResource};
    use flexcore_protocol_ports::manifest::ProtocolAdapterManifest;

    // The reviewed scope/manifest spelling lives in crate::scope (the
    // FRV1-R6-004 rule): the test helpers delegate there.

    fn manifest() -> ProtocolAdapterManifest {
        crate::scope::aggregator_manifest(crate::ADAPTER_ID).expect("valid manifest")
    }

    fn server() -> AggregatorServer {
        AggregatorServer::new(
            manifest(),
            AddressPlan::csip_default(),
            ServerLimits::new(8, 8).expect("valid limits"),
            MtlsRequirements::new(vec!["csip-device-ca".to_string()], true)
                .expect("valid tls requirements"),
        )
        .expect("valid server")
    }

    fn shared(handler: &mut AggregatorServer) -> SharedHandlerCell<'_> {
        Rc::new(RefCell::new(handler as &mut dyn ResourceRequestHandler))
    }

    fn wire_request(method: Method, path: &str, query: Option<&str>, body: Option<&str>) -> HttpRequest<Full<Bytes>> {
        let uri = match query {
            Some(query) => format!("{path}?{query}").parse::<http::Uri>().expect("valid uri"),
            None => path.parse::<http::Uri>().expect("valid uri"),
        };
        let body_bytes = match body {
            Some(body) => Bytes::from(body.to_string()),
            None => Bytes::new(),
        };
        HttpRequest::builder()
            .method(method)
            .uri(uri)
            .body(Full::new(body_bytes))
            .expect("valid request")
    }

    fn control_list_with(n: usize) -> DERControlList {
        let mut controls = Vec::new();
        for i in 0..n {
            controls.push(
                DERControl::new(
                    format!("ctrl-{i}"),
                    Some(TimeType(1)),
                    Some(TimeType(1000 + i as i64)),
                    Some(300),
                    None,
                    None,
                    DERControlBase {
                        op_mod_gen_lim_w: Some(1000.0 + i as f64),
                        ..DERControlBase::default()
                    },
                )
                .expect("valid control"),
            );
        }
        DERControlList {
            meta: ListMeta::unpaged(n as u32),
            poll_rate: Some(60),
            der_controls: controls,
        }
    }

    fn valid_config() -> LiveTransportConfig {
        LiveTransportConfig {
            bind_address: "127.0.0.1:0".parse().expect("valid socket addr"),
            certificate_chain_path: "/pki/server.pem".to_string(),
            private_key_path: "/pki/server.key.pem".to_string(),
            trust_anchor_paths: vec!["/pki/device-ca.pem".to_string()],
            allowed_client_fingerprints: vec!["0a".repeat(32)],
            check_revocation: false,
        }
    }

    // --- FIM-F01.04 (a)/(b) at the logic level: the fail-closed gates ------

    #[test]
    fn config_validation_is_fail_closed() {
        let mut config = valid_config();
        config.certificate_chain_path = "  ".to_string();
        assert!(matches!(
            config.validate(),
            Err(TransportError::ConfigInvalid { field: "certificate_chain_path", .. })
        ));
        let mut config = valid_config();
        config.private_key_path = String::new();
        assert!(matches!(
            config.validate(),
            Err(TransportError::ConfigInvalid { field: "private_key_path", .. })
        ));
        // No trust anchors at all.
        let mut config = valid_config();
        config.trust_anchor_paths = Vec::new();
        assert!(matches!(
            config.validate(),
            Err(TransportError::ConfigInvalid { field: "trust_anchor_paths", .. })
        ));
        // A malformed fingerprint.
        let mut config = valid_config();
        config.allowed_client_fingerprints = vec!["zz".to_string()];
        assert!(matches!(
            config.validate(),
            Err(TransportError::ConfigInvalid { field: "allowed_client_fingerprints", .. })
        ));
        // A declared-but-unperformable revocation check is a named refusal,
        // never a silent weakening.
        let mut config = valid_config();
        config.check_revocation = true;
        assert!(matches!(
            config.validate(),
            Err(TransportError::ConfigInvalid { field: "check_revocation", .. })
        ));
        // The valid configuration passes.
        assert_eq!(valid_config().validate(), Ok(()));
    }

    #[test]
    fn fingerprint_gate_refuses_absent_unlisted_and_empty_allowlists() {
        // Synthetic DER bytes (the fingerprint is a pure function of the
        // bytes; no real certificate is needed to pin the gate's logic).
        let known = CertificateDer::from(vec![0x30, 0x01, 0x00]);
        let known_fingerprint = certificate_fingerprint_hex(&known);
        // An allowlist containing exactly the known fingerprint.
        let allowlist: BTreeSet<String> = [known_fingerprint.clone()].into_iter().collect();

        // (a) No client certificate presented: refused.
        assert!(matches!(
            authorize_client_certificate(None, &allowlist),
            Err(TransportError::PeerRejected { .. })
        ));
        // An empty peer certificate list: refused.
        let empty: Vec<CertificateDer<'static>> = Vec::new();
        assert!(matches!(
            authorize_client_certificate(Some(empty.as_slice()), &allowlist),
            Err(TransportError::PeerRejected { .. })
        ));
        // (b) A certificate whose fingerprint is not in the allowlist:
        // refused.
        let unknown = CertificateDer::from(vec![0x30, 0x02, 0x01, 0x02]);
        assert!(matches!(
            authorize_client_certificate(Some(&[unknown][..]), &allowlist),
            Err(TransportError::PeerRejected { .. })
        ));
        // An EMPTY allowlist denies every client (deny-all is the
        // fail-closed direction).
        let empty: BTreeSet<String> = BTreeSet::new();
        assert!(matches!(
            authorize_client_certificate(Some(&[known][..]), &empty),
            Err(TransportError::PeerRejected { .. })
        ));
        // The listed certificate is accepted, and the accepted fingerprint
        // is the allowlist entry (lowercase hex of the SHA-256 over DER).
        assert_eq!(
            authorize_client_certificate(Some(&[known][..]), &allowlist),
            Ok(known_fingerprint.clone())
        );
        // The fingerprint is deterministic and lowercase.
        assert_eq!(certificate_fingerprint_hex(&known), known_fingerprint);
        assert!(known_fingerprint.chars().all(|c| c.is_ascii_digit()
            || ('a'..='f').contains(&c)));
    }

    // --- FIM-F01.02: the wire mapping --------------------------------------

    #[tokio::test]
    async fn wire_mapping_refuses_unmodelled_verbs_and_malformed_input() {
        let mut server = server();
        let handler = shared(&mut server);
        let mut service = AggregatorHttpService::new(handler);

        // PUT (any verb beyond GET/POST) is a 405.
        let response = service
            .call(wire_request(Method::PUT, "/derc", None, None))
            .await
            .expect("infallible");
        assert_eq!(response.status(), StatusCode::NOT_ALLOWED);

        // A POST with an undecodable body is a 400.
        let response = service
            .call(wire_request(
                Method::POST,
                "/rsps",
                None,
                Some("{not-json"),
            ))
            .await
            .expect("infallible");
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        // A malformed known query parameter is a 400.
        let response = service
            .call(wire_request(Method::GET, "/derc", Some("limit=abc"), None))
            .await
            .expect("infallible");
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        // A zero limit passes the transport (the parameter itself parses)
        // and is refused by the skeleton's query validation: 400.
        let response = service
            .call(wire_request(Method::GET, "/derc", Some("limit=0"), None))
            .await
            .expect("infallible");
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        // A zero SFDI is refused at the transport (model.rs rule).
        let response = service
            .call(wire_request(Method::GET, "/derc", Some("sfdi=0"), None))
            .await
            .expect("infallible");
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        // A malformed query pair (no '=') is a 400.
        let response = service
            .call(wire_request(Method::GET, "/derc", Some("limit"), None))
            .await
            .expect("infallible");
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    // --- FIM-F01.04 (d): a resource-tree GET returns what the
    //     deterministic store holds ----------------------------------------

    #[tokio::test]
    async fn resource_tree_get_returns_what_the_store_holds() {
        let mut server = server();
        let plan = AddressPlan::csip_default();
        server
            .install(
                plan.der_control_list.clone(),
                StoredResource::DERControlList(control_list_with(3)),
            )
            .expect("install");
        let handler = shared(&mut server);
        let mut service = AggregatorHttpService::new(handler);

        let response = service
            .call(wire_request(
                Method::GET,
                plan.der_control_list.as_str(),
                None,
                None,
            ))
            .await
            .expect("infallible");
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.expect("body").to_bytes();
        let json: serde_json::Value = serde_json::from_slice(body.as_ref()).expect("decodes");
        assert_eq!(json["meta"]["all"], 3);
        assert_eq!(json["meta"]["results"], 3);
        let controls = json["derControls"].as_array().expect("array");
        assert_eq!(controls.len(), 3);
        assert_eq!(controls[0]["mrid"], "ctrl-0");
        assert_eq!(controls[2]["mrid"], "ctrl-2");

        // Paged: the transport parses the declared query parameters and the
        // skeleton slices deterministically.
        let response = service
            .call(wire_request(
                Method::GET,
                plan.der_control_list.as_str(),
                Some("limit=2&offset=1"),
                None,
            ))
            .await
            .expect("infallible");
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.expect("body").to_bytes();
        let json: serde_json::Value = serde_json::from_slice(body.as_ref()).expect("decodes");
        assert_eq!(json["meta"]["all"], 3);
        assert_eq!(json["meta"]["results"], 2);
        assert_eq!(json["derControls"][0]["mrid"], "ctrl-1");

        // Undeclared and uninstalled paths are 404, indistinguishable.
        for path in ["/secret", plan.pricing_list.as_str()] {
            let response = service
                .call(wire_request(Method::GET, path, None, None))
                .await
                .expect("infallible");
            assert_eq!(response.status(), StatusCode::NOT_FOUND, "{path}");
        }
    }

    // --- FIM-F01.02: the bounded POST surface over the wire -----------------

    #[tokio::test]
    async fn bounded_post_writes_flow_over_the_wire() {
        let mut server = server();
        let plan = AddressPlan::csip_default();
        let handler = shared(&mut server);
        let mut service = AggregatorHttpService::new(handler);

        // A device Response POSTed to the declared response list is
        // recorded (201) and served back.
        let body = serde_json::json!({
            "endDeviceLfdi": "0a1b2c3d4e5f",
            "createdDateTime": 100,
            "status": 1,
            "subject": "/derc/ctrl-0"
        });
        let response = service
            .call(wire_request(
                Method::POST,
                plan.response_list.as_str(),
                None,
                Some(body.to_string().as_str()),
            ))
            .await
            .expect("infallible");
        assert_eq!(response.status(), StatusCode::CREATED);

        // A POST to the control list is refused by the skeleton (405) —
        // the aggregator never writes controls into its own tree; devices
        // poll.
        let response = service
            .call(wire_request(
                Method::POST,
                plan.der_control_list.as_str(),
                None,
                Some(r#"{"mrid":"injected"}"#),
            ))
            .await
            .expect("infallible");
        assert_eq!(response.status(), StatusCode::NOT_ALLOWED);
    }

    // --- FIM-F01.03 + FIM-F01.04 (c): the DERControl → PROPOSED flow -------

    #[tokio::test]
    async fn registered_der_control_flows_over_the_wire_to_the_proposed_mapping() {
        let mut server = server();
        let plan = AddressPlan::csip_default();
        // The composition root REGISTERS a DERControl into the store (the
        // skeleton's POST surface stays responses/log-events only — the
        // record 02 §5 posture is unchanged).
        server
            .install(
                plan.der_control_list.clone(),
                StoredResource::DERControlList(control_list_with(2)),
            )
            .expect("install");
        let handler = shared(&mut server);
        let mut service = AggregatorHttpService::new(handler);

        // The live path: a GET of the declared control list.
        let response = service
            .call(wire_request(
                Method::GET,
                plan.der_control_list.as_str(),
                None,
                None,
            ))
            .await
            .expect("infallible");
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.expect("body").to_bytes();
        let list: DERControlList = serde_json::from_slice(body.as_ref()).expect("decodes");
        assert_eq!(list.der_controls.len(), 2);

        // The UNCHANGED DERControl → PROPOSED mapping consumes the served
        // control exactly as the existing mapping pins it: entry at
        // PROPOSED, constructed by the frozen vocabulary itself.
        let control = &list.der_controls[0];
        let scope = crate::scope::aggregator_scope(crate::ADAPTER_ID).expect("valid scope");
        let proposal = crate::proposal::ExternalControlProposal::from_der_control(
            &scope,
            crate::proposal::ExternalEventSource::DerControl {
                program_mrid: "prog-1".to_string(),
                control_mrid: control.mrid.clone(),
                sfdi: Sfdi::new(1_234_567_891).expect("valid sfdi"),
            },
            control.start,
            None,
            "DERControl served by the live FIM-F01 transport".to_string(),
        )
        .expect("valid proposal");
        assert_eq!(
            proposal.lifecycle_entry_state,
            flexcore_domain::command_state::CommandState::Proposed
        );
        // The only legal adapter-recorded transition from the entry state
        // is the PROPOSED-gate rejection (frozen transition 2); every
        // forward state is refused by the frozen table.
        assert_eq!(proposal.reject_at_gate(),
            Ok(flexcore_domain::command_state::CommandState::Rejected));
        assert!(matches!(
            proposal.adapter_may_transition_to(
                flexcore_domain::command_state::CommandState::Issued
            ),
            Err(crate::proposal::ProposalError::FrozenTransitionRefused(_))
        ));
    }
}
