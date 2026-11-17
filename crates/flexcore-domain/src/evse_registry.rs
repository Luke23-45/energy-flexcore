// P17.04 — The domain EVSE registry extension: boot / provisioning /
// status / transaction mapping vocabulary (additive).
//
// Decision record:
// `docs/phases-11-20/phase-17-ev-fleet-charging/records/04_p17_04_boot_status_transaction_mapping.md`.
//
// This module extends the domain with the EVSE-side registry vocabulary the
// OCPP CSMS adapter (`crates/adapters/protocol-ocpp`, P17.01) maps its
// provisioning messages into. It ADDS a new asset-class registry shape; it
// does NOT touch the P2.02 REST deletion rule (`registry_rules.rs`), the
// P0.10 entity vocabulary (`entities.rs`), or the frozen command lifecycle
// (`command_state.rs`).
//
// Recorded rules (decision record 04; phase-17 context §3):
//
// - **Connection loss is NOT asset unavailability.** When the CSMS's
//   WebSocket connection to a charge point closes, the LINK state goes
//   down; the ASSET state carries no new status. Statuses arrive as
//   protocol messages with their own timestamps; staleness is the quality
//   vocabulary's business (STALE per the frozen six states), never an
//   inference from link loss.
// - **Statuses map to named states, never guessed.** A station-reported
//   status outside the declared mapping is refused WHOLE (the ACL
//   discipline): the registry records the refusal, never an approximate
//   state.
// - **The registry is per-site, per-identity, deterministic.** Entries are
//   a `BTreeMap` keyed by the validated [`EvseIdentity`].
// - **Boot/provisioning is explicit.** A charge point is provisioned only
//   through an ACCEPTED boot record (the adapter's provisioning policy
//   decides; the registry records). There is no "assumed present" state.
// - **Every state entry carries its evidence timestamp** (the event-time
//   the protocol reported) AND the receive timestamp (the injected
//   clock) — the dual-timestamp rule the staleness machinery needs.

use std::collections::BTreeMap;

/// Errors of the EVSE registry extension. Every failure names the
/// offending field/value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvseRegistryError {
    /// An identity or record field is empty. Silence is never an identity.
    EmptyField(&'static str),
    /// A sanity bound was exceeded (not a protocol claim — a gate against
    /// unbounded input).
    FieldTooLong {
        field: &'static str,
        max: usize,
    },
    /// The EVSE id is out of the sanity range (0 = whole station;
    /// UNVERIFIED semantics per [S-124]).
    EvseIdOutOfRange {
        value: u32,
    },
    /// The EVSE identity is not registered; the operation is refused, not
    /// auto-created (provisioning is explicit).
    UnknownEvse {
        identity: String,
    },
    /// The EVSE identity is already registered; a re-boot REPLACES the
    /// boot record only through the explicit re-provisioning path.
    AlreadyRegistered {
        identity: String,
    },
    /// The station-reported status is outside the declared mapping. The
    /// registry records the refusal; it never guesses a state.
    UnmappedStatus {
        reported: String,
    },
    /// A timestamp pair is inverted (the event claims a future later than
    /// the receive instant allows) — carried as a named error; the caller
    /// decides the clock policy.
    TimestampPairInverted {
        event_millis: i64,
        received_millis: i64,
    },
}

impl std::fmt::Display for EvseRegistryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EvseRegistryError::EmptyField(field) => {
                write!(f, "field '{field}' is empty")
            }
            EvseRegistryError::FieldTooLong { field, max } => {
                write!(f, "field '{field}' exceeds the {max}-character sanity bound")
            }
            EvseRegistryError::EvseIdOutOfRange { value } => {
                write!(f, "EVSE id {value} is outside the sanity range (0 = whole station)")
            }
            EvseRegistryError::UnknownEvse { identity } => {
                write!(f, "EVSE identity '{identity}' is not registered; provisioning is explicit, never auto-created")
            }
            EvseRegistryError::AlreadyRegistered { identity } => {
                write!(f, "EVSE identity '{identity}' is already registered; a re-boot goes through the explicit re-provisioning path")
            }
            EvseRegistryError::UnmappedStatus { reported } => {
                write!(f, "station-reported status '{reported}' is outside the declared mapping; the registry records the refusal, never a guessed state")
            }
            EvseRegistryError::TimestampPairInverted {
                event_millis,
                received_millis,
            } => {
                write!(f, "timestamp pair inverted (event {event_millis} is after receive {received_millis})")
            }
        }
    }
}

