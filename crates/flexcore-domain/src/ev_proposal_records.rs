// P17.06 — Vehicle-proposal constraint records (pure rules, no I/O).
//
// Implements ledger P17.06 ("Vehicle-proposal ingestion"): the domain
// vocabulary for the constraint RECORDS that vehicle- and station-originated
// charging proposals become. The proposal messages are relayed to the
// platform by the OCPP CSMS adapter (Phase 17 P17.01, Agent A's
// `crates/adapters/protocol-ocpp/` inbound path — consumed here BY CITATION;
// this domain module depends on nothing infrastructure).
//
// # THE GOVERNING RULE (ADR-010, extended): proposals are inputs and
// records, NEVER auto-commands
//
// ADR-010 decides that outputs of non-platform components (ML/RL, research
// plugins) enter the platform only as proposals and must pass the protected
// validation, feasibility, and authorization gates. ADR-010-EXT-01
// (`docs/architecture/adr/ADR-010-EXT-01-protocol-adapter-authority.md`)
// extends the SAME decision to protocol adapters: external control objects
// enter the command lifecycle at PROPOSED, an adapter that constructs an
// APPROVED or ISSUED state from external input is a defect, and relay is not
// authority. This module records the extension of that pattern to the EV
// surfaces (the extension statement is recorded in the Phase 17 decision
// record `docs/phases-11-20/phase-17-ev-fleet-charging/records/
// 21_p17_06_vehicle_proposal_ingestion.md`, referencing the ADR-010-EXT-01
// pattern; ADR-010 itself and the EXT-01 file are UNTOUCHED):
//
// - A `NotifyEVChargingNeeds` / `NotifyEVChargingSchedule` message (ISO 15118
//   proposals relayed by the station) and a `NotifyChargingLimit` /
//   `ClearedChargingLimit` message (locally imposed limits) are CONSTRAINT
//   RECORDS. None of them is a command; none of them enters the lifecycle
//   anywhere except at PROPOSED; and no function in this module produces,
//   mutates, or returns a lifecycle state other than PROPOSED.
// - The station's proposal may INFORM a later VPP candidate (the P17.05
//   composer consumes constraints by citation); the proposal itself never
//   becomes a `SetChargingProfile` payload without passing the frozen
//   14-state lifecycle's three distinct gates.
//
// # Message-name honesty
//
// The OCPP message names carried by the closed [`EvProposalKind`]
// vocabulary are SECONDARY-SOURCED from [S-124] (OCPP 2.0.1 smart charging)
// and [S-125] (OCPP 2.1) and are UNVERIFIED against the OCA specification
// text — the implementation-time check is recorded in the decision record.
// A confirmed spelling mismatch is a recorded edit of this vocabulary plus
// the record, never a silent reinterpretation (plan §7 note 5).
//
// # Cleared limits are audit records, never silent deletions
//
// A `ClearedChargingLimit` does not silently erase the constraint it
// clears: it lands as its own record kind ([`EvProposalKind::
// ClearedChargingLimit`]) naming the limit transaction it clears. The
// consumer (composer, scheduler) resolves the constraint set; this module
// never deletes history (the audit discipline of the command lifecycle
// applies: SUPERSEDED is retained, not removed).
//
// Control status: pure domain records for the simulation-only 0.x line.
// No I/O, no adapter coupling, no device-write path, no dispatch authority.

use crate::command_state::CommandState;
use flexcore_contracts::capability::Provenance;

/// The recorded ADR-010 extension statement for vehicle proposals (the
/// EXT-01 pattern applied to the EV surfaces; decided once).
pub const PROPOSALS_NEVER_AUTO_COMMAND_STATEMENT: &str =
    "Vehicle-proposal messages (NotifyEVChargingNeeds, \
NotifyEVChargingSchedule, NotifyChargingLimit, ClearedChargingLimit) are \
inputs and constraint RECORDS, never auto-commands. They enter the command \
lifecycle only at PROPOSED and reach a device only through the frozen \
14-state lifecycle's three distinct gates (validation, feasibility, \
authorization). This extends ADR-010-EXT-01 (protocol adapters are \
supervisory transports) to the EV proposal surfaces; an adapter or \
ingestion path that constructs APPROVED or ISSUED from a proposal is a \
defect, not a shortcut.";

