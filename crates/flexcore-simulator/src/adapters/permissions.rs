// P14.17 — The simulated adapters' permission binding (decision record
// 81; the wave-2 handoff, record 63 §6: "simulated adapters construct
// the same `ProtocolAdapterManifest` and pass the same
// `adapter_permissions` checks — a simulated adapter cannot gain
// authority its real counterpart lacks").
//
// This module binds the SIX protocol adapters' reviewed capability
// manifests — the SAME manifest constructors their real crates expose —
// into the kernel's adapter permission model
// (`flexcore_kernel::adapter_permissions`), proving in one place that:
//
// - every simulated counterpart constructs the manifest its real crate
//   publishes (no parallel manifest, no upgraded authority);
// - every binding passes the kernel's command-relay-only check
//   (`check_command_relay_only` — the one-variant
//   `AuthorityDeclaration::SupervisoryRelayOnly` tripwire, the
//   relay/ingest scope-consistency checks);
// - the whole simulated fleet binds into one `BoundAdapterRegistry`
//   exactly as a composition root would wire it.
//
// The manifests here come from the adapter crates' own constructors
// (`vtn_manifest`, `client_manifest`, `outstation_manifest`,
// `uplink_manifest`, `import_manifest`) and the 2030.5 reviewed shape
// (its crate constructs the manifest inline; the same fields as the
// server skeleton's tests pin). The MQTT manifest UNDER-declares (the
// publish-capability vocabulary gap, record 43 §2) — that gap is
// recorded, not papered over here: the simulated MQTT adapter passes
// the same checks with the same under-declaration as its real crate.

use std::time::Duration;

use flexcore_kernel::adapter_permissions::{AdapterBinding, BoundAdapterRegistry, AdapterPermissionError};
use flexcore_protocol_ports::manifest::{DeclaredCapability, ProtocolAdapterManifest};
use flexcore_protocol_ports::scope::{
    ControlScope, NotModelled, ProtocolRole, ProtocolScopeDeclaration, ResourceClass,
};

/// The reviewed adapter id of the IEEE 2030.5 adapter (its crate's
/// canonical constant, record 02).
pub const ADAPTER_ID_20305: &str = flexcore_protocol_20305::ADAPTER_ID;
/// The reviewed adapter id of the OpenADR VTN adapter (its crate's
/// canonical constant, P14.18).
pub const ADAPTER_ID_OPENADR: &str = flexcore_openadr_adapter::ADAPTER_ID;
/// The reviewed adapter id of the SunSpec Modbus client adapter.
pub const ADAPTER_ID_MODBUS: &str = flexcore_modbus_adapter::ADAPTER_ID;
/// The reviewed adapter id of the DNP3 outstation adapter.
pub const ADAPTER_ID_DNP3: &str = flexcore_dnp3_adapter::ADAPTER_ID;
/// The reviewed adapter id of the MQTT/Sparkplug uplink adapter.
pub const ADAPTER_ID_MQTT: &str = flexcore_mqtt_adapter::ADAPTER_ID;
/// The reviewed adapter id of the CIM/CGMES import adapter.
pub const ADAPTER_ID_CIM: &str = flexcore_cim_adapter::ADAPTER_ID;

