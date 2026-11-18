// P14.02 §4 — The AggregatorServer skeleton (decision record
// 02_p14_02_aggregator_server.md §4).
//
// This is a SKELETON by declared design: the resource store, the routing,
// the query semantics, and the fail-closed TLS requirements are real model
// code; the HTTP/TLS serving itself is a named, UNRUN seam
// ([`AggregatorTransport`]) with NO implementation in this repository. The
// transport is an implementation-time human gate (decision record 02 §6):
// no server has ever run, no device has ever been reached, no message has
// ever been exchanged.
//
// Design decisions recorded here:
//
// - **Deterministic store.** The resource store is a `BTreeMap` keyed by
//   the validated [`ResourceHref`] — the BTreeMap determinism kernel
//   pattern: same inputs, same store order, same responses.
// - **Fail-closed mTLS requirements.** [`MtlsRequirements`] has no field
//   that could disable client-certificate enforcement: requiring client
//   certs is STRUCTURAL (2030.5/CSIP mutual-TLS posture [S-1][S-3][S-7] —
//   security record 03 owns the CA-chain and SFDI/LFDI design). Constructing
//   the requirements with an empty trust-anchor set is a construction
//   error.
// - **Bounded lists.** Response and log-event collections have DECLARED
//   caps ([`ServerLimits`]); insertion beyond the cap drops the OLDEST
//   entry (the bounded-queue pattern). Unbounded device-facing lists are a
//   denial-of-service surface, not a feature.
// - **Declared address plan.** The server serves only paths its declared
//   [`AddressPlan`] names; anything else is a named refusal.
// - **Write surface.** The only write the server skeleton accepts is a
//   device Response (or log event) posted to its declared collection — the
//   read side of controls is device-initiated polling in 2030.5. A write to
//   any other collection is refused. This is NOT a command path: the
//   aggregator never writes controls into a device here; devices poll.
//
// UNVERIFIED-ASSUMPTION register (module-level; the model.rs discipline
// applies):
//
// - DNS-SD discovery: the default service-type string in
//   [`DnsSdAnnouncement::csip_default`] is transcribed from secondary
//   material [S-3][S-5] — UNVERIFIED (exact service type, TXT record
//   contents, and discovery semantics must be confirmed against the CSIP
//   Implementation Guide / standard text at implementation time).
// - The status vocabulary ([`ResponseStatus`]) is the design surface's
//   own; its mapping onto wire status codes is a transport-seam concern
//   and is UNVERIFIED against the standard's error-response semantics.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::addressing::{AddressError, AddressPlan, ResourceHref, ResourceQuery};
use crate::model::{
    DefaultDERControl, DERControlList, DERProgramList, EndDeviceList, FunctionSetAssignmentsList,
    LogEvent, LogEventList, PricingList, Response, ResponseList,
};
use flexcore_protocol_ports::manifest::ProtocolAdapterManifest;

/// Server-level errors. Every failure is named; nothing is silently
/// repaired.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerError {
    /// The manifest is not a manifest for THIS protocol adapter (wrong
    /// adapter id or protocol id) — a composition error, fail-closed.
    ManifestMismatch { expected: String, actual: String },
    /// The address plan is invalid.
    Address(AddressError),
    /// The declared server limits are degenerate (zero caps).
    InvalidLimits(&'static str),
    /// The mTLS requirements are degenerate (empty trust-anchor set).
    InvalidTlsRequirements,
    /// A resource href is not declared by the address plan.
    UndeclaredPath(ResourceHref),
    /// The request body does not decode into the expected resource type.
    InvalidBody { expected: &'static str },
    /// The value could not be encoded for the response body.
    Encoding(&'static str),
}

impl std::fmt::Display for ServerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServerError::ManifestMismatch { expected, actual } => {
                write!(f, "manifest mismatch: expected '{expected}', got '{actual}'")
            }
            ServerError::Address(error) => write!(f, "address plan invalid: {error}"),
            ServerError::InvalidLimits(field) => {
                write!(f, "server limits invalid: '{field}' must be non-zero")
            }
            ServerError::InvalidTlsRequirements => {
                write!(f, "mTLS requirements invalid: the trust-anchor set is empty")
            }
            ServerError::UndeclaredPath(href) => {
                write!(f, "path '{href}' is not declared by the address plan")
            }
            ServerError::InvalidBody { expected } => {
                write!(f, "request body does not decode into {expected}")
            }
            ServerError::Encoding(what) => write!(f, "response encoding failed for {what}"),
        }
    }
}