/// The closed proposal-kind vocabulary. Each variant names the OCPP message
/// family it records (spellings SECONDARY-SOURCED [S-124][S-125],
/// UNVERIFIED against the OCA text).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EvProposalKind {
    /// `NotifyEVChargingNeeds` — the vehicle's charging needs (energy
    /// amounts, target SOC, time windows) relayed by the station (ISO 15118
    /// proposals relayed by the CSMS; OCPP 2.1 message family [S-125],
    /// UNVERIFIED spelling).
    ChargingNeeds,
    /// `NotifyEVChargingSchedule` — a charging schedule proposal the vehicle
    /// (via the station) asked to be considered (OCPP 2.1 message family
    /// [S-125], UNVERIFIED spelling).
    ChargingScheduleProposal,
    /// `NotifyChargingLimit` — a charging limit the station imposed
    /// LOCALLY (an active charging-profile constraint the CSMS must know
    /// about, OCPP 2.0.1 smart-charging family [S-124], UNVERIFIED
    /// spelling).
    ChargingLimit,
    /// `ClearedChargingLimit` — the station's notice that a previously
    /// imposed charging limit no longer applies. An AUDIT RECORD: the
    /// cleared constraint is retained in history (never silently deleted).
    ClearedChargingLimit,
}

impl EvProposalKind {
    /// All proposal kinds, in frozen vocabulary order.
    pub const ALL: [EvProposalKind; 4] = [
        EvProposalKind::ChargingNeeds,
        EvProposalKind::ChargingScheduleProposal,
        EvProposalKind::ChargingLimit,
        EvProposalKind::ClearedChargingLimit,
    ];

    /// The canonical OCPP message-name spelling this kind records
    /// (SECONDARY-SOURCED [S-124][S-125]; UNVERIFIED against the OCA text).
    pub const fn message_name(self) -> &'static str {
        match self {
            EvProposalKind::ChargingNeeds => "NotifyEVChargingNeeds",
            EvProposalKind::ChargingScheduleProposal => "NotifyEVChargingSchedule",
            EvProposalKind::ChargingLimit => "NotifyChargingLimit",
            EvProposalKind::ClearedChargingLimit => "ClearedChargingLimit",
        }
    }
}

/// Where the proposal originated. The CSMS adapter relays both shapes; the
/// origin is declared data, never inferred from the payload shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EvProposalSource {
    /// An ISO 15118 vehicle proposal relayed by the station (the station is
    /// the transport; the vehicle is the origin).
    StationRelayed15118,
    /// A limit the station imposed locally (station-internal policy, e.g. a
    /// local load-management limit it is already enforcing).
    StationLocalLimit,
}

impl EvProposalSource {
    /// All proposal sources, in frozen vocabulary order.
    pub const ALL: [EvProposalSource; 2] = [
        EvProposalSource::StationRelayed15118,
        EvProposalSource::StationLocalLimit,
    ];
}

/// Errors returned by proposal-record construction. Every variant names the
/// offending field (the P3.04 named-error style).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvProposalError {
    /// The EVSE identifier was empty or whitespace-only. The adapter's
    /// spelling is recorded verbatim (never parsed or normalized here).
    EmptyEvseId,
    /// The EVSE identifier exceeded the recorded length bound (256).
    EvseIdTooLong {
        /// The offending length.
        length: usize,
    },
    /// The station identifier was empty or whitespace-only.
    EmptyStationId,
    /// The charging-power limit was negative or non-finite. Limits are
    /// non-negative magnitudes in canonical MW; a zero limit is meaningful
    /// (a full curtailment the station imposed).
    InvalidChargingLimitMw {
        /// The offending value.
        value: f64,
    },
    /// A minimum charging rate was declared negative or non-finite (the
    /// minChargingRate honesty rule: a schedule step below it must be 0 —
    /// [S-124], UNVERIFIED transcription).
    InvalidMinChargingRateMw {
        /// The offending value.
        value: f64,
    },
    /// A schedule-proposal window was inverted (start >= end).
    InvertedProposalWindow {
        /// The offending window index.
        window_index: usize,
        /// Window start in UTC milliseconds.
        start_millis: i64,
        /// Window end in UTC milliseconds.
        end_millis: i64,
    },
    /// A cleared-limit record was constructed without the transaction id of
    /// the limit it clears — a clearance that names nothing is not an
    /// audit record.
    ClearedWithoutLimitTransactionId,
    /// The provenance note was empty (the human-safe provenance-note rule;
    /// never raw wire text — the P1.08 rule).
    EmptyProvenanceNote,
    /// A target SOC was outside [0,1] (boundary-inclusive).
    SocTargetOutOfRange {
        /// The offending value.
        value: f64,
    },
    /// An energy request was negative or non-finite.
    InvalidEnergyRequest {
        /// The offending value.
        value: f64,
    },
    /// A UTC timestamp in milliseconds was outside the representable range.
    TimestampOutOfRange {
        /// The offending timestamp.
        value: i64,
    },
}

