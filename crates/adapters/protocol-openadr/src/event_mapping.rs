// P14.05 — The OpenADR event → command-lifecycle mapping.
//
// THE load-bearing rule of this adapter (phase 14 context §3 constraint 1;
// plan §7 note 4; ADR-010 extended): an OpenADR event is an EXTERNAL
// CONTROL OBJECT. It enters the frozen 14-state command lifecycle at
// **PROPOSED** — never as an issued setpoint. The mapping below constructs
// only `ExternalCommandProposal` records whose entry state is
// `CommandState::Proposed` (structurally, via
// [`ExternalCommandProposal::entry_state`]); there is no code path in this
// crate that can construct VALIDATED, FEASIBLE, APPROVED, ISSUED, or any
// later state from external input. The three gates (validation,
// feasibility, authorization) are the platform's, and remain ahead of
// every proposal this adapter produces.
//
// The scope gate CONSUMES the common protocol scope layer (P14.01): the
// mapper holds the common `ProtocolScopeDeclaration` and runs the
// fail-closed `assert_supports` against the SERVER role and the CONTROL
// resource class before any mapping — the same pattern record 01
// prescribes (and P14.02's proposal.rs follows).
//
// Unit conversion happens HERE, at the adapter boundary
// (`crates/flexcore-contracts/src/units.rs`): signal payloads arrive in
// the event's declared unit (kW by historical OpenADR convention —
// UNVERIFIED for 3.0) and leave as canonical `ActivePowerMW` with the
// frozen P > 0 = export sign rule. Reactive power is deliberately NOT
// mapped: an OpenADR event carries no reactive-sign convention, and the
// platform never assumes one (units.rs: reactive sign is declared per
// grid model and device profile, never inferred) — a reactive-shaped
// request from a DR program is a named refusal, not a guess.

use flexcore_contracts::time_model::{ValidFrom, ValidUntil, ValidityWindow};
use flexcore_contracts::units::ActivePowerMW;
use flexcore_domain::command_state::CommandState;
use flexcore_domain::ids::CommandId;
use flexcore_protocol_ports::scope::{ProtocolRole, ProtocolScopeDeclaration, ResourceClass};

use crate::model::{OpenAdrEvent, SignalUnits};

/// Errors returned by the event mapping. Every refusal is named; nothing
/// about an unmappable event is guessed.
#[derive(Debug, Clone, PartialEq)]
pub enum MappingError {
    /// The common scope gate refused the mapping (role, control direction,
    /// or resource class outside the declared scope — fail-closed).
    ScopeRefused(flexcore_protocol_ports::scope::ProtocolScopeError),
    /// The event is a TEST event: lab traffic is never mapped into fleet
    /// candidates by default.
    TestEventRefused { event_id: String },
    /// The event declares no power-shaped signal (kW/MW) this adapter can
    /// map; amperes and percent signals are not power setpoints.
    NoPowerSignal { event_id: String },
    /// The event declares SEVERAL power-shaped signals: which one is the
    /// setpoint is not decidable here, so the mapping refuses rather than
    /// picks.
    AmbiguousPowerSignals { event_id: String, count: usize },
    /// A signal unit this design does not map (carried verbatim in the
    /// model; refused at the mapping boundary — never guessed).
    UnsupportedUnit { unit: String, event_id: String },
    /// A validity window could not be built from the event intervals.
    InvalidValidityWindow(flexcore_contracts::time_model::TimeModelError),
    /// The caller-supplied command id is the nil UUID.
    NilCommandId,
}

impl std::fmt::Display for MappingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MappingError::ScopeRefused(err) => write!(f, "scope gate refused the mapping: {err}"),
            MappingError::TestEventRefused { event_id } => {
                write!(f, "test event '{event_id}' refused: lab traffic is never mapped into fleet candidates")
            }
            MappingError::NoPowerSignal { event_id } => {
                write!(f, "event '{event_id}' declares no power-shaped signal to map")
            }
            MappingError::AmbiguousPowerSignals { event_id, count } => {
                write!(
                    f, "event '{event_id}' declares {count} power-shaped signals; the setpoint is not decidable and the mapping refuses"
                )
            }
            MappingError::UnsupportedUnit { unit, event_id } => {
                write!(f, "event '{event_id}' uses unit '{unit}', which this design does not map (refused, never guessed)")
            }
            MappingError::InvalidValidityWindow(err) => {
                write!(f, "event intervals do not form a valid validity window: {err}")
            }
            MappingError::NilCommandId => {
                write!(f, "command id is the nil UUID; the application layer assigns ids")
            }
        }
    }
}

