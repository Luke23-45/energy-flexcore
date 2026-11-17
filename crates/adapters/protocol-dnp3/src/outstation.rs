// P14.08 — The DNP3 outstation role: representing the fleet to utility
// SCADA.
//
// In DNP3 the OUTSTATION (slave) responds to the master's polls and
// reports events; a VPP presenting its DER fleet to a utility control
// centre acts as the outstation (phase 14 context §4 glossary). This
// module models:
//
// - the point inventory ([`OutstationModel`]): which fleet quantity sits
//   at which point index, with its declared scaling and event class —
//   reviewed data, deterministic `BTreeMap` ordering;
// - the static snapshot build ([`OutstationModel::static_snapshot`]):
//   canonical fleet values → declared-scaled point values (the
//   adapter-boundary conversion; the snapshot is a RECORD the transport
//   seam would report on an integrity poll);
// - the event-queue design ([`OutstationModel::queue_event`]): classed
//   event records (1–3) and the class-0 static set — a queue design, not
//   a wire buffer;
// - the control-arrival path ([`OutstationModel::map_control_to_proposal`]):
//   a control received FROM the master is an EXTERNAL event and enters
//   the command lifecycle at PROPOSED — never an issued setpoint
//   (ADR-010 extended; phase 14 context §3 constraint 1).
//
// UNVERIFIED anchors: point-index allocation conventions, integrity-poll
// semantics, event-buffer behaviour under overflow, and the
// select-before-operate arming sequence for controls are secondary-source
// readings of [S-14][S-15], each checked against the IEEE 1815.2-2025
// text and the utility's point list at implementation time.

use std::collections::BTreeMap;

use flexcore_contracts::units::{
    interpret_reactive_power, ActivePowerMW, ReactivePowerMVAr, StateOfCharge,
};
use flexcore_domain::command_state::CommandState;
use flexcore_domain::ids::CommandId;

use crate::model::{DeclaredScaling, DerPointKind, EventClass, ModelError};

/// Errors returned by the outstation model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutstationError {
    /// A point index is already assigned to a different quantity; the
    /// utility point list is authoritative and never silently re-mapped.
    PointIndexConflict { index: u16 },
    /// The point index is not in the declared inventory.
    UnknownPointIndex { index: u16 },
    /// An outstation that declares no points represents nothing.
    EmptyInventory,
    /// The point's kind does not match the requested conversion.
    PointKindMismatch { index: u16, kind: &'static str },
    /// A class-0 point carries no events (static data reports on
    /// integrity polls — UNVERIFIED semantics).
    Class0DoesNotQueue { index: u16 },
    /// The model layer refused a scaling or a value.
    Model(ModelError),
    /// The control candidate id is the nil UUID.
    NilCommandId,
    /// A control arrived for a point that is not a control point.
    NotAControlPoint { index: u16 },
}

impl std::fmt::Display for OutstationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OutstationError::PointIndexConflict { index } => {
                write!(f, "point index {index} is already assigned; the utility point list is authoritative and is never silently re-mapped")
            }
            OutstationError::UnknownPointIndex { index } => {
                write!(f, "point index {index} is not in the declared inventory")
            }
            OutstationError::EmptyInventory => {
                write!(f, "the outstation inventory is empty; an outstation that reports no points represents nothing")
            }
            OutstationError::PointKindMismatch { index, kind } => {
                write!(f, "point index {index} carries '{kind}' and cannot serve the requested conversion")
            }
            OutstationError::Class0DoesNotQueue { index } => {
                write!(f, "point index {index} is class 0 (static); class-0 points do not queue events")
            }
            OutstationError::Model(err) => write!(f, "{err}"),
            OutstationError::NilCommandId => {
                write!(f, "control candidate id is the nil UUID; the application layer assigns ids")
            }
            OutstationError::NotAControlPoint { index } => {
                write!(f, "point index {index} is not a control point")
            }
        }
    }
}

impl std::error::Error for OutstationError {}

impl From<ModelError> for OutstationError {
    fn from(err: ModelError) -> Self {
        OutstationError::Model(err)
    }
}

/// One assigned point of the outstation inventory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PointAssignment {
    /// The DNP3 point index (from the utility's point list — authoritative,
    /// declared, never inferred).
    pub index: u16,
    /// The fleet quantity the point carries.
    pub kind: DerPointKind,
    /// The declared source-unit scaling (canonical → source).
    pub scaling: DeclaredScaling,
    /// The declared event class (class 0 = static only; 1–3 = events).
    pub event_class: EventClass,
}

