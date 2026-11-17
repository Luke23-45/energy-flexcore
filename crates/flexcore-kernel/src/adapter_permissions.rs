// P14.16 — The adapter permission model in the kernel (decision record
// `docs/phases-11-20/phase-14-protocol-device-connectivity/records/
// 64_p14_16_adapter_permission_model.md`).
//
// The wave-1 protocol-adapter tier (P14.01, `flexcore-protocol-ports`)
// gives every adapter a capability manifest whose authority declaration is
// a ONE-VARIANT enum: `AuthorityDeclaration::SupervisoryRelayOnly`. This
// module is the KERNEL-side consumption of that tier: per-adapter
// capability manifests wired into kernel profiles as reviewed composition
// data, with the command-relay-only rule re-enforced at the kernel
// boundary (defence in depth — the manifest tier already enforces it
// structurally; the kernel re-checks it before any composition root sees
// the adapters).
//
// THE NEVER-GRANTED REVIEW (conventions §6.4; review ONLY — the set stays
// byte-identical, quoted here verbatim from
// `crates/flexcore-kernel/src/manifest.rs`):
//
//     pub const NEVER_GRANTED_PERMISSIONS: [&str; 4] = [
//         "dispatch.authorize",
//         "dispatch.issue",
//         "secret.read",
//         "network.any",
//     ];
//
// Review verdict (recorded in decision record 64 §3): the Phase 14
// adapter tier needs NONE of the §5.7 permission ids — adapters do not
// request plugin permissions at all. Adapter authority is expressed in the
// protocol-ports tier as `AuthorityDeclaration` (a type, not a permission
// string), and the type has exactly one variant. The never-granted set is
// unchanged; a test in this module pins that the adapter capability
// vocabulary and the §5.7 permission vocabulary remain disjoint, so no
// capability name can ever smuggle a permission id through a ceiling.
//
// THE COMMAND-RELAY-ONLY RULE (phase-14 context §3 constraint 1, ADR-010
// extended): commands reach devices only through the 14-state lifecycle
// with all three gates distinct; an adapter may relay an already-APPROVED
// command and may ingest external control objects at PROPOSED — never
// authorize, never issue, never construct a forward lifecycle state. In
// this module the rule is enforced as named checks over the bound
// manifests: a `CommandRelay` capability is accepted only when the
// adapter's declared control scope actually covers relay; an
// `ExternalEventIngest` capability only when the scope models the Control
// resource class; and the authority declaration must be
// `SupervisoryRelayOnly` (a re-match against the one-variant enum — if a
// future variant is ever added against ADR-010, this check is the kernel
// tripwire that refuses it).
//
// Profile wiring: a composition root declares which adapter bindings a
// profile deploys; [`bind_profile_adapters`] validates the binding set
// against the profile (posture, permission ceiling) and returns the
// reviewed adapter list. Profile DATA itself is unchanged (the P7.15
// closed-key parser is not extended): adapter manifests travel beside the
// profile, not inside it — an adapter manifest is not a plugin manifest
// and must never be parsed by the plugin schema.

use std::collections::BTreeMap;

use flexcore_protocol_ports::manifest::{
    AuthorityDeclaration, DeclaredCapability, ProtocolAdapterManifest,
};
use flexcore_protocol_ports::scope::ResourceClass;

use crate::manifest::NEVER_GRANTED_PERMISSIONS;
use crate::profile::{ControlAuthorityPosture, Profile};

/// The kernel-side binding of one protocol adapter: the adapter id (the
/// composition-registry allowlist key) and the adapter's own capability
/// manifest. The manifest is held BY VALUE — the binding cannot diverge
/// from the manifest the adapter was constructed with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdapterBinding {
    /// The adapter id (non-empty; must match the manifest's own
    /// `adapter_id` — a binding that renames the adapter is refused).
    pub adapter_id: String,
    /// The adapter's capability manifest (validated by its own
    /// constructor at the adapter tier; re-checked here).
    pub manifest: ProtocolAdapterManifest,
}

