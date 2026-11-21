// FIM-F04.02 / FIM-F04.03 — the VTN SERVING STATE: the stateful handler
// logic the designed endpoint seam (`crate::endpoints`) routes onto
// (docs/releases/final-implementation-before-publication.md §5 F-04; ledger
// docs/releases/implementation_ledger/ledger_F04.md).
//
// **Implemented under FIM-F04, WRITTEN-UNRUN: this module has never been
// compiled or executed. VEN interoperability is UNPROVEN and remains a
// HUMAN LAB GATE** (crate conventions §7.3). Nothing in this module is a
// conformance result; every OpenADR protocol detail below stays UNVERIFIED
// against the 3.0 specification text [S-10] (record 21 §5 register).
//
// What the module implements, mapped onto the EXISTING models and the
// EXISTING registry/planner (`crate::model`, `crate::vtn` — unchanged):
//
// - **FIM-F04.02 — registration, poll, and report serving.**
//   [`VtnServingState`] composes the existing [`VtnRegistry`] and
//   [`DeliveryPlanner`]:
//   - `register_ven` revalidates the wire record through the EXISTING
//     `Ven::new` validation and routes it through the EXISTING
//     register/update paths. The wire can NEVER flip operator acceptance:
//     a NEW enrolment record is created with `accepted == false`
//     (acceptance stays a human-reviewed operator decision — the
//     `/registration` route purpose and `model.rs::Ven` rule), and an
//     UPDATE preserves the recorded operator acceptance byte-for-byte.
//   - `poll` executes the [`DeliveryPlanner`] plan the poll route declares
//     (polling mode): the planner's named refusals (unknown VEN, VEN not
//     accepted, mode unavailable) pass through VERBATIM — this module
//     invents no refusal text.
//   - `submit_report` revalidates the wire record through the EXISTING
//     `ReportSpec::new` validation (structurally telemetry-only) and
//     records it against the registered VEN; report payloads are telemetry
//     records and never touch the command lifecycle.
// - **FIM-F04.03 — delivery recording with the event version in force.**
//   A served poll writes one [`DeliveryRecord`] per delivered event, whose
//   version field is the event's VERBATIM
//   `EventDescriptor::modification_number` — the only version concept this
//   crate has (record 21). NO new version vocabulary is introduced.
// - **Event ingest.** `queue_event` is the composition root's publish
//   path: the event is revalidated through the EXISTING `OpenAdrEvent::new`
//   validation and its targets are resolved through the EXISTING
//   `VtnRegistry::resolve_targets` (unknown or not-accepted VENs are the
//   registry's named refusals). The queued events are what the poll route
//   serves. No command lifecycle state is constructed here: the event →
//   PROPOSED mapping stays in `crate::event_mapping`, untouched.
//
// What this module is NOT (declared, fail-closed):
//
// - No socket, no HTTP, no runtime — the wire mapping and the serving loop
//   live in `crate::transport`; this module is the handler logic the
//   service calls (the 20305 server/transport split pattern).
// - **No outbound push sender.** `notify_push` validates the VEN's push
//   plan and queues the event for the VEN's agreed surface; the VTN → VEN
//   outbound HTTP call is NOT implemented in this written-unrun program,
//   so NO [`DeliveryRecord`] is written by a push notification — delivery
//   records exist only for completed poll servings (a poll response IS the
//   delivered bytes). A poll later serves the queued event and records the
//   delivery with the version in force.

use std::collections::BTreeMap;

use crate::model::{OpenAdrEvent, ReportSpec, Ven};
use crate::scope::DeliveryMode;
use crate::vtn::{
    DeliveryPlan, DeliveryPlanError, DeliveryPlanner, VtnRegistry, VtnRegistryError,
};

