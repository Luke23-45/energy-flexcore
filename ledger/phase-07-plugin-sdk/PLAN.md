# Phase 7 — Plugin SDK and Modular Composition: Plan

| Field | Value |
|---|---|
| Phase | 7 — Stage 7 Plugin SDK and modular composition |
| Report trace | §23 Stage 7; §5; §9; §22.2; §2.1 (v0.5.0) |
| Release | v0.5.0 — Plugin SDK |
| Control status | Simulation / shadow. No plugin can bypass dispatch gates. |

## 1. Scope

### In scope

- Plugin manifests and schema: versioned, validated, published (§5.5).
- Capability seams: the contract → provider → consumer map for every
  in-scope contract, with no consumer importing a concrete provider (§5.2).
- Built-in Rust provider registry (Class A): first-party providers compiled
  in, statically linked, registered at composition time (§5.4).
- Plugin profiles/bundles: named, reviewable compositions (`sim-lab`,
  `vpp-operator`, `site-edge`, `research-gnn`, `research-marl`) that never
  silently enable a control-capable plugin (§5.8).
- Wasmtime/WIT sandbox prototype: a separate-process component host
  (`apps/plugin-host/`) running WASM components behind versioned WIT worlds
  on the WASI 0.2 / Component Model baseline, with a host capability broker
  and resource limits (§5.4 Class B).
- Python gRPC worker template (Class C): out-of-process workers over
  gRPC/Protobuf for Python numerical/ML code (§5.4).
- Plugin conformance suite: the same interface suite for every plugin,
  regardless of class, run in CI (§19.2(3), §9.3).
- Signing/provenance/SBOM, lifecycle with drain-before-replace, config
  schema validation with secret references (never secret values), health/
  readiness, console surfacing, and the §22.2 CI gates extended to the SDK.
- Handover to Phase 8: research profiles and the worker template as the
  research-worker base, under the proposal-only boundary (ADR-010).

### Non-goals (explicitly out of this phase)

- **Arbitrary Rust dynamic-library loading.** Rejected for the 1.x line
  (§5.4, ADR-006). No `.so`/`.dll` plugin loader, including via a C ABI
  wrapper as the default mechanism. This is a prohibition, not a deferral.
- Replacing the protected kernel: authorization, tenant isolation, audit
  integrity, command lifecycle, final dispatch validation (§5.1, §5.7,
  §9.1). No plugin implements `CommandAuthorizer` or touches
  `DispatchGateway`.
- WASI 0.3 as a release requirement. The report (§5.4) says: use WASI 0.2 /
  the supported Component Model baseline for the first plugin SDK; do not
  make WASI 0.3 a release blocker; re-evaluate after stable toolchain and
  host compatibility mature. As of the 7 November 2026 cut-off, Rust 1.100
  (which first stabilizes `wasm32-wasip3`) is scheduled for 12 November 2026
  and is not yet released — see RESEARCH.md. WASI 0.3 migration is tracked
  work for after this phase, not a v0.5.0 deliverable.
- Field dispatch authority for any plugin. v0.5.0 is simulation/shadow;
  ML/RL outputs are proposals only (ADR-010, §13.4).
- Research providers themselves (GNN/DMPC/MARL). Phase 8 builds them on
  this SDK; Phase 7 builds the SDK, the profiles, and the worker template.
- Certified conformance of any plugin. The conformance suite is FlexCore's
  own; it certifies nothing externally.
- A second general-purpose Python backend. Python stays in isolated
  workers (§7.2).

## 2. Architecture decisions in play

