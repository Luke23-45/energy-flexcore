// P14.17 — The simulated DNP3 master (decision record 81; the seam is
// `flexcore_dnp3_adapter::transport::Dnp3Transport` — the UNWIRED
// outstation-direction seam, record 24 §4).
//
// The outstation-direction seam serves an [`OutstationReport`] TO the
// master side ("answering polls / unsolicited reporting — UNVERIFIED
// semantics", record 24 §4). The simulated MASTER is the receiving end:
// it implements [`Dnp3Transport`] by RECORDING every served report —
// no socket, no DNP3 stack, no link-layer framing, and no claim about
// poll semantics (the real master's poll schedule is the utility's
// declared data and does not exist here).
//
// The fixture also builds a declared [`OutstationModel`] inventory so a
// test can drive the full loop: canonical fleet values → the
// outstation's declared scaling → a queued/reported event → the
// simulated master's received report. Deterministic: reports are
// received in serve order; there is no wall-clock read. Every identity
// string carries the `sim-dnp3` prefix — never presentable as a real
// master or SCADA endpoint.

use std::sync::{Arc, Mutex};

use flexcore_dnp3_adapter::outstation::{
    OutstationModel, PointAssignment, QueuedEvent,
};
use flexcore_dnp3_adapter::transport::{Dnp3Transport, LinkConfig, OutstationReport, TransportError};
use flexcore_dnp3_adapter::model::{DeclaredScaling, DerPointKind, EventClass};

/// The identity prefix every simulated DNP3 peer carries in its identity
/// strings (never presentable as a real master/SCADA endpoint).
pub const SIM_DNP3_ID_PREFIX: &str = "sim-dnp3";

/// Errors the simulated DNP3 peers report. Every failure is named.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SimDnp3Error {
    /// The link configuration was refused (the adapter's own validation,
    /// re-raised).
    Config(TransportError),
    /// The outstation inventory was refused (the model's own validation,
    /// re-raised).
    Inventory(flexcore_dnp3_adapter::outstation::OutstationError),
}

impl From<TransportError> for SimDnp3Error {
    fn from(error: TransportError) -> Self {
        SimDnp3Error::Config(error)
    }
}

impl std::fmt::Display for SimDnp3Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SimDnp3Error::Config(error) => write!(f, "the simulated link config was refused: {error}"),
            SimDnp3Error::Inventory(error) => write!(f, "the simulated inventory was refused: {error}"),
        }
    }
}

impl std::error::Error for SimDnp3Error {}

/// One received report, as the simulated master records it: the report
/// itself plus the sequence position it arrived at (deterministic serve
/// order).
#[derive(Debug, Clone, PartialEq)]
pub struct ReceivedReport {
    /// The arrival sequence (0-based, in serve order).
    pub sequence: u64,
    /// The report the outstation served.
    pub report: OutstationReport,
}

/// The simulated DNP3 master: the receiving end of the outstation-direction
/// seam. Holds a shared received-report log so the caller can inspect
/// what was served after the fact.
#[derive(Debug, Clone)]
pub struct SimulatedDnp3Master {
    /// The master's identity string (carries the `sim-dnp3` prefix).
    pub master_id: String,
    /// The validated link configuration (the composition's declared
    /// addresses; no connection exists).
    pub link: LinkConfig,
    received: Arc<Mutex<Vec<ReceivedReport>>>,
}

impl SimulatedDnp3Master {
    /// Constructs the simulated master over a validated link config.
    /// The config is validated exactly as the adapter's
    /// `UnwiredTransport::new` validates it — a broken composition
    /// surfaces at construction, never at poll time.
    pub fn new(master_id: impl Into<String>, link: LinkConfig) -> Result<Self, SimDnp3Error> {
        link.validate()?;
        Ok(Self {
            master_id: format!("{SIM_DNP3_ID_PREFIX}:{}", master_id.into()),
            link,
            received: Arc::new(Mutex::new(Vec::new())),
        })
    }

    /// The received-report log handle (shared with the trait impl; the
    /// caller keeps this handle and inspects it after polls are served).
    pub fn received_log(&self) -> Arc<Mutex<Vec<ReceivedReport>>> {
        Arc::clone(&self.received)
    }
}

