// P7.08 — Built-in Rust provider registry, Class A (report §5.4 Class A,
// §5.2, §8.1 `plugins/builtin/`, §6.4 Factory/Registry pattern; ledger
// `ledger_phase_7.md` P7.08).
//
// The registry holds provider FACTORIES per contract id, generic over the
// contract trait-object type `T` (e.g. `dyn Optimizer`, `dyn
// GridModelProvider`). Registration happens ONLY at the composition root
// from a resolved `CompositionPlan` (P7.06) — never via a global mutable
// registry or service locator (§6.4 "avoid"; there is no `static` and no
// `lazy_static` anywhere in this module, and construction is on demand
// through [`ProviderRegistry::construct`]).
//
// Fail-closed rules, each a DISTINCT named error ([`RegistryError::name`]):
//
// | Name | Meaning |
// |---|---|
// | `PROTECTED_KERNEL_CONTRACT_REFUSED` | a factory is offered under a protected-kernel contract id (§5.1/§5.7/§9.1, ADR-005) — refused BEFORE any other check |
// | `CONTRACT_ID_MALFORMED` | the contract id is not a valid namespaced id |
// | `CONTRACT_NOT_IN_PLAN` | a provider is registered for a contract the attached `CompositionPlan` does not resolve |
// | `FACTORY_MISSING_FOR_PLAN_CONTRACT` | `register_from_plan` was given no factory for a plan contract — registration from a plan must cover it completely |
// | `REGISTRY_ALREADY_POPULATED` | `register_from_plan` called on a registry that already holds factories — re-composition builds a FRESH registry (P7.16), it does not mutate in place |
// | `DUPLICATE_CONTRACT_REGISTRATION` | a second factory for one contract — never a silent overwrite |
// | `CONTRACT_NOT_REGISTERED` | construction requested for an unregistered contract |
// | `CONSTRUCTION_FAILED` | a factory refused to build its provider (runtime config invalid, backend unavailable) |
// | `SWAP_CONTRACT_UNKNOWN` | `swap_provider` named a contract this registry does not hold |
// | `SWAP_LIFECYCLE_MISMATCH` | the lifecycle handle passed to `swap_provider` belongs to a different plugin than the registered provider of the contract |
// | `LIFECYCLE_REFUSED` | the P7.07 lifecycle refused the swap (its named error is preserved in the payload) |
//
// Protected-kernel reinforcement (P7.08 step: reuse/align the P7.06
// reserved-id list): the registry independently refuses ANY factory under a
// protected-kernel contract id, reusing [`PROTECTED_KERNEL_CONTRACT_IDS`]
// from [`crate::resolve`]. Reconciliation with the seam inventory (P7.04
// §6.1): the six protected non-seams it names — `CommandAuthorizer`,
// `DispatchGateway`, tenant isolation, audit integrity, command lifecycle,
// final dispatch validation — are exactly the six ids the P7.06 resolver
// hardcoded; the lists ALIGN (the union is the same six ids; the P6.14 edge
// command-path gate is not a contract id and adds none). See the decision
// record §"reserved-id reconciliation".
//
// Seed providers (report §8.1 `plugins/builtin/`; seam inventory P7.04 §3):
// the REAL first-party providers — `HighsTier1Provider`, `ClarabelTier2Provider`
// (`flexcore-optimization-api`), `PandapowerGridProvider` (`flexcore-grid-api`),
// `SimulatorTelemetrySource` (`flexcore-simulator`) — are registered as
// factories at the composition root through [`FnFactory`] closures that
// capture their runtime config (each constructor takes runtime parameters —
// a solver backend, a provider config, a grid backend + scope — so the
// factory takes them as captured parameters; NO constructor was invented
// inside another crate). The kernel crate itself depends on no `flexcore-*`
// crate (the P7.05/P7.06 dependency decision), so the real types cannot be
// constructed inside it; the seed contract/plugin-id constants below are
// DEFAULT-labelled proposals recorded in the decision record. Direct
// imports of concrete provider types are NOT introduced anywhere new
// (seam-inventory audit note G-2 respected).
//
// Determinism: the factory table is a `BTreeMap` keyed by contract id;
// iteration ([`ProviderRegistry::contract_ids`]) and error selection are in
// sorted, deterministic order. No `unwrap`/`expect` on untrusted input.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use crate::lifecycle::{LifecycleError, PluginLifecycle};
use crate::manifest::validate_contract_id;
use crate::resolve::{CompositionPlan, PROTECTED_KERNEL_CONTRACT_IDS};

