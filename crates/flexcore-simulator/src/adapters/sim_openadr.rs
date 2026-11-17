// P14.17 — The simulated OpenADR VEN (decision record 81; the seam is
// `flexcore_openadr_adapter::endpoints::VtnEndpointDesign` +
// `vtn::{VtnRegistry, DeliveryPlanner}` — the designed endpoint/data
// surface, record 21 §3).
//
// The VEN is the CLIENT side of OpenADR: it registers with the VTN,
// polls for events at the AGREED poll frequency (polling delivery mode)
// or receives pushes at its registered endpoint (push delivery mode),
// and hands received events to the adapter's PROPOSED mapping. The
// simulated VEN exercises the DATA surface exactly: it resolves its
// delivery plan through the [`DeliveryPlanner`], honours its agreed poll
// frequency as its deterministic cadence, and accepts only events that
// target it — all without any HTTP request, any socket, and any wire
// encoding (the endpoint paths are UNVERIFIED design data, record 21 §3;
// the simulated VEN never touches them, it touches the planner and the
// registry).
//
// Deterministic and honest: the VEN's cadence is its agreed frequency
// (a parameter), there is no wall-clock read, and every identity string
// carries the `sim-ven` prefix — never presentable as a real VEN.

use flexcore_openadr_adapter::model::{ModelError, OpenAdrEvent, Ven};
use flexcore_openadr_adapter::scope::DeliveryMode;
use flexcore_openadr_adapter::vtn::{DeliveryPlan, DeliveryPlanner, VtnRegistry, VtnRegistryError};

/// The identity prefix every simulated VEN carries in its identity
/// strings (never presentable as a real VEN).
pub const SIM_VEN_ID_PREFIX: &str = "sim-ven";

/// Errors the simulated VEN reports. Every failure is named.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SimVenError {
    /// The VTN-side registry refused (re-raised: registration, target
    /// resolution, and acceptance are the VTN's own named refusals).
    Registry(VtnRegistryError),
    /// The VEN record the simulated peer would present is invalid (the
    /// model constructor's own validation, re-raised).
    InvalidVen(ModelError),
    /// The event does not target this VEN — the simulated VEN refuses to
    /// accept an event addressed elsewhere (a VEN that accepts events
    /// not targeted at it would be fabricating delivery).
    EventNotTargeted { event_id: String, ven_id: String },
    /// The planner could not plan a delivery for this VEN in the
    /// requested modes (re-raised from the planner's own refusal).
    NoDeliveryPlan { ven_id: String, requested: String },
}

impl std::fmt::Display for SimVenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SimVenError::Registry(error) => write!(f, "the VTN registry refused: {error}"),
            SimVenError::InvalidVen(error) => write!(f, "invalid simulated VEN record: {error}"),
            SimVenError::EventNotTargeted { event_id, ven_id } => write!(
                f,
                "event '{event_id}' does not target the simulated VEN '{ven_id}'; it is not accepted"
            ),
            SimVenError::NoDeliveryPlan { ven_id, requested } => write!(
                f,
                "no delivery plan for VEN '{ven_id}' in the requested modes ({requested})"
            ),
        }
    }
}

impl std::error::Error for SimVenError {}

impl From<VtnRegistryError> for SimVenError {
    fn from(error: VtnRegistryError) -> Self {
        SimVenError::Registry(error)
    }
}

impl From<ModelError> for SimVenError {
    fn from(error: ModelError) -> Self {
        SimVenError::InvalidVen(error)
    }
}

/// The simulated VEN: a registered, deterministic client peer over the
/// VTN's data surface.
#[derive(Debug, Clone)]
pub struct SimulatedVen {
    /// The VEN record the simulated peer presents (identity carries the
    /// `sim-ven` prefix).
    pub ven: Ven,
    /// The delivery modes the simulated VEN is willing to use, in
    /// preference order (its request to the planner).
    pub requested_modes: Vec<DeliveryMode>,
    /// The events the simulated VEN has accepted, in acceptance order.
    accepted_events: Vec<OpenAdrEvent>,
}

impl SimulatedVen {
    /// Constructs the simulated VEN: a polling VEN at the agreed
    /// frequency unless a push endpoint is declared. The `accepted`
    /// flag is the OPERATOR's enrolment decision (record 21 §3:
    /// acceptance is never inferred from protocol traffic) — the
    /// simulated VEN presents whatever the caller records, and an
    /// un-accepted simulated VEN gets no delivery plan, exactly like a
    /// real one.
    pub fn new(
        ven_id: impl Into<String>,
        poll_frequency_millis: i64,
        push_endpoint: Option<String>,
        accepted: bool,
        requested_modes: Vec<DeliveryMode>,
    ) -> Result<Self, SimVenError> {
        let raw_id = ven_id.into();
        let ven = Ven::new(
            format!("{SIM_VEN_ID_PREFIX}:{raw_id}"),
            format!("Simulated VEN {raw_id} (TEST/simulation-only; never a real VEN)"),
            push_endpoint,
            poll_frequency_millis,
            accepted,
        )?;
        Ok(Self { ven, requested_modes, accepted_events: Vec::new() })
    }

    /// Registers the simulated VEN in a VTN registry (the enrolment
    /// record; acceptance stays the operator flag the record carries).
    pub fn register_in(&self, registry: &mut VtnRegistry) -> Result<(), SimVenError> {
        registry.register(self.ven.clone())?;
        Ok(())
    }

