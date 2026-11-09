// P0.14 — Event vocabulary: facts, not commands (Phase 0 freeze)
// P1.09 — flexcore-events implementation: the 26 §15.3 events as typed
//         payloads with versioned serde serialization.
//
// Implements `ledger/phase-00-product-freeze/14_event_vocabulary.md`.
// Normative source: report §15.3 (event vocabulary), §15.4 (delivery
// semantics), §6.3 / §6.4 / §15.5 (transactional outbox).
//
// Frozen rules encoded here:
//
// - **Events are facts, commands are imperatives.** An event such as
//   `DispatchIssued` records that a command reached the ISSUED state; it is
//   not itself the command and must never carry imperative semantics. No
//   event name from the frozen list may also be used as a command name.
//   Treating an event as a command risks double-execution.
// - **Exactly 26 events**, spelled exactly as in report §15.3 (see
//   [`DomainEvent`] and [`event_name`]). There is no `DispatchValidated` or
//   `DispatchFeasible` event in §15.3 and none may be invented.
// - **Typed payloads**: every event carries a correlation ID, a tenant ID,
//   the portfolio/site/asset IDs applicable per the frozen event, its
//   `event_time` (contracts type, P0.09), and — for dispatch-lifecycle
//   events — the command ID (see the ASSUMPTIONS note below).
// - **Delivery is at-least-once** (NATS JetStream), so **consumers must be
//   idempotent**; exactly-once side effects are not promised by broker
//   acknowledgements or deduplication options alone.
//
// Serialization: the enum is internally tagged (`"event_type"`) and variant
// names are SCREAMING_SNAKE_CASE (for example `"ASSET_REGISTERED"`); payload
// fields are snake_case. `event_name` returns the frozen PascalCase spelling
// from §15.3 — the human-readable contract name; the wire tag is the
// SCREAMING_SNAKE rendering of the same name.
//
// ASSUMPTIONS (payload field selection): the P0.14 freeze fixes the event
// names and domain areas, not the per-event field lists; the report §15.3
// prose was not transcribed field-by-field into the freeze. The ID fields
// below follow the frozen domain areas and the §14.3 dispatch invariants
// (dispatch events carry the command ID). They are recorded as an
// ASSUMPTION for the P1.12 schema design and P1.14 topology design to
// review; adjusting a payload is a schema-version change, never a silent
// field repurpose.
//
// ASSUMPTIONS (typed IDs): `TelemetryQualityChanged` carries the six-state
// quality vocabulary from P0.08; plugin events carry a `plugin_id`
// (`FlexUuid`); `AssetAvailabilityChanged` (frozen area "Asset state") uses
// the site/asset scope shown below. These choices are likewise recorded for
// later-phase review.

use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::quality::TelemetryQuality;
use flexcore_contracts::time_model::EventTime;
use flexcore_domain::ids::{AssetId, PortfolioId, SiteId, TenantId};
use serde::{Deserialize, Serialize};

/// Field-level (de)serialization helpers for the domain typed identifiers.
///
/// The domain crate deliberately takes no serde dependency, so this crate
/// (which owns its event schema) delegates to `FlexUuid`'s serde impl and
/// wraps/reads the typed ID newtypes around it.
mod serde_ids {
    use flexcore_contracts::ids::FlexUuid;
    use flexcore_domain::ids::{AssetId, PortfolioId, SiteId, TenantId};
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub mod tenant_id {
        use super::*;
        pub fn serialize<S: Serializer>(id: &TenantId, serializer: S) -> Result<S::Ok, S::Error> {
            FlexUuid::serialize(&id.uuid(), serializer)
        }
        pub fn deserialize<'de, D: Deserializer<'de>>(
            deserializer: D,
        ) -> Result<TenantId, D::Error> {
            Ok(TenantId::new(FlexUuid::deserialize(deserializer)?))
        }
    }

    pub mod portfolio_id {
        use super::*;
        pub fn serialize<S: Serializer>(
            id: &PortfolioId,
            serializer: S,
        ) -> Result<S::Ok, S::Error> {
            FlexUuid::serialize(&id.uuid(), serializer)
        }
        pub fn deserialize<'de, D: Deserializer<'de>>(
            deserializer: D,
        ) -> Result<PortfolioId, D::Error> {
            Ok(PortfolioId::new(FlexUuid::deserialize(deserializer)?))
        }
    }

    pub mod site_id {
        use super::*;
        pub fn serialize<S: Serializer>(id: &SiteId, serializer: S) -> Result<S::Ok, S::Error> {
            FlexUuid::serialize(&id.uuid(), serializer)
        }
        pub fn deserialize<'de, D: Deserializer<'de>>(
            deserializer: D,
        ) -> Result<SiteId, D::Error> {
            Ok(SiteId::new(FlexUuid::deserialize(deserializer)?))
        }
    }

