// P1.15 — JetStream topology constants, subject→stream mapping, and
// provisioning functions implementing the P1.14 §3/§4/§5 design verbatim.
//
// What lives here (P1.14 is the normative source; this file implements it):
//
// - The P1.14 §3 subject scheme D-J1: `flexcore.v1.<area>.<event_slug>` —
//   one event, exactly one subject; lower_snake_case slugs; **no tenant
//   identifier in any subject** (D-J2: tenant scoping happens at
//   consumption, from the payload, under TenantContext rules — subject
//   names are enumerable by any connected client and would otherwise leak
//   tenant existence and volume).
// - The P1.14 §4.2 stream inventory. Phase 1 provisions exactly four
//   streams (D-J7): `FLEXCORE_REGISTRY`, `FLEXCORE_STATE`, `FLEXCORE_AUDIT`,
//   `FLEXCORE_DLQ`. The remaining streams have fixed names and subject
//   filters and are created by the phase that first publishes into them.
// - The P1.14 §4.1 per-stream policy shape: `Limits` retention (D-J4 —
//   replayable within the configured window), `File` storage (durable
//   across restarts), `DiscardOld`, replication 1 (D-J5 — clustering is a
//   deployment decision, never a Phase 1 default). The concrete
//   max-age/max-msgs/max-bytes/duplicate-window values are **deployment
//   configuration** ([`DeploymentLimits`]), never design values.
// - Durable pull consumers with `AckExplicit` (P1.14 §5.1): durable names
//   follow the `<area>-<purpose>-v1` convention; `AckWait`, `MaxAckPending`
//   and `MaxDeliver` are deployment configuration (§5.1). Acks happen only
//   after the processing transaction commits — enforced by
//   [`crate::consumer`], not by provisioning.
//
// DLQ subjects sit **outside** the `flexcore.v1` namespace
// (`flexcore.dlq.v1.<area>.<event_slug>`, P1.14 §4.2) so a wildcard on
// `flexcore.v1.>` can never re-consume a poison message.

use std::time::Duration;

use async_nats::jetstream::consumer::{AckPolicy, DeliverPolicy, ReplayPolicy};
use async_nats::jetstream::context::CreateStreamError;
// `ConsumerError` (the `create_consumer_on_stream` error type) is a type
// alias under `jetstream::stream` in async-nats 0.50.0 — VERIFY-BUILD
// checked against the 0.50.0 docs.
use async_nats::jetstream::stream::{Config as StreamConfig, ConsumerError, StorageType};
use async_nats::jetstream::{consumer, Context};

/// The versioned event-subject prefix (P1.14 §3, D-J1).
pub const EVENT_SUBJECT_PREFIX: &str = "flexcore.v1";

/// The quarantined-copy subject prefix (P1.14 §4.2), deliberately outside
/// `flexcore.v1` so event-subject wildcards never re-consume poison
/// messages.
pub const DLQ_SUBJECT_PREFIX: &str = "flexcore.dlq.v1";

// --- Stream names (P1.14 §4.2) ----------------------------------------------

pub const STREAM_REGISTRY: &str = "FLEXCORE_REGISTRY";
pub const STREAM_STATE: &str = "FLEXCORE_STATE";
pub const STREAM_TELEMETRY: &str = "FLEXCORE_TELEMETRY";
pub const STREAM_FLEXIBILITY: &str = "FLEXCORE_FLEXIBILITY";
pub const STREAM_FORECAST: &str = "FLEXCORE_FORECAST";
pub const STREAM_MARKET: &str = "FLEXCORE_MARKET";
pub const STREAM_SCHEDULING: &str = "FLEXCORE_SCHEDULING";
pub const STREAM_VALIDATION: &str = "FLEXCORE_VALIDATION";
pub const STREAM_DISPATCH: &str = "FLEXCORE_DISPATCH";
pub const STREAM_PLUGINS: &str = "FLEXCORE_PLUGINS";
pub const STREAM_AUDIT: &str = "FLEXCORE_AUDIT";
pub const STREAM_DLQ: &str = "FLEXCORE_DLQ";