| Decision | Report | Position in this phase |
|---|---|---|
| ADR-005 — Capability seams + profiles; protected kernel non-replaceable | §22.1, §5.1 | The SDK's central decision. Everything that can safely vary is a plugin; safety/authority/audit/lifecycle are not. |
| ADR-006 — WASM/WIT or gRPC workers; no arbitrary Rust dylib loading | §22.1, §5.4 | Hard prohibition. Any proposal for a dylib loader is out of scope and must be recorded as rejected, not prototyped. |
| ADR-002 — Rust modular monolith initially | §22.1, §8.3 | Control plane stays a modular monolith; only the edge agent, Python workers, WASM host, and (if needed) background worker are separate processes. |
| ADR-004 — Python isolated to scientific/numerical/research workers | §22.1, §7.2 | The gRPC worker template is the isolation mechanism; it must not become a backend. |
| ADR-007 — PostgreSQL source of truth; NATS JetStream transport | §22.1 | Plugins never own authoritative state and never mutate another module's tables (§6.4). |
| ADR-008 — Protobuf/gRPC internal; REST/OpenAPI external | §22.1, §15.1 | Worker contracts are Protobuf; plugin management is REST/OpenAPI under `/api/v1/plugins`. |
| ADR-009 — Deterministic optimizer + independent feasibility validation is the baseline | §22.1 | The swap test (P7.22) must preserve the baseline: a swapped optimizer still goes through independent feasibility validation. |
| ADR-010 — ML/RL outputs are proposals only, cannot directly dispatch | §22.1, §13.4 | Applies to every research plugin from day one of the SDK; the SDK must make a dispatch-bypassing plugin unbuildable, not merely discouraged. |
| ADR-012 — No hard-real-time or protection claims | §22.1 | Nothing in the SDK changes the timing/protection boundary. |
| D4 — License and dependency audit | §25.2 | P7.03 audits the Wasmtime/WIT/gRPC toolchain explicitly. |
| Report §5.4 WASI note | §5.4, [R3] | WASI 0.2 baseline for v0.5.0; WASI 0.3 tracked, not blocking. |

## 3. Component breakdown

| Component | Location (per §8.1) | Owns | Does not own |
|---|---|---|---|
| Plugin kernel / registry | `crates/flexcore-kernel/` | Composition, registry, lifecycle, permissions, dependency resolution | Domain rules; safety invariants |
| Manifest schema + validator | SDK (`plugins/manifests/`, kernel) | Schema, validation, SemVer ranges | Plugin behaviour |
| WIT packages | `wit/` | Versioned worlds, data vocabulary | Host implementation |
| WASM host | `apps/plugin-host/` (separate process) | Component instantiation, capability broker, resource limits | Control-plane domain logic |
| Built-in providers | `plugins/builtin/` | Asset simulator, HiGHS optimizer, market mock (existing) | Contracts (those live in `flexcore-ports`) |
| gRPC worker template | `python/workers/` (template) | Worker lifecycle, Protobuf contract, health, isolation | Control-plane authority |
| Profiles/bundles | Profile definitions (data) | Ordered composition per profile | Control-authority mode (deployment/site setting) |
| Conformance suite | Test suite (CI) | Contract tests for every plugin class | Protocol-specific conformance (Phase 6, P6.19 — cross-referenced, not duplicated) |
| SDK docs + examples | `docs/plugin-sdk/`, `plugins/wasm-examples/` | Guides, examples, incl. a deliberately bad example | Product promises |

Dependency direction (§6.2, §8.2): domain depends on nothing
infrastructure; application depends on domain + ports; adapters depend on
ports; `flexcore-kernel` manages composition and lifecycle and must not
become a second domain layer; `apps/*` are composition roots.

## 4. Data contracts

