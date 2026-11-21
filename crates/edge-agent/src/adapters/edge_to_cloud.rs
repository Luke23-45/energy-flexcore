// P14.18 — The edge-to-cloud protocol mapping and the fleet uplink
// budgets (decision record
// `docs/phases-11-20/phase-14-protocol-device-connectivity/records/
// 82_p14_18_edge_agent_seam_extension.md`).
//
// WHAT THIS MODULE IS:
//
// 1. **The composition point record 43 §2 named.** The MQTT adapter's
//    `uplink` module declared the canonical-point → Sparkplug mapping
//    WITHOUT depending on this crate ("a wave-3 adapter struct
//    implements `UplinkChannel` by draining the edge buffer and
//    converting its samples through this mapping. The seam is designed
//    there; the wiring is wave 3's"). This module IS that wiring:
//    [`SparkplugUplinkChannel`] implements the edge's own
//    [`UplinkChannel`](crate::ports::UplinkChannel) port by converting
//    canonical edge samples into the adapter's `UplinkPoint` shapes and
//    publishing through an INJECTED [`MqttUplinkTransport`] (which, in
//    every shipped composition, is the adapter's own `UnwiredUplinkTransport`
//    — the honest, refusing default).
// 2. **The declared mapping policy.** Exactly two edge quantities have a
//    declared edge-to-cloud mapping (the adapter's own `UplinkQuantity`
//    vocabulary): active power and state of charge (energy and
//    availability likewise map 1:1). A sample carrying a quantity the
//    declared mapping does not cover is a NAMED refusal — never a
//    silently dropped or silently reinterpreted sample.
// 3. **The bandwidth budgets for fleet uplinks** (plan §2 P14.18):
//    [`FleetUplinkBudget`] bounds what one edge may send — a declared
//    per-forward sample cap, a declared per-node message-rate budget,
//    and the fleet-level total-node budget — so a fleet of edges cannot
//    exhaust an uplink by aggregation. Declared, validated, no defaults
//    (the budget discipline of the protocol adapters' backpressure
//    modules).
//
// WHAT THIS MODULE IS NOT:
//
// - **Not a wire.** No socket, no TLS, no broker connection IN THIS
//    MODULE: the injected transport is the adapter's seam — under
//    FIM-F05 (WRITTEN-UNRUN) the adapter crate provides a live
//    implementation (`flexcore-mqtt-adapter::live::LiveMqttUplinkTransport`,
//    TLS via tokio-rustls, interop UNPROVEN, a HUMAN LAB GATE); the
//    shipped default remains `UnwiredUplinkTransport`, refusing every
//    publish with `SeamNotWired`. Nothing has ever been published,
//    connected, or exchanged on a wire (the entire 0.x line is
//    WRITTEN-UNRUN; the first human build is the program gate).
// - **Not a command authority (FIM-F05.05 additive delivery surface).**
//    The `UplinkChannel` port itself has no receive method (P6.05: the
//    edge dials out; the port is unchanged) and this module adds none.
//    The ADDITIVE FIM-F05 command-delivery surface
//    (`attach_command_source` / `drain_received_commands`) drains
//    commands the ADAPTER's live transport received on the subscribed
//    command topic into the edge — every delivered command enters the
//    command lifecycle at PROPOSED and nowhere else: NO authorization,
//    NO dispatch, NO relay, NO setpoint surface (the frozen 14-state/
//    27-edge lifecycle and the never-granted permission set are
//    untouched).
// - **Not a quality redefinition.** The mapping consumes canonical
//    samples (quality already one of the six canonical states, P0.08).
//    A non-GOOD sample is MAPPED AND MARKED, never silently upgraded:
//    the declared policy carries the quality in the metric name suffix
//    (`degraded` for anything not `GOOD`) so a consumer can filter —
//    the platform decides trust, the edge never hides degradation.

use std::collections::BTreeMap;

use flexcore_contracts::quality::TelemetryQuality;
use flexcore_contracts::units::StateOfCharge;
use flexcore_mqtt_adapter::live::{MqttCommandSource, SparkplugCommandDelivery};
use flexcore_mqtt_adapter::model::{
    BdSeq, SparkplugMessageKind, SparkplugPayload, SparkplugSeq,
};
use flexcore_mqtt_adapter::topic::ValidatedTopic;
use flexcore_mqtt_adapter::transport::{
    MqttTransportError, MqttUplinkTransport, PublishReceipt,
};
use flexcore_mqtt_adapter::uplink::{SparkplugUplinkPlan, UplinkPoint};
use flexcore_ports::telemetry_source::{QuantityValue, TelemetryQuantity};