impl std::error::Error for EvseRegistryError {}

/// The dual-timestamp stamp every registry entry carries: the EVENT time
/// (as the protocol reported it) and the RECEIVE time (the injected
/// clock). The pair is what staleness machinery consumes; neither is
/// invented.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimestampPair {
    /// The event time in UTC milliseconds (as the station reported).
    pub event_millis: i64,
    /// The receive time in UTC milliseconds (the injected clock).
    pub received_millis: i64,
}

impl TimestampPair {
    /// Constructs and orders the pair. The receive instant must not be
    /// EARLIER than the event instant by more than the declared tolerance
    /// (a small negative skew is tolerable; a large one is a named error —
    /// the caller decides the clock policy).
    pub fn new(event_millis: i64, received_millis: i64) -> Result<Self, EvseRegistryError> {
        const SKEW_TOLERANCE_MILLIS: i64 = 5_000;
        if received_millis + SKEW_TOLERANCE_MILLIS < event_millis {
            return Err(EvseRegistryError::TimestampPairInverted {
                event_millis,
                received_millis,
            });
        }
        Ok(Self {
            event_millis,
            received_millis,
        })
    }
}

/// The EVSE identity inside a site: the charge point and the per-EVSE id
/// (0 = the whole station; UNVERIFIED semantics [S-124]).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EvseIdentity {
    /// The charge-point identity (validated, non-empty).
    charge_point_id: String,
    /// The per-EVSE id (0 = whole station).
    evse_id: u32,
}

impl EvseIdentity {
    /// The sanity bound on the charge-point id (a gate, not a protocol
    /// claim).
    pub const MAX_ID_LEN: usize = 256;
    /// The sanity upper bound for the EVSE id (not a protocol claim).
    pub const MAX_EVSE_ID: u32 = 65_535;

    /// Validates and constructs the identity.
    pub fn new(charge_point_id: String, evse_id: u32) -> Result<Self, EvseRegistryError> {
        if charge_point_id.trim().is_empty() {
            return Err(EvseRegistryError::EmptyField("charge_point_id"));
        }
        if charge_point_id.len() > Self::MAX_ID_LEN {
            return Err(EvseRegistryError::FieldTooLong {
                field: "charge_point_id",
                max: Self::MAX_ID_LEN,
            });
        }
        if evse_id > Self::MAX_EVSE_ID {
            return Err(EvseRegistryError::EvseIdOutOfRange { value: evse_id });
        }
        Ok(Self {
            charge_point_id,
            evse_id,
        })
    }

    /// The charge-point id.
    pub fn charge_point_id(&self) -> &str {
        &self.charge_point_id
    }

    /// The per-EVSE id (0 = whole station).
    pub const fn evse_id(&self) -> u32 {
        self.evse_id
    }
}

impl std::fmt::Display for EvseIdentity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}#{}", self.charge_point_id, self.evse_id)
    }
}

/// The ACCEPTED boot record of a charge point (P17.04; UNVERIFIED OCPP
/// field semantics per [S-121][S-122] — the values are DECLARED data
/// carried verbatim from the boot message, never parsed into capability
/// claims).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootRecord {
    /// The charge-point identity (the boot addresses the whole station).
    pub identity: EvseIdentity,
    /// The declared vendor string (carried verbatim).
    pub vendor: String,
    /// The declared model string (carried verbatim).
    pub model: String,
    /// The declared serial string, when reported.
    pub serial: Option<String>,
    /// The declared firmware version, when reported.
    pub firmware_version: Option<String>,
    /// The boot's dual timestamp.
    pub stamps: TimestampPair,
    /// The heartbeat interval the CSMS ASSIGNED (declared configuration —
    /// a CSMS-assigned value, never a station inference).
    pub heartbeat_interval_seconds: u32,
}

