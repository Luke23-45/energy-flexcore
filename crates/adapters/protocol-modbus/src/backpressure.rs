// P14.20 — The Modbus adapter's backpressure binding (decision record
// `docs/phases-11-20/phase-14-protocol-device-connectivity/records/
// 83_p14_20_backpressure_rate_limiting.md`).
//
// This module is the ADAPTER-SIDE binding of the shared budget vocabulary
// (`flexcore_protocol_ports::observability`) onto THIS adapter's own
// declared budget surface ([`crate::polling::GatewayPollingBudget`]).
// It defines no new budget semantics of its own: a Modbus read is refused
// when the GATEWAY polling budget refuses it (polling.rs is the owning
// record of the read/scan rules); this module only:
//
// - derives the adapter's message-rate budget FROM the declared gateway
//   budget (reads per scan × scan interval → the shared
//   `MessageRateBudget` shape, so the P14.20 counter families and the
//   alert thresholds have ONE rate vocabulary across adapters), and
// - declares the poll-plan queue policy as a shared [`BoundedQueuePolicy`]
//   (declared, never defaulted — the same no-Default rule the gateway
//   budget already enforces).
//
// No queue, no limiter thread, no socket: pure declared data plus the
// derivation arithmetic. No device is reached and no message is exchanged
// by this module (the transport seam [`crate::transport`] is UNWIRED and
// stays so).

use flexcore_protocol_ports::observability::{
    BoundedQueuePolicy, BudgetError, MessageRateBudget, RateRefusal,
};

use crate::polling::GatewayPollingBudget;

/// Errors the Modbus backpressure binding reports. Every failure is
/// named; nothing is silently squeezed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModbusBackpressureError {
    /// The shared budget vocabulary refused (re-raised with its closed
    /// reason so the refusal maps 1:1 onto the
    /// `flexcore_protocol_adapter_messages_refused_total` label values).
    Budget(BudgetError),
    /// The gateway polling budget refused a planned read/scan, carried
    /// under the adapter's closed refusal vocabulary
    /// ([`map_budget_error`]). The shared `BudgetError` has no faithful
    /// spelling for "read too large" / "device budget exceeded" /
    /// "interval too fast", so the refusal keeps its own name rather than
    /// being mislabelled as a rate refusal.
    Refusal(ModbusReadRefusal),
}

impl std::fmt::Display for ModbusBackpressureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModbusBackpressureError::Budget(error) => write!(f, "{error}"),
            ModbusBackpressureError::Refusal(refusal) => {
                write!(f, "the gateway polling budget refused the plan: {}", refusal.name())
            }
        }
    }
}

impl std::error::Error for ModbusBackpressureError {}

impl From<BudgetError> for ModbusBackpressureError {
    fn from(error: BudgetError) -> Self {
        ModbusBackpressureError::Budget(error)
    }
}

impl From<crate::polling::BudgetError> for ModbusBackpressureError {
    fn from(error: crate::polling::BudgetError) -> Self {
        // The file's own total mapping (`map_budget_error`); only
        // ReadTooLarge / DeviceBudgetExceeded / IntervalTooFast can arise
        // from the `assert_*` calls (polling.rs), so the Undeclared arm of
        // that mapping never fires on this path.
        ModbusBackpressureError::Refusal(map_budget_error(&error))
    }
}

/// The closed refusal reason of a refused Modbus read (the label value
/// for `flexcore_protocol_adapter_messages_refused_total{...}`): the
/// gateway budget's own named refusals mapped onto the shared closed
/// vocabulary — consumed, never redefined.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModbusReadRefusal {
    /// The read exceeded the declared per-read register budget.
    ReadTooLarge,
    /// The scan plan exceeded the gateway's device budget.
    DeviceBudgetExceeded,
    /// The planned interval is below the declared minimum interval.
    IntervalTooFast,
    /// The read exceeded the Modbus application read limit at budget
    /// construction (a declared-budget defect, surfaced at construction,
    /// not at poll time — carried here for vocabulary completeness).
    ProtocolLimitExceeded,
}

impl ModbusReadRefusal {
    /// The shared [`RateRefusal`] this adapter-specific refusal maps onto.
    /// Every Modbus read refusal is a rate/budget refusal in the shared
    /// vocabulary — the mapping is total and constant.
    pub const fn shared_refusal(self) -> RateRefusal {
        RateRefusal::RateExceeded
    }

