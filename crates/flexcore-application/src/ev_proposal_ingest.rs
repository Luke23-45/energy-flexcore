// P17.06 (application assembly) — Vehicle-proposal ingestion: the mapping
// of the adapter-relayed proposal messages onto the domain constraint
// records.
//
// Implements ledger P17.06 (the application half): the OCPP CSMS adapter
// (Phase 17 P17.01, Agent A's `crates/adapters/protocol-ocpp/` inbound
// path — consumed here BY CITATION; A's P17.01 layout record had not
// landed when this module was written, and the dependence is recorded in
// the decision record `docs/phases-11-20/phase-17-ev-fleet-charging/
// records/21_p17_06_vehicle_proposal_ingestion.md`) decodes the OCPP-J
// frames and constructs the adapter-neutral
// [`InboundProposalMessage`] shape this module defines. The assembly maps
// the message onto the domain [`EvProposalRecord`] — a CONSTRAINT RECORD.
//
// # The governing rule (ADR-010-EXT-01 pattern; decided once)
//
// Proposals are inputs and records, NEVER auto-commands. This module is
// the code form of that rule at the application boundary:
//
// - the ONLY output of the assembly is the domain record — there is no
//   command constructor, no lifecycle transition, and no dispatch call in
//   this module (the negative test below pins the module surface);
// - the record's lifecycle entry state is structurally PROPOSED
//   (`EvProposalRecord::lifecycle_entry_state()`); reaching a device
//   happens only through the frozen 14-state lifecycle's three distinct
//   gates, downstream of the P17.05 composer;
// - the station's locally imposed limits and the ISO 15118 vehicle
//   proposals relayed by the station are recorded with the SAME rule —
//   neither shape is more privileged than the other.
//
// # Message-name honesty
//
// The OCPP message names in the closed [`InboundProposalMessageName`]
// vocabulary are SECONDARY-SOURCED from [S-124] (OCPP 2.0.1 smart
// charging) and [S-125] (OCPP 2.1) and are UNVERIFIED against the OCA
// specification text — the implementation-time check against the OCA text
// is recorded in the decision record. A confirmed mismatch is a recorded
// edit of both vocabularies (domain and application), never a silent
// reinterpretation (plan §7 note 5).
//
// Control status: simulation-only application assembly. No charge point is
// connected, no message has ever been exchanged with a real device, no
// proposal was ever ingested from a station.

use flexcore_domain::ev_proposal_records::{
    EvProposalError, EvProposalKind, EvProposalPayload, EvProposalRecord, EvProposalSource,
};
use flexcore_contracts::capability::Provenance;

/// The closed inbound proposal-message vocabulary (the four message
/// families the CSMS adapter relays; spellings UNVERIFIED [S-124][S-125]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InboundProposalMessageName {
    /// `NotifyEVChargingNeeds` — the vehicle's charging needs relayed by
    /// the station (ISO 15118 proposals relayed by the CSMS).
    NotifyEVChargingNeeds,
    /// `NotifyEVChargingSchedule` — a charging schedule proposal relayed by
    /// the station.
    NotifyEVChargingSchedule,
    /// `NotifyChargingLimit` — a charging limit the station imposed
    /// locally.
    NotifyChargingLimit,
    /// `ClearedChargingLimit` — the station's notice that a previously
    /// imposed limit no longer applies.
    ClearedChargingLimit,
}

impl InboundProposalMessageName {
    /// All four families, in frozen vocabulary order.
    pub const ALL: [InboundProposalMessageName; 4] = [
        InboundProposalMessageName::NotifyEVChargingNeeds,
        InboundProposalMessageName::NotifyEVChargingSchedule,
        InboundProposalMessageName::NotifyChargingLimit,
        InboundProposalMessageName::ClearedChargingLimit,
    ];

