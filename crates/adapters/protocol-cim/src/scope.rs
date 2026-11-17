// P14.12 — The CIM/CGMES import adapter's scope declaration and manifest.
//
// CONSUMES the common protocol scope layer (P14.01,
// `crates/flexcore-protocol-ports`) — the same `ProtocolScopeDeclaration`
// and one `ProtocolAdapterManifest` shape every Phase 14 protocol adapter
// implements (decision record 01). The grid-api discipline (declared never
// inferred, mandatory non-empty `not_modelled`, fail-closed
// `assert_supports`, no `Default`) is enforced THERE; this module adds
// only the reviewed CIM CONSTRUCTORS ([`import_scope`],
// [`import_manifest`]).
//
// Role mapping onto the common vocabulary: the import is the INGEST side
// — the platform receives a profile supplied by a counterparty (a DSO
// model exchange), modelled as the common CLIENT role (the platform
// pulls/ingests; nothing listens). Control scope: MONITOR ONLY — model
// import is data, never a control path; there is no command relay and no
// external-event ingest on this surface. Resource class: MODEL-IMPORT
// (the common vocabulary's own name for CIM/CGMES and SCL/ICD surfaces).

use flexcore_protocol_ports::manifest::ProtocolAdapterManifest;
use flexcore_protocol_ports::scope::{
    ControlScope, NotModelled, ProtocolRole, ProtocolScopeDeclaration, ProtocolScopeError,
    ResourceClass,
};

/// The reviewed `not_modelled` record of the CIM/CGMES import adapter
/// (travels inside the common scope declaration; non-empty is enforced
/// there).
fn reviewed_not_modelled() -> Vec<NotModelled> {
    vec![
        NotModelled::new(
            "der_equipment_not_in_network_ir".to_string(),
            "DER equipment (PowerElectronicsConnection and its units) is EXPLICITLY_UNMAPPED \
             in the network import: the grid-api network IR is a bus-branch model with no \
             DER element; DER assets are modelled in flexcore-domain envelopes — the \
             import records them in the register, it never guesses a bus injection"
                .to_string(),
        )
        .expect("valid"),
        NotModelled::new(
            "load_aggregation_not_imported".to_string(),
            "EnergyConsumer is EXPLICITLY_UNMAPPED: the network IR has no load element in \
             this import — load aggregation is not guessed"
                .to_string(),
        )
        .expect("valid"),
        NotModelled::new(
            "switch_state_not_imported".to_string(),
            "Breaker/Disconnector state is EXPLICITLY_UNMAPPED: the bus-branch import does \
             not carry switching state"
                .to_string(),
        )
        .expect("valid"),
        NotModelled::new(
            "parser_is_a_seam".to_string(),
            "no RDF/XML parser dependency: the profile parsing is a designed, UNWIRED seam \
             (crate::rdf_profile) — no profile file has ever been read"
                .to_string(),
        )
        .expect("valid"),
        NotModelled::new(
            "cim_attribute_spellings".to_string(),
            "no CIM schema text or CGMES profile specification is in the repository: every \
             class spelling, attribute key, and unit rule is UNVERIFIED against the \
             CGMES/CIM text [S-17] (implementation-time check, decision record 44)"
                .to_string(),
        )
        .expect("valid"),
        NotModelled::new(
            "der_group_payload_structure".to_string(),
            "the IEC 61968-5 text [S-18] was NOT consulted: the DER-group payload structure \
             is a DECLARED design shape, not a specification quote (implementation-time \
             check, decision record 44)"
                .to_string(),
        )
        .expect("valid"),
    ]
}

