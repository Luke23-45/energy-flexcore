// P14.20/P14.21 — The shared protocol-adapter observability and
// backpressure vocabulary (decision records
// `docs/phases-11-20/phase-14-protocol-device-connectivity/records/
// 83_p14_20_backpressure_rate_limiting.md` and
// `84_p14_21_protocol_observability.md`).
//
// WHY THIS LIVES HERE (the shared-module decision, record 83 §2): every
// protocol adapter already depends on this CORE crate (`flexcore-protocol-
// ports`), and the P11 taxonomy rule is "defined once, consumed never
// redefined" (gap analysis §9 dependency rule; P11.01 record
// `docs/phases-11-20/phase-11-operational-robustness/records/
// 01_p11_01_metric_taxonomy.md`). A shared module on the common port layer
// is the one placement where every adapter can consume the SAME family
// names, label rules, and budget types without any adapter depending on
// another adapter (composition→composition edges), and without a core
// crate ever depending on an adapter (the forbidden direction). The module
// is ADDITIVE: no existing module of this crate is changed, and the
// dependency line stays serde-only (pure model types, no metrics library,
// no runtime — the metric families are NAMES and label RULES here; the
// rendering is a composition-root concern, exactly as the P11.01
// families are rendered in `apps/control-plane/src/metrics.rs`).
//
// WHAT THIS MODULE IS:
//
// - The registered metric FAMILY names and label rules for the protocol-
//   adapter tier, shaped exactly like the P11.01 families (same
//   `flexcore_` prefix convention, same no-free-text-label cardinality
//   discipline, same closed-vocabulary label values). The alert-rule file
//   `deploy/compose/observability/prometheus/rules/
//   protocol_adapter_alerts.yml` binds these names VERBATIM (P14.21; the
//   P11 wave-1 rule files are the pattern).
// - The declared backpressure types the per-adapter budget modules bind
//   their protocol-specific budgets onto ([`MessageRateBudget`],
//   [`BoundedQueuePolicy`], [`ConnectionLimit`]/[`FleetConnectionRegistry`])
//   — declared, validated, fail-closed; no defaults, no partially-filled
//   budget (the scope-declaration discipline applied to budgets).
//
// WHAT THIS MODULE IS NOT:
//
// - **Not a metrics implementation.** No counter increments anywhere:
//   nothing has ever been observed, scraped, or alerted on in this
//   program (the 0–10 base is WRITTEN-UNRUN). A family that renders with
//   zero values on an idle system is the P2.21 registry discipline; the
//   actual rendering code is composition-side and lands with the first
//   real adapter composition.
// - **Not a redefinition of the P11 taxonomy.** The trace-id correlation
//   is the P11.07 structured-log schema's (`flexcore.log.v1`, P11.07
//   record); this module CONSUMES those field names and adds exactly one
//   tier-specific field (`adapter_id`) — it never redefines `trace_id`,
//   `tenant_id`, `command_id`, or `schema_version`.
// - **Not a rate-limiting enforcement point.** The budget types are
//   DECLARED data an adapter checks against its own protocol surface;
//   there is no shared limiter runtime, no token bucket thread, no
//   shared mutable state. Enforcement lives in each adapter's own
//   backpressure module (record 83 §2).
//
// Cardinality decisions (recorded once, mirroring the P2.21/P11.01
// discipline):
// - `adapter_id` is the ONLY per-adapter label, and its value set is the
//   composition registry's allowlisted adapter ids (a bounded, reviewed
//   set — never a free string from wire data).
// - Connection-state label values are EXACTLY [`ConnectionState::name()`]
//   (the five-state vocabulary of [`crate::connection`]) — consumed, never
//   redefined.
// - Refusal-reason label values are the closed [`RateRefusal`] /
//   [`QueueOverflowPolicy`] vocabularies of THIS module — never raw error
//   text.
// - Per-device, per-point, and per-peer breakdowns go to the structured
//   LOGS, never to labels (the P2.21 recorded decision, carried forward).

use serde::{Deserialize, Serialize};

// --- Registered metric family names (P14.21; bound verbatim by the
// --- `protocol_adapter_alerts.yml` rule file) ---------------------------------

/// Connection-state gauge per adapter: the count of adapters (or adapter
/// connections) currently in each [`ConnectionState`] (the five-state
/// vocabulary of [`crate::connection::ConnectionState::name`] — consumed,
/// never redefined). Labels: `adapter_id`, `state`.
pub const METRIC_PROTOCOL_ADAPTER_CONNECTIONS: &str = "flexcore_protocol_adapter_connections";

