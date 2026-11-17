// P14.17 — The simulated IEEE 2030.5 device client and the in-process
// server transport (decision record 81; the seam is
// `flexcore_protocol_20305::server::{AggregatorTransport,
// ResourceRequestHandler}` — UNRUN by design, record 02 §5).
//
// Two simulated peers, one file:
//
// - [`Simulated20305Device`]: the DEVICE side of 2030.5/CSIP. 2030.5 is
//   device-initiated polling: the device walks the aggregator's CSIP
//   tree (GET on the declared collections) and posts device Responses /
//   LogEvents to the declared collections (the server skeleton's only
//   bounded write surface, record 02 §4). The simulated device drives
//   the [`ResourceRequestHandler`] design surface directly — no socket,
//   no TLS, no EXI encoding (the wire encoding is a transport-seam
//   concern and stays UNVERIFIED).
// - [`SimulatedAggregatorTransport`]: an in-process implementation of
//   the [`AggregatorTransport`] SEAM (the server-side serving trait that
//   has NO implementation in the adapter crate by design). It serves a
//   SCRIPTED schedule of design-surface requests through the handler —
//   the mTLS bind it "performs" is the declared [`MtlsRequirements`]
//   check only: no listener exists, no handshake occurs, and the
//   transport records that honestly in its identity strings.
//
// Both are deterministic: the device's poll schedule is a fixed cycle
// over the declared address plan; the transport serves its scripted
// requests in declared order. Every identity string carries the
// `sim-20305` prefix — never presentable as a real device or server.

use std::sync::{Arc, Mutex};

use flexcore_protocol_20305::addressing::AddressPlan;
use flexcore_protocol_20305::model::{LogEvent, Response, TimeType};
use flexcore_protocol_20305::server::{
    AggregatorTransport, MtlsRequirements, ResourceRequest, ResourceRequestHandler,
    ResourceResponse, ResponseStatus, RequestMethod, TransportError,
};

/// The identity prefix every simulated 2030.5 peer carries in its
/// identity strings (never presentable as a real device/server).
pub const SIM_20305_ID_PREFIX: &str = "sim-20305";

/// Errors the simulated 2030.5 peers report. Every failure is named.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Sim20305Error {
    /// The device's poll schedule is empty — a device that never polls
    /// represents nothing (construction error).
    EmptySchedule,
    /// The scripted transport schedule is empty — a server transport
    /// that never serves represents nothing (construction error).
    EmptyScript,
    /// The device was handed a response whose body it expected but did
    /// not receive (the poll returned a body-less response).
    UnexpectedResponse {
        expected: &'static str,
        got_status: u16,
    },
    /// The mTLS requirements handed to the simulated transport are
    /// degenerate (re-validated here so a broken composition surfaces at
    /// the simulated seam exactly where the real seam would refuse).
    InvalidTlsRequirements,
    /// The adapter's model layer refused a simulated LogEvent (the model
    /// constructor's own validation; the refusal is re-raised, never
    /// swallowed).
    InvalidEvent,
}

impl std::fmt::Display for Sim20305Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Sim20305Error::EmptySchedule => {
                write!(f, "the simulated 2030.5 device declares an empty poll schedule")
            }
            Sim20305Error::EmptyScript => {
                write!(f, "the simulated 2030.5 transport declares an empty serve script")
            }
            Sim20305Error::UnexpectedResponse { expected, got_status } => write!(
                f,
                "the simulated device expected a {expected} body but got status {got_status}"
            ),
            Sim20305Error::InvalidTlsRequirements => {
                write!(f, "the simulated transport was handed degenerate mTLS requirements")
            }
            Sim20305Error::InvalidEvent => {
                write!(f, "the adapter's model layer refused the simulated LogEvent")
            }
        }
    }
}

impl std::error::Error for Sim20305Error {}