    pub mod asset_id {
        use super::*;
        pub fn serialize<S: Serializer>(id: &AssetId, serializer: S) -> Result<S::Ok, S::Error> {
            FlexUuid::serialize(&id.uuid(), serializer)
        }
        pub fn deserialize<'de, D: Deserializer<'de>>(
            deserializer: D,
        ) -> Result<AssetId, D::Error> {
            Ok(AssetId::new(FlexUuid::deserialize(deserializer)?))
        }
    }
}

// --- Payload structs (one per frozen event, in report §15.3 order) ----------

/// `AssetRegistered` payload — an asset was registered (asset registry).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetRegisteredPayload {
    pub correlation_id: FlexUuid,
    #[serde(with = "serde_ids::tenant_id")]
    pub tenant_id: TenantId,
    #[serde(with = "serde_ids::portfolio_id")]
    pub portfolio_id: PortfolioId,
    #[serde(with = "serde_ids::site_id")]
    pub site_id: SiteId,
    #[serde(with = "serde_ids::asset_id")]
    pub asset_id: AssetId,
    pub event_time: EventTime,
}

/// `AssetCapabilityUpdated` payload — an asset's capability set was updated
/// (asset registry).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetCapabilityUpdatedPayload {
    pub correlation_id: FlexUuid,
    #[serde(with = "serde_ids::tenant_id")]
    pub tenant_id: TenantId,
    #[serde(with = "serde_ids::portfolio_id")]
    pub portfolio_id: PortfolioId,
    #[serde(with = "serde_ids::site_id")]
    pub site_id: SiteId,
    #[serde(with = "serde_ids::asset_id")]
    pub asset_id: AssetId,
    pub event_time: EventTime,
}

/// `TelemetryReceived` payload — telemetry was received (telemetry).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TelemetryReceivedPayload {
    pub correlation_id: FlexUuid,
    #[serde(with = "serde_ids::tenant_id")]
    pub tenant_id: TenantId,
    #[serde(with = "serde_ids::site_id")]
    pub site_id: SiteId,
    #[serde(with = "serde_ids::asset_id")]
    pub asset_id: AssetId,
    pub event_time: EventTime,
}

/// `TelemetryQualityChanged` payload — a telemetry value's quality state
/// changed (telemetry). Carries the six-state quality vocabulary from P0.08
/// (`flexcore_contracts::quality`); the non-silencing rule applies to every
/// consumer of these fields.
///
/// P2.16 step-2 revision (recorded): the payload carries BOTH the previous
/// and the new state. `quality` keeps its frozen name and meaning (the NEW
/// state — no field repurpose); the additive `previous_quality` field was
/// added with a `serde(default)` so v1 envelopes without it still parse
/// (backward-compatible addition, not a breaking change — the schema version
/// stays 1 per the P1.09 versioning rule). Emitters always set
/// `previous_quality` to `Some` (the event only fires on an actual change,
/// so a previous state always exists at emission).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TelemetryQualityChangedPayload {
    pub correlation_id: FlexUuid,
    #[serde(with = "serde_ids::tenant_id")]
    pub tenant_id: TenantId,
    #[serde(with = "serde_ids::site_id")]
    pub site_id: SiteId,
    #[serde(with = "serde_ids::asset_id")]
    pub asset_id: AssetId,
    /// The NEW canonical quality state (the frozen field name and meaning).
    pub quality: TelemetryQuality,
    /// The quality state BEFORE the change (P2.16 step 2). `None` only in
    /// envelopes produced before this revision (serde default for wire
    /// compatibility); current emitters always set it.
    #[serde(default)]
    pub previous_quality: Option<TelemetryQuality>,
    pub event_time: EventTime,
}

/// `AssetAvailabilityChanged` payload — an asset's availability changed
/// (asset state; ASSUMPTION: site/asset scope, see module header).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetAvailabilityChangedPayload {
    pub correlation_id: FlexUuid,
    #[serde(with = "serde_ids::tenant_id")]
    pub tenant_id: TenantId,
    #[serde(with = "serde_ids::site_id")]
    pub site_id: SiteId,
    #[serde(with = "serde_ids::asset_id")]
    pub asset_id: AssetId,
    pub event_time: EventTime,
}

/// `FlexibilityEnvelopeUpdated` payload — a flexibility envelope was updated
/// (flexibility).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlexibilityEnvelopeUpdatedPayload {
    pub correlation_id: FlexUuid,
    #[serde(with = "serde_ids::tenant_id")]
    pub tenant_id: TenantId,
    #[serde(with = "serde_ids::portfolio_id")]
    pub portfolio_id: PortfolioId,
    #[serde(with = "serde_ids::site_id")]
    pub site_id: SiteId,
    #[serde(with = "serde_ids::asset_id")]
    pub asset_id: AssetId,
    pub event_time: EventTime,
}

