// P14.11 — The declared Sparkplug topic namespace (decision record
// 43_p14_11_mqtt_sparkplug_b_uplink.md; sources [S-39][S-40]).
//
// UNVERIFIED HONESTY: the namespace prefix (`spBv1.0`) and the topic
// segment order (`namespace/group_id/message_type/edge_node_id[/
// device_id]`) are secondary-source readings; NO specification text is in
// the repository. The implementation-time check confirms the prefix, the
// segment order, the segment separators, and whether STATE topics deviate
// from the pattern (public descriptions commonly show STATE published on a
// different topic shape — recorded here as an open question, never
// guessed). A mismatch is a recorded edit to the declared constants and
// this module, never a silent reinterpretation.
//
// Design rules (declared, not inferred):
//
// 1. A topic is built ONLY from validated identity parts and a declared
//    message kind — there is no free-form topic string constructor.
// 2. The device segment exists ONLY for device-scoped kinds (DBIRTH /
//    DDATA / DDEATH); an edge-node-scoped kind with a device segment is a
//    construction error, and a device-scoped kind without one likewise.
// 3. The namespace is a declared constant, never a per-call default: a
//    deployment that needs a different namespace changes the DECLARED
//    constant through a reviewed edit, not a runtime string.

use serde::{Deserialize, Serialize};

use crate::model::{DeviceId, EdgeNodeId, GroupId, ModelError, SparkplugMessageKind};

/// The declared Sparkplug B namespace prefix.
///
/// UNVERIFIED — the implementation-time check against the Sparkplug B
/// specification text confirms the exact prefix string (public secondary
/// sources commonly show `spBv1.0`; the OpenFMB overview [S-40] describes
/// Sparkplug B topics without pinning the string here).
pub const SPARKPLUG_B_NAMESPACE: &str = "spBv1.0";

/// Errors returned by topic construction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TopicError {
    /// The identity parts failed validation (re-raised from the model).
    Model(ModelError),
    /// A device-scoped kind was constructed without a device id, or an
    /// edge-node-scoped kind was given one.
    DeviceSegmentMismatch {
        kind_segment: &'static str,
        has_device_id: bool,
    },
}

impl std::fmt::Display for TopicError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TopicError::Model(error) => write!(f, "topic identity invalid: {error}"),
            TopicError::DeviceSegmentMismatch {
                kind_segment,
                has_device_id,
            } => write!(
                f,
                "message kind {kind_segment} does not match the device-segment presence \
                 (has_device_id: {has_device_id})"
            ),
        }
    }
}

impl std::error::Error for TopicError {}

impl From<ModelError> for TopicError {
    fn from(error: ModelError) -> Self {
        TopicError::Model(error)
    }
}

/// A validated Sparkplug topic: the declared namespace, the group, the
/// message kind, the edge node, and the optional device segment.
///
/// Constructed only through [`ValidatedTopic::new`], which enforces the
/// kind/device-segment agreement — there is no free-form topic string.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ValidatedTopic {
    pub group_id: GroupId,
    pub kind: SparkplugMessageKind,
    pub edge_node_id: EdgeNodeId,
    pub device_id: Option<DeviceId>,
}

impl ValidatedTopic {
    /// Whether the message kind scopes a device segment (per the declared
    /// topic grammar; UNVERIFIED against the specification text).
    fn kind_expects_device(kind: SparkplugMessageKind) -> bool {
        matches!(
            kind,
            SparkplugMessageKind::DeviceBirth
                | SparkplugMessageKind::DeviceData
                | SparkplugMessageKind::DeviceDeath
                | SparkplugMessageKind::Command
        )
    }

    /// Constructs and validates a topic from its parts.
    pub fn new(
        group_id: GroupId,
        kind: SparkplugMessageKind,
        edge_node_id: EdgeNodeId,
        device_id: Option<DeviceId>,
    ) -> Result<Self, TopicError> {
        let expects_device = Self::kind_expects_device(kind);
        if expects_device != device_id.is_some() {
            return Err(TopicError::DeviceSegmentMismatch {
                kind_segment: kind.topic_segment(),
                has_device_id: device_id.is_some(),
            });
        }
        Ok(Self {
            group_id,
            kind,
            edge_node_id,
            device_id,
        })
    }

    /// The deterministic topic string (the declared namespace prefix; the
    /// segment order below is UNVERIFIED — see the module documentation).
    pub fn topic_string(&self) -> String {
        let mut topic = String::from(SPARKPLUG_B_NAMESPACE);
        topic.push('/');
        topic.push_str(self.group_id.value());
        topic.push('/');
        topic.push_str(self.kind.topic_segment());
        topic.push('/');
        topic.push_str(self.edge_node_id.value());
        if let Some(device) = &self.device_id {
            topic.push('/');
            topic.push_str(device.value());
        }
        topic
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parts() -> (GroupId, EdgeNodeId, DeviceId) {
        (
            GroupId::new("site-42").expect("valid"),
            EdgeNodeId::new("edge-07").expect("valid"),
            DeviceId::new("inverter-1").expect("valid"),
        )
    }

    #[test]
    fn node_scoped_topic_has_four_segments() {
        let (group, node, _device) = parts();
        let topic = ValidatedTopic::new(
            group,
            SparkplugMessageKind::NodeBirth,
            node,
            None,
        )
        .expect("valid");
        assert_eq!(topic.topic_string(), "spBv1.0/site-42/NBIRTH/edge-07");
    }

    #[test]
    fn device_scoped_topic_has_five_segments() {
        let (group, node, device) = parts();
        let topic = ValidatedTopic::new(
            group,
            SparkplugMessageKind::DeviceData,
            node,
            Some(device),
        )
        .expect("valid");
        assert_eq!(topic.topic_string(), "spBv1.0/site-42/DDATA/edge-07/inverter-1");
    }

    #[test]
    fn device_segment_mismatch_is_a_construction_error() {
        let (group, node, device) = parts();
        // A device-scoped kind without a device id.
        assert_eq!(
            ValidatedTopic::new(group.clone(), SparkplugMessageKind::DeviceBirth, node.clone(), None),
            Err(TopicError::DeviceSegmentMismatch {
                kind_segment: "DBIRTH",
                has_device_id: false,
            })
        );
        // An edge-node-scoped kind WITH a device id.
        assert_eq!(
            ValidatedTopic::new(
                group,
                SparkplugMessageKind::NodeData,
                node,
                Some(device),
            ),
            Err(TopicError::DeviceSegmentMismatch {
                kind_segment: "NDATA",
                has_device_id: true,
            })
        );
    }
}