/// The deterministic poll schedule of the simulated device: a fixed
/// cycle over the declared address plan's collection hrefs. Tick `t`
/// polls `plan_hrefs[t % len]` — fully determined, no hidden state.
#[derive(Debug, Clone)]
pub struct Simulated20305Device {
    /// The device's identity string (carries the `sim-20305` prefix).
    pub device_id: String,
    /// The declared address plan the device polls (the same plan the
    /// server was constructed with — a device polling an undeclared path
    /// is a composition error, and the server would 404 it).
    plan: AddressPlan,
    /// The collection hrefs the device cycles over (from the plan, in
    /// plan-declaration order).
    schedule: Vec<flexcore_protocol_20305::addressing::ResourceHref>,
    tick: u64,
    /// The device's POSTed responses, in post order (the device's own
    /// record of what it told the aggregator).
    posted_responses: Vec<Response>,
}

impl Simulated20305Device {
    /// Constructs the simulated device over a declared address plan. The
    /// poll schedule is the plan's eight declared collections, cycled in
    /// declaration order.
    pub fn new(device_id: impl Into<String>, plan: AddressPlan) -> Self {
        Self {
            device_id: format!("{SIM_20305_ID_PREFIX}:{}", device_id.into()),
            schedule: vec![
                plan.end_device_list.clone(),
                plan.function_set_assignments_list.clone(),
                plan.der_program_list.clone(),
                plan.der_control_list.clone(),
                plan.default_der_control.clone(),
                plan.response_list.clone(),
                plan.pricing_list.clone(),
                plan.log_event_list.clone(),
            ],
            plan,
            tick: 0,
            posted_responses: Vec::new(),
        }
    }

    /// The next GET poll of the device's schedule: issues the request on
    /// the design surface through the handler and returns the response.
    /// Deterministic: tick `t` polls schedule entry `t % 8`.
    pub fn poll_once(&mut self, server: &mut dyn ResourceRequestHandler) -> ResourceResponse {
        let href = self.schedule[(self.tick as usize) % self.schedule.len()].clone();
        let request = ResourceRequest { method: RequestMethod::Get, href, query: None, body: None };
        self.tick += 1;
        server.handle(&request)
    }

    /// The device's current tick (deterministic schedule position).
    pub fn tick(&self) -> u64 {
        self.tick
    }

    /// Posts one device Response to the declared ResponseList through
    /// the bounded POST route (record 02 §4: the server's only write
    /// surface), recording it in the device's own posted-response log.
    /// The `created_at` instant is a PARAMETER (the injected-instant
    /// rule — the simulated device never reads a wall clock).
    pub fn post_response(
        &mut self,
        server: &mut dyn ResourceRequestHandler,
        status: u8,
        subject: Option<flexcore_protocol_20305::addressing::ResourceHref>,
        created_at: TimeType,
        body: serde_json::Value,
    ) -> ResourceResponse {
        let response = Response {
            href: None,
            end_device_lfdi: None,
            created_date_time: Some(created_at),
            status: Some(status),
            subject,
        };
        self.posted_responses.push(response.clone());
        let request = ResourceRequest {
            method: RequestMethod::Post,
            href: self.plan.response_list.clone(),
            query: None,
            body: Some(body),
        };
        server.handle(&request)
    }

    /// The device's posted responses, in post order.
    pub fn posted_responses(&self) -> &[Response] {
        &self.posted_responses
    }
}

