// P7.06 — Dependency resolution over validated plugin manifests (report §5.5
// dependency-resolution sentence, §5.6 `Configured → Started` gate, §9.3
// compatibility matrix; ledger `ledger_phase_7.md` P7.06).
//
// The resolver consumes manifests ALREADY VALIDATED by [`crate::manifest`]
// (P7.05) and computes the composition plan BEFORE activation (§5.5: cycles
// and conflicts are detected before activation, never at runtime after
// partial activation — structurally guaranteed here: the resolver is a pure
// function with no side effects, and NO activation code exists in this
// crate; a failing resolution leaves nothing behind to activate).
//
// Failure modes, each a DISTINCT named error ([`ResolveError::name`]):
//
// | Name | Meaning (§5.5/§5.6) |
// |---|---|
// | `MISSING_PROVIDER` | a consumed contract has no providing manifest |
// | `UNSUPPORTED_API_VERSION` | provider's and consumer's `api-version` ranges do not intersect (§9.3) |
// | `PLATFORM_API_UNSUPPORTED` | a manifest's range does not cover the running platform plugin-API version |
// | `CYCLIC_DEPENDENCY` | a contract-mediated dependency cycle exists; the cycle is NAMED (plugin-id path) |
// | `CONFLICTING_PROVIDERS` | two or more providers for one contract without an explicit selection — never silent first-wins (§5.5) |
// | `SELECTION_UNRESOLVED` | an explicit selection names a contract/plugin that does not exist in the composition |
// | `PROTECTED_KERNEL_REPLACE` | a manifest claims a protected-kernel contract id in `provider-of` — the §5.7/ADR-010 non-replaceability rule made structural |
// | `DUPLICATE_PLUGIN_ID` | two manifests share one plugin id |
// | `EMPTY_MANIFEST_SET` | nothing to compose (refused, not an empty plan) |
// | `MANIFEST_SET_TOO_LARGE` | composition size above the fixed cap (fail-closed bound) |
//
// `SELECTION_EXPLICIT` (the ledger's fifth mode) is NOT an error — it is the
// deterministic resolution outcome when an explicit selection resolves a
// conflict: [`SelectionBasis::ExplicitSelection`] (machine name
// `SELECTION_EXPLICIT`) on the resolved contract. The conflict-resolution
// rule, in writing (P7.06 step 3): a multi-provider contract is resolved
// ONLY by an explicit selection carried in the composition input; there is
// no priority field, no first-wins, no default provider.
//
// Protected-kernel guard: the reserved contract ids below are hardcoded from
// report §5.1/§5.7/§9.1 (the P7.04 seam record is being written concurrently
// and is not yet citable; when it lands, this list is reconciled against it —
// the citations, not the ids, come from P7.04).
//
// Determinism (P7.42 depends on it): all iteration is over `BTreeMap`/
// `BTreeSet` or explicitly sorted vectors; the plan's registration order is
// the lexicographically-smallest topological order (Kahn's algorithm with a
// sorted ready set, ties broken by plugin id); the same (manifests,
// selections, platform version) input always yields the identical plan.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::manifest::{Manifest, PluginClass, SemVer};

/// The platform plugin-API version this kernel composes against (v0.5.0 SDK
/// baseline, report §2.1/§5.5). Every composed manifest must cover it, and
/// composed pairs must have overlapping ranges (§9.3). Pinned constant — a
/// version bump is a reviewed schema change, never ambient.
pub const CURRENT_PLATFORM_API_VERSION: SemVer = SemVer::new(0, 5, 0);