/// The reviewed common scope declaration of the CIM/CGMES import adapter.
///
/// Role: CLIENT (the platform ingests a counterparty-supplied profile;
/// nothing listens). Control scope: MONITOR ONLY. Resource class:
/// MODEL-IMPORT.
pub fn import_scope(adapter_id: &str) -> Result<ProtocolScopeDeclaration, ProtocolScopeError> {
    ProtocolScopeDeclaration::new(
        adapter_id.to_string(),
        "cim-cgmes".to_string(),
        vec![ProtocolRole::Client],
        ControlScope::MonitorOnly,
        vec![ResourceClass::ModelImport],
        reviewed_not_modelled(),
        "model import carries no control path: no external event enters a lifecycle from \
         this surface and any future group-command surface would enter the command \
         lifecycle at PROPOSED — this adapter is NOT a dispatch authority"
            .to_string(),
        vec![
            "role mapping: profile ingest = common CLIENT role; the import is data, never \
             a control path"
                .to_string(),
            "mapping discipline: every profile object is classified EXACTLY ONE of MAPPED / \
             EXPLICITLY_UNMAPPED / UNMAPPED / AMBIGUOUS (the openems.rs pattern); an \
             ambiguity blocks the import summary — never mapped by guessing"
                .to_string(),
            "mapping targets: the grid-api network IR (VersionedNetworkModel — bus/branch/\
             transformer/external grid); mapped elements only, validated by the IR's own \
             structural validation"
                .to_string(),
        ],
    )
}

/// The reviewed common capability manifest of the CIM/CGMES import adapter
/// (the ONE manifest shape every protocol adapter implements, record 01).
/// Authority is structurally `SupervisoryRelayOnly`; the never-granted
/// permissions frozen in `crates/flexcore-kernel/src/manifest.rs` are not
/// expressible in the common manifest vocabulary.
pub fn import_manifest(
    adapter_id: &str,
) -> Result<ProtocolAdapterManifest, flexcore_protocol_ports::manifest::ManifestError> {
    ProtocolAdapterManifest::new(
        adapter_id.to_string(),
        "cim-cgmes".to_string(),
        "CIM/CGMES profiles + IEC 61968-5 DER groups (edition from [S-17][S-18]; \
         profile details UNVERIFIED)"
            .to_string(),
        import_scope(adapter_id)?,
        vec![flexcore_protocol_ports::manifest::DeclaredCapability::ModelImport],
        // The DEFAULT import cadence seed: network-model imports are
        // event-driven (a new profile version), not polled; the declared
        // non-zero cadence is the manifest contract's minimum-interval
        // statement, the reviewable per-deployment value.
        std::time::Duration::from_secs(3600),
        vec![
            "declared cadence: the DEFAULT minimum interval between import attempts \
             (3600 s); a network-model import is event-driven (a new profile version), \
             never a poll loop"
                .to_string(),
            "the coverage-limitation statements of the import register travel with every \
             imported model verbatim (the P4.02 coverage-limitation pattern)"
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
    fn import_scope_constructs_and_refuses_control_paths() {
        let scope = import_scope("cim-cgmes-import").expect("valid scope");
        assert!(scope.covers_role(ProtocolRole::Client));
        assert!(!scope.covers_role(ProtocolRole::Server));
        assert!(scope.covers_resource(ResourceClass::ModelImport));
        assert!(
            !scope.covers_relay(),
            "monitor-only: model import carries no command path"
        );
        assert_eq!(scope.coverage_limitation_statements().len(), 6);
        // Model import (the ingest surface) is in scope.
        scope
            .assert_supports(ProtocolRole::Client, false, &[ResourceClass::ModelImport])
            .expect("within scope");
        // Command relay refused at the gate.
        assert!(scope
            .assert_supports(ProtocolRole::Client, true, &[ResourceClass::ModelImport])
            .is_err());
        // Telemetry (a different surface) refused — the import is not a
        // telemetry adapter.
        assert!(scope
            .assert_supports(ProtocolRole::Client, false, &[ResourceClass::Telemetry])
            .is_err());
    }

    #[test]
    fn import_manifest_constructs_relay_free() {
        let manifest = import_manifest("cim-cgmes-import").expect("valid manifest");
        assert_eq!(manifest.protocol_id, "cim-cgmes");
        assert_eq!(
            manifest.authority,
            flexcore_protocol_ports::manifest::AuthorityDeclaration::SupervisoryRelayOnly
        );
        assert!(manifest.declares(
            flexcore_protocol_ports::manifest::DeclaredCapability::ModelImport
        ));
        assert!(!manifest
            .declares(flexcore_protocol_ports::manifest::DeclaredCapability::CommandRelay));
        assert!(!manifest.declares(
            flexcore_protocol_ports::manifest::DeclaredCapability::ExternalEventIngest
        ));
    }
}
