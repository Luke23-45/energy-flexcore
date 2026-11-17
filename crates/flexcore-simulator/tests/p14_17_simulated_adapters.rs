// P14.17 — The simulated-counterparts integration suite (decision record
// `docs/phases-11-20/phase-14-protocol-device-connectivity/records/
// 81_p14_17_simulated_counterparts.md`). WRITTEN-UNRUN: execution is a
// human gate; nothing here has ever compiled, run, connected, polled, or
// published.
//
// What the suite pins, end to end, with NO hardware and NO wire:
//
// - the simulated 2030.5 device polls the AggregatorServer skeleton's
//   declared plan and posts a bounded device Response (the record 02 §5
//   seam, driven on the design surface);
// - the simulated 2030.5 transport serves a scripted schedule through
//   the `AggregatorTransport` seam and logs every response;
// - the simulated OpenADR VEN registers, resolves its delivery plan,
//   and accepts only events targeted at it (record 21 §3 surface);
// - the simulated Modbus device serves declared register windows that
//   decode through the adapter's own registry/decoder path (record 23
//   §4 seam);
// - the simulated DNP3 master receives the outstation's served reports
//   (record 24 §4 seam) built from the outstation model's declared
//   scaling and event queue;
// - the simulated MQTT broker sink accepts validated Sparkplug
//   publishes and feeds the REAL BirthDeathTracker (record 43 §2 seam);
// - the CIM fixture source serves a valid ProfileDocument through the
//   `RdfProfileSource` seam (record 44 §2 seam);
// - the whole simulated fleet binds through the kernel's
//   adapter-permission checks with relay-only authority fleet-wide.

use flexcore_dnp3_adapter::outstation::{OutstationModel, PointAssignment};
use flexcore_dnp3_adapter::transport::Dnp3Transport as _;
use flexcore_modbus_adapter::registry::{
    decode_register_window, DeviceModelKey, RegisterField, RegisterMap, RegisterMapRegistry,
};
use flexcore_modbus_adapter::transport::ModbusTransport as _;
use flexcore_mqtt_adapter::transport::MqttUplinkTransport as _;
use flexcore_openadr_adapter::scope::DeliveryMode;
use flexcore_openadr_adapter::vtn::{DeliveryPlanner, VtnRegistry};
use flexcore_protocol_20305::addressing::AddressPlan;
use flexcore_protocol_20305::model::{DERControl, DERControlBase, DERControlList, ListMeta, TimeType};
use flexcore_protocol_20305::server::{
    AggregatorServer, MtlsRequirements, ResourceRequestHandler as _, ServerLimits, StoredResource,
};
use flexcore_simulator::adapters::{
    permissions, sim_20305, sim_cim, sim_dnp3, sim_modbus, sim_mqtt, sim_openadr,
};