/// `ForecastPublished` payload — a forecast was published (forecasting).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForecastPublishedPayload {
    pub correlation_id: FlexUuid,
    #[serde(with = "serde_ids::tenant_id")]
    pub tenant_id: TenantId,
    #[serde(with = "serde_ids::portfolio_id")]
    pub portfolio_id: PortfolioId,
    #[serde(with = "serde_ids::site_id")]
    pub site_id: SiteId,
    #[serde(with = "serde_ids::asset_id")]
    pub asset_id: AssetId,
    pub event_time: EventTime,
}

/// `MarketSignalReceived` payload — a market signal was received (market).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketSignalReceivedPayload {
    pub correlation_id: FlexUuid,
    #[serde(with = "serde_ids::tenant_id")]
    pub tenant_id: TenantId,
    #[serde(with = "serde_ids::portfolio_id")]
    pub portfolio_id: PortfolioId,
    pub event_time: EventTime,
}

/// `ScheduleProposed` payload — a schedule was proposed (scheduling).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScheduleProposedPayload {
    pub correlation_id: FlexUuid,
    #[serde(with = "serde_ids::tenant_id")]
    pub tenant_id: TenantId,
    #[serde(with = "serde_ids::portfolio_id")]
    pub portfolio_id: PortfolioId,
    #[serde(with = "serde_ids::site_id")]
    pub site_id: SiteId,
    #[serde(with = "serde_ids::asset_id")]
    pub asset_id: AssetId,
    pub event_time: EventTime,
}

/// `ScheduleValidated` payload — a schedule was validated (scheduling).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScheduleValidatedPayload {
    pub correlation_id: FlexUuid,
    #[serde(with = "serde_ids::tenant_id")]
    pub tenant_id: TenantId,
    #[serde(with = "serde_ids::portfolio_id")]
    pub portfolio_id: PortfolioId,
    #[serde(with = "serde_ids::site_id")]
    pub site_id: SiteId,
    #[serde(with = "serde_ids::asset_id")]
    pub asset_id: AssetId,
    pub event_time: EventTime,
}

/// `FeasibilityCheckCompleted` payload — a feasibility check completed (grid
/// validation).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeasibilityCheckCompletedPayload {
    pub correlation_id: FlexUuid,
    #[serde(with = "serde_ids::tenant_id")]
    pub tenant_id: TenantId,
    #[serde(with = "serde_ids::portfolio_id")]
    pub portfolio_id: PortfolioId,
    #[serde(with = "serde_ids::site_id")]
    pub site_id: SiteId,
    #[serde(with = "serde_ids::asset_id")]
    pub asset_id: AssetId,
    pub event_time: EventTime,
}

macro_rules! dispatch_payload {
    (
        $(#[$meta:meta])*
        $name:ident
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
        pub struct $name {
            pub correlation_id: FlexUuid,
            #[serde(with = "serde_ids::tenant_id")]
            pub tenant_id: TenantId,
            #[serde(with = "serde_ids::portfolio_id")]
            pub portfolio_id: PortfolioId,
            #[serde(with = "serde_ids::site_id")]
            pub site_id: SiteId,
            #[serde(with = "serde_ids::asset_id")]
            pub asset_id: AssetId,
            /// The §14.2 command this lifecycle fact is about (ASSUMPTION:
            /// dispatch events identify the command, see module header).
            pub command_id: FlexUuid,
            pub event_time: EventTime,
        }
    };
}

dispatch_payload! {
    /// `DispatchProposed` payload — a dispatch command reached PROPOSED
    /// (dispatch lifecycle).
    DispatchProposedPayload
}
dispatch_payload! {
    /// `DispatchApproved` payload — a dispatch command reached APPROVED
    /// (dispatch lifecycle).
    DispatchApprovedPayload
}
dispatch_payload! {
    /// `DispatchRejected` payload — a dispatch command reached REJECTED
    /// (dispatch lifecycle).
    DispatchRejectedPayload
}
dispatch_payload! {
    /// `DispatchIssued` payload — a dispatch command reached ISSUED
    /// (dispatch lifecycle). A fact about issue, never the command itself.
    DispatchIssuedPayload
}
dispatch_payload! {
    /// `DispatchAcknowledged` payload — a dispatch command reached
    /// ACKNOWLEDGED (dispatch lifecycle). Acknowledgement is not physical
    /// delivery (P0.11).
    DispatchAcknowledgedPayload
}
dispatch_payload! {
    /// `DispatchTrackingUpdated` payload — a dispatch command in TRACKING
    /// reported updated measurements (dispatch lifecycle).
    DispatchTrackingUpdatedPayload
}
dispatch_payload! {
    /// `DispatchCompleted` payload — a dispatch command reached COMPLETED
    /// (dispatch lifecycle).
    DispatchCompletedPayload
}
dispatch_payload! {
    /// `DispatchFailed` payload — a dispatch command reached FAILED
    /// (dispatch lifecycle).
    DispatchFailedPayload
}
dispatch_payload! {
    /// `DispatchExpired` payload — a dispatch command reached EXPIRED
    /// (dispatch lifecycle).
    DispatchExpiredPayload
}
dispatch_payload! {
    /// `DispatchCancelled` payload — a dispatch command reached CANCELLED
    /// (dispatch lifecycle; distinct from `DispatchSuperseded`).
    DispatchCancelledPayload
}
dispatch_payload! {
    /// `DispatchSuperseded` payload — a dispatch command reached SUPERSEDED
    /// (dispatch lifecycle; distinct from `DispatchCancelled`).
    DispatchSupersededPayload
}
dispatch_payload! {
    /// `DispatchFallbackActivated` payload — a dispatch command reached
    /// FALLBACK (dispatch lifecycle).
    DispatchFallbackActivatedPayload
}

/// `PluginActivated` payload — a plugin was activated (plugins; ASSUMPTION:
/// `plugin_id` field, see module header).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginActivatedPayload {
    pub correlation_id: FlexUuid,
    #[serde(with = "serde_ids::tenant_id")]
    pub tenant_id: TenantId,
    pub plugin_id: FlexUuid,
    pub event_time: EventTime,
}