impl AdapterBinding {
    /// Constructs and validates one binding: the binding id must equal the
    /// manifest's adapter id (one adapter, one identity — never an alias).
    pub fn new(
        adapter_id: String,
        manifest: ProtocolAdapterManifest,
    ) -> Result<Self, AdapterPermissionError> {
        if adapter_id.trim().is_empty() {
            return Err(AdapterPermissionError::EmptyField("adapter_id"));
        }
        if adapter_id != manifest.adapter_id {
            return Err(AdapterPermissionError::AdapterIdMismatch {
                binding_id: adapter_id,
                manifest_id: manifest.adapter_id,
            });
        }
        Ok(Self { adapter_id, manifest })
    }

    /// The command-relay-only check for this binding (§3 of decision
    /// record 64): relay capability requires a relay-covering control
    /// scope; external-event ingest requires the Control resource class;
    /// the authority must be `SupervisoryRelayOnly`. Every other
    /// capability is scope-neutral.
    pub fn check_command_relay_only(&self) -> Result<(), AdapterPermissionError> {
        // Tripwire: the one-variant authority enum is re-matched
        // explicitly. If a dispatch-authority variant is ever added
        // against ADR-010, this arm refuses it at the kernel boundary.
        match self.manifest.authority {
            AuthorityDeclaration::SupervisoryRelayOnly => {}
        }
        if self.manifest.declares(DeclaredCapability::CommandRelay)
            && !self.manifest.scope.control_scope.covers_relay()
        {
            return Err(AdapterPermissionError::CommandRelayWithoutRelayScope {
                adapter_id: self.adapter_id.clone(),
            });
        }
        if self.manifest.declares(DeclaredCapability::ExternalEventIngest)
            && !self.manifest.scope.modelled_resources.contains(&ResourceClass::Control)
        {
            return Err(AdapterPermissionError::EventIngestWithoutControlClass {
                adapter_id: self.adapter_id.clone(),
            });
        }
        Ok(())
    }
}

/// The reviewed set of adapter bindings, keyed by adapter id (BTreeMap
/// determinism, the kernel's own house rule).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BoundAdapterRegistry {
    adapters: BTreeMap<String, AdapterBinding>,
}

impl BoundAdapterRegistry {
    /// Validates and binds a set of adapter bindings: every binding passes
    /// the command-relay-only check; duplicate adapter ids are refused.
    pub fn bind(
        bindings: Vec<AdapterBinding>,
    ) -> Result<Self, AdapterPermissionError> {
        let mut adapters = BTreeMap::new();
        for binding in bindings {
            binding.check_command_relay_only()?;
            if adapters.contains_key(&binding.adapter_id) {
                return Err(AdapterPermissionError::DuplicateAdapter {
                    adapter_id: binding.adapter_id.clone(),
                });
            }
            adapters.insert(binding.adapter_id.clone(), binding);
        }
        Ok(Self { adapters })
    }

    /// Looks up one adapter binding.
    pub fn get(&self, adapter_id: &str) -> Option<&AdapterBinding> {
        self.adapters.get(adapter_id)
    }

    /// The bound adapter ids (sorted — deterministic output).
    pub fn adapter_ids(&self) -> Vec<&str> {
        self.adapters.keys().map(String::as_str).collect()
    }

    /// The adapters that declare command relay (sorted). A composition
    /// root wires ONLY these to the lifecycle's relay path — and only the
    /// relay path: authorization, issuance, and mode selection stay in the
    /// platform (ADR-010 extended; the never-granted set untouched).
    pub fn command_relay_adapters(&self) -> Vec<&str> {
        self.adapters
            .values()
            .filter(|b| b.manifest.declares(DeclaredCapability::CommandRelay))
            .map(|b| b.adapter_id.as_str())
            .collect()
    }
}

/// The kernel-reviewed result of wiring adapter bindings to a profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundProfileAdapters {
    /// The profile the adapters are bound to.
    pub profile_id: String,
    /// The bound adapter ids (sorted).
    pub adapters: Vec<String>,
}

