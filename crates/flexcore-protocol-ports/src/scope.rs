// P14.01 — The mandatory protocol-adapter model-scope declaration (decision
// record 01_p14_01_adapter_architecture.md; mirrors
// `crates/flexcore-grid-api/src/scope.rs`, P4.02, report §12.4 — the pattern
// phase-14 context §3 constraint 4 makes binding for every adapter).
//
// Every protocol adapter MUST declare its scope: which protocol role(s) it
// plays, which direction of control it carries, which protocol resource
// classes it maps, and what it does NOT model. An undeclared-scope adapter
// is a **construction error**: `ProtocolScopeDeclaration` has no `Default`
// impl, no partially-filled constructor, and every field is validated in
// [`ProtocolScopeDeclaration::new`]. A concrete adapter struct holds a
// `ProtocolScopeDeclaration` by value, so an adapter cannot exist without
// one.
//
// Scope rules recorded here (decision record 01 §3):
//
// 1. The scope is declared, never inferred. There is no "universal protocol
//    adapter": an adapter that cannot state what it does not map has not
//    been reviewed, and a scope that claims to map everything is rejected at
//    construction.
// 2. The scope's `not_modelled` list is **mandatory and non-empty**. The
//    list is the reviewed limitation record (e.g. "no IEEE 2030.5 SFDI/LFDI
//    PKI operations — Phase 18 device identity", "no DNP3 master role",
//    "no GOOSE or SV — protection-grade, foundations §0.3"); it is surfaced
//    verbatim wherever the adapter's outputs are consumed.
// 3. The gate (`ProtocolScopeDeclaration::assert_supports`) refuses any
//    request whose role, control direction, or resource class exceeds the
//    declared scope — the adapter reports the refusal instead of
//    approximating (fail-closed).
// 4. The authority statement must name that external events enter the
//    command lifecycle at PROPOSED and that the adapter is NOT a dispatch
//    authority. A statement that omits either half is a construction error
//    (the P4.02 solver-statement validation pattern, adapted).

use serde::{Deserialize, Serialize};

/// Errors returned by protocol-scope construction and scope checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtocolScopeError {
    /// An identification field (adapter id, protocol name, note) is empty.
    /// Silence is never a declared scope.
    EmptyField(&'static str),
    /// The declared role list is empty — a scope that names no protocol role
    /// plays no role and is not a reviewed scope.
    EmptyRoles,
    /// The declared resource-class list is empty — a scope that maps no
    /// resource classes is not a reviewed scope.
    EmptyModelledResources,
    /// The `not_modelled` list is empty. A scope that claims to model
    /// everything is a construction error (the P4.02 rule: the list is the
    /// reviewed limitation record).
    EmptyNotModelled,
    /// The authority statement does not name the PROPOSED-entry rule or the
    /// not-a-dispatch-authority rule.
    InvalidAuthorityStatement,
    /// A requested role, control direction, or resource class exceeds the
    /// declared scope. The adapter refuses; it never approximates.
    ScopeExceeded {
        requested: String,
        declared: String,
    },
}

impl std::fmt::Display for ProtocolScopeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProtocolScopeError::EmptyField(field) => {
                write!(f, "protocol scope declaration field '{field}' is empty")
            }
            ProtocolScopeError::EmptyRoles => {
                write!(f, "protocol scope declares no protocol role")
            }
            ProtocolScopeError::EmptyModelledResources => {
                write!(f, "protocol scope declares no modelled resource classes")
            }
            ProtocolScopeError::EmptyNotModelled => {
                write!(
                    f, "protocol scope declares no limitations — a scope that models \
                        everything is not permitted (the reviewed-limitation rule)"
                )
            }
            ProtocolScopeError::InvalidAuthorityStatement => {
                write!(
                    f, "authority statement must name that external events enter the \
                        command lifecycle at PROPOSED and that the adapter is NOT a \
                        dispatch authority"
                )
            }
            ProtocolScopeError::ScopeExceeded { requested, declared } => {
                write!(f, "request exceeds the declared protocol scope (requested: {requested}; declared: {declared})")
            }
        }
    }
}

impl std::error::Error for ProtocolScopeError {}

