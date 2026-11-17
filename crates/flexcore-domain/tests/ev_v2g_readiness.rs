// P17.13 — WRITTEN-UNRUN integration tests for the V2G readiness model
// (decision record
// `docs/phases-11-20/phase-17-ev-fleet-charging/records/42_p17_13_v2g_readiness_modeling.md`;
// conventions §1.3: tests are written, their execution is a human gate).
//
// The suite pins the additive contract of `flexcore_domain::ev_v2g_readiness`
// against the FROZEN five-condition `V2xConfig` gate: the gate is consumed,
// never redefined (every fixture here goes through the gate's own
// `V2xConfig::enable` procedure); the readiness verdict is derived, never
// declared; and nothing in the surface can produce a command or a lifecycle
// state (ADR-010). Nothing here transfers energy, reads a device, or claims
// conformance with OCPP, ISO 15118, or any other standard.

use flexcore_domain::ev_fleet_model::{V2xConditionEvidence, V2xConfig, V2xEvidence};
use flexcore_domain::ev_v2g_readiness::{
    BidirectionalDeclaration, Ocpp21V2xHook, SubjectKind, V2gReadinessError,
    V2gReadinessLedger, V2gReadinessLevel, V2gReadinessRecord, DECLARED_OCPP21_HOOKS,
    FIVE_CONDITION_GATE_CONSUMED_STATEMENT, NA_AC_PATH_STATEMENT,
    OCPP21_HOOKS_ARE_DESIGN_RECORDS_STATEMENT, READINESS_NEVER_AUTHORIZES_DISPATCH,
};

fn evidence(text: &str) -> V2xConditionEvidence {
    V2xConditionEvidence {
        supported: true,
        evidence: text.to_string(),
    }
}

/// The complete five-condition evidence set (SYNTHETIC labels — the named
/// fixture discipline of decision record 09_11_load_ev_models.md; never a
/// field claim).
fn full_evidence() -> V2xEvidence {
    V2xEvidence {
        ev: evidence("ev-capability-evidence"),
        evse: evidence("evse-capability-evidence"),
        protocol: evidence("protocol-scope-evidence"),
        interconnection_agreement: evidence("interconnection-evidence"),
        market_rules: evidence("market-rules-evidence"),
    }
}

fn capable_decl() -> BidirectionalDeclaration {
    BidirectionalDeclaration {
        capable: true,
        evidence: "device-model-bidirectional-evidence".to_string(),
    }
}

#[test]
fn the_frozen_gate_is_consumed_not_redefined() {
    // The gate's own enable procedure still refuses an incomplete
    // five-condition set — this module added no enable path of its own.
    let mut missing_market = full_evidence();
    missing_market.market_rules.supported = false;
    assert!(V2xConfig::enable(missing_market).is_err());
    // The default state is DISABLED (decided once, §11.5).
    let disabled = V2xConfig::disabled();
    assert!(!disabled.is_enabled());
    // The complete set enables through the gate's own procedure.
    assert!(V2xConfig::enable(full_evidence()).is_ok());
    assert!(FIVE_CONDITION_GATE_CONSUMED_STATEMENT.contains("CONSUMED here"));
}

#[test]
fn the_readiness_matrix_is_complete_and_device_truth_governs() {
    let enabled_gate = V2xConfig::enable(full_evidence()).expect("enabled");
    let disabled_gate = V2xConfig::disabled();
    let charge_only = BidirectionalDeclaration {
        capable: false,
        evidence: String::new(),
    };
    // (device capable?, gate enabled?) -> verdict: the full 2x2 matrix.
    // Charge-only, gate disabled: NotCapable.
    let record = V2gReadinessRecord::new(
        SubjectKind::Evse,
        "evse-a".to_string(),
        charge_only.clone(),
        disabled_gate.clone(),
    )
    .expect("valid");
    assert_eq!(record.readiness(), V2gReadinessLevel::NotCapable);
    // Charge-only, gate ENABLED: still NotCapable — no gate state can make
    // a charge-only device Readied.
    let record = V2gReadinessRecord::new(
        SubjectKind::Evse,
        "evse-b".to_string(),
        charge_only,
        enabled_gate.clone(),
    )
    .expect("valid");
    assert_eq!(record.readiness(), V2gReadinessLevel::NotCapable);
    // Capable, gate disabled (the §11.5 DEFAULT): CapableButGateDisabled —
    // and the record holds the gate snapshot, so the path to Readied runs
    // through `V2xConfig::enable`, never through this module.
    let record = V2gReadinessRecord::new(
        SubjectKind::Vehicle,
        "veh-a".to_string(),
        capable_decl(),
        disabled_gate,
    )
    .expect("valid");
    assert_eq!(
        record.readiness(),
        V2gReadinessLevel::CapableButGateDisabled
    );
    // Capable, gate enabled: Readied — an assessment, never an
    // authorization.
    let record = V2gReadinessRecord::new(
        SubjectKind::Vehicle,
        "veh-b".to_string(),
        capable_decl(),
        enabled_gate,
    )
    .expect("valid");
    assert_eq!(record.readiness(), V2gReadinessLevel::Readied);
}

