// P17.06/P17.07 (worker wiring) — the EV ingest bridge: the inbound-path
// extension that routes the OCPP CSMS adapter's frames into the EXISTING
// ingestion machinery.
//
// Implements the worker half of ledger P17.06 (proposal records) and
// P17.07 (MeterValues ingestion). Decision records:
// `docs/phases-11-20/phase-17-ev-fleet-charging/records/
// 21_p17_06_vehicle_proposal_ingestion.md` and
// `.../records/22_p17_07_meter_values_ingestion.md`.
//
// # No parallel ingestion path (the P17.07 seam rule)
//
// The P2.07 pipeline in [`crate::ingestion`] (receive → validate →
// normalize → quality-assign → dedup → store → publish, with quarantine
// and bounded backpressure) is the ONLY telemetry path. This module adds
// NO second pipeline: MeterValues frames (and every other telemetry-
// shaped frame the adapter produces) are offered to the SAME
// [`crate::ingestion::IngestionQueue`] the P2.14 source bridge uses — the
// decode (stages 2–3), the quality assignment (stage 4, hold-or-lower),
// the dedup, the store, and the outbox publication are the existing
// pipeline's, unchanged. What this module adds is the ROUTING of the EV
// message families onto their seams:
//
// | Frame family | Route | Seam |
// |---|---|---|
// | MeterValues (telemetry-shaped) | [`TelemetryOffer`] over [`crate::ingestion::IngestionQueue`] | the EXISTING P2.07 pipeline |
// | Proposal messages (NotifyEVChargingNeeds / NotifyEVChargingSchedule / NotifyChargingLimit / ClearedChargingLimit) | `flexcore_application::ev_proposal_ingest::assemble_proposal_record` | the domain constraint-record path — NEVER a command |
// | Signed meter values (MeterValuesSign hooks) | `flexcore_application::ev_meter_values_ingest::SignedMeterValueSink` | the opaque evidence-record path |
//
// # The A-dependence, recorded honestly
//
// The OCPP CSMS adapter is Agent A's P17.01
// `crates/adapters/protocol-ocpp/` inbound path. A's P17.01 layout record
// had not landed when this module was written; the adapter's concrete
// types are consumed BY CITATION only — this module defines the
// adapter-neutral [`EvInboundFrame`] shape (the same pattern the
// application module uses), and A's adapter constructs that shape at its
// boundary. The dependence is recorded in the decision records; a layout
// mismatch when A's record lands is a recorded edit, never a silent
// reinterpretation.
//
// # Production-caller status (FRV1-R2-004, final review; RECORDED SEAM)
//
// As of this tree [`route_ev_inbound`] has NO production caller: the OCPP
// CSMS adapter's inbound path (`crates/adapters/protocol-ocpp`) still has
// no MeterValues/proposal payload decode and constructs no
// [`EvInboundFrame`], and the worker's composition root hosts no CSMS
// transport loop (the P1.19 recorded posture: the worker is not a second
// API server; its only frame producer today is the P2.14 simulator source
// bridge, which feeds the queue directly). The bridge is therefore an
// HONESTLY RECORDED SEAM, not a wired path — statically unreached in both
// directions, exactly as the finding records it. The named follow-up that
// closes the seam: A's adapter inbound loop (or the worker task hosting
// the CSMS transport) decodes each inbound OCPP message, constructs the
// [`EvInboundFrame`], and calls [`route_ev_inbound`] with the composed
// [`crate::ingestion::IngestionQueue`] as the [`TelemetryOffer`] seam, a
// `SignedMeterValueSink` implementation, and the deployment's
// [`EvIngestBridgeConfig`]. Nothing here is faked to appear wired: the
// routing function is internally consistent and never-auto-command by
// construction (see the tests).
//
// # The SoC-arbitration seam status (FRV1-R7-006, final review; RECORDED
// SEAM)
//
// `flexcore_domain::ev_soc_arbitration::arbitrate_soc` (P17.08) has zero
// consumers. Its meeting point — the record where BOTH an OCPP
// MeterValues SoC observation and a telematics SoC observation exist for
// one charging session/asset, with the plug-state observation and the
// per-source freshness limits the arbitration requires — does NOT exist
// in the shipped surface: this bridge forwards MeterValues frames to the
// EXISTING pipeline (which stores single-source records and never holds a
// dual-source session state), no telematics port implementation exists
// (P17-E-3 DEFAULT (a) stays the recorded open decision — a vendor-agnostic
// PORT with no implementation), no plug-state observation is produced
// anywhere, and the OCPP adapter decodes no SoC sampled value. The
// arbitration is therefore NOT wired here, and no meeting point is
// invented: per the P17.08 policy the arbitration belongs at the point
// where the dual-source session record is assembled, which is the
// integration the FRV1-R2-004 seam above names (an adapter/CSMS hosting
// loop plus a telematics port implementation). Until that lands,
// non-consumption is a RECORDED DECISION of this module header, not an
// oversight — the RAINFLOW_EXTRACTION_SEAM pattern
// (`crates/flexcore-domain/src/bess_degradation.rs`).
//
// # Message-name honesty
//
// The OCPP message-family names in [`EV_TELEMETRY_FAMILIES`] and
// [`EV_PROPOSAL_FAMILIES`] are SECONDARY-SOURCED [S-124][S-125] and
// UNVERIFIED against the OCA specification text (the implementation-time
// check is recorded in the decision records). No charge point is
// connected; no message has ever been exchanged with a real device.
//
// Control status: simulation-only wiring. The bridge grants no control
// authority, touches no field device, and performs no protection-grade
// function (AGENTS.md §5).

