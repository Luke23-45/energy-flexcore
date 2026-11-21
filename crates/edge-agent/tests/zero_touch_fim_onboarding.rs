// FIM-F15.04/FIM-F15.06 — WRITTEN-UNRUN integration tests for the FDO-
// pattern zero-touch onboarding surface: the voucher-refusal (the manual
// P9.33 path is the fallback), the claim success through an in-memory
// rendezvous + voucher + fleet composition, the owner-agreement mismatch
// refusal, the P18.08 revocation interplay, the fallback preservation
// (the manual path still works; zero-touch only when configured), and the
// audit records for every claim attempt (record
// docs/phases-11-20/phase-18-edge-fleet-lifecycle-ota/records/
// 22_p18_09_zero_touch_onboarding.md; the FIM hard boundary: these tests
// are written, their execution is a HUMAN GATE).

use std::collections::BTreeMap;
use std::sync::Arc;

use flexcore_ports::clock::Clock;
use flexcore_ports::fakes::FixedClock;

use flexcore_edge_agent::enrollment::{DeviceBinding, ProvisioningToken};
use flexcore_edge_agent::enrollment_ceremony::{
    CeremonyState, EnrollmentCeremony, EnrollmentTokenRecord, TokenSingleUseLedger,
};
use flexcore_edge_agent::fleet_registry::FleetStateView;
use flexcore_edge_agent::ports::{AuditSink, EdgeAuditKind, InMemoryAuditSink};
use flexcore_edge_agent::zero_touch::{
    NoRendezvousConfigured, OwnershipVoucherRegister, RendezvousSeam, VoucherEvent,
    VoucherEventKind, VoucherState,
};
use flexcore_edge_agent::zero_touch_claim::{
    ConfiguredRendezvousResolver, RendezvousEndpointConfig, RendezvousReply,
    RendezvousTransport, RendezvousTransportError, ZeroTouchClaimRequest,
    ZeroTouchOnboardingConfig, ZeroTouchOnboardingError, ZeroTouchOnboardingService,
    EDGE_ZERO_TOUCH_CLAIM_HANDOFF, EDGE_ZERO_TOUCH_CLAIM_REFUSED,
};

const BASE: i64 = 1_700_000_000_000;
const DEVICE: &str = "edge-factory-01";
const IDEVID: &str = "idevid-ref-01";

// --- Fixtures ------------------------------------------------------------------------

fn reply(endpoint: &str, tenant: &str, site: &str) -> RendezvousReply {
    RendezvousReply {
        onboarding_endpoint_reference: endpoint.to_string(),
        resolved_tenant_id: tenant.to_string(),
        resolved_site_id: site.to_string(),
    }
}

fn endpoint_config() -> RendezvousEndpointConfig {
    RendezvousEndpointConfig::new(
        "rendezvous.internal:8038".to_string(),
        "FDO-pattern-analog (UNVERIFIED)".to_string(),
    )
    .expect("valid endpoint")
}

/// The in-memory rendezvous transport (socket-free test double; the real
/// transport is the deployment's HUMAN/INFRA GATE).
struct MapTransport {
    replies: BTreeMap<String, RendezvousReply>,
}

impl MapTransport {
    fn with_replies(replies: BTreeMap<String, RendezvousReply>) -> Self {
        Self { replies }
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
        self.replies
            .get(device_identity)
            .cloned()
            .ok_or(RendezvousTransportError::Unreachable {
                reason: format!("no route to the rendezvous service for '{device_identity}'"),
            })
    }
}

fn vouchered() -> OwnershipVoucherRegister {
    let mut register = OwnershipVoucherRegister::new();
    register
        .append(VoucherEvent {
            kind: VoucherEventKind::VoucherRecorded,
            tenant_id: "tenant-a".to_string(),
            device_identity: DEVICE.to_string(),
            site_id: "pilot-site-A".to_string(),
            actor: "provisioning".to_string(),
            reference: IDEVID.to_string(),
            at_unix_millis: BASE,
        })
        .expect("valid recording event");
    register
}