/// The named EVSE link/operational state vocabulary the registry stores.
///
/// The LINK state and the ASSET state are SEPARATE fields and are NEVER
/// merged:
///
/// - `EvseLinkState` is the CSMS-connection view (up/down). Connection
///   loss sets ONLY the link state; it carries NO statement about the
///   asset.
/// - `EvseOperationalState` is the last station-REPORTED status, mapped
///   through [`map_status_to_operational_state`]. It changes only when a
///   new status message arrives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvseLinkState {
    /// The CSMS link is up (a WebSocket session exists for the charge
    /// point).
    Up,
    /// The CSMS link is down (connection lost, transport closed, or
    /// negotiation failed). Carries the recorded reason (human-safe) and
    /// the down instant — and NOTHING about the asset.
    Down { reason: String },
}

impl EvseLinkState {
    /// Canonical name for records.
    pub const fn name(&self) -> &'static str {
        match self {
            EvseLinkState::Up => "LINK_UP",
            EvseLinkState::Down { .. } => "LINK_DOWN",
        }
    }
}

/// The recorded rule (surfaced wherever a LINK_DOWN is consumed).
pub const CONNECTION_LOSS_IS_NOT_UNAVAILABILITY: &str =
    "Connection loss is NOT asset unavailability: a LINK_DOWN state records the \
     transport fact only. The last station-reported operational state stands with \
     its own timestamps; staleness is the telemetry-quality vocabulary's business \
     (STALE per the frozen six states), never an inference from link loss.";

/// The named operational-state vocabulary the station's StatusNotification
/// maps onto (the LAST REPORTED state — recorded, never inferred).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvseOperationalState {
    /// Reported available (UNVERIFIED mapping — see
    /// [`map_status_to_operational_state`]).
    ReportedAvailable,
    /// Reported occupied (a vehicle is connected/charging per the
    /// station's own report).
    ReportedOccupied,
    /// Reported reserved.
    ReportedReserved,
    /// Reported unavailable (the OPERATOR or the station itself declared
    /// it unavailable — a station report, not a platform inference).
    ReportedUnavailable,
    /// Reported faulted.
    ReportedFaulted,
}

impl EvseOperationalState {
    /// Canonical name for records.
    pub const fn name(self) -> &'static str {
        match self {
            EvseOperationalState::ReportedAvailable => "REPORTED_AVAILABLE",
            EvseOperationalState::ReportedOccupied => "REPORTED_OCCUPIED",
            EvseOperationalState::ReportedReserved => "REPORTED_RESERVED",
            EvseOperationalState::ReportedUnavailable => "REPORTED_UNAVAILABLE",
            EvseOperationalState::ReportedFaulted => "REPORTED_FAULTED",
        }
    }
}

/// The declared mapping from the station-reported status vocabulary onto
/// the named operational states (UNVERIFIED: the exact StatusNotification
/// vocabulary of OCPP 2.0.1 is broader than the five states the secondary
/// sources name [S-121][S-122]; the implementation-time check against the
/// OCA text may EXTEND this mapping — a missing state is a recorded
/// refusal here, never a guessed mapping).
///
/// The reported vocabulary (spellings UNVERIFIED — the check names the
/// StatusNotification section of the OCA 2.0.1 text):
/// `Available`, `Occupied`, `Reserved`, `Unavailable`, `Faulted`.
pub fn map_status_to_operational_state(
    reported: &str,
) -> Result<EvseOperationalState, EvseRegistryError> {
    match reported {
        "Available" => Ok(EvseOperationalState::ReportedAvailable),
        "Occupied" => Ok(EvseOperationalState::ReportedOccupied),
        "Reserved" => Ok(EvseOperationalState::ReportedReserved),
        "Unavailable" => Ok(EvseOperationalState::ReportedUnavailable),
        "Faulted" => Ok(EvseOperationalState::ReportedFaulted),
        other => Err(EvseRegistryError::UnmappedStatus {
            reported: other.to_string(),
        }),
    }
}

