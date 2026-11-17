// P17.09 — Departure-time constraint model (pure rules, no I/O).
//
// Implements ledger P17.09: the per-vehicle "energy required by departure
// time" constraints that vehicle calendars and the telematics port produce,
// feeding the P17.05 smart-charging schedule composer (Agent A's — consumed
// here BY CITATION; this module depends on nothing infrastructure).
// Decision record: `docs/phases-11-20/phase-17-ev-fleet-charging/records/
// 24_p17_09_departure_time_constraints.md`.
//
// # THE GOVERNING RULE (decided once in P3.11, consumed here — never
// relaxed by ingestion)
//
// The domain EV fleet model (`ev_fleet_model`) carries the HARD
// departure-energy policy: a vehicle must reach its required departure SOC
// by its departure instant in every feasible schedule
// ([`DepartureEnergyPolicy::Hard`]); any relaxation is a NAMED, REVIEWED
// decision recorded in the schedule. **Ingestion cannot relax this
// policy.** The constraint model below enforces the direction of change:
//
// - a constraint may TIGHTEN the requirement (raise the required departure
//   SOC, or move the departure instant earlier);
// - a constraint that would RELAX the requirement (lower the required SOC
//   than the fleet configuration declares, or a departure instant later
//   than the configuration's departure instant) is REFUSED at mapping time
//   with a named error ([`EvDepartureError::RelaxationRefused`]) — the
//   inbound value is recorded as data, but it never becomes the operative
//   constraint.
//
// The mapping function ([`map_constraint_onto_fleet_config`]) is the ONE
// place this decision lives; consumers feed its output to the composer.
//
// # Sources and provenance
//
// A constraint arrives from one of three declared sources: the vehicle's
// calendar (an operator-entered schedule), the telematics port (the
// P17-E-3 vendor-agnostic port — no vendor named), or an explicit operator
// declaration. The provenance is declared data carried on every
// constraint; an unknown-provenance constraint narrows (the P3.02 rule),
// it is never silently trusted.
//
// Control status: pure domain rules for the simulation-only 0.x line. No
// calendar was ingested, no telematics value was observed, no fleet
// operator is named.

use crate::ev_fleet_model::EvFleetConfig;
use flexcore_contracts::capability::Provenance;

/// The recorded governing statement (decided once; consumed from P3.11).
pub const DEPARTURE_CONSTRAINT_GOVERNING_STATEMENT: &str =
    "Departure energy is a HARD constraint in the domain EV fleet model: a \
vehicle must reach its required departure SOC by its departure instant in \
every feasible schedule. Ingestion may TIGHTEN this requirement and may \
never RELAX it: a constraint lowering the required SOC below the \
configured value, or moving the departure instant later than the \
configured departure, is refused at mapping time with a named error. Any \
relaxation remains a named, reviewed decision recorded in the schedule \
(P3.19) — never an ingestion outcome.";

/// The declared source of a departure-time constraint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DepartureConstraintSource {
    /// The vehicle's operator-entered calendar.
    VehicleCalendar,
    /// The vendor-agnostic telematics port (P17-E-3 DEFAULT (a); no vendor
    /// named as integrated).
    TelematicsPort,
    /// An explicit operator declaration (dispatch policy configuration).
    OperatorDeclared,
}

impl DepartureConstraintSource {
    /// All sources, in frozen vocabulary order.
    pub const ALL: [DepartureConstraintSource; 3] = [
        DepartureConstraintSource::VehicleCalendar,
        DepartureConstraintSource::TelematicsPort,
        DepartureConstraintSource::OperatorDeclared,
    ];

    /// The canonical name (the stored spelling).
    pub const fn name(self) -> &'static str {
        match self {
            DepartureConstraintSource::VehicleCalendar => "vehicle_calendar",
            DepartureConstraintSource::TelematicsPort => "telematics_port",
            DepartureConstraintSource::OperatorDeclared => "operator_declared",
        }
    }
}

