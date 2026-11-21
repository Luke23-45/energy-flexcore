// FIM-F15 — FDO-pattern zero-touch onboarding, IN-REPO SURFACE: the
// configured rendezvous resolver behind the EXISTING seam, the opt-in
// claim composition, and the audit trail for every claim attempt
// (phase-18 plan P18.09; record
// docs/phases-11-20/phase-18-edge-fleet-lifecycle-ota/records/
// 22_p18_09_zero_touch_onboarding.md; FDO specifics UNVERIFIED [S-165]).
//
// HONESTY HEADER (FIM-F15.07; FIM standing decision 1,
// docs/releases/implementation_ledger/README.md):
//
// - **Implemented-under-FIM-F15, WRITTEN-UNRUN.** This module has never
//   compiled or executed; the first human build/test gates it (conventions
//   §1.3; the FIM hard boundary — no build, no test execution, no
//   installation).
// - **THE MANUAL P9.33 PATH STAYS THE DEFAULT AND THE FALLBACK** — this is
//   structural, and it is preserved verbatim: zero-touch is OPT-IN per
//   configuration ([`ZeroTouchOnboardingConfig::enabled`] DEFAULTS TO
//   FALSE and every claim refuses with the named
//   `ZERO_TOUCH_ONBOARDING_DISABLED` while it is off), and the shipped
//   [`crate::zero_touch::NoRendezvousConfigured`] refuse-everything
//   rendezvous default is UNCHANGED (a deployment composed with this
//   module and nothing else performs ZERO zero-touch claims and behaves
//   exactly as the P9.33 manual path does). The claim flow's handoff
//   ISSUES NOTHING: the P9.33 ceremony's challenge and THE platform-side
//   approval gate run UNCHANGED after it.
// - **HUMAN/INFRA GATES (named honestly):** an external rendezvous
//   service and a voucher-issuance process at provisioning are the two
//   infrastructure prerequisites this code surface CONSUMES and never
//   provides. The real network exchange rides the injected
//   [`RendezvousTransport`] port — the deployment implements it (a new
//   service surface with its own D6 review, per record 22 §4); this module
//   is socket-free and composes in-memory test doubles only. No device is
//   claimed; no voucher exists; no rendezvous service is operated.
// - **AUDIT through the EXISTING shapes:** every claim attempt ends in
//   exactly one audited line through the edge [`AuditSink`] port — a
//   refusal (every refusal reason, named) or the handoff success. The
//   existing `EdgeAuditKind` vocabulary is NOT widened (the additive-kind
//   precedent keeps the dedicated event name LEADING the detail text of
//   the existing enrollment-lifecycle kinds).
// - **HONEST BOUNDS:** the claim is reference-equality only (P18-E-2, no
//   hardware anchor is verified — `VoucherAnchorPosture` records what
//   provisioning claimed, never a platform claim that one exists); the
//   fleet join records COHORT-OUT (joining the fleet is not a rollout
//   cohort assignment — P18.06 owns cohorts).

use flexcore_contracts::time_model::IngestTime;

use crate::fleet_registry::{
    CohortMembership, FleetRegistryError, FleetStateKind, FleetStateRecord, FleetStateView,
};
use crate::ports::{AuditSink, EdgeAuditEvent, EdgeAuditKind};
use crate::zero_touch::{
    NoRendezvousConfigured, RendezvousDirective, RendezvousSeam, VoucherEvent, VoucherEventKind,
    VoucherState, VoucherStore, ZeroTouchClaimFlow, ZeroTouchError,
};

// --- Constants -----------------------------------------------------------------

/// The dedicated event name leading the detail text of every zero-touch
/// claim REFUSAL audit line (the additive-kind precedent: the dedicated
/// name leads, the existing `EdgeAuditKind` carries the classification).
pub const EDGE_ZERO_TOUCH_CLAIM_REFUSED: &str = "EDGE_ZERO_TOUCH_CLAIM_REFUSED";

/// The dedicated event name leading the detail text of the claim HANDOFF
/// success line (handoff issues nothing; the P9.33 ceremony's approval
/// gate runs unchanged).
pub const EDGE_ZERO_TOUCH_CLAIM_HANDOFF: &str = "EDGE_ZERO_TOUCH_CLAIM_HANDOFF";

/// The standing statement of the preserved default (queryable fact for
/// compositions and reviews).
pub const MANUAL_PATH_DEFAULT_STATEMENT: &str = "the manual P9.33 enrollment path is the \
     DEFAULT and the FALLBACK: zero-touch onboarding is opt-in per configuration and OFF by \
     default; the shipped NoRendezvousConfigured rendezvous refuses EVERY lookup; a claim \
     handoff ISSUES NOTHING and the P9.33 ceremony's challenge and platform-side approval \
     gate run unchanged.";

// --- The rendezvous configuration and transport seam (FIM-F15.02) ---------------

