// P17.15 — WRITTEN-UNRUN integration tests for the simulated OCPP charge
// points (decision record
// `docs/phases-11-20/phase-17-ev-fleet-charging/records/44_p17_15_simulated_charge_points.md`;
// conventions §1.3: tests are written, their execution is a human gate).
//
// What the suite pins, end to end, with NO hardware, NO socket, NO TLS,
// and NO charge point ever connected:
//
// - a simulated charge point speaks OCPP-J to a REAL `CsmsServer` THROUGH
//   the adapter's named `CsmsTransport` seam (the same seam
//   `UnwiredCsmsTransport` refuses on) — the in-process loopback the
//   P14.17 promise named for the OCPP adapter;
// - the default depot script (boot → status → heartbeat → occupancy →
//   transaction Started/Updated/Ended → named end) is decoded by the
//   adapter's own codec and state machines;
// - a CSMS-initiated SetChargingProfile composed through the REAL
//   composer + ISSUED-only relay gate (the ISSUED state is the labelled
//   HARNESS FIXTURE) is drained, validated by the simulated station, and
//   answered Accepted with the stored-not-applied statement recorded;
// - the SYNTHETIC V2G behaviour profile attaches ONLY through the frozen
//   five-condition `V2xConfig` gate and is labelled so;
// - the seam contract holds BOTH ways: the unwired transport still
//   refuses with `CSMS_TRANSPORT_SEAM_NOT_WIRED`, the simulated one
//   completes its in-process loopback;
// - every identity string carries the `sim-ocpp` prefix.
//
// NOTHING here is a conformance, interoperability, or certification
// result; nothing here is evidence about real charge-point behaviour.

use flexcore_domain::command_state::CommandState;
use flexcore_domain::ev_fleet_model::{V2xConditionEvidence, V2xConfig, V2xEvidence};
use flexcore_ocpp_adapter::csms::{
    CsmsLimits, CsmsSecurityRequirements, CsmsServer, UnwiredCsmsTransport,
    CSMS_TRANSPORT_SEAM_NOT_WIRED,
};
use flexcore_ocpp_adapter::model::ChargePointId;
use flexcore_ocpp_adapter::scope::csms_manifest;
use flexcore_simulator::adapters::sim_ocpp::{
    RelayFixture, SimOcppError, SimulatedChargePoint, SimulatedCsmsTransport,
    SimulatedOcppLoop, SimulatedV2gProfile, V2gBehaviourLink, HARNESS_ISSUED_FIXTURE_STATEMENT,
    SIMULATED_V2G_IS_SYNTHETIC_STATEMENT, SIM_OCPP_ID_PREFIX,
};

fn evidence_for(text: &str) -> V2xConditionEvidence {
    V2xConditionEvidence {
        supported: true,
        evidence: text.to_string(),
    }
}

/// The full five-condition evidence set (SYNTHETIC labels — the
/// named-fixture discipline; never a field claim).
fn full_evidence() -> V2xEvidence {
    V2xEvidence {
        ev: evidence_for("ev-evidence"),
        evse: evidence_for("evse-evidence"),
        protocol: evidence_for("protocol-evidence"),
        interconnection_agreement: evidence_for("interconnection-evidence"),
        market_rules: evidence_for("market-evidence"),
    }
}

fn evse_params() -> flexcore_simulator::scenario::EvseParams {
    flexcore_simulator::scenario::EvseParams {
        connection_windows: vec![(17 * 3600 + 1800, 8 * 3600)], // wraps midnight
        charging_power_mw: 0.05,
        initial_soc: 0.3,
        soc_target: 0.9,
        emit_energy: true,
        jitter_fraction: 0.0,
    }
}

fn boot() -> flexcore_ocpp_adapter::model::BootNotificationRequest {
    flexcore_ocpp_adapter::model::BootNotificationRequest {
        vendor_name: "sim-vendor".to_string(),
        charge_point_model: "sim-model-1".to_string(),
        charge_point_serial_number: Some("sim-sn-001".to_string()),
        firmware_version: Some("sim-fw-1".to_string()),
        protocol: Some("OCPP 2.0.1 (simulated)".to_string()),
    }
}

fn charge_point(name: &str) -> SimulatedChargePoint {
    SimulatedChargePoint::new(
        name,
        vec!["ocpp2.0.1".to_string()],
        boot(),
        evse_params(),
        None,
        1_700_000_000_000,
    )
    .expect("valid simulated charge point")
}

fn server() -> CsmsServer {
    CsmsServer::new(
        csms_manifest().expect("valid manifest"),
        CsmsLimits::new(4, 8).expect("valid limits"),
        CsmsSecurityRequirements::new(vec!["sim-trust-anchor".to_string()], true)
            .expect("valid security"),
    )
    .expect("valid server")
}

