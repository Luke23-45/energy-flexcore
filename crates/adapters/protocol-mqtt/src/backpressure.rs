// P14.20 — The MQTT/Sparkplug uplink adapter's backpressure binding
// (decision record
// `docs/phases-11-20/phase-14-protocol-device-connectivity/records/
// 83_p14_20_backpressure_rate_limiting.md`).
//
// The uplink is the edge's OUTBOUND surface: without a declared budget a
// fleet of edge nodes is exactly how a broker (or the uplink's own
// bandwidth) melts. This module binds the shared budget vocabulary
// (`flexcore_protocol_ports::observability`) onto the uplink's declared
// cadence:
//
// - the publish-rate budget is DERIVED from the adapter manifest cadence
//   (the reviewed `uplink_manifest` default seed is the BOUND; a
//   deployment declares the real value) — one budget per EDGE NODE, so
//   fleet scale is a registry of per-node budgets, never a shared pool
//   that one noisy node can exhaust;
// - the outbound queue policy is a declared [`BoundedQueuePolicy`] —
//   under `DropOldestAudited` the oldest un-published payload makes room
//   (the P6.09 store-and-forward continuity rule: never drop the NEWEST
//   telemetry), under `FailClosed` the publish is refused and the caller
//   re-buffers IN ORDER (the `UplinkChannel` at-least-once discipline).
//
// No broker connection, no socket, no runtime: pure declared data plus
// the derivation arithmetic. The wire seam (`crate::transport`) stays
// UNWIRED and nothing here publishes anything.

use std::collections::BTreeMap;

use flexcore_protocol_ports::observability::{
    Admission, BoundedQueuePolicy, BudgetError, MessageRateBudget, QueueOverflowPolicy,
};

/// Errors the MQTT backpressure binding reports. Every failure is named.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UplinkBackpressureError {
    /// The shared budget vocabulary refused (the closed reason maps 1:1
    /// onto the `flexcore_protocol_adapter_messages_refused_total` label
    /// values).
    Budget(BudgetError),
    /// The edge node has no declared budget in the fleet registry — a
    /// node that is not budgeted does not publish (fail-closed, never a
    /// silent unlimited default).
    NodeNotBudgeted { edge_node_id: String },
}

impl std::fmt::Display for UplinkBackpressureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UplinkBackpressureError::Budget(error) => write!(f, "{error}"),
            UplinkBackpressureError::NodeNotBudgeted { edge_node_id } => write!(
                f,
                "edge node '{edge_node_id}' has no declared uplink budget; an unbudgeted node does not publish"
            ),
        }
    }
}

impl std::error::Error for UplinkBackpressureError {}

impl From<BudgetError> for UplinkBackpressureError {
    fn from(error: BudgetError) -> Self {
        UplinkBackpressureError::Budget(error)
    }
}

/// The declared uplink backpressure posture of ONE edge node: the
/// publish-rate budget (derived from the declared cadence) plus the
/// outbound queue policy. Constructed only through
/// [`EdgeNodeUplinkBudget::new`]; no `Default`, no partially-filled
/// posture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EdgeNodeUplinkBudget {
    /// The edge node id the budget belongs to (the Sparkplug identity).
    pub edge_node_id: String,
    /// The publish-rate budget: at most `max_publishes` publishes per
    /// `interval_millis`. Declared per deployment (the manifest cadence
    /// is the seed, never a silent default).
    pub publish_rate: MessageRateBudget,
    /// The declared outbound-queue policy (payloads awaiting the wire).
    pub outbound_queue: BoundedQueuePolicy,
}

impl EdgeNodeUplinkBudget {
    /// Declares one edge node's budget: `max_publishes` publishes per
    /// `interval_millis`, with the given queue policy.
    pub fn new(
        edge_node_id: String,
        max_publishes: u64,
        interval_millis: i64,
        outbound_queue: BoundedQueuePolicy,
    ) -> Result<Self, UplinkBackpressureError> {
        if edge_node_id.trim().is_empty() {
            return Err(UplinkBackpressureError::Budget(BudgetError::Undeclared(
                "edge_node_id",
            )));
        }
        Ok(Self {
            edge_node_id,
            publish_rate: MessageRateBudget::new(max_publishes, interval_millis)?,
            outbound_queue,
        })
    }

    /// The shared publish-rate admission check for a planned batch of
    /// `count` publishes.
    pub fn allow_publishes(&self, count: u64) -> Result<(), UplinkBackpressureError> {
        self.publish_rate.allows(count)?;
        Ok(())
    }

    /// The outbound-queue admission for one more payload at the given
    /// depth (including the audited drop-oldest outcome).
    pub fn admit_payload(&self, current_depth: usize) -> Result<Admission, UplinkBackpressureError> {
        Ok(self.outbound_queue.admit(current_depth)?)
    }
}

/// The fleet-scale uplink budget registry: one declared budget per edge
/// node, `BTreeMap`-keyed by node id (the determinism house rule). A node
/// without a declared budget is refused at admission — fleet scale is
/// never an implicit unlimited default.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FleetUplinkBudgetRegistry {
    budgets: BTreeMap<String, EdgeNodeUplinkBudget>,
}