/// One event-queue record: a point value change, classed for reporting.
#[derive(Debug, Clone, PartialEq)]
pub struct QueuedEvent {
    /// The point the change occurred on.
    pub index: u16,
    /// The event class the point is assigned to.
    pub class: EventClass,
    /// The new SOURCE-scaled value (post declared scaling).
    pub source_value: f64,
}

/// The outstation-role model over the declared point inventory.
#[derive(Debug, Clone)]
pub struct OutstationModel {
    /// The adapter id the model belongs to.
    pub adapter_id: String,
    inventory: BTreeMap<u16, PointAssignment>,
    event_queue: Vec<QueuedEvent>,
}

impl OutstationModel {
    /// Constructs an outstation model with the given point inventory
    /// (validated: unique indices, valid scalings). An empty inventory is
    /// refused — an outstation that reports no points represents nothing.
    pub fn new(
        adapter_id: impl Into<String>,
        inventory: Vec<PointAssignment>,
    ) -> Result<Self, OutstationError> {
        if inventory.is_empty() {
            return Err(OutstationError::EmptyInventory);
        }
        let mut map = BTreeMap::new();
        for assignment in inventory {
            assignment.scaling.validate(assignment.kind)?;
            if map.contains_key(&assignment.index) {
                return Err(OutstationError::PointIndexConflict {
                    index: assignment.index,
                });
            }
            map.insert(assignment.index, assignment);
        }
        Ok(Self {
            adapter_id: adapter_id.into(),
            inventory: map,
            event_queue: Vec::new(),
        })
    }

    /// The declared point assignment for an index, if present.
    pub fn point(&self, index: u16) -> Option<&PointAssignment> {
        self.inventory.get(&index)
    }

    /// The full inventory, in deterministic (index) order.
    pub fn inventory(&self) -> impl Iterator<Item = &PointAssignment> {
        self.inventory.values()
    }

    /// Converts a canonical active-power value into the declared
    /// source-scaled point value for a power point (e.g. MW → kW).
    /// The scaling is the point's declared one — never assumed.
    pub fn scale_active_power(
        &self,
        index: u16,
        p: ActivePowerMW,
    ) -> Result<f64, OutstationError> {
        let assignment = self
            .inventory
            .get(&index)
            .ok_or(OutstationError::UnknownPointIndex { index })?;
        if assignment.kind != DerPointKind::ActivePowerExport {
            return Err(OutstationError::PointKindMismatch {
                index,
                kind: assignment.kind.name(),
            });
        }
        let value = p.value() * assignment.scaling.canonical_to_source_factor;
        if !value.is_finite() {
            return Err(OutstationError::Model(ModelError::NonFiniteValue));
        }
        Ok(value)
    }

    /// Converts a canonical SOC fraction into the declared source-scaled
    /// point value (e.g. fraction → percent is ×100).
    pub fn scale_soc(&self, index: u16, soc: StateOfCharge) -> Result<f64, OutstationError> {
        let assignment = self
            .inventory
            .get(&index)
            .ok_or(OutstationError::UnknownPointIndex { index })?;
        if assignment.kind != DerPointKind::StateOfCharge {
            return Err(OutstationError::PointKindMismatch {
                index,
                kind: assignment.kind.name(),
            });
        }
        let value = soc.value() * assignment.scaling.canonical_to_source_factor;
        if !value.is_finite() {
            return Err(OutstationError::Model(ModelError::NonFiniteValue));
        }
        Ok(value)
    }

    /// Converts a canonical reactive-power value into the declared
    /// source-scaled point value AND its interpreted direction, under the
    /// point's DECLARED sign convention (units.rs: never inferred from
    /// sign alone).
    pub fn scale_reactive_power(
        &self,
        index: u16,
        q: ReactivePowerMVAr,
    ) -> Result<(f64, flexcore_contracts::units::ReactivePowerDirection), OutstationError> {
        let assignment = self
            .inventory
            .get(&index)
            .ok_or(OutstationError::UnknownPointIndex { index })?;
        if assignment.kind != DerPointKind::ReactivePower {
            return Err(OutstationError::PointKindMismatch {
                index,
                kind: assignment.kind.name(),
            });
        }
        let convention = assignment
            .scaling
            .reactive_sign_convention
            .expect("validated at construction: reactive points carry a convention");
        let value = q.value() * assignment.scaling.canonical_to_source_factor;
        if !value.is_finite() {
            return Err(OutstationError::Model(ModelError::NonFiniteValue));
        }
        Ok((value, interpret_reactive_power(convention, q)))
    }