impl std::error::Error for ServerError {}

impl From<AddressError> for ServerError {
    fn from(error: AddressError) -> Self {
        ServerError::Address(error)
    }
}

/// The fail-closed mTLS requirements of the AggregatorServer (P14.02 §4;
/// the full security design — CA-chain validation, SFDI/LFDI handling,
/// certificate lifecycle, trust-anchor management — is decision record
/// 03_p14_03_security_design.md; NO CA is stood up in this program).
///
/// There is deliberately NO field to disable client-certificate
/// enforcement: the mTLS posture is structural. The trust-anchor entries
/// are LABELS referencing anchor material managed by the deployment's PKI
/// operations (Phase 13 secrets surface, Phase 18 device identity) — no key
/// material exists in this repository.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct MtlsRequirements {
    /// Labels of the trust anchors the server validates device
    /// certificates against (non-empty). Each label references material
    /// the deployment's PKI operations manage; the mapping label → anchor
    /// is a composition-root artifact.
    pub trust_anchors: Vec<String>,
    /// Whether certificate revocation is checked before a session is
    /// accepted. DEFAULT `true` (REGISTERED RECOMMENDATION, security
    /// record 03 §5): fail-closed means an unverifiable revocation status
    /// is a rejection, not a pass.
    pub check_revocation: bool,
}

impl MtlsRequirements {
    /// Constructs the requirements; the trust-anchor label set must be
    /// non-empty (an aggregator with no trust anchor accepts nothing —
    /// and MUST not silently accept everything instead).
    pub fn new(trust_anchors: Vec<String>, check_revocation: bool) -> Result<Self, ServerError> {
        let cleaned: Vec<String> = trust_anchors
            .into_iter()
            .filter(|label| !label.trim().is_empty())
            .collect();
        if cleaned.is_empty() {
            return Err(ServerError::InvalidTlsRequirements);
        }
        Ok(Self {
            trust_anchors: cleaned,
            check_revocation,
        })
    }

    /// Whether client certificates are required. STRUCTURAL: always true.
    pub const fn requires_client_certificates(&self) -> bool {
        true
    }
}

/// The declared collection caps (P14.02 §4: bounded lists). Zero caps are
/// construction errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ServerLimits {
    /// The maximum number of Response entries retained.
    pub response_list_cap: u32,
    /// The maximum number of LogEvent entries retained.
    pub log_event_list_cap: u32,
}

impl ServerLimits {
    /// Validates and constructs the limits.
    pub fn new(response_list_cap: u32, log_event_list_cap: u32) -> Result<Self, ServerError> {
        if response_list_cap == 0 {
            return Err(ServerError::InvalidLimits("response_list_cap"));
        }
        if log_event_list_cap == 0 {
            return Err(ServerError::InvalidLimits("log_event_list_cap"));
        }
        Ok(Self {
            response_list_cap,
            log_event_list_cap,
        })
    }
}

/// The DNS-SD discovery announcement the server deployment publishes
/// (P14.02 §3 notes). Carried as DECLARED data with an UNVERIFIED default
/// (module header): the service-type spelling below is transcribed from
/// secondary material [S-3][S-5] and must be confirmed against the CSIP
/// Implementation Guide at implementation time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct DnsSdAnnouncement {
    /// The DNS-SD service type string (UNVERIFIED default).
    pub service_type: String,
    /// The service instance name the deployment publishes.
    pub instance_name: String,
    /// The TLS port the AggregatorServer listens on.
    pub port: u16,
}

impl DnsSdAnnouncement {
    /// The REGISTERED RECOMMENDATION service naming (UNVERIFIED — module
    /// header). The instance name is deployment-chosen; the port is
    /// deployment-declared (no default port is assumed here).
    pub fn csip_default(instance_name: String, port: u16) -> Self {
        Self {
            service_type: "csip".to_string(),
            instance_name,
            port,
        }
    }
}

/// The request method the design surface carries (the transport seam maps
/// wire verbs onto this; the AggregatorServer skeleton handles GET reads
/// and bounded POST writes only).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequestMethod {
    /// A read (device poll or internal query).
    Get,
    /// A bounded write (a device posting a Response or LogEvent).
    Post,
}