/// Protected-kernel contract ids (report §5.1 invariants, §5.7 authority
/// table, §9.1 protected interfaces; P6.29 handover §3 "the hard boundary").
/// NO manifest may declare any of these in `provider-of`; a claim is
/// [`ResolveError::ProtectedKernelReplace`]. This is ADR-005's
/// non-replaceable list made structural: the kernel does not check opinions,
/// it refuses the composition.
///
// Report trace per id (see the decision record §"reserved-id list"):
// - `kernel.command-authorizer` — §9.1 `CommandAuthorizer` (protected core).
// - `kernel.dispatch-gateway` — §9.1 `DispatchGateway` (protected core).
// - `kernel.dispatch-validation` — §5.1/§5.7 final dispatch validation.
// - `kernel.command-lifecycle` — §5.1 command lifecycle.
// - `kernel.audit-integrity` — §5.1 audit integrity.
// - `kernel.tenant-isolation` — §5.1 tenant isolation.
pub const PROTECTED_KERNEL_CONTRACT_IDS: [&str; 6] = [
    "kernel.command-authorizer",
    "kernel.dispatch-gateway",
    "kernel.dispatch-validation",
    "kernel.command-lifecycle",
    "kernel.audit-integrity",
    "kernel.tenant-isolation",
];

/// Fail-closed composition-size cap. A composition is a reviewed artifact;
/// nothing in §5.8 composes thousands of bundles, and the cap bounds the
/// resolver's work (and its recursion depth in cycle detection).
pub const MAX_COMPOSITION_MANIFESTS: usize = 1024;

// --- Composition input ---------------------------------------------------------

/// Explicit provider selections: contract id → plugin id. The ONLY way a
/// multi-provider contract resolves (P7.06 step 3: never silent first-wins).
/// Selections are validated against the composition: naming a contract no
/// manifest provides, or a plugin that does not provide the contract, is
/// [`ResolveError::SelectionUnresolved`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExplicitSelections {
    entries: BTreeMap<String, String>,
}

impl ExplicitSelections {
    pub fn new() -> ExplicitSelections {
        ExplicitSelections { entries: BTreeMap::new() }
    }

    /// Pin `plugin_id` as the provider of `contract_id`.
    pub fn insert(&mut self, contract_id: String, plugin_id: String) {
        self.entries.insert(contract_id, plugin_id);
    }

    pub fn get(&self, contract_id: &str) -> Option<&str> {
        self.entries.get(contract_id).map(|s| s.as_str())
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Sorted iteration (determinism: BTreeMap order is contract-id order).
    pub fn iter(&self) -> impl Iterator<Item = (&String, &String)> {
        self.entries.iter()
    }
}

// --- Composition output (consumed by P7.08 registry / P7.16 re-composition) ----

/// How a contract's provider was determined. `SELECTION_EXPLICIT` (the
/// ledger's fifth named mode) is the `ExplicitSelection` variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionBasis {
    /// Exactly one manifest provides the contract.
    UniqueProvider,
    /// An explicit selection pinned the provider (deterministic resolution
    /// of a would-be conflict).
    ExplicitSelection,
}

impl SelectionBasis {
    pub fn name(&self) -> &'static str {
        match self {
            SelectionBasis::UniqueProvider => "SELECTION_UNIQUE_PROVIDER",
            SelectionBasis::ExplicitSelection => "SELECTION_EXPLICIT",
        }
    }
}

impl fmt::Display for SelectionBasis {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// One contract of the plan: its resolved provider and who consumes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedContract {
    pub contract_id: String,
    pub provider_plugin_id: String,
    pub basis: SelectionBasis,
    /// Sorted consumer plugin ids.
    pub consumers: Vec<String>,
}

/// One entry of the ordered registration list (P7.08 consumes this).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanRegistration {
    pub plugin_id: String,
    pub version: String,
    pub class: PluginClass,
    /// Sorted contract ids provided.
    pub provides: Vec<String>,
    /// Sorted contract ids consumed.
    pub consumes: Vec<String>,
}

/// The composition plan: the deterministic, topologically ordered
/// registration list plus the resolved contract table. P7.08 (built-in
/// registry) registers in `registrations` order; P7.16 (re-composition)
/// re-runs the resolver and diffs plans. The plan carries NO activation —
/// it is data for the composition root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompositionPlan {
    /// Topologically ordered: every plugin's consumed providers register
    /// before it; independent plugins are ordered by plugin id.
    pub registrations: Vec<PlanRegistration>,
    /// Sorted by contract id.
    pub contracts: Vec<ResolvedContract>,
}