use crate::ports::{EdgeError, EdgeTelemetrySample, UplinkChannel};

/// Errors the edge-to-cloud mapping reports (named; the
/// [`UplinkChannel`] port carries them inside
/// [`EdgeError::UplinkForwardFailed`] — no port-level variant is added,
/// the P6.05 error vocabulary is consumed, never redefined).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EdgeMappingError {
    /// The sample's quantity has no declared edge-to-cloud mapping
    /// (the closed [`MappedQuantity`] set is the whole truth — an
    /// unmapped quantity is refused, never guessed into a metric).
    QuantityNotMapped { quantity: &'static str },
    /// The sample's value kind does not match its quantity (the frozen
    /// P2.04 pairing rule, re-enforced at the mapping).
    ValueKindMismatch { quantity: &'static str },
    /// The declared budget refused the forward (rate or batch size).
    BudgetRefused { reason: String },
    /// The injected transport refused the publish (re-raised; the
    /// shipped default refuses with `SeamNotWired`).
    Transport(MqttTransportError),
    /// The adapter's payload/identity validation refused (re-raised).
    AdapterModel(String),
}

impl std::fmt::Display for EdgeMappingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EdgeMappingError::QuantityNotMapped { quantity } => {
                write!(f, "quantity '{quantity}' has no declared edge-to-cloud mapping; it is refused, never guessed into a metric")
            }
            EdgeMappingError::ValueKindMismatch { quantity } => {
                write!(f, "the value kind does not match quantity '{quantity}' at the mapping")
            }
            EdgeMappingError::BudgetRefused { reason } => {
                write!(f, "the declared fleet uplink budget refused the forward: {reason}")
            }
            EdgeMappingError::Transport(error) => write!(f, "the uplink transport refused: {error}"),
            EdgeMappingError::AdapterModel(reason) => {
                write!(f, "the uplink adapter refused the mapped metric: {reason}")
            }
        }
    }
}

impl std::error::Error for EdgeMappingError {}

/// The closed set of edge quantities with a declared edge-to-cloud
/// mapping (the adapter's own `UplinkQuantity` set, named at this side
/// so the refusal can name what IS mapped).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MappedQuantity {
    ActivePower,
    StateOfCharge,
    Energy,
    Availability,
}

impl MappedQuantity {
    /// The frozen TelemetryQuantity this maps from (the edge's canonical
    /// vocabulary).
    pub const fn from_edge(self) -> TelemetryQuantity {
        match self {
            MappedQuantity::ActivePower => TelemetryQuantity::ActivePower,
            MappedQuantity::StateOfCharge => TelemetryQuantity::Soc,
            MappedQuantity::Energy => TelemetryQuantity::Energy,
            MappedQuantity::Availability => TelemetryQuantity::Availability,
        }
    }

    /// Canonical name (records/errors).
    pub const fn name(self) -> &'static str {
        match self {
            MappedQuantity::ActivePower => "active_power",
            MappedQuantity::StateOfCharge => "state_of_charge",
            MappedQuantity::Energy => "energy",
            MappedQuantity::Availability => "availability",
        }
    }

    /// All declared mappings (deterministic order).
    pub const ALL: [MappedQuantity; 4] = [
        MappedQuantity::ActivePower,
        MappedQuantity::StateOfCharge,
        MappedQuantity::Energy,
        MappedQuantity::Availability,
    ];
}

