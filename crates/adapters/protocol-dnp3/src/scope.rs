// P14.08 — The DNP3 adapter's scope declaration and manifest.
//
// CONSUMES the common protocol scope layer (P14.01,
// `crates/flexcore-protocol-ports`) — the same `ProtocolScopeDeclaration`
// and one `ProtocolAdapterManifest` shape every Phase 14 protocol adapter
// implements (decision record 01; this crate was reconciled onto the
// landed layer). The grid-api discipline (declared never inferred,
// mandatory non-empty `not_modelled`, fail-closed `assert_supports`, no
// `Default`) is enforced THERE; this module adds only the reviewed
// DNP3 CONSTRUCTORS ([`outstation_scope`], [`outstation_manifest`]).
//
// Role mapping onto the common vocabulary: the OUTSTATION is the passive
// end answering the utility master — the common SERVER role. The MASTER
// is the active end — the common CLIENT role, which this scope does NOT
// declare: any master-role request is refused at the common scope gate,
// and the record-level assessment (`crate::master`) refuses it by name.
// The DER point-kind vocabulary ([`crate::model::DerPointKind`]) is
// protocol-specific elaboration travelling in the scope notes and
// enforced by the outstation model's declared inventory.

use flexcore_protocol_ports::manifest::ProtocolAdapterManifest;
use flexcore_protocol_ports::scope::{
    ControlScope, NotModelled, ProtocolRole, ProtocolScopeDeclaration, ProtocolScopeError,
    ResourceClass,
};

/// The reviewed `not_modelled` record of the DNP3 outstation adapter
/// (travels inside the common scope declaration; non-empty is enforced
/// there).
fn reviewed_not_modelled() -> Vec<NotModelled> {
    vec![
        NotModelled::new(
            "master_role".to_string(),
            "the DNP3 master role is not modelled; it is an assessment record (P14.08: \
             outstation first)"
                .to_string(),
        )
        .expect("valid"),
        NotModelled::new(
            "object_numbers".to_string(),
            "object group/variation numbers and select-before-operate semantics are \
             UNVERIFIED against the IEEE 1815.2-2025 text [S-14]"
                .to_string(),
        )
        .expect("valid"),
        NotModelled::new(
            "wire_transport".to_string(),
            "no DNP3 stack or wire transport is implemented; the transport is a designed, \
             UNWIRED seam"
                .to_string(),
        )
        .expect("valid"),
        NotModelled::new(
            "secure_transport".to_string(),
            "no secure-transport control is implemented and no security claim is made; the \
             posture note (crate::secure_transport) is an input to a security review"
                .to_string(),
        )
        .expect("valid"),
    ]
}

/// The reviewed common scope declaration of the DNP3 outstation adapter.
///
/// Role: SERVER (the outstation is the passive end the utility master
/// polls). Control scope: MONITOR ONLY. Resource classes: TELEMETRY (the
/// fleet snapshot and events the outstation reports) and CONTROL (the
/// controls the utility master sends — mapped at PROPOSED only, never
/// issued by the adapter).
pub fn outstation_scope(adapter_id: &str) -> Result<ProtocolScopeDeclaration, ProtocolScopeError> {
    ProtocolScopeDeclaration::new(
        adapter_id.to_string(),
        "dnp3-1815.2-der".to_string(),
        vec![ProtocolRole::Server],
        ControlScope::MonitorOnly,
        vec![ResourceClass::Telemetry, ResourceClass::Control],
        reviewed_not_modelled(),
        "controls arriving from the utility master enter the command lifecycle at PROPOSED \
         and pass the validation, feasibility, and authorization gates like any other \
         candidate; this adapter is NOT a dispatch authority"
            .to_string(),
        vec![
            "role mapping: outstation = common SERVER role; the master (common CLIENT role) \
             is NOT declared and is refused at this gate"
                .to_string(),
            "declared point kinds travel in the outstation inventory (crate::outstation): \
             active power, reactive power, SOC, availability, energy, setpoint"
                .to_string(),
        ],
    )
}

/// The reviewed common capability manifest of the DNP3 outstation adapter
/// (the ONE manifest shape every protocol adapter implements, record 01).
/// Authority is structurally `SupervisoryRelayOnly`; the never-granted
/// permissions frozen in `crates/flexcore-kernel/src/manifest.rs` are not
/// expressible in the common manifest vocabulary.
pub fn outstation_manifest(
    adapter_id: &str,
) -> Result<ProtocolAdapterManifest, flexcore_protocol_ports::manifest::ManifestError> {
    ProtocolAdapterManifest::new(
        adapter_id.to_string(),
        "dnp3-1815.2-der".to_string(),
        "IEEE 1815.2-2025 DER profile (edition from [S-14]; profile details UNVERIFIED)"
            .to_string(),
        outstation_scope(adapter_id)?,
        vec![
            flexcore_protocol_ports::manifest::DeclaredCapability::Read,
            flexcore_protocol_ports::manifest::DeclaredCapability::ExternalEventIngest,
        ],
        // The DEFAULT integrity-poll cadence seed; the utility's declared
        // poll schedule is the reviewable per-deployment value.
        std::time::Duration::from_secs(60),
        vec![
            "declared cadence: the DEFAULT poll-response cadence seed (60 s); the utility's \
             point list and poll schedule govern the real deployment"
                .to_string(),
            "certification is a human-gated lab process; this manifest is a declaration, \
             never a conformance claim (conventions §7.3)"
                .to_string(),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outstation_scope_constructs_and_refuses_the_master_role() {
        let scope = outstation_scope("dnp3-outstation").expect("valid scope");
        assert!(scope.covers_role(ProtocolRole::Server));
        assert!(!scope.covers_role(ProtocolRole::Client), "the master role is NOT declared");
        assert!(scope.covers_resource(ResourceClass::Telemetry));
        assert!(scope.covers_resource(ResourceClass::Control));
        assert!(!scope.covers_relay(), "monitor-only: no command relay in this phase");
        assert_eq!(scope.coverage_limitation_statements().len(), 4);
        // Outstation reporting (telemetry) is in scope.
        scope
            .assert_supports(ProtocolRole::Server, false, &[ResourceClass::Telemetry])
            .expect("within scope");
        // A master-role request (the CLIENT role) is refused at the gate.
        assert!(scope
            .assert_supports(ProtocolRole::Client, false, &[ResourceClass::Telemetry])
            .is_err());
    }

    #[test]
    fn outstation_manifest_constructs_relay_free() {
        let manifest = outstation_manifest("dnp3-outstation").expect("valid manifest");
        assert_eq!(manifest.protocol_id, "dnp3-1815.2-der");
        assert_eq!(
            manifest.authority,
            flexcore_protocol_ports::manifest::AuthorityDeclaration::SupervisoryRelayOnly
        );
        assert!(manifest.declares(flexcore_protocol_ports::manifest::DeclaredCapability::Read));
        assert!(manifest.declares(
            flexcore_protocol_ports::manifest::DeclaredCapability::ExternalEventIngest
        ));
        assert!(!manifest
            .declares(flexcore_protocol_ports::manifest::DeclaredCapability::CommandRelay));
    }
}