/// Errors of the departure-constraint model. Every variant names the
/// offending vehicle and value (the P3.04 named-error style).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvDepartureError {
    /// The vehicle key was empty or whitespace-only.
    EmptyVehicleKey,
    /// The required departure SOC was outside [0,1] (boundary-inclusive).
    RequiredSocOutOfRange {
        /// The offending vehicle key.
        vehicle_key: String,
        /// The offending value.
        value: f64,
    },
    /// The required energy could not be derived (non-finite or negative
    /// capacity — the fleet configuration is the capacity source).
    InvalidBatteryCapacity {
        /// The offending value in MWh.
        value: f64,
    },
    /// THE RELAXATION REFUSAL: mapping this constraint onto the fleet
    /// configuration would relax the HARD departure-energy policy (a lower
    /// required SOC than configured, or a departure instant later than
    /// configured). The constraint is recorded as data; it never becomes
    /// the operative constraint.
    RelaxationRefused {
        /// The offending vehicle key.
        vehicle_key: String,
        /// The SOC the inbound constraint declared.
        constraint_required_soc: f64,
        /// The SOC the fleet configuration already requires (the floor).
        configured_required_soc: f64,
        /// Why the refusal fired (the direction of the relaxation).
        reason: String,
    },
    /// The constraint set already contains a constraint for this vehicle
    /// key; constraints are one-per-vehicle (a newer constraint supersedes
    /// through the explicit [`DepartureConstraintSet::supersede`] path,
    /// retaining the old record for audit).
    DuplicateVehicleConstraint {
        /// The duplicated vehicle key.
        vehicle_key: String,
    },
    /// The superseded-by timestamp was not strictly newer than the record
    /// it supersedes — a supersession that does not advance time is not an
    /// audit event.
    SupersessionNotNewer {
        /// The vehicle key.
        vehicle_key: String,
        /// The old record's received-at timestamp.
        old_received_at_unix_ms: i64,
        /// The new record's received-at timestamp.
        new_received_at_unix_ms: i64,
    },
    /// The vehicle key was not found in the set.
    UnknownVehicleKey {
        /// The unknown key.
        vehicle_key: String,
    },
}

impl std::fmt::Display for EvDepartureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EvDepartureError::EmptyVehicleKey => {
                write!(f, "the vehicle key is mandatory and must be non-empty")
            }
            EvDepartureError::RequiredSocOutOfRange { vehicle_key, value } => {
                write!(
                    f,
                    "vehicle '{vehicle_key}': the required departure SOC must be a fraction in [0,1] (found {value})"
                )
            }
            EvDepartureError::InvalidBatteryCapacity { value } => {
                write!(f, "the battery capacity must be a finite positive MWh value (found {value})")
            }
            EvDepartureError::RelaxationRefused {
                vehicle_key,
                constraint_required_soc,
                configured_required_soc,
                reason,
            } => write!(
                f,
                "vehicle '{vehicle_key}': the inbound constraint would RELAX the hard \
                 departure-energy policy (constraint SOC {constraint_required_soc} vs \
                 configured floor {configured_required_soc}; {reason}) — recorded as data, \
                 never operative"
            ),
            EvDepartureError::DuplicateVehicleConstraint { vehicle_key } => {
                write!(
                    f,
                    "vehicle '{vehicle_key}': the constraint set already contains a \
                     constraint; use the explicit supersede path"
                )
            }
            EvDepartureError::SupersessionNotNewer {
                vehicle_key,
                old_received_at_unix_ms,
                new_received_at_unix_ms,
            } => write!(
                f,
                "vehicle '{vehicle_key}': a supersession must be strictly newer \
                 (old {old_received_at_unix_ms} ms, new {new_received_at_unix_ms} ms)"
            ),
            EvDepartureError::UnknownVehicleKey { vehicle_key } => {
                write!(f, "vehicle '{vehicle_key}' has no constraint in the set")
            }
        }
    }
}