    /// Canonical name (records/logs; NOT a metric label — metric labels
    /// use the shared vocabulary only, per the recorded cardinality
    /// decision).
    pub const fn name(self) -> &'static str {
        match self {
            ModbusReadRefusal::ReadTooLarge => "read_too_large",
            ModbusReadRefusal::DeviceBudgetExceeded => "device_budget_exceeded",
            ModbusReadRefusal::IntervalTooFast => "interval_too_fast",
            ModbusReadRefusal::ProtocolLimitExceeded => "protocol_limit_exceeded",
        }
    }
}

/// Maps a [`crate::polling::BudgetError`] onto the closed adapter refusal
/// vocabulary (total, constant mapping — no budget error is left
/// unmapped).
pub const fn map_budget_error(error: &crate::polling::BudgetError) -> ModbusReadRefusal {
    match error {
        crate::polling::BudgetError::Undeclared(_)
        | crate::polling::BudgetError::ExceedsProtocolReadLimit { .. } => {
            ModbusReadRefusal::ProtocolLimitExceeded
        }
        crate::polling::BudgetError::ReadTooLarge { .. } => ModbusReadRefusal::ReadTooLarge,
        crate::polling::BudgetError::DeviceBudgetExceeded { .. } => {
            ModbusReadRefusal::DeviceBudgetExceeded
        }
        crate::polling::BudgetError::IntervalTooFast { .. } => ModbusReadRefusal::IntervalTooFast,
    }
}

/// The declared backpressure posture of ONE Modbus gateway path: the
/// gateway's own polling budget (polling.rs — the owning module) plus the
/// shared shapes derived from it. Constructed only through
/// [`GatewayBackpressure::new`]; no `Default`, no partially-filled
/// posture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatewayBackpressure {
    /// The declared gateway polling budget (validated by its own
    /// constructor; held unchanged — this module never loosens it).
    pub budget: GatewayPollingBudget,
    /// The message-rate budget DERIVED from the gateway budget: the
    /// whole-scan read volume against the declared minimum interval.
    /// Derived, not independently declared — the two cannot drift.
    pub message_rate: MessageRateBudget,
    /// The declared poll-plan queue policy (a plan waiting for its scan
    /// slot). Declared by the caller; never defaulted.
    pub plan_queue: BoundedQueuePolicy,
}

impl GatewayBackpressure {
    /// Derives the backpressure posture from a declared gateway budget.
    ///
    /// Derivation arithmetic (recorded, deterministic):
    /// - the worst-case read volume of one whole scan is
    ///   `devices_per_gateway` reads of up to `max_registers_per_read`
    ///   registers each — the message-rate budget counts READ REQUESTS,
    ///   one per device per scan (the poller plans one read per device
    ///   block; larger blocks split into budget-sized reads, which the
    ///   rate floor below already bounds because a split scan takes more
    ///   intervals, not more messages per interval);
    /// - the rate interval is the gateway's declared minimum poll
    ///   interval (a scan may not run faster than that — the budget the
    ///   gateway already enforces).
    pub fn new(
        budget: GatewayPollingBudget,
        plan_queue: BoundedQueuePolicy,
    ) -> Result<Self, ModbusBackpressureError> {
        let message_rate = MessageRateBudget::new(
            u64::from(budget.devices_per_gateway),
            budget.min_poll_interval_millis,
        )?;
        Ok(Self { budget, message_rate, plan_queue })
    }

    /// Asserts one planned read against the gateway budget (a thin,
    /// named pass-through — polling.rs stays the owning checker; this
    /// method exists so a caller checks ONE surface).
    pub fn assert_read_fits(&self, registers: u16) -> Result<(), ModbusBackpressureError> {
        self.budget.assert_read_fits(registers)?;
        Ok(())
    }

    /// Asserts one planned scan (device count + interval) against the
    /// gateway budget.
    pub fn assert_scan_fits(
        &self,
        devices: usize,
        interval_millis: i64,
    ) -> Result<(), ModbusBackpressureError> {
        self.budget.assert_scan_fits(devices, interval_millis)?;
        Ok(())
    }

    /// The shared message-rate admission check for a planned scan burst.
    pub fn allow_scan_messages(&self, messages: u64) -> Result<(), ModbusBackpressureError> {
        self.message_rate.allows(messages)?;
        Ok(())
    }

