// P4.02 — The mandatory adapter model-scope declaration (ledger P4.02
// step 3; report §12.4).
//
// Every grid-model adapter MUST declare its model scope: balanced vs
// unbalanced, radial vs meshed, which equipment types it models, and what it
// does NOT model. An undeclared-scope adapter is a **construction error**:
// `ScopeDeclaration` has no `Default` impl, no partially-filled constructor,
// and every field is validated in [`ScopeDeclaration::new`]. A provider
// struct holds a `ScopeDeclaration` by value, so a provider cannot exist
// without one.
//
// Scope rules recorded here (decision note
// `ledger/phase-04-grid-feasibility-dispatch/01_02_grid_provider.md` §4):
//
// 1. The scope is declared, never inferred. There is no "universal
//    validator" (§12.4): an adapter that cannot state what it does not
//    model has not been reviewed, and a scope that claims to model
//    everything is rejected at construction.
// 2. The scope's `not_modelled` list is **mandatory and non-empty**. A
//    non-converging or uncovered network part can never pass a feasibility
//    check (§12.5 step 6; §17.5 invariant 9 — fail-closed).
// 3. The provider gate (`ScopeDeclaration::assert_supports`) refuses any
//    request whose phase, topology, or equipment exceeds the declared
//    scope — the adapter reports the refusal instead of approximating.
//
// D3 (network class / model fidelity) stays OPEN: the balanced-radial
// default is a RECOMMENDATION (report §25.2), never a decision. The scope
// declaration is exactly the mechanism that keeps D3 open — the recorded
// scope travels with every capability statement and every result's coverage
// limitations.

use serde::{Deserialize, Serialize};

/// Errors returned by scope construction and scope checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScopeError {
    /// An identification field (adapter id, solver statement, note) is
    /// empty. Silence is never a declared scope.
    EmptyField(&'static str),
    /// The declared equipment list is empty — a scope that names no
    /// equipment models nothing and is not a reviewed scope.
    EmptyModelledEquipment,
    /// The `not_modelled` list is empty. A scope that claims to model
    /// everything is a construction error (§12.4: there is no universal
    /// validator; the list is the reviewed limitation record).
    EmptyNotModelled,
    /// A requested phase, topology, or equipment kind exceeds the declared
    /// scope. The adapter refuses; it never approximates.
    ScopeExceeded { requested: String, declared: String },
}

impl std::fmt::Display for ScopeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScopeError::EmptyField(field) => {
                write!(f, "scope declaration field '{field}' is empty")
            }
            ScopeError::EmptyModelledEquipment => {
                write!(f, "scope declares no modelled equipment")
            }
            ScopeError::EmptyNotModelled => {
                write!(
                    f, "scope declares no limitations — a scope that models everything \
                        is not permitted (§12.4: no universal validator)"
                )
            }
            ScopeError::ScopeExceeded { requested, declared } => {
                write!(f, "request exceeds the declared scope (requested: {requested}; declared: {declared})")
            }
        }
    }
}

impl std::error::Error for ScopeError {}

/// The phase class the adapter evaluates: balanced (positive-sequence) vs
/// unbalanced three-phase (§12.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PhaseScope {
    /// Balanced (positive-sequence) power flow.
    Balanced,
    /// Unbalanced three-phase / asymmetric power flow. NOT declared by any
    /// v0.3.0 adapter: unbalanced three-phase modelling enters the track
    /// from v0.4.0 per the recommended default (§25.2; D3 OPEN).
    Unbalanced,
}

impl PhaseScope {
    /// Canonical name for records and error messages.
    pub const fn name(self) -> &'static str {
        match self {
            PhaseScope::Balanced => "balanced",
            PhaseScope::Unbalanced => "unbalanced",
        }
    }
}

/// The topology class the adapter evaluates (§12.4; §25.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TopologyScope {
    /// Radial feeders only.
    RadialOnly,
    /// Meshed networks only.
    MeshedOnly,
    /// Both radial and meshed networks.
    RadialAndMeshed,
}

