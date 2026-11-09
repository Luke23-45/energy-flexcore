# P0.19 — Plugin model and protected kernel freeze

| Field | Value |
|---|---|
| Subtask | P0.19 — Freeze the plugin model and protected kernel |
| Report trace | §5.1–§5.8 (design rule, capability seam, categories, execution classes, manifest, lifecycle, authority, composition profiles), §9.3 (plugin compatibility and supply chain), §22.1 (ADR-005, ADR-006, ADR-010) |
| Source | `docs/plan/Neryva_FlexCore_Product_and_Architecture_Report_v1.1.md` (product baseline; foundations specification governs on conflict per AGENTS.md §2) |
| External reference status | WASI 0.2 / Component Model baseline; WASI 0.3 not a release blocker at the 7 November 2026 cut-off — per `ledger/phase-00-product-freeze/RESEARCH.md` §4 and report §5.4, [R3], [R4] |
| Inputs | P0.11 (command state machine), P0.17 (threat model), P0.18 (control-authority modes) |
| Status | Draft for Stage 0 review |
| Scope limit | Requirements freeze for later phases (plugin SDK is v0.5.0 work); no safety, certification, or protection-grade claim (AGENTS.md §3). |

---

## 1. Purpose

Freeze the plugin architecture, execution classes, and the non-replaceable protected kernel before any plugin code is written (ledger P0.19). Phase 7 (plugin SDK) tests against this note; Phase 1 kernel boundaries reference it (ledger risk/mitigation note).

## 2. Design rule and protected kernel (§5.1; §22.1 ADR-005)

> **Everything that can safely vary is a plugin. Anything that defines safety, authority, tenant isolation, audit integrity, or command lifecycle is a protected platform invariant.**

- The five protected areas are **safety, authority, tenant isolation, audit integrity, and command lifecycle** — non-replaceable (§5.1; §0.2 core principle: safety invariants, authorization, tenant isolation, command lifecycle, audit, and final dispatch validation remain controlled by the platform kernel).
- Do not make literally every module a plugin; stable core contracts and security invariants must not be replaced by arbitrary extensions (§5.1).
- The architectural ideas (capability seams, provider/consumer, composition profiles) follow the documented DeepSeek Harness architecture; FlexCore adopts the ideas, not its implementation, runtime, or maturity claims (§5.1, [R2]).

## 3. Capability-seam rule (§5.2)

For each extension capability, separate three roles:

1. **Contract / service definition** — stable interface and data vocabulary;
2. **Provider** — concrete implementation (e.g. a pandapower adapter, OpenADR adapter, optimizer, or telemetry source);
3. **Consumer** — domain/application module that depends only on the contract.

Worked example (§5.2): the scheduling module depends on an `Optimizer` contract and **must not import a concrete Clarabel, HiGHS, Pyomo, or custom optimizer implementation directly**.

```text
Scheduling use case
       |
       v
Optimizer capability contract
       |
       +-- HiGHS MILP provider
       +-- Clarabel conic (SOCP) provider
       +-- Python CVXPY provider
       +-- Research optimizer provider
```

Solver-class discipline applies at the seams (P0.13; §7.4): HiGHS is LP/QP/MIP, not SOCP; Clarabel is convex conic (including SOCP), not MISOCP. A provider's declared class travels with its result (§12.6).

## 4. Plugin categories (§5.3 — all 11, with permitted responsibilities)

| Category | Example plugin | Permitted responsibility |
|---|---|---|
| Asset adapter | BESS, PV inverter, EVSE, meter | Map vendor/device data into canonical asset/telemetry contracts |
| Site integration | OpenEMS connector | Integrate with a site-level EMS |
| Telemetry source | MQTT, WebSocket, Modbus, file replay | Ingest telemetry with source and quality metadata |
| Grid model provider | pandapower, OpenDSS, GridLAB-D | Load/build/solve a network model for a declared analysis type |
| Optimization provider | HiGHS, Clarabel, Pyomo, custom solver | Solve a declared optimization problem and return diagnostics |
| Forecast provider | Weather, PV, load, price | Publish versioned forecasts and uncertainty |
| Market/program adapter | OpenADR, region-specific ISO/RTO connector | Translate external market/program semantics into internal events/commitments |
| Protocol adapter | SunSpec Modbus, IEEE 2030.5, OCPP | Serialize/translate approved device/site messages |
| Research algorithm | GNN, DMPC, MARL | Propose candidates, rankings, schedules, or policy outputs for validation |
| Analytics/report | Settlement, performance, carbon, portfolio KPI | Compute derived reporting results |
| UI extension | Fleet chart, plugin-specific workflow panel | Add operator views without owning authoritative state |