impl std::fmt::Display for EvProposalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EvProposalError::EmptyEvseId => write!(f, "the EVSE id is mandatory and must be non-empty"),
            EvProposalError::EvseIdTooLong { length } => {
                write!(f, "the EVSE id exceeds the 256-character bound (found {length})")
            }
            EvProposalError::EmptyStationId => {
                write!(f, "the station id is mandatory and must be non-empty")
            }
            EvProposalError::InvalidChargingLimitMw { value } => {
                write!(f, "the charging limit must be a finite non-negative MW magnitude (found {value})")
            }
            EvProposalError::InvalidMinChargingRateMw { value } => {
                write!(f, "the minimum charging rate must be a finite non-negative MW magnitude (found {value})")
            }
            EvProposalError::InvertedProposalWindow { window_index, start_millis, end_millis } => {
                write!(
                    f,
                    "proposal window {window_index} is inverted (start {start_millis} >= end {end_millis} UTC ms)"
                )
            }
            EvProposalError::ClearedWithoutLimitTransactionId => {
                write!(f, "a cleared-limit record must name the limit transaction id it clears")
            }
            EvProposalError::EmptyProvenanceNote => {
                write!(f, "the provenance note is mandatory and must be non-empty")
            }
            EvProposalError::TimestampOutOfRange { value } => {
                write!(
                    f,
                    "the timestamp {value} is outside the record's declared UTC bound \
                     (year 1 through 9999 in UTC milliseconds)"
                )
            }
            EvProposalError::SocTargetOutOfRange { value } => {
                write!(f, "the declared target SOC must be a fraction in [0,1] (found {value})")
            }
            EvProposalError::InvalidEnergyRequest { value } => {
                write!(f, "the declared energy request must be a finite non-negative MWh magnitude (found {value})")
            }
        }
    }
}

impl std::error::Error for EvProposalError {}

/// One declared charging-window of a schedule proposal (start-inclusive,
/// end-exclusive — the §10.3 interval semantics the domain uses everywhere).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EvProposalWindow {
    /// Window start in UTC milliseconds (start-inclusive).
    pub start_millis: i64,
    /// Window end in UTC milliseconds (end-exclusive).
    pub end_millis: i64,
}

/// The declared payload of a vehicle-proposal record — the fields the
/// proposal carries, as DECLARED DATA. Nothing here is validated against
/// the vehicle (the vehicle is not connected to the platform; the station
/// relayed the payload and the adapter decoded it). Every magnitude is in
/// canonical units (MW, the AGENTS.md §4 rule); a SOC-like target travels
/// as a fraction in [0,1].
#[derive(Debug, Clone, PartialEq)]
pub struct EvProposalPayload {
    /// The charging-power limit the proposal declares, in MW
    /// (non-negative; zero = a full curtailment the station imposed).
    /// `None` when the proposal declares no limit (e.g. a pure needs
    /// notification without a limit field).
    pub charging_limit_mw: Option<f64>,
    /// The minimum charging rate the proposal declares, in MW (`None` =
    /// not declared; the minChargingRate rule [S-124] is the composer's
    /// concern — recorded here, never enforced here).
    pub min_charging_rate_mw: Option<f64>,
    /// The target SOC the vehicle asked for, a fraction in [0,1]
    /// (`None` = not declared).
    pub target_soc: Option<f64>,
    /// The energy the vehicle asked for, in MWh (`None` = not declared).
    pub energy_request_mwh: Option<f64>,
    /// The declared charging windows of a schedule proposal (start-
    /// inclusive, end-exclusive; empty = no windows declared).
    pub windows_millis: Vec<EvProposalWindow>,
}

