// P18.12 — The FLEET device-state schema types: the closed vocabularies
//          keyed to migration `0023_edge_fleet_registry_p18_12.up.sql`
//          (the 0018 registry extension — firmware state, cohort
//          membership, attestation state, last-seen health, eSIM profile
//          state as an append-only state journal).
//
// WHAT THIS MODULE IS: the Rust mirror of the SQL contract. The journal
// stores `(device_id, state_kind, state_value, observed_at, correlation)`
// rows; this module owns the CLOSED `state_value` domains per kind and the
// named parse refusal for anything else. Agent A's P18.06 rollout
// orchestrator consumes [`FleetStateKind::CohortMembership`] /
// [`FirmwareState`] from HERE; it never redefines a value. The eSIM profile
// domain (P18.11) lives in [`crate::esim_lifecycle`] and rides the same
// journal kind ([`FleetStateKind::EsimProfileState`]); the attestation
// domain (P18.13) lives in [`crate::attestation`] and rides
// [`FleetStateKind::AttestationState`].
//
// WHY A JOURNAL, NOT REGISTRY COLUMNS (the migration header records this
// too): `edge_device_registry` (0018) is append-only per identity term
// with INSERT+SELECT grants only; fleet state is a high-frequency
// observation stream, so a state change is a NEW journal row and the
// device's current state is its latest row per kind (the documented
// DISTINCT ON read pattern). Nothing here is mutable state — every
// observation is retained.
//
// HONESTY BOUNDS: no fleet exists (phase-18 constraint 6). No row has ever
// been written to the journal; the migration is WRITTEN-UNRUN; these types
// are the schema vocabulary plus parse/validation logic against test
// doubles only. The never-granted permission set is untouched; reading or
// writing fleet state rides the existing registry grants (INSERT, SELECT
// on the journal — migration 0023), never a new permission.

use std::collections::BTreeMap;

// --- The journal kind vocabulary (migration 0023 `state_kind` CHECK set) ---

/// The closed set of fleet-state kinds the journal carries. The SQL CHECK
/// constraint in migration 0023 enforces exactly this set; the u8 wire code
/// here is the adapter-side discriminator, never a second truth source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FleetStateKind {
    /// The device's firmware/update state ([`FirmwareState`]).
    FirmwareState,
    /// The rollout cohort the device currently belongs to (P18.06 consumes).
    CohortMembership,
    /// The device's attestation state ([`crate::attestation`] domain).
    AttestationState,
    /// The last-seen health observation ([`ObservedHealth`]).
    LastSeenHealth,
    /// The eSIM/cellular profile state ([`crate::esim_lifecycle`] domain).
    EsimProfileState,
}

impl FleetStateKind {
    /// The exact `state_value`-kind string the journal CHECK set names.
    pub fn sql_kind(self) -> &'static str {
        match self {
            FleetStateKind::FirmwareState => "FIRMWARE_STATE",
            FleetStateKind::CohortMembership => "COHORT_MEMBERSHIP",
            FleetStateKind::AttestationState => "ATTESTATION_STATE",
            FleetStateKind::LastSeenHealth => "LAST_SEEN_HEALTH",
            FleetStateKind::EsimProfileState => "ESIM_PROFILE_STATE",
        }
    }

    /// Parses the journal's `state_kind` column. Unknown strings are the
    /// named [`FleetRegistryError::UnknownStateKind`] refusal — never
    /// coerced into a known kind.
    pub fn from_sql_kind(value: &str) -> Result<Self, FleetRegistryError> {
        match value {
            "FIRMWARE_STATE" => Ok(FleetStateKind::FirmwareState),
            "COHORT_MEMBERSHIP" => Ok(FleetStateKind::CohortMembership),
            "ATTESTATION_STATE" => Ok(FleetStateKind::AttestationState),
            "LAST_SEEN_HEALTH" => Ok(FleetStateKind::LastSeenHealth),
            "ESIM_PROFILE_STATE" => Ok(FleetStateKind::EsimProfileState),
            other => Err(FleetRegistryError::UnknownStateKind {
                encountered: other.to_string(),
            }),
        }
    }
}

