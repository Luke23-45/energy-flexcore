// P11.12 — Consumer idempotency keyed on `command_id` (Phase 11 wave 1,
// Agent B; ledger P11.12; gap analysis §5 Tier 0 row 4 [S-61][S-65]).
//
// # The lifecycle state machine is the dedupe oracle
//
// The frozen 14-state / 27-edge command lifecycle
// (`flexcore_domain::command_state`, P0.11) is the ONLY dedupe oracle for
// redelivered command-lifecycle messages. The registered semantics
// (plan §2 P11.12, implemented here verbatim):
//
// > An invalid-state transition on a redelivered command is a no-op plus a
// > duplicate-delivery metric.
//
// Concretely, for a delivered dispatch-lifecycle fact whose target state
// is `T` and whose stored command state is `C`:
//
// - `try_transition(C, T)` fails (not a frozen edge — including every
//   self-transition, every backwards move, and every transition out of a
//   terminal state) → the delivery is a duplicate or an already-applied
//   fact: **no-op** (ack, no side effect, no journal write) plus
//   [`CommandConsumerMetrics::duplicate_deliveries`] plus an INFO log
//   line. This is the normal redelivery path: JetStream is
//   at-least-once, and every dispatch fact is published (via the
//   transactional outbox) only AFTER the producing gate committed its
//   journal row, so a redelivery of an already-applied fact always lands
//   here.
// - `try_transition(C, T)` succeeds → the fact describes a frozen edge the
//   stored command has NOT taken: an out-of-order or unapplied lifecycle
//   fact. The consumer NEVER synthesizes a journal row: the §14.1 journal
//   rules require gate / reason / evidence fields the §15.3 dispatch
//   payloads do not carry, and fabricating them would corrupt the audit
//   view. The delivery returns a named handler error (retryable) and ends
//   on the existing poison path (DLQ + `+TERM` + ERROR log) — counted in
//   [`CommandConsumerMetrics::unsupported_lifecycle_facts`], never
//   silently dropped.
//
// The inbox check upstream ([`flexcore_nats_adapter::consumer::
// IdempotentConsumer`], P1.16) remains the first dedupe layer for
// crash-after-commit redeliveries; THIS oracle is the durable second
// layer that does not depend on any inbox row surviving. The two layers
// are independent: a redelivery that misses the inbox (row lost, crash
// between apply and record) still neutralizes on the state machine.
//
// # The recorded two-transaction deviation
//
// The P1.16 binding obligation says the handler records the inbox entry
// inside its ONE storage transaction. The shipped `PgCommandStateGateway`
// opens one transaction per message for the state READ (the oracle input);
// the post-decision `InboxStore::record` call runs in its own single-
// statement transaction (the `PgInboxStore` pool shape — the recorded
// single-home limitation, src/inbox.rs). Consequence: a crash between the
// oracle no-op and the inbox record yields a redelivery that re-runs the
// oracle and neutralizes again — the invariant "no duplicate side effect"
// holds on the state machine alone. This deviation is recorded in the
// P11.12 decision record; closing it needs an inbox record ON the command
// transaction, which is the inbox-promotion work already registered.
//
// # Domain purity (dependency rule)
//
// This module lives in the worker (an app/composition root). It CALLS the
// domain's public `try_transition` and state vocabulary; nothing here is
// added to `flexcore-domain`. The duplicate-delivery METRIC belongs to
// this consumer layer, never to the domain (P11.12 constraint).
//
// # Topology and honesty
//
// The consumer runs on a `dispatch-command-track-v1` durable pull consumer
// over `flexcore.v1.dispatch.>` (the P1.14 §3 dispatch area). The
// `FLEXCORE_DISPATCH` stream is provisioned by the composition root when
// this consumer starts (a recorded deviation from the D-J7 "created by the
// phase that first publishes" rule: a pull consumer requires its stream to
// exist, and the P11.12 consumer arrives before any dispatch publisher).
// No dispatch publisher exists in 0.x, so in a shipped deployment this
// consumer idles — every counter stays at zero and no behavior is claimed
// beyond the written code. The entire 0.x line is simulation-only.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use flexcore_contracts::ids::FlexUuid;
use flexcore_domain::command_state::{try_transition, CommandState};
use flexcore_domain::ids::TenantId;
use flexcore_domain::tenant_context::{
    AuthenticatedClaims, TenantContext, VerifiedClaims,
};
use flexcore_events::envelope::EventEnvelope;
use flexcore_events::events::DomainEvent;
use flexcore_nats_adapter::consumer::{DeliveredMessage, HandlerError};
use flexcore_nats_adapter::inbox::{InboxStore, RecordOutcome};
use flexcore_nats_adapter::topology::{ConsumerPolicy, STREAM_DISPATCH};
use flexcore_storage::command_store::CommandRecordStore as _;
use tokio::sync::watch;