fn sim_20305_server() -> AggregatorServer {
    let scope = flexcore_protocol_ports::scope::ProtocolScopeDeclaration::new(
        flexcore_protocol_20305::ADAPTER_ID.to_string(),
        flexcore_protocol_20305::PROTOCOL_ID.to_string(),
        vec![flexcore_protocol_ports::scope::ProtocolRole::Server],
        flexcore_protocol_ports::scope::ControlScope::MonitorOnly,
        vec![
            flexcore_protocol_ports::scope::ResourceClass::Control,
            flexcore_protocol_ports::scope::ResourceClass::Discovery,
        ],
        vec![
            flexcore_protocol_ports::scope::NotModelled::new(
                "no_pki_operations".to_string(),
                "no CA operations — PKI ties to Phase 13/18 (security record 03)".to_string(),
            )
            .expect("valid"),
            flexcore_protocol_ports::scope::NotModelled::new(
                "no_field_write_authority".to_string(),
                "the aggregator never writes controls into a device; devices poll".to_string(),
            )
            .expect("valid"),
        ],
        "external control objects enter the command lifecycle at PROPOSED; this \
         adapter is NOT a dispatch authority"
            .to_string(),
        Vec::new(),
    )
    .expect("valid scope");
    let manifest = flexcore_protocol_ports::manifest::ProtocolAdapterManifest::new(
        flexcore_protocol_20305::ADAPTER_ID.to_string(),
        flexcore_protocol_20305::PROTOCOL_ID.to_string(),
        flexcore_protocol_20305::PROTOCOL_EDITION.to_string(),
        scope,
        vec![
            flexcore_protocol_ports::manifest::DeclaredCapability::Read,
            flexcore_protocol_ports::manifest::DeclaredCapability::Discovery,
            flexcore_protocol_ports::manifest::DeclaredCapability::ExternalEventIngest,
        ],
        std::time::Duration::from_secs(60),
        Vec::new(),
    )
    .expect("valid manifest");
    let mut server = AggregatorServer::new(
        manifest,
        AddressPlan::csip_default(),
        ServerLimits::new(8, 8).expect("valid limits"),
        MtlsRequirements::new(vec!["sim-trust-anchor".to_string()], true).expect("valid tls"),
    )
    .expect("valid server");
    let controls: Vec<DERControl> = (0..2)
        .map(|i| {
            DERControl::new(
                format!("sim-ctrl-{i}"),
                Some(TimeType(1)),
                Some(TimeType(1000 + i)),
                Some(300),
                None,
                None,
                DERControlBase {
                    op_mod_gen_lim_w: Some(1000.0),
                    ..DERControlBase::default()
                },
            )
            .expect("valid control")
        })
        .collect();
    server
        .install(
            AddressPlan::csip_default().der_control_list.clone(),
            StoredResource::DERControlList(DERControlList {
                meta: ListMeta::unpaged(2),
                poll_rate: Some(60),
                der_controls: controls,
            }),
        )
        .expect("installed");
    server
}

#[test]
fn the_simulated_device_and_server_close_the_2030_5_loop_on_the_design_surface() {
    let mut server = sim_20305_server();
    let mut device = sim_20305::Simulated20305Device::new("device-1", AddressPlan::csip_default());

    // Ticks 0–2 of the declared schedule poll the EndDeviceList, the
    // FunctionSetAssignmentsList, and the DERProgramList — declared but
    // not installed, so the server answers 404 each (the honest answer).
    for _ in 0..3 {
        let response = device.poll_once(&mut server);
        assert_eq!(response.status, flexcore_protocol_20305::server::ResponseStatus::NotFound);
    }
    // Tick 3 is the DERControlList: installed, so the poll returns the
    // two controls.
    let response = device.poll_once(&mut server);
    assert!(sim_20305::is_ok_with_body(&response), "installed resource polls OK");
    let body = response.body.expect("body");
    assert_eq!(body["meta"]["all"], 2);

    // The device posts a Response through the bounded write surface and
    // the server's ResponseList carries it.
    let post = device.post_response(
        &mut server,
        1,
        None,
        TimeType(10),
        serde_json::json!({ "createdDateTime": 10, "status": 1 }),
    );
    assert_eq!(post.status, flexcore_protocol_20305::server::ResponseStatus::Created);
    assert_eq!(device.posted_responses().len(), 1);
    let get = server.handle(&sim_20305::scripted_get(AddressPlan::csip_default().response_list));
    assert_eq!(get.body.as_ref().expect("body")["meta"]["all"], 1);
}

#[test]
fn the_simulated_transport_serves_a_scripted_schedule_through_the_seam() {
    let mut server = sim_20305_server();
    let plan = AddressPlan::csip_default();
    let transport = sim_20305::SimulatedAggregatorTransport::new(
        "transport-1",
        MtlsRequirements::new(vec!["sim-trust-anchor".to_string()], true).expect("valid tls"),
        vec![
            sim_20305::scripted_get(plan.der_control_list.clone()),
            sim_20305::scripted_get(plan.response_list.clone()),
        ],
    )
    .expect("valid transport");
    let log = transport.response_log();
    Box::new(transport)
        .serve(&mut server)
        .expect("the scripted serve completes");
    let responses = log.lock().expect("log mutex");
    assert_eq!(responses.len(), 2);
    assert!(sim_20305::is_ok_with_body(&responses[0]));
}