fn claim_request(device: &str, idevid: &str, now: i64) -> ZeroTouchClaimRequest {
    ZeroTouchClaimRequest {
        device_identity: device.to_string(),
        presented_idevid_reference: idevid.to_string(),
        tenant_id: "tenant-a".to_string(),
        site_id: "pilot-site-A".to_string(),
        token_reference: "tok-ref-1".to_string(),
        token_issued_by: "platform-identity-authority".to_string(),
        token_expires_at_unix_millis: now + 60_000,
        now_unix_millis: now,
        correlation_id: "corr-zt-1".to_string(),
    }
}

/// The enabled configuration (the OPT-IN state; the DEFAULT is
/// [`ZeroTouchOnboardingConfig::disabled`]).
fn enabled_config() -> ZeroTouchOnboardingConfig {
    ZeroTouchOnboardingConfig {
        enabled: true,
        rendezvous_endpoint: Some(endpoint_config()),
    }
}

fn refusal_lines(audit: &InMemoryAuditSink) -> Vec<String> {
    audit
        .events()
        .iter()
        .filter(|event| event.detail.starts_with(EDGE_ZERO_TOUCH_CLAIM_REFUSED))
        .map(|event| event.detail.clone())
        .collect()
}

fn handoff_lines(audit: &InMemoryAuditSink) -> Vec<String> {
    audit
        .events()
        .iter()
        .filter(|event| event.detail.starts_with(EDGE_ZERO_TOUCH_CLAIM_HANDOFF))
        .map(|event| event.detail.clone())
        .collect()
}

// --- The pinned cases ------------------------------------------------------------------

#[test]
fn a_device_without_a_voucher_is_refused_and_the_manual_path_is_the_fallback() {
    let mut register = OwnershipVoucherRegister::new(); // NO voucher recorded
    let transport = MapTransport::with_replies(BTreeMap::new());
    let resolver = ConfiguredRendezvousResolver::new(endpoint_config(), &transport)
        .expect("composed");
    let audit = InMemoryAuditSink::new();
    let mut fleet = FleetStateView::new();
    let mut service = ZeroTouchOnboardingService::new(
        &mut register as &mut dyn flexcore_edge_agent::zero_touch::VoucherStore,
        &resolver,
        &audit,
        &mut fleet,
        enabled_config(),
    );
    let error = service
        .claim(claim_request("edge-unvouchered", "idevid-ref-x", BASE + 1))
        .unwrap_err();
    // The existing rule, now enforced by the claim flow.
    assert_eq!(
        error.name(),
        "ZERO_TOUCH_NO_VOUCHER_FOR_DEVICE",
        "a device WITHOUT a voucher entry is refused — the manual P9.33 path is the \
         fallback for exactly that device"
    );
    // The register is untouched by the refused flow.
    assert_eq!(register.event_len(), 0);
    assert!(fleet.current_cohort("edge-unvouchered").is_none());
    // The refusal is audited with the named reason and the fallback note.
    let lines = refusal_lines(&audit);
    assert_eq!(lines.len(), 1, "every claim attempt is audited");
    assert!(lines[0].contains("ZERO_TOUCH_NO_VOUCHER_FOR_DEVICE"));
    assert!(lines[0].contains("the manual P9.33"));
}