/// Total protocol messages exchanged (declared, counted by the adapter's
/// own surface when a real transport exists). Labels: `adapter_id`,
/// `direction` ([`MessageDirection::name`]).
pub const METRIC_PROTOCOL_ADAPTER_MESSAGES_TOTAL: &str = "flexcore_protocol_adapter_messages_total";

/// Total messages REFUSED by a declared budget (rate, connection, or
/// queue capacity) — the backpressure-refusal counter. Labels:
/// `adapter_id`, `reason` ([`RateRefusal::name`]).
pub const METRIC_PROTOCOL_ADAPTER_MESSAGES_REFUSED_TOTAL: &str =
    "flexcore_protocol_adapter_messages_refused_total";

/// Current depth of the adapter's bounded queue (the store-and-forward /
/// poll-plan queue). Labels: `adapter_id`.
pub const METRIC_PROTOCOL_ADAPTER_QUEUE_DEPTH: &str = "flexcore_protocol_adapter_queue_depth";

/// Total queue entries dropped under the queue's DECLARED overflow policy
/// (only the `drop_oldest_audited` policy ever increments this — a
/// fail-closed queue refuses instead, surfacing as a refusal, not a
/// drop). Labels: `adapter_id`, `policy` ([`QueueOverflowPolicy::name`]).
pub const METRIC_PROTOCOL_ADAPTER_QUEUE_DROPPED_TOTAL: &str =
    "flexcore_protocol_adapter_queue_dropped_total";

/// The canonical label key carrying the adapter id on every protocol-tier
/// metric series. The value set is the composition registry's allowlisted
/// adapter ids — a bounded reviewed set, never free wire text.
pub const LABEL_ADAPTER_ID: &str = "adapter_id";

// --- Structured-log field names (the P11.07 schema, consumed) -----------------

/// The P11.07 structured-log schema version (`flexcore.log.v1`) the
/// protocol-tier log lines carry. CONSUMED — defined once by the P11.07
/// record; never redefined here.
pub const LOG_SCHEMA_VERSION: &str = "flexcore.log.v1";

/// The P11.07 correlation field this tier CONSUMES. The protocol tier adds
/// no correlation vocabulary of its own: a protocol log line carries the
/// same `trace_id` the control-plane correlation line carries, so a
/// protocol hop is correlatable end-to-end. Never redefined.
pub const LOG_FIELD_TRACE_ID: &str = "trace_id";

/// The protocol-tier ADDITIVE log field: which adapter emitted the line.
/// The only new field this phase adds to the P11.07 schema shape.
pub const LOG_FIELD_ADAPTER_ID: &str = "adapter_id";

// --- Closed label vocabularies --------------------------------------------------

/// The closed message-direction label vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageDirection {
    /// Messages received by the platform from the protocol peer (device
    /// polls answered, controls received, events polled).
    Inbound,
    /// Messages sent by the platform to the protocol peer (reports
    /// served, publishes, notifications).
    Outbound,
}

impl MessageDirection {
    /// Canonical name (the label value).
    pub const fn name(self) -> &'static str {
        match self {
            MessageDirection::Inbound => "inbound",
            MessageDirection::Outbound => "outbound",
        }
    }
}

/// Why a message was refused by a declared budget. Closed vocabulary —
/// the refusal-reason label values; raw error text never becomes a label
/// (the P2.21 quarantine-reason rule, carried forward).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RateRefusal {
    /// The message rate exceeded the declared [`MessageRateBudget`].
    RateExceeded,
    /// The peer/connection was refused by the declared
    /// [`ConnectionLimit`] (fleet-scale connection budget).
    ConnectionLimitReached,
    /// The adapter's bounded queue was at capacity under the fail-closed
    /// overflow policy.
    QueueAtCapacity,
    /// The budget itself was degenerate (zero rate) — a configuration
    /// defect surfaced at the refusal, never silently tolerated.
    BudgetNotDeclared,
}

impl RateRefusal {
    /// Canonical name (the label value).
    pub const fn name(self) -> &'static str {
        match self {
            RateRefusal::RateExceeded => "rate_exceeded",
            RateRefusal::ConnectionLimitReached => "connection_limit_reached",
            RateRefusal::QueueAtCapacity => "queue_at_capacity",
            RateRefusal::BudgetNotDeclared => "budget_not_declared",
        }
    }
}

/// The declared overflow behaviour of a bounded adapter queue. Both
/// policies are honest named behaviours; which one a deployment uses is
/// reviewed data, never a silent default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueueOverflowPolicy {
    /// Refuse the append (the P6.08 in-memory buffer behaviour: a
    /// capacity hit is an audited anomaly, never a routine path).
    FailClosed,
    /// Drop the OLDEST entry with an audited counter (the P6.09 durable
    /// buffer behaviour; store-and-forward continuity wins over
    /// retention).
    DropOldestAudited,
}