| Contract | Form | Notes |
|---|---|---|
| Plugin manifest | YAML (+ JSON Schema) | `apiVersion: flexcore.neryva.io/v1`, `kind: Plugin`, metadata, runtime kind/entrypoint, `flexcoreApi` SemVer range, `provides`/`requires`, permissions requested, config schema ref, health readiness, resource limits (§5.5). Schema is versioned and published; the §5.5 example is illustrative, differences recorded (P7.05). |
| WIT worlds | WIT packages under `wit/` | One versioned world per contract family; data vocabulary carries canonical units, SOC [0,1], quality states, UTC timestamps, and §9.2 provenance/diagnostics fields. Breaking changes bump the world major version. |
| Worker Protobuf | `.proto` under `proto/` | Request/response carrying §9.2 result requirements; `buf lint` + `buf breaking` in CI (§10.5). |
| Capability vocabulary | Enumerated strings + scopes | e.g. `telemetry.read`, `schedule.propose`, `forecast.publish`, each scoped to tenant/site/asset (§5.7). Never-granted: `dispatch.authorize`, `secret.read`, `network.any`, direct device-protocol access. |
| Plugin result (§9.2) | Structured result, all classes | Plugin/model/solver version, input snapshot ID + timestamp, horizon + resolution, problem class + assumptions, termination status, objective + units, residuals, feasibility diagnostics, warnings/unmodelled constraints, execution time/resource use, reproducibility metadata (seed/config/artifact ref). A result without provenance/diagnostics is not eligible for operational consideration. |
| Lifecycle events | NATS events (§15.3) | `PluginActivated`, `PluginFailed`, plus audit records for every transition, version change, and denied capability call. |
| Profile definitions | Data files | Ordered bundles per §5.8; explicit, reviewable; control-authority mode is not a profile property. |

## 5. Interfaces

- **Scheduling → `Optimizer`:** the canonical seam (§5.2). Scheduling
  depends on the contract, never on HiGHS/Clarabel/CVXPY directly.
- **Feasibility → `GridModelProvider`:** `load_model`, `run_power_flow`,
  `validate_dispatch` (§9.1).
- **Telemetry → `TelemetrySource`:** `subscribe`, `decode` (§9.1).
- **Assets → `AssetAdapter`:** `discover`, `read_state`, `capabilities`,
  `propose_setpoint` (§9.1) — note `propose_setpoint` proposes; it does
  not issue.
- **Forecast → `ForecastProvider`; Market → `MarketAdapter`** (§9.1).
- **Protected (not plugins):** `CommandAuthorizer.evaluate`,
  `DispatchGateway.issue` (§9.1). A plugin can request validation
  (`dispatch.validate.request`); it cannot authorize or issue.
- **WASM host ↔ control plane:** gRPC/Protobuf between the separate
  `plugin-host` process and the control plane (§8.3, §15.1).
- **Console ↔ plugin management:** REST `/api/v1/plugins` (§15.2) and the
  Plugins & Integrations workspace (§20.1).

## 6. Sequencing

1. **Readiness + scope (P7.01–P7.04):** verify the seams exist, freeze SDK
   scope, audit licenses, map contract → provider → consumer.
2. **Manifest + resolution + lifecycle (P7.05–P7.07):** schema, resolver,
   state machine with drain-before-replace.
3. **Providers (P7.08–P7.10):** built-in registry, WIT worlds, Wasmtime
   host prototype.
4. **Authority (P7.11–P7.14):** capability broker, permission enforcement,
   sandbox negative tests, signing/provenance/SBOM.
5. **Composition (P7.15–P7.16):** profiles/bundles, safe switching.
6. **Workers (P7.17–P7.18):** gRPC template, supervision.
7. **Operations (P7.19–P7.21):** config/secrets, health/observability,
   conformance suite.
8. **Exit-gate proofs (P7.22–P7.23, P7.28):** swap test, untrusted-plugin
   test, witnessed run.
9. **Docs, console, performance, CI, security, drill, handover,
   retrospective (P7.24–P7.27, P7.29–P7.32).**

Relative effort (no calendar dates): the WIT worlds + Wasmtime host +
capability broker are the bulk of the phase and the highest-risk work
(tooling maturity, sandbox correctness); the manifest/resolver/lifecycle
work is substantial but well-specified by §5.5–§5.6; the swap and
untrusted-plugin proofs are small in code and large in evidence value —
they are the gate, not an afterthought.