/// `PluginFailed` payload — a plugin failed (plugins; ASSUMPTION:
/// `plugin_id` field, see module header).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginFailedPayload {
    pub correlation_id: FlexUuid,
    #[serde(with = "serde_ids::tenant_id")]
    pub tenant_id: TenantId,
    pub plugin_id: FlexUuid,
    pub event_time: EventTime,
}

/// `OperatorActionRecorded` payload — an operator action was recorded
/// (audit). Actor, action, and evidence details belong to the audit record
/// (§10.1, P1.10); the event carries identity and time only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperatorActionRecordedPayload {
    pub correlation_id: FlexUuid,
    #[serde(with = "serde_ids::tenant_id")]
    pub tenant_id: TenantId,
    pub event_time: EventTime,
}

// --- The tagged enum --------------------------------------------------------

/// The 26 initial domain events (report §15.3, P0.14), in report order.
///
/// Each variant records a **fact that occurred**, never an imperative: the
/// payload structs describe what happened. Every dispatch-lifecycle event
/// name corresponds to a frozen command state (P0.11) with the `Dispatch`
/// prefix; no event exists for VALIDATED or FEASIBLE (no `DispatchValidated`,
/// `DispatchFeasible`) and none may be invented.
///
/// Wire format: internally tagged with `"event_type"` whose value is the
/// SCREAMING_SNAKE_CASE rendering of the frozen name (for example
/// `"ASSET_REGISTERED"`). The frozen PascalCase spelling is available via
/// [`event_name`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event_type", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DomainEvent {
    /// `AssetRegistered` — an asset was registered.
    AssetRegistered(AssetRegisteredPayload),
    /// `AssetCapabilityUpdated` — an asset's capability set was updated.
    AssetCapabilityUpdated(AssetCapabilityUpdatedPayload),
    /// `TelemetryReceived` — telemetry was received.
    TelemetryReceived(TelemetryReceivedPayload),
    /// `TelemetryQualityChanged` — a telemetry value's quality state changed.
    TelemetryQualityChanged(TelemetryQualityChangedPayload),
    /// `AssetAvailabilityChanged` — an asset's availability changed.
    AssetAvailabilityChanged(AssetAvailabilityChangedPayload),
    /// `FlexibilityEnvelopeUpdated` — a flexibility envelope was updated.
    FlexibilityEnvelopeUpdated(FlexibilityEnvelopeUpdatedPayload),
    /// `ForecastPublished` — a forecast was published.
    ForecastPublished(ForecastPublishedPayload),
    /// `MarketSignalReceived` — a market signal was received.
    MarketSignalReceived(MarketSignalReceivedPayload),
    /// `ScheduleProposed` — a schedule was proposed.
    ScheduleProposed(ScheduleProposedPayload),
    /// `ScheduleValidated` — a schedule was validated.
    ScheduleValidated(ScheduleValidatedPayload),
    /// `FeasibilityCheckCompleted` — a feasibility check completed.
    FeasibilityCheckCompleted(FeasibilityCheckCompletedPayload),
    /// `DispatchProposed` — a dispatch command reached PROPOSED.
    DispatchProposed(DispatchProposedPayload),
    /// `DispatchApproved` — a dispatch command reached APPROVED.
    DispatchApproved(DispatchApprovedPayload),
    /// `DispatchRejected` — a dispatch command reached REJECTED.
    DispatchRejected(DispatchRejectedPayload),
    /// `DispatchIssued` — a dispatch command reached ISSUED. A fact about
    /// issue, never the command itself.
    DispatchIssued(DispatchIssuedPayload),
    /// `DispatchAcknowledged` — a dispatch command reached ACKNOWLEDGED.
    DispatchAcknowledged(DispatchAcknowledgedPayload),
    /// `DispatchTrackingUpdated` — a dispatch command in TRACKING reported
    /// updated measurements.
    DispatchTrackingUpdated(DispatchTrackingUpdatedPayload),
    /// `DispatchCompleted` — a dispatch command reached COMPLETED.
    DispatchCompleted(DispatchCompletedPayload),
    /// `DispatchFailed` — a dispatch command reached FAILED.
    DispatchFailed(DispatchFailedPayload),
    /// `DispatchExpired` — a dispatch command reached EXPIRED.
    DispatchExpired(DispatchExpiredPayload),
    /// `DispatchCancelled` — a dispatch command reached CANCELLED.
    DispatchCancelled(DispatchCancelledPayload),
    /// `DispatchSuperseded` — a dispatch command reached SUPERSEDED.
    DispatchSuperseded(DispatchSupersededPayload),
    /// `DispatchFallbackActivated` — a dispatch command reached FALLBACK.
    DispatchFallbackActivated(DispatchFallbackActivatedPayload),
    /// `PluginActivated` — a plugin was activated.
    PluginActivated(PluginActivatedPayload),
    /// `PluginFailed` — a plugin failed.
    PluginFailed(PluginFailedPayload),
    /// `OperatorActionRecorded` — an operator action was recorded.
    OperatorActionRecorded(OperatorActionRecordedPayload),
}