#[test]
fn the_loopback_completes_the_full_depot_session_through_the_real_surfaces() {
    let mut server = server();
    let mut loop_ = SimulatedOcppLoop::new(charge_point("depot-1"));
    let steps = loop_
        .run_default_script(&mut server, None)
        .expect("the script completed");
    // Ten deterministic steps, every one logged with the sim identity.
    assert_eq!(steps, 10);
    for line in loop_.log() {
        assert!(line.contains("sim-ocpp:depot-1"), "line: {line}");
        assert!(line.contains(SIM_OCPP_ID_PREFIX) || line.starts_with("sim-ocpp:"));
    }
    // The transaction lifecycle travelled the REAL adapter surfaces:
    // the log names Started, Updated, and Ended for the simulated id.
    let log = loop_.log().join("\n");
    assert!(log.contains("transaction: Started sim-txn-1"));
    assert!(log.contains("transaction: Updated sim-txn-1"));
    assert!(log.contains("transaction: Ended sim-txn-1"));
    // The session ended NAMED (the real end path; connection loss is a
    // transport fact, never an asset judgment).
    assert_eq!(
        server.connection_snapshot(),
        vec![("sim-ocpp:depot-1", "ENDED")]
    );
}

#[test]
fn two_simulated_charge_points_register_deterministically() {
    let mut server = server();
    let mut a = SimulatedOcppLoop::new(charge_point("depot-1"));
    let mut b = SimulatedOcppLoop::new(charge_point("depot-2"));
    a.connect(&mut server).expect("connected");
    b.connect(&mut server).expect("connected");
    // The BTreeMap registry is deterministic (sorted identity order).
    assert_eq!(
        server.connection_snapshot(),
        vec![
            ("sim-ocpp:depot-1", "NEGOTIATED"),
            ("sim-ocpp:depot-2", "NEGOTIATED")
        ]
    );
    // A duplicate identity is refused by the real registry rule.
    let mut dup = SimulatedOcppLoop::new(charge_point("depot-1"));
    let error = dup.connect(&mut server).expect_err("duplicate refused");
    assert!(matches!(error, SimOcppError::ServerRefused(_)));
    assert!(error.to_string().contains("already connected"));
}

#[test]
fn the_relay_step_travels_the_real_composer_relay_and_tracker_path() {
    let mut server = server();
    let mut loop_ = SimulatedOcppLoop::new(charge_point("depot-1"));
    loop_.connect(&mut server).expect("connected");
    let fixture = RelayFixture::depot_default(1_700_000_000_000);
    let profile_id = loop_
        .relay_issued_profile(&mut server, &fixture)
        .expect("relayed");
    assert_eq!(profile_id, 1);
    // The stored-not-applied honesty statement travels verbatim in the
    // session log.
    let relay_line = loop_
        .log()
        .iter()
        .find(|line| line.contains("relay"))
        .expect("relay logged");
    assert!(relay_line.contains("STORED, not applied"), "{relay_line}");
    // The harness-fixture honesty note is part of the module's contract.
    assert!(HARNESS_ISSUED_FIXTURE_STATEMENT.contains("not a real dispatch"));
}

#[test]
fn the_transport_seam_holds_both_ways() {
    // The unwired transport STILL refuses with the named seam refusal —
    // wiring the simulator did not change the seam contract.
    let mut server = server();
    let error = Box::new(UnwiredCsmsTransport)
        .serve(&mut server)
        .expect_err("the unwired seam refuses");
    assert!(error.to_string().contains(CSMS_TRANSPORT_SEAM_NOT_WIRED));
    assert!(server.connection_snapshot().is_empty());
    // The simulated transport completes its IN-PROCESS loopback through
    // the same seam trait (no socket, no TLS, no HTTP anywhere).
    let mut server = server();
    let transport = SimulatedCsmsTransport::new(charge_point("depot-1")).expect("valid");
    Box::new(transport).serve(&mut server).expect("loopback ran");
    assert_eq!(
        server.connection_snapshot(),
        vec![("sim-ocpp:depot-1", "ENDED")]
    );
}

#[test]
fn the_transport_with_a_relay_fixture_reports_success_only_for_a_complete_exchange() {
    let mut server = server();
    let transport = SimulatedCsmsTransport::new(charge_point("depot-1"))
        .expect("valid transport")
        .with_relay_fixture(RelayFixture::depot_default(1_700_000_000_000));
    Box::new(transport)
        .serve(&mut server)
        .expect("the exchange completed");
    assert_eq!(
        server.connection_snapshot(),
        vec![("sim-ocpp:depot-1", "ENDED")]
    );
}

#[test]
fn an_unsupported_subprotocol_offer_meets_the_real_fail_closed_negotiation() {
    // The 1.6 offer is refused with the P17-E-1 reason; nothing registers.
    let cp = SimulatedChargePoint::new(
        "legacy",
        vec!["ocpp1.6".to_string()],
        boot(),
        evse_params(),
        None,
        0,
    )
    .expect("valid charge point");
    let mut server = server();
    let mut loop_ = SimulatedOcppLoop::new(cp);
    let error = loop_.connect(&mut server).expect_err("1.6 refused");
    assert!(error.to_string().contains("P17-E-1"));
    assert!(server.connection_snapshot().is_empty());
}