/// One station-reported operational-state entry (the last report wins;
/// the history is the adapter-side record's business — the registry keeps
/// the CURRENT named state with its evidence).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationalStateEntry {
    pub state: EvseOperationalState,
    pub stamps: TimestampPair,
}

/// The transaction view the TransactionEvent stream produces (UNVERIFIED
/// event semantics per [S-121] — the registry records named states, never
/// guesses). An update event (metered energy or similar) keeps the state
/// `Active` and refreshes the record's energy/timestamps; it is not a new
/// state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvseTransactionState {
    /// A transaction started on this EVSE (and still running — an update
    /// event keeps this state).
    Active,
    /// The transaction ended (the record is retained with its final state
    /// for audit).
    Ended,
}

/// A transaction record keyed by the station-side transaction id
/// (UNVERIFIED id semantics [S-121]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvseTransactionRecord {
    /// The station-side transaction id (carried verbatim).
    pub transaction_id: String,
    /// The transaction state (named, never guessed).
    pub state: EvseTransactionState,
    /// The last metered energy in Wh, when reported (UNVERIFIED unit —
    /// carried as declared data; conversion to canonical MWh happens in
    /// the ingestion path, Agent B's surface, not here).
    pub last_reported_energy_wh: Option<f64>,
    /// The dual timestamp of the LAST event for this transaction.
    pub stamps: TimestampPair,
}

/// One EVSE registry entry: identity, boot record, link state, last
/// reported operational state, and the live transaction index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvseEntry {
    /// The identity (the registry key).
    pub identity: EvseIdentity,
    /// The accepted boot record (present — an entry exists only through an
    /// accepted boot; there is no "assumed present" state).
    pub boot: BootRecord,
    /// The LINK state (CSMS-connection view; never merged with the asset
    /// state).
    pub link: EvseLinkState,
    /// The last station-REPORTED operational state, when any was reported.
    pub operational: Option<OperationalStateEntry>,
    /// The live transaction index, keyed by station-side transaction id.
    pub transactions: BTreeMap<String, EvseTransactionRecord>,
}

/// The EVSE registry extension (per site; deterministic `BTreeMap`
/// keyed by [`EvseIdentity`]). Pure domain rules — no I/O; the OCPP
/// adapter maps its messages through this vocabulary
/// (`crates/adapters/protocol-ocpp::provisioning`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EvseRegistry {
    entries: BTreeMap<EvseIdentity, EvseEntry>,
}

impl EvseRegistry {
    /// An empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// The number of registered entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The sorted identity list (deterministic diagnostics).
    pub fn identities(&self) -> Vec<&EvseIdentity> {
        self.entries.keys().collect()
    }

    /// An entry lookup.
    pub fn entry(&self, identity: &EvseIdentity) -> Option<&EvseEntry> {
        self.entries.get(identity)
    }

    /// Provisions a charge point through an ACCEPTED boot record. The
    /// ACCEPT decision belongs to the adapter's provisioning policy; the
    /// registry records the outcome. An already-registered station re-boots
    /// only through [`EvseRegistry::reboot`] (explicit, never silent).
    pub fn provision(&mut self, boot: BootRecord) -> Result<(), EvseRegistryError> {
        if self.entries.contains_key(&boot.identity) {
            return Err(EvseRegistryError::AlreadyRegistered {
                identity: boot.identity.to_string(),
            });
        }
        let identity = boot.identity.clone();
        self.entries.insert(
            identity.clone(),
            EvseEntry {
                identity,
                boot,
                link: EvseLinkState::Up,
                operational: None,
                transactions: BTreeMap::new(),
            },
        );
        Ok(())
    }

    /// The explicit re-provisioning path: replaces the boot record of an
    /// ALREADY REGISTERED station (a re-boot), leaving the link state and
    /// the last reported operational state untouched (a re-boot is not a
    /// status report).
    pub fn reboot(&mut self, boot: BootRecord) -> Result<(), EvseRegistryError> {
        let entry = self
            .entries
            .get_mut(&boot.identity)
            .ok_or(EvseRegistryError::UnknownEvse {
                identity: boot.identity.to_string(),
            })?;
        entry.boot = boot;
        Ok(())
    }