/// The in-process implementation of the server-side
/// [`AggregatorTransport`] SEAM (no implementation ships in the adapter
/// crate by design — record 02 §5). It serves a SCRIPTED schedule of
/// design-surface requests through the handler, in declared order, and
/// records every response into a shared log the caller can inspect after
/// [`AggregatorTransport::serve`] returns.
///
/// What "serving" means here, honestly: NO listener is bound, NO TLS
/// handshake occurs, NO wire byte is decoded. The only security posture
/// the simulated transport exercises is the declared
/// [`MtlsRequirements`] validation (an empty trust-anchor set is refused
/// at serve time — the same fail-closed rule the real seam would
/// enforce). The wire encoding (EXI per the standard — UNVERIFIED) and
/// the HTTP framework choice remain open implementation decisions.
pub struct SimulatedAggregatorTransport {
    /// The transport's identity string (carries the `sim-20305` prefix).
    pub transport_id: String,
    /// The mTLS requirements the composition validated (held; validated
    /// again at serve time — zero-cap defence in depth).
    tls: MtlsRequirements,
    /// The scripted request schedule, in declared order.
    script: Vec<ResourceRequest>,
    /// The shared response log (filled by `serve`; inspectable via
    /// [`SimulatedAggregatorTransport::response_log`]).
    response_log: Arc<Mutex<Vec<ResourceResponse>>>,
}

impl SimulatedAggregatorTransport {
    /// Constructs the simulated transport over a scripted schedule.
    /// Validates the mTLS requirements at construction (fail-fast, the
    /// same rule as the server skeleton).
    pub fn new(
        transport_id: impl Into<String>,
        tls: MtlsRequirements,
        script: Vec<ResourceRequest>,
    ) -> Result<Self, Sim20305Error> {
        if script.is_empty() {
            return Err(Sim20305Error::EmptyScript);
        }
        if tls.trust_anchors.is_empty() || tls.trust_anchors.iter().any(|l| l.trim().is_empty()) {
            return Err(Sim20305Error::InvalidTlsRequirements);
        }
        Ok(Self {
            transport_id: format!("{SIM_20305_ID_PREFIX}:{}", transport_id.into()),
            tls,
            script,
            response_log: Arc::new(Mutex::new(Vec::new())),
        })
    }

    /// The response log handle (shared with `serve`; the caller keeps
    /// this handle and inspects it after serving).
    pub fn response_log(&self) -> Arc<Mutex<Vec<ResourceResponse>>> {
        Arc::clone(&self.response_log)
    }
}

impl AggregatorTransport for SimulatedAggregatorTransport {
    /// Serves the scripted schedule through the handler (in-process; no
    /// listener, no handshake). Returns
    /// [`TransportError::TransportClosed`] only if the response log was
    /// poisoned (an invariant violation), never a fabricated success —
    /// a completed scripted serve IS the success condition, and the
    /// response log is the evidence.
    fn serve(
        mut self: Box<Self>,
        handler: &mut dyn ResourceRequestHandler,
    ) -> Result<(), TransportError> {
        // The declared security posture, re-checked at serve time.
        if self.tls.trust_anchors.is_empty() {
            return Err(TransportError::TlsHandshakeFailed {
                reason: "the simulated transport refuses to serve with an empty trust-anchor set (fail-closed, the real seam's rule)".to_string(),
            });
        }
        let script = std::mem::take(&mut self.script);
        let mut served = Vec::with_capacity(script.len());
        for request in &script {
            served.push(handler.handle(request));
        }
        let mut log = self
            .response_log
            .lock()
            .map_err(|_| TransportError::TransportClosed {
                reason: "the simulated transport's response log mutex was poisoned".to_string(),
            })?;
        *log = served;
        Ok(())
    }
}

/// Convenience: whether a response carries an `Ok` status with a body
/// (the shape a poll of an installed resource returns).
pub fn is_ok_with_body(response: &ResourceResponse) -> bool {
    response.status == ResponseStatus::Ok && response.body.is_some()
}

/// Convenience: builds one scripted GET request for a plan href.
pub fn scripted_get(href: flexcore_protocol_20305::addressing::ResourceHref) -> ResourceRequest {
    ResourceRequest { method: RequestMethod::Get, href, query: None, body: None }
}