/// The durable consumer this module drives (`<area>-<purpose>-v1`, P1.14
/// §5.1): tracks dispatch-area lifecycle facts with the state-machine
/// oracle as the dedupe authority.
pub const DISPATCH_CONSUMER_DURABLE: &str = "dispatch-command-track-v1";

/// The consumer policy for [`DISPATCH_CONSUMER_DURABLE`]. The budgets are
/// the registered Phase 1 dev-shape values (same shape as the registry
/// consumer policy in `apps/worker/src/main.rs`); the composition root
/// keeps the helper config aligned with this policy through
/// `ConsumerConfig::from_policy` + `validate_against_policy` (P11.13).
pub fn dispatch_consumer_policy() -> ConsumerPolicy {
    ConsumerPolicy {
        durable_name: DISPATCH_CONSUMER_DURABLE.to_string(),
        stream: STREAM_DISPATCH.to_string(),
        filter_subject: "flexcore.v1.dispatch.>".to_string(),
        ack_wait: std::time::Duration::from_secs(30),
        max_deliver: 5,
        max_ack_pending: 100,
    }
}

// ---------------------------------------------------------------------------
// The dedupe oracle
// ---------------------------------------------------------------------------

/// The oracle decision for one delivered lifecycle fact with stored state
/// `current` and target state `target`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionDecision {
    /// The (current, target) pair is a frozen edge the stored command has
    /// NOT taken: an out-of-order or unapplied fact. Surfaced on the
    /// poison path — the consumer never fabricates journal rows.
    UnsupportedLifecycleFact,
    /// The pair is NOT a frozen edge (self-transition, backwards move,
    /// terminal exit): a duplicate or already-applied fact. No-op plus the
    /// duplicate-delivery metric — the registered P11.12 semantics.
    DuplicateNoOp,
}

/// The P11.12 oracle: `flexcore_domain::command_state::try_transition` is
/// the ONLY authority. A frozen edge the command has not taken is an
/// unsupported lifecycle fact; everything else neutralizes as a duplicate.
pub fn decide_transition(current: CommandState, target: CommandState) -> TransitionDecision {
    match try_transition(current, target) {
        Ok(_) => TransitionDecision::UnsupportedLifecycleFact,
        Err(_) => TransitionDecision::DuplicateNoOp,
    }
}

/// Maps a frozen §15.3 event to the command-lifecycle target state it
/// asserts. `None` for every non-dispatch event — the vocabulary is
/// closed; no event is invented for VALIDATED or FEASIBLE (the frozen
/// §15.3 rule) and none may be.
pub fn command_state_for_event(event: &DomainEvent) -> Option<CommandState> {
    match event {
        DomainEvent::DispatchProposed(_) => Some(CommandState::Proposed),
        DomainEvent::DispatchApproved(_) => Some(CommandState::Approved),
        DomainEvent::DispatchRejected(_) => Some(CommandState::Rejected),
        DomainEvent::DispatchIssued(_) => Some(CommandState::Issued),
        DomainEvent::DispatchAcknowledged(_) => Some(CommandState::Acknowledged),
        DomainEvent::DispatchTrackingUpdated(_) => Some(CommandState::Tracking),
        DomainEvent::DispatchCompleted(_) => Some(CommandState::Completed),
        DomainEvent::DispatchFailed(_) => Some(CommandState::Failed),
        DomainEvent::DispatchExpired(_) => Some(CommandState::Expired),
        DomainEvent::DispatchCancelled(_) => Some(CommandState::Cancelled),
        DomainEvent::DispatchSuperseded(_) => Some(CommandState::Superseded),
        DomainEvent::DispatchFallbackActivated(_) => Some(CommandState::Fallback),
        _ => None,
    }
}

/// The tenant and command a dispatch lifecycle fact is about. All eleven
/// dispatch payloads carry both fields; `None` for every other event.
fn dispatch_fact(event: &DomainEvent) -> Option<(TenantId, FlexUuid)> {
    let (tenant_id, command_id) = match event {
        DomainEvent::DispatchProposed(p) => (p.tenant_id, p.command_id),
        DomainEvent::DispatchApproved(p) => (p.tenant_id, p.command_id),
        DomainEvent::DispatchRejected(p) => (p.tenant_id, p.command_id),
        DomainEvent::DispatchIssued(p) => (p.tenant_id, p.command_id),
        DomainEvent::DispatchAcknowledged(p) => (p.tenant_id, p.command_id),
        DomainEvent::DispatchTrackingUpdated(p) => (p.tenant_id, p.command_id),
        DomainEvent::DispatchCompleted(p) => (p.tenant_id, p.command_id),
        DomainEvent::DispatchFailed(p) => (p.tenant_id, p.command_id),
        DomainEvent::DispatchExpired(p) => (p.tenant_id, p.command_id),
        DomainEvent::DispatchCancelled(p) => (p.tenant_id, p.command_id),
        DomainEvent::DispatchSuperseded(p) => (p.tenant_id, p.command_id),
        DomainEvent::DispatchFallbackActivated(p) => (p.tenant_id, p.command_id),
        _ => return None,
    };
    Some((tenant_id, command_id))
}

