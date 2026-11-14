// P6.08 — `SimulatedEmsSource`: the in-process, deterministic, scenario-
// fixture-driven EMS adapter (ledger P6.08; the registry registers it as
// TEST/simulation-only).
//
// PURPOSE AND HARD BOUNDARY, stated plainly:
//
// - This adapter exists so the P6.08 read path has an EXECUTABLE test
//   source without OpenEMS (and without any network I/O): the read-path
//   tests, the store-and-forward drills, and later failure-injection waves
//   (P6.13/P6.28) compose it exactly where a real adapter will compose.
// - It is **never a real EMS and never presentable as one.** The registry
//   id is `simulated-test` with status `AdapterStatus::TestOnly`; the
//   adapter id travels on every sample's `source_id`
//   (`edge:<site>:simulated-test/<component>/<channel>`), so a simulated
//   record is identifiable from its identity alone, forever. P6.25 must
//   surface simulated values visually distinct (report §20.2), and no
//   production composition root may wire this id for a real site.
// - Deterministic BY CONSTRUCTION: a [`SimulatedFixture`] is a fixed
//   scenario table (per-point value and quality schedules, fixed start
//   time, fixed step); the adapter's tick advances ONLY through the
//   explicit [`SimControl::advance_tick`] control handle. Given the same
//   fixture and the same sequence of control calls, the same readings
//   result — the Phase-2/Phase-5 deterministic-scenario discipline, reused
//   at the edge. No wall-clock reads, no randomness.
// - The adapter emits NATIVE readings ([`crate::adapters::port::
//   EmsNativeReading`]) — native units, verbatim quality codes, native
//   timestamps — and NEVER canonical samples. Mapping to canonical is the
//   ACL's job (P6.12; §6.4: one translation place). The quality codes it
//   emits are the ACL's named table entries (`GOOD`, `OUT_OF_RANGE`, …),
//   so a fixture can drive degraded-quality scenarios (a "corrupted"
//   source value arrives SUSPECT, never GOOD — the P6.08 acceptance rule).
// - The declared capabilities follow the P6.12 no-defaults rule: the
//   fixture DECLARES its reactive sign convention, cadence, quality-code
//   mapping (derived from the codes the fixture actually uses), and clock
//   quality. The clock quality is `SYNCHRONIZED` — honest for a scenario
//   clock that IS the scenario's time authority (the simulator is its own
//   synchronised time source, the Phase-2 simulator precedent), and
//   recorded, never assumed for a real source.
//
// Read-only: this adapter implements `discover`/`read`/`capabilities`/
// `health` and NOTHING else — no write, no setpoint, no dispatch (the
// P6.12 port has no such method; v0.4.0 is read-only/test-lab by default).

use std::sync::{Arc, Mutex};
use std::time::Duration;

use flexcore_contracts::time_model::ClockQuality;
use flexcore_contracts::units::ReactiveSignConvention;
use flexcore_ports::telemetry_source::{QualityMappingDeclaration, SourceStateMapping};

use crate::adapters::acl::NativeUnit;
use crate::adapters::port::{
    AdapterCapabilities, AdapterError, AdapterHealth, ChannelDescriptor, EmsAdapter, EmsNativeReading,
    PointKey,
};
use crate::adapters::acl::NativeTimestamp;

/// The registered registry id of this adapter (TEST/simulation-only — see
/// the module header; never a real EMS).
pub const SIMULATED_ADAPTER_ID: &str = "simulated-test";

// --- The scenario fixture ----------------------------------------------------

/// One point of the deterministic scenario: its address, its native unit,
/// and the per-tick schedules of value and quality code. Tick `t` reads
/// `values_per_tick[t % len]` with quality `quality_per_tick[t % len]` —
/// fully determined, no hidden state.
#[derive(Debug, Clone, PartialEq)]
pub struct SimulatedPointFixture {
    /// The point this fixture entry drives.
    pub point: PointKey,
    /// The native unit the simulated EMS reports (parsed vocabulary —
    /// unknown units are a fixture-construction error via the ACL table).
    pub native_unit: NativeUnit,
    /// The value at tick t (cycled).
    pub values_per_tick: Vec<f64>,
    /// The verbatim native quality code at tick t (cycled). Must use the
    /// ACL's named codes (`GOOD`, `BAD`, `STALE`, `SUBSTITUTED`,
    /// `OUT_OF_RANGE`, `UNSYNCHRONIZED`, `COMMUNICATION_LOSS`,
    /// `UNRELIABLE`) so the ACL maps them; anything else the ACL refuses —
    /// which is a usable way to exercise refusal paths, and never a
    /// default-quality path.
    pub quality_per_tick: Vec<String>,
}