impl TopologyScope {
    /// Canonical name for records and error messages.
    pub const fn name(self) -> &'static str {
        match self {
            TopologyScope::RadialOnly => "radial-only",
            TopologyScope::MeshedOnly => "meshed-only",
            TopologyScope::RadialAndMeshed => "radial-and-meshed",
        }
    }

    /// Whether this topology scope covers a radial request.
    pub fn covers_radial(self) -> bool {
        matches!(self, TopologyScope::RadialOnly | TopologyScope::RadialAndMeshed)
    }

    /// Whether this topology scope covers a meshed request.
    pub fn covers_meshed(self) -> bool {
        matches!(self, TopologyScope::MeshedOnly | TopologyScope::RadialAndMeshed)
    }
}

/// An equipment type an adapter can declare it models (§12.4 "which
/// equipment types it models").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EquipmentKind {
    /// Distribution line / cable (branch).
    Line,
    /// Two-winding transformer.
    TwoWindingTransformer,
    /// Constant-power / constant-impedance load element.
    LoadElement,
    /// Static generator (injection element).
    StaticGenerator,
    /// Shunt element (capacitor / reactor).
    Shunt,
    /// Switch / disconnect element (closed-state topology).
    Switch,
    /// External grid / slack source.
    ExternalGrid,
}

impl EquipmentKind {
    /// Canonical name for records and error messages.
    pub const fn name(self) -> &'static str {
        match self {
            EquipmentKind::Line => "line",
            EquipmentKind::TwoWindingTransformer => "two_winding_transformer",
            EquipmentKind::LoadElement => "load_element",
            EquipmentKind::StaticGenerator => "static_generator",
            EquipmentKind::Shunt => "shunt",
            EquipmentKind::Switch => "switch",
            EquipmentKind::ExternalGrid => "external_grid",
        }
    }
}

/// One recorded "does NOT model" entry of a scope declaration.
///
/// Free text, reviewed with the scope: the list is the adapter's limitation
/// record and is surfaced verbatim as model-coverage limitations on every
/// power-flow result (§12.5 step 6). Examples: "unbalanced three-phase
/// asymmetry", "protection coordination", "dynamic / transient behaviour",
/// "sub-cycle control", "optimal power flow (the feasibility gate never
/// optimizes, §12.5)".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotModelled {
    /// Short stable label for the limitation (e.g. `"unbalanced_three_phase"`).
    pub label: String,
    /// Recorded statement of what is not modelled and why that matters.
    pub statement: String,
}

impl NotModelled {
    /// Constructs a reviewed limitation entry; both fields must be
    /// non-empty.
    pub fn new(label: String, statement: String) -> Result<Self, ScopeError> {
        if label.trim().is_empty() {
            return Err(ScopeError::EmptyField("not_modelled.label"));
        }
        if statement.trim().is_empty() {
            return Err(ScopeError::EmptyField("not_modelled.statement"));
        }
        Ok(Self { label, statement })
    }
}

/// The mandatory model-scope declaration of a grid-model adapter
/// (ledger P4.02 step 3; report §12.4).
///
/// Constructed only through [`ScopeDeclaration::new`], which enforces every
/// field — an undeclared-scope adapter is a construction error. There is no
/// `Default` impl and no way to build a partially-declared scope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScopeDeclaration {
    /// Adapter identifier (who declared this scope).
    pub adapter_id: String,
    /// Declared phase class.
    pub phase: PhaseScope,
    /// Declared topology class.
    pub topology: TopologyScope,
    /// Equipment types the adapter models (non-empty).
    pub modelled_equipment: Vec<EquipmentKind>,
    /// What the adapter does NOT model (non-empty; reviewed limitation
    /// record; surfaced as result coverage limitations, §12.5 step 6).
    pub not_modelled: Vec<NotModelled>,
    /// The adapter's solver statement: what kind of evaluation it performs.
    /// Must state that it runs an AC power-flow evaluation of specified
    /// injections/controls and is not an AC-OPF optimizer (§12.5; AGENTS.md
    /// rule: solver classes are never conflated).
    pub solver_statement: String,
    /// Recorded notes (version pins, boundary references, review pointers).
    pub notes: Vec<String>,
}