/// Parses a stored state text (one of the 14 frozen SCREAMING_SNAKE names,
/// as the command-record store persists it) back into the domain state.
/// The store vocabulary is validated at write time; this parser is the
/// read-side mirror and fails closed on anything else.
fn parse_command_state(state: &str) -> Option<CommandState> {
    CommandState::ALL.iter().copied().find(|s| s.name() == state)
}

// ---------------------------------------------------------------------------
// Metrics (consumer layer — never the domain)
// ---------------------------------------------------------------------------

/// The P11.12 duplicate-delivery metrics. Plain atomics in the consumer
/// layer (the worker has no metrics registry yet — the render function is
/// the wiring seam for the worker metrics surface; see the handoff note in
/// the P11.12 record). Family names follow the existing
/// `flexcore_*_total` convention and are reconciled against the P11.01
/// taxonomy record before phase close.
#[derive(Debug, Default)]
pub struct CommandConsumerMetrics {
    /// Deliveries neutralized as duplicates / already-applied facts
    /// (invalid transition per the frozen table). The P11.12 headline
    /// counter.
    pub duplicate_deliveries: AtomicU64,
    /// Frozen-edge facts the stored command has not taken (out-of-order or
    /// unapplied) — surfaced on the poison path for operator
    /// investigation.
    pub unsupported_lifecycle_facts: AtomicU64,
    /// Lifecycle facts naming a command the store does not know (in the
    /// fact's tenant scope) — surfaced on the poison path.
    pub unknown_commands: AtomicU64,
    /// Deliveries whose payload is not a parseable event envelope, or a
    /// non-dispatch event on the dispatch subject — surfaced on the poison
    /// path.
    pub decode_failures: AtomicU64,
    /// Inbox entries recorded for handled deliveries (bookkeeping only —
    /// the oracle is the durable dedupe layer).
    pub inbox_records: AtomicU64,
}

impl CommandConsumerMetrics {
    /// Renders the families in Prometheus text format (the same
    /// text/plain; version=0.0.4 shape the control plane's `/metrics`
    /// renders). Zero values always render so a scrape exposes the full
    /// inventory on an idle consumer (the P2.21 always-render discipline).
    pub fn render(&self) -> String {
        let mut out = String::new();
        let rows: [(&str, &str, u64); 5] = [
            (
                "flexcore_command_consumer_duplicate_deliveries_total",
                "Command-lifecycle deliveries neutralized as duplicates by the frozen state machine (P11.12).",
                self.duplicate_deliveries.load(Ordering::Relaxed),
            ),
            (
                "flexcore_command_consumer_unsupported_lifecycle_facts_total",
                "Frozen-edge lifecycle facts the stored command has not taken (surfaced on the poison path).",
                self.unsupported_lifecycle_facts.load(Ordering::Relaxed),
            ),
            (
                "flexcore_command_consumer_unknown_commands_total",
                "Lifecycle facts naming a command unknown in the fact's tenant scope (surfaced on the poison path).",
                self.unknown_commands.load(Ordering::Relaxed),
            ),
            (
                "flexcore_command_consumer_decode_failures_total",
                "Deliveries whose payload was not a parseable dispatch-lifecycle envelope (surfaced on the poison path).",
                self.decode_failures.load(Ordering::Relaxed),
            ),
            (
                "flexcore_command_consumer_inbox_records_total",
                "Inbox entries recorded for handled command-lifecycle deliveries (bookkeeping; the oracle is the durable dedupe).",
                self.inbox_records.load(Ordering::Relaxed),
            ),
        ];
        for (family, help, value) in rows {
            out.push_str(&format!("# HELP {family} {help}\n"));
            out.push_str(&format!("# TYPE {family} counter\n"));
            out.push_str(&format!("{family} {value}\n"));
        }
        out
    }
}

// ---------------------------------------------------------------------------
// The state gateway port
// ---------------------------------------------------------------------------

