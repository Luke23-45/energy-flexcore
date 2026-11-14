# P7.02 — SDK scope freeze: contracts, categories, and the non-replaceable kernel

Date: 2026-11-13. Subtask: P7.02 (`ledger_phase_7.md` §P7.02).
Owner role: Architect. Report trace: §5.1–§5.3; §9.1; §22.1
(ADR-005, ADR-006, ADR-010); §22.2. Inputs: P7.01
(`01_readiness_check.md`); report §5.3 category table; §9.1 interface
list; PLAN.md §1/§9.

## 1. What this record is (and is not)

This record **freezes, in writing, the v0.5.0 Plugin SDK scope before any
interface is written** (ledger P7.02 objective). It records decisions the
report and PLAN.md already make (§5.3, §5.4, §9.1, §22.1, PLAN.md §1);
it decides nothing new. It does **not** close or alter any of D1–D6
(`.agent/DECISIONS.md` — all six remain OPEN; D4 is followed through in
`03_sdk_license_audit.md`, D3 constrains the P7.22 grid swap). It does
not change any §22.1 ADR's scope, so **no new ADR is written**; this
record IS the SDK scope note the ledger's step 4 asks for.

Scope-freeze rule this record enforces: every §5.3 category has an
in/deferred/out verdict (§3); every published contract names its owner
and version state (§2); the non-replaceable list appears verbatim and no
published contract contradicts it (§4). Out-of-scope items are recorded
AS PROHIBITIONS, not deferrals (§5) — a prohibition cannot be re-litigated
as a shortcut later.

## 2. The v0.5.0 published contracts (ledger step 1)