/// One recorded "does NOT model" entry of a protocol scope declaration
/// (the P4.02 `NotModelled` shape, reused verbatim).
///
/// Free text, reviewed with the scope: the list is the adapter's limitation
/// record and is surfaced verbatim wherever the adapter's outputs are
/// consumed. Examples: "no GOOSE or SV — protection-grade (foundations
/// §0.3)", "no CA operations — Phase 18 device identity", "no utility SCADA
/// master role in this scope".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotModelled {
    /// Short stable label for the limitation (e.g. `"no_goose_sv"`).
    pub label: String,
    /// Recorded statement of what is not modelled and why that matters.
    pub statement: String,
}

impl NotModelled {
    /// Constructs a reviewed limitation entry; both fields must be
    /// non-empty.
    pub fn new(label: String, statement: String) -> Result<Self, ProtocolScopeError> {
        if label.trim().is_empty() {
            return Err(ProtocolScopeError::EmptyField("not_modelled.label"));
        }
        if statement.trim().is_empty() {
            return Err(ProtocolScopeError::EmptyField("not_modelled.statement"));
        }
        Ok(Self { label, statement })
    }
}

/// The protocol role an adapter declares it plays (decision record 01 §3).
///
/// A protocol has server and client ends; an adapter declares which end(s)
/// it implements. IEEE 2030.5 at the aggregator is a SERVER role (devices
/// connect to it as TLS clients, [S-1][S-5]); Modbus/SunSpec is a CLIENT
/// role (the platform polls devices, [S-16]); DNP3 starts as OUTSTATION
/// (the platform represents the fleet to utility SCADA, [S-14]) — modelled
/// as `Server` (it is the passive end answering the master).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolRole {
    /// The adapter implements the passive/server end: remote peers connect
    /// to it (IEEE 2030.5 AggregatorServer, DNP3 outstation).
    Server,
    /// The adapter implements the active/client end: it connects out and
    /// polls (SunSpec Modbus client, 104 client, 61850 MMS client).
    Client,
}

impl ProtocolRole {
    /// Canonical name for records and error messages.
    pub const fn name(self) -> &'static str {
        match self {
            ProtocolRole::Server => "server",
            ProtocolRole::Client => "client",
        }
    }
}

/// The control-direction scope of a protocol adapter (decision record 01
/// §3; ADR-010 extended).
///
/// `CommandRelay` does NOT mean dispatch authority. It means the adapter can
/// carry command-lifecycle output TO a device AFTER the three gates —
/// commands reach devices only through the 14-state lifecycle; a
/// `CommandRelay` adapter receives relayed, already-approved commands and
/// never constructs lifecycle states itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlScope {
    /// Read/monitor only: the adapter never carries a command, even a relayed
    /// one. This is the v0.x default for every protocol adapter (release
    /// ladder: v0.4.0 adapters are read-only / test-lab by default).
    MonitorOnly,
    /// Read/monitor plus command relay: the adapter forwards already-approved
    /// lifecycle commands and reports their transport outcomes. External
    /// control objects still enter at PROPOSED only.
    CommandRelay,
}

impl ControlScope {
    /// Canonical name for records and error messages.
    pub const fn name(self) -> &'static str {
        match self {
            ControlScope::MonitorOnly => "monitor-only",
            ControlScope::CommandRelay => "command-relay",
        }
    }

    /// Whether this control scope covers a command-relay request.
    pub fn covers_relay(self) -> bool {
        matches!(self, ControlScope::CommandRelay)
    }
}

/// A protocol resource class an adapter can declare it maps (decision
/// record 01 §3). The vocabulary is protocol-shaped, not vendor-shaped: a
/// 2030.5 DERControl maps to [`ResourceClass::Control`], an OpenADR event to
/// [`ResourceClass::Control`], a SunSpec meter block to
/// [`ResourceClass::Telemetry`] — the classes are what the platform
/// consumes, never the vendor spelling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceClass {
    /// Readings/measurements (2030.5 Reading/UsagePoint, SunSpec meter
    /// blocks, MQTT telemetry topics).
    Telemetry,
    /// Control shapes (2030.5 DERControl, OpenADR events, DNP3 analog/double
    /// bit outputs) — carried as PROPOSED candidates or relayed commands,
    /// never as issued setpoints.
    Control,
    /// Price/tariff signals (2030.5 pricing resources, OpenADR market
    /// contexts).
    Pricing,
    /// Event/log streams (2030.5 LogEventList, device audit events).
    EventLog,
    /// Discovery/enrollment surfaces (2030.5 EndDeviceList/SFDI enrollment,
    /// OpenADR VEN registration, Sparkplug BIRTH).
    Discovery,
    /// Network/device model import (CIM/CGMES profiles, 61850 SCL/ICD).
    ModelImport,
}