impl EvProposalPayload {
    /// Validates the declared payload: non-negative magnitudes, in-range
    /// SOC target, non-inverted windows.
    pub fn validate(&self) -> Result<(), EvProposalError> {
        if let Some(limit) = self.charging_limit_mw {
            if !limit.is_finite() || limit < 0.0 {
                return Err(EvProposalError::InvalidChargingLimitMw { value: limit });
            }
        }
        if let Some(min_rate) = self.min_charging_rate_mw {
            if !min_rate.is_finite() || min_rate < 0.0 {
                return Err(EvProposalError::InvalidMinChargingRateMw { value: min_rate });
            }
        }
        if let Some(soc) = self.target_soc {
            if !soc.is_finite() || soc < 0.0 || soc > 1.0 {
                return Err(EvProposalError::SocTargetOutOfRange { value: soc });
            }
        }
        if let Some(energy) = self.energy_request_mwh {
            if !energy.is_finite() || energy < 0.0 {
                return Err(EvProposalError::InvalidEnergyRequest { value: energy });
            }
        }
        for (index, window) in self.windows_millis.iter().enumerate() {
            if window.start_millis >= window.end_millis {
                return Err(EvProposalError::InvertedProposalWindow {
                    window_index: index,
                    start_millis: window.start_millis,
                    end_millis: window.end_millis,
                });
            }
        }
        Ok(())
    }
}

/// The vehicle-proposal CONSTRAINT RECORD (P17.06). This is the only thing
/// a proposal message can become: a record with provenance, a declared
/// payload, and a lifecycle entry state that is structurally pinned to
/// PROPOSED. There is no field, constructor, accessor, or conversion in
/// this type that yields any other lifecycle state — the type cannot
/// express an auto-command.
#[derive(Debug, Clone, PartialEq)]
pub struct EvProposalRecord {
    evse_id: String,
    station_id: String,
    kind: EvProposalKind,
    source: EvProposalSource,
    payload: EvProposalPayload,
    /// The station's limit-transaction id this record clears (only for
    /// [`EvProposalKind::ClearedChargingLimit`]; validated non-empty at
    /// construction for that kind).
    cleared_limit_transaction_id: Option<String>,
    received_at_unix_ms: i64,
    provenance: Provenance,
    provenance_note: String,
}

impl EvProposalRecord {
    /// Constructs and validates a proposal record. The EVSE id and station
    /// id are the adapter's verbatim spellings (never parsed here); the
    /// payload is validated; a cleared-limit record must name the limit
    /// transaction it clears; the provenance note is mandatory and
    /// human-safe.
    pub fn new(
        evse_id: String,
        station_id: String,
        kind: EvProposalKind,
        source: EvProposalSource,
        payload: EvProposalPayload,
        cleared_limit_transaction_id: Option<String>,
        received_at_unix_ms: i64,
        provenance: Provenance,
        provenance_note: String,
    ) -> Result<Self, EvProposalError> {
        let trimmed_evse = evse_id.trim();
        if trimmed_evse.is_empty() {
            return Err(EvProposalError::EmptyEvseId);
        }
        if trimmed_evse.len() > 256 {
            return Err(EvProposalError::EvseIdTooLong {
                length: trimmed_evse.len(),
            });
        }
        if station_id.trim().is_empty() {
            return Err(EvProposalError::EmptyStationId);
        }
        if provenance_note.trim().is_empty() {
            return Err(EvProposalError::EmptyProvenanceNote);
        }
        // UTC-ms representability (the contracts time-model bound; checked
        // here because the record carries raw declared timestamps, the
        // EV-model style — no EventTime construction happens in this
        // module).
        if received_at_unix_ms < -62_167_219_200_000
            || received_at_unix_ms > 253_402_300_799_999
        {
            return Err(EvProposalError::TimestampOutOfRange {
                value: received_at_unix_ms,
            });
        }
        payload.validate()?;
        let cleared = match kind {
            EvProposalKind::ClearedChargingLimit => {
                let id = cleared_limit_transaction_id.clone().unwrap_or_default();
                if id.trim().is_empty() {
                    return Err(EvProposalError::ClearedWithoutLimitTransactionId);
                }
                Some(id)
            }
            _ => None,
        };
        Ok(Self {
            evse_id: trimmed_evse.to_string(),
            station_id: station_id.trim().to_string(),
            kind,
            source,
            payload,
            cleared_limit_transaction_id: cleared,
            received_at_unix_ms,
            provenance,
            provenance_note: provenance_note.trim().to_string(),
        })
    }