/// The rendezvous endpoint configuration (FIM-F15.02): the address and
/// protocol the deployment configured for ITS rendezvous service. Both are
/// mandatory — an unconfigured rendezvous keeps the shipped
/// [`NoRendezvousConfigured`] refuse-everything default (the composition
/// helper below).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RendezvousEndpointConfig {
    /// The rendezvous service address (an endpoint reference carried as
    /// configuration data; this module performs no network I/O itself).
    pub endpoint_address: String,
    /// The rendezvous protocol label (declared configuration; the FDO
    /// protocol specifics are UNVERIFIED [S-165] — this module is an
    /// analog of the PATTERN, never FDO conformance).
    pub protocol: String,
}

impl RendezvousEndpointConfig {
    /// Validates the configured endpoint (fail-closed: an empty address or
    /// protocol is refused, never defaulted).
    pub fn new(endpoint_address: String, protocol: String) -> Result<Self, ZeroTouchError> {
        if endpoint_address.trim().is_empty() {
            return Err(ZeroTouchError::InvalidClaimInput {
                reason: "the rendezvous endpoint address is mandatory".to_string(),
            });
        }
        if protocol.trim().is_empty() {
            return Err(ZeroTouchError::InvalidClaimInput {
                reason: "the rendezvous protocol is mandatory".to_string(),
            });
        }
        Ok(Self {
            endpoint_address,
            protocol,
        })
    }
}

/// The rendezvous reply the transport seam returns for one device (the
/// resolved onboarding directive's transport-level shape; the resolver maps
/// it into the EXISTING [`RendezvousDirective`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RendezvousReply {
    pub onboarding_endpoint_reference: String,
    pub resolved_tenant_id: String,
    pub resolved_site_id: String,
}

/// The rendezvous transport seam (FIM-F15.02): the actual network exchange
/// with the deployment's rendezvous service. Sync trait, house rules; the
/// deployment implements it (a HUMAN/INFRA GATE — no real transport exists
/// in this repository). The claim logic stays socket-free and testable:
/// tests compose an in-memory transport.
pub trait RendezvousTransport {
    /// The transport name for audit lines.
    fn name(&self) -> &'static str;
    /// Performs the rendezvous lookup for one device identity against the
    /// configured endpoint. `Err` = unreachable or malformed — refused
    /// fail-closed, never guessed to an owner.
    fn lookup(
        &self,
        endpoint: &RendezvousEndpointConfig,
        device_identity: &str,
    ) -> Result<RendezvousReply, RendezvousTransportError>;
}

/// The named rendezvous-transport failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RendezvousTransportError {
    /// The rendezvous service could not be reached.
    Unreachable { reason: String },
    /// The rendezvous service replied with an unusable body.
    MalformedReply { reason: String },
}

impl RendezvousTransportError {
    /// The stable machine-readable error name.
    pub fn name(&self) -> &'static str {
        match self {
            RendezvousTransportError::Unreachable { .. } => "RENDEZVOUS_TRANSPORT_UNREACHABLE",
            RendezvousTransportError::MalformedReply { .. } => "RENDEZVOUS_TRANSPORT_MALFORMED",
        }
    }
}

impl std::fmt::Display for RendezvousTransportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RendezvousTransportError::Unreachable { reason } => {
                write!(f, "the rendezvous service is unreachable: {reason}")
            }
            RendezvousTransportError::MalformedReply { reason } => {
                write!(f, "the rendezvous reply is malformed: {reason}")
            }
        }
    }
}

impl std::error::Error for RendezvousTransportError {}

/// The CONFIGURED rendezvous resolver (FIM-F15.02): implements the
/// EXISTING [`RendezvousSeam`] over the configured endpoint and the
/// injected transport. This REPLACES the shipped
/// [`NoRendezvousConfigured`] default ONLY when a deployment composes it;
/// an unconfigured deployment keeps the refuse-everything default (see
/// [`ZeroTouchOnboardingConfig::configured_rendezvous`]).
#[derive(Debug, Clone)]
pub struct ConfiguredRendezvousResolver<'a> {
    endpoint: RendezvousEndpointConfig,
    transport: &'a dyn RendezvousTransport,
}

impl<'a> ConfiguredRendezvousResolver<'a> {
    /// Composes the resolver (the endpoint is validated at construction).
    pub fn new(
        endpoint: RendezvousEndpointConfig,
        transport: &'a dyn RendezvousTransport,
    ) -> Result<Self, ZeroTouchError> {
        if endpoint.endpoint_address.trim().is_empty() || endpoint.protocol.trim().is_empty() {
            return Err(ZeroTouchError::InvalidClaimInput {
                reason: "the rendezvous endpoint address and protocol are both mandatory"
                    .to_string(),
            });
        }
        Ok(Self { endpoint, transport })
    }

    /// The configured endpoint (diagnostics).
    pub fn endpoint(&self) -> &RendezvousEndpointConfig {
        &self.endpoint
    }
}