impl QueueOverflowPolicy {
    /// Canonical name (the label value).
    pub const fn name(self) -> &'static str {
        match self {
            QueueOverflowPolicy::FailClosed => "fail_closed",
            QueueOverflowPolicy::DropOldestAudited => "drop_oldest_audited",
        }
    }
}

// --- Budget types ----------------------------------------------------------------

/// Errors returned by budget construction and admission checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetError {
    /// A required budget field was zero or undeclared. There are no
    /// defaults: an undeclared budget is not a reviewed budget.
    Undeclared(&'static str),
    /// The admission check refused the operation for the named reason.
    Refused(RateRefusal),
}

impl std::fmt::Display for BudgetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BudgetError::Undeclared(field) => {
                write!(f, "budget field '{field}' is undeclared; there are no defaults")
            }
            BudgetError::Refused(reason) => {
                write!(f, "the declared budget refused the operation: {}", reason.name())
            }
        }
    }
}

impl std::error::Error for BudgetError {}

/// The declared message-rate budget for one adapter surface: at most
/// `max_messages` messages per `interval_millis`. Declared data (the
/// per-adapter modules derive it from the adapter's own cadence — the
/// manifest cadence, the Modbus gateway polling budget, the VEN poll
/// frequency); there is no `Default` and no partially-filled budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageRateBudget {
    /// Maximum messages per interval (non-zero).
    pub max_messages: u64,
    /// The budget interval in milliseconds (positive).
    pub interval_millis: i64,
}

impl MessageRateBudget {
    /// Validates and constructs the budget.
    pub fn new(max_messages: u64, interval_millis: i64) -> Result<Self, BudgetError> {
        if max_messages == 0 {
            return Err(BudgetError::Undeclared("max_messages"));
        }
        if interval_millis <= 0 {
            return Err(BudgetError::Undeclared("interval_millis"));
        }
        Ok(Self { max_messages, interval_millis })
    }

    /// Whether `count` messages fit the budget in one interval.
    pub fn allows(&self, count: u64) -> Result<(), BudgetError> {
        if count > self.max_messages {
            return Err(BudgetError::Refused(RateRefusal::RateExceeded));
        }
        Ok(())
    }

    /// The budget expressed as a message rate per second (the planning
    /// arithmetic the per-adapter modules and the alert thresholds bind
    /// to; a whole-number floor of the declared budget).
    pub fn messages_per_second_floor(&self) -> u64 {
        (self.max_messages * 1000) as u64 / self.interval_millis as u64
    }
}

/// The declared bounded-queue policy for one adapter queue: a capacity
/// plus an explicit overflow behaviour. Validated at construction (a
/// zero-capacity queue is a construction error, mirroring the P6.08
/// buffer rule).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoundedQueuePolicy {
    /// The queue capacity (non-zero — a zero-capacity queue can never
    /// hold an entry and is a construction error, not a policy).
    pub capacity: usize,
    /// The declared overflow behaviour.
    pub overflow: QueueOverflowPolicy,
}

impl BoundedQueuePolicy {
    /// Validates and constructs the policy.
    pub fn new(capacity: usize, overflow: QueueOverflowPolicy) -> Result<Self, BudgetError> {
        if capacity == 0 {
            return Err(BudgetError::Undeclared("capacity"));
        }
        Ok(Self { capacity, overflow })
    }

    /// The admission decision for an append at the current depth: `Ok`
    /// when the entry fits; `Err(QueueAtCapacity)` when the queue is full
    /// under the fail-closed policy. Under `drop_oldest_audited` the
    /// adapter drops the oldest entry (audited, counted on
    /// [`METRIC_PROTOCOL_ADAPTER_QUEUE_DROPPED_TOTAL`]) and admits the
    /// new one — this type reports the drop so the adapter can count it;
    /// it never holds queue state itself.
    pub fn admit(&self, current_depth: usize) -> Result<Admission, BudgetError> {
        if current_depth < self.capacity {
            return Ok(Admission::Admitted);
        }
        match self.overflow {
            QueueOverflowPolicy::FailClosed => Err(BudgetError::Refused(RateRefusal::QueueAtCapacity)),
            QueueOverflowPolicy::DropOldestAudited => Ok(Admission::AdmittedDropOldest),
        }
    }
}

/// The outcome of a bounded-queue admission check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Admission {
    /// The entry was admitted with room to spare.
    Admitted,
    /// The entry was admitted after dropping the OLDEST entry (audited;
    /// the adapter increments the dropped-total counter).
    AdmittedDropOldest,
}