/// The 2030.5 adapter's reviewed manifest, reconstructed with the SAME
/// scope fields its crate's server tests pin (the crate constructs its
/// manifest inline — record 02 §2 — so the reconstruction here mirrors
/// those exact fields and is pinned by test to stay aligned).
pub fn manifest_20305() -> Result<ProtocolAdapterManifest, AdapterPermissionError> {
    let scope = ProtocolScopeDeclaration::new(
        flexcore_protocol_20305::ADAPTER_ID.to_string(),
        flexcore_protocol_20305::PROTOCOL_ID.to_string(),
        vec![ProtocolRole::Server],
        ControlScope::MonitorOnly,
        vec![ResourceClass::Control, ResourceClass::Discovery],
        vec![
            NotModelled::new(
                "no_pki_operations".to_string(),
                "no CA operations — PKI ties to Phase 13/18 (security record 03)".to_string(),
            )
            .map_err(unexpected_scope)?,
            NotModelled::new(
                "no_field_write_authority".to_string(),
                "the aggregator never writes controls into a device; devices poll".to_string(),
            )
            .map_err(unexpected_scope)?,
        ],
        "external control objects enter the command lifecycle at PROPOSED; this \
         adapter is NOT a dispatch authority"
            .to_string(),
        Vec::new(),
    )
    .map_err(unexpected_scope)?;
    ProtocolAdapterManifest::new(
        flexcore_protocol_20305::ADAPTER_ID.to_string(),
        flexcore_protocol_20305::PROTOCOL_ID.to_string(),
        flexcore_protocol_20305::PROTOCOL_EDITION.to_string(),
        scope,
        vec![
            DeclaredCapability::Read,
            DeclaredCapability::Discovery,
            DeclaredCapability::ExternalEventIngest,
        ],
        Duration::from_secs(60),
        Vec::new(),
    )
    .map_err(manifest_error)
}

/// The OpenADR VTN adapter's reviewed manifest (its crate's own
/// constructor — the simulated counterpart consumes the real one).
pub fn manifest_openadr() -> Result<ProtocolAdapterManifest, AdapterPermissionError> {
    flexcore_openadr_adapter::scope::vtn_manifest(flexcore_openadr_adapter::ADAPTER_ID)
        .map_err(manifest_error)
}

/// The SunSpec Modbus client adapter's reviewed manifest (its crate's
/// own constructor).
pub fn manifest_modbus() -> Result<ProtocolAdapterManifest, AdapterPermissionError> {
    flexcore_modbus_adapter::scope::client_manifest(flexcore_modbus_adapter::ADAPTER_ID)
        .map_err(manifest_error)
}

/// The DNP3 outstation adapter's reviewed manifest (its crate's own
/// constructor).
pub fn manifest_dnp3() -> Result<ProtocolAdapterManifest, AdapterPermissionError> {
    flexcore_dnp3_adapter::scope::outstation_manifest(flexcore_dnp3_adapter::ADAPTER_ID)
        .map_err(manifest_error)
}

/// The MQTT/Sparkplug uplink adapter's reviewed manifest (its crate's
/// own constructor — INCLUDING its recorded under-declaration of the
/// publish capability, record 43 §2: the gap is consumed as-is, never
/// papered over here).
pub fn manifest_mqtt() -> Result<ProtocolAdapterManifest, AdapterPermissionError> {
    flexcore_mqtt_adapter::scope::uplink_manifest(flexcore_mqtt_adapter::ADAPTER_ID)
        .map_err(manifest_error)
}

/// The CIM/CGMES import adapter's reviewed manifest (its crate's own
/// constructor).
pub fn manifest_cim() -> Result<ProtocolAdapterManifest, AdapterPermissionError> {
    flexcore_cim_adapter::scope::import_manifest(flexcore_cim_adapter::ADAPTER_ID)
        .map_err(manifest_error)
}

/// Binds ALL SIX simulated counterparts into one kernel-side registry,
/// each through the SAME `check_command_relay_only` rule its real
/// counterpart passes. This is the composition check a wave-3
/// composition root would run before wiring the fleet.
pub fn bind_simulated_fleet() -> Result<BoundAdapterRegistry, AdapterPermissionError> {
    BoundAdapterRegistry::bind(vec![
        AdapterBinding::new(ADAPTER_ID_20305.to_string(), manifest_20305()?)?,
        AdapterBinding::new(ADAPTER_ID_OPENADR.to_string(), manifest_openadr()?)?,
        AdapterBinding::new(ADAPTER_ID_MODBUS.to_string(), manifest_modbus()?)?,
        AdapterBinding::new(ADAPTER_ID_DNP3.to_string(), manifest_dnp3()?)?,
        AdapterBinding::new(ADAPTER_ID_MQTT.to_string(), manifest_mqtt()?)?,
        AdapterBinding::new(ADAPTER_ID_CIM.to_string(), manifest_cim()?)?,
    ])
}

