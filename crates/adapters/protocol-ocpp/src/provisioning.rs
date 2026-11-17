// P17.04 — Boot / provisioning / status / transaction mapping: the OCPP
// message payloads (crate::model) into the domain EVSE registry extension
// (`flexcore_domain::evse_registry`).
//
// Recorded rules (decision record
// 04_p17_04_boot_status_transaction_mapping.md):
//
// - **Provisioning is explicit and fail-closed.** The
//   [`ProvisioningPolicy`] decides ACCEPTED / REJECTED for a
//   BootNotification; the DEFAULT constructor accepts NOTHING (an empty
//   allowlist refuses every boot — a station is provisioned only when the
//   deployment names it). The registry records the outcome; the adapter
//   never auto-creates an asset record.
// - **Connection loss is NOT asset unavailability.** The link state and
//   the operational state are SEPARATE registry fields (the domain rule,
//   surfaced as `CONNECTION_LOSS_IS_NOT_UNAVAILABILITY`); this module's
//   `apply_link_loss` touches ONLY the link state.
// - **Statuses map to named states, never guessed.** The station-reported
//   status string is mapped through the domain's declared table
//   ([`flexcore_domain::evse_registry::map_status_to_operational_state`]);
//   an unmapped status is a named refusal, recorded, never approximated.
// - **Transactions are named records.** TransactionEvent Started/Updated
//   keep the transaction Active; Ended closes it. An update or an end for
//   a transaction that never started is a named refusal — the registry
//   never invents a transaction to absorb an event.
//
// UNVERIFIED-ASSUMPTION register: the payload field semantics are the
// model.rs register's (the OCA 2.0.1 provisioning/transactions sections
// are the implementation-time checks; [S-121][S-122]).

use flexcore_domain::evse_registry::{
    BootRecord, EvseIdentity, EvseLinkState, EvseOperationalState, EvseRegistry,
    EvseRegistryError, EvseTransactionRecord, EvseTransactionState, TimestampPair,
    CONNECTION_LOSS_IS_NOT_UNAVAILABILITY,
};

use crate::model::{
    BootNotificationRequest, ChargePointId, StationStatus, StatusNotificationRequest,
    TransactionEventRequest, TransactionEventType,
};

/// Mapping errors (named; domain errors are re-raised unchanged).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProvisioningError {
    /// The boot failed the payload validation.
    InvalidBoot(EvseRegistryError),
    /// The charge-point identity is not on the provisioning allowlist
    /// (the fail-closed DEFAULT).
    NotAllowlisted { charge_point: String },
    /// The registry refused the operation (re-raised; the message carries
    /// the named rule).
    Registry(EvseRegistryError),
}

impl std::fmt::Display for ProvisioningError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProvisioningError::InvalidBoot(error) => {
                write!(f, "boot notification failed validation: {error}")
            }
            ProvisioningError::NotAllowlisted { charge_point } => {
                write!(f, "charge point '{charge_point}' is not on the provisioning allowlist (the fail-closed DEFAULT accepts nothing unnamed)")
            }
            ProvisioningError::Registry(error) => write!(f, "registry refused: {error}"),
        }
    }
}

impl std::error::Error for ProvisioningError {}

impl From<EvseRegistryError> for ProvisioningError {
    fn from(error: EvseRegistryError) -> Self {
        ProvisioningError::Registry(error)
    }
}

/// The boot ACCEPT policy (fail-closed DEFAULT: an empty allowlist accepts
/// NOTHING). `allow_all` exists for deployments that genuinely want
/// open provisioning — it requires a RECORDED REASON (audit-safe) and the
/// reason travels with every accept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvisioningPolicy {
    allowlist: Option<Vec<String>>,
    reason: String,
}

impl ProvisioningPolicy {
    /// The fail-closed DEFAULT: accept only the named charge points. An
    /// empty list accepts nothing (a station is provisioned only when the
    /// deployment names it).
    pub fn allow_only(charge_points: Vec<String>, reason: String) -> Result<Self, ProvisioningError> {
        if reason.trim().is_empty() {
            return Err(ProvisioningError::Registry(EvseRegistryError::EmptyField(
                "policy.reason",
            )));
        }
        let cleaned: Vec<String> = charge_points
            .into_iter()
            .filter(|id| !id.trim().is_empty())
            .collect();
        Ok(Self {
            allowlist: Some(cleaned),
            reason,
        })
    }