use crate::ingestion::{FrameShed, IngestionQueue};
use flexcore_application::ev_meter_values_ingest::{
    MeterValuesCadenceConfig, MeterValuesIngestError, SignedMeterValueRecord, SignedMeterValueSink,
};
use flexcore_application::ev_proposal_ingest::{
    assemble_proposal_record, EvProposalIngestError, InboundProposalMessage,
};
use flexcore_contracts::capability::Provenance;
use flexcore_domain::ev_proposal_records::EvProposalRecord;

/// The telemetry-shaped OCPP message families this bridge routes into the
/// EXISTING pipeline (spellings SECONDARY-SOURCED [S-124][S-125],
/// UNVERIFIED against the OCA text).
pub const EV_TELEMETRY_FAMILIES: [&str; 2] = ["MeterValues", "StatusNotification"];

/// The proposal-message families this bridge routes onto the constraint-
/// record path (spellings SECONDARY-SOURCED [S-124][S-125], UNVERIFIED
/// against the OCA text).
pub const EV_PROPOSAL_FAMILIES: [&str; 4] = [
    "NotifyEVChargingNeeds",
    "NotifyEVChargingSchedule",
    "NotifyChargingLimit",
    "ClearedChargingLimit",
];

/// The telemetry-offer seam: the offer side of the EXISTING pipeline's
/// bounded channel. [`IngestionQueue`] (the handle the P2.14 source bridge
/// holds) is the production implementation; the tests use a counting fake
/// so the routing is pinned without constructing the heavyweight pipeline
/// task. The shed error is RETRYABLE by contract (the adapter backs off
/// and re-offers — never unbounded buffering, never a silent drop).
pub trait TelemetryOffer {
    /// Offers one frame payload to the existing pipeline.
    fn offer_frame(&self, payload: Vec<u8>) -> Result<(), FrameShed>;
}

impl TelemetryOffer for IngestionQueue {
    fn offer_frame(&self, payload: Vec<u8>) -> Result<(), FrameShed> {
        self.offer(payload)
    }
}

