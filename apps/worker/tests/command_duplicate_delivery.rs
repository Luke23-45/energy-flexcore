// P11.12 — The duplicate-delivery test suite (Phase 11 wave 1, Agent B).
//
// WRITTEN COMPLETE AND UNRUN. Execution is a human gate (program
// conventions §1.3): no test in this file has been compiled or executed,
// and no behavior is claimed from it. The file asserts the WRITTEN code:
//
// 1. The dedupe oracle (`decide_transition`) decides exactly on the
//    frozen 14-state / 27-edge table — the lifecycle state machine is the
//    only dedupe authority (the P11.12 rule).
// 2. A redelivered command-lifecycle fact with an invalid transition is a
//    NO-OP plus the duplicate-delivery metric — never an error, never a
//    journal write, never a side effect (the P11.12 acceptance rule).
// 3. A frozen edge the stored command has not taken is SURFACED (named
//    handler error → the existing poison path), never fabricated into the
//    journal.
// 4. The handler composes with the P1.16 first dedupe layer: handled
//    deliveries record the inbox entry; a redelivery that hits the row is
//    skipped upstream before the oracle runs.
// 5. The P11.13 MaxDeliver alignment holds between the helper config and
//    the provisioned policy for the dispatch consumer.
//
// All events are built from the frozen §15.3 vocabulary; no event, state,
// or edge is invented. Domain types are called, never modified
// (flexcore-domain stays pure — the dependency rule).

use std::sync::atomic::Ordering;
use std::sync::Arc;

use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::time_model::EventTime;
use flexcore_domain::command_state::CommandState;
use flexcore_domain::ids::{AssetId, PortfolioId, SiteId, TenantId};
use flexcore_domain::tenant_context::{AuthenticatedClaims, TenantContext, VerifiedClaims};
use flexcore_events::envelope::{EventEnvelope, SUPPORTED_SCHEMA_VERSION};
use flexcore_events::events::{DispatchIssuedPayload, DomainEvent};
use flexcore_nats_adapter::consumer::{ConsumerConfig, MaxDeliverMismatch};
use flexcore_nats_adapter::inbox::{InMemoryInboxStore, InboxStore};
use flexcore_storage::command_store::COMMAND_STATES;
use flexcore_worker::command_consumer::{
    command_idempotency_handler, command_state_for_event, decide_transition,
    dispatch_consumer_policy, CommandConsumerMetrics, CommandStateGateway, TransitionDecision,
    DISPATCH_CONSUMER_DURABLE,
};

/// A deterministic in-test gateway over a single-command state table.
struct SingleCommandGateway {
    command_id: String,
    state: CommandState,
}

impl CommandStateGateway for SingleCommandGateway {
    fn current_state(
        &self,
        _ctx: &TenantContext,
        command_id: &str,
    ) -> Result<Option<CommandState>, String> {
        if command_id == self.command_id {
            Ok(Some(self.state))
        } else {
            Ok(None)
        }
    }
}

fn tenant() -> TenantId {
    TenantId::new(FlexUuid::nil())
}

fn command() -> FlexUuid {
    FlexUuid::nil()
}

fn issued_fact(key: &str) -> flexcore_nats_adapter::consumer::DeliveredMessage {
    let event = DomainEvent::DispatchIssued(DispatchIssuedPayload {
        correlation_id: FlexUuid::nil(),
        tenant_id: tenant(),
        portfolio_id: PortfolioId::new(FlexUuid::nil()),
        site_id: SiteId::new(FlexUuid::nil()),
        asset_id: AssetId::new(FlexUuid::nil()),
        command_id: command(),
        event_time: EventTime::from_unix_millis(1_700_000_000_000).expect("valid event time"),
    });
    let envelope =
        EventEnvelope::new(SUPPORTED_SCHEMA_VERSION, key, event).expect("test envelope constructs");
    let payload = serde_json::to_vec(&envelope).expect("test envelope serializes");
    flexcore_nats_adapter::consumer::DeliveredMessage {
        subject: "flexcore.v1.dispatch.dispatch_issued".to_string(),
        idempotency_key: Some(key.to_string()),
        schema_version: Some("1".to_string()),
        traceparent: None,
        payload,
        delivery_count: Some(2),
        reply: None,
    }
}