    /// The bounded-queue admission for one more queued poll plan at the
    /// given depth (the shared admission semantics, including the audited
    /// drop-oldest outcome — the caller increments the dropped-total
    /// counter when the result is [`Admission::AdmittedDropOldest`]).
    pub fn admit_plan(
        &self,
        current_depth: usize,
    ) -> Result<flexcore_protocol_ports::observability::Admission, ModbusBackpressureError> {
        Ok(self.plan_queue.admit(current_depth)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flexcore_protocol_ports::observability::QueueOverflowPolicy;

    fn budget() -> GatewayPollingBudget {
        GatewayPollingBudget::new(100, 4, 2_000).expect("valid gateway budget")
    }

    fn queue() -> BoundedQueuePolicy {
        BoundedQueuePolicy::new(8, QueueOverflowPolicy::FailClosed).expect("valid queue policy")
    }

    #[test]
    fn derived_rate_budget_matches_the_gateway_budget_exactly() {
        let bp = GatewayBackpressure::new(budget(), queue()).expect("valid posture");
        // One read request per device per scan, at the declared minimum
        // interval.
        assert_eq!(bp.message_rate.max_messages, 4);
        assert_eq!(bp.message_rate.interval_millis, 2_000);
        assert_eq!(bp.message_rate.messages_per_second_floor(), 2);
        // The owning budget is held unchanged.
        assert_eq!(bp.budget, budget());
    }

    #[test]
    fn refused_reads_and_scans_surface_the_owning_budgets_errors() {
        let bp = GatewayBackpressure::new(budget(), queue()).expect("valid posture");
        // The polling budget's refusals surface under the ADAPTER's closed
        // vocabulary (`map_budget_error`), NOT as the shared `Budget`
        // variant: the shared `BudgetError` has no faithful spelling for
        // "read too large" / "device budget exceeded" / "interval too
        // fast", so the refusal keeps its own name rather than being
        // mislabelled as a rate refusal (the `Refusal` variant's doc;
        // `budget_error_mapping_is_total_and_constant` pins the mapping).
        // The `Budget` variant is for the SHARED vocabulary refusals
        // (message-rate / queue admission) — see `admit_plan` below.
        assert_eq!(
            bp.assert_read_fits(101),
            Err(ModbusBackpressureError::Refusal(ModbusReadRefusal::ReadTooLarge))
        );
        assert!(bp.assert_read_fits(100).is_ok());
        assert_eq!(
            bp.assert_scan_fits(5, 2_000),
            Err(ModbusBackpressureError::Refusal(
                ModbusReadRefusal::DeviceBudgetExceeded
            ))
        );
        assert_eq!(
            bp.assert_scan_fits(4, 1_999),
            Err(ModbusBackpressureError::Refusal(ModbusReadRefusal::IntervalTooFast))
        );
    }

    #[test]
    fn budget_error_mapping_is_total_and_constant() {
        // Every polling.rs refusal maps onto the closed adapter
        // vocabulary, and every adapter refusal is a shared RateExceeded
        // (the label value stays in the shared vocabulary).
        let cases = [
            crate::polling::BudgetError::Undeclared("max_registers_per_read"),
            crate::polling::BudgetError::ExceedsProtocolReadLimit { declared: 200, limit: 125 },
            crate::polling::BudgetError::ReadTooLarge { requested: 300, budget: 100 },
            crate::polling::BudgetError::DeviceBudgetExceeded { requested: 9, budget: 4 },
            crate::polling::BudgetError::IntervalTooFast { requested_millis: 1, minimum_millis: 2 },
        ];
        for error in cases {
            let mapped = map_budget_error(&error);
            assert_eq!(mapped.shared_refusal(), RateRefusal::RateExceeded);
            assert!(!mapped.name().is_empty());
        }
    }

    #[test]
    fn queue_admission_is_fail_closed_at_capacity() {
        let bp = GatewayBackpressure::new(budget(), queue()).expect("valid posture");
        assert!(bp.admit_plan(7).is_ok());
        assert_eq!(
            bp.admit_plan(8),
            Err(ModbusBackpressureError::Budget(BudgetError::Refused(
                RateRefusal::QueueAtCapacity
            )))
        );
    }

    #[test]
    fn a_degenerate_derived_rate_is_impossible_for_a_valid_gateway_budget() {
        // A valid GatewayPollingBudget always yields a valid derived rate:
        // devices >= 1 and min_poll_interval_millis >= 1 are its own
        // construction rules, so the derivation cannot fail for a valid
        // budget (the failure arms exist for the shared constructor's
        // contract, not because a valid gateway budget can hit them).
        let tight = GatewayPollingBudget::new(1, 1, 1).expect("valid");
        let bp = GatewayBackpressure::new(
            tight,
            BoundedQueuePolicy::new(1, QueueOverflowPolicy::DropOldestAudited).expect("valid"),
        )
        .expect("valid posture");
        assert_eq!(bp.message_rate.max_messages, 1);
    }
}