#[test]
fn a_claim_completes_through_the_in_memory_rendezvous_voucher_and_fleet() {
    let mut register = vouchered();
    let mut replies = BTreeMap::new();
    replies.insert(
        DEVICE.to_string(),
        reply("onboard-endpoint-1", "tenant-a", "pilot-site-A"),
    );
    let transport = MapTransport::with_replies(replies);
    let resolver =
        ConfiguredRendezvousResolver::new(endpoint_config(), &transport).expect("composed");
    let audit = InMemoryAuditSink::new();
    let mut fleet = FleetStateView::new();
    let mut service = ZeroTouchOnboardingService::new(
        &mut register as &mut dyn flexcore_edge_agent::zero_touch::VoucherStore,
        &resolver,
        &audit,
        &mut fleet,
        enabled_config(),
    );
    let outcome = service
        .claim(claim_request(DEVICE, IDEVID, BASE + 1))
        .expect("the claim hands off");
    assert_eq!(outcome.endpoint_reference, "onboard-endpoint-1");
    assert_eq!(outcome.voucher_state, "CLAIMED");
    assert!(outcome.fleet_registered);
    // The register moved HELD → RELEASED_TO_RENDEZVOUS → CLAIMED (three
    // events: recorded + released + claimed).
    assert_eq!(register.event_len(), 3);
    let voucher = register
        .voucher("tenant-a", DEVICE)
        .expect("voucher entry");
    assert_eq!(voucher.state, VoucherState::Claimed);
    // The fleet join is a COHORT_MEMBERSHIP row recorded OUT (joining the
    // fleet is not a rollout cohort assignment — P18.06 owns cohorts).
    let cohort = fleet
        .current_cohort(DEVICE)
        .expect("the fleet row is recorded")
        .expect("parses");
    assert!(cohort.cohort_id.is_empty(), "no rollout cohort assigned");
    // The audit trail carries the handoff line (handoff ISSUES NOTHING;
    // the P9.33 ceremony's approval gate runs unchanged).
    let lines = handoff_lines(&audit);
    assert_eq!(lines.len(), 1);
    assert!(lines[0].contains("handoff ISSUES NOTHING"));
    assert!(lines[0].contains("approval gate run unchanged"));
    // The zero-touch claim removed the on-site operator-token step but the
    // handoff token discipline is the SAME single-use, short-lived
    // platform-authority record the manual path presents.
    assert_eq!(outcome.tenant_id, "tenant-a");
}

#[test]
fn an_owner_disagreement_between_rendezvous_and_voucher_is_a_named_refusal() {
    let mut register = vouchered();
    let mut replies = BTreeMap::new();
    // The rendezvous resolves the WRONG site: late binding refuses the
    // disagreement (the existing rule).
    replies.insert(
        DEVICE.to_string(),
        reply("onboard-endpoint-2", "tenant-a", "other-site"),
    );
    let transport = MapTransport::with_replies(replies);
    let resolver =
        ConfiguredRendezvousResolver::new(endpoint_config(), &transport).expect("composed");
    let audit = InMemoryAuditSink::new();
    let mut fleet = FleetStateView::new();
    let mut service = ZeroTouchOnboardingService::new(
        &mut register as &mut dyn flexcore_edge_agent::zero_touch::VoucherStore,
        &resolver,
        &audit,
        &mut fleet,
        enabled_config(),
    );
    let error = service
        .claim(claim_request(DEVICE, IDEVID, BASE + 1))
        .unwrap_err();
    assert_eq!(error.name(), "ZERO_TOUCH_SCOPE_MISMATCH");
    // The register is untouched (the flow refused before the events).
    assert_eq!(register.event_len(), 1);
    assert!(fleet.current_cohort(DEVICE).is_none());
    // The refusal is audited.
    let lines = refusal_lines(&audit);
    assert_eq!(lines.len(), 1);
    assert!(lines[0].contains("ZERO_TOUCH_SCOPE_MISMATCH"));
}

#[test]
fn a_revoked_voucher_never_claims_the_p18_08_interplay() {
    let mut register = vouchered();
    // The P18.08 revocation propagation invalidates the voucher.
    register
        .append(VoucherEvent {
            kind: VoucherEventKind::VoucherInvalidated,
            tenant_id: "tenant-a".to_string(),
            device_identity: DEVICE.to_string(),
            site_id: "pilot-site-A".to_string(),
            actor: "revocation-propagation".to_string(),
            reference: "decommission-ticket-1".to_string(),
            at_unix_millis: BASE + 2_000,
        })
        .expect("valid invalidation");
    let mut replies = BTreeMap::new();
    replies.insert(
        DEVICE.to_string(),
        reply("onboard-endpoint-1", "tenant-a", "pilot-site-A"),
    );
    let transport = MapTransport::with_replies(replies);
    let resolver =
        ConfiguredRendezvousResolver::new(endpoint_config(), &transport).expect("composed");
    let audit = InMemoryAuditSink::new();
    let mut fleet = FleetStateView::new();
    let mut service = ZeroTouchOnboardingService::new(
        &mut register as &mut dyn flexcore_edge_agent::zero_touch::VoucherStore,
        &resolver,
        &audit,
        &mut fleet,
        enabled_config(),
    );
    let error = service
        .claim(claim_request(DEVICE, IDEVID, BASE + 3_000))
        .unwrap_err();
    assert_eq!(
        error.name(),
        "ZERO_TOUCH_DEVICE_REVOKED",
        "a device whose voucher was invalidated can never be claimed"
    );
    assert_eq!(register.event_len(), 2, "no claim events appended");
    let lines = refusal_lines(&audit);
    assert_eq!(lines.len(), 1);
    assert!(lines[0].contains("ZERO_TOUCH_DEVICE_REVOKED"));
}

