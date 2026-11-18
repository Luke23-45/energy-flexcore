// P14.04 — IEEE 2030.5 / CSIP use-case matrix, part 1 of 2: the resource-
// tree use cases (decision record
// docs/phases-11-20/phase-14-protocol-device-connectivity/records/
// 04_p14_04_conformance_test_suite.md).
//
// **WRITTEN-UNRUN.** These tests have never been compiled or executed —
// the no-build/no-test bans (program conventions §1) make execution a
// HUMAN GATE. Nothing here is a conformance result: CSIP certification is
// a human-gated lab process (conventions §7.3; QualityLogic/CSIP labs,
// [S-5]); this suite is in-repo EVIDENCE PREPARATION only, named for the
// P14.19 evidence plan.
//
// **UNVERIFIED standard detail.** Every use case below encodes behaviour
// transcribed from SECONDARY sources ([S-1][S-3][S-5][S-156]); the
// attribute spellings, href paths, list semantics, and status vocabularies
// carry UNVERIFIED labels at their definitions (model.rs / addressing.rs /
// server.rs module headers). When the implementation-time human check
// against the IEEE 2030.5-2018 text / CSIP Implementation Guide finds a
// mismatch, the fix is a code or record edit here — never a silent
// reinterpretation.
//
// Use-case matrix (part 1 — resource tree):
//
// | # | Use case | Test |
// |---|---|---|
// | U1 | Device enrollment appears in the EndDeviceList | `u1_enrollment...` |
// | U2 | FunctionSetAssignments binds DERP and RSPS to a device | `u2_fsa_binding...` |
// | U3 | DERProgram exposes DefaultDERControl and DERControlList | `u3_program_exposes...` |
// | U4 | DefaultDERControl applies when no DERControl is active | `u4_default_control...` |
// | U5 | DERControl scheduling: start/duration/randomization | `u5_control_scheduling...` |
// | U6 | Device posts a Response for an applied control | `u6_response_flow...` |
// | U7 | Device posts LogEvents; the aggregator retains them | `u7_log_event_flow...` |
// | U8 | PricingList is present in the served tree | `u8_pricing_presence...` |
// | U9 | List paging semantics (limit/offset, meta recount) | `u9_paging...` |
// | U10 | Undeclared paths are refused indistinguishably | `u10_undeclared...` |

use flexcore_protocol_20305::addressing::{AddressPlan, ResourceHref, ResourceQuery};
use flexcore_protocol_20305::model::{
    DefaultDERControl, DERControl, DERControlBase, DERProgram, EndDevice, FunctionSetAssignments,
    ListMeta, LogEvent, Lfdi, Pricing, Response, Sfdi, TimeType,
};
use flexcore_protocol_20305::server::{
    AggregatorServer, MtlsRequirements, RequestMethod, ResourceRequest, ResourceRequestHandler,
    ResponseStatus, ServerLimits, StoredResource,
};

fn href(path: &str) -> ResourceHref {
    ResourceHref::new(path.to_string()).expect("valid href")
}