impl RendezvousSeam for ConfiguredRendezvousResolver<'_> {
    fn name(&self) -> &'static str {
        "CONFIGURED_RENDEZVOUS_RESOLVER"
    }

    fn resolve(&self, device_identity: &str) -> Result<RendezvousDirective, ZeroTouchError> {
        if device_identity.trim().is_empty() {
            return Err(ZeroTouchError::InvalidClaimInput {
                reason: "the rendezvous lookup needs the device identity".to_string(),
            });
        }
        let reply = self
            .transport
            .lookup(&self.endpoint, device_identity)
            .map_err(|error| match error {
                // An unreachable CONFIGURED rendezvous is its own named
                // refusal (distinct from the un-configured default).
                RendezvousTransportError::Unreachable { reason } => {
                    ZeroTouchError::RendezvousUnreachable { reason }
                }
                RendezvousTransportError::MalformedReply { reason } => {
                    ZeroTouchError::InvalidClaimInput {
                        reason: format!("the rendezvous reply is unusable: {reason}"),
                    }
                }
            })?;
        if reply.onboarding_endpoint_reference.trim().is_empty()
            || reply.resolved_tenant_id.trim().is_empty()
            || reply.resolved_site_id.trim().is_empty()
        {
            return Err(ZeroTouchError::InvalidClaimInput {
                reason: "the rendezvous reply must name the endpoint reference and the \
                         resolved tenant/site (an incomplete directive is refused, never \
                         completed by guesswork)"
                    .to_string(),
            });
        }
        Ok(RendezvousDirective {
            onboarding_endpoint_reference: reply.onboarding_endpoint_reference,
            resolved_tenant_id: reply.resolved_tenant_id,
            resolved_site_id: reply.resolved_site_id,
        })
    }
}

// --- The onboarding configuration (FIM-F15.03/.04: opt-in, default OFF) ----------

/// The zero-touch onboarding configuration (FIM-F15.03/.04). **Zero-touch
/// is OPT-IN per configuration and OFF by default** — the manual P9.33
/// enrollment path is the default AND the fallback, structurally.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ZeroTouchOnboardingConfig {
    /// Whether zero-touch claiming is enabled for this deployment. FALSE
    /// is the DEFAULT: every claim refuses with the named
    /// [`ZeroTouchOnboardingError::OnboardingDisabled`] while off, and no
    /// claim step runs at all.
    pub enabled: bool,
    /// The configured rendezvous endpoint, when the deployment has one.
    /// `None` (the DEFAULT) composes the shipped
    /// [`NoRendezvousConfigured`] refuse-everything rendezvous — the F-15
    /// refuse-everything default, preserved.
    pub rendezvous_endpoint: Option<RendezvousEndpointConfig>,
}

impl ZeroTouchOnboardingConfig {
    /// The off-by-default configuration (the shipped state).
    pub fn disabled() -> Self {
        Self::default()
    }

    /// The composition helper (FIM-F15.02): composes the rendezvous seam
    /// this configuration yields — the CONFIGURED resolver over the
    /// injected transport when an endpoint is configured, otherwise the
    /// SHIPPED [`NoRendezvousConfigured`] refuse-everything default
    /// (preserved byte-for-byte).
    pub fn configured_rendezvous<'t>(
        &self,
        transport: &'t dyn RendezvousTransport,
    ) -> Result<Box<dyn RendezvousSeam + 't>, ZeroTouchError> {
        match &self.rendezvous_endpoint {
            Some(endpoint) => Ok(Box::new(ConfiguredRendezvousResolver {
                endpoint: endpoint.clone(),
                transport,
            })),
            None => Ok(Box::new(NoRendezvousConfigured::new())),
        }
    }
}

// --- The claim request and outcome -------------------------------------------------

/// One zero-touch claim attempt's input (FIM-F15.03). The handoff token
/// fields are the SAME single-use, short-lived platform-authority token
/// record the manual P9.33 path presents — zero-touch removes the on-site
/// operator-token step, never the approval gate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZeroTouchClaimRequest {
    /// The device identity exactly as the factory IDevID names it.
    pub device_identity: String,
    /// The IDevID reference the DEVICE presented (reference-equality only
    /// against the voucher — no hardware anchor is verified, P18-E-2).
    pub presented_idevid_reference: String,
    pub tenant_id: String,
    pub site_id: String,
    /// The fresh platform-authority token REFERENCE the handoff consumes.
    pub token_reference: String,
    /// The token's issuing authority (an un-attributable token is
    /// refused, mirroring the ceremony).
    pub token_issued_by: String,
    /// The token's expiry (the flow refuses an already-expired record).
    pub token_expires_at_unix_millis: i64,
    /// The injected clock instant (NEVER wall time; also the audit trail's
    /// instant).
    pub now_unix_millis: i64,
    /// The correlation id the audit trail and the fleet journal row carry.
    pub correlation_id: String,
}