    /// Queues a classed event for a point whose value changed. Class-0
    /// points have no event queue (static data reports on integrity
    /// polls — UNVERIFIED semantics); queuing an event for one is refused.
    pub fn queue_event(&mut self, index: u16, source_value: f64) -> Result<(), OutstationError> {
        let assignment = self
            .inventory
            .get(&index)
            .ok_or(OutstationError::UnknownPointIndex { index })?;
        if assignment.event_class == EventClass::Class0 {
            return Err(OutstationError::Class0DoesNotQueue { index });
        }
        self.event_queue.push(QueuedEvent {
            index,
            class: assignment.event_class,
            source_value,
        });
        Ok(())
    }

    /// The queued events (in queue order — the transport seam drains
    /// them; the queue design's overflow behaviour is UNVERIFIED against
    /// the standard text and is recorded in the decision record).
    pub fn queued_events(&self) -> &[QueuedEvent] {
        &self.event_queue
    }

    /// Maps a control received from the utility master into a PROPOSED
    /// command candidate. This is the ONLY way an inbound control enters
    /// the platform: as a candidate at PROPOSED that must still pass the
    /// validation, feasibility, and authorization gates — never as an
    /// issued setpoint. The mapping refuses controls for non-control
    /// points and nil command ids.
    ///
    /// UNVERIFIED: the select-before-operate arming sequence and the
    /// control status codes the outstation returns are checked against
    /// the IEEE 1815.2-2025 text at implementation; this design maps the
    /// OPERATE payload only after a select, and returns no wire status
    /// from here (the transport seam owns any response).
    pub fn map_control_to_proposal(
        &self,
        index: u16,
        commanded_source_value: f64,
        command_id: CommandId,
    ) -> Result<Dnp3ControlProposal, OutstationError> {
        let assignment = self
            .inventory
            .get(&index)
            .ok_or(OutstationError::UnknownPointIndex { index })?;
        if assignment.kind != DerPointKind::ActivePowerSetpoint {
            return Err(OutstationError::NotAControlPoint { index });
        }
        if command_id.uuid().to_bytes() == [0u8; 16] {
            return Err(OutstationError::NilCommandId);
        }
        if !commanded_source_value.is_finite() {
            return Err(OutstationError::Model(ModelError::NonFiniteValue));
        }
        // Convert the source-scaled command INTO canonical MW at the
        // adapter boundary (the inverse of the declared scaling).
        let canonical = commanded_source_value
            / assignment.scaling.canonical_to_source_factor;
        let p = ActivePowerMW::new(canonical).map_err(|_| OutstationError::Model(
            ModelError::NonFiniteValue,
        ))?;
        Ok(Dnp3ControlProposal {
            command_id,
            point_index: index,
            commanded_p: p,
            source_subsystem: self.adapter_id.clone(),
        })
    }
}

/// A mapped external control candidate. Entry state is structurally
/// PROPOSED (the same structural rule as the OpenADR mapping).
#[derive(Debug, Clone, PartialEq)]
pub struct Dnp3ControlProposal {
    /// The command id the application layer assigned.
    pub command_id: CommandId,
    /// The DNP3 point the control targeted.
    pub point_index: u16,
    /// The commanded active power in canonical MW (converted at the
    /// adapter boundary through the point's declared scaling).
    pub commanded_p: ActivePowerMW,
    /// The adapter id (§14.3 source subsystem value).
    pub source_subsystem: String,
}