/// Errors the serving state reports to the wire layer. Wrapped errors
/// surface through `Display` VERBATIM as the inner error's own text (the
/// wire layer puts the recorded text byte-identical into the error body) —
/// this module invents no refusal vocabulary for the registry, planner, or
/// model paths. Only the wire-level body decode failure has new text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServingError {
    /// The VEN registry refused (unknown VEN, duplicate registration,
    /// invalid record, VEN not accepted, delivery mode unavailable — the
    /// EXISTING [`VtnRegistryError`] vocabulary, text verbatim).
    Registry(VtnRegistryError),
    /// The delivery planner refused (the EXISTING [`DeliveryPlanError`]
    /// vocabulary, text verbatim).
    Planner(DeliveryPlanError),
    /// The event/VEN/report model validation refused (the EXISTING
    /// `crate::model::ModelError` vocabulary, text verbatim).
    Model(crate::model::ModelError),
    /// The request body did not decode (wire-level; the only NEW text this
    /// module contributes — the arrival payload is not a valid instance of
    /// the designed payload kind).
    MalformedBody { reason: String },
}

impl std::fmt::Display for ServingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            // The recorded text of the existing vocabularies, verbatim.
            ServingError::Registry(err) => write!(f, "{err}"),
            ServingError::Planner(err) => write!(f, "{err}"),
            ServingError::Model(err) => write!(f, "{err}"),
            ServingError::MalformedBody { reason } => {
                write!(f, "request body is not a valid payload for this route: {reason}")
            }
        }
    }
}

impl std::error::Error for ServingError {}

impl From<VtnRegistryError> for ServingError {
    fn from(err: VtnRegistryError) -> Self {
        ServingError::Registry(err)
    }
}

impl From<DeliveryPlanError> for ServingError {
    fn from(err: DeliveryPlanError) -> Self {
        ServingError::Planner(err)
    }
}

/// One COMPLETED event delivery: a poll response served the event's bytes
/// to the VEN. FIM-F04.03: the record carries the event VERSION IN FORCE
/// at delivery — the verbatim `EventDescriptor::modification_number`
/// (existing model field; no new version concept).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliveryRecord {
    /// The VEN the delivery served.
    pub ven_id: String,
    /// The verbatim event id (audit trace: delivery ← event).
    pub event_id: String,
    /// The event version in force at delivery: the verbatim
    /// `EventDescriptor::modification_number` of the event that was served.
    pub modification_number: u32,
    /// The verbatim program id.
    pub program_id: String,
    /// The declared delivery mode that served the delivery (the planner's
    /// plan mode — polling today; a push record is never written here
    /// because no outbound push sender exists in this written-unrun
    /// program, see the module header).
    pub mode: DeliveryMode,
}

/// The outcome of a wire registration (`/registration`): whether the VEN
/// record was CREATED (first enrolment) or UPDATED (re-registration over
/// an existing record). The wire response status follows (201 / 200).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VenRegistrationOutcome {
    /// A new enrolment record was created (`accepted == false`; the
    /// operator surface owns acceptance).
    Created,
    /// The existing record was replaced through the explicit update path;
    /// the operator acceptance flag was PRESERVED.
    Updated,
}

/// The VTN serving state: the handler logic behind the designed endpoint
/// seam. Composes the EXISTING [`VtnRegistry`] and [`DeliveryPlanner`]
/// (both constructed here and owned; no parallel inventory exists).
/// Construction requires the adapter id and the declared delivery-mode
/// order (the reviewed deployment declaration — the planner refuses an
/// empty order, so no `Default`).
#[derive(Debug, Clone)]
pub struct VtnServingState {
    /// The adapter id this state serves for (the registry's and planner's
    /// id; one spelling, no parallel value).
    pub adapter_id: String,
    /// The EXISTING VEN registry (BTreeMap inventory; register/update
    /// separation; named refusals).
    pub registry: VtnRegistry,
    /// The EXISTING delivery planner (declared mode order; per-VEN plans).
    pub planner: DeliveryPlanner,
    /// Pending (undelivered) events per VEN id, in queue order. Served —
    /// and drained — by `poll`.
    pending: BTreeMap<String, Vec<OpenAdrEvent>>,
    /// Accepted report specs per VEN id, in submission order.
    reports: BTreeMap<String, Vec<ReportSpec>>,
    /// The completed-delivery log (poll servings, in serving order).
    deliveries: Vec<DeliveryRecord>,
}

