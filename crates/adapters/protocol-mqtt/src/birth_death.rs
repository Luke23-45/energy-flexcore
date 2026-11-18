// P14.11 — The Sparkplug BIRTH/DEATH session state machine (decision
// record 43_p14_11_mqtt_sparkplug_b_uplink.md; sources [S-39][S-40]).
//
// WHY THIS MACHINE EXISTS (the subtask's core deliverable): Sparkplug's
// state management is what makes MQTT usable for DER fleets — a BIRTH
// declares the FULL metric inventory of an edge node or device on every
// (re)connection; a DEATH declares disconnection (the MQTT will). A
// consumer that trusts DATA without a live BIRTH, or that keeps accepting
// metrics after a DEATH, is silently ingesting an inventory that no longer
// exists. This machine makes the trust rules STRUCTURAL: every observation
// is accepted or refused BY NAME, never guessed.
//
// UNVERIFIED HONESTY: the ordering rules below encode the secondary-source
// reading of Sparkplug B semantics ([S-39]; OpenFMB [S-40]): BIRTH before
// DATA, DEATH ends the session, a new BIRTH after a session end restarts
// the session with a new bdSeq generation, seq restarts at each BIRTH and
// wraps at the DECLARED modulus. The implementation-time check against the
// Sparkplug B specification text confirms: whether seq may be 0 at BIRTH,
// whether out-of-order seq is a protocol error or a consumer concern, the
// exact rebirth/STATE interaction, and which messages carry a seq. A
// mismatch is a recorded edit to this machine, never a silent
// reinterpretation.
//
// SCOPE NOTE: this is the CONSUMER-side trust machine for the platform
// side of the uplink. The edge node's own publishing behavior (when to
// emit BIRTH, what the will payload is) is the [`crate::uplink`] plan's
// design surface — this module judges observations, it does not produce
// messages.

use serde::{Deserialize, Serialize};

use crate::model::{BdSeq, SparkplugPayload, SparkplugSeq};

/// Why a session ended (human-safe reasons; the P1.08 rule — never raw
/// broker text).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeathReason {
    /// A DEATH message was observed for the live session.
    DeathMessage,
    /// The transport seam reported the session's connection as lost (the
    /// broker-side will would fire, but the platform-side machine also
    /// ends the session — the two agree by design).
    ConnectionLost,
}

/// The session state of ONE edge node (or one device under it — the same
/// machine shape applies at both scopes).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeSessionState {
    /// No BIRTH observed since the last session end (or since
    /// construction). No DATA may be trusted.
    AwaitingBirth,
    /// A BIRTH declared the inventory and the session is live. `seq` is
    /// the message sequence the last accepted message carried; `bd_seq`
    /// identifies the session generation.
    Live {
        seq: SparkplugSeq,
        bd_seq: BdSeq,
    },
    /// The session ended. No DATA may be trusted until a new BIRTH.
    Dead {
        reason: DeathReason,
        bd_seq: BdSeq,
    },
}

/// Why an observation was refused (named, never silent).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefusalReason {
    /// DATA arrived while the session was not live (AwaitingBirth or
    /// Dead). The inventory is unknown — the data is not trusted.
    DataWithoutLiveBirth,
    /// A DEATH arrived while no session was live (unexpected; the machine
    /// refuses it rather than re-ending an already-dead session).
    DeathWithoutLiveBirth,
    /// The payload's sequence did not continue the live chain (the
    /// declared order rule; UNVERIFIED — see the module documentation).
    SequenceMismatch {
        expected: u64,
        observed: u64,
    },
    /// A BIRTH arrived for a generation that is already live (a
    /// same-generation re-birth is refused — a silent inventory reset
    /// would hide a publisher defect). UNVERIFIED ordering rule.
    StaleRebirth,
    /// The payload's bdSeq does not identify the live session's
    /// generation (a stale DEATH or DATA from an older generation is
    /// refused — it must not disturb the newer session).
    GenerationMismatch,
    /// The payload failed its own validation (missing seq for BIRTH/DATA),
    /// or the kind is not a session observation at all (STATE / command
    /// kinds — recorded, never guessed into a session effect).
    InvalidPayload,
}

impl RefusalReason {
    /// Canonical name for records and logs (the P11 taxonomy consumers
    /// map onto these names; P14.21 consumes, never redefines).
    pub const fn name(self) -> &'static str {
        match self {
            RefusalReason::DataWithoutLiveBirth => "data_without_live_birth",
            RefusalReason::DeathWithoutLiveBirth => "death_without_live_birth",
            RefusalReason::SequenceMismatch { .. } => "sequence_mismatch",
            RefusalReason::StaleRebirth => "stale_rebirth",
            RefusalReason::GenerationMismatch => "generation_mismatch",
            RefusalReason::InvalidPayload => "invalid_payload",
        }
    }
}

