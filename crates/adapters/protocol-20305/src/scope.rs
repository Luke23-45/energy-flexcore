// P14.02 — The IEEE 2030.5 AggregatorServer adapter's scope declaration
// and capability manifest constructors.
//
// CONSUMES the common protocol scope layer (P14.01,
// `crates/flexcore-protocol-ports`) — the same `ProtocolScopeDeclaration`
// every Phase 14 protocol adapter implements (decision record 01). The
// grid-api discipline (declared never inferred, mandatory non-empty
// `not_modelled`, fail-closed `assert_supports`, no `Default`) is enforced
// THERE, in one reviewed place; this module adds only the reviewed 2030.5
// scope/manifest CONSTRUCTORS ([`aggregator_scope`], [`aggregator_manifest`])
// that instantiate the common types for this adapter.
//
// FRV1-R6-001 aside: this module carries the reviewed spelling so the
// composition root constructs the scope/manifest through it instead of
// hand-assembling the eight-argument `ProtocolScopeDeclaration::new` (the
// former spelling lived only in test helpers; FRV1-R6-004, final review
// R6). The test helpers delegate here — one reviewed spelling, no drift.

use flexcore_protocol_ports::manifest::ProtocolAdapterManifest;
use flexcore_protocol_ports::scope::{
    ControlScope, NotModelled, ProtocolRole, ProtocolScopeDeclaration, ResourceClass,
};

/// The reviewed `not_modelled` record of the IEEE 2030.5 AggregatorServer
/// adapter (the limitation list travels inside the common scope
/// declaration; non-empty is enforced there).
fn reviewed_not_modelled() -> Vec<NotModelled> {
    vec![
        NotModelled::new(
            "no_pki_operations".to_string(),
            "no CA operations — PKI ties to Phase 13/18 (security record 03)".to_string(),
        )
        .expect("valid"),
        NotModelled::new(
            "no_field_write_authority".to_string(),
            "the aggregator never writes controls into a device; devices poll".to_string(),
        )
        .expect("valid"),
    ]
}

/// The reviewed common scope declaration of the IEEE 2030.5
/// AggregatorServer adapter.
///
/// Role: SERVER (the aggregator is the passive end devices poll).
/// Control scope: MONITOR ONLY — the adapter maps external DERControls
/// into PROPOSED candidates and never relays commands in this phase.
/// Resource classes: Control (DERControls are control shapes entering at
/// PROPOSED) and Discovery (device/FunctionSetAssignments discovery).
pub fn aggregator_scope(adapter_id: &str) -> Result<ProtocolScopeDeclaration, ProtocolScopeError> {
    ProtocolScopeDeclaration::new(
        adapter_id.to_string(),
        crate::PROTOCOL_ID.to_string(),
        vec![ProtocolRole::Server],
        ControlScope::MonitorOnly,
        vec![ResourceClass::Control, ResourceClass::Discovery],
        reviewed_not_modelled(),
        "external control objects enter the command lifecycle at PROPOSED; this \
         adapter is NOT a dispatch authority"
            .to_string(),
        Vec::new(),
    )
}

/// The reviewed common capability manifest of the IEEE 2030.5
/// AggregatorServer adapter: the ONE manifest shape every protocol adapter
/// implements (record 01). The authority declaration is structurally
/// `SupervisoryRelayOnly` — the never-granted permissions
/// (`dispatch.authorize`, `dispatch.issue`, `secret.read`, `network.any`,
/// frozen in `crates/flexcore-kernel/src/manifest.rs`) are not expressible
/// in the common manifest vocabulary at all.
pub fn aggregator_manifest(
    adapter_id: &str,
) -> Result<ProtocolAdapterManifest, flexcore_protocol_ports::manifest::ManifestError> {
    let cadence = std::time::Duration::from_secs(60);
    ProtocolAdapterManifest::new(
        adapter_id.to_string(),
        crate::PROTOCOL_ID.to_string(),
        crate::PROTOCOL_EDITION.to_string(),
        aggregator_scope(adapter_id)?,
        vec![
            flexcore_protocol_ports::manifest::DeclaredCapability::Read,
            flexcore_protocol_ports::manifest::DeclaredCapability::Discovery,
            flexcore_protocol_ports::manifest::DeclaredCapability::ExternalEventIngest,
        ],
        cadence,
        Vec::new(),
    )
}

// Re-export for the crate's error vocabulary (the common errors ARE the
// scope errors of this adapter — one reviewed vocabulary, no parallel one).
pub use flexcore_protocol_ports::scope::ProtocolScopeError;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aggregator_scope_constructs_and_refuses_out_of_scope_requests() {
        let scope = aggregator_scope(crate::ADAPTER_ID).expect("valid scope");
        assert!(scope.covers_role(ProtocolRole::Server));
        assert!(!scope.covers_role(ProtocolRole::Client));
        assert!(scope.covers_resource(ResourceClass::Control));
        assert!(!scope.covers_resource(ResourceClass::Telemetry));
        assert!(
            !scope.covers_relay(),
            "monitor-only: no command relay in this phase"
        );
        // The not_modelled record is non-empty and travels inside the
        // common declaration (one reviewed spelling — the test helpers
        // delegate here).
        assert_eq!(scope.coverage_limitation_statements().len(), 2);
        scope
            .assert_supports(ProtocolRole::Server, false, &[ResourceClass::Control])
            .expect("within scope");
        assert!(scope
            .assert_supports(ProtocolRole::Client, false, &[ResourceClass::Control])
            .is_err());
    }

    #[test]
    fn aggregator_manifest_constructs_with_relay_free_authority() {
        let manifest = aggregator_manifest(crate::ADAPTER_ID).expect("valid manifest");
        assert_eq!(manifest.adapter_id, crate::ADAPTER_ID);
        assert_eq!(manifest.protocol_id, crate::PROTOCOL_ID);
        assert_eq!(manifest.protocol_edition, crate::PROTOCOL_EDITION);
        assert_eq!(
            manifest.authority,
            flexcore_protocol_ports::manifest::AuthorityDeclaration::SupervisoryRelayOnly
        );
        assert!(manifest.declares(flexcore_protocol_ports::manifest::DeclaredCapability::Read));
        assert!(manifest
            .declares(flexcore_protocol_ports::manifest::DeclaredCapability::ExternalEventIngest));
        assert!(!manifest.declares(flexcore_protocol_ports::manifest::DeclaredCapability::CommandRelay),
            "monitor-only scope: the manifest does not declare command relay");
    }
}
