// P14.20 — The OpenADR VTN adapter's backpressure binding (decision
// record `docs/phases-11-20/phase-14-protocol-device-connectivity/
// records/83_p14_20_backpressure_rate_limiting.md`).
//
// The VTN's event-delivery surface is bounded by the VENs' AGREED poll
// frequencies (`crate::model::Ven::poll_frequency_millis`) and the push
// endpoints they registered. Without a fleet-scale cap, a large VEN
// population polling at aggressive frequencies is exactly how the VTN
// surface (and the DR program's own backend) melts. This module binds the
// shared budget vocabulary (`flexcore_protocol_ports::observability`)
// onto the VTN surface:
//
// - the per-VEN poll-rate budget is DERIVED from the VEN's AGREED poll
//   frequency (the registry's reviewable value — the adapter never polls
//   faster than the VEN agreed, and a VEN whose agreed frequency falls
//   below the VTN's declared floor is refused at planning time, never
//   silently accelerated);
// - the fleet connection budget is the shared
//   [`FleetConnectionRegistry`] (one declared limit per adapter; a VTN
//   with no declared limit admits no connections — fail-closed).
//
// No HTTP serving, no socket, no runtime: pure declared data plus the
// derivation arithmetic. The endpoint surface ([`crate::endpoints`]) is a
// designed seam and stays unserved; nothing here delivers an event to
// anyone.

use flexcore_protocol_ports::observability::{
    BudgetError, FleetConnectionRegistry, MessageRateBudget,
};

/// Errors the OpenADR backpressure binding reports. Every failure is
/// named.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VtnBackpressureError {
    /// The shared budget vocabulary refused (the closed reason maps 1:1
    /// onto the `flexcore_protocol_adapter_messages_refused_total` label
    /// values).
    Budget(BudgetError),
    /// The VEN's agreed poll frequency is below the VTN's declared
    /// minimum poll interval — the VTN does not accelerate a VEN past
    /// its declared floor (and does not silently slow it either; the
    /// refusal is named and the enrolment record is the fix).
    PollFrequencyBelowFloor {
        ven_id: String,
        agreed_millis: i64,
        floor_millis: i64,
    },
}

impl std::fmt::Display for VtnBackpressureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VtnBackpressureError::Budget(error) => write!(f, "{error}"),
            VtnBackpressureError::PollFrequencyBelowFloor { ven_id, agreed_millis, floor_millis } => {
                write!(
                    f,
                    "VEN '{ven_id}' agreed poll frequency {agreed_millis} ms is below the VTN's declared minimum poll interval {floor_millis} ms; the VTN does not accelerate past its declared floor"
                )
            }
        }
    }
}

impl std::error::Error for VtnBackpressureError {}

/// The declared VTN backpressure posture: the minimum poll interval the
/// VTN honours across all VENs (a VEN agreeing a faster poll than this
/// floor is refused at planning time — the floor is reviewed data, never
/// a silent clamp), plus the fleet connection registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VtnBackpressure {
    /// The VTN's declared minimum poll interval in milliseconds
    /// (positive). A VEN's agreed frequency must be at or above this.
    pub min_poll_interval_millis: i64,
    /// The fleet-scale connection budget (the shared registry; a VTN
    /// without a declared limit for its adapter id admits nothing).
    pub connections: FleetConnectionRegistry,
}

impl VtnBackpressure {
    /// Declares the posture: the minimum poll interval and the fleet
    /// connection registry (declared by the caller; never defaulted).
    pub fn new(
        min_poll_interval_millis: i64,
        connections: FleetConnectionRegistry,
    ) -> Result<Self, VtnBackpressureError> {
        if min_poll_interval_millis <= 0 {
            return Err(VtnBackpressureError::Budget(BudgetError::Undeclared(
                "min_poll_interval_millis",
            )));
        }
        Ok(Self { min_poll_interval_millis, connections })
    }

    /// The per-VEN poll-rate budget derived from the VEN's AGREED poll
    /// frequency: one poll per agreed interval. Refuses a VEN whose
    /// agreed frequency is below the declared floor — the VTN never
    /// accelerates a VEN past its declared minimum.
    pub fn poll_budget_for(
        &self,
        ven_id: &str,
        agreed_poll_frequency_millis: i64,
    ) -> Result<MessageRateBudget, VtnBackpressureError> {
        if agreed_poll_frequency_millis <= 0 {
            return Err(VtnBackpressureError::Budget(BudgetError::Undeclared(
                "agreed_poll_frequency_millis",
            )));
        }
        if agreed_poll_frequency_millis < self.min_poll_interval_millis {
            return Err(VtnBackpressureError::PollFrequencyBelowFloor {
                ven_id: ven_id.to_string(),
                agreed_millis: agreed_poll_frequency_millis,
                floor_millis: self.min_poll_interval_millis,
            });
        }
        Ok(MessageRateBudget::new(1, agreed_poll_frequency_millis)?)
    }

    /// The fleet-scale connection admission for the VTN adapter id (the
    /// shared registry's fail-closed semantics: an adapter id without a
    /// declared limit admits nothing).
    pub fn admit_connection(
        &self,
        adapter_id: &str,
        current_connections: u32,
    ) -> Result<(), VtnBackpressureError> {
        self.connections.admit(adapter_id, current_connections)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flexcore_protocol_ports::observability::ConnectionLimit;

    fn posture() -> VtnBackpressure {
        let mut connections = FleetConnectionRegistry::new();
        connections
            .declare(ConnectionLimit::new("openadr-vtn".to_string(), 3).expect("valid"))
            .expect("declared");
        VtnBackpressure::new(10_000, connections).expect("valid posture")
    }

    #[test]
    fn undeclared_floors_are_construction_errors() {
        let connections = FleetConnectionRegistry::new();
        assert_eq!(
            VtnBackpressure::new(0, connections),
            Err(VtnBackpressureError::Budget(BudgetError::Undeclared(
                "min_poll_interval_millis"
            )))
        );
    }

    #[test]
    fn poll_budget_is_one_poll_per_agreed_interval() {
        let bp = posture();
        let budget = bp
            .poll_budget_for("ven-a", 60_000)
            .expect("a VEN at the agreed frequency gets its budget");
        assert_eq!(budget.max_messages, 1);
        assert_eq!(budget.interval_millis, 60_000);
    }

    #[test]
    fn a_ven_below_the_declared_floor_is_refused_not_accelerated() {
        let bp = posture();
        let error = bp
            .poll_budget_for("ven-fast", 1_000)
            .expect_err("1 s polling is below the 10 s floor");
        assert_eq!(
            error,
            VtnBackpressureError::PollFrequencyBelowFloor {
                ven_id: "ven-fast".to_string(),
                agreed_millis: 1_000,
                floor_millis: 10_000,
            }
        );
        // Exactly at the floor is accepted (the floor is inclusive).
        bp.poll_budget_for("ven-at-floor", 10_000).expect("at the floor");
    }

    #[test]
    fn fleet_connections_are_fail_closed_and_bounded() {
        let bp = posture();
        bp.admit_connection("openadr-vtn", 2).expect("within the declared limit");
        assert_eq!(
            bp.admit_connection("openadr-vtn", 3),
            Err(VtnBackpressureError::Budget(BudgetError::Refused(
                RateRefusal::ConnectionLimitReached
            )))
        );
        // A different adapter id (no declared limit) admits nothing.
        assert_eq!(
            bp.admit_connection("other-adapter", 0),
            Err(VtnBackpressureError::Budget(BudgetError::Refused(
                RateRefusal::ConnectionLimitReached
            )))
        );
    }
}