impl VtnServingState {
    /// Constructs the serving state over a fresh [`VtnRegistry`] and
    /// [`DeliveryPlanner`] for the given adapter id and declared
    /// delivery-mode order (non-empty enforced by the planner).
    pub fn new(
        adapter_id: impl Into<String>,
        declared_mode_order: Vec<DeliveryMode>,
    ) -> Result<Self, DeliveryPlanError> {
        let adapter_id = adapter_id.into();
        let registry = VtnRegistry::new(adapter_id.clone());
        let planner = DeliveryPlanner::new(adapter_id.clone(), declared_mode_order)?;
        Ok(Self {
            adapter_id,
            registry,
            planner,
            pending: BTreeMap::new(),
            reports: BTreeMap::new(),
            deliveries: Vec::new(),
        })
    }

    // --- FIM-F04.02: registration ------------------------------------------

    /// Registers a VEN from a wire record: revalidates through the
    /// EXISTING `Ven::new` validation (the wire never bypasses model
    /// validation), then routes through the EXISTING register/update
    /// paths. The operator acceptance flag is OWNED by the operator
    /// surface: a created record starts `accepted == false`; an updated
    /// record preserves the recorded flag — the wire cannot flip either.
    pub fn register_ven(&mut self, wire: Ven) -> Result<VenRegistrationOutcome, ServingError> {
        // 1. The EXISTING model validation (a wire record with an empty
        //    name, a non-positive poll frequency, or an empty push
        //    endpoint is refused here with the recorded model text).
        let validated = Ven::new(
            wire.ven_id.clone(),
            wire.ven_name,
            wire.push_endpoint,
            wire.poll_frequency_millis,
            wire.accepted,
        )
        .map_err(ServingError::Model)?;
        // 2. The operator surface owns `accepted` (the `model.rs::Ven`
        //    rule): never inferred from protocol traffic, never flipped by
        //    a wire field.
        let operator_accepted = self
            .registry
            .ven(&validated.ven_id)
            .map(|ven| ven.accepted)
            .unwrap_or(false);
        let mut record = validated;
        record.accepted = operator_accepted;
        // 3. The EXISTING registry paths: create, or the explicit update
        //    path for a re-registration (never a silent overwrite of a
        //    different VEN — the registry's own rule).
        if self.registry.ven(&record.ven_id).is_some() {
            self.registry.update(record)?;
            Ok(VenRegistrationOutcome::Updated)
        } else {
            self.registry.register(record)?;
            Ok(VenRegistrationOutcome::Created)
        }
    }

    // --- Event ingest (the composition root's publish path) ----------------

    /// Queues an event for its declared targets: revalidates through the
    /// EXISTING `OpenAdrEvent::new` validation and resolves the targets
    /// through the EXISTING `VtnRegistry::resolve_targets` (unknown and
    /// not-accepted VENs are the registry's named refusals). Each target
    /// VEN gets the event in its pending queue, in queue order. Returns
    /// the resolved target ids. NO lifecycle state is constructed here —
    /// the event → PROPOSED mapping stays in `crate::event_mapping`.
    pub fn queue_event(&mut self, event: OpenAdrEvent) -> Result<Vec<String>, ServingError> {
        let validated = OpenAdrEvent::new(
            event.descriptor,
            event.target_ven_ids,
            event.signals,
            event.program_id,
        )
        .map_err(ServingError::Model)?;
        let targets: Vec<String> = self
            .registry
            .resolve_targets(&validated.target_ven_ids)?
            .iter()
            .map(|ven| ven.ven_id.clone())
            .collect();
        for ven_id in &targets {
            self.pending
                .entry(ven_id.clone())
                .or_default()
                .push(validated.clone());
        }
        Ok(targets)
    }