/// The number of frozen events (report §15.3).
pub const EVENT_COUNT: usize = 26;

/// The exact frozen event names (report §15.3 spelling), in report order.
///
/// The names are the contract; documentation, audit trails, and any
/// non-serde representation must carry these spellings verbatim.
pub const FROZEN_EVENT_NAMES: [&str; EVENT_COUNT] = [
    "AssetRegistered",
    "AssetCapabilityUpdated",
    "TelemetryReceived",
    "TelemetryQualityChanged",
    "AssetAvailabilityChanged",
    "FlexibilityEnvelopeUpdated",
    "ForecastPublished",
    "MarketSignalReceived",
    "ScheduleProposed",
    "ScheduleValidated",
    "FeasibilityCheckCompleted",
    "DispatchProposed",
    "DispatchApproved",
    "DispatchRejected",
    "DispatchIssued",
    "DispatchAcknowledged",
    "DispatchTrackingUpdated",
    "DispatchCompleted",
    "DispatchFailed",
    "DispatchExpired",
    "DispatchCancelled",
    "DispatchSuperseded",
    "DispatchFallbackActivated",
    "PluginActivated",
    "PluginFailed",
    "OperatorActionRecorded",
];

impl DomainEvent {
    /// One representative instance of every frozen event, in report §15.3
    /// order (nil/zero identifiers — the shape, not the data, matters).
    pub fn all_variants() -> [DomainEvent; EVENT_COUNT] {
        let event_time = EventTime::from_unix_millis(1_700_000_000_000)
            .expect("1_700_000_000_000 ms is a valid UTC timestamp");
        let tenant = TenantId::new(FlexUuid::nil());
        let portfolio = PortfolioId::new(FlexUuid::nil());
        let site = SiteId::new(FlexUuid::nil());
        let asset = AssetId::new(FlexUuid::nil());
        let correlation = FlexUuid::nil();
        [
            DomainEvent::AssetRegistered(AssetRegisteredPayload {
                correlation_id: correlation,
                tenant_id: tenant,
                portfolio_id: portfolio,
                site_id: site,
                asset_id: asset,
                event_time,
            }),
            DomainEvent::AssetCapabilityUpdated(AssetCapabilityUpdatedPayload {
                correlation_id: correlation,
                tenant_id: tenant,
                portfolio_id: portfolio,
                site_id: site,
                asset_id: asset,
                event_time,
            }),
            DomainEvent::TelemetryReceived(TelemetryReceivedPayload {
                correlation_id: correlation,
                tenant_id: tenant,
                site_id: site,
                asset_id: asset,
                event_time,
            }),
            DomainEvent::TelemetryQualityChanged(TelemetryQualityChangedPayload {
                correlation_id: correlation,
                tenant_id: tenant,
                site_id: site,
                asset_id: asset,
                quality: TelemetryQuality::Estimated,
                previous_quality: Some(TelemetryQuality::Good),
                event_time,
            }),
            DomainEvent::AssetAvailabilityChanged(AssetAvailabilityChangedPayload {
                correlation_id: correlation,
                tenant_id: tenant,
                site_id: site,
                asset_id: asset,
                event_time,
            }),
            DomainEvent::FlexibilityEnvelopeUpdated(FlexibilityEnvelopeUpdatedPayload {
                correlation_id: correlation,
                tenant_id: tenant,
                portfolio_id: portfolio,
                site_id: site,
                asset_id: asset,
                event_time,
            }),
            DomainEvent::ForecastPublished(ForecastPublishedPayload {
                correlation_id: correlation,
                tenant_id: tenant,
                portfolio_id: portfolio,
                site_id: site,
                asset_id: asset,
                event_time,
            }),
            DomainEvent::MarketSignalReceived(MarketSignalReceivedPayload {
                correlation_id: correlation,
                tenant_id: tenant,
                portfolio_id: portfolio,
                event_time,
            }),
            DomainEvent::ScheduleProposed(ScheduleProposedPayload {
                correlation_id: correlation,
                tenant_id: tenant,
                portfolio_id: portfolio,
                site_id: site,
                asset_id: asset,
                event_time,
            }),
            DomainEvent::ScheduleValidated(ScheduleValidatedPayload {
                correlation_id: correlation,
                tenant_id: tenant,
                portfolio_id: portfolio,
                site_id: site,
                asset_id: asset,
                event_time,
            }),
            DomainEvent::FeasibilityCheckCompleted(FeasibilityCheckCompletedPayload {
                correlation_id: correlation,
                tenant_id: tenant,
                portfolio_id: portfolio,
                site_id: site,
                asset_id: asset,
                event_time,
            }),
            DomainEvent::DispatchProposed(DispatchProposedPayload {
                correlation_id: correlation,
                tenant_id: tenant,
                portfolio_id: portfolio,
                site_id: site,
                asset_id: asset,
                command_id: FlexUuid::nil(),
                event_time,
            }),
            DomainEvent::DispatchApproved(DispatchApprovedPayload {
                correlation_id: correlation,
                tenant_id: tenant,
                portfolio_id: portfolio,
                site_id: site,
                asset_id: asset,
                command_id: FlexUuid::nil(),
                event_time,
            }),
            DomainEvent::DispatchRejected(DispatchRejectedPayload {
                correlation_id: correlation,
                tenant_id: tenant,
                portfolio_id: portfolio,
                site_id: site,
                asset_id: asset,
                command_id: FlexUuid::nil(),
                event_time,
            }),
            DomainEvent::DispatchIssued(DispatchIssuedPayload {
                correlation_id: correlation,
                tenant_id: tenant,
                portfolio_id: portfolio,
                site_id: site,
                asset_id: asset,
                command_id: FlexUuid::nil(),
                event_time,
            }),
            DomainEvent::DispatchAcknowledged(DispatchAcknowledgedPayload {
                correlation_id: correlation,
                tenant_id: tenant,
                portfolio_id: portfolio,
                site_id: site,
                asset_id: asset,
                command_id: FlexUuid::nil(),
                event_time,
            }),
            DomainEvent::DispatchTrackingUpdated(DispatchTrackingUpdatedPayload {
                correlation_id: correlation,
                tenant_id: tenant,
                portfolio_id: portfolio,
                site_id: site,
                asset_id: asset,
                command_id: FlexUuid::nil(),
                event_time,
            }),
            DomainEvent::DispatchCompleted(DispatchCompletedPayload {
                correlation_id: correlation,
                tenant_id: tenant,
                portfolio_id: portfolio,
                site_id: site,
                asset_id: asset,
                command_id: FlexUuid::nil(),
                event_time,
            }),
            DomainEvent::DispatchFailed(DispatchFailedPayload {
                correlation_id: correlation,
                tenant_id: tenant,
                portfolio_id: portfolio,
                site_id: site,
                asset_id: asset,
                command_id: FlexUuid::nil(),
                event_time,
            }),
            DomainEvent::DispatchExpired(DispatchExpiredPayload {
                correlation_id: correlation,
                tenant_id: tenant,
                portfolio_id: portfolio,
                site_id: site,
                asset_id: asset,
                command_id: FlexUuid::nil(),
                event_time,
            }),
            DomainEvent::DispatchCancelled(DispatchCancelledPayload {
                correlation_id: correlation,
                tenant_id: tenant,
                portfolio_id: portfolio,
                site_id: site,
                asset_id: asset,
                command_id: FlexUuid::nil(),
                event_time,
            }),
            DomainEvent::DispatchSuperseded(DispatchSupersededPayload {
                correlation_id: correlation,
                tenant_id: tenant,
                portfolio_id: portfolio,
                site_id: site,
                asset_id: asset,
                command_id: FlexUuid::nil(),
                event_time,
            }),
            DomainEvent::DispatchFallbackActivated(DispatchFallbackActivatedPayload {
                correlation_id: correlation,
                tenant_id: tenant,
                portfolio_id: portfolio,
                site_id: site,
                asset_id: asset,
                command_id: FlexUuid::nil(),
                event_time,
            }),
            DomainEvent::PluginActivated(PluginActivatedPayload {
                correlation_id: correlation,
                tenant_id: tenant,
                plugin_id: FlexUuid::nil(),
                event_time,
            }),
            DomainEvent::PluginFailed(PluginFailedPayload {
                correlation_id: correlation,
                tenant_id: tenant,
                plugin_id: FlexUuid::nil(),
                event_time,
            }),
            DomainEvent::OperatorActionRecorded(OperatorActionRecordedPayload {
                correlation_id: correlation,
                tenant_id: tenant,
                event_time,
            }),
        ]
    }
}