/// Convenience: builds one LogEvent for the simulated device to post
/// (the event `details` string carries the simulated-device prefix so a
/// recorded event is identifiable as simulated from its content alone).
pub fn simulated_log_event(details: String, created_at: TimeType) -> Result<LogEvent, Sim20305Error> {
    if details.trim().is_empty() {
        return Err(Sim20305Error::EmptySchedule);
    }
    LogEvent::new(Some(created_at), format!("{SIM_20305_ID_PREFIX}: {details}"), None, None)
        .map_err(|_| Sim20305Error::InvalidEvent)
}

#[cfg(test)]
mod tests {
    use super::*;
    use flexcore_protocol_20305::model::{DERControl, DERControlBase, DERControlList, ListMeta};
    use flexcore_protocol_20305::server::{
        AggregatorServer, ServerLimits, StoredResource,
    };
    use flexcore_protocol_ports::manifest::{DeclaredCapability, ProtocolAdapterManifest};
    use flexcore_protocol_ports::scope::{
        ControlScope, NotModelled, ProtocolRole, ProtocolScopeDeclaration, ResourceClass,
    };
    use std::time::Duration;

    fn manifest() -> ProtocolAdapterManifest {
        // The SAME reviewed manifest shape the adapter crate's own tests
        // construct (record 02 §2) — the simulated counterpart cannot
        // gain authority its real counterpart lacks.
        let scope = ProtocolScopeDeclaration::new(
            flexcore_protocol_20305::ADAPTER_ID.to_string(),
            flexcore_protocol_20305::PROTOCOL_ID.to_string(),
            vec![ProtocolRole::Server],
            ControlScope::MonitorOnly,
            vec![ResourceClass::Control, ResourceClass::Discovery],
            vec![
                NotModelled::new(
                    "no_pki_operations".to_string(),
                    "no CA operations — PKI ties to Phase 13/18 (security record 03)"
                        .to_string(),
                )
                .expect("valid"),
                NotModelled::new(
                    "no_field_write_authority".to_string(),
                    "the aggregator never writes controls into a device; devices poll"
                        .to_string(),
                )
                .expect("valid"),
            ],
            "external control objects enter the command lifecycle at PROPOSED; this \
             adapter is NOT a dispatch authority"
                .to_string(),
            Vec::new(),
        )
        .expect("valid scope");
        ProtocolAdapterManifest::new(
            flexcore_protocol_20305::ADAPTER_ID.to_string(),
            flexcore_protocol_20305::PROTOCOL_ID.to_string(),
            flexcore_protocol_20305::PROTOCOL_EDITION.to_string(),
            scope,
            vec![
                DeclaredCapability::Read,
                DeclaredCapability::Discovery,
                DeclaredCapability::ExternalEventIngest,
            ],
            Duration::from_secs(60),
            Vec::new(),
        )
        .expect("valid manifest")
    }

    /// A server with one installed DERControlList of two controls.
    fn server_with_controls() -> AggregatorServer {
        let mut server = AggregatorServer::new(
            manifest(),
            AddressPlan::csip_default(),
            ServerLimits::new(8, 8).expect("valid limits"),
            MtlsRequirements::new(vec!["sim-trust-anchor".to_string()], true)
                .expect("valid tls"),
        )
        .expect("valid server");
        let plan = AddressPlan::csip_default();
        let controls: Vec<DERControl> = (0..2)
            .map(|i| {
                DERControl::new(
                    format!("sim-ctrl-{i}"),
                    Some(TimeType(1)),
                    Some(TimeType(1000 + i)),
                    Some(300),
                    None,
                    None,
                    DERControlBase {
                        op_mod_gen_lim_w: Some(1000.0),
                        ..DERControlBase::default()
                    },
                )
                .expect("valid control")
            })
            .collect();
        server
            .install(
                plan.der_control_list.clone(),
                StoredResource::DERControlList(DERControlList {
                    meta: ListMeta::unpaged(2),
                    poll_rate: Some(60),
                    der_controls: controls,
                }),
            )
            .expect("installed");
        server
    }

