// FIM-F04.01 / FIM-F04.04 — the LIVE OpenADR 3.0 VTN HTTP transport
// (docs/releases/final-implementation-before-publication.md §5 F-04; ledger
// docs/releases/implementation_ledger/ledger_F04.md).
//
// **Implemented under FIM-F04, WRITTEN-UNRUN: this module has never been
// compiled or executed. VEN interoperability is UNPROVEN and remains a
// HUMAN LAB GATE** (crate conventions §7.3; the gated socket-level suite
// `tests/live_transport_gated.rs` is written-unrun evidence preparation
// that skips cleanly without its gate environment — nothing here is a
// conformance result).
//
// What the module implements, against the seam DESIGNED AS DATA in
// `crate::endpoints` (the `RouteSpec` records are UNCHANGED — the route
// data stays the single place this serving layer reads):
//
// - **FIM-F04.01 — the hyper 1.x HTTP/1.1 serving layer.**
//   [`VtnHttpService`] is a `hyper::service::Service` (the house pattern of
//   `crates/adapters/protocol-20305/src/transport.rs`): it matches each
//   wire request against the designed routes (method + path pattern with
//   `{ven_id}`-style path parameters — the design's own token shape),
//   decodes the request body into the designed payload kind, calls the
//   serving state (`crate::serving::VtnServingState` — the EXISTING
//   registry/planner/model code, unchanged), and maps the outcome back to
//   the wire as JSON with `Content-Type: application/json`. A designed
//   path hit with the wrong verb is a named 405; a path no designed route
//   serves is a named 404; a body that does not decode into the designed
//   payload kind is a named 400. Every error body carries the RECORDED
//   error text verbatim (FIM-F04.02: all `VtnRegistryError`,
//   `DeliveryPlanError`, and model-error paths reach the client with
//   their existing `Display` text byte-identical in the JSON error
//   record).
// - **FIM-F04.01 — the live listener.** [`LiveVtnTransport`] is the tokio
//   accept loop wrapping every connection and serving it with
//   `hyper::server::conn::http1` through [`VtnHttpService`]. TLS is
//   OPTIONAL-BY-CONFIG and the construction is FAIL-CLOSED
//   ([`WireSecurityConfig`]): the mTLS posture builds the `rustls` server
//   configuration EAGERLY with a REQUIRED client-certificate verifier (the
//   protocol-20305 house pattern — missing or malformed material refuses
//   construction, never first accept), and the plaintext posture is
//   available ONLY with a recorded non-empty lab-posture reason naming the
//   deployment's compensating controls (the protocol-ocpp
//   recorded-posture-reason precedent — an unexplained plaintext listener
//   is a construction error). There are NO hardcoded paths and NO secret
//   material in code.
// - **FIM-F04.04 — the pinned socket-free tests.** The in-module tests
//   drive [`VtnHttpService`] DIRECTLY with in-memory bodies (no socket):
//   registration round-trip, poll delivering a queued event per the plan,
//   report upload, delivery-version recording, and the byte-identical
//   error paths (unknown VEN, wrong mode, malformed body). The gated
//   socket-level suite (`tests/live_transport_gated.rs`) repeats the
//   round-trip against a real connection and skips cleanly without its
//   gate environment.
//
// UNVERIFIED-ASSUMPTION register (module-level; the record 21 §5
// discipline applies — a mismatch found against the OpenADR 3.0
// specification text [S-10] is a code or record edit, never a silent
// reinterpretation):
//
// - **The wire payloads are the crate's JSON design shapes** (`model.rs`).
//   The 3.0 specification's own REST/JSON payload spellings, endpoint
//   paths, and path-parameter syntax are UNVERIFIED (record 21 §5); a real
//   3.0 VEN will not interoperate until that human lab gate confirms the
//   shapes. JSON-over-HTTP is the declared design surface, not a protocol
//   claim.
// - **No percent-decoding** of path segments or parameters (the 20305
//   transport precedent; the parameter set is the design's own).
// - **No push sender.** The VTN → VEN outbound push call is NOT
//   implemented (no HTTP client in this written-unrun program); the push
//   route queues the event for the VEN's agreed surface and no delivery
//   record is claimed (`crate::serving` module header).
// - **No idle timeouts, no graceful drain, no metrics.** The shutdown
//   signal stops the listener and abandons in-flight connections
//   immediately; observability is the P14.21 seam.
// - **mTLS client-certificate posture.** The required client verifier
//   follows the protocol-20305 house pattern; whether OpenADR 3.0 mandates
//   mutual TLS for the REST/JSON profile is UNVERIFIED ([S-10]) — the
//   deployment that cannot present client certificates uses the recorded
//   plaintext posture instead, never a silently weakened TLS mode.

use std::cell::RefCell;
use std::collections::BTreeMap;
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
// `hyper::rt::Read + Write` — tokio's `TcpStream` and tokio-rustls'
// `TlsStream` implement tokio's `AsyncRead/AsyncWrite`, not hyper's
// traits, so `serve_connection` needs the `TokioIo` wrapper.
use hyper_util::rt::TokioIo;
use serde::{Deserialize, Serialize};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Notify;
use tokio_rustls::TlsAcceptor;

use crate::endpoints::{HttpMethod, PayloadKind, RouteSpec, VtnEndpointDesign};
use crate::model::{OpenAdrEvent, ReportSpec, Ven};
use crate::scope::DeliveryMode;
use crate::serving::{ServingError, VenRegistrationOutcome, VtnServingState};
use crate::vtn::{DeliveryPlanError, VtnRegistryError};

// --- FIM-F04.01: the fail-closed live-transport configuration -----------------

/// The wire-security posture of the live VTN transport (FIM-F04.01;
/// fail-closed construction — there is no default and no silently weakened
/// mode). TLS is optional-BY-CONFIG: an mTLS deployment declares its
/// material and a plaintext deployment RECORDS its lab-posture reason.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WireSecurityConfig {
    /// mTLS: the server identity is loaded from the declared PEM paths and
    /// the `rustls` server configuration is built with a REQUIRED
    /// client-certificate verifier (the protocol-20305 house pattern; no
    /// anonymous-accept variant exists). Every path is loaded at
    /// construction; construction fails closed on missing or malformed
    /// material.
    Mtls {
        /// Path to the server certificate-chain PEM (leaf first).
        certificate_chain_path: String,
        /// Path to the server private-key PEM.
        private_key_path: String,
        /// Paths to the trust-anchor PEM certificates the server validates
        /// VEN client certificates against (non-empty is enforced — a VTN
        /// with no trust anchor accepts nothing, and MUST not silently
        /// accept everything instead).
        trust_anchor_paths: Vec<String>,
    },
    /// Plaintext HTTP/1.1, admitted ONLY with a recorded non-empty
    /// lab-posture reason naming the deployment's compensating controls
    /// (the protocol-ocpp `transport.rs` recorded-posture-reason
    /// precedent; an unexplained plaintext listener is a construction
    /// error).
    Plaintext { lab_posture_reason: String },
}

/// The configuration the composition root fills for the live VTN HTTP
/// transport (FIM-F04.01). NO path or key material is hardcoded anywhere
/// in this crate: every field is a deployment declaration, validated
/// fail-closed at [`LiveVtnTransport::new`]. The struct is serde-derivable
/// so a composition root can load it from its deployment configuration; it
/// is a transport-layer record and does not touch the frozen platform
/// vocabularies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct LiveTransportConfig {
    /// The address the listener binds (declared by the deployment; no
    /// default port is assumed anywhere in this crate).
    pub bind_address: SocketAddr,
    /// The declared wire-security posture (fail-closed; see
    /// [`WireSecurityConfig`]).
    pub security: WireSecurityConfig,
}