/// The declared connection limit for ONE adapter id: the fleet-scale
/// connection budget (how many protocol associations the adapter may hold
/// at once — Modbus gateways, MQTT broker sessions, 2030.5 device
/// sessions). Declared per deployment; no defaults.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectionLimit {
    /// The adapter id the limit applies to (an allowlisted id).
    pub adapter_id: String,
    /// The maximum simultaneous protocol associations (non-zero).
    pub max_connections: u32,
}

impl ConnectionLimit {
    /// Validates and constructs the limit.
    pub fn new(adapter_id: String, max_connections: u32) -> Result<Self, BudgetError> {
        if adapter_id.trim().is_empty() {
            return Err(BudgetError::Undeclared("adapter_id"));
        }
        if max_connections == 0 {
            return Err(BudgetError::Undeclared("max_connections"));
        }
        Ok(Self { adapter_id, max_connections })
    }
}

/// The fleet-scale connection-limit registry: the reviewed per-adapter
/// connection budgets, `BTreeMap`-keyed by adapter id (the determinism
/// house rule). A connection admission for an adapter WITHOUT a declared
/// limit is REFUSED — a fleet connection budget that does not name an
/// adapter does not cover it (fail-closed, never a silent unlimited
/// default).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FleetConnectionRegistry {
    limits: std::collections::BTreeMap<String, ConnectionLimit>,
}

impl FleetConnectionRegistry {
    /// Constructs an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Declares (or replaces) the connection limit for an adapter id.
    /// Replacing is a reviewed edit of the budget — the caller owns the
    /// review, as with the register-map registry.
    pub fn declare(&mut self, limit: ConnectionLimit) -> Result<(), BudgetError> {
        // Re-validate (the fields are public; the checked constructor is
        // not the only construction path — zero-cap defence in depth).
        ConnectionLimit::new(limit.adapter_id.clone(), limit.max_connections)?;
        self.limits.insert(limit.adapter_id.clone(), limit);
        Ok(())
    }

    /// The declared limit for an adapter id, if any.
    pub fn limit_for(&self, adapter_id: &str) -> Option<&ConnectionLimit> {
        self.limits.get(adapter_id)
    }

    /// The admission check for one more connection: refused when the
    /// adapter has no declared limit, or when `current_connections` is
    /// already at the declared maximum.
    pub fn admit(
        &self,
        adapter_id: &str,
        current_connections: u32,
    ) -> Result<(), BudgetError> {
        let limit = self
            .limits
            .get(adapter_id)
            .ok_or(BudgetError::Refused(RateRefusal::ConnectionLimitReached))?;
        // Zero-cap defence in depth: a hand-built limit with a zero
        // maximum is treated as a not-declared budget.
        if limit.max_connections == 0 {
            return Err(BudgetError::Refused(RateRefusal::BudgetNotDeclared));
        }
        if current_connections >= limit.max_connections {
            return Err(BudgetError::Refused(RateRefusal::ConnectionLimitReached));
        }
        Ok(())
    }