## 7. Roles

| Role | Owns |
|---|---|
| Architect | Seam map, scope freeze, profiles, swap test, handover |
| Rust engineer (SDK) | Manifest, resolver, lifecycle, registry, WIT, Wasmtime host, broker |
| Python engineer | gRPC worker template, worker supervision (with Rust engineer) |
| Security engineer | Capability broker review, permission enforcement, sandbox negative tests, signing, security testing |
| Legal + engineering | D4 license audit (Wasmtime/WIT/gRPC stack) |
| Frontend engineer | Plugins & Integrations workspace |
| DevOps | CI gates (incl. WIT/manifest validation, SBOM), signing infrastructure |
| Technical writer | SDK docs and examples |
| Engineering lead | Exit-gate run, retrospective |

## 8. Risks and mitigations

| Risk | Mitigation |
|---|---|
| WIT designed for WASI 0.3 before the toolchain is stable | WASI 0.2 baseline recorded (P7.09); re-evaluation trigger set; RESEARCH.md R7-C1. |
| Wasmtime pinned below the Oct 2026 CVE fixes | Pin ≥50.0.0 (or a fixed backport); RESEARCH.md R7-C2; P7.03/P7.13 enforce it. |
| Manifest permissions decorative, not enforced | P7.12 enforcement matrix with code locations per cell; P7.23 attack proof. |
| A dylib loader proposed as a shortcut | ADR-006 prohibition stated in scope; any such proposal is recorded as rejected. |
| Swap test passes by editing application code | P7.22 requires an empty `git diff` over application/domain code; witness re-runs it. |
| Research profiles inherit dispatch authority | P7.15 explicit no-dispatch property + test; P7.31 handover restates ADR-010. |
| WIT and Protobuf vocabularies drift apart | Same data-vocabulary source (P7.09/P7.17); conformance suite runs the same contract across classes (P7.21). |
| Plugin supply-chain compromise | Signing + SBOM + pinned deps + no self-upgrade/runtime code fetch (P7.14); security testing (P7.29). |
| Sandbox limits configured but never applied | P7.13 hostile components test each limit; P7.26 measures limit-hit behaviour. |

## 9. What must NOT be built yet

- Arbitrary Rust dylib loading (prohibited for the 1.x line).
- A custom authorizer, custom dispatch gateway, or any plugin-replaceable
  safety invariant.
- WASI 0.3 migration as a v0.5.0 deliverable (tracked; re-evaluate after
  Rust 1.100 stable + host compatibility mature).
- GNN/DMPC/MARL providers (Phase 8).
- UI-extension plugins, if deferred in P7.02 — record the deferral, do not
  half-build them.
- Certified-conformance claims for any plugin.
- Field dispatch authority for any plugin.

## 10. How the §24 vertical slice lands here

**Mostly: not in this phase.** The §24 slice lands across v0.1.0–v0.3.0
(Phases 1–5). Phase 7 does not re-build the slice and does not add a new
slice step.

What Phase 7 changes about the slice, and only this:

- Steps 4–5 (optimize a schedule; validate against asset capability and a
  feeder model) can now use a **swapped** optimizer/grid provider through
  the SDK — the P7.22 swap test re-runs the slice scenario with swapped
  providers and requires an empty application/domain diff.
- Step 6's PROPOSED command path is unchanged: a plugin-proposed schedule
  is a candidate that still passes the protected validation → feasibility →
  authorization gates; the SDK adds no shortcut around them.
- Step 9's console gains the Plugins & Integrations workspace (P7.25), so
  the decision/audit trail now also shows which plugin versions produced
  each proposal.
- Step 10's replay must now record plugin versions (and WIT world versions)
  alongside solver/model versions for reproducibility (§9.2, §19.2(5)).

If the slice regresses because of Phase 7 work, the phase has failed its
readiness obligation (P7.01) regardless of what the SDK can do.