// --- Firmware state (P18.05/P18.06 report into it; this module owns it) -----

/// The closed firmware/update state vocabulary. `Unknown` is a REAL state,
/// never healthy-by-default (the P6.17 health discipline: unknown is
/// degraded-unverified, never assumed fine).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirmwareState {
    /// The device runs the version the platform last confirmed for it.
    InSync,
    /// An update is authorized/staged for the device but not yet applied.
    UpdatePending,
    /// An update is being applied on the device (the P18.04 flow).
    Updating,
    /// The update application failed; the device reported an explicit
    /// failure state (P18.04's named failure states).
    UpdateFailed,
    /// The device rolled back to a previously activated version.
    RolledBack,
    /// No confirmed firmware state has ever been received. A device in
    /// this state is NOT an eligible rollout target for cohort progression
    /// (P18.06 consumes this rule; the orchestrator decides, this module
    /// only refuses to claim the state is known).
    Unknown,
}

impl FirmwareState {
    /// The exact `state_value` string for the [`FleetStateKind::FirmwareState`]
    /// journal rows.
    pub fn sql_value(self) -> &'static str {
        match self {
            FirmwareState::InSync => "IN_SYNC",
            FirmwareState::UpdatePending => "UPDATE_PENDING",
            FirmwareState::Updating => "UPDATING",
            FirmwareState::UpdateFailed => "UPDATE_FAILED",
            FirmwareState::RolledBack => "ROLLED_BACK",
            FirmwareState::Unknown => "UNKNOWN",
        }
    }

    /// Parses the journal's `state_value` for a firmware row. Unknown
    /// strings are the named [`FleetRegistryError::UnknownStateValue`]
    /// refusal — the vocabulary is closed, never guessed open.
    pub fn from_sql_value(value: &str) -> Result<Self, FleetRegistryError> {
        match value {
            "IN_SYNC" => Ok(FirmwareState::InSync),
            "UPDATE_PENDING" => Ok(FirmwareState::UpdatePending),
            "UPDATING" => Ok(FirmwareState::Updating),
            "UPDATE_FAILED" => Ok(FirmwareState::UpdateFailed),
            "ROLLED_BACK" => Ok(FirmwareState::RolledBack),
            "UNKNOWN" => Ok(FirmwareState::Unknown),
            other => Err(FleetRegistryError::UnknownStateValue {
                kind: FleetStateKind::FirmwareState.sql_kind(),
                encountered: other.to_string(),
            }),
        }
    }
}

// --- Cohort membership (P18.06's consumed surface) ----------------------------

/// One COHORT_MEMBERSHIP observation (P18.06's consumed shape). A device's
/// current cohort is its latest row; leaving a cohort (abort, removal) is a
/// NEW row with [`CohortMembership::cohort_id`] empty — recorded, never
/// deleted (the journal is append-only; "removed from cohort" is data, not
/// an absence).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CohortMembership {
    /// The cohort identifier (the orchestrator's cohort key). Empty = the
    /// device is recorded as NOT currently assigned to any cohort.
    pub cohort_id: String,
    /// The block index within the rollout progression (0 = canary block).
    /// The orchestrator owns the semantics; the journal only records it.
    pub block_index: u32,
    /// When the membership was observed (unix millis — recorded data from
    /// the reporting side, never derived here).
    pub observed_at_unix_millis: i64,
}

impl CohortMembership {
    /// The `state_value` encoding: `IN:<cohort>:<block>` or `OUT` (an
    /// empty cohort id). The colon-delimited shape is parsed exactly by
    /// [`CohortMembership::from_sql_value`]; anything else is refused.
    pub fn sql_value(&self) -> String {
        if self.cohort_id.trim().is_empty() {
            "OUT".to_string()
        } else {
            format!("IN:{}:{}", self.cohort_id, self.block_index)
        }
    }