impl LiveTransportConfig {
    /// Validates the configuration fail-closed. Every refusal is named;
    /// nothing is repaired or defaulted.
    pub fn validate(&self) -> Result<(), TransportError> {
        match &self.security {
            WireSecurityConfig::Mtls {
                certificate_chain_path,
                private_key_path,
                trust_anchor_paths,
            } => {
                if certificate_chain_path.trim().is_empty() {
                    return Err(TransportError::ConfigInvalid {
                        field: "certificate_chain_path",
                        reason: "the server certificate-chain path is empty".to_string(),
                    });
                }
                if private_key_path.trim().is_empty() {
                    return Err(TransportError::ConfigInvalid {
                        field: "private_key_path",
                        reason: "the server private-key path is empty".to_string(),
                    });
                }
                if trust_anchor_paths.is_empty()
                    || trust_anchor_paths.iter().any(|path| path.trim().is_empty())
                {
                    return Err(TransportError::ConfigInvalid {
                        field: "trust_anchor_paths",
                        reason: "the trust-anchor path set is empty or contains an empty path \
                                 (fail-closed: no trust anchor accepts nothing, and MUST not \
                                 accept everything instead)"
                            .to_string(),
                    });
                }
            }
            WireSecurityConfig::Plaintext { lab_posture_reason } => {
                if lab_posture_reason.trim().is_empty() {
                    return Err(TransportError::ConfigInvalid {
                        field: "lab_posture_reason",
                        reason: "the live transport serves plaintext HTTP/1.1 (no TLS \
                                 termination — a HUMAN LAB GATE); configuring the plaintext \
                                 posture requires a recorded lab-posture reason naming the \
                                 deployment's compensating controls"
                            .to_string(),
                    });
                }
            }
        }
        Ok(())
    }
}

/// Errors of the live VTN transport (named, fail-closed; the shape mirrors
/// the protocol-20305 transport vocabulary).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportError {
    /// A configuration field is empty or declares an unperformable posture.
    ConfigInvalid { field: &'static str, reason: String },
    /// A declared PEM file could not be opened or decoded.
    MaterialLoadFailed { what: &'static str, reason: String },
    /// The `rustls` server configuration could not be built.
    TlsConfigFailed { reason: String },
    /// The listener could not bind.
    BindFailed { reason: String },
    /// A live connection outcome (handshake failure, malformed HTTP).
    TransportClosed { reason: String },
}

impl std::fmt::Display for TransportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransportError::ConfigInvalid { field, reason } => {
                write!(f, "transport config field '{field}' is invalid: {reason}")
            }
            TransportError::MaterialLoadFailed { what, reason } => {
                write!(f, "transport material load failed ({what}): {reason}")
            }
            TransportError::TlsConfigFailed { reason } => {
                write!(f, "TLS configuration failed: {reason}")
            }
            TransportError::BindFailed { reason } => write!(f, "bind failed: {reason}"),
            TransportError::TransportClosed { reason } => write!(f, "transport closed: {reason}"),
        }
    }
}

impl std::error::Error for TransportError {}

/// Loads the server certificate chain (PEM; leaf first).
fn load_cert_chain(
    path: &str,
) -> Result<Vec<rustls::pki_types::CertificateDer<'static>>, TransportError> {
    let file = std::fs::File::open(path).map_err(|error| TransportError::MaterialLoadFailed {
        what: "server certificate chain",
        reason: format!("{path}: {error}"),
    })?;
    let mut reader = std::io::BufReader::new(file);
    let chain: Vec<rustls::pki_types::CertificateDer<'static>> =
        rustls_pemfile::certs(&mut reader)
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
fn load_private_key(
    path: &str,
) -> Result<rustls::pki_types::PrivateKeyDer<'static>, TransportError> {
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
fn load_trust_anchors(paths: &[String]) -> Result<rustls::RootCertStore, TransportError> {
    let mut roots = rustls::RootCertStore::empty();
    for path in paths {
        let file =
            std::fs::File::open(path).map_err(|error| TransportError::MaterialLoadFailed {
                what: "trust anchor",
                reason: format!("{path}: {error}"),
            })?;
        let mut reader = std::io::BufReader::new(file);
        let anchors: Vec<rustls::pki_types::CertificateDer<'static>> =
            rustls_pemfile::certs(&mut reader)
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
            roots
                .add(anchor)
                .map_err(|error| TransportError::MaterialLoadFailed {
                    what: "trust anchor",
                    reason: format!("{path}: {error}"),
                })?;
        }
    }
    Ok(roots)
}

/// Builds the `rustls` server configuration: the client-certificate
/// verifier is REQUIRED (there is no anonymous-accept variant in this
/// module — the mTLS posture is structural), the trust anchors come from
/// the declared PEM paths, and the provider is the pinned ring default
/// (the committed lock graph resolves rustls with ring; the FIM dependency
/// decision forbids adding aws-lc-rs).
fn build_tls_server_config(
    config: &LiveTransportConfig,
) -> Result<rustls::ServerConfig, TransportError> {
    let WireSecurityConfig::Mtls {
        certificate_chain_path,
        private_key_path,
        trust_anchor_paths,
    } = &config.security
    else {
        return Err(TransportError::ConfigInvalid {
            field: "security",
            reason: "the TLS configuration builder requires the mTLS posture".to_string(),
        });
    };
    let chain = load_cert_chain(certificate_chain_path)?;
    let key = load_private_key(private_key_path)?;
    let roots = load_trust_anchors(trust_anchor_paths)?;
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

// --- FIM-F04.01: the designed-route matcher (the route data is the source) ----

/// The serving implementation each designed route maps onto. The mapping is
/// keyed on the route's DECLARED interaction + payload kinds + delivery
/// mode (the design's own vocabulary), not on ad-hoc path strings, so the
/// route data stays the single source (a path-literal edit at the UNVERIFIED
/// lab gate re-routes the serving layer without touching it).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Endpoint {
    /// `POST /registration` — VEN party registration (create or update).
    Registration,
    /// `GET /poll/{ven_id}` — the VEN-initiated event poll.
    Poll,
    /// `POST /push/{ven_id}` — the push notification surface.
    Push,
    /// `POST /report/{ven_id}` — the report-spec submission.
    Report,
    /// `GET /vens` — the operator VEN inventory (FlexCore-specific).
    VenInventory,
}

/// Classifies a designed route into its serving implementation. `None`
/// marks a designed route this transport has no implementation for — the
/// transport refuses such a design AT CONSTRUCTION (fail-closed) instead of
/// serving it incorrectly.
fn classify(route: &RouteSpec) -> Option<Endpoint> {
    use flexcore_protocol_ports::scope::ResourceClass;
    Some(
        match (route.interaction, route.request, route.response, route.delivery_mode) {
            (ResourceClass::Discovery, PayloadKind::Ven, PayloadKind::Ven, None) => {
                Endpoint::Registration
            }
            (
                ResourceClass::Control,
                PayloadKind::Ack,
                PayloadKind::Event,
                Some(DeliveryMode::Polling),
            ) => Endpoint::Poll,
            (
                ResourceClass::Control,
                PayloadKind::Event,
                PayloadKind::Ack,
                Some(DeliveryMode::Push),
            ) => Endpoint::Push,
            (
                ResourceClass::Telemetry,
                PayloadKind::ReportSpec,
                PayloadKind::Ack,
                Some(DeliveryMode::Polling),
            ) => Endpoint::Report,
            (ResourceClass::Discovery, PayloadKind::Ack, PayloadKind::Ven, None) => {
                Endpoint::VenInventory
            }
            _ => return None,
        },
    )
}

