// P14.01 — The per-adapter capability manifest (decision record
// 01_p14_01_adapter_architecture.md §4; conventions §6.4; the kernel
// capability-manifest discipline of `crates/flexcore-kernel/src/manifest.rs`,
// adapted to the protocol-adapter tier without touching the kernel).
//
// Every protocol adapter constructs exactly ONE `ProtocolAdapterManifest`
// and holds it by value — like the scope declaration, an adapter cannot
// exist without its manifest. The manifest is the reviewed, serializable
// statement of what the adapter is permitted to do and declares:
//
// - the protocol identity (id + declared edition/version string),
// - the mandatory protocol scope declaration (held BY VALUE, so the two
//   cannot diverge),
// - the declared capabilities (a closed vocabulary),
// - the declared default cadence (supervisory minutes-scale; §3.5),
// - the authority declaration.
//
// The authority declaration is a ONE-VARIANT enum: `AuthorityDeclaration::
// SupervisoryRelayOnly` is the only constructible value. An adapter that
// holds dispatch authority is not expressible in this type — adding a
// variant is a reviewed ADR change against ADR-010 and the never-granted
// permission set (`dispatch.authorize`, `dispatch.issue`, `secret.read`,
// `network.any`), which stays byte-identical (conventions §6.4; phase-14
// context §3 constraint 2).

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::scope::ProtocolScopeDeclaration;

/// The closed capability vocabulary of a protocol adapter (decision record
/// 01 §4). An adapter declares the capabilities it implements; anything not
/// declared is refused, never approximated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeclaredCapability {
    /// The adapter can read native protocol data (poll or subscribe).
    Read,
    /// The adapter can discover device/endpoint inventories (2030.5
    /// EndDeviceList, SunSpec register scans, Sparkplug BIRTH).
    Discovery,
    /// The adapter can carry command-lifecycle output to a device AFTER the
    /// three gates (relay only — never authorization, never issue).
    CommandRelay,
    /// The adapter can receive external control objects (2030.5 DERControl,
    /// OpenADR events) and submit them into the command lifecycle at
    /// PROPOSED.
    ExternalEventIngest,
    /// The adapter can import network/device models (CIM/CGMES, SCL/ICD).
    ModelImport,
    /// The adapter can maintain protocol-level history/replay buffers
    /// (bounded; the store-and-forward P6.09 pattern).
    BoundedHistory,
}

impl DeclaredCapability {
    /// Canonical name for records and error messages.
    pub const fn name(self) -> &'static str {
        match self {
            DeclaredCapability::Read => "read",
            DeclaredCapability::Discovery => "discovery",
            DeclaredCapability::CommandRelay => "command-relay",
            DeclaredCapability::ExternalEventIngest => "external-event-ingest",
            DeclaredCapability::ModelImport => "model-import",
            DeclaredCapability::BoundedHistory => "bounded-history",
        }
    }
}

/// The control-authority declaration of a protocol adapter.
///
/// ONE variant, deliberately: protocol adapters are supervisory transports.
/// There is no `DispatchAuthority` variant to remove — it was never added.
/// The release ladder (v0.4.0 adapters read-only/test-lab by default) and
/// the simulation/shadow ceiling (conventions §6.7) govern when a
/// `CommandRelay` capability may be exercised; this type only ensures the
/// manifest can never claim more than relay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthorityDeclaration {
    /// The adapter is a supervisory transport: external events enter at
    /// PROPOSED, relayed commands are already gate-approved, and the adapter
    /// holds no dispatch authority (ADR-010 extended).
    SupervisoryRelayOnly,
}

impl AuthorityDeclaration {
    /// The only constructible authority declaration.
    pub const fn supervisory_relay_only() -> Self {
        AuthorityDeclaration::SupervisoryRelayOnly
    }