// --- Errors ----------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolveError {
    /// The composition input contained no manifests at all.
    EmptyManifestSet,
    /// The composition input exceeded [`MAX_COMPOSITION_MANIFESTS`].
    ManifestSetTooLarge { count: usize },
    /// Two manifests share one plugin id.
    DuplicatePluginId { plugin_id: String },
    /// A manifest's `provider-of` claims a protected-kernel contract id
    /// (report §5.1/§5.7/§9.1; ADR-005). Refused structurally.
    ProtectedKernelReplace { plugin_id: String, contract_id: String },
    /// A consumed contract has no provider in the composition.
    MissingProvider { contract_id: String, consumers: Vec<String> },
    /// Consumer and provider api-version ranges do not intersect (§9.3).
    UnsupportedApiVersion {
        contract_id: String,
        consumer_plugin_id: String,
        provider_plugin_id: String,
        consumer_range: String,
        provider_range: String,
    },
    /// A manifest's api-version range does not cover the running platform
    /// plugin-API version.
    PlatformApiUnsupported {
        plugin_id: String,
        declared_range: String,
        current_version: String,
    },
    /// A contract-mediated dependency cycle exists; `cycle` is the ordered
    /// plugin-id path (first == last node of the cycle).
    CyclicDependency { cycle: Vec<String> },
    /// Multiple providers for one contract and no explicit selection —
    /// never resolved silently (§5.5).
    ConflictingProviders { contract_id: String, providers: Vec<String> },
    /// An explicit selection names a contract or plugin the composition
    /// does not contain.
    SelectionUnresolved { contract_id: String, plugin_id: String, reason: String },
}

impl ResolveError {
    /// The stable machine-readable error name (the named-error table key).
    pub fn name(&self) -> &'static str {
        match self {
            ResolveError::EmptyManifestSet => "EMPTY_MANIFEST_SET",
            ResolveError::ManifestSetTooLarge { .. } => "MANIFEST_SET_TOO_LARGE",
            ResolveError::DuplicatePluginId { .. } => "DUPLICATE_PLUGIN_ID",
            ResolveError::ProtectedKernelReplace { .. } => "PROTECTED_KERNEL_REPLACE",
            ResolveError::MissingProvider { .. } => "MISSING_PROVIDER",
            ResolveError::UnsupportedApiVersion { .. } => "UNSUPPORTED_API_VERSION",
            ResolveError::PlatformApiUnsupported { .. } => "PLATFORM_API_UNSUPPORTED",
            ResolveError::CyclicDependency { .. } => "CYCLIC_DEPENDENCY",
            ResolveError::ConflictingProviders { .. } => "CONFLICTING_PROVIDERS",
            ResolveError::SelectionUnresolved { .. } => "SELECTION_UNRESOLVED",
        }
    }
}

impl fmt::Display for ResolveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ResolveError::EmptyManifestSet => {
                write!(f, "composition input contains no manifests (an empty composition is refused, not planned)")
            }
            ResolveError::ManifestSetTooLarge { count } => {
                write!(f, "composition contains {count} manifests, above the cap of {MAX_COMPOSITION_MANIFESTS}")
            }
            ResolveError::DuplicatePluginId { plugin_id } => {
                write!(f, "plugin id '{plugin_id}' is declared by two manifests")
            }
            ResolveError::ProtectedKernelReplace { plugin_id, contract_id } => {
                write!(
                    f,
                    "plugin '{plugin_id}' claims protected-kernel contract '{contract_id}'; the protected kernel (authorization, tenant isolation, audit integrity, command lifecycle, final dispatch validation) is not replaceable by any plugin (report §5.1/§5.7/§9.1, ADR-005)"
                )
            }
            ResolveError::MissingProvider { contract_id, consumers } => {
                write!(f, "no provider for contract '{contract_id}' (consumed by {})", consumers.join(", "))
            }
            ResolveError::UnsupportedApiVersion {
                contract_id,
                consumer_plugin_id,
                provider_plugin_id,
                consumer_range,
                provider_range,
            } => write!(
                f,
                "contract '{contract_id}': consumer '{consumer_plugin_id}' ({consumer_range}) and provider '{provider_plugin_id}' ({provider_range}) have disjoint api-version ranges"
            ),
            ResolveError::PlatformApiUnsupported { plugin_id, declared_range, current_version } => {
                write!(
                    f,
                    "plugin '{plugin_id}' targets api-version {declared_range}, which does not cover the running platform plugin-API version {current_version}"
                )
            }
            ResolveError::CyclicDependency { cycle } => {
                write!(f, "dependency cycle detected: {}", cycle.join(" -> "))
            }
            ResolveError::ConflictingProviders { contract_id, providers } => {
                write!(
                    f,
                    "contract '{contract_id}' has multiple providers ({}) and no explicit selection; conflicting providers are never resolved by a silent default (report §5.5)",
                    providers.join(", ")
                )
            }
            ResolveError::SelectionUnresolved { contract_id, plugin_id, reason } => {
                write!(f, "explicit selection of '{plugin_id}' for contract '{contract_id}' cannot be applied: {reason}")
            }
        }
    }
}