#[test]
fn the_synthetic_v2g_profile_is_gated_by_the_five_condition_gate() {
    // A DISABLED gate refuses the link (the §11.5 gate is structurally in
    // the path — the simulator cannot bypass the domain decision).
    let profile = SimulatedV2gProfile::new((22 * 3600, 6 * 3600), 0.02).expect("valid");
    assert_eq!(
        V2gBehaviourLink::attach(profile.clone(), V2xConfig::disabled()),
        Err(SimOcppError::V2gGateDisabled)
    );
    // An ENABLED gate (through the gate's own enable procedure) attaches.
    let gate = V2xConfig::enable(full_evidence()).expect("enabled");
    let link = V2gBehaviourLink::attach(profile, gate).expect("attached");
    // Window semantics (wrap midnight) and the labelled synthetic value.
    assert_eq!(link.discharge_power_mw_at(23 * 3600), 0.02);
    assert_eq!(link.discharge_power_mw_at(2 * 3600), 0.02);
    assert_eq!(link.discharge_power_mw_at(12 * 3600), 0.0);
    // A charge point WITHOUT the link is charging-only (0.0 everywhere) —
    // the Phase 2 EVSE profile's "no V2G" statement is unchanged.
    let cp = charge_point("depot-1");
    assert_eq!(cp.v2g_discharge_power_mw_at(23 * 3600), 0.0);
    assert_eq!(cp.v2g_discharge_power_mw_at(2 * 3600), 0.0);
    // The SYNTHETIC statement travels verbatim.
    assert!(SIMULATED_V2G_IS_SYNTHETIC_STATEMENT.contains("SYNTHETIC"));
    assert!(SIMULATED_V2G_IS_SYNTHETIC_STATEMENT.contains("authorizes no dispatch"));
}

#[test]
fn the_harness_fixture_issued_state_is_not_a_dispatch_path() {
    // The relay gate is REAL: a non-ISSUED state is refused by the adapter
    // itself (the same typing the wave-1 relay tests pin). The harness
    // fixture passes ONLY because it declares Issued explicitly — and the
    // module's statement says so, verbatim.
    let fixture = RelayFixture::depot_default(1_700_000_000_000);
    let plan = flexcore_domain::ev_charging_composer::compose_external_constraints_profile(
        &fixture.envelope,
        flexcore_domain::ev_charging_composer::RateUnit::W,
        &fixture.supply,
        &fixture.device_constraints,
        fixture.stack_level,
        true,
    )
    .expect("composed");
    // A PROPOSED state cannot build the wire payload — no path exists
    // from a fixture to a real dispatch.
    let error = flexcore_ocpp_adapter::charging::into_relay_request(
        &plan,
        CommandState::Proposed,
        1,
    )
    .expect_err("non-issued refused");
    assert!(error.to_string().contains("ISSUED"));
    assert!(HARNESS_ISSUED_FIXTURE_STATEMENT.contains("no validation, feasibility, or \
authorization gate was passed"));
}

#[test]
fn degenerate_simulated_charge_points_are_refused_fail_fast() {
    // Empty name.
    assert_eq!(
        SimulatedChargePoint::new(
            "  ",
            vec!["ocpp2.0.1".to_string()],
            boot(),
            evse_params(),
            None,
            0
        ),
        Err(SimOcppError::EmptyField("name"))
    );
    // Negative scenario base.
    assert_eq!(
        SimulatedChargePoint::new(
            "cp",
            vec!["ocpp2.0.1".to_string()],
            boot(),
            evse_params(),
            None,
            -1
        ),
        Err(SimOcppError::InvalidScenarioBase { base_millis: -1 })
    );
    // Degenerate EVSE params (negative charging power).
    let mut params = evse_params();
    params.charging_power_mw = -0.1;
    assert!(matches!(
        SimulatedChargePoint::new(
            "cp",
            vec!["ocpp2.0.1".to_string()],
            boot(),
            params,
            None,
            0
        ),
        Err(SimOcppError::InvalidEvseParams(_))
    ));
    // Degenerate V2G power.
    assert_eq!(
        SimulatedV2gProfile::new((0, 3600), 0.0),
        Err(SimOcppError::InvalidV2gPower { value: 0.0 })
    );
}

#[test]
fn the_csms_registry_and_server_report_the_loopback_honestly() {
    // The connection report merges the server's own honest record (the
    // session state and the security profile) — the loopback never
    // upgrades it into a platform judgment.
    let mut server = server();
    let mut loop_ = SimulatedOcppLoop::new(charge_point("depot-1"));
    loop_.connect(&mut server).expect("connected");
    loop_.send_boot(&mut server).expect("boot decoded");
    let report = server
        .connection_report(&ChargePointId::new("sim-ocpp:depot-1".to_string()).expect("valid"))
        .expect("report");
    assert!(report.detail.contains("NEGOTIATED"));
    assert!(report.detail.contains("profile 3"), "report: {}", report.detail);
}