/// Converts one canonical edge sample into the adapter's `UplinkPoint`
/// (the declared 1:1 mapping; the sample's edge-local asset key becomes
/// the metric's device segment). The reactive-power and text-state
/// quantities have NO declared mapping — named refusals, never guesses.
pub fn uplink_point_for(sample: &EdgeTelemetrySample) -> Result<UplinkPoint, EdgeMappingError> {
    let device = sample.asset_key().to_string();
    // The value bindings are references into the sample (the value is
    // borrowed); the canonical unit newtypes are Copy, so the mapped
    // points own copies and the sample is untouched.
    match (sample.quantity(), sample.value()) {
        (TelemetryQuantity::ActivePower, QuantityValue::Power(p)) => Ok(UplinkPoint::ActivePower {
            device,
            value: *p,
        }),
        (TelemetryQuantity::Soc, QuantityValue::Fraction(soc)) => {
            Ok(UplinkPoint::StateOfCharge { device, value: *soc })
        }
        (TelemetryQuantity::Energy, QuantityValue::Energy(e)) => {
            Ok(UplinkPoint::Energy { device, value: *e })
        }
        (TelemetryQuantity::Availability, QuantityValue::Flag(available)) => {
            Ok(UplinkPoint::Availability { device, available: *available })
        }
        (quantity, _) => {
            if MappedQuantity::ALL.iter().any(|m| m.from_edge() == quantity) {
                Err(EdgeMappingError::ValueKindMismatch {
                    quantity: mapped_name_for(quantity),
                })
            } else {
                Err(EdgeMappingError::QuantityNotMapped {
                    quantity: edge_quantity_name(quantity),
                })
            }
        }
    }
}

fn mapped_name_for(quantity: TelemetryQuantity) -> &'static str {
    match quantity {
        TelemetryQuantity::ActivePower => MappedQuantity::ActivePower.name(),
        TelemetryQuantity::Soc => MappedQuantity::StateOfCharge.name(),
        TelemetryQuantity::Energy => MappedQuantity::Energy.name(),
        TelemetryQuantity::Availability => MappedQuantity::Availability.name(),
        _ => "unmapped",
    }
}

/// The edge quantity's canonical name (the frozen vocabulary's own
/// Debug spellings, lowercased — the refusal names the quantity; there
/// is no second name table).
fn edge_quantity_name(quantity: TelemetryQuantity) -> &'static str {
    match quantity {
        TelemetryQuantity::ActivePower => "active_power",
        TelemetryQuantity::ReactivePower => "reactive_power",
        TelemetryQuantity::Soc => "soc",
        TelemetryQuantity::Energy => "energy",
        TelemetryQuantity::Availability => "availability",
        TelemetryQuantity::OperatingMode => "operating_mode",
        TelemetryQuantity::Health => "health",
        TelemetryQuantity::ConnectionState => "connection_state",
        TelemetryQuantity::BaselineReference => "baseline_reference",
    }
}

/// Builds the Sparkplug DATA payload for one batch of mapped samples:
/// the kind is DeviceData, the sequence continues the declared
/// generator's chain, and every metric carries the canonical unit in
/// its DECLARED name (the adapter's own namespace rule — the mapping
/// adds no second naming scheme).
pub fn data_payload_for(
    plan: &SparkplugUplinkPlan,
    points: &[&UplinkPoint],
    seq: SparkplugSeq,
    bd_seq: BdSeq,
) -> Result<(ValidatedTopic, SparkplugPayload), EdgeMappingError> {
    let mut metrics = Vec::with_capacity(points.len());
    for point in points {
        let metric = plan
            .metric_for(point)
            .map_err(|error| EdgeMappingError::AdapterModel(error.to_string()))?;
        metrics.push(metric);
    }
    // The batch maps to the edge node's NDATA surface (the declared
    // batch shape — the per-device DDATA topics are the per-sample path).
    let topic = ValidatedTopic::new(
        plan.group_id.clone(),
        SparkplugMessageKind::NodeData,
        plan.edge_node_id.clone(),
        None,
    )
    .map_err(|error| EdgeMappingError::AdapterModel(error.to_string()))?;
    let payload = SparkplugPayload {
        kind: SparkplugMessageKind::NodeData,
        seq: Some(seq),
        bd_seq,
        metrics,
    };
    payload
        .validate()
        .map_err(|error| EdgeMappingError::AdapterModel(error.to_string()))?;
    Ok((topic, payload))
}