/// One P1.14 §4.2 stream row: name + subject filter. The retention/storage/
/// discard/replication policy is shared ([`provision_stream`]); the limits
/// are deployment configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamDefinition {
    /// Stream name (P1.14 §4.2 column 1).
    pub name: &'static str,
    /// Subject filter (P1.14 §4.2 column 2).
    pub subject_filter: &'static str,
    /// Whether D-J7 schedules this stream for Phase 1 provisioning.
    pub provisioned_in_phase_1: bool,
}

/// The complete P1.14 §4.2 inventory — all twelve rows, in table order.
/// Every §15.3 event area has exactly one stream; the DLQ row covers the
/// quarantine copies of poison messages from any stream.
pub const STREAMS: &[StreamDefinition] = &[
    StreamDefinition {
        name: STREAM_REGISTRY,
        subject_filter: "flexcore.v1.registry.>",
        provisioned_in_phase_1: true,
    },
    StreamDefinition {
        name: STREAM_STATE,
        subject_filter: "flexcore.v1.state.>",
        provisioned_in_phase_1: true,
    },
    StreamDefinition {
        name: STREAM_TELEMETRY,
        subject_filter: "flexcore.v1.telemetry.>",
        provisioned_in_phase_1: false,
    },
    StreamDefinition {
        name: STREAM_FLEXIBILITY,
        subject_filter: "flexcore.v1.flexibility.>",
        provisioned_in_phase_1: false,
    },
    StreamDefinition {
        name: STREAM_FORECAST,
        subject_filter: "flexcore.v1.forecast.>",
        provisioned_in_phase_1: false,
    },
    StreamDefinition {
        name: STREAM_MARKET,
        subject_filter: "flexcore.v1.market.>",
        provisioned_in_phase_1: false,
    },
    StreamDefinition {
        name: STREAM_SCHEDULING,
        subject_filter: "flexcore.v1.scheduling.>",
        provisioned_in_phase_1: false,
    },
    StreamDefinition {
        name: STREAM_VALIDATION,
        subject_filter: "flexcore.v1.validation.>",
        provisioned_in_phase_1: false,
    },
    StreamDefinition {
        name: STREAM_DISPATCH,
        subject_filter: "flexcore.v1.dispatch.>",
        provisioned_in_phase_1: false,
    },
    StreamDefinition {
        name: STREAM_PLUGINS,
        subject_filter: "flexcore.v1.plugins.>",
        provisioned_in_phase_1: false,
    },
    StreamDefinition {
        name: STREAM_AUDIT,
        subject_filter: "flexcore.v1.audit.>",
        provisioned_in_phase_1: true,
    },
    StreamDefinition {
        name: STREAM_DLQ,
        subject_filter: "flexcore.dlq.v1.>",
        provisioned_in_phase_1: true,
    },
];

/// The four streams D-J7 provisions in Phase 1 (plus the DLQ).
pub const PHASE_1_STREAMS: &[&StreamDefinition] = &[
    &STREAMS[0],
    &STREAMS[1],
    &STREAMS[10],
    &STREAMS[11],
];

/// Maps a concrete subject to the P1.14 §4.2 stream that captures it.
///
/// Scheme (D-J1): `flexcore.v1.<area>.<event_slug>` with the P1.14 §3 area
/// tokens — `registry`, `telemetry`, `state`, `flexibility`, `forecast`,
/// `market`, `scheduling`, `validation`, `dispatch`, `plugins`, `audit`.
/// Quarantine copies (`flexcore.dlq.v1.…`) map to [`STREAM_DLQ`]. Returns
/// `None` for anything the P1.14 design does not route — the publisher
/// publishes the outbox `destination_subject` verbatim regardless; this
/// mapping exists for provisioning, tests, and operations.
///
/// See the crate README for the recorded verification result on the
/// flexcore-events area tokens (8 of 26 events diverge from this table and
/// must be reconciled there — the publisher does not remap).
pub fn stream_for_subject(subject: &str) -> Option<&'static str> {
    if subject.starts_with(DLQ_SUBJECT_PREFIX) {
        return Some(STREAM_DLQ);
    }
    let mut tokens = subject.split('.');
    let root = tokens.next()?;
    let version = tokens.next()?;
    let area = tokens.next()?;
    if root != "flexcore" || version != "v1" {
        return None;
    }
    Some(match area {
        "registry" => STREAM_REGISTRY,
        "state" => STREAM_STATE,
        "telemetry" => STREAM_TELEMETRY,
        "flexibility" => STREAM_FLEXIBILITY,
        "forecast" => STREAM_FORECAST,
        "market" => STREAM_MARKET,
        "scheduling" => STREAM_SCHEDULING,
        "validation" => STREAM_VALIDATION,
        "dispatch" => STREAM_DISPATCH,
        "plugins" => STREAM_PLUGINS,
        "audit" => STREAM_AUDIT,
        _ => return None,
    })
}