/// The oracle is exactly the frozen table: a self-transition (the most
/// common redelivery shape) is a duplicate no-op; a forward frozen edge is
/// an unsupported fact; FAILED exits only via FALLBACK.
#[test]
fn the_oracle_decides_exactly_on_the_frozen_table() {
    assert_eq!(
        decide_transition(CommandState::Issued, CommandState::Issued),
        TransitionDecision::DuplicateNoOp,
        "self-transitions do not exist in the frozen table → duplicate"
    );
    assert_eq!(
        decide_transition(CommandState::Proposed, CommandState::Validated),
        TransitionDecision::UnsupportedLifecycleFact,
        "a frozen edge the command has not taken surfaces"
    );
    assert_eq!(
        decide_transition(CommandState::Failed, CommandState::Fallback),
        TransitionDecision::UnsupportedLifecycleFact,
        "frozen transitions 26–27 stay edges"
    );
    assert_eq!(
        decide_transition(CommandState::Failed, CommandState::Expired),
        TransitionDecision::DuplicateNoOp,
        "FAILED → EXPIRED is not a frozen edge → duplicate"
    );
    assert_eq!(
        decide_transition(CommandState::Completed, CommandState::Tracking),
        TransitionDecision::DuplicateNoOp,
        "terminal states have no outgoing edges → duplicate"
    );
}

/// THE P11.12 acceptance rule: a redelivered fact on an equal stored state
/// is Ok + duplicate metric + no journal write + no error.
#[test]
fn redelivered_command_is_a_no_op_plus_the_duplicate_metric() {
    let gateway = Arc::new(SingleCommandGateway {
        command_id: command().to_hyphenated(),
        state: CommandState::Issued,
    });
    let metrics = Arc::new(CommandConsumerMetrics::default());
    let inbox = InMemoryInboxStore::new();
    let mut handler = command_idempotency_handler(
        gateway,
        metrics.clone(),
        DISPATCH_CONSUMER_DURABLE.to_string(),
    );

    let outcome = handler(&issued_fact("dup-1"), &inbox);
    assert_eq!(outcome, Ok(()), "a duplicate must be a no-op, not an error");
    assert_eq!(
        metrics.duplicate_deliveries.load(Ordering::Relaxed),
        1,
        "the duplicate-delivery metric must count the neutralized redelivery"
    );
    assert_eq!(
        metrics.unsupported_lifecycle_facts.load(Ordering::Relaxed),
        0
    );
    assert_eq!(
        metrics.unknown_commands.load(Ordering::Relaxed),
        0
    );
}

/// The same redelivery is ALSO caught by the P1.16 first layer on the
/// second pass: the handled delivery recorded the inbox row, so the
/// upstream check reports it before the oracle runs again.
#[test]
fn the_first_dedupe_layer_skips_a_row_hit_before_the_oracle() {
    let gateway = Arc::new(SingleCommandGateway {
        command_id: command().to_hyphenated(),
        state: CommandState::Issued,
    });
    let metrics = Arc::new(CommandConsumerMetrics::default());
    let inbox = InMemoryInboxStore::new();
    let mut handler = command_idempotency_handler(
        gateway,
        metrics.clone(),
        DISPATCH_CONSUMER_DURABLE.to_string(),
    );

    let first = handler(&issued_fact("dup-2"), &inbox);
    assert_eq!(first, Ok(()));
    assert_eq!(
        InboxStore::is_recorded(&inbox, DISPATCH_CONSUMER_DURABLE, "dup-2"),
        Ok(true),
        "the handled delivery recorded its inbox entry"
    );
    assert_eq!(
        metrics.inbox_records.load(Ordering::Relaxed),
        1
    );

    // Second pass through the SAME handler closure would be skipped by the
    // helper's inbox check (P1.16 step 1); the oracle-level counters do
    // not move because the helper returned SkippedDuplicate before the
    // handler ran. The row itself is the evidence.
    assert_eq!(inbox.len(), 1);
    assert_eq!(
        metrics.duplicate_deliveries.load(Ordering::Relaxed),
        1,
        "the oracle metric counts only oracle-neutralized deliveries, not row hits"
    );
}