fn server_with_tree() -> (AggregatorServer, AddressPlan) {
    let plan = AddressPlan::csip_default();
    let mut server = AggregatorServer::new(
        manifest(),
        plan.clone(),
        ServerLimits::new(16, 16).expect("valid limits"),
        MtlsRequirements::new(vec!["csip-device-ca".to_string()], true).expect("valid tls"),
    )
    .expect("valid server");

    // U1: one enrolled device.
    server
        .install(
            plan.end_device_list.clone(),
            StoredResource::EndDeviceList(flexcore_protocol_20305::model::EndDeviceList {
                meta: ListMeta::unpaged(1),
                poll_rate: Some(900),
                end_devices: vec![EndDevice {
                    sfdi: Sfdi::new(1_234_567_891).expect("valid sfdi"),
                    lfdi: Some(Lfdi::new("0a1b2c3d4e5f".to_string()).expect("valid lfdi")),
                    function_set_assignments_link: Some(href("/fn/fsa-1")),
                    der_program_list_link: Some(href("/derp/1")),
                }],
            }),
        )
        .expect("install devices");

    // U2: the FSA binds the program and response lists.
    server
        .install(
            plan.function_set_assignments_list.clone(),
            StoredResource::FunctionSetAssignmentsList(
                flexcore_protocol_20305::model::FunctionSetAssignmentsList {
                    meta: ListMeta::unpaged(1),
                    poll_rate: Some(900),
                    function_set_assignments: vec![FunctionSetAssignments::new(
                        "fsa-1".to_string(),
                        Some(5),
                        Some(href("/derp")),
                        Some(href("/rsps")),
                    )
                    .expect("valid fsa")],
                },
            ),
        )
        .expect("install fsa");

    // U3: the program exposes its default control and control list.
    server
        .install(
            plan.der_program_list.clone(),
            StoredResource::DERProgramList(flexcore_protocol_20305::model::DERProgramList {
                meta: ListMeta::unpaged(1),
                poll_rate: Some(900),
                der_programs: vec![DERProgram::new(
                    "prog-1".to_string(),
                    Some(plan.default_der_control.clone()),
                    Some(plan.der_control_list.clone()),
                )
                .expect("valid program")],
            }),
        )
        .expect("install programs");

    // U4: the default control of the program.
    server
        .install(
            plan.default_der_control.clone(),
            StoredResource::DefaultDERControl(DefaultDERControl {
                mrid: "dderc-1".to_string(),
                der_control_base: DERControlBase {
                    op_mod_gen_lim_w: Some(5000.0),
                    ..DERControlBase::default()
                },
            }),
        )
        .expect("install default control");

    // U5: two scheduled controls.
    server
        .install(
            plan.der_control_list.clone(),
            StoredResource::DERControlList(flexcore_protocol_20305::model::DERControlList {
                meta: ListMeta::unpaged(2),
                poll_rate: Some(60),
                der_controls: vec![
                    DERControl::new(
                        "ctrl-a".to_string(),
                        Some(TimeType(1_000)),
                        Some(TimeType(10_000)),
                        Some(3_600),
                        None,
                        None,
                        DERControlBase {
                            op_mod_gen_lim_w: Some(2_000.0),
                            ..DERControlBase::default()
                        },
                    )
                    .expect("valid control"),
                    DERControl::new(
                        "ctrl-b".to_string(),
                        Some(TimeType(1_001)),
                        Some(TimeType(20_000)),
                        Some(1_800),
                        Some(300),
                        None,
                        DERControlBase {
                            op_mod_load_lim_w: Some(1_500.0),
                            ..DERControlBase::default()
                        },
                    )
                    .expect("valid control"),
                ],
            }),
        )
        .expect("install controls");

    // U8: pricing presence.
    server
        .install(
            plan.pricing_list.clone(),
            StoredResource::PricingList(flexcore_protocol_20305::model::PricingList {
                meta: ListMeta::unpaged(1),
                poll_rate: Some(3_600),
                pricings: vec![Pricing::new("price-1".to_string(), "TOU rate".to_string())
                    .expect("valid pricing")],
            }),
        )
        .expect("install pricing");

    (server, plan)
}