impl std::error::Error for MappingError {}

/// One mapped delivery window: a canonical MW setpoint held over a
/// time range. The setpoint follows the frozen sign rule (P > 0 = export
/// at the point of connection) — a DR "load reduction" event that names
/// negative payload values arrives as the negative MW number it is, and
/// the platform's validation gate interprets it; the adapter never flips
/// signs on the program's behalf.
#[derive(Debug, Clone, PartialEq)]
pub struct ProposedInterval {
    /// Interval start, Unix-epoch milliseconds (UTC).
    pub start_unix_millis: i64,
    /// Interval duration in milliseconds.
    pub duration_millis: i64,
    /// The canonical active-power setpoint for the window (MW).
    pub p: ActivePowerMW,
}

/// A mapped external command candidate. This is the ONLY output type the
/// OpenADR adapter produces for events, and its entry state is
/// structurally PROPOSED (see [`ExternalCommandProposal::entry_state`]).
///
/// The candidate carries what the application layer needs to open the
/// command record (`flexcore_domain::command_lifecycle::CommandLifecycle::
/// propose` starts a lifecycle at PROPOSED); it does NOT open the record
/// itself — lifecycle ownership is the application/kernel side, never the
/// adapter.
#[derive(Debug, Clone, PartialEq)]
pub struct ExternalCommandProposal {
    /// The command id the application layer assigned to this candidate.
    pub command_id: CommandId,
    /// The verbatim OpenADR event id (audit trace: candidate ← event).
    pub source_event_id: String,
    /// The verbatim modification number (a re-published event with a
    /// higher number is a NEW candidate; SUPERSEDED invalidation of the
    /// old one is the lifecycle's own edge, decided by the platform).
    pub source_modification_number: u32,
    /// The verbatim program id.
    pub source_program_id: String,
    /// The adapter id that produced the candidate (§14.3 source subsystem
    /// value the application layer records).
    pub source_subsystem: String,
    /// The mapped delivery windows, in event order.
    pub intervals: Vec<ProposedInterval>,
    /// The validity window covering every mapped interval
    /// (min start → max end).
    pub validity: ValidityWindow,
}

impl ExternalCommandProposal {
    /// The lifecycle state this candidate enters at. ALWAYS
    /// [`CommandState::Proposed`] — structurally constant, not a field, so
    /// no code path (here or in a caller) can construct a later entry
    /// state from external input.
    pub const fn entry_state(&self) -> CommandState {
        CommandState::Proposed
    }

    /// The lifecycle states an adapter may record, and nothing else: the
    /// PROPOSED entry, and the PROPOSED-gate rejection (frozen transition
    /// 2). Every forward state is produced by the protected gates — an
    /// adapter constructing one is a defect (the P14.02 rule, shared).
    pub const ADAPTER_CONSTRUCTIBLE_STATES: [CommandState; 2] =
        [CommandState::Proposed, CommandState::Rejected];

    /// The §14.3-style reason text the application layer records for the
    /// candidate (why it was proposed and by whom).
    pub fn reason(&self) -> String {
        format!(
            "OpenADR event {} (modification {}) from program {} mapped at the adapter boundary; \
             enters the lifecycle at PROPOSED and passes validation, feasibility, and \
             authorization gates like any other candidate",
            self.source_event_id, self.source_modification_number, self.source_program_id
        )
    }
}

/// The event → proposal mapper. Constructed only through
/// [`OpenAdrEventMapper::new`], which requires the adapter's reviewed
/// COMMON scope declaration — the mapper refuses any event outside it. No
/// `Default`.
#[derive(Debug, Clone)]
pub struct OpenAdrEventMapper {
    /// The adapter id (recorded on every proposal as `source_subsystem`).
    pub adapter_id: String,
    /// The reviewed common scope declaration (the mapper's gate).
    pub scope: ProtocolScopeDeclaration,
}