/// The outcome of one observation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Observation {
    /// The observation was accepted and the state transitioned accordingly.
    Accepted {
        state_after: NodeSessionState,
    },
    /// The observation was refused; the state did not change. The refusal
    /// is a RECORD, never a silent drop.
    Refused {
        reason: RefusalReason,
        state_unchanged: NodeSessionState,
    },
}

impl Observation {
    /// The post-observation state (accepted or unchanged) — the machine's
    /// honest "where are we now" answer for either outcome.
    pub fn state(&self) -> &NodeSessionState {
        match self {
            Observation::Accepted { state_after } => state_after,
            Observation::Refused { state_unchanged, .. } => state_unchanged,
        }
    }
}

/// The BIRTH/DEATH session tracker for one edge node.
///
/// Constructed with [`BirthDeathTracker::new`] (starts
/// [`NodeSessionState::AwaitingBirth`]); advanced ONLY through
/// [`BirthDeathTracker::observe`] and
/// [`BirthDeathTracker::connection_lost`] — the state is not publicly
/// writable, so a skipped BIRTH is not expressible.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BirthDeathTracker {
    state: NodeSessionState,
}

impl BirthDeathTracker {
    pub fn new() -> Self {
        Self {
            state: NodeSessionState::AwaitingBirth,
        }
    }

    /// The current session state.
    pub fn state(&self) -> &NodeSessionState {
        &self.state
    }

    /// Observes one payload (BIRTH, DATA, or DEATH; STATE handling is a
    /// design note in the decision record, not a state of this machine).
    /// Every outcome is named.
    pub fn observe(&mut self, payload: &SparkplugPayload) -> Observation {
        if !payload.kind.is_birth() && !payload.kind.is_death() && !payload.kind.is_data() {
            // STATE and command kinds are not session observations; the
            // machine records the refusal by name rather than guessing an
            // effect on the session.
            return Observation::Refused {
                reason: RefusalReason::InvalidPayload,
                state_unchanged: self.state.clone(),
            };
        }
        if payload.validate().is_err() {
            return Observation::Refused {
                reason: RefusalReason::InvalidPayload,
                state_unchanged: self.state.clone(),
            };
        }
        if payload.kind.is_birth() {
            return self.observe_birth(payload);
        }
        if payload.kind.is_death() {
            return self.observe_death(payload);
        }
        self.observe_data(payload)
    }

    fn observe_birth(&mut self, payload: &SparkplugPayload) -> Observation {
        let seq = match payload.seq {
            Some(seq) => seq,
            None => {
                return Observation::Refused {
                    reason: RefusalReason::InvalidPayload,
                    state_unchanged: self.state.clone(),
                }
            }
        };
        match &self.state {
            NodeSessionState::Live { bd_seq, .. } => {
                // A BIRTH on a live session is a RE-BIRTH: the declared
                // rule accepts it ONLY as a generation change — a new
                // bdSeq. Same-generation re-birth is refused (the session
                // inventory was already declared; a silent reset would
                // hide a publisher defect). UNVERIFIED ordering rule —
                // see the module documentation.
                if *bd_seq != payload.bd_seq {
                    let new_state = NodeSessionState::Live {
                        seq,
                        bd_seq: payload.bd_seq,
                    };
                    self.state = new_state.clone();
                    Observation::Accepted { state_after: new_state }
                } else {
                    Observation::Refused {
                        reason: RefusalReason::StaleRebirth,
                        state_unchanged: self.state.clone(),
                    }
                }
            }
            NodeSessionState::AwaitingBirth | NodeSessionState::Dead { .. } => {
                // A BIRTH starts (or restarts) the session. seq restarts
                // with the BIRTH's own value; bdSeq identifies the new
                // generation.
                let new_state = NodeSessionState::Live {
                    seq,
                    bd_seq: payload.bd_seq,
                };
                self.state = new_state.clone();
                Observation::Accepted { state_after: new_state }
            }
        }
    }

    fn observe_death(&mut self, payload: &SparkplugPayload) -> Observation {
        match &self.state {
            NodeSessionState::Live { bd_seq, .. } => {
                // The DEATH ends the live session. The declared rule
                // requires the DEATH to carry the LIVE generation's bdSeq
                // — a stale DEATH from an older generation is refused
                // (it must not kill the newer session). UNVERIFIED
                // ordering rule — see the module documentation.
                if *bd_seq == payload.bd_seq {
                    let new_state = NodeSessionState::Dead {
                        reason: DeathReason::DeathMessage,
                        bd_seq: payload.bd_seq,
                    };
                    self.state = new_state.clone();
                    Observation::Accepted { state_after: new_state }
                } else {
                    Observation::Refused {
                        reason: RefusalReason::GenerationMismatch,
                        state_unchanged: self.state.clone(),
                    }
                }
            }
            NodeSessionState::AwaitingBirth | NodeSessionState::Dead { .. } => {
                Observation::Refused {
                    reason: RefusalReason::DeathWithoutLiveBirth,
                    state_unchanged: self.state.clone(),
                }
            }
        }
    }