impl FleetUplinkBudgetRegistry {
    /// Constructs an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Declares (or replaces) one edge node's budget. Replacing is a
    /// reviewed edit of the budget — the caller owns the review.
    pub fn declare(&mut self, budget: EdgeNodeUplinkBudget) -> Result<(), UplinkBackpressureError> {
        // Zero-cap defence in depth: re-validate the rate through the
        // shared constructor even though `new` already validated (the
        // struct fields are public).
        MessageRateBudget::new(budget.publish_rate.max_messages, budget.publish_rate.interval_millis)?;
        if budget.outbound_queue.capacity == 0 {
            return Err(UplinkBackpressureError::Budget(BudgetError::Undeclared(
                "outbound_queue.capacity",
            )));
        }
        self.budgets.insert(budget.edge_node_id.clone(), budget);
        Ok(())
    }

    /// The declared budget for an edge node, if any.
    pub fn budget_for(&self, edge_node_id: &str) -> Option<&EdgeNodeUplinkBudget> {
        self.budgets.get(edge_node_id)
    }

    /// The publish-rate admission for one edge node: refused with
    /// [`UplinkBackpressureError::NodeNotBudgeted`] when the node has no
    /// declared budget, and with the shared rate refusal when the batch
    /// exceeds the declared budget.
    pub fn allow_publishes(&self, edge_node_id: &str, count: u64) -> Result<(), UplinkBackpressureError> {
        let budget = self
            .budgets
            .get(edge_node_id)
            .ok_or_else(|| UplinkBackpressureError::NodeNotBudgeted {
                edge_node_id: edge_node_id.to_string(),
            })?;
        budget.allow_publishes(count)
    }

    /// The declared edge node ids, sorted (deterministic diagnostics).
    pub fn edge_node_ids(&self) -> Vec<&str> {
        self.budgets.keys().map(String::as_str).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node_budget(id: &str) -> EdgeNodeUplinkBudget {
        EdgeNodeUplinkBudget::new(
            id.to_string(),
            6,
            1_000,
            BoundedQueuePolicy::new(16, QueueOverflowPolicy::DropOldestAudited).expect("valid"),
        )
        .expect("valid node budget")
    }

    #[test]
    fn undeclared_node_budgets_are_construction_errors() {
        assert_eq!(
            EdgeNodeUplinkBudget::new(
                String::new(),
                6,
                1_000,
                BoundedQueuePolicy::new(1, QueueOverflowPolicy::FailClosed).expect("valid"),
            ),
            Err(UplinkBackpressureError::Budget(BudgetError::Undeclared(
                "edge_node_id"
            )))
        );
        assert_eq!(
            EdgeNodeUplinkBudget::new(
                "edge-07".to_string(),
                0,
                1_000,
                BoundedQueuePolicy::new(1, QueueOverflowPolicy::FailClosed).expect("valid"),
            ),
            Err(UplinkBackpressureError::Budget(BudgetError::Undeclared(
                "max_messages"
            )))
        );
        assert_eq!(
            EdgeNodeUplinkBudget::new(
                "edge-07".to_string(),
                6,
                0,
                BoundedQueuePolicy::new(1, QueueOverflowPolicy::FailClosed).expect("valid"),
            ),
            Err(UplinkBackpressureError::Budget(BudgetError::Undeclared(
                "interval_millis"
            )))
        );
    }

    #[test]
    fn rate_admission_refuses_overages_and_queue_admission_reports_the_drop() {
        let budget = node_budget("edge-07");
        budget.allow_publishes(6).expect("exactly the budget fits");
        assert_eq!(
            budget.allow_publishes(7),
            Err(UplinkBackpressureError::Budget(BudgetError::Refused(
                RateRefusal::RateExceeded
            )))
        );
        // Below capacity: admitted.
        assert_eq!(budget.admit_payload(15), Ok(Admission::Admitted));
        // At capacity under drop-oldest: admitted WITH the audited drop.
        assert_eq!(budget.admit_payload(16), Ok(Admission::AdmittedDropOldest));
    }

    #[test]
    fn fleet_registry_refuses_unbudgeted_nodes() {
        let mut registry = FleetUplinkBudgetRegistry::new();
        registry.declare(node_budget("edge-07")).expect("declared");
        assert_eq!(registry.edge_node_ids(), vec!["edge-07"]);
        registry.allow_publishes("edge-07", 1).expect("budgeted node publishes");
        assert_eq!(
            registry.allow_publishes("edge-unknown", 1),
            Err(UplinkBackpressureError::NodeNotBudgeted {
                edge_node_id: "edge-unknown".to_string()
            })
        );
    }

    #[test]
    fn declaring_a_degenerate_budget_is_refused_even_when_hand_built() {
        let mut registry = FleetUplinkBudgetRegistry::new();
        // A hand-built budget with a zero rate (the fields are public).
        let mut zero_rate = node_budget("edge-07");
        zero_rate.publish_rate = MessageRateBudget { max_messages: 0, interval_millis: 1_000 };
        assert_eq!(
            registry.declare(zero_rate),
            Err(UplinkBackpressureError::Budget(BudgetError::Undeclared(
                "max_messages"
            )))
        );
        // A hand-built budget with a zero queue capacity.
        let mut zero_queue = node_budget("edge-07");
        zero_queue.outbound_queue =
            BoundedQueuePolicy { capacity: 0, overflow: QueueOverflowPolicy::FailClosed };
        assert_eq!(
            registry.declare(zero_queue),
            Err(UplinkBackpressureError::Budget(BudgetError::Undeclared(
                "outbound_queue.capacity"
            )))
        );
    }
}