impl Dnp3Transport for SimulatedDnp3Master {
    /// Receives one served report into the log (in-process; no poll is
    /// answered over a wire, no link-layer byte exists). Always `Ok` —
    /// the only failure the log can hit is a poisoned mutex, which the
    /// trait's error vocabulary has no honest variant for; a poisoned
    /// mutex here is an invariant violation of the test composition and
    /// is surfaced by the caller's `expect` on the log lock instead of
    /// being hidden behind an error variant that would name the wrong
    /// failure.
    fn serve_report(&self, report: OutstationReport) -> Result<(), TransportError> {
        let mut log = self
            .received
            .lock()
            .expect("the simulated master's report log mutex must not be poisoned");
        let sequence = log.len() as u64;
        log.push(ReceivedReport { sequence, report });
        Ok(())
    }
}

/// The declared fixture inventory for a simulated fleet outstation: the
/// four DER points the DNP3 adapter's own tests use (active power class
/// 1, SOC class 0, reactive power class 2, setpoint class 0), under the
/// declared scalings. The same declared data a utility point list would
/// carry — here as synthetic fixture data.
pub fn fixture_inventory() -> Vec<PointAssignment> {
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
                reactive_sign_convention:
                    Some(flexcore_contracts::units::ReactiveSignConvention::InjectionPositive),
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

/// Builds the simulated outstation model over [`fixture_inventory`].
pub fn fixture_outstation() -> Result<OutstationModel, SimDnp3Error> {
    OutstationModel::new(format!("{SIM_DNP3_ID_PREFIX}:outstation"), fixture_inventory())
        .map_err(SimDnp3Error::Inventory)
}

/// Builds one report from the outstation's current event queue plus a
/// declared static-point set (the record the outstation model's design
/// describes: static snapshot + drained event queue — record 24 §4).
pub fn report_from(
    static_points: Vec<(u16, f64)>,
    events: &[QueuedEvent],
) -> OutstationReport {
    OutstationReport {
        static_points,
        events: events.to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flexcore_contracts::units::ActivePowerMW;
    use flexcore_dnp3_adapter::outstation::OutstationError;

    fn master() -> SimulatedDnp3Master {
        SimulatedDnp3Master::new(
            "scada-master",
            LinkConfig::Tcp {
                host: "sim-scada.invalid".to_string(),
                port: 20_000,
                outstation_address: 1,
                master_address: 2,
            },
        )
        .expect("valid simulated master")
    }

    #[test]
    fn served_reports_are_received_in_serve_order_with_sequences() {
        let master = master();
        let log = master.received_log();
        master
            .serve_report(report_from(vec![(0, 2500.0)], &[]))
            .expect("served");
        master
            .serve_report(report_from(vec![(0, 2600.0)], &[]))
            .expect("served");
        let received = log.lock().expect("log mutex");
        assert_eq!(received.len(), 2);
        assert_eq!(received[0].sequence, 0);
        assert_eq!(received[0].report.static_points, vec![(0, 2500.0)]);
        assert_eq!(received[1].sequence, 1);
        assert_eq!(received[1].report.static_points, vec![(0, 2600.0)]);
    }

    #[test]
    fn the_fixture_outstation_scales_and_queues_declared_events() {
        let mut outstation = fixture_outstation().expect("valid outstation");
        // Canonical 2.5 MW through the declared kW scaling.
        let scaled = outstation
            .scale_active_power(0, ActivePowerMW::new(2.5).expect("valid"))
            .expect("scales");
        assert_eq!(scaled, 2500.0);
        outstation.queue_event(0, scaled).expect("class 1 queues");
        assert_eq!(outstation.queued_events().len(), 1);
        // The class-0 SOC point does not queue (the model's own rule).
        assert_eq!(
            outstation.queue_event(1, 73.0),
            Err(OutstationError::Class0DoesNotQueue { index: 1 })
        );
    }

    #[test]
    fn degenerate_constructions_are_refused() {
        // An invalid link config.
        assert!(matches!(
            SimulatedDnp3Master::new(
                "bad",
                LinkConfig::Tcp {
                    host: " ".to_string(),
                    port: 20_000,
                    outstation_address: 1,
                    master_address: 2,
                },
            ),
            Err(SimDnp3Error::Config(TransportError::EmptyField("tcp.host")))
        ));
        // An empty inventory (the model's own rule).
        assert_eq!(
            OutstationModel::new(format!("{SIM_DNP3_ID_PREFIX}:outstation"), Vec::new())
                .expect_err("an empty inventory is refused"),
            OutstationError::EmptyInventory
        );
    }
}
