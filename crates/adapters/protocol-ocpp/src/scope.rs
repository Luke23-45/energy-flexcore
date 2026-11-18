// P17.01 — The OCPP CSMS adapter's scope declaration and capability
// manifest. CONSUMES the common protocol scope layer (P14.01,
// `crates/flexcore-protocol-ports`) — the same `ProtocolScopeDeclaration`
// and one `ProtocolAdapterManifest` shape every Phase 14/17 protocol
// adapter implements. The grid-api discipline (declared never inferred,
// mandatory non-empty `not_modelled`, fail-closed `assert_supports`, no
// `Default`) is enforced THERE; this module adds only the reviewed CSMS
// CONSTRUCTORS ([`csms_scope`], [`csms_manifest`]) — the phase-17 plan §7.3
// rule ("the CSMS adapter's scope declaration mirrors
// `crates/flexcore-grid-api/src/scope.rs` exactly").

use flexcore_protocol_ports::manifest::{
    DeclaredCapability, ManifestError, ProtocolAdapterManifest,
};
use flexcore_protocol_ports::scope::{
    ControlScope, NotModelled, ProtocolRole, ProtocolScopeDeclaration, ProtocolScopeError,
    ResourceClass,
};

/// The reviewed `not_modelled` record of the OCPP CSMS adapter (travels
/// inside the common scope declaration; non-empty is enforced there). Every
/// entry is the reviewed limitation record; surfaced verbatim wherever the
/// adapter's outputs are consumed.
fn reviewed_not_modelled() -> Vec<NotModelled> {
    vec![
        NotModelled::new(
            "no_ocpp_1_6".to_string(),
            "no OCPP 1.6 support: 1.6 is NOT backward compatible with 2.0.1 and P17-E-1's \
             DEFAULT is the 2.0.1 baseline + 2.1 additive shape; adding 1.6 would be a dual \
             stack (option c) and is a reviewed scope change, not an extension"
                .to_string(),
        )
        .expect("valid"),
        NotModelled::new(
            "no_pki_ca_operations".to_string(),
            "no CA is stood up anywhere in this program (P14.03 interplay): certificate \
             MATERIAL is never held here — Profile 3 onboarding carries certificate LABELS \
             and flow records only, and issuance belongs to the deployment's PKI operations \
             (record 03_p17_03_security_profile_3_onboarding.md)"
                .to_string(),
        )
        .expect("valid"),
        NotModelled::new(
            "no_iso_15118_pnc_pki".to_string(),
            "no ISO 15118 Plug&Charge PKI implementation: PnC certificate management is a \
             PHASE-GATED design (P17.12, record owned by wave 2); the V2G Root CA trust \
             store is a design record, never material"
                .to_string(),
        )
        .expect("valid"),
        NotModelled::new(
            "no_bidirectional_v2g_control".to_string(),
            "no bidirectional/V2G control in this scope: OCPP 2.1 bidirectional/DER blocks \
             are design records only (P17.13); V2X stays gated by the domain five-condition \
             V2xConfig, and the North-American AC path (2030.5 V2G-AC / J3072 / UL 1741 SC) \
             is a SEPARATE future integration"
                .to_string(),
        )
        .expect("valid"),
        NotModelled::new(
            "wire_transport".to_string(),
            "no WebSocket library, no TLS stack, no HTTP framework: the OCPP-J WebSocket \
             transport is a designed, UNRUN seam (crate::csms::CsmsTransport); no server has \
             ever run and no charge point has ever connected"
                .to_string(),
        )
        .expect("valid"),
        NotModelled::new(
            "no_protection_grade_control".to_string(),
            "supervisory orchestration only (foundations §0.3): no primary frequency \
             response, no synthetic inertia, no relay protection, no inverter inner-loop \
             control, no sub-cycle control; charging control never overrides vehicle/BMS \
             protection limits (IEC 61851 device-level truths [S-142])"
                .to_string(),
        )
        .expect("valid"),
        NotModelled::new(
            "no_conformance_claim".to_string(),
            "no OCPP conformance/certification claim of any kind: OCA/DEKRA-class \
             conformance is a human-gated lab process (conventions §7.3); the written-unrun \
             test suites are evidence preparation, never results"
                .to_string(),
        )
        .expect("valid"),
    ]
}

/// The reviewed common scope declaration of the OCPP CSMS adapter.
///
/// Role: SERVER (the CSMS is the server role — charge points connect TO
/// FlexCore, never the reverse; phase-17 context §3 constraint 2 [S-121]).
/// Control scope: COMMAND RELAY — NOT dispatch authority: the adapter
/// carries already-gate-approved lifecycle output (e.g. relayed
/// SetChargingProfile payloads) TO a charge point and reports transport
/// outcomes; every external control object (a station-reported
/// NotifyChargingLimit-class input, a vehicle charging proposal) enters the
/// command lifecycle at PROPOSED. Resource classes: TELEMETRY
/// (MeterValues/StatusNotification/TransactionEvent readings), CONTROL
/// (smart-charging profiles as relayed lifecycle output), DISCOVERY
/// (BootNotification provisioning), and EVENT-LOG
/// (SecurityEventNotification ingestion).
pub fn csms_scope() -> Result<ProtocolScopeDeclaration, ProtocolScopeError> {
    ProtocolScopeDeclaration::new(
        crate::ADAPTER_ID.to_string(),
        crate::PROTOCOL_ID.to_string(),
        vec![ProtocolRole::Server],
        ControlScope::CommandRelay,
        vec![
            ResourceClass::Telemetry,
            ResourceClass::Control,
            ResourceClass::Discovery,
            ResourceClass::EventLog,
        ],
        reviewed_not_modelled(),
        "external control objects enter the command lifecycle at PROPOSED and pass the \
         validation, feasibility, and authorization gates; this adapter relays only \
         already-approved ISSUED lifecycle output to charge points and is NOT a dispatch \
         authority"
            .to_string(),
        vec![
            "subprotocol baseline: ocpp2.0.1 with ocpp2.1 additive (P17-E-1 DEFAULT, OPEN; \
             1.6 excluded — not_modelled)"
                .to_string(),
            "both-direction initiation modelled: the CSMS originates requests (e.g. \
             SetChargingProfile, certificate flows) as well as answering charge-point \
             CALLs [S-121][S-124]"
                .to_string(),
            "hosting: edge-agent-hosted for depot sites is the P17-E-2 DEFAULT (OPEN) — \
             see crate::hosting; the composition registry decides per site"
                .to_string(),
            "every protocol detail is UNVERIFIED against the OCA text until the \
             implementation-time check (phase-17 plan §7.5)"
                .to_string(),
        ],
    )
}