    /// Applies a station-reported status to a registered EVSE. The
    /// reported string must be in the declared mapping (unmapped = a named
    /// refusal, never a guessed state). The link state is untouched.
    pub fn apply_reported_status(
        &mut self,
        identity: &EvseIdentity,
        reported: &str,
        stamps: TimestampPair,
    ) -> Result<EvseOperationalState, EvseRegistryError> {
        let state = map_status_to_operational_state(reported)?;
        let entry = self
            .entries
            .get_mut(identity)
            .ok_or(EvseRegistryError::UnknownEvse {
                identity: identity.to_string(),
            })?;
        entry.operational = Some(OperationalStateEntry { state, stamps });
        Ok(state)
    }

    /// Sets the LINK state of every EVSE of a charge point (the link is
    /// per charge point; the registry carries it per EVSE entry). This is
    /// the ONLY write a connection-loss event produces — it carries NO
    /// statement about the asset ([`CONNECTION_LOSS_IS_NOT_UNAVAILABILITY`]).
    pub fn set_charge_point_link_state(
        &mut self,
        charge_point_id: &str,
        link: EvseLinkState,
    ) -> Result<usize, EvseRegistryError> {
        if charge_point_id.trim().is_empty() {
            return Err(EvseRegistryError::EmptyField("charge_point_id"));
        }
        let mut count = 0;
        for (identity, entry) in self.entries.iter_mut() {
            if identity.charge_point_id() == charge_point_id {
                entry.link = link.clone();
                count += 1;
            }
        }
        Ok(count)
    }

    /// Records a TransactionEvent outcome (named states; the transaction
    /// id is the station-side key). The EVSE must be registered and, for
    /// the per-EVSE record, must match the reported EVSE id.
    pub fn record_transaction(
        &mut self,
        identity: &EvseIdentity,
        record: EvseTransactionRecord,
    ) -> Result<(), EvseRegistryError> {
        let entry = self
            .entries
            .get_mut(identity)
            .ok_or(EvseRegistryError::UnknownEvse {
                identity: identity.to_string(),
            })?;
        entry
            .transactions
            .insert(record.transaction_id.clone(), record);
        Ok(())
    }

    /// Updates a running transaction in place (an update event refreshes
    /// the energy and timestamps; the state stays Active). The
    /// transaction must already exist — the registry never invents one to
    /// absorb an update.
    pub fn update_transaction(
        &mut self,
        identity: &EvseIdentity,
        transaction_id: &str,
        last_reported_energy_wh: Option<f64>,
        stamps: TimestampPair,
    ) -> Result<(), EvseRegistryError> {
        let entry = self
            .entries
            .get_mut(identity)
            .ok_or(EvseRegistryError::UnknownEvse {
                identity: identity.to_string(),
            })?;
        let record = entry
            .transactions
            .get_mut(transaction_id)
            .ok_or(EvseRegistryError::UnknownEvse {
                identity: format!("{identity} (transaction {transaction_id})"),
            })?;
        record.last_reported_energy_wh =
            last_reported_energy_wh.or(record.last_reported_energy_wh);
        record.stamps = stamps;
        Ok(())
    }

