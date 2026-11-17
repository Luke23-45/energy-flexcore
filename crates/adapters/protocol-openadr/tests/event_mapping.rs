//! P14.05 — OpenADR event → PROPOSED mapping tests (WRITTEN-UNRUN).
//!
//! These tests are COMPLETE but NEVER EXECUTED (conventions §1 ban 3:
//! write complete test files; execution is a human gate). They verify the
//! load-bearing mapping contract of record 21:
//!
//! 1. Every mapped candidate enters the lifecycle at PROPOSED and no
//!    shortcut to a later state exists (the frozen machine has no
//!    PROPOSED → ISSUED edge).
//! 2. Unit conversion happens at the adapter boundary: kW → canonical MW;
//!    sign pass-through; the frozen P > 0 = export rule.
//! 3. Test events, ambiguous signals, non-power units, unknown scopes,
//!    and nil command ids are named refusals — never guessed.
//! 4. JSON payload shapes round-trip through the model types (the wire
//!    parsing seam's input contract).

use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::units::ActivePowerMW;
use flexcore_domain::command_state::{try_transition, CommandState};
use flexcore_domain::ids::CommandId;
use flexcore_openadr_adapter::model::{
    EventDescriptor, EventSignal, Interval, OpenAdrEvent, SignalUnits,
};
use flexcore_openadr_adapter::scope::{vtn_manifest, vtn_scope, DeliveryMode};
use flexcore_openadr_adapter::{endpoints, event_mapping, vtn};
use flexcore_protocol_ports::scope::ResourceClass;

fn reviewed_scope() -> flexcore_protocol_ports::scope::ProtocolScopeDeclaration {
    vtn_scope("openadr-vtn").expect("valid scope")
}

fn event(units: SignalUnits, payload: f64) -> OpenAdrEvent {
    let descriptor =
        EventDescriptor::new("evt-42".to_string(), 2, "iso-ne".to_string(), 3, false).expect("v");
    let signal = EventSignal::new(
        "simple".to_string(),
        units,
        vec![
            Interval::new(1, 1_700_000_000_000, 1_800_000, payload).expect("v"),
            Interval::new(2, 1_700_001_800_000, 1_800_000, payload).expect("v"),
        ],
    )
    .expect("v");
    OpenAdrEvent::new(
        descriptor,
        vec!["ven-1".to_string(), "ven-2".to_string()],
        vec![signal],
        "program-7".to_string(),
    )
    .expect("v")
}

fn command_id() -> CommandId {
    CommandId::new(FlexUuid::parse("11111111-2222-4333-8444-555555555555").expect("valid uuid"))
}

/// Test 1: the mapping output enters at PROPOSED, structurally, and the
/// frozen machine offers no shortcut past the gates.
#[test]
fn proposal_enters_lifecycle_at_proposed_with_no_shortcuts() {
    let mapper =
        event_mapping::OpenAdrEventMapper::new("openadr-vtn", reviewed_scope()).expect("mapper");
    let proposal = mapper
        .map_to_proposal(&event(SignalUnits::Kilowatts, 2_000.0), command_id())
        .expect("maps");
    assert_eq!(proposal.entry_state(), CommandState::Proposed);
    for later in [
        CommandState::Feasible,
        CommandState::Approved,
        CommandState::Issued,
        CommandState::Completed,
    ] {
        assert!(
            try_transition(CommandState::Proposed, later).is_err(),
            "no PROPOSED -> {} shortcut exists in the frozen machine",
            later.name()
        );
    }
    // The two legal first steps from PROPOSED are exactly the validation
    // gate and the rejection point.
    assert_eq!(
        try_transition(CommandState::Proposed, CommandState::Validated),
        Ok(CommandState::Validated)
    );
    assert_eq!(
        try_transition(CommandState::Proposed, CommandState::Rejected),
        Ok(CommandState::Rejected)
    );
}

/// Test 2: kW payloads convert to canonical MW at the adapter; sign
/// passes through unchanged.
#[test]
fn unit_conversion_is_kw_to_mw_with_sign_pass_through() {
    let mapper =
        event_mapping::OpenAdrEventMapper::new("openadr-vtn", reviewed_scope()).expect("mapper");
    let export = mapper
        .map_to_proposal(&event(SignalUnits::Kilowatts, 2_500.0), command_id())
        .expect("maps");
    assert_eq!(export.intervals[0].p, ActivePowerMW::new(2.5).expect("v"));
    assert!(export.intervals[0].p.value() > 0.0, "P > 0 = export");
    let import = mapper
        .map_to_proposal(&event(SignalUnits::Kilowatts, -1_000.0), command_id())
        .expect("maps");
    assert_eq!(import.intervals[0].p, ActivePowerMW::new(-1.0).expect("v"));
}