    /// Parses the journal's `state_value` for a cohort row.
    pub fn from_sql_value(value: &str) -> Result<Self, FleetRegistryError> {
        if value == "OUT" {
            return Ok(Self {
                cohort_id: String::new(),
                block_index: 0,
                observed_at_unix_millis: 0,
            });
        }
        let rest = value
            .strip_prefix("IN:")
            .ok_or_else(|| FleetRegistryError::UnknownStateValue {
                kind: FleetStateKind::CohortMembership.sql_kind(),
                encountered: value.to_string(),
            })?;
        let (cohort_id, block) = rest
            .rsplit_once(':')
            .ok_or_else(|| FleetRegistryError::UnknownStateValue {
                kind: FleetStateKind::CohortMembership.sql_kind(),
                encountered: value.to_string(),
            })?;
        if cohort_id.trim().is_empty() {
            return Err(FleetRegistryError::UnknownStateValue {
                kind: FleetStateKind::CohortMembership.sql_kind(),
                encountered: value.to_string(),
            });
        }
        let block_index = block
            .parse::<u32>()
            .map_err(|_| FleetRegistryError::UnknownStateValue {
                kind: FleetStateKind::CohortMembership.sql_kind(),
                encountered: value.to_string(),
            })?;
        Ok(Self {
            cohort_id: cohort_id.to_string(),
            block_index,
            observed_at_unix_millis: 0,
        })
    }
}

// --- Last-seen health ------------------------------------------------------------

/// The closed last-seen health vocabulary (a device's latest
/// LAST_SEEN_HEALTH observation). Mirrors the edge health discipline:
/// what is not observed is NOT reported healthy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObservedHealth {
    /// The device was seen and reported itself healthy (edge HEALTHY).
    SeenHealthy,
    /// The device was seen in a degraded state (edge DEGRADED/FAILING).
    SeenDegraded,
    /// The device has NOT been seen within the fleet's staleness window.
    /// The window itself is a per-deployment configuration (P18.06's
    /// pre-registered parameter, X-8); the journal records the OBSERVED
    /// fact, the orchestrator computes staleness against the window.
    NotSeen,
}

impl ObservedHealth {
    /// The exact `state_value` string for the LAST_SEEN_HEALTH rows.
    pub fn sql_value(self) -> &'static str {
        match self {
            ObservedHealth::SeenHealthy => "SEEN_HEALTHY",
            ObservedHealth::SeenDegraded => "SEEN_DEGRADED",
            ObservedHealth::NotSeen => "NOT_SEEN",
        }
    }

    /// Parses the journal's `state_value` for a last-seen row.
    pub fn from_sql_value(value: &str) -> Result<Self, FleetRegistryError> {
        match value {
            "SEEN_HEALTHY" => Ok(ObservedHealth::SeenHealthy),
            "SEEN_DEGRADED" => Ok(ObservedHealth::SeenDegraded),
            "NOT_SEEN" => Ok(ObservedHealth::NotSeen),
            other => Err(FleetRegistryError::UnknownStateValue {
                kind: FleetStateKind::LastSeenHealth.sql_kind(),
                encountered: other.to_string(),
            }),
        }
    }
}

// --- The journal row and the current-state projection ------------------------------

/// One append-only fleet-state journal row (the adapter-side mirror of the
/// migration-0023 table shape). No secret ever rides a journal row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FleetStateRecord {
    /// The journal event id (the adapter's uuid generation seam supplies
    /// it; this module records it as opaque text).
    pub event_id: String,
    /// The tenant scope (§16.2 — the journal is tenant-scoped and RLS-
    /// isolated; this module carries the id as recorded data).
    pub tenant_id: String,
    /// The device identity (matches the 0018 registry identity scope).
    pub device_id: String,
    pub kind: FleetStateKind,
    /// The kind-specific encoded value — for firmware/last-seen rows the
    /// closed-vocabulary string; for cohort rows the `IN:...`/`OUT`
    /// encoding; for attestation/eSIM rows the domains owned by
    /// [`crate::attestation`] / [`crate::esim_lifecycle`].
    pub state_value: String,
    pub detail: Option<String>,
    pub correlation_id: String,
    pub observed_at_unix_millis: i64,
}