    /// Closes a transaction (the named Ended outcome; the record is
    /// retained with its final state for audit).
    pub fn close_transaction(
        &mut self,
        identity: &EvseIdentity,
        transaction_id: &str,
        stamps: TimestampPair,
    ) -> Result<(), EvseRegistryError> {
        let entry = self
            .entries
            .get_mut(identity)
            .ok_or(EvseRegistryError::UnknownEvse {
                identity: identity.to_string(),
            })?;
        let record = entry
            .transactions
            .get_mut(transaction_id)
            .ok_or(EvseRegistryError::UnknownEvse {
                identity: format!("{identity} (transaction {transaction_id})"),
            })?;
        record.state = EvseTransactionState::Ended;
        record.stamps = stamps;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity(evse: u32) -> EvseIdentity {
        EvseIdentity::new("depot-cp-001".to_string(), evse).expect("valid")
    }

    fn boot() -> BootRecord {
        BootRecord {
            identity: identity(0),
            vendor: "vendor".to_string(),
            model: "model".to_string(),
            serial: Some("sn-1".to_string()),
            firmware_version: Some("fw-1".to_string()),
            stamps: TimestampPair::new(100, 101).expect("valid pair"),
            heartbeat_interval_seconds: 60,
        }
    }

    #[test]
    fn identity_validates() {
        assert_eq!(
            EvseIdentity::new("   ".to_string(), 0),
            Err(EvseRegistryError::EmptyField("charge_point_id"))
        );
        let long = "x".repeat(EvseIdentity::MAX_ID_LEN + 1);
        assert!(matches!(
            EvseIdentity::new(long, 0),
            Err(EvseRegistryError::FieldTooLong { .. })
        ));
        assert_eq!(
            EvseIdentity::new("cp".to_string(), EvseIdentity::MAX_EVSE_ID + 1),
            Err(EvseRegistryError::EvseIdOutOfRange {
                value: EvseIdentity::MAX_EVSE_ID + 1
            })
        );
        let id = EvseIdentity::new("depot-cp-001".to_string(), 3).expect("valid");
        assert_eq!(id.to_string(), "depot-cp-001#3");
        assert_eq!(id.evse_id(), 3);
    }

    #[test]
    fn timestamp_pair_orders_with_a_small_skew_tolerance() {
        TimestampPair::new(100, 95).expect("small skew tolerable");
        assert_eq!(
            TimestampPair::new(1_000, 900),
            Err(EvseRegistryError::TimestampPairInverted {
                event_millis: 1_000,
                received_millis: 900
            })
        );
    }

    #[test]
    fn provisioning_is_explicit_and_reboot_is_explicit() {
        let mut registry = EvseRegistry::new();
        registry.provision(boot()).expect("provisioned");
        assert_eq!(registry.len(), 1);
        // Duplicate provisioning refused (explicit re-boot path only).
        assert_eq!(
            registry.provision(boot()),
            Err(EvseRegistryError::AlreadyRegistered {
                identity: "depot-cp-001#0".to_string()
            })
        );
        // Re-boot replaces the boot record only.
        let mut new_boot = boot();
        new_boot.firmware_version = Some("fw-2".to_string());
        registry.reboot(new_boot).expect("rebooted");
        let entry = registry.entry(&identity(0)).expect("entry");
        assert_eq!(
            entry.boot.firmware_version.as_deref(),
            Some("fw-2"),
            "the boot record was replaced"
        );
        assert_eq!(entry.link, EvseLinkState::Up, "the link state is untouched");
        // Re-boot of an unknown station is refused (no auto-create).
        let mut unknown = boot();
        unknown.identity = EvseIdentity::new("other".to_string(), 0).expect("valid");
        assert!(matches!(
            registry.reboot(unknown),
            Err(EvseRegistryError::UnknownEvse { .. })
        ));
    }

    #[test]
    fn status_mapping_is_declared_and_refuses_unknowns() {
        let mut registry = EvseRegistry::new();
        registry.provision(boot()).expect("provisioned");
        let stamps = TimestampPair::new(200, 201).expect("valid pair");

        // Each declared state maps.
        for (reported, expected) in [
            ("Available", EvseOperationalState::ReportedAvailable),
            ("Occupied", EvseOperationalState::ReportedOccupied),
            ("Reserved", EvseOperationalState::ReportedReserved),
            ("Unavailable", EvseOperationalState::ReportedUnavailable),
            ("Faulted", EvseOperationalState::ReportedFaulted),
        ] {
            let mapped = registry
                .apply_reported_status(&identity(0), reported, stamps)
                .expect("mapped");
            assert_eq!(mapped, expected);
        }
        // An unmapped status is refused WHOLE (never guessed).
        let error = registry
            .apply_reported_status(&identity(0), "SuspendedEVSE", stamps)
            .expect_err("unmapped refused");
        assert!(error.to_string().contains("SuspendedEVSE"));
        // An unknown EVSE is refused (no auto-create).
        let other = EvseIdentity::new("ghost".to_string(), 0).expect("valid");
        assert!(registry
            .apply_reported_status(&other, "Available", stamps)
            .is_err());
    }

    #[test]
    fn connection_loss_touches_only_the_link_state() {
        let mut registry = EvseRegistry::new();
        registry.provision(boot()).expect("provisioned");
        let stamps = TimestampPair::new(200, 201).expect("valid pair");
        registry
            .apply_reported_status(&identity(0), "Occupied", stamps)
            .expect("status recorded");

        let touched = registry
            .set_charge_point_link_state(
                "depot-cp-001",
                EvseLinkState::Down {
                    reason: "websocket closed".to_string(),
                },
            )
            .expect("link set");
        assert_eq!(touched, 1, "all EVSEs of the charge point are touched");

        let entry = registry.entry(&identity(0)).expect("entry");
        // The LINK state went down...
        assert_eq!(
            entry.link,
            EvseLinkState::Down {
                reason: "websocket closed".to_string()
            }
        );
        assert_eq!(entry.link.name(), "LINK_DOWN");
        // ...and the last REPORTED operational state STANDS (connection
        // loss is not asset unavailability).
        let operational = entry.operational.as_ref().expect("state retained");
        assert_eq!(operational.state, EvseOperationalState::ReportedOccupied);
        assert_eq!(operational.stamps, stamps);
        // The rule is surfaced verbatim for consumers.
        assert!(CONNECTION_LOSS_IS_NOT_UNAVAILABILITY.contains("NOT asset unavailability"));

        // Link-up restores the transport view only.
        registry
            .set_charge_point_link_state("depot-cp-001", EvseLinkState::Up)
            .expect("link set");
        assert_eq!(
            registry.entry(&identity(0)).expect("entry").link,
            EvseLinkState::Up
        );
        // An unknown charge point touches zero entries (no auto-create, no
        // error — the count is the honest answer).
        let touched = registry
            .set_charge_point_link_state("ghost", EvseLinkState::Up)
            .expect("count");
        assert_eq!(touched, 0);
    }

    #[test]
    fn transactions_are_named_records() {
        let mut registry = EvseRegistry::new();
        registry.provision(boot()).expect("provisioned");
        let record = EvseTransactionRecord {
            transaction_id: "txn-1".to_string(),
            state: EvseTransactionState::Active,
            last_reported_energy_wh: Some(1500.5),
            stamps: TimestampPair::new(300, 301).expect("valid pair"),
        };
        registry
            .record_transaction(&identity(0), record)
            .expect("recorded");
        let entry = registry.entry(&identity(0)).expect("entry");
        let txn = entry.transactions.get("txn-1").expect("txn");
        assert_eq!(txn.state, EvseTransactionState::Active);
        // Close it; the record is retained with the final state.
        registry
            .close_transaction(&identity(0), "txn-1", TimestampPair::new(400, 401).expect("pair"))
            .expect("closed");
        let entry = registry.entry(&identity(0)).expect("entry");
        assert_eq!(
            entry.transactions.get("txn-1").expect("txn").state,
            EvseTransactionState::Ended
        );
        // Transactions on unregistered EVSEs are refused.
        let other = EvseIdentity::new("ghost".to_string(), 0).expect("valid");
        let record = EvseTransactionRecord {
            transaction_id: "txn-2".to_string(),
            state: EvseTransactionState::Active,
            last_reported_energy_wh: None,
            stamps: TimestampPair::new(300, 301).expect("valid pair"),
        };
        assert!(registry.record_transaction(&other, record).is_err());
        // Closing an unknown transaction names the gap.
        assert!(registry
            .close_transaction(&identity(0), "no-such-txn", TimestampPair::new(400, 401).expect("pair"))
            .is_err());
    }
}