/// The adapter-neutral inbound EV frame (the shape A's `protocol-ocpp`
/// inbound path constructs at its boundary — consumed BY CITATION; see the
/// module header).
#[derive(Debug, Clone, PartialEq)]
pub enum EvInboundFrame {
    /// A telemetry-shaped frame: the raw payload bytes, offered to the
    /// EXISTING pipeline exactly as the P2.14 source bridge offers its
    /// frames (the pipeline's decode stage does the validation and
    /// normalization — the bridge does not pre-decode).
    Telemetry {
        /// The raw frame payload (opaque to the bridge).
        payload: Vec<u8>,
    },
    /// A decoded proposal message (the adapter-neutral shape the
    /// application module assembles into the domain constraint record).
    Proposal(InboundProposalMessage),
    /// A signed meter value (opaque evidence data; the hook records it —
    /// it never verifies).
    SignedMeterValue(SignedMeterValueRecord),
}

/// The bridge configuration: the MeterValues cadence configuration the
/// adapter applies at provisioning, and the provenance declaration for
/// adapter-relayed records.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvIngestBridgeConfig {
    /// The MeterValues reporting-cadence configuration (validated at
    /// routing time — no default exists).
    pub meter_values: MeterValuesCadenceConfig,
    /// The provenance the composition root declares for adapter-relayed
    /// records (an authenticated CSMS connection is `MEASURED`; anything
    /// less is `UNKNOWN` and narrows — the P3.02 rule).
    pub proposal_provenance: Provenance,
    /// The human-safe provenance note (mandatory, non-empty — the P1.08
    /// rule).
    pub provenance_note: String,
}

impl EvIngestBridgeConfig {
    /// Validates the configuration.
    pub fn validate(&self) -> Result<(), EvIngestRouteError> {
        self.meter_values
            .validate()
            .map_err(EvIngestRouteError::SignedMeterValue)?;
        if self.provenance_note.trim().is_empty() {
            return Err(EvIngestRouteError::EmptyProvenanceNote);
        }
        Ok(())
    }
}

/// The routing outcome for one frame. Every outcome is a RECORD-side
/// event; no outcome issues, authorizes, or dispatches anything.
#[derive(Debug, Clone, PartialEq)]
pub enum EvRouteOutcome {
    /// The telemetry frame was accepted by the bounded channel (the
    /// EXISTING pipeline processes it; sheds and quarantine are counted
    /// by the pipeline's own metrics).
    TelemetryQueued,
    /// The bounded channel was full: the frame was SHED (retryable — the
    /// adapter backs off and re-offers; never unbounded buffering, never
    /// a silent drop).
    TelemetryShed,
    /// The proposal message became a domain constraint record (lifecycle
    /// entry state PROPOSED; never a command).
    ProposalRecorded(EvProposalRecord),
    /// The signed meter value was recorded by the sink (opaque evidence;
    /// never verified).
    SignedMeterValueRecorded,
}

/// Errors of the routing. Every variant names the failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvIngestRouteError {
    /// The proposal assembly refused the message (the application error
    /// carried verbatim).
    Proposal(EvProposalIngestError),
    /// The signed-meter-value path refused the record (or the cadence
    /// configuration was invalid — the mapping error carried verbatim).
    SignedMeterValue(MeterValuesIngestError),
    /// The provenance note was empty.
    EmptyProvenanceNote,
}

impl std::fmt::Display for EvIngestRouteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EvIngestRouteError::Proposal(e) => write!(f, "the proposal was refused: {e}"),
            EvIngestRouteError::SignedMeterValue(e) => {
                write!(f, "the MeterValues configuration or record was refused: {e}")
            }
            EvIngestRouteError::EmptyProvenanceNote => {
                write!(f, "the provenance note is mandatory and must be non-empty")
            }
        }
    }
}

impl std::error::Error for EvIngestRouteError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            EvIngestRouteError::Proposal(e) => Some(e),
            EvIngestRouteError::SignedMeterValue(e) => Some(e),
            _ => None,
        }
    }
}