#[test]
fn the_manual_p9_33_path_is_the_default_and_still_works() {
    // (1) The DEFAULT configuration refuses before any claim step runs.
    let mut register = vouchered();
    let rendezvous = NoRendezvousConfigured::new();
    let audit = InMemoryAuditSink::new();
    let mut fleet = FleetStateView::new();
    let mut service = ZeroTouchOnboardingService::new(
        &mut register as &mut dyn flexcore_edge_agent::zero_touch::VoucherStore,
        &rendezvous as &dyn RendezvousSeam,
        &audit,
        &mut fleet,
        ZeroTouchOnboardingConfig::disabled(),
    );
    let error = service
        .claim(claim_request(DEVICE, IDEVID, BASE + 1))
        .unwrap_err();
    assert_eq!(error.name(), "ZERO_TOUCH_ONBOARDING_DISABLED");
    assert_eq!(register.event_len(), 1, "no claim step ran");
    assert!(fleet.current_cohort(DEVICE).is_none());
    let disabled_lines = refusal_lines(&audit);
    assert_eq!(disabled_lines.len(), 1);
    assert!(disabled_lines[0].contains("ZERO_TOUCH_ONBOARDING_DISABLED"));

    // (2) The manual P9.33 ceremony composition still works unchanged: a
    // fresh platform-authority token opens the ceremony (the DEFAULT path
    // for every device).
    let ceremony_audit: Arc<InMemoryAuditSink> = Arc::new(InMemoryAuditSink::new());
    let clock: Arc<FixedClock> = Arc::new(FixedClock::at_unix_millis(BASE + 1));
    let mut ceremony = EnrollmentCeremony::new(
        DEVICE.to_string(),
        DeviceBinding::new("tenant-a".to_string(), "pilot-site-A".to_string()),
        None,
        Arc::clone(&ceremony_audit) as Arc<dyn AuditSink>,
        Arc::clone(&clock) as Arc<dyn Clock + Send + Sync>,
    )
    .expect("composed");
    let token = ProvisioningToken::new("manual-path-token-value".to_string())
        .expect("valid token");
    let token_record = EnrollmentTokenRecord {
        token_reference: "manual-token-ref-1".to_string(),
        issued_by: "platform-identity-authority".to_string(),
        issued_at_unix_millis: BASE,
        expires_at_unix_millis: BASE + 10 * 60_000,
    };
    let mut single_use = TokenSingleUseLedger::new();
    ceremony
        .begin_request(&token, token_record, &mut single_use)
        .expect("the manual path still opens");
    assert_eq!(ceremony.state(), CeremonyState::Requested);

    // (3) Zero-touch ENABLED but the rendezvous UNCONFIGURED: the shipped
    // refuse-everything default is preserved (the named
    // ZERO_TOUCH_RENDEZVOUS_NOT_CONFIGURED refusal).
    let enabled_unconfigured = ZeroTouchOnboardingConfig {
        enabled: true,
        rendezvous_endpoint: None,
    };
    let composed = enabled_unconfigured
        .configured_rendezvous(&MapTransport::with_replies(BTreeMap::new()))
        .expect("composed");
    assert_eq!(composed.name(), "NO_RENDEZVOUS_CONFIGURED");
    let mut service_unconfigured = ZeroTouchOnboardingService::new(
        &mut register as &mut dyn flexcore_edge_agent::zero_touch::VoucherStore,
        composed.as_ref(),
        &audit,
        &mut fleet,
        enabled_unconfigured,
    );
    let error = service_unconfigured
        .claim(claim_request(DEVICE, IDEVID, BASE + 2))
        .unwrap_err();
    assert_eq!(error.name(), "ZERO_TOUCH_RENDEZVOUS_NOT_CONFIGURED");
    assert_eq!(register.event_len(), 1, "still no claim events");
    let lines = refusal_lines(&audit);
    assert_eq!(lines.len(), 2, "the disabled + the unconfigured refusals");
    assert!(lines[1].contains("ZERO_TOUCH_RENDEZVOUS_NOT_CONFIGURED"));
}