fn unexpected_scope(_error: flexcore_protocol_ports::scope::ProtocolScopeError) -> AdapterPermissionError {
    // The scope constructor's refusals are invariant violations here
    // (the mirrored fields are the crate's own pinned values); the
    // honest surface names the refusing surface. A valid mirrored scope
    // never reaches this arm.
    AdapterPermissionError::EmptyField("scope")
}

fn manifest_error(error: flexcore_protocol_ports::manifest::ManifestError) -> AdapterPermissionError {
    // The kernel error vocabulary has no manifest-error variant; the
    // named surface here is the refusing field. A valid adapter manifest
    // never hits any arm (the constructors are the crates' own pinned
    // ones); the mapping exists so the binding cannot panic.
    let field: &'static str = match &error {
        flexcore_protocol_ports::manifest::ManifestError::EmptyField(field) => field,
        flexcore_protocol_ports::manifest::ManifestError::EmptyCapabilities => "capabilities",
        flexcore_protocol_ports::manifest::ManifestError::ZeroCadence => "cadence",
        flexcore_protocol_ports::manifest::ManifestError::Scope(_) => "scope",
    };
    AdapterPermissionError::EmptyField(field)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_simulated_fleet_binds_through_the_kernel_checks() {
        let registry = bind_simulated_fleet().expect("all six bind");
        // Sorted adapter ids — the whole simulated fleet, no more.
        assert_eq!(
            registry.adapter_ids(),
            vec![
                ADAPTER_ID_20305,
                ADAPTER_ID_CIM,
                ADAPTER_ID_DNP3,
                ADAPTER_ID_MQTT,
                ADAPTER_ID_MODBUS,
                ADAPTER_ID_OPENADR,
            ]
        );
        // NO simulated adapter declares command relay (the command-relay
        // rule holds fleet-wide — a simulated adapter cannot gain
        // authority its real counterpart lacks).
        assert!(registry.command_relay_adapters().is_empty());
    }

    #[test]
    fn every_simulated_manifest_is_relay_only_by_structure() {
        for manifest in [
            manifest_20305().expect("2030.5 manifest"),
            manifest_openadr().expect("OpenADR manifest"),
            manifest_modbus().expect("Modbus manifest"),
            manifest_dnp3().expect("DNP3 manifest"),
            manifest_mqtt().expect("MQTT manifest"),
            manifest_cim().expect("CIM manifest"),
        ] {
            // The one-variant authority tripwire: relay-only is the only
            // constructible declaration.
            assert_eq!(
                manifest.authority,
                flexcore_protocol_ports::manifest::AuthorityDeclaration::SupervisoryRelayOnly
            );
            // The kernel's command-relay-only check passes for each.
            AdapterBinding::new(manifest.adapter_id.clone(), manifest)
                .expect("valid binding")
                .check_command_relay_only()
                .expect("the relay rule holds");
        }
    }

    #[test]
    fn the_mqtt_manifest_under_declares_as_its_real_crate_does() {
        let manifest = manifest_mqtt().expect("MQTT manifest");
        // The recorded vocabulary gap (record 43 §2): no outbound-
        // publish variant exists in the common capability set, so the
        // manifest declares Discovery + BoundedHistory and the gap stays
        // a handoff — the simulated counterpart does NOT paper over it.
        assert!(manifest.declares(DeclaredCapability::Discovery));
        assert!(manifest.declares(DeclaredCapability::BoundedHistory));
        assert!(!manifest.declares(DeclaredCapability::CommandRelay));
        assert!(!manifest.declares(DeclaredCapability::ExternalEventIngest));
    }
}
