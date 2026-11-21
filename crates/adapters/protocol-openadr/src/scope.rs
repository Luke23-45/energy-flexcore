// P14.05 — The OpenADR VTN adapter's scope declaration and delivery
// modes.
//
// CONSUMES the common protocol scope layer (P14.01,
// `crates/flexcore-protocol-ports`) — the same `ProtocolScopeDeclaration`
// every Phase 14 protocol adapter implements (decision record 01; Agent A
// landed the layer mid-wave and this crate was reconciled onto it). The
// grid-api discipline (declared never inferred, mandatory non-empty
// `not_modelled`, fail-closed `assert_supports`, no `Default`) is enforced
// THERE, in one reviewed place; this module adds only what the common
// vocabulary cannot express:
//
// - [`DeliveryMode`]: the OpenADR-specific polling/push distinction, and
// - the reviewed OpenADR scope/manifest CONSTRUCTORS
//   ([`vtn_scope`], [`vtn_manifest`]) that instantiate the common types
//   for this adapter.

use serde::{Deserialize, Serialize};

use flexcore_protocol_ports::manifest::ProtocolAdapterManifest;
use flexcore_protocol_ports::scope::{
    ControlScope, NotModelled, ProtocolRole, ProtocolScopeDeclaration, ResourceClass,
};

/// The declared event delivery mechanism(s) (phase 14 plan §2 P14.05:
/// "polling + push delivery"). Both modes are declared explicitly when
/// supported; nothing is inferred.
///
/// This is protocol-specific vocabulary the common layer does not carry
/// (the common layer names roles, control direction, and resource
/// classes); it travels INSIDE the common scope as reviewed notes and is
/// enforced by the delivery planner (`crate::vtn`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryMode {
    /// VEN-initiated polling at the agreed poll frequency [S-10]
    /// (UNVERIFIED: poll endpoint semantics against the 3.0 text).
    Polling,
    /// VTN-initiated push notification to a VEN-registered endpoint [S-10]
    /// (UNVERIFIED: push endpoint and payload semantics against the 3.0
    /// text).
    Push,
}

impl DeliveryMode {
    /// Canonical name for records and error messages.
    pub const fn name(self) -> &'static str {
        match self {
            DeliveryMode::Polling => "polling",
            DeliveryMode::Push => "push",
        }
    }
}

/// The reviewed `not_modelled` record of the OpenADR VTN adapter (the
/// limitation list travels inside the common scope declaration; non-empty
/// is enforced there).
fn reviewed_not_modelled() -> Vec<NotModelled> {
    vec![
        NotModelled::new(
            "openadr_2_0b".to_string(),
            "the legacy 2.0b XML/SOAP profile is not modelled by this scope (P14.06 \
             assessment: DEFER; P14-E-2 OPEN)"
                .to_string(),
        )
        .expect("valid"),
        NotModelled::new(
            "ven_client_role".to_string(),
            "the VEN (client) role is not modelled; a simulated VEN is P14.17's deliverable"
                .to_string(),
        )
        .expect("valid"),
        NotModelled::new(
            "http_serving".to_string(),
            "the live VTN HTTP serving added under FIM-F04 is WRITTEN-UNRUN (VEN interop \
             UNPROVEN — a HUMAN LAB GATE; record 21 §3's designed seam holds: the serving \
             layer reads the endpoint design as data)"
                .to_string(),
        )
        .expect("valid"),
        NotModelled::new(
            "endpoint_paths".to_string(),
            "OpenADR 3.0 REST/JSON endpoint paths and payload field names are UNVERIFIED \
             secondary-source readings pending the specification text [S-10]"
                .to_string(),
        )
        .expect("valid"),
    ]
}

/// The reviewed common scope declaration of the OpenADR VTN adapter.
///
/// Role: SERVER (the VTN is the passive end VENs connect to and poll).
/// Control scope: MONITOR ONLY — the adapter maps external events into
/// PROPOSED candidates and never relays commands in this phase. Resource
/// classes: Control (OpenADR events are control shapes entering at
/// PROPOSED), Telemetry (report payloads are telemetry records), and
/// Discovery (party registration / VEN enrolment).
pub fn vtn_scope(adapter_id: &str) -> Result<ProtocolScopeDeclaration, ProtocolScopeError> {
    ProtocolScopeDeclaration::new(
        adapter_id.to_string(),
        "openadr-3.0".to_string(),
        vec![ProtocolRole::Server],
        ControlScope::MonitorOnly,
        vec![ResourceClass::Control, ResourceClass::Telemetry, ResourceClass::Discovery],
        reviewed_not_modelled(),
        "external OpenADR events enter the command lifecycle at PROPOSED and pass the \
         validation, feasibility, and authorization gates like any other candidate; this \
         adapter is NOT a dispatch authority"
            .to_string(),
        vec![
            "delivery modes declared: polling and push (protocol-specific vocabulary; \
             enforced by the delivery planner)"
                .to_string(),
            "endpoint surface designed as data (crate::endpoints); serving is wired onto \
             that seam under FIM-F04 (written-unrun, VEN interop unproven — a human lab \
             gate)"
                .to_string(),
        ],
    )
}