/// The design-surface response status (module header: the wire mapping is
/// a transport-seam concern, UNVERIFIED).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResponseStatus {
    /// Read or write succeeded.
    Ok,
    /// A bounded write created a resource.
    Created,
    /// The request was malformed (bad body, zero limit).
    BadRequest,
    /// The href is not present in the store (or not declared by the plan —
    /// indistinguishable by design: the server does not reveal its plan
    /// through error differences).
    NotFound,
    /// The write targets a collection that accepts no writes.
    NotAllowed,
    /// An internal failure (encoding).
    InternalError,
}

impl ResponseStatus {
    /// The design-surface status code (a 200-family/400-family mapping for
    /// the design record; NOT a wire claim).
    pub const fn code(self) -> u16 {
        match self {
            ResponseStatus::Ok => 200,
            ResponseStatus::Created => 201,
            ResponseStatus::BadRequest => 400,
            ResponseStatus::NotFound => 404,
            ResponseStatus::NotAllowed => 405,
            ResponseStatus::InternalError => 500,
        }
    }
}

/// A request on the design surface (P14.02 §4): method, target href,
/// optional query, optional JSON body. The transport seam builds these
/// from wire input; the wire encoding (EXI/XML per the standard,
/// UNVERIFIED) never crosses this type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ResourceRequest {
    /// The method.
    pub method: RequestMethod,
    /// The target href.
    pub href: ResourceHref,
    /// The query, when the caller paged or scoped the request.
    pub query: Option<ResourceQuery>,
    /// The JSON body, when the method writes.
    pub body: Option<serde_json::Value>,
}

/// A response on the design surface: status plus an optional JSON body.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ResourceResponse {
    /// The status.
    pub status: ResponseStatus,
    /// The JSON body, when the request had one to return.
    pub body: Option<serde_json::Value>,
}

impl ResourceResponse {
    /// A body-less response.
    pub const fn status_only(status: ResponseStatus) -> Self {
        ResourceResponse {
            status,
            body: None,
        }
    }
}

/// A stored resource (the server store's value type): every CSIP
/// collection the declared plan serves, plus the singular resources.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StoredResource {
    /// The enrolled device list.
    EndDeviceList(EndDeviceList),
    /// The function-set assignments list.
    FunctionSetAssignmentsList(FunctionSetAssignmentsList),
    /// The DER program list.
    DERProgramList(DERProgramList),
    /// The DER control list.
    DERControlList(DERControlList),
    /// The singular default control of a program.
    DefaultDERControl(DefaultDERControl),
    /// The response list.
    ResponseList(ResponseList),
    /// The pricing list.
    PricingList(PricingList),
    /// The log-event list.
    LogEventList(LogEventList),
}

impl StoredResource {
    /// The list metadata of a list resource (for paging); `None` for the
    /// singular resource.
    fn entry_count(&self) -> Option<usize> {
        match self {
            StoredResource::EndDeviceList(list) => Some(list.end_devices.len()),
            StoredResource::FunctionSetAssignmentsList(list) => {
                Some(list.function_set_assignments.len())
            }
            StoredResource::DERProgramList(list) => Some(list.der_programs.len()),
            StoredResource::DERControlList(list) => Some(list.der_controls.len()),
            StoredResource::ResponseList(list) => Some(list.responses.len()),
            StoredResource::PricingList(list) => Some(list.pricings.len()),
            StoredResource::LogEventList(list) => Some(list.log_events.len()),
            StoredResource::DefaultDERControl(_) => None,
        }
    }