impl ZeroTouchClaimRequest {
    /// Validates the request shape (fail-closed; the flow re-validates the
    /// token record's lifetime at handoff).
    pub fn validate(&self) -> Result<(), ZeroTouchError> {
        if self.device_identity.trim().is_empty()
            || self.presented_idevid_reference.trim().is_empty()
            || self.tenant_id.trim().is_empty()
            || self.site_id.trim().is_empty()
        {
            return Err(ZeroTouchError::InvalidClaimInput {
                reason: "device identity, presented IDevID reference, tenant, and site are \
                         all mandatory"
                    .to_string(),
            });
        }
        if self.token_reference.trim().is_empty() || self.token_issued_by.trim().is_empty() {
            return Err(ZeroTouchError::InvalidClaimInput {
                reason: "the handoff token reference and its issuing authority are both \
                         mandatory"
                    .to_string(),
            });
        }
        if self.correlation_id.trim().is_empty() {
            return Err(ZeroTouchError::InvalidClaimInput {
                reason: "the correlation id is mandatory (every attempt is audited)"
                    .to_string(),
            });
        }
        if self.now_unix_millis <= 0 {
            return Err(ZeroTouchError::InvalidClaimInput {
                reason: "the claim instant must be a positive unix-millis value (injected \
                         clock, never wall time)"
                    .to_string(),
            });
        }
        Ok(())
    }
}

/// One completed zero-touch claim (the handoff outcome). The handoff
/// ISSUES NOTHING: the P9.33 ceremony owns everything after this point.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZeroTouchClaimOutcome {
    pub device_identity: String,
    pub tenant_id: String,
    pub site_id: String,
    /// The onboarding endpoint reference the rendezvous resolved.
    pub endpoint_reference: String,
    /// The voucher register's post-claim state name (`CLAIMED`).
    pub voucher_state: &'static str,
    /// Whether the fleet-join journal row was recorded.
    pub fleet_registered: bool,
}

// --- Errors -----------------------------------------------------------------------

/// The named zero-touch onboarding-service errors. Every refusal names the
/// rule; nothing implies a silent pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ZeroTouchOnboardingError {
    /// Zero-touch is not enabled in the configuration (the DEFAULT): the
    /// claim refused before ANY step ran; the manual P9.33 path is the
    /// default and the fallback.
    OnboardingDisabled,
    /// The closed claim flow refused (the EXISTING named
    /// [`ZeroTouchError`] refusals, unchanged).
    ClaimRefused(ZeroTouchError),
    /// The fleet-join journal row was refused. The voucher register may
    /// already carry the CLAIMED event and the handoff may have happened —
    /// the caller reconciles from the register (recorded loudly, never
    /// silently absorbed).
    FleetRecordRefused { reason: String },
    /// The audit sink failed: the audit line was NOT recorded (the caller
    /// logs the failure; audit is never silently skipped — the
    /// audit-or-refuse discipline).
    AuditWriteFailed { reason: String },
    /// The request instant is not a usable audit/journal instant.
    TimeInstantInvalid { reason: String },
}

impl ZeroTouchOnboardingError {
    /// The stable machine-readable error name.
    pub fn name(&self) -> &'static str {
        match self {
            ZeroTouchOnboardingError::OnboardingDisabled => "ZERO_TOUCH_ONBOARDING_DISABLED",
            ZeroTouchOnboardingError::ClaimRefused(error) => error.name(),
            ZeroTouchOnboardingError::FleetRecordRefused { .. } => {
                "ZERO_TOUCH_FLEET_RECORD_REFUSED"
            }
            ZeroTouchOnboardingError::AuditWriteFailed { .. } => {
                "ZERO_TOUCH_CLAIM_AUDIT_WRITE_FAILED"
            }
            ZeroTouchOnboardingError::TimeInstantInvalid { .. } => {
                "ZERO_TOUCH_TIME_INSTANT_INVALID"
            }
        }
    }
}

impl std::fmt::Display for ZeroTouchOnboardingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ZeroTouchOnboardingError::OnboardingDisabled => write!(
                f,
                "zero-touch onboarding is not enabled in the configuration — refused before \
                 any claim step ran; the manual P9.33 enrollment path is the default and \
                 the fallback"
            ),
            ZeroTouchOnboardingError::ClaimRefused(error) => {
                write!(f, "the zero-touch claim flow refused: {error}")
            }
            ZeroTouchOnboardingError::FleetRecordRefused { reason } => write!(
                f,
                "the fleet-join journal row was refused ({reason}) — the voucher register \
                 may already carry the CLAIMED event; reconcile from the register, never \
                 silently absorb"
            ),
            ZeroTouchOnboardingError::AuditWriteFailed { reason } => {
                write!(f, "the claim audit line was NOT recorded: {reason}")
            }
            ZeroTouchOnboardingError::TimeInstantInvalid { reason } => {
                write!(f, "the claim instant is invalid: {reason}")
            }
        }
    }
}

impl std::error::Error for ZeroTouchOnboardingError {}

// --- The claim service (FIM-F15.03 + FIM-F15.05) ------------------------------------