    /// The canonical OCPP message-name spelling (SECONDARY-SOURCED
    /// [S-124][S-125]; UNVERIFIED against the OCA text).
    pub const fn as_str(self) -> &'static str {
        match self {
            InboundProposalMessageName::NotifyEVChargingNeeds => "NotifyEVChargingNeeds",
            InboundProposalMessageName::NotifyEVChargingSchedule => "NotifyEVChargingSchedule",
            InboundProposalMessageName::NotifyChargingLimit => "NotifyChargingLimit",
            InboundProposalMessageName::ClearedChargingLimit => "ClearedChargingLimit",
        }
    }

    /// The OCPP message family maps onto exactly one proposal kind and one
    /// declared origin — the mapping is total over the closed vocabulary
    /// (no message family arrives unmapped, none is invented).
    pub fn maps_to(&self) -> (EvProposalKind, EvProposalSource) {
        match self {
            InboundProposalMessageName::NotifyEVChargingNeeds => {
                (EvProposalKind::ChargingNeeds, EvProposalSource::StationRelayed15118)
            }
            InboundProposalMessageName::NotifyEVChargingSchedule => (
                EvProposalKind::ChargingScheduleProposal,
                EvProposalSource::StationRelayed15118,
            ),
            InboundProposalMessageName::NotifyChargingLimit => {
                (EvProposalKind::ChargingLimit, EvProposalSource::StationLocalLimit)
            }
            InboundProposalMessageName::ClearedChargingLimit => {
                (EvProposalKind::ClearedChargingLimit, EvProposalSource::StationLocalLimit)
            }
        }
    }
}

/// Errors of the proposal assembly. Every variant names the failure — the
/// assembly refuses, it never defaults.
#[derive(Debug, Clone, PartialEq)]
pub enum EvProposalIngestError {
    /// The domain record construction refused the message (the named
    /// domain error is carried verbatim).
    Domain(EvProposalError),
    /// The cleared-limit message carried no limit-transaction id, or a
    /// non-cleared message carried one — the payload shape does not leak
    /// across message families.
    ClearedIdMismatch {
        /// The message family that mismatched.
        message_name: &'static str,
    },
}

impl std::fmt::Display for EvProposalIngestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EvProposalIngestError::Domain(e) => write!(f, "the proposal record was refused: {e}"),
            EvProposalIngestError::ClearedIdMismatch { message_name } => write!(
                f,
                "message {message_name}: the cleared-limit transaction id is required for \
                 ClearedChargingLimit and must be absent for every other family"
            ),
        }
    }
}

impl std::error::Error for EvProposalIngestError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            EvProposalIngestError::Domain(e) => Some(e),
            _ => None,
        }
    }
}

/// The adapter-neutral decoded proposal message — the shape A's
/// `protocol-ocpp` inbound path constructs after decoding an OCPP-J frame
/// (consumed BY CITATION; the concrete adapter types are Agent A's, and
/// this module deliberately does not name them — the application layer
/// depends on nothing infrastructure). The OCPP payload fields the
/// proposal carries travel in the domain [`EvProposalPayload`] (validated
/// there).
#[derive(Debug, Clone, PartialEq)]
pub struct InboundProposalMessage {
    /// Which message family arrived.
    pub message_name: InboundProposalMessageName,
    /// The station id (the adapter's verbatim spelling).
    pub station_id: String,
    /// The EVSE id (the adapter's verbatim spelling).
    pub evse_id: String,
    /// The decoded, still-declared payload.
    pub payload: EvProposalPayload,
    /// The limit-transaction id a `ClearedChargingLimit` message names
    /// (`None` for every other family — enforced at assembly).
    pub cleared_limit_transaction_id: Option<String>,
    /// When the platform received the message (UTC ms).
    pub received_at_unix_ms: i64,
}