    /// Applies the query's paging window to a list resource (P14.02 §3):
    /// entries are sliced and the metadata recount is exact. The singular
    /// resource is returned unchanged.
    fn paged(self, query: &ResourceQuery) -> Self {
        let count = match self.entry_count() {
            Some(count) => count,
            None => return self,
        };
        let (start, end) = query.paging_window(count);
        match self {
            StoredResource::EndDeviceList(mut list) => {
                list.end_devices = list.end_devices[start..end].to_vec();
                list.meta.all = count as u32;
                list.meta.results = (end - start) as u32;
                StoredResource::EndDeviceList(list)
            }
            StoredResource::FunctionSetAssignmentsList(mut list) => {
                list.function_set_assignments = list.function_set_assignments[start..end].to_vec();
                list.meta.all = count as u32;
                list.meta.results = (end - start) as u32;
                StoredResource::FunctionSetAssignmentsList(list)
            }
            StoredResource::DERProgramList(mut list) => {
                list.der_programs = list.der_programs[start..end].to_vec();
                list.meta.all = count as u32;
                list.meta.results = (end - start) as u32;
                StoredResource::DERProgramList(list)
            }
            StoredResource::DERControlList(mut list) => {
                list.der_controls = list.der_controls[start..end].to_vec();
                list.meta.all = count as u32;
                list.meta.results = (end - start) as u32;
                StoredResource::DERControlList(list)
            }
            StoredResource::ResponseList(mut list) => {
                list.responses = list.responses[start..end].to_vec();
                list.meta.all = count as u32;
                list.meta.results = (end - start) as u32;
                StoredResource::ResponseList(list)
            }
            StoredResource::PricingList(mut list) => {
                list.pricings = list.pricings[start..end].to_vec();
                list.meta.all = count as u32;
                list.meta.results = (end - start) as u32;
                StoredResource::PricingList(list)
            }
            StoredResource::LogEventList(mut list) => {
                list.log_events = list.log_events[start..end].to_vec();
                list.meta.all = count as u32;
                list.meta.results = (end - start) as u32;
                StoredResource::LogEventList(list)
            }
            StoredResource::DefaultDERControl(singular) => StoredResource::DefaultDERControl(singular),
        }
    }
}

/// The request handler the transport seam calls (P14.02 §4). The
/// AggregatorServer implements it; a transport implementation (NONE exists
/// in this repository — the seam is UNRUN by design) calls it per wire
/// request.
pub trait ResourceRequestHandler {
    /// Handles one request on the design surface.
    fn handle(&mut self, request: &ResourceRequest) -> ResourceResponse;
}

/// The transport seam (P14.02 §4; decision record 02 §6). **UNRUN BY
/// DESIGN — no implementation ships in this repository.** The mTLS/HTTP
/// transport is an implementation-time human gate: it must bind the TLS
/// listener with [`MtlsRequirements`] (client certificates structurally
/// required), enforce the declared [`ServerLimits`] at connection scale,
/// decode wire resources into [`ResourceRequest`]s, and encode
/// [`ResourceResponse`]s back. The wire encoding (EXI per the standard —
/// UNVERIFIED) and the HTTP server framework choice are recorded as open
/// implementation decisions (conventions §4 E-decision discipline; the
/// dependency line admits no HTTP framework in this batch).
pub trait AggregatorTransport: Send {
    /// Binds and serves until the transport closes. Returns a named
    /// [`TransportError`] on failure — a transport never reports success
    /// for a bind it did not perform.
    fn serve(
        self: Box<Self>,
        handler: &mut dyn ResourceRequestHandler,
    ) -> Result<(), TransportError>;
}

/// Transport-level errors (named; the transport seam reports these, never
/// a silent close).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportError {
    /// The listener could not bind.
    BindFailed { reason: String },
    /// A TLS handshake failed (including a rejected client certificate —
    /// the fail-closed mTLS posture).
    TlsHandshakeFailed { reason: String },
    /// The transport closed before the server requested it.
    TransportClosed { reason: String },
}

impl std::fmt::Display for TransportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransportError::BindFailed { reason } => write!(f, "bind failed: {reason}"),
            TransportError::TlsHandshakeFailed { reason } => {
                write!(f, "TLS handshake failed: {reason}")
            }
            TransportError::TransportClosed { reason } => {
                write!(f, "transport closed: {reason}")
            }
        }
    }
}

impl std::error::Error for TransportError {}

/// The AggregatorServer skeleton (P14.02 §4): the deterministic resource
/// store plus the GET/POST routing on the design surface. Construction is
/// fail-closed: a valid manifest for THIS adapter, a valid declared
/// address plan, and non-zero limits.
pub struct AggregatorServer {
    /// The validated manifest (held by value — the P14.01 discipline).
    manifest: ProtocolAdapterManifest,
    /// The declared address plan.
    plan: AddressPlan,
    /// The declared collection caps.
    limits: ServerLimits,
    /// The mTLS requirements (client certificates structurally required).
    tls: MtlsRequirements,
    /// The deterministic resource store, keyed by validated href.
    store: BTreeMap<ResourceHref, StoredResource>,
}