impl ScopeDeclaration {
    /// Constructs and validates a scope declaration. Every field is
    /// required; an undeclared-scope adapter is a construction error.
    pub fn new(
        adapter_id: String,
        phase: PhaseScope,
        topology: TopologyScope,
        modelled_equipment: Vec<EquipmentKind>,
        not_modelled: Vec<NotModelled>,
        solver_statement: String,
        notes: Vec<String>,
    ) -> Result<Self, ScopeError> {
        if adapter_id.trim().is_empty() {
            return Err(ScopeError::EmptyField("adapter_id"));
        }
        if modelled_equipment.is_empty() {
            return Err(ScopeError::EmptyModelledEquipment);
        }
        if not_modelled.is_empty() {
            return Err(ScopeError::EmptyNotModelled);
        }
        if solver_statement.trim().is_empty() {
            return Err(ScopeError::EmptyField("solver_statement"));
        }
        let lowered = solver_statement.to_lowercase();
        let names_power_flow =
            lowered.contains("power flow") || lowered.contains("power-flow");
        let opf_only_with_negation =
            !lowered.contains("opf") || lowered.contains("not");
        if !names_power_flow || !opf_only_with_negation {
            return Err(ScopeError::EmptyField("solver_statement"));
        }
        Ok(Self {
            adapter_id,
            phase,
            topology,
            modelled_equipment,
            not_modelled,
            solver_statement,
            notes,
        })
    }

    /// Whether the scope covers the requested phase class.
    pub fn covers_phase(&self, phase: PhaseScope) -> bool {
        self.phase == phase
    }

    /// Whether the scope covers a radial request.
    pub fn covers_radial(&self) -> bool {
        self.topology.covers_radial()
    }

    /// Whether the scope covers a meshed request.
    pub fn covers_meshed(&self) -> bool {
        self.topology.covers_meshed()
    }

    /// Whether the scope models the given equipment kind.
    pub fn covers_equipment(&self, kind: EquipmentKind) -> bool {
        self.modelled_equipment.contains(&kind)
    }

    /// Asserts the scope covers the requested phase, topology, and every
    /// listed equipment kind; returns [`ScopeError::ScopeExceeded`] naming
    /// the first gap otherwise. Adapters run this gate BEFORE any
    /// evaluation; a request outside the declared scope is refused, never
    /// approximated (§12.4: verify model and solver limitations per case).
    pub fn assert_supports(
        &self,
        phase: PhaseScope,
        radial: bool,
        meshed: bool,
        equipment: &[EquipmentKind],
    ) -> Result<(), ScopeError> {
        if !self.covers_phase(phase) {
            return Err(ScopeError::ScopeExceeded {
                requested: format!("{} power flow", phase.name()),
                declared: format!("{} phase scope", self.phase.name()),
            });
        }
        if radial && !self.covers_radial() {
            return Err(ScopeError::ScopeExceeded {
                requested: "radial topology".to_string(),
                declared: format!("{} topology scope", self.topology.name()),
            });
        }
        if meshed && !self.covers_meshed() {
            return Err(ScopeError::ScopeExceeded {
                requested: "meshed topology".to_string(),
                declared: format!("{} topology scope", self.topology.name()),
            });
        }
        for kind in equipment {
            if !self.covers_equipment(*kind) {
                return Err(ScopeError::ScopeExceeded {
                    requested: format!("equipment kind {}", kind.name()),
                    declared: self
                        .modelled_equipment
                        .iter()
                        .map(|k| k.name())
                        .collect::<Vec<_>>()
                        .join(", "),
                });
            }
        }
        Ok(())
    }