/// The reviewed common capability manifest of the OpenADR VTN adapter:
/// the ONE manifest shape every protocol adapter implements (record 01).
/// The authority declaration is structurally `SupervisoryRelayOnly` — the
/// never-granted permissions (`dispatch.authorize`, `dispatch.issue`,
/// `secret.read`, `network.any`, frozen in
/// `crates/flexcore-kernel/src/manifest.rs`) are not expressible in the
/// common manifest vocabulary at all.
pub fn vtn_manifest(
    adapter_id: &str,
) -> Result<ProtocolAdapterManifest, flexcore_protocol_ports::manifest::ManifestError> {
    let cadence = std::time::Duration::from_secs(60);
    ProtocolAdapterManifest::new(
        adapter_id.to_string(),
        "openadr-3.0".to_string(),
        "OpenADR 3.0".to_string(),
        vtn_scope(adapter_id)?,
        vec![
            flexcore_protocol_ports::manifest::DeclaredCapability::Read,
            flexcore_protocol_ports::manifest::DeclaredCapability::Discovery,
            flexcore_protocol_ports::manifest::DeclaredCapability::ExternalEventIngest,
        ],
        cadence,
        vec![
            "declared cadence: the DEFAULT VEN poll frequency seed (60 s); the per-VEN \
             agreed frequency is the registry's reviewable value"
                .to_string(),
            "certification is a human-gated lab process; this manifest is a declaration, \
             never a conformance claim (conventions §7.3)"
                .to_string(),
        ],
    )
}

// Re-export for the crate's error vocabulary (the common errors ARE the
// scope errors of this adapter — one reviewed vocabulary, no parallel one).
pub use flexcore_protocol_ports::scope::ProtocolScopeError;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vtn_scope_constructs_and_refuses_out_of_scope_requests() {
        let scope = vtn_scope("openadr-vtn").expect("valid scope");
        assert!(scope.covers_role(ProtocolRole::Server));
        assert!(!scope.covers_role(ProtocolRole::Client), "the VEN role is not declared");
        assert!(scope.covers_resource(ResourceClass::Control));
        assert!(!scope.covers_resource(ResourceClass::ModelImport));
        assert!(!scope.covers_relay(), "monitor-only: no command relay in this phase");
        assert_eq!(scope.coverage_limitation_statements().len(), 4);
        // Event mapping (Control) is in scope; model import is not.
        scope
            .assert_supports(ProtocolRole::Server, false, &[ResourceClass::Control])
            .expect("within scope");
        assert!(scope
            .assert_supports(ProtocolRole::Client, false, &[ResourceClass::Control])
            .is_err());
    }

    #[test]
    fn authority_statement_enforcement_is_structural() {
        // A scope whose authority statement does not name the PROPOSED
        // entry and the not-a-dispatch-authority rule is refused by the
        // common constructor with the named variant.
        assert_eq!(
            ProtocolScopeDeclaration::new(
                "openadr-vtn".to_string(),
                "openadr-3.0".to_string(),
                vec![ProtocolRole::Server],
                ControlScope::MonitorOnly,
                vec![ResourceClass::Control],
                reviewed_not_modelled(),
                "the adapter decides everything itself".to_string(),
                Vec::new(),
            ),
            Err(ProtocolScopeError::InvalidAuthorityStatement)
        );
    }

    #[test]
    fn vtn_manifest_constructs_with_relay_free_authority() {
        let manifest = vtn_manifest("openadr-vtn").expect("valid manifest");
        assert_eq!(manifest.protocol_id, "openadr-3.0");
        assert_eq!(
            manifest.authority,
            flexcore_protocol_ports::manifest::AuthorityDeclaration::SupervisoryRelayOnly
        );
        assert!(manifest.declares(flexcore_protocol_ports::manifest::DeclaredCapability::ExternalEventIngest));
        assert!(!manifest.declares(flexcore_protocol_ports::manifest::DeclaredCapability::CommandRelay),
            "monitor-only scope: the manifest does not declare command relay");
        assert_eq!(manifest.coverage_limitation_statements().len(), 4);
    }
}