/// Errors the adapter permission model reports. Every failure is named.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdapterPermissionError {
    /// A required text field was empty.
    EmptyField(&'static str),
    /// The binding's adapter id does not match the manifest's own
    /// adapter id — one adapter, one identity.
    AdapterIdMismatch {
        /// The binding-side id.
        binding_id: String,
        /// The manifest-side id.
        manifest_id: String,
    },
    /// The same adapter id was bound twice.
    DuplicateAdapter {
        /// The duplicated adapter id.
        adapter_id: String,
    },
    /// A manifest declares the CommandRelay capability but its control
    /// scope does not cover relay — an inconsistent declaration.
    CommandRelayWithoutRelayScope {
        /// The offending adapter.
        adapter_id: String,
    },
    /// A manifest declares ExternalEventIngest but its scope does not
    /// model the Control resource class.
    EventIngestWithoutControlClass {
        /// The offending adapter.
        adapter_id: String,
    },
    /// The profile's posture is not the v0.x simulation/shadow posture —
    /// adapter bindings deploy only under the simulation/shadow ceiling
    /// (conventions §6.7; the release ladder governs the first real
    /// surfaces).
    ProfilePostureForbidden {
        /// The profile id.
        profile_id: String,
        /// The offending posture name.
        posture: &'static str,
    },
    /// The profile's permission ceiling contained a never-granted id at
    /// the binding boundary (defence in depth — the P7.15 parser already
    /// refuses this at parse; this is the kernel-side re-check).
    NeverGrantedInCeiling {
        /// The profile id.
        profile_id: String,
        /// The offending permission id.
        permission: String,
    },
    /// The profile names an adapter id that is not in the bound registry —
    /// a fail-closed refusal, never a silent skip.
    UnknownAdapter {
        /// The profile id.
        profile_id: String,
        /// The unbound adapter id.
        adapter_id: String,
    },
}

impl std::fmt::Display for AdapterPermissionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AdapterPermissionError::EmptyField(field) => {
                write!(f, "adapter binding field '{field}' is empty")
            }
            AdapterPermissionError::AdapterIdMismatch { binding_id, manifest_id } => {
                write!(
                    f,
                    "adapter binding id '{binding_id}' does not match the manifest's adapter id '{manifest_id}'"
                )
            }
            AdapterPermissionError::DuplicateAdapter { adapter_id } => {
                write!(f, "adapter '{adapter_id}' is bound twice")
            }
            AdapterPermissionError::CommandRelayWithoutRelayScope { adapter_id } => {
                write!(
                    f,
                    "adapter '{adapter_id}' declares command-relay but its control scope does not cover relay"
                )
            }
            AdapterPermissionError::EventIngestWithoutControlClass { adapter_id } => {
                write!(
                    f,
                    "adapter '{adapter_id}' declares external-event-ingest but does not model the control resource class"
                )
            }
            AdapterPermissionError::ProfilePostureForbidden { profile_id, posture } => {
                write!(
                    f,
                    "profile '{profile_id}' carries posture '{posture}'; adapter bindings deploy only under the simulation/shadow ceiling"
                )
            }
            AdapterPermissionError::NeverGrantedInCeiling { profile_id, permission } => {
                write!(
                    f,
                    "profile '{profile_id}' ceiling contains never-granted permission '{permission}'"
                )
            }
            AdapterPermissionError::UnknownAdapter { profile_id, adapter_id } => {
                write!(
                    f,
                    "profile '{profile_id}' names adapter '{adapter_id}', which is not in the bound registry"
                )
            }
        }
    }
}

impl std::error::Error for AdapterPermissionError {}

/// Wires adapter bindings to a kernel profile (the P14.16 "per-adapter
/// capability manifests wired into kernel profiles" step): the profile
/// stays P7.15 profile DATA; the adapters travel beside it and are
/// validated against it at the composition boundary.
///
/// Checks, in order:
/// 1. the profile's posture must be the simulation/shadow posture
///    (adapter bindings deploy only under the v0.x ceiling — conventions
///    §6.7; the release ladder governs anything beyond);
/// 2. the profile's permission ceiling must contain no never-granted id
///    (defence in depth; the parser already refuses at parse time);
/// 3. every adapter id the caller names must exist in the registry
///    (a profile that names an unbound adapter is a named refusal —
///    never a silent skip).
///
/// The returned [`BoundProfileAdapters`] is the reviewed record a
/// composition root consumes.
pub fn bind_profile_adapters(
    profile: &Profile,
    registry: &BoundAdapterRegistry,
    requested_adapter_ids: &[String],
) -> Result<BoundProfileAdapters, AdapterPermissionError> {
    // 1. The posture check (profiles parsed from data can only carry
    //    simulation-shadow; a hand-constructed profile is refused here —
    //    defence in depth, mirroring the P7.16 restatement).
    if profile.control_authority() != ControlAuthorityPosture::SimulationShadow {
        return Err(AdapterPermissionError::ProfilePostureForbidden {
            profile_id: profile.profile_id().to_string(),
            posture: profile.control_authority().name(),
        });
    }
    // 2. The never-granted ceiling re-check at the adapter boundary.
    for permission in profile.permission_ceiling() {
        if NEVER_GRANTED_PERMISSIONS.contains(&permission.as_str()) {
            return Err(AdapterPermissionError::NeverGrantedInCeiling {
                profile_id: profile.profile_id().to_string(),
                permission: permission.clone(),
            });
        }
    }
    // 3. Every requested adapter must be bound (fail-closed, named).
    let mut adapters: Vec<String> = Vec::with_capacity(requested_adapter_ids.len());
    for id in requested_adapter_ids {
        if registry.get(id).is_none() {
            return Err(AdapterPermissionError::UnknownAdapter {
                profile_id: profile.profile_id().to_string(),
                adapter_id: id.clone(),
            });
        }
        adapters.push(id.clone());
    }
    adapters.sort();
    adapters.dedup();
    Ok(BoundProfileAdapters {
        profile_id: profile.profile_id().to_string(),
        adapters,
    })
}