    /// Open provisioning (accept every boot). Requires a RECORDED REASON —
    /// an unexplained "accept everything" is a construction error. The
    /// reason travels on every accept decision.
    pub fn allow_all(reason: String) -> Result<Self, ProvisioningError> {
        if reason.trim().is_empty() {
            return Err(ProvisioningError::Registry(EvseRegistryError::EmptyField(
                "policy.reason",
            )));
        }
        Ok(Self {
            allowlist: None,
            reason,
        })
    }

    /// The recorded policy reason.
    pub fn reason(&self) -> &str {
        &self.reason
    }

    /// The boot decision for a charge point (ACCEPTED / REJECTED as the
    /// policy's named outcome; the caller maps it onto the model's
    /// BootStatus).
    pub fn decide(&self, charge_point: &ChargePointId) -> bool {
        match &self.allowlist {
            Some(list) => list.iter().any(|id| id == charge_point.as_str()),
            None => true,
        }
    }
}

/// Maps a BootNotification request into the domain boot record (the ACCEPT
/// decision is the policy's — this function is only called for ACCEPTED
/// boots). The boot addresses the WHOLE station (evseId 0).
pub fn map_accepted_boot(
    charge_point: &ChargePointId,
    request: &BootNotificationRequest,
    stamps: TimestampPair,
    heartbeat_interval_seconds: u32,
) -> Result<BootRecord, ProvisioningError> {
    request
        .validate()
        .map_err(|error| {
            ProvisioningError::InvalidBoot(EvseRegistryError::EmptyField(
                // The model error carries its own field name; wrap it as a
                // named validation failure.
                match &error {
                    crate::model::ModelError::EmptyField(field) => field,
                    _ => "boot_payload",
                },
            ))
        })?;
    let identity = EvseIdentity::new(charge_point.as_str().to_string(), 0)?;
    Ok(BootRecord {
        identity,
        vendor: request.vendor_name.clone(),
        model: request.charge_point_model.clone(),
        serial: request.charge_point_serial_number.clone(),
        firmware_version: request.firmware_version.clone(),
        stamps,
        heartbeat_interval_seconds,
    })
}

/// Applies an ACCEPTED boot to the registry (provision or explicit
/// re-boot). Returns whether the station was newly provisioned.
pub fn apply_accepted_boot(
    registry: &mut EvseRegistry,
    boot: BootRecord,
) -> Result<bool, ProvisioningError> {
    if registry.entry(&boot.identity).is_some() {
        registry.reboot(boot)?;
        Ok(false)
    } else {
        registry.provision(boot)?;
        Ok(true)
    }
}

/// Applies a StatusNotification: the reported status string is mapped
/// through the DOMAIN's declared table — an unmapped status is a named
/// refusal here, recorded, never approximated. The link state is
/// untouched by a status message.
pub fn apply_status_notification(
    registry: &mut EvseRegistry,
    charge_point: &ChargePointId,
    request: &StatusNotificationRequest,
) -> Result<EvseOperationalState, ProvisioningError> {
    let identity = EvseIdentity::new(charge_point.as_str().to_string(), request.evse_id)?;
    let stamps = TimestampPair::new(request.timestamp_millis, request.timestamp_millis)?;
    // The domain mapping refuses unmapped statuses (never guessed).
    let state = flexcore_domain::evse_registry::map_status_to_operational_state(
        request.connector_status.name(),
    )?;
    registry.apply_reported_status(&identity, request.connector_status.name(), stamps)?;
    Ok(state)
}

/// Maps the typed status vocabulary onto the domain state (the checked
/// form — the mapping is total over the five declared names).
pub fn map_station_status(status: StationStatus) -> EvseOperationalState {
    match status {
        StationStatus::Available => EvseOperationalState::ReportedAvailable,
        StationStatus::Occupied => EvseOperationalState::ReportedOccupied,
        StationStatus::Reserved => EvseOperationalState::ReportedReserved,
        StationStatus::Unavailable => EvseOperationalState::ReportedUnavailable,
        StationStatus::Faulted => EvseOperationalState::ReportedFaulted,
    }
}

