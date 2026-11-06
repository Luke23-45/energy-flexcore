# Phase 7 — Plugin SDK and Modular Composition: Context

| Field | Value |
|---|---|
| Phase | 7 — Stage 7 Plugin SDK and modular composition |
| Report trace | §23 Stage 7; §5; §9; §22.2; §2.1 (v0.5.0) |
| Release | v0.5.0 — Plugin SDK |
| Control status | Simulation / shadow. No plugin can bypass dispatch gates. |

## 1. Background

FlexCore is a plugin-oriented modular monolith (§3.1, §6.1): a small core
with capability interfaces separated from concrete providers, and ordered
bundles/profiles that compose a runtime. The idea is adopted from the
DeepSeek Harness architecture ([R2]) — the ideas, not the implementation,
runtime, or maturity claims; that project is in developer preview and warns
of breaking changes (§5.1).

The design rule (§5.1) governs this whole phase:

> **Everything that can safely vary is a plugin. Anything that defines
> safety, authority, tenant isolation, audit integrity, or command lifecycle
> is a protected platform invariant.**

Do not make literally every module a plugin. Stable core contracts and
security invariants must not be replaced by arbitrary extensions.

Through Phases 1–6 the seams existed only as Rust traits with built-in
providers (v0.1.0 shipped only the static built-in providers and the
`sim-lab` profile — §5.8). Phase 7 turns those seams into a published SDK:
versioned manifests, WIT worlds, a sandboxed Wasmtime host, a Python gRPC
worker template, profiles/bundles, and a conformance suite — so that a
provider can be swapped without changing application or domain code, and so
that an untrusted plugin cannot access capabilities it was not granted.
That is the §23 Stage 7 exit gate, in two halves.

The three execution classes (§5.4) must not be collapsed into one mechanism:

- **Class A — Trusted built-in Rust modules.** First-party, compiled into
  the release, statically linked, registered at composition time. Best for
  core-approved adapters and low-overhead services. Replaceable at the
  contract level even though statically linked.
- **Class B — Sandboxed WebAssembly components.** Third-party or separately
  distributed extensions. WIT packages and versioned worlds; Wasmtime host
  with explicit host-provided capabilities; no ambient filesystem, network,
  database, or secret access; memory, fuel/time, concurrency, output-size,
  and call-rate limits.
- **Class C — Out-of-process workers.** Separate processes/containers with
  gRPC/Protobuf for Python power-system calculations, optimization stacks,
  GNN/MARL training and inference, simulators, and third-party components
  with large native dependencies. This isolates Python dependencies,
  crashes, CPU/GPU load, and research packages from the Rust control plane.

Explicitly rejected for the 1.x line: arbitrary Rust dynamic libraries
(§5.4). Rust's native ABI has no stability guarantee ([R5]); a C ABI wrapper
adds unsafe FFI, memory ownership, versioning, and crash-isolation problems
and should not be the default extension mechanism.

## 2. Key report sections to re-read