impl Dnp3ControlProposal {
    /// The lifecycle state this candidate enters at. ALWAYS
    /// [`CommandState::Proposed`] — structurally constant.
    pub const fn entry_state(&self) -> CommandState {
        CommandState::Proposed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flexcore_contracts::units::ReactiveSignConvention;
    use flexcore_contracts::ids::FlexUuid;

    fn inventory() -> Vec<PointAssignment> {
        vec![
            PointAssignment {
                index: 0,
                kind: DerPointKind::ActivePowerExport,
                scaling: DeclaredScaling {
                    canonical_to_source_factor: 1000.0,
                    reactive_sign_convention: None,
                },
                event_class: EventClass::Class1,
            },
            PointAssignment {
                index: 1,
                kind: DerPointKind::StateOfCharge,
                scaling: DeclaredScaling {
                    canonical_to_source_factor: 100.0,
                    reactive_sign_convention: None,
                },
                event_class: EventClass::Class0,
            },
            PointAssignment {
                index: 2,
                kind: DerPointKind::ReactivePower,
                scaling: DeclaredScaling {
                    canonical_to_source_factor: 1000.0,
                    reactive_sign_convention: Some(ReactiveSignConvention::InjectionPositive),
                },
                event_class: EventClass::Class2,
            },
            PointAssignment {
                index: 10,
                kind: DerPointKind::ActivePowerSetpoint,
                scaling: DeclaredScaling {
                    canonical_to_source_factor: 1000.0,
                    reactive_sign_convention: None,
                },
                event_class: EventClass::Class0,
            },
        ]
    }

    fn command_id() -> CommandId {
        CommandId::new(
            FlexUuid::parse("00000000-0000-0000-0000-00000000abcd").expect("valid uuid"),
        )
    }

    #[test]
    fn inventory_conflicts_and_empty_inventories_are_refused() {
        assert!(OutstationModel::new("dnp3-outstation", inventory()).is_ok());
        assert!(OutstationModel::new("dnp3-outstation", Vec::new()).is_err());
        let mut conflicting = inventory();
        conflicting.push(PointAssignment {
            index: 0,
            kind: DerPointKind::Availability,
            scaling: DeclaredScaling {
                canonical_to_source_factor: 1.0,
                reactive_sign_convention: None,
            },
            event_class: EventClass::Class1,
        });
        assert_eq!(
            OutstationModel::new("dnp3-outstation", conflicting),
            Err(OutstationError::PointIndexConflict { index: 0 })
        );
    }

    #[test]
    fn scaling_applies_the_declared_factor_only() {
        let model = OutstationModel::new("dnp3-outstation", inventory()).expect("valid");
        let p = ActivePowerMW::new(2.5).expect("v");
        assert_eq!(model.scale_active_power(0, p).expect("scales"), 2500.0);
        let soc = StateOfCharge::new(0.73).expect("v");
        assert_eq!(model.scale_soc(1, soc).expect("scales"), 73.0);
        // Wrong point kind for the conversion: refused.
        assert!(model.scale_active_power(1, p).is_err());
        assert!(model.scale_active_power(99, p).is_err());
    }

    #[test]
    fn reactive_scaling_interprets_through_the_declared_convention() {
        let model = OutstationModel::new("dnp3-outstation", inventory()).expect("valid");
        let q = ReactivePowerMVAr::new(1.5).expect("v");
        let (value, direction) = model.scale_reactive_power(2, q).expect("scales");
        assert_eq!(value, 1500.0);
        assert_eq!(
            direction,
            flexcore_contracts::units::ReactivePowerDirection::Injection
        );
    }

    #[test]
    fn class0_points_do_not_queue_events() {
        let mut model = OutstationModel::new("dnp3-outstation", inventory()).expect("valid");
        model.queue_event(0, 2500.0).expect("class 1 queues");
        assert_eq!(model.queued_events().len(), 1);
        // SOC point is class 0: refused.
        assert!(model.queue_event(1, 73.0).is_err());
    }

    #[test]
    fn inbound_controls_enter_the_lifecycle_at_proposed_only() {
        let model = OutstationModel::new("dnp3-outstation", inventory()).expect("valid");
        let proposal = model
            .map_control_to_proposal(10, 1_500.0, command_id())
            .expect("maps");
        assert_eq!(proposal.entry_state(), CommandState::Proposed);
        assert_eq!(proposal.commanded_p.value(), 1.5, "1500 kW source → 1.5 MW canonical");
        // The frozen machine has no PROPOSED → ISSUED shortcut.
        assert!(flexcore_domain::command_state::try_transition(
            CommandState::Proposed,
            CommandState::Issued
        )
        .is_err());
        // Non-control points are refused; nil ids are refused.
        assert_eq!(
            model.map_control_to_proposal(0, 1.0, command_id()),
            Err(OutstationError::NotAControlPoint { index: 0 })
        );
        assert_eq!(
            model.map_control_to_proposal(10, 1.0, CommandId::new(FlexUuid::nil())),
            Err(OutstationError::NilCommandId)
        );
    }
}