impl AggregatorServer {
    /// Constructs the server. Fails closed on a manifest for a different
    /// adapter/protocol, an invalid plan, degenerate limits, or an empty
    /// trust-anchor set.
    pub fn new(
        manifest: ProtocolAdapterManifest,
        plan: AddressPlan,
        limits: ServerLimits,
        tls: MtlsRequirements,
    ) -> Result<Self, ServerError> {
        if manifest.adapter_id != crate::ADAPTER_ID || manifest.protocol_id != crate::PROTOCOL_ID {
            return Err(ServerError::ManifestMismatch {
                expected: format!("{}/{}", crate::ADAPTER_ID, crate::PROTOCOL_ID),
                actual: format!("{}/{}", manifest.adapter_id, manifest.protocol_id),
            });
        }
        plan.validate()?;
        // Defense in depth: the limits and TLS fields are public, so a
        // caller could bypass the checked constructors with a struct
        // literal. The server re-validates both fail-closed.
        if limits.response_list_cap == 0 {
            return Err(ServerError::InvalidLimits("response_list_cap"));
        }
        if limits.log_event_list_cap == 0 {
            return Err(ServerError::InvalidLimits("log_event_list_cap"));
        }
        if tls.trust_anchors.iter().any(|label| label.trim().is_empty())
            || tls.trust_anchors.is_empty()
        {
            return Err(ServerError::InvalidTlsRequirements);
        }
        let _ = tls.requires_client_certificates();
        Ok(Self {
            manifest,
            plan,
            limits,
            tls,
            store: BTreeMap::new(),
        })
    }

    /// The validated manifest.
    pub fn manifest(&self) -> &ProtocolAdapterManifest {
        &self.manifest
    }

    /// The declared address plan.
    pub fn plan(&self) -> &AddressPlan {
        &self.plan
    }

    /// The mTLS requirements.
    pub fn tls_requirements(&self) -> &MtlsRequirements {
        &self.tls
    }

    /// Installs a resource at a declared href. Refuses undeclared paths —
    /// the store cannot grow outside the plan.
    pub fn install(&mut self, href: ResourceHref, resource: StoredResource) -> Result<(), ServerError> {
        if !self.plan.is_declared(&href) {
            return Err(ServerError::UndeclaredPath(href));
        }
        self.store.insert(href, resource);
        Ok(())
    }

    /// Records one device Response into the declared ResponseList,
    /// enforcing the declared cap (oldest dropped). The convenience the
    /// POST route and the tests share.
    pub fn record_response(&mut self, response: Response) -> Result<(), ServerError> {
        let href = self.plan.response_list.clone();
        let mut list = match self.store.remove(&href) {
            Some(StoredResource::ResponseList(list)) => list,
            Some(_) => return Err(ServerError::InvalidBody { expected: "ResponseList" }),
            None => ResponseList {
                meta: crate::model::ListMeta::unpaged(0),
                poll_rate: None,
                responses: Vec::new(),
            },
        };
        list.responses.push(response);
        while list.responses.len() > self.limits.response_list_cap as usize {
            list.responses.remove(0);
        }
        list.meta = crate::model::ListMeta::unpaged(list.responses.len() as u32);
        self.store.insert(href, StoredResource::ResponseList(list));
        Ok(())
    }

    /// Records one LogEvent into the declared LogEventList, enforcing the
    /// declared cap (oldest dropped).
    pub fn record_log_event(&mut self, event: LogEvent) -> Result<(), ServerError> {
        let href = self.plan.log_event_list.clone();
        let mut list = match self.store.remove(&href) {
            Some(StoredResource::LogEventList(list)) => list,
            Some(_) => return Err(ServerError::InvalidBody { expected: "LogEventList" }),
            None => LogEventList {
                meta: crate::model::ListMeta::unpaged(0),
                poll_rate: None,
                log_events: Vec::new(),
            },
        };
        list.log_events.push(event);
        while list.log_events.len() > self.limits.log_event_list_cap as usize {
            list.log_events.remove(0);
        }
        list.meta = crate::model::ListMeta::unpaged(list.log_events.len() as u32);
        self.store.insert(href, StoredResource::LogEventList(list));
        Ok(())
    }

    /// The store snapshot as sorted (href, resource) pairs — deterministic
    /// diagnostics for the composition root and the tests.
    pub fn store_snapshot(&self) -> Vec<(&str, &StoredResource)> {
        self.store
            .iter()
            .map(|(href, resource)| (href.as_str(), resource))
            .collect()
    }