    /// Canonical name for records and error messages.
    pub const fn name(self) -> &'static str {
        match self {
            AuthorityDeclaration::SupervisoryRelayOnly => "supervisory-relay-only",
        }
    }
}

/// Errors returned by manifest construction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestError {
    /// An identification field (adapter id, protocol id, edition) is empty.
    /// Silence is never a declared manifest.
    EmptyField(&'static str),
    /// The declared capability list is empty — a manifest that names no
    /// capabilities declares nothing and is not a reviewed manifest.
    EmptyCapabilities,
    /// The scope declaration failed validation (re-raised from
    /// `ProtocolScopeDeclaration::new`).
    Scope(crate::scope::ProtocolScopeError),
    /// The declared cadence is zero — a zero poll cadence is not a cadence.
    ZeroCadence,
}

impl std::fmt::Display for ManifestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ManifestError::EmptyField(field) => {
                write!(f, "manifest field '{field}' is empty")
            }
            ManifestError::EmptyCapabilities => {
                write!(f, "manifest declares no capabilities")
            }
            ManifestError::Scope(error) => write!(f, "manifest scope invalid: {error}"),
            ManifestError::ZeroCadence => write!(f, "manifest cadence must be non-zero"),
        }
    }
}

impl std::error::Error for ManifestError {}

impl From<crate::scope::ProtocolScopeError> for ManifestError {
    fn from(error: crate::scope::ProtocolScopeError) -> Self {
        ManifestError::Scope(error)
    }
}

/// The mandatory capability manifest of a protocol adapter (P14.01).
///
/// Constructed only through [`ProtocolAdapterManifest::new`], which
/// enforces every field and builds the scope declaration inline — an
/// adapter cannot exist without a valid, reviewed manifest, and the
/// manifest cannot exist without a valid scope declaration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolAdapterManifest {
    /// The registered adapter id (the composition-registry allowlist key,
    /// e.g. `ieee-2030.5`).
    pub adapter_id: String,
    /// The protocol identifier (e.g. `ieee-2030.5`, `openadr-3.0`).
    pub protocol_id: String,
    /// The declared protocol edition/version string (e.g.
    /// `IEEE 2030.5-2018`). A declared string, not a conformance claim —
    /// certification is a human-gated lab process (conventions §7.3).
    pub protocol_edition: String,
    /// The mandatory scope declaration, held by value.
    pub scope: ProtocolScopeDeclaration,
    /// The declared capabilities (non-empty, closed vocabulary).
    pub capabilities: Vec<DeclaredCapability>,
    /// The declared default interaction cadence (supervisory minutes-scale,
    /// §3.5; the polling-budget seam of P14.20 consumes this).
    pub cadence: Duration,
    /// The authority declaration (always `SupervisoryRelayOnly`).
    pub authority: AuthorityDeclaration,
    /// Recorded notes (review pointers, boundary references).
    pub notes: Vec<String>,
}