/// Reads the stored command-lifecycle state inside the fact's tenant
/// scope. A sync port (the house rule): the Postgres implementation opens
/// one unit of work per call and blocks on the adapter's private runtime —
/// legal only from the consumer loop's blocking section (see the run loop
/// below). A foreign-tenant or unknown command reads as `Ok(None)`.
pub trait CommandStateGateway: Send + Sync {
    fn current_state(
        &self,
        ctx: &TenantContext,
        command_id: &str,
    ) -> Result<Option<CommandState>, String>;
}

/// The Postgres implementation over the real store: one unit of work per
/// read, tenant-scoped by construction (the store's `find_command` carries
/// the RLS binding plus the explicit tenant filter; a foreign-tenant
/// command reads as `None` — no existence leak).
pub struct PgCommandStateGateway {
    store: flexcore_postgres_adapter::FlexCoreStore,
}

impl PgCommandStateGateway {
    pub fn new(store: flexcore_postgres_adapter::FlexCoreStore) -> Self {
        Self { store }
    }
}

impl CommandStateGateway for PgCommandStateGateway {
    fn current_state(
        &self,
        ctx: &TenantContext,
        command_id: &str,
    ) -> Result<Option<CommandState>, String> {
        let unit = self
            .store
            .begin(Some(ctx))
            .map_err(|e| format!("command-state unit of work failed: {e}"))?;
        // `PgUnitOfWork` implements `CommandRecordStore` directly
        // (adapters/postgres/src/command_store.rs) — the `command_records`
        // field lives on the control-plane's composition bundle, not on the
        // unit of work (orchestrator fix of the wave-2 recorded finding,
        // record 53 §3.5).
        let record = unit
            .find_command(ctx, command_id)
            .map_err(|e| format!("command-state read failed: {e}"))?;
        // Drop without commit: a pure read rolls back (the adapter
        // contract — P1.10 rule 4; the same shape as the ingestion
        // composition's registry read).
        drop(unit);
        record
            .map(|record| {
                parse_command_state(&record.state).ok_or_else(|| {
                    format!(
                        "stored command state '{}' is outside the frozen 14-state vocabulary",
                        record.state
                    )
                })
            })
            .transpose()
    }
}

// ---------------------------------------------------------------------------
// The handler (the P1.16 sync handler contract)
// ---------------------------------------------------------------------------