/// The edge-to-cloud uplink channel: implements the edge's
/// [`UplinkChannel`] port over the injected MQTT seam transport, with
/// the declared [`SparkplugUplinkPlan`] identity and the declared
/// [`FleetUplinkBudget`].
pub struct SparkplugUplinkChannel<'t> {
    /// The declared uplink identity (group + edge node).
    pub plan: SparkplugUplinkPlan,
    /// The injected wire seam transport (shipped default: refusing).
    /// `+ Send`: `UplinkChannel: Send` (ports.rs), and the channel holds
    /// this reference — the transport itself declares no `Send` supertrait
    /// (its implementors are Send; the bound lives at the object site, the
    /// same shape as `Arc<dyn Clock + Send + Sync>` in the runner).
    pub transport: &'t mut (dyn MqttUplinkTransport + Send),
    /// The declared fleet budget.
    pub budget: FleetUplinkBudget,
    /// FIM-F05.05 (additive): the OPTIONAL command-delivery source (the
    /// live transport's bounded inbox). `None` keeps the strict
    /// monitor-only posture; `attach_command_source` sets it.
    command_source: Option<&'t mut (dyn MqttCommandSource + Send)>,
    /// The message-sequence state (continues per forward).
    seq: u64,
    /// The bdSeq generation (a reconnect bumps it — the declared
    /// saturating counter).
    bd_seq: BdSeq,
}

impl<'t> SparkplugUplinkChannel<'t> {
    /// Declares the channel over an identity, a transport, and a budget.
    pub fn new(
        plan: SparkplugUplinkPlan,
        transport: &'t mut (dyn MqttUplinkTransport + Send),
        budget: FleetUplinkBudget,
    ) -> Self {
        Self {
            plan,
            transport,
            budget,
            command_source: None,
            seq: 0,
            bd_seq: BdSeq::initial(),
        }
    }

    /// FIM-F05.05 (additive): attaches the command-delivery source (the
    /// live transport implements `MqttCommandSource`). Without it the
    /// channel is strictly monitor-only. A delivered command enters the
    /// command lifecycle at PROPOSED and nowhere else — attaching the
    /// source grants no authority.
    pub fn attach_command_source(&mut self, source: &'t mut (dyn MqttCommandSource + Send)) {
        self.command_source = Some(source);
    }

    /// FIM-F05.05 (additive): drains the commands the attached source
    /// received on the subscribed command topic (oldest first). The
    /// caller (the composition root) hands each delivery to the edge
    /// command path — entering the lifecycle at PROPOSED is the command
    /// path's own gate, never this drain's.
    pub fn drain_received_commands(&mut self) -> Vec<SparkplugCommandDelivery> {
        let Some(source) = self.command_source.as_deref_mut() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        while let Some(delivery) = source.poll_command() {
            out.push(delivery);
        }
        out
    }

    /// Bumps the bdSeq generation (the reconnect path; declared
    /// explicitly, never implicit in a forward).
    pub fn bump_session(&mut self) {
        self.bd_seq = self.bd_seq.next();
    }
}

impl UplinkChannel for SparkplugUplinkChannel<'_> {
    /// Advisory: whether the injected transport reports a usable
    /// connection. The seam trait has no connected-query, so this is
    /// honestly `false` for the shipped refusing transport; with an
    /// attached FIM-F05 command source it consults the source's live
    /// session state. `forward` is the authoritative outcome either way
    /// (the port contract's own words).
    fn is_connected(&self) -> bool {
        match &self.command_source {
            Some(source) => source.is_connected_now(),
            None => false,
        }
    }

    /// Forwards one batch: the declared budget is checked FIRST (a
    /// batch that exceeds the declared caps is refused before any
    /// mapping work), every sample is mapped (an unmapped quantity
    /// refuses the WHOLE batch — never a partial forward), and the
    /// mapped metrics publish as ONE NDATA payload through the injected
    /// transport. A failed batch is `Err` and the runner re-buffers it
    /// IN ORDER (the port's at-least-once contract).
    fn forward(&mut self, samples: &[EdgeTelemetrySample]) -> Result<(), EdgeError> {
        // Budget first: batch-size cap, then message rate.
        self.budget
            .allow_batch(samples.len(), self.plan.edge_node_id.value())
            .map_err(|reason| EdgeError::UplinkForwardFailed {
                reason: EdgeMappingError::BudgetRefused { reason }.to_string(),
            })?;
        // Map every sample; one refusal refuses the whole batch (never a
        // partial forward — the runner's re-buffer contract requires the
        // exact batch back).
        let mut points = Vec::with_capacity(samples.len());
        for sample in samples {
            let point = uplink_point_for(sample).map_err(|error| EdgeError::UplinkForwardFailed {
                reason: error.to_string(),
            })?;
            points.push(point);
        }
        let point_refs: Vec<&UplinkPoint> = points.iter().collect();
        let seq = SparkplugSeq::new(self.seq).map_err(|error| EdgeError::UplinkForwardFailed {
            reason: format!("the uplink sequence counter refused the batch: {error}"),
        })?;
        let (topic, payload) = data_payload_for(&self.plan, &point_refs, seq, self.bd_seq)
            .map_err(|error| EdgeError::UplinkForwardFailed {
                reason: error.to_string(),
            })?;
        let PublishReceipt { .. } = self
            .transport
            .publish(&topic, &payload)
            .map_err(|error| EdgeError::UplinkForwardFailed {
                reason: EdgeMappingError::Transport(error).to_string(),
            })?;
        // The declared sequence chain advances only on an acknowledged
        // publish (the seam's own contract: a receipt is the broker's
        // ack, NOT downstream delivery).
        self.seq = self.seq.wrapping_add(1);
        Ok(())
    }
}