    /// The EVSE id (the adapter's verbatim spelling).
    pub fn evse_id(&self) -> &str {
        &self.evse_id
    }

    /// The station id (the adapter's verbatim spelling).
    pub fn station_id(&self) -> &str {
        &self.station_id
    }

    /// The proposal kind.
    pub fn kind(&self) -> EvProposalKind {
        self.kind
    }

    /// The declared origin.
    pub fn source(&self) -> EvProposalSource {
        self.source
    }

    /// The declared payload.
    pub fn payload(&self) -> &EvProposalPayload {
        &self.payload
    }

    /// The limit-transaction id this record clears (cleared-limit records
    /// only).
    pub fn cleared_limit_transaction_id(&self) -> Option<&str> {
        self.cleared_limit_transaction_id.as_deref()
    }

    /// When the platform received the proposal (UTC ms, declared data).
    pub fn received_at_unix_ms(&self) -> i64 {
        self.received_at_unix_ms
    }

    /// The provenance of the record.
    pub fn provenance(&self) -> Provenance {
        self.provenance
    }

    /// The human-safe provenance note.
    pub fn provenance_note(&self) -> &str {
        &self.provenance_note
    }

    /// THE LIFECYCLE ENTRY STATE — structurally pinned to PROPOSED.
    ///
    /// This is the code form of the ADR-010-EXT-01 pattern for proposals:
    /// a proposal enters the command lifecycle at PROPOSED, nowhere else.
    /// There is no parameter, no variant, and no way for a caller to
    /// obtain APPROVED or ISSUED from a proposal record; the only
    /// lifecycle movement happens in the protected lifecycle engine
    /// (`command_lifecycle`), behind its three distinct gates.
    pub const fn lifecycle_entry_state() -> CommandState {
        CommandState::Proposed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn payload() -> EvProposalPayload {
        EvProposalPayload {
            charging_limit_mw: Some(0.011),
            min_charging_rate_mw: Some(0.0042),
            target_soc: Some(0.8),
            energy_request_mwh: Some(0.03),
            windows_millis: vec![EvProposalWindow {
                start_millis: 1_700_000_000_000,
                end_millis: 1_700_003_600_000,
            }],
        }
    }

    fn record() -> EvProposalRecord {
        EvProposalRecord::new(
            "EVSE-DEPOT-01-2".to_string(),
            "cs-depot-01".to_string(),
            EvProposalKind::ChargingLimit,
            EvProposalSource::StationLocalLimit,
            payload(),
            None,
            1_700_000_000_123,
            Provenance::Measured,
            "relayed by the simulated station adapter; spelling verbatim"
                .to_string(),
        )
        .unwrap()
    }

    #[test]
    fn the_kind_vocabulary_is_closed_and_names_the_message_families() {
        assert_eq!(EvProposalKind::ALL.len(), 4);
        assert_eq!(
            EvProposalKind::ChargingNeeds.message_name(),
            "NotifyEVChargingNeeds"
        );
        assert_eq!(
            EvProposalKind::ChargingScheduleProposal.message_name(),
            "NotifyEVChargingSchedule"
        );
        assert_eq!(
            EvProposalKind::ChargingLimit.message_name(),
            "NotifyChargingLimit"
        );
        assert_eq!(
            EvProposalKind::ClearedChargingLimit.message_name(),
            "ClearedChargingLimit"
        );
    }

    #[test]
    fn the_lifecycle_entry_state_is_structurally_proposed() {
        // The ADR-010-EXT-01 pattern for proposals: the entry state is a
        // constant, not a parameter. No call shape exists that could make
        // a proposal record enter the lifecycle anywhere else.
        assert_eq!(
            EvProposalRecord::lifecycle_entry_state(),
            CommandState::Proposed
        );
        // And the frozen lifecycle itself refuses any PROPOSED shortcut to
        // an issued state (the oracle the record relies on):
        assert!(crate::command_state::try_transition(
            CommandState::Proposed,
            CommandState::Issued,
        )
        .is_err());
    }

    #[test]
    fn the_statement_carries_the_never_auto_command_rule() {
        assert!(PROPOSALS_NEVER_AUTO_COMMAND_STATEMENT.contains("never auto-commands"));
        assert!(PROPOSALS_NEVER_AUTO_COMMAND_STATEMENT.contains("three distinct gates"));
        assert!(PROPOSALS_NEVER_AUTO_COMMAND_STATEMENT.contains("ADR-010-EXT-01"));
    }

    #[test]
    fn a_valid_record_round_trips() {
        let record = record();
        assert_eq!(record.evse_id(), "EVSE-DEPOT-01-2");
        assert_eq!(record.station_id(), "cs-depot-01");
        assert_eq!(record.kind(), EvProposalKind::ChargingLimit);
        assert_eq!(record.source(), EvProposalSource::StationLocalLimit);
        assert_eq!(record.payload().charging_limit_mw, Some(0.011));
        assert_eq!(record.cleared_limit_transaction_id(), None);
        assert_eq!(record.received_at_unix_ms(), 1_700_000_000_123);
        assert_eq!(record.provenance(), Provenance::Measured);
    }

    #[test]
    fn construction_refuses_empty_ids_notes_and_bad_magnitudes() {
        // Empty EVSE id.
        assert_eq!(
            EvProposalRecord::new(
                "   ".to_string(),
                "cs".to_string(),
                EvProposalKind::ChargingNeeds,
                EvProposalSource::StationRelayed15118,
                payload(),
                None,
                0,
                Provenance::Measured,
                "note".to_string(),
            ),
            Err(EvProposalError::EmptyEvseId)
        );
        // Empty station id.
        assert_eq!(
            EvProposalRecord::new(
                "EVSE-1".to_string(),
                String::new(),
                EvProposalKind::ChargingNeeds,
                EvProposalSource::StationRelayed15118,
                payload(),
                None,
                0,
                Provenance::Measured,
                "note".to_string(),
            ),
            Err(EvProposalError::EmptyStationId)
        );
        // Empty provenance note (the human-safe note rule).
        assert_eq!(
            EvProposalRecord::new(
                "EVSE-1".to_string(),
                "cs".to_string(),
                EvProposalKind::ChargingNeeds,
                EvProposalSource::StationRelayed15118,
                payload(),
                None,
                0,
                Provenance::Measured,
                "  ".to_string(),
            ),
            Err(EvProposalError::EmptyProvenanceNote)
        );
        // A negative charging limit.
        let mut bad = payload();
        bad.charging_limit_mw = Some(-0.5);
        assert!(EvProposalRecord::new(
            "EVSE-1".to_string(),
            "cs".to_string(),
            EvProposalKind::ChargingLimit,
            EvProposalSource::StationLocalLimit,
            bad,
            None,
            0,
            Provenance::Measured,
            "note".to_string(),
        )
        .is_err());
        // A non-finite charging limit.
        let mut bad = payload();
        bad.charging_limit_mw = Some(f64::NAN);
        assert!(EvProposalRecord::new(
            "EVSE-1".to_string(),
            "cs".to_string(),
            EvProposalKind::ChargingLimit,
            EvProposalSource::StationLocalLimit,
            bad,
            None,
            0,
            Provenance::Measured,
            "note".to_string(),
        )
        .is_err());
        // An out-of-range SOC target.
        let mut bad = payload();
        bad.target_soc = Some(1.5);
        assert!(EvProposalRecord::new(
            "EVSE-1".to_string(),
            "cs".to_string(),
            EvProposalKind::ChargingNeeds,
            EvProposalSource::StationRelayed15118,
            bad,
            None,
            0,
            Provenance::Measured,
            "note".to_string(),
        )
        .is_err());
        // An inverted proposal window.
        let mut bad = payload();
        bad.windows_millis = vec![EvProposalWindow {
            start_millis: 2_000,
            end_millis: 1_000,
        }];
        assert!(EvProposalRecord::new(
            "EVSE-1".to_string(),
            "cs".to_string(),
            EvProposalKind::ChargingScheduleProposal,
            EvProposalSource::StationRelayed15118,
            bad,
            None,
            0,
            Provenance::Measured,
            "note".to_string(),
        )
        .is_err());
        // A timestamp outside the representable UTC range.
        assert_eq!(
            EvProposalRecord::new(
                "EVSE-1".to_string(),
                "cs".to_string(),
                EvProposalKind::ChargingNeeds,
                EvProposalSource::StationRelayed15118,
                payload(),
                None,
                i64::MAX,
                Provenance::Measured,
                "note".to_string(),
            ),
            Err(EvProposalError::TimestampOutOfRange { value: i64::MAX })
        );
    }

    #[test]
    fn a_zero_charging_limit_is_a_meaningful_full_curtailment() {
        // Zero is a valid limit (the station imposed a full curtailment),
        // never a default.
        let mut zero = payload();
        zero.charging_limit_mw = Some(0.0);
        let record = EvProposalRecord::new(
            "EVSE-1".to_string(),
            "cs".to_string(),
            EvProposalKind::ChargingLimit,
            EvProposalSource::StationLocalLimit,
            zero,
            None,
            0,
            Provenance::Measured,
            "note".to_string(),
        )
        .unwrap();
        assert_eq!(record.payload().charging_limit_mw, Some(0.0));
    }

    #[test]
    fn a_cleared_limit_is_an_audit_record_that_names_what_it_clears() {
        // A cleared-limit record MUST carry the limit transaction id.
        assert_eq!(
            EvProposalRecord::new(
                "EVSE-1".to_string(),
                "cs".to_string(),
                EvProposalKind::ClearedChargingLimit,
                EvProposalSource::StationLocalLimit,
                EvProposalPayload {
                    charging_limit_mw: None,
                    min_charging_rate_mw: None,
                    target_soc: None,
                    energy_request_mwh: None,
                    windows_millis: Vec::new(),
                },
                None,
                0,
                Provenance::Measured,
                "note".to_string(),
            ),
            Err(EvProposalError::ClearedWithoutLimitTransactionId)
        );
        // A blank transaction id is also refused.
        assert_eq!(
            EvProposalRecord::new(
                "EVSE-1".to_string(),
                "cs".to_string(),
                EvProposalKind::ClearedChargingLimit,
                EvProposalSource::StationLocalLimit,
                EvProposalPayload {
                    charging_limit_mw: None,
                    min_charging_rate_mw: None,
                    target_soc: None,
                    energy_request_mwh: None,
                    windows_millis: Vec::new(),
                },
                Some("  ".to_string()),
                0,
                Provenance::Measured,
                "note".to_string(),
            ),
            Err(EvProposalError::ClearedWithoutLimitTransactionId)
        );
        // A valid clearance carries the id and keeps the kind.
        let cleared = EvProposalRecord::new(
            "EVSE-1".to_string(),
            "cs".to_string(),
            EvProposalKind::ClearedChargingLimit,
            EvProposalSource::StationLocalLimit,
            EvProposalPayload {
                charging_limit_mw: None,
                min_charging_rate_mw: None,
                target_soc: None,
                energy_request_mwh: None,
                windows_millis: Vec::new(),
            },
            Some("tx-limit-7".to_string()),
            0,
            Provenance::Measured,
            "note".to_string(),
        )
        .unwrap();
        assert_eq!(
            cleared.cleared_limit_transaction_id(),
            Some("tx-limit-7")
        );
        assert_eq!(cleared.kind(), EvProposalKind::ClearedChargingLimit);
    }

    #[test]
    fn non_cleared_kinds_carry_no_cleared_transaction_id() {
        // The cleared-id field is only meaningful for the cleared kind; a
        // needs notification that supplies one still records None (the
        // payload shape does not leak across kinds).
        let record = EvProposalRecord::new(
            "EVSE-1".to_string(),
            "cs".to_string(),
            EvProposalKind::ChargingNeeds,
            EvProposalSource::StationRelayed15118,
            payload(),
            Some("tx-ignored".to_string()),
            0,
            Provenance::Measured,
            "note".to_string(),
        )
        .unwrap();
        assert_eq!(record.cleared_limit_transaction_id(), None);
    }
}