impl std::error::Error for EvDepartureError {}

/// One per-vehicle departure-time constraint, as ingested (declared data
/// with provenance). The operative constraint a consumer uses is the
/// MAPPED form ([`MappedDepartureConstraint`]) — never the raw inbound
/// record.
#[derive(Debug, Clone, PartialEq)]
pub struct DepartureConstraint {
    vehicle_key: String,
    required_departure_soc: f64,
    constraint_departure_unix_ms: i64,
    source: DepartureConstraintSource,
    provenance: Provenance,
    received_at_unix_ms: i64,
}

impl DepartureConstraint {
    /// Constructs and validates an inbound constraint. The SOC is validated
    /// in [0,1]; the vehicle key is non-empty; timestamps are carried as
    /// declared data (the EV-model style — raw UTC ms).
    pub fn new(
        vehicle_key: String,
        required_departure_soc: f64,
        constraint_departure_unix_ms: i64,
        source: DepartureConstraintSource,
        provenance: Provenance,
        received_at_unix_ms: i64,
    ) -> Result<Self, EvDepartureError> {
        if vehicle_key.trim().is_empty() {
            return Err(EvDepartureError::EmptyVehicleKey);
        }
        if !required_departure_soc.is_finite()
            || required_departure_soc < 0.0
            || required_departure_soc > 1.0
        {
            return Err(EvDepartureError::RequiredSocOutOfRange {
                vehicle_key,
                value: required_departure_soc,
            });
        }
        Ok(Self {
            vehicle_key,
            required_departure_soc,
            constraint_departure_unix_ms,
            source,
            provenance,
            received_at_unix_ms,
        })
    }

    /// The vehicle key.
    pub fn vehicle_key(&self) -> &str {
        &self.vehicle_key
    }

    /// The inbound required departure SOC (the declared value — may be
    /// refused at mapping time; see the relaxation rule).
    pub fn required_departure_soc(&self) -> f64 {
        self.required_departure_soc
    }

    /// The departure instant the constraint declares (UTC ms).
    pub fn constraint_departure_unix_ms(&self) -> i64 {
        self.constraint_departure_unix_ms
    }

    /// The declared source.
    pub fn source(&self) -> DepartureConstraintSource {
        self.source
    }

    /// The provenance.
    pub fn provenance(&self) -> Provenance {
        self.provenance
    }

    /// When the platform received the constraint (UTC ms).
    pub fn received_at_unix_ms(&self) -> i64 {
        self.received_at_unix_ms
    }
}

/// The MAPPED, operative constraint: the value the composer consumes. It
/// exists only as the output of [`map_constraint_onto_fleet_config`] —
/// the one place the never-relaxed rule is enforced — so a raw inbound
/// constraint cannot bypass the direction check.
#[derive(Debug, Clone, PartialEq)]
pub struct MappedDepartureConstraint {
    /// The vehicle key.
    pub vehicle_key: String,
    /// The operative required departure SOC (never below the configured
    /// floor; the max of constraint and configuration).
    pub operative_required_soc: f64,
    /// The operative departure instant in UTC ms (never later than the
    /// configured departure; the min of constraint and configuration).
    pub operative_departure_unix_ms: i64,
    /// The configured floor the mapping preserved (audit trail of the
    /// never-relaxed rule).
    pub configured_floor_soc: f64,
    /// The configured departure instant the mapping preserved.
    pub configured_departure_unix_ms: i64,
    /// The source of the INBOUND constraint that motivated the mapping.
    pub source: DepartureConstraintSource,
    /// Whether the constraint tightened anything (false = it matched the
    /// configuration exactly; a no-op tightening is still a valid record).
    pub tightened: bool,
}

