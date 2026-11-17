// P14.11 — The edge-agent uplink OPTION (decision record
// 43_p14_11_mqtt_sparkplug_b_uplink.md; the plan §2 row P14.11 deliverable).
//
// WHAT THIS MODULE IS: the declared mapping from canonical uplink points
// (the quantities the edge's telemetry path carries) to Sparkplug topics
// and metrics — the design the wave-3 composition root would implement
// over the edge-agent's `UplinkChannel` port
// (`crates/edge-agent/src/ports.rs`).
//
// WHAT THIS MODULE DELIBERATELY DOES NOT DO:
//
// 1. **No dependency on `flexcore-edge-agent`.** The edge agent is a
//    COMPOSITION crate (tools/check_dependency_rule.py policy: nothing
//    depends ON composition crates except apps). This module therefore
//    declares its own [`UplinkPoint`] input type with the canonical unit
//    types from `flexcore-contracts`, and the composition point is NAMED
//    in the docs — a wave-3 adapter struct implements `UplinkChannel` by
//    draining the edge buffer and converting its samples through this
//    mapping. The seam is designed here; the wiring is wave 3's.
// 2. **No unit is guessed.** Sparkplug metrics carry a typed numeric
//    value but NO unit field (UNVERIFIED — the implementation-time check
//    against the Sparkplug B text confirms the payload's field set).
//    FlexCore therefore REQUIRES every point's metric to carry its unit
//    in the DECLARED metric name (the namespace convention this module
//    validates), and a point whose unit declaration is missing is
//    refused — never defaulted (the P6.10 quality-declaration rule, and
//    the reactive-sign rule of `flexcore-contracts::units` applied to the
//    uplink surface).
// 3. **No NATS productization.** OpenFMB's NATS-at-the-grid-edge evidence
//    [S-40] is recorded in the decision record; the existing
//    `crates/adapters/nats` crate stays the platform-internal bus, and no
//    NATS edge transport exists here.

use flexcore_contracts::units::{ActivePowerMW, EnergyMWh, StateOfCharge};

use serde::{Deserialize, Serialize};

use crate::model::{DeviceId, EdgeNodeId, GroupId, MetricType, MetricValue, ModelError,
    SparkplugMetric};
use crate::topic::{TopicError, ValidatedTopic};

/// The canonical uplink quantities this option declares (the Phase 0–10
/// exit-gate asset quantities; the unit is part of the DECLARED name).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UplinkQuantity {
    /// Active power at the point of connection (canonical MW; `P > 0` =
    /// export — the frozen rule travels in the metric NAME, see below).
    ActivePower,
    /// Energy (canonical MWh).
    Energy,
    /// State of charge (canonical fraction in [0,1]).
    StateOfCharge,
    /// Availability (declared boolean, dimensionless).
    Availability,
}

impl UplinkQuantity {
    /// The metric-name unit suffix this quantity carries in the declared
    /// namespace convention. The suffix carries the canonical unit (the
    /// display-only conversions of `flexcore-contracts::units` are NOT
    /// used on the uplink: the metric carries the canonical value).
    pub const fn unit_suffix(self) -> &'static str {
        match self {
            UplinkQuantity::ActivePower => "mw",
            UplinkQuantity::Energy => "mwh",
            UplinkQuantity::StateOfCharge => "soc_fraction",
            UplinkQuantity::Availability => "available",
        }
    }
}

/// One canonical uplink point: the declared quantity, the device it
/// belongs to, and the canonical value. This is the module's own input
/// type (see the edge-agent composition note above); the wave-3 adapter
/// converts the edge-agent's samples into it at the boundary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum UplinkPoint {
    /// Active power (canonical MW, `P > 0` export).
    ActivePower {
        device: String,
        value: ActivePowerMW,
    },
    /// Energy (canonical MWh).
    Energy {
        device: String,
        value: EnergyMWh,
    },
    /// State of charge (canonical fraction in [0,1]).
    StateOfCharge {
        device: String,
        value: StateOfCharge,
    },
    /// Availability (declared boolean).
    Availability {
        device: String,
        available: bool,
    },
}