#[test]
fn construction_refuses_untraceable_capability_claims() {
    let error = V2gReadinessRecord::new(
        SubjectKind::Evse,
        "evse-9".to_string(),
        BidirectionalDeclaration {
            capable: true,
            evidence: "   ".to_string(),
        },
        V2xConfig::disabled(),
    )
    .expect_err("refused");
    assert!(matches!(error, V2gReadinessError::EmptyEvidence { .. }));
    assert!(error.to_string().contains("evse-9"));
    assert!(error.to_string().contains("traceability"));
}

#[test]
fn the_ledger_is_deterministic_and_never_authorizes() {
    let mut ledger = V2gReadinessLedger::new();
    let gate = V2xConfig::enable(full_evidence()).expect("enabled");
    for (kind, key, decl, gate) in [
        (SubjectKind::Vehicle, "veh-2", None, None),
        (SubjectKind::Vehicle, "veh-1", Some(capable_decl()), Some(gate.clone())),
        (SubjectKind::Evse, "evse-1", None, None),
    ] {
        let declaration = decl.unwrap_or(BidirectionalDeclaration {
            capable: false,
            evidence: String::new(),
        });
        let gate = gate.unwrap_or_else(V2xConfig::disabled);
        ledger
            .record(
                V2gReadinessRecord::new(kind, key.to_string(), declaration, gate)
                    .expect("valid"),
            )
            .expect("recorded");
    }
    // Key-order determinism (BTreeMap): kind name, then subject key.
    assert_eq!(
        ledger.snapshot_keys(),
        vec![
            "EVSE:evse-1".to_string(),
            "VEHICLE:veh-1".to_string(),
            "VEHICLE:veh-2".to_string(),
        ]
    );
    assert_eq!(
        ledger.readiness_of(SubjectKind::Vehicle, "veh-1"),
        Some(V2gReadinessLevel::Readied)
    );
    // The strongest value the ledger returns is a verdict enum — there is
    // no method on it that produces a CommandState, a DispatchCommand, or
    // any lifecycle state (the structural ADR-010 guarantee, pinned by the
    // statement + the verdict-only surface).
    assert!(READINESS_NEVER_AUTHORIZES_DISPATCH.contains("never a command"));
    assert!(READINESS_NEVER_AUTHORIZES_DISPATCH.contains("three gates"));
}

#[test]
fn the_ocpp21_hooks_are_declared_design_records_never_adapter_actions() {
    let names: Vec<&str> = DECLARED_OCPP21_HOOKS
        .iter()
        .map(|hook| hook.name())
        .collect();
    assert_eq!(
        names,
        vec!["NotifyAllowedEnergyTransfer", "AFRRSignal", "SetDERControl"]
    );
    for hook in DECLARED_OCPP21_HOOKS {
        let note = Ocpp21V2xHook::note(*hook);
        assert!(note.contains("UNVERIFIED"), "hook {}", hook.name());
        assert!(note.contains("[S-125]"), "hook {}", hook.name());
    }
    assert!(OCPP21_HOOKS_ARE_DESIGN_RECORDS_STATEMENT.contains("never exercised"));
}

#[test]
fn the_na_ac_path_is_recorded_as_a_separate_future_integration() {
    assert!(NA_AC_PATH_STATEMENT.contains("SEPARATE FUTURE INTEGRATION"));
    assert!(NA_AC_PATH_STATEMENT.contains("[S-4]"));
    assert!(NA_AC_PATH_STATEMENT.contains("[S-131]"));
    assert!(NA_AC_PATH_STATEMENT.contains("[S-144]"));
    // The J3072 honesty label: named in the phase plan, no §11 source.
    assert!(NA_AC_PATH_STATEMENT.contains("UNVERIFIED/ASSUMPTION"));
}