    // --- FIM-F04.02 + FIM-F04.03: the poll route ---------------------------

    /// Serves one poll for the VEN: executes the delivery plan the poll
    /// route declares (polling mode — the planner's named refusals pass
    /// through verbatim), drains the VEN's pending events, and writes one
    /// [`DeliveryRecord`] per delivered event with the event version in
    /// force (FIM-F04.03). Returns the delivered events (the poll
    /// response body).
    pub fn poll(&mut self, ven_id: &str) -> Result<Vec<OpenAdrEvent>, ServingError> {
        // 1. The plan the poll route serves: polling mode, per the
        //    designed `/poll/{ven_id}` route's declared delivery mode. The
        //    planner's refusals (unknown VEN, not accepted, mode
        //    unavailable) surface VERBATIM.
        let plan = self
            .planner
            .plan_for(&self.registry, ven_id, &[DeliveryMode::Polling])?;
        let DeliveryPlan { mode, .. } = plan;
        // 2. Deliver: drain the VEN's pending events (queue order) and
        //    record each delivery with the version in force.
        let drained = self.pending.remove(ven_id).unwrap_or_default();
        for event in &drained {
            self.deliveries.push(DeliveryRecord {
                ven_id: ven_id.to_string(),
                event_id: event.descriptor.event_id.clone(),
                modification_number: event.descriptor.modification_number,
                program_id: event.program_id.clone(),
                mode,
            });
        }
        Ok(drained)
    }

    // --- FIM-F04.02: the report route --------------------------------------

    /// Serves one report-spec submission: revalidates through the EXISTING
    /// `ReportSpec::new` validation (structurally telemetry-only — report
    /// payloads never touch the command lifecycle) and records the spec
    /// against the REGISTERED VEN (an unknown VEN is the registry's named
    /// refusal, verbatim). Returns nothing — the receipt is the wire
    /// layer's acknowledgement.
    pub fn submit_report(&mut self, ven_id: &str, wire: ReportSpec) -> Result<(), ServingError> {
        let validated = ReportSpec::new(
            wire.report_specifier_id,
            wire.report_name,
            wire.granularity_millis,
        )
        .map_err(ServingError::Model)?;
        if self.registry.ven(ven_id).is_none() {
            return Err(VtnRegistryError::VenNotRegistered {
                ven_id: ven_id.to_string(),
            }
            .into());
        }
        self.reports
            .entry(ven_id.to_string())
            .or_default()
            .push(validated);
        Ok(())
    }

    // --- The push route (planned, queued — never claimed delivered) --------

    /// Serves one push notification request for the VEN: executes the
    /// push plan the push route declares (the planner refuses a VEN with
    /// no registered push endpoint, verbatim) and QUEUES the event in the
    /// VEN's pending queue. NO [`DeliveryRecord`] is written: the VTN →
    /// VEN outbound call is not implemented in this written-unrun program
    /// (module header) — a delivery is recorded only when a poll serves
    /// the event. Returns the plan that was executed.
    pub fn notify_push(&mut self, ven_id: &str, event: OpenAdrEvent) -> Result<DeliveryPlan, ServingError> {
        let validated = OpenAdrEvent::new(
            event.descriptor,
            event.target_ven_ids,
            event.signals,
            event.program_id,
        )
        .map_err(ServingError::Model)?;
        let plan = self
            .planner
            .plan_for(&self.registry, ven_id, &[DeliveryMode::Push])?;
        let DeliveryPlan { ven_id, mode, surface } = plan;
        self.pending
            .entry(ven_id.clone())
            .or_default()
            .push(validated);
        Ok(DeliveryPlan { ven_id, mode, surface })
    }

    // --- Read accessors (the operator / VEN inventory surface) --------------

    /// The report specs recorded for one VEN (in submission order; empty
    /// when none).
    pub fn reports_for(&self, ven_id: &str) -> &[ReportSpec] {
        self.reports
            .get(ven_id)
            .map(|specs| specs.as_slice())
            .unwrap_or(&[])
    }

