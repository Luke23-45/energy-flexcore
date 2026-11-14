# P7.04 — Capability-seam inventory: contract → provider → consumer map

Date: 2026-11-13. Subtask: P7.04 (`ledger_phase_7.md` §P7.04).
Owner role: Architect. Report trace: §5.2; §9.1; §6.2; §7.4; §8.2.
Inputs: P7.01 (`01_readiness_check.md`); P7.02 (`02_scope_freeze.md`);
P6.29 handover §5 lift-ready table; the source of every crate named
below (grep-verified 2026-11-13).

## 1. Method and honesty status

- **Started from P6.29 §5** (the edge seam table) and **extended to the
  whole platform** by enumerating every `pub trait` in
  `crates/flexcore-ports/src/`, `crates/flexcore-optimization-api/src/`,
  `crates/flexcore-grid-api/src/`, `crates/flexcore-storage/src/`,
  `crates/flexcore-audit/src/`, and the provider/consumer wiring in
  `crates/flexcore-simulator/`, `crates/adapters/`, and `apps/`.
- **No invented names.** Every trait/struct/enum below was grep-verified
  verbatim in the named module on 2026-11-13. Where a report §9.1 name
  has NO Rust counterpart, the row says so (it is a non-seam until the
  contract exists — the P7.01 fiction risk).
- **Nothing was executed.** Per-row state is "written, awaiting the
  human build/test pass" (P6.29 §5 standing caveat; P7.01 G-1). This map
  is the SDK's seam index (ledger step 4) as a *design record*, not a
  runtime-verified registry.

## 2. Class-suitability legend (report §5.4)

| Mark | Meaning |
|---|---|
| A | Suitable for a Class A built-in Rust provider (registry-swappable, P7.08) |
| B | Suitable for a Class B sandboxed WASM component (WIT world, P7.09/P7.10/P7.11) |
| C | Suitable for a Class C out-of-process gRPC worker (P7.17) |
| K | **Kernel-protected / NOT a seam** — no plugin may provide or replace it (§5.1, §5.7, §9.1) |
| N | Not plugin-suitable for structural reasons other than the protected list (e.g. ADR-007 authoritative state) |
| P8 | Phase 8 will extend this seam with research providers (marked per row) |

## 3. Platform seams (contract → providers → consumers)

### 3.1 `Optimizer` — the canonical scheduling seam (§5.2)