/// Derives the P1.14 §4.2 DLQ subject for a quarantined original:
/// `flexcore.dlq.v1.<area>.<event_slug>` (the last two tokens of a
/// conforming `flexcore.v1.<area>.<event_slug>` original). Returns `None`
/// for a subject the P1.14 scheme does not recognise — the caller must then
/// refuse to quarantine silently and surface the error loudly (the
/// poison-message rule: never silently drop).
pub fn dlq_subject_for(original_subject: &str) -> Option<String> {
    let tokens: Vec<&str> = original_subject.split('.').collect();
    if tokens.len() < 4 || tokens[0] != "flexcore" || tokens[1] != "v1" {
        return None;
    }
    Some(format!("{}.{}.{}", DLQ_SUBJECT_PREFIX, tokens[2], tokens[3]))
}

/// Deployment-configured stream limits (P1.14 §4.1/D-J4: retention and the
/// duplicate window are deployment configuration, never design values; the
/// Compose dev stack sets explicit dev-only values, production values are a
/// per-deployment decision per data class).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeploymentLimits {
    /// `Nats-Msg-Id` deduplication window. Broker-side dedup applies **only
    /// within this window** (P1.14 §2); the durable guarantee is the outbox
    /// plus the consumer inbox, never the window.
    pub duplicate_window: Duration,
    /// Maximum message age (`max_age`); `None` leaves the server default.
    pub max_age: Option<Duration>,
    /// Maximum messages per stream (`max_messages`); `None` leaves the
    /// server default.
    pub max_messages: Option<i64>,
    /// Maximum stream bytes (`max_bytes`); `None` leaves the server default.
    pub max_bytes: Option<i64>,
}

/// Durable pull-consumer policy (P1.14 §5.1). `AckWait`, `MaxAckPending`
/// and `MaxDeliver` are deployment configuration — no universal values are
/// fixed by the design.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsumerPolicy {
    /// Durable name, `<area>-<purpose>-v1` (e.g. `audit-persist-v1`).
    /// Durable consumers only for anything with side effects (P1.14 §5.1);
    /// ephemeral consumers are for debugging and must never do stateful
    /// work.
    pub durable_name: String,
    /// The stream to consume from (a [`STREAMS`] name).
    pub stream: String,
    /// Subject filter inside the stream (supports wildcards).
    pub filter_subject: String,
    /// How long a delivered message may stay unacknowledged before
    /// redelivery.
    pub ack_wait: Duration,
    /// Redelivery budget; after it is exhausted the consumer's poison path
    /// routes the message to the DLQ (P1.14 §5.2) — never a silent drop.
    pub max_deliver: i64,
    /// Maximum unacknowledged in-flight messages.
    pub max_ack_pending: i64,
}

/// Provisioning failures. The message carries the upstream error's own
/// `Display` for operator logs — this is an infrastructure adapter, not an
/// API surface, so no envelope rule applies here (the API's never-leak rule
/// is the control plane's, P1.17).
#[derive(Debug)]
pub enum ProvisionError {
    /// `get_or_create_stream` failed for the named stream.
    CreateStream {
        stream: &'static str,
        message: String,
    },
    /// Consumer creation failed for the named durable consumer.
    CreateConsumer { durable_name: String, message: String },
}

impl std::fmt::Display for ProvisionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProvisionError::CreateStream { stream, message } => {
                write!(f, "provisioning stream {stream} failed: {message}")
            }
            ProvisionError::CreateConsumer { durable_name, message } => {
                write!(f, "provisioning consumer {durable_name} failed: {message}")
            }
        }
    }
}