    fn observe_data(&mut self, payload: &SparkplugPayload) -> Observation {
        let NodeSessionState::Live { seq, bd_seq } = &self.state else {
            return Observation::Refused {
                reason: RefusalReason::DataWithoutLiveBirth,
                state_unchanged: self.state.clone(),
            };
        };
        // The generation rule: a DATA message belongs to the live
        // session's bdSeq generation; anything else is refused.
        if payload.bd_seq != *bd_seq {
            return Observation::Refused {
                reason: RefusalReason::GenerationMismatch,
                state_unchanged: self.state.clone(),
            };
        }
        // The sequence rule (declared, UNVERIFIED): a DATA message must
        // continue the live session's sequence chain — its seq is the
        // successor of the last accepted seq. A gap is a refusal, never a
        // silent skip (the consumer's honesty rule: a missed message is
        // RECORDED, and the recovery path is a re-BIRTH, not a guess).
        let observed = match payload.seq {
            Some(seq) => seq,
            None => {
                return Observation::Refused {
                    reason: RefusalReason::InvalidPayload,
                    state_unchanged: self.state.clone(),
                }
            }
        };
        let expected = seq.next();
        if observed.value() != expected.value() {
            return Observation::Refused {
                reason: RefusalReason::SequenceMismatch {
                    expected: expected.value(),
                    observed: observed.value(),
                },
                state_unchanged: self.state.clone(),
            };
        }
        let new_state = NodeSessionState::Live {
            seq: observed,
            bd_seq: *bd_seq,
        };
        self.state = new_state.clone();
        Observation::Accepted { state_after: new_state }
    }

    /// The transport seam reports the connection lost. The session ends
    /// with [`DeathReason::ConnectionLost`] if it was live; an already
    /// dead or never-live session is unchanged (a named no-op).
    pub fn connection_lost(&mut self) -> Option<Observation> {
        let NodeSessionState::Live { bd_seq, .. } = &self.state else {
            return None;
        };
        let new_state = NodeSessionState::Dead {
            reason: DeathReason::ConnectionLost,
            bd_seq: *bd_seq,
        };
        self.state = new_state.clone();
        Some(Observation::Accepted { state_after: new_state })
    }
}