impl ProtocolAdapterManifest {
    /// Constructs and validates a manifest. Every field is required; the
    /// scope declaration is validated in the same call.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        adapter_id: String,
        protocol_id: String,
        protocol_edition: String,
        scope: ProtocolScopeDeclaration,
        capabilities: Vec<DeclaredCapability>,
        cadence: Duration,
        notes: Vec<String>,
    ) -> Result<Self, ManifestError> {
        if adapter_id.trim().is_empty() {
            return Err(ManifestError::EmptyField("adapter_id"));
        }
        if protocol_id.trim().is_empty() {
            return Err(ManifestError::EmptyField("protocol_id"));
        }
        if protocol_edition.trim().is_empty() {
            return Err(ManifestError::EmptyField("protocol_edition"));
        }
        if capabilities.is_empty() {
            return Err(ManifestError::EmptyCapabilities);
        }
        if cadence.is_zero() {
            return Err(ManifestError::ZeroCadence);
        }
        Ok(Self {
            adapter_id,
            protocol_id,
            protocol_edition,
            scope,
            capabilities,
            cadence,
            authority: AuthorityDeclaration::supervisory_relay_only(),
            notes,
        })
    }

    /// Whether the manifest declares the given capability.
    pub fn declares(&self, capability: DeclaredCapability) -> bool {
        self.capabilities.contains(&capability)
    }

    /// The coverage-limitation statements of the manifest's scope
    /// (re-exported for every consumer surface).
    pub fn coverage_limitation_statements(&self) -> Vec<String> {
        self.scope.coverage_limitation_statements()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scope::{ControlScope, NotModelled, ProtocolRole, ResourceClass};

    fn scope() -> ProtocolScopeDeclaration {
        ProtocolScopeDeclaration::new(
            "test.adapter".to_string(),
            "test-protocol".to_string(),
            vec![ProtocolRole::Client],
            ControlScope::MonitorOnly,
            vec![ResourceClass::Telemetry],
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

    #[test]
    fn valid_manifest_constructs_with_relay_only_authority() {
        let manifest = ProtocolAdapterManifest::new(
            "test.adapter".to_string(),
            "test-protocol".to_string(),
            "test protocol edition 1.0".to_string(),
            scope(),
            vec![DeclaredCapability::Read],
            Duration::from_secs(60),
            Vec::new(),
        )
        .expect("valid manifest");
        assert_eq!(
            manifest.authority,
            AuthorityDeclaration::SupervisoryRelayOnly
        );
        assert!(manifest.declares(DeclaredCapability::Read));
        assert!(!manifest.declares(DeclaredCapability::CommandRelay));
        assert_eq!(manifest.coverage_limitation_statements().len(), 1);
    }

    #[test]
    fn manifest_rejects_empty_and_degenerate_declarations() {
        let base_capabilities = vec![DeclaredCapability::Read];
        // Empty adapter id.
        assert_eq!(
            ProtocolAdapterManifest::new(
                String::new(),
                "test-protocol".to_string(),
                "edition".to_string(),
                scope(),
                base_capabilities.clone(),
                Duration::from_secs(60),
                Vec::new(),
            ),
            Err(ManifestError::EmptyField("adapter_id"))
        );
        // Empty capabilities.
        assert_eq!(
            ProtocolAdapterManifest::new(
                "test.adapter".to_string(),
                "test-protocol".to_string(),
                "edition".to_string(),
                scope(),
                Vec::new(),
                Duration::from_secs(60),
                Vec::new(),
            ),
            Err(ManifestError::EmptyCapabilities)
        );
        // Zero cadence.
        assert_eq!(
            ProtocolAdapterManifest::new(
                "test.adapter".to_string(),
                "test-protocol".to_string(),
                "edition".to_string(),
                scope(),
                base_capabilities,
                Duration::ZERO,
                Vec::new(),
            ),
            Err(ManifestError::ZeroCadence)
        );
    }

    #[test]
    fn manifest_scope_errors_are_re_raised() {
        // An invalid scope declaration must surface as a manifest error:
        // a manifest cannot exist without a valid scope declaration.
        let scope_error = ProtocolScopeDeclaration::new(
            "test.adapter".to_string(),
            "test-protocol".to_string(),
            Vec::new(), // no roles — rejected
            ControlScope::MonitorOnly,
            vec![ResourceClass::Telemetry],
            vec![NotModelled::new(
                "reviewed_limitation".to_string(),
                "reviewed".to_string(),
            )
            .expect("valid")],
            "external events enter the command lifecycle at PROPOSED; \
             this adapter is NOT a dispatch authority"
                .to_string(),
            Vec::new(),
        )
        .expect_err("no roles is a scope construction error");
        assert_eq!(scope_error, ProtocolScopeError::EmptyRoles);
        assert_eq!(
            ManifestError::from(scope_error),
            ManifestError::Scope(ProtocolScopeError::EmptyRoles)
        );
    }
}