/// The reviewed common capability manifest of the OCPP CSMS adapter.
///
/// Declares: `Read` (charge-point-originated payloads are read/ingested),
/// `Discovery` (BootNotification provisioning), `ExternalEventIngest`
/// (external control objects enter at PROPOSED), `CommandRelay` (relayed
/// ISSUED output to charge points), and `BoundedHistory` (the
/// store-and-forward buffer reuse the edge-agent-hosted depot shape depends
/// on, P17-E-2 DEFAULT). The authority declaration is the one-variant
/// `SupervisoryRelayOnly` — a manifest that claims dispatch authority is
/// not expressible in the common type (P14.01).
pub fn csms_manifest() -> Result<ProtocolAdapterManifest, ManifestError> {
    let scope = csms_scope()?;
    let manifest = ProtocolAdapterManifest::new(
        crate::ADAPTER_ID.to_string(),
        crate::PROTOCOL_ID.to_string(),
        crate::PROTOCOL_EDITION.to_string(),
        scope,
        vec![
            DeclaredCapability::Read,
            DeclaredCapability::Discovery,
            DeclaredCapability::ExternalEventIngest,
            DeclaredCapability::CommandRelay,
            DeclaredCapability::BoundedHistory,
        ],
        std::time::Duration::from_secs(60),
        vec![
            "declared cadence: the supervisory minutes-scale default; OCPP heartbeat and \
             MeterValues cadences are per-connection DECLARED configuration, never inferred"
                .to_string(),
            "security: Profile 3 is the onboarding target (record \
             03_p17_03_security_profile_3_onboarding.md); Profiles 1/2 are recorded, not \
             accepted for new onboarding"
                .to_string(),
        ],
    )?;
    // The authority field is set inside `new` to the one-variant value; the
    // assertion documents the invariant for reviewers (the type cannot carry
    // another variant).
    debug_assert_eq!(
        manifest.authority,
        flexcore_protocol_ports::manifest::AuthorityDeclaration::SupervisoryRelayOnly
    );
    Ok(manifest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_constructs_and_answers_coverage_questions() {
        let scope = csms_scope().expect("valid scope");
        assert!(scope.covers_role(ProtocolRole::Server));
        assert!(!scope.covers_role(ProtocolRole::Client));
        assert!(scope.covers_relay());
        for class in [
            ResourceClass::Telemetry,
            ResourceClass::Control,
            ResourceClass::Discovery,
            ResourceClass::EventLog,
        ] {
            assert!(
                scope.covers_resource(class),
                "declared class {} must be covered",
                class.name()
            );
        }
        // The mandatory reviewed limitation record is non-empty and travels.
        assert!(scope.coverage_limitation_statements().len() >= 5);
        // The 1.6 exclusion is a recorded limitation, not an omission.
        let joined = scope.coverage_limitation_statements().join("\n");
        assert!(joined.contains("no_ocpp_1_6"));
    }

    #[test]
    fn scope_gate_refuses_out_of_scope_requests() {
        let scope = csms_scope().expect("valid scope");
        // The CSMS is the server role only: a client-role request (e.g. the
        // platform dialling INTO a charge point as OCPP client) is refused.
        assert!(scope
            .assert_supports(ProtocolRole::Client, false, &[ResourceClass::Telemetry])
            .is_err());
        // Pricing is not a declared resource class of this adapter.
        let error = scope
            .assert_supports(ProtocolRole::Server, false, &[ResourceClass::Pricing])
            .expect_err("pricing is not declared");
        assert!(
            error.to_string().contains("pricing"),
            "the error names the undeclared resource class: {error}"
        );
        // The declared combination passes, including relay.
        scope
            .assert_supports(
                ProtocolRole::Server,
                true,
                &[ResourceClass::Telemetry, ResourceClass::Control],
            )
            .expect("within scope");
    }

    #[test]
    fn manifest_constructs_with_relay_only_authority() {
        let manifest = csms_manifest().expect("valid manifest");
        assert_eq!(
            manifest.authority,
            flexcore_protocol_ports::manifest::AuthorityDeclaration::SupervisoryRelayOnly
        );
        assert!(manifest.declares(DeclaredCapability::CommandRelay));
        assert!(manifest.declares(DeclaredCapability::ExternalEventIngest));
        assert!(manifest.declares(DeclaredCapability::BoundedHistory));
        assert_eq!(manifest.adapter_id, crate::ADAPTER_ID);
        assert_eq!(manifest.protocol_id, crate::PROTOCOL_ID);
        // The edition string names the OPEN E-decision rather than claiming a
        // closed edition.
        assert!(manifest.protocol_edition.contains("P17-E-1 OPEN"));
    }
}
