//! P14.11 — BIRTH/DEATH session-machine integration tests (decision record
//! 43_p14_11_mqtt_sparkplug_b_uplink.md).
//!
//! WRITTEN-UNRUN: complete test files whose execution is a human gate
//! (conventions §1.3). These tests exercise the PUBLIC session surface
//! end-to-end: birth → data chain → death → re-birth, the refusal names,
//! sequence wrap under the DECLARED modulus, and the connection-lost path.
//! Nothing here connects to a broker — the machine judges payloads only,
//! and every payload in these tests is a constructed fixture.

use flexcore_mqtt_adapter::birth_death::{
    BirthDeathTracker, DeathReason, NodeSessionState, Observation, RefusalReason,
};
use flexcore_mqtt_adapter::model::{
    BdSeq, MetricType, MetricValue, SparkplugMessageKind, SparkplugMetric, SparkplugPayload,
    SparkplugSeq, SEQ_MODULUS,
};

fn metric() -> SparkplugMetric {
    SparkplugMetric::new(
        "battery-1.soc.soc_fraction".to_string(),
        None,
        MetricType::Double,
        MetricValue::Double(0.82),
    )
    .expect("valid metric")
}

fn payload(kind: SparkplugMessageKind, seq: Option<u64>, bd: u64) -> SparkplugPayload {
    SparkplugPayload {
        kind,
        seq: seq.and_then(|v| SparkplugSeq::new(v).ok()),
        bd_seq: BdSeq::from_raw(bd),
        metrics: vec![metric()],
    }
}

fn accept(obs: Observation) -> NodeSessionState {
    match obs {
        Observation::Accepted { state_after } => state_after,
        other => panic!("expected acceptance, got {other:?}"),
    }
}

#[test]
fn the_full_session_lifecycle_birth_data_death_rebirth() {
    let mut tracker = BirthDeathTracker::new();
    // Nothing is trusted before the BIRTH.
    assert_eq!(
        tracker.observe(&payload(SparkplugMessageKind::NodeData, Some(1), 0)),
        Observation::Refused {
            reason: RefusalReason::DataWithoutLiveBirth,
            state_unchanged: NodeSessionState::AwaitingBirth,
        }
    );
    // BIRTH opens generation 3 at seq 0.
    let state = accept(tracker.observe(&payload(SparkplugMessageKind::NodeBirth, Some(0), 3)));
    assert_eq!(
        state,
        NodeSessionState::Live {
            seq: SparkplugSeq::new(0).expect("valid"),
            bd_seq: BdSeq::from_raw(3),
        }
    );
    // The chain continues 1, 2 within generation 3.
    let state = accept(tracker.observe(&payload(SparkplugMessageKind::NodeData, Some(1), 3)));
    assert!(matches!(state, NodeSessionState::Live { .. }));
    let state = accept(tracker.observe(&payload(SparkplugMessageKind::NodeData, Some(2), 3)));
    assert!(matches!(state, NodeSessionState::Live { .. }));
    // DEATH (generation 3) ends the session.
    let state = accept(tracker.observe(&payload(SparkplugMessageKind::NodeDeath, None, 3)));
    assert_eq!(
        state,
        NodeSessionState::Dead {
            reason: DeathReason::DeathMessage,
            bd_seq: BdSeq::from_raw(3),
        }
    );
    // Re-BIRTH opens a NEW generation; the chain restarts.
    let state = accept(tracker.observe(&payload(SparkplugMessageKind::NodeBirth, Some(0), 4)));
    assert_eq!(
        state,
        NodeSessionState::Live {
            seq: SparkplugSeq::new(0).expect("valid"),
            bd_seq: BdSeq::from_raw(4),
        }
    );
}