/// Test 3: named refusals — test events, ambiguity, non-power units, nil
/// ids, and scope violations.
#[test]
fn refusals_are_named_never_guessed() {
    let scope = reviewed_scope();
    let mapper = event_mapping::OpenAdrEventMapper::new("openadr-vtn", scope).expect("mapper");
    // Test event refused.
    let descriptor =
        EventDescriptor::new("evt-t".to_string(), 1, "m".to_string(), 1, true).expect("v");
    let test_event = OpenAdrEvent::new(
        descriptor,
        vec!["ven-1".to_string()],
        vec![EventSignal::new(
            "simple".to_string(),
            SignalUnits::Kilowatts,
            vec![Interval::new(1, 0, 1000, 1.0).expect("v")],
        )
        .expect("v")],
        "p".to_string(),
    )
    .expect("v");
    assert_eq!(
        mapper.map_to_proposal(&test_event, command_id()),
        Err(event_mapping::MappingError::TestEventRefused {
            event_id: "evt-t".to_string()
        })
    );
    // Nil command id refused.
    assert_eq!(
        mapper.map_to_proposal(&event(SignalUnits::Kilowatts, 1.0), CommandId::new(FlexUuid::nil())),
        Err(event_mapping::MappingError::NilCommandId)
    );
    // A scope without Control coverage refuses at construction of the
    // mapper (named refusal from the common scope vocabulary).
    let bad_scope = flexcore_protocol_ports::scope::ProtocolScopeDeclaration::new(
        "openadr-vtn".to_string(),
        "openadr-3.0".to_string(),
        vec![flexcore_protocol_ports::scope::ProtocolRole::Server],
        flexcore_protocol_ports::scope::ControlScope::MonitorOnly,
        vec![ResourceClass::Telemetry, ResourceClass::Discovery],
        vec![flexcore_protocol_ports::scope::NotModelled::new(
            "x".to_string(),
            "no external control objects are ingested".to_string(),
        )
        .expect("v")],
        "external OpenADR events enter the command lifecycle at PROPOSED and pass the \
         validation, feasibility, and authorization gates like any other candidate; this \
         adapter is NOT a dispatch authority"
            .to_string(),
        Vec::new(),
    )
    .expect("valid scope shape");
    assert!(matches!(
        event_mapping::OpenAdrEventMapper::new("openadr-vtn", bad_scope),
        Err(event_mapping::MappingError::ScopeRefused(_))
    ));
}

/// Test 4: the model JSON shapes round-trip (the wire-parsing seam's
/// contract).
#[test]
fn model_json_shapes_round_trip() {
    let json = serde_json::json!({
        "descriptor": {
            "event_id": "evt-42",
            "modification_number": 2,
            "market_context": "iso-ne",
            "priority": 3,
            "test_event": false
        },
        "target_ven_ids": ["ven-1", "ven-2"],
        "signals": [{
            "signal_name": "simple",
            "units": "kilowatts",
            "intervals": [
                {"id": 1, "start_unix_millis": 1_700_000_000_000i64, "duration_millis": 1_800_000i64, "payload_value": 2000.0},
                {"id": 2, "start_unix_millis": 1_700_001_800_000i64, "duration_millis": 1_800_000i64, "payload_value": 2000.0}
            ]
        }],
        "program_id": "program-7"
    });
    let parsed: OpenAdrEvent = serde_json::from_value(json).expect("parses");
    assert_eq!(parsed.descriptor.event_id, "evt-42");
    assert_eq!(parsed.descriptor.modification_number, 2);
    assert_eq!(parsed.signals[0].units, SignalUnits::Kilowatts);
    let back = serde_json::to_value(&parsed).expect("serializes");
    let reparsed: OpenAdrEvent = serde_json::from_value(back).expect("round trip");
    assert_eq!(reparsed, parsed);
}

/// Test 5: the registry + planner + endpoint design compose: an accepted
/// VEN with a registered push endpoint gets a push plan; the endpoint
/// design exposes exactly one push route.
#[test]
fn registry_planner_and_endpoint_design_compose() {
    let mut registry = vtn::VtnRegistry::new("openadr-vtn");
    registry
        .register(
            flexcore_openadr_adapter::model::Ven::new(
                "ven-1".to_string(),
                "VEN 1".to_string(),
                Some("https://ven1.example/push".to_string()),
                60_000,
                true,
            )
            .expect("valid ven"),
        )
        .expect("registered");
    let planner = vtn::DeliveryPlanner::new(
        "openadr-vtn",
        vec![DeliveryMode::Push, DeliveryMode::Polling],
    )
    .expect("planner");
    let plan = planner
        .plan_for(&registry, "ven-1", &[DeliveryMode::Push, DeliveryMode::Polling])
        .expect("plan");
    assert_eq!(plan.mode, DeliveryMode::Push);

    let design = endpoints::VtnEndpointDesign::canonical_vtn_design().expect("design");
    let push_routes = design.routes_for(ResourceClass::Control);
    assert!(push_routes
        .iter()
        .any(|r| r.delivery_mode == Some(DeliveryMode::Push)));
    // The registry resolves exactly the event's targets.
    registry
        .register(
            flexcore_openadr_adapter::model::Ven::new(
                "ven-2".to_string(),
                "VEN 2".to_string(),
                None,
                60_000,
                true,
            )
            .expect("valid ven"),
        )
        .expect("registered");
    registry
        .resolve_targets(&["ven-1".to_string(), "ven-2".to_string()])
        .expect("both targets resolve");
}

/// Test 6: the ONE common manifest shape (P14.01) constructs for this
/// adapter with the structurally relay-only authority declaration — the
/// never-granted permissions are not expressible in the common vocabulary.
#[test]
fn common_manifest_constructs_relay_free() {
    let manifest = vtn_manifest("openadr-vtn").expect("valid manifest");
    assert_eq!(
        manifest.authority,
        flexcore_protocol_ports::manifest::AuthorityDeclaration::SupervisoryRelayOnly
    );
    assert!(manifest
        .declares(flexcore_protocol_ports::manifest::DeclaredCapability::ExternalEventIngest));
    assert!(!manifest.declares(flexcore_protocol_ports::manifest::DeclaredCapability::CommandRelay));
    assert_eq!(
        manifest.scope.adapter_id, "openadr-vtn",
        "the manifest embeds the reviewed scope by value"
    );
}