impl MappedDepartureConstraint {
    /// The energy the operative SOC implies, in MWh (the composer's input;
    /// the capacity comes from the fleet configuration the mapping ran
    /// against). Refuses a non-finite capacity (a configuration error,
    /// never a zero-energy default).
    pub fn required_energy_mwh(&self, battery_capacity_mwh: f64) -> Result<f64, EvDepartureError> {
        if !battery_capacity_mwh.is_finite() || battery_capacity_mwh <= 0.0 {
            return Err(EvDepartureError::InvalidBatteryCapacity {
                value: battery_capacity_mwh,
            });
        }
        Ok(self.operative_required_soc * battery_capacity_mwh)
    }
}

/// Maps one inbound departure constraint onto the fleet configuration,
/// enforcing the HARD departure-energy policy direction rule: TIGHTEN ok,
/// RELAX refused.
///
/// The operative values are:
/// - SOC: `max(configured_required_departure_soc, constraint_soc)` — an
///   inbound lower SOC is refused (never relaxed); an equal SOC is a
///   recorded no-op tightening (`tightened == false`);
/// - departure instant: `min(configured_departure, constraint_departure)` —
///   an inbound LATER instant is refused (never relaxed); an equal instant
///   is a recorded no-op tightening.
pub fn map_constraint_onto_fleet_config(
    constraint: &DepartureConstraint,
    fleet: &EvFleetConfig,
) -> Result<MappedDepartureConstraint, EvDepartureError> {
    let vehicle = fleet
        .vehicles
        .iter()
        .find(|v| v.vehicle_key == constraint.vehicle_key());
    let Some(vehicle) = vehicle else {
        // An unknown vehicle is not a relaxation case; the composer only
        // consumes constraints for vehicles the fleet declares. Refusing
        // here keeps the mapping total over its declared inputs.
        return Err(EvDepartureError::UnknownVehicleKey {
            vehicle_key: constraint.vehicle_key().to_string(),
        });
    };
    let configured_soc = vehicle.required_departure_soc;
    let configured_departure = vehicle.departure_millis;

    if constraint.required_departure_soc() + crate::ev_fleet_model::EV_FLEET_EVALUATION_TOLERANCE
        < configured_soc
    {
        return Err(EvDepartureError::RelaxationRefused {
            vehicle_key: constraint.vehicle_key().to_string(),
            constraint_required_soc: constraint.required_departure_soc(),
            configured_required_soc: configured_soc,
            reason: "the constraint's required departure SOC is below the configured floor"
                .to_string(),
        });
    }
    if constraint.constraint_departure_unix_ms() > configured_departure {
        return Err(EvDepartureError::RelaxationRefused {
            vehicle_key: constraint.vehicle_key().to_string(),
            constraint_required_soc: constraint.required_departure_soc(),
            configured_required_soc: configured_soc,
            reason: "the constraint's departure instant is later than the configured departure"
                .to_string(),
        });
    }
    let tightened = constraint.required_departure_soc() > configured_soc
        || constraint.constraint_departure_unix_ms() < configured_departure;
    Ok(MappedDepartureConstraint {
        vehicle_key: constraint.vehicle_key().to_string(),
        operative_required_soc: configured_soc.max(constraint.required_departure_soc()),
        operative_departure_unix_ms: configured_departure
            .min(constraint.constraint_departure_unix_ms()),
        configured_floor_soc: configured_soc,
        configured_departure_unix_ms: configured_departure,
        source: constraint.source(),
        tightened,
    })
}

/// The per-vehicle departure-constraint set (BTreeMap-backed — the
/// deterministic-iteration rule). One operative constraint per vehicle; a
/// newer constraint supersedes through the explicit path, retaining the
/// old record for audit.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DepartureConstraintSet {
    operative: std::collections::BTreeMap<String, MappedDepartureConstraint>,
    superseded: Vec<MappedDepartureConstraint>,
}