#[test]
fn every_refusal_is_named_and_leaves_the_state_unchanged() {
    let mut tracker = BirthDeathTracker::new();
    accept(tracker.observe(&payload(SparkplugMessageKind::NodeBirth, Some(0), 3)));

    // Sequence gap: named, state unchanged.
    let refused = tracker.observe(&payload(SparkplugMessageKind::NodeData, Some(7), 3));
    match &refused {
        Observation::Refused { reason, .. } => {
            assert_eq!(reason.name(), "sequence_mismatch");
        }
        other => panic!("expected refusal, got {other:?}"),
    }
    assert!(matches!(tracker.state(), NodeSessionState::Live { .. }));

    // Stale-generation DATA: named, state unchanged.
    let refused = tracker.observe(&payload(SparkplugMessageKind::NodeData, Some(1), 2));
    match &refused {
        Observation::Refused { reason, .. } => {
            assert_eq!(reason.name(), "generation_mismatch");
        }
        other => panic!("expected refusal, got {other:?}"),
    }

    // Same-generation re-birth: named, state unchanged.
    let refused = tracker.observe(&payload(SparkplugMessageKind::NodeBirth, Some(0), 3));
    match &refused {
        Observation::Refused { reason, .. } => {
            assert_eq!(reason.name(), "stale_rebirth");
        }
        other => panic!("expected refusal, got {other:?}"),
    }
}

#[test]
fn data_without_a_live_birth_is_never_trusted() {
    let mut tracker = BirthDeathTracker::new();
    // Even a fully-formed payload from a device is refused before any
    // BIRTH — the inventory is unknown, so the value means nothing.
    assert!(matches!(
        tracker.observe(&payload(SparkplugMessageKind::DeviceData, Some(1), 0)),
        Observation::Refused {
            reason: RefusalReason::DataWithoutLiveBirth,
            ..
        }
    ));
    assert!(matches!(tracker.state(), NodeSessionState::AwaitingBirth));
}

#[test]
fn sequence_wraps_under_the_declared_modulus() {
    let mut tracker = BirthDeathTracker::new();
    let last = SEQ_MODULUS - 1;
    accept(tracker.observe(&payload(SparkplugMessageKind::NodeBirth, Some(last), 0)));
    let state = accept(tracker.observe(&payload(SparkplugMessageKind::NodeData, Some(0), 0)));
    match state {
        NodeSessionState::Live { seq, .. } => assert_eq!(seq.value(), 0, "wrapped to 0"),
        other => panic!("expected a live session, got {other:?}"),
    }
}

#[test]
fn connection_lost_ends_the_session_and_rebirth_recovers() {
    let mut tracker = BirthDeathTracker::new();
    // No live session: a named no-op.
    assert!(tracker.connection_lost().is_none());
    accept(tracker.observe(&payload(SparkplugMessageKind::NodeBirth, Some(0), 9)));
    let lost = tracker.connection_lost().expect("live session ends");
    match &lost {
        Observation::Accepted { state_after } => assert_eq!(
            state_after,
            &NodeSessionState::Dead {
                reason: DeathReason::ConnectionLost,
                bd_seq: BdSeq::from_raw(9),
            }
        ),
        other => panic!("expected acceptance, got {other:?}"),
    }
    // A new BIRTH recovers; the machine does not need a DEATH first.
    let state = accept(tracker.observe(&payload(SparkplugMessageKind::NodeBirth, Some(0), 10)));
    assert!(matches!(state, NodeSessionState::Live { .. }));
}

#[test]
fn birth_and_data_payloads_without_seq_are_invalid_by_construction() {
    let mut tracker = BirthDeathTracker::new();
    // A BIRTH without a seq fails payload validation → invalid_payload.
    assert_eq!(
        tracker.observe(&payload(SparkplugMessageKind::NodeBirth, None, 0)),
        Observation::Refused {
            reason: RefusalReason::InvalidPayload,
            state_unchanged: NodeSessionState::AwaitingBirth,
        }
    );
}

#[test]
fn the_state_kind_is_recorded_never_guessed_into_a_session_effect() {
    let mut tracker = BirthDeathTracker::new();
    accept(tracker.observe(&payload(SparkplugMessageKind::NodeBirth, Some(0), 3)));
    // STATE is not a session observation: refused BY NAME, session intact.
    assert_eq!(
        tracker.observe(&payload(SparkplugMessageKind::State, None, 3)),
        Observation::Refused {
            reason: RefusalReason::InvalidPayload,
            state_unchanged: NodeSessionState::Live {
                seq: SparkplugSeq::new(0).expect("valid"),
                bd_seq: BdSeq::from_raw(3),
            },
        }
    );
}