| Role | Real names |
|---|---|
| Contract | `Optimizer` — `crates/flexcore-optimization-api/src/port.rs` (`capabilities() -> SolverCapabilities`, `solve(problem, config)`); §9.1 `supported_problem_classes()` maps to `capabilities()`; §9.1 `solve(problem, options)` maps to `solve(problem, config)`. Provider declarations carried by `SolverCapabilities` (`LicenseDeclaration`, `DeterminismDeclaration`, `DiagnosticsDeclaration`, `TolerancesDeclaration`, `ScalingDeclaration`, `WarmStartSupport`, `ConstraintFeature`) |
| Providers (built-in, real) | `HighsTier1Provider` (`src/highs_provider.rs`, `impl Optimizer for`); `ClarabelTier2Provider` (`src/clarabel_provider.rs`, `impl Optimizer for`) |
| Providers (planned) | Class B/C optimizer plugins (v0.5.0); **P8: research optimizer providers** via the gRPC worker template — proposal-only (ADR-010, §13.4) |
| Consumers (real) | `flexcore-application` (scheduling/rolling-horizon use cases: `src/rolling_horizon.rs`, `src/schedule_comparison.rs`, `src/optimization_result_record.rs` consume the crate's port types); `apps/sim-runner` (`src/scenario_run.rs` — composition root) |
| Suitability | **A, B, C** — the report's own example seam (§5.2: HiGHS MILP / Clarabel SOCP / Python CVXPY / research providers). P8 mark applies to the research-provider extension |

### 3.2 `GridModelProvider` — the feasibility seam

| Role | Real names |
|---|---|
| Contract | `GridModelProvider` — `crates/flexcore-grid-api/src/port.rs` (`capabilities() -> GridProviderCapabilities`, `run_power_flow(model, injections)`, `evaluate_dispatch(model, injections, limits)` with the port-level default = `run_power_flow` + `evaluate_limits`). §9.1's `load_model(model_ref)` has NO Rust counterpart — models arrive as validated `VersionedNetworkModel` values; §9.1's `validate_dispatch` maps to `evaluate_dispatch`. The trait doc pins: an AC power-flow evaluation, **NOT an AC-OPF optimization** |
| Providers (built-in, real) | `PandapowerGridProvider` (`src/adapter.rs`; re-exported `crates/flexcore-grid-api/src/lib.rs`), over the `GridModelBackend` seam: `WorkerProcessBackend` (`src/worker_backend.rs`, the P5.15 Python leg) and the in-process `Scripted` double (`src/adapter.rs` test/sim-runner selection) |
| Providers (planned) | OpenDSS / GridLAB-D adapters (recommended-default table; D3 governs fidelity) — **P8: research grid-model providers** |
| Consumers (real) | `flexcore-application` (`src/command_gates.rs`, `src/feasibility_return.rs` — the feasibility gate inside the protected candidate path); `apps/sim-runner` (`src/scenario_run.rs`, `src/feasibility_consumption.rs` — composition root) |
| Suitability | **A** (Class A swap — P7.22 proves it); **B/C deferred** until the WIT vocabulary and D3 are settled (02_scope_freeze §3). P8 mark on the research extension |

### 3.3 `GridModelBackend` — internal grid-crate seam

| Role | Real names |
|---|---|
| Contract | `GridModelBackend` — `crates/flexcore-grid-api/src/backend.rs` (`BackendPowerFlowOutcome`, `BackendInjection`, `BackendRunOptions`, `BackendError`) |
| Providers (real) | `WorkerProcessBackend` (`src/worker_backend.rs`); `Scripted` / `NeverBackend` (in-crate doubles, `src/adapter.rs` tests) |
| Consumers (real) | `PandapowerGridProvider` only |
| Suitability | **A** — an internal seam of the grid crate, not a published plugin surface in v0.5.0; a future external backend arrives through `GridModelProvider`, not this trait |

### 3.4 `TelemetrySource` — the telemetry-ingestion seam

| Role | Real names |
|---|---|
| Contract | `TelemetrySource` — `crates/flexcore-ports/src/telemetry_source.rs` (`provider_declaration`, `subscribe`, `decode`, `source_id`, `health`; data types `TelemetryQuery`, `NormalizedTelemetry`, `TelemetryQuantity`, `QuantityValue`, quality from `TelemetryQuality`). Contract tests: `crates/flexcore-ports/src/telemetry_source_contract.rs` (`run_contract_suite`, `check_provider_declaration`, `check_quality_closure`, …) — the model for the plugin conformance suite (P7.21) |
| Providers (real) | `SimulatorTelemetrySource` (`crates/flexcore-simulator/src/source.rs`, `impl TelemetrySource for`); `FakeTelemetrySource` (`crates/flexcore-ports/src/fakes.rs`, test). Edge note: the edge read path does NOT consume this trait — it uses the edge-local `EmsAdapter` → `AdapterIngestSource` chain (P6.29 §1.1) |
| Providers (planned) | Class B/C telemetry-source plugins (v0.5.0) — **P8: research data sources** |
| Consumers (real) | `apps/worker` (`src/main.rs` holds `Arc<dyn flexcore_ports::telemetry_source::TelemetrySource>` — composition root constructs `SimulatorTelemetrySource`), `apps/worker/src/ingestion.rs`; `apps/sim-runner` (`src/replay.rs`, `src/scenario_run.rs`); `flexcore-simulator` (`src/engine.rs`) |
| Suitability | **A, B, C** — publish a WIT world (02_scope_freeze §2). Quality states must survive the seam unchanged (§10.4; plugins never silently upgrade quality — CONTEXT.md §4). P8 mark |

### 3.5 Telemetry persistence seams (ports, storage adapters)

| Contract (`crates/flexcore-ports/src/`) | Providers (real) | Consumers (real) | Suitability |
|---|---|---|---|
| `TelemetryIngestStore`, `IngestionInbox` (`telemetry_ingest.rs`) | `PgUnitOfWork` / `FlexCoreStore` / `PgIngestionInbox` (`crates/adapters/postgres/src/`); fakes in `flexcore-ports` | `apps/worker` ingestion pipeline; application use cases | **N** — ADR-007: plugins never own authoritative state; a plugin publishes telemetry *through* `TelemetrySource`, never by writing these stores |
| `TelemetryReader` (`telemetry_read.rs`) | `PgUnitOfWork` (adapters/postgres); `FakeTelemetryReader` | application read use cases; `apps/control-plane` (via `apps/control-plane/src/state.rs` port objects) | **N** (same reason); read-side plugins are a Phase 8+ question, not v0.5.0 |

### 3.6 Registry / persistence seams

| Contract (`crates/flexcore-ports/src/repository.rs`) | Provider (real) | Consumers (real) | Suitability |
|---|---|---|---|
| `TenantRepository`, `PortfolioRepository`, `SiteRepository`, `AssetRepository` | `PgUnitOfWork` (`crates/adapters/postgres/src/unit_of_work.rs`, all four `impl` lines verified) | application use cases (`register_tenant.rs`, `create_portfolio.rs`, `create_site.rs`, `register_asset.rs`, …) wired in `apps/control-plane/src/state.rs` | **N** — authoritative registry state (ADR-007); tenant isolation is protected (§5.7). The §5.3 *asset-adapter* category is served by `AssetAdapter`/`EmsAdapter`-style seams, never by replacing these repositories |

### 3.7 Event / outbox seams

| Contract | Provider (real) | Consumers (real) | Suitability |
|---|---|---|---|
| `EventOutbox` (`crates/flexcore-ports/src/event_outbox.rs`, with `OutboxWrite`, `PendingOutboxEntry`) | `PgUnitOfWork`, `FlexCoreStore` (`crates/adapters/postgres/src/store.rs`, `impl EventOutbox` verified) | application use cases (transactional outbox, §6.3); `apps/control-plane` | **N** — plugins consume platform events via the NATS event vocabulary (§15.3); they never write the outbox (02_scope_freeze P-8). The plugin lifecycle events themselves (`PluginActivated`, `PluginFailed`) ride this outbox from the kernel (PLAN.md §4) |

### 3.8 Supporting platform seams (kernel-held services)

| Contract | Providers (real) | Consumers (real) | Suitability |
|---|---|---|---|
| `Clock` (`crates/flexcore-ports/src/clock.rs`) | `SystemClock` (`crates/adapters/postgres/src/clock.rs`), `WallClock` (`crates/edge-agent/src/clock.rs`), `FixedClock` (fakes) | application use cases; edge runner; verifier (expiry checks are fail-closed on clock quality — P6.15) | **K** — time authority is host/platform; a plugin receives time through the host, never AS the platform `Clock` |
| `IdGenerator` (`crates/flexcore-ports/src/id_generator.rs`) | `PostgresIdGenerator` (`crates/adapters/postgres/src/id_generator.rs`) | application use cases | **N** |
| `TenantContextSource` (`crates/flexcore-ports/src/tenant_context_source.rs`) | Postgres-backed tenant context (adapters) | application security paths (`dispatch_security.rs` tenant denials, RLS) | **K** — tenant isolation is protected (§5.7) |
| `AuditSink` (`crates/flexcore-audit/src/sink.rs`) + `AuditEventReader` (`crates/flexcore-storage/src/audit_read.rs`) | `PgUnitOfWork`, `FlexCoreStore` (adapters/postgres); `MemorySink` (test) | application use cases; `apps/control-plane` audit queries | **K** — audit integrity is protected (§5.1). Naming note: the edge agent has a DISTINCT edge-local trait also named `AuditSink` (`crates/edge-agent/src/ports.rs`, impl `FileAuditBuffer`) — two different traits, same name; P6.29 §5 records them separately |
| `SecurityAuditSink` (`crates/flexcore-application/src/dispatch_security.rs`), `DispatchEnvelopeVerifier` (same module) | `PayloadBoundVerifier`, `VerifyingDispatchTransport` (application) | dispatch simulator security path | **K** — command-path security is protected (§14.1, §17.4) |
| `flexcore-storage` record stores: `ScheduleRecordStore`, `ScheduleComparisonStore`, `ScheduleRevisionChainStore`, `EnvelopeRecordStore`, `CommandRecordStore`, `FeasibilityCheckStore`, `EdgeEnrollmentStore` (`crates/flexcore-storage/src/*.rs`) | `PgUnitOfWork` / `Pg*` impls (`crates/adapters/postgres/src/`) | `apps/control-plane/src/state.rs` (all wired as `Arc<dyn …>`); application use cases | **N/K** — authoritative state (ADR-007); `CommandRecordStore` and `FeasibilityCheckStore` carry command-lifecycle and final-dispatch-validation evidence — protected (§5.1) |

### 3.9 Authorization seam — present, protected

| Role | Real names |
|---|---|
| Contract-equivalent | There is NO authorization *trait*. The decision logic is `PolicySet::evaluate` / `evaluate_action_id` (`crates/flexcore-authz/src/policy.rs`) with `Action`, `Scope`, `AuthorizationDecision`, `AllowReason`/`DenyReason` (`src/action.rs`, `src/scope.rs`, `src/decision.rs`), plus Phase 4 policy tables (`src/phase4_policies.rs`) |
| Provider | `flexcore-authz` (the only implementation) |
| Consumers | `apps/control-plane` (`auth.rs`), application gates |
| Suitability | **K** — authorization is protected kernel (§5.1, §5.7). `AuthorizationDecision` is a *type*, not a plugin contract. The §9.1 `CommandAuthorizer` name has no Rust counterpart and never becomes one that plugins can implement |

### 3.10 Edge seams (P6.29 §5, restated with class suitability)

Full detail (methods, version state, § traces) is in P6.29 §1; this
table adds only the Phase 7 class-suitability verdict. All rows are
"written, UNBUILT".

| Seam | Contract (exact type) | Provider(s) | Consumer | Suitability |
|---|---|---|---|---|
| Site EMS adapter | `EmsAdapter` (+ `AdapterCapabilities`, `ChannelDescriptor`, `AdapterHealth`, `AdapterError`, `PointKey`, `EmsNativeReading`) — `adapters/port.rs` | `OpenEmsRestAdapter` (`openems-rest`), `SimulatedEmsSource` (`simulated-test`, TEST-ONLY) | `AdapterIngestSource` → `EdgeRunner` | **A, B, C** — the adapter-plugin seam; plugins inherit the FULL P6.29 §3 command-path boundary; `AdapterRegistry` allowlist extension is a reviewed change |
| Anti-corruption | `AntiCorruptionLayer`, `PointMapping` — `adapters/acl.rs` | the ACL itself (frozen mapping tables) | `AdapterIngestSource` | **N** — a plugin adapter's data passes THROUGH the platform ACL; the ACL is not replaceable (canonical-units guarantee) |
| Edge telemetry source | `EmsSource` — `ports.rs` | `AdapterIngestSource`, `FailClosedEmsSource` | `EdgeRunner` | **N** — internal composition port |
| Uplink | `UplinkChannel` (forward-only) — `ports.rs` | `MtlsUplinkChannel`, `FailClosedUplink` | `EdgeRunner` | **K/N** — outbound identity is the edge's security boundary (§17.3); not a plugin capability |
| Uplink wire format | `BatchPayloadEncoder` — `uplink_mtls.rs` | `NoBatchPayloadEncoder` (REFUSES — OPEN item, P6.30 O-4) | `MtlsUplinkChannel` | **N** — OPEN item; Phase 7 must not assume a wire format exists (P6.29 §2) |
| Local buffer | `LocalBuffer` — `ports.rs` | `FileLocalBuffer`, `InMemoryLocalBuffer` | `EdgeRunner` | **N** |
| Edge audit | `AuditSink` (edge-local), `EdgeAuditEvent`, `EdgeAuditKind` — `ports.rs` | `FileAuditBuffer`, `InMemoryAuditSink` | composition root; command path | **K** — audit integrity (§5.1) |
| Command gate | `CommandPathRuntime`, `CommandReceiver` — `command_path.rs` | the edge agent itself | `GatedCommandPipeline` | **K** — PROTECTED KERNEL (P6.29 §3) |
| Command verification | `CommandVerifier` (+ `ClockQuality` parameter) — `command_verify.rs` | the edge agent itself | `GatedCommandPipeline` | **K** |
| Command forwarding | `CommandForwarder` — `command_forward.rs` | `FailClosedCommandForwarder` only | `CommandForwardingEngine` | **K** |
| Site policy | `CommandPolicyGate` — `policy.rs` | `SitePolicyEngine` | `CommandReceiver`, `CommandForwardingEngine` | **K** — plugin-visible policy configuration does not exist (P6.29 §2) |
| Health | `HealthSnapshot`, `AdapterHealthMonitor` — `health.rs` | `AdapterHealthMonitor` | P6.25 console (display-only) | **N** (display seam; plugin health uses the P7.20 mechanism) |
| Clock quality | `ClockQualityState` → `ClockQuality` — `clock_quality.rs` | `ClockQualityMonitor` (P6.31 owns status) | `CommandVerifier::verify` | **K** — feeds the protected verifier |
| Enrollment/identity | `EnrollmentClient`, `CredentialIssuer`, `CredentialStore`, `RevocationChecker` — `enrollment.rs` | `TokenEnrollmentAdapter`, `DenyAllCredentialIssuer`, `FilesystemCredentialStore`, `UnconfiguredRevocationChecker` | composition root; server route `POST /api/v1/edge/enroll` | **K/N** — identity is the security boundary (§17.3); CA/token service is a deployment requirement |
| Agent config | `EdgeConfig` + `FLEXCORE_EDGE_*` — `config.rs` | `EdgeConfig::from_env` | composition root | **N** — deployment config, not plugin metadata (P6.29 §2) |
| Conformance harness | `ConformanceHarness`, `ConformanceCase` — `conformance.rs` | suite `flexcore-edge-adapter-conformance-suite-v1` | P7.21 (cross-referenced, not duplicated) | **N** — a testing asset, not a provider seam |

## 4. Solver-class declarations (ledger step 3 — §7.4, AGENTS.md rule 4)

Declarations for every optimizer/analysis provider named in §3. These
must accompany each provider into its WIT world (P7.09) and §9.2 result
metadata; a wrong declaration is a repo-forbidden solver-class
conflation.

| Provider | Declared class | Explicitly NOT | Evidence |
|---|---|---|---|
| `HighsTier1Provider` | LP / QP / MIP (MILP) — Tier 1 | **not SOCP**; no conic capability claimed | `crates/flexcore-optimization-api/src/highs_provider.rs`; `SolverCapabilities` declares `ConstraintFeature`s; AGENTS.md rule 4 |
| `ClarabelTier2Provider` | Convex conic — LP/QP/SOCP (+ SDP/expo/power cones per the crate's pin comment); continuous only | **not MISOCP** — no integer variables | `clarabel = "=0.11.1"` pin comment (`Cargo.toml`: "NO integer variables — never MISOCP (P3.18 discipline)"); `src/clarabel_provider.rs`; `SOCP_EXACTNESS_CAVEAT` |
| Tier 3/4 policy | Tier 3 (MISOCP/MINLP) and Tier 4 (nonlinear AC-OPF) are **rejected at the port** by the boundary guard | — | `TIER_BOUNDARY_POLICY` (`src/tier_guard.rs`); reformulation rule: Tier 3 reformulates to Tier 1 MILP or Tier 2 SOCP or the gap is recorded |
| `PandapowerGridProvider` | Balanced power-flow evaluation over the versioned network model | **not AC-OPF** — the trait doc pins "An AC power-flow evaluation … NOT an AC-OPF optimization" | `crates/flexcore-grid-api/src/port.rs` (`run_power_flow` doc); §7.4; AGENTS.md rule 4 ("AC power flow is not AC-OPF") |
| `WorkerProcessBackend` (Python leg) | Executes the Python power-flow worker the grid crate's §7.4 stack defines; class = the worker's declared analysis type | — | `crates/flexcore-grid-api/src/worker_backend.rs` (P5.15) |
| IPOPT | **Not a workspace dependency.** If ever introduced (Stage 8), it is local, nonlinear NLP — no global-optimality guarantee | — | AGENTS.md rule 4; `deny.toml` EPL note (P6.02) |
| CVXPY (future Class C optimizer) | A modelling layer — capabilities depend on the selected solver; never a class declaration of its own | — | §5.2 example; §7.4 |

Tolerances/diagnostics: every provider carries them via
`TolerancesDeclaration` and `DiagnosticsDeclaration` in
`SolverCapabilities` (§7.4 contract; verified in `src/port.rs`) — the
"provider whose declared problem class is wrong" failure mode from the
ledger is checked against this table.

## 5. Direct-import audit (ledger step 2)

Grep over `crates/` and `apps/` for imports of concrete provider types:

| Finding | Verdict |
|---|---|
| `flexcore-application` → `flexcore_optimization_api` (port types; the Optimizer is held as `dyn Optimizer` in composition, never constructed in the application) | Clean — recorded allowed edge (`tools/check_dependency_rule.py` `ALLOWED_INTERNAL_EDGES["flexcore-application"]`; passes, `01_readiness_check.md` §8) |
| `flexcore-application/src/optimization_result_record.rs` → `clarabel_provider::SOCP_EXACTNESS_CAVEAT` (a documentation constant from a *provider module*) | Borderline, recorded as P7.01 G-2: no provider type, no construction, no behavioural dependency; re-check at first build |
| `apps/sim-runner/src/scenario_run.rs` constructs `PandapowerGridProvider`; `apps/worker/src/main.rs` constructs `SimulatorTelemetrySource` | Clean — `apps/*` are composition roots (§6.2/§8.2); construction belongs there |
| Provider construction inside the provider's own crate (`flexcore-optimization-api` tests/fixtures, `flexcore-grid-api` adapter) | Clean |
| Application/domain code importing `clarabel`, `highs`, `pandapower`, or any solver/infrastructure crate | ZERO hits — the domain depends on nothing infrastructure (§6.2) |

Zero behavioural consumer→provider imports found. This audit is
inspection-based; the build-graph query and grep re-run (the ledger's
verification method) is repeated at the first build.

## 6. The non-seams (protected kernel and open items)

### 6.1 Protected-kernel non-seams (§5.1, §5.7, §9.1 — non-replaceable)

| Non-seam | What exists instead (real names) | Rule |
|---|---|---|
| `CommandAuthorizer` (§9.1 protected core) | NO Rust type exists (`01_readiness_check.md` G-3); authorization logic is `flexcore-authz::PolicySet::evaluate` | A plugin never implements, overrides, or calls it as a capability; `dispatch.authorize` is never granted (§5.7) |
| `DispatchGateway` (§9.1 protected core-owned) | NO Rust type exists; dispatch issue path lives in the application/dispatch security modules (`dispatch_security.rs`, simulator dispatch envelope) | A plugin never gains direct access; `dispatch.issue` is owned by the protected dispatch service (§5.7) |
| Tenant isolation | `TenantContextSource`, tenant-scoped queries, `TenantDenialAuditRecord` (`dispatch_security.rs`), Postgres RLS | Every capability is tenant/site/asset-scoped; plugins never mutate another module's tables (§6.4) |
| Audit integrity | `flexcore-audit::AuditSink`, `AuditRecord` (append-only), edge `FileAuditBuffer` (fail-closed upload) | Plugins emit events through the host; they never become the audit sink |
| Command lifecycle | `flexcore-domain::command_lifecycle` / `command_state` (14 states, frozen edges); `flexcore-application::command_gates` | Validation, feasibility, and authorization are distinct gates; plugins enter candidates, never states |
| Final dispatch validation | `GridModelProvider::evaluate_dispatch` inside `flexcore-application::command_gates` / `feasibility_return` (the protected path); `dispatch.validate.request` forwards a *request* only (§5.7) | The verdict never returns to the plugin as authorization |
| P6.14 edge command-path gate | `CommandPathRuntime` 4-condition gate + signed `flexcore-edge-command-enablement-v1` record (P6.29 §3) | The never-granted capability set implicitly includes the gate; enablement is structurally immutable from shared references |

### 6.2 Open non-seams carried from P6.29 §2 (Phase 7 owns defining these)

| Non-seam | State | Phase 7 owner |
|---|---|---|
| WIT interfaces for adapters | **NONE** — no `wit/` package exists anywhere | P7.09 (translating the §3.10 Rust traits; record where a trait deliberately does not translate) |
| Plugin manifest fields | **NONE** — no manifest schema exists | P7.05 |
| Uplink wire format | **OPEN** — `NoBatchPayloadEncoder` refuses | Wire-format owner; Phase 7 inherits the open item if plugin-visible payloads are affected |
| Plugin-visible capability vocabulary / broker | **NONE** — `telemetry.read`/`schedule.propose`/… exist only in the report (§5.7) | P7.11 |
| Plugin capability surface for the command path | **NONE, deliberately** | Phase 7 keeps it negative (§6.1, last row) |

## 7. Phase 8 extension marks (summary)

Seams P8 will extend with research providers, all under the
proposal-only boundary (ADR-010, §13.4) and all via the Class C worker
template (or Class B components where the profile allows):

- `Optimizer` (§3.1) — GNN/DMPC/MARL optimizers → `research-gnn` /
  `research-marl` profiles.
- `GridModelProvider` (§3.2) — research grid-model providers.
- `TelemetrySource` (§3.4) — research data sources.
- The deferred §9.1 contracts (`AssetAdapter`, `ForecastProvider`,
  `MarketAdapter`) when their Rust contracts exist
  (`02_scope_freeze.md` §2).

None of the §6 non-seams is ever extended by Phase 8; the protected
kernel has no Phase 8 provider.

## 8. Acceptance check (ledger P7.04)

- Every in-scope contract (02_scope_freeze §2 IN rows) has ≥1 provider
  and ≥1 consumer listed with real names: YES (§3.1–§3.4, §3.10).
- Zero direct consumer→provider imports found: YES, by inspection (§5),
  with the one borderline constant import recorded (G-2).
- Solver-class declarations per provider, checked against §7.4 and
  AGENTS.md rule 4: YES (§4).
- Protected-kernel exclusions named with the §5.7/§5.1/§9.1 citations:
  YES (§6.1).
- P6.29 non-seams carried and owned: YES (§6.2).