/// Errors of fixture construction (fail-fast, named).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SimulatedFixtureError {
    /// The fixture declared no points.
    EmptyPointSet,
    /// A point schedule was empty (a point that produces no values can
    /// never be read — refuse rather than discover at poll time).
    EmptySchedule {
        component: String,
        channel: String,
    },
    /// The scenario step was zero or negative (event times would not
    /// advance).
    NonPositiveStepMillis,
}

impl std::fmt::Display for SimulatedFixtureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SimulatedFixtureError::EmptyPointSet => {
                write!(f, "a simulated scenario fixture must declare at least one point")
            }
            SimulatedFixtureError::EmptySchedule { component, channel } => write!(
                f,
                "the value/quality schedule of point '{component}/{channel}' is empty; \
                 a point that can never produce a reading is a fixture error"
            ),
            SimulatedFixtureError::NonPositiveStepMillis => {
                write!(f, "the scenario step must be a positive number of milliseconds")
            }
        }
    }
}

impl std::error::Error for SimulatedFixtureError {}

/// The deterministic scenario fixture: the fixed table the simulated EMS
/// serves, tick by tick.
#[derive(Debug, Clone, PartialEq)]
pub struct SimulatedFixture {
    /// The points, in declared order.
    pub points: Vec<SimulatedPointFixture>,
    /// The event_time of tick 0 (Unix milliseconds).
    pub start_millis: i64,
    /// The event_time step per tick (positive).
    pub step_millis: i64,
    /// The DECLARED reactive-power sign convention of the simulated source
    /// (P0.07 §3 — a declaration, never inferred).
    pub reactive_source_convention: ReactiveSignConvention,
    /// The declared cadence of the simulated source (P2.11 provider-rule
    /// shape at the adapter level).
    pub cadence_millis: u64,
}

impl SimulatedFixture {
    /// Validates the fixture (fail-fast: named errors, no half-valid
    /// scenarios).
    pub fn validate(&self) -> Result<(), SimulatedFixtureError> {
        if self.points.is_empty() {
            return Err(SimulatedFixtureError::EmptyPointSet);
        }
        if self.step_millis <= 0 {
            return Err(SimulatedFixtureError::NonPositiveStepMillis);
        }
        for point in &self.points {
            if point.values_per_tick.is_empty() || point.quality_per_tick.is_empty() {
                return Err(SimulatedFixtureError::EmptySchedule {
                    component: point.point.component.clone(),
                    channel: point.point.channel.clone(),
                });
            }
        }
        Ok(())
    }

    /// The distinct quality codes the fixture uses (for the DECLARED
    /// quality mapping — the adapter declares every code it can report,
    /// P0.08 §3).
    fn distinct_quality_codes(&self) -> Vec<String> {
        let mut codes: Vec<String> = Vec::new();
        for point in &self.points {
            for code in &point.quality_per_tick {
                if !codes.contains(code) {
                    codes.push(code.clone());
                }
            }
        }
        codes
    }

    fn event_millis(&self, tick: u64) -> i64 {
        self.start_millis + (tick as i64) * self.step_millis
    }
}

// --- Shared control state ----------------------------------------------------

/// The mutable scenario state (tick + reachability), shared between the
/// adapter and its control handle.
#[derive(Debug)]
struct SimState {
    fixture: SimulatedFixture,
    tick: u64,
    unreachable: Option<String>,
}

/// A cloneable control handle over the deterministic scenario. Tests (and
/// later failure-injection waves) hold the handle while the adapter is
/// composed inside the read path — the same shared-state pattern the
/// runner's scripted fakes use. `advance_tick` is EXPLICIT: nothing about
/// the scenario moves on its own.
#[derive(Clone)]
pub struct SimControl {
    state: Arc<Mutex<SimState>>,
}

impl SimControl {
    /// Advances the deterministic scenario clock by one tick (the next
    /// `event_time`, the next schedule entry).
    pub fn advance_tick(&self) {
        self.state
            .lock()
            .expect("poisoned simulated-scenario mutex")
            .tick += 1;
    }