/// The outcome of matching one wire request against the designed routes.
#[derive(Debug, Clone, PartialEq, Eq)]
enum RouteMatch<'d> {
    /// A designed route matches: the serving implementation, the design
    /// record, and the captured path parameters (e.g. `ven_id`).
    Served {
        endpoint: Endpoint,
        route: &'d RouteSpec,
        params: BTreeMap<String, String>,
    },
    /// A designed path pattern matched but the verb did not: the verb the
    /// design declares for it.
    MethodMismatch { designed: HttpMethod },
    /// No designed route serves the path.
    NotFound,
}

/// Matches the wire path against the designed route patterns. Pattern
/// segments of the shape `{name}` capture one path segment (the design's
/// own path-parameter token shape — UNVERIFIED against the 3.0 text,
/// record 21 §5). No percent-decoding is performed (module header).
fn match_route<'d>(
    design: &'d VtnEndpointDesign,
    method: HttpMethod,
    path: &str,
) -> RouteMatch<'d> {
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    let mut method_mismatch: Option<HttpMethod> = None;
    for route in &design.routes {
        let pattern: Vec<&str> = route
            .path_pattern
            .split('/')
            .filter(|s| !s.is_empty())
            .collect();
        if pattern.len() != segments.len() {
            continue;
        }
        let mut params = BTreeMap::new();
        let mut matched = true;
        for (pattern_segment, segment) in pattern.iter().zip(segments.iter()) {
            if let Some(name) = pattern_segment
                .strip_prefix('{')
                .and_then(|token| token.strip_suffix('}'))
            {
                if segment.is_empty() {
                    matched = false;
                    break;
                }
                params.insert(name.to_string(), (*segment).to_string());
            } else if pattern_segment != segment {
                matched = false;
                break;
            }
        }
        if matched {
            if route.method == method {
                return RouteMatch::Served {
                    endpoint: classify(route).expect("design validated servable at construction"),
                    route,
                    params,
                };
            }
            method_mismatch.get_or_insert(route.method);
        }
    }
    match method_mismatch {
        Some(designed) => RouteMatch::MethodMismatch { designed },
        None => RouteMatch::NotFound,
    }
}

/// Converts a wire verb onto the design's closed verb vocabulary. `None`
/// marks a verb outside the vocabulary (the design does not invent verbs).
fn wire_method(method: &Method) -> Option<HttpMethod> {
    match *method {
        Method::GET => Some(HttpMethod::Get),
        Method::POST => Some(HttpMethod::Post),
        Method::PUT => Some(HttpMethod::Put),
        Method::DELETE => Some(HttpMethod::Delete),
        _ => None,
    }
}

/// The JSON error record a refusal is answered with (the designed
/// `PayloadKind::Error`: a JSON error record; the `error` field carries
/// the RECORDED error text verbatim — byte-identical to the existing
/// vocabulary's `Display`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorBody {
    /// The recorded error text (verbatim).
    pub error: String,
}

/// The named wire-routing refusals (the ONLY new refusal text this module
/// contributes — routing-level, not registry/planner/model level).
fn not_found_text(method: HttpMethod, path: &str) -> String {
    format!("no designed VTN route serves {} {path}", method.name())
}

fn method_mismatch_text(method: HttpMethod, designed: HttpMethod, path: &str) -> String {
    format!(
        "method {} is not the designed verb ({}) for {path}",
        method.name(),
        designed.name()
    )
}

fn unknown_verb_text(method: &Method) -> String {
    format!("method {method} is not in the designed verb vocabulary (GET, POST, PUT, DELETE)")
}

/// Maps a [`ServingError`] onto the wire status code. The mapping is the
/// transport's declared decision table: registry refusals name what is
/// missing (404 unknown, 403 not accepted, 409 duplicate, 400 invalid
/// record), planner and model refusals are 400 (the request is correct
/// wire-shaped but unservable), and body decode failures are 400.
fn status_for(error: &ServingError) -> StatusCode {
    match error {
        ServingError::Registry(VtnRegistryError::VenNotRegistered { .. }) => StatusCode::NOT_FOUND,
        ServingError::Registry(VtnRegistryError::VenNotAccepted { .. }) => StatusCode::FORBIDDEN,
        ServingError::Registry(VtnRegistryError::VenAlreadyRegistered { .. }) => {
            StatusCode::CONFLICT
        }
        ServingError::Registry(VtnRegistryError::InvalidVen(_)) => StatusCode::BAD_REQUEST,
        ServingError::Registry(VtnRegistryError::DeliveryModeUnavailable { .. }) => {
            StatusCode::BAD_REQUEST
        }
        ServingError::Planner(DeliveryPlanError::Registry(inner)) => {
            status_for(&ServingError::Registry(inner.clone()))
        }
        ServingError::Planner(DeliveryPlanError::ModeUnavailable { .. }) => {
            StatusCode::BAD_REQUEST
        }
        ServingError::Model(_) => StatusCode::BAD_REQUEST,
        ServingError::MalformedBody { .. } => StatusCode::BAD_REQUEST,
    }
}

/// The shared serving-state cell: HTTP/1.1 serves each request sequentially
/// per connection, but MULTIPLE connections run concurrently in the one
/// current-thread serving loop — the shared `Rc<RefCell<..>>` multiplexes
/// the single `&mut VtnServingState` borrow across them. The serving state
/// is synchronous, so the borrow is taken and dropped within one poll; it
/// is never held across an await.
type SharedStateCell<'s> = Rc<RefCell<&'s mut VtnServingState>>;

/// Maps one wire request onto the designed routes and the serving state,
/// and maps the outcome back to the wire. FIM-F04.01.
async fn serve_wire_request<B: HttpBody<Data = Bytes> + 'static>(
    request: HttpRequest<B>,
    state: SharedStateCell<'_>,
    design: &VtnEndpointDesign,
) -> HttpResponse<Full<Bytes>>
where
    B::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    // 1. The verb, onto the design's closed vocabulary. A verb outside the
    //    vocabulary is a named 405 (the design does not invent verbs).
    let Some(method) = wire_method(request.method()) else {
        return error_to_wire(
            StatusCode::METHOD_NOT_ALLOWED,
            &unknown_verb_text(request.method()),
        );
    };
    // 2. The route match against the DESIGNED route data (method + path
    //    pattern with path parameters; no percent-decoding — module
    //    header).
    let path = request.uri().path().to_string();
    let matched = match_route(design, method, &path);
    let (endpoint, route, params) = match matched {
        RouteMatch::Served {
            endpoint,
            route,
            params,
        } => (endpoint, route, params),
        RouteMatch::MethodMismatch { designed } => {
            return error_to_wire(
                StatusCode::METHOD_NOT_ALLOWED,
                &method_mismatch_text(method, designed, &path),
            );
        }
        RouteMatch::NotFound => {
            return error_to_wire(StatusCode::NOT_FOUND, &not_found_text(method, &path));
        }
    };
    // 3. The body: routes whose designed request payload is a model type
    //    decode the body into the design's JSON shape; a body that does
    //    not decode is a named 400 whose text is the recorded
    //    `ServingError::MalformedBody` wording. Routes with an `Ack`
    //    request ignore the body (the design's request kind).
    let mut body_bytes: Vec<u8> = Vec::new();
    if route.request != PayloadKind::Ack {
        match request.into_body().collect().await {
            Ok(collected) => body_bytes = collected.to_bytes().to_vec(),
            Err(_) => {
                return error_to_wire(
                    StatusCode::BAD_REQUEST,
                    &ServingError::MalformedBody {
                        reason: "the request body could not be read".to_string(),
                    }
                    .to_string(),
                );
            }
        }
    }
    // 4. The serving call: the borrow is taken and released within this
    //    synchronous block (never held across an await).
    let outcome: Result<OutcomePayload, ServingError> = {
        let mut state = state.borrow_mut();
        dispatch(&mut state, endpoint, route, &params, &body_bytes)
    };
    // 5. The wire response: the designed payload encoding, the declared
    //    status, and — on refusal — the RECORDED error text verbatim.
    match outcome {
        Ok(OutcomePayload::Value(value, status)) => value_to_wire(&value, status),
        Err(error) => error_to_wire(status_for(&error), &error.to_string()),
    }
}