/// The zero-touch onboarding service (FIM-F15.03): composes the voucher
/// store (the FIM-F15.01 persistence seam), the rendezvous seam (the
/// EXISTING one — the shipped refuse-everything default or a configured
/// resolver), the edge audit sink (the EXISTING shapes), and the fleet
/// state view. Opt-in per configuration; the manual P9.33 path is the
/// default and the fallback; every claim attempt is audited (FIM-F15.05).
pub struct ZeroTouchOnboardingService<'a> {
    /// The voucher store (the EXISTING in-memory register is the default
    /// [`VoucherStore`]; a durable store is the deployment's
    /// implementation behind the seam — a HUMAN/INFRA GATE).
    store: &'a mut dyn VoucherStore,
    /// The rendezvous seam (the EXISTING one; the shipped
    /// `NoRendezvousConfigured` default or a configured resolver).
    rendezvous: &'a dyn RendezvousSeam,
    /// The edge audit sink (the EXISTING port; the audit trail of
    /// FIM-F15.05).
    audit: &'a dyn AuditSink,
    /// The fleet state view (the EXISTING migration-0023 projection; the
    /// claim's fleet join is a COHORT_MEMBERSHIP row).
    fleet: &'a mut FleetStateView,
    /// The opt-in configuration (DEFAULT OFF).
    config: ZeroTouchOnboardingConfig,
}

impl<'a> ZeroTouchOnboardingService<'a> {
    /// Composes the service.
    pub fn new(
        store: &'a mut dyn VoucherStore,
        rendezvous: &'a dyn RendezvousSeam,
        audit: &'a dyn AuditSink,
        fleet: &'a mut FleetStateView,
        config: ZeroTouchOnboardingConfig,
    ) -> Self {
        Self {
            store,
            rendezvous,
            audit,
            fleet,
            config,
        }
    }

    /// The configuration (diagnostics).
    pub fn config(&self) -> &ZeroTouchOnboardingConfig {
        &self.config
    }

