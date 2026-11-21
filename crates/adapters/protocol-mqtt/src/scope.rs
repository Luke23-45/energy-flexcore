// P14.11 — The MQTT/Sparkplug B adapter's scope declaration and manifest.
//
// CONSUMES the common protocol scope layer (P14.01,
// `crates/flexcore-protocol-ports`) — the same `ProtocolScopeDeclaration`
// and one `ProtocolAdapterManifest` shape every Phase 14 protocol adapter
// implements (decision record 01). The grid-api discipline (declared never
// inferred, mandatory non-empty `not_modelled`, fail-closed
// `assert_supports`, no `Default`) is enforced THERE; this module adds
// only the reviewed MQTT CONSTRUCTORS ([`uplink_scope`],
// [`uplink_manifest`]).
//
// Role mapping onto the common vocabulary: the uplink adapter is the
// ACTIVE end — it connects OUT to the broker and publishes — the common
// CLIENT role. Control scope: MONITOR ONLY (under FIM-F05, written-unrun,
// a command-TOPIC DELIVERY surface delivers received commands into the
// edge command lifecycle at PROPOSED; there is still no command relay, no
// dispatch authority, nothing that carries a setpoint through this
// adapter — the release ladder keeps v0.x adapters read/monitor-first).
//
// CAPABILITY VOCABULARY NOTE (recorded honestly, decision record 43 §4):
// the common `DeclaredCapability` vocabulary (record 01) expresses
// READ-side capabilities — it has no variant for an OUTBOUND telemetry
// publisher. This manifest therefore declares the two capabilities the
// vocabulary DOES support honestly — `Discovery` (the Sparkplug BIRTH
// inventory declaration, named in the common vocabulary's own
// documentation) and `BoundedHistory` (the offline store-and-forward
// re-publish semantics, the P6.09 pattern) — and records the gap in the
// manifest notes. Extending the common vocabulary with a Publish/Telemetry
// variant is a REVIEWED common-layer change (record 01 is the owning
// record), not a drive-by edit; it is a handoff item to wave 3 / the
// human. Until then the manifest UNDER-declares rather than mis-declares.

use flexcore_protocol_ports::manifest::ProtocolAdapterManifest;
use flexcore_protocol_ports::scope::{
    ControlScope, NotModelled, ProtocolRole, ProtocolScopeDeclaration, ProtocolScopeError,
    ResourceClass,
};

/// The reviewed `not_modelled` record of the MQTT/Sparkplug uplink adapter
/// (travels inside the common scope declaration; non-empty is enforced
/// there).
fn reviewed_not_modelled() -> Vec<NotModelled> {
    vec![
        NotModelled::new(
            "no_command_path".to_string(),
            "no command RELAY, no dispatch authority: under FIM-F05 (written-unrun) a \
             command-TOPIC DELIVERY surface exists (live.rs subscribes the declared exact \
             command topic and drains received commands into the edge command lifecycle \
             at PROPOSED and nowhere else), but nothing here authorizes, dispatches, or \
             relays a command — the monitor-only control scope and the common relay gate \
             still stand"
                .to_string(),
        )
        .expect("valid"),
        NotModelled::new(
            "wire_transport".to_string(),
            "FIM-F05 (written-unrun): the wire transport EXISTS as hand-written codecs \
             (mqtt_wire.rs, sparkplug_wire.rs — no MQTT client crate, no prost) and the \
             live transport (live.rs, TLS via tokio-rustls), but it has never been \
             compiled or executed: no broker has ever been reached, and interop is \
             UNPROVEN — a HUMAN LAB GATE"
                .to_string(),
        )
        .expect("valid"),
        NotModelled::new(
            "specification_text".to_string(),
            "no Sparkplug B specification text is in the repository: every topic spelling, \
             message-kind string, sequence semantic, and payload field is UNVERIFIED \
             against the text (implementation-time check, decision record 43)"
                .to_string(),
        )
        .expect("valid"),
        NotModelled::new(
            "secret_material".to_string(),
            "no secret material of any kind: the binding type has no secret field and the \
             transport seam receives a credential HANDLE from the deployment's key source \
             (the never-granted secret.read rule, conventions §6.4)"
                .to_string(),
        )
        .expect("valid"),
        NotModelled::new(
            "no_nats_at_the_edge".to_string(),
            "OpenFMB's NATS-at-the-grid-edge evidence [S-40] is NOTED, not productized: \
             the designed edge transport is MQTT 5 + Sparkplug B, and the existing \
             crates/adapters/nats crate stays the platform-internal bus"
                .to_string(),
        )
        .expect("valid"),
    ]
}