impl OpenAdrEventMapper {
    /// Constructs a mapper over the given common scope declaration. The
    /// scope MUST declare the SERVER role (the VTN end) and the CONTROL
    /// resource class (events are control shapes entering at PROPOSED),
    /// or construction fails — a mapper whose scope cannot ingest external
    /// control objects is not a reviewed scope for this adapter.
    pub fn new(
        adapter_id: impl Into<String>,
        scope: ProtocolScopeDeclaration,
    ) -> Result<Self, MappingError> {
        let adapter_id = adapter_id.into();
        if !scope.covers_role(ProtocolRole::Server) {
            return Err(MappingError::ScopeRefused(
                flexcore_protocol_ports::scope::ProtocolScopeError::ScopeExceeded {
                    requested: "server role (the VTN end)".to_string(),
                    declared: scope
                        .roles
                        .iter()
                        .map(|r| r.name())
                        .collect::<Vec<_>>()
                        .join(", "),
                },
            ));
        }
        if !scope.covers_resource(ResourceClass::Control) {
            return Err(MappingError::ScopeRefused(
                flexcore_protocol_ports::scope::ProtocolScopeError::ScopeExceeded {
                    requested: "resource class control (external event ingest)".to_string(),
                    declared: scope
                        .modelled_resources
                        .iter()
                        .map(|c| c.name())
                        .collect::<Vec<_>>()
                        .join(", "),
                },
            ));
        }
        Ok(Self { adapter_id, scope })
    }