impl std::error::Error for ProvisionError {}

// VERIFY-BUILD: `get_or_create_stream` is documented for async-nats 0.50.0
// as `Result<Stream, CreateStreamError>` (no tuple) and `create_consumer_on_stream`
// as the create-or-update entry point taking `(config, stream_name)`; the
// `pull::Config` field types (`max_deliver: i64`, `max_ack_pending: i64`,
// `ack_wait: Duration`) come from the 0.50.0 docs. Re-check all three at
// the first `cargo build`; adjust mechanically if the signatures moved.

/// Creates one stream with the P1.14 §4.1 policy shape (create-or-update:
/// re-provisioning converges the policy; already-converged calls are
/// no-ops on the server).
pub async fn provision_stream(
    jetstream: &Context,
    definition: &StreamDefinition,
    limits: &DeploymentLimits,
) -> Result<(), ProvisionError> {
    let config = StreamConfig {
        name: definition.name.to_string(),
        subjects: vec![definition.subject_filter.to_string()],
        // D-J4: Limits retention — replayable within the configured window.
        retention: async_nats::jetstream::stream::RetentionPolicy::Limits,
        // P1.14 §4.1: durable across restarts.
        storage: StorageType::File,
        // P1.14 §4.1: oldest first when a limit is hit.
        discard: async_nats::jetstream::stream::DiscardPolicy::Old,
        // D-J5: single node in Phase 1; clustering is a deployment decision.
        num_replicas: 1,
        duplicate_window: limits.duplicate_window,
        max_age: limits.max_age.unwrap_or_default(),
        max_messages: limits.max_messages.unwrap_or(-1),
        max_bytes: limits.max_bytes.unwrap_or(-1),
        ..Default::default()
    };
    jetstream
        .get_or_create_stream(config)
        .await
        .map_err(|e: CreateStreamError| ProvisionError::CreateStream {
            stream: definition.name,
            message: e.to_string(),
        })?;
    Ok(())
}

/// Provisions the four Phase 1 streams (D-J7) with the given deployment
/// limits. Idempotent: safe to call on every worker start (P1.19).
pub async fn provision_phase_1_streams(
    jetstream: &Context,
    limits: &DeploymentLimits,
) -> Result<(), ProvisionError> {
    for definition in PHASE_1_STREAMS {
        provision_stream(jetstream, definition, limits).await?;
    }
    Ok(())
}

