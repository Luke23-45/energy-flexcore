//! P14.11 — Topic-namespace integration tests (decision record
//! 43_p14_11_mqtt_sparkplug_b_uplink.md).
//!
//! WRITTEN-UNRUN: complete test files whose execution is a human gate
//! (conventions §1.3). These tests exercise the PUBLIC topic-namespace
//! surface: the deterministic topic-string form, the kind/device-segment
//! agreement, the identity validation, and the declared-vs-unverified
//! honesty pins. Nothing here executes wire I/O — the transport seam is
//! unwired, no broker exists, no topic is ever published.

use flexcore_mqtt_adapter::model::{
    DeviceId, EdgeNodeId, GroupId, ModelError, SparkplugMessageKind,
};
use flexcore_mqtt_adapter::topic::{TopicError, ValidatedTopic, SPARKPLUG_B_NAMESPACE};

fn node_topic(kind: SparkplugMessageKind, group: &str, node: &str) -> ValidatedTopic {
    ValidatedTopic::new(
        GroupId::new(group).expect("valid group"),
        kind,
        EdgeNodeId::new(node).expect("valid node"),
        None,
    )
    .expect("valid topic")
}

fn device_topic(
    kind: SparkplugMessageKind,
    group: &str,
    node: &str,
    device: &str,
) -> ValidatedTopic {
    ValidatedTopic::new(
        GroupId::new(group).expect("valid group"),
        kind,
        EdgeNodeId::new(node).expect("valid node"),
        Some(DeviceId::new(device).expect("valid device")),
    )
    .expect("valid topic")
}

// --- Declared namespace grammar ---------------------------------------------

#[test]
fn node_scoped_kinds_produce_four_segment_topics() {
    let topic = node_topic(SparkplugMessageKind::NodeBirth, "site-42", "edge-07");
    assert_eq!(topic.topic_string(), "spBv1.0/site-42/NBIRTH/edge-07");
    let topic = node_topic(SparkplugMessageKind::NodeData, "site-42", "edge-07");
    assert_eq!(topic.topic_string(), "spBv1.0/site-42/NDATA/edge-07");
    let topic = node_topic(SparkplugMessageKind::NodeDeath, "site-42", "edge-07");
    assert_eq!(topic.topic_string(), "spBv1.0/site-42/NDEATH/edge-07");
}

#[test]
fn device_scoped_kinds_produce_five_segment_topics() {
    let topic = device_topic(
        SparkplugMessageKind::DeviceBirth,
        "site-42",
        "edge-07",
        "inverter-1",
    );
    assert_eq!(topic.topic_string(), "spBv1.0/site-42/DBIRTH/edge-07/inverter-1");
    let topic = device_topic(
        SparkplugMessageKind::DeviceData,
        "site-42",
        "edge-07",
        "meter-1",
    );
    assert_eq!(topic.topic_string(), "spBv1.0/site-42/DDATA/edge-07/meter-1");
    let topic = device_topic(
        SparkplugMessageKind::DeviceDeath,
        "site-42",
        "edge-07",
        "battery-1",
    );
    assert_eq!(topic.topic_string(), "spBv1.0/site-42/DDEATH/edge-07/battery-1");
}

#[test]
fn the_declared_namespace_prefix_is_a_constant_with_one_definition_site() {
    // The prefix is UNVERIFIED (a secondary-source reading); this test
    // pins it as the DECLARED constant so a specification-confirmed
    // correction is a visible one-line edit in the model, never a silent
    // drift across call sites.
    assert_eq!(SPARKPLUG_B_NAMESPACE, "spBv1.0");
    let topic = node_topic(SparkplugMessageKind::NodeBirth, "g", "n");
    assert!(topic.topic_string().starts_with(SPARKPLUG_B_NAMESPACE));
}

// --- Kind / device-segment agreement ----------------------------------------

#[test]
fn device_segment_mismatches_are_construction_errors() {
    // Device-scoped kinds REQUIRE a device segment.
    assert_eq!(
        ValidatedTopic::new(
            GroupId::new("site-42").expect("valid"),
            SparkplugMessageKind::DeviceBirth,
            EdgeNodeId::new("edge-07").expect("valid"),
            None,
        ),
        Err(TopicError::DeviceSegmentMismatch {
            kind_segment: "DBIRTH",
            has_device_id: false,
        })
    );
    // Edge-node-scoped kinds FORBID one.
    assert_eq!(
        ValidatedTopic::new(
            GroupId::new("site-42").expect("valid"),
            SparkplugMessageKind::NodeDeath,
            EdgeNodeId::new("edge-07").expect("valid"),
            Some(DeviceId::new("inverter-1").expect("valid")),
        ),
        Err(TopicError::DeviceSegmentMismatch {
            kind_segment: "NDEATH",
            has_device_id: true,
        })
    );
}

#[test]
fn the_state_kind_takes_no_device_segment_under_the_declared_grammar() {
    // STATE is declared as an edge-node-shaped kind under the declared
    // grammar; the Sparkplug STATE topic's actual shape is an open
    // question recorded in the decision record (UNVERIFIED) — the check
    // against the specification text may move it, which would be a
    // recorded edit to topic.rs.
    let topic = node_topic(SparkplugMessageKind::State, "site-42", "edge-07");
    assert_eq!(topic.topic_string(), "spBv1.0/site-42/STATE/edge-07");
}

// --- Identity validation ----------------------------------------------------

#[test]
fn identity_parts_reject_empty_and_whitespace() {
    assert_eq!(GroupId::new("   "), Err(ModelError::EmptyField("group_id")));
    assert_eq!(
        EdgeNodeId::new(""),
        Err(ModelError::EmptyField("edge_node_id"))
    );
    assert_eq!(DeviceId::new("  \t "), Err(ModelError::EmptyField("device_id")));
}

#[test]
fn identity_parts_reject_oversized_values() {
    let oversized = "x".repeat(129);
    assert!(matches!(
        GroupId::new(&oversized),
        Err(ModelError::TooLong { field: "group_id", .. })
    ));
    let at_bound = "x".repeat(128);
    assert!(GroupId::new(&at_bound).is_ok(), "128 is the declared bound");
}

#[test]
fn identity_parts_trim_declared_whitespace() {
    let group = GroupId::new(" site-42 ").expect("valid");
    assert_eq!(group.value(), "site-42");
}