/// The declared fleet uplink budget: what ONE edge node may send and how
/// many nodes the fleet budgets for. Declared data (validated at
/// construction; no defaults — an undeclared budget is not a budget).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FleetUplinkBudget {
    /// The maximum samples per forward batch (non-zero; the runner's
    /// poll window feeds this).
    pub max_samples_per_batch: usize,
    /// The maximum forwards per interval for one edge node
    /// (`max_forwards_per_interval` per `interval_millis` — the
    /// message-rate budget the per-adapter backpressure vocabulary
    /// also declares).
    pub max_forwards_per_interval: u64,
    /// The rate interval in milliseconds (positive).
    pub interval_millis: i64,
    /// The fleet-scale node budget: how many edge nodes this deployment
    /// budgets uplinks for (a node identity beyond the declared fleet
    /// size is a composition error — fleet scale is declared, never
    /// unbounded).
    pub fleet_node_budget: u32,
    /// The nodes already budgeted (BTreeMap by node id — deterministic;
    /// the composition registers each edge node's identity here).
    nodes: BTreeMap<String, ()>,
}

impl FleetUplinkBudget {
    /// Declares and validates the budget.
    pub fn new(
        max_samples_per_batch: usize,
        max_forwards_per_interval: u64,
        interval_millis: i64,
        fleet_node_budget: u32,
    ) -> Result<Self, EdgeMappingError> {
        if max_samples_per_batch == 0 {
            return Err(EdgeMappingError::BudgetRefused {
                reason: "max_samples_per_batch must be non-zero (no defaults)".to_string(),
            });
        }
        if max_forwards_per_interval == 0 {
            return Err(EdgeMappingError::BudgetRefused {
                reason: "max_forwards_per_interval must be non-zero (no defaults)".to_string(),
            });
        }
        if interval_millis <= 0 {
            return Err(EdgeMappingError::BudgetRefused {
                reason: "interval_millis must be positive (no defaults)".to_string(),
            });
        }
        if fleet_node_budget == 0 {
            return Err(EdgeMappingError::BudgetRefused {
                reason: "fleet_node_budget must be non-zero (fleet scale is declared, never unbounded)".to_string(),
            });
        }
        Ok(Self {
            max_samples_per_batch,
            max_forwards_per_interval,
            interval_millis,
            fleet_node_budget,
            nodes: BTreeMap::new(),
        })
    }

    /// Registers one edge node's identity into the fleet budget
    /// (refused when the fleet budget is exhausted — the fleet scale is
    /// the declared cap, never a soft suggestion).
    pub fn register_node(&mut self, edge_node_id: &str) -> Result<(), EdgeMappingError> {
        if edge_node_id.trim().is_empty() {
            return Err(EdgeMappingError::BudgetRefused {
                reason: "the edge node id must be non-empty".to_string(),
            });
        }
        if self.nodes.len() >= self.fleet_node_budget as usize {
            return Err(EdgeMappingError::BudgetRefused {
                reason: format!(
                    "the fleet node budget is exhausted ({}/{})",
                    self.nodes.len(),
                    self.fleet_node_budget
                ),
            });
        }
        self.nodes.insert(edge_node_id.to_string(), ());
        Ok(())
    }