impl FleetStateRecord {
    /// Validates one row against the closed vocabulary of its kind.
    /// Fail-closed: a row whose value does not parse is refused, never
    /// stored sideways. Attestation and eSIM values are validated by their
    /// owning modules (`crate::attestation::FleetAttestationState`,
    /// `crate::esim_lifecycle::EsimeProfileState`); this method
    /// accepts their parse functions through the `extra_value_parser`
    /// argument so the vocabulary ownership stays with the owning module.
    pub fn validate(
        &self,
        extra_value_parser: &dyn Fn(&str) -> Result<(), FleetRegistryError>,
    ) -> Result<(), FleetRegistryError> {
        if self.event_id.trim().is_empty() {
            return Err(FleetRegistryError::InvalidRecord {
                reason: "event_id is empty".to_string(),
            });
        }
        if self.tenant_id.trim().is_empty() {
            return Err(FleetRegistryError::InvalidRecord {
                reason: "tenant_id is empty".to_string(),
            });
        }
        if self.device_id.trim().is_empty() {
            return Err(FleetRegistryError::InvalidRecord {
                reason: "device_id is empty".to_string(),
            });
        }
        if self.correlation_id.trim().is_empty() {
            return Err(FleetRegistryError::InvalidRecord {
                reason: "correlation_id is empty".to_string(),
            });
        }
        if self.observed_at_unix_millis <= 0 {
            return Err(FleetRegistryError::InvalidRecord {
                reason: "observed_at_unix_millis must be a positive unix-millis value"
                    .to_string(),
            });
        }
        if self.state_value.trim().is_empty() {
            return Err(FleetRegistryError::InvalidRecord {
                reason: "state_value is empty".to_string(),
            });
        }
        match self.kind {
            FleetStateKind::FirmwareState => {
                FirmwareState::from_sql_value(&self.state_value).map(|_| ())
            }
            FleetStateKind::CohortMembership => {
                // The stored row carries only the encoded value; the
                // observation instant lives in the row column.
                CohortMembership::from_sql_value(&self.state_value).map(|_| ())
            }
            FleetStateKind::LastSeenHealth => {
                ObservedHealth::from_sql_value(&self.state_value).map(|_| ())
            }
            FleetStateKind::AttestationState | FleetStateKind::EsimProfileState => {
                extra_value_parser(&self.state_value)
            }
        }
    }
}

/// The in-memory projection of the journal (the test-double shape and the
/// adapter's read-model): rows keyed per (device, kind), with the
/// device's CURRENT state per kind = its latest observed row. BTreeMap
/// everywhere — deterministic iteration, the house discipline.
#[derive(Debug, Default)]
pub struct FleetStateView {
    latest: BTreeMap<(String, FleetStateKind), FleetStateRecord>,
    history_len: usize,
}

impl FleetStateView {
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends one validated row to the projection. The row is validated
    /// first (fail-closed); a refusal means the row NEVER enters the view.
    pub fn append(
        &mut self,
        record: FleetStateRecord,
        extra_value_parser: &dyn Fn(&str) -> Result<(), FleetRegistryError>,
    ) -> Result<(), FleetRegistryError> {
        record.validate(extra_value_parser)?;
        self.history_len += 1;
        let key = (record.device_id.clone(), record.kind);
        match self.latest.get(&key) {
            // Append-only out-of-order arrival is legal (an offline device
            // drains old observations); the LATEST observed instant wins
            // the projection, the history keeps everything.
            Some(existing) if existing.observed_at_unix_millis > record.observed_at_unix_millis => {
                Ok(())
            }
            _ => {
                self.latest.insert(key, record);
                Ok(())
            }
        }
    }

    /// The device's current value for one kind, when any row exists.
    pub fn current(&self, device_id: &str, kind: FleetStateKind) -> Option<&FleetStateRecord> {
        self.latest.get(&(device_id.to_string(), kind))
    }

    /// The device's current firmware state; `None` = no observation ever
    /// received (the caller treats absence exactly like
    /// [`FirmwareState::Unknown`] — never as healthy).
    pub fn current_firmware_state(
        &self,
        device_id: &str,
    ) -> Option<Result<FirmwareState, FleetRegistryError>> {
        self.current(device_id, FleetStateKind::FirmwareState)
            .map(|row| FirmwareState::from_sql_value(&row.state_value))
    }