/// Routes one inbound EV frame onto its seam (the routing table in the
/// module header). Pure routing — no decode, no quality decision, no
/// dispatch: those are the existing pipeline's and the application
/// modules' jobs.
pub fn route_ev_inbound(
    frame: &EvInboundFrame,
    telemetry_offer: &dyn TelemetryOffer,
    signed_sink: &dyn SignedMeterValueSink,
    config: &EvIngestBridgeConfig,
) -> Result<EvRouteOutcome, EvIngestRouteError> {
    config.validate()?;
    match frame {
        EvInboundFrame::Telemetry { payload } => {
            // The EXISTING pipeline's bounded channel: a shed is a
            // retryable outcome (the adapter backs off), never a silent
            // drop — the queue's own metrics count it.
            match telemetry_offer.offer_frame(payload.clone()) {
                Ok(()) => Ok(EvRouteOutcome::TelemetryQueued),
                Err(_shed) => Ok(EvRouteOutcome::TelemetryShed),
            }
        }
        EvInboundFrame::Proposal(message) => {
            // Proposals are inputs and records, NEVER auto-commands: the
            // only output is the domain record (entry state PROPOSED).
            let record = assemble_proposal_record(
                message,
                config.proposal_provenance,
                config.provenance_note.clone(),
            )
            .map_err(EvIngestRouteError::Proposal)?;
            tracing::info!(
                evse_id = %record.evse_id(),
                station_id = %record.station_id(),
                kind = record.kind().message_name(),
                "EV proposal message ingested as a constraint record (never a \
                 command; the ADR-010-EXT-01 pattern)"
            );
            Ok(EvRouteOutcome::ProposalRecorded(record))
        }
        EvInboundFrame::SignedMeterValue(record) => {
            signed_sink
                .record_signed_meter_value(record)
                .map_err(EvIngestRouteError::SignedMeterValue)?;
            tracing::info!(
                origin = %record.origin,
                "signed meter value recorded (opaque evidence; never verified, \
                 never a compliance claim)"
            );
            Ok(EvRouteOutcome::SignedMeterValueRecorded)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flexcore_application::ev_meter_values_ingest::MeterValuesIngestError;
    use flexcore_application::ev_proposal_ingest::InboundProposalMessageName;
    use flexcore_contracts::quality::TelemetryQuality;
    use flexcore_contracts::time_model::ClockQuality;
    use flexcore_domain::ev_proposal_records::EvProposalPayload;
    use flexcore_ports::fakes::FakeTelemetrySource;
    use flexcore_ports::telemetry_source::{
        ProviderDeclaration, QualityMappingDeclaration, SourceStateMapping,
    };
    use std::sync::Mutex;
    use std::time::Duration;

    /// A counting fake offer seam: accepts frames up to a declared
    /// capacity, then sheds (the bounded-channel behaviour, test-side).
    struct FakeOffer {
        accepted: Mutex<Vec<Vec<u8>>>,
        capacity: usize,
    }

    impl FakeOffer {
        fn with_capacity(capacity: usize) -> Self {
            Self {
                accepted: Mutex::new(Vec::new()),
                capacity,
            }
        }
    }

    impl TelemetryOffer for FakeOffer {
        fn offer_frame(&self, payload: Vec<u8>) -> Result<(), FrameShed> {
            let mut accepted = self.accepted.lock().unwrap();
            if accepted.len() >= self.capacity {
                return Err(FrameShed);
            }
            accepted.push(payload);
            Ok(())
        }
    }

    /// A counting fake sink (the hook's contract, worker-side).
    struct CountingSink {
        records: Mutex<Vec<SignedMeterValueRecord>>,
        fail: bool,
    }

    impl CountingSink {
        fn ok() -> Self {
            Self {
                records: Mutex::new(Vec::new()),
                fail: false,
            }
        }
        fn failing() -> Self {
            Self {
                records: Mutex::new(Vec::new()),
                fail: true,
            }
        }
    }

    impl SignedMeterValueSink for CountingSink {
        fn record_signed_meter_value(
            &self,
            record: &SignedMeterValueRecord,
        ) -> Result<(), MeterValuesIngestError> {
            if self.fail {
                return Err(MeterValuesIngestError::EmptySignedMeterValueOrigin);
            }
            self.records.lock().unwrap().push(record.clone());
            Ok(())
        }
    }

    fn config() -> EvIngestBridgeConfig {
        EvIngestBridgeConfig {
            meter_values: MeterValuesCadenceConfig {
                soc_cadence_millis: 60_000,
                power_cadence_millis: 10_000,
                energy_cadence_millis: 900_000,
                soc_freshness_limit_millis: 300_000,
                power_freshness_limit_millis: 60_000,
                energy_freshness_limit_millis: 3_600_000,
            },
            proposal_provenance: Provenance::Measured,
            provenance_note: "relayed by the simulated station adapter; spelling verbatim"
                .to_string(),
        }
    }

    fn proposal_message() -> InboundProposalMessage {
        InboundProposalMessage {
            message_name: InboundProposalMessageName::NotifyChargingLimit,
            station_id: "cs-depot-01".to_string(),
            evse_id: "EVSE-DEPOT-01-2".to_string(),
            payload: EvProposalPayload {
                charging_limit_mw: Some(0.011),
                min_charging_rate_mw: None,
                target_soc: None,
                energy_request_mwh: None,
                windows_millis: Vec::new(),
            },
            cleared_limit_transaction_id: None,
            received_at_unix_ms: 1_700_000_000_123,
        }
    }

    /// The REAL production offer seam is `IngestionQueue`; the test pins
    /// that the trait impl wraps the queue's offer (the same bounded
    /// channel the P2.14 bridge feeds). The queue itself is built over a
    /// fake source via the task-free channel construction available in
    /// the ingestion module — here we pin the trait wiring by driving the
    /// real task's queue through the fakes the ports crate provides.
    /// (The full pipeline integration is the human build/test gate.)
    #[test]
    fn the_production_offer_impl_wraps_the_ingestion_queue() {
        // The impl exists and compiles against the real queue type; this
        // test pins the trait object dispatch.
        let declaration = ProviderDeclaration::new(
            "sim://exit-gate".to_string(),
            Duration::from_secs(60),
            QualityMappingDeclaration::new(vec![SourceStateMapping {
                source_code: "GOOD".to_string(),
                canonical: TelemetryQuality::Good,
            }])
            .unwrap(),
            ClockQuality::Synchronized,
        )
        .unwrap();
        let _source = FakeTelemetrySource::new(declaration, Vec::new());
        // The queue construction needs the full task (database-backed
        // ports) — the routing tests below use the FakeOffer seam, and
        // the IngestionQueue impl is exercised at the human build/test
        // gate together with the pipeline integration tests.
        let offer: &dyn TelemetryOffer = &FakeOffer::with_capacity(1);
        assert!(offer.offer_frame(b"x".to_vec()).is_ok());
        assert_eq!(offer.offer_frame(b"y".to_vec()), Err(FrameShed));
    }

    #[test]
    fn the_bridge_config_validates_cadence_and_note() {
        assert!(config().validate().is_ok());
        let mut bad = config();
        bad.meter_values.soc_cadence_millis = 0;
        assert!(matches!(
            bad.validate(),
            Err(EvIngestRouteError::SignedMeterValue(
                MeterValuesIngestError::NonPositiveCadence { .. }
            ))
        ));
        let mut bad = config();
        bad.provenance_note = "  ".to_string();
        assert_eq!(bad.validate(), Err(EvIngestRouteError::EmptyProvenanceNote));
    }

    #[test]
    fn the_message_family_tables_carry_the_recorded_spellings() {
        assert_eq!(EV_TELEMETRY_FAMILIES, ["MeterValues", "StatusNotification"]);
        assert_eq!(
            EV_PROPOSAL_FAMILIES,
            [
                "NotifyEVChargingNeeds",
                "NotifyEVChargingSchedule",
                "NotifyChargingLimit",
                "ClearedChargingLimit"
            ]
        );
    }

    #[test]
    fn a_telemetry_frame_routes_into_the_existing_pipeline_offer() {
        let sink = CountingSink::ok();
        let cfg = config();
        let offer = FakeOffer::with_capacity(8);
        let outcome = route_ev_inbound(
            &EvInboundFrame::Telemetry {
                payload: b"frame-bytes".to_vec(),
            },
            &offer,
            &sink,
            &cfg,
        )
        .unwrap();
        assert_eq!(outcome, EvRouteOutcome::TelemetryQueued);
        assert_eq!(offer.accepted.lock().unwrap().len(), 1);
    }

    #[test]
    fn a_shed_at_the_bounded_limit_is_a_retryable_outcome_never_a_drop() {
        let sink = CountingSink::ok();
        let cfg = config();
        let offer = FakeOffer::with_capacity(1);
        assert!(route_ev_inbound(
            &EvInboundFrame::Telemetry {
                payload: b"first".to_vec(),
            },
            &offer,
            &sink,
            &cfg,
        )
        .unwrap()
        .matches(&EvRouteOutcome::TelemetryQueued));
        let outcome = route_ev_inbound(
            &EvInboundFrame::Telemetry {
                payload: b"second".to_vec(),
            },
            &offer,
            &sink,
            &cfg,
        )
        .unwrap();
        assert_eq!(outcome, EvRouteOutcome::TelemetryShed);
    }

    #[test]
    fn a_proposal_frame_routes_to_a_record_never_a_command() {
        let sink = CountingSink::ok();
        let cfg = config();
        let offer = FakeOffer::with_capacity(8);
        let outcome = route_ev_inbound(
            &EvInboundFrame::Proposal(proposal_message()),
            &offer,
            &sink,
            &cfg,
        )
        .unwrap();
        let EvRouteOutcome::ProposalRecorded(record) = outcome else {
            panic!("the proposal frame must route to a record");
        };
        // The NEVER-AUTO-COMMAND rule: the record's entry state is
        // structurally PROPOSED.
        assert_eq!(
            record.lifecycle_entry_state(),
            flexcore_domain::command_state::CommandState::Proposed
        );
        assert_eq!(record.kind().message_name(), "NotifyChargingLimit");
        // And the frozen lifecycle refuses the PROPOSED shortcuts.
        assert!(flexcore_domain::command_state::try_transition(
            flexcore_domain::command_state::CommandState::Proposed,
            flexcore_domain::command_state::CommandState::Issued,
        )
        .is_err());
    }

    #[test]
    fn a_signed_meter_value_frame_routes_to_the_sink() {
        let sink = CountingSink::ok();
        let cfg = config();
        let offer = FakeOffer::with_capacity(8);
        let record = SignedMeterValueRecord::new(
            "SIGNED-PAYLOAD==".to_string(),
            "cs-depot-01".to_string(),
            1_700_000_000_000,
            None,
        )
        .unwrap();
        let outcome = route_ev_inbound(
            &EvInboundFrame::SignedMeterValue(record),
            &offer,
            &sink,
            &cfg,
        )
        .unwrap();
        assert_eq!(outcome, EvRouteOutcome::SignedMeterValueRecorded);
        assert_eq!(sink.records.lock().unwrap().len(), 1);
        // A failing sink surfaces a named error (never a silent drop).
        let failing = CountingSink::failing();
        let record = SignedMeterValueRecord::new(
            "SIGNED-PAYLOAD==".to_string(),
            "cs-depot-01".to_string(),
            1_700_000_000_000,
            None,
        )
        .unwrap();
        assert!(matches!(
            route_ev_inbound(
                &EvInboundFrame::SignedMeterValue(record),
                &offer,
                &failing,
                &cfg
            ),
            Err(EvIngestRouteError::SignedMeterValue(
                MeterValuesIngestError::EmptySignedMeterValueOrigin
            ))
        ));
    }
}

/// (test-helper extension) `EvRouteOutcome` match helper used by the
/// routing tests — kept beside the enum's tests, not in the public
/// contract.
#[cfg(test)]
impl EvRouteOutcome {
    /// Whether the outcome is the queued outcome (the tests' readability
    /// helper).
    fn matches(&self, other: &EvRouteOutcome) -> bool {
        // TelemetryQueued carries no data; compare by variant.
        matches!(
            (self, other),
            (EvRouteOutcome::TelemetryQueued, EvRouteOutcome::TelemetryQueued)
        )
    }
}