    /// The current tick.
    pub fn tick(&self) -> u64 {
        self.state
            .lock()
            .expect("poisoned simulated-scenario mutex")
            .tick
    }

    /// Forces the adapter unreachable (`Some(reason)` → every `read` fails
    /// with `AdapterError::NotConnected` and `health` reports
    /// `Unreachable`) or restores it (`None`) — the forced-disconnect /
    /// failover hook the recovery drills use.
    pub fn set_unreachable(&self, reason: Option<String>) {
        self.state
            .lock()
            .expect("poisoned simulated-scenario mutex")
            .unreachable = reason;
    }
}

// --- The adapter -------------------------------------------------------------

/// The canonical state each ACL-named quality code maps to (the P6.12 ACL
/// table, mirrored here ONLY to build the adapter's DECLARED quality
/// mapping — the adapter must declare every code it can report, P0.08 §3).
/// A fixture code outside this table is intentionally NOT declared: the
/// ACL refuses undeclared codes at read time, which is exactly the refusal
/// path the fault-injection fixtures exercise.
fn canonical_for_acl_code(code: &str) -> Option<flexcore_contracts::quality::TelemetryQuality> {
    use flexcore_contracts::quality::{map_source_state, TelemetryQuality};
    match code.trim() {
        "GOOD" => Some(TelemetryQuality::Good),
        "BAD" => Some(TelemetryQuality::Bad),
        "STALE" => Some(TelemetryQuality::Stale),
        "SUBSTITUTED" => Some(map_source_state(
            flexcore_contracts::quality::SourceQualityState::Substituted,
        )),
        "OUT_OF_RANGE" => Some(map_source_state(
            flexcore_contracts::quality::SourceQualityState::OutOfRange,
        )),
        "UNSYNCHRONIZED" => Some(map_source_state(
            flexcore_contracts::quality::SourceQualityState::Unsynchronized,
        )),
        "COMMUNICATION_LOSS" => Some(map_source_state(
            flexcore_contracts::quality::SourceQualityState::CommunicationLoss,
        )),
        "UNRELIABLE" => Some(map_source_state(
            flexcore_contracts::quality::SourceQualityState::Unreliable,
        )),
        _ => None,
    }
}

/// The TEST/simulation-only [`EmsAdapter`] (see the module header for the
/// hard boundary). In-process, deterministic, no network.
#[derive(Debug, Clone)]
pub struct SimulatedEmsSource {
    state: Arc<Mutex<SimState>>,
    /// The DECLARED capabilities, built once from the (immutable) fixture
    /// schedules at construction — the port hands out a reference, so the
    /// declaration is stored, not rebuilt (P6.12 no-defaults rule).
    capabilities: AdapterCapabilities,
}

impl SimulatedEmsSource {
    /// Builds the adapter over a validated fixture. Panics on an invalid
    /// fixture (fail-fast: a broken scenario is a construction-time
    /// programmer error, the same rule as the buffer capacities).
    pub fn new(fixture: SimulatedFixture) -> Self {
        fixture
            .validate()
            .expect("a simulated fixture must be valid at construction (fail-fast)");
        // The declared quality mapping: every ACL-named code the fixture
        // actually uses, mapped through the ACL's frozen table. Codes
        // outside the table stay undeclared (= refused at read time). A
        // fixture that uses ONLY undeclared codes (a deliberate
        // refusal-drill scenario) falls back to declaring the minimal
        // honest entry (`GOOD`) — the P2.11 declaration shape requires a
        // non-empty mapping, and such a fixture never reports GOOD, so the
        // declaration claims nothing the scenario emits.
        let mut mappings: Vec<SourceStateMapping> = fixture
            .distinct_quality_codes()
            .into_iter()
            .filter_map(|code| {
                canonical_for_acl_code(&code).map(|canonical| SourceStateMapping {
                    source_code: code,
                    canonical,
                })
            })
            .collect();
        if mappings.is_empty() {
            mappings.push(SourceStateMapping {
                source_code: "GOOD".to_string(),
                canonical: flexcore_contracts::quality::TelemetryQuality::Good,
            });
        }
        let quality_mapping = QualityMappingDeclaration::new(mappings)
            .expect("the fixture must use at least one ACL-named quality code");
        let capabilities = AdapterCapabilities {
            adapter_id: SIMULATED_ADAPTER_ID.to_string(),
            reactive_sign_convention: fixture.reactive_source_convention,
            cadence: Duration::from_millis(fixture.cadence_millis),
            quality_mapping,
            // The scenario clock IS the scenario's time authority — a
            // synchronised declaration is honest for a deterministic
            // simulator (the Phase-2 simulator precedent) and recorded,
            // never assumed for a real source.
            clock_quality: ClockQuality::Synchronized,
        };
        Self {
            state: Arc::new(Mutex::new(SimState {
                fixture,
                tick: 0,
                unreachable: None,
            })),
            capabilities,
        }
    }