impl ResourceClass {
    /// Canonical name for records and error messages.
    pub const fn name(self) -> &'static str {
        match self {
            ResourceClass::Telemetry => "telemetry",
            ResourceClass::Control => "control",
            ResourceClass::Pricing => "pricing",
            ResourceClass::EventLog => "event-log",
            ResourceClass::Discovery => "discovery",
            ResourceClass::ModelImport => "model-import",
        }
    }
}

/// The mandatory model-scope declaration of a protocol adapter (P14.01;
/// mirrors `crates/flexcore-grid-api/src/scope.rs` exactly — the pattern
/// phase-14 context §3 constraint 4 requires).
///
/// Constructed only through [`ProtocolScopeDeclaration::new`], which
/// enforces every field — an undeclared-scope adapter is a construction
/// error. There is no `Default` impl and no way to build a partially
/// declared scope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolScopeDeclaration {
    /// Adapter identifier (who declared this scope; the registry allowlist
    /// key shape, e.g. `ieee-2030.5`).
    pub adapter_id: String,
    /// The protocol the adapter speaks (e.g. `ieee-2030.5`, `openadr-3.0`,
    /// `sunspec-modbus`). Declared, never inferred.
    pub protocol: String,
    /// The protocol role(s) the adapter plays (non-empty).
    pub roles: Vec<ProtocolRole>,
    /// The declared control direction.
    pub control_scope: ControlScope,
    /// Protocol resource classes the adapter maps (non-empty).
    pub modelled_resources: Vec<ResourceClass>,
    /// What the adapter does NOT model (non-empty; the reviewed limitation
    /// record; surfaced verbatim wherever the adapter's outputs are
    /// consumed).
    pub not_modelled: Vec<NotModelled>,
    /// The adapter's authority statement: it must state that external events
    /// enter the command lifecycle at PROPOSED and that the adapter is NOT a
    /// dispatch authority (ADR-010 extended; phase-14 context §3 constraint
    /// 1). Validated in [`ProtocolScopeDeclaration::new`].
    pub authority_statement: String,
    /// Recorded notes (protocol edition pins, boundary references, review
    /// pointers).
    pub notes: Vec<String>,
}

impl ProtocolScopeDeclaration {
    /// Constructs and validates a scope declaration. Every field is
    /// required; an undeclared-scope adapter is a construction error.
    pub fn new(
        adapter_id: String,
        protocol: String,
        roles: Vec<ProtocolRole>,
        control_scope: ControlScope,
        modelled_resources: Vec<ResourceClass>,
        not_modelled: Vec<NotModelled>,
        authority_statement: String,
        notes: Vec<String>,
    ) -> Result<Self, ProtocolScopeError> {
        if adapter_id.trim().is_empty() {
            return Err(ProtocolScopeError::EmptyField("adapter_id"));
        }
        if protocol.trim().is_empty() {
            return Err(ProtocolScopeError::EmptyField("protocol"));
        }
        if roles.is_empty() {
            return Err(ProtocolScopeError::EmptyRoles);
        }
        if modelled_resources.is_empty() {
            return Err(ProtocolScopeError::EmptyModelledResources);
        }
        if not_modelled.is_empty() {
            return Err(ProtocolScopeError::EmptyNotModelled);
        }
        let lowered = authority_statement.to_lowercase();
        let names_proposed_entry = lowered.contains("proposed") && lowered.contains("command lifecycle");
        let names_not_authority =
            lowered.contains("not") && lowered.contains("dispatch authority");
        if !names_proposed_entry || !names_not_authority {
            return Err(ProtocolScopeError::InvalidAuthorityStatement);
        }
        Ok(Self {
            adapter_id,
            protocol,
            roles,
            control_scope,
            modelled_resources,
            not_modelled,
            authority_statement,
            notes,
        })
    }

    /// Whether the scope covers the requested protocol role.
    pub fn covers_role(&self, role: ProtocolRole) -> bool {
        self.roles.contains(&role)
    }

    /// Whether the scope covers a command-relay request.
    pub fn covers_relay(&self) -> bool {
        self.control_scope.covers_relay()
    }

    /// Whether the scope maps the given resource class.
    pub fn covers_resource(&self, class: ResourceClass) -> bool {
        self.modelled_resources.contains(&class)
    }