No category includes a permit to authorize, issue, or directly command: research algorithms and optimizers propose only (§5.3 research-algorithm row; §5.7; §22.1 ADR-010).

## 5. Execution classes (§5.4)

Do not use one execution mechanism for every plugin (§5.4).

### Class A — Trusted built-in Rust modules

- First-party components compiled into the FlexCore release; implement Rust traits/contracts in the Cargo workspace.
- Statically linked and registered at application composition time; best for core-approved adapters and services requiring low overhead.
- A module remains replaceable at the contract level even when statically linked.

### Class B — Sandboxed WebAssembly components

- Third-party or separately distributed extensions; interfaces defined with WIT packages and versioned worlds.
- Run under Wasmtime with explicit host-provided capabilities: **no ambient filesystem, network, database, or secret access**.
- Memory, fuel/time, concurrency, output-size, and call-rate limits are applied; specific scoped capabilities are granted (e.g. `telemetry.read`, `schedule.propose`, `forecast.publish`).
- **WASI 0.2 / the supported Component Model baseline is the first plugin SDK target.** WASI 0.3 adds async functions, streams, and futures, but the Rust `wasm32-wasip3` target requires nightly before the planned Rust 1.100 release on 12 November 2026; at the 7 November 2026 cut-off, **WASI 0.3 is not a release blocker**. Re-evaluate after stable toolchain and host compatibility mature (§5.4, [R3]; `RESEARCH.md` §4).
- Wasmtime provides Component Model embedding APIs and sandboxing mechanisms, but sandboxing must still be combined with host capability policy, resource limits, signatures, and isolation (§5.4, [R4]).

### Class C — Out-of-process workers

- Separate processes/containers with gRPC/Protobuf, used for: Python power-system calculations; Python optimization stacks; GNN/MARL training and inference; simulators and co-simulation coordinators; and third-party components with large native dependencies.
- Isolates Python dependencies, crashes, CPU/GPU load, and research packages from the Rust control-plane runtime.

### Dylib rejection (§5.4; §22.1 ADR-006)

**No arbitrary Rust dynamic-library (`.so`/`.dll`) plugin loading in the 1.x line** — Rust's native ABI has no stability guarantee (§5.4, [R5]). A C ABI wrapper could be designed, but it adds unsafe FFI, memory ownership, versioning, and crash-isolation problems and is not the default extension mechanism. Use static Rust modules, WASM components, or gRPC workers instead. A dylib loader introduced for convenience contradicts this freeze (ledger risk note).

## 6. Manifest and lifecycle rules (§5.5–§5.6)

- Every plugin includes a **validated manifest** (§5.5): identity (`metadata.id`, version, publisher), runtime kind (`builtin-rust | wasm-component | grpc-worker`), compatibility range, provided and required capabilities, requested permissions, configuration schema, health, and resource limits. The manifest schema must be **versioned, validated, and published with the plugin SDK**; the report example targets the v0.5.0 SDK and is illustrative, not a final schema. Plugin packaging is not part of the v0.1.0 build.
- **Dependency resolution must detect — before activation — missing capabilities, version incompatibility, cycles, and conflicting providers** (§5.5).
- Lifecycle states (§5.6): Discovered → Verified (signature/provenance accepted) → Compatible (manifest/API pass) → Configured (schema and secret references valid) → Started (dependency graph resolved) → Ready (health checks pass) → Draining → Stopped; failure paths lead to Rejected or Failed → Draining.
- **No hot-reload of safety or dispatch providers in the 1.x line** (§5.6).
- **Activation or upgrade must not occur in the middle of an active dispatch transaction**: drain the plugin, stop accepting new work, finish or cancel pending operations, record the version change, then replace it (§5.6).

Supply-chain rules (§9.3) apply to the same lifecycle: SemVer per contract and package; explicit compatibility matrix; SBOMs for releases and plugin packages; signatures/provenance required for production installation; manifest, publisher, version, capabilities, and permissions validated before activation; pinned dependencies with retained rollback artifacts; conformance tests in CI; **a plugin may not self-upgrade or fetch executable code at runtime**.

## 7. Authority model (§5.7 — full permission table)