impl std::error::Error for ResolveError {}

// --- The resolver -----------------------------------------------------------------

/// Resolve the composition. Pure: same inputs → same plan, every run.
pub fn resolve(
    manifests: &[Manifest],
    selections: &ExplicitSelections,
) -> Result<CompositionPlan, ResolveError> {
    if manifests.is_empty() {
        return Err(ResolveError::EmptyManifestSet);
    }
    if manifests.len() > MAX_COMPOSITION_MANIFESTS {
        return Err(ResolveError::ManifestSetTooLarge { count: manifests.len() });
    }

    // 1. Index by plugin id; duplicates are an identity conflict, not a
    //    provider conflict (distinct named error).
    let mut by_id: BTreeMap<&str, &Manifest> = BTreeMap::new();
    for m in manifests {
        if by_id.insert(m.plugin_id.as_str(), m).is_some() {
            return Err(ResolveError::DuplicatePluginId { plugin_id: m.plugin_id.clone() });
        }
    }

    // 2. Protected-kernel guard (P7.06; ADR-005/§5.7 made structural).
    for (id, m) in &by_id {
        let mut claims: Vec<&String> = m.provider_of.iter().collect();
        claims.sort();
        for contract in claims {
            if PROTECTED_KERNEL_CONTRACT_IDS.contains(&contract.as_str()) {
                return Err(ResolveError::ProtectedKernelReplace {
                    plugin_id: (*id).to_string(),
                    contract_id: contract.clone(),
                });
            }
        }
    }

    // 3. Platform API gate: every manifest must cover the pinned platform
    //    plugin-API version (§9.3 compatibility matrix baseline).
    for (id, m) in &by_id {
        if !m.covers_api_version(CURRENT_PLATFORM_API_VERSION) {
            return Err(ResolveError::PlatformApiUnsupported {
                plugin_id: (*id).to_string(),
                declared_range: m.api_version.source().to_string(),
                current_version: CURRENT_PLATFORM_API_VERSION.to_string(),
            });
        }
    }

    // 4. Provider map: contract → sorted providing plugin ids.
    let mut providers: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for (id, m) in &by_id {
        for contract in &m.provider_of {
            providers.entry(contract.as_str()).or_default().push(*id);
        }
    }
    for list in providers.values_mut() {
        list.sort();
    }

    // 5. Explicit selections must reference real contracts and real providers.
    for (contract, plugin_id) in selections.iter() {
        match providers.get(contract.as_str()) {
            None => {
                return Err(ResolveError::SelectionUnresolved {
                    contract_id: contract.clone(),
                    plugin_id: plugin_id.clone(),
                    reason: "no manifest in the composition provides this contract".to_string(),
                })
            }
            Some(list) => {
                if !list.iter().any(|p| *p == plugin_id.as_str()) {
                    return Err(ResolveError::SelectionUnresolved {
                        contract_id: contract.clone(),
                        plugin_id: plugin_id.clone(),
                        reason: "the selected plugin does not declare this contract in its provider-of"
                            .to_string(),
                    });
                }
            }
        }
    }

    // 6. Resolve every provided contract. Conflicts without an explicit
    //    selection are an error — never a silent first-wins (§5.5).
    let mut resolved: BTreeMap<&str, (&str, SelectionBasis)> = BTreeMap::new();
    for (contract, list) in &providers {
        if list.len() == 1 {
            let basis = if selections.get(contract).is_some() {
                SelectionBasis::ExplicitSelection
            } else {
                SelectionBasis::UniqueProvider
            };
            resolved.insert(contract, (list[0], basis));
        } else {
            match selections.get(contract) {
                Some(pid) => {
                    // Already validated above to be one of `list`.
                    resolved.insert(contract, (pid, SelectionBasis::ExplicitSelection));
                }
                None => {
                    return Err(ResolveError::ConflictingProviders {
                        contract_id: contract.to_string(),
                        providers: list.iter().map(|p| p.to_string()).collect(),
                    })
                }
            }
        }
    }

    // 7. Consumer pass: missing providers, api-version compatibility, and
    //    the dependency edges (consumer → providing plugin, contract-mediated).
    let mut depends_on: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for (id, m) in &by_id {
        let node = depends_on.entry(*id).or_default();
        let mut consumed: Vec<&String> = m.consumes.iter().collect();
        consumed.sort();
        for contract in consumed {
            let provider = match resolved.get(contract.as_str()) {
                Some((p, _)) => *p,
                None => {
                    // Contract consumed but provided by nobody (or only by a
                    // protected-kernel claim already refused above).
                    let mut consumers: Vec<String> = by_id
                        .values()
                        .filter(|m2| m2.consumes.iter().any(|c| c == contract))
                        .map(|m2| m2.plugin_id.clone())
                        .collect();
                    consumers.sort();
                    return Err(ResolveError::MissingProvider {
                        contract_id: contract.clone(),
                        consumers,
                    });
                }
            };
            let provider_manifest = match by_id.get(provider) {
                Some(pm) => *pm,
                None => {
                    // Unreachable: providers were built from by_id. Kept as a
                    // named error, never a panic (no unwrap policy).
                    let mut consumers: Vec<String> = by_id
                        .values()
                        .filter(|m2| m2.consumes.iter().any(|c| c == contract))
                        .map(|m2| m2.plugin_id.clone())
                        .collect();
                    consumers.sort();
                    return Err(ResolveError::MissingProvider {
                        contract_id: contract.clone(),
                        consumers,
                    });
                }
            };
            if !provider_manifest.api_version.intersects(&m.api_version) {
                return Err(ResolveError::UnsupportedApiVersion {
                    contract_id: contract.to_string(),
                    consumer_plugin_id: (*id).to_string(),
                    provider_plugin_id: provider_manifest.plugin_id.clone(),
                    consumer_range: m.api_version.source().to_string(),
                    provider_range: provider_manifest.api_version.source().to_string(),
                });
            }
            node.insert(provider);
        }
    }

    // 8. Cycle detection over the contract-mediated dependency graph. The
    //    cycle is named as the ordered plugin-id path (first node repeated
    //    at the end). Neighbors iterate in sorted order, so the FIRST cycle
    //    reported is deterministic for a given input.
    let nodes: Vec<&str> = depends_on.keys().copied().collect();
    if let Some(cycle) = find_cycle(&nodes, &depends_on) {
        return Err(ResolveError::CyclicDependency { cycle });
    }

    // 9. Topological registration order: dependencies first; among ready
    //    nodes, the smallest plugin id first (Kahn's algorithm over a sorted
    //    ready set — P7.42's determinism requirement).
    let mut dependents: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    let mut indegree: BTreeMap<&str, usize> = BTreeMap::new();
    for (node, deps) in &depends_on {
        indegree.insert(node, deps.len());
        for dep in deps {
            dependents.entry(*dep).or_default().insert(node);
        }
    }
    let mut ready: BTreeSet<&str> = indegree
        .iter()
        .filter(|(_, degree)| **degree == 0)
        .map(|(node, _)| *node)
        .collect();
    let mut order: Vec<&str> = Vec::with_capacity(nodes.len());
    while let Some(next) = ready.iter().next().copied() {
        ready.remove(next);
        order.push(next);
        if let Some(consumers) = dependents.get(next) {
            for consumer in consumers {
                if let Some(degree) = indegree.get_mut(consumer) {
                    if *degree > 0 {
                        *degree -= 1;
                        if *degree == 0 {
                            ready.insert(consumer);
                        }
                    }
                }
            }
        }
    }

    // 10. Emit the plan (all aggregates sorted).
    let registrations: Vec<PlanRegistration> = order
        .iter()
        .filter_map(|id| by_id.get(*id).map(|m| build_registration(id, m)))
        .collect();
    let contracts: Vec<ResolvedContract> = resolved
        .iter()
        .map(|(contract, (provider, basis))| {
            let mut consumers: Vec<String> = by_id
                .values()
                .filter(|m| m.consumes.iter().any(|c| c == contract))
                .map(|m| m.plugin_id.clone())
                .collect();
            consumers.sort();
            ResolvedContract {
                contract_id: contract.to_string(),
                provider_plugin_id: provider.to_string(),
                basis: *basis,
                consumers,
            }
        })
        .collect();

    Ok(CompositionPlan { registrations, contracts })
}