/// The successful outcome of one dispatch (payload + declared status).
/// The designed `Ack` response kind is served as its JSON receipt record
/// (an object), so every success carries a value.
enum OutcomePayload {
    /// A designed payload value with its status (200/201).
    Value(serde_json::Value, StatusCode),
}

/// Dispatches one matched request onto the serving state, per the designed
/// route's serving implementation. Every registry/planner/model refusal
/// surfaces VERBATIM through [`ServingError`]'s `Display` (FIM-F04.02).
fn dispatch(
    state: &mut VtnServingState,
    endpoint: Endpoint,
    route: &RouteSpec,
    params: &BTreeMap<String, String>,
    body: &[u8],
) -> Result<OutcomePayload, ServingError> {
    match endpoint {
        Endpoint::Registration => {
            let wire: Ven = decode_body(body, route)?;
            let ven_id = wire.ven_id.clone();
            let outcome = state.register_ven(wire)?;
            // The response is the designed `Ven` payload — the record AS
            // SERVED (with the operator-owned acceptance flag).
            let served = state
                .registry
                .ven(&ven_id)
                .map(|ven| serde_json::to_value(ven))
                .transpose()
                .map_err(|error| ServingError::MalformedBody {
                    reason: format!("the served VEN record failed to encode: {error}"),
                })?
                .unwrap_or(serde_json::Value::Null);
            let status = match outcome {
                VenRegistrationOutcome::Created => StatusCode::CREATED,
                VenRegistrationOutcome::Updated => StatusCode::OK,
            };
            Ok(OutcomePayload::Value(served, status))
        }
        Endpoint::Poll => {
            let ven_id = require_ven_id(params)?;
            let delivered = state.poll(&ven_id)?;
            // The response is the designed `Event` payload — the delivered
            // event set, each carrying the version in force (its
            // modification number) in the body itself.
            let value = serde_json::to_value(&delivered).map_err(|error| {
                ServingError::MalformedBody {
                    reason: format!("the delivered events failed to encode: {error}"),
                }
            })?;
            Ok(OutcomePayload::Value(value, StatusCode::OK))
        }
        Endpoint::Push => {
            let ven_id = require_ven_id(params)?;
            let wire: OpenAdrEvent = decode_body(body, route)?;
            let plan = state.notify_push(&ven_id, wire)?;
            // The receipt names what was (and was NOT) done: the event is
            // queued for the VEN's agreed push surface; no delivery is
            // claimed (module header: no outbound push sender).
            let value = serde_json::json!({
                "queued": true,
                "plan": plan.describe(),
            });
            Ok(OutcomePayload::Value(value, StatusCode::OK))
        }
        Endpoint::Report => {
            let ven_id = require_ven_id(params)?;
            let wire: ReportSpec = decode_body(body, route)?;
            state.submit_report(&ven_id, wire)?;
            Ok(OutcomePayload::Value(
                serde_json::json!({ "recorded": true }),
                StatusCode::OK,
            ))
        }
        Endpoint::VenInventory => {
            let vens: Vec<&Ven> = state.registry.all().collect();
            let value =
                serde_json::to_value(&vens).map_err(|error| ServingError::MalformedBody {
                    reason: format!("the VEN inventory failed to encode: {error}"),
                })?;
            Ok(OutcomePayload::Value(value, StatusCode::OK))
        }
    }
}

/// Extracts the designed `{ven_id}` path parameter.
fn require_ven_id(params: &BTreeMap<String, String>) -> Result<String, ServingError> {
    params.get("ven_id").cloned().ok_or(ServingError::MalformedBody {
        reason: "the route matched without its {ven_id} path parameter (design error)"
            .to_string(),
    })
}

/// Decodes the request body into the designed payload kind (the design's
/// JSON shape; the serving state revalidates through the EXISTING model
/// constructors). A body that does not decode is a named
/// [`ServingError::MalformedBody`].
fn decode_body<T: serde::de::DeserializeOwned>(
    body: &[u8],
    route: &RouteSpec,
) -> Result<T, ServingError> {
    serde_json::from_slice(body).map_err(|error| ServingError::MalformedBody {
        reason: format!(
            "the body does not decode as the designed '{}' payload of {} {}: {error}",
            route.request.name(),
            route.method.name(),
            route.path_pattern
        ),
    })
}

// --- FIM-F04.01: the wire response mapping -------------------------------------

/// Encodes a designed payload onto the wire: the declared status, the JSON
/// design-surface encoding, `Content-Type: application/json`. An encoding
/// failure degrades to a named 500 — never to an Ok status with an empty
/// body.
fn value_to_wire(value: &serde_json::Value, status: StatusCode) -> HttpResponse<Full<Bytes>> {
    let body_bytes = match serde_json::to_vec(value) {
        Ok(bytes) => bytes,
        Err(_) => return empty_wire_response(StatusCode::INTERNAL_SERVER_ERROR),
    };
    HttpResponse::builder()
        .status(status)
        .header(CONTENT_TYPE, "application/json")
        .body(Full::new(Bytes::from(body_bytes)))
        .unwrap_or_else(|_| empty_wire_response(StatusCode::INTERNAL_SERVER_ERROR))
}

/// Encodes a refusal onto the wire: the declared status and the JSON error
/// record whose `error` field carries the RECORDED text verbatim.
fn error_to_wire(status: StatusCode, text: &str) -> HttpResponse<Full<Bytes>> {
    let record = ErrorBody {
        error: text.to_string(),
    };
    let body_bytes = match serde_json::to_vec(&record) {
        Ok(bytes) => bytes,
        Err(_) => return empty_wire_response(StatusCode::INTERNAL_SERVER_ERROR),
    };
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

/// The hyper service that serves the DESIGNED VTN surface over HTTP/1.1
/// (the FIM-F01 pattern of `protocol-20305` — FIM-F04.01 here). The
/// serving state is the shared cell; each request's future owns a clone of
/// it. Generic over the request body so the socket-free pinned tests can
/// drive the service directly with `Full<Bytes>` bodies while the live
/// loop feeds it hyper's `Incoming`.
pub struct VtnHttpService<'s> {
    state: SharedStateCell<'s>,
    design: Arc<VtnEndpointDesign>,
}