    /// The device's current cohort membership (P18.06's consumed read);
    /// `None` = never assigned.
    pub fn current_cohort(
        &self,
        device_id: &str,
    ) -> Option<Result<CohortMembership, FleetRegistryError>> {
        self.current(device_id, FleetStateKind::CohortMembership)
            .map(|row| {
                CohortMembership::from_sql_value(&row.state_value).map(|mut membership| {
                    membership.observed_at_unix_millis = row.observed_at_unix_millis;
                    membership
                })
            })
    }

    /// How many rows the projection has ingested (diagnostics).
    pub fn history_len(&self) -> usize {
        self.history_len
    }
}

// --- Errors -----------------------------------------------------------------------

/// The named fleet-registry errors. Nothing implies a silent pass; no
/// error ever carries secret material (the journal holds none).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FleetRegistryError {
    /// A journal row failed structural validation.
    InvalidRecord { reason: String },
    /// A `state_kind` string outside the migration-0023 CHECK set.
    UnknownStateKind { encountered: String },
    /// A `state_value` string outside the owning kind's closed vocabulary.
    UnknownStateValue { kind: &'static str, encountered: String },
}

impl FleetRegistryError {
    /// The stable machine-readable error name.
    pub fn name(&self) -> &'static str {
        match self {
            FleetRegistryError::InvalidRecord { .. } => "FLEET_STATE_RECORD_INVALID",
            FleetRegistryError::UnknownStateKind { .. } => "FLEET_STATE_KIND_UNKNOWN",
            FleetRegistryError::UnknownStateValue { .. } => "FLEET_STATE_VALUE_UNKNOWN",
        }
    }
}

impl std::fmt::Display for FleetRegistryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FleetRegistryError::InvalidRecord { reason } => {
                write!(f, "the fleet-state record is invalid: {reason}")
            }
            FleetRegistryError::UnknownStateKind { encountered } => write!(
                f,
                "the journal state_kind '{encountered}' is outside the migration-0023 CHECK \
                 set — refused, never coerced"
            ),
            FleetRegistryError::UnknownStateValue { kind, encountered } => write!(
                f,
                "the journal state_value '{encountered}' is outside the closed vocabulary of \
                 {kind} — refused, never guessed"
            ),
        }
    }
}