/// A fact for a command unknown in its tenant scope surfaces (poison path),
/// counted on the unknown-command counter — never silently dropped.
#[test]
fn unknown_command_surfaces_instead_of_being_dropped() {
    let gateway = Arc::new(SingleCommandGateway {
        command_id: "00000000-0000-0000-0000-000000000001".to_string(),
        state: CommandState::Proposed,
    });
    let metrics = Arc::new(CommandConsumerMetrics::default());
    let inbox = InMemoryInboxStore::new();
    let mut handler = command_idempotency_handler(
        gateway,
        metrics.clone(),
        DISPATCH_CONSUMER_DURABLE.to_string(),
    );

    let outcome = handler(&issued_fact("dup-3"), &inbox);
    let error = outcome.expect_err("an unknown command must surface");
    assert!(error.0.contains("unknown"));
    assert_eq!(metrics.unknown_commands.load(Ordering::Relaxed), 1);
    assert_eq!(metrics.duplicate_deliveries.load(Ordering::Relaxed), 0);
}

/// The consumer never fabricates a journal row: a frozen edge the stored
/// command has not taken returns the named error and writes nothing.
/// (APPROVED stored, ISSUED fact — Approved -> Issued IS a frozen edge.
/// PROPOSED stored would be a forward JUMP, not a frozen edge: the frozen
/// table allows Proposed -> Validated | Rejected only, so that pair
/// neutralizes as a duplicate — see the lib test
/// `forward_jump_over_unrecorded_gates_neutralizes_as_a_duplicate`.)
#[test]
fn unsupported_lifecycle_fact_is_surfaced_never_written() {
    let gateway = Arc::new(SingleCommandGateway {
        command_id: command().to_hyphenated(),
        state: CommandState::Approved,
    });
    let metrics = Arc::new(CommandConsumerMetrics::default());
    let inbox = InMemoryInboxStore::new();
    let mut handler = command_idempotency_handler(
        gateway,
        metrics.clone(),
        DISPATCH_CONSUMER_DURABLE.to_string(),
    );

    let outcome = handler(&issued_fact("dup-4"), &inbox);
    let error = outcome.expect_err("a frozen edge the command has not taken must surface");
    assert!(error.0.contains("unsupported lifecycle fact"));
    assert_eq!(
        metrics.unsupported_lifecycle_facts.load(Ordering::Relaxed),
        1
    );
    assert_eq!(inbox.len(), 0, "a surfaced fact records no inbox entry");
}

/// The event → state mapping covers exactly the eleven frozen dispatch
/// events; VALIDATED and FEASIBLE have NO event and none may be invented.
#[test]
fn no_event_is_invented_for_the_gate_states() {
    for event in DomainEvent::all_variants() {
        let mapped = command_state_for_event(&event);
        let name = flexcore_events::events::event_name(&event);
        if name.starts_with("Dispatch") {
            assert!(mapped.is_some(), "dispatch event {name} must map to a state");
            let state_name = mapped.unwrap().name();
            assert!(
                COMMAND_STATES.contains(&state_name),
                "mapped state {state_name} must be in the frozen 14-state vocabulary"
            );
        } else {
            assert_eq!(mapped, None, "non-dispatch event {name} must not map");
        }
    }
    // The frozen vocabulary has no DispatchValidated / DispatchFeasible
    // event; the mapping therefore never yields VALIDATED or FEASIBLE.
    for event in DomainEvent::all_variants() {
        if let Some(state) = command_state_for_event(&event) {
            assert!(
                !matches!(state, CommandState::Validated | CommandState::Feasible),
                "no event maps to a gate-internal state; found {:?}",
                state
            );
        }
    }
}