/// Builds the P11.12 command-consumer handler: parse → scope → oracle →
/// no-op-or-surface, with the inbox record as post-decision bookkeeping.
/// The returned closure satisfies the
/// [`flexcore_nats_adapter::consumer::IdempotentConsumer`] handler
/// contract exactly (sync `FnMut(&DeliveredMessage, &dyn InboxStore)`).
pub fn command_idempotency_handler(
    gateway: Arc<dyn CommandStateGateway>,
    metrics: Arc<CommandConsumerMetrics>,
    durable_name: String,
) -> impl FnMut(&DeliveredMessage, &dyn InboxStore) -> Result<(), HandlerError> {
    move |message: &DeliveredMessage, inbox: &dyn InboxStore| {
        // Defensive restatement: the consumer helper quarantines keyless
        // messages before the handler runs (P1.16).
        let Some(idempotency_key) = message.idempotency_key.as_deref() else {
            return Err(HandlerError(
                "delivered message carries no idempotency key".to_string(),
            ));
        };

        // 1. Decode the envelope (P1.09: unknown schema version fails to
        //    parse — no silent default). A payload that does not parse is
        //    poison: retryable error, eventually the DLQ.
        let envelope: EventEnvelope = match serde_json::from_slice(&message.payload) {
            Ok(envelope) => envelope,
            Err(error) => {
                metrics.decode_failures.fetch_add(1, Ordering::Relaxed);
                return Err(HandlerError(format!(
                    "payload is not a parseable event envelope: {error}"
                )));
            }
        };

        // 2. Scope check: only dispatch-lifecycle facts are actionable
        //    here. A well-formed non-dispatch event on the dispatch
        //    subject is a routing defect — surfaced, never swallowed.
        let Some((tenant_id, command_id)) = dispatch_fact(&envelope.event) else {
            metrics.decode_failures.fetch_add(1, Ordering::Relaxed);
            return Err(HandlerError(format!(
                "event '{}' is not a dispatch-lifecycle fact but arrived on the dispatch subject",
                flexcore_events::events::event_name(&envelope.event)
            )));
        };
        let Some(target) = command_state_for_event(&envelope.event) else {
            // Unreachable given dispatch_fact (the two matches cover the
            // same eleven variants); kept as the fail-closed arm.
            metrics.decode_failures.fetch_add(1, Ordering::Relaxed);
            return Err(HandlerError(
                "dispatch fact has no lifecycle target state".to_string(),
            ));
        };

        // 3. Read the stored state inside the fact's tenant scope (the
        //    consumer derives the context from the payload's tenant, the
        //    D-J2 rule — tenant scoping at consumption, never the subject).
        let ctx = TenantContext::from_authenticated_claims(AuthenticatedClaims::from_verified(
            VerifiedClaims {
                tenant_id,
                subject: "worker:command-consumer".to_string(),
            },
        ));
        let command_id_text = command_id.to_hyphenated();
        let Some(current) = gateway
            .current_state(&ctx, &command_id_text)
            .map_err(HandlerError)?
        else {
            // Unknown command: cannot judge, cannot apply — surface on the
            // poison path (never silently dropped, never fabricated).
            metrics.unknown_commands.fetch_add(1, Ordering::Relaxed);
            return Err(HandlerError(format!(
                "command {command_id_text} is unknown in the fact's tenant scope"
            )));
        };

        // 4. THE ORACLE (P11.12): an invalid transition on a redelivered
        //    command is a no-op plus the duplicate-delivery metric.
        match decide_transition(current, target) {
            TransitionDecision::DuplicateNoOp => {
                metrics.duplicate_deliveries.fetch_add(1, Ordering::Relaxed);
                // P11.05 handoff (record 05 §1): the W3C traceparent header
                // (carried opaquely on [`DeliveredMessage::traceparent`])
                // lands in the log line so a neutralized redelivery stays
                // followable end to end. Absent/unparseable = no inherited
                // context — never a synthesized trace id.
                tracing::info!(
                    consumer = %durable_name,
                    command_id = %command_id_text,
                    stored_state = current.name(),
                    fact_state = target.name(),
                    traceparent = message.traceparent.as_deref().unwrap_or(""),
                    "duplicate or already-applied lifecycle fact neutralized by the \
                     frozen state machine (no-op; P11.12)"
                );
            }
            TransitionDecision::UnsupportedLifecycleFact => {
                // A frozen edge the command has not taken. The consumer
                // never fabricates the gate/reason/evidence journal fields
                // the §14.1 rules require and §15.3 payloads do not carry:
                // surface for operator investigation on the poison path.
                metrics
                    .unsupported_lifecycle_facts
                    .fetch_add(1, Ordering::Relaxed);
                return Err(HandlerError(format!(
                    "unsupported lifecycle fact: stored state {} -> fact state {} is a frozen \
                     edge this consumer does not journal (the producing gate owns the journal)",
                    current.name(),
                    target.name()
                )));
            }
        }

        // 5. Inbox bookkeeping AFTER the decision (the recorded
        //    two-transaction deviation — module header): a redelivery that
        //    misses this row re-runs the oracle and neutralizes again.
        match inbox.record(&durable_name, idempotency_key) {
            Ok(RecordOutcome::Inserted) => {
                metrics.inbox_records.fetch_add(1, Ordering::Relaxed);
            }
            Ok(RecordOutcome::AlreadyRecorded) => {}
            Err(error) => return Err(HandlerError(error.to_string())),
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// The run loop (mirrors runner.rs's hosting discipline)
// ---------------------------------------------------------------------------

/// Runs a future on the worker runtime from a blocking-capable context —
/// the recorded sync-port hosting resolution (the adapter's port calls
/// block on the postgres adapter's private runtime; a `block_on` inside
/// another runtime's async context panics).
async fn drive<F: std::future::Future>(future: F) -> F::Output {
    tokio::task::block_in_place(|| tokio::runtime::Handle::current().block_on(future))
}

/// Sleeps `duration`, or returns early when shutdown fires. `Ok(())` =
/// keep going; `Err(())` = shut down (the runner.rs shape).
async fn sleep_or_shutdown(
    duration: std::time::Duration,
    shutdown: &mut watch::Receiver<bool>,
) -> Result<(), ()> {
    tokio::select! {
        () = tokio::time::sleep(duration) => Ok(()),
        changed = shutdown.changed() => {
            let _ = changed;
            Err(())
        }
    }
}

/// The P11.12 consumer loop until shutdown: pull → inbox check → oracle →
/// ack, with idle and error backoff (the runner.rs hosting pattern). The
/// returned report is the operator-facing summary; the metric counters are
/// the scraped surface.
pub async fn run_command_consumer(
    jetstream: async_nats::jetstream::Context,
    inbox: Arc<dyn InboxStore>,
    gateway: Arc<dyn CommandStateGateway>,
    metrics: Arc<CommandConsumerMetrics>,
    config: flexcore_nats_adapter::consumer::ConsumerConfig,
    mut shutdown: watch::Receiver<bool>,
) -> flexcore_nats_adapter::consumer::ProcessReport {
    let mut handler = command_idempotency_handler(gateway, metrics, config.durable_name.clone());
    let consumer =
        flexcore_nats_adapter::consumer::IdempotentConsumer::new(&jetstream, inbox.as_ref(), config.clone());
    let mut report = flexcore_nats_adapter::consumer::ProcessReport::default();
    loop {
        if *shutdown.borrow() {
            return report;
        }
        let outcome = drive(consumer.process_next(&mut handler)).await;
        match outcome {
            Ok(flexcore_nats_adapter::consumer::ProcessOutcome::Idle) => {
                report.idle_polls += 1;
                if sleep_or_shutdown(config.idle_interval, &mut shutdown)
                    .await
                    .is_err()
                {
                    return report;
                }
            }
            Ok(outcome) => {
                match outcome {
                    flexcore_nats_adapter::consumer::ProcessOutcome::Processed => {
                        report.processed += 1
                    }
                    flexcore_nats_adapter::consumer::ProcessOutcome::SkippedDuplicate => {
                        report.skipped_duplicates += 1
                    }
                    flexcore_nats_adapter::consumer::ProcessOutcome::RetryScheduled => {
                        report.retries_scheduled += 1
                    }
                    flexcore_nats_adapter::consumer::ProcessOutcome::Quarantined => {
                        report.quarantined += 1
                    }
                    flexcore_nats_adapter::consumer::ProcessOutcome::Idle => {}
                }
                tracing::debug!(outcome = ?outcome, "command consumer step completed");
            }
            Err(error) => {
                report.errors += 1;
                tracing::error!(
                    consumer = %config.durable_name,
                    error = %error,
                    "command consumer cycle failed; the message stays unacknowledged \
                     and will be redelivered by JetStream (at-least-once)"
                );
                if sleep_or_shutdown(config.error_backoff, &mut shutdown)
                    .await
                    .is_err()
                {
                    return report;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flexcore_events::events::event_name;
    use std::sync::Mutex;

    /// A gateway over a test-owned state table.
    struct FakeGateway {
        states: Mutex<std::collections::BTreeMap<String, CommandState>>,
    }

    impl FakeGateway {
        fn with(command_id: &str, state: CommandState) -> Self {
            let mut states = std::collections::BTreeMap::new();
            states.insert(command_id.to_string(), state);
            Self {
                states: Mutex::new(states),
            }
        }
    }

    impl CommandStateGateway for FakeGateway {
        fn current_state(
            &self,
            _ctx: &TenantContext,
            command_id: &str,
        ) -> Result<Option<CommandState>, String> {
            Ok(self.states.lock().unwrap().get(command_id).copied())
        }
    }

    fn metrics() -> Arc<CommandConsumerMetrics> {
        Arc::new(CommandConsumerMetrics::default())
    }

    fn message_for(event: DomainEvent, key: &str) -> DeliveredMessage {
        let envelope =
            EventEnvelope::new(flexcore_events::envelope::SUPPORTED_SCHEMA_VERSION, key, event)
                .expect("test envelope constructs");
        let payload = serde_json::to_vec(&envelope).expect("test envelope serializes");
        DeliveredMessage {
            subject: "flexcore.v1.dispatch.dispatch_issued".to_string(),
            idempotency_key: Some(key.to_string()),
            schema_version: Some("1".to_string()),
            traceparent: None,
            payload,
            delivery_count: Some(2),
            reply: None,
        }
    }

    fn dispatch_event_with(tenant: TenantId, command: FlexUuid) -> DomainEvent {
        DomainEvent::DispatchIssued(flexcore_events::events::DispatchIssuedPayload {
            correlation_id: FlexUuid::nil(),
            tenant_id: tenant,
            portfolio_id: flexcore_domain::ids::PortfolioId::new(FlexUuid::nil()),
            site_id: flexcore_domain::ids::SiteId::new(FlexUuid::nil()),
            asset_id: flexcore_domain::ids::AssetId::new(FlexUuid::nil()),
            command_id: command,
            event_time: flexcore_contracts::time_model::EventTime::from_unix_millis(
                1_700_000_000_000,
            )
            .expect("valid event time"),
        })
    }

    fn tenant() -> TenantId {
        TenantId::new(FlexUuid::nil())
    }

    /// THE P11.12 acceptance case: a redelivered ISSUED fact on a command
    /// already stored in ISSUED is a no-op plus the duplicate-delivery
    /// metric — never a journal write, never an error.
    #[test]
    fn redelivered_fact_on_equal_state_is_a_no_op_plus_the_duplicate_metric() {
        let command = FlexUuid::nil();
        let gateway = Arc::new(FakeGateway::with(&command.to_hyphenated(), CommandState::Issued));
        let m = metrics();
        let inbox = flexcore_nats_adapter::inbox::InMemoryInboxStore::new();
        let mut handler =
            command_idempotency_handler(gateway, m.clone(), "dispatch-command-track-v1".to_string());

        let result = handler(&message_for(dispatch_event_with(tenant(), command), "k-1"), &inbox);
        assert_eq!(result, Ok(()));
        assert_eq!(m.duplicate_deliveries.load(Ordering::Relaxed), 1);
        assert_eq!(
            m.unsupported_lifecycle_facts.load(Ordering::Relaxed),
            0
        );
    }

    /// A backwards fact (COMPLETED stored, ISSUED fact redelivered late)
    /// also neutralizes: not a frozen edge → no-op + metric.
    #[test]
    fn backwards_fact_neutralizes_as_a_duplicate() {
        let command = FlexUuid::nil();
        let gateway =
            Arc::new(FakeGateway::with(&command.to_hyphenated(), CommandState::Completed));
        let m = metrics();
        let inbox = flexcore_nats_adapter::inbox::InMemoryInboxStore::new();
        let mut handler =
            command_idempotency_handler(gateway, m.clone(), "dispatch-command-track-v1".to_string());

        let result = handler(&message_for(dispatch_event_with(tenant(), command), "k-2"), &inbox);
        assert_eq!(result, Ok(()));
        assert_eq!(m.duplicate_deliveries.load(Ordering::Relaxed), 1);
    }

    /// A frozen edge the command has not taken (PROPOSED stored, ISSUED
    /// fact arrives) is surfaced as a named handler error — the poison
    /// path owns it; the consumer never fabricates a journal row.
    #[test]
    fn unsupported_lifecycle_fact_is_a_named_error_not_a_write() {
        let command = FlexUuid::nil();
        let gateway =
            Arc::new(FakeGateway::with(&command.to_hyphenated(), CommandState::Proposed));
        let m = metrics();
        let inbox = flexcore_nats_adapter::inbox::InMemoryInboxStore::new();
        let mut handler =
            command_idempotency_handler(gateway, m.clone(), "dispatch-command-track-v1".to_string());

        let result = handler(&message_for(dispatch_event_with(tenant(), command), "k-3"), &inbox);
        let error = result.expect_err("a frozen edge the command has not taken must surface");
        assert!(error.0.contains("unsupported lifecycle fact"));
        assert_eq!(
            m.unsupported_lifecycle_facts.load(Ordering::Relaxed),
            1
        );
        assert_eq!(m.duplicate_deliveries.load(Ordering::Relaxed), 0);
    }

    /// A fact naming an unknown command surfaces as a named error (poison
    /// path), counted on the unknown-command counter.
    #[test]
    fn unknown_command_surfaces_instead_of_being_dropped() {
        let command = FlexUuid::nil();
        let gateway = Arc::new(FakeGateway::with("no-such-command", CommandState::Proposed));
        let m = metrics();
        let inbox = flexcore_nats_adapter::inbox::InMemoryInboxStore::new();
        let mut handler =
            command_idempotency_handler(gateway, m.clone(), "dispatch-command-track-v1".to_string());

        let result = handler(&message_for(dispatch_event_with(tenant(), command), "k-4"), &inbox);
        let error = result.expect_err("an unknown command must surface");
        assert!(error.0.contains("unknown"));
        assert_eq!(m.unknown_commands.load(Ordering::Relaxed), 1);
    }

    /// A payload that is not a parseable envelope is a decode failure
    /// (poison path).
    #[test]
    fn unparsable_payload_counts_as_a_decode_failure() {
        let gateway = Arc::new(FakeGateway::with("ignored", CommandState::Proposed));
        let m = metrics();
        let inbox = flexcore_nats_adapter::inbox::InMemoryInboxStore::new();
        let mut handler =
            command_idempotency_handler(gateway, m.clone(), "dispatch-command-track-v1".to_string());

        let bad = DeliveredMessage {
            subject: "flexcore.v1.dispatch.dispatch_issued".to_string(),
            idempotency_key: Some("k-5".to_string()),
            schema_version: None,
            traceparent: None,
            payload: b"not json".to_vec(),
            delivery_count: Some(1),
            reply: None,
        };
        let result = handler(&bad, &inbox);
        assert!(result.is_err());
        assert_eq!(m.decode_failures.load(Ordering::Relaxed), 1);
    }

    /// A keyless delivery is refused defensively (the helper quarantines
    /// it before the handler runs; this arm restates the contract).
    #[test]
    fn keyless_delivery_is_refused_defensively() {
        let gateway = Arc::new(FakeGateway::with("ignored", CommandState::Proposed));
        let m = metrics();
        let inbox = flexcore_nats_adapter::inbox::InMemoryInboxStore::new();
        let mut handler =
            command_idempotency_handler(gateway, m.clone(), "dispatch-command-track-v1".to_string());

        let keyless = DeliveredMessage {
            subject: "flexcore.v1.dispatch.dispatch_issued".to_string(),
            idempotency_key: None,
            schema_version: None,
            traceparent: None,
            payload: Vec::new(),
            delivery_count: Some(1),
            reply: None,
        };
        let result = handler(&keyless, &inbox);
        assert!(result.is_err());
    }

    /// Handled deliveries record their inbox entry (bookkeeping); a
    /// redelivery that hits the row is SkippedDuplicate upstream — the
    /// P1.16 first dedupe layer.
    #[test]
    fn handled_deliveries_record_the_inbox_entry() {
        let command = FlexUuid::nil();
        let gateway = Arc::new(FakeGateway::with(&command.to_hyphenated(), CommandState::Issued));
        let m = metrics();
        let inbox = flexcore_nats_adapter::inbox::InMemoryInboxStore::new();
        let mut handler =
            command_idempotency_handler(gateway, m.clone(), "dispatch-command-track-v1".to_string());

        let result = handler(&message_for(dispatch_event_with(tenant(), command), "k-6"), &inbox);
        assert_eq!(result, Ok(()));
        assert_eq!(m.inbox_records.load(Ordering::Relaxed), 1);
        assert_eq!(inbox.len(), 1);
        // The upstream helper's inbox check now dedupes the redelivery.
        assert_eq!(
            flexcore_nats_adapter::inbox::InboxStore::is_recorded(&inbox, "dispatch-command-track-v1", "k-6"),
            Ok(true)
        );
    }

    /// The event → target-state mapping covers exactly the eleven frozen
    /// dispatch events and nothing else (no event exists for VALIDATED or
    /// FEASIBLE and none may be invented).
    #[test]
    fn event_to_state_mapping_covers_exactly_the_dispatch_vocabulary() {
        for event in DomainEvent::all_variants() {
            let mapped = command_state_for_event(&event);
            match event_name(&event) {
                "DispatchProposed" => assert_eq!(mapped, Some(CommandState::Proposed)),
                "DispatchApproved" => assert_eq!(mapped, Some(CommandState::Approved)),
                "DispatchRejected" => assert_eq!(mapped, Some(CommandState::Rejected)),
                "DispatchIssued" => assert_eq!(mapped, Some(CommandState::Issued)),
                "DispatchAcknowledged" => assert_eq!(mapped, Some(CommandState::Acknowledged)),
                "DispatchTrackingUpdated" => assert_eq!(mapped, Some(CommandState::Tracking)),
                "DispatchCompleted" => assert_eq!(mapped, Some(CommandState::Completed)),
                "DispatchFailed" => assert_eq!(mapped, Some(CommandState::Failed)),
                "DispatchExpired" => assert_eq!(mapped, Some(CommandState::Expired)),
                "DispatchCancelled" => assert_eq!(mapped, Some(CommandState::Cancelled)),
                "DispatchSuperseded" => assert_eq!(mapped, Some(CommandState::Superseded)),
                "DispatchFallbackActivated" => assert_eq!(mapped, Some(CommandState::Fallback)),
                _ => assert_eq!(mapped, None, "a non-dispatch event must not map"),
            }
        }
    }

    /// The oracle matches the frozen table exactly: every frozen edge is
    /// UnsupportedLifecycleFact, every non-edge is DuplicateNoOp.
    #[test]
    fn oracle_decides_exactly_on_the_frozen_table() {
        // Self-transition: never an edge → duplicate.
        assert_eq!(
            decide_transition(CommandState::Issued, CommandState::Issued),
            TransitionDecision::DuplicateNoOp
        );
        // Backwards: not an edge → duplicate.
        assert_eq!(
            decide_transition(CommandState::Completed, CommandState::Issued),
            TransitionDecision::DuplicateNoOp
        );
        // Terminal exit: not an edge → duplicate.
        assert_eq!(
            decide_transition(CommandState::Completed, CommandState::Tracking),
            TransitionDecision::DuplicateNoOp
        );
        // A frozen forward edge the command has not taken → unsupported.
        assert_eq!(
            decide_transition(CommandState::Proposed, CommandState::Validated),
            TransitionDecision::UnsupportedLifecycleFact
        );
        // FAILED exits only via FALLBACK: that edge is unsupported, any
        // other FAILED exit is a duplicate.
        assert_eq!(
            decide_transition(CommandState::Failed, CommandState::Fallback),
            TransitionDecision::UnsupportedLifecycleFact
        );
        assert_eq!(
            decide_transition(CommandState::Failed, CommandState::Completed),
            TransitionDecision::DuplicateNoOp
        );
    }

    /// The metric families render in Prometheus text format, zero values
    /// included (the always-render discipline).
    #[test]
    fn metrics_render_all_five_families_at_zero() {
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
}