impl<'s> VtnHttpService<'s> {
    /// Wraps a shared serving-state cell and the validated endpoint design.
    pub fn new(state: SharedStateCell<'s>, design: Arc<VtnEndpointDesign>) -> Self {
        Self { state, design }
    }
}

impl<'s, B> Service<HttpRequest<B>> for VtnHttpService<'s>
where
    B: HttpBody<Data = Bytes> + Send + 'static,
    B::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    type Response = HttpResponse<Full<Bytes>>;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + 's>>;

    // Batch 68: this implements `hyper::service::Service` (hyper 1.x),
    // whose `call` takes `&self` and which has NO `poll_ready` — the
    // previous `poll_ready` + `call(&mut self)` shape was tower's
    // `Service`, not hyper's. The body only clones the shared state, so
    // `&self` is sufficient (the serving state is always ready; refusals
    // are named statuses, not backpressure).
    fn call(&self, request: HttpRequest<B>) -> Self::Future {
        let state = Rc::clone(&self.state);
        let design = Arc::clone(&self.design);
        Box::pin(async move {
            let response = serve_wire_request(request, state, &design).await;
            Ok::<_, Infallible>(response)
        })
    }
}

// --- FIM-F04.01: the live listener ---------------------------------------------

/// The LIVE VTN transport (FIM-F04.01): a tokio TCP accept loop serving
/// every connection with hyper 1.x HTTP/1.1 through [`VtnHttpService`] onto
/// the EXISTING serving state (`crate::serving`), under the configured
/// fail-closed wire-security posture.
///
/// **WRITTEN-UNRUN; interop with a real OpenADR 3.0 VEN is UNPROVEN and
/// remains a HUMAN LAB GATE** (module header).
pub struct LiveVtnTransport {
    /// The validated configuration (held by value — the composition root
    /// declares it, this transport never mutates it).
    config: LiveTransportConfig,
    /// The validated endpoint design (the route data this serving layer
    /// reads; held behind an `Arc` so per-connection services clone it).
    design: Arc<VtnEndpointDesign>,
    /// The eagerly-built TLS configuration (`Some` only for the mTLS
    /// posture; construction is fail-closed: missing material refuses
    /// HERE, never at first accept).
    tls_config: Option<Arc<rustls::ServerConfig>>,
    /// The shutdown signal (the composition root's stop mechanism).
    shutdown: Arc<Notify>,
}

impl LiveVtnTransport {
    /// Constructs the transport fail-closed: the configuration is
    /// validated, the endpoint design is validated SERVABLE (every
    /// designed route maps onto a serving implementation — a design this
    /// transport cannot serve is refused, never served incorrectly), and
    /// ALL TLS material is loaded and the `rustls` server configuration is
    /// built BEFORE anything binds. Every refusal is a named
    /// [`TransportError`].
    pub fn new(
        config: LiveTransportConfig,
        design: VtnEndpointDesign,
    ) -> Result<Self, TransportError> {
        config.validate()?;
        for route in &design.routes {
            if classify(route).is_none() {
                return Err(TransportError::ConfigInvalid {
                    field: "routes",
                    reason: format!(
                        "the designed route '{} {}' carries no serving implementation in this \
                         transport (fail-closed: an unservable design is refused, never \
                         served incorrectly)",
                        route.method.name(),
                        route.path_pattern
                    ),
                });
            }
        }
        let tls_config = match &config.security {
            WireSecurityConfig::Mtls { .. } => Some(Arc::new(build_tls_server_config(&config)?)),
            WireSecurityConfig::Plaintext { .. } => None,
        };
        Ok(Self {
            config,
            design: Arc::new(design),
            tls_config,
            shutdown: Arc::new(Notify::new()),
        })
    }

    /// The validated configuration.
    pub fn config(&self) -> &LiveTransportConfig {
        &self.config
    }

    /// The validated endpoint design the serving layer reads.
    pub fn design(&self) -> &VtnEndpointDesign {
        &self.design
    }

    /// The recorded lab-posture reason (present only for the plaintext
    /// posture; the recorded-reason shape of the protocol-ocpp precedent —
    /// the reason travels with the deployment).
    pub fn lab_posture_reason(&self) -> Option<&str> {
        match &self.config.security {
            WireSecurityConfig::Mtls { .. } => None,
            WireSecurityConfig::Plaintext { lab_posture_reason } => {
                Some(lab_posture_reason.as_str())
            }
        }
    }

    /// The shutdown handle: the composition root clones this BEFORE
    /// calling `serve` (which consumes the transport) and notifies it to
    /// stop the serving loop. A stored permit makes the signal race-free:
    /// a notify arriving before the loop's first await still stops the
    /// transport.
    pub fn shutdown_handle(&self) -> Arc<Notify> {
        Arc::clone(&self.shutdown)
    }

    /// The current-thread serving loop (FIM-F04.01): bind, accept, apply
    /// the configured wire-security posture, and serve HTTP/1.1 through
    /// [`VtnHttpService`] onto the serving state. Returns `Ok(())` on a
    /// shutdown signal; every other outcome is a named
    /// [`TransportError`].
    async fn serve_current_thread(&self, state: &mut VtnServingState) -> Result<(), TransportError> {
        let listener = TcpListener::bind(self.config.bind_address)
            .await
            .map_err(|error| TransportError::BindFailed {
                reason: format!("{}: {error}", self.config.bind_address),
            })?;
        let acceptor = self.tls_config.clone().map(TlsAcceptor::from);
        let state: SharedStateCell<'_> = Rc::new(RefCell::new(state));
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
                            let state = Rc::clone(&state);
                            let design = Arc::clone(&self.design);
                            connections.push(Box::pin(Self::serve_one_connection(
                                acceptor, state, design, stream,
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
                    // Per-connection failures (handshake refusals,
                    // malformed HTTP) are fail-closed CONNECTION outcomes
                    // and do not stop the server: every one is a named
                    // TransportError value that this loop drops (the
                    // transport has no logging facility in this batch —
                    // observability is the P14.21 seam).
                    let _ = finished;
                }
            }
        }
    }

    /// Serves ONE accepted TCP connection to completion: under the mTLS
    /// posture, the TLS handshake (fail-closed on any error — the required
    /// client-certificate verifier refuses anonymous VENs at the protocol
    /// layer); then hyper HTTP/1.1 over the (possibly plain) stream.
    async fn serve_one_connection(
        acceptor: Option<TlsAcceptor>,
        state: SharedStateCell<'_>,
        design: Arc<VtnEndpointDesign>,
        stream: TcpStream,
    ) -> Result<(), TransportError> {
        let service = VtnHttpService::new(state, design);
        match acceptor {
            Some(acceptor) => {
                let tls_stream = acceptor
                    .accept(stream)
                    .await
                    .map_err(|error| TransportError::TransportClosed {
                        reason: format!("tls handshake failed: {error}"),
                    })?;
                http1::Builder::new()
                    .serve_connection(TokioIo::new(tls_stream), service)
                    .await
                    .map_err(|error| TransportError::TransportClosed {
                        reason: format!("http1 connection error: {error}"),
                    })?;
            }
            None => {
                // The recorded plaintext posture: served as configured —
                // never a silently downgraded mTLS deployment.
                http1::Builder::new()
                    .serve_connection(TokioIo::new(stream), service)
                    .await
                    .map_err(|error| TransportError::TransportClosed {
                        reason: format!("http1 connection error: {error}"),
                    })?;
            }
        }
        Ok(())
    }