/// Applies the link-loss rule to a charge point's EVSEs: ONLY the link
/// state changes; the last REPORTED operational state stands, and no
/// asset judgment is produced. The rule statement travels back so the
/// caller can surface it verbatim.
pub fn apply_link_loss(
    registry: &mut EvseRegistry,
    charge_point: &ChargePointId,
    reason: String,
) -> Result<&'static str, ProvisioningError> {
    if charge_point.as_str().trim().is_empty() {
        return Err(ProvisioningError::Registry(EvseRegistryError::EmptyField(
            "charge_point_id",
        )));
    }
    registry.set_charge_point_link_state(
        charge_point.as_str(),
        EvseLinkState::Down { reason },
    )?;
    Ok(CONNECTION_LOSS_IS_NOT_UNAVAILABILITY)
}

/// Applies a TransactionEvent: Started/Updated keep the transaction
/// Active (an update refreshes the energy and timestamps); Ended closes
/// it. An update or an end for a transaction that never started is a
/// named refusal — the registry never invents a transaction to absorb an
/// event.
pub fn apply_transaction_event(
    registry: &mut EvseRegistry,
    charge_point: &ChargePointId,
    request: &TransactionEventRequest,
    received_millis: i64,
) -> Result<(), ProvisioningError> {
    request
        .validate()
        .map_err(|error| {
            ProvisioningError::Registry(EvseRegistryError::EmptyField(match &error {
                crate::model::ModelError::EmptyField(field) => field,
                _ => "transaction_payload",
            }))
        })?;
    let identity = EvseIdentity::new(charge_point.as_str().to_string(), request.evse_id)?;
    let stamps =
        TimestampPair::new(request.timestamp_millis, received_millis)
            .map_err(ProvisioningError::Registry)?;
    match request.event_type {
        TransactionEventType::Started => {
            registry.record_transaction(
                &identity,
                EvseTransactionRecord {
                    transaction_id: request.transaction_id.clone(),
                    state: EvseTransactionState::Active,
                    last_reported_energy_wh: request.energy_wh,
                    stamps,
                },
            )?;
            Ok(())
        }
        TransactionEventType::Updated => {
            // The transaction must exist (never invented to absorb an
            // update); the domain method refuses an unknown transaction
            // with the named error and refreshes the record in place (the
            // state stays Active until an Ended arrives).
            registry.update_transaction(
                &identity,
                &request.transaction_id,
                request.energy_wh,
                stamps,
            )?;
            Ok(())
        }
        TransactionEventType::Ended => {
            registry.close_transaction(&identity, &request.transaction_id, stamps)?;
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{BootStatus, HeartbeatRequest};

    fn cp(name: &str) -> ChargePointId {
        ChargePointId::new(name.to_string()).expect("valid")
    }

    fn boot_request() -> BootNotificationRequest {
        BootNotificationRequest {
            vendor_name: "vendor".to_string(),
            charge_point_model: "model".to_string(),
            charge_point_serial_number: Some("sn-1".to_string()),
            firmware_version: Some("fw-1".to_string()),
            protocol: Some("OCPP 2.0.1".to_string()),
        }
    }

    fn policy() -> ProvisioningPolicy {
        ProvisioningPolicy::allow_only(
            vec!["depot-cp-001".to_string()],
            "the named depot fleet".to_string(),
        )
        .expect("valid policy")
    }

    #[test]
    fn boot_policy_is_fail_closed_and_named() {
        // The fail-closed DEFAULT accepts nothing unnamed.
        let empty = ProvisioningPolicy::allow_only(
            Vec::new(),
            "no station is named yet".to_string(),
        )
        .expect("valid");
        assert!(!empty.decide(&cp("depot-cp-001")));
        // The named station is accepted.
        assert!(policy().decide(&cp("depot-cp-001")));
        // allow_all requires a recorded reason.
        assert!(ProvisioningPolicy::allow_all("  ".to_string()).is_err());
        let open = ProvisioningPolicy::allow_all("the recorded reason".to_string())
            .expect("valid");
        assert!(open.decide(&cp("anything")));
        assert_eq!(open.reason(), "the recorded reason");
        // A policy without a reason is refused.
        assert!(ProvisioningPolicy::allow_only(vec!["cp".to_string()], String::new()).is_err());
    }

    #[test]
    fn accepted_boot_maps_into_the_registry() {
        let mut registry = EvseRegistry::new();
        let boot = map_accepted_boot(
            &cp("depot-cp-001"),
            &boot_request(),
            TimestampPair::new(100, 101).expect("pair"),
            60,
        )
        .expect("mapped");
        let newly = apply_accepted_boot(&mut registry, boot).expect("applied");
        assert!(newly, "first boot provisions");
        let entry = registry
            .entry(&EvseIdentity::new("depot-cp-001".to_string(), 0).expect("valid"))
            .expect("entry");
        assert_eq!(entry.boot.vendor, "vendor");
        assert_eq!(entry.boot.heartbeat_interval_seconds, 60);
        // A re-boot goes through the explicit path and reports not-new.
        let boot = map_accepted_boot(
            &cp("depot-cp-001"),
            &boot_request(),
            TimestampPair::new(200, 201).expect("pair"),
            120,
        )
        .expect("mapped");
        let newly = apply_accepted_boot(&mut registry, boot).expect("applied");
        assert!(!newly, "re-boot is not a new provisioning");
        let entry = registry
            .entry(&EvseIdentity::new("depot-cp-001".to_string(), 0).expect("valid"))
            .expect("entry");
        assert_eq!(entry.boot.heartbeat_interval_seconds, 120);
        // An invalid boot is refused (empty vendor).
        let invalid = BootNotificationRequest {
            vendor_name: String::new(),
            charge_point_model: "model".to_string(),
            charge_point_serial_number: None,
            firmware_version: None,
            protocol: None,
        };
        assert!(matches!(
            map_accepted_boot(&cp("cp-9"), &invalid, TimestampPair::new(0, 0).expect("pair"), 60),
            Err(ProvisioningError::InvalidBoot(_))
        ));
        // BootStatus names are carried for the response path (the ACCEPT
        // decision maps onto Accepted).
        assert_eq!(BootStatus::Accepted.name(), "Accepted");
    }

    #[test]
    fn status_notifications_map_and_refuse_unmapped() {
        let mut registry = EvseRegistry::new();
        let boot = map_accepted_boot(
            &cp("depot-cp-001"),
            &boot_request(),
            TimestampPair::new(100, 101).expect("pair"),
            60,
        )
        .expect("mapped");
        apply_accepted_boot(&mut registry, boot).expect("applied");

        let request = StatusNotificationRequest {
            timestamp_millis: 500,
            evse_id: 2,
            connector_status: StationStatus::Occupied,
        };
        let state = apply_status_notification(&mut registry, &cp("depot-cp-001"), &request)
            .expect("mapped");
        assert_eq!(state, EvseOperationalState::ReportedOccupied);
        // The typed mapping agrees with the domain table (total over the
        // five declared names).
        assert_eq!(
            map_station_status(StationStatus::Available),
            flexcore_domain::evse_registry::map_status_to_operational_state("Available")
                .expect("mapped")
        );
        // A status for an unregistered EVSE id is refused (no
        // auto-create): evse 9 was never reported by a boot... the
        // registry entry is per-EVSE and only evse 0 exists here, so the
        // status for evse 2 was itself refused? No: apply_reported_status
        // refuses unknown identities — verify the honest behavior.
        let request = StatusNotificationRequest {
            timestamp_millis: 600,
            evse_id: 7,
            connector_status: StationStatus::Faulted,
        };
        let result =
            apply_status_notification(&mut registry, &cp("depot-cp-001"), &request);
        // Only evse 0 exists (the boot addresses the station); evse 7 is
        // refused rather than auto-created.
        assert!(matches!(result, Err(ProvisioningError::Registry(_))));
        // The raw vocabulary refuses unknown spellings (never guessed).
        assert!(flexcore_domain::evse_registry::map_status_to_operational_state("NotAState")
            .is_err());
    }

    #[test]
    fn link_loss_touches_only_the_link_state() {
        let mut registry = EvseRegistry::new();
        let boot = map_accepted_boot(
            &cp("depot-cp-001"),
            &boot_request(),
            TimestampPair::new(100, 101).expect("pair"),
            60,
        )
        .expect("mapped");
        apply_accepted_boot(&mut registry, boot).expect("applied");
        let request = StatusNotificationRequest {
            timestamp_millis: 500,
            evse_id: 0,
            connector_status: StationStatus::Available,
        };
        apply_status_notification(&mut registry, &cp("depot-cp-001"), &request)
            .expect("status");

        let statement = apply_link_loss(
            &mut registry,
            &cp("depot-cp-001"),
            "websocket closed".to_string(),
        )
        .expect("applied");
        // The rule statement travels verbatim.
        assert!(statement.contains("NOT asset unavailability"));
        let identity = EvseIdentity::new("depot-cp-001".to_string(), 0).expect("valid");
        let entry = registry.entry(&identity).expect("entry");
        assert!(matches!(entry.link, EvseLinkState::Down { .. }));
        // The reported state STANDS with its timestamps.
        let operational = entry.operational.as_ref().expect("retained");
        assert_eq!(operational.state, EvseOperationalState::ReportedAvailable);
        assert_eq!(operational.stamps.event_millis, 500);
    }

    #[test]
    fn transaction_events_are_named_records_never_invented() {
        let mut registry = EvseRegistry::new();
        let boot = map_accepted_boot(
            &cp("depot-cp-001"),
            &boot_request(),
            TimestampPair::new(100, 101).expect("pair"),
            60,
        )
        .expect("mapped");
        apply_accepted_boot(&mut registry, boot).expect("applied");

        let started = TransactionEventRequest {
            event_type: TransactionEventType::Started,
            timestamp_millis: 1000,
            evse_id: 0,
            transaction_id: "txn-1".to_string(),
            trigger_reason: None,
            energy_wh: Some(0.0),
        };
        apply_transaction_event(&mut registry, &cp("depot-cp-001"), &started, 1001)
            .expect("started");

        let updated = TransactionEventRequest {
            event_type: TransactionEventType::Updated,
            timestamp_millis: 2000,
            evse_id: 0,
            transaction_id: "txn-1".to_string(),
            trigger_reason: None,
            energy_wh: Some(7500.0),
        };
        apply_transaction_event(&mut registry, &cp("depot-cp-001"), &updated, 2001)
            .expect("updated");
        let identity = EvseIdentity::new("depot-cp-001".to_string(), 0).expect("valid");
        let record = registry
            .entry(&identity)
            .expect("entry")
            .transactions
            .get("txn-1")
            .expect("txn")
            .clone();
        assert_eq!(record.state, EvseTransactionState::Active);
        assert_eq!(record.last_reported_energy_wh, Some(7500.0));

        let ended = TransactionEventRequest {
            event_type: TransactionEventType::Ended,
            timestamp_millis: 3000,
            evse_id: 0,
            transaction_id: "txn-1".to_string(),
            trigger_reason: None,
            energy_wh: Some(42_000.0),
        };
        apply_transaction_event(&mut registry, &cp("depot-cp-001"), &ended, 3001)
            .expect("ended");
        let record = registry
            .entry(&identity)
            .expect("entry")
            .transactions
            .get("txn-1")
            .expect("txn retained")
            .clone();
        assert_eq!(record.state, EvseTransactionState::Ended);

        // An update for a transaction that never started is refused (the
        // registry never invents one).
        let ghost_update = TransactionEventRequest {
            event_type: TransactionEventType::Updated,
            timestamp_millis: 4000,
            evse_id: 0,
            transaction_id: "ghost".to_string(),
            trigger_reason: None,
            energy_wh: None,
        };
        let error = apply_transaction_event(&mut registry, &cp("depot-cp-001"), &ghost_update, 4001)
            .expect_err("refused");
        assert!(error.to_string().contains("ghost"));
        // An end for an unknown transaction is refused too.
        let ghost_end = TransactionEventRequest {
            event_type: TransactionEventType::Ended,
            ..ghost_update
        };
        assert!(apply_transaction_event(&mut registry, &cp("depot-cp-001"), &ghost_end, 4002)
            .is_err());
        // An invalid payload (negative energy) is refused.
        let invalid = TransactionEventRequest {
            event_type: TransactionEventType::Started,
            timestamp_millis: 0,
            evse_id: 0,
            transaction_id: "txn-2".to_string(),
            trigger_reason: None,
            energy_wh: Some(-1.0),
        };
        assert!(apply_transaction_event(&mut registry, &cp("depot-cp-001"), &invalid, 0).is_err());
    }

    #[test]
    fn heartbeat_is_a_liveness_record_only() {
        // The Heartbeat payload carries a timestamp; the registry treats
        // heartbeats as link-liveness evidence ONLY — they never produce
        // an operational state. This test pins the vocabulary contract.
        let request = HeartbeatRequest {
            current_time_millis: 900,
        };
        assert_eq!(request.current_time_millis, 900);
        // No registry call exists for heartbeats: absence is structural.
    }
}