    /// Runs ONE zero-touch claim attempt (FIM-F15.03): the config gate,
    /// the closed claim flow (verify voucher → resolve rendezvous →
    /// confirm ownership → hand to ceremony), the voucher register events
    /// (HELD → RELEASED_TO_RENDEZVOUS → CLAIMED), the fleet join, and the
    /// audit trail (FIM-F15.05: every attempt — success and EVERY refusal
    /// reason — is audited).
    ///
    /// The handoff ISSUES NOTHING: the P9.33 ceremony's challenge and THE
    /// platform-side approval gate run unchanged afterwards. A device
    /// WITHOUT a voucher entry is refused (the existing
    /// `ZERO_TOUCH_NO_VOUCHER_FOR_DEVICE` rule) — the manual P9.33 path is
    /// the fallback for exactly that device.
    pub fn claim(
        &mut self,
        request: ZeroTouchClaimRequest,
    ) -> Result<ZeroTouchClaimOutcome, ZeroTouchOnboardingError> {
        // (0) The instant must be a usable audit/journal instant before
        // anything else (the audit trail needs a valid instant).
        if request.now_unix_millis <= 0 {
            return Err(ZeroTouchOnboardingError::TimeInstantInvalid {
                reason: "the claim instant must be a positive unix-millis value (injected \
                         clock, never wall time)"
                    .to_string(),
            });
        }
        // (1) THE CONFIG GATE: zero-touch is opt-in per configuration and
        // OFF by default. A disabled deployment refuses before ANY claim
        // step runs — the manual P9.33 path is the default and the
        // fallback, structurally.
        if !self.config.enabled {
            let refused = ZeroTouchOnboardingError::OnboardingDisabled;
            self.audit_refusal(&request, &refused)?;
            return Err(refused);
        }
        // (2) Request shape validation (fail-closed, audited).
        if let Err(error) = request.validate() {
            self.audit_refusal(&request, &ZeroTouchOnboardingError::ClaimRefused(error.clone()))?;
            return Err(ZeroTouchOnboardingError::ClaimRefused(error));
        }
        // (3) The closed claim flow. The flow borrows the voucher store
        // immutably; the register events append AFTER the flow completes
        // (the register stays untouched by a refused flow — the existing
        // behaviour). A construction refusal (invalid claim input) is
        // audited like every other refusal reason.
        let mut flow = match ZeroTouchClaimFlow::new_over_store(
            &request.device_identity,
            &request.presented_idevid_reference,
            &request.tenant_id,
            &request.site_id,
            &*self.store,
            self.rendezvous,
        ) {
            Ok(flow) => flow,
            Err(error) => {
                self.audit_refusal(&request, &ZeroTouchOnboardingError::ClaimRefused(error.clone()))?;
                return Err(ZeroTouchOnboardingError::ClaimRefused(error));
            }
        };
        if let Err(error) = flow.verify_voucher() {
            self.audit_refusal(&request, &ZeroTouchOnboardingError::ClaimRefused(error.clone()))?;
            return Err(ZeroTouchOnboardingError::ClaimRefused(error));
        }
        let directive = match flow.resolve_rendezvous() {
            Ok(directive) => directive,
            Err(error) => {
                self.audit_refusal(&request, &ZeroTouchOnboardingError::ClaimRefused(error.clone()))?;
                return Err(ZeroTouchOnboardingError::ClaimRefused(error));
            }
        };
        if let Err(error) = flow.confirm_ownership(&directive) {
            self.audit_refusal(&request, &ZeroTouchOnboardingError::ClaimRefused(error.clone()))?;
            return Err(ZeroTouchOnboardingError::ClaimRefused(error));
        }
        if let Err(error) = flow.hand_to_ceremony(
            &request.token_reference,
            &request.token_issued_by,
            request.token_expires_at_unix_millis,
            request.now_unix_millis,
        ) {
            self.audit_refusal(&request, &ZeroTouchOnboardingError::ClaimRefused(error.clone()))?;
            return Err(ZeroTouchOnboardingError::ClaimRefused(error));
        }
        drop(flow);
        // (4) The register events: the composition moves the voucher
        // HELD → RELEASED_TO_RENDEZVOUS → CLAIMED (the flow itself leaves
        // the register untouched by design; the closed machine validates
        // both edges). A register refusal is audited like every other
        // refusal reason.
        let released = self.store.append_event(VoucherEvent {
            kind: VoucherEventKind::ReleasedToRendezvous,
            tenant_id: request.tenant_id.clone(),
            device_identity: request.device_identity.clone(),
            site_id: request.site_id.clone(),
            actor: "zero-touch-onboarding".to_string(),
            reference: directive.onboarding_endpoint_reference.clone(),
            at_unix_millis: request.now_unix_millis,
        });
        if let Err(error) = released {
            self.audit_refusal(&request, &ZeroTouchOnboardingError::ClaimRefused(error.clone()))?;
            return Err(ZeroTouchOnboardingError::ClaimRefused(error));
        }
        let claimed = self.store.append_event(VoucherEvent {
            kind: VoucherEventKind::VoucherClaimed,
            tenant_id: request.tenant_id.clone(),
            device_identity: request.device_identity.clone(),
            site_id: request.site_id.clone(),
            actor: "zero-touch-onboarding".to_string(),
            reference: request.token_reference.clone(),
            at_unix_millis: request.now_unix_millis,
        });
        if let Err(error) = claimed {
            self.audit_refusal(&request, &ZeroTouchOnboardingError::ClaimRefused(error.clone()))?;
            return Err(ZeroTouchOnboardingError::ClaimRefused(error));
        }
        // (5) The fleet join: the device joins the fleet with NO rollout
        // cohort assignment (joining the fleet is not a cohort assignment
        // — P18.06 owns cohorts). The row rides the EXISTING
        // migration-0023 COHORT_MEMBERSHIP vocabulary ("OUT").
        let fleet_record = FleetStateRecord {
            event_id: format!(
                "zero-touch-claim:{}:{}:{}",
                request.tenant_id, request.device_identity, request.now_unix_millis
            ),
            tenant_id: request.tenant_id.clone(),
            device_id: request.device_identity.clone(),
            kind: FleetStateKind::CohortMembership,
            state_value: CohortMembership {
                cohort_id: String::new(),
                block_index: 0,
                observed_at_unix_millis: request.now_unix_millis,
            }
            .sql_value(),
            detail: Some(
                "joined the fleet through the zero-touch claim flow (voucher verified, \
                 rendezvous resolved, ownership confirmed, handoff to the P9.33 ceremony; \
                 the ceremony's platform-side approval gate runs unchanged)"
                    .to_string(),
            ),
            correlation_id: request.correlation_id.clone(),
            observed_at_unix_millis: request.now_unix_millis,
        };
        if let Err(error) = self.fleet.append(fleet_record, &refuse_extra_value_parser) {
            let reason = error.to_string();
            self.audit_event(
                &request,
                EDGE_ZERO_TOUCH_CLAIM_REFUSED,
                EdgeAuditKind::EnrollmentRejected,
                format!(
                    "device='{}' tenant='{}' site='{}' reason=ZERO_TOUCH_FLEET_RECORD_REFUSED \
                     ({reason}) — the handoff happened and the voucher register carries the \
                     CLAIMED event; reconcile from the register",
                    request.device_identity, request.tenant_id, request.site_id,
                ),
            )?;
            return Err(ZeroTouchOnboardingError::FleetRecordRefused { reason });
        }
        // (6) The handoff success line (the P9.33 ceremony's approval gate
        // runs unchanged after it).
        self.audit_event(
            &request,
            EDGE_ZERO_TOUCH_CLAIM_HANDOFF,
            EdgeAuditKind::EnrollmentCompleted,
            format!(
                "device='{}' tenant='{}' site='{}' endpoint_ref='{}' voucher=CLAIMED \
                 fleet-registered=true — handoff ISSUES NOTHING; the P9.33 ceremony's \
                 challenge and platform-side approval gate run unchanged",
                request.device_identity,
                request.tenant_id,
                request.site_id,
                directive.onboarding_endpoint_reference,
            ),
        )?;
        Ok(ZeroTouchClaimOutcome {
            device_identity: request.device_identity,
            tenant_id: request.tenant_id,
            site_id: request.site_id,
            endpoint_reference: directive.onboarding_endpoint_reference,
            voucher_state: VoucherState::Claimed.name(),
            fleet_registered: true,
        })
    }