    /// The coverage-limitation records derived from this scope, for
    /// embedding on every power-flow result (§12.5 step 6). The scope's
    /// `not_modelled` list is the source; the adapter adds nothing else.
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
                "unbalanced_three_phase".to_string(),
                "unbalanced three-phase asymmetry is not modelled in this scope".to_string(),
            )
            .expect("valid"),
            NotModelled::new(
                "ac_opf".to_string(),
                "the adapter runs an AC power-flow evaluation; it is not an AC-OPF optimizer (§12.5)".to_string(),
            )
            .expect("valid"),
        ]
    }

    fn statement() -> String {
        "runs an AC power-flow evaluation of specified injections and controls; \
         it is NOT an AC-OPF optimizer"
            .to_string()
    }

    fn valid_scope() -> ScopeDeclaration {
        ScopeDeclaration::new(
            "test.adapter".to_string(),
            PhaseScope::Balanced,
            TopologyScope::RadialAndMeshed,
            vec![EquipmentKind::Line, EquipmentKind::LoadElement],
            reviewed_not_modelled(),
            statement(),
            Vec::new(),
        )
        .expect("valid scope")
    }

    #[test]
    fn valid_scope_constructs_and_answers_coverage_questions() {
        let scope = valid_scope();
        assert!(scope.covers_phase(PhaseScope::Balanced));
        assert!(!scope.covers_phase(PhaseScope::Unbalanced));
        assert!(scope.covers_radial());
        assert!(scope.covers_meshed());
        assert!(scope.covers_equipment(EquipmentKind::Line));
        assert!(!scope.covers_equipment(EquipmentKind::Shunt));
        assert_eq!(scope.coverage_limitation_statements().len(), 2);
    }

    #[test]
    fn undeclared_scope_is_a_construction_error() {
        // Empty adapter id.
        assert_eq!(
            ScopeDeclaration::new(
                "  ".to_string(),
                PhaseScope::Balanced,
                TopologyScope::RadialOnly,
                vec![EquipmentKind::Line],
                reviewed_not_modelled(),
                statement(),
                Vec::new(),
            ),
            Err(ScopeError::EmptyField("adapter_id"))
        );
        // No modelled equipment.
        assert_eq!(
            ScopeDeclaration::new(
                "test.adapter".to_string(),
                PhaseScope::Balanced,
                TopologyScope::RadialOnly,
                Vec::new(),
                reviewed_not_modelled(),
                statement(),
                Vec::new(),
            ),
            Err(ScopeError::EmptyModelledEquipment)
        );
        // Claims to model everything: rejected.
        assert_eq!(
            ScopeDeclaration::new(
                "test.adapter".to_string(),
                PhaseScope::Balanced,
                TopologyScope::RadialOnly,
                vec![EquipmentKind::Line],
                Vec::new(),
                statement(),
                Vec::new(),
            ),
            Err(ScopeError::EmptyNotModelled)
        );
        // Empty solver statement.
        assert_eq!(
            ScopeDeclaration::new(
                "test.adapter".to_string(),
                PhaseScope::Balanced,
                TopologyScope::RadialOnly,
                vec![EquipmentKind::Line],
                reviewed_not_modelled(),
                "  ".to_string(),
                Vec::new(),
            ),
            Err(ScopeError::EmptyField("solver_statement"))
        );
        // A statement that names OPF without the explicit negation.
        assert_eq!(
            ScopeDeclaration::new(
                "test.adapter".to_string(),
                PhaseScope::Balanced,
                TopologyScope::RadialOnly,
                vec![EquipmentKind::Line],
                reviewed_not_modelled(),
                "runs an AC-OPF optimizer".to_string(),
                Vec::new(),
            ),
            Err(ScopeError::EmptyField("solver_statement"))
        );
    }

    #[test]
    fn scope_gate_refuses_undeclared_requests() {
        let scope = valid_scope();
        // Unbalanced request refused.
        assert_eq!(
            scope.assert_supports(
                PhaseScope::Unbalanced,
                true,
                false,
                &[EquipmentKind::Line]
            ),
            Err(ScopeError::ScopeExceeded {
                requested: "unbalanced power flow".to_string(),
                declared: "balanced phase scope".to_string(),
            })
        );
        // Undeclared equipment refused, naming the gap.
        let error = scope
            .assert_supports(
                PhaseScope::Balanced,
                true,
                false,
                &[EquipmentKind::Shunt],
            )
            .expect_err("shunt is not in the declared equipment list");
        assert!(
            error.to_string().contains("shunt"),
            "the error names the undeclared equipment kind: {error}"
        );
        // Everything declared: passes.
        scope
            .assert_supports(
                PhaseScope::Balanced,
                true,
                true,
                &[EquipmentKind::Line, EquipmentKind::LoadElement],
            )
            .expect("within scope");
    }

    #[test]
    fn topology_only_scope_refuses_the_other_topology() {
        let scope = ScopeDeclaration::new(
            "test.adapter".to_string(),
            PhaseScope::Balanced,
            TopologyScope::RadialOnly,
            vec![EquipmentKind::Line],
            reviewed_not_modelled(),
            statement(),
            Vec::new(),
        )
        .expect("valid scope");
        assert!(scope.covers_radial());
        assert!(!scope.covers_meshed());
        assert!(scope
            .assert_supports(PhaseScope::Balanced, false, true, &[])
            .is_err());
    }
}