A plugin **may propose work; it must not grant itself authority** (§5.7).

| Capability | Example permission | Initial policy |
|---|---|---|
| Read telemetry | `telemetry.read` | Scoped to tenant/site/asset |
| Read asset metadata | `asset.read` | Scoped |
| Publish forecast | `forecast.publish` | Allowed after schema validation |
| Propose schedule | `schedule.propose` | Candidate only |
| Propose dispatch | `dispatch.propose` | Candidate only |
| Validate candidate | `dispatch.validate.request` | Request to protected validator |
| Authorize dispatch | `dispatch.authorize` | **Never granted to general plugins** |
| Issue physical command | `dispatch.issue` | **Owned by protected dispatch service and protocol adapter under a signed authorization token** |
| Read secrets | `secret.read` | **Never granted directly; secret references are resolved by host/adapter** |
| Arbitrary network | `network.any` | **Prohibited by default** |

The platform kernel owns final authorization and dispatch state transitions. An optimizer, GNN, MARL policy, or external plugin cannot call device protocols directly (§5.7; §17.5 invariant 7; §22.1 ADR-010: ML/RL outputs are proposals only and cannot directly dispatch). A plugin can never select a control-authority mode (§21.4; P0.18 §3).

## 8. Composition profiles and bundles (§5.8)

Named runtime profiles composed from ordered bundles (§5.8):

| Profile | Components |
|---|---|
| `sim-lab` | Synthetic DERs, feeder simulator, mock market, deterministic optimizer, replay tools, no device writes |
| `vpp-operator` | Asset registry, telemetry, flexibility, scheduler, audit, operator console, selected site connectors |
| `site-edge` | Edge agent, selected protocol adapters, local buffering, outbound identity, limited command receiver |
| `research-gnn` | Grid model provider, GNN worker, benchmark suite, no dispatch authority |
| `research-marl` | Simulator, MARL worker, shadow action logger, safety evaluation, no device writes |

- A profile is an **explicit, reviewable composition**; it must not silently enable a control-capable plugin (§5.8).
- **v0.1.0 ships only the static built-in providers and the `sim-lab` profile**; the `research-gnn` and `research-marl` profiles are not enabled in v0.1.0 (§5.8).
- **The constrained WASM plugin host with third-party composition is deferred to v0.5.0** (§5.8; §2.1 v0.5.0 row).

## 9. ADR alignment (§22.1)

| ADR | Recorded decision | Source sections |
|---|---|---|
| ADR-005 | Plugin model uses capability seams and profiles; protected safety kernel is non-replaceable | §5.1–§5.3, §5.8 |
| ADR-006 | Third-party plugins use WASM/WIT or out-of-process workers; no arbitrary Rust dylib loading | §5.4 |
| ADR-010 | ML/RL outputs are proposals only and cannot directly dispatch | §5.3, §5.7 |

## 10. What this document does not do

- It defines no manifest schema, no WIT world, and no plugin code; those are v0.5.0 SDK deliverables (§5.5, §5.8).
- It selects no solver or grid-model provider; providers are seam implementations chosen in later phases (§3; §12.4).
- It makes no safety, certification, or protection-grade claim; the protected kernel and §17.5 invariants are system requirements, not a certification claim (AGENTS.md §3).

## 11. Acceptance-criteria coverage (ledger P0.19)

| Ledger acceptance criterion | Where satisfied |
|---|---|
| Protected-kernel rule names safety, authority, tenant isolation, audit integrity, and command lifecycle as non-replaceable | §2 |
| All three execution classes and the dylib rejection stated | §5 (A, B, C; dylib rejection in §5 "Dylib rejection") |
| The §5.7 permission table is reproduced, including the never-granted rows | §7 (`dispatch.authorize`, `dispatch.issue`, `secret.read`, `network.any` rows bolded) |
| v0.1.0 stated as static built-in providers + `sim-lab` profile only | §8 |

Additional ledger steps covered: capability-seam rule (§3), all 11 §5.3 categories with permitted responsibilities (§4), WASI 0.2/0.3 status per `RESEARCH.md` §4 (§5 Class B), manifest and lifecycle rules incl. drain-first and no hot-reload (§6), §5.8 profile table incl. WASM deferral to v0.5.0 (§8), §9.3 supply chain (§6), ADR-005/006/010 alignment (§9). Verification method (ledger): comparison against §5.1–§5.8.