    /// Records one refusal line through the EXISTING audit shapes (the
    /// dedicated event name leads the detail text; the
    /// `EnrollmentRejected` kind carries the classification). The
    /// audit-or-refuse discipline: a failed audit line surfaces as the
    /// named `AuditWriteFailed` error, never a silent skip.
    fn audit_refusal(
        &self,
        request: &ZeroTouchClaimRequest,
        refused: &ZeroTouchOnboardingError,
    ) -> Result<(), ZeroTouchOnboardingError> {
        self.audit_event(
            request,
            EDGE_ZERO_TOUCH_CLAIM_REFUSED,
            EdgeAuditKind::EnrollmentRejected,
            format!(
                "device='{}' tenant='{}' site='{}' reason={} ({refused}) — the manual P9.33 \
                 enrollment path is the fallback for this attempt (zero-touch is opt-in \
                 per configuration; the P9.33 ceremony is unchanged)",
                request.device_identity, request.tenant_id, request.site_id,
                refused.name(),
            ),
        )
    }

    /// Records one audit line (the shared shape: the dedicated event name
    /// leads the detail text).
    fn audit_event(
        &self,
        request: &ZeroTouchClaimRequest,
        event_name: &str,
        kind: EdgeAuditKind,
        detail: String,
    ) -> Result<(), ZeroTouchOnboardingError> {
        let recorded_at = IngestTime::from_unix_millis(request.now_unix_millis).map_err(
            |error| ZeroTouchOnboardingError::TimeInstantInvalid {
                reason: error.to_string(),
            },
        )?;
        self.audit
            .record(EdgeAuditEvent {
                recorded_at,
                kind,
                detail: format!("{event_name} {detail}"),
            })
            .map_err(|error| ZeroTouchOnboardingError::AuditWriteFailed {
                reason: error.to_string(),
            })
    }
}

