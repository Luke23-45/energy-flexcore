# `wit/` — FlexCore plugin WIT packages (P7.09)

Versioned WIT interface packages for the v0.5.0 plugin SDK (ledger
`ledger/phase-07-plugin-sdk/ledger_phase_7.md` §P7.09; report §5.4 Class B,
§8.1, §9.1, §10.5, §22.2).

## 0. Honest status — READ FIRST

**These WIT files are WRITTEN-UNVALIDATED.** `wasm-tools` has never been run
in this loop (no install, no build, no execution — the delivery loop is
documentation-and-design only). The first `wasm-tools` validation is a
**human gate input**, prepared verbatim in `TOOLCHAIN.md` §3 and recorded in
`ledger/phase-07-plugin-sdk/40_toolchain_pinning.md`. Until that run is
recorded, every claim below is a design statement, not a tool-verified fact.

The P7.27 CI wave owns wiring `wasm-tools`/`wit-bindgen` compatibility
validation as config-only jobs (ledger P7.27 step 1: "WIT compatibility
validation"); that seam is recorded in
`ledger/phase-07-plugin-sdk/09_wit_worlds.md` §6. The P7.09 acceptance
criterion "component tooling validates the packages" is therefore met only
when either (a) a human runs the prepared command and records the output, or
(b) P7.27 wires it into CI and a green run exists.

## 1. Package inventory

| Package | Path | World | Mirrors (Rust contract) | Owner subtask |
|---|---|---|---|---|
| `flexcore:core@1.0.0` | `deps/flexcore-core@1.0.0/types.wit` | — (shared data vocabulary, no world) | `flexcore-contracts/src/{ids,quality,units,time_model}.rs`; `flexcore-ports/src/telemetry_source.rs` (quantity/value/record types) | P7.09 |
| `flexcore:optimizer@1.0.0` | `flexcore-optimizer@1.0.0/optimizer.wit` | `optimizer-plugin` (exports `optimizer`) | `flexcore-optimization-api/src/port.rs::Optimizer` (+ `problem.rs`, `config.rs`, `result.rs`) | P7.09 |
| `flexcore:grid@1.0.0` | `flexcore-grid@1.0.0/grid.wit` | `grid-model-provider-plugin` (exports `grid-model-provider`) | `flexcore-grid-api/src/port.rs::GridModelProvider` (+ `scope.rs`) | P7.09 |
| `flexcore:telemetry@1.0.0` | `flexcore-telemetry@1.0.0/telemetry.wit` | `telemetry-source-plugin` (exports `telemetry-source`) | `flexcore-ports/src/telemetry_source.rs::TelemetrySource` (+ `backend_error.rs`) | P7.09 |

Every in-scope contract of `02_scope_freeze.md` §2 has a world above, with
one deliberate exception recorded in §4 below.

### Directory-naming note (Windows-safe)

WIT package directories conventionally carry the package name
(`flexcore:core@1.0.0`), but `:` is illegal in NTFS file and directory names
and this repository is developed on Windows. The directory names therefore
use the hyphen rendering (`deps/flexcore-core@1.0.0/`); the `package`
declaration inside each `.wit` file is authoritative for the package
identity, and the mapping table above is the directory → package record.
Tooling invocations in `TOOLCHAIN.md` use the directory names as they exist.

## 2. Versioning rules (SemVer for worlds)

- Every package is declared with an exact version
  (`package flexcore:<name>@X.Y.Z;`). The world version equals its package
  version — worlds do not carry their own version numbers.
- **Breaking change ⇒ major bump.** Breaking means: removing or renaming any
  type, field, variant case, flag, enum case, or function; changing any
  parameter, result, or error type; changing a variant case payload; adding
  a required function to an exported interface (a consumer cannot ignore
  it). A major bump means a NEW package version directory
  (`flexcore:optimizer@2.0.0/`) alongside the old one, never an edit of the
  released one.
- **Additive changes** (new optional field, new variant case, new function on
  an exported interface, new type) may bump the minor version. Every
  additive change is RECORDED — a changelog entry in this README §5, never a
  silent edit.
- Compatibility policy: a plugin manifest declares a `flexcoreApi` SemVer
  range (P7.05); the host resolves it against the world versions above.
  `wit-bindgen` compatibility checks (a breaking WIT change without a
  version bump must fail) are a **P7.27 CI gate**, not a local manual step.
- Design-for-host note (P7.10–P7.12): the host instantiates a component and
  binds ONLY what the component's world declares — no ambient filesystem,
  network, database, or secret access (report §5.4). Worlds therefore carry
  zero `wasi:` imports; the capability broker (P7.11), not the WIT world, is
  the grant mechanism for any future host-provided interface.

## 3. Mapping tables — world ↔ Rust trait (no contract invented)

Rule: every WIT function maps to a real Rust trait method; every WIT type
maps to a named Rust type. Nothing below is invented. Each `*.wit` file
carries the traceability header naming the Rust file and trait it mirrors.

### 3.1 `flexcore:optimizer@1.0.0` — interface `optimizer` ↔ `Optimizer`

Rust source: `crates/flexcore-optimization-api/src/port.rs::Optimizer`
(trait), verified 2026-11-13.

| WIT function | Rust trait method | Notes |
|---|---|---|
| `capabilities: func() -> solver-capabilities` | `Optimizer::capabilities(&self) -> SolverCapabilities` | 1:1 |
| `solve: func(problem: optimization-problem, config: solver-config) -> result<optimization-result, optimizer-error>` | `Optimizer::solve(&self, problem: &OptimizationProblem, config: &SolverConfig) -> Result<OptimizationResult, OptimizerError>` | 1:1; `&T` params → by-value WIT params (boundary copy) |

| WIT type | Rust type | Translation decisions |
|---|---|---|
| `problem-class` | `problem.rs::ProblemClass` | kebab-case renames only (`convex_qp` → `convex-quadratic-program`) |
| `variable-kind` | `problem.rs::VariableKind` | 1:1 |
| `variable` | `problem.rs::Variable` | 1:1; `unit` is `flexcore:core/canonical-unit` |
| `coefficient` | `problem.rs::Coefficient` | **`usize` → `u32`** (index domain narrowing; payload limits apply) |
| `row-bounds`, `range-bounds` | `problem.rs::RowBounds` | enum-with-payload → WIT `variant` |
| `linear-row`, `quadratic-term` | `problem.rs::LinearRow`, `QuadraticTerm` | 1:1 |
| `objective`, `objective-sense` | `problem.rs::Objective`, `ObjectiveSense` | 1:1 |
| `second-order-cone` | `problem.rs::SecondOrderCone` | 1:1 |
| `problem-provenance` | `problem.rs::ProblemProvenance` | `Option<OffsetDateTime>` → `option<utc-timestamp>` |
| `optimization-problem` | `problem.rs::OptimizationProblem` | 1:1 |
| `scaling-policy` | `config.rs::ScalingPolicy` | 1:1 |
| `solver-config` | `config.rs::SolverConfig` | `Option<Duration>` → `option<u64>` milliseconds |
| `constraint-feature` | `port.rs::ConstraintFeature` | **`Vec<ConstraintFeature>` → WIT `flags`** (set representation) |
| `warm-start-support`, `license-declaration`, `determinism-declaration`, `diagnostics-declaration`, `tolerances-declaration`, `scaling-declaration`, `solver-capabilities` | `port.rs::{WarmStartSupport, LicenseDeclaration, DeterminismDeclaration, DiagnosticsDeclaration, TolerancesDeclaration, ScalingDeclaration, SolverCapabilities}` | 1:1 |
| `solve-status` | `result.rs::SolveStatus` | exactly the six §12.6 statuses |
| `variable-value` | `result.rs::VariableValue` | 1:1 |
| `primal-diagnostics`, `dual-diagnostics`, `residual-report`, `infeasibility-evidence` | `result.rs::{PrimalDiagnostics, DualDiagnostics, ResidualReport, InfeasibilityEvidence}` | payload records, exact names |
| `primal-diagnostic`, `dual-diagnostic`, `residuals-diagnostic`, `infeasibility-evidence-diagnostic` | `result.rs::Diagnostics<T>` | **WIT has no user-defined generics** — one concrete variant per payload |
| `determinism-record` | `result.rs::DeterminismRecord` | the optimizer instantiation of §9.2 reproducibility metadata |
| `optimization-result` | `result.rs::OptimizationResult` | **`Duration` → `u64` nanoseconds** (`runtime-nanos`) |
| `optimizer-error` | `port.rs::OptimizerError` | **nested typed errors flattened to tag + Display string** (the four tags are the Rust four) |

### 3.2 `flexcore:grid@1.0.0` — interface `grid-model-provider` ↔ `GridModelProvider`

Rust source: `crates/flexcore-grid-api/src/port.rs::GridModelProvider`
(trait), verified 2026-11-13.

| WIT function | Rust trait method | Notes |
|---|---|---|
| `capabilities: func() -> grid-provider-capabilities` | `GridModelProvider::capabilities(&self) -> GridProviderCapabilities` | 1:1 |
| `run-power-flow: func(model: versioned-network-model, injections: list<candidate-injection>) -> result<power-flow-result, grid-port-error>` | `run_power_flow(&self, model: &VersionedNetworkModel, injections: &[CandidateInjection])` | 1:1; AC power flow, NOT AC-OPF |
| `evaluate-dispatch: func(model, injections, limits) -> result<feasibility-verdict, grid-port-error>` | `evaluate_dispatch(&self, model, injections, limits: &NetworkLimits)` | Rust default method = `run_power_flow` + port-level `evaluate_limits`; exposed explicitly in WIT |

Deliberate difference: §9.1's `load_model(model_ref)` has **no Rust
counterpart** (`02_scope_freeze.md` §2) and is **not** a WIT function —
models arrive as validated `VersionedNetworkModel` values. §9.1's
`validate_dispatch` maps to `evaluate_dispatch`.

| WIT type | Rust type | Translation decisions |
|---|---|---|
| `phase-scope`, `topology-scope`, `equipment-kind`, `not-modelled`, `scope-declaration` | `scope.rs::{PhaseScope, TopologyScope, EquipmentKind, NotModelled, ScopeDeclaration}` | kebab-case renames (`two_winding_transformer` → `two-winding-transformer`) |
| `grid-model-ref`, `bus-spec`, `branch-spec`, `transformer-spec`, `external-grid-spec`, `versioned-network-model` | `port.rs::{GridModelRef, BusSpec, BranchSpec, TransformerSpec, ExternalGridSpec, VersionedNetworkModel}` | 1:1 |
| `candidate-injection`, `applied-injection-record` | `port.rs::{CandidateInjection, AppliedInjectionRecord}` | unit newtypes (`ActivePowerMW`, `ReactivePowerMVAr`) → plain `f64` fields WITH the unit in the field name (`active-power-mw`, `reactive-power-mvar`) and the declared convention carried alongside — no bare uncontextualised float |
| `network-limits` | `port.rs::NetworkLimits` | 1:1 |
| `limit-violation-kind`, `limit-violation` | `port.rs::{LimitViolationKind, LimitViolation}` | 1:1 |
| `feasibility-verdict` | `port.rs::FeasibilityVerdict` | `Violations { violations }` / `Inconclusive { reason }` → payload cases |
| `power-flow-status` | `port.rs::PowerFlowStatus` | closed three-status vocabulary |
| `bus-voltage`, `branch-loading`, `transformer-loading`, `power-flow-residuals`, `coverage-limitation`, `power-flow-result` | `port.rs::{BusVoltage, BranchLoading, TransformerLoading, PowerFlowResiduals, CoverageLimitation, PowerFlowResult}` | 1:1; **`Duration` → `u64` nanoseconds** (`runtime-nanos`) |
| `residuals-diagnostic` | `port.rs::Diagnostic<PowerFlowResiduals>` | concrete variant (no user generics in WIT) |
| `grid-provider-capabilities` | `port.rs::GridProviderCapabilities` | carries the full `scope-declaration` |
| `grid-port-error` | `port.rs::GridPortError` | nested typed errors flattened to tag + Display string (the six tags are the Rust six) |

### 3.3 `flexcore:telemetry@1.0.0` — interface `telemetry-source` ↔ `TelemetrySource`

Rust source: `crates/flexcore-ports/src/telemetry_source.rs::TelemetrySource`
(trait), verified 2026-11-13.

| WIT function | Rust trait method | Notes |
|---|---|---|
| `provider-declaration: func() -> provider-declaration` | `TelemetrySource::provider_declaration(&self) -> &ProviderDeclaration` | 1:1 |
| `subscribe: func(query: telemetry-query) -> result<telemetry-subscription, telemetry-source-error>` | `subscribe(&self, query: &TelemetryQuery)` | 1:1 |
| `decode: func(subscription: telemetry-subscription, payload: list<u8>) -> result<list<normalized-telemetry>, telemetry-source-error>` | `decode(&self, subscription: &TelemetrySubscription, payload: &[u8])` | `&[u8]` → `list<u8>` |
| `source-id: func() -> string` | `TelemetrySource::source_id(&self) -> &str` | 1:1 |
| `health: func() -> telemetry-source-health` | `TelemetrySource::health(&self)` | 1:1 |

| WIT type | Rust type | Translation decisions |
|---|---|---|
| `telemetry-query` | `telemetry_source.rs::TelemetryQuery` | Rust derives the tenant structurally from a `TenantContext` (private fields, no alternative constructor); WIT records are open, so `tenant-id` is an explicit field **set and validated by the host** from the authenticated session (P7.11/P7.39) |
| `telemetry-subscription` | `telemetry_source.rs::TelemetrySubscription` | newtype `u64` → type alias |
| `source-state-mapping`, `provider-declaration` | `telemetry_source.rs::{SourceStateMapping, ProviderDeclaration}` | `Duration` → `u64` nanoseconds (`cadence-nanos`); `QualityMappingDeclaration` → `list<source-state-mapping>` (its validation rules travel as comments; the host re-validates) |
| `telemetry-source-health` | `telemetry_source.rs::TelemetrySourceHealth` | `Degraded { reason }` / `Unavailable { reason }` → payload strings |
| `backend-error` | `backend_error.rs::BackendError` | 1:1 |
| `telemetry-source-error` | `telemetry_source.rs::TelemetrySourceError` | 1:1 (the `Backend(e)` case carries the `backend-error` record) |

Shared vocabulary (`flexcore:core@1.0.0`) types — `flex-uuid`, `utc-timestamp`,
`clock-quality`, `canonical-unit`, `reactive-sign-convention`,
`telemetry-quality`, `telemetry-quantity`, `quantity-value`,
`reactive-power-value`, `normalized-telemetry`, `reproducibility-metadata` —
mirror `flexcore-contracts/src/{ids,quality,units,time_model}.rs` and
`telemetry_source.rs::{TelemetryQuantity, QuantityValue, NormalizedTelemetry}`;
the per-file header comments name the exact sources.

Naming rule across all tables: Rust SCREAMING_SNAKE_CASE serialisation
(e.g. `GOOD`, `MISSING`) ↔ WIT kebab-case (`good`, `missing`) — value
spellings on the wire are the WIT kebab forms; the mapping is 1:1.

## 4. Deliberate exclusions (recorded, not forgotten)

| Excluded item | Reason | Where it returns |
|---|---|---|
| `AssetAdapter`, `ForecastProvider`, `MarketAdapter` worlds | DEFERRED — no Rust contract exists (`02_scope_freeze.md` §2); publishing a WIT world for a non-existent Rust trait is the recorded P7.01 fiction risk | When the Rust ports exist |
| `EmsAdapter` (edge adapter port) world | IN-scope per `02_scope_freeze.md` §2, but the Rust trait is "written, UNBUILT" (`04_seam_inventory.md` §3.10) and sits behind the P6.29 command-path boundary; not written in this wave | A follow-up P7.09 wave translating `crates/edge-agent/src/adapters/port.rs` once the seam's build status is settled |
| Any `CommandAuthorizer` / `DispatchGateway` / authorization WIT | Protected kernel — NEVER a plugin contract (`02_scope_freeze.md` §4; report §5.1, §5.7) | Never |
| WASI 0.3 syntax (`stream<T>`, `future<T>`, `async`) | WASI 0.2 baseline (RESEARCH.md R7-C1; report §5.4) | P7.32 re-evaluation |

## 5. First-validation checklist (for the human `wasm-tools` run)

The file was authored conservatively, but the following constructs are
flagged because they could not be tool-checked. On a failure, fix the
construct, not the contract — every function and type must keep its Rust
mirror.

1. `use` path spelling: `use flexcore:core/types@1.0.0.{ ... };` — confirm
   the exact version-in-use-path form for the pinned `wasm-tools` (expected
   `interface@version` inside the package path, as in WASI 0.2's own
   `use wasi:io/streams@0.2.0.{...}`; if the tool wants the version on the
   package instead, move it — the referenced items are unchanged).
2. `///` doc comments before `interface` items (used in
   `optimizer.wit`, `grid.wit`, `telemetry.wit`): if the pinned toolchain
   rejects them, convert to `//` — comment text unchanged.
3. `flags` for `constraint-feature` (a set-valued declaration where Rust
   carries a `Vec`): confirm `flags` semantics are acceptable at the
   host-binding level.
4. Multi-line `use ....{ a, b, c };` lists across lines: if the parser is
   strict about the brace position, reformat without changing the items.
5. `world` / `interface` name pairs (`optimizer-plugin` world vs `optimizer`
   interface): confirm they do not collide in the package namespace (the
   `-plugin` suffix was chosen to keep them distinct; if worlds and
   interfaces share a namespace, the suffix is what keeps it valid).
6. `option<f64>` for `None`-able bounds/values where Rust uses
   `Option<f64>` — confirm no tool-level NaN/infinity defaulting is
   introduced by bindings (WIT f64 is not NaN-free; the Rust newtypes and
   validation are the guards, and the host re-validates).
7. Package-directory naming (`deps/flexcore-core@1.0.0/` etc., §1 note):
   confirm `wasm-tools` resolves the packages from these directory names
   (the package identity comes from the `package` declaration, not the
   directory).
8. Confirmation that the whole tree parses and resolves with ZERO `wasi:`
   imports (the least-privilege claim in every file header).

## 6. Changelog (additive changes are recorded here)

| Date | Change | Version |
|---|---|---|
| 2026-11-13 | Initial packages written (P7.09): `flexcore:core@1.0.0`, `flexcore:optimizer@1.0.0`, `flexcore:grid@1.0.0`, `flexcore:telemetry@1.0.0`. WRITTEN-UNVALIDATED. | 1.0.0 |

## 7. Related records

- `ledger/phase-07-plugin-sdk/09_wit_worlds.md` — the P7.09 decision record.
- `ledger/phase-07-plugin-sdk/40_toolchain_pinning.md` — toolchain pins and
  the verbatim human verification steps.
- `TOOLCHAIN.md` — the same pins, kept next to the files the tooling
  consumes.
- `ledger/phase-07-plugin-sdk/04_seam_inventory.md` — the contract →
  provider → consumer map these worlds mirror.
- `ledger/phase-07-plugin-sdk/02_scope_freeze.md` — the scope freeze this
  package set implements (§2 IN rows).