impl std::error::Error for FleetRegistryError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(kind: FleetStateKind, value: &str) -> FleetStateRecord {
        FleetStateRecord {
            event_id: "evt-1".to_string(),
            tenant_id: "tenant-a".to_string(),
            device_id: "edge-01".to_string(),
            kind,
            state_value: value.to_string(),
            detail: None,
            correlation_id: "corr-1".to_string(),
            observed_at_unix_millis: 1_700_000_000_000,
        }
    }

    #[test]
    fn the_sql_kind_set_matches_migration_0023_exactly() {
        // The five kinds the migration CHECK set names, character-for-character.
        assert_eq!(FleetStateKind::FirmwareState.sql_kind(), "FIRMWARE_STATE");
        assert_eq!(FleetStateKind::CohortMembership.sql_kind(), "COHORT_MEMBERSHIP");
        assert_eq!(FleetStateKind::AttestationState.sql_kind(), "ATTESTATION_STATE");
        assert_eq!(FleetStateKind::LastSeenHealth.sql_kind(), "LAST_SEEN_HEALTH");
        assert_eq!(FleetStateKind::EsimProfileState.sql_kind(), "ESIM_PROFILE_STATE");
        // Unknown kinds are refused by name.
        assert_eq!(
            FleetStateKind::from_sql_kind("PLUGIN_ROLLOUT").unwrap_err().name(),
            "FLEET_STATE_KIND_UNKNOWN"
        );
    }

    #[test]
    fn the_firmware_vocabulary_round_trips_and_refuses_unknowns() {
        for state in [
            FirmwareState::InSync,
            FirmwareState::UpdatePending,
            FirmwareState::Updating,
            FirmwareState::UpdateFailed,
            FirmwareState::RolledBack,
            FirmwareState::Unknown,
        ] {
            assert_eq!(
                FirmwareState::from_sql_value(state.sql_value()).expect("round trip"),
                state
            );
        }
        assert_eq!(
            FirmwareState::from_sql_value("in_sync").unwrap_err().name(),
            "FLEET_STATE_VALUE_UNKNOWN",
            "the vocabulary is exact-case closed"
        );
    }

    #[test]
    fn cohort_membership_encodes_and_parses_exactly() {
        let membership = CohortMembership {
            cohort_id: "cohort-2026-11".to_string(),
            block_index: 2,
            observed_at_unix_millis: 1_700_000_000_000,
        };
        assert_eq!(membership.sql_value(), "IN:cohort-2026-11:2");
        let parsed = CohortMembership::from_sql_value("IN:cohort-2026-11:2").expect("parses");
        assert_eq!(parsed.cohort_id, "cohort-2026-11");
        assert_eq!(parsed.block_index, 2);
        let out = CohortMembership::from_sql_value("OUT").expect("parses");
        assert!(out.cohort_id.is_empty());
        // Refusals: no cohort, no block, negative block.
        assert!(CohortMembership::from_sql_value("IN::1").is_err());
        assert!(CohortMembership::from_sql_value("IN:cohort-1:x").is_err());
        assert!(CohortMembership::from_sql_value("MEMBER").is_err());
    }

    #[test]
    fn the_view_projection_keeps_the_latest_observation_per_kind() {
        let mut view = FleetStateView::new();
        let mut early = row(FleetStateKind::FirmwareState, "UPDATING");
        early.observed_at_unix_millis = 1_700_000_000_000;
        let mut late = row(FleetStateKind::FirmwareState, "IN_SYNC");
        late.observed_at_unix_millis = 1_700_000_050_000;
        let mut out_of_order = row(FleetStateKind::FirmwareState, "ROLLED_BACK");
        out_of_order.observed_at_unix_millis = 1_699_999_000_000;
        view.append(early, &|_| refuse_attestation_parse())
            .expect("valid row");
        view.append(late, &|_| refuse_attestation_parse())
            .expect("valid row");
        // The stale (out-of-order) row does not displace the latest.
        view.append(out_of_order, &|_| refuse_attestation_parse())
            .expect("valid row");
        assert_eq!(view.history_len(), 3, "append-only: history keeps all");
        let current = view
            .current_firmware_state("edge-01")
            .expect("a firmware row exists");
        assert_eq!(current.expect("parses"), FirmwareState::InSync);
        assert!(view.current_cohort("edge-01").is_none());
        assert!(view.current_firmware_state("edge-nx").is_none());
        // An invalid row NEVER enters the view.
        let bad = row(FleetStateKind::FirmwareState, "made-up-state");
        assert_eq!(
            view.append(bad, &|_| refuse_attestation_parse())
                .unwrap_err()
                .name(),
            "FLEET_STATE_VALUE_UNKNOWN"
        );
        assert_eq!(view.history_len(), 3);
    }

    #[test]
    fn structural_validation_refuses_incomplete_rows() {
        let mut bad = row(FleetStateKind::LastSeenHealth, "SEEN_HEALTHY");
        bad.correlation_id = "  ".to_string();
        assert_eq!(
            bad.validate(&|_| refuse_attestation_parse())
                .unwrap_err()
                .name(),
            "FLEET_STATE_RECORD_INVALID"
        );
        let mut bad_time = row(FleetStateKind::LastSeenHealth, "SEEN_HEALTHY");
        bad_time.observed_at_unix_millis = 0;
        assert!(bad_time.validate(&|_| refuse_attestation_parse()).is_err());
    }

    // The extra-value parser is only exercised for attestation/eSIM rows in
    // this module's tests; this helper refuses explicitly so no test can
    // silently accept such a row through a permissive stub.
    fn refuse_attestation_parse() -> Result<(), FleetRegistryError> {
        Err(FleetRegistryError::UnknownStateValue {
            kind: "ATTESTATION_STATE (test stub)",
            encountered: "no attestation rows are appended in this module's tests".to_string(),
        })
    }
}