    #[test]
    fn simulated_device_polls_the_declared_schedule_in_order() {
        let mut server = server_with_controls();
        let mut device =
            Simulated20305Device::new("device-1", AddressPlan::csip_default());
        // Tick 0: the EndDeviceList (installed nowhere → 404; the plan is
        // declared but the resource is absent — the server's honest
        // answer).
        let response = device.poll_once(&mut server);
        assert_eq!(response.status, ResponseStatus::NotFound);
        assert_eq!(device.tick(), 1);
        // Ticks 1–2: the FunctionSetAssignmentsList and the
        // DERProgramList (declared, absent → 404 each).
        let _ = device.poll_once(&mut server);
        let _ = device.poll_once(&mut server);
        // Tick 3: the DERControlList (installed → 200 with a body).
        let response = device.poll_once(&mut server);
        assert!(is_ok_with_body(&response), "installed resource polls OK");
        let body = response.body.expect("body");
        assert_eq!(body["meta"]["all"], 2);
        assert_eq!(body["derControls"][0]["mrid"], "sim-ctrl-0");
    }

    #[test]
    fn simulated_device_posts_a_bounded_response_and_logs_it() {
        let mut server = server_with_controls();
        let mut device =
            Simulated20305Device::new("device-1", AddressPlan::csip_default());
        let body = serde_json::json!({
            "createdDateTime": 10,
            "status": 1,
        });
        let response = device.post_response(
            &mut server,
            1,
            None,
            TimeType(10),
            body,
        );
        assert_eq!(response.status, ResponseStatus::Created);
        assert_eq!(device.posted_responses().len(), 1);
        // The server's ResponseList now carries the device's response.
        let plan = AddressPlan::csip_default();
        let get = server.handle(&scripted_get(plan.response_list.clone()));
        assert_eq!(get.body.as_ref().expect("body")["meta"]["all"], 1);
    }

    #[test]
    fn simulated_transport_serves_the_script_and_logs_every_response() {
        let mut server = server_with_controls();
        let plan = AddressPlan::csip_default();
        let transport = SimulatedAggregatorTransport::new(
            "transport-1",
            MtlsRequirements::new(vec!["sim-trust-anchor".to_string()], true)
                .expect("valid tls"),
            vec![
                scripted_get(plan.der_control_list.clone()),
                scripted_get(plan.pricing_list.clone()),
            ],
        )
        .expect("valid transport");
        let log = transport.response_log();
        Box::new(transport)
            .serve(&mut server)
            .expect("the scripted serve completes");
        let responses = log.lock().expect("log mutex");
        assert_eq!(responses.len(), 2);
        assert_eq!(responses[0].status, ResponseStatus::Ok);
        assert_eq!(responses[1].status, ResponseStatus::NotFound);
    }

    #[test]
    fn an_empty_trust_anchor_set_refuses_at_construction() {
        // Hand-built degenerate requirements (the fields are public):
        // construction is refused...
        assert!(matches!(
            SimulatedAggregatorTransport::new(
                "transport-2",
                MtlsRequirements { trust_anchors: Vec::new(), check_revocation: true },
                vec![scripted_get(AddressPlan::csip_default().der_control_list)],
            ),
            Err(Sim20305Error::InvalidTlsRequirements)
        ));
        // ...and an empty script is refused too.
        assert!(matches!(
            SimulatedAggregatorTransport::new(
                "transport-3",
                MtlsRequirements::new(vec!["sim-trust-anchor".to_string()], true)
                    .expect("valid tls"),
                Vec::new(),
            ),
            Err(Sim20305Error::EmptyScript)
        ));
    }

    #[test]
    fn simulated_log_events_carry_the_sim_prefix() {
        let event = simulated_log_event("poll completed".to_string(), TimeType(5))
            .expect("valid event");
        // The serialized details string carries the simulated prefix
        // (identity from content alone).
        let json = serde_json::to_value(&event).expect("serializes");
        let details = json["details"].as_str().expect("details string");
        assert!(details.starts_with(SIM_20305_ID_PREFIX));
    }
}