Report §9.1 names the initial capability contracts. The SDK publishes a
WIT world (P7.09) only where the underlying Rust contract actually
exists — publishing a WIT interface for a contract that does not exist
in Rust yet is the recorded P7.01 risk ("the SDK then specifies
fiction"). The readiness check (§5 item 5, §6 G-3) verified which §9.1
names exist in code.

| §9.1 contract | Owner crate::module | Rust state (verified 2026-11-13, `01_readiness_check.md` §4) | v0.5.0 SDK verdict |
|---|---|---|---|
| `Optimizer` | `flexcore-optimization-api` (`src/port.rs`) | EXISTS — trait `Optimizer` (`capabilities`, `solve`); §9.1 `supported_problem_classes()` maps to `capabilities()`; providers `HighsTier1Provider`, `ClarabelTier2Provider` | **IN — publish WIT world (P7.09).** The canonical seam (§5.2) |
| `GridModelProvider` | `flexcore-grid-api` (`src/port.rs`) | EXISTS — `capabilities`, `run_power_flow`, `evaluate_dispatch` (default = `run_power_flow` + `evaluate_limits`). §9.1's `load_model(model_ref)` has NO Rust counterpart — models arrive as validated `VersionedNetworkModel` values; `validate_dispatch` maps to `evaluate_dispatch`. The deliberate differences are recorded for P7.09 | **IN — publish WIT world**, translating the REAL trait, not the §9.1 sketch |
| `TelemetrySource` | `flexcore-ports` (`src/telemetry_source.rs`) | EXISTS — `provider_declaration`, `subscribe`, `decode`, `source_id`, `health`; matches §9.1's `subscribe`/`decode` shape | **IN — publish WIT world** |
| `EmsAdapter` (Phase 6 adapter port) | `crates/edge-agent/src/adapters/port.rs` | EXISTS (written, UNBUILT) — `discover`, `read`, `capabilities`, `health`; structurally NO write method (P6.12) | **IN — publish WIT world**, within the P6.29 hard boundary (§4 below): adapter plugins never reach the command path |
| `AssetAdapter` | — | §9.1 name only; **no Rust trait exists** (`discover`/`read_state`/`capabilities`/`propose_setpoint` unimplemented; grep: zero hits) | **DEFERRED until the Rust contract exists.** No WIT world for it in v0.5.0; recorded as the fiction-risk avoidance, not a scope reduction of §9.1 |
| `ForecastProvider` | — | §9.1 name only; no Rust trait (`forecast(request) -> ForecastBundle` unimplemented) | **DEFERRED until the Rust contract exists.** (The P7.10 example component may demonstrate the *shape* against a Phase-7-defined WIT world, but that world is NOT a published `ForecastProvider` contract until the Rust port exists — recorded for P7.09/P7.24.) |
| `MarketAdapter` | — | §9.1 name only; no Rust trait | **DEFERRED until the Rust contract exists.** D1 (target jurisdiction/market, OPEN) bounds it — a market WIT world would pre-empt D1 |
| `CommandAuthorizer` (protected core) | — | §9.1 protected name; **no Rust type, and none may be published** | **NEVER a plugin contract** (§4) |
| `DispatchGateway` (protected core-owned) | — | §9.1 protected name; **no Rust type, and none may be published** | **NEVER a plugin contract** (§4) |

Every published contract's version home: the WIT world major version
(P7.09) plus the `flexcoreApi` SemVer range in the manifest (P7.05,
`flexcore.neryva.io/v1` schema — exact string is P7.05's to record, per
CONTEXT.md open question 2).

## 3. §5.3 plugin categories — verdicts (ledger step 2)

Verbatim category list from report §5.3. Verdicts apply to **v0.5.0**;
"deferred" records WHERE it returns (Phase 8 or later), never a silent
half-build (PLAN.md §9).

| §5.3 category | Example (report) | Verdict for v0.5.0 | Basis |
|---|---|---|---|
| Asset adapter | BESS, PV inverter, EVSE, meter | **DEFERRED** — no `AssetAdapter` Rust contract exists (§2); D5/D2 bound the asset mix | §9.1; D5 OPEN |
| Site integration | OpenEMS connector | **IN (adapter plugins only, read-path)** — via the `EmsAdapter` seam; the OpenEMS connector itself remains the Phase 6 built-in; a plugin adapter is subject to the full P6.29 command-path boundary | §5.3; P6.29 §1.1/§3 |
| Telemetry source | MQTT, WebSocket, Modbus, file replay | **IN** — `TelemetrySource` contract exists; WASM/gRPC telemetry-source plugins are the first Class B/C target | §9.1; PLAN.md §1 |
| Grid model provider | pandapower, OpenDSS, GridLAB-D | **IN (Class A swap) / DEFERRED (third-party Class B/C)** — the P7.22 swap proves Class A replaceability; publishing third-party grid-model plugins waits for the WIT vocabulary (P7.09) and D3 | §5.3; D3 OPEN |
| Optimization provider | HiGHS, Clarabel, Pyomo, custom solver | **IN** — `Optimizer` contract exists; Class B/C optimizer plugins are in scope (proposal-only outputs, §4) | §9.1; §13.4; ADR-010 |
| Forecast provider | Weather, PV, load, price | **DEFERRED (as a published contract)** — no Rust contract (§2); a forecast-shaped WASM example is permitted as a prototype (P7.10) | §9.1; P7.01 fiction risk |
| Market/program adapter | OpenADR, ISO/RTO connector | **OUT for v0.5.0** — no contract; D1 OPEN bounds it entirely | §9.1; D1 OPEN |
| Protocol adapter | SunSpec Modbus, IEEE 2030.5, OCPP | **DEFERRED** — later adapters per §2.3 (P6.30 O-9); a plugin touching a site adapter inherits the Phase 6 command-path gate (P6.14) | P6.30 O-9; ledger header |
| Research algorithm | GNN, DMPC, MARL | **SDK IN / providers OUT** — Phase 7 ships the worker template and `research-gnn`/`research-marl` profile *shells* the SDK defines; the research providers themselves are Phase 8 (PLAN.md §1) under the proposal-only boundary | ADR-010; §13.4; PLAN.md §1 |
| Analytics/report | Settlement, performance, carbon, KPI | **DEFERRED** — no contract; no conformance suite exists for it this phase (the ledger risk: "a category silently supported without a conformance suite") | §5.3; P7.21 scope |
| UI extension | Fleet chart, plugin workflow panel | **OUT for v0.5.0** — the console's Plugins & Integrations workspace (P7.25) displays plugins; it does not host them. Recorded deferral per PLAN.md §9 ("UI-extension plugins, if deferred in P7.02 — record the deferral, do not half-build them") | §5.3; §20.1; PLAN.md §9 |

No category is left without a verdict; no category marked IN lacks a
conformance path (P7.21 runs the same suite across classes for every
published contract).

## 4. The non-replaceable list (ledger step 3 — verbatim, with the two explicit statements)

From report §5.1/§5.7/§9.1 (quoted in the ledger header and PLAN.md §1):

> **Everything that can safely vary is a plugin. Anything that defines
> safety, authority, tenant isolation, audit integrity, or command
> lifecycle is a protected platform invariant.**

Non-replaceable, verbatim names (§9.1, §5.1, §5.7):

1. **`CommandAuthorizer`** — protected core. No plugin implements or
   overrides it.
2. **`DispatchGateway`** — protected core-owned invocation. No plugin
   gains direct access to it.
3. **Tenant isolation** — every capability is scoped to
   tenant/site/asset (§5.7); plugins never mutate another module's
   tables (§6.4).
4. **Audit integrity** — append-only audit for commands and approvals
   (§6.1); every lifecycle transition and denied capability call is
   audited (§5.6, §5.7).
5. **Command lifecycle** — the §14.1 state machine and its distinct
   validation/feasibility/authorization gates.
6. **Final dispatch validation** — the protected validator; a plugin may
   *request* validation (`dispatch.validate.request`) but the request
   path never returns a verdict to the plugin and never substitutes for
   the gate (§5.7).

Explicit statements required by the task, recorded verbatim:

- **No plugin implements `CommandAuthorizer`.** The name does not yet
  exist as a Rust type (`01_readiness_check.md` §5 item 5); when the
  protected kernel defines it, it is kernel code — the SDK will never
  publish a WIT world, manifest capability, or conformance suite for it.
- **No plugin touches `DispatchGateway`.** Same treatment.
- **No plugin can enable the P6.14 command-path gate.** The gate's
  4-condition enablement (config flag → signed, time-bounded record →
  site command allowlist → D6 record reference) is the ONLY enablement
  path; its state is structurally immutable from any shared reference
  (P6.29 §3). `dispatch.authorize`, `secret.read`, `network.any`, and
  direct device-protocol access are never granted (§5.7), and the
  never-granted set implicitly includes the P6.14 gate (P6.29 §3).
  A plugin that treats a received validation outcome as authorization
  has crossed the boundary (§5.7; P7.11 step 4).
- Control-authority mode is a deployment/per-site setting; **a plugin —
  and a profile switch — can never select or raise it** (§5.7, §21.4).

## 5. Out of scope — PROHIBITIONS, not deferrals

Each item below is closed for the 1.x line (or permanently, as marked)
unless a reviewed ADR reverses it. None of them may be prototyped "to
evaluate".

| # | Prohibition | Trace |
|---|---|---|
| P-1 | **Arbitrary Rust dynamic-library loading is REJECTED**: no `.so`/`.dll` plugin loader, including via a C ABI wrapper as the default mechanism. Extensions are WASM/WIT components (Class B) or out-of-process gRPC workers (Class C) only. Rust's native ABI has no stability guarantee. A proposal to build one is recorded as rejected, not prototyped (P7.35 makes it a CI gate) | §5.4; ADR-006; [R5]; PLAN.md §1 |
| P-2 | **Protected kernel non-replaceable** — §4's six items; no plugin-replaceable safety invariant, no custom authorizer, no custom dispatch gateway | §5.1, §5.7, §9.1; ADR-005 |
| P-3 | **WASI 0.3 is tracked, not blocking** — v0.5.0 builds on the WASI 0.2 / Component Model baseline; `wasm32-wasip3` is nightly-only until Rust 1.100 (scheduled 12 Nov 2026, not released at the 7 Nov 2026 cut-off); WASI 0.3 migration is not a v0.5.0 deliverable. (This one item is genuinely *tracked work*, per the report's own wording — the prohibition is on making it a blocker or designing for it before the toolchain is stable) | §5.4; [R3]; RESEARCH.md R7-C1/§3 |
| P-4 | **No field dispatch authority for any plugin** — v0.5.0 is simulation/shadow; ML/RL outputs are proposals only and enter the protected candidate path; the SDK must make a dispatch-bypassing plugin UNBUILDABLE, not merely discouraged | ADR-010; §13.4; §21.4 |
| P-5 | **Research providers (GNN/DMPC/MARL) are Phase 8** — Phase 7 builds the SDK, profiles, and worker template they will run on | PLAN.md §1, §9; §5.8 |
| P-6 | **No second general-purpose Python backend** — Python stays in isolated Class C workers; the gRPC worker template must not become a backend | ADR-004; §7.2 |
| P-7 | **No certified conformance** — the conformance suite is FlexCore's own; passing it is not external certification of any plugin or protocol | §19.2(3); P6.19 bounded-claim boundary; AGENTS.md rule 2 |
| P-8 | **Plugins never own authoritative state** — PostgreSQL stays the source of truth; plugins never mutate another module's tables and never write the outbox directly | ADR-007; §6.4 |
| P-9 | **Never-granted capabilities**: `dispatch.authorize`, `secret.read` (references resolved by host/adapter, never handed to the plugin), `network.any` (prohibited by default), direct device-protocol access — no manifest or profile can grant them | §5.7; P6.29 §3 |

## 6. Plugin execution classes supported (recorded for completeness, §5.4)

| Class | Definition (§5.4) | v0.5.0 support |
|---|---|---|
| A — Trusted built-in Rust modules | First-party, compiled in, statically linked, registered at composition time; replaceable at the contract level | **IN** — Class A registry (P7.08); the swap test (P7.22) proves contract-level replaceability |
| B — Sandboxed WebAssembly components | WIT packages + versioned worlds; Wasmtime host with explicit host-provided capabilities; no ambient access; memory/fuel/concurrency/output/call-rate limits; scoped capabilities | **IN** — host prototype (P7.10), capability broker (P7.11), sandbox negative tests (P7.13) |
| C — Out-of-process gRPC workers | Separate processes/containers, gRPC/Protobuf, for Python numerical/ML code and heavy native dependencies | **IN** — worker template (P7.17), supervision (P7.18) |

## 7. Acceptance check (ledger P7.02)

- Every published contract (§2, IN rows) names its owner crate and
  version home: YES (owner crate::module + WIT-major/`flexcoreApi` note).
- Every §5.3 category has an in/deferred/out verdict: YES (§3, 11 rows).
- The non-replaceable list is present verbatim and no published contract
  contradicts it: YES (§4; §2's NEVER rows; §5 P-2/P-8/P-9).
- ADR required? NO — nothing here changes a §22.1 ADR's scope; this
  record is the SDK scope note (§1).

## 8. Related records

- `01_readiness_check.md` — the code-existence evidence this freeze relies on.
- `03_sdk_license_audit.md` — D4 follow-through for the SDK stack.
- `04_seam_inventory.md` — the contract → provider → consumer map for the
  IN-scope contracts, including the protected non-seams.