    /// Asserts the scope covers the requested role, control direction, and
    /// every listed resource class; returns
    /// [`ProtocolScopeError::ScopeExceeded`] naming the first gap otherwise.
    /// Adapters run this gate BEFORE any protocol interaction; a request
    /// outside the declared scope is refused, never approximated (the P4.02
    /// fail-closed rule).
    pub fn assert_supports(
        &self,
        role: ProtocolRole,
        command_relay: bool,
        resources: &[ResourceClass],
    ) -> Result<(), ProtocolScopeError> {
        if !self.covers_role(role) {
            return Err(ProtocolScopeError::ScopeExceeded {
                requested: format!("{} role", role.name()),
                declared: self
                    .roles
                    .iter()
                    .map(|r| r.name())
                    .collect::<Vec<_>>()
                    .join(", "),
            });
        }
        if command_relay && !self.covers_relay() {
            return Err(ProtocolScopeError::ScopeExceeded {
                requested: format!("command relay (declared: {})", self.control_scope.name()),
                declared: self.control_scope.name().to_string(),
            });
        }
        for class in resources {
            if !self.covers_resource(*class) {
                return Err(ProtocolScopeError::ScopeExceeded {
                    requested: format!("resource class {}", class.name()),
                    declared: self
                        .modelled_resources
                        .iter()
                        .map(|c| c.name())
                        .collect::<Vec<_>>()
                        .join(", "),
                });
            }
        }
        Ok(())
    }