    /// Blocks on a dedicated current-thread tokio runtime and serves until
    /// the shutdown handle fires (see
    /// [`LiveVtnTransport::shutdown_handle`]) or a named failure occurs.
    pub fn serve(self, state: &mut VtnServingState) -> Result<(), TransportError> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| TransportError::BindFailed {
                reason: format!("tokio current-thread runtime: {error}"),
            })?;
        runtime.block_on(self.serve_current_thread(state))
    }
}

// --- FIM-F04.04: pinned socket-free tests ---------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{EventDescriptor, EventSignal, Interval, SignalUnits};
    use crate::serving::DeliveryRecord;

    fn design() -> Arc<VtnEndpointDesign> {
        Arc::new(VtnEndpointDesign::canonical_vtn_design().expect("valid canonical design"))
    }

    fn state() -> VtnServingState {
        VtnServingState::new("openadr-vtn", vec![DeliveryMode::Polling, DeliveryMode::Push])
            .expect("valid state")
    }

    fn shared(state: &mut VtnServingState) -> SharedStateCell<'_> {
        Rc::new(RefCell::new(state))
    }

    fn wire_request(method: Method, path: &str, body: Option<&str>) -> HttpRequest<Full<Bytes>> {
        let uri = path.parse::<http::Uri>().expect("valid uri");
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

    async fn body_text(response: HttpResponse<Full<Bytes>>) -> Vec<u8> {
        response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes()
            .to_vec()
    }

    async fn error_text(response: HttpResponse<Full<Bytes>>) -> String {
        let bytes = body_text(response).await;
        let record: ErrorBody = serde_json::from_slice(&bytes).expect("error record decodes");
        record.error
    }

    fn accepted_polling_ven(id: &str) -> Ven {
        Ven::new(id.to_string(), format!("VEN {id}"), None, 60_000, true).expect("valid ven")
    }

    fn event(modification_number: u32) -> OpenAdrEvent {
        let descriptor = EventDescriptor::new(
            "evt-42".to_string(),
            modification_number,
            "iso-ne".to_string(),
            3,
            false,
        )
        .expect("valid descriptor");
        let signal = EventSignal::new(
            "simple".to_string(),
            SignalUnits::Kilowatts,
            vec![Interval::new(1, 1_700_000_000_000, 1_800_000, 2_000.0).expect("valid interval")],
        )
        .expect("valid signal");
        OpenAdrEvent::new(
            descriptor,
            vec!["ven-1".to_string()],
            vec![signal],
            "program-7".to_string(),
        )
        .expect("valid event")
    }

    // --- FIM-F04.01: the fail-closed construction ---------------------------

    fn valid_config() -> LiveTransportConfig {
        LiveTransportConfig {
            bind_address: "127.0.0.1:0".parse().expect("valid socket addr"),
            security: WireSecurityConfig::Mtls {
                certificate_chain_path: "/pki/server.pem".to_string(),
                private_key_path: "/pki/server.key.pem".to_string(),
                trust_anchor_paths: vec!["/pki/ven-ca.pem".to_string()],
            },
        }
    }

    #[test]
    fn transport_config_is_fail_closed() {
        // An empty certificate-chain path.
        let mut config = valid_config();
        if let WireSecurityConfig::Mtls {
            certificate_chain_path,
            ..
        } = &mut config.security
        {
            *certificate_chain_path = "  ".to_string();
        }
        assert!(matches!(
            config.validate(),
            Err(TransportError::ConfigInvalid { field: "certificate_chain_path", .. })
        ));
        // An empty private-key path.
        let mut config = valid_config();
        if let WireSecurityConfig::Mtls { private_key_path, .. } = &mut config.security {
            *private_key_path = String::new();
        }
        assert!(matches!(
            config.validate(),
            Err(TransportError::ConfigInvalid { field: "private_key_path", .. })
        ));
        // No trust anchors at all (fail-closed: no anchor accepts nothing).
        let mut config = valid_config();
        if let WireSecurityConfig::Mtls {
            trust_anchor_paths, ..
        } = &mut config.security
        {
            *trust_anchor_paths = Vec::new();
        }
        assert!(matches!(
            config.validate(),
            Err(TransportError::ConfigInvalid { field: "trust_anchor_paths", .. })
        ));
        // An unexplained plaintext listener is refused at construction
        // (the recorded-posture-reason precedent).
        let config = LiveTransportConfig {
            bind_address: "127.0.0.1:0".parse().expect("valid socket addr"),
            security: WireSecurityConfig::Plaintext {
                lab_posture_reason: "   ".to_string(),
            },
        };
        assert!(matches!(
            config.validate(),
            Err(TransportError::ConfigInvalid { field: "lab_posture_reason", .. })
        ));
        // The valid configurations pass.
        assert_eq!(valid_config().validate(), Ok(()));
        let config = LiveTransportConfig {
            bind_address: "127.0.0.1:0".parse().expect("valid socket addr"),
            security: WireSecurityConfig::Plaintext {
                lab_posture_reason: "lab posture: simulation-only deployment; the listener is \
                                     reachable only on an isolated lab network"
                    .to_string(),
            },
        };
        assert_eq!(config.validate(), Ok(()));
    }

    #[test]
    fn construction_refuses_an_unservable_design() {
        // A designed route this transport has no implementation for is
        // refused AT CONSTRUCTION (fail-closed), never served incorrectly.
        let unservable = VtnEndpointDesign::new(
            "openadr-vtn".to_string(),
            vec![RouteSpec {
                method: HttpMethod::Post,
                path_pattern: "/mystery/{ven_id}".to_string(),
                interaction: flexcore_protocol_ports::scope::ResourceClass::Control,
                delivery_mode: Some(DeliveryMode::Polling),
                request: PayloadKind::ReportSpec,
                response: PayloadKind::Event,
                purpose: "a payload-kind combination this transport does not serve".to_string(),
            }],
            "an unservable design".to_string(),
        )
        .expect("valid design shape");
        let error = LiveVtnTransport::new(valid_config(), unservable).expect_err("refused");
        assert!(matches!(
            error,
            TransportError::ConfigInvalid { field: "routes", .. }
        ));
    }

    #[test]
    fn the_canonical_design_is_fully_servable() {
        // The designed surface maps 1:1 onto serving implementations: the
        // transport construction succeeds on the canonical design (with a
        // plaintext posture the material check can accept without PKI).
        let config = LiveTransportConfig {
            bind_address: "127.0.0.1:0".parse().expect("valid socket addr"),
            security: WireSecurityConfig::Plaintext {
                lab_posture_reason: "lab posture: unit-test construction only; no socket is \
                                     opened in this test"
                    .to_string(),
            },
        };
        let transport =
            LiveVtnTransport::new(config, design().as_ref().clone()).expect("servable design");
        assert_eq!(transport.design().routes.len(), 5);
        assert_eq!(
            transport.lab_posture_reason(),
            Some("lab posture: unit-test construction only; no socket is opened in this test"),
        );
    }

    // --- FIM-F04.01: the route matcher ---------------------------------------

    #[test]
    fn the_route_matcher_reads_the_design_data() {
        let design = design();
        // The designed routes match, with the path parameters captured.
        let matched = match_route(&design, HttpMethod::Get, "/poll/ven-1");
        let RouteMatch::Served {
            endpoint, params, ..
        } = matched
        else {
            panic!("the designed poll route must match");
        };
        assert_eq!(endpoint, Endpoint::Poll);
        assert_eq!(params.get("ven_id").map(String::as_str), Some("ven-1"));
        // A designed path hit with the wrong verb is a named mismatch.
        let mismatch = match_route(&design, HttpMethod::Get, "/registration");
        assert_eq!(
            mismatch,
            RouteMatch::MethodMismatch {
                designed: HttpMethod::Post
            }
        );
        // An undesigned path is not found.
        assert_eq!(
            match_route(&design, HttpMethod::Get, "/secret"),
            RouteMatch::NotFound
        );
        // An extra segment is not found (no prefix matching).
        assert_eq!(
            match_route(&design, HttpMethod::Get, "/poll/ven-1/extra"),
            RouteMatch::NotFound
        );
    }

    // --- FIM-F04.04 (a): the registration round-trip over the wire ----------

    #[tokio::test]
    async fn registration_round_trip_creates_then_updates_preserving_acceptance() {
        let mut vt = state();
        {
            let mut service = VtnHttpService::new(shared(&mut vt), design());

            // First registration: created (201), returned UNACCEPTED — the
            // wire field claiming acceptance is ignored (the operator
            // surface owns the flag; the model.rs rule).
            let wire = serde_json::json!({
                "ven_id": "ven-1",
                "ven_name": "VEN 1",
                "push_endpoint": null,
                "poll_frequency_millis": 60_000,
                "accepted": true
            });
            let response = service
                .call(wire_request(
                    Method::POST,
                    "/registration",
                    Some(wire.to_string().as_str()),
                ))
                .await
                .expect("infallible");
            assert_eq!(response.status(), StatusCode::CREATED);
            let bytes = body_text(response).await;
            let served: serde_json::Value = serde_json::from_slice(&bytes).expect("decodes");
            assert_eq!(served["ven_id"], "ven-1");
            assert_eq!(
                served["accepted"], false,
                "a wire registration is never self-accepted"
            );
        }

        // The operator accepts the VEN (the operator surface's decision).
        vt.registry
            .update(accepted_polling_ven("ven-1"))
            .expect("operator update");

        {
            let mut service = VtnHttpService::new(shared(&mut vt), design());
            // Re-registration: updated (200) and the operator acceptance is
            // PRESERVED (the wire cannot flip it).
            let wire = serde_json::json!({
                "ven_id": "ven-1",
                "ven_name": "VEN 1 renamed",
                "push_endpoint": null,
                "poll_frequency_millis": 30_000,
                "accepted": false
            });
            let response = service
                .call(wire_request(
                    Method::POST,
                    "/registration",
                    Some(wire.to_string().as_str()),
                ))
                .await
                .expect("infallible");
            assert_eq!(response.status(), StatusCode::OK);
            let bytes = body_text(response).await;
            let served: serde_json::Value = serde_json::from_slice(&bytes).expect("decodes");
            assert_eq!(served["ven_name"], "VEN 1 renamed");
            assert_eq!(served["poll_frequency_millis"], 30_000);
            assert_eq!(served["accepted"], true, "the wire cannot flip acceptance");
        }
    }

    // --- FIM-F04.04 (b): poll delivering an event per the plan ---------------

    #[tokio::test]
    async fn poll_delivers_the_planned_event_set_over_the_wire() {
        let mut vt = state();
        vt.registry
            .register(accepted_polling_ven("ven-1"))
            .expect("register");
        vt.queue_event(event(2)).expect("queued");
        {
            let mut service = VtnHttpService::new(shared(&mut vt), design());

            let response = service
                .call(wire_request(Method::GET, "/poll/ven-1", None))
                .await
                .expect("infallible");
            assert_eq!(response.status(), StatusCode::OK);
            let bytes = body_text(response).await;
            let events: Vec<serde_json::Value> = serde_json::from_slice(&bytes).expect("decodes");
            assert_eq!(events.len(), 1);
            assert_eq!(events[0]["descriptor"]["event_id"], "evt-42");
            assert_eq!(
                events[0]["descriptor"]["modification_number"], 2,
                "the served event carries the version in force in its own body"
            );
            // The queue is drained: a second poll delivers an empty set.
            let response = service
                .call(wire_request(Method::GET, "/poll/ven-1", None))
                .await
                .expect("infallible");
            assert_eq!(response.status(), StatusCode::OK);
            let bytes = body_text(response).await;
            let events: Vec<serde_json::Value> = serde_json::from_slice(&bytes).expect("decodes");
            assert!(events.is_empty());
        }

        // The delivery is recorded (FIM-F04.03) with the event version in
        // force: the verbatim modification number of the served event.
        let log = vt.delivery_log();
        assert_eq!(log.len(), 1);
        let DeliveryRecord {
            ven_id,
            event_id,
            modification_number,
            mode,
            ..
        } = &log[0];
        assert_eq!(ven_id, "ven-1");
        assert_eq!(event_id, "evt-42");
        assert_eq!(*modification_number, 2);
        assert_eq!(*mode, DeliveryMode::Polling);
    }

    // --- FIM-F04.04 (c): report upload ----------------------------------------

    #[tokio::test]
    async fn report_upload_lands_in_the_existing_report_model() {
        let mut vt = state();
        vt.registry
            .register(accepted_polling_ven("ven-1"))
            .expect("register");
        {
            let mut service = VtnHttpService::new(shared(&mut vt), design());

            let wire = serde_json::json!({
                "report_specifier_id": "r-1",
                "report_name": "telemetry",
                "granularity_millis": 60_000
            });
            let response = service
                .call(wire_request(
                    Method::POST,
                    "/report/ven-1",
                    Some(wire.to_string().as_str()),
                ))
                .await
                .expect("infallible");
            assert_eq!(response.status(), StatusCode::OK);
            let bytes = body_text(response).await;
            let receipt: serde_json::Value = serde_json::from_slice(&bytes).expect("decodes");
            assert_eq!(receipt["recorded"], true);
        }
        // The spec landed in the existing report model with its EXISTING
        // validation and its EXISTING structural telemetry-only rule.
        let specs = vt.reports_for("ven-1");
        assert_eq!(specs.len(), 1);
        assert_eq!(specs[0].report_specifier_id, "r-1");
        assert!(specs[0].telemetry_only);
    }

    // --- FIM-F04.04 (d): the byte-identical error paths -----------------------

    #[tokio::test]
    async fn error_paths_carry_the_recorded_text_byte_identical() {
        let mut vt = state();
        vt.registry
            .register(accepted_polling_ven("ven-1"))
            .expect("register");

        // UNKNOWN VEN poll: the recorded VtnRegistryError text, verbatim.
        {
            let mut service = VtnHttpService::new(shared(&mut vt), design());
            let response = service
                .call(wire_request(Method::GET, "/poll/ven-z", None))
                .await
                .expect("infallible");
            assert_eq!(response.status(), StatusCode::NOT_FOUND);
            let expected = VtnRegistryError::VenNotRegistered {
                ven_id: "ven-z".to_string(),
            }
            .to_string();
            assert_eq!(error_text(response).await, expected);
        }

        // WRONG MODE: a planner whose declared modes exclude polling
        // refuses the poll route with the recorded DeliveryPlanError text,
        // verbatim.
        let mut push_only =
            VtnServingState::new("openadr-vtn", vec![DeliveryMode::Push]).expect("valid state");
        push_only
            .registry
            .register(accepted_polling_ven("ven-1"))
            .expect("register");
        {
            let mut service = VtnHttpService::new(shared(&mut push_only), design());
            let response = service
                .call(wire_request(Method::GET, "/poll/ven-1", None))
                .await
                .expect("infallible");
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
            let expected = DeliveryPlanError::ModeUnavailable {
                ven_id: "ven-1".to_string(),
                mode: "polling".to_string(),
            }
            .to_string();
            assert_eq!(error_text(response).await, expected);
        }

        // MALFORMED BODY (a registration body that is not the designed
        // payload): the named MalformedBody text (byte-identical to the
        // recorded wording, with the underlying decoder's own message).
        {
            let mut service = VtnHttpService::new(shared(&mut vt), design());
            let response = service
                .call(wire_request(
                    Method::POST,
                    "/registration",
                    Some("{not-json"),
                ))
                .await
                .expect("infallible");
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
            let decoder_text = match serde_json::from_slice::<Ven>(b"{not-json") {
                Err(error) => error.to_string(),
                Ok(_) => panic!("the malformed body must not decode"),
            };
            let expected = ServingError::MalformedBody {
                reason: format!(
                    "the body does not decode as the designed 'ven' payload of POST \
                     /registration: {decoder_text}"
                ),
            }
            .to_string();
            assert_eq!(error_text(response).await, expected);
        }

        // A body that decodes as JSON but fails the EXISTING model
        // validation carries the recorded ModelError text, verbatim.
        {
            let mut service = VtnHttpService::new(shared(&mut vt), design());
            let wire = serde_json::json!({
                "ven_id": "ven-bad",
                "ven_name": "",
                "push_endpoint": null,
                "poll_frequency_millis": 60_000,
                "accepted": false
            });
            let response = service
                .call(wire_request(
                    Method::POST,
                    "/registration",
                    Some(wire.to_string().as_str()),
                ))
                .await
                .expect("infallible");
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
            let expected = VtnRegistryError::InvalidVen(crate::model::ModelError::EmptyField(
                "ven_name",
            ))
            .to_string();
            assert_eq!(error_text(response).await, expected);
        }
    }

    // --- FIM-F04.04 (e): routing refusals and the auth posture -----------------

    #[tokio::test]
    async fn routing_refusals_and_the_report_validation_are_named() {
        let mut vt = state();
        vt.registry
            .register(accepted_polling_ven("ven-1"))
            .expect("register");

        // A designed path hit with the wrong verb: named 405.
        {
            let mut service = VtnHttpService::new(shared(&mut vt), design());
            let response = service
                .call(wire_request(Method::GET, "/registration", None))
                .await
                .expect("infallible");
            assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
            assert_eq!(
                error_text(response).await,
                method_mismatch_text(HttpMethod::Get, HttpMethod::Post, "/registration")
            );

            // A verb outside the design's vocabulary: named 405.
            let response = service
                .call(wire_request(Method::PATCH, "/poll/ven-1", None))
                .await
                .expect("infallible");
            assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
            assert_eq!(
                error_text(response).await,
                unknown_verb_text(&Method::PATCH)
            );

            // An undesigned path: named 404.
            let response = service
                .call(wire_request(Method::GET, "/secret", None))
                .await
                .expect("infallible");
            assert_eq!(response.status(), StatusCode::NOT_FOUND);
            assert_eq!(
                error_text(response).await,
                not_found_text(HttpMethod::Get, "/secret")
            );

            // A report body failing the EXISTING validation carries the
            // recorded ModelError text, verbatim.
            let wire = serde_json::json!({
                "report_specifier_id": "r-bad",
                "report_name": "bad",
                "granularity_millis": 0
            });
            let response = service
                .call(wire_request(
                    Method::POST,
                    "/report/ven-1",
                    Some(wire.to_string().as_str()),
                ))
                .await
                .expect("infallible");
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
            let expected = crate::model::ModelError::InvalidInterval.to_string();
            assert_eq!(error_text(response).await, expected);
        }

        // The auth posture: the endpoint design declares NO authentication
        // route, so the refusal surface is the registry's (unknown VEN,
        // not accepted) plus the fail-closed TLS construction (pinned in
        // `transport_config_is_fail_closed` and the gated live suite). A
        // not-accepted VEN is refused with the recorded text, verbatim.
        vt.registry
            .register(Ven::new(
                "ven-pending".to_string(),
                "pending".to_string(),
                None,
                60_000,
                false,
            )
            .expect("valid ven"))
            .expect("register");
        {
            let mut service = VtnHttpService::new(shared(&mut vt), design());
            let response = service
                .call(wire_request(Method::GET, "/poll/ven-pending", None))
                .await
                .expect("infallible");
            assert_eq!(response.status(), StatusCode::FORBIDDEN);
            let expected = VtnRegistryError::VenNotAccepted {
                ven_id: "ven-pending".to_string(),
            }
            .to_string();
            assert_eq!(error_text(response).await, expected);
        }
    }

    // --- FIM-F04.04 (f): the push route queues, never claims delivery --------

    #[tokio::test]
    async fn push_route_queues_the_event_and_never_claims_delivery() {
        let mut vt = state();
        vt.registry
            .register(Ven::new(
                "ven-1".to_string(),
                "VEN 1".to_string(),
                Some("https://ven.example/push".to_string()),
                60_000,
                true,
            )
            .expect("valid ven"))
            .expect("register");
        {
            let mut service = VtnHttpService::new(shared(&mut vt), design());
            let wire = serde_json::to_value(event(5)).expect("serializes");
            let response = service
                .call(wire_request(
                    Method::POST,
                    "/push/ven-1",
                    Some(wire.to_string().as_str()),
                ))
                .await
                .expect("infallible");
            assert_eq!(response.status(), StatusCode::OK);
            let bytes = body_text(response).await;
            let receipt: serde_json::Value = serde_json::from_slice(&bytes).expect("decodes");
            assert_eq!(receipt["queued"], true);
            assert!(
                receipt["plan"]
                    .as_str()
                    .expect("plan text")
                    .contains("https://ven.example/push"),
                "the receipt names the agreed push surface"
            );
        }
        // Queued for the agreed surface — but NO delivery record exists
        // (no outbound push sender in this written-unrun program).
        assert_eq!(vt.pending_count("ven-1"), 1);
        assert!(vt.delivery_log().is_empty());
    }

    // --- FIM-F04.04 (g): the operator inventory surface -----------------------

    #[tokio::test]
    async fn the_operator_inventory_serves_the_registered_vens() {
        let mut vt = state();
        vt.registry
            .register(accepted_polling_ven("ven-1"))
            .expect("register");
        vt.registry
            .register(accepted_polling_ven("ven-2"))
            .expect("register");
        let mut service = VtnHttpService::new(shared(&mut vt), design());

        let response = service
            .call(wire_request(Method::GET, "/vens", None))
            .await
            .expect("infallible");
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = body_text(response).await;
        let vens: Vec<serde_json::Value> = serde_json::from_slice(&bytes).expect("decodes");
        assert_eq!(vens.len(), 2);
        assert_eq!(vens[0]["ven_id"], "ven-1", "BTreeMap ordering is deterministic");
        assert_eq!(vens[1]["ven_id"], "ven-2");
    }
}