#[test]
fn every_claim_attempt_leaves_exactly_one_audited_line() {
    // Success, no-voucher, scope mismatch, revoked, disabled, and bad
    // input — each attempt ends in exactly one audit line (the refusal or
    // the handoff), through the EXISTING audit shapes with the dedicated
    // event name LEADING the detail text and the existing
    // enrollment-lifecycle kind carrying the classification.
    let mut register = vouchered();
    let mut replies = BTreeMap::new();
    replies.insert(
        DEVICE.to_string(),
        reply("onboard-endpoint-1", "tenant-a", "pilot-site-A"),
    );
    let transport = MapTransport::with_replies(replies);
    let resolver =
        ConfiguredRendezvousResolver::new(endpoint_config(), &transport).expect("composed");
    let audit = InMemoryAuditSink::new();
    let mut fleet = FleetStateView::new();
    let mut service = ZeroTouchOnboardingService::new(
        &mut register as &mut dyn flexcore_edge_agent::zero_touch::VoucherStore,
        &resolver,
        &audit,
        &mut fleet,
        enabled_config(),
    );
    // (a) A malformed request (no correlation id) — refused and audited.
    let mut bad = claim_request(DEVICE, IDEVID, BASE + 1);
    bad.correlation_id = "  ".to_string();
    assert_eq!(
        service.claim(bad).unwrap_err().name(),
        "ZERO_TOUCH_CLAIM_INPUT_INVALID"
    );
    // (b) The good claim — the handoff line.
    service
        .claim(claim_request(DEVICE, IDEVID, BASE + 2))
        .expect("the claim hands off");
    // (c) The claimed voucher cannot claim again (the existing
    // ZERO_TOUCH_VOUCHER_NOT_CLAIMABLE refusal).
    let error = service
        .claim(claim_request(DEVICE, IDEVID, BASE + 3))
        .unwrap_err();
    assert_eq!(error.name(), "ZERO_TOUCH_VOUCHER_NOT_CLAIMABLE");

    let events = audit.events();
    assert_eq!(events.len(), 3, "one audited line per attempt");
    assert!(matches!(events[0].kind, EdgeAuditKind::EnrollmentRejected));
    assert!(matches!(events[1].kind, EdgeAuditKind::EnrollmentCompleted));
    assert!(matches!(events[2].kind, EdgeAuditKind::EnrollmentRejected));
    assert!(events[0].detail.starts_with(EDGE_ZERO_TOUCH_CLAIM_REFUSED));
    assert!(events[1].detail.starts_with(EDGE_ZERO_TOUCH_CLAIM_HANDOFF));
    assert!(events[2].detail.starts_with(EDGE_ZERO_TOUCH_CLAIM_REFUSED));
    assert!(events[2].detail.contains("ZERO_TOUCH_VOUCHER_NOT_CLAIMABLE"));
    // The disabled-config error names the onboarding gate itself.
    assert_eq!(
        ZeroTouchOnboardingError::OnboardingDisabled.name(),
        "ZERO_TOUCH_ONBOARDING_DISABLED"
    );
    // The standing statement is queryable and names the preserved default.
    assert!(flexcore_edge_agent::zero_touch_claim::MANUAL_PATH_DEFAULT_STATEMENT
        .contains("DEFAULT and the FALLBACK"));
}