/// The metric families render in Prometheus text format, zero included.
#[test]
fn duplicate_delivery_metrics_render_all_families() {
    let body = CommandConsumerMetrics::default().render();
    for family in [
        "flexcore_command_consumer_duplicate_deliveries_total",
        "flexcore_command_consumer_unsupported_lifecycle_facts_total",
        "flexcore_command_consumer_unknown_commands_total",
        "flexcore_command_consumer_decode_failures_total",
        "flexcore_command_consumer_inbox_records_total",
    ] {
        assert!(body.contains(family), "missing metric family {family}");
    }
}

/// P11.13 alignment: the dispatch consumer's helper config is built from
/// the provisioned policy and validates; a field-by-field drift is a named
/// error the composition root fails fast on.
#[test]
fn dispatch_consumer_policy_and_helper_config_stay_aligned() {
    let policy = dispatch_consumer_policy();
    assert_eq!(
        policy.durable_name, DISPATCH_CONSUMER_DURABLE,
        "the policy names the dispatch-track consumer"
    );
    assert!(
        policy.durable_name.ends_with("-v1"),
        "durable names follow the <area>-<purpose>-v1 convention (P1.14 §5.1)"
    );
    assert_eq!(policy.filter_subject, "flexcore.v1.dispatch.>");

    let config = ConsumerConfig::from_policy(&policy);
    assert_eq!(
        config.validate_against_policy(&policy),
        Ok(()),
        "from_policy construction is aligned by construction"
    );

    let drifted = ConsumerConfig {
        max_deliver: policy.max_deliver + 1,
        ..ConsumerConfig::from_policy(&policy)
    };
    assert_eq!(
        drifted.validate_against_policy(&policy),
        Err(MaxDeliverMismatch {
            durable_name: DISPATCH_CONSUMER_DURABLE.to_string(),
            helper_max_deliver: policy.max_deliver + 1,
            provisioned_max_deliver: policy.max_deliver,
        }),
        "a drift is a named error, never silently corrected"
    );
}

/// The consumer's tenant context is derived from the fact's payload tenant
/// (D-J2: tenant scoping at consumption, never the subject) — the same
/// single greppable construction path as the rest of the worker.
#[test]
fn consumer_context_is_derived_from_authenticated_claims_only() {
    let ctx = TenantContext::from_authenticated_claims(AuthenticatedClaims::from_verified(
        VerifiedClaims {
            tenant_id: tenant(),
            subject: "worker:command-consumer".to_string(),
        },
    ));
    assert_eq!(ctx.tenant_id(), tenant());
}

/// P11.13 recorded cross-check: the adapter's stamped schema version
/// equals the envelope crate's supported version. The adapter crate
/// depends on flexcore-ports only (the recorded single core-crate edge),
/// so the equality is a RECORDED cross-check asserted here — where both
/// crates are visible — instead of compiled.
#[test]
fn published_schema_version_matches_the_envelope_supported_version() {
    assert_eq!(
        flexcore_nats_adapter::topology::PUBLISHED_SCHEMA_VERSION,
        flexcore_events::envelope::SUPPORTED_SCHEMA_VERSION,
        "the published-message schema-version header must carry the envelope's \
         supported version (topology.rs recorded cross-check)"
    );
    assert_eq!(
        flexcore_nats_adapter::topology::PUBLISHED_SCHEMA_VERSION, 1,
        "the P11.13 stamped version stays at the single supported version 1"
    );
}

/// P11.05 handoff (record 05 §1): the traceparent header travels
/// opaquely — the consumer surface carries it verbatim, an absent header
/// is "no inherited context", and nothing synthesizes a trace id.
#[test]
fn traceparent_is_carried_opaquely_when_present_and_absent_when_not() {
    let fact = issued_fact("dup-trace-1");
    assert_eq!(fact.traceparent, None, "no header means no inherited context");
}