impl DepartureConstraintSet {
    /// An empty set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts a mapped (already direction-checked) constraint. Refuses a
    /// duplicate vehicle key — supersession is the explicit path below.
    pub fn insert(
        &mut self,
        mapped: MappedDepartureConstraint,
    ) -> Result<(), EvDepartureError> {
        if self.operative.contains_key(&mapped.vehicle_key) {
            return Err(EvDepartureError::DuplicateVehicleConstraint {
                vehicle_key: mapped.vehicle_key.clone(),
            });
        }
        self.operative.insert(mapped.vehicle_key.clone(), mapped);
        Ok(())
    }

    /// The explicit supersession path: the NEW mapped constraint replaces
    /// the operative one for the vehicle, and the old record is retained
    /// in the superseded list (audit — never a silent overwrite). Requires
    /// `new_received_at > old_received_at`, so a supersession advances
    /// time.
    pub fn supersede(
        &mut self,
        vehicle_key: &str,
        new_mapped: MappedDepartureConstraint,
        new_received_at_unix_ms: i64,
        old_received_at_unix_ms: i64,
    ) -> Result<(), EvDepartureError> {
        if new_received_at_unix_ms <= old_received_at_unix_ms {
            return Err(EvDepartureError::SupersessionNotNewer {
                vehicle_key: vehicle_key.to_string(),
                old_received_at_unix_ms,
                new_received_at_unix_ms,
            });
        }
        let old = self.operative.remove(vehicle_key).ok_or(
            EvDepartureError::UnknownVehicleKey {
                vehicle_key: vehicle_key.to_string(),
            },
        )?;
        self.superseded.push(old);
        self.operative
            .insert(new_mapped.vehicle_key.clone(), new_mapped);
        Ok(())
    }

    /// The operative constraints, in deterministic (vehicle-key) order —
    /// the composer's input.
    pub fn operative(&self) -> Vec<&MappedDepartureConstraint> {
        self.operative.values().collect()
    }

    /// The retained superseded records (audit; order of supersession).
    pub fn superseded(&self) -> &[MappedDepartureConstraint] {
        &self.superseded
    }