    /// The completed-delivery log (in serving order).
    pub fn delivery_log(&self) -> &[DeliveryRecord] {
        &self.deliveries
    }

    /// The number of pending (undelivered) events for one VEN.
    pub fn pending_count(&self, ven_id: &str) -> usize {
        self.pending
            .get(ven_id)
            .map(|events| events.len())
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{EventDescriptor, EventSignal, Interval, SignalUnits};

    fn accepted_polling_ven(id: &str) -> Ven {
        Ven::new(id.to_string(), format!("VEN {id}"), None, 60_000, true).expect("valid ven")
    }

    fn state() -> VtnServingState {
        VtnServingState::new("openadr-vtn", vec![DeliveryMode::Polling, DeliveryMode::Push])
            .expect("valid state")
    }

    fn event(modification_number: u32) -> OpenAdrEvent {
        let descriptor = EventDescriptor::new(
            "evt-42".to_string(),
            modification_number,
            "iso-ne".to_string(),
            3,
            false,
        )
        .expect("valid descriptor");
        let signal = EventSignal::new(
            "simple".to_string(),
            SignalUnits::Kilowatts,
            vec![Interval::new(1, 1_700_000_000_000, 1_800_000, 2_000.0).expect("valid interval")],
        )
        .expect("valid signal");
        OpenAdrEvent::new(
            descriptor,
            vec!["ven-1".to_string()],
            vec![signal],
            "program-7".to_string(),
        )
        .expect("valid event")
    }

    // --- FIM-F04.02: registration -------------------------------------------

    #[test]
    fn wire_registration_creates_unaccepted_and_update_preserves_operator_acceptance() {
        let mut vt = state();
        // A wire record claiming acceptance is created UNACCEPTED: the
        // operator surface owns the flag (model.rs rule), never the wire.
        let wire = Ven::new(
            "ven-1".to_string(),
            "VEN 1".to_string(),
            None,
            60_000,
            true,
        )
        .expect("valid wire ven");
        assert_eq!(
            vt.register_ven(wire).expect("registered"),
            VenRegistrationOutcome::Created
        );
        let created = vt.registry.ven("ven-1").expect("present");
        assert!(!created.accepted, "a wire registration is never self-accepted");
        // The operator accepts the VEN (the operator surface's decision).
        let operator_record = accepted_polling_ven("ven-1");
        vt.registry.update(operator_record).expect("operator update");
        // Re-registration over the wire PRESERVES the operator acceptance
        // and updates the agreed poll frequency.
        let wire = Ven::new(
            "ven-1".to_string(),
            "VEN 1 renamed".to_string(),
            None,
            30_000,
            false,
        )
        .expect("valid wire ven");
        assert_eq!(
            vt.register_ven(wire).expect("updated"),
            VenRegistrationOutcome::Updated
        );
        let updated = vt.registry.ven("ven-1").expect("present");
        assert!(updated.accepted, "the wire cannot flip operator acceptance");
        assert_eq!(updated.ven_name, "VEN 1 renamed");
        assert_eq!(updated.poll_frequency_millis, 30_000);
        // A wire record failing the EXISTING model validation is refused
        // with the recorded model text.
        let invalid = Ven::new("ven-bad".to_string(), String::new(), None, 60_000, false)
            .expect_err("Ven::new refuses the empty name");
        assert_eq!(
            vt.register_ven(
                Ven {
                    ven_id: "ven-bad".to_string(),
                    ven_name: String::new(),
                    push_endpoint: None,
                    poll_frequency_millis: 60_000,
                    accepted: false,
                }
            ),
            Err(ServingError::Model(invalid))
        );
    }

    // --- FIM-F04.02 + FIM-F04.03: poll delivery ------------------------------

    #[test]
    fn poll_delivers_the_queued_events_and_records_the_version_in_force() {
        let mut vt = state();
        vt.registry
            .register(accepted_polling_ven("ven-1"))
            .expect("register");
        // The composition root queues modification 1, then modification 2
        // (a re-published event is a NEW pending entry; SUPERSEDED
        // invalidation is the lifecycle's own edge, not the adapter's).
        vt.queue_event(event(1)).expect("queued");
        vt.queue_event(event(2)).expect("queued");
        assert_eq!(vt.pending_count("ven-1"), 2);
        // The poll delivers BOTH entries and records each with the version
        // in force at its delivery.
        let delivered = vt.poll("ven-1").expect("delivered");
        assert_eq!(delivered.len(), 2);
        assert_eq!(delivered[0].descriptor.modification_number, 1);
        assert_eq!(delivered[1].descriptor.modification_number, 2);
        let log = vt.delivery_log();
        assert_eq!(log.len(), 2);
        assert_eq!(log[0].ven_id, "ven-1");
        assert_eq!(log[0].event_id, "evt-42");
        assert_eq!(log[0].modification_number, 1, "the version in force");
        assert_eq!(log[1].modification_number, 2, "the version in force");
        assert_eq!(log[0].mode, DeliveryMode::Polling);
        assert_eq!(log[0].program_id, "program-7");
        // The queue is drained: a second poll delivers nothing and records
        // nothing.
        assert!(vt.poll("ven-1").expect("empty poll").is_empty());
        assert_eq!(vt.delivery_log().len(), 2);
    }

    // --- FIM-F04.02: report upload -------------------------------------------

    #[test]
    fn report_upload_lands_in_the_existing_report_model_with_its_validation() {
        let mut vt = state();
        vt.registry
            .register(accepted_polling_ven("ven-1"))
            .expect("register");
        let wire = ReportSpec::new("r-1".to_string(), "telemetry".to_string(), 60_000)
            .expect("valid spec");
        vt.submit_report("ven-1", wire).expect("recorded");
        let specs = vt.reports_for("ven-1");
        assert_eq!(specs.len(), 1);
        assert_eq!(specs[0].report_specifier_id, "r-1");
        // The EXISTING structural rule is intact: report payloads are
        // telemetry records, never commands.
        assert!(specs[0].telemetry_only);
        // The EXISTING validation refuses a non-positive granularity with
        // the recorded model text.
        let invalid = ReportSpec::new("r-2".to_string(), "bad".to_string(), 0)
            .expect_err("ReportSpec::new refuses a zero granularity");
        assert_eq!(
            vt.submit_report("ven-1", ReportSpec {
                report_specifier_id: "r-2".to_string(),
                report_name: "bad".to_string(),
                granularity_millis: 0,
                telemetry_only: false,
            }),
            Err(ServingError::Model(invalid))
        );
        // An unknown VEN is the registry's named refusal, verbatim.
        assert_eq!(
            vt.submit_report(
                "ven-z",
                ReportSpec::new("r-3".to_string(), "t".to_string(), 60_000).expect("valid")
            ),
            Err(ServingError::Registry(VtnRegistryError::VenNotRegistered {
                ven_id: "ven-z".to_string()
            }))
        );
    }

    // --- Named refusals (FIM-F04.04 error paths at the logic level) ---------

    #[test]
    fn refusals_pass_through_with_the_recorded_text_verbatim() {
        let mut vt = state();
        // Unknown VEN poll: the planner's registry refusal, verbatim.
        assert_eq!(
            vt.poll("ven-z"),
            Err(ServingError::Planner(DeliveryPlanError::Registry(
                VtnRegistryError::VenNotRegistered {
                    ven_id: "ven-z".to_string()
                }
            )))
        );
        // A registered-but-not-accepted VEN poll: the recorded not-accepted
        // text, verbatim.
        vt.registry
            .register(Ven::new(
                "ven-pending".to_string(),
                "pending".to_string(),
                None,
                60_000,
                false,
            )
            .expect("valid ven"))
            .expect("register");
        assert_eq!(
            vt.poll("ven-pending"),
            Err(ServingError::Planner(DeliveryPlanError::Registry(
                VtnRegistryError::VenNotAccepted {
                    ven_id: "ven-pending".to_string()
                }
            )))
        );
        // The wrapped Display text is byte-identical to the inner error's.
        let expected = VtnRegistryError::VenNotRegistered {
            ven_id: "ven-z".to_string(),
        }
        .to_string();
        let surfaced = match vt.poll("ven-z") {
            Err(err) => err.to_string(),
            Ok(_) => panic!("unreachable"),
        };
        assert_eq!(surfaced, expected);
        // A push notification for a VEN without a registered push endpoint
        // is the planner's recorded refusal, verbatim.
        vt.registry
            .register(accepted_polling_ven("ven-1"))
            .expect("register");
        assert!(matches!(
            vt.notify_push("ven-1", event(1)),
            Err(ServingError::Planner(DeliveryPlanError::ModeUnavailable { .. }))
        ));
        // An event whose targets include an unknown VEN is refused at
        // queue time by the EXISTING target resolution.
        let descriptor = EventDescriptor::new(
            "evt-x".to_string(),
            1,
            "m".to_string(),
            1,
            false,
        )
        .expect("valid");
        let signal = EventSignal::new(
            "simple".to_string(),
            SignalUnits::Kilowatts,
            vec![Interval::new(1, 0, 1000, 1.0).expect("valid")],
        )
        .expect("valid");
        let bad_targets = OpenAdrEvent::new(
            descriptor,
            vec!["ven-1".to_string(), "ven-ghost".to_string()],
            vec![signal],
            "p".to_string(),
        )
        .expect("valid event");
        assert_eq!(
            vt.queue_event(bad_targets),
            Err(ServingError::Registry(VtnRegistryError::VenNotRegistered {
                ven_id: "ven-ghost".to_string()
            }))
        );
    }

    // --- The push route: queued, never claimed delivered ---------------------

    #[test]
    fn push_notification_queues_the_event_and_writes_no_delivery_record() {
        let mut vt = state();
        vt.registry
            .register(Ven::new(
                "ven-push".to_string(),
                "push VEN".to_string(),
                Some("https://ven.example/push".to_string()),
                60_000,
                true,
            )
            .expect("valid ven"))
            .expect("register");
        let plan = vt.notify_push("ven-push", event(3)).expect("push planned");
        assert_eq!(plan.mode, DeliveryMode::Push);
        assert_eq!(plan.surface, "https://ven.example/push");
        assert_eq!(plan.ven_id, "ven-push");
        // Queued for the agreed surface — but NOT recorded as delivered
        // (no outbound push sender exists in this written-unrun program).
        assert_eq!(vt.pending_count("ven-push"), 1);
        assert!(vt.delivery_log().is_empty());
        // The agreed poll later serves it and records the delivery with
        // the version in force.
        let delivered = vt.poll("ven-push").expect("delivered");
        assert_eq!(delivered.len(), 1);
        assert_eq!(vt.delivery_log()[0].modification_number, 3);
    }

    // --- FIM-F04.02: a VEN state without a declared polling mode -------------

    #[test]
    fn a_state_whose_declared_modes_exclude_polling_refuses_polls_with_the_recorded_text() {
        // A planner declared push-only serves no poll route: the recorded
        // ModeUnavailable text, verbatim (the wrong-mode error path).
        let mut vt = VtnServingState::new("openadr-vtn", vec![DeliveryMode::Push])
            .expect("valid state");
        vt.registry
            .register(accepted_polling_ven("ven-1"))
            .expect("register");
        let expected = DeliveryPlanError::ModeUnavailable {
            ven_id: "ven-1".to_string(),
            mode: "polling".to_string(),
        }
        .to_string();
        let surfaced = match vt.poll("ven-1") {
            Err(err) => err.to_string(),
            Ok(_) => panic!("a push-only state must not serve polls"),
        };
        assert_eq!(surfaced, expected);
    }
}