/// The never-granted review as DATA (decision record 64 §3): the review
/// verdict, quotable by records and the console. The set itself stays
/// byte-identical in [`crate::manifest::NEVER_GRANTED_PERMISSIONS`] —
/// this function only states the Phase 14 review outcome.
pub fn never_granted_review_statement() -> String {
    format!(
        "Phase 14 (P14.16) never-granted review: the set {:?} is unchanged; \
         the protocol-adapter tier expresses authority as the one-variant \
         AuthorityDeclaration::SupervisoryRelayOnly type (never as a §5.7 \
         permission id), and the adapter capability vocabulary is disjoint \
         from the §5.7 permission vocabulary (pinned by test).",
        NEVER_GRANTED_PERMISSIONS
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    use flexcore_protocol_ports::scope::{
        ControlScope, NotModelled, ProtocolRole, ProtocolScopeDeclaration, ResourceClass,
    };

    const NEVER_GRANTED_QUOTED: [&str; 4] = [
        "dispatch.authorize",
        "dispatch.issue",
        "secret.read",
        "network.any",
    ];

    fn scope_for(
        adapter_id: &str,
        control: ControlScope,
        resources: Vec<ResourceClass>,
    ) -> ProtocolScopeDeclaration {
        ProtocolScopeDeclaration::new(
            adapter_id.to_string(),
            "test-protocol".to_string(),
            vec![ProtocolRole::Server],
            control,
            resources,
            vec![NotModelled::new(
                "reviewed_limitation".to_string(),
                "the reviewed limitation record is mandatory and non-empty".to_string(),
            )
            .expect("valid")],
            "external events enter the command lifecycle at PROPOSED; this adapter \
             is NOT a dispatch authority"
                .to_string(),
            Vec::new(),
        )
        .expect("valid scope")
    }

    fn scope(
        control: ControlScope,
        resources: Vec<ResourceClass>,
    ) -> ProtocolScopeDeclaration {
        scope_for("test.adapter", control, resources)
    }

    fn manifest(
        control: ControlScope,
        resources: Vec<ResourceClass>,
        capabilities: Vec<DeclaredCapability>,
    ) -> ProtocolAdapterManifest {
        manifest_for("test.adapter", control, resources, capabilities)
    }

    fn manifest_for(
        adapter_id: &str,
        control: ControlScope,
        resources: Vec<ResourceClass>,
        capabilities: Vec<DeclaredCapability>,
    ) -> ProtocolAdapterManifest {
        ProtocolAdapterManifest::new(
            adapter_id.to_string(),
            "test-protocol".to_string(),
            "test protocol edition 1.0 (declared, never a conformance claim)"
                .to_string(),
            scope_for(adapter_id, control, resources),
            capabilities,
            Duration::from_secs(60),
            Vec::new(),
        )
        .expect("valid manifest")
    }

    // -- the never-granted review ---------------------------------------------

    #[test]
    fn never_granted_set_is_exactly_the_frozen_four_and_disjoint_from_capabilities() {
        // Byte-identical review: the frozen set is exactly the four ids.
        assert_eq!(NEVER_GRANTED_PERMISSIONS, NEVER_GRANTED_QUOTED);
        // The adapter capability vocabulary is DISJOINT from the §5.7
        // permission vocabulary — no capability name can smuggle a
        // permission id through a ceiling.
        let all_capabilities: [&str; 6] = [
            DeclaredCapability::Read.name(),
            DeclaredCapability::Discovery.name(),
            DeclaredCapability::CommandRelay.name(),
            DeclaredCapability::ExternalEventIngest.name(),
            DeclaredCapability::ModelImport.name(),
            DeclaredCapability::BoundedHistory.name(),
        ];
        for capability in all_capabilities {
            assert!(
                !NEVER_GRANTED_PERMISSIONS.contains(&capability),
                "capability '{capability}' must never collide with the never-granted set"
            );
        }
        // And the authority declaration names exactly relay-only.
        assert_eq!(
            AuthorityDeclaration::supervisory_relay_only().name(),
            "supervisory-relay-only"
        );
        // The review statement quotes the same four ids.
        let statement = never_granted_review_statement();
        for id in NEVER_GRANTED_QUOTED {
            assert!(statement.contains(id), "statement must quote '{id}'");
        }
    }

    // -- the command-relay-only rule ---------------------------------------------

    #[test]
    fn command_relay_requires_a_relay_covering_scope() {
        // A relay capability with a MonitorOnly scope is an inconsistent
        // declaration — named refusal.
        let binding = AdapterBinding::new(
            "test.adapter".to_string(),
            manifest(
                ControlScope::MonitorOnly,
                vec![ResourceClass::Telemetry],
                vec![DeclaredCapability::CommandRelay],
            ),
        )
        .expect("valid binding");
        assert_eq!(
            binding.check_command_relay_only(),
            Err(AdapterPermissionError::CommandRelayWithoutRelayScope {
                adapter_id: "test.adapter".to_string(),
            })
        );
        // The same capability under a relay-covering scope passes.
        let binding = AdapterBinding::new(
            "test.adapter".to_string(),
            manifest(
                ControlScope::CommandRelay,
                vec![ResourceClass::Telemetry],
                vec![DeclaredCapability::CommandRelay],
            ),
        )
        .expect("valid binding");
        binding.check_command_relay_only().expect("relay rule holds");
    }

    #[test]
    fn event_ingest_requires_the_control_resource_class() {
        let binding = AdapterBinding::new(
            "test.adapter".to_string(),
            manifest(
                ControlScope::MonitorOnly,
                vec![ResourceClass::Telemetry],
                vec![DeclaredCapability::ExternalEventIngest],
            ),
        )
        .expect("valid binding");
        assert_eq!(
            binding.check_command_relay_only(),
            Err(AdapterPermissionError::EventIngestWithoutControlClass {
                adapter_id: "test.adapter".to_string(),
            })
        );
        let binding = AdapterBinding::new(
            "test.adapter".to_string(),
            manifest(
                ControlScope::MonitorOnly,
                vec![ResourceClass::Telemetry, ResourceClass::Control],
                vec![DeclaredCapability::ExternalEventIngest],
            ),
        )
        .expect("valid binding");
        binding.check_command_relay_only().expect("ingest rule holds");
    }

    #[test]
    fn binding_refuses_identity_mismatch_and_registry_refuses_duplicates() {
        let good = manifest(
            ControlScope::MonitorOnly,
            vec![ResourceClass::Telemetry],
            vec![DeclaredCapability::Read],
        );
        assert_eq!(
            AdapterBinding::new("other.id".to_string(), good.clone()),
            Err(AdapterPermissionError::AdapterIdMismatch {
                binding_id: "other.id".to_string(),
                manifest_id: "test.adapter".to_string(),
            })
        );
        let registry = BoundAdapterRegistry::bind(vec![
            AdapterBinding::new("test.adapter".to_string(), good.clone())
                .expect("valid"),
            AdapterBinding::new("test.adapter".to_string(), good).expect("valid"),
        ]);
        assert!(matches!(
            registry,
            Err(AdapterPermissionError::DuplicateAdapter { .. })
        ));
    }

    // -- profile wiring -----------------------------------------------------------

    #[test]
    fn profile_binding_refuses_unknown_adapters_and_reports_sorted() {
        // A minimal valid profile shell (parsed through the real P7.15
        // parser; empty selections are legal AT PARSE and this module
        // does not compose — the plan-validation refusal is the P7.15
        // validator's own rule).
        let profile_text = r#"{
            "profile-schema": "flexcore-profile-v1",
            "profile-id": "adapter-lab",
            "name": "Adapter lab",
            "version": "0.1.0",
            "description": "A profile shell exercising the adapter binding surface.",
            "control-authority": "simulation-shadow",
            "permission-ceiling": ["telemetry.read"],
            "selections": []
        }"#;
        let profile = crate::profile::parse_profile(profile_text).expect("valid profile");
        let bound = AdapterBinding::new(
            "test.adapter".to_string(),
            manifest(
                ControlScope::MonitorOnly,
                vec![ResourceClass::Telemetry],
                vec![DeclaredCapability::Read],
            ),
        )
        .expect("valid binding");
        let registry =
            BoundAdapterRegistry::bind(vec![bound]).expect("valid registry");
        // Unknown adapter: named refusal.
        assert!(matches!(
            bind_profile_adapters(
                &profile,
                &registry,
                &["unknown.adapter".to_string()],
            ),
            Err(AdapterPermissionError::UnknownAdapter { ref adapter_id, .. })
                if adapter_id == "unknown.adapter"
        ));
        // Known adapter: bound, sorted.
        let result = bind_profile_adapters(
            &profile,
            &registry,
            &["test.adapter".to_string()],
        )
        .expect("binding");
        assert_eq!(result.profile_id, "adapter-lab");
        assert_eq!(result.adapters, vec!["test.adapter"]);
    }

    #[test]
    fn clean_ceiling_binds_and_the_parser_refuses_never_granted_at_parse() {
        // The binding boundary re-check runs over a profile whose ceiling
        // the P7.15 parser validated; a clean profile binds cleanly.
        let profile_text = r#"{
            "profile-schema": "flexcore-profile-v1",
            "profile-id": "clean-profile",
            "name": "Clean",
            "version": "0.1.0",
            "description": "A clean profile shell for the boundary test.",
            "control-authority": "simulation-shadow",
            "permission-ceiling": [],
            "selections": []
        }"#;
        let profile = crate::profile::parse_profile(profile_text).expect("valid profile");
        let registry = BoundAdapterRegistry::default();
        assert!(matches!(
            bind_profile_adapters(&profile, &registry, &[]),
            Ok(BoundProfileAdapters { adapters, .. }) if adapters.is_empty()
        ));
        // Defence in depth at the source: the parser itself refuses a
        // never-granted id in the ceiling (P7.15), so the kernel-side
        // re-check in bind_profile_adapters guards only hand-constructed
        // Profile values.
        let bad_profile_text = r#"{
            "profile-schema": "flexcore-profile-v1",
            "profile-id": "bad-ceiling",
            "name": "Bad ceiling",
            "version": "0.1.0",
            "description": "A profile shell whose ceiling names a never-granted id.",
            "control-authority": "simulation-shadow",
            "permission-ceiling": ["dispatch.authorize"],
            "selections": []
        }"#;
        let error = crate::profile::parse_profile(bad_profile_text)
            .expect_err("the parser must refuse a never-granted ceiling entry");
        assert_eq!(
            error.name(),
            "PROFILE_PERMISSION_CEILING_NEVER_GRANTED"
        );
    }

    #[test]
    fn command_relay_adapters_are_listed_deterministically() {
        let manifest_a = manifest_for(
            "adapter.a",
            ControlScope::CommandRelay,
            vec![ResourceClass::Telemetry, ResourceClass::Control],
            vec![DeclaredCapability::Read, DeclaredCapability::CommandRelay],
        );
        let manifest_b = manifest_for(
            "adapter.b",
            ControlScope::CommandRelay,
            vec![ResourceClass::Telemetry, ResourceClass::Control],
            vec![DeclaredCapability::Read],
        );
        let registry = BoundAdapterRegistry::bind(vec![
            AdapterBinding::new("adapter.b".to_string(), manifest_b).expect("valid"),
            AdapterBinding::new("adapter.a".to_string(), manifest_a).expect("valid"),
        ])
        .expect("valid registry");
        // Sorted ids, and only the relay-declaring adapter listed.
        assert_eq!(registry.adapter_ids(), vec!["adapter.a", "adapter.b"]);
        assert_eq!(registry.command_relay_adapters(), vec!["adapter.a"]);
    }
}