fn build_registration(id: &str, m: &Manifest) -> PlanRegistration {
    let mut provides: Vec<String> = m.provider_of.clone();
    provides.sort();
    let mut consumes: Vec<String> = m.consumes.clone();
    consumes.sort();
    PlanRegistration {
        plugin_id: id.to_string(),
        version: m.version.to_string(),
        class: m.class,
        provides,
        consumes,
    }
}

const COLOR_UNVISITED: u8 = 0;
const COLOR_IN_PROGRESS: u8 = 1;
const COLOR_DONE: u8 = 2;

/// Deterministic DFS cycle detection: nodes and neighbors in sorted order;
/// the first back edge found names the cycle (path from the in-progress node
/// back to itself). Recursion depth is bounded by [`MAX_COMPOSITION_MANIFESTS`].
fn find_cycle(
    nodes: &[&str],
    depends_on: &BTreeMap<&str, BTreeSet<&str>>,
) -> Option<Vec<String>> {
    let mut color: BTreeMap<&str, u8> =
        nodes.iter().map(|n| (*n, COLOR_UNVISITED)).collect();
    let mut path: Vec<&str> = Vec::new();
    for start in nodes {
        if color.get(start).copied().unwrap_or(COLOR_DONE) != COLOR_UNVISITED {
            continue;
        }
        if let Some(cycle) = dfs_visit(start, depends_on, &mut color, &mut path) {
            return Some(cycle);
        }
    }
    None
}

fn dfs_visit<'a>(
    node: &'a str,
    depends_on: &BTreeMap<&'a str, BTreeSet<&'a str>>,
    color: &mut BTreeMap<&'a str, u8>,
    path: &mut Vec<&'a str>,
) -> Option<Vec<String>> {
    color.insert(node, COLOR_IN_PROGRESS);
    path.push(node);
    if let Some(neighbors) = depends_on.get(node) {
        for next in neighbors {
            match color.get(next).copied().unwrap_or(COLOR_DONE) {
                COLOR_IN_PROGRESS => {
                    let position = match path.iter().position(|n| *n == *next) {
                        Some(pos) => pos,
                        // Defensive: an in-progress node is always on the
                        // current path. Fallback keeps the error named and
                        // non-panicking regardless.
                        None => 0,
                    };
                    let mut cycle: Vec<String> =
                        path[position..].iter().map(|n| n.to_string()).collect();
                    cycle.push((*next).to_string());
                    return Some(cycle);
                }
                COLOR_UNVISITED => {
                    if let Some(cycle) = dfs_visit(next, depends_on, color, path) {
                        return Some(cycle);
                    }
                }
                _ => {}
            }
        }
    }
    path.pop();
    color.insert(node, COLOR_DONE);
    None
}