// --- Seed seams (DEFAULT-labelled; see the decision record) -----------------------

/// The Class A seed seams and their first-party providers. The seam
/// inventory names the CONTRACT TRAITS, not contract ids; these ids are the
/// registry's declared seed ids (DEFAULT decision, proposed for the P7.15
/// profile work to ratify) and the plugin ids follow the report §8.1
/// `plugins/builtin/` naming intent.
pub mod seed {
    /// The §5.2 canonical scheduling seam (`Optimizer`, seam inventory §3.1).
    pub const CONTRACT_OPTIMIZER: &str = "optimization.solver";
    /// Default Class A provider plugin id for the optimizer seam.
    pub const PLUGIN_OPTIMIZER_DEFAULT: &str = "builtin.highs-optimizer";
    /// The §5.2 feasibility seam (`GridModelProvider`, seam inventory §3.2).
    pub const CONTRACT_GRID_MODEL: &str = "grid.model";
    /// Default Class A provider plugin id for the grid-model seam.
    pub const PLUGIN_GRID_MODEL_DEFAULT: &str = "builtin.pandapower-grid";
    /// The telemetry-ingestion seam (`TelemetrySource`, seam inventory §3.4).
    pub const CONTRACT_TELEMETRY_SOURCE: &str = "telemetry.source";
    /// Default Class A provider plugin id for the telemetry seam.
    pub const PLUGIN_TELEMETRY_SOURCE_DEFAULT: &str = "builtin.simulator-telemetry";

    /// Real first-party provider implementation names (seam inventory §3,
    /// grep-verified there; used as factory names so audit records and the
    /// decision record carry the seam inventory's exact names).
    pub const PROVIDER_HIGHS_TIER1: &str = "HighsTier1Provider";
    pub const PROVIDER_CLARABEL_TIER2: &str = "ClarabelTier2Provider";
    pub const PROVIDER_PANDAPOWER_GRID: &str = "PandapowerGridProvider";
    pub const PROVIDER_SIMULATOR_TELEMETRY: &str = "SimulatorTelemetrySource";
}

// --- The factory port ---------------------------------------------------------------

/// A provider factory for the contract trait object `T`. Construction is on
/// demand: the registry stores factories, never live providers (§6.4).
pub trait ProviderFactory<T: ?Sized> {
    /// The provider implementation's name (the seam inventory's name for
    /// first-party providers; carried into audit records).
    fn provider_name(&self) -> &str;
    /// Construct one provider instance. Failure is a NAMED
    /// [`RegistryError::ConstructionFailed`] — a factory never panics on
    /// invalid runtime config.
    fn construct(&self) -> Result<Arc<T>, RegistryError>;
}

/// Closure-backed factory adapter. This is how the composition root
/// registers the REAL built-in providers without the kernel depending on
/// them, e.g.:
///
/// ```ignore
/// // in apps/* (composition root only — §6.4):
/// let backend: Arc<dyn Tier1Backend> = /* runtime config */;
/// Box::new(FnFactory::new(seed::PROVIDER_HIGHS_TIER1, move || {
///     Ok(Arc::new(HighsTier1Provider::new(backend.clone())) as Arc<dyn Optimizer>)
/// }))
/// ```
///
/// The runtime config is captured by the closure — the factory "takes it as
/// a parameter"; no constructor was invented inside any other crate.
pub struct FnFactory<F, T: ?Sized>
where
    F: Fn() -> Result<Arc<T>, RegistryError>,
{
    name: String,
    construct: F,
    _marker: std::marker::PhantomData<fn() -> Arc<T>>,
}