#[test]
fn the_simulated_ven_resolves_its_plan_and_accepts_only_targeted_events() {
    let mut registry = VtnRegistry::new("openadr-vtn");
    let ven = sim_openadr::SimulatedVen::new("ven-1", 60_000, None, true, vec![
        DeliveryMode::Polling,
    ])
    .expect("valid simulated ven");
    ven.register_in(&mut registry).expect("registered");
    let planner = DeliveryPlanner::new("openadr-vtn", vec![DeliveryMode::Polling, DeliveryMode::Push])
        .expect("valid planner");
    let plan = ven.resolve_delivery_plan(&registry, &planner).expect("polling plan");
    assert_eq!(plan.mode, DeliveryMode::Polling);
    // The agreed cadence is the deterministic poll frequency.
    assert_eq!(ven.poll_frequency_millis(), 60_000);
    // Only events targeted at the simulated record are accepted.
    let full_id = ven.ven.ven_id.clone();
    let descriptor = flexcore_openadr_adapter::model::EventDescriptor::new(
        "sim-evt-1".to_string(),
        1,
        "sim-market".to_string(),
        1,
        false,
    )
    .expect("valid descriptor");
    let signal = flexcore_openadr_adapter::model::EventSignal::new(
        "simple".to_string(),
        flexcore_openadr_adapter::model::SignalUnits::Kilowatts,
        vec![flexcore_openadr_adapter::model::Interval::new(
            1,
            1_700_000_000_000,
            900_000,
            42.0,
        )
        .expect("valid interval")],
    )
    .expect("valid signal");
    let targeted = flexcore_openadr_adapter::model::OpenAdrEvent::new(
        descriptor.clone(),
        vec![full_id.clone()],
        vec![signal.clone()],
        "sim-program".to_string(),
    )
    .expect("valid event");
    let not_targeted = flexcore_openadr_adapter::model::OpenAdrEvent::new(
        descriptor,
        vec!["someone-else".to_string()],
        vec![signal],
        "sim-program".to_string(),
    )
    .expect("valid event");
    assert!(ven.receive_event(not_targeted).is_err());
    ven.receive_event(targeted).expect("accepted");
    assert_eq!(ven.accepted_events().len(), 1);
}

#[test]
fn the_simulated_modbus_device_feeds_the_real_decode_path() {
    let device = sim_modbus::SimulatedModbusDevice::new(
        "inverter-1",
        flexcore_modbus_adapter::transport::TransportConfig::Tcp {
            host: "sim-gateway.invalid".to_string(),
            port: 502,
            unit_id: 1,
        },
        40_001,
        vec![0xFA1C, 0x0001, 0x86A0, 0x0000],
    )
    .expect("valid simulated device");
    let mut registry = RegisterMapRegistry::new("modbus-client");
    let map = RegisterMap::new(
        DeviceModelKey::new("sim-modbus/inverter-1").expect("valid key"),
        flexcore_modbus_adapter::model::SunSpecModel::InverterThreePhase,
        vec![
            RegisterField::new(
                "W",
                flexcore_modbus_adapter::model::SunSpecModel::InverterThreePhase,
                0,
                flexcore_modbus_adapter::model::RegisterType::I16,
                None,
                flexcore_modbus_adapter::model::RegisterUnitKind::Watts,
            )
            .expect("valid field"),
        ],
        4,
    )
    .expect("valid map");
    registry.register(map).expect("registered");
    let registered = registry
        .map_for(&DeviceModelKey::new("sim-modbus/inverter-1").expect("valid key"))
        .expect("map present");
    let window = device.read_holding_registers(40_001, 4).expect("serves");
    let field = registered.field("W").expect("mapped").clone();
    let decoded = decode_register_window(registered, &field, &window).expect("decodes");
    assert_eq!(decoded.value, -1500.0, "the simulated block decodes like a real window");
}