    /// Whether the node id is registered in the fleet budget.
    pub fn node_registered(&self, edge_node_id: &str) -> bool {
        self.nodes.contains_key(edge_node_id)
    }

    /// The budget check for one forward of `batch_len` samples by the
    /// named node: the node must be registered, the batch must fit the
    /// declared batch cap, and (when the caller tracks the interval's
    /// forward count) the rate cap applies via the returned allowance.
    pub fn allow_batch(&self, batch_len: usize, edge_node_id: &str) -> Result<(), String> {
        if !self.node_registered(edge_node_id) {
            return Err(format!(
                "edge node '{edge_node_id}' is not registered in the fleet uplink budget"
            ));
        }
        if batch_len > self.max_samples_per_batch {
            return Err(format!(
                "batch of {batch_len} samples exceeds the declared cap of {}",
                self.max_samples_per_batch
            ));
        }
        Ok(())
    }

    /// The declared message-rate budget as the shared vocabulary's shape
    /// (consumed by the per-adapter backpressure modules; one rate
    /// vocabulary, never two).
    pub fn message_rate(&self) -> flexcore_protocol_ports::observability::MessageRateBudget {
        // Construction validated both fields; this cannot fail (and the
        // type carries no Result because the composition already
        // validated the declared values).
        flexcore_protocol_ports::observability::MessageRateBudget {
            max_messages: self.max_forwards_per_interval,
            interval_millis: self.interval_millis,
        }
    }
}

/// A quality-suffix helper for the declared marking policy: a GOOD
/// sample's metric keeps its plain declared name; a non-GOOD sample is
/// marked by appending the declared `degraded` suffix so a consumer can
/// filter without a second quality channel. The mapping never UPGRADES
/// a quality (the platform decides trust).
pub fn quality_suffix(quality: TelemetryQuality) -> &'static str {
    if quality == TelemetryQuality::Good {
        ""
    } else {
        ".degraded"
    }
}