impl<F, T: ?Sized> FnFactory<F, T>
where
    F: Fn() -> Result<Arc<T>, RegistryError>,
{
    pub fn new(name: &str, construct: F) -> FnFactory<F, T> {
        FnFactory { name: name.to_string(), construct, _marker: std::marker::PhantomData }
    }
}

impl<F, T: ?Sized> ProviderFactory<T> for FnFactory<F, T>
where
    F: Fn() -> Result<Arc<T>, RegistryError>,
{
    fn provider_name(&self) -> &str {
        &self.name
    }

    fn construct(&self) -> Result<Arc<T>, RegistryError> {
        (self.construct)()
    }
}

// --- Errors -------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryError {
    /// A factory was offered under a protected-kernel contract id. Checked
    /// FIRST, before shape, plan, or duplicate checks (fail-closed ordering:
    /// the protected kernel is never even discussed in the same breath as a
    /// malformed id).
    ProtectedKernelContract { contract_id: String },
    /// The contract id is not a valid namespaced id (same grammar the
    /// manifest validator enforces — revalidated here for direct
    /// registrations that bypass a manifest).
    ContractIdMalformed { contract_id: String, reason: String },
    /// The contract is not resolved by the attached composition plan.
    ContractNotInPlan { contract_id: String },
    /// `register_from_plan` had no factory for a plan contract.
    FactoryMissingForPlanContract { contract_id: String },
    /// `register_from_plan` was called on a registry that already holds
    /// factories; re-composition builds a fresh registry instead.
    RegistryAlreadyPopulated { held_contracts: usize },
    /// A second factory for one contract. Never a silent overwrite.
    DuplicateContractRegistration { contract_id: String },
    /// Construction requested for an unregistered contract.
    ContractNotRegistered { contract_id: String },
    /// A factory refused to build its provider (runtime config invalid,
    /// backend unavailable). The reason is the factory's own message.
    ConstructionFailed { contract_id: String, reason: String },
    /// `swap_provider` named a contract this registry does not hold.
    SwapContractUnknown { contract_id: String },
    /// The lifecycle handle belongs to a different plugin than the
    /// registered provider of the contract.
    SwapLifecycleMismatch {
        contract_id: String,
        registered_plugin_id: String,
        lifecycle_plugin_id: String,
    },
    /// The P7.07 lifecycle refused the swap; the source error's name is the
    /// specific reason (e.g. `REPLACEMENT_BLOCKED_UNDRAINED`).
    LifecycleRefused { contract_id: String, source: LifecycleError },
}

impl RegistryError {
    /// The stable machine-readable error name (the named-error table key).
    pub fn name(&self) -> &'static str {
        match self {
            RegistryError::ProtectedKernelContract { .. } => "PROTECTED_KERNEL_CONTRACT_REFUSED",
            RegistryError::ContractIdMalformed { .. } => "CONTRACT_ID_MALFORMED",
            RegistryError::ContractNotInPlan { .. } => "CONTRACT_NOT_IN_PLAN",
            RegistryError::FactoryMissingForPlanContract { .. } => "FACTORY_MISSING_FOR_PLAN_CONTRACT",
            RegistryError::RegistryAlreadyPopulated { .. } => "REGISTRY_ALREADY_POPULATED",
            RegistryError::DuplicateContractRegistration { .. } => "DUPLICATE_CONTRACT_REGISTRATION",
            RegistryError::ContractNotRegistered { .. } => "CONTRACT_NOT_REGISTERED",
            RegistryError::ConstructionFailed { .. } => "CONSTRUCTION_FAILED",
            RegistryError::SwapContractUnknown { .. } => "SWAP_CONTRACT_UNKNOWN",
            RegistryError::SwapLifecycleMismatch { .. } => "SWAP_LIFECYCLE_MISMATCH",
            RegistryError::LifecycleRefused { .. } => "LIFECYCLE_REFUSED",
        }
    }

    /// Convenience constructor for factories: a construction failure naming
    /// the contract and the factory's own reason.
    pub fn construction(contract_id: &str, reason: &str) -> RegistryError {
        RegistryError::ConstructionFailed {
            contract_id: contract_id.to_string(),
            reason: reason.to_string(),
        }
    }
}

impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RegistryError::ProtectedKernelContract { contract_id } => write!(
                f,
                "contract '{contract_id}' is a protected-kernel contract id; no provider factory may ever be registered under it (report §5.1/§5.7/§9.1, ADR-005)"
            ),
            RegistryError::ContractIdMalformed { contract_id, reason } => {
                write!(f, "contract id '{contract_id}' is malformed: {reason}")
            }
            RegistryError::ContractNotInPlan { contract_id } => write!(
                f,
                "contract '{contract_id}' is not resolved by the attached composition plan; a provider may only be registered for a plan contract"
            ),
            RegistryError::FactoryMissingForPlanContract { contract_id } => write!(
                f,
                "the composition plan resolves contract '{contract_id}' but no factory was supplied; registration from a plan must cover every plan contract"
            ),
            RegistryError::RegistryAlreadyPopulated { held_contracts } => write!(
                f,
                "the registry already holds {held_contracts} factory(ies); re-composition builds a fresh registry (P7.16), it does not mutate a populated one"
            ),
            RegistryError::DuplicateContractRegistration { contract_id } => write!(
                f,
                "a factory for contract '{contract_id}' is already registered; duplicates are refused, never silently overwritten"
            ),
            RegistryError::ContractNotRegistered { contract_id } => {
                write!(f, "no factory is registered for contract '{contract_id}'")
            }
            RegistryError::ConstructionFailed { contract_id, reason } => {
                write!(f, "the factory for contract '{contract_id}' refused to construct its provider: {reason}")
            }
            RegistryError::SwapContractUnknown { contract_id } => write!(
                f,
                "swap refused: contract '{contract_id}' is not registered"
            ),
            RegistryError::SwapLifecycleMismatch { contract_id, registered_plugin_id, lifecycle_plugin_id } => write!(
                f,
                "swap refused for contract '{contract_id}': the lifecycle handle is for plugin '{lifecycle_plugin_id}' but the registered provider is '{registered_plugin_id}'"
            ),
            RegistryError::LifecycleRefused { contract_id, source } => write!(
                f,
                "swap refused for contract '{contract_id}' by the plugin lifecycle ({}): {source}",
                source.name()
            ),
        }
    }
}

impl std::error::Error for RegistryError {}

/// Whether `contract_id` is one of the protected-kernel contract ids
/// (report §5.1/§5.7/§9.1; ADR-005). Shared by registration and swap; the
/// single source of the id list is [`PROTECTED_KERNEL_CONTRACT_IDS`].
pub fn is_protected_kernel_contract(contract_id: &str) -> bool {
    PROTECTED_KERNEL_CONTRACT_IDS.contains(&contract_id)
}

// --- The registry --------------------------------------------------------------------

struct RegisteredFactory<T: ?Sized> {
    /// The resolved provider plugin id from the composition plan.
    plugin_id: String,
    factory: Box<dyn ProviderFactory<T>>,
}

/// The Class A built-in provider registry, generic over the contract trait
/// object type. One registry instance per contract trait per composition —
/// a composition-root-owned value, never a global.
pub struct ProviderRegistry<T: ?Sized> {
    /// contract id → resolved provider plugin id (from the attached plan).
    plan_contracts: BTreeMap<String, String>,
    /// contract id → registered factory (sorted iteration by contract id).
    factories: BTreeMap<String, RegisteredFactory<T>>,
}