/// Creates (or updates) one durable pull consumer per the P1.14 §5.1
/// policy: `AckExplicit`, `DeliverPolicy::All`, `ReplayPolicy::Instant`
/// (default per §4.1; `ReplayOriginal` for review/replay scenarios is
/// selectable by editing the policy before provisioning, not at runtime).
/// Idempotent.
pub async fn provision_durable_pull_consumer(
    jetstream: &Context,
    policy: &ConsumerPolicy,
) -> Result<(), ProvisionError> {
    let config = consumer::pull::Config {
        durable_name: Some(policy.durable_name.clone()),
        // P1.14 §5.1: explicit acks only; ack-after-commit is enforced by
        // the consumer helper, never assumed here.
        ack_policy: AckPolicy::Explicit,
        deliver_policy: DeliverPolicy::All,
        replay_policy: ReplayPolicy::Instant,
        ack_wait: policy.ack_wait,
        max_deliver: policy.max_deliver,
        max_ack_pending: policy.max_ack_pending,
        filter_subject: policy.filter_subject.clone(),
        ..Default::default()
    };
    jetstream
        .create_consumer_on_stream(config, policy.stream.as_str())
        .await
        .map_err(|e: ConsumerError| ProvisionError::CreateConsumer {
            durable_name: policy.durable_name.clone(),
            message: e.to_string(),
        })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The P1.14 §3 mapping table, asserted row by row through
    /// [`stream_for_subject`]: one event, exactly one stream.
    #[test]
    fn subject_to_stream_mapping_matches_p1_14_table() {
        let rows = [
            ("flexcore.v1.registry.asset_registered", STREAM_REGISTRY),
            (
                "flexcore.v1.registry.asset_capability_updated",
                STREAM_REGISTRY,
            ),
            (
                "flexcore.v1.telemetry.telemetry_received",
                STREAM_TELEMETRY,
            ),
            (
                "flexcore.v1.telemetry.telemetry_quality_changed",
                STREAM_TELEMETRY,
            ),
            (
                "flexcore.v1.state.asset_availability_changed",
                STREAM_STATE,
            ),
            (
                "flexcore.v1.flexibility.flexibility_envelope_updated",
                STREAM_FLEXIBILITY,
            ),
            ("flexcore.v1.forecast.forecast_published", STREAM_FORECAST),
            (
                "flexcore.v1.market.market_signal_received",
                STREAM_MARKET,
            ),
            (
                "flexcore.v1.scheduling.schedule_proposed",
                STREAM_SCHEDULING,
            ),
            (
                "flexcore.v1.scheduling.schedule_validated",
                STREAM_SCHEDULING,
            ),
            (
                "flexcore.v1.validation.feasibility_check_completed",
                STREAM_VALIDATION,
            ),
            ("flexcore.v1.dispatch.dispatch_proposed", STREAM_DISPATCH),
            ("flexcore.v1.dispatch.dispatch_approved", STREAM_DISPATCH),
            ("flexcore.v1.dispatch.dispatch_rejected", STREAM_DISPATCH),
            ("flexcore.v1.dispatch.dispatch_issued", STREAM_DISPATCH),
            (
                "flexcore.v1.dispatch.dispatch_acknowledged",
                STREAM_DISPATCH,
            ),
            (
                "flexcore.v1.dispatch.dispatch_tracking_updated",
                STREAM_DISPATCH,
            ),
            ("flexcore.v1.dispatch.dispatch_completed", STREAM_DISPATCH),
            ("flexcore.v1.dispatch.dispatch_failed", STREAM_DISPATCH),
            ("flexcore.v1.dispatch.dispatch_expired", STREAM_DISPATCH),
            ("flexcore.v1.dispatch.dispatch_cancelled", STREAM_DISPATCH),
            (
                "flexcore.v1.dispatch.dispatch_superseded",
                STREAM_DISPATCH,
            ),
            (
                "flexcore.v1.dispatch.dispatch_fallback_activated",
                STREAM_DISPATCH,
            ),
            ("flexcore.v1.plugins.plugin_activated", STREAM_PLUGINS),
            ("flexcore.v1.plugins.plugin_failed", STREAM_PLUGINS),
            (
                "flexcore.v1.audit.operator_action_recorded",
                STREAM_AUDIT,
            ),
        ];
        assert_eq!(rows.len(), 26, "the P1.14 §3 table has 26 rows");
        for (subject, stream) in rows {
            assert_eq!(
                stream_for_subject(subject),
                Some(stream),
                "subject {subject} must map to {stream}"
            );
        }
    }

    #[test]
    fn dlq_and_unknown_subjects_map_correctly() {
        assert_eq!(
            stream_for_subject("flexcore.dlq.v1.registry.asset_registered"),
            Some(STREAM_DLQ)
        );
        assert_eq!(stream_for_subject("flexcore.v2.registry.asset_registered"), None);
        assert_eq!(stream_for_subject("flexcore.v1.unknown.event"), None);
        assert_eq!(stream_for_subject("other.v1.registry.asset_registered"), None);
    }

    #[test]
    fn dlq_subject_derivation_follows_p1_14() {
        // flexcore.dlq.v1.<area>.<event_slug> for the quarantined original.
        assert_eq!(
            dlq_subject_for("flexcore.v1.registry.asset_registered"),
            Some("flexcore.dlq.v1.registry.asset_registered".to_string())
        );
        // A non-conforming subject must be refused, never silently mapped.
        assert_eq!(dlq_subject_for("not-flexcore"), None);
        assert_eq!(dlq_subject_for("flexcore.v1.only-three"), None);
    }

    #[test]
    fn phase_1_streams_are_exactly_d_j7s_four() {
        let names: Vec<&str> = PHASE_1_STREAMS.iter().map(|d| d.name).collect();
        assert_eq!(
            names,
            vec![STREAM_REGISTRY, STREAM_STATE, STREAM_AUDIT, STREAM_DLQ]
        );
    }
}