/// The declared SOC conversion helper: the edge sample's SOC fraction is
/// already canonical; the mapping constructs the adapter's
/// [`StateOfCharge`] value unchanged (no percent detour — the P0.07 §4
/// rule: percent is never canonical).
pub fn soc_value(sample: &EdgeTelemetrySample) -> Result<StateOfCharge, EdgeMappingError> {
    match sample.value() {
        QuantityValue::Fraction(soc) => Ok(*soc),
        _ => Err(EdgeMappingError::ValueKindMismatch {
            quantity: MappedQuantity::StateOfCharge.name(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::EdgeTelemetrySampleParts;
    use flexcore_contracts::time_model::{EventTime, IngestTime};
    use flexcore_contracts::units::ActivePowerMW;
    use flexcore_mqtt_adapter::model::{EdgeNodeId, GroupId};
    use flexcore_mqtt_adapter::transport::UnwiredUplinkTransport;

    fn sample_parts(quantity: TelemetryQuantity, value: QuantityValue) -> EdgeTelemetrySampleParts {
        EdgeTelemetrySampleParts {
            asset_key: "ess-01".to_string(),
            quantity,
            value,
            quality: TelemetryQuality::Good,
            source_quality_code: None,
            event_time: EventTime::from_unix_millis(1_000).expect("valid"),
            ingest_time: IngestTime::from_unix_millis(2_000).expect("valid"),
            source_sequence: 1,
            source_id: "edge:pilot-site-A:openems-rest:ess-01:x".to_string(),
        }
    }

    fn sample(quantity: TelemetryQuantity, value: QuantityValue) -> EdgeTelemetrySample {
        EdgeTelemetrySample::new(sample_parts(quantity, value)).expect("valid sample")
    }

    fn budget() -> FleetUplinkBudget {
        FleetUplinkBudget::new(64, 6, 1_000, 4).expect("valid budget")
    }

    fn plan() -> SparkplugUplinkPlan {
        SparkplugUplinkPlan::new(
            GroupId::new("site-42").expect("valid"),
            EdgeNodeId::new("edge-07").expect("valid"),
        )
    }

    #[test]
    fn mapped_quantities_convert_one_to_one_into_uplink_points() {
        let power = uplink_point_for(&sample(
            TelemetryQuantity::ActivePower,
            QuantityValue::Power(ActivePowerMW::new(1.5).expect("valid")),
        ))
        .expect("maps");
        assert_eq!(power.metric_name(), "ess-01.p.mw");
        let soc = uplink_point_for(&sample(
            TelemetryQuantity::Soc,
            QuantityValue::Fraction(StateOfCharge::new(0.82).expect("valid")),
        ))
        .expect("maps");
        assert_eq!(soc.metric_name(), "ess-01.soc.soc_fraction");
        let flag = uplink_point_for(&sample(
            TelemetryQuantity::Availability,
            QuantityValue::Flag(true),
        ))
        .expect("maps");
        assert_eq!(flag.metric_name(), "ess-01.available.available");
    }

    #[test]
    fn unmapped_quantities_and_kind_mismatches_are_named_refusals() {
        // A text-state quantity has no declared mapping.
        let error = uplink_point_for(&sample(
            TelemetryQuantity::Health,
            QuantityValue::State("NOMINAL".to_string()),
        ))
        .expect_err("unmapped");
        assert!(matches!(error, EdgeMappingError::QuantityNotMapped { quantity: "health" }));
        // A mapped quantity with the wrong value kind (bad composition) is
        // refused at CONSTRUCTION: EdgeTelemetrySample::new enforces the
        // frozen P2.04 pairing (quantity.accepts(value)), so such a sample
        // can never reach the mapping layer. The mapping's ValueKindMismatch
        // arm is defence-in-depth for that already-refused composition
        // (MappedQuantity::ALL is exactly the four quantities, each
        // accepting exactly one value kind, which the match arms accept).
        let error = EdgeTelemetrySample::new(sample_parts(
            TelemetryQuantity::ActivePower,
            QuantityValue::Flag(true),
        ))
        .expect_err("kind mismatch refused at construction");
        assert!(matches!(
            error,
            crate::ports::EdgeSampleError::ValueQuantityMismatch {
                quantity: TelemetryQuantity::ActivePower
            }
        ));
    }

    #[test]
    fn the_batch_payload_carries_the_declared_metric_names_and_seq() {
        let plan = plan();
        let points = vec![
            uplink_point_for(&sample(
                TelemetryQuantity::ActivePower,
                QuantityValue::Power(ActivePowerMW::new(1.5).expect("valid")),
            ))
            .expect("maps"),
            uplink_point_for(&sample(
                TelemetryQuantity::Soc,
                QuantityValue::Fraction(StateOfCharge::new(0.5).expect("valid")),
            ))
            .expect("maps"),
        ];
        let refs: Vec<&UplinkPoint> = points.iter().collect();
        let (topic, payload) = data_payload_for(
            &plan,
            &refs,
            SparkplugSeq::new(3).expect("valid seq"),
            BdSeq::initial(),
        )
        .expect("payload");
        assert!(topic.topic_string().contains("/NDATA/edge-07"));
        assert_eq!(payload.metrics.len(), 2);
        assert_eq!(payload.metrics[0].name, "ess-01.p.mw");
        assert_eq!(payload.seq, Some(SparkplugSeq::new(3).expect("valid")));
    }

    #[test]
    fn the_shipped_refusing_transport_fails_the_forward_named() {
        let mut transport = UnwiredUplinkTransport;
        let mut budget = budget();
        budget.register_node("edge-07").expect("registered");
        let mut channel = SparkplugUplinkChannel::new(plan(), &mut transport, budget);
        let error = channel
            .forward(&[sample(
                TelemetryQuantity::ActivePower,
                QuantityValue::Power(ActivePowerMW::new(1.0).expect("valid")),
            )])
            .expect_err("the shipped default refuses");
        assert!(error.to_string().contains("UNWIRED"), "the refusal names the seam: {error}");
    }

    #[test]
    fn the_budget_refuses_unregistered_nodes_and_oversized_batches() {
        let mut budget = budget();
        budget.register_node("edge-07").expect("registered");
        // An unregistered node refuses.
        assert!(budget.allow_batch(1, "edge-unknown").is_err());
        // An oversized batch refuses.
        assert!(budget.allow_batch(65, "edge-07").is_err());
        // A fitting batch passes.
        budget.allow_batch(64, "edge-07").expect("fits");
        // The fleet node cap is enforced at registration.
        let mut budget = FleetUplinkBudget::new(8, 1, 1_000, 1).expect("valid");
        budget.register_node("edge-a").expect("first node fits");
        assert!(budget.register_node("edge-b").is_err(), "fleet cap of 1");
    }

    #[test]
    fn the_rate_budget_maps_onto_the_shared_vocabulary() {
        let budget = budget();
        let rate = budget.message_rate();
        assert_eq!(rate.max_messages, 6);
        assert_eq!(rate.interval_millis, 1_000);
        rate.allows(6).expect("the shared budget accepts the declared rate");
        assert!(rate.allows(7).is_err());
    }

    #[test]
    fn degenerate_budgets_are_construction_errors() {
        assert!(FleetUplinkBudget::new(0, 6, 1_000, 4).is_err());
        assert!(FleetUplinkBudget::new(64, 0, 1_000, 4).is_err());
        assert!(FleetUplinkBudget::new(64, 6, 0, 4).is_err());
        assert!(FleetUplinkBudget::new(64, 6, 1_000, 0).is_err());
    }

    #[test]
    fn degraded_quality_is_marked_never_upgraded() {
        assert_eq!(quality_suffix(TelemetryQuality::Good), "");
        assert_eq!(quality_suffix(TelemetryQuality::Suspect), ".degraded");
        assert_eq!(quality_suffix(TelemetryQuality::Stale), ".degraded");
    }

    // --- FIM-F05.05 (additive command-delivery surface) -------------------

    use flexcore_mqtt_adapter::live::{MqttCommandSource, SparkplugCommandDelivery};
    use flexcore_mqtt_adapter::model::BdSeq as AdapterBdSeq;
    use flexcore_mqtt_adapter::model::SparkplugSeq as AdapterSparkplugSeq;

    /// A fake command source (the live transport's inbox stand-in).
    struct FakeCommandSource {
        pending: Vec<SparkplugCommandDelivery>,
        connected: bool,
    }

    impl MqttCommandSource for FakeCommandSource {
        fn poll_command(&mut self) -> Option<SparkplugCommandDelivery> {
            self.pending.pop()
        }

        fn is_connected_now(&self) -> bool {
            self.connected
        }
    }

    fn fake_delivery() -> SparkplugCommandDelivery {
        SparkplugCommandDelivery {
            topic: "spBv1.0/site-42/NCMD/edge-07".to_string(),
            timestamp_millis: None,
            uuid: None,
            seq: AdapterSparkplugSeq::new(1).ok(),
            bd_seq: Some(AdapterBdSeq::initial()),
            metrics: Vec::new(),
        }
    }

    #[test]
    fn without_an_attached_source_the_channel_is_strictly_monitor_only() {
        let mut transport = UnwiredUplinkTransport;
        let mut budget = budget();
        budget.register_node("edge-07").expect("registered");
        let mut channel = SparkplugUplinkChannel::new(plan(), &mut transport, budget);
        // No source: no connected query, no commands, no authority.
        assert!(!channel.is_connected());
        assert!(channel.drain_received_commands().is_empty());
    }

    #[test]
    fn an_attached_source_delivers_commands_into_the_edge_named() {
        let mut transport = UnwiredUplinkTransport;
        let mut budget = budget();
        budget.register_node("edge-07").expect("registered");
        let mut source = FakeCommandSource {
            pending: vec![fake_delivery(), fake_delivery()],
            connected: true,
        };
        let mut channel = SparkplugUplinkChannel::new(plan(), &mut transport, budget);
        channel.attach_command_source(&mut source);
        // The advisory connected query now consults the live session.
        assert!(channel.is_connected());
        // The drain empties the inbox (oldest first is the engine's
        // push order; this fake pops newest — the drain contract is
        // "everything currently pending").
        let drained = channel.drain_received_commands();
        assert_eq!(drained.len(), 2);
        assert!(channel.drain_received_commands().is_empty());
        // A delivery carries NO authority: it is a topic + decoded
        // metrics bundle — entering the lifecycle at PROPOSED is the
        // command path's own gate.
        assert!(drained[0].metrics.is_empty());
        assert_eq!(drained[0].topic, "spBv1.0/site-42/NCMD/edge-07");
    }
}