    /// Resolves the simulated VEN's delivery plan through the planner
    /// (polling surface: the designed poll route at the agreed
    /// frequency; push surface: the registered endpoint). No HTTP
    /// request is made — the plan is a RECORD (record 21 §3), and the
    /// simulated VEN's execution of the plan is the deterministic poll
    /// cadence it exposes via [`SimulatedVen::poll_frequency_millis`].
    pub fn resolve_delivery_plan(
        &self,
        registry: &VtnRegistry,
        planner: &DeliveryPlanner,
    ) -> Result<DeliveryPlan, SimVenError> {
        planner
            .plan_for(registry, &self.ven.ven_id, &self.requested_modes)
            .map_err(|_| SimVenError::NoDeliveryPlan {
                ven_id: self.ven.ven_id.clone(),
                requested: self
                    .requested_modes
                    .iter()
                    .map(|m| m.name())
                    .collect::<Vec<_>>()
                    .join("|"),
                // The planner's own refusal reason (unknown/unaccepted
                // VEN, unservable mode) is the underlying error's
                // Display; the re-raised named refusal here marks the
                // simulated peer's planning outcome.
            })
    }

    /// The deterministic poll cadence: the AGREED poll frequency in
    /// milliseconds (the simulated VEN polls exactly this often; the
    /// planner's plan surface names the same frequency).
    pub fn poll_frequency_millis(&self) -> i64 {
        self.ven.poll_frequency_millis
    }

    /// Receives one event: accepted only when the event targets this
    /// VEN (by id) — an event addressed elsewhere is a named refusal,
    /// never a silently swallowed delivery. Deterministic: acceptance
    /// order is receive order.
    pub fn receive_event(&mut self, event: OpenAdrEvent) -> Result<(), SimVenError> {
        if !event.target_ven_ids.contains(&self.ven.ven_id) {
            return Err(SimVenError::EventNotTargeted {
                event_id: event.descriptor.event_id,
                ven_id: self.ven.ven_id.clone(),
            });
        }
        self.accepted_events.push(event);
        Ok(())
    }

    /// The accepted events, in acceptance order.
    pub fn accepted_events(&self) -> &[OpenAdrEvent] {
        &self.accepted_events
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flexcore_openadr_adapter::model::{
        EventDescriptor, EventSignal, Interval, SignalUnits,
    };

    fn planner() -> DeliveryPlanner {
        DeliveryPlanner::new("openadr-vtn", vec![DeliveryMode::Polling, DeliveryMode::Push])
            .expect("valid planner")
    }

    fn event_for(ven_id: &str) -> OpenAdrEvent {
        let descriptor = EventDescriptor::new(
            "sim-evt-1".to_string(),
            1,
            "sim-market".to_string(),
            1,
            false,
        )
        .expect("valid descriptor");
        let signal = EventSignal::new(
            "simple".to_string(),
            SignalUnits::Kilowatts,
            vec![Interval::new(1, 1_700_000_000_000, 900_000, 42.0).expect("valid interval")],
        )
        .expect("valid signal");
        OpenAdrEvent::new(
            descriptor,
            vec![ven_id.to_string()],
            vec![signal],
            "sim-program".to_string(),
        )
        .expect("valid event")
    }

    #[test]
    fn a_registered_accepted_polling_ven_resolves_its_plan() {
        let mut registry = VtnRegistry::new("openadr-vtn");
        let ven = SimulatedVen::new(
            "ven-1",
            60_000,
            None,
            true,
            vec![DeliveryMode::Polling],
        )
        .expect("valid simulated ven");
        ven.register_in(&mut registry).expect("registered");
        let plan = ven
            .resolve_delivery_plan(&registry, &planner())
            .expect("polling plan");
        assert_eq!(plan.mode, DeliveryMode::Polling);
        assert_eq!(ven.poll_frequency_millis(), 60_000);
        assert!(plan.describe().contains("60000 ms"), "the plan names the agreed frequency");
    }

    #[test]
    fn an_unaccepted_ven_gets_no_plan_like_a_real_one() {
        let mut registry = VtnRegistry::new("openadr-vtn");
        let ven = SimulatedVen::new(
            "ven-pending",
            60_000,
            None,
            false,
            vec![DeliveryMode::Polling],
        )
        .expect("valid simulated ven");
        ven.register_in(&mut registry).expect("registered");
        // The planner refuses an unaccepted VEN — the operator enrolment
        // gate is not bypassed by being simulated.
        assert!(ven.resolve_delivery_plan(&registry, &planner()).is_err());
    }

    #[test]
    fn events_not_targeted_at_the_ven_are_refused() {
        let mut ven = SimulatedVen::new(
            "ven-1",
            60_000,
            None,
            true,
            vec![DeliveryMode::Polling],
        )
        .expect("valid simulated ven");
        // The simulated VEN's registry id carries the sim prefix, so the
        // event targeting the RAW id is not targeted at the simulated
        // record — refused.
        let error = ven.receive_event(event_for("ven-1")).expect_err("raw id does not match");
        assert!(matches!(error, SimVenError::EventNotTargeted { .. }));
        // An event targeting the simulated record's full id is accepted.
        let full_id = ven.ven.ven_id.clone();
        ven.receive_event(event_for(&full_id)).expect("accepted");
        assert_eq!(ven.accepted_events().len(), 1);
    }

    #[test]
    fn a_push_ven_resolves_the_push_surface_when_it_registered_one() {
        let mut registry = VtnRegistry::new("openadr-vtn");
        let ven = SimulatedVen::new(
            "ven-push",
            60_000,
            Some("https://sim.invalid/push".to_string()),
            true,
            vec![DeliveryMode::Push, DeliveryMode::Polling],
        )
        .expect("valid simulated ven");
        ven.register_in(&mut registry).expect("registered");
        let plan = ven.resolve_delivery_plan(&registry, &planner()).expect("push plan");
        assert_eq!(plan.mode, DeliveryMode::Push);
        assert!(plan.describe().contains("https://sim.invalid/push"));
    }
}