impl<T: ?Sized> ProviderRegistry<T> {
    /// Build a registry bound to a resolved composition plan. The plan's
    /// contract table is the ONLY set of contract ids this registry will
    /// ever accept a factory for; a protected-kernel id cannot appear here
    /// (the resolver refuses it structurally) but the registry still
    /// independently refuses one (defense in depth — see
    /// [`is_protected_kernel_contract`]).
    pub fn new(plan: &CompositionPlan) -> ProviderRegistry<T> {
        let plan_contracts: BTreeMap<String, String> = plan
            .contracts
            .iter()
            .map(|c| (c.contract_id.clone(), c.provider_plugin_id.clone()))
            .collect();
        ProviderRegistry { plan_contracts, factories: BTreeMap::new() }
    }

    /// Register factories for the whole plan at once (the composition
    /// root's happy path). Every plan contract MUST have a factory and
    /// every supplied factory MUST belong to a plan contract — both
    /// directions are enforced, with named errors, in deterministic
    /// (contract-id) order. The registry must be freshly built.
    pub fn register_from_plan(
        &mut self,
        plan: &CompositionPlan,
        factories: BTreeMap<String, Box<dyn ProviderFactory<T>>>,
    ) -> Result<(), RegistryError> {
        if !self.factories.is_empty() {
            return Err(RegistryError::RegistryAlreadyPopulated { held_contracts: self.factories.len() });
        }
        // Re-bind the plan table (the caller may pass the same plan it
        // built the registry with; the table is rebuilt identically).
        self.plan_contracts = plan
            .contracts
            .iter()
            .map(|c| (c.contract_id.clone(), c.provider_plugin_id.clone()))
            .collect();

        // Plan contracts in sorted order (the plan's `contracts` are already
        // sorted, but the registry does not rely on that here — it iterates
        // its own BTreeMap view). Protected check first, then coverage.
        let mut pending: BTreeMap<String, Box<dyn ProviderFactory<T>>> = factories;
        let contract_ids: Vec<String> = self.plan_contracts.keys().cloned().collect();
        for contract_id in &contract_ids {
            if is_protected_kernel_contract(contract_id) {
                return Err(RegistryError::ProtectedKernelContract { contract_id: contract_id.clone() });
            }
            let factory = match pending.remove(contract_id) {
                Some(factory) => factory,
                None => {
                    return Err(RegistryError::FactoryMissingForPlanContract { contract_id: contract_id.clone() })
                }
            };
            let plugin_id = match self.plan_contracts.get(contract_id) {
                Some(plugin_id) => plugin_id.clone(),
                // Unreachable: `contract_ids` comes from `plan_contracts`.
                // Kept explicit (no unwrap, even where provably safe).
                None => {
                    return Err(RegistryError::ContractNotInPlan { contract_id: contract_id.clone() })
                }
            };
            self.factories.insert(
                contract_id.clone(),
                RegisteredFactory { plugin_id, factory },
            );
        }

        // Any leftover factory names a contract the plan does not resolve —
        // refused (deterministic order: BTreeMap keys).
        if let Some(stray) = pending.keys().next() {
            return Err(RegistryError::ContractNotInPlan { contract_id: stray.clone() });
        }
        Ok(())
    }

    /// Register ONE factory for one plan contract (used by composition
    /// roots that assemble registrations incrementally, and by the swap
    /// tests). Refusal order: protected kernel first, then id grammar,
    /// then plan membership, then duplicates.
    pub fn register(
        &mut self,
        contract_id: &str,
        factory: Box<dyn ProviderFactory<T>>,
    ) -> Result<(), RegistryError> {
        if is_protected_kernel_contract(contract_id) {
            return Err(RegistryError::ProtectedKernelContract { contract_id: contract_id.to_string() });
        }
        validate_contract_id(contract_id).map_err(|reason| RegistryError::ContractIdMalformed {
            contract_id: contract_id.to_string(),
            reason,
        })?;
        let plugin_id = match self.plan_contracts.get(contract_id) {
            Some(plugin_id) => plugin_id.clone(),
            None => return Err(RegistryError::ContractNotInPlan { contract_id: contract_id.to_string() }),
        };
        if self.factories.contains_key(contract_id) {
            return Err(RegistryError::DuplicateContractRegistration {
                contract_id: contract_id.to_string(),
            });
        }
        self.factories
            .insert(contract_id.to_string(), RegisteredFactory { plugin_id, factory });
        Ok(())
    }