#[test]
fn the_simulated_dnp3_master_receives_the_outstations_served_reports() {
    let master = sim_dnp3::SimulatedDnp3Master::new(
        "scada-master",
        flexcore_dnp3_adapter::transport::LinkConfig::Tcp {
            host: "sim-scada.invalid".to_string(),
            port: 20_000,
            outstation_address: 1,
            master_address: 2,
        },
    )
    .expect("valid simulated master");
    let mut outstation = OutstationModel::new(
        "sim-dnp3:outstation",
        vec![PointAssignment {
            index: 0,
            kind: flexcore_dnp3_adapter::model::DerPointKind::ActivePowerExport,
            scaling: flexcore_dnp3_adapter::model::DeclaredScaling {
                canonical_to_source_factor: 1000.0,
                reactive_sign_convention: None,
            },
            event_class: flexcore_dnp3_adapter::model::EventClass::Class1,
        }],
    )
    .expect("valid outstation");
    let scaled = outstation
        .scale_active_power(0, flexcore_contracts::units::ActivePowerMW::new(2.5).expect("valid"))
        .expect("scales");
    outstation.queue_event(0, scaled).expect("class 1 queues");
    let report = flexcore_dnp3_adapter::transport::OutstationReport {
        static_points: vec![(0, scaled)],
        events: outstation.queued_events().to_vec(),
    };
    let log = master.received_log();
    master.serve_report(report).expect("served");
    let received = log.lock().expect("log mutex");
    assert_eq!(received.len(), 1);
    assert_eq!(received[0].report.events.len(), 1);
    assert_eq!(received[0].report.events[0].source_value, 2500.0);
}

#[test]
fn the_simulated_mqtt_broker_acks_valid_publishes_and_tracks_the_session() {
    let mut broker = sim_mqtt::SimulatedMqttBroker::new(
        "broker-1",
        flexcore_mqtt_adapter::transport::MqttConnectionConfig {
            endpoint: "mqtts://sim-broker.invalid:8883".to_string(),
            require_tls: true,
            client_id: "sim-edge-07".to_string(),
            qos: flexcore_mqtt_adapter::transport::DeclaredQos::AtLeastOnce,
            trust_anchor_reference: "phase13-secrets/sim-trust-anchor".to_string(),
        },
    )
    .expect("valid simulated broker");
    let topic = flexcore_mqtt_adapter::topic::ValidatedTopic::new(
        flexcore_mqtt_adapter::model::GroupId::new("sim-site-42").expect("valid"),
        flexcore_mqtt_adapter::model::SparkplugMessageKind::NodeBirth,
        flexcore_mqtt_adapter::model::EdgeNodeId::new("sim-edge-07").expect("valid"),
        None,
    )
    .expect("valid topic");
    let payload = flexcore_mqtt_adapter::model::SparkplugPayload {
        kind: flexcore_mqtt_adapter::model::SparkplugMessageKind::NodeBirth,
        seq: flexcore_mqtt_adapter::model::SparkplugSeq::new(0).ok(),
        bd_seq: flexcore_mqtt_adapter::model::BdSeq::initial(),
        metrics: Vec::new(),
    };
    let receipt = broker.publish(&topic, &payload).expect("accepted");
    assert!(receipt.topic.ends_with("/NBIRTH/sim-edge-07"));
    assert_eq!(broker.accepted_count(), 1);
    // The broker-side session view is the REAL tracker.
    let tracker = broker.tracker("sim-edge-07").expect("tracker");
    assert!(matches!(
        tracker.state(),
        flexcore_mqtt_adapter::birth_death::NodeSessionState::Live { .. }
    ));
}

#[test]
fn the_cim_fixture_source_serves_a_valid_document_through_the_seam() {
    let source = sim_cim::FixtureProfileSource::new(
        "cgmes-fixture",
        sim_cim::canonical_fixture().expect("valid fixture"),
    );
    let parsed = source.parse(b"fixture-selection-key").expect("serves");
    assert_eq!(parsed.len(), 3);
    // Empty input: the named not-well-formed refusal.
    assert!(source.parse(b"").is_err());
}

#[test]
fn the_whole_simulated_fleet_binds_through_the_kernel_permission_checks() {
    let registry = permissions::bind_simulated_fleet().expect("all six bind");
    assert_eq!(registry.adapter_ids().len(), 6);
    // Relay-only fleet-wide: no simulated adapter gained authority its
    // real counterpart lacks.
    assert!(registry.command_relay_adapters().is_empty());
}