| Section | Why it matters here |
|---|---|
| §5.1–§5.8 | The whole plugin architecture: design intent, three-role seam, categories, execution classes, manifest, lifecycle, authority model, profiles/bundles. |
| §6.1–§6.5 | DDD + hexagonal + modular monolith; dependency rule; ownership/transactional boundaries; patterns (incl. what to avoid: service locator, global mutable registries, plugins mutating another module's tables); storage strategy. |
| §7.2–§7.4 | Language allocation (Python is workers only); Rust stack (Wasmtime, Tonic/Prost, SQLx, NATS); Python stack and the solver selection rule. |
| §8.1–§8.3 | Target tree (`wit/`, `proto/`, `crates/flexcore-kernel/`, `apps/plugin-host/`, `plugins/builtin/`, `plugins/wasm-examples/`, `plugins/manifests/`, `python/workers/`); dependency rules; extraction policy (WASM host is a separate process for security isolation). |
| §9.1–§9.3 | Core interfaces; plugin result requirements (§9.2 — provenance/diagnostics are eligibility conditions, not nice-to-haves); compatibility and supply chain. |
| §10.2, §10.5 | Units/signs; Protobuf and schema governance (`buf lint`, `buf breaking`). |
| §13.4 | Research plugin boundary: proposals only, never an authorization token. |
| §14.1–§14.3 | Command lifecycle and invariants (what plugins cannot touch). |
| §15.1–§15.5 | API styles (REST, gRPC, NATS, WIT); `/api/v1/plugins`; event vocabulary; at-least-once + idempotency; outbox. |
| §17.3–§17.5 | Required controls (incl. plugin signature/provenance, SBOM, WASM limits); threat model (compromised plugin, supply chain); safety invariants. |
| §19.2(3), §19.4 | Contract tests; evaluation metrics (incl. platform metrics). |
| §20.1, §20.2 | Plugins & Integrations workspace; display principles. |
| §21.4 | Build form (WASM Plugin Host OCI image, enabled from v0.5.0) and control-authority modes (plugins cannot select mode). |
| §22.1 (ADR-005, ADR-006, ADR-010), §22.2 | ADRs; CI gates (incl. manifest and WIT compatibility validation). |
| §23 Stage 7 | Deliverables and exit gate (quoted in CHECKLIST.md). |
| §24 | Vertical slice — mostly not in this phase (see PLAN.md §10). |
| §25.1–§25.2 | Provisional choices (Wasmtime + WIT; revisit trigger: component tooling not stable enough for v0.5.0) and deferred decisions (D4 license audit). |
| §27 [R2]–[R5], [R17] | DeepSeek Harness; WASI/Component Model; Wasmtime security; Rust ABI; Buf breaking-change detection. |

## 3. Canonical units and sign conventions (§10.2)

- MW, MVAr, MVA, MWh in persisted values and API contracts; UI may display
  kW/kWh.
- `P > 0` = export/injection into the grid at the point of connection;
  `P < 0` = import/consumption.
- Reactive-power sign is declared per grid model and device profile, never
  assumed; each adapter records the mapping it applies.
- SOC is a fraction in [0,1] at the API boundary.
- WIT and Protobuf types must carry these conventions: a bare float with no
  unit, sign convention, or timestamp context is a defect in an interface
  definition (P7.09), not a minor omission.

## 4. Telemetry quality (§10.4)

Exactly one per value: `GOOD` / `SUSPECT` / `BAD` / `MISSING` / `ESTIMATED` /
`STALE`. Plugin results that consume or produce telemetry must preserve the
quality state and never silently upgrade it. A plugin that returns a result
without provenance and diagnostics (§9.2) is not eligible for operational
consideration — the conformance suite enforces this (P7.21).

## 5. Terminology

| Term | Meaning in this phase |
|---|---|
| Capability seam | One contract + its providers + its consumers (§5.2). Example: Scheduling → `Optimizer` contract → HiGHS / Clarabel / CVXPY providers. |
| Contract / provider / consumer | The three roles of a seam. The consumer depends only on the contract. |
| Class A / B / C | Trusted built-in Rust / sandboxed WASM component / out-of-process gRPC worker (§5.4). |
| WIT | WebAssembly Interface Types — the language-neutral IDL for component interfaces. A WIT package defines interfaces; a world defines what a component imports/exports. |
| Component Model | The WebAssembly Component Model: components with typed imports/exports, composability, cross-language linking. |
| WASI | WebAssembly System Interface — capability-based APIs specified in WIT. WASI 0.2 (Preview 2) is the v0.5.0 baseline; WASI 0.3 (Preview 3, native async) is tracked, not required (§5.4). |
| Wasmtime | The Bytecode Alliance runtime; reference implementation of the Component Model. The host embeds it; it provides sandboxing mechanisms that must still be combined with host capability policy, resource limits, signatures, and isolation (§5.4, [R4]). |
| Manifest | A plugin's validated declaration: id, version, runtime, compatibility, capabilities provided/required, permissions requested, config schema, health, resources (§5.5). |
| Profile / bundle | A named runtime composition of ordered bundles (§5.8): `sim-lab`, `vpp-operator`, `site-edge`, `research-gnn`, `research-marl`. A profile is data, reviewable, and must not silently enable a control-capable plugin. |
| Capability broker | Host-side enforcement that a component can call only the scoped capabilities its manifest requests and its profile allows. |
| Candidate only | A plugin may propose a schedule/dispatch; the proposal has no authority until it passes the protected validation → feasibility → authorization path (§5.7, §13.4). |
| Protected kernel | Authorization, tenant isolation, audit integrity, command lifecycle, final dispatch validation — non-replaceable (§5.1). |
| Conformance suite | The contract tests every plugin must pass (§19.2(3)), run in CI. Passing it is not external certification. |

Use "license" (US spelling) and "acknowledgement" (per `.agent/WRITING.md`).
Use "operator-approved control", never "manual control" or
"human-in-the-loop". Use "dispatch command", not "setpoint", for the
lifecycle object. "Validation", "feasibility", and "authorization" are
different gates — do not use them interchangeably.

## 6. Constraints

- **Simulation/shadow in v0.5.0.** No plugin can bypass dispatch gates; no
  plugin selects control-authority mode (§5.7, §21.4).
- **No arbitrary Rust dylib loading** (§5.4, ADR-006) — a prohibition, not
  a deferral.
- **No safety/certification/protection claims** (AGENTS.md rules 2–3).
  The SDK certifies no plugin; a conformance-suite pass is not external
  certification.
- **Solver-class discipline** (AGENTS.md rule 4; §7.4): HiGHS LP/QP/MIP
  not SOCP; Clarabel convex conic not MISOCP; AC power flow ≠ AC-OPF;
  IPOPT local not global. Every optimizer provider must declare its
  problem class (§7.4) and the swap test must respect it.
- **No hallucinated facts** (AGENTS.md rule 1): Wasmtime/WASI/Rust versions
  must trace to primary sources or be labelled ASSUMPTION. See RESEARCH.md.
- **WASI 0.2 baseline for v0.5.0** (§5.4). WASI 0.3 is not a release
  blocker; re-evaluate after stable toolchain and host compatibility
  mature. As of 7 November 2026 Rust 1.100 is not yet released (scheduled
  12 November 2026).
- **Minimal, surgical edits; verify before claiming done** (AGENTS.md
  rules 6–7). Do not edit `docs/foundations/*` or the report in this phase.

## 7. Prior-phase outputs consumed

Phases 1–5 per §23 Stages 1–5 (workspace, API, telemetry, flexibility,
scheduler, feasibility, command lifecycle, console, simulator). Phase 6:
edge agent, OpenEMS integration, one protocol adapter, command path
disabled by default, and the P6.29 handover listing the seams that exist
and those that do not — including the `site-edge` profile input. P7.01
verifies the seams in code before SDK design starts. Prior-phase ledgers
exist in this repo under `ledger/phase-0*/` through
`ledger/phase-06-edge-protocol/`; where a prior ledger's exit-gate evidence
contradicts an assumption in this ledger, P7.01 must be re-run against the
ledger, not only the report.

## 8. D1–D6 relevance

| Gate | Status at ledger writing | Relevance to Phase 7 |
|---|---|---|
| D1 Target jurisdiction/market | OPEN | Bounds market-adapter contracts; does not gate SDK work. |
| D2 First portfolio/service | OPEN | Bounds which providers matter first; does not gate SDK work. |
| D3 Network class/model fidelity | OPEN | A grid-provider swap (P7.22) must record any model-fidelity change; the swap is not a way to silently change D3's answer. |
| **D4 License/dependency audit** | **OPEN** | **Directly in scope: P7.03 audits the Wasmtime/WIT/gRPC toolchain.** |
| D5 First protocol/asset mix | OPEN (before v0.4.0) | Belongs to Phase 6; Phase 7 inherits its outcome through the P6.29 handover. A plugin touching a site adapter inherits the Phase 6 command-path gate. |
| D6 Cyber/OT boundary review | OPEN (before any read-only pilot) | Belongs to Phase 6/Stage 9; the SDK's sandbox and capability model must be consistent with it, not a substitute for it. |

## 9. Open questions

1. Which §5.3 categories are in scope for v0.5.0 and which are deferred?
   (P7.02 freezes this; UI extensions and some research categories are the
   likely deferrals.)
2. What is the manifest schema's exact version string and SemVer range
   syntax? (P7.05; the §5.5 example is illustrative.)
3. What is the conflict-resolution rule when two providers claim the same
   capability? (P7.06 — explicit priority/selection, never silent
   first-wins.)
4. What are the default resource limits per plugin class (memory, fuel/time,
   concurrency, output size, call rate)? (§5.4 requires them; values are
   configuration decisions to be recorded, not assumed.)
5. What is the signing-key management story for plugin packages (who holds
   keys, how revocation works)? (P7.14.)
6. When exactly is WASI 0.3 re-evaluated — which Rust stable release and
   which Wasmtime version are the trigger? (P7.09/P7.32; RESEARCH.md gives
   the current state.)
7. Which optimizer and which grid provider are swapped in P7.22? The
   choice must respect solver classes (§7.4) and model fidelity (D3).