/// The extra-value parser the claim passes to the fleet view: the claim
/// appends COHORT rows only, so an attestation/eSIM row can never reach it
/// — the parser refuses loudly if one ever does (fail-closed, mirroring
/// the fleet-registry test stub).
fn refuse_extra_value_parser(value: &str) -> Result<(), FleetRegistryError> {
    Err(FleetRegistryError::UnknownStateValue {
        kind: FleetStateKind::AttestationState.sql_kind(),
        encountered: value.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::ports::InMemoryAuditSink;
    use crate::zero_touch::{
        OwnershipVoucherRegister, VoucherAnchorPosture, VoucherEvent, VoucherEventKind,
    };

    const BASE: i64 = 1_700_000_000_000;

    fn recorded(register: &mut OwnershipVoucherRegister) {
        register
            .append(VoucherEvent {
                kind: VoucherEventKind::VoucherRecorded,
                tenant_id: "tenant-a".to_string(),
                device_identity: "edge-factory-01".to_string(),
                site_id: "pilot-site-A".to_string(),
                actor: "provisioning".to_string(),
                reference: "idevid-ref-01".to_string(),
                at_unix_millis: BASE,
            })
            .expect("valid recording event");
    }

    fn reply() -> RendezvousReply {
        RendezvousReply {
            onboarding_endpoint_reference: "onboard-endpoint-1".to_string(),
            resolved_tenant_id: "tenant-a".to_string(),
            resolved_site_id: "pilot-site-A".to_string(),
        }
    }

    /// The in-memory rendezvous transport (socket-free test double).
    struct MapTransport {
        replies: BTreeMap<String, RendezvousReply>,
        unreachable: Vec<String>,
    }

    impl MapTransport {
        fn with_reply(device: &str, reply: RendezvousReply) -> Self {
            let mut replies = BTreeMap::new();
            replies.insert(device.to_string(), reply);
            Self {
                replies,
                unreachable: Vec::new(),
            }
        }

        fn unreachable_for(device: &str) -> Self {
            Self {
                replies: BTreeMap::new(),
                unreachable: vec![device.to_string()],
            }
        }
    }

    impl RendezvousTransport for MapTransport {
        fn name(&self) -> &'static str {
            "MAP_RENDEZVOUS_TRANSPORT_TEST_DOUBLE"
        }

        fn lookup(
            &self,
            _endpoint: &RendezvousEndpointConfig,
            device_identity: &str,
        ) -> Result<RendezvousReply, RendezvousTransportError> {
            if self.unreachable.iter().any(|d| d == device_identity) {
                return Err(RendezvousTransportError::Unreachable {
                    reason: format!("no route to the rendezvous service for '{device_identity}'"),
                });
            }
            self.replies
                .get(device_identity)
                .cloned()
                .ok_or(RendezvousTransportError::MalformedReply {
                    reason: format!("no reply for '{device_identity}'"),
                })
        }
    }

    fn endpoint_config() -> RendezvousEndpointConfig {
        RendezvousEndpointConfig::new(
            "rendezvous.internal:8038".to_string(),
            "FDO-pattern-analog (UNVERIFIED)".to_string(),
        )
        .expect("valid endpoint")
    }

    fn claim_request(device: &str, idevid: &str) -> ZeroTouchClaimRequest {
        ZeroTouchClaimRequest {
            device_identity: device.to_string(),
            presented_idevid_reference: idevid.to_string(),
            tenant_id: "tenant-a".to_string(),
            site_id: "pilot-site-A".to_string(),
            token_reference: "tok-ref-1".to_string(),
            token_issued_by: "platform-identity-authority".to_string(),
            token_expires_at_unix_millis: BASE + 60_000,
            now_unix_millis: BASE + 1,
            correlation_id: "corr-zt-1".to_string(),
        }
    }

    fn audit_lines(sink: &InMemoryAuditSink, event_name: &str) -> Vec<String> {
        sink.events()
            .into_iter()
            .filter(|event| event.detail.starts_with(event_name))
            .map(|event| event.detail)
            .collect()
    }

    #[test]
    fn the_shipped_default_composition_refuses_everything_and_audits_it() {
        // The DEFAULT configuration (disabled, no endpoint): the
        // refuse-everything default, preserved.
        let mut vouchers = OwnershipVoucherRegister::new();
        recorded(&mut vouchers);
        let rendezvous = NoRendezvousConfigured::new();
        let audit = InMemoryAuditSink::new();
        let mut fleet = FleetStateView::new();
        let mut service = ZeroTouchOnboardingService::new(
            &mut vouchers,
            &rendezvous,
            &audit,
            &mut fleet,
            ZeroTouchOnboardingConfig::disabled(),
        );
        let error = service.claim(claim_request("edge-factory-01", "idevid-ref-01")).unwrap_err();
        assert_eq!(error.name(), "ZERO_TOUCH_ONBOARDING_DISABLED");
        assert!(error.to_string().contains("manual P9.33"));
        // The refusal is audited with its named reason.
        let lines = audit_lines(&audit, EDGE_ZERO_TOUCH_CLAIM_REFUSED);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("ZERO_TOUCH_ONBOARDING_DISABLED"));
        // No claim step ran: the register is untouched and the fleet view
        // holds no row.
        assert_eq!(vouchers.event_len(), 1);
        assert!(fleet.current_cohort("edge-factory-01").is_none());
        // The disabled default composes the refuse-everything rendezvous.
        let composed = ZeroTouchOnboardingConfig::disabled()
            .configured_rendezvous(&MapTransport::with_reply("edge-factory-01", reply()))
            .expect("composed");
        assert_eq!(composed.name(), "NO_RENDEZVOUS_CONFIGURED");
        assert_eq!(
            composed.resolve("edge-factory-01").unwrap_err().name(),
            "ZERO_TOUCH_RENDEZVOUS_NOT_CONFIGURED"
        );
    }

    #[test]
    fn the_configured_resolver_maps_transport_failures_to_named_refusals() {
        let transport = MapTransport::unreachable_for("edge-factory-01");
        let resolver = ConfiguredRendezvousResolver::new(endpoint_config(), &transport)
            .expect("composed");
        assert_eq!(resolver.name(), "CONFIGURED_RENDEZVOUS_RESOLVER");
        // An unreachable CONFIGURED rendezvous is its own named refusal.
        assert_eq!(
            resolver
                .resolve("edge-factory-01")
                .unwrap_err()
                .name(),
            "ZERO_TOUCH_RENDEZVOUS_UNREACHABLE"
        );
        // An incomplete reply is refused, never completed by guesswork.
        let transport = MapTransport::with_reply(
            "edge-factory-01",
            RendezvousReply {
                onboarding_endpoint_reference: "  ".to_string(),
                resolved_tenant_id: "tenant-a".to_string(),
                resolved_site_id: "pilot-site-A".to_string(),
            },
        );
        let resolver =
            ConfiguredRendezvousResolver::new(endpoint_config(), &transport).expect("composed");
        assert!(resolver.resolve("edge-factory-01").is_err());
        // An empty endpoint configuration is refused at construction.
        assert!(RendezvousEndpointConfig::new("  ".to_string(), "p".to_string()).is_err());
        assert!(RendezvousEndpointConfig::new("a".to_string(), " ".to_string()).is_err());
        // The anchor-posture vocabulary rides the voucher (recorded data,
        // never a platform claim of a hardware anchor).
        assert_eq!(VoucherAnchorPosture::NotRecorded.name(), "NOT_RECORDED");
    }
}