    fn handle_get(&self, request: &ResourceRequest) -> ResourceResponse {
        let href = &request.href;
        if !self.plan.is_declared(href) {
            return ResourceResponse::status_only(ResponseStatus::NotFound);
        }
        // A query with a zero limit is a caller error, named.
        if let Some(query) = &request.query {
            if query.validate().is_err() {
                return ResourceResponse::status_only(ResponseStatus::BadRequest);
            }
        }
        let Some(resource) = self.store.get(href) else {
            return ResourceResponse::status_only(ResponseStatus::NotFound);
        };
        let served = match &request.query {
            Some(query) => resource.clone().paged(query),
            None => resource.clone(),
        };
        // The wire body is the RESOURCE, not the store's enum wrapper:
        // serializing `StoredResource` directly would emit the externally
        // tagged form (`{"der_control_list": {...}}`), so `body["meta"]`
        // would be absent and every list consumer would see an empty
        // envelope. Each variant serializes its inner resource instead.
        let body = match &served {
            StoredResource::EndDeviceList(list) => serde_json::to_value(list),
            StoredResource::FunctionSetAssignmentsList(list) => serde_json::to_value(list),
            StoredResource::DERProgramList(list) => serde_json::to_value(list),
            StoredResource::DERControlList(list) => serde_json::to_value(list),
            StoredResource::DefaultDERControl(control) => serde_json::to_value(control),
            StoredResource::ResponseList(list) => serde_json::to_value(list),
            StoredResource::PricingList(list) => serde_json::to_value(list),
            StoredResource::LogEventList(list) => serde_json::to_value(list),
        };
        match body {
            Ok(body) => ResourceResponse {
                status: ResponseStatus::Ok,
                body: Some(body),
            },
            Err(_) => ResourceResponse::status_only(ResponseStatus::InternalError),
        }
    }

    fn handle_post(&mut self, request: &ResourceRequest) -> ResourceResponse {
        let href = &request.href;
        if !self.plan.is_declared(href) {
            return ResourceResponse::status_only(ResponseStatus::NotFound);
        }
        // The bounded write surface: responses and log events only.
        if *href == self.plan.response_list {
            let Some(body) = &request.body else {
                return ResourceResponse::status_only(ResponseStatus::BadRequest);
            };
            let Ok(response) = serde_json::from_value::<Response>(body.clone()) else {
                return ResourceResponse::status_only(ResponseStatus::BadRequest);
            };
            match self.record_response(response) {
                Ok(()) => ResourceResponse::status_only(ResponseStatus::Created),
                Err(_) => ResourceResponse::status_only(ResponseStatus::InternalError),
            }
        } else if *href == self.plan.log_event_list {
            let Some(body) = &request.body else {
                return ResourceResponse::status_only(ResponseStatus::BadRequest);
            };
            let Ok(event) = serde_json::from_value::<LogEvent>(body.clone()) else {
                return ResourceResponse::status_only(ResponseStatus::BadRequest);
            };
            match self.record_log_event(event) {
                Ok(()) => ResourceResponse::status_only(ResponseStatus::Created),
                Err(_) => ResourceResponse::status_only(ResponseStatus::InternalError),
            }
        } else {
            ResourceResponse::status_only(ResponseStatus::NotAllowed)
        }
    }
}