/// The reviewed common scope declaration of the MQTT/Sparkplug uplink
/// adapter.
///
/// Role: CLIENT (the adapter connects OUT to the broker and publishes —
/// nothing listens). Control scope: MONITOR ONLY. Resource classes:
/// TELEMETRY (the canonical uplink points published as Sparkplug metrics)
/// and DISCOVERY (the BIRTH inventory declaration).
pub fn uplink_scope(adapter_id: &str) -> Result<ProtocolScopeDeclaration, ProtocolScopeError> {
    ProtocolScopeDeclaration::new(
        adapter_id.to_string(),
        "mqtt5-sparkplug-b".to_string(),
        vec![ProtocolRole::Client],
        ControlScope::MonitorOnly,
        vec![ResourceClass::Telemetry, ResourceClass::Discovery],
        reviewed_not_modelled(),
        "this adapter publishes platform-originated telemetry outbound; it ingests no \
         external control objects, and any future external event surface would enter the \
         command lifecycle at PROPOSED — this adapter is NOT a dispatch authority"
            .to_string(),
        vec![
            "role mapping: uplink publisher = common CLIENT role (edge-initiated; nothing \
             listens — the P6.07 structural rule)"
                .to_string(),
            "capability vocabulary note: the common DeclaredCapability set (record 01) has \
             no outbound-publish variant; this manifest declares Discovery + BoundedHistory \
             honestly and records the gap as a reviewed common-layer handoff item"
                .to_string(),
            "canonical unit rule: Sparkplug metrics carry no unit field (UNVERIFIED), so \
             the canonical unit travels in the DECLARED metric name and a point without a \
             unit declaration is refused, never defaulted"
                .to_string(),
        ],
    )
}

/// The reviewed common capability manifest of the MQTT/Sparkplug uplink
/// adapter (the ONE manifest shape every protocol adapter implements,
/// record 01). Authority is structurally `SupervisoryRelayOnly`; the
/// never-granted permissions frozen in `crates/flexcore-kernel/src/
/// manifest.rs` are not expressible in the common manifest vocabulary.
pub fn uplink_manifest(
    adapter_id: &str,
) -> Result<ProtocolAdapterManifest, flexcore_protocol_ports::manifest::ManifestError> {
    ProtocolAdapterManifest::new(
        adapter_id.to_string(),
        "mqtt5-sparkplug-b".to_string(),
        "MQTT 5 + Sparkplug B (edition from [S-39][S-40]; all wire details UNVERIFIED)"
            .to_string(),
        uplink_scope(adapter_id)?,
        vec![
            flexcore_protocol_ports::manifest::DeclaredCapability::Discovery,
            flexcore_protocol_ports::manifest::DeclaredCapability::BoundedHistory,
        ],
        // The DEFAULT uplink cadence seed: the edge forwards on the
        // runner's poll cadence; the declared per-deployment cadence is
        // the reviewable value (the P14.20 budget seam).
        std::time::Duration::from_secs(60),
        vec![
            "declared cadence: the DEFAULT uplink cadence seed (60 s); the deployment's \
             telemetry budget governs the real value"
                .to_string(),
            "vocabulary gap recorded: no common DeclaredCapability variant expresses an \
             outbound telemetry publisher; Discovery + BoundedHistory are declared, the \
             Publish variant is a reviewed common-layer change (handoff, wave 3)"
                .to_string(),
            "certification is a human-gated process; this manifest is a declaration, never \
             a conformance claim (conventions §7.3)"
                .to_string(),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uplink_scope_constructs_and_refuses_command_paths() {
        let scope = uplink_scope("mqtt-sparkplug-uplink").expect("valid scope");
        assert!(scope.covers_role(ProtocolRole::Client));
        assert!(!scope.covers_role(ProtocolRole::Server));
        assert!(scope.covers_resource(ResourceClass::Telemetry));
        assert!(scope.covers_resource(ResourceClass::Discovery));
        assert!(
            !scope.covers_relay(),
            "monitor-only: no command relay in this batch"
        );
        assert_eq!(scope.coverage_limitation_statements().len(), 5);
        // Publishing telemetry (outbound) is the declared surface.
        scope
            .assert_supports(ProtocolRole::Client, false, &[ResourceClass::Telemetry])
            .expect("within scope");
        // A command-relay request is refused at the gate.
        assert!(scope
            .assert_supports(ProtocolRole::Client, true, &[ResourceClass::Telemetry])
            .is_err());
    }

    #[test]
    fn uplink_manifest_constructs_relay_free() {
        let manifest = uplink_manifest("mqtt-sparkplug-uplink").expect("valid manifest");
        assert_eq!(manifest.protocol_id, "mqtt5-sparkplug-b");
        assert_eq!(
            manifest.authority,
            flexcore_protocol_ports::manifest::AuthorityDeclaration::SupervisoryRelayOnly
        );
        assert!(manifest.declares(
            flexcore_protocol_ports::manifest::DeclaredCapability::Discovery
        ));
        assert!(manifest.declares(
            flexcore_protocol_ports::manifest::DeclaredCapability::BoundedHistory
        ));
        assert!(!manifest
            .declares(flexcore_protocol_ports::manifest::DeclaredCapability::CommandRelay));
        assert!(!manifest.declares(
            flexcore_protocol_ports::manifest::DeclaredCapability::ExternalEventIngest
        ));
    }
}
