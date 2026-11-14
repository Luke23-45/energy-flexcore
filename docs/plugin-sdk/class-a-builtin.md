# Class A — trusted built-in Rust modules

| Field | Value |
|---|---|
| Manifest `class` | `builtin-rust` |
| Entry point | `registered-type` (e.g. `neryva.builtin/sim-ess`) |
| Enforcement location | the kernel provider registry (P7.08, `crates/flexcore-kernel/src/registry.rs`) — recorded in `ledger/phase-07-plugin-sdk/12_13_enforcement_negatives.md` §2 |
| Trust level | "trusted" means composed from a reviewed manifest at the composition root — NOT exempt from the §5.7 permission vocabulary or the protected-kernel guard |

## 1. What a Class A plugin is

A Rust module compiled into the platform workspace, registered as a
PROVIDER of named contracts through the built-in provider registry
(`registry.rs::ProviderRegistry`). It is generic over the contract trait
object; the registry is populated at the composition root from a
`CompositionPlan` (`crates/flexcore-kernel/src/resolve.rs`) — there is no
global mutable registry anywhere (§6.4).

The shipped seed built-ins (DEFAULT-labelled seed ids, ratified by the
P7.15 profile work; `registry.rs::seed`):

| Contract id | Default provider | Registered type |
|---|---|---|
| `optimization.solver` | `builtin.highs-optimizer` | `HighsTier1Provider` |
| `grid.model` | `builtin.pandapower-grid` | `PandapowerGridProvider` |
| `telemetry.source` | `builtin.simulator-telemetry` | `SimulatorTelemetrySource` |

The shipped example manifest is
`plugins/manifests/builtin-ess-provider.manifest.json` (Class A, provides
`ess.simulation`, consumes `telemetry.edge-sample`, permission
`telemetry.read` only).

## 2. Registration path (registry / CompositionPlan)

1. Author the manifest with `"class": "builtin-rust"` and
   `"entry-point": { "registered-type": "<namespace>.<name>/<type>" }`.
2. Implement the Rust contract trait the seam names (the §9.1 interfaces:
   e.g. `Optimizer`, `GridModelProvider`, `TelemetrySource` — the seam
   inventory `ledger/phase-07-plugin-sdk/04_seam_inventory.md` is the
   contract → provider → consumer map).
3. Supply the validated manifest with the composition input; the resolver
   (P7.06) emits the ordered `CompositionPlan` (deterministic
   topological order, ties broken by plugin id).
4. At the composition root, register each plan entry through
   `registry.rs::ProviderRegistry` — a factory per contract id. The
   registry refuses protected-kernel contract ids outright
   (`PROTECTED_KERNEL_CONTRACT_REFUSED`), refuses a factory whose contract
   is not in the plan (`CONTRACT_NOT_IN_PLAN`), refuses double population
   (`REGISTRY_ALREADY_POPULATED`) and duplicate contract registration
   (`DUPLICATE_CONTRACT_REGISTRATION`).

Nothing auto-registers: registration is an explicit composition-root act
from a reviewed plan.

## 3. Class A rules you cannot opt out of

- The manifest gate is identical for all classes: the §5.7 permission
  vocabulary, the never-granted set, and the closed field sets all apply.
- `provider-of` claims of protected kernel contracts are refused twice:
  `PROTECTED_KERNEL_REPLACE` (resolver, P7.06) and
  `PROTECTED_KERNEL_CONTRACT_REFUSED` (registry, P7.08).
- Lifecycle: a Class A plugin runs the SAME §5.6 state machine as every
  other class (see [lifecycle-and-profiles.md](lifecycle-and-profiles.md)),
  including drain-before-replace with the `REPLACEMENT_BLOCKED_UNDRAINED`
  refusal.
- Profile ceiling: the composed profile's `permission-ceiling` is a maximum
  for Class A too (`PROFILE_PERMISSION_CEILING_EXCEEDED` at plan
  validation).

## 4. Honest status

The registry and its seed providers are WRITTEN, NOT BUILT and NOT RUN.
The swap exercise (P7.22) and conformance suite (P7.21) that exercise
Class A registration are written-unrun test files; the human build pass is
the verification gate. No claim here is a tool-verified fact.
