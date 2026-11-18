// P14.20 — The DNP3 outstation adapter's backpressure binding (decision
// record `docs/phases-11-20/phase-14-protocol-device-connectivity/
// records/83_p14_20_backpressure_rate_limiting.md`).
//
// The outstation's event queue ([`crate::outstation::OutstationModel`])
// is a QUEUE DESIGN, not a wire buffer — and it currently has no declared
// cap. Fleet-scale reporting without a cap is exactly how an outstation's
// event buffer grows without bound between integrity polls. This module
// binds the shared budget vocabulary
// (`flexcore_protocol_ports::observability`) onto the outstation surface:
//
// - the report rate budget is DERIVED from the adapter manifest cadence
//   (the declared poll-response cadence seed is the bound; the utility's
//   declared poll schedule is the reviewable per-deployment value), and
// - the event-queue policy is a declared [`BoundedQueuePolicy`] applied
//   AT THE ADMISSION CHECK (`admit_event`), BEFORE
//   [`crate::outstation::OutstationModel::queue_event`] is called — this
//   module never reaches into the outstation's own queue state. Under
//   `FailClosed` the event admission is refused (the caller decides the
//   honest degraded behaviour: the outstation reports the overflow
//   rather than silently losing events — overflow behaviour itself is
//   UNVERIFIED against the IEEE 1815.2-2025 text and stays an
//   implementation-time check); under `DropOldestAudited` the caller
//   drains the oldest queued event first (the drained event is REPORTED
//   through the transport seam, never discarded unreported — a DNP3
//   event queue overflow means reporting pressure, not data loss by
//   default).
//
// No socket, no DNP3 stack, no runtime: pure declared data plus the
// derivation arithmetic. The wire seam ([`crate::transport`]) stays
// UNWIRED and nothing here serves a report to anyone.

use flexcore_protocol_ports::observability::{
    Admission, BoundedQueuePolicy, BudgetError, MessageRateBudget, RateRefusal,
};

/// Errors the DNP3 backpressure binding reports. Every failure is named.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutstationBackpressureError {
    /// The shared budget vocabulary refused (the closed reason maps 1:1
    /// onto the `flexcore_protocol_adapter_messages_refused_total` label
    /// values).
    Budget(BudgetError),
}

impl std::fmt::Display for OutstationBackpressureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OutstationBackpressureError::Budget(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for OutstationBackpressureError {}

impl From<BudgetError> for OutstationBackpressureError {
    fn from(error: BudgetError) -> Self {
        OutstationBackpressureError::Budget(error)
    }
}

/// The declared backpressure posture of ONE outstation deployment: the
/// report-rate budget plus the event-queue policy. Constructed only
/// through [`OutstationBackpressure::new`]; no `Default`, no
/// partially-filled posture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutstationBackpressure {
    /// The report-rate budget: at most `max_reports` poll responses per
    /// `interval_millis` (derived from the declared cadence at
    /// construction by the caller; the utility's poll schedule governs
    /// the real value).
    pub report_rate: MessageRateBudget,
    /// The declared event-queue policy applied before each
    /// [`crate::outstation::OutstationModel::queue_event`] admission.
    pub event_queue: BoundedQueuePolicy,
}

impl OutstationBackpressure {
    /// Declares the posture: `max_reports` poll responses per
    /// `interval_millis`, with the given event-queue policy.
    pub fn new(
        max_reports: u64,
        interval_millis: i64,
        event_queue: BoundedQueuePolicy,
    ) -> Result<Self, OutstationBackpressureError> {
        Ok(Self {
            report_rate: MessageRateBudget::new(max_reports, interval_millis)?,
            event_queue,
        })
    }

    /// The shared report-rate admission check for a planned reporting
    /// burst of `count` responses.
    pub fn allow_reports(&self, count: u64) -> Result<(), OutstationBackpressureError> {
        self.report_rate.allows(count)?;
        Ok(())
    }

    /// The event-queue admission for one more queued event at the given
    /// depth — checked BEFORE the outstation model queues the event.
    /// Under `FailClosed` a full queue refuses (named, the caller
    /// reports the overflow honestly); under `DropOldestAudited` the
    /// caller drains and REPORTS the oldest event first (the drained
    /// event goes through the transport seam — an overflow is reporting
    /// pressure, never silent data loss).
    pub fn admit_event(&self, current_depth: usize) -> Result<Admission, OutstationBackpressureError> {
        Ok(self.event_queue.admit(current_depth)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flexcore_protocol_ports::observability::QueueOverflowPolicy;

    fn posture() -> OutstationBackpressure {
        OutstationBackpressure::new(
            10,
            1_000,
            BoundedQueuePolicy::new(64, QueueOverflowPolicy::FailClosed).expect("valid"),
        )
        .expect("valid posture")
    }

    #[test]
    fn undeclared_postures_are_construction_errors() {
        assert_eq!(
            OutstationBackpressure::new(
                0,
                1_000,
                BoundedQueuePolicy::new(1, QueueOverflowPolicy::FailClosed).expect("valid"),
            ),
            Err(OutstationBackpressureError::Budget(BudgetError::Undeclared(
                "max_messages"
            )))
        );
        assert_eq!(
            OutstationBackpressure::new(
                10,
                0,
                BoundedQueuePolicy::new(1, QueueOverflowPolicy::FailClosed).expect("valid"),
            ),
            Err(OutstationBackpressureError::Budget(BudgetError::Undeclared(
                "interval_millis"
            )))
        );
    }

    #[test]
    fn report_rate_refuses_overages() {
        let bp = posture();
        bp.allow_reports(10).expect("exactly the budget fits");
        assert_eq!(
            bp.allow_reports(11),
            Err(OutstationBackpressureError::Budget(BudgetError::Refused(
                RateRefusal::RateExceeded
            )))
        );
        assert_eq!(bp.report_rate.messages_per_second_floor(), 10);
    }

    #[test]
    fn event_queue_admission_is_fail_closed_at_capacity_by_default() {
        let bp = posture();
        assert_eq!(bp.admit_event(63), Ok(Admission::Admitted));
        assert_eq!(
            bp.admit_event(64),
            Err(OutstationBackpressureError::Budget(BudgetError::Refused(
                RateRefusal::QueueAtCapacity
            )))
        );
    }

    #[test]
    fn drop_oldest_policy_reports_the_audited_drop() {
        let bp = OutstationBackpressure::new(
            10,
            1_000,
            BoundedQueuePolicy::new(4, QueueOverflowPolicy::DropOldestAudited).expect("valid"),
        )
        .expect("valid posture");
        assert_eq!(bp.admit_event(3), Ok(Admission::Admitted));
        // Full: the caller drains and REPORTS the oldest event through
        // the seam, then admits — the drop is audited, never silent.
        assert_eq!(bp.admit_event(4), Ok(Admission::AdmittedDropOldest));
    }
}