    /// Construct the provider for `contract_id` on demand. The registry
    /// stores factories, never live providers.
    pub fn construct(&self, contract_id: &str) -> Result<Arc<T>, RegistryError> {
        let registered = match self.factories.get(contract_id) {
            Some(registered) => registered,
            None => return Err(RegistryError::ContractNotRegistered { contract_id: contract_id.to_string() }),
        };
        registered.factory.construct()
    }

    /// The registered contract ids, sorted (deterministic iteration; P7.42).
    pub fn contract_ids(&self) -> Vec<&str> {
        self.factories.keys().map(|k| k.as_str()).collect()
    }

    /// The resolved provider plugin id for a registered contract.
    pub fn registered_plugin_id(&self, contract_id: &str) -> Option<&str> {
        self.factories.get(contract_id).map(|r| r.plugin_id.as_str())
    }

    /// How many factories are registered.
    pub fn len(&self) -> usize {
        self.factories.len()
    }

    pub fn is_empty(&self) -> bool {
        self.factories.is_empty()
    }

    /// The lifecycle-aware provider swap (P7.08 step 3): the ONLY path the
    /// P7.22 swap test will use. Goes through P7.07's drain-before-replace:
    /// the incumbent's lifecycle handle is required, and the swap proceeds
    /// ONLY if that lifecycle is fully drained (`LifecycleState::Stopped`)
    /// — an incumbent that is `Ready` (active without draining) or
    /// `Draining` (not yet quiesced) is REFUSED with the named error
    /// `REPLACEMENT_BLOCKED_UNDRAINED` (carried in
    /// [`RegistryError::LifecycleRefused`]), never a warning. On success the
    /// factory is replaced and the version change is recorded through the
    /// lifecycle (audited).
    ///
    /// Refusal order: protected kernel, contract known, lifecycle identity,
    /// lifecycle drain state, version-change record — then the commit.
    pub fn swap_provider(
        &mut self,
        contract_id: &str,
        factory: Box<dyn ProviderFactory<T>>,
        lifecycle: &mut PluginLifecycle,
        new_version: &str,
        actor: &str,
        reason: &str,
    ) -> Result<(), RegistryError> {
        if is_protected_kernel_contract(contract_id) {
            return Err(RegistryError::ProtectedKernelContract { contract_id: contract_id.to_string() });
        }
        let registered_plugin_id = match self.factories.get(contract_id) {
            Some(registered) => registered.plugin_id.clone(),
            None => return Err(RegistryError::SwapContractUnknown { contract_id: contract_id.to_string() }),
        };
        if lifecycle.plugin_id() != registered_plugin_id {
            return Err(RegistryError::SwapLifecycleMismatch {
                contract_id: contract_id.to_string(),
                registered_plugin_id,
                lifecycle_plugin_id: lifecycle.plugin_id().to_string(),
            });
        }
        // Drain-before-replace: the incumbent must be fully drained. The
        // refusal (including its audit record) is the lifecycle's.
        lifecycle.require_fully_drained(actor, reason).map_err(|source| {
            RegistryError::LifecycleRefused { contract_id: contract_id.to_string(), source }
        })?;

        // Commit the replacement, then record the version change through
        // the lifecycle (audited; the incumbent is Stopped).
        let registered = match self.factories.get_mut(contract_id) {
            Some(registered) => registered,
            None => return Err(RegistryError::SwapContractUnknown { contract_id: contract_id.to_string() }),
        };
        registered.factory = factory;
        lifecycle
            .record_version_change(new_version, actor, reason)
            .map_err(|source| RegistryError::LifecycleRefused { contract_id: contract_id.to_string(), source })?;
        Ok(())
    }
}