    /// The coverage-limitation records derived from this scope, for
    /// embedding wherever the adapter's outputs are consumed (the P4.02
    /// coverage-limitation pattern). The scope's `not_modelled` list is the
    /// source; the adapter adds nothing else.
    pub fn coverage_limitation_statements(&self) -> Vec<String> {
        self.not_modelled
            .iter()
            .map(|item| format!("{}: {}", item.label, item.statement))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reviewed_not_modelled() -> Vec<NotModelled> {
        vec![
            NotModelled::new(
                "no_pki_operations".to_string(),
                "no CA operations in this scope — PKI ties to Phase 13/18".to_string(),
            )
            .expect("valid"),
            NotModelled::new(
                "no_goose_sv".to_string(),
                "GOOSE and SV are protection-grade and out of scope (foundations §0.3)"
                    .to_string(),
            )
            .expect("valid"),
        ]
    }

    fn authority_statement() -> String {
        "external control objects enter the command lifecycle at PROPOSED; \
         this adapter is NOT a dispatch authority"
            .to_string()
    }

    fn valid_scope() -> ProtocolScopeDeclaration {
        ProtocolScopeDeclaration::new(
            "test.protocol".to_string(),
            "test-protocol".to_string(),
            vec![ProtocolRole::Client],
            ControlScope::MonitorOnly,
            vec![ResourceClass::Telemetry, ResourceClass::Discovery],
            reviewed_not_modelled(),
            authority_statement(),
            Vec::new(),
        )
        .expect("valid scope")
    }

    #[test]
    fn valid_scope_constructs_and_answers_coverage_questions() {
        let scope = valid_scope();
        assert!(scope.covers_role(ProtocolRole::Client));
        assert!(!scope.covers_role(ProtocolRole::Server));
        assert!(!scope.covers_relay());
        assert!(scope.covers_resource(ResourceClass::Telemetry));
        assert!(!scope.covers_resource(ResourceClass::Control));
        assert_eq!(scope.coverage_limitation_statements().len(), 2);
    }

    #[test]
    fn undeclared_scope_is_a_construction_error() {
        // Empty adapter id.
        assert_eq!(
            ProtocolScopeDeclaration::new(
                "  ".to_string(),
                "test-protocol".to_string(),
                vec![ProtocolRole::Client],
                ControlScope::MonitorOnly,
                vec![ResourceClass::Telemetry],
                reviewed_not_modelled(),
                authority_statement(),
                Vec::new(),
            ),
            Err(ProtocolScopeError::EmptyField("adapter_id"))
        );
        // Empty protocol name.
        assert_eq!(
            ProtocolScopeDeclaration::new(
                "test.protocol".to_string(),
                String::new(),
                vec![ProtocolRole::Client],
                ControlScope::MonitorOnly,
                vec![ResourceClass::Telemetry],
                reviewed_not_modelled(),
                authority_statement(),
                Vec::new(),
            ),
            Err(ProtocolScopeError::EmptyField("protocol"))
        );
        // No roles.
        assert_eq!(
            ProtocolScopeDeclaration::new(
                "test.protocol".to_string(),
                "test-protocol".to_string(),
                Vec::new(),
                ControlScope::MonitorOnly,
                vec![ResourceClass::Telemetry],
                reviewed_not_modelled(),
                authority_statement(),
                Vec::new(),
            ),
            Err(ProtocolScopeError::EmptyRoles)
        );
        // No modelled resource classes.
        assert_eq!(
            ProtocolScopeDeclaration::new(
                "test.protocol".to_string(),
                "test-protocol".to_string(),
                vec![ProtocolRole::Client],
                ControlScope::MonitorOnly,
                Vec::new(),
                reviewed_not_modelled(),
                authority_statement(),
                Vec::new(),
            ),
            Err(ProtocolScopeError::EmptyModelledResources)
        );
        // Claims to model everything: rejected.
        assert_eq!(
            ProtocolScopeDeclaration::new(
                "test.protocol".to_string(),
                "test-protocol".to_string(),
                vec![ProtocolRole::Client],
                ControlScope::MonitorOnly,
                vec![ResourceClass::Telemetry],
                Vec::new(),
                authority_statement(),
                Vec::new(),
            ),
            Err(ProtocolScopeError::EmptyNotModelled)
        );
        // An authority statement that omits the PROPOSED-entry rule.
        assert_eq!(
            ProtocolScopeDeclaration::new(
                "test.protocol".to_string(),
                "test-protocol".to_string(),
                vec![ProtocolRole::Client],
                ControlScope::MonitorOnly,
                vec![ResourceClass::Telemetry],
                reviewed_not_modelled(),
                "this adapter is NOT a dispatch authority".to_string(),
                Vec::new(),
            ),
            Err(ProtocolScopeError::InvalidAuthorityStatement)
        );
        // An authority statement that claims dispatch authority.
        assert_eq!(
            ProtocolScopeDeclaration::new(
                "test.protocol".to_string(),
                "test-protocol".to_string(),
                vec![ProtocolRole::Client],
                ControlScope::MonitorOnly,
                vec![ResourceClass::Telemetry],
                reviewed_not_modelled(),
                "external events enter at PROPOSED; this adapter IS a dispatch authority"
                    .to_string(),
                Vec::new(),
            ),
            Err(ProtocolScopeError::InvalidAuthorityStatement)
        );
    }

    #[test]
    fn scope_gate_refuses_undeclared_requests() {
        let scope = valid_scope();
        // Server role refused (declared: client).
        assert_eq!(
            scope.assert_supports(ProtocolRole::Server, false, &[ResourceClass::Telemetry]),
            Err(ProtocolScopeError::ScopeExceeded {
                requested: "server role".to_string(),
                declared: "client".to_string(),
            })
        );
        // Command relay refused (declared: monitor-only).
        assert_eq!(
            scope.assert_supports(ProtocolRole::Client, true, &[ResourceClass::Telemetry]),
            Err(ProtocolScopeError::ScopeExceeded {
                requested: "command relay (declared: monitor-only)".to_string(),
                declared: "monitor-only".to_string(),
            })
        );
        // Undeclared resource class refused, naming the gap.
        let error = scope
            .assert_supports(ProtocolRole::Client, false, &[ResourceClass::Control])
            .expect_err("control is not in the declared resource list");
        assert!(
            error.to_string().contains("control"),
            "the error names the undeclared resource class: {error}"
        );
        // Everything declared: passes.
        scope
            .assert_supports(
                ProtocolRole::Client,
                false,
                &[ResourceClass::Telemetry, ResourceClass::Discovery],
            )
            .expect("within scope");
    }

    #[test]
    fn command_relay_scope_covers_relay_requests() {
        let scope = ProtocolScopeDeclaration::new(
            "test.protocol".to_string(),
            "test-protocol".to_string(),
            vec![ProtocolRole::Client],
            ControlScope::CommandRelay,
            vec![ResourceClass::Telemetry, ResourceClass::Control],
            reviewed_not_modelled(),
            authority_statement(),
            Vec::new(),
        )
        .expect("valid scope");
        assert!(scope.covers_relay());
        scope
            .assert_supports(ProtocolRole::Client, true, &[ResourceClass::Control])
            .expect("command relay within scope");
    }
}