    /// The scenario control handle (advance ticks; force disconnects).
    pub fn control(&self) -> SimControl {
        SimControl {
            state: Arc::clone(&self.state),
        }
    }
}

impl EmsAdapter for SimulatedEmsSource {
    fn id(&self) -> &str {
        SIMULATED_ADAPTER_ID
    }

    /// The declared capabilities — every field a REQUIRED declaration
    /// (P6.12 port contract; no defaults), built from the fixture at
    /// construction.
    fn capabilities(&self) -> &AdapterCapabilities {
        &self.capabilities
    }

    fn discover(&mut self) -> Result<Vec<ChannelDescriptor>, AdapterError> {
        let state = self.state.lock().expect("poisoned simulated-scenario mutex");
        if let Some(reason) = &state.unreachable {
            return Err(AdapterError::DiscoveryFailed {
                reason: format!("the simulated EMS is unreachable: {reason}"),
            });
        }
        Ok(state
            .fixture
            .points
            .iter()
            .map(|point| ChannelDescriptor {
                point: point.point.clone(),
                native_unit: point.native_unit,
                description: format!(
                    "simulated scenario point (TEST/simulation-only; id '{}')",
                    SIMULATED_ADAPTER_ID
                ),
            })
            .collect())
    }

    fn read(&mut self, point: &PointKey) -> Result<EmsNativeReading, AdapterError> {
        let state = self.state.lock().expect("poisoned simulated-scenario mutex");
        if let Some(reason) = &state.unreachable {
            return Err(AdapterError::NotConnected {
                reason: format!("the simulated EMS is unreachable: {reason}"),
            });
        }
        let entry = state
            .fixture
            .points
            .iter()
            .find(|entry| &entry.point == point)
            .ok_or_else(|| AdapterError::ReadFailed {
                point: point.clone(),
                reason: format!(
                    "the point is not part of the simulated scenario fixture (TEST id '{}')",
                    SIMULATED_ADAPTER_ID
                ),
            })?;
        let tick = state.tick;
        let value = entry.values_per_tick[(tick as usize) % entry.values_per_tick.len()];
        let quality = entry.quality_per_tick[(tick as usize) % entry.quality_per_tick.len()].clone();
        Ok(EmsNativeReading {
            point: point.clone(),
            native_unit: entry.native_unit,
            raw_value: value,
            native_quality_code: Some(quality),
            event_time: NativeTimestamp::UnixMillis(state.fixture.event_millis(tick)),
            // Source sequence: monotonically increasing per (point, source)
            // (P0.09 §1) — the tick index + 1 is deterministic and strict.
            source_sequence: tick + 1,
        })
    }