    /// The declared adapter ids, sorted (deterministic diagnostics).
    pub fn adapter_ids(&self) -> Vec<&str> {
        self.limits.keys().map(String::as_str).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metric_family_names_are_the_registered_spellings() {
        // The rule file binds these verbatim; the test pins the spellings
        // so a rename is a visible edit, never a silent drift.
        assert_eq!(
            METRIC_PROTOCOL_ADAPTER_CONNECTIONS,
            "flexcore_protocol_adapter_connections"
        );
        assert_eq!(
            METRIC_PROTOCOL_ADAPTER_MESSAGES_TOTAL,
            "flexcore_protocol_adapter_messages_total"
        );
        assert_eq!(
            METRIC_PROTOCOL_ADAPTER_MESSAGES_REFUSED_TOTAL,
            "flexcore_protocol_adapter_messages_refused_total"
        );
        assert_eq!(
            METRIC_PROTOCOL_ADAPTER_QUEUE_DEPTH,
            "flexcore_protocol_adapter_queue_depth"
        );
        assert_eq!(
            METRIC_PROTOCOL_ADAPTER_QUEUE_DROPPED_TOTAL,
            "flexcore_protocol_adapter_queue_dropped_total"
        );
        assert_eq!(LABEL_ADAPTER_ID, "adapter_id");
    }

    #[test]
    fn label_vocabularies_are_closed_and_name_the_consumed_states() {
        assert_eq!(MessageDirection::Inbound.name(), "inbound");
        assert_eq!(MessageDirection::Outbound.name(), "outbound");
        assert_eq!(RateRefusal::RateExceeded.name(), "rate_exceeded");
        assert_eq!(
            RateRefusal::ConnectionLimitReached.name(),
            "connection_limit_reached"
        );
        assert_eq!(RateRefusal::QueueAtCapacity.name(), "queue_at_capacity");
        assert_eq!(RateRefusal::BudgetNotDeclared.name(), "budget_not_declared");
        assert_eq!(QueueOverflowPolicy::FailClosed.name(), "fail_closed");
        assert_eq!(
            QueueOverflowPolicy::DropOldestAudited.name(),
            "drop_oldest_audited"
        );
        // The connection-state label values are the COMMON connection
        // vocabulary's names — consumed, never redefined (P14.21).
        assert_eq!(crate::connection::ConnectionState::Connected.name(), "CONNECTED");
        assert_eq!(crate::connection::ConnectionState::Unreachable { reason: String::new() }.name(), "UNREACHABLE");
        // The log schema is the P11.07 one, consumed.
        assert_eq!(LOG_SCHEMA_VERSION, "flexcore.log.v1");
        assert_eq!(LOG_FIELD_TRACE_ID, "trace_id");
        assert_eq!(LOG_FIELD_ADAPTER_ID, "adapter_id");
    }

    #[test]
    fn undeclared_budgets_are_construction_errors() {
        assert_eq!(
            MessageRateBudget::new(0, 1_000),
            Err(BudgetError::Undeclared("max_messages"))
        );
        assert_eq!(
            MessageRateBudget::new(10, 0),
            Err(BudgetError::Undeclared("interval_millis"))
        );
        assert_eq!(
            BoundedQueuePolicy::new(0, QueueOverflowPolicy::FailClosed),
            Err(BudgetError::Undeclared("capacity"))
        );
        assert_eq!(
            ConnectionLimit::new(String::new(), 4),
            Err(BudgetError::Undeclared("adapter_id"))
        );
        assert_eq!(
            ConnectionLimit::new("modbus-client".to_string(), 0),
            Err(BudgetError::Undeclared("max_connections"))
        );
    }

    #[test]
    fn rate_budget_refuses_overages_and_reports_its_floor() {
        let budget = MessageRateBudget::new(60, 1_000).expect("valid");
        budget.allows(60).expect("exactly the budget fits");
        assert_eq!(
            budget.allows(61),
            Err(BudgetError::Refused(RateRefusal::RateExceeded))
        );
        assert_eq!(budget.messages_per_second_floor(), 60);
        let sub_second = MessageRateBudget::new(1, 2_000).expect("valid");
        assert_eq!(sub_second.messages_per_second_floor(), 0, "floor, not round");
    }

    #[test]
    fn queue_policy_admits_fits_and_refuses_or_reports_the_drop() {
        let fail_closed = BoundedQueuePolicy::new(3, QueueOverflowPolicy::FailClosed)
            .expect("valid");
        assert_eq!(fail_closed.admit(2), Ok(Admission::Admitted));
        assert_eq!(
            fail_closed.admit(3),
            Err(BudgetError::Refused(RateRefusal::QueueAtCapacity))
        );
        let drop_oldest =
            BoundedQueuePolicy::new(3, QueueOverflowPolicy::DropOldestAudited).expect("valid");
        // Full under drop-oldest: admitted WITH the audited drop report.
        assert_eq!(drop_oldest.admit(3), Ok(Admission::AdmittedDropOldest));
    }

    #[test]
    fn fleet_registry_refuses_undeclared_and_at_limit_admissions() {
        let mut registry = FleetConnectionRegistry::new();
        registry
            .declare(ConnectionLimit::new("modbus-client".to_string(), 2).expect("valid"))
            .expect("declared");
        assert_eq!(registry.adapter_ids(), vec!["modbus-client"]);
        // Within the limit.
        registry.admit("modbus-client", 1).expect("admitted");
        // At the limit: refused.
        assert_eq!(
            registry.admit("modbus-client", 2),
            Err(BudgetError::Refused(RateRefusal::ConnectionLimitReached))
        );
        // An adapter without a declared limit: refused — no unlimited
        // default exists.
        assert_eq!(
            registry.admit("mqtt-sparkplug-uplink", 0),
            Err(BudgetError::Refused(RateRefusal::ConnectionLimitReached))
        );
        // A zero-maximum limit (hand-built) is a not-declared budget.
        registry
            .declare(ConnectionLimit { adapter_id: "openadr-vtn".to_string(), max_connections: 0 })
            .expect_err("zero maximum is refused at declare");
    }
}