impl ResourceRequestHandler for AggregatorServer {
    fn handle(&mut self, request: &ResourceRequest) -> ResourceResponse {
        match request.method {
            RequestMethod::Get => self.handle_get(request),
            RequestMethod::Post => self.handle_post(request),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::addressing::{AddressPlan, ResourceQuery};
    use crate::model::{ListMeta, TimeType};
    use flexcore_protocol_ports::manifest::ProtocolAdapterManifest;
    // The reviewed scope/manifest spelling lives in crate::scope
    // (FRV1-R6-004, final review R6): the test helper delegates there —
    // one reviewed spelling, no drift.

    fn manifest() -> ProtocolAdapterManifest {
        crate::scope::aggregator_manifest(crate::ADAPTER_ID).expect("valid manifest")
    }

    fn limits() -> ServerLimits {
        ServerLimits::new(8, 8).expect("valid limits")
    }

    fn tls() -> MtlsRequirements {
        MtlsRequirements::new(vec!["csip-device-ca".to_string()], true)
            .expect("valid tls requirements")
    }

    fn server() -> AggregatorServer {
        AggregatorServer::new(manifest(), AddressPlan::csip_default(), limits(), tls())
            .expect("valid server")
    }

    fn href(path: &str) -> ResourceHref {
        ResourceHref::new(path.to_string()).expect("valid href")
    }

    fn control_list_with(n: usize) -> DERControlList {
        let mut controls = Vec::new();
        for i in 0..n {
            controls.push(
                crate::model::DERControl::new(
                    format!("ctrl-{i}"),
                    Some(TimeType(1)),
                    Some(TimeType(1000 + i as i64)),
                    Some(300),
                    None,
                    None,
                    crate::model::DERControlBase {
                        op_mod_gen_lim_w: Some(1000.0 + i as f64),
                        ..crate::model::DERControlBase::default()
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

    #[test]
    fn construction_is_fail_closed() {
        // Wrong adapter id.
        let wrong_manifest = {
            let mut m = manifest();
            m.adapter_id = "other".to_string();
            m
        };
        // AggregatorServer carries no Debug (server state), so the
        // refusal is extracted with a match, not expect_err.
        let error = match AggregatorServer::new(
            wrong_manifest,
            AddressPlan::csip_default(),
            limits(),
            tls(),
        ) {
            Ok(_) => panic!("a mismatched manifest is refused"),
            Err(error) => error,
        };
        assert_eq!(
            error,
            ServerError::ManifestMismatch {
                expected: "ieee-2030.5/ieee-2030.5".to_string(),
                actual: "other/ieee-2030.5".to_string(),
            }
        );
        // Empty trust anchors.
        assert_eq!(
            MtlsRequirements::new(Vec::new(), true),
            Err(ServerError::InvalidTlsRequirements)
        );
        // Zero caps.
        assert_eq!(
            ServerLimits::new(0, 8),
            Err(ServerError::InvalidLimits("response_list_cap"))
        );
        assert_eq!(
            ServerLimits::new(8, 0),
            Err(ServerError::InvalidLimits("log_event_list_cap"))
        );
        // Client certificates are structurally required.
        assert!(tls().requires_client_certificates());
    }

    #[test]
    fn install_refuses_undeclared_paths() {
        let mut server = server();
        let undeclared = href("/not-in-plan");
        assert_eq!(
            server.install(
                undeclared.clone(),
                StoredResource::DefaultDERControl(DefaultDERControl::default())
            ),
            Err(ServerError::UndeclaredPath(undeclared))
        );
        // A declared path installs.
        let plan = AddressPlan::csip_default();
        server
            .install(
                plan.der_control_list.clone(),
                StoredResource::DERControlList(control_list_with(3)),
            )
            .expect("declared path installs");
        assert_eq!(server.store_snapshot().len(), 1);
    }

    #[test]
    fn get_serves_paged_lists_and_missing_paths() {
        let mut server = server();
        let plan = AddressPlan::csip_default();
        server
            .install(
                plan.der_control_list.clone(),
                StoredResource::DERControlList(control_list_with(5)),
            )
            .expect("install");

        // Unpaged GET.
        let response = server.handle(&ResourceRequest {
            method: RequestMethod::Get,
            href: plan.der_control_list.clone(),
            query: None,
            body: None,
        });
        assert_eq!(response.status, ResponseStatus::Ok);
        let body = response.body.expect("body present");
        assert_eq!(body["meta"]["all"], 5);
        assert_eq!(body["meta"]["results"], 5);

        // Paged GET (limit 2, offset 1).
        let response = server.handle(&ResourceRequest {
            method: RequestMethod::Get,
            href: plan.der_control_list.clone(),
            query: Some(ResourceQuery {
                href: plan.der_control_list.clone(),
                sfdi: None,
                limit: Some(2),
                offset: Some(1),
            }),
            body: None,
        });
        assert_eq!(response.status, ResponseStatus::Ok);
        let body = response.body.expect("body present");
        assert_eq!(body["meta"]["all"], 5);
        assert_eq!(body["meta"]["results"], 2);
        assert_eq!(body["derControls"][0]["mrid"], "ctrl-1");
        assert_eq!(body["derControls"][1]["mrid"], "ctrl-2");

        // Undeclared path is indistinguishable from a missing one (404).
        let response = server.handle(&ResourceRequest {
            method: RequestMethod::Get,
            href: href("/not-in-plan"),
            query: None,
            body: None,
        });
        assert_eq!(response.status, ResponseStatus::NotFound);
        // Declared but not installed is also 404.
        let response = server.handle(&ResourceRequest {
            method: RequestMethod::Get,
            href: plan.pricing_list.clone(),
            query: None,
            body: None,
        });
        assert_eq!(response.status, ResponseStatus::NotFound);

        // Zero-limit query is a bad request.
        let response = server.handle(&ResourceRequest {
            method: RequestMethod::Get,
            href: plan.der_control_list.clone(),
            query: Some(ResourceQuery {
                href: plan.der_control_list.clone(),
                sfdi: None,
                limit: Some(0),
                offset: None,
            }),
            body: None,
        });
        assert_eq!(response.status, ResponseStatus::BadRequest);
    }

    #[test]
    fn post_accepts_bounded_responses_and_refuses_other_collections() {
        let mut server = server();
        let plan = AddressPlan::csip_default();
        let response_payload = Response {
            href: None,
            end_device_lfdi: Some(
                crate::model::Lfdi::new("0a1b".to_string()).expect("valid lfdi"),
            ),
            created_date_time: Some(TimeType(10)),
            status: Some(1),
            subject: Some(href("/derc/ctrl-0")),
        };
        let body = serde_json::to_value(&response_payload).expect("serializes");

        // POST a response to the declared response list.
        let response = server.handle(&ResourceRequest {
            method: RequestMethod::Post,
            href: plan.response_list.clone(),
            query: None,
            body: Some(body.clone()),
        });
        assert_eq!(response.status, ResponseStatus::Created);
        // POST the same again (cap 8 is not reached by 2).
        let _ = server.handle(&ResourceRequest {
            method: RequestMethod::Post,
            href: plan.response_list.clone(),
            query: None,
            body: Some(body),
        });
        // The list reflects both.
        let response = server.handle(&ResourceRequest {
            method: RequestMethod::Get,
            href: plan.response_list.clone(),
            query: None,
            body: None,
        });
        assert_eq!(response.body.as_ref().expect("body")["meta"]["all"], 2);

        // A POST to the control list is refused (NotAllowed), never a
        // silent write of a control into the platform's own tree.
        let response = server.handle(&ResourceRequest {
            method: RequestMethod::Post,
            href: plan.der_control_list.clone(),
            query: None,
            body: Some(serde_json::json!({"mrid": "x"})),
        });
        assert_eq!(response.status, ResponseStatus::NotAllowed);
    }

    #[test]
    fn response_cap_drops_oldest() {
        let mut server = AggregatorServer::new(
            manifest(),
            AddressPlan::csip_default(),
            ServerLimits::new(3, 3).expect("valid limits"),
            tls(),
        )
        .expect("valid server");
        for i in 0..5u8 {
            server
                .record_response(Response {
                    href: None,
                    end_device_lfdi: None,
                    created_date_time: Some(TimeType(i as i64)),
                    status: Some(i),
                    subject: None,
                })
                .expect("recorded");
        }
        let plan = AddressPlan::csip_default();
        let response = server.handle(&ResourceRequest {
            method: RequestMethod::Get,
            href: plan.response_list.clone(),
            query: None,
            body: None,
        });
        let body = response.body.expect("body");
        assert_eq!(body["meta"]["all"], 3, "cap enforced");
        // The two oldest (0 and 1) were dropped; statuses 2, 3, 4 remain.
        let statuses: Vec<u8> = body["responses"]
            .as_array()
            .expect("array")
            .iter()
            .map(|r| r["status"].as_u64().expect("status") as u8)
            .collect();
        assert_eq!(statuses, vec![2, 3, 4]);
    }

    #[test]
    fn log_events_record_bounded_and_serve() {
        let mut server = AggregatorServer::new(
            manifest(),
            AddressPlan::csip_default(),
            ServerLimits::new(4, 2).expect("valid limits"),
            tls(),
        )
        .expect("valid server");
        for i in 0..3u8 {
            server
                .record_log_event(
                    LogEvent::new(
                        Some(TimeType(i as i64)),
                        format!("event-{i}"),
                        None,
                        None,
                    )
                    .expect("valid event"),
                )
                .expect("recorded");
        }
        let plan = AddressPlan::csip_default();
        let response = server.handle(&ResourceRequest {
            method: RequestMethod::Get,
            href: plan.log_event_list.clone(),
            query: None,
            body: None,
        });
        let body = response.body.expect("body");
        assert_eq!(body["meta"]["all"], 2, "cap enforced");
        let details: Vec<String> = body["logEvents"]
            .as_array()
            .expect("array")
            .iter()
            .map(|e| e["details"].as_str().expect("details").to_string())
            .collect();
        assert_eq!(details, vec!["event-1".to_string(), "event-2".to_string()]);
    }
}