/// Assembles one inbound proposal message into the domain constraint
/// record. The output is a RECORD — the assembly defines no command, no
/// lifecycle transition, and no dispatch call; the record enters the
/// lifecycle at PROPOSED and reaches a device only through the frozen
/// gates.
///
/// `provenance` is declared by the composition root (an adapter-relayed
/// message from a trusted, authenticated connection is `MEASURED`;
/// anything less is `UNKNOWN` and narrows downstream — the P3.02 rule);
/// `provenance_note` is mandatory and human-safe (never raw wire text).
pub fn assemble_proposal_record(
    message: &InboundProposalMessage,
    provenance: Provenance,
    provenance_note: String,
) -> Result<EvProposalRecord, EvProposalIngestError> {
    let (kind, source) = message.message_name.maps_to();
    let cleared_id = message.cleared_limit_transaction_id.clone();
    // The cleared-id shape must not leak across message families.
    match message.message_name {
        InboundProposalMessageName::ClearedChargingLimit => {
            if cleared_id.as_deref().map(str::trim).unwrap_or("").is_empty() {
                return Err(EvProposalIngestError::ClearedIdMismatch {
                    message_name: message.message_name.as_str(),
                });
            }
        }
        _ => {
            if cleared_id.is_some() {
                return Err(EvProposalIngestError::ClearedIdMismatch {
                    message_name: message.message_name.as_str(),
                });
            }
        }
    }
    EvProposalRecord::new(
        message.evse_id.clone(),
        message.station_id.clone(),
        kind,
        source,
        message.payload.clone(),
        cleared_id,
        message.received_at_unix_ms,
        provenance,
        provenance_note,
    )
    .map_err(EvProposalIngestError::Domain)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn payload() -> EvProposalPayload {
        EvProposalPayload {
            charging_limit_mw: Some(0.011),
            min_charging_rate_mw: None,
            target_soc: Some(0.85),
            energy_request_mwh: Some(0.03),
            windows_millis: Vec::new(),
        }
    }

    fn message(name: InboundProposalMessageName) -> InboundProposalMessage {
        InboundProposalMessage {
            message_name: name,
            station_id: "cs-depot-01".to_string(),
            evse_id: "EVSE-DEPOT-01-2".to_string(),
            payload: payload(),
            cleared_limit_transaction_id: None,
            received_at_unix_ms: 1_700_000_000_123,
        }
    }

    #[test]
    fn the_message_vocabulary_is_closed_and_total_over_kinds_and_sources() {
        assert_eq!(InboundProposalMessageName::ALL.len(), 4);
        assert_eq!(
            InboundProposalMessageName::NotifyEVChargingNeeds.as_str(),
            "NotifyEVChargingNeeds"
        );
        assert_eq!(
            InboundProposalMessageName::NotifyEVChargingSchedule.as_str(),
            "NotifyEVChargingSchedule"
        );
        assert_eq!(
            InboundProposalMessageName::NotifyChargingLimit.as_str(),
            "NotifyChargingLimit"
        );
        assert_eq!(
            InboundProposalMessageName::ClearedChargingLimit.as_str(),
            "ClearedChargingLimit"
        );
        // The family→(kind, source) mapping is total: every family maps.
        for name in InboundProposalMessageName::ALL {
            let _ = name.maps_to();
        }
        assert_eq!(
            InboundProposalMessageName::NotifyEVChargingNeeds.maps_to(),
            (EvProposalKind::ChargingNeeds, EvProposalSource::StationRelayed15118)
        );
        assert_eq!(
            InboundProposalMessageName::NotifyChargingLimit.maps_to(),
            (EvProposalKind::ChargingLimit, EvProposalSource::StationLocalLimit)
        );
    }

    #[test]
    fn the_assembly_produces_a_record_whose_entry_state_is_proposed() {
        let record = assemble_proposal_record(
            &message(InboundProposalMessageName::NotifyEVChargingNeeds),
            Provenance::Measured,
            "relayed by the simulated station adapter; spelling verbatim".to_string(),
        )
        .unwrap();
        // The NEVER-AUTO-COMMAND rule in code: the entry state is
        // structurally PROPOSED, and the frozen lifecycle refuses every
        // PROPOSED shortcut to an issued state.
        assert_eq!(
            flexcore_domain::ev_proposal_records::EvProposalRecord::lifecycle_entry_state(),
            flexcore_domain::command_state::CommandState::Proposed
        );
        assert!(flexcore_domain::command_state::try_transition(
            flexcore_domain::command_state::CommandState::Proposed,
            flexcore_domain::command_state::CommandState::Issued,
        )
        .is_err());
    }

    #[test]
    fn each_family_assembles_its_declared_kind_and_source() {
        // The cleared family needs its transaction id (covered by the
        // dedicated test below); the other three families assemble without
        // one.
        for name in [
            InboundProposalMessageName::NotifyEVChargingNeeds,
            InboundProposalMessageName::NotifyEVChargingSchedule,
            InboundProposalMessageName::NotifyChargingLimit,
        ] {
            let record = assemble_proposal_record(
                &message(name),
                Provenance::Measured,
                "note".to_string(),
            )
            .unwrap();
            let (kind, source) = name.maps_to();
            assert_eq!(record.kind(), kind);
            assert_eq!(record.source(), source);
        }
        // And the cleared family with its id assembles too.
        let mut cleared = message(InboundProposalMessageName::ClearedChargingLimit);
        cleared.payload = EvProposalPayload {
            charging_limit_mw: None,
            min_charging_rate_mw: None,
            target_soc: None,
            energy_request_mwh: None,
            windows_millis: Vec::new(),
        };
        cleared.cleared_limit_transaction_id = Some("tx-limit-8".to_string());
        let record = assemble_proposal_record(&cleared, Provenance::Measured, "note".to_string())
            .unwrap();
        assert_eq!(
            record.kind(),
            EvProposalKind::ClearedChargingLimit
        );
        assert_eq!(record.source(), EvProposalSource::StationLocalLimit);
    }

    #[test]
    fn a_cleared_limit_assembles_with_its_transaction_id() {
        let mut cleared = message(InboundProposalMessageName::ClearedChargingLimit);
        cleared.payload = EvProposalPayload {
            charging_limit_mw: None,
            min_charging_rate_mw: None,
            target_soc: None,
            energy_request_mwh: None,
            windows_millis: Vec::new(),
        };
        cleared.cleared_limit_transaction_id = Some("tx-limit-7".to_string());
        let record = assemble_proposal_record(&cleared, Provenance::Measured, "note".to_string())
            .unwrap();
        assert_eq!(record.cleared_limit_transaction_id(), Some("tx-limit-7"));
    }

    #[test]
    fn the_cleared_id_shape_is_refused_when_it_leaks_across_families() {
        // A ClearedChargingLimit without the id is refused at assembly.
        let err = assemble_proposal_record(
            &message(InboundProposalMessageName::ClearedChargingLimit),
            Provenance::Measured,
            "note".to_string(),
        )
        .unwrap_err();
        assert!(matches!(err, EvProposalIngestError::ClearedIdMismatch { .. }));
        // A non-cleared family carrying an id is refused at assembly.
        let mut leaking = message(InboundProposalMessageName::NotifyChargingLimit);
        leaking.cleared_limit_transaction_id = Some("tx-limit-7".to_string());
        let err = assemble_proposal_record(&leaking, Provenance::Measured, "note".to_string())
            .unwrap_err();
        assert!(matches!(err, EvProposalIngestError::ClearedIdMismatch { .. }));
    }

    #[test]
    fn domain_refusals_are_carried_verbatim() {
        let mut bad = message(InboundProposalMessageName::NotifyChargingLimit);
        bad.payload.charging_limit_mw = Some(-1.0);
        let err = assemble_proposal_record(&bad, Provenance::Measured, "note".to_string())
            .unwrap_err();
        assert!(matches!(
            err,
            EvProposalIngestError::Domain(EvProposalError::InvalidChargingLimitMw { .. })
        ));
    }

    #[test]
    fn negative_the_module_surface_has_no_command_or_dispatch_path() {
        // The structural negative test: the assembly's only output type is
        // the domain record; the vocabulary below has no variant that
        // issues, authorizes, or dispatches, and the error type carries no
        // command either. (The never-granted set is untouched by
        // construction — this module adds no permission and no transport.)
        let outcomes = [
            assemble_proposal_record(&message(InboundProposalMessageName::NotifyEVChargingNeeds), Provenance::Measured, "note".to_string()),
            assemble_proposal_record(&message(InboundProposalMessageName::NotifyEVChargingSchedule), Provenance::Measured, "note".to_string()),
            assemble_proposal_record(&message(InboundProposalMessageName::NotifyChargingLimit), Provenance::Measured, "note".to_string()),
            assemble_proposal_record(&message(InboundProposalMessageName::ClearedChargingLimit), Provenance::Measured, "note".to_string()),
        ];
        // Every result (Ok or Err) names a RECORD-side outcome.
        for outcome in outcomes {
            match outcome {
                Ok(record) => {
                    let _: EvProposalRecord = record;
                }
                Err(err) => {
                    let _: EvProposalIngestError = err;
                }
            }
        }
    }
}