/// Returns the exact frozen event name (report §15.3 PascalCase spelling).
pub fn event_name(event: &DomainEvent) -> &'static str {
    match event {
        DomainEvent::AssetRegistered(_) => "AssetRegistered",
        DomainEvent::AssetCapabilityUpdated(_) => "AssetCapabilityUpdated",
        DomainEvent::TelemetryReceived(_) => "TelemetryReceived",
        DomainEvent::TelemetryQualityChanged(_) => "TelemetryQualityChanged",
        DomainEvent::AssetAvailabilityChanged(_) => "AssetAvailabilityChanged",
        DomainEvent::FlexibilityEnvelopeUpdated(_) => "FlexibilityEnvelopeUpdated",
        DomainEvent::ForecastPublished(_) => "ForecastPublished",
        DomainEvent::MarketSignalReceived(_) => "MarketSignalReceived",
        DomainEvent::ScheduleProposed(_) => "ScheduleProposed",
        DomainEvent::ScheduleValidated(_) => "ScheduleValidated",
        DomainEvent::FeasibilityCheckCompleted(_) => "FeasibilityCheckCompleted",
        DomainEvent::DispatchProposed(_) => "DispatchProposed",
        DomainEvent::DispatchApproved(_) => "DispatchApproved",
        DomainEvent::DispatchRejected(_) => "DispatchRejected",
        DomainEvent::DispatchIssued(_) => "DispatchIssued",
        DomainEvent::DispatchAcknowledged(_) => "DispatchAcknowledged",
        DomainEvent::DispatchTrackingUpdated(_) => "DispatchTrackingUpdated",
        DomainEvent::DispatchCompleted(_) => "DispatchCompleted",
        DomainEvent::DispatchFailed(_) => "DispatchFailed",
        DomainEvent::DispatchExpired(_) => "DispatchExpired",
        DomainEvent::DispatchCancelled(_) => "DispatchCancelled",
        DomainEvent::DispatchSuperseded(_) => "DispatchSuperseded",
        DomainEvent::DispatchFallbackActivated(_) => "DispatchFallbackActivated",
        DomainEvent::PluginActivated(_) => "PluginActivated",
        DomainEvent::PluginFailed(_) => "PluginFailed",
        DomainEvent::OperatorActionRecorded(_) => "OperatorActionRecorded",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screaming_snake(name: &str) -> String {
        let mut out = String::with_capacity(name.len() + 8);
        for (i, c) in name.chars().enumerate() {
            if c.is_ascii_uppercase() {
                if i > 0 {
                    out.push('_');
                }
                out.push(c);
            } else {
                out.push(c.to_ascii_uppercase());
            }
        }
        out
    }

    #[test]
    fn exactly_26_events() {
        // P1.09 acceptance: a count test asserts 26.
        assert_eq!(DomainEvent::all_variants().len(), 26);
        assert_eq!(EVENT_COUNT, 26);
        assert_eq!(FROZEN_EVENT_NAMES.len(), 26);
    }

    #[test]
    fn event_names_match_frozen_spelling() {
        for (event, expected) in DomainEvent::all_variants().iter().zip(FROZEN_EVENT_NAMES.iter())
        {
            assert_eq!(event_name(event), *expected, "event spelling must match §15.3 verbatim");
        }
    }

    #[test]
    fn all_names_are_distinct() {
        for (i, a) in FROZEN_EVENT_NAMES.iter().enumerate() {
            for b in FROZEN_EVENT_NAMES.iter().skip(i + 1) {
                assert_ne!(a, b, "frozen event names must be unique");
            }
        }
    }

    #[test]
    fn wire_tag_is_the_screaming_snake_frozen_name() {
        for event in DomainEvent::all_variants().iter() {
            let json = serde_json::to_value(event).unwrap();
            let tag = json
                .get("event_type")
                .and_then(|v| v.as_str())
                .unwrap_or_else(|| panic!("event {} must carry the event_type tag", event_name(event)));
            assert_eq!(tag, screaming_snake(event_name(event)));
        }
    }

    #[test]
    fn every_event_round_trips_through_serde_json() {
        // P1.09 acceptance: round-trip serialization tests for every event.
        for event in DomainEvent::all_variants().iter() {
            let json = serde_json::to_string(event).unwrap_or_else(|e| {
                panic!("{} failed to serialize: {e}", event_name(event))
            });
            let back: DomainEvent = serde_json::from_str(&json)
                .unwrap_or_else(|e| panic!("{} failed to deserialize: {e}", event_name(event)));
            assert_eq!(back, *event, "{} must round-trip identically", event_name(event));
        }
    }

    #[test]
    fn telemetry_quality_changed_carries_the_frozen_quality_names() {
        let events = DomainEvent::all_variants();
        let event = &events[3]; // TelemetryQualityChanged, report order.
        assert_eq!(event_name(event), "TelemetryQualityChanged");
        let json = serde_json::to_value(event).unwrap();
        assert_eq!(json["quality"], serde_json::json!("ESTIMATED"));
        let back: DomainEvent = serde_json::from_value(json).unwrap();
        assert_eq!(back, *event);
    }

    #[test]
    fn telemetry_quality_changed_carries_previous_and_new_states() {
        // P2.16 step 2: the payload carries BOTH the previous and the new
        // quality state, and both round-trip through the wire format.
        let events = DomainEvent::all_variants();
        let event = &events[3];
        let json = serde_json::to_value(event).unwrap();
        assert_eq!(json["quality"], serde_json::json!("ESTIMATED"));
        assert_eq!(json["previous_quality"], serde_json::json!("GOOD"));
        let back: DomainEvent = serde_json::from_value(json).unwrap();
        match back {
            DomainEvent::TelemetryQualityChanged(payload) => {
                assert_eq!(payload.quality, TelemetryQuality::Estimated);
                assert_eq!(payload.previous_quality, Some(TelemetryQuality::Good));
            }
            other => panic!("expected the TelemetryQualityChanged variant, got {other:?}"),
        }
    }

    #[test]
    fn telemetry_quality_changed_parses_a_v1_envelope_without_the_previous_state() {
        // The P2.16 previous_quality field is ADDITIVE with a serde default:
        // an envelope produced before the revision (no previous_quality
        // field) still parses — backward compatibility without a schema
        // version bump (P1.09: only a BREAKING change requires one).
        let mut json = serde_json::to_value(DomainEvent::all_variants()[3].clone()).unwrap();
        json.as_object_mut().unwrap().remove("previous_quality");
        let back: DomainEvent = serde_json::from_value(json).unwrap();
        match back {
            DomainEvent::TelemetryQualityChanged(payload) => {
                assert_eq!(payload.quality, TelemetryQuality::Estimated);
                assert_eq!(payload.previous_quality, None);
            }
            other => panic!("expected the TelemetryQualityChanged variant, got {other:?}"),
        }
    }

    #[test]
    fn typed_ids_serialize_as_hyphenated_uuid_strings() {
        let events = DomainEvent::all_variants();
        let event = &events[0]; // AssetRegistered.
        let json = serde_json::to_value(event).unwrap();
        let tenant = json["tenant_id"].as_str().expect("tenant_id is a string");
        assert_eq!(FlexUuid::parse(tenant).unwrap(), FlexUuid::nil());
        assert_eq!(json["asset_id"].as_str().unwrap(), FlexUuid::nil().to_string());
    }

    #[test]
    fn dispatch_events_carry_the_command_id() {
        for event in DomainEvent::all_variants().iter() {
            let name = event_name(event);
            if name.starts_with("Dispatch") {
                assert_ne!(name, "DispatchValidated");
                assert_ne!(name, "DispatchFeasible");
                let json = serde_json::to_value(event).unwrap();
                let command = json["command_id"].as_str().expect(
                    "every dispatch-lifecycle payload carries the command_id",
                );
                assert_eq!(FlexUuid::parse(command).unwrap(), FlexUuid::nil());
            }
        }
    }

    #[test]
    fn serde_rejects_an_unknown_event_type_tag() {
        // Events are a closed frozen vocabulary: an invented event type or
        // an unknown tag spelling never deserializes.
        assert!(serde_json::from_value::<DomainEvent>(serde_json::json!({
            "event_type": "DISPATCH_VALIDATED",
        }))
        .is_err());
        assert!(serde_json::from_value::<DomainEvent>(serde_json::json!({
            "event_type": "NOT_A_REAL_EVENT",
        }))
        .is_err());
    }
}