impl UplinkPoint {
    /// The point's device identifier.
    pub fn device(&self) -> &str {
        match self {
            UplinkPoint::ActivePower { device, .. }
            | UplinkPoint::Energy { device, .. }
            | UplinkPoint::StateOfCharge { device, .. }
            | UplinkPoint::Availability { device, .. } => device,
        }
    }

    /// The point's declared quantity.
    pub const fn quantity(&self) -> UplinkQuantity {
        match self {
            UplinkPoint::ActivePower { .. } => UplinkQuantity::ActivePower,
            UplinkPoint::Energy { .. } => UplinkQuantity::Energy,
            UplinkPoint::StateOfCharge { .. } => UplinkQuantity::StateOfCharge,
            UplinkPoint::Availability { .. } => UplinkQuantity::Availability,
        }
    }

    /// The declared Sparkplug metric name: `<device>.<quantity>.<unit>`.
    /// The unit suffix is part of the name BY RULE (see the module
    /// documentation) — a consumer of the metric can read the canonical
    /// unit from the name alone.
    pub fn metric_name(&self) -> String {
        format!(
            "{}.{}.{}",
            self.device(),
            match self.quantity() {
                UplinkQuantity::ActivePower => "p",
                UplinkQuantity::Energy => "e",
                UplinkQuantity::StateOfCharge => "soc",
                UplinkQuantity::Availability => "available",
            },
            self.quantity().unit_suffix()
        )
    }

    /// The typed metric value for this point (canonical values only — no
    /// display-unit conversions on the uplink).
    pub fn metric_value(&self) -> MetricValue {
        match self {
            UplinkPoint::ActivePower { value, .. } => MetricValue::Double(value.value()),
            UplinkPoint::Energy { value, .. } => MetricValue::Double(value.value()),
            UplinkPoint::StateOfCharge { value, .. } => MetricValue::Double(value.value()),
            UplinkPoint::Availability { available, .. } => MetricValue::Boolean(*available),
        }
    }

    /// The declared metric type for this point.
    pub const fn metric_type(&self) -> MetricType {
        match self.quantity() {
            UplinkQuantity::ActivePower
            | UplinkQuantity::Energy
            | UplinkQuantity::StateOfCharge => MetricType::Double,
            UplinkQuantity::Availability => MetricType::Boolean,
        }
    }
}

/// Errors returned by the uplink mapping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UplinkError {
    /// The identity parts failed validation (re-raised).
    Model(ModelError),
    /// The topic parts failed validation (re-raised).
    Topic(TopicError),
    /// The device identifier is empty.
    EmptyDevice,
}

impl std::fmt::Display for UplinkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UplinkError::Model(error) => write!(f, "uplink identity invalid: {error}"),
            UplinkError::Topic(error) => write!(f, "uplink topic invalid: {error}"),
            UplinkError::EmptyDevice => write!(f, "uplink point device identifier is empty"),
        }
    }
}

impl std::error::Error for UplinkError {}

impl From<ModelError> for UplinkError {
    fn from(error: ModelError) -> Self {
        UplinkError::Model(error)
    }
}

impl From<TopicError> for UplinkError {
    fn from(error: TopicError) -> Self {
        UplinkError::Topic(error)
    }
}

/// The declared uplink mapping: canonical points to Sparkplug metrics and
/// device-scoped DATA topics. Constructed with the deployment's declared
/// identity (group + edge node); every conversion goes through validated
/// constructors — nothing is inferred.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SparkplugUplinkPlan {
    pub group_id: GroupId,
    pub edge_node_id: EdgeNodeId,
}

impl SparkplugUplinkPlan {
    /// Declares the uplink plan for one deployment.
    pub fn new(group_id: GroupId, edge_node_id: EdgeNodeId) -> Self {
        Self {
            group_id,
            edge_node_id,
        }
    }

