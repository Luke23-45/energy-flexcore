// P14.07 — The SunSpec Modbus client adapter's scope declaration and
// manifest.
//
// CONSUMES the common protocol scope layer (P14.01,
// `crates/flexcore-protocol-ports`) — the same `ProtocolScopeDeclaration`
// and one `ProtocolAdapterManifest` shape every Phase 14 protocol adapter
// implements (decision record 01; this crate was reconciled onto the
// landed layer). The grid-api discipline (declared never inferred,
// mandatory non-empty `not_modelled`, fail-closed `assert_supports`, no
// `Default`) is enforced THERE; this module adds only the reviewed
// SunSpec/Modbus CONSTRUCTORS ([`client_scope`], [`client_manifest`]).
//
// The SunSpec model-family vocabulary ([`crate::model::SunSpecModel`])
// and the operation-class note below are protocol-specific elaborations
// the common vocabulary does not carry; they travel inside the common
// scope as reviewed notes and are enforced by the register-map registry
// (an unregistered model is a named refusal) — the write-operation
// refusal is structural: the manifest cannot express command relay in
// this scope and the client declares MONITOR ONLY.

use flexcore_protocol_ports::manifest::ProtocolAdapterManifest;
use flexcore_protocol_ports::scope::{
    ControlScope, NotModelled, ProtocolRole, ProtocolScopeDeclaration, ProtocolScopeError,
    ResourceClass,
};

/// The reviewed `not_modelled` record of the SunSpec Modbus client
/// adapter (travels inside the common scope declaration; non-empty is
/// enforced there).
fn reviewed_not_modelled() -> Vec<NotModelled> {
    vec![
        NotModelled::new(
            "write_operations".to_string(),
            "no write/actuate path exists: the adapter is read/monitor + command-relay only \
             (phase 14 context §3 constraint 1; absence is structural)"
                .to_string(),
        )
        .expect("valid"),
        NotModelled::new(
            "vendor_models".to_string(),
            "vendor-specific SunSpec models and non-SunSpec register maps are not modelled; \
             a device without a reviewed register map is refused"
                .to_string(),
        )
        .expect("valid"),
        NotModelled::new(
            "register_semantics".to_string(),
            "SunSpec register semantics (model ids, marker, scale factors, byte order) are \
             UNVERIFIED against the specification text [S-16] and a lab device; per-device \
             maps are lab-verified before registration"
                .to_string(),
        )
        .expect("valid"),
        NotModelled::new(
            "wire_transport".to_string(),
            "the Modbus TCP transport is implemented live under FIM-F03 (WRITTEN-UNRUN; \
             device interop UNPROVEN — a HUMAN LAB GATE); the RTU serial transport remains \
             an UNWIRED seam; no device has been reached and no message has been exchanged \
             by this program"
                .to_string(),
        )
        .expect("valid"),
    ]
}

/// The reviewed common scope declaration of the SunSpec Modbus client.
///
/// Role: CLIENT (the platform polls devices). Control scope: MONITOR
/// ONLY. Resource class: TELEMETRY (the client reads meter/inverter/
/// battery blocks into the telemetry path; it maps no control objects —
/// SunSpec write points are outside the declared scope entirely).
pub fn client_scope(adapter_id: &str) -> Result<ProtocolScopeDeclaration, ProtocolScopeError> {
    ProtocolScopeDeclaration::new(
        adapter_id.to_string(),
        "sunspec-modbus".to_string(),
        vec![ProtocolRole::Client],
        ControlScope::MonitorOnly,
        vec![ResourceClass::Telemetry],
        reviewed_not_modelled(),
        "readings from SunSpec devices enter the telemetry path only; no external control \
         object is mapped and no lifecycle state is constructed by this adapter — it is NOT \
         a dispatch authority (external events would enter the command lifecycle at PROPOSED, \
         and none are ingested in this scope)"
            .to_string(),
        vec![
            "operation class: READ only (the write direction is structural absence, not a \
             config flag)"
                .to_string(),
            "declared families travel in the register-map registry (crate::registry): \
             101/103 inverters, 201/203 meters, 80x battery models — UNVERIFIED ids per [S-16]"
                .to_string(),
        ],
    )
}

/// The reviewed common capability manifest of the SunSpec Modbus client
/// (the ONE manifest shape every protocol adapter implements, record 01).
/// Authority is structurally `SupervisoryRelayOnly`; the never-granted
/// permissions frozen in `crates/flexcore-kernel/src/manifest.rs` are not
/// expressible in the common manifest vocabulary.
pub fn client_manifest(
    adapter_id: &str,
) -> Result<ProtocolAdapterManifest, flexcore_protocol_ports::manifest::ManifestError> {
    ProtocolAdapterManifest::new(
        adapter_id.to_string(),
        "sunspec-modbus".to_string(),
        "SunSpec Modbus (information models; edition pinned at implementation per [S-16])"
            .to_string(),
        client_scope(adapter_id)?,
        vec![
            flexcore_protocol_ports::manifest::DeclaredCapability::Read,
            flexcore_protocol_ports::manifest::DeclaredCapability::Discovery,
        ],
        // The DEFAULT cadence seed; the per-gateway declared polling
        // budget (crate::polling) is the reviewable per-deployment value.
        std::time::Duration::from_secs(60),
        vec![
            "declared cadence: the DEFAULT scan-interval seed (60 s); the per-gateway budget \
             is declared, never defaulted"
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
    fn client_scope_constructs_and_refuses_out_of_scope_requests() {
        let scope = client_scope("modbus-client").expect("valid scope");
        assert!(scope.covers_role(ProtocolRole::Client));
        assert!(!scope.covers_role(ProtocolRole::Server));
        assert!(scope.covers_resource(ResourceClass::Telemetry));
        assert!(!scope.covers_resource(ResourceClass::Control));
        assert!(!scope.covers_relay(), "monitor-only: no command relay");
        assert_eq!(scope.coverage_limitation_statements().len(), 4);
        // Telemetry reads are in scope; a command-relay request and a
        // control mapping are refused (the write direction is structural).
        scope
            .assert_supports(ProtocolRole::Client, false, &[ResourceClass::Telemetry])
            .expect("within scope");
        assert!(scope
            .assert_supports(ProtocolRole::Client, true, &[ResourceClass::Telemetry])
            .is_err());
        assert!(scope
            .assert_supports(ProtocolRole::Client, false, &[ResourceClass::Control])
            .is_err());
    }

    #[test]
    fn client_manifest_constructs_relay_free() {
        let manifest = client_manifest("modbus-client").expect("valid manifest");
        assert_eq!(manifest.protocol_id, "sunspec-modbus");
        assert_eq!(
            manifest.authority,
            flexcore_protocol_ports::manifest::AuthorityDeclaration::SupervisoryRelayOnly
        );
        assert!(manifest.declares(flexcore_protocol_ports::manifest::DeclaredCapability::Read));
        assert!(!manifest
            .declares(flexcore_protocol_ports::manifest::DeclaredCapability::CommandRelay));
        assert!(!manifest.declares(
            flexcore_protocol_ports::manifest::DeclaredCapability::ExternalEventIngest
        ));
    }
}