    /// Maps one OpenADR event into a PROPOSED candidate.
    ///
    /// Ordering of refusals (deterministic, for tests and review): scope →
    /// test event → power-signal selection → unit conversion → validity
    /// window.
    pub fn map_to_proposal(
        &self,
        event: &OpenAdrEvent,
        command_id: CommandId,
    ) -> Result<ExternalCommandProposal, MappingError> {
        // 1. Scope gate: fail-closed, on the common declaration (SERVER
        //    role, no command relay, CONTROL resource class).
        self.scope
            .assert_supports(ProtocolRole::Server, false, &[ResourceClass::Control])
            .map_err(MappingError::ScopeRefused)?;
        // 2. Test events are lab traffic, never fleet candidates.
        if event.descriptor.test_event {
            return Err(MappingError::TestEventRefused {
                event_id: event.descriptor.event_id.clone(),
            });
        }
        // 3. Command id: application-assigned, never nil.
        if command_id.uuid().to_bytes() == [0u8; 16] {
            return Err(MappingError::NilCommandId);
        }
        // 4. Power-signal selection: exactly one power-shaped signal, or
        //    refuse. Amperes/percent/other are not power setpoints.
        let power_signals: Vec<&crate::model::EventSignal> = event
            .signals
            .iter()
            .filter(|s| matches!(s.units, SignalUnits::Kilowatts | SignalUnits::Megawatts))
            .collect();
        if power_signals.len() > 1 {
            return Err(MappingError::AmbiguousPowerSignals {
                event_id: event.descriptor.event_id.clone(),
                count: power_signals.len(),
            });
        }
        let signal = power_signals
            .first()
            .ok_or_else(|| MappingError::NoPowerSignal {
                event_id: event.descriptor.event_id.clone(),
            })?;
        // 5. Unit conversion at the adapter boundary. kW → MW is /1000;
        //    MW passes through. Any other unit is refused (the model carries
        //    it verbatim; the mapping never guesses).
        let intervals: Vec<ProposedInterval> = signal
            .intervals
            .iter()
            .map(|interval| {
                let mw = match signal.units {
                    SignalUnits::Kilowatts => interval.payload_value / 1_000.0,
                    SignalUnits::Megawatts => interval.payload_value,
                    other => {
                        return Err(MappingError::UnsupportedUnit {
                            unit: other.to_string(),
                            event_id: event.descriptor.event_id.clone(),
                        })
                    }
                };
                Ok(ProposedInterval {
                    start_unix_millis: interval.start_unix_millis,
                    duration_millis: interval.duration_millis,
                    p: ActivePowerMW::new(mw).map_err(|_| MappingError::UnsupportedUnit {
                        // A non-finite payload cannot become a unit value;
                        // the interval constructor already refuses NaN/inf,
                        // so this arm is defensive only.
                        unit: "non-finite payload".to_string(),
                        event_id: event.descriptor.event_id.clone(),
                    })?,
                })
            })
            .collect::<Result<Vec<_>, MappingError>>()?;
        // 6. The validity window covers every mapped interval.
        let first_start = intervals
            .iter()
            .map(|i| i.start_unix_millis)
            .min()
            .unwrap_or_default();
        let last_end = intervals
            .iter()
            .map(|i| i.start_unix_millis + i.duration_millis)
            .max()
            .unwrap_or_default();
        let validity = ValidityWindow::new(
            ValidFrom::from_unix_millis(first_start),
            ValidUntil::from_unix_millis(last_end),
        )
        .map_err(MappingError::InvalidValidityWindow)?;
        Ok(ExternalCommandProposal {
            command_id,
            source_event_id: event.descriptor.event_id.clone(),
            source_modification_number: event.descriptor.modification_number,
            source_program_id: event.program_id.clone(),
            source_subsystem: self.adapter_id.clone(),
            intervals,
            validity,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{EventDescriptor, EventSignal, Interval};
    use flexcore_contracts::units::ReactiveSignConvention;

    fn event_with_signal(units: SignalUnits, payload: f64, test_event: bool) -> OpenAdrEvent {
        let descriptor =
            EventDescriptor::new("evt-1".to_string(), 1, "market".to_string(), 1, test_event)
                .expect("valid descriptor");
        let signal = EventSignal::new(
            "simple".to_string(),
            units,
            vec![
                Interval::new(1, 1_700_000_000_000, 900_000, payload).expect("valid interval"),
                Interval::new(2, 1_700_000_900_000, 900_000, payload).expect("valid interval"),
            ],
        )
        .expect("valid signal");
        OpenAdrEvent::new(
            descriptor,
            vec!["ven-1".to_string()],
            vec![signal],
            "program-1".to_string(),
        )
        .expect("valid event")
    }

    fn mapper() -> OpenAdrEventMapper {
        OpenAdrEventMapper::new(
            "openadr-vtn",
            crate::scope::vtn_scope("openadr-vtn").expect("valid scope"),
        )
        .expect("valid mapper")
    }

    fn command_id() -> CommandId {
        CommandId::new(
            flexcore_contracts::ids::FlexUuid::parse("00000000-0000-0000-0000-00000000c0de")
                .expect("valid uuid"),
        )
    }

    #[test]
    fn kw_payload_converts_to_canonical_mw() {
        let mapper = mapper();
        let proposal = mapper
            .map_to_proposal(&event_with_signal(SignalUnits::Kilowatts, 1_500.0, false), command_id())
            .expect("maps");
        assert_eq!(proposal.intervals.len(), 2);
        assert_eq!(proposal.intervals[0].p.value(), 1.5, "1500 kW is 1.5 MW");
        assert_eq!(proposal.intervals[0].p.value() > 0.0, true, "P > 0 = export (frozen rule)");
        assert_eq!(proposal.entry_state(), CommandState::Proposed);
        assert_eq!(proposal.source_subsystem, "openadr-vtn");
    }

    #[test]
    fn entry_state_is_structurally_proposed_and_no_shortcut_exists() {
        let mapper = mapper();
        let proposal = mapper
            .map_to_proposal(&event_with_signal(SignalUnits::Megawatts, -2.0, false), command_id())
            .expect("maps");
        // The entry state is PROPOSED, structurally.
        assert_eq!(proposal.entry_state(), CommandState::Proposed);
        // And the frozen machine has no PROPOSED → ISSUED (or any later)
        // shortcut: every later state is unreachable from PROPOSED in one
        // step except through the gates.
        for later in [
            CommandState::Feasible,
            CommandState::Approved,
            CommandState::Issued,
            CommandState::Acknowledged,
            CommandState::Tracking,
            CommandState::Completed,
        ] {
            assert!(flexcore_domain::command_state::try_transition(
                CommandState::Proposed,
                later
            )
            .is_err(), "no shortcut {} -> {} exists", CommandState::Proposed.name(), later.name());
        }
        // The adapter-constructible set is exactly {PROPOSED, REJECTED}.
        assert_eq!(
            ExternalCommandProposal::ADAPTER_CONSTRUCTIBLE_STATES
                .iter()
                .map(|s| s.name())
                .collect::<Vec<_>>(),
            vec!["PROPOSED", "REJECTED"]
        );
    }

    #[test]
    fn negative_payloads_pass_through_unchanged() {
        // A load-reduction event that arrives as a negative number is
        // mapped as the negative number it is; sign interpretation is the
        // platform's validation gate, never the adapter's flip.
        let mapper = mapper();
        let proposal = mapper
            .map_to_proposal(&event_with_signal(SignalUnits::Kilowatts, -800.0, false), command_id())
            .expect("maps");
        assert_eq!(proposal.intervals[0].p.value(), -0.8);
    }

    #[test]
    fn test_events_are_refused() {
        let mapper = mapper();
        assert_eq!(
            mapper.map_to_proposal(&event_with_signal(SignalUnits::Kilowatts, 1.0, true), command_id()),
            Err(MappingError::TestEventRefused {
                event_id: "evt-1".to_string()
            })
        );
    }

    #[test]
    fn ambiguous_power_signals_are_refused() {
        let mapper = mapper();
        let descriptor =
            EventDescriptor::new("evt-2".to_string(), 1, "market".to_string(), 1, false).expect("v");
        let s1 = EventSignal::new(
            "a".to_string(),
            SignalUnits::Kilowatts,
            vec![Interval::new(1, 0, 1000, 1.0).expect("v")],
        )
        .expect("v");
        let s2 = EventSignal::new(
            "b".to_string(),
            SignalUnits::Megawatts,
            vec![Interval::new(2, 0, 1000, 1.0).expect("v")],
        )
        .expect("v");
        let event =
            OpenAdrEvent::new(descriptor, vec!["ven-1".to_string()], vec![s1, s2], "p".to_string())
                .expect("v");
        assert_eq!(
            mapper.map_to_proposal(&event, command_id()),
            Err(MappingError::AmbiguousPowerSignals {
                event_id: "evt-2".to_string(),
                count: 2
            })
        );
    }

    #[test]
    fn non_power_units_are_refused_not_guessed() {
        let mapper = mapper();
        for units in [SignalUnits::Percent, SignalUnits::Amperes, SignalUnits::Other("f".to_string())] {
            assert!(
                matches!(
                    mapper.map_to_proposal(&event_with_signal(units, 50.0, false), command_id()),
                    Err(MappingError::NoPowerSignal { .. })
                        | Err(MappingError::UnsupportedUnit { .. })
                ),
                "a non-power signal is refused, never coerced"
            );
        }
    }

    #[test]
    fn validity_window_covers_every_interval() {
        let mapper = mapper();
        let proposal = mapper
            .map_to_proposal(&event_with_signal(SignalUnits::Kilowatts, 1.0, false), command_id())
            .expect("maps");
        assert_eq!(
            proposal.validity.valid_from.unix_millis(),
            1_700_000_000_000i128,
            "window starts at the first interval start"
        );
        assert_eq!(
            proposal.validity.valid_until.unix_millis(),
            1_700_001_800_000i128,
            "window ends at the last interval end"
        );
    }

    #[test]
    fn nil_command_ids_are_refused() {
        let mapper = mapper();
        assert_eq!(
            mapper.map_to_proposal(
                &event_with_signal(SignalUnits::Kilowatts, 1.0, false),
                CommandId::new(flexcore_contracts::ids::FlexUuid::nil()),
            ),
            Err(MappingError::NilCommandId)
        );
    }

    #[test]
    fn a_scope_without_control_coverage_refuses_mapping() {
        // A telemetry-only scope (no Control resource class) cannot ingest
        // events: the mapper construction refuses by name.
        let telemetry_only = flexcore_protocol_ports::scope::ProtocolScopeDeclaration::new(
            "openadr-vtn".to_string(),
            "openadr-3.0".to_string(),
            vec![ProtocolRole::Server],
            flexcore_protocol_ports::scope::ControlScope::MonitorOnly,
            vec![ResourceClass::Telemetry],
            vec![flexcore_protocol_ports::scope::NotModelled::new(
                "control_ingest".to_string(),
                "this scope ingests no external control objects".to_string(),
            )
            .expect("valid")],
            "external OpenADR events enter the command lifecycle at PROPOSED and pass the \
             validation, feasibility, and authorization gates like any other candidate; this \
             adapter is NOT a dispatch authority"
                .to_string(),
            Vec::new(),
        )
        .expect("valid scope shape");
        assert!(matches!(
            OpenAdrEventMapper::new("openadr-vtn", telemetry_only),
            Err(MappingError::ScopeRefused(_))
        ));
    }

    #[test]
    fn reactive_power_never_originates_from_openadr_events() {
        // Documentation-by-construction: the platform's reactive handling
        // REQUIRES a declared sign convention (units.rs); an OpenADR event
        // carries none, so the proposal type has no reactive field at all.
        // The compile-time proof is the type; this runtime assertion pins
        // that the convention enum exists and the proposal does not use it.
        let _ = ReactiveSignConvention::InjectionPositive;
        let proposal = mapper()
            .map_to_proposal(&event_with_signal(SignalUnits::Kilowatts, 1.0, false), command_id())
            .expect("maps");
        let fields = format!("{:?}", proposal);
        assert!(
            !fields.contains("reactive"),
            "an OpenADR proposal carries no reactive target (no declared sign convention exists)"
        );
    }
}