    /// Maps one canonical point to its typed Sparkplug metric (the
    /// declared name carries the unit; the value is canonical).
    pub fn metric_for(&self, point: &UplinkPoint) -> Result<SparkplugMetric, UplinkError> {
        if point.device().trim().is_empty() {
            return Err(UplinkError::EmptyDevice);
        }
        Ok(SparkplugMetric::new(
            point.metric_name(),
            None,
            point.metric_type(),
            point.metric_value(),
        )?)
    }

    /// Builds the device-scoped DATA topic for one point's device.
    pub fn data_topic_for(&self, point: &UplinkPoint) -> Result<ValidatedTopic, UplinkError> {
        if point.device().trim().is_empty() {
            return Err(UplinkError::EmptyDevice);
        }
        Ok(ValidatedTopic::new(
            self.group_id.clone(),
            crate::model::SparkplugMessageKind::DeviceData,
            self.edge_node_id.clone(),
            Some(DeviceId::new(point.device())?),
        )?)
    }

    /// Builds the edge node's BIRTH topic (the inventory declaration the
    /// session machine requires before any DATA is trusted).
    pub fn birth_topic(&self) -> Result<ValidatedTopic, UplinkError> {
        Ok(ValidatedTopic::new(
            self.group_id.clone(),
            crate::model::SparkplugMessageKind::NodeBirth,
            self.edge_node_id.clone(),
            None,
        )?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan() -> SparkplugUplinkPlan {
        SparkplugUplinkPlan::new(
            GroupId::new("site-42").expect("valid"),
            EdgeNodeId::new("edge-07").expect("valid"),
        )
    }

    #[test]
    fn metric_names_carry_the_declared_unit() {
        let plan = plan();
        let point = UplinkPoint::ActivePower {
            device: "inverter-1".to_string(),
            value: ActivePowerMW::new(3.5).expect("valid"),
        };
        let metric = plan.metric_for(&point).expect("valid metric");
        assert_eq!(metric.name, "inverter-1.p.mw");
        assert_eq!(metric.value, MetricValue::Double(3.5));
        // The canonical unit travels in the NAME (Sparkplug metrics carry
        // no unit field — UNVERIFIED; see the module documentation).
        assert!(metric.name.ends_with(".mw"));
    }

    #[test]
    fn soc_stays_a_fraction_on_the_uplink() {
        let plan = plan();
        let point = UplinkPoint::StateOfCharge {
            device: "battery-1".to_string(),
            value: StateOfCharge::from_percent(82.0).expect("valid"),
        };
        let metric = plan.metric_for(&point).expect("valid metric");
        assert_eq!(metric.name, "battery-1.soc.soc_fraction");
        assert_eq!(metric.value, MetricValue::Double(0.82));
    }

    #[test]
    fn device_scoped_topics_carry_the_device_segment() {
        let plan = plan();
        let point = UplinkPoint::Energy {
            device: "meter-1".to_string(),
            value: EnergyMWh::new(1.25).expect("valid"),
        };
        let topic = plan.data_topic_for(&point).expect("valid topic");
        assert_eq!(topic.topic_string(), "spBv1.0/site-42/DDATA/edge-07/meter-1");
    }

    #[test]
    fn birth_topic_has_no_device_segment() {
        let plan = plan();
        let topic = plan.birth_topic().expect("valid topic");
        assert_eq!(topic.topic_string(), "spBv1.0/site-42/NBIRTH/edge-07");
    }

    #[test]
    fn empty_devices_are_refused() {
        let plan = plan();
        let point = UplinkPoint::Availability {
            device: "   ".to_string(),
            available: true,
        };
        assert_eq!(plan.metric_for(&point), Err(UplinkError::EmptyDevice));
        assert_eq!(plan.data_topic_for(&point), Err(UplinkError::EmptyDevice));
    }
}