    /// Whether the set holds an operative constraint for a vehicle.
    pub fn contains(&self, vehicle_key: &str) -> bool {
        self.operative.contains_key(vehicle_key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ev_fleet_model::{EvseConfig, EvVehicleConfig, V2xConfig};

    fn vehicle(key: &str, soc: f64, departure: i64) -> EvVehicleConfig {
        EvVehicleConfig {
            vehicle_key: key.to_string(),
            battery_capacity_mwh: 0.06,
            initial_soc: 0.3,
            required_departure_soc: soc,
            departure_millis: departure,
            // The window covers both fixtures' departures (end-exclusive).
            connection_windows_millis: vec![(1_700_000_000_000, 1_700_000_050_000_000)],
            max_charging_power_mw: 0.011,
            min_charging_power_mw: 0.0,
            ramp_limit_mw_per_minute: None,
            charging_priority: 1,
            opt_out_from_millis: None,
        }
    }

    fn fleet_config() -> EvFleetConfig {
        // Grid honesty: with a 900 s interval, both departures fall on
        // interval boundaries (offsets 36,000 s and 40,500 s — 40 and 45
        // intervals) and both are covered by the connection window, so
        // `EvFleetModel::new` accepts this fixture (the hard-policy test
        // constructs the model from it).
        EvFleetConfig {
            evse: EvseConfig {
                rated_power_mw: 0.011,
                phase: "3-phase".to_string(),
            },
            vehicles: vec![
                vehicle("veh-01", 0.9, 1_700_000_036_000_000),
                vehicle("veh-02", 0.8, 1_700_000_040_500_000),
            ],
            site_transformer_limit_mw: 0.05,
            v2x: V2xConfig::disabled(),
            interval_seconds: 900,
            horizon_start_millis: 1_700_000_000_000,
        }
    }

    fn constraint(key: &str, soc: f64, departure: i64) -> DepartureConstraint {
        DepartureConstraint::new(
            key.to_string(),
            soc,
            departure,
            DepartureConstraintSource::VehicleCalendar,
            Provenance::Configured,
            1_700_000_000_001,
        )
        .unwrap()
    }

    #[test]
    fn the_governing_statement_carries_the_never_relaxed_rule() {
        assert!(DEPARTURE_CONSTRAINT_GOVERNING_STATEMENT.contains("HARD constraint"));
        assert!(DEPARTURE_CONSTRAINT_GOVERNING_STATEMENT.contains("never RELAX"));
        assert!(DEPARTURE_CONSTRAINT_GOVERNING_STATEMENT.contains("TIGHTEN"));
    }

    #[test]
    fn a_tightening_constraint_maps_operatively() {
        // Raise the required SOC and move the departure earlier: both are
        // tightenings; both land operatively.
        let inbound = constraint("veh-01", 0.95, 1_700_000_030_000_000);
        let mapped = map_constraint_onto_fleet_config(&inbound, &fleet_config()).unwrap();
        assert_eq!(mapped.operative_required_soc, 0.95);
        assert_eq!(mapped.operative_departure_unix_ms, 1_700_000_030_000_000);
        assert!(mapped.tightened);
        assert_eq!(mapped.configured_floor_soc, 0.9);
        let energy = mapped.required_energy_mwh(0.06).unwrap();
        assert!((energy - 0.95 * 0.06).abs() < 1e-12);
    }

    #[test]
    fn a_matching_constraint_is_a_recorded_no_op() {
        // Exactly the configured SOC and departure instant (the fixture's
        // veh-02 departure is 1_700_000_040_500_000).
        let inbound = constraint("veh-02", 0.8, 1_700_000_040_500_000);
        let mapped = map_constraint_onto_fleet_config(&inbound, &fleet_config()).unwrap();
        assert_eq!(mapped.operative_required_soc, 0.8);
        assert_eq!(mapped.operative_departure_unix_ms, 1_700_000_040_500_000);
        assert!(!mapped.tightened);
    }

    #[test]
    fn a_soc_relaxation_is_refused() {
        // The inbound SOC (0.7) is below the configured floor (0.8): the
        // hard policy would be relaxed — refused with a named error.
        let inbound = constraint("veh-02", 0.7, 1_700_000_040_000_000);
        let err = map_constraint_onto_fleet_config(&inbound, &fleet_config()).unwrap_err();
        assert!(matches!(err, EvDepartureError::RelaxationRefused { .. }));
        assert!(err.to_string().contains("RELAX"));
    }

    #[test]
    fn a_later_departure_instant_is_refused() {
        // The inbound departure is LATER than the configured one: also a
        // relaxation of the hard policy.
        let inbound = constraint("veh-01", 0.9, 1_700_000_050_000_000);
        let err = map_constraint_onto_fleet_config(&inbound, &fleet_config()).unwrap_err();
        assert!(matches!(err, EvDepartureError::RelaxationRefused { .. }));
        assert!(err.to_string().contains("later"));
    }

    #[test]
    fn the_hard_policy_declaration_is_untouched_by_the_mapping() {
        // The mapping never produces a soft variant: the fleet model's own
        // declaration stays Hard.
        let model = crate::ev_fleet_model::EvFleetModel::new(fleet_config()).unwrap();
        assert_eq!(
            model.departure_policy(),
            crate::ev_fleet_model::DepartureEnergyPolicy::Hard
        );
    }

    #[test]
    fn an_unknown_vehicle_is_refused_at_mapping() {
        let inbound = constraint("veh-ghost", 0.9, 1_700_000_030_000_000);
        let err = map_constraint_onto_fleet_config(&inbound, &fleet_config()).unwrap_err();
        assert_eq!(
            err,
            EvDepartureError::UnknownVehicleKey {
                vehicle_key: "veh-ghost".to_string()
            }
        );
    }

    #[test]
    fn construction_validates_key_and_soc() {
        assert_eq!(
            DepartureConstraint::new(
                "  ".to_string(),
                0.9,
                0,
                DepartureConstraintSource::VehicleCalendar,
                Provenance::Configured,
                0
            ),
            Err(EvDepartureError::EmptyVehicleKey)
        );
        assert!(matches!(
            DepartureConstraint::new(
                "veh-01".to_string(),
                1.2,
                0,
                DepartureConstraintSource::VehicleCalendar,
                Provenance::Configured,
                0
            ),
            Err(EvDepartureError::RequiredSocOutOfRange { .. })
        ));
    }

    #[test]
    fn the_set_is_deterministic_and_refuses_duplicates() {
        let mut set = DepartureConstraintSet::new();
        let first = map_constraint_onto_fleet_config(
            &constraint("veh-02", 0.85, 1_700_000_035_000_000),
            &fleet_config(),
        )
        .unwrap();
        set.insert(first).unwrap();
        let duplicate = map_constraint_onto_fleet_config(
            &constraint("veh-02", 0.9, 1_700_000_030_000_000),
            &fleet_config(),
        )
        .unwrap();
        assert_eq!(
            set.insert(duplicate),
            Err(EvDepartureError::DuplicateVehicleConstraint {
                vehicle_key: "veh-02".to_string()
            })
        );
        // BTreeMap-backed deterministic order: veh-01 before veh-02.
        let second = map_constraint_onto_fleet_config(
            &constraint("veh-01", 0.9, 1_700_000_036_000_000),
            &fleet_config(),
        )
        .unwrap();
        set.insert(second).unwrap();
        let keys: Vec<&str> = set.operative().iter().map(|c| c.vehicle_key.as_str()).collect();
        assert_eq!(keys, ["veh-01", "veh-02"]);
    }

    #[test]
    fn supersession_retains_the_old_record_and_requires_newer_time() {
        let mut set = DepartureConstraintSet::new();
        let first = map_constraint_onto_fleet_config(
            &constraint("veh-01", 0.9, 1_700_000_036_000_000),
            &fleet_config(),
        )
        .unwrap();
        set.insert(first).unwrap();
        // A supersession that does not advance time is refused.
        let newer = map_constraint_onto_fleet_config(
            &constraint("veh-01", 0.95, 1_700_000_030_000_000),
            &fleet_config(),
        )
        .unwrap();
        assert_eq!(
            set.supersede("veh-01", newer.clone(), 1_700_000_000_001, 1_700_000_000_001),
            Err(EvDepartureError::SupersessionNotNewer {
                vehicle_key: "veh-01".to_string(),
                old_received_at_unix_ms: 1_700_000_000_001,
                new_received_at_unix_ms: 1_700_000_000_001,
            })
        );
        // A strictly newer supersession retains the old record for audit.
        set.supersede("veh-01", newer, 1_700_000_000_002, 1_700_000_000_001)
            .unwrap();
        assert_eq!(set.superseded().len(), 1);
        assert_eq!(set.superseded()[0].operative_required_soc, 0.9);
        assert_eq!(set.operative()[0].operative_required_soc, 0.95);
        // Superseding an unknown vehicle is refused.
        let ghost = map_constraint_onto_fleet_config(
            &constraint("veh-ghost", 0.9, 1_700_000_036_000_000),
            &fleet_config(),
        );
        assert!(ghost.is_err());
    }

    #[test]
    fn energy_derivation_refuses_an_invalid_capacity() {
        let mapped = map_constraint_onto_fleet_config(
            &constraint("veh-01", 0.95, 1_700_000_030_000_000),
            &fleet_config(),
        )
        .unwrap();
        assert!(mapped.required_energy_mwh(0.0).is_err());
        assert!(mapped.required_energy_mwh(f64::NAN).is_err());
    }
}