    fn health(&self) -> AdapterHealth {
        let state = self.state.lock().expect("poisoned simulated-scenario mutex");
        if let Some(reason) = &state.unreachable {
            AdapterHealth::Unreachable {
                reason: format!("the simulated EMS is forced unreachable: {reason}"),
            }
        } else {
            AdapterHealth::Healthy
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> SimulatedFixture {
        SimulatedFixture {
            points: vec![SimulatedPointFixture {
                point: PointKey::new("ess0", "ActivePower"),
                native_unit: NativeUnit::Kw,
                values_per_tick: vec![1_500.0],
                quality_per_tick: vec!["GOOD".to_string()],
            }],
            start_millis: 1_700_000_000_000,
            step_millis: 1_000,
            reactive_source_convention: ReactiveSignConvention::InjectionPositive,
            cadence_millis: 5_000,
        }
    }

    #[test]
    fn readings_are_deterministic_and_advance_only_explicitly() {
        let sim = SimulatedEmsSource::new(fixture());
        let control = sim.control();
        let mut adapter = sim;

        let point = PointKey::new("ess0", "ActivePower");
        let first = adapter.read(&point).expect("reads");
        assert_eq!(first.raw_value, 1_500.0);
        assert_eq!(first.native_quality_code, Some("GOOD".to_string()));
        assert_eq!(
            first.event_time,
            NativeTimestamp::UnixMillis(1_700_000_000_000)
        );
        assert_eq!(first.source_sequence, 1);

        // Without an explicit advance: identical reading (deterministic;
        // the re-read identity the edge dedup window catches).
        let again = adapter.read(&point).expect("reads");
        assert_eq!(first, again);

        control.advance_tick();
        let next = adapter.read(&point).expect("reads");
        assert_eq!(
            next.event_time,
            NativeTimestamp::UnixMillis(1_700_000_001_000)
        );
        assert_eq!(next.source_sequence, 2, "the sequence advances per tick");
    }

    #[test]
    fn forced_unreachable_fails_closed_and_health_reports_it() {
        let sim = SimulatedEmsSource::new(fixture());
        let control = sim.control();
        let mut adapter = sim;
        assert_eq!(adapter.health(), AdapterHealth::Healthy);

        control.set_unreachable(Some("drill: forced disconnect".to_string()));
        assert!(matches!(
            adapter.health(),
            AdapterHealth::Unreachable { .. }
        ));
        let error = adapter
            .read(&PointKey::new("ess0", "ActivePower"))
            .expect_err("forced-unreachable read fails");
        assert!(
            matches!(error, AdapterError::NotConnected { .. }),
            "the read failure is NotConnected (the hard poll-failure path): {error}"
        );
        assert!(matches!(
            adapter.discover(),
            Err(AdapterError::DiscoveryFailed { .. })
        ));

        // Restore: the failover path.
        control.set_unreachable(None);
        assert!(adapter
            .read(&PointKey::new("ess0", "ActivePower"))
            .is_ok());
    }

    #[test]
    fn unknown_point_is_a_named_error_never_an_empty_result() {
        let mut adapter = SimulatedEmsSource::new(fixture());
        let error = adapter
            .read(&PointKey::new("ghost", "Channel"))
            .expect_err("unknown point");
        assert!(matches!(error, AdapterError::ReadFailed { .. }));
        assert!(error.to_string().contains("ghost/Channel"));
    }

    #[test]
    fn invalid_fixtures_are_construction_errors() {
        let mut empty = fixture();
        empty.points.clear();
        assert!(matches!(
            empty.validate(),
            Err(SimulatedFixtureError::EmptyPointSet)
        ));

        let mut no_schedule = fixture();
        no_schedule.points[0].values_per_tick.clear();
        assert!(matches!(
            no_schedule.validate(),
            Err(SimulatedFixtureError::EmptySchedule { .. })
        ));

        let mut no_step = fixture();
        no_step.step_millis = 0;
        assert!(matches!(
            no_step.validate(),
            Err(SimulatedFixtureError::NonPositiveStepMillis)
        ));
    }

    #[test]
    fn distinct_quality_codes_derive_the_declared_mapping() {
        let mut scenario = fixture();
        scenario.points.push(SimulatedPointFixture {
            point: PointKey::new("ess0", "Soc"),
            native_unit: NativeUnit::SocPercent,
            values_per_tick: vec![87.0],
            quality_per_tick: vec!["GOOD".to_string(), "OUT_OF_RANGE".to_string()],
        });
        let codes = scenario.distinct_quality_codes();
        assert_eq!(codes.len(), 2);
        assert!(codes.contains(&"GOOD".to_string()));
        assert!(codes.contains(&"OUT_OF_RANGE".to_string()));
        // Every code maps through the ACL's frozen table (declared, not
        // invented) — the QualityMappingDeclaration built from these codes
        // validates.
        let declaration = QualityMappingDeclaration::new(
            codes
                .into_iter()
                .map(|code| SourceStateMapping {
                    source_code: code,
                    canonical: flexcore_contracts::quality::TelemetryQuality::Good,
                })
                .collect::<Vec<_>>(),
        )
        .expect("declares without duplicates");
        assert_eq!(declaration.mappings().len(), 2);
    }
}