impl Default for BirthDeathTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{MetricType, MetricValue, SparkplugMetric};

    fn payload(kind: SparkplugMessageKind, seq: Option<u64>, bd: u64) -> SparkplugPayload {
        SparkplugPayload {
            kind,
            seq: seq.and_then(|v| SparkplugSeq::new(v).ok()),
            bd_seq: BdSeq::from_raw(bd),
            metrics: vec![SparkplugMetric::new(
                "inverter_1.p_mw".to_string(),
                None,
                MetricType::Double,
                MetricValue::Double(3.5),
            )
            .expect("valid metric")],
        }
    }

    use crate::model::SparkplugMessageKind;

    fn accept(obs: Observation) -> NodeSessionState {
        match obs {
            Observation::Accepted { state_after } => state_after,
            other => panic!("expected acceptance, got {other:?}"),
        }
    }

    #[test]
    fn data_before_birth_is_refused_by_name() {
        let mut tracker = BirthDeathTracker::new();
        assert_eq!(
            tracker.observe(&payload(SparkplugMessageKind::NodeData, Some(1), 0)),
            Observation::Refused {
                reason: RefusalReason::DataWithoutLiveBirth,
                state_unchanged: NodeSessionState::AwaitingBirth,
            }
        );
    }

    #[test]
    fn non_session_kinds_are_refused_by_name() {
        let mut tracker = BirthDeathTracker::new();
        tracker.observe(&payload(SparkplugMessageKind::NodeBirth, Some(0), 3));
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

    #[test]
    fn birth_starts_the_session_and_data_must_continue_the_chain() {
        let mut tracker = BirthDeathTracker::new();
        let state = accept(tracker.observe(&payload(
            SparkplugMessageKind::NodeBirth,
            Some(0),
            3,
        )));
        assert_eq!(
            state,
            NodeSessionState::Live {
                seq: SparkplugSeq::new(0).expect("valid"),
                bd_seq: BdSeq::from_raw(3),
            }
        );
        // The next DATA must carry seq 1 (the declared chain rule).
        let state = accept(tracker.observe(&payload(
            SparkplugMessageKind::NodeData,
            Some(1),
            3,
        )));
        assert_eq!(
            state,
            NodeSessionState::Live {
                seq: SparkplugSeq::new(1).expect("valid"),
                bd_seq: BdSeq::from_raw(3),
            }
        );
        // A gap is refused BY NAME, never skipped.
        assert_eq!(
            tracker.observe(&payload(SparkplugMessageKind::NodeData, Some(5), 3)),
            Observation::Refused {
                reason: RefusalReason::SequenceMismatch {
                    expected: 2,
                    observed: 5,
                },
                state_unchanged: NodeSessionState::Live {
                    seq: SparkplugSeq::new(1).expect("valid"),
                    bd_seq: BdSeq::from_raw(3),
                },
            }
        );
    }

    #[test]
    fn death_ends_the_session_and_stale_death_is_refused() {
        let mut tracker = BirthDeathTracker::new();
        accept(tracker.observe(&payload(SparkplugMessageKind::NodeBirth, Some(0), 3)));
        // A DEATH from an older generation must NOT kill the live session.
        assert_eq!(
            tracker.observe(&payload(SparkplugMessageKind::NodeDeath, None, 1)),
            Observation::Refused {
                reason: RefusalReason::GenerationMismatch,
                state_unchanged: NodeSessionState::Live {
                    seq: SparkplugSeq::new(0).expect("valid"),
                    bd_seq: BdSeq::from_raw(3),
                },
            }
        );
        // The live generation's DEATH ends the session.
        let state = accept(tracker.observe(&payload(
            SparkplugMessageKind::NodeDeath,
            None,
            3,
        )));
        assert_eq!(
            state,
            NodeSessionState::Dead {
                reason: DeathReason::DeathMessage,
                bd_seq: BdSeq::from_raw(3),
            }
        );
        // DATA after death is refused.
        assert_eq!(
            tracker.observe(&payload(SparkplugMessageKind::NodeData, Some(1), 3)),
            Observation::Refused {
                reason: RefusalReason::DataWithoutLiveBirth,
                state_unchanged: NodeSessionState::Dead {
                    reason: DeathReason::DeathMessage,
                    bd_seq: BdSeq::from_raw(3),
                },
            }
        );
        // A second DEATH is refused (nothing live to end).
        assert_eq!(
            tracker.observe(&payload(SparkplugMessageKind::NodeDeath, None, 3)),
            Observation::Refused {
                reason: RefusalReason::DeathWithoutLiveBirth,
                state_unchanged: NodeSessionState::Dead {
                    reason: DeathReason::DeathMessage,
                    bd_seq: BdSeq::from_raw(3),
                },
            }
        );
    }

    #[test]
    fn rebirth_requires_a_new_generation() {
        let mut tracker = BirthDeathTracker::new();
        accept(tracker.observe(&payload(SparkplugMessageKind::NodeBirth, Some(0), 3)));
        // Same-generation re-birth refused.
        assert_eq!(
            tracker.observe(&payload(SparkplugMessageKind::NodeBirth, Some(0), 3)),
            Observation::Refused {
                reason: RefusalReason::StaleRebirth,
                state_unchanged: NodeSessionState::Live {
                    seq: SparkplugSeq::new(0).expect("valid"),
                    bd_seq: BdSeq::from_raw(3),
                },
            }
        );
        // A new generation re-birth is accepted and restarts the chain.
        let state = accept(tracker.observe(&payload(
            SparkplugMessageKind::NodeBirth,
            Some(0),
            4,
        )));
        assert_eq!(
            state,
            NodeSessionState::Live {
                seq: SparkplugSeq::new(0).expect("valid"),
                bd_seq: BdSeq::from_raw(4),
            }
        );
    }

    #[test]
    fn seq_wraps_across_the_declared_modulus() {
        let mut tracker = BirthDeathTracker::new();
        accept(tracker.observe(&payload(
            SparkplugMessageKind::NodeBirth,
            Some(crate::model::SEQ_MODULUS - 1),
            0,
        )));
        // The successor of (modulus - 1) is 0 under the declared wrap.
        let state = accept(tracker.observe(&payload(
            SparkplugMessageKind::NodeData,
            Some(0),
            0,
        )));
        assert!(matches!(state, NodeSessionState::Live { .. }));
    }

    #[test]
    fn connection_lost_ends_a_live_session_and_is_a_noop_otherwise() {
        let mut tracker = BirthDeathTracker::new();
        assert!(tracker.connection_lost().is_none());
        accept(tracker.observe(&payload(SparkplugMessageKind::NodeBirth, Some(0), 7)));
        let lost = tracker.connection_lost().expect("live session ends");
        assert_eq!(
            lost,
            Observation::Accepted {
                state_after: NodeSessionState::Dead {
                    reason: DeathReason::ConnectionLost,
                    bd_seq: BdSeq::from_raw(7),
                }
            }
        );
    }
}