fn manifest() -> flexcore_protocol_ports::manifest::ProtocolAdapterManifest {
    let scope = flexcore_protocol_ports::scope::ProtocolScopeDeclaration::new(
        flexcore_protocol_20305::ADAPTER_ID.to_string(),
        flexcore_protocol_20305::PROTOCOL_ID.to_string(),
        vec![flexcore_protocol_ports::scope::ProtocolRole::Server],
        flexcore_protocol_ports::scope::ControlScope::MonitorOnly,
        vec![
            flexcore_protocol_ports::scope::ResourceClass::Control,
            flexcore_protocol_ports::scope::ResourceClass::Discovery,
            flexcore_protocol_ports::scope::ResourceClass::Pricing,
            flexcore_protocol_ports::scope::ResourceClass::EventLog,
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
        "external control objects enter the command lifecycle at PROPOSED; this adapter \
         is NOT a dispatch authority"
            .to_string(),
        Vec::new(),
    )
    .expect("valid scope");
    flexcore_protocol_ports::manifest::ProtocolAdapterManifest::new(
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
    .expect("valid manifest")
}

fn get(server: &mut AggregatorServer, path: &str, query: Option<ResourceQuery>) -> serde_json::Value {
    let response = server.handle(&ResourceRequest {
        method: RequestMethod::Get,
        href: href(path),
        query,
        body: None,
    });
    assert_eq!(
        response.status,
        ResponseStatus::Ok,
        "GET {path} must succeed"
    );
    response.body.expect("body present")
}

// --- U1: enrollment -----------------------------------------------------------

#[test]
fn u1_enrollment_appears_in_the_end_device_list() {
    let (mut server, plan) = server_with_tree();
    let body = get(&mut server, plan.end_device_list.as_str(), None);
    assert_eq!(body["meta"]["all"], 1);
    let devices = body["endDevices"].as_array().expect("device array");
    assert_eq!(devices.len(), 1);
    assert_eq!(devices[0]["sfdi"], 1_234_567_891);
    // The device carries its FSA and program links (the enrollment shape).
    assert_eq!(devices[0]["functionSetAssignmentsLink"], "/fn/fsa-1");
    assert_eq!(devices[0]["derProgramListLink"], "/derp/1");
}

// --- U2: FSA binding ----------------------------------------------------------

#[test]
fn u2_fsa_binds_the_program_and_response_lists() {
    let (mut server, plan) = server_with_tree();
    let body = get(
        &mut server,
        plan.function_set_assignments_list.as_str(),
        None,
    );
    let fsas = body["functionSetAssignments"].as_array().expect("fsa array");
    assert_eq!(fsas.len(), 1);
    assert_eq!(fsas[0]["mrid"], "fsa-1");
    // The declared primacy travels with the assignment (UNVERIFIED field
    // semantics; carried, never inferred).
    assert_eq!(fsas[0]["primacy"], 5);
    assert_eq!(fsas[0]["derProgramListLink"], "/derp");
    assert_eq!(fsas[0]["responseListLink"], "/rsps");
}

// --- U3: program structure ------------------------------------------------------

#[test]
fn u3_program_exposes_default_control_and_control_list() {
    let (mut server, plan) = server_with_tree();
    let body = get(&mut server, plan.der_program_list.as_str(), None);
    let programs = body["derPrograms"].as_array().expect("program array");
    assert_eq!(programs.len(), 1);
    assert_eq!(programs[0]["mrid"], "prog-1");
    // Plain camelCase, the model's uniform rule (`rename_all =
    // "camelCase"` on every struct; spellings UNVERIFIED per the lib
    // header): `default_der_control_link` serializes as
    // `defaultDerControlLink`, matching the sibling `derControlListLink`
    // — not the spec's DER-caps `defaultDERControlLink`. If the wire
    // spelling is ever verified as DER-caps, the MODEL needs an explicit
    // rename, not a test change.
    assert_eq!(programs[0]["defaultDerControlLink"], "/derp/dderc");
    assert_eq!(programs[0]["derControlListLink"], "/derc");
}

// --- U4: default control fallback ------------------------------------------------

#[test]
fn u4_default_control_is_served_as_the_no_active_control_state() {
    let (mut server, plan) = server_with_tree();
    let body = get(&mut server, plan.default_der_control.as_str(), None);
    assert_eq!(body["mrid"], "dderc-1");
    // The default base carries the fallback generation limit (native W;
    // canonical conversion happens at the ACL, never here).
    assert_eq!(body["derControlBase"]["opModGenLimW"], 5000.0);
}

// --- U5: DERControl scheduling ----------------------------------------------------

#[test]
fn u5_control_scheduling_windows_answer_active_at_correctly() {
    let (mut server, plan) = server_with_tree();
    let body = get(&mut server, plan.der_control_list.as_str(), None);
    let controls = body["derControls"].as_array().expect("control array");
    assert_eq!(controls.len(), 2);
    let first: DERControl = serde_json::from_value(controls[0].clone()).expect("decodes");
    let window = first.window().expect("window present");
    // ctrl-a: [10_000, 13_600) — active inside, inactive outside.
    assert_eq!(window.active_at(TimeType(10_000)), Some(true));
    assert_eq!(window.active_at(TimeType(13_599)), Some(true));
    assert_eq!(window.active_at(TimeType(13_600)), Some(false));
    // ctrl-b carries a declared randomization offset: the effective start
    // shifts to 20_300 (the conservative declared reading; UNVERIFIED —
    // see the scheduling module header).
    let second: DERControl = serde_json::from_value(controls[1].clone()).expect("decodes");
    let window_b = second.window().expect("window present");
    assert_eq!(window_b.effective_start(), TimeType(20_300));
    assert_eq!(window_b.active_at(TimeType(20_200)), Some(false));
    assert_eq!(window_b.active_at(TimeType(20_300)), Some(true));
}

// --- U6: response flow --------------------------------------------------------------

#[test]
fn u6_device_response_is_recorded_and_served() {
    let (mut server, plan) = server_with_tree();
    let response_payload = Response {
        href: None,
        end_device_lfdi: Some(Lfdi::new("0a1b2c3d4e5f".to_string()).expect("valid lfdi")),
        created_date_time: Some(TimeType(2_000)),
        status: Some(1),
        subject: Some(href("/derc/ctrl-a")),
    };
    let created = server.handle(&ResourceRequest {
        method: RequestMethod::Post,
        href: plan.response_list.clone(),
        query: None,
        body: Some(serde_json::to_value(&response_payload).expect("serializes")),
    });
    assert_eq!(created.status, ResponseStatus::Created);
    let body = get(&mut server, plan.response_list.as_str(), None);
    let responses = body["responses"].as_array().expect("response array");
    assert_eq!(responses.len(), 1);
    assert_eq!(responses[0]["status"], 1);
    assert_eq!(responses[0]["subject"], "/derc/ctrl-a");
}

// --- U7: log-event flow ---------------------------------------------------------------

#[test]
fn u7_device_log_events_are_recorded_bounded_and_served() {
    let (mut server, plan) = server_with_tree();
    for i in 0..4u8 {
        server
            .record_log_event(
                LogEvent::new(
                    Some(TimeType(i as i64)),
                    format!("enrollment step {i}"),
                    Some("fs-control".to_string()),
                    None,
                )
                .expect("valid event"),
            )
            .expect("recorded");
    }
    let body = get(&mut server, plan.log_event_list.as_str(), None);
    let events = body["logEvents"].as_array().expect("event array");
    assert_eq!(events.len(), 4);
    assert_eq!(events[0]["details"], "enrollment step 0");
}

// --- U8: pricing presence ---------------------------------------------------------------

#[test]
fn u8_pricing_list_is_present_in_the_served_tree() {
    let (mut server, plan) = server_with_tree();
    let body = get(&mut server, plan.pricing_list.as_str(), None);
    assert_eq!(body["meta"]["all"], 1);
    // The minimal pricing record: the class inventory is UNVERIFIED (the
    // model.rs register) — this suite asserts PRESENCE and shape of OUR
    // declared type, never protocol pricing semantics.
    assert_eq!(body["pricings"][0]["mrid"], "price-1");
}

// --- U9: paging ---------------------------------------------------------------------------

#[test]
fn u9_list_paging_recounts_meta_and_slices_entries() {
    let (mut server, plan) = server_with_tree();
    let body = get(
        &mut server,
        plan.der_control_list.as_str(),
        Some(ResourceQuery {
            href: plan.der_control_list.clone(),
            sfdi: None,
            limit: Some(1),
            offset: Some(1),
        }),
    );
    assert_eq!(body["meta"]["all"], 2);
    assert_eq!(body["meta"]["results"], 1);
    let controls = body["derControls"].as_array().expect("control array");
    assert_eq!(controls.len(), 1);
    assert_eq!(controls[0]["mrid"], "ctrl-b");
}

// --- U10: undeclared paths -------------------------------------------------------------------

#[test]
fn u10_undeclared_and_uninstalled_paths_are_indistinguishable() {
    let (mut server, plan) = server_with_tree();
    // Not declared by the plan.
    let response = server.handle(&ResourceRequest {
        method: RequestMethod::Get,
        href: href("/secret"),
        query: None,
        body: None,
    });
    assert_eq!(response.status, ResponseStatus::NotFound);
    // Declared but not installed (response list has nothing yet).
    let response = server.handle(&ResourceRequest {
        method: RequestMethod::Get,
        href: plan.response_list.clone(),
        query: None,
        body: None,
    });
    assert_eq!(response.status, ResponseStatus::NotFound);
    // A write to a read-only collection is refused (NotAllowed — the only
    // distinguishable refusal, and only for DECLARED collections).
    let response = server.handle(&ResourceRequest {
        method: RequestMethod::Post,
        href: plan.der_control_list.clone(),
        query: None,
        body: Some(serde_json::json!({"mrid": "injected"})),
    });
    assert_eq!(response.status, ResponseStatus::NotAllowed);
}
